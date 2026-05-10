//! snowstorm — snowflakes drift down with gentle horizontal sway.
//! Each flake is assigned a SHEDOS target cell and "sticks" when it
//! arrives, revealing the target glyph. By the end of the duration
//! every target cell has had a flake settle on it.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::seq::SliceRandom;
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
/// Travel time from spawn (above the canvas) to target, normalized
/// to the total duration.
const TRAVEL_NORM: f32 = 0.30;
/// Horizontal sway amplitude in cells.
const SWAY_AMP: f32 = 1.5;
const SNOW_GLYPHS: &[char] = &['❅', '❆', '❈', '·', '*'];
const SNOW_COLOR: Color = Color::rgb(0xd9, 0xe0, 0xee);

#[derive(Clone, Copy)]
struct Flake {
    target_row: u16,
    target_col: u16,
    target_ch: char,
    target_color: Color,
    spawn_x: f32,
    /// Time the flake spawns (normalized [0, 1]).
    spawn_t: f32,
    /// Time the flake arrives at its target cell.
    arrival_t: f32,
    /// Phase offset for the sin-wave horizontal sway.
    phase: f32,
    glyph: char,
}

pub struct Snowstorm {
    flakes: Vec<Flake>,
    canvas_rows: u16,
    canvas_cols: u16,
    elapsed: Duration,
}

impl Snowstorm {
    pub fn new() -> Self {
        Self { flakes: Vec::new(), canvas_rows: 0, canvas_cols: 0, elapsed: Duration::ZERO }
    }
}

impl Default for Snowstorm {
    fn default() -> Self { Self::new() }
}

impl Effect for Snowstorm {
    fn name(&self) -> &'static str { "snowstorm" }
    fn title(&self) -> &'static str { "Snowstorm" }
    fn description(&self) -> &'static str {
        "Snowflakes drift down with gentle horizontal sway, sticking to SHEDOS cells when they arrive."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.flakes.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();

        let mut lit: Vec<(u16, u16, char, Color)> = Vec::new();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            lit.push((r, c, cell.ch, cell.fg));
        }
        // Shuffle so the arrival order looks scattered, not row-by-row.
        lit.shuffle(ctx.rng);

        if lit.is_empty() {
            return;
        }
        // Spread spawn times evenly across (1 - TRAVEL_NORM) so that
        // every flake has TRAVEL_NORM left in the duration to arrive.
        let spawn_window = (1.0 - TRAVEL_NORM).max(0.05);
        let spacing = spawn_window / (lit.len() as f32);
        for (i, (r, c, ch, color)) in lit.into_iter().enumerate() {
            let spawn_t = (i as f32) * spacing;
            let spawn_x = ctx.rng.gen_range(0.0..(self.canvas_cols as f32));
            let phase = ctx.rng.gen_range(0.0..std::f32::consts::TAU);
            let glyph = SNOW_GLYPHS[ctx.rng.gen_range(0..SNOW_GLYPHS.len())];
            self.flakes.push(Flake {
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

        for f in &self.flakes {
            if progress < f.spawn_t {
                continue;
            }
            if progress >= f.arrival_t {
                // Settled: render the target cell.
                frame.set(f.target_row, f.target_col, Cell {
                    ch: f.target_ch,
                    fg: f.target_color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
                continue;
            }
            // In transit. Falls from above the canvas down to target row,
            // with a sin-wave horizontal sway around the path between
            // spawn_x and target_col.
            let t = (progress - f.spawn_t) / (f.arrival_t - f.spawn_t);
            let start_y = -2.0_f32;
            let y = start_y + (f.target_row as f32 - start_y) * t;
            let base_x = f.spawn_x + (f.target_col as f32 - f.spawn_x) * t;
            let sway = SWAY_AMP * (f.phase + t * 6.0).sin() * (1.0 - t);
            let x = base_x + sway;
            let r = y.round() as i32;
            let c = x.round() as i32;
            if r < 0 || r >= self.canvas_rows as i32 || c < 0 || c >= self.canvas_cols as i32 {
                continue;
            }
            frame.set(r as u16, c as u16, Cell {
                ch: f.glyph,
                fg: SNOW_COLOR,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.flakes.clear();
        self.elapsed = Duration::ZERO;
    }
}
