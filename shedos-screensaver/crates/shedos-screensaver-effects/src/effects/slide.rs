//! slide — cells slide horizontally in from the edges. Even rows
//! enter from the left, odd rows from the right; converging into the
//! target. Looks like rolling shutters.

use crate::easing;
use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 3_500;

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    end_col: u16,
    ch: char,
    color: Color,
    /// Starting column (off-screen left or right depending on row parity).
    start_col: f32,
}

pub struct Slide {
    cells: Vec<TargetCell>,
    elapsed: Duration,
    cols: u16,
}

impl Slide {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO, cols: 0 }
    }
}

impl Default for Slide {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Slide {
    fn name(&self) -> &'static str { "slide" }
    fn title(&self) -> &'static str { "Slide" }
    fn description(&self) -> &'static str {
        "Cells slide in from the edges; even rows from the left, odd from the right."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.cols = target.cols();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            let start = if r % 2 == 0 {
                -(self.cols as f32) // off-screen left
            } else {
                self.cols as f32 * 2.0 // off-screen right
            };
            self.cells.push(TargetCell {
                row: r,
                end_col: c,
                ch: cell.ch,
                color: cell.fg,
                start_col: start,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let eased = easing::ease_out_quart(progress);

        frame.clear();
        for c in &self.cells {
            let cur = easing::lerp(c.start_col, c.end_col as f32, eased);
            let col = cur.round() as i32;
            if col < 0 || col >= self.cols as i32 {
                continue;
            }
            frame.set(c.row, col as u16, Cell {
                ch: c.ch,
                fg: c.color,
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
