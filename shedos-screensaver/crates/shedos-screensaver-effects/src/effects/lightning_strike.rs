//! lightning-strike — branched lightning bolts strike at random
//! angles; each bolt illuminates a cluster of cells in a brief
//! white flash that fades to the target color.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::seq::SliceRandom;
use std::time::Duration;

const DURATION_MS: u64 = 4_000;
const N_STRIKES: usize = 6;
/// White-flash decay window per strike (ms).
const FLASH_WIDTH_MS: u64 = 280;

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// Normalized strike-trigger time [0, 1].
    trigger_t: f32,
}

pub struct LightningStrike {
    cells: Vec<TargetCell>,
    elapsed: Duration,
}

impl LightningStrike {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO }
    }
}

impl Default for LightningStrike {
    fn default() -> Self { Self::new() }
}

impl Effect for LightningStrike {
    fn name(&self) -> &'static str { "lightning-strike" }
    fn title(&self) -> &'static str { "Lightning Strike" }
    fn description(&self) -> &'static str {
        "Branched lightning bolts strike at random angles; each bolt illuminates a cluster of cells in a brief white flash that fades to the target color."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;

        // Reserve the last flash window for the final group's settle.
        let total_ms = DURATION_MS as f32;
        let flash_norm = (FLASH_WIDTH_MS as f32) / total_ms;
        // Strikes fire over the first (1 - flash_norm) of the duration,
        // each evenly spaced.
        let last_trigger = (1.0 - flash_norm).max(0.05);
        let strike_spacing = last_trigger / (N_STRIKES as f32 - 1.0).max(1.0);

        let mut lit: Vec<(u16, u16, char, Color)> = Vec::new();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            lit.push((r, c, cell.ch, cell.fg));
        }
        lit.shuffle(ctx.rng);

        if lit.is_empty() {
            return;
        }
        let group_size = lit.len().div_ceil(N_STRIKES);
        for (i, &(r, c, ch, color)) in lit.iter().enumerate() {
            let group = (i / group_size).min(N_STRIKES - 1);
            let trigger_t = (group as f32) * strike_spacing;
            self.cells.push(TargetCell { row: r, col: c, ch, color, trigger_t });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let flash_width = (FLASH_WIDTH_MS as f32) / (DURATION_MS as f32);

        frame.clear();

        for c in &self.cells {
            if progress < c.trigger_t {
                continue;
            }
            // Bright white at trigger, fades to target color over flash_width.
            let flash_t = ((progress - c.trigger_t) / flash_width).clamp(0.0, 1.0);
            let fg = if flash_t >= 1.0 {
                c.color
            } else {
                Color::rgb(
                    lerp_u8(0xff, c.color.r, flash_t),
                    lerp_u8(0xff, c.color.g, flash_t),
                    lerp_u8(0xff, c.color.b, flash_t),
                )
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

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (a as f32 * (1.0 - t) + b as f32 * t) as u8
}
