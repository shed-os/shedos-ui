//! boot-sequence — fake terminal boot log scrolls up the canvas
//! (`[ OK ] Loading kernel modules…` etc.). After ~12 lines the
//! log says `[ OK ] Initializing SHEDOS animated screensaver…`
//! and the SHEDOS art emerges from the bottom, displacing the log
//! lines off the top. Fades into the resolved target by the end.

use crate::easing;
use crate::{AudioFrame, Cell, Color, Effect, EffectCtx, Frame};
use std::time::Duration;

const DURATION_MS: u64 = 7_000;
const LOG_PHASE_END: f32 = 0.65;
const REVEAL_PHASE_END: f32 = 0.95;

const BOOT_LINES: &[&str] = &[
    "[    0.000000] Linux version 6.19.14-shedos (gcc-15.1)",
    "[    0.001234] CPU: x86_64 family 6 model 167",
    "[    0.002104] ACPI: BIOS bug detected — quirk applied",
    "[    0.003456] systemd[1]: Detected architecture x86-64",
    "[ OK ] Mounted /dev",
    "[ OK ] Mounted /sys/kernel/security",
    "[ OK ] Mounted /tmp (tmpfs, size=4G)",
    "[ OK ] Started Hyprland Wayland Compositor",
    "[ OK ] Started PipeWire Multimedia Service",
    "[ OK ] Started shedman update daemon",
    "[ OK ] Reached target Multi-User System",
    "[ OK ] Initializing SHEDOS animated screensaver…",
];

const LOG_DIM: Color = Color::rgb(0x55, 0xaa, 0x88);
const LOG_BRIGHT: Color = Color::rgb(0x88, 0xff, 0xbb);

#[derive(Clone, Copy)]
struct TargetCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}

pub struct BootSequence {
    cells: Vec<TargetCell>,
    elapsed: Duration,
    rows: u16,
    cols: u16,
}

impl BootSequence {
    pub fn new() -> Self {
        Self { cells: Vec::new(), elapsed: Duration::ZERO, rows: 0, cols: 0 }
    }
}

impl Default for BootSequence {
    fn default() -> Self { Self::new() }
}

impl Effect for BootSequence {
    fn name(&self) -> &'static str { "boot-sequence" }
    fn title(&self) -> &'static str { "Boot Sequence" }
    fn description(&self) -> &'static str {
        "Fake terminal boot log scrolls up; the last line is `Initializing SHEDOS…` and the art emerges from below."
    }
    fn duration(&self) -> Duration { Duration::from_millis(DURATION_MS) }

    fn setup(&mut self, target: &Frame, _ctx: &mut EffectCtx<'_>) {
        self.cells.clear();
        self.elapsed = Duration::ZERO;
        self.rows = target.rows();
        self.cols = target.cols();
        for (r, c, cell) in target.cells() {
            if cell.ch != ' ' {
                self.cells.push(TargetCell { row: r, col: c, ch: cell.ch, color: cell.fg });
            }
        }
    }

    fn step(&mut self, frame: &mut Frame, dt: Duration, _audio: Option<&AudioFrame>) -> bool {
        self.elapsed += dt;
        let total = Duration::from_millis(DURATION_MS).as_secs_f32();
        let progress = (self.elapsed.as_secs_f32() / total).min(1.0);

        frame.clear();

        if progress < LOG_PHASE_END {
            // Boot log scroll — show lines proportional to progress.
            let log_progress = progress / LOG_PHASE_END;
            let lines_shown = (log_progress * BOOT_LINES.len() as f32).ceil() as usize;
            let lines_shown = lines_shown.min(BOOT_LINES.len());
            // Show the most recent `min(lines_shown, rows)` lines at
            // the bottom of the canvas; older lines (if any) are
            // already scrolled off the top.
            let visible = lines_shown.min(self.rows as usize);
            let start_idx = lines_shown.saturating_sub(visible);
            for (i, line) in BOOT_LINES[start_idx..lines_shown].iter().enumerate() {
                let r = (self.rows as usize - visible + i) as u16;
                let is_latest = (start_idx + i) == lines_shown - 1;
                let fg = if is_latest { LOG_BRIGHT } else { LOG_DIM };
                write_line(frame, r, line, fg);
            }
            // Blinking cursor at the end of the latest line.
            let blink = (self.elapsed.as_millis() / 250) % 2 == 0;
            if blink && lines_shown > 0 {
                let last_line = BOOT_LINES[lines_shown - 1];
                let r = (self.rows as usize - 1) as u16;
                let cursor_col = (last_line.chars().count() as u16).min(self.cols.saturating_sub(1));
                frame.set(r, cursor_col, Cell {
                    ch: '█',
                    fg: LOG_BRIGHT,
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
        } else if progress < REVEAL_PHASE_END {
            // SHEDOS emerges from below; log lines scroll off the top.
            let local = (progress - LOG_PHASE_END) / (REVEAL_PHASE_END - LOG_PHASE_END);
            let eased = easing::ease_out_cubic(local);
            // Reveal the SHEDOS art by mixing brightness from 0 → 1.
            let intensity = eased;
            for c in &self.cells {
                let r_ch = (c.color.r as f32 * intensity) as u8;
                let g_ch = (c.color.g as f32 * intensity) as u8;
                let b_ch = (c.color.b as f32 * intensity) as u8;
                frame.set(c.row, c.col, Cell {
                    ch: c.ch,
                    fg: Color::rgb(r_ch, g_ch, b_ch),
                    bg: Color::BASE,
                    attrs: Default::default(),
                });
            }
            // Faded-out log fragment near the top during transition.
            let fade = 1.0 - eased;
            if fade > 0.1 {
                let last_line = BOOT_LINES[BOOT_LINES.len() - 1];
                let r = 0u16;
                let r_ch = (LOG_DIM.r as f32 * fade) as u8;
                let g_ch = (LOG_DIM.g as f32 * fade) as u8;
                let b_ch = (LOG_DIM.b as f32 * fade) as u8;
                write_line(frame, r, last_line, Color::rgb(r_ch, g_ch, b_ch));
            }
        } else {
            // Settled — full SHEDOS at target color.
            for c in &self.cells {
                frame.set(c.row, c.col, Cell {
                    ch: c.ch,
                    fg: c.color,
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

fn write_line(frame: &mut Frame, row: u16, text: &str, fg: Color) {
    if row >= frame.rows() {
        return;
    }
    for (i, ch) in text.chars().enumerate() {
        let col = i as u16;
        if col >= frame.cols() {
            break;
        }
        frame.set(row, col, Cell {
            ch,
            fg,
            bg: Color::BASE,
            attrs: Default::default(),
        });
    }
}
