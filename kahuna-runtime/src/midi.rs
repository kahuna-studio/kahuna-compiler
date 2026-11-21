use crate::audio::find_by_name;
use crate::RuntimeError;
use kahuna_core::midi::{MidiEvent, MidiQueue};
use midir::{MidiInput, MidiInputConnection};

const NO_PORTS_MESSAGE: &str =
    "No MIDI input ports found. The instrument is silent until a MIDI device is connected and the instrument is restarted.";

/// The names of the MIDI input ports that are available now.
pub fn list_inputs() -> Result<Vec<String>, RuntimeError> {
    let midi_in = open_client().map_err(RuntimeError::Midi)?;
    Ok(port_names(&midi_in))
}

/// Connects to the requested MIDI input port, or to the first one. Without a MIDI device the
/// instrument still starts, silent: that is reported on stderr and the result is `Ok(None)`. A port
/// that was asked for by name and cannot be used is an error.
pub fn start_midi(midi_queue: MidiQueue, requested: Option<&str>) -> Result<Option<MidiInputConnection<()>>, RuntimeError> {
    let midi_in = match open_client() {
        Ok(midi_in) => midi_in,
        Err(reason) => return without_midi(requested, &reason),
    };
    let names = port_names(&midi_in);
    if names.is_empty() {
        return match requested {
            Some(name) => Err(RuntimeError::MidiPortNotFound { name: name.to_string(), available: names }),
            None => without_midi(None, NO_PORTS_MESSAGE),
        };
    }
    let index = match requested {
        Some(name) => find_by_name(&names, name)
            .ok_or_else(|| RuntimeError::MidiPortNotFound { name: name.to_string(), available: names.clone() })?,
        None => 0,
    };

    let port = midi_in.ports().swap_remove(index);
    let name = names[index].clone();
    let connection = midi_in.connect(
        &port,
        "kahuna_midi_in",
        move |_, message, _| {
            if let Some(event) = parse_midi_message(message) {
                midi_queue.lock().unwrap().push(event);
            }
        },
        (),
    );
    match connection {
        Ok(connection) => {
            eprintln!("[MIDI] Connected to: {name}");
            Ok(Some(connection))
        }
        Err(error) => without_midi(requested, &format!("Cannot connect to '{name}': {error}")),
    }
}

/// The note event a raw MIDI message stands for. Messages of other types, empty messages and
/// truncated messages are none. The channel is ignored: all 16 channels play.
pub fn parse_midi_message(message: &[u8]) -> Option<MidiEvent> {
    let (&status, data) = message.split_first()?;
    match (status & 0xF0, data) {
        (0x90, &[note, velocity, ..]) if velocity > 0 => Some(MidiEvent::NoteOn { note, velocity }),
        (0x90, &[note, _, ..]) => Some(MidiEvent::NoteOff { note }),
        (0x80, &[note, _, ..]) => Some(MidiEvent::NoteOff { note }),
        _ => None,
    }
}

fn open_client() -> Result<MidiInput, String> {
    MidiInput::new("kahuna").map_err(|error| format!("Cannot open the MIDI system: {error}"))
}

fn port_names(midi_in: &MidiInput) -> Vec<String> {
    midi_in.ports().iter().map(|port| midi_in.port_name(port).unwrap_or_default()).collect()
}

/// Silent instead of failing, unless the user asked for this port.
fn without_midi(requested: Option<&str>, reason: &str) -> Result<Option<MidiInputConnection<()>>, RuntimeError> {
    if requested.is_some() {
        return Err(RuntimeError::Midi(reason.to_string()));
    }
    eprintln!("[MIDI] {reason}");
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_note_on(event: Option<MidiEvent>, expected_note: u8, expected_velocity: u8) -> bool {
        matches!(event, Some(MidiEvent::NoteOn { note, velocity }) if note == expected_note && velocity == expected_velocity)
    }

    fn is_note_off(event: Option<MidiEvent>, expected_note: u8) -> bool {
        matches!(event, Some(MidiEvent::NoteOff { note }) if note == expected_note)
    }

    #[test]
    fn note_on_becomes_a_note_on_event() {
        assert!(is_note_on(parse_midi_message(&[0x90, 60, 100]), 60, 100));
    }

    #[test]
    fn note_on_with_velocity_zero_is_a_note_off() {
        assert!(is_note_off(parse_midi_message(&[0x90, 60, 0]), 60));
    }

    #[test]
    fn note_off_becomes_a_note_off_event() {
        assert!(is_note_off(parse_midi_message(&[0x80, 64, 40]), 64));
    }

    #[test]
    fn the_channel_is_ignored() {
        assert!(is_note_on(parse_midi_message(&[0x9F, 60, 100]), 60, 100));
        assert!(is_note_off(parse_midi_message(&[0x85, 60, 0]), 60));
    }

    #[test]
    fn truncated_and_empty_messages_are_none_without_panicking() {
        for message in [&[][..], &[0x90][..], &[0x90, 60][..], &[0x80][..], &[0x80, 60][..]] {
            assert!(parse_midi_message(message).is_none(), "{message:?}");
        }
    }

    #[test]
    fn other_message_types_are_none() {
        for message in [&[0xB0, 7, 100][..], &[0xE0, 0, 64][..], &[0xF8][..], &[0xC0, 5][..], &[60, 60, 60][..]] {
            assert!(parse_midi_message(message).is_none(), "{message:?}");
        }
    }
}
