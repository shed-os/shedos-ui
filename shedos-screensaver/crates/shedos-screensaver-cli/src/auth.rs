//! PAM password verification for `--mode=lock`.

use anyhow::{anyhow, Context, Result};
use pam::{Client, Conversation, PamReturnCode};
use shedos_screensaver_wayland::calloop_ping::Ping;
use std::ffi::{CStr, CString};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// PAM conversation handler dedicated to the fingerprint thread.
/// pam_fprintd communicates via `TEXT_INFO` ("Place your finger") and
/// `ERROR_MSG` ("Failed to match fingerprint"). The error path is the
/// per-scan rejection signal — fired by pam_fprintd immediately when
/// libfprint reports `verify-no-match`, well before the outer
/// pam_authenticate() returns its aggregate `MaxTries`. We forward
/// each `ERROR_MSG` through the same channel as the success signal so
/// the lock UI flashes red within milliseconds of a failed scan
/// instead of waiting 30s for the outer call's MaxTries.
///
/// `prompt_echo`/`prompt_blind` are required by the trait but
/// pam_fprintd never invokes those styles.
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
        // pam_fprintd's "Place your finger" — informational, no
        // user-facing state change. The icon's idle color is already
        // the affordance that says "ready for a touch."
    }
    fn error(&mut self, _msg: &CStr) {
        // pam_fprintd's "Failed to match fingerprint" — fires per
        // verify-no-match. Push immediately so the lock UI sees the
        // failure event in real time.
        let _ = self.tx.send(Err(()));
        self.ping.ping();
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
    pub fn user_message(&self) -> String {
        match self {
            Self::WrongPassword => "Wrong password.".into(),
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

/// Result of probing fprintd for the user's enrolled fingers. `None`
/// means the lock surface should not show the fingerprint UI — fprintd
/// is missing, no device is present, or no fingers are enrolled. `Some`
/// means the lock surface should show the icon + hint text and listen
/// on a parallel auth thread.
#[derive(Debug, Clone)]
pub struct FingerprintInfo {
    pub finger_count: usize,
}

/// Probe `fprintd-list <username>` for enrolled fingers, with a 2-second
/// hard timeout so a hung daemon doesn't block lock-screen startup.
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

/// Spawn a background thread that authenticates the user via the
/// `shedos-screensaver-fp` PAM service. The conversation handler
/// (`FingerprintConv`) emits `Err(())` on the channel for every
/// `verify-no-match` reported by pam_fprintd as soon as it happens —
/// real-time per-scan feedback. The auth thread itself only emits
/// `Ok(())` on the outer pam_authenticate success; aggregate
/// `MaxTries` returns are NOT surfaced as flashes because they fire
/// after pam_fprintd's internal 3-cycle retry, which is much later
/// than the user's actual touch and confuses the cause-effect link.
///
/// On any pam_authenticate Err the thread still loops (after a brief
/// pause) so the user can keep trying. The thread exits on success.
pub fn spawn_fingerprint_auth_loop(
    username: String,
    ping: Ping,
) -> (Receiver<std::result::Result<(), ()>>, JoinHandle<()>) {
    let (tx, rx) = channel::<std::result::Result<(), ()>>();
    let handle = thread::Builder::new()
        .name("shedos-fp-auth".into())
        .spawn(move || loop {
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
                    // Per-scan failures were already pushed by the
                    // FingerprintConv::error hook in real time. Don't
                    // emit another aggregate Err here — the old
                    // behavior of flashing red on the outer MaxTries
                    // return showed up 30 seconds after the user's
                    // last touch and felt unrelated.
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
    for var in ["USER", "LOGNAME"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return Ok(v);
            }
        }
    }
    let uid = unsafe { libc::getuid() };
    let pw = unsafe { libc::getpwuid(uid) };
    if pw.is_null() {
        return Err(anyhow!("cannot resolve current uid {} via getpwuid", uid));
    }
    let name = unsafe { CStr::from_ptr((*pw).pw_name) };
    name.to_str()
        .map(|s| s.to_owned())
        .context("pw_name is not valid UTF-8")
}
