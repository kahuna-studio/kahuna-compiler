// ============================================================
// compiler/graph.rs
//
// A directed graph representing a Kahuna patch.
//
// Nodes are unique patch entities: interfaces, modules,
// converters, the poly allocator, and the merge node.
//
// Edges are port-to-port connections between nodes.
// ============================================================

use serde::{Deserialize, Serialize};

pub type NodeId = usize;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceType {
    MidiIn,
    MidiOut,
    AudioIn,
    AudioOut
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(tag = "kind")]
pub enum NodeKind {
    Interface { interface_type: InterfaceType, index: usize },
    Module { module_type: String, index: usize },
    Converter { converter_type: String, index: usize },
    Poly,
    Merge,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Node {
    pub id: NodeId,
    #[serde(flatten)]
    pub kind: NodeKind,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdgeFromPort {
    pub node: NodeId,
    pub port: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdgeToPort {
    pub node: NodeId,
    pub port: String,
    #[serde(default)]
    pub index: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    pub from: EdgeFromPort,
    pub to: EdgeToPort,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PatchGraph {
    pub name: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl PatchGraph {
    pub fn new(name: String) -> Self {
        Self {
            name,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Returns the id of the existing node with this kind, or adds it and returns the new id.
    pub fn get_or_add_node(&mut self, kind: NodeKind) -> NodeId {
        if let Some(node) = self.nodes.iter().find(|n| n.kind == kind) {
            return node.id;
        }
        let id = self.nodes.len();
        self.nodes.push(Node { id, kind });
        id
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }
}
