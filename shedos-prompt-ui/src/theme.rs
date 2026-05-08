//! Theme: concrete palette + wallpaper paths + fonts. Phase 0.3 will
//! teach `Theme::load_or_default()` to read
//! `/etc/shedos/themes/current/greeter.toml` and fall back to the
//! bundled defaults when the theme dir is missing or corrupt.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Theme {
    pub wallpaper: PathBuf,
    pub wallpaper_blurred: PathBuf,
    pub font_ui: String,
    pub font_mono: String,
    /// 0xAARRGGBB packed.
    pub base: u32,
    pub text: u32,
    pub accent: u32,
    pub red: u32,
}

impl Theme {
    /// Bundled-into-the-binary defaults. Robust safety net so the
    /// surface always paints *something*, even when
    /// `/etc/shedos/themes/current/` is missing or corrupt.
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
