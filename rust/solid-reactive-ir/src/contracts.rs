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

use solid_facts::ProjectFacts;
use typefacts::Location;

use super::{
    CachedContractExports, ContractCallback, ContractExport, ContractExportFragment,
    ContractNodeKey, ContractReactiveRead, ContractReturn, EntitySymbols, PackageContract,
    ReactiveSourceKind, StaticViolation, SummaryNode, SummaryRead, SummaryReads, location,
    location_order,
};

#[derive(Clone)]
pub(super) struct ResolvedContractBinding {
    pub(super) local_name: String,
    pub(super) imported_name: String,
    pub(super) package_name: String,
    pub(super) symbol: String,
    pub(super) contract_location: Location,
    pub(super) summary: ContractExport,
}

pub(super) struct ResolvedContracts {
    pub(super) bindings: Vec<ResolvedContractBinding>,
    pub(super) by_symbol: HashMap<String, ResolvedContractBinding>,
    pub(super) missing_exports: Vec<StaticViolation>,
}

pub(super) fn resolve_contract_imports(
    facts: &ProjectFacts,
    contracts: &[PackageContract],
    entities: &EntitySymbols,
) -> ResolvedContracts {
    let mut bindings = Vec::new();
    let mut by_symbol = HashMap::new();
    let mut missing_exports = Vec::new();
    for file in &facts.files {
        for import in &file.ast.imports {
            if import.type_only {
                continue;
            }
            let Some(contract) = contracts
                .iter()
                .filter(|contract| {
                    import.module == contract.package.name
                        || import
                            .module
                            .strip_prefix(&contract.package.name)
                            .is_some_and(|suffix| suffix.starts_with('/'))
                })
                .max_by_key(|contract| contract.package.name.len())
            else {
                continue;
            };
            for binding in &import.bindings {
                if binding.type_only {
                    continue;
                }
                let Some(imported) = binding.imported.as_deref().or_else(|| {
                    (binding.kind == solid_ast_facts::ImportKind::Default).then_some("default")
                }) else {
                    continue;
                };
                let binding_location = location(file.path.as_str(), binding.local.span);
                let Some(symbol) = entities.get(&binding_location).cloned() else {
                    continue;
                };
                let Some(summary) = contract.exports.get(imported).cloned() else {
                    if contract.package.name == "solid-js" {
                        continue;
                    }
                    missing_exports.push(StaticViolation {
                        id: "SC9001".into(),
                        rule: "package-contract-export-missing".into(),
                        message: format!(
                            "the reactivity contract for {} has no entry for imported export {imported}; solid-checker cannot tell whether it reads reactive values, takes tracked callbacks, or returns accessors, so code flowing through it cannot be certified",
                            import.module
                        ),
                        hint: format!(
                            "Add an export summary for {imported} to the package's solid-reactivity.json (reactive reads, callbacks, return kind); an empty summary certifies explicitly that the export is not reactive. See docs/package-contracts.md for the format."
                        ),
                        location: binding_location,
                        analysis_context: String::new(),
                        fixes: vec![],
                    });
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
                if contract.package.name != "solid-js" {
                    bindings.push(resolved.clone());
                    by_symbol.insert(symbol, resolved);
                }
            }
        }
    }
    ResolvedContracts {
        bindings,
        by_symbol,
        missing_exports,
    }
}

pub(super) struct ContractSemantics<'a> {
    pub(super) bundled_returns: &'a HashMap<String, ContractReturn>,
    pub(super) source_kinds: &'a HashMap<String, ReactiveSourceKind>,
    pub(super) source_primitives: &'a HashMap<String, String>,
}

pub(super) struct ContractGraph<'a> {
    pub(super) nodes: &'a [SummaryNode],
    pub(super) nodes_by_path: &'a HashMap<String, Vec<usize>>,
    pub(super) by_symbol: &'a HashMap<String, usize>,
    pub(super) entities: &'a EntitySymbols,
}

pub(super) struct ContractAnalysis<'a> {
    pub(super) summaries: &'a [SummaryReads],
    pub(super) returned: &'a [SummaryReads],
    pub(super) callbacks: &'a [Vec<ContractCallback>],
    pub(super) semantics: ContractSemantics<'a>,
}

fn contract_export_function(
    node: &SummaryNode,
    summary: &SummaryReads,
    returned_summary: &SummaryReads,
    callbacks: &[ContractCallback],
    semantics: &ContractSemantics<'_>,
) -> ContractExport {
    let reactive_reads = summary
        .iter()
        .map(|read| ContractReactiveRead {
            kind: read.kind.clone().unwrap_or_else(|| {
                if read.display.contains('.') {
                    "store-path".into()
                } else {
                    "accessor".into()
                }
            }),
            label: semantics
                .source_primitives
                .get(&read.symbol)
                .and_then(|primitive| semantics.bundled_returns.get(primitive))
                .map_or_else(|| read.display.clone(), |returned| returned.label.clone()),
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
    let returns = first_returned.map(|read| ContractReturn {
        kind: if semantics.source_kinds.get(&read.symbol) == Some(&ReactiveSourceKind::Store) {
            "store-path".into()
        } else {
            "accessor".into()
        },
        label: semantics
            .source_primitives
            .get(&read.symbol)
            .and_then(|primitive| semantics.bundled_returns.get(primitive))
            .map_or_else(|| read.display.clone(), |returned| returned.label.clone()),
    });
    let mut callback_summary = callbacks.to_vec();
    callback_summary.sort_by_key(|callback| callback.parameter);
    ContractExport {
        kind: "function".into(),
        reactive_reads,
        callbacks: callback_summary,
        returns,
        async_behavior: if node.r#async {
            "promise".into()
        } else {
            String::new()
        },
    }
}

fn contract_export_fragment(
    file: &solid_facts::FileFacts,
    project_directory: Option<&Path>,
    graph: &ContractGraph<'_>,
    node_keys: &[ContractNodeKey],
    node_contracts: &HashMap<ContractNodeKey, ContractExport>,
) -> ContractExportFragment {
    let mut fragment = ContractExportFragment::default();
    if project_directory
        .is_some_and(|directory| !Path::new(file.path.as_str()).starts_with(directory))
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
                .get(&location(file.path.as_str(), specifier.local.span))
                .and_then(|symbol| graph.by_symbol.get(symbol))
                .copied();
            let summary = target
                .and_then(|index| {
                    fragment.dependencies.insert(node_keys[index].clone());
                    node_contracts.get(&node_keys[index]).cloned()
                })
                .unwrap_or_else(value_contract_export);
            fragment
                .syntax
                .push((specifier.exported.to_string(), summary, true));
        }
        for binding in file.ast.bindings.iter().filter(|binding| {
            binding.shape != solid_ast_facts::BindingShape::Array
                && export.span.contains(binding.declaration)
                && !file.ast.functions.iter().any(|function| {
                    export.span.contains(function.span)
                        && function.body.contains(binding.declaration)
                })
        }) {
            for name in &binding.names {
                let target = graph
                    .entities
                    .get(&location(file.path.as_str(), name.span))
                    .and_then(|symbol| graph.by_symbol.get(symbol))
                    .copied();
                let summary = target
                    .and_then(|index| {
                        fragment.dependencies.insert(node_keys[index].clone());
                        node_contracts.get(&node_keys[index]).cloned()
                    })
                    .unwrap_or_else(value_contract_export);
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
    changed_semantic_symbols: Option<&HashSet<String>>,
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
    let mut changed_nodes = HashSet::<ContractNodeKey>::new();
    for index in dirty_set {
        let contract = contract_export_function(
            &graph.nodes[index],
            &analysis.summaries[index],
            &analysis.returned[index],
            &analysis.callbacks[index],
            &analysis.semantics,
        );
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
    for file in &facts.files {
        let rebuild = !graph_node_reused_paths.contains(file.path.as_str())
            || cache
                .files
                .get(file.path.as_str())
                .is_none_or(|fragment| !fragment.dependencies.is_disjoint(&changed_nodes));
        if !rebuild {
            continue;
        }
        let fragment =
            contract_export_fragment(file, project_directory, graph, &node_keys, &cache.nodes);
        fragments_changed |= cache.files.get(file.path.as_str()) != Some(&fragment);
        cache.files.insert(file.path.to_string(), fragment);
    }
    if !fragments_changed && let Some(aggregate) = &cache.aggregate {
        return Arc::clone(aggregate);
    }
    let mut aggregate = BTreeMap::new();
    for file in &facts.files {
        if let Some(fragment) = cache.files.get(file.path.as_str()) {
            for (name, summary) in &fragment.direct {
                aggregate.insert(name.clone(), summary.clone());
            }
        }
    }
    for file in &facts.files {
        if let Some(fragment) = cache.files.get(file.path.as_str()) {
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
    let aggregate = Arc::new(aggregate);
    cache.aggregate = Some(Arc::clone(&aggregate));
    aggregate
}

pub(super) fn contract_export_summaries(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    analysis: &ContractAnalysis<'_>,
) -> BTreeMap<String, ContractExport> {
    let project_directory = Path::new(&facts.project_id).parent();
    let mut by_symbol = HashMap::<String, ContractExport>::with_capacity(graph.nodes.len());
    for (index, node) in graph.nodes.iter().enumerate() {
        let Some(symbol) = &node.symbol else {
            continue;
        };
        let contribution = contract_export_function(
            node,
            &analysis.summaries[index],
            &analysis.returned[index],
            &analysis.callbacks[index],
            &analysis.semantics,
        );
        by_symbol.insert(symbol.clone(), contribution);
    }

    let mut exports = BTreeMap::new();
    for node in graph.nodes.iter().filter(|node| {
        node.exported
            && project_directory
                .is_none_or(|directory| Path::new(&node.path).starts_with(directory))
    }) {
        if let (Some(name), Some(symbol)) = (&node.name, &node.symbol)
            && let Some(summary) = by_symbol.get(symbol)
        {
            exports.insert(name.clone(), summary.clone());
        }
    }
    for file in facts.files.iter().filter(|file| {
        project_directory
            .is_none_or(|directory| Path::new(file.path.as_str()).starts_with(directory))
    }) {
        for export in file.ast.exports.iter().filter(|export| !export.type_only) {
            for specifier in export
                .specifiers
                .iter()
                .chain(export.declarations.iter())
                .filter(|specifier| !specifier.type_only)
            {
                let summary = graph
                    .entities
                    .get(&location(file.path.as_str(), specifier.local.span))
                    .and_then(|symbol| by_symbol.get(symbol))
                    .cloned();
                if let Some(summary) = summary {
                    exports.insert(specifier.exported.to_string(), summary);
                } else {
                    exports
                        .entry(specifier.exported.to_string())
                        .or_insert_with(value_contract_export);
                }
            }
            for binding in file.ast.bindings.iter().filter(|binding| {
                binding.shape != solid_ast_facts::BindingShape::Array
                    && export.span.contains(binding.declaration)
                    && !file.ast.functions.iter().any(|function| {
                        export.span.contains(function.span)
                            && function.body.contains(binding.declaration)
                    })
            }) {
                for name in &binding.names {
                    let name_text = file.source_text(name.span).unwrap_or_default();
                    if !exports.contains_key(name_text) {
                        let summary = graph
                            .entities
                            .get(&location(file.path.as_str(), name.span))
                            .and_then(|symbol| by_symbol.get(symbol))
                            .cloned()
                            .unwrap_or_else(value_contract_export);
                        exports.insert(name_text.to_owned(), summary);
                    }
                }
            }
        }
    }
    exports
}

fn value_contract_export() -> ContractExport {
    ContractExport {
        kind: "value".into(),
        ..ContractExport::default()
    }
}
