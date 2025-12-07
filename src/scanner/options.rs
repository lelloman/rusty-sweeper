/// Configuration options for directory scanning operations.
#[derive(Debug, Clone, Default)]
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

/// Linux virtual filesystem paths that should be excluded by default.
/// These can report incorrect/huge sizes and cause scanning issues.
pub const LINUX_VIRTUAL_FS_PATHS: &[&str] = &["/proc", "/dev", "/sys", "/run"];


impl ScanOptions {
    /// Check if a path should be excluded based on Linux virtual filesystem paths
    pub fn is_linux_virtual_fs(path: &std::path::Path) -> bool {
        let path_str = path.to_string_lossy();
        LINUX_VIRTUAL_FS_PATHS
            .iter()
            .any(|vfs| path_str.starts_with(vfs) || path_str.as_ref() == *vfs)
    }

    /// Create a new ScanOptions with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum recursion depth
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Set whether to include hidden files
    pub fn with_hidden(mut self, include: bool) -> Self {
        self.include_hidden = include;
        self
    }

    /// Set whether to stay on the same filesystem
    pub fn with_one_file_system(mut self, enabled: bool) -> Self {
        self.one_file_system = enabled;
        self
    }

    /// Set number of parallel threads
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    /// Set exclusion patterns
    pub fn with_exclude(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns = patterns;
        self
    }

    /// Set whether to follow symbolic links
    pub fn with_follow_symlinks(mut self, follow: bool) -> Self {
        self.follow_symlinks = follow;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_options() {
        let opts = ScanOptions::default();
        assert_eq!(opts.max_depth, None);
        assert!(!opts.include_hidden);
        assert!(!opts.one_file_system);
        assert_eq!(opts.threads, 0);
        assert!(opts.exclude_patterns.is_empty());
        assert!(!opts.follow_symlinks);
    }

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

    #[test]
    fn test_scan_options_threads() {
        let opts = ScanOptions::new().with_threads(4);
        assert_eq!(opts.threads, 4);
    }

    #[test]
    fn test_scan_options_exclude() {
        let opts = ScanOptions::new().with_exclude(vec![
            "*.tmp".to_string(),
            "node_modules".to_string(),
        ]);
        assert_eq!(opts.exclude_patterns.len(), 2);
        assert_eq!(opts.exclude_patterns[0], "*.tmp");
    }

    #[test]
    fn test_scan_options_follow_symlinks() {
        let opts = ScanOptions::new().with_follow_symlinks(true);
        assert!(opts.follow_symlinks);
    }

    #[test]
    fn test_scan_options_chaining() {
        let opts = ScanOptions::new()
            .with_max_depth(10)
            .with_hidden(true)
            .with_threads(8)
            .with_one_file_system(true)
            .with_follow_symlinks(false)
            .with_exclude(vec!["*.log".to_string()]);

        assert_eq!(opts.max_depth, Some(10));
        assert!(opts.include_hidden);
        assert_eq!(opts.threads, 8);
        assert!(opts.one_file_system);
        assert!(!opts.follow_symlinks);
        assert_eq!(opts.exclude_patterns.len(), 1);
    }

    #[test]
    fn test_is_linux_virtual_fs() {
        use std::path::Path;

        // Should detect virtual filesystem paths
        assert!(ScanOptions::is_linux_virtual_fs(Path::new("/proc")));
        assert!(ScanOptions::is_linux_virtual_fs(Path::new("/proc/1/status")));
        assert!(ScanOptions::is_linux_virtual_fs(Path::new("/dev")));
        assert!(ScanOptions::is_linux_virtual_fs(Path::new("/dev/sda")));
        assert!(ScanOptions::is_linux_virtual_fs(Path::new("/sys")));
        assert!(ScanOptions::is_linux_virtual_fs(Path::new("/sys/class/net")));
        assert!(ScanOptions::is_linux_virtual_fs(Path::new("/run")));
        assert!(ScanOptions::is_linux_virtual_fs(Path::new("/run/user/1000")));

        // Should not detect regular paths
        assert!(!ScanOptions::is_linux_virtual_fs(Path::new("/home")));
        assert!(!ScanOptions::is_linux_virtual_fs(Path::new("/home/user")));
        assert!(!ScanOptions::is_linux_virtual_fs(Path::new("/tmp")));
        assert!(!ScanOptions::is_linux_virtual_fs(Path::new("/var/log")));
        assert!(!ScanOptions::is_linux_virtual_fs(Path::new("/usr/bin")));
    }
}
