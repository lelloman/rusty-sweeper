use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::error::{Result, SweeperError};

use super::entry::DirEntry;
use super::options::ScanOptions;
use super::size::{apparent_size, disk_usage};

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
}
