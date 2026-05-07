//! prism-split — each lit cell starts as three RGB ghost copies
//! at horizontal offsets (red shifted left, blue shifted right,
//! center at canonical position in target color). The offsets
//! shrink to zero, the chromatic aberration focuses to a clean
//! image at progress=1.0.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 4_000;
/// Maximum ghost offset at progress=0 (cells).
const MAX_OFFSET: f32 = 4.0;
/// Catppuccin red and blue used as the R/B ghost tints.
const RED_TINT: Color = Color::rgb(0xf3, 0x8b, 0xa8);
const BLUE_TINT: Color = Color::rgb(0x89, 0xb4, 0xfa);

#[derive(Clone, Copy)]
struct PrismCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}

pub struct PrismSplit {
    cells: Vec<PrismCell>,
    canvas_cols: u16,
    elapsed: Duration,
}

impl PrismSplit {
    pub fn new() -> Self {
        Self { cells: Vec::new(), canvas_cols: 0, elapsed: Duration::ZERO }
    }
}

impl Default for PrismSplit {
    fn default() -> Self { Self::new() }
}

impl Effect for PrismSplit {
    fn name(&self) -> &'static str { "prism-split" }
    fn title(&self) -> &'static str { "Prism Split" }
    fn description(&self) -> &'static str {
        "Each lit cell starts as three RGB ghost copies at horizontal offsets — red left, blue right, center at canonical; offsets shrink to zero as the chromatic aberration focuses."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_cols = target.cols();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.cells.push(PrismCell {
                row: r,
                col: c,
                ch: cell.ch,
                color: cell.fg,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        // Offset shrinks linearly from MAX_OFFSET at t=0 to 0 at t=1.
        // round() gives integer pixel offsets — once it hits 0, R/B
        // ghosts are at the same position as center and get skipped.
        let offset = ((1.0 - progress) * MAX_OFFSET).round() as i32;

        // Pass 1: R/B ghosts at horizontal offsets. Drawn first so
        // pass 2's center ghosts override any overlap at canonical.
        if offset > 0 {
            for cell in &self.cells {
                let col_l = cell.col as i32 - offset;
                if (0..self.canvas_cols as i32).contains(&col_l) {
                    frame.set(cell.row, col_l as u16, Cell {
                        ch: cell.ch,
                        fg: RED_TINT,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
                let col_r = cell.col as i32 + offset;
                if (0..self.canvas_cols as i32).contains(&col_r) {
                    frame.set(cell.row, col_r as u16, Cell {
                        ch: cell.ch,
                        fg: BLUE_TINT,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
            }
        }

        // Pass 2: center ghosts at canonical positions, target color.
        for cell in &self.cells {
            frame.set(cell.row, cell.col, Cell {
                ch: cell.ch,
                fg: cell.color,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
    }
}
