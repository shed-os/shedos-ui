//! Shared rendering for ShedOS lock surfaces — the greeter
//! (`shedos-greeter`) and the screensaver-as-lock-client
//! (`shedos-screensaver --mode=lock`) draw the same widgets through
//! this crate so the two surfaces stay pixel-identical and both
//! react to `shedman theme set` the same way.
//!
//! The crate is stateless. The caller owns its wl_shm buffer, theme,
//! and prompt state; this crate paints widgets into a `u32` row-major
//! ARGB buffer at the supplied dimensions and returns. Surface
//! lifecycle (commit, frame callbacks, damage tracking) is the
//! caller's concern.
//!
//! Phase 0.1 ships only the scaffold — the public types, fallback
//! theme, and a paint-the-background-color render. Phase 0.2 ports
//! the wallpaper blit, big clock, prompt input, and branding label
//! from `shedos-greeter`.

#![forbid(unsafe_code)]

use std::path::PathBuf;

/// Caller-owned prompt input state. Never holds the typed password —
/// only the count of characters typed (so the renderer can paint
/// dots) and a few flags.
#[derive(Debug, Clone, Default)]
pub struct PromptState {
    /// Number of characters in the password buffer.
    pub typed_chars: usize,
    /// True after a failed authentication; the prompt flashes the
    /// fail color and the caller is expected to clear `typed_chars`.
    pub fail: bool,
    /// True briefly after a successful unlock has been scheduled, so
    /// the prompt can show a check-color flash before the surface
    /// goes away.
    pub success: bool,
    /// Caps-lock currently engaged; surfaces a small indicator.
    pub capslock: bool,
}

/// Concrete theme values resolved by the caller.
///
/// Greeter and lock screen both load this via
/// `Theme::load_or_default()` (Phase 0.3) which reads
/// `/etc/shedos/themes/current/greeter.toml` and falls back to
/// `Theme::fallback()` if the theme dir is missing or corrupt.
#[derive(Debug, Clone)]
pub struct Theme {
    pub wallpaper: PathBuf,
    pub wallpaper_blurred: PathBuf,
    pub font_ui: String,
    pub font_mono: String,
    /// All four colors are 0xAARRGGBB (alpha in the high byte).
    pub base: u32,
    pub text: u32,
    pub accent: u32,
    pub red: u32,
}

impl Theme {
    /// Bundled-into-the-binary defaults. Robust safety net per the
    /// Phase 0.3 spec: greeter and lock screen always render
    /// *something*, even when `/etc/shedos/themes/current/` is
    /// missing or corrupt.
    pub fn fallback() -> Self {
        Self {
            wallpaper: PathBuf::from("/usr/share/shedos/wallpapers/dusk.png"),
            wallpaper_blurred: PathBuf::from(
                "/usr/share/shedos/wallpapers/dusk-blurred.png",
            ),
            font_ui: "Inter 11".to_string(),
            font_mono: "JetBrainsMono Nerd Font".to_string(),
            base: 0xFF1E1E2E,
            text: 0xFFCDD6F4,
            accent: 0xFF89B4FA,
            red: 0xFFF38BA8,
        }
    }
}

/// Render the prompt UI into the supplied buffer.
///
/// `buffer` is row-major in native-endian ARGB; `dim` is `(width,
/// height)` in pixels. The caller must provide at least `width *
/// height` `u32`s. Smaller buffers return without writing — never
/// panic.
pub fn render(
    buffer: &mut [u32],
    dim: (u32, u32),
    _state: &PromptState,
    theme: &Theme,
) {
    let expected = (dim.0 as usize)
        .checked_mul(dim.1 as usize)
        .unwrap_or(0);
    if expected == 0 || buffer.len() < expected {
        return;
    }
    // Phase 0.1 scaffold: paint the theme's base color so consumers
    // can see the crate is wired correctly. Phase 0.2 replaces this
    // with wallpaper blit + widgets.
    for px in &mut buffer[..expected] {
        *px = theme.base;
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
    fn render_fills_the_buffer_with_base_color() {
        let dim = (4, 3);
        let mut buf = vec![0u32; (dim.0 * dim.1) as usize];
        let theme = Theme::fallback();
        render(&mut buf, dim, &PromptState::default(), &theme);
        assert!(buf.iter().all(|&px| px == theme.base));
    }

    #[test]
    fn render_is_safe_with_undersized_buffer() {
        let dim = (16, 16);
        let mut buf = vec![0u32; 4]; // way too small
        render(&mut buf, dim, &PromptState::default(), &Theme::fallback());
        // Must not panic; buffer untouched.
        assert!(buf.iter().all(|&px| px == 0));
    }

    #[test]
    fn render_is_safe_with_zero_dim() {
        let mut buf = vec![0u32; 100];
        render(&mut buf, (0, 0), &PromptState::default(), &Theme::fallback());
        // No write — buffer unchanged.
        assert!(buf.iter().all(|&px| px == 0));
    }
}
