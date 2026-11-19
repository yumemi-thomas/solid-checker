//! The places that may break the plain-program premise (ADR 1120, facts schema 101).
//!
//! A proof that some evaluation runs no project code (the kernel's K1, `crate::kernel`) holds for every value a program can make only
//! where nothing listed here exists in the code the program runs: no Proxy, no write to a prototype or under a standard method's
//! name, no coercion or protocol hook, no accessor, no Solid private key, no shadow root, no `with`. The IR scopes these sites by
//! dependency reach and answers per value where it can; code built from strings and computed keys are already facts
//! (`code_sinks`, `computed_keys`), and an unread module is the IR's.
//!
//! Every test errs toward listing: a write is refused by its key whatever the receiver (no list of routes to the window is complete:
//! `event.currentTarget.escape = …`), a member is a write unless its parent provably reads it, and a key is compared by its cooked
//! value. A name is the global it names only when no binding of the file declares it, and every name is global where the binder
//! reported errors or a `with` may rebind it (`resolve` false).

use compact_str::CompactString;
use oxc_ast::AstKind;
use oxc_ast::ast::{CallExpression, Expression, IdentifierReference, PropertyKey, PropertyKind};
use oxc_semantic::{NodeId, Semantic};
use oxc_span::GetSpan;
use oxc_syntax::operator::UnaryOperator;

use super::extract_helpers::{literal_property_key, peel_ts_sugar};
use super::{PremiseClause, PremiseSiteFact};
use crate::core::Span;

/// Every function-valued standard name (scripts/kernel/standard-members.mjs, from TypeScript's lib files), sorted.
const STANDARD_MEMBERS: &str = include_str!("standard_members.txt");

/// The prototype routes, by name.
const PROTOTYPE_ROUTES: [&str; 13] = [
    "prototype",
    "__proto__",
    "constructor",
    "setPrototypeOf",
    "getPrototypeOf",
    "defineProperty",
    "defineProperties",
    "getOwnPropertyDescriptor",
    "getOwnPropertyDescriptors",
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
];

/// The string-keyed coercion and protocol hooks.
const HOOK_KEYS: [&str; 6] = [
    "toString",
    "valueOf",
    "toJSON",
    "then",
    "handleEvent",
    "toLocaleString",
];

/// ADR 1125 (G2): the standard readers of an object's own keys or values (`Object.*`).
const KEY_READERS: [&str; 9] = [
    "keys",
    "values",
    "entries",
    "getOwnPropertyNames",
    "getOwnPropertySymbols",
    "getOwnPropertyDescriptor",
    "getOwnPropertyDescriptors",
    "assign",
    "groupBy",
];

/// ADR 1125 (G4): the reflective definers and the index of their key argument.
const DEFINERS: [(&str, usize); 4] = [
    ("defineProperty", 1),
    ("__defineGetter__", 0),
    ("__defineSetter__", 0),
    ("set", 1),
];

/// ADR 1115's spellings.
const SHADOW_SPELLINGS: [&str; 9] = [
    "attachShadow",
    "shadowRoot",
    "ShadowRoot",
    "shadowrootmode",
    "setHTMLUnsafe",
    "parseHTMLUnsafe",
    "getRootNode",
    "composedPath",
    "customElements",
];

/// The table's lines, sorted by byte order (the generator sorts them; a unit test holds it).
static STANDARD: std::sync::LazyLock<Vec<&'static str>> =
    std::sync::LazyLock::new(|| STANDARD_MEMBERS.lines().collect());

/// Whether `name` is a function-valued standard name.
#[must_use]
pub fn is_standard_member(name: &str) -> bool {
    STANDARD.binary_search(&name).is_ok()
}

struct Sites<'s, 'a> {
    semantic: &'s Semantic<'a>,
    resolve: bool,
    found: Vec<PremiseSiteFact>,
    /// Each owner's [`Sites::contained`] answer, asked once per object literal or class.
    owners: std::collections::HashMap<NodeId, bool>,
}

/// The premise sites of one file, in source order (see the module's text).
pub(super) fn premise_sites<'a>(semantic: &Semantic<'a>, resolve: bool) -> Vec<PremiseSiteFact> {
    let mut sites = Sites {
        semantic,
        resolve,
        found: Vec::new(),
        owners: std::collections::HashMap::new(),
    };
    let mut keys = super::computed_keys::KeyProver::new(semantic, resolve);
    for node in semantic.nodes().iter() {
        match node.kind() {
            AstKind::WithStatement(statement) => {
                sites.add(statement.span, PremiseClause::With, "with")
            }
            AstKind::IdentifierReference(identifier) => sites.reference(identifier),
            AstKind::ForInStatement(statement) => {
                sites.add(statement.span, PremiseClause::KeyReader, "for-in")
            }
            AstKind::StaticMemberExpression(member) => {
                let key = member.property.name.as_str();
                // ADR 1125 (G2): a standard reader of own keys or values, named in any position (a call, a value handed on, `.call`).
                if let Expression::Identifier(object) = peel_ts_sugar(&member.object)
                    && sites.global(object)
                    && ((object.name == "Object" && KEY_READERS.contains(&key))
                        || (object.name == "JSON"
                            && key == "stringify"
                            && !sites.is_callee(node.id(), member.span)))
                {
                    sites.add(member.span, PremiseClause::KeyReader, key);
                }
                sites.key(member.span, key);
                if sites.is_write(node.id(), member.span) {
                    sites.written(member.span, key);
                }
                // `Symbol.toPrimitive`, `Symbol.iterator`, …: a protocol hook named.
                if let Expression::Identifier(symbol) = peel_ts_sugar(&member.object)
                    && symbol.name == "Symbol"
                    && sites.global(symbol)
                    && key != "for"
                    && key != "keyFor"
                {
                    sites.add(
                        member.span,
                        PremiseClause::CoercionHook,
                        &format!("Symbol.{key}"),
                    );
                }
            }
            AstKind::ComputedMemberExpression(member) => {
                // An unproven key is the `computed_keys` fact's; a literal one is checked as a name.
                if let Some(key) = literal_property_key(&member.expression) {
                    sites.key(member.span, &key);
                    if sites.is_write(node.id(), member.span) {
                        sites.written(member.span, &key);
                    }
                } else if !sites.is_write(node.id(), member.span) {
                    // ADR 1125 (G3/G4): a read under a key proven to be constant texts (`"attach" + "Shadow"`, `"proto" + "type"`) is
                    // checked against every spelling, as a literal key is.
                    if let Some((texts, _)) = keys.key(&member.expression) {
                        for text in texts {
                            sites.key(member.span, &text);
                        }
                    }
                } else if sites.is_write(node.id(), member.span) {
                    // A write under a key no literal names: the key's proof (`computed_keys`) narrows it to its texts; a number or a
                    // symbol names nothing; text that may hold a digit, or nothing proven, may be any standard name (`Int8Array`).
                    match keys.key(&member.expression) {
                        Some((texts, false)) => {
                            for text in texts {
                                sites.key(member.span, &text);
                                sites.written(member.span, &text);
                            }
                        }
                        // A receiver the program made here (a binding of an object or array literal that every write keeps one)
                        // is no prototype, window or element: the write gives that object an own key, which may be a hook on it.
                        _ if sites.fresh_receiver(&member.object) => {
                            sites.add(member.span, PremiseClause::CoercionHook, "*");
                        }
                        _ => sites.add(member.span, PremiseClause::StandardKeyWrite, "*"),
                    }
                }
            }
            AstKind::ObjectProperty(property) => {
                let from = sites.found.len();
                let value_is_code = property.method
                    || property.kind != PropertyKind::Init
                    || !matches!(
                        peel_ts_sugar(&property.value),
                        Expression::StringLiteral(_)
                            | Expression::NumericLiteral(_)
                            | Expression::BooleanLiteral(_)
                            | Expression::NullLiteral(_)
                            | Expression::BigIntLiteral(_)
                            | Expression::TemplateLiteral(_)
                    );
                sites.definition(
                    property.span,
                    &property.key,
                    property.computed,
                    value_is_code,
                );
                if property.kind != PropertyKind::Init {
                    sites.add(
                        property.span,
                        PremiseClause::Accessor,
                        &kernel_key(&property.key).unwrap_or_else(|| "*".into()),
                    );
                }
                sites.contain(from, semantic.nodes().parent_id(node.id()));
            }
            AstKind::MethodDefinition(method) => {
                let from = sites.found.len();
                sites.definition(method.span, &method.key, method.computed, true);
                if matches!(
                    method.kind,
                    oxc_ast::ast::MethodDefinitionKind::Get
                        | oxc_ast::ast::MethodDefinitionKind::Set
                ) {
                    sites.add(
                        method.span,
                        PremiseClause::Accessor,
                        &kernel_key(&method.key).unwrap_or_else(|| "*".into()),
                    );
                }
                sites.contain(from, class_of(semantic, node.id()));
            }
            AstKind::PropertyDefinition(property) => {
                let from = sites.found.len();
                sites.definition(
                    property.span,
                    &property.key,
                    property.computed,
                    property.value.is_some(),
                );
                sites.contain(from, class_of(semantic, node.id()));
            }
            AstKind::AccessorProperty(property) => {
                let from = sites.found.len();
                sites.add(
                    property.span,
                    PremiseClause::Accessor,
                    &kernel_key(&property.key).unwrap_or_else(|| "*".into()),
                );
                sites.contain(from, class_of(semantic, node.id()));
            }
            AstKind::BindingProperty(property) => {
                if let Some(key) = kernel_key(&property.key) {
                    sites.key(property.span, &key);
                    // ADR 1125 (G2): `const { entries } = Object` takes a reader out by name (whatever the object: fail closed).
                    if KEY_READERS.contains(&key.as_str()) {
                        sites.add(property.span, PremiseClause::KeyReader, &key);
                    }
                }
            }
            AstKind::AssignmentTargetPropertyProperty(property) => {
                if let Some(key) = kernel_key(&property.name) {
                    sites.key(property.span, &key);
                }
            }
            AstKind::CallExpression(call) => {
                sites.definer_key(call, &mut keys);
                if let Expression::StaticMemberExpression(callee) = peel_ts_sugar(&call.callee)
                    && callee.property.name == "stringify"
                    && let Expression::Identifier(object) = peel_ts_sugar(&callee.object)
                    && object.name == "JSON"
                    && sites.global(object)
                    && call.arguments.len() >= 2
                {
                    // ADR 1125 (G2): a replacer is handed every own enumerable value of every object it walks.
                    sites.add(call.span, PremiseClause::KeyReader, "JSON.stringify");
                }
                if let Expression::StaticMemberExpression(callee) = peel_ts_sugar(&call.callee)
                    && callee.property.name == "assign"
                    && let Expression::Identifier(object) = peel_ts_sugar(&callee.object)
                    && object.name == "Object"
                    && sites.global(object)
                    && !matches!(
                        call.arguments
                            .first()
                            .and_then(|argument| argument.as_expression())
                            .map(peel_ts_sugar),
                        Some(Expression::ObjectExpression(_))
                    )
                {
                    // ADR 1125: the keys written are the sources'. Sources that are all object literals with literal keys (no spread)
                    // write those keys only, each checked as a written key; anything else may write any key.
                    let literal_keys =
                        call.arguments
                            .iter()
                            .skip(1)
                            .try_fold(Vec::new(), |mut keys, argument| {
                                let Some(Expression::ObjectExpression(source)) =
                                    argument.as_expression().map(peel_ts_sugar)
                                else {
                                    return None;
                                };
                                for property in &source.properties {
                                    let oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property) =
                                        property
                                    else {
                                        return None;
                                    };
                                    keys.push(kernel_key(&property.key)?);
                                }
                                Some(keys)
                            });
                    match literal_keys {
                        Some(keys) => {
                            for key in keys {
                                sites.key(call.span, &key);
                                sites.written(call.span, &key);
                            }
                        }
                        None => sites.add(call.span, PremiseClause::ObjectAssign, "*"),
                    }
                }
            }
            AstKind::StringLiteral(literal) => {
                sites.text(literal.span, literal.value.as_str());
                if PROTOTYPE_ROUTES.contains(&literal.value.as_str()) {
                    sites.add(
                        literal.span,
                        PremiseClause::PrototypeRoute,
                        literal.value.as_str(),
                    );
                }
            }
            AstKind::TemplateLiteral(template) => {
                for quasi in &template.quasis {
                    if let Some(cooked) = &quasi.value.cooked {
                        sites.text(quasi.span, cooked.as_str());
                    }
                }
            }
            AstKind::JSXAttribute(attribute) => {
                let name = attribute.name.get_identifier().name;
                sites.text(attribute.span, name.as_str());
            }
            _ => {}
        }
    }
    sites
        .found
        .sort_by(|a, b| (a.at, a.clause, &a.key).cmp(&(b.at, b.clause, &b.key)));
    sites.found.dedup();
    sites.found
}

/// The class node a class element belongs to (its body's parent).
fn class_of(semantic: &Semantic<'_>, element: NodeId) -> NodeId {
    let nodes = semantic.nodes();
    nodes.parent_id(nodes.parent_id(element))
}

/// A property key's cooked text, from the kernel (K3): a name, a string, a canonical integer, a template with no substitution.
fn kernel_key(key: &PropertyKey<'_>) -> Option<CompactString> {
    crate::kernel::property_key_value(key).map(CompactString::from)
}

impl<'a> Sites<'_, 'a> {
    fn add(&mut self, at: oxc_span::Span, clause: PremiseClause, key: &str) {
        self.found.push(PremiseSiteFact {
            at: Span::new(at.start, at.end),
            clause,
            key: key.into(),
            contained: false,
        });
    }

    /// Marks the hook and accessor sites added since `from` as contained when `owner` (an object literal or a class node) is.
    fn contain(&mut self, from: usize, owner: NodeId) {
        if self.found.len() <= from {
            return;
        }
        let contained = match self.owners.get(&owner) {
            Some(&answer) => answer,
            None => {
                let answer = self.contained(owner);
                self.owners.insert(owner, answer);
                answer
            }
        };
        if contained {
            for site in &mut self.found[from..] {
                if matches!(
                    site.clause,
                    PremiseClause::CoercionHook | PremiseClause::Accessor
                ) {
                    site.contained = true;
                }
            }
        }
    }

    /// Whether the value `owner` makes reaches nothing but static member reads of its own binding (see [`PremiseSiteFact::contained`]).
    fn contained(&self, owner: NodeId) -> bool {
        if !self.resolve {
            return false;
        }
        let nodes = self.semantic.nodes();
        let scoping = self.semantic.scoping();
        let (symbol, span) = match nodes.kind(owner) {
            AstKind::ObjectExpression(object) => {
                let mut parent = nodes.parent_id(owner);
                while matches!(
                    nodes.kind(parent),
                    AstKind::ParenthesizedExpression(_)
                        | AstKind::TSAsExpression(_)
                        | AstKind::TSSatisfiesExpression(_)
                ) {
                    parent = nodes.parent_id(parent);
                }
                let AstKind::VariableDeclarator(declarator) = nodes.kind(parent) else {
                    return false;
                };
                if declarator.kind != oxc_ast::ast::VariableDeclarationKind::Const {
                    return false;
                }
                let Some(identifier) = declarator.id.get_binding_identifier() else {
                    return false;
                };
                let Some(symbol) = identifier.symbol_id.get() else {
                    return false;
                };
                (symbol, object.span)
            }
            AstKind::Class(class) => {
                let Some(symbol) = class.id.as_ref().and_then(|id| id.symbol_id.get()) else {
                    return false;
                };
                (symbol, class.span)
            }
            _ => return false,
        };
        // An export declaration holds the binding (`export const h = …`, `export class K`).
        let mut up = owner;
        loop {
            let parent = nodes.parent_id(up);
            if parent == up {
                break;
            }
            match nodes.kind(parent) {
                AstKind::ExportNamedDeclaration(_) | AstKind::ExportDefaultDeclaration(_) => {
                    return false;
                }
                AstKind::Program(_) => break,
                _ => up = parent,
            }
        }
        if nodes.iter().any(|node| {
            matches!(node.kind(), AstKind::ThisExpression(_) | AstKind::Super(_))
                && span.start <= node.kind().span().start
                && node.kind().span().end <= span.end
        }) {
            return false;
        }
        scoping
            .get_resolved_reference_ids(symbol)
            .iter()
            .all(|&reference| {
                let at = scoping.get_reference(reference).node_id();
                let reference_span = nodes.kind(at).span();
                let parent = nodes.parent_id(at);
                match nodes.kind(parent) {
                    AstKind::StaticMemberExpression(member) => {
                        member.object.span() == reference_span
                            && !matches!(
                                member.property.name.as_str(),
                                "constructor" | "__proto__" | "prototype"
                            )
                            && !self.is_write(parent, member.span)
                    }
                    _ => false,
                }
            })
    }

    /// Whether `identifier` names a global: no binding of the file declares it, or names cannot be followed.
    fn global(&self, identifier: &IdentifierReference<'_>) -> bool {
        !self.resolve
            || identifier
                .reference_id
                .get()
                .and_then(|reference| self.semantic.scoping().get_reference(reference).symbol_id())
                .is_none()
    }

    fn reference(&mut self, identifier: &IdentifierReference<'a>) {
        let name = identifier.name.as_str();
        if !self.global(identifier) {
            return;
        }
        if matches!(name, "Proxy" | "Reflect") {
            self.add(identifier.span, PremiseClause::ProxyOrReflect, name);
        }
        if SHADOW_SPELLINGS.contains(&name) {
            self.add(identifier.span, PremiseClause::ShadowRoot, name);
        }
        let written = identifier
            .reference_id
            .get()
            .is_some_and(|reference| self.semantic.scoping().get_reference(reference).is_write());
        if written {
            self.add(identifier.span, PremiseClause::GlobalWrite, name);
        }
    }

    /// A key read or written (a member name, a pattern key): the routes and the spellings it may be.
    fn key(&mut self, at: oxc_span::Span, key: &str) {
        if PROTOTYPE_ROUTES.contains(&key) {
            self.add(at, PremiseClause::PrototypeRoute, key);
        }
        if key.contains("$$") || key.starts_with("_$") {
            self.add(at, PremiseClause::PrivateKey, key);
        }
        if SHADOW_SPELLINGS.contains(&key) {
            self.add(at, PremiseClause::ShadowRoot, key);
        }
    }

    /// A member written under `key`.
    fn written(&mut self, at: oxc_span::Span, key: &str) {
        // ADR 1125: a written `__proto__` swaps the receiver's prototype (an element's or a prototype's own), a write a consumer refuses by
        // its key; a literal's `__proto__:` sets only the new object's.
        if is_standard_member(key) || key == "__proto__" {
            self.add(at, PremiseClause::StandardKeyWrite, key);
        }
        if HOOK_KEYS.contains(&key) {
            self.add(at, PremiseClause::CoercionHook, key);
        }
    }

    /// A property, method or field defined under `key`; `code` says whether the value may be code.
    fn definition(
        &mut self,
        at: oxc_span::Span,
        key: &PropertyKey<'a>,
        computed: bool,
        code: bool,
    ) {
        match kernel_key(key) {
            Some(text) => {
                self.key(at, &text);
                if HOOK_KEYS.contains(&text.as_str()) {
                    self.add(at, PremiseClause::CoercionHook, &text);
                }
                if text == "__proto__" && !computed {
                    self.add(at, PremiseClause::PrototypeRoute, "__proto__");
                }
            }
            // A key no literal names may be `toString` or a well-known symbol; the symbols themselves are listed where they are named.
            None if computed && code => self.add(at, PremiseClause::CoercionHook, "*"),
            None => {}
        }
    }

    /// Text that may spell a shadow-root API (a string, a template piece, a JSX attribute).
    fn text(&mut self, at: oxc_span::Span, text: &str) {
        for spelling in SHADOW_SPELLINGS {
            if text.contains(spelling) {
                self.add(at, PremiseClause::ShadowRoot, spelling);
            }
        }
    }

    /// ADR 1120 (per value): whether `receiver` is a name bound to an object or array literal the program made, every write to which is
    /// such a literal: a variable declarator's binding (no parameter, no import, no pattern, no loop head), its initializer a literal,
    /// each assignment to it (`=` only) a literal. Such an object is no shared prototype, the window or an element, whatever key is written.
    fn fresh_receiver(&self, receiver: &Expression<'a>) -> bool {
        let Expression::Identifier(identifier) = peel_ts_sugar(receiver) else {
            return false;
        };
        if !self.resolve {
            return false;
        }
        let scoping = self.semantic.scoping();
        let nodes = self.semantic.nodes();
        let Some(symbol) = identifier
            .reference_id
            .get()
            .and_then(|reference| scoping.get_reference(reference).symbol_id())
        else {
            return false;
        };
        let literal = |expression: &Expression<'_>| {
            matches!(
                peel_ts_sugar(expression),
                Expression::ObjectExpression(_) | Expression::ArrayExpression(_)
            )
        };
        // The declaration node is the declarator itself (or, for a binding identifier, its parent).
        let declaration = scoping.symbol_declaration(symbol);
        let declarator_node = match nodes.kind(declaration) {
            AstKind::VariableDeclarator(_) => declaration,
            _ => nodes.parent_id(declaration),
        };
        let AstKind::VariableDeclarator(declarator) = nodes.kind(declarator_node) else {
            return false;
        };
        if !declarator.id.is_binding_identifier() || !declarator.init.as_ref().is_some_and(literal)
        {
            return false;
        }
        // A loop head's binding (`for (const o of xs)`) holds each element, never the literal.
        if matches!(
            nodes.parent_kind(nodes.parent_id(declarator_node)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        ) {
            return false;
        }
        scoping
            .get_resolved_reference_ids(symbol)
            .iter()
            .all(|&reference| {
                let reference = scoping.get_reference(reference);
                if !reference.is_write() {
                    return true;
                }
                match nodes.parent_kind(reference.node_id()) {
                    AstKind::AssignmentExpression(assignment) => {
                        assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign
                            && literal(&assignment.right)
                    }
                    _ => false,
                }
            })
    }

    /// Whether the member at `span` is the callee of a call (`x.m(…)`).
    fn is_callee(&self, id: NodeId, span: oxc_span::Span) -> bool {
        matches!(self.semantic.nodes().parent_kind(id), AstKind::CallExpression(call) if call.callee.span() == span)
    }

    /// ADR 1125 (G4): the key a reflective definer writes (`Object.defineProperty(o, k, d)`, `Reflect.defineProperty`,
    /// `o.__defineGetter__(k, f)`, `Reflect.set(o, k, v)`) is a key written, by its value: a literal or proven texts are checked as
    /// written keys, anything else may be any standard name. `defineProperties` writes every key of its map: any.
    fn definer_key(
        &mut self,
        call: &'a CallExpression<'a>,
        keys: &mut super::computed_keys::KeyProver<'_, 'a>,
    ) {
        let Expression::StaticMemberExpression(callee) = peel_ts_sugar(&call.callee) else {
            return;
        };
        let name = callee.property.name.as_str();
        if name == "defineProperties" {
            self.add(call.span, PremiseClause::StandardKeyWrite, "*");
            return;
        }
        let Some(&(_, index)) = DEFINERS.iter().find(|(definer, _)| *definer == name) else {
            return;
        };
        // `set` is a definer only as `Reflect.set`; any other `set` (a Map's) writes no property.
        if name == "set"
            && !matches!(peel_ts_sugar(&callee.object), Expression::Identifier(object) if object.name == "Reflect" && self.global(object))
        {
            return;
        }
        let Some(key) = call
            .arguments
            .get(index)
            .and_then(|argument| argument.as_expression())
        else {
            self.add(call.span, PremiseClause::StandardKeyWrite, "*");
            return;
        };
        if let Some(text) = literal_property_key(key) {
            self.key(call.span, &text);
            self.written(call.span, &text);
            return;
        }
        match keys.key(key) {
            Some((texts, false)) => {
                for text in texts {
                    self.key(call.span, &text);
                    self.written(call.span, &text);
                }
            }
            _ => self.add(call.span, PremiseClause::StandardKeyWrite, "*"),
        }
    }

    /// Whether the member at `span` is written: the target of an assignment, an update, a `delete`, a destructuring assignment or a
    /// `for in`/`for of` head. Errs toward a write: any assignment-target parent counts.
    fn is_write(&self, id: NodeId, span: oxc_span::Span) -> bool {
        let nodes = self.semantic.nodes();
        match nodes.parent_kind(id) {
            AstKind::AssignmentExpression(assignment) => assignment.left.span() == span,
            AstKind::UpdateExpression(_) => true,
            AstKind::UnaryExpression(unary) => unary.operator == UnaryOperator::Delete,
            AstKind::ForInStatement(statement) => statement.left.span() == span,
            AstKind::ForOfStatement(statement) => statement.left.span() == span,
            AstKind::ArrayAssignmentTarget(_)
            | AstKind::ObjectAssignmentTarget(_)
            | AstKind::AssignmentTargetRest(_)
            | AstKind::AssignmentTargetWithDefault(_)
            | AstKind::AssignmentTargetPropertyIdentifier(_)
            | AstKind::AssignmentTargetPropertyProperty(_) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sites(source: &str) -> Vec<(PremiseClause, String)> {
        let facts = crate::ast::extract("case.tsx", source).unwrap();
        facts
            .premise_sites
            .into_iter()
            .map(|site| (site.clause, site.key.to_string()))
            .collect()
    }

    fn has(source: &str, clause: PremiseClause, key: &str) -> bool {
        sites(source).iter().any(|(c, k)| *c == clause && k == key)
    }

    #[test]
    fn the_table_is_sorted_and_holds_the_patch_targets() {
        let all: Vec<&str> = STANDARD_MEMBERS.lines().collect();
        assert!(all.windows(2).all(|pair| pair[0] < pair[1]));
        for name in [
            "addEventListener",
            "escape",
            "reverse",
            "exec",
            "join",
            "toISOString",
            "click",
            "dispatchEvent",
        ] {
            assert!(is_standard_member(name), "{name}");
        }
    }

    #[test]
    fn writes_are_refused_by_key_on_any_receiver() {
        assert!(has(
            "addEventListener('x', (event) => { (event.currentTarget as any).escape = f; });",
            PremiseClause::StandardKeyWrite,
            "escape"
        ));
        assert!(has(
            "EventTarget.prototype.addEventListener = f;",
            PremiseClause::StandardKeyWrite,
            "addEventListener"
        ));
        assert!(has(
            "EventTarget.prototype.addEventListener = f;",
            PremiseClause::PrototypeRoute,
            "prototype"
        ));
        assert!(has(
            "el['addEventListener'] = f;",
            PremiseClause::StandardKeyWrite,
            "addEventListener"
        ));
        assert!(has("el[k] = f;", PremiseClause::StandardKeyWrite, "*"));
        assert!(
            !has(
                "const params = {}; params[k] = v;",
                PremiseClause::StandardKeyWrite,
                "*"
            ),
            "a fresh receiver"
        );
        assert!(
            has(
                "const params = {}; params[k] = v;",
                PremiseClause::CoercionHook,
                "*"
            ),
            "which may get a hook"
        );
        assert!(
            has(
                "let o = {}; o = window; o[k] = f;",
                PremiseClause::StandardKeyWrite,
                "*"
            ),
            "rebound to the window"
        );
        assert!(
            has(
                "function f(o) { o[k] = g; }",
                PremiseClause::StandardKeyWrite,
                "*"
            ),
            "a parameter"
        );
        assert!(
            has(
                "const o = make(); o[k] = g;",
                PremiseClause::StandardKeyWrite,
                "*"
            ),
            "a call may return a prototype"
        );
        assert!(
            !has(
                "const a = []; for (let i = 0; i < 3; i++) a[i] = i;",
                PremiseClause::StandardKeyWrite,
                "*"
            ),
            "a number names nothing"
        );
        assert!(
            has(
                "const k = 'Int' + 8; el[k + 'Array'] = f;",
                PremiseClause::StandardKeyWrite,
                "*"
            ),
            "text with a digit may be a standard name"
        );
        assert!(has(
            "const k = c ? 'escape' : 'x'; el[k] = f;",
            PremiseClause::StandardKeyWrite,
            "escape"
        ));
        assert!(!has(
            "const k = c ? 'a' : 'b'; el[k] = f;",
            PremiseClause::StandardKeyWrite,
            "*"
        ));
        assert!(has(
            "({ a: el.click } = o);",
            PremiseClause::StandardKeyWrite,
            "click"
        ));
        assert!(!has(
            "el.textContent = 'x';",
            PremiseClause::StandardKeyWrite,
            "textContent"
        ));
        assert!(
            !has(
                "el.addEventListener('click', f);",
                PremiseClause::StandardKeyWrite,
                "addEventListener"
            ),
            "a call reads the member"
        );
        assert!(has("escape = f;", PremiseClause::GlobalWrite, "escape"));
        assert!(!has(
            "let escape = 1; escape = 2;",
            PremiseClause::GlobalWrite,
            "escape"
        ));
    }

    #[test]
    fn hooks_accessors_proxies_and_private_keys() {
        assert!(has(
            "const h = { toString() { return 'x'; } };",
            PremiseClause::CoercionHook,
            "toString"
        ));
        assert!(has(
            "class K { static [Symbol.hasInstance]() { return true; } }",
            PremiseClause::CoercionHook,
            "Symbol.hasInstance"
        ));
        assert!(has(
            "const o = { [k]: () => 1 };",
            PremiseClause::CoercionHook,
            "*"
        ));
        assert!(!has(
            "const o = { [k]: 1 };",
            PremiseClause::CoercionHook,
            "*"
        ));
        assert!(has(
            "const o = { get v() { return 1; } };",
            PremiseClause::Accessor,
            "v"
        ));
        assert!(has("class C { set v(x) {} }", PremiseClause::Accessor, "v"));
        assert!(has(
            "const p = new Proxy({}, {});",
            PremiseClause::ProxyOrReflect,
            "Proxy"
        ));
        assert!(!has(
            "const Proxy = 1; Proxy;",
            PremiseClause::ProxyOrReflect,
            "Proxy"
        ));
        assert!(has(
            "el['_\\x24\\x24click'];",
            PremiseClause::PrivateKey,
            "_$$click"
        ));
        assert!(has(
            "Object.defineProperty(Element.prototype, '_$$' + 'click', d);",
            PremiseClause::PrototypeRoute,
            "defineProperty"
        ));
        assert!(has(
            "Object.assign(target, src);",
            PremiseClause::ObjectAssign,
            "*"
        ));
        assert!(!has(
            "Object.assign({}, src);",
            PremiseClause::ObjectAssign,
            "*"
        ));
    }

    fn contained(source: &str, key: &str) -> Option<bool> {
        let facts = crate::ast::extract("case.tsx", source).unwrap();
        facts
            .premise_sites
            .iter()
            .find(|site| {
                matches!(
                    site.clause,
                    PremiseClause::CoercionHook | PremiseClause::Accessor
                ) && site.key == key
            })
            .map(|site| site.contained)
    }

    #[test]
    fn a_hook_is_contained_only_on_a_value_nothing_else_reaches() {
        assert_eq!(
            contained(
                "const h = { toString() { return 'x'; }, n: 1 }; f(h.n);",
                "toString"
            ),
            Some(true)
        );
        assert_eq!(
            contained(
                "const h = { toString() { return 'x'; } }; f(`${h}`);",
                "toString"
            ),
            Some(false),
            "a value use"
        );
        assert_eq!(
            contained(
                "export const h = { toString() { return 'x'; } };",
                "toString"
            ),
            Some(false)
        );
        assert_eq!(
            contained(
                "const h = { toString() { return 'x'; } }; h.toString = g;",
                "toString"
            ),
            Some(false)
        );
        assert_eq!(
            contained("const h = { get v() { return this; } }; f(h.v);", "v"),
            Some(false),
            "this hands it on"
        );
        assert_eq!(
            contained("let h = { get v() { return 1; } }; f(h.v);", "v"),
            Some(false)
        );
        assert_eq!(
            contained("class K { static get v() { return 1; } } f(K.v);", "v"),
            Some(true)
        );
        assert_eq!(
            contained("class K { get v() { return 1; } } f(new K());", "v"),
            Some(false),
            "an instance escapes"
        );
        assert_eq!(
            contained(
                "const h = { get v() { return 1; } }; f(h.constructor);",
                "v"
            ),
            Some(false)
        );
    }

    #[test]
    fn keys_are_read_by_value_and_key_readers_by_any_name() {
        // G3, G4: a computed read under a key proven to be constant texts is checked against every spelling.
        assert!(has(
            "const r = (host as any)[\"attach\" + \"Shadow\"];",
            PremiseClause::ShadowRoot,
            "attachShadow"
        ));
        assert!(has(
            "const d = (Object as any)[\"define\" + \"Property\"];",
            PremiseClause::PrototypeRoute,
            "defineProperty"
        ));
        assert!(has(
            "const p = (Element as any)[\"proto\" + \"type\"];",
            PremiseClause::PrototypeRoute,
            "prototype"
        ));
        // G4: a definer's key argument is a written key, by its value.
        assert!(has(
            "Object.defineProperty(o, \"_$\" + \"$click\", d);",
            PremiseClause::PrivateKey,
            "_$$click"
        ));
        assert!(has(
            "Object.defineProperty(o, \"add\" + \"EventListener\", d);",
            PremiseClause::StandardKeyWrite,
            "addEventListener"
        ));
        assert!(has(
            "Object.defineProperty(o, k, d);",
            PremiseClause::StandardKeyWrite,
            "*"
        ));
        assert!(has(
            "Reflect.set(o, \"event\", v);",
            PremiseClause::StandardKeyWrite,
            "event"
        ));
        assert!(
            !has("map.set(k, v);", PremiseClause::StandardKeyWrite, "*"),
            "a Map's set writes no property"
        );
        // G2: key readers in any position.
        assert!(has(
            "[el].flatMap(Object.values);",
            PremiseClause::KeyReader,
            "values"
        ));
        assert!(has(
            "const { entries } = Object;",
            PremiseClause::KeyReader,
            "entries"
        ));
        assert!(has(
            "Object.values.call(Object, bag);",
            PremiseClause::KeyReader,
            "values"
        ));
        assert!(has(
            "JSON.stringify(el, (k, v) => v);",
            PremiseClause::KeyReader,
            "JSON.stringify"
        ));
        assert!(!has(
            "JSON.stringify(data);",
            PremiseClause::KeyReader,
            "JSON.stringify"
        ));
        assert!(has(
            "for (const k in o) void k;",
            PremiseClause::KeyReader,
            "for-in"
        ));
    }

    #[test]
    fn a_timer_by_any_name_may_run_a_string() {
        let sinks = |source: &str| {
            crate::ast::extract("case.tsx", source)
                .unwrap()
                .code_sinks
                .len()
        };
        assert_eq!(
            sinks("const later = setTimeout; later(\"code\", 0);"),
            1,
            "the alias"
        );
        assert_eq!(
            sinks("document.defaultView?.setTimeout(\"code\", 0);"),
            1,
            "any receiver"
        );
        assert_eq!(sinks("const t = window.setTimeout;"), 1, "a member read");
        assert_eq!(sinks("setTimeout.call(null, \"code\");"), 1);
        assert_eq!(sinks("setTimeout(() => 1, 0);"), 0);
        assert_eq!(
            sinks("let t: ReturnType<typeof setTimeout> | undefined; t = setTimeout(() => 1, 0);"),
            0,
            "a type query"
        );
        assert_eq!(sinks("const ok = typeof setTimeout === \"function\";"), 0);
        assert_eq!(
            sinks("el.ownerDocument.defaultView!.setTimeout(() => 1, 0);"),
            0
        );
        // ADR 1125: a global `new Promise` executor's first two parameters are functions; anything else may be a string.
        assert_eq!(
            sinks("const pause = (ms: number) => new Promise((done) => setTimeout(done, ms));"),
            0,
            "the resolver"
        );
        assert_eq!(
            sinks("new Promise(function (ok, fail) { setTimeout(fail, 0); });"),
            0
        );
        assert_eq!(
            sinks("new Promise((a, b, c) => setTimeout(c, 0));"),
            1,
            "a third parameter"
        );
        assert_eq!(
            sinks("new Promise((done = \"x\") => setTimeout(done, 0));"),
            1,
            "a default"
        );
        assert_eq!(
            sinks("new Promise((done) => { done = \"x\"; setTimeout(done, 0); });"),
            1,
            "written"
        );
        assert_eq!(
            sinks(
                "class Promise { constructor(f: any) {} } new Promise((done: any) => setTimeout(done, 0));"
            ),
            1,
            "a local Promise"
        );
        assert_eq!(
            sinks("const run = (done: any) => setTimeout(done, 0); new Promise(run);"),
            1,
            "an executor not written in place"
        );
        assert_eq!(
            sinks("new Promise((done) => [1].forEach((d: any) => setTimeout(d, 0)));"),
            1,
            "an inner function's parameter"
        );
    }

    #[test]
    fn shadow_spellings_and_the_window_read() {
        assert!(has(
            "host.attachShadow({ mode: 'open' });",
            PremiseClause::ShadowRoot,
            "attachShadow"
        ));
        assert!(has(
            "customElements.define('x-a', K);",
            PremiseClause::ShadowRoot,
            "customElements"
        ));
        assert!(has(
            "el.innerHTML = '<template shadowrootmode=\"open\"></template>';",
            PremiseClause::ShadowRoot,
            "shadowrootmode"
        ));
        assert!(
            sites("if (typeof window !== 'undefined') window.addEventListener('x', f);").is_empty(),
            "reading the window is no hazard"
        );
    }
}
