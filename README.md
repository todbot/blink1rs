# blink1rs

[![crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]
[![CI][ci-badge]][ci-url]
[![MSRV][msrv-badge]][crates-url]
[![license][license-badge]][license-url]

Control the [blink(1)](https://blink1.thingm.com/) USB RGB LED from Rust.

A small synchronous API over `hidapi`, aimed at wiring a blink(1) into
alerting: red when the build breaks, green when it passes, and dark when
the thing that was watching stops running.

```rust
use blink1rs::{Blink1, Color};
use std::time::Duration;

let mut b = Blink1::open()?;
b.fade(Color::RED, Duration::from_millis(500))?;
```

## Why this crate

- **hidapi, not libusb.** No driver swapping on Windows, no `rusb` context
  to thread around. Works on Linux, macOS and Windows.
- **The watchdog is first class.** The device can change its own colour when
  your process stops talking to it. That is the thing a desktop notification
  cannot do.
- **Colours are sent as given.** Gamma correction is off by default, so
  `#ff8000` is the byte triple the device receives, and reading it back
  returns exactly that.
- **Real error types**, no stringly-typed failures, and `hidapi` stays out of
  the public API.

## Install

```sh
cargo add blink1rs
```

Library only, without the CLI and its dependencies:

```toml
[dependencies]
blink1rs = { version = "0.2", default-features = false }
```

## The watchdog

Arm it, then tickle it from your health-check loop. If the loop dies, the
device acts by itself.

```rust
use blink1rs::{Blink1, Color, Led, OnTimeout};
use std::time::Duration;

let mut b = Blink1::open()?;

// 'D' can only go dark, stay lit, or play a pattern, so "turn red" means
// parking red in pattern line 0 and pointing the watchdog at it.
b.write_pattern_line(0, Color::RED, Duration::ZERO, Led::All)?;
let fire = OnTimeout::PlayPattern { start: 0, end: 0 };
b.watchdog_enable(Duration::from_secs(30), fire)?;
b.set(Color::GREEN)?;

loop {
    // ... check whatever you are watching ...
    b.watchdog_tickle()?;
    std::thread::sleep(Duration::from_secs(10));
}
```

Firmware 204 tops out near 62 seconds no matter what you ask for, so tickle
well inside the timeout.

## Command line

The crate also installs a `blink1rs` binary.

```sh
blink1rs list                       # attached devices, by serial
blink1rs info                       # hardware, firmware, current colour
blink1rs set red                    # a name, #rrggbb, or r,g,b
blink1rs set '#00ff80' --fade 2s
blink1rs blink red --count 3
blink1rs off
blink1rs --device 1 set blue        # index from `list`
blink1rs --serial 20001234 set blue
blink1rs watchdog --timeout 30s --color red
blink1rs watchdog --off
```

## Several devices

`Blink1::list()` sorts by serial number, the same order `blink1-tool -d N`
uses, so indices agree between the two.

```rust
for info in blink1rs::Blink1::list()? {
    println!("{} ({})", info.serial, info.kind);
}
```

## Hardware support

| | mk1 | mk2 | mk3 | mk4 |
|---|---|---|---|---|
| set / fade | yes | yes | yes | yes |
| read colour back | no | yes | yes | yes |
| per-LED addressing | no | yes | yes | yes |
| pattern lines | 16 | 16 | 32 | 32 |
| `save_patterns` needed | no | yes | yes | yes |
| watchdog | yes | yes | yes | yes |

Generation is read from the serial number prefix, the same way `blink1-lib`
does it. Per-LED addressing and per-LED pattern lines need firmware 204 or
later; startup parameters need firmware 206 or later, or mk3 and up. This
crate documents those requirements but does not enforce them, matching the C
library; older firmware ignores the command rather than failing.

## Linux

A blink(1) with no read permission reports no serial number and is skipped
during enumeration, so it looks like no device is attached rather than like a
permission error. Install the udev rule:

```sh
sudo tee /etc/udev/rules.d/51-blink1.rules <<'EOF'
ATTRS{idVendor}=="27b8", ATTRS{idProduct}=="01ed", \
  MODE:="666", GROUP="plugdev"
EOF
sudo udevadm control --reload && sudo udevadm trigger
```

Then replug the device.

Building needs `libudev-dev` and `pkg-config`, which `hidapi`'s default
hidraw backend links against. To use a different `hidapi` backend, depend on
`hidapi` directly and configure it there; `blink1rs` does not re-export its
backend features, because Cargo would unify them in ways that silently
undo the choice.

## Differences from blink1-tool

- **Gamma is off by default.** `blink1-tool` applies a correction table, so
  the same hex value looks different between the two. Call `set_gamma(true)`
  to match it.
- **`save_patterns()` returns `Ok` without confirmation.** The device stalls
  the USB transfer while it writes flash, so there is nothing to confirm with;
  `blink1-lib` does the same. Read a pattern line back if you need to be sure.
- **Durations are `Duration`.** The wire format counts 10ms ticks, so values
  under 10ms jump instantly and anything over 655.35s is clamped.

## Threads

`Blink1` is `Send` but not `Sync`: move it into a thread, or share one behind
a `Mutex`.

## Testing

```sh
cargo test                                              # no hardware needed
cargo test --test hardware -- --ignored --test-threads=1  # with a device
```

The unit tests assert every report against byte vectors transcribed from
`blink1-lib.c`, with the source function cited in each test, so they catch a
wire-format regression without a device attached. The hardware tests must run
single-threaded: there is one HID handle per device.

## License

MIT

[crates-badge]: https://img.shields.io/crates/v/blink1rs.svg
[crates-url]: https://crates.io/crates/blink1rs
[docs-badge]: https://docs.rs/blink1rs/badge.svg
[docs-url]: https://docs.rs/blink1rs
[ci-badge]: https://github.com/todbot/blink1rs/actions/workflows/ci.yml/badge.svg
[ci-url]: https://github.com/todbot/blink1rs/actions/workflows/ci.yml
[msrv-badge]: https://img.shields.io/crates/msrv/blink1rs.svg
[license-badge]: https://img.shields.io/crates/l/blink1rs.svg
[license-url]: LICENSE
