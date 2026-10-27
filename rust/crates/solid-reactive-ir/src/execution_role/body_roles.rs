//! Roles within a component body: awaits, named callback census, and literals that run during the body.

use super::*;

/// Whether `literal` is the argument of a call that is not a primitive and is
/// not proven to invoke that argument during the call: an unresolved callee, a
/// project function that keeps or forwards it, a standard-library call not
/// modelled as running it inline, a callee with no accepted `inline` row. A
/// primitive's callback is classified by the dialect arms, so it is not asked
/// here; a literal that is no call's argument is not either.
pub(super) fn call_argument_invocation_unproven(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
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
        return false;
    };
    if lookup.primitive_at_call(file, call.span).is_some() {
        return false;
    }
    let argument = &call.arguments[index];
    let direct = !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == literal.span;
    // The compiler's resolved signature names the called declaration exactly;
    // a project function reached only through the receiver's root (`list` in
    // `list().filter(…)`, whose binding is a `createMemo` call) is not it.
    let standard = lookup
        .resolved_callee_call(file, call.callee)
        .and_then(|resolved| resolved.declaration.as_ref())
        .is_some_and(|declaration| declaration.standard_library);
    if !standard
        && let Some((callee_file, callee, exact)) =
            project_callee_at_parameter(file, call, index, lookup)
    {
        return !(direct
            && exact
            && invokes_parameter_during_call(callee_file, callee, index, lookup));
    }
    if let Some(resolved) = lookup
        .resolved_callee_call(file, call.callee)
        .filter(|resolved| {
            resolved
                .declaration
                .as_ref()
                .is_some_and(|declaration| declaration.standard_library)
        })
    {
        let callability = lookup
            .entity_at(file.path.as_str(), argument.span)
            .and_then(|entity| entity.callability);
        return !(direct
            && matches!(
                crate::runtime_semantics::argument_behavior(resolved, callability, index),
                Some(RuntimeArgumentBehavior::InlineCallback)
            ));
    }
    true
}

/// Whether `span` sits in a function literal that is merely *stored* where it
/// is written -- an object property, a getter, an array element, a returned
/// closure -- or in a function nested inside one. Constructing such a literal
/// runs nothing, so no scope around the construction runs what is inside.
///
/// A literal handed to a call, invoked in place, or written as a control-flow
/// component's render callback is not stored: those have their own proofs.
pub(super) fn enclosed_by_stored_function(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast
        .functions_body_containing(span)
        .any(|literal| stored_function(file, literal, entities, symbol_names, lookup))
}

/// Whether `literal` is a function stored where it is written: not handed to a
/// call as an argument, not invoked in place, not a control-flow render
/// callback ([`enclosed_by_stored_function`]).
fn stored_function(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    stored_literal_invocation_unproven(file, literal, lookup)
        && !file
            .ast
            .arguments_containing(literal.span)
            .any(|(call, index)| {
                let argument = &call.arguments[index];
                !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == literal.span
            })
        && !file
            .ast
            .calls
            .iter()
            .any(|call| file.ast.peel_ts_sugar_span(call.callee) == literal.span)
        && control_flow_execution_role(file, literal.body, entities, symbol_names, lookup.dialect)
            .is_none()
}

/// ADR 0450, reads: whether `span` sits in a function written strictly inside
/// the tracked `region` that the region's tracking pass does not run: a
/// function stored where it is written (a returned closure, an object member,
/// [`stored_function`]), or a closure bound to a name that is not entered only
/// by calls in tracked positions (a name that is returned, stored or called
/// elsewhere). Building such a function runs nothing, so the region's role is
/// no proof for what is read in it; it takes none, and the read is
/// uncertifiable rather than certified tracked.
pub(super) fn unrun_closure_within(
    file: &solid_facts::FileFacts,
    region: Span,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    thread_local! {
        /// The named closures whose calls are being placed on this thread: a
        /// call written in the closure itself (recursion) or in another being
        /// placed asks again, and a closure already in progress is unproven.
        static IN_PROGRESS: std::cell::RefCell<HashSet<(String, Span)>> = std::cell::RefCell::default();
    }
    file.ast.functions_body_containing(span).any(|function| {
        region.contains(function.span)
            && region != function.span
            && if function.method_name.is_none() && function_binding_name(file, function).is_some()
            {
                let key = (file.path.to_string(), function.span);
                lookup.function_component_status(file, function) == ComponentStatus::No
                    && (!IN_PROGRESS.with(|seen| seen.borrow_mut().insert(key.clone())) || {
                        let proven = named_helper_runs_during_body(
                            file,
                            function,
                            None,
                            ExecutionRole::TrackedJsx,
                            ExecutionRole::TrackedJsx,
                            lookup,
                            &mut HashSet::default(),
                        );
                        IN_PROGRESS.with(|seen| seen.borrow_mut().remove(&key));
                        !proven
                    })
            } else {
                stored_function(file, function, entities, symbol_names, lookup)
            }
    })
}

/// ADR 0450: the innermost function around `span` that is bound to a name
/// (`const openFn = () => …`, `function openFn() {…}`) and constructed inside the
/// tracked scope whose role the lexical position gave `span`. A tracked scope
/// runs what is written in it, but constructing a named closure runs nothing:
/// the closure runs where it is called, so the scope's role is no proof for
/// its body. A stored literal has the same standing
/// ([`enclosed_by_stored_function`]) but is anonymous; a named one is entered
/// through its references, which the call sites of the write walk place (a call
/// in the scope's own pass is a tracked call and proves the write; a closure
/// only returned or stored has no call and stays unplaced). The caller walks
/// the returned closure's call sites, once the layers between it and the write
/// are proven to run in place.
pub(super) fn named_closure_in_tracked_scope<'a>(
    file: &'a solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<&'a solid_facts::ast::FunctionFact> {
    file.ast
        .functions_body_containing(span)
        .filter(|function| {
            function.method_name.is_none()
                && function_binding_name(file, function).is_some()
                && lookup.function_component_status(file, function) == ComponentStatus::No
                && semantic_execution_role_within(
                    file,
                    function.span,
                    allowed,
                    entities,
                    symbol_names,
                    lookup,
                    &mut HashSet::from_iter([write_classification_marker()]),
                ) == ExecutionRole::TrackedJsx
        })
        .min_by_key(|function| function.body.end - function.body.start)
}

/// Whether `span` is written in an async function after an `await` of that
/// same function, by position. Only the function's own awaits count: one in a
/// nested closure suspends that closure.
/// ADR 0320: whether the code at `span` runs after an await its async
/// function reaches on every path (an unconditional await before it in the
/// same function): an ownerless continuation on a later task, never in a
/// render window or a tracking scope.
pub(crate) fn follows_unconditional_await(file: &solid_facts::FileFacts, span: Span) -> bool {
    containing_ast_function(&file.ast, span).is_some_and(|function| {
        // An async generator is resumed by its caller after each `yield`,
        // synchronously, in whatever window or owner that caller has.
        function.r#async
            && !function.generator
            && file.ast.unconditional_awaits.iter().any(|awaited| {
                awaited.end <= span.start
                    && function.body.contains(*awaited)
                    && containing_ast_function(&file.ast, *awaited)
                        .is_some_and(|owner| owner.span == function.span)
            })
    })
}

/// Whether `span` runs only after an await of its own function suspended: it
/// lies in the rest of the block an await dominates (`dominating_awaits`),
/// in the same function as that await. A closure written there, an instance
/// field initializer and a JSX expression run whenever they are called,
/// constructed or rendered, which may be under an owner again, so they are
/// not included.
pub(crate) fn follows_dominating_await(file: &solid_facts::FileFacts, span: Span) -> bool {
    let Some(function) = continuation_function(file, span) else {
        return false;
    };
    file.ast.dominating_awaits.iter().any(|(awaited, region)| {
        region.contains(span)
            && containing_ast_function(&file.ast, *awaited)
                .is_some_and(|owner| owner.span == function.span)
    })
}

/// The async, non-generator function whose continuation `span` would run in,
/// when nothing at `span` runs later under an owner instead: an instance
/// field initializer (it runs at construction), a JSX expression or a
/// compiler tracked region (a render effect the compiler creates runs it,
/// review round 2). An async generator suspends again at each `yield`, and
/// its caller may resume it under an owner (review round 3).
fn continuation_function(
    file: &solid_facts::FileFacts,
    span: Span,
) -> Option<&solid_facts::ast::FunctionFact> {
    if inside_instance_field(file, span)
        || file.ast.jsx_containing(span).next().is_some()
        || file
            .compiler
            .tracked_regions
            .iter()
            .any(|region| region.span.contains(span))
    {
        return None;
    }
    containing_ast_function(&file.ast, span)
        .filter(|function| function.r#async && !function.generator)
}

/// Whether `span` sits in the `catch` or `finally` of a `try` whose block
/// starts with an await of an operand that cannot throw synchronously: then
/// nothing in the block runs before the await suspends, so those blocks run
/// only after it did, on a later task with no owner.
///
/// The operand must be a call of a plain identifier naming an async,
/// non-generator function *declaration* (hoisted, so initialized before any
/// code of its scope runs, even across an import cycle), with only literals
/// and the parameters of an enclosing function as arguments (initialized when
/// that function's body starts). Calling an async function never throws, even
/// from its parameter defaults, and it returns a native promise, which the
/// await adopts without running project code (review round 1: a `let`/`const`
/// argument may still be in its temporal dead zone, and awaiting an arbitrary
/// value reads its `constructor`, which a getter can make throw).
pub(crate) fn follows_try_head_await(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(function) = continuation_function(file, span) else {
        return false;
    };
    file.ast.try_head_awaits.iter().any(|head| {
        head.regions.iter().any(|region| region.contains(span))
            && containing_ast_function(&file.ast, head.awaited)
                .is_some_and(|owner| owner.span == function.span)
            && operand_cannot_throw(file, head, lookup)
    })
}

fn operand_cannot_throw(
    file: &solid_facts::FileFacts,
    head: &solid_facts::ast::TryHeadAwaitFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let awaited = head.awaited;
    // Literal nodes by their syntax (review rounds 2-3: source text matched
    // `NaN`, an identifier, and `1+box.value`, which may throw).
    let literal_arguments = &head.literal_arguments;
    let operand = ast.peel_ts_sugar_span(head.operand);
    let Some(call) = ast.calls.iter().find(|call| call.span == operand) else {
        return false;
    };
    let reference = |span: Span| {
        ast.identifiers.iter().any(|identifier| {
            identifier.span == span
                && identifier.role == solid_facts::ast::IdentifierRole::Reference
        })
    };
    let parameter = |span: Span| {
        reference(span)
            && ast.reference_declaration(span).is_some_and(|declaration| {
                ast.functions.iter().any(|enclosing| {
                    enclosing.body.contains(awaited)
                        && enclosing
                            .parameters
                            .iter()
                            .flat_map(|parameter| &parameter.names)
                            .chain(&enclosing.rest_parameter_names)
                            .any(|name| name.span == declaration)
                })
            })
    };
    let callee = ast.peel_ts_sugar_span(call.callee);
    if !reference(callee)
        || !call.arguments.iter().all(|argument| {
            !argument.spread
                && (parameter(argument.span) || literal_arguments.contains(&argument.span))
        })
    {
        return false;
    }
    let Some((declared_file, declared)) = lookup.function_called_at(file.path.as_str(), callee)
    else {
        return false;
    };
    let Some(called) = lookup.entities().at(file.path.as_str(), callee) else {
        return false;
    };
    declared.kind == solid_facts::ast::FunctionKind::Declaration
        && declared.r#async
        && !declared.generator
        && declared.name.as_ref().is_some_and(|name| {
            lookup.entities().at(declared_file.path.as_str(), name.span) == Some(called)
        })
}

/// The functions whose body holds `span`, innermost first and short of the
/// first component, that take their role from a named callback position
/// (`onClick={press}`, `untrack(read)`): the role [`named_callback_roles`]
/// admits for them describes that position, not a direct call of the same
/// function, so it holds for `span` only when every invocation does.
pub(super) fn named_callbacks_within_component<'f>(
    file: &'f solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Vec<&'f solid_facts::ast::FunctionFact> {
    let roles = lookup.named_callback_roles(file);
    let mut enclosing = file.ast.functions_body_containing(span).collect::<Vec<_>>();
    // Innermost first: a function nested in another starts later.
    enclosing.sort_by_key(|function| std::cmp::Reverse(function.span.start));
    enclosing
        .into_iter()
        .take_while(|function| {
            lookup.function_component_status(file, function) == ComponentStatus::No
        })
        .filter(|function| roles.get(function.span).admitted)
        .collect()
}

/// ADR 0320: whether every reference to the named function `function` is a
/// position that admits it as a named callback -- an event-handler
/// attribute value, or an argument of a known primitive call -- so the role
/// those positions give it is the role of every invocation. A direct call,
/// any other use, an export, a reference in another file or a direct `eval`
/// in the file refuses. Answers for open programs too, where ADR 0319 asks
/// nothing: the census is of one file-local binding.
pub(super) fn named_callback_census_holds(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_facts::ast::IdentifierRole;
    let Some(binding) = crate::owners::function_binding_name(file, function) else {
        return false;
    };
    if crate::owners::own_method_name(file, function).is_some()
        || flow_binding_exported(file, binding.span)
        || file.ast.identifiers.iter().any(|id| {
            id.role == IdentifierRole::Reference && file.source_text(id.span) == Some("eval")
        })
    {
        return false;
    }
    let Some(symbol) = lookup.entities().at(file.path.as_str(), binding.span) else {
        return false;
    };
    let mut references = file
        .ast
        .identifiers
        .iter()
        .filter(|id| {
            id.role == IdentifierRole::Reference
                && file.ast.reference_declaration(id.span) == Some(binding.span)
        })
        .map(|id| id.span)
        .collect::<HashSet<_>>();
    for reference in lookup.symbol_references(symbol.as_str()) {
        if reference.path.as_ref() != file.path.as_str() {
            return false;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            return false;
        };
        let reference = Span::new(start, end);
        if reference != binding.span {
            if file.ast.reference_declaration(reference) != Some(binding.span) {
                return false;
            }
            references.insert(reference);
        }
    }
    if references.is_empty() {
        return false;
    }
    let primitives = lookup.primitives(file);
    references.into_iter().all(|reference| {
        file.compiler.callback_roles.iter().any(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && callback.span == reference
        }) || file.ast.calls.iter().enumerate().any(|(index, call)| {
            known_primitive(&primitives.calls[index]).is_some_and(|primitive| {
                call.arguments
                    .iter()
                    .enumerate()
                    .any(|(argument_index, argument)| {
                        !argument.spread
                            && file.ast.peel_ts_sugar_span(argument.span) == reference
                            && crate::owners::callback_execution_at_call(
                                file,
                                call,
                                primitive,
                                argument_index,
                                lookup,
                            )
                            .is_some()
                    })
            })
        })
    })
}

/// ADR 0320: whether a function a named callback position admits is invoked
/// only in the role it is admitted with: its ADR 0319 invocation-flow answer
/// is an excluded role, or its file-local reference census holds.
pub(super) fn named_callback_invocations_hold(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    flow_answers: bool,
) -> bool {
    (flow_answers
        && lookup.program_closed
        && lookup
            .invocation_flow_answers()
            .get(&(file.path.to_string(), function.span))
            .is_some_and(|answer| flow_safe_role(*answer)))
        || named_callback_census_holds(file, function, lookup)
}

/// ADR 0320: whether an excluded execution role at `span` (an event, a
/// deferred callback, a tracked position, deleted code) also holds for every
/// invocation of the functions around it. A role read off a named
/// function's callback references says nothing about a direct call of the
/// same function -- `press()` in a component body beside `onClick={press}`
/// -- and a dispatch obligation, unlike a read, is not re-raised at that
/// call. So each function between `span` and its component that a named
/// callback position admits must have an ADR 0319 invocation-flow answer
/// that is itself an excluded role; any other role is read off the position
/// the code is written in, which every invocation shares.
pub(crate) fn excluded_role_holds(
    file: &solid_facts::FileFacts,
    span: Span,
    role: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> bool {
    match role {
        ExecutionRole::DiscardedRendering => true,
        ExecutionRole::TrackedJsx
        | ExecutionRole::EventCallback
        | ExecutionRole::DeferredCallback => named_callbacks_within_component(file, span, lookup)
            .into_iter()
            .all(|function| named_callback_invocations_hold(file, function, lookup, true)),
        _ => false,
    }
}

/// ADR 0320: whether the code at `span` cannot run in a render window or a
/// tracking scope because of an await: it follows an await its async function
/// reaches on every path, or it may follow one and every invocation of that
/// function runs as an event, a deferred callback or a tracked computation
/// (ADR 0319) -- so it runs either in that role, before the await, or on a
/// later task after it.
pub(crate) fn await_continuation_decides_nothing(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if follows_unconditional_await(file, span) {
        return true;
    }
    let Some(function) = containing_ast_function(&file.ast, span) else {
        return false;
    };
    function.r#async
        && follows_await_in_async_function(file, span)
        && lookup.program_closed
        && lookup
            .invocation_flow_answers()
            .get(&(file.path.to_string(), function.span))
            .is_some_and(|role| flow_safe_role(*role))
}

/// The argument list of the call written at `span` (as its callee, or as the
/// whole call). Its arguments are evaluated before the call runs.
fn written_call_arguments(file: &solid_facts::FileFacts, span: Span) -> Option<Span> {
    file.ast
        .calls
        .iter()
        .filter(|call| call.callee == span || call.span == span)
        .map(|call| Span::new(call.callee.end, call.span.end))
        .min_by_key(|arguments| arguments.end - arguments.start)
}

pub(super) fn follows_await_in_async_function(file: &solid_facts::FileFacts, span: Span) -> bool {
    // A call's arguments are evaluated before the call runs, so an await in
    // the written call's own argument list (`setX(await load())`) also
    // precedes it.
    let arguments = written_call_arguments(file, span);
    containing_ast_function(&file.ast, span).is_some_and(|function| {
        function.r#async
            && file.ast.awaits.iter().any(|awaited| {
                (awaited.end <= span.start
                    || arguments.is_some_and(|arguments| arguments.contains(*awaited)))
                    && function.body.contains(*awaited)
                    && containing_ast_function(&file.ast, *awaited)
                        .is_some_and(|owner| owner.span == function.span)
            })
    })
}

/// ADR 0354: whether code at `span` may run after a suspension of its own
/// async function. Some await or implicit suspension (`for await`, `yield`)
/// of that function may execute before it in the same activation: one written
/// earlier (or in the written call's own argument list), unless the two are
/// the arms of one `if` or `?:`, of which only one runs; one in a loop whose
/// repeated part holds `span`, which runs again after it on a later iteration;
/// or one in the value a destructuring pattern holds `span` in, since the
/// pattern's defaults run after that value.
pub(crate) fn may_follow_own_await(file: &solid_facts::FileFacts, span: Span) -> bool {
    let Some(function) = containing_ast_function(&file.ast, span) else {
        return false;
    };
    if !function.r#async {
        return false;
    }
    let arguments = written_call_arguments(file, span);
    file.ast
        .awaits
        .iter()
        .chain(&file.ast.implicit_suspensions)
        .any(|&awaited| {
            function.body.contains(awaited)
                && containing_ast_function(&file.ast, awaited)
                    .is_some_and(|owner| owner.span == function.span)
                && (file.ast.loop_repeats.iter().any(|&(looped, repeated)| {
                    repeated.contains(span)
                        && looped.contains(awaited)
                        && function.body.contains(looped)
                }) || ((awaited.end <= span.start
                    || arguments.is_some_and(|arguments| arguments.contains(awaited)))
                    && !in_exclusive_arms(file, awaited, span))
                    || file
                        .ast
                        .pattern_values
                        .iter()
                        .any(|&(pattern, value)| pattern.contains(span) && value.contains(awaited)))
        })
}

/// Whether code at `span` may run after a suspension of its own async
/// function by any of the tests that place code by position: the textual
/// await order, an implicit suspension before it, or [`may_follow_own_await`].
/// A call site or a write that may follow one has no proven placement.
pub(super) fn may_follow_suspension(file: &solid_facts::FileFacts, span: Span) -> bool {
    follows_await_in_async_function(file, span)
        || follows_any_suspension(file, span)
        || may_follow_own_await(file, span)
}

/// Whether `awaited` and `span` lie in the two different arms of one `if` or
/// one `?:`: only one arm runs, so the await cannot come before `span`.
fn in_exclusive_arms(file: &solid_facts::FileFacts, awaited: Span, span: Span) -> bool {
    let split = |first: Span, second: Span| first.contains(awaited) && second.contains(span);
    file.ast.if_regions.iter().any(|region| {
        region.alternate.is_some_and(|alternate| {
            split(region.consequent, alternate) || split(alternate, region.consequent)
        })
    }) || file.ast.conditional_expressions.iter().any(|conditional| {
        split(conditional.consequent, conditional.alternate)
            || split(conditional.alternate, conditional.consequent)
    })
}

/// Whether code at `span`, classified [`ExecutionRole::UntrackedRendering`],
/// provably runs during the body it is written in.
///
/// Walks outward from the innermost function containing `span`. A
/// (possible) component is the body itself: the rendering role is its own.
/// Any other function literal runs during the body only as an argument whose
/// invocation during the call is established:
///
/// - a primitive's callback, which the dialect arms of
///   [`semantic_execution_role_within`] already classified;
/// - a literal handed to a project function that invokes that parameter
///   during the call, a standard-library inline callback, or a package
///   callback a callee summary states inline -- exactly the invocations
///   [`callee_callback_timing`] proves for a read.
///
/// An IIFE runs where it is written, and a control-flow component's render
/// callback runs while the component's children render, so both continue the
/// walk or answer `true`. Any other literal that is no call's argument --
/// stored in a binding, returned from another callback
/// (`keep(() => () => setCount(1))`), a JSX attribute -- and one handed to a
/// function not proven to invoke it during the
/// call (`later(() => setCount(1))`, which keeps it for a timer) prove
/// nothing: `false`. Then the write takes the role its invocation sites
/// prove, and with none it is unclassified, which reports nothing
/// (`docs/rules/reactive-write-in-owned-scope.md`: an unproven write position
/// is never a violation).
pub(crate) fn nested_literal_runs_during_body(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    // A default-parameter initializer is outside its function's body but
    // runs only when that function is called, never where it is declared.
    if file.ast.functions.iter().any(|function| {
        function.span.contains(span)
            && !function.body.contains(span)
            && span.end <= function.body.start
            && lookup.function_component_status(file, function) == ComponentStatus::No
    }) {
        return false;
    }
    let mut span = span;
    loop {
        let Some(literal) = containing_ast_function(&file.ast, span) else {
            return true;
        };
        if lookup.function_component_status(file, literal) != ComponentStatus::No {
            return true;
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
            // Two literals that are no argument still run in place: an IIFE,
            // invoked where it is written, and a control-flow component's
            // render callback, which the component runs while its children
            // render (`control_flow_execution_role` proved that role).
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
            return control_flow_execution_role(file, span, entities, symbol_names, lookup.dialect)
                .is_some_and(ExecutionRole::reports_disallowed_write);
        };
        match lookup.primitive_at_call(file, call.span) {
            // A primitive whose dialect models no callback at this position
            // (a cleanup registration, say) does not say it runs the
            // function during the call: it keeps it for later, or never.
            Some(primitive)
                if callback_execution_at_call(file, call, primitive, index, lookup).is_none() =>
            {
                return false;
            }
            Some(_) => {}
            None => {
                if callee_callback_timing(file, span, ExecutionRole::UntrackedRendering, lookup) {
                    return false;
                }
            }
        }
        // The call contains the literal strictly, so the walk terminates.
        span = call.span;
    }
}

/// Whether `span` sits directly in the body of a function that is, or may be,
/// a component, on a dialect whose component body runs under a root
/// ([`Dialect::component_body_runs_under_root`]).
///
/// A possible component is included: the only premise that made a write there
/// a violation is that it runs as a component body, and under this guard a
/// component body is legal.
pub(super) fn runs_directly_in_component_root(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    lookup.dialect.component_body_runs_under_root()
        && containing_ast_function(&file.ast, span).is_some_and(|function| {
            lookup.function_component_status(file, function) != ComponentStatus::No
        })
}

/// The write-legality answer a same-file function passed **by name** takes
/// from the calls that pass it, where an inline callback in the same position
/// would take a [`WriteRegionAdjustment`]: `createRoot(init)` runs `init` as
/// the root body, and `untrack(write)` or `flush(write)` runs `write` exactly
/// as legally as at the call.
///
/// The function is resolved by symbol: the argument must be a bare identifier
/// whose symbol is the function's own binding (`entities.at` on a call span
/// would answer with the callee instead). Only a rejecting site answers,
/// because the function may also run from positions this does not look at --
/// a direct call, another callback slot -- which keep their own
/// classification; one invocation that throws is a violation, while one that
/// does not proves nothing about the others.
#[allow(clippy::too_many_arguments)]
pub(super) fn named_callback_write_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    visiting: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    let function = containing_ast_function(&file.ast, span)?;
    let symbol = function_symbol(file, function, entities)?;
    let key = (file.path.to_string(), function.span);
    if !visiting.insert(key.clone()) {
        return None;
    }
    let mut rejected = None;
    'calls: for call in &file.ast.calls {
        for (index, argument) in call.arguments.iter().enumerate() {
            if argument.value != solid_facts::ast::ArgumentValueKind::Identifier
                || entities.at(file.path.as_str(), argument.span) != Some(symbol)
            {
                continue;
            }
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let execution = callback_execution_at_call(file, call, primitive, index, lookup);
            // A root body is judged by its owner whenever it runs, so the
            // root claim needs no stated timing (rc.17 `hydrate`).
            let site = if lookup
                .dialect
                .callback_runs_in_created_root(primitive, index)
            {
                (root_body == RootBodyGuard::Rejects).then_some(ExecutionRole::UntrackedRendering)
            } else if lookup
                .dialect
                .callback_preserves_owner_write_context(primitive)
                && execution.is_some()
            {
                let at_call = semantic_write_execution_role_within(
                    file,
                    call.span,
                    allowed,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    visiting,
                );
                at_call.reports_disallowed_write().then_some(at_call)
            } else {
                None
            };
            if site.is_some() {
                rejected = site;
                break 'calls;
            }
        }
    }
    visiting.remove(&key);
    rejected
}

/// `classifying` is the stack of spans whose role is currently being derived
/// from their own invocation sites. A returned adapter invoked inside its own
/// factory callback — `const a = on(() => a(), fn)`, or two adapters invoked
/// in each other's callbacks — makes that derivation cyclic; a site already on
/// the stack supplies no independent execution context and is skipped instead
/// of re-entered.
pub(super) fn semantic_execution_role_within(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> ExecutionRole {
    // ADR 0360: no site of a file the compiler refused is placed. Its syntax
    // is real, but which of its code runs tracked, deferred, on an event or
    // during rendering is exactly what the missing compiler facts decide.
    if file.compiler.withheld() {
        return ExecutionRole::Unknown;
    }
    let role = semantic_role_before_awaits(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        classifying,
    );
    if role == ExecutionRole::EventCallback && lookup.synchronous_event_read_dispatch_present() {
        return ExecutionRole::Unknown;
    }
    // ADR 0323: code after an await its async function reaches on every path
    // runs from a promise continuation, on a later task. The strict-read
    // window is the synchronous extent of `untrack(fn, label)` (`strictRead`
    // is restored in its `finally`, `@solidjs/signals@2.0.0-rc.13`
    // `dist/dev-shared.js:5801-5885`), so a read there has no window and no
    // tracking scope: none of the reporting roles describes it. Only
    // statement-level awaits prove the window closed (`unconditional_awaits`).
    // Code that only may follow an await, past a skippable one (`x ??= await p`,
    // an optional-call argument, a labelled break, an `if` arm) or in a loop
    // that awaits, is unknown rather than placed in the window (ADR 0354).
    // Past an unconditional await the window is proven closed: no breach,
    // only no single role. An implicit suspension (`for await`, `await using`)
    // is placed by position alone, which does not prove it runs (a loop under
    // a branch), so there the read is unplaced, not proven silent.
    if role.reports_untracked_read()
        && (follows_unconditional_await(file, span)
            || follows_dominating_await(file, span)
            || follows_try_head_await(file, span, lookup))
    {
        return ExecutionRole::Unreported;
    }
    if role.reports_untracked_read()
        && (follows_any_suspension(file, span) || may_follow_own_await(file, span))
    {
        return ExecutionRole::Unknown;
    }
    // Review round 4: code written in a component element's children (an
    // event-handler factory on a `<button>` inside `<Show>`, `<Loading>`, a
    // portal or a wrapper rendering `props.children`) runs when the receiving
    // component evaluates its children -- inside Show's memo, an insert, a
    // later render -- not in this body's strict-read window, which the
    // lexical rendering role assumes. No proven untracked read.
    if role == ExecutionRole::UntrackedRendering && inside_component_element_children(file, span) {
        return ExecutionRole::Unknown;
    }
    role
}

/// Whether `span` is written in the children of a component (non-intrinsic)
/// element, in the same function as that element, with no function between:
/// code the receiving component runs when it evaluates `children`.
pub(crate) fn inside_component_element_children(file: &solid_facts::FileFacts, span: Span) -> bool {
    let function = containing_ast_function(&file.ast, span);
    file.ast.jsx_containing(span).any(|element| {
        !intrinsic_element(file, element)
            && function.is_none_or(|function| function.body.contains(element.span))
            && element.children.iter().any(|child| child.contains(span))
    })
}

/// Whether code at `span` is written, in its async function's own body,
/// inside or after a `for await` loop: the loop awaits its first `next()`
/// before the body runs, so code there has left the component body's
/// strict-read window (ADR 0323) on every path that reaches it. A skippable
/// `await` (`if (c) await p`, `x ??= await p`) is not an implicit suspension;
/// [`may_follow_own_await`] covers it (ADR 0354). The loop's iterated operand
/// is evaluated before the first suspension.
pub(super) fn follows_any_suspension(file: &solid_facts::FileFacts, span: Span) -> bool {
    let Some(function) = containing_ast_function(&file.ast, span) else {
        return false;
    };
    if !function.r#async {
        return false;
    }
    let own = |point: Span| {
        function.body.contains(point)
            && containing_ast_function(&file.ast, point)
                .is_some_and(|owner| owner.span == function.span)
    };
    file.ast.implicit_suspensions.iter().any(|suspension| {
        own(*suspension)
            && suspension.start < span.start
            && !file
                .ast
                .iterated_operands
                .iter()
                .any(|operand| suspension.contains(*operand) && operand.contains(span))
    })
}
