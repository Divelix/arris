//! The checker's answer.

use core::fmt;

use crate::unchecked::Unchecked;
use crate::violation::{Level, Violation};

/// The Euler–Poincaré line of a body (`docs/DATA-MODEL.md`
/// §Euler–Poincaré): the five counts of its closure and the genus they
/// imply through `V − E + F − (L − F) − 2(S − G) = 0`. The genus is
/// *derived*, as the oracle derives it, so the line is not a violation on
/// its own; what it checks is its parity — a count set that leaves a
/// [`EulerLine::residual`] of one cannot come from any closed orientable
/// surface, whatever its genus. A degenerate edge is not counted: it is a
/// singular point of its surface (a cone's apex, a sphere's pole), not a
/// boundary between faces — S2's reading of it — and counting it would
/// give a sphere genus 1 and a cone an odd line.
///
/// ```
/// use arris_check::{Level, check};
/// use arris_debug::sample;
/// use arris_topo::Model;
///
/// let mut m = Model::default();
/// let body = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
/// let line = check(&m, body, Level::Fast).euler().unwrap();
/// assert_eq!(line.to_string(), "2/3/3/3/1 g0 = 0");
/// assert!(line.closes());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EulerLine {
    /// Vertices in the closure.
    pub vertices: usize,
    /// Edges in the closure that are not degenerate.
    pub edges: usize,
    /// Faces in the closure.
    pub faces: usize,
    /// Loops over those faces.
    pub loops: usize,
    /// Shells in the closure.
    pub shells: usize,
    /// The genus the counts imply, `S − ⌊(V − E + 2F − L) / 2⌋`.
    pub genus: i64,
}

impl EulerLine {
    /// The line of the counts, with the genus derived from them.
    pub const fn new(
        vertices: usize,
        edges: usize,
        faces: usize,
        loops: usize,
        shells: usize,
    ) -> Self {
        // V − E + F − (L − F) − 2(S − G) = 0  ⇒  2G = 2S − (V − E + 2F − L).
        let x = Self::characteristic(vertices, edges, faces, loops);
        EulerLine {
            vertices,
            edges,
            faces,
            loops,
            shells,
            genus: shells as i64 - x.div_euclid(2),
        }
    }

    const fn characteristic(vertices: usize, edges: usize, faces: usize, loops: usize) -> i64 {
        vertices as i64 - edges as i64 + 2 * faces as i64 - loops as i64
    }

    /// What the counts leave once the genus is taken out: `0` for a line
    /// that closes, `1` for one that cannot come from any genus.
    pub const fn residual(&self) -> i64 {
        Self::characteristic(self.vertices, self.edges, self.faces, self.loops).rem_euclid(2)
    }

    /// `true` when [`EulerLine::residual`] is zero.
    pub const fn closes(&self) -> bool {
        self.residual() == 0
    }
}

impl fmt::Display for EulerLine {
    /// `V/E/F/L/S g<genus> = <residual>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{}/{}/{}/{} g{} = {}",
            self.vertices,
            self.edges,
            self.faces,
            self.loops,
            self.shells,
            self.genus,
            self.residual()
        )
    }
}

/// Every violation the checker found, in a deterministic order: by
/// entity (kind, then id), then by invariant code, then by content. Two
/// runs over the same model print the same report byte for byte.
///
/// `Report::is_ok()` is what every test asserts and what every operation
/// asserts on its own output before returning `Ok`. It speaks for the
/// violations alone: a `Full` row the kernel could not decide is listed
/// by [`Report::unchecked`], printed with a `?` after its code, and is
/// neither a violation nor a silent pass.
///
/// ```
/// use arris_check::{Report, Violation};
/// use arris_topo::EdgeId;
///
/// let mut report = Report::default();
/// assert!(report.is_ok());
/// report.push(Violation::EdgeUnused { edge: EdgeId::new(2, 0) });
/// assert!(!report.is_ok());
/// assert_eq!(report.to_string(), "E3 e2: used by no coedge and not a free edge\n");
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    violations: Vec<Violation>,
    euler: Option<EulerLine>,
    unchecked: Vec<Unchecked>,
}

impl Report {
    /// A report holding `violations`, sorted into report order.
    pub fn new(mut violations: Vec<Violation>) -> Self {
        violations.sort_by(Self::order);
        Report {
            violations,
            euler: None,
            unchecked: Vec::new(),
        }
    }

    /// The same report carrying `line` as its Euler–Poincaré line.
    pub fn with_euler(mut self, line: EulerLine) -> Self {
        self.euler = Some(line);
        self
    }

    /// The body's Euler–Poincaré line, or `None` when the body did not
    /// resolve and there were no counts to take. It is a line, never a
    /// violation: a body whose line does not close still reports `is_ok`
    /// unless a row was broken as well.
    pub const fn euler(&self) -> Option<EulerLine> {
        self.euler
    }

    /// The same report carrying `unchecked`, sorted into report order.
    pub fn with_unchecked(mut self, mut unchecked: Vec<Unchecked>) -> Self {
        unchecked.sort_by(|a, b| {
            a.entity()
                .cmp(&b.entity())
                .then_with(|| a.code().cmp(b.code()))
                .then_with(|| a.cmp(b))
        });
        unchecked.dedup();
        self.unchecked = unchecked;
        self
    }

    /// The `Full` rows the checker could not decide on this body, in
    /// report order. Empty for a body every row could be decided on; a
    /// non-empty list is not a failure, and not a pass either.
    pub fn unchecked(&self) -> &[Unchecked] {
        &self.unchecked
    }

    /// `true` when nothing was violated.
    pub fn is_ok(&self) -> bool {
        self.violations.is_empty()
    }

    /// Adds a violation, keeping the report sorted.
    pub fn push(&mut self, violation: Violation) {
        let at = self
            .violations
            .partition_point(|v| Self::order(v, &violation).is_lt());
        self.violations.insert(at, violation);
    }

    /// The violations in report order.
    pub fn violations(&self) -> &[Violation] {
        &self.violations
    }

    /// Number of violations.
    pub fn len(&self) -> usize {
        self.violations.len()
    }

    /// `true` when there are no violations; the same as [`Report::is_ok`].
    pub fn is_empty(&self) -> bool {
        self.violations.is_empty()
    }

    /// The violations of invariants that run at `level` or below: `Fast`
    /// keeps only the fast rows, `Full` keeps everything.
    pub fn at_level(&self, level: Level) -> Report {
        Report {
            violations: self
                .violations
                .iter()
                .filter(|v| v.level() <= level)
                .cloned()
                .collect(),
            euler: self.euler,
            unchecked: if level == Level::Full {
                self.unchecked.clone()
            } else {
                Vec::new()
            },
        }
    }

    fn order(a: &Violation, b: &Violation) -> core::cmp::Ordering {
        a.entity()
            .cmp(&b.entity())
            .then_with(|| a.code().cmp(b.code()))
            .then_with(|| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
            .then_with(|| format!("{a:?}").cmp(&format!("{b:?}")))
    }
}

impl fmt::Display for Report {
    /// One line per violation, in report order, then one per undecided
    /// row; empty for an ok report that decided everything.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for v in &self.violations {
            writeln!(f, "{v}")?;
        }
        for u in &self.unchecked {
            writeln!(f, "{u}")?;
        }
        Ok(())
    }
}

impl IntoIterator for Report {
    type Item = Violation;
    type IntoIter = std::vec::IntoIter<Violation>;

    fn into_iter(self) -> Self::IntoIter {
        self.violations.into_iter()
    }
}

impl<'a> IntoIterator for &'a Report {
    type Item = &'a Violation;
    type IntoIter = core::slice::Iter<'a, Violation>;

    fn into_iter(self) -> Self::IntoIter {
        self.violations.iter()
    }
}

impl FromIterator<Violation> for Report {
    fn from_iter<I: IntoIterator<Item = Violation>>(iter: I) -> Self {
        Report::new(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::violation::{EdgeUseFault, ToleranceBound};
    use arris_topo::{EdgeId, FaceId, ShellId, VertexId};

    fn sample() -> Vec<Violation> {
        vec![
            Violation::EdgeUses {
                shell: ShellId::new(0, 0),
                edge: EdgeId::new(5, 0),
                fault: EdgeUseFault::Count { coedges: 3 },
            },
            Violation::EdgeUnused {
                edge: EdgeId::new(2, 0),
            },
            Violation::FaceTolerance {
                face: FaceId::new(1, 0),
                tolerance: 1e-3,
                bound: ToleranceBound::BelowMinimum,
            },
            Violation::VertexOffEdge {
                vertex: VertexId::new(4, 0),
                edge: EdgeId::new(2, 0),
                distance: 0.5,
            },
            Violation::EdgeRange {
                edge: EdgeId::new(2, 0),
            },
        ]
    }

    #[test]
    fn sorted_by_entity_then_code_whatever_the_insertion_order() {
        let a = Report::new(sample());
        let mut b = Report::default();
        for v in sample().into_iter().rev() {
            b.push(v);
        }
        let c: Report = sample().into_iter().collect();
        assert_eq!(a, b);
        assert_eq!(a, c);
        let codes: Vec<_> = a
            .violations()
            .iter()
            .map(|v| (v.entity().to_string(), v.code()))
            .collect();
        assert_eq!(
            codes,
            [
                ("v4".to_string(), "V2"),
                ("e2".to_string(), "E1"),
                ("e2".to_string(), "E3"),
                ("f1".to_string(), "F2"),
                ("s0".to_string(), "S2"),
            ]
        );
    }

    #[test]
    fn display_is_one_line_per_violation() {
        let text = Report::new(sample()).to_string();
        assert_eq!(
            text,
            "V2 v4: e2's curve ends 5e-1 away\n\
             E1 e2: no curve or an invalid range\n\
             E3 e2: used by no coedge and not a free edge\n\
             F2 f1: tolerance 1e-3 is below Precision::min_tolerance\n\
             S2 s0: e5 is used by 3 coedge(s)\n"
        );
        assert_eq!(Report::default().to_string(), "");
    }

    #[test]
    fn level_filter_keeps_fast_rows_only() {
        let full = Violation::LoopsIntersect {
            face: FaceId::new(0, 0),
            loop_a: 0,
            loop_b: 0,
        };
        let fast = Violation::EdgeUnused {
            edge: EdgeId::new(0, 0),
        };
        let r = Report::new(vec![full.clone(), fast.clone()]);
        assert_eq!(r.at_level(Level::Fast).violations(), [fast]);
        assert_eq!(r.at_level(Level::Full).len(), 2);
        assert_eq!(full.level(), Level::Full);
    }
}
