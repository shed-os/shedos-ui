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
pub const LAST_LOGIN_FILE: &str = "/var/lib/shedos/last-login";

pub fn resolve() -> Option<String> {
    if let Ok(text) = fs::read_to_string(LOGIN_USER_FILE) {
        if let Some(user) = from_login_file(&text) {
            log::info!("login user from {}: {}", LOGIN_USER_FILE, user);
            return Some(user);
        }
    }
    autodetect()
}

/// The username the dropdown preselects. Precedence: the persisted
/// last-login marker (if still a real user), then /etc/shedos/login-user,
/// then the first enumerated user.
pub fn default_pick(users: &[shedos_prompt_ui::User]) -> Option<String> {
    let last = fs::read_to_string(LAST_LOGIN_FILE).ok();
    let last = last.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let login = fs::read_to_string(LOGIN_USER_FILE).ok();
    let login = login.as_deref().map(str::trim).filter(|s| !s.is_empty());
    default_pick_with(users, last, login)
}

fn default_pick_with(
    users: &[shedos_prompt_ui::User],
    last_login: Option<&str>,
    login_file: Option<&str>,
) -> Option<String> {
    if users.is_empty() {
        return None;
    }
    let known = |name: &str| users.iter().any(|u| u.name == name);
    if let Some(n) = last_login.filter(|n| known(n)) {
        return Some(n.to_string());
    }
    if let Some(n) = login_file.filter(|n| known(n)) {
        return Some(n.to_string());
    }
    Some(users[0].name.clone())
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

    use shedos_prompt_ui::User;

    fn users() -> Vec<User> {
        vec![
            User { name: "alice".into(), uid: 1000 },
            User { name: "bob".into(), uid: 1001 },
        ]
    }

    #[test]
    fn default_pick_prefers_last_login_when_valid() {
        let pick = default_pick_with(&users(), Some("bob"), Some("alice"));
        assert_eq!(pick.as_deref(), Some("bob"));
    }

    #[test]
    fn default_pick_ignores_stale_last_login() {
        let pick = default_pick_with(&users(), Some("ghost"), Some("alice"));
        assert_eq!(pick.as_deref(), Some("alice"));
    }

    #[test]
    fn default_pick_login_file_when_no_marker() {
        let pick = default_pick_with(&users(), None, Some("bob"));
        assert_eq!(pick.as_deref(), Some("bob"));
    }

    #[test]
    fn default_pick_falls_to_first_user() {
        let pick = default_pick_with(&users(), None, None);
        assert_eq!(pick.as_deref(), Some("alice"));
    }

    #[test]
    fn default_pick_none_on_empty() {
        assert_eq!(default_pick_with(&[], Some("x"), Some("y")), None);
    }
}
