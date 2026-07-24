//! Closing a holder from the popover.
//!
//! The cross next to an app sends it SIGTERM — the same thing Cmd+Q does, so a
//! browser closes its tabs and a torrent client saves its state instead of
//! being torn down. SIGKILL is deliberately not offered: an app that ignores a
//! polite quit is a bug report, not something to force.
//!
//! Doing it with a signal rather than AppleScript is what keeps Nod free of
//! permission prompts — `tell application "X" to quit` needs an Automation
//! grant, and an app that nags on every update is the thing the whole signing
//! setup exists to avoid.

/// Ask the process to quit. `Err` carries something a person can read.
pub fn ask_to_quit(pid: u32) -> Result<(), String> {
    if pid == 0 {
        return Err("no process to close".into());
    }
    signal(pid)
}

#[cfg(unix)]
fn signal(pid: u32) -> Result<(), String> {
    // SAFETY: kill() with a pid we read out of pmset seconds ago. The pid can
    // be gone by now, which the errno below reports as an ordinary failure.
    let sent = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    if sent == 0 {
        crate::debug_log::log(&format!("quit: SIGTERM sent to pid {}", pid));
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    crate::debug_log::log(&format!("quit: pid {} refused the signal: {}", pid, err));
    Err(match err.raw_os_error() {
        Some(libc::ESRCH) => "the app is already gone".into(),
        Some(libc::EPERM) => "that app belongs to another user".into(),
        _ => err.to_string(),
    })
}

#[cfg(not(unix))]
fn signal(_pid: u32) -> Result<(), String> {
    Err("closing apps is macOS-only for now".into())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_pid_that_is_not_there_reads_as_gone() {
        // Above the pid ceiling, so nothing can legitimately answer to it.
        assert_eq!(ask_to_quit(4_194_303), Err("the app is already gone".into()));
    }

    #[test]
    fn pid_zero_is_never_signalled() {
        // kill(0) means "every process in my group" — a stray zero from a parse
        // would take the whole session down with it.
        assert!(ask_to_quit(0).is_err());
    }
}
