//! Python virtual environment detector.

use crate::cleaner::ProjectDetector;

/// Detector for Python projects with virtual environments.
///
/// Identifies projects by the presence of `venv/` or `.venv/` directories
/// and cleans them via direct deletion.
pub struct PythonDetector;

impl ProjectDetector for PythonDetector {
    fn id(&self) -> &'static str {
        "python"
    }

    fn display_name(&self) -> &'static str {
        "Python venv"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &["venv", ".venv"]
    }

    fn artifact_dirs(&self) -> &'static [&'static str] {
        &["venv", ".venv", "__pycache__"]
    }

    fn clean_command(&self) -> Option<&'static str> {
        None // Direct deletion
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_python_detector_properties() {
        let detector = PythonDetector;

        assert_eq!(detector.id(), "python");
        assert_eq!(detector.display_name(), "Python venv");
        assert_eq!(detector.clean_command(), None);
    }

    #[test]
    fn test_python_detection_venv() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("venv")).unwrap();

        assert!(PythonDetector.detect(tmp.path()));
    }

    #[test]
    fn test_python_detection_dot_venv() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".venv")).unwrap();

        assert!(PythonDetector.detect(tmp.path()));
    }

    #[test]
    fn test_python_find_artifacts() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("venv")).unwrap();
        fs::create_dir(tmp.path().join("__pycache__")).unwrap();

        let artifacts = PythonDetector.find_artifacts(tmp.path());
        assert_eq!(artifacts.len(), 2);
    }
}
