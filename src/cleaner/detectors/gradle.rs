//! Gradle/Android project detector.

use crate::cleaner::ProjectDetector;

/// Detector for Gradle/Android projects.
///
/// Identifies projects by the presence of `build.gradle`, `build.gradle.kts`,
/// or `gradlew` and cleans build directories.
pub struct GradleDetector;

impl ProjectDetector for GradleDetector {
    fn id(&self) -> &'static str {
        "gradle"
    }

    fn display_name(&self) -> &'static str {
        "Gradle/Android"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &["build.gradle", "build.gradle.kts", "gradlew"]
    }

    fn artifact_dirs(&self) -> &'static [&'static str] {
        &["build", ".gradle", "app/build"]
    }

    fn clean_command(&self) -> Option<&'static str> {
        Some("./gradlew clean")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_gradle_detector_properties() {
        let detector = GradleDetector;

        assert_eq!(detector.id(), "gradle");
        assert_eq!(detector.display_name(), "Gradle/Android");
        assert_eq!(detector.clean_command(), Some("./gradlew clean"));
    }

    #[test]
    fn test_gradle_detection_build_gradle() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("build.gradle"), "").unwrap();

        assert!(GradleDetector.detect(tmp.path()));
    }

    #[test]
    fn test_gradle_detection_kotlin_dsl() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("build.gradle.kts"), "").unwrap();

        assert!(GradleDetector.detect(tmp.path()));
    }

    #[test]
    fn test_gradle_detection_gradlew() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("gradlew"), "").unwrap();

        assert!(GradleDetector.detect(tmp.path()));
    }

    #[test]
    fn test_gradle_find_artifacts() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("build")).unwrap();
        fs::create_dir(tmp.path().join(".gradle")).unwrap();

        let artifacts = GradleDetector.find_artifacts(tmp.path());
        assert_eq!(artifacts.len(), 2);
    }
}
