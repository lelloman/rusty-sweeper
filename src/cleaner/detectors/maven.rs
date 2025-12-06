//! Maven project detector.

use crate::cleaner::ProjectDetector;

/// Detector for Maven projects.
///
/// Identifies projects by the presence of `pom.xml` and cleans
/// the `target/` directory.
pub struct MavenDetector;

impl ProjectDetector for MavenDetector {
    fn id(&self) -> &'static str {
        "maven"
    }

    fn display_name(&self) -> &'static str {
        "Maven"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &["pom.xml"]
    }

    fn artifact_dirs(&self) -> &'static [&'static str] {
        &["target"]
    }

    fn clean_command(&self) -> Option<&'static str> {
        Some("mvn clean")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_maven_detector_properties() {
        let detector = MavenDetector;

        assert_eq!(detector.id(), "maven");
        assert_eq!(detector.display_name(), "Maven");
        assert_eq!(detector.detection_files(), &["pom.xml"]);
        assert_eq!(detector.artifact_dirs(), &["target"]);
        assert_eq!(detector.clean_command(), Some("mvn clean"));
    }

    #[test]
    fn test_maven_detection() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("pom.xml"), "<project/>").unwrap();

        assert!(MavenDetector.detect(tmp.path()));
    }

    #[test]
    fn test_maven_no_detection() {
        let tmp = TempDir::new().unwrap();

        assert!(!MavenDetector.detect(tmp.path()));
    }
}
