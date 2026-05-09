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
    closed: bool,
}

impl LockBinding {
    pub(crate) fn new(lock: ExtSessionLockV1) -> Self {
        Self {
            lock,
            locked: false,
            finished: false,
            closed: false,
        }
    }

    /// Idempotent. `unlock_and_destroy` is only legal once `Locked` has been seen.
    pub(crate) fn close(&mut self) {
        if self.closed {
            return;
        }
        if self.locked {
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
