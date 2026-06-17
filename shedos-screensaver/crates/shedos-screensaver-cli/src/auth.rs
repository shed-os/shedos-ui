//! PAM password verification for `--mode=lock`.

use anyhow::{anyhow, Result};
use pam::{Client, Conversation, PamReturnCode};
use shedos_screensaver_wayland::calloop_ping::Ping;
use std::ffi::{CStr, CString};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// PAM conversation handler for the fingerprint thread. pam_fprintd
/// fires `ERROR_MSG` on every `verify-no-match`, well before its outer
/// pam_authenticate() returns the aggregate `MaxTries`. Forwarding
/// those errors to the channel as they happen lets the lock UI flash
/// red within milliseconds instead of waiting ~30s for the aggregate.
///
/// `prompt_echo`/`prompt_blind` are required by the trait but
/// pam_fprintd never invokes them.
struct FingerprintConv {
    username: String,
    tx: Sender<std::result::Result<(), ()>>,
    ping: Ping,
}

impl Conversation for FingerprintConv {
    fn prompt_echo(&mut self, _msg: &CStr) -> std::result::Result<CString, ()> {
        CString::new(self.username.clone()).map_err(|_| ())
    }
    fn prompt_blind(&mut self, _msg: &CStr) -> std::result::Result<CString, ()> {
        CString::new("").map_err(|_| ())
    }
    fn info(&mut self, _msg: &CStr) {
        // pam_fprintd's "Place your finger" — informational only.
    }
    fn error(&mut self, msg: &CStr) {
        let text = msg.to_string_lossy();
        // pam_fprintd routes verify-disconnected/verify-unknown-error here
        // as well as verify-no-match; only the last is a real wrong-finger
        // touch worth flashing. It hands us its own gettext-localized text
        // (no result code reaches this PAM callback), so match its exact
        // no-match msgid. Under a non-English LC_MESSAGES this won't match
        // and the flash is skipped — driver chatter is suppressed either way.
        if text == "Failed to match fingerprint" {
            let _ = self.tx.send(Err(()));
            self.ping.ping();
        } else {
            eprintln!("shedos-screensaver: pam-fp error (ignored): {text}");
        }
    }
}

#[derive(Debug)]
pub enum AuthFailure {
    WrongPassword,
    AccountLocked,
    AccountExpired,
    UnknownUser,
    Other(PamReturnCode),
    Init(String),
}

impl AuthFailure {
    pub fn user_message(&self, show_username: bool) -> String {
        match self {
            Self::WrongPassword => {
                shedos_prompt_ui::wrong_credentials_copy(show_username).to_string()
            }
            Self::AccountLocked => {
                "Account locked. Run `faillock --user $USER --reset` from a tty.".into()
            }
            Self::AccountExpired => "Account expired.".into(),
            Self::UnknownUser => "User not recognized.".into(),
            Self::Other(code) => format!("Authentication failed ({code})."),
            Self::Init(msg) => format!("PAM init failed: {msg}"),
        }
    }
}

pub struct PamSession {
    service: String,
    username: String,
}

impl PamSession {
    pub fn new(service: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            username: username.into(),
        }
    }

    pub fn authenticate(&self, password: &str) -> Result<(), AuthFailure> {
        let mut client = Client::with_password(&self.service)
            .map_err(|e| AuthFailure::Init(format!("{e}")))?;
        client
            .conversation_mut()
            .set_credentials(self.username.clone(), password.to_owned());
        client.authenticate().map_err(|e| match e.0 {
            PamReturnCode::Auth_Err => AuthFailure::WrongPassword,
            PamReturnCode::MaxTries => AuthFailure::AccountLocked,
            PamReturnCode::Acct_Expired => AuthFailure::AccountExpired,
            PamReturnCode::User_Unknown => AuthFailure::UnknownUser,
            other => AuthFailure::Other(other),
        })
    }
}

/// Enrolled-finger summary returned when fprintd has at least one.
#[derive(Debug, Clone)]
pub struct FingerprintInfo {
    pub finger_count: usize,
}

/// Probe `fprintd-list <username>` with a 2-second timeout so a hung
/// daemon doesn't block lock startup. Returns None when fprintd is
/// missing, no device is present, or no fingers are enrolled.
pub fn fingerprint_available(username: &str) -> Option<FingerprintInfo> {
    let output = Command::new("timeout")
        .arg("2")
        .arg("fprintd-list")
        .arg(username)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let finger_count = stdout
        .lines()
        .filter(|l| l.trim_start().starts_with("- #"))
        .count();
    if finger_count == 0 {
        None
    } else {
        Some(FingerprintInfo { finger_count })
    }
}

/// Spawn a thread that authenticates via `shedos-screensaver-fp`.
/// `FingerprintConv` emits `Err(())` for every `verify-no-match` as
/// it happens; the thread itself emits `Ok(())` on outer success.
///
/// Aggregate `MaxTries` returns are not surfaced. They fire after
/// pam_fprintd's internal 3-cycle retry, far after the user's touch.
///
/// The thread loops on Err so the user can keep trying; exits on success.
pub fn spawn_fingerprint_auth_loop(
    username: String,
    ping: Ping,
    paused: Arc<AtomicBool>,
) -> (Receiver<std::result::Result<(), ()>>, JoinHandle<()>) {
    let (tx, rx) = channel::<std::result::Result<(), ()>>();
    let handle = thread::Builder::new()
        .name("shedos-fp-auth".into())
        .spawn(move || loop {
            if paused.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            let conv = FingerprintConv {
                username: username.clone(),
                tx: tx.clone(),
                ping: ping.clone(),
            };
            match Client::with_conversation(
                "shedos-screensaver-fp",
                conv,
            )
            .and_then(|mut c| c.authenticate())
            {
                Ok(()) => {
                    let _ = tx.send(Ok(()));
                    ping.ping();
                    return;
                }
                Err(e) => {
                    // Per-scan failures already pushed via
                    // FingerprintConv::error. Don't emit aggregate
                    // Err here — outer MaxTries fires ~30s after the
                    // user's last touch and feels unrelated.
                    eprintln!(
                        "shedos-screensaver: pam-fp authenticate: {:?}",
                        e.0
                    );
                }
            }
            // Brief pause before re-entering pam_authenticate so a
            // misbehaving init failure can't pin a CPU.
            thread::sleep(Duration::from_millis(200));
        })
        .expect("spawn fp auth thread");
    (rx, handle)
}

pub fn current_username() -> Result<String> {
    // getpwuid first: this name feeds PAM, and the environment is
    // attacker-influencable in ways the password database is not
    // (a stale or spoofed $USER would aim the auth at someone else).
    let uid = unsafe { libc::getuid() };
    let pw = unsafe { libc::getpwuid(uid) };
    if !pw.is_null() {
        let name = unsafe { CStr::from_ptr((*pw).pw_name) };
        if let Ok(s) = name.to_str() {
            if !s.is_empty() {
                return Ok(s.to_owned());
            }
        }
    }
    for var in ["USER", "LOGNAME"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return Ok(v);
            }
        }
    }
    Err(anyhow!("cannot resolve current uid {} via getpwuid or env", uid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_password_copy_matches_shared_helper() {
        let with = AuthFailure::WrongPassword.user_message(true);
        let without = AuthFailure::WrongPassword.user_message(false);
        assert_eq!(with, shedos_prompt_ui::wrong_credentials_copy(true));
        assert_eq!(without, shedos_prompt_ui::wrong_credentials_copy(false));
    }
}
