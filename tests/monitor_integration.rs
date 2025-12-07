use assert_cmd::Command;
use predicates::prelude::*;

fn rusty_sweeper() -> Command {
    Command::cargo_bin("rusty-sweeper").unwrap()
}

#[test]
fn test_monitor_help() {
    rusty_sweeper()
        .args(["monitor", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--daemon"))
        .stdout(predicate::str::contains("--interval"))
        .stdout(predicate::str::contains("--warn"))
        .stdout(predicate::str::contains("--critical"))
        .stdout(predicate::str::contains("--mount"))
        .stdout(predicate::str::contains("--once"))
        .stdout(predicate::str::contains("--stop"))
        .stdout(predicate::str::contains("--status"))
        .stdout(predicate::str::contains("--notify"));
}

#[test]
fn test_monitor_once() {
    // Run monitor once with stderr backend to avoid D-Bus issues in CI
    rusty_sweeper()
        .args(["monitor", "--once", "--notify", "stderr"])
        .assert()
        .success();
}

#[test]
fn test_monitor_once_with_custom_mount() {
    rusty_sweeper()
        .args(["monitor", "--once", "--notify", "stderr", "--mount", "/"])
        .assert()
        .success();
}

#[test]
fn test_monitor_once_multiple_mounts() {
    rusty_sweeper()
        .args([
            "monitor", "--once", "--notify", "stderr", "--mount", "/", "--mount", "/home",
        ])
        .assert()
        .success();
}

#[test]
fn test_monitor_invalid_thresholds_warn_gt_critical() {
    rusty_sweeper()
        .args([
            "monitor",
            "--once",
            "--notify",
            "stderr",
            "--warn",
            "90",
            "--critical",
            "80",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("threshold"));
}

#[test]
fn test_monitor_invalid_thresholds_warn_eq_critical() {
    rusty_sweeper()
        .args([
            "monitor",
            "--once",
            "--notify",
            "stderr",
            "--warn",
            "85",
            "--critical",
            "85",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("threshold"));
}

#[test]
fn test_monitor_invalid_thresholds_over_100() {
    rusty_sweeper()
        .args([
            "monitor",
            "--once",
            "--notify",
            "stderr",
            "--warn",
            "80",
            "--critical",
            "120",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("threshold").or(predicate::str::contains("100")));
}

#[test]
fn test_monitor_status_not_running() {
    rusty_sweeper()
        .args(["monitor", "--status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not running"));
}

#[test]
fn test_monitor_stop_not_running() {
    rusty_sweeper()
        .args(["monitor", "--stop"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No monitor daemon running"));
}

#[test]
fn test_monitor_stderr_backend() {
    // Use a very low threshold to ensure we get output
    rusty_sweeper()
        .args([
            "monitor",
            "--once",
            "--notify",
            "stderr",
            "--warn",
            "1",
            "--critical",
            "2",
        ])
        .assert()
        .success();
}

#[test]
fn test_monitor_unknown_backend() {
    rusty_sweeper()
        .args(["monitor", "--once", "--notify", "invalid-backend"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown notification backend"));
}

#[test]
fn test_monitor_custom_interval() {
    rusty_sweeper()
        .args([
            "monitor",
            "--once",
            "--notify",
            "stderr",
            "--interval",
            "60",
        ])
        .assert()
        .success();
}
