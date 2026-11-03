use kahuna_core::midi::MidiEvent;
use kahuna_core::ModuleEnv;
use kahuna_unit::kahuna_converter;

#[kahuna_converter]
pub struct MidiEventToFrequencyParameter {
    active_notes: Vec<u8>,
    last_frequency: f32,

    #[input]
    midi_events: Midi,

    #[output]
    out: Parameter<f32>,
}

impl MidiEventToFrequencyParameter {
    pub fn init(_module_env: &ModuleEnv) -> Self {
        Self {
            active_notes: Vec::new(),
            last_frequency: 440.0,
        }
    }

    pub fn process(&mut self, midi_events: &[MidiEvent], out: &mut f32) {
        for event in midi_events {
            match event {
                MidiEvent::NoteOn { note, velocity } if *velocity > 0 => {
                    if !self.active_notes.contains(note) {
                        self.active_notes.push(*note);
                    }
                }
                MidiEvent::NoteOn { note, .. } | MidiEvent::NoteOff { note, .. } => {
                    self.active_notes.retain(|n| n != note);
                }
            }
        }

        *out = self.active_notes.last()
            .map(|&note| {
                let freq = midi_note_to_frequency(note);
                self.last_frequency = freq;
                freq
            })
            .unwrap_or(self.last_frequency);
    }
}

#[inline]
fn midi_note_to_frequency(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kahuna_core::KahunaConverter;

    fn make() -> (MidiEventToFrequencyParameter, f32) {
        (MidiEventToFrequencyParameter::new(&ModuleEnv { sample_rate: 48000, max_buffer_size: 256 }), 440.0)
    }

    fn note_on(note: u8) -> MidiEvent {
        MidiEvent::NoteOn { note, velocity: 100 }
    }

    fn note_off(note: u8) -> MidiEvent {
        MidiEvent::NoteOff { note }
    }

    fn note_on_vel0(note: u8) -> MidiEvent {
        MidiEvent::NoteOn { note, velocity: 0 }
    }

    #[test]
    fn no_events_yields_default() {
        let (mut m, mut out) = make();
        m.process(&[], &mut out);
        assert_eq!(out, 440.0);
    }

    #[test]
    fn note_off_holds_last_frequency() {
        let (mut m, mut out) = make();
        m.process(&[note_on(69)], &mut out);
        let freq_at_69 = out;
        m.process(&[note_off(69)], &mut out);
        assert!((out - freq_at_69).abs() < 0.001);
    }

    #[test]
    fn note_on_yields_frequency() {
        let (mut m, mut out) = make();
        m.process(&[note_on(69)], &mut out);
        assert!((out - 440.0).abs() < 0.001);
    }

    #[test]
    fn note_off_holds_last_frequency_when_no_notes_remain() {
        let (mut m, mut out) = make();
        m.process(&[note_on(69)], &mut out);
        let freq_at_69 = out;
        m.process(&[note_off(69)], &mut out);
        assert!((out - freq_at_69).abs() < 0.001);
    }

    #[test]
    fn note_on_velocity_0_holds_last_frequency_when_no_notes_remain() {
        let (mut m, mut out) = make();
        m.process(&[note_on(69)], &mut out);
        let freq_at_69 = out;
        m.process(&[note_on_vel0(69)], &mut out);
        assert!((out - freq_at_69).abs() < 0.001);
    }

    #[test]
    fn last_pressed_note_wins() {
        let (mut m, mut out) = make();
        m.process(&[note_on(60), note_on(64), note_on(67)], &mut out);
        assert!((out - midi_note_to_frequency(67)).abs() < 0.001);
    }

    #[test]
    fn releasing_top_note_falls_back_to_previous() {
        let (mut m, mut out) = make();
        m.process(&[note_on(60), note_on(64)], &mut out);
        m.process(&[note_off(64)], &mut out);
        assert!((out - midi_note_to_frequency(60)).abs() < 0.001);
    }

    #[test]
    fn duplicate_note_on_does_not_stack() {
        let (mut m, mut out) = make();
        m.process(&[note_on(69), note_on(69)], &mut out);
        m.process(&[note_off(69)], &mut out);
        assert_eq!(out, 440.0);
        assert!(m.active_notes.is_empty());
    }
}
