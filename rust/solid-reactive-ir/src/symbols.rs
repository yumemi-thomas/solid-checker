//! TypeScript symbol/index construction.
//!
//! Builds the alias-root, entity, symbol-name and reference indexes from the
//! raw TypeScript fact table, and patches them incrementally from a change set.
//! These are the "static API" inputs the reactive pipeline queries; the module
//! owns both the full build and the incremental patch.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use solid_facts::ProjectFacts;
use typefacts::{Declaration, FactTable, Location, SymbolFact};

use super::{
    CachedTypeScriptIndexes, EntitySymbols, SourceDiscoverySymbolSemantics,
    SourceDiscoveryTypeScriptDelta, location, location_order,
    source_discovery_declaration_semantic, source_discovery_declaration_semantics,
};

pub(super) fn add_solid_namespace_names(
    facts: &ProjectFacts,
    entities: &EntitySymbols,
    names: &mut HashMap<String, String>,
) {
    for file in &facts.files {
        for import in &file.ast.imports {
            let primitives: &[&str] = if import.module == "solid-js" {
                &[
                    "createSignal",
                    "createMemo",
                    "mapArray",
                    "createStore",
                    "createProjection",
                    "createOptimistic",
                    "createOptimisticStore",
                    "createEffect",
                    "createRenderEffect",
                    "createTrackedEffect",
                    "createReaction",
                    "createRoot",
                    "createOwner",
                    "untrack",
                    "onSettled",
                    "onCleanup",
                    "flush",
                    "Loading",
                    "Show",
                    "Match",
                    "Switch",
                    "merge",
                    "refresh",
                    "affects",
                    "action",
                ]
            } else if import.module.starts_with("@solidjs/") {
                &["dynamic"]
            } else {
                continue;
            };
            for binding in &import.bindings {
                if binding.kind != solid_ast_facts::ImportKind::Namespace {
                    continue;
                }
                let location = location(file.path.as_str(), binding.local.span);
                let Some(symbol) = entities.get(&location) else {
                    continue;
                };
                for primitive in primitives {
                    names.insert(format!("{symbol}::{primitive}"), (*primitive).into());
                }
            }
        }
    }
}

pub(super) fn alias_roots_and_source_declarations(
    table: &FactTable,
) -> (HashMap<String, String>, HashMap<String, Declaration>) {
    let targets = symbol_alias_targets(table);
    let mut roots = HashMap::with_capacity(table.symbols.len());
    let mut declarations = HashMap::new();
    for symbol in table.symbols.iter() {
        let mut root = symbol.id.to_string();
        for _ in 0..=targets.len() {
            let Some(next) = targets.get(root.as_str()) else {
                break;
            };
            root.clone_from(next);
        }
        if !declarations.contains_key(root.as_str())
            && let Some(declaration) = symbol
                .declarations
                .iter()
                .find(|declaration| !declaration.location.path.ends_with(".d.ts"))
        {
            declarations.insert(root.clone(), declaration.clone());
        }
        roots.insert(symbol.id.to_string(), root);
    }
    (roots, declarations)
}

pub(super) fn symbol_alias_targets(table: &FactTable) -> HashMap<String, String> {
    table
        .symbols
        .iter()
        .filter(|symbol| !symbol.alias_target.is_empty())
        .map(|symbol| (symbol.id.to_string(), symbol.alias_target.to_string()))
        .collect()
}

pub(super) fn source_discovery_symbol_semantics(
    table: &FactTable,
) -> HashMap<String, SourceDiscoverySymbolSemantics> {
    table
        .symbols
        .iter()
        .map(|symbol| {
            (
                symbol.id.to_string(),
                SourceDiscoverySymbolSemantics {
                    alias_target: symbol.alias_target.to_string(),
                    declarations: source_discovery_declaration_semantics(&symbol.declarations),
                },
            )
        })
        .collect()
}

pub(super) fn symbols_by_root(
    table: &FactTable,
    aliases: &HashMap<String, String>,
) -> HashMap<String, Vec<String>> {
    let mut by_root = HashMap::<String, Vec<String>>::new();
    for symbol in table.symbols.iter() {
        let root = aliases
            .get(symbol.id.as_ref())
            .cloned()
            .unwrap_or_else(|| symbol.id.to_string());
        by_root.entry(root).or_default().push(symbol.id.to_string());
    }
    by_root
}

fn alias_root(symbol: &str, targets: &HashMap<String, String>) -> String {
    let mut root = symbol.to_owned();
    for _ in 0..=targets.len() {
        let Some(next) = targets.get(&root) else {
            break;
        };
        root.clone_from(next);
    }
    root
}

pub(super) fn patch_typescript_indexes(
    cache: &mut CachedTypeScriptIndexes,
    table: &FactTable,
    symbols_by_id: &HashMap<&str, &SymbolFact>,
    changes: &solid_facts::TypeScriptChanges,
) -> Option<(Duration, Duration)> {
    // An empty non-reuse change set is the sidecar's fail-closed description
    // of a full table replacement, so only named deltas are patchable.
    if changes.unchanged
        || changes.entity_paths.is_empty()
            && changes.symbol_ids.is_empty()
            && changes.file_paths.is_empty()
    {
        return None;
    }

    let alias_started = Instant::now();
    let changed_symbols = changes
        .symbol_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let current_targets = changes
        .symbol_ids
        .iter()
        .map(|id| {
            (
                id,
                symbols_by_id
                    .get(id.as_str())
                    .map(|symbol| symbol.alias_target.as_ref())
                    .filter(|target| !target.is_empty()),
            )
        })
        .collect::<Vec<_>>();
    let removed_aliases = current_targets
        .iter()
        .filter(|(id, current)| {
            cache.symbol_alias_targets.contains_key(id.as_str()) && current.is_none()
        })
        .count();
    let added_aliases = current_targets
        .iter()
        .filter(|(id, current)| {
            !cache.symbol_alias_targets.contains_key(id.as_str()) && current.is_some()
        })
        .count();
    let structurally_changed_symbols = current_targets
        .iter()
        .filter_map(|(id, current_target)| {
            let existed = cache.aliases.contains_key(id.as_str());
            let exists = symbols_by_id.contains_key(id.as_str());
            let old_target = cache
                .symbol_alias_targets
                .get(id.as_str())
                .map(String::as_str);
            (existed != exists || old_target != *current_target).then_some(id.as_str())
        })
        .collect::<HashSet<_>>();
    let alias_graph_is_local = added_aliases == removed_aliases
        && current_targets.iter().all(|(id, current)| {
            cache
                .symbol_alias_targets
                .get(id.as_str())
                .zip(*current)
                .is_none_or(|(old, current)| old == current)
        })
        && cache.symbol_alias_targets.iter().all(|(symbol, target)| {
            changed_symbols.contains(symbol.as_str())
                || !structurally_changed_symbols.contains(target.as_str())
        });
    if !alias_graph_is_local {
        return None;
    }

    let mut semantic_symbol_ids = changes
        .symbol_ids
        .iter()
        .filter(|id| {
            let current =
                symbols_by_id
                    .get(id.as_str())
                    .map(|symbol| SourceDiscoverySymbolSemantics {
                        alias_target: symbol.alias_target.to_string(),
                        declarations: source_discovery_declaration_semantics(&symbol.declarations),
                    });
            cache.source_discovery_symbol_semantics.get(id.as_str()) != current.as_ref()
        })
        .cloned()
        .collect::<HashSet<_>>();

    let mut affected_roots = changes
        .symbol_ids
        .iter()
        .filter_map(|id| cache.aliases.get(id))
        .cloned()
        .collect::<HashSet<_>>();
    for (id, target) in current_targets {
        if let Some(target) = target {
            cache
                .symbol_alias_targets
                .insert(id.clone(), target.to_owned());
        } else {
            cache.symbol_alias_targets.remove(id.as_str());
        }
    }
    affected_roots.extend(changes.symbol_ids.iter().filter_map(|id| {
        symbols_by_id
            .get(id.as_str())
            .map(|_| alias_root(id, &cache.symbol_alias_targets))
    }));
    let retained_root_semantics = affected_roots
        .iter()
        .map(|root| {
            (
                root.clone(),
                (
                    cache
                        .source_declarations
                        .get(root)
                        .map(source_discovery_declaration_semantic),
                    cache.symbol_names.get(root).cloned(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    for id in &changes.symbol_ids {
        if let Some(old_root) = cache.aliases.get(id)
            && let Some(members) = cache.symbols_by_root.get_mut(old_root)
            && let Ok(index) = members.binary_search(id)
        {
            members.remove(index);
        }
    }
    for id in &changes.symbol_ids {
        if let Some(symbol) = symbols_by_id.get(id.as_str()) {
            let root = alias_root(id, &cache.symbol_alias_targets);
            cache.aliases.insert(id.clone(), root.clone());
            let members = cache.symbols_by_root.entry(root).or_default();
            if let Err(index) = members.binary_search(id) {
                members.insert(index, id.clone());
            }
            cache.source_discovery_symbol_semantics.insert(
                id.clone(),
                SourceDiscoverySymbolSemantics {
                    alias_target: symbol.alias_target.to_string(),
                    declarations: source_discovery_declaration_semantics(&symbol.declarations),
                },
            );
        } else {
            cache.aliases.remove(id);
            cache.source_discovery_symbol_semantics.remove(id);
        }
    }
    for root in &affected_roots {
        cache.source_declarations.remove(root);
        cache.symbol_names.remove(root);
        cache.references_by_source.remove(root);
        for id in cache.symbols_by_root.get(root).into_iter().flatten() {
            let Some(symbol) = symbols_by_id.get(id.as_str()) else {
                continue;
            };
            if !cache.source_declarations.contains_key(root)
                && let Some(declaration) = symbol
                    .declarations
                    .iter()
                    .find(|declaration| !declaration.location.path.ends_with(".d.ts"))
            {
                cache
                    .source_declarations
                    .insert(root.clone(), declaration.clone());
            }
            for declaration in symbol.declarations.iter() {
                if solid_primitive_declaration(declaration) {
                    cache
                        .symbol_names
                        .insert(root.clone(), declaration.name.to_string());
                }
            }
            if !symbol.references.is_empty() {
                cache
                    .references_by_source
                    .entry(root.clone())
                    .or_default()
                    .extend(symbol.references.iter().cloned());
            }
        }
    }
    for root in &affected_roots {
        if let Some(locations) = cache.references_by_source.get_mut(root) {
            locations.sort_by(location_order);
            locations.dedup();
        }
    }
    semantic_symbol_ids.extend(
        affected_roots
            .iter()
            .filter(|root| {
                retained_root_semantics
                    .get(root.as_str())
                    .is_none_or(|(declaration, name)| {
                        cache
                            .source_declarations
                            .get(root.as_str())
                            .map(source_discovery_declaration_semantic)
                            != *declaration
                            || cache.symbol_names.get(root.as_str()) != name.as_ref()
                    })
            })
            .cloned(),
    );
    let alias_elapsed = alias_started.elapsed();

    let entities_started = Instant::now();
    for path in &changes.entity_paths {
        cache.entities.by_path.remove(path);
        let start = table
            .entities
            .partition_point(|entity| entity.location.path.as_ref() < path.as_str());
        let end = table
            .entities
            .partition_point(|entity| entity.location.path.as_ref() <= path.as_str());
        for entity in &table.entities[start..end] {
            if entity.symbol.is_empty() {
                continue;
            }
            cache
                .entities
                .by_path
                .entry(entity.location.path.to_string())
                .or_default()
                .insert(
                    (entity.location.start_byte, entity.location.end_byte),
                    cache
                        .aliases
                        .get(entity.symbol.as_ref())
                        .cloned()
                        .unwrap_or_else(|| entity.symbol.to_string()),
                );
        }
    }
    let entities_elapsed = entities_started.elapsed();
    cache.source_discovery_delta = Some(SourceDiscoveryTypeScriptDelta {
        entity_paths: changes.entity_paths.iter().cloned().collect(),
        file_paths: changes.file_paths.iter().cloned().collect(),
        semantic_symbol_ids,
    });

    Some((alias_elapsed, entities_elapsed))
}

pub(super) fn async_symbol_root(symbol: &str, table: &FactTable) -> String {
    let aliases = table
        .files
        .iter()
        .flat_map(|file| file.async_functions.iter())
        .filter(|function| !function.symbol.is_empty() && !function.target.is_empty())
        .map(|function| (function.symbol.as_ref(), function.target.as_ref()))
        .collect::<HashMap<_, _>>();
    let mut current = symbol;
    let mut seen = HashSet::new();
    while seen.insert(current) {
        let Some(target) = aliases.get(current).copied() else {
            break;
        };
        current = target;
    }
    current.into()
}

pub(super) fn entity_symbols(table: &FactTable, roots: &HashMap<String, String>) -> EntitySymbols {
    let mut by_path = HashMap::<String, HashMap<(u64, u64), String>>::new();
    for entity in table
        .entities
        .iter()
        .filter(|entity| !entity.symbol.is_empty())
    {
        by_path
            .entry(entity.location.path.to_string())
            .or_default()
            .insert(
                (entity.location.start_byte, entity.location.end_byte),
                roots
                    .get(entity.symbol.as_ref())
                    .cloned()
                    .unwrap_or_else(|| entity.symbol.to_string()),
            );
    }
    EntitySymbols { by_path }
}

pub(super) fn symbol_names(
    table: &FactTable,
    roots: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut names = HashMap::new();
    for symbol in table.symbols.iter() {
        let root = roots
            .get(symbol.id.as_ref())
            .cloned()
            .unwrap_or_else(|| symbol.id.to_string());
        for declaration in symbol.declarations.iter() {
            if solid_primitive_declaration(declaration) {
                names.insert(root.clone(), declaration.name.to_string());
            }
        }
    }
    names
}

pub(super) fn references_by_source(
    table: &FactTable,
    roots: &HashMap<String, String>,
) -> HashMap<String, Vec<Location>> {
    let mut references = HashMap::<String, Vec<Location>>::new();
    for symbol in table.symbols.iter() {
        if symbol.references.is_empty() {
            continue;
        }
        let root = roots
            .get(symbol.id.as_ref())
            .cloned()
            .unwrap_or_else(|| symbol.id.to_string());
        references
            .entry(root)
            .or_default()
            .extend(symbol.references.iter().cloned());
    }
    for locations in references.values_mut() {
        locations.sort_by(location_order);
        locations.dedup();
    }
    references
}

fn solid_primitive_declaration(declaration: &Declaration) -> bool {
    (declaration.location.path.contains("solid-js")
        || declaration.location.path.contains("@solidjs"))
        && matches!(
            declaration.name.as_ref(),
            "createSignal"
                | "createMemo"
                | "mapArray"
                | "createStore"
                | "createProjection"
                | "createOptimistic"
                | "createOptimisticStore"
                | "dynamic"
                | "createEffect"
                | "createRenderEffect"
                | "createTrackedEffect"
                | "createReaction"
                | "createRoot"
                | "createOwner"
                | "untrack"
                | "onSettled"
                | "onCleanup"
                | "flush"
                | "Loading"
                | "Show"
                | "Match"
                | "Switch"
                | "merge"
                | "refresh"
                | "affects"
                | "action"
        )
}
