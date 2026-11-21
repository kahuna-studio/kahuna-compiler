use std::fmt;

/// Why an instrument could not be started. The `Display` text is meant to be shown to the user as is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
  NoAudioOutputDevice,
  AudioDeviceNotFound { name: String, available: Vec<String> },
  UnsupportedSampleFormat { format: String },
  Audio(String),
  MidiPortNotFound { name: String, available: Vec<String> },
  Midi(String),
}

impl fmt::Display for RuntimeError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::NoAudioOutputDevice => write!(formatter, "No audio output device found"),
      Self::AudioDeviceNotFound { name, available } => {
        write!(formatter, "Audio output device '{name}' not found. Available: {}", listed(available))
      }
      Self::UnsupportedSampleFormat { format } => {
        write!(formatter, "The audio output device uses the sample format {format}, but Kahuna supports f32 only")
      }
      Self::Audio(message) | Self::Midi(message) => write!(formatter, "{message}"),
      Self::MidiPortNotFound { name, available } => {
        write!(formatter, "MIDI input port '{name}' not found. Available: {}", listed(available))
      }
    }
  }
}

impl std::error::Error for RuntimeError {}

fn listed(names: &[String]) -> String {
  if names.is_empty() { "none".to_string() } else { names.join(", ") }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
  }

  #[test]
  fn no_audio_device_is_reported_plainly() {
    assert_eq!(RuntimeError::NoAudioOutputDevice.to_string(), "No audio output device found");
  }

  #[test]
  fn an_unknown_audio_device_lists_the_available_ones() {
    let error = RuntimeError::AudioDeviceNotFound { name: "X".into(), available: names(&["A", "B"]) };

    assert_eq!(error.to_string(), "Audio output device 'X' not found. Available: A, B");
  }

  #[test]
  fn an_unknown_port_without_any_port_says_none() {
    let error = RuntimeError::MidiPortNotFound { name: "X".into(), available: vec![] };

    assert_eq!(error.to_string(), "MIDI input port 'X' not found. Available: none");
  }

  #[test]
  fn an_unsupported_sample_format_names_the_format() {
    let error = RuntimeError::UnsupportedSampleFormat { format: "I16".into() };

    assert!(error.to_string().contains("I16") && error.to_string().contains("f32"), "{error}");
  }

  #[test]
  fn messages_from_cpal_and_midir_are_shown_as_they_are() {
    assert_eq!(RuntimeError::Audio("Cannot open the audio stream: busy".into()).to_string(), "Cannot open the audio stream: busy");
    assert_eq!(RuntimeError::Midi("Cannot open the MIDI system".into()).to_string(), "Cannot open the MIDI system");
  }
}
