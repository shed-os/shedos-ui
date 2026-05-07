//! lighthouse — a single rotating beam from the canvas center
//! sweeps clockwise; cells reveal as the beam first reaches their
//! angle.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::f32::consts::PI;
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
const ASPECT: f32 = 0.5;
/// Number of full beam sweeps over the duration.
const TURNS: f32 = 1.5;
/// Cap reveal_t so cells past this value land cleanly on target by
/// progress=1.0.
const REVEAL_CAP: f32 = 0.94;

#[derive(Clone, Copy)]
struct BeamCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    reveal_t: f32,
}

pub struct Lighthouse {
    cells: Vec<BeamCell>,
    elapsed: Duration,
}

impl Lighthouse {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO }
    }
}

impl Default for Lighthouse {
    fn default() -> Self { Self::new() }
}

impl Effect for Lighthouse {
    fn name(&self) -> &'static str { "lighthouse" }
    fn title(&self) -> &'static str { "Lighthouse" }
    fn description(&self) -> &'static str {
        "A single rotating beam from the canvas center sweeps clockwise; cells reveal as the beam first reaches their angle."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        let cx = target.cols() as f32 * 0.5;
        let cy = target.rows() as f32 * 0.5;
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            let dx = c as f32 - cx;
            let dy = (r as f32 - cy) / ASPECT;
            // Clockwise angle from "12 o'clock", normalized [0, 1).
            let angle = dy.atan2(dx);
            let mut ang_norm = (angle + PI / 2.0) / (2.0 * PI);
            if ang_norm < 0.0 {
                ang_norm += 1.0;
            }
            ang_norm = 1.0 - ang_norm;
            // Beam passes the cell on its first sweep; with TURNS > 1
            // this happens within the first 1/TURNS of the duration.
            let reveal_t = (ang_norm / TURNS).clamp(0.0, REVEAL_CAP);
            self.cells.push(BeamCell {
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
