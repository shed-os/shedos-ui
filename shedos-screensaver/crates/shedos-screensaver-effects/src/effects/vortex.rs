//! vortex — a clockwise spiral arm sweeps inward from the canvas
//! perimeter to the center; cells reveal at canonical positions as
//! the arm passes their (angle, radius).

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::f32::consts::PI;
use std::time::Duration;

const DURATION_MS: u64 = 4_500;
/// Number of full sweeps the arm makes from rim to center.
const TURNS: f32 = 2.0;
const ASPECT: f32 = 0.5;
/// Cap reveal_t so cells past this point fully resolve before
/// the duration ends — keeps the final frame an exact match for
/// the target.
const REVEAL_CAP: f32 = 0.94;

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// Normalized [0, 1] time at which this cell should reveal.
    reveal_t: f32,
}

pub struct Vortex {
    cells: Vec<TargetCell>,
    elapsed: Duration,
}

impl Vortex {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO }
    }
}

impl Default for Vortex {
    fn default() -> Self { Self::new() }
}

impl Effect for Vortex {
    fn name(&self) -> &'static str { "vortex" }
    fn title(&self) -> &'static str { "Vortex" }
    fn description(&self) -> &'static str {
        "A clockwise spiral arm sweeps inward from canvas perimeter to center; cells reveal at their canonical positions as the arm passes their (angle, radius)."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        let cx = target.cols() as f32 * 0.5;
        let cy = target.rows() as f32 * 0.5;
        let max_r = ((cx * cx) + (cy / ASPECT * cy / ASPECT)).sqrt().max(1.0);
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            let dx = c as f32 - cx;
            let dy = (r as f32 - cy) / ASPECT;
            let radius = (dx * dx + dy * dy).sqrt();
            // Clockwise angle from "12 o'clock", normalized [0, 1).
            let angle = dy.atan2(dx);                       // [-π, π], 0 = east
            let mut ang_norm = (angle + PI / 2.0) / (2.0 * PI); // shift so north = 0
            if ang_norm < 0.0 {
                ang_norm += 1.0;
            }
            ang_norm = 1.0 - ang_norm;                       // clockwise

            let norm_radius = (radius / max_r).clamp(0.0, 1.0);
            // Outer cells reveal in the first turn; inner cells in the
            // second. Within each turn, cells reveal in clockwise
            // angular order.
            let raw = (1.0 - norm_radius) * (1.0 / TURNS) + ang_norm / TURNS;
            let reveal_t = raw.clamp(0.0, REVEAL_CAP);
            self.cells.push(TargetCell {
                row: r,
                col: c,
                ch: cell.ch,
                color: cell.fg,
                reveal_t,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        for c in &self.cells {
            if progress < c.reveal_t {
                continue;
            }
            frame.set(c.row, c.col, Cell {
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
