//! Wallpaper loading + per-output Lanczos3 resize + BGRA cache.
//!
//! Decoding a wallpaper PNG is cheap; resizing it with Lanczos3 to a
//! given target rect is expensive (hundreds of ms for 4K → 1080p).
//! The cache stores one pre-scaled BGRA payload per distinct
//! `(rect_w, rect_h)` so:
//!
//! * the same monitor's per-keystroke redraws are pure memcpy,
//! * a multi-monitor mirror across N identically-sized outputs hits
//!   one cache entry,
//! * a multi-monitor setup with distinct sizes (e.g. 1080p + 1440p)
//!   gets one cache entry per distinct size.
//!
//! Each output is treated independently: the source image is scaled
//! to *that output's* (w, h), preserving aspect ratio and cropping
//! the long axis to fill. Critically, this means the wallpaper is
//! NOT stretched across cage's spanned surface — every monitor gets
//! a self-contained, correctly-aspected wallpaper.
//!
//! NOTE on `OutputRect.{x,y}` interpretation: these are canvas-local
//! pixel coordinates. cage places its spanned-surface origin at the
//! topleft-most output, so for a side-by-side dual-monitor setup the
//! left output is at x=0 and the right at x=1920 (etc.). We blit the
//! pre-scaled BGRA into the rect's region by stamping rect.h rows
//! with row-pitch = rect_w * 4 into canvas at offset (rect.x, rect.y).
//! Rects that clip the canvas (off-screen, negative coords) are
//! handled per-row.
//!
//! This module is intentionally `OutputRect`-aware so the renderer
//! call site stays a one-liner per rect.
//!
//! See lib.rs::render for the call ordering: every rect's wallpaper
//! is painted before any widgets are composed on top.
//!
//! # Test note
//! The cache is keyed on dimensions only; the source path is checked
//! by `WidgetCache::refresh_wallpaper` which rebuilds the entire
//! `Wallpaper` (and thus drops the cache) on theme change. So a
//! wallpaper swap correctly invalidates per-output entries too.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::imageops::FilterType;

use crate::OutputRect;

pub struct Wallpaper {
    source_path: PathBuf,
    decoded: image::DynamicImage,
    /// Pre-scaled BGRA payloads keyed on (rect_w, rect_h). Each entry
    /// is exactly `w * h * 4` bytes, ready to memcpy row-by-row.
    cache: HashMap<(u32, u32), Vec<u8>>,
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
            cache: HashMap::new(),
        })
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    /// Paint the wallpaper into `rect`'s region of `canvas`, scaled
    /// with Lanczos3 to fill `(rect.w, rect.h)` while preserving
    /// aspect ratio (cropping the long axis). First call at a given
    /// rect size rebuilds the cache entry; subsequent calls memcpy
    /// row-by-row into the right canvas offset.
    ///
    /// Off-canvas rect rows (`rect.y < 0`, `rect.y + rect.h > canvas_h`)
    /// and off-canvas columns are clipped. A rect entirely outside
    /// the canvas is a no-op.
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
            // Per-row x-clipping: clamp the start and end columns to
            // the canvas, then compute the matching slice of the
            // source row to copy.
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
