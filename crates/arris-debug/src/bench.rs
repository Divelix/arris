//! The benchmark timer (ADR-0024 §4): a warm-up, a fixed number of timed
//! iterations, their median and median absolute deviation, a JSON
//! [`Report`], and a [`compare`] against a saved one that prints each
//! case's ratio.
//!
//! It exists to catch a change that costs 10× — a robustness fix that
//! turns a closed form into a march — not a 5% drift, so it is in-house
//! and has no statistics beyond the median: shared runners and a busy
//! workstation move a timing by tens of percent, and a finer estimate of
//! a number that noisy buys nothing at that threshold. Time is never a
//! gate: a ratio past [`RATIO_FLAG`] is flagged, and nothing fails on it.

use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// The ratio against a saved report past which [`Comparison::flagged`]
/// holds. 3×, not 10×: the regressions the harness is for are 10× and
/// clear 3× on any machine, while run-to-run noise on a shared runner —
/// another job on the host, a cold page cache — stays under 2×, so 3× is
/// the lowest flag that noise alone does not raise.
pub const RATIO_FLAG: f64 = 3.0;

/// Where a timing comes from: the wall clock in a benchmark, a scripted
/// sequence in a test of the timer.
pub trait Clock {
    /// The time since an arbitrary fixed point.
    fn now(&mut self) -> Duration;
}

/// The monotonic wall clock.
#[derive(Debug, Clone, Copy)]
pub struct Wall {
    start: Instant,
}

impl Wall {
    /// A clock reading zero now.
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for Wall {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for Wall {
    fn now(&mut self) -> Duration {
        self.start.elapsed()
    }
}

/// How a case is timed: `warmup` untimed runs, then `iterations` timed
/// ones. `iterations` is at least one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Runs before timing, to fault in the code and the allocator's pages.
    pub warmup: usize,
    /// Timed runs.
    pub iterations: usize,
}

impl Default for Config {
    /// One warm-up and five timed runs: a median of five survives two
    /// outliers, and the whole corpus stays a few minutes.
    fn default() -> Self {
        Self {
            warmup: 1,
            iterations: 5,
        }
    }
}

/// One case's timing, in seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    /// The case's name, unique in its report: what [`compare`] matches on.
    pub name: String,
    /// The number of timed runs.
    pub iterations: usize,
    /// The median of the timed runs.
    pub median: f64,
    /// The median absolute deviation of the timed runs from [`Case::median`].
    pub mad: f64,
}

/// Times `f` under `config` on `clock`: the median and the median
/// absolute deviation of the timed runs, each run measured alone.
///
/// ```
/// use arris_debug::bench::{Config, Wall, time};
///
/// let case = time("sum", Config::default(), &mut Wall::new(), || {
///     std::hint::black_box((0..1000u64).sum::<u64>());
/// });
/// assert_eq!(case.iterations, 5);
/// assert!(case.median >= 0.0 && case.mad >= 0.0);
/// ```
pub fn time(name: &str, config: Config, clock: &mut impl Clock, mut f: impl FnMut()) -> Case {
    for _ in 0..config.warmup {
        f();
    }
    let iterations = config.iterations.max(1);
    let mut runs: Vec<f64> = (0..iterations)
        .map(|_| {
            let start = clock.now();
            f();
            clock.now().saturating_sub(start).as_secs_f64()
        })
        .collect();
    let median = median_of(&mut runs);
    let mut deviations: Vec<f64> = runs.iter().map(|r| (r - median).abs()).collect();
    Case {
        name: name.to_string(),
        iterations,
        median,
        mad: median_of(&mut deviations),
    }
}

/// The median of a non-empty sample, the mean of the middle two for an
/// even count.
fn median_of(xs: &mut [f64]) -> f64 {
    xs.sort_by(f64::total_cmp);
    let n = xs.len();
    if n == 0 {
        return 0.0;
    }
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        0.5 * (xs[n / 2 - 1] + xs[n / 2])
    }
}

/// Every case of one benchmark run, in the order they ran.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// The cases.
    pub cases: Vec<Case>,
}

impl Report {
    /// The sum of every case's median: the one number a run is quoted by.
    pub fn total(&self) -> f64 {
        self.cases.iter().map(|c| c.median).sum()
    }

    /// Writes the report as pretty JSON.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text + "\n")
    }

    /// Reads a report [`Report::save`] wrote.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(std::io::Error::other)
    }
}

/// One case against the same case of a saved report.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// The case's name.
    pub name: String,
    /// The saved median, or `None` for a case the saved report lacks.
    pub before: Option<f64>,
    /// The new median.
    pub after: f64,
}

impl Comparison {
    /// `after / before`; `None` for a new case or a saved median of zero.
    pub fn ratio(&self) -> Option<f64> {
        self.before
            .filter(|b| *b > 0.0)
            .map(|before| self.after / before)
    }

    /// Whether the case got slower by more than [`RATIO_FLAG`].
    pub fn flagged(&self) -> bool {
        self.ratio().is_some_and(|r| r > RATIO_FLAG)
    }
}

/// Every case of `after` against `before` by name, in `after`'s order.
///
/// ```
/// use arris_debug::bench::{Case, Report, compare};
///
/// let case = |median| Case { name: "a".into(), iterations: 5, median, mad: 0.0 };
/// let before = Report { cases: vec![case(1.0)] };
/// let after = Report { cases: vec![case(4.0)] };
/// let c = &compare(&before, &after)[0];
/// assert_eq!(c.ratio(), Some(4.0));
/// assert!(c.flagged());
/// ```
pub fn compare(before: &Report, after: &Report) -> Vec<Comparison> {
    after
        .cases
        .iter()
        .map(|c| Comparison {
            name: c.name.clone(),
            before: before
                .cases
                .iter()
                .find(|b| b.name == c.name)
                .map(|b| b.median),
            after: c.median,
        })
        .collect()
}

/// The comparison as a markdown table, flagged cases marked, and the two
/// totals over the cases both reports hold.
pub fn comparison_table(comparisons: &[Comparison]) -> String {
    let mut out = String::from("| Case | Before (s) | After (s) | Ratio |\n|---|---|---|---|\n");
    let (mut before_total, mut after_total) = (0.0, 0.0);
    for c in comparisons {
        let before = c.before.map_or("—".to_string(), |b| format!("{b:.4}"));
        let ratio = match c.ratio() {
            Some(r) if c.flagged() => format!("**{r:.2}× ⚠**"),
            Some(r) => format!("{r:.2}×"),
            None => "new".to_string(),
        };
        out += &format!("| `{}` | {before} | {:.4} | {ratio} |\n", c.name, c.after);
        if let Some(b) = c.before {
            before_total += b;
            after_total += c.after;
        }
    }
    let flagged = comparisons.iter().filter(|c| c.flagged()).count();
    out += &format!(
        "\nTotal over the shared cases: {before_total:.3} s → {after_total:.3} s. \
         {flagged} case(s) past {RATIO_FLAG}×.\n"
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clock that returns the scripted readings in turn.
    struct Scripted(std::vec::IntoIter<f64>);

    impl Clock for Scripted {
        fn now(&mut self) -> Duration {
            Duration::from_secs_f64(self.0.next().expect("the script ran out"))
        }
    }

    #[test]
    fn the_median_and_its_deviation_come_from_the_timed_runs_only() {
        // Five runs of 1, 2, 3, 10 and 4 seconds: two readings each.
        let readings = vec![0.0, 1.0, 1.0, 3.0, 3.0, 6.0, 6.0, 16.0, 16.0, 20.0];
        let mut clock = Scripted(readings.into_iter());
        let mut calls = 0;
        let config = Config {
            warmup: 2,
            iterations: 5,
        };
        let case = time("scripted", config, &mut clock, || calls += 1);
        // The warm-up runs read no clock; every run calls `f`.
        assert_eq!(calls, 7);
        assert_eq!(case.iterations, 5);
        assert_eq!(case.median, 3.0);
        // Deviations 2, 1, 0, 7, 1: their median is 1.
        assert_eq!(case.mad, 1.0);
    }

    #[test]
    fn an_even_count_takes_the_middle_pair() {
        let readings = vec![0.0, 1.0, 1.0, 3.0, 3.0, 6.0, 6.0, 10.0];
        let config = Config {
            warmup: 0,
            iterations: 4,
        };
        let case = time("even", config, &mut Scripted(readings.into_iter()), || {});
        // Runs 1, 2, 3, 4.
        assert_eq!(case.median, 2.5);
        assert_eq!(case.mad, 1.0);
    }

    #[test]
    fn a_comparison_flags_past_the_ratio_and_names_new_cases() {
        let case = |name: &str, median| Case {
            name: name.into(),
            iterations: 5,
            median,
            mad: 0.0,
        };
        let before = Report {
            cases: vec![case("a", 1.0), case("b", 1.0)],
        };
        let after = Report {
            cases: vec![case("a", 2.9), case("b", 3.1), case("c", 1.0)],
        };
        let c = compare(&before, &after);
        assert_eq!(
            c.iter().map(Comparison::flagged).collect::<Vec<_>>(),
            [false, true, false]
        );
        assert_eq!(c[2].ratio(), None);
        let table = comparison_table(&c);
        assert!(table.contains("| `c` | — | 1.0000 | new |"), "{table}");
        assert!(table.contains("1 case(s) past 3×"), "{table}");
    }

    #[test]
    fn a_report_round_trips_through_its_file() {
        let report = Report {
            cases: vec![Case {
                name: "boolean/through-hole build".into(),
                iterations: 5,
                median: 0.012,
                mad: 0.001,
            }],
        };
        let dir = std::env::temp_dir().join(format!("arris-bench-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.json");
        report.save(&path).unwrap();
        assert_eq!(Report::load(&path).unwrap(), report);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
