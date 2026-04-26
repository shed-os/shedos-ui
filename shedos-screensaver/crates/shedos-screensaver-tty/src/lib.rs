//! Terminal renderer for shedos-screensaver.
//!
//! Two pieces, decoupled so headless tests can drive the renderer
//! without touching the global terminal state:
//!
//! - [`TerminalGuard`] — RAII guard that flips the real stdout into
//!   raw + alt-screen + cursor-hidden on enter and restores everything
//!   on Drop. Live mode constructs one before instantiating a renderer.
//! - [`TtyRenderer<W>`] — generic over any [`std::io::Write`]. Diff-emits
//!   only the cells that changed since the previous frame. Tests pass
//!   a `Vec<u8>` and snapshot the resulting ANSI bytes.

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    style::{Color as CtColor, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor, Attribute},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    QueueableCommand,
};
use shedos_screensaver_core::{CellAttrs, Color, Frame};
use std::io::{self, IsTerminal, Write};

/// RAII handle for live-terminal state. Constructing flips raw mode +
/// alt-screen + hide-cursor on; dropping restores. Drop runs even on
/// panic, so the user's terminal is always cleaned up.
pub struct TerminalGuard {
    raw_was_enabled: bool,
}

impl TerminalGuard {
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        stdout.queue(EnterAlternateScreen)?;
        stdout.queue(Hide)?;
        stdout.flush()?;
        Ok(Self { raw_was_enabled: true })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = stdout.queue(Show);
        let _ = stdout.queue(ResetColor);
        let _ = stdout.queue(LeaveAlternateScreen);
        let _ = stdout.flush();
        if self.raw_was_enabled {
            let _ = disable_raw_mode();
        }
    }
}

/// Convert a `core::Color` into the crossterm `Color::Rgb` truecolor variant.
fn to_ct(c: Color) -> CtColor {
    CtColor::Rgb { r: c.r, g: c.g, b: c.b }
}

/// Generic over the output sink so tests can use `Vec<u8>` while live
/// mode wires `io::stdout().lock()`.
pub struct TtyRenderer<W: Write> {
    out: W,
    shadow: Option<Frame>,
    cols: u16,
    rows: u16,
}

impl<W: Write> TtyRenderer<W> {
    pub fn new(out: W, rows: u16, cols: u16) -> Self {
        Self { out, shadow: None, cols, rows }
    }

    pub fn rows(&self) -> u16 { self.rows }
    pub fn cols(&self) -> u16 { self.cols }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
        self.shadow = None; // force a full repaint after resize
    }

    pub fn into_inner(self) -> W { self.out }

    /// Emit only changed cells from `frame`, batched, with minimal
    /// cursor moves and SGR re-applies.
    pub fn submit(&mut self, frame: &Frame) -> io::Result<()> {
        let mut last_pos: Option<(u16, u16)> = None;
        let mut last_fg: Option<Color> = None;
        let mut last_bg: Option<Color> = None;
        let mut last_attrs: CellAttrs = CellAttrs::NONE;

        for (r, c, cell) in frame.cells() {
            // If shadow exists and the cell is unchanged, skip.
            if let Some(shadow) = &self.shadow {
                if shadow.get(r, c) == Some(cell) {
                    continue;
                }
            }

            // Move cursor if not contiguous with previous emit.
            if last_pos != Some((r, c)) {
                self.out.queue(MoveTo(c, r))?;
            }

            // Re-apply SGR only on change.
            if last_fg != Some(cell.fg) {
                self.out.queue(SetForegroundColor(to_ct(cell.fg)))?;
                last_fg = Some(cell.fg);
            }
            if last_bg != Some(cell.bg) {
                self.out.queue(SetBackgroundColor(to_ct(cell.bg)))?;
                last_bg = Some(cell.bg);
            }
            if cell.attrs != last_attrs {
                // Reset and re-apply only the diff bits we care about.
                self.out.queue(SetAttribute(Attribute::Reset))?;
                self.out.queue(SetForegroundColor(to_ct(cell.fg)))?;
                self.out.queue(SetBackgroundColor(to_ct(cell.bg)))?;
                last_fg = Some(cell.fg);
                last_bg = Some(cell.bg);
                if cell.attrs.contains(CellAttrs::BOLD) {
                    self.out.queue(SetAttribute(Attribute::Bold))?;
                }
                if cell.attrs.contains(CellAttrs::DIM) {
                    self.out.queue(SetAttribute(Attribute::Dim))?;
                }
                if cell.attrs.contains(CellAttrs::ITALIC) {
                    self.out.queue(SetAttribute(Attribute::Italic))?;
                }
                if cell.attrs.contains(CellAttrs::UNDERLINE) {
                    self.out.queue(SetAttribute(Attribute::Underlined))?;
                }
                if cell.attrs.contains(CellAttrs::REVERSE) {
                    self.out.queue(SetAttribute(Attribute::Reverse))?;
                }
                last_attrs = cell.attrs;
            }

            write!(self.out, "{}", cell.ch)?;
            last_pos = Some((r, c + 1));
        }

        self.out.flush()?;
        self.shadow = Some(frame.clone());
        Ok(())
    }

    /// Force a full repaint on the next submit (e.g. after window resize).
    pub fn invalidate(&mut self) {
        self.shadow = None;
    }
}

impl TtyRenderer<Vec<u8>> {
    /// Test helper: peek at the cumulative byte count without consuming
    /// the renderer. Only available when the inner sink is a `Vec<u8>`.
    pub fn bytes_written(&self) -> usize {
        self.out.len()
    }
}

/// Read the actual terminal size; falls back to (24, 80) on error.
pub fn detect_terminal_size() -> (u16, u16) {
    crossterm::terminal::size()
        .map(|(c, r)| (r, c))
        .unwrap_or((24, 80))
}

/// True if stdout looks like an interactive terminal.
pub fn stdout_is_tty() -> bool {
    io::stdout().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shedos_screensaver_core::{Cell, Color, Frame};

    #[test]
    fn first_submit_emits_full_frame() {
        let mut r = TtyRenderer::new(Vec::new(), 2, 2);
        let mut f = Frame::new(2, 2);
        f.set(0, 0, Cell { ch: 'A', fg: Color::WHITE, bg: Color::BLACK, attrs: CellAttrs::NONE });
        r.submit(&f).unwrap();
        let buf = r.into_inner();
        let out = String::from_utf8_lossy(&buf);
        assert!(out.contains('A'));
        // Truecolor SGR for WHITE on BLACK should appear.
        assert!(out.contains("\x1b[38;2;255;255;255m"), "expected fg SGR; got {out:?}");
    }

    #[test]
    fn second_submit_only_emits_diffs() {
        let mut r = TtyRenderer::new(Vec::new(), 1, 5);
        let mut f1 = Frame::new(1, 5);
        for c in 0..5 {
            f1.set(0, c, Cell { ch: 'A', fg: Color::WHITE, bg: Color::BLACK, attrs: CellAttrs::NONE });
        }
        r.submit(&f1).unwrap();
        let len_after_first = r.bytes_written();

        // Same frame: no new bytes.
        r.submit(&f1).unwrap();
        assert_eq!(r.bytes_written(), len_after_first, "identical frame should not emit any bytes");

        // Change one cell.
        let mut f2 = f1.clone();
        f2.set(0, 2, Cell { ch: 'B', fg: Color::WHITE, bg: Color::BLACK, attrs: CellAttrs::NONE });
        r.submit(&f2).unwrap();
        let buf = r.into_inner();
        let delta = &buf[len_after_first..];
        let s = String::from_utf8_lossy(delta);
        assert!(s.contains('B'));
        assert!(!s.contains('A'), "diff-emit should not include unchanged 'A' cells");
    }

    #[test]
    fn invalidate_forces_full_repaint() {
        let mut r = TtyRenderer::new(Vec::new(), 1, 3);
        let mut f = Frame::new(1, 3);
        f.set(0, 0, Cell { ch: 'X', fg: Color::WHITE, bg: Color::BLACK, attrs: CellAttrs::NONE });
        r.submit(&f).unwrap();
        let len = r.bytes_written();
        r.invalidate();
        r.submit(&f).unwrap();
        assert!(r.bytes_written() > len, "invalidate then resubmit should emit again");
    }

}
