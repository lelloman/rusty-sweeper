# Phase 1: Foundation - Implementation Plan

## Overview

**Goal**: Establish project scaffold and core infrastructure that all other phases will build upon.

**Estimated Tasks**: 12 tasks across 5 areas

---

## Task Status Legend

- `[ ]` - Not started
- `[~]` - In progress
- `[x]` - Completed

---

## 1. Project Setup

### Task 1.1: Initialize Cargo Project

**Status**: `[x]`

**Description**: Create the Cargo project with appropriate metadata and initial structure.

**Context**: We're building a single binary crate (not a workspace) since all components are tightly coupled. The binary will be named `rusty-sweeper`.

**Actions**:
1. Run `cargo init` in the project directory
2. Configure `Cargo.toml` with metadata
3. Create initial directory structure

**Sample `Cargo.toml`**:
```toml
[package]
name = "rusty-sweeper"
version = "0.1.0"
edition = "2021"
authors = ["Your Name <your.email@example.com>"]
description = "A Linux disk usage management utility"
license = "MIT OR Apache-2.0"
repository = "https://github.com/username/rusty-sweeper"
keywords = ["disk", "cleanup", "tui", "linux", "utility"]
categories = ["command-line-utilities", "filesystem"]

[dependencies]
# To be added incrementally

[dev-dependencies]
# To be added incrementally

[[bin]]
name = "rusty-sweeper"
path = "src/main.rs"
```

**Directory Structure**:
```
rusty-sweeper/
├── Cargo.toml
├── SPEC.md
├── PHASE1_PLAN.md
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── cli.rs
│   ├── config.rs
│   └── error.rs
```

**Tests**:
- `cargo check` passes
- `cargo build` produces binary

---

### Task 1.2: Add Core Dependencies

**Status**: `[x]`

**Description**: Add all dependencies needed for Phase 1 to `Cargo.toml`.

**Context**: We add dependencies incrementally to keep compile times manageable during development. Phase 1 needs CLI parsing, configuration, error handling, and logging.

**Dependencies to add**:
```toml
[dependencies]
# CLI
clap = { version = "4", features = ["derive", "env"] }

# Configuration
serde = { version = "1", features = ["derive"] }
toml = "0.8"
dirs = "5"

# Error handling
thiserror = "1"
anyhow = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
predicates = "3"
```

**Tests**:
- `cargo check` passes with all dependencies
- No conflicting versions

---

## 2. Error Handling

### Task 2.1: Define Error Types

**Status**: `[x]`

**Description**: Create custom error types using `thiserror` for all anticipated error conditions.

**Context**: Good error types are foundational. We define them early so all modules can use consistent error handling. We use `thiserror` for library errors and `anyhow` for application-level error propagation.

**File**: `src/error.rs`

**Sample Implementation**:
```rust
use std::path::PathBuf;
use thiserror::Error;

/// Core library errors
#[derive(Error, Debug)]
pub enum SweeperError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("IO error at path '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("Path not found: {0}")]
    PathNotFound(PathBuf),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Configuration-specific errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file '{path}': {source}")]
    ReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse config file '{path}': {source}")]
    ParseError {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("Invalid configuration: {0}")]
    Invalid(String),
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, SweeperError>;
```

**Tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = ConfigError::Invalid("threshold must be 0-100".into());
        assert!(err.to_string().contains("threshold"));
    }

    #[test]
    fn error_conversion() {
        let config_err = ConfigError::Invalid("test".into());
        let sweeper_err: SweeperError = config_err.into();
        assert!(matches!(sweeper_err, SweeperError::Config(_)));
    }
}
```

---

## 3. Configuration

### Task 3.1: Define Configuration Structures

**Status**: `[x]`

**Description**: Define Serde-compatible structs representing the TOML configuration schema.

**Context**: Configuration structures mirror the TOML format from the spec. All fields have sensible defaults using `#[serde(default)]` so the tool works without a config file.

**File**: `src/config.rs`

**Sample Implementation**:
```rust
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
```

**Tests**:
```rust
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
}
```

---

### Task 3.2: Implement Default Trait for Config Structs

**Status**: `[x]`

**Description**: Implement `Default` for all config structs with sensible values from the spec.

**Context**: Users should be able to run `rusty-sweeper` without any configuration file. All defaults come from the spec document.

**Sample Implementation** (add to `src/config.rs`):
```rust
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
            exclude_patterns: vec![
                "**/.git".to_string(),
                "**/vendor".to_string(),
            ],
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
```

**Tests**:
```rust
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
```

---

### Task 3.3: Implement Config File Loading

**Status**: `[x]`

**Description**: Implement functions to locate and load the configuration file following XDG Base Directory spec.

**Context**: Config file locations (in priority order):
1. Path passed via `--config` CLI flag
2. `$XDG_CONFIG_HOME/rusty-sweeper/config.toml`
3. `~/.config/rusty-sweeper/config.toml` (fallback)
4. `/etc/rusty-sweeper/config.toml` (system-wide)

If no config file exists, use defaults silently.

**Sample Implementation** (add to `src/config.rs`):
```rust
use crate::error::{ConfigError, Result};
use std::path::{Path, PathBuf};

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
        let contents = std::fs::read_to_string(path).map_err(|e| {
            ConfigError::ReadError {
                path: path.to_path_buf(),
                source: e,
            }
        })?;

        let config: Config = toml::from_str(&contents).map_err(|e| {
            ConfigError::ParseError {
                path: path.to_path_buf(),
                source: e,
            }
        })?;

        config.validate()?;
        Ok(config)
    }

    /// Validate configuration values
    fn validate(&self) -> Result<()> {
        if self.monitor.warn_threshold > 100 {
            return Err(ConfigError::Invalid(
                "warn_threshold must be 0-100".to_string(),
            ).into());
        }
        if self.monitor.critical_threshold > 100 {
            return Err(ConfigError::Invalid(
                "critical_threshold must be 0-100".to_string(),
            ).into());
        }
        if self.monitor.warn_threshold >= self.monitor.critical_threshold {
            return Err(ConfigError::Invalid(
                "warn_threshold must be less than critical_threshold".to_string(),
            ).into());
        }
        Ok(())
    }

    /// Get the default config file path (for --config help text)
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("rusty-sweeper").join("config.toml"))
    }
}
```

**Tests**:
```rust
#[test]
fn load_returns_defaults_when_no_file() {
    let config = Config::load(None).unwrap();
    assert_eq!(config.monitor.interval, 300);
}

#[test]
fn load_fails_on_invalid_explicit_path() {
    let result = Config::load(Some(Path::new("/nonexistent/config.toml")));
    assert!(result.is_err());
}

#[test]
fn validate_catches_invalid_thresholds() {
    let mut config = Config::default();
    config.monitor.warn_threshold = 95;
    config.monitor.critical_threshold = 90;
    assert!(config.validate().is_err());
}
```

---

### Task 3.4: Add Config File Parsing Integration Test

**Status**: `[x]`

**Description**: Create integration test that writes a temp config file and verifies parsing.

**Context**: Ensures end-to-end config loading works correctly with actual file I/O.

**File**: `tests/config_integration.rs`

**Sample Implementation**:
```rust
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
```

---

## 4. CLI Definition

### Task 4.1: Define CLI Structure with Clap

**Status**: `[x]`

**Description**: Define the complete CLI interface using clap's derive macros.

**Context**: We define all subcommands and their arguments upfront, even though most won't be implemented until later phases. This establishes the user interface contract early.

**File**: `src/cli.rs`

**Sample Implementation**:
```rust
use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;

/// Rusty Sweeper - A Linux disk usage management utility
#[derive(Parser, Debug)]
#[command(name = "rusty-sweeper")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start disk usage monitoring
    Monitor(MonitorArgs),

    /// Scan for projects and clean build artifacts
    Clean(CleanArgs),

    /// Analyze disk usage of a directory
    Scan(ScanArgs),

    /// Launch interactive TUI
    Tui(TuiArgs),
}

#[derive(Args, Debug)]
pub struct MonitorArgs {
    /// Run as background daemon
    #[arg(short, long)]
    pub daemon: bool,

    /// Check interval in seconds
    #[arg(short, long, default_value = "300", value_name = "SECS")]
    pub interval: u64,

    /// Warning threshold percentage
    #[arg(short, long, default_value = "80", value_name = "PERCENT")]
    pub warn: u8,

    /// Critical threshold percentage
    #[arg(short = 'C', long, default_value = "90", value_name = "PERCENT")]
    pub critical: u8,

    /// Mount point to monitor
    #[arg(short, long, default_value = "/", value_name = "PATH")]
    pub mount: PathBuf,

    /// Check once and exit
    #[arg(long)]
    pub once: bool,
}

#[derive(Args, Debug)]
pub struct CleanArgs {
    /// Root directory to scan
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Show what would be cleaned without doing it
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Maximum recursion depth
    #[arg(short = 'd', long, default_value = "10", value_name = "N")]
    pub max_depth: usize,

    /// Project types to clean (comma-separated)
    #[arg(short, long, value_delimiter = ',', value_name = "TYPES")]
    pub types: Option<Vec<String>>,

    /// Paths to exclude (glob patterns)
    #[arg(short, long, value_name = "PATTERNS")]
    pub exclude: Option<Vec<String>>,

    /// Only clean projects not modified in N days
    #[arg(short, long, value_name = "DAYS")]
    pub age: Option<u32>,

    /// Skip confirmation prompts
    #[arg(short, long)]
    pub force: bool,

    /// Parallel clean jobs
    #[arg(short, long, default_value = "4", value_name = "N")]
    pub jobs: usize,

    /// Only report sizes, don't clean
    #[arg(long)]
    pub size_only: bool,
}

#[derive(Args, Debug)]
pub struct ScanArgs {
    /// Directory to analyze
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Maximum depth to display
    #[arg(short = 'd', long, default_value = "3", value_name = "N")]
    pub max_depth: usize,

    /// Show top N entries by size
    #[arg(short = 'n', long, default_value = "20", value_name = "N")]
    pub top: usize,

    /// Include hidden files
    #[arg(short, long)]
    pub all: bool,

    /// Don't cross filesystem boundaries
    #[arg(short = 'x', long)]
    pub one_file_system: bool,

    /// Parallel scan threads
    #[arg(short, long, value_name = "N")]
    pub jobs: Option<usize>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Sort by: size, name, mtime
    #[arg(long, default_value = "size", value_name = "BY")]
    pub sort: String,
}

#[derive(Args, Debug)]
pub struct TuiArgs {
    /// Starting directory
    #[arg(default_value = "/")]
    pub path: PathBuf,

    /// Don't cross filesystem boundaries
    #[arg(short = 'x', long)]
    pub one_file_system: bool,

    /// Disable colors
    #[arg(long)]
    pub no_color: bool,
}
```

**Tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli_structure() {
        // Validates the CLI definition is correct
        Cli::command().debug_assert();
    }

    #[test]
    fn parse_scan_command() {
        let cli = Cli::parse_from(["rusty-sweeper", "scan", "/home"]);
        match cli.command {
            Command::Scan(args) => {
                assert_eq!(args.path, PathBuf::from("/home"));
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn parse_clean_with_options() {
        let cli = Cli::parse_from([
            "rusty-sweeper",
            "clean",
            "--dry-run",
            "--types", "cargo,npm",
            "--max-depth", "5",
            "/projects",
        ]);
        match cli.command {
            Command::Clean(args) => {
                assert!(args.dry_run);
                assert_eq!(args.max_depth, 5);
                assert_eq!(args.types, Some(vec!["cargo".to_string(), "npm".to_string()]));
            }
            _ => panic!("Expected Clean command"),
        }
    }

    #[test]
    fn global_verbose_flag() {
        let cli = Cli::parse_from(["rusty-sweeper", "-vvv", "scan"]);
        assert_eq!(cli.verbose, 3);
    }
}
```

---

### Task 4.2: Implement Main Entry Point

**Status**: `[ ]`

**Description**: Create the main entry point that parses CLI args, loads config, and dispatches to subcommands.

**Context**: The main function ties everything together. For Phase 1, subcommands will print placeholder messages. Real implementations come in later phases.

**File**: `src/main.rs`

**Sample Implementation**:
```rust
use anyhow::Result;
use clap::Parser;

mod cli;
mod config;
mod error;

use cli::{Cli, Command};
use config::Config;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on verbosity
    init_logging(cli.verbose, cli.quiet);

    // Load configuration
    let config = Config::load(cli.config.as_deref())?;

    tracing::debug!(?config, "Loaded configuration");

    // Dispatch to subcommand
    match cli.command {
        Command::Monitor(args) => {
            tracing::info!(?args, "Starting monitor");
            println!("Monitor command not yet implemented");
            // TODO: Phase 5
        }
        Command::Clean(args) => {
            tracing::info!(?args, "Starting clean");
            println!("Clean command not yet implemented");
            // TODO: Phase 3
        }
        Command::Scan(args) => {
            tracing::info!(?args, "Starting scan");
            println!("Scan command not yet implemented");
            // TODO: Phase 2
        }
        Command::Tui(args) => {
            tracing::info!(?args, "Starting TUI");
            println!("TUI command not yet implemented");
            // TODO: Phase 4
        }
    }

    Ok(())
}

fn init_logging(verbosity: u8, quiet: bool) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let level = if quiet {
        "warn"
    } else {
        match verbosity {
            0 => "info",
            1 => "debug",
            _ => "trace",
        }
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("rusty_sweeper={}", level)));

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter)
        .init();
}
```

**Tests**: See Task 4.3 for CLI integration tests.

---

### Task 4.3: Add CLI Integration Tests

**Status**: `[ ]`

**Description**: Create integration tests that invoke the binary and verify behavior.

**Context**: Uses `assert_cmd` crate to run the actual binary and check outputs/exit codes.

**File**: `tests/cli_integration.rs`

**Sample Implementation**:
```rust
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
```

---

## 5. Logging

### Task 5.1: Configure Tracing Subscriber

**Status**: `[ ]`

**Description**: Set up the tracing subscriber with appropriate formatting and filtering.

**Context**: We use `tracing` for structured logging. The subscriber is configured based on verbosity flags and `RUST_LOG` environment variable.

**Already covered in**: Task 4.2 (`init_logging` function)

**Additional considerations**:
- `RUST_LOG` environment variable takes precedence
- Quiet mode (`-q`) suppresses info/debug
- Default shows only warnings/errors
- `-v` shows info, `-vv` shows debug, `-vvv` shows trace

**Tests**:
```rust
// In src/main.rs tests
#[cfg(test)]
mod tests {
    #[test]
    fn logging_level_from_verbosity() {
        // Verbosity 0 -> info
        // Verbosity 1 -> debug
        // Verbosity 2+ -> trace
        // This is implicitly tested via CLI integration tests
    }
}
```

---

## 6. Library Structure

### Task 6.1: Create Library Root

**Status**: `[ ]`

**Description**: Create `lib.rs` that exports public modules for use by the binary and tests.

**Context**: Separating library code from binary allows integration tests to import modules directly.

**File**: `src/lib.rs`

**Sample Implementation**:
```rust
//! Rusty Sweeper - A Linux disk usage management utility
//!
//! This crate provides functionality for:
//! - Monitoring disk usage with desktop notifications
//! - Discovering and cleaning build artifacts
//! - Interactive TUI for disk exploration

pub mod config;
pub mod error;

// Re-export commonly used types
pub use config::Config;
pub use error::{Result, SweeperError};
```

**Tests**: Implicit through other tests using `use rusty_sweeper::...`

---

## Summary Checklist

| Task | Area | Status |
|------|------|--------|
| 1.1 | Initialize Cargo Project | `[x]` |
| 1.2 | Add Core Dependencies | `[x]` |
| 2.1 | Define Error Types | `[x]` |
| 3.1 | Define Configuration Structures | `[x]` |
| 3.2 | Implement Default Trait | `[x]` |
| 3.3 | Implement Config File Loading | `[x]` |
| 3.4 | Config Integration Test | `[x]` |
| 4.1 | Define CLI Structure | `[x]` |
| 4.2 | Implement Main Entry Point | `[ ]` |
| 4.3 | CLI Integration Tests | `[ ]` |
| 5.1 | Configure Tracing | `[ ]` |
| 6.1 | Create Library Root | `[ ]` |

---

## Completion Criteria

Phase 1 is complete when:

1. `cargo build` produces a working binary
2. `rusty-sweeper --help` shows all subcommands
3. `rusty-sweeper --version` shows version
4. `rusty-sweeper scan --help` shows scan options
5. `rusty-sweeper clean --help` shows clean options
6. `rusty-sweeper monitor --help` shows monitor options
7. `rusty-sweeper tui --help` shows tui options
8. `rusty-sweeper -c config.toml scan` loads config file
9. `rusty-sweeper -vvv scan` enables verbose logging
10. `cargo test` passes all unit and integration tests
11. Each subcommand prints "not yet implemented" placeholder

---

## Next Phase

After completing Phase 1, proceed to **Phase 2: Disk Scanner** where we implement the core scanning functionality.
