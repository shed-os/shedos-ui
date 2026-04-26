//! Helpers for rendering a Logo onto a Frame as the target state.

use shedos_screensaver_core::{Cell, Color, Frame, Logo};

/// Paint `logo` onto `frame` (centered horizontally + vertically),
/// using `fg` as the foreground for every lit cell. Non-lit cells
/// are left at their current state — typically the frame is cleared
/// to `Cell::default()` first by the caller.
///
/// Used by the engine to build the "target Frame" each effect tries
/// to land on. Pure, deterministic, no allocation past the writes.
pub fn render_logo_centered(frame: &mut Frame, logo: &Logo, fg: Color) {
    if logo.rows == 0 || logo.cols == 0 {
        return;
    }
    let row_offset = (frame.rows().saturating_sub(logo.rows) / 2) as i32;
    let col_offset = (frame.cols().saturating_sub(logo.cols) / 2) as i32;
    for lr in 0..logo.rows as i32 {
        for lc in 0..logo.cols as i32 {
            if !logo.lit(lr as usize, lc as usize) {
                continue;
            }
            let fr = row_offset + lr;
            let fc = col_offset + lc;
            if fr < 0 || fc < 0 || fr >= frame.rows() as i32 || fc >= frame.cols() as i32 {
                continue;
            }
            frame.set(fr as u16, fc as u16, Cell {
                ch: logo.glyph_at(lr as usize, lc as usize),
                fg,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }
    }
}

/// Build a fresh target Frame at the given size with the logo
/// centered. Convenience wrapper for the engine.
pub fn build_target(rows: u16, cols: u16, logo: &Logo, fg: Color) -> Frame {
    let mut f = Frame::new(rows, cols);
    render_logo_centered(&mut f, logo, fg);
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
    fn render_centered_lands_logo_in_middle_of_canvas() {
        let logo = shedos_logo();
        let mut f = Frame::new(20, 60);
        render_logo_centered(&mut f, &logo, Color::WHITE);
        // Top of logo should land near (rows - logo.rows) / 2 = 7
        let top_row = (20 - logo.rows) / 2;
        // Some lit cell should exist on that row.
        let any_lit = (0..f.cols())
            .any(|c| f.get(top_row, c).map(|cell| cell.ch != ' ').unwrap_or(false));
        assert!(any_lit, "expected lit cells on row {}", top_row);
    }

    #[test]
    fn build_target_returns_logo_only_frame() {
        let logo = shedos_logo();
        let f = build_target(10, 30, &logo, Color::WHITE);
        // Lit cell count in target should equal the logo's lit count
        // (assuming the logo fits — these dims do).
        let target_lit: usize = f
            .cells()
            .filter(|(_, _, cell)| cell.ch != ' ')
            .count();
        assert_eq!(target_lit, logo.lit_count());
    }

    #[test]
    fn render_oversized_logo_clips_to_frame() {
        let logo = shedos_logo(); // 5x23
        let mut f = Frame::new(2, 5); // way smaller
        render_logo_centered(&mut f, &logo, Color::WHITE);
        // Should not panic; out-of-bounds writes are dropped.
    }
}
