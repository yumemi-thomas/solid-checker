//! The source-discovery stage: finds reactive sources, accessors,
//! setters, and contract-backed facts per file, with per-file reuse.

use crate::cache::{
    CachedSourceDiscovery, CachedTypeScriptIndexes, SourceDiscoveryContribution,
    SourceDiscoveryIdentity, SourceDiscoveryTypeScriptDelta,
};
use crate::owners::{
    binding_returns_reactive_source, computation_is_async_with_contracts, containing_ast_function,
};
use crate::pipeline::{parallel_file_chunk_results, parallel_file_results, parallel_slice_results};
use crate::{
    BuildTimings, ContractCallback, ContractReturn, PrimitiveName, ReactiveSourceKind,
    RuntimeEnvironment, RuntimeRendering, call_primitive_name, jsx_primitive_name, known_primitive,
    location,
};

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::contracts::ResolvedContracts;
use crate::identity::{SymbolId, symbol_id};
use crate::indexes::{CachedAstFileIndex, EntitySymbols, ProjectIndexes, SemanticLookup};
use crate::timings::{ReactiveIrStage, StageClock};
use solid_dialect::{Dialect, Primitive};
use solid_facts::core::{SourceHash, SourcePath};
use solid_facts::{FileFacts, ProjectFacts};
use typefacts::{Declaration, Location, ResolvedCallValidity};

pub(crate) fn source_discovery_identity(
    file: &FileFacts,
    indexes: &ProjectIndexes<'_>,
) -> SourceDiscoveryIdentity {
    let mut symbol_ids = HashSet::<SymbolId>::new();
    for entity in indexes.entities_for_path(file.path.as_str()) {
        if !entity.symbol.is_empty() {
            symbol_ids.insert(symbol_id(entity.symbol.as_ref()));
        }
        if let Some(call) = &entity.resolved_call
            && call.validity == ResolvedCallValidity::Valid
            && !call.target.is_empty()
        {
            symbol_ids.insert(symbol_id(call.target.as_ref()));
        }
    }
    let mut pending = symbol_ids.iter().cloned().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        let Some(symbol) = indexes.symbols_by_id.get(id.as_str()) else {
            continue;
        };
        if !symbol.alias_target().is_empty() && symbol_ids.insert(symbol_id(symbol.alias_target()))
        {
            pending.push(symbol_id(symbol.alias_target()));
        }
    }
    let mut symbols = symbol_ids.into_iter().collect::<Vec<_>>();
    symbols.sort_unstable();
    SourceDiscoveryIdentity {
        source_hash: file.source_hash.clone(),
        symbols,
    }
}

pub(crate) fn source_discovery_identity_matches(
    cached: &SourceDiscoveryIdentity,
    path: &str,
    source_hash: &SourceHash,
    typescript_unchanged: bool,
    typescript_delta: Option<&SourceDiscoveryTypeScriptDelta>,
) -> bool {
    if &cached.source_hash != source_hash {
        return false;
    }
    if typescript_unchanged {
        return true;
    }
    if let Some(delta) = typescript_delta {
        if delta.entity_paths.contains(path) || delta.file_paths.contains(path) {
            return false;
        }
        if delta.semantic_symbol_ids.is_empty() {
            return true;
        }
        return cached
            .symbols
            .iter()
            .all(|symbol| !delta.semantic_symbol_ids.contains(symbol.as_str()));
    }
    false
}

fn push_contracted_return_source(
    result: &mut SourceDiscoveryContribution,
    symbol: &SymbolId,
    display: SymbolId,
    returned: &ContractReturn,
    export_name: &str,
    contract_location: &Location,
) {
    if !matches!(returned.kind.as_str(), "accessor" | "store-path") {
        return;
    }
    result
        .accessors
        .push((symbol.clone(), (display, contract_location.clone())));
    result.contracted_accessor_symbols.push(symbol.clone());
    result.accessor_origins.push((
        symbol.clone(),
        (
            symbol_id(&returned.label),
            symbol_id(export_name),
            contract_location.clone(),
        ),
    ));
    result.source_kinds.push((
        symbol.clone(),
        if returned.kind == "store-path" {
            ReactiveSourceKind::Store
        } else {
            ReactiveSourceKind::Accessor
        },
    ));
}

/// Rebind the exact callback value, never the callback's parameter or its
/// function identity. This produces the same local read evidence as a direct
/// factory binding and leaves the caller's execution role untouched.
fn push_callback_return_sources(
    result: &mut SourceDiscoveryContribution,
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    entities: &EntitySymbols,
    resolved_contracts: &ResolvedContracts,
    returned: crate::callback_return::CallbackReturn<'_>,
) {
    for (name, value) in returned.bindings {
        let Some(symbol) = entities.at(file.path.as_str(), name) else {
            continue;
        };
        let declaration = crate::location(file.path.shared(), name);
        let display = symbol_id(file.source_text(name).unwrap_or_default());
        if value.kind == "accessor" {
            if let Some((export, origin)) = returned.origin {
                push_contracted_return_source(result, symbol, display, &value, export, origin);
            } else {
                result
                    .accessors
                    .push((symbol.clone(), (display, declaration)));
                result
                    .source_kinds
                    .push((symbol.clone(), ReactiveSourceKind::Accessor));
            }
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
                && returned.created.arguments.first().is_some_and(|argument| {
                    computation_is_async_with_contracts(
                        lookup,
                        file,
                        argument.span,
                        &resolved_contracts.by_symbol,
                    )
                })
            {
                result.async_sources.push(symbol.clone());
            }
        }
    }
}

struct EffectiveReturnContext<'a> {
    file: &'a FileFacts,
    ast_index: &'a CachedAstFileIndex,
    entities: &'a EntitySymbols,
    symbol_names: &'a HashMap<SymbolId, SymbolId>,
    resolved_contracts: &'a ResolvedContracts,
    dialect: &'a dyn Dialect,
}

fn effective_call_return(
    returned: &ContractReturn,
    call: &solid_facts::ast::CallFact,
    context: &EffectiveReturnContext<'_>,
    depth: usize,
) -> Option<ContractReturn> {
    if depth == 0 {
        return None;
    }
    if returned.kind == "tuple" {
        return Some(ContractReturn {
            elements: returned
                .elements
                .iter()
                .map(|element| {
                    element.as_ref().and_then(|element| {
                        effective_call_return(element, call, context, depth - 1)
                    })
                })
                .collect(),
            prototype: None,
            ..returned.clone()
        });
    }
    if returned.kind == "object" {
        return Some(ContractReturn {
            properties: returned
                .properties
                .iter()
                .filter_map(|(name, value)| {
                    effective_call_return(value, call, context, depth - 1)
                        .map(|value| (name.clone(), value))
                })
                .collect(),
            prototype: None,
            ..returned.clone()
        });
    }
    if !matches!(returned.kind.as_str(), "argument" | "callback-result") {
        return Some(returned.clone());
    }
    let argument = call.arguments.get(returned.parameter?)?;
    if returned.kind == "callback-result" {
        let function_span = if context
            .file
            .ast
            .functions
            .iter()
            .any(|function| function.span == argument.span)
        {
            argument.span
        } else {
            let symbol = context
                .entities
                .at(context.file.path.as_str(), argument.span)?;
            context.file.ast.bindings.iter().find_map(|binding| {
                binding
                    .names
                    .iter()
                    .any(|name| {
                        context.entities.at(context.file.path.as_str(), name.span) == Some(symbol)
                    })
                    .then_some(binding.initializer)
                    .flatten()
            })?
        };
        let function = context
            .file
            .ast
            .functions
            .iter()
            .find(|function| function.span == function_span)?;
        let returns =
            function
                .expression_return
                .iter()
                .chain(context.file.ast.returns.iter().filter(|candidate| {
                    containing_ast_function(&context.file.ast, candidate.span)
                        .is_some_and(|owner| owner.span == function.span)
                }));
        let mut resolved = None::<ContractReturn>;
        for returned_value in returns {
            let value = returned_value.argument?;
            let inner = context.ast_index.call_by_span(value).or_else(|| {
                context
                    .file
                    .ast
                    .calls
                    .iter()
                    .filter(|candidate| value.contains(candidate.span))
                    .max_by_key(|candidate| candidate.span.end - candidate.span.start)
            })?;
            let candidate = effective_inner_call_return(inner, context, depth - 1)?;
            if resolved.as_ref().is_some_and(|prior| prior != &candidate) {
                return None;
            }
            resolved = Some(candidate);
        }
        return resolved;
    }
    effective_value_return(argument.span, context, depth - 1)
}

/// A fresh source returned by one exact project function. This is deliberately
/// separate from structured-return summaries: finding one reactive leaf, or
/// merging equal kinds, cannot prove that every completion returns one value.
fn project_returned_source<'a>(
    lookup: &SemanticLookup<'a>,
    caller: &FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    resolved_contracts: &ResolvedContracts,
) -> Option<(&'a FileFacts, &'a solid_facts::ast::CallFact, Primitive)> {
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
    for returned in &returns {
        let value = file.ast.peel_ts_sugar_span(returned.argument?);
        if !file.ast.identifiers.iter().any(|id| id.span == value) {
            return None;
        }
        let symbol = entities.at(file.path.as_str(), value)?;
        if source.is_some_and(|prior| prior != symbol) {
            return None;
        }
        source = Some(symbol);
        sites.push(value);
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
    let primitive = known_primitive(&call_primitive_name(
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
        file,
        ast_index: lookup.ast_file_index(file.path.as_str())?,
        entities,
        symbol_names,
        resolved_contracts,
        dialect: lookup.dialect,
    };
    let returned = effective_value_return(returns.first()?.argument?, &context, 16)?;
    if !matches!(returned.kind.as_str(), "accessor" | "store-path") {
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
        if file.ast.calls.iter().any(|call| {
            call.arguments
                .iter()
                .any(|argument| argument.span.contains(at))
        }) || !(returns.iter().any(|returned| {
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

/// Follow an exact immutable identity into a proved source-producing call.
/// Nested/default/rest binding slots carry no direct-value identity fact.
fn effective_value_return(
    span: solid_facts::core::Span,
    context: &EffectiveReturnContext<'_>,
    depth: usize,
) -> Option<ContractReturn> {
    if depth == 0 {
        return None;
    }
    let span = context.file.ast.peel_ts_sugar_span(span);
    if let Some(call) = context.ast_index.call_by_span(span) {
        return effective_inner_call_return(call, context, depth - 1);
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
                )
            {
                return None;
            }
            Some(returned)
        }
        solid_facts::ast::BindingShape::Array if returned.kind == "tuple" => {
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

/// A declared member type cannot restore a container identity invalidated by
/// runtime writes/escape. Contracted containers use their explicit value flow
/// instead of the generic typed-accessor fallback, including direct aliases.
fn is_contracted_container_value(
    file: &FileFacts,
    entities: &EntitySymbols,
    contracts: &ResolvedContracts,
    span: solid_facts::core::Span,
    depth: usize,
) -> bool {
    if depth == 0 {
        return true;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if let Some(call) = file.ast.calls.iter().find(|call| call.span == span) {
        return entities
            .at(file.path.as_str(), call.callee)
            .and_then(|symbol| contracts.by_symbol.get(symbol))
            .and_then(|contract| contract.summary.returns.known())
            .and_then(Option::as_ref)
            .is_some_and(|value| {
                matches!(
                    value.kind.as_str(),
                    "tuple" | "object" | crate::contracts::RETURNED_CALLABLE
                )
            });
    }
    let Some(symbol) = entities.at(file.path.as_str(), span) else {
        return false;
    };
    file.ast
        .bindings
        .iter()
        .find(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.names.len() == 1
                && entities.at(file.path.as_str(), binding.names[0].span) == Some(symbol)
        })
        .and_then(|binding| binding.initializer)
        .is_some_and(|initializer| {
            is_contracted_container_value(file, entities, contracts, initializer, depth - 1)
        })
}

fn effective_inner_call_return(
    inner: &solid_facts::ast::CallFact,
    context: &EffectiveReturnContext<'_>,
    depth: usize,
) -> Option<ContractReturn> {
    if let Some(contracted) = context
        .entities
        .at(context.file.path.as_str(), inner.callee)
        .and_then(|symbol| context.resolved_contracts.by_symbol.get(symbol))
        .and_then(|contracted| contracted.summary.returns.known())
        .and_then(Option::as_ref)
    {
        return effective_call_return(contracted, inner, context, depth);
    }
    let primitive = call_primitive_name(
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
        return Some(ContractReturn {
            kind: "tuple".into(),
            elements: vec![
                Some(ContractReturn {
                    kind: kind.into(),
                    label: "wrapped reactive value".into(),
                    prototype: None,
                    ..ContractReturn::default()
                }),
                None,
            ],
            prototype: None,
            ..ContractReturn::default()
        });
    }
    context
        .dialect
        .creates_reactive_source(primitive)
        .then(|| ContractReturn {
            kind: kind.into(),
            label: "wrapped reactive value".into(),
            prototype: None,
            ..ContractReturn::default()
        })
}

/// Resolve a call through one exact local binding whose initializer is a
/// contracted callback-result function factory. The relation's parameter is
/// scoped to the outer contracted call, not to the returned function's own
/// arguments. Aliases, assignments, cross-file values, and unresolved
/// callbacks deliberately yield no fact.
fn effective_returned_callable_call(
    call: &solid_facts::ast::CallFact,
    context: &EffectiveReturnContext<'_>,
    depth: usize,
) -> Option<(ContractReturn, String, Location)> {
    if depth == 0 {
        return None;
    }
    let callee = context
        .entities
        .at(context.file.path.as_str(), call.callee)?;
    let initializer = context.file.ast.bindings.iter().find_map(|binding| {
        binding
            .names
            .iter()
            .any(|name| context.entities.at(context.file.path.as_str(), name.span) == Some(callee))
            .then_some(binding.call_initializer)
            .flatten()
    })?;
    let factory_call = context.ast_index.call_by_span(initializer)?;
    let contracted = context
        .entities
        .at(context.file.path.as_str(), factory_call.callee)
        .and_then(|symbol| context.resolved_contracts.by_symbol.get(symbol))?;
    let returned = contracted
        .summary
        .returns
        .known()
        .and_then(Option::as_ref)?;
    if returned.kind != "callback-result-function" {
        return None;
    }
    let relation = ContractReturn {
        kind: "callback-result".into(),
        parameter: returned.parameter,
        prototype: None,
        ..ContractReturn::default()
    };
    effective_call_return(&relation, factory_call, context, depth - 1).map(|returned| {
        (
            returned,
            contracted.local_name.clone(),
            contracted.contract_location.clone(),
        )
    })
}

/// The statically-proven rc.0 async/hydration options declared on one
/// reactive source. All flags default to `false` when there is no options
/// argument (or an explicit `undefined`/`null`), which keeps that case on the
/// fully-proven path.
///
/// Runtime ground truth, probed against `solid-js@2.0.0-rc.0` /
/// `@solidjs/signals@2.0.0-rc.0` (2026-08-15):
///
/// - A computation created with `loadingValue` (or a store-family source with
///   `seedLoadingValue: true`) is born committed. During its first flight,
///   untracked strict reads return the declared value (no
///   `PENDING_ASYNC_UNTRACKED_READ`), tracked reads serve it without
///   suspending (no `Loading` boundary participation, no
///   `ASYNC_OUTSIDE_LOADING_BOUNDARY`), reads inside `createTrackedEffect`
///   neither warn nor throw, and `isPending` reads `false`.
/// - The declared window ends at the first real answer: with a re-ask in
///   flight (input change or refresh), an untracked strict read throws
///   `PENDING_ASYNC_UNTRACKED_READ` and a `createTrackedEffect` read warns
///   `PENDING_ASYNC_FORBIDDEN_SCOPE` and throws — exactly like an undeclared
///   async node. SC5001/SC5002 therefore stay reported on declared sources,
///   with conditional wording.
/// - A bare `ssrSource: "client"` source (no declaration) never runs its
///   compute on the server; *any* read of it during SSR outside a `Loading`
///   fallback flush throws `ssrSource: "client" read during SSR outside a
///   <Loading> boundary` — including reads of a fully synchronous compute —
///   while under a boundary it suspends finally so the fallback is flushed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub(crate) struct AsyncSourceOptions {
    /// The source provably declares `loadingValue` (presence-keyed at
    /// runtime: `"loadingValue" in options`) or `seedLoadingValue: true` in
    /// an exact object literal.
    pub(crate) declared_loading: bool,
    /// The source provably declares `ssrSource: "client"` with no
    /// `loadingValue`/`seedLoadingValue` declaration, on a function-form
    /// call: the server installs a client hole for it.
    pub(crate) ssr_client_bare: bool,
    /// The source is a proven bare `ssrSource: "client"` source, but the
    /// analyzed project does not contain enough evidence to decide whether a
    /// server-rendering entry exists. Absence of an import is not proof that
    /// the application is CSR-only: the server entry may live in another
    /// tsconfig or package.
    pub(crate) server_rendering_unresolved: bool,
    /// An options argument exists that the analyzer cannot read as an exact
    /// object literal, so option-dependent claims cannot be proven either
    /// way.
    pub(crate) opaque: bool,
}

/// Reads the async/hydration option surface of one source-creating call, as
/// far as the options argument is statically readable. Value claims
/// (`seedLoadingValue: true`, `ssrSource: "client"` with nothing declared)
/// require an exact object literal; the presence claim for `loadingValue`
/// survives spreads because a later spread cannot remove the key from the
/// runtime's `in` check.
pub(crate) fn async_source_options(
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Option<Primitive>,
    dialect: &dyn Dialect,
) -> AsyncSourceOptions {
    use solid_facts::ast::ArgumentValueKind;
    let Some(index) = primitive.and_then(|primitive| dialect.options_argument(primitive)) else {
        return AsyncSourceOptions::default();
    };
    let Some(argument) = call.arguments.get(index) else {
        return AsyncSourceOptions::default();
    };
    if matches!(
        argument.value,
        ArgumentValueKind::Undefined | ArgumentValueKind::Null
    ) {
        return AsyncSourceOptions::default();
    }
    let named = |span, expected: &str| file.source_text(span) == Some(expected);
    let loading_key = argument
        .property_names
        .iter()
        .any(|key| named(*key, "loadingValue"));
    let seed_key = argument
        .property_names
        .iter()
        .any(|key| named(*key, "seedLoadingValue"));
    let seed_true = argument
        .boolean_properties
        .iter()
        .any(|property| named(property.name, "seedLoadingValue") && property.value);
    let seed_false = argument
        .boolean_properties
        .iter()
        .any(|property| named(property.name, "seedLoadingValue") && !property.value);
    let ssr_client = argument
        .string_properties
        .iter()
        .any(|property| named(property.name, "ssrSource") && property.value == "client");
    let declared_loading = loading_key || (argument.exact_object_literal && seed_true);
    // The server installs the client hole only for function-form sources (a
    // value-form `createSignal(0, …)` never runs a compute, so `ssrSource`
    // is inert there); an unresolvable computation argument fails the proof.
    let function_form = matches!(
        call.arguments.first().map(|argument| argument.value),
        Some(ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction)
    );
    let ssr_client_bare = argument.exact_object_literal
        && ssr_client
        && !loading_key
        && (!seed_key || seed_false)
        && function_form;
    AsyncSourceOptions {
        declared_loading,
        ssr_client_bare,
        server_rendering_unresolved: false,
        opaque: !argument.exact_object_literal && !declared_loading,
    }
}

/// The exports whose import proves the project server-renders (or hydrates
/// server-rendered HTML). A bare `ssrSource: "client"` source is only a
/// runtime error on the server path. Named imports prove that path; their
/// absence leaves the rendering mode unresolved because the server entry may
/// live outside the analyzed project. The export names come from the
/// historical `@solidjs/web` audit; no contract is read during analysis.
///
/// **Not asked of the dialect, and it could not answer.** Five of these six
/// are not primitives — only `hydrate` is in the vocabulary — so
/// `export_modules` returns nothing for them and a seam question keyed on the
/// name would silence the rule rather than generalize it. Which exports prove
/// server rendering is this rule's own subject; which *module* they may come
/// from is the dialect's, and that half is asked below.
const SERVER_RENDER_IMPORTS: [&str; 6] = [
    "renderToStream",
    "renderToString",
    "renderToFrameStream",
    "renderServerComponent",
    "handleServerFunctionRequest",
    "hydrate",
];

/// Whether the analyzed application server-renders.
///
/// The three outcomes are not two: a project with no visible server entry is
/// not the same fact as a project the user has explicitly selected a
/// client-only rendering mode for. The first is an absence of evidence -- the
/// server entry may live in another tsconfig or package -- and rules must
/// report a proof obligation. The second is evidence, and a rule whose whole
/// premise is "if this application server-renders" has had that premise
/// disproven and must stay silent rather than report an obligation the user
/// has already discharged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServerRenderingPremise {
    /// An explicit rendering selector, or a visible server-rendering entry
    /// import, proves the application server-renders.
    Renders,
    /// An explicit rendering selector proves it does not.
    ProvenClientOnly,
    /// No selector, and no server-rendering entry point is visible in the
    /// analyzed project -- which does not prove the application is CSR-only.
    Unresolved,
}

impl ServerRenderingPremise {
    /// The premise from the two inputs that decide it: the explicit rendering
    /// selector when the user set one, and otherwise whether a
    /// server-rendering entry point is visible in the analyzed project.
    ///
    /// A selector answers the question outright in both directions. Only
    /// without one does the import survey matter, and then only one way: a
    /// visible entry proves server rendering, while no visible entry proves
    /// nothing at all.
    pub(crate) const fn select(
        rendering: Option<RuntimeRendering>,
        imports_server_entry: bool,
    ) -> Self {
        match rendering {
            Some(RuntimeRendering::StringSsr | RuntimeRendering::StreamingSsr) => Self::Renders,
            Some(RuntimeRendering::Csr) => Self::ProvenClientOnly,
            None if imports_server_entry => Self::Renders,
            None => Self::Unresolved,
        }
    }

    /// Whether server rendering is proven to happen. `ProvenClientOnly` and
    /// `Unresolved` are both "not proven", and callers that need to tell them
    /// apart must match on the enum instead.
    pub(crate) const fn renders(self) -> bool {
        matches!(self, Self::Renders)
    }
}

/// Whether the analyzed project server-renders: an explicit rendering
/// selector when there is one, otherwise whether any analyzed file imports a
/// server rendering entry point from a module this dialect owns.
pub(crate) fn project_server_rendering(
    facts: &ProjectFacts,
    environment: &RuntimeEnvironment,
    dialect: &dyn solid_dialect::Dialect,
) -> ServerRenderingPremise {
    if let Some(rendering) = environment.rendering {
        return ServerRenderingPremise::select(Some(rendering), false);
    }
    let imports_server_entry = facts.files.iter().any(|file| {
        file.ast.imports.iter().any(|import| {
            // `@solidjs/web` and its subpaths, spelled by the dialect rather
            // than by this module. `modules()` already enumerates them, so a
            // dialect that publishes its render entries from somewhere else
            // says so once instead of being matched against a literal here.
            dialect.owns_module(import.module.as_str())
                && !import.type_only
                && import.bindings.iter().any(|binding| {
                    !binding.type_only
                        && binding
                            .imported
                            .as_deref()
                            .is_some_and(|imported| SERVER_RENDER_IMPORTS.contains(&imported))
                })
        })
    });
    ServerRenderingPremise::select(None, imports_server_entry)
}

#[cfg(test)]
mod server_rendering_premise_tests {
    use super::ServerRenderingPremise;
    use crate::RuntimeRendering;

    /// The three states must stay three. Folding `ProvenClientOnly` into
    /// `Unresolved` is what made an explicitly CSR project report an
    /// uncertifiable result whose own message said the premise could not be
    /// proven, and folding it into `Renders` would invent an SSR violation
    /// for an application that has no server.
    #[test]
    fn an_explicit_rendering_selector_decides_the_premise_in_both_directions() {
        for rendering in [RuntimeRendering::StringSsr, RuntimeRendering::StreamingSsr] {
            for imports in [false, true] {
                let premise = ServerRenderingPremise::select(Some(rendering), imports);
                assert_eq!(premise, ServerRenderingPremise::Renders);
                assert!(premise.renders());
            }
        }
        // The selector outranks the survey in the other direction too: an
        // unused `renderToStream` import in a project the user has declared
        // client-only does not resurrect the server.
        for imports in [false, true] {
            let premise = ServerRenderingPremise::select(Some(RuntimeRendering::Csr), imports);
            assert_eq!(premise, ServerRenderingPremise::ProvenClientOnly);
            assert!(!premise.renders());
        }
    }

    /// Without a selector the survey decides one way only. A visible server
    /// entry proves server rendering; no visible entry is an absence of
    /// evidence, not evidence of absence, because the entry may live in
    /// another tsconfig or package.
    #[test]
    fn without_a_selector_a_visible_entry_proves_and_its_absence_does_not() {
        assert_eq!(
            ServerRenderingPremise::select(None, true),
            ServerRenderingPremise::Renders
        );
        let unresolved = ServerRenderingPremise::select(None, false);
        assert_eq!(unresolved, ServerRenderingPremise::Unresolved);
        assert!(!unresolved.renders());
    }
}

/// Whether a store-family creation is provably the value form
/// (`createStore(value)` / `createOptimisticStore(value)`), which never
/// builds a compute node. Runtime ground truth (probed, rc.0): `refresh()`
/// on such a store — or on any of its child records — throws
/// `INVALID_REFRESH_TARGET` in dev, while the function forms, projections,
/// and function-form optimistic stores all accept it. The runtime branches
/// on `typeof first === "function"`, so the proof is that argument 0 is a
/// non-callable value: a container or primitive literal, `null`, or
/// `undefined`. An identifier or other expression could still be a derive
/// function, so it stays unknown and refresh acceptance is preserved.
fn store_is_value_form(call: &solid_facts::ast::CallFact, primitive: Option<Primitive>) -> bool {
    use solid_facts::ast::ArgumentValueKind;
    if !matches!(
        primitive,
        Some(Primitive::CreateStore | Primitive::CreateOptimisticStore)
    ) {
        return false;
    }
    call.arguments.first().is_some_and(|argument| {
        matches!(
            argument.value,
            ArgumentValueKind::Null | ArgumentValueKind::Undefined
        ) || argument.runtime_value_kind.is_data_literal()
    })
}

pub(crate) fn discover_file_sources(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    ast_index: &CachedAstFileIndex,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    resolved_contracts: &ResolvedContracts,
) -> SourceDiscoveryContribution {
    let mut result = SourceDiscoveryContribution::default();
    for binding in &file.ast.bindings {
        let Some(initializer) = binding.call_initializer else {
            continue;
        };
        let Some(call) = ast_index.call_by_span(initializer) else {
            continue;
        };
        if crate::callback_return::callback_slot(lookup, file, call).is_some() {
            if let Some(returned) = crate::callback_return::resolve(lookup, file, call) {
                push_callback_return_sources(
                    &mut result,
                    lookup,
                    file,
                    entities,
                    resolved_contracts,
                    returned,
                );
            }
            continue;
        }
        let contracted = entities
            .get(&location(file.path.shared(), call.callee))
            .and_then(|symbol| resolved_contracts.by_symbol.get(symbol));
        if let Some(contracted) = contracted
            && let Some(contracted_return) =
                contracted.summary.returns.known().and_then(Option::as_ref)
        {
            let context = EffectiveReturnContext {
                file,
                ast_index,
                entities,
                symbol_names,
                resolved_contracts,
                dialect: lookup.dialect,
            };
            let effective_return = effective_call_return(contracted_return, call, &context, 16);
            let Some(contracted_return) = effective_return.as_ref() else {
                continue;
            };
            match contracted_return.kind.as_str() {
                "accessor" | "store-path" => {
                    if let Some(name) = binding.names.first() {
                        let declaration = location(file.path.shared(), name.span);
                        if let Some(symbol) = entities.get(&declaration) {
                            push_contracted_return_source(
                                &mut result,
                                symbol,
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                contracted_return,
                                &contracted.local_name,
                                &contracted.contract_location,
                            );
                        }
                    }
                }
                "tuple" if binding.shape == solid_facts::ast::BindingShape::Array => {
                    for (name, returned) in binding
                        .array_slots
                        .iter()
                        .zip(&contracted_return.elements)
                        .filter_map(|(name, returned)| name.as_ref().zip(returned.as_ref()))
                    {
                        let declaration = location(file.path.shared(), name.span);
                        if let Some(symbol) = entities.get(&declaration) {
                            push_contracted_return_source(
                                &mut result,
                                symbol,
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                returned,
                                &contracted.local_name,
                                &contracted.contract_location,
                            );
                        }
                    }
                }
                "object" if binding.shape == solid_facts::ast::BindingShape::Object => {
                    for slot in &binding.object_slots {
                        let Some(returned) =
                            contracted_return.properties.get(slot.property.as_str())
                        else {
                            continue;
                        };
                        let declaration = location(file.path.shared(), slot.local.span);
                        if let Some(symbol) = entities.get(&declaration) {
                            push_contracted_return_source(
                                &mut result,
                                symbol,
                                symbol_id(file.source_text(slot.local.span).unwrap_or_default()),
                                returned,
                                &contracted.local_name,
                                &contracted.contract_location,
                            );
                        }
                    }
                }
                "object" => {
                    let root = binding
                        .names
                        .first()
                        .and_then(|name| entities.at(file.path.as_str(), name.span));
                    if let Some(root_symbol) = root {
                        if !crate::value_identity::structural_binding_is_stable(
                            file,
                            entities,
                            root_symbol,
                            contracted_return,
                        ) {
                            continue;
                        }
                        for member in &file.ast.members {
                            // Exact receiver identity only. A same-spelled
                            // member elsewhere in the file -- a shadowing
                            // local, an unrelated object -- is not this
                            // contracted root, and registering its property
                            // as a reactive source would invent a source the
                            // contract never described.
                            let same_root = entities
                                .at(file.path.as_str(), member.object)
                                .is_some_and(|symbol| symbol == root_symbol);
                            if !same_root {
                                continue;
                            }
                            let property = file.source_text(member.property).unwrap_or_default();
                            let Some(returned) = contracted_return.properties.get(property) else {
                                continue;
                            };
                            if let Some(symbol) = entities.at(file.path.as_str(), member.property) {
                                push_contracted_return_source(
                                    &mut result,
                                    symbol,
                                    symbol_id(property),
                                    returned,
                                    &contracted.local_name,
                                    &contracted.contract_location,
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        let context = EffectiveReturnContext {
            file,
            ast_index,
            entities,
            symbol_names,
            resolved_contracts,
            dialect: lookup.dialect,
        };
        if let Some((returned, export_name, contract_location)) =
            effective_returned_callable_call(call, &context, 16)
            && matches!(returned.kind.as_str(), "accessor" | "store-path")
            && let Some(name) = binding.names.first()
            && let Some(symbol) = entities.at(file.path.as_str(), name.span)
        {
            push_contracted_return_source(
                &mut result,
                symbol,
                symbol_id(file.source_text(name.span).unwrap_or_default()),
                &returned,
                &export_name,
                &contract_location,
            );
            continue;
        }
        let primitive = call_primitive_name(file, call, entities, symbol_names, lookup.dialect);
        let resolved = known_primitive(&primitive);
        if resolved.is_none()
            && binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.len() == 1
            && binding
                .initializer
                .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == call.span)
            && let Some(name) = binding.names.first()
            && !crate::indexes::binding_written(file, name.span)
            && let Some(symbol) = entities.at(file.path.as_str(), name.span)
            && let Some((origin_file, created, primitive)) = project_returned_source(
                lookup,
                file,
                call,
                entities,
                symbol_names,
                resolved_contracts,
            )
        {
            // The caller binding is this call's source, never the callee's
            // declaration symbol. Separate calls keep separate identities.
            let declaration = location(file.path.shared(), name.span);
            result.accessors.push((
                symbol.clone(),
                (
                    symbol_id(file.source_text(name.span).unwrap_or_default()),
                    declaration,
                ),
            ));
            result.source_kinds.push((
                symbol.clone(),
                if lookup.dialect.returns_store(primitive) {
                    ReactiveSourceKind::Store
                } else {
                    ReactiveSourceKind::Accessor
                },
            ));
            result.source_phases.push((symbol.clone(), 1));
            if let Some(spelling) = lookup.dialect.name_of(primitive) {
                result
                    .source_primitives
                    .push((symbol.clone(), spelling.into()));
            }
            result
                .source_owned_write
                .push((symbol.clone(), created.owned_write_option));
            if store_is_value_form(created, Some(primitive)) {
                result.value_form_stores.push(symbol.clone());
            }
            let options =
                async_source_options(origin_file, created, Some(primitive), lookup.dialect);
            if options != AsyncSourceOptions::default() {
                result.source_async_options.push((symbol.clone(), options));
            }
            if created.arguments.first().is_some_and(|argument| {
                computation_is_async_with_contracts(
                    lookup,
                    origin_file,
                    argument.span,
                    &resolved_contracts.by_symbol,
                )
            }) {
                result.async_sources.push(symbol.clone());
            }
            continue;
        }
        if resolved == Some(Primitive::Action) {
            if let Some(name) = binding.names.first() {
                let location = location(file.path.shared(), name.span);
                if let Some(symbol) = entities.get(&location) {
                    result.actions.push((
                        symbol.clone(),
                        (
                            symbol_id(file.source_text(name.span).unwrap_or_default()),
                            location,
                        ),
                    ));
                }
            }
            continue;
        }
        if resolved == Some(Primitive::Dynamic) {
            if let Some(name) = binding.names.first() {
                let declaration = location(file.path.shared(), name.span);
                if let Some(symbol) = entities.get(&declaration) {
                    result
                        .source_primitives
                        .push((symbol.clone(), "dynamic".into()));
                    if call.arguments.first().is_some_and(|argument| {
                        computation_is_async_with_contracts(
                            lookup,
                            file,
                            argument.span,
                            &resolved_contracts.by_symbol,
                        )
                    }) {
                        result.async_sources.push(symbol.clone());
                    }
                }
            }
            continue;
        }
        if !matches!(
            resolved,
            Some(primitive) if lookup.dialect.creates_reactive_source(primitive)
        ) {
            continue;
        }
        let source_kind = if matches!(
            resolved,
            Some(primitive) if lookup.dialect.returns_store(primitive)
        ) {
            ReactiveSourceKind::Store
        } else {
            ReactiveSourceKind::Accessor
        };
        let source_name = if binding.shape == solid_facts::ast::BindingShape::Array {
            binding.array_slots.first().and_then(Option::as_ref)
        } else {
            binding.names.first()
        };
        if let Some(name) = source_name {
            let declaration = location(file.path.shared(), name.span);
            if let Some(symbol) = entities.get(&declaration) {
                result.accessors.push((
                    symbol.clone(),
                    (
                        symbol_id(file.source_text(name.span).unwrap_or_default()),
                        declaration,
                    ),
                ));
                // `Dialect::returns_reactive_tuple`, not a list: the list here
                // was 1.x's (`createResource` is 1.x-only) and so missed 2.0's
                // `createOptimistic` and `createOptimisticStore` entirely. That
                // method's own documentation names this as the thing it exists
                // to replace -- "shared code carried one hardcoded list that
                // was neither dialect's".
                let go_returned_source = binding.shape == solid_facts::ast::BindingShape::Array
                    && resolved
                        .is_some_and(|primitive| lookup.dialect.returns_reactive_tuple(primitive))
                    && binding_returns_reactive_source(binding, call);
                result.source_phases.push((
                    symbol.clone(),
                    if go_returned_source
                        && resolved.is_some_and(|primitive| lookup.dialect.returns_store(primitive))
                    {
                        2
                    } else if go_returned_source {
                        0
                    } else {
                        1
                    },
                ));
                if go_returned_source {
                    result.returned_source_symbols.push(symbol.clone());
                    result.summary_source_symbols.push(symbol.clone());
                }
                result.source_kinds.push((symbol.clone(), source_kind));
                if let Some(primitive) = primitive.as_deref() {
                    result
                        .source_primitives
                        .push((symbol.clone(), primitive.into()));
                }
                if store_is_value_form(call, resolved) {
                    result.value_form_stores.push(symbol.clone());
                }
                result
                    .source_owned_write
                    .push((symbol.clone(), call.owned_write_option));
                let options = async_source_options(file, call, resolved, lookup.dialect);
                if options != AsyncSourceOptions::default() {
                    result.source_async_options.push((symbol.clone(), options));
                }
                if call.arguments.first().is_some_and(|argument| {
                    computation_is_async_with_contracts(
                        lookup,
                        file,
                        argument.span,
                        &resolved_contracts.by_symbol,
                    )
                }) {
                    result.async_sources.push(symbol.clone());
                }
            }
        }
        if resolved != Some(Primitive::CreateMemo)
            && let Some(name) = if binding.shape == solid_facts::ast::BindingShape::Array {
                binding.array_slots.get(1).and_then(Option::as_ref)
            } else {
                binding.names.get(1)
            }
        {
            let declaration = location(file.path.shared(), name.span);
            if let Some(symbol) = entities.get(&declaration) {
                result.setters.push((
                    symbol.clone(),
                    (
                        symbol_id(file.source_text(name.span).unwrap_or_default()),
                        declaration,
                        call.owned_write_option,
                        source_kind,
                    ),
                ));
                if let Some(primitive) = primitive.as_deref() {
                    result
                        .source_primitives
                        .push((symbol.clone(), primitive.into()));
                }
            }
        }
    }
    result.accessors.retain(|(symbol, _)| {
        !result.contracted_accessor_symbols.contains(symbol)
            || !crate::value_identity::binding_has_write(file, entities, symbol)
    });
    for assignment in &file.ast.assignments {
        let (Some(initializer), Some(name)) = (
            assignment.call_initializer,
            assignment.array_slots.first().and_then(|slot| *slot),
        ) else {
            continue;
        };
        let Some(call) = ast_index.call_by_span(initializer) else {
            continue;
        };
        let symbol = entities.at(file.path.as_str(), name);
        let contracted = entities
            .at(file.path.as_str(), call.callee)
            .and_then(|callee| resolved_contracts.by_symbol.get(callee));
        if let Some((symbol, (contracted_return, contracted))) = symbol.zip(
            contracted
                .and_then(|contracted| contracted.summary.returns.known())
                .and_then(Option::as_ref)
                .and_then(|returned| returned.elements.first())
                .and_then(Option::as_ref)
                .zip(contracted),
        ) {
            push_contracted_return_source(
                &mut result,
                symbol,
                symbol_id(file.source_text(name).unwrap_or_default()),
                contracted_return,
                &contracted.local_name,
                &contracted.contract_location,
            );
            continue;
        }
        let primitive = call_primitive_name(file, call, entities, symbol_names, lookup.dialect);
        let resolved = known_primitive(&primitive);
        // Same seam as above, and the same reason.
        if !resolved.is_some_and(|primitive| lookup.dialect.returns_reactive_tuple(primitive)) {
            continue;
        }
        let Some(symbol) = symbol else {
            continue;
        };
        let declaration = location(file.path.shared(), name);
        let source_kind =
            if resolved.is_some_and(|primitive| lookup.dialect.returns_store(primitive)) {
                ReactiveSourceKind::Store
            } else {
                ReactiveSourceKind::Accessor
            };
        result.accessors.push((
            symbol.clone(),
            (
                symbol_id(file.source_text(name).unwrap_or_default()),
                declaration.clone(),
            ),
        ));
        result.source_kinds.push((symbol.clone(), source_kind));
        result.source_phases.push((
            symbol.clone(),
            if source_kind == ReactiveSourceKind::Store {
                2
            } else {
                0
            },
        ));
        result.returned_source_symbols.push(symbol.clone());
        result.summary_source_symbols.push(symbol.clone());
        if let Some(primitive) = primitive.as_deref() {
            result
                .source_primitives
                .push((symbol.clone(), primitive.into()));
        }
        if store_is_value_form(call, resolved) {
            result.value_form_stores.push(symbol.clone());
        }
        result
            .source_owned_write
            .push((symbol.clone(), call.owned_write_option));
        if let Some(setter) = assignment.array_slots.get(1).and_then(|slot| *slot)
            && let Some(setter_symbol) = entities.at(file.path.as_str(), setter)
        {
            result.setters.push((
                setter_symbol.clone(),
                (
                    symbol_id(file.source_text(setter).unwrap_or_default()),
                    location(file.path.shared(), setter),
                    call.owned_write_option,
                    source_kind,
                ),
            ));
            if let Some(primitive) = primitive.as_deref() {
                result
                    .source_primitives
                    .push((setter_symbol.clone(), primitive.into()));
            }
        }
    }
    for member in &file.ast.members {
        let Some(call) = ast_index.call_by_span(member.object) else {
            continue;
        };
        let Some(contracted) = entities
            .at(file.path.as_str(), call.callee)
            .and_then(|symbol| resolved_contracts.by_symbol.get(symbol))
        else {
            continue;
        };
        let Some(contracted_return) = contracted.summary.returns.known().and_then(Option::as_ref)
        else {
            continue;
        };
        let property = file.source_text(member.property).unwrap_or_default();
        let returned = match contracted_return.kind.as_str() {
            "object" => contracted_return.properties.get(property),
            "store-path" => Some(contracted_return),
            _ => None,
        };
        let Some((returned, symbol)) =
            returned.zip(entities.at(file.path.as_str(), member.property))
        else {
            continue;
        };
        push_contracted_return_source(
            &mut result,
            symbol,
            symbol_id(property),
            returned,
            &contracted.local_name,
            &contracted.contract_location,
        );
    }
    result
}

pub(crate) struct SourceDiscoveryMergeTarget<'a> {
    pub(crate) accessors: &'a mut HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) accessor_origins: &'a mut HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: &'a mut HashMap<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(crate) actions: &'a mut HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_kinds: &'a mut HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) source_primitives: &'a mut HashMap<SymbolId, SymbolId>,
    pub(crate) source_phases: &'a mut HashMap<SymbolId, u8>,
    pub(crate) returned_source_symbols: &'a mut HashSet<SymbolId>,
    pub(crate) summary_source_symbols: &'a mut HashSet<SymbolId>,
    pub(crate) source_owned_write: &'a mut HashMap<SymbolId, bool>,
    pub(crate) async_sources: &'a mut HashSet<SymbolId>,
    pub(crate) source_async_options: &'a mut HashMap<SymbolId, AsyncSourceOptions>,
    pub(crate) value_form_stores: &'a mut HashSet<SymbolId>,
    pub(crate) contracted_accessor_symbols: &'a mut HashSet<SymbolId>,
}

#[derive(Default)]
pub(crate) struct SourceDiscoveryAggregate {
    pub(crate) accessors: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) accessor_origins: HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: HashMap<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(crate) actions: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_kinds: HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) source_primitives: HashMap<SymbolId, SymbolId>,
    pub(crate) source_phases: HashMap<SymbolId, u8>,
    pub(crate) returned_source_symbols: HashSet<SymbolId>,
    pub(crate) summary_source_symbols: HashSet<SymbolId>,
    pub(crate) source_owned_write: HashMap<SymbolId, bool>,
    pub(crate) async_sources: HashSet<SymbolId>,
    pub(crate) source_async_options: HashMap<SymbolId, AsyncSourceOptions>,
    pub(crate) value_form_stores: HashSet<SymbolId>,
    pub(crate) contracted_accessor_symbols: HashSet<SymbolId>,
}

impl SourceDiscoveryAggregate {
    pub(crate) fn merge(&mut self, contribution: &SourceDiscoveryContribution) {
        merge_source_discovery(
            contribution,
            SourceDiscoveryMergeTarget {
                accessors: &mut self.accessors,
                accessor_origins: &mut self.accessor_origins,
                setters: &mut self.setters,
                actions: &mut self.actions,
                source_kinds: &mut self.source_kinds,
                source_primitives: &mut self.source_primitives,
                source_phases: &mut self.source_phases,
                returned_source_symbols: &mut self.returned_source_symbols,
                summary_source_symbols: &mut self.summary_source_symbols,
                source_owned_write: &mut self.source_owned_write,
                async_sources: &mut self.async_sources,
                source_async_options: &mut self.source_async_options,
                value_form_stores: &mut self.value_form_stores,
                contracted_accessor_symbols: &mut self.contracted_accessor_symbols,
            },
        );
    }

    pub(crate) fn append_to(self, target: SourceDiscoveryMergeTarget<'_>) {
        target.accessors.extend(self.accessors);
        target.accessor_origins.extend(self.accessor_origins);
        target.setters.extend(self.setters);
        target.actions.extend(self.actions);
        target.source_kinds.extend(self.source_kinds);
        target.source_primitives.extend(self.source_primitives);
        target.source_phases.extend(self.source_phases);
        target
            .returned_source_symbols
            .extend(self.returned_source_symbols);
        target
            .summary_source_symbols
            .extend(self.summary_source_symbols);
        target.source_owned_write.extend(self.source_owned_write);
        target.async_sources.extend(self.async_sources);
        target
            .source_async_options
            .extend(self.source_async_options);
        target.value_form_stores.extend(self.value_form_stores);
        target
            .contracted_accessor_symbols
            .extend(self.contracted_accessor_symbols);
    }
}

pub(crate) fn merge_source_discovery(
    contribution: &SourceDiscoveryContribution,
    target: SourceDiscoveryMergeTarget<'_>,
) {
    target
        .accessors
        .extend(contribution.accessors.iter().cloned());
    target
        .accessor_origins
        .extend(contribution.accessor_origins.iter().cloned());
    target.setters.extend(contribution.setters.iter().cloned());
    target.actions.extend(contribution.actions.iter().cloned());
    target
        .source_kinds
        .extend(contribution.source_kinds.iter().cloned());
    target
        .source_primitives
        .extend(contribution.source_primitives.iter().cloned());
    target
        .source_phases
        .extend(contribution.source_phases.iter().cloned());
    target
        .returned_source_symbols
        .extend(contribution.returned_source_symbols.iter().cloned());
    target
        .summary_source_symbols
        .extend(contribution.summary_source_symbols.iter().cloned());
    target
        .source_owned_write
        .extend(contribution.source_owned_write.iter().cloned());
    target
        .async_sources
        .extend(contribution.async_sources.iter().cloned());
    target
        .source_async_options
        .extend(contribution.source_async_options.iter().cloned());
    target
        .value_form_stores
        .extend(contribution.value_form_stores.iter().cloned());
    target
        .contracted_accessor_symbols
        .extend(contribution.contracted_accessor_symbols.iter().cloned());
}

pub(crate) fn extend_source_discovery_symbols(
    symbols: &mut HashSet<SymbolId>,
    contribution: &SourceDiscoveryContribution,
) {
    symbols.extend(
        contribution
            .accessors
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .accessor_origins
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .setters
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .actions
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .source_kinds
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .source_primitives
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(contribution.async_sources.iter().cloned());
    symbols.extend(
        contribution
            .source_async_options
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(contribution.value_form_stores.iter().cloned());
}

/// Owned reactive-source facts produced by the source-discovery stage and
/// consumed by the later interprocedural, static, and owner stages.
pub(crate) struct SourceDiscovery {
    pub(crate) accessors: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) accessor_origins: HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: HashMap<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(crate) actions: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_kinds: HashMap<SymbolId, ReactiveSourceKind>,
    /// The dialect primitive (canonical spelling) that created each reactive
    /// source, and, for a setter bound from the same tuple, the primitive that
    /// returned it: an owned-scope write guard can depend on which primitive
    /// made the setter (`Dialect::optimistic_store_setter_guarded`). A setter
    /// found only by its type has no entry.
    pub(crate) source_primitives: HashMap<SymbolId, SymbolId>,
    pub(crate) source_phases: HashMap<SymbolId, u8>,
    pub(crate) returned_source_symbols: HashSet<SymbolId>,
    pub(crate) summary_source_symbols: HashSet<SymbolId>,
    pub(crate) source_owned_write: HashMap<SymbolId, bool>,
    pub(crate) async_sources: HashSet<SymbolId>,
    pub(crate) source_async_options: HashMap<SymbolId, AsyncSourceOptions>,
    /// Store bindings proven to come from the value form
    /// (`createStore(value)` / `createOptimisticStore(value)`), which builds
    /// no compute node: `refresh()` on them (or a child record) throws
    /// `INVALID_REFRESH_TARGET` in dev (probed, rc.0). A store whose
    /// construction form is unknown is absent, keeping refresh acceptance.
    pub(crate) value_form_stores: HashSet<SymbolId>,
    pub(crate) contract_reads: HashMap<SymbolId, Vec<crate::ContractReadSite>>,
    pub(crate) contract_parameter_reads: HashMap<SymbolId, Vec<crate::ContractParameterReadSite>>,
    pub(crate) contract_callbacks: HashMap<SymbolId, Vec<ContractCallback>>,
    pub(crate) contract_returns: HashMap<SymbolId, (ContractReturn, Location)>,
    pub(crate) contracted_accessor_symbols: HashSet<SymbolId>,
    pub(crate) prop_sources: HashMap<SymbolId, (SymbolId, Location)>,
    /// Prop roots whose enclosing function may be a component only by a
    /// dialect convention. Reads through them remain explicit proof
    /// obligations until a JSX call site or exact component type resolves the
    /// identity.
    pub(crate) uncertain_prop_sources: HashSet<SymbolId>,
    /// Caller-proven props reactivity per props declaration; empty (answering
    /// `Reactive` everywhere) for dialects that keep the upstream
    /// over-approximation.
    pub(crate) props_reactivity: PropsReactivityIndex,
    pub(crate) retained_source_paths: HashSet<String>,
    pub(crate) changed_source_symbols: HashSet<SymbolId>,
}

/// The stable, read-mostly environment threaded through every pipeline stage:
/// project facts, prebuilt indexes, resolved contracts, and the semantic lookup.
#[derive(Clone, Copy)]
pub(crate) struct StageContext<'a> {
    pub(crate) facts: &'a ProjectFacts,
    pub(crate) project_indexes: &'a ProjectIndexes<'a>,
    pub(crate) typescript_indexes: &'a CachedTypeScriptIndexes,
    pub(crate) entities: &'a EntitySymbols,
    pub(crate) source_declarations: &'a HashMap<SymbolId, Declaration>,
    pub(crate) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) semantic_lookup: &'a SemanticLookup<'a>,
    pub(crate) resolved_contracts: &'a ResolvedContracts,
    pub(crate) runtime: &'a crate::RuntimeEnvironment,
}

/// The spans of the function expressions that are `element`'s children value:
/// a `{...}` child expression container holding exactly one function (after
/// parentheses and type wrappers), or a `children={...}` attribute value.
/// Anything else lexically inside the element -- an attribute handler, a ref
/// callback, a nested element's own callbacks -- is not the children callback.
fn jsx_children_function_spans(
    file: &FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> Vec<solid_facts::core::Span> {
    let mut candidates = Vec::new();
    for child in &element.children {
        let Some(text) = file.source_text(*child) else {
            continue;
        };
        let Some(inner) = text
            .strip_prefix('{')
            .and_then(|rest| rest.strip_suffix('}'))
        else {
            continue;
        };
        let (Ok(leading), Ok(trailing)) = (
            u32::try_from(inner.len() - inner.trim_start().len()),
            u32::try_from(inner.len() - inner.trim_end().len()),
        ) else {
            continue;
        };
        let start = child.start + 1 + leading;
        let end = child.end - 1 - trailing;
        if start < end {
            candidates.push(solid_facts::core::Span::new(start, end));
        }
    }
    for attribute in &element.attributes {
        if attribute.namespace.is_none()
            && attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
            && file.source_text(attribute.name) == Some("children")
            && let Some(expression) = attribute.expression
        {
            candidates.push(expression);
        }
    }
    candidates
        .into_iter()
        .map(|candidate| file.ast.peel_ts_sugar_span(candidate))
        .collect()
}

/// Classifies a non-literal `keyed` attribute value.
///
/// Only a *proven function* selects the custom-key overload statically: an
/// inline function literal, or a value whose demanded type facts say
/// callable. Everything else — a boolean-typed expression, a member read, an
/// unresolved value — is a [`solid_dialect::KeyForm::DynamicFlag`], whose
/// runtime truthiness picks the keyed or unkeyed overload. RFC 03 tells
/// authors to "avoid dynamic boolean `keyed` values with function children"
/// and "prefer a literal `true`, literal `false`, or a custom key function";
/// until they do, the callback shape is ambiguous and the dialect tables
/// claim no accessor for it rather than fabricate a source for what may be a
/// raw value (see `children_accessor_parameters` in both dialects).
fn dynamic_key_form(
    file: &FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    semantic_lookup: &SemanticLookup<'_>,
) -> solid_dialect::KeyForm {
    let expression = element.attributes.iter().find_map(|attribute| {
        (attribute.namespace.is_none() && file.source_text(attribute.local_name) == Some("keyed"))
            .then_some(attribute.expression)
            .flatten()
    });
    let Some(expression) = expression else {
        // A string or element value: truthy at runtime, but not a key
        // function — the ambiguous-flag stance applies.
        return solid_dialect::KeyForm::DynamicFlag;
    };
    if file
        .ast
        .functions
        .iter()
        .any(|function| function.span == expression)
    {
        return solid_dialect::KeyForm::CustomKey;
    }
    // Callability is trusted for plain identifier values only. At a call
    // expression's span the demanded entity answers for the *callee* — a
    // boolean-returning `keyed={cond()}` reads back `Callable` — so any
    // computed value stays in the ambiguous bucket.
    let identifier = file.ast.identifiers.iter().any(|identifier| {
        identifier.span == expression
            && identifier.role == solid_facts::ast::IdentifierRole::Reference
    });
    if identifier
        && matches!(
            semantic_lookup
                .entity_at(file.path.as_str(), expression)
                .and_then(|entity| entity.callability),
            Some(typefacts::Callability::Callable | typefacts::Callability::UntypedCallable)
        )
    {
        return solid_dialect::KeyForm::CustomKey;
    }
    // Boolean-typed, computed, or unresolved: refuse to fabricate.
    solid_dialect::KeyForm::DynamicFlag
}

pub(crate) fn discover_sources(
    ctx: &StageContext<'_>,
    source_discovery_cache: Option<&mut HashMap<SourcePath, CachedSourceDiscovery>>,
    typescript_unchanged: bool,
    build_timings: &mut BuildTimings,
    emit_timings: bool,
) -> SourceDiscovery {
    let StageContext {
        facts,
        project_indexes,
        typescript_indexes,
        entities,
        source_declarations,
        symbol_names,
        semantic_lookup,
        resolved_contracts,
        runtime,
    } = *ctx;
    let mut clock = StageClock::new(emit_timings);
    let mut accessors = HashMap::<SymbolId, (SymbolId, Location)>::new();
    let mut accessor_origins = HashMap::<SymbolId, (SymbolId, SymbolId, Location)>::new();
    let mut setters = HashMap::<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>::new();
    let mut actions = HashMap::<SymbolId, (SymbolId, Location)>::new();
    let mut source_kinds = HashMap::<SymbolId, ReactiveSourceKind>::new();
    let mut source_primitives = HashMap::<SymbolId, SymbolId>::new();
    let mut source_phases = HashMap::<SymbolId, u8>::new();
    let mut returned_source_symbols = HashSet::<SymbolId>::new();
    let mut summary_source_symbols = HashSet::<SymbolId>::new();
    let mut source_owned_write = HashMap::<SymbolId, bool>::new();
    let mut async_sources = HashSet::<SymbolId>::new();
    let mut source_async_options = HashMap::<SymbolId, AsyncSourceOptions>::new();
    let mut value_form_stores = HashSet::<SymbolId>::new();
    let mut contract_reads = HashMap::<SymbolId, Vec<crate::ContractReadSite>>::new();
    let mut contract_parameter_reads =
        HashMap::<SymbolId, Vec<crate::ContractParameterReadSite>>::new();
    let mut contract_callbacks = HashMap::<SymbolId, Vec<ContractCallback>>::new();
    let mut contract_returns = HashMap::<SymbolId, (ContractReturn, Location)>::new();
    let mut contracted_accessor_symbols = HashSet::<SymbolId>::new();

    for contracted in &resolved_contracts.bindings {
        let direct_reads = contracted
            .summary
            .reactive_reads
            .known()
            .into_iter()
            .flatten()
            .filter(|read| matches!(read.kind.as_str(), "accessor" | "store-path"))
            .map(|read| {
                (
                    format!("{}.{}", contracted.package_name, contracted.imported_name),
                    contracted.local_name.clone(),
                    contracted.contract_location.clone(),
                    read.kind.clone(),
                    read.execution.clone(),
                )
            })
            .collect::<Vec<_>>();
        if !direct_reads.is_empty() {
            contract_reads.insert(contracted.symbol.clone(), direct_reads);
        }
        let parameter_reads = contracted
            .summary
            .reactive_reads
            .known()
            .into_iter()
            .flatten()
            .filter_map(|read| {
                read.parameter.map(|parameter| {
                    (
                        parameter,
                        format!("{}.{}", contracted.package_name, contracted.imported_name),
                        contracted.local_name.clone(),
                        contracted.contract_location.clone(),
                        read.execution.clone(),
                    )
                })
            })
            .collect::<Vec<_>>();
        if !parameter_reads.is_empty() {
            contract_parameter_reads.insert(contracted.symbol.clone(), parameter_reads);
        }
        // Callable rows and explicit value enumerations only. An enumeration
        // is retained for the occurrence Get consumer, never as a callable:
        // a non-call row (`ContractCallback::is_invocation`) is no
        // graph edge, no invoked parameter, no wrapper and no re-pushed row. A
        // member-path row is kept and every reader resolves the member it
        // calls before folding anything (`contract_callback_invoked_value`,
        // item B of ways-to-improve § 3.3). The key is still
        // inserted for a known enumeration holding none, because its presence
        // is the "callbacks known" fact the interprocedural pass reads.
        if let Some(callbacks) = contracted.summary.callbacks.known() {
            contract_callbacks.insert(
                contracted.symbol.clone(),
                callbacks
                    .iter()
                    .filter(|callback| {
                        callback.is_invocation() || callback.protocol.is_value_enumeration()
                    })
                    .cloned()
                    .collect(),
            );
        }
        if let Some(returned) = contracted.summary.returns.known().and_then(Option::as_ref) {
            contract_returns.insert(
                contracted.symbol.clone(),
                (returned.clone(), contracted.contract_location.clone()),
            );
            source_kinds.insert(
                contracted.symbol.clone(),
                if returned.kind == "store-path" {
                    ReactiveSourceKind::Store
                } else {
                    ReactiveSourceKind::Accessor
                },
            );
        }
    }

    let mut retained_source_paths = HashSet::<String>::new();
    let mut changed_source_symbols = HashSet::<SymbolId>::new();
    match source_discovery_cache {
        None => {
            let contributions = parallel_file_results(&facts.files, |file| {
                discover_file_sources(
                    semantic_lookup,
                    file,
                    project_indexes
                        .ast_files_by_path
                        .get(file.path.as_str())
                        .expect("project index contains every source file"),
                    entities,
                    symbol_names,
                    resolved_contracts,
                )
            });
            let mut aggregate = SourceDiscoveryAggregate::default();
            for contribution in &contributions {
                aggregate.merge(contribution);
            }
            aggregate.append_to(SourceDiscoveryMergeTarget {
                accessors: &mut accessors,
                accessor_origins: &mut accessor_origins,
                setters: &mut setters,
                actions: &mut actions,
                source_kinds: &mut source_kinds,
                source_primitives: &mut source_primitives,
                source_phases: &mut source_phases,
                returned_source_symbols: &mut returned_source_symbols,
                summary_source_symbols: &mut summary_source_symbols,
                source_owned_write: &mut source_owned_write,
                async_sources: &mut async_sources,
                source_async_options: &mut source_async_options,
                value_form_stores: &mut value_form_stores,
                contracted_accessor_symbols: &mut contracted_accessor_symbols,
            });
        }
        Some(cache) => {
            let current_paths = facts
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<HashSet<_>>();
            cache.retain(|path, _| current_paths.contains(path.as_str()));
            let reusable_paths = facts
                .files
                .iter()
                .filter_map(|file| {
                    cache
                        .get(file.path.as_str())
                        .is_some_and(|cached| {
                            // A returned-source proof's callee syntax is bound
                            // by `cross_file_proof_digest` (ADR 0222); its
                            // symbol resolution by the per-file delta.
                            source_discovery_identity_matches(
                                &cached.identity,
                                file.path.as_str(),
                                &file.source_hash,
                                typescript_unchanged,
                                typescript_indexes.source_discovery_delta.as_ref(),
                            ) && cached.cross_file_proofs
                                == semantic_lookup.cross_file_proof_digest()
                        })
                        .then_some(file.path.as_str())
                })
                .collect::<HashSet<_>>();
            let recomputed = facts
                .files
                .iter()
                .filter(|file| !reusable_paths.contains(file.path.as_str()))
                .collect::<Vec<_>>();
            let discovered = parallel_slice_results(&recomputed, |file| {
                (
                    source_discovery_identity(file, project_indexes),
                    discover_file_sources(
                        semantic_lookup,
                        file,
                        project_indexes
                            .ast_files_by_path
                            .get(file.path.as_str())
                            .expect("project index contains every source file"),
                        entities,
                        symbol_names,
                        resolved_contracts,
                    ),
                )
            });
            let mut discovered = discovered.into_iter();
            for file in &facts.files {
                if reusable_paths.contains(file.path.as_str()) {
                    build_timings.source_discovery_reused_files += 1;
                    retained_source_paths.insert(file.path.to_string());
                    continue;
                }
                let (identity, contribution) = discovered
                    .next()
                    .expect("recomputed source path has a fresh contribution");
                build_timings.source_discovery_recomputed_files += 1;
                if let Some(cached) = cache.get(file.path.as_str()) {
                    extend_source_discovery_symbols(
                        &mut changed_source_symbols,
                        &cached.contribution,
                    );
                }
                extend_source_discovery_symbols(&mut changed_source_symbols, &contribution);
                cache.insert(
                    file.path.clone(),
                    CachedSourceDiscovery {
                        identity,
                        cross_file_proofs: semantic_lookup.cross_file_proof_digest(),
                        contribution,
                    },
                );
            }
            debug_assert!(discovered.next().is_none());
            let cache = &*cache;
            for aggregate in parallel_file_chunk_results(&facts.files, |files| {
                let mut aggregate = SourceDiscoveryAggregate::default();
                for file in files {
                    if let Some(cached) = cache.get(file.path.as_str()) {
                        aggregate.merge(&cached.contribution);
                    }
                }
                aggregate
            }) {
                aggregate.append_to(SourceDiscoveryMergeTarget {
                    accessors: &mut accessors,
                    accessor_origins: &mut accessor_origins,
                    setters: &mut setters,
                    actions: &mut actions,
                    source_kinds: &mut source_kinds,
                    source_primitives: &mut source_primitives,
                    source_phases: &mut source_phases,
                    returned_source_symbols: &mut returned_source_symbols,
                    summary_source_symbols: &mut summary_source_symbols,
                    source_owned_write: &mut source_owned_write,
                    async_sources: &mut async_sources,
                    source_async_options: &mut source_async_options,
                    value_form_stores: &mut value_form_stores,
                    contracted_accessor_symbols: &mut contracted_accessor_symbols,
                });
            }
        }
    }
    clock.finish(build_timings, ReactiveIrStage::SourceDiscovery);
    // Source construction/accepted-return facts collected above, before the
    // type-only fallback. A published Accessor/Store annotation alone cannot
    // witness a runtime reactive Get in an incoming prop expression.
    let mut runtime_sources = accessors.keys().cloned().collect::<HashSet<_>>();
    for entity in facts.typescript.entities() {
        let Some(descriptor) = &entity.type_descriptor else {
            continue;
        };
        if !semantic_lookup
            .dialect
            .owns_module(descriptor.origin_module.as_ref())
        {
            continue;
        }
        let Some(symbol) = entities.get(&entity.location) else {
            continue;
        };
        if resolved_contracts.by_symbol.contains_key(symbol) {
            continue;
        }
        let Some((role, type_declaration)) =
            descriptor
                .alias_declarations
                .iter()
                .find_map(|declaration| {
                    semantic_lookup
                        .dialect
                        .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                        .map(|role| (role, declaration.location.clone()))
                })
        else {
            continue;
        };
        if matches!(
            role,
            solid_dialect::TypeRole::Component | solid_dialect::TypeRole::Owner
        ) {
            continue;
        }
        if semantic_lookup
            .file_by_path(entity.location.path.as_ref())
            .is_some_and(|file| {
                let (Ok(start), Ok(end)) = (
                    u32::try_from(entity.location.start_byte),
                    u32::try_from(entity.location.end_byte),
                ) else {
                    return true;
                };
                crate::callback_return::binding_is_callback_result(
                    semantic_lookup,
                    file,
                    solid_facts::core::Span::new(start, end),
                )
            })
        {
            continue;
        }
        // ADR 0172: a tuple containing an accessor is not itself an accessor.
        // Demanded member/call spans may carry the member's type beside a
        // receiver symbol. Only an exact value's own callable type can
        // introduce the typed root; a member must also resolve separately
        // from its receiver. Nested alias declarations cannot do so.
        if matches!(
            role,
            solid_dialect::TypeRole::Accessor | solid_dialect::TypeRole::Signal
        ) && (entity.callability != Some(typefacts::Callability::Callable)
            || semantic_lookup
                .file_by_path(entity.location.path.as_ref())
                .is_some_and(|file| {
                    crate::value_identity::binding_has_write(file, entities, symbol)
                })
            || !semantic_lookup
                .file_by_path(entity.location.path.as_ref())
                .is_some_and(|file| {
                    file.ast.identifiers.iter().any(|identifier| {
                        u64::from(identifier.span.start) == entity.location.start_byte
                            && u64::from(identifier.span.end) == entity.location.end_byte
                    }) || file.ast.members.iter().any(|member| {
                        ((u64::from(member.span.start) == entity.location.start_byte
                            && u64::from(member.span.end) == entity.location.end_byte)
                            || (u64::from(member.property.start) == entity.location.start_byte
                                && u64::from(member.property.end) == entity.location.end_byte))
                            && !file.ast.computed_members.contains(&member.span)
                            && entities.at(file.path.as_str(), member.property) == Some(symbol)
                            && entities.at(file.path.as_str(), member.object) != Some(symbol)
                            && !is_contracted_container_value(
                                file,
                                entities,
                                resolved_contracts,
                                member.object,
                                16,
                            )
                    })
                }))
        {
            continue;
        }
        let declaration = source_declarations.get(symbol);
        let (name, local_location) = declaration.map_or_else(
            || ("accessor".into(), entity.location.clone()),
            |declaration| (declaration.name.clone(), declaration.location.clone()),
        );
        let declaration_location = if type_declaration.path.is_empty() {
            local_location
        } else {
            type_declaration
        };
        match role {
            solid_dialect::TypeRole::Accessor | solid_dialect::TypeRole::Signal => {
                accessors
                    .entry(symbol.clone())
                    .or_insert((symbol_id(name.as_ref()), declaration_location));
                source_kinds
                    .entry(symbol.clone())
                    .or_insert(ReactiveSourceKind::Accessor);
                source_phases.entry(symbol.clone()).or_insert(1);
            }
            solid_dialect::TypeRole::Store => {
                accessors
                    .entry(symbol.clone())
                    .or_insert((symbol_id(name.as_ref()), declaration_location));
                source_kinds
                    .entry(symbol.clone())
                    .or_insert(ReactiveSourceKind::Store);
                source_phases.entry(symbol.clone()).or_insert(1);
            }
            solid_dialect::TypeRole::Setter | solid_dialect::TypeRole::StoreSetter => {
                let kind = if role == solid_dialect::TypeRole::StoreSetter {
                    ReactiveSourceKind::Store
                } else {
                    ReactiveSourceKind::Accessor
                };
                setters.entry(symbol.clone()).or_insert((
                    symbol_id(name.as_ref()),
                    declaration_location,
                    false,
                    kind,
                ));
            }
            solid_dialect::TypeRole::Component | solid_dialect::TypeRole::Owner => unreachable!(),
        }
    }
    for file in &facts.files {
        for element in &file.ast.jsx_elements {
            let dialect = semantic_lookup.dialect;
            let primitive = jsx_primitive_name(file, element, entities, symbol_names, dialect);
            let keyed = element
                .boolean_properties
                .iter()
                .find(|property| file.source_text(property.name) == Some("keyed"))
                .map(|property| property.value);
            let key = match keyed {
                Some(true) => solid_dialect::KeyForm::Keyed,
                Some(false) => solid_dialect::KeyForm::Unkeyed,
                None if element
                    .properties
                    .iter()
                    .any(|property| file.source_text(*property) == Some("keyed")) =>
                {
                    dynamic_key_form(file, element, semantic_lookup)
                }
                None => solid_dialect::KeyForm::Absent,
            };
            // Which children parameters are accessors is the dialect's
            // question. The match this replaced knew `<For>` and not
            // `<Index>`, which are exact mirrors in 1.x, so every 1.x
            // `<Index>` item accessor would be invisible to source discovery.
            let parameter_indices = known_primitive(&primitive).map_or(&[][..], |primitive| {
                dialect.children_accessor_parameters(primitive, key)
            });
            if parameter_indices.is_empty() {
                continue;
            }
            // The accessor parameters belong to the function that IS this
            // element's children value -- the sole `{...}` child expression
            // or a `children={...}` attribute -- and to no other function that
            // happens to sit lexically inside the element (a `<For>` row
            // callback, an event handler, or a ref callback are plain-value
            // callbacks of their own).
            let children_functions = jsx_children_function_spans(file, element);
            for function in file
                .ast
                .functions
                .iter()
                .filter(|function| children_functions.contains(&function.span))
            {
                for index in parameter_indices {
                    let Some(parameter) = function
                        .parameters
                        .get(*index)
                        .and_then(|parameter| parameter.names.first())
                    else {
                        continue;
                    };
                    let declaration = location(file.path.shared(), parameter.span);
                    if let Some(symbol) = entities.get(&declaration) {
                        runtime_sources.insert(symbol.clone());
                        accessors.entry(symbol.clone()).or_insert((
                            symbol_id(file.source_text(parameter.span).unwrap_or_default()),
                            declaration,
                        ));
                    }
                }
            }
        }

        // Callback parameters can themselves be runtime-created accessors.
        // This is not derivable from the callback's TypeScript declaration:
        // mapArray/indexArray create the index/item signals internally and
        // hand them to the mapper. Keep that contract in the dialect beside
        // the JSX-children equivalent above.
        for call in &file.ast.calls {
            if let Some(symbol) = semantic_lookup.callee_symbol(file, call.callee)
                && let Some(callbacks) = contract_callbacks.get(symbol)
            {
                for callback in callbacks {
                    // The function the row invokes: the argument itself, or
                    // for a member-path row (item B) the member the call's own
                    // literal names -- never the argument when it is only the
                    // container of the invoked member.
                    let Some((_, Some(invoked))) =
                        crate::interproc::contract_callback_invoked_value(
                            file,
                            semantic_lookup,
                            call,
                            callback,
                        )
                    else {
                        continue;
                    };
                    let Some(function) = file
                        .ast
                        .functions
                        .iter()
                        .find(|function| function.span == file.ast.peel_ts_sugar_span(invoked))
                    else {
                        continue;
                    };
                    for (parameter_index, descriptor) in callback.arguments.iter().enumerate() {
                        let Some(descriptor) = descriptor else {
                            continue;
                        };
                        if descriptor.kind != "accessor" {
                            continue;
                        }
                        let Some(parameter) = function
                            .parameters
                            .get(parameter_index)
                            .and_then(|parameter| parameter.names.first())
                        else {
                            continue;
                        };
                        let declaration = location(file.path.shared(), parameter.span);
                        if let Some(parameter_symbol) = entities.get(&declaration) {
                            runtime_sources.insert(parameter_symbol.clone());
                            accessors.entry(parameter_symbol.clone()).or_insert((
                                symbol_id(file.source_text(parameter.span).unwrap_or_default()),
                                declaration,
                            ));
                        }
                    }
                }
            }
            let Some(primitive) =
                call_primitive_name(file, call, entities, symbol_names, semantic_lookup.dialect)
                    .as_ref()
                    .and_then(PrimitiveName::primitive)
            else {
                continue;
            };
            for (argument_index, argument) in call.arguments.iter().enumerate() {
                let parameter_indices = semantic_lookup
                    .dialect
                    .callback_semantics_at(primitive, argument_index, call.arguments.len())
                    .accessor_parameters;
                if parameter_indices.is_empty() {
                    continue;
                }
                let Some(function) =
                    file.ast.functions.iter().find(|function| {
                        function.span == file.ast.peel_ts_sugar_span(argument.span)
                    })
                else {
                    continue;
                };
                for parameter_index in parameter_indices {
                    let Some(parameter) = function
                        .parameters
                        .get(*parameter_index)
                        .and_then(|parameter| parameter.names.first())
                    else {
                        continue;
                    };
                    let declaration = location(file.path.shared(), parameter.span);
                    if let Some(symbol) = entities.get(&declaration) {
                        runtime_sources.insert(symbol.clone());
                        accessors.entry(symbol.clone()).or_insert((
                            symbol_id(file.source_text(parameter.span).unwrap_or_default()),
                            declaration,
                        ));
                    }
                }
            }
        }
    }
    for file in &facts.files {
        for call in &file.ast.calls {
            if !call_primitive_name(file, call, entities, symbol_names, semantic_lookup.dialect)
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| {
                    matches!(
                        primitive,
                        Primitive::CreateEffect | Primitive::CreateRenderEffect
                    )
                })
            {
                continue;
            }
            let Some(compute) = call.arguments.first().and_then(|argument| {
                file.ast
                    .functions
                    .iter()
                    .filter(|function| argument.span.contains(function.span))
                    .max_by_key(|function| function.span.end - function.span.start)
            }) else {
                continue;
            };
            let returned = compute.expression_return.as_ref().or_else(|| {
                file.ast.returns.iter().find(|returned| {
                    compute.body.contains(returned.span)
                        && containing_ast_function(&file.ast, returned.span)
                            .is_some_and(|owner| owner.span == compute.span)
                })
            });
            let Some(source_symbol) = returned
                .and_then(|returned| {
                    entities
                        .get(&location(file.path.shared(), returned.span))
                        .or_else(|| {
                            (returned.value == solid_facts::ast::ReturnValueKind::Identifier)
                                .then_some(returned.span)
                                .and_then(|span| file.source_text(span))
                                .and_then(|name| {
                                    source_declarations
                                        .iter()
                                        .find_map(|(symbol, declaration)| {
                                            (declaration.name == name.into()
                                                && declaration.location.path
                                                    == file.path.as_str().into())
                                            .then_some(symbol)
                                        })
                                })
                        })
                })
                // The second half of this test used to name `createStore` and
                // `createOptimisticStore`, and it could never change the
                // answer. `source_kinds` and `source_primitives` are written
                // together wherever a reactive source is discovered (the
                // `creates_reactive_source` path above and the tuple-binding
                // path), and the kind recorded there is `Store` exactly when
                // `returns_store` holds for that same primitive. So "the
                // primitive returns a store" implies "the kind is Store", for
                // any dialect, and only the kind is worth asking.
                .filter(|symbol| source_kinds.get(*symbol) == Some(&ReactiveSourceKind::Store))
            else {
                continue;
            };
            let Some(apply) = call.arguments.get(1).and_then(|argument| {
                file.ast
                    .functions
                    .iter()
                    .filter(|function| argument.span.contains(function.span))
                    .max_by_key(|function| function.span.end - function.span.start)
            }) else {
                continue;
            };
            let Some(parameter) = apply
                .parameters
                .first()
                .and_then(|parameter| parameter.names.first())
            else {
                continue;
            };
            let parameter_location = location(file.path.shared(), parameter.span);
            let Some(parameter_symbol) = entities.get(&parameter_location) else {
                continue;
            };
            let (display, declaration) =
                accessors.get(source_symbol).cloned().unwrap_or_else(|| {
                    (
                        symbol_id(file.source_text(parameter.span).unwrap_or_default()),
                        location(file.path.shared(), parameter.span),
                    )
                });
            accessors.insert(parameter_symbol.clone(), (display, declaration));
            source_kinds.insert(parameter_symbol.clone(), ReactiveSourceKind::Store);
        }
    }
    loop {
        let mut setter_aliases = Vec::new();
        let mut action_aliases = Vec::new();
        for file in &facts.files {
            for binding in &file.ast.bindings {
                let Some(source_symbol) =
                    binding
                        .initializer_identifier
                        .as_ref()
                        .and_then(|identifier| {
                            entities.get(&location(file.path.shared(), identifier.span))
                        })
                else {
                    continue;
                };
                let setter = setters.get(source_symbol).cloned();
                let action = actions.get(source_symbol).cloned();
                if setter.is_none() && action.is_none() {
                    continue;
                }
                for name in &binding.names {
                    let declaration = location(file.path.shared(), name.span);
                    let Some(symbol) = entities.get(&declaration) else {
                        continue;
                    };
                    if let Some((_, source, owned_write, source_kind)) = &setter
                        && !setters.contains_key(symbol)
                    {
                        setter_aliases.push((
                            symbol.clone(),
                            (
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                source.clone(),
                                *owned_write,
                                *source_kind,
                            ),
                        ));
                    }
                    if let Some((_, source)) = &action
                        && !actions.contains_key(symbol)
                    {
                        action_aliases.push((
                            symbol.clone(),
                            (
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                source.clone(),
                            ),
                        ));
                    }
                }
            }
        }
        if setter_aliases.is_empty() && action_aliases.is_empty() {
            break;
        }
        setters.extend(setter_aliases);
        actions.extend(action_aliases);
    }
    let mut prop_sources = HashMap::<SymbolId, (SymbolId, Location)>::new();
    let mut uncertain_prop_sources = HashSet::<SymbolId>::new();
    for file in &facts.files {
        for function in &file.ast.functions {
            let component_status = semantic_lookup.function_component_status(file, function);
            if component_status == crate::indexes::ComponentStatus::No {
                continue;
            }
            let Some(parameter) = function
                .parameters
                .first()
                .filter(|parameter| parameter.shape == solid_facts::ast::BindingShape::Identifier)
                .and_then(|parameter| parameter.names.first())
            else {
                continue;
            };
            let declaration = location(file.path.shared(), parameter.span);
            if let Some(symbol) = entities.get(&declaration) {
                if component_status == crate::indexes::ComponentStatus::Uncertain {
                    uncertain_prop_sources.insert(symbol.clone());
                }
                prop_sources.insert(
                    symbol.clone(),
                    (
                        symbol_id(file.source_text(parameter.span).unwrap_or_default()),
                        declaration,
                    ),
                );
            }
        }
    }
    clock.finish(build_timings, ReactiveIrStage::TypedAccessorsAndPropRoots);
    loop {
        let mut changed = false;
        for file in &facts.files {
            for binding in &file.ast.bindings {
                let source = binding
                    .initializer_identifier
                    .as_ref()
                    .and_then(|identifier| {
                        entities.get(&location(file.path.shared(), identifier.span))
                    })
                    .and_then(|symbol| {
                        prop_sources
                            .get(symbol)
                            .cloned()
                            .map(|source| (source, uncertain_prop_sources.contains(symbol)))
                    })
                    .or_else(|| {
                        let initializer = binding.call_initializer?;
                        let call = file.ast.call_at(initializer)?;
                        let primitive = call_primitive_name(
                            file,
                            call,
                            entities,
                            symbol_names,
                            semantic_lookup.dialect,
                        );
                        // The dialect names its own merge. This was a
                        // literal `"merge"` before the dialect seam existed,
                        // and the extraction translated the string to
                        // `Primitive::Merge` rather than to a row -- so the
                        // propagation answered for 2.0's spelling and was
                        // silent for 1.x's `mergeProps`, which is the same
                        // primitive under the other dialect's vocabulary.
                        if !known_primitive(&primitive).is_some_and(|primitive| {
                            semantic_lookup.dialect.merges_props_reactivity(primitive)
                        }) {
                            return None;
                        }
                        call.arguments.iter().find_map(|argument| {
                            let symbol =
                                entities.get(&location(file.path.shared(), argument.span))?;
                            prop_sources
                                .get(symbol)
                                .cloned()
                                .map(|source| (source, uncertain_prop_sources.contains(symbol)))
                        })
                    });
                let Some(((_, declaration), source_uncertain)) = source else {
                    continue;
                };
                for name in &binding.names {
                    let binding_location = location(file.path.shared(), name.span);
                    if let Some(symbol) = entities.get(&binding_location)
                        && !prop_sources.contains_key(symbol)
                    {
                        if source_uncertain {
                            uncertain_prop_sources.insert(symbol.clone());
                        }
                        prop_sources.insert(
                            symbol.clone(),
                            (
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                declaration.clone(),
                            ),
                        );
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    runtime_sources.retain(|symbol| {
        !facts.files.iter().any(|file| {
            crate::value_identity::binding_has_write(file, entities, symbol)
                || file.ast.iteration_targets.iter().any(|target| {
                    file.ast.identifiers.iter().any(|identifier| {
                        target.contains(identifier.span)
                            && entities.at(file.path.as_str(), identifier.span) == Some(symbol)
                    })
                })
        })
    });
    let props_reactivity = classify_component_props(
        runtime,
        facts,
        semantic_lookup,
        entities,
        &accessors,
        &source_kinds,
        &prop_sources,
        &runtime_sources,
    );
    clock.finish(
        build_timings,
        ReactiveIrStage::PropPropagationAndControlFlow,
    );
    SourceDiscovery {
        accessors,
        accessor_origins,
        setters,
        actions,
        source_kinds,
        source_primitives,
        source_phases,
        returned_source_symbols,
        summary_source_symbols,
        source_owned_write,
        async_sources,
        source_async_options,
        value_form_stores,
        contract_reads,
        contract_parameter_reads,
        contract_callbacks,
        contract_returns,
        contracted_accessor_symbols,
        prop_sources,
        uncertain_prop_sources,
        props_reactivity,
        retained_source_paths,
        changed_source_symbols,
    }
}

/// How each use of one prop is classified once the component's callers are
/// enumerated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PropUse {
    /// Every visible call site passes a static value: the prop compiles to a
    /// plain property and reading it can never warn or misbehave.
    Static,
    /// Some visible call site passes a proven reactive expression: the prop
    /// is signal-backed and untracked reads are proven runtime warnings.
    Reactive,
    /// Neither provable: the component escapes enumeration, or a passed value
    /// resolves to nothing the engine can classify.
    Unknown,
}

impl PropUse {
    fn worst(self, other: Self) -> Self {
        match (self, other) {
            (Self::Reactive, _) | (_, Self::Reactive) => Self::Reactive,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::Static,
        }
    }
}

/// Caller-proven reactivity of one component's props.
///
/// rc.0 ground truth (probed): `devComponent` opens a strict-read window with
/// `untrack(() => Comp(props), '<Name>')`, and `STRICT_READ_UNTRACKED` fires
/// only when a prop *getter* reads reactive state inside it. `{ title:
/// "Hello" }` never warns; `{ get title() { return sig() } }` warns. Whether
/// a prop is signal-backed is therefore a fact about the component's callers,
/// which this records per prop name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PropsReactivity {
    /// Every in-project use is an enumerated JSX call site with no spread
    /// attributes. `reactive` holds the prop names some call site passes a
    /// proven reactive expression for; `unresolved` holds the names whose
    /// passed value could be proven neither way. Every other prop is proven
    /// static.
    Enumerated {
        reactive: BTreeSet<String>,
        unresolved: BTreeSet<String>,
        /// Prop names for which at least one exact JSX call site passes a
        /// proven accessor value. Reading the property is static, but calling
        /// that value performs a reactive read and must stay distinguishable
        /// from ordinary prop backing.
        accessor_values: BTreeSet<String>,
    },
    /// The component escapes enumeration — exported, referenced outside JSX,
    /// or spread into at a call site — so no prop can be proven *static*.
    ///
    /// The JSX call sites that are visible still prove what they pass, and
    /// that direction of the proof survives the escape. "Some caller passes a
    /// reactive expression" is monotone under adding callers: a consumer
    /// outside the project can add a call site, never remove the one written
    /// here, so an untracked read of a witnessed prop is a proven defect on
    /// that path. "Every caller passes a static value" is the opposite — one
    /// unseen caller falsifies it — which is why it needs complete
    /// enumeration and is never concluded here.
    ///
    /// `reactive` holds the witnessed prop names and `accessor_values` the
    /// names some visible call site passes a proven accessor for. Every other
    /// prop is a proof obligation.
    Escaping {
        reactive: BTreeSet<String>,
        accessor_values: BTreeSet<String>,
    },
}

/// The per-declaration classification map, keyed by the props parameter's
/// declaration location (the same location `prop_sources` carries, so aliases
/// and `merge` results resolve to their component's classification) and by
/// the parameter pattern span (how the destructure rule addresses a
/// destructured parameter).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PropsReactivityIndex {
    /// Whether the dialect asked for caller proof at all. When false, every
    /// query answers [`PropUse::Reactive`] — the upstream over-approximation
    /// the 1.x catalog pins for eslint-plugin-solid parity.
    caller_proof: bool,
    by_declaration: HashMap<Location, PropsReactivity>,
}

impl PropsReactivityIndex {
    pub(crate) fn prop_use(&self, declaration: &Location, name: &str) -> PropUse {
        if !self.caller_proof {
            return PropUse::Reactive;
        }
        self.by_declaration
            .get(declaration)
            .map_or(PropUse::Unknown, |classification| {
                classification_use(classification, name)
            })
    }

    /// The classification of a whole-object use (aliasing, spreading, or
    /// destructuring with a rest element): reactive if any prop is, unknown
    /// if any prop is, static only when every caller-passed value is.
    pub(crate) fn object_use(&self, declaration: &Location) -> PropUse {
        if !self.caller_proof {
            return PropUse::Reactive;
        }
        match self.by_declaration.get(declaration) {
            None => PropUse::Unknown,
            Some(PropsReactivity::Escaping { reactive, .. }) => {
                if reactive.is_empty() {
                    PropUse::Unknown
                } else {
                    PropUse::Reactive
                }
            }
            Some(PropsReactivity::Enumerated {
                reactive,
                unresolved,
                ..
            }) => {
                if !reactive.is_empty() {
                    PropUse::Reactive
                } else if !unresolved.is_empty() {
                    PropUse::Unknown
                } else {
                    PropUse::Static
                }
            }
        }
    }

    /// Whether invoking this prop value is proven to invoke an accessor at
    /// any enumerated JSX call site. This is separate from [`Self::prop_use`]:
    /// passing `items` as `<List items={items}>` stores a stable function in
    /// the prop, while `props.items()` still subscribes when evaluated.
    pub(crate) fn accessor_value_use(&self, declaration: &Location, name: &str) -> PropUse {
        if !self.caller_proof {
            return PropUse::Unknown;
        }
        match self.by_declaration.get(declaration) {
            Some(
                PropsReactivity::Enumerated {
                    accessor_values, ..
                }
                | PropsReactivity::Escaping {
                    accessor_values, ..
                },
            ) if accessor_values.contains(name) => PropUse::Reactive,
            // Only a complete caller set can prove the value is *not* an
            // accessor; an escaping component keeps the obligation.
            Some(PropsReactivity::Enumerated { .. }) => PropUse::Static,
            None | Some(PropsReactivity::Escaping { .. }) => PropUse::Unknown,
        }
    }

    /// The worst classification across an enumerated set of prop names.
    /// Without caller proof every destructure keeps the upstream answer —
    /// including the empty pattern, which binds nothing but is still the
    /// shape the 1.x rule reports.
    pub(crate) fn names_use<'n>(
        &self,
        declaration: &Location,
        names: impl Iterator<Item = &'n str>,
    ) -> PropUse {
        if !self.caller_proof {
            return PropUse::Reactive;
        }
        names.fold(PropUse::Static, |worst, name| {
            worst.worst(self.prop_use(declaration, name))
        })
    }

    /// The per-symbol view the incremental cache fingerprints: the
    /// classification behind one props symbol's declaration, if any.
    pub(crate) fn for_declaration(&self, declaration: &Location) -> Option<&PropsReactivity> {
        self.by_declaration.get(declaration)
    }
}

/// Builds [`PropsReactivityIndex`] from the proven components' JSX call
/// sites. Empty (answering [`PropUse::Reactive`] everywhere) when the dialect
/// keeps the upstream over-approximation.
#[allow(clippy::too_many_arguments)]
fn classify_component_props(
    runtime: &crate::RuntimeEnvironment,
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
    source_kinds: &HashMap<SymbolId, ReactiveSourceKind>,
    prop_sources: &HashMap<SymbolId, (SymbolId, Location)>,
    runtime_sources: &HashSet<SymbolId>,
) -> PropsReactivityIndex {
    if !lookup.dialect.props_require_caller_proof() {
        return PropsReactivityIndex::default();
    }
    // Every JSX element resolved to the project function it renders.
    let mut uses = HashMap::<(&str, solid_facts::core::Span), Vec<(usize, usize)>>::new();
    for (file_index, file) in facts.files.iter().enumerate() {
        for (element_index, element) in file.ast.jsx_elements.iter().enumerate() {
            if let Some((target_file, target)) =
                lookup.function_called_at(file.path.as_str(), element.name.span)
            {
                uses.entry((target_file.path.as_str(), target.span))
                    .or_default()
                    .push((file_index, element_index));
            }
        }
    }
    let mut by_declaration = HashMap::new();
    let mut pending = Vec::new();
    for file in &facts.files {
        for function in &file.ast.functions {
            let Some(parameter) = function.parameters.first() else {
                continue;
            };
            if function.parameters.len() > 1 || !lookup.function_may_be_component(file, function) {
                continue;
            }
            let (classification, forwarded) = classify_one_component(
                runtime,
                facts,
                lookup,
                entities,
                accessors,
                source_kinds,
                prop_sources,
                runtime_sources,
                &uses,
                file,
                function,
            );
            pending.push((
                location(file.path.shared(), parameter.pattern),
                forwarded.clone(),
            ));
            if let Some(name) = parameter.names.first() {
                pending.push((location(file.path.shared(), name.span), forwarded));
            }
            by_declaration.insert(
                location(file.path.shared(), parameter.pattern),
                classification.clone(),
            );
            if let Some(name) = parameter.names.first() {
                by_declaration.insert(location(file.path.shared(), name.span), classification);
            }
        }
    }
    resolve_forwarded_props(&mut by_declaration, &pending);
    PropsReactivityIndex {
        caller_proof: true,
        by_declaration,
    }
}

/// What one component's classification says about the prop `name`.
fn classification_use(classification: &PropsReactivity, name: &str) -> PropUse {
    match classification {
        // Witnessed reactive survives the escape; nothing else does.
        PropsReactivity::Escaping { reactive, .. } => {
            if reactive.contains(name) {
                PropUse::Reactive
            } else {
                PropUse::Unknown
            }
        }
        PropsReactivity::Enumerated {
            reactive,
            unresolved,
            ..
        } => {
            if reactive.contains(name) {
                PropUse::Reactive
            } else if unresolved.contains(name) {
                PropUse::Unknown
            } else {
                PropUse::Static
            }
        }
    }
}

/// Settles forwarded props (`<Inner value={props.value} />`): the receiving
/// prop is reactive when the forwarded parent prop is, unresolved when that is
/// unresolved or unknown, and otherwise contributes nothing. This is the least
/// fixpoint from "contributes nothing", so a prop is proven static only when
/// no chain of forwards reaches a reactive or unresolved value. A forward into
/// a component whose own classification has no entry is unknown.
fn resolve_forwarded_props(
    by_declaration: &mut HashMap<Location, PropsReactivity>,
    pending: &[(Location, ForwardedProps)],
) {
    loop {
        let mut changed = false;
        for (declaration, forwarded) in pending {
            for (name, sources) in forwarded {
                let mut verdict = PropUse::Static;
                for (source, prop, exact) in sources {
                    let source_use = match by_declaration
                        .get(source)
                        .map_or(PropUse::Unknown, |classification| {
                            classification_use(classification, prop)
                        }) {
                        // A static head says nothing about the rest of a
                        // longer chain (`props.a.b` with `a` a store or an
                        // object with getters).
                        PropUse::Static if !exact => PropUse::Unknown,
                        other => other,
                    };
                    verdict = match (verdict, source_use) {
                        (PropUse::Reactive, _) | (_, PropUse::Reactive) => PropUse::Reactive,
                        (PropUse::Unknown, _) | (_, PropUse::Unknown) => PropUse::Unknown,
                        _ => PropUse::Static,
                    };
                }
                let Some(classification) = by_declaration.get_mut(declaration) else {
                    continue;
                };
                changed |= match (classification, verdict) {
                    (_, PropUse::Static) => false,
                    (
                        PropsReactivity::Enumerated { reactive, .. }
                        | PropsReactivity::Escaping { reactive, .. },
                        PropUse::Reactive,
                    ) => reactive.insert(name.clone()),
                    (
                        PropsReactivity::Enumerated {
                            unresolved,
                            reactive,
                            ..
                        },
                        PropUse::Unknown,
                    ) => !reactive.contains(name) && unresolved.insert(name.clone()),
                    (PropsReactivity::Escaping { .. }, PropUse::Unknown) => false,
                };
            }
        }
        if !changed {
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_one_component(
    runtime: &crate::RuntimeEnvironment,
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
    source_kinds: &HashMap<SymbolId, ReactiveSourceKind>,
    prop_sources: &HashMap<SymbolId, (SymbolId, Location)>,
    runtime_sources: &HashSet<SymbolId>,
    uses: &HashMap<(&str, solid_facts::core::Span), Vec<(usize, usize)>>,
    file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> (PropsReactivity, ForwardedProps) {
    use solid_facts::core::Span;
    let mut forwarded = ForwardedProps::new();
    let name = crate::owners::component_binding_name(file, function).or(function.name.as_ref());
    let Some(symbol) = name.and_then(|name| entities.get(&location(file.path.shared(), name.span)))
    else {
        // An anonymous component value (a HOC argument, say) has no symbol to
        // enumerate references through, so there are no call sites to witness
        // either way.
        return (
            PropsReactivity::Escaping {
                reactive: BTreeSet::new(),
                accessor_values: BTreeSet::new(),
            },
            forwarded,
        );
    };
    // Escape hatches below only forfeit the *static* half of the proof. Each
    // one sets this flag and keeps scanning, because the JSX call sites that
    // are visible still witness what they pass.
    let mut escapes = false;
    // Exported: callers outside the project can pass anything as well --
    // unless the user has asserted that there is no outside. That assertion
    // removes only the assumption of an *unseen* caller; every reference below
    // must still resolve to a use this analysis understands.
    let closed = runtime.program_is_closed();
    if !closed && component_symbol_is_exported(facts, entities, symbol) {
        escapes = true;
    }
    let empty = Vec::new();
    let component_uses = uses
        .get(&(file.path.as_str(), function.span))
        .unwrap_or(&empty);
    // Every reference must be the declaration itself or one of the resolved
    // JSX uses; anything else (passed as a value, aliased, re-exported) hands
    // the component to callers this cannot see. An empty reference list is
    // the absence of the fact, not proof of no references.
    let references = lookup.symbol_references(symbol.as_str());
    if references.is_empty() {
        escapes = true;
    }
    let name_text = name
        .and_then(|name| file.source_text(name.span))
        .unwrap_or_default();
    for reference in &references {
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            escapes = true;
            continue;
        };
        let span = Span::new(start, end);
        let own_declaration = *reference.path == *file.path.as_str()
            && (function.span.contains(span) || name.is_some_and(|name| name.span == span));
        if own_declaration {
            continue;
        }
        let jsx_use = component_uses.iter().any(|(file_index, element_index)| {
            let use_file = &facts.files[*file_index];
            let element = &use_file.ast.jsx_elements[*element_index];
            *reference.path == *use_file.path.as_str()
                && (element.name.span == span
                    || (element.span.contains(span)
                        && use_file.source_text(span) == Some(name_text)))
        });
        if !jsx_use {
            // Passed as a value, aliased, or re-exported: this hands the
            // component to callers that cannot be seen. It does not unwrite
            // the JSX call sites that can.
            //
            // An export specifier is the one reference a closed program
            // disposes of: `export { Card }` reaches an importer only if an
            // importer exists, and a closed program says none does outside the
            // files analyzed here -- where every importer's use is itself a
            // reference in this list. Aliasing and passing as a value still
            // escape under a closed program, because the analyzer still does
            // not know what the receiver does with the component.
            if !(closed && reference_is_export_specifier(facts, span, reference.path.as_ref())) {
                escapes = true;
            }
        }
    }
    let mut reactive = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut accessor_values = BTreeSet::new();
    for (file_index, element_index) in component_uses {
        let use_file = &facts.files[*file_index];
        let element = &use_file.ast.jsx_elements[*element_index];
        // A spread hands over an object whose properties this cannot
        // enumerate — and, because a later spread wins over an earlier
        // explicit attribute, it can also overwrite one of this element's own
        // attributes with a static value. So this element witnesses nothing
        // in either direction; other elements still do.
        if !element.spreads.is_empty() {
            escapes = true;
            continue;
        }
        for attribute in &element.attributes {
            let Some(attribute_name) = use_file.source_text(attribute.local_name) else {
                escapes = true;
                continue;
            };
            let value_use = if attribute.namespace.is_some() {
                PropUse::Unknown
            } else {
                match attribute.value_kind {
                    solid_facts::ast::JsxAttributeValueKind::Boolean
                    | solid_facts::ast::JsxAttributeValueKind::String => PropUse::Static,
                    solid_facts::ast::JsxAttributeValueKind::Element
                    | solid_facts::ast::JsxAttributeValueKind::Fragment => PropUse::Unknown,
                    solid_facts::ast::JsxAttributeValueKind::Expression => {
                        match attribute.expression {
                            Some(expression) => classify_passed_expression(
                                lookup,
                                entities,
                                accessors,
                                source_kinds,
                                prop_sources,
                                runtime_sources,
                                use_file,
                                expression,
                                forwarded.entry(attribute_name.to_owned()).or_default(),
                            ),
                            None => PropUse::Unknown,
                        }
                    }
                }
            };
            if attribute.namespace.is_none()
                && attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                && attribute.expression.is_some_and(|expression| {
                    passed_expression_is_accessor(lookup, entities, accessors, use_file, expression)
                })
            {
                accessor_values.insert(attribute_name.to_owned());
            }
            match value_use {
                PropUse::Static => {}
                PropUse::Reactive => {
                    reactive.insert(attribute_name.to_owned());
                }
                PropUse::Unknown => {
                    unresolved.insert(attribute_name.to_owned());
                }
            }
        }
        // Several children compile to an array whose dynamic entries are memo
        // accessors: reading `props.children` reads none of them. Only a
        // single child is the getter's own expression.
        let several = element
            .children
            .iter()
            .filter(|child| {
                use_file
                    .source_text(**child)
                    .is_some_and(|text| !text.trim().is_empty())
            })
            .count()
            > 1;
        for child in &element.children {
            let mut child_forwards = Vec::new();
            let child_use = classify_passed_expression(
                lookup,
                entities,
                accessors,
                source_kinds,
                prop_sources,
                runtime_sources,
                use_file,
                *child,
                &mut child_forwards,
            );
            if several {
                if child_use != PropUse::Static || !child_forwards.is_empty() {
                    unresolved.insert("children".to_owned());
                }
                continue;
            }
            forwarded
                .entry("children".to_owned())
                .or_default()
                .extend(child_forwards);
            match child_use {
                PropUse::Static => {}
                PropUse::Reactive => {
                    reactive.insert("children".to_owned());
                }
                PropUse::Unknown => {
                    unresolved.insert("children".to_owned());
                }
            }
        }
    }
    if escapes {
        // `unresolved` is dropped on purpose: without a complete caller set
        // every prop that is not a proven reactive witness is unresolved
        // anyway, so recording the distinction would only invite a reader to
        // treat the complement as static.
        return (
            PropsReactivity::Escaping {
                reactive,
                accessor_values,
            },
            forwarded,
        );
    }
    (
        PropsReactivity::Enumerated {
            reactive,
            unresolved,
            accessor_values,
        },
        forwarded,
    )
}

/// Per prop name, the parent props a call site forwards into it
/// (`<Inner value={props.value} />` inside `Outer`): the parent's props
/// declaration and the prop name read there.
type ForwardedProps = BTreeMap<String, Vec<Forward>>;

/// One forwarded parent prop: the parent's props declaration, the prop name
/// at the head of the forwarded chain, and whether the chain is exactly that
/// one member (`props.a`, not `props.a.b`).
type Forward = (Location, String, bool);

fn passed_expression_is_accessor(
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
    file: &FileFacts,
    expression: solid_facts::core::Span,
) -> bool {
    let expression = file.ast.peel_ts_sugar_span(expression);
    entities
        .get(&location(file.path.shared(), expression))
        .cloned()
        .or_else(|| {
            lookup
                .binding_at_reference(file.path.as_str(), expression)
                .map(|(_, _, symbol)| symbol)
        })
        .is_some_and(|symbol| accessors.contains_key(&symbol))
}

/// Whether a reference span is an export specifier's local name in the file it
/// appears in. Only an exact specifier span counts; a re-export with a module
/// clause is a different fact and is not one of these.
fn reference_is_export_specifier(
    facts: &ProjectFacts,
    span: solid_facts::core::Span,
    path: &str,
) -> bool {
    facts
        .files
        .iter()
        .filter(|file| file.path.as_str() == path)
        .any(|file| {
            file.ast.exports.iter().any(|export| {
                export.module.is_none()
                    && export
                        .specifiers
                        .iter()
                        .chain(export.declarations.iter())
                        .any(|specifier| specifier.local.span == span)
            })
        })
}

/// Whether the component's canonical symbol is exported from any analyzed
/// file — by declaration (`export function Card`), by specifier
/// (`export { Card }`), or as a default export.
fn component_symbol_is_exported(
    facts: &ProjectFacts,
    entities: &EntitySymbols,
    symbol: &SymbolId,
) -> bool {
    facts.files.iter().any(|file| {
        file.ast.exports.iter().any(|export| {
            export
                .specifiers
                .iter()
                .chain(export.declarations.iter())
                .filter(|specifier| !specifier.type_only)
                .any(|specifier| {
                    entities.get(&location(file.path.shared(), specifier.local.span))
                        == Some(symbol)
                })
        })
    })
}

/// Classifies one expression a call site passes for a prop (or renders as a
/// child).
///
/// Reactive proof: a call to a proven accessor, a member chain rooted at a
/// proven store or props object, or a store/props object passed whole — the
/// compiled getter re-evaluates the expression per read, so those subscribe.
/// Static proof: everything in the expression resolves to values that are not
/// reactive (bindings, function values — including nested function literals,
/// whose bodies run later, not in the getter). Anything unresolvable, any
/// call the engine cannot classify, and JSX evaluated inside the getter stay
/// unknown.
#[allow(clippy::too_many_arguments)]
fn classify_passed_expression(
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
    source_kinds: &HashMap<SymbolId, ReactiveSourceKind>,
    prop_sources: &HashMap<SymbolId, (SymbolId, Location)>,
    runtime_sources: &HashSet<SymbolId>,
    file: &FileFacts,
    expression: solid_facts::core::Span,
    forwards: &mut Vec<Forward>,
) -> PropUse {
    use solid_facts::core::Span;
    let expression = file.ast.peel_ts_sugar_span(expression);
    // Identifiers that are a member chain's root; the chain decides them.
    let mut chain_roots = Vec::new();
    let nested_functions: Vec<Span> = file
        .ast
        .functions_within(expression)
        .map(|function| function.span)
        .collect();
    let inside_nested = |span: Span| {
        nested_functions
            .iter()
            .any(|function| function.contains(span))
    };
    let mut result = PropUse::Static;
    // Member chains rooted at a proven store or props object are reactive;
    // rooted at anything resolvable they are static; unresolvable roots are
    // unknown.
    for member in &file.ast.members {
        if !expression.contains(member.span)
            || inside_nested(member.span)
            // Only chain roots: prefixes repeat the same root.
            || file
                .ast
                .members
                .iter()
                .any(|candidate| expression.contains(candidate.span) && candidate.object == member.span)
        {
            continue;
        }
        let mut root = member.object;
        let mut head = member;
        while let Some(inner) = file
            .ast
            .members
            .iter()
            .find(|candidate| candidate.span == file.ast.peel_ts_sugar_span(root))
        {
            head = inner;
            root = inner.object;
        }
        let root = file.ast.peel_ts_sugar_span(root);
        chain_roots.push(root);
        let symbol = entities
            .get(&location(file.path.shared(), root))
            .cloned()
            .or_else(|| {
                // The binder fallback covers plain locals entity facts skip.
                lookup
                    .binding_at_reference(file.path.as_str(), root)
                    .map(|(_, _, symbol)| symbol)
            });
        match symbol {
            Some(symbol) if source_kinds.get(&symbol) == Some(&ReactiveSourceKind::Store) => {
                let key = if file.ast.computed_members.binary_search(&head.span).is_ok() {
                    file.ast
                        .literal_computed_members
                        .iter()
                        .find(|literal| literal.span == head.span)
                        .map(|literal| literal.key.as_ref())
                } else {
                    file.source_text(head.property)
                };
                if runtime_sources.contains(&symbol)
                    && key.is_some_and(|key| lookup.dialect.store_key_warns_strict_read(key))
                {
                    return PropUse::Reactive;
                }
                result = PropUse::Unknown;
            }
            // A parent's prop forwarded is exactly as reactive as that prop is
            // in the parent: decided once every component is classified.
            Some(symbol) if prop_sources.contains_key(&symbol) => {
                let (_, declaration) = &prop_sources[&symbol];
                // Only the props parameter itself: a `merge` result or a
                // destructured binding attached to it is a view whose keys
                // this does not map back to the parent's props.
                if entities.get(declaration) != Some(&symbol)
                    || !crate::local_access::props_root_is_current(
                        file,
                        declaration,
                        &symbol,
                        entities,
                    )
                    || prop_sources
                        .iter()
                        .any(|(other, (_, attached))| other != &symbol && attached == declaration)
                {
                    result = PropUse::Unknown;
                    continue;
                }
                let exact = head.span == member.span;
                let name = if file.ast.computed_members.binary_search(&head.span).is_ok() {
                    file.ast
                        .literal_computed_members
                        .iter()
                        .find(|literal| literal.span == head.span)
                        .map(|literal| literal.key.to_string())
                } else {
                    file.source_text(head.property).map(str::to_owned)
                };
                match name {
                    Some(name) => forwards.push((declaration.clone(), name, exact)),
                    None => result = PropUse::Unknown,
                }
            }
            Some(_) => {}
            None => result = PropUse::Unknown,
        }
    }
    for call in file.ast.calls_within(expression) {
        if inside_nested(call.span) {
            continue;
        }
        let callee = entities.get(&location(file.path.shared(), call.callee));
        if callee.is_some_and(|symbol| {
            accessors.contains_key(symbol) && runtime_sources.contains(symbol)
        }) {
            return PropUse::Reactive;
        }
        // Any other call may read reactive state each time the compiled
        // getter re-evaluates: not provable either way.
        result = PropUse::Unknown;
    }
    if file
        .ast
        .jsx_within(expression)
        .any(|element| !inside_nested(element.span))
        || file
            .ast
            .jsx_fragments
            .iter()
            .any(|fragment| expression.contains(*fragment) && !inside_nested(*fragment))
    {
        result = PropUse::Unknown;
    }
    for identifier in file.ast.identifiers_within(expression) {
        if identifier.role != solid_facts::ast::IdentifierRole::Reference
            || inside_nested(identifier.span)
            || chain_roots.contains(&identifier.span)
        {
            continue;
        }
        let symbol = entities
            .get(&location(file.path.shared(), identifier.span))
            .cloned()
            .or_else(|| {
                lookup
                    .binding_at_reference(file.path.as_str(), identifier.span)
                    .map(|(_, _, symbol)| symbol)
            });
        match symbol {
            // A store passed whole is a reference: reading the prop hands over
            // the proxy and reads no key. The receiver's own key reads are
            // what is reactive, and this classification does not follow them.
            Some(symbol) if source_kinds.get(&symbol) == Some(&ReactiveSourceKind::Store) => {
                result = PropUse::Unknown;
            }
            // A props object passed whole forwards every prop; which ones the
            // receiver reads, and whether they are live, is not followed.
            Some(symbol) if prop_sources.contains_key(&symbol) => result = PropUse::Unknown,
            Some(_) => {}
            None => result = PropUse::Unknown,
        }
    }
    result
}
