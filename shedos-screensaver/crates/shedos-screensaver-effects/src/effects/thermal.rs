//! thermal — every target cell is visible from the start in dim
//! blue. Cells "heat up" through a thermal colormap (blue → cyan →
//! green → yellow → orange → red → white) at staggered times,
//! then settle to the final target color. No motion — pure color.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 5_000;

const THERMAL_RAMP: &[Color] = &[
    Color::rgb(0x10, 0x10, 0x40), // deep blue (cold)
    Color::rgb(0x20, 0x40, 0x80),
    Color::rgb(0x10, 0x80, 0xc0), // cyan
    Color::rgb(0x10, 0xc0, 0x80),
    Color::rgb(0x80, 0xc0, 0x20), // yellow-green
    Color::rgb(0xff, 0xc0, 0x10), // orange-yellow
    Color::rgb(0xff, 0x60, 0x10), // orange
    Color::rgb(0xff, 0x20, 0x20), // red
    Color::rgb(0xff, 0xff, 0xff), // white-hot
];

#[derive(Clone, Copy)]
struct ThermalCell {
    row: u16,
    col: u16,
    target_ch: char,
    target_color: Color,
    /// 0..1 progress at which this cell starts heating.
    heat_at: f32,
}

pub struct Thermal {
    cells: Vec<ThermalCell>,
    elapsed: Duration,
}

impl Thermal {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO }
    }
}

impl Default for Thermal {
    fn default() -> Self { Self::new() }
}

impl Effect for Thermal {
    fn name(&self) -> &'static str { "thermal" }
    fn title(&self) -> &'static str { "Thermal" }
    fn description(&self) -> &'static str {
        "Cells heat up through a thermal colormap — blue → cyan → yellow → red → white — then settle to the final color."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.cells.push(ThermalCell {
                row: r,
                col: c,
                target_ch: cell.ch,
                target_color: cell.fg,
                heat_at: ctx.rng.gen_range(0.0..0.45),
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();
        for c in &self.cells {
            let warmth = if progress < c.heat_at {
                0.0
            } else {
                ((progress - c.heat_at) / (0.85 - c.heat_at).max(0.001)).clamp(0.0, 1.0)
            };
            // Pick color from thermal ramp based on warmth.
            let idx = warmth * (THERMAL_RAMP.len() - 1) as f32;
            let i0 = (idx as usize).min(THERMAL_RAMP.len() - 1);
            let i1 = (i0 + 1).min(THERMAL_RAMP.len() - 1);
            let frac = idx - idx.floor();
            let mid = mix(THERMAL_RAMP[i0], THERMAL_RAMP[i1], frac);
            // Past 0.9, fade toward target color.
            let settle = ((progress - 0.9) / 0.1).clamp(0.0, 1.0);
            let fg = mix(mid, c.target_color, settle);
            frame.set(c.row, c.col, Cell {
                ch: c.target_ch,
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

fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::rgb(
        ((a.r as f32) * (1.0 - t) + b.r as f32 * t) as u8,
        ((a.g as f32) * (1.0 - t) + b.g as f32 * t) as u8,
        ((a.b as f32) * (1.0 - t) + b.b as f32 * t) as u8,
    )
}
