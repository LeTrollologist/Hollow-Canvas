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

#[cfg(test)]
mod tests {
    use super::Channel;

    /// A buffer that survives a round trip unchanged is what makes it safe for the
    /// compositor to convert, compute, and convert back - including on the paths where the
    /// maths decides the source is transparent and hands the destination straight back.
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
