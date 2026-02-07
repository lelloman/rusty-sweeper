//! Trait and types for system-wide cleanup targets.
//!
//! Unlike `ProjectDetector` which finds per-project build artifacts,
//! system cleaners target resources managed by system-level services
//! (e.g., Docker daemon) that aren't tied to individual project directories.

/// A detected system-level resource that can be cleaned.
#[derive(Debug, Clone)]
pub struct DetectedSystemResource {
    /// Unique identifier (e.g., "docker-build-cache", "docker-images").
    pub resource_id: String,
    /// Human-readable name (e.g., "Docker Build Cache").
    pub display_name: String,
    /// Category grouping (e.g., "docker").
    pub category: String,
    /// Reclaimable size in bytes.
    pub size: u64,
    /// Description of the resource.
    pub description: String,
    /// Number of items, if applicable.
    pub item_count: Option<u64>,
}

/// Result of a system clean operation.
#[derive(Debug)]
pub enum SystemCleanResult {
    /// Cleaning succeeded.
    Success {
        resource: DetectedSystemResource,
        freed_bytes: u64,
    },
    /// Cleaning failed.
    Failed {
        resource: DetectedSystemResource,
        error: String,
    },
    /// Cleaning was skipped.
    Skipped {
        resource: DetectedSystemResource,
        reason: String,
    },
}

/// Trait for system-wide cleanup targets.
pub trait SystemCleaner: Send + Sync {
    /// Unique identifier for this cleaner (e.g., "docker").
    fn id(&self) -> &'static str;

    /// Human-readable name (e.g., "Docker").
    fn display_name(&self) -> &'static str;

    /// Check if the underlying system service is available.
    fn is_available(&self) -> bool;

    /// Detect reclaimable resources.
    fn detect(&self) -> Vec<DetectedSystemResource>;

    /// Clean a specific resource.
    fn clean(&self, resource: &DetectedSystemResource, dry_run: bool) -> SystemCleanResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detected_system_resource_creation() {
        let resource = DetectedSystemResource {
            resource_id: "test-resource".to_string(),
            display_name: "Test Resource".to_string(),
            category: "test".to_string(),
            size: 1024,
            description: "A test resource".to_string(),
            item_count: Some(5),
        };

        assert_eq!(resource.resource_id, "test-resource");
        assert_eq!(resource.display_name, "Test Resource");
        assert_eq!(resource.category, "test");
        assert_eq!(resource.size, 1024);
        assert_eq!(resource.description, "A test resource");
        assert_eq!(resource.item_count, Some(5));
    }

    #[test]
    fn test_system_clean_result_variants() {
        let resource = DetectedSystemResource {
            resource_id: "test".to_string(),
            display_name: "Test".to_string(),
            category: "test".to_string(),
            size: 100,
            description: "test".to_string(),
            item_count: None,
        };

        let _success = SystemCleanResult::Success {
            resource: resource.clone(),
            freed_bytes: 100,
        };

        let _failed = SystemCleanResult::Failed {
            resource: resource.clone(),
            error: "error".to_string(),
        };

        let _skipped = SystemCleanResult::Skipped {
            resource,
            reason: "reason".to_string(),
        };
    }
}
