# Ports

A **port** is a named input or output of a module, connected to a stream. Ports define:

- **Name:** The variable identifier for the port.
- **Stream Type:** Signal (`~`), Parameter Stream (`@`), or Event Stream (`$`).
- **Value Type:** The type of values carried by the stream (e.g., `float`, `Midi_Message[]`).
- **Range** (for numeric types): Minimum and maximum values.
- **Processing Rate:** The processing rate of the stream (`audio_rate` or `control_rate`).
- **Interpolation** (for parameter streams; optional): Method for smooth transitions.
- **Default Value:** (for input ports with parameter streams) Used if no external connection is provided.

### Port Declaration Syntax

    <stream_type_symbol><name>: <value_type>[([<range>], [<rate>], [<interpolation>])] [= default]

- **Example**:


    module Filter:

      inputs:
        @cutoff:    frequency(20Hz..20_000Hz, control_rate, linear(max_step=100.0)) = 1000.0
        @resonance: float(0.0..1.0, control_rate) = 0.5
        $notes:     (Midi_Note_On|Midi_Note_Off)[]

      outputs:
        ~out:       audio
        $notes_out: (Midi_Note_On|Midi_Note_Off)[]

### Notes:

- **Parameter Streams:** Can be accessed without `@` inside pipelines: `@cutoff` points to the stream, `cutoff` to the current value.
- **Defaults:** Can only be set for inputs; outputs cannot have a default and must be assigned by the module code.

---

## Port Type Aliases

There are built-in port type aliases

    port_type audio   = float(-1.0..1.0, audio_rate)
    port_type control = float(0.0..1.0, control_rate)
    port_type midi    = Midi_Message[]

Common port types can be defined as **aliases** for reuse:

    port_type My_Frequency_Range = frequency(10Hz..100Hz, audio_rate)

You can override parameters of aliases if needed:

    @volume: control = 0.8       # Uses default range and rate of the port type alias 'control' 
    ~sig: audio(control_rate)    # Overrides rate to control_rate

---

## Example: White Noise Module

    module White_Noise:

      outputs:
        ~out: audio

      ~out = ~()
        | random(-1.0..1.0)

- `~out` is a signal.
- `~()` is a constructor for a signal of zeros at audio rate.
- The `| random(-1.0..1.0)` operator maps a random value to each frame of the stream.
