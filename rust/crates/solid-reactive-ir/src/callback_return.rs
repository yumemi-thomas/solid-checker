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

/// ADR 1147: the owner arm's id. `SOLID_CHECKER_IR_ABLATE_SILENCE=context-provider-children-owned` switches it off (a measurement only).
pub(crate) const CONTEXT_PROVIDER_CHILDREN_OWNED: &str = "context-provider-children-owned";

/// ADR 1147: whether `element` is a dialect context provider whose children only the provider's own `children()` memo evaluates: rc.21's provider is
/// `createRoot(() => { setContext(provider, props.value); return children(() => props.children); })`, so a child runs inside that memo's computation, whose
/// owner is the memo (created under the provider's root) wherever and whenever the memo is read. The tag is the context object itself, by the tuple arm's test
/// ([`sole_reference_binding`]: a `const` of one name, unwritten, unexported; the review of 1123's F2: a `let`, a written binding, a pattern's name or an import
/// may hold another component), initialized by the dialect's `createContext` (any default: the default changes no provider), resolved exactly ([`exactly_resolved`]).
/// The element has no spread and no `children` attribute (only its written children reach the memo), and the program has no direct `eval`.
pub(crate) fn exact_context_provider(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> bool {
    if element.member_object.is_some()
        || !element.spreads.is_empty()
        || lookup.program_has_eval()
        || element.attributes.iter().any(|attribute| {
            attribute.namespace.is_some()
                || file.source_text(attribute.local_name) == Some("children")
        })
    {
        return false;
    }
    let Some(context) = sole_reference_binding(file, element.name.span) else {
        return false;
    };
    let Some(created) = context
        .call_initializer
        .and_then(|call| file.ast.call_at(call))
    else {
        return false;
    };
    context
        .initializer
        .map(|value| file.ast.peel_ts_sugar_span(value))
        == Some(created.span)
        && !created.construct
        && lookup.primitive_at_call(file, created.span) == Some(Primitive::CreateContext)
        && exactly_resolved(lookup, file, created.callee)
}

/// ADR 1155 (b): the id `SOLID_CHECKER_IR_ABLATE_SILENCE` names to turn [`in_exact_provider_children`] off (the monotone gate's ablation).
pub(crate) const PROVIDER_CHILDREN_MEMO_OWNER: &str = "context-provider-children-memo-owner";

/// ADR 1155 (b), the Opus review of ADR 1147 (N1): whether code at `span` is written in a child of an exact context provider ([`exact_context_provider`]) with no
/// function between. rc.21 evaluates a provider's children only inside its `children()` memo, whose owner is
/// that memo (created under the provider's root), so such code creates and registers cleanups under an owner wherever the provider element is created: an owner
/// question there is answered by the memo, never by the code around the provider (a tracked effect, an effect's apply, a timer). The tag's own creation is not in its
/// children and keeps its row; a flush or an after-await read (stack questions) must not ask this.
pub(crate) fn in_exact_provider_children(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    span: Span,
) -> bool {
    if crate::site_accounting::ablated_id(PROVIDER_CHILDREN_MEMO_OWNER) {
        return false;
    }
    let ast = &file.ast;
    let owned = ast.jsx_elements.iter().any(|element| {
        element.span != span
            && element.children.iter().any(|child| child.contains(span))
            && !ast
                .functions_containing(span)
                .any(|between| element.span.contains(between.span))
            && exact_context_provider(lookup, file, element)
    });
    // The proof log names the arm where it decides (monotone.mjs M2b).
    if owned {
        crate::proof_log::decision(
            "owner-call",
            file.path.as_str(),
            span,
            crate::ExecutionRole::Unknown,
            Some((PROVIDER_CHILDREN_MEMO_OWNER, false)),
            &[PROVIDER_CHILDREN_MEMO_OWNER],
            Some(false),
        );
    }
    owned
}

/// ADR 1155 (b): the child spans of the exact context providers of `file`, owner-providing regions of the owner analysis
/// ([`crate::owners::OwnerFileIndex`]): each child is evaluated by the provider's children memo, under that memo's owner.
pub(crate) fn exact_provider_child_spans(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
) -> Vec<Span> {
    if crate::site_accounting::ablated_id(PROVIDER_CHILDREN_MEMO_OWNER) {
        return Vec::new();
    }
    file.ast
        .jsx_elements
        .iter()
        .filter(|element| {
            !element.children.is_empty() && exact_context_provider(lookup, file, element)
        })
        .flat_map(|element| element.children.iter().copied())
        .collect()
}

/// ADR 1147: the store-tuple arm's id. `SOLID_CHECKER_IR_ABLATE_SILENCE=context-provided-store-tuple` switches it off (a measurement only).
pub(crate) const CONTEXT_PROVIDED_STORE_TUPLE: &str = "context-provided-store-tuple";

/// ADR 1147 (arm 3): one context whose one value is a fresh `[store, { actions }]` literal, and every reader that takes it apart.
#[derive(Clone, Debug)]
pub(crate) struct ProvidedStoreTuple {
    /// The provider's value, as written (`[cart, { add }] as const`, or the `const` that holds it).
    pub(crate) value: Span,
    /// The `createStore` call.
    pub(crate) created: Span,
    /// Each action: its key, the identifier the literal holds it by, and the function the identifier's `const` holds.
    pub(crate) actions: Vec<(String, Span, Span)>,
    pub(crate) consumers: Vec<StoreTupleConsumer>,
}

/// ADR 1147 (arm 3): a reader `const [store, { add }] = useContext(Ctx)`.
#[derive(Clone, Debug)]
pub(crate) struct StoreTupleConsumer {
    /// The binding's index in the file's `bindings`.
    pub(crate) binding: usize,
    /// The slot-0 name, the store: `None` for a hole.
    pub(crate) root: Option<Span>,
    /// The slot-1 object pattern's names: (key, local name). Each local is only called.
    pub(crate) actions: Vec<(String, Span)>,
}

/// ADR 1147 (arm 3): the contexts of `file` whose one value is a fresh `[store, { actions }]` literal (textbook `id-context`). The premises are ADR 1123's tuple
/// arm's, with the value a literal instead of a signal's tuple:
/// - the context is an unwritten, unexported `const` of one name initialized by the dialect's `createContext` with no argument (no default: a read under no
///   provider throws), resolved exactly; every reference of it is the tag of the one provider element (no spread, one `value`) or the argument of an exact
///   `useContext` whose result a `const` array pattern takes apart at once ([`context_read`]): so the only holders of the shared tuple are the readers' patterns;
/// - the value is an array literal (or a `const` it initializes whose one reference is the value), of a store root and, optionally, an object literal: the root is
///   slot 0 of a `const` array pattern a `createStore` call initializes, resolved exactly; the object is closed (no spread, computed key, accessor or method), each
///   property a data property whose value is a `const` identifier bound to a function the binding holds now;
/// - each reader's pattern names only slot 0 (one identifier, no default) and slot 1 (an object pattern of plain keys the object holds, each an identifier with no
///   default; no rest): no holder of the action object exists but the literal, and each action's local is only called (`add(…)`), never written or handed on;
/// - no file builds code from a string (ADR 1137's veto, [`SemanticLookup::program_builds_code`]) and nothing may mutate the iteration protocol the patterns use
///   (`array_slots_withdrawn`, as for ADR 1123).
///
/// Anything else is no plan: the reads keep SC9012 and the writes their rows.
pub(crate) fn provided_store_tuples(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
) -> Vec<ProvidedStoreTuple> {
    if crate::site_accounting::ablated_id(CONTEXT_PROVIDED_STORE_TUPLE)
        || !file.source.contains("useContext")
        || lookup.program_builds_code()
        || lookup.array_slots_withdrawn()
    {
        return Vec::new();
    }
    file.ast
        .bindings
        .iter()
        .filter_map(|binding| provided_store_tuple(lookup, file, binding))
        .collect()
}

fn provided_store_tuple(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    context: &BindingFact,
) -> Option<ProvidedStoreTuple> {
    let ast = &file.ast;
    let created_context = ast.call_at(context.call_initializer?)?;
    if lookup.primitive_at_call(file, created_context.span) != Some(Primitive::CreateContext) {
        return None;
    }
    let declaration = context.names.first()?.span;
    if !context.immutable
        || context.shape != BindingShape::Identifier
        || context.names.len() != 1
        || binding_written(file, declaration)
        || crate::execution_role::flow_binding_exported(file, declaration)
        || crate::indexes::binding_exported(file, declaration)
        || context
            .initializer
            .map(|value| ast.peel_ts_sugar_span(value))
            != Some(created_context.span)
        || created_context.construct
        || !created_context.arguments.is_empty()
        || !exactly_resolved(lookup, file, created_context.callee)
    {
        return None;
    }
    let mut provider: Option<&solid_facts::ast::JsxElementFact> = None;
    let mut readers = Vec::new();
    for reference in references(file, declaration) {
        if let Some(element) = ast.jsx_elements.iter().find(|element| {
            element.member_object.is_none()
                && (element.name.span == reference || element.closing_name == Some(reference))
        }) {
            if provider.is_some_and(|seen| seen.span != element.span) {
                return None;
            }
            provider = Some(element);
            continue;
        }
        let read = ast.calls.iter().find(|read| {
            read.arguments.len() == 1
                && ast.peel_ts_sugar_span(read.arguments[0].span) == reference
                && lookup.primitive_at_call(file, read.span) == Some(Primitive::UseContext)
                && context_read(lookup, file, read)
        })?;
        let (index, _) = ast.bindings.iter().enumerate().find(|(_, binding)| {
            binding.shape == BindingShape::Array
                && binding.immutable
                && !binding.pattern_rest
                && binding.call_initializer == Some(read.span)
                && binding
                    .initializer
                    .is_some_and(|value| ast.peel_ts_sugar_span(value) == read.span)
        })?;
        readers.push(index);
    }
    let provider = provider?;
    let mut values = provider.attributes.iter().filter(|attribute| {
        attribute.namespace.is_none() && file.source_text(attribute.local_name) == Some("value")
    });
    let value_attribute = values.next()?;
    // `as const` is an assertion (`runtime_type_escape`): it changes what type-checks, not the value; the value is read by its syntax below.
    if values.next().is_some() || provider.attributes.len() != 1 || !provider.spreads.is_empty() {
        return None;
    }
    let value = ast.peel_ts_sugar_span(value_attribute.expression?);
    let literal_span = if ast
        .array_literal_elements
        .iter()
        .any(|array| array.span == value)
    {
        value
    } else {
        let held = sole_reference_binding(file, value)?;
        let initializer = ast.peel_ts_sugar_span(held.initializer?);
        if references(file, held.names[0].span).ne(std::iter::once(value)) {
            return None;
        }
        initializer
    };
    let literal = ast
        .array_literal_elements
        .iter()
        .find(|array| array.span == literal_span)?;
    if literal.elements.is_empty()
        || literal.elements.len() > 2
        || literal
            .elements
            .iter()
            .any(|element| element.kind != solid_facts::ast::ArrayElementKind::Value)
    {
        return None;
    }
    let (_root, created) = store_root(
        lookup,
        file,
        ast.peel_ts_sugar_span(literal.elements[0].span),
    )?;
    let actions = match literal.elements.get(1) {
        Some(element) => action_object(lookup, file, ast.peel_ts_sugar_span(element.span))?,
        None => Vec::new(),
    };
    let consumers = readers
        .into_iter()
        .map(|index| store_tuple_consumer(lookup, file, index, &actions))
        .collect::<Option<Vec<_>>>()?;
    Some(ProvidedStoreTuple {
        value,
        created,
        actions,
        consumers,
    })
}

/// ADR 1147: the declaration of the store root the identifier at `reference` reads: slot 0 of an unwritten `const` array pattern a `createStore` call initializes,
/// resolved exactly.
fn store_root(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    reference: Span,
) -> Option<(Span, Span)> {
    let ast = &file.ast;
    if !ast.identifiers.iter().any(|identifier| {
        identifier.span == reference && identifier.role == IdentifierRole::Reference
    }) {
        return None;
    }
    let declaration = ast.reference_declaration(reference)?;
    let binding = ast.bindings.iter().find(|binding| {
        binding.shape == BindingShape::Array
            && binding
                .array_slots
                .first()
                .and_then(Option::as_ref)
                .map(|name| name.span)
                == Some(declaration)
    })?;
    let created = ast.call_at(binding.call_initializer?)?;
    (binding.immutable
        && !binding_written(file, declaration)
        && binding
            .initializer
            .map(|value| ast.peel_ts_sugar_span(value))
            == Some(created.span)
        && !created.construct
        && lookup.primitive_at_call(file, created.span) == Some(Primitive::CreateStore)
        && exactly_resolved(lookup, file, created.callee))
    .then_some((declaration, created.span))
}

/// ADR 1147 (arm 3): the proof log names the arm wherever it decides (monotone.mjs M2b): the provider's value (the root's census), each reader's declarator (its
/// pattern and read), each reference of a reader's store name (the alias's reads) and each action's function (the writes its entries place).
pub(crate) fn log_store_tuple_decisions(file: &FileFacts, plans: &[ProvidedStoreTuple]) {
    if !crate::proof_log::enabled() {
        return;
    }
    let decide = |span: Span| {
        crate::proof_log::decision(
            "context-store-tuple",
            file.path.as_str(),
            span,
            crate::ExecutionRole::Unknown,
            Some((CONTEXT_PROVIDED_STORE_TUPLE, false)),
            &[CONTEXT_PROVIDED_STORE_TUPLE],
            Some(false),
        );
    };
    for plan in plans {
        decide(plan.value);
        for (_, held, function) in &plan.actions {
            decide(*held);
            decide(*function);
        }
        for consumer in &plan.consumers {
            decide(file.ast.bindings[consumer.binding].declaration);
            if let Some(name) = consumer.root {
                references(file, name).for_each(decide);
            }
            for (_, local) in &consumer.actions {
                references(file, *local).for_each(decide);
            }
        }
    }
}

/// ADR 1147 (arm 3): the entries of a function that is an action of a provided `[store, { actions }]` literal of `file`: the references the literal holds it
/// by (each an entry the readers' calls account for) and the callee of every call of a reader's local for its key (each local is only called). `None` for any
/// other function.
pub(crate) fn store_tuple_action_entries(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    function: Span,
) -> Option<(Vec<Span>, Vec<Span>)> {
    if !file.source.contains("useContext") {
        return None;
    }
    let mut held = Vec::new();
    let mut calls = Vec::new();
    for plan in provided_store_tuples(lookup, file) {
        for (key, at, action) in &plan.actions {
            if *action != function {
                continue;
            }
            held.push(*at);
            for consumer in &plan.consumers {
                for (local_key, local) in &consumer.actions {
                    if local_key != key {
                        continue;
                    }
                    for reference in references(file, *local) {
                        calls.extend(
                            file.ast
                                .calls_with_peeled_callee(reference)
                                .filter(|call| !call.construct)
                                .map(|call| call.callee),
                        );
                    }
                }
            }
        }
    }
    (!held.is_empty()).then_some((held, calls))
}

/// ADR 1147 (the review's F1): whether every function between the reader `reader` (its function; `None`, module scope, refuses) and an action's call at `call` is
/// run by the dialect or the DOM where it is written: the whole argument of a dialect primitive's call, or the whole expression of an intrinsic element's
/// attribute. A callback handed to a package (`hand(() => add("x"))`), a project function, or any other position refuses the plan: the write keeps its row.
fn action_call_placed(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: Span,
    reader: Option<Span>,
) -> bool {
    // A reader with no function of its own (module scope) is no component: refused.
    let Some(reader) = reader else {
        return false;
    };
    let ast = &file.ast;
    ast.functions_containing(call)
        .filter(|function| function.span != reader && reader.contains(function.span))
        .all(|function| {
            let primitive_argument =
                ast.arguments_containing(function.span)
                    .any(|(outer, index)| {
                        ast.peel_ts_sugar_span(outer.arguments[index].span) == function.span
                            && !outer.arguments[index].spread
                            && lookup.primitive_at_call(file, outer.span).is_some()
                    });
            let intrinsic_attribute = ast.jsx_elements.iter().any(|element| {
                crate::execution_role::intrinsic_element(file, element)
                    && element.attributes.iter().any(|attribute| {
                        attribute
                            .expression
                            .is_some_and(|value| ast.peel_ts_sugar_span(value) == function.span)
                    })
            });
            primitive_argument || intrinsic_attribute
        })
}

/// ADR 1147: the actions of a closed object literal at `span`: each property a data property under a plain key whose value is a `const` identifier bound to a
/// function the binding holds now, as (key, the identifier, the function).
fn action_object(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    span: Span,
) -> Option<Vec<(String, Span, Span)>> {
    let ast = &file.ast;
    let shape = ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == span)?;
    if !shape.closed || ast.spreads.iter().any(|spread| span.contains(spread.span)) {
        return None;
    }
    let direct = |inner: Span| {
        span.contains(inner)
            && inner != span
            && !ast.object_properties.iter().any(|property| {
                property.span != inner
                    && property.span.contains(inner)
                    && span.contains(property.span)
            })
            && !ast.object_get_shapes.iter().any(|other| {
                other.span != span && span.contains(other.span) && other.span.contains(inner)
            })
    };
    let mut actions: Vec<(String, Span, Span)> = Vec::new();
    for property in ast
        .object_properties
        .iter()
        .filter(|property| direct(property.span))
    {
        let key = file.source_text(property.key)?;
        if property.computed
            || !property.data
            || property.method
            || property.runtime_type_escape
            || !key
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
            || actions.iter().any(|(seen, _, _)| seen == key)
        {
            return None;
        }
        let held = ast.peel_ts_sugar_span(property.value);
        let binding = sole_local_binding(file, held)?;
        let initializer = ast.peel_ts_sugar_span(binding.initializer?);
        let function = ast
            .functions
            .iter()
            .find(|function| function.span == initializer)?;
        if !lookup.function_value_is_current(file, function) {
            return None;
        }
        actions.push((key.to_owned(), held, function.span));
    }
    (actions.len() == shape.properties.len()).then_some(actions)
}

/// The `const` of one name, in `file`, that the identifier reference at `span` reads, when nothing writes it (exported or not: a reader's other uses of the
/// function are its call sites, which the call graph keeps).
fn sole_local_binding(file: &FileFacts, span: Span) -> Option<&BindingFact> {
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
        && !binding_written(file, declaration))
    .then_some(binding)
}

/// ADR 1147: a reader's pattern, by the pattern elements of the binding (facts schema 62: a hole has no element): slot 0 one identifier with no default, slot 1
/// an object pattern of plain keys of `actions`, each an identifier with no default, and nothing else; each action local only called.
fn store_tuple_consumer(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    index: usize,
    actions: &[(String, Span, Span)],
) -> Option<StoreTupleConsumer> {
    let ast = &file.ast;
    let binding = &ast.bindings[index];
    let mut root = None;
    let mut locals = Vec::new();
    let mut named = 0;
    for element in ast
        .pattern_elements
        .iter()
        .filter(|element| element.pattern == binding.pattern)
    {
        if element.assignment || element.default.is_some() {
            return None;
        }
        match element.slot {
            solid_facts::ast::PatternSlot::Index(0) => {
                if !binding.names.iter().any(|name| name.span == element.target) {
                    return None;
                }
                root = Some(element.target);
                named += 1;
            }
            solid_facts::ast::PatternSlot::Index(1) => {
                for inner in ast
                    .pattern_elements
                    .iter()
                    .filter(|inner| inner.pattern == element.target)
                {
                    let solid_facts::ast::PatternSlot::Key(key) = &inner.slot else {
                        return None;
                    };
                    if inner.assignment
                        || inner.default.is_some()
                        || !binding.names.iter().any(|name| name.span == inner.target)
                        || !actions.iter().any(|(action, _, _)| action == key.as_str())
                        || locals
                            .iter()
                            .any(|(seen, _): &(String, Span)| seen == key.as_str())
                    {
                        return None;
                    }
                    // The local is only called: never written, handed on or read as a value; and each call is written where the dialect or the DOM runs
                    // it (the review of 1147, F1: a call in a callback handed to a package, `hand(() => add("x"))`, runs where the package says).
                    let reader = ast
                        .innermost_function_owning(binding.declaration)
                        .map(|function| function.span);
                    let only_called = references(file, inner.target).all(|reference| {
                        ast.calls_with_peeled_callee(reference).any(|call| {
                            !call.construct && action_call_placed(lookup, file, call.span, reader)
                        })
                    });
                    if !only_called {
                        return None;
                    }
                    locals.push((key.to_string(), inner.target));
                    named += 1;
                }
                // An object pattern element holds no plain name itself; a nested array pattern or an identifier at slot 1 is refused below.
                if binding.names.iter().any(|name| name.span == element.target) {
                    return None;
                }
            }
            _ => return None,
        }
    }
    (named == binding.names.len()).then_some(StoreTupleConsumer {
        binding: index,
        root,
        actions: locals,
    })
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
