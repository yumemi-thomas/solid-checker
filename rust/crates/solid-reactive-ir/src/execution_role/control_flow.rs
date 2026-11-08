//! Control-flow roles: Show/Match/For bodies and the members a function contains.

use super::*;

pub(crate) fn assigned_member_function_contains(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
) -> bool {
    containing_ast_function(&file.ast, span).is_some_and(|function| {
        file.ast.assignments.iter().any(|assignment| {
            if assignment.value != solid_facts::ast::AssignmentValueKind::Function
                || !assignment.value_span.contains(function.span)
            {
                return false;
            }
            let Some(member) = file.ast.member_at(assignment.target) else {
                return false;
            };
            let Some(object_symbol) = entities.at(file.path.as_str(), member.object) else {
                return false;
            };
            let Some(owner) = containing_ast_function(&file.ast, assignment.target) else {
                return false;
            };
            let caller_owned = owner.parameters.iter().any(|parameter| {
                parameter
                    .names
                    .iter()
                    .any(|name| entities.at(file.path.as_str(), name.span) == Some(object_symbol))
            });
            let returned = file.ast.returns.iter().any(|returned| {
                returned.value == solid_facts::ast::ReturnValueKind::Identifier
                    && containing_ast_function(&file.ast, returned.span)
                        .is_some_and(|candidate| candidate.span == owner.span)
                    && entities.at(file.path.as_str(), returned.span) == Some(object_symbol)
            });
            caller_owned || returned
        })
    })
}

/// The body of the control-flow children callback around `span`, chosen as
/// [`control_flow_execution_role`] chooses it: the innermost element that
/// renders its children through a callback, and the outermost function
/// written in one of its children with no other element between.
pub(crate) fn flow_callback_body(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<Span> {
    let element = file
        .ast
        .jsx_containing(span)
        .filter(|element| {
            jsx_primitive_name(file, element, entities, symbol_names, dialect)
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| dialect.renders_children_through_callback(primitive))
        })
        .min_by_key(|element| element.span.end - element.span.start)?;
    file.ast
        .functions_body_containing(span)
        .filter(|function| {
            element.span.contains(function.span)
                && element
                    .children
                    .iter()
                    .any(|child| child.contains(function.span))
                && !file
                    .ast
                    .jsx_containing(function.span)
                    .any(|nested| nested.span != element.span && element.span.contains(nested.span))
        })
        .max_by_key(|function| function.body.end - function.body.start)
        .map(|function| function.body)
}

pub(crate) fn control_flow_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let dialect = lookup.dialect;
    let element = file
        .ast
        .jsx_containing(span)
        .filter(|element| {
            jsx_primitive_name(file, element, entities, symbol_names, dialect)
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| dialect.renders_children_through_callback(primitive))
        })
        .min_by_key(|element| element.span.end - element.span.start)?;
    // The control-flow component invokes only a function written at its own
    // level: its children callback (`<For>{(item) => …}</For>`) or code run
    // inline from there. A function inside a *nested* element belongs to that
    // element -- `<Show><div onClick={() => …} /></Show>` hands the arrow to
    // the `<div>` as an event handler, which the compiler classifies, and
    // which runs when the event fires, with no owner (probed on the audited
    // 2.0.0-rc.9, dev and prod: a signal write there neither throws nor
    // warns). The named-callback index draws the same line for identifiers.
    //
    // Only a function written in one of the element's *children* is that
    // callback (or code run inline from it). A function in an attribute --
    // the predicate inside `each={items.filter((p) => p.id !== props.id)}`, a
    // `when` expression's callback -- is an ordinary expression of the
    // attribute it is written in, not what the component invokes with the
    // item.
    let callback =
        file.ast
            .functions_body_containing(span)
            .filter(|function| {
                element.span.contains(function.span)
                    && element
                        .children
                        .iter()
                        .any(|child| child.contains(function.span))
                    && !file.ast.jsx_containing(function.span).any(|nested| {
                        nested.span != element.span && element.span.contains(nested.span)
                    })
            })
            .max_by_key(|function| function.body.end - function.body.start)?;
    let owner = containing_ast_function(&file.ast, span)?;
    if owner.span != callback.span {
        // A nested handler of an event the DOM also delivers on its own
        // (`onLoad`), as `execution_role.denied-event` says in a body (ADR 0780).
        if super::in_denied_handler(file, span) {
            return Some(role_arm!(
                "control_flow.control_flow_execution_role.denied-event",
                ExecutionRole::Unknown
            ));
        }
        return Some(role_arm!(
            "control_flow.control_flow_execution_role.deferred-callback",
            ExecutionRole::DeferredCallback
        ));
    }
    // `Show`/`Match` run a function child as a callback only when its
    // `length` is above 0; a function of length 0 (no parameters, or a
    // default on the first) is inserted as an ordinary child and runs in the
    // insert's tracked effect (review round 4). Only the child's own body:
    // a function nested in it keeps its own role (review round 5).
    if super::named_callbacks::function_length(callback) == 0
        && jsx_primitive_name(file, element, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| dialect.children_callback_requires_parameters(primitive))
    {
        // Reviewer R57: it runs in the insert's tracked effect only where the
        // element's value is inserted; a value held and called (`shown()()`)
        // runs where it is called, which the position does not say.
        let inserted = jsx_value_inserted(file, element.span, lookup, 8);
        return Some(if inserted {
            role_arm!(
                "control_flow.control_flow_execution_role.tracked-jsx",
                ExecutionRole::TrackedJsx
            )
        } else {
            role_arm!(
                "control_flow.control_flow_execution_role.value-not-inserted",
                ExecutionRole::Unknown
            )
        });
    }
    // A fragment has no element fact, so its children are consulted through
    // its own span table: `<>{tone()}</>` is as tracked as `<b>{tone()}</b>`.
    if file
        .ast
        .jsx_containing(span)
        .any(|nested| callback.body.contains(nested.span))
        || file
            .ast
            .jsx_fragments
            .iter()
            .any(|fragment| fragment.contains(span) && callback.body.contains(*fragment))
    {
        Some(role_arm!(
            "control_flow.control_flow_execution_role.tracked-jsx-2",
            ExecutionRole::TrackedJsx
        ))
    } else {
        Some(role_arm!(
            "control_flow.control_flow_execution_role.untracked-rendering",
            ExecutionRole::UntrackedRendering
        ))
    }
}

/// A role for a site directly in an exact control-flow children literal.
/// Containment alone is insufficient: `{wrap(() => ... )}` passes the result
/// of `wrap`, not that literal. Unknown wrappers, competing children props and
/// spreads do not acquire a render-body proof through this admission gate.
pub(crate) fn direct_control_flow_body_role(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let dialect = lookup.dialect;
    let owner = containing_ast_function(&file.ast, span)?;
    if owner.r#async
        || owner.generator
        || !crate::owners::written_directly_in(&file.ast, owner, span)
    {
        return None;
    }
    let exact = file.ast.jsx_containing(owner.span).any(|element| {
        element.spreads.is_empty()
            && element
                .children
                .iter()
                .filter(|child| {
                    file.source_text(**child)
                        .is_none_or(|text| !text.trim().is_empty())
                })
                .count()
                == 1
            && !element
                .attributes
                .iter()
                .any(|attribute| file.source_text(attribute.name) == Some("children"))
            && element
                .children
                .iter()
                .any(|child| jsx_child_expression(file, *child) == Some(owner.span))
            && exact_jsx_primitive_name(file, element, entities, symbol_names, dialect)
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| dialect.renders_children_through_callback(primitive))
    });
    if !exact {
        return None;
    }
    control_flow_execution_role(file, span, entities, symbol_names, lookup).filter(|role| {
        matches!(
            role,
            ExecutionRole::UntrackedRendering | ExecutionRole::TrackedJsx
        )
    })
}

/// Whether the function at `function` is a callback a dialect primitive
/// places itself: written directly as a control-flow element's child
/// (`<For>{(item) => …}</For>`), or passed directly at a primitive's callback
/// slot. It runs in the context that primitive creates, however the function
/// around it is entered. Merely being written somewhere inside such an element
/// (`<Show><Table columns={[{ cell: (row) => … }]} /></Show>`) is not this.
pub(crate) fn function_placed_by_dialect(
    file: &solid_facts::FileFacts,
    function: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast.jsx_containing(function).any(|element| {
        element
            .children
            .iter()
            .any(|child| jsx_child_expression(file, *child) == Some(function))
            && crate::jsx_primitive_name_cached(
                lookup,
                file,
                element,
                lookup.entities(),
                lookup.symbol_names(),
                lookup.dialect,
            )
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| lookup.dialect.renders_children_through_callback(primitive))
    }) || file
        .ast
        .arguments_containing(function)
        .any(|(call, index)| {
            !call.arguments[index].spread
                && file.ast.peel_ts_sugar_span(call.arguments[index].span) == function
                && lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| {
                        lookup
                            .dialect
                            .callback_semantics_at(primitive, index, call.arguments.len())
                            .execution
                            .is_some()
                    })
        })
}

/// Whether the function at `function` is passed directly at a primitive's
/// inline synchronous callback slot (`untrack(() => …)`): it runs at once, in
/// the context of the function around the call.
pub(crate) fn function_runs_inline(
    file: &solid_facts::FileFacts,
    function: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast
        .arguments_containing(function)
        .any(|(call, index)| {
            !call.arguments[index].spread
                && file.ast.peel_ts_sugar_span(call.arguments[index].span) == function
                && lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| {
                        lookup.dialect.synchronous_callback_slot(
                            primitive,
                            index,
                            call.arguments.len(),
                        )
                    })
        })
}

/// ADR 0542: whether `function` is written as the callback of a primitive that
/// runs it during the call and keeps the caller's owner and read role
/// ([`solid_dialect::Dialect::callback_preserves_owner_write_context`] with an
/// inline execution: `flush(fn)`, `untrack(fn)`). Where the caller runs, it
/// runs: no primitive places it.
pub(crate) fn function_keeps_caller_context(
    file: &solid_facts::FileFacts,
    function: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast
        .arguments_containing(function)
        .any(|(call, index)| {
            !call.arguments[index].spread
                && file.ast.peel_ts_sugar_span(call.arguments[index].span) == function
                && lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| {
                        lookup
                            .dialect
                            .callback_preserves_owner_write_context(primitive)
                            && lookup
                                .dialect
                                .callback_semantics_at(primitive, index, call.arguments.len())
                                .execution
                                == Some(Execution::Inline)
                    })
        })
}

/// The expression an expression-container child (`{expression}`) holds, with
/// transparent TypeScript wrappers peeled. `None` for a text, element or
/// fragment child, which holds no expression of its own.
pub(super) fn jsx_child_expression(file: &solid_facts::FileFacts, child: Span) -> Option<Span> {
    let text = file.source_text(child)?;
    let inner = text.strip_prefix('{')?.strip_suffix('}')?;
    let leading = u32::try_from(inner.len() - inner.trim_start().len()).ok()?;
    let trailing = u32::try_from(inner.len() - inner.trim_end().len()).ok()?;
    let (start, end) = (child.start + 1 + leading, child.end - 1 - trailing);
    (start < end).then(|| file.ast.peel_ts_sugar_span(Span::new(start, end)))
}

/// Reviewer R57/R59/R62: whether the JSX value at `span` (an element or a
/// fragment) is itself inserted, so a length-0 function child of it runs in
/// the insert's tracked effect. It is when it is the inserted value itself,
/// written as a child or the value of a child expression (or a branch of a
/// conditional or logical one), of an intrinsic element (whose children
/// compile to `insert`) or of a fragment that is itself inserted; or a
/// proven component's return, which the runtime renders. A component
/// element's children are a prop it may hold and call; a part of a value
/// (`{(h = <Show/>, null)}`, `return { s: <Show/> }`), a plain function's
/// return and a held fragment prove nothing.
fn jsx_value_inserted(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> bool {
    if depth == 0 {
        return false;
    }
    let ast = &file.ast;
    let value_at = |expression: Span| {
        let expression = ast.peel_ts_sugar_span(expression);
        expression == span
            || ast.conditional_expressions.iter().any(|conditional| {
                conditional.span == expression
                    && [conditional.consequent, conditional.alternate]
                        .into_iter()
                        .any(|branch| ast.peel_ts_sugar_span(branch) == span)
            })
            || ast.logical_expressions.iter().any(|logical| {
                logical.span == expression
                    && [logical.left, logical.right]
                        .into_iter()
                        .any(|branch| ast.peel_ts_sugar_span(branch) == span)
            })
    };
    // The innermost element or fragment holding `inner`: the element, or
    // the fragment's span.
    let container = |inner: Span| -> Option<(Span, Option<&solid_facts::ast::JsxElementFact>)> {
        let element = ast
            .jsx_elements
            .iter()
            .filter(|element| element.span != inner && element.span.contains(inner))
            .min_by_key(|element| element.span.end - element.span.start);
        let fragment = ast
            .jsx_fragments
            .iter()
            .copied()
            .filter(|fragment| *fragment != inner && fragment.contains(inner))
            .min_by_key(|fragment| fragment.end - fragment.start);
        match (element, fragment) {
            (Some(element), Some(fragment))
                if fragment.end - fragment.start < element.span.end - element.span.start =>
            {
                Some((fragment, None))
            }
            (Some(element), _) => Some((element.span, Some(element))),
            (None, Some(fragment)) => Some((fragment, None)),
            (None, None) => None,
        }
    };
    let inserted_by =
        |holder: Option<(Span, Option<&solid_facts::ast::JsxElementFact>)>| match holder {
            Some((_, Some(element))) => super::events::intrinsic_element(file, element),
            Some((fragment, None)) => jsx_value_inserted(file, fragment, lookup, depth - 1),
            None => false,
        };
    let holder = container(span);
    // The child expression in the same holder that contains the value, if any.
    let expression = ast
        .jsx_child_expressions
        .iter()
        .copied()
        .find(|expression| {
            expression.contains(span)
                && container(*expression).map(|(holder, _)| holder)
                    == holder.map(|(holder, _)| holder)
        });
    let as_child = match expression {
        None => holder.is_some() && inserted_by(holder),
        Some(expression) => value_at(expression) && inserted_by(holder),
    };
    if as_child {
        return true;
    }
    let owner_of = |at: Span| crate::owners::owning_function(ast, at);
    ast.returns
        .iter()
        .filter_map(|returned| Some((owner_of(returned.span)?, returned.argument?)))
        .chain(ast.functions.iter().filter_map(|function| {
            Some((function, function.expression_return.as_ref()?.argument?))
        }))
        .any(|(function, argument)| {
            value_at(argument)
                && owner_of(span).is_some_and(|owner| owner.span == function.span)
                && lookup.function_proven_component(file, function)
        })
}
