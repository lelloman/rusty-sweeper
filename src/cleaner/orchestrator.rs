//! Orchestrator for parallel cleaning operations.

use crate::cleaner::detector::DetectedProject;
use crate::cleaner::executor::{CleanExecutor, CleanOptions, CleanResult};
use crate::cleaner::registry::DetectorRegistry;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Progress tracker for cleaning operations.
pub struct CleanProgress {
    /// Total number of projects to clean.
    pub total: usize,
    /// Number of completed projects.
    completed: AtomicUsize,
    /// Current project being cleaned.
    current_project: std::sync::Mutex<Option<String>>,
}

impl CleanProgress {
    /// Create a new progress tracker.
    pub fn new(total: usize) -> Self {
        Self {
            total,
            completed: AtomicUsize::new(0),
            current_project: std::sync::Mutex::new(None),
        }
    }

    /// Increment the completed count.
    pub fn increment(&self) {
        self.completed.fetch_add(1, Ordering::SeqCst);
    }

    /// Set the current project being cleaned.
    pub fn set_current(&self, name: String) {
        *self.current_project.lock().unwrap() = Some(name);
    }

    /// Get the number of completed projects.
    pub fn completed(&self) -> usize {
        self.completed.load(Ordering::SeqCst)
    }

    /// Get the current project name.
    pub fn current(&self) -> Option<String> {
        self.current_project.lock().unwrap().clone()
    }
}

/// Summary of cleaning results.
#[derive(Debug, Default)]
pub struct CleanSummary {
    /// Number of successfully cleaned projects.
    pub success_count: usize,
    /// Number of failed cleanups.
    pub failed_count: usize,
    /// Number of skipped projects.
    pub skipped_count: usize,
    /// Total bytes freed.
    pub total_freed: u64,
}

/// Orchestrator for parallel cleaning of multiple projects.
pub struct CleanOrchestrator {
    registry: DetectorRegistry,
    executor: CleanExecutor,
    parallelism: usize,
}

impl CleanOrchestrator {
    /// Create a new orchestrator.
    pub fn new(registry: DetectorRegistry, options: CleanOptions, parallelism: usize) -> Self {
        Self {
            registry,
            executor: CleanExecutor::new(options),
            parallelism,
        }
    }

    /// Clean multiple projects in parallel.
    pub fn clean_all(
        &self,
        projects: Vec<DetectedProject>,
        progress: Option<Arc<CleanProgress>>,
    ) -> Vec<CleanResult> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.parallelism)
            .build()
            .unwrap();

        pool.install(|| {
            projects
                .into_par_iter()
                .map(|project| {
                    if let Some(ref prog) = progress {
                        prog.set_current(project.path.display().to_string());
                    }

                    let clean_cmd = self
                        .registry
                        .get(&project.project_type)
                        .and_then(|d| d.clean_command());

                    let result = self.executor.clean(&project, clean_cmd);

                    if let Some(ref prog) = progress {
                        prog.increment();
                    }

                    result
                })
                .collect()
        })
    }

    /// Get summary statistics from results.
    pub fn summarize(results: &[CleanResult]) -> CleanSummary {
        let mut summary = CleanSummary::default();

        for result in results {
            match result {
                CleanResult::Success { freed_bytes, .. } => {
                    summary.success_count += 1;
                    summary.total_freed += freed_bytes;
                }
                CleanResult::Failed { .. } => {
                    summary.failed_count += 1;
                }
                CleanResult::Skipped { .. } => {
                    summary.skipped_count += 1;
                }
            }
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_projects(count: usize) -> (TempDir, Vec<DetectedProject>) {
        let tmp = TempDir::new().unwrap();
        let mut projects = Vec::new();

        for i in 0..count {
            let proj_dir = tmp.path().join(format!("project-{}", i));
            fs::create_dir(&proj_dir).unwrap();

            let target = proj_dir.join("target");
            fs::create_dir(&target).unwrap();
            fs::write(target.join("artifact"), "x".repeat(100)).unwrap();

            projects.push(DetectedProject {
                path: proj_dir,
                project_type: "cargo".to_string(),
                display_name: "Rust/Cargo".to_string(),
                artifact_size: 100,
                artifact_paths: vec![target],
            });
        }

        (tmp, projects)
    }

    #[test]
    fn test_clean_all_parallel() {
        let (_tmp, projects) = create_test_projects(10);

        let registry = DetectorRegistry::new();
        let options = CleanOptions {
            dry_run: false,
            use_native_commands: false,
        };
        let orchestrator = CleanOrchestrator::new(registry, options, 4);

        let results = orchestrator.clean_all(projects, None);

        assert_eq!(results.len(), 10);
        let successes = results
            .iter()
            .filter(|r| matches!(r, CleanResult::Success { .. }))
            .count();
        assert_eq!(successes, 10);
    }

    #[test]
    fn test_clean_all_with_progress() {
        let (_tmp, projects) = create_test_projects(5);

        let registry = DetectorRegistry::new();
        let options = CleanOptions {
            dry_run: true,
            use_native_commands: false,
        };
        let orchestrator = CleanOrchestrator::new(registry, options, 2);

        let progress = Arc::new(CleanProgress::new(5));
        let results = orchestrator.clean_all(projects, Some(Arc::clone(&progress)));

        assert_eq!(results.len(), 5);
        assert_eq!(progress.completed(), 5);
    }

    #[test]
    fn test_summarize() {
        let results = vec![
            CleanResult::Success {
                project: DetectedProject {
                    path: PathBuf::from("/a"),
                    project_type: "t".into(),
                    display_name: "T".into(),
                    artifact_size: 100,
                    artifact_paths: vec![],
                },
                freed_bytes: 100,
            },
            CleanResult::Success {
                project: DetectedProject {
                    path: PathBuf::from("/b"),
                    project_type: "t".into(),
                    display_name: "T".into(),
                    artifact_size: 200,
                    artifact_paths: vec![],
                },
                freed_bytes: 200,
            },
            CleanResult::Failed {
                project: DetectedProject {
                    path: PathBuf::from("/c"),
                    project_type: "t".into(),
                    display_name: "T".into(),
                    artifact_size: 0,
                    artifact_paths: vec![],
                },
                error: "oops".into(),
            },
            CleanResult::Skipped {
                project: DetectedProject {
                    path: PathBuf::from("/d"),
                    project_type: "t".into(),
                    display_name: "T".into(),
                    artifact_size: 0,
                    artifact_paths: vec![],
                },
                reason: "skipped".into(),
            },
        ];

        let summary = CleanOrchestrator::summarize(&results);

        assert_eq!(summary.success_count, 2);
        assert_eq!(summary.failed_count, 1);
        assert_eq!(summary.skipped_count, 1);
        assert_eq!(summary.total_freed, 300);
    }

    #[test]
    fn test_progress_tracker() {
        let progress = CleanProgress::new(10);

        assert_eq!(progress.total, 10);
        assert_eq!(progress.completed(), 0);
        assert!(progress.current().is_none());

        progress.increment();
        assert_eq!(progress.completed(), 1);

        progress.set_current("test".to_string());
        assert_eq!(progress.current(), Some("test".to_string()));
    }

    #[test]
    fn test_empty_projects_list() {
        let registry = DetectorRegistry::new();
        let options = CleanOptions::default();
        let orchestrator = CleanOrchestrator::new(registry, options, 4);

        let results = orchestrator.clean_all(vec![], None);
        assert!(results.is_empty());
    }

    #[test]
    fn test_single_thread() {
        let (_tmp, projects) = create_test_projects(3);

        let registry = DetectorRegistry::new();
        let options = CleanOptions {
            dry_run: true,
            use_native_commands: false,
        };
        let orchestrator = CleanOrchestrator::new(registry, options, 1);

        let results = orchestrator.clean_all(projects, None);
        assert_eq!(results.len(), 3);
    }
}
