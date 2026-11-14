//! What a computed key provably is (facts schema 95, the re-check of #232).
//!
//! A read under a key the program does not spell (`x[k]`, `({ [k]: p } = x)`, `Reflect.get(x, k)`) may be a read of `constructor` (the global `Object` of a plain object),
//! `__proto__` (the prototype) or `prototype`, whatever `k` is at run time: a key built from pieces (`"const" + "ructor"`) is as good a spelling as the word. A consumer that
//! asks whether a program can name `Object.prototype` has to tell the reads whose key it can show is none of those from the rest, and it has to do it from the syntax:
//! a key's type says nothing (an `any`, a cast, a union), so the proofs here are the expression's own, never a type's, and a key nothing proves is listed as unproven.
//!
//! A proof is the set of constant strings a key may be, plus three classes of value that no consumer asks about:
//!
//! - a number or a bigint (a numeric operator, `++`/`--`, a literal);
//! - text that holds a digit, `NaN` or `Infinity` (a number added to anything, a template that converts a number): no key equals it, and a number's text holds one;
//! - another value that is no such key: a symbol (`Symbol(..)` and the well-known ones), a boolean, `undefined`, the text `typeof` gives.
//!
//! A name is its declaration's proof: a `const` is its initializer's, and a `let` or `var` is its initializer's joined with every write's, where each write must be one of the
//! forms below (`=`, an arithmetic compound assignment, `+=`, a logical assignment, `++`/`--`). A write by a destructuring, by a `for in` or `for of`, a parameter, an import, a
//! catch clause, a declaration with a pattern, a redeclared name and an ambient one prove nothing. The proof is flow insensitive and conservative: a name read before it is
//! initialized is `undefined` (no key of interest) or throws, and a cycle (`i = i + 1`) is read as unproven where it matters (a number added to anything is a number or text
//! holding a digit, so `i + 1` needs no proof of `i`).

use std::collections::{BTreeSet, HashMap, HashSet};

use compact_str::CompactString;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, AssignmentTarget, BindingPattern, CallExpression, Expression, IdentifierReference,
    TemplateLiteral, VariableDeclarationKind,
};
use oxc_semantic::{NodeId, Semantic, SymbolId};
use oxc_span::GetSpan;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};

use super::extract_helpers::{literal_property_key, peel_ts_sugar};
use super::{ComputedKeyFact, ComputedKeySite};
use crate::core::Span;

/// How many constant strings a proof may carry before it is no proof (a chain of conditionals and concatenations).
const MAX_TEXTS: usize = 16;
/// How deep an expression is followed (nesting and the names it reads).
const MAX_DEPTH: usize = 24;

/// What a key may be: one of `texts`, a number, text that holds a digit, or another value no consumer asks about.
#[derive(Clone, Debug, Default)]
struct Proof {
    texts: BTreeSet<String>,
    number: bool,
    digit: bool,
    other: bool,
}

impl Proof {
    fn number() -> Self {
        Self {
            number: true,
            ..Self::default()
        }
    }

    fn other() -> Self {
        Self {
            other: true,
            ..Self::default()
        }
    }

    fn text(text: &str) -> Self {
        Self {
            texts: BTreeSet::from([text.to_owned()]),
            ..Self::default()
        }
    }

    fn digit() -> Self {
        Self {
            digit: true,
            ..Self::default()
        }
    }

    /// Every value is a number or a bigint.
    fn is_number(&self) -> bool {
        self.texts.is_empty() && self.number && !self.digit && !self.other
    }

    /// Every value is a number, or text that holds a digit: whatever is added to it (a `+` that is no sum of numbers) is text that holds that digit still.
    fn is_digit_bearing(&self) -> bool {
        self.texts.is_empty() && (self.number || self.digit) && !self.other
    }

    /// Every value is one of the texts.
    fn is_text(&self) -> bool {
        !self.texts.is_empty() && !self.number && !self.digit && !self.other
    }

    /// What either of two proofs allows; `None` past [`MAX_TEXTS`].
    fn join(mut self, other: Self) -> Option<Self> {
        self.texts.extend(other.texts);
        self.number |= other.number;
        self.digit |= other.digit;
        self.other |= other.other;
        (self.texts.len() <= MAX_TEXTS).then_some(self)
    }
}

/// The proof of `left + right` (also a template's concatenation).
fn plus(left: Option<Proof>, right: Option<Proof>) -> Option<Proof> {
    let number = |proof: &Option<Proof>| proof.as_ref().is_some_and(Proof::is_number);
    let digit = |proof: &Option<Proof>| proof.as_ref().is_some_and(Proof::is_digit_bearing);
    if number(&left) && number(&right) {
        return Some(Proof::number());
    }
    // A number's text holds a digit, `NaN` or `Infinity`, and text that holds a digit still does with anything beside it: no key a consumer asks about is equal to it.
    if digit(&left) || digit(&right) {
        return Some(Proof::digit());
    }
    let (left, right) = (left?, right?);
    if !left.is_text() || !right.is_text() || left.texts.len() * right.texts.len() > MAX_TEXTS {
        return None;
    }
    let texts = left
        .texts
        .iter()
        .flat_map(|head| right.texts.iter().map(move |tail| format!("{head}{tail}")))
        .collect::<BTreeSet<_>>();
    Some(Proof {
        texts,
        ..Proof::default()
    })
}

/// The value of an assignment to a name or a key, for an operator that gives it a value of its own; the value a `||=`, `&&=` or `??=` leaves is the old one, which the caller
/// joins in where it knows it (a name's proof), and is unproven where it does not.
fn compound(operator: AssignmentOperator, right: Option<Proof>) -> Option<Proof> {
    match operator {
        AssignmentOperator::Assign
        | AssignmentOperator::LogicalAnd
        | AssignmentOperator::LogicalOr
        | AssignmentOperator::LogicalNullish => right,
        AssignmentOperator::Addition => plus(None, right),
        AssignmentOperator::Subtraction
        | AssignmentOperator::Multiplication
        | AssignmentOperator::Division
        | AssignmentOperator::Remainder
        | AssignmentOperator::Exponential
        | AssignmentOperator::ShiftLeft
        | AssignmentOperator::ShiftRight
        | AssignmentOperator::ShiftRightZeroFill
        | AssignmentOperator::BitwiseOR
        | AssignmentOperator::BitwiseXOR
        | AssignmentOperator::BitwiseAnd => Some(Proof::number()),
    }
}

struct Prover<'s, 'a> {
    semantic: &'s Semantic<'a>,
    /// Whether names are followed to their declarations: false when the binder reported errors, where a name's declaration is no fact.
    resolve: bool,
    symbols: HashMap<SymbolId, Option<Proof>>,
    in_progress: HashSet<SymbolId>,
}

impl<'s, 'a> Prover<'s, 'a> {
    fn expression(&mut self, expression: &'a Expression<'a>, depth: usize) -> Option<Proof> {
        if depth > MAX_DEPTH {
            return None;
        }
        match peel_ts_sugar(expression) {
            Expression::NumericLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::UpdateExpression(_) => Some(Proof::number()),
            // Text with a lone surrogate is no text a key of interest can be (each is ASCII).
            Expression::StringLiteral(literal) if literal.lone_surrogates => Some(Proof::other()),
            Expression::StringLiteral(literal) => Some(Proof::text(literal.value.as_str())),
            Expression::TemplateLiteral(template) => self.template(template, depth),
            Expression::UnaryExpression(unary) => Some(match unary.operator {
                UnaryOperator::UnaryPlus
                | UnaryOperator::UnaryNegation
                | UnaryOperator::BitwiseNot => Proof::number(),
                // `!`, `typeof`, `void` and `delete` give a boolean, one of eight fixed words or `undefined`.
                UnaryOperator::LogicalNot
                | UnaryOperator::Typeof
                | UnaryOperator::Void
                | UnaryOperator::Delete => Proof::other(),
            }),
            Expression::BinaryExpression(binary) => match binary.operator {
                BinaryOperator::Addition => {
                    let left = self.expression(&binary.left, depth + 1);
                    let right = self.expression(&binary.right, depth + 1);
                    plus(left, right)
                }
                BinaryOperator::Subtraction
                | BinaryOperator::Multiplication
                | BinaryOperator::Division
                | BinaryOperator::Remainder
                | BinaryOperator::Exponential
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::ShiftRightZeroFill
                | BinaryOperator::BitwiseOR
                | BinaryOperator::BitwiseXOR
                | BinaryOperator::BitwiseAnd => Some(Proof::number()),
                // The comparisons, `in` and `instanceof` give a boolean.
                BinaryOperator::Equality
                | BinaryOperator::Inequality
                | BinaryOperator::StrictEquality
                | BinaryOperator::StrictInequality
                | BinaryOperator::LessThan
                | BinaryOperator::LessEqualThan
                | BinaryOperator::GreaterThan
                | BinaryOperator::GreaterEqualThan
                | BinaryOperator::In
                | BinaryOperator::Instanceof => Some(Proof::other()),
            },
            Expression::LogicalExpression(logical) => {
                let left = self.expression(&logical.left, depth + 1)?;
                let right = self.expression(&logical.right, depth + 1)?;
                left.join(right)
            }
            Expression::ConditionalExpression(conditional) => {
                let consequent = self.expression(&conditional.consequent, depth + 1)?;
                let alternate = self.expression(&conditional.alternate, depth + 1)?;
                consequent.join(alternate)
            }
            Expression::SequenceExpression(sequence) => {
                let last = sequence.expressions.last()?;
                self.expression(last, depth + 1)
            }
            Expression::AssignmentExpression(assignment) => match assignment.operator {
                // The old value is the other possibility of a logical assignment, and nothing here says what it is.
                AssignmentOperator::LogicalAnd
                | AssignmentOperator::LogicalOr
                | AssignmentOperator::LogicalNullish => None,
                operator => {
                    let right = self.expression(&assignment.right, depth + 1);
                    compound(operator, right)
                }
            },
            Expression::CallExpression(call) if self.is_symbol_call(call) => Some(Proof::other()),
            Expression::StaticMemberExpression(member)
                if self.is_unresolved(&member.object, "Symbol")
                    && matches!(
                        member.property.name.as_str(),
                        "asyncIterator"
                            | "hasInstance"
                            | "isConcatSpreadable"
                            | "iterator"
                            | "match"
                            | "matchAll"
                            | "replace"
                            | "search"
                            | "species"
                            | "split"
                            | "toPrimitive"
                            | "toStringTag"
                            | "unscopables"
                            | "dispose"
                            | "asyncDispose"
                    ) =>
            {
                Some(Proof::other())
            }
            Expression::Identifier(identifier) => self.identifier(identifier, depth),
            _ => None,
        }
    }

    /// `Symbol(..)` and `Symbol.for(..)` of the global `Symbol`.
    fn is_symbol_call(&self, call: &'a CallExpression<'a>) -> bool {
        match peel_ts_sugar(&call.callee) {
            callee @ Expression::Identifier(_) => self.is_unresolved(callee, "Symbol"),
            Expression::StaticMemberExpression(member) => {
                member.property.name == "for" && self.is_unresolved(&member.object, "Symbol")
            }
            _ => false,
        }
    }

    /// Whether the expression is the identifier `name` that no binding of the file declares.
    fn is_unresolved(&self, expression: &'a Expression<'a>, name: &str) -> bool {
        matches!(
            peel_ts_sugar(expression),
            Expression::Identifier(identifier)
                if identifier.name == name && self.symbol_of(identifier).is_none()
        )
    }

    fn symbol_of(&self, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
        identifier
            .reference_id
            .get()
            .and_then(|reference| self.semantic.scoping().get_reference(reference).symbol_id())
    }

    /// A template's text: the concatenation of its parts, each substitution converted with `ToString`.
    fn template(&mut self, template: &'a TemplateLiteral<'a>, depth: usize) -> Option<Proof> {
        let mut proof = Some(Proof::text(""));
        for (index, quasi) in template.quasis.iter().enumerate() {
            let cooked = quasi.value.cooked.as_ref()?;
            proof = plus(proof, Some(Proof::text(cooked.as_str())));
            if let Some(expression) = template.expressions.get(index) {
                let part = self.expression(expression, depth + 1);
                proof = plus(proof, part);
            }
        }
        proof
    }

    fn identifier(
        &mut self,
        identifier: &'a IdentifierReference<'a>,
        depth: usize,
    ) -> Option<Proof> {
        match self.symbol_of(identifier) {
            None => match identifier.name.as_str() {
                // The globals that are values no key of interest is: nothing declares them, and nothing here follows a write to one.
                "undefined" => Some(Proof::other()),
                "NaN" | "Infinity" => Some(Proof::number()),
                _ => None,
            },
            Some(symbol) if self.resolve => self.symbol(symbol, depth),
            Some(_) => None,
        }
    }

    /// A name's proof: see the module's text.
    fn symbol(&mut self, symbol: SymbolId, depth: usize) -> Option<Proof> {
        if let Some(known) = self.symbols.get(&symbol) {
            return known.clone();
        }
        // A cycle through this name is read as unproven (not recorded: the name's own answer is the one asked for).
        if !self.in_progress.insert(symbol) {
            return None;
        }
        let answer = self.declared_proof(symbol, depth);
        self.in_progress.remove(&symbol);
        self.symbols.insert(symbol, answer.clone());
        answer
    }

    fn declared_proof(&mut self, symbol: SymbolId, depth: usize) -> Option<Proof> {
        let semantic = self.semantic;
        let (scoping, nodes) = (semantic.scoping(), semantic.nodes());
        if !scoping.symbol_redeclarations(symbol).is_empty() {
            return None;
        }
        let declarator_id = scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(declarator) = nodes.kind(declarator_id) else {
            return None;
        };
        let BindingPattern::BindingIdentifier(_) = &declarator.id else {
            return None;
        };
        let AstKind::VariableDeclaration(declaration) = nodes.parent_kind(declarator_id) else {
            return None;
        };
        // An ambient name has no value, and the name a `for in` or `for of` declares holds each key or element.
        if declaration.declare
            || matches!(
                nodes.parent_kind(nodes.parent_id(declarator_id)),
                AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
            )
        {
            return None;
        }
        // A `let` or `var` without an initializer is `undefined` until a write.
        let mut proof = match &declarator.init {
            Some(init) => self.expression(init, depth + 1)?,
            None => Proof::other(),
        };
        if declaration.kind == VariableDeclarationKind::Const {
            return Some(proof);
        }
        // The writes are read in two passes: every write that gives a value of its own first, then each `+=`, whose value depends on what the name holds, which is any of them (it
        // is evaluated against the proof of the rest, and a text is never folded through it: an update that repeats has no bound).
        let mut appends = Vec::new();
        for reference in scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            match self.write(reference.node_id(), depth)? {
                Write::Value(written) => proof = proof.join(written)?,
                Write::Append(right) => appends.push(right),
            }
        }
        for right in appends {
            let appended = if proof.is_digit_bearing()
                || right.as_ref().is_some_and(Proof::is_digit_bearing)
            {
                Proof::digit()
            } else {
                return None;
            };
            proof = proof.join(appended)?;
        }
        Some(proof)
    }

    /// What a write to a name does, for the reference node the write is at: an update, or an assignment whose whole left side is the name.
    fn write(&mut self, node: NodeId, depth: usize) -> Option<Write> {
        let semantic = self.semantic;
        let nodes = semantic.nodes();
        match nodes.parent_kind(node) {
            AstKind::UpdateExpression(_) => Some(Write::Value(Proof::number())),
            AstKind::AssignmentExpression(assignment) => {
                let whole_left = matches!(
                    &assignment.left,
                    AssignmentTarget::AssignmentTargetIdentifier(left)
                        if left.span == nodes.kind(node).span()
                );
                if !whole_left {
                    return None;
                }
                let right = self.expression(&assignment.right, depth + 1);
                if assignment.operator == AssignmentOperator::Addition {
                    return Some(Write::Append(right));
                }
                compound(assignment.operator, right).map(Write::Value)
            }
            _ => None,
        }
    }
}

/// What a write to a name does to its proof.
enum Write {
    /// The name holds this after the write, whatever it held.
    Value(Proof),
    /// `name += right`: the name holds what it held plus `right`.
    Append(Option<Proof>),
}

fn span(span: oxc_span::Span) -> Span {
    Span::new(span.start, span.end)
}

/// Whether the call is `Reflect.get(..)` (or `Reflect["get"](..)`) of the global `Reflect`: a read of the key it is given.
fn is_reflect_get<'a>(prover: &Prover<'_, 'a>, call: &'a CallExpression<'a>) -> bool {
    match peel_ts_sugar(&call.callee) {
        Expression::StaticMemberExpression(member) => {
            member.property.name == "get" && prover.is_unresolved(&member.object, "Reflect")
        }
        Expression::ComputedMemberExpression(member) => {
            literal_property_key(&member.expression).is_some_and(|key| key == "get")
                && prover.is_unresolved(&member.object, "Reflect")
        }
        _ => false,
    }
}

/// Lists a read unless its key is proven to be a number or another value no key of interest is.
fn push_fact(
    facts: &mut Vec<ComputedKeyFact>,
    site: ComputedKeySite,
    read: oxc_span::Span,
    proof: Option<Proof>,
) {
    let (proven, texts) = match proof {
        Some(proof) if proof.texts.is_empty() => return,
        Some(proof) => (
            true,
            proof
                .texts
                .iter()
                .map(|text| CompactString::from(text.as_str()))
                .collect(),
        ),
        None => (false, Vec::new()),
    };
    facts.push(ComputedKeyFact {
        read: span(read),
        site,
        proven,
        texts,
    });
}

/// Every read under a computed key that nothing proves is a number, a symbol or a plain non-text value, in source order of the read: see [`ComputedKeyFact`].
pub(super) fn computed_keys(semantic: &Semantic<'_>, resolve: bool) -> Vec<ComputedKeyFact> {
    let mut prover = Prover {
        semantic,
        resolve,
        symbols: HashMap::new(),
        in_progress: HashSet::new(),
    };
    let mut facts = Vec::new();
    for node in semantic.nodes().iter() {
        match node.kind() {
            // A literal key (`x["k"]`, `x[0]`) is spelled: the facts' own `literal_computed_members`.
            AstKind::ComputedMemberExpression(member)
                if literal_property_key(&member.expression).is_none() =>
            {
                let proof = prover.expression(&member.expression, 0);
                push_fact(&mut facts, ComputedKeySite::Member, member.span, proof);
            }
            AstKind::BindingProperty(property) if property.computed => {
                if let Some(key) = property.key.as_expression() {
                    let proof = prover.expression(key, 0);
                    push_fact(&mut facts, ComputedKeySite::Pattern, property.span, proof);
                }
            }
            AstKind::AssignmentTargetPropertyProperty(property) if property.computed => {
                if let Some(key) = property.name.as_expression() {
                    let proof = prover.expression(key, 0);
                    push_fact(&mut facts, ComputedKeySite::Pattern, property.span, proof);
                }
            }
            AstKind::CallExpression(call) if is_reflect_get(&prover, call) => {
                // `Reflect.get(target)` reads the key `undefined`; a spread among the first two arguments may carry any key.
                let spread = call
                    .arguments
                    .iter()
                    .take(2)
                    .any(|argument| matches!(argument, Argument::SpreadElement(_)));
                let proof = match call.arguments.get(1) {
                    _ if spread => None,
                    None => continue,
                    Some(argument) => argument
                        .as_expression()
                        .and_then(|key| prover.expression(key, 0)),
                };
                push_fact(&mut facts, ComputedKeySite::ReflectGet, call.span, proof);
            }
            _ => {}
        }
    }
    facts.sort_by_key(|fact| (fact.read, fact.site as u8));
    facts
}
