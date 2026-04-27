//! matrix-rain — green Matrix-style rain falls everywhere across
//! the canvas. As each column's trail head sweeps down, any SHEDOS
//! target cell whose row the head has reached "freezes" out of the
//! rain into its final glyph and color. Letters crystallize as the
//! rain washes over them; once every target cell is frozen the rain
//! stops and the resolved SHEDOS sits solid.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use shedos_screensaver_core::CellAttrs;
use rand::Rng;
use rand::SeedableRng;
use std::time::Duration;

const DURATION_MS: u64 = 6_500;
/// Hard fallback: any straggler target cell freezes by this progress
/// even if no trail has swept it. Keeps the resolved frame complete.
const FORCE_FREEZE_AT: f32 = 0.95;
const KATAKANA: &str = "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン";

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    ch: char,
    color: Color,
    frozen: bool,
}

struct Trail {
    /// Float row of the trail's leading head.
    head: f32,
    speed: f32,
    /// Trail length in cells.
    length: i32,
}

pub struct MatrixRain {
    /// Target cells grouped by column so each trail-head sweep only
    /// walks its own column's cells.
    target_by_col: Vec<Vec<TargetCell>>,
    trails_per_col: Vec<Option<Trail>>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
    glyph_chars: Vec<char>,
}

impl MatrixRain {
    pub fn new() -> Self {
        Self {
            target_by_col: Vec::new(),
            trails_per_col: Vec::new(),
            elapsed: Duration::ZERO,
            rows: 0,
            cols: 0,
            glyph_chars: KATAKANA.chars().collect(),
        }
    }
}

impl Default for MatrixRain {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for MatrixRain {
    fn name(&self) -> &'static str { "matrix-rain" }
    fn title(&self) -> &'static str { "Matrix Rain" }
    fn description(&self) -> &'static str {
        "Green katakana rain falls; letters crystallize out of the rain as each trail sweeps over them."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }
    fn reactive(&self) -> bool { true }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();
        self.cols = target.cols();
        self.target_by_col = (0..self.cols as usize).map(|_| Vec::new()).collect();
        self.trails_per_col = (0..self.cols as usize).map(|_| None).collect();

        for (r, c, cell) in target.cells() {
            if cell.ch != ' ' {
                self.target_by_col[c as usize].push(TargetCell {
                    row: r,
                    ch: cell.ch,
                    color: cell.fg,
                    frozen: false,
                });
            }
        }
        for col_cells in &mut self.target_by_col {
            col_cells.sort_by_key(|c| c.row);
        }

        // Pre-populate every column with a trail so there's no awkward
        // empty start.
        for col in 0..self.cols as usize {
            self.trails_per_col[col] = Some(Trail {
                head: ctx.rng.gen_range(-(self.rows as f32) * 0.3..0.0),
                speed: ctx.rng.gen_range(8.0..22.0),
                length: ctx.rng.gen_range(8..18),
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let dt_s = dt.as_secs_f32();

        let mut frame_rng = rand_chacha::ChaCha8Rng::from_seed(seed_from_u64(self.elapsed.as_millis() as u64));

        frame.clear();

        // 1. Advance trails; the head sweeps target cells frozen.
        for col in 0..self.cols as usize {
            if let Some(trail) = self.trails_per_col[col].as_mut() {
                trail.head += trail.speed * dt_s;
                let head_r = trail.head as i32;

                for cell in &mut self.target_by_col[col] {
                    if !cell.frozen && (cell.row as i32) <= head_r {
                        cell.frozen = true;
                    }
                }

                // Recycle the trail when the whole tail has run off
                // the bottom. After 85 % progress, retire instead of
                // recycling so the canvas can empty out cleanly.
                if head_r - trail.length > self.rows as i32 {
                    if progress < 0.85 {
                        trail.head = -(frame_rng.gen_range(0..self.rows as i32) as f32) * 0.5;
                        trail.speed = frame_rng.gen_range(8.0..22.0);
                        trail.length = frame_rng.gen_range(8..18);
                    } else {
                        self.trails_per_col[col] = None;
                    }
                }
            }
        }

        // 2. Hard fallback at 95 % so the resolved SHEDOS is always
        // complete by progress=1.0 even if any column lost its trail
        // before sweeping its lowest target row.
        if progress >= FORCE_FREEZE_AT {
            for col_cells in &mut self.target_by_col {
                for cell in col_cells {
                    cell.frozen = true;
                }
            }
        }

        // 3. Suppress rain once every cell is frozen (or at the 95 %
        // cutoff). Drop trails so they don't reappear next frame.
        let all_frozen = self
            .target_by_col
            .iter()
            .all(|col| col.iter().all(|c| c.frozen));
        let suppress_rain = all_frozen || progress >= FORCE_FREEZE_AT;

        if suppress_rain {
            for t in &mut self.trails_per_col {
                *t = None;
            }
        } else {
            for col in 0..self.cols as usize {
                if let Some(trail) = self.trails_per_col[col].as_ref() {
                    let head_r = trail.head as i32;
                    for k in 0..trail.length {
                        let r = head_r - k;
                        if !(0..self.rows as i32).contains(&r) {
                            continue;
                        }
                        let intensity = 1.0 - (k as f32 / trail.length as f32);
                        let g = self.glyph_chars[(frame_rng.gen::<usize>()) % self.glyph_chars.len()];
                        let (fg, attrs) = if k == 0 {
                            (Color::rgb(0xff, 0xff, 0xff), CellAttrs::BOLD)
                        } else {
                            let r_ch = (0x33_u8 as f32 * intensity) as u8;
                            let g_ch = (0xff_u8 as f32 * intensity) as u8;
                            let b_ch = (0x77_u8 as f32 * intensity) as u8;
                            (Color::rgb(r_ch, g_ch, b_ch), CellAttrs::NONE)
                        };
                        frame.set(r as u16, col as u16, Cell {
                            ch: g,
                            fg,
                            bg: Color::BASE,
                            attrs,
                        });
                    }
                }
            }
        }

        // 4. Overlay frozen target cells last so they always win
        // against any trail residue landing on the same cell.
        for (col_idx, col_cells) in self.target_by_col.iter().enumerate() {
            for cell in col_cells {
                if cell.frozen {
                    frame.set(cell.row, col_idx as u16, Cell {
                        ch: cell.ch,
                        fg: cell.color,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
            }
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.elapsed = Duration::ZERO;
        for t in &mut self.trails_per_col {
            *t = None;
        }
        for col_cells in &mut self.target_by_col {
            for cell in col_cells {
                cell.frozen = false;
            }
        }
    }
}

fn seed_from_u64(seed: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&seed.to_le_bytes());
    out
}
