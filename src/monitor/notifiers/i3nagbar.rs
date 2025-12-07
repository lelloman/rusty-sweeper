use std::process::Command;

use crate::error::{Result, SweeperError};
use crate::monitor::notifier::{format_alert_body, format_alert_title, Notifier};
use crate::monitor::types::{AlertLevel, DiskStatus, NotificationUrgency};

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
        // Check if running under i3 or sway
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
            .arg("-t")
            .arg(bar_type)
            .arg("-m")
            .arg(&message)
            .arg("-b")
            .arg("Open TUI")
            .arg("rusty-sweeper tui")
            .arg("-b")
            .arg("Dismiss")
            .arg("true")
            .spawn()
            .map_err(|e| SweeperError::Other(format!("Failed to spawn i3-nagbar: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i3nagbar_name() {
        let notifier = I3NagbarNotifier::new();
        assert_eq!(notifier.name(), "i3-nagbar");
    }

    #[test]
    fn test_i3nagbar_availability() {
        let notifier = I3NagbarNotifier::new();

        // Should return false unless running in i3
        let available = notifier.is_available();

        // Verify it matches environment
        let expected =
            std::env::var("I3SOCK").is_ok() || std::env::var("SWAYSOCK").is_ok();
        assert_eq!(available, expected);
    }

    #[test]
    fn test_i3nagbar_skips_non_critical() {
        let notifier = I3NagbarNotifier::new();

        let status = DiskStatus {
            mount_point: std::path::PathBuf::from("/"),
            device: None,
            total: 100,
            used: 80,
            available: 20,
            percent: 80.0,
        };

        // Warning level should be skipped (returns Ok without doing anything)
        let result = notifier.send_alert(AlertLevel::Warning, &status);
        assert!(result.is_ok());
    }
}
