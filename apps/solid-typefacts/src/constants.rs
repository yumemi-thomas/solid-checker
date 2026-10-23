//! Compiler-proven constant values (Go: `tsgo/constant_values.go`).
//!
//! The evaluator folds exactly the demanded expression: string and number
//! literals, unary plus and minus, `+` of two values of the same kind, type
//! wrappers (`as`, `satisfies`, `!`, parentheses), and identifiers or property
//! accesses that resolve to a `const` variable, a `readonly` property or an
//! enum member with an initializer. Anything else, including template
//! expressions, stays absent. A node whose type is `any`, `unknown` or the
//! error type is never folded, and a cycle or a chain deeper than
//! [`MAX_DEPTH`] answers absent.

use crate::gocompat::clean;
use crate::identity::Env;
use crate::semantic_runs::Evidence;
use std::collections::HashSet;
use std::sync::Arc;
use tsrs_ast::{
    Kind, ModifierFlags, Node, get_source_file_of_node, has_syntactic_modifier, is_as_expression,
    is_binary_expression, is_enum_member, is_identifier, is_no_substitution_template_literal,
    is_non_null_expression, is_numeric_literal, is_parenthesized_expression,
    is_prefix_unary_expression, is_property_access_expression, is_property_declaration,
    is_satisfies_expression, is_string_literal, is_var_const, is_variable_declaration,
};
use tsrs_checker::types::TypeFlags;
use tsrs_core::P;
use tsrs_scanner::skip_trivia;
use typefacts::{ConstantValue, ConstantValueKind, Location};

/// Go: `constantValueMaxDepth`.
const MAX_DEPTH: usize = 64;

/// Go: `constantValueAtLocked`.
pub fn constant_value_at(
    env: &mut Env<'_>,
    node: P<Node>,
    evidence: &mut Evidence,
) -> Option<ConstantValue> {
    Evaluator {
        env,
        evidence,
        seen: HashSet::new(),
    }
    .evaluate(node, 0)
}

struct Evaluator<'a, 'e> {
    env: &'a mut Env<'e>,
    evidence: &'a mut Evidence,
    seen: HashSet<usize>,
}

fn string_value(text: &str) -> ConstantValue {
    ConstantValue {
        kind: ConstantValueKind::String,
        string: Arc::from(text),
        number: 0.0,
    }
}

fn number_value(number: f64) -> ConstantValue {
    ConstantValue {
        kind: ConstantValueKind::Number,
        string: Arc::from(""),
        number,
    }
}

impl Evaluator<'_, '_> {
    /// Go: `constantValueEvaluator.evaluate`.
    fn evaluate(&mut self, node: P<Node>, depth: usize) -> Option<ConstantValue> {
        if depth >= MAX_DEPTH || !self.has_reliable_type(node) {
            return None;
        }
        if !self.seen.insert(node.addr()) {
            return None;
        }
        let value = self.evaluate_node(node, depth);
        self.seen.remove(&node.addr());
        value
    }

    fn evaluate_node(&mut self, node: P<Node>, depth: usize) -> Option<ConstantValue> {
        if is_string_literal(node) || is_no_substitution_template_literal(node) {
            return Some(string_value(node.text()));
        }
        if is_numeric_literal(node) {
            return parse_numeric_literal(node.text()).map(number_value);
        }
        if is_parenthesized_expression(node)
            || is_as_expression(node)
            || is_satisfies_expression(node)
            || is_non_null_expression(node)
        {
            return self.evaluate(node.expression()?, depth + 1);
        }
        if is_prefix_unary_expression(node) {
            let unary = node.as_prefix_unary_expression();
            let operator = unary.operator();
            if operator != Kind::PlusToken && operator != Kind::MinusToken {
                return None;
            }
            let mut value = self.evaluate(unary.operand(), depth + 1)?;
            if value.kind != ConstantValueKind::Number {
                return None;
            }
            if operator == Kind::MinusToken {
                value.number = -value.number;
            }
            return Some(value);
        }
        if is_binary_expression(node) {
            let binary = node.as_binary_expression();
            if binary.operator_token().kind() != Kind::PlusToken {
                return None;
            }
            let left = self.evaluate(binary.left(), depth + 1);
            let right = self.evaluate(binary.right(), depth + 1);
            let (mut left, right) = (left?, right?);
            if left.kind != right.kind {
                return None;
            }
            return match left.kind {
                ConstantValueKind::String => {
                    left.string = Arc::from(format!("{}{}", left.string, right.string));
                    Some(left)
                }
                ConstantValueKind::Number => {
                    left.number += right.number;
                    if left.number.is_nan() {
                        return None;
                    }
                    Some(left)
                }
            };
        }
        if is_identifier(node) || is_property_access_expression(node) {
            return self.evaluate_resolved_declaration(node, depth + 1);
        }
        // Template expressions always contain substitutions and deliberately do
        // not participate, even when those substitutions happen to be constant.
        None
    }

    /// Go: `hasReliableType`.
    fn has_reliable_type(&mut self, node: P<Node>) -> bool {
        let value = self.env.checker.get_type_at_location(node);
        !value
            .flags()
            .intersects(TypeFlags::Any | TypeFlags::Unknown | TypeFlags::IncludesError)
    }

    /// Go: `evaluateResolvedDeclaration`.
    fn evaluate_resolved_declaration(
        &mut self,
        node: P<Node>,
        depth: usize,
    ) -> Option<ConstantValue> {
        let symbol = self.env.checker.get_symbol_at_location_exported(node)?;
        let symbol = self.env.canonical_symbol(symbol);
        let declaration = symbol.value_declaration()?;
        let initializer = if is_variable_declaration(declaration) {
            if !is_var_const(declaration) {
                return None;
            }
            declaration.initializer()
        } else if is_property_declaration(declaration) {
            if !has_syntactic_modifier(declaration, ModifierFlags::Readonly) {
                return None;
            }
            declaration.initializer()
        } else if is_enum_member(declaration) {
            declaration.initializer()
        } else {
            return None;
        }?;
        if let Some(file) = get_source_file_of_node(declaration)
            && let (Some(pos), Some(end)) = (
                declaration.pos().source_offset(),
                declaration.end().source_offset(),
            )
        {
            self.evidence.dependency(&Location {
                path: clean(file.file_name()).into(),
                start_byte: u64::from(skip_trivia(file.text(), pos)),
                end_byte: u64::from(end),
            });
        }
        self.evaluate(initializer, depth)
    }
}

/// Go: `parseNumericLiteral`: a numeric literal's value, `None` when it is
/// not a finite-or-infinite number (NaN is refused).
fn parse_numeric_literal(text: &str) -> Option<f64> {
    let text = text.replace('_', "");
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'0' && matches!(bytes[1] | 0x20, b'x' | b'b' | b'o') {
        let base = match bytes[1] | 0x20 {
            b'x' => 16,
            b'b' => 2,
            _ => 8,
        };
        return radix_integer_to_f64(&text[2..], base);
    }
    let value: f64 = text.parse().ok()?;
    (!value.is_nan()).then_some(value)
}

/// `new(big.Int).SetString(digits, base)` converted to the nearest `float64`
/// (ties to even; an out-of-range value is infinite). `None` for an empty or
/// malformed digit string.
fn radix_integer_to_f64(digits: &str, base: u32) -> Option<f64> {
    if digits.is_empty() || !digits.chars().all(|digit| digit.is_digit(base)) {
        return None;
    }
    let bits_per_digit = base.trailing_zeros();
    let mut bits: Vec<bool> = Vec::with_capacity(digits.len() * bits_per_digit as usize);
    for digit in digits.chars() {
        let value = digit.to_digit(base)?;
        for shift in (0..bits_per_digit).rev() {
            bits.push((value >> shift) & 1 == 1);
        }
    }
    let first = bits.iter().position(|bit| *bit);
    let Some(first) = first else {
        return Some(0.0);
    };
    let significant = &bits[first..];
    // Up to 128 leading bits are exact in a u128; every bit below them only
    // matters as a sticky bit, which a u128 -> f64 round to nearest even
    // honours because the sticky bit sits far below the 53-bit mantissa.
    let kept = significant.len().min(127);
    let mut mantissa: u128 = 0;
    for bit in &significant[..kept] {
        mantissa = (mantissa << 1) | u128::from(*bit);
    }
    let dropped = &significant[kept..];
    if dropped.iter().any(|bit| *bit) {
        mantissa = (mantissa << 1) | 1;
    } else if !dropped.is_empty() {
        mantissa <<= 1;
    }
    let shift = dropped.len() as i32 - i32::from(!dropped.is_empty());
    #[allow(clippy::cast_precision_loss)]
    let scaled = mantissa as f64;
    Some(if shift == 0 {
        scaled
    } else {
        scaled * 2f64.powi(shift)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radix_literals_convert_like_big_float() {
        assert_eq!(parse_numeric_literal("0xFF"), Some(255.0));
        assert_eq!(parse_numeric_literal("0b101"), Some(5.0));
        assert_eq!(parse_numeric_literal("0o17"), Some(15.0));
        assert_eq!(parse_numeric_literal("0x1_0"), Some(16.0));
        assert_eq!(parse_numeric_literal("0x"), None);
        assert_eq!(parse_numeric_literal("0xG"), None);
        // 2^64 + 1 is not representable: ties to even gives 2^64.
        assert_eq!(
            parse_numeric_literal("0x10000000000000001"),
            Some(18_446_744_073_709_551_616.0)
        );
        // 2^200 exactly, and a value past the float64 range.
        let two_200 = format!("0x1{}", "0".repeat(50));
        assert_eq!(parse_numeric_literal(&two_200), Some(2f64.powi(200)));
        let huge = format!("0x1{}", "0".repeat(300));
        assert_eq!(parse_numeric_literal(&huge), Some(f64::INFINITY));
        // 1 followed by 130 zero bits and a final 1: sticky rounds up only
        // past the halfway point, never changing an exact power of two.
        let sticky = format!("0b1{}1", "0".repeat(130));
        assert_eq!(parse_numeric_literal(&sticky), Some(2f64.powi(131)));
    }

    #[test]
    fn decimal_literals_parse_and_nan_is_refused() {
        assert_eq!(parse_numeric_literal("1_000"), Some(1000.0));
        assert_eq!(parse_numeric_literal("1.5e3"), Some(1500.0));
        assert_eq!(parse_numeric_literal(".5"), Some(0.5));
        assert_eq!(parse_numeric_literal("1e999"), Some(f64::INFINITY));
        assert_eq!(parse_numeric_literal("NaN"), None);
        assert_eq!(parse_numeric_literal("x"), None);
    }
}
