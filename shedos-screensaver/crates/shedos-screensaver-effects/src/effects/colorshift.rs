//! colorshift — the target is shown immediately at full opacity;
//! the cells then cycle through the Catppuccin Mocha palette in a
//! continuous color shift before settling on the final color.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use shedos_screensaver_core::Catppuccin;
use std::time::Duration;

const DURATION_MS: u64 = 5_000;

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
}

pub struct Colorshift {
    cells: Vec<TargetCell>,
    elapsed: Duration,
    final_color: Color,
}

impl Colorshift {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO, final_color: Color::TEXT }
    }
}

impl Default for Colorshift {
    fn default() -> Self {
        Self::new()
    }
}

const PALETTE: &[Color] = &[
    Catppuccin::MOCHA.blue,
    Catppuccin::MOCHA.mauve,
    Catppuccin::MOCHA.peach,
    Catppuccin::MOCHA.green,
    Catppuccin::MOCHA.teal,
    Catppuccin::MOCHA.sky,
    Catppuccin::MOCHA.lavender,
    Catppuccin::MOCHA.pink,
];

impl Effect for Colorshift {
    fn name(&self) -> &'static str { "colorshift" }
    fn title(&self) -> &'static str { "Colorshift" }
    fn description(&self) -> &'static str {
        "Target is shown immediately; cells cycle through the Catppuccin palette before settling."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }
    fn reactive(&self) -> bool { true }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.final_color = ctx.final_color;
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.cells.push(TargetCell { row: r, col: c, ch: cell.ch });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let t = self.elapsed.as_secs_f32();
        // Audio: peak amplitude accelerates the color cycle.
        let cycle_speed = 1.0 + audio.map(|a| a.peak).unwrap_or(0.0) * 1.5;

        frame.clear();
        for c in &self.cells {
            // Each cell's hue offset depends on its diagonal index so
            // the wave moves across the canvas, not in lockstep.
            let phase = (c.row as f32 * 0.15 + c.col as f32 * 0.1)
                + t * 1.6 * cycle_speed;
            let pos = (phase % PALETTE.len() as f32 + PALETTE.len() as f32) % PALETTE.len() as f32;
            let i0 = pos as usize % PALETTE.len();
            let i1 = (i0 + 1) % PALETTE.len();
            let frac = pos - pos.floor();
            let mid = mix(PALETTE[i0], PALETTE[i1], frac);
            // Toward the end of the duration, fade toward the final color.
            let settle = ((progress - 0.7) / 0.3).clamp(0.0, 1.0);
            let fg = mix(mid, self.final_color, settle);
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

fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::rgb(
        ((a.r as f32) * (1.0 - t) + b.r as f32 * t) as u8,
        ((a.g as f32) * (1.0 - t) + b.g as f32 * t) as u8,
        ((a.b as f32) * (1.0 - t) + b.b as f32 * t) as u8,
    )
}
