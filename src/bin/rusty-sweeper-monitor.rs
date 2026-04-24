use anyhow::Result;
use clap::Parser;

use rusty_sweeper::cli::MonitorCli;
use rusty_sweeper::commands;
use rusty_sweeper::config::Config;

fn main() -> Result<()> {
    let cli = MonitorCli::parse();

    init_logging(cli.verbose, cli.quiet);

    let config = Config::load(cli.config.as_deref())?;
    tracing::debug!(?config, "Loaded configuration");

    tracing::info!(?cli.args, "Starting monitor");
    commands::monitor::run(cli.args)?;
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
