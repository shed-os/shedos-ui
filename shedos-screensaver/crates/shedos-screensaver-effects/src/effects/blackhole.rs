//! blackhole — cells start at random positions across the canvas,
//! spiral inward to the center under a "gravity" curve, vanish for a
//! beat at the singularity, then explode outward to their target
//! positions while counter-rotating. Three-phase dramatic rebirth.

use crate::easing;
use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 6_000;
const PHASE_INWARD_END: f32 = 0.42;
const PHASE_VANISHED_END: f32 = 0.50;
/// Aspect correction: terminal cells are roughly twice as tall as
/// wide, so radial geometry needs to compensate.
const ASPECT: f32 = 0.5;

struct Particle {
    /// Where the particle starts (random scatter on canvas).
    start_x: f32,
    start_y: f32,
    /// Where it lands (target position).
    end_row: u16,
    end_col: u16,
    ch: char,
    color: Color,
}

pub struct Blackhole {
    particles: Vec<Particle>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
}

impl Blackhole {
    pub fn new() -> Self {
        Self { particles: Vec::new(), elapsed: Duration::ZERO, rows: 0, cols: 0 }
    }
}

impl Default for Blackhole {
    fn default() -> Self { Self::new() }
}

impl Effect for Blackhole {
    fn name(&self) -> &'static str { "blackhole" }
    fn title(&self) -> &'static str { "Blackhole" }
    fn description(&self) -> &'static str {
        "Cells spiral into the canvas center under gravity, vanish, then explode outward to their target positions. Dramatic rebirth."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.particles.clear();
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();
        self.cols = target.cols();
        let max_x = self.cols.max(1) as f32;
        let max_y = self.rows.max(1) as f32;
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.particles.push(Particle {
                start_x: ctx.rng.gen_range(0.0..max_x),
                start_y: ctx.rng.gen_range(0.0..max_y),
                end_row: r,
                end_col: c,
                ch: cell.ch,
                color: cell.fg,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let cx = self.cols as f32 * 0.5;
        let cy = self.rows as f32 * 0.5;

        frame.clear();

        if progress < PHASE_INWARD_END {
            // Spiral inward.
            let local = progress / PHASE_INWARD_END;
            let eased = easing::ease_in_quad(local);
            for p in &self.particles {
                let dx = p.start_x - cx;
                let dy = (p.start_y - cy) / ASPECT;
                let r0 = (dx * dx + dy * dy).sqrt();
                let theta0 = dy.atan2(dx);
                let r = r0 * (1.0 - eased);
                let theta = theta0 + eased * 6.0; // ~1 full rotation as r → 0
                let x = cx + r * theta.cos();
                let y = cy + r * theta.sin() * ASPECT;
                let xi = x.round() as i32;
                let yi = y.round() as i32;
                if xi < 0 || yi < 0 || xi >= self.cols as i32 || yi >= self.rows as i32 {
                    continue;
                }
                // Brighten as cells approach the singularity.
                let brightness = 0.5 + 0.5 * eased;
                let fg = scale(p.color, brightness);
                frame.set(yi as u16, xi as u16, Cell {
                    ch: p.ch,
                    fg,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        } else if progress < PHASE_VANISHED_END {
            // Vanished — single bright pixel at the center suggests the singularity.
            let cx_i = cx.round() as i32;
            let cy_i = cy.round() as i32;
            if cx_i >= 0 && cy_i >= 0 && cx_i < self.cols as i32 && cy_i < self.rows as i32 {
                frame.set(cy_i as u16, cx_i as u16, Cell {
                    ch: '◉',
                    fg: Color::rgb(0xff, 0xff, 0xff),
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        } else {
            // Explode outward to target.
            let local = (progress - PHASE_VANISHED_END) / (1.0 - PHASE_VANISHED_END);
            let eased = easing::ease_out_cubic(local);
            for p in &self.particles {
                let to_x = p.end_col as f32;
                let to_y = p.end_row as f32;
                let dx = to_x - cx;
                let dy = (to_y - cy) / ASPECT;
                let r_dest = (dx * dx + dy * dy).sqrt();
                let theta_dest = dy.atan2(dx);
                let r = r_dest * eased;
                // Counter-rotate as cells approach (start spinning,
                // settle still). 4 rad of counter-rotation total.
                let theta = theta_dest - (1.0 - eased) * 4.0;
                let x = cx + r * theta.cos();
                let y = cy + r * theta.sin() * ASPECT;
                let xi = x.round() as i32;
                let yi = y.round() as i32;
                if xi < 0 || yi < 0 || xi >= self.cols as i32 || yi >= self.rows as i32 {
                    continue;
                }
                frame.set(yi as u16, xi as u16, Cell {
                    ch: p.ch,
                    fg: p.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.particles.clear();
        self.elapsed = Duration::ZERO;
    }
}

fn scale(c: Color, k: f32) -> Color {
    let k = k.clamp(0.0, 1.0);
    Color::rgb(
        (c.r as f32 * k) as u8,
        (c.g as f32 * k) as u8,
        (c.b as f32 * k) as u8,
    )
}
