//! Docker system cleaner.
//!
//! Detects and cleans Docker build cache and dangling images via the Docker CLI.

use crate::cleaner::system_cleaner::{DetectedSystemResource, SystemCleanResult, SystemCleaner};
use std::process::Command;

/// Docker system cleaner.
pub struct DockerCleaner;

impl SystemCleaner for DockerCleaner {
    fn id(&self) -> &'static str {
        "docker"
    }

    fn display_name(&self) -> &'static str {
        "Docker"
    }

    fn is_available(&self) -> bool {
        Command::new("docker")
            .arg("info")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn detect(&self) -> Vec<DetectedSystemResource> {
        let output = Command::new("docker")
            .args(["system", "df", "--format", "{{json .}}"])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                parse_system_df_output(&stdout)
            }
            _ => Vec::new(),
        }
    }

    fn clean(&self, resource: &DetectedSystemResource, dry_run: bool) -> SystemCleanResult {
        if dry_run {
            return SystemCleanResult::Success {
                resource: resource.clone(),
                freed_bytes: resource.size,
            };
        }

        let result = match resource.resource_id.as_str() {
            "docker-build-cache" => Command::new("docker")
                .args(["builder", "prune", "-f"])
                .output(),
            "docker-images" => Command::new("docker")
                .args(["image", "prune", "-f", "-a"])
                .output(),
            _ => {
                return SystemCleanResult::Skipped {
                    resource: resource.clone(),
                    reason: format!("Unknown resource: {}", resource.resource_id),
                };
            }
        };

        match result {
            Ok(output) if output.status.success() => SystemCleanResult::Success {
                resource: resource.clone(),
                freed_bytes: resource.size,
            },
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                SystemCleanResult::Failed {
                    resource: resource.clone(),
                    error: format!("Command failed: {}", stderr.trim()),
                }
            }
            Err(e) => SystemCleanResult::Failed {
                resource: resource.clone(),
                error: e.to_string(),
            },
        }
    }
}

/// Parse the output of `docker system df --format '{{json .}}'`.
///
/// Each line is a JSON object like:
/// ```json
/// {"Type":"Build Cache","TotalCount":"0","Active":"0","Size":"0B","Reclaimable":"0B"}
/// {"Type":"Images","TotalCount":"5","Active":"2","Size":"1.2GB","Reclaimable":"500MB (41%)"}
/// ```
///
/// We extract Build Cache and Images, skipping Containers and Volumes.
pub fn parse_system_df_output(output: &str) -> Vec<DetectedSystemResource> {
    let mut resources = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parsed: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let type_name = parsed["Type"].as_str().unwrap_or("");
        let reclaimable_str = parsed["Reclaimable"].as_str().unwrap_or("0B");
        let total_count_str = parsed["TotalCount"].as_str().unwrap_or("0");

        let (resource_id, display_name, description) = match type_name {
            "Build Cache" => (
                "docker-build-cache",
                "Docker Build Cache",
                "Cached build layers",
            ),
            "Images" => (
                "docker-images",
                "Docker Images",
                "Dangling images (reclaimable)",
            ),
            _ => continue, // Skip Containers, Volumes
        };

        let size = parse_docker_size(reclaimable_str);
        let item_count = total_count_str.parse::<u64>().ok();

        resources.push(DetectedSystemResource {
            resource_id: resource_id.to_string(),
            display_name: display_name.to_string(),
            category: "docker".to_string(),
            size,
            description: description.to_string(),
            item_count,
        });
    }

    resources
}

/// Parse a Docker size string into bytes.
///
/// Handles formats like:
/// - `"2.547GB"`
/// - `"500MB (41%)"`
/// - `"0B"`
/// - `"1.5kB"`
fn parse_docker_size(s: &str) -> u64 {
    // Strip any parenthetical suffix like " (41%)"
    let size_part = s.split('(').next().unwrap_or(s).trim();

    if size_part == "0B" || size_part.is_empty() {
        return 0;
    }

    // Find where the numeric part ends and the unit begins
    let unit_start = size_part
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(size_part.len());

    let (num_str, unit) = size_part.split_at(unit_start);

    let number: f64 = match num_str.parse() {
        Ok(n) => n,
        Err(_) => return 0,
    };

    let multiplier: f64 = match unit {
        "B" => 1.0,
        "kB" | "KB" => 1000.0,
        "MB" => 1_000_000.0,
        "GB" => 1_000_000_000.0,
        "TB" => 1_000_000_000_000.0,
        _ => return 0,
    };

    (number * multiplier) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_docker_size_bytes() {
        assert_eq!(parse_docker_size("0B"), 0);
        assert_eq!(parse_docker_size("100B"), 100);
    }

    #[test]
    fn test_parse_docker_size_kilobytes() {
        assert_eq!(parse_docker_size("1kB"), 1000);
        assert_eq!(parse_docker_size("1.5kB"), 1500);
    }

    #[test]
    fn test_parse_docker_size_megabytes() {
        assert_eq!(parse_docker_size("500MB"), 500_000_000);
        assert_eq!(parse_docker_size("1.2MB"), 1_200_000);
    }

    #[test]
    fn test_parse_docker_size_gigabytes() {
        assert_eq!(parse_docker_size("2.547GB"), 2_547_000_000);
        assert_eq!(parse_docker_size("1GB"), 1_000_000_000);
    }

    #[test]
    fn test_parse_docker_size_terabytes() {
        assert_eq!(parse_docker_size("1TB"), 1_000_000_000_000);
    }

    #[test]
    fn test_parse_docker_size_with_percentage() {
        assert_eq!(parse_docker_size("500MB (41%)"), 500_000_000);
        assert_eq!(parse_docker_size("2.547GB (50%)"), 2_547_000_000);
    }

    #[test]
    fn test_parse_docker_size_empty() {
        assert_eq!(parse_docker_size(""), 0);
    }

    #[test]
    fn test_parse_docker_size_invalid() {
        assert_eq!(parse_docker_size("invalid"), 0);
        assert_eq!(parse_docker_size("notanumber GB"), 0);
    }

    #[test]
    fn test_parse_system_df_output_full() {
        let output = r#"{"Type":"Images","TotalCount":"5","Active":"2","Size":"1.2GB","Reclaimable":"500MB (41%)"}
{"Type":"Containers","TotalCount":"3","Active":"1","Size":"50MB","Reclaimable":"30MB (60%)"}
{"Type":"Local Volumes","TotalCount":"2","Active":"1","Size":"100MB","Reclaimable":"50MB (50%)"}
{"Type":"Build Cache","TotalCount":"10","Active":"0","Size":"2.547GB","Reclaimable":"2.547GB"}"#;

        let resources = parse_system_df_output(output);

        assert_eq!(resources.len(), 2);

        // Images should be first (in output order)
        assert_eq!(resources[0].resource_id, "docker-images");
        assert_eq!(resources[0].display_name, "Docker Images");
        assert_eq!(resources[0].category, "docker");
        assert_eq!(resources[0].size, 500_000_000);
        assert_eq!(resources[0].item_count, Some(5));

        // Build Cache second
        assert_eq!(resources[1].resource_id, "docker-build-cache");
        assert_eq!(resources[1].display_name, "Docker Build Cache");
        assert_eq!(resources[1].size, 2_547_000_000);
        assert_eq!(resources[1].item_count, Some(10));
    }

    #[test]
    fn test_parse_system_df_output_empty() {
        let resources = parse_system_df_output("");
        assert!(resources.is_empty());
    }

    #[test]
    fn test_parse_system_df_output_invalid_json() {
        let resources = parse_system_df_output("not json\nalso not json");
        assert!(resources.is_empty());
    }

    #[test]
    fn test_parse_system_df_output_zero_reclaimable() {
        let output = r#"{"Type":"Build Cache","TotalCount":"0","Active":"0","Size":"0B","Reclaimable":"0B"}"#;

        let resources = parse_system_df_output(output);
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].size, 0);
    }

    #[test]
    fn test_parse_system_df_output_only_containers_and_volumes() {
        let output = r#"{"Type":"Containers","TotalCount":"3","Active":"1","Size":"50MB","Reclaimable":"30MB (60%)"}
{"Type":"Local Volumes","TotalCount":"2","Active":"1","Size":"100MB","Reclaimable":"50MB (50%)"}"#;

        let resources = parse_system_df_output(output);
        assert!(resources.is_empty());
    }
}
