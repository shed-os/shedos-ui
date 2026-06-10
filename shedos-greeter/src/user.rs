//! Resolve the username the greeter should authenticate.
//!
//! Precedence:
//! 1. /etc/shedos/login-user (Calamares writes this for the install
//!    user; the live ISO has `shedos`).
//! 2. Single regular user in /etc/passwd (uid 1000..65534, real
//!    shell). ShedOS is single-user by design.
//!
//! Returns None if neither hits.

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
        // A malformed line skips; it must not abort the whole scan.
        let Ok(uid) = fields[2].parse::<u32>() else {
            continue;
        };
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
