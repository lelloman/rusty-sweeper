use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use nix::sys::statvfs::statvfs;

use super::types::DiskStatus;
use crate::error::Result;

/// Check disk usage for a specific path
pub fn check_disk_usage(path: &Path) -> Result<DiskStatus> {
    let stat = statvfs(path)?;

    let block_size = stat.fragment_size() as u64;
    let total = stat.blocks() as u64 * block_size;
    let available = stat.blocks_available() as u64 * block_size;
    let free = stat.blocks_free() as u64 * block_size;

    // Used = total - free (not available, as available excludes reserved blocks)
    let used = total - free;

    // Percent is based on non-reserved space (what users can actually use)
    let usable_total = used + available;
    let percent = if usable_total > 0 {
        (used as f64 / usable_total as f64 * 100.0) as f32
    } else {
        0.0
    };

    Ok(DiskStatus {
        mount_point: path.to_path_buf(),
        device: None, // Filled in by caller if needed
        total,
        used,
        available,
        percent,
    })
}

/// Information about a mount point
#[derive(Debug, Clone)]
pub struct MountPoint {
    pub device: String,
    pub path: PathBuf,
    pub fs_type: String,
}

/// Get list of real (non-virtual) mount points
pub fn get_mount_points() -> Result<Vec<MountPoint>> {
    let file = File::open("/proc/mounts")?;
    let reader = BufReader::new(file);

    let mut mounts = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 3 {
            continue;
        }

        let device = parts[0];
        let mount_point = parts[1];
        let fs_type = parts[2];

        // Skip virtual filesystems
        if is_virtual_filesystem(fs_type, device, mount_point) {
            continue;
        }

        mounts.push(MountPoint {
            device: device.to_string(),
            path: PathBuf::from(mount_point),
            fs_type: fs_type.to_string(),
        });
    }

    Ok(mounts)
}

/// Check if a filesystem type is virtual (not real disk)
fn is_virtual_filesystem(fs_type: &str, device: &str, mount_point: &str) -> bool {
    // Virtual filesystem types to skip
    const VIRTUAL_FS: &[&str] = &[
        "proc",
        "sysfs",
        "devtmpfs",
        "devpts",
        "tmpfs",
        "securityfs",
        "cgroup",
        "cgroup2",
        "pstore",
        "debugfs",
        "hugetlbfs",
        "mqueue",
        "fusectl",
        "configfs",
        "binfmt_misc",
        "autofs",
        "efivarfs",
        "tracefs",
        "bpf",
        "overlay",
        "squashfs",
        "nsfs",
        "ramfs",
    ];

    // Skip by filesystem type
    if VIRTUAL_FS.contains(&fs_type) {
        return true;
    }

    // Skip snap mounts
    if mount_point.starts_with("/snap/") {
        return true;
    }

    // Skip docker overlay mounts
    if mount_point.starts_with("/var/lib/docker/") {
        return true;
    }

    // Skip if device doesn't start with / (virtual devices)
    if !device.starts_with('/') && device != "none" {
        // Exception: some network mounts like NFS
        if !device.contains(':') {
            return true;
        }
    }

    false
}

/// Check all mount points and return their status
pub fn check_all_mount_points() -> Result<Vec<DiskStatus>> {
    let mounts = get_mount_points()?;
    let mut results = Vec::new();

    for mount in mounts {
        match check_disk_usage(&mount.path) {
            Ok(mut status) => {
                status.device = Some(mount.device);
                results.push(status);
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to check mount point {}: {}",
                    mount.path.display(),
                    e
                );
            }
        }
    }

    Ok(results)
}

/// Check specific mount points
pub fn check_mount_points(paths: &[PathBuf]) -> Result<Vec<DiskStatus>> {
    let mut results = Vec::new();

    for path in paths {
        match check_disk_usage(path) {
            Ok(status) => results.push(status),
            Err(e) => {
                tracing::warn!("Failed to check mount point {}: {}", path.display(), e);
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_disk_usage_root() {
        let status = check_disk_usage(Path::new("/")).unwrap();

        assert!(status.total > 0);
        assert!(status.percent >= 0.0 && status.percent <= 100.0);
        assert!(status.used + status.available <= status.total);
    }

    #[test]
    fn test_check_disk_usage_home() {
        if Path::new("/home").exists() {
            let status = check_disk_usage(Path::new("/home")).unwrap();
            assert!(status.total > 0);
        }
    }

    #[test]
    fn test_get_mount_points() {
        let mounts = get_mount_points().unwrap();

        // Should find at least root
        assert!(!mounts.is_empty());
        assert!(mounts.iter().any(|m| m.path == PathBuf::from("/")));
    }

    #[test]
    fn test_virtual_fs_detection() {
        assert!(is_virtual_filesystem("proc", "proc", "/proc"));
        assert!(is_virtual_filesystem("sysfs", "sysfs", "/sys"));
        assert!(is_virtual_filesystem("tmpfs", "tmpfs", "/tmp"));
        assert!(is_virtual_filesystem(
            "squashfs",
            "/dev/loop0",
            "/snap/core/1234"
        ));

        assert!(!is_virtual_filesystem("ext4", "/dev/sda1", "/"));
        assert!(!is_virtual_filesystem("xfs", "/dev/nvme0n1p2", "/home"));
    }

    #[test]
    fn test_check_all_mount_points() {
        let statuses = check_all_mount_points().unwrap();

        // Should have at least one mount point
        assert!(!statuses.is_empty());

        // All should have valid percentages
        for status in &statuses {
            assert!(status.percent >= 0.0 && status.percent <= 100.0);
        }
    }

    #[test]
    fn test_check_mount_points_custom() {
        let paths = vec![PathBuf::from("/")];
        let statuses = check_mount_points(&paths).unwrap();

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].mount_point, PathBuf::from("/"));
    }
}
