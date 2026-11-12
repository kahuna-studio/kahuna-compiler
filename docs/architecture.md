# Kahuna — Architecture

## Overview

Kahuna compiles a patch — a graph of modules and converters — into a generated Rust crate. That crate can run
directly inside the Kahuna IDE, or be compiled further into a VST3/CLAP plugin or a standalone application.

## Model

- **Module** — an audio-rate processing unit with one or more input and/or output ports (e.g., an oscillator, a
  low-pass filter, an ADSR envelope).
- **Converter** — a lightweight transform between two streams; one input, one output (e.g. transforming a MIDI
  event stream into a frequency signal).
- **Value operator** — arithmetic on single values with no sense of time, e.g. multiply, add, clamp, crossfade.
  Written as a plain function; the compiler calls it once per sample, so it works on audio signals and
  parameters alike, depending on what is connected.
- **Unit** — a module, a converter, or a value operator. Used where something applies to all of them without
  distinguishing them.
- **Patch** — a graph that wires units together, using I/O nodes to connect to the outside world.
- **Parameter definition** — one value exposed for host automation: name, type, range. Hand-maintained; the
  compiler can suggest a starting set from a patch's unconnected ports and the parameters a library manifest
  declares for a unit, but the authored file is the source of truth, not something continuously derived.
- **Library** (unit library) — a collection of units that is distributed and versioned together, e.g. `kahuna-std`,
  or a designer's private collection. Technically a Rust library crate, kept outside the instrument directory.
- **Library manifest** — `kahuna-library.json`, the description a library publishes of its units (id, ports), so a
  patch can reference them without inspecting their source.
- **Instrument** — a patch, together with the units it uses and the parameter definitions it exposes.

Cardinalities:

- An instrument has exactly one patch and exactly one parameters file.
- A patch wires together units (zero units would be possible but quite useless).
- Every unit belongs to exactly one library and has exactly one entry in that library's manifest.
- A library has exactly one manifest and one or more units.
- An instrument uses any number of libraries: `kahuna-std`, plus those listed in `instrument.toml`.

## Instrument structure

An instrument lives in a directory:

```
my-synth/                   instrument directory
  instrument.toml           metadata: name, vendor, version, targets, [libraries]
  parameters.json           hand-maintained parameter definitions
  patch.json                patch graph
  .kahuna/                  generated, regenerated on every compile, not committed
    rust/                   the generated Rust project (Cargo.toml, src/, target/)
    bin/                    the compiled instrument
```

There is no Rust in the instrument directory. Units come from libraries, which live anywhere on disk or in a
repository, and an IDE works the same way wherever they are.

`instrument.toml` is simple, flat, hand-written metadata. `patch.json` and `parameters.json` are
both structured and both touched by tooling (the IDE writes patches; the compiler can seed parameter
suggestions).

A library is any Rust library crate that provides one or more units and a manifest — a private collection shared
across instruments, `kahuna-std`, or any other library. Every library except `kahuna-std` is listed in
`instrument.toml` under `[libraries]`, written like a Cargo dependency (version, `path`, or `git`). A patch can
reference units from any mix of these as `{library}/{unit id}`, e.g. `kahuna-std/adsr`.

Currently `[libraries]` is not supported yet: the compiler rejects it. Every instrument gets `kahuna-std` as an
implicit dependency, without listing it.

## Compiling an instrument

Kahuna's compiler is a CLI tool, installed separately from any one instrument. Compiling one looks like:

```
kahuna compile my-synth/ vst3
```

1. The compiler reads `instrument.toml`, `patch.json`, and `parameters.json`.
2. It locates every library — `kahuna-std` and those listed under `[libraries]` — via `cargo metadata`, and
   reads their manifests.
3. It validates `parameters.json` against the patch and the collected manifests: no entry may reference a port
   that's actually wired elsewhere in the graph, or a unit parameter that doesn't exist in that unit's manifest.
4. It generates a crate with the name of the instrument (`{instrument}`) in `.kahuna/rust/`: the compiled patch, plus
   `PatchParameters` (live atomic handles) and the `PARAMETERS` const (static descriptors) — both derived from
   `parameters.json` — depending on every library.
5. For a plugin or standalone target, it also generates `{instrument}-plugin`, wrapping `{instrument}` via
   `nice-plug`.
6. `cargo build` compiles the generated crate into the requested artifact: a `.vst3`/`.clap` bundle, or a
   standalone binary.

Playing an instrument inside the IDE follows the same steps 1–4, but stops there — the IDE loads `{instrument}`
directly, in-process, without a plugin wrapper or packaging step.

## Why

**The generated patch crate is named `{instrument}`, not `{instrument}-patch`.** It's the primary artifact; the
instrument's own name should be the plain one, with supporting crates suffixed off it — `{instrument}-plugin`
depends on `{instrument}`, not on an implementation detail of what it was generated from.

**`{instrument}` is generated into `.kahuna/rust/`, never next to hand-written code.** Nothing hand-written depends
on the generated patch — the dependency only goes the other way. That makes regeneration safe: the project can be
deleted and rewritten on every patch change without touching hand-written code, and there's nothing in it a
developer would want to edit. The directory has a fixed name rather than the instrument's, so nothing else in
`.kahuna/` can collide with an instrument called `bin`.

**Instruments contain no Rust.** Custom units are written in libraries, with whatever tools the developer already
uses for Rust, and an instrument refers to them under `[libraries]`. The trade-off: a repository that holds only the
instrument doesn't contain its custom units; a library is its own repository, or the instrument repository is split
into `instrument/` and `crates/`.

**Parameter definitions are hand-maintained, not derived.** The patch graph alone doesn't know about parameters
a library manifest declares for a unit independent of any port, so a purely derived list would always be incomplete. The
compiler can seed a starting file from the patch's unconnected ports and the manifests it collects, but
`parameters.json` itself is authored and validated, not generated fresh each time.

**Libraries publish a manifest instead of relying on the Cargo dependency alone.** Cargo already solves getting the
code. What it doesn't solve is letting the compiler know a unit's ports and name before generating code against
it — the manifest is what closes that gap, and what makes "import units from anywhere" actually work.

**Manifests are generated from source and committed to the library's own repo, not computed on demand.**
`kahuna-compiler manifest` reads the library's source with `syn` and writes `kahuna-library.json`; the
`#[kahuna_module]` macro and this generator share one parser (`kahuna-unit-syntax`), so they always agree on a
unit's id and ports. The alternative — discovering manifests by compiling and executing a library's code at
patch-compile time — was rejected to avoid running arbitrary code just to resolve a patch. The trade-off: a
manifest can fall out of sync with its library's code if the source changes without regenerating it. Kahuna
doesn't verify this itself; keeping manifests current is left to the library's own CI/CD pipeline.

**Units are referenced by id, not by Rust path.** An id defaults to the struct name in kebab-case
(`AdsrEnvelope` → `adsr-envelope`) and can be pinned with `#[kahuna_module(id = "...")]`, so renaming or moving a
struct doesn't have to break existing patches. Ids are short names that match `[a-z][a-z0-9]*(-[a-z0-9]+)*`
(`to-freq`, `adsr`), because the IDE builds node ids from them (`to-freq_0`), and an id is unique within its library:
the compiler rejects a `kahuna-library.json` that uses one twice. Where two libraries of an instrument have a unit with
the same id, the catalog gives both a short name with the library and an underscore as prefix (`kahuna-std_sine`,
`my-effects_sine`); references in patches always carry the library, so they stay `kahuna-std/sine` and
`my-effects/sine`.

**The instrument directory is addressed by path, not by crate name.** The compiler always looks for `instrument.toml`,
`patch.json`, and `parameters.json` at fixed relative locations, so renaming an instrument or a library crate
doesn't break anything.

**IO nodes are the one thing that varies between targets.** `{instrument}` never knows whether it's driven by a
DAW's buffer callback or by `cpal` directly — that's decided entirely in `{instrument}-plugin`.

## Crates reference

| Crate                 | Role                                                                  |
|------------------------|-----------------------------------------------------------------------|
| `kahuna-core`         | Module, port, and patch abstractions                                  |
| `kahuna-unit`         | `#[kahuna_module]`, `#[kahuna_converter]`, `#[kahuna_value_operator]` macros |
| `kahuna-unit-syntax`  | Parser for unit declarations, shared by the macro and the compiler    |
| `kahuna-std`          | Ready-made units, importable like any other library                   |
| `kahuna-plugin`       | Bridges `{instrument}` to `nice-plug`                                 |
| `kahuna-runtime`      | Audio/MIDI I/O for the standalone target                              |
| `{instrument}`        | Generated: the compiled patch                                         |
| `{instrument}-plugin` | Generated: the `nice-plug` wrapper, for plugin and standalone targets |
