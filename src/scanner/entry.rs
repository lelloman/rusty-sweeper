use std::path::PathBuf;
use std::time::SystemTime;
use serde::Serialize;

/// Represents a file or directory with its metadata and size information.
/// This is the core data structure for the disk scanner.
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

    /// Error message if we couldn't read this entry (permission denied, etc.)
    pub error: Option<String>,
}

impl DirEntry {
    /// Create a new file entry from metadata
    pub fn new_file(path: PathBuf, size: u64, disk_usage: u64, mtime: Option<SystemTime>) -> Self {
        let name = path
            .file_name()
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
        let name = path
            .file_name()
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
        let name = path
            .file_name()
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
        assert_eq!(entry.name, "test");
        assert_eq!(entry.file_count, 5);
        assert_eq!(entry.dir_count, 2);
    }

    #[test]
    fn test_dir_entry_with_error() {
        let entry = DirEntry {
            path: PathBuf::from("/forbidden"),
            name: "forbidden".to_string(),
            is_dir: true,
            size: 0,
            disk_usage: 0,
            file_count: 0,
            dir_count: 0,
            mtime: None,
            children: vec![],
            error: Some("Permission denied".to_string()),
        };
        assert!(entry.error.is_some());
        assert_eq!(entry.error.as_ref().unwrap(), "Permission denied");
    }

    #[test]
    fn test_new_file() {
        let entry = DirEntry::new_file(
            PathBuf::from("/parent/file.txt"),
            1024,
            4096,
            None,
        );
        assert!(!entry.is_dir);
        assert_eq!(entry.name, "file.txt");
        assert_eq!(entry.size, 1024);
        assert_eq!(entry.disk_usage, 4096);
        assert_eq!(entry.file_count, 1);
        assert_eq!(entry.dir_count, 0);
    }

    #[test]
    fn test_new_dir() {
        let entry = DirEntry::new_dir(PathBuf::from("/parent/subdir"), None);
        assert!(entry.is_dir);
        assert_eq!(entry.name, "subdir");
        assert_eq!(entry.size, 0);
        assert_eq!(entry.file_count, 0);
    }

    #[test]
    fn test_new_dir_root_path() {
        let entry = DirEntry::new_dir(PathBuf::from("/"), None);
        assert!(entry.is_dir);
        assert_eq!(entry.name, "/");
    }

    #[test]
    fn test_new_error() {
        let entry = DirEntry::new_error(
            PathBuf::from("/forbidden/file"),
            "Permission denied".to_string(),
        );
        assert!(entry.error.is_some());
        assert_eq!(entry.name, "file");
        assert_eq!(entry.size, 0);
    }

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
    fn test_recalculate_totals_with_subdirs() {
        let mut parent = DirEntry::new_dir(PathBuf::from("/parent"), None);

        let mut subdir = DirEntry::new_dir(PathBuf::from("/parent/subdir"), None);
        subdir.children.push(DirEntry::new_file(
            PathBuf::from("/parent/subdir/nested.txt"),
            500,
            4096,
            None,
        ));
        subdir.recalculate_totals();

        parent.children.push(subdir);
        parent.children.push(DirEntry::new_file(
            PathBuf::from("/parent/file.txt"),
            100,
            4096,
            None,
        ));
        parent.recalculate_totals();

        assert_eq!(parent.size, 600);
        assert_eq!(parent.file_count, 2);
        assert_eq!(parent.dir_count, 1);
    }

    #[test]
    fn test_sort_by_size() {
        let mut parent = DirEntry::new_dir(PathBuf::from("/parent"), None);
        parent.children.push(DirEntry::new_file(
            PathBuf::from("/parent/small.txt"),
            100,
            4096,
            None,
        ));
        parent.children.push(DirEntry::new_file(
            PathBuf::from("/parent/large.txt"),
            1000,
            4096,
            None,
        ));
        parent.children.push(DirEntry::new_file(
            PathBuf::from("/parent/medium.txt"),
            500,
            4096,
            None,
        ));

        parent.sort_by_size();

        assert_eq!(parent.children[0].name, "large.txt");
        assert_eq!(parent.children[1].name, "medium.txt");
        assert_eq!(parent.children[2].name, "small.txt");
    }

    #[test]
    fn test_sort_by_name() {
        let mut parent = DirEntry::new_dir(PathBuf::from("/parent"), None);
        parent.children.push(DirEntry::new_file(
            PathBuf::from("/parent/charlie.txt"),
            100,
            4096,
            None,
        ));
        parent.children.push(DirEntry::new_file(
            PathBuf::from("/parent/alpha.txt"),
            100,
            4096,
            None,
        ));
        parent.children.push(DirEntry::new_file(
            PathBuf::from("/parent/bravo.txt"),
            100,
            4096,
            None,
        ));

        parent.sort_by_name();

        assert_eq!(parent.children[0].name, "alpha.txt");
        assert_eq!(parent.children[1].name, "bravo.txt");
        assert_eq!(parent.children[2].name, "charlie.txt");
    }

    #[test]
    fn test_total_entries() {
        let mut parent = DirEntry::new_dir(PathBuf::from("/parent"), None);
        parent.file_count = 5;
        parent.dir_count = 2;

        assert_eq!(parent.total_entries(), 8); // 5 files + 2 dirs + 1 (self)
    }

    #[test]
    fn test_total_entries_file() {
        let file = DirEntry::new_file(PathBuf::from("/file.txt"), 100, 4096, None);
        assert_eq!(file.total_entries(), 1); // Just the file itself
    }
}
