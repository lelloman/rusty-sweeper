//! Run only the explicitly confirmed deletion with elevated permissions.

use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path};
use std::process::Command;

use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};

fn delete_command(path: &Path) -> io::Result<Command> {
    if !path.is_absolute()
        || path == Path::new("/")
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid sudo deletion path",
        ));
    }
    let mut command = Command::new("/usr/bin/sudo");
    // Separate argv entries and -- keep filenames out of shell/option parsing.
    command.args(["--", "/usr/bin/rm", "-rf", "--"]).arg(path);
    Ok(command)
}

struct RestoreSigint(SigAction);

impl Drop for RestoreSigint {
    fn drop(&mut self) {
        // SAFETY: restore the disposition saved by this foreground operation.
        let _ = unsafe { sigaction(Signal::SIGINT, &self.0) };
    }
}

/// The input worker must be stopped and the terminal restored before calling.
/// Sudo owns password input; the application never reads or stores it.
pub(super) fn delete(path: &Path) -> io::Result<()> {
    let mut command = delete_command(path)?;
    let ignore = SigAction::new(SigHandler::SigIgn, SaFlags::empty(), SigSet::empty());
    // Keep Ctrl-C at sudo's prompt from terminating the application too.
    // SAFETY: this installs a standard disposition, not a Rust signal handler.
    let _restore = RestoreSigint(unsafe { sigaction(Signal::SIGINT, &ignore) }?);
    // SAFETY: the child performs only the async-signal-safe sigaction syscall
    // between fork and exec. No locks, allocation, or application state access.
    unsafe {
        command.pre_exec(|| {
            let default = SigAction::new(SigHandler::SigDfl, SaFlags::empty(), SigSet::empty());
            sigaction(Signal::SIGINT, &default)?;
            Ok(())
        });
    }
    eprintln!("Retrying deletion with sudo: {}", path.display());
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "Sudo deletion cancelled or failed ({status})"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sudo_receives_exact_path_without_shell_interpretation() {
        let path = Path::new("/tmp/-example ; $(touch nope) ' spaced");
        let command = delete_command(path).unwrap();
        assert_eq!(command.get_program(), "/usr/bin/sudo");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                std::ffi::OsStr::new("--"),
                std::ffi::OsStr::new("/usr/bin/rm"),
                std::ffi::OsStr::new("-rf"),
                std::ffi::OsStr::new("--"),
                path.as_os_str(),
            ]
        );
    }

    #[test]
    fn rejects_root_relative_and_parent_traversal_paths() {
        for path in ["/", "relative", "/tmp/../", "/tmp/../etc"] {
            assert!(delete_command(Path::new(path)).is_err());
        }
    }
}
