//! Wallpaper loading + per-output Lanczos3 resize + BGRA cache.
//!
//! Lanczos3 resize is expensive (hundreds of ms for 4K → 1080p), so
//! the cache stores one pre-scaled BGRA payload per `(rect_w,
//! rect_h)`. Identically-sized outputs share an entry; distinct
//! sizes get distinct entries.
//!
//! Each output's wallpaper is scaled to that output's (w, h)
//! independently, preserving aspect ratio and cropping the long
//! axis. The wallpaper is not stretched across cage's spanned surface.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::imageops::FilterType;

use crate::OutputRect;

pub struct Wallpaper {
    /// Canonicalized target of the (symlinked) source at load time, or
    /// `None` if it couldn't be resolved. Lets `is_stale` detect a live
    /// wallpaper swap: the theme's `wallpaper-blurred.png` is a fixed
    /// symlink name, so only its target moves.
    resolved: Option<PathBuf>,
    decoded: image::DynamicImage,
    /// Pre-scaled BGRA payloads keyed on (rect_w, rect_h). Each entry
    /// is exactly `w * h * 4` bytes, ready to memcpy row-by-row.
    cache: HashMap<(u32, u32), Vec<u8>>,
}

impl Wallpaper {
    pub fn load(path: &Path) -> Result<Self> {
        log::info!("loading wallpaper from {}", path.display());
        // Decode by content, not by extension. A theme's
        // `wallpaper-blurred.png` is a symlink the reconciler points at
        // the chosen wallpaper's blurred variant, several of which ship
        // as `.jpg` — so the `.png` name lies. `image::open` trusts the
        // extension and would feed JPEG bytes to the PNG decoder
        // ("Invalid PNG signature"), bricking the greeter/lock screen;
        // sniff the magic bytes so any supported format loads.
        let decoded = image::ImageReader::open(path)
            .with_context(|| format!("opening wallpaper {}", path.display()))?
            .with_guessed_format()
            .with_context(|| format!("sniffing wallpaper format {}", path.display()))?
            .decode()
            .with_context(|| format!("decoding wallpaper {}", path.display()))?;
        log::info!(
            "wallpaper decoded: {}x{}",
            decoded.width(),
            decoded.height()
        );
        Ok(Self {
            resolved: std::fs::canonicalize(path).ok(),
            decoded,
            cache: HashMap::new(),
        })
    }

    /// True if `path` now resolves to a different file than the one
    /// loaded — i.e. the theme's stable `wallpaper-blurred.png` symlink
    /// was re-pointed by `shedman theme set wallpaper`. Comparing the
    /// path string is never enough: it is a fixed symlink name.
    pub fn is_stale(&self, path: &Path) -> bool {
        match (std::fs::canonicalize(path).ok(), &self.resolved) {
            (Some(now), Some(loaded)) => &now != loaded,
            // Either side unresolvable (broken/missing link) — reload
            // rather than risk keeping a stale image.
            _ => true,
        }
    }

    /// A solid-color fallback for when the themed wallpaper can't be
    /// decoded, so the greeter and lock screen still come up on a plain
    /// background instead of failing to build their cache. The 64x64
    /// fill scales to any output through the same blit path. `resolved`
    /// is None so the next theme change retries the real wallpaper.
    pub fn solid(color: u32) -> Self {
        let r = ((color >> 16) & 0xff) as u8;
        let g = ((color >> 8) & 0xff) as u8;
        let b = (color & 0xff) as u8;
        Self {
            resolved: None,
            decoded: image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                64,
                64,
                image::Rgb([r, g, b]),
            )),
            cache: HashMap::new(),
        }
    }

    /// Return true if the wallpaper's average luminance is below
    /// 128. Samples a 10x10 grid (100 pixels) using Rec. 601 luma:
    /// `Y = 0.299 R + 0.587 G + 0.114 B`. Used by the prompt-ui
    /// cache to pick the wordmark variant whose glyphs contrast
    /// with the background.
    pub fn is_average_dark(&self) -> bool {
        let rgba = self.decoded.to_rgba8();
        let w = rgba.width();
        let h = rgba.height();
        if w == 0 || h == 0 {
            return true;
        }
        const SAMPLES_PER_AXIS: u32 = 10;
        let mut total: u64 = 0;
        for sy in 0..SAMPLES_PER_AXIS {
            for sx in 0..SAMPLES_PER_AXIS {
                let x = (sx * w) / SAMPLES_PER_AXIS;
                let y = (sy * h) / SAMPLES_PER_AXIS;
                let px = rgba.get_pixel(x, y);
                let r = px[0] as u64;
                let g = px[1] as u64;
                let b = px[2] as u64;
                total += (299 * r + 587 * g + 114 * b) / 1000;
            }
        }
        let avg = total / (SAMPLES_PER_AXIS as u64 * SAMPLES_PER_AXIS as u64);
        avg < 128
    }

    /// Paint the wallpaper into `rect`'s region of `canvas`. First
    /// call at a given (rect.w, rect.h) builds the cache entry with
    /// Lanczos3; subsequent calls memcpy from cache.
    ///
    /// Off-canvas rows and columns are clipped per-row. Fully
    /// off-canvas rects are no-ops.
    pub fn blit_rect(
        &mut self,
        canvas: &mut [u8],
        canvas_w: u32,
        canvas_h: u32,
        rect: &OutputRect,
    ) {
        if canvas_w == 0 || canvas_h == 0 || rect.w <= 0 || rect.h <= 0 {
            return;
        }
        let rw = rect.w as u32;
        let rh = rect.h as u32;
        let need = (rw as usize) * (rh as usize) * 4;

        let bgra = self.cache.entry((rw, rh)).or_insert_with(|| {
            log::info!("rebuilding wallpaper cache for {}x{}", rw, rh);
            let scaled = self
                .decoded
                .resize_to_fill(rw, rh, FilterType::Lanczos3)
                .to_rgba8();
            let mut bgra = Vec::with_capacity(need);
            for px in scaled.pixels() {
                bgra.push(px[2]);
                bgra.push(px[1]);
                bgra.push(px[0]);
                bgra.push(0xff);
            }
            bgra
        });

        let canvas_pitch = (canvas_w as usize) * 4;
        let src_pitch = (rw as usize) * 4;
        for row in 0..rh as i32 {
            let dst_y = rect.y + row;
            if dst_y < 0 || (dst_y as u32) >= canvas_h {
                continue;
            }
            // Per-row x-clipping: clamp dst columns to the canvas,
            // then take the matching source-row slice.
            let dst_x_start = rect.x.max(0);
            let dst_x_end = (rect.x + rect.w).min(canvas_w as i32);
            if dst_x_end <= dst_x_start {
                continue;
            }
            let src_x_start = (dst_x_start - rect.x) as usize;
            let cols = (dst_x_end - dst_x_start) as usize;

            let src_off = (row as usize) * src_pitch + src_x_start * 4;
            let dst_off = (dst_y as usize) * canvas_pitch + (dst_x_start as usize) * 4;
            let len = cols * 4;

            // Defensive: skip if either side would index past the slice.
            if src_off + len > bgra.len() || dst_off + len > canvas.len() {
                continue;
            }
            canvas[dst_off..dst_off + len].copy_from_slice(&bgra[src_off..src_off + len]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, RgbImage};

    #[test]
    fn decodes_jpeg_bytes_under_a_png_name() {
        // A theme's `wallpaper-blurred.png` is a symlink that can point
        // at JPEG bytes (shipped `-blurred.jpg` variants), so the loader
        // must decode by content, not by the `.png` extension.
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 4, image::Rgb([10, 20, 30])));
        let path = std::env::temp_dir().join("shedos-wallpaper-jpeg-as.png");
        img.save_with_format(&path, ImageFormat::Jpeg)
            .expect("write JPEG fixture");
        let loaded = Wallpaper::load(&path);
        let _ = std::fs::remove_file(&path);
        let wp = loaded.expect("JPEG content must decode despite the .png name");
        assert_eq!((wp.decoded.width(), wp.decoded.height()), (4, 4));
    }

    #[test]
    fn is_stale_detects_a_symlink_retarget() {
        use std::os::unix::fs::symlink;
        let dir = std::env::temp_dir().join("shedos-wallpaper-stale-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let a = dir.join("a.png");
        let b = dir.join("b.png");
        DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 2, image::Rgb([1, 2, 3])))
            .save_with_format(&a, ImageFormat::Png)
            .expect("write a");
        DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 2, image::Rgb([4, 5, 6])))
            .save_with_format(&b, ImageFormat::Png)
            .expect("write b");
        let link = dir.join("current.png");
        symlink(&a, &link).expect("symlink -> a");

        let wp = Wallpaper::load(&link).expect("load via symlink");
        assert!(!wp.is_stale(&link), "unchanged target must not be stale");

        std::fs::remove_file(&link).expect("unlink");
        symlink(&b, &link).expect("symlink -> b");
        assert!(wp.is_stale(&link), "re-pointed symlink must be stale");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn solid_is_a_uniform_fallback() {
        let dark = Wallpaper::solid(0x1e1e2e);
        assert_eq!((dark.decoded.width(), dark.decoded.height()), (64, 64));
        assert!(dark.is_average_dark(), "a dark base reads as dark");
        assert!(
            !Wallpaper::solid(0xffffff).is_average_dark(),
            "white reads as light"
        );
    }
}
