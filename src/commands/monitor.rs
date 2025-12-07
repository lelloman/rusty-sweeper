use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use crate::cli::MonitorArgs;
use crate::error::{Result, SweeperError};
use crate::monitor::{
    cleanup_pid_file, daemon_status, daemonize, install_signal_handlers, stop_daemon,
    DaemonPaths, MonitorOptions, MonitorService, NotificationBackend,
};

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
        return Err(SweeperError::Other(
            "Warning threshold must be less than critical threshold".to_string(),
        ));
    }

    if args.warn > 100 || args.critical > 100 {
        return Err(SweeperError::Other(
            "Thresholds must be between 0 and 100".to_string(),
        ));
    }

    // Parse notification backend
    let notification_backend = parse_backend(&args.notify)?;

    // Build monitor options
    let options = MonitorOptions {
        interval: Duration::from_secs(args.interval),
        warn_threshold: args.warn,
        critical_threshold: args.critical,
        mount_points: args.mount,
        daemon: args.daemon,
        once: args.once,
        notification_backend,
    };

    // Daemonize if requested
    if args.daemon {
        daemonize(&paths)?;
        tracing::info!("Daemonized successfully");
    }

    // Set up signal handlers
    let running = Arc::new(AtomicBool::new(true));
    let reload = Arc::new(AtomicBool::new(false));

    install_signal_handlers(Arc::clone(&running), Arc::clone(&reload))?;

    // Create and run the monitor service (pass running flag so Ctrl+C works)
    let mut service = MonitorService::new(options, Arc::clone(&running));

    // Run the monitoring loop
    let result = service.run();

    // Clean up
    if args.daemon {
        cleanup_pid_file(&paths);
    }

    result
}

fn handle_stop(paths: &DaemonPaths) -> Result<()> {
    match stop_daemon(paths) {
        Ok(true) => {
            println!("Monitor daemon stopped");
            Ok(())
        }
        Ok(false) => {
            println!("No monitor daemon running");
            Ok(())
        }
        Err(e) => Err(SweeperError::Other(format!(
            "Failed to stop daemon: {}",
            e
        ))),
    }
}

fn handle_status(paths: &DaemonPaths) -> Result<()> {
    match daemon_status(paths) {
        Some(pid) => {
            println!("Monitor daemon running (PID: {})", pid);
            println!("PID file: {}", paths.pid_file.display());
            println!("Log file: {}", paths.log_file.display());
        }
        None => {
            println!("Monitor daemon not running");
        }
    }
    Ok(())
}

fn parse_backend(name: &str) -> Result<NotificationBackend> {
    match name.to_lowercase().as_str() {
        "auto" => Ok(NotificationBackend::Auto),
        "dbus" => Ok(NotificationBackend::DBus),
        "notify-send" => Ok(NotificationBackend::NotifySend),
        "stderr" => Ok(NotificationBackend::Stderr),
        _ => Err(SweeperError::Other(format!(
            "Unknown notification backend: {}. Valid options: auto, dbus, notify-send, stderr",
            name
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_backend_auto() {
        assert!(matches!(parse_backend("auto"), Ok(NotificationBackend::Auto)));
        assert!(matches!(parse_backend("AUTO"), Ok(NotificationBackend::Auto)));
    }

    #[test]
    fn test_parse_backend_dbus() {
        assert!(matches!(parse_backend("dbus"), Ok(NotificationBackend::DBus)));
        assert!(matches!(parse_backend("DBus"), Ok(NotificationBackend::DBus)));
    }

    #[test]
    fn test_parse_backend_notify_send() {
        assert!(matches!(
            parse_backend("notify-send"),
            Ok(NotificationBackend::NotifySend)
        ));
    }

    #[test]
    fn test_parse_backend_stderr() {
        assert!(matches!(
            parse_backend("stderr"),
            Ok(NotificationBackend::Stderr)
        ));
    }

    #[test]
    fn test_parse_backend_invalid() {
        assert!(parse_backend("invalid").is_err());
    }

    #[test]
    fn test_handle_status_not_running() {
        let paths = DaemonPaths {
            pid_file: std::path::PathBuf::from("/tmp/nonexistent.pid"),
            log_file: std::path::PathBuf::from("/tmp/nonexistent.log"),
        };
        assert!(handle_status(&paths).is_ok());
    }

    #[test]
    fn test_handle_stop_not_running() {
        let paths = DaemonPaths {
            pid_file: std::path::PathBuf::from("/tmp/nonexistent.pid"),
            log_file: std::path::PathBuf::from("/tmp/nonexistent.log"),
        };
        assert!(handle_stop(&paths).is_ok());
    }
}
