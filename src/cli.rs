use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;
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

    /// Generate shell completions
    Completions(CompletionsArgs),
}

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
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

    /// Mount points to monitor (can be specified multiple times)
    #[arg(short, long, value_name = "PATH")]
    pub mount: Vec<PathBuf>,

    /// Check once and exit
    #[arg(long)]
    pub once: bool,

    /// Stop running daemon
    #[arg(long)]
    pub stop: bool,

    /// Show daemon status
    #[arg(long)]
    pub status: bool,

    /// Notification backend (auto, dbus, notify-send, stderr)
    #[arg(long, default_value = "auto", value_name = "BACKEND")]
    pub notify: String,
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
            "--types",
            "cargo,npm",
            "--max-depth",
            "5",
            "/projects",
        ]);
        match cli.command {
            Command::Clean(args) => {
                assert!(args.dry_run);
                assert_eq!(args.max_depth, 5);
                assert_eq!(
                    args.types,
                    Some(vec!["cargo".to_string(), "npm".to_string()])
                );
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
