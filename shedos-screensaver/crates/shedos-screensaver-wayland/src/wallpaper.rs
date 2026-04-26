//! Wallpaper backdrop loader + dim/scale helper.
//!
//! Loaded once at renderer construction; on each frame the prepared
//! framebuffer is memcpy'd as the starting layer before cells are
//! composited on top.

use crate::pack_argb;
use image::{imageops::FilterType, GenericImageView};
use shedos_screensaver_core::Color;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Wallpaper {
    pub source_path: PathBuf,
    /// Pre-baked ARGB pixels, sized to the surface; ready to memcpy.
    pub pixels: Vec<u32>,
    pub width: u32,
    pub height: u32,
}

impl Wallpaper {
    /// Load + scale + dim. `dim` is the brightness multiplier (0..=1).
    pub fn prepare(
        path: &Path,
        target_width: u32,
        target_height: u32,
        dim: f32,
    ) -> Result<Self, WallpaperError> {
        let img = image::open(path).map_err(|e| WallpaperError::Decode {
            path: path.to_path_buf(),
            source: e,
        })?;
        let scaled = img.resize_to_fill(target_width, target_height, FilterType::Lanczos3);
        let mut pixels = Vec::with_capacity((target_width * target_height) as usize);
        for (_, _, px) in scaled.pixels() {
            let r = (px[0] as f32 * dim).clamp(0.0, 255.0) as u8;
            let g = (px[1] as f32 * dim).clamp(0.0, 255.0) as u8;
            let b = (px[2] as f32 * dim).clamp(0.0, 255.0) as u8;
            pixels.push(pack_argb(Color::rgb(r, g, b)));
        }
        Ok(Self {
            source_path: path.to_path_buf(),
            pixels,
            width: target_width,
            height: target_height,
        })
    }
}

#[derive(Debug)]
pub enum WallpaperError {
    Decode { path: PathBuf, source: image::ImageError },
}

impl std::fmt::Display for WallpaperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode { path, source } => {
                write!(f, "failed to decode wallpaper {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for WallpaperError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_missing_file_errors() {
        let err = Wallpaper::prepare(
            Path::new("/no/such/wallpaper.png"),
            100,
            100,
            0.5,
        )
        .unwrap_err();
        let WallpaperError::Decode { .. } = err;
    }
}
