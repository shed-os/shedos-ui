//! brick-build — cells slide in from the nearest edge like bricks
//! stacking up. Each row queues at its source edge and slides
//! toward target columns; bottom rows arrive first so the wall
//! "builds upward".

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 4_500;
/// Each brick's slide window, normalized to total duration.
const SLIDE_NORM: f32 = 0.30;

#[derive(Clone, Copy)]
struct Brick {
    target_row: u16,
    target_col: u16,
    target_ch: char,
    target_color: Color,
    /// X position the brick slides from (off-canvas left or right).
    start_x: f32,
    start_t: f32,
    end_t: f32,
}

pub struct BrickBuild {
    bricks: Vec<Brick>,
    canvas_rows: u16,
    canvas_cols: u16,
    elapsed: Duration,
}

impl BrickBuild {
    pub fn new() -> Self {
        Self { bricks: Vec::new(), canvas_rows: 0, canvas_cols: 0, elapsed: Duration::ZERO }
    }
}

impl Default for BrickBuild {
    fn default() -> Self { Self::new() }
}

impl Effect for BrickBuild {
    fn name(&self) -> &'static str { "brick-build" }
    fn title(&self) -> &'static str { "Brick Build" }
    fn description(&self) -> &'static str {
        "Cells slide in from the nearest edge like bricks stacking up; bottom rows arrive first so the wall builds upward."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.bricks.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();

        let rows = target.rows() as f32;
        let cols = target.cols() as f32;
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            // Pick the nearest edge to slide from.
            let from_left = (c as f32) < cols * 0.5;
            let start_x = if from_left { -2.0 } else { cols + 2.0 };
            // Bottom rows (high `r`) arrive first; top rows last.
            let row_norm = (r as f32) / rows.max(1.0);
            let start_t = (1.0 - row_norm) * (1.0 - SLIDE_NORM);
            let end_t = (start_t + SLIDE_NORM).min(1.0);
            self.bricks.push(Brick {
                target_row: r,
                target_col: c,
                target_ch: cell.ch,
                target_color: cell.fg,
                start_x,
                start_t,
                end_t,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        for b in &self.bricks {
            if progress < b.start_t {
                continue;
            }
            if progress >= b.end_t {
                frame.set(b.target_row, b.target_col, Cell {
                    ch: b.target_ch,
                    fg: b.target_color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
                continue;
            }
            // Ease-out for a "sliding to a stop" feel.
            let lin = (progress - b.start_t) / (b.end_t - b.start_t);
            let t = 1.0 - (1.0 - lin).powi(3);
            let x = b.start_x + ((b.target_col as f32) - b.start_x) * t;
            let c = x.round() as i32;
            if c < 0 || c >= self.canvas_cols as i32 {
                continue;
            }
            frame.set(b.target_row, c as u16, Cell {
                ch: b.target_ch,
                fg: b.target_color,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.bricks.clear();
        self.elapsed = Duration::ZERO;
    }
}
