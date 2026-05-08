//! Synchronous greetd IPC client.
//!
//! Auth flow:
//! 1. `CreateSession { username }` on a fresh socket.
//! 2. Loop on `Response`:
//!    - `AuthMessage` (Secret | Visible) → caller supplies password →
//!      `PostAuthMessageResponse { response: Some(...) }`.
//!    - `AuthMessage` (Info | Error) → log it, ack with
//!      `PostAuthMessageResponse { response: None }` and continue.
//!    - `Success` → break.
//!    - `Error` → return failure. greetd considers a session tarnished
//!      after an Error and only honors `CancelSession` until reset; our
//!      `Drop` impl sends `CancelSession` on every non-success path so
//!      the next `CreateSession` on a fresh socket sees a clean slate.
//!      Without that cleanup a single wrong-password attempt soft-bricks
//!      the greeter — every subsequent attempt fails with "a session
//!      is already being configured" until reboot.
//! 3. `StartSession { cmd, env }` to launch the user session;
//!    on `Success` we mark the auth as succeeded (suppressing the Drop
//!    cancel, since greetd is now mid-handoff to the user session) and
//!    exit.

use std::os::unix::net::UnixStream;

use anyhow::{anyhow, Context, Result};
use greetd_ipc::{codec::SyncCodec, AuthMessageType, ErrorType, Request, Response};

pub struct Auth {
    stream: UnixStream,
    /// Set true after `StartSession` returns `Success`. The Drop impl
    /// sends `CancelSession` iff this is false, so any error path —
    /// named branches, `?` propagation from I/O errors, unexpected
    /// responses — cleanly resets greetd's per-worker session state.
    succeeded: bool,
}

impl Drop for Auth {
    fn drop(&mut self) {
        if !self.succeeded {
            // Best-effort cancel; if the socket is already broken we
            // can't do better than the original error report. Silent
            // drop of any I/O failure here is intentional.
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

    /// Authenticate `username` with `password` and start `cmd`. Returns
    /// `Ok(())` on session-start success; the caller should then exit so
    /// greetd execs the user session. Any non-success path returns an
    /// `Err` whose message can be logged for diagnostics.
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
                    let kind = match error_type {
                        ErrorType::AuthError => "auth",
                        ErrorType::Error => "error",
                    };
                    return Err(anyhow!("greetd {}: {}", kind, description));
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
            } => Err(anyhow!(
                "greetd start_session ({:?}): {}",
                error_type,
                description
            )),
            other => Err(anyhow!("unexpected response to StartSession: {:?}", other)),
        }
    }
}
