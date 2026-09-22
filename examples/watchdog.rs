//! Hold the light green while this process lives; the device turns itself
//! red on its own once the tickles stop.
//!
//! Run it, then press Ctrl-C and watch the LED go red without any help.

use blink1rs::{Blink1, Color, Led, OnTimeout};
use std::thread::sleep;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

fn main() -> Result<(), blink1rs::Error> {
    let mut b = Blink1::open()?;

    // 'D' can only go dark, stay lit, or play a pattern, so "turn red" means
    // parking red in pattern line 0 and pointing the watchdog at it.
    b.write_pattern_line(0, Color::RED, Duration::ZERO, Led::All)?;
    b.watchdog_enable(TIMEOUT, OnTimeout::PlayPattern { start: 0, end: 0 })?;
    b.set(Color::GREEN)?;

    println!("green while alive, red {TIMEOUT:?} after you stop me. Ctrl-C to try it.");
    loop {
        sleep(TIMEOUT / 3);
        b.watchdog_tickle()?;
        print!(".");
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
}
