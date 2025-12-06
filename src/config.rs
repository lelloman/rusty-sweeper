use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub monitor: MonitorConfig,
    pub cleaner: CleanerConfig,
    pub scanner: ScannerConfig,
    pub tui: TuiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MonitorConfig {
    /// Check interval in seconds
    pub interval: u64,
    /// Warning threshold percentage (0-100)
    pub warn_threshold: u8,
    /// Critical threshold percentage (0-100)
    pub critical_threshold: u8,
    /// Mount points to monitor (empty = all)
    pub mount_points: Vec<PathBuf>,
    /// Notification backend: auto, dbus, notify-send, stderr
    pub notification_backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CleanerConfig {
    /// Project types to clean
    pub project_types: Vec<String>,
    /// Glob patterns to exclude
    pub exclude_patterns: Vec<String>,
    /// Minimum age in days before cleanup
    pub min_age_days: u32,
    /// Maximum scan depth
    pub max_depth: usize,
    /// Parallel clean jobs
    pub parallel_jobs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScannerConfig {
    /// Number of parallel threads (0 = auto)
    pub parallel_threads: usize,
    /// Cross filesystem boundaries
    pub cross_filesystems: bool,
    /// Use result cache
    pub use_cache: bool,
    /// Cache TTL in seconds
    pub cache_ttl: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiConfig {
    /// Color scheme: auto, dark, light, none
    pub color_scheme: String,
    /// Show hidden files by default
    pub show_hidden: bool,
    /// Default sort order: size, name, mtime
    pub default_sort: String,
    /// Size threshold for large directory warning (bytes)
    pub large_dir_threshold: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            monitor: MonitorConfig::default(),
            cleaner: CleanerConfig::default(),
            scanner: ScannerConfig::default(),
            tui: TuiConfig::default(),
        }
    }
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            interval: 300,
            warn_threshold: 80,
            critical_threshold: 90,
            mount_points: vec![],
            notification_backend: "auto".to_string(),
        }
    }
}

impl Default for CleanerConfig {
    fn default() -> Self {
        Self {
            project_types: vec![
                "cargo".to_string(),
                "gradle".to_string(),
                "npm".to_string(),
                "maven".to_string(),
            ],
            exclude_patterns: vec!["**/.git".to_string(), "**/vendor".to_string()],
            min_age_days: 7,
            max_depth: 10,
            parallel_jobs: 4,
        }
    }
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            parallel_threads: 0,
            cross_filesystems: false,
            use_cache: true,
            cache_ttl: 3600,
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            color_scheme: "auto".to_string(),
            show_hidden: false,
            default_sort: "size".to_string(),
            large_dir_threshold: 1024 * 1024 * 1024, // 1 GB
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = Config::default();
        assert_eq!(config.monitor.warn_threshold, 80);
        assert_eq!(config.monitor.critical_threshold, 90);
    }

    #[test]
    fn config_serializes_to_toml() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("[monitor]"));
    }

    #[test]
    fn default_thresholds_are_valid() {
        let config = MonitorConfig::default();
        assert!(config.warn_threshold < config.critical_threshold);
        assert!(config.critical_threshold <= 100);
    }

    #[test]
    fn default_cleaner_has_common_project_types() {
        let config = CleanerConfig::default();
        assert!(config.project_types.contains(&"cargo".to_string()));
        assert!(config.project_types.contains(&"npm".to_string()));
    }
}
