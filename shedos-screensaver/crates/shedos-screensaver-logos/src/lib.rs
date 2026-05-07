//! Library of SHEDOS ASCII art variants.
//!
//! Each variant is a pre-rendered SHEDOS in a different "font" style,
//! bundled into the binary via `include_str!` so the screensaver
//! has zero runtime file dependencies. The Logo loader from -core
//! parses each variant's text into a `Logo { mask, glyphs, … }` ready
//! for an effect to animate toward.
//!
//! Each variant ships with a small curated palette of Catppuccin
//! Mocha colors. The cycle engine picks one at random per session,
//! turning every `(logo, effect)` pair into several visually distinct
//! frames. The first palette entry is treated as the canonical brand
//! color and is used by deterministic call sites (the survey tool,
//! fixtures, any future snapshot tests).
//!
//! Adding a variant is a 3-step PR:
//!   1. Drop a `.txt` under art/<name>.txt with the new SHEDOS rendition.
//!   2. Add a `LogoVariant` entry to [`LIBRARY`] below with its palette.
//!   3. (Optional) Add a snapshot test that loads it and asserts row/col.
//!
//! `/etc/shedos-ascii.txt` still wins as a per-system override at
//! runtime — handled by the CLI, not here. This library is the
//! built-in catalog.

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

    /// Canonical brand color — the first palette entry. Used by
    /// deterministic call sites where a single representative color
    /// is needed (survey tool, fixtures).
    pub fn default_color(&self) -> Color {
        self.colors[0].color
    }

    /// Pick a color uniformly from the palette. The cycle engine
    /// calls this each session when no `--color` override is set,
    /// so the same logo appears in different palette members across
    /// cycles.
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
        description: "Solid block letters, 5 rows. The canonical SHEDOS mark — also what fastfetch shows.",
        colors: &[
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
        ],
    },
    LogoVariant {
        name: "ansi-shadow",
        title: "ANSI Shadow",
        art: include_str!("../art/ansi-shadow.txt"),
        description: "Block letters with depth shading via Unicode box-drawing. 6 rows.",
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
        description: "Bold filled block letters at a larger scale. 7 rows.",
        colors: &[
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "red", color: MOCHA.red },
        ],
    },
    LogoVariant {
        name: "outline",
        title: "Outline",
        art: include_str!("../art/outline.txt"),
        description: "Hollow letters in box-drawing characters. 5 rows.",
        colors: &[
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "pink", color: MOCHA.pink },
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "rosewater", color: MOCHA.rosewater },
        ],
    },
    LogoVariant {
        name: "3d-iso",
        title: "3D Iso",
        art: include_str!("../art/3d-iso.txt"),
        description: "Block letters with a single-cell ▒ depth shadow on each letter's right edge. 5 rows.",
        colors: &[
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "sky", color: MOCHA.sky },
        ],
    },
    LogoVariant {
        name: "gradient",
        title: "Gradient",
        art: include_str!("../art/gradient.txt"),
        description: "Block letters with a vertical density gradient — solid █ at the top fading through ▓▒░ to the bottom. 5 rows.",
        colors: &[
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "blue", color: MOCHA.blue },
            NamedColor { name: "teal", color: MOCHA.teal },
        ],
    },
    LogoVariant {
        name: "emboss",
        title: "Emboss",
        art: include_str!("../art/emboss.txt"),
        description: "Block letters with a single-row ░ drop shadow directly underneath. 6 rows.",
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
        description: "Block letters with a two-row offset drop shadow that fades from ▒ to ░. 7 rows.",
        colors: &[
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "peach", color: MOCHA.peach },
        ],
    },
    LogoVariant {
        name: "wide",
        title: "Wide",
        art: include_str!("../art/wide.txt"),
        description: "Block letters with extra inter-letter spacing for a more open layout. 5 rows.",
        colors: &[
            NamedColor { name: "lavender", color: MOCHA.lavender },
            NamedColor { name: "sky", color: MOCHA.sky },
            NamedColor { name: "teal", color: MOCHA.teal },
            NamedColor { name: "mauve", color: MOCHA.mauve },
            NamedColor { name: "sapphire", color: MOCHA.sapphire },
        ],
    },
    LogoVariant {
        name: "triple-line",
        title: "Triple Line",
        art: include_str!("../art/triple-line.txt"),
        description: "Block letters with 3-cell-wide strokes — bolder than the canonical 2-cell block. 5 rows.",
        colors: &[
            NamedColor { name: "red", color: MOCHA.red },
            NamedColor { name: "peach", color: MOCHA.peach },
            NamedColor { name: "yellow", color: MOCHA.yellow },
            NamedColor { name: "green", color: MOCHA.green },
            NamedColor { name: "mauve", color: MOCHA.mauve },
        ],
    },
    LogoVariant {
        name: "tall",
        title: "Tall",
        art: include_str!("../art/tall.txt"),
        description: "Block letters vertically stretched — each row of the canonical block doubled. 10 rows.",
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
/// If nothing fits (extremely small canvas), falls back to the
/// narrowest variant in the library — the catalog can shrink over
/// time, so we don't hardcode a specific name.
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
        // 5 rows × 20 cols — none of the current logos fit; fallback
        // path returns whichever LIBRARY entry has the fewest cols.
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
