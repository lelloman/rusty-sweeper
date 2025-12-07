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
            total: 1024 * 1024 * 1024 * 100, // 100 GiB
            used: 1024 * 1024 * 1024 * 80,   // 80 GiB
            available: 1024 * 1024 * 1024 * 20, // 20 GiB
            percent: 80.0,
        };

        assert!(status.total_human().contains("100"));
        assert!(status.used_human().contains("80"));
    }

    #[test]
    fn test_default_monitor_options() {
        let options = MonitorOptions::default();
        assert_eq!(options.interval, Duration::from_secs(300));
        assert_eq!(options.warn_threshold, 80);
        assert_eq!(options.critical_threshold, 90);
        assert!(!options.daemon);
        assert!(!options.once);
    }

    #[test]
    fn test_urgency_mapping() {
        assert_eq!(AlertLevel::Normal.urgency(), NotificationUrgency::Low);
        assert_eq!(AlertLevel::Warning.urgency(), NotificationUrgency::Normal);
        assert_eq!(AlertLevel::Critical.urgency(), NotificationUrgency::Critical);
        assert_eq!(AlertLevel::Emergency.urgency(), NotificationUrgency::Critical);
    }
}
