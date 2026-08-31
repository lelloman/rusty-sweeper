//! Application state for the TUI.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use nix::sys::statvfs::statvfs;

use crate::cleaner::{
    CleanExecutor, CleanOptions, CleanResult, DetectedProject, DetectedSystemResource,
    DetectorRegistry, SystemCleanerRegistry,
};
use crate::scanner::{
    scan_directory, scan_directory_progressive, DirEntry, ScanOptions, ScanUpdate,
};
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

/// Long-running action currently executing outside the UI thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    Delete,
    Clean,
}

struct OperationState {
    kind: OperationKind,
    cancel: Arc<AtomicBool>,
}

struct OperationOutcome {
    message: String,
    rescan: bool,
    refresh_system_resources: bool,
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
    /// System resource, if this entry represents a system-level cleanable resource.
    pub system_resource: Option<DetectedSystemResource>,
    /// Whether this entry is a visual separator line.
    pub is_separator: bool,
}

/// Delete a file or directory.
fn delete_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn cancelled_outcome(action: &str) -> OperationOutcome {
    OperationOutcome {
        message: format!("{} cancelled", action),
        rescan: false,
        refresh_system_resources: false,
    }
}

fn clean_project(path: PathBuf, cancel: Arc<AtomicBool>) -> OperationOutcome {
    if cancel.load(Ordering::Acquire) {
        return cancelled_outcome("Clean");
    }

    let registry = DetectorRegistry::new();
    let Some(detector) = registry
        .detectors()
        .iter()
        .find(|detector| detector.detect(&path))
    else {
        return OperationOutcome {
            message: "Not a recognized project".to_string(),
            rescan: false,
            refresh_system_resources: false,
        };
    };

    let artifact_paths = detector.find_artifacts(&path);
    if artifact_paths.is_empty() {
        return OperationOutcome {
            message: "No artifacts to clean".to_string(),
            rescan: false,
            refresh_system_resources: false,
        };
    }
    let artifact_size = artifact_paths
        .iter()
        .map(|artifact| dir_size(artifact))
        .sum();
    let project = DetectedProject {
        path,
        project_type: detector.id().to_string(),
        display_name: detector.display_name().to_string(),
        artifact_size,
        artifact_paths,
    };
    let clean_command = detector.clean_command();

    if cancel.load(Ordering::Acquire) {
        return cancelled_outcome("Clean");
    }

    match CleanExecutor::new(CleanOptions::default()).clean(&project, clean_command) {
        CleanResult::Success { freed_bytes, .. } => OperationOutcome {
            message: format!(
                "Cleaned {} project, freed {}",
                project.display_name,
                humansize::format_size(freed_bytes, humansize::BINARY)
            ),
            rescan: true,
            refresh_system_resources: false,
        },
        CleanResult::Failed {
            error, freed_bytes, ..
        } if freed_bytes > 0 => OperationOutcome {
            message: format!(
                "Project partially cleaned, freed {} before failure: {}",
                humansize::format_size(freed_bytes, humansize::BINARY),
                error
            ),
            rescan: true,
            refresh_system_resources: false,
        },
        CleanResult::Failed { error, .. } => OperationOutcome {
            message: format!("Clean failed: {}", error),
            rescan: false,
            refresh_system_resources: false,
        },
        CleanResult::Skipped { reason, .. } => OperationOutcome {
            message: format!("Clean skipped: {}", reason),
            rescan: false,
            refresh_system_resources: false,
        },
    }
}

fn clean_system_resource(
    resource: DetectedSystemResource,
    cancel: Arc<AtomicBool>,
) -> OperationOutcome {
    use crate::cleaner::SystemCleanResult;

    if cancel.load(Ordering::Acquire) {
        return cancelled_outcome("Clean");
    }
    let registry = SystemCleanerRegistry::new();
    let Some(cleaner) = registry.get_cleaner(&resource.category) else {
        return OperationOutcome {
            message: "No cleaner found for this resource".to_string(),
            rescan: false,
            refresh_system_resources: false,
        };
    };

    match cleaner.clean(&resource, false) {
        SystemCleanResult::Success { freed_bytes, .. } => OperationOutcome {
            message: format!(
                "Cleaned {}, freed {}",
                resource.display_name,
                humansize::format_size(freed_bytes, humansize::BINARY)
            ),
            rescan: false,
            refresh_system_resources: true,
        },
        SystemCleanResult::Failed { error, .. } => OperationOutcome {
            message: format!("Clean failed: {}", error),
            rescan: false,
            refresh_system_resources: false,
        },
        SystemCleanResult::Skipped { reason, .. } => OperationOutcome {
            message: format!("Clean skipped: {}", reason),
            rescan: false,
            refresh_system_resources: false,
        },
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
fn detect_project_type(registry: &DetectorRegistry, path: &Path) -> Option<String> {
    for detector in registry.detectors() {
        if detector.detect(path) {
            // Extract short name from display_name (e.g., "Rust/Cargo" -> "Rust")
            let display_name = detector.display_name();
            let short_name = display_name.split('/').next().unwrap_or(display_name);
            return Some(short_name.to_string());
        }
    }

    // Fallback: detect Docker projects by Dockerfile/compose files
    if path.join("Dockerfile").exists()
        || path.join("docker-compose.yml").exists()
        || path.join("docker-compose.yaml").exists()
        || path.join("compose.yml").exists()
        || path.join("compose.yaml").exists()
    {
        return Some("Docker".to_string());
    }

    None
}

/// Preview info for clean confirmation dialog.
#[derive(Debug, Clone, Default)]
pub struct CleanPreview {
    /// Project display name (e.g., "Rust/Cargo").
    pub project_name: String,
    /// Artifact directories that would be cleaned.
    pub artifacts: Vec<(String, u64)>, // (name, size)
    /// Total size that would be freed.
    pub total_size: u64,
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

    /// Action result retained while its automatic rescan is in progress.
    post_operation_message: Option<String>,

    /// Is currently scanning.
    pub scanning: bool,

    /// Preview info for clean confirmation.
    pub clean_preview: Option<CleanPreview>,

    /// Entry captured when a destructive confirmation dialog is opened.
    /// This prevents background scan updates from changing the action target.
    pub confirmation_target: Option<VisibleEntry>,

    /// Detected system-level resources (e.g., Docker build cache).
    pub system_resources: Vec<DetectedSystemResource>,

    /// Whether to detect system-level resources (Docker, etc.).
    pub detect_system: bool,

    /// Reused detector registry and per-path results for cheap UI rebuilds.
    detector_registry: DetectorRegistry,
    project_type_cache: HashMap<PathBuf, Option<String>>,

    /// Receiver for progressive scan updates.
    scan_receiver: Option<Receiver<ScanUpdate>>,

    /// Handle to the background scan thread.
    #[allow(dead_code)]
    scan_thread: Option<JoinHandle<()>>,

    /// Current cleanup/deletion operation and its result channel.
    operation: Option<OperationState>,
    operation_receiver: Option<Receiver<OperationOutcome>>,
    #[allow(dead_code)]
    operation_thread: Option<JoinHandle<()>>,

    /// System-resource discovery runs separately from filesystem scans.
    system_detection_running: bool,
    system_receiver: Option<Receiver<Vec<DetectedSystemResource>>>,
    #[allow(dead_code)]
    system_thread: Option<JoinHandle<()>>,
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
            post_operation_message: None,
            scanning: false,
            clean_preview: None,
            confirmation_target: None,
            system_resources: Vec::new(),
            detect_system: false,
            detector_registry: DetectorRegistry::new(),
            project_type_cache: HashMap::new(),
            scan_receiver: None,
            scan_thread: None,
            operation: None,
            operation_receiver: None,
            operation_thread: None,
            system_detection_running: false,
            system_receiver: None,
            system_thread: None,
        }
    }

    /// Get the currently selected entry, if any.
    pub fn selected_entry(&self) -> Option<&VisibleEntry> {
        self.visible_entries.get(self.selected)
    }

    /// Get the entry targeted by the active confirmation, falling back to the
    /// current selection for direct programmatic actions.
    fn action_target(&self) -> Option<&VisibleEntry> {
        self.confirmation_target
            .as_ref()
            .or_else(|| self.selected_entry())
    }

    /// Rebuild the visible_entries list from the tree based on expansion state.
    pub fn rebuild_visible_entries(&mut self) {
        self.visible_entries.clear();

        // Prepend system resources (if any) before the directory tree
        let dummy_entry = DirEntry::new_dir(PathBuf::from(""), None);
        for resource in &self.system_resources {
            self.visible_entries.push(VisibleEntry {
                entry: dummy_entry.clone(),
                depth: 0,
                is_expanded: false,
                project_type: None,
                system_resource: Some(resource.clone()),
                is_separator: false,
            });
        }

        // Add separator if we have system resources and tree data
        if !self.system_resources.is_empty() && self.tree.is_some() {
            self.visible_entries.push(VisibleEntry {
                entry: dummy_entry,
                depth: 0,
                is_expanded: false,
                project_type: None,
                system_resource: None,
                is_separator: true,
            });
        }

        // Temporarily move the tree out so flattening can mutate the row list
        // without cloning the complete scan result.
        if let Some(tree) = self.tree.take() {
            self.flatten_tree(&tree, 0);
            self.tree = Some(tree);
        }

        // Ensure selection is valid
        if !self.visible_entries.is_empty() && self.selected >= self.visible_entries.len() {
            self.selected = self.visible_entries.len() - 1;
        }
    }

    /// Detect system-level resources (e.g., Docker build cache, images).
    pub fn detect_system_resources(&mut self) {
        if !self.detect_system {
            return;
        }
        let registry = SystemCleanerRegistry::new();
        self.system_resources = registry.detect_all();
    }

    fn start_system_detection(&mut self) {
        if !self.detect_system || self.system_detection_running {
            return;
        }

        let (tx, rx) = mpsc::channel();
        self.system_receiver = Some(rx);
        self.system_detection_running = true;
        self.system_thread = Some(thread::spawn(move || {
            let resources = SystemCleanerRegistry::new().detect_all();
            let _ = tx.send(resources);
        }));
    }

    fn poll_system_detection(&mut self) -> bool {
        let resources = self
            .system_receiver
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok());
        if let Some(resources) = resources {
            self.system_resources = resources;
            self.system_detection_running = false;
            self.system_receiver = None;
            self.system_thread = None;
            self.rebuild_visible_entries();
            return true;
        }
        false
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
            self.project_type_for_path(&entry.path)
        } else {
            None
        };

        self.visible_entries.push(VisibleEntry {
            entry: entry.shallow_clone(),
            depth,
            is_expanded,
            project_type,
            system_resource: None,
            is_separator: false,
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

    fn project_type_for_path(&mut self, path: &Path) -> Option<String> {
        if let Some(project_type) = self.project_type_cache.get(path) {
            return project_type.clone();
        }

        let project_type = detect_project_type(&self.detector_registry, path);
        self.project_type_cache
            .insert(path.to_path_buf(), project_type.clone());
        project_type
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
            // No-op for system resources and separators
            if entry.system_resource.is_some() || entry.is_separator {
                return;
            }
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
    /// Skips over separator entries.
    pub fn move_selection(&mut self, delta: i32) {
        if self.visible_entries.is_empty() {
            return;
        }

        let max_idx = self.visible_entries.len() - 1;
        let new_selected = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            self.selected.saturating_add(delta as usize)
        };

        self.selected = new_selected.min(max_idx);

        // Skip over separator entries
        if self.visible_entries[self.selected].is_separator {
            if delta > 0 && self.selected < max_idx {
                self.selected += 1;
            } else if delta < 0 && self.selected > 0 {
                self.selected -= 1;
            }
        }
    }

    /// Expand selected directory.
    pub fn expand_selected(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            // No-op for system resources and separators
            if entry.system_resource.is_some() || entry.is_separator {
                return;
            }
            if entry.entry.is_dir && !entry.is_expanded {
                self.expanded.insert(entry.entry.path);
                self.rebuild_visible_entries();
            }
        }
    }

    /// Collapse selected directory or go to parent.
    pub fn collapse_selected(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            // No-op for system resources and separators
            if entry.system_resource.is_some() || entry.is_separator {
                return;
            }
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
            // No-op for system resources and separators
            if entry.system_resource.is_some() || entry.is_separator {
                return;
            }
            if let Some(parent) = entry.entry.path.parent() {
                for (i, ve) in self.visible_entries.iter().enumerate() {
                    if ve.system_resource.is_none() && !ve.is_separator && ve.entry.path == parent {
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

        // Docker/system queries have their own worker so process startup cannot
        // stall terminal rendering.
        self.start_system_detection();
        self.rebuild_visible_entries();

        self.scanning = true;
        self.status_message = Some("Scanning...".to_string());

        let (tx, rx) = mpsc::channel();
        self.scan_receiver = Some(rx);

        let root = self.root.clone();

        // Spawn background thread for progressive scanning
        let handle = thread::spawn(move || {
            let options = ScanOptions::default()
                .with_hidden(true)
                .with_one_file_system(true);
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
                ScanUpdate::Started { root, total } => {
                    self.project_type_cache.clear();
                    self.tree = Some(root);
                    self.rebuild_visible_entries();
                    if self.post_operation_message.is_none() {
                        self.status_message = Some(format!("Scanning 0/{}", total));
                    }
                }
                ScanUpdate::Entry {
                    entry,
                    completed,
                    total,
                } => {
                    let name = entry.name.clone();
                    if let Some(tree) = &mut self.tree {
                        tree.children.push(entry);
                        tree.recalculate_totals();
                        tree.sort_by_size();
                    }
                    self.rebuild_visible_entries();
                    if self.post_operation_message.is_none() {
                        self.status_message =
                            Some(format!("Scanning ({}/{}) {}", completed, total, name));
                    }
                }
                ScanUpdate::Complete => {
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

                    self.rebuild_visible_entries();
                    self.status_message = self.post_operation_message.take();
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

    /// Poll all background work performed by the TUI.
    pub fn poll_background_work(&mut self) -> bool {
        let operation = self.poll_operation_result();
        let scan = self.poll_scan_result();
        let system = self.poll_system_detection();
        operation || scan || system
    }

    /// Return the active destructive/cleanup operation, if any.
    pub fn operation_kind(&self) -> Option<OperationKind> {
        self.operation.as_ref().map(|operation| operation.kind)
    }

    /// Request cancellation of the active operation between work phases.
    pub fn cancel_operation(&mut self) {
        if let Some(operation) = &self.operation {
            operation.cancel.store(true, Ordering::Release);
            self.status_message = Some("Cancelling operation...".to_string());
        }
    }

    fn start_operation(
        &mut self,
        kind: OperationKind,
        status: String,
        work: impl FnOnce(Arc<AtomicBool>) -> OperationOutcome + Send + 'static,
    ) {
        if self.operation.is_some() {
            self.status_message = Some("Another operation is already running".to_string());
            return;
        }

        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let (tx, rx) = mpsc::channel();
        self.operation = Some(OperationState { kind, cancel });
        self.operation_receiver = Some(rx);
        self.status_message = Some(status);
        self.operation_thread = Some(thread::spawn(move || {
            let outcome = work(worker_cancel);
            let _ = tx.send(outcome);
        }));
    }

    /// Apply a completed background operation result without blocking the UI.
    pub fn poll_operation_result(&mut self) -> bool {
        let outcome = self
            .operation_receiver
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok());
        let Some(outcome) = outcome else {
            return false;
        };

        self.operation = None;
        self.operation_receiver = None;
        self.operation_thread = None;
        let message = outcome.message;

        if outcome.refresh_system_resources {
            self.start_system_detection();
        }
        if outcome.rescan {
            self.post_operation_message = Some(message.clone());
            self.cancel_scan();
            self.start_background_scan();
        }
        self.status_message = Some(message);
        true
    }

    /// Cancel a filesystem scan by disconnecting its update channel.
    pub fn cancel_scan(&mut self) {
        if self.scanning {
            self.scan_receiver = None;
            self.scan_thread = None;
            self.scanning = false;
        }
    }

    /// Trigger a rescan of the root directory (blocking version for tests).
    pub fn trigger_rescan(&mut self) {
        self.scanning = true;
        self.project_type_cache.clear();

        // Detect system resources first — fast query that gives immediate results
        self.detect_system_resources();

        // Always scan with hidden files included so we can toggle visibility in the UI
        let options = ScanOptions::default()
            .with_hidden(true)
            .with_one_file_system(true);
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
        let entry = match self.action_target() {
            Some(entry) => entry.clone(),
            None => {
                self.status_message = Some("No entry selected".to_string());
                return;
            }
        };

        // Cannot delete system resources or separators
        if entry.system_resource.is_some() {
            self.status_message = Some("Cannot delete system resources".to_string());
            return;
        }
        if entry.is_separator {
            return;
        }

        let path = entry.entry.path;

        // Don't allow deleting the root
        if path == self.root {
            self.status_message = Some("Cannot delete root directory".to_string());
            return;
        }

        let display_path = path.display().to_string();
        self.start_operation(
            OperationKind::Delete,
            format!("Deleting: {}", display_path),
            move |cancel| {
                if cancel.load(Ordering::Acquire) {
                    return OperationOutcome {
                        message: "Delete cancelled".to_string(),
                        rescan: false,
                        refresh_system_resources: false,
                    };
                }
                match delete_path(&path) {
                    Ok(()) => OperationOutcome {
                        message: format!("Deleted: {}", display_path),
                        rescan: true,
                        refresh_system_resources: false,
                    },
                    Err(error) => OperationOutcome {
                        message: format!("Error: {}", error),
                        rescan: false,
                        refresh_system_resources: false,
                    },
                }
            },
        );
    }

    /// Clean the selected project or system resource.
    pub fn clean_selected(&mut self) {
        let entry = match self.action_target() {
            Some(entry) => entry.clone(),
            None => {
                self.status_message = Some("No entry selected".to_string());
                return;
            }
        };

        // Handle separator - no-op
        if entry.is_separator {
            return;
        }

        // Handle system resource cleaning
        if let Some(resource) = entry.system_resource {
            let display_name = resource.display_name.clone();
            self.start_operation(
                OperationKind::Clean,
                format!("Cleaning {}...", display_name),
                move |cancel| clean_system_resource(resource, cancel),
            );
            return;
        }

        // Handle regular project cleaning
        let path = entry.entry.path;
        self.start_operation(
            OperationKind::Clean,
            format!("Cleaning {}...", path.display()),
            move |cancel| clean_project(path, cancel),
        );
    }

    /// Check if the selected entry is a cleanable project or system resource.
    pub fn selected_is_project(&self) -> bool {
        if let Some(entry) = self.selected_entry() {
            // System resources are cleanable
            if entry.system_resource.is_some() {
                return true;
            }
            // Separators are not cleanable
            if entry.is_separator {
                return false;
            }
            let registry = DetectorRegistry::new();
            registry
                .detectors()
                .iter()
                .any(|d| d.detect(&entry.entry.path))
        } else {
            false
        }
    }

    /// Prepare clean preview for the selected project or system resource.
    /// Returns true if preview was prepared, false if not a valid project.
    pub fn prepare_clean_preview(&mut self) -> bool {
        let entry = match self.selected_entry() {
            Some(entry) => entry.clone(),
            None => return false,
        };

        // Separator - not cleanable
        if entry.is_separator {
            return false;
        }

        // System resource preview
        if let Some(resource) = &entry.system_resource {
            let item_info = resource
                .item_count
                .map(|n| format!("{} items", n))
                .unwrap_or_else(|| resource.description.clone());

            self.clean_preview = Some(CleanPreview {
                project_name: resource.display_name.clone(),
                artifacts: vec![(item_info, resource.size)],
                total_size: resource.size,
            });
            return true;
        }

        // Regular project preview
        let path = entry.entry.path;

        let registry = DetectorRegistry::new();
        let matching_detector = registry.detectors().iter().find(|d| d.detect(&path));

        match matching_detector {
            Some(detector) => {
                let artifact_paths = detector.find_artifacts(&path);
                if artifact_paths.is_empty() {
                    self.status_message = Some("No artifacts to clean".to_string());
                    return false;
                }

                let mut artifacts = Vec::new();
                let mut total_size = 0u64;

                for artifact_path in &artifact_paths {
                    let name = artifact_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| artifact_path.display().to_string());
                    let size = dir_size(artifact_path);
                    artifacts.push((name, size));
                    total_size += size;
                }

                self.clean_preview = Some(CleanPreview {
                    project_name: detector.display_name().to_string(),
                    artifacts,
                    total_size,
                });
                true
            }
            None => {
                self.status_message = Some("Not a recognized project".to_string());
                false
            }
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

    fn wait_for_operation(app: &mut App) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while app.operation_kind().is_some() && std::time::Instant::now() < deadline {
            app.poll_operation_result();
            std::thread::yield_now();
        }
        assert!(
            app.operation_kind().is_none(),
            "background operation timed out"
        );
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
    fn test_visible_entries_do_not_clone_descendant_trees() {
        let mut app = App::new(PathBuf::from("/root"));
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.rebuild_visible_entries();

        assert!(app.tree.as_ref().unwrap().children.len() > 1);
        assert!(app
            .visible_entries
            .iter()
            .all(|visible| visible.entry.children.is_empty()));
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

    #[test]
    fn test_delete_selected_runs_as_background_operation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("delete-me.txt");
        fs::write(&file_path, "test").unwrap();

        let mut app = App::new(temp_dir.path().to_path_buf());
        app.initial_scan();
        app.selected = app
            .visible_entries
            .iter()
            .position(|entry| entry.entry.path == file_path)
            .unwrap();

        app.delete_selected();
        assert_eq!(app.operation_kind(), Some(OperationKind::Delete));

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while app.operation_kind().is_some() && std::time::Instant::now() < deadline {
            app.poll_operation_result();
            std::thread::yield_now();
        }

        assert_eq!(app.operation_kind(), None);
        assert!(!file_path.exists());
        assert!(
            app.scanning,
            "successful deletion should start an async rescan"
        );
    }

    #[test]
    fn test_system_detection_starts_in_background() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut app = App::new(temp_dir.path().to_path_buf());
        app.detect_system = true;

        app.start_background_scan();

        assert!(app.system_detection_running);
    }

    // Clean tests

    #[test]
    fn test_clean_selected_no_entry() {
        let mut app = App::new(PathBuf::from("/"));
        app.clean_selected();
        wait_for_operation(&mut app);
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
        wait_for_operation(&mut app);
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
        wait_for_operation(&mut app);

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
        wait_for_operation(&mut app);
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

        let project_type = detect_project_type(&DetectorRegistry::new(), temp_dir.path());
        assert_eq!(project_type, Some("Rust".to_string()));
    }

    #[test]
    fn test_detect_project_type_npm() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::write(temp_dir.path().join("package.json"), "{}").unwrap();

        let project_type = detect_project_type(&DetectorRegistry::new(), temp_dir.path());
        assert_eq!(project_type, Some("npm".to_string()));
    }

    #[test]
    fn test_detect_project_type_none() {
        let temp_dir = tempfile::tempdir().unwrap();

        let project_type = detect_project_type(&DetectorRegistry::new(), temp_dir.path());
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
        assert_eq!(
            app.visible_entries[0].project_type,
            Some("Rust".to_string())
        );
    }

    #[test]
    fn test_project_detection_is_cached_until_rescan() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]").unwrap();

        let mut app = App::new(temp_dir.path().to_path_buf());
        app.tree = Some(DirEntry::new_dir(temp_dir.path().to_path_buf(), None));
        app.rebuild_visible_entries();
        assert_eq!(app.project_type_cache.len(), 1);
        assert_eq!(
            app.visible_entries[0].project_type,
            Some("Rust".to_string())
        );

        fs::remove_file(temp_dir.path().join("Cargo.toml")).unwrap();
        app.rebuild_visible_entries();
        assert_eq!(
            app.visible_entries[0].project_type,
            Some("Rust".to_string()),
            "ordinary UI rebuilds should reuse project detection"
        );

        app.trigger_rescan();
        assert_eq!(app.visible_entries[0].project_type, None);
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

    // System resource tests

    fn make_test_resource(id: &str, name: &str, size: u64) -> DetectedSystemResource {
        DetectedSystemResource {
            resource_id: id.to_string(),
            display_name: name.to_string(),
            category: "docker".to_string(),
            size,
            description: "test".to_string(),
            item_count: Some(10),
        }
    }

    #[test]
    fn test_system_resources_prepended_to_visible_entries() {
        let mut app = App::new(PathBuf::from("/root"));
        app.system_resources = vec![make_test_resource("res1", "Resource 1", 1000)];
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.rebuild_visible_entries();

        // First entry should be system resource
        assert!(app.visible_entries[0].system_resource.is_some());
        assert_eq!(
            app.visible_entries[0]
                .system_resource
                .as_ref()
                .unwrap()
                .display_name,
            "Resource 1"
        );

        // Second should be separator
        assert!(app.visible_entries[1].is_separator);

        // Third should be tree root
        assert!(app.visible_entries[2].system_resource.is_none());
        assert!(!app.visible_entries[2].is_separator);
        assert_eq!(app.visible_entries[2].entry.name, "root");
    }

    #[test]
    fn test_system_resources_no_separator_without_tree() {
        let mut app = App::new(PathBuf::from("/root"));
        app.system_resources = vec![make_test_resource("res1", "Resource 1", 1000)];
        // No tree set
        app.rebuild_visible_entries();

        // Should have resource but no separator
        assert_eq!(app.visible_entries.len(), 1);
        assert!(app.visible_entries[0].system_resource.is_some());
    }

    #[test]
    fn test_move_selection_skips_separator() {
        let mut app = App::new(PathBuf::from("/root"));
        app.system_resources = vec![make_test_resource("res1", "Resource 1", 1000)];
        app.tree = Some(create_test_tree());
        app.expanded.insert(PathBuf::from("/root"));
        app.rebuild_visible_entries();

        // entries: [resource, separator, root, dir_b, dir_a]
        // Start at resource (index 0)
        app.selected = 0;

        // Move down — should skip separator (index 1), land on root (index 2)
        app.move_selection(1);
        assert!(!app.visible_entries[app.selected].is_separator);

        // Move back up from root (index 2) — should skip separator, land on resource (index 0)
        app.move_selection(-1);
        assert!(!app.visible_entries[app.selected].is_separator);
    }

    #[test]
    fn test_selected_is_project_system_resource() {
        let mut app = App::new(PathBuf::from("/root"));
        app.system_resources = vec![make_test_resource("res1", "Resource 1", 1000)];
        app.tree = Some(create_test_tree());
        app.rebuild_visible_entries();

        // Select system resource
        app.selected = 0;
        assert!(app.selected_is_project());
    }

    #[test]
    fn test_selected_is_project_separator() {
        let mut app = App::new(PathBuf::from("/root"));
        app.system_resources = vec![make_test_resource("res1", "Resource 1", 1000)];
        app.tree = Some(create_test_tree());
        app.rebuild_visible_entries();

        // Force select separator
        app.selected = 1;
        assert!(!app.selected_is_project());
    }

    #[test]
    fn test_toggle_noop_on_system_resource() {
        let mut app = App::new(PathBuf::from("/root"));
        app.system_resources = vec![make_test_resource("res1", "Resource 1", 1000)];
        app.tree = Some(create_test_tree());
        app.rebuild_visible_entries();

        let count_before = app.visible_entries.len();
        app.selected = 0; // system resource
        app.toggle_selected();
        assert_eq!(app.visible_entries.len(), count_before);
    }

    #[test]
    fn test_expand_noop_on_separator() {
        let mut app = App::new(PathBuf::from("/root"));
        app.system_resources = vec![make_test_resource("res1", "Resource 1", 1000)];
        app.tree = Some(create_test_tree());
        app.rebuild_visible_entries();

        let count_before = app.visible_entries.len();
        app.selected = 1; // separator
        app.expand_selected();
        assert_eq!(app.visible_entries.len(), count_before);
    }

    #[test]
    fn test_delete_blocked_on_system_resource() {
        let mut app = App::new(PathBuf::from("/root"));
        app.system_resources = vec![make_test_resource("res1", "Resource 1", 1000)];
        app.tree = Some(create_test_tree());
        app.rebuild_visible_entries();

        app.selected = 0; // system resource
        app.delete_selected();

        assert!(app.status_message.is_some());
        assert!(app
            .status_message
            .as_ref()
            .unwrap()
            .contains("Cannot delete system resources"));
    }

    #[test]
    fn test_prepare_clean_preview_system_resource() {
        let mut app = App::new(PathBuf::from("/root"));
        app.system_resources = vec![make_test_resource("res1", "Docker Build Cache", 5000)];
        app.tree = Some(create_test_tree());
        app.rebuild_visible_entries();

        app.selected = 0; // system resource
        let result = app.prepare_clean_preview();
        assert!(result);
        assert!(app.clean_preview.is_some());

        let preview = app.clean_preview.as_ref().unwrap();
        assert_eq!(preview.project_name, "Docker Build Cache");
        assert_eq!(preview.total_size, 5000);
    }

    #[test]
    fn test_prepare_clean_preview_separator() {
        let mut app = App::new(PathBuf::from("/root"));
        app.system_resources = vec![make_test_resource("res1", "Resource 1", 1000)];
        app.tree = Some(create_test_tree());
        app.rebuild_visible_entries();

        app.selected = 1; // separator
        let result = app.prepare_clean_preview();
        assert!(!result);
    }
}
