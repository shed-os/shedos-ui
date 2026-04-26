//! Plasma Field — sin/cos blended plasma using ANSI block characters
//! and continuously-varying truecolor.

use crate::opts::{validate_f32_range, OptType, OptVal, OptionDoc, OptionSchema};
use crate::{Ctx, Style};
use shedos_screensaver_core::{Cell, Color, Frame, Logo};

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
                frame.set(r, c, Cell {
                    ch: '█',
                    fg: Color::rgb(gr, gg, gb),
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        // SHEDOS logo overlaid in the center, white-on-transparent so
        // the plasma still bleeds through behind it. The logo "breathes"
        // gently with t.
        let pulse = 0.7 + 0.3 * (t * 0.6).sin();
        overlay_logo_centered(frame, ctx.logo, pulse, t);
    }
}

/// Render the SHEDOS art centered horizontally + vertically on the
/// frame, overwriting the background pattern. `pulse` (0..1) scales
/// the brightness of the overlay so styles can make it breathe with
/// the animation. Used by plasma, waves, and any other "abstract
/// background" style that wants the brand mark prominent.
pub(crate) fn overlay_logo_centered(frame: &mut Frame, logo: &Logo, pulse: f32, _t: f32) {
    if logo.rows == 0 || logo.cols == 0 {
        return;
    }
    let row_offset = (frame.rows().saturating_sub(logo.rows) / 2) as i32;
    let col_offset = (frame.cols().saturating_sub(logo.cols) / 2) as i32;
    let pulse = pulse.clamp(0.0, 1.0);
    let r_ch = (255.0 * pulse) as u8;
    let g_ch = (255.0 * pulse) as u8;
    let b_ch = (255.0 * pulse) as u8;
    let fg = Color::rgb(r_ch, g_ch, b_ch);
    for lr in 0..logo.rows as i32 {
        for lc in 0..logo.cols as i32 {
            if !logo.lit(lr as usize, lc as usize) {
                continue;
            }
            let fr = row_offset + lr;
            let fc = col_offset + lc;
            if fr < 0 || fc < 0 || fr >= frame.rows() as i32 || fc >= frame.cols() as i32 {
                continue;
            }
            frame.set(fr as u16, fc as u16, Cell {
                ch: logo.glyph_at(lr as usize, lc as usize),
                fg,
                bg: Color::BASE,
                attrs: Default::default(),
            });
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
