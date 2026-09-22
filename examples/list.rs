//! Print every attached blink(1).

fn main() -> Result<(), blink1rs::Error> {
    for (i, d) in blink1rs::Blink1::list()?.iter().enumerate() {
        println!("{i}: serial={} kind={} path={}", d.serial, d.kind, d.path);
    }
    Ok(())
}
