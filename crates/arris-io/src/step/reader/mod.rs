//! The STEP reader (ADR-0025): what a parsed exchange structure
//! ([`super::part21`]) means, entity by entity. This module holds the
//! reader's public vocabulary — the options a caller reads with, and the
//! typed refusal each solid the reader cannot take comes back as — and its
//! layers: [`entities`] resolves references and reads parameters by the
//! schema's types, and [`units`] reads a representation context's units
//! and converts every length and angle to the caller's.

// Nothing outside the tests reaches the layers until `step::read` does
// (plans/step-reader step 10), which lifts these allowances.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod entities;
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
        }
    }

    /// The file instance the refusal names.
    pub fn entity(&self) -> u64 {
        match self {
            Refusal::NoLengthUnit { context } => *context,
            Refusal::Malformed { entity, .. } => *entity,
        }
    }
}

impl fmt::Display for RefusalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RefusalKind::NoLengthUnit => "no length unit",
            RefusalKind::Malformed => "malformed",
        })
    }
}
