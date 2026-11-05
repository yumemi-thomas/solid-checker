//! ADR 0643: what puts a member on an object where no Get or write is followed
//! to it. A prototype that is a store makes every Get and write of the object
//! the store's (`({ __proto__: store }).v`, `Object.create(store).v = 1`), and a
//! signal pair installed as an accessor makes `o.a = 2` the setter's call
//! (`Object.defineProperty(o, "a", { get, set })`). Chrome warns or throws at
//! each; the object is a plain local to every rule. An install is flow
//! insensitive, so this asks only what was installed: a function literal or a
//! declared function has a role of its own, and a prototype proved to hold no
//! proxy and no reactive member installs nothing. Anything else is an
//! obligation at the install, wherever the object goes.

use crate::{StaticDefect, StaticDefectKind, location};
use solid_facts::ast::{ArgumentFact, RuntimeValueKind};
use solid_facts::core::Span;
use solid_facts::{FileFacts, ast::CallFact};

const DEPTH: usize = 6;

/// Names one global reflective operation; a project `Object` is none of them.
fn global_receiver(file: &FileFacts, span: Span) -> Option<&str> {
    let span = file.ast.peel_ts_sugar_span(span);
    let text = file.source_text(span)?;
    let name = text
        .strip_prefix("globalThis.")
        .or_else(|| text.strip_prefix("window."))
        .unwrap_or(text);
    if !matches!(name, "Object" | "Reflect") {
        return None;
    }
    let head = file
        .ast
        .member_at(span)
        .map_or(span, |member| file.ast.peel_ts_sugar_span(member.object));
    file.ast
        .reference_declaration(head)
        .is_none()
        .then_some(name)
}

/// The key a member names: its property, or a computed member's literal key.
fn member_name<'f>(file: &'f FileFacts, member: &solid_facts::ast::MemberFact) -> Option<&'f str> {
    if file
        .ast
        .computed_members
        .binary_search(&member.span)
        .is_ok()
    {
        return file
            .ast
            .literal_computed_members
            .iter()
            .find(|literal| literal.span == member.span)
            .map(|literal| literal.key.as_str());
    }
    file.source_text(member.property)
}

/// The value of a `const` an identifier names, or the expression itself.
fn resolved(file: &FileFacts, span: Span, depth: usize) -> Option<Span> {
    let span = file.ast.peel_ts_sugar_span(span);
    if depth == 0 || !file.ast.identifiers.iter().any(|name| name.span == span) {
        return Some(span);
    }
    let declaration = file.ast.reference_declaration(span)?;
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.iter().any(|name| name.span == declaration)
    });
    match binding.and_then(|binding| binding.initializer) {
        Some(value) => resolved(file, value, depth - 1),
        None => Some(span),
    }
}

fn own_properties(
    file: &FileFacts,
    literal: Span,
) -> impl Iterator<Item = &solid_facts::ast::ObjectPropertyFact> {
    let properties = &file.ast.object_properties;
    properties.iter().filter(move |property| {
        literal.contains(property.span)
            && !properties.iter().any(|outer| {
                outer.span != property.span
                    && literal.contains(outer.span)
                    && outer.span.contains(property.span)
            })
    })
}

fn closed_literal(file: &FileFacts, span: Span) -> bool {
    file.ast
        .object_get_shapes
        .iter()
        .any(|shape| shape.span == span && shape.closed)
}

/// A function with a body of its own: its reads and writes are judged in it.
fn declared_function(file: &FileFacts, span: Span) -> bool {
    file.ast.function_at(span).is_some()
        || file
            .ast
            .reference_declaration(span)
            .is_some_and(|declaration| {
                file.ast.functions.iter().any(|function| {
                    function
                        .name
                        .as_ref()
                        .is_some_and(|name| name.span == declaration)
                })
            })
}

/// An installed `get` or `set`: nothing, or a function with a body of its own.
fn slot_is_clear(file: &FileFacts, value: Span, kind: RuntimeValueKind) -> bool {
    matches!(
        kind,
        RuntimeValueKind::Function | RuntimeValueKind::Primitive | RuntimeValueKind::Nullish
    ) || resolved(file, value, DEPTH).is_some_and(|value| declared_function(file, value))
}

/// A descriptor whose `get` and `set` are clear. A data descriptor is
/// `o.k = value` by another name, and that is not this question.
fn descriptor_is_clear(file: &FileFacts, span: Span) -> bool {
    let Some(literal) = resolved(file, span, DEPTH) else {
        return false;
    };
    closed_literal(file, literal)
        && own_properties(file, literal).all(|property| {
            !matches!(
                file.source_text(property.key)
                    .map(|key| key.trim_matches(['\'', '"'])),
                Some("get" | "set")
            ) || slot_is_clear(file, property.value, property.value_kind)
        })
}

fn descriptor_map_is_clear(file: &FileFacts, span: Span) -> bool {
    let Some(literal) = resolved(file, span, DEPTH) else {
        return false;
    };
    closed_literal(file, literal)
        && own_properties(file, literal).all(|property| descriptor_is_clear(file, property.value))
}

/// A value proved to be no proxy and to hold none, so that a Get or a write
/// through it as a prototype stays on plain objects: `null`, a literal of
/// functions and primitives, a `.prototype`, `Object.getPrototypeOf(x)` (a
/// reactive prototype was installed somewhere, and is judged there), or
/// `Object.create` of such a value.
fn prototype_is_plain(file: &FileFacts, span: Span, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let peeled = file.ast.peel_ts_sugar_span(span);
    if matches!(file.source_text(peeled), Some("null" | "undefined")) {
        return true;
    }
    let Some(value) = resolved(file, peeled, depth) else {
        return false;
    };
    if closed_literal(file, value) {
        return own_properties(file, value).all(|property| {
            matches!(
                property.value_kind,
                RuntimeValueKind::Function
                    | RuntimeValueKind::Primitive
                    | RuntimeValueKind::Nullish
            )
        });
    }
    if let Some(member) = file.ast.member_at(value) {
        return member_name(file, member) == Some("prototype");
    }
    let Some(call) = file.ast.call_at(value) else {
        return false;
    };
    let Some(member) = file.ast.member_at(file.ast.peel_ts_sugar_span(call.callee)) else {
        return false;
    };
    if global_receiver(file, member.object) != Some("Object") {
        return false;
    }
    match member_name(file, member) {
        Some("getPrototypeOf") => true,
        Some("create") => {
            call.arguments.len() == 1
                && !call.arguments[0].spread
                && prototype_is_plain(file, call.arguments[0].span, depth - 1)
        }
        _ => false,
    }
}

fn argument(call: &CallFact, index: usize) -> Option<&ArgumentFact> {
    if call
        .arguments
        .iter()
        .take(index + 1)
        .any(|argument| argument.spread)
    {
        return None;
    }
    call.arguments.get(index)
}

/// Whether a call of the named global operation installs nothing unproved.
fn call_is_clear(file: &FileFacts, receiver: &str, name: &str, call: &CallFact) -> bool {
    let proto = |index| {
        argument(call, index).is_some_and(|value| prototype_is_plain(file, value.span, DEPTH))
    };
    match (receiver, name) {
        ("Object", "create") => {
            proto(0)
                && (call.arguments.len() < 2
                    || argument(call, 1).is_some_and(|map| descriptor_map_is_clear(file, map.span)))
        }
        ("Object" | "Reflect", "setPrototypeOf") => proto(1),
        ("Object" | "Reflect", "defineProperty") => {
            argument(call, 2).is_some_and(|descriptor| descriptor_is_clear(file, descriptor.span))
        }
        ("Object", "defineProperties") => {
            argument(call, 1).is_some_and(|map| descriptor_map_is_clear(file, map.span))
        }
        // The optional fourth argument is the object the write lands on.
        ("Reflect", "set") => call.arguments.len() < 4 || proto(3),
        _ => true,
    }
}

fn installs(receiver: &str, name: &str) -> bool {
    match name {
        "setPrototypeOf" | "defineProperty" => true,
        "create" | "defineProperties" => receiver == "Object",
        "set" => receiver == "Reflect",
        _ => false,
    }
}

fn push(file: &FileFacts, site: Span, defects: &mut Vec<StaticDefect>) {
    defects.push(StaticDefect {
        kind: StaticDefectKind::ReactiveDispatchUnresolved {
            callee: file.source_text(site).unwrap_or("this install").to_owned(),
            member: None,
        },
        location: location(file.path.shared(), site),
        analysis_context: crate::INSTALLED_MEMBER.to_owned(),
        fixes: vec![],
        uncertain: true,
    });
}

/// ADR 0643's obligations of one file.
pub(crate) fn check_file(file: &FileFacts, defects: &mut Vec<StaticDefect>) {
    let ast = &file.ast;
    for property in &ast.object_properties {
        let key = file
            .source_text(property.key)
            .map(|key| key.trim_matches(['\'', '"']));
        if key == Some("__proto__")
            && !property.computed
            && property.shorthand_binding.is_none()
            && !prototype_is_plain(file, property.value, DEPTH)
        {
            push(file, property.span, defects);
        }
    }
    for assignment in &ast.assignments {
        let Some(member) = ast.member_at(ast.peel_ts_sugar_span(assignment.target)) else {
            continue;
        };
        if matches!(member_name(file, member), Some("__proto__" | "prototype"))
            && !prototype_is_plain(file, assignment.value_span, DEPTH)
        {
            push(file, member.span, defects);
        }
    }
    // The key handed on: `set("__proto__", store)` reaches `o[k] = v` in a
    // body this does not follow.
    for call in &ast.calls {
        for argument in &call.arguments {
            if matches!(
                file.source_text(argument.span),
                Some("\"__proto__\"" | "'__proto__'" | "`__proto__`")
            ) {
                push(file, argument.span, defects);
            }
        }
    }
    // Anything but a direct call reads the operation and loses its arguments.
    for binding in &ast.bindings {
        if binding
            .initializer
            .is_some_and(|value| global_receiver(file, value).is_some())
        {
            push(file, binding.pattern, defects);
        }
    }
    for member in &ast.members {
        let name = member_name(file, member);
        if matches!(name, Some("__defineGetter__" | "__defineSetter__")) {
            let call = ast
                .calls
                .iter()
                .find(|call| ast.peel_ts_sugar_span(call.callee) == member.span);
            if !call.is_some_and(|call| {
                argument(call, 1)
                    .is_some_and(|value| slot_is_clear(file, value.span, value.runtime_value_kind))
            }) {
                push(file, member.span, defects);
            }
            continue;
        }
        let Some(receiver) = global_receiver(file, member.object) else {
            continue;
        };
        let Some(name) = name.filter(|name| installs(receiver, name)) else {
            continue;
        };
        match ast
            .calls
            .iter()
            .find(|call| ast.peel_ts_sugar_span(call.callee) == member.span)
        {
            Some(call) if call_is_clear(file, receiver, name, call) => {}
            Some(call) => push(file, call.span, defects),
            None => push(file, member.span, defects),
        }
    }
}
