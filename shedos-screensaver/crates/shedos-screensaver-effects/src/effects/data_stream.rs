//! data-stream — each row streams 1s and 0s horizontally. As the
//! stream passes a target cell, the cell locks to its target glyph.
//! Rows with all target cells locked stop streaming.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 5_500;
const STREAM_GREEN: Color = Color::rgb(0x33, 0xdd, 0x55);

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// 0..1 progress at which this cell locks.
    lock_at: f32,
}

pub struct DataStream {
    cells: Vec<TargetCell>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
}

impl DataStream {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO, rows: 0, cols: 0 }
    }
}

impl Default for DataStream {
    fn default() -> Self { Self::new() }
}

impl Effect for DataStream {
    fn name(&self) -> &'static str { "data-stream" }
    fn title(&self) -> &'static str { "Data Stream" }
    fn description(&self) -> &'static str {
        "Streams of 1s and 0s flow across each row; cells lock to the target glyph as the stream passes them."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();
        self.cols = target.cols();
        let mut lit: Vec<(u16, u16, char, Color)> = target
            .cells()
            .filter_map(|(r, c, cell)| {
                if cell.ch != ' ' { Some((r, c, cell.ch, cell.fg)) } else { None }
            })
            .collect();
        // Lock order: left-to-right, then top-to-bottom (column-major)
        // so the stream "captures" cells progressively along each row.
        lit.sort_by_key(|&(r, c, _, _)| (c as u32, r as u32));
        let total = lit.len().max(1) as f32;
        for (i, (r, c, ch, color)) in lit.into_iter().enumerate() {
            self.cells.push(TargetCell {
                row: r,
                col: c,
                ch,
                color,
                lock_at: i as f32 / total,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let tick = (self.elapsed.as_millis() / 60) as u64;

        frame.clear();

        // Render 1s/0s only for rows with unlocked cells. Skipping
        // fully-locked rows avoids visual noise late in the run.
        for r in 0..self.rows {
            let row_active = self
                .cells
                .iter()
                .any(|c| c.row == r && progress < c.lock_at);
            if !row_active {
                continue;
            }
            for c in 0..self.cols {
                // Per-cell digit decided by hash(r, c, tick).
                let h = tick
                    .wrapping_mul(2_654_435_761)
                    .wrapping_add(r as u64 * 17 + c as u64 * 31);
                let bit = if h % 2 == 0 { '0' } else { '1' };
                // Stream brightness varies by column for a flowing
                // feel. Cells near "stream front" are brighter.
                let phase = (c as f32 + (tick as f32 * 0.3)).sin() * 0.5 + 0.5;
                let brightness = 0.3 + 0.7 * phase;
                let fg = scale(STREAM_GREEN, brightness);
                frame.set(r, c, Cell {
                    ch: bit,
                    fg,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        // Locked cells overwrite the stream.
        for c in &self.cells {
            if progress >= c.lock_at {
                frame.set(c.row, c.col, Cell {
                    ch: c.ch,
                    fg: c.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.cells.clear();
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
