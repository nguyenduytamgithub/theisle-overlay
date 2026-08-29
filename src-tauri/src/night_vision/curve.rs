pub(crate) type GammaRamp = [[u16; 256]; 3];

pub(crate) const READBACK_TOLERANCE: u16 = 257;

#[cfg(any(test, feature = "devtools"))]
pub(crate) fn lifted_ramp(strength: u8) -> GammaRamp {
    let strength = strength.min(100) as f64 / 100.0;
    let gamma = 1.0 - 0.65 * strength;
    let channel = std::array::from_fn(|index| {
        if index == 0 {
            return 0;
        }
        if index == 255 {
            return u16::MAX;
        }
        ((index as f64 / 255.0).powf(gamma) * u16::MAX as f64).round() as u16
    });
    [channel; 3]
}

pub(crate) fn ramps_match(expected: &GammaRamp, actual: &GammaRamp, tolerance: u16) -> bool {
    expected
        .iter()
        .zip(actual)
        .all(|(expected_channel, actual_channel)| {
            expected_channel
                .iter()
                .zip(actual_channel)
                .all(|(expected_value, actual_value)| {
                    expected_value.abs_diff(*actual_value) <= tolerance
                })
        })
}

#[cfg(test)]
mod tests {
    use super::{lifted_ramp, ramps_match, READBACK_TOLERANCE};

    #[test]
    fn every_strength_is_neutral_monotonic_and_preserves_endpoints() {
        for strength in 0..=100 {
            let ramp = lifted_ramp(strength);
            assert_eq!(ramp[0], ramp[1], "red/green differ at {strength}");
            assert_eq!(ramp[1], ramp[2], "green/blue differ at {strength}");
            assert_eq!(ramp[0][0], 0, "black moved at {strength}");
            assert_eq!(ramp[0][255], u16::MAX, "white moved at {strength}");
            assert!(
                ramp[0].windows(2).all(|pair| pair[0] <= pair[1]),
                "curve is not monotonic at {strength}"
            );
        }
    }

    #[test]
    fn strength_70_lifts_midtones_without_clipping_highlights() {
        let ramp = lifted_ramp(70);
        assert!(ramp[0][128] >= 42_598);
        assert!(ramp[0][192] < u16::MAX);
        assert!(ramp[0][64] > 16_448);
    }

    #[test]
    fn strength_is_clamped_to_one_hundred() {
        assert_eq!(lifted_ramp(100), lifted_ramp(u8::MAX));
    }

    #[test]
    fn readback_tolerance_accepts_driver_quantization_only() {
        let expected = lifted_ramp(70);
        let mut accepted = expected;
        accepted[0][128] = accepted[0][128].saturating_sub(257);
        assert!(ramps_match(&expected, &accepted, READBACK_TOLERANCE));

        let mut rejected = expected;
        rejected[0][128] = rejected[0][128].saturating_sub(258);
        assert!(!ramps_match(&expected, &rejected, READBACK_TOLERANCE));
    }
}
