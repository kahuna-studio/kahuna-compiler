use eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Patch graph schema used by the compiler frontend.
///
/// Minimal and forgiving: unknown fields are ignored so the graph can evolve.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PatchGraph {
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Node {
    Interface { id: String, interface: String },
    Poly { id: String, voices: usize },
    Module { id: String, unit: String },
    Converter { id: String, unit: String },
    Merge { id: String, mode: String },
}

impl Node {
    pub fn id(&self) -> &str {
        match self {
            Node::Interface { id, .. }
            | Node::Poly { id, .. }
            | Node::Module { id, .. }
            | Node::Converter { id, .. }
            | Node::Merge { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Edge {
    pub from: PortRef,
    pub to: PortRef,
}

/// A reference to a node's port, optionally indexed — e.g. `mult_0.in[0]` or `merge_0.out`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PortRef {
    pub node: String,
    pub port: String,
    pub index: Option<usize>,
}

impl FromStr for PortRef {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (node, rest) = s.split_once('.').ok_or_else(|| {
            eyre::eyre!("invalid port reference '{s}': expected 'node.port' or 'node.port[index]'")
        })?;

        let (port, index) = if let Some(open) = rest.find('[') {
            let close = rest
              .rfind(']')
              .ok_or_else(|| eyre::eyre!("invalid port reference '{s}': unterminated '['"))?;
            let index_str = &rest[open + 1..close];
            let index = index_str.parse::<usize>().map_err(|_| {
                eyre::eyre!("invalid port reference '{s}': index '{index_str}' is not a number")
            })?;
            (&rest[..open], Some(index))
        } else {
            (rest, None)
        };

        Ok(PortRef {
            node: node.to_string(),
            port: port.to_string(),
            index,
        })
    }
}

impl TryFrom<String> for PortRef {
    type Error = eyre::Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl std::fmt::Display for PortRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.index {
            Some(i) => write!(f, "{}.{}[{}]", self.node, self.port, i),
            None => write!(f, "{}.{}", self.node, self.port),
        }
    }
}

impl From<PortRef> for String {
    fn from(port_ref: PortRef) -> String {
        port_ref.to_string()
    }
}

impl FromStr for PatchGraph {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).wrap_err("failed to parse patch graph")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal() {
        let json = r#"
{
  "nodes": [
    { "id": "midi_in_0", "kind": "interface", "interface": "midi_in" },
    { "id": "audio_out_0", "kind": "interface", "interface": "audio_out" }
  ],
  "edges": []
}
"#;
        let graph: PatchGraph = json.parse().unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 0);
        assert_eq!(graph.nodes[0].id(), "midi_in_0");
        assert!(
            matches!(&graph.nodes[0], Node::Interface { interface, .. } if interface == "midi_in")
        );
    }

    #[test]
    fn test_parse_port_ref_with_index() {
        let port_ref: PortRef = "mult_0.in[0]".parse().unwrap();
        assert_eq!(port_ref.node, "mult_0");
        assert_eq!(port_ref.port, "in");
        assert_eq!(port_ref.index, Some(0));
    }

    #[test]
    fn test_parse_port_ref_without_index() {
        let port_ref: PortRef = "merge_0.out".parse().unwrap();
        assert_eq!(port_ref.node, "merge_0");
        assert_eq!(port_ref.port, "out");
        assert_eq!(port_ref.index, None);
    }

    #[test]
    fn test_parse_edges_with_port_refs() {
        let json = r#"
{
  "nodes": [
    { "id": "mult_0", "kind": "module", "unit": "kahuna_std::Multiply" }
  ],
  "edges": [
    { "from": "sine_osc_0.out", "to": "mult_0.in[0]" }
  ]
}
"#;
        let graph: PatchGraph = json.parse().unwrap();
        assert_eq!(graph.edges[0].to.node, "mult_0");
        assert_eq!(graph.edges[0].to.index, Some(0));
    }
}
