//! The corpus benchmark (ADR-0024 §4): for every fixture under
//! `boolean/`, `sweep/` and `blend/` whose recipe builds, the recipe's
//! build and the result's tessellation at the fixture's `mesh_chord`,
//! timed apart, so a change that costs 10× in either shows as a ratio.
//!
//! ```sh
//! cargo bench -p arris --bench corpus                          # print the timings
//! cargo bench -p arris --bench corpus -- --save target/bench.json
//! cargo bench -p arris --bench corpus -- --compare target/bench.json
//! cargo bench -p arris --bench corpus -- --compare target/bench.json --table target/table.md
//! cargo bench -p arris --bench corpus -- --filter boolean/     # a subset
//! ```
//!
//! A relative path is taken from the workspace root. `--compare` prints
//! each case's ratio against the saved report and
//! flags those past `bench::RATIO_FLAG`; it never fails on a time.
//! `--table` also writes that comparison, as markdown, to a file — what
//! `tools/bench-compare.sh` hands the nightly's job summary. A
//! fixture that refuses by design (`expected.degenerate`) has nothing to
//! time and is skipped by name.

use std::path::PathBuf;
use std::process::ExitCode;

use arris::mesh::{MeshRequest, tessellate_with};
use arris_debug::bench::{self, Config, Report, Wall};
use arris_debug::{corpus, fixtures};

/// The corpus areas whose fixtures build a solid from operations.
const AREAS: [&str; 3] = ["boolean/", "sweep/", "blend/"];

struct Args {
    save: Option<PathBuf>,
    compare: Option<PathBuf>,
    table: Option<PathBuf>,
    filter: Option<String>,
}

fn args() -> Result<Args, String> {
    let mut out = Args {
        save: None,
        compare: None,
        table: None,
        filter: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--save" => out.save = Some(at_root(value()?)),
            "--compare" => out.compare = Some(at_root(value()?)),
            "--table" => out.table = Some(at_root(value()?)),
            "--filter" => out.filter = Some(value()?),
            // What `cargo bench` passes every bench target.
            "--bench" => {}
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(out)
}

/// `path` against the workspace root when relative: `cargo bench` runs
/// this binary in the crate's directory, and the examples above name
/// paths from the root.
fn at_root(path: String) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return path;
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn main() -> ExitCode {
    let args = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("corpus bench: {e}");
            return ExitCode::from(2);
        }
    };
    if args.table.is_some() && args.compare.is_none() {
        eprintln!("corpus bench: --table needs --compare");
        return ExitCode::from(2);
    }
    let config = Config::default();
    let mut clock = Wall::new();
    let mut report = Report::default();
    for dir in fixtures::corpus() {
        let name = fixtures::name_of(&dir);
        if !AREAS.iter().any(|a| name.starts_with(a)) {
            continue;
        }
        if args.filter.as_ref().is_some_and(|f| !name.contains(f)) {
            continue;
        }
        let fixture = match fixtures::load(&dir) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("corpus bench: {name}: {e}");
                return ExitCode::FAILURE;
            }
        };
        let chain = match corpus::chain(&dir, "default") {
            Ok(c) => c,
            Err(e) => {
                eprintln!("skipped {name}: {e}");
                continue;
            }
        };
        let Some(body) = chain.result() else {
            eprintln!("skipped {name}: no result body");
            continue;
        };
        let build = bench::time(&format!("{name} build"), config, &mut clock, || {
            std::hint::black_box(corpus::chain(&dir, "default").ok());
        });
        let request = MeshRequest::new(fixture.recipe.tolerances.mesh_chord);
        let mesh = bench::time(&format!("{name} mesh"), config, &mut clock, || {
            std::hint::black_box(tessellate_with(&chain.model, body, &request).ok());
        });
        for case in [build, mesh] {
            println!(
                "{:<64} {:>10.4} s ± {:.4}",
                case.name, case.median, case.mad
            );
            report.cases.push(case);
        }
    }
    let (builds, meshes): (Vec<_>, Vec<_>) = report
        .cases
        .iter()
        .partition(|c| c.name.ends_with(" build"));
    println!(
        "\n{} cases: build {:.3} s, mesh {:.3} s, total {:.3} s",
        report.cases.len(),
        builds.iter().map(|c| c.median).sum::<f64>(),
        meshes.iter().map(|c| c.median).sum::<f64>(),
        report.total()
    );
    if let Some(path) = &args.compare {
        match Report::load(path) {
            Ok(before) => {
                let table = bench::comparison_table(&bench::compare(&before, &report));
                println!("\nAgainst {}:\n\n{table}", path.display());
                if let Some(out) = &args.table {
                    if let Err(e) = std::fs::write(out, &table) {
                        eprintln!("corpus bench: {}: {e}", out.display());
                        return ExitCode::FAILURE;
                    }
                }
            }
            Err(e) => {
                eprintln!("corpus bench: {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }
    if let Some(path) = &args.save {
        if let Err(e) = report.save(path) {
            eprintln!("corpus bench: {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        println!("saved {}", path.display());
    }
    ExitCode::SUCCESS
}
