use crate::std_lib::mapping::midi_event_to_frequency_parameter::MidiEventToFrequencyParameter;
use crate::host::io_nodes::{AudioOut, MidiIn};
use crate::host::polyphony::voice_allocator::VoiceAllocator;
use crate::host::lib::PolyBuffer;
use crate::modules::gate_envelope::GateEnvelope;
use crate::modules::sine_osc::SineOsc;
use crate::host::midi::{MidiEvent, MidiQueue};
use crate::std_lib::mapping::midi_event_to_gate_parameter::MidiEventToGateParameter;
use crate::std_lib::operators::Multiply;


const VOICES: usize = 8;

#[allow(non_snake_case)]
pub struct PatchState {

    midi_in_0__midi_events: Vec<MidiEvent>,
    poly__out: Vec<Vec<MidiEvent>>,
    midi_events_to_frequency_parameter_0__out: Vec<Option<f32>>,
    sine_osc_0__out: PolyBuffer<f32>,
    midi_events_to_gate_parameter_0__out: Vec<f32>,
    gate_envelope_0__out: PolyBuffer<f32>,
    multiply_0__out: PolyBuffer<f32>,
    merge__out: Vec<f32>,
}

impl PatchState {
    pub fn new(max_buffer_size: usize) -> Self {
        Self {
            midi_in_0__midi_events: Vec::new(),
            poly__out: (0..VOICES)
                .map(|_| Vec::new())
                .collect(),
            midi_events_to_frequency_parameter_0__out:vec![None; VOICES],
            sine_osc_0__out: PolyBuffer::new(VOICES, max_buffer_size),
            midi_events_to_gate_parameter_0__out:vec![0.0; VOICES],
            gate_envelope_0__out: PolyBuffer::new(VOICES, max_buffer_size),
            multiply_0__out: PolyBuffer::new(VOICES, max_buffer_size),
            merge__out: vec![0.0; max_buffer_size],
        }
    }
}

pub struct Patch {
    max_buffer_size: usize,

    midi_in_0: MidiIn,
    voice_allocator_0: VoiceAllocator,
    midi_events_to_frequency_parameter_0: Vec<MidiEventToFrequencyParameter>,
    sine_osc_0: Vec<SineOsc>,
    midi_events_to_gate_parameter_0: Vec<MidiEventToGateParameter>,
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

            midi_in_0: MidiIn::new(midi_queue),

            voice_allocator_0: VoiceAllocator::new(VOICES),

            midi_events_to_frequency_parameter_0: (0..VOICES)
                .map(|_| MidiEventToFrequencyParameter::new(sample_rate, max_buffer_size))
                .collect(),

            sine_osc_0: (0..VOICES)
                .map(|_| SineOsc::new(sample_rate, max_buffer_size))
                .collect(),

            midi_events_to_gate_parameter_0: (0..VOICES)
                .map(|_| MidiEventToGateParameter::new(sample_rate, max_buffer_size))
                .collect(),

            gate_envelope_0: (0..VOICES)
                .map(|_| GateEnvelope::new(sample_rate, max_buffer_size))
                .collect(),

            multiply_0: (0..VOICES)
                .map(|_| Multiply::new(sample_rate, max_buffer_size))
                .collect(),

            audio_out_0: AudioOut::new(sample_rate, max_buffer_size),

            state: PatchState::new(max_buffer_size),
        }
    }

    pub fn process(&mut self, host_audio_out: &mut [f32]) {

        let frames = (host_audio_out.len() / 2).min(self.max_buffer_size);

        self.midi_in_0.process(
            &mut self.state.midi_in_0__midi_events,
        );

        self.voice_allocator_0.process(
            &self.state.midi_in_0__midi_events,
            &mut self.state.poly__out,
        );

        for voice in 0..VOICES {

            self.midi_events_to_frequency_parameter_0[voice].process(
                &self.state.poly__out[voice],
                &mut self.state.midi_events_to_frequency_parameter_0__out[voice],
            );

            self.sine_osc_0[voice].process(
                self.state.midi_events_to_frequency_parameter_0__out[voice],
                &mut self.state.sine_osc_0__out.voice_mut(voice)[..frames],
            );

            self.midi_events_to_gate_parameter_0[voice].process(
                &self.state.poly__out[voice],
                &mut self.state.midi_events_to_gate_parameter_0__out[voice],
            );

            self.gate_envelope_0[voice].process(
                self.state.midi_events_to_gate_parameter_0__out[voice],
                &mut self.state.gate_envelope_0__out.voice_mut(voice)[..frames],
            );

            self.multiply_0[voice].process(
                &[
                    &self.state.sine_osc_0__out.voice(voice)[..frames],
                    &self.state.gate_envelope_0__out.voice(voice)[..frames],
                ],
                &mut self.state.multiply_0__out.voice_mut(voice)[..frames],
            );
        }

        self.state.merge__out[..frames].fill(0.0);
        for voice in 0..VOICES {
            let src = self.state.multiply_0__out.voice(voice);
            for i in 0..frames {
                self.state.merge__out[i] += src[i] / VOICES as f32;
            }
        }

        self.audio_out_0.process(
            &self.state.merge__out[..frames],
            &self.state.merge__out[..frames],
            &mut host_audio_out[..frames * 2]
        );
    }
}
