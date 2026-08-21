//! Package-contract resolution and export-summary construction.
//!
//! Resolves imported contract bindings to local symbols (`resolve_contract_imports`)
//! and turns the interprocedural summaries into the per-export contract artifacts
//! that a downstream package sees. Owns both the full and incremental summary
//! builds; the public contract data types stay in the crate root.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use solid_dialect::Dialect;
use solid_facts::ProjectFacts;
use typefacts::{Callability, Location, ReferenceSpace};

use super::{
    ContractCallback, ContractExport, ContractReactiveRead, ContractReturn, EntitySymbols,
    PackageContract, ReactiveSourceKind, RuntimeEnvironment, StaticDefect, StaticDefectKind,
    SummaryNode, SummaryRead, SummaryReads, SymbolId, location, location_order,
};
use crate::cache::{CachedContractExports, ContractExportFragment, ContractNodeKey};
use crate::pipeline::parallel_slice_results;
#[derive(Clone)]
pub(super) struct ResolvedContractBinding {
    pub(super) local_name: String,
    pub(super) imported_name: String,
    pub(super) package_name: String,
    pub(super) symbol: SymbolId,
    pub(super) runtime_identity: String,
    pub(super) contract_location: Location,
    pub(super) summary: ContractExport,
}

pub(super) struct ResolvedContracts {
    pub(super) bindings: Vec<ResolvedContractBinding>,
    pub(super) by_symbol: HashMap<SymbolId, ResolvedContractBinding>,
    pub(super) missing_exports: Vec<StaticDefect>,
}

fn runtime_identity_at(facts: &ProjectFacts, location: &Location) -> String {
    facts
        .typescript
        .entities()
        .find(|entity| entity.location == *location)
        .map_or_else(String::new, |entity| entity.runtime_identity.to_string())
}

fn source_name_at(facts: &ProjectFacts, location: &Location) -> String {
    facts
        .files
        .iter()
        .find(|file| file.path.as_str() == location.path.as_ref())
        .and_then(|file| {
            file.source_text(solid_facts::core::Span::new(
                u32::try_from(location.start_byte).ok()?,
                u32::try_from(location.end_byte).ok()?,
            ))
        })
        .unwrap_or_default()
        .to_owned()
}

fn push_runtime_identity_conflict(
    missing_exports: &mut Vec<StaticDefect>,
    location: &Location,
    seen_locations: &mut HashSet<(String, u64, u64)>,
) {
    let key = (
        location.path.to_string(),
        location.start_byte,
        location.end_byte,
    );
    if !seen_locations.insert(key) {
        return;
    }
    missing_exports.push(StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: "<runtime-identity-conflict>".into(),
            export: "<conflicting-contract-summaries>".into(),
            reexported: true,
        },
        location: location.clone(),
        analysis_context:
            "multiple exact package contracts describe the same runtime export differently".into(),
        fixes: vec![],
        uncertain: false,
    });
}

/// Joins package summaries through exact runtime identity after direct import
/// and explicit re-export discovery.
///
/// This is deliberately an O(entities + contracted-bindings) pass. It does
/// not resolve by spelling, scan every entity for every shorthand, or turn an
/// empty identity into project ownership. Export-star chains can participate
/// when TypeFacts exposes the same identity at a concrete binding; a missing
/// identity remains fail-closed.
fn join_runtime_identity_aliases(
    facts: &ProjectFacts,
    entities: &EntitySymbols,
    bindings: &mut Vec<ResolvedContractBinding>,
    by_symbol: &mut HashMap<SymbolId, ResolvedContractBinding>,
    missing_exports: &mut Vec<StaticDefect>,
) {
    let mut candidates = HashMap::<String, Vec<ResolvedContractBinding>>::new();
    for binding in bindings
        .iter()
        .filter(|binding| !binding.runtime_identity.is_empty())
    {
        let entries = candidates
            .entry(binding.runtime_identity.clone())
            .or_default();
        if !entries.iter().any(|existing| {
            existing.package_name == binding.package_name
                && existing.contract_location == binding.contract_location
                && existing.summary == binding.summary
        }) {
            entries.push(binding.clone());
        }
    }

    let mut index = HashMap::new();
    let mut conflicts = HashSet::new();
    for (identity, entries) in candidates {
        let Some(first) = entries.first().cloned() else {
            continue;
        };
        if entries
            .iter()
            .skip(1)
            .any(|entry| entry.package_name != first.package_name || entry.summary != first.summary)
        {
            conflicts.insert(identity);
            continue;
        }
        index.insert(identity, first);
    }

    let mut bound_symbols = by_symbol.keys().cloned().collect::<HashSet<_>>();
    let mut seen_locations = HashSet::new();
    for entity in facts
        .typescript
        .entities()
        .filter(|entity| !entity.runtime_identity.is_empty())
    {
        let Some(symbol) = entities.get(&entity.location).cloned() else {
            continue;
        };
        if conflicts.contains(entity.runtime_identity.as_ref()) {
            push_runtime_identity_conflict(missing_exports, &entity.location, &mut seen_locations);
            continue;
        }
        let Some(template) = index.get(entity.runtime_identity.as_ref()).cloned() else {
            continue;
        };
        if let Some(existing) = by_symbol.get(&symbol)
            && (existing.package_name != template.package_name
                || existing.summary != template.summary)
        {
            push_runtime_identity_conflict(missing_exports, &entity.location, &mut seen_locations);
            continue;
        }
        if !bound_symbols.insert(symbol.clone()) {
            continue;
        }
        let mut binding = template;
        binding.local_name = source_name_at(facts, &entity.location);
        binding.symbol = symbol.clone();
        binding.runtime_identity = entity.runtime_identity.to_string();
        by_symbol.insert(symbol, binding.clone());
        bindings.push(binding);
    }
}

/// Whether the dialect's own vocabulary outranks a package contract for a name
/// imported from `module`.
///
/// Solid's built-ins have richer native semantics than any cross-package
/// contract summary can express: ownership, async provenance, writes, and
/// cleanup phases. The reviewed contract stays as evidence and for export
/// completeness, but its coarse callbacks/returns must not be layered over
/// native facts.
///
/// The gate is the dialect's module-ownership answer, not the literal package
/// name `solid-js`. 1.x reaches `createStore` only through `solid-js/store` and
/// `Portal` only through `solid-js/web`; 2.0 moved the whole DOM surface to the
/// separate `@solidjs/web` package. Comparing package names gave the package
/// root native precedence and every other entrypoint the contract's coarse
/// answer, for the same primitives.
fn native_vocabulary_outranks_contract(
    dialect: &dyn Dialect,
    module: &str,
    imported: &str,
) -> bool {
    dialect.owns_module(module) && dialect.declares_primitive(imported)
}

fn push_environment_dependent_export(
    missing_exports: &mut Vec<StaticDefect>,
    module: &str,
    export: &str,
    reexported: bool,
    location: Location,
) {
    missing_exports.push(StaticDefect {
        kind: StaticDefectKind::PackageContractEnvironmentDependent {
            module: module.to_owned(),
            export: export.to_owned(),
            reexported,
        },
        location,
        analysis_context: String::new(),
        fixes: vec![],
        uncertain: false,
    });
}

fn selected_contract_export(
    contract: &PackageContract,
    module: &str,
    summary: ContractExport,
    environment: &RuntimeEnvironment,
) -> Option<ContractExport> {
    let suffix = module.strip_prefix(&contract.package.name)?;
    let entrypoint_name = if suffix.is_empty() {
        ".".to_owned()
    } else if suffix.starts_with('/') {
        format!(".{suffix}")
    } else {
        return None;
    };
    let entrypoint = contract.entrypoints.get(&entrypoint_name)?;
    if !entrypoint.conditions.is_empty()
        && !environment.matches_entrypoint_conditions(&entrypoint.conditions)
    {
        return None;
    }
    if summary.variants.is_empty() {
        return Some(summary);
    }
    let matching = summary
        .variants
        .iter()
        .filter(|variant| environment.matches_conditions(&variant.conditions))
        .collect::<Vec<_>>();
    (matching.len() == 1).then(|| *matching[0].summary.clone())
}

pub(super) fn resolve_contract_imports(
    facts: &ProjectFacts,
    contracts: &[PackageContract],
    entities: &EntitySymbols,
    dialect: &dyn Dialect,
    environment: &RuntimeEnvironment,
) -> ResolvedContracts {
    let mut bindings = Vec::new();
    let mut by_symbol = HashMap::new();
    let mut missing_exports = Vec::new();
    for file in &facts.files {
        for import in &file.ast.imports {
            if import.type_only {
                continue;
            }
            let Some(contract) = PackageContract::for_module(contracts, &import.module) else {
                continue;
            };
            for binding in &import.bindings {
                if binding.type_only {
                    continue;
                }
                if binding.kind == solid_facts::ast::ImportKind::Namespace {
                    let namespace_location = location(file.path.shared(), binding.local.span);
                    let Some(namespace_symbol) = entities.get(&namespace_location) else {
                        continue;
                    };
                    for member in file.ast.members.iter().filter(|member| {
                        file.ast
                            .computed_members
                            .binary_search(&member.span)
                            .is_err()
                            && entities.get(&location(file.path.shared(), member.object))
                                == Some(namespace_symbol)
                    }) {
                        let imported = file
                            .source_text(member.property)
                            .unwrap_or_default()
                            .to_owned();
                        let Some(summary) = contract
                            .exports_for_module(&import.module)
                            .and_then(|exports| exports.get(imported.as_str()))
                            .cloned()
                        else {
                            missing_exports.push(StaticDefect {
                                kind: StaticDefectKind::PackageContractExportMissing {
                                    module: import.module.to_string(),
                                    export: imported,
                                    reexported: false,
                                },
                                location: location(file.path.shared(), member.property),
                                analysis_context: String::new(),
                                fixes: vec![],
                                uncertain: false,
                            });
                            continue;
                        };
                        let member_location = location(file.path.shared(), member.property);
                        let Some(symbol) = entities.get(&member_location).cloned() else {
                            continue;
                        };
                        if native_vocabulary_outranks_contract(dialect, &import.module, &imported) {
                            continue;
                        }
                        let Some(summary) = selected_contract_export(
                            contract,
                            &import.module,
                            summary,
                            environment,
                        ) else {
                            push_environment_dependent_export(
                                &mut missing_exports,
                                &import.module,
                                &imported,
                                false,
                                member_location,
                            );
                            continue;
                        };
                        let resolved = ResolvedContractBinding {
                            local_name: imported.clone(),
                            imported_name: imported.clone(),
                            package_name: contract.package.name.clone(),
                            symbol: symbol.clone(),
                            runtime_identity: runtime_identity_at(facts, &member_location),
                            contract_location: Location {
                                path: format!("{}#{imported}", contract.source_path).into(),
                                start_byte: 0,
                                end_byte: 0,
                            },
                            summary,
                        };
                        // Native dialect facts are richer than the package
                        // schema, but the reviewed package contract remains
                        // the only semantic evidence for public Solid exports
                        // outside that native vocabulary. Apply the same
                        // precedence to namespace and named imports.
                        bindings.push(resolved.clone());
                        by_symbol.insert(symbol, resolved);
                    }
                    continue;
                }
                let Some(imported) = binding.imported.as_deref().or_else(|| {
                    (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
                }) else {
                    continue;
                };
                let binding_location = location(file.path.shared(), binding.local.span);
                let Some(symbol) = entities.get(&binding_location).cloned() else {
                    continue;
                };
                let Some(summary) = contract
                    .exports_for_module(&import.module)
                    .and_then(|exports| exports.get(imported))
                    .cloned()
                else {
                    let import_entity = facts.typescript.entities().find(|entity| {
                        entity.location.path == binding_location.path
                            && entity.location.start_byte == binding_location.start_byte
                            && entity.location.end_byte == binding_location.end_byte
                    });
                    let runtime_referenced = import_entity
                        .and_then(|entity| entity.reference_space)
                        .is_none_or(|space| {
                            matches!(space, ReferenceSpace::Value | ReferenceSpace::Both)
                        });
                    if !runtime_referenced {
                        // TypeScript reports no value-space reference for
                        // mixed imports such as `import { JSX, Portal }`.
                        // A type-only binding cannot consume runtime
                        // reactivity and therefore needs no export summary.
                        continue;
                    }
                    missing_exports.push(StaticDefect {
                        kind: StaticDefectKind::PackageContractExportMissing {
                            module: import.module.to_string(),
                            export: imported.to_owned(),
                            reexported: false,
                        },
                        location: binding_location,
                        analysis_context: String::new(),
                        fixes: vec![],
                        uncertain: false,
                    });
                    continue;
                };
                if native_vocabulary_outranks_contract(dialect, &import.module, imported) {
                    continue;
                }
                let Some(summary) =
                    selected_contract_export(contract, &import.module, summary, environment)
                else {
                    push_environment_dependent_export(
                        &mut missing_exports,
                        &import.module,
                        imported,
                        false,
                        binding_location,
                    );
                    continue;
                };
                let resolved = ResolvedContractBinding {
                    local_name: file
                        .source_text(binding.local.span)
                        .unwrap_or_default()
                        .to_owned(),
                    imported_name: imported.into(),
                    package_name: contract.package.name.clone(),
                    symbol: symbol.clone(),
                    runtime_identity: runtime_identity_at(facts, &binding_location),
                    contract_location: Location {
                        path: format!("{}#{imported}", contract.source_path).into(),
                        start_byte: 0,
                        end_byte: 0,
                    },
                    summary,
                };
                // Solid's built-ins have richer native semantics than their
                // cross-package contract summary (ownership, async
                // provenance, writes, and cleanup phases). Keep the bundled
                // contract as evidence and for export completeness, but do
                // not layer its coarse callbacks/returns over native facts.
                bindings.push(resolved.clone());
                by_symbol.insert(symbol, resolved);
            }
        }
        for export in &file.ast.exports {
            if export.type_only {
                continue;
            }
            let Some(module) = export.module.as_deref() else {
                continue;
            };
            let Some(contract) = PackageContract::for_module(contracts, module) else {
                continue;
            };
            for specifier in &export.specifiers {
                if specifier.type_only {
                    continue;
                }
                let imported = file.source_text(specifier.local.span).unwrap_or_default();
                let specifier_location = location(file.path.shared(), specifier.local.span);
                let Some(symbol) = entities.get(&specifier_location).cloned() else {
                    continue;
                };
                let Some(summary) = contract
                    .exports_for_module(module)
                    .and_then(|exports| exports.get(imported))
                    .cloned()
                else {
                    missing_exports.push(StaticDefect {
                        kind: StaticDefectKind::PackageContractExportMissing {
                            module: module.to_owned(),
                            export: imported.to_owned(),
                            reexported: true,
                        },
                        location: specifier_location,
                        analysis_context: String::new(),
                        fixes: vec![],
                        uncertain: false,
                    });
                    continue;
                };
                if native_vocabulary_outranks_contract(dialect, module, imported) {
                    continue;
                }
                let Some(summary) =
                    selected_contract_export(contract, module, summary, environment)
                else {
                    push_environment_dependent_export(
                        &mut missing_exports,
                        module,
                        imported,
                        true,
                        specifier_location,
                    );
                    continue;
                };
                let resolved = ResolvedContractBinding {
                    local_name: specifier.exported.to_string(),
                    imported_name: imported.to_owned(),
                    package_name: contract.package.name.clone(),
                    symbol: symbol.clone(),
                    runtime_identity: runtime_identity_at(facts, &specifier_location),
                    contract_location: Location {
                        path: format!("{}#{imported}", contract.source_path).into(),
                        start_byte: 0,
                        end_byte: 0,
                    },
                    summary,
                };
                bindings.push(resolved.clone());
                by_symbol.insert(symbol, resolved);
            }
        }
    }
    join_runtime_identity_aliases(
        facts,
        entities,
        &mut bindings,
        &mut by_symbol,
        &mut missing_exports,
    );
    ResolvedContracts {
        bindings,
        by_symbol,
        missing_exports,
    }
}

pub(super) struct ContractSemantics<'a> {
    pub(super) bundled_returns: &'a HashMap<SymbolId, ContractReturn>,
    pub(super) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(super) source_primitives: &'a HashMap<SymbolId, SymbolId>,
}

pub(super) struct ContractGraph<'a> {
    pub(super) nodes: &'a [SummaryNode],
    pub(super) nodes_by_path: &'a HashMap<String, Vec<usize>>,
    pub(super) by_symbol: &'a HashMap<SymbolId, usize>,
    pub(super) entities: &'a EntitySymbols,
}

pub(super) struct ContractAnalysis<'a> {
    pub(super) summaries: &'a [SummaryReads],
    pub(super) returned: &'a [SummaryReads],
    pub(super) structured_returns: &'a [Option<ContractReturn>],
    pub(super) callbacks: &'a [Vec<ContractCallback>],
    pub(super) semantics: ContractSemantics<'a>,
}

fn contract_export_function(
    node: &SummaryNode,
    summary: &SummaryReads,
    returned_summary: &SummaryReads,
    structured_return: Option<&ContractReturn>,
    callbacks: &[ContractCallback],
    semantics: &ContractSemantics<'_>,
) -> ContractExport {
    let mut seen_reactive_reads = HashSet::new();
    let reactive_reads = summary
        .iter()
        .filter_map(|read| {
            let reactive_read = ContractReactiveRead {
                kind: read.kind.clone().unwrap_or_else(|| "accessor".into()),
                label: semantics
                    .source_primitives
                    .get(&read.symbol)
                    .and_then(|primitive| semantics.bundled_returns.get(primitive))
                    .map_or_else(
                        || read.display.to_string(),
                        |returned| returned.label.clone(),
                    ),
                evidence: None,
            };
            seen_reactive_reads
                .insert((reactive_read.kind.clone(), reactive_read.label.clone()))
                .then_some(reactive_read)
        })
        .collect::<Vec<_>>();
    let first_returned =
        returned_summary
            .iter()
            .fold(None::<&SummaryRead>, |current, candidate| match current {
                None => Some(candidate),
                Some(best) if location_order(&candidate.declaration, &best.declaration).is_lt() => {
                    Some(candidate)
                }
                Some(best) => Some(best),
            });
    let returns = structured_return.cloned().or_else(|| {
        first_returned.map(|read| ContractReturn {
            kind: if semantics.source_kinds.get(&read.symbol) == Some(&ReactiveSourceKind::Store) {
                "store-path".into()
            } else {
                "accessor".into()
            },
            label: semantics
                .source_primitives
                .get(&read.symbol)
                .and_then(|primitive| semantics.bundled_returns.get(primitive))
                .map_or_else(
                    || read.display.to_string(),
                    |returned| returned.label.clone(),
                ),
            parameter: None,
            elements: Vec::new(),
            properties: BTreeMap::new(),
            evidence: None,
        })
    });
    let mut callback_summary = callbacks.to_vec();
    callback_summary.sort_by_key(|callback| callback.parameter);
    ContractExport {
        kind: "function".into(),
        evidence: None,
        variants: Vec::new(),
        reactive_reads,
        callbacks: callback_summary,
        owner_requirements: Vec::new(),
        returns,
        async_behavior: if node.r#async {
            "promise".into()
        } else {
            String::new()
        },
    }
}

fn resolve_local_reexport(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    module: &str,
    name: &str,
) -> Option<(ContractExport, usize)> {
    let mut visiting = HashSet::new();
    resolve_local_reexport_with_visiting(
        facts,
        graph,
        by_symbol,
        source_file,
        module,
        name,
        &mut visiting,
    )
}

fn resolve_local_import(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    local_name: &str,
) -> Option<(ContractExport, usize)> {
    let mut visiting = HashSet::new();
    resolve_local_import_with_visiting(
        facts,
        graph,
        by_symbol,
        source_file,
        local_name,
        &mut visiting,
    )
}

fn resolve_local_binding_initializer(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    local: solid_facts::core::Span,
) -> Option<(ContractExport, usize)> {
    let local_symbol = graph.entities.get(&location(file.path.shared(), local));
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.names.iter().any(|name| {
            name.span == local
                || local_symbol.is_some_and(|symbol| {
                    graph.entities.get(&location(file.path.shared(), name.span)) == Some(symbol)
                })
        })
    })?;
    let initializer = binding.initializer?;
    graph
        .entities
        .get(&location(file.path.shared(), initializer))
        .and_then(|symbol| {
            graph.by_symbol.get(symbol).and_then(|index| {
                by_symbol
                    .get(symbol)
                    .cloned()
                    .map(|summary| (summary, *index))
            })
        })
        .or_else(|| {
            file.source_text(initializer)
                .and_then(|name| resolve_local_import(facts, graph, by_symbol, file, name))
        })
}

fn resolve_local_import_with_visiting(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    local_name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> Option<(ContractExport, usize)> {
    for import in source_file
        .ast
        .imports
        .iter()
        .filter(|import| !import.type_only)
    {
        for binding in import.bindings.iter().filter(|binding| !binding.type_only) {
            if source_file.source_text(binding.local.span) != Some(local_name) {
                continue;
            }
            let imported_name = binding.imported.as_deref().or_else(|| {
                (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
            })?;
            if import.module.starts_with('.') {
                return resolve_local_reexport_with_visiting(
                    facts,
                    graph,
                    by_symbol,
                    source_file,
                    import.module.as_str(),
                    imported_name,
                    visiting,
                );
            }
        }
    }
    None
}

fn resolve_local_reexport_with_visiting(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    module: &str,
    name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> Option<(ContractExport, usize)> {
    let source_path = Path::new(source_file.path.as_str());
    let target_path = source_path.parent()?.join(module).canonicalize().ok()?;
    let target = facts.files.iter().find(|file| {
        Path::new(file.path.as_str()).canonicalize().ok().as_ref() == Some(&target_path)
    })?;
    resolve_named_export(facts, graph, by_symbol, target, name, visiting)
}

fn resolve_named_export(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    file: &solid_facts::FileFacts,
    name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> Option<(ContractExport, usize)> {
    if !visiting.insert((file.path.to_string(), name.to_owned())) {
        return None;
    }
    for index in graph
        .nodes_by_path
        .get(file.path.as_str())
        .into_iter()
        .flatten()
        .copied()
    {
        let node = &graph.nodes[index];
        if node.exported
            && node.name.as_deref() == Some(name)
            && let Some(symbol) = &node.symbol
            && let Some(summary) = by_symbol.get(symbol)
        {
            return Some((summary.clone(), index));
        }
    }
    for export in file.ast.exports.iter().filter(|export| !export.type_only) {
        for specifier in export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .filter(|specifier| !specifier.type_only && specifier.exported.as_str() == name)
        {
            let local_name = file.source_text(specifier.local.span).unwrap_or(name);
            if let Some(module) = export.module.as_deref()
                && let Some(summary) = resolve_local_reexport_with_visiting(
                    facts, graph, by_symbol, file, module, local_name, visiting,
                )
            {
                return Some(summary);
            }
            if let Some(summary) = resolve_local_import_with_visiting(
                facts, graph, by_symbol, file, local_name, visiting,
            ) {
                return Some(summary);
            }
            if let Some(summary) = resolve_local_binding_initializer(
                facts,
                file,
                graph,
                by_symbol,
                specifier.local.span,
            ) {
                return Some(summary);
            }
            if let Some(resolved) = graph
                .entities
                .get(&location(file.path.shared(), specifier.local.span))
                .and_then(|symbol| {
                    graph.by_symbol.get(symbol).and_then(|index| {
                        by_symbol
                            .get(symbol)
                            .cloned()
                            .map(|summary| (summary, *index))
                    })
                })
            {
                return Some(resolved);
            }
        }
        if export.kind == solid_facts::ast::ExportKind::All
            && let Some(module) = export.module.as_deref()
            && let Some(summary) = resolve_local_reexport_with_visiting(
                facts, graph, by_symbol, file, module, name, visiting,
            )
        {
            return Some(summary);
        }
    }
    None
}

fn contract_export_fragment(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    project_directory: Option<&Path>,
    graph: &ContractGraph<'_>,
    node_keys: &[ContractNodeKey],
    node_contracts: &HashMap<ContractNodeKey, ContractExport>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
) -> ContractExportFragment {
    let mut fragment = ContractExportFragment::default();
    if project_directory
        .is_some_and(|directory| !path_within_project(Path::new(file.path.as_str()), directory))
    {
        return fragment;
    }
    for index in graph
        .nodes_by_path
        .get(file.path.as_str())
        .into_iter()
        .flatten()
        .copied()
    {
        let node = &graph.nodes[index];
        if node.exported
            && let (Some(name), Some(symbol)) = (&node.name, &node.symbol)
            && let Some(target) = graph.by_symbol.get(symbol).copied()
            && let Some(summary) = node_contracts.get(&node_keys[target])
        {
            fragment.dependencies.insert(node_keys[target].clone());
            fragment.direct.push((name.clone(), summary.clone()));
        }
    }
    for export in file.ast.exports.iter().filter(|export| !export.type_only) {
        for specifier in export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .filter(|specifier| !specifier.type_only)
        {
            let target = graph
                .entities
                .get(&location(file.path.shared(), specifier.local.span))
                .and_then(|symbol| graph.by_symbol.get(symbol))
                .copied();
            let summary = export
                .module
                .as_deref()
                .and_then(|module| {
                    let local_name = file
                        .source_text(specifier.local.span)
                        .unwrap_or(specifier.exported.as_str());
                    resolve_local_reexport(facts, graph, by_symbol, file, module, local_name)
                })
                .or_else(|| {
                    let local_name = file
                        .source_text(specifier.local.span)
                        .unwrap_or(specifier.exported.as_str());
                    resolve_local_import(facts, graph, by_symbol, file, local_name)
                })
                .or_else(|| {
                    resolve_local_binding_initializer(
                        facts,
                        file,
                        graph,
                        by_symbol,
                        specifier.local.span,
                    )
                })
                .map(|(summary, index)| {
                    fragment.dependencies.insert(node_keys[index].clone());
                    summary
                })
                .or_else(|| {
                    target.and_then(|index| {
                        fragment.dependencies.insert(node_keys[index].clone());
                        node_contracts.get(&node_keys[index]).cloned()
                    })
                })
                .unwrap_or_else(value_contract_export);
            let summary = promote_callable_export(facts, file, specifier.local.span, summary);
            fragment
                .syntax
                .push((specifier.exported.to_string(), summary, true));
        }
        for binding in file.ast.exported_bindings(export) {
            for name in &binding.names {
                let target = graph
                    .entities
                    .get(&location(file.path.shared(), name.span))
                    .and_then(|symbol| graph.by_symbol.get(symbol))
                    .copied();
                let summary = target
                    .and_then(|index| {
                        fragment.dependencies.insert(node_keys[index].clone());
                        node_contracts.get(&node_keys[index]).cloned()
                    })
                    .or_else(|| {
                        resolve_local_binding_initializer(facts, file, graph, by_symbol, name.span)
                            .map(|(summary, index)| {
                                fragment.dependencies.insert(node_keys[index].clone());
                                summary
                            })
                    })
                    .unwrap_or_else(value_contract_export);
                let summary = promote_callable_export(facts, file, name.span, summary);
                fragment.syntax.push((
                    file.source_text(name.span).unwrap_or_default().to_owned(),
                    summary,
                    false,
                ));
            }
        }
    }
    fragment
}

pub(super) fn contract_export_summaries_incremental(
    cache: &mut CachedContractExports,
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    reverse_edges: &[Vec<usize>],
    graph_node_reused_paths: &HashSet<&str>,
    changed_semantic_symbols: Option<&HashSet<SymbolId>>,
    analysis: &ContractAnalysis<'_>,
) -> Arc<BTreeMap<String, ContractExport>> {
    let mut ordinals = HashMap::<&str, usize>::new();
    let node_keys = graph
        .nodes
        .iter()
        .map(|node| {
            let ordinal = ordinals.entry(node.path.as_str()).or_default();
            let key = ContractNodeKey {
                path: node.path.clone(),
                ordinal: *ordinal,
            };
            *ordinal += 1;
            key
        })
        .collect::<Vec<_>>();
    let current_keys = node_keys.iter().cloned().collect::<HashSet<_>>();
    let mut dirty = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            (!graph_node_reused_paths.contains(node.path.as_str())
                || !cache.nodes.contains_key(&node_keys[index])
                || changed_semantic_symbols.is_some_and(|changed| {
                    analysis.summaries[index]
                        .iter()
                        .chain(analysis.returned[index].iter())
                        .any(|read| changed.contains(&read.symbol))
                }))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let mut dirty_set = dirty.iter().copied().collect::<HashSet<_>>();
    while let Some(target) = dirty.pop() {
        for owner in reverse_edges.get(target).into_iter().flatten().copied() {
            if dirty_set.insert(owner) {
                dirty.push(owner);
            }
        }
    }
    let mut dirty_indices = dirty_set.into_iter().collect::<Vec<_>>();
    dirty_indices.sort_unstable();
    let rebuilt_nodes = parallel_slice_results(&dirty_indices, |index| {
        contract_export_function(
            &graph.nodes[*index],
            &analysis.summaries[*index],
            &analysis.returned[*index],
            analysis.structured_returns[*index].as_ref(),
            &analysis.callbacks[*index],
            &analysis.semantics,
        )
    });
    let mut changed_nodes = HashSet::<ContractNodeKey>::new();
    for (index, contract) in dirty_indices.into_iter().zip(rebuilt_nodes) {
        let key = node_keys[index].clone();
        if cache.nodes.get(&key) != Some(&contract) {
            changed_nodes.insert(key.clone());
            cache.nodes.insert(key, contract);
        }
    }
    let removed_nodes = cache
        .nodes
        .keys()
        .filter(|key| !current_keys.contains(*key))
        .cloned()
        .collect::<Vec<_>>();
    for key in removed_nodes {
        cache.nodes.remove(&key);
        changed_nodes.insert(key);
    }
    let current_paths = facts
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    let removed_files = cache
        .files
        .keys()
        .filter(|path| !current_paths.contains(path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let mut fragments_changed = !removed_files.is_empty();
    for path in removed_files {
        cache.files.remove(&path);
    }
    let project_directory = Path::new(&facts.project_id).parent();
    let by_symbol = graph
        .by_symbol
        .iter()
        .filter_map(|(symbol, index)| {
            cache
                .nodes
                .get(&node_keys[*index])
                .cloned()
                .map(|summary| (symbol.clone(), summary))
        })
        .collect::<HashMap<_, _>>();
    let rebuild_files = facts
        .files
        .iter()
        .filter(|file| {
            !graph_node_reused_paths.contains(file.path.as_str())
                || cache
                    .files
                    .get(file.path.as_str())
                    .is_none_or(|fragment| !fragment.dependencies.is_disjoint(&changed_nodes))
        })
        .collect::<Vec<_>>();
    let rebuilt_fragments = parallel_slice_results(&rebuild_files, |file| {
        contract_export_fragment(
            facts,
            file,
            project_directory,
            graph,
            &node_keys,
            &cache.nodes,
            &by_symbol,
        )
    });
    for (file, fragment) in rebuild_files.into_iter().zip(rebuilt_fragments) {
        fragments_changed |= cache.files.get(file.path.as_str()) != Some(&fragment);
        cache.files.insert(file.path.to_string(), fragment);
    }
    if !fragments_changed && let Some(aggregate) = &cache.aggregate {
        return Arc::clone(aggregate);
    }
    let aggregate = aggregate_contract_fragments(facts, &cache.files);
    let aggregate = Arc::new(aggregate);
    cache.aggregate = Some(Arc::clone(&aggregate));
    aggregate
}

pub(super) fn contract_export_summaries(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    analysis: &ContractAnalysis<'_>,
) -> BTreeMap<String, ContractExport> {
    let mut ordinals = HashMap::<&str, usize>::new();
    let node_keys = graph
        .nodes
        .iter()
        .map(|node| {
            let ordinal = ordinals.entry(node.path.as_str()).or_default();
            let key = ContractNodeKey {
                path: node.path.clone(),
                ordinal: *ordinal,
            };
            *ordinal += 1;
            key
        })
        .collect::<Vec<_>>();
    let node_contracts = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            (
                node_keys[index].clone(),
                contract_export_function(
                    node,
                    &analysis.summaries[index],
                    &analysis.returned[index],
                    analysis.structured_returns[index].as_ref(),
                    &analysis.callbacks[index],
                    &analysis.semantics,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    let by_symbol = graph
        .by_symbol
        .iter()
        .filter_map(|(symbol, index)| {
            node_contracts
                .get(&node_keys[*index])
                .cloned()
                .map(|summary| (symbol.clone(), summary))
        })
        .collect::<HashMap<_, _>>();
    let project_directory = Path::new(&facts.project_id).parent();
    let fragments = facts
        .files
        .iter()
        .map(|file| {
            (
                file.path.to_string(),
                contract_export_fragment(
                    facts,
                    file,
                    project_directory,
                    graph,
                    &node_keys,
                    &node_contracts,
                    &by_symbol,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    aggregate_contract_fragments(facts, &fragments)
}

fn aggregate_contract_fragments(
    facts: &ProjectFacts,
    fragments: &HashMap<String, ContractExportFragment>,
) -> BTreeMap<String, ContractExport> {
    let mut aggregate = BTreeMap::new();
    for file in &facts.files {
        if let Some(fragment) = fragments.get(file.path.as_str()) {
            for (name, summary) in &fragment.direct {
                aggregate.insert(name.clone(), summary.clone());
            }
        }
    }
    for file in &facts.files {
        if let Some(fragment) = fragments.get(file.path.as_str()) {
            for (name, summary, replace) in &fragment.syntax {
                if *replace {
                    aggregate.insert(name.clone(), summary.clone());
                } else {
                    aggregate
                        .entry(name.clone())
                        .or_insert_with(|| summary.clone());
                }
            }
        }
    }
    aggregate
}

fn path_within_project(path: &Path, directory: &Path) -> bool {
    path.starts_with(directory)
        || path.canonicalize().is_ok_and(|path| {
            directory
                .canonicalize()
                .is_ok_and(|directory| path.starts_with(directory))
        })
}

fn value_contract_export() -> ContractExport {
    ContractExport {
        kind: "value".into(),
        variants: Vec::new(),
        ..ContractExport::default()
    }
}

fn promote_callable_export(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    mut summary: ContractExport,
) -> ContractExport {
    if summary.kind != "value" {
        return summary;
    }
    let target = location(file.path.shared(), span);
    let entity = facts.typescript.entities().find(|entity| {
        entity.location.path == target.path
            && entity.location.start_byte == target.start_byte
            && entity.location.end_byte == target.end_byte
    });
    let Some(entity) = entity else {
        return summary;
    };
    if entity.callability == Some(Callability::Callable) {
        summary.kind = "function".into();
    }
    summary
}
