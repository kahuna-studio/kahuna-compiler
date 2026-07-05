use crate::host::midi::MidiEvent;

pub struct MidiEventToFrequencyParameter {
    active_notes: Vec<u8>,
}

impl MidiEventToFrequencyParameter {
    pub fn new(_sample_rate: u32, _max_buffer_size: usize) -> Self {
        Self {
            active_notes: Vec::new(),
        }
    }

    pub fn process(&mut self, midi_in: &[MidiEvent], out: &mut Option<f32>) {
        for event in midi_in {
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

        *out = self.active_notes.last().map(|&note| midi_note_to_frequency(note));
    }
}

#[inline]
fn midi_note_to_frequency(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make() -> (MidiEventToFrequencyParameter, Option<f32>) {
        (MidiEventToFrequencyParameter::new(48000, 256), None)
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
    fn no_events_yields_none() {
        let (mut m, mut out) = make();
        m.process(&[], &mut out);
        assert_eq!(out, None);
    }

    #[test]
    fn note_on_yields_frequency() {
        let (mut m, mut out) = make();
        m.process(&[note_on(69)], &mut out);
        assert!((out.unwrap() - 440.0).abs() < 0.001);
    }

    #[test]
    fn note_off_yields_none_when_no_notes_remain() {
        let (mut m, mut out) = make();
        m.process(&[note_on(69)], &mut out);
        m.process(&[note_off(69)], &mut out);
        assert_eq!(out, None);
    }

    #[test]
    fn note_on_velocity_0_yields_none_when_no_notes_remain() {
        let (mut m, mut out) = make();
        m.process(&[note_on(69)], &mut out);
        m.process(&[note_on_vel0(69)], &mut out);
        assert_eq!(out, None);
    }

    #[test]
    fn last_pressed_note_wins() {
        let (mut m, mut out) = make();
        m.process(&[note_on(60), note_on(64), note_on(67)], &mut out);
        assert!((out.unwrap() - midi_note_to_frequency(67)).abs() < 0.001);
    }

    #[test]
    fn releasing_top_note_falls_back_to_previous() {
        let (mut m, mut out) = make();
        m.process(&[note_on(60), note_on(64)], &mut out);
        m.process(&[note_off(64)], &mut out);
        assert!((out.unwrap() - midi_note_to_frequency(60)).abs() < 0.001);
    }

    #[test]
    fn duplicate_note_on_does_not_stack() {
        let (mut m, mut out) = make();
        m.process(&[note_on(69), note_on(69)], &mut out);
        m.process(&[note_off(69)], &mut out);
        assert_eq!(out, None);
        assert!(m.active_notes.is_empty());
    }
}
