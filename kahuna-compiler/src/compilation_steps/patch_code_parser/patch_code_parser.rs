// ============================================================
// compiler/patch_code_parser
//
// Parses the Kahuna patch text format into a PatchGraph.
//
// At this moment superseded by the patch_file_parser - so patches are defined
// in JSON for now.
//
// Connection syntax:
//   source_node.port -> target_port[.sub_index].target_node
//   source_node.port -> converter -> target_port[.sub_index].target_node
//
// Node syntax:
//   (kind:index)   interface, e.g. (midi_in:0), (audio_out:0)
//   [Kind:index]   module,    e.g. [Sine_Osc:0], [Multiply:0]
//   {poly}         voice allocator
//   {merge}        polyphony merger
//
// Converters are inline between two arrows. They become Converter
// nodes with auto-assigned indices (per kind, in order of appearance).
// ============================================================

use std::collections::HashMap;
use crate::compilation_steps::patch_code_parser::graph::{Edge, EdgeFromPort, EdgeToPort, InterfaceType, NodeId, NodeKind, PatchGraph};

pub fn parse_patch_code(input: &str) -> Result<PatchGraph, String> {
    let name = find_patch_name(input)?;
    let mut graph = PatchGraph::new(name);
    let mut converter_indices: HashMap<String, usize> = HashMap::new();
    let mut in_patch_block = false;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "patch:" {
            in_patch_block = true;
            continue;
        }
        if in_patch_block && trimmed.contains(" -> ") {
            parse_connection(trimmed, &mut graph, &mut converter_indices)?;
        }
    }

    Ok(graph)
}

// ---- Connection parsing ----

fn parse_connection(
    line: &str,
    graph: &mut PatchGraph,
    converter_indices: &mut HashMap<String, usize>,
) -> Result<(), String> {
    let parts: Vec<&str> = line.split(" -> ").collect();

    match parts.as_slice() {
        [src, tgt] => {
            let (from, from_port) = parse_source(src.trim(), graph)?;
            let (to, to_port, to_sub) = parse_target(tgt.trim(), graph)?;
            graph.add_edge(Edge {
                from: EdgeFromPort {
                    node: from,
                    port: from_port,
                },
                to: EdgeToPort {
                    node: to,
                    port: to_port,
                    index: to_sub,
                }
            });
        }

        [src, conv_name, tgt] => {

            let (from, from_port) = parse_source(src.trim(), graph)?;

            let converter_type = conv_name.trim().to_string();
            let index = {
                let next = converter_indices.entry(converter_type.clone()).or_insert(0);
                let idx = *next;
                *next += 1;
                idx
            };
            let conv_id = graph.get_or_add_node(NodeKind::Converter { converter_type, index });

            let (to, to_port, to_sub) = parse_target(tgt.trim(), graph)?;

            graph.add_edge(Edge {
                from: EdgeFromPort {
                    node: from,
                    port: from_port,
                },
                to: EdgeToPort {
                    node: conv_id,
                    port: "in".into(),
                    index: None,
                }
            });
            graph.add_edge(Edge {
                from: EdgeFromPort {
                    node: conv_id,
                    port: "out".into(),
                },
                to: EdgeToPort {
                    node: to,
                    port: to_port,
                    index: to_sub,
                }
            });
        }

        _ => return Err(format!("expected 2 or 3 ' -> ' parts in: '{}'", line)),
    }

    Ok(())
}

// ---- Endpoint parsing ----

/// Parse a source endpoint: `node.port`
fn parse_source(s: &str, graph: &mut PatchGraph) -> Result<(NodeId, String), String> {
    let (kind, rest) = parse_node(s)?;
    let port = rest
        .strip_prefix('.')
        .ok_or_else(|| format!("expected '.' after node in source '{}'", s))?
        .to_string();
    let id = graph.get_or_add_node(kind);
    Ok((id, port))
}

/// Parse a target endpoint: `port.node` or `port.sub_index.node`
fn parse_target(s: &str, graph: &mut PatchGraph) -> Result<(NodeId, String, Option<usize>), String> {
    let node_start = s
        .find(|c| c == '(' || c == '[' || c == '{')
        .ok_or_else(|| format!("no node found in target '{}'", s))?;

    let prefix = s[..node_start].trim_end_matches('.');
    let (kind, _) = parse_node(&s[node_start..])?;
    let id = graph.get_or_add_node(kind);

    // prefix is "port" or "port.sub_index"
    if let Some(dot) = prefix.rfind('.') {
        let port = prefix[..dot].to_string();
        let sub_index = prefix[dot + 1..]
            .parse::<usize>()
            .map_err(|_| format!("expected numeric sub-index in '{}'", prefix))?;
        Ok((id, port, Some(sub_index)))
    } else {
        Ok((id, prefix.to_string(), None))
    }
}

// ---- Node token parsing ----

/// Parse the node token at the start of `s` and return `(NodeKind, remaining)`.
fn parse_node(s: &str) -> Result<(NodeKind, &str), String> {
    if s.starts_with('(') {
        let end = s
            .find(')')
            .ok_or_else(|| format!("unclosed '(' in '{}'", s))?;
        let (interface_type_string, index) = parse_kind_index(&s[1..end])?;
        let interface_type = match interface_type_string.as_str() {
          "midi_in" => InterfaceType::MidiIn,
          "midi_out" => InterfaceType::MidiOut,
          "audio_in" => InterfaceType::AudioIn,
          "audio_out" => InterfaceType::AudioOut,
          _ => return Err(format!("unknown interface type '{}' in '{}'", interface_type_string, s)),
        };
        Ok((NodeKind::Interface { interface_type, index }, &s[end + 1..]))
    } else if s.starts_with('[') {
        let end = s
            .find(']')
            .ok_or_else(|| format!("unclosed '[' in '{}'", s))?;
        let (module_type, index) = parse_kind_index(&s[1..end])?;
        Ok((NodeKind::Module { module_type, index }, &s[end + 1..]))
    } else if s.starts_with('{') {
        let end = s
            .find('}')
            .ok_or_else(|| format!("unclosed '{{' in '{}'", s))?;
        let inner = &s[1..end];
        let ty = match inner {
            "poly" => NodeKind::Poly,
            "merge" => NodeKind::Merge,
            _ => return Err(format!("unknown brace node '{}' in '{}'", inner, s)),
        };
        Ok((ty, &s[end + 1..]))
    } else {
        Err(format!(
            "expected node starting with '(', '[', or '{{' at '{}'",
            s
        ))
    }
}

/// Parse `"Kind:index"` into `(String, usize)`.
fn parse_kind_index(s: &str) -> Result<(String, usize), String> {
    let (kind, idx_str) = s
        .split_once(':')
        .ok_or_else(|| format!("expected 'Kind:index', got '{}'", s))?;
    let index = idx_str
        .parse()
        .map_err(|_| format!("expected numeric index in '{}'", s))?;
    Ok((kind.to_string(), index))
}

// ---- Header parsing ----

fn find_patch_name(input: &str) -> Result<String, String> {
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("patch ") && trimmed.ends_with(':') {
            return Ok(trimmed["patch ".len()..trimmed.len() - 1].trim().to_string());
        }
    }
    Err("no 'patch Name:' header found".into())
}
