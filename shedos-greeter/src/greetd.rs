//! greetd conversation worker.
//!
//! Auth runs on its own thread so PAM (pam_unix's fail delay,
//! faillock, fprintd's hardware wait) never freezes the render loop.
//! The worker drives one greetd session at a time:
//!
//! 1. `CreateSession { username }` eagerly at greeter start — with
//!    pam_fprintd in /etc/pam.d/greetd this arms the reader
//!    immediately, so touch-to-login works straight from boot.
//! 2. PAM info messages naming the reader surface as
//!    `AuthEvent::Fingerprint`; the UI shows the affordance while the
//!    window is open.
//! 3. A password typed while fprintd still owns the conversation is
//!    buffered and auto-submitted the moment the secret prompt
//!    arrives. If the prompt is already waiting, it answers at once.
//! 4. Wrong password → `AuthEvent::Failed` → the session is cancelled
//!    and a fresh one created, so the next attempt (and the
//!    fingerprint window) re-arms. Real failures hit faillock as
//!    designed; cancels don't.
//!
//! Every `events.send` is followed by `wake()` (a calloop ping) so the
//! Wayland loop drains the channel promptly.

use std::os::unix::net::UnixStream;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use greetd_ipc::{codec::SyncCodec, AuthMessageType, ErrorType, Request, Response};
use zeroize::Zeroizing;

/// Worker → UI.
pub enum AuthEvent {
    /// fprintd's "place your finger…" info message; affordance on.
    Fingerprint(String),
    /// The secret prompt arrived (fingerprint window over, if any).
    PromptReady,
    /// Attempt failed; message for the error line. A fresh session is
    /// already being created for the retry.
    Failed(String),
    /// start_session succeeded — exit so greetd execs the session.
    SessionStarted,
}

struct Session {
    stream: UnixStream,
    /// True after StartSession succeeds; Drop cancels otherwise so
    /// greetd's per-worker state resets for the next CreateSession.
    succeeded: bool,
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.succeeded {
            let _ = Request::CancelSession.write_to(&mut self.stream);
        }
    }
}

// Bounds a wedged greetd: long enough to clear PAM fail delays,
// faillock and fprintd hardware waits, short enough that a hung daemon
// surfaces as an error instead of pinning the auth worker forever.
const SOCKET_TIMEOUT: Duration = Duration::from_secs(120);

fn connect() -> Result<UnixStream> {
    let path = std::env::var("GREETD_SOCK")
        .context("GREETD_SOCK not set; greeter must be launched by greetd")?;
    let stream = UnixStream::connect(&path)
        .with_context(|| format!("connecting to greetd socket at {path}"))?;
    stream
        .set_read_timeout(Some(SOCKET_TIMEOUT))
        .context("set greetd read timeout")?;
    stream
        .set_write_timeout(Some(SOCKET_TIMEOUT))
        .context("set greetd write timeout")?;
    Ok(stream)
}

/// Most-recent queued password, if any.
fn drain_latest(rx: &Receiver<Zeroizing<String>>) -> Option<Zeroizing<String>> {
    let mut latest = None;
    loop {
        match rx.try_recv() {
            Ok(pw) => latest = Some(pw),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return latest,
        }
    }
}

/// One full auth attempt. Ok(true) = session started; Ok(false) =
/// auth failed (already reported); Err = transport/protocol failure.
fn run_attempt(
    username: &str,
    cmd: &[String],
    show_username: bool,
    events: &Sender<AuthEvent>,
    wake: &(impl Fn() + Send),
    pw_rx: &Receiver<Zeroizing<String>>,
) -> Result<bool> {
    let mut session = Session { stream: connect()?, succeeded: false };
    Request::CreateSession { username: username.to_string() }
        .write_to(&mut session.stream)
        .context("write CreateSession")?;

    let mut buffered = drain_latest(pw_rx);
    loop {
        match Response::read_from(&mut session.stream).context("read greetd response")? {
            Response::Success => break,
            Response::Error { error_type, description } => {
                log::warn!("greetd auth ended in {:?}: {}", error_type, description);
                let msg = match error_type {
                    ErrorType::AuthError => {
                        shedos_prompt_ui::wrong_credentials_copy(show_username).to_string()
                    }
                    ErrorType::Error => "Authentication failed. Please try again.".to_string(),
                };
                let _ = events.send(AuthEvent::Failed(msg));
                wake();
                return Ok(false);
            }
            Response::AuthMessage { auth_message_type, auth_message } => match auth_message_type {
                AuthMessageType::Secret | AuthMessageType::Visible => {
                    let _ = events.send(AuthEvent::PromptReady);
                    wake();
                    // Buffered password (typed during the fingerprint
                    // window) answers immediately; otherwise block for
                    // the user's Enter.
                    if let Some(latest) = drain_latest(pw_rx) {
                        buffered = Some(latest);
                    }
                    let pw = match buffered.take() {
                        Some(pw) => pw,
                        None => pw_rx.recv().context("UI hung up")?,
                    };
                    // `response` must own a String for the IPC codec; the
                    // Zeroizing source is wiped when `pw` drops below.
                    let result = Request::PostAuthMessageResponse { response: Some(pw.to_string()) }
                        .write_to(&mut session.stream)
                        .context("write PostAuthMessageResponse");
                    drop(pw);
                    result?;
                }
                AuthMessageType::Info | AuthMessageType::Error => {
                    log::info!("greetd msg ({:?}): {}", auth_message_type, auth_message);
                    if auth_message.to_lowercase().contains("finger") {
                        let _ = events.send(AuthEvent::Fingerprint(auth_message.clone()));
                        wake();
                    }
                    Request::PostAuthMessageResponse { response: None }
                        .write_to(&mut session.stream)
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
    Request::StartSession { cmd: cmd.to_vec(), env }
        .write_to(&mut session.stream)
        .context("write StartSession")?;
    match Response::read_from(&mut session.stream).context("read StartSession response")? {
        Response::Success => {
            session.succeeded = true;
            Ok(true)
        }
        other => {
            log::warn!("StartSession failed: {:?}", other);
            Err(anyhow!("Could not start session. Please try again."))
        }
    }
}

/// Spawn the conversation worker. Returned sender delivers password
/// submissions; the worker owns retries until the session starts.
pub fn spawn(
    username: String,
    cmd: Vec<String>,
    show_username: bool,
    events: Sender<AuthEvent>,
    wake: impl Fn() + Send + 'static,
) -> (Sender<Zeroizing<String>>, JoinHandle<()>) {
    let (pw_tx, pw_rx) = mpsc::channel::<Zeroizing<String>>();
    let handle = std::thread::spawn(move || loop {
        match run_attempt(&username, &cmd, show_username, &events, &wake, &pw_rx) {
            Ok(true) => {
                let _ = events.send(AuthEvent::SessionStarted);
                wake();
                return;
            }
            Ok(false) => continue,
            Err(e) => {
                // The UI dropped its sender — rebind to another user or
                // shutdown. The session was cancelled on drop; exit so we
                // don't reconnect and fight the replacement worker over
                // greetd's single session.
                if matches!(pw_rx.try_recv(), Err(TryRecvError::Disconnected)) {
                    return;
                }
                log::warn!("auth attempt error: {e:#}");
                let _ = events.send(AuthEvent::Failed(format!("{e:#}")));
                wake();
                // Transport errors (greetd restart, socket hiccup):
                // back off briefly instead of spinning.
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    });
    (pw_tx, handle)
}
