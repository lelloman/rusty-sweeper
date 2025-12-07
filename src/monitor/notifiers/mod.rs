mod dbus;
mod i3nagbar;
mod notify_send;
mod stderr;

pub use dbus::DBusNotifier;
pub use i3nagbar::I3NagbarNotifier;
pub use notify_send::NotifySendNotifier;
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
}
