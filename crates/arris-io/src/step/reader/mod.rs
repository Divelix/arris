//! The STEP reader (ADR-0025): what a parsed exchange structure
//! ([`super::part21`]) means, entity by entity. This module holds the
//! reader's public vocabulary — the options a caller reads with, and the
//! typed refusal each solid the reader cannot take comes back as — and its
//! layers: [`entities`] resolves references and reads parameters by the
//! schema's types, [`units`] reads a representation context's units and
//! converts every length and angle to the caller's, and [`geometry`] maps
//! each curve and surface onto its Arris variant or refuses it by name.

// Nothing outside the tests reaches the layers until `step::read` does
// (plans/step-reader step 10), which lifts these allowances.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod entities;
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod geometry;
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod units;

use core::fmt;

/// How the reader reads a file: the unit the caller's model is in.
///
/// ```
/// use arris_io::step::{LengthUnit, ReadOptions};
///
/// let options = ReadOptions::default();
/// assert_eq!(options.length_unit, LengthUnit::Millimetre);
/// let inches = ReadOptions { length_unit: LengthUnit::Inch };
/// assert_ne!(inches, options);
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReadOptions {
    /// The unit every length the reader returns is in: a file in any
    /// other converts to it (ADR-0025 §5). Millimetres by default, the
    /// unit [`write`](super::write) declares.
    pub length_unit: LengthUnit,
}

/// A unit of length a caller's model is in (ADR-0025 §5).
///
/// Guarantees: the conversion between two units is their exact ratio
/// where it is a representable number — millimetres to millimetres is
/// `1`, inches to millimetres `25.4`, metres to millimetres `1000` — and
/// the nearest `f64` to it otherwise.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LengthUnit {
    /// 10⁻⁶ m.
    Micrometre,
    /// 10⁻³ m.
    #[default]
    Millimetre,
    /// 10⁻² m.
    Centimetre,
    /// 1 m.
    Metre,
    /// 25.4 mm.
    Inch,
    /// 304.8 mm.
    Foot,
}

impl LengthUnit {
    #[cfg_attr(not(test), allow(dead_code))]
    /// The unit as `mantissa · 10^exponent` metres, so that a ratio of two
    /// units divides the mantissas and adds the exponents: exact wherever
    /// the ratio is.
    pub(crate) fn scale(self) -> units::Scale {
        let (mantissa, exponent) = match self {
            LengthUnit::Micrometre => (1.0, -6),
            LengthUnit::Millimetre => (1.0, -3),
            LengthUnit::Centimetre => (1.0, -2),
            LengthUnit::Metre => (1.0, 0),
            LengthUnit::Inch => (25.4, -3),
            LengthUnit::Foot => (304.8, -3),
        };
        units::Scale { mantissa, exponent }
    }
}

/// Why the reader did not return a solid, naming the file entity
/// (`#id`) where it stopped (ADR-0025 §2). One refused solid never hides
/// another: a refusal is per solid, and only a parse error fails a file.
///
/// Guarantees: every refusal names an instance of the file; [`kind`]
/// is its fieldless twin, which a histogram counts.
///
/// [`kind`]: Refusal::kind
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Refusal {
    /// A representation context declares no length unit, so no length in
    /// it has a size.
    #[error("#{context}: the context declares no length unit")]
    NoLengthUnit {
        /// The context.
        context: u64,
    },
    /// An `OFFSET_SURFACE` or an `OFFSET_CURVE_3D`: Arris has no offset
    /// variant, and a fitted one would not be the part (ADR-0025 §2).
    #[error("#{entity}: {name} is an offset, which Arris does not hold")]
    Offset {
        /// The entity.
        entity: u64,
        /// Its type.
        name: String,
    },
    /// A composite curve or surface: `COMPOSITE_CURVE`,
    /// `COMPOSITE_CURVE_ON_SURFACE` and its boundary subtypes,
    /// `RECTANGULAR_COMPOSITE_SURFACE`.
    #[error("#{entity}: {name} is a composite, which Arris does not hold")]
    Composite {
        /// The entity.
        entity: u64,
        /// Its type.
        name: String,
    },
    /// A `CURVE_BOUNDED_SURFACE`.
    #[error("#{entity}: {name} is a curve-bounded surface, which Arris does not hold")]
    CurveBounded {
        /// The entity.
        entity: u64,
        /// Its type.
        name: String,
    },
    /// A `DEGENERATE_TOROIDAL_SURFACE`: a torus whose tube meets its axis.
    #[error("#{entity}: a degenerate torus, which Arris does not hold")]
    DegenerateTorus {
        /// The entity.
        entity: u64,
    },
    /// A torus whose major radius is not above its minor one, written as
    /// a `TOROIDAL_SURFACE` or turned from a circle: it passes through its
    /// own axis, and Arris holds `R > r` only.
    #[error("#{entity}: a torus of major radius {major} and minor radius {minor} crosses its axis")]
    SelfIntersectingTorus {
        /// The entity.
        entity: u64,
        /// `R`, in the caller's unit.
        major: f64,
        /// `r`, in the caller's unit.
        minor: f64,
    },
    /// An entity where the subset has none of its type: outside the
    /// AP203/214/242 B-Rep subset the reader maps (ADR-0025 §1), or a
    /// B-spline of a degree Arris does not hold.
    #[error("#{entity}: {name} is outside the subset the reader maps")]
    Unsupported {
        /// The entity.
        entity: u64,
        /// Its type, or what about it is outside.
        name: String,
    },
    /// Geometry of the subset whose values describe nothing Arris can
    /// hold: a radius that is not positive, a direction of zero length,
    /// a line extruded along itself, a knot vector that is not one.
    #[error("#{entity}: {what}")]
    Degenerate {
        /// The entity.
        entity: u64,
        /// What is wrong with it.
        what: String,
    },
    /// An instance is not what the schema says belongs where it is: a
    /// parameter of the wrong type or count, a reference to an entity
    /// the file does not define or of the wrong type, a unit nested past
    /// any file's need.
    #[error("#{entity}: {what}")]
    Malformed {
        /// The instance.
        entity: u64,
        /// What is wrong with it.
        what: String,
    },
}

/// The fieldless twin of [`Refusal`]: what a refusal histogram counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RefusalKind {
    /// [`Refusal::NoLengthUnit`].
    NoLengthUnit,
    /// [`Refusal::Malformed`].
    Malformed,
    /// [`Refusal::Offset`].
    Offset,
    /// [`Refusal::Composite`].
    Composite,
    /// [`Refusal::CurveBounded`].
    CurveBounded,
    /// [`Refusal::DegenerateTorus`].
    DegenerateTorus,
    /// [`Refusal::SelfIntersectingTorus`].
    SelfIntersectingTorus,
    /// [`Refusal::Unsupported`].
    Unsupported,
    /// [`Refusal::Degenerate`].
    Degenerate,
}

impl Refusal {
    /// Which refusal this is.
    ///
    /// ```
    /// use arris_io::step::{Refusal, RefusalKind};
    ///
    /// let r = Refusal::NoLengthUnit { context: 12 };
    /// assert_eq!(r.kind(), RefusalKind::NoLengthUnit);
    /// assert_eq!(r.to_string(), "#12: the context declares no length unit");
    /// ```
    pub fn kind(&self) -> RefusalKind {
        match self {
            Refusal::NoLengthUnit { .. } => RefusalKind::NoLengthUnit,
            Refusal::Malformed { .. } => RefusalKind::Malformed,
            Refusal::Offset { .. } => RefusalKind::Offset,
            Refusal::Composite { .. } => RefusalKind::Composite,
            Refusal::CurveBounded { .. } => RefusalKind::CurveBounded,
            Refusal::DegenerateTorus { .. } => RefusalKind::DegenerateTorus,
            Refusal::SelfIntersectingTorus { .. } => RefusalKind::SelfIntersectingTorus,
            Refusal::Unsupported { .. } => RefusalKind::Unsupported,
            Refusal::Degenerate { .. } => RefusalKind::Degenerate,
        }
    }

    /// The file instance the refusal names.
    pub fn entity(&self) -> u64 {
        match self {
            Refusal::NoLengthUnit { context } => *context,
            Refusal::Malformed { entity, .. }
            | Refusal::Offset { entity, .. }
            | Refusal::Composite { entity, .. }
            | Refusal::CurveBounded { entity, .. }
            | Refusal::DegenerateTorus { entity }
            | Refusal::SelfIntersectingTorus { entity, .. }
            | Refusal::Unsupported { entity, .. }
            | Refusal::Degenerate { entity, .. } => *entity,
        }
    }
}

impl fmt::Display for RefusalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RefusalKind::NoLengthUnit => "no length unit",
            RefusalKind::Malformed => "malformed",
            RefusalKind::Offset => "offset",
            RefusalKind::Composite => "composite",
            RefusalKind::CurveBounded => "curve-bounded surface",
            RefusalKind::DegenerateTorus => "degenerate torus",
            RefusalKind::SelfIntersectingTorus => "self-intersecting torus",
            RefusalKind::Unsupported => "unsupported entity",
            RefusalKind::Degenerate => "degenerate geometry",
        })
    }
}
