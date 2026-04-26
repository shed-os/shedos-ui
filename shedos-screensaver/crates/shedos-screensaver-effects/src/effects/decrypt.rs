//! decrypt — movie-style decryption: every target cell shows random
//! ciphertext characters, and one by one the cells "decrypt" to
//! reveal the underlying glyph. Sequential reveal in shuffled order.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::seq::SliceRandom;
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 5_500;
const CIPHER_GLYPHS: &[char] = &[
    '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '-', '_', '+', '=',
    '<', '>', '{', '}', '[', ']', '/', '\\', '?', ':', ';', '~', '`',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    'a', 'b', 'c', 'd', 'e', 'f', '※', '⊕', '◊', '◇', '◈',
];

struct CipherCell {
    row: u16,
    col: u16,
    target_ch: char,
    target_color: Color,
    /// 0..1 normalized progress at which this cell is fully revealed.
    reveal_at: f32,
}

pub struct Decrypt {
    cells: Vec<CipherCell>,
    elapsed: Duration,
    final_color: Color,
}

impl Decrypt {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO, final_color: Color::TEXT }
    }
}

impl Default for Decrypt {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Decrypt {
    fn name(&self) -> &'static str { "decrypt" }
    fn title(&self) -> &'static str { "Decrypt" }
    fn description(&self) -> &'static str {
        "Movie-style decryption: ciphertext flickers in every cell, then resolves one by one to reveal the glyph."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.final_color = ctx.final_color;

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
            self.cells.push(CipherCell {
                row: r,
                col: c,
                target_ch: ch,
                target_color: color,
                reveal_at: i as f32 / total,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let progress = (self.elapsed.as_secs_f32() / Duration::from_millis(DURATION_MS).as_secs_f32()).min(1.0);

        frame.clear();
        // Use a deterministic-but-time-varying RNG for cipher glyphs
        // so the flicker shimmers without snapshot-test instability:
        // hash the elapsed millisecond and the cell index.
        let tick = (self.elapsed.as_millis() / 60) as u64;

        for (i, cc) in self.cells.iter().enumerate() {
            if progress >= cc.reveal_at {
                // Revealed.
                frame.set(cc.row, cc.col, Cell {
                    ch: cc.target_ch,
                    fg: cc.target_color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else {
                // Show a cipher glyph that changes every ~60 ms.
                let h = tick.wrapping_mul(2654435761).wrapping_add(i as u64);
                let idx = (h as usize) % CIPHER_GLYPHS.len();
                let ch = CIPHER_GLYPHS[idx];
                // Cipher rendered in matrix-green with bold for the
                // most-recent quarter of cells.
                let cipher_color = Color::rgb(0x00, 0xc8, 0x00);
                frame.set(cc.row, cc.col, Cell {
                    ch,
                    fg: cipher_color,
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

// Force-keep the import; we use Rng's range gen indirectly via
// rand::Rng trait at setup time only via shuffle. Suppress unused.
#[allow(dead_code)]
fn _force_rng_use(rng: &mut impl Rng) -> u32 { rng.gen() }
