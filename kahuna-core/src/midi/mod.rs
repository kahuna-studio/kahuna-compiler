use std::sync::{Arc, Mutex};

pub mod polyphony;

#[derive(Clone, Debug)]
pub enum MidiEvent {
  NoteOn  { note: u8, velocity: u8 },
  NoteOff { note: u8 },
}

pub type MidiQueue = Arc<Mutex<Vec<MidiEvent>>>;
