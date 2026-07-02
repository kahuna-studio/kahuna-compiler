use crate::host::midi::MidiEvent;

pub struct GateEnvelope {
    gate: f32,
}

impl GateEnvelope {
    pub fn new(_sample_rate: u32, _max_buffer_size: usize) -> Self {
        Self { gate: 0.0 }
    }

    pub fn process(
        &mut self,
        midi_in: &[MidiEvent],
        output: &mut [f32],
    ) {
        for event in midi_in {
            match event {
                MidiEvent::NoteOn { velocity, .. } => {
                    if *velocity > 0 {
                        self.gate = 1.0;
                    } else {
                        self.gate = 0.0;
                    }
                }
                MidiEvent::NoteOff { .. } => {
                    self.gate = 0.0;
                }
            }
        }

        output.fill(self.gate);
    }
}
