//! Wire format: pure buffer builders and response parsers, no I/O.
//!
//! Every layout here is transcribed from `blink1-lib.c` in the blink1-tool
//! repository; the tests cite the exact function each one came from. The
//! device's report is 8 bytes and hidapi prepends the report ID, so every
//! buffer is 9 bytes with `buf[0] == 1`.

use crate::color::Color;
use crate::error::{Error, Result};
use crate::gamma::degamma;
use crate::types::{BootMode, Led, PatternLine, PlayState, StartupParams};
use std::time::Duration;

pub(crate) const REPORT_ID: u8 = 1;
pub(crate) const BUF_SIZE: usize = 9;

pub(crate) type Report = [u8; BUF_SIZE];

/// Durations go on the wire as 10ms ticks in a 16-bit field.
pub(crate) fn ticks(d: Duration) -> u16 {
    (d.as_millis() / 10).min(u16::MAX as u128) as u16
}

fn millis(hi: u8, lo: u8) -> Duration {
    Duration::from_millis(u16::from_be_bytes([hi, lo]) as u64 * 10)
}

fn chan(c: Color, gamma: bool) -> (u8, u8, u8) {
    if gamma {
        (degamma(c.r), degamma(c.g), degamma(c.b))
    } else {
        (c.r, c.g, c.b)
    }
}

/// `'c'` fade to RGB over `dur`.
pub(crate) fn fade(c: Color, dur: Duration, led: Led, gamma: bool) -> Report {
    let (r, g, b) = chan(c, gamma);
    let [hi, lo] = ticks(dur).to_be_bytes();
    [REPORT_ID, b'c', r, g, b, hi, lo, led.index(), 0]
}

/// `'n'` set RGB immediately, all LEDs.
pub(crate) fn set_now(c: Color, gamma: bool) -> Report {
    let (r, g, b) = chan(c, gamma);
    [REPORT_ID, b'n', r, g, b, 0, 0, 0, 0]
}

/// `'r'` request the current colour of `led`.
pub(crate) fn read_rgb_req(led: Led) -> Report {
    [REPORT_ID, b'r', 0, 0, 0, 0, 0, led.index(), 0]
}

pub(crate) fn parse_read_rgb(buf: &Report) -> (Color, Duration) {
    (Color::rgb(buf[2], buf[3], buf[4]), millis(buf[5], buf[6]))
}

/// `'v'` request the firmware version.
pub(crate) fn version_req() -> Report {
    [REPORT_ID, b'v', 0, 0, 0, 0, 0, 0, 0]
}

/// Decode the version as a scaled integer: firmware "v1.1" is 101.
///
/// The digits live at indices 3 and 4, not 2 and 3; byte 2 is unaccounted
/// for in the C source and is deliberately ignored.
pub(crate) fn parse_version(buf: &Report) -> Result<u16> {
    let digit = |b: u8| (b as char).to_digit(10).ok_or(Error::BadResponse);
    Ok((digit(buf[3])? * 100 + digit(buf[4])?) as u16)
}

/// `'D'` arm or disarm the watchdog.
pub(crate) fn serverdown(on: bool, dur: Duration, stay: bool, start: u8, end: u8) -> Report {
    let [hi, lo] = ticks(dur).to_be_bytes();
    [REPORT_ID, b'D', on as u8, hi, lo, stay as u8, start, end, 0]
}

/// `'p'` start or stop pattern playback.
pub(crate) fn play(playing: bool, start: u8, end: u8, count: u8) -> Report {
    [REPORT_ID, b'p', playing as u8, start, end, count, 0, 0, 0]
}

/// `'S'` request playback state.
pub(crate) fn play_state_req() -> Report {
    [REPORT_ID, b'S', 0, 0, 0, 0, 0, 0, 0]
}

pub(crate) fn parse_play_state(buf: &Report) -> PlayState {
    PlayState {
        playing: buf[2] != 0,
        start: buf[3],
        end: buf[4],
        count: buf[5],
        pos: buf[6],
    }
}

/// `'P'` write one pattern line.
///
/// Carries no LED field; the LED comes from a preceding [`set_led_n`].
pub(crate) fn write_pattern_line(c: Color, dur: Duration, pos: u8, gamma: bool) -> Report {
    let (r, g, b) = chan(c, gamma);
    let [hi, lo] = ticks(dur).to_be_bytes();
    [REPORT_ID, b'P', r, g, b, hi, lo, pos, 0]
}

/// `'R'` request one pattern line.
pub(crate) fn read_pattern_line_req(pos: u8) -> Report {
    [REPORT_ID, b'R', 0, 0, 0, 0, 0, pos, 0]
}

/// Unlike `'P'`, the read side *does* report which LED the line drives.
pub(crate) fn parse_pattern_line(buf: &Report) -> PatternLine {
    PatternLine {
        color: Color::rgb(buf[2], buf[3], buf[4]),
        duration: millis(buf[5], buf[6]),
        led: match buf[7] {
            0 => Led::All,
            n => Led::N(n),
        },
    }
}

/// `'W'` commit the pattern to flash. The payload is a fixed magic value.
pub(crate) fn save_patterns() -> Report {
    [REPORT_ID, b'W', 0xBE, 0xEF, 0xCA, 0xFE, 0x00, 0x00, 0]
}

/// `'l'` latch the LED that the next `'P'` applies to.
pub(crate) fn set_led_n(led: Led) -> Report {
    [REPORT_ID, b'l', led.index(), 0, 0, 0, 0, 0, 0]
}

/// `'b'` request power-on behaviour.
pub(crate) fn startup_params_req() -> Report {
    [REPORT_ID, b'b', 0, 0, 0, 0, 0, 0, 0]
}

pub(crate) fn parse_startup_params(buf: &Report) -> StartupParams {
    StartupParams {
        mode: BootMode::from_byte(buf[2]),
        start: buf[3],
        end: buf[4],
        count: buf[5],
    }
}

/// `'B'` set power-on behaviour.
pub(crate) fn set_startup_params(p: StartupParams) -> Report {
    [
        REPORT_ID,
        b'B',
        p.mode.to_byte(),
        p.start,
        p.end,
        p.count,
        0,
        0,
        0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS1000: Duration = Duration::from_millis(1000); // dms = 100 = 0x0064

    #[test]
    fn fade_matches_c_lib() {
        // blink1-lib.c:239 blink1_fadeToRGBN:
        //   buf = {1,'c', r,g,b, dms>>8, dms&0xff, n}
        assert_eq!(
            fade(Color::rgb(0xff, 0x00, 0x80), MS1000, Led::All, false),
            [0x01, b'c', 0xff, 0x00, 0x80, 0x00, 0x64, 0x00, 0x00]
        );
        assert_eq!(
            fade(Color::rgb(1, 2, 3), MS1000, Led::N(2), false),
            [0x01, b'c', 1, 2, 3, 0x00, 0x64, 0x02, 0x00]
        );
    }

    #[test]
    fn set_now_matches_c_lib() {
        // blink1-lib.c:284 blink1_setRGB: buf = {1,'n', r,g,b, 0,0,0}
        assert_eq!(
            set_now(Color::rgb(0x11, 0x22, 0x33), false),
            [0x01, b'n', 0x11, 0x22, 0x33, 0, 0, 0, 0]
        );
    }

    #[test]
    fn gamma_changes_the_bytes_on_the_wire() {
        // blink1-lib.c:246 buf[2] = degamma(r) when enabled.
        let c = Color::rgb(128, 64, 32);
        assert_eq!(&set_now(c, false)[2..5], &[128, 64, 32]);
        assert_eq!(&set_now(c, true)[2..5], &[55, 11, 2]);
        assert_eq!(&fade(c, MS1000, Led::All, true)[2..5], &[55, 11, 2]);
        assert_eq!(
            &write_pattern_line(c, MS1000, 0, true)[2..5],
            &[55, 11, 2],
            "'P' is degamma'd too (blink1-lib.c:400)"
        );
    }

    #[test]
    fn read_rgb_request_carries_the_led_in_byte_7() {
        // blink1-lib.c:312 buf = {1,'r', 0,0,0, 0,0, ledn}
        assert_eq!(read_rgb_req(Led::N(2)), [0x01, b'r', 0, 0, 0, 0, 0, 2, 0]);
        assert_eq!(read_rgb_req(Led::All), [0x01, b'r', 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn read_rgb_response_scales_millis_by_ten() {
        // blink1-lib.c:314-317: rgb from buf[2..4],
        //   *fadeMillis = ((buf[5]<<8) + (buf[6]&0xff)) * 10
        let buf = [0x01, b'r', 0xde, 0xad, 0xbe, 0x00, 0x64, 0x00, 0x00];
        let (c, d) = parse_read_rgb(&buf);
        assert_eq!(c, Color::rgb(0xde, 0xad, 0xbe));
        assert_eq!(d, Duration::from_millis(1000));
    }

    #[test]
    fn version_digits_live_at_indices_three_and_four() {
        // blink1-lib.c:206: rc = ((buf[3]-'0')*100) + (buf[4]-'0')
        // Byte 2 is unaccounted for in the C source, so it must not matter.
        let buf = [0x01, b'v', 0x5a, b'1', b'1', 0, 0, 0, 0];
        assert_eq!(parse_version(&buf).unwrap(), 101);

        let mk3 = [0x01, b'v', 0x00, b'3', b'0', 0, 0, 0, 0];
        assert_eq!(parse_version(&mk3).unwrap(), 300);

        let junk = [0x01, b'v', 0, 0xff, 0xff, 0, 0, 0, 0];
        assert!(matches!(parse_version(&junk), Err(Error::BadResponse)));
    }

    #[test]
    fn serverdown_matches_c_lib() {
        // blink1-lib.c:333 blink1_serverdown:
        //   {1,'D', on, dms>>8, dms&0xff, st, startpos, endpos}
        assert_eq!(
            serverdown(true, Duration::from_secs(30), false, 0, 0),
            [0x01, b'D', 1, 0x0b, 0xb8, 0, 0, 0, 0]
        );
        assert_eq!(
            serverdown(false, Duration::ZERO, false, 0, 0),
            [0x01, b'D', 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            serverdown(true, Duration::from_secs(1), true, 3, 7),
            [0x01, b'D', 1, 0x00, 0x64, 1, 3, 7, 0]
        );
    }

    #[test]
    fn play_matches_c_lib() {
        // blink1-lib.c:354 blink1_playloop: {1,'p', play, start, end, count, 0,0}
        assert_eq!(play(true, 0, 0, 0), [0x01, b'p', 1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(play(false, 0, 0, 0), [0x01, b'p', 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(play(true, 2, 5, 3), [0x01, b'p', 1, 2, 5, 3, 0, 0, 0]);
    }

    #[test]
    fn play_state_parses_all_five_fields() {
        // blink1-lib.c:384-388
        let buf = [0x01, b'S', 1, 2, 5, 3, 4, 0, 0];
        assert_eq!(
            parse_play_state(&buf),
            PlayState {
                playing: true,
                start: 2,
                end: 5,
                count: 3,
                pos: 4
            }
        );
    }

    #[test]
    fn pattern_line_round_trips_through_the_wire_format() {
        // write: blink1-lib.c:405 {1,'P', r,g,b, dms>>8, dms&0xff, pos}
        assert_eq!(
            write_pattern_line(Color::rgb(9, 8, 7), MS1000, 5, false),
            [0x01, b'P', 9, 8, 7, 0x00, 0x64, 5, 0]
        );
        // read request: blink1-lib.c:427 {1,'R', 0,0,0, 0,0, pos}
        assert_eq!(read_pattern_line_req(5), [0x01, b'R', 0, 0, 0, 0, 0, 5, 0]);
        // read response: blink1-lib.c:430-434, ledn from buf[7], millis * 10
        let resp = [0x01, b'R', 9, 8, 7, 0x00, 0x64, 2, 0];
        assert_eq!(
            parse_pattern_line(&resp),
            PatternLine {
                color: Color::rgb(9, 8, 7),
                duration: MS1000,
                led: Led::N(2),
            }
        );
    }

    #[test]
    fn save_patterns_sends_the_magic() {
        // blink1-lib.c:439 blink1_savePattern: BE EF CA FE
        assert_eq!(
            save_patterns(),
            [0x01, b'W', 0xBE, 0xEF, 0xCA, 0xFE, 0x00, 0x00, 0]
        );
    }

    #[test]
    fn set_led_n_is_zero_filled() {
        // blink1-lib.c:459 assigns only buf[0..2] and leaks stack bytes 3..8.
        // We send zeros instead.
        assert_eq!(set_led_n(Led::N(2)), [0x01, b'l', 2, 0, 0, 0, 0, 0, 0]);
        assert_eq!(set_led_n(Led::All), [0x01, b'l', 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn startup_params_match_c_lib() {
        // blink1-lib.c:471 'b' request, :488 'B' set
        assert_eq!(startup_params_req(), [0x01, b'b', 0, 0, 0, 0, 0, 0, 0]);
        let p = StartupParams {
            mode: BootMode::Play,
            start: 1,
            end: 4,
            count: 0,
        };
        assert_eq!(set_startup_params(p), [0x01, b'B', 1, 1, 4, 0, 0, 0, 0]);
        let resp = [0x01, b'b', 2, 1, 4, 7, 0, 0, 0];
        assert_eq!(
            parse_startup_params(&resp),
            StartupParams {
                mode: BootMode::Off,
                start: 1,
                end: 4,
                count: 7
            }
        );
    }

    #[test]
    fn every_report_carries_report_id_one() {
        for r in [
            fade(Color::RED, MS1000, Led::All, false),
            set_now(Color::RED, false),
            read_rgb_req(Led::All),
            version_req(),
            serverdown(true, MS1000, false, 0, 0),
            play(true, 0, 0, 0),
            play_state_req(),
            write_pattern_line(Color::RED, MS1000, 0, false),
            read_pattern_line_req(0),
            save_patterns(),
            set_led_n(Led::All),
            startup_params_req(),
            set_startup_params(StartupParams {
                mode: BootMode::Normal,
                start: 0,
                end: 0,
                count: 0,
            }),
        ] {
            assert_eq!(r[0], REPORT_ID, "report id missing from {r:?}");
            assert_eq!(r.len(), BUF_SIZE);
        }
    }

    #[test]
    fn durations_below_one_tick_become_instant() {
        assert_eq!(ticks(Duration::ZERO), 0);
        assert_eq!(ticks(Duration::from_millis(9)), 0);
        assert_eq!(ticks(Duration::from_millis(10)), 1);
        assert_eq!(ticks(Duration::from_millis(19)), 1);
    }

    #[test]
    fn durations_past_the_field_width_saturate() {
        assert_eq!(ticks(Duration::from_millis(655_350)), u16::MAX);
        assert_eq!(ticks(Duration::from_secs(3600)), u16::MAX);
        assert_eq!(
            fade(Color::RED, Duration::from_secs(3600), Led::All, false)[5..7],
            [0xff, 0xff]
        );
    }
}
