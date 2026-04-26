//! tetris — each lit cell is a falling block. Blocks per column
//! land bottom-up: the lowest-target block in a column falls first
//! and locks to its target row, then the next-lowest falls and
//! stacks above it, etc. Visual gravity, no overlaps.

use crate::easing;
use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::collections::HashMap;
use std::time::Duration;

const DURATION_MS: u64 = 6_000;

struct Block {
    end_row: u16,
    end_col: u16,
    ch: char,
    color: Color,
    /// 0..1 progress when this block starts falling.
    fall_start: f32,
    /// 0..1 progress when this block lands at end_row.
    fall_end: f32,
}

pub struct Tetris {
    blocks: Vec<Block>,
    elapsed: Duration,
    rows: u16,
}

impl Tetris {
    pub fn new() -> Self {
        Self { blocks: Vec::new(), elapsed: Duration::ZERO, rows: 0 }
    }
}

impl Default for Tetris {
    fn default() -> Self { Self::new() }
}

impl Effect for Tetris {
    fn name(&self) -> &'static str { "tetris" }
    fn title(&self) -> &'static str { "Tetris" }
    fn description(&self) -> &'static str {
        "Each lit cell is a falling block; blocks per column land bottom-up and lock at their target rows."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.blocks.clear();
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();

        // Group cells by column, sorted by row descending (lowest
        // target row first = it lands first within its column).
        let mut by_col: HashMap<u16, Vec<(u16, char, Color)>> = HashMap::new();
        for (r, c, cell) in target.cells() {
            if cell.ch != ' ' {
                by_col.entry(c).or_default().push((r, cell.ch, cell.fg));
            }
        }

        // Find the deepest column to compute time budget.
        let max_stack = by_col.values().map(|v| v.len()).max().unwrap_or(1).max(1) as f32;
        // Reserve 75 % of duration for the staggered falls; each
        // block's fall takes 1 / max_stack of that.
        let stagger_window = 0.85;
        let fall_dt = stagger_window / max_stack;

        for (col, mut cells) in by_col.into_iter() {
            cells.sort_by(|a, b| b.0.cmp(&a.0)); // descending row
            for (idx, (row, ch, color)) in cells.into_iter().enumerate() {
                let fall_start = (idx as f32 / max_stack) * stagger_window;
                let fall_end = (fall_start + fall_dt).min(1.0);
                self.blocks.push(Block {
                    end_row: row,
                    end_col: col,
                    ch,
                    color,
                    fall_start,
                    fall_end,
                });
            }
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();
        let mut all_landed = true;

        for b in &self.blocks {
            if progress >= b.fall_end {
                // Locked.
                frame.set(b.end_row, b.end_col, Cell {
                    ch: b.ch,
                    fg: b.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else if progress >= b.fall_start {
                all_landed = false;
                // Fall from row -1 to end_row over (fall_start..fall_end).
                let local = (progress - b.fall_start) / (b.fall_end - b.fall_start);
                let eased = easing::ease_in_quad(local);
                let r_f = easing::lerp(-1.0, b.end_row as f32, eased);
                let r = r_f.round() as i32;
                if r < 0 || r >= self.rows as i32 {
                    continue;
                }
                // Falling block — slightly dimmer than landed.
                let brightness = 0.6 + 0.4 * eased;
                let fg = scale(b.color, brightness);
                frame.set(r as u16, b.end_col, Cell {
                    ch: b.ch,
                    fg,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else {
                all_landed = false;
                // Hasn't started — empty.
            }
        }

        all_landed && progress >= 1.0
    }

    fn reset(&mut self) {
        self.blocks.clear();
        self.elapsed = Duration::ZERO;
    }
}

fn scale(c: Color, k: f32) -> Color {
    let k = k.clamp(0.0, 1.0);
    Color::rgb(
        (c.r as f32 * k) as u8,
        (c.g as f32 * k) as u8,
        (c.b as f32 * k) as u8,
    )
}
