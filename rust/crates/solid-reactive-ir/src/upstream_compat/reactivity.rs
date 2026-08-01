//! The fine-grained rules decomposed out of upstream's `reactivity`.
//!
//! Upstream ships one rule reporting eight unrelated defects behind eight
//! message ids, so a project that disagrees with one of them has to silence
//! the other seven too. Here each is its own rule; the mapping is documented
//! in `docs/rules/README.md`.
//!
//! Two of the eight — the untracked read and the read after `await` — are the
//! engine's own analysis and are reported from the reactive IR as SC1001 and
//! SC1002, not from here. This module owns the rest.
//!
//! # Proven sources, not named ones
//!
//! Upstream decides what is reactive from syntax and convention: a variable
//! destructured from a call spelled `createSignal`, a parameter whose name
//! looks like `props`, a function whose name matches `/^(?:use|create)[A-Z]/`.
//! That is why it reports a memo passed to a helper but not the structurally
//! identical signal (upstream issue #182), and why a signal reached through a
//! wrapper like `makePersisted(createSignal(...))` defeats it entirely (#190).
//!
//! These rules ask the engine instead: [`UpstreamCompatContext::accessors`]
//! and `setters` are the sources it *proved*, through TypeScript symbol
//! resolution, package contracts and cross-file propagation. A local named
//! `createSignal` that is not Solid's is not a source here, and a signal that
//! arrives through three wrappers still is.

use solid_dialect::Execution;
use solid_facts::FileFacts;
use solid_facts::ast::{FunctionFact, IdentifierRole};
use solid_facts::core::Span;

use super::{UpstreamCompatContext, is_lowercase_led, text};
use crate::{ReactiveSourceKind, StaticViolation, known_primitive, location};

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    uncalled_accessor(file, context, violations);
    no_direct_mutation(file, context, violations);
    no_async_tracked_scope(file, context, violations);
    expected_function_got_expression(file, context, violations);
}

/// `v1/no-async-tracked-scope` — upstream's `noAsyncTrackedScope`.
///
/// An `async` function handed to a position the dialect tracks. A computation
/// collects dependencies only until its first suspension point, so every read
/// after an `await` subscribes to nothing and the scope silently stops
/// responding.
///
/// Which slot tracks is asked of the dialect rather than assumed, and that is
/// the whole reason this is a per-dialect rule: `createEffect`'s callback is
/// argument 0 in 1.x and argument 1 in 2.0, so a table baked for one version
/// reports the other's seed value as a tracked scope.
///
/// Scoped to functions written at the call. An identifier naming an `async`
/// function declared elsewhere is the same defect, but proving the binding
/// reaches this slot is interprocedural work the engine already does for
/// reads — [`crate::StaticViolation`]s for those surface as
/// `v1/reactive-read-after-await` instead, per-read and with the offending
/// read located.
fn no_async_tracked_scope(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let primitives = context.lookup.primitives(file);
    for (index, call) in file.ast.calls.iter().enumerate() {
        let Some(primitive) = known_primitive(&primitives.calls[index]) else {
            continue;
        };
        let name = primitives.calls[index]
            .as_ref()
            .map_or("", crate::PrimitiveName::as_str);
        for (slot, execution) in context.dialect.callback_executions(primitive) {
            if *execution != Execution::Tracked {
                continue;
            }
            let Some(argument) = call.arguments.get(*slot) else {
                continue;
            };
            let Some(function) = function_at(file, argument.span) else {
                continue;
            };
            if !function.r#async {
                continue;
            }
            violations.push(StaticViolation {
                id: "SC5004".into(),
                rule: "no-async-tracked-scope".into(),
                message: format!(
                    "this {name} scope is an async function; Solid tracks dependencies only up to the first await, so every reactive read after it subscribes to nothing"
                ),
                hint: format!(
                    "Keep the {name} scope synchronous and move the async work into createResource, whose source function stays tracked; read the resulting accessor from here."
                ),
                location: location(file.path.shared(), argument.span),
                analysis_context: String::new(),
                fixes: vec![],
            });
        }
    }
}

/// `v1/expected-function-got-expression` — upstream's
/// `expectedFunctionGotExpression`.
///
/// A native element's event-handler binding receiving an already-evaluated
/// expression: `onClick={count()}` binds whatever `count()` returned during
/// setup as the listener.
///
/// Narrower than upstream, deliberately. `onClick={makeHandler()}` is a
/// factory call and correct, and upstream cannot tell the two apart because it
/// works from syntax. Two things are proof here, and nothing else is reported:
/// the callee is a *proven* reactive accessor, so the call yields that
/// accessor's value; or TypeScript resolved the whole expression to a type
/// that is not callable. A factory whose return type is a function satisfies
/// neither and stays silent.
fn expected_function_got_expression(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    for element in &file.ast.jsx_elements {
        let element_name = text(file, element.name.span);
        // Components receive props, not listeners: `onFoo` on a component is
        // an ordinary prop and its value may legitimately be anything.
        if element_name.contains('.') || !is_lowercase_led(element_name) {
            continue;
        }
        for attribute in element
            .attributes
            .iter()
            .filter(|attribute| attribute.namespace.is_none())
        {
            let name = text(file, attribute.name);
            if !name.starts_with("on")
                || !name.as_bytes().get(2).is_some_and(u8::is_ascii_alphabetic)
            {
                continue;
            }
            let Some(expression) = attribute.expression else {
                continue;
            };
            // The binding must be a call spanning the whole expression. A bare
            // reference is the correct form, and a call merely *inside* the
            // expression — `onClick={() => save(id())}` — is the fix, not the
            // defect, so equality with the trimmed text is what matters.
            let trimmed = text(file, expression).trim();
            let Some(call) =
                file.ast.calls.iter().find(|call| {
                    expression.contains(call.span) && text(file, call.span) == trimmed
                })
            else {
                continue;
            };
            let proven_accessor = context
                .entities
                .at(file.path.as_str(), call.callee)
                .is_some_and(|symbol| {
                    context.accessors.contains_key(symbol)
                        && context.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
                });
            let proven_not_callable = context
                .lookup
                .smallest_contained_descriptor(file.path.as_str(), expression)
                .is_some_and(|descriptor| is_not_callable(descriptor.text.as_ref()));
            if !proven_accessor && !proven_not_callable {
                continue;
            }
            violations.push(StaticViolation {
                id: "SC1007".into(),
                rule: "expected-function-got-expression".into(),
                message: format!(
                    "{name} is given the result of calling {}, not a function; the call runs once during setup and its value is bound as the listener",
                    text(file, call.callee)
                ),
                hint: format!(
                    "Wrap it: {name}={{() => {}}}, or pass the function itself uncalled.",
                    text(file, call.span)
                ),
                location: location(file.path.shared(), expression),
                analysis_context: String::new(),
                fixes: vec![],
            });
        }
    }
}

/// Whether a resolved type is provably not callable.
///
/// Conservative by construction: only the primitive types that can never hold
/// a function count. Anything structural, generic, aliased, or unresolved is
/// left alone, because a type this cannot read is not proof of anything.
fn is_not_callable(descriptor: &str) -> bool {
    matches!(
        descriptor,
        "string" | "number" | "boolean" | "bigint" | "symbol" | "void" | "null" | "undefined"
    ) || descriptor.starts_with('"')
        || descriptor.parse::<f64>().is_ok()
}

/// The function written at exactly this span, if the argument is a literal
/// function rather than a reference to one.
fn function_at(file: &FileFacts, span: Span) -> Option<&FunctionFact> {
    file.ast
        .functions
        .iter()
        .find(|function| function.span == span)
}

/// `v1/uncalled-accessor` — upstream's `badSignal`.
///
/// An accessor referenced where a *value* was meant: interpolated into a
/// template literal, used as an operand, used as a computed member key. The
/// accessor is a function, so the expression sees `function count() {...}`
/// rather than the number, and it never updates.
///
/// The positions are the ones upstream enumerates; what differs is the
/// premise. A reference only counts when the engine proved the symbol is an
/// accessor, so a plain function of the same shape is not reported, and an
/// accessor that reached this file through a helper or a re-export is.
fn uncalled_accessor(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    for identifier in &file.ast.identifiers {
        if identifier.role != IdentifierRole::Reference {
            continue;
        }
        let Some(symbol) = context.entities.at(file.path.as_str(), identifier.span) else {
            continue;
        };
        // Proven accessor, and specifically an accessor: a store is read by
        // path, not by call, so `store.items` is correct and not this rule's
        // business.
        let Some((name, declaration)) = context.accessors.get(symbol) else {
            continue;
        };
        let name = name.as_str();
        if context.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
            continue;
        }
        // A call of the accessor is the correct use; so is passing it on,
        // which hands the callee something it can call later.
        if is_called(file, identifier.span) || is_argument(file, identifier.span) {
            continue;
        }
        let Some(position) = value_position(file, identifier.span) else {
            continue;
        };
        violations.push(StaticViolation {
            id: "SC1005".into(),
            rule: "uncalled-accessor".into(),
            message: format!(
                "accessor {name:?} is used as a value in {position}; the expression receives the accessor function itself, not the value it holds, and never updates"
            ),
            hint: format!("Call it: {name}(). Passing {name} uncalled is only correct where the receiver calls it later."),
            location: location(file.path.shared(), identifier.span),
            analysis_context: String::new(),
            fixes: vec![],
        });
        let _ = declaration;
    }
}

/// `v1/no-direct-mutation` — upstream's `noWrite`.
///
/// A reactive value written through instead of through its setter: `count =
/// 2` on a signal accessor, or `props.value = x` / `store.a.b = x` on props
/// and stores, which are readonly proxies in Solid.
fn no_direct_mutation(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    for assignment in &file.ast.assignments {
        // The assignment target is either the reactive binding itself or the
        // object of a member chain rooted at one.
        let root = member_root(file, assignment.target).unwrap_or(assignment.target);
        let Some(symbol) = context.entities.at(file.path.as_str(), root) else {
            continue;
        };
        let Some((name, _)) = context.accessors.get(symbol) else {
            continue;
        };
        let name = name.as_str();
        let through_member = root != assignment.target;
        let kind = context.source_kinds.get(symbol).copied();
        let (message, hint) = if through_member {
            (
                format!(
                    "{name:?} is a reactive {} and is written through directly; Solid hands out a readonly proxy, so the write is dropped and nothing re-runs",
                    match kind {
                        Some(ReactiveSourceKind::Store) => "store",
                        _ => "value",
                    }
                ),
                match kind {
                    Some(ReactiveSourceKind::Store) => format!(
                        "Write through the store's setter: setStore(\"key\", value), or produce(draft => ...) for an in-place update. Direct assignment to {name} does not notify subscribers."
                    ),
                    _ => format!(
                        "Props are readonly by design: the parent owns the value. Lift the state to the parent and pass a callback down, rather than assigning to {name}."
                    ),
                },
            )
        } else {
            (
                format!(
                    "reactive accessor {name:?} is reassigned; the binding is replaced and every subscriber keeps reading the old accessor"
                ),
                format!("Call the setter returned beside it instead of rebinding {name}."),
            )
        };
        violations.push(StaticViolation {
            id: "SC2003".into(),
            rule: "no-direct-mutation".into(),
            message,
            hint,
            location: location(file.path.shared(), assignment.target),
            analysis_context: String::new(),
            fixes: vec![],
        });
    }
}

/// Whether this identifier is the callee of a call — the correct use.
fn is_called(file: &FileFacts, span: Span) -> bool {
    file.ast.calls.iter().any(|call| call.callee == span)
}

/// Whether this identifier is being passed to a call, which hands the
/// receiver something it can call later and is how accessors travel.
fn is_argument(file: &FileFacts, span: Span) -> bool {
    file.ast
        .calls
        .iter()
        .any(|call| call.arguments.iter().any(|argument| argument.span == span))
}

/// The value position this reference sits in, phrased for the message, or
/// `None` when the position does not demand a value.
///
/// Deliberately narrow. Every arm is a position where a function object is
/// provably not what the author meant: string interpolation renders
/// `function () {...}`, a computed key stringifies it, and the rest is
/// upstream's own enumeration. Anywhere else — a bare reference, a return, a
/// property value — passing the accessor on is idiomatic, and reporting it
/// would be the false positive upstream's issue #193 describes.
fn value_position(file: &FileFacts, span: Span) -> Option<&'static str> {
    // An untagged template literal stringifies each interpolation, so an
    // accessor there renders its own source text. A *tagged* one hands the
    // interpolations to the tag as values, which may legitimately call them —
    // and since a tag's quasi is also a template literal, the containment
    // check is what keeps `css`color: ${theme}`` out of this rule.
    let tagged = |slot: Span| {
        file.ast
            .tagged_templates
            .iter()
            .any(|template| template.span.contains(slot))
    };
    if file
        .ast
        .template_literals
        .iter()
        .any(|template| !tagged(template.span) && template.expressions.contains(&span))
    {
        return Some("a template literal");
    }
    if file
        .ast
        .members
        .iter()
        .any(|member| member.property == span && file.ast.computed_members.contains(&member.span))
    {
        return Some("a computed property access");
    }
    None
}

/// The object a member chain is rooted at: `store.a.b` → `store`.
fn member_root(file: &FileFacts, span: Span) -> Option<Span> {
    let mut current = file.ast.members.iter().find(|member| member.span == span)?;
    loop {
        match file
            .ast
            .members
            .iter()
            .find(|member| member.span == current.object)
        {
            Some(outer) => current = outer,
            None => return Some(current.object),
        }
    }
}
