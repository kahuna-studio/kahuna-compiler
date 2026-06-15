// ============================================================
// host/io_nodes.rs — hardware I/O as KahunaModule slots
//
// I/O nodes implement the same trait as DSP modules and operators.
// Their "process" is reading from / writing to a buffer that the
// runtime fills from hardware (or, in a plugin target, from the
// host) before and after the graph runs. The patch graph itself
// has no hardware knowledge — it only sees a node with inputs
// and outputs like any other.
// ============================================================

use crate::std_lib::midi::MidiEvent;

// ----------------------------------------------------------
// MidiIn — produces the MIDI event buffer for the current tick.
// No inputs. The runtime sets `pending` before calling process();
// process() just emits it as the output and clears it.
// ----------------------------------------------------------

pub struct MidiIn {
    pub pending: Vec<MidiEvent>,
}

impl MidiIn {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    // called by host BEFORE patch.process()
    pub fn set_events(&mut self, events: Vec<MidiEvent>) {
        self.pending = events;
    }

    // called by generated patch code
    pub fn take_events(&mut self) -> Vec<MidiEvent> {
        std::mem::take(&mut self.pending)
    }
}


pub struct AudioOut {
    pub out_0: Vec<f32>,
    pub out_1: Vec<f32>,
}

impl AudioOut {
    pub fn new(_sample_rate: u32, max_buffer_size: usize) -> Self {
        Self {
            out_0: vec![0.0f32; max_buffer_size],
            out_1: vec![0.0f32; max_buffer_size],
        }
    }
}
