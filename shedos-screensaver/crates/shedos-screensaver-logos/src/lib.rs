//! Library of SHEDOS ASCII art variants.
//!
//! Each variant is a pre-rendered SHEDOS in a different "font" style,
//! bundled into the binary via `include_str!` so the screensaver
//! has zero runtime file dependencies. The Logo loader from -core
//! parses each variant's text into a `Logo { mask, glyphs, … }` ready
//! for an effect to animate toward.
//!
//! Adding a variant is a 3-step PR:
//!   1. Drop a `.txt` under art/<name>.txt with the new SHEDOS rendition.
//!   2. Add a `LogoVariant` entry to [`LIBRARY`] below.
//!   3. (Optional) Add a snapshot test that loads it and asserts row/col.
//!
//! `/etc/shedos-ascii.txt` still wins as a per-system override at
//! runtime — handled by the CLI, not here. This library is the
//! built-in catalog.

use rand::seq::SliceRandom;
use rand::Rng;
use shedos_screensaver_core::{Color, Logo};
use std::path::PathBuf;

/// One named SHEDOS art variant.
#[derive(Debug, Clone, Copy)]
pub struct LogoVariant {
    pub name: &'static str,
    pub title: &'static str,
    /// The raw ASCII art (`include_str!`'d at compile time).
    pub art: &'static str,
    /// Rough description for `--list-logos` output.
    pub description: &'static str,
    /// Suggested default brand color when this variant is rendered
    /// without an explicit `--color`. Mostly Catppuccin Mocha.
    pub default_color: Color,
}

impl LogoVariant {
    pub fn load(&self) -> Logo {
        Logo::parse(self.art, PathBuf::from(format!("<embedded:{}>", self.name)))
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
        default_color: Color::rgb(0x89, 0xb4, 0xfa), // Catppuccin blue
    },
    LogoVariant {
        name: "ansi-shadow",
        title: "ANSI Shadow",
        art: include_str!("../art/ansi-shadow.txt"),
        description: "Block letters with depth shading via Unicode box-drawing. 6 rows.",
        default_color: Color::rgb(0xcb, 0xa6, 0xf7), // Catppuccin mauve
    },
    LogoVariant {
        name: "big",
        title: "Big",
        art: include_str!("../art/big.txt"),
        description: "Bold filled block letters at a larger scale. 7 rows.",
        default_color: Color::rgb(0xa6, 0xe3, 0xa1), // Catppuccin green
    },
    LogoVariant {
        name: "outline",
        title: "Outline",
        art: include_str!("../art/outline.txt"),
        description: "Hollow letters in box-drawing characters. 5 rows.",
        default_color: Color::rgb(0x89, 0xdc, 0xeb), // Catppuccin sky
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
