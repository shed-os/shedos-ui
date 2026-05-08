//! Shared rendering for ShedOS lock surfaces — the greeter
//! (`shedos-greeter`) and the screensaver-as-lock-client
//! (`shedos-screensaver --mode=lock`) both draw the same widgets
//! through this crate so the two surfaces stay pixel-identical
//! and react to `shedman theme set` the same way.
//!
//! The crate is stateless apart from a `WidgetCache` (font + scaled
//! wallpaper). Callers own their wl_shm buffer, theme, and prompt
//! state; this crate composites widgets onto the byte buffer and
//! returns. Surface lifecycle (commit, frame callbacks, damage)
//! stays with the caller.

#![forbid(unsafe_code)]

use anyhow::Result;

pub mod primitives;
pub mod text;
pub mod theme;
pub mod wallpaper;
pub mod widgets;

pub use theme::Theme;

use text::{FontFace, JBM_BOLD_CANDIDATES, JBM_REGULAR_CANDIDATES};
use wallpaper::Wallpaper;

/// Caller-owned prompt input state. Never holds the typed password —
/// only the count of characters typed (so the renderer can paint
/// dots) plus a few flags.
#[derive(Debug, Clone, Default)]
pub struct PromptState {
    pub typed_chars: usize,
    pub fail: bool,
    pub success: bool,
    pub capslock: bool,
}

/// Logical rect (in canvas-local pixels) where one output sits. Used
/// to centre widgets per output for multi-monitor mirror rendering.
#[derive(Debug, Clone, Copy)]
pub struct OutputRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Heavy state that's expensive to rebuild on every redraw: font
/// faces (loaded once) and the per-surface wallpaper cache. Caller
/// constructs this once, reuses it across frames, drops it on
/// surface teardown.
pub struct WidgetCache {
    pub regular: FontFace,
    pub bold: FontFace,
    wallpaper: Wallpaper,
}

impl WidgetCache {
    /// Load fonts + decode the (blurred) wallpaper from the theme.
    /// Use the blurred wallpaper for lock surfaces — it's softened
    /// so the prompt UI reads cleanly. Desktop wallpaper daemons
    /// (awww) read the sharp `theme.wallpaper` directly, not via
    /// this cache.
    pub fn new(theme: &Theme) -> Result<Self> {
        let regular = FontFace::load(JBM_REGULAR_CANDIDATES)?;
        let bold = FontFace::load(JBM_BOLD_CANDIDATES)?;
        let wallpaper = Wallpaper::load(&theme.wallpaper_blurred)?;
        Ok(Self { regular, bold, wallpaper })
    }

    /// Re-decode the wallpaper if the theme path changed (e.g. live
    /// `shedman theme set` while the surface is up).
    pub fn refresh_wallpaper(&mut self, theme: &Theme) -> Result<()> {
        if self.wallpaper.source_path() != theme.wallpaper_blurred {
            self.wallpaper = Wallpaper::load(&theme.wallpaper_blurred)?;
        }
        Ok(())
    }
}

/// Per-frame render parameters that consumers tweak between calls.
/// Kept as a struct so adding fields (e.g. accessibility hints,
/// override greeting) doesn't churn the call sites.
#[derive(Debug, Clone, Default)]
pub struct RenderParams<'a> {
    /// Override the "Hi, $user" greeting. None → no greeting line.
    pub greeting: Option<&'a str>,
    /// Show this error message in red below the prompt instead of
    /// the greeting. None → no error.
    pub error_message: Option<&'a str>,
}

/// Paint the wallpaper across the whole canvas, then mirror the
/// widgets onto each output's rect. `outputs` should hold one rect
/// per `wl_output` (single-monitor → a single rect equal to the
/// canvas; multi-monitor → one rect per physical output, all
/// rendered identically per the no-dimming spec).
///
/// Caller's wl_shm buffer is wl_shm::Format::Argb8888 (BGRA byte
/// order on little-endian); `canvas` length must be at least
/// `canvas_w * canvas_h * 4`.
pub fn render(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    outputs: &[OutputRect],
    state: &PromptState,
    theme: &Theme,
    cache: &mut WidgetCache,
    params: &RenderParams<'_>,
) {
    if canvas_w == 0 || canvas_h == 0 {
        return;
    }
    let need = (canvas_w as usize) * (canvas_h as usize) * 4;
    if canvas.len() < need {
        return;
    }
    cache.wallpaper.blit(canvas, canvas_w, canvas_h);
    let fallback_rect = OutputRect {
        x: 0,
        y: 0,
        w: canvas_w as i32,
        h: canvas_h as i32,
    };
    let rects: &[OutputRect] = if outputs.is_empty() {
        std::slice::from_ref(&fallback_rect)
    } else {
        outputs
    };
    for rect in rects {
        widgets::paint_widgets(
            canvas,
            canvas_w,
            canvas_h,
            rect,
            state,
            theme,
            &cache.regular,
            &cache.bold,
            params.error_message,
            params.greeting,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_theme_has_opaque_alpha() {
        let t = Theme::fallback();
        for c in [t.base, t.text, t.accent, t.red] {
            assert_eq!(c & 0xFF000000, 0xFF000000, "color {c:#010x} not opaque");
        }
    }

    #[test]
    fn fallback_paths_resolve_to_shipped_assets() {
        let t = Theme::fallback();
        assert!(t.wallpaper.starts_with("/usr/share/shedos"));
        assert!(t.wallpaper_blurred.starts_with("/usr/share/shedos"));
    }

    #[test]
    fn output_rect_is_copy() {
        let r = OutputRect { x: 0, y: 0, w: 100, h: 100 };
        let _r2 = r; // needs Copy
    }
}
