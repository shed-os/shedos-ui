//! hologram — a sci-fi hologram reveal: horizontal scanlines move
//! across the canvas with cyan tint and slight horizontal jitter,
//! and target cells solidify in the scanline's wake. Combines the
//! "pass" of wipe with the visual texture of CRT scan animation.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
const SCAN_LINES: usize = 3;
/// How tall (in cells) each scanline's "active band" is.
const SCAN_BAND: f32 = 1.5;

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// Cumulative reveal: 0..1 maximum brightness reached.
    reveal: f32,
}

pub struct Hologram {
    cells: Vec<TargetCell>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
}

impl Hologram {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO, rows: 0, cols: 0 }
    }
}

impl Default for Hologram {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Hologram {
    fn name(&self) -> &'static str { "hologram" }
    fn title(&self) -> &'static str { "Hologram" }
    fn description(&self) -> &'static str {
        "Horizontal scanlines sweep cyan across the canvas; target cells solidify in their wake with a CRT shimmer."
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
            self.cells.push(TargetCell { row: r, col: c, ch: cell.ch, color: cell.fg, reveal: 0.0 });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let t = self.elapsed.as_secs_f32();
        let rows_f = self.rows as f32;

        // Scanline positions: each scanline traverses top→bottom over
        // the full duration, offset so they don't all hit the same row.
        let scan_positions: [f32; SCAN_LINES] = std::array::from_fn(|i| {
            let phase = i as f32 / SCAN_LINES as f32;
            ((progress + phase) % 1.0) * (rows_f + 2.0) - 1.0
        });

        // Per-cell illumination from the scanlines.
        for c in &mut self.cells {
            let mut max_illum = 0.0_f32;
            for &scan_y in &scan_positions {
                let dy = (c.row as f32 - scan_y).abs();
                if dy < SCAN_BAND {
                    max_illum = max_illum.max(1.0 - (dy / SCAN_BAND));
                }
            }
            c.reveal = c.reveal.max(max_illum * 0.5 + max_illum * progress * 0.5);
        }
        // Past 80%, force-reveal so we always finish clean.
        let force_reveal = ((progress - 0.8) / 0.2).clamp(0.0, 1.0);

        frame.clear();

        // Scanline stripes (full-width cyan bars).
        for &scan_y in &scan_positions {
            let r_int = scan_y.round() as i32;
            if r_int >= 0 && r_int < self.rows as i32 {
                for c in 0..self.cols {
                    // Light scan stripe — only every other column for "TV scanline" texture.
                    if (c as f32 + t * 4.0) as u16 % 2 == 0 {
                        frame.set(r_int as u16, c, Cell {
                            ch: '─',
                            fg: Color::rgb(0x00, 0xff, 0xff),
                            bg: Color::BASE,
                            attrs: Default::default(),
                        });
                    }
                }
            }
        }

        // Reveal target cells (overwrites scan stripes where they coincide).
        for c in &self.cells {
            let intensity = c.reveal.max(force_reveal);
            if intensity <= 0.05 {
                continue;
            }
            // Mix from cyan (transient) at low intensity to target color at high.
            let cyan = Color::rgb(0x00, 0xff, 0xff);
            let r_ch = lerp_u8(cyan.r, c.color.r, intensity);
            let g_ch = lerp_u8(cyan.g, c.color.g, intensity);
            let b_ch = lerp_u8(cyan.b, c.color.b, intensity);
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
        for c in &mut self.cells {
            c.reveal = 0.0;
        }
        self.elapsed = Duration::ZERO;
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (a as f32 * (1.0 - t) + b as f32 * t) as u8
}
