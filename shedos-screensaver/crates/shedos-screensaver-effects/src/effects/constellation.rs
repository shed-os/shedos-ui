//! constellation — cells appear first as tiny dots (·) at their
//! target positions, brighten into stars (✦), then morph into the
//! target glyph. By the end, the canvas displays the SHEDOS art as
//! if a star map was drawn into letterforms.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use shedos_screensaver_core::CellAttrs;
use rand::seq::SliceRandom;
use std::time::Duration;

const DURATION_MS: u64 = 5_500;

#[derive(Clone, Copy)]
struct StarCell {
    row: u16,
    col: u16,
    target_ch: char,
    target_color: Color,
    /// 0..1 progress at which the dot first appears.
    dot_at: f32,
}

pub struct Constellation {
    cells: Vec<StarCell>,
    elapsed: Duration,
}

impl Constellation {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO }
    }
}

impl Default for Constellation {
    fn default() -> Self { Self::new() }
}

impl Effect for Constellation {
    fn name(&self) -> &'static str { "constellation" }
    fn title(&self) -> &'static str { "Constellation" }
    fn description(&self) -> &'static str {
        "Cells appear as dots, brighten into stars, then morph into letterforms — the SHEDOS art drawn as a star map."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        let mut lit: Vec<(u16, u16, char, Color)> = target
            .cells()
            .filter_map(|(r, c, cell)| {
                if cell.ch != ' ' { Some((r, c, cell.ch, cell.fg)) } else { None }
            })
            .collect();
        lit.shuffle(ctx.rng);
        let total = lit.len().max(1) as f32;
        // All dots appear within the first 35 % of the duration so
        // the brighten + morph phases have room to play out.
        for (i, (r, c, ch, color)) in lit.into_iter().enumerate() {
            self.cells.push(StarCell {
                row: r,
                col: c,
                target_ch: ch,
                target_color: color,
                dot_at: (i as f32 / total) * 0.35,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        // Phase windows:
        //   appear (0..0.35) — dots fade in
        //   brighten (0.35..0.6) — dots become stars
        //   morph (0.6..0.85) — stars become target glyphs
        //   settle (0.85..1.0) — color converges to target
        frame.clear();

        let dim_white = Color::rgb(0x80, 0x80, 0xa0);
        let bright_white = Color::rgb(0xff, 0xff, 0xff);

        for c in &self.cells {
            if progress < c.dot_at {
                continue;
            }
            if progress < 0.4 {
                // Dot phase.
                frame.set(c.row, c.col, Cell {
                    ch: '·',
                    fg: dim_white,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else if progress < 0.6 {
                // Brightening to star.
                let local = (progress - 0.4) / 0.2;
                let r_ch = lerp_u8(dim_white.r, bright_white.r, local);
                let g_ch = lerp_u8(dim_white.g, bright_white.g, local);
                let b_ch = lerp_u8(dim_white.b, bright_white.b, local);
                let glyph = if local > 0.5 { '✦' } else { '·' };
                frame.set(c.row, c.col, Cell {
                    ch: glyph,
                    fg: Color::rgb(r_ch, g_ch, b_ch),
                    bg: Color::BASE,
                    attrs: CellAttrs::BOLD,
                });
            } else if progress < 0.85 {
                // Star → target glyph morph (glyph swap is discrete;
                // alternate '✦' and target_ch with rapid blink, then
                // settle on target_ch).
                let local = (progress - 0.6) / 0.25;
                let glyph = if local < 0.5 {
                    if (self.elapsed.as_millis() / 80) % 2 == 0 { '✦' } else { c.target_ch }
                } else {
                    c.target_ch
                };
                let r_ch = lerp_u8(bright_white.r, c.target_color.r, local);
                let g_ch = lerp_u8(bright_white.g, c.target_color.g, local);
                let b_ch = lerp_u8(bright_white.b, c.target_color.b, local);
                frame.set(c.row, c.col, Cell {
                    ch: glyph,
                    fg: Color::rgb(r_ch, g_ch, b_ch),
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else {
                // Settled.
                frame.set(c.row, c.col, Cell {
                    ch: c.target_ch,
                    fg: c.target_color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
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
