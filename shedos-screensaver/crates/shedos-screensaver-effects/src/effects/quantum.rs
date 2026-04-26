//! quantum — every target cell holds a "superposition" of random
//! glyphs that flickers rapidly. One by one, cells "collapse" to
//! their definite target glyph — like wavefunction collapse on
//! observation. Modern, sci-fi feel.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::seq::SliceRandom;
use std::time::Duration;

const DURATION_MS: u64 = 6_000;
const SUPERPOSITION_GLYPHS: &[char] = &[
    '◯', '◉', '◈', '◊', '◇', '◆', '○', '●', '◐', '◑', '◒', '◓',
    '⊙', '⊚', '⊛', '⊜', '⊝', '✦', '✧', '✩', '✪', '✫', '⌬', 'ψ', 'φ',
];

#[derive(Clone, Copy)]
struct QuantumCell {
    row: u16,
    col: u16,
    target_ch: char,
    target_color: Color,
    /// 0..1 progress at which this cell collapses.
    collapse_at: f32,
}

pub struct Quantum {
    cells: Vec<QuantumCell>,
    elapsed: Duration,
}

impl Quantum {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO }
    }
}

impl Default for Quantum {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Quantum {
    fn name(&self) -> &'static str { "quantum" }
    fn title(&self) -> &'static str { "Quantum" }
    fn description(&self) -> &'static str {
        "Cells flicker through superposed glyphs and collapse one by one to their definite target — wavefunction collapse on observation."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
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
        lit.shuffle(ctx.rng);
        let total = lit.len().max(1) as f32;
        for (i, (r, c, ch, color)) in lit.into_iter().enumerate() {
            self.cells.push(QuantumCell {
                row: r,
                col: c,
                target_ch: ch,
                target_color: color,
                collapse_at: i as f32 / total * 0.95, // last 5% holds the full art
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        // Glyph flicker tick — change every ~50 ms.
        let tick = (self.elapsed.as_millis() / 50) as u64;

        frame.clear();
        for (i, c) in self.cells.iter().enumerate() {
            if progress >= c.collapse_at {
                // Collapsed — show the definite target.
                frame.set(c.row, c.col, Cell {
                    ch: c.target_ch,
                    fg: c.target_color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else {
                // Superposition: flickering glyph in transient blue.
                let h = tick.wrapping_mul(2654435761).wrapping_add(i as u64);
                let idx = (h as usize) % SUPERPOSITION_GLYPHS.len();
                let glyph = SUPERPOSITION_GLYPHS[idx];
                // Soft-blue "uncollapsed" color.
                let fg = Color::rgb(0x74, 0xc7, 0xec);
                frame.set(c.row, c.col, Cell {
                    ch: glyph,
                    fg,
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
