use std::fs;
use std::path::{Path, PathBuf};

/// Where shedos-branding installs the canonical SHEDOS art.
pub const DEFAULT_LOGO_PATH: &str = "/etc/shedos-ascii.txt";

/// Compile-time copy of the canonical SHEDOS art. Pulled from the
/// shedos-branding tree so the screensaver can't drift from
/// fastfetch's source. `Logo::load_default()` falls back to this
/// when the runtime path is missing.
pub const EMBEDDED_SHEDOS_ART: &str =
    include_str!("../../../../shedos-branding/tree/etc/shedos-ascii.txt");

/// SHEDOS ASCII art parsed into a row-major grid plus a derived
/// binary mask (true = lit, false = blank). Loaded once from
/// `/etc/shedos-ascii.txt`.
#[derive(Clone, Debug)]
pub struct Logo {
    pub source_path: PathBuf,
    pub glyphs: Vec<Vec<char>>,
    pub mask: Vec<Vec<bool>>,
    pub rows: u16,
    pub cols: u16,
}

impl Logo {
    /// Load the canonical SHEDOS art. Tries `/etc/shedos-ascii.txt`
    /// first so a shedos-branding update is picked up live; falls
    /// back to the embedded copy. Never fails.
    pub fn load_default() -> Self {
        match Self::load(Path::new(DEFAULT_LOGO_PATH)) {
            Ok(l) => l,
            Err(_) => Self::embedded(),
        }
    }

    /// Compile-time embedded SHEDOS art, used as the fallback when
    /// `/etc/shedos-ascii.txt` is missing.
    pub fn embedded() -> Self {
        Self::parse(EMBEDDED_SHEDOS_ART, PathBuf::from("<embedded>"))
    }

    pub fn load(path: &Path) -> Result<Self, LogoLoadError> {
        let text = fs::read_to_string(path).map_err(|e| LogoLoadError::Read {
            path: path.to_path_buf(),
            source: e,
        })?;
        Ok(Self::parse(&text, path.to_path_buf()))
    }

    pub fn parse(text: &str, source_path: PathBuf) -> Self {
        let glyphs: Vec<Vec<char>> = text
            .lines()
            .map(|line| line.chars().collect::<Vec<char>>())
            .collect();
        let cols = glyphs.iter().map(|r| r.len()).max().unwrap_or(0) as u16;
        let rows = glyphs.len() as u16;
        let mask: Vec<Vec<bool>> = glyphs
            .iter()
            .map(|row| row.iter().map(|c| !c.is_whitespace()).collect())
            .collect();
        Self { source_path, glyphs, mask, rows, cols }
    }

    /// True iff the (row, col) cell is "lit" (non-whitespace).
    /// Out-of-bounds reads return false.
    pub fn lit(&self, row: usize, col: usize) -> bool {
        self.mask
            .get(row)
            .and_then(|r| r.get(col))
            .copied()
            .unwrap_or(false)
    }

    /// Glyph at (row, col); space if out of bounds.
    pub fn glyph_at(&self, row: usize, col: usize) -> char {
        self.glyphs
            .get(row)
            .and_then(|r| r.get(col))
            .copied()
            .unwrap_or(' ')
    }

    /// Total count of lit cells — useful for Conway seeding density checks.
    pub fn lit_count(&self) -> usize {
        self.mask.iter().flatten().filter(|b| **b).count()
    }
}

#[derive(Debug)]
pub enum LogoLoadError {
    Read { path: PathBuf, source: std::io::Error },
}

impl std::fmt::Display for LogoLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, source } => write!(
                f,
                "failed to read logo at {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for LogoLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shedos_art() -> &'static str {
        // Mirror of packaging/shedos-branding/tree/etc/shedos-ascii.txt.
        // Hard-coded here so the test passes even when /etc/shedos-ascii.txt
        // doesn't exist on the dev machine.
        "███████ ██   ██ ███████ ██████  ██████  ███████\n\
         ██      ██   ██ ██      ██   ██ ██   ██ ██\n\
         ███████ ███████ █████   ██   ██ ██   ██ ███████\n\
              ██ ██   ██ ██      ██   ██ ██   ██      ██\n\
         ███████ ██   ██ ███████ ██████  ██████  ███████\n"
    }

    #[test]
    fn parse_shedos_art() {
        let logo = Logo::parse(shedos_art(), PathBuf::from("test"));
        assert_eq!(logo.rows, 5);
        assert!(logo.cols >= 47);
        // First row is all blocks except for the inter-letter gaps.
        assert!(logo.lit(0, 0));
        // Top row, position 7 should be a space (gap between S and H).
        assert!(!logo.lit(0, 7));
    }

    #[test]
    fn lit_count_matches_block_density() {
        let logo = Logo::parse(shedos_art(), PathBuf::from("test"));
        let lit = logo.lit_count();
        // Sanity: SHEDOS art has dozens of lit cells, well under the
        // total area; somewhere in 130..=200 is healthy.
        assert!((130..=200).contains(&lit), "expected 130..=200 lit cells, got {lit}");
    }

    #[test]
    fn out_of_bounds_lit_is_false() {
        let logo = Logo::parse(shedos_art(), PathBuf::from("test"));
        assert!(!logo.lit(99, 99));
        assert!(!logo.lit(0, 9999));
    }

    #[test]
    fn empty_input_is_valid_zero_dim() {
        let logo = Logo::parse("", PathBuf::from("test"));
        assert_eq!(logo.rows, 0);
        assert_eq!(logo.cols, 0);
        assert_eq!(logo.lit_count(), 0);
    }

    #[test]
    fn load_missing_file_errors() {
        let err = Logo::load(Path::new("/this/path/does/not/exist/shedos.txt")).unwrap_err();
        let LogoLoadError::Read { .. } = err;
    }

    #[test]
    fn embedded_logo_has_real_shedos_dimensions() {
        // The packaged shedos-ascii.txt is 5 rows of block-letter
        // SHEDOS, ≥45 cols wide. Anything smaller means the
        // include_str! is pointing at the wrong file (e.g. an
        // accidentally-checked-in placeholder).
        let l = Logo::embedded();
        assert_eq!(l.rows, 5, "embedded logo should be 5 rows; got {}", l.rows);
        assert!(l.cols >= 45, "embedded logo should be ≥45 cols; got {}", l.cols);
        assert!(l.lit_count() > 100, "embedded logo should have >100 lit cells; got {}", l.lit_count());
    }

    #[test]
    fn load_default_falls_back_to_embedded() {
        // Returns real art even on a dev box without shedos-branding.
        let l = Logo::load_default();
        assert!(l.rows > 0);
        assert!(l.cols > 0);
        assert!(l.lit_count() > 0);
    }
}
