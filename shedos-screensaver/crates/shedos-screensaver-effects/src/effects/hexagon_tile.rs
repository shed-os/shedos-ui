//! hexagon-tile — cells reveal in a diagonal hex-flow sweep. Each
//! cell, just before its reveal, briefly shows a hex glyph; once
//! the flip completes, the target glyph + color appears. Rows are
//! offset so the sweep front has a hex-tile feel.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 4_000;
/// Each cell's flip animation window, normalized.
const FLIP_NORM: f32 = 0.05;
const HEX_GLYPHS: &[char] = &['⬢', '⬡', '◆', '◇'];
const FLIP_COLOR: Color = Color::rgb(0xfa, 0xb3, 0x87);

#[derive(Clone, Copy)]
struct Tile {
    row: u16,
    col: u16,
    target_ch: char,
    target_color: Color,
    /// Time the flip animation starts.
    flip_t: f32,
    /// Hex glyph shown during flip.
    flip_glyph: char,
}

pub struct HexagonTile {
    tiles: Vec<Tile>,
    elapsed: Duration,
}

impl HexagonTile {
    pub fn new() -> Self {
        Self { tiles: Vec::new(), elapsed: Duration::ZERO }
    }
}

impl Default for HexagonTile {
    fn default() -> Self { Self::new() }
}

impl Effect for HexagonTile {
    fn name(&self) -> &'static str { "hexagon-tile" }
    fn title(&self) -> &'static str { "Hexagon Tile" }
    fn description(&self) -> &'static str {
        "Cells reveal in a diagonal hex-flow sweep; each shows a brief hex glyph during its flip, then settles to the target."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        use rand::Rng;
        self.tiles.clear();
        self.elapsed = Duration::ZERO;

        let rows = target.rows() as f32;
        let cols = target.cols() as f32;
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            // Hex stagger: even rows have offset 0, odd rows have +0.5.
            // Combined with col + row creates a diagonal sweep front
            // across the canvas, hexagonally offset.
            let offset = if (r % 2) == 0 { 0.0 } else { 0.5 };
            let sweep_norm =
                ((r as f32) + (c as f32) * 0.5 + offset) / (rows + cols * 0.5).max(1.0);
            // Reserve the last FLIP_NORM so every tile has time to
            // settle to the target before progress=1.0.
            let flip_t = sweep_norm * (1.0 - FLIP_NORM);
            let glyph = HEX_GLYPHS[ctx.rng.gen_range(0..HEX_GLYPHS.len())];
            self.tiles.push(Tile {
                row: r,
                col: c,
                target_ch: cell.ch,
                target_color: cell.fg,
                flip_t,
                flip_glyph: glyph,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        for t in &self.tiles {
            if progress < t.flip_t {
                continue;
            }
            if progress < t.flip_t + FLIP_NORM {
                frame.set(t.row, t.col, Cell {
                    ch: t.flip_glyph,
                    fg: FLIP_COLOR,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else {
                frame.set(t.row, t.col, Cell {
                    ch: t.target_ch,
                    fg: t.target_color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.tiles.clear();
        self.elapsed = Duration::ZERO;
    }
}
