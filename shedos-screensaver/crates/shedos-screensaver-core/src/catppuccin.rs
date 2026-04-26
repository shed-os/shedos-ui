//! Catppuccin Mocha palette — the canonical ShedOS brand colors.
//! Mirrors the values pinned across the rest of the distro
//! (Hyprland border, hyprlock, sddm theme, fastfetch ANSI_COLOR).

use crate::color::Color;

#[derive(Clone, Copy, Debug)]
pub struct Catppuccin {
    pub rosewater: Color,
    pub flamingo: Color,
    pub pink: Color,
    pub mauve: Color,
    pub red: Color,
    pub maroon: Color,
    pub peach: Color,
    pub yellow: Color,
    pub green: Color,
    pub teal: Color,
    pub sky: Color,
    pub sapphire: Color,
    pub blue: Color,
    pub lavender: Color,
    pub text: Color,
    pub subtext1: Color,
    pub subtext0: Color,
    pub overlay2: Color,
    pub overlay1: Color,
    pub overlay0: Color,
    pub surface2: Color,
    pub surface1: Color,
    pub surface0: Color,
    pub base: Color,
    pub mantle: Color,
    pub crust: Color,
}

impl Catppuccin {
    /// Catppuccin Mocha (the dark variant ShedOS ships).
    /// https://github.com/catppuccin/catppuccin/blob/main/docs/style-guide.md
    pub const MOCHA: Catppuccin = Catppuccin {
        rosewater: Color::rgb(0xf5, 0xe0, 0xdc),
        flamingo: Color::rgb(0xf2, 0xcd, 0xcd),
        pink: Color::rgb(0xf5, 0xc2, 0xe7),
        mauve: Color::rgb(0xcb, 0xa6, 0xf7),
        red: Color::rgb(0xf3, 0x8b, 0xa8),
        maroon: Color::rgb(0xeb, 0xa0, 0xac),
        peach: Color::rgb(0xfa, 0xb3, 0x87),
        yellow: Color::rgb(0xf9, 0xe2, 0xaf),
        green: Color::rgb(0xa6, 0xe3, 0xa1),
        teal: Color::rgb(0x94, 0xe2, 0xd5),
        sky: Color::rgb(0x89, 0xdc, 0xeb),
        sapphire: Color::rgb(0x74, 0xc7, 0xec),
        blue: Color::rgb(0x89, 0xb4, 0xfa),
        lavender: Color::rgb(0xb4, 0xbe, 0xfe),
        text: Color::rgb(0xcd, 0xd6, 0xf4),
        subtext1: Color::rgb(0xba, 0xc2, 0xde),
        subtext0: Color::rgb(0xa6, 0xad, 0xc8),
        overlay2: Color::rgb(0x93, 0x99, 0xb2),
        overlay1: Color::rgb(0x7f, 0x84, 0x9c),
        overlay0: Color::rgb(0x6c, 0x70, 0x86),
        surface2: Color::rgb(0x58, 0x5b, 0x70),
        surface1: Color::rgb(0x45, 0x47, 0x5a),
        surface0: Color::rgb(0x31, 0x32, 0x44),
        base: Color::rgb(0x1e, 0x1e, 0x2e),
        mantle: Color::rgb(0x18, 0x18, 0x25),
        crust: Color::rgb(0x11, 0x11, 0x1b),
    };

    pub fn lookup(&self, name: &str) -> Option<Color> {
        Some(match name {
            "rosewater" => self.rosewater,
            "flamingo" => self.flamingo,
            "pink" => self.pink,
            "mauve" => self.mauve,
            "red" => self.red,
            "maroon" => self.maroon,
            "peach" => self.peach,
            "yellow" => self.yellow,
            "green" => self.green,
            "teal" => self.teal,
            "sky" => self.sky,
            "sapphire" => self.sapphire,
            "blue" => self.blue,
            "lavender" => self.lavender,
            "text" => self.text,
            "subtext1" => self.subtext1,
            "subtext0" => self.subtext0,
            "overlay2" => self.overlay2,
            "overlay1" => self.overlay1,
            "overlay0" => self.overlay0,
            "surface2" => self.surface2,
            "surface1" => self.surface1,
            "surface0" => self.surface0,
            "base" => self.base,
            "mantle" => self.mantle,
            "crust" => self.crust,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brand_blue_matches_hyprland_border() {
        // The ShedOS-wide brand blue is #89b4fa (Catppuccin "blue").
        // It lands in os-release ANSI_COLOR, hyprlock input outline,
        // and dozens of other surfaces. Verify the constant.
        assert_eq!(Catppuccin::MOCHA.blue, Color::rgb(0x89, 0xb4, 0xfa));
    }

    #[test]
    fn lookup_known_names() {
        assert_eq!(Catppuccin::MOCHA.lookup("blue"), Some(Catppuccin::MOCHA.blue));
        assert_eq!(Catppuccin::MOCHA.lookup("mauve"), Some(Catppuccin::MOCHA.mauve));
        assert_eq!(Catppuccin::MOCHA.lookup("peach"), Some(Catppuccin::MOCHA.peach));
    }

    #[test]
    fn lookup_unknown_returns_none() {
        assert!(Catppuccin::MOCHA.lookup("not-a-color").is_none());
        assert!(Catppuccin::MOCHA.lookup("").is_none());
    }
}
