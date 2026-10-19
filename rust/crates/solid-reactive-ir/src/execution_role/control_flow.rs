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
            let Some(member) = file
                .ast
                .members
                .iter()
                .find(|member| member.span == assignment.target)
            else {
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

pub(crate) fn control_flow_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<ExecutionRole> {
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
        return Some(ExecutionRole::DeferredCallback);
    }
    // `Show`/`Match` run a function child as a callback only when its
    // `length` is above 0; a function of length 0 (no parameters, or a
    // default on the first) is inserted as an ordinary child and runs in the
    // insert's tracked effect (review round 4). Only the child's own body:
    // a function nested in it keeps its own role (review round 5).
    let length = callback
        .parameters
        .iter()
        .take_while(|parameter| parameter.initializer.is_none())
        .count()
        .min(
            callback
                .parameters
                .len()
                .saturating_sub(usize::from(callback.rest_parameter)),
        );
    if length == 0
        && jsx_primitive_name(file, element, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| dialect.children_callback_requires_parameters(primitive))
    {
        return Some(ExecutionRole::TrackedJsx);
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
        Some(ExecutionRole::TrackedJsx)
    } else {
        Some(ExecutionRole::UntrackedRendering)
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
    dialect: &dyn Dialect,
) -> Option<ExecutionRole> {
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
    control_flow_execution_role(file, span, entities, symbol_names, dialect).filter(|role| {
        matches!(
            role,
            ExecutionRole::UntrackedRendering | ExecutionRole::TrackedJsx
        )
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
