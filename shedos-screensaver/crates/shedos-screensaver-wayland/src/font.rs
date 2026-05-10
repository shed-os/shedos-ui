//! TrueType font baked into a glyph-bitmap cache.
//!
//! Fontdue emits a grayscale alpha bitmap per codepoint; cached so
//! repeated glyph emits are O(1).

use fontdue::{Font, FontSettings};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// System DejaVu Sans Mono paths in priority order. First match wins;
/// loading errors with `NoDefaultAvailable` if all are missing.
const DEJAVU_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
];

pub struct GlyphBitmap {
    pub width: usize,
    pub height: usize,
    /// Top-left coordinate inside the cell box (positive = down/right).
    pub x_offset: i32,
    pub y_offset: i32,
    /// Grayscale alpha, row-major.
    pub bitmap: Vec<u8>,
}

pub struct FontAtlas {
    font: Font,
    px_size: f32,
    cell_w: u32,
    cell_h: u32,
    cache: HashMap<char, GlyphBitmap>,
    /// Distance from cell top to text baseline.
    baseline: i32,
}

impl FontAtlas {
    pub fn load(path: Option<&Path>, px_size: f32) -> Result<Self, FontLoadError> {
        let bytes = match path {
            Some(p) => std::fs::read(p).map_err(|e| FontLoadError::Read {
                path: p.to_path_buf(),
                source: e,
            })?,
            None => Self::read_default()?,
        };
        let font = Font::from_bytes(bytes, FontSettings::default())
            .map_err(FontLoadError::Parse)?;

        // Cell dimensions: take the metrics of 'M' (representative
        // monospaced advance) and the font's line height.
        let metrics_m = font.metrics('M', px_size);
        let line_metrics = font.horizontal_line_metrics(px_size).unwrap_or_else(|| {
            fontdue::LineMetrics {
                ascent: px_size * 0.8,
                descent: -px_size * 0.2,
                line_gap: 0.0,
                new_line_size: px_size,
            }
        });
        let cell_w = metrics_m.advance_width.ceil() as u32;
        let cell_h = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil() as u32;
        let baseline = line_metrics.ascent.ceil() as i32;

        Ok(Self {
            font,
            px_size,
            cell_w: cell_w.max(1),
            cell_h: cell_h.max(1),
            cache: HashMap::new(),
            baseline,
        })
    }

    fn read_default() -> Result<Vec<u8>, FontLoadError> {
        for cand in DEJAVU_CANDIDATES {
            if let Ok(bytes) = std::fs::read(cand) {
                return Ok(bytes);
            }
        }
        Err(FontLoadError::NoDefaultAvailable)
    }

    pub fn cell_size(&self) -> (u32, u32) {
        (self.cell_w, self.cell_h)
    }

    pub fn baseline(&self) -> i32 {
        self.baseline
    }

    pub fn glyph(&mut self, ch: char) -> &GlyphBitmap {
        if !self.cache.contains_key(&ch) {
            let (metrics, bitmap) = self.font.rasterize(ch, self.px_size);
            self.cache.insert(
                ch,
                GlyphBitmap {
                    width: metrics.width,
                    height: metrics.height,
                    x_offset: metrics.xmin,
                    y_offset: -(metrics.ymin + metrics.height as i32),
                    bitmap,
                },
            );
        }
        self.cache.get(&ch).expect("inserted above")
    }

}

#[derive(Debug)]
pub enum FontLoadError {
    Read { path: PathBuf, source: std::io::Error },
    Parse(&'static str),
    NoDefaultAvailable,
}

impl std::fmt::Display for FontLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, source } => write!(f, "failed to read font {}: {source}", path.display()),
            Self::Parse(s) => write!(f, "fontdue could not parse font: {s}"),
            Self::NoDefaultAvailable => write!(
                f,
                "no default font found; install ttf-dejavu or pass --font-path"
            ),
        }
    }
}

impl std::error::Error for FontLoadError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn maybe_font() -> Option<FontAtlas> {
        FontAtlas::load(None, 16.0).ok()
    }

    #[test]
    fn default_font_loads_when_dejavu_present() {
        if let Some(a) = maybe_font() {
            let (w, h) = a.cell_size();
            assert!(w > 0 && h > 0, "cell dims must be positive; got {}x{}", w, h);
            assert!(a.baseline() > 0, "baseline must be positive");
        }
        // DejaVu may not be installed in CI; the runtime fallback to
        // hard error is exercised separately.
    }

    #[test]
    fn glyph_cache_populates_on_first_call() {
        let Some(mut a) = maybe_font() else { return };
        let g1 = a.glyph('A');
        assert!(g1.width > 0);
        assert!(g1.height > 0);
        assert!(!g1.bitmap.is_empty());
        let _ = a.glyph('A'); // second call hits cache
    }
}
