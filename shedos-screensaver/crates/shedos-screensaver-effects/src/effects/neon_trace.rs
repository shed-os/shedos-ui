//! neon-trace — cells light up one by one with a bright neon glow
//! (pink/cyan), then fade to the target color. The glow leaves a
//! phosphor trail behind the leading edge — like a CRT vector
//! display drawing the letters.

use crate::easing;
use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use shedos_screensaver_core::CellAttrs;
use std::time::Duration;

const DURATION_MS: u64 = 5_500;
/// Neon glow lead-in: cell holds in pink/cyan for this fraction of
/// total before fading to the target color.
const GLOW_WINDOW: f32 = 0.18;
const NEON_PINK: Color = Color::rgb(0xff, 0x33, 0xcc);
const NEON_CYAN: Color = Color::rgb(0x33, 0xff, 0xee);

#[derive(Clone, Copy)]
struct TraceCell {
    row: u16,
    col: u16,
    target_ch: char,
    target_color: Color,
    /// 0..1 progress when the cell first lights up neon.
    activate_at: f32,
    /// 0..1 progress when the cell finishes settling to target.
    settle_at: f32,
    /// Which neon hue this cell glows in (alternates pink/cyan
    /// across diagonals so the leading edge looks two-tone).
    use_pink: bool,
}

pub struct NeonTrace {
    cells: Vec<TraceCell>,
    elapsed: Duration,
}

impl NeonTrace {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO }
    }
}

impl Default for NeonTrace {
    fn default() -> Self { Self::new() }
}

impl Effect for NeonTrace {
    fn name(&self) -> &'static str { "neon-trace" }
    fn title(&self) -> &'static str { "Neon Trace" }
    fn description(&self) -> &'static str {
        "Cells light up neon-pink/cyan one by one along a diagonal sweep, then fade to the target color — CRT vector phosphor trail."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        let mut lit: Vec<(u16, u16, char, Color)> = target
            .cells()
            .filter_map(|(r, c, cell)| {
                if cell.ch != ' ' { Some((r, c, cell.ch, cell.fg)) } else { None }
            })
            .collect();
        // Top-left → bottom-right diagonal sweep so the trace front
        // moves predictably across the canvas.
        lit.sort_by_key(|&(r, c, _, _)| r as u32 + c as u32);
        let total = lit.len().max(1) as f32;
        for (i, (r, c, ch, color)) in lit.into_iter().enumerate() {
            let activate = (i as f32 / total) * (1.0 - GLOW_WINDOW);
            self.cells.push(TraceCell {
                row: r,
                col: c,
                target_ch: ch,
                target_color: color,
                activate_at: activate,
                settle_at: (activate + GLOW_WINDOW).min(1.0),
                use_pink: ((r as u32 + c as u32) % 2) == 0,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();
        for c in &self.cells {
            if progress < c.activate_at {
                continue; // not yet lit
            }
            let neon = if c.use_pink { NEON_PINK } else { NEON_CYAN };
            if progress >= c.settle_at {
                // Settled at target color.
                frame.set(c.row, c.col, Cell {
                    ch: c.target_ch,
                    fg: c.target_color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else {
                // Glowing — interpolate from neon → target.
                let local = (progress - c.activate_at) / (c.settle_at - c.activate_at);
                let eased = easing::ease_out_cubic(local);
                let r_ch = lerp_u8(neon.r, c.target_color.r, eased);
                let g_ch = lerp_u8(neon.g, c.target_color.g, eased);
                let b_ch = lerp_u8(neon.b, c.target_color.b, eased);
                let attrs = if eased < 0.4 { CellAttrs::BOLD } else { CellAttrs::NONE };
                frame.set(c.row, c.col, Cell {
                    ch: c.target_ch,
                    fg: Color::rgb(r_ch, g_ch, b_ch),
                    bg: Color::BASE,
                    attrs,
                });
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
