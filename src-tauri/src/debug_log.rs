//! The session log: what the app did, never more than it has to.
//!
//! Nod lives in the menu bar, so when the icon shows a colour that looks wrong
//! there is no console to look at — this file is the only witness. Two rules
//! keep it small and dull:
//!
//! * **Events, not inventory.** Lines record that N holders were found and
//!   which one was named in a notification. A full dump of everything running
//!   every minute would turn the log into a record of the workday.
//! * **Fresh file per launch.** An append-only log on an app that runs for
//!   weeks is a slow disk leak, and only the current session is ever useful.
//!
//! Owner-only (0600) through `private.rs` either way — Ribbit, Quill and Iago
//! keep their logs the same way, deliberately.

use chrono::Local;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

fn log_file() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("nod").join("logs");
    let _ = crate::private::create_dir(&dir);
    Some(dir.join("debug.log"))
}

pub fn init() {
    let Some(path) = log_file() else { return };
    let _ = crate::private::write(&path, b"");
    if let Ok(mut g) = LOG_PATH.lock() {
        *g = Some(path);
    }
}

pub fn log(msg: &str) {
    let cached = LOG_PATH.lock().ok().and_then(|g| g.clone());
    // Before init() the path is not cached yet — resolve it rather than drop the
    // line; a log that starts late is still a log.
    let Some(path) = cached.or_else(log_file) else { return };
    if let Ok(mut file) = crate::private::append(&path) {
        let ts = Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(file, "[{}] {}", ts, msg);
    }
}
