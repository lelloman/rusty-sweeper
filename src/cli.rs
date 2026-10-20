use clap::Parser;
use std::path::PathBuf;

/// Rusty Sweeper - A Linux disk usage management utility
#[derive(Parser, Debug)]
#[command(name = "rusty-sweeper")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Directory to scan (defaults to the current working directory)
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Use command-line reports and cleanup instead of opening the TUI
    #[arg(long)]
    pub cli: bool,

    /// Discover project artifacts, report them, and confirm before cleaning
    #[arg(long, requires = "cli", conflicts_with_all = ["json", "depth"])]
    pub clean: bool,

    /// Clean without asking for confirmation (requires --clean)
    #[arg(short = 'y', long, requires = "clean")]
    pub yes: bool,

    /// Print the complete scan as JSON (requires --cli)
    #[arg(long, requires = "cli")]
    pub json: bool,

    /// Maximum display depth for text reports (root = 0; scans all depths)
    #[arg(long, requires = "cli", conflicts_with = "json")]
    pub depth: Option<usize>,

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

/// Resolve the starting path, defaulting to the process working directory.
pub fn starting_path(path: Option<PathBuf>) -> std::io::Result<PathBuf> {
    match path {
        Some(path) => Ok(path),
        None => std::env::current_dir(),
    }
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
        assert!(cli.path.is_none());
        assert_eq!(
            starting_path(cli.path).unwrap(),
            std::env::current_dir().unwrap()
        );
        assert!(cli.config.is_none());
        assert!(!cli.quiet);
    }

    #[test]
    fn parse_main_cli_with_starting_path() {
        let cli = Cli::parse_from(["rusty-sweeper", "/tmp/projects"]);
        assert_eq!(cli.path, Some(PathBuf::from("/tmp/projects")));
        assert_eq!(
            starting_path(cli.path).unwrap(),
            PathBuf::from("/tmp/projects")
        );
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
    fn parse_report_mode() {
        let cli = Cli::parse_from(["rusty-sweeper", "--cli", "--json", "/tmp"]);
        assert!(cli.cli);
        assert!(cli.json);
        assert_eq!(cli.path, Some(PathBuf::from("/tmp")));
    }

    #[test]
    fn report_options_require_cli() {
        assert!(Cli::try_parse_from(["rusty-sweeper", "--json"]).is_err());
        assert!(Cli::try_parse_from(["rusty-sweeper", "--depth", "1"]).is_err());
    }
}
