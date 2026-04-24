use assert_cmd::Command;
use predicates::prelude::*;

fn rusty_sweeper() -> Command {
    Command::cargo_bin("rusty-sweeper").unwrap()
}

#[test]
fn shows_help() {
    rusty_sweeper()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("disk usage management"));
}

#[test]
fn shows_version() {
    rusty_sweeper()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn scan_subcommand_help() {
    rusty_sweeper()
        .args(["scan", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Analyze disk usage"));
}

#[test]
fn clean_subcommand_help() {
    rusty_sweeper()
        .args(["clean", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("build artifacts"));
}

#[test]
fn tui_subcommand_help() {
    rusty_sweeper()
        .args(["tui", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("interactive"));
}

#[test]
fn help_does_not_list_monitor_subcommand() {
    rusty_sweeper()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("tui"))
        .stdout(predicate::str::contains("monitor").not());
}

#[test]
fn verbose_flag_accepted() {
    rusty_sweeper()
        .args(["-vvv", "scan", "."])
        .assert()
        .success();
}

#[test]
fn invalid_config_path_fails() {
    rusty_sweeper()
        .args(["--config", "/nonexistent/path.toml", "scan"])
        .assert()
        .failure();
}

#[test]
fn scan_outputs_tree() {
    rusty_sweeper()
        .args(["scan", "."])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total:"));
}

#[test]
fn scan_with_json_output() {
    rusty_sweeper()
        .args(["scan", "--json", "."])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"));
}

#[test]
fn scan_respects_depth() {
    rusty_sweeper()
        .args(["scan", "-d", "1", "."])
        .assert()
        .success();
}

#[test]
fn clean_size_only_shows_projects() {
    rusty_sweeper()
        .args(["clean", "--size-only", "."])
        .assert()
        .success()
        .stdout(predicate::str::contains("Scanning for projects"));
}

// Note: TUI command is implemented but can't be tested in CI
// because it requires a real terminal. The tui_subcommand_help test
// above verifies the command is recognized.
