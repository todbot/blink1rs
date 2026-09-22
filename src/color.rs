// SPDX-FileCopyrightText: 2026 Tod Kurt
// SPDX-License-Identifier: MIT

//! 24-bit RGB colors.

use crate::error::{Error, Result};
use std::fmt;
use std::str::FromStr;

/// A 24-bit RGB color.
///
/// Values are sent to the device as-is unless gamma correction is turned on
/// with [`Blink1::set_gamma`](crate::Blink1::set_gamma).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Color {
    /// Black, i.e. the LED off.
    pub const OFF: Color = Color::rgb(0, 0, 0);
    /// Full red.
    pub const RED: Color = Color::rgb(255, 0, 0);
    /// Full green.
    pub const GREEN: Color = Color::rgb(0, 255, 0);
    /// Full blue.
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    /// Full yellow.
    pub const YELLOW: Color = Color::rgb(255, 255, 0);
    /// Full cyan.
    pub const CYAN: Color = Color::rgb(0, 255, 255);
    /// Full magenta.
    pub const MAGENTA: Color = Color::rgb(255, 0, 255);
    /// Orange, useful for "degraded" states alongside [`RED`](Color::RED).
    pub const ORANGE: Color = Color::rgb(255, 165, 0);
    /// Full white.
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    /// Build a color from its channels.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b }
    }

    /// Scale all three channels by `brightness / 255`.
    ///
    /// Mirrors `blink1_adjustBrightness`: a `brightness` of 0 is a no-op, not
    /// a blackout, so a "no brightness limit" setting can be passed straight
    /// through.
    pub fn with_brightness(self, brightness: u8) -> Color {
        if brightness == 0 {
            return self;
        }
        let scale = |c: u8| ((c as u16 * brightness as u16) >> 8) as u8;
        Color::rgb(scale(self.r), scale(self.g), scale(self.b))
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from((r, g, b): (u8, u8, u8)) -> Color {
        Color::rgb(r, g, b)
    }
}

impl From<[u8; 3]> for Color {
    fn from([r, g, b]: [u8; 3]) -> Color {
        Color::rgb(r, g, b)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl FromStr for Color {
    type Err = Error;

    /// Accepts `#ff00ff`, `ff00ff`, `0xff00ff`, `255,0,255`, `0xff,0x00,0xff`,
    /// and the named constants on [`Color`] (`red`, `off`, ...).
    ///
    /// Hex forms are case-insensitive. The comma forms accept decimal or
    /// `0x`-prefixed channels.
    fn from_str(s: &str) -> Result<Color> {
        let s = s.trim();
        let bad = || Error::BadColor(s.to_string());

        if let Some(c) = named(&s.to_ascii_lowercase()) {
            return Ok(c);
        }

        if s.contains(',') {
            let mut ch = [0u8; 3];
            let mut parts = s.split(',');
            for slot in &mut ch {
                let p = parts.next().ok_or_else(bad)?.trim();
                *slot = match p.strip_prefix("0x").or_else(|| p.strip_prefix("0X")) {
                    Some(hex) => u8::from_str_radix(hex, 16).map_err(|_| bad())?,
                    None => p.parse().map_err(|_| bad())?,
                };
            }
            if parts.next().is_some() {
                return Err(bad());
            }
            return Ok(Color::rgb(ch[0], ch[1], ch[2]));
        }

        let hex = s
            .strip_prefix('#')
            .or_else(|| s.strip_prefix("0x"))
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad());
        }
        let n = u32::from_str_radix(hex, 16).map_err(|_| bad())?;
        Ok(Color::rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
    }
}

fn named(s: &str) -> Option<Color> {
    Some(match s {
        "off" | "black" | "none" => Color::OFF,
        "red" => Color::RED,
        "green" => Color::GREEN,
        "blue" => Color::BLUE,
        "yellow" => Color::YELLOW,
        "cyan" | "aqua" => Color::CYAN,
        "magenta" | "purple" => Color::MAGENTA,
        "orange" => Color::ORANGE,
        "white" => Color::WHITE,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_forms() {
        let want = Color::rgb(0xff, 0x00, 0x80);
        for s in ["#ff0080", "ff0080", "0xff0080", "#FF0080", "FF0080"] {
            assert_eq!(s.parse::<Color>().unwrap(), want, "parsing {s}");
        }
    }

    #[test]
    fn parses_comma_forms() {
        assert_eq!(
            "255,0,128".parse::<Color>().unwrap(),
            Color::rgb(255, 0, 128)
        );
        assert_eq!(
            "0xff,0x00,0x80".parse::<Color>().unwrap(),
            Color::rgb(255, 0, 128)
        );
        assert_eq!(" 1 , 2 , 3 ".parse::<Color>().unwrap(), Color::rgb(1, 2, 3));
    }

    #[test]
    fn parses_names_case_insensitively() {
        assert_eq!("red".parse::<Color>().unwrap(), Color::RED);
        assert_eq!("RED".parse::<Color>().unwrap(), Color::RED);
        assert_eq!("off".parse::<Color>().unwrap(), Color::OFF);
    }

    #[test]
    fn rejects_junk() {
        for s in ["", "#ff00", "gggggg", "1,2", "1,2,3,4", "256,0,0", "nope"] {
            assert!(s.parse::<Color>().is_err(), "should reject {s:?}");
        }
    }

    #[test]
    fn display_round_trips() {
        let c = Color::rgb(0x0a, 0xb1, 0xff);
        assert_eq!(c.to_string(), "#0ab1ff");
        assert_eq!(c.to_string().parse::<Color>().unwrap(), c);
    }

    #[test]
    fn brightness_zero_is_a_no_op() {
        assert_eq!(Color::RED.with_brightness(0), Color::RED);
        assert_eq!(Color::WHITE.with_brightness(128), Color::rgb(127, 127, 127));
    }
}
