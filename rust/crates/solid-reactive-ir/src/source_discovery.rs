//! The source-discovery stage: finds reactive sources, accessors,
//! setters, and contract-backed facts per file, with per-file reuse.

use crate::*;

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use crate::contracts::ResolvedContracts;
use crate::identity::{SymbolId, symbol_id};
use crate::indexes::{CachedAstFileIndex, EntitySymbols, ProjectIndexes, SemanticLookup};
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
            if let Some(name) = binding.names.first() {
                let declaration = location(file.path.shared(), name.span);
                if let Some(symbol) = entities.get(&declaration) {
                    result.accessors.push((
                        symbol.clone(),
                        (
                            symbol_id(file.source_text(name.span).unwrap_or_default()),
                            contracted.contract_location.clone(),
                        ),
                    ));
                    result.contracted_accessor_symbols.push(symbol.clone());
                    result.accessor_origins.push((
                        symbol.clone(),
                        (
                            symbol_id(&contracted_return.label),
                            symbol_id(&contracted.local_name),
                            contracted.contract_location.clone(),
                        ),
                    ));
                    result.source_kinds.push((
                        symbol.clone(),
                        if contracted_return.kind == "store-path" {
                            ReactiveSourceKind::Store
                        } else {
                            ReactiveSourceKind::Accessor
                        },
                    ));
                }
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
                    if call
                        .arguments
                        .first()
                        .is_some_and(|argument| computation_is_async(lookup, file, argument.span))
                    {
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
                        Some(Primitive::CreateSignal | Primitive::CreateStore)
                    )
                    && go_binding_pattern_accepts_call(file.source.as_ref(), binding, call);
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
                            Location {
                                path: format!(
                                    "bundled://{}#{primitive}",
                                    lookup.dialect.bundled_contract_label()
                                )
                                .into(),
                                start_byte: 0,
                                end_byte: 0,
                            },
                        ),
                    ));
                }
                result.source_kinds.push((
                    symbol.clone(),
                    if primitive
                        .as_deref()
                        .and_then(|primitive| bundled_returns.get(primitive))
                        .is_some_and(|returned| returned.kind == "store-path")
                        || matches!(
                            resolved,
                            Some(primitive) if lookup.dialect.returns_store(primitive)
                        )
                    {
                        ReactiveSourceKind::Store
                    } else {
                        ReactiveSourceKind::Accessor
                    },
                ));
                if let Some(primitive) = primitive.as_deref() {
                    result
                        .source_primitives
                        .push((symbol.clone(), primitive.into()));
                }
                result
                    .source_owned_write
                    .push((symbol.clone(), call.owned_write_option));
                if call
                    .arguments
                    .first()
                    .is_some_and(|argument| computation_is_async(lookup, file, argument.span))
                {
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
                    ),
                ));
            }
        }
    }
    result
}

pub(crate) struct SourceDiscoveryMergeTarget<'a> {
    pub(crate) accessors: &'a mut HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) accessor_origins: &'a mut HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: &'a mut HashMap<SymbolId, (SymbolId, Location, bool)>,
    pub(crate) actions: &'a mut HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_kinds: &'a mut HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) source_primitives: &'a mut HashMap<SymbolId, SymbolId>,
    pub(crate) source_phases: &'a mut HashMap<SymbolId, u8>,
    pub(crate) returned_source_symbols: &'a mut HashSet<SymbolId>,
    pub(crate) summary_source_symbols: &'a mut HashSet<SymbolId>,
    pub(crate) source_owned_write: &'a mut HashMap<SymbolId, bool>,
    pub(crate) async_sources: &'a mut HashSet<SymbolId>,
    pub(crate) contracted_accessor_symbols: &'a mut HashSet<SymbolId>,
}

#[derive(Default)]
pub(crate) struct SourceDiscoveryAggregate {
    pub(crate) accessors: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) accessor_origins: HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: HashMap<SymbolId, (SymbolId, Location, bool)>,
    pub(crate) actions: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_kinds: HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) source_primitives: HashMap<SymbolId, SymbolId>,
    pub(crate) source_phases: HashMap<SymbolId, u8>,
    pub(crate) returned_source_symbols: HashSet<SymbolId>,
    pub(crate) summary_source_symbols: HashSet<SymbolId>,
    pub(crate) source_owned_write: HashMap<SymbolId, bool>,
    pub(crate) async_sources: HashSet<SymbolId>,
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
}

/// Owned reactive-source facts produced by the source-discovery stage and
/// consumed by the later interprocedural, static, and owner stages.
pub(crate) struct SourceDiscovery {
    pub(crate) accessors: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) accessor_origins: HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: HashMap<SymbolId, (SymbolId, Location, bool)>,
    pub(crate) actions: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_kinds: HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) source_primitives: HashMap<SymbolId, SymbolId>,
    pub(crate) source_phases: HashMap<SymbolId, u8>,
    pub(crate) returned_source_symbols: HashSet<SymbolId>,
    pub(crate) summary_source_symbols: HashSet<SymbolId>,
    pub(crate) source_owned_write: HashMap<SymbolId, bool>,
    pub(crate) async_sources: HashSet<SymbolId>,
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

// The final `finish_stage!` resets the stage timer for symmetry; that last write
// is intentionally unused because the stage ends here.
#[allow(unused_assignments)]
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
    let mut stage_started = Instant::now();
    macro_rules! finish_stage {
        ($field:ident, $name:literal) => {{
            let elapsed = stage_started.elapsed();
            build_timings.$field = elapsed;
            if emit_timings {
                eprintln!(
                    "{{\"reactiveIrStage\":\"{}\",\"elapsedNs\":{}}}",
                    $name,
                    elapsed.as_nanos()
                );
            }
            stage_started = Instant::now();
        }};
    }
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
    let mut setters = HashMap::<SymbolId, (SymbolId, Location, bool)>::new();
    let mut actions = HashMap::<SymbolId, (SymbolId, Location)>::new();
    let mut source_kinds = HashMap::<SymbolId, ReactiveSourceKind>::new();
    let mut source_primitives = HashMap::<SymbolId, SymbolId>::new();
    let mut source_phases = HashMap::<SymbolId, u8>::new();
    let mut returned_source_symbols = HashSet::<SymbolId>::new();
    let mut summary_source_symbols = HashSet::<SymbolId>::new();
    let mut source_owned_write = HashMap::<SymbolId, bool>::new();
    let mut async_sources = HashSet::<SymbolId>::new();
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
            for file in &facts.files {
                for binding in &file.ast.bindings {
                    let Some(initializer) = binding.call_initializer else {
                        continue;
                    };
                    let Some(call) = project_indexes
                        .ast_files_by_path
                        .get(file.path.as_str())
                        .and_then(|index| index.call_by_span(initializer))
                    else {
                        continue;
                    };
                    let contracted = entities
                        .get(&location(file.path.shared(), call.callee))
                        .and_then(|symbol| resolved_contracts.by_symbol.get(symbol));
                    if let Some(contracted) = contracted
                        && let Some(contracted_return) = contracted.summary.returns.as_ref()
                    {
                        let source_name = binding.names.first();
                        if let Some(name) = source_name {
                            let declaration = location(file.path.shared(), name.span);
                            if let Some(symbol) = entities.get(&declaration) {
                                accessors.insert(
                                    symbol.clone(),
                                    (
                                        symbol_id(file.source_text(name.span).unwrap_or_default()),
                                        contracted.contract_location.clone(),
                                    ),
                                );
                                contracted_accessor_symbols.insert(symbol.clone());
                                accessor_origins.insert(
                                    symbol.clone(),
                                    (
                                        symbol_id(&contracted_return.label),
                                        symbol_id(&contracted.local_name),
                                        contracted.contract_location.clone(),
                                    ),
                                );
                                source_kinds.insert(
                                    symbol.clone(),
                                    if contracted_return.kind == "store-path" {
                                        ReactiveSourceKind::Store
                                    } else {
                                        ReactiveSourceKind::Accessor
                                    },
                                );
                            }
                        }
                        continue;
                    }
                    let primitive = primitive_name(
                        file.path.as_str(),
                        call.callee,
                        call.static_callee(&file.source),
                        entities,
                        symbol_names,
                        semantic_lookup.dialect,
                    );
                    let resolved = known_primitive(&primitive);
                    if resolved == Some(Primitive::Action) {
                        if let Some(name) = binding.names.first() {
                            let location = location(file.path.shared(), name.span);
                            if let Some(symbol) = entities.get(&location) {
                                actions.insert(
                                    symbol.clone(),
                                    (
                                        symbol_id(file.source_text(name.span).unwrap_or_default()),
                                        location,
                                    ),
                                );
                            }
                        }
                        continue;
                    }
                    if resolved == Some(Primitive::Dynamic) {
                        if let Some(name) = binding.names.first() {
                            let declaration = location(file.path.shared(), name.span);
                            if let Some(symbol) = entities.get(&declaration) {
                                source_primitives.insert(symbol.clone(), "dynamic".into());
                                if call.arguments.first().is_some_and(|argument| {
                                    computation_is_async(semantic_lookup, file, argument.span)
                                }) {
                                    async_sources.insert(symbol.clone());
                                }
                            }
                        }
                        continue;
                    }
                    if !matches!(
                        resolved,
                        Some(primitive)
                            if semantic_lookup.dialect.creates_reactive_source(primitive)
                    ) && !primitive
                        .as_deref()
                        .is_some_and(|primitive| bundled_returns.contains_key(primitive))
                    {
                        continue;
                    }
                    let source_name = if binding.shape == solid_facts::ast::BindingShape::Array {
                        binding.array_slots.first().and_then(Option::as_ref)
                    } else {
                        binding.names.first()
                    };
                    if let Some(name) = source_name {
                        let declaration = location(file.path.shared(), name.span);
                        if let Some(symbol) = entities.get(&declaration) {
                            accessors.insert(
                                symbol.clone(),
                                (
                                    symbol_id(file.source_text(name.span).unwrap_or_default()),
                                    declaration,
                                ),
                            );
                            let go_returned_source = binding.shape
                                == solid_facts::ast::BindingShape::Array
                                && matches!(
                                    resolved,
                                    Some(Primitive::CreateSignal | Primitive::CreateStore)
                                )
                                && go_binding_pattern_accepts_call(
                                    file.source.as_ref(),
                                    binding,
                                    call,
                                );
                            source_phases.insert(
                                symbol.clone(),
                                if go_returned_source && resolved == Some(Primitive::CreateStore) {
                                    2
                                } else if go_returned_source {
                                    0
                                } else {
                                    1
                                },
                            );
                            if go_returned_source {
                                returned_source_symbols.insert(symbol.clone());
                                summary_source_symbols.insert(symbol.clone());
                            }
                            if binding.shape != solid_facts::ast::BindingShape::Array
                                && primitive.as_deref().is_some_and(|primitive| {
                                    bundled_returns.contains_key(primitive)
                                })
                            {
                                summary_source_symbols.insert(symbol.clone());
                            }
                            if let Some(primitive) = primitive.as_deref()
                                && let Some(returned) = bundled_returns.get(primitive)
                            {
                                accessor_origins.insert(
                                    symbol.clone(),
                                    (
                                        symbol_id(&returned.label),
                                        primitive.into(),
                                        Location {
                                            path: format!(
                                                "bundled://{}#{primitive}",
                                                semantic_lookup.dialect.bundled_contract_label()
                                            )
                                            .into(),
                                            start_byte: 0,
                                            end_byte: 0,
                                        },
                                    ),
                                );
                            }
                            source_kinds.insert(
                                symbol.clone(),
                                if primitive
                                    .as_deref()
                                    .and_then(|primitive| bundled_returns.get(primitive))
                                    .is_some_and(|returned| returned.kind == "store-path")
                                    || matches!(
                                        resolved,
                                        Some(primitive)
                                            if semantic_lookup.dialect.returns_store(primitive)
                                    )
                                {
                                    ReactiveSourceKind::Store
                                } else {
                                    ReactiveSourceKind::Accessor
                                },
                            );
                            if let Some(primitive) = primitive.as_deref() {
                                source_primitives.insert(symbol.clone(), primitive.into());
                            }
                            source_owned_write.insert(symbol.clone(), call.owned_write_option);
                            if call.arguments.first().is_some_and(|argument| {
                                computation_is_async(semantic_lookup, file, argument.span)
                            }) {
                                async_sources.insert(symbol.clone());
                            }
                        }
                    }
                    if resolved != Some(Primitive::CreateMemo)
                        && let Some(name) =
                            if binding.shape == solid_facts::ast::BindingShape::Array {
                                binding.array_slots.get(1).and_then(Option::as_ref)
                            } else {
                                binding.names.get(1)
                            }
                    {
                        let declaration = location(file.path.shared(), name.span);
                        if let Some(symbol) = entities.get(&declaration) {
                            setters.insert(
                                symbol.clone(),
                                (
                                    symbol_id(file.source_text(name.span).unwrap_or_default()),
                                    declaration,
                                    call.owned_write_option,
                                ),
                            );
                        }
                    }
                }
            }
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
                            )
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
                    contracted_accessor_symbols: &mut contracted_accessor_symbols,
                });
            }
        }
    }
    finish_stage!(source_discovery, "source-discovery");
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
        let declaration = source_declarations.get(symbol);
        let (name, local_location) = declaration.map_or_else(
            || ("accessor".into(), entity.location.clone()),
            |declaration| (declaration.name.clone(), declaration.location.clone()),
        );
        let declaration_location = descriptor
            .alias_declarations
            .iter()
            .find(|declaration| matches!(declaration.name.as_ref(), "Accessor" | "Setter"))
            .map_or(local_location, |declaration| declaration.location.clone());
        accessors
            .entry(symbol.clone())
            .or_insert((symbol_id(name.as_ref()), declaration_location));
        source_phases.entry(symbol.clone()).or_insert(1);
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
                    .callback_accessor_parameters(primitive, argument_index);
                if parameter_indices.is_empty() {
                    continue;
                }
                let Some(function) = file
                    .ast
                    .functions
                    .iter()
                    .find(|function| function.span == argument.span)
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
                    if let Some((_, source, owned_write)) = &setter
                        && !setters.contains_key(symbol)
                    {
                        setter_aliases.push((
                            symbol.clone(),
                            (
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                source.clone(),
                                *owned_write,
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
            if !function_binding_name(file, function)
                .and_then(|name| {
                    file.source_text(name.span)
                        .unwrap_or_default()
                        .chars()
                        .next()
                })
                .is_some_and(char::is_uppercase)
            {
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
    finish_stage!(
        typed_accessors_and_prop_roots,
        "typed-accessors-and-prop-roots"
    );
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
    finish_stage!(
        prop_propagation_and_control_flow,
        "prop-propagation-and-control-flow"
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
