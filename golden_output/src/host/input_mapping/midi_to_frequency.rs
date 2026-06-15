use crate::std_lib::midi::MidiEvent;

pub fn midi_to_frequency(midi: &[MidiEvent]) -> Option<f32> {
    midi
        .iter()
        .rev()
        .find_map(|e| match e {
            MidiEvent::NoteOn { note, velocity } if *velocity > 0 => {
                Some(midi_note_to_freq(*note))
            }
            _ => None,
        })
}

#[inline]
fn midi_note_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}
