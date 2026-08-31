use anyhow::Result;
use clap::Parser;

use rusty_sweeper::cli::{starting_path, Cli};
use rusty_sweeper::config::Config;
use rusty_sweeper::scanner::ScanOptions;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on verbosity
    init_logging(cli.verbose, cli.quiet);

    // Load configuration
    let config = Config::load(cli.config.as_deref())?;

    tracing::debug!(?config, "Loaded configuration");

    let root = starting_path(cli.path)?;
    let mut scan_options = ScanOptions::default()
        .with_threads(config.scanner.parallel_threads)
        .with_one_file_system(!config.scanner.cross_filesystems);
    if config.scanner.use_cache {
        if let Some(cache_path) = ScanOptions::default_cache_path() {
            scan_options = scan_options.with_cache(cache_path, config.scanner.cache_ttl);
        }
    }
    tracing::info!(path = %root.display(), "Starting TUI");
    rusty_sweeper::tui::run_with_scan_options(root, scan_options)?;

    Ok(())
}

fn init_logging(verbosity: u8, quiet: bool) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let level = if quiet {
        "warn"
    } else {
        match verbosity {
            0 => "warn",
            1 => "info",
            2 => "debug",
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
