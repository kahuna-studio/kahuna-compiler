pub mod midi;
pub mod io_nodes;

use crate::io_nodes::CpalIoNodeFactory;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use io_nodes::{AudioBuffer, AudioBufferData};
use kahuna_core::midi::MidiEvent;
use kahuna_core::patch::KahunaPatch;
use midi::start_midi;
use std::sync::{Arc, Mutex};
use std::thread::sleep;

pub fn run_standalone<P, F>(max_buffer_size: usize, build_patch: F)
where
  P: KahunaPatch + Send + 'static,
  F: FnOnce(u32, usize, &dyn kahuna_core::io_nodes::IoNodeFactory) -> P,
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

  let audio_buffer: AudioBuffer = Arc::new(Mutex::new(AudioBufferData::default()));

  let factory = CpalIoNodeFactory {
    midi_queue: Arc::clone(&midi_queue),
    audio_buffer: Arc::clone(&audio_buffer),
  };

  let mut patch = build_patch(sample_rate, max_buffer_size, &factory);

  let stream = device
    .build_output_stream(
      stream_config,
      move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let frames = output.len() / 2;
        patch.process(frames);

        let buf = audio_buffer.lock().unwrap();
        for frame in 0..frames {
          output[frame * 2] = buf.left.get(frame).copied().unwrap_or(0.0);
          output[frame * 2 + 1] = buf.right.get(frame).copied().unwrap_or(0.0);
        }
      },
      |err| eprintln!("[Audio] Error: {}", err),
      None,
    )
    .expect("Failed to build stream");

  stream.play().expect("Failed to start stream");

  eprintln!("[Kahuna] Running. Press Enter to quit.");


  midi_queue.lock().unwrap().append(&mut vec![MidiEvent::NoteOn { note: 60, velocity: 100 }]);
  sleep(std::time::Duration::from_micros(300_000));
  midi_queue.lock().unwrap().append(&mut vec![MidiEvent::NoteOn { note: 64, velocity: 100 }]);
  sleep(std::time::Duration::from_micros(300_000));
  midi_queue.lock().unwrap().append(&mut vec![MidiEvent::NoteOn { note: 67, velocity: 100 }]);
  sleep(std::time::Duration::from_micros(300_000));
  midi_queue.lock().unwrap().append(&mut vec![MidiEvent::NoteOff { note: 60 }]);
  midi_queue.lock().unwrap().append(&mut vec![MidiEvent::NoteOff { note: 64 }]);
  midi_queue.lock().unwrap().append(&mut vec![MidiEvent::NoteOff { note: 67 }]);

  let mut buf = String::new();
  std::io::stdin().read_line(&mut buf).ok();
}
