use kahuna_core::midi::MidiEvent;

pub struct MidiEventToGateParameter {
    active_notes: Vec<u8>,
}

impl MidiEventToGateParameter {
    pub fn new(_sample_rate: u32, _max_buffer_size: usize) -> Self {
        Self {
            active_notes: Vec::new(),
        }
    }

    pub fn process(&mut self, midi_in: &[MidiEvent], out: &mut f32) {
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

        *out = if self.active_notes.is_empty() { 0.0 } else { 1.0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make() -> (MidiEventToGateParameter, f32) {
        (MidiEventToGateParameter::new(48000, 256), 0.0)
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
    fn no_events_yields_zero() {
        let (mut m, mut out) = make();
        m.process(&[], &mut out);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn note_on_yields_one() {
        let (mut m, mut out) = make();
        m.process(&[note_on(60)], &mut out);
        assert_eq!(out, 1.0);
    }

    #[test]
    fn note_off_yields_zero_when_no_notes_remain() {
        let (mut m, mut out) = make();
        m.process(&[note_on(60)], &mut out);
        m.process(&[note_off(60)], &mut out);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn note_on_velocity_0_yields_zero_when_no_notes_remain() {
        let (mut m, mut out) = make();
        m.process(&[note_on(60)], &mut out);
        m.process(&[note_on_vel0(60)], &mut out);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn gate_stays_open_while_multiple_notes_held() {
        let (mut m, mut out) = make();
        m.process(&[note_on(60), note_on(64)], &mut out);
        m.process(&[note_off(60)], &mut out);
        assert_eq!(out, 1.0);
        m.process(&[note_off(64)], &mut out);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn spurious_note_off_does_not_underflow() {
        let (mut m, mut out) = make();
        m.process(&[note_off(60), note_off(64)], &mut out);
        assert_eq!(out, 0.0);
        assert!(m.active_notes.is_empty());
    }

    #[test]
    fn duplicate_note_on_does_not_stack() {
        let (mut m, mut out) = make();
        m.process(&[note_on(60), note_on(60)], &mut out);
        m.process(&[note_off(60)], &mut out);
        assert_eq!(out, 0.0);
        assert!(m.active_notes.is_empty());
    }
}
