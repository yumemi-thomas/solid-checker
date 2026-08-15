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
    BuildTimings, ContractCallback, ContractReturn, PackageContract, PrimitiveName,
    ReactiveSourceKind, jsx_primitive_name, known_primitive, location, primitive_name,
};

use std::collections::{HashMap, HashSet};

use crate::contracts::ResolvedContracts;
use crate::identity::{SymbolId, symbol_id};
use crate::indexes::{CachedAstFileIndex, EntitySymbols, ProjectIndexes, SemanticLookup};
use crate::timings::{ReactiveIrStage, StageClock};
use solid_dialect::{Dialect, Primitive};
use solid_facts::core::{SourceHash, SourcePath};
use solid_facts::{FileFacts, ProjectFacts};
use typefacts::{Declaration, Location, ResolvedCallValidity};

/// Whether a caller-supplied `solid-js` contract is the artifact whose name the
/// dialect stamps into provenance.
///
/// Every return kind `bundled_returns` contributes is cited as
/// `bundled://<dialect label>#<primitive>`. The two majors answer different
/// questions under the same export names, so reading one contract while citing
/// the other's filename attributes a fact to a file that was never consulted.
/// Two discriminators, in order of strength: the backend stamps its own
/// embedded artifacts' `source_path`, and both those artifacts (like every real
/// install of the package) carry a semver `package.version`. A `solid-js`
/// contract with neither proves nothing about which dialect it describes and is
/// therefore not read here -- dropping a fact is a silent gap, citing the wrong
/// file is a false claim.
pub(crate) fn bundled_contract_matches_dialect(
    contract: &PackageContract,
    dialect: &dyn Dialect,
) -> bool {
    if let Some(label) = contract.source_path.strip_prefix("bundled://") {
        return label == dialect.bundled_contract_label();
    }
    solid_dialect::Version::for_solid_js(&contract.package.version) == Some(dialect.version())
}

/// The provenance stamped on facts read from a bundled contract:
/// `bundled://<dialect label>#<primitive>`, carrying no span of its own.
pub(crate) fn bundled_contract_location(dialect: &dyn Dialect, primitive: &str) -> Location {
    Location {
        path: format!("bundled://{}#{primitive}", dialect.bundled_contract_label()).into(),
        start_byte: 0,
        end_byte: 0,
    }
}

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

struct EffectiveReturnContext<'a> {
    file: &'a FileFacts,
    ast_index: &'a CachedAstFileIndex,
    entities: &'a EntitySymbols,
    symbol_names: &'a HashMap<SymbolId, SymbolId>,
    resolved_contracts: &'a ResolvedContracts,
    bundled_returns: &'a HashMap<SymbolId, ContractReturn>,
    dialect: &'a dyn Dialect,
}

fn effective_call_return(
    returned: &ContractReturn,
    call: &solid_facts::ast::CallFact,
    context: &EffectiveReturnContext<'_>,
    depth: usize,
) -> Option<ContractReturn> {
    if returned.kind != "argument" {
        return Some(returned.clone());
    }
    if depth == 0 {
        return None;
    }
    let argument = call.arguments.get(returned.parameter?)?;
    let inner = context.ast_index.call_by_span(argument.span).or_else(|| {
        context
            .file
            .ast
            .calls
            .iter()
            .filter(|candidate| argument.span.contains(candidate.span))
            .max_by_key(|candidate| candidate.span.end - candidate.span.start)
    })?;
    if let Some(contracted) = context
        .entities
        .at(context.file.path.as_str(), inner.callee)
        .and_then(|symbol| context.resolved_contracts.by_symbol.get(symbol))
        .and_then(|contracted| contracted.summary.returns.as_ref())
    {
        return effective_call_return(contracted, inner, context, depth - 1);
    }
    let primitive = primitive_name(
        context.file.path.as_str(),
        inner.callee,
        inner.static_callee(&context.file.source),
        context.entities,
        context.symbol_names,
        context.dialect,
    );
    if let Some(returned) = primitive
        .as_deref()
        .and_then(|primitive| context.bundled_returns.get(primitive))
    {
        return Some(returned.clone());
    }
    let primitive = known_primitive(&primitive)?;
    let kind = if context.dialect.returns_store(primitive) {
        "store-path"
    } else {
        "accessor"
    };
    if matches!(
        primitive,
        Primitive::CreateSignal | Primitive::CreateStore | Primitive::CreateResource
    ) {
        return Some(ContractReturn {
            kind: "tuple".into(),
            elements: vec![
                Some(ContractReturn {
                    kind: kind.into(),
                    label: "wrapped reactive value".into(),
                    ..ContractReturn::default()
                }),
                None,
            ],
            ..ContractReturn::default()
        });
    }
    context
        .dialect
        .creates_reactive_source(primitive)
        .then(|| ContractReturn {
            kind: kind.into(),
            label: "wrapped reactive value".into(),
            ..ContractReturn::default()
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
        opaque: !argument.exact_object_literal && !declared_loading,
    }
}

/// The `@solidjs/web` exports whose import proves the project server-renders
/// (or hydrates server-rendered HTML). A bare `ssrSource: "client"` source is
/// only a runtime error on the server path, so SC5005 stays silent for
/// CSR-only projects. Named imports only; the export names come from the
/// bundled `@solidjs/web` contract.
const SERVER_RENDER_IMPORTS: [&str; 6] = [
    "renderToStream",
    "renderToString",
    "renderToFrameStream",
    "renderServerComponent",
    "handleServerFunctionRequest",
    "hydrate",
];

/// Whether any analyzed file imports a server rendering entry point from
/// `@solidjs/web` (or one of its subpaths).
pub(crate) fn project_server_renders(facts: &ProjectFacts) -> bool {
    facts.files.iter().any(|file| {
        file.ast.imports.iter().any(|import| {
            (import.module == "@solidjs/web" || import.module.starts_with("@solidjs/web/"))
                && !import.type_only
                && import.bindings.iter().any(|binding| {
                    !binding.type_only
                        && binding.imported.as_deref().is_some_and(|imported| {
                            SERVER_RENDER_IMPORTS.contains(&imported)
                        })
                })
        })
    })
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
fn store_is_value_form(
    call: &solid_facts::ast::CallFact,
    primitive: Option<Primitive>,
) -> bool {
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
        ) || argument.primitive_literal
            || argument.container_literal
    })
}

pub(crate) fn discover_file_sources(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    ast_index: &CachedAstFileIndex,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    resolved_contracts: &ResolvedContracts,
    bundled_returns: &HashMap<SymbolId, ContractReturn>,
) -> SourceDiscoveryContribution {
    let mut result = SourceDiscoveryContribution::default();
    for binding in &file.ast.bindings {
        let Some(initializer) = binding.call_initializer else {
            continue;
        };
        let Some(call) = ast_index.call_by_span(initializer) else {
            continue;
        };
        let contracted = entities
            .get(&location(file.path.shared(), call.callee))
            .and_then(|symbol| resolved_contracts.by_symbol.get(symbol));
        if let Some(contracted) = contracted
            && let Some(contracted_return) = contracted.summary.returns.as_ref()
        {
            let context = EffectiveReturnContext {
                file,
                ast_index,
                entities,
                symbol_names,
                resolved_contracts,
                bundled_returns,
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
        let primitive = primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
            lookup.dialect,
        );
        let resolved = known_primitive(&primitive);
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
        ) && !primitive
            .as_deref()
            .is_some_and(|primitive| bundled_returns.contains_key(primitive))
        {
            continue;
        }
        let source_kind = if primitive
            .as_deref()
            .and_then(|primitive| bundled_returns.get(primitive))
            .is_some_and(|returned| returned.kind == "store-path")
            || matches!(
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
                let go_returned_source = binding.shape == solid_facts::ast::BindingShape::Array
                    && matches!(
                        resolved,
                        Some(
                            Primitive::CreateSignal
                                | Primitive::CreateStore
                                | Primitive::CreateResource
                        )
                    )
                    && binding_returns_reactive_source(binding, call);
                result.source_phases.push((
                    symbol.clone(),
                    if go_returned_source && resolved == Some(Primitive::CreateStore) {
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
                if binding.shape != solid_facts::ast::BindingShape::Array
                    && primitive
                        .as_deref()
                        .is_some_and(|primitive| bundled_returns.contains_key(primitive))
                {
                    result.summary_source_symbols.push(symbol.clone());
                }
                if let Some(primitive) = primitive.as_deref()
                    && let Some(returned) = bundled_returns.get(primitive)
                {
                    result.accessor_origins.push((
                        symbol.clone(),
                        (
                            symbol_id(&returned.label),
                            primitive.into(),
                            bundled_contract_location(lookup.dialect, primitive),
                        ),
                    ));
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
            }
        }
    }
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
                .and_then(|contracted| contracted.summary.returns.as_ref())
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
        let primitive = primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
            lookup.dialect,
        );
        let resolved = known_primitive(&primitive);
        if !matches!(
            resolved,
            Some(Primitive::CreateSignal | Primitive::CreateStore | Primitive::CreateResource)
        ) {
            continue;
        }
        let Some(symbol) = symbol else {
            continue;
        };
        let declaration = location(file.path.shared(), name);
        let source_kind = if resolved == Some(Primitive::CreateStore) {
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
            if let Some(returned) = bundled_returns.get(primitive) {
                result.accessor_origins.push((
                    symbol.clone(),
                    (
                        symbol_id(&returned.label),
                        primitive.into(),
                        bundled_contract_location(lookup.dialect, primitive),
                    ),
                ));
            }
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
        let Some(contracted_return) = contracted.summary.returns.as_ref() else {
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
    pub(crate) contract_reads: HashMap<SymbolId, Vec<(String, String, Location, String)>>,
    pub(crate) contract_callbacks: HashMap<SymbolId, Vec<ContractCallback>>,
    pub(crate) contract_returns: HashMap<SymbolId, (ContractReturn, Location)>,
    pub(crate) contracted_accessor_symbols: HashSet<SymbolId>,
    pub(crate) prop_sources: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) bundled_returns: HashMap<SymbolId, ContractReturn>,
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
    pub(crate) contracts: &'a [PackageContract],
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
        contracts,
    } = *ctx;
    let mut clock = StageClock::new(emit_timings);
    let mut accessors = HashMap::<SymbolId, (SymbolId, Location)>::new();
    let bundled_returns = contracts
        .iter()
        .find(|contract| {
            contract.package.name == "solid-js"
                && bundled_contract_matches_dialect(contract, semantic_lookup.dialect)
        })
        .map(|contract| {
            contract
                .root_exports()
                .iter()
                .filter_map(|(name, summary)| {
                    summary
                        .returns
                        .clone()
                        .map(|returned| (symbol_id(name), returned))
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
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
    let mut contract_reads = HashMap::<SymbolId, Vec<(String, String, Location, String)>>::new();
    let mut contract_callbacks = HashMap::<SymbolId, Vec<ContractCallback>>::new();
    let mut contract_returns = HashMap::<SymbolId, (ContractReturn, Location)>::new();
    let mut contracted_accessor_symbols = HashSet::<SymbolId>::new();

    for contracted in &resolved_contracts.bindings {
        if !contracted.summary.reactive_reads.is_empty() {
            contract_reads.insert(
                contracted.symbol.clone(),
                contracted
                    .summary
                    .reactive_reads
                    .iter()
                    .map(|read| {
                        (
                            format!("{}.{}", contracted.package_name, contracted.imported_name),
                            contracted.local_name.clone(),
                            contracted.contract_location.clone(),
                            read.kind.clone(),
                        )
                    })
                    .collect(),
            );
        }
        contract_callbacks.insert(
            contracted.symbol.clone(),
            contracted.summary.callbacks.clone(),
        );
        if let Some(returned) = &contracted.summary.returns {
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
                    &bundled_returns,
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
                        &bundled_returns,
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
        if role == solid_dialect::TypeRole::Component {
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
            solid_dialect::TypeRole::Component => unreachable!(),
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
                    solid_dialect::KeyForm::CustomKey
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
            for function in file.ast.functions.iter().filter(|function| {
                element.span.contains(function.span)
                    && !file.ast.functions.iter().any(|outer| {
                        outer.span != function.span
                            && element.span.contains(outer.span)
                            && outer.span.contains(function.span)
                    })
            }) {
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
            let Some(primitive) = primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
                entities,
                symbol_names,
                semantic_lookup.dialect,
            )
            .as_ref()
            .and_then(PrimitiveName::primitive) else {
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
            if !primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
                entities,
                symbol_names,
                semantic_lookup.dialect,
            )
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                matches!(
                    primitive,
                    Primitive::CreateEffect | Primitive::CreateRenderEffect
                )
            }) {
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
                .filter(|symbol| {
                    source_kinds.get(*symbol) == Some(&ReactiveSourceKind::Store)
                        || matches!(
                            source_primitives.get(*symbol).map(SymbolId::as_str),
                            Some("createStore" | "createOptimisticStore")
                        )
                })
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
    for file in &facts.files {
        for function in &file.ast.functions {
            if !semantic_lookup.function_is_component(file, function) {
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
                    .and_then(|symbol| prop_sources.get(symbol))
                    .cloned()
                    .or_else(|| {
                        let initializer = binding.call_initializer?;
                        let call = file.ast.call_at(initializer)?;
                        let primitive = primitive_name(
                            file.path.as_str(),
                            call.callee,
                            call.static_callee(&file.source),
                            entities,
                            symbol_names,
                            semantic_lookup.dialect,
                        );
                        if known_primitive(&primitive) != Some(Primitive::Merge) {
                            return None;
                        }
                        call.arguments.iter().find_map(|argument| {
                            entities
                                .get(&location(file.path.shared(), argument.span))
                                .and_then(|symbol| prop_sources.get(symbol))
                                .cloned()
                        })
                    });
                let Some((_, declaration)) = source else {
                    continue;
                };
                for name in &binding.names {
                    let binding_location = location(file.path.shared(), name.span);
                    if let Some(symbol) = entities.get(&binding_location)
                        && !prop_sources.contains_key(symbol)
                    {
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
        contract_callbacks,
        contract_returns,
        contracted_accessor_symbols,
        prop_sources,
        bundled_returns,
        retained_source_paths,
        changed_source_symbols,
    }
}
