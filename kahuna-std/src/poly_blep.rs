/// Polynomial band-limited step correction (PolyBLEP).
///
/// Returns the correction to add to a naive waveform at a discontinuity.
/// `phase` is in 0..1, `increment` is the phase advance per sample (frequency / sample rate).
/// The result is non-zero only within one sample of the discontinuity at phase 0 (= 1).
#[inline]
pub fn poly_blep(phase: f32, increment: f32) -> f32 {
  if phase < increment {
    let t = phase / increment;
    t + t - t * t - 1.0
  } else if phase > 1.0 - increment {
    let t = (phase - 1.0) / increment;
    t * t + t + t + 1.0
  } else {
    0.0
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn is_zero_away_from_the_discontinuity() {
    assert_eq!(poly_blep(0.5, 0.01), 0.0);
  }

  #[test]
  fn is_antisymmetric_around_the_discontinuity() {
    let before = poly_blep(0.995, 0.01);
    let after = poly_blep(0.005, 0.01);
    assert!((before + after).abs() < 1e-6);
  }
}
