//! Application state for the TUI.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::scanner::DirEntry;

/// The current UI mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal navigation mode.
    Normal,
    /// Search/filter mode.
    Search,
    /// Confirmation dialog mode.
    Confirm(ConfirmAction),
    /// Help overlay mode.
    Help,
}

/// Action requiring confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    /// Delete a file or directory.
    Delete,
    /// Clean a project's build artifacts.
    Clean,
}

/// Sort order for entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Sort by size (largest first).
    #[default]
    Size,
    /// Sort by name (alphabetical).
    Name,
    /// Sort by modification time (newest first).
    Mtime,
}

/// A visible entry in the flattened tree view.
#[derive(Debug, Clone)]
pub struct VisibleEntry {
    /// The directory entry.
    pub entry: DirEntry,
    /// Depth in the tree (0 = root).
    pub depth: usize,
    /// Whether this directory is expanded.
    pub is_expanded: bool,
}

/// Main application state for the TUI.
pub struct App {
    /// Root directory being explored.
    pub root: PathBuf,

    /// Scanned directory tree.
    pub tree: Option<DirEntry>,

    /// Flattened visible entries (for rendering).
    pub visible_entries: Vec<VisibleEntry>,

    /// Currently selected index in visible_entries.
    pub selected: usize,

    /// Set of expanded directory paths.
    pub expanded: HashSet<PathBuf>,

    /// Current UI mode.
    pub mode: Mode,

    /// Search/filter input.
    pub search_query: String,

    /// Current sort order.
    pub sort_order: SortOrder,

    /// Whether to show hidden files.
    pub show_hidden: bool,

    /// Application should quit.
    pub should_quit: bool,

    /// Status message to display.
    pub status_message: Option<String>,

    /// Is currently scanning.
    pub scanning: bool,
}

impl App {
    /// Create a new App instance.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            tree: None,
            visible_entries: Vec::new(),
            selected: 0,
            expanded: HashSet::new(),
            mode: Mode::Normal,
            search_query: String::new(),
            sort_order: SortOrder::default(),
            show_hidden: false,
            should_quit: false,
            status_message: None,
            scanning: false,
        }
    }

    /// Get the currently selected entry, if any.
    pub fn selected_entry(&self) -> Option<&VisibleEntry> {
        self.visible_entries.get(self.selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new() {
        let app = App::new(PathBuf::from("/home/user"));
        assert_eq!(app.root, PathBuf::from("/home/user"));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.selected, 0);
        assert!(!app.should_quit);
        assert!(app.visible_entries.is_empty());
        assert!(app.expanded.is_empty());
        assert_eq!(app.sort_order, SortOrder::Size);
    }

    #[test]
    fn test_app_selected_entry_empty() {
        let app = App::new(PathBuf::from("/"));
        assert!(app.selected_entry().is_none());
    }

    #[test]
    fn test_mode_equality() {
        assert_eq!(Mode::Normal, Mode::Normal);
        assert_eq!(Mode::Search, Mode::Search);
        assert_eq!(Mode::Help, Mode::Help);
        assert_eq!(
            Mode::Confirm(ConfirmAction::Delete),
            Mode::Confirm(ConfirmAction::Delete)
        );
        assert_ne!(Mode::Normal, Mode::Search);
        assert_ne!(
            Mode::Confirm(ConfirmAction::Delete),
            Mode::Confirm(ConfirmAction::Clean)
        );
    }

    #[test]
    fn test_sort_order_default() {
        let order = SortOrder::default();
        assert_eq!(order, SortOrder::Size);
    }
}
