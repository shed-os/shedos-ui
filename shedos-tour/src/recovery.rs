//! The mandatory recovery-key slide (#202). Both in-place `shedman encrypt` and the
//! Calamares install escrow stash the disk recovery key here; the tour shows it as an
//! un-skippable first slide, gates the tour until the user types the acknowledgement,
//! then shreds the stash. This module is the testable core — the stash read/shred and
//! the acknowledgement state machine; render.rs drives it from the Wayland input loop.

use std::path::{Path, PathBuf};

/// The phrase the user must type to dismiss the slide. Deliberate, not a reflex Enter:
/// this key is the only way back into the disk if the passphrase is lost.
pub const ACK_PHRASE: &str = "yes";

/// The stashed key, written `root:wheel 0660` by the producers. Overridable for tests.
pub fn stash_path() -> PathBuf {
    std::env::var_os("SHEDOS_RECOVERY_STASH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/shedos/encrypt/recovery-key"))
}

/// Read the stashed recovery key, trimmed. None if it is absent, unreadable, empty, or
/// carries no printable content (a shredded leftover) — any of which means "no key to
/// show", so the tour runs (or exits) normally.
pub fn read_stash(path: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let key = raw
        .trim_matches(|c: char| c.is_whitespace() || c == '\0')
        .to_string();
    (!key.is_empty() && !key.chars().any(char::is_control)).then_some(key)
}

/// Drop the stash once the user has acknowledged. Truncate before removing: the stash
/// dir may deny this user the unlink (root-owned on older installs), and an empty
/// leftover reads as "no key" everywhere, where a same-length zero fill would not.
pub fn shred_stash(path: &Path) {
    if std::fs::metadata(path).is_ok() {
        let _ = std::fs::write(path, b"");
    }
    if let Err(e) = std::fs::remove_file(path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            eprintln!(
                "shedos-tour: recovery-key stash not removed ({e}); left empty at {}",
                path.display()
            );
        }
    }
}

/// The interactive state of the recovery slide: the key to show and the acknowledgement
/// the user is typing.
pub struct Recovery {
    pub key: String,
    typed: String,
}

impl Recovery {
    pub fn new(key: String) -> Self {
        Self { key, typed: String::new() }
    }

    /// Feed a typed character into the acknowledgement buffer (control chars ignored).
    pub fn type_char(&mut self, c: char) {
        if !c.is_control() {
            self.typed.push(c);
        }
    }

    pub fn backspace(&mut self) {
        self.typed.pop();
    }

    pub fn typed(&self) -> &str {
        &self.typed
    }

    /// True once the buffer matches the acknowledgement phrase (case-insensitive,
    /// surrounding whitespace ignored).
    pub fn acknowledged(&self) -> bool {
        self.typed.trim().eq_ignore_ascii_case(ACK_PHRASE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unique per test — they run in parallel threads of one process, so a
    // PID-only name would collide.
    fn tmp(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("shedos-tour-recovery-{}-{}", std::process::id(), tag));
        p
    }

    #[test]
    fn read_stash_returns_trimmed_key() {
        let p = tmp("read");
        std::fs::write(&p, "  PRNDW-EWGMR-AAAAA\n").unwrap();
        assert_eq!(read_stash(&p).as_deref(), Some("PRNDW-EWGMR-AAAAA"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn read_stash_none_when_absent_or_empty() {
        let p = tmp("none");
        let _ = std::fs::remove_file(&p);
        assert_eq!(read_stash(&p), None);
        std::fs::write(&p, "   \n").unwrap();
        assert_eq!(read_stash(&p), None);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn read_stash_none_on_shred_residue() {
        // A zero-filled leftover (old shred + denied unlink) must never read as a key.
        let p = tmp("residue");
        std::fs::write(&p, vec![0u8; 30]).unwrap();
        assert_eq!(read_stash(&p), None);
        // Nor content whose "key" would be control characters.
        std::fs::write(&p, "\u{1}\u{2}\u{3}\n").unwrap();
        assert_eq!(read_stash(&p), None);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn shred_removes_the_stash() {
        let p = tmp("shred");
        std::fs::write(&p, "secret").unwrap();
        shred_stash(&p);
        assert!(!p.exists());
    }

    #[test]
    fn shred_leaves_an_empty_file_when_the_unlink_is_denied() {
        use std::os::unix::fs::PermissionsExt;
        let mut dir = std::env::temp_dir();
        dir.push(format!("shedos-tour-shredden-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("recovery-key");
        std::fs::write(&p, "SECRET-KEY-MATERIAL\n").unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        shred_stash(&p);

        // Root (CI containers) can unlink regardless; the guarantee to assert is
        // simply that whatever remains never reads as a key.
        if p.exists() {
            assert_eq!(std::fs::metadata(&p).unwrap().len(), 0, "content must be gone");
        }
        assert_eq!(read_stash(&p), None);

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn acknowledged_only_on_the_exact_phrase() {
        let mut r = Recovery::new("KEY".into());
        assert!(!r.acknowledged());
        for c in "ye".chars() {
            r.type_char(c);
        }
        assert!(!r.acknowledged(), "partial phrase must not acknowledge");
        r.type_char('s');
        assert!(r.acknowledged(), "full phrase acknowledges");
    }

    #[test]
    fn acknowledge_is_case_insensitive_and_trimmed() {
        let mut r = Recovery::new("KEY".into());
        for c in "  YES ".chars() {
            r.type_char(c);
        }
        assert!(r.acknowledged());
    }

    #[test]
    fn backspace_unmakes_acknowledgement() {
        let mut r = Recovery::new("KEY".into());
        for c in ACK_PHRASE.chars() {
            r.type_char(c);
        }
        assert!(r.acknowledged());
        r.backspace();
        assert!(!r.acknowledged());
    }

    #[test]
    fn control_chars_are_ignored() {
        let mut r = Recovery::new("KEY".into());
        r.type_char('\n');
        r.type_char('\t');
        assert_eq!(r.typed(), "");
    }
}
