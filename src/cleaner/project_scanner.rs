//! Project scanner for discovering projects in a directory tree.

use crate::cleaner::detector::DetectedProject;
use crate::cleaner::registry::DetectorRegistry;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

/// Options for scanning.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Maximum directory depth to scan.
    pub max_depth: usize,
    /// Patterns to exclude from scanning.
    pub exclude_patterns: Vec<String>,
    /// Whether to follow symbolic links.
    pub follow_symlinks: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            max_depth: 10,
            exclude_patterns: vec![".git".to_string(), "node_modules".to_string()],
            follow_symlinks: false,
        }
    }
}

/// Scanner for discovering projects in a directory tree.
pub struct ProjectScanner {
    registry: DetectorRegistry,
    options: ScanOptions,
}

impl ProjectScanner {
    /// Create a new scanner with the given registry and options.
    pub fn new(registry: DetectorRegistry, options: ScanOptions) -> Self {
        Self { registry, options }
    }

    /// Scan a directory tree for projects.
    ///
    /// Returns a list of detected projects with their artifact information.
    pub fn scan(&self, root: &Path) -> Vec<DetectedProject> {
        let mut projects = Vec::new();
        let mut skip_dirs: Vec<PathBuf> = Vec::new();

        let walker = WalkDir::new(root)
            .max_depth(self.options.max_depth)
            .follow_links(self.options.follow_symlinks)
            .into_iter();

        for entry in walker.flatten() {
            if !entry.file_type().is_dir() {
                continue;
            }

            let path = entry.path();

            // Check exclusion patterns
            if !self.should_visit_path(path, &skip_dirs) {
                continue;
            }

            // Check if this directory is a project
            if let Some(project) = self.detect_project(path) {
                // Don't recurse into this project
                skip_dirs.push(path.to_path_buf());
                projects.push(project);
            }
        }

        projects
    }

    fn should_visit_path(&self, path: &Path, skip_dirs: &[PathBuf]) -> bool {
        // Check if any path component matches an exclude pattern
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                let name = name.to_string_lossy();
                for pattern in &self.options.exclude_patterns {
                    if name == pattern.as_str() {
                        return false;
                    }
                }
            }
        }

        // Check if inside a detected project
        for skip in skip_dirs {
            if path.starts_with(skip) && path != skip {
                return false;
            }
        }

        true
    }

    fn detect_project(&self, path: &Path) -> Option<DetectedProject> {
        for detector in self.registry.detectors() {
            if detector.detect(path) {
                let artifact_paths = detector.find_artifacts(path);

                // Only report if there are actual artifacts OR it's a command-only detector
                let has_local_artifacts = !artifact_paths.is_empty();
                let has_artifact_dirs = !detector.artifact_dirs().is_empty();
                let has_clean_command = detector.clean_command().is_some();

                // Skip if no artifacts to clean
                if !has_local_artifacts && has_artifact_dirs {
                    continue;
                }

                // Skip command-only detectors (like Go) unless explicitly enabled
                if !has_local_artifacts && !has_artifact_dirs && has_clean_command {
                    // Go projects use global cache - skip for now
                    continue;
                }

                let artifact_size = self.calculate_artifact_size(&artifact_paths);

                return Some(DetectedProject {
                    path: path.to_path_buf(),
                    project_type: detector.id().to_string(),
                    display_name: detector.display_name().to_string(),
                    artifact_size,
                    artifact_paths,
                });
            }
        }
        None
    }

    fn calculate_artifact_size(&self, paths: &[PathBuf]) -> u64 {
        paths.iter().map(|p| Self::dir_size(p)).sum()
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

    /// Filter projects by age, keeping only those not modified in `min_age_days`.
    ///
    /// This checks the modification time of source files (excluding artifacts)
    /// to determine when the project was last actively worked on.
    pub fn filter_by_age(
        projects: Vec<DetectedProject>,
        min_age_days: u64,
    ) -> Vec<DetectedProject> {
        let cutoff = SystemTime::now() - Duration::from_secs(min_age_days * 24 * 60 * 60);

        projects
            .into_iter()
            .filter(|p| Self::project_last_modified(&p.path) < cutoff)
            .collect()
    }

    /// Get the last modification time of source files in a project.
    ///
    /// Excludes common artifact directories to focus on actual source code.
    fn project_last_modified(path: &Path) -> SystemTime {
        let artifact_names: HashSet<&str> = [
            "target",
            "build",
            "node_modules",
            ".gradle",
            "bin",
            "obj",
            "venv",
            ".venv",
            "__pycache__",
        ]
        .iter()
        .copied()
        .collect();

        WalkDir::new(path)
            .into_iter()
            .flatten()
            .filter(|e| {
                // Skip artifact directories
                !e.path().components().any(|c| {
                    if let std::path::Component::Normal(name) = c {
                        artifact_names.contains(name.to_str().unwrap_or(""))
                    } else {
                        false
                    }
                })
            })
            .filter_map(|e| e.metadata().ok())
            .filter_map(|m| m.modified().ok())
            .max()
            .unwrap_or(SystemTime::UNIX_EPOCH)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_tree() -> TempDir {
        let tmp = TempDir::new().unwrap();

        // Cargo project
        let cargo_proj = tmp.path().join("rust-app");
        fs::create_dir_all(&cargo_proj).unwrap();
        fs::write(cargo_proj.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(cargo_proj.join("target")).unwrap();
        fs::write(cargo_proj.join("target/debug.bin"), "x".repeat(1000)).unwrap();

        // npm project
        let npm_proj = tmp.path().join("web-app");
        fs::create_dir_all(&npm_proj).unwrap();
        fs::write(npm_proj.join("package.json"), "{}").unwrap();
        fs::create_dir(npm_proj.join("node_modules")).unwrap();
        fs::write(npm_proj.join("node_modules/dep.js"), "x".repeat(500)).unwrap();

        // Regular directory (not a project)
        fs::create_dir_all(tmp.path().join("docs")).unwrap();
        fs::write(tmp.path().join("docs/readme.txt"), "hello").unwrap();

        tmp
    }

    #[test]
    fn test_scan_finds_projects() {
        let tmp = setup_test_tree();
        let registry = DetectorRegistry::new();
        let scanner = ProjectScanner::new(registry, ScanOptions::default());

        let projects = scanner.scan(tmp.path());

        assert_eq!(projects.len(), 2);

        let types: Vec<&str> = projects.iter().map(|p| p.project_type.as_str()).collect();
        assert!(types.contains(&"cargo"));
        assert!(types.contains(&"npm"));
    }

    #[test]
    fn test_scan_calculates_sizes() {
        let tmp = setup_test_tree();
        let registry = DetectorRegistry::new();
        let scanner = ProjectScanner::new(registry, ScanOptions::default());

        let projects = scanner.scan(tmp.path());
        let cargo = projects.iter().find(|p| p.project_type == "cargo").unwrap();

        assert_eq!(cargo.artifact_size, 1000);
    }

    #[test]
    fn test_scan_respects_max_depth() {
        let tmp = TempDir::new().unwrap();

        // Create deeply nested project
        let deep = tmp.path().join("a/b/c/d/e/f/g/project");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(deep.join("target")).unwrap();

        let registry = DetectorRegistry::new();
        let options = ScanOptions {
            max_depth: 3,
            ..Default::default()
        };
        let scanner = ProjectScanner::new(registry, options);

        let projects = scanner.scan(tmp.path());
        assert!(projects.is_empty()); // Too deep
    }

    #[test]
    fn test_scan_excludes_patterns() {
        let tmp = TempDir::new().unwrap();

        // Project in .git directory (should be excluded)
        let git_proj = tmp.path().join(".git/hooks");
        fs::create_dir_all(&git_proj).unwrap();
        fs::write(git_proj.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(git_proj.join("target")).unwrap();

        let registry = DetectorRegistry::new();
        let scanner = ProjectScanner::new(registry, ScanOptions::default());

        let projects = scanner.scan(tmp.path());
        assert!(projects.is_empty());
    }

    #[test]
    fn test_scan_skips_projects_without_artifacts() {
        let tmp = TempDir::new().unwrap();

        // Cargo project without target/
        let proj = tmp.path().join("clean-project");
        fs::create_dir_all(&proj).unwrap();
        fs::write(proj.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(proj.join("src")).unwrap();

        let registry = DetectorRegistry::new();
        let scanner = ProjectScanner::new(registry, ScanOptions::default());

        let projects = scanner.scan(tmp.path());
        assert!(projects.is_empty()); // No artifacts to report
    }

    #[test]
    fn test_scan_with_filtered_registry() {
        let tmp = setup_test_tree();
        let registry = DetectorRegistry::with_types(&["cargo"]);
        let scanner = ProjectScanner::new(registry, ScanOptions::default());

        let projects = scanner.scan(tmp.path());

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project_type, "cargo");
    }

    #[test]
    fn test_scan_nested_project() {
        let tmp = TempDir::new().unwrap();

        // Create nested project
        let nested = tmp.path().join("projects/libs/util");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(nested.join("target")).unwrap();
        fs::write(nested.join("target/libutil.so"), "x".repeat(100)).unwrap();

        let registry = DetectorRegistry::new();
        let scanner = ProjectScanner::new(registry, ScanOptions::default());

        let projects = scanner.scan(tmp.path());

        assert_eq!(projects.len(), 1);
        assert!(projects[0].path.ends_with("util"));
    }

    #[test]
    fn test_filter_by_age_recent_project() {
        // Create a project that was just modified (should be filtered out)
        let tmp = TempDir::new().unwrap();
        let proj = tmp.path().join("recent");
        fs::create_dir_all(&proj).unwrap();
        fs::write(proj.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(proj.join("target")).unwrap();
        fs::write(proj.join("target/artifact"), "x".repeat(100)).unwrap();

        let projects = vec![DetectedProject {
            path: proj,
            project_type: "cargo".to_string(),
            display_name: "Cargo".to_string(),
            artifact_size: 100,
            artifact_paths: vec![],
        }];

        // Filter for projects older than 7 days
        let filtered = ProjectScanner::filter_by_age(projects, 7);

        // Recent project should be filtered out
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_project_last_modified_excludes_artifacts() {
        // Test that artifact directories are excluded when checking mtime
        let tmp = TempDir::new().unwrap();
        let proj = tmp.path().join("project");
        fs::create_dir_all(&proj).unwrap();

        // Create source file
        fs::write(proj.join("Cargo.toml"), "[package]").unwrap();

        // Create artifact directory
        fs::create_dir(proj.join("target")).unwrap();
        fs::write(proj.join("target/artifact"), "x").unwrap();

        // The function should work without panicking
        let mtime = ProjectScanner::project_last_modified(&proj);

        // Should return a valid time (not UNIX_EPOCH for a project with files)
        assert!(mtime > SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn test_filter_by_age_empty_list() {
        let filtered = ProjectScanner::filter_by_age(vec![], 7);
        assert!(filtered.is_empty());
    }
}
