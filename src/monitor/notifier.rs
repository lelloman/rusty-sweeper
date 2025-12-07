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

    #[test]
    fn test_format_alert_title_normal() {
        assert!(format_alert_title(AlertLevel::Normal).contains("Normal"));
    }
}
