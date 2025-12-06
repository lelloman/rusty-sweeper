//! CMake project detector.

use crate::cleaner::ProjectDetector;
use std::path::Path;

/// Detector for CMake projects.
///
/// Identifies projects by the presence of `CMakeLists.txt` AND a `build/`
/// directory. This avoids false positives on CMake projects that haven't
/// been built yet.
pub struct CMakeDetector;

impl ProjectDetector for CMakeDetector {
    fn id(&self) -> &'static str {
        "cmake"
    }

    fn display_name(&self) -> &'static str {
        "CMake"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &["CMakeLists.txt"]
    }

    fn artifact_dirs(&self) -> &'static [&'static str] {
        &["build"]
    }

    fn clean_command(&self) -> Option<&'static str> {
        None // Direct deletion
    }

    /// Override: only detect if CMakeLists.txt AND build/ exist.
    fn detect(&self, path: &Path) -> bool {
        path.join("CMakeLists.txt").exists() && path.join("build").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cmake_detector_properties() {
        let detector = CMakeDetector;

        assert_eq!(detector.id(), "cmake");
        assert_eq!(detector.display_name(), "CMake");
        assert_eq!(detector.artifact_dirs(), &["build"]);
        assert_eq!(detector.clean_command(), None);
    }

    #[test]
    fn test_cmake_detection_with_build() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("CMakeLists.txt"), "cmake_minimum_required(VERSION 3.10)").unwrap();
        fs::create_dir(tmp.path().join("build")).unwrap();

        assert!(CMakeDetector.detect(tmp.path()));
    }

    #[test]
    fn test_cmake_no_detection_without_build() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("CMakeLists.txt"), "cmake_minimum_required(VERSION 3.10)").unwrap();

        assert!(!CMakeDetector.detect(tmp.path()));
    }

    #[test]
    fn test_cmake_no_detection_without_cmakelists() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("build")).unwrap();

        assert!(!CMakeDetector.detect(tmp.path()));
    }
}
