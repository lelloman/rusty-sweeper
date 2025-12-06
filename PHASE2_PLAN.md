# Phase 2: Disk Scanner - Implementation Plan

## Overview

Phase 2 implements the core disk scanning functionality that will be used by the `scan` command, TUI, and cleaner. This is the foundation for all disk analysis features.

**Prerequisites**: Phase 1 complete (CLI skeleton, config, error handling, logging)

---

## Task List

### 2.1 Data Structures

#### [x] Task 2.1.1: Define `DirEntry` struct

**Description**: Create the core data structure representing a file or directory with its metadata and size information.

**Context**: This struct is the building block for the entire scanner. It needs to support both files and directories, track size information, and allow tree construction.

**File**: `src/scanner/entry.rs`

**Implementation**:
```rust
use std::path::PathBuf;
use std::time::SystemTime;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DirEntry {
    /// Full path to the file or directory
    pub path: PathBuf,

    /// Entry name (last component of path)
    pub name: String,

    /// True if this is a directory
    pub is_dir: bool,

    /// Apparent size in bytes (sum of file sizes)
    pub size: u64,

    /// Actual disk usage in bytes (accounting for block size)
    pub disk_usage: u64,

    /// Number of files (1 for files, recursive count for dirs)
    pub file_count: u64,

    /// Number of subdirectories (recursive)
    pub dir_count: u64,

    /// Last modification time
    pub mtime: Option<SystemTime>,

    /// Child entries (empty for files)
    pub children: Vec<DirEntry>,

    /// True if we couldn't read this entry (permission denied, etc.)
    pub error: Option<String>,
}
```

**Tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dir_entry_creation() {
        let entry = DirEntry {
            path: PathBuf::from("/test"),
            name: "test".to_string(),
            is_dir: true,
            size: 1024,
            disk_usage: 4096,
            file_count: 5,
            dir_count: 2,
            mtime: None,
            children: vec![],
            error: None,
        };
        assert!(entry.is_dir);
        assert_eq!(entry.size, 1024);
    }
}
```

---

#### [x] Task 2.1.2: Implement `DirEntry` helper methods

**Description**: Add utility methods for creating entries, calculating totals, and sorting children.

**Context**: These helpers will be used during tree construction and display.

**File**: `src/scanner/entry.rs`

**Implementation**:
```rust
impl DirEntry {
    /// Create a new file entry from metadata
    pub fn new_file(path: PathBuf, size: u64, disk_usage: u64, mtime: Option<SystemTime>) -> Self {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            path,
            name,
            is_dir: false,
            size,
            disk_usage,
            file_count: 1,
            dir_count: 0,
            mtime,
            children: vec![],
            error: None,
        }
    }

    /// Create a new directory entry (children added later)
    pub fn new_dir(path: PathBuf, mtime: Option<SystemTime>) -> Self {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        Self {
            path,
            name,
            is_dir: true,
            size: 0,
            disk_usage: 0,
            file_count: 0,
            dir_count: 0,
            mtime,
            children: vec![],
            error: None,
        }
    }

    /// Create an error entry for inaccessible paths
    pub fn new_error(path: PathBuf, error: String) -> Self {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            path,
            name,
            is_dir: false,
            size: 0,
            disk_usage: 0,
            file_count: 0,
            dir_count: 0,
            mtime: None,
            children: vec![],
            error: Some(error),
        }
    }

    /// Recalculate size totals from children
    pub fn recalculate_totals(&mut self) {
        if !self.is_dir {
            return;
        }

        self.size = 0;
        self.disk_usage = 0;
        self.file_count = 0;
        self.dir_count = 0;

        for child in &self.children {
            self.size += child.size;
            self.disk_usage += child.disk_usage;
            self.file_count += child.file_count;
            if child.is_dir {
                self.dir_count += 1 + child.dir_count;
            }
        }
    }

    /// Sort children by size (largest first)
    pub fn sort_by_size(&mut self) {
        self.children.sort_by(|a, b| b.size.cmp(&a.size));
        for child in &mut self.children {
            child.sort_by_size();
        }
    }

    /// Sort children by name (alphabetical)
    pub fn sort_by_name(&mut self) {
        self.children.sort_by(|a, b| a.name.cmp(&b.name));
        for child in &mut self.children {
            child.sort_by_name();
        }
    }

    /// Get total entry count (self + all descendants)
    pub fn total_entries(&self) -> u64 {
        self.file_count + self.dir_count + if self.is_dir { 1 } else { 0 }
    }
}
```

**Tests**:
```rust
#[test]
fn test_recalculate_totals() {
    let mut parent = DirEntry::new_dir(PathBuf::from("/parent"), None);
    parent.children.push(DirEntry::new_file(
        PathBuf::from("/parent/file1.txt"),
        100,
        4096,
        None,
    ));
    parent.children.push(DirEntry::new_file(
        PathBuf::from("/parent/file2.txt"),
        200,
        4096,
        None,
    ));

    parent.recalculate_totals();

    assert_eq!(parent.size, 300);
    assert_eq!(parent.disk_usage, 8192);
    assert_eq!(parent.file_count, 2);
}

#[test]
fn test_sort_by_size() {
    let mut parent = DirEntry::new_dir(PathBuf::from("/parent"), None);
    parent.children.push(DirEntry::new_file(PathBuf::from("/parent/small.txt"), 100, 4096, None));
    parent.children.push(DirEntry::new_file(PathBuf::from("/parent/large.txt"), 1000, 4096, None));
    parent.children.push(DirEntry::new_file(PathBuf::from("/parent/medium.txt"), 500, 4096, None));

    parent.sort_by_size();

    assert_eq!(parent.children[0].name, "large.txt");
    assert_eq!(parent.children[1].name, "medium.txt");
    assert_eq!(parent.children[2].name, "small.txt");
}
```

---

#### [x] Task 2.1.3: Define `ScanOptions` struct

**Description**: Create a configuration struct for scan operations.

**Context**: This allows callers to customize scan behavior (depth limits, hidden files, filesystem boundaries).

**File**: `src/scanner/options.rs`

**Implementation**:
```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Maximum depth to recurse (None = unlimited)
    pub max_depth: Option<usize>,

    /// Include hidden files/directories (starting with .)
    pub include_hidden: bool,

    /// Stay on the same filesystem (don't cross mount points)
    pub one_file_system: bool,

    /// Number of parallel threads (0 = auto)
    pub threads: usize,

    /// Paths to exclude (glob patterns)
    pub exclude_patterns: Vec<String>,

    /// Follow symbolic links
    pub follow_symlinks: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            max_depth: None,
            include_hidden: false,
            one_file_system: false,
            threads: 0,
            exclude_patterns: vec![],
            follow_symlinks: false,
        }
    }
}

impl ScanOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn with_hidden(mut self, include: bool) -> Self {
        self.include_hidden = include;
        self
    }

    pub fn with_one_file_system(mut self, enabled: bool) -> Self {
        self.one_file_system = enabled;
        self
    }

    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    pub fn with_exclude(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns = patterns;
        self
    }
}
```

**Tests**:
```rust
#[test]
fn test_scan_options_builder() {
    let opts = ScanOptions::new()
        .with_max_depth(5)
        .with_hidden(true)
        .with_one_file_system(true);

    assert_eq!(opts.max_depth, Some(5));
    assert!(opts.include_hidden);
    assert!(opts.one_file_system);
}
```

---

### 2.2 Scanner Module Structure

#### [x] Task 2.2.1: Create scanner module hierarchy

**Description**: Set up the module structure for the scanner component.

**Context**: Proper module organization makes the codebase maintainable and allows for clear separation of concerns.

**Files to create**:
```
src/scanner/
├── mod.rs          # Module root, public exports
├── entry.rs        # DirEntry struct (from 2.1.1, 2.1.2)
├── options.rs      # ScanOptions struct (from 2.1.3)
├── walker.rs       # Directory traversal logic
├── size.rs         # Size calculation utilities
└── formatter.rs    # Output formatting (tree, table)
```

**File**: `src/scanner/mod.rs`

**Implementation**:
```rust
mod entry;
mod options;
mod walker;
mod size;
mod formatter;

pub use entry::DirEntry;
pub use options::ScanOptions;
pub use walker::{scan_directory, scan_directory_parallel};
pub use formatter::{format_tree, format_table, format_json};
```

**File**: `src/lib.rs` (update)

```rust
pub mod scanner;
pub mod config;
pub mod error;

pub use scanner::{DirEntry, ScanOptions, scan_directory};
```

---

### 2.3 Size Calculation

#### [x] Task 2.3.1: Implement file size utilities

**Description**: Create functions to get file size and disk usage from metadata.

**Context**: On Linux, apparent size (file content) differs from disk usage (blocks allocated). We need both for accurate reporting.

**File**: `src/scanner/size.rs`

**Implementation**:
```rust
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;

/// Get apparent file size (content length)
pub fn apparent_size(metadata: &Metadata) -> u64 {
    metadata.len()
}

/// Get actual disk usage (blocks * block_size)
/// On most Linux systems, st_blocks is in 512-byte units
pub fn disk_usage(metadata: &Metadata) -> u64 {
    metadata.blocks() * 512
}

/// Format size in human-readable format
pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} B", bytes)
    } else if size >= 100.0 {
        format!("{:.0} {}", size, UNITS[unit_idx])
    } else if size >= 10.0 {
        format!("{:.1} {}", size, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// Parse a size string like "1GB" into bytes
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();

    let (num_str, unit) = if s.ends_with("TB") {
        (&s[..s.len()-2], 1024u64.pow(4))
    } else if s.ends_with("GB") {
        (&s[..s.len()-2], 1024u64.pow(3))
    } else if s.ends_with("MB") {
        (&s[..s.len()-2], 1024u64.pow(2))
    } else if s.ends_with("KB") {
        (&s[..s.len()-2], 1024u64)
    } else if s.ends_with("B") {
        (&s[..s.len()-1], 1u64)
    } else {
        (s.as_str(), 1u64)
    };

    num_str.trim().parse::<f64>().ok().map(|n| (n * unit as f64) as u64)
}
```

**Tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
        assert_eq!(format_size(1073741824), "1.00 GB");
        assert_eq!(format_size(1099511627776), "1.00 TB");
    }

    #[test]
    fn test_format_size_precision() {
        assert_eq!(format_size(1024 * 15), "15.0 KB");
        assert_eq!(format_size(1024 * 150), "150 KB");
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1024"), Some(1024));
        assert_eq!(parse_size("1KB"), Some(1024));
        assert_eq!(parse_size("1 MB"), Some(1048576));
        assert_eq!(parse_size("1.5GB"), Some(1610612736));
        assert_eq!(parse_size("invalid"), None);
    }
}
```

---

### 2.4 Directory Walking

#### [x] Task 2.4.1: Implement basic directory walker

**Description**: Create a synchronous directory walker using `walkdir`.

**Context**: Start with a simple single-threaded implementation before adding parallelism. This serves as a reference and fallback.

**File**: `src/scanner/walker.rs`

**Dependencies**: Add to `Cargo.toml`:
```toml
[dependencies]
walkdir = "2"
```

**Implementation**:
```rust
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use crate::error::Result;
use super::entry::DirEntry;
use super::options::ScanOptions;
use super::size::{apparent_size, disk_usage};

/// Scan a directory and return a tree of DirEntry
pub fn scan_directory(root: &Path, options: &ScanOptions) -> Result<DirEntry> {
    let root = root.canonicalize()?;
    let root_dev = if options.one_file_system {
        Some(fs::metadata(&root)?.dev())
    } else {
        None
    };

    // Build walker with options
    let mut walker = WalkDir::new(&root)
        .follow_links(options.follow_symlinks);

    if let Some(depth) = options.max_depth {
        walker = walker.max_depth(depth);
    }

    // Collect all entries, building a path -> entry map
    let mut entries: HashMap<PathBuf, DirEntry> = HashMap::new();

    for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(err) => {
                // Handle permission errors gracefully
                if let Some(path) = err.path() {
                    let error_entry = DirEntry::new_error(
                        path.to_path_buf(),
                        err.to_string(),
                    );
                    entries.insert(path.to_path_buf(), error_entry);
                }
                continue;
            }
        };

        let path = entry.path().to_path_buf();

        // Skip hidden files if not requested
        if !options.include_hidden {
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') && path != root {
                    continue;
                }
            }
        }

        // Handle metadata
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(err) => {
                entries.insert(path.clone(), DirEntry::new_error(path, err.to_string()));
                continue;
            }
        };

        // Check filesystem boundary
        if let Some(root_dev) = root_dev {
            use std::os::unix::fs::MetadataExt;
            if metadata.dev() != root_dev {
                continue;
            }
        }

        // Create entry
        let dir_entry = if metadata.is_dir() {
            DirEntry::new_dir(path.clone(), metadata.modified().ok())
        } else {
            DirEntry::new_file(
                path.clone(),
                apparent_size(&metadata),
                disk_usage(&metadata),
                metadata.modified().ok(),
            )
        };

        entries.insert(path, dir_entry);
    }

    // Build tree from flat map
    build_tree(&root, entries)
}

/// Build a tree structure from a flat HashMap of entries
fn build_tree(root: &Path, mut entries: HashMap<PathBuf, DirEntry>) -> Result<DirEntry> {
    // Sort paths by depth (deepest first) so we process children before parents
    let mut paths: Vec<_> = entries.keys().cloned().collect();
    paths.sort_by(|a, b| {
        let depth_a = a.components().count();
        let depth_b = b.components().count();
        depth_b.cmp(&depth_a) // Deepest first
    });

    // Move children into their parents
    for path in &paths {
        if path == root {
            continue;
        }

        if let Some(parent_path) = path.parent() {
            if let Some(entry) = entries.remove(path) {
                if let Some(parent) = entries.get_mut(parent_path) {
                    parent.children.push(entry);
                }
            }
        }
    }

    // Get root entry and recalculate totals
    let mut root_entry = entries.remove(root)
        .ok_or_else(|| crate::error::Error::NotFound(root.to_path_buf()))?;

    recalculate_totals_recursive(&mut root_entry);
    root_entry.sort_by_size();

    Ok(root_entry)
}

/// Recursively recalculate totals from leaves up
fn recalculate_totals_recursive(entry: &mut DirEntry) {
    for child in &mut entry.children {
        recalculate_totals_recursive(child);
    }
    entry.recalculate_totals();
}
```

**Tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_structure() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create files
        File::create(root.join("file1.txt")).unwrap().write_all(b"hello").unwrap();
        File::create(root.join("file2.txt")).unwrap().write_all(b"world!").unwrap();

        // Create subdirectory with files
        fs::create_dir(root.join("subdir")).unwrap();
        File::create(root.join("subdir/nested.txt")).unwrap()
            .write_all(b"nested content").unwrap();

        // Create hidden file
        File::create(root.join(".hidden")).unwrap().write_all(b"secret").unwrap();

        dir
    }

    #[test]
    fn test_scan_directory_basic() {
        let dir = create_test_structure();
        let options = ScanOptions::default();

        let result = scan_directory(dir.path(), &options).unwrap();

        assert!(result.is_dir);
        assert!(result.size > 0);
        assert!(result.file_count >= 3); // file1, file2, nested
    }

    #[test]
    fn test_scan_excludes_hidden() {
        let dir = create_test_structure();
        let options = ScanOptions::new().with_hidden(false);

        let result = scan_directory(dir.path(), &options).unwrap();

        // .hidden should not be included
        let has_hidden = result.children.iter().any(|c| c.name == ".hidden");
        assert!(!has_hidden);
    }

    #[test]
    fn test_scan_includes_hidden() {
        let dir = create_test_structure();
        let options = ScanOptions::new().with_hidden(true);

        let result = scan_directory(dir.path(), &options).unwrap();

        let has_hidden = result.children.iter().any(|c| c.name == ".hidden");
        assert!(has_hidden);
    }

    #[test]
    fn test_scan_max_depth() {
        let dir = create_test_structure();
        let options = ScanOptions::new().with_max_depth(1);

        let result = scan_directory(dir.path(), &options).unwrap();

        // Should have subdir but not its contents counted separately
        let subdir = result.children.iter().find(|c| c.name == "subdir");
        assert!(subdir.is_some());
    }
}
```

---

#### [x] Task 2.4.2: Implement parallel directory walker

**Description**: Add parallel scanning using `rayon` for faster performance on large directories.

**Context**: Large directory trees benefit significantly from parallel I/O. We'll scan subdirectories in parallel and merge results.

**Dependencies**: Add to `Cargo.toml`:
```toml
[dependencies]
rayon = "1"
```

**File**: `src/scanner/walker.rs` (add to existing)

**Implementation**:
```rust
use rayon::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// Parallel directory scanner for better performance
pub fn scan_directory_parallel(root: &Path, options: &ScanOptions) -> Result<DirEntry> {
    let root = root.canonicalize()?;

    // Configure thread pool if specified
    if options.threads > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(options.threads)
            .build_global()
            .ok(); // Ignore if already initialized
    }

    scan_dir_recursive(&root, options, 0)
}

fn scan_dir_recursive(path: &Path, options: &ScanOptions, depth: usize) -> Result<DirEntry> {
    // Check depth limit
    if let Some(max_depth) = options.max_depth {
        if depth > max_depth {
            return Ok(DirEntry::new_dir(path.to_path_buf(), None));
        }
    }

    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Ok(DirEntry::new_error(path.to_path_buf(), e.to_string())),
    };

    // If it's a file, return immediately
    if !metadata.is_dir() {
        return Ok(DirEntry::new_file(
            path.to_path_buf(),
            apparent_size(&metadata),
            disk_usage(&metadata),
            metadata.modified().ok(),
        ));
    }

    // Read directory entries
    let read_dir = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(e) => return Ok(DirEntry::new_error(path.to_path_buf(), e.to_string())),
    };

    let entries: Vec<_> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| {
            // Filter hidden files if needed
            if !options.include_hidden {
                if let Some(name) = e.path().file_name() {
                    if name.to_string_lossy().starts_with('.') {
                        return false;
                    }
                }
            }
            true
        })
        .collect();

    // Process entries in parallel
    let children: Vec<DirEntry> = entries
        .par_iter()
        .filter_map(|entry| {
            let child_path = entry.path();
            scan_dir_recursive(&child_path, options, depth + 1).ok()
        })
        .collect();

    // Build directory entry
    let mut dir_entry = DirEntry::new_dir(path.to_path_buf(), metadata.modified().ok());
    dir_entry.children = children;
    dir_entry.recalculate_totals();
    dir_entry.sort_by_size();

    Ok(dir_entry)
}

/// Progress callback type for tracking scan progress
pub type ProgressCallback = Box<dyn Fn(u64, &Path) + Send + Sync>;

/// Scan with progress reporting
pub fn scan_directory_with_progress(
    root: &Path,
    options: &ScanOptions,
    on_progress: ProgressCallback,
) -> Result<DirEntry> {
    let root = root.canonicalize()?;
    let count = AtomicU64::new(0);

    let result = scan_dir_with_progress_recursive(&root, options, 0, &count, &on_progress)?;

    Ok(result)
}

fn scan_dir_with_progress_recursive(
    path: &Path,
    options: &ScanOptions,
    depth: usize,
    count: &AtomicU64,
    on_progress: &ProgressCallback,
) -> Result<DirEntry> {
    // Increment and report progress
    let current = count.fetch_add(1, Ordering::Relaxed);
    if current % 100 == 0 {
        on_progress(current, path);
    }

    // Same logic as scan_dir_recursive...
    scan_dir_recursive(path, options, depth)
}
```

**Tests**:
```rust
#[test]
fn test_parallel_scan_matches_sequential() {
    let dir = create_test_structure();
    let options = ScanOptions::default();

    let sequential = scan_directory(dir.path(), &options).unwrap();
    let parallel = scan_directory_parallel(dir.path(), &options).unwrap();

    assert_eq!(sequential.size, parallel.size);
    assert_eq!(sequential.file_count, parallel.file_count);
}

#[test]
fn test_parallel_scan_performance() {
    // Create a larger structure for performance testing
    let dir = TempDir::new().unwrap();
    for i in 0..100 {
        let subdir = dir.path().join(format!("dir{}", i));
        fs::create_dir(&subdir).unwrap();
        for j in 0..10 {
            File::create(subdir.join(format!("file{}.txt", j)))
                .unwrap()
                .write_all(b"content")
                .unwrap();
        }
    }

    let options = ScanOptions::default();
    let result = scan_directory_parallel(dir.path(), &options).unwrap();

    assert_eq!(result.file_count, 1000);
    assert_eq!(result.dir_count, 100);
}
```

---

### 2.5 Output Formatting

#### [x] Task 2.5.1: Implement tree formatter

**Description**: Create a function to format `DirEntry` as a tree structure for terminal output.

**Context**: The tree view is the primary output format for the `scan` command, similar to `tree` or `ncdu`.

**File**: `src/scanner/formatter.rs`

**Implementation**:
```rust
use super::entry::DirEntry;
use super::size::format_size;

/// Format options for tree output
#[derive(Debug, Clone)]
pub struct FormatOptions {
    /// Maximum depth to display
    pub max_depth: Option<usize>,
    /// Show only top N entries per directory
    pub top_n: Option<usize>,
    /// Use colors in output
    pub colors: bool,
    /// Show file counts
    pub show_counts: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            max_depth: Some(3),
            top_n: Some(20),
            colors: true,
            show_counts: false,
        }
    }
}

/// Format entry as a tree string
pub fn format_tree(entry: &DirEntry, options: &FormatOptions) -> String {
    let mut output = String::new();
    format_tree_recursive(entry, &mut output, "", true, 0, options);
    output
}

fn format_tree_recursive(
    entry: &DirEntry,
    output: &mut String,
    prefix: &str,
    is_last: bool,
    depth: usize,
    options: &FormatOptions,
) {
    // Check depth limit
    if let Some(max_depth) = options.max_depth {
        if depth > max_depth {
            return;
        }
    }

    // Build the current line
    let connector = if depth == 0 {
        ""
    } else if is_last {
        "└── "
    } else {
        "├── "
    };

    let size_str = format_size(entry.size);
    let name = if entry.is_dir {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    };

    // Error indicator
    let error_indicator = if entry.error.is_some() { " [!]" } else { "" };

    // Count indicator
    let count_str = if options.show_counts && entry.is_dir {
        format!(" ({} files)", entry.file_count)
    } else {
        String::new()
    };

    output.push_str(&format!(
        "{}{}{:>10}  {}{}{}\n",
        prefix, connector, size_str, name, count_str, error_indicator
    ));

    // Process children
    if entry.is_dir && !entry.children.is_empty() {
        let new_prefix = if depth == 0 {
            String::new()
        } else if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        let children_to_show: Vec<_> = if let Some(top_n) = options.top_n {
            entry.children.iter().take(top_n).collect()
        } else {
            entry.children.iter().collect()
        };

        let total = children_to_show.len();
        for (i, child) in children_to_show.iter().enumerate() {
            let is_last_child = i == total - 1;
            format_tree_recursive(child, output, &new_prefix, is_last_child, depth + 1, options);
        }

        // Show truncation indicator if needed
        if let Some(top_n) = options.top_n {
            if entry.children.len() > top_n {
                let remaining = entry.children.len() - top_n;
                output.push_str(&format!(
                    "{}└── ... and {} more entries\n",
                    new_prefix, remaining
                ));
            }
        }
    }
}

/// Format entry as a simple table
pub fn format_table(entry: &DirEntry, options: &FormatOptions) -> String {
    let mut output = String::new();

    output.push_str(&format!("{:>12}  {}\n", "SIZE", "PATH"));
    output.push_str(&format!("{:->12}  {:-<50}\n", "", ""));

    format_table_recursive(entry, &mut output, 0, options);

    output
}

fn format_table_recursive(
    entry: &DirEntry,
    output: &mut String,
    depth: usize,
    options: &FormatOptions,
) {
    if let Some(max_depth) = options.max_depth {
        if depth > max_depth {
            return;
        }
    }

    let size_str = format_size(entry.size);
    let indent = "  ".repeat(depth);

    output.push_str(&format!(
        "{:>12}  {}{}\n",
        size_str,
        indent,
        entry.name
    ));

    let children_to_show: Vec<_> = if let Some(top_n) = options.top_n {
        entry.children.iter().take(top_n).collect()
    } else {
        entry.children.iter().collect()
    };

    for child in children_to_show {
        format_table_recursive(child, output, depth + 1, options);
    }
}
```

**Tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_entry() -> DirEntry {
        let mut root = DirEntry::new_dir(PathBuf::from("/test"), None);

        let mut subdir = DirEntry::new_dir(PathBuf::from("/test/subdir"), None);
        subdir.children.push(DirEntry::new_file(
            PathBuf::from("/test/subdir/file.txt"),
            1024,
            4096,
            None,
        ));
        subdir.recalculate_totals();

        root.children.push(DirEntry::new_file(
            PathBuf::from("/test/large.bin"),
            1048576,
            1048576,
            None,
        ));
        root.children.push(subdir);
        root.recalculate_totals();
        root.sort_by_size();

        root
    }

    #[test]
    fn test_format_tree_basic() {
        let entry = create_test_entry();
        let options = FormatOptions::default();
        let output = format_tree(&entry, &options);

        assert!(output.contains("test/"));
        assert!(output.contains("large.bin"));
        assert!(output.contains("subdir/"));
        assert!(output.contains("1.00 MB")); // large.bin
    }

    #[test]
    fn test_format_tree_depth_limit() {
        let entry = create_test_entry();
        let options = FormatOptions {
            max_depth: Some(1),
            ..Default::default()
        };
        let output = format_tree(&entry, &options);

        // Should show root and immediate children, but not nested file
        assert!(output.contains("subdir/"));
        assert!(!output.contains("file.txt"));
    }

    #[test]
    fn test_format_table() {
        let entry = create_test_entry();
        let options = FormatOptions::default();
        let output = format_table(&entry, &options);

        assert!(output.contains("SIZE"));
        assert!(output.contains("PATH"));
        assert!(output.contains("1.00 MB"));
    }
}
```

---

#### [x] Task 2.5.2: Implement JSON formatter

**Description**: Add JSON output for scripting and integration with other tools.

**Context**: JSON output enables piping to `jq` or integration with monitoring systems.

**File**: `src/scanner/formatter.rs` (add to existing)

**Dependencies**: `serde` and `serde_json` already available from Phase 1.

**Implementation**:
```rust
use serde_json;

/// Format entry as JSON
pub fn format_json(entry: &DirEntry, pretty: bool) -> Result<String, serde_json::Error> {
    if pretty {
        serde_json::to_string_pretty(entry)
    } else {
        serde_json::to_string(entry)
    }
}

/// Simplified JSON structure for large outputs
#[derive(serde::Serialize)]
pub struct SummarizedEntry {
    pub path: String,
    pub size: u64,
    pub size_human: String,
    pub file_count: u64,
    pub dir_count: u64,
    pub children: Vec<SummarizedEntry>,
}

impl From<&DirEntry> for SummarizedEntry {
    fn from(entry: &DirEntry) -> Self {
        Self {
            path: entry.path.to_string_lossy().to_string(),
            size: entry.size,
            size_human: format_size(entry.size),
            file_count: entry.file_count,
            dir_count: entry.dir_count,
            children: entry.children.iter().map(SummarizedEntry::from).collect(),
        }
    }
}

/// Format as summarized JSON (smaller output)
pub fn format_json_summary(entry: &DirEntry, pretty: bool) -> Result<String, serde_json::Error> {
    let summary = SummarizedEntry::from(entry);
    if pretty {
        serde_json::to_string_pretty(&summary)
    } else {
        serde_json::to_string(&summary)
    }
}
```

**Tests**:
```rust
#[test]
fn test_format_json() {
    let entry = create_test_entry();
    let json = format_json(&entry, false).unwrap();

    assert!(json.contains("\"path\""));
    assert!(json.contains("\"size\""));
    assert!(json.contains("\"children\""));
}

#[test]
fn test_format_json_pretty() {
    let entry = create_test_entry();
    let json = format_json(&entry, true).unwrap();

    // Pretty JSON should have newlines
    assert!(json.contains('\n'));
}

#[test]
fn test_format_json_summary() {
    let entry = create_test_entry();
    let json = format_json_summary(&entry, false).unwrap();

    assert!(json.contains("size_human"));
    assert!(json.contains("1.00 MB") || json.contains("1 MB"));
}
```

---

### 2.6 CLI Integration

#### [x] Task 2.6.1: Implement `scan` subcommand handler

**Description**: Wire up the `scan` CLI subcommand to the scanner module.

**Context**: This connects the CLI arguments defined in Phase 1 to the actual scanning functionality.

**File**: `src/commands/scan.rs`

**Implementation**:
```rust
use std::path::PathBuf;
use clap::Args;
use crate::scanner::{
    scan_directory_parallel,
    ScanOptions,
    format_tree,
    format_table,
    format_json,
    FormatOptions,
};
use crate::error::Result;

#[derive(Args, Debug)]
pub struct ScanArgs {
    /// Directory to scan
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Maximum depth to display
    #[arg(short = 'd', long, default_value = "3")]
    pub max_depth: usize,

    /// Show top N entries per directory
    #[arg(short = 'n', long, default_value = "20")]
    pub top: usize,

    /// Include hidden files and directories
    #[arg(short = 'a', long)]
    pub all: bool,

    /// Don't cross filesystem boundaries
    #[arg(short = 'x', long)]
    pub one_file_system: bool,

    /// Number of parallel threads (0 = auto)
    #[arg(short = 'j', long, default_value = "0")]
    pub jobs: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Pretty-print JSON output
    #[arg(long, requires = "json")]
    pub pretty: bool,

    /// Sort by: size, name, mtime
    #[arg(long, default_value = "size")]
    pub sort: SortOrder,

    /// Show as table instead of tree
    #[arg(long)]
    pub table: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum SortOrder {
    Size,
    Name,
    Mtime,
}

pub fn run(args: ScanArgs) -> Result<()> {
    // Build scan options
    let scan_options = ScanOptions::new()
        .with_max_depth(args.max_depth + 10) // Scan deeper than display for accurate totals
        .with_hidden(args.all)
        .with_one_file_system(args.one_file_system)
        .with_threads(args.jobs);

    // Perform scan
    tracing::info!("Scanning {}...", args.path.display());
    let mut entry = scan_directory_parallel(&args.path, &scan_options)?;

    // Apply sorting
    match args.sort {
        SortOrder::Size => entry.sort_by_size(),
        SortOrder::Name => entry.sort_by_name(),
        SortOrder::Mtime => {} // TODO: implement mtime sorting
    }

    // Format output
    let output = if args.json {
        format_json(&entry, args.pretty)?
    } else {
        let format_options = FormatOptions {
            max_depth: Some(args.max_depth),
            top_n: Some(args.top),
            colors: atty::is(atty::Stream::Stdout),
            show_counts: false,
        };

        if args.table {
            format_table(&entry, &format_options)
        } else {
            format_tree(&entry, &format_options)
        }
    };

    println!("{}", output);

    Ok(())
}
```

**File**: `src/commands/mod.rs`

```rust
pub mod scan;

pub use scan::ScanArgs;
```

**File**: `src/main.rs` (update)

```rust
use clap::{Parser, Subcommand};

mod commands;
mod config;
mod error;
mod scanner;

#[derive(Parser)]
#[command(name = "rusty-sweeper")]
#[command(about = "Disk usage management utility")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Config file path
    #[arg(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    /// Increase verbosity
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze disk usage of a directory
    Scan(commands::ScanArgs),

    /// Discover and clean build artifacts
    Clean,  // TODO: Phase 3

    /// Launch interactive TUI
    Tui,    // TODO: Phase 4

    /// Start disk usage monitoring
    Monitor, // TODO: Phase 5
}

fn main() -> error::Result<()> {
    let cli = Cli::parse();

    // Setup logging based on verbosity
    let log_level = match cli.verbose {
        0 => tracing::Level::WARN,
        1 => tracing::Level::INFO,
        2 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    match cli.command {
        Commands::Scan(args) => commands::scan::run(args),
        Commands::Clean => {
            eprintln!("Clean command not yet implemented");
            Ok(())
        }
        Commands::Tui => {
            eprintln!("TUI not yet implemented");
            Ok(())
        }
        Commands::Monitor => {
            eprintln!("Monitor not yet implemented");
            Ok(())
        }
    }
}
```

**Dependencies**: Add to `Cargo.toml`:
```toml
[dependencies]
atty = "0.2"
tracing-subscriber = "0.3"
```

---

#### [x] Task 2.6.2: Add progress indicator

**Description**: Show a progress spinner/bar during scanning for better UX.

**Context**: Scanning large directories can take time; users need feedback.

**Dependencies**: Add to `Cargo.toml`:
```toml
[dependencies]
indicatif = "0.17"
```

**File**: `src/commands/scan.rs` (update)

**Implementation**:
```rust
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::time::Duration;

pub fn run(args: ScanArgs) -> Result<()> {
    let scan_options = ScanOptions::new()
        .with_max_depth(args.max_depth + 10)
        .with_hidden(args.all)
        .with_one_file_system(args.one_file_system)
        .with_threads(args.jobs);

    // Create progress spinner
    let pb = if !args.quiet && atty::is(atty::Stream::Stderr) {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap()
        );
        pb.set_message(format!("Scanning {}...", args.path.display()));
        pb.enable_steady_tick(Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    // Perform scan
    let entry = scan_directory_parallel(&args.path, &scan_options)?;

    // Finish progress
    if let Some(pb) = pb {
        pb.finish_with_message(format!(
            "Scanned {} files in {} directories",
            entry.file_count,
            entry.dir_count
        ));
    }

    // ... rest of formatting code
}
```

---

### 2.7 Error Handling

#### [x] Task 2.7.1: Define scanner-specific errors

**Description**: Add specific error types for scanner failures.

**Context**: Clear error messages help users understand and fix issues.

**File**: `src/error.rs` (update)

**Implementation**:
```rust
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Path not found: {0}")]
    NotFound(PathBuf),

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("Not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::NotFound(_) => 1,
            Error::PermissionDenied(_) => 3,
            Error::NotADirectory(_) => 1,
            Error::Io(_) => 1,
            Error::Json(_) => 1,
            Error::Config(_) => 2,
            Error::Other(_) => 1,
        }
    }
}
```

---

### 2.8 Integration Testing

#### [x] Task 2.8.1: Create integration test suite

**Description**: End-to-end tests for the `scan` command.

**Context**: Verify the complete flow from CLI to output.

**File**: `tests/scan_integration.rs`

**Implementation**:
```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

fn create_test_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Create a mock project structure
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("target/debug")).unwrap();

    File::create(root.join("Cargo.toml"))
        .unwrap()
        .write_all(b"[package]\nname = \"test\"")
        .unwrap();

    File::create(root.join("src/main.rs"))
        .unwrap()
        .write_all(b"fn main() {}")
        .unwrap();

    // Create some "build artifacts"
    for i in 0..10 {
        let mut f = File::create(root.join(format!("target/debug/artifact{}.o", i))).unwrap();
        f.write_all(&vec![0u8; 10240]).unwrap(); // 10KB each
    }

    dir
}

#[test]
fn test_scan_basic() {
    let dir = create_test_project();

    Command::cargo_bin("rusty-sweeper")
        .unwrap()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("target/"));
}

#[test]
fn test_scan_json_output() {
    let dir = create_test_project();

    Command::cargo_bin("rusty-sweeper")
        .unwrap()
        .arg("scan")
        .arg("--json")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"));
}

#[test]
fn test_scan_max_depth() {
    let dir = create_test_project();

    Command::cargo_bin("rusty-sweeper")
        .unwrap()
        .arg("scan")
        .arg("-d")
        .arg("1")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("target/"))
        .stdout(predicate::str::contains("artifact").not());
}

#[test]
fn test_scan_hidden_files() {
    let dir = TempDir::new().unwrap();
    File::create(dir.path().join(".hidden")).unwrap();
    File::create(dir.path().join("visible")).unwrap();

    // Without -a flag
    Command::cargo_bin("rusty-sweeper")
        .unwrap()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(".hidden").not());

    // With -a flag
    Command::cargo_bin("rusty-sweeper")
        .unwrap()
        .arg("scan")
        .arg("-a")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(".hidden"));
}

#[test]
fn test_scan_nonexistent_path() {
    Command::cargo_bin("rusty-sweeper")
        .unwrap()
        .arg("scan")
        .arg("/nonexistent/path/12345")
        .assert()
        .failure();
}

#[test]
fn test_scan_table_format() {
    let dir = create_test_project();

    Command::cargo_bin("rusty-sweeper")
        .unwrap()
        .arg("scan")
        .arg("--table")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("SIZE"))
        .stdout(predicate::str::contains("PATH"));
}
```

**Dependencies**: Add to `Cargo.toml`:
```toml
[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

---

#### [x] Task 2.8.2: Add benchmark tests

**Description**: Performance benchmarks for scanner on various directory sizes.

**Context**: Ensure parallel scanning provides real benefits and track performance regressions.

**File**: `benches/scanner_bench.rs`

**Dependencies**: Add to `Cargo.toml`:
```toml
[[bench]]
name = "scanner_bench"
harness = false

[dev-dependencies]
criterion = "0.5"
```

**Implementation**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;
use rusty_sweeper::scanner::{scan_directory, scan_directory_parallel, ScanOptions};

fn create_benchmark_dir(file_count: usize, dir_count: usize) -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    for d in 0..dir_count {
        let subdir = root.join(format!("dir{}", d));
        fs::create_dir(&subdir).unwrap();

        let files_per_dir = file_count / dir_count;
        for f in 0..files_per_dir {
            let mut file = File::create(subdir.join(format!("file{}.txt", f))).unwrap();
            file.write_all(&vec![b'x'; 1024]).unwrap();
        }
    }

    dir
}

fn benchmark_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("scan");

    for size in [100, 1000, 5000].iter() {
        let dir = create_benchmark_dir(*size, 10);
        let options = ScanOptions::default();

        group.bench_with_input(
            BenchmarkId::new("sequential", size),
            size,
            |b, _| {
                b.iter(|| scan_directory(black_box(dir.path()), &options))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("parallel", size),
            size,
            |b, _| {
                b.iter(|| scan_directory_parallel(black_box(dir.path()), &options))
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_scan);
criterion_main!(benches);
```

---

## Summary

| Task | Description | Status |
|------|-------------|--------|
| 2.1.1 | Define `DirEntry` struct | [x] |
| 2.1.2 | Implement `DirEntry` helper methods | [x] |
| 2.1.3 | Define `ScanOptions` struct | [x] |
| 2.2.1 | Create scanner module hierarchy | [x] |
| 2.3.1 | Implement file size utilities | [x] |
| 2.4.1 | Implement basic directory walker | [x] |
| 2.4.2 | Implement parallel directory walker | [x] |
| 2.5.1 | Implement tree formatter | [x] |
| 2.5.2 | Implement JSON formatter | [x] |
| 2.6.1 | Implement `scan` subcommand handler | [x] |
| 2.6.2 | Add progress indicator | [x] |
| 2.7.1 | Define scanner-specific errors | [x] |
| 2.8.1 | Create integration test suite | [x] |
| 2.8.2 | Add benchmark tests | [x] |

**Legend**: [ ] Not started | [~] In progress | [x] Complete

---

## Definition of Done

Phase 2 is complete when:

1. `cargo build` succeeds with no warnings
2. `cargo test` passes all unit and integration tests
3. `cargo clippy` reports no warnings
4. `rusty-sweeper scan .` produces correct tree output
5. `rusty-sweeper scan --json .` produces valid JSON
6. `rusty-sweeper scan -a .` includes hidden files
7. `rusty-sweeper scan -d 2 .` respects depth limit
8. Parallel scan is measurably faster than sequential on large directories
9. Permission errors are handled gracefully (skip and continue)
