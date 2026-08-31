use std::fs;
use std::path::PathBuf;

fn repository_file(path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    fs::read_to_string(path).unwrap()
}

#[test]
fn package_installs_monitor_at_service_executable_path() {
    let pkgbuild = repository_file("dist/PKGBUILD");
    let service = repository_file("dist/rusty-sweeper-monitor.service");

    assert!(
        pkgbuild.contains("target/release/rusty-sweeper-monitor"),
        "the package must install the monitor binary"
    );
    assert!(
        service.contains("ExecStart=/usr/bin/rusty-sweeper-monitor"),
        "the packaged service must use the distro binary path"
    );
}
