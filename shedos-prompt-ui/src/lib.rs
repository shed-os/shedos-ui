//! Shared widget rendering for the greeter and the screensaver lock
//! client. Both draw the same widgets through this crate so they stay
//! pixel-identical and react to `shedman theme set` the same way.
//!
//! Stateless except for `WidgetCache` (font + scaled wallpaper).
//! Callers own their wl_shm buffer; this crate paints widgets onto
//! it and returns.

#![forbid(unsafe_code)]

use anyhow::Result;

pub mod power;
pub mod primitives;
pub mod text;
pub mod theme;
pub mod wallpaper;
pub mod watch;
pub mod widgets;
pub mod wordmark;

pub use power::{PowerAction, PowerHit, PowerMenuState};
pub use theme::Theme;

use text::{FontFace, JBM_BOLD_CANDIDATES, JBM_REGULAR_CANDIDATES};
use wallpaper::Wallpaper;
use wordmark::Wordmark;

/// Prompt input state. Holds the count of typed characters (for dot
/// rendering) and a few flags. Never the typed password itself.
#[derive(Debug, Clone, Default)]
pub struct PromptState {
    pub typed_chars: usize,
    pub fail: bool,
    pub success: bool,
    pub capslock: bool,
    pub power_menu: PowerMenuState,
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

/// Heavy state expensive to rebuild per frame: font faces, the
/// per-surface wallpaper cache, and the brand wordmark. Build once,
/// reuse across frames.
pub struct WidgetCache {
    pub regular: FontFace,
    pub bold: FontFace,
    wallpaper: Wallpaper,
    pub wordmark: Wordmark,
}

impl WidgetCache {
    /// Load fonts, decode the blurred wallpaper, then sample the
    /// wallpaper's average luminance to pick the matching wordmark
    /// variant (light glyphs on dark wallpaper / dark glyphs on
    /// light wallpaper). The blurred variant softens the background
    /// so the prompt reads cleanly; desktop wallpaper daemons read
    /// `theme.wallpaper` directly.
    pub fn new(theme: &Theme) -> Result<Self> {
        let regular = FontFace::load(JBM_REGULAR_CANDIDATES)?;
        let bold = FontFace::load(JBM_BOLD_CANDIDATES)?;
        let wallpaper = Wallpaper::load(&theme.wallpaper_blurred)?;
        let wordmark_path = if wallpaper.is_average_dark() {
            &theme.wordmark_on_dark
        } else {
            &theme.wordmark_on_light
        };
        let wordmark = Wordmark::load(wordmark_path)?;
        Ok(Self { regular, bold, wallpaper, wordmark })
    }

    /// Re-decode the wallpaper if the theme path changed (e.g. live
    /// `shedman theme set` while the surface is up). Also re-picks
    /// the wordmark variant if the wallpaper's luminance class
    /// flipped (dark ↔ light), so the wordmark stays legible
    /// against the new background.
    pub fn refresh_wallpaper(&mut self, theme: &Theme) -> Result<()> {
        if self.wallpaper.source_path() != theme.wallpaper_blurred {
            self.wallpaper = Wallpaper::load(&theme.wallpaper_blurred)?;
            let wordmark_path = if self.wallpaper.is_average_dark() {
                &theme.wordmark_on_dark
            } else {
                &theme.wordmark_on_light
            };
            if self.wordmark.source_path() != wordmark_path.as_path() {
                self.wordmark = Wordmark::load(wordmark_path)?;
            }
        }
        Ok(())
    }
}

/// Per-frame render parameters. Struct-shaped so adding fields
/// doesn't churn call sites.
#[derive(Debug, Clone, Default)]
pub struct RenderParams<'a> {
    /// Override the greeting. None → no greeting line.
    pub greeting: Option<&'a str>,
    /// Error message in red, taking the greeting's slot. None → no error.
    pub error_message: Option<&'a str>,
    /// Fingerprint icon and hint. None → no fingerprint affordance.
    pub fingerprint: Option<FingerprintRender<'a>>,
}

/// Per-frame fingerprint affordance state. Caller picks the color
/// and hint from auth-thread state so the renderer stays presentational.
#[derive(Debug, Clone, Copy)]
pub struct FingerprintRender<'a> {
    pub hint: &'a str,
    pub icon_color_argb: u32,
}

/// Paint wallpaper and widgets for every output rect. Each output
/// gets a correctly-aspected wallpaper (no stretching across cage's
/// spanned canvas) and a mirrored widget set. With `outputs` empty,
/// renders once at the full canvas.
///
/// `canvas` is wl_shm Argb8888 (BGRA on little-endian); length must
/// be at least `canvas_w * canvas_h * 4`.
#[allow(clippy::too_many_arguments)]
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
    // Wallpapers first, then widgets, so overlays aren't clobbered
    // if rects share boundary pixels.
    for rect in rects {
        cache.wallpaper.blit_rect(canvas, canvas_w, canvas_h, rect);
    }
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
            &mut cache.wordmark,
            params.error_message,
            params.greeting,
            params.fingerprint.as_ref(),
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
