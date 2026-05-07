//! ember-rise — glowing embers float upward from below with
//! flickering brightness; settle into the SHEDOS shape from
//! bottom up.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
const TRAVEL_NORM: f32 = 0.32;
const SWAY_AMP: f32 = 1.0;
const EMBER_GLYPHS: &[char] = &['•', '*', '°', '·'];
/// Bright ember start color; settles toward target as the ember rises.
const EMBER_HOT: Color = Color::rgb(0xfa, 0xb3, 0x87);

#[derive(Clone, Copy)]
struct Ember {
    target_row: u16,
    target_col: u16,
    target_ch: char,
    target_color: Color,
    spawn_x: f32,
    spawn_t: f32,
    arrival_t: f32,
    phase: f32,
    glyph: char,
}

pub struct EmberRise {
    embers: Vec<Ember>,
    canvas_rows: u16,
    canvas_cols: u16,
    elapsed: Duration,
}

impl EmberRise {
    pub fn new() -> Self {
        Self { embers: Vec::new(), canvas_rows: 0, canvas_cols: 0, elapsed: Duration::ZERO }
    }
}

impl Default for EmberRise {
    fn default() -> Self { Self::new() }
}

impl Effect for EmberRise {
    fn name(&self) -> &'static str { "ember-rise" }
    fn title(&self) -> &'static str { "Ember Rise" }
    fn description(&self) -> &'static str {
        "Glowing embers float upward from below with flickering brightness; settle into the SHEDOS shape from bottom up."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.embers.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();

        // Collect lit cells, sort by row descending so bottom rows
        // spawn first → bottom-up settling order.
        let mut lit: Vec<(u16, u16, char, Color)> = Vec::new();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            lit.push((r, c, cell.ch, cell.fg));
        }
        lit.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        if lit.is_empty() {
            return;
        }
        let spawn_window = (1.0 - TRAVEL_NORM).max(0.05);
        let spacing = spawn_window / (lit.len() as f32);
        for (i, (r, c, ch, color)) in lit.into_iter().enumerate() {
            let spawn_t = (i as f32) * spacing;
            let spawn_x = (c as f32) + ctx.rng.gen_range(-3.0..3.0);
            let phase = ctx.rng.gen_range(0.0..std::f32::consts::TAU);
            let glyph = EMBER_GLYPHS[ctx.rng.gen_range(0..EMBER_GLYPHS.len())];
            self.embers.push(Ember {
                target_row: r,
                target_col: c,
                target_ch: ch,
                target_color: color,
                spawn_x,
                spawn_t,
                arrival_t: spawn_t + TRAVEL_NORM,
                phase,
                glyph,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        for e in &self.embers {
            if progress < e.spawn_t {
                continue;
            }
            if progress >= e.arrival_t {
                frame.set(e.target_row, e.target_col, Cell {
                    ch: e.target_ch,
                    fg: e.target_color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
                continue;
            }
            // In transit. Rises from canvas bottom up to target row,
            // with sin-wave horizontal sway and a flickering brightness.
            let t = (progress - e.spawn_t) / (e.arrival_t - e.spawn_t);
            let start_y = (self.canvas_rows as f32) + 1.0;
            let y = start_y + (e.target_row as f32 - start_y) * t;
            let base_x = e.spawn_x + (e.target_col as f32 - e.spawn_x) * t;
            let sway = SWAY_AMP * (e.phase + t * 7.0).sin() * (1.0 - t);
            let x = base_x + sway;
            let r = y.round() as i32;
            let c = x.round() as i32;
            if r < 0 || r >= self.canvas_rows as i32 || c < 0 || c >= self.canvas_cols as i32 {
                continue;
            }
            // Flicker: brightness multiplied by a high-frequency wave.
            let flick = 0.65 + 0.35 * (e.phase + progress * 40.0).sin().abs();
            let fg = lerp_color(EMBER_HOT, e.target_color, t);
            let dimmed = scale_color(fg, flick);
            frame.set(r as u16, c as u16, Cell {
                ch: e.glyph,
                fg: dimmed,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.embers.clear();
        self.elapsed = Duration::ZERO;
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::rgb(
        ((a.r as f32) * (1.0 - t) + (b.r as f32) * t) as u8,
        ((a.g as f32) * (1.0 - t) + (b.g as f32) * t) as u8,
        ((a.b as f32) * (1.0 - t) + (b.b as f32) * t) as u8,
    )
}

fn scale_color(c: Color, scale: f32) -> Color {
    let s = scale.clamp(0.0, 1.0);
    Color::rgb(
        ((c.r as f32) * s) as u8,
        ((c.g as f32) * s) as u8,
        ((c.b as f32) * s) as u8,
    )
}
