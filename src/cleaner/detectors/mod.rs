//! Built-in project detectors.

mod bazel;
mod cargo;
mod cmake;
mod dotnet;
mod go;
mod gradle;
mod maven;
mod npm;
mod python;

pub use bazel::BazelDetector;
pub use cargo::CargoDetector;
pub use cmake::CMakeDetector;
pub use dotnet::DotnetDetector;
pub use go::GoDetector;
pub use gradle::GradleDetector;
pub use maven::MavenDetector;
pub use npm::NpmDetector;
pub use python::PythonDetector;

use crate::cleaner::ProjectDetector;

/// Returns all built-in detectors.
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
