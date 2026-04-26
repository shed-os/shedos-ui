//! Wayland-native fullscreen overlay renderer for shedos-screensaver.
//!
//! The renderer attaches an `wlr-layer-shell-unstable-v1` surface at
//! `Layer::Overlay` with `keyboard_interactivity = Exclusive` and
//! `exclusive_zone = -1`, so it covers every other surface and grabs
//! all input. Buffers are CPU-rasterized into a `wl_shm` pool: each
//! frame, the cell grid is rendered through a fontdue-baked DejaVu
//! Sans Mono atlas onto an optional dimmed wallpaper backdrop, then
//! the resulting RGBA framebuffer is committed to the surface.
//!
//! Why CPU instead of wgpu: the visual budget is text-cell granularity
//! at terminal-typical density (≤ a few thousand cells), which the
//! CPU handles comfortably at 60 fps for any reasonable display. Skip-
//! ping wgpu cuts ~7 MB of binary size, drops the vulkan-icd-loader
//! runtime dep, and removes the entire class of "GPU not available
//! on this iGPU" failures. If perf becomes an issue we can revisit.

mod font;
mod surface;
mod wallpaper;

pub use font::FontAtlas;
pub use surface::WaylandRenderer;
pub use wallpaper::Wallpaper;

use shedos_screensaver_core::{Color, Frame};
use std::path::PathBuf;

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
            wallpaper_dim: 0.3,
            fps_cap: 60,
        idle_daemon: false,
        }
    }
}

/// What the renderer can ask of the caller every frame.
pub trait FrameProducer {
    /// Render the next animation frame into `frame`. The renderer hands
    /// over the canvas dimensions through `frame.rows()` / `frame.cols()`;
    /// the producer must respect them.
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
