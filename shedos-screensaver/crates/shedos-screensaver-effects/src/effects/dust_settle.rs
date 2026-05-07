//! dust-settle — dust motes swirl chaotically with decaying
//! amplitude; gradually crystallize into the SHEDOS shape as the
//! swirl damps to zero.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
/// Initial swirl amplitude in cells (decays to zero by progress=1.0).
const SWIRL_AMP: f32 = 6.0;
/// Aspect-corrected vertical-to-horizontal ratio for swirl arcs.
const ASPECT: f32 = 0.5;
const DUST_GLYPHS: &[char] = &['·', '⋅', '∙', '*'];
const DUST_COLOR: Color = Color::rgb(0xa6, 0xad, 0xc8);

#[derive(Clone, Copy)]
struct Mote {
    target_row: u16,
    target_col: u16,
    target_ch: char,
    target_color: Color,
    /// Phase offsets for x/y swirl waves.
    phase_x: f32,
    phase_y: f32,
    /// Chaotic-period multiplier — different motes orbit at different
    /// frequencies so they don't move in lockstep.
    freq: f32,
    glyph: char,
}

pub struct DustSettle {
    motes: Vec<Mote>,
    canvas_rows: u16,
    canvas_cols: u16,
    elapsed: Duration,
}

impl DustSettle {
    pub fn new() -> Self {
        Self { motes: Vec::new(), canvas_rows: 0, canvas_cols: 0, elapsed: Duration::ZERO }
    }
}

impl Default for DustSettle {
    fn default() -> Self { Self::new() }
}

impl Effect for DustSettle {
    fn name(&self) -> &'static str { "dust-settle" }
    fn title(&self) -> &'static str { "Dust Settle" }
    fn description(&self) -> &'static str {
        "Dust motes swirl chaotically with decaying amplitude, crystallizing into the SHEDOS shape as the swirl damps to zero."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.motes.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();

        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.motes.push(Mote {
                target_row: r,
                target_col: c,
                target_ch: cell.ch,
                target_color: cell.fg,
                phase_x: ctx.rng.gen_range(0.0..std::f32::consts::TAU),
                phase_y: ctx.rng.gen_range(0.0..std::f32::consts::TAU),
                freq: ctx.rng.gen_range(2.5..5.0),
                glyph: DUST_GLYPHS[ctx.rng.gen_range(0..DUST_GLYPHS.len())],
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        // Quadratic decay: swirl amplitude (1-progress)² so the last
        // ~30% of the duration the motes barely move.
        let decay = (1.0 - progress).max(0.0);
        let amp = SWIRL_AMP * decay * decay;

        for m in &self.motes {
            if progress >= 1.0 {
                frame.set(m.target_row, m.target_col, Cell {
                    ch: m.target_ch,
                    fg: m.target_color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
                continue;
            }
            let dx = amp * (m.phase_x + progress * m.freq * 6.0).cos();
            let dy = amp * ASPECT * (m.phase_y + progress * m.freq * 6.0).sin();
            let r_f = (m.target_row as f32) + dy;
            let c_f = (m.target_col as f32) + dx;
            let r = r_f.round() as i32;
            let c = c_f.round() as i32;
            if r < 0 || r >= self.canvas_rows as i32 || c < 0 || c >= self.canvas_cols as i32 {
                continue;
            }
            // Color blends from dust → target as decay → 0.
            let fg = lerp_color(DUST_COLOR, m.target_color, 1.0 - decay);
            let ch = if decay > 0.15 { m.glyph } else { m.target_ch };
            frame.set(r as u16, c as u16, Cell {
                ch,
                fg,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.motes.clear();
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
