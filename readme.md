# Kahuna Compiler

The compiler of [Kahuna Studio](https://codeberg.org/kahuna-studio/kahuna-studio), the application for building
electronic instruments by patching and exporting them as plugins or standalone programs. The Studio's readme has the
vision. This repository holds what is relevant for compiling.

The compiler turns a Kahuna instrument into a Rust crate that compiles to a standalone program, using units from Kahuna
libraries. Plugin targets are planned.

---

## Current state

What works today:

- **Instrument compilation.** `kahuna-compiler build` turns an instrument directory (`instrument.toml` and
  `patch.json`) into a Rust crate: a `Patch` type with the processing code for the whole graph, including polyphonic
  voice regions, and a `main.rs` that runs it with MIDI input and audio output. `build` then runs
  `cargo build --release` on the crate and copies the standalone executable to `.kahuna/bin/<name>`. An instrument
  without a `.gitignore` gets one that ignores `.kahuna/`.
- **Validation.** The patch is checked against the unit catalog. An unknown unit is reported with the node and the
  unit it names; nothing is generated from a patch that does not pass.
- **Kahuna libraries.** Units are plain Rust, declared with the attribute macros `#[kahuna_module]`,
  `#[kahuna_converter]` and `#[kahuna_value_operator]`. A library publishes a `kahuna-library.json` manifest with the
  ports of its units. The compiler generates that manifest from the library's source, using the same parser as the
  macros, so both agree on what a unit looks like.
- **Standard library.** `kahuna-std` has a sine oscillator, an ADSR envelope, MIDI converters, and arithmetic,
  comparison and mixing operators.
- **Watch mode.** `kahuna-compiler watch` rechecks an instrument whenever its files change. It is built on
  [salsa](https://github.com/salsa-rs/salsa), so only what a changed file affects is recomputed.
- **Kahuna source code, front end.** A [pest](https://pest.rs) grammar with an indentation preprocessor, a Pratt parser
  for expressions, an AST, and serializers that write the AST back as Kahuna source and as JSON. It is documented in
  [docs/language](docs/language/01-modules.md).

Not yet there, roughly in the order I intend to tackle it:

- Plugin targets (VST3, CLAP) through `nice-plug`, the successor of nih-plug, with exposed parameters for host
  automation.
- Instruments that use libraries other than `kahuna-std`, listed in `instrument.toml`.
- Generating Rust from Kahuna source code. Units are written in Rust directly for now.

---

## A personal playground

Kahuna is my personal audio playground: a place to explore how far a toolchain for electronic instruments can go,
from a language for describing signal processing to instruments that run as standalone programs and, later, as
plugins. It started as the design of an audio DSL and a compiler for it, and a good part of that work is in this
repository. There is no fixed roadmap; the list above is the direction I am heading in at the moment.

---

## What an instrument is

An instrument is a directory with two files and no code:

```
my-synth/
  instrument.toml     name, vendor, version
  patch.json          the patch graph: units, and the cables between them
```

The patch wires together units: modules (oscillators, envelopes, filters), converters (a MIDI event stream into a
frequency) and value operators (multiply, mix, scale).

```
kahuna-compiler build my-synth/
```

writes the crate to `my-synth/.kahuna/rust/`, builds it with cargo and puts the executable in
`my-synth/.kahuna/bin/`. Everything in `.kahuna/` is generated and can be deleted at any time.

---

## Try it

You need a Rust toolchain. On Linux, `kahuna-runtime` needs the ALSA headers (`sudo apt install libasound2-dev`).

```
cargo run -p kahuna-compiler -- build kahuna-compiler/tests/fixtures/instruments/golden-synth \
  --output-dir /tmp/golden-synth
```

The golden synth is a polyphonic sine oscillator with a gate envelope. `--output-dir` keeps the generated files out of
the repository; without it they go to `.kahuna/` in the instrument directory. The first build compiles all
dependencies and takes a few minutes. When it is done, `build` prints where the executable is
(`/tmp/golden-synth/bin/golden-synth`) and how to run it.

`kahuna-compiler --help` lists the other commands: `watch` and `manifest`, which writes the manifest of a unit
library.

---

## Crates

| Crate                | Role                                                            |
|----------------------|-----------------------------------------------------------------|
| `kahuna-compiler`    | CLI and library: instrument compiler, language front end        |
| `kahuna-core`        | Unit and patch abstractions, stream and port types              |
| `kahuna-unit`        | The `#[kahuna_*]` attribute macros                              |
| `kahuna-unit-syntax` | Parser for unit declarations, shared by macros and compiler     |
| `kahuna-std`         | Standard unit library                                           |
| `kahuna-runtime`     | Audio (cpal) and MIDI (midir) I/O for standalone instruments   |
| `golden_output`      | Hand-written instrument that the generated code is compared to  |

More background is in [docs/architecture.md](docs/architecture.md).

---

## License

[MIT](./license.md)