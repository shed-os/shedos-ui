//! fog-clear — full canvas covered by a fog texture; the fog
//! dissipates radially outward from center, revealing SHEDOS as
//! it thins. Inner cells clear first; outer cells last.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 4_000;
const ASPECT: f32 = 0.5;
/// Length of each cell's fog→clear transition window, as a fraction
/// of the total duration.
const TRANSITION: f32 = 0.15;
const FOG_GLYPHS: &[char] = &['░', '▒', '·', '⋅'];

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// Normalized distance from canvas center [0, 1].
    norm_dist: f32,
}

pub struct FogClear {
    cells: Vec<TargetCell>,
    fog_glyph_per_cell: Vec<char>,
    canvas_rows: u16,
    canvas_cols: u16,
    canvas_max_dist: f32,
    elapsed: Duration,
}

impl FogClear {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            fog_glyph_per_cell: Vec::new(),
            canvas_rows: 0,
            canvas_cols: 0,
            canvas_max_dist: 1.0,
            elapsed: Duration::ZERO,
        }
    }
}

impl Default for FogClear {
    fn default() -> Self { Self::new() }
}

impl Effect for FogClear {
    fn name(&self) -> &'static str { "fog-clear" }
    fn title(&self) -> &'static str { "Fog Clear" }
    fn description(&self) -> &'static str {
        "Full canvas covered by a fog texture; the fog dissipates radially outward from center, revealing SHEDOS as it thins."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();
        let cx = target.cols() as f32 * 0.5;
        let cy = target.rows() as f32 * 0.5;
        let max_d = ((cx * cx) + (cy / ASPECT * cy / ASPECT)).sqrt().max(1.0);
        self.canvas_max_dist = max_d;

        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            let dx = c as f32 - cx;
            let dy = (r as f32 - cy) / ASPECT;
            let dist = (dx * dx + dy * dy).sqrt();
            self.cells.push(TargetCell {
                row: r,
                col: c,
                ch: cell.ch,
                color: cell.fg,
                norm_dist: (dist / self.canvas_max_dist).clamp(0.0, 1.0),
            });
        }

        // Pre-pick a fog glyph per canvas cell — stable through the
        // animation so the fog texture doesn't flicker.
        let n = (target.rows() as usize) * (target.cols() as usize);
        self.fog_glyph_per_cell.clear();
        self.fog_glyph_per_cell.reserve(n);
        for _ in 0..n {
            let i = ctx.rng.gen_range(0..FOG_GLYPHS.len());
            self.fog_glyph_per_cell.push(FOG_GLYPHS[i]);
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        // Render the fog texture across the whole canvas. Each cell's
        // fog dissipation finishes at fog_end = nd * (1 - TRANSITION) +
        // TRANSITION, so inner cells (nd ≈ 0) clear at progress ≈ TRANSITION
        // and corner cells (nd ≈ 1) clear at progress = 1.0.
        let cx = self.canvas_cols as f32 * 0.5;
        let cy = self.canvas_rows as f32 * 0.5;
        let cols = self.canvas_cols as usize;
        for r in 0..self.canvas_rows {
            for c in 0..self.canvas_cols {
                let dx = c as f32 - cx;
                let dy = (r as f32 - cy) / ASPECT;
                let dist = (dx * dx + dy * dy).sqrt();
                let nd = (dist / self.canvas_max_dist).clamp(0.0, 1.0);
                let fog_end = nd * (1.0 - TRANSITION) + TRANSITION;
                if progress < fog_end {
                    let alpha = ((fog_end - progress) / TRANSITION).clamp(0.0, 1.0);
                    let dim = lerp_color(Color::BASE, Color::rgb(0x80, 0x80, 0x80), alpha);
                    let idx = (r as usize) * cols + (c as usize);
                    let glyph = self.fog_glyph_per_cell[idx];
                    frame.set(r, c, Cell {
                        ch: glyph,
                        fg: dim,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
            }
        }

        // Overlay the target's lit cells where fog has fully cleared.
        for tc in &self.cells {
            let fog_end = tc.norm_dist * (1.0 - TRANSITION) + TRANSITION;
            if progress >= fog_end {
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
        self.fog_glyph_per_cell.clear();
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
