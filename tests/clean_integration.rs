//! Integration tests for the clean command.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn rusty_sweeper() -> Command {
    Command::cargo_bin("rusty-sweeper").unwrap()
}

/// Create a realistic test environment with multiple project types.
fn create_test_workspace() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Rust/Cargo project
    let rust_proj = root.join("rust-app");
    fs::create_dir_all(rust_proj.join("src")).unwrap();
    fs::write(
        rust_proj.join("Cargo.toml"),
        r#"
[package]
name = "rust-app"
version = "0.1.0"
"#,
    )
    .unwrap();
    fs::write(rust_proj.join("src/main.rs"), "fn main() {}").unwrap();
    fs::create_dir_all(rust_proj.join("target/debug")).unwrap();
    fs::write(rust_proj.join("target/debug/rust-app"), "x".repeat(50000)).unwrap();
    fs::write(rust_proj.join("target/debug/deps.rlib"), "x".repeat(30000)).unwrap();

    // Node.js/npm project
    let node_proj = root.join("web-app");
    fs::create_dir_all(&node_proj).unwrap();
    fs::write(node_proj.join("package.json"), r#"{"name": "web-app"}"#).unwrap();
    fs::write(node_proj.join("index.js"), "console.log('hi')").unwrap();
    fs::create_dir_all(node_proj.join("node_modules/lodash")).unwrap();
    fs::write(
        node_proj.join("node_modules/lodash/index.js"),
        "x".repeat(20000),
    )
    .unwrap();

    // Gradle/Android project
    let gradle_proj = root.join("android-app");
    fs::create_dir_all(&gradle_proj).unwrap();
    fs::write(gradle_proj.join("build.gradle"), "apply plugin: 'android'").unwrap();
    fs::create_dir_all(gradle_proj.join("build/outputs")).unwrap();
    fs::write(
        gradle_proj.join("build/outputs/app.apk"),
        "x".repeat(100000),
    )
    .unwrap();
    fs::create_dir_all(gradle_proj.join(".gradle/caches")).unwrap();
    fs::write(
        gradle_proj.join(".gradle/caches/cache.bin"),
        "x".repeat(40000),
    )
    .unwrap();

    // Regular directory (not a project)
    let docs = root.join("docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(docs.join("readme.md"), "# Documentation").unwrap();

    // Nested project (should be found)
    let nested = root.join("projects/libs/util-lib");
    fs::create_dir_all(nested.join("src")).unwrap();
    fs::write(nested.join("Cargo.toml"), "[package]\nname = \"util\"").unwrap();
    fs::create_dir_all(nested.join("target/release")).unwrap();
    fs::write(nested.join("target/release/libutil.so"), "x".repeat(25000)).unwrap();

    tmp
}

#[test]
fn test_scan_finds_all_projects() {
    let tmp = create_test_workspace();

    rusty_sweeper()
        .args(["clean", "--size-only"])
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("cargo"))
        .stdout(predicate::str::contains("npm"))
        .stdout(predicate::str::contains("gradle"))
        .stdout(predicate::str::contains("4 project"));
}

#[test]
fn test_clean_removes_artifacts() {
    let tmp = create_test_workspace();

    rusty_sweeper()
        .args(["clean", "--force"])
        .arg(tmp.path())
        .assert()
        .success();

    // Artifacts should be gone
    assert!(!tmp.path().join("rust-app/target").exists());
    assert!(!tmp.path().join("web-app/node_modules").exists());
    assert!(!tmp.path().join("android-app/build").exists());
    assert!(!tmp.path().join("projects/libs/util-lib/target").exists());

    // Source files should remain
    assert!(tmp.path().join("rust-app/src/main.rs").exists());
    assert!(tmp.path().join("web-app/index.js").exists());
    assert!(tmp.path().join("docs/readme.md").exists());
}

#[test]
fn test_type_filtering() {
    let tmp = create_test_workspace();

    rusty_sweeper()
        .args(["clean", "--force", "--types=cargo"])
        .arg(tmp.path())
        .assert()
        .success();

    // Only cargo artifacts removed
    assert!(!tmp.path().join("rust-app/target").exists());
    assert!(!tmp.path().join("projects/libs/util-lib/target").exists());

    // Other project artifacts remain
    assert!(tmp.path().join("web-app/node_modules").exists());
    assert!(tmp.path().join("android-app/build").exists());
}

#[test]
fn test_exclude_patterns() {
    let tmp = create_test_workspace();

    rusty_sweeper()
        .args(["clean", "--force", "--exclude=projects"])
        .arg(tmp.path())
        .assert()
        .success();

    // Excluded project should remain
    assert!(tmp.path().join("projects/libs/util-lib/target").exists());

    // Other projects cleaned
    assert!(!tmp.path().join("rust-app/target").exists());
}

#[test]
fn test_dry_run_preserves_all() {
    let tmp = create_test_workspace();

    // Verify target exists before
    assert!(tmp.path().join("rust-app/target").exists());

    rusty_sweeper()
        .args(["clean", "--dry-run"])
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("[DRY RUN]"));

    // Everything still exists
    assert!(tmp.path().join("rust-app/target").exists());
    assert!(tmp.path().join("web-app/node_modules").exists());
    assert!(tmp.path().join("android-app/build").exists());
}

#[test]
fn test_size_only_does_not_clean() {
    let tmp = create_test_workspace();

    rusty_sweeper()
        .args(["clean", "--size-only"])
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Total:"));

    // Everything should still exist
    assert!(tmp.path().join("rust-app/target").exists());
    assert!(tmp.path().join("web-app/node_modules").exists());
}

#[test]
fn test_size_calculation() {
    let tmp = create_test_workspace();

    rusty_sweeper()
        .args(["clean", "--size-only"])
        .arg(tmp.path())
        .assert()
        .success()
        // Should show human-readable sizes
        .stdout(predicate::str::contains("KiB").or(predicate::str::contains("MiB")));
}

#[test]
fn test_empty_directory() {
    let tmp = TempDir::new().unwrap();

    rusty_sweeper()
        .args(["clean", "--size-only"])
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No projects"));
}

#[test]
fn test_max_depth_limiting() {
    let tmp = create_test_workspace();

    rusty_sweeper()
        .args(["clean", "--size-only", "--max-depth=1"])
        .arg(tmp.path())
        .assert()
        .success()
        // Should find top-level projects but not nested one
        .stdout(predicate::str::contains("3 project"));
}

#[test]
fn test_invalid_types_filter() {
    let tmp = create_test_workspace();

    rusty_sweeper()
        .args(["clean", "--size-only", "--types=nonexistent"])
        .arg(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No valid project types"));
}

#[test]
fn test_age_filter_excludes_recent() {
    let tmp = create_test_workspace();

    rusty_sweeper()
        .args(["clean", "--size-only", "--age=1"])
        .arg(tmp.path())
        .assert()
        .success()
        // All projects are recent, so should be filtered out
        .stdout(predicate::str::contains("none older than"));
}

#[test]
fn test_multiple_artifact_directories() {
    let tmp = create_test_workspace();

    // Gradle project should have multiple artifact dirs
    rusty_sweeper()
        .args(["clean", "--force", "--types=gradle"])
        .arg(tmp.path())
        .assert()
        .success();

    // Both build/ and .gradle/ should be removed
    assert!(!tmp.path().join("android-app/build").exists());
    assert!(!tmp.path().join("android-app/.gradle").exists());
}

#[test]
fn test_parallel_cleaning() {
    let tmp = create_test_workspace();

    // Test with different job counts
    rusty_sweeper()
        .args(["clean", "--force", "--jobs=1"])
        .arg(tmp.path())
        .assert()
        .success();

    // All artifacts should be cleaned
    assert!(!tmp.path().join("rust-app/target").exists());
    assert!(!tmp.path().join("web-app/node_modules").exists());
}

#[test]
fn test_help_output() {
    rusty_sweeper()
        .args(["clean", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--types"))
        .stdout(predicate::str::contains("--exclude"))
        .stdout(predicate::str::contains("--age"))
        .stdout(predicate::str::contains("--force"))
        .stdout(predicate::str::contains("--jobs"))
        .stdout(predicate::str::contains("--size-only"));
}
