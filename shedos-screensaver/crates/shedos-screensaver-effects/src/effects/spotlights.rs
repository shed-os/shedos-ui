//! spotlights — three circular spotlights move around the canvas.
//! Cells are revealed at full brightness inside spotlights, dimly
//! visible at the edges, and hidden outside. By the end of the
//! duration, all cells have been illuminated and remain visible.

use crate::easing;
use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::Rng;
use std::time::Duration;

const DURATION_MS: u64 = 5_500;
const SPOTLIGHT_COUNT: usize = 3;
const SPOTLIGHT_RADIUS: f32 = 6.0;
const ASPECT: f32 = 0.5; // terminal cells are ~2x tall as wide

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// Once this cell has been touched by a spotlight, we keep it
    /// visible (cumulative reveal). 0..1 maximum brightness reached.
    revealed: f32,
}

struct Spotlight {
    cx_phase: f32,
    cy_phase: f32,
    cx_amp: f32,
    cy_amp: f32,
    speed: f32,
}

pub struct Spotlights {
    cells: Vec<TargetCell>,
    spots: [Spotlight; SPOTLIGHT_COUNT],
    elapsed: Duration,
    rows: u16,
    cols: u16,
}

impl Spotlights {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            spots: [
                Spotlight { cx_phase: 0.0, cy_phase: 0.0, cx_amp: 0.0, cy_amp: 0.0, speed: 0.0 },
                Spotlight { cx_phase: 0.0, cy_phase: 0.0, cx_amp: 0.0, cy_amp: 0.0, speed: 0.0 },
                Spotlight { cx_phase: 0.0, cy_phase: 0.0, cx_amp: 0.0, cy_amp: 0.0, speed: 0.0 },
            ],
            elapsed: Duration::ZERO,
            rows: 0,
            cols: 0,
        }
    }
}

impl Default for Spotlights {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Spotlights {
    fn name(&self) -> &'static str { "spotlights" }
    fn title(&self) -> &'static str { "Spotlights" }
    fn description(&self) -> &'static str {
        "Three moving spotlights illuminate the canvas; cells are progressively revealed as the lights pass over them."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();
        self.cols = target.cols();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.cells.push(TargetCell {
                row: r,
                col: c,
                ch: cell.ch,
                color: cell.fg,
                revealed: 0.0,
            });
        }
        // Randomize spotlight orbits per setup so each session
        // looks different.
        for s in &mut self.spots {
            s.cx_phase = ctx.rng.gen_range(0.0..std::f32::consts::TAU);
            s.cy_phase = ctx.rng.gen_range(0.0..std::f32::consts::TAU);
            s.cx_amp = ctx.rng.gen_range(0.3..0.45) * self.cols as f32;
            s.cy_amp = ctx.rng.gen_range(0.3..0.45) * self.rows as f32;
            s.speed = ctx.rng.gen_range(1.6..2.4);
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);
        let t = self.elapsed.as_secs_f32();
        let cx_center = self.cols as f32 * 0.5;
        let cy_center = self.rows as f32 * 0.5;

        // Compute spotlight positions.
        let positions: [(f32, f32); SPOTLIGHT_COUNT] = std::array::from_fn(|i| {
            let s = &self.spots[i];
            let x = cx_center + s.cx_amp * (s.cx_phase + t * s.speed).cos();
            let y = cy_center + s.cy_amp * (s.cy_phase + t * s.speed * 1.3).sin();
            (x, y)
        });

        // For each lit cell, compute spotlight illumination + accumulate.
        for c in &mut self.cells {
            let max_illum = positions
                .iter()
                .map(|&(sx, sy)| {
                    let dx = c.col as f32 - sx;
                    let dy = (c.row as f32 - sy) / ASPECT;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > SPOTLIGHT_RADIUS {
                        0.0
                    } else {
                        1.0 - (dist / SPOTLIGHT_RADIUS).powi(2)
                    }
                })
                .fold(0.0_f32, f32::max);
            // Cumulative reveal: cells stay lit at max past brightness.
            c.revealed = c.revealed.max(max_illum);
        }

        // Past 80% of the duration, force-reveal anything still dark
        // so the effect always finishes with the full target visible.
        let force_reveal = ((progress - 0.8) / 0.2).clamp(0.0, 1.0);

        frame.clear();
        for c in &self.cells {
            let intensity = c.revealed.max(force_reveal);
            if intensity <= 0.01 {
                continue;
            }
            let i = easing::ease_out_quad(intensity);
            let fg = Color::rgb(
                (c.color.r as f32 * i) as u8,
                (c.color.g as f32 * i) as u8,
                (c.color.b as f32 * i) as u8,
            );
            frame.set(c.row, c.col, Cell {
                ch: c.ch,
                fg,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        for c in &mut self.cells {
            c.revealed = 0.0;
        }
        self.elapsed = Duration::ZERO;
    }
}
