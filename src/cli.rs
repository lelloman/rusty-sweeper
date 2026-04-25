use clap::{Args, Parser};
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

}

/// Dedicated CLI for the monitor binary.
#[derive(Parser, Debug)]
#[command(name = "rusty-sweeper-monitor")]
#[command(author, version, about = "Disk usage monitor daemon and alerting service")]
#[command(propagate_version = true)]
pub struct MonitorCli {
    /// Path to configuration file
    #[arg(short, long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(flatten)]
    pub args: MonitorArgs,
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
    fn parse_main_cli_without_subcommands() {
        let cli = Cli::parse_from(["rusty-sweeper"]);
        assert!(cli.config.is_none());
        assert!(!cli.quiet);
    }

    #[test]
    fn parse_main_cli_with_globals() {
        let cli = Cli::parse_from(["rusty-sweeper", "--quiet", "--config", "/tmp/test.toml"]);
        assert!(cli.quiet);
        assert_eq!(cli.config, Some(PathBuf::from("/tmp/test.toml")));
    }

    #[test]
    fn global_verbose_flag() {
        let cli = Cli::parse_from(["rusty-sweeper", "-vvv"]);
        assert_eq!(cli.verbose, 3);
    }

    #[test]
    fn parse_monitor_cli() {
        let cli = MonitorCli::parse_from(["rusty-sweeper-monitor", "--once", "--notify", "stderr"]);
        assert!(cli.args.once);
        assert_eq!(cli.args.notify, "stderr");
    }
}
