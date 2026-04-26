//! wipe — a diagonal "front" sweeps across the canvas; cells behind
//! the front are revealed at full brightness, cells right at the front
//! glow brightly, cells ahead of the front are hidden.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 3_500;
/// Width of the bright "leading edge" in cells.
const FRONT_WIDTH: f32 = 4.0;

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// Diagonal distance from top-left corner; cells reveal when the
    /// front passes their distance.
    distance: f32,
}

pub struct Wipe {
    cells: Vec<TargetCell>,
    elapsed: Duration,
    max_distance: f32,
}

impl Wipe {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO, max_distance: 0.0 }
    }
}

impl Default for Wipe {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Wipe {
    fn name(&self) -> &'static str { "wipe" }
    fn title(&self) -> &'static str { "Wipe" }
    fn description(&self) -> &'static str {
        "A diagonal sweep front moves across the canvas, revealing cells in its wake."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        let max_r = target.rows().max(1) as f32 - 1.0;
        let max_c = target.cols().max(1) as f32 - 1.0;
        self.max_distance = max_r + max_c + FRONT_WIDTH;
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.cells.push(TargetCell {
                row: r,
                col: c,
                ch: cell.ch,
                color: cell.fg,
                distance: r as f32 + c as f32,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let front = progress * self.max_distance;

        frame.clear();
        for c in &self.cells {
            if c.distance + FRONT_WIDTH < front {
                // Fully revealed.
                frame.set(c.row, c.col, Cell {
                    ch: c.ch,
                    fg: c.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else if c.distance < front {
                // In the leading edge — bright accent color (white-ish).
                let edge_progress = (front - c.distance) / FRONT_WIDTH;
                // Mix from full white at front (edge_progress=0) to
                // c.color at trailing edge (edge_progress=1).
                let r = lerp_u8(0xff, c.color.r, edge_progress);
                let g = lerp_u8(0xff, c.color.g, edge_progress);
                let b = lerp_u8(0xff, c.color.b, edge_progress);
                frame.set(c.row, c.col, Cell {
                    ch: c.ch,
                    fg: Color::rgb(r, g, b),
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else {
                // Ahead of front — hidden.
            }
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (a as f32 * (1.0 - t) + b as f32 * t) as u8
}
