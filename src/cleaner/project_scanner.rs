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
        self.scan_internal(root, true)
    }

    /// Scan for projects without calculating artifact sizes.
    ///
    /// This is intended for interactive recognition. Artifact sizes can be
    /// expensive to calculate for large `target`/`node_modules` directories
    /// and are not needed to determine whether a project exists.
    pub fn scan_fast(&self, root: &Path) -> Vec<DetectedProject> {
        self.scan_internal(root, false)
    }

    fn scan_internal(&self, root: &Path, calculate_sizes: bool) -> Vec<DetectedProject> {
        let mut projects = Vec::new();
        let mut artifact_dirs: Vec<PathBuf> = Vec::new();
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

        let mut walker = WalkDir::new(&root)
            .max_depth(self.options.max_depth)
            .follow_links(self.options.follow_symlinks)
            .into_iter();

        while let Some(result) = walker.next() {
            let Ok(entry) = result else {
                continue;
            };
            if !entry.file_type().is_dir() {
                continue;
            }

            let path = entry.path();

            // Artifact contents cannot contain a project that belongs to the
            // source tree, and may contain millions of files. Prune them once
            // their owning project has been recognized.
            if artifact_dirs
                .iter()
                .any(|artifact| path.starts_with(artifact))
            {
                walker.skip_current_dir();
                continue;
            }

            // Check exclusion patterns
            if !self.should_visit_path(path, &root) {
                continue;
            }

            // Check if this directory is a project
            if let Some(mut project) = self.detect_project(path) {
                if calculate_sizes {
                    project.artifact_size = self.calculate_artifact_size(&project.artifact_paths);
                }
                artifact_dirs.extend(project.artifact_paths.iter().cloned());
                projects.push(project);
            }
        }

        projects
    }

    fn should_visit_path(&self, path: &Path, root: &Path) -> bool {
        let relative = path.strip_prefix(root).unwrap_or(path);
        let relative_text = relative.to_string_lossy();
        if self.options.exclude_patterns.iter().any(|pattern| {
            let pattern = pattern.trim_start_matches("./");
            if let Some(component_pattern) = pattern.strip_prefix("**/") {
                relative.components().any(|component| {
                    component
                        .as_os_str()
                        .to_str()
                        .is_some_and(|name| Self::wildcard_matches(component_pattern, name))
                })
            } else if pattern.contains('/') {
                Self::wildcard_matches(pattern, &relative_text)
            } else {
                relative.components().any(|component| {
                    component
                        .as_os_str()
                        .to_str()
                        .is_some_and(|name| Self::wildcard_matches(pattern, name))
                })
            }
        }) {
            return false;
        }

        true
    }

    fn wildcard_matches(pattern: &str, value: &str) -> bool {
        let pattern: Vec<char> = pattern.chars().collect();
        let value: Vec<char> = value.chars().collect();
        let mut previous = vec![false; value.len() + 1];
        previous[0] = true;

        for token in pattern {
            let mut current = vec![false; value.len() + 1];
            if token == '*' {
                current[0] = previous[0];
            }
            for (index, character) in value.iter().enumerate() {
                current[index + 1] = match token {
                    '*' => previous[index + 1] || current[index],
                    '?' => previous[index],
                    literal => previous[index] && literal == *character,
                };
            }
            previous = current;
        }
        previous[value.len()]
    }

    fn detect_project(&self, path: &Path) -> Option<DetectedProject> {
        for detector in self.registry.detectors() {
            if detector.detect(path) {
                let artifact_paths = detector.find_artifacts(path);

                // Keep recognized local projects even when currently clean so
                // callers can distinguish them from ordinary directories.
                let has_local_artifacts = !artifact_paths.is_empty();
                let has_artifact_dirs = !detector.artifact_dirs().is_empty();
                let has_clean_command = detector.clean_command().is_some();

                // Skip command-only detectors (like Go) unless explicitly enabled
                if !has_local_artifacts && !has_artifact_dirs && has_clean_command {
                    // Go projects use global cache - skip for now
                    continue;
                }

                return Some(DetectedProject {
                    path: path.to_path_buf(),
                    project_type: detector.id().to_string(),
                    display_name: detector.display_name().to_string(),
                    artifact_size: 0,
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
    fn test_scan_reports_recognized_projects_without_artifacts() {
        let tmp = TempDir::new().unwrap();

        // Cargo project without target/
        let proj = tmp.path().join("clean-project");
        fs::create_dir_all(&proj).unwrap();
        fs::write(proj.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(proj.join("src")).unwrap();

        let registry = DetectorRegistry::new();
        let scanner = ProjectScanner::new(registry, ScanOptions::default());

        let projects = scanner.scan(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project_type, "cargo");
        assert_eq!(projects[0].artifact_size, 0);
        assert!(projects[0].artifact_paths.is_empty());
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

        // Both a parent project and a nested project must be discovered.
        let parent = tmp.path().join("projects");
        fs::create_dir_all(&parent).unwrap();
        fs::write(parent.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(parent.join("target")).unwrap();

        let nested = tmp.path().join("projects/libs/util");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(nested.join("target")).unwrap();
        fs::write(nested.join("target/libutil.so"), "x".repeat(100)).unwrap();

        let registry = DetectorRegistry::new();
        let scanner = ProjectScanner::new(registry, ScanOptions::default());

        let projects = scanner.scan(tmp.path());

        assert_eq!(projects.len(), 2);
        assert!(projects.iter().any(|project| project.path == parent));
        assert!(projects.iter().any(|project| project.path == nested));
    }

    #[test]
    fn test_scan_fast_recognizes_project_without_artifact_size_walk() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        fs::create_dir_all(project.join("target/debug")).unwrap();
        fs::write(project.join("Cargo.toml"), "[package]").unwrap();
        fs::write(project.join("target/debug/artifact"), "x").unwrap();

        let scanner =
            ProjectScanner::new(DetectorRegistry::with_types(&["cargo"]), Default::default());
        let projects = scanner.scan_fast(tmp.path());

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, project);
        assert_eq!(projects[0].artifact_size, 0);
        assert_eq!(projects[0].artifact_paths.len(), 1);
    }

    #[test]
    fn test_scan_normalizes_relative_root_paths() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("workspace");
        let project = root.join("project");
        fs::create_dir_all(project.join("target")).unwrap();
        fs::create_dir(root.join("alias")).unwrap();
        fs::write(project.join("Cargo.toml"), "[package]").unwrap();

        let relative_root = root.join("alias/../");
        let scanner =
            ProjectScanner::new(DetectorRegistry::with_types(&["cargo"]), Default::default());
        let projects = scanner.scan_fast(&relative_root);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, project.canonicalize().unwrap());
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
