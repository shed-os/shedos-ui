//! Insta golden-snapshot tests for every style.
//!
//! Each test:
//! 1. Constructs a fixed-seed ChaCha8Rng + a deterministic Logo.
//! 2. Drives the style for N frames, writing each frame into a
//!    deterministic textual digest (glyph + RGB triple per cell).
//! 3. Asserts the digest matches the committed snapshot.
//!
//! The snapshots live in tests/snapshots/. After an intentional
//! visual change, re-record with `INSTA_UPDATE=always cargo test
//! --package shedos-screensaver-styles --test snapshot`.
//!
//! We deliberately do NOT route through TtyRenderer here — the
//! snapshot's "look" is the cell grid itself, not the SGR escape
//! sequence representation. Decoupling means snapshots survive
//! crossterm version bumps that change SGR phrasing.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use shedos_screensaver_core::{Color, Frame, Logo};
use shedos_screensaver_styles::{Ctx, Registry, StyleOpts};
use std::path::PathBuf;
use std::time::Duration;

const ROWS: u16 = 12;
const COLS: u16 = 60;
const SEED: u64 = 0xDEADBEEFCAFEBABE;

fn shedos_logo() -> Logo {
    // Mirror of /etc/shedos-ascii.txt — kept inline so the test
    // doesn't depend on the runtime path.
    let art = "███████ ██   ██ ███████ ██████  ██████  ███████\n\
               ██      ██   ██ ██      ██   ██ ██   ██ ██\n\
               ███████ ███████ █████   ██   ██ ██   ██ ███████\n\
                    ██ ██   ██ ██      ██   ██ ██   ██      ██\n\
               ███████ ██   ██ ███████ ██████  ██████  ███████\n";
    Logo::parse(art, PathBuf::from("test"))
}

/// Convert a frame to a compact deterministic digest:
/// each row → "{glyph}@{r:02x}{g:02x}{b:02x}" per cell, separated by space.
fn digest(frame: &Frame) -> String {
    let mut out = String::new();
    for r in 0..frame.rows() {
        for c in 0..frame.cols() {
            let cell = frame.get(r, c).unwrap();
            // Hide pure-default cells to keep snapshots compact.
            let default_fg = Color::TEXT;
            let default_bg = Color::BASE;
            if cell.ch == ' ' && cell.fg == default_fg && cell.bg == default_bg {
                out.push(' ');
            } else {
                out.push(cell.ch);
            }
        }
        out.push('\n');
    }
    out
}

fn run_n_frames(style_key: &str, n: usize) -> String {
    let registry = Registry::new();
    let mut style = registry.instantiate(style_key)
        .unwrap_or_else(|| panic!("style '{style_key}' not in registry"));
    let logo = shedos_logo();
    let opts = StyleOpts::from_defaults(style.option_schema());
    let mut rng = ChaCha8Rng::seed_from_u64(SEED);
    let color = style.default_color();
    let mut frame = Frame::new(ROWS, COLS);

    let mut all = String::new();
    let mut t = Duration::ZERO;
    let dt = Duration::from_millis(33); // ~30 fps

    for i in 0..n {
        frame.clear();
        let mut ctx = Ctx { t, dt, color, logo: &logo, opts: &opts, rng: &mut rng };
        style.draw(&mut frame, &mut ctx);
        all.push_str(&format!("--- frame {i} (t={}ms) ---\n", t.as_millis()));
        all.push_str(&digest(&frame));
        t += dt;
    }
    all
}

#[test]
fn snapshot_logo_bounce() {
    insta::assert_snapshot!(run_n_frames("logo-bounce", 5));
}

#[test]
fn snapshot_matrix() {
    insta::assert_snapshot!(run_n_frames("matrix", 5));
}

#[test]
fn snapshot_plasma() {
    insta::assert_snapshot!(run_n_frames("plasma", 5));
}

#[test]
fn snapshot_starfield() {
    insta::assert_snapshot!(run_n_frames("starfield", 5));
}

#[test]
fn snapshot_conway() {
    insta::assert_snapshot!(run_n_frames("conway", 5));
}

#[test]
fn snapshot_tunnel() {
    insta::assert_snapshot!(run_n_frames("tunnel", 5));
}

#[test]
fn snapshot_waves() {
    insta::assert_snapshot!(run_n_frames("waves", 5));
}

#[test]
fn snapshot_mandala() {
    insta::assert_snapshot!(run_n_frames("mandala", 5));
}
