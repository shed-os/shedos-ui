//! Bouncing SHEDOS — DVD-screensaver bounce of the logo.
//! Color cycles through the Catppuccin palette on each wall hit.

use crate::opts::{validate_any, validate_f32_range, OptType, OptVal, OptionDoc, OptionSchema};
use crate::{Ctx, Style};
use shedos_screensaver_core::{Cell, Color, Frame};

static SCHEMA: OptionSchema = OptionSchema {
    options: &[
        OptionDoc {
            key: "speed",
            ty: OptType::Float,
            default: OptVal::Float(1.0),
            desc: "multiplier on bounce velocity (0.1..=10.0)",
            validate: validate_speed,
        },
        OptionDoc {
            key: "color_cycle",
            ty: OptType::Bool,
            default: OptVal::Bool(true),
            desc: "shift color on each wall hit",
            validate: validate_any,
        },
    ],
};
fn validate_speed(v: &OptVal) -> Result<(), String> {
    validate_f32_range(0.1, 10.0)(v)
}

const CYCLE: &[Color] = &[
    Color::rgb(0x89, 0xb4, 0xfa), // blue
    Color::rgb(0xcb, 0xa6, 0xf7), // mauve
    Color::rgb(0xfa, 0xb3, 0x87), // peach
    Color::rgb(0xa6, 0xe3, 0xa1), // green
    Color::rgb(0x94, 0xe2, 0xd5), // teal
    Color::rgb(0xf3, 0x8b, 0xa8), // red
    Color::rgb(0xf9, 0xe2, 0xaf), // yellow
    Color::rgb(0x89, 0xdc, 0xeb), // sky
];

pub struct LogoBounce {
    /// Float position so we can move at sub-cell speeds.
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    color_idx: usize,
    initialized: bool,
}

impl LogoBounce {
    pub fn new() -> Self {
        Self { x: 0.0, y: 0.0, vx: 12.0, vy: 6.0, color_idx: 0, initialized: false }
    }
}

impl Default for LogoBounce {
    fn default() -> Self {
        Self::new()
    }
}

impl Style for LogoBounce {
    fn name(&self) -> &'static str { "logo-bounce" }
    fn title(&self) -> &'static str { "Bouncing SHEDOS" }
    fn default_color(&self) -> Color { CYCLE[0] }
    fn option_schema(&self) -> &'static OptionSchema { &SCHEMA }

    fn draw(&mut self, frame: &mut Frame, ctx: &mut Ctx<'_>) {
        let speed = ctx.opts.get_f32("speed").unwrap_or(1.0);
        let color_cycle = ctx.opts.get_bool("color_cycle").unwrap_or(true);
        let dt = ctx.dt.as_secs_f32();
        let logo = ctx.logo;

        if !self.initialized {
            // Center the logo at start.
            self.x = (frame.cols().saturating_sub(logo.cols) as f32) * 0.5;
            self.y = (frame.rows().saturating_sub(logo.rows) as f32) * 0.5;
            self.initialized = true;
        }

        // Advance position.
        self.x += self.vx * speed * dt;
        self.y += self.vy * speed * dt;

        // Bounce off walls (right/bottom limited by logo extent).
        let max_x = (frame.cols().saturating_sub(logo.cols)) as f32;
        let max_y = (frame.rows().saturating_sub(logo.rows)) as f32;
        let mut bounced = false;
        if self.x <= 0.0 {
            self.x = 0.0;
            self.vx = self.vx.abs();
            bounced = true;
        } else if self.x >= max_x {
            self.x = max_x;
            self.vx = -self.vx.abs();
            bounced = true;
        }
        if self.y <= 0.0 {
            self.y = 0.0;
            self.vy = self.vy.abs();
            bounced = true;
        } else if self.y >= max_y {
            self.y = max_y;
            self.vy = -self.vy.abs();
            bounced = true;
        }
        if bounced && color_cycle {
            self.color_idx = (self.color_idx + 1) % CYCLE.len();
        }

        let color = if color_cycle { CYCLE[self.color_idx] } else { ctx.color };

        // Render logo at (self.y, self.x).
        let base_r = self.y as i32;
        let base_c = self.x as i32;
        for r in 0..logo.rows as i32 {
            for c in 0..logo.cols as i32 {
                if !logo.lit(r as usize, c as usize) {
                    continue;
                }
                let fr = base_r + r;
                let fc = base_c + c;
                if fr < 0 || fc < 0 {
                    continue;
                }
                let glyph = logo.glyph_at(r as usize, c as usize);
                frame.set(fr as u16, fc as u16, Cell {
                    ch: glyph,
                    fg: color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }
    }
}
