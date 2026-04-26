//! scattered — every target cell starts at a random position on the
//! canvas and flies to its destination with eased motion. Looks like
//! pieces snapping into a puzzle.

use crate::easing;
use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 4_000;

struct Particle {
    start_row: f32,
    start_col: f32,
    end_row: u16,
    end_col: u16,
    ch: char,
    color: Color,
    /// Per-particle stagger so they don't all start moving at once.
    delay: f32,
}

pub struct Scattered {
    particles: Vec<Particle>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
}

impl Scattered {
    pub fn new() -> Self {
        Self { particles: Vec::new(), elapsed: Duration::ZERO, rows: 0, cols: 0 }
    }
}

impl Default for Scattered {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Scattered {
    fn name(&self) -> &'static str { "scattered" }
    fn title(&self) -> &'static str { "Scattered" }
    fn description(&self) -> &'static str {
        "Cells start scattered across the canvas and fly to their target positions with eased motion."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.particles.clear();
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();
        self.cols = target.cols();

        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.particles.push(Particle {
                start_row: ctx.rng.gen_range(0..self.rows.max(1)) as f32,
                start_col: ctx.rng.gen_range(0..self.cols.max(1)) as f32,
                end_row: r,
                end_col: c,
                ch: cell.ch,
                color: cell.fg,
                delay: ctx.rng.gen_range(0.0..0.3),
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();
        let mut all_landed = true;

        for p in &self.particles {
            // Each particle's local progress: starts after its delay,
            // ends at duration. Compress into [0, 1].
            let active_start = p.delay;
            let active_end = 1.0;
            let local = ((progress - active_start) / (active_end - active_start)).clamp(0.0, 1.0);
            let eased = easing::ease_out_cubic(local);
            if eased < 1.0 {
                all_landed = false;
            }

            let r_f = easing::lerp(p.start_row, p.end_row as f32, eased);
            let c_f = easing::lerp(p.start_col, p.end_col as f32, eased);
            let r = r_f.round() as i32;
            let c = c_f.round() as i32;
            if r < 0 || c < 0 || r >= self.rows as i32 || c >= self.cols as i32 {
                continue;
            }
            // Dimmer while in flight; full brightness once landed.
            let brightness = 0.4 + 0.6 * eased;
            let fg = Color::rgb(
                (p.color.r as f32 * brightness) as u8,
                (p.color.g as f32 * brightness) as u8,
                (p.color.b as f32 * brightness) as u8,
            );
            frame.set(r as u16, c as u16, Cell {
                ch: p.ch,
                fg,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }

        all_landed && progress >= 1.0
    }

    fn reset(&mut self) {
        self.particles.clear();
        self.elapsed = Duration::ZERO;
    }
}
