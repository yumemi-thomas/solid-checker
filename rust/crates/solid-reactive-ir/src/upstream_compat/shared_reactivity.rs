//! Shared fine-grained rules decomposed out of upstream's `reactivity`.
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
//! Unlike the sibling modules, which port a 1.x-era ESLint surface, the
//! defects here are defects in both language versions — an accessor used as
//! a value, a proxy written through, a listener bound to a call's result —
//! so both dialects' catalogs carry them (1.x as `v1/<rule>`, 2.0 under the
//! checker's plain names, same SC codes so suppressions survive a
//! migration).
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
use solid_facts::ast::{ArgumentValueKind, IdentifierRole};
use solid_facts::core::Span;
use typefacts::{ArrayShape, Callability, ResolvedCallValidity};

use super::{
    UpstreamCompatContext, expression_array_shape, expression_runtime_value_domain,
    is_lowercase_led, jsx_name_is_type_checked, static_string_expression, text,
};
use crate::runtime_semantics::{RuntimeArgumentBehavior, argument_behavior};
use crate::{
    DirectMutationTarget, ReactiveSourceKind, StaticDefect, StaticDefectKind, known_primitive,
    location,
};

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    uncalled_accessor(file, context, defects);
    no_direct_mutation(file, context, defects);
    expected_function_got_expression(file, context, defects);
    reactive_source_uncaptured(file, context, defects);
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
/// `v1/package-contract-incomplete`: that rule fires once at an
/// undescribed import, this one fires where the missing description actually
/// costs certification.
///
/// Passing a source on is idiomatic — it is how accessors travel — so the
/// unknown callee is the entire premise. A helper defined in the project is
/// read directly and never reported here.
///
/// The premise is a callee whose behavior *could* be described: a package
/// export, or project code with no body here. A standard-library declaration
/// is neither -- no `solid-reactivity.json` entry can describe `setTimeout` --
/// so it is excluded by that compiler fact rather than by the bare-specifier
/// import test it replaces. Among the callees that remain, an exact
/// compiler-selected `ValueOnly` parameter certifies that the source is not
/// invoked; every other valid unresolved boundary remains an explicit
/// obligation, and invalid/recovery calls stay TypeScript-owned.
///
/// The argument must be a bare identifier. `entities.at` answers a call span
/// with the *callee's* symbol, so without that gate `console.log(count())`
/// reads as handing `count` itself to the callee.
fn reactive_source_uncaptured(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    // Nothing here can report unless some call hands a bare, proven, non-store
    // source to a callee. That is two map lookups per identifier argument, and
    // answering it first skips resolving the file's primitives -- the dominant
    // cost -- on every file that has no such call at all.
    let hands_over_a_source = |call: &solid_facts::ast::CallFact| {
        call.arguments.iter().any(|argument| {
            argument.value == ArgumentValueKind::Identifier
                && context
                    .entities
                    .at(file.path.as_str(), argument.span)
                    .is_some_and(|symbol| {
                        context.accessors.contains_key(symbol)
                            && context.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
                    })
        })
    };
    if !file.ast.calls.iter().any(hands_over_a_source) {
        return;
    }
    let primitives = context.lookup.primitives(file);
    for (index, call) in file.ast.calls.iter().enumerate() {
        if !hands_over_a_source(call) {
            continue;
        }
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
        if context.contracted.contains_key(callee_symbol) {
            continue;
        }
        let Some(resolved_call) = context.lookup.resolved_callee_call(file, call.callee) else {
            continue;
        };
        if resolved_call.validity != ResolvedCallValidity::Valid {
            continue;
        }
        // A standard-library callee is ambient: it belongs to no package, so
        // no `solid-reactivity.json` entry could ever describe it and the
        // finding would demand a fix nobody can apply. This is the exact
        // compiler fact that replaces the old bare-specifier import test --
        // which also excluded them, by accident of never reaching them.
        if resolved_call
            .declaration
            .as_ref()
            .is_some_and(|declaration| declaration.standard_library)
        {
            continue;
        }
        let callee_text = text(file, call.callee);
        for (argument_index, argument) in call.arguments.iter().enumerate() {
            // Only a bare reference to the source itself. A call, a member
            // read, or an expression built from one passes a *value*, whose
            // reactivity was already resolved before the callee saw it.
            // `entities.at` answers the callee's symbol for a call span, so
            // the argument's own kind is what enforces this -- without it
            // `console.log(count())` reads as handing `count` over.
            if argument.value != ArgumentValueKind::Identifier {
                continue;
            }
            let Some(symbol) = context.entities.at(file.path.as_str(), argument.span) else {
                continue;
            };
            let Some((name, _)) = context.accessors.get(symbol) else {
                continue;
            };
            if context.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                continue;
            }
            if argument_behavior(resolved_call, Some(Callability::Callable), argument_index)
                == Some(RuntimeArgumentBehavior::ValueOnly)
            {
                continue;
            }
            let name = name.as_str();
            defects.push(StaticDefect {
                kind: StaticDefectKind::ReactiveSourceUncaptured {
                    source: name.to_owned(),
                    callee: callee_text.to_owned(),
                },
                location: location(file.path.shared(), argument.span),
                analysis_context: String::new(),
                fixes: vec![],
                uncertain: false,
            });
        }
    }
}

/// `v1/reactive-handler-frozen` — upstream's
/// `expectedFunctionGotExpression`.
///
/// A native element's event-handler binding receiving an already-evaluated
/// expression: `onClick={count()}` binds whatever `count()` returned during
/// setup as the listener.
///
/// Narrower than upstream, deliberately. `onClick={makeHandler()}` is a
/// factory call and correct, and upstream cannot tell the two apart because it
/// works from syntax. The normal, declared attribute path reports only a
/// reactive source member whose callable value is frozen during setup.
/// Hyphenated attribute names form a separate boundary TypeScript does not
/// check: there the runtime value is classified as safe, invalid, or
/// unresolved against Solid's function-or-bound-pair representations.
fn expected_function_got_expression(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
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
            if !name.starts_with("on") {
                continue;
            }
            let Some(expression) = attribute.expression else {
                continue;
            };
            if context.dialect.static_event_values_are_attributes()
                && (static_string_expression(context, file, expression).is_some()
                    || super::solid1x_syntax::expression_is_static_literal(file, expression))
            {
                // Solid 1.x freezes this value into a plain attribute instead
                // of installing it as a listener. It is therefore not a
                // non-callable runtime listener, so this rule stays silent.
                continue;
            }
            // TypeScript deliberately skips every JSX attribute whose name
            // contains a hyphen. Solid's compiler does not: any `on` prefix
            // on a native element is lowered as an event listener. Keep the
            // uncovered runtime value in this rule's tri-state domain instead
            // of leaving `on-foo={someNumber}` to fail only when dispatched.
            if !jsx_name_is_type_checked(name) {
                let proof = unchecked_handler_value_proof(file, context, attribute, expression);
                match proof {
                    HandlerValueProof::Safe => {}
                    HandlerValueProof::Invalid | HandlerValueProof::Unresolved => {
                        defects.push(StaticDefect {
                            kind: StaticDefectKind::HandlerValueUnresolved {
                                attribute: name.to_owned(),
                                expression: text(file, expression).to_owned(),
                            },
                            location: location(file.path.shared(), expression),
                            analysis_context: "unchecked-native-handler-value".into(),
                            fixes: vec![],
                            uncertain: proof == HandlerValueProof::Unresolved,
                        });
                        continue;
                    }
                }
            }
            // A native listener receives its function value once during DOM
            // setup. Reading that function through reactive props/store state
            // here freezes the initial handler. The member root is a proven
            // source; a plain object member is left alone. Props follow the
            // caller classification: a handler prop every call site passes
            // statically is a plain property whose value never changes —
            // installing it once is exactly right and stays silent.
            let member_symbol = file
                .ast
                .members
                .iter()
                .find(|member| member.span == expression)
                .and_then(|_| member_root(file, expression))
                .and_then(|root| Some((root, context.entities.at(file.path.as_str(), root)?)));
            if let Some((root, symbol)) = member_symbol {
                let uncertain = if context.accessors.contains_key(symbol) {
                    Some(false)
                } else if let Some((_, declaration)) = context.prop_sources.get(symbol) {
                    if context.uncertain_prop_sources.contains(symbol) {
                        Some(true)
                    } else {
                        let property = file
                            .ast
                            .members
                            .iter()
                            .find(|member| {
                                member.object == root && expression.contains(member.span)
                            })
                            .map(|member| text(file, member.property))
                            .unwrap_or_default();
                        match context.props_reactivity.prop_use(declaration, property) {
                            crate::source_discovery::PropUse::Static => None,
                            crate::source_discovery::PropUse::Reactive => Some(false),
                            crate::source_discovery::PropUse::Unknown => Some(true),
                        }
                    }
                } else {
                    None
                };
                if let Some(uncertain) = uncertain {
                    defects.push(StaticDefect {
                        kind: StaticDefectKind::ReactiveHandlerRead {
                            attribute: name.to_owned(),
                            expression: text(file, expression).to_owned(),
                        },
                        location: location(file.path.shared(), expression),
                        analysis_context: String::new(),
                        fixes: vec![],
                        uncertain,
                    });
                    continue;
                }
                if context.accessors.contains_key(symbol)
                    || context.prop_sources.contains_key(symbol)
                {
                    // A proven-static prop member: not a defect, and not the
                    // call-result shape below either.
                    continue;
                }
            }
            // The call-result arm was removed on 2026-08-17 under AGENTS.md's
            // absolute rule. It fired for a handler expression that is itself a
            // call, on either of two triggers, and neither survives:
            //
            //   * the expression is *proven non-callable* -- which is exactly
            //     when TypeScript reports TS2322 ("Type 'number' is not
            //     assignable to type 'EventHandlerUnion<…>'") at that same
            //     attribute, so the finding was the type error restated;
            //   * the callee is a proven accessor -- which lands on the same
            //     TS2322 whenever the accessor's value is not callable
            //     (`onClick={count()}` with `count: Accessor<number>`). The one
            //     spelling TypeScript permits is an accessor holding a
            //     function, `onClick={handler()}`, and there the finding would
            //     be *wrong*: a JSX attribute expression is a tracked read, so
            //     that handler does update.
            //
            // What remains above is the reactive-handler-read arm, where the
            // value is callable, TypeScript is silent, and the claim is that a
            // native listener is installed once from a value that changes.
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HandlerValueProof {
    Safe,
    Invalid,
    Unresolved,
}

fn unchecked_handler_value_proof(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    attribute: &solid_facts::ast::JsxAttributeFact,
    expression: Span,
) -> HandlerValueProof {
    let runtime = if attribute.runtime_type_escape {
        file.ast.peel_ts_sugar_span(expression)
    } else {
        expression
    };
    if context
        .lookup
        .entity_at(file.path.as_str(), runtime)
        .is_none()
    {
        return HandlerValueProof::Unresolved;
    }
    let Some(domain) = expression_runtime_value_domain(context, file, runtime) else {
        return HandlerValueProof::Unresolved;
    };
    if domain.unknown {
        return HandlerValueProof::Unresolved;
    }
    // `never` cannot reach the listener. A callable (optionally absent) is a
    // valid handler, while absence sentinels disable the handler harmlessly.
    if (!domain.may_be_callable && !domain.may_be_undefined && !domain.may_be_other)
        || (domain.may_be_callable && !domain.may_be_other)
        || (!domain.may_be_callable && domain.may_be_undefined && !domain.may_be_other)
    {
        return HandlerValueProof::Safe;
    }
    if !domain.may_be_callable
        && !domain.may_be_undefined
        && handler_value_is_absent_sentinel(context, file, runtime)
    {
        return HandlerValueProof::Safe;
    }
    if domain.may_be_callable || domain.may_be_undefined {
        return HandlerValueProof::Unresolved;
    }
    match expression_array_shape(context, file, runtime) {
        // A non-callable, non-array runtime value cannot be either accepted
        // handler representation: a function or `[handler, data]`.
        Some(ArrayShape::NotArray) => HandlerValueProof::Invalid,
        // An array may or may not be a valid two-slot bound-handler pair.
        Some(ArrayShape::Array | ArrayShape::Mixed | ArrayShape::Unknown) | None => {
            HandlerValueProof::Unresolved
        }
    }
}

/// Whether the value handed to the listener is one of the absent-handler
/// sentinels this rule certifies: `null` or `false`.
///
/// Proved from the literal the program actually passes -- written at the
/// attribute, or as the initializer of an immutable binding this reference
/// resolves to -- and never from a rendered type name. `type Falsy = false`
/// renders as `Falsy`, so a `TypeDescriptor.text` test reported the alias and
/// certified the literal for one runtime value. The runtime value domain
/// cannot answer this: `null` and `false` both arrive as `may_be_other`,
/// indistinguishable from a number.
fn handler_value_is_absent_sentinel(
    context: &UpstreamCompatContext<'_>,
    file: &FileFacts,
    span: Span,
) -> bool {
    fn is_sentinel_literal(file: &FileFacts, span: Span) -> bool {
        matches!(
            text(file, file.ast.peel_ts_sugar_span(span)).trim(),
            "null" | "false"
        )
    }
    if is_sentinel_literal(file, span) {
        return true;
    }
    context
        .lookup
        .binding_at_reference(file.path.as_str(), span)
        .is_some_and(|(binding_file, binding, _)| {
            binding.immutable
                && binding
                    .initializer
                    .is_some_and(|initializer| is_sentinel_literal(binding_file, initializer))
        })
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
    defects: &mut Vec<StaticDefect>,
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
        let Some((name, _)) = context.accessors.get(symbol) else {
            continue;
        };
        let name = name.as_str();
        if context.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
            continue;
        }
        // A call of the accessor is the correct use; so is passing it on,
        // which hands the callee something it can call later. The exemption
        // is deliberately unconditional: TypeFacts' resolved-call parameter
        // callability could in principle prove a callee's slot non-callable,
        // but a non-callable parameter receiving an accessor is already a
        // type error TypeScript reports itself, and recovery-signature
        // resolutions would make the "proof" wrong exactly where it fired.
        if is_called(file, identifier.span) || is_argument(file, identifier.span) {
            continue;
        }
        let Some(position) = value_position(file, identifier.span) else {
            continue;
        };
        defects.push(StaticDefect {
            kind: StaticDefectKind::UncalledAccessor {
                name: name.to_owned(),
                position: position.to_owned(),
            },
            location: location(file.path.shared(), identifier.span),
            analysis_context: String::new(),
            fixes: vec![],
            uncertain: false,
        });
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
    defects: &mut Vec<StaticDefect>,
) {
    for assignment in &file.ast.assignments {
        // The assignment target is either the reactive binding itself or the
        // object of a member chain rooted at one. The root is resolved with
        // the same reference-index fallback `uncalled-accessor` uses: an
        // assignment target is an ordinary operator operand, exactly the
        // expression shape entity facts may skip (see the field doc on
        // `source_reference_index`).
        let root = member_root(file, assignment.target).unwrap_or(assignment.target);
        let Some(symbol) = source_symbol_at(context, file, root) else {
            continue;
        };
        // Proven accessors and proven props roots are both readonly through
        // members; `prop_sources` covers the props objects the accessor map
        // does not.
        let (name, _) = match context.accessors.get(symbol) {
            Some(source) => source,
            None => {
                let Some(source) = context.prop_sources.get(symbol) else {
                    continue;
                };
                source
            }
        };
        let props = !context.accessors.contains_key(symbol);
        let name = name.as_str();
        let through_member = root != assignment.target;
        let kind = context.source_kinds.get(symbol).copied();
        // `createMutable` is the one Solid 1.x source whose proxy is designed
        // to be written through directly. It still behaves like a store for
        // reads, but treating that shared read shape as readonly recreates
        // eslint-plugin-solid issue #99.
        if through_member
            && context
                .source_primitives
                .get(symbol)
                .is_some_and(|primitive| primitive.as_str() == "createMutable")
        {
            continue;
        }
        // A props root only misbehaves when written *through* — rebinding a
        // local `props`-like identifier is shadowing, not a dropped write.
        if props && !through_member {
            continue;
        }
        let target = if through_member {
            if props {
                DirectMutationTarget::Props
            } else if kind == Some(ReactiveSourceKind::Store) {
                DirectMutationTarget::Store
            } else {
                DirectMutationTarget::ReactiveValue
            }
        } else {
            DirectMutationTarget::AccessorBinding
        };
        // A write to the root record's *own* property is TS2540 where the
        // dialect's store type is `Readonly` at that level -- 2.0 -- so it is
        // TypeScript's, not this rule's. The readonly-ness is shallow, so a
        // nested record and a props object both stay here; 1.x's store type is
        // mutable throughout, and its rule is unaffected.
        //
        // The root must be a bare identifier. `(state as { count: number }).count
        // = 1` casts the readonly away, so TypeScript falls silent there while
        // the write is still dropped at runtime -- and `member_root` resolves
        // through the cast, so a span comparison alone would have handed that
        // case to a TypeScript diagnostic that does not exist.
        if target == DirectMutationTarget::Store
            && context.dialect.store_root_properties_are_readonly()
            && file
                .ast
                .members
                .iter()
                .find(|member| member.span == assignment.target)
                .is_some_and(|member| member.object == root)
            && file
                .ast
                .identifiers
                .iter()
                .any(|identifier| identifier.span == root)
        {
            continue;
        }
        // 2.0 write-enables the original store proxy for the duration of its
        // own setter's draft callback (probed on rc.0: the write commits;
        // through another store's setter it is silently dropped). A write
        // through the store rooted lexically inside that store's own setter
        // callback is therefore correct code, not a dropped write.
        if target == DirectMutationTarget::Store
            && context.dialect.store_setter_callback_enables_proxy_writes()
            && inside_own_setter_callback(file, context, assignment.target, symbol)
        {
            continue;
        }
        defects.push(StaticDefect {
            kind: StaticDefectKind::DirectMutation {
                name: name.to_owned(),
                target,
            },
            location: location(file.path.shared(), assignment.target),
            analysis_context: String::new(),
            fixes: vec![],
            uncertain: props && context.uncertain_prop_sources.contains(symbol),
        });
    }
}

/// Whether `target` (an assignment through a store proxy rooted at
/// `store_symbol`) sits lexically inside a callback passed to that store's
/// *own* setter.
///
/// The pairing is proven, not guessed: the callee must resolve to the second
/// slot of the exact array destructuring whose first slot is `store_symbol` —
/// `const [store, setStore] = createStore(...)`. Another store's setter, a
/// same-spelled local, or an eagerly evaluated argument (no function between
/// the assignment and the argument) all fail the proof and keep the finding.
fn inside_own_setter_callback(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    target: Span,
    store_symbol: &crate::SymbolId,
) -> bool {
    file.ast.arguments_containing(target).any(|(call, index)| {
        let argument = &call.arguments[index];
        // The write must run when the setter invokes the callback, not while
        // building the argument list.
        if !file
            .ast
            .functions_within(argument.span)
            .any(|function| function.body.contains(target))
        {
            return false;
        }
        let Some((binding_file, binding, callee_symbol)) = context
            .lookup
            .binding_at_reference(file.path.as_str(), call.callee)
        else {
            return false;
        };
        if binding.shape != solid_facts::ast::BindingShape::Array {
            return false;
        }
        let slot_symbol = |slot: usize| {
            binding
                .array_slots
                .get(slot)
                .and_then(Option::as_ref)
                .and_then(|name| context.entities.at(binding_file.path.as_str(), name.span))
        };
        slot_symbol(1) == Some(&callee_symbol) && slot_symbol(0) == Some(store_symbol)
    })
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
            solid_facts::ast::CoerciveOperandKind::StringConcatenation => "a string concatenation",
            solid_facts::ast::CoerciveOperandKind::LogicalNot => "a logical-not operator",
            solid_facts::ast::CoerciveOperandKind::NumericCoercion => "a numeric coercion",
        });
    }

    // Narrowed 2026-08-17 under AGENTS.md's absolute rule. Three of this
    // rule's six positions are ones the type system closes, in *both*
    // dialects, so they are gone:
    //
    //   a native JSX attribute       `<div title={label} />` is TS2322 --
    //                                `Accessor<string>` is not assignable to
    //                                `string` (1.x) or `string |
    //                                RemoveAttribute` (2.0);
    //   a class object value         `<div class={{ active: count }} />` is
    //                                TS2322 against `Record<string, boolean>`
    //                                in 2.0, the only dialect where the
    //                                position was enabled;
    //   a computed property access   `table[label]` is TS2538, "cannot be used
    //                                as an index type".
    //
    // What remains are the positions where an accessor is *legal* to
    // TypeScript and silently wrong at runtime -- see the returns below. This
    // also removed the last consumers of the dialect's
    // `class_object_values_are_truthiness_coerced` and
    // `native_children_attribute_invokes_functions` predicates, which went with
    // them.
    //
    // The native-attribute position has one surviving shape, and it is the
    // boundary of the argument above: TypeScript does not check a JSX attribute
    // name containing a hyphen at all (a deliberate exemption for HTML's own
    // hyphenated attributes), so `<div data-count={count} />` is accepted and
    // the accessor is stringified into the attribute with nothing to say so.
    if file.ast.jsx_elements.iter().any(|element| {
        is_lowercase_led(text(file, element.name.span))
            && element.attributes.iter().any(|attribute| {
                attribute.namespace.is_none()
                    && attribute.expression == Some(span)
                    && !super::jsx_name_is_type_checked(text(file, attribute.name))
            })
    }) {
        return Some("a native JSX attribute");
    }
    // An untagged template literal stringifies each interpolation, so an
    // accessor there renders its own source text. A *tagged* one hands the
    // interpolations to the tag as values, which may legitimately call
    // them. The fact table records only untagged literals — a tag's quasi
    // is filtered out at collection — so `css`color: ${theme}`` stays out
    // of this rule while an untagged template nested inside a tagged
    // interpolation still reports.
    if file
        .ast
        .template_literals
        .iter()
        .any(|template| template.expressions.contains(&span))
    {
        return Some("a template literal");
    }
    // The third position the narrowing above removed: a computed key is
    // TS2538 ("Type 'Accessor<string>' cannot be used as an index type") in
    // both dialects, so it never reaches this function's callers any more.
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
