use std::process::Command;

use crate::error::{Result, SweeperError};
use crate::monitor::notifier::{format_alert_body, format_alert_title, Notifier};
use crate::monitor::types::{AlertLevel, DiskStatus, NotificationUrgency};

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
        let binary =
            Self::find_binary().ok_or_else(|| SweeperError::NotFound("notify-send".into()))?;

        let urgency_str = match urgency {
            NotificationUrgency::Low => "low",
            NotificationUrgency::Normal => "normal",
            NotificationUrgency::Critical => "critical",
        };

        let mut cmd = Command::new(binary);
        cmd.arg("--urgency")
            .arg(urgency_str)
            .arg("--app-name")
            .arg("Rusty Sweeper")
            .arg("--icon")
            .arg("drive-harddisk")
            .arg(title)
            .arg(body);

        // Critical notifications should be persistent
        if urgency == NotificationUrgency::Critical {
            cmd.arg("--expire-time=0");
        }

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SweeperError::Command(format!(
                "notify-send failed: {}",
                stderr
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notify_send_name() {
        let notifier = NotifySendNotifier::new();
        assert_eq!(notifier.name(), "notify-send");
    }

    #[test]
    fn test_notify_send_availability() {
        let notifier = NotifySendNotifier::new();
        // Just test that availability check doesn't panic
        let _ = notifier.is_available();
    }

    #[test]
    fn test_find_binary() {
        // This may or may not find notify-send depending on the system
        let _ = NotifySendNotifier::find_binary();
    }
}
