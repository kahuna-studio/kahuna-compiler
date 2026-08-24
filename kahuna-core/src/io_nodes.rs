use crate::midi::{MidiEvent, MidiQueue};

pub struct MidiIn {
    midi_queue: MidiQueue,
}

impl MidiIn {
    pub fn new(midi_queue: MidiQueue) -> Self {
        Self {
            midi_queue,
        }
    }

     pub fn process(&self, midi_events: &mut Vec<MidiEvent>) {

        midi_events.clear();
        midi_events.extend(
            self.midi_queue.lock().unwrap().drain(..)
        );
    }
}


pub struct AudioOut {
}

impl AudioOut {
    pub fn new(_sample_rate: u32, _max_buffer_size: usize) -> Self {
        Self {}
    }

    pub fn process(&self, left_in: &[f32], right_in: &[f32], host_audio_out: &mut [f32]) {

        let frames = host_audio_out.len() / 2;

        for frame in 0..frames {
            for ch in 0..2 {
                let sample = if ch == 0 {
                    left_in[frame]
                } else {
                    right_in[frame]
                };
                host_audio_out[frame * 2 + ch] = sample;
            }
        }
    }
}
