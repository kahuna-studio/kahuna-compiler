use kahuna_unit::kahuna_value_operator;

/// Blends from `a` (position -1) over an equal mix (0) to `b` (1). Bipolar, so an LFO can sweep
/// the whole range and the default sits in the middle. Positions beyond -1 or 1 are limited to them.
#[kahuna_value_operator]
pub fn crossfade(
    a: f32,
    b: f32,
    #[input(default = 0.0, range = -1.0..=1.0)] position: f32,
) -> f32 {
    let mix = (position.max(-1.0).min(1.0) + 1.0) * 0.5;
    a + (b - a) * mix
}

/// Passes one of its inputs through. The position from 0 to 1 is split evenly across the inputs,
/// so an envelope or a knob can step through them however many are connected. Positions beyond
/// 0 or 1 select the first or last input.
#[kahuna_value_operator]
pub fn select<T: Copy>(
    #[input(default = 0.0, range = 0.0..=1.0)] position: f32,
    #[input(min = 2)] inputs: &[T],
) -> T {
    // `as usize` maps negative values and NaN to 0; the compiler guarantees at least two inputs.
    let index = (position * inputs.len() as f32) as usize;
    inputs[index.min(inputs.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossfade_moves_from_a_over_the_middle_to_b() {
        assert_eq!(crossfade(2.0, 4.0, -1.0), 2.0);
        assert_eq!(crossfade(2.0, 4.0, 0.0), 3.0);
        assert_eq!(crossfade(2.0, 4.0, 1.0), 4.0);
        assert_eq!(crossfade(2.0, 4.0, 5.0), 4.0);
    }

    #[test]
    fn select_splits_the_position_evenly_across_the_inputs() {
        let inputs = [10.0, 20.0, 30.0];
        assert_eq!(select(0.0, &inputs), 10.0);
        assert_eq!(select(0.34, &inputs), 20.0);
        assert_eq!(select(0.99, &inputs), 30.0);
        assert_eq!(select(1.0, &inputs), 30.0);
        assert_eq!(select(-0.5, &inputs), 10.0);
        assert_eq!(select(f32::NAN, &inputs), 10.0);
    }

    #[test]
    fn select_works_for_any_value_type() {
        assert!(select(0.9, &[false, true]));
    }
}
