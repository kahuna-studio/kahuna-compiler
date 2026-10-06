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
            KahunaModule, ModuleEnv,
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
  let instance = instance_expression(node);
  let arguments = match &spec.call_style {
    CallStyle::Converter => converter_arguments(node)?,
    CallStyle::Module { io_module_path } => module_arguments(node, io_module_path)?,
    CallStyle::VariadicSlice => slice_arguments(node)?,
  };

  Ok(quote!(#instance.process(#arguments);))
}

fn converter_arguments(node: &AnalyzedNode) -> Result<TokenStream> {
  let input = read_expression(&only_connection(node)?.source, &node.run_domain);
  let output = write_expression(only_output(node)?);
  Ok(quote!(#input, #output))
}

fn module_arguments(node: &AnalyzedNode, io_module_path: &str) -> Result<TokenStream> {
  let io_module = parse_path(io_module_path)?;
  let inputs = node
    .inputs
    .iter()
    .map(|input| module_input_argument(input, &node.run_domain));
  let outputs = node.outputs.iter().map(write_expression);

  Ok(quote! {
        &#io_module::Inputs::new(#(#inputs),*),
        &mut #io_module::Outputs::new(#(#outputs),*)
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

fn slice_arguments(node: &AnalyzedNode) -> Result<TokenStream> {
  let inputs = node
    .inputs
    .iter()
    .flat_map(|input| &input.connections)
    .map(|connection| read_expression(&connection.source, &node.run_domain));
  let output = write_expression(only_output(node)?);

  Ok(quote!(&[#(#inputs),*], #output))
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
    { "id": "mult_0", "kind": "module", "unit": "self::Multiply" },
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

    assert!(source.contains("for voice in 0..self.poly_0_voices"));
    assert!(source.contains("self.poly_0__mult_0[voice].process"));
    assert!(source.contains("self.state.merge_0__out[..frames].fill(0.0)"));
    assert!(source.contains("self.audio_out_0.process"));
  }

  #[test]
  fn rejects_unmerged_poly_output() {
    let json = SINE_PATCH.replace("\"to\": \"merge_0.in\"", "\"to\": \"audio_out_0.in\"");
    let graph: PatchGraph = json.parse().unwrap();

    let error = generate_patch_source(&graph, &builtin_catalog()).unwrap_err();
    assert!(format!("{error:#}").contains("merge"));
  }

  #[test]
  fn rejects_cycles() {
    let json = r#"
{
  "nodes": [
    { "id": "a", "kind": "module", "unit": "self::Multiply" },
    { "id": "b", "kind": "module", "unit": "self::Multiply" }
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
