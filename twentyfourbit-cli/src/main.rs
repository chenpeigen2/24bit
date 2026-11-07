//! Command-line interface for 24bit.net.

use clap::{Parser, Subcommand, ValueEnum};

mod commands;
mod output;

/// Downloads Hi-Res lossless music from 24bit.net.
#[derive(Debug, Parser)]
#[command(
    name = "24bit",
    version,
    about = "Unofficial client for 24bit.net — Hi-Res lossless music",
    long_about = None,
    arg_required_else_help = true,
)]
pub struct Cli {
    /// Emit machine-readable JSON instead of human-readable text.
    #[arg(long, global = true)]
    pub json: bool,

    /// Override the API origin (mainly for testing).
    #[arg(long, global = true, env = "TWENTYFOURBIT_BASE_URL", hide = true)]
    pub base_url: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Search both catalogues for a song or artist.
    Search {
        /// What to look for.
        keyword: String,

        /// Only show this many hits.
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Show the weekly song and artist charts.
    Charts,

    /// Show full detail for one track, including its download URL.
    Track {
        /// Track id, as printed by `search`.
        id: String,

        /// Quality tier to inspect.
        #[arg(short, long, value_enum, default_value_t = QualityArg::Master)]
        quality: QualityArg,
    },

    /// Download a track.
    Download {
        /// Track id, as printed by `search`.
        id: String,

        /// Where to save it. Defaults to the configured download directory.
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,

        /// Quality tier to download.
        #[arg(short, long, value_enum, default_value_t = QualityArg::Master)]
        quality: QualityArg,
    },

    /// Play a track through the default audio device.
    Play {
        /// Track id, as printed by `search`.
        id: String,

        /// Quality tier to play.
        #[arg(short, long, value_enum, default_value_t = QualityArg::Master)]
        quality: QualityArg,

        /// Keep the downloaded file instead of discarding it after playback.
        #[arg(long)]
        keep: bool,
    },

    /// Sign in and store the token in the config file.
    Login {
        /// Account name.
        account: String,

        /// Password.
        #[arg(long, env = "TWENTYFOURBIT_PASSWORD", hide_env_values = true)]
        password: String,
    },

    /// Show or modify stored configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print the effective configuration.
    Show,
    /// Set the preferred quality tier.
    SetQuality {
        #[arg(value_enum)]
        quality: QualityArg,
    },
    /// Print the path of the config file.
    Path,
}

/// Quality tiers, named for humans rather than for the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum QualityArg {
    /// 至臻母带 — highest fidelity, ships lyrics.
    #[value(name = "master")]
    Master,
    /// 高清环绕声 — smaller file, no lyrics.
    #[value(name = "surround")]
    Surround,
}

impl From<QualityArg> for twentyfourbit_core::Quality {
    fn from(q: QualityArg) -> Self {
        match q {
            QualityArg::Master => twentyfourbit_core::Quality::Master,
            QualityArg::Surround => twentyfourbit_core::Quality::Surround,
        }
    }
}

impl QualityArg {
    fn label(self) -> &'static str {
        match self {
            QualityArg::Master => "master",
            QualityArg::Surround => "surround",
        }
    }
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match commands::run(&cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            output::report_error(&err, cli.json);
            std::process::ExitCode::FAILURE
        }
    }
}
