//! The `[login]` section of `/etc/shedos/system.toml`: whether the
//! greeter and lock screen show the username dropdown.

use serde::Deserialize;

const SYSTEM_TOML: &str = "/etc/shedos/system.toml";

#[derive(Debug, Default, Deserialize)]
struct SystemToml {
    login: Option<LoginSection>,
}

#[derive(Debug, Default, Deserialize)]
struct LoginSection {
    show_username: Option<bool>,
}

/// Defaults to `true` when the file or key is absent or unparseable, so
/// a broken config never hides the field by accident.
pub fn show_username() -> bool {
    match std::fs::read_to_string(SYSTEM_TOML) {
        Ok(text) => show_username_from(&text),
        Err(e) => {
            log::warn!("login: cannot read {SYSTEM_TOML}: {e}; showing username");
            true
        }
    }
}

pub fn show_username_from(toml_text: &str) -> bool {
    match toml::from_str::<SystemToml>(toml_text) {
        Ok(cfg) => cfg.login.and_then(|l| l.show_username).unwrap_or(true),
        Err(_) => true,
    }
}

/// One helper so the greeter and lock screen never drift apart.
pub fn wrong_credentials_copy(show_username: bool) -> &'static str {
    if show_username {
        "Incorrect username or password"
    } else {
        "Incorrect password"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_true_when_section_absent() {
        assert!(show_username_from("[theme]\npalette = \"x\"\n"));
        assert!(show_username_from(""));
    }

    #[test]
    fn reads_explicit_false() {
        assert!(!show_username_from("[login]\nshow_username = false\n"));
    }

    #[test]
    fn reads_explicit_true() {
        assert!(show_username_from("[login]\nshow_username = true\n"));
    }

    #[test]
    fn default_true_on_garbled_toml() {
        assert!(show_username_from("[login]\nshow_username = "));
        assert!(show_username_from("this is not toml ]["));
    }

    #[test]
    fn default_true_when_key_wrong_type() {
        assert!(show_username_from("[login]\nshow_username = \"yes\"\n"));
    }

    #[test]
    fn copy_includes_username_when_shown() {
        assert_eq!(wrong_credentials_copy(true), "Incorrect username or password");
    }

    #[test]
    fn copy_password_only_when_hidden() {
        assert_eq!(wrong_credentials_copy(false), "Incorrect password");
    }
}
