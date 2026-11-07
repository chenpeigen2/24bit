//! Behavioural tests for the CLI surface.
//!
//! The CLI is exercised as a real binary, because the contract that matters is
//! what a user sees: argument parsing, output format, exit codes.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_24bit"))
}

/// `--help` must work without network access and list the commands.
#[test]
fn help_lists_commands() {
    let out = bin().arg("--help").output().unwrap();
    assert!(out.status.success(), "help must exit 0");
    let text = String::from_utf8_lossy(&out.stdout);
    for cmd in ["search", "charts", "track", "download", "play", "login", "config"] {
        assert!(text.contains(cmd), "help should mention `{cmd}`:\n{text}");
    }
}

/// `--version` reports the crate version.
#[test]
fn version_is_reported() {
    let out = bin().arg("--version").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(env!("CARGO_PKG_VERSION")), "got {text}");
}

/// A bare invocation is a usage error, not a panic and not a silent no-op.
#[test]
fn no_arguments_is_a_usage_error() {
    let out = bin().output().unwrap();
    assert!(!out.status.success(), "bare invocation must fail");
    // clap's exit code for usage errors is 2.
    assert_eq!(out.status.code(), Some(2), "usage errors exit with 2");
}

/// Unknown subcommands fail cleanly.
#[test]
fn unknown_command_fails_cleanly() {
    let out = bin().arg("nonsense").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(!text.contains("panicked"), "must not panic:\n{text}");
}

/// `search` requires a keyword.
#[test]
fn search_requires_keyword() {
    let out = bin().arg("search").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("keyword") || text.contains("KEYWORD"), "got:\n{text}");
}

/// `search` accepts `--json` so it can be composed with other tools, and the
/// output must be valid JSON on stdout.
#[test]
fn search_json_is_machine_readable() {
    let out = bin()
        .args(["search", "富士山下", "--json"])
        .env("TWENTYFOURBIT_BASE_URL", "http://127.0.0.1:1")
        .output()
        .unwrap();

    // The network call fails (dead port), but the *error* must still be
    // structured when JSON output was requested.
    assert!(!out.status.success(), "unreachable host must fail");
    let text = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("stdout was not JSON ({e}): {text}"));
    assert!(parsed.get("error").is_some(), "JSON error object expected, got {parsed}");
}

/// `config` shows the effective configuration and works offline.
#[test]
fn config_prints_defaults_offline() {
    let dir = std::env::temp_dir().join(format!("24bit-cli-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let out = bin()
        .args(["config", "show"])
        .env("XDG_CONFIG_HOME", &dir)
        .env("TWENTYFOURBIT_BASE_URL", "http://127.0.0.1:1")
        .output()
        .unwrap();

    assert!(out.status.success(), "config show must not need the network");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("quality"), "got:\n{text}");
    assert!(text.contains("master"), "default quality is master:\n{text}");
}
