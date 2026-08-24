use kahuna_core::midi::MidiEvent;

pub struct VoiceAllocator {
    assignments: Vec<Option<u8>>,
}

impl VoiceAllocator {
    pub fn new(voices: usize) -> Self {
        Self {
            assignments: vec![None; voices],
        }
    }

    pub fn process(
        &mut self,
        input: &[MidiEvent],
        output: &mut [Vec<MidiEvent>],
    ) {
        for voice_events in output.iter_mut() {
            voice_events.clear();
        }

        for event in input {
            match event {

                MidiEvent::NoteOn { note, velocity }
                if *velocity > 0 =>
                    {
                        if let Some(voice) =
                            self.assignments.iter()
                                .position(|v| v.is_none())
                        {
                            self.assignments[voice] = Some(*note);

                            output[voice].push(event.clone());
                        }
                    }

                MidiEvent::NoteOff { note }
                |
                MidiEvent::NoteOn {
                    note,
                    velocity: 0
                } =>
                    {
                        if let Some(voice) =
                            self.assignments.iter()
                                .position(|v| *v == Some(*note))
                        {
                            output[voice].push(event.clone());

                            self.assignments[voice] = None;
                        }
                    }

                _ => {}
            }
        }
    }
}
