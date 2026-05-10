//! flashlight — a circular cone of light moves diagonally from
//! top-left to bottom-right; cells reveal in its wake. Cells
//! currently inside the cone render bright (white→target);
//! cells the cone has passed render at target color.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 4_500;
/// Cone radius in cells.
const CONE_R: f32 = 12.0;

#[derive(Clone, Copy)]
struct LightCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// Time at which the cone's leading edge first reaches the cell.
    reveal_t: f32,
}

pub struct Flashlight {
    cells: Vec<LightCell>,
    canvas_cols: f32,
    canvas_rows: f32,
    elapsed: Duration,
}

impl Flashlight {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            canvas_cols: 0.0,
            canvas_rows: 0.0,
            elapsed: Duration::ZERO,
        }
    }
}

impl Default for Flashlight {
    fn default() -> Self { Self::new() }
}

impl Effect for Flashlight {
    fn name(&self) -> &'static str { "flashlight" }
    fn title(&self) -> &'static str { "Flashlight" }
    fn description(&self) -> &'static str {
        "A circular cone of light moves diagonally from top-left to bottom-right; cells inside the cone flash bright, cells past it settle to target color."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.canvas_cols = target.cols() as f32;
        self.canvas_rows = target.rows() as f32;
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            let reveal_t = compute_reveal_t(
                c as f32,
                r as f32,
                self.canvas_cols,
                self.canvas_rows,
                CONE_R,
            );
            self.cells.push(LightCell {
                row: r,
                col: c,
                ch: cell.ch,
                color: cell.fg,
                reveal_t,
            });
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        // Light position at current progress.
        let light_x = -CONE_R + progress * (self.canvas_cols + 2.0 * CONE_R);
        let light_y = -CONE_R + progress * (self.canvas_rows + 2.0 * CONE_R);

        frame.clear();

        for c in &self.cells {
            if progress < c.reveal_t {
                continue;
            }
            // Compute current distance from light to cell.
            let dx = c.col as f32 - light_x;
            let dy = c.row as f32 - light_y;
            let dist = (dx * dx + dy * dy).sqrt();
            let fg = if dist < CONE_R {
                // Inside cone: bright white at center, blending to
                // target color at edges.
                let edge_t = (dist / CONE_R).clamp(0.0, 1.0);
                lerp_color(Color::rgb(0xff, 0xff, 0xff), c.color, edge_t)
            } else {
                c.color
            };
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
        self.cells.clear();
        self.elapsed = Duration::ZERO;
    }
}

/// Solve the quadratic `|cell - light(t)|² = R²` for the smaller
/// root — i.e., the time the light cone first reaches the cell.
fn compute_reveal_t(cx: f32, cy: f32, cols: f32, rows: f32, r: f32) -> f32 {
    let a = cx + r;
    let b = cy + r;
    let u = cols + 2.0 * r;
    let v = rows + 2.0 * r;
    let aa = u * u + v * v;
    let bb = a * u + b * v;
    let cc = a * a + b * b - r * r;
    if aa <= 0.0 {
        return 0.95;
    }
    let disc = bb * bb - aa * cc;
    if disc < 0.0 {
        return 0.95;
    }
    let t = (bb - disc.sqrt()) / aa;
    t.clamp(0.0, 0.95)
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::rgb(
        ((a.r as f32) * (1.0 - t) + (b.r as f32) * t) as u8,
        ((a.g as f32) * (1.0 - t) + (b.g as f32) * t) as u8,
        ((a.b as f32) * (1.0 - t) + (b.b as f32) * t) as u8,
    )
}
