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
fn requires_subcommand() {
    rusty_sweeper()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:"));
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
fn monitor_subcommand_help() {
    rusty_sweeper()
        .args(["monitor", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("monitoring"));
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

#[test]
fn monitor_prints_not_implemented() {
    rusty_sweeper()
        .args(["monitor", "--once"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
}

#[test]
fn tui_prints_not_implemented() {
    rusty_sweeper()
        .args(["tui", "."])
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
}
