//! circuit-trace — bright traces draw between consecutive SHEDOS
//! cells in a greedy nearest-neighbor traversal. The cell at the
//! end of each trace lights up in target color; intermediate
//! trace cells render as PCB-style dots and disappear by the time
//! the duration ends.

use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::collections::HashSet;
use std::time::Duration;

const DURATION_MS: u64 = 5_000;
const TRACE_GLYPH: char = '·';
const TRACE_COLOR: Color = Color::rgb(0x94, 0xe2, 0xd5);
/// Each trace segment's draw window; smaller = snappier.
const SEGMENT_NORM: f32 = 0.04;
/// All segments must finish drawing by this normalized time so the
/// last cell has time to settle to its target color.
const MATERIALIZE_END: f32 = 0.96;

#[derive(Clone, Copy)]
struct Endpoint {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
    /// Time the cell materializes (i.e. its trace from the previous
    /// endpoint completes). Cell renders as target from this time
    /// onward.
    materialize_t: f32,
}

#[derive(Clone, Copy)]
struct PathCell {
    row: u16,
    col: u16,
    /// Window during which the trace cell is visible.
    on_t: f32,
    off_t: f32,
}

pub struct CircuitTrace {
    endpoints: Vec<Endpoint>,
    path_cells: Vec<PathCell>,
    elapsed: Duration,
}

impl CircuitTrace {
    pub fn new() -> Self {
        Self { endpoints: Vec::new(), path_cells: Vec::new(), elapsed: Duration::ZERO }
    }
}

impl Default for CircuitTrace {
    fn default() -> Self { Self::new() }
}

impl Effect for CircuitTrace {
    fn name(&self) -> &'static str { "circuit-trace" }
    fn title(&self) -> &'static str { "Circuit Trace" }
    fn description(&self) -> &'static str {
        "Bright PCB-style traces draw between consecutive SHEDOS cells in a greedy nearest-neighbor traversal; trace cells fade after the connecting cell materializes."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.endpoints.clear();
        self.path_cells.clear();
        self.elapsed = Duration::ZERO;

        // Collect lit cells.
        let mut remaining: Vec<(u16, u16, char, Color)> = Vec::new();
        for (r, c, cell) in target.cells() {
            if cell.ch == ' ' {
                continue;
            }
            remaining.push((r, c, cell.ch, cell.fg));
        }
        if remaining.is_empty() {
            return;
        }

        // Greedy nearest-neighbor traversal starting at the
        // top-left-most lit cell.
        remaining.sort_by_key(|a| (a.0, a.1));
        let mut ordered: Vec<(u16, u16, char, Color)> = Vec::with_capacity(remaining.len());
        ordered.push(remaining.remove(0));
        while !remaining.is_empty() {
            let last = *ordered.last().unwrap();
            let (idx, _) = remaining
                .iter()
                .enumerate()
                .min_by_key(|(_, &(r, c, _, _))| {
                    let dr = (r as i32 - last.0 as i32).abs();
                    let dc = (c as i32 - last.1 as i32).abs();
                    (dr + dc, r, c)
                })
                .unwrap();
            ordered.push(remaining.remove(idx));
        }

        // Lay out timing across the first MATERIALIZE_END of the duration.
        let n = ordered.len();
        let lit_set: HashSet<(u16, u16)> = ordered.iter().map(|c| (c.0, c.1)).collect();
        let span = MATERIALIZE_END;
        let cell_spacing = span / (n as f32);

        for (i, &(r, c, ch, color)) in ordered.iter().enumerate() {
            let materialize_t = ((i as f32) + 1.0) * cell_spacing;
            self.endpoints.push(Endpoint { row: r, col: c, ch, color, materialize_t });
            if i == 0 {
                continue;
            }
            // Trace from previous endpoint to current. Manhattan path,
            // vertical leg first.
            let (pr, pc) = (ordered[i - 1].0, ordered[i - 1].1);
            let trace_window_start = (i as f32) * cell_spacing;
            let trace_window_end = ((i as f32) + 1.0) * cell_spacing;
            let path = manhattan_path(pr, pc, r, c);
            // Path hides any cell that's also a lit endpoint; those
            // are owned by the materialization timeline.
            for (pr_t, pc_t) in path {
                if lit_set.contains(&(pr_t, pc_t)) {
                    continue;
                }
                self.path_cells.push(PathCell {
                    row: pr_t,
                    col: pc_t,
                    on_t: trace_window_start,
                    // Trace persists briefly past its segment, then vanishes
                    // before MATERIALIZE_END so the final frame is clean.
                    off_t: (trace_window_end + SEGMENT_NORM * 4.0).min(MATERIALIZE_END),
                });
            }
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        // Render trace cells (only during their on/off window).
        for p in &self.path_cells {
            if progress >= p.on_t && progress < p.off_t {
                frame.set(p.row, p.col, Cell {
                    ch: TRACE_GLYPH,
                    fg: TRACE_COLOR,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        // Render endpoints (at target color from materialize_t onward).
        for e in &self.endpoints {
            if progress >= e.materialize_t {
                frame.set(e.row, e.col, Cell {
                    ch: e.ch,
                    fg: e.color,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        }

        progress >= 1.0
    }

    fn reset(&mut self) {
        self.endpoints.clear();
        self.path_cells.clear();
        self.elapsed = Duration::ZERO;
    }
}

fn manhattan_path(r0: u16, c0: u16, r1: u16, c1: u16) -> Vec<(u16, u16)> {
    let mut path = Vec::new();
    let mut r = r0 as i32;
    let mut c = c0 as i32;
    while r != r1 as i32 {
        if r < r1 as i32 {
            r += 1;
        } else {
            r -= 1;
        }
        path.push((r as u16, c as u16));
    }
    while c != c1 as i32 {
        if c < c1 as i32 {
            c += 1;
        } else {
            c -= 1;
        }
        path.push((r as u16, c as u16));
    }
    // Drop the final cell; it's the destination endpoint, rendered
    // through the materialize timeline.
    if !path.is_empty() {
        path.pop();
    }
    path
}
