//! Animation styles for the ShedOS screensaver.
//!
//! Each style implements [`Style`] and is registered into [`Registry`].
//! The CLI iterates [`Registry::keys`] for `--list`, looks up the
//! per-style [`OptionSchema`] for `--help-style`, and asks the
//! registry for a fresh [`Style`] instance to drive the frame loop.

pub mod opts;
pub mod registry;
pub mod styles;

pub use opts::{OptType, OptVal, OptionDoc, OptionSchema, OptionSetError, StyleOpts};
pub use registry::{Registry, StyleFactory};
pub use shedos_screensaver_core::{AudioFrame, Color, Frame, Logo};

use rand_chacha::ChaCha8Rng;
use std::time::Duration;

/// Per-frame context handed to [`Style::draw`].
pub struct Ctx<'a> {
    /// Total time elapsed since the style started rendering.
    pub t: Duration,
    /// Time since the previous frame.
    pub dt: Duration,
    /// Resolved color for this style instance.
    pub color: Color,
    /// Logo (parsed from /etc/shedos-ascii.txt).
    pub logo: &'a Logo,
    /// Resolved per-style options (CLI + config file merged with style defaults).
    pub opts: &'a StyleOpts,
    /// Deterministic RNG. Live mode seeds from process entropy; tests
    /// seed from a fixed value so snapshots are stable.
    pub rng: &'a mut ChaCha8Rng,
    /// Audio analysis frame, when `--audio-source` is enabled and
    /// pipewire is reachable; `None` otherwise. Styles that opted in
    /// via `wants_audio()` should drive their visuals from this when
    /// present and fall back to time-only otherwise.
    pub audio: Option<&'a AudioFrame>,
}

/// One animation style.
pub trait Style: Send {
    fn name(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn default_color(&self) -> Color;
    fn option_schema(&self) -> &'static OptionSchema;

    /// Whether this style can usefully consume an audio frame
    /// (stage 5 wires this; styles that return false are never
    /// fed an audio frame).
    fn wants_audio(&self) -> bool {
        false
    }

    /// In Wayland mode, how transparent the foreground style is over
    /// the wallpaper backdrop. 1.0 = fully opaque (wallpaper hidden).
    fn wallpaper_alpha(&self) -> f32 {
        1.0
    }

    /// One-time setup. Called once before the first `draw` call;
    /// when the canvas resizes (e.g. terminal window resize), the
    /// runner instantiates a fresh style instead of calling setup
    /// again, so styles may assume the dimensions don't change.
    fn setup(&mut self, _frame: &Frame, _ctx: &mut Ctx<'_>) {}

    /// Render one frame into the provided canvas. `ctx` is taken
    /// by mutable reference so styles can advance the RNG.
    fn draw(&mut self, frame: &mut Frame, ctx: &mut Ctx<'_>);
}
