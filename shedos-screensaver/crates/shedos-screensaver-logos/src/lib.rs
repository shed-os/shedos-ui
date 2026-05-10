//! Library of SHEDOS ASCII art variants.
//!
//! Each variant is a pre-rendered SHEDOS in a different "font" style,
//! `include_str!`'d at compile time so the screensaver has no runtime
//! file dependencies. -core's Logo loader parses each variant into a
//! `Logo { mask, glyphs, … }`.
//!
//! Each variant ships with a curated Catppuccin Mocha palette. The
//! cycle engine picks one at random per session; the first entry is
//! the canonical brand color used by deterministic call sites.
//!
//! Adding a variant: drop `art/<name>.txt`, add a `LogoVariant` entry
//! to [`LIBRARY`] with its palette, optionally add a snapshot test.
//!
//! `/etc/shedos-ascii.txt` overrides at runtime via the CLI; this
//! library is the built-in catalog.

use rand::seq::SliceRandom;
use rand::Rng;
use shedos_screensaver_core::{Catppuccin, Color, Logo};
use std::path::PathBuf;

const MOCHA: Catppuccin = Catppuccin::MOCHA;

/// One named color in a logo's palette. The `name` matches the
/// Catppuccin Mocha key (`blue`, `mauve`, …) so it round-trips
/// through `--color <name>` and the `Catppuccin::lookup` resolver.
#[derive(Debug, Clone, Copy)]
pub struct NamedColor {
    pub name: &'static str,
    pub color: Color,
}

/// One named SHEDOS art variant.
#[derive(Debug, Clone, Copy)]
pub struct LogoVariant {
    pub name: &'static str,
    pub title: &'static str,
    /// The raw ASCII art (`include_str!`'d at compile time).
    pub art: &'static str,
    /// Rough description for `--list-logos` output.
    pub description: &'static str,
    /// Curated palette. Non-empty by construction; `colors[0]` is
    /// treated as the canonical brand color for this variant.
    pub colors: &'static [NamedColor],
}

impl LogoVariant {
    pub fn load(&self) -> Logo {
        Logo::parse(self.art, PathBuf::from(format!("<embedded:{}>", self.name)))
    }

    /// Canonical brand color: the first palette entry. Used by
    /// deterministic call sites (survey tool, fixtures).
    pub fn default_color(&self) -> Color {
        self.colors[0].color
    }

    /// Pick a color uniformly from the palette. The cycle engine
    /// calls this per session when no `--color` override is set.
    pub fn pick_color(&self, rng: &mut impl Rng) -> Color {
        self.colors
            .choose(rng)
            .expect("LogoVariant palettes are non-empty by construction")
            .color
    }
}

/// The full catalog. Order is the canonical `--list-logos` order;
/// `--logo random` picks uniformly across this slice.
pub const LIBRARY: &[LogoVariant] = &[
    LogoVariant {
        name: "block",
        title: "Block",
        art: include_str!("../art/block.txt"),
        description: "The canonical SHEDOS mark. 2-cell solid strokes, 5 rows.",
        colors: &[
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
        ],
    },
    LogoVariant {
        name: "slim",
        title: "Slim",
        art: include_str!("../art/slim.txt"),
        description: "Narrow 1-cell-thick strokes. Same 5-row footprint, lighter visual weight.",
        colors: &[
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
        ],
    },
    LogoVariant {
        name: "fat",
        title: "Fat",
        art: include_str!("../art/fat.txt"),
        description: "Heavy 3-cell-thick strokes. 5 rows, wider letterforms.",
        colors: &[
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "blue", color: MOCHA.blue },
        ],
    },
    LogoVariant {
        name: "chunky",
        title: "Chunky",
        art: include_str!("../art/chunky.txt"),
        description: "Extra-thick 4-cell strokes. The heaviest 5-row variant in the catalog.",
        colors: &[
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "blue", color: MOCHA.blue },
        ],
    },
    LogoVariant {
        name: "wide",
        title: "Wide",
        art: include_str!("../art/wide.txt"),
        description: "Block letters with extra inter-letter spacing. Same strokes, more breathing room.",
        colors: &[
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
        ],
    },
    LogoVariant {
        name: "shaded",
        title: "Shaded",
        art: include_str!("../art/shaded.txt"),
        description: "Same shape as block but rendered entirely in ▓ medium-density shading.",
        colors: &[
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "maroon", color: MOCHA.maroon },
        ],
    },
    LogoVariant {
        name: "faint",
        title: "Faint",
        art: include_str!("../art/faint.txt"),
        description: "Block shape rendered entirely in ░ light shading. Subdued, distant feel.",
        colors: &[
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "pink", color: MOCHA.pink },
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "rosewater", color: MOCHA.rosewater },
        ],
    },
    LogoVariant {
        name: "gradient",
        title: "Gradient",
        art: include_str!("../art/gradient.txt"),
        description: "Vertical density gradient — solid █ at the top fading through ▓▒ to ░ at the bottom.",
        colors: &[
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "teal", color: MOCHA.teal },
        ],
    },
    LogoVariant {
        name: "gradient-up",
        title: "Gradient Up",
        art: include_str!("../art/gradient-up.txt"),
        description: "Inverse vertical gradient — light ░ at the top building up to solid █ at the bottom.",
        colors: &[
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "peach", color: MOCHA.peach },
        ],
    },
    LogoVariant {
        name: "gradient-pulse",
        title: "Gradient Pulse",
        art: include_str!("../art/gradient-pulse.txt"),
        description: "Alternating high/low density — █ outline rows interleaved with ░ body rows for a strobe effect.",
        colors: &[
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "blue", color: MOCHA.blue },
        ],
    },
    LogoVariant {
        name: "gradient-fade",
        title: "Gradient Fade",
        art: include_str!("../art/gradient-fade.txt"),
        description: "Symmetric vertical fade — solid █ in the middle row, dimming through ▒ to ░ at top and bottom.",
        colors: &[
            NamedColor { name: "pink", color: MOCHA.pink },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "sky", color: MOCHA.sky },
        ],
    },
    LogoVariant {
        name: "gradient-bloom",
        title: "Gradient Bloom",
        art: include_str!("../art/gradient-bloom.txt"),
        description: "Inverse fade — solid █ at top and bottom, dimming through ▒ to ░ in the middle row.",
        colors: &[
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "sky", color: MOCHA.sky },
        ],
    },
    LogoVariant {
        name: "gradient-h",
        title: "Gradient H",
        art: include_str!("../art/gradient-h.txt"),
        description: "Horizontal density gradient — █ at the left fading through ▓▒ to ░ at the right.",
        colors: &[
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "blue", color: MOCHA.blue },
        ],
    },
    LogoVariant {
        name: "gradient-diag",
        title: "Gradient Diag",
        art: include_str!("../art/gradient-diag.txt"),
        description: "Diagonal density gradient — solid █ at top-left fading through ▓▒ to ░ at bottom-right.",
        colors: &[
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "blue", color: MOCHA.blue },
        ],
    },
    LogoVariant {
        name: "gradient-h-reverse",
        title: "Gradient H Reverse",
        art: include_str!("../art/gradient-h-reverse.txt"),
        description: "Horizontal density gradient, right-to-left — light ░ at the left building through ▒▓ to solid █ at the right.",
        colors: &[
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "green", color: MOCHA.green },
        ],
    },
    LogoVariant {
        name: "gradient-diag-reverse",
        title: "Gradient Diag Reverse",
        art: include_str!("../art/gradient-diag-reverse.txt"),
        description: "Diagonal density gradient — solid █ at top-right fading through ▓▒ to ░ at bottom-left.",
        colors: &[
            NamedColor { name: "rosewater", color: MOCHA.rosewater },
            NamedColor { name: "pink", color: MOCHA.pink },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "sky", color: MOCHA.sky },
        ],
    },
    LogoVariant {
        name: "gradient-spotlight",
        title: "Gradient Spotlight",
        art: include_str!("../art/gradient-spotlight.txt"),
        description: "Radial density gradient — solid █ at the geometric center fading outward through ▓▒ to ░ at the corners.",
        colors: &[
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
        ],
    },
    LogoVariant {
        name: "gradient-tunnel",
        title: "Gradient Tunnel",
        art: include_str!("../art/gradient-tunnel.txt"),
        description: "Inverse radial density — light ░ at the center building outward through ▒▓ to solid █ at the corners.",
        colors: &[
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "lavender", color: MOCHA.lavender },
        ],
    },
    LogoVariant {
        name: "gradient-bands",
        title: "Gradient Bands",
        art: include_str!("../art/gradient-bands.txt"),
        description: "Wide horizontal density bands — top two rows solid █, middle row ▓, bottom two rows light ░.",
        colors: &[
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "blue", color: MOCHA.blue },
        ],
    },
    LogoVariant {
        name: "stripe-h",
        title: "Stripe H",
        art: include_str!("../art/stripe-h.txt"),
        description: "Horizontal stripes — outline rows in █, body rows in ▒.",
        colors: &[
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "lavender", color: MOCHA.lavender },
        ],
    },
    LogoVariant {
        name: "stripe-v",
        title: "Stripe V",
        art: include_str!("../art/stripe-v.txt"),
        description: "Vertical stripes — alternating █/▓ columns within each stroke.",
        colors: &[
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "teal", color: MOCHA.teal },
        ],
    },
    LogoVariant {
        name: "checker",
        title: "Checker",
        art: include_str!("../art/checker.txt"),
        description: "Fine █/▓ checkerboard pattern within letter shapes.",
        colors: &[
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "mauve", color: MOCHA.mauve },
        ],
    },
    LogoVariant {
        name: "brick",
        title: "Brick",
        art: include_str!("../art/brick.txt"),
        description: "Block letters with a brick-pattern texture — █ strokes broken by ▒ mortar accents.",
        colors: &[
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "maroon", color: MOCHA.maroon },
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "mauve", color: MOCHA.mauve },
        ],
    },
    LogoVariant {
        name: "3d-iso",
        title: "3D Iso",
        art: include_str!("../art/3d-iso.txt"),
        description: "Per-letter ▒ depth shadow at each stroke's right edge for an isometric feel.",
        colors: &[
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "sky", color: MOCHA.sky },
        ],
    },
    LogoVariant {
        name: "drop-shadow",
        title: "Drop Shadow",
        art: include_str!("../art/drop-shadow.txt"),
        description: "Block letters with a 1-row ░ shadow offset diagonally to the lower-right.",
        colors: &[
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
        ],
    },
    LogoVariant {
        name: "emboss",
        title: "Emboss",
        art: include_str!("../art/emboss.txt"),
        description: "Block letters with a 1-row ░ shadow directly beneath, no offset.",
        colors: &[
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "blue", color: MOCHA.blue },
        ],
    },
    LogoVariant {
        name: "shadow-cast",
        title: "Shadow Cast",
        art: include_str!("../art/shadow-cast.txt"),
        description: "Two-row offset shadow fading from ▒ to ░. The most dramatic shadow variant.",
        colors: &[
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "peach", color: MOCHA.peach },
        ],
    },
    LogoVariant {
        name: "ansi-shadow",
        title: "ANSI Shadow",
        art: include_str!("../art/ansi-shadow.txt"),
        description: "Block letters with ▄/▀ rounded outer corners on every letter for a uniform soft-edge feel.",
        colors: &[
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "maroon", color: MOCHA.maroon },
        ],
    },
    LogoVariant {
        name: "big",
        title: "Big",
        art: include_str!("../art/big.txt"),
        description: "Larger 7-row block letters with 2-cell strokes. The tallest non-stretched variant.",
        colors: &[
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "red", color: MOCHA.red },
        ],
    },
    LogoVariant {
        name: "boxed",
        title: "Boxed",
        art: include_str!("../art/boxed.txt"),
        description: "Block letters surrounded by a 1-cell ░ rectangular frame.",
        colors: &[
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "blue", color: MOCHA.blue },
        ],
    },
    LogoVariant {
        name: "double-frame",
        title: "Double Frame",
        art: include_str!("../art/double-frame.txt"),
        description: "Block letters inside a thick ▒/░ double border with internal padding.",
        colors: &[
            NamedColor { name: "pink", color: MOCHA.pink },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "rosewater", color: MOCHA.rosewater },
        ],
    },
    LogoVariant {
        name: "glow",
        title: "Glow",
        art: include_str!("../art/glow.txt"),
        description: "Block letters with a ░ contour halo following the letter shapes — interior cavities preserved.",
        colors: &[
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "lavender", color: MOCHA.lavender },
        ],
    },
    LogoVariant {
        name: "inset",
        title: "Inset",
        art: include_str!("../art/inset.txt"),
        description: "Block letters with ▒ accents at the second-from-edge cells of horizontal bars.",
        colors: &[
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "sky", color: MOCHA.sky },
        ],
    },
    LogoVariant {
        name: "outlined",
        title: "Outlined",
        art: include_str!("../art/outlined.txt"),
        description: "Block letters with ▓ accent at the leading and trailing cells of every horizontal bar.",
        colors: &[
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
        ],
    },
    LogoVariant {
        name: "dual-layer",
        title: "Dual Layer",
        art: include_str!("../art/dual-layer.txt"),
        description: "Block letters with an offset ▒ overstrike row beneath — heavier than emboss, lighter than shadow-cast.",
        colors: &[
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "lavender", color: MOCHA.lavender },
        ],
    },
    LogoVariant {
        name: "diamond",
        title: "Diamond",
        art: include_str!("../art/diamond.txt"),
        description: "Diagonal █/▓ stipple pattern within strokes — diamond-shaped fill, distinct from the orthogonal checker.",
        colors: &[
            NamedColor { name: "pink", color: MOCHA.pink },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "sky", color: MOCHA.sky },
        ],
    },
    LogoVariant {
        name: "tall",
        title: "Tall",
        art: include_str!("../art/tall.txt"),
        description: "Block letters vertically stretched to 10 rows — outline rows duplicated, body rows doubled.",
        colors: &[
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "sky", color: MOCHA.sky },
        ],
    },
    LogoVariant {
        name: "mirror-flip",
        title: "Mirror Flip",
        art: include_str!("../art/mirror-flip.txt"),
        description: "Block letters with a vertically-flipped ░ reflection below — water-reflection feel. 10 rows.",
        colors: &[
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "lavender", color: MOCHA.lavender },
        ],
    },
];

/// Look up a variant by name. None if no variant matches.
pub fn by_name(name: &str) -> Option<&'static LogoVariant> {
    LIBRARY.iter().find(|v| v.name == name)
}

/// All variant names in catalog order.
pub fn names() -> impl Iterator<Item = &'static str> {
    LIBRARY.iter().map(|v| v.name)
}

/// Pick a random variant. Panics if the library is empty (which
/// is a compile-time impossibility).
pub fn pick_random(rng: &mut impl Rng) -> &'static LogoVariant {
    LIBRARY.choose(rng).expect("LIBRARY is non-empty by construction")
}

/// Pick a random variant whose footprint fits in the given canvas.
/// If nothing fits, falls back to the narrowest variant; doesn't
/// hardcode a name so the catalog can shrink.
pub fn pick_random_for_canvas(rng: &mut impl Rng, rows: u16, cols: u16) -> &'static LogoVariant {
    let candidates: Vec<&'static LogoVariant> = LIBRARY
        .iter()
        .filter(|v| {
            let logo = v.load();
            logo.rows <= rows.saturating_sub(1) && logo.cols <= cols.saturating_sub(1)
        })
        .collect();
    if let Some(v) = candidates.choose(rng).copied() {
        return v;
    }
    LIBRARY
        .iter()
        .min_by_key(|v| v.load().cols)
        .expect("LIBRARY is non-empty by construction")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn library_has_at_least_four_variants() {
        assert!(LIBRARY.len() >= 4, "library shrunk to {}", LIBRARY.len());
    }

    #[test]
    fn library_names_are_unique_and_kebab_case() {
        let mut seen = std::collections::HashSet::new();
        for v in LIBRARY {
            assert!(seen.insert(v.name), "duplicate logo name: {}", v.name);
            for ch in v.name.chars() {
                assert!(
                    ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-',
                    "logo name '{}' contains non-kebab-case char '{}'",
                    v.name,
                    ch
                );
            }
        }
    }

    #[test]
    fn every_variant_parses_to_nonempty_logo() {
        for v in LIBRARY {
            let logo = v.load();
            assert!(logo.rows > 0, "variant '{}' has zero rows", v.name);
            assert!(logo.cols > 0, "variant '{}' has zero cols", v.name);
            assert!(
                logo.lit_count() > 10,
                "variant '{}' has only {} lit cells; check art file",
                v.name,
                logo.lit_count()
            );
        }
    }

    #[test]
    fn every_variant_has_a_non_empty_palette_with_unique_names() {
        for v in LIBRARY {
            assert!(
                !v.colors.is_empty(),
                "variant '{}' has an empty palette",
                v.name
            );
            let mut seen = std::collections::HashSet::new();
            for c in v.colors {
                assert!(
                    seen.insert(c.name),
                    "variant '{}' has duplicate palette entry '{}'",
                    v.name,
                    c.name
                );
                assert_eq!(
                    Catppuccin::MOCHA.lookup(c.name),
                    Some(c.color),
                    "variant '{}' palette entry '{}' RGB does not match Catppuccin lookup",
                    v.name,
                    c.name
                );
            }
        }
    }

    #[test]
    fn default_color_is_first_palette_entry() {
        for v in LIBRARY {
            assert_eq!(v.default_color(), v.colors[0].color, "variant '{}'", v.name);
        }
    }

    #[test]
    fn pick_color_returns_a_palette_member() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        for v in LIBRARY {
            let c = v.pick_color(&mut rng);
            assert!(
                v.colors.iter().any(|n| n.color == c),
                "variant '{}' returned a color not in its palette",
                v.name
            );
        }
    }

    #[test]
    fn by_name_finds_known_variants() {
        assert!(by_name("block").is_some());
        assert!(by_name("ansi-shadow").is_some());
        assert!(by_name("nonsense").is_none());
    }

    #[test]
    fn pick_random_is_deterministic_for_seeded_rng() {
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let a = pick_random(&mut rng).name;
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let b = pick_random(&mut rng).name;
        assert_eq!(a, b);
    }

    #[test]
    fn pick_random_for_tiny_canvas_falls_back_to_smallest() {
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        // 5 rows × 20 cols: none of the current logos fit; fallback
        // returns whichever LIBRARY entry has the fewest cols.
        let v = pick_random_for_canvas(&mut rng, 5, 20);
        let smallest_cols = LIBRARY
            .iter()
            .map(|x| x.load().cols)
            .min()
            .expect("LIBRARY non-empty");
        assert_eq!(
            v.load().cols,
            smallest_cols,
            "tiny-canvas fallback returned {} (cols={}) but the catalog's smallest is cols={}",
            v.name,
            v.load().cols,
            smallest_cols
        );
    }
}
