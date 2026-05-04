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
//!    - `Error` → return failure (caller clears password and retries
//!      on a *new* socket; greetd considers a session tarnished after
//!      an Error and will only honor `CancelSession` until then).
//! 3. `StartSession { cmd, env }` to launch the user session;
//!    on `Success` we exit and greetd execs the user's session.

use std::os::unix::net::UnixStream;

use anyhow::{anyhow, Context, Result};
use greetd_ipc::{codec::SyncCodec, AuthMessageType, ErrorType, Request, Response};

pub struct Auth {
    stream: UnixStream,
}

impl Auth {
    /// Connect to the greetd socket. Errors out cleanly if `GREETD_SOCK`
    /// is unset (i.e. running outside greetd, e.g. interactive dev).
    pub fn connect() -> Result<Self> {
        let path = std::env::var("GREETD_SOCK")
            .context("GREETD_SOCK not set; greeter must be launched by greetd")?;
        let stream = UnixStream::connect(&path)
            .with_context(|| format!("connecting to greetd socket at {}", path))?;
        Ok(Self { stream })
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
            Response::Success => Ok(()),
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
