//! Integration tests for the scan command

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

fn rusty_sweeper() -> Command {
    Command::cargo_bin("rusty-sweeper").unwrap()
}

fn create_test_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Create a mock project structure
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("target/debug")).unwrap();

    File::create(root.join("Cargo.toml"))
        .unwrap()
        .write_all(b"[package]\nname = \"test\"")
        .unwrap();

    File::create(root.join("src/main.rs"))
        .unwrap()
        .write_all(b"fn main() {}")
        .unwrap();

    // Create some "build artifacts"
    for i in 0..10 {
        let mut f = File::create(root.join(format!("target/debug/artifact{}.o", i))).unwrap();
        f.write_all(&vec![0u8; 10240]).unwrap(); // 10KB each
    }

    dir
}

#[test]
fn test_scan_basic() {
    let dir = create_test_project();

    rusty_sweeper()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("target/"));
}

#[test]
fn test_scan_json_output() {
    let dir = create_test_project();

    rusty_sweeper()
        .arg("scan")
        .arg("--json")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"));
}

#[test]
fn test_scan_max_depth() {
    let dir = create_test_project();

    // With depth 1, we should see target/ but not target/debug/
    rusty_sweeper()
        .arg("scan")
        .arg("-d")
        .arg("1")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("target/"))
        .stdout(predicate::str::contains("artifact").not());
}

#[test]
fn test_scan_hidden_files() {
    let dir = TempDir::new().unwrap();
    File::create(dir.path().join(".hidden")).unwrap();
    File::create(dir.path().join("visible")).unwrap();

    // Without -a flag
    rusty_sweeper()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(".hidden").not());

    // With -a flag
    rusty_sweeper()
        .arg("scan")
        .arg("-a")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(".hidden"));
}

#[test]
fn test_scan_nonexistent_path() {
    rusty_sweeper()
        .arg("scan")
        .arg("/nonexistent/path/12345")
        .assert()
        .failure();
}

#[test]
fn test_scan_shows_total() {
    let dir = create_test_project();

    rusty_sweeper()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Total:"))
        .stdout(predicate::str::contains("files"))
        .stdout(predicate::str::contains("directories"));
}

#[test]
fn test_scan_json_has_required_fields() {
    let dir = TempDir::new().unwrap();
    File::create(dir.path().join("test.txt"))
        .unwrap()
        .write_all(b"hello")
        .unwrap();

    let output = rusty_sweeper()
        .arg("scan")
        .arg("--json")
        .arg(dir.path())
        .output()
        .unwrap();

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(json["path"].is_string());
    assert!(json["size"].is_number());
    assert!(json["is_dir"].is_boolean());
    assert!(json["file_count"].is_number());
    assert!(json["children"].is_array());
}

#[test]
fn test_scan_top_n_limits() {
    let dir = TempDir::new().unwrap();

    // Create 10 files
    for i in 0..10 {
        File::create(dir.path().join(format!("file{}.txt", i)))
            .unwrap()
            .write_all(b"content")
            .unwrap();
    }

    // With top 3, should mention "more entries"
    rusty_sweeper()
        .arg("scan")
        .arg("-n")
        .arg("3")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("more entries"));
}

#[test]
fn test_scan_sort_by_name() {
    let dir = TempDir::new().unwrap();

    // Create files with specific sizes
    File::create(dir.path().join("aaa.txt"))
        .unwrap()
        .write_all(&vec![0u8; 100])
        .unwrap();
    File::create(dir.path().join("zzz.txt"))
        .unwrap()
        .write_all(&vec![0u8; 1000])
        .unwrap();

    // Sort by name - aaa should appear before zzz regardless of size
    let output = rusty_sweeper()
        .arg("scan")
        .arg("--sort")
        .arg("name")
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let aaa_pos = stdout.find("aaa.txt").unwrap();
    let zzz_pos = stdout.find("zzz.txt").unwrap();
    assert!(aaa_pos < zzz_pos);
}

#[test]
fn test_scan_sort_by_size_default() {
    let dir = TempDir::new().unwrap();

    // Create files with specific sizes
    File::create(dir.path().join("small.txt"))
        .unwrap()
        .write_all(&vec![0u8; 100])
        .unwrap();
    File::create(dir.path().join("large.txt"))
        .unwrap()
        .write_all(&vec![0u8; 10000])
        .unwrap();

    // Default sort is by size - large should appear first
    let output = rusty_sweeper()
        .arg("scan")
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let large_pos = stdout.find("large.txt").unwrap();
    let small_pos = stdout.find("small.txt").unwrap();
    assert!(large_pos < small_pos);
}

#[test]
fn test_scan_nested_directories() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Create nested structure
    fs::create_dir_all(root.join("a/b/c")).unwrap();
    File::create(root.join("a/b/c/deep.txt"))
        .unwrap()
        .write_all(b"deep content")
        .unwrap();

    rusty_sweeper()
        .arg("scan")
        .arg("-d")
        .arg("10")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("deep.txt"));
}

#[test]
fn test_scan_empty_directory() {
    let dir = TempDir::new().unwrap();

    rusty_sweeper()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("0 files"));
}

#[test]
fn test_scan_with_verbose_flag() {
    let dir = TempDir::new().unwrap();

    rusty_sweeper()
        .arg("-v")
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success();
}
