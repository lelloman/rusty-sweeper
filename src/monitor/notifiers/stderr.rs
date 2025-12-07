use std::io::{self, Write};

use crate::error::Result;
use crate::monitor::notifier::{format_alert_body, format_alert_title, Notifier};
use crate::monitor::types::{AlertLevel, DiskStatus, NotificationUrgency};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stderr_name() {
        let notifier = StderrNotifier::new();
        assert_eq!(notifier.name(), "stderr");
    }

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

    #[test]
    fn test_stderr_send_alert() {
        let notifier = StderrNotifier::new();
        let status = DiskStatus {
            mount_point: std::path::PathBuf::from("/"),
            device: None,
            total: 100,
            used: 90,
            available: 10,
            percent: 90.0,
        };
        let result = notifier.send_alert(AlertLevel::Critical, &status);
        assert!(result.is_ok());
    }
}
