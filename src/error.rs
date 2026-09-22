// SPDX-FileCopyrightText: 2026 Tod Kurt
// SPDX-License-Identifier: MIT

//! Error type for all fallible operations.

/// Errors returned by this crate.
///
/// The `Hid` variant deliberately keeps `hidapi` out of the rest of the
/// public API so a `hidapi` major bump is not a `blink1rs` major bump.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No blink(1) matched the request.
    #[error("no blink(1) device found")]
    NotFound,

    /// A color string could not be parsed.
    #[error("invalid color: {0}")]
    BadColor(String),

    /// A pattern position beyond what this device can store.
    #[error("pattern position {pos} out of range (device holds {max} lines)")]
    PatternPos {
        /// The position that was requested.
        pos: u8,
        /// The number of pattern lines this device holds.
        max: u8,
    },

    /// The device answered, but not with something we could decode.
    #[error("device returned an unexpected response")]
    BadResponse,

    /// This hardware generation cannot do what was asked.
    #[error("{op} is not supported on {kind}")]
    Unsupported {
        /// The operation that was attempted.
        op: &'static str,
        /// The generation of the device it was attempted on.
        kind: crate::DeviceKind,
    },

    /// [`Blink1::watchdog_tickle`](crate::Blink1::watchdog_tickle) was called
    /// before [`Blink1::watchdog_enable`](crate::Blink1::watchdog_enable).
    #[error("watchdog_tickle called before watchdog_enable")]
    WatchdogNotArmed,

    /// The underlying USB HID layer failed.
    #[error("USB HID error: {0}")]
    Hid(#[from] hidapi::HidError),
}

/// Convenience alias for results from this crate.
pub type Result<T> = std::result::Result<T, Error>;
