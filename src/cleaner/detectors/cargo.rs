//! Cargo/Rust project detector.

use crate::cleaner::ProjectDetector;

/// Detector for Rust/Cargo projects.
///
/// Identifies projects by the presence of `Cargo.toml` and cleans
/// the `target/` directory using `cargo clean`.
pub struct CargoDetector;

impl ProjectDetector for CargoDetector {
    fn id(&self) -> &'static str {
        "cargo"
    }

    fn display_name(&self) -> &'static str {
        "Rust/Cargo"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &["Cargo.toml"]
    }

    fn artifact_dirs(&self) -> &'static [&'static str] {
        &["target"]
    }

    fn clean_command(&self) -> Option<&'static str> {
        Some("cargo clean")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cargo_detector_properties() {
        let detector = CargoDetector;

        assert_eq!(detector.id(), "cargo");
        assert_eq!(detector.display_name(), "Rust/Cargo");
        assert_eq!(detector.detection_files(), &["Cargo.toml"]);
        assert_eq!(detector.artifact_dirs(), &["target"]);
        assert_eq!(detector.clean_command(), Some("cargo clean"));
    }

    #[test]
    fn test_cargo_detection_positive() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();

        let detector = CargoDetector;
        assert!(detector.detect(tmp.path()));
    }

    #[test]
    fn test_cargo_detection_negative() {
        let tmp = TempDir::new().unwrap();

        let detector = CargoDetector;
        assert!(!detector.detect(tmp.path()));
    }

    #[test]
    fn test_cargo_find_artifacts() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("target")).unwrap();
        fs::create_dir(tmp.path().join("src")).unwrap();

        let detector = CargoDetector;
        let artifacts = detector.find_artifacts(tmp.path());

        assert_eq!(artifacts.len(), 1);
        assert!(artifacts[0].ends_with("target"));
    }

    #[test]
    fn test_cargo_no_artifacts_without_target() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(tmp.path().join("src")).unwrap();

        let detector = CargoDetector;
        let artifacts = detector.find_artifacts(tmp.path());

        assert!(artifacts.is_empty());
    }
}
