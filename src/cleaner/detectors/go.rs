//! Go module project detector.

use crate::cleaner::ProjectDetector;

/// Detector for Go module projects.
///
/// Identifies projects by the presence of `go.mod`. Uses `go clean -cache`
/// which cleans the global Go build cache.
pub struct GoDetector;

impl ProjectDetector for GoDetector {
    fn id(&self) -> &'static str {
        "go"
    }

    fn display_name(&self) -> &'static str {
        "Go"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &["go.mod"]
    }

    fn artifact_dirs(&self) -> &'static [&'static str] {
        &[] // Go uses global cache, no local artifacts
    }

    fn clean_command(&self) -> Option<&'static str> {
        Some("go clean -cache")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_go_detector_properties() {
        let detector = GoDetector;

        assert_eq!(detector.id(), "go");
        assert_eq!(detector.display_name(), "Go");
        assert_eq!(detector.detection_files(), &["go.mod"]);
        assert!(detector.artifact_dirs().is_empty());
        assert_eq!(detector.clean_command(), Some("go clean -cache"));
    }

    #[test]
    fn test_go_detection() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("go.mod"), "module example.com/app").unwrap();

        assert!(GoDetector.detect(tmp.path()));
    }

    #[test]
    fn test_go_no_local_artifacts() {
        let tmp = TempDir::new().unwrap();

        let artifacts = GoDetector.find_artifacts(tmp.path());
        assert!(artifacts.is_empty());
    }
}
