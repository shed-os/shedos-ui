//! Pixel-level drawing primitives shared between widgets:
//! anti-aliased rounded rectangle and an alpha-blend pixel writer.

/// Alpha-blend a single (R, G, B) pixel into a wl_shm Argb8888
/// canvas (BGRA byte order on little-endian) at logical (x, y).
#[inline]
pub fn blend_pixel(
    canvas: &mut [u8],
    cw: u32,
    x: i32,
    y: i32,
    color: (u8, u8, u8),
    alpha: u8,
) {
    if x < 0 || y < 0 || alpha == 0 {
        return;
    }
    if (x as u32) >= cw {
        return;
    }
    let dst = ((y as u32) * cw * 4 + (x as u32) * 4) as usize;
    if dst + 3 >= canvas.len() {
        return;
    }
    if alpha == 255 {
        canvas[dst] = color.2;
        canvas[dst + 1] = color.1;
        canvas[dst + 2] = color.0;
        canvas[dst + 3] = 0xff;
        return;
    }
    let av = alpha as u32;
    let inv = 255 - av;
    canvas[dst] = ((color.2 as u32 * av + canvas[dst] as u32 * inv) / 255) as u8;
    canvas[dst + 1] = ((color.1 as u32 * av + canvas[dst + 1] as u32 * inv) / 255) as u8;
    canvas[dst + 2] = ((color.0 as u32 * av + canvas[dst + 2] as u32 * inv) / 255) as u8;
    canvas[dst + 3] = 0xff;
}

/// Coverage of a pixel at (`px`, `py`) by a rounded-rect of given
/// geometry, returning 0.0..=1.0. Corners are quarter-circles of
/// radius `r`, anti-aliased with a 1-pixel band around `r`.
pub fn rounded_rect_coverage(
    px: f32,
    py: f32,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
    r: f32,
) -> f32 {
    if px < bx || py < by || px > bx + bw || py > by + bh {
        return 0.0;
    }
    let cx = if px < bx + r {
        bx + r
    } else if px > bx + bw - r {
        bx + bw - r
    } else {
        return 1.0;
    };
    let cy = if py < by + r {
        by + r
    } else if py > by + bh - r {
        by + bh - r
    } else {
        return 1.0;
    };
    let dx = px - cx;
    let dy = py - cy;
    let d = (dx * dx + dy * dy).sqrt();
    if d <= r - 0.5 {
        1.0
    } else if d >= r + 0.5 {
        0.0
    } else {
        (r + 0.5 - d).clamp(0.0, 1.0)
    }
}

/// Fill + stroke a rounded rectangle. Fill blends at `fill_alpha`;
/// border blends at `border_alpha`. Both are anti-aliased on the
/// curved edges.
#[allow(clippy::too_many_arguments)]
pub fn draw_rounded_box(
    canvas: &mut [u8],
    cw: u32,
    ch: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    radius: u32,
    thick: u32,
    fill: (u8, u8, u8),
    fill_alpha: u8,
    border: (u8, u8, u8),
    border_alpha: u8,
) {
    let _ = ch; // bounds enforced by blend_pixel via canvas.len()
    let bx = x as f32;
    let by = y as f32;
    let bw = w as f32;
    let bh = h as f32;
    let r = radius as f32;
    let t = thick as f32;
    for dy in 0..h as i32 {
        for dx in 0..w as i32 {
            let px = (x + dx) as f32 + 0.5;
            let py = (y + dy) as f32 + 0.5;
            let outer = rounded_rect_coverage(px, py, bx, by, bw, bh, r);
            if outer <= 0.0 {
                continue;
            }
            let inner = rounded_rect_coverage(
                px,
                py,
                bx + t,
                by + t,
                bw - 2.0 * t,
                bh - 2.0 * t,
                (r - t).max(0.0),
            );
            let cx = x + dx;
            let cy = y + dy;
            let fa = (fill_alpha as f32 * inner) as u8;
            if fa > 0 {
                blend_pixel(canvas, cw, cx, cy, fill, fa);
            }
            let stroke = (outer - inner).max(0.0);
            let ba = (border_alpha as f32 * stroke) as u8;
            if ba > 0 {
                blend_pixel(canvas, cw, cx, cy, border, ba);
            }
        }
    }
}
