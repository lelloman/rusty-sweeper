use crate::error::{ConfigError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub cleaner: CleanerConfig,
    pub scanner: ScannerConfig,
    pub tui: TuiConfig,
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

impl Default for CleanerConfig {
    fn default() -> Self {
        Self {
            project_types: vec![
                "cargo".to_string(),
                "gradle".to_string(),
                "maven".to_string(),
                "npm".to_string(),
                "go".to_string(),
                "cmake".to_string(),
                "python".to_string(),
                "bazel".to_string(),
                "dotnet".to_string(),
            ],
            exclude_patterns: vec!["**/.git".to_string(), "**/vendor".to_string()],
            min_age_days: 0,
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
            show_hidden: true,
            default_sort: "size".to_string(),
            large_dir_threshold: 1024 * 1024 * 1024, // 1 GB
        }
    }
}

impl Config {
    /// Load configuration from file, falling back to defaults
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        // If explicit path provided, it must exist
        if let Some(path) = config_path {
            return Self::load_from_file(path);
        }

        // Try XDG config locations
        if let Some(path) = Self::find_config_file() {
            return Self::load_from_file(&path);
        }

        // No config file found, use defaults
        Ok(Self::default())
    }

    /// Find config file in standard locations
    fn find_config_file() -> Option<PathBuf> {
        // Try XDG_CONFIG_HOME first
        if let Some(config_dir) = dirs::config_dir() {
            let path = config_dir.join("rusty-sweeper").join("config.toml");
            if path.exists() {
                return Some(path);
            }
        }

        // Try system-wide config
        let system_path = PathBuf::from("/etc/rusty-sweeper/config.toml");
        if system_path.exists() {
            return Some(system_path);
        }

        None
    }

    /// Load config from a specific file
    fn load_from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadError {
            path: path.to_path_buf(),
            source: e,
        })?;

        let config: Config = toml::from_str(&contents).map_err(|e| ConfigError::ParseError {
            path: path.to_path_buf(),
            source: e,
        })?;

        Ok(config)
    }

    /// Get the default config file path (for --config help text)
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("rusty-sweeper").join("config.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_serializes_to_toml() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("[scanner]"));
    }

    #[test]
    fn default_cleaner_has_common_project_types() {
        let config = CleanerConfig::default();
        assert!(config.project_types.contains(&"cargo".to_string()));
        assert!(config.project_types.contains(&"npm".to_string()));
    }

    #[test]
    fn load_returns_defaults_when_no_file() {
        let config = Config::load(None).unwrap();
        assert_eq!(config.scanner.cache_ttl, 3600);
    }

    #[test]
    fn load_fails_on_invalid_explicit_path() {
        let result = Config::load(Some(Path::new("/nonexistent/config.toml")));
        assert!(result.is_err());
    }
}
