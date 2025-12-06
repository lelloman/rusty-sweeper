//! TUI module for interactive disk usage exploration.

pub mod app;
pub mod event;
pub mod ui;
pub mod widgets;

pub use app::App;

use std::io::{self, stdout, Stdout};

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

/// Type alias for our terminal backend.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal for TUI mode.
///
/// This enables raw mode and switches to the alternate screen buffer.
pub fn init_terminal() -> io::Result<Tui> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    Terminal::new(backend)
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

    #[test]
    fn test_restore_terminal_without_init_is_safe() {
        // Calling restore without init should not panic
        // (it may fail, but shouldn't crash)
        let _ = restore_terminal();
    }
}
