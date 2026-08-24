use std::io::Write;
use std::sync::{Arc, Mutex};

use crate::host::lib::start_midi;
use crate::modules::gate_envelope::GateEnvelope;
use crate::modules::sine_osc::SineOsc;
use crate::patch::Patch;
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, BufferSize, FrameCount, StreamConfig};
use kahuna_core::midi::MidiEvent;
use kahuna_core::port_manifest::PortManifest;
use kahuna_core::KahunaModule;

pub fn main() {
    let max_buffer_size: usize = 2048;

    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .expect("No audio output device");

    let config = device.default_output_config().expect("Failed to get default output config");
    let mut stream_config: StreamConfig = config.into();
    stream_config.buffer_size = BufferSize::Fixed(max_buffer_size as FrameCount);

    let sample_rate = stream_config.sample_rate;

    eprintln!("[Audio] Device: {}", device.description().expect("No audio device description"));
    eprintln!("[Audio] Sample rate: {}", sample_rate);
    eprintln!("[Audio] Channels: {}", "Hardcoded to 2 so far");
    eprintln!("[Audio] Config: {:?}", stream_config);

    let midi_queue = Arc::new(Mutex::new(Vec::<MidiEvent>::new()));

    let _midi_conn = start_midi(Arc::clone(&midi_queue));

    let mut patch = Patch::new(
        sample_rate,
        max_buffer_size,
        Arc::clone(&midi_queue),
    );

    let stream = device
        .build_output_stream(
            stream_config,
            move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                patch.process(output);
            },
            |err| eprintln!("[Audio] Error: {}", err),
            None,
        )
        .expect("Failed to build stream");

    stream.play().expect("Failed to start stream");

    write_manifest();

    eprintln!("[Kahuna] Running. Press Enter to quit.");

    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).ok();
}
fn write_manifest() {
    let modules: Vec<(&str, Vec<PortManifest>)> = vec![
        ("SineOsc", SineOsc::PORTS.iter().map(Into::into).collect()),
        ("GateEnvelope", GateEnvelope::PORTS.iter().map(Into::into).collect()),
    ];
    let json = serde_json::to_string_pretty(&modules).unwrap();

    eprintln!("[Kahuna] Module interfaces:");
    std::io::stdout().write(json.as_bytes()).unwrap();
}
