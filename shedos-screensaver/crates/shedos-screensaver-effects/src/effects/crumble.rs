//! crumble — cells tumble in from above with gravity. Each particle
//! has a slight horizontal drift and bounces lightly on landing
//! before settling.

use crate::easing;
use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 4_500;

struct Particle {
    end_row: u16,
    end_col: u16,
    start_col: f32,
    drift_amplitude: f32,
    drift_phase: f32,
    ch: char,
    color: Color,
    /// 0..1 normalized landing time (when this particle settles).
    land_at: f32,
}

pub struct Crumble {
    particles: Vec<Particle>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
}

impl Crumble {
    pub fn new() -> Self {
        Self { particles: Vec::new(), elapsed: Duration::ZERO, rows: 0, cols: 0 }
    }
}

impl Default for Crumble {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Crumble {
    fn name(&self) -> &'static str { "crumble" }
    fn title(&self) -> &'static str { "Crumble" }
    fn description(&self) -> &'static str {
        "Cells tumble in from above with gravity and a small horizontal drift; settle into the target."
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
            // Per-particle randomization: horizontal drift, landing time.
            self.particles.push(Particle {
                end_row: r,
                end_col: c,
                start_col: c as f32 + ctx.rng.gen_range(-3.0..3.0),
                drift_amplitude: ctx.rng.gen_range(0.5..2.0),
                drift_phase: ctx.rng.gen_range(0.0..std::f32::consts::TAU),
                ch: cell.ch,
                color: cell.fg,
                land_at: ctx.rng.gen_range(0.6..1.0),
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
            if progress >= p.land_at {
                // Settled.
                frame.set(p.end_row, p.end_col, Cell {
                    ch: p.ch,
                    fg: p.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else {
                all_landed = false;
                // Local progress 0..1 for this particle's fall.
                let local = progress / p.land_at;
                let eased = easing::ease_in_quad(local);
                let r_f = easing::lerp(-1.0, p.end_row as f32, eased);
                // Drift horizontally with a sine wave that converges to 0.
                let drift = p.drift_amplitude
                    * (p.drift_phase + local * std::f32::consts::TAU * 1.5).sin()
                    * (1.0 - local);
                let c_f = easing::lerp(p.start_col, p.end_col as f32, eased) + drift;
                let r = r_f.round() as i32;
                let c = c_f.round() as i32;
                if r < 0 || c < 0 || r >= self.rows as i32 || c >= self.cols as i32 {
                    continue;
                }
                let brightness = 0.5 + 0.5 * eased;
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
        }

        all_landed && progress >= 1.0
    }

    fn reset(&mut self) {
        self.particles.clear();
        self.elapsed = Duration::ZERO;
    }
}
