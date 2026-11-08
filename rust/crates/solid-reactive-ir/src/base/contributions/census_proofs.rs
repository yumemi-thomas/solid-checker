//! ADR 0903 (ADR 0840 stage 3): the proofs that a census site puts no
//! function at a census member. Syntax only: a key that names no census
//! member, `prototype`, `__proto__` or host key, a value that is no function,
//! and a copy of a literal's own members. A proof may lean on standard
//! functions ([`TRUSTED`]); the site then raises them, and a program that
//! patches one reopens it as a key no fact names.

use solid_facts::FileFacts;
use solid_facts::ast::{AssignmentFact, BindingShape};
use solid_facts::core::Span;

use crate::HashMap;

use super::{HOST_KEYS, IMPOSTOR_MEMBERS};

/// The census key of a site whose proof trusts a standard function: no member
/// while the program leaves each function it names ([`TRUSTED`]) alone.
pub(super) const TRUST_SITE: &str = "@@trust";

/// The standard functions a proof may trust, each with its census key. A
/// number comes out of the first group whatever the argument (ToNumber, or a
/// throw); `new Array(…)` and `new Object()` make no function; `Object.keys`
/// and `Object.entries` list own string keys.
pub(super) const TRUSTED: [(&str, &str); 38] = [
    ("Math.abs", "@@trust:Math.abs"),
    ("Math.ceil", "@@trust:Math.ceil"),
    ("Math.floor", "@@trust:Math.floor"),
    ("Math.log2", "@@trust:Math.log2"),
    ("Math.max", "@@trust:Math.max"),
    ("Math.min", "@@trust:Math.min"),
    ("Math.pow", "@@trust:Math.pow"),
    ("Math.random", "@@trust:Math.random"),
    ("Math.round", "@@trust:Math.round"),
    ("Math.sign", "@@trust:Math.sign"),
    ("Math.sqrt", "@@trust:Math.sqrt"),
    ("Math.trunc", "@@trust:Math.trunc"),
    ("Number", "@@trust:Number"),
    ("Number.parseFloat", "@@trust:Number.parseFloat"),
    ("Number.parseInt", "@@trust:Number.parseInt"),
    ("parseFloat", "@@trust:parseFloat"),
    ("parseInt", "@@trust:parseInt"),
    ("Object.keys", "@@trust:Object.keys"),
    ("Object.entries", "@@trust:Object.entries"),
    ("Array", "@@trust:Array"),
    ("Object", "@@trust:Object"),
    ("Map", "@@trust:Map"),
    ("Set", "@@trust:Set"),
    ("WeakMap", "@@trust:WeakMap"),
    ("WeakSet", "@@trust:WeakSet"),
    ("Date", "@@trust:Date"),
    ("RegExp", "@@trust:RegExp"),
    ("Promise", "@@trust:Promise"),
    ("URL", "@@trust:URL"),
    ("URLSearchParams", "@@trust:URLSearchParams"),
    ("Error", "@@trust:Error"),
    ("TypeError", "@@trust:TypeError"),
    ("RangeError", "@@trust:RangeError"),
    ("SyntaxError", "@@trust:SyntaxError"),
    ("ReferenceError", "@@trust:ReferenceError"),
    ("EvalError", "@@trust:EvalError"),
    ("URIError", "@@trust:URIError"),
    ("AggregateError", "@@trust:AggregateError"),
];

/// The constructors whose `new` makes no function whatever its arguments, under
/// a key that names a member (review D35 A3): not `Proxy`, `Object` with an
/// argument, or `Function`. Under a key no fact names the value may become a
/// prototype, so only `Array` and an empty `Object` (A1).
const NONCALLABLE_NEW: [&str; 18] = [
    "Array",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "Date",
    "RegExp",
    "Promise",
    "URL",
    "URLSearchParams",
    "Error",
    "TypeError",
    "RangeError",
    "SyntaxError",
    "ReferenceError",
    "EvalError",
    "URIError",
    "AggregateError",
];

/// The keys a template or concatenation must not spell (review D35 A1).
fn affix_names() -> impl Iterator<Item = &'static str> {
    IMPOSTOR_MEMBERS
        .iter()
        .chain(&HOST_KEYS)
        .chain(&["prototype", "__proto__"])
        .copied()
}

/// What a proof leaned on: the trusted functions it named.
pub(super) type Trust = Vec<&'static str>;

/// The proofs of one file. Counters are asked once per declaration.
pub(super) struct Proofs<'f> {
    file: &'f FileFacts,
    counters: HashMap<Span, Option<Trust>>,
    writes: Option<HashMap<Span, Vec<Span>>>,
    assignments: Option<HashMap<Span, &'f AssignmentFact>>,
}

/// How deep a key's chain is followed (a `const` naming a `const`, nested
/// operators). Past it the key is unproved.
const MAX_DEPTH: u32 = 12;

impl<'f> Proofs<'f> {
    pub(super) fn new(file: &'f FileFacts) -> Self {
        Self {
            file,
            counters: HashMap::default(),
            writes: None,
            assignments: None,
        }
    }

    /// ADR 0903 S3a, S3c: whether the computed key at `key` names no census
    /// member, `prototype`, `__proto__` or host key, and the functions that
    /// proof trusts. A number, by its syntax (tiers 1-4, 6); or a template or
    /// a concatenation whose literal head or tail no such name has.
    pub(super) fn key(&mut self, key: Span) -> Option<Trust> {
        let mut trust = Vec::new();
        (self.number(key, &mut trust, &mut Vec::new(), 0) || self.affixed(key, false))
            .then_some(trust)
    }

    /// A writer key must exclude trusted global/member names too (R65).
    pub(super) fn trust_key(&mut self, key: Span) -> Option<Trust> {
        let mut trust = Vec::new();
        (self.number(key, &mut trust, &mut Vec::new(), 0) || self.affixed(key, true))
            .then_some(trust)
    }

    /// Whether the value at `span` is a number, a bigint or a boolean, with no
    /// TypeScript assertion on the way: each stringifies to a name outside the
    /// census. `assumed` holds the counters being proved.
    fn number(
        &mut self,
        span: Span,
        trust: &mut Trust,
        assumed: &mut Vec<Span>,
        depth: u32,
    ) -> bool {
        if depth > MAX_DEPTH {
            return false;
        }
        let file = self.file;
        let ast = &file.ast;
        let Some(span) = peel_parens(file, span) else {
            return false;
        };
        let Some(text) = file.source_text(span).map(str::trim) else {
            return false;
        };
        // Tier 1: a numeric literal (a bigint's too).
        if text.starts_with(|c: char| c.is_ascii_digit())
            || (text.starts_with('.') && text[1..].starts_with(|c: char| c.is_ascii_digit()))
        {
            return true;
        }
        // Tiers 2, 3: an operator. Every one but `+` yields a number, a bigint
        // or a boolean (`typeof` a fixed type name), whatever its operands.
        if ast.primitive_results.binary_search(&span).is_ok() {
            return match operator(file, span) {
                Operator::Plus(left, right) => {
                    self.number(left, trust, assumed, depth + 1)
                        && self.number(right, trust, assumed, depth + 1)
                }
                Operator::Other => true,
                Operator::Unread => false,
            };
        }
        // Tier 6: `.length` of an array or string, own and fixed.
        if let Some(member) = ast.member_at(span) {
            return ast.computed_members.binary_search(&member.span).is_err()
                && file.source_text(member.property) == Some("length")
                && fixed_length(file, member.object);
        }
        // Tier 6: a standard function that answers a number.
        if let Some(call) = ast.call_at(span) {
            let Some(name) = standard_callee(file, call.callee) else {
                return false;
            };
            if call.construct || !numeric_function(name) {
                return false;
            }
            trust.push(name);
            return true;
        }
        let Some(declaration) = ast.reference_declaration(span) else {
            return false;
        };
        if assumed.contains(&declaration) {
            return true;
        }
        let Some(binding) = ast.identifier_binding(declaration) else {
            return false;
        };
        if binding.shape != BindingShape::Identifier {
            return false;
        }
        let Some(initializer) = binding.initializer else {
            return false;
        };
        if binding.immutable {
            return self.number(initializer, trust, assumed, depth + 1);
        }
        // Tier 4: a counter.
        binding.runtime_let && self.counter(declaration, initializer, trust, assumed, depth)
    }

    /// Tier 4 (review D35 A4): a `let` of a module, or of a function, whose
    /// initializer and every write is a number. No other script can write it,
    /// and an `eval` in the file could, unseen.
    fn counter(
        &mut self,
        declaration: Span,
        initializer: Span,
        trust: &mut Trust,
        assumed: &mut Vec<Span>,
        depth: u32,
    ) -> bool {
        let file = self.file;
        if let Some(known) = self.counters.get(&declaration) {
            if let Some(leaned) = known {
                trust.extend(leaned);
            }
            return known.is_some();
        }
        if !(crate::execution_role::file_is_module(file)
            || file
                .ast
                .innermost_function_containing(declaration)
                .is_some())
            || crate::interproc::property_gets::direct_eval_in(file, Span::new(0, u32::MAX))
        {
            return false;
        }
        let top = assumed.is_empty();
        let mut leaned = Vec::new();
        assumed.push(declaration);
        let mut proved = self.number(initializer, &mut leaned, assumed, depth + 1);
        let targets = self.writes_of(declaration);
        for target in targets {
            if !proved {
                break;
            }
            proved = match self.assignment_of(target) {
                None => false,
                Some(assignment) if assignment.value_span.contains(target) => true,
                Some(assignment) if assignment.reads_target && assignment.logical.is_none() => {
                    match compound_operator(file, assignment) {
                        Some("+=") => {
                            self.number(assignment.value_span, &mut leaned, assumed, depth + 1)
                        }
                        Some(_) => true,
                        None => false,
                    }
                }
                Some(assignment) => {
                    self.number(assignment.value_span, &mut leaned, assumed, depth + 1)
                }
            };
        }
        assumed.pop();
        if top {
            self.counters
                .insert(declaration, proved.then(|| leaned.clone()));
        }
        if proved {
            trust.extend(leaned);
        }
        proved
    }

    /// The assignment whose target, past TypeScript wrappers, is `target`.
    fn assignment_of(&mut self, target: Span) -> Option<&'f AssignmentFact> {
        let ast = &self.file.ast;
        self.assignments
            .get_or_insert_with(|| {
                ast.assignments
                    .iter()
                    .map(|assignment| (ast.peel_ts_sugar_span(assignment.target), assignment))
                    .collect()
            })
            .get(&target)
            .copied()
    }

    /// The write targets of the binding declared at `declaration`.
    fn writes_of(&mut self, declaration: Span) -> Vec<Span> {
        let ast = &self.file.ast;
        self.writes
            .get_or_insert_with(|| {
                let mut writes = HashMap::<Span, Vec<Span>>::default();
                for target in &ast.write_targets {
                    if let Some(declared) = ast.reference_declaration(*target) {
                        writes.entry(declared).or_default().push(*target);
                    }
                }
                writes
            })
            .get(&declaration)
            .cloned()
            .unwrap_or_default()
    }

    /// S3c: a template, or a `+` chain, with a literal head no listed name
    /// starts with, or a literal tail none ends with.
    fn affixed(&self, key: Span, include_trusted: bool) -> bool {
        let file = self.file;
        let Some(key) = peel_parens(file, key) else {
            return false;
        };
        let (head, tail) = if let Some(template) = file
            .ast
            .template_literals
            .iter()
            .find(|template| template.span == key && !template.expressions.is_empty())
        {
            template_affixes(file, template.span, &template.expressions)
        } else {
            (self.leaf(key, true), self.leaf(key, false))
        };
        let clear = |affix: Option<&str>, starts: bool| {
            affix.is_some_and(|affix| {
                !affix.is_empty()
                    && affix_names()
                        .chain(
                            TRUSTED
                                .iter()
                                .flat_map(|(name, _)| name.split('.'))
                                .filter(|_| include_trusted),
                        )
                        .all(|name| {
                            if starts {
                                !name.starts_with(affix)
                            } else {
                                !name.ends_with(affix)
                            }
                        })
            })
        };
        clear(head.as_deref(), true) || clear(tail.as_deref(), false)
    }

    /// The string literal at the left (or right) end of a `+` chain at `span`.
    fn leaf(&self, span: Span, left: bool) -> Option<String> {
        let file = self.file;
        let mut at = span;
        let mut chained = false;
        for _ in 0..MAX_DEPTH {
            at = peel_parens(file, at)?;
            if file.ast.primitive_results.binary_search(&at).is_err() {
                break;
            }
            let Operator::Plus(l, r) = operator(file, at) else {
                return None;
            };
            chained = true;
            at = if left { l } else { r };
        }
        if !chained {
            return None;
        }
        let text = file.source_text(peel_parens(file, at)?)?.trim();
        string_literal(text).map(str::to_owned)
    }

    /// ADR 0903 S3b: whether the value at `span` is no function, and what that
    /// leans on. `open` when the key may be `__proto__` or `prototype`: the
    /// value may then become a prototype, so a `new` is only `Array` or an
    /// empty `Object` (review D35 A1), and an object literal only one whose
    /// every member is data that passes this test, with no `__proto__` entry
    /// (review of D35).
    pub(super) fn value(&self, span: Span, open: bool) -> Option<Trust> {
        self.value_at(span, open, 0)
    }

    fn value_at(&self, span: Span, open: bool, depth: u32) -> Option<Trust> {
        if depth > 4 {
            return None;
        }
        let file = self.file;
        let ast = &file.ast;
        let value = ast.peel_ts_sugar_span(span);
        if ast.array_literals.binary_search(&value).is_ok()
            || ast.primitive_results.binary_search(&value).is_ok()
            || ast
                .template_literals
                .iter()
                .any(|template| template.span == value)
        {
            return Some(Vec::new());
        }
        if let Some(call) = ast.call_at(value) {
            let name = standard_callee(file, call.callee)?;
            let made = call.construct
                && if open {
                    name == "Array" || (name == "Object" && call.arguments.is_empty())
                } else {
                    NONCALLABLE_NEW.contains(&name)
                        || (name == "Object" && call.arguments.is_empty())
                };
            return made.then(|| vec![name]);
        }
        let text = file.source_text(value).map(str::trim)?;
        if text.starts_with('{') {
            return if open {
                self.inert_literal(value, depth)
            } else {
                Some(Vec::new())
            };
        }
        (text.starts_with(['\'', '"'])
            || text.starts_with(|c: char| c.is_ascii_digit())
            || matches!(text, "true" | "false" | "null")
            || (text == "undefined" && ast.reference_declaration(value).is_none()))
        .then(Vec::new)
    }

    /// An object literal at `span` that may become a prototype and still holds
    /// no function: closed (no spread, computed key, `__proto__` entry or
    /// setter), with no getter or method, each member's value inert.
    fn inert_literal(&self, span: Span, depth: u32) -> Option<Trust> {
        let ast = &self.file.ast;
        let shape = ast
            .object_get_shapes
            .iter()
            .find(|shape| shape.span == span)?;
        if !shape.closed
            || shape.properties.len() > 32
            || shape
                .properties
                .iter()
                .any(|property| property.getter.is_some())
        {
            return None;
        }
        let nested = |at: Span| {
            ast.object_get_shapes.iter().any(|inner| {
                inner.span != span && span.contains(inner.span) && inner.span.contains(at)
            })
        };
        let members = ast
            .object_properties
            .iter()
            .filter(|property| span.contains(property.span) && !nested(property.span))
            .collect::<Vec<_>>();
        if members.len() != shape.properties.len() {
            return None;
        }
        let mut trust = Vec::new();
        for member in members {
            if !member.data || member.method || member.computed {
                return None;
            }
            trust.extend(self.value_at(member.value, true, depth + 1)?);
        }
        Some(trust)
    }

    /// ADR 0903 S3c (review D35 A2): a copy of a literal's own member under
    /// its own key: `d[k] = s[k]` in `for (const k of Object.keys(s))`, or
    /// `d[k] = v` in `for (const [k, v] of Object.entries(s))`, with `s` a
    /// `const` of this file bound to a closed object literal. Each member of
    /// `s` is a census site of its own, so the copy puts no function at a
    /// member where none was.
    pub(super) fn copy(&self, key: Span, value: Span) -> Option<Trust> {
        let file = self.file;
        let ast = &file.ast;
        let key = ast.peel_ts_sugar_span(key);
        let value = ast.peel_ts_sugar_span(value);
        let declaration = ast.reference_declaration(key)?;
        let binding = ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == declaration))?;
        if !binding.immutable || binding.initializer.is_some() {
            return None;
        }
        let iterated = loop_source(file, binding.pattern)?;
        let call = ast.call_at(ast.peel_ts_sugar_span(iterated))?;
        let name = standard_callee(file, call.callee)?;
        let source = call.arguments.first().filter(|argument| !argument.spread)?;
        let source = ast.reference_declaration(ast.peel_ts_sugar_span(source.span))?;
        if call.construct || call.arguments.len() != 1 || !own_data(file, source) {
            return None;
        }
        let copied = match (name, binding.shape) {
            ("Object.keys", BindingShape::Identifier) => {
                ast.computed_members.binary_search(&value).is_ok()
                    && ast.member_at(value).is_some_and(|member| {
                        ast.reference_declaration(ast.peel_ts_sugar_span(member.property))
                            == Some(declaration)
                            && ast.reference_declaration(ast.peel_ts_sugar_span(member.object))
                                == Some(source)
                    })
            }
            ("Object.entries", BindingShape::Array) => {
                matches!(binding.array_slots.as_slice(), [Some(k), Some(v)]
                    if k.span == declaration
                        && ast.reference_declaration(value) == Some(v.span))
            }
            _ => false,
        };
        copied.then(|| vec![name])
    }
}

/// The operator of the primitive-result expression at `span`.
enum Operator {
    /// `left + right`.
    Plus(Span, Span),
    /// Any other operator: a number, a bigint or a boolean, or `typeof`'s name.
    Other,
    /// A binary whose operator text is not one token.
    Unread,
}

/// The binary operators that apply ToPrimitive to both operands (the
/// `coercing_operands` fact), except `+`.
const NUMERIC_BINARY: [&str; 17] = [
    "-", "*", "/", "%", "**", "<", ">", "<=", ">=", "==", "!=", "<<", ">>", ">>>", "&", "|", "^",
];

/// Reads the operator of the operator expression at `span` from its operands:
/// a binary that coerces has both in `coercing_operands`, the longest
/// starting at its start and ending at its end. An expression with no such
/// pair is a unary, an update, a strict equality, `in` or `instanceof`.
fn operator(file: &FileFacts, span: Span) -> Operator {
    let operands = &file.ast.coercing_operands;
    let from = operands.partition_point(|operand| operand.start < span.start);
    let to = operands.partition_point(|operand| operand.start < span.end);
    let inside = &operands[from..to];
    let left = inside
        .iter()
        .take_while(|operand| operand.start == span.start)
        .filter(|operand| operand.end < span.end)
        .max_by_key(|operand| operand.end);
    let right = inside
        .iter()
        .filter(|operand| operand.end == span.end && operand.start > span.start)
        .min_by_key(|operand| operand.start);
    let (Some(left), Some(right)) = (left, right) else {
        return Operator::Other;
    };
    if left.end > right.start {
        return Operator::Unread;
    }
    match file
        .source
        .get(left.end as usize..right.start as usize)
        .map(str::trim)
    {
        Some("+") => Operator::Plus(*left, *right),
        Some(token) if NUMERIC_BINARY.contains(&token) => Operator::Other,
        _ => Operator::Unread,
    }
}

/// The operator text of a compound assignment.
fn compound_operator<'s>(file: &'s FileFacts, assignment: &AssignmentFact) -> Option<&'s str> {
    let token = file
        .source
        .get(assignment.target.end as usize..assignment.value_span.start as usize)?
        .trim();
    (token == "+="
        || NUMERIC_BINARY
            .iter()
            .any(|op| token.strip_suffix('=') == Some(op)))
    .then_some(token)
}

/// `span` past parentheses, or `None` when a TypeScript assertion wraps it:
/// an `as` or `!` on a key's chain fails its proof.
fn peel_parens(file: &FileFacts, mut span: Span) -> Option<Span> {
    let wrappers = &file.ast.transparent_wrappers;
    while let Ok(index) = wrappers.binary_search_by_key(&span, |wrapper| wrapper.span) {
        let wrapper = &wrappers[index];
        if wrapper.inner == span {
            break;
        }
        let text = file.source_text(span)?.trim();
        if !(text.starts_with('(') && text.ends_with(')')) {
            return None;
        }
        span = wrapper.inner;
    }
    Some(span)
}

/// Whether the object at `span` has an own, fixed `length` (review D35 A5): an
/// array literal, a string literal or untagged template, or a `const` bound to
/// an array or string literal.
fn fixed_length(file: &FileFacts, span: Span) -> bool {
    let ast = &file.ast;
    let literal = |span: Span| {
        let span = ast.peel_ts_sugar_span(span);
        ast.array_literals.binary_search(&span).is_ok()
            || ast
                .template_literals
                .iter()
                .any(|template| template.span == span)
            || file
                .source_text(span)
                .map(str::trim)
                .and_then(string_literal)
                .is_some()
    };
    let object = ast.peel_ts_sugar_span(span);
    literal(object)
        || ast
            .reference_declaration(object)
            .and_then(|declaration| ast.identifier_binding(declaration))
            .is_some_and(|binding| {
                binding.immutable
                    && binding.shape == BindingShape::Identifier
                    && binding.initializer.is_some_and(literal)
            })
}

/// The standard function a callee spells: `Name` or `Name.member`, with
/// `Name` the global (no declaration of this file) and no computed member.
fn standard_callee(file: &FileFacts, callee: Span) -> Option<&'static str> {
    let ast = &file.ast;
    let callee = ast.peel_ts_sugar_span(callee);
    let text = file.source_text(callee)?;
    let (name, _) = TRUSTED.iter().find(|(name, _)| *name == text)?;
    let root = match ast.member_at(callee) {
        Some(member) => {
            if ast.computed_members.binary_search(&member.span).is_ok() {
                return None;
            }
            member.object
        }
        None => callee,
    };
    ast.reference_declaration(root).is_none().then_some(*name)
}

/// The trusted functions whose result is a number.
fn numeric_function(name: &str) -> bool {
    name.starts_with("Math.")
        || matches!(
            name,
            "Number" | "Number.parseFloat" | "Number.parseInt" | "parseFloat" | "parseInt"
        )
}

/// The contents of a quoted string literal with no escape.
fn string_literal(text: &str) -> Option<&str> {
    text.strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .or_else(|| {
            text.strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
        })
        .filter(|inner| !inner.contains(['\\', '"', '\'']))
}

/// A template's literal head and tail, as written; `None` for a part with an
/// escape or a comment beside a substitution.
fn template_affixes(
    file: &FileFacts,
    template: Span,
    expressions: &[Span],
) -> (Option<String>, Option<String>) {
    let source = &file.source;
    let (Some(first), Some(last)) = (expressions.first(), expressions.last()) else {
        return (None, None);
    };
    let head = source
        .get(template.start as usize + 1..first.start as usize)
        .map(str::trim_end)
        .and_then(|head| head.strip_suffix("${"))
        .filter(|head| !head.contains(['\\', '`', '$']))
        .map(str::to_owned);
    let tail = source
        .get(last.end as usize..(template.end as usize).saturating_sub(1))
        .map(str::trim_start)
        .and_then(|tail| tail.strip_prefix('}'))
        .filter(|tail| !tail.contains(['\\', '`', '$']))
        .map(str::to_owned);
    (head, tail)
}

/// The iterated operand of the `for (const … of operand)` that declares the
/// binding pattern at `pattern`: exactly `of` between them.
fn loop_source(file: &FileFacts, pattern: Span) -> Option<Span> {
    let ast = &file.ast;
    let start = ast
        .iterated_operands
        .partition_point(|operand| operand.start < pattern.end);
    let operand = *ast.iterated_operands.get(start)?;
    let between = file
        .source
        .get(pattern.end as usize..operand.start as usize)?;
    let before = file.source.get(..pattern.start as usize)?.trim_end();
    (between.trim() == "of"
        && between.starts_with(char::is_whitespace)
        && between.ends_with(char::is_whitespace)
        && before.ends_with("const")
        && before[..before.len() - "const".len()]
            .trim_end()
            .ends_with('('))
    .then_some(operand)
}

/// Whether the declaration at `declaration` is a `const` of this file bound to
/// a closed object literal (no spread, computed key, `__proto__` entry,
/// duplicate or setter): its own keys are the literal's, each a census site.
fn own_data(file: &FileFacts, declaration: Span) -> bool {
    let ast = &file.ast;
    ast.identifier_binding(declaration)
        .filter(|binding| binding.immutable && binding.shape == BindingShape::Identifier)
        .and_then(|binding| binding.initializer)
        .map(|initializer| ast.peel_ts_sugar_span(initializer))
        .is_some_and(|literal| {
            ast.object_get_shapes
                .iter()
                .any(|shape| shape.span == literal && shape.closed)
        })
}
