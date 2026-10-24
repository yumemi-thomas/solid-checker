//! Returned sources: callback return sources, forwarded and proxied targets, and effective return contexts.

use super::*;

/// Rebind the exact callback value, never the callback's parameter or its
/// function identity. This produces the same local read evidence as a direct
/// factory binding and leaves the caller's execution role untouched.
pub(super) fn push_callback_return_sources(
    result: &mut SourceDiscoveryContribution,
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    entities: &EntitySymbols,
    returned: crate::callback_return::CallbackReturn<'_>,
) {
    for (name, value) in returned.bindings {
        let Some(symbol) = entities.at(file.path.as_str(), name) else {
            continue;
        };
        let declaration = crate::location(file.path.shared(), name);
        let display = symbol_id(file.source_text(name).unwrap_or_default());
        if value.kind == "accessor" {
            result
                .accessors
                .push((symbol.clone(), (display, declaration)));
            result
                .source_kinds
                .push((symbol.clone(), ReactiveSourceKind::Accessor));
        } else if value.kind == "setter" {
            result.setters.push((
                symbol.clone(),
                (
                    display,
                    declaration,
                    returned.created.owned_write_option,
                    ReactiveSourceKind::Accessor,
                ),
            ));
        } else {
            continue;
        }
        result.source_phases.push((symbol.clone(), 1));
        if let Some(primitive) = returned.primitive {
            if let Some(spelling) = lookup.dialect.name_of(primitive) {
                result
                    .source_primitives
                    .push((symbol.clone(), spelling.into()));
            }
            result
                .source_owned_write
                .push((symbol.clone(), returned.created.owned_write_option));
            let options =
                async_source_options(file, returned.created, Some(primitive), lookup.dialect);
            if options != AsyncSourceOptions::default() {
                result.source_async_options.push((symbol.clone(), options));
            }
            if value.kind == "accessor"
                && returned
                    .created
                    .arguments
                    .first()
                    .is_some_and(|argument| computation_is_async(lookup, file, argument.span))
            {
                result.async_sources.push(symbol.clone());
            }
        }
    }
}

pub(super) struct EffectiveReturnContext<'a> {
    pub(super) tuple_iteration_veto: bool,
    pub(super) file: &'a FileFacts,
    pub(super) ast_index: &'a CachedAstFileIndex,
    pub(super) entities: &'a EntitySymbols,
    pub(super) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(super) dialect: &'a dyn Dialect,
}

/// A fresh source returned by one exact project function. This is deliberately
/// separate from structured-return summaries: finding one reactive leaf, or
/// merging equal kinds, cannot prove that every completion returns one value.
pub(crate) fn project_returned_source<'a>(
    lookup: &SemanticLookup<'a>,
    caller: &FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> Option<(&'a FileFacts, &'a solid_facts::ast::CallFact, Primitive)> {
    returned_source_through(
        lookup,
        caller,
        call,
        entities,
        symbol_names,
        (
            RETURNED_SOURCE_FORWARDS,
            lookup.tuple_iteration_may_be_mutated(),
        ),
    )
}

/// How many same-file `return g(…)` hops (ADR 0287) a returned-source proof
/// follows before it refuses.
pub(super) const RETURNED_SOURCE_FORWARDS: usize = 4;

/// ADR 0287: the store a `new Proxy(root, { get })` return reads exactly, as
/// the span of `root`. The construct must be the standard library's `Proxy`
/// constructor, and its handler must forward every read
/// ([`solid_facts::ast::proxy_get_forward_target`]); which binding `root`
/// names, and that it is a store, the caller proves.
pub(super) fn proxied_target(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    value: solid_facts::core::Span,
) -> Option<solid_facts::core::Span> {
    let construct = file
        .ast
        .calls
        .iter()
        .find(|call| call.construct && call.span == value)?;
    let resolved = lookup.resolved_callee_call(file, construct.callee)?;
    let declaration = resolved.declaration.as_ref()?;
    if resolved.kind != typefacts::CallKind::Construct
        || !declaration.standard_library
        || declaration.qualified_name.as_ref() != "ProxyConstructor.construct"
        || construct.arguments.len() != 2
        || construct.arguments.iter().any(|argument| argument.spread)
    {
        return None;
    }
    let target = solid_facts::ast::proxy_get_forward_target(
        std::path::Path::new(file.path.as_str()),
        &file.source,
        construct.span,
    )?;
    (file.ast.peel_ts_sugar_span(construct.arguments[0].span) == target).then_some(target)
}

pub(super) fn returned_source_through<'a>(
    lookup: &SemanticLookup<'a>,
    caller: &FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    proof: (usize, bool),
) -> Option<(&'a FileFacts, &'a solid_facts::ast::CallFact, Primitive)> {
    let (forwards, tuple_iteration_veto) = proof;
    // This first slice only admits straight-line calls, which also excludes
    // optional invocation and optional receivers. No selected signature is
    // substituted for a value identity.
    if call.construct || !caller.ast.straight_line_calls.contains(&call.span) {
        return None;
    }
    let callee = caller.ast.peel_ts_sugar_span(call.callee);
    let target = if caller.ast.identifiers.iter().any(|id| id.span == callee) {
        lookup.function_called_at(caller.path.as_str(), callee)
    } else {
        lookup.namespace_member_function(caller, callee)
    };
    let (file, function) = target?;
    if function.r#async
        || function.generator
        || function.expression_body
        || function.method_name.is_some()
        || !lookup.function_value_is_current(file, function)
    {
        return None;
    }
    let returns = file
        .ast
        .returns
        .iter()
        .filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        })
        .collect::<Vec<_>>();
    let mut source = None;
    let mut sites = Vec::new();
    let mut roots = Vec::new();
    // ADR 0287: the target of a forwarding proxy is the root, and naming it
    // there hands it only to the proven trap.
    let mut proxy_targets = Vec::new();
    let mut forwarded = None::<(&'a FileFacts, &'a solid_facts::ast::CallFact, Primitive)>;
    for returned in &returns {
        let value = file.ast.peel_ts_sugar_span(returned.argument?);
        sites.push(value);
        if file.ast.identifiers.iter().any(|id| id.span == value) {
            roots.push(value);
            continue;
        }
        if let Some(target) = proxied_target(lookup, file, value) {
            roots.push(target);
            proxy_targets.push(target);
            continue;
        }
        // ADR 0287: `return g(…)`, where `g` is a function in this same file
        // whose every return yields a fresh source. Each call of this
        // function runs that one call and returns its value. The proof stays
        // in one file; the cross-file digest binds its answer, not this
        // syntax (ADR 03XX-s6a).
        let inner = file
            .ast
            .calls
            .iter()
            .find(|inner| !inner.construct && inner.span == value)?;
        let next = forwards.checked_sub(1)?;
        let found = returned_source_through(
            lookup,
            file,
            inner,
            entities,
            symbol_names,
            (next, tuple_iteration_veto),
        )?;
        if found.0.path != file.path
            || forwarded.is_some_and(|prior: (&FileFacts, &solid_facts::ast::CallFact, _)| {
                prior.1.span != found.1.span
            })
        {
            return None;
        }
        forwarded = Some(found);
    }
    if let Some(found) = forwarded {
        return (roots.is_empty()
            && solid_facts::ast::completion_return_cover(
                std::path::Path::new(file.path.as_str()),
                &file.source,
                function.body,
                &sites,
            ) == Some(true))
        .then_some(found);
    }
    for root in &roots {
        let symbol = entities.at(file.path.as_str(), *root)?;
        if source.is_some_and(|prior| prior != symbol) {
            return None;
        }
        source = Some(symbol);
    }
    let source = source?;
    let binding = file.ast.bindings.iter().find(|binding| {
        binding
            .names
            .iter()
            .any(|name| entities.at(file.path.as_str(), name.span) == Some(source))
    })?;
    let name = binding
        .names
        .iter()
        .find(|name| entities.at(file.path.as_str(), name.span) == Some(source))?;
    if !binding.immutable
        || crate::indexes::binding_written(file, name.span)
        || crate::value_identity::binding_has_write(file, entities, source)
        || !containing_ast_function(&file.ast, binding.declaration)
            .is_some_and(|owner| owner.span == function.span)
    {
        return None;
    }
    let initializer = file.ast.peel_ts_sugar_span(binding.initializer?);
    let created = file.ast.call_at(initializer)?;
    if created.construct
        || !file.ast.straight_line_calls.contains(&created.span)
        || returns
            .iter()
            .any(|returned| created.span.end > returned.span.start)
    {
        return None;
    }
    let primitive = known_primitive(&crate::call_primitive_name_cached(
        lookup,
        file,
        created,
        entities,
        symbol_names,
        lookup.dialect,
    ))?;
    if !lookup.dialect.creates_reactive_source(primitive) {
        return None;
    }
    let context = EffectiveReturnContext {
        tuple_iteration_veto,
        file,
        ast_index: lookup.ast_file_index(file.path.as_str())?,
        entities,
        symbol_names,
        dialect: lookup.dialect,
    };
    let returned = effective_value_return(*roots.first()?, &context, 16)?;
    if !matches!(returned.kind.as_str(), "accessor" | "store-path")
        || (!proxy_targets.is_empty() && returned.kind != "store-path")
    {
        return None;
    }
    // No alias or escape of the root: another call could receive it through
    // an alias not represented by this proof. Only exact returns, member
    // receivers, and direct accessor invocations use the root here.
    for reference in file.ast.identifiers.iter().filter(|id| {
        id.role == solid_facts::ast::IdentifierRole::Reference
            && (file.ast.reference_declaration(id.span) == Some(name.span)
                || entities.at(file.path.as_str(), id.span) == Some(source))
    }) {
        let at = reference.span;
        if proxy_targets.contains(&at) {
            continue;
        }
        if (file.ast.calls.iter().any(|call| {
            call.arguments
                .iter()
                .any(|argument| argument.span.contains(at))
        }) && member_chain_escapes(file, at))
            || !(returns.iter().any(|returned| {
                returned
                    .argument
                    .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == at)
            }) || file
                .ast
                .members
                .iter()
                .any(|member| file.ast.peel_ts_sugar_span(member.object) == at)
                || (returned.kind == "accessor"
                    && file
                        .ast
                        .calls
                        .iter()
                        .any(|call| file.ast.peel_ts_sugar_span(call.callee) == at)))
        {
            return None;
        }
    }
    (solid_facts::ast::completion_return_cover(
        std::path::Path::new(file.path.as_str()),
        &file.source,
        function.body,
        &sites,
    ) == Some(true))
    .then_some((file, created, primitive))
}

/// ADR 0281: whether a reference to a returned source root written inside a
/// call argument can hand the root, or a sub-proxy of it, to other code.
/// Only a member chain rooted at the reference and consumed where it is
/// written (a statement, a declaration initializer, an operand inside a
/// callback the argument holds) is not an escape. The bare root, a chain that
/// is itself a call argument or a callee (which receives the proxy as
/// `this`), and a chain a function returns all are.
pub(super) fn member_chain_escapes(
    file: &solid_facts::FileFacts,
    at: solid_facts::core::Span,
) -> bool {
    let ast = &file.ast;
    let mut chain = at;
    while let Some(member) = ast
        .members
        .iter()
        .find(|member| ast.peel_ts_sugar_span(member.object) == chain)
    {
        chain = member.span;
    }
    chain == at
        || ast.calls.iter().any(|call| {
            ast.peel_ts_sugar_span(call.callee) == chain
                || call
                    .arguments
                    .iter()
                    .any(|argument| ast.peel_ts_sugar_span(argument.span) == chain)
        })
        || ast
            .returns
            .iter()
            .chain(
                ast.functions
                    .iter()
                    .filter_map(|function| function.expression_return.as_ref()),
            )
            .any(|returned| {
                returned
                    .argument
                    .is_some_and(|value| ast.peel_ts_sugar_span(value) == chain)
            })
}

/// Follow an exact immutable identity into a proved source-producing call.
/// Nested/default/rest binding slots carry no direct-value identity fact.
pub(super) fn effective_value_return(
    span: solid_facts::core::Span,
    context: &EffectiveReturnContext<'_>,
    depth: usize,
) -> Option<ReturnSummary> {
    if depth == 0 {
        return None;
    }
    let span = context.file.ast.peel_ts_sugar_span(span);
    if let Some(call) = context.ast_index.call_by_span(span) {
        return effective_inner_call_return(call, context);
    }
    if !context
        .file
        .ast
        .identifiers
        .iter()
        .any(|id| id.span == span)
    {
        return None;
    }
    let symbol = context.entities.at(context.file.path.as_str(), span)?;
    if crate::value_identity::binding_has_write(context.file, context.entities, symbol) {
        return None;
    }
    let binding = context.file.ast.bindings.iter().find(|binding| {
        binding
            .names
            .iter()
            .any(|name| context.entities.at(context.file.path.as_str(), name.span) == Some(symbol))
    })?;
    let initializer = binding.initializer?;
    let returned = effective_value_return(initializer, context, depth - 1)?;
    match binding.shape {
        solid_facts::ast::BindingShape::Identifier => {
            if matches!(returned.kind.as_str(), "tuple" | "object")
                && !crate::value_identity::structural_binding_is_stable(
                    context.file,
                    context.entities,
                    symbol,
                    &returned,
                    context.tuple_iteration_veto,
                )
            {
                return None;
            }
            Some(returned)
        }
        solid_facts::ast::BindingShape::Array
            if returned.kind == "tuple" && !context.tuple_iteration_veto =>
        {
            let index = binding.array_slots.iter().position(|slot| {
                slot.as_ref().is_some_and(|name| {
                    context.entities.at(context.file.path.as_str(), name.span) == Some(symbol)
                })
            })?;
            returned.elements.get(index)?.clone()
        }
        solid_facts::ast::BindingShape::Object if returned.kind == "object" => {
            let slot = binding.object_slots.iter().find(|slot| {
                context
                    .entities
                    .at(context.file.path.as_str(), slot.local.span)
                    == Some(symbol)
            })?;
            returned.properties.get(slot.property.as_str()).cloned()
        }
        _ => None,
    }
}

pub(super) fn effective_inner_call_return(
    inner: &solid_facts::ast::CallFact,
    context: &EffectiveReturnContext<'_>,
) -> Option<ReturnSummary> {
    let primitive = crate::call_primitive_name(
        context.file,
        inner,
        context.entities,
        context.symbol_names,
        context.dialect,
    );
    let primitive = known_primitive(&primitive)?;
    let kind = if context.dialect.returns_store(primitive) {
        "store-path"
    } else {
        "accessor"
    };
    // The dialect's own tuple list. This was a hardcoded
    // `CreateSignal | CreateStore | CreateResource`, which is neither
    // dialect's: `createResource` does not exist in 2.0, and 2.0's
    // `createOptimistic` and `createOptimisticStore` -- both declared to
    // return two-slot tuples -- were missing, so a read traced through either
    // was told the call returns the store itself.
    if context.dialect.returns_reactive_tuple(primitive) {
        return Some(ReturnSummary {
            kind: "tuple".into(),
            elements: vec![
                Some(ReturnSummary {
                    kind: kind.into(),
                    label: "wrapped reactive value".into(),
                    ..ReturnSummary::default()
                }),
                None,
            ],
            ..ReturnSummary::default()
        });
    }
    context
        .dialect
        .creates_reactive_source(primitive)
        .then(|| ReturnSummary {
            kind: kind.into(),
            label: "wrapped reactive value".into(),
            ..ReturnSummary::default()
        })
}
