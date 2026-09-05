//! The checker's answer.

use core::fmt;

use crate::violation::{Level, Violation};

/// Every violation the checker found, in a deterministic order: by
/// entity (kind, then id), then by invariant code, then by content. Two
/// runs over the same model print the same report byte for byte.
///
/// `Report::is_ok()` is what every test asserts and what every operation
/// asserts on its own output before returning `Ok`.
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
}

impl Report {
    /// A report holding `violations`, sorted into report order.
    pub fn new(mut violations: Vec<Violation>) -> Self {
        violations.sort_by(Self::order);
        Report { violations }
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
    /// One line per violation, in report order; empty for an ok report.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for v in &self.violations {
            writeln!(f, "{v}")?;
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
