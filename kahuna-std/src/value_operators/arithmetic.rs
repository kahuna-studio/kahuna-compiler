use kahuna_unit::kahuna_value_operator;

/// Multiplies all inputs, e.g. a signal with an envelope (a VCA).
#[kahuna_value_operator(fold = "in")]
pub fn multiply(a: f32, b: f32) -> f32 {
    a * b
}

/// Adds all inputs, e.g. to mix signals or to add modulation to a value.
#[kahuna_value_operator(fold = "in")]
pub fn add(a: f32, b: f32) -> f32 {
    a + b
}

#[kahuna_value_operator]
pub fn subtract(a: f32, b: f32) -> f32 {
    a - b
}

#[kahuna_value_operator]
pub fn invert(input: f32) -> f32 {
    -input
}

/// Scales and shifts a value; with a negative scale it inverts (an attenuverter).
#[kahuna_value_operator]
pub fn scale_offset(
    input: f32,
    #[input(default = 1.0, range = -1.0..=1.0)] scale: f32,
    #[input(default = 0.0, range = -1.0..=1.0)] offset: f32,
) -> f32 {
    input * scale + offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_offset_with_defaults_passes_the_input_through() {
        assert_eq!(scale_offset(0.3, 1.0, 0.0), 0.3);
        assert_eq!(scale_offset(0.5, -1.0, 0.25), -0.25);
    }
}
