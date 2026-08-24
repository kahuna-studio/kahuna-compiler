# kahuna-synth — compiler target

Hand-written Rust code representing the desired output of the Kahuna compiler.
Once the compiler can produce equivalent code from Kahuna source, this directory is deleted.

## What it does

Polyphonic sine oscillator controlled by MIDI, with a gate envelope, running via cpal.

```
MIDI In ──┬──► SineOsc ──────────► Multiply ──► Audio Out
          └──► GateEnvelope ──────►
```

## Prerequisites

- Rust toolchain (`rustup.rs`)
- On Linux: `sudo apt install libasound2-dev`
- A MIDI keyboard or virtual MIDI port (optional — no sound without MIDI input)

## Run

```
cargo run
```

Connect a MIDI keyboard, play notes.