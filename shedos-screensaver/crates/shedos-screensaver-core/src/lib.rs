//! Foundation types shared by every screensaver crate.
//!
//! No I/O beyond reading the logo file; no terminal or Wayland deps.
//! Depends on nothing outside `std`.

pub mod audio;
pub mod catppuccin;
pub mod clock;
pub mod color;
pub mod frame;
pub mod lock_state;
pub mod logo;
pub mod signals;

pub use audio::{AudioFrame, NUM_BANDS};
pub use catppuccin::Catppuccin;
pub use clock::{Clock, MockClock, RealClock};
pub use color::{Color, ColorParseError};
pub use frame::{Cell, CellAttrs, Frame};
pub use lock_state::{LockPhase, LockState, LockStateConfig};
pub use logo::{Logo, LogoLoadError};
pub use signals::SignalListener;
