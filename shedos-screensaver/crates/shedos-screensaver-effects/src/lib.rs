//! Animation effects that *form* an ASCII-art target into existence
//! on a canvas.
//!
//! The model: you start with a blank canvas and a target Frame
//! (the desired final state — e.g. a SHEDOS variant rendered at the
//! canvas size), and the effect's job is to drive the canvas from
//! blank → target through some visually interesting intermediate
//! frames.
//!
//! Every effect implements [`Effect`]; the catalog of registered
//! effects lives in [`registry::Registry`]. New effects drop into
//! `effects/<name>.rs`, register in [`registry::Registry::new`],
//! and are picked up automatically by `--effect random` and the
//! shuffle engine.

pub mod easing;
pub mod effects;
pub mod registry;
pub mod target;

pub use registry::{EffectFactory, Registry};
pub use shedos_screensaver_core::{AudioFrame, Cell, Color, Frame};

use rand_chacha::ChaCha8Rng;
use std::time::Duration;

/// Per-effect context handed to setup() and step(). Effects use it
/// to pick transient colors, seed RNG, and consume audio.
pub struct EffectCtx<'a> {
    /// Color the resolved art should land on. Effects pick a
    /// transient/animated color separately.
    pub final_color: Color,
    /// Deterministic RNG (process-seeded in live mode, fixed-seed
    /// in tests). Effects should use this rather than thread_rng.
    pub rng: &'a mut ChaCha8Rng,
}

/// The contract every animation effect implements.
///
/// Lifecycle:
/// 1. Construct via factory (`Registry::instantiate`).
/// 2. `setup(target, ctx)` — capture the target Frame, plan animation.
/// 3. `step(frame, dt, audio)` repeatedly until it returns `true`.
///    Each call updates the canvas in place toward the target.
/// 4. `reset()` to start over for `--shuffle` rotations.
pub trait Effect: Send {
    /// Stable kebab-case identifier (used by `--effect=NAME`).
    fn name(&self) -> &'static str;

    /// Human-readable title for `--list`.
    fn title(&self) -> &'static str;

    /// One-line description for `--list-effects`.
    fn description(&self) -> &'static str;

    /// Approximate animation length. The engine uses this for
    /// `--shuffle` and `--duration` planning.
    fn duration(&self) -> Duration;

    /// True if this effect's reactivity is enhanced when audio is fed in.
    /// (All effects work without audio; some look better with it.)
    fn reactive(&self) -> bool {
        false
    }

    /// Called once before the first `step`. The effect saves a copy
    /// of the target and plans its animation.
    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>);

    /// Advance the animation by `dt` and write the current state
    /// into `frame`. Returns `true` once the canvas equals the target
    /// (the effect has finished). After the first `true`, additional
    /// step calls should remain no-ops or keep returning true.
    fn step(
        &mut self,
        frame: &mut Frame,
        dt: Duration,
        audio: Option<&AudioFrame>,
    ) -> bool;

    /// Reset to pre-setup state. The next `setup` call may reuse
    /// allocated buffers; reset clears progress only.
    fn reset(&mut self);
}
