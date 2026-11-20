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
        // ADR 0532: no result identity was proven; retain typed obligations.
        return false;
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
    Some(CallbackReturn {
        bindings: called_slots(lookup, file, outer, &returned, tuple_iteration_veto)?,
        created,
        primitive: Some(primitive),
    })
}

/// The names `outer` binds from `returned`, each one a slot the catalogue
/// gives a role, when every one is unwritten, unexported, has an entity, and
/// is read only as the callee of a call (no alias can carry it elsewhere).
fn called_slots(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    outer: &BindingFact,
    returned: &ReturnSummary,
    tuple_iteration_veto: bool,
) -> Option<Vec<(Span, ReturnSummary)>> {
    let bindings = selected(tuple_iteration_veto, outer, returned)?;
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
    Some(bindings)
}

/// ADR 1123: the arm's id. `SOLID_CHECKER_IR_ABLATE_SILENCE=context-provided-tuple`
/// switches it off (a measurement only: the gate's ablation identity).
pub(crate) const CONTEXT_PROVIDED_TUPLE: &str = "context-provided-tuple";

/// ADR 1123: `const [get, set] = useContext(Ctx)` where the one value `Ctx`
/// can hold is the fresh tuple of one `createSignal` call. The consumer's
/// slots are then that signal's accessor and setter, as a direct creation's
/// are (the slots' own symbols are the identity, as [`resolve`]'s are).
///
/// What proves it, all in `file` (an unexported binding is reachable from no
/// other module; a direct `eval` anywhere refuses). Each of the three Solid
/// calls it names (the context read, the `createContext` creation and the
/// `createSignal` creation) is resolved exactly ([`exactly_resolved`]): a
/// missing record, or one the producer recovered, is no evidence and refuses.
/// - `Ctx` is an unwritten, unexported `const` initialized by the dialect's
///   `createContext` with no argument: no default, so a read under no
///   provider throws (`ContextNotFoundError`) and returns nothing;
/// - every reference of `Ctx` is the tag of the one provider element
///   `<Ctx value={…}>` (no spread, one `value`), or the argument of a
///   dialect `useContext` call that an array pattern destructures (so no
///   holder of the shared tuple can replace a slot);
/// - the provider's value is the `createSignal` call itself, or an
///   unwritten, unexported `const` it initializes whose one reference is
///   that value, a plain value signal ([`plain_value_creation`]: no function
///   form, no options);
/// - the slots are read only as callees ([`called_slots`]), and nothing in
///   the program may mutate the iteration protocol the patterns use.
pub(super) fn context_provided_tuple<'a>(
    lookup: &'a SemanticLookup<'_>,
    file: &'a FileFacts,
    call: &CallFact,
) -> Option<CallbackReturn<'a>> {
    if lookup.primitive_at_call(file, call.span) != Some(Primitive::UseContext)
        || !context_read(lookup, file, call)
        || crate::site_accounting::ablated_id(CONTEXT_PROVIDED_TUPLE)
        || lookup.array_slots_withdrawn()
        || lookup.program_has_eval()
    {
        return None;
    }
    let argument = file.ast.peel_ts_sugar_span(call.arguments[0].span);
    let context = sole_reference_binding(file, argument)?;
    let created_context = file.ast.call_at(context.call_initializer?)?;
    if context
        .initializer
        .map(|value| file.ast.peel_ts_sugar_span(value))
        != Some(created_context.span)
        || created_context.construct
        || !created_context.arguments.is_empty()
        || lookup.primitive_at_call(file, created_context.span) != Some(Primitive::CreateContext)
        || !exactly_resolved(lookup, file, created_context.callee)
    {
        return None;
    }
    let declaration = context.names[0].span;
    let mut provider = None;
    for reference in references(file, declaration) {
        if let Some(element) = file.ast.jsx_elements.iter().find(|element| {
            element.member_object.is_none()
                && (element.name.span == reference || element.closing_name == Some(reference))
        }) {
            if provider
                .is_some_and(|seen: &solid_facts::ast::JsxElementFact| seen.span != element.span)
            {
                return None;
            }
            provider = Some(element);
            continue;
        }
        // A read the result of which an array pattern takes apart at once.
        let destructured = file.ast.calls.iter().any(|read| {
            read.arguments.len() == 1
                && file.ast.peel_ts_sugar_span(read.arguments[0].span) == reference
                && lookup.primitive_at_call(file, read.span) == Some(Primitive::UseContext)
                && context_read(lookup, file, read)
                && file.ast.bindings.iter().any(|binding| {
                    binding.shape == BindingShape::Array
                        && binding.call_initializer == Some(read.span)
                        && binding
                            .initializer
                            .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == read.span)
                })
        });
        if !destructured {
            return None;
        }
    }
    let provider = provider?;
    let mut values = provider.attributes.iter().filter(|attribute| {
        attribute.namespace.is_none() && file.source_text(attribute.local_name) == Some("value")
    });
    let value = values.next()?;
    if values.next().is_some() || !provider.spreads.is_empty() || value.runtime_type_escape {
        return None;
    }
    let value = file.ast.peel_ts_sugar_span(value.expression?);
    let created_span = if file.ast.call_at(value).is_some() {
        value
    } else {
        let held = sole_reference_binding(file, value)?;
        let initializer = file.ast.peel_ts_sugar_span(held.initializer?);
        if held.call_initializer != Some(initializer)
            || references(file, held.names[0].span).ne(std::iter::once(value))
        {
            return None;
        }
        initializer
    };
    let created = file.ast.call_at(created_span)?;
    if created.construct
        || !file.ast.straight_line_calls.contains(&created.span)
        || !plain_value_creation(created)
        // Fail closed like the context read: a creation with no resolved-call record passed here before (only a recovery refused).
        || !exactly_resolved(lookup, file, created.callee)
    {
        return None;
    }
    let primitive = lookup.primitive_at_call(file, created.span)?;
    if !lookup.dialect.returns_reactive_tuple(primitive) {
        return None;
    }
    let slot = |index| {
        lookup
            .dialect
            .reactive_result_slot(primitive, ResultSlot::TupleItem(index))
            .map(|role| ReturnSummary {
                kind: match role {
                    ReactiveRole::Accessor => "accessor",
                    ReactiveRole::Setter => "setter",
                }
                .into(),
                label: "context provided source".into(),
                ..ReturnSummary::default()
            })
    };
    let returned = ReturnSummary {
        kind: "tuple".into(),
        elements: vec![Some(slot(0)?), Some(slot(1)?)],
        ..ReturnSummary::default()
    };
    let outer = file.ast.bindings.iter().find(|binding| {
        binding.shape == BindingShape::Array
            && binding.call_initializer == Some(call.span)
            && binding
                .initializer
                .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == call.span)
    })?;
    let bindings = called_slots(lookup, file, outer, &returned, false)?;
    // The proof log names the arm at each call of a slot, the reads and writes it decided (monotone.mjs M2b).
    if crate::proof_log::enabled() {
        for (declaration, _) in &bindings {
            for reference in references(file, *declaration) {
                for use_call in file.ast.calls_with_peeled_callee(reference) {
                    crate::proof_log::decision(
                        "context-tuple",
                        file.path.as_str(),
                        use_call.span,
                        crate::ExecutionRole::Unknown,
                        Some((CONTEXT_PROVIDED_TUPLE, false)),
                        &[CONTEXT_PROVIDED_TUPLE],
                        Some(false),
                    );
                }
            }
        }
    }
    Some(CallbackReturn {
        bindings,
        created,
        primitive: Some(primitive),
    })
}

/// ADR 1123 (the review's F1): the provided signal is a plain value signal. `createSignal(fn)` is the function form, a derived signal whose
/// computation may be async (rc.21 makes it a writable memo), so a read under no `Loading` throws where the value form returns. Only no
/// argument, or one argument the facts prove is a non-function literal (null, a boolean, an integer, a string, an object or array literal),
/// with no options object and no spread, qualifies; anything else (an identifier, a call, a function, a second argument) keeps its rows.
fn plain_value_creation(created: &CallFact) -> bool {
    use solid_facts::ast::ArgumentLiteralFact;
    match created.arguments.as_slice() {
        [] => true,
        [value] => {
            !value.spread
                && !value.runtime_type_escape
                && matches!(
                    value.literal_value,
                    ArgumentLiteralFact::Null
                        | ArgumentLiteralFact::Boolean(_)
                        | ArgumentLiteralFact::Integer(_)
                        | ArgumentLiteralFact::String(_)
                        | ArgumentLiteralFact::Object(_)
                        | ArgumentLiteralFact::ArrayLength(_)
                )
        }
        _ => false,
    }
}

/// A plain call of the context read with one argument, no spread, resolved exactly.
fn context_read(lookup: &SemanticLookup<'_>, file: &FileFacts, call: &CallFact) -> bool {
    !call.construct
        && file.ast.straight_line_calls.contains(&call.span)
        && call.arguments.len() == 1
        && !call.arguments[0].spread
        && exactly_resolved(lookup, file, call.callee)
}

/// The callee at `callee` has a resolved-call record, and the producer did not recover it: a missing record is no evidence (fail closed), as a recovery
/// is none. `primitive_at_call` names a primitive from the symbol alone and needs no record, so each Solid call a proof here rests on asks this too.
fn exactly_resolved(lookup: &SemanticLookup<'_>, file: &FileFacts, callee: Span) -> bool {
    lookup
        .resolved_callee_call(file, callee)
        .is_some_and(|resolved| resolved.validity != typefacts::ResolvedCallValidity::Recovery)
}

/// The `const` of one name, in `file`, that the identifier reference at `span`
/// reads, when nothing writes it and no export (or a script's global scope)
/// publishes it.
fn sole_reference_binding(file: &FileFacts, span: Span) -> Option<&BindingFact> {
    if !file
        .ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == span && identifier.role == IdentifierRole::Reference)
    {
        return None;
    }
    let declaration = file.ast.reference_declaration(span)?;
    let binding = file
        .ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == declaration))?;
    (binding.immutable
        && binding.shape == BindingShape::Identifier
        && binding.names.len() == 1
        && !binding_written(file, declaration)
        && !crate::execution_role::flow_binding_exported(file, declaration)
        && !crate::indexes::binding_exported(file, declaration))
    .then_some(binding)
}

/// Every identifier reference in `file` whose declaration is `declaration`.
fn references(file: &FileFacts, declaration: Span) -> impl Iterator<Item = Span> + '_ {
    file.ast
        .identifiers
        .iter()
        .filter(move |identifier| {
            identifier.role == IdentifierRole::Reference
                && file.ast.reference_declaration(identifier.span) == Some(declaration)
        })
        .map(|identifier| identifier.span)
}
