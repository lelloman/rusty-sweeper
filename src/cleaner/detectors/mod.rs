//! Built-in project detectors.

mod cargo;

pub use cargo::CargoDetector;

use crate::cleaner::ProjectDetector;

/// Returns all built-in detectors.
pub fn all_detectors() -> Vec<Box<dyn ProjectDetector>> {
    vec![Box::new(CargoDetector)]
}
