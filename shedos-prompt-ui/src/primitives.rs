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

/// Anti-aliased arc segment. Stroke is centered on `radius`; pixels
/// outside `[theta_start, theta_end]` (degrees, 0° = top / 12 o'clock,
/// going clockwise) are skipped. `theta_start > theta_end` wraps
/// through 0°/360° so a 270° arc opening at the bottom can be
/// expressed as `(225, 135)`.
#[allow(clippy::too_many_arguments)]
pub fn draw_arc(
    canvas: &mut [u8],
    cw: u32,
    cx: f32,
    cy: f32,
    radius: f32,
    stroke: f32,
    theta_start_deg: f32,
    theta_end_deg: f32,
    color: (u8, u8, u8),
    alpha: u8,
) {
    let r_outer = radius + stroke / 2.0 + 1.0;
    let xmin = (cx - r_outer).floor() as i32;
    let xmax = (cx + r_outer).ceil() as i32;
    let ymin = (cy - r_outer).floor() as i32;
    let ymax = (cy + r_outer).ceil() as i32;
    let edge = stroke / 2.0;
    for y in ymin..=ymax {
        for x in xmin..=xmax {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let off = (dist - radius).abs();
            if off >= edge + 0.5 {
                continue;
            }
            // Angle in "0° = top, clockwise" with canvas-y pointing
            // down: north = (0,-1) → atan2(-1, 0) = -π/2 → -90°;
            // adding 90° + wrap puts north at 0° and east at 90°,
            // matching the convention above.
            let angle = (dy.atan2(dx).to_degrees() + 90.0 + 360.0) % 360.0;
            let in_range = if theta_start_deg <= theta_end_deg {
                angle >= theta_start_deg && angle <= theta_end_deg
            } else {
                angle >= theta_start_deg || angle <= theta_end_deg
            };
            if !in_range {
                continue;
            }
            let coverage = if off <= edge - 0.5 {
                1.0
            } else {
                (edge + 0.5 - off).clamp(0.0, 1.0)
            };
            let a = (alpha as f32 * coverage) as u8;
            if a > 0 {
                blend_pixel(canvas, cw, x, y, color, a);
            }
        }
    }
}

/// Concentric-arc fingerprint icon centered at (`cx`, `cy`). `size`
/// is the diameter in pixels. Four arcs from outer to inner with a
/// gap at the bottom — recognizable as a fingerprint at sizes ≥ 24px.
pub fn draw_fingerprint_icon(
    canvas: &mut [u8],
    cw: u32,
    cx: f32,
    cy: f32,
    size: f32,
    color: (u8, u8, u8),
    alpha: u8,
) {
    let max_r = size / 2.0;
    let stroke = (size * 0.08).max(1.5);
    let arcs = [
        (max_r * 0.95, 215.0_f32, 145.0_f32),
        (max_r * 0.72, 225.0, 135.0),
        (max_r * 0.50, 210.0, 150.0),
        (max_r * 0.25, 0.0, 360.0),
    ];
    for (r, start, end) in arcs {
        if r <= stroke / 2.0 {
            continue;
        }
        draw_arc(canvas, cw, cx, cy, r, stroke, start, end, color, alpha);
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
