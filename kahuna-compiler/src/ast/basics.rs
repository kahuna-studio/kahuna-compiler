use serde::{Deserialize, Serialize};
use crate::ast::functions::{ConstructorCall, FunctionCall, FunctionDeclaration};
use crate::ast::modules::ModuleDeclaration;
use crate::ast::streams::StreamExpression;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum Type {
    Primitive(String),
    Named(String),
    Parameterized {
        name: String,
        parameters: Vec<Type>
    },
    Tuple(Vec<Type>),
    Map {
        key: Box<Type>,
        value: Box<Type>
    },
    Array {
        element: Box<Type>,
        size: Option<usize>
    },
    Option {
        element: Box<Type>
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PortType {
    pub stream_type: StreamType,
    pub value_type: Type,
    pub range: Option<Range>,
    pub rate: Option<Rate>,
    pub default_value: Option<Literal>
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Range {
    pub min: String,
    pub max: String
}


#[derive(Debug, PartialEq, Serialize, Deserialize, Copy, Clone)]
pub enum StreamType {
    Signal,
    Parameter,
    Event
}

impl StreamType {
  pub fn get_symbol(&self) -> &'static str {
    match self {
      StreamType::Signal => "~",
      StreamType::Parameter => "@",
      StreamType::Event => "$",
    }
  }
}


#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum Rate {
    AudioRate,
    ControlRate
}

impl Rate {
  pub fn get_literal(&self) -> &'static str {
    match self {
      Rate::AudioRate => "audio_rate",
      Rate::ControlRate => "control_rate",
    }
  }
}


#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum Literal {
    Number(String)
}

impl Literal {
  pub(crate) fn get_literal(&self) -> &str {
    match self {
      Literal::Number(n) => n.as_str(),
    }
  }
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputPort {
    pub name: String,
    pub ty: Type
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct File {
    pub module_declaration: Option<ModuleDeclaration>,
    pub function_declarations: Vec<FunctionDeclaration>,
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlockOrExpression {
    Expression(Expression),
    Block {
        assignments: Vec<Assignment>,
        result: Expression,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub is_local: bool,
    pub name: String,
    pub ty: Option<Type>,
    pub value: Expression,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    Identifier(String),
    Literal(Literal),
    Binary {
        op: String,
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    If {
        condition: Box<Expression>,
        then: Box<BlockOrExpression>,
        else_: Box<BlockOrExpression>,
    },
    FunctionCall(FunctionCall),
    ConstructorCall(ConstructorCall),
    StreamExpression(StreamExpression)
}
