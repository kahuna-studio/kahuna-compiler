use crate::host::input_mapping::midi_to_frequency::midi_to_frequency;
use crate::host::io_nodes::{AudioOut, MidiIn};
use crate::host::polyphony::voice_allocator::VoiceAllocator;
use crate::host::lib::PolyBuffer;
use crate::modules::gate_envelope::GateEnvelope;
use crate::modules::sine_osc::SineOsc;
use crate::host::midi::{MidiEvent, MidiQueue};
use crate::std_lib::operators::Multiply;


const VOICES: usize = 8;

#[allow(non_snake_case)]
pub struct PatchState {
    midi_in_0__out: Vec<MidiEvent>,
    polyphony_start_0__out: Vec<Vec<MidiEvent>>,
    sine_osc_0__out: PolyBuffer<f32>,
    gate_envelope_0__out: PolyBuffer<f32>,
    multiply_0__out: PolyBuffer<f32>,
}

impl PatchState {
    pub fn new(max_buffer_size: usize) -> Self {
        Self {
            midi_in_0__out: Vec::new(),
            polyphony_start_0__out: (0..VOICES)
                .map(|_| Vec::new())
                .collect(),
            sine_osc_0__out: PolyBuffer::new(VOICES, max_buffer_size),
            gate_envelope_0__out: PolyBuffer::new(VOICES, max_buffer_size),
            multiply_0__out: PolyBuffer::new(VOICES, max_buffer_size),
        }
    }
}

pub struct Patch {
    max_buffer_size: usize,

    voice_allocator_0: VoiceAllocator,

    midi_in_0: MidiIn,
    sine_osc_0: Vec<SineOsc>,
    gate_envelope_0: Vec<GateEnvelope>,
    multiply_0: Vec<Multiply>,

    audio_out_0: AudioOut,

    state: PatchState,
}

impl Patch {
    pub fn new(
        sample_rate: u32,
        max_buffer_size: usize,
        midi_queue: MidiQueue,
    ) -> Self {
        Self {
            max_buffer_size,

            voice_allocator_0: VoiceAllocator::new(VOICES),

            midi_in_0: MidiIn::new(midi_queue),

            sine_osc_0: (0..VOICES)
                .map(|_| SineOsc::new(sample_rate, max_buffer_size))
                .collect(),

            gate_envelope_0: (0..VOICES)
                .map(|_| GateEnvelope::new(sample_rate, max_buffer_size))
                .collect(),

            multiply_0: (0..VOICES)
                .map(|_| Multiply::new(sample_rate, max_buffer_size, 2))
                .collect(),

            audio_out_0: AudioOut::new(sample_rate, max_buffer_size),

            state: PatchState::new(max_buffer_size),
        }
    }

    pub fn process(&mut self, host_audio_out: &mut [f32], channels: usize) {
        let frames = (host_audio_out.len() / channels).min(self.max_buffer_size);

        self.midi_in_0.process(
            &mut self.state.midi_in_0__out,
        );

        self.voice_allocator_0.process(
            &self.state.midi_in_0__out,
            &mut self.state.polyphony_start_0__out,
        );

        for voice in 0..VOICES {
            self.sine_osc_0[voice].process(
                midi_to_frequency(&self.state.polyphony_start_0__out[voice]),
                &mut self.state.sine_osc_0__out.voice_mut(voice)[..frames],
            );

            self.gate_envelope_0[voice].process(
                &self.state.polyphony_start_0__out[voice],
                &mut self.state.gate_envelope_0__out.voice_mut(voice)[..frames],
            );

            self.multiply_0[voice].process(
                &self.state.sine_osc_0__out.voice(voice)[..frames],
                &self.state.gate_envelope_0__out.voice(voice)[..frames],
                &mut self.state.multiply_0__out.voice_mut(voice)[..frames],
            );
        }

        // merge: sum all voices, normalised by voice count
        self.audio_out_0.out_0[..frames].fill(0.0);
        self.audio_out_0.out_1[..frames].fill(0.0);

        for voice in 0..VOICES {
            let src = self.state.multiply_0__out.voice(voice);
            for i in 0..frames {
                self.audio_out_0.out_0[i] += src[i] / VOICES as f32;
                self.audio_out_0.out_1[i] += src[i] / VOICES as f32;
            }
        }

        for frame in 0..frames {
            for ch in 0..channels {
                let sample = if ch == 0 {
                    self.audio_out_0.out_0[frame]
                } else {
                    self.audio_out_0.out_1[frame]
                };
                host_audio_out[frame * channels + ch] = sample;
            }
        }
    }
}
