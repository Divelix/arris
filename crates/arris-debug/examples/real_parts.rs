//! `cargo run -p arris-debug --example real_parts -- --committed`: the
//! refusal histogram over the real-part corpus (ADR-0026 §5), printed as
//! markdown (`arris_debug::histogram`).
//!
//! `--committed` counts the committed tier from its fixtures: each
//! refusal under the cycle the fixture records for it, which the part
//! runner holds to the table on every `cargo test`.

use arris_debug::histogram::{COMMITTED_TIER, Histogram};
use arris_debug::{fixtures, part};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args().nth(1).unwrap_or_default();
    if mode != "--committed" {
        eprintln!("usage: real_parts --committed");
        std::process::exit(2);
    }
    let mut histogram = Histogram::new();
    for name in COMMITTED_TIER {
        let fixture = part::load(&fixtures::corpus_root().join(name))?;
        histogram.add_part(&fixture)?;
    }
    print!("{}", histogram.markdown());
    Ok(())
}
