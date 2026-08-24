pub trait KahunaPatch {
  fn process(&mut self, output: &mut [f32]);
}
