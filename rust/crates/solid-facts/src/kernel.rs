//! ADR 1120: the inertness kernel, syntax half. Shared allow-lists that answer two questions exactly, for any proof that needs them:
//!
//! - **K1** [`evaluation`]: does evaluating this expression run project code? The answer names the forms known to run none, whatever
//!   the operands are (a Proxy, an object with `toString`, `valueOf` or `Symbol.toPrimitive`, a getter, a class with
//!   `Symbol.hasInstance`). Every other form refuses: the `match` over the expression kinds has no wildcard arm, so new syntax fails the
//!   build instead of passing. What syntax cannot settle comes back as operand obligations for the IR half
//!   (`solid_reactive_ir::kernel`): an identifier must name a binding (a global may be an accessor of the global object), and an
//!   operand a coercing operator converts must be proved primitive.
//! - **K4** [`value_inert`]: is an expression's value provably neither called nor kept (it only meets operators, tests, keys or a
//!   discarded statement)?
//! - **K3** [`cooked_key`]: the exact property key a key's source text denotes, from the parser's cooked value (escapes decoded,
//!   numbers canonical), never from trimmed text.
//!
//! Neither judges anything about the runtime or a library: they are pure functions of source text.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrayExpressionElement, BinaryOperator, Expression, ObjectPropertyKind, PropertyKey,
    PropertyKind, UnaryOperator,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use crate::core::Span;

/// What an operand must be for the evaluation to run no project code.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OperandNeed {
    /// Read only: the identifier must name a binding, not a property of the global object (which may be an accessor).
    Bound,
    /// Converted by an operator (`+`, `<`, `-x`, a template substitution, …): the value must be a primitive, or the conversion
    /// calls its `Symbol.toPrimitive`, `valueOf` or `toString`.
    Primitive,
}

/// One identifier of the expression and what it must be.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperandObligation {
    /// The identifier's span in the file.
    pub span: Span,
    pub name: String,
    pub need: OperandNeed,
}

/// The kind of value an inert expression yields.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ValueKind {
    /// A primitive by construction (a literal, a comparison, `typeof`, an arithmetic result).
    Primitive,
    /// An object made here with no code in it: an array or object literal of data, a regular expression.
    Data,
    /// An object made here that holds code: a function, or a literal with a method or accessor. Creating it runs nothing.
    Code,
    /// Depends on an operand.
    Operand,
}

/// A positive K1 answer: the expression runs no project code once every obligation holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Evaluation {
    pub kind: ValueKind,
    pub obligations: Vec<OperandObligation>,
}

/// K1: whether evaluating the expression at `span` of `source` (a file at `path`, which picks the dialect: TypeScript, JSX) runs no
/// project code, and on what condition. `None` is a refusal: some form is outside the allow-list, or the text is no single expression.
#[must_use]
pub fn evaluation(path: &str, source: &str, span: Span) -> Option<Evaluation> {
    let text = source.get(span.start as usize..span.end as usize)?;
    let source_type = SourceType::from_path(path).ok()?;
    let allocator = Allocator::default();
    let expression = Parser::new(&allocator, text, source_type)
        .parse_expression()
        .ok()?;
    // The whole text must be the expression: `a, b` parses as one sequence, `a) + (b` does not parse.
    let lead = text.len() - text.trim_start().len();
    if expression.span().start as usize != lead
        || expression.span().end as usize != text.trim_end().len()
    {
        return None;
    }
    let mut walk = Walk {
        offset: span.start,
        obligations: Vec::new(),
    };
    let value = walk.expression(&expression)?;
    let kind = match value {
        Value::Kind(kind) => kind,
        Value::Leaves(_) => ValueKind::Operand,
    };
    Some(Evaluation {
        kind,
        obligations: walk.obligations,
    })
}

/// A sub-expression's value: a kind settled by syntax, or the identifiers (indexes into the obligations) it may be.
enum Value {
    Kind(ValueKind),
    Leaves(Vec<usize>),
}

struct Walk {
    offset: u32,
    obligations: Vec<OperandObligation>,
}

impl Walk {
    fn span(&self, span: oxc_span::Span) -> Span {
        Span::new(span.start + self.offset, span.end + self.offset)
    }

    /// Requires `value` to be a primitive: a settled kind must be one, an operand gets the obligation.
    fn primitive(&mut self, value: Value) -> Option<()> {
        match value {
            Value::Kind(ValueKind::Primitive) => Some(()),
            // An object made here converts through `Object.prototype` or `Array.prototype` methods, which the program may have
            // replaced, or through its own code.
            Value::Kind(ValueKind::Data | ValueKind::Code | ValueKind::Operand) => None,
            Value::Leaves(leaves) => {
                for leaf in leaves {
                    self.obligations[leaf].need = OperandNeed::Primitive;
                }
                Some(())
            }
        }
    }

    fn join(left: Value, right: Value) -> Value {
        match (left, right) {
            (Value::Kind(ValueKind::Primitive), Value::Kind(ValueKind::Primitive)) => {
                Value::Kind(ValueKind::Primitive)
            }
            (Value::Leaves(mut a), Value::Leaves(b)) => {
                a.extend(b);
                Value::Leaves(a)
            }
            (Value::Leaves(a), Value::Kind(ValueKind::Primitive))
            | (Value::Kind(ValueKind::Primitive), Value::Leaves(a)) => Value::Leaves(a),
            (Value::Kind(ValueKind::Code), _) | (_, Value::Kind(ValueKind::Code)) => {
                Value::Kind(ValueKind::Code)
            }
            // Mixed: an object or an operand. Never primitive.
            _ => Value::Kind(ValueKind::Operand),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn expression(&mut self, expression: &Expression<'_>) -> Option<Value> {
        match expression {
            Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::StringLiteral(_) => Some(Value::Kind(ValueKind::Primitive)),
            // Creating a regular expression runs the intrinsic constructor's steps, no lookup a program can replace.
            Expression::RegExpLiteral(_) => Some(Value::Kind(ValueKind::Data)),
            // Each substitution goes through ToString.
            Expression::TemplateLiteral(template) => {
                for substitution in &template.expressions {
                    let value = self.expression(substitution)?;
                    self.primitive(value)?;
                }
                Some(Value::Kind(ValueKind::Primitive))
            }
            Expression::Identifier(identifier) => {
                self.obligations.push(OperandObligation {
                    span: self.span(identifier.span),
                    name: identifier.name.to_string(),
                    need: OperandNeed::Bound,
                });
                Some(Value::Leaves(vec![self.obligations.len() - 1]))
            }
            Expression::ParenthesizedExpression(inner) => self.expression(&inner.expression),
            Expression::TSAsExpression(inner) => self.expression(&inner.expression),
            Expression::TSSatisfiesExpression(inner) => self.expression(&inner.expression),
            Expression::TSTypeAssertion(inner) => self.expression(&inner.expression),
            Expression::TSNonNullExpression(inner) => self.expression(&inner.expression),
            Expression::TSInstantiationExpression(inner) => self.expression(&inner.expression),
            Expression::SequenceExpression(sequence) => {
                let mut last = Value::Kind(ValueKind::Primitive);
                for item in &sequence.expressions {
                    last = self.expression(item)?;
                }
                Some(last)
            }
            // ToBoolean of the test calls nothing.
            Expression::ConditionalExpression(conditional) => {
                self.expression(&conditional.test)?;
                let consequent = self.expression(&conditional.consequent)?;
                let alternate = self.expression(&conditional.alternate)?;
                Some(Self::join(consequent, alternate))
            }
            Expression::LogicalExpression(logical) => {
                let left = self.expression(&logical.left)?;
                let right = self.expression(&logical.right)?;
                Some(Self::join(left, right))
            }
            Expression::UnaryExpression(unary) => {
                let operand = self.expression(&unary.argument)?;
                match unary.operator {
                    // Neither looks at the operand's properties or traps.
                    UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void => {}
                    UnaryOperator::UnaryNegation
                    | UnaryOperator::UnaryPlus
                    | UnaryOperator::BitwiseNot => self.primitive(operand)?,
                    // `delete` runs a Proxy's `deleteProperty`, and on a member it is a write.
                    UnaryOperator::Delete => return None,
                }
                Some(Value::Kind(ValueKind::Primitive))
            }
            Expression::BinaryExpression(binary) => {
                let left = self.expression(&binary.left)?;
                let right = self.expression(&binary.right)?;
                match binary.operator {
                    // Strict comparison converts nothing.
                    BinaryOperator::StrictEquality | BinaryOperator::StrictInequality => {}
                    BinaryOperator::Equality
                    | BinaryOperator::Inequality
                    | BinaryOperator::LessThan
                    | BinaryOperator::LessEqualThan
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::GreaterEqualThan
                    | BinaryOperator::Addition
                    | BinaryOperator::Subtraction
                    | BinaryOperator::Multiplication
                    | BinaryOperator::Division
                    | BinaryOperator::Remainder
                    | BinaryOperator::Exponential
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::ShiftRightZeroFill
                    | BinaryOperator::BitwiseOR
                    | BinaryOperator::BitwiseXOR
                    | BinaryOperator::BitwiseAnd => {
                        self.primitive(left)?;
                        self.primitive(right)?;
                    }
                    // `in`: ToPropertyKey of the left, a Proxy's `has` on the right. `instanceof`: `Symbol.hasInstance`, a
                    // `prototype` getter, a Proxy's `getPrototypeOf`.
                    BinaryOperator::In | BinaryOperator::Instanceof => return None,
                }
                Some(Value::Kind(ValueKind::Primitive))
            }
            Expression::ArrayExpression(array) => {
                let mut kind = ValueKind::Data;
                for element in &array.elements {
                    match element {
                        // A spread iterates: `Symbol.iterator`, the iterator's `next`.
                        ArrayExpressionElement::SpreadElement(_) => return None,
                        ArrayExpressionElement::Elision(_) => {}
                        element => {
                            let value = self.expression(element.as_expression()?)?;
                            if matches!(value, Value::Kind(ValueKind::Code)) {
                                kind = ValueKind::Code;
                            } else if !matches!(value, Value::Kind(ValueKind::Primitive | ValueKind::Data)) {
                                kind = Self::worse(kind, ValueKind::Operand);
                            }
                        }
                    }
                }
                Some(Value::Kind(kind))
            }
            Expression::ObjectExpression(object) => {
                let mut kind = ValueKind::Data;
                for property in &object.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        // A spread reads every own property: getters, a Proxy's `ownKeys` and `get`.
                        return None;
                    };
                    // `__proto__: v` sets the prototype (a non-computed, non-shorthand key only), which changes what every later
                    // lookup on the object runs.
                    if !property.computed
                        && !property.shorthand
                        && property_key_value(&property.key).as_deref() == Some("__proto__")
                    {
                        return None;
                    }
                    if property.computed {
                        // A computed key goes through ToPropertyKey: only a literal is safe.
                        let key = property.key.as_expression()?;
                        if !matches!(
                            key.get_inner_expression(),
                            Expression::StringLiteral(_) | Expression::NumericLiteral(_)
                        ) && !matches!(key.get_inner_expression(), Expression::TemplateLiteral(t) if t.expressions.is_empty())
                        {
                            return None;
                        }
                    }
                    if property.method || property.kind != PropertyKind::Init {
                        kind = ValueKind::Code;
                        continue;
                    }
                    let value = self.expression(&property.value)?;
                    match value {
                        Value::Kind(ValueKind::Code) => kind = ValueKind::Code,
                        Value::Kind(ValueKind::Primitive | ValueKind::Data) => {}
                        _ => kind = Self::worse(kind, ValueKind::Operand),
                    }
                }
                Some(Value::Kind(kind))
            }
            // Creating a function runs nothing; what the function does is the caller's question.
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                Some(Value::Kind(ValueKind::Code))
            }
            // A class runs its `extends` expression, computed keys, static fields and static blocks.
            Expression::ClassExpression(_)
            // Calls, construction, tagged templates, member reads (getters, Proxy `get`), JSX (component calls), and every form
            // that writes, suspends or loads.
            | Expression::CallExpression(_)
            | Expression::NewExpression(_)
            | Expression::TaggedTemplateExpression(_)
            | Expression::ChainExpression(_)
            | Expression::StaticMemberExpression(_)
            | Expression::ComputedMemberExpression(_)
            | Expression::PrivateFieldExpression(_)
            | Expression::PrivateInExpression(_)
            | Expression::AssignmentExpression(_)
            | Expression::UpdateExpression(_)
            | Expression::AwaitExpression(_)
            | Expression::YieldExpression(_)
            | Expression::ImportExpression(_)
            | Expression::MetaProperty(_)
            | Expression::Super(_)
            | Expression::ThisExpression(_)
            | Expression::JSXElement(_)
            | Expression::JSXFragment(_)
            | Expression::V8IntrinsicExpression(_) => None,
        }
    }

    const fn worse(kind: ValueKind, other: ValueKind) -> ValueKind {
        match (kind, other) {
            (ValueKind::Code, _) | (_, ValueKind::Code) => ValueKind::Code,
            (ValueKind::Operand, _) | (_, ValueKind::Operand) => ValueKind::Operand,
            _ => ValueKind::Data,
        }
    }
}

/// A property key's value when syntax settles it: a name, a string, a canonical integer, a template with no substitution.
pub(crate) fn property_key_value(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::PrivateIdentifier(_) => None,
        key => literal_key_value(key.as_expression()?),
    }
}

/// The ToPropertyKey value of a literal, or `None` when it is no literal or its canonical string is not settled here.
fn literal_key_value(expression: &Expression<'_>) -> Option<String> {
    match expression.get_inner_expression() {
        Expression::StringLiteral(string) => Some(string.value.to_string()),
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => template
            .quasis
            .first()?
            .value
            .cooked
            .map(|cooked| cooked.to_string()),
        Expression::NumericLiteral(number) => canonical_integer(number.value),
        _ => None,
    }
}

/// Number::toString for a non-negative integer below 2^53 (`0x10` is "16", `1e3` is "1000"). Any other number (a fraction, an
/// exponent form, `-0`) is not settled here.
fn canonical_integer(value: f64) -> Option<String> {
    const LIMIT: f64 = 9_007_199_254_740_992.0;
    if (0.0..LIMIT).contains(&value)
        && value.fract() == 0.0
        && !(value == 0.0 && value.is_sign_negative())
    {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let integer = value as u64;
        return Some(integer.to_string());
    }
    None
}

/// K3: the exact property key that `text`, the source of one property key or member name, denotes: `foo`, `foo`, `"foo"`,
/// `'_\x24\x24click'`, `` `x` ``, `16`, `0x10`, or a bracketed literal `["foo"]`. `None` when the text is no such key (a computed
/// expression, a private name, two keys) or its value is not settled here.
#[must_use]
pub fn cooked_key(text: &str) -> Option<String> {
    let wrapped = format!("({{{text}: 0}})");
    let allocator = Allocator::default();
    let expression = Parser::new(&allocator, &wrapped, SourceType::mjs())
        .parse_expression()
        .ok()?;
    let Expression::ObjectExpression(object) = expression.get_inner_expression() else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(property)] = object.properties.as_slice() else {
        return None;
    };
    if property.shorthand || property.method || property.kind != PropertyKind::Init {
        return None;
    }
    if property.computed {
        return literal_key_value(property.key.as_expression()?);
    }
    property_key_value(&property.key)
}

/// ADR 1133: a key that names no standard function-valued member, prototype route, coercion or protocol hook, Solid private key,
/// shadow-root spelling, nor a name the delivery premise watches (`event`, `Promise`, `__proto__`).
fn plain_data_key(key: &str) -> bool {
    !crate::ast::premise_sites::is_standard_member(key)
        && !crate::ast::premise_sites::PROTOTYPE_ROUTES.contains(&key)
        && !crate::ast::premise_sites::HOOK_KEYS.contains(&key)
        && !crate::ast::premise_sites::SHADOW_SPELLINGS.contains(&key)
        && !key.contains("$$")
        && !key.starts_with("_$")
        && !matches!(key, "event" | "Promise" | "__proto__")
}

/// K4 (ADR 1125): whether the value of the expression at `node` is provably neither called nor kept ([`crate::ast::ComputedKeyFact::value_inert`]).
pub(crate) fn value_inert(
    semantic: &oxc_semantic::Semantic<'_>,
    node: oxc_semantic::NodeId,
) -> bool {
    use oxc_ast::AstKind;
    let nodes = semantic.nodes();
    let mut child = node;
    loop {
        let parent = nodes.parent_id(child);
        if parent == child {
            return false;
        }
        let child_span = nodes.kind(child).span();
        match nodes.kind(parent) {
            // The value passes through: judge the parent's own position.
            AstKind::ParenthesizedExpression(_)
            | AstKind::TSAsExpression(_)
            | AstKind::TSSatisfiesExpression(_)
            | AstKind::TSNonNullExpression(_)
            | AstKind::TSTypeAssertion(_)
            | AstKind::LogicalExpression(_) => child = parent,
            AstKind::ConditionalExpression(conditional) => {
                if conditional.test.span() == child_span {
                    return true;
                }
                child = parent;
            }
            AstKind::UnaryExpression(unary) => return unary.operator != UnaryOperator::Delete,
            AstKind::BinaryExpression(binary) => {
                return binary.operator != BinaryOperator::Instanceof;
            }
            AstKind::IfStatement(statement) => return statement.test.span() == child_span,
            AstKind::WhileStatement(statement) => return statement.test.span() == child_span,
            AstKind::DoWhileStatement(statement) => return statement.test.span() == child_span,
            AstKind::ForStatement(statement) => {
                return statement
                    .test
                    .as_ref()
                    .is_some_and(|test| test.span() == child_span);
            }
            AstKind::ComputedMemberExpression(member) => {
                return member.expression.span() == child_span;
            }
            // ADR 1133 (#296 textbook): the value only as the object of a member read or written under a plain data key
            // (`draft[index()].done = …`), never called: no standard function-valued name, prototype route, hook, private key,
            // shadow spelling or delivery name, so the member can hold no definer or handler that a visible write did not put there.
            AstKind::StaticMemberExpression(member) => {
                if member.object.span() != child_span {
                    return false;
                }
                let key = member.property.name.as_str();
                let called = matches!(
                    nodes.parent_kind(parent),
                    AstKind::CallExpression(call) if call.callee.span() == member.span
                ) || matches!(
                    nodes.parent_kind(parent),
                    AstKind::TaggedTemplateExpression(_)
                );
                return !called && plain_data_key(key);
            }
            // An arrow's expression body is an expression statement in Oxc's tree: its value is the arrow's return (F3).
            AstKind::ExpressionStatement(_) => {
                let body = nodes.parent_id(parent);
                let arrow = nodes.parent_id(body);
                return !matches!(
                    (nodes.kind(body), nodes.kind(arrow)),
                    (AstKind::FunctionBody(_), AstKind::ArrowFunctionExpression(function)) if function.expression
                );
            }
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(text: &str) -> Option<Evaluation> {
        evaluation(
            "a.tsx",
            text,
            Span::new(0, u32::try_from(text.len()).unwrap()),
        )
    }

    fn needs(text: &str) -> Vec<(String, OperandNeed)> {
        eval(text)
            .unwrap()
            .obligations
            .into_iter()
            .map(|o| (o.name, o.need))
            .collect()
    }

    #[test]
    fn literals_and_strict_forms_are_inert_with_bound_operands() {
        assert_eq!(eval("1").unwrap().kind, ValueKind::Primitive);
        assert_eq!(eval("`x`").unwrap().kind, ValueKind::Primitive);
        assert_eq!(
            needs("a === b"),
            vec![
                ("a".into(), OperandNeed::Bound),
                ("b".into(), OperandNeed::Bound)
            ]
        );
        assert_eq!(needs("!a"), vec![("a".into(), OperandNeed::Bound)]);
        assert_eq!(needs("typeof a"), vec![("a".into(), OperandNeed::Bound)]);
        assert_eq!(
            needs("c ? a : 1"),
            vec![
                ("c".into(), OperandNeed::Bound),
                ("a".into(), OperandNeed::Bound)
            ]
        );
        assert_eq!(
            eval("({ a: 1, b: [2, 'x'] })").unwrap().kind,
            ValueKind::Data
        );
        assert_eq!(eval("() => x").unwrap().kind, ValueKind::Code);
        assert_eq!(eval("(x as number)!").unwrap().kind, ValueKind::Operand);
    }

    #[test]
    fn a_coercing_operator_makes_its_operands_primitive_obligations() {
        assert_eq!(needs("a + 1"), vec![("a".into(), OperandNeed::Primitive)]);
        assert_eq!(needs("`${a}`"), vec![("a".into(), OperandNeed::Primitive)]);
        assert_eq!(
            needs("-(c ? a : b)"),
            vec![
                ("c".into(), OperandNeed::Bound),
                ("a".into(), OperandNeed::Primitive),
                ("b".into(), OperandNeed::Primitive)
            ]
        );
        assert_eq!(
            needs("(a === b) + 1"),
            vec![
                ("a".into(), OperandNeed::Bound),
                ("b".into(), OperandNeed::Bound)
            ]
        );
        assert!(
            eval("[] + 1").is_none(),
            "an object made here converts through a replaceable prototype method"
        );
        assert!(eval("({}) + ''").is_none());
    }

    #[test]
    fn every_form_that_may_run_code_refuses() {
        for text in [
            "a.b",
            "a[0]",
            "a?.b",
            "f()",
            "new K()",
            "t`x`",
            "'x' in a",
            "a instanceof K",
            "delete a.b",
            "[...a]",
            "({ ...a })",
            "({ [k]: 1 })",
            "({ __proto__: a })",
            "a = 1",
            "a++",
            "class {}",
            "this",
            "import('x')",
            "<div />",
            "import.meta",
        ] {
            assert!(eval(text).is_none(), "{text}");
        }
        assert!(
            eval("({ ['k']: 1, \"__proto__\": 2 })").is_none(),
            "a quoted __proto__ key still sets the prototype"
        );
        assert!(
            eval("({ ['__proto__']: 1 })").is_some(),
            "a computed __proto__ key defines an own property"
        );
        assert!(eval("a) + (b").is_none());
    }

    #[test]
    fn keys_are_cooked_never_trimmed() {
        assert_eq!(cooked_key("foo").as_deref(), Some("foo"));
        assert_eq!(cooked_key("f\\u006fo").as_deref(), Some("foo"));
        assert_eq!(
            cooked_key("'_\\x24\\x24click'").as_deref(),
            Some("_$$click")
        );
        assert_eq!(cooked_key("\"'useIt'\"").as_deref(), Some("'useIt'"));
        assert_eq!(cooked_key("[`x`]").as_deref(), Some("x"));
        assert_eq!(
            cooked_key("`x`"),
            None,
            "a template is a key only in brackets"
        );
        assert_eq!(cooked_key("0x10").as_deref(), Some("16"));
        assert_eq!(cooked_key("1e3").as_deref(), Some("1000"));
        assert_eq!(cooked_key("[\"a\"]").as_deref(), Some("a"));
        assert_eq!(cooked_key("1.5"), None);
        assert_eq!(cooked_key("[k]"), None);
        assert_eq!(cooked_key("['_$$' + 'click']"), None);
        assert_eq!(cooked_key("a: 0, b"), None);
        assert_eq!(cooked_key("#x"), None);
    }

    /// The kernel oracle's hook (`scripts/kernel-oracle.mjs`): one expression per line of `SOLID_KERNEL_ORACLE_IN`, one JSON verdict per
    /// line of `SOLID_KERNEL_ORACLE_OUT` (`null` is a refusal). Run with `--ignored`; the script judges the verdicts in Node.
    #[test]
    #[ignore = "driven by scripts/kernel-oracle.mjs"]
    fn kernel_oracle() {
        let (Some(input), Some(output)) = (
            std::env::var_os("SOLID_KERNEL_ORACLE_IN"),
            std::env::var_os("SOLID_KERNEL_ORACLE_OUT"),
        ) else {
            return;
        };
        let cases = std::fs::read_to_string(input).unwrap();
        let mut lines = Vec::new();
        for text in cases.lines() {
            let verdict = eval(text).map(|evaluation| {
                serde_json::json!({
                    "kind": format!("{:?}", evaluation.kind),
                    "obligations": evaluation
                        .obligations
                        .iter()
                        .map(|o| serde_json::json!([o.name, format!("{:?}", o.need)]))
                        .collect::<Vec<_>>(),
                })
            });
            lines.push(serde_json::to_string(&verdict).unwrap());
        }
        std::fs::write(output, lines.join("\n") + "\n").unwrap();
    }
}
