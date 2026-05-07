//! iris-out — a circular iris opens from a small dot at the canvas
//! center, expanding to fill the canvas. Outside the iris a faint
//! vignette texture covers the canvas; inside, SHEDOS is revealed.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 4_000;
const ASPECT: f32 = 0.5;
/// Vignette glyphs sampled per canvas cell.
const VIGNETTE_GLYPHS: &[char] = &['░', '▒', '·'];
/// Dim color for vignette outside the iris.
const VIGNETTE_COLOR: Color = Color::rgb(0x45, 0x47, 0x5a);

#[derive(Clone, Copy)]
struct LitCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}

pub struct IrisOut {
    cells: Vec<LitCell>,
    canvas_rows: u16,
    canvas_cols: u16,
    /// Distance from center to the farthest cell — iris fully opens
    /// when iris_radius >= canvas_max_dist.
    canvas_max_dist: f32,
    /// Pre-picked vignette glyph per canvas cell (stable, no flicker).
    vignette_glyph: Vec<char>,
    elapsed: Duration,
}

impl IrisOut {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            canvas_rows: 0,
            canvas_cols: 0,
            canvas_max_dist: 1.0,
            vignette_glyph: Vec::new(),
            elapsed: Duration::ZERO,
        }
    }
}

impl Default for IrisOut {
    fn default() -> Self { Self::new() }
}

impl Effect for IrisOut {
    fn name(&self) -> &'static str { "iris-out" }
    fn title(&self) -> &'static str { "Iris Out" }
    fn description(&self) -> &'static str {
        "A circular iris opens from a small dot at canvas center, expanding to fill the canvas; a faint vignette texture covers the area outside the iris."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();

        let cx = target.cols() as f32 * 0.5;
        let cy = target.rows() as f32 * 0.5;
        // Distance to the farthest corner — guarantees iris covers
        // every canvas cell at progress=1.0.
        let dx = cx;
        let dy = cy / ASPECT;
        self.canvas_max_dist = (dx * dx + dy * dy).sqrt() + 1.0;

        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.cells.push(LitCell {
                row: r,
                col: c,
                ch: cell.ch,
                color: cell.fg,
            });
        }

        let n = (target.rows() as usize) * (target.cols() as usize);
        self.vignette_glyph.clear();
        self.vignette_glyph.reserve(n);
        for _ in 0..n {
            self.vignette_glyph
                .push(VIGNETTE_GLYPHS[ctx.rng.gen_range(0..VIGNETTE_GLYPHS.len())]);
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let iris_radius = progress * self.canvas_max_dist;

        frame.clear();

        let cx = self.canvas_cols as f32 * 0.5;
        let cy = self.canvas_rows as f32 * 0.5;
        let cols = self.canvas_cols as usize;

        // Render vignette outside the iris.
        for r in 0..self.canvas_rows {
            for c in 0..self.canvas_cols {
                let dx = c as f32 - cx;
                let dy = (r as f32 - cy) / ASPECT;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist >= iris_radius {
                    let idx = (r as usize) * cols + (c as usize);
                    let glyph = self.vignette_glyph[idx];
                    frame.set(r, c, Cell {
                        ch: glyph,
                        fg: VIGNETTE_COLOR,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
            }
        }

        // Render target lit cells inside the iris.
        for cell in &self.cells {
            let dx = cell.col as f32 - cx;
            let dy = (cell.row as f32 - cy) / ASPECT;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < iris_radius {
                frame.set(cell.row, cell.col, Cell {
                    ch: cell.ch,
                    fg: cell.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.cells.clear();
        self.vignette_glyph.clear();
        self.elapsed = Duration::ZERO;
    }
}
