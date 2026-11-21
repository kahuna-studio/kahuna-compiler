use crate::io_nodes::AudioBuffer;
use crate::RuntimeError;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{BufferSize, SampleFormat, SupportedBufferSize};
use kahuna_core::patch::KahunaPatch;
use std::ops::Range;

/// The output device and the stream settings the instrument runs with.
pub(crate) struct OutputSettings {
  pub device: cpal::Device,
  pub device_name: String,
  pub stream_config: cpal::StreamConfig,
}

/// The names of the audio output devices that are available now.
pub fn list_outputs() -> Result<Vec<String>, RuntimeError> {
  let devices = output_devices()?;
  Ok(devices.into_iter().map(|(name, _)| name).collect())
}

/// Finds the requested output device, or the system default, and decides the stream settings:
/// the device's default format, with a fixed buffer size where the device supports it.
pub(crate) fn output_settings(requested: Option<&str>, max_buffer_size: usize) -> Result<OutputSettings, RuntimeError> {
  let (device_name, device) = find_output_device(requested)?;
  let config = device
    .default_output_config()
    .map_err(|error| RuntimeError::Audio(format!("Cannot read the audio output configuration of '{device_name}': {error}")))?;
  if config.sample_format() != SampleFormat::F32 {
    return Err(RuntimeError::UnsupportedSampleFormat { format: format!("{:?}", config.sample_format()) });
  }

  let buffer_size = choose_buffer_size(config.buffer_size(), max_buffer_size as cpal::FrameCount);
  let mut stream_config: cpal::StreamConfig = config.into();
  stream_config.buffer_size = buffer_size;
  Ok(OutputSettings { device, device_name, stream_config })
}

/// One line each for what the instrument runs on.
pub(crate) fn describe(settings: &OutputSettings, max_buffer_size: usize) -> [String; 4] {
  let buffer_size = match settings.stream_config.buffer_size {
    BufferSize::Fixed(frames) => format!("{frames} frames"),
    BufferSize::Default => format!("device default ({max_buffer_size} frames not supported)"),
  };
  [
    format!("[Audio] Device: {}", settings.device_name),
    format!("[Audio] Sample rate: {}", settings.stream_config.sample_rate),
    format!("[Audio] Channels: {}", settings.stream_config.channels),
    format!("[Audio] Buffer size: {buffer_size}"),
  ]
}

/// What the audio callback does: runs the patch for the frames of `output`, in chunks of at most
/// `max_buffer_size` frames, and writes what the patch produced to `output`, interleaved.
pub(crate) fn render<P: KahunaPatch>(
  patch: &mut P,
  audio_buffer: &AudioBuffer,
  channels: usize,
  max_buffer_size: usize,
  output: &mut [f32],
) {
  if channels == 0 {
    return;
  }
  let frames = output.len() / channels;
  for chunk in chunk_ranges(frames, max_buffer_size) {
    patch.process(chunk.len());
    let produced = audio_buffer.lock().unwrap();
    write_interleaved(&produced.left, &produced.right, channels, &mut output[chunk.start * channels..chunk.end * channels]);
  }
}

/// The frame ranges of consecutive chunks that are at most `max_chunk` frames long.
pub(crate) fn chunk_ranges(frames: usize, max_chunk: usize) -> impl Iterator<Item = Range<usize>> {
  let max_chunk = max_chunk.max(1);
  (0..frames).step_by(max_chunk).map(move |start| start..(start + max_chunk).min(frames))
}

/// Writes the left and right signal interleaved for a device with `channels` channels. Mono gets the
/// mix of both, more than two channels get left and right on the first two and silence on the rest.
/// Frames the signal does not have are silent.
pub(crate) fn write_interleaved(left: &[f32], right: &[f32], channels: usize, output: &mut [f32]) {
  if channels == 0 {
    return;
  }
  for (frame, samples) in output.chunks_exact_mut(channels).enumerate() {
    let left_sample = left.get(frame).copied().unwrap_or(0.0);
    let right_sample = right.get(frame).copied().unwrap_or(0.0);
    samples.fill(0.0);
    if channels == 1 {
      samples[0] = (left_sample + right_sample) / 2.0;
    } else {
      samples[0] = left_sample;
      samples[1] = right_sample;
    }
  }
}

/// The index of the entry with exactly this name.
pub(crate) fn find_by_name(names: &[String], wanted: &str) -> Option<usize> {
  names.iter().position(|name| name == wanted)
}

/// A fixed buffer size where the device supports it, otherwise the device's own choice. Chunking in
/// `render` makes the device's choice safe.
fn choose_buffer_size(supported: &SupportedBufferSize, wanted: cpal::FrameCount) -> BufferSize {
  match supported {
    SupportedBufferSize::Range { min, max } if (*min..=*max).contains(&wanted) => BufferSize::Fixed(wanted),
    _ => BufferSize::Default,
  }
}

fn find_output_device(requested: Option<&str>) -> Result<(String, cpal::Device), RuntimeError> {
  let Some(wanted) = requested else {
    let device = cpal::default_host().default_output_device().ok_or(RuntimeError::NoAudioOutputDevice)?;
    return Ok((device_name(&device)?, device));
  };
  let mut devices = output_devices()?;
  let names: Vec<String> = devices.iter().map(|(name, _)| name.clone()).collect();
  match find_by_name(&names, wanted) {
    Some(index) => Ok(devices.swap_remove(index)),
    None => Err(RuntimeError::AudioDeviceNotFound { name: wanted.to_string(), available: names }),
  }
}

fn output_devices() -> Result<Vec<(String, cpal::Device)>, RuntimeError> {
  let devices = cpal::default_host()
    .output_devices()
    .map_err(|error| RuntimeError::Audio(format!("Cannot list the audio output devices: {error}")))?;
  devices.map(|device| Ok((device_name(&device)?, device))).collect()
}

fn device_name(device: &cpal::Device) -> Result<String, RuntimeError> {
  device
    .description()
    .map(|description| description.name().to_string())
    .map_err(|error| RuntimeError::Audio(format!("Cannot read the name of an audio device: {error}")))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::io_nodes::AudioBufferData;
  use std::sync::{Arc, Mutex};

  fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
  }

  struct RecordingPatch {
    audio_buffer: AudioBuffer,
    frames_per_call: Vec<usize>,
  }

  impl KahunaPatch for RecordingPatch {
    fn process(&mut self, frames: usize) {
      self.frames_per_call.push(frames);
      let mut produced = self.audio_buffer.lock().unwrap();
      produced.left = vec![1.0; frames];
      produced.right = vec![0.5; frames];
    }
  }

  fn render_frames(frames: usize, channels: usize, max_buffer_size: usize) -> (Vec<usize>, Vec<f32>) {
    let audio_buffer: AudioBuffer = Arc::new(Mutex::new(AudioBufferData::default()));
    let mut patch = RecordingPatch { audio_buffer: audio_buffer.clone(), frames_per_call: vec![] };
    let mut output = vec![-1.0; frames * channels];

    render(&mut patch, &audio_buffer, channels, max_buffer_size, &mut output);

    (patch.frames_per_call, output)
  }

  #[test]
  fn a_callback_larger_than_the_buffer_is_split_into_chunks() {
    let (frames_per_call, _) = render_frames(5000, 2, 2048);

    assert_eq!(frames_per_call, [2048, 2048, 904]);
  }

  #[test]
  fn a_callback_of_exactly_the_buffer_size_is_one_chunk() {
    let (frames_per_call, _) = render_frames(2048, 2, 2048);

    assert_eq!(frames_per_call, [2048]);
  }

  #[test]
  fn a_callback_without_frames_calls_the_patch_never() {
    let (frames_per_call, output) = render_frames(0, 2, 2048);

    assert!(frames_per_call.is_empty());
    assert!(output.is_empty());
  }

  #[test]
  fn every_frame_of_every_chunk_is_written() {
    let (_, output) = render_frames(5000, 2, 2048);

    assert!(output.chunks(2).all(|frame| frame == [1.0, 0.5]));
  }

  #[test]
  fn chunks_follow_each_other_without_gaps() {
    let ranges: Vec<_> = chunk_ranges(5000, 2048).collect();

    assert_eq!(ranges, [0..2048, 2048..4096, 4096..5000]);
  }

  #[test]
  fn a_chunk_size_of_zero_does_not_loop_forever() {
    assert_eq!(chunk_ranges(3, 0).count(), 3);
  }

  #[test]
  fn two_channels_get_left_and_right() {
    let mut output = [-1.0; 4];

    write_interleaved(&[0.1, 0.2], &[0.3, 0.4], 2, &mut output);

    assert_eq!(output, [0.1, 0.3, 0.2, 0.4]);
  }

  #[test]
  fn a_mono_device_gets_the_mix_of_both_channels() {
    let mut output = [-1.0; 2];

    write_interleaved(&[1.0, 0.0], &[0.0, 0.5], 1, &mut output);

    assert_eq!(output, [0.5, 0.25]);
  }

  #[test]
  fn more_than_two_channels_get_left_and_right_first_and_silence_after() {
    let mut output = [-1.0; 8];

    write_interleaved(&[0.1, 0.2], &[0.3, 0.4], 4, &mut output);

    assert_eq!(output, [0.1, 0.3, 0.0, 0.0, 0.2, 0.4, 0.0, 0.0]);
  }

  #[test]
  fn frames_the_signal_does_not_have_are_silent() {
    let mut output = [-1.0; 4];

    write_interleaved(&[0.1], &[], 2, &mut output);

    assert_eq!(output, [0.1, 0.0, 0.0, 0.0]);
  }

  #[test]
  fn a_device_is_found_by_its_exact_name() {
    let devices = names(&["Built-in", "USB Interface"]);

    assert_eq!(find_by_name(&devices, "USB Interface"), Some(1));
    assert_eq!(find_by_name(&devices, "usb interface"), None);
    assert_eq!(find_by_name(&devices, "Nothing"), None);
  }

  #[test]
  fn the_fixed_buffer_size_is_used_where_the_device_supports_it() {
    let supported = SupportedBufferSize::Range { min: 64, max: 4096 };

    assert_eq!(choose_buffer_size(&supported, 2048), BufferSize::Fixed(2048));
    assert_eq!(choose_buffer_size(&supported, 64), BufferSize::Fixed(64));
  }

  #[test]
  fn the_device_default_is_used_where_the_fixed_size_is_out_of_range_or_unknown() {
    assert_eq!(choose_buffer_size(&SupportedBufferSize::Range { min: 64, max: 1024 }, 2048), BufferSize::Default);
    assert_eq!(choose_buffer_size(&SupportedBufferSize::Unknown, 2048), BufferSize::Default);
  }
}
