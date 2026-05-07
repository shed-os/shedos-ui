//! kaleidoscope — top-left quadrant of the canonical SHEDOS reveals
//! first, then mirrored copies appear in the other three quadrants
//! in a fade-in/fade-out window. As mirrors fade, the canonical
//! cells outside the top-left quadrant materialize, leaving only
//! the real SHEDOS at the end.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
/// When canonical non-TL cells start revealing.
const NON_TL_REVEAL_START: f32 = 0.65;
/// When non-TL cells must all be revealed.
const NON_TL_REVEAL_END: f32 = 0.92;
/// When the mirrored quadrants start fading in.
const MIRROR_FADE_IN_START: f32 = 0.40;
/// When mirrors are at peak visibility.
const MIRROR_PEAK: f32 = 0.55;
/// When mirrors must be fully invisible.
const MIRROR_FADE_OUT_END: f32 = 0.92;

#[derive(Clone, Copy)]
struct LitCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// Reveal time normalized to total duration.
    reveal_t: f32,
}

pub struct Kaleidoscope {
    tl_cells: Vec<LitCell>,
    non_tl_cells: Vec<LitCell>,
    canvas_rows: u16,
    canvas_cols: u16,
    elapsed: Duration,
}

impl Kaleidoscope {
    pub fn new() -> Self {
        Self {
            tl_cells: Vec::new(),
            non_tl_cells: Vec::new(),
            canvas_rows: 0,
            canvas_cols: 0,
            elapsed: Duration::ZERO,
        }
    }
}

impl Default for Kaleidoscope {
    fn default() -> Self { Self::new() }
}

impl Effect for Kaleidoscope {
    fn name(&self) -> &'static str { "kaleidoscope" }
    fn title(&self) -> &'static str { "Kaleidoscope" }
    fn description(&self) -> &'static str {
        "Top-left quadrant fills first; mirrored copies appear in the other three quadrants then fade as the canonical SHEDOS resolves."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.tl_cells.clear();
        self.non_tl_cells.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();

        let mid_r = target.rows() / 2;
        let mid_c = target.cols() / 2;

        let mut tl: Vec<(u16, u16, char, Color)> = Vec::new();
        let mut other: Vec<(u16, u16, char, Color)> = Vec::new();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            if r < mid_r && c < mid_c {
                tl.push((r, c, cell.ch, cell.fg));
            } else {
                other.push((r, c, cell.ch, cell.fg));
            }
        }

        // TL cells reveal across [0, MIRROR_FADE_IN_START - small gap].
        let tl_end = MIRROR_FADE_IN_START - 0.05;
        if !tl.is_empty() {
            let spacing = tl_end / (tl.len() as f32);
            for (i, (r, c, ch, color)) in tl.into_iter().enumerate() {
                let reveal_t = (i as f32) * spacing;
                self.tl_cells.push(LitCell { row: r, col: c, ch, color, reveal_t });
            }
        }
        // Non-TL cells reveal across [NON_TL_REVEAL_START, NON_TL_REVEAL_END].
        if !other.is_empty() {
            let span = NON_TL_REVEAL_END - NON_TL_REVEAL_START;
            let spacing = span / (other.len() as f32);
            for (i, (r, c, ch, color)) in other.into_iter().enumerate() {
                let reveal_t = NON_TL_REVEAL_START + (i as f32) * spacing;
                self.non_tl_cells.push(LitCell { row: r, col: c, ch, color, reveal_t });
            }
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        // Mirror visibility (rises 0→1 between FADE_IN_START and PEAK,
        // then 1→0 between PEAK and FADE_OUT_END).
        let mirror_alpha = if progress < MIRROR_FADE_IN_START {
            0.0
        } else if progress < MIRROR_PEAK {
            (progress - MIRROR_FADE_IN_START) / (MIRROR_PEAK - MIRROR_FADE_IN_START)
        } else if progress < MIRROR_FADE_OUT_END {
            1.0 - (progress - MIRROR_PEAK) / (MIRROR_FADE_OUT_END - MIRROR_PEAK)
        } else {
            0.0
        };
        let mirror_alpha = mirror_alpha.clamp(0.0, 1.0);

        // 1) Render mirrored TL cells in TR/BL/BR quadrants. Done first
        //    so canonical renders below can overwrite.
        if mirror_alpha > 0.0 {
            for tl in &self.tl_cells {
                if progress < tl.reveal_t {
                    continue;
                }
                let mr = self.canvas_rows.saturating_sub(1).saturating_sub(tl.row);
                let mc = self.canvas_cols.saturating_sub(1).saturating_sub(tl.col);
                let fg = lerp_color(Color::BASE, tl.color, mirror_alpha);
                // TR quadrant
                if mc < self.canvas_cols && mc != tl.col {
                    frame.set(tl.row, mc, Cell {
                        ch: tl.ch,
                        fg,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
                // BL quadrant
                if mr < self.canvas_rows && mr != tl.row {
                    frame.set(mr, tl.col, Cell {
                        ch: tl.ch,
                        fg,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
                // BR quadrant
                if mr < self.canvas_rows
                    && mc < self.canvas_cols
                    && (mr != tl.row || mc != tl.col)
                {
                    frame.set(mr, mc, Cell {
                        ch: tl.ch,
                        fg,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
            }
        }

        // 2) Render canonical TL cells.
        for tl in &self.tl_cells {
            if progress >= tl.reveal_t {
                frame.set(tl.row, tl.col, Cell {
                    ch: tl.ch,
                    fg: tl.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        // 3) Render canonical non-TL cells (overwrites mirrors).
        for nc in &self.non_tl_cells {
            if progress >= nc.reveal_t {
                frame.set(nc.row, nc.col, Cell {
                    ch: nc.ch,
                    fg: nc.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.tl_cells.clear();
        self.non_tl_cells.clear();
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
