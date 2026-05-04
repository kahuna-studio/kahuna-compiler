use crate::compiler::ast::basics::{
    Assignment, BlockOrExpression, Expression, File, PortType, Type,
};
use crate::compiler::ast::functions::{
    Argument, ConstructorCall, FunctionCall, FunctionDeclaration, Parameter,
};
use crate::compiler::ast::modules::{
    InputPort, ModuleDeclaration, OutputPort as ModuleOutputPort,
};
use crate::compiler::ast::streams::{StreamExpression, StreamOperation};

pub trait AstVisitor {
    fn visit_file(&mut self, file: &File);
    fn visit_module_declaration(&mut self, module: &ModuleDeclaration);
    fn visit_function_declaration(&mut self, indentation: u8, func: &FunctionDeclaration);
    fn visit_assignment(&mut self, indentation: u8, assignment: &Assignment);
    fn visit_expression(&mut self, indentation: u8, expr: &Expression);
    fn visit_block_or_expression(&mut self, indentation: u8, block_or_expr: &BlockOrExpression);
    fn visit_type(&mut self, ty: &Type);
    fn visit_port_type(&mut self, port_type: &PortType);
    fn visit_input_port(&mut self, input: &InputPort);
    fn visit_output_port(&mut self, output: &ModuleOutputPort);
    fn visit_parameter(&mut self, param: &Parameter);
    fn visit_argument(&mut self, indentation: u8, arg: &Argument);
    fn visit_function_call(&mut self, indentation: u8, call: &FunctionCall);
    fn visit_constructor_call(&mut self, indentation: u8, call: &ConstructorCall);
    fn visit_stream_expression(&mut self, indentation: u8, stream: &StreamExpression);
    fn visit_stream_operation(&mut self, indentation: u8, op: &StreamOperation);
}

pub struct AstPrinter {
    output: String,
}

impl AstPrinter {
    pub fn new() -> Self {
        AstPrinter {
            output: String::new(),
        }
    }

    pub fn get_output(&self) -> &str {
        &self.output
    }

    fn push(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn push_fmt(&mut self, args: std::fmt::Arguments) {
        self.output.push_str(&format!("{}", args));
    }
}

impl AstVisitor for AstPrinter {
    fn visit_file(&mut self, file: &File) {
        if let Some(module) = &file.module_declaration {
            self.visit_module_declaration(module);
        }
        for func in &file.function_declarations {
            self.visit_function_declaration(0, func);
        }
    }

    fn visit_module_declaration(&mut self, module: &ModuleDeclaration) {
        self.push_fmt(format_args!("module {}:\n\n", module.name));

        if !module.inputs.is_empty() {
            self.push("  inputs:\n");
            for input in &module.inputs {
                self.push("    ");
                self.visit_input_port(input);
                self.push("\n");
            }
            self.push("\n\n");
        }

        if !module.outputs.is_empty() {
            self.push("  outputs:\n");
            for output in &module.outputs {
                self.push_fmt(format_args!(
                    "    {}{} ",
                    output.stream_type.get_symbol(),
                    output.name,
                ));
                self.visit_port_type(&output.port_type);
                self.push("\n");
            }
            self.push("\n\n");
        }
        for assignment in &module.assignments {
            self.visit_assignment(1, assignment);
            self.push("\n");
        }
        self.push("\n");
    }

    fn visit_function_declaration(&mut self, indentation: u8, func: &FunctionDeclaration) {
        for _ in 0..indentation {
            self.push("  ");
        }
        self.push_fmt(format_args!("{} = (", func.name));

        for (i, parameter) in func.parameters.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            self.visit_parameter(parameter);
        }
        self.push(") ->");
        if let Some(return_type) = &func.return_type {
            self.push(" ");
            self.visit_type(return_type);
        }
        self.visit_block_or_expression(indentation, &func.body);
        self.push("\n");
    }

    fn visit_assignment(&mut self, indentation: u8, assignment: &Assignment) {
        if assignment.is_local {
            self.push("let ");
        }
        self.push(&assignment.name);
        if let Some(ty) = &assignment.ty {
            self.push(": ");
            self.visit_type(ty);
        }
        self.push(" = ");
        self.visit_expression(indentation, &assignment.value);
    }

    fn visit_expression(&mut self, indentation: u8, expr: &Expression) {
        match expr {
            Expression::Identifier(name) => self.push(name),
            Expression::Literal(lit) => self.push(lit.get_literal()),
            Expression::Binary { op, lhs, rhs } => {
                self.visit_expression(indentation, lhs);
                self.push_fmt(format_args!(" {} ", op));
                self.visit_expression(indentation, rhs);
            }
            Expression::If {
                condition,
                then,
                else_,
            } => {
                self.push("if ");
                self.visit_expression(indentation, condition);
                self.push(" then ");
                self.visit_block_or_expression(indentation, then);
                self.push(" else ");
                self.visit_block_or_expression(indentation, else_);
            }
            Expression::FunctionCall(call) => self.visit_function_call(indentation, call),
            Expression::ConstructorCall(call) => self.visit_constructor_call(indentation, call),
            Expression::StreamExpression(stream) => {
                self.visit_stream_expression(indentation, stream)
            }
        }
    }

    fn visit_block_or_expression(&mut self, indentation: u8, block_or_expr: &BlockOrExpression) {
        match block_or_expr {
            BlockOrExpression::Expression(expr) => {
                self.push(" ");
                self.visit_expression(indentation, expr)
            }
            BlockOrExpression::Block {
                assignments,
                result,
            } => {
                self.push("\n");
                for assignment in assignments {
                    self.visit_assignment(indentation + 1, assignment);
                    self.push("\n");
                }

                for _ in 0..indentation + 1 {
                    self.push("  ");
                }
                self.visit_expression(indentation + 1, result);
                self.push("\n");
            }
        }
    }

    fn visit_type(&mut self, ty: &Type) {
        match ty {
            Type::Primitive(name) | Type::Named(name) => self.push(name),
            Type::Parameterized { name, parameters } => {
                self.push(name);
                self.push("<");
                for (i, param) in parameters.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.visit_type(param);
                }
                self.push(">");
            }
            Type::Tuple(elements) => {
                self.push("(");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.visit_type(elem);
                }
                self.push(")");
            }
            Type::Map { key, value } => {
                self.push("{");
                self.visit_type(key);
                self.push(", ");
                self.visit_type(value);
                self.push("}");
            }
            Type::Array { element, size } => {
                self.visit_type(element);
                if let Some(size) = size {
                    self.push_fmt(format_args!("[{}]", size));
                } else {
                    self.push("[]");
                }
            }
            Type::Option { element } => {
                self.push("Option<");
                self.visit_type(element);
                self.push(">");
            }
        }
    }

    fn visit_port_type(&mut self, port_type: &PortType) {
        self.visit_type(&port_type.value_type);
        if port_type.range.is_some() || port_type.rate.is_some() {
            self.push(" (");
            if let Some(range) = &port_type.range {
                self.push_fmt(format_args!("({}..{}", range.min, range.max));
                if port_type.rate.is_some() {
                    self.push(", ");
                }
            }
            if let Some(rate) = &port_type.rate {
                self.push(rate.get_literal());
            }
            self.push(")");
        }
        if let Some(default_value) = &port_type.default_value {
            self.push_fmt(format_args!("= {}", default_value.get_literal()));
        }
    }

    fn visit_input_port(&mut self, input: &InputPort) {
        self.push_fmt(format_args!(
            "{}{} ",
            input.stream_type.get_symbol(),
            input.name
        ));
        self.visit_port_type(&input.port_type);
        if let Some(default_value) = &input.default_value {
            self.push_fmt(format_args!(" = {}", default_value));
        }
    }

    fn visit_output_port(&mut self, output: &ModuleOutputPort) {
        self.push_fmt(format_args!(
            "{}{} ",
            output.stream_type.get_symbol(),
            output.name
        ));
        self.visit_port_type(&output.port_type);
    }

    fn visit_parameter(&mut self, param: &Parameter) {
        self.push(&param.name);
        self.push(" ");
        self.visit_type(&param.ty);
    }

    fn visit_argument(&mut self, indentation: u8, arg: &Argument) {
        if let Some(name) = &arg.name {
            self.push_fmt(format_args!("{}: ", name));
        }
        self.visit_expression(indentation, &arg.value);
    }

    fn visit_function_call(&mut self, indentation: u8, call: &FunctionCall) {
        self.push(&call.name);
        self.push("(");
        for (i, arg) in call.arguments.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            self.visit_argument(indentation, arg);
        }
        self.push(")");
    }

    fn visit_constructor_call(&mut self, indentation: u8, call: &ConstructorCall) {
        self.visit_type(&call.ty);
        self.push("(");
        for (i, arg) in call.arguments.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            self.visit_argument(indentation, arg);
        }
        self.push(")");
    }

    fn visit_stream_expression(&mut self, indentation: u8, stream: &StreamExpression) {
        self.visit_expression(indentation, &stream.source);
        for op in &stream.operations {
            self.visit_stream_operation(indentation + 1, op);
        }
    }

    fn visit_stream_operation(&mut self, indentation: u8, op: &StreamOperation) {
        match op {
            StreamOperation::Map(expr) => {
                self.push(".map(");
                self.visit_expression(indentation, expr);
                self.push(")");
            }
            StreamOperation::Filter(expr) => {
                self.push(".filter(");
                self.visit_expression(indentation, expr);
                self.push(")");
            }
        }
    }
}
