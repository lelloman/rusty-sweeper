//! Application state for the TUI.

use std::path::PathBuf;

/// Placeholder for TUI application state.
pub struct App {
    /// Root directory being explored
    pub root: PathBuf,
}

impl App {
    /// Create a new App instance.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}
