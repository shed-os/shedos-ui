//! synthgrid — synthwave perspective grid forms first (lines emerge
//! from the horizon and grow outward), then dissolves into the
//! target ASCII art. Vibrant magenta + cyan retro aesthetic.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 5_500;

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}

pub struct Synthgrid {
    cells: Vec<TargetCell>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
}

impl Synthgrid {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO, rows: 0, cols: 0 }
    }
}

impl Default for Synthgrid {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Synthgrid {
    fn name(&self) -> &'static str { "synthgrid" }
    fn title(&self) -> &'static str { "Synth Grid" }
    fn description(&self) -> &'static str {
        "Synthwave perspective grid forms from the horizon, then dissolves into the target."
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
        let t = self.elapsed.as_secs_f32();
        let cols_f = self.cols as f32;
        let rows_f = self.rows as f32;
        let horizon = rows_f * 0.5;

        frame.clear();
        // Phase 1 (0..0.6): build the grid.
        let grid_progress = (progress / 0.6).min(1.0);
        if grid_progress > 0.0 {
            // Vertical "perspective" lines: 8 vanishing-point rays.
            for line_i in 0..8 {
                let line_x_at_horizon = cols_f * 0.5;
                let line_x_at_bottom = (line_i as f32 - 3.5) * cols_f / 6.0 + cols_f * 0.5;
                for r in horizon as i32..rows_f as i32 {
                    let r_progress = (r as f32 - horizon) / (rows_f - horizon).max(1.0);
                    let x = line_x_at_horizon + (line_x_at_bottom - line_x_at_horizon) * r_progress;
                    let xi = x.round() as i32;
                    if xi >= 0 && xi < self.cols as i32 {
                        // Only render up to the grid_progress fraction of rays from horizon out.
                        if r_progress <= grid_progress {
                            frame.set(r as u16, xi as u16, Cell {
                                ch: '│',
                                fg: Color::rgb(0xff, 0x00, 0xff), // magenta
                                bg: Color::BASE,
                                attrs: Default::default(),
                            });
                        }
                    }
                }
            }
            // Horizontal "scanlines" emerging from the horizon.
            let scanline_count = 5;
            for sl in 0..scanline_count {
                let r_target = horizon + (rows_f - horizon) * ((sl as f32 + 1.0) / scanline_count as f32);
                let r_at = horizon + (r_target - horizon) * grid_progress;
                let ri = r_at.round() as i32;
                if ri >= 0 && ri < self.rows as i32 {
                    for c in 0..self.cols {
                        // Use cyan, animated phase to suggest motion.
                        let phase = (c as f32 * 0.1 + t * 4.0).sin();
                        if phase > 0.0 {
                            frame.set(ri as u16, c, Cell {
                                ch: '─',
                                fg: Color::rgb(0x00, 0xff, 0xff),
                                bg: Color::BASE,
                                attrs: Default::default(),
                            });
                        }
                    }
                }
            }
        }

        // Phase 2 (0.55..1.0): grid fades + target emerges.
        let target_progress = ((progress - 0.55) / 0.45).clamp(0.0, 1.0);
        for c in &self.cells {
            // Mix from grid-magenta toward c.color as target_progress rises.
            let grid_mag = Color::rgb(0xff, 0x00, 0xff);
            let r_ch = lerp_u8(grid_mag.r, c.color.r, target_progress);
            let g_ch = lerp_u8(grid_mag.g, c.color.g, target_progress);
            let b_ch = lerp_u8(grid_mag.b, c.color.b, target_progress);
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
