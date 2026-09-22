//! Control the [blink(1)](https://blink1.thingm.com/) USB RGB LED.
//!
//! A small synchronous API over `hidapi`, aimed at wiring a blink(1) into
//! alerting: red when the build breaks, green when it passes, dark when the
//! thing that was watching stops running.
//!
//! # Light it up
//!
//! ```no_run
//! use blink1rs::{Blink1, Color};
//! use std::time::Duration;
//!
//! let mut b = Blink1::open()?;
//! b.fade(Color::RED, Duration::from_millis(500))?;
//! std::thread::sleep(Duration::from_secs(1));
//! b.off()?;
//! # Ok::<(), blink1rs::Error>(())
//! ```
//!
//! # Watchdog
//!
//! The device can change its own colour when you stop talking to it, which
//! is the part a desktop notification cannot do. Arm it, then tickle it from
//! your health-check loop; if the loop dies, the light goes red on its own.
//!
//! ```no_run
//! use blink1rs::{Blink1, Color, OnTimeout};
//! use std::time::Duration;
//!
//! let mut b = Blink1::open()?;
//! b.set(Color::GREEN)?;
//! b.write_pattern_line(0, Color::RED, Duration::from_millis(200), blink1rs::Led::All)?;
//! b.watchdog_enable(Duration::from_secs(30), OnTimeout::PlayPattern { start: 0, end: 0 })?;
//!
//! loop {
//!     // ... check whatever you are watching ...
//!     b.watchdog_tickle()?;
//!     std::thread::sleep(Duration::from_secs(10));
//! }
//! # Ok::<(), blink1rs::Error>(())
//! ```
//!
//! # Several devices
//!
//! [`Blink1::list`] sorts by serial number, the same order `blink1-tool -d N`
//! uses, so indices agree between the two.
//!
//! ```no_run
//! use blink1rs::Blink1;
//!
//! for info in Blink1::list()? {
//!     println!("{} ({})", info.serial, info.kind);
//! }
//! # Ok::<(), blink1rs::Error>(())
//! ```
//!
//! # Colours are sent as given
//!
//! Unlike `blink1-tool`, gamma correction is off by default, so `#ff8000` is
//! the byte triple the device receives and
//! [`read_rgb`](Blink1::read_rgb) gives it straight back. Turn it on with
//! [`set_gamma`](Blink1::set_gamma) to match `blink1-tool`'s appearance.
//!
//! # Threads
//!
//! [`Blink1`] is `Send` but not `Sync`: move it into a thread, or share one
//! behind a `Mutex`.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod color;
mod device;
mod error;
mod gamma;
mod protocol;
mod transport;
mod types;

pub use color::Color;
pub use device::{Blink1, PRODUCT_ID, VENDOR_ID};
pub use error::{Error, Result};
pub use types::{
    BootMode, DeviceInfo, DeviceKind, Led, OnTimeout, PatternLine, PlayState, StartupParams,
};
