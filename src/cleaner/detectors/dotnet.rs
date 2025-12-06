//! .NET project detector.

use crate::cleaner::ProjectDetector;
use std::path::Path;

/// Detector for .NET projects.
///
/// Identifies projects by the presence of `*.csproj` or `*.sln` files
/// and cleans `bin/` and `obj/` directories.
pub struct DotnetDetector;

impl ProjectDetector for DotnetDetector {
    fn id(&self) -> &'static str {
        "dotnet"
    }

    fn display_name(&self) -> &'static str {
        ".NET"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &[] // Uses custom detection
    }

    fn artifact_dirs(&self) -> &'static [&'static str] {
        &["bin", "obj"]
    }

    fn clean_command(&self) -> Option<&'static str> {
        Some("dotnet clean")
    }

    /// Custom detection for .csproj and .sln files.
    fn detect(&self, path: &Path) -> bool {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "csproj" || ext == "sln" {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_dotnet_detector_properties() {
        let detector = DotnetDetector;

        assert_eq!(detector.id(), "dotnet");
        assert_eq!(detector.display_name(), ".NET");
        assert_eq!(detector.artifact_dirs(), &["bin", "obj"]);
        assert_eq!(detector.clean_command(), Some("dotnet clean"));
    }

    #[test]
    fn test_dotnet_detection_csproj() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("MyApp.csproj"), "<Project/>").unwrap();

        assert!(DotnetDetector.detect(tmp.path()));
    }

    #[test]
    fn test_dotnet_detection_sln() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("MyApp.sln"), "").unwrap();

        assert!(DotnetDetector.detect(tmp.path()));
    }

    #[test]
    fn test_dotnet_no_detection() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("readme.md"), "").unwrap();

        assert!(!DotnetDetector.detect(tmp.path()));
    }

    #[test]
    fn test_dotnet_find_artifacts() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("bin")).unwrap();
        fs::create_dir(tmp.path().join("obj")).unwrap();

        let artifacts = DotnetDetector.find_artifacts(tmp.path());
        assert_eq!(artifacts.len(), 2);
    }
}
