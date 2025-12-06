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

    /// Rebuild the visible_entries list from the tree based on expansion state.
    pub fn rebuild_visible_entries(&mut self) {
        self.visible_entries.clear();

        if let Some(tree) = self.tree.clone() {
            self.flatten_tree(&tree, 0);
        }

        // Ensure selection is valid
        if !self.visible_entries.is_empty() && self.selected >= self.visible_entries.len() {
            self.selected = self.visible_entries.len() - 1;
        }
    }

    /// Recursively flatten the tree into visible_entries.
    fn flatten_tree(&mut self, entry: &DirEntry, depth: usize) {
        // Apply hidden filter
        if !self.show_hidden && entry.name.starts_with('.') && depth > 0 {
            return;
        }

        // Apply search filter
        if !self.search_query.is_empty() {
            let name_lower = entry.name.to_lowercase();
            let query_lower = self.search_query.to_lowercase();
            if !name_lower.contains(&query_lower) && !self.has_matching_descendants(entry) {
                return;
            }
        }

        let is_expanded = self.expanded.contains(&entry.path);

        self.visible_entries.push(VisibleEntry {
            entry: entry.clone(),
            depth,
            is_expanded,
        });

        // If expanded and has children, recurse
        if is_expanded && entry.is_dir {
            let mut children: Vec<_> = entry.children.iter().collect();
            self.sort_entries(&mut children);

            for child in children {
                self.flatten_tree(child, depth + 1);
            }
        }
    }

    /// Check if any descendants match the search query.
    fn has_matching_descendants(&self, entry: &DirEntry) -> bool {
        let query_lower = self.search_query.to_lowercase();

        for child in &entry.children {
            let name_lower = child.name.to_lowercase();
            if name_lower.contains(&query_lower) {
                return true;
            }
            if self.has_matching_descendants(child) {
                return true;
            }
        }
        false
    }

    /// Sort entries according to current sort order.
    fn sort_entries(&self, entries: &mut [&DirEntry]) {
        match self.sort_order {
            SortOrder::Size => entries.sort_by(|a, b| b.size.cmp(&a.size)),
            SortOrder::Name => entries.sort_by(|a, b| a.name.cmp(&b.name)),
            SortOrder::Mtime => entries.sort_by(|a, b| b.mtime.cmp(&a.mtime)),
        }
    }

    /// Toggle expansion state of selected entry.
    pub fn toggle_selected(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            if entry.entry.is_dir {
                if self.expanded.contains(&entry.entry.path) {
                    self.expanded.remove(&entry.entry.path);
                } else {
                    self.expanded.insert(entry.entry.path);
                }
                self.rebuild_visible_entries();
            }
        }
    }

    // Navigation methods (stubs - will be fully implemented in Task 4.7)

    /// Move selection by delta, clamping to valid range.
    pub fn move_selection(&mut self, delta: i32) {
        if self.visible_entries.is_empty() {
            return;
        }

        let new_selected = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            self.selected.saturating_add(delta as usize)
        };

        self.selected = new_selected.min(self.visible_entries.len() - 1);
    }

    /// Expand selected directory.
    pub fn expand_selected(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            if entry.entry.is_dir && !entry.is_expanded {
                self.expanded.insert(entry.entry.path);
                self.rebuild_visible_entries();
            }
        }
    }

    /// Collapse selected directory or go to parent.
    pub fn collapse_selected(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            if entry.is_expanded {
                self.expanded.remove(&entry.entry.path);
                self.rebuild_visible_entries();
            } else {
                self.go_to_parent();
            }
        }
    }

    /// Move selection to parent directory.
    pub fn go_to_parent(&mut self) {
        if let Some(entry) = self.selected_entry() {
            if let Some(parent) = entry.entry.path.parent() {
                for (i, ve) in self.visible_entries.iter().enumerate() {
                    if ve.entry.path == parent {
                        self.selected = i;
                        break;
                    }
                }
            }
        }
    }

    /// Cycle through sort orders.
    pub fn cycle_sort_order(&mut self) {
        self.sort_order = match self.sort_order {
            SortOrder::Size => SortOrder::Name,
            SortOrder::Name => SortOrder::Mtime,
            SortOrder::Mtime => SortOrder::Size,
        };
        self.rebuild_visible_entries();
        self.status_message = Some(format!("Sort: {:?}", self.sort_order));
    }

    // Action stubs (will be fully implemented in Tasks 4.14-4.16)

    /// Trigger a rescan of the root directory.
    pub fn trigger_rescan(&mut self) {
        self.status_message = Some("Rescan not yet implemented".to_string());
    }

    /// Delete the selected entry.
    pub fn delete_selected(&mut self) {
        self.status_message = Some("Delete not yet implemented".to_string());
    }

    /// Clean the selected project.
    pub fn clean_selected(&mut self) {
        self.status_message = Some("Clean not yet implemented".to_string());
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

    fn create_test_tree() -> DirEntry {
        let mut root = DirEntry::new_dir(PathBuf::from("/root"), None);

        let mut dir_a = DirEntry::new_dir(PathBuf::from("/root/dir_a"), None);
        dir_a.children.push(DirEntry::new_file(
            PathBuf::from("/root/dir_a/file1.txt"),
            100,
            4096,
            None,
        ));
        dir_a.children.push(DirEntry::new_file(
            PathBuf::from("/root/dir_a/file2.txt"),
            200,
            4096,
            None,
        ));
        dir_a.recalculate_totals();

        let mut dir_b = DirEntry::new_dir(PathBuf::from("/root/dir_b"), None);
        dir_b.children.push(DirEntry::new_file(
            PathBuf::from("/root/dir_b/file3.txt"),
            500,
            4096,
            None,
        ));
        dir_b.recalculate_totals();

        let hidden = DirEntry::new_dir(PathBuf::from("/root/.hidden"), None);

        root.children.push(dir_a);
        root.children.push(dir_b);
        root.children.push(hidden);
        root.recalculate_totals();

        root
    }

    #[test]
    fn test_flatten_empty_tree() {
        let mut app = App::new(PathBuf::from("/"));
        app.rebuild_visible_entries();
        assert!(app.visible_entries.is_empty());
    }

    #[test]
    fn test_flatten_single_entry() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(DirEntry::new_dir(PathBuf::from("/root"), None));
        app.rebuild_visible_entries();
        assert_eq!(app.visible_entries.len(), 1);
        assert_eq!(app.visible_entries[0].depth, 0);
    }

    #[test]
    fn test_flatten_collapsed_tree() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.rebuild_visible_entries();

        // Only root visible when collapsed (hidden not shown by default)
        assert_eq!(app.visible_entries.len(), 1);
        assert_eq!(app.visible_entries[0].entry.name, "root");
    }

    #[test]
    fn test_flatten_expanded_tree() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.rebuild_visible_entries();

        // root + dir_a + dir_b (hidden not shown)
        assert_eq!(app.visible_entries.len(), 3);
        assert_eq!(app.visible_entries[0].entry.name, "root");
        assert_eq!(app.visible_entries[0].depth, 0);
        // dir_b has larger size, so comes first with default sort
        assert_eq!(app.visible_entries[1].entry.name, "dir_b");
        assert_eq!(app.visible_entries[1].depth, 1);
        assert_eq!(app.visible_entries[2].entry.name, "dir_a");
        assert_eq!(app.visible_entries[2].depth, 1);
    }

    #[test]
    fn test_flatten_deeply_expanded() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.expanded.insert(PathBuf::from("/root/dir_a"));
        app.rebuild_visible_entries();

        // root + dir_b + dir_a + file2 + file1
        assert_eq!(app.visible_entries.len(), 5);
    }

    #[test]
    fn test_flatten_shows_hidden_when_enabled() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.show_hidden = true;
        app.rebuild_visible_entries();

        // root + dir_a + dir_b + .hidden
        assert_eq!(app.visible_entries.len(), 4);
        assert!(app
            .visible_entries
            .iter()
            .any(|e| e.entry.name == ".hidden"));
    }

    #[test]
    fn test_flatten_search_filter() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.expanded.insert(PathBuf::from("/root/dir_a"));
        app.search_query = "file1".to_string();
        app.rebuild_visible_entries();

        // Should show root -> dir_a -> file1
        assert!(app.visible_entries.iter().any(|e| e.entry.name == "root"));
        assert!(app.visible_entries.iter().any(|e| e.entry.name == "dir_a"));
        assert!(app
            .visible_entries
            .iter()
            .any(|e| e.entry.name == "file1.txt"));
        // dir_b should be filtered out
        assert!(!app.visible_entries.iter().any(|e| e.entry.name == "dir_b"));
    }

    #[test]
    fn test_toggle_expand() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.rebuild_visible_entries();

        let initial_count = app.visible_entries.len();
        assert_eq!(initial_count, 1);

        // Toggle expand on root
        app.toggle_selected();

        // Should now have more entries (root + children)
        assert!(app.visible_entries.len() > initial_count);
        assert!(app.expanded.contains(&PathBuf::from("/root")));

        // Toggle collapse
        app.toggle_selected();
        assert_eq!(app.visible_entries.len(), 1);
        assert!(!app.expanded.contains(&PathBuf::from("/root")));
    }

    #[test]
    fn test_selection_clamps_on_rebuild() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.rebuild_visible_entries();

        // Select last entry
        app.selected = app.visible_entries.len() - 1;

        // Collapse, reducing entries
        app.expanded.clear();
        app.rebuild_visible_entries();

        // Selection should be clamped
        assert!(app.selected < app.visible_entries.len());
    }
}
