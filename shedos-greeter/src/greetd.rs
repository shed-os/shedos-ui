//! Synchronous greetd IPC client.
//!
//! Auth flow:
//! 1. `CreateSession { username }` on a fresh socket.
//! 2. Loop on `Response`:
//!    - `AuthMessage` (Secret | Visible) → caller supplies password →
//!      `PostAuthMessageResponse { response: Some(...) }`.
//!    - `AuthMessage` (Info | Error) → log it, ack with `response: None`.
//!    - `Success` → break.
//!    - `Error` → return failure. greetd then only honors
//!      `CancelSession` until reset; our `Drop` impl sends one on
//!      every non-success path so a fresh `CreateSession` works.
//!      Without it, a single wrong-password attempt locks the greeter
//!      until reboot.
//! 3. `StartSession { cmd, env }` to launch the user session. On
//!    `Success` set `succeeded = true` so Drop skips the cancel.

use std::os::unix::net::UnixStream;

use anyhow::{anyhow, Context, Result};
use greetd_ipc::{codec::SyncCodec, AuthMessageType, ErrorType, Request, Response};

pub struct Auth {
    stream: UnixStream,
    /// Set true after `StartSession` returns `Success`. Drop sends
    /// `CancelSession` when false so any error path resets greetd's
    /// per-worker session state.
    succeeded: bool,
}

impl Drop for Auth {
    fn drop(&mut self) {
        if !self.succeeded {
            // Best-effort cancel; ignore I/O errors (the original
            // error report is already on its way up).
            let _ = Request::CancelSession.write_to(&mut self.stream);
        }
    }
}

impl Auth {
    /// Connect to the greetd socket. Errors out cleanly if `GREETD_SOCK`
    /// is unset (i.e. running outside greetd, e.g. interactive dev).
    pub fn connect() -> Result<Self> {
        let path = std::env::var("GREETD_SOCK")
            .context("GREETD_SOCK not set; greeter must be launched by greetd")?;
        let stream = UnixStream::connect(&path)
            .with_context(|| format!("connecting to greetd socket at {}", path))?;
        Ok(Self { stream, succeeded: false })
    }

    /// Authenticate `username` with `password` and start `cmd`. On
    /// success the caller should exit so greetd execs the user session.
    pub fn login(&mut self, username: &str, password: &str, cmd: Vec<String>) -> Result<()> {
        Request::CreateSession {
            username: username.to_string(),
        }
        .write_to(&mut self.stream)
        .context("write CreateSession")?;

        loop {
            match Response::read_from(&mut self.stream).context("read greetd response")? {
                Response::Success => break,
                Response::Error {
                    error_type,
                    description,
                } => {
                    // Log the full PAM/greetd diagnostic; surface a
                    // friendly message to the user. AuthError → user
                    // mistyped; Error → server-side failure.
                    log::warn!(
                        "greetd auth ended in {:?}: {}",
                        error_type,
                        description
                    );
                    let msg = match error_type {
                        ErrorType::AuthError => {
                            "Incorrect username or password".to_string()
                        }
                        ErrorType::Error => {
                            "Authentication failed. Please try again.".to_string()
                        }
                    };
                    return Err(anyhow!("{}", msg));
                }
                Response::AuthMessage {
                    auth_message_type,
                    auth_message,
                } => match auth_message_type {
                    AuthMessageType::Secret | AuthMessageType::Visible => {
                        Request::PostAuthMessageResponse {
                            response: Some(password.to_string()),
                        }
                        .write_to(&mut self.stream)
                        .context("write PostAuthMessageResponse")?;
                    }
                    AuthMessageType::Info | AuthMessageType::Error => {
                        log::info!("greetd msg ({:?}): {}", auth_message_type, auth_message);
                        Request::PostAuthMessageResponse { response: None }
                            .write_to(&mut self.stream)
                            .context("ack info/error")?;
                    }
                },
            }
        }

        let env = vec![
            "XDG_SESSION_TYPE=wayland".to_string(),
            "XDG_CURRENT_DESKTOP=Hyprland".to_string(),
            "XDG_SESSION_DESKTOP=Hyprland".to_string(),
            "PATH=/usr/local/bin:/usr/bin:/bin".to_string(),
            "UWSM_LOG_LEVEL=warning".to_string(),
        ];
        Request::StartSession { cmd, env }
            .write_to(&mut self.stream)
            .context("write StartSession")?;
        match Response::read_from(&mut self.stream).context("read StartSession response")? {
            Response::Success => {
                self.succeeded = true;
                Ok(())
            }
            Response::Error {
                error_type,
                description,
            } => {
                log::warn!(
                    "greetd start_session failed ({:?}): {}",
                    error_type,
                    description
                );
                Err(anyhow!("Could not start session. Please try again."))
            }
            other => {
                log::warn!("unexpected response to StartSession: {:?}", other);
                Err(anyhow!("Could not start session. Please try again."))
            }
        }
    }
}
