pub mod midi;
pub mod io_nodes;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kahuna_core::midi::MidiEvent;
use kahuna_core::patch::KahunaPatch;
use midi::start_midi;
use std::sync::{Arc, Mutex};

pub fn run_standalone<P, F>(max_buffer_size: usize, build_patch: F)
where
  P: KahunaPatch + Send + 'static,
  F: FnOnce(u32, usize, Arc<Mutex<Vec<MidiEvent>>>) -> P,
{
    let host = cpal::default_host();

    let device = host
      .default_output_device()
      .expect("No audio output device");

    let config = device
      .default_output_config()
      .expect("Failed to get default output config");
    let mut stream_config: cpal::StreamConfig = config.into();
    stream_config.buffer_size = cpal::BufferSize::Fixed(max_buffer_size as cpal::FrameCount);

    let sample_rate = stream_config.sample_rate;

    eprintln!("[Audio] Device: {}", device.description().expect("No audio device description"));
    eprintln!("[Audio] Sample rate: {}", sample_rate);
    eprintln!("[Audio] Config: {:?}", stream_config);

    let midi_queue = Arc::new(Mutex::new(Vec::<MidiEvent>::new()));
    let _midi_conn = start_midi(Arc::clone(&midi_queue));

    let mut patch = build_patch(sample_rate, max_buffer_size, Arc::clone(&midi_queue));

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

    eprintln!("[Kahuna] Running. Press Enter to quit.");
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).ok();
}
