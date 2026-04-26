//! interlace — CRT scanline interlace reveal. Even rows fill in
//! left-to-right first (one full sweep), then odd rows fill in.
//! A bright cyan leading edge marks the current sweep position
//! within each row.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 4_500;
const EDGE_WIDTH: f32 = 2.0;
const EDGE_COLOR: Color = Color::rgb(0x55, 0xff, 0xff);

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}

pub struct Interlace {
    even_cells: Vec<TargetCell>,
    odd_cells: Vec<TargetCell>,
    elapsed: Duration,
    cols: u16,
}

impl Interlace {
    pub fn new() -> Self {
        Self {
            even_cells: Vec::new(),
            odd_cells: Vec::new(),
            elapsed: Duration::ZERO,
            cols: 0,
        }
    }
}

impl Default for Interlace {
    fn default() -> Self { Self::new() }
}

impl Effect for Interlace {
    fn name(&self) -> &'static str { "interlace" }
    fn title(&self) -> &'static str { "Interlace" }
    fn description(&self) -> &'static str {
        "CRT interlace reveal: even rows sweep left-to-right first, then odd rows fill in. Cyan leading edge."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.even_cells.clear();
        self.odd_cells.clear();
        self.elapsed = Duration::ZERO;
        self.cols = target.cols();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            let entry = TargetCell { row: r, col: c, ch: cell.ch, color: cell.fg };
            if r % 2 == 0 {
                self.even_cells.push(entry);
            } else {
                self.odd_cells.push(entry);
            }
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let even_progress = (progress / 0.5).min(1.0);
        let odd_progress = ((progress - 0.5) / 0.5).clamp(0.0, 1.0);

        // Sweep front in column space (with extra room so the front
        // travels past the right edge by progress=1.0, ensuring every
        // cell finishes settled).
        let max_col = (self.cols as f32) + EDGE_WIDTH + 1.0;
        let even_front = even_progress * max_col;
        let odd_front = odd_progress * max_col;

        frame.clear();
        for c in &self.even_cells {
            render_cell(frame, c, even_front);
        }
        for c in &self.odd_cells {
            render_cell(frame, c, odd_front);
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.even_cells.clear();
        self.odd_cells.clear();
        self.elapsed = Duration::ZERO;
    }
}

fn render_cell(frame: &mut Frame, c: &TargetCell, front: f32) {
    let col_f = c.col as f32;
    if col_f + EDGE_WIDTH < front {
        // Settled.
        frame.set(c.row, c.col, Cell {
            ch: c.ch,
            fg: c.color,
            bg: Color::BASE,
            attrs: Default::default(),
        });
    } else if col_f < front {
        // Edge — cyan leading.
        let edge_t = (front - col_f) / EDGE_WIDTH;
        let r_ch = lerp_u8(EDGE_COLOR.r, c.color.r, edge_t);
        let g_ch = lerp_u8(EDGE_COLOR.g, c.color.g, edge_t);
        let b_ch = lerp_u8(EDGE_COLOR.b, c.color.b, edge_t);
        frame.set(c.row, c.col, Cell {
            ch: c.ch,
            fg: Color::rgb(r_ch, g_ch, b_ch),
            bg: Color::BASE,
            attrs: Default::default(),
        });
    }
    // else not yet reached
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (a as f32 * (1.0 - t) + b as f32 * t) as u8
}
