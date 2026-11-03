use serde::{Deserialize, Serialize};
use crate::ast::basics::{Assignment, PortType, StreamType};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleDeclaration {
    pub name: String,
    pub inputs: Vec<InputPort>,
    pub outputs: Vec<OutputPort>,
    pub assignments: Vec<Assignment>
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputPort {
  pub name: String,
  pub stream_type: StreamType,
  pub port_type: PortType,
  pub default_value: Option<String>
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputPort {
  pub name: String,
  pub stream_type: StreamType,
  pub port_type: PortType
}

// impl OutputPort removed, use AstPrinter
