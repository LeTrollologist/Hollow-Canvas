//! Storage channels.
//!
//! One trait, one implementation today (`u8`). It exists so the compositor can be written
//! once against working precision and instantiated per storage type, instead of carrying a
//! `match` on the storage format inside the pixel loop. `u16` is the planned second
//! instantiation; it is deliberately not present yet - see `docs/pixel-format-proposal.md`.
//!
//! The conversions are the only places storage precision meets working precision, and they
//! are exact in both directions for every value of the type, which is asserted below.

/// One channel of layer storage.
pub trait Channel: Copy + PartialEq + 'static {
    /// The all-zero value, used for transparent-pixel checks and buffer initialisation.
    const ZERO: Self;

    /// Storage -> working precision.
    fn to_f32(self) -> f32;

    /// Working precision -> storage, with the rounding rule the format has always used:
    /// clamp to the unit range, scale, round.
    fn from_f32(value: f32) -> Self;
}

impl Channel for u8 {
    const ZERO: Self = 0;

    #[inline]
    fn to_f32(self) -> f32 {
        self as f32 / 255.0
    }

    #[inline]
    fn from_f32(value: f32) -> Self {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    }
}

/// `Srgb16`: the same gamma-encoded sRGB as `u8`, with 256x the levels.
///
/// This is a **storage** channel, not a colour space. Blending happens on the same encoded
/// values as before, which is what lets the 8-bit documents of today be an exact subset:
/// see `srgb8_to_srgb16`.
impl Channel for u16 {
    const ZERO: Self = 0;

    #[inline]
    fn to_f32(self) -> f32 {
        self as f32 / 65535.0
    }

    #[inline]
    fn from_f32(value: f32) -> Self {
        (value.clamp(0.0, 1.0) * 65535.0).round() as u16
    }
}

/// Promotes an 8-bit channel onto the 16-bit lattice.
///
/// `v * 257` is exact, and because `65535 == 255 * 257` the promoted value has the *same*
/// working-precision value as the original: `(257v) / 65535 == v / 255` in real arithmetic,
/// and both sides are correctly rounded from the same real number. That identity is what
/// lets an 8-bit document be a subset of a 16-bit one, and it is asserted below rather than
/// assumed.
#[inline]
pub fn srgb8_to_srgb16(value: u8) -> u16 {
    value as u16 * 257
}

/// Converts a 16-bit channel back to 8 bits.
///
/// Exact for values that came from `srgb8_to_srgb16`; round-to-nearest otherwise, which is
/// the documented `+-1 LSB` behaviour of the boundary.
#[inline]
pub fn srgb16_to_srgb8(value: u16) -> u8 {
    ((value as u32 + 128) / 257) as u8
}

#[cfg(test)]
mod tests {
    use super::{srgb16_to_srgb8, srgb8_to_srgb16, Channel};

    /// The property the whole 16-bit plan rests on: an 8-bit document is an exact subset of
    /// a 16-bit one, in storage *and* in working precision.
    #[test]
    fn eight_bit_values_promote_to_the_same_working_value() {
        for value in 0u8..=255 {
            let promoted = srgb8_to_srgb16(value);
            assert_eq!(
                u16::to_f32(promoted),
                u8::to_f32(value),
                "promoted {value} does not compute to the same working value"
            );
            assert_eq!(srgb16_to_srgb8(promoted), value, "{value} did not survive the round trip");
        }
    }

    /// Non-lattice values are where 16-bit earns its keep: they must stay distinct through
    /// storage, and they must not collide when read back as working precision.
    #[test]
    fn off_lattice_values_are_distinct_and_do_not_collide() {
        let mut seen = std::collections::HashSet::new();
        // 255 has no half-step above it: 255 * 257 + 128 would not fit in a u16.
        for step in 0..255u16 {
            let value = srgb8_to_srgb16(step as u8) + 128;
            let working = u16::to_f32(value);
            assert!(
                seen.insert(working.to_bits()),
                "half-step value {value} collided with an existing working value"
            );
            assert_eq!(srgb16_to_srgb8(value), step as u8, "half way up rounds back to the step");
        }
    }

    /// A buffer that survives a round trip unchanged is what makes it safe for the
    /// compositor to convert, compute, and convert back - including on the paths where the
    /// maths decides the source is transparent and hands the destination straight back.
    #[test]
    fn u16_round_trip_through_working_precision_is_exact() {
        for value in 0u16..=u16::MAX {
            assert_eq!(
                u16::from_f32(u16::to_f32(value)),
                value,
                "u16 {value} did not survive the round trip"
            );
        }
    }

    #[test]
    fn u8_round_trip_through_working_precision_is_exact() {
        for value in 0u8..=255 {
            assert_eq!(
                u8::from_f32(value.to_f32()),
                value,
                "u8 {value} did not survive the round trip"
            );
        }
    }
}
