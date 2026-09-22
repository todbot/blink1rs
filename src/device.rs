// SPDX-FileCopyrightText: 2026 Tod Kurt
// SPDX-License-Identifier: MIT

//! The [`Blink1`] handle and every command it supports.

use crate::color::Color;
use crate::error::{Error, Result};
use crate::protocol;
use crate::transport::{HidTransport, Transport};
use crate::types::{DeviceInfo, DeviceKind, Led, OnTimeout, PatternLine, PlayState, StartupParams};
use std::time::Duration;

/// USB vendor ID, ThingM.
pub const VENDOR_ID: u16 = 0x27B8;
/// USB product ID for every blink(1) generation.
pub const PRODUCT_ID: u16 = 0x01ED;

/// Flash programming outruns the USB timeout, so the device needs a moment
/// to itself after a save before it will answer again.
const FLASH_SETTLE: Duration = Duration::from_millis(100);

/// An open blink(1).
///
/// Dropping it closes the USB handle.
///
/// ```no_run
/// use blink1rs::{Blink1, Color};
/// use std::time::Duration;
///
/// let mut b = Blink1::open()?;
/// b.fade(Color::RED, Duration::from_millis(500))?;
/// # Ok::<(), blink1rs::Error>(())
/// ```
pub struct Blink1 {
    transport: Box<dyn Transport + Send>,
    serial: String,
    kind: DeviceKind,
    gamma: bool,
    watchdog: Option<(Duration, OnTimeout)>,
}

impl std::fmt::Debug for Blink1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Blink1")
            .field("serial", &self.serial)
            .field("kind", &self.kind)
            .field("gamma", &self.gamma)
            .finish_non_exhaustive()
    }
}

impl Blink1 {
    /// Every blink(1) attached, sorted by serial number ascending.
    ///
    /// The sort matches `blink1-tool`, so index 0 here is the same device
    /// `blink1-tool -d 0` talks to.
    ///
    /// Devices that report no serial number are skipped, matching
    /// `blink1-lib`. On Linux that usually means the udev rule is missing;
    /// see the README.
    pub fn list() -> Result<Vec<DeviceInfo>> {
        Blink1::list_with(&hidapi::HidApi::new()?)
    }

    fn list_with(api: &hidapi::HidApi) -> Result<Vec<DeviceInfo>> {
        let mut found: Vec<DeviceInfo> = api
            .device_list()
            .filter(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
            .filter_map(|d| {
                let serial = d.serial_number()?.to_string();
                Some(DeviceInfo {
                    kind: DeviceKind::from_serial(&serial),
                    path: d.path().to_string_lossy().into_owned(),
                    serial,
                })
            })
            .collect();
        found.sort_by(|a, b| a.serial.cmp(&b.serial));
        Ok(found)
    }

    /// Open the first blink(1), by the ordering [`Blink1::list`] returns.
    pub fn open() -> Result<Blink1> {
        Blink1::open_index(0)
    }

    /// Open the `n`th blink(1) in [`Blink1::list`] order.
    pub fn open_index(n: usize) -> Result<Blink1> {
        let api = hidapi::HidApi::new()?;
        let info = Blink1::list_with(&api)?
            .into_iter()
            .nth(n)
            .ok_or(Error::NotFound)?;
        Blink1::open_with(&api, &info)
    }

    /// Open the blink(1) with this serial number, case-insensitively.
    pub fn open_serial(serial: &str) -> Result<Blink1> {
        let api = hidapi::HidApi::new()?;
        let info = Blink1::list_with(&api)?
            .into_iter()
            .find(|d| d.serial.eq_ignore_ascii_case(serial))
            .ok_or(Error::NotFound)?;
        Blink1::open_with(&api, &info)
    }

    /// Open every attached blink(1), in [`Blink1::list`] order.
    pub fn open_all() -> Result<Vec<Blink1>> {
        let api = hidapi::HidApi::new()?;
        Blink1::list_with(&api)?
            .iter()
            .map(|info| Blink1::open_with(&api, info))
            .collect()
    }

    /// Open a specific device returned by [`Blink1::list`].
    ///
    /// The device list is re-read, so a device unplugged since the listing
    /// gives [`Error::NotFound`] rather than opening whatever took its place.
    pub fn open_info(info: &DeviceInfo) -> Result<Blink1> {
        Blink1::open_with(&hidapi::HidApi::new()?, info)
    }

    /// Enumerating and opening share one `HidApi`, so the device list cannot
    /// shift between choosing a device and opening it.
    fn open_with(api: &hidapi::HidApi, info: &DeviceInfo) -> Result<Blink1> {
        let path = std::ffi::CString::new(info.path.as_str()).map_err(|_| Error::NotFound)?;
        let dev = api.open_path(&path)?;
        Ok(Blink1 {
            transport: Box::new(HidTransport::new(dev)),
            serial: info.serial.clone(),
            kind: info.kind,
            gamma: false,
            watchdog: None,
        })
    }

    /// This device's serial number.
    pub fn serial(&self) -> &str {
        &self.serial
    }

    /// Which hardware generation this is, from the serial number prefix.
    pub fn kind(&self) -> DeviceKind {
        self.kind
    }

    /// How many pattern lines this device can store.
    pub fn pattern_max(&self) -> u8 {
        self.kind.pattern_max()
    }

    /// Turn gamma correction on or off. Off by default.
    ///
    /// When on, colours are mapped through the same table `blink1-tool` uses
    /// before being sent, which makes low values look more linear to the eye
    /// but means [`read_rgb`](Blink1::read_rgb) will not return what you
    /// passed to [`set`](Blink1::set).
    pub fn set_gamma(&mut self, on: bool) {
        self.gamma = on;
    }

    /// Whether gamma correction is on.
    pub fn gamma(&self) -> bool {
        self.gamma
    }

    // --- immediate colour ---

    /// Set the colour immediately, with no fade, on all LEDs.
    pub fn set(&mut self, color: Color) -> Result<()> {
        self.transport.send(&protocol::set_now(color, self.gamma))
    }

    /// Turn the LED off immediately.
    pub fn off(&mut self) -> Result<()> {
        self.set(Color::OFF)
    }

    /// Fade all LEDs to `color` over `duration`.
    ///
    /// The wire format counts 10ms ticks, so durations under 10ms jump
    /// instantly and anything over 655.35s is clamped to that.
    pub fn fade(&mut self, color: Color, duration: Duration) -> Result<()> {
        self.fade_led(color, duration, Led::All)
    }

    /// Fade one LED to `color` over `duration`.
    ///
    /// Addressing a single LED needs firmware 204 or later; older firmware
    /// treats it as all LEDs. Same duration granularity as [`fade`](Blink1::fade).
    pub fn fade_led(&mut self, color: Color, duration: Duration, led: Led) -> Result<()> {
        self.transport
            .send(&protocol::fade(color, duration, led, self.gamma))
    }

    /// Read back the last colour *sent* to the device.
    ///
    /// This is the fade target, not a live sample: during a fade it already
    /// reports the destination colour. If gamma is on, the value returned is
    /// post-correction and will not match what you passed in.
    ///
    /// Not supported on mk1, whose only read path `blink1-lib` itself marks
    /// unreliable; this crate rejects it rather than returning junk.
    pub fn read_rgb(&mut self, led: Led) -> Result<Color> {
        if !self.kind.can_read_rgb() {
            return Err(Error::Unsupported {
                op: "read_rgb",
                kind: self.kind,
            });
        }
        let mut buf = protocol::read_rgb_req(led);
        self.transport.recv(&mut buf)?;
        Ok(protocol::parse_read_rgb(&buf).0)
    }

    /// Firmware version as a scaled integer: firmware "v1.1" reads as 101.
    pub fn firmware_version(&mut self) -> Result<u16> {
        let mut buf = protocol::version_req();
        self.transport.recv(&mut buf)?;
        protocol::parse_version(&buf)
    }

    // --- stored patterns ---

    /// Play the whole stored pattern, looping forever.
    pub fn play(&mut self) -> Result<()> {
        self.transport.send(&protocol::play(true, 0, 0, 0))
    }

    /// Play stored lines `start..=end`, looping `count` times (0 for forever).
    pub fn play_range(&mut self, start: u8, end: u8, count: u8) -> Result<()> {
        self.check_pos(start)?;
        self.check_pos(end)?;
        self.transport
            .send(&protocol::play(true, start, end, count))
    }

    /// Stop pattern playback.
    pub fn stop(&mut self) -> Result<()> {
        self.transport.send(&protocol::play(false, 0, 0, 0))
    }

    /// Read what the device is currently playing.
    pub fn play_state(&mut self) -> Result<PlayState> {
        let mut buf = protocol::play_state_req();
        self.transport.recv(&mut buf)?;
        Ok(protocol::parse_play_state(&buf))
    }

    /// Write one line of the stored colour pattern.
    ///
    /// The `'P'` report has no LED field, so addressing a single LED sends a
    /// `'l'` latch first; that part needs firmware 204 or later.
    ///
    /// On mk1 this writes straight to nonvolatile storage. On mk2 and later
    /// it only touches RAM until [`save_patterns`](Blink1::save_patterns).
    pub fn write_pattern_line(
        &mut self,
        pos: u8,
        color: Color,
        duration: Duration,
        led: Led,
    ) -> Result<()> {
        self.check_pos(pos)?;
        if led != Led::All {
            self.transport.send(&protocol::set_led_n(led))?;
        }
        self.transport.send(&protocol::write_pattern_line(
            color, duration, pos, self.gamma,
        ))
    }

    /// Read one line of the stored colour pattern.
    pub fn read_pattern_line(&mut self, pos: u8) -> Result<PatternLine> {
        self.check_pos(pos)?;
        let mut buf = protocol::read_pattern_line_req(pos);
        self.transport.recv(&mut buf)?;
        Ok(protocol::parse_pattern_line(&buf))
    }

    /// Commit the pattern held in RAM to flash, so it survives a replug.
    ///
    /// Blocks for about 100ms. The device stalls the USB transfer while it
    /// programs flash, so the write itself almost always reports an error and
    /// that error is ignored, exactly as `blink1_savePattern` does. **The
    /// flash write cannot be confirmed.** What this does check is that the
    /// device is still answering afterwards, so an unplugged device is an
    /// error rather than a silent success. Read a pattern line back if you
    /// need to know the contents took.
    ///
    /// Not needed on mk1, where each line is written directly.
    pub fn save_patterns(&mut self) -> Result<()> {
        let _ = self.transport.send(&protocol::save_patterns());
        std::thread::sleep(FLASH_SETTLE);
        if self.kind != DeviceKind::Mk1 {
            self.firmware_version()?;
        }
        Ok(())
    }

    // --- watchdog ---

    /// Arm the hardware watchdog: if `timeout` passes with no
    /// [`watchdog_tickle`](Blink1::watchdog_tickle), the device does
    /// `on_timeout` by itself.
    ///
    /// This is what makes a blink(1) useful for liveness alerting: the light
    /// changes when your process stops running, without your process being
    /// around to change it.
    ///
    /// Firmware 204 tops out near 62 seconds regardless of the value sent, so
    /// tickle well inside the timeout. `OnTimeout::PlayPattern` needs
    /// firmware 205 or later.
    pub fn watchdog_enable(&mut self, timeout: Duration, on_timeout: OnTimeout) -> Result<()> {
        let (stay, start, end) = match on_timeout {
            OnTimeout::Off => (false, 0, 0),
            OnTimeout::StayLit => (true, 0, 0),
            OnTimeout::PlayPattern { start, end } => {
                self.check_pos(start)?;
                self.check_pos(end)?;
                (false, start, end)
            }
        };
        self.transport
            .send(&protocol::serverdown(true, timeout, stay, start, end))?;
        self.watchdog = Some((timeout, on_timeout));
        Ok(())
    }

    /// Re-arm the watchdog with the settings last passed to
    /// [`watchdog_enable`](Blink1::watchdog_enable).
    ///
    /// Call this on a timer well inside the timeout.
    pub fn watchdog_tickle(&mut self) -> Result<()> {
        let (timeout, on_timeout) = self.watchdog.ok_or(Error::WatchdogNotArmed)?;
        self.watchdog_enable(timeout, on_timeout)
    }

    /// Disarm the watchdog.
    pub fn watchdog_disable(&mut self) -> Result<()> {
        self.transport
            .send(&protocol::serverdown(false, Duration::ZERO, false, 0, 0))?;
        self.watchdog = None;
        Ok(())
    }

    // --- power-on behaviour ---

    /// Read what the device does when plugged in.
    ///
    /// Needs firmware 206 or later, or mk3 and up.
    pub fn startup_params(&mut self) -> Result<StartupParams> {
        let mut buf = protocol::startup_params_req();
        self.transport.recv(&mut buf)?;
        Ok(protocol::parse_startup_params(&buf))
    }

    /// Set what the device does when plugged in.
    ///
    /// Needs firmware 206 or later, or mk3 and up. Follow with
    /// [`save_patterns`](Blink1::save_patterns) to make it stick.
    pub fn set_startup_params(&mut self, params: StartupParams) -> Result<()> {
        self.transport.send(&protocol::set_startup_params(params))
    }

    fn check_pos(&self, pos: u8) -> Result<()> {
        let max = self.pattern_max();
        if pos >= max {
            return Err(Error::PatternPos { pos, max });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Report;
    use crate::transport::mock::MockTransport;
    use crate::types::BootMode;
    use std::sync::Arc;

    fn mock(kind: DeviceKind) -> (Blink1, Arc<MockTransport>) {
        let t = Arc::new(MockTransport::new());
        let b = Blink1 {
            transport: Box::new(ArcTransport(t.clone())),
            serial: "20001234".into(),
            kind,
            gamma: false,
            watchdog: None,
        };
        (b, t)
    }

    /// Lets a test keep a handle on the mock after it is boxed into `Blink1`.
    struct ArcTransport(Arc<MockTransport>);

    impl Transport for ArcTransport {
        fn send(&self, buf: &Report) -> Result<()> {
            self.0.send(buf)
        }
        fn recv(&self, buf: &mut Report) -> Result<()> {
            self.0.recv(buf)
        }
    }

    #[test]
    fn set_and_off_emit_the_right_reports() {
        let (mut b, t) = mock(DeviceKind::Mk2);
        b.set(Color::RED).unwrap();
        b.off().unwrap();
        assert_eq!(
            t.sent(),
            vec![
                [1, b'n', 255, 0, 0, 0, 0, 0, 0],
                [1, b'n', 0, 0, 0, 0, 0, 0, 0],
            ]
        );
    }

    #[test]
    fn gamma_toggle_changes_what_goes_out() {
        let (mut b, t) = mock(DeviceKind::Mk2);
        b.set(Color::rgb(128, 64, 32)).unwrap();
        b.set_gamma(true);
        b.set(Color::rgb(128, 64, 32)).unwrap();
        let sent = t.sent();
        assert_eq!(&sent[0][2..5], &[128, 64, 32], "gamma off sends raw");
        assert_eq!(&sent[1][2..5], &[55, 11, 2], "gamma on maps the channels");
    }

    #[test]
    fn read_rgb_sends_then_gets() {
        let (mut b, t) = mock(DeviceKind::Mk2);
        t.push_response([1, b'r', 0x11, 0x22, 0x33, 0, 0, 0, 0]);
        let c = b.read_rgb(Led::N(2)).unwrap();
        assert_eq!(c, Color::rgb(0x11, 0x22, 0x33));
        assert_eq!(
            t.sent(),
            vec![[1, b'r', 0, 0, 0, 0, 0, 2, 0]],
            "the request must go out first, it carries the LED index"
        );
    }

    #[test]
    fn mk1_refuses_rgb_readback() {
        let (mut b, t) = mock(DeviceKind::Mk1);
        assert!(matches!(
            b.read_rgb(Led::All),
            Err(Error::Unsupported {
                op: "read_rgb",
                kind: DeviceKind::Mk1
            })
        ));
        assert!(t.sent().is_empty(), "nothing should reach the device");
    }

    #[test]
    fn firmware_version_decodes_from_indices_three_and_four() {
        let (mut b, _t) = mock(DeviceKind::Mk3);
        _t.push_response([1, b'v', 0xff, b'3', b'0', 0, 0, 0, 0]);
        assert_eq!(b.firmware_version().unwrap(), 300);
    }

    #[test]
    fn per_led_pattern_line_latches_first() {
        let (mut b, t) = mock(DeviceKind::Mk2);
        b.write_pattern_line(3, Color::GREEN, Duration::from_millis(500), Led::N(2))
            .unwrap();
        assert_eq!(
            t.sent(),
            vec![
                [1, b'l', 2, 0, 0, 0, 0, 0, 0],
                [1, b'P', 0, 255, 0, 0, 50, 3, 0],
            ],
            "'l' must precede 'P'; 'P' has no LED field"
        );
    }

    #[test]
    fn all_leds_pattern_line_skips_the_latch() {
        let (mut b, t) = mock(DeviceKind::Mk2);
        b.write_pattern_line(0, Color::BLUE, Duration::from_millis(10), Led::All)
            .unwrap();
        assert_eq!(t.sent().len(), 1);
        assert_eq!(t.sent()[0][1], b'P');
    }

    #[test]
    fn pattern_positions_are_bounded_by_generation() {
        let (mut b, _) = mock(DeviceKind::Mk2);
        let e = b
            .write_pattern_line(16, Color::RED, Duration::ZERO, Led::All)
            .unwrap_err();
        assert!(matches!(e, Error::PatternPos { pos: 16, max: 16 }));
        assert!(b
            .write_pattern_line(15, Color::RED, Duration::ZERO, Led::All)
            .is_ok());

        let (mut b3, _) = mock(DeviceKind::Mk3);
        assert!(b3
            .write_pattern_line(31, Color::RED, Duration::ZERO, Led::All)
            .is_ok());
        assert!(b3
            .write_pattern_line(32, Color::RED, Duration::ZERO, Led::All)
            .is_err());
    }

    #[test]
    fn watchdog_stores_settings_for_tickle() {
        let (mut b, t) = mock(DeviceKind::Mk2);
        assert!(matches!(
            b.watchdog_tickle().unwrap_err(),
            Error::WatchdogNotArmed
        ));

        b.watchdog_enable(Duration::from_secs(30), OnTimeout::StayLit)
            .unwrap();
        b.watchdog_tickle().unwrap();
        let sent = t.sent();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0], sent[1], "tickle re-sends the stored settings");
        assert_eq!(sent[0], [1, b'D', 1, 0x0b, 0xb8, 1, 0, 0, 0]);

        b.watchdog_disable().unwrap();
        assert_eq!(t.sent()[2], [1, b'D', 0, 0, 0, 0, 0, 0, 0]);
        assert!(matches!(
            b.watchdog_tickle().unwrap_err(),
            Error::WatchdogNotArmed
        ));
    }

    #[test]
    fn watchdog_pattern_range_is_bounds_checked() {
        let (mut b, _) = mock(DeviceKind::Mk2);
        assert!(b
            .watchdog_enable(
                Duration::from_secs(5),
                OnTimeout::PlayPattern { start: 0, end: 20 }
            )
            .is_err());
    }

    #[test]
    fn play_state_round_trips() {
        let (mut b, _t) = mock(DeviceKind::Mk3);
        _t.push_response([1, b'S', 1, 2, 5, 3, 4, 0, 0]);
        assert_eq!(
            b.play_state().unwrap(),
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
    fn startup_params_round_trip() {
        let (mut b, t) = mock(DeviceKind::Mk3);
        t.push_response([1, b'b', 1, 0, 7, 0, 0, 0, 0]);
        assert_eq!(
            b.startup_params().unwrap(),
            StartupParams {
                mode: BootMode::Play,
                start: 0,
                end: 7,
                count: 0
            }
        );
        b.set_startup_params(StartupParams {
            mode: BootMode::Off,
            start: 1,
            end: 2,
            count: 3,
        })
        .unwrap();
        assert_eq!(t.sent()[1], [1, b'B', 2, 1, 2, 3, 0, 0, 0]);
    }

    #[test]
    fn transport_errors_surface_to_the_caller() {
        let (mut b, t) = mock(DeviceKind::Mk2);
        t.fail_after(0);
        assert!(matches!(b.set(Color::RED), Err(Error::Hid(_))));
        assert!(matches!(
            b.fade(Color::RED, Duration::ZERO),
            Err(Error::Hid(_))
        ));
        assert!(matches!(b.play(), Err(Error::Hid(_))));
        assert!(matches!(b.stop(), Err(Error::Hid(_))));
        assert!(matches!(b.read_rgb(Led::All), Err(Error::Hid(_))));
        assert!(matches!(b.firmware_version(), Err(Error::Hid(_))));
        assert!(matches!(
            b.watchdog_enable(Duration::ZERO, OnTimeout::Off),
            Err(Error::Hid(_))
        ));
    }

    #[test]
    fn save_patterns_ignores_the_usb_stall_but_not_a_dead_device() {
        // The 'W' write is expected to fail; a device that still answers the
        // follow-up probe means the save was merely unconfirmable, not lost.
        let (mut b, t) = mock(DeviceKind::Mk2);
        t.push_response([1, b'v', 0, b'3', b'0', 0, 0, 0, 0]);
        assert!(b.save_patterns().is_ok(), "flash write always stalls USB");
        assert_eq!(t.sent()[0], [1, b'W', 0xBE, 0xEF, 0xCA, 0xFE, 0, 0, 0]);
        assert_eq!(t.sent()[1][1], b'v', "liveness probe follows the save");

        let (mut gone, t2) = mock(DeviceKind::Mk2);
        t2.fail_after(1);
        assert!(
            gone.save_patterns().is_err(),
            "a device that stopped answering must not report success"
        );
    }

    #[test]
    fn mk1_skips_the_save_probe() {
        // mk1 writes each line directly and its read path is unreliable, so
        // probing it would fail spuriously.
        let (mut b, t) = mock(DeviceKind::Mk1);
        t.fail_after(0);
        assert!(b.save_patterns().is_ok());
        assert_eq!(t.sent().len(), 1);
    }

    /// The CLI reaches instant-color two ways and they are different opcodes:
    /// 'n' has no LED field, so per-LED instant must go out as 'c' with a
    /// zero duration.
    #[test]
    fn instant_color_uses_n_for_all_leds_and_c_for_one() {
        let (mut b, t) = mock(DeviceKind::Mk2);
        b.set(Color::RED).unwrap();
        b.fade_led(Color::RED, Duration::ZERO, Led::N(2)).unwrap();
        assert_eq!(
            t.sent(),
            vec![
                [1, b'n', 255, 0, 0, 0, 0, 0, 0],
                [1, b'c', 255, 0, 0, 0, 0, 2, 0],
            ]
        );
    }

    #[test]
    fn a_device_can_move_to_another_thread() {
        let (mut b, t) = mock(DeviceKind::Mk2);
        std::thread::spawn(move || b.set(Color::RED).unwrap())
            .join()
            .unwrap();
        assert_eq!(t.sent().len(), 1);
    }
}
