//! Wayland-native fullscreen overlay renderer for shedos-screensaver.
//!
//! Attaches a `wlr-layer-shell-unstable-v1` surface at `Layer::Overlay`
//! with `keyboard_interactivity = Exclusive` and `exclusive_zone = -1`
//! so it covers every other surface and grabs all input. Buffers are
//! CPU-rasterized into a `wl_shm` pool.
//!
//! CPU instead of wgpu: text-cell density (≤ a few thousand cells)
//! runs comfortably at 60 fps. Skipping wgpu also cuts ~7 MB of
//! binary size and the vulkan-icd-loader runtime dep.

mod dpms;
mod font;
mod lock;
mod surface;
mod switch;
mod wallpaper;

pub use font::FontAtlas;
pub use shedos_prompt_ui::{Theme, WidgetCache};
pub use shedos_screensaver_core::LockStateConfig;
pub use surface::{ProducerFactory, WaylandRenderer};
pub use switch::first_free_vt;
pub use wallpaper::Wallpaper;

/// Re-exports of calloop's ping primitives so the cli crate can build
/// a Ping/PingSource pair without depending on calloop. The fingerprint
/// auth thread pings the lock loop on each attempt completion.
pub mod calloop_ping {
    pub use smithay_client_toolkit::reexports::calloop::ping::{make_ping, Ping, PingSource};
}

use shedos_screensaver_core::{Color, Frame};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

pub type AuthFn = Box<dyn Fn(&str) -> Result<(), String>>;

pub struct LockConfig {
    pub authenticate: AuthFn,
    pub state_config: LockStateConfig,
    pub username: String,
    pub fingerprint: Option<FingerprintConfig>,
    /// Live ISO: unlock on any keypress without PAM. Set only when
    /// /run/archiso is present (never on an installed disk).
    pub no_auth: bool,
}

pub struct FingerprintConfig {
    /// Channel of fingerprint attempt outcomes. `Ok(())` releases the
    /// lock; `Err(())` is dropped silently (the thread already logged
    /// via stderr). No string keeps fingerprint failures from leaking
    /// into the password prompt's error slot.
    pub rx: Receiver<Result<(), ()>>,
    pub ping_source: calloop_ping::PingSource,
    pub hint_text: String,
    /// When `true`, the auth thread idles instead of calling
    /// `pam_authenticate`. Set on entry to / exit from `LockPhase::Prompt`.
    pub paused: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Configuration handed to [`WaylandRenderer::run`].
pub struct WaylandConfig {
    /// Path to a TrueType font; `None` falls back to system DejaVu Sans Mono.
    pub font_path: Option<PathBuf>,
    /// Cell pixel height; cell width is derived from the font's metrics.
    pub cell_height_px: u32,
    /// Wallpaper PNG/JPG; `None` means draw on solid `Color::BASE`.
    pub wallpaper_path: Option<PathBuf>,
    /// Multiplier on wallpaper brightness (0..=1).
    pub wallpaper_dim: f32,
    /// Frame budget; renderer will try to stay above this.
    pub fps_cap: u32,
    /// If true, ignore keyboard/pointer input and only exit on the
    /// stop flag (used for hypridle's --idle-daemon).
    pub idle_daemon: bool,
}

impl Default for WaylandConfig {
    fn default() -> Self {
        Self {
            font_path: None,
            cell_height_px: 18,
            wallpaper_path: None,
            wallpaper_dim: 0.5,
            fps_cap: 60,
            idle_daemon: false,
        }
    }
}

/// What the renderer asks of the caller per frame.
pub trait FrameProducer {
    /// Render the next animation frame into `frame`. Use the canvas
    /// dimensions from `frame.rows()` / `frame.cols()`.
    fn produce(&mut self, frame: &mut Frame);
}

/// Color → 32-bit ARGB (premultiplied alpha-1.0). Wayland's wl_shm
/// `Argb8888` format is what Hyprland exposes by default.
#[inline]
pub(crate) fn pack_argb(c: Color) -> u32 {
    0xff00_0000 | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)
}

#[inline]
pub(crate) fn unpack_argb(p: u32) -> (u8, u8, u8) {
    (((p >> 16) & 0xff) as u8, ((p >> 8) & 0xff) as u8, (p & 0xff) as u8)
}

#[inline]
pub(crate) fn blend_over(fg: Color, bg_argb: u32, alpha: u8) -> u32 {
    if alpha == 0 {
        return bg_argb;
    }
    if alpha == 255 {
        return pack_argb(fg);
    }
    let (br, bg_, bb) = unpack_argb(bg_argb);
    let a = alpha as u32;
    let inv = 255 - a;
    let r = ((fg.r as u32 * a + br as u32 * inv) / 255) as u8;
    let g = ((fg.g as u32 * a + bg_ as u32 * inv) / 255) as u8;
    let b = ((fg.b as u32 * a + bb as u32 * inv) / 255) as u8;
    pack_argb(Color::rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shedos_screensaver_core::Color;

    #[test]
    fn pack_unpack_roundtrip() {
        let c = Color::rgb(0x12, 0x34, 0x56);
        let p = pack_argb(c);
        assert_eq!(p & 0xff00_0000, 0xff00_0000);
        let (r, g, b) = unpack_argb(p);
        assert_eq!((r, g, b), (0x12, 0x34, 0x56));
    }

    #[test]
    fn blend_over_zero_alpha_keeps_bg() {
        let bg = pack_argb(Color::rgb(10, 20, 30));
        let out = blend_over(Color::rgb(255, 255, 255), bg, 0);
        assert_eq!(out, bg);
    }

    #[test]
    fn blend_over_full_alpha_replaces_with_fg() {
        let bg = pack_argb(Color::rgb(10, 20, 30));
        let fg = Color::rgb(200, 100, 50);
        let out = blend_over(fg, bg, 255);
        assert_eq!(out, pack_argb(fg));
    }

    #[test]
    fn blend_over_half_alpha_mixes() {
        let bg = pack_argb(Color::rgb(0, 0, 0));
        let fg = Color::rgb(200, 200, 200);
        let out = blend_over(fg, bg, 128);
        // Roughly half-mix: each channel near 100.
        let (r, g, b) = unpack_argb(out);
        assert!((95..=105).contains(&r));
        assert!((95..=105).contains(&g));
        assert!((95..=105).contains(&b));
    }
}
