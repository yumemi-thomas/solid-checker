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
pub(crate) const PROTOTYPE_ROUTES: [&str; 13] = [
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
pub(crate) const HOOK_KEYS: [&str; 6] = [
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

/// ADR 1133: the names whose value, kept or called other than as a written call, may deliver an event no scan reads.
const DISPATCHERS: [&str; 2] = ["dispatchEvent", "click"];

/// ADR 1125 (G4): the reflective definers and the index of their key argument.
const DEFINERS: [(&str, usize); 4] = [
    ("defineProperty", 1),
    ("__defineGetter__", 0),
    ("__defineSetter__", 0),
    ("set", 1),
];

/// ADR 1115's spellings.
pub(crate) const SHADOW_SPELLINGS: [&str; 9] = [
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
            AstKind::IdentifierReference(identifier) => {
                // ADR 1133 (#296 owed): `Reflect.apply`, `Reflect.construct` only call what they are given; `Reflect.get` and
                // `Reflect.has` read the key they are given, which is checked as a key when a literal or proven texts name it.
                let reflect_reads = identifier.name == "Reflect"
                    && sites.global(identifier)
                    && sites.reflect_member_reads_only(node.id(), identifier.span, &mut keys);
                sites.reference(identifier, reflect_reads);
                // ADR 1133 (F6): `Object` handed on as a value (`const O = Object`, `const { assign } = Object`, `f(Object)`) may be
                // called as `Object.assign` by any name; only a member read, a call or `new`, `instanceof` and `typeof` keep it in sight.
                if identifier.name == "Object"
                    && sites.global(identifier)
                    && !sites.object_in_sight(node.id(), identifier.span)
                {
                    sites.add(identifier.span, PremiseClause::ObjectAssign, "*");
                }
                // ADR 1133 (round 1 F4): `JSON` handed on (`const J = JSON`, `f(JSON)`) may be called as `JSON.stringify` with a replacer
                // by any name, which walks every own enumerable value of what it is given (Solid's stored handler among them).
                if identifier.name == "JSON"
                    && sites.global(identifier)
                    && !sites.object_in_sight(node.id(), identifier.span)
                {
                    sites.add(identifier.span, PremiseClause::KeyReader, "JSON");
                }
            }
            AstKind::ForInStatement(statement) => {
                sites.add(statement.span, PremiseClause::KeyReader, "for-in")
            }
            AstKind::StaticMemberExpression(member) => {
                let key = member.property.name.as_str();
                // ADR 1125 (G2): a standard reader of own keys or values, named in any position (a call, a value handed on, `.call`).
                // ADR 1133 (round 1 F4): `JSON` by any route (`globalThis.JSON.stringify`).
                if (sites.names_object(&member.object, &mut keys) && KEY_READERS.contains(&key))
                    || (key == "stringify"
                        && sites.names_json(&member.object, &mut keys)
                        && !sites.is_callee(node.id(), member.span))
                {
                    sites.add(member.span, PremiseClause::KeyReader, key);
                }
                sites.member_name(node.id(), member.span, key, false);
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
                // ADR 1125 (Fable round 2, N1): a reader spelled by a computed key (`Object["values"]`, `Object?.["values"]`,
                // `JSON["stringify"]`) is the reader; a key proven to be texts is checked against each, and a key nothing proves may be
                // any reader.
                // ADR 1133 (N2): `globalThis.Object["values"]` too (`names_object`).
                // ADR 1133 (round 1 F4): `JSON` by any route (`globalThis.JSON["stringify"]`), `Object` by a computed member too.
                let readers: Option<&[&str]> = if sites.names_object(&member.object, &mut keys) {
                    Some(&KEY_READERS)
                } else if sites.names_json(&member.object, &mut keys) {
                    Some(&["stringify"])
                } else {
                    None
                };
                if let Some(readers) = readers {
                    let texts = match literal_property_key(&member.expression) {
                        Some(key) => Some(vec![key.to_string()]),
                        None => keys.key(&member.expression).map(|(texts, _)| {
                            texts.into_iter().map(|text| text.to_string()).collect()
                        }),
                    };
                    match texts {
                        Some(texts) => {
                            for text in texts.iter().filter(|text| readers.contains(&text.as_str()))
                            {
                                sites.add(member.span, PremiseClause::KeyReader, text);
                            }
                        }
                        None => sites.add(member.span, PremiseClause::KeyReader, "*"),
                    }
                }
                // An unproven key is the `computed_keys` fact's; a literal one is checked as a name.
                if let Some(key) = literal_property_key(&member.expression) {
                    // ADR 1133: `el["dispatchEvent"]` read as a value, as the static member; round 1 F4: every name test of the static
                    // member (`globalThis["Object"]`, `({})["constructor"]`, `x["Reflect"]`).
                    sites.member_name(node.id(), member.span, &key, false);
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
                            // Round 1 F3: a key the program builds is a key the dispatch census cannot read, so a dispatcher under it is
                            // handed on whether its value is kept or called (`el["dispatch" + "Event"](e)`).
                            sites.member_name(node.id(), member.span, &text, true);
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
            // ADR 1133 (round 1 F3, F4): a pattern takes a member out under its key as a value (`const { dispatchEvent } = el`,
            // `({ dispatchEvent: d } = el)`, `({ entries } = Object)`, `const { Object: O } = globalThis`), so every name test of a
            // member read applies to the key, whatever spelling gives it (a name, a literal, proven texts).
            AstKind::BindingProperty(property) => {
                sites.pattern_member(property.span, &property.key, property.computed, &mut keys);
            }
            AstKind::AssignmentTargetPropertyProperty(property) => {
                sites.pattern_member(property.span, &property.name, property.computed, &mut keys);
            }
            AstKind::AssignmentTargetPropertyIdentifier(property) => {
                sites.pattern_key(property.span, property.binding.name.as_str());
            }
            AstKind::CallExpression(call) => {
                sites.definer_key(call, &mut keys);
                if let Expression::StaticMemberExpression(callee) = peel_ts_sugar(&call.callee)
                    && callee.property.name == "stringify"
                    && sites.names_json(&callee.object, &mut keys)
                    // N1: a spread may hand any number of arguments, a replacer among them.
                    && (call.arguments.len() >= 2
                        || call.arguments.iter().any(|argument| argument.is_spread()))
                {
                    // ADR 1125 (G2): a replacer is handed every own enumerable value of every object it walks.
                    sites.add(call.span, PremiseClause::KeyReader, "JSON.stringify");
                }
                if let Expression::StaticMemberExpression(callee) = peel_ts_sugar(&call.callee)
                    && callee.property.name == "assign"
                    && sites.names_object(&callee.object, &mut keys)
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
        // An export declaration holds the binding (`export const h = …`, `export class K`): the declaration's own parent, not any
        // ancestor (ADR 1133: a `const` in an exported function's body is no export).
        let declaration = match nodes.kind(owner) {
            AstKind::Class(_) => owner,
            _ => {
                // The literal's `const` declarator, then its declaration.
                let mut up = nodes.parent_id(owner);
                while !matches!(nodes.kind(up), AstKind::VariableDeclarator(_)) {
                    up = nodes.parent_id(up);
                }
                nodes.parent_id(up)
            }
        };
        if matches!(
            nodes.parent_kind(declaration),
            AstKind::ExportNamedDeclaration(_) | AstKind::ExportDefaultDeclaration(_)
        ) {
            return false;
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

    fn reference(&mut self, identifier: &IdentifierReference<'a>, reflect_reads: bool) {
        let name = identifier.name.as_str();
        if !self.global(identifier) {
            return;
        }
        if name == "Proxy" || (name == "Reflect" && !reflect_reads) {
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

    /// ADR 1133 (round 1 F3, F4): the name tests of a member read under `key` at `span` (`id` is the member's node), shared by the
    /// static member and every spelling of its key. `hidden`: the key is one the program builds (proven texts), which the dispatch
    /// census cannot read, so a dispatcher under it is handed on whether its value is kept or called; a key written as a name or a
    /// literal is the census's own, and a dispatcher is handed on only where its value is kept.
    fn member_name(&mut self, id: NodeId, span: oxc_span::Span, key: &str, hidden: bool) {
        // A dispatcher read as a value (`el.dispatchEvent.call(…)`, `Reflect.apply(….dispatchEvent, …)`, an alias).
        if DISPATCHERS.contains(&key) && (hidden || self.value_kept(id, span)) {
            self.add(span, PremiseClause::DispatchHandedOn, key);
        }
        // `globalThis.Reflect`, `window.Proxy`, and `globalThis.Object` handed on, are the globals by another route.
        if matches!(key, "Proxy" | "Reflect") {
            self.add(span, PremiseClause::ProxyOrReflect, key);
        }
        // `Object` and `constructor` kept or handed on may be called as `Object.assign` by any name, `JSON` as `JSON.stringify` with a
        // replacer: used as the object of a member read they are in sight, and the member's own test reads what it names.
        if matches!(key, "Object" | "constructor") && !self.object_in_sight(id, span) {
            self.add(span, PremiseClause::ObjectAssign, "*");
        }
        if key == "JSON" && !self.object_in_sight(id, span) {
            self.add(span, PremiseClause::KeyReader, "JSON");
        }
    }

    /// A pattern's property (`const { k } = x`, `({ k: v } = x)`, `({ [expr]: v } = x)`) under the key it names: a name or literal, or
    /// the texts a proof gives a computed key (a key nothing proves is `computed_keys`' and refuses the program).
    fn pattern_member(
        &mut self,
        at: oxc_span::Span,
        key: &'a PropertyKey<'a>,
        computed: bool,
        keys: &mut super::computed_keys::KeyProver<'_, 'a>,
    ) {
        match kernel_key(key) {
            Some(text) => self.pattern_key(at, &text),
            None if computed => {
                if let Some((texts, _)) = key
                    .as_expression()
                    .and_then(|expression| keys.key(expression))
                {
                    for text in texts {
                        self.pattern_key(at, &text);
                    }
                }
            }
            None => {}
        }
    }

    /// A member taken out of an object by a pattern under `key` is read as a value and kept in a name no scan follows (a binding, a
    /// parameter): every name test applies to the key whatever the object (fail closed), not only the three of [`Sites::key`].
    fn pattern_key(&mut self, at: oxc_span::Span, key: &str) {
        self.key(at, key);
        // `const { dispatchEvent } = el` takes the dispatcher out (`dispatchEvent.call(el, e)`).
        if DISPATCHERS.contains(&key) {
            self.add(at, PremiseClause::DispatchHandedOn, key);
        }
        // ADR 1125 (G2): `const { entries } = Object` takes a reader out by name (whatever the object: fail closed).
        if KEY_READERS.contains(&key) {
            self.add(at, PremiseClause::KeyReader, key);
        }
        // `const { Object: O } = globalThis`, `const { constructor: C } = {}`, `const { JSON: J } = globalThis`, `({ Reflect: R } = window)`.
        if matches!(key, "Object" | "constructor") {
            self.add(at, PremiseClause::ObjectAssign, "*");
        }
        if matches!(key, "Proxy" | "Reflect") {
            self.add(at, PremiseClause::ProxyOrReflect, key);
        }
        if key == "JSON" {
            self.add(at, PremiseClause::KeyReader, "JSON");
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

    /// ADR 1133 (F6, N2): whether the expression may be the global `Object`: the name no binding declares, a member so named of anything
    /// (`globalThis.Object`, `window.Object`: fail closed on the receiver), or any value's `constructor`, by whatever spelling the key
    /// has: a name, a literal (`({})["constructor"]`, `globalThis["Object"]`) or texts a proof gives it (round 1 F4).
    fn names_object(
        &self,
        expression: &'a Expression<'a>,
        keys: &mut super::computed_keys::KeyProver<'_, 'a>,
    ) -> bool {
        self.names_global(expression, keys, "Object", true)
    }

    /// ADR 1133 (round 1 F4): whether the expression may be the global `JSON`, as [`Self::names_object`] reads `Object` (without
    /// `constructor`): the name no binding declares, or a member so named of anything.
    fn names_json(
        &self,
        expression: &'a Expression<'a>,
        keys: &mut super::computed_keys::KeyProver<'_, 'a>,
    ) -> bool {
        self.names_global(expression, keys, "JSON", false)
    }

    fn names_global(
        &self,
        expression: &'a Expression<'a>,
        keys: &mut super::computed_keys::KeyProver<'_, 'a>,
        name: &str,
        constructor: bool,
    ) -> bool {
        let named = |key: &str| key == name || (constructor && key == "constructor");
        match peel_ts_sugar(expression) {
            Expression::Identifier(object) => object.name == name && self.global(object),
            // N2: any value's `constructor` may be `Object` (`({}).constructor.values(el)`).
            Expression::StaticMemberExpression(member) => named(member.property.name.as_str()),
            // F4: the same member under a literal or proven key; a key nothing proves is `computed_keys`' (the value is no plain
            // data, so `unproven-key` refuses it).
            Expression::ComputedMemberExpression(member) => {
                match literal_property_key(&member.expression) {
                    Some(key) => named(&key),
                    None => keys
                        .key(&member.expression)
                        .is_some_and(|(texts, _)| texts.iter().any(|text| named(text))),
                }
            }
            _ => false,
        }
    }

    /// ADR 1133 (F6): whether the global `Object` (or a value's `constructor`) at `span` is used where no other name can take
    /// `Object.assign` from it: the object of a member read under a key a literal names (the member's own test sees `assign`), the callee
    /// of a call or `new`, the right side of `instanceof`, an operand of an equality, or the operand of `typeof`. TypeScript's wrappers and parentheses are no use (`(Object as any)[k]`): fail closed.
    fn object_in_sight(&self, id: NodeId, span: oxc_span::Span) -> bool {
        match self.semantic.nodes().parent_kind(id) {
            AstKind::StaticMemberExpression(member) => member.object.span() == span,
            AstKind::ComputedMemberExpression(member) => {
                member.object.span() == span && literal_property_key(&member.expression).is_some()
            }
            AstKind::CallExpression(call) => call.callee.span() == span,
            AstKind::NewExpression(new) => new.callee.span() == span,
            AstKind::BinaryExpression(binary) => {
                (binary.operator == oxc_syntax::operator::BinaryOperator::Instanceof
                    && binary.right.span() == span)
                    // `x.constructor === Object`: a comparison reads no member.
                    || binary.operator.is_equality()
            }
            AstKind::UnaryExpression(unary) => unary.operator == UnaryOperator::Typeof,
            _ => false,
        }
    }

    /// ADR 1133 (#296 owed): whether the global `Reflect` at `span` is the object of a call of a member that only calls or tests:
    /// `Reflect.apply(f, t, args)` and `Reflect.construct(F, args)` call a function value the program names elsewhere (its own
    /// read is checked there), and `Reflect.has(o, k)` answers a boolean (its key is checked as a read key: a route, a private key, a
    /// spelling). `Reflect.get` hands back the member itself, and every name test of the scans (a key reader, `Object`, `Reflect`,
    /// `JSON`, a dispatcher, a timer, `eval`, `location`, `open`, a call-spelled sink) lives on a member named where it is read, which
    /// `Reflect.get(o, "constructor")` hides: it is a hazard (ADR 1133, round 1 F1; it recovered no proof). Any other member
    /// (`defineProperty`, `set`, `setPrototypeOf`, `deleteProperty`, `ownKeys`, …), a member read but not called, a computed member,
    /// or a key nothing proves: fail closed.
    fn reflect_member_reads_only(
        &mut self,
        id: NodeId,
        span: oxc_span::Span,
        keys: &mut super::computed_keys::KeyProver<'_, 'a>,
    ) -> bool {
        let nodes = self.semantic.nodes();
        let AstKind::StaticMemberExpression(member) = nodes.parent_kind(id) else {
            return false;
        };
        if member.object.span() != span {
            return false;
        }
        let member_id = nodes.parent_id(id);
        let AstKind::CallExpression(call) = nodes.parent_kind(member_id) else {
            return false;
        };
        if call.callee.span() != member.span {
            return false;
        }
        match member.property.name.as_str() {
            "apply" | "construct" => true,
            "has" => {
                let Some(key) = call
                    .arguments
                    .get(1)
                    .and_then(|argument| argument.as_expression())
                else {
                    return false;
                };
                if call.arguments.iter().any(|argument| argument.is_spread()) {
                    return false;
                }
                let texts: Vec<String> = match literal_property_key(key) {
                    Some(text) => vec![text.to_string()],
                    None => match keys.key(key) {
                        Some((texts, _)) => {
                            texts.into_iter().map(|text| text.to_string()).collect()
                        }
                        None => return false,
                    },
                };
                for text in texts {
                    self.key(call.span, &text);
                }
                true
            }
            _ => false,
        }
    }

    /// ADR 1133: whether the value read at `span` may be called or kept other than by a call written on it: anything but the callee
    /// of a call, the target of a write, the operand of `typeof`, `!` or `void`, an equality's operand, a condition's test or a
    /// template's part.
    fn value_kept(&self, id: NodeId, span: oxc_span::Span) -> bool {
        match self.semantic.nodes().parent_kind(id) {
            AstKind::CallExpression(call) => call.callee.span() != span,
            AstKind::AssignmentExpression(assignment) => assignment.left.span() != span,
            AstKind::UnaryExpression(unary) => !matches!(
                unary.operator,
                UnaryOperator::Typeof
                    | UnaryOperator::LogicalNot
                    | UnaryOperator::Void
                    | UnaryOperator::Delete
            ),
            AstKind::BinaryExpression(binary) => !binary.operator.is_equality(),
            AstKind::IfStatement(statement) => statement.test.span() != span,
            AstKind::ConditionalExpression(conditional) => conditional.test.span() != span,
            AstKind::TemplateLiteral(_) => false,
            _ => true,
        }
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
    fn a_hook_on_a_literal_in_an_exported_function_is_contained_and_an_exported_binding_is_not() {
        let contained = |source: &str| {
            crate::ast::extract("case.tsx", source)
                .unwrap()
                .premise_sites
                .iter()
                .filter(|site| site.clause == PremiseClause::CoercionHook)
                .map(|site| site.contained)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            contained(
                "export function C() { const t = { then: (cb: () => void) => cb() } as unknown as PromiseLike<void>; t.then(() => {}); }"
            ),
            vec![true]
        );
        assert_eq!(
            contained("export const t = { then: (cb: () => void) => cb() }; t.then(() => {});"),
            vec![false]
        );
        assert_eq!(
            contained(
                "export function C() { const t = { then: (cb: () => void) => cb() }; return t; }"
            ),
            vec![false]
        );
    }

    #[test]
    fn a_dispatcher_handed_on_is_listed_and_a_written_dispatch_or_a_narrow_reflect_is_not() {
        for source in [
            "declare const el: any; el.dispatchEvent.call(el, new Event(\"x\"));",
            "declare const el: any; Reflect.apply(EventTarget.prototype.dispatchEvent, el, [new Event(\"x\")]);",
            "declare const el: any; const d = el[\"dispatchEvent\"];",
            "declare const el: any; const { dispatchEvent } = el;",
            "declare const el: any; const c = el.click.bind(el);",
        ] {
            assert!(
                sites(source)
                    .iter()
                    .any(|(clause, _)| *clause == PremiseClause::DispatchHandedOn),
                "{source}"
            );
        }
        let written = "declare const el: any; el.dispatchEvent(new Event(\"x\")); el?.click(); if (el.click) {}";
        assert!(
            !sites(written)
                .iter()
                .any(|(clause, _)| *clause == PremiseClause::DispatchHandedOn),
            "{written}"
        );
        // #296: `Reflect.apply`, `Reflect.construct` and `Reflect.has` under a literal key are no Reflect hazard; the key tested is a key.
        for source in [
            "declare const f: any; Reflect.apply(f, null, []);",
            "declare const F: any; Reflect.construct(F, []);",
            "declare const o: any; Reflect.has(o, \"y\");",
        ] {
            assert!(
                !has(source, PremiseClause::ProxyOrReflect, "Reflect"),
                "{source}"
            );
        }
        assert!(has(
            "declare const el: any; Reflect.has(el, \"_$$click\");",
            PremiseClause::PrivateKey,
            "_$$click"
        ));
        // Round 1 F1: `Reflect.get` hands the member back whatever its key (`Reflect.get({}, \"constructor\")`,
        // `Reflect.get(el, \"dispatchEvent\")`, `Reflect.get(globalThis, \"JSON\")`), and every name test lives on a member read where it
        // is named: a hazard under a literal, a proven or an unknown key.
        for source in [
            "declare const o: any; Reflect.get(o, \"x\");",
            "Reflect.get({}, \"constructor\");",
            "declare const el: any; (Reflect.get(el, \"dispatchEvent\") as any).call(el, new Event(\"x\"));",
            "(Reflect.get(globalThis, \"Object\") as any).values({});",
            "(Reflect.get(window, \"setTimeout\") as any)(\"code\");",
            "declare const o: any, k: string; Reflect.get(o, k);",
            "declare const o: any; Reflect.set(o, \"x\", 1);",
            "declare const o: any; Reflect.ownKeys(o);",
            "const R = Reflect;",
            "declare const o: any; const g = Reflect.get; g(o, \"x\");",
            "declare const o: any, m: string; (Reflect as any)[m](o);",
        ] {
            assert!(
                has(source, PremiseClause::ProxyOrReflect, "Reflect"),
                "{source}"
            );
        }
    }

    #[test]
    fn object_handed_on_is_object_assign_and_routes_through_members_are_the_globals() {
        // ADR 1133 (F6): `Object` kept or handed on may be called as `Object.assign` by any name.
        for source in [
            "const O = Object; O.assign(globalThis, { event: undefined });",
            "declare const f: any; f(Object);",
            "const O = (Object as any);",
            "const O = globalThis.Object;",
        ] {
            assert!(has(source, PremiseClause::ObjectAssign, "*"), "{source}");
        }
        assert!(has(
            "declare const a: any, b: any; globalThis.Object.assign(a, b);",
            PremiseClause::ObjectAssign,
            "*"
        ));
        assert!(has(
            "const put = Object[\"assign\"];",
            PremiseClause::KeyReader,
            "assign"
        ));
        assert!(has(
            "globalThis.Reflect.set(globalThis, \"event\", undefined);",
            PremiseClause::ProxyOrReflect,
            "Reflect"
        ));
        // N2: a value's `constructor` may be `Object`.
        assert!(has(
            "declare const el: any; ({}).constructor.values(el);",
            PremiseClause::KeyReader,
            "values"
        ));
        assert!(has(
            "declare const el: any; const C = el.constructor; C.assign(globalThis, {});",
            PremiseClause::ObjectAssign,
            "*"
        ));
        for source in [
            "declare const x: any; Object.freeze(x); const t = typeof Object; const i = x instanceof Object; Object(x); new Object();",
            "declare const x: any; const plain = x.constructor === Object; const n = x.constructor.name;",
            "declare const x: any; const k = Object[\"freeze\"](x);",
        ] {
            assert!(!has(source, PremiseClause::ObjectAssign, "*"), "{source}");
        }
    }

    #[test]
    fn a_dispatcher_by_a_built_key_or_a_pattern_is_handed_on_whether_kept_or_called() {
        // ADR 1133, round 1 F3: the dispatch census counts only dispatch calls spelled as dispatches; a key the program builds, and every
        // pattern form (a declaration, an assignment, a shorthand, a built key), takes the dispatcher out by a route it cannot read.
        for source in [
            "declare const el: any; (el as any)[\"dispatch\" + \"Event\"].call(el, new Event(\"x\"));",
            "declare const el: any; (el as any)[\"dispatch\" + \"Event\"](new Event(\"x\"));",
            "declare const el: any; (el as any)[\"cl\" + \"ick\"]();",
            "declare const el: any; let d: any; ({ dispatchEvent: d } = el as any); d.call(el, new Event(\"x\"));",
            "declare const el: any; let dispatchEvent: any; ({ dispatchEvent } = el as any);",
            "declare const el: any; const { [\"dispatch\" + \"Event\"]: d } = el as any;",
            "declare const el: any; let d: any; ({ [\"click\"]: d } = el as any);",
        ] {
            assert!(
                sites(source)
                    .iter()
                    .any(|(clause, _)| *clause == PremiseClause::DispatchHandedOn),
                "{source}"
            );
        }
        // A dispatch written as a name or a literal key is the census's: it stays no hazard.
        let written = "declare const el: any; el.dispatchEvent(new Event(\"x\")); el[\"click\"](); el?.[\"click\"](); if (el[\"click\"]) {}";
        assert!(
            !sites(written)
                .iter()
                .any(|(clause, _)| *clause == PremiseClause::DispatchHandedOn),
            "{written}"
        );
    }

    #[test]
    fn json_and_object_by_a_computed_member_an_alias_or_a_pattern_are_the_globals() {
        // ADR 1133, round 1 F4: the spellings round 2's N2 named, which the scans read only as the global name or a static member.
        for (source, clause, key) in [
            (
                "declare const el: any; globalThis.JSON.stringify(el, (_k: string, v: unknown) => v);",
                PremiseClause::KeyReader,
                "JSON.stringify",
            ),
            (
                "declare const el: any; (globalThis as any)[\"JSON\"].stringify(el, (_k: string, v: unknown) => v);",
                PremiseClause::KeyReader,
                "JSON.stringify",
            ),
            (
                "const J = JSON; J.stringify({}, null);",
                PremiseClause::KeyReader,
                "JSON",
            ),
            (
                "const { stringify } = JSON; stringify({}, null);",
                PremiseClause::KeyReader,
                "JSON",
            ),
            (
                "const J = globalThis.JSON;",
                PremiseClause::KeyReader,
                "JSON",
            ),
            (
                "const { JSON: J } = globalThis as any;",
                PremiseClause::KeyReader,
                "JSON",
            ),
            (
                "declare const el: any; ({} as any)[\"constructor\"].values(el);",
                PremiseClause::KeyReader,
                "values",
            ),
            (
                "declare const el: any; (globalThis as any)[\"Object\"].values(el);",
                PremiseClause::KeyReader,
                "values",
            ),
            (
                "declare const el: any; (globalThis as any)[\"Obj\" + \"ect\"].values(el);",
                PremiseClause::KeyReader,
                "values",
            ),
            (
                "const { Object: O } = globalThis as any; O.values({});",
                PremiseClause::ObjectAssign,
                "*",
            ),
            (
                "const { constructor: C } = {} as any; C.values({});",
                PremiseClause::ObjectAssign,
                "*",
            ),
            (
                "const { Reflect: R } = globalThis as any;",
                PremiseClause::ProxyOrReflect,
                "Reflect",
            ),
            (
                "const R = (globalThis as any)[\"Reflect\"];",
                PremiseClause::ProxyOrReflect,
                "Reflect",
            ),
        ] {
            assert!(has(source, clause, key), "{source}");
        }
        // `JSON` read as the object of a member, a one-argument `stringify` and `typeof`, a plain `constructor` read: no hazard.
        for source in [
            "declare const x: any; const s = JSON.stringify(x); const p = JSON.parse(s); const t = typeof JSON;",
            "declare const x: any; const s = globalThis.JSON.stringify(x);",
            "declare const x: any; const n = (x as any)[\"constructor\"][\"name\"];",
        ] {
            assert!(
                !sites(source).iter().any(|(clause, _)| matches!(
                    clause,
                    PremiseClause::KeyReader | PremiseClause::ObjectAssign
                )),
                "{source}"
            );
        }
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
        // Fable round 2, N1: a computed, optional or proven-text spelling, and a spread handing the replacer.
        for (source, key) in [
            ("Object[\"values\"](el);", "values"),
            ("(Object as typeof Object)?.[\"values\"](el);", "values"),
            ("Object[\"val\" + \"ues\"](el);", "values"),
            ("declare const k: string; Object[k](el);", "*"),
            ("JSON[\"stringify\"](el);", "stringify"),
            (
                "JSON.stringify(...([el, (k: string, v: unknown) => v] as const));",
                "JSON.stringify",
            ),
        ] {
            assert!(has(source, PremiseClause::KeyReader, key), "{source}");
        }
        assert!(
            !sites("Object[\"freeze\"](el);")
                .iter()
                .any(|(clause, _)| *clause == PremiseClause::KeyReader)
        );
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
