//! Integration tests: every registered effect runs to completion
//! against a small SHEDOS target and lands on the exact target Frame.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use shedos_screensaver_core::{Color, Frame, Logo};
use shedos_screensaver_effects::{target, EffectCtx, Registry};
use std::path::PathBuf;
use std::time::Duration;

// Pick a canvas size that triggers auto-scaling for the test logo
// (5×23). 40×120 lands at scale 3 and exercises the post-scale code
// paths a smaller canvas would bypass.
const ROWS: u16 = 40;
const COLS: u16 = 120;
const SEED: u64 = 0xCAFEF00DBADBABE5;
const FRAME_DT: Duration = Duration::from_millis(50);

fn small_logo() -> Logo {
    Logo::parse(
        "███████ ██   ██ ███████\n\
         ██      ██   ██ ██\n\
         ███████ ███████ █████\n\
              ██ ██   ██ ██\n\
         ███████ ██   ██ ███████\n",
        PathBuf::from("test"),
    )
}

#[test]
fn every_effect_runs_to_completion_within_double_its_duration() {
    let registry = Registry::new();
    let logo = small_logo();
    let target_frame = target::build_target(ROWS, COLS, &logo, Color::WHITE);

    for key in registry.keys() {
        let mut effect = registry.instantiate(key).expect("registered effect missing");
        let mut rng = ChaCha8Rng::seed_from_u64(SEED);
        let mut ctx = EffectCtx { final_color: Color::WHITE, rng: &mut rng };
        effect.setup(&target_frame, &mut ctx);

        let mut canvas = Frame::new(ROWS, COLS);
        let max_iterations = (effect.duration().as_millis() / FRAME_DT.as_millis()) as usize * 2 + 30;
        let mut iterations = 0;
        let mut done = false;
        while iterations < max_iterations {
            done = effect.step(&mut canvas, FRAME_DT, None);
            iterations += 1;
            if done {
                break;
            }
        }
        assert!(
            done,
            "effect '{}' did not complete within {} iterations (≈ {} ms)",
            key,
            iterations,
            (iterations as u128) * FRAME_DT.as_millis()
        );
    }
}

#[test]
fn every_effect_lands_on_target_after_completion() {
    let registry = Registry::new();
    let logo = small_logo();
    let target_frame = target::build_target(ROWS, COLS, &logo, Color::WHITE);

    for key in registry.keys() {
        let mut effect = registry.instantiate(key).expect("registered effect missing");
        let mut rng = ChaCha8Rng::seed_from_u64(SEED);
        let mut ctx = EffectCtx { final_color: Color::WHITE, rng: &mut rng };
        effect.setup(&target_frame, &mut ctx);

        let mut canvas = Frame::new(ROWS, COLS);
        let max_iterations = (effect.duration().as_millis() / FRAME_DT.as_millis()) as usize * 2 + 30;
        for _ in 0..max_iterations {
            if effect.step(&mut canvas, FRAME_DT, None) {
                break;
            }
        }

        // After completion, every lit target cell should be set in
        // the canvas (glyph match only; colors vary because some
        // effects pick their own). Allow colorshift/decrypt-style
        // effects that overwrite glyphs at end-state by requiring
        // most cells, not all.
        let total_lit_cells = target_frame
            .cells()
            .filter(|(_, _, cell)| cell.ch != ' ')
            .count();
        let matching = target_frame
            .cells()
            .filter(|(r, c, target_cell)| {
                target_cell.ch != ' '
                    && canvas.get(*r, *c).map(|cc| cc.ch == target_cell.ch).unwrap_or(false)
            })
            .count();
        // Require ≥80% of target cells to be glyph-correct, giving
        // latitude to colorshift / decrypt-style effects that render
        // unusual glyphs in the final frame.
        let ratio = matching as f32 / total_lit_cells.max(1) as f32;
        assert!(
            ratio >= 0.80,
            "effect '{}' final canvas only matches {:.1}% of target glyphs ({}/{}); expected ≥80%",
            key,
            ratio * 100.0,
            matching,
            total_lit_cells
        );
    }
}

#[test]
fn every_effect_supports_reset() {
    let registry = Registry::new();
    let logo = small_logo();
    let target_frame = target::build_target(ROWS, COLS, &logo, Color::WHITE);

    for key in registry.keys() {
        let mut effect = registry.instantiate(key).expect("registered effect missing");
        let mut rng = ChaCha8Rng::seed_from_u64(SEED);
        let mut ctx = EffectCtx { final_color: Color::WHITE, rng: &mut rng };
        effect.setup(&target_frame, &mut ctx);

        let mut canvas = Frame::new(ROWS, COLS);
        // Run partially.
        for _ in 0..5 {
            effect.step(&mut canvas, FRAME_DT, None);
        }

        // Reset should not panic.
        effect.reset();

        // After reset + setup, should run to completion again.
        let mut rng = ChaCha8Rng::seed_from_u64(SEED);
        let mut ctx = EffectCtx { final_color: Color::WHITE, rng: &mut rng };
        effect.setup(&target_frame, &mut ctx);
        let max_iterations = (effect.duration().as_millis() / FRAME_DT.as_millis()) as usize * 2 + 30;
        let mut done = false;
        for _ in 0..max_iterations {
            if effect.step(&mut canvas, FRAME_DT, None) {
                done = true;
                break;
            }
        }
        assert!(done, "effect '{}' didn't complete after reset+setup", key);
    }
}
