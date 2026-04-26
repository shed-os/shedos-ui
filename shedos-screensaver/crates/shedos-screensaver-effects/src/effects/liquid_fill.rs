//! liquid-fill — cells fill from the bottom up like rising water.
//! The water level oscillates with surface tension; cells right at
//! the meniscus shimmer cyan, cells below settled to the target
//! color, cells above stay dark.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
/// Foam-edge band height (in cells).
const SURFACE_BAND: f32 = 1.2;

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}

pub struct LiquidFill {
    cells: Vec<TargetCell>,
    elapsed: Duration,
    rows: u16,
}

impl LiquidFill {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO, rows: 0 }
    }
}

impl Default for LiquidFill {
    fn default() -> Self { Self::new() }
}

impl Effect for LiquidFill {
    fn name(&self) -> &'static str { "liquid-fill" }
    fn title(&self) -> &'static str { "Liquid Fill" }
    fn description(&self) -> &'static str {
        "Cells fill from the bottom up like rising water; surface oscillates with foam at the meniscus."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();
        for (r, c, cell) in target.cells() {
            if cell.ch != ' ' {
                self.cells.push(TargetCell { row: r, col: c, ch: cell.ch, color: cell.fg });
            }
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let t = self.elapsed.as_secs_f32();
        // Water level rises from below the canvas (rows + 1) up past
        // the top (-2) so by progress=1.0 every cell is submerged.
        let level_base = (self.rows as f32 + 1.0) - progress * (self.rows as f32 + 4.0);

        frame.clear();
        for c in &self.cells {
            // Surface tension: small per-column oscillation.
            let wobble = (t * 5.0 + c.col as f32 * 0.4).sin() * 0.4;
            let local_level = level_base + wobble;
            let r_f = c.row as f32;
            if r_f >= local_level + SURFACE_BAND {
                // Submerged — settled at target color.
                frame.set(c.row, c.col, Cell {
                    ch: c.ch,
                    fg: c.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else if r_f >= local_level {
                // At surface — foam shimmer (cyan).
                let depth = (r_f - local_level) / SURFACE_BAND;
                let foam_cyan = Color::rgb(0x90, 0xee, 0xff);
                let r_ch = lerp_u8(foam_cyan.r, c.color.r, depth);
                let g_ch = lerp_u8(foam_cyan.g, c.color.g, depth);
                let b_ch = lerp_u8(foam_cyan.b, c.color.b, depth);
                frame.set(c.row, c.col, Cell {
                    ch: c.ch,
                    fg: Color::rgb(r_ch, g_ch, b_ch),
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
            // else above water — hidden
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
