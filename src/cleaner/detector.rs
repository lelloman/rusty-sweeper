//! Core trait and types for project detection.

use std::path::{Path, PathBuf};

/// Represents a detected project with its metadata.
#[derive(Debug, Clone)]
pub struct DetectedProject {
    /// Root path of the project.
    pub path: PathBuf,
    /// Type identifier (e.g., "cargo", "gradle").
    pub project_type: String,
    /// Human-readable name (e.g., "Rust/Cargo").
    pub display_name: String,
    /// Total size of artifact directories in bytes.
    pub artifact_size: u64,
    /// List of artifact directories found.
    pub artifact_paths: Vec<PathBuf>,
}

/// Trait for project type detectors.
///
/// Implement this trait to add detection support for a new project type.
/// The detector is responsible for:
/// - Identifying whether a directory contains a specific project type
/// - Listing the artifact directories that can be cleaned
/// - Providing the clean command (if a native one exists)
pub trait ProjectDetector: Send + Sync {
    /// Unique identifier for this project type (e.g., "cargo").
    fn id(&self) -> &'static str;

    /// Human-readable name (e.g., "Rust/Cargo").
    fn display_name(&self) -> &'static str;

    /// Files that indicate this project type exists.
    ///
    /// Returns true if ANY of these files/directories exist.
    fn detection_files(&self) -> &'static [&'static str];

    /// Directories containing build artifacts.
    fn artifact_dirs(&self) -> &'static [&'static str];

    /// Command to clean the project.
    ///
    /// Returns `None` if direct deletion should be used instead.
    fn clean_command(&self) -> Option<&'static str>;

    /// Check if this project type exists at the given path.
    ///
    /// Default implementation checks if any detection file exists.
    fn detect(&self, path: &Path) -> bool {
        self.detection_files().iter().any(|f| path.join(f).exists())
    }

    /// Get existing artifact directories at the given path.
    ///
    /// Only returns directories that actually exist.
    fn find_artifacts(&self, path: &Path) -> Vec<PathBuf> {
        self.artifact_dirs()
            .iter()
            .map(|d| path.join(d))
            .filter(|p| p.exists())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDetector;

    impl ProjectDetector for MockDetector {
        fn id(&self) -> &'static str {
            "mock"
        }

        fn display_name(&self) -> &'static str {
            "Mock Project"
        }

        fn detection_files(&self) -> &'static [&'static str] {
            &["mock.toml"]
        }

        fn artifact_dirs(&self) -> &'static [&'static str] {
            &["build"]
        }

        fn clean_command(&self) -> Option<&'static str> {
            Some("mock clean")
        }
    }

    #[test]
    fn test_detected_project_creation() {
        let project = DetectedProject {
            path: PathBuf::from("/test"),
            project_type: "mock".to_string(),
            display_name: "Mock".to_string(),
            artifact_size: 1024,
            artifact_paths: vec![PathBuf::from("/test/build")],
        };

        assert_eq!(project.project_type, "mock");
        assert_eq!(project.artifact_size, 1024);
        assert_eq!(project.path, PathBuf::from("/test"));
        assert_eq!(project.artifact_paths.len(), 1);
    }

    #[test]
    fn test_detector_trait_methods() {
        let detector = MockDetector;

        assert_eq!(detector.id(), "mock");
        assert_eq!(detector.display_name(), "Mock Project");
        assert_eq!(detector.detection_files(), &["mock.toml"]);
        assert_eq!(detector.artifact_dirs(), &["build"]);
        assert_eq!(detector.clean_command(), Some("mock clean"));
    }

    #[test]
    fn test_detect_with_existing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("mock.toml"), "").unwrap();

        let detector = MockDetector;
        assert!(detector.detect(tmp.path()));
    }

    #[test]
    fn test_detect_without_file() {
        let tmp = tempfile::TempDir::new().unwrap();

        let detector = MockDetector;
        assert!(!detector.detect(tmp.path()));
    }

    #[test]
    fn test_find_artifacts_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("build")).unwrap();
        std::fs::create_dir(tmp.path().join("src")).unwrap();

        let detector = MockDetector;
        let artifacts = detector.find_artifacts(tmp.path());

        assert_eq!(artifacts.len(), 1);
        assert!(artifacts[0].ends_with("build"));
    }

    #[test]
    fn test_find_artifacts_none_exist() {
        let tmp = tempfile::TempDir::new().unwrap();

        let detector = MockDetector;
        let artifacts = detector.find_artifacts(tmp.path());

        assert!(artifacts.is_empty());
    }
}
