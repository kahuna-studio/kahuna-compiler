use crate::compiler::ast_construction::ast::basics::{
    Assignment, BlockOrExpression, Expression, File, PortType, Type,
};
use crate::compiler::ast_construction::ast::functions::{
    Argument, ConstructorCall, FunctionCall, FunctionDeclaration, Parameter,
};
use crate::compiler::ast_construction::ast::modules::{
    InputPort, ModuleDeclaration, OutputPort as ModuleOutputPort,
};
use crate::compiler::ast_construction::ast::streams::{StreamExpression, StreamOperation};
use serde_json::{json, Value};

pub struct AstJsonSerializer;

impl AstJsonSerializer {
    /// Serialize a File AST to JSON Value
    pub fn serialize_file(file: &File) -> Value {
        let mut obj = serde_json::Map::new();

        if let Some(module) = &file.module_declaration {
            obj.insert("module".to_string(), Self::serialize_module(module));
        }

        if !file.function_declarations.is_empty() {
            obj.insert(
                "functions".to_string(),
                Value::Array(
                    file.function_declarations
                        .iter()
                        .map(Self::serialize_function_declaration)
                        .collect(),
                ),
            );
        }

        Value::Object(obj)
    }

    pub fn serialize_module(module: &ModuleDeclaration) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), Value::String(module.name.clone()));

        if !module.inputs.is_empty() {
            obj.insert(
                "inputs".to_string(),
                Value::Array(
                    module.inputs
                        .iter()
                        .map(Self::serialize_input_port)
                        .collect(),
                ),
            );
        }

        if !module.outputs.is_empty() {
            obj.insert(
                "outputs".to_string(),
                Value::Array(
                    module.outputs
                        .iter()
                        .map(Self::serialize_output_port)
                        .collect(),
                ),
            );
        }

        if !module.assignments.is_empty() {
            obj.insert(
                "assignments".to_string(),
                Value::Array(
                    module.assignments
                        .iter()
                        .map(Self::serialize_assignment)
                        .collect(),
                ),
            );
        }

        Value::Object(obj)
    }

    pub fn serialize_function_declaration(func: &FunctionDeclaration) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), Value::String(func.name.clone()));

        obj.insert(
            "parameters".to_string(),
            Value::Array(
                func.parameters
                    .iter()
                    .map(Self::serialize_parameter)
                    .collect(),
            ),
        );

        if let Some(return_type) = &func.return_type {
            obj.insert("returnType".to_string(), Self::serialize_type(return_type));
        }

        obj.insert(
            "body".to_string(),
            Self::serialize_block_or_expression(&func.body),
        );

        Value::Object(obj)
    }

    pub fn serialize_assignment(assignment: &Assignment) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), Value::String(assignment.name.clone()));
        obj.insert("isLocal".to_string(), Value::Bool(assignment.is_local));

        if let Some(ty) = &assignment.ty {
            obj.insert("type".to_string(), Self::serialize_type(ty));
        }

        obj.insert(
            "value".to_string(),
            Self::serialize_expression(&assignment.value),
        );

        Value::Object(obj)
    }

    pub fn serialize_expression(expr: &Expression) -> Value {
        match expr {
            Expression::Identifier(name) => {
                json!({
                    "type": "Identifier",
                    "name": name
                })
            }
            Expression::Literal(lit) => {
                json!({
                    "type": "Literal",
                    "value": lit.get_literal()
                })
            }
            Expression::Binary { op, lhs, rhs } => {
                json!({
                    "type": "Binary",
                    "operator": op.to_string(),
                    "left": Self::serialize_expression(lhs),
                    "right": Self::serialize_expression(rhs)
                })
            }
            Expression::If {
                condition,
                then,
                else_,
            } => {
                json!({
                    "type": "If",
                    "condition": Self::serialize_expression(condition),
                    "then": Self::serialize_block_or_expression(then),
                    "else": Self::serialize_block_or_expression(else_)
                })
            }
            Expression::FunctionCall(call) => {
                json!({
                    "type": "FunctionCall",
                    "call": Self::serialize_function_call(call)
                })
            }
            Expression::ConstructorCall(call) => {
                json!({
                    "type": "ConstructorCall",
                    "call": Self::serialize_constructor_call(call)
                })
            }
            Expression::StreamExpression(stream) => {
                json!({
                    "type": "StreamExpression",
                    "stream": Self::serialize_stream_expression(stream)
                })
            }
        }
    }

    pub fn serialize_block_or_expression(block_or_expr: &BlockOrExpression) -> Value {
        match block_or_expr {
            BlockOrExpression::Expression(expr) => {
                json!({
                    "type": "Expression",
                    "expression": Self::serialize_expression(expr)
                })
            }
            BlockOrExpression::Block {
                assignments,
                result,
            } => {
                json!({
                    "type": "Block",
                    "assignments": Value::Array(
                        assignments
                            .iter()
                            .map(Self::serialize_assignment)
                            .collect()
                    ),
                    "result": Self::serialize_expression(result)
                })
            }
        }
    }

    pub fn serialize_type(ty: &Type) -> Value {
        match ty {
            Type::Primitive(name) => {
                json!({
                    "type": "Primitive",
                    "name": name
                })
            }
            Type::Named(name) => {
                json!({
                    "type": "Named",
                    "name": name
                })
            }
            Type::Parameterized { name, parameters } => {
                json!({
                    "type": "Parameterized",
                    "name": name,
                    "parameters": Value::Array(
                        parameters
                            .iter()
                            .map(Self::serialize_type)
                            .collect()
                    )
                })
            }
            Type::Tuple(elements) => {
                json!({
                    "type": "Tuple",
                    "elements": Value::Array(
                        elements
                            .iter()
                            .map(Self::serialize_type)
                            .collect()
                    )
                })
            }
            Type::Map { key, value } => {
                json!({
                    "type": "Map",
                    "keyType": Self::serialize_type(key),
                    "valueType": Self::serialize_type(value)
                })
            }
            Type::Array { element, size } => {
                let mut obj = serde_json::Map::new();
                obj.insert("type".to_string(), Value::String("Array".to_string()));
                obj.insert("elementType".to_string(), Self::serialize_type(element));
                if let Some(size) = size {
                    obj.insert("size".to_string(), Value::Number((*size).into()));
                }
                Value::Object(obj)
            }
            Type::Option { element } => {
                json!({
                    "type": "Option",
                    "elementType": Self::serialize_type(element)
                })
            }
        }
    }

    pub fn serialize_port_type(port_type: &PortType) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "valueType".to_string(),
            Self::serialize_type(&port_type.value_type),
        );

        if let Some(range) = &port_type.range {
            obj.insert(
                "range".to_string(),
                json!({
                    "min": range.min,
                    "max": range.max
                }),
            );
        }

        if let Some(rate) = &port_type.rate {
            obj.insert("rate".to_string(), Value::String(rate.get_literal().to_string()));
        }

        if let Some(default_value) = &port_type.default_value {
            obj.insert(
                "defaultValue".to_string(),
                Value::String(default_value.get_literal().to_string()),
            );
        }

        Value::Object(obj)
    }

    pub fn serialize_input_port(input: &InputPort) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), Value::String(input.name.clone()));
        obj.insert(
            "streamType".to_string(),
            Value::String(input.stream_type.get_symbol().to_string()),
        );
        obj.insert("portType".to_string(), Self::serialize_port_type(&input.port_type));

        if let Some(default_value) = &input.default_value {
            obj.insert("defaultValue".to_string(), Value::String(default_value.clone()));
        }

        Value::Object(obj)
    }

    pub fn serialize_output_port(output: &ModuleOutputPort) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), Value::String(output.name.clone()));
        obj.insert(
            "streamType".to_string(),
            Value::String(output.stream_type.get_symbol().to_string()),
        );
        obj.insert("portType".to_string(), Self::serialize_port_type(&output.port_type));

        Value::Object(obj)
    }

    pub fn serialize_parameter(param: &Parameter) -> Value {
        json!({
            "name": param.name,
            "type": Self::serialize_type(&param.ty)
        })
    }

    pub fn serialize_argument(arg: &Argument) -> Value {
        let mut obj = serde_json::Map::new();
        if let Some(name) = &arg.name {
            obj.insert("name".to_string(), Value::String(name.clone()));
        }
        obj.insert(
            "value".to_string(),
            Self::serialize_expression(&arg.value),
        );
        Value::Object(obj)
    }

    pub fn serialize_function_call(call: &FunctionCall) -> Value {
        json!({
            "name": call.name,
            "arguments": Value::Array(
                call.arguments
                    .iter()
                    .map(Self::serialize_argument)
                    .collect()
            )
        })
    }

    pub fn serialize_constructor_call(call: &ConstructorCall) -> Value {
        json!({
            "type": Self::serialize_type(&call.ty),
            "arguments": Value::Array(
                call.arguments
                    .iter()
                    .map(Self::serialize_argument)
                    .collect()
            )
        })
    }

    pub fn serialize_stream_expression(stream: &StreamExpression) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "source".to_string(),
            Self::serialize_expression(&stream.source),
        );
        obj.insert(
            "operations".to_string(),
            Value::Array(
                stream
                    .operations
                    .iter()
                    .map(Self::serialize_stream_operation)
                    .collect(),
            ),
        );
        Value::Object(obj)
    }

    pub fn serialize_stream_operation(op: &StreamOperation) -> Value {
        match op {
            StreamOperation::Map(expr) => {
                json!({
                    "type": "Map",
                    "expression": Self::serialize_expression(expr)
                })
            }
            StreamOperation::Filter(expr) => {
                json!({
                    "type": "Filter",
                    "expression": Self::serialize_expression(expr)
                })
            }
        }
    }

    /// Convert a File AST to a pretty-printed JSON string
    pub fn serialize_file_to_string(file: &File) -> Result<String, serde_json::Error> {
        let json = Self::serialize_file(file);
        serde_json::to_string_pretty(&json)
    }

    /// Convert a File AST to a compact JSON string
    pub fn serialize_file_to_compact_string(file: &File) -> Result<String, serde_json::Error> {
        let json = Self::serialize_file(file);
        serde_json::to_string(&json)
    }
}

