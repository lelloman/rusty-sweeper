//! Registry for system-wide cleaners.

use crate::cleaner::docker::DockerCleaner;
use crate::cleaner::system_cleaner::{DetectedSystemResource, SystemCleaner};
use std::collections::HashSet;

/// Registry that manages system-wide cleaners.
pub struct SystemCleanerRegistry {
    cleaners: Vec<Box<dyn SystemCleaner>>,
}

impl SystemCleanerRegistry {
    /// Create a registry with all built-in system cleaners.
    pub fn new() -> Self {
        Self {
            cleaners: vec![Box::new(DockerCleaner)],
        }
    }

    /// Create a registry with only cleaners matching the given type IDs.
    pub fn with_types(types: &[&str]) -> Self {
        let type_set: HashSet<&str> = types.iter().copied().collect();
        let all_cleaners: Vec<Box<dyn SystemCleaner>> = vec![Box::new(DockerCleaner)];
        Self {
            cleaners: all_cleaners
                .into_iter()
                .filter(|c| type_set.contains(c.id()))
                .collect(),
        }
    }

    /// List all system cleaner IDs.
    pub fn ids(&self) -> Vec<&str> {
        self.cleaners.iter().map(|c| c.id()).collect()
    }

    /// Detect all reclaimable system resources.
    ///
    /// Skips cleaners whose underlying service is unavailable.
    pub fn detect_all(&self) -> Vec<DetectedSystemResource> {
        let mut resources = Vec::new();
        for cleaner in &self.cleaners {
            if cleaner.is_available() {
                resources.extend(cleaner.detect());
            }
        }
        resources
    }

    /// Get the cleaner for a given resource category.
    pub fn get_cleaner(&self, category: &str) -> Option<&dyn SystemCleaner> {
        self.cleaners
            .iter()
            .find(|c| c.id() == category)
            .map(|c| c.as_ref())
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.cleaners.is_empty()
    }
}

impl Default for SystemCleanerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Return all known system cleaner IDs.
pub fn all_system_cleaner_ids() -> Vec<&'static str> {
    vec!["docker"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new() {
        let registry = SystemCleanerRegistry::new();
        assert_eq!(registry.ids(), vec!["docker"]);
    }

    #[test]
    fn test_registry_with_types_matching() {
        let registry = SystemCleanerRegistry::with_types(&["docker"]);
        assert_eq!(registry.ids(), vec!["docker"]);
    }

    #[test]
    fn test_registry_with_types_no_match() {
        let registry = SystemCleanerRegistry::with_types(&["nonexistent"]);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_with_types_empty() {
        let registry = SystemCleanerRegistry::with_types(&[]);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_get_cleaner() {
        let registry = SystemCleanerRegistry::new();
        assert!(registry.get_cleaner("docker").is_some());
        assert!(registry.get_cleaner("nonexistent").is_none());
    }

    #[test]
    fn test_all_system_cleaner_ids() {
        let ids = all_system_cleaner_ids();
        assert_eq!(ids, vec!["docker"]);
    }
}
