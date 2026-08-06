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
    reactive_source_uncaptured(file, context, violations);
    untracked_derived_function(file, context, violations);
}

/// `v1/untracked-derived-function` — upstream's `untrackedReactive`.
///
/// A function that derives from reactive state and is only ever called where
/// nothing tracks. Wrapping reads in a function defers them, but deferral only
/// pays off if something eventually calls it in a tracking scope; otherwise
/// the derivation reads its inputs once per call and subscribes to nothing.
///
/// # Why this one is narrow
///
/// Every other rule here proves something local and positive. This one has to
/// prove a *negative* — that no tracking scope anywhere ever calls the
/// function — and a wrong negative reports the single most common shape in
/// Solid code. So it fires only when every use is enumerable and every one of
/// them is untracked:
///
/// - the function is bound inside another function, so no reference to it can
///   exist outside this file;
/// - every reference is the callee of a direct call, never an argument, never
///   a return value, never a JSX child — anything else could hand it to a
///   tracking scope this cannot see;
/// - every call sits directly in the declaring function's own body, not in a
///   nested one, since a nested function may itself be a callback somewhere
///   tracked;
/// - and none is inside JSX, which tracks.
///
/// A single reference that fails any of those abandons the function rather
/// than guessing. That leaves the textbook case and little else, which is the
/// right trade for a rule whose false positives would land on correct code.
fn untracked_derived_function(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let candidates = file
        .ast
        .bindings
        .iter()
        .filter_map(|binding| {
            if !binding.initializer_function {
                return None;
            }
            let (Some(initializer), [declared]) = (binding.initializer, binding.names.as_slice())
            else {
                return None;
            };
            let function = function_at(file, initializer)?;
            // Bound inside another function: references cannot escape the
            // file except by being passed or returned, both refused below.
            let enclosing = crate::containing_ast_function(&file.ast, binding.declaration)?;
            let symbol = context
                .entities
                .at(file.path.as_str(), declared.span)?
                .clone();
            Some((binding, declared, function, enclosing, symbol))
        })
        .collect::<Vec<_>>();

    // Derivation is transitive: `bar = () => foo()` derives whenever `foo`
    // does. Only reads/calls directly owned by the candidate function count;
    // a dormant nested function is code the outer function never executes.
    let mut derived = std::collections::HashSet::new();
    loop {
        let mut changed = false;
        for (_, _, function, _, symbol) in &candidates {
            if derived.contains(symbol) {
                continue;
            }
            let direct_source = file.ast.identifiers.iter().any(|identifier| {
                identifier.role == IdentifierRole::Reference
                    && function.body.contains(identifier.span)
                    && crate::containing_ast_function(&file.ast, identifier.span)
                        .is_some_and(|owner| owner.span == function.span)
                    && context
                        .entities
                        .at(file.path.as_str(), identifier.span)
                        .is_some_and(|read| {
                            read != symbol
                                && (context.accessors.contains_key(read)
                                    || context.prop_sources.contains_key(read))
                        })
            });
            let derived_call = file.ast.calls.iter().any(|call| {
                function.body.contains(call.span)
                    && crate::containing_ast_function(&file.ast, call.span)
                        .is_some_and(|owner| owner.span == function.span)
                    && context
                        .entities
                        .at(file.path.as_str(), call.callee)
                        .is_some_and(|callee| derived.contains(callee))
            });
            if direct_source || derived_call {
                changed |= derived.insert(symbol.clone());
            }
        }
        if !changed {
            break;
        }
    }

    for (_, declared, _, enclosing, symbol) in candidates {
        if !derived.contains(&symbol) {
            continue;
        }
        let mut calls = 0usize;
        let enumerable = file
            .ast
            .identifiers
            .iter()
            .filter(|identifier| {
                identifier.role == IdentifierRole::Reference
                    && context.entities.at(file.path.as_str(), identifier.span) == Some(&symbol)
            })
            .all(|identifier| {
                let Some(call) = file
                    .ast
                    .calls
                    .iter()
                    .find(|call| call.callee == identifier.span)
                else {
                    return false; // referenced without being called
                };
                let directly_enclosed = crate::containing_ast_function(&file.ast, call.span)
                    .is_some_and(|owner| owner.span == enclosing.span);
                if !directly_enclosed || within_jsx(file, call.span) {
                    return false;
                }
                calls += 1;
                true
            });
        if !enumerable || calls == 0 {
            continue;
        }
        let name = text(file, declared.span);
        violations.push(StaticViolation {
            id: "SC1006".into(),
            rule: "untracked-derived-function".into(),
            message: format!(
                "{name} derives from reactive state but every call to it is untracked, so its reads subscribe to nothing and the derivation never updates"
            ),
            hint: format!(
                "Call {name} from a tracking scope — JSX, a createMemo, or a createEffect callback — or inline the value if a one-off read at setup is what was meant."
            ),
            location: location(file.path.shared(), declared.span),
            analysis_context: String::new(),
            fixes: vec![],
        });
    }
}

/// Whether a span sits anywhere inside JSX, which is a tracking scope.
///
/// Fragments track their children exactly as elements do and live in their
/// own table, so both are consulted; checking only elements reported a
/// derived function rendered through `<>{…}</>` as never tracked.
fn within_jsx(file: &FileFacts, span: Span) -> bool {
    file.ast
        .jsx_elements
        .iter()
        .any(|element| element.span.contains(span))
        || file
            .ast
            .jsx_fragments
            .iter()
            .any(|fragment| fragment.contains(span))
}

/// `v1/reactive-source-uncaptured` — the uncertifiable half of upstream's
/// `reactivity`.
///
/// A proven reactive source handed, uncalled, to a callee nothing in the
/// analysis can describe: not one of the dialect's primitives, no body in the
/// project, no package-contract summary. The callee may call it in a tracking
/// scope, read it once and sever the reactivity, or store it for later — and
/// which of those it does decides whether the surrounding code is correct.
///
/// This reports the *gap*, not a defect, which is why it is uncertifiable
/// rather than a violation. It is the call-site companion to
/// `v1/package-contract-export-missing`: that rule fires once at an
/// undescribed import, this one fires where the missing description actually
/// costs certification.
///
/// Passing a source on is idiomatic — it is how accessors travel — so the
/// unknown callee is the entire premise. A helper defined in the project is
/// read directly and never reported here.
///
/// Scoped to callees imported from a package, because those are the callees
/// the fix applies to: the hint says "describe this export in the package's
/// contract", which is only actionable for a package export. An ambient
/// global (`setTimeout`, `console.log`, an array method) comes from no
/// package, so reporting it demanded a contract nobody can write — the rule
/// stays silent there and the reads flowing through remain uncertified
/// rather than reported.
fn reactive_source_uncaptured(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    // Symbols bound by value imports from bare specifiers — the only callees
    // whose reactive behaviour a package contract could ever describe.
    let package_imported: std::collections::HashSet<&crate::SymbolId> = file
        .ast
        .imports
        .iter()
        .filter(|import| !import.module.starts_with('.') && !import.module.starts_with('/'))
        .flat_map(|import| &import.bindings)
        .filter(|binding| !binding.type_only)
        .filter_map(|binding| context.entities.at(file.path.as_str(), binding.local.span))
        .collect();
    if package_imported.is_empty() {
        return;
    }
    let primitives = context.lookup.primitives(file);
    for (index, call) in file.ast.calls.iter().enumerate() {
        // A dialect primitive is described by definition, and a call the
        // engine resolved into the project has a body it already walked.
        if known_primitive(&primitives.calls[index]).is_some()
            || context
                .lookup
                .function_called_at(file.path.as_str(), call.callee)
                .is_some()
        {
            continue;
        }
        let callee_symbol = context.entities.at(file.path.as_str(), call.callee);
        let Some(callee_symbol) = callee_symbol else {
            continue;
        };
        if !package_imported.contains(callee_symbol)
            || context.contracted.contains_key(callee_symbol)
        {
            continue;
        }
        let callee_text = text(file, call.callee);
        for argument in &call.arguments {
            // Only a bare reference to the source itself. A call, a member
            // read, or an expression built from one passes a *value*, whose
            // reactivity was already resolved before the callee saw it.
            let Some(symbol) = context.entities.at(file.path.as_str(), argument.span) else {
                continue;
            };
            let Some((name, _)) = context.accessors.get(symbol) else {
                continue;
            };
            if context.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                continue;
            }
            let name = name.as_str();
            violations.push(StaticViolation {
                id: "SC9011".into(),
                rule: "reactive-source-uncaptured".into(),
                message: format!(
                    "the reactive source {name:?} is passed to {callee_text}, whose reactive behaviour is not described anywhere: it has no body in this project, no package contract entry, and is not a Solid primitive; whether reads through it stay tracked cannot be certified"
                ),
                hint: format!(
                    "Describe {callee_text} in its package's solid-reactivity.json — which arguments it tracks and what it returns — or keep the function in the project so its body is analysed. See docs/package-contracts.md."
                ),
                location: location(file.path.shared(), argument.span),
                analysis_context: String::new(),
                fixes: vec![],
            });
        }
    }
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
        for slot in 0..call.arguments.len() {
            if !context
                .dialect
                .callback_tracks_reads_at(primitive, slot, call.arguments.len())
            {
                continue;
            }
            let argument = &call.arguments[slot];
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
            // A native listener receives its function value once during DOM
            // setup. Reading that function through reactive props/store state
            // here freezes the initial handler. The member root is a proven
            // source; a plain object member is left alone.
            let reactive_member = file
                .ast
                .members
                .iter()
                .find(|member| member.span == expression)
                .and_then(|_| member_root(file, expression))
                .and_then(|root| context.entities.at(file.path.as_str(), root))
                .is_some_and(|symbol| {
                    context.accessors.contains_key(symbol)
                        || context.prop_sources.contains_key(symbol)
                });
            if reactive_member {
                violations.push(StaticViolation {
                    id: "SC1007".into(),
                    rule: "expected-function-got-expression".into(),
                    message: format!(
                        "{name} reads {} once during DOM setup; later reactive updates cannot replace the installed listener",
                        text(file, expression)
                    ),
                    hint: format!(
                        "Wrap the read so it happens when the event fires: {name}={{event => {}(event)}}.",
                        text(file, expression)
                    ),
                    location: location(file.path.shared(), expression),
                    analysis_context: String::new(),
                    fixes: vec![],
                });
                continue;
            }
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
/// template literal, used as a coercive operand, used as a computed member
/// key, or assigned to a native JSX value attribute. The accessor is a
/// function, so the expression sees `function count() {...}` rather than the
/// current value, and it never updates.
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
        let Some(symbol) = source_symbol_at(context, file, identifier.span) else {
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

fn source_symbol_at<'a>(
    context: &'a UpstreamCompatContext<'_>,
    file: &FileFacts,
    span: Span,
) -> Option<&'a crate::SymbolId> {
    context.entities.at(file.path.as_str(), span).or_else(|| {
        context
            .source_reference_index
            .get(file.path.as_str())
            .and_then(|by_range| by_range.get(&(u64::from(span.start), u64::from(span.end))))
    })
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
    if let Some(operand) = file
        .ast
        .coercive_operands
        .iter()
        .find(|operand| operand.span == span)
    {
        return Some(match operand.kind {
            solid_facts::ast::CoerciveOperandKind::Binary => "a binary operator",
            solid_facts::ast::CoerciveOperandKind::Unary => "a unary operator",
        });
    }

    // Component props are lazy getters and may intentionally receive an
    // accessor. Native attributes are different: the compiler hands the
    // expression's value to the DOM operation, so a bare accessor is
    // stringified/assigned as the function object. Callback-like attributes
    // remain excluded because their contract expects a function.
    if file.ast.jsx_elements.iter().any(|element| {
        let element_name = text(file, element.name.span);
        is_lowercase_led(element_name)
            && element.attributes.iter().any(|attribute| {
                let name = text(file, attribute.name);
                attribute.namespace.is_none()
                    && attribute.expression == Some(span)
                    && name != "ref"
                    && !(name.starts_with("on")
                        && name.as_bytes().get(2).is_some_and(u8::is_ascii_alphabetic))
            })
    }) {
        return Some("a native JSX attribute");
    }

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
