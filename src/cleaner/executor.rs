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
        /// Bytes removed, including cleanup that continued after errors.
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

        let mut native_error = None;
        let size_before = Self::artifact_size(&project.artifact_paths);

        // Try native command first if available and enabled
        if self.options.use_native_commands {
            if let Some(cmd) = clean_command {
                match self.run_clean_command(&project.path, cmd) {
                    Ok(()) => {
                        let size_after = Self::artifact_size(&project.artifact_paths);
                        return CleanResult::Success {
                            project: project.clone(),
                            freed_bytes: size_before.saturating_sub(size_after),
                        };
                    }
                    Err(e) => {
                        // Report diagnostics through the result, never through
                        // terminal logging (even when verbose logging is on).
                        native_error = Some(e);
                    }
                }
            }
        }

        // Direct deletion
        let result = self.delete_artifacts(project);
        let freed_bytes = size_before.saturating_sub(Self::artifact_size(&project.artifact_paths));
        match result {
            Ok(()) => CleanResult::Success {
                project: project.clone(),
                freed_bytes,
            },
            Err(e) => CleanResult::Failed {
                project: project.clone(),
                error: match native_error {
                    Some(native) => format!("{e}; native clean failed: {native}"),
                    None => e.to_string(),
                },
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

    fn delete_artifacts(&self, project: &DetectedProject) -> io::Result<()> {
        let mut first_error = None;
        for artifact_path in &project.artifact_paths {
            if let Err(error) = Self::delete_artifact(artifact_path) {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    fn removal_error(path: &Path, error: io::Error) -> io::Error {
        io::Error::new(
            error.kind(),
            format!("Failed to remove {}: {error}", path.display()),
        )
    }

    fn delete_artifact(path: &Path) -> io::Result<()> {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                // Preserve directory-only artifact handling. Never traverse a
                // symlink that replaced the directory after removal failed.
                if !fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()) {
                    return Err(Self::removal_error(path, error));
                }
            }
        }

        // remove_dir_all stops at the first error. Visit the remaining tree
        // bottom-up so an inaccessible entry doesn't block removable siblings.
        let mut first_error = None;
        for entry in WalkDir::new(path)
            .follow_links(false)
            .follow_root_links(false)
            .contents_first(true)
        {
            let result = match entry {
                Ok(entry) => {
                    let result = if entry.file_type().is_dir() {
                        fs::remove_dir(entry.path())
                    } else {
                        fs::remove_file(entry.path())
                    };
                    result.map_err(|error| Self::removal_error(entry.path(), error))
                }
                Err(error) => {
                    let failed_path = error.path().unwrap_or(path).to_path_buf();
                    Err(Self::removal_error(&failed_path, error.into()))
                }
            };
            if let Err(error) = result {
                if error.kind() != io::ErrorKind::NotFound {
                    first_error.get_or_insert(error);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
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
    fn test_clean_continues_to_artifacts_after_failure() {
        let (tmp, mut project) = create_test_project();
        let invalid = tmp.path().join("not-a-directory");
        fs::write(&invalid, "file").unwrap();
        project.artifact_paths.insert(0, invalid);

        let result = CleanExecutor::new(CleanOptions::default()).clean(&project, None);

        assert!(!tmp.path().join("target").exists());
        assert!(matches!(
            result,
            CleanResult::Failed {
                freed_bytes: 1000,
                ..
            }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_clean_continues_past_unwritable_directory_without_following_symlinks() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let (tmp, project) = create_test_project();
        let target = &project.artifact_paths[0];
        let blocked = target.join("blocked");
        fs::create_dir(&blocked).unwrap();
        fs::write(blocked.join("keep"), "protected").unwrap();
        let outside = tmp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("keep"), "outside").unwrap();
        symlink(&outside, target.join("link")).unwrap();
        fs::create_dir(target.join("sibling")).unwrap();
        fs::write(target.join("sibling/removable"), "x".repeat(200)).unwrap();
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o555)).unwrap();

        let result = CleanExecutor::new(CleanOptions::default()).clean(&project, None);
        // Restore permissions before assertions so the fixture can be removed.
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o755)).unwrap();

        assert!(blocked.join("keep").exists());
        assert!(outside.join("keep").exists());
        assert!(!target.join("link").exists());
        assert!(!target.join("artifact.bin").exists());
        assert!(!target.join("sibling").exists());
        let CleanResult::Failed {
            freed_bytes, error, ..
        } = result
        else {
            panic!("Expected partial failure");
        };
        assert_eq!(freed_bytes, 1200);
        assert!(error.contains("blocked"));
    }

    #[test]
    fn test_native_partial_deletion_is_counted_after_fallback() {
        let (tmp, project) = create_test_project();
        fs::write(
            tmp.path().join("clean.sh"),
            "rm target/artifact.bin\nexit 1\n",
        )
        .unwrap();

        let result =
            CleanExecutor::new(CleanOptions::default()).clean(&project, Some("sh clean.sh"));

        assert!(!tmp.path().join("target").exists());
        assert!(matches!(
            result,
            CleanResult::Success {
                freed_bytes: 1000,
                ..
            }
        ));
    }

    #[test]
    fn test_native_failure_emits_no_terminal_logs_even_at_trace_level() {
        use std::sync::{Arc, Mutex};
        struct Writer(Arc<Mutex<Vec<u8>>>);
        impl io::Write for Writer {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let output = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || Writer(Arc::clone(&captured)))
            .finish();
        let (_tmp, project) = create_test_project();

        tracing::subscriber::with_default(subscriber, || {
            let result = CleanExecutor::new(CleanOptions::default()).clean(&project, Some("false"));
            assert!(matches!(result, CleanResult::Success { .. }));
        });

        assert!(output.lock().unwrap().is_empty());
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

    #[test]
    fn test_failed_native_clean_falls_back_to_deletion() {
        let (_tmp, project) = create_test_project();
        let result = CleanExecutor::new(CleanOptions::default()).clean(&project, Some("false"));

        assert!(!project.artifact_paths[0].exists());
        assert!(matches!(
            result,
            CleanResult::Success {
                freed_bytes: 1000,
                ..
            }
        ));
    }

    #[test]
    fn test_failed_fallback_preserves_native_error_and_artifact_path() {
        let (tmp, mut project) = create_test_project();
        let invalid = tmp.path().join("not-a-directory");
        fs::write(&invalid, "file").unwrap();
        fs::write(
            tmp.path().join("clean.sh"),
            "echo 'failed to remove directory target/blocked: Permission denied' >&2\nexit 1\n",
        )
        .unwrap();
        project.artifact_paths = vec![invalid.clone()];

        let result =
            CleanExecutor::new(CleanOptions::default()).clean(&project, Some("sh clean.sh"));

        let CleanResult::Failed { error, .. } = result else {
            panic!("Expected failure");
        };
        assert!(error.contains(&invalid.display().to_string()));
        assert!(error.contains("target/blocked: Permission denied"));
    }
}
