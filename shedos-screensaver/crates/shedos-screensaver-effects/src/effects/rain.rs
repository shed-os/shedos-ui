//! rain — characters fall from the top of the canvas into their
//! target positions. Each lit cell of the target gets one raindrop;
//! drop start times are staggered across the duration so the
//! waterfall is continuous rather than a single salvo.

use crate::easing;
use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use rand::seq::SliceRandom;
use std::time::Duration;

const DURATION_MS: u64 = 4_500;

struct Drop {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// 0..1 normalized start time within the effect's total duration.
    start_at: f32,
    /// 0..1 normalized end time (when this drop has landed).
    end_at: f32,
}

pub struct Rain {
    drops: Vec<Drop>,
    elapsed: Duration,
    target_rows: u16,
    target_cols: u16,
    final_color: Color,
}

impl Rain {
    pub fn new() -> Self {
        Self {
            drops: Vec::new(),
            elapsed: Duration::ZERO,
            target_rows: 0,
            target_cols: 0,
            final_color: Color::TEXT,
        }
    }
}

impl Default for Rain {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for Rain {
    fn name(&self) -> &'static str { "rain" }
    fn title(&self) -> &'static str { "Rain" }
    fn description(&self) -> &'static str {
        "Characters fall from the top of the canvas into their target positions, staggered like a waterfall."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }
    fn reactive(&self) -> bool { true }

    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>) {
        self.drops.clear();
        self.elapsed = Duration::ZERO;
        self.target_rows = target.rows();
        self.target_cols = target.cols();
        self.final_color = ctx.final_color;

        let mut order: Vec<(u16, u16, char, Color)> = target
            .cells()
            .filter_map(|(r, c, cell)| {
                if cell.ch != ' ' {
                    Some((r, c, cell.ch, cell.fg))
                } else {
                    None
                }
            })
            .collect();
        order.shuffle(ctx.rng);

        let total = order.len().max(1) as f32;
        for (i, (r, c, ch, color)) in order.into_iter().enumerate() {
            // Stagger start times across the first 60% of the
            // duration so all drops land before the effect ends.
            let start = (i as f32 / total) * 0.6;
            let fall_time = 0.4; // each drop takes 40% of duration to fall
            self.drops.push(Drop {
                row: r,
                col: c,
                ch,
                color,
                start_at: start,
                end_at: (start + fall_time).min(1.0),
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        // Audio: a beat slightly accelerates remaining drops by
        // pulling their end_at earlier. Subtle so it doesn't make
        // the fall feel jittery.
        let speed_boost = if audio.map(|a| a.beat).unwrap_or(false) { 0.85 } else { 1.0 };

        frame.clear();
        let mut all_landed = true;

        for d in &self.drops {
            let drop_end = d.end_at * speed_boost;
            if progress >= drop_end {
                // Landed at target.
                frame.set(d.row, d.col, Cell {
                    ch: d.ch,
                    fg: d.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            } else if progress >= d.start_at {
                all_landed = false;
                // In flight: linearly interpolate row from -1 to target.
                let local = (progress - d.start_at) / (drop_end - d.start_at).max(1e-6);
                let eased = easing::ease_in_quad(local);
                let from_row = -1.0_f32;
                let to_row = d.row as f32;
                let r = easing::lerp(from_row, to_row, eased);
                if r >= 0.0 && (r as u16) < self.target_rows {
                    let dim = (eased * 0.5 + 0.5).min(1.0);
                    let fg = scale(d.color, dim);
                    frame.set(r as u16, d.col, Cell {
                        ch: d.ch,
                        fg,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
            } else {
                all_landed = false;
                // Hasn't started yet — leave canvas blank at this position.
            }
        }

        all_landed && progress >= 1.0
    }

    fn reset(&mut self) {
        self.drops.clear();
        self.elapsed = Duration::ZERO;
    }
}

fn scale(c: Color, k: f32) -> Color {
    let k = k.clamp(0.0, 1.0);
    Color::rgb(
        (c.r as f32 * k) as u8,
        (c.g as f32 * k) as u8,
        (c.b as f32 * k) as u8,
    )
}
