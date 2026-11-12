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
pub(crate) fn member_name<'f>(
    file: &'f FileFacts,
    member: &solid_facts::ast::MemberFact,
) -> Option<&'f str> {
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

/// The objects this file names that an install put a member on, for the Gets
/// and writes of them (`installed_object_accesses`).
#[derive(Default)]
struct Installed {
    /// An object's expression, and the install's own member (not an access).
    objects: Vec<(Span, Option<Span>)>,
    /// The functions whose `this` an install was handed (`None`: outside any
    /// function with a `this` of its own).
    receivers: Vec<Option<Span>>,
    /// An install operation was read and called where this does not follow:
    /// any object may be the one it lands on.
    anywhere: bool,
}

/// The innermost function with a `this` of its own around `span`.
fn this_function(file: &FileFacts, span: Span) -> Option<Span> {
    file.ast
        .functions
        .iter()
        .filter(|function| {
            function.kind != solid_facts::ast::FunctionKind::Arrow
                && function.span != span
                && function.span.contains(span)
        })
        .map(|function| function.span)
        .min_by_key(|function| function.end - function.start)
}

/// ADR 0925: the object an install was handed. `this` is the receiver of
/// the function around it and, where that function is a method of an object
/// literal, the literal (`t.install()`).
fn mark_object(file: &FileFacts, span: Span, installed: &mut Installed) {
    let span = file.ast.peel_ts_sugar_span(span);
    if file.source_text(span) != Some("this") {
        installed.objects.push((span, None));
        return;
    }
    let function = this_function(file, span);
    installed.receivers.push(function);
    let Some(function) = function else {
        return;
    };
    let Some(literal) = file
        .ast
        .object_properties
        .iter()
        .find(|property| property.value == function)
        .and_then(|property| {
            file.ast
                .object_get_shapes
                .iter()
                .map(|shape| shape.span)
                .filter(|object| object.contains(property.span))
                .min_by_key(|object| object.end - object.start)
        })
    else {
        return;
    };
    installed.objects.push((literal, None));
    // The literal's other methods see the same `this`.
    for property in own_properties(file, literal) {
        if file
            .ast
            .function_at(property.value)
            .is_some_and(|method| method.kind != solid_facts::ast::FunctionKind::Arrow)
        {
            installed.receivers.push(Some(property.value));
        }
    }
}

/// ADR 0925: what a call of an install operation put a member on: the
/// object it was handed (`shift` arguments in, past `.call`'s receiver), and
/// for `Object`'s forms the call itself, which returns that object.
fn mark_targets(
    file: &FileFacts,
    receiver: &str,
    name: &str,
    call: &CallFact,
    shift: usize,
    installed: &mut Installed,
) {
    if receiver == "Object" {
        installed.objects.push((call.span, None));
    }
    if name == "create" || name == "set" {
        return;
    }
    match argument(call, shift) {
        Some(object) => mark_object(file, object.span, installed),
        None => installed.anywhere = true,
    }
}

/// ADR 0925: the calls through a name an install operation was read into
/// (`const dp = Object.defineProperty`, `const { defineProperty } = Object`):
/// each is that install. Any other use of the name loses its arguments.
fn follow_name(
    file: &FileFacts,
    receiver: &str,
    name: &str,
    declaration: Span,
    installed: &mut Installed,
    defects: &mut Vec<StaticDefect>,
) {
    let ast = &file.ast;
    for &(reference, _) in ast
        .reference_declarations
        .iter()
        .filter(|(_, target)| *target == declaration)
    {
        match ast
            .calls
            .iter()
            .find(|call| ast.peel_ts_sugar_span(call.callee) == reference)
        {
            Some(call) if call_is_clear(file, receiver, name, call) => {}
            Some(call) => {
                push(file, call.span, defects);
                mark_targets(file, receiver, name, call, 0, installed);
            }
            None => installed.anywhere = true,
        }
    }
}

/// ADR 0925: the uses of a name the global itself was read into
/// (`const O = Object`): `O.defineProperty(…)` is that install, another
/// member names no install, and any other use hands the operations on.
fn follow_receiver(
    file: &FileFacts,
    receiver: &str,
    declaration: Span,
    installed: &mut Installed,
    defects: &mut Vec<StaticDefect>,
) {
    let ast = &file.ast;
    for &(reference, _) in ast
        .reference_declarations
        .iter()
        .filter(|(_, target)| *target == declaration)
    {
        let Some(member) = ast
            .members
            .iter()
            .find(|member| ast.peel_ts_sugar_span(member.object) == reference)
        else {
            installed.anywhere = true;
            continue;
        };
        let Some(name) = member_name(file, member).filter(|name| installs(receiver, name)) else {
            continue;
        };
        match ast
            .calls
            .iter()
            .find(|call| ast.peel_ts_sugar_span(call.callee) == member.span)
        {
            Some(call) if call_is_clear(file, receiver, name, call) => {}
            Some(call) => {
                push(file, call.span, defects);
                mark_targets(file, receiver, name, call, 0, installed);
            }
            None => {
                push(file, member.span, defects);
                follow_read(file, receiver, name, member.span, installed, defects);
            }
        }
    }
}

/// ADR 0925: an install operation read and not called: through a `const`
/// alias or `.call`, the calls are followed; anything else lands anywhere.
fn follow_read(
    file: &FileFacts,
    receiver: &str,
    name: &str,
    member: Span,
    installed: &mut Installed,
    defects: &mut Vec<StaticDefect>,
) {
    let ast = &file.ast;
    let alias = ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding
                .initializer
                .is_some_and(|value| ast.peel_ts_sugar_span(value) == member)
    });
    if let Some(name_span) = alias.and_then(|binding| binding.names.first()) {
        follow_name(file, receiver, name, name_span.span, installed, defects);
        return;
    }
    let called = ast.members.iter().find(|outer| {
        ast.peel_ts_sugar_span(outer.object) == member && member_name(file, outer) == Some("call")
    });
    match called.and_then(|outer| {
        ast.calls
            .iter()
            .find(|call| ast.peel_ts_sugar_span(call.callee) == outer.span)
    }) {
        Some(call) => mark_targets(file, receiver, name, call, 1, installed),
        None => installed.anywhere = true,
    }
}

/// ADR 0643's obligations of one file.
pub(crate) fn check_file(file: &FileFacts, defects: &mut Vec<StaticDefect>) {
    let ast = &file.ast;
    // The objects a store prototype or an accessor was put on, for the Gets
    // and writes of them this file names (`installed_object_accesses`).
    let mut installed = Installed::default();
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
            if let Some(object) = ast
                .object_get_shapes
                .iter()
                .map(|shape| shape.span)
                .filter(|object| object.contains(property.span))
                .min_by_key(|object| object.end - object.start)
            {
                installed.objects.push((object, Some(property.span)));
            }
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
            installed
                .objects
                .push((ast.peel_ts_sugar_span(member.object), Some(member.span)));
        }
    }
    // The key handed on: `set("__proto__", store)` reaches `o[k] = v` in a
    // body this does not follow.
    let mut key_handed = false;
    for call in &ast.calls {
        for argument in &call.arguments {
            if matches!(
                file.source_text(argument.span),
                Some("\"__proto__\"" | "'__proto__'" | "`__proto__`")
            ) {
                push(file, argument.span, defects);
                key_handed = true;
            }
        }
    }
    // Where the key is handed on, any object written through a key no
    // literal names may be the one it lands on.
    if key_handed {
        for assignment in &ast.assignments {
            let target = ast.peel_ts_sugar_span(assignment.target);
            let Some(member) = ast.member_at(target) else {
                continue;
            };
            let literal_key = ast
                .literal_computed_members
                .iter()
                .any(|literal| literal.span == member.span);
            if ast.computed_members.binary_search(&member.span).is_ok() && !literal_key {
                installed
                    .objects
                    .push((ast.peel_ts_sugar_span(member.object), Some(member.span)));
            }
        }
    }
    // Anything but a direct call reads the operation and loses its arguments,
    // unless the read is a destructured name whose calls are followed.
    for binding in &ast.bindings {
        let Some(receiver) = binding
            .initializer
            .and_then(|value| global_receiver(file, value))
        else {
            continue;
        };
        push(file, binding.pattern, defects);
        if binding.shape == solid_facts::ast::BindingShape::Identifier {
            if let Some(name) = binding.names.first() {
                follow_receiver(file, receiver, name.span, &mut installed, defects);
            }
            continue;
        }
        for name in &binding.names {
            match binding
                .object_slots
                .iter()
                .find(|slot| slot.local.span == name.span)
            {
                Some(slot) if installs(receiver, &slot.property) => follow_name(
                    file,
                    receiver,
                    &slot.property,
                    name.span,
                    &mut installed,
                    defects,
                ),
                Some(_) => {}
                None => installed.anywhere = true,
            }
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
                if call.is_some() {
                    let object = ast.peel_ts_sugar_span(member.object);
                    if file.source_text(object) == Some("this") {
                        mark_object(file, object, &mut installed);
                    } else {
                        installed.objects.push((object, Some(member.span)));
                    }
                } else {
                    installed.anywhere = true;
                }
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
            Some(call) => {
                push(file, call.span, defects);
                mark_targets(file, receiver, name, call, 0, &mut installed);
            }
            None => {
                push(file, member.span, defects);
                follow_read(file, receiver, name, member.span, &mut installed, defects);
            }
        }
    }
    installed_object_accesses(file, &installed, defects);
}

/// ADR 0643 amendment: each Get or write this file writes of an object a
/// store prototype or an accessor was put on, at the member's property
/// (where V8 places the access, and so Chrome's frame): the object itself, a
/// binding it was declared or assigned in (the install's own object binding),
/// a `const` that names one, `this` in the function an install was handed it
/// (and, for a method of a literal, the literal), or a call handed it whole
/// (`merge(o, …).v`, whose result reads through it, placed at that call).
/// Where the `__proto__` key is handed on, every object a non-literal key
/// writes is such an object; where an install operation is called where this
/// does not follow (ADR 0925), every access is. The install's obligation does
/// not say which access runs where; each access is its own, so a rule's site
/// is never left with no row.
fn installed_object_accesses(
    file: &FileFacts,
    installed: &Installed,
    defects: &mut Vec<StaticDefect>,
) {
    let ast = &file.ast;
    if installed.objects.is_empty() && installed.receivers.is_empty() && !installed.anywhere {
        return;
    }
    let sites = |member: Span| {
        installed
            .objects
            .iter()
            .any(|&(_, site)| site == Some(member))
    };
    if installed.anywhere {
        for member in &ast.members {
            if !sites(member.span) {
                push(file, member.property, defects);
            }
        }
        return;
    }
    // The declaration a span names: an identifier's, or the binding a value
    // initializes.
    let declaration_of = |span: Span| {
        let span = ast.peel_ts_sugar_span(span);
        ast.reference_declaration(span).or_else(|| {
            ast.bindings_initializer_containing(span)
                .find(|binding| {
                    binding
                        .initializer
                        .is_some_and(|value| ast.peel_ts_sugar_span(value) == span)
                })
                .and_then(|binding| binding.names.first())
                .map(|name| name.span)
        })
    };
    let objects = installed
        .objects
        .iter()
        .map(|&(object, _)| object)
        .collect::<Vec<_>>();
    let mut declarations = objects
        .iter()
        .filter_map(|&object| declaration_of(object))
        .collect::<Vec<_>>();
    let is_this = |span: Span| {
        file.source_text(span) == Some("this")
            && installed.receivers.contains(&this_function(file, span))
    };
    let is_object = |span: Span, declarations: &[Span]| {
        let span = ast.peel_ts_sugar_span(span);
        objects.contains(&span)
            || is_this(span)
            || declarations.iter().any(|&declaration| {
                span == declaration || ast.reference_declaration(span) == Some(declaration)
            })
    };
    // ADR 0925: a `const` holding an installed object names it too
    // (`const self = this`, `const u = t`).
    for _ in 0..DEPTH {
        let aliases = ast
            .bindings
            .iter()
            .filter(|binding| {
                binding.immutable
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding
                        .initializer
                        .is_some_and(|value| is_object(value, &declarations))
            })
            .filter_map(|binding| binding.names.first().map(|name| name.span))
            .filter(|name| !declarations.contains(name))
            .collect::<Vec<_>>();
        if aliases.is_empty() {
            break;
        }
        declarations.extend(aliases);
    }
    for member in &ast.members {
        if sites(member.span) {
            continue;
        }
        let receiver = ast.peel_ts_sugar_span(member.object);
        // The object itself, written or read through: Chrome's frame is the
        // member's property.
        if is_object(receiver, &declarations) {
            push(file, member.property, defects);
            continue;
        }
        // A call handed the object reads through it in its own body (a
        // package's proxy): Chrome's frame is the call's.
        let through_call = ast.calls.iter().any(|call| {
            call.span == receiver
                && call
                    .arguments
                    .iter()
                    .any(|argument| !argument.spread && is_object(argument.span, &declarations))
        });
        if through_call {
            push(file, receiver, defects);
        }
    }
}
