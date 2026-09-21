//! The tolerance band (plans/c3-tolerance-apart): every designed contact
//! the corpus holds — flush planes, coincident and coaxial cylinders,
//! tangent walls, a tube circle, an edge on an edge — in a random pose,
//! one operand moved off it by a fraction of the tolerance to sixteen
//! tolerances, along the contact's normal, tilted, or grown
//! ([`prop::body::band_pair`]), and `fuse`, `common` and `cut` run over
//! each.
//!
//! Step 1 of the plan *records* rather than asserts: the outcome per
//! perturbation — the body the contact itself gives (**flush**), another
//! clean body (**generic**), a named refusal, an `Internal` fault,
//! checker-red, a panic — and the volume against the contact's own. Step
//! 9 makes it assert. The survey at 256 cases, the default seed, 10752
//! booleans off their contact (`the_band_survey`):
//!
//! ```text
//! outcome          booleans   per step     failed   per contact    failed
//! flush                3430   ±16          21–23%   CoaxialBore        0%
//! generic              2612   ±4           27–29%   FlushBoxes        22%
//! designed refusal      462   ±2           30–66%   EdgeOnEdge        22%
//!   TangentContact 294        ±1½          35–75%   PinInBore         29%
//!   Empty 153                 ±1           44–62%   TangentOutside    38%
//!   NonManifold 15            ±½           36–37%   TangentCylinders  39%
//! failed               4248   ±¼           33–35%   TangentHole       40%
//!                                                   FlushBoss         54%
//!                                                   CoaxialRod        58%
//!                                                   PipeElbow         76%
//!
//! failures by the plan step whose mechanism they are (`mechanism`),
//! with the 64 at TangentHole's own contact (its fuse and common):
//!   step 2    two points a tolerance apart, one vertex            157
//!   step 2/4  a section edge ending where nothing else does      1356
//!   step 4    two curves crossing with no vertex, a seam crossed  459
//!   step 5    a fit off its branch, S5 between two fits            56
//!   step 6    two splines of one section compared                   0
//!   none      no verdict a hair off parallel or tangent:
//!               Unsupported plane–plane 333, plane–point 69,
//!               cylinder–plane 60, plane–line 18, circle 18;
//!               torus sections called degenerate 672;
//!               a degenerate cylinder frame, an ellipse off
//!               its surfaces 66, a singular fit 3;
//!             slivers: NoInterior 449, Hole 13, L4/L2 panics 98;
//!             the builder and the checker: EdgeUses 434 (64 at
//!               the contact), NotClosed 5, B1 15, V2/V3 panics 4;
//!             shells that meet (Lumps) 27                        2284
//!
//! volume past its bound: 17, every one a posed PipeElbow flush fuse
//! at a radius off by ¼ or ½ of a tolerance, up to 5.2× the bound and
//! the same either sign — the measurement's drift with the distance
//! from the origin (`docs/BACKLOG.md`), not the boolean's.
//! ```
//!
//! Each failure's distinct kind is a `regression/` fixture, at its
//! contact fixture's own numbers where it reproduces there
//! (`the_band_at_the_fixtures_numbers`) and at the survey's where it
//! does not; the report prints every failure as the recipe it came from.

use arris_debug::prop::body::{BAND_STEPS, BandContact, BandMotion, BandPair};
use arris_debug::testing::REL;
use arris_debug::{dump, prop};
use arris_ops::arris_check::arris_topo::{Body, Model, Provenance};
use arris_ops::arris_check::{Level, check};
use arris_ops::measure::mass_properties;
use arris_ops::{Fault, OpError, common, cut, fuse};

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

/// A boolean of two bodies: `fuse`, `common` or `cut`.
type Boolean = fn(&mut Model, Body, Body) -> Result<(Body, Provenance), OpError>;

/// The three booleans, by name.
const OPS: [(&str, Boolean); 3] = [("fuse", fuse), ("common", common), ("cut", cut)];

/// What one boolean of one perturbation gave.
#[derive(Debug, Clone, PartialEq)]
enum Outcome {
    /// A body clean at `Full` with nothing unchecked: its counts and
    /// volume.
    Body { counts: String, volume: f64 },
    /// A typed refusal that is not a kernel fault: its variant and
    /// reason.
    Refused(String),
    /// `OpError::Internal`: the fault's kind.
    Internal(String),
    /// A body the checker rejects, or leaves undecided.
    CheckerRed(String),
    /// A panic: the checker rows its message names, else its first line.
    /// In a debug build a checker-red result panics inside the
    /// operation.
    Panic(String),
}

/// The first line of `s`, cut to `n` characters.
fn head(s: &str, n: usize) -> String {
    s.lines().next().unwrap_or("").chars().take(n).collect()
}

/// The fault's kind: its variant and the split fault's, no ids.
fn fault_kind(fault: &Fault) -> String {
    let text = format!("{fault:?}");
    let kind: String = text
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '(')
        .collect();
    match fault {
        Fault::Split(s) => format!(
            "Split({})",
            head(&format!("{s:?}"), 40).split(' ').next().unwrap_or("")
        ),
        Fault::Builder(b) => format!(
            "Builder({})",
            head(&format!("{b:?}"), 60)
                .split([' ', '{', '('])
                .next()
                .unwrap_or("")
        ),
        Fault::Geometry(g) => {
            let text = g.to_string();
            let words: String = text.chars().take_while(|c| !c.is_ascii_digit()).collect();
            format!(
                "Geometry({})",
                head(words.trim_end_matches(['t', '=', ' ']), 48)
            )
        }
        _ => kind.trim_end_matches('(').to_string(),
    }
}

/// `op(a, b)` classified.
fn outcome(m: &mut Model, op: Boolean, a: Body, b: Body) -> Outcome {
    let result = catch_unwind(AssertUnwindSafe(|| op(m, a, b)));
    let result = match result {
        Ok(r) => r,
        Err(payload) => {
            let text = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            return Outcome::Panic(checker_rows(&text).unwrap_or_else(|| head(&text, 90)));
        }
    };
    match result {
        Ok((body, _)) => {
            let report = check(m, body, Level::Full);
            if !report.is_ok() || !report.unchecked().is_empty() {
                let text = report.to_string();
                return Outcome::CheckerRed(checker_rows(&text).unwrap_or_else(|| head(&text, 90)));
            }
            let counts = dump::euler_line(m, body).unwrap_or_default();
            match mass_properties(m, body) {
                Ok(p) => Outcome::Body {
                    counts,
                    volume: p.volume,
                },
                Err(e) => Outcome::Internal(format!("measure: {}", head(&e.to_string(), 60))),
            }
        }
        Err(OpError::Internal(fault)) => Outcome::Internal(fault_kind(&fault)),
        Err(OpError::Degenerate { reason, .. }) => Outcome::Refused(format!("{reason:?}")),
        Err(e @ OpError::Unsupported { .. }) => {
            // The two kinds, from "no closed form for +f7 (plane surface)
            // against +e3 (circle curve)".
            let text = e.to_string();
            let kinds: Vec<&str> = text
                .split('(')
                .skip(1)
                .filter_map(|k| k.split(')').next())
                .collect();
            Outcome::Refused(format!("Unsupported({})", kinds.join(" against ")))
        }
        Err(e) => {
            let text = format!("{e:?}");
            Outcome::Refused(text.split([' ', '{', '(']).next().unwrap_or("").to_string())
        }
    }
}

/// The rows of a checker report — `S5`, `L4`, … — its text holds, one
/// each, in order: what a checker-red result, or a debug build's panic
/// on one, is filed under.
fn checker_rows(text: &str) -> Option<String> {
    let mut rows: Vec<&str> = Vec::new();
    for line in text.lines() {
        let Some(code) = line.split_whitespace().next() else {
            continue;
        };
        let is_row = code.len() == 2
            && code.starts_with(|c: char| c.is_ascii_uppercase())
            && code.ends_with(|c: char| c.is_ascii_digit());
        if is_row && !rows.contains(&code) {
            rows.push(code);
        }
    }
    (!rows.is_empty()).then(|| rows.join(" "))
}

/// The plan step whose mechanism a failure's class is attributed to
/// (plans/c3-tolerance-apart, open question 5), by what the fault says
/// the pave model got wrong:
///
/// - **2**, two points a tolerance apart that should be one vertex: a
///   pave at an edge's end that is not its vertex, a piece of an edge
///   paved differently from the coincident boundary it lies along, an
///   edge whose ends a tolerance apart were given one vertex (E2);
/// - **2/4**, a section edge ending where nothing else does — a vertex
///   a tolerance from the one it should share, or a crossing of two
///   curves within a tolerance of each other that was never paved;
/// - **4**, two curves crossing, or a section crossing a seam, with no
///   vertex there: a block along an operand edge;
/// - **5**, a fitted section off its branch: a fit that will not come
///   within the tolerance, or S5 finding two fits apart;
/// - **6**, two splines of one section compared (NURBS against NURBS);
/// - **none** of steps 2 to 6: no verdict for two surfaces, a curve
///   and a surface or a point and a surface a hair off parallel or
///   tangent, a section called degenerate at a tangency, sliver faces
///   the polygons cannot resolve, the builder or the checker refusing
///   the result, shells that meet.
fn mechanism(class: &str) -> &'static str {
    let has = |k: &str| class.contains(k);
    if has("EmptySubEdge") || has("CommonBlock") || has("E2") {
        "2"
    } else if has("Dangling") {
        "2/4"
    } else if has("Turn") || has("INTERNAL Seam") {
        "4"
    } else if has("fit: the fit") || has("S5") {
        "5"
    } else if has("NURBS CURVE AGAINST NURBS CURVE") {
        "6"
    } else {
        "none"
    }
}

/// The class a perturbed outcome falls in, against the contact's own.
fn class(at: &Outcome, contact: &Outcome) -> String {
    match (at, contact) {
        (Outcome::Body { counts, .. }, Outcome::Body { counts: c, .. }) if counts == c => {
            "flush".into()
        }
        (Outcome::Body { .. }, _) => "generic".into(),
        (Outcome::Refused(r), Outcome::Refused(c)) if r == c => "flush".into(),
        // No closed form is not one of the designed refusals: a failure.
        (Outcome::Refused(r), _) if r.starts_with("Unsupported") => r.to_uppercase(),
        (Outcome::Refused(r), _) => format!("refused {r}"),
        (Outcome::Internal(k), _) => format!("INTERNAL {k}"),
        (Outcome::CheckerRed(k), _) => format!("CHECKER {k}"),
        (Outcome::Panic(k), _) => format!("PANIC {k}"),
    }
}

/// The contact's own volume of a boolean, from the three at offset zero:
/// the body's where it built, else by the identities from the operands'
/// and the common's — zero where the common is refused, the operands
/// sharing no material at a touch. None where the contact's own boolean,
/// or the common the identity needs, failed rather than refused.
fn contact_volume(name: &str, own: &[Outcome], va: f64, vb: f64) -> Option<f64> {
    let volume = |o: &Outcome| match o {
        Outcome::Body { volume, .. } => Some(*volume),
        Outcome::Refused(_) => Some(0.0),
        _ => None,
    };
    let at = OPS.iter().position(|(n, _)| *n == name)?;
    if let Outcome::Body { volume, .. } = own.get(at)? {
        return Some(*volume);
    }
    volume(own.get(at)?)?;
    let common_v = volume(own.get(1)?)?;
    Some(match name {
        "fuse" => va + vb - common_v,
        "common" => common_v,
        _ => va - common_v,
    })
}

/// One record of the survey.
#[derive(Debug, Clone)]
struct Record {
    contact: BandContact,
    motion: BandMotion,
    step: f64,
    op: &'static str,
    class: String,
    /// `|ΔV|` over the pair's bound at the step plus `REL·V`, where both
    /// the body and a contact volume exist.
    ratio: Option<f64>,
    pair: BandPair,
}

/// The records of one pair: the three booleans at offset zero, then at
/// every step of [`BAND_STEPS`], both signs.
fn survey(pair: &BandPair) -> Vec<Record> {
    let tol = Model::default().precision().default_tolerance;
    let at = |delta: f64| -> Option<(Vec<Outcome>, f64, f64)> {
        let mut outs = Vec::new();
        let mut volumes = (0.0, 0.0);
        for (_, op) in OPS {
            let mut m = Model::default();
            let (a, b) = match catch_unwind(AssertUnwindSafe(|| pair.build(&mut m, delta))) {
                Ok(Ok(ab)) => ab,
                _ => return None,
            };
            volumes = (
                mass_properties(&m, a).map(|p| p.volume).unwrap_or(f64::NAN),
                mass_properties(&m, b).map(|p| p.volume).unwrap_or(f64::NAN),
            );
            outs.push(outcome(&mut m, op, a, b));
        }
        Some((outs, volumes.0, volumes.1))
    };
    let mut records = Vec::new();
    let Some((own, va, vb)) = at(0.0) else {
        records.push(Record {
            contact: pair.contact,
            motion: pair.motion,
            step: 0.0,
            op: "build",
            class: "OPERANDS".into(),
            ratio: None,
            pair: *pair,
        });
        return records;
    };
    for ((name, _), o) in OPS.iter().zip(&own) {
        let class = match o {
            Outcome::Body { .. } => "body".to_string(),
            other => class(other, &Outcome::Refused(String::new())),
        };
        records.push(Record {
            contact: pair.contact,
            motion: pair.motion,
            step: 0.0,
            op: name,
            class,
            ratio: None,
            pair: *pair,
        });
    }
    for &s in &BAND_STEPS {
        for step in [-s, s] {
            let delta = step * tol;
            let Some((outs, va_d, vb_d)) = at(delta) else {
                records.push(Record {
                    contact: pair.contact,
                    motion: pair.motion,
                    step,
                    op: "build",
                    class: "OPERANDS".into(),
                    ratio: None,
                    pair: *pair,
                });
                continue;
            };
            for (((name, _), o), own_o) in OPS.iter().zip(&outs).zip(&own) {
                let ratio = match o {
                    Outcome::Body { volume, .. } => {
                        contact_volume(name, &own, va, vb).map(|expected| {
                            let bound = pair.volume_bound(delta)
                                + (va_d - va).abs()
                                + (vb_d - vb).abs()
                                + REL * va.max(vb);
                            (volume - expected).abs() / bound
                        })
                    }
                    _ => None,
                };
                records.push(Record {
                    contact: pair.contact,
                    motion: pair.motion,
                    step,
                    op: name,
                    class: class(o, own_o),
                    ratio,
                    pair: *pair,
                });
            }
        }
    }
    records
}

/// The survey: [`prop::cases`] pairs from [`prop::seed`], spread over the
/// machine's threads as shards, every record kept; printed as a histogram
/// per contact, motion and boolean, then every distinct failure with the
/// first pair that shows it.
#[test]
#[ignore = "the band survey records and does not assert: plans/c3-tolerance-apart step 9 makes it"]
fn the_band_survey() {
    std::panic::set_hook(Box::new(|_| {}));
    let shards = std::thread::available_parallelism().map_or(8, |n| n.get() as u32);
    let base = prop::seed();
    let records = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for shard in 0..shards {
            let records = &records;
            scope.spawn(move || {
                let _ =
                    prop::try_check_shard(&base, shard, shards, prop::body::band_pair(), |pair| {
                        let found = survey(&pair);
                        records.lock().expect("no poisoned survey").extend(found);
                        Ok(())
                    });
            });
        }
    });
    let _ = std::panic::take_hook();
    let records = records.into_inner().expect("no poisoned survey");
    print!("{}", report(&records));
}

/// The survey as text.
fn report(records: &[Record]) -> String {
    use core::fmt::Write;
    let mut out = String::new();
    // Per contact, motion and boolean: each class and how many steps
    // gave it, the offset-zero outcome first.
    let mut rows: BTreeMap<(BandContact, BandMotion, &str), BTreeMap<String, usize>> =
        BTreeMap::new();
    let mut own: BTreeMap<(BandContact, &str), BTreeMap<String, usize>> = BTreeMap::new();
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_step: BTreeMap<i64, BTreeMap<String, usize>> = BTreeMap::new();
    let mut failures: BTreeMap<String, (usize, &Record)> = BTreeMap::new();
    let mut jumps: Vec<&Record> = Vec::new();
    let mut pairs = 0usize;
    for r in records {
        if r.step == 0.0 {
            if r.op == "fuse" || r.op == "build" {
                pairs += 1;
            }
            *own.entry((r.contact, r.op))
                .or_default()
                .entry(r.class.clone())
                .or_default() += 1;
            if r.class != "body" && r.class.chars().next().is_some_and(char::is_uppercase) {
                let e = failures
                    .entry(format!("{:?} {} at 0: {}", r.contact, r.op, r.class))
                    .or_insert((0, r));
                e.0 += 1;
            }
            continue;
        }
        let key = coarse(&r.class);
        *rows
            .entry((r.contact, r.motion, r.op))
            .or_default()
            .entry(key.clone())
            .or_default() += 1;
        *totals.entry(key.clone()).or_default() += 1;
        *by_step
            .entry((r.step * 4.0) as i64)
            .or_default()
            .entry(key)
            .or_default() += 1;
        if r.class.chars().next().is_some_and(char::is_uppercase) {
            let e = failures
                .entry(format!(
                    "{:?} {} {:?}: {}",
                    r.contact, r.op, r.motion, r.class
                ))
                .or_insert((0, r));
            e.0 += 1;
            if r.step.abs() < e.1.step.abs() {
                e.1 = r;
            }
        }
        if r.ratio.is_some_and(|x| x > 1.0) {
            jumps.push(r);
        }
    }
    let _ = writeln!(
        out,
        "pairs: {pairs}, booleans off the contact: {}",
        totals.values().sum::<usize>()
    );
    let _ = writeln!(out, "\ntotals:");
    for (k, n) in &totals {
        let _ = writeln!(out, "  {n:6}  {k}");
    }
    let _ = writeln!(out, "\nat offset zero, per contact and boolean:");
    for ((c, op), classes) in &own {
        let _ = writeln!(out, "  {c:?} {op}: {}", line(classes));
    }
    let _ = writeln!(out, "\noff the contact, per contact, motion and boolean:");
    for ((c, mo, op), classes) in &rows {
        let _ = writeln!(out, "  {c:?} {mo:?} {op}: {}", line(classes));
    }
    let _ = writeln!(out, "\nper step (in tolerances):");
    for (s, classes) in &by_step {
        let _ = writeln!(out, "  {:+6.2}: {}", *s as f64 / 4.0, line(classes));
    }
    let mut by_mechanism: BTreeMap<&str, usize> = BTreeMap::new();
    for r in records {
        if r.class.chars().next().is_some_and(char::is_uppercase) {
            *by_mechanism.entry(mechanism(&r.class)).or_default() += 1;
        }
    }
    let _ = writeln!(out, "\nfailures by the plan step whose mechanism they are:");
    for (k, n) in &by_mechanism {
        let _ = writeln!(out, "  {n:6}  {k}");
    }
    let _ = writeln!(
        out,
        "\nfailures, each with its count, its step's mechanism and the pair at the smallest offset as a recipe:"
    );
    for (k, (n, r)) in &failures {
        let _ = writeln!(
            out,
            "  {n:5}  [{}] {k}  (offset {})\n{}",
            mechanism(k),
            r.step,
            recipe("", &r.pair, r.op, &[("default", r.motion, r.step)])
        );
    }
    jumps.sort_by(|a, b| {
        b.ratio
            .partial_cmp(&a.ratio)
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    let _ = writeln!(out, "\nvolume past its bound: {}", jumps.len());
    for r in jumps.iter().take(12) {
        let _ = writeln!(
            out,
            "  {:.3e}×  {:?} {:?} {} step {}: {}\n{}",
            r.ratio.unwrap_or(0.0),
            r.contact,
            r.motion,
            r.op,
            r.step,
            r.class,
            recipe("", &r.pair, r.op, &[("default", r.motion, r.step)])
        );
    }
    out
}

/// A class without its detail, for the tables: a failure's kind is kept,
/// its message is not.
fn coarse(class: &str) -> String {
    if class.starts_with("CHECKER") || class.starts_with("PANIC") {
        class
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        class.to_string()
    }
}

/// `classes` as `name ×n, …`.
fn line(classes: &BTreeMap<String, usize>) -> String {
    classes
        .iter()
        .map(|(k, n)| format!("{k} ×{n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Each contact at its fixture's own numbers, unposed, under every motion
/// that moves it: where a failure of the survey is shrunk to.
fn fixture_pairs() -> Vec<BandPair> {
    use arris_debug::prop::body::BandSolid;
    use arris_ops::arris_check::arris_topo::arris_math::{Axis, Isometry, Point3, Vec3};
    let p = Point3::new;
    let rod = |at: Point3, d: Vec3, radius: f64, height: f64| BandSolid::Rod {
        axis: Axis::new(at, d).expect("a unit axis"),
        radius,
        height,
    };
    let block = |min: Point3, max: Point3| BandSolid::Block { min, max };
    let (x, y, z) = (Vec3::x(), Vec3::y(), Vec3::z());
    let s = core::f64::consts::FRAC_1_SQRT_2;
    let pi = core::f64::consts::PI;
    let all = [BandMotion::Offset, BandMotion::Tilt, BandMotion::Radius];
    let pair = |contact, fixed, moving, normal, pivot, hinge, lever, area, line| BandPair {
        contact,
        motion: BandMotion::Offset,
        fixed,
        moving,
        normal,
        pivot,
        hinge,
        lever,
        area,
        line,
        pose: Isometry::identity(),
    };
    let plate = block(p(0.0, 0.0, 0.0), p(40.0, 30.0, 10.0));
    let bases = [
        pair(
            BandContact::FlushBoxes,
            plate,
            block(p(40.0, 0.0, 0.0), p(80.0, 30.0, 10.0)),
            -x,
            p(40.0, 15.0, 5.0),
            y,
            10.0,
            300.0,
            None,
        ),
        pair(
            BandContact::FlushBoss,
            plate,
            rod(p(20.0, 15.0, 10.0), z, 4.0, 10.0),
            -z,
            p(20.0, 15.0, 10.0),
            x,
            8.0,
            16.0 * pi,
            None,
        ),
        pair(
            BandContact::PinInBore,
            rod(p(0.0, 0.0, 0.0), z, 2.0, 4.0),
            rod(p(0.0, 1.0, 1.0), z, 1.0, 2.0),
            y,
            p(0.0, 2.0, 2.0),
            x,
            2.0,
            0.0,
            Some((2.0, 2.0)),
        ),
        pair(
            BandContact::CoaxialRod,
            BandSolid::Tube {
                axis: Axis::z_at(p(0.0, 0.0, 0.0)),
                outer: 2.0,
                bore: 1.0,
                height: 2.0,
            },
            rod(p(0.0, 0.0, 0.0), z, 1.0, 2.0),
            x,
            p(0.0, 0.0, 1.0),
            y,
            2.0,
            4.0 * pi,
            None,
        ),
        pair(
            BandContact::CoaxialBore,
            rod(p(0.0, 0.0, 0.0), z, 2.0, 2.0),
            rod(p(0.0, 0.0, -1.0), z, 1.0, 4.0),
            x,
            p(0.0, 0.0, 1.0),
            y,
            2.0,
            0.0,
            None,
        ),
        pair(
            BandContact::TangentCylinders,
            rod(p(0.0, 0.0, 0.0), z, 1.0, 2.0),
            rod(p(0.0, 2.0, -1.0), z, 1.0, 4.0),
            -y,
            p(0.0, 1.0, 1.0),
            x,
            2.0,
            0.0,
            Some((2.0, 0.5)),
        ),
        pair(
            BandContact::TangentHole,
            plate,
            rod(p(3.0, 15.0, 4.0), z, 3.0, 12.0),
            -x,
            p(0.0, 15.0, 7.0),
            y,
            6.0,
            0.0,
            Some((6.0, 3.0)),
        ),
        pair(
            BandContact::TangentOutside,
            plate,
            rod(p(44.0, 15.0, -1.0), z, 4.0, 12.0),
            -x,
            p(40.0, 15.0, 5.0),
            y,
            10.0,
            0.0,
            Some((10.0, 4.0)),
        ),
        pair(
            BandContact::PipeElbow,
            BandSolid::Bend {
                major: 3.0,
                minor: 1.0,
            },
            rod(p(3.0, 0.0, 0.0), -y, 1.0, 4.0),
            y,
            p(3.0, 0.0, 0.0),
            z,
            2.0,
            pi,
            None,
        ),
        pair(
            BandContact::EdgeOnEdge,
            block(p(0.0, 0.0, 0.0), p(10.0, 10.0, 10.0)),
            block(p(10.0, 10.0, 0.0), p(20.0, 20.0, 10.0)),
            Vec3::new(-s, -s, 0.0),
            p(10.0, 10.0, 5.0),
            Vec3::new(s, -s, 0.0),
            10.0,
            0.0,
            Some((10.0, 0.0)),
        ),
    ];
    let mut out = Vec::new();
    for base in bases {
        let grows = !matches!(
            base.contact,
            BandContact::FlushBoxes
                | BandContact::FlushBoss
                | BandContact::CoaxialBore
                | BandContact::EdgeOnEdge
        );
        for motion in all {
            if motion != BandMotion::Radius || grows {
                out.push(BandPair { motion, ..base });
            }
        }
    }
    out
}

/// The band at the fixtures' own numbers: per pair and boolean, the class
/// at every step, the contact's own first.
#[test]
#[ignore = "the band survey records and does not assert: plans/c3-tolerance-apart step 9 makes it"]
fn the_band_at_the_fixtures_numbers() {
    std::panic::set_hook(Box::new(|_| {}));
    let pairs = fixture_pairs();
    let lines = Mutex::new(BTreeMap::new());
    std::thread::scope(|scope| {
        for (i, pair) in pairs.iter().enumerate() {
            let lines = &lines;
            scope.spawn(move || {
                let records = survey(pair);
                let mut text = String::new();
                for op in ["fuse", "common", "cut"] {
                    let cells: Vec<String> = records
                        .iter()
                        .filter(|r| r.op == op)
                        .map(|r| format!("{:+}:{}", r.step, r.class))
                        .collect();
                    text += &format!(
                        "{:?} {:?} {op}: {}\n",
                        pair.contact,
                        pair.motion,
                        cells.join(" | ")
                    );
                }
                lines.lock().expect("no poisoned survey").insert(i, text);
            });
        }
    });
    let _ = std::panic::take_hook();
    for text in lines.into_inner().expect("no poisoned survey").values() {
        print!("{text}");
    }
}

/// A number as the recipe grammar reads it.
fn num(x: f64) -> String {
    let x = if x == 0.0 { 0.0 } else { x };
    format!("{x}")
}

/// A point or a vector as a recipe array.
fn arr(v: [f64; 3]) -> String {
    format!("[{}, {}, {}]", num(v[0]), num(v[1]), num(v[2]))
}

/// The recipe steps that build `solid` under the name `name`, its radius
/// grown by the parameter `g` where `grow`.
fn solid_steps(name: &str, solid: &arris_debug::prop::body::BandSolid, grow: bool) -> Vec<String> {
    use arris_debug::prop::body::BandSolid;
    let radius = |r: f64| {
        if grow {
            format!("\"{} + g\"", num(r))
        } else {
            num(r)
        }
    };
    let cylinder = |name: &str, base: [f64; 3], axis: [f64; 3], r: String, h: f64| {
        format!(
            "{{\"name\": \"{name}\", \"op\": \"cylinder\", \"base\": {}, \"axis\": {}, \"radius\": {r}, \"height\": {}}}",
            arr(base),
            arr(axis),
            num(h)
        )
    };
    let v = |p: arris_ops::arris_check::arris_topo::arris_math::Point3| [p.x, p.y, p.z];
    match *solid {
        BandSolid::Block { min, max } => vec![format!(
            "{{\"name\": \"{name}\", \"op\": \"box\", \"min\": {}, \"max\": {}}}",
            arr(v(min)),
            arr(v(max))
        )],
        BandSolid::Rod {
            axis,
            radius: r,
            height,
        } => {
            let d = axis.direction.into_inner();
            vec![cylinder(
                name,
                v(axis.origin),
                [d.x, d.y, d.z],
                radius(r),
                height,
            )]
        }
        BandSolid::Tube {
            axis,
            outer,
            bore,
            height,
        } => {
            let d = axis.direction.into_inner();
            vec![
                cylinder(
                    &format!("{name}_wall"),
                    v(axis.origin),
                    [d.x, d.y, d.z],
                    num(outer),
                    height,
                ),
                cylinder(
                    &format!("{name}_bore"),
                    v(axis.at(-1.0)),
                    [d.x, d.y, d.z],
                    radius(bore),
                    height + 2.0,
                ),
                format!(
                    "{{\"name\": \"{name}\", \"op\": \"cut\", \"target\": \"{name}_wall\", \"tool\": \"{name}_bore\"}}"
                ),
            ]
        }
        BandSolid::Bend { major, minor } => vec![
            format!(
                "{{\"name\": \"{name}_disc\", \"op\": \"profile\", \"plane\": {{\"origin\": [0, 0, 0], \"x\": [1, 0, 0], \"y\": [0, 0, 1]}}, \"outer\": {{\"circle\": {{\"center\": [{}, 0], \"radius\": {}}}}}}}",
                num(major),
                radius(minor)
            ),
            format!(
                "{{\"name\": \"{name}\", \"op\": \"revolve\", \"profile\": \"{name}_disc\", \"axis\": {{\"origin\": [0, 0, 0], \"direction\": [0, 0, 1]}}, \"angle_deg\": 90}}"
            ),
        ],
    }
}

/// `pair` as a fixture recipe of `op`, one variant per `(name, motion,
/// step)` — the first the default — its offset `d`, tilt reach `t` and
/// radius growth `g` the parameters, the pose baked in: how a failure of
/// the survey becomes a `regression/` fixture.
fn recipe(
    description: &str,
    pair: &BandPair,
    op: &str,
    variants: &[(&str, BandMotion, f64)],
) -> String {
    let tol = Model::default().precision().default_tolerance;
    let params = |motion: BandMotion, step: f64| {
        let at = |m: BandMotion| num(if m == motion { step * tol } else { 0.0 });
        format!(
            "{{\"d\": {}, \"t\": {}, \"g\": {}}}",
            at(BandMotion::Offset),
            at(BandMotion::Tilt),
            at(BandMotion::Radius)
        )
    };
    let grows = variants.iter().any(|v| v.1 == BandMotion::Radius);
    let mut steps = solid_steps("a", &pair.fixed, false);
    steps.extend(solid_steps("b0", &pair.moving, grows));
    let (n, h, c) = (pair.normal, pair.hinge, pair.pivot);
    steps.push(format!(
        "{{\"name\": \"b\", \"op\": \"transform\", \"of\": \"b0\", \"rotate\": {{\"axis\": {}, \"origin\": {}, \"angle_deg\": \"degrees(2 * t / {})\"}}, \"translate\": [\"{} * d\", \"{} * d\", \"{} * d\"]}}",
        arr([h.x, h.y, h.z]),
        arr([c.x, c.y, c.z]),
        num(pair.lever),
        num(n.x),
        num(n.y),
        num(n.z)
    ));
    let (mut first, mut second) = ("a".to_string(), "b".to_string());
    if pair.pose != arris_ops::arris_check::arris_topo::arris_math::Isometry::identity() {
        let q = pair.pose.rotation();
        let (axis, angle) = q
            .axis_angle()
            .map_or(([0.0, 0.0, 1.0], 0.0), |(a, t)| ([a.x, a.y, a.z], t));
        let tr = pair.pose.translation();
        for (from, to) in [("a", "a_posed"), ("b", "b_posed")] {
            steps.push(format!(
                "{{\"name\": \"{to}\", \"op\": \"transform\", \"of\": \"{from}\", \"rotate\": {{\"axis\": {}, \"angle_deg\": {}}}, \"translate\": {}}}",
                arr(axis),
                num(angle.to_degrees()),
                arr([tr.x, tr.y, tr.z])
            ));
        }
        (first, second) = ("a_posed".into(), "b_posed".into());
    }
    steps.push(match op {
        "cut" => format!("{{\"name\": \"result\", \"op\": \"cut\", \"target\": \"{first}\", \"tool\": \"{second}\"}}"),
        _ => format!("{{\"name\": \"result\", \"op\": \"{op}\", \"a\": \"{first}\", \"b\": \"{second}\"}}"),
    });
    let (default, rest) = variants.split_first().expect("a variant at least");
    let rest: Vec<String> = rest
        .iter()
        .map(|(name, motion, step)| format!("\"{name}\": {}", params(*motion, *step)))
        .collect();
    let variants = if rest.is_empty() {
        String::new()
    } else {
        format!("  \"variants\": {{\n    {}\n  }},\n", rest.join(",\n    "))
    };
    format!(
        "{{\n  \"description\": {:?},\n  \"params\": {},\n{variants}  \"steps\": [\n    {}\n  ],\n  \"result\": \"result\",\n  \"analytic\": {{}}\n}}\n",
        description,
        params(default.1, default.2),
        steps.join(",\n    ")
    )
}
