# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog][kac], and this project adheres
to [Semantic Versioning][semver].

## [Unreleased]

## [0.1.1] - 2026-09-22

### Added

- `Error::Unsupported { op, kind }`, naming the operation and the hardware
  generation that cannot do it. `Error` is `#[non_exhaustive]`, so adding
  the variant is not a breaking change.

### Changed

- `read_rgb` on a mk1 returns `Error::Unsupported` instead of
  `Error::BadResponse`. The old variant claimed the device had sent
  something undecodable, when in fact the command was never sent.

## [0.1.0] - 2026-09-22

Initial release.

### Added

- `Blink1` with `open`, `open_index`, `open_serial`, `open_all`,
  `open_info` and `list`. Enumeration is sorted by serial number, matching
  `blink1-tool -d N`, and listing and opening share one `hidapi` context so
  the device list cannot shift between the two.
- Immediate colour: `set`, `off`, `fade`, `fade_led`, `read_rgb`.
- Stored patterns: `play`, `play_range`, `stop`, `play_state`,
  `write_pattern_line`, `read_pattern_line`, `save_patterns`,
  `pattern_max`.
- Hardware watchdog: `watchdog_enable`, `watchdog_tickle`,
  `watchdog_disable`, so the device changes its own colour when the host
  stops talking to it.
- Power-on behaviour: `startup_params`, `set_startup_params`.
- `Color` with named constants, `FromStr` for `#rrggbb`, `r,g,b` and
  colour names, and `with_brightness`.
- `DeviceKind` detection from the serial number prefix, covering mk1
  through mk4.
- Optional gamma correction via `set_gamma`, off by default.
- `blink1rs` command-line tool behind the default `cli` feature.

### Notes

- Colours are sent as given. `blink1-tool` applies gamma correction by
  default and this crate does not, so the same hex value looks different
  between the two unless `set_gamma(true)` is used.
- `save_patterns` cannot confirm the flash write: the device stalls the USB
  transfer while programming. It does verify the device is still responding
  afterwards, so an unplugged device is an error rather than a silent
  success.
- Not implemented: the mk3+ 61-byte report family (notes, unique ID,
  bootloader), mk1 EEPROM access, and the unreliable mk1 colour read-back.

[kac]: https://keepachangelog.com/en/1.1.0/
[semver]: https://semver.org/spec/v2.0.0.html
[Unreleased]: https://github.com/todbot/blink1rs/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/todbot/blink1rs/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/todbot/blink1rs/releases/tag/v0.1.0
