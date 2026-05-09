//! PAM password verification for `--mode=lock`.

use anyhow::{anyhow, Context, Result};
use pam::{Client, PamReturnCode};
use shedos_screensaver_wayland::calloop_ping::Ping;
use std::ffi::CStr;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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
/// `shedos-screensaver-fp` PAM service in a loop. Each attempt blocks
/// until the user touches the sensor or pam_fprintd's internal timer
/// fires; the result is sent on the receiver and the calloop ping
/// fires so the lock-binary's main loop wakes immediately to process
/// it. After a success the thread exits; after a failure it loops back
/// for the next attempt so a wrong-finger touch doesn't disable
/// fingerprint unlock for the rest of the lock session.
pub fn spawn_fingerprint_auth_loop(
    username: String,
    ping: Ping,
) -> (Receiver<Result<(), String>>, JoinHandle<()>) {
    let (tx, rx) = channel();
    let handle = thread::Builder::new()
        .name("shedos-fp-auth".into())
        .spawn(move || loop {
            let session = PamSession::new("shedos-screensaver-fp", username.clone());
            let result = session.authenticate("");
            let to_send = result.map_err(|e| {
                eprintln!("shedos-screensaver: pam-fp: {e:?}");
                e.user_message()
            });
            let succeeded = to_send.is_ok();
            if tx.send(to_send).is_err() {
                return;
            }
            ping.ping();
            if succeeded {
                return;
            }
            // Brief pause so a misbehaving driver can't pin a CPU
            // looping on instant-failures. Fingerprint hardware-side
            // retry rate is human-paced anyway.
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
