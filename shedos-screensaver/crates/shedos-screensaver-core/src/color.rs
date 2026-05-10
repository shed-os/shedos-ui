use crate::catppuccin::Catppuccin;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const RED: Self = Self::rgb(255, 0, 0);
    pub const GREEN: Self = Self::rgb(0, 255, 0);
    pub const BLUE: Self = Self::rgb(0, 0, 255);
    pub const YELLOW: Self = Self::rgb(255, 255, 0);
    pub const MAGENTA: Self = Self::rgb(255, 0, 255);
    pub const CYAN: Self = Self::rgb(0, 255, 255);

    /// Catppuccin Mocha brand defaults.
    pub const BASE: Self = Catppuccin::MOCHA.base;
    pub const TEXT: Self = Catppuccin::MOCHA.text;

    /// Parse a `--color` spec: hex (`#rrggbb`/`rrggbb`), decimal RGB
    /// (`r,g,b`), or a name. Names: traditional ANSI (red, green,
    /// blue, cyan, magenta, yellow, white, black) or Catppuccin Mocha
    /// (blue, mauve, peach, text, rosewater, flamingo, pink, red,
    /// maroon, yellow, green, teal, sky, sapphire, lavender,
    /// subtext0/1, surface0/1/2, base, mantle, crust, overlay0/1/2).
    /// Catppuccin wins on overlapping names; pure ANSI is reachable
    /// via hex.
    pub fn parse(spec: &str) -> Result<Self, ColorParseError> {
        let s = spec.trim();
        if s.is_empty() {
            return Err(ColorParseError::Empty);
        }

        // Hex with optional leading '#'.
        let hex = s.strip_prefix('#').unwrap_or(s);
        if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ColorParseError::Invalid(s.to_string()))?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ColorParseError::Invalid(s.to_string()))?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ColorParseError::Invalid(s.to_string()))?;
            return Ok(Self::rgb(r, g, b));
        }

        // Decimal r,g,b
        if s.contains(',') {
            let parts: Vec<&str> = s.split(',').map(str::trim).collect();
            if parts.len() != 3 {
                return Err(ColorParseError::Invalid(s.to_string()));
            }
            let r: u8 = parts[0].parse().map_err(|_| ColorParseError::Invalid(s.to_string()))?;
            let g: u8 = parts[1].parse().map_err(|_| ColorParseError::Invalid(s.to_string()))?;
            let b: u8 = parts[2].parse().map_err(|_| ColorParseError::Invalid(s.to_string()))?;
            return Ok(Self::rgb(r, g, b));
        }

        // Catppuccin first so brand wins on overlapping names (blue,
        // red, green, yellow); ANSI fallthrough catches names not in
        // Catppuccin (cyan, magenta, white, black).
        let lower = s.to_ascii_lowercase();
        if let Some(c) = Catppuccin::MOCHA.lookup(&lower) {
            return Ok(c);
        }
        if let Some(c) = named_ansi(&lower) {
            return Ok(c);
        }

        Err(ColorParseError::Invalid(s.to_string()))
    }
}

fn named_ansi(name: &str) -> Option<Color> {
    Some(match name {
        "black" => Color::BLACK,
        "white" => Color::WHITE,
        "red" => Color::RED,
        "green" => Color::GREEN,
        "blue" => Color::BLUE,
        "yellow" => Color::YELLOW,
        "magenta" | "purple" => Color::MAGENTA,
        "cyan" => Color::CYAN,
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorParseError {
    Empty,
    Invalid(String),
}

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "color spec is empty"),
            Self::Invalid(s) => write!(
                f,
                "invalid color '{s}'; expected #rrggbb, r,g,b, named ANSI \
                 (red/green/blue/cyan/magenta/yellow/white/black), or Catppuccin \
                 Mocha shorthand (blue/mauve/peach/text/...)"
            ),
        }
    }
}

impl std::error::Error for ColorParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_with_hash() {
        assert_eq!(Color::parse("#89b4fa").unwrap(), Color::rgb(0x89, 0xb4, 0xfa));
    }

    #[test]
    fn parse_hex_without_hash() {
        assert_eq!(Color::parse("89b4fa").unwrap(), Color::rgb(0x89, 0xb4, 0xfa));
    }

    #[test]
    fn parse_hex_uppercase() {
        assert_eq!(Color::parse("#89B4FA").unwrap(), Color::rgb(0x89, 0xb4, 0xfa));
    }

    #[test]
    fn parse_rgb_decimal() {
        assert_eq!(Color::parse("137,180,250").unwrap(), Color::rgb(137, 180, 250));
        assert_eq!(Color::parse("137, 180, 250").unwrap(), Color::rgb(137, 180, 250));
    }

    #[test]
    fn parse_named_ansi_falls_through_for_non_catppuccin() {
        // ANSI-only names (no Catppuccin overlap) still resolve to ANSI.
        assert_eq!(Color::parse("magenta").unwrap(), Color::MAGENTA);
        assert_eq!(Color::parse("MAGENTA").unwrap(), Color::MAGENTA);
        assert_eq!(Color::parse(" cyan ").unwrap(), Color::CYAN);
        assert_eq!(Color::parse("white").unwrap(), Color::WHITE);
        assert_eq!(Color::parse("black").unwrap(), Color::BLACK);
        assert_eq!(Color::parse("purple").unwrap(), Color::MAGENTA);
    }

    #[test]
    fn parse_catppuccin_shorthand_wins_on_overlap() {
        // Bare names overlapping ANSI (red, green, blue, yellow)
        // resolve to Catppuccin; pure ANSI is reachable via hex.
        assert_eq!(Color::parse("blue").unwrap(), Catppuccin::MOCHA.blue);
        assert_eq!(Color::parse("red").unwrap(), Catppuccin::MOCHA.red);
        assert_eq!(Color::parse("green").unwrap(), Catppuccin::MOCHA.green);
        assert_eq!(Color::parse("yellow").unwrap(), Catppuccin::MOCHA.yellow);
        assert_eq!(Color::parse("mauve").unwrap(), Catppuccin::MOCHA.mauve);
        assert_eq!(Color::parse("peach").unwrap(), Catppuccin::MOCHA.peach);
        // Pure ANSI red still reachable via hex.
        assert_eq!(Color::parse("#ff0000").unwrap(), Color::RED);
    }

    #[test]
    fn parse_empty_errors() {
        assert!(matches!(Color::parse(""), Err(ColorParseError::Empty)));
        assert!(matches!(Color::parse("   "), Err(ColorParseError::Empty)));
    }

    #[test]
    fn parse_garbage_errors() {
        assert!(matches!(Color::parse("not-a-color"), Err(ColorParseError::Invalid(_))));
        assert!(matches!(Color::parse("#zzzzzz"), Err(ColorParseError::Invalid(_))));
        assert!(matches!(Color::parse("999,0,0"), Err(ColorParseError::Invalid(_))));
        assert!(matches!(Color::parse("1,2,3,4"), Err(ColorParseError::Invalid(_))));
    }
}
