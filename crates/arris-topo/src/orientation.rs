//! The two-valued orientation and its XOR composition.

use core::fmt;
use core::ops::{BitXor, BitXorAssign, Not};

/// The sense in which one entity uses another: a shell's use of a face, a
/// coedge's use of an edge, a handle's view of a body.
///
/// Composes by XOR down the hierarchy (`docs/02-data-model.md`
/// §Orientation): `Forward ∘ o = o`, `Reversed ∘ o = !o`. Composition is
/// associative and commutative, `Forward` is the identity and every
/// orientation is its own inverse. Entities are never oriented themselves;
/// only references carry an orientation.
///
/// ```
/// use arris_topo::Orientation::{Forward, Reversed};
///
/// assert_eq!(Forward.compose(Reversed), Reversed);
/// assert_eq!(Reversed.compose(Reversed), Forward);
/// assert_eq!(Reversed ^ Forward, !Forward);
/// assert_eq!(Reversed.sign(), -1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Orientation {
    /// Same sense as the entity's own: a face's normal is its surface's,
    /// an edge's direction is its curve's.
    #[default]
    Forward,
    /// Opposite sense: the normal or direction is flipped.
    Reversed,
}

impl Orientation {
    /// The composed orientation of `inner` seen through `self`: `self` is
    /// the orientation of the path so far, `inner` the next reference on it.
    pub const fn compose(self, inner: Orientation) -> Orientation {
        match (self, inner) {
            (Orientation::Forward, o) => o,
            (Orientation::Reversed, Orientation::Forward) => Orientation::Reversed,
            (Orientation::Reversed, Orientation::Reversed) => Orientation::Forward,
        }
    }

    /// The opposite orientation.
    pub const fn flipped(self) -> Orientation {
        match self {
            Orientation::Forward => Orientation::Reversed,
            Orientation::Reversed => Orientation::Forward,
        }
    }

    /// `true` for [`Orientation::Reversed`].
    pub const fn is_reversed(self) -> bool {
        matches!(self, Orientation::Reversed)
    }

    /// `+1.0` for `Forward`, `-1.0` for `Reversed`: the factor a normal or
    /// tangent is multiplied by.
    pub const fn sign(self) -> f64 {
        match self {
            Orientation::Forward => 1.0,
            Orientation::Reversed => -1.0,
        }
    }
}

impl Not for Orientation {
    type Output = Orientation;

    fn not(self) -> Orientation {
        self.flipped()
    }
}

impl BitXor for Orientation {
    type Output = Orientation;

    fn bitxor(self, rhs: Orientation) -> Orientation {
        self.compose(rhs)
    }
}

impl BitXorAssign for Orientation {
    fn bitxor_assign(&mut self, rhs: Orientation) {
        *self = self.compose(rhs);
    }
}

impl fmt::Display for Orientation {
    /// `+` for `Forward`, `-` for `Reversed`: the text-dump form.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Orientation::Forward => "+",
            Orientation::Reversed => "-",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Orientation::{self, Forward, Reversed};
    use proptest::prelude::*;

    fn orientation() -> impl Strategy<Value = Orientation> {
        prop_oneof![Just(Forward), Just(Reversed)]
    }

    #[test]
    fn table() {
        assert_eq!(Forward.compose(Forward), Forward);
        assert_eq!(Forward.compose(Reversed), Reversed);
        assert_eq!(Reversed.compose(Forward), Reversed);
        assert_eq!(Reversed.compose(Reversed), Forward);
        assert_eq!(Orientation::default(), Forward);
        assert_eq!(Forward.to_string(), "+");
        assert_eq!(Reversed.to_string(), "-");
    }

    proptest! {
        #[test]
        fn composition_is_xor(a in orientation(), b in orientation()) {
            prop_assert_eq!(a.compose(b).is_reversed(), a.is_reversed() ^ b.is_reversed());
            prop_assert_eq!(a ^ b, a.compose(b));
        }

        #[test]
        fn composition_is_associative_and_commutative(
            a in orientation(), b in orientation(), c in orientation()
        ) {
            prop_assert_eq!((a ^ b) ^ c, a ^ (b ^ c));
            prop_assert_eq!(a ^ b, b ^ a);
        }

        #[test]
        fn forward_is_identity_and_each_is_its_own_inverse(a in orientation()) {
            prop_assert_eq!(Forward ^ a, a);
            prop_assert_eq!(a ^ a, Forward);
            prop_assert_eq!(!!a, a);
            prop_assert_eq!(a.sign() * (!a).sign(), -1.0);
        }
    }
}
