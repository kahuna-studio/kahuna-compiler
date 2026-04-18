use pest::iterators::Pair;
use pest::pratt_parser::{Assoc, Op, PrattParser};

use crate::compiler::ast_construction::ast::basics::{Expression, Literal};
use crate::compiler::ast_construction::ast_mapper::{
    parse_constructor_call, parse_function_call, parse_if_else, parse_pipelined_expression,
};
use crate::compiler::source_file_parser::source_file_parser::Rule;

lazy_static::lazy_static! {
    pub static ref PRATT: PrattParser<Rule> = PrattParser::new()
        // Lowest precedence (logical OR)
        .op(Op::infix(Rule::logical_or, Assoc::Left))
        // Then logical AND
        .op(Op::infix(Rule::logical_and, Assoc::Left))
        // Then comparisons
        .op(Op::infix(Rule::eq, Assoc::Left)
            | Op::infix(Rule::ne, Assoc::Left)
            | Op::infix(Rule::lt, Assoc::Left)
            | Op::infix(Rule::le, Assoc::Left)
            | Op::infix(Rule::gt, Assoc::Left)
            | Op::infix(Rule::ge, Assoc::Left))
        // Then additive
        .op(Op::infix(Rule::add, Assoc::Left)
            | Op::infix(Rule::subtract, Assoc::Left))
        // Then multiplicative
        .op(Op::infix(Rule::multiply, Assoc::Left)
            | Op::infix(Rule::divide, Assoc::Left)
            | Op::infix(Rule::modulo, Assoc::Left))
        // Highest: power
        .op(Op::infix(Rule::power, Assoc::Right));
}

pub fn parse_expr(pair: Pair<Rule>) -> Expression {
    PRATT
        .map_primary(|primary| match primary.as_rule() {
            Rule::sc_identifier => Expression::Identifier(primary.as_str().to_string()),
            Rule::literal => {
                Expression::Literal(Literal::Number(primary.as_str().parse().unwrap()))
            }
            Rule::number => Expression::Literal(Literal::Number(primary.as_str().parse().unwrap())),

            Rule::atom => {
                let inner = primary.into_inner().next().unwrap();
                match inner.as_rule() {
                    Rule::sc_identifier => Expression::Identifier(inner.as_str().to_string()),
                    Rule::literal => {
                        Expression::Literal(Literal::Number(inner.as_str().parse().unwrap()))
                    }
                    Rule::constructor_call => parse_constructor_call(inner),
                    Rule::function_call => parse_function_call(inner),
                    _ => unreachable!("Unexpected inside term: {:?}", inner),
                }
            }

            Rule::if_expr => parse_if_else(primary),
            Rule::function_call => parse_function_call(primary),
            Rule::constructor_call => parse_constructor_call(primary),
            Rule::pipelined_expression => parse_pipelined_expression(primary),
            _ => unreachable!("Unexpected primary: {:?}", primary),
        })
        .map_infix(|lhs, op, rhs| Expression::Binary {
            op: op.as_str().to_string(),
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
        .parse(pair.into_inner())
}
