//! Output formatting.
//!
//! Human output goes to stdout for data and stderr for errors, so that
//! `24bit search foo > hits.txt` never mixes the two. In `--json` mode every
//! result — success or failure — is a single JSON document on stdout.

use serde_json::json;
use twentyfourbit_core::{SearchItem, TrackDetail, WeeklyCharts};

/// Print a search result set.
pub fn print_search(items: &[SearchItem], as_json: bool) {
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "results": items })).unwrap()
        );
        return;
    }
    if items.is_empty() {
        println!("No results.");
        return;
    }
    for (i, item) in items.iter().enumerate() {
        println!("{:>3}. {} — {}", i + 1, item.name, item.player);
        println!("     album: {}", item.album);
        println!("     id:    {}", item.id);
    }
}

/// Print one track's detail.
pub fn print_track(d: &TrackDetail, as_json: bool) {
    if as_json {
        println!("{}", serde_json::to_string_pretty(d).unwrap());
        return;
    }
    println!("{} — {}", d.name, d.player);
    println!("  album:   {}", d.album);
    println!("  quality: {}", d.quality);
    println!("  format:  {}", d.format);
    println!("  size:    {}", d.size);
    println!("  id:      {}", d.id);
    match d.download_url_expires_at() {
        Some(exp) => println!("  link expires: {exp}"),
        None => println!("  link expires: unknown"),
    }
    if d.lrc.is_some() {
        println!("  lyrics:  embedded");
    }
}

/// Print the weekly charts.
pub fn print_charts(charts: &WeeklyCharts, as_json: bool) {
    if as_json {
        println!("{}", serde_json::to_string_pretty(charts).unwrap());
        return;
    }
    println!("Weekly songs");
    for (i, row) in charts.songs.iter().enumerate() {
        println!("{:>3}. {:<32} {:>8} plays", i + 1, row.name, row.plays);
    }
    println!("\nWeekly artists");
    for (i, row) in charts.artists.iter().enumerate() {
        println!("{:>3}. {:<32} {:>8} plays", i + 1, row.name, row.plays);
    }
}

/// Report an error, structured in JSON mode and human-readable otherwise.
pub fn report_error(err: &anyhow::Error, as_json: bool) {
    if as_json {
        let mut detail = json!({ "error": err.to_string() });
        if let Some(core) = err.downcast_ref::<twentyfourbit_core::Error>() {
            detail["kind"] = json!(core_kind(core));
            if core.is_retryable_with_fresh_link() {
                detail["retryable"] = json!(true);
            }
        }
        println!("{}", serde_json::to_string_pretty(&detail).unwrap());
        return;
    }

    eprintln!("error: {err}");
    for cause in err.chain().skip(1) {
        eprintln!("  caused by: {cause}");
    }
    if let Some(core) = err.downcast_ref::<twentyfourbit_core::Error>() {
        if core.is_retryable_with_fresh_link() {
            eprintln!("  hint: the signed link expired; run the command again to get a fresh one");
        }
    }
}

fn core_kind(err: &twentyfourbit_core::Error) -> &'static str {
    use twentyfourbit_core::Error::*;
    match err {
        Api { .. } => "api",
        Network(_) => "network",
        Http { .. } => "http",
        ExpiredLink { .. } => "expired_link",
        InvalidToken { .. } => "invalid_token",
        Decode(_) => "decode",
        Io { .. } => "io",
        Config { .. } => "config",
        ConfigEncode { .. } => "config_encode",
    }
}
