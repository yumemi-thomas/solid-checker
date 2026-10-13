//! Runtime primitive completions, without trusting a declared result type.
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::Expression;
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};

use crate::core::Span;

/// A proof about the value on normal completion, not about effects or throws.
/// Exact expression spans only; neither identifiers nor calls are inferred.
/// Transparent TS wrappers preserve the runtime value, never prove its type.
#[must_use]
pub fn primitive_completion_by_syntax(path: &Path, source: &str, at: Span) -> bool {
    if source.len() > 4 * 1024 * 1024 {
        return false;
    }
    let Ok(source_type) = SourceType::from_path(path) else {
        return false;
    };
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type.with_module(true)).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return false;
    }
    let mut query = PrimitiveQuery { at, proven: false };
    query.visit_program(&parsed.program);
    query.proven
}

struct PrimitiveQuery {
    at: Span,
    proven: bool,
}

impl<'a> Visit<'a> for PrimitiveQuery {
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        let span = expression.span();
        if span.start == self.at.start && span.end == self.at.end {
            self.proven = primitive(expression, 0);
        } else if span.start <= self.at.start && span.end >= self.at.end {
            walk::walk_expression(self, expression);
        }
    }
}

fn primitive(expression: &Expression<'_>, depth: usize) -> bool {
    if depth > 32 {
        return false;
    }
    match super::peel_ts_sugar(expression) {
        Expression::NullLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::TemplateLiteral(_) => true,
        Expression::UnaryExpression(unary) => match unary.operator {
            // These operations cannot produce an object, regardless of their
            // operand. Evaluation effects remain the caller's own obligations.
            UnaryOperator::Typeof | UnaryOperator::Void | UnaryOperator::LogicalNot => true,
            UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot => {
                primitive(&unary.argument, depth + 1)
            }
            _ => false,
        },
        Expression::BinaryExpression(binary) => match binary.operator {
            BinaryOperator::StrictEquality | BinaryOperator::StrictInequality => true,
            BinaryOperator::Addition
            | BinaryOperator::Subtraction
            | BinaryOperator::Multiplication
            | BinaryOperator::Exponential
            | BinaryOperator::Division
            | BinaryOperator::Remainder
            | BinaryOperator::LessThan
            | BinaryOperator::GreaterThan
            | BinaryOperator::LessEqualThan
            | BinaryOperator::GreaterEqualThan
            | BinaryOperator::Equality
            | BinaryOperator::Inequality
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::ShiftRightZeroFill
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOR
            | BinaryOperator::BitwiseXOR => {
                primitive(&binary.left, depth + 1) && primitive(&binary.right, depth + 1)
            }
            _ => false,
        },
        _ => false,
    }
}
