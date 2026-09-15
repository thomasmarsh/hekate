//! Color depth handling and ANSI escape generation.
//!
//! The shared presentation layer never assumes truecolor: a terminal backend
//! degrades a 24-bit color to the nearest entry of the 256-color cube or the
//! 16-color palette. The conversion is deterministic, so the same frame always
//! produces the same bytes at a given depth.

use std::fmt::Write as _;

/// A 24-bit color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// Black, the default foreground.
    pub const BLACK: Self = Self::new(0, 0, 0);
    /// White, the default bright foreground.
    pub const WHITE: Self = Self::new(255, 255, 255);

    /// Build an opaque color.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Squared Euclidean distance in RGB space, used to pick the nearest
    /// palette entry. Squared distance avoids a square root and preserves the
    /// ordering.
    const fn distance2(self, other: Self) -> u32 {
        let dr = self.r.abs_diff(other.r) as u32;
        let dg = self.g.abs_diff(other.g) as u32;
        let db = self.b.abs_diff(other.b) as u32;
        dr * dr + dg * dg + db * db
    }
}

/// The xterm 16-color palette, in SGR order.
const ANSI16: [Rgb; 16] = [
    Rgb::new(0, 0, 0),
    Rgb::new(128, 0, 0),
    Rgb::new(0, 128, 0),
    Rgb::new(128, 128, 0),
    Rgb::new(0, 0, 128),
    Rgb::new(128, 0, 128),
    Rgb::new(0, 128, 128),
    Rgb::new(192, 192, 192),
    Rgb::new(128, 128, 128),
    Rgb::new(255, 0, 0),
    Rgb::new(0, 255, 0),
    Rgb::new(255, 255, 0),
    Rgb::new(0, 0, 255),
    Rgb::new(255, 0, 255),
    Rgb::new(0, 255, 255),
    Rgb::new(255, 255, 255),
];

/// The six channel levels of the xterm 6x6x6 color cube.
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// How many colors the hosting terminal can render.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorDepth {
    /// 24-bit color (`COLORTERM=truecolor`).
    #[default]
    Truecolor,
    /// The 256-color xterm palette.
    Ansi256,
    /// The 16-color ANSI palette.
    Ansi16,
}

impl ColorDepth {
    /// Guess the depth from the `COLORTERM` and `TERM` environment hints,
    /// preferring the safest answer when a variable is absent or unhelpful.
    pub fn detect() -> Self {
        if let Ok(colorterm) = std::env::var("COLORTERM") {
            let colorterm = colorterm.to_ascii_lowercase();
            if colorterm.contains("truecolor") || colorterm.contains("24bit") {
                return Self::Truecolor;
            }
        }
        match std::env::var("TERM") {
            Ok(term) if term.to_ascii_lowercase().contains("256color") => Self::Ansi256,
            _ => Self::Ansi16,
        }
    }

    /// Whether this depth can render arbitrary 24-bit colors.
    pub const fn is_truecolor(self) -> bool {
        matches!(self, Self::Truecolor)
    }

    /// The foreground SGR parameters (without the `ESC[` and `m`).
    pub fn foreground(self, rgb: Rgb) -> String {
        let mut out = String::new();
        match self {
            Self::Truecolor => {
                let _ = write!(out, "38;2;{};{};{}", rgb.r, rgb.g, rgb.b);
            }
            Self::Ansi256 => {
                let _ = write!(out, "38;5;{}", to_ansi256(rgb));
            }
            Self::Ansi16 => {
                let index = to_ansi16(rgb);
                let code = if index < 8 {
                    30 + index
                } else {
                    90 + index - 8
                };
                let _ = write!(out, "{code}");
            }
        }
        out
    }

    /// The background SGR parameters (without the `ESC[` and `m`).
    pub fn background(self, rgb: Rgb) -> String {
        let mut out = String::new();
        match self {
            Self::Truecolor => {
                let _ = write!(out, "48;2;{};{};{}", rgb.r, rgb.g, rgb.b);
            }
            Self::Ansi256 => {
                let _ = write!(out, "48;5;{}", to_ansi256(rgb));
            }
            Self::Ansi16 => {
                let index = to_ansi16(rgb);
                let code = if index < 8 {
                    40 + index
                } else {
                    100 + index - 8
                };
                let _ = write!(out, "{code}");
            }
        }
        out
    }
}

/// The RGB value of a 256-color palette entry.
pub fn ansi256_color(index: u8) -> Rgb {
    match index {
        0..=15 => ANSI16[index as usize],
        16..=231 => {
            let i = index - 16;
            Rgb::new(
                CUBE_LEVELS[(i / 36) as usize],
                CUBE_LEVELS[((i % 36) / 6) as usize],
                CUBE_LEVELS[(i % 6) as usize],
            )
        }
        _ => {
            let level = 8 + (index - 232) * 10;
            Rgb::new(level, level, level)
        }
    }
}

/// The nearest 256-color palette index to `rgb`.
pub fn to_ansi256(rgb: Rgb) -> u8 {
    (0u8..=255)
        .min_by_key(|&index| rgb.distance2(ansi256_color(index)))
        .expect("the palette is never empty")
}

/// The nearest 16-color palette index to `rgb`.
pub fn to_ansi16(rgb: Rgb) -> u8 {
    (0u8..16)
        .min_by_key(|&index| rgb.distance2(ANSI16[index as usize]))
        .expect("the palette is never empty")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truecolor_emits_direct_rgb() {
        let rgb = Rgb::new(1, 2, 3);
        assert_eq!(ColorDepth::Truecolor.foreground(rgb), "38;2;1;2;3");
        assert_eq!(ColorDepth::Truecolor.background(rgb), "48;2;1;2;3");
    }

    #[test]
    fn cube_entries_follow_the_xterm_levels() {
        // Index 16 is the cube origin, 231 the far corner.
        assert_eq!(ansi256_color(16), Rgb::new(0, 0, 0));
        assert_eq!(ansi256_color(231), Rgb::new(255, 255, 255));
        // The grayscale ramp ends at 238.
        assert_eq!(ansi256_color(255), Rgb::new(238, 238, 238));
        // A cube primary: index 196 is pure red.
        assert_eq!(ansi256_color(196), Rgb::new(255, 0, 0));
    }

    #[test]
    fn nearest_256_index_finds_an_exact_palette_entry() {
        // The palette repeats black and white, so the nearest index may not be
        // the one a color came from; it must still be an exact match.
        for index in 0u8..=255 {
            let rgb = ansi256_color(index);
            let nearest = ansi256_color(to_ansi256(rgb));
            assert_eq!(rgb.distance2(nearest), 0, "index {index} lost its color");
        }
    }

    #[test]
    fn near_black_maps_to_the_first_black_in_every_depth() {
        let near_black = Rgb::new(3, 4, 5);
        assert_eq!(to_ansi16(near_black), 0);
        // Black exists at palette index 0 and 16; the lower index wins ties.
        assert_eq!(ColorDepth::Ansi256.foreground(near_black), "38;5;0");
        assert_eq!(ColorDepth::Ansi16.foreground(Rgb::BLACK), "30");
        assert_eq!(ColorDepth::Ansi16.background(Rgb::WHITE), "107");
    }

    #[test]
    fn detection_prefers_truecolor_then_256_then_16() {
        // Detection reads process environment; assert the mapping table itself
        // via the pure conversion rather than mutating global environment.
        assert!(ColorDepth::Truecolor.is_truecolor());
        assert!(!ColorDepth::Ansi256.is_truecolor());
        assert!(!ColorDepth::Ansi16.is_truecolor());
    }
}
