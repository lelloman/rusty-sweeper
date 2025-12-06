# Phase 3: Project Detection & Cleaner - Implementation Plan

## Overview

Phase 3 implements the core cleaning functionality: detecting coding projects and executing appropriate clean commands to remove build artifacts.

**Prerequisites**: Phase 1 (foundation) and Phase 2 (scanner) must be complete.

**Status Legend**:
- `[ ]` Not started
- `[~]` In progress
- `[x]` Completed

---

## Task 1: ProjectDetector Trait Definition

**Status**: `[x]`

### Description

Define the core trait that all project detectors will implement. This establishes the contract for project detection and cleaning.

### Context

The trait needs to provide:
- Project type identification
- Detection logic (what files/patterns indicate this project type)
- List of artifact directories to clean
- The clean command to execute
- Optional: whether to use native command or direct deletion

### Implementation

**File**: `src/cleaner/detector.rs`

```rust
use std::path::Path;

/// Represents a detected project with its metadata
#[derive(Debug, Clone)]
pub struct DetectedProject {
    /// Root path of the project
    pub path: std::path::PathBuf,
    /// Type identifier (e.g., "cargo", "gradle")
    pub project_type: String,
    /// Human-readable name
    pub display_name: String,
    /// Total size of artifact directories in bytes
    pub artifact_size: u64,
    /// List of artifact directories found
    pub artifact_paths: Vec<std::path::PathBuf>,
}

/// Trait for project type detectors
pub trait ProjectDetector: Send + Sync {
    /// Unique identifier for this project type (e.g., "cargo")
    fn id(&self) -> &'static str;

    /// Human-readable name (e.g., "Rust/Cargo")
    fn display_name(&self) -> &'static str;

    /// Files that indicate this project type exists
    /// Returns true if ANY of these files exist
    fn detection_files(&self) -> &'static [&'static str];

    /// Directories containing build artifacts
    fn artifact_dirs(&self) -> &'static [&'static str];

    /// Command to clean the project (None = direct deletion)
    fn clean_command(&self) -> Option<&'static str>;

    /// Check if this project type exists at the given path
    fn detect(&self, path: &Path) -> bool {
        self.detection_files()
            .iter()
            .any(|f| path.join(f).exists())
    }

    /// Get existing artifact directories at the given path
    fn find_artifacts(&self, path: &Path) -> Vec<std::path::PathBuf> {
        self.artifact_dirs()
            .iter()
            .map(|d| path.join(d))
            .filter(|p| p.exists())
            .collect()
    }
}
```

### Tests

**File**: `src/cleaner/detector_tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Mock detector for testing
    struct MockDetector;

    impl ProjectDetector for MockDetector {
        fn id(&self) -> &'static str { "mock" }
        fn display_name(&self) -> &'static str { "Mock Project" }
        fn detection_files(&self) -> &'static [&'static str] { &["mock.toml"] }
        fn artifact_dirs(&self) -> &'static [&'static str] { &["build"] }
        fn clean_command(&self) -> Option<&'static str> { Some("mock clean") }
    }

    #[test]
    fn test_detected_project_creation() {
        let project = DetectedProject {
            path: PathBuf::from("/test"),
            project_type: "mock".to_string(),
            display_name: "Mock".to_string(),
            artifact_size: 1024,
            artifact_paths: vec![PathBuf::from("/test/build")],
        };
        assert_eq!(project.project_type, "mock");
        assert_eq!(project.artifact_size, 1024);
    }

    #[test]
    fn test_detector_trait_defaults() {
        let detector = MockDetector;
        assert_eq!(detector.id(), "mock");
        assert_eq!(detector.detection_files(), &["mock.toml"]);
    }
}
```

### Acceptance Criteria

- [ ] `ProjectDetector` trait compiles
- [ ] `DetectedProject` struct holds all necessary metadata
- [ ] Default `detect()` implementation works
- [ ] Default `find_artifacts()` implementation works
- [ ] Unit tests pass

---

## Task 2: Cargo Detector Implementation

**Status**: `[x]`

### Description

Implement the first concrete detector for Rust/Cargo projects. This serves as the reference implementation for other detectors.

### Context

Cargo projects are identified by `Cargo.toml` and store build artifacts in `target/`. The `cargo clean` command is the preferred cleanup method.

### Implementation

**File**: `src/cleaner/detectors/cargo.rs`

```rust
use crate::cleaner::detector::ProjectDetector;

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
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

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
        // Empty directory

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
}
```

### Acceptance Criteria

- [ ] CargoDetector implements ProjectDetector
- [ ] Correctly identifies Cargo projects
- [ ] Finds target/ directory when present
- [ ] All tests pass

---

## Task 3: Remaining Detector Implementations

**Status**: `[x]`

### Description

Implement detectors for all remaining project types: Gradle, Maven, npm, Go, CMake, Python, Bazel, and .NET.

### Context

Each detector follows the same pattern as CargoDetector but with project-specific detection files and artifact directories.

### Implementation

**File**: `src/cleaner/detectors/mod.rs`

```rust
mod cargo;
mod gradle;
mod maven;
mod npm;
mod go;
mod cmake;
mod python;
mod bazel;
mod dotnet;

pub use cargo::CargoDetector;
pub use gradle::GradleDetector;
pub use maven::MavenDetector;
pub use npm::NpmDetector;
pub use go::GoDetector;
pub use cmake::CMakeDetector;
pub use python::PythonDetector;
pub use bazel::BazelDetector;
pub use dotnet::DotnetDetector;

use crate::cleaner::detector::ProjectDetector;

/// Returns all built-in detectors
pub fn all_detectors() -> Vec<Box<dyn ProjectDetector>> {
    vec![
        Box::new(CargoDetector),
        Box::new(GradleDetector),
        Box::new(MavenDetector),
        Box::new(NpmDetector),
        Box::new(GoDetector),
        Box::new(CMakeDetector),
        Box::new(PythonDetector),
        Box::new(BazelDetector),
        Box::new(DotnetDetector),
    ]
}
```

**File**: `src/cleaner/detectors/gradle.rs`

```rust
use crate::cleaner::detector::ProjectDetector;

pub struct GradleDetector;

impl ProjectDetector for GradleDetector {
    fn id(&self) -> &'static str { "gradle" }
    fn display_name(&self) -> &'static str { "Gradle/Android" }

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
```

**File**: `src/cleaner/detectors/maven.rs`

```rust
pub struct MavenDetector;

impl ProjectDetector for MavenDetector {
    fn id(&self) -> &'static str { "maven" }
    fn display_name(&self) -> &'static str { "Maven" }
    fn detection_files(&self) -> &'static [&'static str] { &["pom.xml"] }
    fn artifact_dirs(&self) -> &'static [&'static str] { &["target"] }
    fn clean_command(&self) -> Option<&'static str> { Some("mvn clean") }
}
```

**File**: `src/cleaner/detectors/npm.rs`

```rust
pub struct NpmDetector;

impl ProjectDetector for NpmDetector {
    fn id(&self) -> &'static str { "npm" }
    fn display_name(&self) -> &'static str { "npm/Node.js" }
    fn detection_files(&self) -> &'static [&'static str] { &["package.json"] }
    fn artifact_dirs(&self) -> &'static [&'static str] { &["node_modules"] }
    fn clean_command(&self) -> Option<&'static str> { None } // Direct deletion
}
```

**File**: `src/cleaner/detectors/go.rs`

```rust
pub struct GoDetector;

impl ProjectDetector for GoDetector {
    fn id(&self) -> &'static str { "go" }
    fn display_name(&self) -> &'static str { "Go" }
    fn detection_files(&self) -> &'static [&'static str] { &["go.mod"] }
    fn artifact_dirs(&self) -> &'static [&'static str] { &[] } // Uses go clean -cache
    fn clean_command(&self) -> Option<&'static str> { Some("go clean -cache") }
}
```

**File**: `src/cleaner/detectors/cmake.rs`

```rust
use crate::cleaner::detector::ProjectDetector;
use std::path::Path;

pub struct CMakeDetector;

impl ProjectDetector for CMakeDetector {
    fn id(&self) -> &'static str { "cmake" }
    fn display_name(&self) -> &'static str { "CMake" }
    fn detection_files(&self) -> &'static [&'static str] { &["CMakeLists.txt"] }
    fn artifact_dirs(&self) -> &'static [&'static str] { &["build"] }
    fn clean_command(&self) -> Option<&'static str> { None } // Direct deletion

    // Override: only detect if CMakeLists.txt AND build/ exist
    fn detect(&self, path: &Path) -> bool {
        path.join("CMakeLists.txt").exists() && path.join("build").exists()
    }
}
```

**File**: `src/cleaner/detectors/python.rs`

```rust
pub struct PythonDetector;

impl ProjectDetector for PythonDetector {
    fn id(&self) -> &'static str { "python" }
    fn display_name(&self) -> &'static str { "Python venv" }
    fn detection_files(&self) -> &'static [&'static str] { &["venv", ".venv"] }
    fn artifact_dirs(&self) -> &'static [&'static str] { &["venv", ".venv", "__pycache__"] }
    fn clean_command(&self) -> Option<&'static str> { None } // Direct deletion
}
```

**File**: `src/cleaner/detectors/bazel.rs`

```rust
pub struct BazelDetector;

impl ProjectDetector for BazelDetector {
    fn id(&self) -> &'static str { "bazel" }
    fn display_name(&self) -> &'static str { "Bazel" }
    fn detection_files(&self) -> &'static [&'static str] { &["WORKSPACE", "WORKSPACE.bazel"] }
    fn artifact_dirs(&self) -> &'static [&'static str] { &[] } // bazel clean handles it
    fn clean_command(&self) -> Option<&'static str> { Some("bazel clean --expunge") }
}
```

**File**: `src/cleaner/detectors/dotnet.rs`

```rust
use crate::cleaner::detector::ProjectDetector;
use std::path::Path;

pub struct DotnetDetector;

impl ProjectDetector for DotnetDetector {
    fn id(&self) -> &'static str { "dotnet" }
    fn display_name(&self) -> &'static str { ".NET" }
    fn detection_files(&self) -> &'static [&'static str] { &[] } // Uses custom detection
    fn artifact_dirs(&self) -> &'static [&'static str] { &["bin", "obj"] }
    fn clean_command(&self) -> Option<&'static str> { Some("dotnet clean") }

    fn detect(&self, path: &Path) -> bool {
        // Look for *.csproj or *.sln files
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
```

### Tests

Each detector needs similar tests to CargoDetector. Create a test helper:

**File**: `src/cleaner/detectors/test_helpers.rs`

```rust
#[cfg(test)]
pub mod helpers {
    use tempfile::TempDir;
    use std::fs;
    use std::path::Path;

    pub fn create_project_structure(
        files: &[&str],
        dirs: &[&str],
    ) -> TempDir {
        let tmp = TempDir::new().unwrap();

        for file in files {
            let path = tmp.path().join(file);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, "").unwrap();
        }

        for dir in dirs {
            fs::create_dir_all(tmp.path().join(dir)).unwrap();
        }

        tmp
    }
}
```

### Acceptance Criteria

- [ ] All 9 detectors implemented
- [ ] `all_detectors()` returns all detectors
- [ ] Each detector has unit tests
- [ ] Custom detection logic works (CMake, .NET)

---

## Task 4: Detector Registry

**Status**: `[x]`

### Description

Create a registry that manages detectors and supports filtering by type, enabling/disabling from config.

### Context

Users may want to:
- Only scan for specific project types (`--types cargo,npm`)
- Disable certain detectors via config
- Add custom detectors (future)

### Implementation

**File**: `src/cleaner/registry.rs`

```rust
use crate::cleaner::detector::ProjectDetector;
use crate::cleaner::detectors::all_detectors;
use std::collections::HashSet;

pub struct DetectorRegistry {
    detectors: Vec<Box<dyn ProjectDetector>>,
}

impl DetectorRegistry {
    /// Create registry with all built-in detectors
    pub fn new() -> Self {
        Self {
            detectors: all_detectors(),
        }
    }

    /// Create registry with only specified detector types
    pub fn with_types(types: &[&str]) -> Self {
        let type_set: HashSet<&str> = types.iter().copied().collect();
        Self {
            detectors: all_detectors()
                .into_iter()
                .filter(|d| type_set.contains(d.id()))
                .collect(),
        }
    }

    /// Create registry excluding specified types
    pub fn without_types(types: &[&str]) -> Self {
        let type_set: HashSet<&str> = types.iter().copied().collect();
        Self {
            detectors: all_detectors()
                .into_iter()
                .filter(|d| !type_set.contains(d.id()))
                .collect(),
        }
    }

    /// Get all registered detectors
    pub fn detectors(&self) -> &[Box<dyn ProjectDetector>] {
        &self.detectors
    }

    /// Get detector by ID
    pub fn get(&self, id: &str) -> Option<&dyn ProjectDetector> {
        self.detectors
            .iter()
            .find(|d| d.id() == id)
            .map(|d| d.as_ref())
    }

    /// List all detector IDs
    pub fn ids(&self) -> Vec<&str> {
        self.detectors.iter().map(|d| d.id()).collect()
    }
}

impl Default for DetectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new_has_all_detectors() {
        let registry = DetectorRegistry::new();
        let ids = registry.ids();

        assert!(ids.contains(&"cargo"));
        assert!(ids.contains(&"gradle"));
        assert!(ids.contains(&"npm"));
    }

    #[test]
    fn test_registry_with_types() {
        let registry = DetectorRegistry::with_types(&["cargo", "npm"]);
        let ids = registry.ids();

        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"cargo"));
        assert!(ids.contains(&"npm"));
        assert!(!ids.contains(&"gradle"));
    }

    #[test]
    fn test_registry_without_types() {
        let registry = DetectorRegistry::without_types(&["cargo"]);
        let ids = registry.ids();

        assert!(!ids.contains(&"cargo"));
        assert!(ids.contains(&"npm"));
    }

    #[test]
    fn test_registry_get() {
        let registry = DetectorRegistry::new();

        let cargo = registry.get("cargo");
        assert!(cargo.is_some());
        assert_eq!(cargo.unwrap().id(), "cargo");

        let unknown = registry.get("unknown");
        assert!(unknown.is_none());
    }
}
```

### Acceptance Criteria

- [ ] Registry holds all detectors by default
- [ ] Filtering by type works (`with_types`)
- [ ] Exclusion works (`without_types`)
- [ ] Individual detector lookup works
- [ ] All tests pass

---

## Task 5: Project Scanner

**Status**: `[x]`

### Description

Implement directory traversal that discovers projects using the detector registry.

### Context

The scanner walks a directory tree, checking each directory against all detectors. When a project is found, it doesn't recurse into it (to avoid finding nested projects like node_modules in a project).

### Implementation

**File**: `src/cleaner/scanner.rs`

```rust
use crate::cleaner::detector::{DetectedProject, ProjectDetector};
use crate::cleaner::registry::DetectorRegistry;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub max_depth: usize,
    pub exclude_patterns: Vec<String>,
    pub follow_symlinks: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            max_depth: 10,
            exclude_patterns: vec![
                ".git".to_string(),
                "node_modules".to_string(),
            ],
            follow_symlinks: false,
        }
    }
}

pub struct ProjectScanner {
    registry: DetectorRegistry,
    options: ScanOptions,
}

impl ProjectScanner {
    pub fn new(registry: DetectorRegistry, options: ScanOptions) -> Self {
        Self { registry, options }
    }

    /// Scan a directory tree for projects
    pub fn scan(&self, root: &Path) -> Vec<DetectedProject> {
        let mut projects = Vec::new();
        let mut skip_dirs: Vec<PathBuf> = Vec::new();

        let walker = WalkDir::new(root)
            .max_depth(self.options.max_depth)
            .follow_links(self.options.follow_symlinks)
            .into_iter()
            .filter_entry(|e| self.should_visit(e, &skip_dirs));

        for entry in walker.flatten() {
            if !entry.file_type().is_dir() {
                continue;
            }

            let path = entry.path();

            // Check if this directory is a project
            if let Some(project) = self.detect_project(path) {
                // Don't recurse into this project
                skip_dirs.push(path.to_path_buf());
                projects.push(project);
            }
        }

        projects
    }

    fn should_visit(&self, entry: &DirEntry, skip_dirs: &[PathBuf]) -> bool {
        let path = entry.path();

        // Check exclusion patterns
        if let Some(name) = path.file_name() {
            let name = name.to_string_lossy();
            for pattern in &self.options.exclude_patterns {
                if name == pattern.as_str() {
                    return false;
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

                // Only report if there are actual artifacts
                if artifact_paths.is_empty() && detector.artifact_dirs().is_empty() {
                    // Detector uses clean command without local artifacts (e.g., go)
                } else if artifact_paths.is_empty() {
                    continue; // No artifacts to clean
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
        paths
            .iter()
            .map(|p| Self::dir_size(p))
            .sum()
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
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

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

        let registry = DetectorRegistry::new();
        let scanner = ProjectScanner::new(registry, ScanOptions::default());

        let projects = scanner.scan(tmp.path());
        assert!(projects.is_empty());
    }
}
```

### Acceptance Criteria

- [ ] Scanner finds all project types
- [ ] Respects max_depth
- [ ] Respects exclusion patterns
- [ ] Calculates artifact sizes correctly
- [ ] Doesn't recurse into detected projects
- [ ] All tests pass

---

## Task 6: Clean Executor

**Status**: `[x]`

### Description

Execute clean operations on detected projects, either via native command or direct deletion.

### Context

Some projects use native clean commands (`cargo clean`), others need direct deletion (`rm -rf node_modules`). The executor handles both cases and reports results.

### Implementation

**File**: `src/cleaner/executor.rs`

```rust
use crate::cleaner::detector::DetectedProject;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub enum CleanResult {
    Success {
        project: DetectedProject,
        freed_bytes: u64,
    },
    Failed {
        project: DetectedProject,
        error: String,
    },
    Skipped {
        project: DetectedProject,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct CleanOptions {
    pub dry_run: bool,
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

pub struct CleanExecutor {
    options: CleanOptions,
}

impl CleanExecutor {
    pub fn new(options: CleanOptions) -> Self {
        Self { options }
    }

    /// Clean a single project
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
                match self.run_clean_command(&project.path, cmd) {
                    Ok(()) => {
                        return CleanResult::Success {
                            project: project.clone(),
                            freed_bytes: project.artifact_size,
                        };
                    }
                    Err(e) => {
                        // Fall back to direct deletion
                        tracing::warn!(
                            "Native clean command failed for {}: {}, falling back to direct deletion",
                            project.path.display(),
                            e
                        );
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
            Err(e) => CleanResult::Failed {
                project: project.clone(),
                error: e.to_string(),
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
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Command failed: {}", stderr),
            ))
        }
    }

    fn delete_artifacts(&self, project: &DetectedProject) -> io::Result<u64> {
        let mut freed = 0u64;

        for artifact_path in &project.artifact_paths {
            if artifact_path.exists() {
                let size = Self::dir_size(artifact_path);
                fs::remove_dir_all(artifact_path)?;
                freed += size;
            }
        }

        Ok(freed)
    }

    fn dir_size(path: &Path) -> u64 {
        walkdir::WalkDir::new(path)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum()
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use std::path::PathBuf;

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
        assert!(matches!(result, CleanResult::Success { freed_bytes: 0, .. }));
    }
}
```

### Acceptance Criteria

- [ ] Dry run doesn't delete anything
- [ ] Native commands execute correctly
- [ ] Fallback to direct deletion works
- [ ] Size freed is calculated correctly
- [ ] Handles missing artifacts gracefully
- [ ] All tests pass

---

## Task 7: Parallel Clean Orchestrator

**Status**: `[ ]`

### Description

Coordinate parallel cleaning of multiple projects with progress reporting.

### Context

When cleaning many projects, we want to parallelize for speed but also report progress and collect results.

### Implementation

**File**: `src/cleaner/orchestrator.rs`

```rust
use crate::cleaner::detector::DetectedProject;
use crate::cleaner::executor::{CleanExecutor, CleanOptions, CleanResult};
use crate::cleaner::registry::DetectorRegistry;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct CleanProgress {
    pub total: usize,
    pub completed: AtomicUsize,
    pub current_project: std::sync::Mutex<Option<String>>,
}

impl CleanProgress {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            completed: AtomicUsize::new(0),
            current_project: std::sync::Mutex::new(None),
        }
    }

    pub fn increment(&self) {
        self.completed.fetch_add(1, Ordering::SeqCst);
    }

    pub fn set_current(&self, name: String) {
        *self.current_project.lock().unwrap() = Some(name);
    }

    pub fn completed(&self) -> usize {
        self.completed.load(Ordering::SeqCst)
    }
}

pub struct CleanOrchestrator {
    registry: DetectorRegistry,
    executor: CleanExecutor,
    parallelism: usize,
}

impl CleanOrchestrator {
    pub fn new(registry: DetectorRegistry, options: CleanOptions, parallelism: usize) -> Self {
        Self {
            registry,
            executor: CleanExecutor::new(options),
            parallelism,
        }
    }

    /// Clean multiple projects in parallel
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

    /// Get summary statistics from results
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

#[derive(Debug, Default)]
pub struct CleanSummary {
    pub success_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub total_freed: u64,
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

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
                project_type: "test".to_string(),
                display_name: "Test".to_string(),
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
        let successes = results.iter().filter(|r| matches!(r, CleanResult::Success { .. })).count();
        assert_eq!(successes, 10);
    }

    #[test]
    fn test_summarize() {
        let results = vec![
            CleanResult::Success {
                project: DetectedProject {
                    path: "/a".into(),
                    project_type: "t".into(),
                    display_name: "T".into(),
                    artifact_size: 100,
                    artifact_paths: vec![],
                },
                freed_bytes: 100,
            },
            CleanResult::Success {
                project: DetectedProject {
                    path: "/b".into(),
                    project_type: "t".into(),
                    display_name: "T".into(),
                    artifact_size: 200,
                    artifact_paths: vec![],
                },
                freed_bytes: 200,
            },
            CleanResult::Failed {
                project: DetectedProject {
                    path: "/c".into(),
                    project_type: "t".into(),
                    display_name: "T".into(),
                    artifact_size: 0,
                    artifact_paths: vec![],
                },
                error: "oops".into(),
            },
        ];

        let summary = CleanOrchestrator::summarize(&results);

        assert_eq!(summary.success_count, 2);
        assert_eq!(summary.failed_count, 1);
        assert_eq!(summary.total_freed, 300);
    }
}
```

### Acceptance Criteria

- [ ] Parallel execution works
- [ ] Progress tracking works
- [ ] Summary statistics are correct
- [ ] Thread pool respects parallelism setting
- [ ] All tests pass

---

## Task 8: CLI Subcommand Implementation

**Status**: `[ ]`

### Description

Implement the `clean` CLI subcommand with all options.

### Context

This connects all the cleaner components to the CLI, handling argument parsing, user confirmation, and output formatting.

### Implementation

**File**: `src/cli/clean.rs`

```rust
use crate::cleaner::executor::CleanOptions;
use crate::cleaner::orchestrator::{CleanOrchestrator, CleanProgress};
use crate::cleaner::registry::DetectorRegistry;
use crate::cleaner::scanner::{ProjectScanner, ScanOptions};
use clap::Args;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Args, Debug)]
pub struct CleanArgs {
    /// Root directory to scan
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Show what would be cleaned without doing it
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Maximum recursion depth
    #[arg(short = 'd', long, default_value = "10")]
    pub max_depth: usize,

    /// Project types to clean (comma-separated)
    #[arg(short = 't', long, value_delimiter = ',')]
    pub types: Option<Vec<String>>,

    /// Paths to exclude (glob patterns)
    #[arg(short = 'e', long)]
    pub exclude: Vec<String>,

    /// Only clean projects not modified in N days
    #[arg(short = 'a', long)]
    pub age: Option<u64>,

    /// Skip confirmation prompts
    #[arg(short = 'f', long)]
    pub force: bool,

    /// Number of parallel clean jobs
    #[arg(short = 'j', long, default_value = "4")]
    pub jobs: usize,

    /// Only report sizes without cleaning
    #[arg(long)]
    pub size_only: bool,
}

pub fn run(args: CleanArgs) -> anyhow::Result<()> {
    // Set up registry
    let registry = if let Some(types) = &args.types {
        let types: Vec<&str> = types.iter().map(|s| s.as_str()).collect();
        DetectorRegistry::with_types(&types)
    } else {
        DetectorRegistry::new()
    };

    // Set up scanner
    let scan_options = ScanOptions {
        max_depth: args.max_depth,
        exclude_patterns: args.exclude.clone(),
        ..Default::default()
    };
    let scanner = ProjectScanner::new(registry.clone(), scan_options);

    // Scan for projects
    println!("Scanning for projects in {}...", args.path.display());
    let projects = scanner.scan(&args.path);

    if projects.is_empty() {
        println!("No projects with cleanable artifacts found.");
        return Ok(());
    }

    // Display found projects
    print_projects_table(&projects);

    let total_size: u64 = projects.iter().map(|p| p.artifact_size).sum();
    println!("\nTotal: {} in {} projects",
        humansize::format_size(total_size, humansize::BINARY),
        projects.len()
    );

    if args.size_only {
        return Ok(());
    }

    // Confirmation
    if !args.force && !args.dry_run {
        print!("\nProceed with cleanup? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Execute cleanup
    let clean_options = CleanOptions {
        dry_run: args.dry_run,
        use_native_commands: true,
    };
    let orchestrator = CleanOrchestrator::new(registry, clean_options, args.jobs);

    let progress = Arc::new(CleanProgress::new(projects.len()));

    if args.dry_run {
        println!("\n[DRY RUN] Would clean:");
    } else {
        println!("\nCleaning...");
    }

    let results = orchestrator.clean_all(projects, Some(progress));
    let summary = CleanOrchestrator::summarize(&results);

    // Print results
    println!("\nResults:");
    println!("  Cleaned: {} projects", summary.success_count);
    println!("  Failed:  {} projects", summary.failed_count);
    println!("  Freed:   {}", humansize::format_size(summary.total_freed, humansize::BINARY));

    // Print failures
    for result in &results {
        if let crate::cleaner::executor::CleanResult::Failed { project, error } = result {
            eprintln!("  Error cleaning {}: {}", project.path.display(), error);
        }
    }

    if summary.failed_count > 0 {
        std::process::exit(5); // Partial failure
    }

    Ok(())
}

fn print_projects_table(projects: &[crate::cleaner::detector::DetectedProject]) {
    println!("\n  {:<10} {:<50} {:>10}", "TYPE", "PATH", "SIZE");
    println!("  {}", "─".repeat(72));

    for project in projects {
        let path_str = project.path.display().to_string();
        let path_display = if path_str.len() > 48 {
            format!("...{}", &path_str[path_str.len()-45..])
        } else {
            path_str
        };

        println!(
            "  {:<10} {:<50} {:>10}",
            project.project_type,
            path_display,
            humansize::format_size(project.artifact_size, humansize::BINARY),
        );
    }
}
```

**Update**: `src/cli/mod.rs`

```rust
mod clean;

pub use clean::{CleanArgs, run as run_clean};
```

**Update**: `src/main.rs`

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rusty-sweeper")]
#[command(about = "Disk usage management utility")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Discover and clean build artifacts
    Clean(cli::CleanArgs),
    // ... other commands
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Clean(args) => cli::run_clean(args),
        // ... other commands
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use assert_cmd::Command;
    use tempfile::TempDir;
    use std::fs;

    fn setup_test_env() -> TempDir {
        let tmp = TempDir::new().unwrap();

        // Create a cargo project
        let proj = tmp.path().join("test-project");
        fs::create_dir(&proj).unwrap();
        fs::write(proj.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        let target = proj.join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("debug.bin"), "x".repeat(10000)).unwrap();

        tmp
    }

    #[test]
    fn test_clean_dry_run() {
        let tmp = setup_test_env();

        let mut cmd = Command::cargo_bin("rusty-sweeper").unwrap();
        cmd.arg("clean")
            .arg("--dry-run")
            .arg(tmp.path());

        cmd.assert().success();

        // Target should still exist
        assert!(tmp.path().join("test-project/target").exists());
    }

    #[test]
    fn test_clean_size_only() {
        let tmp = setup_test_env();

        let mut cmd = Command::cargo_bin("rusty-sweeper").unwrap();
        cmd.arg("clean")
            .arg("--size-only")
            .arg(tmp.path());

        cmd.assert()
            .success()
            .stdout(predicates::str::contains("cargo"));
    }

    #[test]
    fn test_clean_with_types_filter() {
        let tmp = setup_test_env();

        let mut cmd = Command::cargo_bin("rusty-sweeper").unwrap();
        cmd.arg("clean")
            .arg("--types")
            .arg("npm")  // Only npm, not cargo
            .arg("--size-only")
            .arg(tmp.path());

        cmd.assert()
            .success()
            .stdout(predicates::str::contains("No projects"));
    }
}
```

### Acceptance Criteria

- [ ] `--dry-run` shows but doesn't delete
- [ ] `--size-only` only reports sizes
- [ ] `--types` filters project types
- [ ] `--exclude` excludes paths
- [ ] `--force` skips confirmation
- [ ] `--jobs` controls parallelism
- [ ] Exit code 5 on partial failure
- [ ] All tests pass

---

## Task 9: Age Filtering

**Status**: `[ ]`

### Description

Add ability to filter projects by last modification time.

### Context

Users may want to only clean old projects that haven't been touched recently, keeping active projects untouched.

### Implementation

**File**: `src/cleaner/scanner.rs` (add to existing)

```rust
use std::time::{Duration, SystemTime};

impl ProjectScanner {
    /// Filter projects by age (only keep those older than min_age)
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

    fn project_last_modified(path: &std::path::Path) -> SystemTime {
        // Check the most recent mtime of any file in the project (excluding artifacts)
        let artifact_names: std::collections::HashSet<&str> =
            ["target", "build", "node_modules", ".gradle", "bin", "obj"]
                .iter()
                .copied()
                .collect();

        walkdir::WalkDir::new(path)
            .into_iter()
            .flatten()
            .filter(|e| {
                // Skip artifact directories
                !e.path()
                    .components()
                    .any(|c| artifact_names.contains(c.as_os_str().to_str().unwrap_or("")))
            })
            .filter_map(|e| e.metadata().ok())
            .filter_map(|m| m.modified().ok())
            .max()
            .unwrap_or(SystemTime::UNIX_EPOCH)
    }
}
```

**Update CLI** (`src/cli/clean.rs`):

```rust
// After scanning, before display:
let projects = if let Some(age_days) = args.age {
    println!("Filtering projects not modified in {} days...", age_days);
    ProjectScanner::filter_by_age(projects, age_days)
} else {
    projects
};
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use filetime::{set_file_mtime, FileTime};

    #[test]
    fn test_filter_by_age() {
        let tmp = TempDir::new().unwrap();

        // Recent project
        let recent = tmp.path().join("recent");
        fs::create_dir(&recent).unwrap();
        fs::write(recent.join("Cargo.toml"), "[package]").unwrap();

        // Old project (set mtime to 30 days ago)
        let old = tmp.path().join("old");
        fs::create_dir(&old).unwrap();
        let old_toml = old.join("Cargo.toml");
        fs::write(&old_toml, "[package]").unwrap();

        let thirty_days_ago = SystemTime::now() - Duration::from_secs(30 * 24 * 60 * 60);
        let ft = FileTime::from_system_time(thirty_days_ago);
        set_file_mtime(&old_toml, ft).unwrap();

        let projects = vec![
            DetectedProject {
                path: recent,
                project_type: "cargo".into(),
                display_name: "Cargo".into(),
                artifact_size: 100,
                artifact_paths: vec![],
            },
            DetectedProject {
                path: old,
                project_type: "cargo".into(),
                display_name: "Cargo".into(),
                artifact_size: 100,
                artifact_paths: vec![],
            },
        ];

        // Filter: only projects older than 7 days
        let filtered = ProjectScanner::filter_by_age(projects, 7);

        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].path.ends_with("old"));
    }
}
```

### Acceptance Criteria

- [ ] Age filtering excludes recent projects
- [ ] mtime calculation ignores artifact directories
- [ ] CLI `--age` flag works
- [ ] Tests pass

---

## Task 10: Integration Tests

**Status**: `[ ]`

### Description

Create comprehensive integration tests with realistic project structures.

### Context

End-to-end tests ensure all components work together correctly.

### Implementation

**File**: `tests/clean_integration.rs`

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Create a realistic test environment with multiple project types
fn create_test_workspace() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Rust/Cargo project
    let rust_proj = root.join("rust-app");
    fs::create_dir_all(rust_proj.join("src")).unwrap();
    fs::write(rust_proj.join("Cargo.toml"), r#"
[package]
name = "rust-app"
version = "0.1.0"
"#).unwrap();
    fs::write(rust_proj.join("src/main.rs"), "fn main() {}").unwrap();
    fs::create_dir_all(rust_proj.join("target/debug")).unwrap();
    fs::write(rust_proj.join("target/debug/rust-app"), "x".repeat(50000)).unwrap();
    fs::write(rust_proj.join("target/debug/deps.rlib"), "x".repeat(30000)).unwrap();

    // Node.js/npm project
    let node_proj = root.join("web-app");
    fs::create_dir_all(&node_proj).unwrap();
    fs::write(node_proj.join("package.json"), r#"{"name": "web-app"}"#).unwrap();
    fs::write(node_proj.join("index.js"), "console.log('hi')").unwrap();
    fs::create_dir_all(node_proj.join("node_modules/lodash")).unwrap();
    fs::write(node_proj.join("node_modules/lodash/index.js"), "x".repeat(20000)).unwrap();

    // Gradle/Android project
    let gradle_proj = root.join("android-app");
    fs::create_dir_all(&gradle_proj).unwrap();
    fs::write(gradle_proj.join("build.gradle"), "apply plugin: 'android'").unwrap();
    fs::create_dir_all(gradle_proj.join("build/outputs")).unwrap();
    fs::write(gradle_proj.join("build/outputs/app.apk"), "x".repeat(100000)).unwrap();
    fs::create_dir_all(gradle_proj.join(".gradle/caches")).unwrap();
    fs::write(gradle_proj.join(".gradle/caches/cache.bin"), "x".repeat(40000)).unwrap();

    // Regular directory (not a project)
    let docs = root.join("docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(docs.join("readme.md"), "# Documentation").unwrap();

    // Nested project (should be found)
    let nested = root.join("projects/libs/util-lib");
    fs::create_dir_all(nested.join("src")).unwrap();
    fs::write(nested.join("Cargo.toml"), "[package]\nname = \"util\"").unwrap();
    fs::create_dir_all(nested.join("target/release")).unwrap();
    fs::write(nested.join("target/release/libutil.so"), "x".repeat(25000)).unwrap();

    tmp
}

#[test]
fn test_scan_finds_all_projects() {
    let tmp = create_test_workspace();

    let mut cmd = Command::cargo_bin("rusty-sweeper").unwrap();
    cmd.arg("clean")
        .arg("--size-only")
        .arg(tmp.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("cargo"))
        .stdout(predicate::str::contains("npm"))
        .stdout(predicate::str::contains("gradle"))
        .stdout(predicate::str::contains("4 projects"));
}

#[test]
fn test_clean_removes_artifacts() {
    let tmp = create_test_workspace();

    let mut cmd = Command::cargo_bin("rusty-sweeper").unwrap();
    cmd.arg("clean")
        .arg("--force")
        .arg(tmp.path());

    cmd.assert().success();

    // Artifacts should be gone
    assert!(!tmp.path().join("rust-app/target").exists());
    assert!(!tmp.path().join("web-app/node_modules").exists());
    assert!(!tmp.path().join("android-app/build").exists());
    assert!(!tmp.path().join("projects/libs/util-lib/target").exists());

    // Source files should remain
    assert!(tmp.path().join("rust-app/src/main.rs").exists());
    assert!(tmp.path().join("web-app/index.js").exists());
    assert!(tmp.path().join("docs/readme.md").exists());
}

#[test]
fn test_type_filtering() {
    let tmp = create_test_workspace();

    let mut cmd = Command::cargo_bin("rusty-sweeper").unwrap();
    cmd.arg("clean")
        .arg("--force")
        .arg("--types=cargo")
        .arg(tmp.path());

    cmd.assert().success();

    // Only cargo artifacts removed
    assert!(!tmp.path().join("rust-app/target").exists());
    assert!(!tmp.path().join("projects/libs/util-lib/target").exists());

    // Other project artifacts remain
    assert!(tmp.path().join("web-app/node_modules").exists());
    assert!(tmp.path().join("android-app/build").exists());
}

#[test]
fn test_exclude_patterns() {
    let tmp = create_test_workspace();

    let mut cmd = Command::cargo_bin("rusty-sweeper").unwrap();
    cmd.arg("clean")
        .arg("--force")
        .arg("--exclude=projects")
        .arg(tmp.path());

    cmd.assert().success();

    // Excluded project should remain
    assert!(tmp.path().join("projects/libs/util-lib/target").exists());

    // Other projects cleaned
    assert!(!tmp.path().join("rust-app/target").exists());
}

#[test]
fn test_dry_run_preserves_all() {
    let tmp = create_test_workspace();

    // Get initial sizes
    let initial_target = tmp.path().join("rust-app/target");
    assert!(initial_target.exists());

    let mut cmd = Command::cargo_bin("rusty-sweeper").unwrap();
    cmd.arg("clean")
        .arg("--dry-run")
        .arg(tmp.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[DRY RUN]"));

    // Everything still exists
    assert!(tmp.path().join("rust-app/target").exists());
    assert!(tmp.path().join("web-app/node_modules").exists());
    assert!(tmp.path().join("android-app/build").exists());
}

#[test]
fn test_size_calculation() {
    let tmp = create_test_workspace();

    let mut cmd = Command::cargo_bin("rusty-sweeper").unwrap();
    cmd.arg("clean")
        .arg("--size-only")
        .arg(tmp.path());

    // Total artifacts: 50000 + 30000 + 20000 + 100000 + 40000 + 25000 = 265000 bytes
    // ~259 KiB or ~0.25 MiB
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("KiB").or(predicate::str::contains("MiB")));
}

#[test]
fn test_empty_directory() {
    let tmp = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("rusty-sweeper").unwrap();
    cmd.arg("clean")
        .arg("--size-only")
        .arg(tmp.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No projects"));
}

#[test]
fn test_max_depth_limiting() {
    let tmp = create_test_workspace();

    let mut cmd = Command::cargo_bin("rusty-sweeper").unwrap();
    cmd.arg("clean")
        .arg("--size-only")
        .arg("--max-depth=1")
        .arg(tmp.path());

    cmd.assert()
        .success()
        // Should find top-level projects but not nested one
        .stdout(predicate::str::contains("3 projects"));
}
```

### Acceptance Criteria

- [ ] Finds all project types in realistic structure
- [ ] Cleans artifacts correctly
- [ ] Type filtering works
- [ ] Exclude patterns work
- [ ] Dry run preserves everything
- [ ] Size calculation is reasonable
- [ ] Handles empty directories
- [ ] Max depth limiting works
- [ ] All integration tests pass

---

## Summary

| Task | Description | Status |
|------|-------------|--------|
| 1 | ProjectDetector trait | `[x]` |
| 2 | Cargo detector | `[x]` |
| 3 | Remaining detectors (8) | `[x]` |
| 4 | Detector registry | `[x]` |
| 5 | Project scanner | `[x]` |
| 6 | Clean executor | `[x]` |
| 7 | Parallel orchestrator | `[ ]` |
| 8 | CLI subcommand | `[ ]` |
| 9 | Age filtering | `[ ]` |
| 10 | Integration tests | `[ ]` |

**Total estimated complexity**: Medium-High

**Dependencies**:
- Phase 1 must be complete (CLI skeleton, config, errors)
- Phase 2 must be complete (scanner for size calculation)

**Crates to add**:
```toml
[dependencies]
walkdir = "2"
rayon = "1"
humansize = "2"
filetime = "0.2"  # For age filtering tests

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
predicates = "3"
```
