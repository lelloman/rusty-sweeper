//! Application state for the TUI.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};

use nix::sys::statvfs::statvfs;

use crate::cleaner::{
    CleanExecutor, CleanOptions, CleanResult, DetectedProject, DetectorRegistry,
};
use crate::scanner::{scan_directory, scan_directory_progressive, DirEntry, ScanOptions, ScanUpdate};
use walkdir::WalkDir;

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
    /// Project type display name, if this is a detected project.
    pub project_type: Option<String>,
}

/// Delete a file or directory.
fn delete_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Calculate the total size of a directory.
fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Detect project type at the given path.
/// Returns the short display name (e.g., "Rust", "Node") if detected.
fn detect_project_type(path: &Path) -> Option<String> {
    let registry = DetectorRegistry::new();
    for detector in registry.detectors() {
        if detector.detect(path) {
            // Extract short name from display_name (e.g., "Rust/Cargo" -> "Rust")
            let display_name = detector.display_name();
            let short_name = display_name.split('/').next().unwrap_or(display_name);
            return Some(short_name.to_string());
        }
    }
    None
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

    /// Receiver for progressive scan updates.
    scan_receiver: Option<Receiver<ScanUpdate>>,

    /// Handle to the background scan thread.
    #[allow(dead_code)]
    scan_thread: Option<JoinHandle<()>>,
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
            scan_receiver: None,
            scan_thread: None,
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

        // Detect project type for directories
        let project_type = if entry.is_dir {
            detect_project_type(&entry.path)
        } else {
            None
        };

        self.visible_entries.push(VisibleEntry {
            entry: entry.clone(),
            depth,
            is_expanded,
            project_type,
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

    // Scanning methods

    /// Start a background scan of the root directory.
    /// The scan runs in a separate thread and sends progressive updates via `poll_scan_result`.
    pub fn start_background_scan(&mut self) {
        // Don't start a new scan if one is already running
        if self.scanning {
            return;
        }

        self.scanning = true;
        self.status_message = Some("Scanning...".to_string());

        let (tx, rx) = mpsc::channel();
        self.scan_receiver = Some(rx);

        let root = self.root.clone();

        // Spawn background thread for progressive scanning
        let handle = thread::spawn(move || {
            let options = ScanOptions::default().with_hidden(true);
            scan_directory_progressive(&root, &options, tx);
        });

        self.scan_thread = Some(handle);
    }

    /// Poll for scan updates from the background thread.
    /// Returns true if an update was received and processed.
    pub fn poll_scan_result(&mut self) -> bool {
        let update = if let Some(ref rx) = self.scan_receiver {
            rx.try_recv().ok()
        } else {
            None
        };

        if let Some(scan_update) = update {
            match scan_update {
                ScanUpdate::Progress { tree, scanning } => {
                    self.tree = Some(tree);
                    self.rebuild_visible_entries();
                    if let Some(name) = scanning {
                        self.status_message = Some(format!("Scanning: {}", name));
                    }
                }
                ScanUpdate::Complete { tree } => {
                    self.scanning = false;
                    self.scan_receiver = None;
                    self.scan_thread = None;

                    // Preserve expanded paths that still exist
                    let old_expanded: Vec<_> = self.expanded.iter().cloned().collect();
                    self.expanded.clear();
                    for path in old_expanded {
                        if path.exists() {
                            self.expanded.insert(path);
                        }
                    }

                    self.tree = Some(tree);
                    self.rebuild_visible_entries();
                    self.status_message = Some("Scan complete".to_string());
                }
                ScanUpdate::Error { message } => {
                    self.scanning = false;
                    self.scan_receiver = None;
                    self.scan_thread = None;
                    self.status_message = Some(format!("Scan error: {}", message));
                }
            }
            return true;
        }

        false
    }

    /// Trigger a rescan of the root directory (blocking version for tests).
    pub fn trigger_rescan(&mut self) {
        self.scanning = true;

        // Always scan with hidden files included so we can toggle visibility in the UI
        let options = ScanOptions::default().with_hidden(true);
        match scan_directory(&self.root, &options) {
            Ok(tree) => {
                // Preserve expanded paths that still exist
                let old_expanded: Vec<_> = self.expanded.iter().cloned().collect();
                self.expanded.clear();

                for path in old_expanded {
                    if path.exists() {
                        self.expanded.insert(path);
                    }
                }

                self.tree = Some(tree);
                self.rebuild_visible_entries();
            }
            Err(e) => {
                self.status_message = Some(format!("Scan error: {}", e));
            }
        }

        self.scanning = false;
    }

    /// Perform initial scan, expanding root by default.
    pub fn initial_scan(&mut self) {
        self.expanded.insert(self.root.clone());
        self.trigger_rescan();
    }

    /// Start initial scan in background, expanding root by default.
    pub fn start_initial_scan(&mut self) {
        self.expanded.insert(self.root.clone());
        self.start_background_scan();
    }

    /// Delete the selected entry.
    pub fn delete_selected(&mut self) {
        let path = match self.selected_entry() {
            Some(entry) => entry.entry.path.clone(),
            None => {
                self.status_message = Some("No entry selected".to_string());
                return;
            }
        };

        // Don't allow deleting the root
        if path == self.root {
            self.status_message = Some("Cannot delete root directory".to_string());
            return;
        }

        match delete_path(&path) {
            Ok(()) => {
                self.status_message = Some(format!("Deleted: {}", path.display()));
                self.trigger_rescan();
            }
            Err(e) => {
                self.status_message = Some(format!("Error: {}", e));
            }
        }
    }

    /// Clean the selected project.
    pub fn clean_selected(&mut self) {
        let path = match self.selected_entry() {
            Some(entry) => entry.entry.path.clone(),
            None => {
                self.status_message = Some("No entry selected".to_string());
                return;
            }
        };

        // Try to detect a project at this path
        let registry = DetectorRegistry::new();

        // Find a matching detector
        let matching_detector = registry.detectors().iter().find(|d| d.detect(&path));

        match matching_detector {
            Some(detector) => {
                // Find artifact paths and calculate size
                let artifact_paths = detector.find_artifacts(&path);
                if artifact_paths.is_empty() {
                    self.status_message = Some("No artifacts to clean".to_string());
                    return;
                }

                let artifact_size: u64 = artifact_paths.iter().map(|p| dir_size(p)).sum();

                let project = DetectedProject {
                    path: path.clone(),
                    project_type: detector.id().to_string(),
                    display_name: detector.display_name().to_string(),
                    artifact_size,
                    artifact_paths,
                };

                let clean_cmd = detector.clean_command();
                let executor = CleanExecutor::new(CleanOptions::default());

                match executor.clean(&project, clean_cmd) {
                    CleanResult::Success { freed_bytes, .. } => {
                        let freed_str = humansize::format_size(freed_bytes, humansize::BINARY);
                        self.status_message = Some(format!(
                            "Cleaned {} project, freed {}",
                            project.display_name, freed_str
                        ));
                        self.trigger_rescan();
                    }
                    CleanResult::Failed { error, .. } => {
                        self.status_message = Some(format!("Clean failed: {}", error));
                    }
                    CleanResult::Skipped { reason, .. } => {
                        self.status_message = Some(format!("Clean skipped: {}", reason));
                    }
                }
            }
            None => {
                self.status_message = Some("Not a recognized project".to_string());
            }
        }
    }

    /// Check if the selected entry is a cleanable project.
    pub fn selected_is_project(&self) -> bool {
        if let Some(entry) = self.selected_entry() {
            let registry = DetectorRegistry::new();
            registry.detectors().iter().any(|d| d.detect(&entry.entry.path))
        } else {
            false
        }
    }

    /// Get filesystem disk usage information.
    /// Returns (total_bytes, used_bytes, available_bytes) or None if unavailable.
    pub fn get_disk_usage(&self) -> Option<(u64, u64, u64)> {
        // Only get disk usage if the root path actually exists
        if !self.root.exists() {
            return None;
        }

        match statvfs(&self.root) {
            Ok(stat) => {
                let block_size = stat.fragment_size();
                let total = stat.blocks() * block_size;
                let available = stat.blocks_available() * block_size;
                let used = total.saturating_sub(available);
                Some((total, used, available))
            }
            Err(_) => None,
        }
    }

    /// Get disk usage as a percentage.
    pub fn get_disk_usage_percent(&self) -> Option<f32> {
        self.get_disk_usage().map(|(total, used, _)| {
            if total > 0 {
                (used as f64 / total as f64 * 100.0) as f32
            } else {
                0.0
            }
        })
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

    // Navigation method tests

    #[test]
    fn test_move_selection_down() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.rebuild_visible_entries();

        assert_eq!(app.selected, 0);
        app.move_selection(1);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn test_move_selection_up() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.rebuild_visible_entries();

        app.selected = 2;
        app.move_selection(-1);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn test_move_selection_clamps_lower() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.rebuild_visible_entries();

        app.selected = 0;
        app.move_selection(-10);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn test_move_selection_clamps_upper() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.rebuild_visible_entries();

        let last = app.visible_entries.len() - 1;
        app.selected = last;
        app.move_selection(10);
        assert_eq!(app.selected, last);
    }

    #[test]
    fn test_move_selection_empty_list() {
        let mut app = App::new(PathBuf::from("/root"));
        // No tree, empty visible_entries
        app.move_selection(1);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn test_expand_selected() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.rebuild_visible_entries();

        assert!(!app.expanded.contains(&PathBuf::from("/root")));
        app.expand_selected();
        assert!(app.expanded.contains(&PathBuf::from("/root")));
    }

    #[test]
    fn test_collapse_selected() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.rebuild_visible_entries();

        assert!(app.visible_entries[0].is_expanded);
        app.collapse_selected();
        assert!(!app.expanded.contains(&PathBuf::from("/root")));
    }

    #[test]
    fn test_go_to_parent() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.rebuild_visible_entries();

        // Select a child (dir_b at index 1)
        app.selected = 1;
        app.go_to_parent();
        // Should be back at root (index 0)
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn test_cycle_sort_order() {
        let mut app = App::new(PathBuf::from("/root"));

        assert_eq!(app.sort_order, SortOrder::Size);
        app.cycle_sort_order();
        assert_eq!(app.sort_order, SortOrder::Name);
        app.cycle_sort_order();
        assert_eq!(app.sort_order, SortOrder::Mtime);
        app.cycle_sort_order();
        assert_eq!(app.sort_order, SortOrder::Size);
    }

    #[test]
    fn test_sort_order_affects_display() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));

        // Default sort by size - dir_b (500 bytes) comes before dir_a (300 bytes)
        app.rebuild_visible_entries();
        assert_eq!(app.visible_entries[1].entry.name, "dir_b");
        assert_eq!(app.visible_entries[2].entry.name, "dir_a");

        // Sort by name
        app.sort_order = SortOrder::Name;
        app.rebuild_visible_entries();
        assert_eq!(app.visible_entries[1].entry.name, "dir_a");
        assert_eq!(app.visible_entries[2].entry.name, "dir_b");
    }

    // Delete tests

    #[test]
    fn test_delete_path_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test").unwrap();

        assert!(file_path.exists());
        delete_path(&file_path).unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn test_delete_path_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path().join("subdir");
        fs::create_dir(&dir_path).unwrap();
        fs::write(dir_path.join("file.txt"), "test").unwrap();

        assert!(dir_path.exists());
        delete_path(&dir_path).unwrap();
        assert!(!dir_path.exists());
    }

    #[test]
    fn test_delete_selected_no_entry() {
        let mut app = App::new(PathBuf::from("/"));
        app.delete_selected();
        assert!(app.status_message.is_some());
        assert!(app.status_message.as_ref().unwrap().contains("No entry"));
    }

    // Clean tests

    #[test]
    fn test_clean_selected_no_entry() {
        let mut app = App::new(PathBuf::from("/"));
        app.clean_selected();
        assert!(app.status_message.is_some());
        assert!(app.status_message.as_ref().unwrap().contains("No entry"));
    }

    #[test]
    fn test_clean_selected_not_a_project() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = DirEntry::new_dir(temp_dir.path().to_path_buf(), None);

        let mut app = App::new(temp_dir.path().to_path_buf());
        app.tree = Some(root);
        app.rebuild_visible_entries();

        app.clean_selected();
        assert!(app.status_message.is_some());
        assert!(app
            .status_message
            .as_ref()
            .unwrap()
            .contains("Not a recognized project"));
    }

    #[test]
    fn test_clean_selected_cargo_project() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create a cargo project structure
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]").unwrap();
        let target_dir = temp_dir.path().join("target");
        fs::create_dir(&target_dir).unwrap();
        fs::write(target_dir.join("artifact.bin"), "x".repeat(1000)).unwrap();

        let root = DirEntry::new_dir(temp_dir.path().to_path_buf(), None);

        let mut app = App::new(temp_dir.path().to_path_buf());
        app.tree = Some(root);
        app.rebuild_visible_entries();

        assert!(target_dir.exists());
        app.clean_selected();

        // Target should be removed (regardless of whether cargo clean or direct deletion was used)
        assert!(!target_dir.exists());
        assert!(app.status_message.is_some());
        // The clean succeeded - should contain "Cleaned" or show freed space
        let msg = app.status_message.as_ref().unwrap();
        assert!(
            msg.contains("Cleaned") || msg.contains("freed"),
            "Expected success message, got: {}",
            msg
        );
    }

    #[test]
    fn test_clean_selected_no_artifacts() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create a cargo project structure without target directory
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]").unwrap();

        let root = DirEntry::new_dir(temp_dir.path().to_path_buf(), None);

        let mut app = App::new(temp_dir.path().to_path_buf());
        app.tree = Some(root);
        app.rebuild_visible_entries();

        app.clean_selected();
        assert!(app.status_message.is_some());
        assert!(app
            .status_message
            .as_ref()
            .unwrap()
            .contains("No artifacts"));
    }

    #[test]
    fn test_selected_is_project_true() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]").unwrap();

        let root = DirEntry::new_dir(temp_dir.path().to_path_buf(), None);

        let mut app = App::new(temp_dir.path().to_path_buf());
        app.tree = Some(root);
        app.rebuild_visible_entries();

        assert!(app.selected_is_project());
    }

    #[test]
    fn test_selected_is_project_false() {
        let temp_dir = tempfile::tempdir().unwrap();

        let root = DirEntry::new_dir(temp_dir.path().to_path_buf(), None);

        let mut app = App::new(temp_dir.path().to_path_buf());
        app.tree = Some(root);
        app.rebuild_visible_entries();

        assert!(!app.selected_is_project());
    }

    #[test]
    fn test_detect_project_type_cargo() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]").unwrap();

        let project_type = detect_project_type(temp_dir.path());
        assert_eq!(project_type, Some("Rust".to_string()));
    }

    #[test]
    fn test_detect_project_type_npm() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::write(temp_dir.path().join("package.json"), "{}").unwrap();

        let project_type = detect_project_type(temp_dir.path());
        assert_eq!(project_type, Some("npm".to_string()));
    }

    #[test]
    fn test_detect_project_type_none() {
        let temp_dir = tempfile::tempdir().unwrap();

        let project_type = detect_project_type(temp_dir.path());
        assert!(project_type.is_none());
    }

    #[test]
    fn test_visible_entry_has_project_type() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]").unwrap();

        let root = DirEntry::new_dir(temp_dir.path().to_path_buf(), None);

        let mut app = App::new(temp_dir.path().to_path_buf());
        app.tree = Some(root);
        app.rebuild_visible_entries();

        assert!(app.visible_entries[0].project_type.is_some());
        assert_eq!(app.visible_entries[0].project_type, Some("Rust".to_string()));
    }

    #[test]
    fn test_get_disk_usage() {
        let app = App::new(PathBuf::from("/"));
        let usage = app.get_disk_usage();

        // Should succeed for root filesystem
        assert!(usage.is_some());
        let (total, used, avail) = usage.unwrap();
        assert!(total > 0);
        assert!(used <= total);
        assert!(used + avail >= total * 9 / 10); // Allow some margin for rounding
    }

    #[test]
    fn test_get_disk_usage_percent() {
        let app = App::new(PathBuf::from("/"));
        let percent = app.get_disk_usage_percent();

        assert!(percent.is_some());
        let p = percent.unwrap();
        assert!(p >= 0.0);
        assert!(p <= 100.0);
    }

    #[test]
    fn test_get_disk_usage_nonexistent_path() {
        let app = App::new(PathBuf::from("/nonexistent/path/that/does/not/exist"));
        let usage = app.get_disk_usage();

        // Should return None for nonexistent path
        assert!(usage.is_none());
    }

    #[test]
    fn test_dir_size_calculation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("file1.bin"), "x".repeat(100)).unwrap();
        fs::write(subdir.join("file2.bin"), "y".repeat(200)).unwrap();

        let size = dir_size(&subdir);
        assert_eq!(size, 300);
    }

    // Rescan tests

    #[test]
    fn test_trigger_rescan() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::write(temp_dir.path().join("file.txt"), "test").unwrap();

        let mut app = App::new(temp_dir.path().to_path_buf());
        assert!(app.tree.is_none());

        app.trigger_rescan();

        assert!(app.tree.is_some());
        assert!(!app.scanning);
    }

    #[test]
    fn test_rescan_preserves_expansion() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        fs::write(sub_dir.join("file.txt"), "test").unwrap();

        let mut app = App::new(temp_dir.path().to_path_buf());
        app.expanded.insert(sub_dir.clone());

        app.trigger_rescan();

        assert!(app.expanded.contains(&sub_dir));
    }

    #[test]
    fn test_rescan_removes_deleted_expansion() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let mut app = App::new(temp_dir.path().to_path_buf());
        app.expanded.insert(sub_dir.clone());

        // Delete the directory
        fs::remove_dir(&sub_dir).unwrap();

        app.trigger_rescan();

        // Expansion for deleted directory should be removed
        assert!(!app.expanded.contains(&sub_dir));
    }

    #[test]
    fn test_initial_scan() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::write(temp_dir.path().join("file.txt"), "test").unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let mut app = App::new(temp_dir.path().to_path_buf());

        app.initial_scan();

        assert!(app.tree.is_some());
        // Root should be expanded by default
        assert!(app.expanded.contains(&temp_dir.path().to_path_buf()));
        // Visible entries should include root and its children
        assert!(app.visible_entries.len() > 1);
    }

    #[test]
    fn test_rescan_nonexistent_directory() {
        let mut app = App::new(PathBuf::from("/nonexistent/path/that/does/not/exist"));

        app.trigger_rescan();

        // Should have an error status message
        assert!(app.status_message.is_some());
        assert!(app.status_message.as_ref().unwrap().contains("Scan error"));
    }
}
