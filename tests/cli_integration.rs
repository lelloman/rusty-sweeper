use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::{tempdir, TempDir};

fn fixture() -> TempDir {
    let temp = tempdir().unwrap();
    fs::create_dir(temp.path().join("nested")).unwrap();
    fs::write(temp.path().join("nested/data.txt"), "hello").unwrap();
    fs::write(temp.path().join(".hidden"), "abc").unwrap();
    temp
}

fn command(temp: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rusty-sweeper"));
    command
        .env_remove("RUST_LOG")
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .env("XDG_CACHE_HOME", temp.path().join("cache"));
    command
}

#[test]
fn cli_runs_without_a_terminal_and_defaults_to_working_directory() {
    let temp = fixture();
    command(&temp)
        .current_dir(temp.path())
        .arg("--cli")
        .assert()
        .success()
        .stdout(predicate::str::contains("data.txt"))
        .stdout(predicate::str::contains(".hidden"))
        .stdout(predicate::str::contains("\x1b").not());
}

#[test]
fn json_is_parseable_even_with_verbose_logging_and_contains_totals() {
    let temp = fixture();
    let result = command(&temp)
        .args(["--cli", "--json", "-vv"])
        .arg(temp.path())
        .assert()
        .success();
    let report: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert_eq!(report["size"], 8);
    assert_eq!(report["file_count"], 2);
    assert_eq!(report["children"][0]["children"][0]["name"], "data.txt");
    assert!(String::from_utf8_lossy(&result.get_output().stderr).contains("Loaded configuration"));
}

#[test]
fn display_depth_does_not_limit_scanned_totals() {
    let temp = fixture();
    command(&temp)
        .args(["--cli", "--depth", "0"])
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("8 B"))
        .stdout(predicate::str::contains("nested").not())
        .stdout(predicate::str::contains("data.txt").not());
}

#[test]
fn missing_path_fails_without_report_output() {
    let temp = tempdir().unwrap();
    command(&temp)
        .args(["--cli", "--json"])
        .arg(temp.path().join("missing"))
        .assert()
        .failure()
        .stdout("")
        .stderr(predicate::str::contains("missing"));
}

#[test]
fn incompatible_report_options_are_rejected() {
    let temp = tempdir().unwrap();
    command(&temp)
        .args(["--cli", "--json", "--depth", "1"])
        .assert()
        .code(2);
}

fn clean_fixture() -> TempDir {
    let temp = tempdir().unwrap();
    for name in ["first", "nested/second"] {
        let project = temp.path().join(name);
        fs::create_dir_all(project.join("target")).unwrap();
        fs::write(project.join("Cargo.toml"), "[package]").unwrap();
        fs::write(project.join("target/artifact"), "artifact").unwrap();
    }
    temp
}

#[test]
fn clean_confirmation_reports_then_deletes_only_artifacts() {
    let temp = clean_fixture();
    let result = command(&temp)
        .args(["--cli", "--clean"])
        .arg(temp.path())
        .write_stdin("yes\n")
        .assert()
        .success();
    let output = String::from_utf8_lossy(&result.get_output().stdout);
    assert!(
        output.find("estimated reclaimable size: 16 B").unwrap()
            < output.find("Delete these project artifacts?").unwrap()
    );
    assert!(output.contains("Cleaned: 2; failed: 0; skipped: 0; freed: 16 B"));
    for name in ["first", "nested/second"] {
        assert!(!temp.path().join(name).join("target").exists());
        assert!(temp.path().join(name).join("Cargo.toml").exists());
    }
}

#[test]
fn clean_decline_empty_input_and_eof_preserve_artifacts() {
    for answer in ["n\n", "\n", "", "maybe\n"] {
        let temp = clean_fixture();
        command(&temp)
            .args(["--cli", "--clean"])
            .arg(temp.path())
            .write_stdin(answer)
            .assert()
            .success()
            .stdout(predicate::str::contains("Cancelled"));
        assert!(temp.path().join("first/target/artifact").exists());
        assert!(temp.path().join("nested/second/target/artifact").exists());
    }
}

#[test]
fn clean_yes_reports_without_prompting_and_honors_exclusions() {
    let temp = clean_fixture();
    let config = temp.path().join("settings.toml");
    fs::write(&config, "[cleaner]\nexclude_patterns = ['nested']\n").unwrap();
    command(&temp)
        .args(["--cli", "--clean", "-y", "--config"])
        .arg(&config)
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "1 project(s), estimated reclaimable size: 8 B",
        ))
        .stdout(predicate::str::contains("[y/N]").not());
    assert!(!temp.path().join("first/target").exists());
    assert!(temp.path().join("nested/second/target/artifact").exists());
}

#[test]
fn clean_honors_types_depth_and_age() {
    for setting in [
        "project_types = ['npm']",
        "max_depth = 0",
        "min_age_days = 7",
    ] {
        let temp = clean_fixture();
        let config = temp.path().join("settings.toml");
        fs::write(&config, format!("[cleaner]\n{setting}\n")).unwrap();
        command(&temp)
            .args(["--cli", "--clean", "-y", "--config"])
            .arg(&config)
            .arg(temp.path())
            .assert()
            .success()
            .stdout(predicate::str::contains("No eligible project artifacts"));
        assert!(temp.path().join("first/target/artifact").exists());
        assert!(temp.path().join("nested/second/target/artifact").exists());
    }
}

#[test]
fn clean_root_project_and_partial_failure() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("build.gradle"), "").unwrap();
    fs::create_dir(temp.path().join("build")).unwrap();
    fs::write(temp.path().join("build/artifact"), "hello").unwrap();
    // A detected artifact that isn't a directory forces a partial failure.
    fs::write(temp.path().join(".gradle"), "keep").unwrap();
    command(&temp)
        .args(["--cli", "--clean", "--yes"])
        .arg(temp.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "failed: 1; skipped: 0; freed: 5 B",
        ));
    assert!(!temp.path().join("build").exists());
    assert!(temp.path().join(".gradle").exists());
    assert!(temp.path().join("build.gradle").exists());
}

#[test]
fn clean_rejects_invalid_flags_and_paths() {
    let temp = clean_fixture();
    for args in [
        vec!["--clean"],
        vec!["--cli", "-y"],
        vec!["--cli", "--clean", "--json"],
        vec!["--cli", "--clean", "--depth", "1"],
    ] {
        command(&temp).args(args).arg(temp.path()).assert().code(2);
    }
    for path in ["missing", "first/Cargo.toml"] {
        command(&temp)
            .args(["--cli", "--clean", "-y"])
            .arg(temp.path().join(path))
            .assert()
            .failure();
    }
    assert!(temp.path().join("first/target/artifact").exists());
}
