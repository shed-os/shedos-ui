//! firework — three burst points emit colored sparks that drift
//! along parabolic arcs to SHEDOS cells. Each burst fires at its
//! own time; sparks land at full target color.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::seq::SliceRandom;
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
const TRAVEL_NORM: f32 = 0.30;
const N_BURSTS: usize = 3;
/// Per-spark apex bump multiplier sampled from this range.
const APEX_BUMP: f32 = 4.0;
const SPARK_GLYPHS: &[char] = &['*', '•', '✦', '✧', '·'];

/// Three burst tints — Catppuccin red, peach, sky.
const BURST_TINTS: [Color; N_BURSTS] = [
    Color::rgb(0xf3, 0x8b, 0xa8),
    Color::rgb(0xfa, 0xb3, 0x87),
    Color::rgb(0x89, 0xdc, 0xeb),
];

#[derive(Clone, Copy)]
struct Spark {
    target_row: u16,
    target_col: u16,
    target_ch: char,
    target_color: Color,
    burst_x: f32,
    burst_y: f32,
    burst_color: Color,
    spawn_t: f32,
    arrival_t: f32,
    glyph: char,
    apex_mult: f32,
}

pub struct Firework {
    sparks: Vec<Spark>,
    canvas_rows: u16,
    canvas_cols: u16,
    elapsed: Duration,
}

impl Firework {
    pub fn new() -> Self {
        Self { sparks: Vec::new(), canvas_rows: 0, canvas_cols: 0, elapsed: Duration::ZERO }
    }
}

impl Default for Firework {
    fn default() -> Self { Self::new() }
}

impl Effect for Firework {
    fn name(&self) -> &'static str { "firework" }
    fn title(&self) -> &'static str { "Firework" }
    fn description(&self) -> &'static str {
        "Three burst points emit colored sparks that drift along parabolic arcs to SHEDOS cells."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.sparks.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();

        // Three burst positions, distributed across the canvas.
        let cols = target.cols() as f32;
        let rows = target.rows() as f32;
        let bursts = [
            (cols * 0.25, rows * 0.30),
            (cols * 0.75, rows * 0.30),
            (cols * 0.50, rows * 0.75),
        ];

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

        // Fire each burst at a different normalized time. Each burst's
        // sparks all spawn within a tight window of their burst's
        // trigger; they then take TRAVEL_NORM to arrive.
        let burst_times: [f32; N_BURSTS] = [0.00, 0.25, 0.50];
        let burst_window: f32 = 0.04;
        let group_size = lit.len().div_ceil(N_BURSTS);

        for (i, (r, c, ch, color)) in lit.into_iter().enumerate() {
            let burst_idx = (i / group_size).min(N_BURSTS - 1);
            let (bx, by) = bursts[burst_idx];
            let spawn_t = burst_times[burst_idx]
                + ctx.rng.gen_range(0.0..burst_window);
            let arrival_t = (spawn_t + TRAVEL_NORM).min(0.99);
            let glyph = SPARK_GLYPHS[ctx.rng.gen_range(0..SPARK_GLYPHS.len())];
            let apex_mult = ctx.rng.gen_range(0.5..1.5);
            self.sparks.push(Spark {
                target_row: r,
                target_col: c,
                target_ch: ch,
                target_color: color,
                burst_x: bx,
                burst_y: by,
                burst_color: BURST_TINTS[burst_idx],
                spawn_t,
                arrival_t,
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

        for s in &self.sparks {
            if progress < s.spawn_t {
                continue;
            }
            if progress >= s.arrival_t {
                frame.set(s.target_row, s.target_col, Cell {
                    ch: s.target_ch,
                    fg: s.target_color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
                continue;
            }
            let t = (progress - s.spawn_t) / (s.arrival_t - s.spawn_t);
            let lin_x = s.burst_x + (s.target_col as f32 - s.burst_x) * t;
            let lin_y = s.burst_y + (s.target_row as f32 - s.burst_y) * t;
            let bump = APEX_BUMP * s.apex_mult * 4.0 * t * (1.0 - t);
            // Bump pushes the spark up (smaller y).
            let y = lin_y - bump;
            let r = y.round() as i32;
            let c = lin_x.round() as i32;
            if r < 0 || r >= self.canvas_rows as i32 || c < 0 || c >= self.canvas_cols as i32 {
                continue;
            }
            // Color fades from burst tint → target color along travel.
            let fg = lerp_color(s.burst_color, s.target_color, t);
            frame.set(r as u16, c as u16, Cell {
                ch: s.glyph,
                fg,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.sparks.clear();
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
