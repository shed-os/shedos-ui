//! Per-output power management via `wlr-output-power-management-v1`.
//!
//! Caller binds the manager once, asks for one power object per
//! output, and sends `set_mode(On|Off)` to drive the panel. The
//! compositor's response is informational — `mode` confirms the
//! transition; `failed` means it refused, after which the proxy is
//! inert and further requests are no-ops.

use crate::surface::AppState;
use wayland_client::{globals::GlobalList, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::output_power_management::v1::client::{
    zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1,
    zwlr_output_power_v1::{self, ZwlrOutputPowerV1},
};

#[allow(dead_code)]
pub(crate) fn bind_manager(
    globals: &GlobalList,
    qh: &QueueHandle<AppState>,
) -> Option<ZwlrOutputPowerManagerV1> {
    globals.bind(qh, 1..=1, ()).ok()
}

impl Dispatch<ZwlrOutputPowerManagerV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrOutputPowerManagerV1,
        _event: <ZwlrOutputPowerManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrOutputPowerV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        proxy: &ZwlrOutputPowerV1,
        event: <ZwlrOutputPowerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let zwlr_output_power_v1::Event::Failed = event {
            eprintln!(
                "shedos-screensaver: dpms power management failed for {:?}; \
                 monitors will not power off for that output",
                proxy.id()
            );
        }
    }
}
