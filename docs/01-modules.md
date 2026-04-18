# Modules and Ports

Kahuna programs are built from two main building blocks: **modules** and **patches** (alongside function declarations
and structures like records and enums).

Modules define **processing units** that operate on streams of data, while patches describe how modules are connected.
Think of modules like the modules in modular synthesizers. A module can be a sine wave oscillator, an envelope
generator, or a low pass filter.

---

## What is a Module?

A **module** is a self-contained unit that processes streams of data. Each module declares its **inputs** and
**outputs**, and defines a processing pipeline that transforms input streams into output streams.

Modules are reusable and can be instantiated multiple times in a patch.

    module Osc:

      inputs:
        @frequency:    frequency = 440.0
        @phase_offset: control = 0.0
        @amplitude:    control = 1.0
        @waveform:     Waveform = Waveform.sine

      outputs:
        ~output: audio

      ~output = Audio_Rate_Phase(@frequency)
        | + phase_offset
        |-> phase
        ? waveform
          : sine     -> sin(phase * 2 * PI)
          : saw      -> 2.0 * (phase % 1.0) - 1.0
          : square   -> (phase % 1.0) < 0.5 ? 1.0 : -1.0
          : triangle -> abs(4.0 * ((phase % 1.0) - 0.5)) - 1.0
        | * amplitude

Modules are stateless.

[--> Ports](02-ports.md)