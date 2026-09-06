//! Executor for cleaning project artifacts.

use crate::cleaner::detector::DetectedProject;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

/// Result of a clean operation.
#[derive(Debug, Clone)]
pub enum CleanResult {
    /// Cleaning succeeded.
    Success {
        project: DetectedProject,
        freed_bytes: u64,
    },
    /// Cleaning failed.
    Failed {
        project: DetectedProject,
        error: String,
        /// Bytes removed before the operation failed.
        freed_bytes: u64,
    },
    /// Cleaning was skipped.
    Skipped {
        project: DetectedProject,
        reason: String,
    },
}

/// Options for the clean executor.
#[derive(Debug, Clone)]
pub struct CleanOptions {
    /// If true, don't actually delete anything.
    pub dry_run: bool,
    /// If true, try native clean commands before direct deletion.
    pub use_native_commands: bool,
}

impl Default for CleanOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            use_native_commands: true,
        }
    }
}

/// Executor for cleaning project artifacts.
pub struct CleanExecutor {
    options: CleanOptions,
}

impl CleanExecutor {
    /// Create a new executor with the given options.
    pub fn new(options: CleanOptions) -> Self {
        Self { options }
    }

    /// Clean a single project.
    ///
    /// If `clean_command` is provided and native commands are enabled,
    /// it will be tried first. Falls back to direct deletion on failure.
    pub fn clean(&self, project: &DetectedProject, clean_command: Option<&str>) -> CleanResult {
        if self.options.dry_run {
            return CleanResult::Success {
                project: project.clone(),
                freed_bytes: project.artifact_size,
            };
        }

        // Try native command first if available and enabled
        if self.options.use_native_commands {
            if let Some(cmd) = clean_command {
                let size_before = Self::artifact_size(&project.artifact_paths);
                match self.run_clean_command(&project.path, cmd) {
                    Ok(()) => {
                        let size_after = Self::artifact_size(&project.artifact_paths);
                        return CleanResult::Success {
                            project: project.clone(),
                            freed_bytes: size_before.saturating_sub(size_after),
                        };
                    }
                    Err(e) => {
                        if e.kind() == io::ErrorKind::NotFound {
                            // A project may be a Gradle submodule without its
                            // own wrapper. Falling back to direct deletion is
                            // expected and should not corrupt the TUI with a
                            // warning on stderr.
                            tracing::debug!(
                                "Native clean command unavailable for {}, falling back to direct deletion",
                                project.path.display()
                            );
                        } else {
                            tracing::warn!(
                                "Native clean command failed for {}: {}, falling back to direct deletion",
                                project.path.display(),
                                e
                            );
                        }
                    }
                }
            }
        }

        // Direct deletion
        match self.delete_artifacts(project) {
            Ok(freed) => CleanResult::Success {
                project: project.clone(),
                freed_bytes: freed,
            },
            Err((e, freed_bytes)) => CleanResult::Failed {
                project: project.clone(),
                error: e.to_string(),
                freed_bytes,
            },
        }
    }

    fn run_clean_command(&self, project_path: &Path, command: &str) -> io::Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Empty command"));
        }

        let (program, args) = parts.split_first().unwrap();

        let output = Command::new(program)
            .args(args)
            .current_dir(project_path)
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(io::Error::other(format!("Command failed: {}", stderr)))
        }
    }

    fn delete_artifacts(&self, project: &DetectedProject) -> Result<u64, (io::Error, u64)> {
        let mut freed = 0u64;

        for artifact_path in &project.artifact_paths {
            if artifact_path.exists() {
                let size = Self::dir_size(artifact_path);
                if let Err(error) = fs::remove_dir_all(artifact_path) {
                    return Err((error, freed));
                }
                freed += size;
            }
        }

        Ok(freed)
    }

    fn dir_size(path: &Path) -> u64 {
        WalkDir::new(path)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum()
    }

    fn artifact_size(paths: &[std::path::PathBuf]) -> u64 {
        paths.iter().map(|path| Self::dir_size(path)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_project() -> (TempDir, DetectedProject) {
        let tmp = TempDir::new().unwrap();

        let target = tmp.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("artifact.bin"), "x".repeat(1000)).unwrap();

        let project = DetectedProject {
            path: tmp.path().to_path_buf(),
            project_type: "test".to_string(),
            display_name: "Test".to_string(),
            artifact_size: 1000,
            artifact_paths: vec![target],
        };

        (tmp, project)
    }

    #[test]
    fn test_clean_dry_run() {
        let (_tmp, project) = create_test_project();

        let executor = CleanExecutor::new(CleanOptions {
            dry_run: true,
            use_native_commands: false,
        });

        let result = executor.clean(&project, None);

        assert!(matches!(result, CleanResult::Success { .. }));
        // Artifacts should still exist
        assert!(project.artifact_paths[0].exists());
    }

    #[test]
    fn test_clean_direct_deletion() {
        let (tmp, project) = create_test_project();

        let executor = CleanExecutor::new(CleanOptions {
            dry_run: false,
            use_native_commands: false,
        });

        let result = executor.clean(&project, None);

        match result {
            CleanResult::Success { freed_bytes, .. } => {
                assert_eq!(freed_bytes, 1000);
            }
            _ => panic!("Expected success"),
        }

        // Artifacts should be gone
        assert!(!tmp.path().join("target").exists());
    }

    #[test]
    fn test_clean_nonexistent_artifacts() {
        let tmp = TempDir::new().unwrap();

        let project = DetectedProject {
            path: tmp.path().to_path_buf(),
            project_type: "test".to_string(),
            display_name: "Test".to_string(),
            artifact_size: 0,
            artifact_paths: vec![tmp.path().join("nonexistent")],
        };

        let executor = CleanExecutor::new(CleanOptions {
            dry_run: false,
            use_native_commands: false,
        });

        let result = executor.clean(&project, None);

        // Should succeed with 0 bytes freed
        assert!(matches!(
            result,
            CleanResult::Success { freed_bytes: 0, .. }
        ));
    }

    #[test]
    fn test_clean_multiple_artifacts() {
        let tmp = TempDir::new().unwrap();

        let target = tmp.path().join("target");
        let build = tmp.path().join("build");
        fs::create_dir(&target).unwrap();
        fs::create_dir(&build).unwrap();
        fs::write(target.join("a.bin"), "x".repeat(500)).unwrap();
        fs::write(build.join("b.bin"), "x".repeat(300)).unwrap();

        let project = DetectedProject {
            path: tmp.path().to_path_buf(),
            project_type: "test".to_string(),
            display_name: "Test".to_string(),
            artifact_size: 800,
            artifact_paths: vec![target.clone(), build.clone()],
        };

        let executor = CleanExecutor::new(CleanOptions {
            dry_run: false,
            use_native_commands: false,
        });

        let result = executor.clean(&project, None);

        match result {
            CleanResult::Success { freed_bytes, .. } => {
                assert_eq!(freed_bytes, 800);
            }
            _ => panic!("Expected success"),
        }

        assert!(!target.exists());
        assert!(!build.exists());
    }

    #[test]
    fn test_clean_result_variants() {
        let project = DetectedProject {
            path: PathBuf::from("/test"),
            project_type: "test".to_string(),
            display_name: "Test".to_string(),
            artifact_size: 100,
            artifact_paths: vec![],
        };

        // Test all variants can be constructed
        let _success = CleanResult::Success {
            project: project.clone(),
            freed_bytes: 100,
        };

        let _failed = CleanResult::Failed {
            project: project.clone(),
            error: "error".to_string(),
            freed_bytes: 0,
        };

        let _skipped = CleanResult::Skipped {
            project,
            reason: "reason".to_string(),
        };
    }

    #[test]
    fn test_clean_reports_bytes_removed_before_failure() {
        let tmp = TempDir::new().unwrap();
        let removable = tmp.path().join("removable");
        let invalid = tmp.path().join("not-a-directory");
        fs::create_dir(&removable).unwrap();
        fs::write(removable.join("artifact"), "x".repeat(100)).unwrap();
        fs::write(&invalid, "file").unwrap();
        let project = DetectedProject {
            path: tmp.path().to_path_buf(),
            project_type: "test".into(),
            display_name: "Test".into(),
            artifact_size: 104,
            artifact_paths: vec![removable.clone(), invalid],
        };
        let executor = CleanExecutor::new(CleanOptions {
            dry_run: false,
            use_native_commands: false,
        });

        let result = executor.clean(&project, None);

        assert!(!removable.exists());
        assert!(matches!(
            result,
            CleanResult::Failed {
                freed_bytes: 100,
                ..
            }
        ));
    }

    #[test]
    fn test_native_clean_reports_measured_bytes_freed() {
        let (_tmp, project) = create_test_project();
        let executor = CleanExecutor::new(CleanOptions {
            dry_run: false,
            use_native_commands: true,
        });

        let result = executor.clean(&project, Some("true"));

        assert!(project.artifact_paths[0].exists());
        assert!(matches!(
            result,
            CleanResult::Success { freed_bytes: 0, .. }
        ));
    }
}
