//! Wave Lattice — two interfering sine waves sweep glyphs across
//! the canvas. Phase shifts continuously over time.

use crate::opts::{validate_f32_range, OptType, OptVal, OptionDoc, OptionSchema};
use crate::{Ctx, Style};
use shedos_screensaver_core::{Cell, Color, Frame};

static SCHEMA: OptionSchema = OptionSchema {
    options: &[
        OptionDoc {
            key: "wavelength_x",
            ty: OptType::Float,
            default: OptVal::Float(1.0),
            desc: "X-axis wavelength multiplier (0.1..=10.0)",
            validate: validate_wave,
        },
        OptionDoc {
            key: "wavelength_y",
            ty: OptType::Float,
            default: OptVal::Float(1.5),
            desc: "Y-axis wavelength multiplier (0.1..=10.0)",
            validate: validate_wave,
        },
        OptionDoc {
            key: "speed",
            ty: OptType::Float,
            default: OptVal::Float(1.0),
            desc: "phase advance per second (0.1..=10.0)",
            validate: validate_wave,
        },
    ],
};
fn validate_wave(v: &OptVal) -> Result<(), String> {
    validate_f32_range(0.1, 10.0)(v)
}

const RAMP: &[char] = &[' ', '·', '▒', '▓', '█'];

pub struct Waves;

impl Waves {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Waves {
    fn default() -> Self {
        Self::new()
    }
}

impl Style for Waves {
    fn name(&self) -> &'static str { "waves" }
    fn title(&self) -> &'static str { "Wave Lattice" }
    fn default_color(&self) -> Color { Color::rgb(0xcb, 0xa6, 0xf7) }
    fn option_schema(&self) -> &'static OptionSchema { &SCHEMA }
    fn wants_audio(&self) -> bool { true }
    fn wallpaper_alpha(&self) -> f32 { 0.7 }

    fn draw(&mut self, frame: &mut Frame, ctx: &mut Ctx<'_>) {
        let wlx = ctx.opts.get_f32("wavelength_x").unwrap_or(1.0);
        let wly = ctx.opts.get_f32("wavelength_y").unwrap_or(1.5);
        let speed = ctx.opts.get_f32("speed").unwrap_or(1.0);
        // Audio reactivity: bass shifts wavelength inward (denser waves);
        // peak boosts amplitude (brighter glyphs).
        let bass = ctx.audio.map(|a| a.bass(4)).unwrap_or(0.0);
        let peak = ctx.audio.map(|a| a.peak).unwrap_or(0.0);
        let wlx = wlx * (1.0 + bass * 0.5);
        let wly = wly * (1.0 + bass * 0.3);
        let amp_boost = 1.0 + peak * 0.5;
        let t = ctx.t.as_secs_f32() * speed;
        let cols = frame.cols() as f32;
        let rows = frame.rows() as f32;

        for r in 0..frame.rows() {
            for c in 0..frame.cols() {
                let xn = c as f32 / cols * std::f32::consts::TAU * wlx;
                let yn = r as f32 / rows * std::f32::consts::TAU * wly;
                let v1 = (xn + t).sin();
                let v2 = (yn + t * 1.3).cos();
                let v = (v1 + v2) * 0.5; // -1..1
                let intensity = ((v + 1.0) * 0.5 * amp_boost).clamp(0.0, 1.0); // 0..1
                let idx = (intensity * (RAMP.len() - 1) as f32).round() as usize;
                let glyph = RAMP[idx.min(RAMP.len() - 1)];
                if glyph == ' ' {
                    continue;
                }
                let base = ctx.color;
                let fg = Color::rgb(
                    (base.r as f32 * intensity) as u8,
                    (base.g as f32 * intensity) as u8,
                    (base.b as f32 * intensity) as u8,
                );
                frame.set(r, c, Cell {
                    ch: glyph,
                    fg,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        // SHEDOS logo riding the wave: brightness oscillates with the
        // average wave at the logo's rows, so it looks like the logo
        // is part of the lattice rather than glued on top.
        let avg_intensity = 0.5 + 0.5 * (t * 0.7).sin();
        crate::styles::plasma::overlay_logo_centered(frame, ctx.logo, avg_intensity, t);
    }
}
