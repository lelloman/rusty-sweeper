//! TUI module for interactive disk usage exploration.

pub mod app;
pub mod event;
pub mod ui;
pub mod widgets;

pub use app::App;

use std::io::{self, stdout, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use event::handle_events;
use ui::render;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

/// Run the TUI application.
///
/// This is the main entry point for the TUI. It initializes the terminal,
/// performs an initial scan, and enters the main event loop.
pub fn run(root: PathBuf) -> anyhow::Result<()> {
    run_with_scan_options(
        root,
        crate::scanner::ScanOptions::default().with_one_file_system(true),
    )
}

/// Run the TUI with explicit scanner configuration.
pub fn run_with_scan_options(
    root: PathBuf,
    scan_options: crate::scanner::ScanOptions,
) -> anyhow::Result<()> {
    // Setup
    install_panic_hook();
    let mut terminal = init_terminal()?;

    // Initialize app and start background scan
    let mut app = App::with_scan_options(root, scan_options);
    app.detect_system = true;
    app.start_initial_scan();

    run_with_cleanup(
        || {
            // Main loop
            while !app.should_quit {
                // Poll for scan results from background thread
                app.poll_background_work();

                // Render
                terminal.draw(|frame| render(&app, frame))?;

                // Handle events (with 100ms timeout for responsive UI)
                handle_events(&mut app, Duration::from_millis(100))?;
            }

            Ok(())
        },
        restore_terminal,
    )
}

/// Run a TUI operation and restore terminal state on both success and ordinary
/// error returns. Panics are handled separately by the installed panic hook.
fn run_with_cleanup<T>(
    operation: impl FnOnce() -> anyhow::Result<T>,
    cleanup: impl FnOnce() -> io::Result<()>,
) -> anyhow::Result<T> {
    let operation_result = operation();
    let cleanup_result = cleanup();

    match (operation_result, cleanup_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(cleanup_error)) => Err(cleanup_error.into()),
        (Err(operation_error), _) => Err(operation_error),
    }
}

/// Type alias for our terminal backend.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal for TUI mode.
///
/// This enables raw mode and switches to the alternate screen buffer.
pub fn init_terminal() -> io::Result<Tui> {
    enable_raw_mode()?;
    if let Err(error) = execute!(stdout(), EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(error);
    }
    let backend = CrosstermBackend::new(stdout());
    match Terminal::new(backend) {
        Ok(terminal) => Ok(terminal),
        Err(error) => {
            let _ = restore_terminal();
            Err(error)
        }
    }
}

/// Restore the terminal to its original state.
///
/// This disables raw mode and returns to the main screen buffer.
pub fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

/// Install a panic hook that restores the terminal before printing the panic message.
///
/// This ensures the terminal is left in a usable state even if the application panics.
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_restore_terminal_without_init_is_safe() {
        // Calling restore without init should not panic
        // (it may fail, but shouldn't crash)
        let _ = restore_terminal();
    }

    #[test]
    fn test_cleanup_runs_when_tui_loop_returns_error() {
        let cleaned = AtomicBool::new(false);

        let result: anyhow::Result<()> = run_with_cleanup(
            || Err(anyhow::anyhow!("injected event-loop failure")),
            || {
                cleaned.store(true, Ordering::SeqCst);
                Ok(())
            },
        );

        assert!(result.is_err());
        assert!(cleaned.load(Ordering::SeqCst));
    }
}
