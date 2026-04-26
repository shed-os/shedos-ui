//! Warp Stars — 3D-perspective stars streaming outward from center.
//! SHEDOS logo pulses at the vanishing point.

use crate::opts::{validate_f32_range, validate_u32_range, OptType, OptVal, OptionDoc, OptionSchema};
use crate::{Ctx, Style};
use rand::Rng;
use shedos_screensaver_core::{Cell, Color, Frame};

static SCHEMA: OptionSchema = OptionSchema {
    options: &[
        OptionDoc {
            key: "count",
            ty: OptType::UInt,
            default: OptVal::UInt(200),
            desc: "number of stars (1..=10000)",
            validate: validate_count,
        },
        OptionDoc {
            key: "warp_factor",
            ty: OptType::Float,
            default: OptVal::Float(5.0),
            desc: "speed of perspective motion (1.0..=100.0)",
            validate: validate_warp,
        },
    ],
};
fn validate_count(v: &OptVal) -> Result<(), String> {
    validate_u32_range(1, 10000)(v)
}
fn validate_warp(v: &OptVal) -> Result<(), String> {
    validate_f32_range(1.0, 100.0)(v)
}

#[derive(Debug, Clone, Copy)]
struct Star {
    x: f32, // -1..1 normalized; * z = screen offset
    y: f32,
    z: f32, // 0..1 (1 = far, 0 = at viewer)
}

pub struct Starfield {
    stars: Vec<Star>,
    seeded: bool,
}

impl Starfield {
    pub fn new() -> Self {
        Self { stars: Vec::new(), seeded: false }
    }
}

impl Default for Starfield {
    fn default() -> Self {
        Self::new()
    }
}

impl Style for Starfield {
    fn name(&self) -> &'static str { "starfield" }
    fn title(&self) -> &'static str { "Warp Stars" }
    fn default_color(&self) -> Color { Color::rgb(0xcd, 0xd6, 0xf4) }
    fn option_schema(&self) -> &'static OptionSchema { &SCHEMA }
    fn wants_audio(&self) -> bool { true }

    fn draw(&mut self, frame: &mut Frame, ctx: &mut Ctx<'_>) {
        let count = ctx.opts.get_u32("count").unwrap_or(200) as usize;
        let warp = ctx.opts.get_f32("warp_factor").unwrap_or(5.0);
        // Audio reactivity: a beat doubles warp factor for that frame
        // (snapshot effect — feels like an FTL "kick").
        let warp = if ctx.audio.map(|a| a.beat).unwrap_or(false) {
            warp * 2.0
        } else {
            warp
        };
        let dt = ctx.dt.as_secs_f32();

        // (Re)populate to current count.
        if !self.seeded || self.stars.len() != count {
            self.stars.clear();
            for _ in 0..count {
                self.stars.push(spawn(ctx.rng));
            }
            self.seeded = true;
        }

        let cx = frame.cols() as f32 * 0.5;
        let cy = frame.rows() as f32 * 0.5;

        // Pulse the logo at center: oscillate intensity with sin(t).
        let pulse = ((ctx.t.as_secs_f32() * 1.5).sin() * 0.5 + 0.5) * 0.8 + 0.2;
        let logo = ctx.logo;
        let logo_ofs_r = (cy - logo.rows as f32 * 0.5) as i32;
        let logo_ofs_c = (cx - logo.cols as f32 * 0.5) as i32;
        for r in 0..logo.rows as i32 {
            for c in 0..logo.cols as i32 {
                if !logo.lit(r as usize, c as usize) {
                    continue;
                }
                let fr = logo_ofs_r + r;
                let fc = logo_ofs_c + c;
                if fr < 0 || fc < 0 {
                    continue;
                }
                let base = ctx.color;
                let fg = Color::rgb(
                    (base.r as f32 * pulse) as u8,
                    (base.g as f32 * pulse) as u8,
                    (base.b as f32 * pulse) as u8,
                );
                frame.set(fr as u16, fc as u16, Cell {
                    ch: logo.glyph_at(r as usize, c as usize),
                    fg,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        // Advance + render stars.
        for s in &mut self.stars {
            s.z -= warp * 0.05 * dt;
            if s.z <= 0.01 {
                *s = spawn(ctx.rng);
            }
            // Project: screen pos = (cx + x/z * scale, cy + y/z * scale).
            let scale = (frame.rows().min(frame.cols()) as f32) * 0.5;
            let sx = cx + (s.x / s.z) * scale;
            let sy = cy + (s.y / s.z) * scale;
            if sx < 0.0 || sy < 0.0 || sx >= frame.cols() as f32 || sy >= frame.rows() as f32 {
                continue;
            }
            let intensity = (1.0 - s.z).clamp(0.0, 1.0);
            let glyph = if intensity > 0.85 {
                '*'
            } else if intensity > 0.5 {
                '+'
            } else {
                '.'
            };
            let base = ctx.color;
            let fg = Color::rgb(
                (base.r as f32 * intensity) as u8,
                (base.g as f32 * intensity) as u8,
                (base.b as f32 * intensity) as u8,
            );
            frame.set(sy as u16, sx as u16, Cell {
                ch: glyph,
                fg,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }
    }
}

fn spawn(rng: &mut impl Rng) -> Star {
    Star {
        x: rng.gen_range(-1.0..1.0),
        y: rng.gen_range(-1.0..1.0),
        z: rng.gen_range(0.5..1.0),
    }
}
