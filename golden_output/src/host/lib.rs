use kahuna_core::midi::{MidiEvent, MidiQueue};
use midir::{MidiInput, MidiInputConnection};

pub fn start_midi(midi_queue: MidiQueue) -> Option<MidiInputConnection<()>> {
    let midi_in = MidiInput::new("kahuna_synth").ok()?;
    let ports   = midi_in.ports();
    if ports.is_empty() {
        eprintln!("[MIDI] No MIDI input ports found.");
        return None;
    }
    eprintln!("[MIDI] Connected to: {}", midi_in.port_name(&ports[0]).unwrap_or_default());
    midi_in.connect(&ports[0], "kahuna_midi_in", move |_, msg, _| {
        let event = match msg[0] & 0xF0 {
            0x90 if msg[2] > 0 => Some(MidiEvent::NoteOn  { note: msg[1], velocity: msg[2] }),
            0x90               => Some(MidiEvent::NoteOff { note: msg[1] }),
            0x80               => Some(MidiEvent::NoteOff { note: msg[1] }),
            _                  => None,
        };
        if let Some(ev) = event {
            midi_queue.lock().unwrap().push(ev);
        }
    }, ()).ok()
}


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
