//! fountain — particles spray upward from a center point at the
//! bottom of the canvas, arc outward under gravity, and land at
//! SHEDOS cell positions.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::seq::SliceRandom;
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
const TRAVEL_NORM: f32 = 0.32;
/// Vertical apex bump above the linear path (cells).
const APEX_BUMP: f32 = 3.5;
/// Glyphs used for in-flight droplet rendering.
const DROP_GLYPHS: &[char] = &['•', '°', '·', '*'];
const SPRAY_COLOR: Color = Color::rgb(0x74, 0xc7, 0xec);

#[derive(Clone, Copy)]
struct Drop {
    target_row: u16,
    target_col: u16,
    target_ch: char,
    target_color: Color,
    spawn_t: f32,
    arrival_t: f32,
    glyph: char,
    /// Per-droplet apex bump multiplier (some go higher than others).
    apex_mult: f32,
}

pub struct Fountain {
    drops: Vec<Drop>,
    canvas_rows: u16,
    canvas_cols: u16,
    source_x: f32,
    source_y: f32,
    elapsed: Duration,
}

impl Fountain {
    pub fn new() -> Self {
        Self {
            drops: Vec::new(),
            canvas_rows: 0,
            canvas_cols: 0,
            source_x: 0.0,
            source_y: 0.0,
            elapsed: Duration::ZERO,
        }
    }
}

impl Default for Fountain {
    fn default() -> Self { Self::new() }
}

impl Effect for Fountain {
    fn name(&self) -> &'static str { "fountain" }
    fn title(&self) -> &'static str { "Fountain" }
    fn description(&self) -> &'static str {
        "Particles spray upward from the canvas-bottom center, arc outward under gravity, and land at SHEDOS cell positions."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.drops.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();
        self.source_x = (target.cols() as f32) * 0.5;
        self.source_y = (target.rows() as f32) - 1.0;

        let mut lit: Vec<(u16, u16, char, Color)> = Vec::new();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            lit.push((r, c, cell.ch, cell.fg));
        }
        lit.shuffle(ctx.rng);

        if lit.is_empty() {
            return;
        }
        let spawn_window = (1.0 - TRAVEL_NORM).max(0.05);
        let spacing = spawn_window / (lit.len() as f32);
        for (i, (r, c, ch, color)) in lit.into_iter().enumerate() {
            let spawn_t = (i as f32) * spacing;
            let glyph = DROP_GLYPHS[ctx.rng.gen_range(0..DROP_GLYPHS.len())];
            let apex_mult = ctx.rng.gen_range(0.6..1.4);
            self.drops.push(Drop {
                target_row: r,
                target_col: c,
                target_ch: ch,
                target_color: color,
                spawn_t,
                arrival_t: spawn_t + TRAVEL_NORM,
                glyph,
                apex_mult,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        for d in &self.drops {
            if progress < d.spawn_t {
                continue;
            }
            if progress >= d.arrival_t {
                frame.set(d.target_row, d.target_col, Cell {
                    ch: d.target_ch,
                    fg: d.target_color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
                continue;
            }
            let t = (progress - d.spawn_t) / (d.arrival_t - d.spawn_t);
            // Linear interpolation along (source → target), plus a
            // parabolic bump that peaks at t=0.5 above the linear path.
            let lin_x = self.source_x + (d.target_col as f32 - self.source_x) * t;
            let lin_y = self.source_y + (d.target_row as f32 - self.source_y) * t;
            let bump = APEX_BUMP * d.apex_mult * 4.0 * t * (1.0 - t);
            let y = lin_y - bump;
            let r = y.round() as i32;
            let c = lin_x.round() as i32;
            if r < 0 || r >= self.canvas_rows as i32 || c < 0 || c >= self.canvas_cols as i32 {
                continue;
            }
            frame.set(r as u16, c as u16, Cell {
                ch: d.glyph,
                fg: SPRAY_COLOR,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.drops.clear();
        self.elapsed = Duration::ZERO;
    }
}
