# Kahuna Stream Types

Kahuna has three kinds of streams that flow between module ports. All stream values are owned by `PatchState`. Modules only borrow — they never own wire values.

## The Three Types

**Signal** — a buffer of audio-rate samples, one per frame per tick.

**Event** — a variable-length list of zero or more events per tick (e.g. MIDI).

**Parameter** — a single scalar value per tick, control-rate.

## Rust Representation

| Stream type | State field | Producer receives | Consumer receives |
|---|---|---|---|
| Signal | `Vec<f32>` / `PolyBuffer<f32>` | `&mut [f32]` | `&[f32]` |
| Event | `Vec<MidiEvent>` | `&mut Vec<MidiEvent>` | `&[MidiEvent]` |
| Parameter | `f32` | `&mut f32` | `f32` |

Signal and event inputs are sliced to `frames` before being passed to modules:

```rust
&self.state.sine_osc_0__out.voice(voice)[..frames]
&mut self.state.sine_osc_0__out.voice_mut(voice)[..frames]
```

Parameters are `Copy` scalars — passed by value to consumers, no slicing needed.

## Module Constructor and Process Signatures

Every module is constructed with:

```rust
ModuleName::new(sample_rate: u32, max_buffer_size: usize)
```

`sample_rate` allows modules to precompute sample-rate-dependent values (e.g. ramp lengths, filter coefficients). `max_buffer_size` allows modules to pre-allocate internal scratch buffers.

The actual frame count per tick is implicit in the slice lengths passed to `process` — modules never receive a separate `buffer_size` parameter.

## Ownership Invariant

State owns all wire values. Each tick:

1. The event state fields are cleared.
2. Modules are called in topological order.
3. Each module writes into its output state field(s) and reads from its input state field(s).
4. No module allocates on the audio thread.