//! matrix-rain — green Matrix-style rain falls everywhere across
//! the canvas; once trails reach a target cell's row, they "freeze"
//! into that cell with the target's glyph and color. By the end,
//! every target cell has been frozen and the result is the SHEDOS
//! art with a Matrix afterglow.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use shedos_screensaver_core::CellAttrs;
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 6_500;
const KATAKANA: &str = "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン";

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// 0..1 progress at which this cell freezes to the target.
    freeze_at: f32,
}

struct Trail {
    /// Float row of the trail's leading head.
    head: f32,
    speed: f32,
    /// Trail length in cells.
    length: i32,
}

pub struct MatrixRain {
    target_cells: Vec<TargetCell>,
    trails_per_col: Vec<Option<Trail>>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
    glyph_chars: Vec<char>,
}

impl MatrixRain {
    pub fn new() -> Self {
        Self {
            target_cells: Vec::new(),
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
        "Green katakana rain falls across the canvas; trails freeze into the target cells one by one."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }
    fn reactive(&self) -> bool { true }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.target_cells.clear();
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();
        self.cols = target.cols();
        self.trails_per_col = (0..self.cols as usize).map(|_| None).collect();

        let mut lit: Vec<(u16, u16, char, Color)> = target
            .cells()
            .filter_map(|(r, c, cell)| {
                if cell.ch != ' ' {
                    Some((r, c, cell.ch, cell.fg))
                } else {
                    None
                }
            })
            .collect();
        // Stagger freeze times by column-then-row for a "filling" feel.
        lit.sort_by_key(|&(r, c, _, _)| c as u32 * 100 + r as u32);
        let total = lit.len().max(1) as f32;
        for (i, (r, c, ch, color)) in lit.into_iter().enumerate() {
            self.target_cells.push(TargetCell {
                row: r,
                col: c,
                ch,
                color,
                freeze_at: 0.4 + 0.55 * (i as f32 / total),
            });
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

        // Step trails.
        for col in 0..self.cols as usize {
            if let Some(trail) = self.trails_per_col[col].as_mut() {
                trail.head += trail.speed * dt_s;
                let head_r = trail.head as i32;
                if head_r - trail.length > self.rows as i32 {
                    // Respawn (or stop if we're past 70% — let frozen cells dominate).
                    if progress < 0.85 {
                        trail.head = -(frame_rng.gen_range(0..self.rows as i32) as f32) * 0.5;
                        trail.speed = frame_rng.gen_range(8.0..22.0);
                        trail.length = frame_rng.gen_range(8..18);
                    } else {
                        self.trails_per_col[col] = None;
                        continue;
                    }
                }
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

        // Overwrite frozen target cells. Iterate after trails so frozen
        // cells always win.
        for c in &self.target_cells {
            if progress >= c.freeze_at {
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
        self.elapsed = Duration::ZERO;
        for t in &mut self.trails_per_col {
            *t = None;
        }
    }
}

use rand::SeedableRng;

fn seed_from_u64(seed: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&seed.to_le_bytes());
    out
}
