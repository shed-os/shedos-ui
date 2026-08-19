//! Concrete palette, wallpaper paths, and fonts loaded from
//! `/etc/shedos/themes/current/greeter.toml`. Per-field fallback to
//! bundled defaults keeps the surface paintable when the theme dir
//! is missing or partially corrupt.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Only schema version 1 is consumed today. Newer schemas land in
/// the reconciler first and are refused here to avoid misinterpreting
/// fields.
const ACCEPTED_SCHEMA_VERSION: i64 = 1;

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
    pub accent_secondary: u32,
    pub yellow: u32,
    pub mantle: u32,
    pub surface0: u32,
    pub surface2: u32,
    pub overlay1: u32,
    /// Wordmark variant for dark backgrounds (blue "Shed" + white "os").
    pub wordmark_on_dark: PathBuf,
    /// Wordmark variant for light backgrounds (blue "Shed" + black "os").
    pub wordmark_on_light: PathBuf,
}

#[derive(Debug, Default, Deserialize)]
struct GreeterToml {
    output_schema_version: Option<i64>,
    wallpaper: Option<String>,
    wallpaper_blurred: Option<String>,
    fonts: Option<GreeterFonts>,
    colors: Option<GreeterColors>,
}

#[derive(Debug, Default, Deserialize)]
struct GreeterFonts {
    ui: Option<String>,
    mono: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct GreeterColors {
    base: Option<String>,
    text: Option<String>,
    accent: Option<String>,
    red: Option<String>,
    accent_secondary: Option<String>,
    yellow: Option<String>,
    mantle: Option<String>,
    surface0: Option<String>,
    surface2: Option<String>,
    overlay1: Option<String>,
}

impl Theme {
    pub const CURRENT_DIR: &'static str = "/etc/shedos/themes/current";

    /// Bundled defaults so surfaces paint when the theme dir is
    /// missing entirely.
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
            accent: 0xFFA6E3A1,
            red: 0xFFF38BA8,
            accent_secondary: 0xFF94E2D5,
            yellow: 0xFFF9E2AF,
            mantle: 0xFF181825,
            surface0: 0xFF313244,
            surface2: 0xFF585B70,
            overlay1: 0xFF7F849C,
            wordmark_on_dark: PathBuf::from(
                "/usr/share/shedos/shedos-wordmark-on-dark.png",
            ),
            wordmark_on_light: PathBuf::from(
                "/usr/share/shedos/shedos-wordmark-on-light.png",
            ),
        }
    }

    /// Load from `/etc/shedos/themes/current/greeter.toml` with
    /// per-field fallback to bundled defaults.
    pub fn load_or_default() -> Self {
        Self::load_or_default_from(Path::new(Self::CURRENT_DIR))
    }

    /// Load from `<dir>/greeter.toml`. Test-friendly: caller picks
    /// the directory.
    pub fn load_or_default_from(dir: &Path) -> Self {
        let path = dir.join("greeter.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                log::warn!(
                    "theme: cannot read {}: {} — using bundled defaults",
                    path.display(),
                    e
                );
                return Self::fallback();
            }
        };
        let parsed: GreeterToml = match toml::from_str(&text) {
            Ok(p) => p,
            Err(e) => {
                log::warn!(
                    "theme: parse error in {}: {} — using bundled defaults",
                    path.display(),
                    e
                );
                return Self::fallback();
            }
        };
        if parsed.output_schema_version != Some(ACCEPTED_SCHEMA_VERSION) {
            log::warn!(
                "theme: {} schema version {:?} not supported (want {}) — using bundled",
                path.display(),
                parsed.output_schema_version,
                ACCEPTED_SCHEMA_VERSION
            );
            return Self::fallback();
        }
        Self::merge(parsed)
    }

    fn merge(parsed: GreeterToml) -> Self {
        let fb = Self::fallback();
        let wallpaper = parsed
            .wallpaper
            .as_deref()
            .and_then(|p| validate_readable_path(p).map(PathBuf::from))
            .unwrap_or_else(|| {
                log_fallback("wallpaper", parsed.wallpaper.as_deref());
                fb.wallpaper.clone()
            });
        let wallpaper_blurred = parsed
            .wallpaper_blurred
            .as_deref()
            .and_then(|p| validate_readable_path(p).map(PathBuf::from))
            .unwrap_or_else(|| {
                log_fallback("wallpaper_blurred", parsed.wallpaper_blurred.as_deref());
                fb.wallpaper_blurred.clone()
            });
        let fonts = parsed.fonts.unwrap_or_default();
        let colors = parsed.colors.unwrap_or_default();
        Self {
            wallpaper,
            wallpaper_blurred,
            font_ui: fonts.ui.unwrap_or_else(|| fb.font_ui.clone()),
            font_mono: fonts.mono.unwrap_or_else(|| fb.font_mono.clone()),
            base: parse_hex_or_fallback("base", colors.base.as_deref(), fb.base),
            text: parse_hex_or_fallback("text", colors.text.as_deref(), fb.text),
            accent: parse_hex_or_fallback("accent", colors.accent.as_deref(), fb.accent),
            red: parse_hex_or_fallback("red", colors.red.as_deref(), fb.red),
            accent_secondary: parse_hex_or_fallback(
                "accent_secondary",
                colors.accent_secondary.as_deref(),
                fb.accent_secondary,
            ),
            yellow: parse_hex_or_fallback("yellow", colors.yellow.as_deref(), fb.yellow),
            mantle: parse_hex_or_fallback("mantle", colors.mantle.as_deref(), fb.mantle),
            surface0: parse_hex_or_fallback(
                "surface0",
                colors.surface0.as_deref(),
                fb.surface0,
            ),
            surface2: parse_hex_or_fallback(
                "surface2",
                colors.surface2.as_deref(),
                fb.surface2,
            ),
            overlay1: parse_hex_or_fallback(
                "overlay1",
                colors.overlay1.as_deref(),
                fb.overlay1,
            ),
            wordmark_on_dark: fb.wordmark_on_dark.clone(),
            wordmark_on_light: fb.wordmark_on_light.clone(),
        }
    }
}

fn validate_readable_path(p: &str) -> Option<&str> {
    let path = Path::new(p);
    if path.is_file() {
        Some(p)
    } else {
        None
    }
}

fn log_fallback(field: &str, value: Option<&str>) {
    log::warn!(
        "theme: {} {:?} unreadable — using bundled fallback",
        field,
        value
    );
}

/// Parse `#rrggbb` (case-insensitive) into 0xFFRRGGBB. None or
/// malformed → log + fallback.
fn parse_hex_or_fallback(field: &str, raw: Option<&str>, fallback: u32) -> u32 {
    let Some(s) = raw else {
        return fallback;
    };
    if let Some(parsed) = parse_hex(s) {
        parsed
    } else {
        log::warn!(
            "theme: color {field}={raw:?} is not #rrggbb — using fallback {fallback:#010x}"
        );
        fallback
    }
}

fn parse_hex(s: &str) -> Option<u32> {
    let stripped = s.strip_prefix('#')?;
    if stripped.len() != 6 || !stripped.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(stripped, 16).ok()?;
    Some(0xFF000000 | n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_hex_ok() {
        assert_eq!(parse_hex("#1e1e2e"), Some(0xFF1E1E2E));
        assert_eq!(parse_hex("#FFFFFF"), Some(0xFFFFFFFF));
        assert_eq!(parse_hex("#000000"), Some(0xFF000000));
    }

    #[test]
    fn rejects_malformed_hex() {
        assert_eq!(parse_hex("1e1e2e"), None); // missing #
        assert_eq!(parse_hex("#1e1e2"), None); // too short
        assert_eq!(parse_hex("#1e1e2eg"), None); // non-hex
    }

    // shedos_palette.py's MOCHA is the canonical fallback and says it matches
    // this one, so the TUIs and the GUIs paint the same colours when the theme
    // directory is missing. The accent pair drifted apart anyway; these are the
    // engine's values, and test/release-checks holds the two files together
    // across the repositories neither of them can see.
    #[test]
    fn fallback_accents_are_the_engines() {
        let fb = Theme::fallback();
        assert_eq!(fb.accent, 0xFFA6E3A1); // Mocha green
        assert_eq!(fb.accent_secondary, 0xFF94E2D5); // Mocha teal
    }

    #[test]
    fn fallback_when_dir_missing() {
        let t = Theme::load_or_default_from(Path::new("/nonexistent/shedos-test"));
        assert_eq!(t.base, Theme::fallback().base);
    }

    #[test]
    fn loads_full_theme_when_present() {
        let dir = tempdir();
        // Write a minimal valid greeter.toml referencing this very
        // file as the "wallpaper" so the readable-path check passes.
        let dummy_wallpaper = dir.join("dummy.png");
        fs::write(&dummy_wallpaper, b"\x89PNG\r\n").unwrap();
        let toml = format!(
            r##"
output_schema_version = 1
wallpaper = "{p}"
wallpaper_blurred = "{p}"
[fonts]
ui = "TestFont 12"
mono = "TestMono"
[colors]
base = "#112233"
text = "#aabbcc"
accent = "#ff0000"
red = "#00ff00"
"##,
            p = dummy_wallpaper.display()
        );
        fs::write(dir.join("greeter.toml"), toml).unwrap();
        let t = Theme::load_or_default_from(&dir);
        assert_eq!(t.font_ui, "TestFont 12");
        assert_eq!(t.font_mono, "TestMono");
        assert_eq!(t.base, 0xFF112233);
        assert_eq!(t.text, 0xFFAABBCC);
        assert_eq!(t.accent, 0xFFFF0000);
        assert_eq!(t.red, 0xFF00FF00);
        assert_eq!(t.wallpaper, dummy_wallpaper);
        // Cleanup
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn falls_back_per_field_on_partial_corruption() {
        let dir = tempdir();
        // Valid colors + fonts, but wallpaper paths don't exist.
        let toml = r##"
output_schema_version = 1
wallpaper = "/nope/missing.png"
wallpaper_blurred = "/nope/missing-blurred.png"
[fonts]
ui = "OnlyUI"
[colors]
base = "#112233"
text = "BAD"
"##;
        fs::write(dir.join("greeter.toml"), toml).unwrap();
        let t = Theme::load_or_default_from(&dir);
        // Wallpaper paths fall back to bundled.
        assert_eq!(t.wallpaper, Theme::fallback().wallpaper);
        assert_eq!(t.wallpaper_blurred, Theme::fallback().wallpaper_blurred);
        // Provided fields stick where valid.
        assert_eq!(t.font_ui, "OnlyUI");
        assert_eq!(t.font_mono, Theme::fallback().font_mono); // missing
        assert_eq!(t.base, 0xFF112233);
        assert_eq!(t.text, Theme::fallback().text); // malformed → fallback
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn falls_back_on_unknown_schema_version() {
        let dir = tempdir();
        fs::write(
            dir.join("greeter.toml"),
            "output_schema_version = 999\n",
        )
        .unwrap();
        let t = Theme::load_or_default_from(&dir);
        assert_eq!(t.base, Theme::fallback().base);
        let _ = fs::remove_dir_all(dir);
    }

    fn tempdir() -> PathBuf {
        let unique = format!(
            "shedos-theme-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        );
        let p = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
