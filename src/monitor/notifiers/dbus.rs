use notify_rust::{Notification, Timeout, Urgency};

use crate::error::Result;
use crate::monitor::notifier::{format_alert_body, format_alert_title, Notifier};
use crate::monitor::types::{AlertLevel, DiskStatus, NotificationUrgency};

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
        // Check if we have a display server available
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
            DBusNotifier::map_urgency(NotificationUrgency::Normal),
            Urgency::Normal
        ));
        assert!(matches!(
            DBusNotifier::map_urgency(NotificationUrgency::Critical),
            Urgency::Critical
        ));
    }

    #[test]
    fn test_dbus_availability_check() {
        let notifier = DBusNotifier::new();
        // Just check it doesn't panic
        let _ = notifier.is_available();
    }
}
