//! Matrix Rain — falling columns of glyphs with a bright head and
//! a fading tail. Glyph set is configurable: katakana (default),
//! ascii, hex, or "brand" (SHEDOS letters).

use crate::opts::{validate_enum, validate_f32_range, validate_u32_range, OptType, OptVal, OptionDoc, OptionSchema};
use crate::{Ctx, Style};
use rand::Rng;
use shedos_screensaver_core::{Cell, CellAttrs, Color, Frame};

static SCHEMA: OptionSchema = OptionSchema {
    options: &[
        OptionDoc {
            key: "density",
            ty: OptType::Float,
            default: OptVal::Float(0.5),
            desc: "probability per column per frame of starting a new trail (0.0..=1.0)",
            validate: validate_density,
        },
        OptionDoc {
            key: "trail_length",
            ty: OptType::UInt,
            default: OptVal::UInt(20),
            desc: "trail length in cells (1..=100)",
            validate: validate_trail_length,
        },
        OptionDoc {
            key: "glyphs",
            ty: OptType::Enum,
            default: OptVal::String(String::new()), // overridden below by setup-time default
            desc: "katakana | ascii | hex | brand",
            validate: validate_glyphs,
        },
    ],
};

fn validate_density(v: &OptVal) -> Result<(), String> {
    validate_f32_range(0.0, 1.0)(v)
}
fn validate_trail_length(v: &OptVal) -> Result<(), String> {
    validate_u32_range(1, 100)(v)
}
const GLYPH_SETS: &[&str] = &["katakana", "ascii", "hex", "brand"];
fn validate_glyphs(v: &OptVal) -> Result<(), String> {
    validate_enum(GLYPH_SETS)(v)
}

const KATAKANA: &str = "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン";
const ASCII_GL: &str = "abcdefghijklmnopqrstuvwxyz0123456789";
const HEX_GL: &str = "0123456789abcdef";
const BRAND_GL: &str = "SHEDOSshedos";

pub struct Matrix {
    /// One trail per column. None = no trail right now.
    trails: Vec<Option<Trail>>,
    glyph_chars: Vec<char>,
}

#[derive(Debug, Clone, Copy)]
struct Trail {
    /// Vertical position of the bright head (float for sub-cell speed).
    head: f32,
    /// Cells per second for this trail.
    speed: f32,
}

impl Matrix {
    pub fn new() -> Self {
        Self {
            trails: Vec::new(),
            glyph_chars: KATAKANA.chars().collect(),
        }
    }
}

impl Default for Matrix {
    fn default() -> Self {
        Self::new()
    }
}

impl Style for Matrix {
    fn name(&self) -> &'static str { "matrix" }
    fn title(&self) -> &'static str { "Matrix Rain" }
    fn default_color(&self) -> Color { Color::rgb(0x88, 0xc9, 0x70) }
    fn option_schema(&self) -> &'static OptionSchema { &SCHEMA }
    fn wants_audio(&self) -> bool { true }
    fn wallpaper_alpha(&self) -> f32 { 0.85 }

    fn draw(&mut self, frame: &mut Frame, ctx: &mut Ctx<'_>) {
        let cols = frame.cols() as usize;
        let rows = frame.rows() as i32;

        if self.trails.len() != cols {
            self.trails.clear();
            self.trails.resize(cols, None);
        }

        let glyph_set = ctx.opts.get_str("glyphs").unwrap_or("katakana");
        let chars: &[char] = match glyph_set {
            "ascii" => {
                self.glyph_chars = ASCII_GL.chars().collect();
                &self.glyph_chars
            }
            "hex" => {
                self.glyph_chars = HEX_GL.chars().collect();
                &self.glyph_chars
            }
            "brand" => {
                self.glyph_chars = BRAND_GL.chars().collect();
                &self.glyph_chars
            }
            _ => {
                if !std::ptr::eq::<[char]>(&self.glyph_chars[..], KATAKANA.chars().collect::<Vec<_>>().as_slice()) {
                    // already populated; only swap if needed
                }
                if self.glyph_chars.is_empty() || self.glyph_chars.first() != Some(&'ア') {
                    self.glyph_chars = KATAKANA.chars().collect();
                }
                &self.glyph_chars
            }
        };

        let density = ctx.opts.get_f32("density").unwrap_or(0.5);
        let trail_len = ctx.opts.get_u32("trail_length").unwrap_or(20) as i32;
        let dt = ctx.dt.as_secs_f32();

        // Spawn new trails for empty columns.
        for col in 0..cols {
            if self.trails[col].is_none() && ctx.rng.gen::<f32>() < density * dt * 4.0 {
                let speed = ctx.rng.gen_range(8.0..30.0);
                self.trails[col] = Some(Trail { head: 0.0, speed });
            }
        }

        // Advance + render trails.
        for col in 0..cols {
            if let Some(trail) = self.trails[col].as_mut() {
                trail.head += trail.speed * dt;
                let head_row = trail.head as i32;
                if head_row - trail_len > rows {
                    self.trails[col] = None;
                    continue;
                }
                for k in 0..trail_len {
                    let r = head_row - k;
                    if !(0..rows).contains(&r) {
                        continue;
                    }
                    let intensity = 1.0 - (k as f32 / trail_len as f32);
                    let glyph = chars[ctx.rng.gen_range(0..chars.len())];
                    let (fg, attrs) = if k == 0 {
                        // Head is brightest white-ish.
                        (Color::rgb(0xff, 0xff, 0xff), CellAttrs::BOLD)
                    } else {
                        let base = ctx.color;
                        let r = (base.r as f32 * intensity) as u8;
                        let g = (base.g as f32 * intensity) as u8;
                        let b = (base.b as f32 * intensity) as u8;
                        (Color::rgb(r, g, b), CellAttrs::NONE)
                    };
                    frame.set(r as u16, col as u16, Cell {
                        ch: glyph,
                        fg,
                        bg: Color::BASE,
                        attrs,
                    });
                }
            }
        }
    }
}
