use std::fs;
use std::path::PathBuf;

fn repository_file(path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    fs::read_to_string(path).unwrap()
}

#[test]
fn package_ships_only_main_binary() {
    let pkgbuild = repository_file("dist/PKGBUILD");
    let release = repository_file(".github/workflows/release.yml");
    assert!(pkgbuild.contains("target/release/$pkgname"));
    assert!(release.contains("target/release/rusty-sweeper dist/pkg/"));
    assert!(!pkgbuild.contains("rusty-sweeper-monitor"));
    assert!(!release.contains("rusty-sweeper-monitor"));
}
