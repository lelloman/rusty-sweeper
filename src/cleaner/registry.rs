//! Detector registry for managing project detectors.

use crate::cleaner::detectors::all_detectors;
use crate::cleaner::ProjectDetector;
use std::collections::HashSet;

/// Registry that manages project detectors.
///
/// Provides functionality to filter detectors by type, which is useful
/// for CLI options like `--types cargo,npm`.
pub struct DetectorRegistry {
    detectors: Vec<Box<dyn ProjectDetector>>,
}

impl DetectorRegistry {
    /// Create a registry with all built-in detectors.
    pub fn new() -> Self {
        Self {
            detectors: all_detectors(),
        }
    }

    /// Create a registry with only the specified detector types.
    ///
    /// # Example
    /// ```
    /// use rusty_sweeper::cleaner::DetectorRegistry;
    ///
    /// let registry = DetectorRegistry::with_types(&["cargo", "npm"]);
    /// assert_eq!(registry.len(), 2);
    /// ```
    pub fn with_types(types: &[&str]) -> Self {
        let type_set: HashSet<&str> = types.iter().copied().collect();
        Self {
            detectors: all_detectors()
                .into_iter()
                .filter(|d| type_set.contains(d.id()))
                .collect(),
        }
    }

    /// Create a registry excluding the specified detector types.
    pub fn without_types(types: &[&str]) -> Self {
        let type_set: HashSet<&str> = types.iter().copied().collect();
        Self {
            detectors: all_detectors()
                .into_iter()
                .filter(|d| !type_set.contains(d.id()))
                .collect(),
        }
    }

    /// Get all registered detectors.
    pub fn detectors(&self) -> &[Box<dyn ProjectDetector>] {
        &self.detectors
    }

    /// Get a detector by ID.
    pub fn get(&self, id: &str) -> Option<&dyn ProjectDetector> {
        self.detectors
            .iter()
            .find(|d| d.id() == id)
            .map(|d| d.as_ref())
    }

    /// List all detector IDs.
    pub fn ids(&self) -> Vec<&str> {
        self.detectors.iter().map(|d| d.id()).collect()
    }

    /// Get the number of registered detectors.
    pub fn len(&self) -> usize {
        self.detectors.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.detectors.is_empty()
    }
}

impl Default for DetectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DetectorRegistry {
    fn clone(&self) -> Self {
        // Re-create from all_detectors filtered by current IDs
        let current_ids: HashSet<&str> = self.ids().into_iter().collect();
        Self {
            detectors: all_detectors()
                .into_iter()
                .filter(|d| current_ids.contains(d.id()))
                .collect(),
        }
    }
}

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
        assert!(ids.contains(&"maven"));
        assert!(ids.contains(&"go"));
        assert!(ids.contains(&"cmake"));
        assert!(ids.contains(&"python"));
        assert!(ids.contains(&"bazel"));
        assert!(ids.contains(&"dotnet"));
        assert_eq!(ids.len(), 9);
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
    fn test_registry_with_empty_types() {
        let registry = DetectorRegistry::with_types(&[]);

        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_without_types() {
        let registry = DetectorRegistry::without_types(&["cargo"]);
        let ids = registry.ids();

        assert!(!ids.contains(&"cargo"));
        assert!(ids.contains(&"npm"));
        assert!(ids.contains(&"gradle"));
        assert_eq!(ids.len(), 8);
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

    #[test]
    fn test_registry_len() {
        let full = DetectorRegistry::new();
        assert_eq!(full.len(), 9);

        let partial = DetectorRegistry::with_types(&["cargo"]);
        assert_eq!(partial.len(), 1);
    }

    #[test]
    fn test_registry_default() {
        let registry = DetectorRegistry::default();
        assert_eq!(registry.len(), 9);
    }

    #[test]
    fn test_registry_clone() {
        let registry = DetectorRegistry::with_types(&["cargo", "npm"]);
        let cloned = registry.clone();

        assert_eq!(cloned.len(), 2);
        assert!(cloned.get("cargo").is_some());
        assert!(cloned.get("npm").is_some());
    }
}
