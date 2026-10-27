//! Exact callback-result identity. Runtime passthrough belongs to the dialect;
//! literal, completion, binding and slot proofs belong to the shared engine.

use solid_dialect::{Primitive, ReactiveRole, ResultSlot};
use solid_facts::FileFacts;
use solid_facts::ast::{BindingFact, BindingShape, CallFact, FunctionKind, IdentifierRole};
use solid_facts::core::Span;

use crate::ReturnSummary;
use crate::indexes::{SemanticLookup, binding_written};
use crate::owners::containing_ast_function;

pub(super) struct CallbackReturn<'a> {
    pub(super) bindings: Vec<(Span, ReturnSummary)>,
    pub(super) created: &'a CallFact,
    pub(super) primitive: Option<Primitive>,
}

pub(super) fn callback_slot(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &CallFact,
) -> Option<usize> {
    lookup
        .dialect
        .returned_callback_value(lookup.primitive_at_call(file, call.span)?)
}

/// A resolved callback result's identity comes from the passthrough proof, not
/// from type annotations. Match references (including ones without a demanded
/// entity) by the binder.
pub(super) fn binding_is_callback_result(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    span: Span,
) -> bool {
    callback_result_binding_chain(lookup, file, span, 16)
}

fn callback_result_binding_chain(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    span: Span,
    depth: usize,
) -> bool {
    if depth == 0 {
        return true;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if let Some(call) = file.ast.call_at(span) {
        // Only a resolved passthrough replaces type-based identity; an
        // unresolved result keeps the evidence it had before.
        return resolve(lookup, file, call).is_some();
    }
    if let Some(member) = file.ast.member_at(span) {
        return callback_result_binding_chain(lookup, file, member.object, depth - 1);
    }
    let declaration = file.ast.reference_declaration(span).unwrap_or(span);
    file.ast.bindings.iter().any(|binding| {
        binding.names.iter().any(|name| name.span == declaration)
            && binding
                .initializer
                .is_some_and(|value| callback_result_binding_chain(lookup, file, value, depth - 1))
    })
}

fn selected(
    tuple_iteration_veto: bool,
    binding: &BindingFact,
    value: &ReturnSummary,
) -> Option<Vec<(Span, ReturnSummary)>> {
    if binding.shape == BindingShape::Array && value.kind == "tuple" && tuple_iteration_veto {
        return None;
    }
    let values = match binding.shape {
        BindingShape::Identifier if binding.names.len() == 1 => {
            vec![(binding.names[0].span, value.clone())]
        }
        BindingShape::Array if value.kind == "tuple" => binding
            .array_slots
            .iter()
            .enumerate()
            .filter_map(|(index, name)| name.as_ref().map(|name| (index, name)))
            .map(|(index, name)| Some((name.span, value.elements.get(index)?.as_ref()?.clone())))
            .collect::<Option<Vec<_>>>()?,
        BindingShape::Object if value.kind == "object" => binding
            .object_slots
            .iter()
            .map(|slot| {
                Some((
                    slot.local.span,
                    value.properties.get(slot.property.as_str())?.clone(),
                ))
            })
            .collect::<Option<Vec<_>>>()?,
        BindingShape::Identifier | BindingShape::Array | BindingShape::Object => return None,
    };
    // Rest, defaults, nested and computed patterns have names but no exact
    // slots. Refuse the whole binding instead of guessing any omitted slot.
    (binding.immutable
        && !values.is_empty()
        && values.len() == binding.names.len()
        && binding
            .names
            .iter()
            .all(|name| values.iter().any(|(span, _)| *span == name.span)))
    .then_some(values)
}

fn only_returned(file: &FileFacts, binding: &BindingFact, returned: Span) -> bool {
    binding.names.iter().all(|name| {
        !binding_written(file, name.span)
            && file.ast.identifiers.iter().all(|id| {
                id.role != IdentifierRole::Reference
                    || file.ast.reference_declaration(id.span) != Some(name.span)
                    || id.span == returned
            })
    })
}

/// One synchronous literal, no parameters (dispose is deliberately open),
/// one return with complete normal-completion coverage, and one exact factory.
pub(super) fn resolve<'a>(
    lookup: &'a SemanticLookup<'_>,
    file: &'a FileFacts,
    call: &CallFact,
) -> Option<CallbackReturn<'a>> {
    resolve_with_iteration_veto(lookup, file, call, lookup.tuple_iteration_may_be_mutated())
}

/// Former slot identities are candidates for uncertainty only. A veto must
/// not erase their calls merely because published types are plain functions.
pub(super) fn withdrawn_tuple_candidates(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &CallFact,
) -> Vec<(Span, ReturnSummary)> {
    if !lookup.tuple_iteration_may_be_mutated() || resolve(lookup, file, call).is_some() {
        return Vec::new();
    }
    resolve_with_iteration_veto(lookup, file, call, false)
        .map(|returned| returned.bindings)
        .unwrap_or_default()
}

fn resolve_with_iteration_veto<'a>(
    lookup: &'a SemanticLookup<'_>,
    file: &'a FileFacts,
    call: &CallFact,
    tuple_iteration_veto: bool,
) -> Option<CallbackReturn<'a>> {
    let index = callback_slot(lookup, file, call)?;
    if call.construct
        || !file.ast.straight_line_calls.contains(&call.span)
        || call.arguments.iter().any(|argument| argument.spread)
        || lookup
            .resolved_callee_call(file, call.callee)
            .is_some_and(|resolved| resolved.validity == typefacts::ResolvedCallValidity::Recovery)
    {
        return None;
    }
    let argument = file.ast.peel_ts_sugar_span(call.arguments.get(index)?.span);
    let function = file.ast.functions.iter().find(|function| {
        function.span == argument
            && matches!(
                function.kind,
                FunctionKind::Arrow | FunctionKind::Expression
            )
    })?;
    if function.r#async
        || function.generator
        || function.name.is_some()
        || !function.parameters.is_empty()
        || function.rest_parameter
    {
        return None;
    }
    let returns = function
        .expression_return
        .iter()
        .chain(file.ast.returns.iter().filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        }))
        .collect::<Vec<_>>();
    if returns.len() != 1 {
        return None;
    }
    let value = file.ast.peel_ts_sugar_span(returns[0].argument?);
    if !function.expression_body
        && solid_facts::ast::completion_return_cover(
            std::path::Path::new(file.path.as_str()),
            &file.source,
            function.body,
            &[value],
        ) != Some(true)
    {
        return None;
    }
    let local = file
        .ast
        .reference_declaration(value)
        .and_then(|declaration| {
            file.ast.bindings.iter().find(|binding| {
                binding.names.iter().any(|name| name.span == declaration)
                    && crate::owners::owned_by_body(&file.ast, function, binding.declaration)
            })
        });
    let created_span = match local {
        Some(binding) if only_returned(file, binding, value) => {
            let initializer = file.ast.peel_ts_sugar_span(binding.initializer?);
            if initializer.end > returns[0].span.start {
                return None;
            }
            initializer
        }
        Some(_) => return None,
        None => value,
    };
    let created = file.ast.call_at(created_span)?;
    if created.construct
        || !file.ast.straight_line_calls.contains(&created.span)
        || created.arguments.iter().any(|argument| argument.spread)
        || lookup
            .resolved_callee_call(file, created.callee)
            .is_some_and(|resolved| resolved.validity == typefacts::ResolvedCallValidity::Recovery)
        || !crate::owners::owned_by_body(&file.ast, function, created.span)
    {
        return None;
    }
    // Core primitives use the dialect's exact slot catalogue; no other
    // callee states its result.
    let primitive = lookup.primitive_at_call(file, created.span)?;
    let shape = |slot| {
        lookup
            .dialect
            .reactive_result_slot(primitive, slot)
            .map(|role| ReturnSummary {
                kind: match role {
                    ReactiveRole::Accessor => "accessor",
                    ReactiveRole::Setter => "setter",
                }
                .into(),
                label: "callback returned source".into(),
                ..ReturnSummary::default()
            })
    };
    let mut returned = if lookup.dialect.returns_reactive_tuple(primitive) {
        ReturnSummary {
            kind: "tuple".into(),
            elements: vec![
                shape(ResultSlot::TupleItem(0)),
                shape(ResultSlot::TupleItem(1)),
            ],
            ..ReturnSummary::default()
        }
    } else {
        shape(ResultSlot::Whole)?
    };
    if let Some(binding) = local {
        returned = selected(tuple_iteration_veto, binding, &returned)?
            .into_iter()
            .find(|(declaration, _)| file.ast.reference_declaration(value) == Some(*declaration))?
            .1;
    }
    let outer = file.ast.bindings.iter().find(|binding| {
        binding
            .initializer
            .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == call.span)
    })?;
    let bindings = selected(tuple_iteration_veto, outer, &returned)?;
    for (declaration, returned) in &bindings {
        if !matches!(returned.kind.as_str(), "accessor" | "setter" | "plain")
            || binding_written(file, *declaration)
            || lookup
                .entity_at(file.path.as_str(), *declaration)
                .is_none_or(|entity| entity.symbol.is_empty())
            || file.ast.exports.iter().any(|export| {
                export
                    .declarations
                    .iter()
                    .chain(&export.specifiers)
                    .any(|specifier| {
                        specifier.local.span == *declaration
                            || file.ast.reference_declaration(specifier.local.span)
                                == Some(*declaration)
                    })
            })
            || file.ast.identifiers.iter().any(|id| {
                id.role == IdentifierRole::Reference
                    && file.ast.reference_declaration(id.span) == Some(*declaration)
                    && !file.ast.calls.iter().any(|use_call| {
                        file.ast.peel_ts_sugar_span(use_call.callee) == id.span
                            && !use_call.construct
                    })
            })
        {
            return None;
        }
    }
    Some(CallbackReturn {
        bindings,
        created,
        primitive: Some(primitive),
    })
}
