use anyhow::Result;
use clap::Parser;

use rusty_sweeper::cli::{Cli, Command};
use rusty_sweeper::commands;
use rusty_sweeper::config::Config;

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
            commands::monitor::run(args)?;
        }
        Command::Clean(args) => {
            tracing::info!(?args, "Starting clean");
            commands::clean::run(args)?;
        }
        Command::Scan(args) => {
            tracing::info!(?args, "Starting scan");
            commands::scan::run(args)?;
        }
        Command::Tui(args) => {
            tracing::info!(?args, "Starting TUI");
            let root = args.path.canonicalize()?;
            rusty_sweeper::tui::run(root)?;
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
