use serde::{Deserialize, Serialize};
use crate::ast::basics::Expression;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreamExpression {
    pub source: Box<Expression>,
    pub operations: Vec<StreamOperation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StreamOperation {
    Map(Expression),
    Filter(Expression)
}
