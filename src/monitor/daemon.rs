use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

use nix::fcntl::{open, OFlag};
use nix::sys::stat::Mode;
use nix::unistd::{close, dup2, fork, setsid, ForkResult, Pid};

use crate::error::{Result, SweeperError};

/// Paths for daemon files
#[derive(Debug, Clone)]
pub struct DaemonPaths {
    pub pid_file: PathBuf,
    pub log_file: PathBuf,
}

impl DaemonPaths {
    pub fn new() -> Self {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));

        let state_dir = std::env::var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .map(|h| h.join(".local/state"))
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
            });

        let log_dir = state_dir.join("rusty-sweeper");
        let _ = fs::create_dir_all(&log_dir);

        Self {
            pid_file: runtime_dir.join("rusty-sweeper.pid"),
            log_file: log_dir.join("monitor.log"),
        }
    }
}

impl Default for DaemonPaths {
    fn default() -> Self {
        Self::new()
    }
}

/// Daemonize the current process
pub fn daemonize(paths: &DaemonPaths) -> Result<()> {
    // Check if already running
    if let Some(pid) = read_pid_file(&paths.pid_file) {
        if is_process_running(pid) {
            return Err(SweeperError::AlreadyRunning(pid as u32));
        }
        // Stale PID file, remove it
        let _ = fs::remove_file(&paths.pid_file);
    }

    // First fork
    match unsafe { fork() }? {
        ForkResult::Parent { .. } => {
            // Parent exits
            std::process::exit(0);
        }
        ForkResult::Child => {
            // Continue as child
        }
    }

    // Create new session
    setsid()?;

    // Second fork to prevent acquiring a controlling terminal
    match unsafe { fork() }? {
        ForkResult::Parent { .. } => {
            std::process::exit(0);
        }
        ForkResult::Child => {
            // Continue as grandchild (daemon)
        }
    }

    // Change working directory to root
    std::env::set_current_dir("/")?;

    // Redirect standard file descriptors to log file / null
    redirect_stdio(paths)?;

    // Write PID file
    write_pid_file(&paths.pid_file)?;

    Ok(())
}

/// Redirect stdin/stdout/stderr
fn redirect_stdio(paths: &DaemonPaths) -> Result<()> {
    // Open /dev/null for stdin
    let dev_null = open("/dev/null", OFlag::O_RDWR, Mode::empty())?;

    // Open log file for stdout/stderr
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)?;
    let log_fd = log_file.as_raw_fd();

    // Redirect stdin to /dev/null
    dup2(dev_null, 0)?;

    // Redirect stdout and stderr to log file
    dup2(log_fd, 1)?;
    dup2(log_fd, 2)?;

    // Close original file descriptors
    if dev_null > 2 {
        close(dev_null)?;
    }

    Ok(())
}

/// Write current PID to file
fn write_pid_file(path: &PathBuf) -> Result<()> {
    let pid = std::process::id();
    let mut file = File::create(path)?;
    writeln!(file, "{}", pid)?;
    Ok(())
}

/// Read PID from file
fn read_pid_file(path: &PathBuf) -> Option<i32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Check if a process is running
fn is_process_running(pid: i32) -> bool {
    // Try to send signal 0 (doesn't actually send a signal, just checks)
    nix::sys::signal::kill(Pid::from_raw(pid), None).is_ok()
}

/// Remove PID file on shutdown
pub fn cleanup_pid_file(paths: &DaemonPaths) {
    let _ = fs::remove_file(&paths.pid_file);
}

/// Stop a running daemon
pub fn stop_daemon(paths: &DaemonPaths) -> Result<bool> {
    if let Some(pid) = read_pid_file(&paths.pid_file) {
        if is_process_running(pid) {
            nix::sys::signal::kill(Pid::from_raw(pid), nix::sys::signal::Signal::SIGTERM)?;

            // Wait a bit and check if it stopped
            std::thread::sleep(std::time::Duration::from_secs(1));

            if !is_process_running(pid) {
                let _ = fs::remove_file(&paths.pid_file);
                return Ok(true);
            }

            // Force kill
            nix::sys::signal::kill(Pid::from_raw(pid), nix::sys::signal::Signal::SIGKILL)?;
            let _ = fs::remove_file(&paths.pid_file);
            return Ok(true);
        }
    }

    Ok(false)
}

/// Get status of daemon
pub fn daemon_status(paths: &DaemonPaths) -> Option<u32> {
    read_pid_file(&paths.pid_file)
        .filter(|&pid| is_process_running(pid))
        .map(|pid| pid as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_daemon_paths() {
        let paths = DaemonPaths::new();
        assert!(paths
            .pid_file
            .to_string_lossy()
            .contains("rusty-sweeper"));
        assert!(paths.log_file.to_string_lossy().contains("monitor.log"));
    }

    #[test]
    fn test_pid_file_operations() {
        let temp = tempdir().unwrap();
        let pid_path = temp.path().join("test.pid");

        // Write PID
        write_pid_file(&pid_path).unwrap();

        // Read PID
        let pid = read_pid_file(&pid_path);
        assert!(pid.is_some());
        assert_eq!(pid.unwrap(), std::process::id() as i32);
    }

    #[test]
    fn test_is_process_running() {
        // Current process should be running
        let pid = std::process::id() as i32;
        assert!(is_process_running(pid));

        // Non-existent process should not be running
        // Use a high PID that's unlikely to exist
        assert!(!is_process_running(99999));
    }

    #[test]
    fn test_daemon_status_not_running() {
        let temp = tempdir().unwrap();
        let paths = DaemonPaths {
            pid_file: temp.path().join("test.pid"),
            log_file: temp.path().join("test.log"),
        };

        // No PID file exists
        assert!(daemon_status(&paths).is_none());
    }

    #[test]
    fn test_cleanup_pid_file() {
        let temp = tempdir().unwrap();
        let paths = DaemonPaths {
            pid_file: temp.path().join("test.pid"),
            log_file: temp.path().join("test.log"),
        };

        // Create PID file
        write_pid_file(&paths.pid_file).unwrap();
        assert!(paths.pid_file.exists());

        // Clean up
        cleanup_pid_file(&paths);
        assert!(!paths.pid_file.exists());
    }
}
