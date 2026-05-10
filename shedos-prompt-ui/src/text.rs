//! Text rendering: fontdue glyph composition onto wl_shm Argb8888
//! (BGRA byte order on little-endian) with alpha blending.

use anyhow::{anyhow, Context, Result};
use fontdue::{Font, FontSettings};

/// Arch ships JetBrainsMono Nerd Font at /usr/share/fonts/TTF/. Other
/// distros may shuffle paths; the fallback list keeps the binary
/// portable to a hand-installed font.
pub const JBM_REGULAR_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf",
    "/usr/share/fonts/jetbrains-mono-nerd/JetBrainsMonoNerdFont-Regular.ttf",
    "/usr/share/fonts/JetBrainsMono/JetBrainsMonoNerdFont-Regular.ttf",
];

pub const JBM_BOLD_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Bold.ttf",
    "/usr/share/fonts/jetbrains-mono-nerd/JetBrainsMonoNerdFont-Bold.ttf",
    "/usr/share/fonts/JetBrainsMono/JetBrainsMonoNerdFont-Bold.ttf",
];

pub struct FontFace {
    font: Font,
}

impl FontFace {
    pub fn load(candidates: &[&str]) -> Result<Self> {
        for path in candidates {
            if let Ok(bytes) = std::fs::read(path) {
                let font = Font::from_bytes(bytes, FontSettings::default())
                    .map_err(|e| anyhow!("fontdue parse failure: {e}"))?;
                log::info!("loaded font: {}", path);
                return Ok(Self { font });
            }
        }
        Err(anyhow!(
            "no font found among {:?}; install ttf-jetbrains-mono-nerd",
            candidates
        ))
        .context("FontFace::load")
    }

    /// Width-in-pixels of `text` rendered at `px` size.
    pub fn measure_width(&self, text: &str, px: f32) -> i32 {
        let mut w = 0.0_f32;
        for ch in text.chars() {
            let (m, _) = self.font.rasterize(ch, px);
            w += m.advance_width;
        }
        w.ceil() as i32
    }

    /// Composite `text` onto `canvas` at baseline (`x`, `y`). `color`
    /// is sRGB (R, G, B); `alpha` (0..=255) combines with fontdue's
    /// per-glyph alpha. Canvas is wl_shm Argb8888 (BGRA on LE).
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        text: &str,
        px: f32,
        x: i32,
        y: i32,
        color: (u8, u8, u8),
        alpha: u8,
        canvas: &mut [u8],
        canvas_w: u32,
        canvas_h: u32,
    ) {
        let stride = canvas_w as i32 * 4;
        let mut pen_x = x as f32;
        for ch in text.chars() {
            let (metrics, bitmap) = self.font.rasterize(ch, px);
            let glyph_left = pen_x as i32 + metrics.xmin;
            let glyph_top = y - metrics.ymin - metrics.height as i32;
            for gy in 0..metrics.height {
                let cy = glyph_top + gy as i32;
                if cy < 0 || cy as u32 >= canvas_h {
                    continue;
                }
                for gx in 0..metrics.width {
                    let cx = glyph_left + gx as i32;
                    if cx < 0 || cx as u32 >= canvas_w {
                        continue;
                    }
                    let glyph_alpha = bitmap[gy * metrics.width + gx];
                    if glyph_alpha == 0 {
                        continue;
                    }
                    let a = (glyph_alpha as u32 * alpha as u32 / 255) as u8;
                    let dst = (cy * stride + cx * 4) as usize;
                    if a == 255 {
                        canvas[dst] = color.2;
                        canvas[dst + 1] = color.1;
                        canvas[dst + 2] = color.0;
                        canvas[dst + 3] = 0xff;
                    } else {
                        let av = a as u32;
                        let inv = 255 - av;
                        canvas[dst] =
                            ((color.2 as u32 * av + canvas[dst] as u32 * inv) / 255) as u8;
                        canvas[dst + 1] =
                            ((color.1 as u32 * av + canvas[dst + 1] as u32 * inv) / 255) as u8;
                        canvas[dst + 2] =
                            ((color.0 as u32 * av + canvas[dst + 2] as u32 * inv) / 255) as u8;
                        canvas[dst + 3] = 0xff;
                    }
                }
            }
            pen_x += metrics.advance_width;
        }
    }
}
