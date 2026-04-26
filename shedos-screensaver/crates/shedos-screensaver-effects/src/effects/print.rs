//! print — typewriter reveal. Cells appear in row-major order at a
//! steady rate, with a blinking cursor at the next-to-be-printed
//! cell. Classic terminal feel.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 5_000;

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}

pub struct Print {
    cells: Vec<TargetCell>,
    elapsed: Duration,
}

impl Print {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO }
    }
}

impl Default for Print {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Print {
    fn name(&self) -> &'static str { "print" }
    fn title(&self) -> &'static str { "Print" }
    fn description(&self) -> &'static str {
        "Typewriter reveal: cells print left-to-right, top-to-bottom with a blinking cursor."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        // Capture lit cells in row-major order (the natural typing order).
        for (r, c, cell) in target.cells() {
            if cell.ch != ' ' {
                self.cells.push(TargetCell { row: r, col: c, ch: cell.ch, color: cell.fg });
            }
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let idx_revealed = (progress * self.cells.len() as f32) as usize;

        frame.clear();
        for (i, c) in self.cells.iter().enumerate() {
            if i < idx_revealed {
                frame.set(c.row, c.col, Cell {
                    ch: c.ch,
                    fg: c.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        // Blinking cursor at the next cell to be printed.
        if idx_revealed < self.cells.len() {
            let blink = (self.elapsed.as_millis() / 250) % 2 == 0;
            if blink {
                let next = self.cells[idx_revealed];
                frame.set(next.row, next.col, Cell {
                    ch: '█',
                    fg: Color::rgb(0xcd, 0xd6, 0xf4),
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
    }
}
