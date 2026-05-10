//! Wide-canvas effect tests with a strict end-state assertion.
//!
//! The integration test uses a 40×120 canvas at an 80% match
//! threshold; this file tightens that to ~100% on a canvas large
//! enough to exercise scale=3 across all effect implementations.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use shedos_screensaver_core::{Color, Frame, Logo};
use shedos_screensaver_effects::{target, EffectCtx, Registry};
use std::path::PathBuf;
use std::time::Duration;

const ROWS: u16 = 40;
const COLS: u16 = 140;
const SEED: u64 = 0xCAFEF00DBADBABE5;
const FRAME_DT: Duration = Duration::from_millis(33);

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

fn real_block_logo() -> Logo {
    // The packaged `block` variant (5 rows × 47 cols).
    Logo::parse(
        "███████ ██   ██ ███████ ██████  ██████  ███████\n\
         ██      ██   ██ ██      ██   ██ ██   ██ ██\n\
         ███████ ███████ █████   ██   ██ ██   ██ ███████\n\
              ██ ██   ██ ██      ██   ██ ██   ██      ██\n\
         ███████ ██   ██ ███████ ██████  ██████  ███████\n",
        PathBuf::from("test"),
    )
}

fn run_effect_to_completion(
    effect: &mut Box<dyn shedos_screensaver_effects::Effect>,
    target_frame: &Frame,
    canvas: &mut Frame,
) -> bool {
    let max_iterations = (effect.duration().as_millis() / FRAME_DT.as_millis()) as usize * 2 + 60;
    let mut done = false;
    for _ in 0..max_iterations {
        done = effect.step(canvas, FRAME_DT, None);
        if done {
            break;
        }
    }
    let _ = target_frame;
    done
}

#[test]
fn every_effect_finishes_with_a_solid_target_no_residue() {
    // Strict end-state: at progress=1.0 the canvas must equal the
    // target exactly (every target cell at the right glyph, no
    // non-target cells lit). Catches lingering trails and partial
    // resolves.
    let registry = Registry::new();
    let logo = real_block_logo();
    let target_frame = target::build_target(50, 200, &logo, Color::WHITE);

    let target_lit: Vec<(u16, u16, char)> = target_frame
        .cells()
        .filter_map(|(r, c, cell)| if cell.ch != ' ' { Some((r, c, cell.ch)) } else { None })
        .collect();

    let mut violations: Vec<String> = Vec::new();
    for key in registry.keys() {
        let mut effect = registry.instantiate(key).unwrap();
        let mut rng = ChaCha8Rng::seed_from_u64(SEED);
        let mut ctx = EffectCtx { final_color: Color::WHITE, rng: &mut rng };
        effect.setup(&target_frame, &mut ctx);

        let mut canvas = Frame::new(50, 200);
        let _ = run_effect_to_completion(&mut effect, &target_frame, &mut canvas);

        // Check 1: every target cell present with the correct glyph.
        let missing: Vec<(u16, u16, char)> = target_lit
            .iter()
            .filter(|(r, c, ch)| canvas.get(*r, *c).map(|cc| cc.ch != *ch).unwrap_or(true))
            .copied()
            .collect();
        // Check 2: no non-target cells lit.
        let extra: Vec<(u16, u16, char)> = canvas
            .cells()
            .filter_map(|(r, c, cell)| {
                if cell.ch == ' ' {
                    return None;
                }
                let in_target = target_frame
                    .get(r, c)
                    .map(|tc| tc.ch == cell.ch)
                    .unwrap_or(false);
                if in_target { None } else { Some((r, c, cell.ch)) }
            })
            .collect();
        if !missing.is_empty() || !extra.is_empty() {
            violations.push(format!(
                "{key}: {} target cells missing, {} non-target cells lit (e.g. {:?})",
                missing.len(),
                extra.len(),
                extra.iter().take(3).collect::<Vec<_>>(),
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "effects don't end with a solid target:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn at_typical_developer_terminal_real_block_logo_completes_cleanly() {
    // The block variant is 5×47. In a 50×200 terminal (a normal
    // wide developer terminal) auto-scale picks 3 → 15×141 rendered.
    let logo = real_block_logo();
    let target_frame = target::build_target(50, 200, &logo, Color::WHITE);
    let scale = target::auto_scale(50, 200, &logo);
    assert_eq!(scale, 3, "expected scale=3 at 50×200 for block; got {scale}");

    let registry = Registry::new();
    let mut failures: Vec<(String, f32)> = Vec::new();
    for key in registry.keys() {
        let mut effect = registry.instantiate(key).unwrap();
        let mut rng = ChaCha8Rng::seed_from_u64(SEED);
        let mut ctx = EffectCtx { final_color: Color::WHITE, rng: &mut rng };
        effect.setup(&target_frame, &mut ctx);

        let mut canvas = Frame::new(50, 200);
        let done = run_effect_to_completion(&mut effect, &target_frame, &mut canvas);
        assert!(done, "effect '{key}' did not complete");

        let total = target_frame.cells().filter(|(_, _, c)| c.ch != ' ').count();
        let matching = target_frame
            .cells()
            .filter(|(r, c, target_cell)| {
                target_cell.ch != ' '
                    && canvas.get(*r, *c).map(|cc| cc.ch == target_cell.ch).unwrap_or(false)
            })
            .count();
        let ratio = matching as f32 / total.max(1) as f32;
        if ratio < 0.99 {
            failures.push((key.to_string(), ratio));
        }
    }
    assert!(
        failures.is_empty(),
        "effects with <99% target match on real block variant scale=3: {failures:?}"
    );
}

#[test]
fn at_scale_3_every_effect_completes_with_full_target() {
    let registry = Registry::new();
    let logo = small_logo();
    let target_frame = target::build_target(ROWS, COLS, &logo, Color::WHITE);

    // Sanity: this canvas + logo should make auto_scale pick at
    // least 2; otherwise we're not exercising scaled rendering.
    let scale = target::auto_scale(ROWS, COLS, &logo);
    assert!(scale >= 2, "expected auto-scale ≥ 2 at canvas {}×{} for logo {}×{}; got {}",
            ROWS, COLS, logo.rows, logo.cols, scale);
    let target_lit = target_frame
        .cells()
        .filter(|(_, _, c)| c.ch != ' ')
        .count();
    let scaled_lit = logo.lit_count() * (scale as usize) * (scale as usize);
    assert_eq!(target_lit, scaled_lit, "target lit count mismatch (cell replication broken?)");

    for key in registry.keys() {
        let mut effect = registry.instantiate(key).unwrap();
        let mut rng = ChaCha8Rng::seed_from_u64(SEED);
        let mut ctx = EffectCtx { final_color: Color::WHITE, rng: &mut rng };
        effect.setup(&target_frame, &mut ctx);

        let mut canvas = Frame::new(ROWS, COLS);
        let max_iterations = (effect.duration().as_millis() / FRAME_DT.as_millis()) as usize * 2 + 60;
        let mut done = false;
        for _ in 0..max_iterations {
            done = effect.step(&mut canvas, FRAME_DT, None);
            if done {
                break;
            }
        }
        assert!(done, "effect '{key}' didn't complete on scale-{scale} canvas");

        // After completion, every lit target cell must have its glyph
        // at the same position in the canvas.
        let total = target_frame
            .cells()
            .filter(|(_, _, c)| c.ch != ' ')
            .count();
        let matching = target_frame
            .cells()
            .filter(|(r, c, target_cell)| {
                target_cell.ch != ' '
                    && canvas.get(*r, *c).map(|cc| cc.ch == target_cell.ch).unwrap_or(false)
            })
            .count();
        let ratio = matching as f32 / total.max(1) as f32;
        assert!(
            ratio >= 0.99,
            "effect '{}' on scale-{} canvas finished but only matches {:.1}% of target glyphs ({}/{}); user sees a half-drawn SHEDOS",
            key, scale, ratio * 100.0, matching, total
        );
    }
}
