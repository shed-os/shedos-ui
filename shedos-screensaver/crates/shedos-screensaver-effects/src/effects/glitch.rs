//! glitch — datamosh aesthetic. Target is mostly visible from the
//! start, but random rows scroll/garble/duplicate every few frames,
//! and the corruption gradually heals until the canvas matches the
//! target. A modern, "cyber" feel.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
const GLITCH_GLYPHS: &[char] = &[
    '░', '▒', '▓', '█', '▄', '▀', '▌', '▐',
    '/', '\\', '|', '-', '_', '=', '+', '*', '#',
    '⌬', '⌭', '⌯', '◧', '◨',
];

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}

pub struct Glitch {
    cells: Vec<TargetCell>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
    /// Per-cell "stability" 0..1; 1.0 = no glitch at this cell. Grows
    /// from 0 over time so the canvas heals.
    stability: Vec<Vec<f32>>,
}

impl Glitch {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            elapsed: Duration::ZERO,
            rows: 0,
            cols: 0,
            stability: Vec::new(),
        }
    }
}

impl Default for Glitch {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Glitch {
    fn name(&self) -> &'static str { "glitch" }
    fn title(&self) -> &'static str { "Glitch" }
    fn description(&self) -> &'static str {
        "Datamosh corruption: target is visible but rows scroll and garble; corruption heals over time."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }
    fn reactive(&self) -> bool { true }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();
        self.cols = target.cols();
        self.stability = vec![vec![0.0; self.cols as usize]; self.rows as usize];
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.cells.push(TargetCell { row: r, col: c, ch: cell.ch, color: cell.fg });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let dt_s = dt.as_secs_f32();

        // Two phases:
        //   - "active" (progress < 0.85): kicks knock random rows back,
        //     heal_rate slowly grows stability.
        //   - "settling" (progress >= 0.85): no more kicks; heal
        //     accelerates so we always converge by progress = 1.0.
        let beat_destabilize = audio.map(|a| a.beat).unwrap_or(false);
        let settling = progress >= 0.85;
        let heal_rate = if settling { 8.0 / total } else { 1.5 / total };

        let tick = self.elapsed.as_millis() as u64;
        let mut frame_rng = rand_chacha::ChaCha8Rng::from_seed(seed_from_u64(tick));

        for r in 0..self.rows as usize {
            for c in 0..self.cols as usize {
                self.stability[r][c] = (self.stability[r][c] + heal_rate * dt_s).min(1.0);
            }
        }
        if !settling {
            // Occasional row-glitch: pick a random row, knock its
            // stability down (more frequent if a beat just hit).
            let kick_chance = if beat_destabilize { 0.6 } else { 0.18 };
            if frame_rng.gen::<f32>() < kick_chance {
                let r = frame_rng.gen_range(0..self.rows as usize);
                for c in 0..self.cols as usize {
                    self.stability[r][c] = (self.stability[r][c] - 0.5).max(0.0);
                }
            }
        }

        frame.clear();
        // Render every target cell. Stable cells show the target;
        // unstable cells show a glitch glyph in saturated color.
        for c in &self.cells {
            let stab = self.stability[c.row as usize][c.col as usize];
            if stab >= 0.85 {
                frame.set(c.row, c.col, Cell {
                    ch: c.ch,
                    fg: c.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else {
                // Pick a glitch glyph based on cell + tick so it shimmers.
                let h = (tick.wrapping_mul(2654435761).wrapping_add(c.row as u64).wrapping_add(c.col as u64 * 17)) as usize;
                let g = GLITCH_GLYPHS[h % GLITCH_GLYPHS.len()];
                // Glitch color: saturated red/cyan based on column parity.
                let glitch_fg = if c.col % 2 == 0 {
                    Color::rgb(0xff, 0x00, 0x80)
                } else {
                    Color::rgb(0x00, 0xff, 0xc0)
                };
                frame.set(c.row, c.col, Cell {
                    ch: g,
                    fg: glitch_fg,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        // Done when ≥99% of cells are stable AND the duration elapsed.
        let total_cells = self.cells.len().max(1) as f32;
        let stable_count = self
            .cells
            .iter()
            .filter(|c| self.stability[c.row as usize][c.col as usize] >= 0.85)
            .count() as f32;
        let stable_ratio = stable_count / total_cells;
        progress >= 1.0 && stable_ratio >= 0.99
    }

    fn reset(&mut self) {
        self.elapsed = Duration::ZERO;
        for row in &mut self.stability {
            row.fill(0.0);
        }
    }
}

use rand::SeedableRng;

fn seed_from_u64(seed: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&seed.to_le_bytes());
    out
}
