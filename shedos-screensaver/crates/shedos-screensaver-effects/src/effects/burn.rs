//! burn — fire rises from the bottom of the canvas, consuming
//! blank space; in its wake the target cells appear, glowing first
//! with ember colors, then settling to the final color. Inspired by
//! tte's "burn" but recast as a reveal rather than a consumption.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
const FLAME_GLYPHS: &[char] = &['▲', '▴', '▾', '▿', '⫷', '⫸', '※', '⁕', '✦'];

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}

pub struct Burn {
    cells: Vec<TargetCell>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
}

impl Burn {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO, rows: 0, cols: 0 }
    }
}

impl Default for Burn {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Burn {
    fn name(&self) -> &'static str { "burn" }
    fn title(&self) -> &'static str { "Burn" }
    fn description(&self) -> &'static str {
        "A flame front rises from the bottom; cells appear in its wake glowing through ember to the final color."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();
        self.cols = target.cols();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.cells.push(TargetCell { row: r, col: c, ch: cell.ch, color: cell.fg });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        // Flame front rises from the bottom: at progress=0 front sits
        // at the bottom row; at progress=1 it's well past the top so
        // every cell is revealed.
        let rows = self.rows as f32;
        let front = (rows - 1.0) - progress * (rows + 4.0);
        let tick = (self.elapsed.as_millis() / 70) as u64;

        frame.clear();

        // Render flame glyphs in a band around the front.
        for r in 0..self.rows {
            let dist = (r as f32) - front;
            if dist >= 0.0 && dist <= 4.0 {
                for c in 0..self.cols {
                    // Density ramps with column hash so the flame
                    // looks textured rather than a solid bar.
                    let h = (tick.wrapping_mul(2654435761).wrapping_add(c as u64).wrapping_add(r as u64)) as usize;
                    if h % 3 == 0 {
                        let g_idx = (h / 3) % FLAME_GLYPHS.len();
                        let glyph = FLAME_GLYPHS[g_idx];
                        // Color ramp: white-hot at the front, orange behind.
                        let intensity = 1.0 - (dist / 4.0);
                        let r_ch = lerp_u8(0xff, 0xfa, 1.0 - intensity);
                        let g_ch = lerp_u8(0xee, 0x68, 1.0 - intensity);
                        let b_ch = lerp_u8(0x77, 0x10, 1.0 - intensity);
                        frame.set(r, c, Cell {
                            ch: glyph,
                            fg: Color::rgb(r_ch, g_ch, b_ch),
                            bg: Color::BASE,
                            attrs: Default::default(),
                        });
                    }
                }
            }
        }

        // Render target cells that the front has already passed (cells
        // above the rising flame). The front decreases over time, so a
        // cell at row r is revealed once `front < r`.
        for c in &self.cells {
            if (c.row as f32) <= front + 1.0 {
                continue; // front hasn't reached this row yet
            }
            // Reveal time was when front passed c.row; for cells just
            // revealed, blend toward ember orange; deeper into the
            // burn they fade to c.color. In the last 5 % force the
            // settle to 1.0 so the resolved SHEDOS is solid target
            // color across all rows (no leftover orange tint on the
            // top rows that the ember-distance window never quite
            // reached).
            let cells_since_reveal = c.row as f32 - front;
            let settle = if progress >= 0.95 {
                1.0
            } else {
                (cells_since_reveal / 6.0).clamp(0.0, 1.0)
            };
            let r_ch = lerp_u8(0xfa, c.color.r, settle);
            let g_ch = lerp_u8(0x68, c.color.g, settle);
            let b_ch = lerp_u8(0x10, c.color.b, settle);
            frame.set(c.row, c.col, Cell {
                ch: c.ch,
                fg: Color::rgb(r_ch, g_ch, b_ch),
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (a as f32 * (1.0 - t) + b as f32 * t) as u8
}
