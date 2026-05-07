//! fault-line — a horizontal seismic crack tears the canvas open
//! at the middle row; the halves separate, revealing SHEDOS in
//! the widening gap.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 3_500;
/// Per-column jag amplitude (cells).
const MAX_JAG: i32 = 2;
/// Pre-quake fraction of the duration — full ground, no movement.
const STILL_FRAC: f32 = 0.15;
const GROUND_GLYPHS: &[char] = &['▒', '▓', '░'];

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}

pub struct FaultLine {
    cells: Vec<TargetCell>,
    canvas_rows: u16,
    canvas_cols: u16,
    /// Per-column jag offset, [-MAX_JAG, MAX_JAG].
    jag_per_col: Vec<i32>,
    /// Per-canvas-cell ground glyph (stable through the animation).
    ground_per_cell: Vec<char>,
    elapsed: Duration,
}

impl FaultLine {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            canvas_rows: 0,
            canvas_cols: 0,
            jag_per_col: Vec::new(),
            ground_per_cell: Vec::new(),
            elapsed: Duration::ZERO,
        }
    }
}

impl Default for FaultLine {
    fn default() -> Self { Self::new() }
}

impl Effect for FaultLine {
    fn name(&self) -> &'static str { "fault-line" }
    fn title(&self) -> &'static str { "Fault Line" }
    fn description(&self) -> &'static str {
        "A horizontal seismic crack tears the canvas open at the middle row; the halves separate, revealing SHEDOS in the widening gap."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();

        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.cells.push(TargetCell {
                row: r,
                col: c,
                ch: cell.ch,
                color: cell.fg,
            });
        }

        self.jag_per_col.clear();
        self.jag_per_col.reserve(target.cols() as usize);
        for _ in 0..target.cols() {
            self.jag_per_col.push(ctx.rng.gen_range(-MAX_JAG..=MAX_JAG));
        }

        let n = (target.rows() as usize) * (target.cols() as usize);
        self.ground_per_cell.clear();
        self.ground_per_cell.reserve(n);
        for _ in 0..n {
            self.ground_per_cell
                .push(GROUND_GLYPHS[ctx.rng.gen_range(0..GROUND_GLYPHS.len())]);
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        let mid_row = self.canvas_rows as i32 / 2;
        // gap_radius caps at rows/2 + MAX_JAG + 1 so by progress=1.0 the
        // entire canvas (worst-case col with maximum negative jag) is in
        // the gap.
        let max_gap = (self.canvas_rows as i32 / 2) + MAX_JAG + 1;
        let gap_progress = if progress < STILL_FRAC {
            0.0
        } else {
            ((progress - STILL_FRAC) / (1.0 - STILL_FRAC)).clamp(0.0, 1.0)
        };
        let gap_radius = (gap_progress * max_gap as f32).ceil() as i32;

        let cols = self.canvas_cols as usize;
        for r in 0..self.canvas_rows {
            for c in 0..self.canvas_cols {
                let jag = self.jag_per_col[c as usize];
                let r_i = r as i32;
                let gap_top = mid_row - gap_radius - jag;
                let gap_bottom = mid_row + gap_radius + jag;
                if r_i < gap_top || r_i > gap_bottom {
                    let idx = (r as usize) * cols + (c as usize);
                    let glyph = self.ground_per_cell[idx];
                    frame.set(r, c, Cell {
                        ch: glyph,
                        fg: Color::rgb(0x4a, 0x4a, 0x60),
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
            }
        }

        // Overlay target cells inside the gap.
        for tc in &self.cells {
            let jag = self.jag_per_col[tc.col as usize];
            let r_i = tc.row as i32;
            let gap_top = mid_row - gap_radius - jag;
            let gap_bottom = mid_row + gap_radius + jag;
            if r_i >= gap_top && r_i <= gap_bottom {
                frame.set(tc.row, tc.col, Cell {
                    ch: tc.ch,
                    fg: tc.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.cells.clear();
        self.jag_per_col.clear();
        self.ground_per_cell.clear();
        self.elapsed = Duration::ZERO;
    }
}
