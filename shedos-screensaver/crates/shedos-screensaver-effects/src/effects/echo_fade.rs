//! echo-fade — multiple ghost copies of SHEDOS appear at offsets
//! and converge toward the canonical position, each fading in then
//! out. The final copy lands on canonical and solidifies.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 4_500;
const N_ECHOES: usize = 5;

#[derive(Clone, Copy)]
struct Echo {
    /// Offset from canonical position (cols, rows).
    dx: i32,
    dy: i32,
    /// Active window [start, end] in normalized [0, 1] progress.
    start: f32,
    end: f32,
}

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}

pub struct EchoFade {
    echoes: [Echo; N_ECHOES],
    cells: Vec<TargetCell>,
    canvas_rows: u16,
    canvas_cols: u16,
    elapsed: Duration,
}

impl EchoFade {
    pub fn new() -> Self {
        // Five echoes converging toward canonical. The last has
        // offset (0, 0) and its window holds opacity at 1.0.
        let echoes = [
            Echo { dx: -16, dy: -2, start: 0.00, end: 0.20 },
            Echo { dx: -8,  dy: -1, start: 0.20, end: 0.40 },
            Echo { dx:  8,  dy:  1, start: 0.40, end: 0.60 },
            Echo { dx: -4,  dy:  0, start: 0.60, end: 0.78 },
            Echo { dx:  0,  dy:  0, start: 0.78, end: 1.00 },
        ];
        Self {
            echoes,
            cells: Vec::new(),
            canvas_rows: 0,
            canvas_cols: 0,
            elapsed: Duration::ZERO,
        }
    }
}

impl Default for EchoFade {
    fn default() -> Self { Self::new() }
}

impl Effect for EchoFade {
    fn name(&self) -> &'static str { "echo-fade" }
    fn title(&self) -> &'static str { "Echo Fade" }
    fn description(&self) -> &'static str {
        "Multiple ghost copies of SHEDOS appear at offsets and converge toward canonical position; each fades in then out, the final copy solidifies."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_rows = target.rows();
        self.canvas_cols = target.cols();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            self.cells.push(TargetCell {
                row: r,
                col: c,
                ch: cell.ch,
                color: cell.fg,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        let last_idx = N_ECHOES - 1;
        // Pick the active echo. Past the last echo's start, lock to it.
        let active_idx = if progress >= self.echoes[last_idx].start {
            last_idx
        } else {
            self.echoes
                .iter()
                .position(|e| progress >= e.start && progress < e.end)
                .unwrap_or(0)
        };
        let echo = &self.echoes[active_idx];

        // Opacity within the active window. Last echo rises monotonically
        // to 1.0 and stays; earlier echoes fade in (15%), hold (70%), fade
        // out (15%).
        let span = (echo.end - echo.start).max(1e-4);
        let in_win = ((progress - echo.start) / span).clamp(0.0, 1.0);
        let opacity = if active_idx == last_idx {
            in_win
        } else if in_win < 0.15 {
            in_win / 0.15
        } else if in_win > 0.85 {
            (1.0 - in_win) / 0.15
        } else {
            1.0
        };
        let opacity = opacity.clamp(0.0, 1.0);

        for tc in &self.cells {
            let r = tc.row as i32 + echo.dy;
            let c = tc.col as i32 + echo.dx;
            if r < 0 || r >= self.canvas_rows as i32 || c < 0 || c >= self.canvas_cols as i32 {
                continue;
            }
            let fg = lerp_color(Color::BASE, tc.color, opacity);
            frame.set(r as u16, c as u16, Cell {
                ch: tc.ch,
                fg,
                bg: Color::BASE,
                attrs: Default::default(),
            });
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.cells.clear();
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
