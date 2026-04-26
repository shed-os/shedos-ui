//! Plasma Field — sin/cos blended plasma using ANSI block characters
//! and continuously-varying truecolor.

use crate::opts::{validate_f32_range, OptType, OptVal, OptionDoc, OptionSchema};
use crate::{Ctx, Style};
use shedos_screensaver_core::{Cell, Color, Frame};

static SCHEMA: OptionSchema = OptionSchema {
    options: &[
        OptionDoc {
            key: "freq_x",
            ty: OptType::Float,
            default: OptVal::Float(1.0),
            desc: "X-axis spatial frequency (0.1..=10.0)",
            validate: validate_freq,
        },
        OptionDoc {
            key: "freq_y",
            ty: OptType::Float,
            default: OptVal::Float(1.5),
            desc: "Y-axis spatial frequency (0.1..=10.0)",
            validate: validate_freq,
        },
    ],
};
fn validate_freq(v: &OptVal) -> Result<(), String> {
    validate_f32_range(0.1, 10.0)(v)
}

pub struct Plasma;

impl Plasma {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Plasma {
    fn default() -> Self {
        Self::new()
    }
}

impl Style for Plasma {
    fn name(&self) -> &'static str { "plasma" }
    fn title(&self) -> &'static str { "Plasma Field" }
    fn default_color(&self) -> Color { Color::rgb(0xcb, 0xa6, 0xf7) }
    fn option_schema(&self) -> &'static OptionSchema { &SCHEMA }
    fn wants_audio(&self) -> bool { true }
    fn wallpaper_alpha(&self) -> f32 { 0.65 }

    fn draw(&mut self, frame: &mut Frame, ctx: &mut Ctx<'_>) {
        let fx = ctx.opts.get_f32("freq_x").unwrap_or(1.0);
        let fy = ctx.opts.get_f32("freq_y").unwrap_or(1.5);
        // Audio reactivity: bass deforms freq_x; treble deforms freq_y.
        // Range expansion is gentle (≤ ~1× extra) so visuals stay smooth.
        let bass = ctx.audio.map(|a| a.bass(4)).unwrap_or(0.0);
        let treble = ctx.audio.map(|a| a.band_range(20, 32)).unwrap_or(0.0);
        let fx = fx * (1.0 + bass);
        let fy = fy * (1.0 + treble * 0.7);
        let t = ctx.t.as_secs_f32();
        let rows = frame.rows() as f32;
        let cols = frame.cols() as f32;

        for r in 0..frame.rows() {
            for c in 0..frame.cols() {
                let x = c as f32 / cols * std::f32::consts::TAU * fx;
                let y = r as f32 / rows * std::f32::consts::TAU * fy;
                let v = (x + t * 0.7).sin()
                    + (y + t * 0.5).cos()
                    + ((x * 0.5 + y * 0.5) + t * 0.3).sin();
                let n = (v + 3.0) / 6.0; // normalize 0..1
                let (gr, gg, gb) = plasma_color(n, ctx.color);
                // Use a half-block to stack two colors per cell vertically;
                // simpler: just fill with full block for the foreground color.
                frame.set(r, c, Cell {
                    ch: '█',
                    fg: Color::rgb(gr, gg, gb),
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }
    }
}

fn plasma_color(n: f32, base: Color) -> (u8, u8, u8) {
    // Tone-shift the base color through a small phase wheel.
    let t = (n * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    let r = lerp(base.r as f32 * 0.3, base.r as f32, t) as u8;
    let g = lerp(base.g as f32 * 0.3, base.g as f32, t) as u8;
    let b = lerp(base.b as f32 * 0.3, base.b as f32, t) as u8;
    (r, g, b)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}
