//! shockwave — a radial pulse expands from the canvas center.
//! Cells reveal as the wavefront passes over them, with a bright
//! white-hot leading edge that fades to the target color behind.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 4_000;
/// Width of the leading edge in cells.
const EDGE_WIDTH: f32 = 5.0;
const ASPECT: f32 = 0.5;

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// Distance from canvas center (cell units, aspect-corrected).
    dist: f32,
}

pub struct Shockwave {
    cells: Vec<TargetCell>,
    elapsed: Duration,
    max_dist: f32,
}

impl Shockwave {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO, max_dist: 0.0 }
    }
}

impl Default for Shockwave {
    fn default() -> Self { Self::new() }
}

impl Effect for Shockwave {
    fn name(&self) -> &'static str { "shockwave" }
    fn title(&self) -> &'static str { "Shockwave" }
    fn description(&self) -> &'static str {
        "A radial pulse expands from the canvas center; cells reveal as the wavefront passes them, with a bright leading edge."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        let cx = target.cols() as f32 * 0.5;
        let cy = target.rows() as f32 * 0.5;
        let mut max_d = 0.0_f32;
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            let dx = c as f32 - cx;
            let dy = (r as f32 - cy) / ASPECT;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > max_d {
                max_d = dist;
            }
            self.cells.push(TargetCell {
                row: r,
                col: c,
                ch: cell.ch,
                color: cell.fg,
                dist,
            });
        }
        self.max_dist = max_d + EDGE_WIDTH;
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let front = progress * self.max_dist;

        frame.clear();
        for c in &self.cells {
            if c.dist + EDGE_WIDTH < front {
                // Fully revealed.
                frame.set(c.row, c.col, Cell {
                    ch: c.ch,
                    fg: c.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else if c.dist < front {
                // In the leading edge: bright white-hot fading to target.
                let edge_t = (front - c.dist) / EDGE_WIDTH;
                let r_ch = lerp_u8(0xff, c.color.r, edge_t);
                let g_ch = lerp_u8(0xff, c.color.g, edge_t);
                let b_ch = lerp_u8(0xff, c.color.b, edge_t);
                frame.set(c.row, c.col, Cell {
                    ch: c.ch,
                    fg: Color::rgb(r_ch, g_ch, b_ch),
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
            // else hidden: wave hasn't reached yet
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
