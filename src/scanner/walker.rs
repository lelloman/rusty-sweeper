use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

use crate::error::{Result, SweeperError};

use super::cache::{MetadataCache, ScanCacheStats};
use super::entry::DirEntry;
use super::options::ScanOptions;

/// Update sent during progressive scan.
#[derive(Debug, Clone)]
pub enum ScanUpdate {
    /// Scan initialized with a lightweight root entry.
    Started { root: DirEntry, total: usize },
    /// A completed top-level entry to merge into the tree.
    Entry {
        entry: DirEntry,
        completed: usize,
        total: usize,
    },
    /// Scan finished successfully.
    Complete,
    /// Error occurred during scan.
    Error { message: String },
}

/// Scan a directory and return a tree of DirEntry
pub fn scan_directory(root: &Path, options: &ScanOptions) -> Result<DirEntry> {
    scan_directory_with_cache_stats(root, options).map(|(tree, _)| tree)
}

fn scan_directory_with_cache_stats(
    root: &Path,
    options: &ScanOptions,
) -> Result<(DirEntry, ScanCacheStats)> {
    let cache = MetadataCache::load(options.cache.as_ref());
    let tree = scan_directory_inner(root, options, &cache)?;
    let stats = cache.stats();
    cache.save();
    Ok((tree, stats))
}

fn scan_directory_inner(
    root: &Path,
    options: &ScanOptions,
    cache: &MetadataCache,
) -> Result<DirEntry> {
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

    // Prune excluded trees before descent. Filtering only after WalkDir yields
    // an entry still causes it to traverse the directory's descendants.
    let walker = walker
        .into_iter()
        .filter_entry(|entry| should_visit_path(entry.path(), &root, options, root_dev));

    // Collect all entries, building a path -> entry map
    let mut entries: HashMap<PathBuf, DirEntry> = HashMap::new();
    let visited_files = Arc::new(Mutex::new(HashSet::new()));

    for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(err) => {
                // Handle permission errors gracefully
                if let Some(path) = err.path() {
                    let error_entry = DirEntry::new_error(path.to_path_buf(), err.to_string());
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
            let (apparent_size, disk_usage) =
                accounted_file_sizes(cache, &path, &metadata, &visited_files);
            DirEntry::new_file(
                path.clone(),
                apparent_size,
                disk_usage,
                metadata.modified().ok(),
            )
        };

        entries.insert(path, dir_entry);
    }

    // Build tree from flat map
    build_tree(&root, entries)
}

fn accounted_file_sizes(
    cache: &MetadataCache,
    path: &Path,
    metadata: &fs::Metadata,
    visited_files: &Arc<Mutex<HashSet<(u64, u64)>>>,
) -> (u64, u64) {
    let sizes = cache.sizes(path, metadata);
    let first_link = visited_files
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert((metadata.dev(), metadata.ino()));
    if first_link {
        sizes
    } else {
        (0, 0)
    }
}

fn should_visit_path(
    path: &Path,
    root: &Path,
    options: &ScanOptions,
    root_dev: Option<u64>,
) -> bool {
    if path == root {
        return true;
    }

    if ScanOptions::is_linux_virtual_fs(path) {
        return false;
    }

    if !options.include_hidden
        && path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'))
    {
        return false;
    }

    if matches_exclusion(path, root, &options.exclude_patterns) {
        return false;
    }

    if let Some(root_dev) = root_dev {
        if fs::metadata(path).is_ok_and(|metadata| metadata.dev() != root_dev) {
            return false;
        }
    }

    true
}

fn matches_exclusion(path: &Path, root: &Path, patterns: &[String]) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let relative_text = relative.to_string_lossy();

    patterns.iter().any(|pattern| {
        let pattern = pattern.trim_start_matches("./");
        if let Some(component_pattern) = pattern.strip_prefix("**/") {
            relative.components().any(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .is_some_and(|name| wildcard_matches(component_pattern, name))
            })
        } else if pattern.contains('/') {
            wildcard_matches(pattern, &relative_text)
        } else {
            relative
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| wildcard_matches(pattern, name))
        }
    })
}

/// Match the `*` and `?` forms needed by scanner exclusion patterns.
fn wildcard_matches(pattern: &str, value: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let value: Vec<char> = value.chars().collect();
    let mut previous = vec![false; value.len() + 1];
    previous[0] = true;

    for token in pattern {
        let mut current = vec![false; value.len() + 1];
        if token == '*' {
            current[0] = previous[0];
        }
        for (index, character) in value.iter().enumerate() {
            current[index + 1] = match token {
                '*' => previous[index + 1] || current[index],
                '?' => previous[index],
                literal => previous[index] && literal == *character,
            };
        }
        previous = current;
    }

    previous[value.len()]
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
    let cache = MetadataCache::load(options.cache.as_ref());
    let result = match configured_thread_pool(options)? {
        Some(pool) => pool.install(|| scan_directory_parallel_inner(root, options, &cache)),
        None => scan_directory_parallel_inner(root, options, &cache),
    };
    if result.is_ok() {
        cache.save();
    }
    result
}

fn configured_thread_pool(options: &ScanOptions) -> Result<Option<rayon::ThreadPool>> {
    if options.threads == 0 {
        return Ok(None);
    }

    rayon::ThreadPoolBuilder::new()
        .num_threads(options.threads)
        .build()
        .map(Some)
        .map_err(|error| SweeperError::Other(format!("Cannot create scanner thread pool: {error}")))
}

fn scan_directory_parallel_inner(
    root: &Path,
    options: &ScanOptions,
    cache: &MetadataCache,
) -> Result<DirEntry> {
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

    let visited_dirs = Arc::new(Mutex::new(HashSet::new()));
    let visited_files = Arc::new(Mutex::new(HashSet::new()));
    scan_dir_recursive_parallel(
        &root,
        &root,
        options,
        0,
        root_dev,
        &visited_dirs,
        &visited_files,
        cache,
    )
}

fn scan_dir_recursive_parallel(
    path: &Path,
    root: &Path,
    options: &ScanOptions,
    depth: usize,
    root_dev: Option<u64>,
    visited_dirs: &Arc<Mutex<HashSet<(u64, u64)>>>,
    visited_files: &Arc<Mutex<HashSet<(u64, u64)>>>,
    cache: &MetadataCache,
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
        let (apparent_size, disk_usage) =
            accounted_file_sizes(cache, path, &metadata, visited_files);
        return Ok(DirEntry::new_file(
            path.to_path_buf(),
            apparent_size,
            disk_usage,
            metadata.modified().ok(),
        ));
    }

    if options.follow_symlinks {
        let identity = (metadata.dev(), metadata.ino());
        let mut visited = visited_dirs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !visited.insert(identity) {
            let mut cycle = DirEntry::new_dir(path.to_path_buf(), metadata.modified().ok());
            cycle.error = Some("Directory already visited through a symbolic link".to_string());
            return Ok(cycle);
        }
    }

    // Check if we've reached the depth limit - if so, return empty directory
    if let Some(max_depth) = options.max_depth {
        if depth >= max_depth {
            return Ok(DirEntry::new_dir(
                path.to_path_buf(),
                metadata.modified().ok(),
            ));
        }
    }

    // Read directory entries
    let read_dir = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(e) => return Ok(DirEntry::new_error(path.to_path_buf(), e.to_string())),
    };

    let entries: Vec<_> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| should_visit_path(&e.path(), root, options, root_dev))
        .collect();

    // Process entries in parallel
    let children: Vec<DirEntry> = entries
        .par_iter()
        .filter_map(|entry| {
            let child_path = entry.path();
            scan_dir_recursive_parallel(
                &child_path,
                root,
                options,
                depth + 1,
                root_dev,
                visited_dirs,
                visited_files,
                cache,
            )
            .ok()
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
    let cache = MetadataCache::load(options.cache.as_ref());
    let thread_pool = match configured_thread_pool(options) {
        Ok(pool) => pool,
        Err(error) => {
            let _ = tx.send(ScanUpdate::Error {
                message: error.to_string(),
            });
            return;
        }
    };

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

    let root_dev = if options.one_file_system {
        Some(metadata.dev())
    } else {
        None
    };

    let visited_dirs = Arc::new(Mutex::new(HashSet::new()));
    let visited_files = Arc::new(Mutex::new(HashSet::new()));
    if options.follow_symlinks {
        visited_dirs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert((metadata.dev(), metadata.ino()));
    }

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
        .filter(|e| should_visit_path(&e.path(), &root, options, root_dev))
        .collect();

    let total = entries.len();
    let root_entry = DirEntry::new_dir(root.clone(), metadata.modified().ok());
    if tx
        .send(ScanUpdate::Started {
            root: root_entry,
            total,
        })
        .is_err()
    {
        return;
    }

    // Process each top-level entry sequentially, but each entry scans in parallel internally
    for (idx, entry) in entries.into_iter().enumerate() {
        let child_path = entry.path();

        // Scan this entry (parallel internally via scan_dir_recursive_parallel)
        let scan_child = || {
            scan_dir_recursive_parallel(
                &child_path,
                &root,
                options,
                1,
                root_dev,
                &visited_dirs,
                &visited_files,
                &cache,
            )
        };
        let child_result = match &thread_pool {
            Some(pool) => pool.install(scan_child),
            None => scan_child(),
        };
        let child_entry = match child_result {
            Ok(e) => e,
            Err(_) => DirEntry::new_error(child_path.clone(), "Scan failed".to_string()),
        };
        if tx
            .send(ScanUpdate::Entry {
                entry: child_entry,
                completed: idx + 1,
                total,
            })
            .is_err()
        {
            return;
        }
    }

    cache.save();
    let _ = tx.send(ScanUpdate::Complete);
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
    fn test_scanners_prune_configured_excluded_subtree() {
        let dir = TempDir::new().unwrap();
        let excluded = dir.path().join("excluded");
        fs::create_dir(&excluded).unwrap();
        File::create(excluded.join("large.bin"))
            .unwrap()
            .write_all(&[b'x'; 100])
            .unwrap();
        File::create(dir.path().join("visible.txt"))
            .unwrap()
            .write_all(b"visible")
            .unwrap();

        let options = ScanOptions::new().with_exclude(vec!["excluded".to_string()]);

        for result in [
            scan_directory(dir.path(), &options).unwrap(),
            scan_directory_parallel(dir.path(), &options).unwrap(),
        ] {
            assert!(result
                .children
                .iter()
                .any(|entry| entry.name == "visible.txt"));
            assert!(
                !result.children.iter().any(|entry| entry.name == "excluded"),
                "excluded subtree must not be returned"
            );
        }
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
        let result = scan_directory(
            Path::new("/nonexistent/path/12345"),
            &ScanOptions::default(),
        );
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

    #[test]
    fn test_scanners_count_hard_link_storage_once() {
        use crate::scanner::size::disk_usage;

        let dir = TempDir::new().unwrap();
        let original = dir.path().join("original.bin");
        let alias = dir.path().join("alias.bin");
        fs::write(&original, vec![b'x'; 128]).unwrap();
        fs::hard_link(&original, &alias).unwrap();
        let expected_disk_usage = disk_usage(&fs::metadata(&original).unwrap());
        let options = ScanOptions::new();

        for tree in [
            scan_directory(dir.path(), &options).unwrap(),
            scan_directory_parallel(dir.path(), &options).unwrap(),
        ] {
            assert_eq!(tree.size, 128);
            assert_eq!(tree.disk_usage, expected_disk_usage);
            assert_eq!(tree.file_count, 2);
        }

        let (tx, rx) = std::sync::mpsc::channel();
        scan_directory_progressive(dir.path(), &options, tx);
        let deltas: Vec<_> = rx
            .into_iter()
            .filter_map(|update| match update {
                ScanUpdate::Entry { entry, .. } => Some(entry),
                _ => None,
            })
            .collect();
        assert_eq!(deltas.iter().map(|entry| entry.size).sum::<u64>(), 128);
        assert_eq!(
            deltas.iter().map(|entry| entry.disk_usage).sum::<u64>(),
            expected_disk_usage
        );
        assert_eq!(deltas.iter().map(|entry| entry.file_count).sum::<u64>(), 2);
    }

    #[test]
    fn test_scan_cache_reuses_unchanged_file_metadata() {
        let dir = TempDir::new().unwrap();
        let cache_dir = TempDir::new().unwrap();
        fs::write(dir.path().join("data.bin"), vec![b'x'; 128]).unwrap();
        let cache_path = cache_dir.path().join("scan-cache.json");
        let options = ScanOptions::new().with_cache(cache_path, 3600);

        let (_, first_stats) = scan_directory_with_cache_stats(dir.path(), &options).unwrap();
        assert_eq!(first_stats.hits, 0);
        assert_eq!(first_stats.misses, 1);

        let (tree, second_stats) = scan_directory_with_cache_stats(dir.path(), &options).unwrap();
        assert_eq!(tree.size, 128);
        assert_eq!(second_stats.hits, 1);
        assert_eq!(second_stats.misses, 0);

        fs::write(dir.path().join("data.bin"), vec![b'y'; 256]).unwrap();
        let (tree, changed_stats) = scan_directory_with_cache_stats(dir.path(), &options).unwrap();
        assert_eq!(tree.size, 256);
        assert_eq!(changed_stats.hits, 0);
        assert_eq!(changed_stats.misses, 1);
    }

    #[test]
    fn test_progressive_scan_sends_each_entry_as_a_delta() {
        let dir = create_test_structure();
        let expected_entries = fs::read_dir(dir.path()).unwrap().count();
        let (tx, rx) = std::sync::mpsc::channel();

        scan_directory_progressive(dir.path(), &ScanOptions::new().with_hidden(true), tx);

        let updates: Vec<_> = rx.into_iter().collect();
        assert!(matches!(updates.first(), Some(ScanUpdate::Started { .. })));
        assert!(matches!(updates.last(), Some(ScanUpdate::Complete)));

        let deltas: Vec<_> = updates
            .iter()
            .filter_map(|update| match update {
                ScanUpdate::Entry { entry, .. } => Some(entry),
                _ => None,
            })
            .collect();
        assert_eq!(deltas.len(), expected_entries);
        assert_eq!(
            deltas
                .iter()
                .map(|entry| entry.path.clone())
                .collect::<HashSet<_>>()
                .len(),
            expected_entries
        );
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
    fn test_parallel_scanner_builds_configured_thread_pool() {
        let options = ScanOptions::new().with_threads(2);
        let pool = configured_thread_pool(&options).unwrap().unwrap();

        assert_eq!(pool.current_num_threads(), 2);
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

    #[test]
    fn test_parallel_scan_does_not_follow_symlink_cycle_repeatedly() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new().unwrap();
        let target = dir.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("file.txt"), "content").unwrap();
        symlink(&target, target.join("loop")).unwrap();

        let options = ScanOptions::new()
            .with_follow_symlinks(true)
            .with_max_depth(12);
        let result = scan_directory_parallel(dir.path(), &options).unwrap();

        assert_eq!(result.file_count, 1, "cycle must not duplicate file sizes");
        let target = result
            .children
            .iter()
            .find(|entry| entry.name == "target")
            .unwrap();
        let loop_entry = target
            .children
            .iter()
            .find(|entry| entry.name == "loop")
            .unwrap();
        assert!(loop_entry.children.is_empty());
        assert!(loop_entry.error.is_some());
    }
}
