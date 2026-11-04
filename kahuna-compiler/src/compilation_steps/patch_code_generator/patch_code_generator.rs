use crate::compilation_steps::patch_code_generator::analysis::{analyze, AnalyzedNode, AnalyzedPatch, Behavior, BufferRef, Connection, Domain, MergeMode, ResolvedInput};
use crate::compilation_steps::patch_code_generator::catalog::{Arity, CallStyle, UnitCatalog, UnitSpec, ValueKind};
use eyre::{bail, eyre, Result, WrapErr};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use crate::compilation_steps::patch_file_parser::patch_graph::PatchGraph;

pub fn generate_patch_source(graph: &PatchGraph, catalog: &dyn UnitCatalog) -> Result<String> {
  let patch = analyze(graph, catalog)?;
  let tokens = emit_file(&patch)?;
  let file = syn::parse2::<syn::File>(tokens).wrap_err("generated tokens are not valid Rust")?;
  Ok(prettyplease::unparse(&file))
}

fn emit_file(patch: &AnalyzedPatch) -> Result<TokenStream> {
  let imports = emit_imports();
  let state = emit_state(patch);
  let patch_type = emit_patch(patch)?;
  let process = emit_process(patch)?;

  Ok(quote! {
        #imports
        #state
        #patch_type
        #process
    })
}

fn emit_imports() -> TokenStream {
  quote! {
        #[allow(unused_imports)]
        use kahuna_core::{
            io_nodes::{AudioOutputNode, IoNodeFactory, MidiInputNode},
            midi::{
                polyphony::{poly_buffer::PolyBuffer, voice_allocator::VoiceAllocator},
                MidiEvent,
            },
            patch::KahunaPatch,
            KahunaConverter, KahunaModule, ModuleEnv,
        };
    }
}

// ---------------------------------------------------------------------------
// State: one buffer per output port
// ---------------------------------------------------------------------------

fn emit_state(patch: &AnalyzedPatch) -> TokenStream {
  let buffers = patch
    .nodes
    .iter()
    .flat_map(|node| node.outputs.iter())
    .collect::<Vec<_>>();
  let field_idents = buffers
    .iter()
    .map(|buffer| buffer_field(buffer))
    .collect::<Vec<_>>();
  let field_types = buffers.iter().map(|buffer| buffer_type(buffer));
  let initializers = buffers.iter().map(|buffer| buffer_initializer(buffer));
  let voice_parameters = patch
    .poly_regions()
    .iter()
    .map(|region| voices_ident(&region.id))
    .collect::<Vec<_>>();

  quote! {
        #[allow(non_snake_case)]
        pub struct PatchState {
            #(#field_idents: #field_types,)*
        }

        impl PatchState {
            pub fn new(#(#voice_parameters: usize,)* max_buffer_size: usize) -> Self {
                Self {
                    #(#field_idents: #initializers,)*
                }
            }
        }
    }
}

fn buffer_type(buffer: &BufferRef) -> TokenStream {
  match (buffer.kind, &buffer.domain) {
    (ValueKind::Events, Domain::Mono) => quote!(Vec<MidiEvent>),
    (ValueKind::Events, Domain::Poly { .. }) => quote!(Vec<Vec<MidiEvent>>),
    (ValueKind::Signal, Domain::Mono) => quote!(Vec<f32>),
    (ValueKind::Signal, Domain::Poly { .. }) => quote!(PolyBuffer<f32>),
    (ValueKind::Float, Domain::Mono) => quote!(f32),
    (ValueKind::Float, Domain::Poly { .. }) => quote!(Vec<f32>),
    (ValueKind::Bool, Domain::Mono) => quote!(bool),
    (ValueKind::Bool, Domain::Poly { .. }) => quote!(Vec<bool>),
    (ValueKind::Number, _) => unreachable!("Number is a port kind, never the kind of a buffer"),
  }
}

fn buffer_initializer(buffer: &BufferRef) -> TokenStream {
  match (buffer.kind, &buffer.domain) {
    (ValueKind::Events, Domain::Mono) => quote!(Vec::new()),
    (ValueKind::Events, Domain::Poly { poly_id }) => {
      let voices = voices_ident(poly_id);
      quote!((0..#voices).map(|_| Vec::new()).collect())
    }
    (ValueKind::Signal, Domain::Mono) => quote!(vec![0.0; max_buffer_size]),
    (ValueKind::Signal, Domain::Poly { poly_id }) => {
      let voices = voices_ident(poly_id);
      quote!(PolyBuffer::new(#voices, max_buffer_size))
    }
    (ValueKind::Float, Domain::Mono) => quote!(0.0),
    (ValueKind::Float, Domain::Poly { poly_id }) => {
      let voices = voices_ident(poly_id);
      quote!(vec![0.0; #voices])
    }
    (ValueKind::Bool, Domain::Mono) => quote!(false),
    (ValueKind::Bool, Domain::Poly { poly_id }) => {
      let voices = voices_ident(poly_id);
      quote!(vec![false; #voices])
    }
    (ValueKind::Number, _) => unreachable!("Number is a port kind, never the kind of a buffer"),
  }
}

// ---------------------------------------------------------------------------
// Patch struct and constructor: one instance per node (one per voice in poly regions)
// ---------------------------------------------------------------------------

struct InstanceCode {
  ident: Ident,
  field_type: TokenStream,
  initializer: TokenStream,
}

fn emit_patch(patch: &AnalyzedPatch) -> Result<TokenStream> {
  let instances = patch
    .nodes
    .iter()
    .map(instance_code)
    .collect::<Result<Vec<_>>>()?
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

  let regions = patch.poly_regions();
  let voice_idents = regions
    .iter()
    .map(|region| voices_ident(&region.id))
    .collect::<Vec<_>>();
  let voice_counts = regions
    .iter()
    .map(|region| region.voices)
    .collect::<Vec<_>>();

  let instance_idents = instances
    .iter()
    .map(|instance| &instance.ident)
    .collect::<Vec<_>>();
  let instance_types = instances
    .iter()
    .map(|instance| &instance.field_type)
    .collect::<Vec<_>>();
  let instance_initializers = instances
    .iter()
    .map(|instance| &instance.initializer)
    .collect::<Vec<_>>();

  Ok(quote! {
        #[allow(non_snake_case)]
        pub struct Patch {
            #(#voice_idents: usize,)*
            #(#instance_idents: #instance_types,)*
            state: PatchState,
        }

        impl Patch {
            #[allow(unused_variables)]
            pub fn new(
                sample_rate: u32,
                max_buffer_size: usize,
                factory: &dyn IoNodeFactory,
            ) -> Self {
                #(let #voice_idents: usize = #voice_counts;)*

                let env = ModuleEnv {
                    sample_rate,
                    max_buffer_size,
                };

                Self {
                    #(#voice_idents,)*
                    #(#instance_idents: #instance_initializers,)*
                    state: PatchState::new(#(#voice_idents,)* max_buffer_size),
                }
            }
        }
    })
}

fn instance_code(node: &AnalyzedNode) -> Result<Option<InstanceCode>> {
  let ident = instance_ident(node);

  let types_and_initializer = match &node.behavior {
    Behavior::MidiIn => Some((
      quote!(Box<dyn MidiInputNode>),
      quote!(factory.create_midi_in()),
    )),
    Behavior::AudioOut => Some((
      quote!(Box<dyn AudioOutputNode>),
      quote!(factory.create_audio_out()),
    )),
    Behavior::VoiceAllocator { poly_id, .. } => {
      let voices = voices_ident(poly_id);
      Some((quote!(VoiceAllocator), quote!(VoiceAllocator::new(#voices))))
    }
    // value operators are plain functions without an instance
    Behavior::Unit { spec } if matches!(spec.call_style, CallStyle::ValueOperator { .. }) => None,
    Behavior::Unit { spec } => Some(unit_instance(spec, &node.run_domain)?),
    Behavior::Merge { .. } => None,
  };

  Ok(types_and_initializer.map(|(field_type, initializer)| InstanceCode {
    ident,
    field_type,
    initializer,
  }))
}

fn unit_instance(spec: &UnitSpec, domain: &Domain) -> Result<(TokenStream, TokenStream)> {
  let unit_type = parse_path(&spec.type_path)?;

  Ok(match domain {
    Domain::Mono => (quote!(#unit_type), quote!(#unit_type::new(&env))),
    Domain::Poly { poly_id } => {
      let voices = voices_ident(poly_id);
      (
        quote!(Vec<#unit_type>),
        quote!((0..#voices).map(|_| #unit_type::new(&env)).collect()),
      )
    }
  })
}

// ---------------------------------------------------------------------------
// Process: nodes in topological order, consecutive poly nodes share one voice loop
// ---------------------------------------------------------------------------

enum ProcessStep<'patch> {
  Single(&'patch AnalyzedNode),
  PolyRegion {
    poly_id: &'patch str,
    nodes: &'patch [AnalyzedNode],
  },
}

fn emit_process(patch: &AnalyzedPatch) -> Result<TokenStream> {
  let steps = group_into_steps(&patch.nodes)
    .iter()
    .map(emit_step)
    .collect::<Result<Vec<_>>>()?;

  Ok(quote! {
        impl KahunaPatch for Patch {
            fn process(&mut self, frames: usize) {
                #(#steps)*
            }
        }
    })
}

fn group_into_steps(nodes: &[AnalyzedNode]) -> Vec<ProcessStep<'_>> {
  nodes
    .chunk_by(|left, right| left.run_domain == right.run_domain)
    .flat_map(|run| match &run[0].run_domain {
      Domain::Mono => run.iter().map(ProcessStep::Single).collect::<Vec<_>>(),
      Domain::Poly { poly_id } => vec![ProcessStep::PolyRegion {
        poly_id,
        nodes: run,
      }],
    })
    .collect()
}

fn emit_step(step: &ProcessStep) -> Result<TokenStream> {
  match step {
    ProcessStep::Single(node) => emit_node_call(node),
    ProcessStep::PolyRegion { poly_id, nodes } => {
      let voices = voices_ident(poly_id);
      let calls = nodes
        .iter()
        .map(emit_node_call)
        .collect::<Result<Vec<_>>>()?;

      Ok(quote! {
                for voice in 0..self.#voices {
                    #(#calls)*
                }
            })
    }
  }
}

fn emit_node_call(node: &AnalyzedNode) -> Result<TokenStream> {
  let instance = instance_expression(node);

  match &node.behavior {
    Behavior::MidiIn => {
      let events = whole_buffer_mut(only_output(node)?);
      Ok(quote!(#instance.process(#events);))
    }
    Behavior::VoiceAllocator { .. } => {
      let events_in = read_expression(&only_connection(node)?.source, &node.run_domain);
      let voice_events_out = whole_buffer_mut(only_output(node)?);
      Ok(quote!(#instance.process(#events_in, #voice_events_out);))
    }
    Behavior::AudioOut => {
      let samples = read_expression(&only_connection(node)?.source, &node.run_domain);
      Ok(quote!(#instance.process(#samples, #samples);))
    }
    Behavior::Merge { mode } => emit_merge(node, *mode),
    Behavior::Unit { spec } => emit_unit_call(node, spec),
  }
}

fn emit_merge(node: &AnalyzedNode, mode: MergeMode) -> Result<TokenStream> {
  let source = &only_connection(node)?.source;
  let Domain::Poly { poly_id } = &source.domain else {
    bail!("merge node '{}' is not fed by a poly region", node.id);
  };

  let voices = voices_ident(poly_id);
  let source_field = buffer_field(source);
  let output_field = buffer_field(only_output(node)?);
  let contribution = match mode {
    MergeMode::Average => quote!(voice_samples[frame] / self.#voices as f32),
    MergeMode::Sum => quote!(voice_samples[frame]),
  };

  Ok(quote! {
        self.state.#output_field[..frames].fill(0.0);
        for voice in 0..self.#voices {
            let voice_samples = self.state.#source_field.voice(voice);
            for frame in 0..frames {
                self.state.#output_field[frame] += #contribution;
            }
        }
    })
}

fn emit_unit_call(node: &AnalyzedNode, spec: &UnitSpec) -> Result<TokenStream> {
  let arguments = match &spec.call_style {
    CallStyle::Converter => converter_arguments(node)?,
    CallStyle::Module => module_arguments(node, &spec.type_path)?,
    CallStyle::ValueOperator { fold } => return emit_value_operator(node, spec, *fold),
  };

  let instance = instance_expression(node);
  Ok(quote!(#instance.process(#arguments);))
}

/// Calls the operator once per sample for an audio signal output, once per block for a
/// parameter output; a parameter input contributes its value to every sample.
fn emit_value_operator(node: &AnalyzedNode, spec: &UnitSpec, fold: bool) -> Result<TokenStream> {
  let function = parse_path(&spec.type_path)?;
  let output = only_output(node)?;
  let per_sample = output.kind == ValueKind::Signal;

  let mut arguments = Vec::new();
  for input in &node.inputs {
    if input.connections.is_empty() {
      let default = input
        .port
        .default
        .ok_or_else(|| eyre!("input '{}.{}' is not connected and has no default", node.id, input.port.name))?;
      arguments.push(quote!(#default));
    }
    for connection in &input.connections {
      arguments.push(operator_argument(&connection.source, &node.run_domain, per_sample));
    }
  }

  let call = if fold {
    let mut values = arguments.into_iter();
    let first = values
      .next()
      .ok_or_else(|| eyre!("operator '{}' has no connections to combine", node.id))?;
    values.fold(first, |left, right| quote!(#function(#left, #right)))
  } else {
    quote!(#function(#(#arguments),*))
  };

  if per_sample {
    let samples = write_expression(output);
    Ok(quote! {
            {
                let out = #samples;
                for frame in 0..frames {
                    out[frame] = #call;
                }
            }
        })
  } else {
    let value = operator_output_place(output);
    Ok(quote!(#value = #call;))
  }
}

/// One input value: the current sample of an audio signal, or a parameter's value.
fn operator_argument(source: &BufferRef, consumer: &Domain, per_sample: bool) -> TokenStream {
  let field = buffer_field(source);
  let voice_scoped = matches!(
        (&source.domain, consumer),
        (Domain::Poly { .. }, Domain::Poly { .. })
    );

  match (source.kind, voice_scoped) {
    (ValueKind::Signal, false) if per_sample => quote!(self.state.#field[frame]),
    (ValueKind::Signal, true) if per_sample => quote!(self.state.#field.voice(voice)[frame]),
    (_, false) => quote!(self.state.#field),
    (_, true) => quote!(self.state.#field[voice]),
  }
}

/// Where a parameter output's value goes.
fn operator_output_place(output: &BufferRef) -> TokenStream {
  let field = buffer_field(output);
  match output.domain {
    Domain::Mono => quote!(self.state.#field),
    Domain::Poly { .. } => quote!(self.state.#field[voice]),
  }
}

fn converter_arguments(node: &AnalyzedNode) -> Result<TokenStream> {
  let input = read_expression(&only_connection(node)?.source, &node.run_domain);
  let output = write_expression(only_output(node)?);
  Ok(quote!(#input, #output))
}

fn module_arguments(node: &AnalyzedNode, type_path: &str) -> Result<TokenStream> {
  let unit_type = parse_path(type_path)?;
  let inputs = node
    .inputs
    .iter()
    .map(|input| module_input_argument(input, &node.run_domain));
  let outputs = node.outputs.iter().map(write_expression);

  Ok(quote! {
        &<#unit_type as kahuna_core::KahunaModule>::Inputs::new(#(#inputs),*),
        &mut <#unit_type as kahuna_core::KahunaModule>::Outputs::new(#(#outputs),*)
    })
}

fn module_input_argument(input: &ResolvedInput, consumer: &Domain) -> TokenStream {
  match (input.connections.first(), input.port.arity) {
    (Some(connection), Arity::Optional) => {
      let value = read_expression(&connection.source, consumer);
      quote!(Some(#value))
    }
    (Some(connection), _) => read_expression(&connection.source, consumer),
    (None, _) => quote!(None),
  }
}

// ---------------------------------------------------------------------------
// Buffer access expressions
// ---------------------------------------------------------------------------

/// Expression reading `source` from a consumer running in `consumer` domain.
/// Poly buffers are voice-scoped only when the consumer is itself polyphonic.
fn read_expression(source: &BufferRef, consumer: &Domain) -> TokenStream {
  let field = buffer_field(source);
  let voice_scoped = matches!(
        (&source.domain, consumer),
        (Domain::Poly { .. }, Domain::Poly { .. })
    );

  match (source.kind, voice_scoped) {
    (ValueKind::Events, false) => quote!(&self.state.#field),
    (ValueKind::Events, true) => quote!(&self.state.#field[voice]),
    (ValueKind::Signal, false) => quote!(&self.state.#field[..frames]),
    (ValueKind::Signal, true) => quote!(&self.state.#field.voice(voice)[..frames]),
    (ValueKind::Float | ValueKind::Bool, false) => quote!(self.state.#field),
    (ValueKind::Float | ValueKind::Bool, true) => quote!(self.state.#field[voice]),
    (ValueKind::Number, _) => unreachable!("Number is a port kind, never the kind of a buffer"),
  }
}

fn write_expression(output: &BufferRef) -> TokenStream {
  let field = buffer_field(output);
  let voice_scoped = matches!(output.domain, Domain::Poly { .. });

  match (output.kind, voice_scoped) {
    (ValueKind::Events, false) => quote!(&mut self.state.#field),
    (ValueKind::Events, true) => quote!(&mut self.state.#field[voice]),
    (ValueKind::Signal, false) => quote!(&mut self.state.#field[..frames]),
    (ValueKind::Signal, true) => quote!(&mut self.state.#field.voice_mut(voice)[..frames]),
    (ValueKind::Float | ValueKind::Bool, false) => quote!(&mut self.state.#field),
    (ValueKind::Float | ValueKind::Bool, true) => quote!(&mut self.state.#field[voice]),
    (ValueKind::Number, _) => unreachable!("Number is a port kind, never the kind of a buffer"),
  }
}

fn whole_buffer_mut(output: &BufferRef) -> TokenStream {
  let field = buffer_field(output);
  quote!(&mut self.state.#field)
}

// ---------------------------------------------------------------------------
// Naming and lookup helpers
// ---------------------------------------------------------------------------

fn voices_ident(poly_id: &str) -> Ident {
  format_ident!("{}_voices", poly_id)
}

fn buffer_field(buffer: &BufferRef) -> Ident {
  let region_prefix = match &buffer.domain {
    Domain::Poly { poly_id } if *poly_id != buffer.node_id => Some(poly_id.as_str()),
    _ => None,
  };
  let name = region_prefix
    .into_iter()
    .chain([buffer.node_id.as_str(), buffer.port_name.as_str()])
    .collect::<Vec<_>>()
    .join("__");

  format_ident!("{}", name)
}

fn instance_ident(node: &AnalyzedNode) -> Ident {
  let name = match (&node.behavior, &node.run_domain) {
    (Behavior::VoiceAllocator { poly_id, .. }, _) => format!("{poly_id}__voice_allocator"),
    (_, Domain::Poly { poly_id }) => format!("{poly_id}__{}", node.id),
    _ => node.id.clone(),
  };

  format_ident!("{}", name)
}

fn instance_expression(node: &AnalyzedNode) -> TokenStream {
  let ident = instance_ident(node);
  match node.run_domain {
    Domain::Mono => quote!(self.#ident),
    Domain::Poly { .. } => quote!(self.#ident[voice]),
  }
}

fn only_output(node: &AnalyzedNode) -> Result<&BufferRef> {
  node.outputs
    .first()
    .ok_or_else(|| eyre!("node '{}' has no output port", node.id))
}

fn only_connection(node: &AnalyzedNode) -> Result<&Connection> {
  node.inputs
    .first()
    .and_then(|input| input.connections.first())
    .ok_or_else(|| eyre!("node '{}' has no connected input", node.id))
}

fn parse_path(path: &str) -> Result<syn::Path> {
  syn::parse_str(path).wrap_err_with(|| format!("'{path}' is not a valid Rust path"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::compilation_steps::patch_code_generator::catalog::builtin_catalog;
  use crate::compilation_steps::patch_file_parser::patch_graph::PatchGraph;

  const SINE_PATCH: &str = r#"
{
  "nodes": [
    { "id": "midi_in_0", "kind": "interface", "interface": "midi_in" },
    { "id": "poly_0", "kind": "poly", "voices": 8 },
    { "id": "freq_conv_0", "kind": "converter", "unit": "kahuna_std::MidiEventToFrequencyParameter" },
    { "id": "sine_osc_0", "kind": "module", "unit": "self::SineOsc" },
    { "id": "gate_conv_0", "kind": "converter", "unit": "kahuna_std::MidiEventToGateParameter" },
    { "id": "adsr_0", "kind": "module", "unit": "self::AdsrEnvelope" },
    { "id": "mult_0", "kind": "operator", "unit": "kahuna-std/multiply" },
    { "id": "merge_0", "kind": "merge", "mode": "average" },
    { "id": "audio_out_0", "kind": "interface", "interface": "audio_out" }
  ],
  "edges": [
    { "from": "midi_in_0.midi_events", "to": "poly_0.midi_events" },
    { "from": "poly_0.out", "to": "freq_conv_0.midi_events" },
    { "from": "freq_conv_0.out", "to": "sine_osc_0.frequency" },
    { "from": "poly_0.out", "to": "gate_conv_0.midi_events" },
    { "from": "gate_conv_0.out", "to": "adsr_0.gate" },
    { "from": "sine_osc_0.out", "to": "mult_0.in[0]" },
    { "from": "adsr_0.envelope", "to": "mult_0.in[1]" },
    { "from": "mult_0.out", "to": "merge_0.in" },
    { "from": "merge_0.out", "to": "audio_out_0.in" }
  ]
}
"#;

  #[test]
  fn generates_voice_loop_and_merge() {
    let graph: PatchGraph = SINE_PATCH.parse().unwrap();
    let source = generate_patch_source(&graph, &builtin_catalog()).unwrap();

    assert!(contains_code(&source, "for voice in 0..self.poly_0_voices"));
    assert!(contains_code(
      &source,
      "out[frame] = kahuna_std::value_operators::arithmetic::multiply(\
         self.state.poly_0__sine_osc_0__out.voice(voice)[frame], \
         self.state.poly_0__adsr_0__envelope.voice(voice)[frame])"
    ), "{source}");
    assert!(contains_code(&source, "self.state.merge_0__out[..frames].fill(0.0)"));
    assert!(contains_code(&source, "self.audio_out_0.process"));
  }

  #[test]
  fn reaches_module_io_through_the_module_trait() {
    let graph: PatchGraph = SINE_PATCH.parse().unwrap();
    let source = generate_patch_source(&graph, &builtin_catalog()).unwrap();

    assert!(contains_code(&source, "<crate::modules::sine_osc::SineOsc as kahuna_core::KahunaModule>::Inputs::new("));
    assert!(contains_code(&source, "<crate::modules::sine_osc::SineOsc as kahuna_core::KahunaModule>::Outputs::new("));
  }

  /// prettyplease wraps long lines and then adds trailing commas, so compare without both.
  fn contains_code(source: &str, snippet: &str) -> bool {
    let strip = |text: &str| text.chars().filter(|c| !c.is_whitespace()).collect::<String>().replace(",)", ")");
    strip(source).contains(&strip(snippet))
  }

  #[test]
  fn rejects_unmerged_poly_output() {
    let json = SINE_PATCH.replace("\"to\": \"merge_0.in\"", "\"to\": \"audio_out_0.in\"");
    let graph: PatchGraph = json.parse().unwrap();

    let error = generate_patch_source(&graph, &builtin_catalog()).unwrap_err();
    assert!(format!("{error:#}").contains("merge"));
  }

  /// A patch where a value operator only sees parameters: the frequency, scaled.
  const PARAMETER_OPERATOR_PATCH: &str = r#"
{
  "nodes": [
    { "id": "midi_in_0", "kind": "interface", "interface": "midi_in" },
    { "id": "poly_0", "kind": "poly", "voices": 4 },
    { "id": "freq_conv_0", "kind": "converter", "unit": "kahuna_std::MidiEventToFrequencyParameter" },
    { "id": "octave_0", "kind": "operator", "unit": "kahuna-std/scale-offset" },
    { "id": "sine_osc_0", "kind": "module", "unit": "self::SineOsc" },
    { "id": "merge_0", "kind": "merge", "mode": "sum" },
    { "id": "audio_out_0", "kind": "interface", "interface": "audio_out" }
  ],
  "edges": [
    { "from": "midi_in_0.midi_events", "to": "poly_0.midi_events" },
    { "from": "poly_0.out", "to": "freq_conv_0.midi_events" },
    { "from": "freq_conv_0.out", "to": "octave_0.input" },
    { "from": "octave_0.out", "to": "sine_osc_0.frequency" },
    { "from": "sine_osc_0.out", "to": "merge_0.in" },
    { "from": "merge_0.out", "to": "audio_out_0.in" }
  ]
}
"#;

  #[test]
  fn operator_on_parameters_runs_once_per_block_with_defaults_for_unconnected_inputs() {
    let graph: PatchGraph = PARAMETER_OPERATOR_PATCH.parse().unwrap();
    let source = generate_patch_source(&graph, &builtin_catalog()).unwrap();

    assert!(contains_code(
      &source,
      "self.state.poly_0__octave_0__out[voice] = kahuna_std::value_operators::arithmetic::scale_offset(\
         self.state.poly_0__freq_conv_0__out[voice], 1f32, 0f32);"
    ), "{source}");
    assert!(contains_code(&source, "poly_0__octave_0__out: Vec<f32>"));
    assert!(!source.contains("octave_0: "), "operators have no instance");
  }

  #[test]
  fn rejects_too_few_connections_to_a_variadic_input() {
    let json = SINE_PATCH.replace("{ \"from\": \"adsr_0.envelope\", \"to\": \"mult_0.in[1]\" },", "");
    let graph: PatchGraph = json.parse().unwrap();

    let error = generate_patch_source(&graph, &builtin_catalog()).unwrap_err();
    assert!(format!("{error:#}").contains("needs at least 2 connections"), "{error:#}");
  }

  #[test]
  fn rejects_nodes_whose_kind_does_not_match_the_unit() {
    let json = SINE_PATCH.replace("\"kind\": \"operator\"", "\"kind\": \"module\"");
    let graph: PatchGraph = json.parse().unwrap();

    let error = generate_patch_source(&graph, &builtin_catalog()).unwrap_err();
    assert!(format!("{error:#}").contains("wrong kind for unit 'kahuna-std/multiply'"), "{error:#}");
  }

  #[test]
  fn rejects_cycles() {
    let json = r#"
{
  "nodes": [
    { "id": "a", "kind": "operator", "unit": "kahuna-std/multiply" },
    { "id": "b", "kind": "operator", "unit": "kahuna-std/multiply" }
  ],
  "edges": [
    { "from": "a.out", "to": "b.in[0]" },
    { "from": "b.out", "to": "a.in[0]" }
  ]
}
"#;
    let graph: PatchGraph = json.parse().unwrap();

    let error = generate_patch_source(&graph, &builtin_catalog()).unwrap_err();
    assert!(format!("{error:#}").contains("cycle"));
  }
}
