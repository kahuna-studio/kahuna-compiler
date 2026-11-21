//! Runs a compiled instrument on the audio output and MIDI input of the system.
//!
//! Two layers: `start` opens the devices and returns a `RunningInstrument` that plays until it is
//! dropped. `run_standalone` is what the generated `main.rs` of an instrument calls: it starts with
//! the system's default devices and plays until the user presses Enter. A host such as the Studio
//! can use `start` and the device lists directly.

mod audio;
mod error;
pub mod io_nodes;
pub mod midi;

pub use error::RuntimeError;

use crate::io_nodes::{AudioBuffer, AudioBufferData, CpalIoNodeFactory};
use cpal::traits::{DeviceTrait, StreamTrait};
use kahuna_core::io_nodes::IoNodeFactory;
use kahuna_core::patch::KahunaPatch;
use std::sync::{Arc, Mutex};

/// Which devices an instrument runs on. `None` is the system default, a name selects the device
/// with exactly that name (see `list_audio_outputs` and `list_midi_inputs`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeConfig {
  pub audio_output: Option<String>,
  pub midi_input: Option<String>,
}

/// An instrument that is playing. Audio and MIDI stop when it is dropped.
pub struct RunningInstrument {
  _stream: cpal::Stream,
  _midi_connection: Option<midir::MidiInputConnection<()>>,
}

/// Plays the instrument on the default devices until the user presses Enter, or stdin is closed.
pub fn run_standalone<P, F>(max_buffer_size: usize, build_patch: F) -> Result<(), RuntimeError>
where
  P: KahunaPatch + Send + 'static,
  F: FnOnce(u32, usize, &dyn IoNodeFactory) -> P,
{
  let running_instrument = start(&RuntimeConfig::default(), max_buffer_size, build_patch)?;
  eprintln!("[Kahuna] Running. Press Enter to quit.");
  wait_for_enter();
  drop(running_instrument);
  Ok(())
}

/// Opens the devices, builds the patch for them and starts playing. The patch is built with the
/// sample rate of the device and gets at most `max_buffer_size` frames per `process` call, whatever
/// buffer size the device delivers. Startup is described on stderr; nothing is printed while running.
pub fn start<P, F>(config: &RuntimeConfig, max_buffer_size: usize, build_patch: F) -> Result<RunningInstrument, RuntimeError>
where
  P: KahunaPatch + Send + 'static,
  F: FnOnce(u32, usize, &dyn IoNodeFactory) -> P,
{
  let output = audio::output_settings(config.audio_output.as_deref(), max_buffer_size)?;
  for line in audio::describe(&output, max_buffer_size) {
    eprintln!("{line}");
  }

  let midi_queue = Arc::new(Mutex::new(Vec::new()));
  let midi_connection = midi::start_midi(Arc::clone(&midi_queue), config.midi_input.as_deref())?;
  let audio_buffer: AudioBuffer = Arc::new(Mutex::new(AudioBufferData::default()));
  let factory = CpalIoNodeFactory { midi_queue, audio_buffer: Arc::clone(&audio_buffer) };
  let patch = build_patch(output.stream_config.sample_rate, max_buffer_size, &factory);

  let stream = start_stream(&output, patch, audio_buffer, max_buffer_size)?;
  Ok(RunningInstrument { _stream: stream, _midi_connection: midi_connection })
}

/// The names of the audio output devices that are available now.
pub fn list_audio_outputs() -> Result<Vec<String>, RuntimeError> {
  audio::list_outputs()
}

/// The names of the MIDI input ports that are available now.
pub fn list_midi_inputs() -> Result<Vec<String>, RuntimeError> {
  midi::list_inputs()
}

fn start_stream<P>(
  output: &audio::OutputSettings,
  mut patch: P,
  audio_buffer: AudioBuffer,
  max_buffer_size: usize,
) -> Result<cpal::Stream, RuntimeError>
where
  P: KahunaPatch + Send + 'static,
{
  let channels = output.stream_config.channels as usize;
  let stream = output
    .device
    .build_output_stream(
      output.stream_config,
      move |samples: &mut [f32], _: &cpal::OutputCallbackInfo| {
        audio::render(&mut patch, &audio_buffer, channels, max_buffer_size, samples);
      },
      |error| eprintln!("[Audio] Error: {error}"),
      None,
    )
    .map_err(|error| RuntimeError::Audio(format!("Cannot open the audio stream on '{}': {error}", output.device_name)))?;
  stream
    .play()
    .map_err(|error| RuntimeError::Audio(format!("Cannot start the audio stream on '{}': {error}", output.device_name)))?;
  Ok(stream)
}

fn wait_for_enter() {
  let mut line = String::new();
  std::io::stdin().read_line(&mut line).ok();
}
