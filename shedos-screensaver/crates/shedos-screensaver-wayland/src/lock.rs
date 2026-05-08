//! `ext-session-lock-v1` lock-client plumbing.
//!
//! sctk 0.19 does not expose a `SessionLockState` helper, so we drive
//! the three-object handshake manually via `wayland-protocols`:
//!
//!   1. Bind `ext_session_lock_manager_v1` from the registry.
//!   2. Call `lock()` to obtain `ext_session_lock_v1`. The compositor
//!      replies with either `Locked` (success) or `Finished` (denied or
//!      another client already holds the lock).
//!   3. Per output, call `get_lock_surface(wl_surface, wl_output)` to
//!      obtain `ext_session_lock_surface_v1`, then ack each `configure`
//!      with the same serial. Once acked, the surface is the only thing
//!      the compositor will composite — every other surface is hidden
//!      until `unlock_and_destroy()` fires.
//!
//! This module is the *plumbing*: a `LockHandle` type and a `claim()`
//! constructor with the bind+lock dance. Phase 1.3 fills in the
//! per-output surface wiring, render loop, and unlock path. Phase 2
//! adds PAM auth on top.

use crate::surface::WaylandError;

/// Owned handle to an active `ext_session_lock_v1`. Drop releases the
/// lock via `unlock_and_destroy` — a panic anywhere in lock-mode
/// returns the user to the desktop instead of leaving the seat
/// permanently locked.
pub struct LockHandle {
    _private: (),
}

impl LockHandle {
    /// Bind `ext_session_lock_manager_v1`, send `lock`, wait for
    /// the compositor's `Locked` reply. Phase 1.3 fills in the
    /// handshake; this stub keeps the public API stable.
    pub fn claim() -> Result<Self, WaylandError> {
        Err(WaylandError::Bind(
            "LockHandle::claim() not yet implemented (Phase 1.3)".into(),
        ))
    }
}

impl Drop for LockHandle {
    fn drop(&mut self) {
        // Phase 1.3 sends `unlock_and_destroy` here.
    }
}
