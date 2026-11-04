use kahuna_unit::kahuna_value_operator;

#[kahuna_value_operator(fold = "in")]
pub fn min(a: f32, b: f32) -> f32 {
    a.min(b)
}

#[kahuna_value_operator(fold = "in")]
pub fn max(a: f32, b: f32) -> f32 {
    a.max(b)
}

/// Limits the input to the range between `low` and `high`: values beyond are set to the nearer
/// bound (hard clipping). If `low` is above `high`, the two are swapped, so modulating the bounds
/// can never fail.
#[kahuna_value_operator]
pub fn clamp(
    input: f32,
    #[input(default = -1.0)] low: f32,
    #[input(default = 1.0)] high: f32,
) -> f32 {
    // Not f32::clamp: it panics if low > high, which must never happen on the audio thread.
    let (low, high) = if low <= high { (low, high) } else { (high, low) };
    input.max(low).min(high)
}

/// Compares `a` to a threshold `b`, e.g. to turn a signal into a gate.
#[kahuna_value_operator]
pub fn greater(a: f32, #[input(default = 0.0)] b: f32) -> bool {
    a > b
}

#[kahuna_value_operator]
pub fn greater_or_equal(a: f32, #[input(default = 0.0)] b: f32) -> bool {
    a >= b
}

#[kahuna_value_operator]
pub fn less(a: f32, #[input(default = 0.0)] b: f32) -> bool {
    a < b
}

#[kahuna_value_operator]
pub fn less_or_equal(a: f32, #[input(default = 0.0)] b: f32) -> bool {
    a <= b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_sets_values_beyond_the_range_to_the_nearer_bound() {
        assert_eq!(clamp(1.5, -1.0, 1.0), 1.0);
        assert_eq!(clamp(-3.0, -1.0, 1.0), -1.0);
        assert_eq!(clamp(0.2, -1.0, 1.0), 0.2);
    }

    #[test]
    fn clamp_swaps_inverted_bounds_instead_of_panicking() {
        assert_eq!(clamp(1.5, 1.0, -1.0), 1.0);
        assert_eq!(clamp(0.2, 1.0, -1.0), 0.2);
    }
}
