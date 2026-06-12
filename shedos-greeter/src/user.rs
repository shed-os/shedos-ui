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
        if let Some(user) = from_login_file(&text) {
            log::info!("login user from {}: {}", LOGIN_USER_FILE, user);
            return Some(user);
        }
    }
    autodetect()
}

fn from_login_file(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn autodetect() -> Option<String> {
    let passwd = fs::read_to_string(PASSWD).ok()?;
    autodetect_from(&passwd)
}

fn autodetect_from(passwd: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_file_wins_and_is_trimmed() {
        assert_eq!(from_login_file("  alice \n"), Some("alice".into()));
        assert_eq!(from_login_file("\n  \n"), None);
        assert_eq!(from_login_file(""), None);
    }

    #[test]
    fn autodetect_picks_the_first_real_user() {
        let passwd = "\
root:x:0:0::/root:/bin/bash
bin:x:1:1::/:/usr/bin/nologin
svc:x:999:999::/:/usr/bin/nologin
alice:x:1000:1000::/home/alice:/usr/bin/zsh
bob:x:1001:1001::/home/bob:/bin/bash
";
        assert_eq!(autodetect_from(passwd), Some("alice".into()));
    }

    #[test]
    fn autodetect_skips_service_shells_and_nobody() {
        let passwd = "\
helper:x:1000:1000::/:/usr/bin/nologin
daemon:x:1001:1001::/:/bin/false
nobody:x:65534:65534::/:/usr/bin/nologin
carol:x:1002:1002::/home/carol:/bin/bash
";
        assert_eq!(autodetect_from(passwd), Some("carol".into()));
    }

    #[test]
    fn autodetect_survives_malformed_lines() {
        let passwd = "\
garbage-without-colons
short:x:1000
dave:x:notanumber:1000::/home/dave:/bin/bash
erin:x:1000:1000::/home/erin:/bin/bash
";
        assert_eq!(autodetect_from(passwd), Some("erin".into()));
    }

    #[test]
    fn autodetect_none_when_no_regular_user() {
        assert_eq!(autodetect_from("root:x:0:0::/root:/bin/bash\n"), None);
        assert_eq!(autodetect_from(""), None);
    }
}
