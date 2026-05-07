//! dawn — gradient brightness sweeps left-to-right across the
//! canvas like a sunrise. Cells reveal through a warm tint as the
//! light front passes their column, then settle to the target.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 4_500;
/// Per-cell warm-tint→target transition window, normalized.
const TRANSITION_NORM: f32 = 0.12;
/// Warm dawn tint (Catppuccin peach) cells pass through during reveal.
const DAWN_TINT: Color = Color::rgb(0xfa, 0xb3, 0x87);

#[derive(Clone, Copy)]
struct DawnCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// Time at which this cell's transition begins.
    reveal_t: f32,
}

pub struct Dawn {
    cells: Vec<DawnCell>,
    elapsed: Duration,
}

impl Dawn {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO }
    }
}

impl Default for Dawn {
    fn default() -> Self { Self::new() }
}

impl Effect for Dawn {
    fn name(&self) -> &'static str { "dawn" }
    fn title(&self) -> &'static str { "Dawn" }
    fn description(&self) -> &'static str {
        "Gradient brightness sweeps left-to-right across the canvas like a sunrise; cells reveal through a warm tint as the light front passes."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        let cols = target.cols() as f32;
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            // Reveal_t leaves room for the transition window so
            // even the rightmost cell finishes by progress=1.0.
            let reveal_t = (c as f32 / cols.max(1.0)) * (1.0 - TRANSITION_NORM);
            self.cells.push(DawnCell {
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
            let trans_t = ((progress - c.reveal_t) / TRANSITION_NORM).clamp(0.0, 1.0);
            let fg = if trans_t >= 1.0 {
                c.color
            } else {
                lerp_color(DAWN_TINT, c.color, trans_t)
            };
            frame.set(c.row, c.col, Cell {
                ch: c.ch,
                fg,
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

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::rgb(
        ((a.r as f32) * (1.0 - t) + (b.r as f32) * t) as u8,
        ((a.g as f32) * (1.0 - t) + (b.g as f32) * t) as u8,
        ((a.b as f32) * (1.0 - t) + (b.b as f32) * t) as u8,
    )
}
