//! PAM password verification for `--mode=lock`.

use anyhow::{anyhow, Context, Result};
use pam::{Client, PamReturnCode};
use std::ffi::CStr;

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
