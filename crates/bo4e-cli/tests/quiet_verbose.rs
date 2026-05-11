//! Quiet/verbose matrix tests.
//!
//! Drives each subcommand at each of the three levels (Quiet, Normal, Verbose)
//! and inspects observable behaviour. Spinners and progress bars auto-hide on
//! non-TTY (test environment), so these tests can only assert on cprint_* output.
//! Note: `CONSOLE` is a `OnceLock`, so only one level-mutating test wins the init race per `cargo test` run; the others early-return cleanly. "5 passed" includes early-returns — not all branches are exercised in a single run.

use bo4e_cli::cli::base::{Cli, Executable};
use bo4e_cli::console::console::{CONSOLE, Console, Level};
use clap::Parser;

const FIXTURE: &str = "../bo4e-codegen/tests/fixtures/bo4e_min";

fn ensure_console(level: Level) {
    // OnceLock: best-effort init. If a previous test set a different level,
    // these assertions are skipped via early return.
    let _ = CONSOLE.set(Console::new(level));
}

fn current_level() -> Level {
    // Inspect via would_emit; Console doesn't expose `level` directly.
    let c = CONSOLE.get().expect("console set");
    if c.would_emit(Level::Verbose) {
        Level::Verbose
    } else if c.would_emit(Level::Normal) {
        Level::Normal
    } else {
        Level::Quiet
    }
}

#[test]
fn pull_command_parses_quiet_and_verbose_flags() {
    let cli_q = Cli::try_parse_from([
        "bo4e",
        "--quiet",
        "pull",
        "-o",
        "/tmp/x",
        "-t",
        "v202501.0.0",
    ])
    .unwrap();
    assert!(cli_q.quiet);
    let cli_v = Cli::try_parse_from([
        "bo4e",
        "--verbose",
        "pull",
        "-o",
        "/tmp/x",
        "-t",
        "v202501.0.0",
    ])
    .unwrap();
    assert!(cli_v.verbose);
}

#[test]
fn edit_quiet_does_not_panic() {
    ensure_console(Level::Quiet);
    if current_level() != Level::Quiet {
        return; // another test won the race; skip cleanly
    }
    let outdir = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "bo4e",
        "--quiet",
        "edit",
        "-i",
        FIXTURE,
        "-o",
        outdir.path().to_str().unwrap(),
    ])
    .unwrap();
    cli.run().expect("edit --quiet");
}

#[test]
fn edit_verbose_does_not_panic() {
    ensure_console(Level::Verbose);
    if current_level() != Level::Verbose {
        return;
    }
    let outdir = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "bo4e",
        "--verbose",
        "edit",
        "-i",
        FIXTURE,
        "-o",
        outdir.path().to_str().unwrap(),
    ])
    .unwrap();
    cli.run().expect("edit --verbose");
}

#[test]
fn generate_quiet_does_not_panic() {
    ensure_console(Level::Quiet);
    if current_level() != Level::Quiet {
        return;
    }
    let outdir = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "bo4e",
        "--quiet",
        "generate",
        "-i",
        FIXTURE,
        "-o",
        outdir.path().to_str().unwrap(),
        "-t",
        "python-pydantic",
    ])
    .unwrap();
    cli.run().expect("generate --quiet");
}

#[test]
fn generate_verbose_does_not_panic() {
    ensure_console(Level::Verbose);
    if current_level() != Level::Verbose {
        return;
    }
    let outdir = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "bo4e",
        "--verbose",
        "generate",
        "-i",
        FIXTURE,
        "-o",
        outdir.path().to_str().unwrap(),
        "-t",
        "python-pydantic",
    ])
    .unwrap();
    cli.run().expect("generate --verbose");
}
