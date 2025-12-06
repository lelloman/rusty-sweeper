use rusty_sweeper::config::Config;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn parse_complete_config_file() {
    let config_content = r#"
[monitor]
interval = 600
warn_threshold = 75
critical_threshold = 85
mount_points = ["/", "/home"]
notification_backend = "dbus"

[cleaner]
project_types = ["cargo", "npm"]
exclude_patterns = ["**/node_modules"]
min_age_days = 14
max_depth = 5
parallel_jobs = 2

[scanner]
parallel_threads = 4
cross_filesystems = true
use_cache = false
cache_ttl = 1800

[tui]
color_scheme = "dark"
show_hidden = true
default_sort = "name"
large_dir_threshold = 536870912
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_content.as_bytes()).unwrap();

    let config = Config::load(Some(file.path())).unwrap();

    assert_eq!(config.monitor.interval, 600);
    assert_eq!(config.monitor.warn_threshold, 75);
    assert_eq!(config.cleaner.min_age_days, 14);
    assert_eq!(config.scanner.parallel_threads, 4);
    assert!(config.tui.show_hidden);
}

#[test]
fn parse_partial_config_uses_defaults() {
    let config_content = r#"
[monitor]
interval = 120
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_content.as_bytes()).unwrap();

    let config = Config::load(Some(file.path())).unwrap();

    // Explicit value
    assert_eq!(config.monitor.interval, 120);
    // Default values
    assert_eq!(config.monitor.warn_threshold, 80);
    assert_eq!(config.cleaner.max_depth, 10);
}

#[test]
fn parse_invalid_toml_returns_error() {
    let config_content = "this is not valid toml [[[";

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_content.as_bytes()).unwrap();

    let result = Config::load(Some(file.path()));
    assert!(result.is_err());
}

#[test]
fn parse_invalid_threshold_returns_error() {
    let config_content = r#"
[monitor]
warn_threshold = 95
critical_threshold = 90
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_content.as_bytes()).unwrap();

    let result = Config::load(Some(file.path()));
    assert!(result.is_err());
}
