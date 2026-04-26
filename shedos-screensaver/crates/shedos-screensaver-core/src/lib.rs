//! Foundation types shared by every screensaver crate.
//!
//! No I/O beyond reading the logo file; no terminal or Wayland deps.
//! Backends and styles depend on this crate; this crate depends on
//! nothing outside `std`.

pub mod audio;
pub mod catppuccin;
pub mod clock;
pub mod color;
pub mod frame;
pub mod logo;
pub mod signals;

pub use audio::{AudioFrame, NUM_BANDS};
pub use catppuccin::Catppuccin;
pub use clock::{Clock, MockClock, RealClock};
pub use color::{Color, ColorParseError};
pub use frame::{Cell, CellAttrs, Frame};
pub use logo::{Logo, LogoLoadError};
pub use signals::SignalListener;
