//! Opens web pages in the user's browser.

use std::io;
use std::process::{Command, Stdio};

/// Opens `url` with the system's opener and returns at once.
pub fn open(url: &str) -> io::Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let mut child = Command::new(program)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // Reap the opener when it exits, without blocking the UI.
    std::thread::spawn(move || child.wait());
    Ok(())
}
