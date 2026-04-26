//! Helpers for rendering a Logo onto a Frame as the target state.
//!
//! The art is integer-scaled (cell replication) so SHEDOS fills the
//! canvas like Omarchy's screensaver does — each lit cell of the
//! logo becomes an N×N block of canvas cells, where N is the largest
//! integer that keeps the rendered art within ~75% of the canvas.
//! Solid-block variants (`block`, `mini`) scale crisply; line-drawing
//! variants tile their glyphs (which still reads as a stylized "big"
//! version of the letter, similar to a half-tone print).

use shedos_screensaver_core::{Cell, Color, Frame, Logo};

/// Target fraction of the canvas the rendered art should occupy.
const TARGET_FILL_RATIO: f32 = 0.75;

/// Compute the largest integer cell-replication factor that keeps the
/// scaled logo within `TARGET_FILL_RATIO` of the canvas. Always ≥ 1.
pub fn auto_scale(canvas_rows: u16, canvas_cols: u16, logo: &Logo) -> u16 {
    if logo.rows == 0 || logo.cols == 0 {
        return 1;
    }
    let max_h = (canvas_rows as f32 * TARGET_FILL_RATIO / logo.rows as f32).floor() as u16;
    let max_w = (canvas_cols as f32 * TARGET_FILL_RATIO / logo.cols as f32).floor() as u16;
    max_h.min(max_w).max(1)
}

/// Paint `logo` onto `frame` (centered horizontally + vertically) at
/// the given integer `scale`. `scale=1` is the original cell-by-cell
/// rendering; `scale>=2` replicates each lit cell into a `scale × scale`
/// block.
pub fn render_logo_scaled_centered(frame: &mut Frame, logo: &Logo, fg: Color, scale: u16) {
    let scale = scale.max(1);
    if logo.rows == 0 || logo.cols == 0 {
        return;
    }
    let scaled_rows = logo.rows.saturating_mul(scale);
    let scaled_cols = logo.cols.saturating_mul(scale);
    let row_offset = (frame.rows().saturating_sub(scaled_rows) / 2) as i32;
    let col_offset = (frame.cols().saturating_sub(scaled_cols) / 2) as i32;
    for lr in 0..logo.rows as i32 {
        for lc in 0..logo.cols as i32 {
            if !logo.lit(lr as usize, lc as usize) {
                continue;
            }
            let glyph = logo.glyph_at(lr as usize, lc as usize);
            for sy in 0..scale as i32 {
                for sx in 0..scale as i32 {
                    let fr = row_offset + lr * scale as i32 + sy;
                    let fc = col_offset + lc * scale as i32 + sx;
                    if fr < 0
                        || fc < 0
                        || fr >= frame.rows() as i32
                        || fc >= frame.cols() as i32
                    {
                        continue;
                    }
                    frame.set(fr as u16, fc as u16, Cell {
                        ch: glyph,
                        fg,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
            }
        }
    }
}

/// Paint `logo` onto `frame` centered, at scale=1 (no scaling).
/// Kept as a thin alias for callers that want raw 1:1 rendering.
pub fn render_logo_centered(frame: &mut Frame, logo: &Logo, fg: Color) {
    render_logo_scaled_centered(frame, logo, fg, 1);
}

/// Build a fresh target Frame at the given size with the logo
/// auto-scaled to fill ~75% of the canvas.
pub fn build_target(rows: u16, cols: u16, logo: &Logo, fg: Color) -> Frame {
    let scale = auto_scale(rows, cols, logo);
    build_target_with_scale(rows, cols, logo, fg, scale)
}

/// Build a fresh target Frame with an explicit scale factor (1 = no
/// scaling, like the prior behavior). Useful for tests and for users
/// who pass `--scale=N` to override the auto-fill heuristic.
pub fn build_target_with_scale(rows: u16, cols: u16, logo: &Logo, fg: Color, scale: u16) -> Frame {
    let mut f = Frame::new(rows, cols);
    render_logo_scaled_centered(&mut f, logo, fg, scale);
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn shedos_logo() -> Logo {
        Logo::parse(
            "███████ ██   ██ ███████\n\
             ██      ██   ██ ██\n\
             ███████ ███████ █████\n\
                  ██ ██   ██ ██\n\
             ███████ ██   ██ ███████\n",
            PathBuf::from("test"),
        )
    }

    #[test]
    fn render_centered_unscaled_lands_logo_in_middle_of_canvas() {
        let logo = shedos_logo();
        let mut f = Frame::new(20, 60);
        render_logo_scaled_centered(&mut f, &logo, Color::WHITE, 1);
        let top_row = (20 - logo.rows) / 2;
        let any_lit = (0..f.cols())
            .any(|c| f.get(top_row, c).map(|cell| cell.ch != ' ').unwrap_or(false));
        assert!(any_lit, "expected lit cells on row {}", top_row);
    }

    #[test]
    fn build_target_at_scale_1_matches_original_lit_count() {
        let logo = shedos_logo();
        let f = build_target_with_scale(10, 30, &logo, Color::WHITE, 1);
        let target_lit: usize = f
            .cells()
            .filter(|(_, _, cell)| cell.ch != ' ')
            .count();
        assert_eq!(target_lit, logo.lit_count());
    }

    #[test]
    fn build_target_at_scale_2_quadruples_lit_count() {
        let logo = shedos_logo();
        let f = build_target_with_scale(20, 60, &logo, Color::WHITE, 2);
        let target_lit: usize = f
            .cells()
            .filter(|(_, _, cell)| cell.ch != ' ')
            .count();
        // 2x scaling = each lit cell becomes a 2x2 block.
        assert_eq!(target_lit, logo.lit_count() * 4);
    }

    #[test]
    fn auto_scale_picks_at_least_one() {
        let logo = shedos_logo();
        // Canvas barely larger than logo — should still pick scale 1.
        assert_eq!(auto_scale(6, 24, &logo), 1);
    }

    #[test]
    fn auto_scale_grows_for_larger_canvas() {
        let logo = shedos_logo(); // 5 rows × ~23 cols
        // 60 rows × 200 cols at 75% fill:
        // max_h = 60 * 0.75 / 5 = 9, max_w = 200 * 0.75 / 23 = 6
        // → scale = 6
        assert_eq!(auto_scale(60, 200, &logo), 6);
    }

    #[test]
    fn auto_scale_handles_empty_logo() {
        let logo = Logo::parse("", PathBuf::from("test"));
        assert_eq!(auto_scale(60, 200, &logo), 1);
    }

    #[test]
    fn render_oversized_logo_clips_to_frame() {
        let logo = shedos_logo();
        let mut f = Frame::new(2, 5);
        render_logo_scaled_centered(&mut f, &logo, Color::WHITE, 1);
        // Should not panic; out-of-bounds writes are dropped.
    }

    #[test]
    fn scaled_logo_stays_within_canvas() {
        let logo = shedos_logo(); // 5 × 23
        // Canvas big enough to comfortably fit scale=4 (= 20 rows × 92 cols).
        let mut f = Frame::new(40, 120);
        render_logo_scaled_centered(&mut f, &logo, Color::WHITE, 4);
        // Verify nothing rendered outside (0..40, 0..120) — Frame::set
        // already drops OOB writes, but check no panic + non-empty render.
        let lit: usize = f.cells().filter(|(_, _, c)| c.ch != ' ').count();
        assert_eq!(lit, logo.lit_count() * 16); // 4*4 = 16
    }
}
