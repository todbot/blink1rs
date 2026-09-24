// SPDX-FileCopyrightText: 2026 Tod Kurt
// SPDX-License-Identifier: MIT

//! Device selection tests that need no blink(1) attached.

use blink1rs::{Blink1, DeviceInfo, DeviceKind, Error};

/// A `DeviceInfo` whose device is gone must not fall back to its stored HID
/// path: paths get reused, so that could open a different device.
#[test]
fn open_info_of_a_vanished_device_is_not_found() {
    // With no working HID backend the failure would be Error::Hid either
    // way, leaving nothing to distinguish.
    if Blink1::list().is_err() {
        return;
    }
    let gone = DeviceInfo {
        serial: "DEADBEEF".into(),
        path: "/nonexistent-hid-path".into(),
        kind: DeviceKind::Mk4,
    };
    assert!(matches!(Blink1::open_info(&gone), Err(Error::NotFound)));
}
