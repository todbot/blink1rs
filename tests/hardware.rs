// SPDX-FileCopyrightText: 2026 Tod Kurt
// SPDX-License-Identifier: MIT

//! Tests that need a blink(1) plugged in.
//!
//! All `#[ignore]`, so a plain `cargo test` skips them. One device means one
//! HID handle, so they must not run concurrently:
//!
//! ```text
//! cargo test --test hardware -- --ignored --test-threads=1
//! ```
//!
//! Every test leaves the LED off.

use blink1rs::{Blink1, Color, DeviceKind, Led, OnTimeout};
use std::thread::sleep;
use std::time::Duration;

fn device() -> Blink1 {
    Blink1::open().expect("no blink(1) attached; these tests need one")
}

/// Give the device a moment to settle so the next test opens cleanly.
fn quiesce(mut b: Blink1) {
    let _ = b.off();
    drop(b);
    sleep(Duration::from_millis(50));
}

#[test]
#[ignore = "needs hardware"]
fn lists_and_opens_the_same_device() {
    let found = Blink1::list().unwrap();
    assert!(!found.is_empty(), "no blink(1) attached");

    let mut serials: Vec<_> = found.iter().map(|d| d.serial.clone()).collect();
    let sorted = {
        let mut s = serials.clone();
        s.sort();
        s
    };
    assert_eq!(serials, sorted, "list() must be sorted by serial");
    serials.dedup();
    assert_eq!(serials.len(), found.len(), "serials must be unique");

    let b = device();
    assert_eq!(b.serial(), found[0].serial, "open() is list()[0]");
    assert_eq!(b.kind(), DeviceKind::from_serial(b.serial()));
    quiesce(b);
}

#[test]
#[ignore = "needs hardware"]
fn firmware_version_is_plausible() {
    let mut b = device();
    let v = b.firmware_version().unwrap();
    assert!((100..=999).contains(&v), "implausible firmware version {v}");
    println!(
        "{} {} firmware {}.{}",
        b.serial(),
        b.kind(),
        v / 100,
        v % 100
    );
    quiesce(b);
}

/// The test that proves the wire format end to end: gamma is off, so what we
/// set is exactly what the device reports back.
#[test]
#[ignore = "needs hardware"]
fn set_then_read_rgb_round_trips_exactly() {
    let mut b = device();
    if !b.kind().can_read_rgb() {
        println!("skipping: {} has no color read-back", b.kind());
        return;
    }
    assert!(!b.gamma(), "gamma must default to off for this to hold");

    for want in [
        Color::rgb(0x11, 0x22, 0x33),
        Color::RED,
        Color::rgb(0x01, 0x00, 0xff),
        Color::OFF,
    ] {
        b.set(want).unwrap();
        sleep(Duration::from_millis(50));
        assert_eq!(b.read_rgb(Led::All).unwrap(), want, "round trip of {want}");
    }
    quiesce(b);
}

#[test]
#[ignore = "needs hardware"]
fn gamma_on_changes_what_the_device_reports() {
    let mut b = device();
    if !b.kind().can_read_rgb() {
        return;
    }
    b.set_gamma(true);
    b.set(Color::rgb(128, 128, 128)).unwrap();
    sleep(Duration::from_millis(50));
    // GammaE[128] == 55, per blink1-lib.c.
    assert_eq!(b.read_rgb(Led::All).unwrap(), Color::rgb(55, 55, 55));
    quiesce(b);
}

/// `'r'` reports the last colour *sent*, not a live sample, so this can only
/// check the destination. Watch the LED to see the fade itself.
#[test]
#[ignore = "needs hardware"]
fn fade_reaches_its_target() {
    let mut b = device();
    if !b.kind().can_read_rgb() {
        return;
    }
    b.set(Color::OFF).unwrap();
    b.fade(Color::WHITE, Duration::from_millis(600)).unwrap();
    sleep(Duration::from_millis(900));
    assert_eq!(b.read_rgb(Led::All).unwrap(), Color::WHITE);
    quiesce(b);
}

#[test]
#[ignore = "needs hardware"]
fn pattern_line_round_trips() {
    let mut b = device();
    let want = Color::rgb(0x40, 0x80, 0xc0);
    let dur = Duration::from_millis(250);

    b.write_pattern_line(0, want, dur, Led::All).unwrap();
    sleep(Duration::from_millis(50));

    let got = b.read_pattern_line(0).unwrap();
    assert_eq!(got.color, want);
    assert_eq!(got.duration, dur);
    quiesce(b);
}

#[test]
#[ignore = "needs hardware"]
fn pattern_position_beyond_the_device_is_rejected() {
    let mut b = device();
    let max = b.pattern_max();
    assert!(b
        .write_pattern_line(max, Color::RED, Duration::ZERO, Led::All)
        .is_err());
    assert!(b.read_pattern_line(max).is_err());
    quiesce(b);
}

#[test]
#[ignore = "needs hardware"]
fn play_and_stop_report_their_state() {
    let mut b = device();
    if b.kind() == DeviceKind::Mk1 {
        println!("skipping: mk1 has no play-state read-back");
        return;
    }

    b.write_pattern_line(
        0,
        Color::rgb(0, 0, 40),
        Duration::from_millis(200),
        Led::All,
    )
    .unwrap();
    b.write_pattern_line(
        1,
        Color::rgb(0, 40, 0),
        Duration::from_millis(200),
        Led::All,
    )
    .unwrap();

    b.play_range(0, 1, 0).unwrap();
    sleep(Duration::from_millis(100));
    assert!(b.play_state().unwrap().playing, "should be playing");

    b.stop().unwrap();
    sleep(Duration::from_millis(100));
    assert!(!b.play_state().unwrap().playing, "should have stopped");
    quiesce(b);
}

#[test]
#[ignore = "needs hardware"]
fn per_led_addressing_drives_the_two_leds_apart() {
    let mut b = device();
    if b.kind() == DeviceKind::Mk1 {
        println!("skipping: mk1 has one LED");
        return;
    }
    b.off().unwrap();
    b.fade_led(Color::RED, Duration::ZERO, Led::N(1)).unwrap();
    b.fade_led(Color::BLUE, Duration::ZERO, Led::N(2)).unwrap();
    sleep(Duration::from_millis(100));

    assert_eq!(b.read_rgb(Led::N(1)).unwrap(), Color::RED);
    assert_eq!(b.read_rgb(Led::N(2)).unwrap(), Color::BLUE);
    quiesce(b);
}

#[test]
#[ignore = "needs hardware"]
fn save_patterns_survives_the_usb_stall() {
    let mut b = device();
    b.write_pattern_line(0, Color::rgb(7, 7, 7), Duration::from_millis(100), Led::All)
        .unwrap();
    b.save_patterns().unwrap();
    // The device must still answer afterwards; that is what the settle delay
    // in save_patterns() buys.
    assert_eq!(b.read_pattern_line(0).unwrap().color, Color::rgb(7, 7, 7));
    quiesce(b);
}

/// Slow: really waits for the device to act by itself.
/// Observed through `play_state`, not colour: `'r'` reports the last colour
/// the *host* sent, so it would still say green after the device acted.
#[test]
#[ignore = "needs hardware, takes ~8s"]
fn watchdog_fires_on_its_own() {
    let mut b = device();
    if b.kind() == DeviceKind::Mk1 {
        println!("skipping: mk1 has no play-state read-back");
        return;
    }

    b.write_pattern_line(0, Color::RED, Duration::from_millis(200), Led::All)
        .unwrap();
    b.stop().unwrap();
    b.set(Color::GREEN).unwrap();
    b.watchdog_enable(
        Duration::from_secs(3),
        OnTimeout::PlayPattern { start: 0, end: 0 },
    )
    .unwrap();

    // Tickling keeps the device quiet.
    for _ in 0..3 {
        sleep(Duration::from_secs(1));
        b.watchdog_tickle().unwrap();
    }
    assert!(!b.play_state().unwrap().playing, "should still be idle");

    // Stop tickling and the device acts without us.
    sleep(Duration::from_secs(5));
    assert!(
        b.play_state().unwrap().playing,
        "watchdog should have started the pattern by itself"
    );

    b.watchdog_disable().unwrap();
    b.stop().unwrap();
    quiesce(b);
}
