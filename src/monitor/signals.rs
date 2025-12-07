use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nix::sys::signal::{self, SigHandler, Signal};

static mut RUNNING: Option<Arc<AtomicBool>> = None;
static mut RELOAD: Option<Arc<AtomicBool>> = None;

/// Install signal handlers for graceful shutdown
///
/// # Safety
/// This function modifies global static variables and installs signal handlers.
/// It should only be called once, before starting the monitor loop.
pub fn install_signal_handlers(
    running: Arc<AtomicBool>,
    reload: Arc<AtomicBool>,
) -> nix::Result<()> {
    unsafe {
        RUNNING = Some(running);
        RELOAD = Some(reload);

        // Handle SIGTERM and SIGINT for shutdown
        signal::signal(Signal::SIGTERM, SigHandler::Handler(handle_shutdown))?;
        signal::signal(Signal::SIGINT, SigHandler::Handler(handle_shutdown))?;

        // Handle SIGHUP for reload
        signal::signal(Signal::SIGHUP, SigHandler::Handler(handle_reload))?;
    }

    Ok(())
}

extern "C" fn handle_shutdown(_: i32) {
    unsafe {
        if let Some(ref running) = RUNNING {
            running.store(false, Ordering::SeqCst);
        }
    }
}

extern "C" fn handle_reload(_: i32) {
    unsafe {
        if let Some(ref reload) = RELOAD {
            reload.store(true, Ordering::SeqCst);
        }
    }
}

/// Check and clear reload flag
pub fn check_reload(reload: &AtomicBool) -> bool {
    reload.swap(false, Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_signal_handlers() {
        let running = Arc::new(AtomicBool::new(true));
        let reload = Arc::new(AtomicBool::new(false));

        let result = install_signal_handlers(running.clone(), reload.clone());
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_reload_clears_flag() {
        let reload = AtomicBool::new(true);

        assert!(check_reload(&reload));
        assert!(!check_reload(&reload)); // Should be cleared
    }

    #[test]
    fn test_check_reload_false() {
        let reload = AtomicBool::new(false);

        assert!(!check_reload(&reload));
    }
}
