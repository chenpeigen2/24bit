//! Subcommand implementations.

use std::path::PathBuf;

use anyhow::{Context, Result};
use twentyfourbit_core::{Client, Config};

use crate::{output, Cli, Command, ConfigAction};

/// Build a client from the CLI overrides plus the stored config.
fn build_client(cli: &Cli, cfg: &Config) -> Result<Client> {
    let mut builder = Client::builder();
    if let Some(url) = &cli.base_url {
        builder = builder.base_url(url);
    }
    if let Some(token) = &cfg.token {
        builder = builder.token(token);
    }
    builder = builder.ip_placeholder(cfg.ip_placeholder.clone());
    builder.build().context("cannot build HTTP client")
}

fn load_config() -> Result<Config> {
    Config::load().context("cannot load configuration")
}

/// Dispatch one CLI invocation.
pub fn run(cli: &Cli) -> Result<()> {
    let cfg = load_config()?;

    match &cli.command {
        Command::Search { keyword, limit } => {
            let client = build_client(cli, &cfg)?;
            let mut hits = tokio::runtime::Runtime::new()
                .context("cannot start async runtime")?
                .block_on(client.search(keyword))
                .with_context(|| format!("search for {keyword:?} failed"))?;
            if let Some(n) = limit {
                hits.truncate(*n);
            }
            output::print_search(&hits, cli.json);
            Ok(())
        }

        Command::Charts => {
            let client = build_client(cli, &cfg)?;
            let charts = tokio::runtime::Runtime::new()
                .context("cannot start async runtime")?
                .block_on(client.weekly_charts())
                .context("fetching weekly charts failed")?;
            output::print_charts(&charts, cli.json);
            Ok(())
        }

        Command::Track { id, quality } => {
            let client = build_client(cli, &cfg)?;
            let detail = tokio::runtime::Runtime::new()
                .context("cannot start async runtime")?
                .block_on(client.track(id, (*quality).into()))
                .with_context(|| format!("fetching track {id} failed"))?;
            output::print_track(&detail, cli.json);
            Ok(())
        }

        Command::Download { id, output: path, quality } => {
            let rt = tokio::runtime::Runtime::new().context("cannot start async runtime")?;
            let client = build_client(cli, &cfg)?;
            let detail = rt
                .block_on(client.track(id, (*quality).into()))
                .with_context(|| format!("fetching track {id} failed"))?;
            let target = resolve_target(path.clone(), &cfg, &detail.suggested_filename());
            download_one(&rt, &client, &detail, &target, cli.json)
        }

        Command::Play { id, quality, keep } => {
            let rt = tokio::runtime::Runtime::new().context("cannot start async runtime")?;
            let client = build_client(cli, &cfg)?;
            let detail = rt
                .block_on(client.track(id, (*quality).into()))
                .with_context(|| format!("fetching track {id} failed"))?;
            play_one(&rt, &client, &detail, *keep, cli.json)
        }

        Command::Login { account, password } => {
            let rt = tokio::runtime::Runtime::new().context("cannot start async runtime")?;
            let client = build_client(cli, &cfg)?;
            let token = rt
                .block_on(client.login(account, password))
                .context("login failed")?;
            let mut cfg = cfg;
            cfg.token = Some(token);
            cfg.save().context("cannot save token")?;
            if cli.json {
                println!("{}", serde_json::json!({ "status": "ok", "account": account }));
            } else {
                println!("Signed in as {account}. Token stored.");
            }
            Ok(())
        }

        Command::Config { action } => handle_config(action, &cfg, cli.json),
    }
}

fn handle_config(action: &ConfigAction, cfg: &Config, as_json: bool) -> Result<()> {
    match action {
        ConfigAction::Show => {
            if as_json {
                println!("{}", serde_json::to_string_pretty(cfg)?);
            } else {
                println!("quality:         {}", quality_label(cfg.quality));
                println!("ip placeholder:  {}", cfg.ip_placeholder);
                println!(
                    "token:           {}",
                    if cfg.token.is_some() { "set" } else { "not set" }
                );
                println!(
                    "download dir:    {}",
                    cfg.download_dir
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(current directory)".to_owned())
                );
                println!("config file:     {}", Config::default_path().display());
            }
            Ok(())
        }
        ConfigAction::SetQuality { quality } => {
            let mut cfg = cfg.clone();
            cfg.quality = (*quality).into();
            cfg.save().context("cannot save configuration")?;
            if as_json {
                println!("{}", serde_json::json!({ "status": "ok", "quality": quality.label() }));
            } else {
                println!("Preferred quality set to {}.", quality.label());
            }
            Ok(())
        }
        ConfigAction::Path => {
            println!("{}", Config::default_path().display());
            Ok(())
        }
    }
}

fn quality_label(q: twentyfourbit_core::Quality) -> &'static str {
    match q {
        twentyfourbit_core::Quality::Master => "master",
        twentyfourbit_core::Quality::Surround => "surround",
    }
}

/// Decide where a download lands: explicit path, then configured directory,
/// then the suggested filename in the current directory.
fn resolve_target(explicit: Option<PathBuf>, cfg: &Config, suggested: &str) -> PathBuf {
    match explicit {
        Some(p) if p.is_dir() => p.join(suggested),
        Some(p) => p,
        None => cfg
            .download_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(suggested),
    }
}

fn download_one(
    rt: &tokio::runtime::Runtime,
    client: &Client,
    detail: &twentyfourbit_core::TrackDetail,
    target: &std::path::Path,
    as_json: bool,
) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create directory {}", parent.display()))?;
    }

    let bar = if as_json {
        None
    } else {
        Some(indicatif::ProgressBar::new_spinner())
    };

    let result = rt.block_on(client.download_to_with_progress(&detail.download_url, target, {
        let bar = bar.clone();
        move |done, total| {
            if let Some(bar) = &bar {
                match total {
                    Some(total) if total > 0 => {
                        bar.set_length(total);
                        bar.set_position(done);
                    }
                    _ => bar.set_position(done),
                }
            }
        }
    }));

    match result {
        Ok(bytes) => {
            if let Some(bar) = bar {
                bar.finish_and_clear();
            }
            if as_json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "ok",
                        "path": target.display().to_string(),
                        "bytes": bytes,
                        "quality": detail.quality,
                    })
                );
            } else {
                println!(
                    "Saved {} ({}) to {}",
                    detail.name,
                    human_bytes(bytes),
                    target.display()
                );
            }
            Ok(())
        }
        Err(err) => {
            if let Some(bar) = bar {
                bar.abandon();
            }
            Err(err).with_context(|| format!("downloading {}", detail.name))
        }
    }
}

fn play_one(
    rt: &tokio::runtime::Runtime,
    client: &Client,
    detail: &twentyfourbit_core::TrackDetail,
    keep: bool,
    as_json: bool,
) -> Result<()> {
    let target = std::env::temp_dir().join(detail.suggested_filename());

    let bar = if as_json {
        None
    } else {
        Some(indicatif::ProgressBar::new_spinner())
    };
    if let Some(bar) = &bar {
        bar.set_message(format!("fetching {}", detail.name));
    }

    rt.block_on(client.download_to_with_progress(&detail.download_url, &target, {
        let bar = bar.clone();
        move |done, total| {
            if let Some(bar) = &bar {
                if let Some(total) = total.filter(|t| *t > 0) {
                    bar.set_length(total);
                    bar.set_position(done);
                } else {
                    bar.set_position(done);
                }
            }
        }
    }))
    .with_context(|| format!("downloading {}", detail.name))?;

    if let Some(bar) = bar {
        bar.finish_and_clear();
    }

    if !as_json {
        println!("Playing {} — {}", detail.name, detail.player);
    }

    twentyfourbit_player::play_file(&target).with_context(|| format!("playing {}", target.display()))?;

    if keep {
        if !as_json {
            println!("Kept file at {}", target.display());
        }
    } else {
        let _ = std::fs::remove_file(&target);
    }

    if as_json {
        println!("{}", serde_json::json!({ "status": "ok", "played": detail.name }));
    }
    Ok(())
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.2}{}", UNITS[unit])
}
