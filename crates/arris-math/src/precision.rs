//! Model-wide tolerance configuration.

/// The tolerance configuration of one model, set when the model is created
/// (`docs/DATA-MODEL.md` §Tolerances).
///
/// Arris carries no unit; `Precision` is what makes a model's numbers
/// meaningful. Every tolerance an algorithm uses is an entity's own or a
/// field of this struct — never a literal. The defaults are for a model
/// whose features are of order 1–1000 units; a consumer in metres sets
/// `default_tolerance` at the micrometre scale.
///
/// The fields are public: a `Precision` is plain configuration data, and
/// the checker's V1/F2 rows (`docs/DATA-MODEL.md` §Invariants) hold every
/// entity to the bounds it states.
///
/// ```
/// use arris_math::Precision;
///
/// let p = Precision { default_tolerance: 1e-6, ..Precision::DEFAULT };
/// assert!(p.min_tolerance <= p.default_tolerance);
/// assert!(p.default_tolerance <= p.max_tolerance);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Precision {
    /// The tolerance a primitive's entities are created with. Two points
    /// closer than this are the same point.
    pub default_tolerance: f64,
    /// The floor: no entity carries a tolerance below it. It bounds how
    /// far an operation may *tighten* a tolerance and is the smallest
    /// distance the model distinguishes.
    pub min_tolerance: f64,
    /// The ceiling: an operation whose result would need an entity
    /// tolerance above it returns an error instead of a sloppy body.
    pub max_tolerance: f64,
    /// Angle in radians below which two directions are parallel and two
    /// surfaces are tangent.
    pub angular_tolerance: f64,
    /// How far a pcurve may deviate in (u, v), at unit parametric speed —
    /// on a surface where one unit of parameter is one unit of length. The
    /// checker scales it by the surface's parametric derivative, so on a
    /// cylinder of radius `r` the bound in `u` is this divided by `r`.
    pub parametric_tolerance: f64,
    /// How many parameters the checker samples along an edge when it
    /// compares a pcurve's image against the 3D curve (invariant E4),
    /// both ends included.
    pub check_samples: usize,
}

impl Precision {
    /// The defaults, as a constant so they can be spread into a struct
    /// literal. Values: `default_tolerance` 1e-7 (Open CASCADE's
    /// `Precision::Confusion`, so the oracle and Arris merge the same
    /// points), `min_tolerance` 1e-12, `max_tolerance` 1e-2,
    /// `angular_tolerance` 1e-12 (Open CASCADE's `Precision::Angular`),
    /// `parametric_tolerance` 1e-7, `check_samples` 23 (the sample count of
    /// Open CASCADE's edge check, so an edge the oracle accepts is sampled
    /// at least as finely here).
    pub const DEFAULT: Precision = Precision {
        default_tolerance: 1e-7,
        min_tolerance: 1e-12,
        max_tolerance: 1e-2,
        angular_tolerance: 1e-12,
        parametric_tolerance: 1e-7,
        check_samples: 23,
    };

    /// `true` when the fields are finite, positive and ordered
    /// (`min_tolerance ≤ default_tolerance ≤ max_tolerance`) and there is at
    /// least one sample; a `Precision` that fails this is rejected when a
    /// model is created.
    pub fn is_consistent(&self) -> bool {
        let finite_positive = |x: f64| x.is_finite() && x > 0.0;
        finite_positive(self.min_tolerance)
            && finite_positive(self.default_tolerance)
            && finite_positive(self.max_tolerance)
            && finite_positive(self.angular_tolerance)
            && finite_positive(self.parametric_tolerance)
            && self.min_tolerance <= self.default_tolerance
            && self.default_tolerance <= self.max_tolerance
            && self.check_samples >= 1
    }
}

impl Default for Precision {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_consistent() {
        assert!(Precision::DEFAULT.is_consistent());
        assert_eq!(Precision::default(), Precision::DEFAULT);
    }

    #[test]
    fn disordered_or_non_finite_is_inconsistent() {
        let p = Precision {
            min_tolerance: 1.0,
            ..Precision::DEFAULT
        };
        assert!(!p.is_consistent());
        let p = Precision {
            max_tolerance: f64::NAN,
            ..Precision::DEFAULT
        };
        assert!(!p.is_consistent());
        let p = Precision {
            check_samples: 0,
            ..Precision::DEFAULT
        };
        assert!(!p.is_consistent());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip() {
        let p = Precision {
            default_tolerance: 1e-6,
            ..Precision::DEFAULT
        };
        let text = serde_json::to_string(&p).unwrap();
        let back: Precision = serde_json::from_str(&text).unwrap();
        assert_eq!(p, back);
    }
}
