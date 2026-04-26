//! Tunnel — concentric rings of glyphs zoom toward the viewer;
//! SHEDOS logo glows at the vanishing point.

use crate::opts::{validate_f32_range, validate_u32_range, OptType, OptVal, OptionDoc, OptionSchema};
use crate::{Ctx, Style};
use shedos_screensaver_core::{Cell, Color, Frame};

static SCHEMA: OptionSchema = OptionSchema {
    options: &[
        OptionDoc {
            key: "rings",
            ty: OptType::UInt,
            default: OptVal::UInt(20),
            desc: "number of concentric rings (5..=50)",
            validate: validate_rings,
        },
        OptionDoc {
            key: "speed",
            ty: OptType::Float,
            default: OptVal::Float(1.0),
            desc: "inward zoom speed multiplier (0.1..=10.0)",
            validate: validate_speed,
        },
    ],
};
fn validate_rings(v: &OptVal) -> Result<(), String> {
    validate_u32_range(5, 50)(v)
}
fn validate_speed(v: &OptVal) -> Result<(), String> {
    validate_f32_range(0.1, 10.0)(v)
}

pub struct Tunnel;

impl Tunnel {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Tunnel {
    fn default() -> Self {
        Self::new()
    }
}

impl Style for Tunnel {
    fn name(&self) -> &'static str { "tunnel" }
    fn title(&self) -> &'static str { "Tunnel" }
    fn default_color(&self) -> Color { Color::rgb(0x89, 0xb4, 0xfa) }
    fn option_schema(&self) -> &'static OptionSchema { &SCHEMA }
    fn wants_audio(&self) -> bool { true }

    fn draw(&mut self, frame: &mut Frame, ctx: &mut Ctx<'_>) {
        let rings = ctx.opts.get_u32("rings").unwrap_or(20) as f32;
        let speed = ctx.opts.get_f32("speed").unwrap_or(1.0);
        let t = ctx.t.as_secs_f32() * speed;
        let cx = frame.cols() as f32 * 0.5;
        let cy = frame.rows() as f32 * 0.5;
        let aspect = 0.5; // terminal cells are roughly twice as tall as wide
        let max_dist = (cx * cx + (cy / aspect) * (cy / aspect)).sqrt();

        for r in 0..frame.rows() {
            for c in 0..frame.cols() {
                let dx = c as f32 - cx;
                let dy = (r as f32 - cy) / aspect;
                let dist = (dx * dx + dy * dy).sqrt();
                let normalized = dist / max_dist; // 0..1 from center to corner
                // Ring position: shift inward over time so rings appear to
                // travel from center outward (the viewer flying forward).
                let ring_pos = (normalized * rings - t * 4.0).fract();
                let ring_thickness = 0.18;
                if ring_pos < ring_thickness {
                    let intensity = 1.0 - (1.0 - normalized).powi(2);
                    let base = ctx.color;
                    let fg = Color::rgb(
                        (base.r as f32 * intensity) as u8,
                        (base.g as f32 * intensity) as u8,
                        (base.b as f32 * intensity) as u8,
                    );
                    let glyph = if ring_pos < ring_thickness * 0.4 { '█' }
                        else if ring_pos < ring_thickness * 0.7 { '▓' }
                        else { '▒' };
                    frame.set(r, c, Cell {
                        ch: glyph,
                        fg,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
            }
        }

        // Logo at vanishing point (centered, untouched by tunnel).
        let logo = ctx.logo;
        let logo_r0 = (cy - logo.rows as f32 * 0.5) as i32;
        let logo_c0 = (cx - logo.cols as f32 * 0.5) as i32;
        for r in 0..logo.rows as i32 {
            for c in 0..logo.cols as i32 {
                if !logo.lit(r as usize, c as usize) {
                    continue;
                }
                let fr = logo_r0 + r;
                let fc = logo_c0 + c;
                if fr < 0 || fc < 0 {
                    continue;
                }
                frame.set(fr as u16, fc as u16, Cell {
                    ch: logo.glyph_at(r as usize, c as usize),
                    fg: ctx.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }
    }
}
