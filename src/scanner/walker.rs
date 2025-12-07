use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use walkdir::WalkDir;

use crate::error::{Result, SweeperError};

use super::entry::DirEntry;
use super::options::ScanOptions;
use super::size::{apparent_size, disk_usage};

/// Update sent during progressive scan.
#[derive(Debug, Clone)]
pub enum ScanUpdate {
    /// Partial tree with current progress. `scanning` is the name of the entry being scanned.
    Progress { tree: DirEntry, scanning: Option<String> },
    /// Scan finished successfully.
    Complete { tree: DirEntry },
    /// Error occurred during scan.
    Error { message: String },
}

/// Scan a directory and return a tree of DirEntry
pub fn scan_directory(root: &Path, options: &ScanOptions) -> Result<DirEntry> {
    let root = root.canonicalize().map_err(|e| SweeperError::Io {
        path: root.to_path_buf(),
        source: e,
    })?;

    let root_dev = if options.one_file_system {
        Some(
            fs::metadata(&root)
                .map_err(|e| SweeperError::Io {
                    path: root.clone(),
                    source: e,
                })?
                .dev(),
        )
    } else {
        None
    };

    // Build walker with options
    let mut walker = WalkDir::new(&root).follow_links(options.follow_symlinks);

    if let Some(depth) = options.max_depth {
        // walkdir max_depth: 0 = only root, 1 = root + children, etc.
        // Our max_depth: 0 = only root, 1 = root + children, etc.
        // So they match directly
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
                    let error_entry =
                        DirEntry::new_error(path.to_path_buf(), err.to_string());
                    entries.insert(path.to_path_buf(), error_entry);
                }
                continue;
            }
        };

        let path = entry.path().to_path_buf();

        // Skip Linux virtual filesystem paths
        if ScanOptions::is_linux_virtual_fs(&path) {
            continue;
        }

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
    let mut root_entry = entries
        .remove(root)
        .ok_or_else(|| SweeperError::PathNotFound(root.to_path_buf()))?;

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

/// Parallel directory scanner for better performance on large directories
pub fn scan_directory_parallel(root: &Path, options: &ScanOptions) -> Result<DirEntry> {
    let root = root.canonicalize().map_err(|e| SweeperError::Io {
        path: root.to_path_buf(),
        source: e,
    })?;

    scan_dir_recursive_parallel(&root, options, 0)
}

fn scan_dir_recursive_parallel(
    path: &Path,
    options: &ScanOptions,
    depth: usize,
) -> Result<DirEntry> {
    use rayon::prelude::*;

    // Skip Linux virtual filesystem paths
    if ScanOptions::is_linux_virtual_fs(path) {
        return Ok(DirEntry::new_dir(path.to_path_buf(), None));
    }

    // Use symlink_metadata to not follow symlinks
    let symlink_meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => return Ok(DirEntry::new_error(path.to_path_buf(), e.to_string())),
    };

    // If it's a symlink, skip it (unless follow_symlinks is enabled)
    if symlink_meta.file_type().is_symlink() && !options.follow_symlinks {
        // Return a zero-size file entry for the symlink itself
        return Ok(DirEntry::new_file(
            path.to_path_buf(),
            0,
            0,
            symlink_meta.modified().ok(),
        ));
    }
    // If following symlinks, get the target metadata

    // Get actual metadata (follows symlinks if it is one and we got here)
    let metadata = if symlink_meta.file_type().is_symlink() {
        match fs::metadata(path) {
            Ok(m) => m,
            Err(e) => return Ok(DirEntry::new_error(path.to_path_buf(), e.to_string())),
        }
    } else {
        symlink_meta
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

    // Check if we've reached the depth limit - if so, return empty directory
    if let Some(max_depth) = options.max_depth {
        if depth >= max_depth {
            return Ok(DirEntry::new_dir(path.to_path_buf(), metadata.modified().ok()));
        }
    }

    // Read directory entries
    let read_dir = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(e) => return Ok(DirEntry::new_error(path.to_path_buf(), e.to_string())),
    };

    let entries: Vec<_> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| {
            let child_path = e.path();
            // Filter out Linux virtual filesystem paths
            if ScanOptions::is_linux_virtual_fs(&child_path) {
                return false;
            }
            // Filter hidden files if needed
            if !options.include_hidden {
                if let Some(name) = child_path.file_name() {
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
            scan_dir_recursive_parallel(&child_path, options, depth + 1).ok()
        })
        .collect();

    // Build directory entry
    let mut dir_entry = DirEntry::new_dir(path.to_path_buf(), metadata.modified().ok());
    dir_entry.children = children;
    dir_entry.recalculate_totals();
    dir_entry.sort_by_size();

    Ok(dir_entry)
}

/// Progressive directory scanner that sends updates as it scans.
///
/// Scans top-level entries sequentially, sending a tree update after each completes.
/// Each individual entry is scanned in parallel internally for speed.
pub fn scan_directory_progressive(root: &Path, options: &ScanOptions, tx: Sender<ScanUpdate>) {
    let root = match root.canonicalize() {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(ScanUpdate::Error {
                message: format!("Cannot access path: {}", e),
            });
            return;
        }
    };

    let metadata = match fs::metadata(&root) {
        Ok(m) => m,
        Err(e) => {
            let _ = tx.send(ScanUpdate::Error {
                message: format!("Cannot read root: {}", e),
            });
            return;
        }
    };

    // Read top-level directory entries
    let read_dir = match fs::read_dir(&root) {
        Ok(rd) => rd,
        Err(e) => {
            let _ = tx.send(ScanUpdate::Error {
                message: format!("Cannot read directory: {}", e),
            });
            return;
        }
    };

    // Collect top-level entries to process
    let entries: Vec<_> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            if ScanOptions::is_linux_virtual_fs(&path) {
                return false;
            }
            if !options.include_hidden {
                if let Some(name) = path.file_name() {
                    if name.to_string_lossy().starts_with('.') {
                        return false;
                    }
                }
            }
            true
        })
        .collect();

    let total = entries.len();
    let mut children: Vec<DirEntry> = Vec::new();

    // Process each top-level entry sequentially, but each entry scans in parallel internally
    for (idx, entry) in entries.into_iter().enumerate() {
        let child_path = entry.path();
        let entry_name = child_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Send "scanning" status before we start
        let _ = tx.send(ScanUpdate::Progress {
            tree: build_partial_tree(&root, &metadata, &children),
            scanning: Some(format!("({}/{}) {}", idx + 1, total, entry_name)),
        });

        // Scan this entry (parallel internally via scan_dir_recursive_parallel)
        let child_entry = match scan_dir_recursive_parallel(&child_path, options, 1) {
            Ok(e) => e,
            Err(_) => DirEntry::new_error(child_path.clone(), "Scan failed".to_string()),
        };
        children.push(child_entry);
    }

    // Build final tree
    let tree = build_partial_tree(&root, &metadata, &children);
    let _ = tx.send(ScanUpdate::Complete { tree });
}

/// Helper to build a partial tree from accumulated children.
fn build_partial_tree(
    root: &Path,
    metadata: &std::fs::Metadata,
    children: &[DirEntry],
) -> DirEntry {
    let mut tree = DirEntry::new_dir(root.to_path_buf(), metadata.modified().ok());
    tree.children = children.to_vec();
    tree.recalculate_totals();
    tree.sort_by_size();
    tree
}

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
        File::create(root.join("file1.txt"))
            .unwrap()
            .write_all(b"hello")
            .unwrap();
        File::create(root.join("file2.txt"))
            .unwrap()
            .write_all(b"world!")
            .unwrap();

        // Create subdirectory with files
        fs::create_dir(root.join("subdir")).unwrap();
        File::create(root.join("subdir/nested.txt"))
            .unwrap()
            .write_all(b"nested content")
            .unwrap();

        // Create hidden file
        File::create(root.join(".hidden"))
            .unwrap()
            .write_all(b"secret")
            .unwrap();

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
        // max_depth(0) means only root, max_depth(1) means root + children
        let options = ScanOptions::new().with_max_depth(0);

        let result = scan_directory(dir.path(), &options).unwrap();

        // With depth 0, we only get root, no children
        assert!(result.children.is_empty());
    }

    #[test]
    fn test_scan_max_depth_one() {
        let dir = create_test_structure();
        // max_depth(1) means root + immediate children, but not grandchildren
        let options = ScanOptions::new().with_max_depth(1);

        let result = scan_directory(dir.path(), &options).unwrap();

        // Should have subdir and files at depth 1
        let subdir = result.children.iter().find(|c| c.name == "subdir");
        assert!(subdir.is_some());

        // Subdir should exist but be empty (no grandchildren at depth 2)
        let subdir = subdir.unwrap();
        assert!(subdir.children.is_empty());
    }

    #[test]
    fn test_scan_counts_files_correctly() {
        let dir = create_test_structure();
        let options = ScanOptions::new().with_hidden(false);

        let result = scan_directory(dir.path(), &options).unwrap();

        // Should have: file1.txt, file2.txt, nested.txt = 3 files
        assert_eq!(result.file_count, 3);
        // Should have: subdir = 1 directory
        assert_eq!(result.dir_count, 1);
    }

    #[test]
    fn test_scan_sorted_by_size() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create files of different sizes
        File::create(root.join("small.txt"))
            .unwrap()
            .write_all(b"a")
            .unwrap();
        File::create(root.join("large.txt"))
            .unwrap()
            .write_all(&vec![b'x'; 1000])
            .unwrap();
        File::create(root.join("medium.txt"))
            .unwrap()
            .write_all(&vec![b'y'; 100])
            .unwrap();

        let options = ScanOptions::default();
        let result = scan_directory(dir.path(), &options).unwrap();

        // Should be sorted largest first
        assert_eq!(result.children[0].name, "large.txt");
        assert_eq!(result.children[1].name, "medium.txt");
        assert_eq!(result.children[2].name, "small.txt");
    }

    #[test]
    fn test_scan_nonexistent_path() {
        let result = scan_directory(Path::new("/nonexistent/path/12345"), &ScanOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_scan_sizes_are_accumulated() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create subdir with files
        fs::create_dir(root.join("subdir")).unwrap();
        File::create(root.join("subdir/file1.txt"))
            .unwrap()
            .write_all(&vec![b'a'; 100])
            .unwrap();
        File::create(root.join("subdir/file2.txt"))
            .unwrap()
            .write_all(&vec![b'b'; 200])
            .unwrap();

        let options = ScanOptions::default();
        let result = scan_directory(dir.path(), &options).unwrap();

        // Root should have combined size
        assert_eq!(result.size, 300);

        // Subdir should also have combined size
        let subdir = result.children.iter().find(|c| c.name == "subdir").unwrap();
        assert_eq!(subdir.size, 300);
    }

    // Parallel scanner tests

    #[test]
    fn test_parallel_scan_basic() {
        let dir = create_test_structure();
        let options = ScanOptions::default();

        let result = scan_directory_parallel(dir.path(), &options).unwrap();

        assert!(result.is_dir);
        assert!(result.size > 0);
        assert!(result.file_count >= 3);
    }

    #[test]
    fn test_parallel_scan_matches_sequential() {
        let dir = create_test_structure();
        let options = ScanOptions::new().with_hidden(false);

        let sequential = scan_directory(dir.path(), &options).unwrap();
        let parallel = scan_directory_parallel(dir.path(), &options).unwrap();

        assert_eq!(sequential.size, parallel.size);
        assert_eq!(sequential.file_count, parallel.file_count);
        assert_eq!(sequential.dir_count, parallel.dir_count);
    }

    #[test]
    fn test_parallel_scan_excludes_hidden() {
        let dir = create_test_structure();
        let options = ScanOptions::new().with_hidden(false);

        let result = scan_directory_parallel(dir.path(), &options).unwrap();

        let has_hidden = result.children.iter().any(|c| c.name == ".hidden");
        assert!(!has_hidden);
    }

    #[test]
    fn test_parallel_scan_max_depth() {
        let dir = create_test_structure();
        let options = ScanOptions::new().with_max_depth(1);

        let result = scan_directory_parallel(dir.path(), &options).unwrap();

        let subdir = result.children.iter().find(|c| c.name == "subdir");
        assert!(subdir.is_some());
        assert!(subdir.unwrap().children.is_empty());
    }

    #[test]
    fn test_parallel_scan_large_structure() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create a larger structure for parallel testing
        for i in 0..10 {
            let subdir = root.join(format!("dir{}", i));
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

        assert_eq!(result.file_count, 100);
        assert_eq!(result.dir_count, 10);
    }
}
