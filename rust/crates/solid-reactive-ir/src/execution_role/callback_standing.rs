//! Callback standing: whether an unclassified callback reaches a modelled consumer, and parameter call standing.

use super::*;

/// ADR 0323: whether an anonymous callback around `span`, which no compiler
/// role places, reaches only consumers whose invocation question the read
/// rules already answer with [`callee_callback_timing`]: every function
/// layer between `span` and its component is
///
/// - an IIFE (the literal is the callee of a call), or
/// - an argument (directly, or inside an array or object literal argument) of
///   a call that a project function resolves for, or of a standard-library
///   call the audited rows model as running that argument inline.
///
/// Anything else answers `false`: a component attribute, an unknown or
/// package callee, a primitive, a standard-library call that may keep or
/// schedule the function, and a literal that is merely stored. Those
/// callbacks keep the silence a props or store read had before this ADR.
///
/// The walk mirrors [`callee_callback_timing_within`] on purpose (the same
/// outermost-literal and innermost-call choice), so a read admitted here is
/// the read that function then proves or withholds; it never makes a claim.
pub(crate) fn unclassified_callback_reaches_modelled_consumer(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    let mut span = span;
    let mut innermost = true;
    loop {
        // A default-parameter initializer runs when its function is called.
        if parameter_default_owner(file, span).is_some() {
            return false;
        }
        let Some(literal) = containing_ast_function(&file.ast, span) else {
            return true;
        };
        // The component the callbacks are written in. An anonymous function
        // that is itself the innermost layer and may be a component is a
        // renderer (`render: (row) => <tr/>`), whose parameter is not a props
        // object: it keeps the silence it had. A layer that is only an
        // uncertain component proves nothing about where its body runs.
        match lookup.function_component_status(file, literal) {
            ComponentStatus::Proven if !innermost => return true,
            ComponentStatus::No => {}
            ComponentStatus::Proven | ComponentStatus::Uncertain => return false,
        }
        innermost = false;
        // A function bound to a name is the interprocedural summary's
        // business, not an anonymous callback.
        if literal.method_name.is_some() || function_binding_name(file, literal).is_some() {
            return false;
        }
        // Calling a generator function runs none of its body: it runs on
        // the returned iterator's `next()`, wherever that is.
        if literal.generator {
            return false;
        }
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
            // An IIFE runs where it is written.
            let Some(call) = file
                .ast
                .calls
                .iter()
                .filter(|call| file.ast.peel_ts_sugar_span(call.callee) == literal.span)
                .min_by_key(|call| call.span.end - call.span.start)
            else {
                return false;
            };
            span = call.span;
            continue;
        };
        if let Some(primitive) = lookup.primitive_at_call(file, call.span) {
            // A callback that runs synchronously in its call's own window
            // (`createRoot`, `runWithOwner`) is a layer like an IIFE: it runs
            // where the call does.
            if lookup
                .dialect
                .callback_inherits_calling_window(primitive, index)
                && callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                && direct_callback_contains(file, call.arguments[index].span, span)
            {
                span = call.span;
                continue;
            }
            return false;
        }
        if project_callee_at_parameter(file, call, index, lookup).is_some() {
            span = call.span;
            continue;
        }
        let argument = &call.arguments[index];
        let Some(resolved) = lookup
            .resolved_callee_call(file, call.callee)
            .filter(|resolved| {
                resolved
                    .declaration
                    .as_ref()
                    .is_some_and(|declaration| declaration.standard_library)
            })
        else {
            return false;
        };
        let callability = lookup
            .entity_at(file.path.as_str(), argument.span)
            .and_then(|entity| entity.callability);
        if matches!(
            crate::runtime_semantics::argument_behavior(resolved, callability, index),
            Some(RuntimeArgumentBehavior::InlineCallback)
        ) && direct_callback_contains(file, argument.span, span)
        {
            span = call.span;
            continue;
        }
        return false;
    }
}

/// Whether a function literal that is no call's argument is *not proven* to run
/// where it is written -- the complement of every position that does:
///
/// - the component body itself, the rendering role's own proof;
/// - a control-flow component's children callback
///   ([`control_flow_execution_role`] proves it);
/// - a function bound to a name whose every reference is a call and whose every
///   call runs during the body ([`named_helper_runs_during_body`]).
///
/// Every other literal is *stored* (an object property, method or getter, an
/// array element, a returned closure, an anonymous value) or handed to a JSX
/// element that is not a rendering control-flow component. Constructing such a
/// literal runs nothing, and wherever it is later called -- from a handler,
/// from the object's reader, from a hook's returned closure, or never -- is not
/// something its lexical position proves.
pub(super) fn literal_invocation_unproven(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
    following: &mut HashSet<(String, Span)>,
) -> bool {
    // An anonymous component (`export default function () {}`) is the body
    // itself, which is the rendering role's own proof.
    if lookup.function_component_status(file, literal) != ComponentStatus::No {
        return false;
    }
    if control_flow_execution_role(
        file,
        literal.body,
        lookup.entities(),
        lookup.symbol_names(),
        lookup.dialect,
    )
    .is_some()
    {
        return false;
    }
    // A function passed by name as a control-flow component's children callback
    // (`<For each={xs}>{renderItem}</For>`) is that callback.
    if named_callback_execution_role(file, literal.body, lookup)
        == Some(ExecutionRole::UntrackedRendering)
    {
        return false;
    }
    if literal.method_name.is_none() && function_binding_name(file, literal).is_some() {
        return !named_helper_runs_during_body(file, literal, None, execution, lookup, following);
    }
    true
}

/// Whether `literal` is a function that is *stored* where it is written: an
/// anonymous function expression or an object-literal method or getter, outside
/// every position that runs it ([`literal_invocation_unproven`]). A function
/// bound to a name is the interprocedural summary's business through its call
/// sites, and is left alone here.
pub(super) fn stored_literal_invocation_unproven(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    (literal.method_name.is_some() || function_binding_name(file, literal).is_none())
        && literal_invocation_unproven(
            file,
            literal,
            ExecutionRole::UntrackedRendering,
            lookup,
            &mut HashSet::new(),
        )
}

/// Whether the function bound to a name runs only during the body it is written
/// in: it has at least one call, every reference to its binding is one of those
/// calls (no alias, no handler, no argument), and every call is itself proven
/// to run during the body.
///
/// A call written in a handler, a timer, a stored closure or a callback prop is
/// unproven by the same walk, so the helper is too; a call cycle is unproven.
///
/// With `default_parameter` the question is about that parameter's default
/// initializer, which runs only when a call leaves the argument out: every call
/// must then pass fewer arguments than the parameter's position, spread-free.
pub(super) fn named_helper_runs_during_body(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    default_parameter: Option<usize>,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
    following: &mut HashSet<(String, Span)>,
) -> bool {
    let Some(binding) = function_binding_name(file, literal) else {
        return false;
    };
    let path = file.path.as_str();
    let Some(symbol) = lookup.entities().at(path, binding.span) else {
        return false;
    };
    let sites = lookup.function_call_sites(path, literal.span);
    if sites.is_empty() {
        return false;
    }
    let accounted: HashSet<(String, u32, u32)> = lookup
        .function_call_site_references(path, literal.span)
        .into_iter()
        .map(|(caller, callee)| (caller.path.to_string(), callee.start, callee.end))
        .collect();
    let escapes = lookup
        .symbol_references(symbol.as_str())
        .iter()
        .any(|reference| {
            let start = u32::try_from(reference.start_byte).unwrap_or(u32::MAX);
            let end = u32::try_from(reference.end_byte).unwrap_or(u32::MAX);
            let declaration = reference.path.as_ref() == path
                && start == binding.span.start
                && end == binding.span.end;
            !declaration && !accounted.contains(&(reference.path.to_string(), start, end))
        });
    if escapes {
        return false;
    }
    let key = (path.to_string(), literal.span);
    if !following.insert(key.clone()) {
        return false;
    }
    let proven = sites.iter().all(|(caller, callee)| {
        let omits_argument = default_parameter.is_none_or(|index| {
            caller
                .ast
                .calls
                .iter()
                .find(|call| caller.ast.peel_ts_sugar_span(call.callee) == *callee)
                .is_some_and(|call| {
                    call.arguments.len() <= index && !call.arguments.iter().any(|a| a.spread)
                })
        });
        // The call must itself be written where the rendering role holds: a
        // call in a tracked JSX attribute or child, a memo or a deferred
        // callback runs in that scope, not in the body's strict-read window.
        omits_argument
            && semantic_execution_role(
                caller,
                *callee,
                &allowed_callback_spans(caller, lookup),
                lookup.entities(),
                lookup.symbol_names(),
                lookup,
            ) == ExecutionRole::UntrackedRendering
            && !callee_callback_timing_within(caller, *callee, execution, lookup, following)
    });
    following.remove(&key);
    proven
}

/// The innermost function whose default-parameter initializer contains `span`.
///
/// A parameter initializer lies outside its function's *body*, so the
/// body-based containment queries place a read in it in the surrounding
/// function. Only functions nested in the span's own enclosing body (or any
/// function, at module level) can own it, which bounds the scan.
pub(super) fn parameter_default_owner(
    file: &solid_facts::FileFacts,
    span: Span,
) -> Option<&solid_facts::ast::FunctionFact> {
    let region = containing_ast_function(&file.ast, span)
        .map_or_else(|| Span::new(0, u32::MAX), |function| function.body);
    file.ast
        .functions_within(region)
        .filter(|function| {
            function.span.contains(span)
                && !function.body.contains(span)
                && function.parameters.iter().any(|parameter| {
                    parameter
                        .initializer
                        .is_some_and(|value| value.contains(span))
                })
        })
        .min_by_key(|function| function.span.end - function.span.start)
}

/// ADR 0278: the project function `call` runs, resolved as the call sites
/// here always have, and whether argument `index` provably lands in that
/// function's parameter `index`. `None` when no project function resolves.
/// `Some((_, false))` when one does but the mapping is not exact:
/// - the callee is a member whose own declaration is not that function
///   (`f.call(thisArg, …)` shifts every argument);
/// - its binding may have been replaced since (ADR 0221);
/// - a spread argument before `index` may fill that parameter.
pub(crate) fn project_callee_at_parameter<'a>(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    index: usize,
    lookup: &SemanticLookup<'a>,
) -> Option<(
    &'a solid_facts::FileFacts,
    &'a solid_facts::ast::FunctionFact,
    bool,
)> {
    // A local method declaration cannot bypass ADR 0299's receiver escape
    // and initialization census when deciding callback timing either.
    if crate::object_members::call_withheld(file, call.callee, lookup) {
        return None;
    }
    if let Some((file, function)) = lookup.object_member_function(file, call.callee) {
        return Some((
            file,
            function,
            !call
                .arguments
                .iter()
                .take(index.saturating_add(1))
                .any(|argument| argument.spread),
        ));
    }
    let declared = lookup
        .callee_symbol(file, call.callee)
        .and_then(|symbol| lookup.function_for_symbol(symbol));
    let (callee_file, callee) =
        declared.or_else(|| lookup.function_called_at(file.path.as_str(), call.callee))?;
    // A member callee resolves to its own declaration above. Reaching a
    // function only through the entity at the whole member span names the
    // receiver's root instead: `f` in `f.call(thisArg, …)`.
    let callee_span = file.ast.peel_ts_sugar_span(call.callee);
    let member = file
        .ast
        .members
        .iter()
        .any(|member| member.span == callee_span)
        || file
            .ast
            .computed_members
            .binary_search(&callee_span)
            .is_ok();
    let exact = !(member && declared.is_none())
        && (!member || member_runs_declared_method(file, callee_span, callee_file, callee, lookup))
        && lookup.function_value_is_current(callee_file, callee)
        && !call
            .arguments
            .iter()
            .take(index.saturating_add(1))
            .any(|argument| argument.spread);
    Some((callee_file, callee, exact))
}

/// ADR 0279: whether a member call `receiver.name(…)` runs the declared
/// method `method`, rather than an override or a replacement. A module
/// namespace member qualifies. Otherwise no assignment anywhere may write a
/// member of that name, and the receiver is a `const` bound to the object
/// literal that holds the method, or to `new C()` of the exact class that
/// declares it.
pub(super) fn member_runs_declared_method(
    file: &solid_facts::FileFacts,
    callee: Span,
    method_file: &solid_facts::FileFacts,
    method: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    // A module namespace member: the namespace object is frozen, and the
    // export binding's own currency is checked by the caller.
    if lookup
        .namespace_member_function(file, callee)
        .is_some_and(|(namespace_file, function)| {
            namespace_file.path == method_file.path && function.span == method.span
        })
    {
        return true;
    }
    let Some(member) = file.ast.members.iter().find(|member| member.span == callee) else {
        return false;
    };
    let Some(name) = file.source_text(member.property) else {
        return false;
    };
    // ADR 0314: a fresh object literal's chain holds only `Object.prototype`.
    // A class instance's is checked with its class below.
    if lookup.member_may_be_reassigned_on_builtin(name) {
        return false;
    }
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    let Some((binding_file, binding, _)) =
        lookup.binding_at_reference(file.path.as_str(), receiver)
    else {
        return false;
    };
    if !binding.immutable || binding.shape != solid_facts::ast::BindingShape::Identifier {
        return false;
    }
    let Some(initializer) = binding.initializer else {
        return false;
    };
    // The receiver must not escape: `Object.assign(bus, …)` or
    // `Object.defineProperty(bus, "run", …)` replaces the method without an
    // assignment (review round 4). Every reference reads a member of it, and
    // no module export exposes it to an importer.
    let Some(declaration) = binding.names.first().map(|name| name.span) else {
        return false;
    };
    let only_member_reads = binding_file
        .ast
        .reference_declarations
        .iter()
        .filter(|(_, bound)| *bound == declaration)
        .all(|(reference, _)| {
            binding_file
                .ast
                .members
                .iter()
                .any(|member| binding_file.ast.peel_ts_sugar_span(member.object) == *reference)
        });
    // Only a module-level binding can be exported: an exported function's
    // span contains its whole body (as in `property_gets::receiver`).
    let module_level = containing_ast_function(&binding_file.ast, declaration).is_none();
    let exported = binding_file.ast.exports.iter().any(|export| {
        (module_level && export.span.contains(declaration))
            || export.specifiers.iter().any(|specifier| {
                binding_file.ast.reference_declaration(specifier.local.span) == Some(declaration)
            })
    });
    if !only_member_reads || exported {
        return false;
    }
    let initializer = binding_file.ast.peel_ts_sugar_span(initializer);
    if binding.initializer_value_kind == solid_facts::ast::RuntimeValueKind::Object {
        return binding_file.path == method_file.path && initializer.contains(method.span);
    }
    let Some(construction) = binding_file
        .ast
        .call_at(initializer)
        .filter(|construction| construction.construct)
    else {
        return false;
    };
    let Some((class_file, class)) = lookup
        .callee_symbol(binding_file, construction.callee)
        .and_then(|symbol| lookup.class_for_symbol(symbol))
    else {
        return false;
    };
    // Nor may the class's prototype be reachable for patching.
    let class_exposed = class.name.as_ref().is_some_and(|class_name| {
        class_file
            .ast
            .reference_declarations
            .iter()
            .filter(|(_, bound)| *bound == class_name.span)
            .any(|(reference, _)| {
                class_file.ast.members.iter().any(|member| {
                    class_file.ast.peel_ts_sugar_span(member.object) == *reference
                        && class_file.source_text(member.property) == Some("prototype")
                }) || class_file.ast.calls.iter().any(|call| {
                    call.arguments.iter().any(|argument| {
                        class_file.ast.peel_ts_sugar_span(argument.span) == *reference
                    })
                })
            })
    });
    class_file.path == method_file.path
        && class.span.contains(method.span)
        && !class_exposed
        && lookup.class_instance_is_exact(class_file, class, name, 4)
        && !lookup.member_may_be_reassigned_on_class(name, class_file, class)
}

/// ADR 0280: how the calls of `function`'s parameter `index` stand on
/// `call`, given what its arguments prove. `Guaranteed` when one of them
/// always runs, `Unreachable` when none can, `None` when the arguments prove
/// nothing or the parameter is written.
pub(crate) fn parameter_call_standing(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    function_file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    index: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_facts::ast::SiteStanding> {
    use solid_facts::ast::SiteStanding;
    let parameter = function.parameters.get(index)?;
    if parameter.shape != solid_facts::ast::BindingShape::Identifier {
        return None;
    }
    let name = parameter.names.first()?;
    if crate::indexes::binding_written(function_file, name.span) {
        return None;
    }
    let symbol = lookup
        .entities()
        .at(function_file.path.as_str(), name.span)?;
    let sites = function_file
        .ast
        .calls
        .iter()
        .filter(|inner| {
            crate::owners::written_directly_in(&function_file.ast, function, inner.span)
                && lookup.callee_symbol(function_file, inner.callee) == Some(symbol.as_str())
        })
        .map(|inner| inner.span)
        .collect::<Vec<_>>();
    if sites.is_empty() {
        return None;
    }
    let values =
        crate::argument_values::parameter_values(file, call, function_file, function, lookup)
            .filter(|values| !values.is_empty())?;
    let standings = solid_facts::ast::argument_site_standings(
        std::path::Path::new(function_file.path.as_str()),
        &function_file.source,
        function.span,
        &values,
        &lookup.never_returning_calls_within(function_file, function),
        &sites,
    )?;
    // Every other reference -- an alias, an argument (`list.forEach(cb)`),
    // a member -- may run the callback too, so ruling out the direct calls
    // proves no absence (review round 4).
    let only_direct_calls = function_file
        .ast
        .reference_declarations
        .iter()
        .filter(|(_, declaration)| *declaration == name.span)
        .all(|(reference, _)| {
            function_file.ast.calls.iter().any(|inner| {
                sites.contains(&inner.span)
                    && function_file.ast.peel_ts_sugar_span(inner.callee) == *reference
            })
        });
    if standings.contains(&SiteStanding::Guaranteed) {
        Some(SiteStanding::Guaranteed)
    } else if only_direct_calls
        && standings
            .iter()
            .all(|standing| *standing == SiteStanding::Unreachable)
    {
        Some(SiteStanding::Unreachable)
    } else {
        Some(SiteStanding::Possible)
    }
}

/// Whether the synchronous project function `function` calls its parameter
/// `index` anywhere written directly in its own body, by the parameter's
/// symbol: the call may run during a call of `function`, on some paths
/// (ADR 0277). [`invokes_parameter_during_call`] is the every-completion
/// subset.
pub(crate) fn calls_parameter_in_body(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    index: usize,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if function.r#async || function.generator {
        return false;
    }
    let Some(parameter) = function.parameters.get(index) else {
        return false;
    };
    if parameter.shape != solid_facts::ast::BindingShape::Identifier {
        return false;
    }
    let Some(symbol) = parameter
        .names
        .first()
        .and_then(|name| lookup.entities().at(file.path.as_str(), name.span))
    else {
        return false;
    };
    file.ast.calls.iter().any(|call| {
        containing_ast_function(&file.ast, call.span)
            .is_some_and(|owner| owner.span == function.span)
            && lookup.callee_symbol(file, call.callee) == Some(symbol.as_str())
    })
}

/// Whether the synchronous project function `function` calls its parameter
/// `index` on every completion of its own body -- not in a closure it
/// creates, not behind a branch, loop or early exit -- by the parameter's
/// symbol. Destructured, rest and missing parameters prove
/// nothing, and neither does an async function or a generator, whose body may
/// run after the call returns.
pub(crate) fn invokes_parameter_during_call(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    index: usize,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if function.r#async || function.generator {
        return false;
    }
    let Some(parameter) = function.parameters.get(index) else {
        return false;
    };
    if parameter.shape != solid_facts::ast::BindingShape::Identifier {
        return false;
    }
    let Some(name) = parameter.names.first() else {
        return false;
    };
    if lookup
        .entities()
        .at(file.path.as_str(), name.span)
        .is_none()
    {
        return false;
    }
    // ADR 0277: a call of the parameter on the function's every-completion
    // path, not merely one written in its body. `if (flag) cb()` or a loop
    // body may never call it. The table is bound into the cross-file proof
    // digest, so moving that call invalidates retained caller results.
    lookup.parameter_called_on_every_completion(file.path.as_str(), function.span, index)
}

/// Whether `span` lies inside an argument the dialect says runs on reads of
/// the call's returned object ([`Dialect::callback_runs_on_result_access`]).
///
/// Containment, not direct-callback identity: every function written inside
/// such an argument runs, if at all, from inside it. An argument a project
/// wrapper forwards into such a slot
/// ([`SemanticLookup::result_access_forwarded_arguments`]) counts too.
pub(super) fn result_access_callback_contains(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if lookup
        .result_access_forwarded_arguments(file.path.as_str())
        .iter()
        .any(|forwarded| forwarded.argument.contains(span))
    {
        return true;
    }
    file.ast.arguments_containing(span).any(|(call, index)| {
        // ADR 0323 (review round 10): the slot's *callback* runs on result
        // access; an expression written at the slot (`omit(props, props.k)`)
        // is evaluated by the call, where it is written.
        file.ast
            .functions_within(call.arguments[index].span)
            .any(|function| function.span.contains(span))
            && lookup
                .primitive_at_call(file, call.span)
                .is_some_and(|primitive| {
                    lookup.dialect.callback_runs_on_result_access(
                        primitive,
                        index,
                        call.arguments.len(),
                    )
                })
    })
}

/// Inline callbacks that do not clear the reactive listener inherit the
/// caller's execution role. This covers wrappers such as `batch`,
/// `catchError`'s protected body, and `modifyMutable`; `untrack` and other
/// explicit untracked callbacks are classified by the later dialect branch.
pub(super) fn inline_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            if !direct_callback_contains(file, call.arguments[index].span, span) {
                return None;
            }
            let primitive = lookup.primitive_at_call(file, call.span)?;
            if callback_execution_at_call(file, call, primitive, index, lookup)?
                != Execution::Inline
                || callback_runs_outside_tracking(
                    lookup.dialect,
                    primitive,
                    index,
                    call.arguments.len(),
                )
            {
                return None;
            }
            let key = (file.path.to_string(), call.span);
            if !classifying.insert(key.clone()) {
                return None;
            }
            let role = semantic_execution_role_within(
                file,
                call.span,
                allowed,
                lookup.entities(),
                lookup.symbol_names(),
                lookup,
                classifying,
            );
            classifying.remove(&key);
            // The callback runs where the call runs. The lexical rendering
            // role of the call is proven only when the call itself runs during
            // the body; a call written in a nested helper that runs later
            // (`const isCurrent = id => id === latest(() => draft())`) is not.
            if role == ExecutionRole::UntrackedRendering
                && !nested_literal_runs_during_body(
                    file,
                    call.span,
                    lookup.entities(),
                    lookup.symbol_names(),
                    lookup,
                )
            {
                return Some(ExecutionRole::Unknown);
            }
            Some(role)
        })
}

/// Compose an inline callback of a higher-order factory with the proven use of
/// the returned function. For example, `on` reads its dependency inline when
/// the returned adapter runs: `createEffect(on(...))` therefore tracks that
/// read, while a direct top-level adapter call does not.
pub(super) fn returned_factory_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    // Every proof this composition needs starts from
    // `callback_requires_return_invocation`, which Solid 2.0 leaves at its
    // `false` default for every primitive.
    if !lookup.models_returned_callbacks() {
        return None;
    }
    file.ast
        .arguments_containing(span)
        .find_map(|(factory_call, index)| {
            if !direct_callback_contains(file, factory_call.arguments[index].span, span) {
                return None;
            }
            let primitive = lookup.primitive_at_call(file, factory_call.span)?;
            let semantics = lookup.dialect.callback_semantics_at(
                primitive,
                index,
                factory_call.arguments.len(),
            );
            if !semantics.requires_return_invocation
                || semantics.execution != Some(Execution::Inline)
            {
                return None;
            }

            role_at_returned_invocations(
                returned_callback_invocation_sites(file, factory_call, lookup),
                lookup,
                classifying,
            )
        })
}

/// The one execution role the proven invocations of a returned function give
/// the callbacks it runs on its invoker's stack, or `None` when there is no
/// proven invocation. Shared by the dialect's returned-callback composition and
/// ADR 0152's contract composition, so the two cannot read the same sites
/// differently.
pub(super) fn role_at_returned_invocations(
    sites: Vec<crate::owners::ReturnedCallbackInvocationSite>,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    let mut roles = Vec::new();
    for site in sites {
        let role = match site.inherited_execution {
            Some(Execution::Tracked) => Some(ExecutionRole::TrackedJsx),
            Some(Execution::Deferred) => Some(ExecutionRole::DeferredCallback),
            Some(Execution::Inline) | None => {
                let key = (site.path.clone(), site.span);
                if classifying.contains(&key) {
                    // A cyclic invocation site — the adapter calling
                    // itself through its own callback — has no context
                    // of its own to contribute.
                    None
                } else {
                    lookup
                        .files()
                        .iter()
                        .find(|candidate| candidate.path.as_str() == site.path)
                        .map(|use_file| {
                            classifying.insert(key.clone());
                            let role = semantic_execution_role_within(
                                use_file,
                                site.span,
                                &[],
                                lookup.entities(),
                                lookup.symbol_names(),
                                lookup,
                                classifying,
                            );
                            classifying.remove(&key);
                            role
                        })
                }
            }
        };
        roles.extend(role);
    }
    roles.sort_by_key(|role| *role as u8);
    roles.dedup();
    match roles.as_slice() {
        [] => None,
        [role] => Some(*role),
        // The same returned adapter is used in incompatible execution
        // contexts. A single diagnostic site cannot truthfully claim one
        // dominates, so preserve uncertainty instead of manufacturing a
        // false positive in either direction.
        _ => Some(ExecutionRole::DeferredCallback),
    }
}

pub(super) fn returned_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            if !direct_callback_contains(file, call.arguments[index].span, span) {
                return None;
            }
            let (primitive, result_slot) = returned_primitive_invocation(file, call, lookup)?;
            match lookup
                .dialect
                .returned_callback_semantics_at(primitive, result_slot, index, call.arguments.len())
                .execution?
            {
                Execution::Tracked => Some(ExecutionRole::TrackedJsx),
                Execution::Deferred => Some(ExecutionRole::DeferredCallback),
                // The returned function restores/inherits its caller's execution
                // context. Classify the proven invocation span, outside the
                // callback body itself, so a top-level transition remains
                // untracked while one started from an effect remains tracked.
                Execution::Inline => {
                    let key = (file.path.to_string(), call.span);
                    if !classifying.insert(key.clone()) {
                        return None;
                    }
                    let role = semantic_execution_role_within(
                        file,
                        call.span,
                        &[],
                        lookup.entities(),
                        lookup.symbol_names(),
                        lookup,
                        classifying,
                    );
                    classifying.remove(&key);
                    Some(role)
                }
            }
        })
}

/// Whether `span` is evaluated as the `value` getter of a resolved Solid 1.x
/// `createContext().Provider`.
///
/// Both halves are semantic proof: the JSX member object resolves to the
/// binding initialized by the exact Solid `createContext` primitive, and the
/// final member is `Provider`. An arbitrary component named `SomeProvider` or
/// object with a same-spelled property satisfies neither condition.
pub(super) fn context_provider_value_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let provider_member = lookup.dialect.context_provider_member()?;
    let element = file
        .ast
        .jsx_elements
        .iter()
        .filter(|element| {
            element.attributes.iter().any(|attribute| {
                file.source_text(attribute.local_name) == Some("value")
                    && attribute
                        .expression
                        .is_some_and(|expression| expression.contains(span))
            })
        })
        .min_by_key(|element| element.span.end - element.span.start)?;
    let (Some(object), Some(property)) = (element.member_object, element.member_property) else {
        return None;
    };
    if file.source_text(property) != Some(provider_member) {
        return None;
    }
    if !lookup.is_context_reference(file.path.as_str(), object) {
        return None;
    }
    // Solid 1.x's createProvider implementation eagerly reads props.value
    // inside untrack. A function expression is the one exception: reading
    // the getter only creates/stores the function; its body is not run.
    let stores_function = file.ast.functions_body_containing(span).any(|function| {
        element.attributes.iter().any(|attribute| {
            attribute
                .expression
                .is_some_and(|expression| expression.contains(function.span))
        })
    });
    Some(if stores_function {
        ExecutionRole::DeferredCallback
    } else {
        ExecutionRole::UntrackedRendering
    })
}
