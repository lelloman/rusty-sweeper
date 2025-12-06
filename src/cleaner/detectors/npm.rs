//! npm/Node.js project detector.

use crate::cleaner::ProjectDetector;

/// Detector for npm/Node.js projects.
///
/// Identifies projects by the presence of `package.json` and removes
/// the `node_modules/` directory directly (no native clean command).
pub struct NpmDetector;

impl ProjectDetector for NpmDetector {
    fn id(&self) -> &'static str {
        "npm"
    }

    fn display_name(&self) -> &'static str {
        "npm/Node.js"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &["package.json"]
    }

    fn artifact_dirs(&self) -> &'static [&'static str] {
        &["node_modules"]
    }

    fn clean_command(&self) -> Option<&'static str> {
        None // Direct deletion is more reliable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_npm_detector_properties() {
        let detector = NpmDetector;

        assert_eq!(detector.id(), "npm");
        assert_eq!(detector.display_name(), "npm/Node.js");
        assert_eq!(detector.detection_files(), &["package.json"]);
        assert_eq!(detector.artifact_dirs(), &["node_modules"]);
        assert_eq!(detector.clean_command(), None);
    }

    #[test]
    fn test_npm_detection() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();

        assert!(NpmDetector.detect(tmp.path()));
    }

    #[test]
    fn test_npm_find_artifacts() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("node_modules")).unwrap();

        let artifacts = NpmDetector.find_artifacts(tmp.path());
        assert_eq!(artifacts.len(), 1);
        assert!(artifacts[0].ends_with("node_modules"));
    }
}
