//! Manual `ext-session-lock-v1` handshake (sctk 0.19 has no helper).

use crate::surface::AppState;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1::ExtSessionLockManagerV1,
    ext_session_lock_surface_v1::{self, ExtSessionLockSurfaceV1},
    ext_session_lock_v1::{self, ExtSessionLockV1},
};

pub(crate) struct LockBinding {
    pub(crate) lock: ExtSessionLockV1,
    pub(crate) locked: bool,
    pub(crate) finished: bool,
    /// Only a successful auth unlocks; any other teardown stays locked.
    pub(crate) authenticated: bool,
    closed: bool,
}

/// `unlock_and_destroy` is valid only while locked, authed, and not
/// finished (after which it's a protocol error). Pure for testing.
pub(crate) fn should_unlock(locked: bool, authenticated: bool, finished: bool) -> bool {
    locked && authenticated && !finished
}

/// A clean loop exit that neither authenticated nor was ended by the
/// compositor is reported as failure so the unit respawns and re-locks.
pub(crate) fn exit_is_failure(authenticated: bool, finished: bool) -> bool {
    !authenticated && !finished
}

impl LockBinding {
    pub(crate) fn new(lock: ExtSessionLockV1) -> Self {
        Self {
            lock,
            locked: false,
            finished: false,
            authenticated: false,
            closed: false,
        }
    }

    /// Idempotent. Unlocks only after auth; otherwise destroys, staying locked.
    pub(crate) fn close(&mut self) {
        if self.closed {
            return;
        }
        if should_unlock(self.locked, self.authenticated, self.finished) {
            self.lock.unlock_and_destroy();
        } else {
            self.lock.destroy();
        }
        self.closed = true;
    }
}

impl Drop for LockBinding {
    fn drop(&mut self) {
        self.close();
    }
}

impl Dispatch<ExtSessionLockManagerV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &ExtSessionLockManagerV1,
        _event: <ExtSessionLockManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtSessionLockV1, ()> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &ExtSessionLockV1,
        event: <ExtSessionLockV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let Some(lb) = state.lock_binding.as_mut() {
            match event {
                ext_session_lock_v1::Event::Locked => lb.locked = true,
                ext_session_lock_v1::Event::Finished => lb.finished = true,
                _ => {}
            }
        }
    }
}

impl Dispatch<ExtSessionLockSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        proxy: &ExtSessionLockSurfaceV1,
        event: <ExtSessionLockSurfaceV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let ext_session_lock_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            proxy.ack_configure(serial);
            state.apply_lock_surface_configure(proxy, width, height);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::should_unlock;

    #[test]
    fn locked_without_auth_never_unlocks() {
        // Error/signal teardown (locked, not authed) must not unlock.
        assert!(!should_unlock(true, false, false));
    }

    #[test]
    fn authenticated_unlock() {
        assert!(should_unlock(true, true, false));
    }

    #[test]
    fn never_unlock_before_locked() {
        assert!(!should_unlock(false, true, false));
    }

    #[test]
    fn never_unlock_after_finished() {
        // After `finished`, unlock_and_destroy is a protocol error.
        assert!(!should_unlock(true, true, true));
    }

    #[test]
    fn signal_exit_fails_to_respawn() {
        use super::exit_is_failure;
        assert!(exit_is_failure(false, false)); // signal/error → respawn
        assert!(!exit_is_failure(true, false)); // authed unlock → done
        assert!(!exit_is_failure(false, true)); // compositor ended → done
    }
}
