use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::disk::{check_all_mount_points, check_mount_points};
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
    pub fn new(options: MonitorOptions, running: Arc<AtomicBool>) -> Self {
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
            running,
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
        let last_level = self
            .last_alerts
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
                "Sending {:?} alert for {} ({}%)",
                level,
                status.mount_point.display(),
                status.percent as u32
            );

            // Only advance alert state after the primary notification (which
            // may itself contain an automatic fallback chain) is delivered.
            let delivered = match self.notifier.send_alert(level, status) {
                Ok(()) => true,
                Err(e) => {
                    tracing::error!("Failed to send notification: {}", e);
                    false
                }
            };

            // Also send via i3-nagbar for critical/emergency
            if level >= AlertLevel::Critical {
                if let Some(ref i3) = self.i3_notifier {
                    if let Err(e) = i3.send_alert(level, status) {
                        tracing::warn!("Failed to send i3-nagbar notification: {}", e);
                    }
                }
            }

            if delivered {
                self.last_alerts.insert(status.mount_point.clone(), level);
            }

            return Ok(());
        }

        // Record normal/decreased levels even though they do not notify, so a
        // later increase can alert again.
        self.last_alerts.insert(status.mount_point.clone(), level);

        Ok(())
    }

    /// Stop the monitor
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SweeperError;
    use crate::monitor::types::NotificationUrgency;
    use std::sync::atomic::AtomicUsize;

    struct FailingNotifier {
        attempts: Arc<AtomicUsize>,
    }

    impl Notifier for FailingNotifier {
        fn name(&self) -> &'static str {
            "failing-test-notifier"
        }

        fn is_available(&self) -> bool {
            true
        }

        fn send_alert(&self, _level: AlertLevel, _status: &DiskStatus) -> Result<()> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            Err(SweeperError::Other("injected notification failure".into()))
        }

        fn send(&self, _title: &str, _body: &str, _urgency: NotificationUrgency) -> Result<()> {
            Err(SweeperError::Other("injected notification failure".into()))
        }
    }

    #[test]
    fn test_monitor_service_creation() {
        let options = MonitorOptions::default();
        let running = Arc::new(AtomicBool::new(true));
        let service = MonitorService::new(options, running);

        assert!(service.running.load(Ordering::SeqCst));
    }

    #[test]
    fn test_monitor_once_mode() {
        let options = MonitorOptions {
            once: true,
            ..Default::default()
        };
        let running = Arc::new(AtomicBool::new(true));
        let mut service = MonitorService::new(options, running);

        // Should complete without hanging
        let result = service.run();
        assert!(result.is_ok());
    }

    #[test]
    fn test_monitor_stop() {
        let options = MonitorOptions::default();
        let running = Arc::new(AtomicBool::new(true));
        let service = MonitorService::new(options, running);

        service.stop();
        assert!(!service.running.load(Ordering::SeqCst));
    }

    #[test]
    fn test_running_flag_shared() {
        let options = MonitorOptions::default();
        let running = Arc::new(AtomicBool::new(true));
        let service = MonitorService::new(options, Arc::clone(&running));

        let flag = service.running_flag();
        assert!(flag.load(Ordering::SeqCst));

        // External flag controls the service
        running.store(false, Ordering::SeqCst);
        assert!(!flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_failed_alert_is_retried_at_same_level() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let mut service = MonitorService {
            options: MonitorOptions::default(),
            notifier: Box::new(FailingNotifier {
                attempts: Arc::clone(&attempts),
            }),
            i3_notifier: None,
            running: Arc::new(AtomicBool::new(true)),
            last_alerts: HashMap::new(),
        };
        let status = DiskStatus {
            mount_point: PathBuf::from("/test"),
            device: None,
            total: 100,
            used: 85,
            available: 15,
            percent: 85.0,
        };

        service
            .maybe_send_alert(AlertLevel::Warning, &status)
            .unwrap();
        service
            .maybe_send_alert(AlertLevel::Warning, &status)
            .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}
