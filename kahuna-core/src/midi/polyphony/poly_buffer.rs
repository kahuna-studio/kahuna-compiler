pub struct PolyBuffer<T> {
  data: Vec<T>,
  len_per_voice: usize,
}

impl<T: Default + Clone> PolyBuffer<T> {
  pub fn new(voices: usize, len_per_voice: usize) -> Self {
    Self {
      data: vec![T::default(); voices * len_per_voice],
      len_per_voice,
    }
  }
}

impl<T> PolyBuffer<T> {
  #[inline]
  pub fn voice(&self, voice: usize) -> &[T] {
    let start = voice * self.len_per_voice;
    let end = start + self.len_per_voice;

    &self.data[start..end]
  }

  #[inline]
  pub fn voice_mut(&mut self, voice: usize) -> &mut [T] {
    let start = voice * self.len_per_voice;
    let end = start + self.len_per_voice;

    &mut self.data[start..end]
  }
}
