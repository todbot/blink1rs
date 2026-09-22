//! Command-line control of a blink(1).

use anyhow::{bail, Context, Result};
use blink1rs::{Blink1, Color, Led, OnTimeout};
use clap::{Parser, Subcommand};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "blink1rs", version, about = "Control a blink(1) USB RGB LED")]
struct Cli {
    /// Device index, in serial-number order (see `list`)
    #[arg(short, long, global = true, conflicts_with = "serial")]
    device: Option<usize>,

    /// Device serial number
    #[arg(short, long, global = true)]
    serial: Option<String>,

    /// Apply blink1-tool's gamma correction to colors
    #[arg(long, global = true)]
    gamma: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List attached blink(1) devices
    List,

    /// Show details of the selected device
    Info,

    /// Set a color, optionally fading to it
    Set {
        /// Color: a name, #rrggbb, or r,g,b
        color: String,
        /// Fade time, e.g. 500ms or 2s
        #[arg(short, long)]
        fade: Option<String>,
        /// Which LED, 0 for all
        #[arg(short, long, default_value_t = 0)]
        led: u8,
    },

    /// Turn the LED off
    Off,

    /// Blink a color on and off
    Blink {
        /// Color: a name, #rrggbb, or r,g,b
        color: String,
        /// How many times
        #[arg(short, long, default_value_t = 3)]
        count: u32,
        /// Time the LED spends on, and off, each cycle
        #[arg(short, long, default_value = "200ms")]
        rate: String,
    },

    /// Play the stored pattern
    Play,

    /// Stop the stored pattern
    Stop,

    /// Change the LED by itself if nothing tickles it in time
    ///
    /// Arms the device watchdog and exits. Unless something re-arms it
    /// before the timeout, the device acts on its own.
    Watchdog {
        /// How long the device waits before acting
        #[arg(short, long, default_value = "60s")]
        timeout: String,
        /// Color to show on timeout; omit to go dark instead
        #[arg(short, long)]
        color: Option<String>,
        /// Disarm instead of arming
        #[arg(long)]
        off: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Command::List = cli.command {
        let found = Blink1::list().context("enumerating USB HID devices")?;
        if found.is_empty() {
            println!("no blink(1) found");
            #[cfg(target_os = "linux")]
            println!("on Linux this usually means the udev rule is missing; see the README");
        }
        for (i, d) in found.iter().enumerate() {
            println!("{i}: {} ({})", d.serial, d.kind);
        }
        return Ok(());
    }

    let mut b = open(&cli)?;
    b.set_gamma(cli.gamma);

    match cli.command {
        Command::List => unreachable!("handled above"),

        Command::Info => {
            println!("serial:   {}", b.serial());
            println!("hardware: {}", b.kind());
            println!("patterns: {} lines", b.pattern_max());
            match b.firmware_version() {
                Ok(v) => println!("firmware: {}.{}", v / 100, v % 100),
                Err(e) => println!("firmware: unavailable ({e})"),
            }
            if b.kind().can_read_rgb() {
                match b.read_rgb(Led::All) {
                    Ok(c) => println!("color:    {c}"),
                    Err(e) => println!("color:    unavailable ({e})"),
                }
            }
        }

        Command::Set { color, fade, led } => {
            let c = parse_color(&color)?;
            let led = if led == 0 { Led::All } else { Led::N(led) };
            match fade {
                Some(f) => b.fade_led(c, parse_duration(&f)?, led)?,
                None if led == Led::All => b.set(c)?,
                None => b.fade_led(c, Duration::ZERO, led)?,
            }
        }

        Command::Off => b.off()?,

        Command::Blink { color, count, rate } => {
            let c = parse_color(&color)?;
            let rate = parse_duration(&rate)?;
            for _ in 0..count {
                b.set(c)?;
                std::thread::sleep(rate);
                b.off()?;
                std::thread::sleep(rate);
            }
        }

        Command::Play => b.play()?,
        Command::Stop => b.stop()?,

        Command::Watchdog {
            timeout,
            color,
            off,
        } => {
            if off {
                b.watchdog_disable()?;
                println!("watchdog disarmed");
            } else {
                let timeout = parse_duration(&timeout)?;
                let action = match &color {
                    Some(s) => {
                        // 'D' can only go dark, stay lit, or play a pattern,
                        // so a color means parking it in pattern line 0.
                        b.write_pattern_line(0, parse_color(s)?, Duration::ZERO, Led::All)?;
                        OnTimeout::PlayPattern { start: 0, end: 0 }
                    }
                    None => OnTimeout::Off,
                };
                b.watchdog_enable(timeout, action)?;
                let what = color.as_deref().unwrap_or("off");
                println!("watchdog armed: {what} in {timeout:.1?} unless re-armed");
            }
        }
    }

    Ok(())
}

fn open(cli: &Cli) -> Result<Blink1> {
    let b = match (&cli.serial, cli.device) {
        (Some(s), _) => Blink1::open_serial(s).with_context(|| format!("opening serial {s}")),
        (None, Some(n)) => Blink1::open_index(n).with_context(|| format!("opening device {n}")),
        (None, None) => Blink1::open().context("opening the first blink(1)"),
    };
    b.map_err(|e| {
        if cfg!(target_os = "linux") {
            e.context("on Linux a missing udev rule hides the device; see the README")
        } else {
            e
        }
    })
}

fn parse_color(s: &str) -> Result<Color> {
    s.parse().map_err(anyhow::Error::from)
}

/// Accepts `500ms`, `2s`, `1.5s`, or a bare number of milliseconds.
fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    let (num, scale) = if let Some(n) = s.strip_suffix("ms") {
        (n, 1.0)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1000.0)
    } else {
        (s, 1.0)
    };
    let v: f64 = num
        .trim()
        .parse()
        .with_context(|| format!("not a duration: {s:?}"))?;
    if !v.is_finite() || v < 0.0 {
        bail!("not a duration: {s:?}");
    }
    Ok(Duration::from_millis((v * scale) as u64))
}
