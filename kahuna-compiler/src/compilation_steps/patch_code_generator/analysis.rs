use eyre::{bail, ensure, eyre, Result, WrapErr};
use std::collections::{BTreeSet, HashMap, HashSet};
use crate::compilation_steps::patch_code_generator::catalog::{Arity, CallStyle, PortSpec, UnitCatalog, UnitSpec, ValueKind};
use crate::compilation_steps::patch_file_parser::patch_graph::{Edge, Node, PatchGraph};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Domain {
  Mono,
  Poly { poly_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMode {
  Average,
  Sum,
}

#[derive(Debug, Clone)]
pub enum Behavior {
  MidiIn,
  AudioOut,
  VoiceAllocator { poly_id: String, voices: usize },
  Unit { spec: UnitSpec },
  Merge { mode: MergeMode },
}

/// One output port of a node, which owns exactly one buffer in the generated state.
#[derive(Debug, Clone)]
pub struct BufferRef {
  pub node_id: String,
  pub port_name: String,
  pub kind: ValueKind,
  pub domain: Domain,
}

#[derive(Debug, Clone)]
pub struct Connection {
  pub index: Option<usize>,
  pub source: BufferRef,
}

#[derive(Debug, Clone)]
pub struct ResolvedInput {
  pub port: PortSpec,
  pub connections: Vec<Connection>,
}

#[derive(Debug, Clone)]
pub struct AnalyzedNode {
  pub id: String,
  pub behavior: Behavior,
  pub run_domain: Domain,
  pub inputs: Vec<ResolvedInput>,
  pub outputs: Vec<BufferRef>,
}

#[derive(Debug, Clone)]
pub struct PolyRegion {
  pub id: String,
  pub voices: usize,
}

/// Nodes in topological order, each with resolved ports and derived domain.
#[derive(Debug, Clone)]
pub struct AnalyzedPatch {
  pub nodes: Vec<AnalyzedNode>,
}

impl AnalyzedPatch {
  pub fn poly_regions(&self) -> Vec<PolyRegion> {
    self.nodes
      .iter()
      .filter_map(|node| match &node.behavior {
        Behavior::VoiceAllocator { poly_id, voices } => Some(PolyRegion {
          id: poly_id.clone(),
          voices: *voices,
        }),
        _ => None,
      })
      .collect()
  }
}

pub fn analyze(graph: &PatchGraph, catalog: &dyn UnitCatalog) -> Result<AnalyzedPatch> {
  validate_node_ids(graph)?;
  validate_edge_endpoints(graph)?;

  let nodes = topological_order(graph)?
    .into_iter()
    .try_fold(Vec::new(), |mut analyzed, node| {
      let analyzed_node = analyze_node(node, graph, catalog, &analyzed)
        .wrap_err_with(|| format!("while analyzing node '{}'", node.id()))?;
      analyzed.push(analyzed_node);
      Ok::<_, eyre::Report>(analyzed)
    })?;

  Ok(AnalyzedPatch { nodes })
}

fn analyze_node(
  node: &Node,
  graph: &PatchGraph,
  catalog: &dyn UnitCatalog,
  analyzed: &[AnalyzedNode],
) -> Result<AnalyzedNode> {
  let wired = graph
    .edges
    .iter()
    .filter(|edge| edge.to.node == node.id())
    .map(|edge| resolve_source(edge, analyzed).map(|source| (edge, source)))
    .collect::<Result<Vec<_>>>()?;
  let sources = wired
    .iter()
    .map(|(_, source)| source.clone())
    .collect::<Vec<_>>();

  let (run_domain, output_domain) = derive_domains(node, &sources)?;
  let behavior = behavior_of(node, catalog)?;
  let (input_ports, output_ports) = port_specs(&behavior);

  let inputs = resolve_inputs(node.id(), &input_ports, &wired)?;
  let outputs = output_ports
    .iter()
    .map(|port| BufferRef {
      node_id: node.id().to_string(),
      port_name: port.name.clone(),
      kind: resolved_output_kind(port.kind, &inputs),
      domain: output_domain.clone(),
    })
    .collect();

  Ok(AnalyzedNode {
    id: node.id().to_string(),
    behavior,
    run_domain,
    inputs,
    outputs,
  })
}

fn behavior_of(node: &Node, catalog: &dyn UnitCatalog) -> Result<Behavior> {
  match node {
    Node::Interface { interface, .. } => match interface.as_str() {
      "midi_in" => Ok(Behavior::MidiIn),
      "audio_out" => Ok(Behavior::AudioOut),
      other => bail!("interface '{other}' is not supported yet"),
    },
    Node::Poly { id, voices } => {
      ensure!(*voices > 0, "poly node '{id}' needs at least one voice");
      Ok(Behavior::VoiceAllocator {
        poly_id: id.clone(),
        voices: *voices,
      })
    }
    Node::Module { id, unit } | Node::Converter { id, unit } | Node::Operator { id, unit } => {
      let spec = catalog
        .find(unit)
        .cloned()
        .ok_or_else(|| eyre!("node '{id}' uses unknown unit '{unit}'"))?;
      let matches_node_kind = matches!(
        (node, &spec.call_style),
        (Node::Module { .. }, CallStyle::Module)
          | (Node::Converter { .. }, CallStyle::Converter)
          | (Node::Operator { .. }, CallStyle::ValueOperator { .. })
      );
      ensure!(matches_node_kind, "node '{id}' has the wrong kind for unit '{unit}', which is a {}", unit_kind_name(&spec.call_style));
      Ok(Behavior::Unit { spec })
    }
    Node::Merge { mode, .. } => match mode.as_str() {
      "average" => Ok(Behavior::Merge {
        mode: MergeMode::Average,
      }),
      "sum" => Ok(Behavior::Merge {
        mode: MergeMode::Sum,
      }),
      other => bail!("merge mode '{other}' is not supported (expected 'average' or 'sum')"),
    },
  }
}

fn unit_kind_name(call_style: &CallStyle) -> &'static str {
  match call_style {
    CallStyle::Module => "module",
    CallStyle::Converter => "converter",
    CallStyle::ValueOperator { .. } => "operator",
  }
}

fn port_specs(behavior: &Behavior) -> (Vec<PortSpec>, Vec<PortSpec>) {
  match behavior {
    Behavior::MidiIn => (
      vec![],
      vec![PortSpec::required("midi_events", ValueKind::Events)],
    ),
    Behavior::AudioOut => (vec![PortSpec::required("in", ValueKind::Signal)], vec![]),
    Behavior::VoiceAllocator { .. } => (
      vec![PortSpec::required("midi_events", ValueKind::Events)],
      vec![PortSpec::required("out", ValueKind::Events)],
    ),
    Behavior::Unit { spec } => (spec.inputs.clone(), spec.outputs.clone()),
    Behavior::Merge { .. } => (
      vec![PortSpec::required("in", ValueKind::Signal)],
      vec![PortSpec::required("out", ValueKind::Signal)],
    ),
  }
}

/// Returns `(run_domain, output_domain)`.
///
/// A poly region starts at a `Poly` node and extends forward through every node
/// that consumes voice data, until a `Merge` node collapses it back to mono.
fn derive_domains(node: &Node, sources: &[BufferRef]) -> Result<(Domain, Domain)> {
  let poly_ids = distinct_poly_ids(sources);

  match node {
    Node::Poly { id, .. } if poly_ids.is_empty() => Ok((
      Domain::Mono,
      Domain::Poly {
        poly_id: id.clone(),
      },
    )),
    Node::Poly { id, .. } => bail!("poly node '{id}' cannot be nested in a poly region"),
    Node::Merge { id, .. } => {
      let all_sources_are_poly = sources
        .iter()
        .all(|source| matches!(source.domain, Domain::Poly { .. }));
      ensure!(
                poly_ids.len() == 1 && all_sources_are_poly,
                "merge node '{id}' needs input from exactly one poly region"
            );
      Ok((Domain::Mono, Domain::Mono))
    }
    Node::Interface { id, .. } => {
      ensure!(
                poly_ids.is_empty(),
                "interface '{id}' is fed by a poly region, route it through a merge node"
            );
      Ok((Domain::Mono, Domain::Mono))
    }
    Node::Module { id, .. } | Node::Converter { id, .. } | Node::Operator { id, .. } => match poly_ids.as_slice() {
      [] => Ok((Domain::Mono, Domain::Mono)),
      [poly_id] => {
        let domain = Domain::Poly {
          poly_id: poly_id.to_string(),
        };
        Ok((domain.clone(), domain))
      }
      _ => bail!("node '{id}' mixes voices of several poly regions"),
    },
  }
}

fn distinct_poly_ids(sources: &[BufferRef]) -> Vec<&str> {
  sources
    .iter()
    .filter_map(|source| match &source.domain {
      Domain::Poly { poly_id } => Some(poly_id.as_str()),
      Domain::Mono => None,
    })
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect()
}

fn resolve_inputs(
  node_id: &str,
  ports: &[PortSpec],
  wired: &[(&Edge, BufferRef)],
) -> Result<Vec<ResolvedInput>> {
  if let Some((edge, _)) = wired
    .iter()
    .find(|(edge, _)| !ports.iter().any(|port| port.name == edge.to.port))
  {
    bail!("node '{node_id}' has no input port '{}'", edge.to.port);
  }

  ports
    .iter()
    .map(|port| resolve_input(node_id, port, wired))
    .collect()
}

fn resolve_input(
  node_id: &str,
  port: &PortSpec,
  wired: &[(&Edge, BufferRef)],
) -> Result<ResolvedInput> {
  let mut connections = wired
    .iter()
    .filter(|(edge, _)| edge.to.port == port.name)
    .map(|(edge, source)| Connection {
      index: edge.to.index,
      source: source.clone(),
    })
    .collect::<Vec<_>>();
  connections.sort_by_key(|connection| connection.index);

  validate_connections(node_id, port, &connections)?;

  Ok(ResolvedInput {
    port: port.clone(),
    connections,
  })
}

/// A value operator's `Number` output carries an audio signal as soon as one input does,
/// and a parameter otherwise.
fn resolved_output_kind(kind: ValueKind, inputs: &[ResolvedInput]) -> ValueKind {
  if kind != ValueKind::Number {
    return kind;
  }
  let any_signal = inputs
    .iter()
    .flat_map(|input| &input.connections)
    .any(|connection| connection.source.kind == ValueKind::Signal);
  if any_signal { ValueKind::Signal } else { ValueKind::Float }
}

fn validate_connections(node_id: &str, port: &PortSpec, connections: &[Connection]) -> Result<()> {
  if let Some(mismatch) = connections
    .iter()
    .find(|connection| !port.kind.accepts(connection.source.kind))
  {
    bail!(
            "type mismatch on '{node_id}.{}': expected {:?}, but '{}.{}' provides {:?}",
            port.name,
            port.kind,
            mismatch.source.node_id,
            mismatch.source.port_name,
            mismatch.source.kind
        );
  }

  match port.arity {
    Arity::Required => {
      ensure!(
                connections.len() == 1,
                "required input '{node_id}.{}' needs exactly one connection, found {}",
                port.name,
                connections.len()
            );
      validate_unindexed(node_id, port, connections)
    }
    Arity::Optional => {
      ensure!(
                connections.len() <= 1,
                "input '{node_id}.{}' accepts at most one connection, found {}",
                port.name,
                connections.len()
            );
      validate_unindexed(node_id, port, connections)
    }
    Arity::Variadic => {
      ensure!(
                connections.len() >= port.min_connections,
                "input '{node_id}.{}' needs at least {} connections, found {}",
                port.name,
                port.min_connections,
                connections.len()
            );
      validate_contiguous_indices(node_id, port, connections)
    }
  }
}

fn validate_unindexed(node_id: &str, port: &PortSpec, connections: &[Connection]) -> Result<()> {
  ensure!(
        connections
            .iter()
            .all(|connection| connection.index.is_none()),
        "input '{node_id}.{}' is not indexed",
        port.name
    );
  Ok(())
}

fn validate_contiguous_indices(
  node_id: &str,
  port: &PortSpec,
  connections: &[Connection],
) -> Result<()> {
  let indices = connections
    .iter()
    .map(|connection| connection.index)
    .collect::<Option<Vec<_>>>()
    .ok_or_else(|| eyre!("every connection to '{node_id}.{}' needs an index", port.name))?;

  ensure!(
        !indices.is_empty() && indices.iter().copied().eq(0..indices.len()),
        "indices of '{node_id}.{}' must be contiguous and start at 0, found {indices:?}",
        port.name
    );
  Ok(())
}

fn resolve_source(edge: &Edge, analyzed: &[AnalyzedNode]) -> Result<BufferRef> {
  ensure!(
        edge.from.index.is_none(),
        "output port '{}' must not be indexed",
        edge.from
    );

  analyzed
    .iter()
    .find(|node| node.id == edge.from.node)
    .and_then(|node| {
      node.outputs
        .iter()
        .find(|output| output.port_name == edge.from.port)
    })
    .cloned()
    .ok_or_else(|| eyre!("edge source '{}' does not exist", edge.from))
}

/// Node ids are written like `adsr_0` or `to-freq_0`: a unit's short name and a number. Hyphens are
/// allowed because unit ids have them, and generated code spells them as underscores.
pub fn rust_name(id: &str) -> String {
  id.replace('-', "_")
}

fn is_valid_node_id(id: &str) -> bool {
  id.chars().next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
    && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn validate_node_ids(graph: &PatchGraph) -> Result<()> {
  let mut seen = HashSet::new();
  if let Some(duplicate) = graph.nodes.iter().map(Node::id).find(|id| !seen.insert(*id)) {
    bail!("duplicate node id '{duplicate}'");
  }

  let mut rust_names: HashMap<String, &str> = HashMap::new();
  for id in graph.nodes.iter().map(Node::id) {
    ensure!(
      is_valid_node_id(id),
      "node id '{id}' is not valid: start with a letter or '_', then use letters, digits, '_' and '-'"
    );
    if let Some(other) = rust_names.insert(rust_name(id), id) {
      bail!("node ids '{other}' and '{id}' would both become '{}' in the generated code", rust_name(id));
    }
  }
  Ok(())
}

fn validate_edge_endpoints(graph: &PatchGraph) -> Result<()> {
  let known_ids = graph.nodes.iter().map(Node::id).collect::<HashSet<_>>();
  let unknown = graph
    .edges
    .iter()
    .flat_map(|edge| [&edge.from, &edge.to])
    .find(|port| !known_ids.contains(port.node.as_str()));

  match unknown {
    Some(port) => bail!("edge references unknown node '{}'", port.node),
    None => Ok(()),
  }
}

/// Kahn-style ordering that keeps the declaration order among independent nodes.
fn topological_order(graph: &PatchGraph) -> Result<Vec<&Node>> {
  let mut pending = graph.nodes.iter().collect::<Vec<_>>();
  let mut ordered = Vec::with_capacity(pending.len());

  while !pending.is_empty() {
    let ready_position = pending
      .iter()
      .position(|candidate| all_sources_ordered(candidate, graph, &ordered))
      .ok_or_else(|| {
        let stuck = pending.iter().map(|node| node.id()).collect::<Vec<_>>();
        eyre!("patch graph contains a cycle among: {}", stuck.join(", "))
      })?;
    ordered.push(pending.remove(ready_position));
  }

  Ok(ordered)
}

fn all_sources_ordered(candidate: &Node, graph: &PatchGraph, ordered: &[&Node]) -> bool {
  graph
    .edges
    .iter()
    .filter(|edge| edge.to.node == candidate.id())
    .all(|edge| ordered.iter().any(|done| done.id() == edge.from.node))
}
