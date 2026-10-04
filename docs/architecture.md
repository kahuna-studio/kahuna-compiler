# Kahuna — Architecture

## Overview

Kahuna compiles a patch — a graph of modules and converters — into a generated Rust crate. That crate can run
directly inside the Kahuna IDE, or be compiled further into a VST3/CLAP plugin or a standalone application.

## Model

- **Module** — an audio-rate processing unit with one or more input and/or output ports (e.g., an oscillator, a
  low-pass filter, an ADSR envelope).
- **Converter** — a lightweight transform between two streams; one input, one output (e.g. transforming a MIDI
  event stream into a frequency signal).
- **Unit** — a module or a converter. Used where something applies to both without distinguishing them.
- **Patch** — a graph that wires units together, using I/O nodes to connect to the outside world.
- **Parameter definition** — one value exposed for host automation: name, type, range. Hand-maintained; the
  compiler can suggest a starting set from a patch's unconnected ports and the parameters a unit's manifest
  declares, but the authored file is the source of truth, not something continuously derived.
- **Unit manifest** — the description a unit publishes about itself (name, ports), so a patch can reference it
  without inspecting its source.
- **Instrument** — a patch, together with the units it uses and the parameter definitions it exposes.

Cardinalities:

- An instrument has exactly one patch and exactly one parameters file.
- A patch wires together units (zero units would be possible but quite useless).
- Every unit belongs to exactly one unit crate (see below) and publishes exactly one unit manifest.

## Instrument structure

An instrument lives in a directory:

```
my-synth/                   instrument directory
  instrument.toml           metadata: name, vendor, version, targets
  parameters.json           hand-maintained parameter definitions
  patch.json                patch graph
  units/                    optional: this instrument's own unit crate
    Cargo.toml
    src/
```

`instrument.toml` is simple, flat, hand-written metadata. `patch.json` and `parameters.json` are
both structured and both touched by tooling (the IDE writes patches; the compiler can seed parameter
suggestions).

`units/` is optional. A unit crate is any crate that provides one or more units and their manifests — the
instrument's own `units/`, a separate library crate shared across instruments, `kahuna-std`, or any other crate.
A patch can reference units from any mix of these; it's entirely possible for an instrument to have no
instrument-local `units/` at all, using only units imported from elsewhere.

## Compiling an instrument

Kahuna's compiler is a CLI tool, installed separately from any one instrument. Compiling one looks like:

```
kahuna compile my-synth/ vst3
```

1. The compiler reads `instrument.toml`, `patch.json`, and `parameters.json`.
2. It resolves every unit the patch references, collecting the unit manifest of each — from `units/` if present, and
   from any other unit crates named by the instrument or the patch.
3. It validates `parameters.json` against the patch and the collected unit manifests: no entry may reference a port
   that's actually wired elsewhere in the graph, or a unit parameter that doesn't exist in that unit's manifest.
4. It generates a crate with the name of the instrument (`{instrument}`): the compiled patch, plus `PatchParameters`
   (live atomic handles) and the `PARAMETERS` const (static descriptors) — both derived from `parameters.json` —
   depending on `units/` (if present) and every other referenced unit crate.
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

**`{instrument}` is generated into its own crate, never into `units/`.** Nothing hand-written depends on the
generated patch — the dependency only goes the other way. That makes regeneration safe: the crate can be deleted
and rewritten on every patch change without touching hand-written code, and there's nothing in it a developer
would want to edit.

**Parameter definitions are hand-maintained, not derived.** The patch graph alone doesn't know about parameters
a unit's manifest declares independent of any port, so a purely derived list would always be incomplete. The
compiler can seed a starting file from the patch's unconnected ports and the manifests it collects, but
`parameters.json` itself is authored and validated, not generated fresh each time.

**Units publish a manifest instead of relying on the Cargo dependency alone.** Cargo already solves getting the
code. What it doesn't solve is letting the compiler know a unit's ports and name before generating code against
it — the manifest is what closes that gap, and what makes "import units from anywhere" actually work.

**Manifests are generated at build time and committed to the unit crate's own repo, not computed on demand.**
The alternative — discovering manifests by compiling and executing a unit crate's code at patch-compile time —
was rejected to avoid running arbitrary code just to resolve a patch. The trade-off: a manifest can fall out of
sync with its crate's actual code if the crate changes without rebuilding. Kahuna doesn't verify this itself;
keeping manifests current is left to the unit crate's own CI/CD pipeline.

**The instrument directory is addressed by path, not by crate name.** The compiler always looks for `units/`,
`patch.json`, and `parameters.json` at fixed relative locations, and reads `units/Cargo.toml` fresh each time it
needs the crate's name. Nothing caches that name elsewhere, so renaming the crate doesn't break anything.

**IO nodes are the one thing that varies between targets.** `{instrument}` never knows whether it's driven by a
DAW's buffer callback or by `cpal` directly — that's decided entirely in `{instrument}-plugin`.

## Crates reference

| Crate                 | Role                                                                  |
|------------------------|-----------------------------------------------------------------------|
| `kahuna-core`         | Module, port, and patch abstractions                                  |
| `kahuna-module`       | `#[kahuna_module]` macro for authoring units and emitting manifests   |
| `kahuna-std`          | Ready-made units, importable like any other unit crate                |
| `kahuna-plugin`       | Bridges `{instrument}` to `nice-plug`                                 |
| `kahuna-runtime`      | Audio/MIDI I/O for the standalone target                              |
| `{instrument}`        | Generated: the compiled patch                                         |
| `{instrument}-plugin` | Generated: the `nice-plug` wrapper, for plugin and standalone targets |
