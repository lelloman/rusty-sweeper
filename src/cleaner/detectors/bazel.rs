//! Bazel project detector.

use crate::cleaner::ProjectDetector;

/// Detector for Bazel projects.
///
/// Identifies projects by the presence of `WORKSPACE` or `WORKSPACE.bazel`
/// and uses `bazel clean --expunge` to clean.
pub struct BazelDetector;

impl ProjectDetector for BazelDetector {
    fn id(&self) -> &'static str {
        "bazel"
    }

    fn display_name(&self) -> &'static str {
        "Bazel"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &["WORKSPACE", "WORKSPACE.bazel"]
    }

    fn artifact_dirs(&self) -> &'static [&'static str] {
        &[] // bazel clean handles it
    }

    fn clean_command(&self) -> Option<&'static str> {
        Some("bazel clean --expunge")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_bazel_detector_properties() {
        let detector = BazelDetector;

        assert_eq!(detector.id(), "bazel");
        assert_eq!(detector.display_name(), "Bazel");
        assert!(detector.artifact_dirs().is_empty());
        assert_eq!(detector.clean_command(), Some("bazel clean --expunge"));
    }

    #[test]
    fn test_bazel_detection_workspace() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("WORKSPACE"), "").unwrap();

        assert!(BazelDetector.detect(tmp.path()));
    }

    #[test]
    fn test_bazel_detection_workspace_bazel() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("WORKSPACE.bazel"), "").unwrap();

        assert!(BazelDetector.detect(tmp.path()));
    }

    #[test]
    fn test_bazel_no_detection() {
        let tmp = TempDir::new().unwrap();

        assert!(!BazelDetector.detect(tmp.path()));
    }
}
