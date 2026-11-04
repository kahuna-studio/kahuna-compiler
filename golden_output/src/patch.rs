use crate::modules::adsr_envelope::AdsrEnvelope;
use crate::modules::sine_osc::SineOsc;
use kahuna_core::io_nodes::{AudioOutputNode, IoNodeFactory, MidiInputNode};
use kahuna_core::midi::MidiEvent;
use kahuna_core::midi::polyphony::poly_buffer::PolyBuffer;
use kahuna_core::midi::polyphony::voice_allocator::VoiceAllocator;
use kahuna_core::patch::KahunaPatch;
use kahuna_core::{KahunaConverter, KahunaModule, ModuleEnv};
use kahuna_std::converters::midi_event_to_frequency_parameter::MidiEventToFrequencyParameter;
use kahuna_std::converters::midi_event_to_gate_parameter::MidiEventToGateParameter;

#[allow(non_snake_case)]
pub struct PatchState {
    midi_in_0__midi_events: Vec<MidiEvent>,
    poly_0__out: Vec<Vec<MidiEvent>>,
    poly_0__midi_events_to_frequency_parameter_0__out: Vec<f32>,
    poly_0__sine_osc_0__out: PolyBuffer<f32>,
    poly_0__midi_events_to_gate_parameter_0__out: Vec<bool>,
    poly_0__adsr_envelope_0__envelope: PolyBuffer<f32>,
    poly_0__multiply_0__out: PolyBuffer<f32>,
    merge__out: Vec<f32>,
}

impl PatchState {
    pub fn new(poly_0_voices: usize, max_buffer_size: usize) -> Self {
        Self {
            midi_in_0__midi_events: Vec::new(),
            poly_0__out: (0..poly_0_voices).map(|_| Vec::new()).collect(),
            poly_0__midi_events_to_frequency_parameter_0__out: vec![0.0; poly_0_voices],
            poly_0__sine_osc_0__out: PolyBuffer::new(poly_0_voices, max_buffer_size),
            poly_0__midi_events_to_gate_parameter_0__out: vec![false; poly_0_voices],
            poly_0__adsr_envelope_0__envelope: PolyBuffer::new(poly_0_voices, max_buffer_size),
            poly_0__multiply_0__out: PolyBuffer::new(poly_0_voices, max_buffer_size),
            merge__out: vec![0.0; max_buffer_size],
        }
    }
}

#[allow(non_snake_case)]
pub struct Patch {
    poly_0_voices: usize,

    midi_in_0: Box<dyn MidiInputNode>,
    poly_0__voice_allocator_0: VoiceAllocator,
    poly_0__midi_events_to_frequency_parameter_0: Vec<MidiEventToFrequencyParameter>,
    poly_0__sine_osc_0: Vec<SineOsc>,
    poly_0__midi_events_to_gate_parameter_0: Vec<MidiEventToGateParameter>,
    poly_0__adsr_envelope_0: Vec<AdsrEnvelope>,
    audio_out_0: Box<dyn AudioOutputNode>,

    state: PatchState,
}

impl Patch {
    pub fn new(sample_rate: u32, max_buffer_size: usize, factory: &dyn IoNodeFactory) -> Self {
        let poly_0_voices: usize = 8;

        let env = ModuleEnv {
            sample_rate,
            max_buffer_size,
        };

        Self {
            poly_0_voices,

            midi_in_0: factory.create_midi_in(),

            poly_0__voice_allocator_0: VoiceAllocator::new(poly_0_voices),

            poly_0__midi_events_to_frequency_parameter_0: (0..poly_0_voices)
                .map(|_| MidiEventToFrequencyParameter::new(&env))
                .collect(),

            poly_0__sine_osc_0: (0..poly_0_voices).map(|_| SineOsc::new(&env)).collect(),

            poly_0__midi_events_to_gate_parameter_0: (0..poly_0_voices)
                .map(|_| MidiEventToGateParameter::new(&env))
                .collect(),

            poly_0__adsr_envelope_0: (0..poly_0_voices)
                .map(|_| AdsrEnvelope::new(&env))
                .collect(),


            audio_out_0: factory.create_audio_out(),

            state: PatchState::new(poly_0_voices, max_buffer_size),
        }
    }
}

impl KahunaPatch for Patch {
    fn process(&mut self, frames: usize) {
        self.midi_in_0
            .process(&mut self.state.midi_in_0__midi_events);

        self.poly_0__voice_allocator_0.process(
            &self.state.midi_in_0__midi_events,
            &mut self.state.poly_0__out,
        );

        for voice in 0..self.poly_0_voices {
            self.poly_0__midi_events_to_frequency_parameter_0[voice].process(
                &self.state.poly_0__out[voice],
                &mut self.state.poly_0__midi_events_to_frequency_parameter_0__out[voice],
            );

            self.poly_0__sine_osc_0[voice].process(
                &<SineOsc as KahunaModule>::Inputs::new(
                    self.state.poly_0__midi_events_to_frequency_parameter_0__out[voice],
                ),
                &mut <SineOsc as KahunaModule>::Outputs::new(
                    &mut self.state.poly_0__sine_osc_0__out.voice_mut(voice)[..frames],
                ),
            );

            self.poly_0__midi_events_to_gate_parameter_0[voice].process(
                &self.state.poly_0__out[voice],
                &mut self.state.poly_0__midi_events_to_gate_parameter_0__out[voice],
            );

            self.poly_0__adsr_envelope_0[voice].process(
                &<AdsrEnvelope as KahunaModule>::Inputs::new(
                    self.state.poly_0__midi_events_to_gate_parameter_0__out[voice],
                    None,
                    None,
                    None,
                    None,
                ),
                &mut <AdsrEnvelope as KahunaModule>::Outputs::new(
                    &mut self
                        .state
                        .poly_0__adsr_envelope_0__envelope
                        .voice_mut(voice)[..frames],
                ),
            );

            {
                let out = &mut self.state.poly_0__multiply_0__out.voice_mut(voice)[..frames];
                for frame in 0..frames {
                    out[frame] = kahuna_std::value_operators::arithmetic::multiply(
                        self.state.poly_0__sine_osc_0__out.voice(voice)[frame],
                        self.state.poly_0__adsr_envelope_0__envelope.voice(voice)[frame],
                    );
                }
            }
        }

        self.state.merge__out[..frames].fill(0.0);
        for voice in 0..self.poly_0_voices {
            let src = self.state.poly_0__multiply_0__out.voice(voice);
            for i in 0..frames {
                self.state.merge__out[i] += src[i] / self.poly_0_voices as f32;
            }
        }

        self.audio_out_0.process(
            &self.state.merge__out[..frames],
            &self.state.merge__out[..frames],
        );
    }
}
