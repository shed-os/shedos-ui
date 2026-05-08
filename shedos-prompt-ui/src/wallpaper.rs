//! Wallpaper loading + Lanczos3 resize + per-surface BGRA cache.
//!
//! Decoding and resizing a 4K PNG is expensive (hundreds of ms);
//! the cache makes per-keystroke redraws a pure memcpy. The cached
//! payload is keyed on `(canvas_w, canvas_h)` so a configure event
//! that changes the surface dimensions invalidates the cache once
//! and re-decodes.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::imageops::FilterType;

/// Loaded source image, kept in memory so repeated resizes don't
/// re-decode the file from disk.
pub struct Wallpaper {
    source_path: PathBuf,
    decoded: image::DynamicImage,
    cache: Option<(u32, u32, Vec<u8>)>,
}

impl Wallpaper {
    pub fn load(path: &Path) -> Result<Self> {
        log::info!("loading wallpaper from {}", path.display());
        let decoded = image::open(path)
            .with_context(|| format!("opening wallpaper {}", path.display()))?;
        log::info!(
            "wallpaper decoded: {}x{}",
            decoded.width(),
            decoded.height()
        );
        Ok(Self {
            source_path: path.to_path_buf(),
            decoded,
            cache: None,
        })
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    /// Blit the wallpaper across the entire canvas, scaled with
    /// Lanczos3 to fill `(w, h)` while preserving aspect ratio
    /// (cropping if necessary). The first call at a given
    /// `(w, h)` rebuilds the BGRA cache; subsequent calls memcpy.
    pub fn blit(&mut self, canvas: &mut [u8], w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let need = (w as usize) * (h as usize) * 4;
        if canvas.len() < need {
            return;
        }
        let cache_hit = self
            .cache
            .as_ref()
            .is_some_and(|(cw, ch, _)| *cw == w && *ch == h);
        if !cache_hit {
            log::info!("rebuilding wallpaper cache for {}x{}", w, h);
            let scaled = self
                .decoded
                .resize_to_fill(w, h, FilterType::Lanczos3)
                .to_rgba8();
            let mut bgra = Vec::with_capacity(need);
            for px in scaled.pixels() {
                bgra.push(px[2]);
                bgra.push(px[1]);
                bgra.push(px[0]);
                bgra.push(0xff);
            }
            self.cache = Some((w, h, bgra));
        }
        let bgra = &self.cache.as_ref().expect("just populated").2;
        canvas[..bgra.len()].copy_from_slice(bgra);
    }
}
