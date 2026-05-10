//! Animation effects that form an ASCII-art target into existence
//! on a canvas.
//!
//! Start with a blank canvas and a target Frame (the desired final
//! state, e.g. a SHEDOS variant). The effect drives the canvas from
//! blank to target through intermediate frames.
//!
//! Every effect implements [`Effect`]; the catalog lives in
//! [`registry::Registry`]. New effects drop into `effects/<name>.rs`
//! and register in [`Registry::new`].

pub mod easing;
pub mod effects;
pub mod registry;
pub mod target;

pub use registry::{EffectFactory, Registry};
pub use shedos_screensaver_core::{AudioFrame, Cell, Color, Frame};

use rand_chacha::ChaCha8Rng;
use std::time::Duration;

/// Per-effect context handed to setup() and step().
pub struct EffectCtx<'a> {
    /// Color the resolved art lands on. Effects pick a transient
    /// color separately.
    pub final_color: Color,
    /// Deterministic RNG (process-seeded in live mode, fixed-seed
    /// in tests). Use this, not thread_rng.
    pub rng: &'a mut ChaCha8Rng,
}

/// The contract every animation effect implements.
///
/// Lifecycle:
/// 1. Construct via factory (`Registry::instantiate`).
/// 2. `setup(target, ctx)`: capture target, plan animation.
/// 3. `step(frame, dt, audio)` repeatedly until it returns `true`.
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

    /// True if audio input enhances this effect.
    fn reactive(&self) -> bool {
        false
    }

    /// Called once before the first `step`. The effect saves a copy
    /// of the target and plans its animation.
    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>);

    /// Advance the animation by `dt` and write the current state into
    /// `frame`. Returns `true` once the canvas equals the target.
    /// Subsequent step calls remain no-ops.
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
