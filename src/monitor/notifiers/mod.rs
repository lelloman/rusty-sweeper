mod dbus;
mod i3nagbar;
mod notify_send;
mod stderr;

pub use dbus::DBusNotifier;
pub use i3nagbar::I3NagbarNotifier;
pub use notify_send::NotifySendNotifier;
pub use stderr::StderrNotifier;

use super::notifier::Notifier;
use super::types::{AlertLevel, DiskStatus, NotificationBackend, NotificationUrgency};
use crate::error::{Result, SweeperError};

/// A notifier chain used by automatic selection. Backends are tried in order
/// for every delivery, allowing transient D-Bus failures to fall through to a
/// command-based or stderr notification.
struct FallbackNotifier {
    notifiers: Vec<Box<dyn Notifier>>,
}

impl FallbackNotifier {
    fn new(notifiers: Vec<Box<dyn Notifier>>) -> Self {
        Self { notifiers }
    }

    fn try_send(&self, mut send: impl FnMut(&dyn Notifier) -> Result<()>) -> Result<()> {
        let mut last_error = None;
        for notifier in &self.notifiers {
            match send(notifier.as_ref()) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    tracing::warn!(backend = notifier.name(), %error, "Notification backend failed");
                    last_error = Some(error);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            SweeperError::Other("No notification backends are available".to_string())
        }))
    }
}

impl Notifier for FallbackNotifier {
    fn name(&self) -> &'static str {
        "automatic fallback chain"
    }

    fn is_available(&self) -> bool {
        !self.notifiers.is_empty()
    }

    fn send_alert(&self, level: AlertLevel, status: &DiskStatus) -> Result<()> {
        self.try_send(|notifier| notifier.send_alert(level, status))
    }

    fn send(&self, title: &str, body: &str, urgency: NotificationUrgency) -> Result<()> {
        self.try_send(|notifier| notifier.send(title, body, urgency))
    }
}

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
    let mut notifiers: Vec<Box<dyn Notifier>> = Vec::new();

    // Try D-Bus first (most feature-rich).
    let dbus = DBusNotifier::new();
    if dbus.is_available() {
        notifiers.push(Box::new(dbus));
    }

    // Then try notify-send.
    let notify_send = NotifySendNotifier::new();
    if notify_send.is_available() {
        notifiers.push(Box::new(notify_send));
    }

    // stderr is always the final fallback.
    notifiers.push(Box::new(StderrNotifier::new()));
    Box::new(FallbackNotifier::new(notifiers))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct TestNotifier {
        attempts: Arc<AtomicUsize>,
        fail: bool,
    }

    impl Notifier for TestNotifier {
        fn name(&self) -> &'static str {
            "test"
        }

        fn is_available(&self) -> bool {
            true
        }

        fn send_alert(&self, _level: AlertLevel, _status: &DiskStatus) -> Result<()> {
            unreachable!()
        }

        fn send(&self, _title: &str, _body: &str, _urgency: NotificationUrgency) -> Result<()> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(SweeperError::Other("injected failure".into()))
            } else {
                Ok(())
            }
        }
    }

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

    #[test]
    fn test_explicit_dbus_selection() {
        let notifier = create_notifier(NotificationBackend::DBus);
        assert_eq!(notifier.name(), "D-Bus");
    }

    #[test]
    fn test_get_i3_notifier() {
        // Should return None unless running in i3
        let notifier = get_i3_notifier();
        let expected = std::env::var("I3SOCK").is_ok() || std::env::var("SWAYSOCK").is_ok();
        assert_eq!(notifier.is_some(), expected);
    }

    #[test]
    fn test_fallback_notifier_tries_next_backend_after_failure() {
        let first_attempts = Arc::new(AtomicUsize::new(0));
        let second_attempts = Arc::new(AtomicUsize::new(0));
        let notifier = FallbackNotifier::new(vec![
            Box::new(TestNotifier {
                attempts: Arc::clone(&first_attempts),
                fail: true,
            }),
            Box::new(TestNotifier {
                attempts: Arc::clone(&second_attempts),
                fail: false,
            }),
        ]);

        notifier
            .send("title", "body", NotificationUrgency::Normal)
            .unwrap();

        assert_eq!(first_attempts.load(Ordering::SeqCst), 1);
        assert_eq!(second_attempts.load(Ordering::SeqCst), 1);
    }
}
