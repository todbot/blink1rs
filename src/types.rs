// SPDX-FileCopyrightText: 2026 Tod Kurt
// SPDX-License-Identifier: MIT

//! Small value types used across the API.

use std::time::Duration;

/// Which LED a command addresses.
///
/// mk1 has a single LED; mk2 and later have two. Per-LED addressing needs
/// firmware 204 or later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Led {
    /// Every LED on the device.
    #[default]
    All,
    /// A single LED, numbered from 1.
    N(u8),
}

impl Led {
    pub(crate) fn index(self) -> u8 {
        match self {
            Led::All => 0,
            Led::N(n) => n,
        }
    }
}

/// Which generation of blink(1) hardware this is.
///
/// Determined from the serial number prefix, the same way `blink1-lib` does
/// it; no firmware query is involved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DeviceKind {
    /// Original Kickstarter hardware, one LED.
    Mk1,
    /// Two LEDs, adds colour read-back.
    Mk2,
    /// EFM32HG based, 32 pattern lines.
    Mk3,
    /// STM32 based.
    Mk4,
    /// Serial number did not match any known prefix.
    Unknown,
}

impl DeviceKind {
    /// Classify by serial number, which is 8 hex digits.
    ///
    /// Thresholds from `blink1-lib.h`: mk2 at `0x20000000`, mk3 at
    /// `0x30000000`, mk4 at `0x40000000`.
    pub fn from_serial(serial: &str) -> DeviceKind {
        match u32::from_str_radix(serial.trim(), 16) {
            Ok(n) if n >= 0x4000_0000 => DeviceKind::Mk4,
            Ok(n) if n >= 0x3000_0000 => DeviceKind::Mk3,
            Ok(n) if n >= 0x2000_0000 => DeviceKind::Mk2,
            Ok(_) => DeviceKind::Mk1,
            Err(_) => DeviceKind::Unknown,
        }
    }

    /// How many pattern lines this device stores.
    ///
    /// `Unknown` reports the conservative 16.
    pub fn pattern_max(self) -> u8 {
        match self {
            DeviceKind::Mk3 | DeviceKind::Mk4 => 32,
            _ => 16,
        }
    }

    /// Whether this device can report the colour it is currently showing.
    ///
    /// mk1 has only the unreliable `readRGB_mk1` path, which this crate does
    /// not expose.
    pub fn can_read_rgb(self) -> bool {
        !matches!(self, DeviceKind::Mk1)
    }
}

impl std::fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DeviceKind::Mk1 => "mk1",
            DeviceKind::Mk2 => "mk2",
            DeviceKind::Mk3 => "mk3",
            DeviceKind::Mk4 => "mk4",
            DeviceKind::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

/// A blink(1) found by [`Blink1::list`](crate::Blink1::list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// 8 hex digit serial number, unique per device.
    pub serial: String,
    /// Platform-specific HID path.
    pub path: String,
    /// Hardware generation, derived from `serial`.
    pub kind: DeviceKind,
}

/// What the device should do when the watchdog fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnTimeout {
    /// Turn the LED off.
    Off,
    /// Leave the LED showing whatever it last showed.
    StayLit,
    /// Play stored pattern lines `start..=end`, looping forever.
    ///
    /// Needs firmware 205 or later.
    PlayPattern {
        /// First pattern line to play.
        start: u8,
        /// Last pattern line to play.
        end: u8,
    },
}

/// Snapshot of the device's pattern playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayState {
    /// Whether a pattern is currently playing.
    pub playing: bool,
    /// First line of the playing range.
    pub start: u8,
    /// Last line of the playing range.
    pub end: u8,
    /// Loops remaining, or 0 when looping forever.
    pub count: u8,
    /// Line currently being played.
    pub pos: u8,
}

/// One line of the device's stored colour pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternLine {
    /// Colour to fade to.
    pub color: crate::Color,
    /// How long the fade takes.
    pub duration: Duration,
    /// Which LED the line drives.
    pub led: Led,
}

/// What the device does when it is plugged in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BootMode {
    /// Stay dark and wait for commands.
    Normal,
    /// Play the stored pattern.
    Play,
    /// Stay off.
    Off,
    /// A value this crate does not recognise.
    Other(u8),
}

impl BootMode {
    pub(crate) fn to_byte(self) -> u8 {
        match self {
            BootMode::Normal => 0,
            BootMode::Play => 1,
            BootMode::Off => 2,
            BootMode::Other(n) => n,
        }
    }

    pub(crate) fn from_byte(b: u8) -> BootMode {
        match b {
            0 => BootMode::Normal,
            1 => BootMode::Play,
            2 => BootMode::Off,
            n => BootMode::Other(n),
        }
    }
}

/// Power-on behaviour, readable and writable on firmware 206+ and mk3+.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupParams {
    /// What to do at power-on.
    pub mode: BootMode,
    /// First pattern line to play.
    pub start: u8,
    /// Last pattern line to play.
    pub end: u8,
    /// Loop count, 0 for forever.
    pub count: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_prefix_picks_the_generation() {
        // blink1-lib.h:31-33 mk2/mk3/mk4 serial starts.
        assert_eq!(DeviceKind::from_serial("1a001b02"), DeviceKind::Mk1);
        assert_eq!(DeviceKind::from_serial("1FFFFFFF"), DeviceKind::Mk1);
        assert_eq!(DeviceKind::from_serial("20000000"), DeviceKind::Mk2);
        assert_eq!(DeviceKind::from_serial("2a0b1c2d"), DeviceKind::Mk2);
        assert_eq!(DeviceKind::from_serial("30000000"), DeviceKind::Mk3);
        assert_eq!(DeviceKind::from_serial("40000000"), DeviceKind::Mk4);
        assert_eq!(DeviceKind::from_serial("FFFFFFFF"), DeviceKind::Mk4);
        assert_eq!(DeviceKind::from_serial("not-hex"), DeviceKind::Unknown);
    }

    #[test]
    fn pattern_capacity_follows_generation() {
        // blink1-lib.h:54 blink1_pattMaxes { 0, 16, 16, 32, 32 }
        assert_eq!(DeviceKind::Mk1.pattern_max(), 16);
        assert_eq!(DeviceKind::Mk2.pattern_max(), 16);
        assert_eq!(DeviceKind::Mk3.pattern_max(), 32);
        assert_eq!(DeviceKind::Mk4.pattern_max(), 32);
        assert_eq!(DeviceKind::Unknown.pattern_max(), 16);
    }

    #[test]
    fn only_mk1_lacks_rgb_readback() {
        assert!(!DeviceKind::Mk1.can_read_rgb());
        assert!(DeviceKind::Mk2.can_read_rgb());
    }
}
