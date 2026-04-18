use pest::iterators::{Pair, Pairs};
use crate::compiler::ast_construction::ast::basics::{Assignment, BlockOrExpression, Expression, File, Literal, PortType, Range, Rate, StreamType, Type};
use crate::compiler::ast_construction::ast::functions::{Argument, ConstructorCall, FunctionCall, FunctionDeclaration, Parameter};
use crate::compiler::ast_construction::ast::modules::{InputPort, ModuleDeclaration, OutputPort};
use crate::compiler::ast_construction::ast::streams::StreamExpression;
use crate::compiler::ast_construction::ast::streams::StreamOperation::Map;
use crate::compiler::source_file_parser::source_file_parser::Rule;
use crate::compiler::ast_construction::ast::streams::StreamOperation;
use crate::compiler::ast_construction::pratt_parser;

pub fn map_to_ast(pairs: &mut Pairs<Rule>) -> Result<File, String> {

    let file_pair = pairs.next().unwrap();

    let mut module_declaration: Option<ModuleDeclaration> = None;
    let mut function_declarations = Vec::new();

    for pair in file_pair.into_inner() {
        match pair.as_rule() {
            Rule::module_declaration => module_declaration = Some(parse_module_declaration(pair)),
            Rule::function_declaration => {
                function_declarations.push(parse_function_declaration(pair))
            }
            _ => {}
        }
    }

    Ok(File {
        function_declarations,
        module_declaration,
    })
}

fn parse_module_declaration(pair: Pair<Rule>) -> ModuleDeclaration {
    let mut inner = pair.into_inner();

    let name = expect_string(&mut inner);
    let mut input_ports = Vec::new();
    let mut output_ports = Vec::new();
    let mut assignments = Vec::new();

    for child in inner {
        let rule = child.as_rule();
        match rule {
            Rule::module_inputs => {
                for input_port in child.into_inner() {
                    input_ports.push(parse_input_port(input_port));
                }
            }
            Rule::module_outputs => {
                for output_port in child.into_inner() {
                    output_ports.push(parse_output_port(output_port));
                }
            }
            Rule::local_assignment => {
                assignments.push(parse_assignment(child, true));
            }
            Rule::output_port_assignment => {
                assignments.push(parse_assignment(child, false));
            }
            other => {
                println!("Unexpected rule in module_declaration: {:?}", other);
            }
        }
    }

    ModuleDeclaration {
        name,
        inputs: input_ports,
        outputs: output_ports,
        assignments,
    }
}

fn parse_input_port(pair: Pair<Rule>) -> InputPort {
  let mut inner = pair.into_inner();

  let name = inner.next().unwrap().as_str().to_string();

  let stream_type = if name.starts_with("~") {
    StreamType::Signal
  } else if name.starts_with("@") {
    StreamType::Parameter
  } else {
    StreamType::Event
  };
  let ty_pair = inner.next().unwrap();
  let port_type = parse_port_type(ty_pair, stream_type.clone());

  InputPort {
    name,
    stream_type,
    port_type,
    default_value: None //@todo: support default values
  }
}

fn parse_output_port(pair: Pair<Rule>) -> OutputPort {
  let mut inner = pair.into_inner();

  let name = inner.next().unwrap().as_str().to_string();

  let stream_type = if name.starts_with("~") {
    StreamType::Signal
  } else if name.starts_with("@") {
    StreamType::Parameter
  } else {
    StreamType::Event
  };
  let ty_pair = inner.next().unwrap();
  let port_type = parse_port_type(ty_pair, stream_type.clone());

  OutputPort {
    name,
    stream_type,
    port_type,
  }
}

fn parse_port_type(pair: Pair<Rule>, stream_type: StreamType) -> PortType {
    let mut inner = pair.into_inner();

    // Stream type or alias
    let first = inner.next().unwrap();
    let value_type;
    let mut range: Option<Range> = None;
    let mut rate: Option<Rate> = None;
    let mut default_value: Option<Literal> = None;

    match first.as_rule() {
        Rule::audio_stream_alias => {
            // alias: float(-1.0..1.0) audio_rate
            value_type = Type::Primitive("float".to_string());
            range = Some(Range {
                min: "-1.0".to_string(),
                max: "1.0".to_string(),
            });
            rate = Some(Rate::AudioRate);
            default_value = None;
        }
        Rule::control_stream_alias => {
            // alias: float(0.0..1.0) control_rate = 0.0
            value_type = Type::Primitive("float".to_string());
            range = Some(Range {
                min: "0.0".to_string(),
                max: "1.0".to_string(),
            });
            rate = Some(Rate::ControlRate);
            default_value = Some(Literal::Number("0.0".to_string()));
        }
        Rule::midi_stream_alias => {
            // alias: Midi_Message[]
            value_type = Type::Array {
                element: Box::from(Type::Named("Midi_Message".to_string())),
                size: None,
            };
        }
        Rule::stream_type => {
            let inner_ty = first.into_inner().next().unwrap();
            value_type = parse_type(inner_ty);
        }
        _ => unreachable!(),
    }

    // Remaining parts (rate, default_value)
    for p in inner {
        match p.as_rule() {
            Rule::audio_rate => rate = Some(Rate::AudioRate),
            Rule::control_rate => rate = Some(Rate::ControlRate),
            Rule::literal => default_value = Some(Literal::Number(p.as_str().to_string())),
            _ => {}
        }
    }

    PortType {
        stream_type,
        value_type,
        range,
        rate,
        default_value,
    }
}

fn parse_type(pair: Pair<Rule>) -> Type {
    match pair.as_rule() {
        Rule::ty => {
            let mut inner = pair.into_inner();
            let base = inner.next().unwrap();
            let mut ty = parse_base_type(base);

            // Handle array suffixes
            for suffix in inner {
                let size = if suffix.as_str().is_empty() {
                    None
                } else {
                    Some(suffix.as_str().parse::<usize>().unwrap())
                };
                ty = Type::Array {
                    element: Box::new(ty),
                    size,
                };
            }

            ty
        }
        _ => unreachable!(),
    }
}

fn parse_base_type(pair: Pair<Rule>) -> Type {
    match pair.as_rule() {
        Rule::numeric_type => Type::Primitive(pair.as_str().to_string()),
        Rule::option_type => {
            let mut inner = pair.into_inner();
            let element_ty = parse_type(inner.next().unwrap());
            Type::Option {
                element: Box::new(element_ty),
            }
        }
        Rule::class_type => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap().as_str().to_string();
            if let Some(params) = inner.next() {
                let parameters = params.into_inner().map(parse_type).collect();
                Type::Parameterized { name, parameters }
            } else {
                Type::Named(name)
            }
        }
        Rule::tuple_type => {
            let elements = pair.into_inner().map(parse_type).collect();
            Type::Tuple(elements)
        }
        Rule::map_type => {
            let mut inner = pair.into_inner();
            let key_ty = parse_type(inner.next().unwrap());
            let value_ty = parse_type(inner.next().unwrap());
            Type::Map {
                key: Box::new(key_ty),
                value: Box::new(value_ty),
            }
        }
        Rule::base_type => {
            // base_type may wrap one of the above, recurse
            parse_base_type(pair.into_inner().next().unwrap())
        }
        _ => panic!("Unexpected base type: {:?}", pair.as_rule()),
    }
}

fn parse_function_declaration(pair: Pair<Rule>) -> FunctionDeclaration {
    let mut inner = pair.into_inner();

    // function_signature
    let sig_pair = inner.next().unwrap();
    let mut sig_inner = sig_pair.into_inner();

    let name = sig_inner.next().unwrap().as_str().to_string();

    // parameters
    let mut params = Vec::new();
    let function_declaration_parameters = sig_inner.next().unwrap();
    let params_or_paren = function_declaration_parameters.into_inner().next();
    match params_or_paren {
        Some(params_or_paren) => {
            if params_or_paren.as_rule() == Rule::parameter_list {
                for param_pair in params_or_paren.into_inner() {
                    let mut param_inner = param_pair.into_inner();
                    let pname = param_inner.next().unwrap().as_str().to_string();
                    let pty = param_inner.next().unwrap();
                    params.push(Parameter {
                        name: pname,
                        ty: parse_type(pty),
                    });
                }
            }
        }
        None => {}
    }

    // return type
    let return_type = sig_inner.next().map(|x| parse_type(x.into_inner().next().unwrap()));

    // function body (after "->")
    let block_pair = inner.next().unwrap();
    let body = parse_block_or_expression(block_pair);

    FunctionDeclaration {
        name,
        parameters: params,
        return_type,
        body,
    }
}

pub fn parse_function_call(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();
    let arguments = parse_arguments(&mut inner);

    Expression::FunctionCall(FunctionCall { name, arguments })
}

pub fn parse_constructor_call(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();

    let ty = parse_type(inner.next().unwrap());
    let arguments = parse_arguments(&mut inner);
    Expression::ConstructorCall(ConstructorCall { ty, arguments })
}

fn parse_arguments(inner: &mut Pairs<Rule>) -> Vec<Argument> {
    let mut arguments = Vec::new();
    if let Some(arguments_or_paren) = inner.next() {
        if arguments_or_paren.as_rule() == Rule::argument_list {
            for argument_pair in arguments_or_paren.into_inner() {
                arguments.push(parse_argument(argument_pair));
            }
        }
    }
    arguments
}

fn parse_argument(argument_pair: Pair<Rule>) -> Argument {
    let mut argument_inner = argument_pair.into_inner();

    let mut name: Option<String> = None;
    let next_pair = argument_inner.next().unwrap();
    let expr_pair = if next_pair.as_rule() == Rule::sc_identifier {
        name = Some(next_pair.as_str().to_string());
        argument_inner.next().unwrap() // the expression
    } else {
        next_pair
    };

    let value = parse_expression(expr_pair);

    Argument { name, value }
}

fn parse_block_or_expression(block_pair: Pair<Rule>) -> BlockOrExpression {
    match block_pair.as_rule() {
        Rule::block => {
            let (assignments, result) = parse_block(block_pair);
            BlockOrExpression::Block {
                assignments,
                result,
            }
        }
        Rule::expression => {
            let expression = parse_expression(block_pair);
            BlockOrExpression::Expression(expression)
        }
        _ => panic!(
            "Unexpected rule in function body: {:?}",
            block_pair.as_rule()
        ),
    }
}

fn parse_block(block_pair: Pair<Rule>) -> (Vec<Assignment>, Expression) {
    let mut assignments = Vec::new();
    let mut final_expr: Option<Expression> = None;

    for stmt_pair in block_pair.into_inner() {
        match stmt_pair.as_rule() {
            Rule::local_assignment => assignments.push(parse_assignment(stmt_pair, true)),
            Rule::expression => final_expr = Some(parse_expression(stmt_pair)),
            _ => panic!("Unexpected rule in block: {:?}", stmt_pair.as_rule()),
        }
    }

    (
        assignments,
        final_expr.expect("Block must have a final expression"),
    )
}

pub fn parse_expression(pair: Pair<Rule>) -> Expression {
    match pair.as_rule() {
        Rule::if_expr => parse_if_else(pair),
        Rule::pipelined_expression => parse_pipelined_expression(pair),
        _ => pratt_parser::parse_expr(pair),
    }
}

pub fn parse_pipelined_expression(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();

    let source = parse_stream_source(inner.next().unwrap());
    let operations = parse_pipeline_operations(&mut inner);

    Expression::StreamExpression(StreamExpression {
        source: Box::new(source),
        operations,
    })
}

pub fn parse_stream_source(pair: Pair<Rule>) -> Expression {
    let next = pair.into_inner().next().unwrap();
    match next.as_rule() {
        Rule::constructor_call => parse_constructor_call(next),
        _ => {
            panic!(
                "Yes, very dirty here... Unexpected rule in stream source: {:?}",
                next.as_rule()
            )
        }
    }
}

fn parse_pipeline_operations(pairs: &mut Pairs<Rule>) -> Vec<StreamOperation> {
    let mut operations = Vec::new();
    for pair in pairs {
        operations.push(parse_pipeline_operation(pair));
    }
    operations
}

fn parse_pipeline_operation(pair: Pair<Rule>) -> StreamOperation {
    match pair.as_rule() {
        Rule::map_operation => parse_map_operation(pair),
        _ => panic!("Unexpected rule in block: {:?}", pair.as_rule()),
    }
}

fn parse_map_operation(pair: Pair<Rule>) -> StreamOperation {
    Map(parse_expression(pair.into_inner().next().unwrap()))
}

pub fn parse_if_else(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();

    let condition_pair = inner.next().expect("if must have condition");
    let condition = Box::new(parse_expression(condition_pair));

    let then_pair = inner.next().expect("if must have then branch");
    let then_expr = Box::new(parse_block_or_expression(then_pair));

    let else_pair = inner.next().expect("if must have else branch");
    let else_expr = Box::new(parse_block_or_expression(else_pair));

    Expression::If {
        condition,
        then: then_expr,
        else_: else_expr,
    }
}

fn parse_assignment(pair: Pair<Rule>, is_local: bool) -> Assignment {
    let mut inner = pair.into_inner();

    // variable name
    let name = expect_string(&mut inner);

    // optional type annotation
    let mut ty: Option<Type> = None;
    let next_pair = inner.next().unwrap();
    let expr_pair = if next_pair.as_rule() == Rule::ty {
        ty = Some(parse_type(next_pair));
        inner.next().unwrap() // the expression
    } else {
        next_pair
    };

    let value = parse_expression(expr_pair);

    Assignment {
        name,
        ty,
        value,
        is_local,
    }
}

fn expect_string(inner: &mut Pairs<Rule>) -> String {
    inner.next().unwrap().as_str().to_string()
}
