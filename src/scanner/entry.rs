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
}
