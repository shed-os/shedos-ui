//! expand — cells start at the canvas center and fly outward to
//! their target positions. Reverse of "collapse".

use crate::easing;
use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 3_500;

#[derive(Clone, Copy)]
struct Particle {
    end_row: u16,
    end_col: u16,
    ch: char,
    color: Color,
}

pub struct Expand {
    particles: Vec<Particle>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
}

impl Expand {
    pub fn new() -> Self {
        Self { particles: Vec::new(), elapsed: Duration::ZERO, rows: 0, cols: 0 }
    }
}

impl Default for Expand {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Expand {
    fn name(&self) -> &'static str { "expand" }
    fn title(&self) -> &'static str { "Expand" }
    fn description(&self) -> &'static str {
        "Cells emerge from the canvas center and fly outward to their target positions."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.particles.clear();
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();
        self.cols = target.cols();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.particles.push(Particle {
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
        let eased = easing::ease_out_cubic(progress);

        let cy = self.rows as f32 * 0.5;
        let cx = self.cols as f32 * 0.5;

        frame.clear();
        for p in &self.particles {
            let target_r = p.end_row as f32;
            let target_c = p.end_col as f32;
            let r_f = easing::lerp(cy, target_r, eased);
            let c_f = easing::lerp(cx, target_c, eased);
            let r = r_f.round() as i32;
            let c = c_f.round() as i32;
            if r < 0 || c < 0 || r >= self.rows as i32 || c >= self.cols as i32 {
                continue;
            }
            // Brightness scales with eased progress.
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

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.particles.clear();
        self.elapsed = Duration::ZERO;
    }
}
