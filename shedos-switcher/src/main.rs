use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};

use anyhow::Result;

mod model;
mod render;
mod ui;

fn socket_path() -> std::path::PathBuf {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    std::path::Path::new(&dir).join("shedos-switcher.sock")
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cmd = if std::env::args().any(|a| a == "--prev") { "prev" } else { "next" };

    // Single instance: Hyprland's own ALT+Tab bind fires this binary
    // on every press, so a second invocation just tells the running
    // strip to cycle and exits. That IS the Tab-cycling mechanism.
    let sock = socket_path();
    if let Ok(mut stream) = UnixStream::connect(&sock) {
        let _ = stream.write_all(cmd.as_bytes());
        return Ok(());
    }
    let _ = std::fs::remove_file(&sock); // stale leftover from a crash

    let windows = model::list_windows()?;
    if windows.len() < 2 {
        if let Some(w) = windows.first() {
            model::focus(&w.address);
        }
        return Ok(());
    }

    let listener = UnixListener::bind(&sock)?;
    let result = render::run(windows, listener);
    let _ = std::fs::remove_file(&sock);
    result
}

/// Blocks on the socket, forwarding one command per connection.
pub fn listen(listener: UnixListener, tx: render::CmdSender) {
    for stream in listener.incoming().flatten() {
        let mut buf = String::new();
        let mut s = stream;
        if s.read_to_string(&mut buf).is_ok() {
            let cmd = if buf.trim() == "prev" { render::Cmd::Prev } else { render::Cmd::Next };
            if tx.send(cmd).is_err() {
                return;
            }
        }
    }
}
