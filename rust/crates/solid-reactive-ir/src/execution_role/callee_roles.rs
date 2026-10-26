//! Callee-use-site roles: how a callee's parameters use a callback and when the callback runs.

use super::*;

/// Whether the function written around `span` is, as a whole argument, the
/// callback of a primitive slot that creates its own tracked computation (an
/// effect or memo compute): `span` is in that computation's own body, not in a
/// wrapper such as `untrack` between it and the computation.
fn directly_in_tracked_compute(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> bool {
    let Some(function) = containing_ast_function(&file.ast, span) else {
        return false;
    };
    file.ast
        .arguments_containing(function.span)
        .any(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
                && crate::call_primitive_name_cached(
                    lookup,
                    file,
                    call,
                    entities,
                    symbol_names,
                    lookup.dialect,
                )
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| {
                    callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                        && lookup
                            .dialect
                            .callback_semantics_at(primitive, index, call.arguments.len())
                            .tracks_reads
                })
        })
}

/// Whether a compiler-tracked JSX region that lies inside `argument` holds
/// `span`: the callback wrote JSX, and that JSX tracks its own expressions.
fn tracked_region_within(file: &solid_facts::FileFacts, argument: Span, span: Span) -> bool {
    file.compiler.tracked_regions.iter().any(|region| {
        region.span.contains(span) && argument.contains(region.span) && region.span != argument
    })
}

/// ADR 0401: the role of a callback that clears tracking where its call is in a
/// tracked scope (`runWithOwner`): untracked with no window, unless the call
/// itself runs after an await its async function reaches. Past one the code runs
/// from a promise continuation, outside every tracking scope and strict-read
/// window, which is `reactive-read-after-await`'s and not this rule's; where an
/// await only may precede it, the role is unknown (ADR 0354).
pub(crate) fn unwindowed_role_at_call(
    file: &solid_facts::FileFacts,
    call: Span,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    // A call `tsc` rejects is `tsc`'s subject (invariant 1): code inside its
    // arguments is claimed by no one here. A one-argument `createEffect` is
    // TS2554 and its compute would otherwise still read as tracked.
    if file.ast.arguments_containing(call).any(|(enclosing, _)| {
        lookup
            .resolved_callee_call(file, enclosing.callee)
            .is_some_and(|resolved| resolved.validity != typefacts::ResolvedCallValidity::Valid)
    }) {
        return ExecutionRole::Unreported;
    }
    // A function literal invoked in place runs where its call is written, so
    // the awaits that precede that call count too (ADR 0340's reach).
    let mut at = call;
    let mut suspended = false;
    for _ in 0..8 {
        if follows_unconditional_await(file, at)
            || follows_dominating_await(file, at)
            || follows_try_head_await(file, at, lookup)
        {
            return ExecutionRole::Unreported;
        }
        suspended |= follows_any_suspension(file, at) || may_follow_own_await(file, at);
        let Some(function) = containing_ast_function(&file.ast, at) else {
            break;
        };
        let Some(invocation) = file
            .ast
            .calls
            .iter()
            .find(|invocation| file.ast.peel_ts_sugar_span(invocation.callee) == function.span)
        else {
            break;
        };
        at = invocation.span;
    }
    if suspended {
        ExecutionRole::Unknown
    } else {
        ExecutionRole::UntrackedUnwindowed
    }
}

pub(super) fn semantic_role_before_awaits(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> ExecutionRole {
    // Before every semantic path, because the semantic paths answer a question
    // that no longer applies. They classify *how* code executes — the callback
    // that will invoke it, the primitive whose argument it is, the owner it
    // answers to — and a discarded region is the compiler saying the code is
    // not there to execute. Left below them, a read inside a deleted value
    // would take its role from a dialect-proven `untrack()` or effect-apply
    // position and be reported as a proven untracked read, or from a deferred
    // position and be silently *certified*: a positive claim about dead code
    // either way.
    if discarded_region_contains(file, span) {
        return ExecutionRole::DiscardedRendering;
    }
    // An instance field initializer is no function fact, so every path below
    // would place it -- and any literal written in it -- in the function the
    // class is declared in. It runs at construction instead, which no path
    // here places. (A static field or block runs where the class is
    // evaluated, in that function, so it keeps its placement.)
    if inside_instance_field(file, span) {
        return ExecutionRole::Unknown;
    }
    // Also ahead of every semantic path, for the mirror-image reason: code
    // inside a callback that runs on reads of a returned view (rc.9's `omit`
    // predicate) executes wherever the view is read, which none of the paths
    // below follow. Each of them would place it -- most often in the
    // enclosing component body, the one place it is least likely to run --
    // so the role is left unknown and projection claims nothing about it.
    if result_access_callback_contains(file, span, lookup) {
        return ExecutionRole::Unknown;
    }
    // A function argument the primitive wraps in a memo of its own (2.0's
    // `merge`) runs inside that computation, not in the caller's body.
    if file.ast.functions_body_containing(span).any(|function| {
        file.ast
            .arguments_containing(function.span)
            .any(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
                    && crate::call_primitive_name_cached(
                        lookup,
                        file,
                        call,
                        entities,
                        symbol_names,
                        lookup.dialect,
                    )
                    .as_ref()
                    .and_then(PrimitiveName::primitive)
                    .is_some_and(|primitive| {
                        lookup.dialect.wraps_function_arguments_in_memo(primitive)
                    })
            })
    }) {
        return ExecutionRole::Unknown;
    }
    if let Some(role) = context_provider_value_role(file, span, lookup) {
        return role;
    }
    if assigned_member_function_contains(file, span, entities) {
        return ExecutionRole::DeferredCallback;
    }
    if let Some(role) = named_callback_execution_role(file, span, lookup) {
        return role;
    }
    if let Some(role) = returned_callback_execution_role(file, span, lookup, classifying) {
        return role;
    }
    if let Some(role) = returned_factory_callback_execution_role(file, span, lookup, classifying) {
        return role;
    }
    if let Some(role) = inline_callback_execution_role(file, span, allowed, lookup, classifying) {
        return role;
    }
    let dialect = lookup.dialect;
    if file.ast.arguments_containing(span).any(|(call, index)| {
        crate::call_primitive_name_cached(lookup, file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .and_then(|primitive| effect_apply_argument(dialect, primitive, call.arguments.len()))
            == Some(index)
            && direct_callback_contains(file, call.arguments[index].span, span)
    }) {
        return ExecutionRole::EffectApply;
    }
    // ADR 0323 (review round 10): a callback that runs synchronously at its
    // call with the listener cleared but the strict-read window untouched
    // (`createRoot`, `runWithOwner`) reports where the call does. Reads only:
    // a write directly in a created root is `CreatedRoot`'s question, decided
    // before this function is asked.
    if !classifying.contains(&write_classification_marker())
        && let Some((call, clears_tracking)) =
            file.ast
                .arguments_containing(span)
                .find_map(|(call, index)| {
                    crate::call_primitive_name_cached(
                        lookup,
                        file,
                        call,
                        entities,
                        symbol_names,
                        dialect,
                    )
                    .as_ref()
                    .and_then(PrimitiveName::primitive)
                    .filter(|primitive| {
                        dialect.callback_inherits_calling_window(*primitive, index)
                            && callback_execution_at_call(file, call, *primitive, index, lookup)
                                .is_some()
                            && direct_callback_contains(file, call.arguments[index].span, span)
                    })
                    .map(|primitive| {
                        (
                            call,
                            dialect
                                .callback_clears_tracking_in_calling_window(primitive, index)
                                // JSX the callback writes tracks its own
                                // expressions, as it did before ADR 0401: the
                                // silent deferred role, whatever the call is.
                                .then(|| {
                                    tracked_region_within(file, call.arguments[index].span, span)
                                }),
                        )
                    })
                })
    {
        if clears_tracking == Some(true) {
            return ExecutionRole::DeferredCallback;
        }
        let clears_tracking = clears_tracking.is_some();
        let at_call = semantic_execution_role_within(
            file,
            call.callee,
            allowed,
            entities,
            symbol_names,
            lookup,
            classifying,
        );
        // The `allowed` deferred spans (an enclosing `createRoot` callback, an
        // `untrack`) outrank a tracked computation written inside them, which
        // answers `DeferredCallback` for the whole span. A clearing call
        // directly in such a computation is in its tracked scope all the same:
        // ask again without them, and keep the answer only if it is that
        // computation. Any other answer there (a call directly in an `untrack`
        // callback) stays the silent deferred role.
        let at_call = if clears_tracking
            && at_call == ExecutionRole::DeferredCallback
            && !allowed.is_empty()
            && directly_in_tracked_compute(file, call.callee, lookup, entities, symbol_names)
            && semantic_execution_role_within(
                file,
                call.callee,
                &[],
                entities,
                symbol_names,
                lookup,
                classifying,
            ) == ExecutionRole::TrackedJsx
        {
            ExecutionRole::TrackedJsx
        } else {
            at_call
        };
        return match at_call {
            // The listener is cleared and no window is open there. For a call
            // that clears tracking and leaves the window as it found it
            // (`runWithOwner`, ADR 0401) the read is stale and unwarned: it is
            // reported. For one the engine does not model that way
            // (`createRoot`) it stays the silent deferred role.
            ExecutionRole::TrackedJsx if clears_tracking => {
                // A call `tsc` rejects is its subject, not this rule's.
                if lookup
                    .resolved_callee_call(file, call.callee)
                    .is_some_and(|resolved| {
                        resolved.validity == typefacts::ResolvedCallValidity::Valid
                    })
                {
                    unwindowed_role_at_call(file, call.callee, lookup)
                } else {
                    ExecutionRole::Unreported
                }
            }
            ExecutionRole::TrackedJsx => ExecutionRole::DeferredCallback,
            // One-shot module evaluation is no component window, and the
            // role before this ADR was the silent deferred one.
            ExecutionRole::ModuleInitialization => ExecutionRole::Unreported,
            other => other,
        };
    }
    if file.ast.arguments_containing(span).any(|(call, index)| {
        crate::call_primitive_name_cached(lookup, file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                    && dialect.reports_untracked_reads_at(primitive, index, call.arguments.len())
                    && direct_callback_contains(file, call.arguments[index].span, span)
                    // JSX the map function returns tracks its own expressions.
                    && !(dialect.callback_runs_untracked_without_window(primitive, index)
                        && tracked_region_within(file, call.arguments[index].span, span))
            })
    }) {
        // A map function runs untracked in no window of its own (ADR 0401); the
        // other reporting slots (an apply, a render entry) run in one.
        return if file.ast.arguments_containing(span).any(|(call, index)| {
            crate::call_primitive_name_cached(lookup, file, call, entities, symbol_names, dialect)
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| {
                    dialect.callback_runs_untracked_without_window(primitive, index)
                        && direct_callback_contains(file, call.arguments[index].span, span)
                })
                // A call `tsc` rejects is its subject, not this rule's.
                && lookup.resolved_callee_call(file, call.callee).is_some_and(|resolved| {
                    resolved.validity == typefacts::ResolvedCallValidity::Valid
                })
        }) {
            ExecutionRole::UntrackedUnwindowed
        } else {
            ExecutionRole::UntrackedCallback
        };
    }
    if let Some(role) = forwarded_tracked_compute_role(file, span, lookup) {
        return role;
    }
    if allowed.iter().any(|region| region.contains(span)) {
        return ExecutionRole::DeferredCallback;
    }
    if let Some(role) = control_flow_execution_role(file, span, entities, symbol_names, dialect) {
        return role;
    }
    {
        // A function written as a component's child expression
        // (`<Kids>{(x) => …}</Kids>`) is the `children` prop's value, handed
        // to the component exactly as an attribute's is; the region lowering
        // that child does not run it. Such a region places nothing, and the
        // flow and lexical arms below decide where the function runs.
        let mut regions = file
            .compiler
            .tracked_regions
            .iter()
            .filter(|region| {
                region.span.contains(span)
                    && !component_child_function_within(file, region.span, span)
            })
            .peekable();
        if regions.peek().is_some() {
            // A region is tracked for its *expression*. A function literal
            // that is a JSX attribute's value inside it (an event handler, a
            // callback prop) is handed to the element and runs when something
            // calls it, not in the region's tracking pass; JSX written inside
            // such a function is a region of its own, which still tracks.
            return if regions
                .any(|region| !attribute_function_within(file, region.span, span, lookup))
            {
                ExecutionRole::TrackedJsx
            } else {
                ExecutionRole::Unknown
            };
        }
    }
    let mut tracked_callbacks = file.ast.arguments_containing(span).filter(|(call, index)| {
        matches!(
            call.arguments[*index].value,
            solid_facts::ast::ArgumentValueKind::Identifier
                | solid_facts::ast::ArgumentValueKind::Function
                | solid_facts::ast::ArgumentValueKind::AsyncFunction
        ) && crate::call_primitive_name_cached(lookup, file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                callback_execution_at_call(file, call, primitive, *index, lookup).is_some()
                    && dialect
                        .callback_semantics_at(primitive, *index, call.arguments.len())
                        .tracks_reads
            })
    });
    if let Some((call, index)) = tracked_callbacks.next() {
        // As for a tracked JSX region: a function handed on inside the
        // callback to a call not proven to invoke it there, or written as an
        // attribute value, runs when something calls it, not in the
        // callback's tracking pass (`client.subscribe(() => setX(..))`).
        return if attribute_function_within(file, call.arguments[index].span, span, lookup) {
            ExecutionRole::Unknown
        } else {
            ExecutionRole::TrackedJsx
        };
    }
    // Ahead of the compiler role and the lexical fallbacks, which place a
    // callback prop's body where the JSX is written.
    if let Some(role) = forwarded_event_prop_role(file, span, lookup) {
        return role;
    }
    if let Some(role) = callee_use_site_role(file, span, lookup, classifying) {
        return role;
    }
    if let Some(role) = invocation_flow_role(file, span, lookup, classifying) {
        return role;
    }
    let compiler_role = source_execution_role(file, span, allowed);
    if compiler_role != ExecutionRole::Unknown {
        return compiler_role;
    }
    // Ahead of the lexical fallbacks below, which place code by where it is
    // written: a callback a fresh-stack host scheduler runs is written in the
    // component body but does not execute there.
    if let Some(role) = fresh_stack_callback_role(file, span, lookup) {
        return role;
    }
    if lookup.inside_component(file, span) {
        return ExecutionRole::UntrackedRendering;
    }
    if lookup.inside_possible_component(file, span) {
        // Select the component execution branch so the relevant rule is
        // projected, while LocalAccess marks the read uncertifiable. Leaving
        // this Unknown would silently certify the ordinary-helper branch.
        return ExecutionRole::UntrackedRendering;
    }
    // Module initialization is an AST-proven one-shot execution context. It
    // is not a compiler-fact gap: no reactive owner or subscriber can be
    // active before a containing function is invoked.
    if !file.ast.any_function_body_containing(span) {
        return ExecutionRole::ModuleInitialization;
    }
    ExecutionRole::Unknown
}

/// The `classifying` entry that marks a write-legality classification, which
/// the ADR 0315 arm does not answer.
pub(super) fn write_classification_marker() -> (String, Span) {
    ("\0write-legality".to_owned(), Span::new(0, 0))
}

/// ADR 0315: code directly in a function literal that a project callee runs
/// only through one parameter, every call of which is in one proven role,
/// takes that role.
///
/// The literal is exactly one of: the argument at a slot of an exact project
/// callee; the value of a data or method property of a closed object literal
/// (no getter, spread, computed or duplicate key) that is exactly such an
/// argument; or the value of a project component's prop. The callee
/// ([`callee_parameter_use_role`]) uses the parameter only to read other
/// properties and to call the literal's: then the literal runs only where
/// those calls run. Each call's own role is classified in the callee's file,
/// not by where the literal is written, and must be an event, a deferred
/// callback or a tracked computation, the same for every call; that role is
/// the literal's. Anything else answers nothing here. Write legality never
/// asks this arm.
pub(super) fn callee_use_site_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    if classifying.contains(&write_classification_marker()) {
        return None;
    }
    let literal = containing_ast_function(&file.ast, span)?;
    if !literal.body.contains(span)
        || literal.r#async
        || literal.generator
        || lookup.function_component_status(file, literal) != ComponentStatus::No
    {
        return None;
    }
    let (callee_file, callee, index, field, siblings) = literal_use_site(file, literal, lookup)?;
    let key = (file.path.to_string(), literal.span);
    if !classifying.insert(key.clone()) {
        return None;
    }
    let receiver_safe = |name: &str| sibling_ignores_receiver(file, siblings, name, lookup);
    let role = callee_parameter_use_role(
        callee_file,
        callee,
        index,
        field.as_deref(),
        &receiver_safe,
        lookup,
        classifying,
    );
    classifying.remove(&key);
    role
}

/// Where a function literal is handed to a project callee, for
/// [`callee_use_site_role`]: the callee, the parameter slot, and the property
/// of that parameter the literal is (`None` for the parameter itself).
pub(super) fn literal_use_site<'a>(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'a>,
) -> Option<(
    &'a solid_facts::FileFacts,
    &'a solid_facts::ast::FunctionFact,
    usize,
    Option<String>,
    Siblings,
)> {
    let exact_argument = |value: Span| {
        file.ast.arguments_containing(value).find(|(call, index)| {
            let argument = &call.arguments[*index];
            !argument.spread
                && !call.construct
                && file.ast.peel_ts_sugar_span(argument.span) == value
                && lookup.primitive_at_call(file, call.span).is_none()
        })
    };
    let at_argument = |value: Span| {
        let (call, index) = exact_argument(value)?;
        let (callee_file, callee, exact) = project_callee_at_parameter(file, call, index, lookup)?;
        exact.then_some((callee_file, callee, index))
    };
    // The literal itself is the argument.
    if let Some((callee_file, callee, index)) = at_argument(literal.span) {
        return Some((callee_file, callee, index, None, Siblings::None));
    }
    // A property of an options object that is the argument.
    let property = file
        .ast
        .object_properties_containing(literal.span)
        .find(|property| {
            !property.computed && file.ast.peel_ts_sugar_span(property.value) == literal.span
        });
    let key = match (property, crate::owners::own_method_name(file, literal)) {
        (Some(property), _) => property.key,
        // An object literal's own method, not a class's.
        (None, Some(name))
            if file
                .ast
                .object_properties_containing(name.span)
                .any(|property| property.key == name.span) =>
        {
            name.span
        }
        (None, Some(_)) => return None,
        (None, None) => {
            // A project component's prop. A spread may carry anything, and a
            // non-arrow function value receives the props object as `this`
            // and could run another prop out of the callee's sight.
            let (element, prop) = component_prop_literal(file, literal.span)?;
            if !element.spreads.is_empty()
                || element.attributes.iter().any(|attribute| {
                    attribute
                        .expression
                        .is_some_and(|value| receiver_function_at(file, value))
                })
            {
                return None;
            }
            let (callee_file, callee) =
                lookup.function_called_at(file.path.as_str(), element.name.span)?;
            return Some((
                callee_file,
                callee,
                0,
                Some(prop.to_owned()),
                Siblings::Element(element.span),
            ));
        }
    };
    let object = file
        .ast
        .object_get_shapes
        .iter()
        .filter(|shape| shape.span.contains(key))
        .min_by_key(|shape| shape.span.end - shape.span.start)?;
    let name = static_key_text(file.source_text(key)?, false)?;
    let name = name.as_str();
    if !object.closed
        || object
            .properties
            .iter()
            .any(|property| property.getter.is_some())
        || !object
            .properties
            .iter()
            .any(|property| property.key.as_str() == name)
    {
        return None;
    }
    // A method or function expression in the literal receives the object as
    // `this` and could run a property out of the callee's sight.
    if file
        .ast
        .object_properties_within(object.span)
        .any(|property| property.method || receiver_function_at(file, property.value))
    {
        return None;
    }
    let (callee_file, callee, index) = at_argument(object.span)?;
    Some((
        callee_file,
        callee,
        index,
        Some(name.to_owned()),
        Siblings::Object(object.span),
    ))
}

/// The caller-side object a callee's parameter holds, for the receiver
/// question of [`callee_parameter_use_role`]: an options literal, a JSX
/// element's props, or nothing (the literal itself is the parameter).
#[derive(Clone, Copy)]
pub(super) enum Siblings {
    None,
    Object(Span),
    Element(Span),
}

/// Whether the caller's value for the property `name` of `siblings` cannot use
/// a `this` it is called with: an arrow (or a `const` bound to one) or a
/// non-function literal. An absent property proves nothing: code outside the
/// analyzed program may have installed one on `Object.prototype`
/// (`project-callback-uses`' `SiblingAbsent`).
pub(super) fn sibling_ignores_receiver(
    file: &solid_facts::FileFacts,
    siblings: Siblings,
    name: &str,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let arrow = |span: Span| {
        file.ast.functions.iter().any(|function| {
            function.span == span && function.kind == solid_facts::ast::FunctionKind::Arrow
        })
    };
    let arrow_value = |value: Span| {
        let value = file.ast.peel_ts_sugar_span(value);
        arrow(value)
            || lookup
                .binding_at_reference(file.path.as_str(), value)
                .is_some_and(|(binding_file, binding, _)| {
                    binding_file.path == file.path
                        && binding.immutable
                        && binding.shape == solid_facts::ast::BindingShape::Identifier
                        && binding.initializer.is_some_and(|initializer| {
                            arrow(file.ast.peel_ts_sugar_span(initializer))
                        })
                })
    };
    match siblings {
        Siblings::None => false,
        Siblings::Object(object) => {
            let direct = file
                .ast
                .object_properties
                .iter()
                .filter(|property| {
                    object.contains(property.span)
                        && !property.computed
                        && property_static_key(file, property).as_deref() == Some(name)
                        && !file.ast.object_get_shapes.iter().any(|inner| {
                            inner.span != object
                                && object.contains(inner.span)
                                && inner.span.contains(property.span)
                        })
                })
                .max_by_key(|property| property.span.start);
            match direct {
                None => false,
                Some(property) => {
                    !property.method
                        && (arrow_value(property.value)
                            || property.value_kind == solid_facts::ast::RuntimeValueKind::Primitive)
                }
            }
        }
        Siblings::Element(element) => {
            let Some(element) = file.ast.jsx_element_at(element) else {
                return false;
            };
            match element
                .attributes
                .iter()
                .rev()
                .find(|attribute| file.source_text(attribute.name) == Some(name))
            {
                None => false,
                Some(attribute) => match attribute.value_kind {
                    solid_facts::ast::JsxAttributeValueKind::String
                    | solid_facts::ast::JsxAttributeValueKind::Boolean => true,
                    solid_facts::ast::JsxAttributeValueKind::Expression => {
                        attribute.expression.is_some_and(arrow_value)
                    }
                    _ => false,
                },
            }
        }
    }
}

/// Whether `value` is exactly a function that binds its own `this`: not an
/// arrow.
pub(super) fn receiver_function_at(file: &solid_facts::FileFacts, value: Span) -> bool {
    let value = file.ast.peel_ts_sugar_span(value);
    file.ast.functions.iter().any(|function| {
        function.span == value && function.kind != solid_facts::ast::FunctionKind::Arrow
    })
}

/// The one role every call of `function`'s parameter `index` (or of its
/// property `field`) runs in, when the parameter is used for nothing else;
/// see [`callee_use_site_role`].
pub(super) fn callee_parameter_use_role(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    index: usize,
    field: Option<&str>,
    receiver_safe: &dyn Fn(&str) -> bool,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    if function.r#async || function.generator {
        return None;
    }
    let parameter = function.parameters.get(index)?;
    let [name] = parameter.names.as_slice() else {
        return None;
    };
    if parameter.shape != solid_facts::ast::BindingShape::Identifier
        || parameter.initializer.is_some()
        || (function.rest_parameter && index + 1 == function.parameters.len())
        || crate::indexes::binding_written(file, name.span)
        || file.ast.identifiers.iter().any(|identifier| {
            function.span.contains(identifier.span)
                && matches!(
                    file.source_text(identifier.span),
                    Some("eval" | "arguments")
                )
        })
    {
        return None;
    }
    let targets = file
        .ast
        .assignments
        .iter()
        .map(|assignment| assignment.target)
        .chain(file.ast.iteration_targets.iter().copied())
        .collect::<Vec<_>>();
    let mut role = None::<ExecutionRole>;
    let mut calls = 0usize;
    for (reference, declaration) in &file.ast.reference_declarations {
        if *declaration != name.span || *reference == name.span {
            continue;
        }
        if !function.body.contains(*reference) {
            return None;
        }
        let invoked =
            match field {
                None => *reference,
                Some(field) => {
                    let member =
                        file.ast.members.iter().find(|member| {
                            file.ast.peel_ts_sugar_span(member.object) == *reference
                        })?;
                    if file
                        .ast
                        .computed_members
                        .binary_search(&member.span)
                        .is_ok()
                        || targets.iter().any(|target| target.contains(member.span))
                    {
                        return None;
                    }
                    if member_static_key(file, member)? != field {
                        // Reading another property runs nothing of the caller's
                        // literal. Calling it, tagging a template with it or
                        // reaching through it (`p.g.call(p)`) hands the object
                        // over as `this`, and that function -- an inherited one,
                        // for an absent property -- could run this property out
                        // of the callee's sight.
                        let next = file
                            .source
                            .get(member.span.end as usize..)
                            .map(str::trim_start)
                            .unwrap_or_default();
                        let hands_receiver = next.starts_with(['(', '`', '.', '['])
                            || next.starts_with("?.")
                            || file.ast.calls.iter().any(|call| {
                                file.ast.peel_ts_sugar_span(call.callee) == member.span
                            })
                            || file.ast.members.iter().any(|outer| {
                                file.ast.peel_ts_sugar_span(outer.object) == member.span
                            });
                        if hands_receiver && !receiver_safe(&member_static_key(file, member)?) {
                            return None;
                        }
                        continue;
                    }
                    member.span
                }
            };
        let peeled = |span: Span| file.ast.peel_ts_sugar_span(span);
        // Testing the property runs nothing (`props.f ? … : …`,
        // `props.f && …`): a falsy value is not the literal, and a truthy one
        // is not the `&&`'s value.
        if field.is_some()
            && (file
                .ast
                .conditional_expressions
                .iter()
                .any(|conditional| peeled(conditional.test) == invoked)
                || file.ast.logical_expressions.iter().any(|logical| {
                    logical.operator == solid_facts::ast::LogicalOperatorKind::And
                        && peeled(logical.left) == invoked
                }))
        {
            continue;
        }
        // Forwarded as exactly another project component's prop, which this
        // proof places in turn.
        if field.is_some()
            && let Some((element, prop)) = forwarded_component_prop(file, invoked)
        {
            if !element.spreads.is_empty()
                || element.attributes.iter().any(|attribute| {
                    attribute
                        .expression
                        .is_some_and(|value| receiver_function_at(file, value))
                })
            {
                return None;
            }
            let (next_file, next) =
                lookup.function_called_at(file.path.as_str(), element.name.span)?;
            let key = (file.path.to_string(), element.span);
            if !classifying.insert(key.clone()) {
                return None;
            }
            let element_span = element.span;
            let next_safe = |name: &str| {
                sibling_ignores_receiver(file, Siblings::Element(element_span), name, lookup)
            };
            let forwarded = callee_parameter_use_role(
                next_file,
                next,
                0,
                Some(prop),
                &next_safe,
                lookup,
                classifying,
            );
            classifying.remove(&key);
            let forwarded = forwarded?;
            if role.is_some_and(|role| role != forwarded) {
                return None;
            }
            role = Some(forwarded);
            calls += 1;
            continue;
        }
        let call =
            file.ast.calls.iter().find(|call| {
                !call.construct && file.ast.peel_ts_sugar_span(call.callee) == invoked
            })?;
        // Every function between the callee and the call is handed, as itself,
        // to a call or a JSX attribute: no helper, returned or stored closure
        // runs it, and no async function runs it after an `await`.
        for nested in file.ast.functions.iter().filter(|nested| {
            nested.span != function.span
                && function.span.contains(nested.span)
                && nested.span.contains(call.span)
        }) {
            // Handed to a primitive, a standard-library call or an intrinsic
            // element's attribute, whose timing the role below reads; and the
            // call is in its body, not a default initializer that runs only
            // when an argument is left out.
            let handed = file
                .ast
                .arguments_containing(nested.span)
                .any(|(outer, slot)| {
                    !outer.arguments[slot].spread
                        && file.ast.peel_ts_sugar_span(outer.arguments[slot].span) == nested.span
                        && (lookup.primitive_at_call(file, outer.span).is_some()
                            || lookup
                                .resolved_callee_call(file, outer.callee)
                                .and_then(|resolved| resolved.declaration.as_ref())
                                .is_some_and(|declaration| declaration.standard_library))
                })
                || file.ast.jsx_elements.iter().any(|element| {
                    intrinsic_element(file, element)
                        && element.attributes.iter().any(|attribute| {
                            attribute.expression.is_some_and(|value| {
                                file.ast.peel_ts_sugar_span(value) == nested.span
                            })
                        })
                });
            if nested.r#async || nested.generator || !handed || !nested.body.contains(call.span) {
                return None;
            }
        }
        let here = semantic_execution_role_within(
            file,
            call.callee,
            &[],
            lookup.entities(),
            lookup.symbol_names(),
            lookup,
            classifying,
        );
        if !matches!(
            here,
            ExecutionRole::EventCallback
                | ExecutionRole::DeferredCallback
                | ExecutionRole::TrackedJsx
        ) || role.is_some_and(|role| role != here)
        {
            return None;
        }
        role = Some(here);
        calls += 1;
    }
    (calls > 0).then_some(role).flatten()
}

/// The project component element and prop name `value` is exactly the
/// value of, when it is one ([`component_prop_literal`] for any value).
pub(super) fn forwarded_component_prop(
    file: &solid_facts::FileFacts,
    value: Span,
) -> Option<(&solid_facts::ast::JsxElementFact, &str)> {
    let (element, attribute) = file.ast.jsx_containing(value).find_map(|element| {
        element
            .attributes
            .iter()
            .find(|attribute| {
                attribute.namespace.is_none()
                    && attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                    && attribute
                        .expression
                        .is_some_and(|expression| file.ast.peel_ts_sugar_span(expression) == value)
            })
            .map(|attribute| (element, attribute))
    })?;
    if intrinsic_element(file, element) {
        return None;
    }
    Some((element, file.source_text(attribute.name)?))
}

/// ADR 0200: code directly in a function literal handed, as the whole
/// argument, to a project function that only ever invokes that parameter as
/// the tracked compute of a computation, takes the tracked role.
///
/// The project function `wrapper` must be synchronous, its parameter at the
/// slot one identifier, and every reference to that parameter a direct call
/// `p()` written directly in one function literal `compute`. `compute` must be
/// written directly in `wrapper`'s body, as the whole argument at a slot that
/// tracks the reads of the callback it runs: a callee summary's
/// guaranteed tracked-compute slot (ADR 0183) or a primitive's tracked
/// callback (`createMemo`'s compute). Then the literal runs only inside that
/// tracked compute, as it would written there itself. A parameter referenced
/// any other way can run the literal somewhere else as well, so it proves
/// nothing.
pub(super) fn forwarded_tracked_compute_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let literal = containing_ast_function(&file.ast, span)?;
    let (call, index) = file.ast.arguments_containing(span).find(|(call, index)| {
        let argument = &call.arguments[*index];
        !argument.spread
            && file.ast.peel_ts_sugar_span(argument.span) == literal.span
            && lookup.primitive_at_call(file, call.span).is_none()
    })?;
    let (wrapper_file, wrapper) = lookup
        .callee_symbol(file, call.callee)
        .and_then(|symbol| lookup.function_for_symbol(symbol))
        .or_else(|| lookup.function_called_at(file.path.as_str(), call.callee))?;
    if wrapper.r#async || wrapper.generator {
        return None;
    }
    let parameter = wrapper.parameters.get(index)?;
    if parameter.shape != solid_facts::ast::BindingShape::Identifier
        || parameter.initializer.is_some()
    {
        return None;
    }
    let name = parameter.names.first()?;
    let symbol = lookup
        .entities()
        .at(wrapper_file.path.as_str(), name.span)?;
    let mut compute: Option<Span> = None;
    for reference in lookup.symbol_references(symbol.as_str()) {
        if reference.path.as_ref() != wrapper_file.path.as_str() {
            return None;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            return None;
        };
        let reference = Span::new(start, end);
        if reference == name.span {
            continue;
        }
        // A direct call `p()`, written directly in one function literal.
        let invoked = wrapper_file
            .ast
            .calls
            .iter()
            .any(|inner| wrapper_file.ast.peel_ts_sugar_span(inner.callee) == reference);
        let owner = containing_ast_function(&wrapper_file.ast, reference)?;
        if !invoked || owner.span == wrapper.span || compute.is_some_and(|seen| seen != owner.span)
        {
            return None;
        }
        compute = Some(owner.span);
    }
    let compute = compute?;
    // `compute` is the whole argument of a call written directly in the
    // wrapper's body, at a slot that tracks its callback's reads.
    let (outer, slot) = wrapper_file
        .ast
        .arguments_containing(compute)
        .find(|(outer, slot)| {
            let argument = &outer.arguments[*slot];
            !argument.spread && wrapper_file.ast.peel_ts_sugar_span(argument.span) == compute
        })?;
    if containing_ast_function(&wrapper_file.ast, outer.span)
        .is_none_or(|function| function.span != wrapper.span)
    {
        return None;
    }
    let tracked = match lookup.primitive_at_call(wrapper_file, outer.span) {
        Some(primitive) => {
            callback_execution_at_call(wrapper_file, outer, primitive, slot, lookup).is_some()
                && lookup
                    .dialect
                    .callback_semantics_at(primitive, slot, outer.arguments.len())
                    .tracks_reads
        }
        None => false,
    };
    if !tracked {
        return None;
    }
    Some(
        if attribute_function_within(file, call.arguments[index].span, span, lookup) {
            ExecutionRole::Unknown
        } else {
            ExecutionRole::TrackedJsx
        },
    )
}

/// [`ExecutionRole::DeferredCallback`] for code in a callback that a reviewed
/// fresh-stack host scheduler runs (`runtime_semantics::FRESH_STACK_SCHEDULERS`:
/// `setTimeout`, `queueMicrotask`, `Promise.then`, `requestAnimationFrame`, the
/// observer constructors, ...), wherever the scheduling call is written.
///
/// The scheduler invokes the callback from a task or microtask queue on an
/// otherwise empty execution-context stack, so none of the scheduling code's
/// dynamic state is current there: no listener (the read subscribes to nothing
/// and nothing re-runs the callback), no owner, and no strict-read label.
/// Probed on 2.0.0-rc.3 and rc.9, dev and prod, from a component body: a read
/// in the body raises `STRICT_READ_UNTRACKED` and the same read in each such
/// callback does not; `getObserver()` and `getOwner()` are `null` there; a
/// write or an action there does not raise `REACTIVE_WRITE_IN_OWNED_SCOPE` or
/// `ACTION_CALLED_IN_OWNED_SCOPE`; a pending async read there does not raise
/// `PENDING_ASYNC_UNTRACKED_READ` -- a re-ask serves the settled value, and a
/// source that never settled throws a plain `NotReadyError`, exactly as it does
/// in a listener dispatched after mount. That is the role's meaning -- after
/// the scheduling call returns, outside its tracking pass -- the same one a
/// deferred primitive position gets.
///
/// The host fact comes from the compiler-selected standard-library declaration
/// through [`crate::runtime_semantics::argument_behavior`], never from
/// spelling, so a local `setTimeout` keeps its lexical role. Only a function
/// literal handed to the scheduler is its callback; `setTimeout(wrap(fn))`
/// hands over whatever `wrap` returns. Code in a callback that a
/// standard-library call runs inline (`list.forEach(fn)`) executes wherever
/// that call does, so the walk continues outward from the call. Any other
/// enclosing call ends the walk with no answer, leaving the arms after this
/// one to decide.
///
/// The other deferring host callbacks (`addEventListener`, `bind`'s bound
/// arguments, `PromiseLike.then`, Geolocation) are deliberately not answered:
/// each can run on its invoker's stack -- probed, a listener dispatched or a
/// bound function called in the component body, or a synchronous thenable,
/// runs inside the strict-read window and raises `STRICT_READ_UNTRACKED`. They
/// keep the lexical role, and [`host_callback_timing`] makes a read there
/// uncertifiable rather than a proven violation.
pub(super) fn fresh_stack_callback_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    fresh_stack_callback_call(file, span, lookup).map(|_| ExecutionRole::DeferredCallback)
}

/// The selected scheduler declaration enclosing a literal callback, following
/// only audited inline callback carriers. Consumers that rely on receiver
/// independence must additionally refuse `catch`/`finally`'s receiver `then`.
pub(crate) fn fresh_stack_callback_call<'file, 'lookup>(
    file: &'file solid_facts::FileFacts,
    span: Span,
    lookup: &'lookup SemanticLookup<'_>,
) -> Option<(
    &'file solid_facts::ast::CallFact,
    &'lookup typefacts::ResolvedCall,
)> {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    let mut span = span;
    loop {
        let (call, index) = file.ast.arguments_containing(span).find(|(call, index)| {
            matches!(
                call.arguments[*index].value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
            ) && direct_callback_contains(file, call.arguments[*index].span, span)
        })?;
        let resolved = lookup.resolved_callee_call(file, call.callee)?;
        let callability = lookup
            .entity_at(file.path.as_str(), call.arguments[index].span)
            .and_then(|entity| entity.callability);
        let Some(behavior) =
            crate::runtime_semantics::argument_behavior(resolved, callability, index)
        else {
            // ADR 0396: a callback handed directly to a reviewed test runner's
            // registration runs from the runner's own scheduler, except a suite
            // body its runner runs inside the registration call (a synchronous
            // function: an async body's resumptions run elsewhere), which
            // takes the role of the call's position.
            match lookup.test_runner_callback(file, call)? {
                crate::indexes::TestRunnerCallback::Scheduled => return Some((call, resolved)),
                crate::indexes::TestRunnerCallback::InlineSuiteBody
                    if matches!(
                        call.arguments[index].value,
                        solid_facts::ast::ArgumentValueKind::Function
                    ) =>
                {
                    span = call.span;
                    continue;
                }
                crate::indexes::TestRunnerCallback::InlineSuiteBody => return None,
            }
        };
        match behavior {
            RuntimeArgumentBehavior::FreshStackCallback => {
                return Some((call, resolved));
            }
            // The call contains `span` strictly, so the walk terminates.
            RuntimeArgumentBehavior::InlineCallback => span = call.span,
            RuntimeArgumentBehavior::DeferredCallback
            | RuntimeArgumentBehavior::RetainedValue
            | RuntimeArgumentBehavior::ValueOnly => return None,
        }
    }
}

/// Whether a read classified in `execution` is **uncertifiable** because it sits
/// in a callback a host API retains and may invoke on its invoker's stack
/// (`runtime_semantics::runs_on_invoker_stack`: an `addEventListener`
/// listener, a `bind` bound argument, a `PromiseLike.then` callback, a
/// Geolocation callback).
///
/// Such a callback is written in the component body but runs wherever the host
/// invokes it: inside the body's strict-read window when the body itself hands
/// control back synchronously, after it otherwise. Probed on 2.0.0-rc.3 and
/// rc.9, dev and prod, from a component body: a listener dispatched with
/// `dispatchEvent` or clicked with `el.click()` in the body, a bound function
/// called in the body, and a synchronous thenable all raise
/// `STRICT_READ_UNTRACKED` for a signal read and `PENDING_ASYNC_UNTRACKED_READ`
/// for a pending async read; the same callbacks dispatched after mount, or run
/// by a real promise, raise neither (a never-settled source throws a plain
/// `NotReadyError`). Chrome ran neither Geolocation callback in the body, from
/// an active or a detached document, but the specification's synchronous
/// "call back with error" path exists, so it is treated the same way.
///
/// Registration alone does not decide which case holds, and proving a
/// synchronous invocation in the window needs the dispatch to follow the
/// registration on every path, on the same target, with a matching event type
/// and no removal in between -- facts the syntax facts here do not carry. So
/// the answer is uncertifiable, never a proven violation, for every such read
/// the lexical fallback places in a role that reports untracked reads. A role
/// that reports nothing is left alone: this withholds a claim, it never makes
/// one.
///
/// The walk is [`fresh_stack_callback_role`]'s: the innermost function
/// literal handed directly to a call, continuing outward through
/// standard-library inline callbacks (`el.addEventListener("x", () =>
/// list.forEach(() => read()))`), and ending with no answer at any other call.
pub(crate) fn host_callback_timing(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    if !execution.reports_untracked_read() {
        return false;
    }
    let mut span = span;
    loop {
        let Some((call, index)) = file.ast.arguments_containing(span).find(|(call, index)| {
            matches!(
                call.arguments[*index].value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
            ) && direct_callback_contains(file, call.arguments[*index].span, span)
        }) else {
            return false;
        };
        let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
            return false;
        };
        let callability = lookup
            .entity_at(file.path.as_str(), call.arguments[index].span)
            .and_then(|entity| entity.callability);
        if crate::runtime_semantics::runs_on_invoker_stack(resolved, callability, index) {
            return true;
        }
        match crate::runtime_semantics::argument_behavior(resolved, callability, index) {
            // The call contains `span` strictly, so the walk terminates.
            Some(RuntimeArgumentBehavior::InlineCallback) => span = call.span,
            _ => return false,
        }
    }
}

/// Whether a read classified in `execution` is **uncertifiable** because it
/// sits in a function literal handed to a *project* function whose body is not
/// proven to invoke it during the call.
///
/// The lexical fallback of [`semantic_execution_role_within`] places code by
/// where it is written, so a literal written in a component body takes the
/// body's role. That is a proof only when the literal runs during the body.
/// Handed to a project function, it runs wherever that function runs it: in
/// its own body during the call, from a closure it returns or keeps, or never.
/// Probed on the audited 2.0.0-rc.9, dev and prod: `localChain([() => n()])`
/// whose returned invoker runs only from a click handler raises no
/// `STRICT_READ_UNTRACKED`, while the same chain invoked in the body, and a
/// helper that invokes its array's elements during the call, both warn. The
/// syntax does not tell these apart, so the read is a proof obligation, not a
/// proven untracked read.
///
/// The one invocation this proves is the direct one: the literal *is* the
/// argument at parameter `index`, the callee is a synchronous function whose
/// own body (not a nested closure) calls that parameter by symbol -- the named
/// callback case the interprocedural summary already covers. Then the literal
/// runs during the call, and the walk continues from the call, exactly as it
/// does through a standard-library inline callback (`items.forEach`). An
/// element of an array or object literal argument proves nothing: whether the
/// callee iterates it, or invokes it later, has no fact here.
///
/// A callee with no body in the project -- a package export seen through its
/// declarations, or a callee nothing resolves -- proves the invocation only
/// through a callee summary's `inline` row for that parameter; without one
/// the read is a proof obligation too. Primitive and standard-library calls
/// are other arms' business (their dialect and runtime models). This withholds
/// a claim; it never makes one.
pub(crate) fn callee_callback_timing(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> bool {
    callee_callback_timing_within(file, span, execution, lookup, &mut HashSet::default())
}

/// [`callee_callback_timing`], carrying the invocation sites ADR 0152's arm
/// has already followed: a returned value invoked from inside the very
/// callback it runs has no timing of its own to contribute, and is read as
/// unproven rather than followed forever.
pub(super) fn callee_callback_timing_within(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
    following: &mut HashSet<(String, Span)>,
) -> bool {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    if !execution.reports_untracked_read() {
        return false;
    }
    let mut span = span;
    loop {
        // A default-parameter initializer is written in the enclosing body but
        // runs when its function is *called* without that argument, which no
        // lexical position proves happens during that body.
        if let Some(function) = parameter_default_owner(file, span)
            && lookup.function_component_status(file, function) == ComponentStatus::No
        {
            let Some(index) = function.parameters.iter().position(|parameter| {
                parameter
                    .initializer
                    .is_some_and(|value| value.contains(span))
            }) else {
                // A default inside a destructuring pattern has no argument slot
                // of its own.
                return true;
            };
            // Proven only for a named helper every call of which omits that
            // argument and runs during the body.
            return !(function.method_name.is_none()
                && named_helper_runs_during_body(
                    file,
                    function,
                    Some(index),
                    execution,
                    lookup,
                    following,
                ));
        }
        let Some(literal) = containing_ast_function(&file.ast, span) else {
            return false;
        };
        // ADR 0334/0335: an audited router renders a function written as its
        // component value with `createComponent`; it is no callback of the
        // call it is written in, and its body is a component body.
        if literal.name.is_none() && lookup.route_component_value(file.path.as_str(), literal.span)
        {
            return false;
        }
        // Calling a generator function runs none of its body: it runs on
        // the returned iterator's `next()`, wherever that is.
        if literal.generator {
            return true;
        }
        // The call the literal is written in: an argument in which the literal
        // is the outermost function (directly, or inside an array or object
        // literal), the innermost such call.
        let Some((call, index)) = file
            .ast
            .arguments_containing(literal.span)
            .filter(|(call, index)| {
                file.ast
                    .functions_within(call.arguments[*index].span)
                    .filter(|function| function.span.contains(literal.span))
                    .max_by_key(|function| function.span.end - function.span.start)
                    .is_some_and(|outer| outer.span == literal.span)
            })
            .min_by_key(|(call, _)| call.span.end - call.span.start)
        else {
            // Constructing a callback-valued JSX prop does not invoke it.
            // Its consumer may forward it to a DOM event, retain it, invoke
            // it during rendering, or never call it. The lexical component
            // context proves none of those invocation times. Require the
            // literal itself as the prop value: an IIFE inside that value
            // still executes while the value is evaluated.
            if file.ast.jsx_elements.iter().any(|element| {
                element.attributes.iter().any(|attribute| {
                    attribute
                        .expression
                        .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == literal.span)
                })
            }) {
                // ADR 0204: unless the consumer is proven to call it while it
                // renders.
                return !prop_literal_invoked_during_render(file, literal.span, lookup);
            }
            // An IIFE runs where it is written, so its timing is its
            // surroundings'.
            if let Some(call) = file
                .ast
                .calls
                .iter()
                .filter(|call| file.ast.peel_ts_sugar_span(call.callee) == literal.span)
                .min_by_key(|call| call.span.end - call.span.start)
            {
                span = call.span;
                continue;
            }
            // Only the lexical rendering role places code by where it is
            // written; a role a dialect arm proved for this position (an
            // effect's apply callback, an untrack callback) is not.
            return execution == ExecutionRole::UntrackedRendering
                && literal_invocation_unproven(file, literal, execution, lookup, following);
        };
        if lookup.primitive_at_call(file, call.span).is_some() {
            // A primitive's own callback argument is classified by the
            // dialect arms. A literal nested in a container argument
            // (`merge(props, { get class() { return cls(); } })`) is a value
            // the primitive stores, not a callback it invokes.
            return !(!call.arguments[index].spread
                && file.ast.peel_ts_sugar_span(call.arguments[index].span) == literal.span)
                && stored_literal_invocation_unproven(file, literal, lookup);
        }
        let argument = &call.arguments[index];
        // Resolved by symbol, members included (`bus.addEventListener` of a
        // project object resolves to its method's declaration).
        if let Some((callee_file, callee, exact)) =
            project_callee_at_parameter(file, call, index, lookup)
        {
            let direct =
                !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == literal.span;
            if !(direct
                && exact
                && invokes_parameter_during_call(callee_file, callee, index, lookup))
            {
                return true;
            }
            // The call contains the literal strictly, so the walk terminates.
            span = call.span;
            continue;
        }
        let resolved = lookup.resolved_callee_call(file, call.callee);
        if let Some(resolved) = resolved.filter(|resolved| {
            resolved
                .declaration
                .as_ref()
                .is_some_and(|declaration| declaration.standard_library)
        }) {
            let callability = lookup
                .entity_at(file.path.as_str(), argument.span)
                .and_then(|entity| entity.callability);
            match crate::runtime_semantics::argument_behavior(resolved, callability, index) {
                Some(RuntimeArgumentBehavior::InlineCallback)
                    if direct_callback_contains(file, argument.span, span) =>
                {
                    span = call.span;
                    continue;
                }
                // A standard-library call that is not modelled as running this
                // argument inline (it may keep it, schedule it, or hand it to
                // something that does) does not prove the callback runs during
                // the call.
                _ => return true,
            }
        }
        return true;
    }
}
