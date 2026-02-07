//! Project detection and cleaning functionality.
//!
//! This module provides:
//! - Detection of various project types (Cargo, Gradle, npm, etc.)
//! - Cleanup of build artifacts
//! - Parallel cleaning orchestration

mod detector;
pub mod detectors;
pub mod docker;
mod executor;
mod orchestrator;
mod project_scanner;
mod registry;
pub mod system_cleaner;
pub mod system_registry;

pub use detector::{DetectedProject, ProjectDetector};
pub use detectors::all_detectors;
pub use executor::{CleanExecutor, CleanOptions, CleanResult};
pub use orchestrator::{CleanOrchestrator, CleanProgress, CleanSummary};
pub use project_scanner::{ProjectScanner, ScanOptions};
pub use registry::{all_valid_type_ids, DetectorRegistry};
pub use system_cleaner::{DetectedSystemResource, SystemCleanResult, SystemCleaner};
pub use system_registry::SystemCleanerRegistry;
