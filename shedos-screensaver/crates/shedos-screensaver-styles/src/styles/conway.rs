//! Conway's SHEDOS — Game of Life seeded from the SHEDOS silhouette.
//! Reseeds every `reseed_interval` seconds; rule is parametric
//! (default B3/S23 — classic Life).

use crate::opts::{validate_u32_range, OptType, OptVal, OptionDoc, OptionSchema};
use crate::{Ctx, Style};
use shedos_screensaver_core::{Cell, Color, Frame};

static SCHEMA: OptionSchema = OptionSchema {
    options: &[
        OptionDoc {
            key: "rule",
            ty: OptType::String,
            default: OptVal::String(String::new()), // populated to "B3/S23" via fallback
            desc: "Game of Life rule in B/S notation (e.g. B3/S23)",
            validate: validate_rule,
        },
        OptionDoc {
            key: "reseed_interval",
            ty: OptType::UInt,
            default: OptVal::UInt(30),
            desc: "reseed from logo every N seconds (1..=600)",
            validate: validate_reseed,
        },
    ],
};

fn validate_rule(v: &OptVal) -> Result<(), String> {
    match v {
        OptVal::String(s) if s.is_empty() => Ok(()),
        OptVal::String(s) => {
            // Match `B[0-8]+/S[0-8]+` (case-insensitive on B/S).
            let s = s.to_uppercase();
            if let Some((b, sv)) = s.split_once('/') {
                if let (Some(brest), Some(srest)) = (b.strip_prefix('B'), sv.strip_prefix('S')) {
                    if !brest.is_empty()
                        && !srest.is_empty()
                        && brest.chars().all(|c| c.is_ascii_digit())
                        && srest.chars().all(|c| c.is_ascii_digit())
                    {
                        return Ok(());
                    }
                }
            }
            Err(format!("expected B<n>/S<n> notation; got '{s}'"))
        }
        _ => Err("expected string".into()),
    }
}
fn validate_reseed(v: &OptVal) -> Result<(), String> {
    validate_u32_range(1, 600)(v)
}

#[derive(Clone)]
struct Grid {
    rows: usize,
    cols: usize,
    cells: Vec<bool>,
}

impl Grid {
    fn new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, cells: vec![false; rows * cols] }
    }

    fn idx(&self, r: usize, c: usize) -> usize {
        r * self.cols + c
    }

    fn get(&self, r: i32, c: i32) -> bool {
        if r < 0 || c < 0 || r >= self.rows as i32 || c >= self.cols as i32 {
            return false;
        }
        self.cells[self.idx(r as usize, c as usize)]
    }

    fn set(&mut self, r: usize, c: usize, v: bool) {
        if r < self.rows && c < self.cols {
            let i = self.idx(r, c);
            self.cells[i] = v;
        }
    }
}

#[derive(Default)]
struct Rule {
    born: [bool; 9],
    survive: [bool; 9],
}

impl Rule {
    fn parse(s: &str) -> Self {
        let s = s.to_uppercase();
        let mut r = Rule::default();
        if let Some((b, sv)) = s.split_once('/') {
            if let Some(rest) = b.strip_prefix('B') {
                for d in rest.chars().filter_map(|c| c.to_digit(10)) {
                    if (d as usize) < 9 {
                        r.born[d as usize] = true;
                    }
                }
            }
            if let Some(rest) = sv.strip_prefix('S') {
                for d in rest.chars().filter_map(|c| c.to_digit(10)) {
                    if (d as usize) < 9 {
                        r.survive[d as usize] = true;
                    }
                }
            }
        }
        r
    }

    fn b3s23() -> Self {
        let mut r = Rule::default();
        r.born[3] = true;
        r.survive[2] = true;
        r.survive[3] = true;
        r
    }
}

pub struct Conway {
    grid: Grid,
    next: Grid,
    rule: Rule,
    last_seed_t: f32,
    seeded: bool,
    grid_size: (usize, usize),
}

impl Conway {
    pub fn new() -> Self {
        Self {
            grid: Grid::new(0, 0),
            next: Grid::new(0, 0),
            rule: Rule::b3s23(),
            last_seed_t: 0.0,
            seeded: false,
            grid_size: (0, 0),
        }
    }

    fn reseed(&mut self, ctx: &mut Ctx<'_>) {
        let logo = ctx.logo;
        let r0 = (self.grid.rows.saturating_sub(logo.rows as usize)) / 2;
        let c0 = (self.grid.cols.saturating_sub(logo.cols as usize)) / 2;
        for r in 0..logo.rows as usize {
            for c in 0..logo.cols as usize {
                if logo.lit(r, c) {
                    self.grid.set(r0 + r, c0 + c, true);
                }
            }
        }
    }
}

impl Default for Conway {
    fn default() -> Self {
        Self::new()
    }
}

impl Style for Conway {
    fn name(&self) -> &'static str { "conway" }
    fn title(&self) -> &'static str { "Conway's SHEDOS" }
    fn default_color(&self) -> Color { Color::rgb(0xfa, 0xb3, 0x87) }
    fn option_schema(&self) -> &'static OptionSchema { &SCHEMA }
    fn wallpaper_alpha(&self) -> f32 { 0.9 }

    fn draw(&mut self, frame: &mut Frame, ctx: &mut Ctx<'_>) {
        let target = (frame.rows() as usize, frame.cols() as usize);
        if self.grid_size != target {
            self.grid = Grid::new(target.0, target.1);
            self.next = Grid::new(target.0, target.1);
            self.grid_size = target;
            self.seeded = false;
        }

        let rule_str = ctx.opts.get_str("rule").unwrap_or("");
        if !rule_str.is_empty() {
            self.rule = Rule::parse(rule_str);
        }
        let reseed_interval = ctx.opts.get_u32("reseed_interval").unwrap_or(30) as f32;

        if !self.seeded {
            self.reseed(ctx);
            self.last_seed_t = ctx.t.as_secs_f32();
            self.seeded = true;
        }

        if ctx.t.as_secs_f32() - self.last_seed_t >= reseed_interval {
            // Don't fully overwrite; just stamp the logo on top of the
            // existing field so the reseed is visible without a full
            // wipe.
            self.reseed(ctx);
            self.last_seed_t = ctx.t.as_secs_f32();
        }

        // One Life step per frame. Fast enough for 30 fps in Python;
        // trivially fast in Rust for terminal-scale grids.
        for r in 0..self.grid.rows {
            for c in 0..self.grid.cols {
                let mut neigh = 0;
                for dr in -1..=1 {
                    for dc in -1..=1 {
                        if dr == 0 && dc == 0 {
                            continue;
                        }
                        if self.grid.get(r as i32 + dr, c as i32 + dc) {
                            neigh += 1;
                        }
                    }
                }
                let alive = self.grid.get(r as i32, c as i32);
                let next_alive = if alive {
                    self.rule.survive[neigh]
                } else {
                    self.rule.born[neigh]
                };
                self.next.set(r, c, next_alive);
            }
        }
        std::mem::swap(&mut self.grid, &mut self.next);

        // Render: lit cells = full block in the chosen color.
        for r in 0..self.grid.rows {
            for c in 0..self.grid.cols {
                if self.grid.get(r as i32, c as i32) {
                    frame.set(r as u16, c as u16, Cell {
                        ch: '█',
                        fg: ctx.color,
                        bg: Color::BASE,
                        attrs: Default::default(),
                    });
                }
            }
        }
    }
}
