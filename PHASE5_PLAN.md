# Phase 5: Monitor Service - Implementation Plan

## Overview

Phase 5 implements the background disk monitoring service that periodically checks disk usage and sends desktop notifications when thresholds are exceeded.

**Prerequisites**: Phase 1-4 must be complete.

**Status Legend**:
- `[ ]` Not started
- `[~]` In progress
- `[x]` Completed

---

## Task 1: Add Monitor Dependencies

**Status**: `[x]`

### Description

Add the required crates for disk monitoring and notifications.

### Context

We need:
- `notify-rust` for D-Bus desktop notifications
- `nix` for Unix system calls (statvfs, fork, signals)
- `daemonize` (optional) or manual fork for daemon mode

### Implementation

Add to `Cargo.toml`:

```toml
[dependencies]
notify-rust = "4"
nix = { version = "0.29", features = ["fs", "signal", "process"] }
```

### Acceptance Criteria

- [ ] `cargo check` passes
- [ ] `cargo build` compiles without errors

---

## Task 2: Define Monitor Data Structures

**Status**: `[x]`

### Description

Create the core data structures for disk status and alert levels.

### Context

We need types to represent:
- Current disk usage status for a mount point
- Alert severity levels (Warning, Critical, Emergency)
- Monitor configuration (thresholds, intervals)

### Implementation

**File**: `src/monitor/types.rs`

```rust
use std::path::PathBuf;
use std::time::Duration;

/// Disk usage status for a single mount point
#[derive(Debug, Clone)]
pub struct DiskStatus {
    /// Mount point path (e.g., "/", "/home")
    pub mount_point: PathBuf,

    /// Device name (e.g., "/dev/sda1")
    pub device: Option<String>,

    /// Total capacity in bytes
    pub total: u64,

    /// Used space in bytes
    pub used: u64,

    /// Available space in bytes
    pub available: u64,

    /// Usage percentage (0.0 - 100.0)
    pub percent: f32,
}

impl DiskStatus {
    /// Human-readable used space
    pub fn used_human(&self) -> String {
        humansize::format_size(self.used, humansize::BINARY)
    }

    /// Human-readable total space
    pub fn total_human(&self) -> String {
        humansize::format_size(self.total, humansize::BINARY)
    }

    /// Human-readable available space
    pub fn available_human(&self) -> String {
        humansize::format_size(self.available, humansize::BINARY)
    }
}

/// Alert severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertLevel {
    /// Normal - no alert needed
    Normal,
    /// Warning - usage >= warn_threshold (default 80%)
    Warning,
    /// Critical - usage >= critical_threshold (default 90%)
    Critical,
    /// Emergency - usage >= 95%
    Emergency,
}

impl AlertLevel {
    /// Determine alert level from usage percentage and thresholds
    pub fn from_percent(percent: f32, warn: u8, critical: u8) -> Self {
        if percent >= 95.0 {
            AlertLevel::Emergency
        } else if percent >= critical as f32 {
            AlertLevel::Critical
        } else if percent >= warn as f32 {
            AlertLevel::Warning
        } else {
            AlertLevel::Normal
        }
    }

    /// Get notification urgency for this level
    pub fn urgency(&self) -> NotificationUrgency {
        match self {
            AlertLevel::Normal => NotificationUrgency::Low,
            AlertLevel::Warning => NotificationUrgency::Normal,
            AlertLevel::Critical => NotificationUrgency::Critical,
            AlertLevel::Emergency => NotificationUrgency::Critical,
        }
    }
}

/// Notification urgency (maps to freedesktop urgency levels)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationUrgency {
    Low,
    Normal,
    Critical,
}

/// Runtime configuration for the monitor
#[derive(Debug, Clone)]
pub struct MonitorOptions {
    /// Check interval
    pub interval: Duration,

    /// Warning threshold percentage
    pub warn_threshold: u8,

    /// Critical threshold percentage
    pub critical_threshold: u8,

    /// Mount points to monitor (empty = auto-detect)
    pub mount_points: Vec<PathBuf>,

    /// Run as daemon
    pub daemon: bool,

    /// Check once and exit
    pub once: bool,

    /// Notification backend preference
    pub notification_backend: NotificationBackend,
}

/// Notification backend selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationBackend {
    #[default]
    Auto,
    DBus,
    NotifySend,
    I3Nagbar,
    Stderr,
}

impl Default for MonitorOptions {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(300),
            warn_threshold: 80,
            critical_threshold: 90,
            mount_points: vec![],
            daemon: false,
            once: false,
            notification_backend: NotificationBackend::Auto,
        }
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_level_from_percent() {
        assert_eq!(AlertLevel::from_percent(50.0, 80, 90), AlertLevel::Normal);
        assert_eq!(AlertLevel::from_percent(80.0, 80, 90), AlertLevel::Warning);
        assert_eq!(AlertLevel::from_percent(85.0, 80, 90), AlertLevel::Warning);
        assert_eq!(AlertLevel::from_percent(90.0, 80, 90), AlertLevel::Critical);
        assert_eq!(AlertLevel::from_percent(95.0, 80, 90), AlertLevel::Emergency);
        assert_eq!(AlertLevel::from_percent(99.0, 80, 90), AlertLevel::Emergency);
    }

    #[test]
    fn test_alert_level_ordering() {
        assert!(AlertLevel::Normal < AlertLevel::Warning);
        assert!(AlertLevel::Warning < AlertLevel::Critical);
        assert!(AlertLevel::Critical < AlertLevel::Emergency);
    }

    #[test]
    fn test_disk_status_human_readable() {
        let status = DiskStatus {
            mount_point: PathBuf::from("/"),
            device: Some("/dev/sda1".to_string()),
            total: 1024 * 1024 * 1024 * 100, // 100 GB
            used: 1024 * 1024 * 1024 * 80,   // 80 GB
            available: 1024 * 1024 * 1024 * 20, // 20 GB
            percent: 80.0,
        };

        assert!(status.total_human().contains("100"));
        assert!(status.used_human().contains("80"));
    }
}
```

### Acceptance Criteria

- [ ] All types compile
- [ ] AlertLevel ordering works correctly
- [ ] Default options match spec (300s interval, 80/90 thresholds)
- [ ] Unit tests pass

---

## Task 3: Implement Disk Usage Checker

**Status**: `[x]`

### Description

Create functions to check disk usage using `statvfs` system call.

### Context

We use `nix::sys::statvfs` to get filesystem statistics. We need to:
- Check a specific mount point
- Auto-detect mount points from `/proc/mounts` or `/etc/mtab`
- Filter out virtual filesystems (proc, sys, tmpfs, etc.)

### Implementation

**File**: `src/monitor/disk.rs`

```rust
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

    // Percent is based on non-reserved space
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

/// Information about a mount point
#[derive(Debug, Clone)]
pub struct MountPoint {
    pub device: String,
    pub path: PathBuf,
    pub fs_type: String,
}

/// Check if a filesystem type is virtual (not real disk)
fn is_virtual_filesystem(fs_type: &str, device: &str, mount_point: &str) -> bool {
    // Virtual filesystem types to skip
    const VIRTUAL_FS: &[&str] = &[
        "proc", "sysfs", "devtmpfs", "devpts", "tmpfs", "securityfs",
        "cgroup", "cgroup2", "pstore", "debugfs", "hugetlbfs", "mqueue",
        "fusectl", "configfs", "binfmt_misc", "autofs", "efivarfs",
        "tracefs", "bpf", "overlay", "squashfs", "nsfs", "ramfs",
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
                tracing::warn!(
                    "Failed to check mount point {}: {}",
                    path.display(),
                    e
                );
            }
        }
    }

    Ok(results)
}
```

### Tests

```rust
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
        assert!(is_virtual_filesystem("squashfs", "/dev/loop0", "/snap/core/1234"));

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
}
```

### Acceptance Criteria

- [ ] Can check disk usage for specific paths
- [ ] Auto-detects real mount points (excludes virtual fs)
- [ ] Percentage calculation is correct
- [ ] Handles errors gracefully (logs warning, continues)
- [ ] All tests pass

---

## Task 4: Define Notifier Trait

**Status**: `[x]`

### Description

Create a trait for notification backends that all implementations will use.

### Context

We support multiple notification methods. The trait provides a common interface so the monitor doesn't need to know which backend is being used.

### Implementation

**File**: `src/monitor/notifier.rs`

```rust
use super::types::{AlertLevel, DiskStatus, NotificationUrgency};
use crate::error::Result;

/// Trait for notification backends
pub trait Notifier: Send + Sync {
    /// Get the name of this backend
    fn name(&self) -> &'static str;

    /// Check if this backend is available on the current system
    fn is_available(&self) -> bool;

    /// Send a disk usage alert notification
    fn send_alert(&self, level: AlertLevel, status: &DiskStatus) -> Result<()>;

    /// Send a generic notification (for testing/custom messages)
    fn send(&self, title: &str, body: &str, urgency: NotificationUrgency) -> Result<()>;
}

/// Format the alert message body
pub fn format_alert_body(status: &DiskStatus) -> String {
    format!(
        "{} is {}% full\n\
         Used: {} of {}\n\
         Available: {}",
        status.mount_point.display(),
        status.percent as u32,
        status.used_human(),
        status.total_human(),
        status.available_human(),
    )
}

/// Format the alert title
pub fn format_alert_title(level: AlertLevel) -> &'static str {
    match level {
        AlertLevel::Normal => "Disk Usage Normal",
        AlertLevel::Warning => "⚠️ Disk Usage Warning",
        AlertLevel::Critical => "🔴 Disk Usage Critical",
        AlertLevel::Emergency => "🚨 DISK SPACE EMERGENCY",
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_format_alert_body() {
        let status = DiskStatus {
            mount_point: PathBuf::from("/home"),
            device: None,
            total: 100 * 1024 * 1024 * 1024,
            used: 85 * 1024 * 1024 * 1024,
            available: 15 * 1024 * 1024 * 1024,
            percent: 85.0,
        };

        let body = format_alert_body(&status);

        assert!(body.contains("/home"));
        assert!(body.contains("85%"));
    }

    #[test]
    fn test_format_alert_title() {
        assert!(format_alert_title(AlertLevel::Warning).contains("Warning"));
        assert!(format_alert_title(AlertLevel::Critical).contains("Critical"));
        assert!(format_alert_title(AlertLevel::Emergency).contains("EMERGENCY"));
    }
}
```

### Acceptance Criteria

- [ ] Trait defines all necessary methods
- [ ] Alert formatting produces readable messages
- [ ] Title reflects severity level
- [ ] Tests pass

---

## Task 5: Implement D-Bus Notifier

**Status**: `[x]`

### Description

Implement the primary notification backend using `notify-rust` for D-Bus notifications.

### Context

D-Bus notifications are the standard on modern Linux desktops. The `notify-rust` crate handles the complexity of the D-Bus protocol.

### Implementation

**File**: `src/monitor/notifiers/dbus.rs`

```rust
use notify_rust::{Notification, Urgency, Timeout};

use crate::monitor::notifier::{Notifier, format_alert_body, format_alert_title};
use crate::monitor::types::{AlertLevel, DiskStatus, NotificationUrgency};
use crate::error::Result;

pub struct DBusNotifier {
    app_name: String,
}

impl DBusNotifier {
    pub fn new() -> Self {
        Self {
            app_name: "Rusty Sweeper".to_string(),
        }
    }

    fn map_urgency(urgency: NotificationUrgency) -> Urgency {
        match urgency {
            NotificationUrgency::Low => Urgency::Low,
            NotificationUrgency::Normal => Urgency::Normal,
            NotificationUrgency::Critical => Urgency::Critical,
        }
    }
}

impl Default for DBusNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifier for DBusNotifier {
    fn name(&self) -> &'static str {
        "D-Bus"
    }

    fn is_available(&self) -> bool {
        // Try to create a notification to test D-Bus availability
        // This is a lightweight check that doesn't actually send anything
        std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok()
    }

    fn send_alert(&self, level: AlertLevel, status: &DiskStatus) -> Result<()> {
        let title = format_alert_title(level);
        let body = format_alert_body(status);
        let urgency = level.urgency();

        self.send(title, &body, urgency)
    }

    fn send(&self, title: &str, body: &str, urgency: NotificationUrgency) -> Result<()> {
        let timeout = match urgency {
            NotificationUrgency::Critical => Timeout::Never, // Persistent
            NotificationUrgency::Normal => Timeout::Milliseconds(10000),
            NotificationUrgency::Low => Timeout::Milliseconds(5000),
        };

        Notification::new()
            .appname(&self.app_name)
            .summary(title)
            .body(body)
            .icon("drive-harddisk")
            .urgency(Self::map_urgency(urgency))
            .timeout(timeout)
            .show()?;

        Ok(())
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dbus_notifier_creation() {
        let notifier = DBusNotifier::new();
        assert_eq!(notifier.name(), "D-Bus");
    }

    #[test]
    fn test_urgency_mapping() {
        assert!(matches!(
            DBusNotifier::map_urgency(NotificationUrgency::Low),
            Urgency::Low
        ));
        assert!(matches!(
            DBusNotifier::map_urgency(NotificationUrgency::Critical),
            Urgency::Critical
        ));
    }

    #[test]
    #[ignore] // Requires display server
    fn test_dbus_notification_send() {
        let notifier = DBusNotifier::new();

        if notifier.is_available() {
            let result = notifier.send(
                "Test Notification",
                "This is a test from rusty-sweeper",
                NotificationUrgency::Low,
            );
            assert!(result.is_ok());
        }
    }
}
```

### Acceptance Criteria

- [ ] Can send notifications via D-Bus
- [ ] Urgency levels map correctly
- [ ] Critical notifications are persistent
- [ ] Availability check works
- [ ] Tests pass

---

## Task 6: Implement notify-send Fallback

**Status**: `[x]`

### Description

Implement fallback notifier that shells out to `notify-send` command.

### Context

Some systems may not have D-Bus libraries available, but most have `notify-send` from `libnotify-bin`. This serves as a fallback.

### Implementation

**File**: `src/monitor/notifiers/notify_send.rs`

```rust
use std::process::Command;

use crate::monitor::notifier::{Notifier, format_alert_body, format_alert_title};
use crate::monitor::types::{AlertLevel, DiskStatus, NotificationUrgency};
use crate::error::Result;

pub struct NotifySendNotifier;

impl NotifySendNotifier {
    pub fn new() -> Self {
        Self
    }

    fn find_binary() -> Option<&'static str> {
        // Check common locations
        for path in &["/usr/bin/notify-send", "/bin/notify-send"] {
            if std::path::Path::new(path).exists() {
                return Some(path);
            }
        }

        // Try PATH lookup
        if Command::new("which")
            .arg("notify-send")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some("notify-send");
        }

        None
    }
}

impl Default for NotifySendNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifier for NotifySendNotifier {
    fn name(&self) -> &'static str {
        "notify-send"
    }

    fn is_available(&self) -> bool {
        Self::find_binary().is_some()
    }

    fn send_alert(&self, level: AlertLevel, status: &DiskStatus) -> Result<()> {
        let title = format_alert_title(level);
        let body = format_alert_body(status);
        let urgency = level.urgency();

        self.send(title, &body, urgency)
    }

    fn send(&self, title: &str, body: &str, urgency: NotificationUrgency) -> Result<()> {
        let binary = Self::find_binary()
            .ok_or_else(|| crate::error::Error::NotFound("notify-send".into()))?;

        let urgency_str = match urgency {
            NotificationUrgency::Low => "low",
            NotificationUrgency::Normal => "normal",
            NotificationUrgency::Critical => "critical",
        };

        let mut cmd = Command::new(binary);
        cmd.arg("--urgency").arg(urgency_str)
           .arg("--app-name").arg("Rusty Sweeper")
           .arg("--icon").arg("drive-harddisk")
           .arg(title)
           .arg(body);

        // Critical notifications should be persistent
        if urgency == NotificationUrgency::Critical {
            cmd.arg("--expire-time=0");
        }

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::Error::Command(format!(
                "notify-send failed: {}",
                stderr
            )));
        }

        Ok(())
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notify_send_availability() {
        let notifier = NotifySendNotifier::new();
        // Just test that availability check doesn't panic
        let _ = notifier.is_available();
    }

    #[test]
    #[ignore] // Requires notify-send and display
    fn test_notify_send_notification() {
        let notifier = NotifySendNotifier::new();

        if notifier.is_available() {
            let result = notifier.send(
                "Test",
                "Test notification",
                NotificationUrgency::Low,
            );
            assert!(result.is_ok());
        }
    }
}
```

### Acceptance Criteria

- [ ] Finds notify-send binary
- [ ] Sends notifications via command line
- [ ] Passes urgency correctly
- [ ] Handles missing binary gracefully
- [ ] Tests pass

---

## Task 7: Implement i3-nagbar Notifier

**Status**: `[x]`

### Description

Implement notifier for i3 window manager using `i3-nagbar`.

### Context

For i3wm users, `i3-nagbar` provides prominent notifications that require acknowledgment. This is useful for critical alerts.

### Implementation

**File**: `src/monitor/notifiers/i3nagbar.rs`

```rust
use std::process::Command;

use crate::monitor::notifier::{Notifier, format_alert_body, format_alert_title};
use crate::monitor::types::{AlertLevel, DiskStatus, NotificationUrgency};
use crate::error::Result;

pub struct I3NagbarNotifier;

impl I3NagbarNotifier {
    pub fn new() -> Self {
        Self
    }
}

impl Default for I3NagbarNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifier for I3NagbarNotifier {
    fn name(&self) -> &'static str {
        "i3-nagbar"
    }

    fn is_available(&self) -> bool {
        // Check if running under i3
        std::env::var("I3SOCK").is_ok() || std::env::var("SWAYSOCK").is_ok()
    }

    fn send_alert(&self, level: AlertLevel, status: &DiskStatus) -> Result<()> {
        // Only use i3-nagbar for critical/emergency
        if level < AlertLevel::Critical {
            return Ok(());
        }

        let title = format_alert_title(level);
        let body = format_alert_body(status);
        let urgency = level.urgency();

        self.send(title, &body, urgency)
    }

    fn send(&self, title: &str, body: &str, urgency: NotificationUrgency) -> Result<()> {
        let bar_type = match urgency {
            NotificationUrgency::Critical => "error",
            _ => "warning",
        };

        let message = format!("{}: {}", title, body.replace('\n', " | "));

        // Run in background (i3-nagbar blocks until dismissed)
        Command::new("i3-nagbar")
            .arg("-t").arg(bar_type)
            .arg("-m").arg(&message)
            .arg("-b").arg("Open TUI").arg("rusty-sweeper tui")
            .arg("-b").arg("Dismiss").arg("true")
            .spawn()?;

        Ok(())
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i3nagbar_availability() {
        let notifier = I3NagbarNotifier::new();
        // Should return false unless running in i3
        let available = notifier.is_available();

        // Verify it matches environment
        let expected = std::env::var("I3SOCK").is_ok()
            || std::env::var("SWAYSOCK").is_ok();
        assert_eq!(available, expected);
    }
}
```

### Acceptance Criteria

- [ ] Detects i3 environment correctly
- [ ] Only shows for critical/emergency alerts
- [ ] Includes action buttons
- [ ] Runs non-blocking
- [ ] Tests pass

---

## Task 8: Implement Stderr Fallback

**Status**: `[x]`

### Description

Implement final fallback that prints to stderr.

### Context

When no graphical notification method is available (e.g., headless server, SSH session), we fall back to printing to stderr.

### Implementation

**File**: `src/monitor/notifiers/stderr.rs`

```rust
use std::io::{self, Write};

use crate::monitor::notifier::{Notifier, format_alert_body, format_alert_title};
use crate::monitor::types::{AlertLevel, DiskStatus, NotificationUrgency};
use crate::error::Result;

pub struct StderrNotifier;

impl StderrNotifier {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StderrNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifier for StderrNotifier {
    fn name(&self) -> &'static str {
        "stderr"
    }

    fn is_available(&self) -> bool {
        true // Always available
    }

    fn send_alert(&self, level: AlertLevel, status: &DiskStatus) -> Result<()> {
        let title = format_alert_title(level);
        let body = format_alert_body(status);
        let urgency = level.urgency();

        self.send(title, &body, urgency)
    }

    fn send(&self, title: &str, body: &str, urgency: NotificationUrgency) -> Result<()> {
        let prefix = match urgency {
            NotificationUrgency::Critical => "[CRITICAL]",
            NotificationUrgency::Normal => "[WARNING]",
            NotificationUrgency::Low => "[INFO]",
        };

        let mut stderr = io::stderr().lock();
        writeln!(stderr, "\n{} {}", prefix, title)?;
        writeln!(stderr, "{}", "-".repeat(60))?;
        for line in body.lines() {
            writeln!(stderr, "  {}", line)?;
        }
        writeln!(stderr)?;

        Ok(())
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stderr_always_available() {
        let notifier = StderrNotifier::new();
        assert!(notifier.is_available());
    }

    #[test]
    fn test_stderr_send() {
        let notifier = StderrNotifier::new();
        let result = notifier.send("Test", "Test body", NotificationUrgency::Low);
        assert!(result.is_ok());
    }
}
```

### Acceptance Criteria

- [ ] Always available
- [ ] Formats output readably
- [ ] Indicates severity level
- [ ] Tests pass

---

## Task 9: Create Notifier Registry

**Status**: `[x]`

### Description

Create a registry that selects the best available notifier.

### Context

The registry tries notifiers in priority order and selects the first available one, or allows explicit selection via configuration.

### Implementation

**File**: `src/monitor/notifiers/mod.rs`

```rust
mod dbus;
mod notify_send;
mod i3nagbar;
mod stderr;

pub use dbus::DBusNotifier;
pub use notify_send::NotifySendNotifier;
pub use i3nagbar::I3NagbarNotifier;
pub use stderr::StderrNotifier;

use super::notifier::Notifier;
use super::types::NotificationBackend;

/// Create the best available notifier
pub fn create_notifier(preference: NotificationBackend) -> Box<dyn Notifier> {
    match preference {
        NotificationBackend::Auto => auto_select_notifier(),
        NotificationBackend::DBus => Box::new(DBusNotifier::new()),
        NotificationBackend::NotifySend => Box::new(NotifySendNotifier::new()),
        NotificationBackend::I3Nagbar => Box::new(I3NagbarNotifier::new()),
        NotificationBackend::Stderr => Box::new(StderrNotifier::new()),
    }
}

/// Auto-select the best available notifier
fn auto_select_notifier() -> Box<dyn Notifier> {
    // Try D-Bus first (most feature-rich)
    let dbus = DBusNotifier::new();
    if dbus.is_available() {
        tracing::debug!("Selected D-Bus notifier");
        return Box::new(dbus);
    }

    // Try notify-send
    let notify_send = NotifySendNotifier::new();
    if notify_send.is_available() {
        tracing::debug!("Selected notify-send notifier");
        return Box::new(notify_send);
    }

    // Fall back to stderr
    tracing::debug!("Falling back to stderr notifier");
    Box::new(StderrNotifier::new())
}

/// Get the i3-nagbar notifier if available (for critical alerts)
pub fn get_i3_notifier() -> Option<Box<dyn Notifier>> {
    let i3 = I3NagbarNotifier::new();
    if i3.is_available() {
        Some(Box::new(i3))
    } else {
        None
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_select_returns_notifier() {
        let notifier = create_notifier(NotificationBackend::Auto);
        assert!(!notifier.name().is_empty());
    }

    #[test]
    fn test_explicit_stderr_selection() {
        let notifier = create_notifier(NotificationBackend::Stderr);
        assert_eq!(notifier.name(), "stderr");
    }
}
```

### Acceptance Criteria

- [ ] Auto-selection prefers D-Bus > notify-send > stderr
- [ ] Explicit selection works
- [ ] i3-nagbar available separately for critical alerts
- [ ] Tests pass

---

## Task 10: Implement Monitor Loop

**Status**: `[x]`

### Description

Create the main monitoring loop that checks disk usage and sends alerts.

### Context

The monitor loop:
1. Checks disk usage on configured mount points
2. Compares against thresholds
3. Sends notifications for warnings/critical
4. Sleeps for the configured interval
5. Handles signals for clean shutdown

### Implementation

**File**: `src/monitor/service.rs`

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::disk::{check_disk_usage, check_all_mount_points, check_mount_points};
use super::notifier::Notifier;
use super::notifiers::{create_notifier, get_i3_notifier};
use super::types::{AlertLevel, DiskStatus, MonitorOptions};
use crate::error::Result;

pub struct MonitorService {
    options: MonitorOptions,
    notifier: Box<dyn Notifier>,
    i3_notifier: Option<Box<dyn Notifier>>,
    running: Arc<AtomicBool>,
    /// Track last alert level per mount point to avoid spam
    last_alerts: HashMap<PathBuf, AlertLevel>,
}

impl MonitorService {
    pub fn new(options: MonitorOptions) -> Self {
        let notifier = create_notifier(options.notification_backend);
        let i3_notifier = get_i3_notifier();

        tracing::info!("Using notification backend: {}", notifier.name());
        if i3_notifier.is_some() {
            tracing::info!("i3-nagbar available for critical alerts");
        }

        Self {
            options,
            notifier,
            i3_notifier,
            running: Arc::new(AtomicBool::new(true)),
            last_alerts: HashMap::new(),
        }
    }

    /// Get the running flag for signal handlers
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Run the monitoring loop
    pub fn run(&mut self) -> Result<()> {
        tracing::info!(
            "Starting monitor with {}s interval, warn={}%, critical={}%",
            self.options.interval.as_secs(),
            self.options.warn_threshold,
            self.options.critical_threshold
        );

        loop {
            let start = Instant::now();

            // Check disk usage
            self.check_and_notify()?;

            // Exit if one-shot mode
            if self.options.once {
                break;
            }

            // Check if we should stop
            if !self.running.load(Ordering::SeqCst) {
                tracing::info!("Monitor stopping");
                break;
            }

            // Sleep for remaining interval time
            let elapsed = start.elapsed();
            if elapsed < self.options.interval {
                let sleep_time = self.options.interval - elapsed;

                // Sleep in small chunks to check running flag
                let chunk = Duration::from_secs(1);
                let mut remaining = sleep_time;

                while remaining > Duration::ZERO && self.running.load(Ordering::SeqCst) {
                    let sleep = remaining.min(chunk);
                    thread::sleep(sleep);
                    remaining = remaining.saturating_sub(sleep);
                }
            }
        }

        Ok(())
    }

    /// Check disk usage and send notifications if needed
    fn check_and_notify(&mut self) -> Result<()> {
        let statuses = if self.options.mount_points.is_empty() {
            check_all_mount_points()?
        } else {
            check_mount_points(&self.options.mount_points)?
        };

        for status in statuses {
            let level = AlertLevel::from_percent(
                status.percent,
                self.options.warn_threshold,
                self.options.critical_threshold,
            );

            self.maybe_send_alert(level, &status)?;
        }

        Ok(())
    }

    /// Send alert if level changed or is critical
    fn maybe_send_alert(&mut self, level: AlertLevel, status: &DiskStatus) -> Result<()> {
        let last_level = self.last_alerts
            .get(&status.mount_point)
            .copied()
            .unwrap_or(AlertLevel::Normal);

        // Always notify on first critical/emergency
        // Re-notify if level increased
        // Don't notify if level decreased or stayed same (except emergency)
        let should_notify = match level {
            AlertLevel::Normal => false,
            AlertLevel::Emergency => true, // Always notify emergency
            _ => level > last_level,
        };

        if should_notify {
            tracing::info!(
                "Sending {} alert for {} ({}%)",
                format!("{:?}", level),
                status.mount_point.display(),
                status.percent as u32
            );

            // Send via primary notifier
            if let Err(e) = self.notifier.send_alert(level, status) {
                tracing::error!("Failed to send notification: {}", e);
            }

            // Also send via i3-nagbar for critical/emergency
            if level >= AlertLevel::Critical {
                if let Some(ref i3) = self.i3_notifier {
                    if let Err(e) = i3.send_alert(level, status) {
                        tracing::warn!("Failed to send i3-nagbar notification: {}", e);
                    }
                }
            }
        }

        // Update last alert level
        self.last_alerts.insert(status.mount_point.clone(), level);

        Ok(())
    }

    /// Stop the monitor
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_service_creation() {
        let options = MonitorOptions::default();
        let service = MonitorService::new(options);

        assert!(service.running.load(Ordering::SeqCst));
    }

    #[test]
    fn test_monitor_once_mode() {
        let options = MonitorOptions {
            once: true,
            ..Default::default()
        };
        let mut service = MonitorService::new(options);

        // Should complete without hanging
        let result = service.run();
        assert!(result.is_ok());
    }

    #[test]
    fn test_monitor_stop() {
        let options = MonitorOptions::default();
        let service = MonitorService::new(options);

        service.stop();
        assert!(!service.running.load(Ordering::SeqCst));
    }
}
```

### Acceptance Criteria

- [ ] Checks disk usage at configured interval
- [ ] Sends alerts when thresholds exceeded
- [ ] Doesn't spam repeated alerts
- [ ] Re-alerts when level increases
- [ ] One-shot mode works
- [ ] Can be stopped gracefully
- [ ] Tests pass

---

## Task 11: Implement Signal Handling

**Status**: `[x]`

### Description

Set up signal handlers for SIGHUP (reload config) and SIGTERM/SIGINT (shutdown).

### Context

Daemon processes need to handle Unix signals:
- SIGTERM/SIGINT: Clean shutdown
- SIGHUP: Reload configuration (convention for daemons)

### Implementation

**File**: `src/monitor/signals.rs`

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nix::sys::signal::{self, Signal, SigHandler};

static mut RUNNING: Option<Arc<AtomicBool>> = None;
static mut RELOAD: Option<Arc<AtomicBool>> = None;

/// Install signal handlers
pub fn install_signal_handlers(
    running: Arc<AtomicBool>,
    reload: Arc<AtomicBool>,
) -> nix::Result<()> {
    unsafe {
        RUNNING = Some(running);
        RELOAD = Some(reload);

        // Handle SIGTERM and SIGINT for shutdown
        signal::signal(Signal::SIGTERM, SigHandler::Handler(handle_shutdown))?;
        signal::signal(Signal::SIGINT, SigHandler::Handler(handle_shutdown))?;

        // Handle SIGHUP for reload
        signal::signal(Signal::SIGHUP, SigHandler::Handler(handle_reload))?;
    }

    Ok(())
}

extern "C" fn handle_shutdown(_: i32) {
    unsafe {
        if let Some(ref running) = RUNNING {
            running.store(false, Ordering::SeqCst);
        }
    }
}

extern "C" fn handle_reload(_: i32) {
    unsafe {
        if let Some(ref reload) = RELOAD {
            reload.store(true, Ordering::SeqCst);
        }
    }
}

/// Check and clear reload flag
pub fn check_reload(reload: &AtomicBool) -> bool {
    reload.swap(false, Ordering::SeqCst)
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_signal_handlers() {
        let running = Arc::new(AtomicBool::new(true));
        let reload = Arc::new(AtomicBool::new(false));

        let result = install_signal_handlers(running.clone(), reload.clone());
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_reload() {
        let reload = AtomicBool::new(true);

        assert!(check_reload(&reload));
        assert!(!check_reload(&reload)); // Should be cleared
    }
}
```

### Acceptance Criteria

- [ ] SIGTERM/SIGINT trigger clean shutdown
- [ ] SIGHUP sets reload flag
- [ ] Signal handlers are safe
- [ ] Tests pass

---

## Task 12: Implement Daemon Mode

**Status**: `[x]`

### Description

Implement daemonization: fork, PID file, log redirection.

### Context

Daemon mode allows the monitor to run in the background. We need to:
- Fork and detach from terminal
- Write PID to file for management
- Redirect output to log file

### Implementation

**File**: `src/monitor/daemon.rs`

```rust
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

use nix::unistd::{fork, setsid, ForkResult, Pid, close, dup2};
use nix::sys::stat::Mode;
use nix::fcntl::{open, OFlag};

use crate::error::Result;

/// Paths for daemon files
pub struct DaemonPaths {
    pub pid_file: PathBuf,
    pub log_file: PathBuf,
}

impl DaemonPaths {
    pub fn new() -> Self {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));

        let state_dir = std::env::var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .map(|h| h.join(".local/state"))
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
            });

        let log_dir = state_dir.join("rusty-sweeper");
        let _ = fs::create_dir_all(&log_dir);

        Self {
            pid_file: runtime_dir.join("rusty-sweeper.pid"),
            log_file: log_dir.join("monitor.log"),
        }
    }
}

impl Default for DaemonPaths {
    fn default() -> Self {
        Self::new()
    }
}

/// Daemonize the current process
pub fn daemonize(paths: &DaemonPaths) -> Result<()> {
    // Check if already running
    if let Some(pid) = read_pid_file(&paths.pid_file) {
        if is_process_running(pid) {
            return Err(crate::error::Error::AlreadyRunning(pid as u32));
        }
        // Stale PID file, remove it
        let _ = fs::remove_file(&paths.pid_file);
    }

    // First fork
    match unsafe { fork() }? {
        ForkResult::Parent { .. } => {
            // Parent exits
            std::process::exit(0);
        }
        ForkResult::Child => {
            // Continue as child
        }
    }

    // Create new session
    setsid()?;

    // Second fork to prevent acquiring a controlling terminal
    match unsafe { fork() }? {
        ForkResult::Parent { .. } => {
            std::process::exit(0);
        }
        ForkResult::Child => {
            // Continue as grandchild (daemon)
        }
    }

    // Change working directory to root
    std::env::set_current_dir("/")?;

    // Redirect standard file descriptors to log file / null
    redirect_stdio(paths)?;

    // Write PID file
    write_pid_file(&paths.pid_file)?;

    Ok(())
}

/// Redirect stdin/stdout/stderr
fn redirect_stdio(paths: &DaemonPaths) -> Result<()> {
    // Open /dev/null for stdin
    let dev_null = open("/dev/null", OFlag::O_RDWR, Mode::empty())?;

    // Open log file for stdout/stderr
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)?;
    let log_fd = log_file.as_raw_fd();

    // Redirect stdin to /dev/null
    dup2(dev_null, 0)?;

    // Redirect stdout and stderr to log file
    dup2(log_fd, 1)?;
    dup2(log_fd, 2)?;

    // Close original file descriptors
    if dev_null > 2 {
        close(dev_null)?;
    }

    Ok(())
}

/// Write current PID to file
fn write_pid_file(path: &PathBuf) -> Result<()> {
    let pid = std::process::id();
    let mut file = File::create(path)?;
    writeln!(file, "{}", pid)?;
    Ok(())
}

/// Read PID from file
fn read_pid_file(path: &PathBuf) -> Option<i32> {
    fs::read_to_string(path)
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// Check if a process is running
fn is_process_running(pid: i32) -> bool {
    // Try to send signal 0 (doesn't actually send a signal, just checks)
    nix::sys::signal::kill(Pid::from_raw(pid), None).is_ok()
}

/// Remove PID file on shutdown
pub fn cleanup_pid_file(paths: &DaemonPaths) {
    let _ = fs::remove_file(&paths.pid_file);
}

/// Stop a running daemon
pub fn stop_daemon(paths: &DaemonPaths) -> Result<bool> {
    if let Some(pid) = read_pid_file(&paths.pid_file) {
        if is_process_running(pid) {
            nix::sys::signal::kill(
                Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGTERM
            )?;

            // Wait a bit and check if it stopped
            std::thread::sleep(std::time::Duration::from_secs(1));

            if !is_process_running(pid) {
                let _ = fs::remove_file(&paths.pid_file);
                return Ok(true);
            }

            // Force kill
            nix::sys::signal::kill(
                Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGKILL
            )?;
            let _ = fs::remove_file(&paths.pid_file);
            return Ok(true);
        }
    }

    Ok(false)
}

/// Get status of daemon
pub fn daemon_status(paths: &DaemonPaths) -> Option<u32> {
    read_pid_file(&paths.pid_file)
        .filter(|&pid| is_process_running(pid))
        .map(|pid| pid as u32)
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_daemon_paths() {
        let paths = DaemonPaths::new();
        assert!(paths.pid_file.to_string_lossy().contains("rusty-sweeper"));
        assert!(paths.log_file.to_string_lossy().contains("monitor.log"));
    }

    #[test]
    fn test_pid_file_operations() {
        let temp = tempdir().unwrap();
        let pid_path = temp.path().join("test.pid");

        // Write PID
        write_pid_file(&pid_path).unwrap();

        // Read PID
        let pid = read_pid_file(&pid_path);
        assert!(pid.is_some());
        assert_eq!(pid.unwrap(), std::process::id() as i32);
    }

    #[test]
    fn test_is_process_running() {
        // Current process should be running
        let pid = std::process::id() as i32;
        assert!(is_process_running(pid));

        // Non-existent process should not be running
        assert!(!is_process_running(99999));
    }
}
```

### Acceptance Criteria

- [ ] Double-fork daemonization works
- [ ] PID file is created and managed
- [ ] Prevents duplicate daemons
- [ ] Log file is created and written to
- [ ] Can stop running daemon
- [ ] Tests pass

---

## Task 13: Implement CLI Subcommand

**Status**: `[ ]`

### Description

Wire up the `monitor` CLI subcommand with all options.

### Context

The CLI needs to support:
- `rusty-sweeper monitor` - start foreground monitoring
- `rusty-sweeper monitor --daemon` - start as daemon
- `rusty-sweeper monitor --once` - check once and exit
- `rusty-sweeper monitor --stop` - stop running daemon
- `rusty-sweeper monitor --status` - show daemon status

### Implementation

**File**: `src/commands/monitor.rs`

```rust
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use clap::Args;

use crate::monitor::daemon::{DaemonPaths, daemonize, cleanup_pid_file, stop_daemon, daemon_status};
use crate::monitor::service::MonitorService;
use crate::monitor::signals::install_signal_handlers;
use crate::monitor::types::{MonitorOptions, NotificationBackend};
use crate::error::Result;

#[derive(Args, Debug)]
pub struct MonitorArgs {
    /// Run as background daemon
    #[arg(short, long)]
    pub daemon: bool,

    /// Check interval in seconds
    #[arg(short, long, default_value = "300", value_name = "SECS")]
    pub interval: u64,

    /// Warning threshold percentage
    #[arg(short, long, default_value = "80", value_name = "PERCENT")]
    pub warn: u8,

    /// Critical threshold percentage
    #[arg(short = 'C', long, default_value = "90", value_name = "PERCENT")]
    pub critical: u8,

    /// Mount point to monitor (can be specified multiple times)
    #[arg(short, long, value_name = "PATH")]
    pub mount: Vec<PathBuf>,

    /// Check once and exit
    #[arg(long)]
    pub once: bool,

    /// Notification backend: auto, dbus, notify-send, stderr
    #[arg(long, default_value = "auto")]
    pub notify: String,

    /// Stop running daemon
    #[arg(long)]
    pub stop: bool,

    /// Show daemon status
    #[arg(long)]
    pub status: bool,
}

pub fn run(args: MonitorArgs) -> Result<()> {
    let paths = DaemonPaths::new();

    // Handle --stop
    if args.stop {
        return handle_stop(&paths);
    }

    // Handle --status
    if args.status {
        return handle_status(&paths);
    }

    // Validate thresholds
    if args.warn >= args.critical {
        return Err(crate::error::Error::Config(
            "warn threshold must be less than critical threshold".to_string()
        ));
    }
    if args.critical > 100 {
        return Err(crate::error::Error::Config(
            "critical threshold must be <= 100".to_string()
        ));
    }

    // Parse notification backend
    let backend = match args.notify.to_lowercase().as_str() {
        "auto" => NotificationBackend::Auto,
        "dbus" => NotificationBackend::DBus,
        "notify-send" => NotificationBackend::NotifySend,
        "i3-nagbar" | "i3nagbar" => NotificationBackend::I3Nagbar,
        "stderr" => NotificationBackend::Stderr,
        _ => {
            return Err(crate::error::Error::Config(
                format!("unknown notification backend: {}", args.notify)
            ));
        }
    };

    // Build options
    let options = MonitorOptions {
        interval: Duration::from_secs(args.interval),
        warn_threshold: args.warn,
        critical_threshold: args.critical,
        mount_points: args.mount,
        daemon: args.daemon,
        once: args.once,
        notification_backend: backend,
    };

    // Daemonize if requested
    if args.daemon {
        println!("Starting monitor daemon...");
        daemonize(&paths)?;
    }

    // Create service
    let mut service = MonitorService::new(options);

    // Install signal handlers
    let running = service.running_flag();
    let reload = Arc::new(AtomicBool::new(false));

    if let Err(e) = install_signal_handlers(running, reload) {
        tracing::warn!("Failed to install signal handlers: {}", e);
    }

    // Run the monitor
    let result = service.run();

    // Cleanup
    if args.daemon {
        cleanup_pid_file(&paths);
    }

    result
}

fn handle_stop(paths: &DaemonPaths) -> Result<()> {
    match stop_daemon(paths)? {
        true => {
            println!("Monitor daemon stopped");
            Ok(())
        }
        false => {
            println!("No monitor daemon running");
            Ok(())
        }
    }
}

fn handle_status(paths: &DaemonPaths) -> Result<()> {
    match daemon_status(paths) {
        Some(pid) => {
            println!("Monitor daemon running (PID: {})", pid);
        }
        None => {
            println!("Monitor daemon not running");
        }
    }
    Ok(())
}
```

**Update** `src/main.rs`:

```rust
Commands::Monitor(args) => commands::monitor::run(args),
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_validation() {
        let args = MonitorArgs {
            warn: 90,
            critical: 80, // Invalid: warn >= critical
            ..Default::default()
        };

        let result = run(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_once_mode() {
        let args = MonitorArgs {
            once: true,
            ..Default::default()
        };

        // Should complete without hanging
        let result = run(args);
        assert!(result.is_ok());
    }
}
```

### Acceptance Criteria

- [ ] `--daemon` starts background daemon
- [ ] `--once` checks and exits
- [ ] `--stop` stops running daemon
- [ ] `--status` shows daemon status
- [ ] Threshold validation works
- [ ] All CLI options are handled
- [ ] Tests pass

---

## Task 14: Create Systemd Service File

**Status**: `[ ]`

### Description

Create a systemd user service file for automatic startup.

### Context

Users can install this service to have the monitor start automatically on login.

### Implementation

**File**: `dist/rusty-sweeper-monitor.service`

```ini
[Unit]
Description=Rusty Sweeper Disk Monitor
Documentation=https://github.com/user/rusty-sweeper
After=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/bin/rusty-sweeper monitor
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=10

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true

[Install]
WantedBy=default.target
```

**File**: `dist/install-service.sh`

```bash
#!/bin/bash
set -e

SERVICE_FILE="rusty-sweeper-monitor.service"
USER_SERVICE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"

# Create directory if needed
mkdir -p "$USER_SERVICE_DIR"

# Copy service file
cp "$SERVICE_FILE" "$USER_SERVICE_DIR/"

# Reload systemd
systemctl --user daemon-reload

echo "Service installed. To enable:"
echo "  systemctl --user enable rusty-sweeper-monitor"
echo "  systemctl --user start rusty-sweeper-monitor"
```

### Acceptance Criteria

- [ ] Service file is valid systemd syntax
- [ ] Service starts successfully
- [ ] Restart on failure works
- [ ] Security hardening applied
- [ ] Install script works

---

## Task 15: Add Module Structure

**Status**: `[ ]`

### Description

Create the module hierarchy for the monitor code.

### Context

Organize all monitor code into a clean module structure.

### Implementation

**File structure**:
```
src/monitor/
├── mod.rs
├── types.rs
├── disk.rs
├── notifier.rs
├── notifiers/
│   ├── mod.rs
│   ├── dbus.rs
│   ├── notify_send.rs
│   ├── i3nagbar.rs
│   └── stderr.rs
├── service.rs
├── signals.rs
└── daemon.rs
```

**File**: `src/monitor/mod.rs`

```rust
pub mod types;
pub mod disk;
pub mod notifier;
pub mod notifiers;
pub mod service;
pub mod signals;
pub mod daemon;

pub use types::{DiskStatus, AlertLevel, MonitorOptions};
pub use service::MonitorService;
```

**File**: `src/lib.rs` (update)

```rust
pub mod config;
pub mod error;
pub mod scanner;
pub mod cleaner;
pub mod tui;
pub mod monitor;
```

### Acceptance Criteria

- [ ] All modules compile
- [ ] Public exports are correct
- [ ] No circular dependencies

---

## Task 16: Update Error Types

**Status**: `[ ]`

### Description

Add monitor-specific error variants to the error module.

### Context

The monitor has some unique error conditions that need specific handling.

### Implementation

**File**: `src/error.rs` (update)

```rust
#[derive(Error, Debug)]
pub enum Error {
    // ... existing variants ...

    #[error("Monitor already running (PID: {0})")]
    AlreadyRunning(u32),

    #[error("Notification backend not found: {0}")]
    NotifierNotFound(String),

    #[error("Command failed: {0}")]
    Command(String),

    #[error("System call failed: {0}")]
    Nix(#[from] nix::Error),

    #[error("Notification failed: {0}")]
    Notification(#[from] notify_rust::error::Error),
}
```

### Acceptance Criteria

- [ ] All new error types defined
- [ ] Error conversions work
- [ ] Tests pass

---

## Task 17: Integration Tests

**Status**: `[ ]`

### Description

Create integration tests for the monitor functionality.

### Context

Test the complete monitor workflow including disk checking and notifications.

### Implementation

**File**: `tests/monitor_integration.rs`

```rust
use assert_cmd::Command;
use predicates::prelude::*;

fn rusty_sweeper() -> Command {
    Command::cargo_bin("rusty-sweeper").unwrap()
}

#[test]
fn test_monitor_once() {
    rusty_sweeper()
        .args(["monitor", "--once"])
        .assert()
        .success();
}

#[test]
fn test_monitor_help() {
    rusty_sweeper()
        .args(["monitor", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--daemon"))
        .stdout(predicate::str::contains("--interval"))
        .stdout(predicate::str::contains("--warn"))
        .stdout(predicate::str::contains("--critical"));
}

#[test]
fn test_monitor_invalid_thresholds() {
    rusty_sweeper()
        .args(["monitor", "--once", "--warn", "90", "--critical", "80"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("threshold"));
}

#[test]
fn test_monitor_custom_mount() {
    rusty_sweeper()
        .args(["monitor", "--once", "--mount", "/"])
        .assert()
        .success();
}

#[test]
fn test_monitor_status_not_running() {
    rusty_sweeper()
        .args(["monitor", "--status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not running"));
}

#[test]
fn test_monitor_stderr_backend() {
    rusty_sweeper()
        .args(["monitor", "--once", "--notify", "stderr", "--warn", "1"])
        .assert()
        .success();
}
```

### Acceptance Criteria

- [ ] One-shot mode tests pass
- [ ] Invalid arguments are rejected
- [ ] Custom mount point works
- [ ] Status check works
- [ ] Different backends work

---

## Summary

| Task | Description | Status |
|------|-------------|--------|
| 1 | Add monitor dependencies | `[ ]` |
| 2 | Define data structures | `[ ]` |
| 3 | Implement disk usage checker | `[ ]` |
| 4 | Define notifier trait | `[ ]` |
| 5 | Implement D-Bus notifier | `[ ]` |
| 6 | Implement notify-send fallback | `[ ]` |
| 7 | Implement i3-nagbar notifier | `[ ]` |
| 8 | Implement stderr fallback | `[ ]` |
| 9 | Create notifier registry | `[ ]` |
| 10 | Implement monitor loop | `[ ]` |
| 11 | Implement signal handling | `[ ]` |
| 12 | Implement daemon mode | `[ ]` |
| 13 | Implement CLI subcommand | `[ ]` |
| 14 | Create systemd service file | `[ ]` |
| 15 | Add module structure | `[ ]` |
| 16 | Update error types | `[ ]` |
| 17 | Integration tests | `[ ]` |

**Total: 17 tasks**

---

## Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
notify-rust = "4"
nix = { version = "0.29", features = ["fs", "signal", "process"] }
dirs = "5"  # Already present from Phase 1
humansize = "2"  # Already present from Phase 3
```

---

## Definition of Done

Phase 5 is complete when:

1. `cargo build` succeeds with no warnings
2. `cargo test` passes all unit and integration tests
3. `cargo clippy` reports no warnings
4. `rusty-sweeper monitor --once` checks disk and exits
5. `rusty-sweeper monitor --daemon` starts background daemon
6. `rusty-sweeper monitor --stop` stops the daemon
7. `rusty-sweeper monitor --status` shows daemon status
8. Desktop notifications appear when thresholds exceeded
9. Stderr fallback works when no display available
10. Systemd service can be installed and started
11. SIGTERM/SIGINT cleanly shut down the daemon
12. PID file prevents duplicate daemons
