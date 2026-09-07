use kahuna_core::io_nodes::{AudioOutputNode, MidiInputNode};
use kahuna_core::midi::{MidiEvent, MidiQueue};
use std::sync::{Arc, Mutex};

pub struct CpalIoNodeFactory {
    pub midi_queue: MidiQueue,
    pub audio_buffer: AudioBuffer,
}

impl kahuna_core::io_nodes::IoNodeFactory for CpalIoNodeFactory {
    fn create_midi_in(&self) -> Box<dyn kahuna_core::io_nodes::MidiInputNode> {
        Box::new(MidiIn::new(self.midi_queue.clone()))
    }
    fn create_midi_out(&self) -> Box<dyn kahuna_core::io_nodes::MidiOutputNode> {
        unimplemented!("no MIDI output wired up yet")
    }
    fn create_audio_in(&self) -> Box<dyn kahuna_core::io_nodes::AudioInputNode> {
        unimplemented!("no audio input wired up yet")
    }
    fn create_audio_out(&self) -> Box<dyn kahuna_core::io_nodes::AudioOutputNode> {
        Box::new(AudioOut::new(self.audio_buffer.clone()))
    }
}

pub struct MidiIn {
    midi_queue: MidiQueue,
}

impl MidiIn {
    pub fn new(midi_queue: MidiQueue) -> Self {
        Self { midi_queue }
    }
}

impl MidiInputNode for MidiIn {
    fn process(&mut self, out: &mut Vec<MidiEvent>) {
        out.clear();
        out.extend(self.midi_queue.lock().unwrap().drain(..));
    }
}

#[derive(Default)]
pub struct AudioBufferData {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

pub type AudioBuffer = Arc<Mutex<AudioBufferData>>;

pub struct AudioOut {
    buffer: AudioBuffer,
}

impl AudioOut {
    pub fn new(buffer: AudioBuffer) -> Self {
        Self { buffer }
    }
}

impl AudioOutputNode for AudioOut {
    fn process(&mut self, left: &[f32], right: &[f32]) {
        let mut buf = self.buffer.lock().unwrap();
        buf.left.clear();
        buf.left.extend_from_slice(left);
        buf.right.clear();
        buf.right.extend_from_slice(right);
    }
}
