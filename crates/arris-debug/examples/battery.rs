//! `cargo run -p arris-debug --release --example battery -- --write|--record <fixture-dir>...`:
//! a part fixture's battery (`arris_debug::battery`).
//!
//! `--write` derives the operands of every `read` solid from the file and
//! `expected.json`'s oracle reading, and writes them into `fixture.json`;
//! `tools/oracle/expected.py` then builds them in Open CASCADE. `--record`
//! runs the battery against that answer and records each stage's class
//! on its solid, and the cycle each refusal blocks — the reader's and
//! the battery's Arris refusals — by ADR-0026 §5's table. An outcome that is a kernel bug is printed and not
//! recorded, and the exit status is 1: it is shrunk to `regression/`
//! (ADR-0026 §4), never recorded as the part's outcome, and the stage is
//! recorded by hand as waiting on that fixture (`{"waits-on": "regression/<slug>"}`),
//! which later records keep.

use std::path::Path;

use arris_debug::battery::{self, Class};
use arris_debug::differential::Outcome;
use arris_debug::histogram::Stage;
use arris_debug::part;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_default();
    let dirs: Vec<String> = args.collect();
    if !matches!(mode.as_str(), "--write" | "--record") || dirs.is_empty() {
        eprintln!("usage: battery --write|--record <fixture-dir>...");
        std::process::exit(2);
    }
    let mut failed = false;
    for dir in &dirs {
        let dir = Path::new(dir);
        let mut fixture = part::load(dir)?;
        if mode == "--write" {
            fixture.part.battery = battery::derive(&fixture)?;
            part::save(dir, &fixture.part)?;
            println!("{}: {} solid(s)", fixture.name, fixture.part.battery.len());
            continue;
        }
        let outcomes = battery::outcomes(&fixture)?;
        for solid in &mut fixture.part.solids {
            solid.blocks.clear();
        }
        for ((key, stage), judged) in &outcomes {
            let outcome = &judged.outcome;
            println!("{} {key} {stage}: {outcome}", fixture.name);
            let Some(solid) =
                (fixture.part.solids.iter_mut()).find(|s| &battery::key(s.id, s.instance) == key)
            else {
                continue;
            };
            // The cycle an Arris refusal blocks (ADR-0026 §5).
            if let (Outcome::ArrisRefuses(_), Some(refused)) = (outcome, &judged.refused)
                && let Some(cycle) = Stage::of_name(stage).and_then(|s| refused.blocks(s))
            {
                solid.blocks.insert(stage.clone(), cycle.to_string());
            }
            if stage == "read" {
                continue;
            }
            match Class::of(outcome) {
                Some(class) => {
                    solid.battery.insert(stage.clone(), class);
                }
                // A bug already shrunk keeps its wait; a new one is shrunk
                // before it is recorded at all.
                None if matches!(solid.battery.get(stage), Some(Class::WaitsOn(_))) => {}
                None => {
                    solid.battery.remove(stage);
                    failed = true;
                }
            }
        }
        part::save(dir, &fixture.part)?;
    }
    if failed {
        std::process::exit(1);
    }
    Ok(())
}
