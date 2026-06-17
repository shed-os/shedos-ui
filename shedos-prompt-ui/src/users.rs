//! Regular-user enumeration for the username dropdown, shared by the
//! greeter and the lock screen.

use std::fs;

const PASSWD: &str = "/etc/passwd";
// ShedOS's own accounts, not human logins.
const EXCLUDED: &[&str] = &["shedos", "greeter"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub name: String,
    pub uid: u32,
}

/// Regular users (uid 1000..65534, real login shell) sorted by name;
/// empty if `/etc/passwd` is unreadable.
pub fn enumerate() -> Vec<User> {
    match fs::read_to_string(PASSWD) {
        Ok(text) => enumerate_from(&text),
        Err(e) => {
            log::warn!("users: cannot read {PASSWD}: {e}");
            Vec::new()
        }
    }
}

pub fn enumerate_from(passwd: &str) -> Vec<User> {
    let mut users: Vec<User> = passwd
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.splitn(7, ':').collect();
            if fields.len() < 7 {
                return None;
            }
            let uid = fields[2].parse::<u32>().ok()?;
            let shell = fields[6];
            if (1000..65534).contains(&uid)
                && !shell.ends_with("/false")
                && !shell.ends_with("/nologin")
                && !EXCLUDED.contains(&fields[0])
            {
                Some(User { name: fields[0].to_string(), uid })
            } else {
                None
            }
        })
        .collect();
    users.sort_by(|a, b| a.name.cmp(&b.name));
    users
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerate_filters_and_sorts() {
        let passwd = "\
root:x:0:0::/root:/bin/bash
bin:x:1:1::/:/usr/bin/nologin
svc:x:999:999::/:/usr/bin/nologin
bob:x:1001:1001::/home/bob:/bin/bash
alice:x:1000:1000::/home/alice:/usr/bin/zsh
nobody:x:65534:65534::/:/usr/bin/nologin
";
        let users = enumerate_from(passwd);
        assert_eq!(
            users,
            vec![
                User { name: "alice".into(), uid: 1000 },
                User { name: "bob".into(), uid: 1001 },
            ]
        );
    }

    #[test]
    fn enumerate_skips_service_shells() {
        let passwd = "\
helper:x:1000:1000::/:/usr/bin/nologin
daemon:x:1001:1001::/:/bin/false
carol:x:1002:1002::/home/carol:/bin/bash
";
        assert_eq!(enumerate_from(passwd), vec![User { name: "carol".into(), uid: 1002 }]);
    }

    #[test]
    fn enumerate_survives_malformed_lines() {
        let passwd = "\
garbage-without-colons
short:x:1000
dave:x:notanumber:1000::/home/dave:/bin/bash
erin:x:1000:1000::/home/erin:/bin/bash
";
        assert_eq!(enumerate_from(passwd), vec![User { name: "erin".into(), uid: 1000 }]);
    }

    #[test]
    fn enumerate_empty_when_no_regular_user() {
        assert!(enumerate_from("root:x:0:0::/root:/bin/bash\n").is_empty());
        assert!(enumerate_from("").is_empty());
    }

    #[test]
    fn enumerate_excludes_shedos_and_greeter() {
        let passwd = "\
shedos:x:1000:1000::/home/shedos:/usr/bin/zsh
greeter:x:1001:1001::/var/lib/greeter:/bin/bash
theshedman:x:1002:1002::/home/theshedman:/usr/bin/zsh
";
        assert_eq!(enumerate_from(passwd), vec![User { name: "theshedman".into(), uid: 1002 }]);
    }
}
