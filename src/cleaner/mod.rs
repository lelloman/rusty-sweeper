//! Project detection and cleaning functionality.
//!
//! This module provides:
//! - Detection of various project types (Cargo, Gradle, npm, etc.)
//! - Cleanup of build artifacts
//! - Parallel cleaning orchestration

mod detector;
pub mod detectors;
mod project_scanner;
mod registry;

pub use detector::{DetectedProject, ProjectDetector};
pub use detectors::all_detectors;
pub use project_scanner::{ProjectScanner, ScanOptions};
pub use registry::DetectorRegistry;
