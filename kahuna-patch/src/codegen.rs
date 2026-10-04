use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Error;
use syn::spanned::Spanned;
use crate::ast::{ConnectionDecl, PatchDef, PortPath};

pub fn generate(patch: &PatchDef) -> syn::Result<TokenStream> {
  check_targets_connected_once(patch)?;

  let state_fields = state_fields(patch);
  let state_init = state_init(patch);

  let io_fields = io_fields(patch);
  let module_fields = module_fields(patch);

  let ctor_params = ctor_params(patch);
  let io_init = io_init(patch)?;
  let module_init = module_init(patch);

  let process_body = process_body(patch)?;

  Ok(quote! {
        #[allow(non_snake_case)]
        pub struct PatchState {
            #(#state_fields),*
        }

        impl PatchState {
            pub fn new(max_buffer_size: usize) -> Self {
                Self {
                    #(#state_init),*
                }
            }
        }

        pub struct Patch {
            max_buffer_size: usize,
            #(#io_fields),*,
            #(#module_fields),*,
            state: PatchState,
        }

        impl Patch {
            pub fn new(
                sample_rate: u32,
                max_buffer_size: usize,
                #(#ctor_params),*
            ) -> Self {
                let env = ::kahuna_core::ModuleEnv { sample_rate, max_buffer_size };
                Self {
                    max_buffer_size,
                    #(#io_init),*,
                    #(#module_init),*,
                    state: PatchState::new(max_buffer_size),
                }
            }
        }

        impl ::kahuna_core::patch::KahunaPatch for Patch {
            fn process(&mut self, frames: usize) {
                #process_body
            }
        }
    })
}

fn buffer_field(port: &PortPath) -> syn::Ident {
  format_ident!("{}__{}", port.node, port.port)
}

fn unique_sources(patch: &PatchDef) -> Vec<&PortPath> {
  let mut seen = std::collections::HashSet::new();
  let mut result = Vec::new();
  for conn in &patch.connections {
    let key = (conn.source.node.to_string(), conn.source.port.to_string());
    if seen.insert(key) {
      result.push(&conn.source);
    }
  }
  result
}

fn state_fields(patch: &PatchDef) -> Vec<TokenStream> {
  unique_sources(patch).iter().map(|source| {
    let field = buffer_field(source);
    quote! { #field: Vec<f32> }
  }).collect()
}

fn state_init(patch: &PatchDef) -> Vec<TokenStream> {
  unique_sources(patch).iter().map(|source| {
    let field = buffer_field(source);
    quote! { #field: vec![0.0; max_buffer_size] }
  }).collect()
}

fn io_fields(patch: &PatchDef) -> Vec<TokenStream> {
  patch.io_nodes.iter().map(|io| {
    let name = &io.name;
    let trait_ty = &io.ty; // now a trait name, e.g. AudioOutputNode
    quote! { #name: Box<dyn #trait_ty> }
  }).collect()
}

fn module_fields(patch: &PatchDef) -> Vec<TokenStream> {
  patch.modules.iter().map(|m| {
    let name = &m.name;
    let ty = &m.ty;
    quote! { #name: #ty }
  }).collect()
}

// Single factory parameter regardless of how many io nodes this patch declares.
fn ctor_params(_patch: &PatchDef) -> Vec<TokenStream> {
  vec![quote! { factory: &dyn ::kahuna_core::io_nodes::IoNodeFactory }]
}

fn io_init(patch: &PatchDef) -> syn::Result<Vec<TokenStream>> {
  patch.io_nodes.iter().map(|io| {
    let name = &io.name;
    let method = match type_name(&io.ty).as_deref() {
      Some("AudioOutputNode") => quote! { create_audio_out },
      Some("AudioInputNode") => quote! { create_audio_in },
      Some("MidiInputNode") => quote! { create_midi_in },
      Some("MidiOutputNode") => quote! { create_midi_out },
      other => return Err(Error::new(
        io.ty.span(),
        format!(
          "`{}` is not a known IO node trait — expected one of \
                     AudioOutputNode, AudioInputNode, MidiInputNode, MidiOutputNode",
          other.unwrap_or_default(),
        ),
      )),
    };
    Ok(quote! { #name: factory.#method() })
  }).collect()
}

fn module_init(patch: &PatchDef) -> Vec<TokenStream> {
  patch.modules.iter().map(|m| {
    let name = &m.name;
    let ty = &m.ty;
    quote! { #name: <#ty as ::kahuna_core::KahunaModule>::new(&env) }
  }).collect()
}

enum NodeKind {
  AudioOutputNode,
  AudioInputNode,
  MidiInputNode,
  MidiOutputNode,
  Module,
}

fn type_name(ty: &syn::Type) -> Option<String> {
  match ty {
    syn::Type::Path(type_path) => {
      type_path.path.segments.last().map(|s| s.ident.to_string())
    }
    _ => None,
  }
}

// Only four concrete IO node types exist, so the mapping is a direct
// name lookup rather than a resolved trait bound.
fn target_node_kind(patch: &PatchDef, node: &syn::Ident) -> NodeKind {
  let Some(io) = patch.io_nodes.iter().find(|n| &n.name == node) else {
    return NodeKind::Module;
  };
  match type_name(&io.ty).as_deref() {
    Some("AudioOutputNode") => NodeKind::AudioOutputNode,
    Some("AudioInputNode") => NodeKind::AudioInputNode,
    Some("MidiInputNode") => NodeKind::MidiInputNode,
    Some("MidiOutputNode") => NodeKind::MidiOutputNode,
    _ => NodeKind::Module,
  }
}
use std::collections::{HashMap, HashSet};

// Emits one process() call per node (module or IO node), in dependency
// order: a node's call is emitted only after every node whose output
// feeds one of its input ports has already been emitted.
fn process_body(patch: &PatchDef) -> syn::Result<TokenStream> {
  let order = topological_order(patch)?;

  let mut by_target: std::collections::BTreeMap<String, Vec<&ConnectionDecl>> =
    Default::default();
  for conn in &patch.connections {
    by_target
      .entry(conn.target.node.to_string())
      .or_default()
      .push(conn);
  }

  let mut calls = Vec::new();

  for node_name in &order {
    let is_module = patch.modules.iter().any(|m| m.name == node_name.as_str());
    let conns = by_target.get(node_name.as_str());

    let call = match (is_module, conns) {
      // Module with an incoming connection: read the source, write its own output.
      (true, Some(conns)) => {
        let target_node = &conns[0].target.node;
        let source_field = buffer_field(&conns[0].source);
        let output_field = format_ident!("{}__out", target_node);
        quote! {
                    self.#target_node.process(
                        &self.state.#source_field[..frames],
                        &mut self.state.#output_field[..frames],
                    );
                }
      }
      // Module with no incoming connection: still runs every block,
      // fed an empty input, writing to its own output.
      (true, None) => {
        let target_node = format_ident!("{}", node_name);
        let output_field = format_ident!("{}__out", node_name);
        quote! {
                    self.#target_node.process(
                        &[],
                        &mut self.state.#output_field[..frames],
                    );
                }
      }
      // IO node with an incoming connection: AudioOutputNode / MidiOutputNode.
      (false, Some(conns)) => {
        let target_node = &conns[0].target.node;

        let port_arg = |port_name: &str| -> Option<TokenStream> {
          conns.iter().find(|c| c.target.port == port_name).map(|conn| {
            let source_field = buffer_field(&conn.source);
            quote! { &self.state.#source_field[..frames] }
          })
        };

        match target_node_kind(patch, target_node) {
          NodeKind::AudioOutputNode => {
            let left = port_arg("left").ok_or_else(|| {
              Error::new(conns[0].target.span, "audio output node requires `left` to be connected")
            })?;
            let right = port_arg("right").ok_or_else(|| {
              Error::new(conns[0].target.span, "audio output node requires `right` to be connected")
            })?;
            quote! { self.#target_node.process(#left, #right); }
          }
          NodeKind::MidiOutputNode => {
            let events = port_arg("events").ok_or_else(|| {
              Error::new(conns[0].target.span, "midi output node requires `events` to be connected")
            })?;
            quote! { self.#target_node.process(#events); }
          }
          NodeKind::MidiInputNode => {
            return Err(Error::new(
              conns[0].target.span,
              "midi input nodes are signal sources and cannot be a connection target",
            ));
          }
          NodeKind::AudioInputNode => {
            return Err(Error::new(
              conns[0].target.span,
              "audio input nodes are signal sources and cannot be a connection target",
            ));
          }
          NodeKind::Module => unreachable!("is_module already handled above"),
        }
      }
      // IO node with no incoming connection: MidiInputNode / AudioInputNode.
      // These produce output without reading state.
      (false, None) => {
        let target_node = format_ident!("{}", node_name);
        match target_node_kind(patch, &syn::parse_str(node_name).unwrap()) {
          NodeKind::MidiInputNode => {
            let output_field = format_ident!("{}__events", node_name);
            quote! { self.#target_node.process(&mut self.state.#output_field); }
          }
          _ => {
            return Err(Error::new(
              proc_macro2::Span::call_site(),
              format!("io node `{}` has no incoming connection and is not a recognized input source", node_name),
            ));
          }
        }
      }
    };

    calls.push(call);
  }

  Ok(quote! {
        #(#calls)*
    })
}

// Returns node names (module or io) in an order where every node
// appears after all nodes whose output it consumes.
fn topological_order(patch: &PatchDef) -> syn::Result<Vec<String>> {
  let all_nodes: Vec<String> = patch.io_nodes.iter().map(|n| n.name.to_string())
    .chain(patch.modules.iter().map(|m| m.name.to_string()))
    .collect();

  // edges[a] = nodes that depend on a's output (a must run before them)
  let mut edges: HashMap<String, Vec<String>> = HashMap::new();
  let mut in_degree: HashMap<String, usize> = all_nodes.iter().map(|n| (n.clone(), 0)).collect();

  for conn in &patch.connections {
    let from = conn.source.node.to_string();
    let to = conn.target.node.to_string();
    edges.entry(from).or_default().push(to.clone());
    *in_degree.entry(to).or_insert(0) += 1;
  }

  let mut queue: std::collections::VecDeque<String> = all_nodes.iter()
    .filter(|n| in_degree.get(*n).copied().unwrap_or(0) == 0)
    .cloned()
    .collect();

  let mut order = Vec::new();
  let mut visited = HashSet::new();

  while let Some(node) = queue.pop_front() {
    if !visited.insert(node.clone()) {
      continue;
    }
    order.push(node.clone());
    if let Some(dependents) = edges.get(&node) {
      for dep in dependents {
        let degree = in_degree.get_mut(dep).unwrap();
        *degree -= 1;
        if *degree == 0 {
          queue.push_back(dep.clone());
        }
      }
    }
  }

  if order.len() != all_nodes.len() {
    return Err(Error::new(
      proc_macro2::Span::call_site(),
      "patch graph contains a cycle — connections must not form a loop",
    ));
  }

  Ok(order)
}

fn check_targets_connected_once(patch: &PatchDef) -> syn::Result<()> {
  let mut seen = std::collections::HashSet::new();
  for conn in &patch.connections {
    let key = (conn.target.node.to_string(), conn.target.port.to_string());
    if !seen.insert(key) {
      return Err(Error::new(
        conn.target.span,
        format!(
          "port `{}.{}` is already connected — without `merge`, \
                     a target port can only have one incoming connection",
          conn.target.node, conn.target.port,
        ),
      ));
    }
  }
  Ok(())
}
