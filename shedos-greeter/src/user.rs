//! Resolve the username the greeter should authenticate.
//!
//! Order:
//! 1. /etc/shedos/login-user if present and non-empty (Calamares writes
//!    this for the chosen install user; customize_airootfs.sh writes
//!    `shedos` for the live ISO).
//! 2. Auto-detect: the single regular user in /etc/passwd (uid in
//!    1000..65534, has a real shell). ShedOS is single-user by design.
//!
//! Returns None if neither hits — the greeter then renders an empty
//! "Hi, " greeting and the auth dance still works as long as the user
//! types into a future username field. (commit 4 does not yet expose
//! one; the field lands when multi-user lands.)

use std::fs;

const LOGIN_USER_FILE: &str = "/etc/shedos/login-user";
const PASSWD: &str = "/etc/passwd";

pub fn resolve() -> Option<String> {
    if let Ok(text) = fs::read_to_string(LOGIN_USER_FILE) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            log::info!("login user from {}: {}", LOGIN_USER_FILE, trimmed);
            return Some(trimmed.to_string());
        }
    }
    autodetect()
}

fn autodetect() -> Option<String> {
    let passwd = fs::read_to_string(PASSWD).ok()?;
    for line in passwd.lines() {
        let fields: Vec<&str> = line.splitn(7, ':').collect();
        if fields.len() < 7 {
            continue;
        }
        let name = fields[0];
        let uid: u32 = fields[2].parse().ok()?;
        let shell = fields[6];
        // Real users live in 1000..65534. Skip nobody (65534).
        if (1000..65534).contains(&uid) && !shell.ends_with("/false") && !shell.ends_with("/nologin")
        {
            log::info!("login user auto-detected: {} (uid {})", name, uid);
            return Some(name.to_string());
        }
    }
    None
}
