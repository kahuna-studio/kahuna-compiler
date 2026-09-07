use crate::midi::MidiEvent;

pub trait IoNodeFactory {
  fn create_midi_in(&self) -> Box<dyn MidiInputNode>;
  fn create_midi_out(&self) -> Box<dyn MidiOutputNode>;
  fn create_audio_in(&self) -> Box<dyn AudioInputNode>;
  fn create_audio_out(&self) -> Box<dyn AudioOutputNode>;
}

pub trait AudioOutputNode: Send {
  fn process(&mut self, left: &[f32], right: &[f32]);
}

pub trait AudioInputNode: Send {
  fn process(&mut self, left: &mut [f32], right: &mut [f32]);
}

pub trait MidiInputNode: Send {
  fn process(&mut self, out: &mut Vec<MidiEvent>);
}

pub trait MidiOutputNode: Send {
  fn process(&mut self, events: &[MidiEvent]);
}
