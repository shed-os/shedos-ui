//! pixelate-in — coarse blocky version of SHEDOS appears immediately
//! at low resolution; refines through 8× → 4× → 2× → 1× until the
//! art is sharp. Each stage averages target colors over its block.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 4_000;
/// Block scales the effect steps through, in order. The last entry
/// (1) is the target itself.
const SCALES: &[u16] = &[8, 4, 2, 1];

#[derive(Clone, Copy)]
struct ResolvedCell {
    row: u16,
    col: u16,
    ch: char,
    fg: Color,
}

pub struct PixelateIn {
    /// Snapshot of every lit target cell — used at the final stage.
    target_cells: Vec<ResolvedCell>,
    /// Pre-computed cell-render tables for each non-final scale.
    /// Each entry maps a canvas (row, col) to the block fill color
    /// for that scale; absent entries render as blank.
    stages: Vec<Vec<ResolvedCell>>,
    canvas_rows: u16,
    canvas_cols: u16,
    elapsed: Duration,
}

impl PixelateIn {
    pub fn new() -> Self {
        Self {
            target_cells: Vec::new(),
            stages: Vec::new(),
            canvas_rows: 0,
            canvas_cols: 0,
            elapsed: Duration::ZERO,
        }
    }
}

impl Default for PixelateIn {
    fn default() -> Self { Self::new() }
}

impl Effect for PixelateIn {
    fn name(&self) -> &'static str { "pixelate-in" }
    fn title(&self) -> &'static str { "Pixelate In" }
    fn description(&self) -> &'static str {
        "A coarse 8× pixelated SHEDOS appears immediately and refines through 4× and 2× until the art is sharp."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.target_cells.clear();
        self.stages.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();

        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.target_cells.push(ResolvedCell { row: r, col: c, ch: cell.ch, fg: cell.fg });
        }

        // Pre-compute one stage per non-final scale (8, 4, 2). For
        // each block of size scale × scale, average the colors of
        // its lit cells; if the block has any lit cells, fill the
        // whole block with '█' in that average color.
        for &scale in &SCALES[..SCALES.len() - 1] {
            let mut stage_cells: Vec<ResolvedCell> = Vec::new();
            let block_rows = target.rows().div_ceil(scale);
            let block_cols = target.cols().div_ceil(scale);
            for br in 0..block_rows {
                for bc in 0..block_cols {
                    let r0 = br * scale;
                    let c0 = bc * scale;
                    let r1 = (r0 + scale).min(target.rows());
                    let c1 = (c0 + scale).min(target.cols());
                    let mut sum_r: u32 = 0;
                    let mut sum_g: u32 = 0;
                    let mut sum_b: u32 = 0;
                    let mut count: u32 = 0;
                    for rr in r0..r1 {
                        for cc in c0..c1 {
                            if let Some(cell) = target.get(rr, cc) {
                                if cell.ch != ' ' {
                                    sum_r += cell.fg.r as u32;
                                    sum_g += cell.fg.g as u32;
                                    sum_b += cell.fg.b as u32;
                                    count += 1;
                                }
                            }
                        }
                    }
                    if count == 0 {
                        continue;
                    }
                    let fg = Color::rgb(
                        (sum_r / count) as u8,
                        (sum_g / count) as u8,
                        (sum_b / count) as u8,
                    );
                    for rr in r0..r1 {
                        for cc in c0..c1 {
                            stage_cells.push(ResolvedCell { row: rr, col: cc, ch: '█', fg });
                        }
                    }
                }
            }
            self.stages.push(stage_cells);
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        // Each non-final stage occupies an equal slice of the duration.
        // The final 1× stage occupies the last slice. SCALES.len() = 4
        // gives 4 slices; with 3 pre-computed stages plus 1 final.
        let n = SCALES.len();
        let slice_size = 1.0 / (n as f32);
        let stage_idx = (progress / slice_size).floor() as usize;

        if stage_idx >= self.stages.len() {
            // Final stage — render exact target.
            for tc in &self.target_cells {
                frame.set(tc.row, tc.col, Cell {
                    ch: tc.ch,
                    fg: tc.fg,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        } else {
            let stage = &self.stages[stage_idx];
            for cell in stage {
                frame.set(cell.row, cell.col, Cell {
                    ch: cell.ch,
                    fg: cell.fg,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.target_cells.clear();
        self.stages.clear();
        self.elapsed = Duration::ZERO;
    }
}
