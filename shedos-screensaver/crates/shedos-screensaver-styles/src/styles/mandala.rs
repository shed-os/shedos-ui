//! SHEDOS Mandala — N-fold rotational symmetry kaleidoscope.
//! A small "kernel" pattern (drawn from the SHEDOS logo glyphs) is
//! rotated and replicated; rotation+growth animate over time.

use crate::opts::{validate_f32_range, validate_u32_range, OptType, OptVal, OptionDoc, OptionSchema};
use crate::{Ctx, Style};
use shedos_screensaver_core::{Cell, Color, Frame};

static SCHEMA: OptionSchema = OptionSchema {
    options: &[
        OptionDoc {
            key: "symmetry",
            ty: OptType::UInt,
            default: OptVal::UInt(8),
            desc: "N-fold rotational symmetry (2..=16)",
            validate: validate_symmetry,
        },
        OptionDoc {
            key: "growth",
            ty: OptType::Float,
            default: OptVal::Float(1.0),
            desc: "growth speed of kernel (0.1..=10.0)",
            validate: validate_growth,
        },
    ],
};
fn validate_symmetry(v: &OptVal) -> Result<(), String> {
    validate_u32_range(2, 16)(v)
}
fn validate_growth(v: &OptVal) -> Result<(), String> {
    validate_f32_range(0.1, 10.0)(v)
}

const KERNEL_GLYPHS: &[char] = &['▘', '▝', '▖', '▗', '▚', '▞', '▙', '▟', '◆', '●', '◇', '○'];

pub struct Mandala;

impl Mandala {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Mandala {
    fn default() -> Self {
        Self::new()
    }
}

impl Style for Mandala {
    fn name(&self) -> &'static str { "mandala" }
    fn title(&self) -> &'static str { "SHEDOS Mandala" }
    fn default_color(&self) -> Color { Color::rgb(0xfa, 0xb3, 0x87) }
    fn option_schema(&self) -> &'static OptionSchema { &SCHEMA }
    fn wallpaper_alpha(&self) -> f32 { 0.85 }

    fn draw(&mut self, frame: &mut Frame, ctx: &mut Ctx<'_>) {
        let n = ctx.opts.get_u32("symmetry").unwrap_or(8) as usize;
        let growth = ctx.opts.get_f32("growth").unwrap_or(1.0);
        let t = ctx.t.as_secs_f32() * growth;
        let cx = frame.cols() as f32 * 0.5;
        let cy = frame.rows() as f32 * 0.5;
        let aspect = 0.5;
        let max_radius = (cx.min(cy / aspect)) * 0.95;

        // Generate a few "seed" points in a wedge of angle 2π/N; replicate
        // around the full circle by rotation.
        let seeds: usize = 14;
        let wedge = (2.0 * std::f32::consts::PI) / n as f32;
        for i in 0..seeds {
            let f = (i as f32 + 1.0) / (seeds as f32 + 1.0);
            // Pulsing radial position.
            let radius = max_radius * (f + 0.15 * (t + i as f32 * 0.3).sin());
            let angle_in_wedge = wedge * (0.15 + 0.7 * (t * 0.4 + i as f32 * 0.5).cos().abs());
            let glyph = KERNEL_GLYPHS[i % KERNEL_GLYPHS.len()];
            let intensity = 0.5 + 0.5 * (t + i as f32).sin();
            let base = ctx.color;
            let fg = Color::rgb(
                (base.r as f32 * intensity) as u8,
                (base.g as f32 * intensity) as u8,
                (base.b as f32 * intensity) as u8,
            );
            for k in 0..n {
                let a = angle_in_wedge + wedge * k as f32;
                let dx = a.cos() * radius;
                let dy = a.sin() * radius * aspect;
                let fr = (cy + dy) as i32;
                let fc = (cx + dx) as i32;
                if fr < 0 || fc < 0 || fr >= frame.rows() as i32 || fc >= frame.cols() as i32 {
                    continue;
                }
                frame.set(fr as u16, fc as u16, Cell {
                    ch: glyph,
                    fg,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }
    }
}
