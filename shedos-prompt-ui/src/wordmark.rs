//! Brand wordmark loading + Lanczos3 resize cache + alpha-aware
//! blit. Mirrors `Wallpaper` in shape but blends rather than memcpy
//! because the wordmark PNG carries an alpha channel.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::imageops::FilterType;

pub struct Wordmark {
    source_path: PathBuf,
    decoded: image::DynamicImage,
    /// Per-target-width BGRA cache. Key is the requested width;
    /// height is derived from the source aspect ratio.
    cache: HashMap<u32, Scaled>,
}

struct Scaled {
    pixels: Vec<u8>,
    w: u32,
    h: u32,
}

impl Wordmark {
    pub fn load(path: &Path) -> Result<Self> {
        log::info!("loading wordmark from {}", path.display());
        let decoded = image::open(path)
            .with_context(|| format!("opening wordmark {}", path.display()))?;
        log::info!(
            "wordmark decoded: {}x{}",
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

    fn scaled_for(&mut self, target_w: u32) -> &Scaled {
        if !self.cache.contains_key(&target_w) {
            let src_w = self.decoded.width().max(1);
            let src_h = self.decoded.height().max(1);
            let target_h = ((target_w as u64 * src_h as u64) / src_w as u64) as u32;
            log::info!("rebuilding wordmark cache at {}x{}", target_w, target_h);
            let rgba = self
                .decoded
                .resize(target_w, target_h, FilterType::Lanczos3)
                .to_rgba8();
            let w = rgba.width();
            let h = rgba.height();
            let mut pixels = Vec::with_capacity((w * h * 4) as usize);
            for px in rgba.pixels() {
                pixels.push(px[2]); // B
                pixels.push(px[1]); // G
                pixels.push(px[0]); // R
                pixels.push(px[3]); // A
            }
            self.cache.insert(target_w, Scaled { pixels, w, h });
        }
        self.cache.get(&target_w).expect("just inserted")
    }

    /// Blit the wordmark centered at `(cx, cy)` with target rendered
    /// width `target_w` (height computed from aspect ratio). Alpha-
    /// blends against the existing canvas pixels so transparent regions
    /// of the PNG keep the wallpaper underneath.
    pub fn blit_centered(
        &mut self,
        canvas: &mut [u8],
        canvas_w: u32,
        canvas_h: u32,
        cx: i32,
        cy: i32,
        target_w: u32,
    ) {
        if canvas_w == 0 || canvas_h == 0 || target_w == 0 {
            return;
        }
        let s = self.scaled_for(target_w);
        let w = s.w as i32;
        let h = s.h as i32;
        let src_pitch = (s.w as usize) * 4;
        let dst_pitch = (canvas_w as usize) * 4;
        let x0 = cx - w / 2;
        let y0 = cy - h / 2;
        for row in 0..h {
            let dst_y = y0 + row;
            if dst_y < 0 || (dst_y as u32) >= canvas_h {
                continue;
            }
            let src_row_off = (row as usize) * src_pitch;
            for col in 0..w {
                let dst_x = x0 + col;
                if dst_x < 0 || (dst_x as u32) >= canvas_w {
                    continue;
                }
                let src_off = src_row_off + (col as usize) * 4;
                let dst_off = (dst_y as usize) * dst_pitch + (dst_x as usize) * 4;
                let sa = s.pixels[src_off + 3];
                if sa == 0 {
                    continue;
                }
                let sb = s.pixels[src_off] as u32;
                let sg = s.pixels[src_off + 1] as u32;
                let sr = s.pixels[src_off + 2] as u32;
                if sa == 0xff {
                    canvas[dst_off] = sb as u8;
                    canvas[dst_off + 1] = sg as u8;
                    canvas[dst_off + 2] = sr as u8;
                    canvas[dst_off + 3] = 0xff;
                } else {
                    let a = sa as u32;
                    let inv = 255 - a;
                    let db = canvas[dst_off] as u32;
                    let dg = canvas[dst_off + 1] as u32;
                    let dr = canvas[dst_off + 2] as u32;
                    canvas[dst_off] = ((sb * a + db * inv + 127) / 255) as u8;
                    canvas[dst_off + 1] = ((sg * a + dg * inv + 127) / 255) as u8;
                    canvas[dst_off + 2] = ((sr * a + dr * inv + 127) / 255) as u8;
                    canvas[dst_off + 3] = 0xff;
                }
            }
        }
    }
}
