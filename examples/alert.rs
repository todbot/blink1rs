//! Flash through the states you would use for a build status.

use blink1rs::{Blink1, Color};
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), blink1rs::Error> {
    let mut b = Blink1::open()?;
    println!("{} ({})", b.serial(), b.kind());

    for (name, color) in [
        ("building", Color::BLUE),
        ("degraded", Color::ORANGE),
        ("failed", Color::RED),
        ("passed", Color::GREEN),
    ] {
        println!("{name}");
        b.fade(color, Duration::from_millis(300))?;
        sleep(Duration::from_secs(1));
    }

    b.fade(Color::OFF, Duration::from_millis(500))?;
    sleep(Duration::from_millis(500));
    Ok(())
}
