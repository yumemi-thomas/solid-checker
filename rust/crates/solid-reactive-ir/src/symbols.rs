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

use solid_facts::{ProjectFacts, TypeScriptSymbol, TypeScriptTable};
use typefacts::{Declaration, Location};

use super::{
    CachedTypeScriptIndexes, EntitySymbols, SourceDiscoverySymbolFingerprint,
    SourceDiscoveryTypeScriptDelta, SymbolId, SymbolInterner, SymbolName, location, location_order,
    source_discovery_declaration_semantic, source_discovery_symbol_fingerprint, symbol_id,
    symbol_name,
};

pub(super) fn add_solid_import_names(
    facts: &ProjectFacts,
    entities: &EntitySymbols,
    names: &mut HashMap<SymbolId, SymbolName>,
) {
    for file in &facts.files {
        for import in &file.ast.imports {
            let Some(primitives) = solid_module_primitives(import.module.as_str()) else {
                continue;
            };
            for binding in &import.bindings {
                let location = location(file.path.shared(), binding.local.span);
                let Some(symbol) = entities.get(&location) else {
                    continue;
                };
                if binding.kind == solid_facts::ast::ImportKind::Namespace {
                    for primitive in primitives {
                        names.insert(
                            symbol_id(&format!("{symbol}::{primitive}")),
                            (*primitive).into(),
                        );
                    }
                } else if let Some(imported) = binding.imported.as_deref()
                    && primitives.contains(&imported)
                {
                    names.insert(symbol.clone(), imported.into());
                }
            }
        }
        for export in &file.ast.exports {
            let Some(module) = export.module.as_deref() else {
                continue;
            };
            let Some(primitives) = solid_module_primitives(module) else {
                continue;
            };
            for specifier in &export.specifiers {
                let Some(primitive) = file.source_text(specifier.local.span) else {
                    continue;
                };
                if !primitives.contains(&primitive) {
                    continue;
                }
                let location = location(file.path.shared(), specifier.local.span);
                if let Some(symbol) = entities.get(&location) {
                    names.insert(symbol.clone(), primitive.into());
                }
            }
        }
    }
}

fn solid_module_primitives(module: &str) -> Option<&'static [&'static str]> {
    match module {
        "solid-js" => Some(&[
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
        ]),
        "@solidjs/web" => Some(&["dynamic"]),
        _ => None,
    }
}

pub(super) fn alias_roots_and_source_declarations(
    table: &TypeScriptTable,
    interner: &SymbolInterner,
) -> (HashMap<SymbolId, SymbolId>, HashMap<SymbolId, Declaration>) {
    let targets = symbol_alias_targets(table, interner);
    let mut roots = HashMap::new();
    let mut declarations = HashMap::new();
    for symbol in table.symbols() {
        let mut root = interner.intern(symbol.id());
        for _ in 0..=targets.len() {
            let Some(next) = targets.get(root.as_str()) else {
                break;
            };
            root.clone_from(next);
        }
        if !declarations.contains_key(root.as_str())
            && let Some(declaration) = symbol
                .declarations()
                .iter()
                .find(|declaration| !declaration.location.path.ends_with(".d.ts"))
        {
            declarations.insert(root.clone(), declaration.clone());
        }
        roots.insert(interner.intern(symbol.id()), root);
    }
    (roots, declarations)
}

pub(super) fn symbol_alias_targets(
    table: &TypeScriptTable,
    interner: &SymbolInterner,
) -> HashMap<SymbolId, SymbolId> {
    table
        .symbols()
        .filter(|symbol| !symbol.alias_target().is_empty())
        .map(|symbol| {
            (
                interner.intern(symbol.id()),
                interner.intern(symbol.alias_target()),
            )
        })
        .collect()
}

pub(super) fn source_discovery_symbol_semantics(
    table: &TypeScriptTable,
    interner: &SymbolInterner,
) -> HashMap<SymbolId, SourceDiscoverySymbolFingerprint> {
    table
        .symbols()
        .map(|symbol| {
            (
                interner.intern(symbol.id()),
                source_discovery_symbol_fingerprint(symbol.alias_target(), symbol.declarations()),
            )
        })
        .collect()
}

pub(super) fn symbols_by_root(
    table: &TypeScriptTable,
    aliases: &HashMap<SymbolId, SymbolId>,
    interner: &SymbolInterner,
) -> HashMap<SymbolId, Vec<SymbolId>> {
    let mut by_root = HashMap::<SymbolId, Vec<SymbolId>>::new();
    for symbol in table.symbols() {
        let root = aliases
            .get(symbol.id())
            .cloned()
            .unwrap_or_else(|| interner.intern(symbol.id()));
        by_root
            .entry(root)
            .or_default()
            .push(interner.intern(symbol.id()));
    }
    by_root
}

fn alias_root(
    symbol: &str,
    targets: &HashMap<SymbolId, SymbolId>,
    interner: &SymbolInterner,
) -> SymbolId {
    let mut root = interner.intern(symbol);
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
    table: &TypeScriptTable,
    symbols_by_id: &HashMap<&str, TypeScriptSymbol<'_>>,
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
                    .map(|symbol| symbol.alias_target())
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
                .map(SymbolId::as_str);
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
            let current = symbols_by_id.get(id.as_str()).map(|symbol| {
                source_discovery_symbol_fingerprint(symbol.alias_target(), symbol.declarations())
            });
            cache.source_discovery_symbol_semantics.get(id.as_str()) != current.as_ref()
        })
        .map(|id| cache.interner.intern(id))
        .collect::<HashSet<_>>();

    let mut affected_roots = changes
        .symbol_ids
        .iter()
        .filter_map(|id| cache.aliases.get(id.as_str()))
        .cloned()
        .collect::<HashSet<_>>();
    for (id, target) in current_targets {
        if let Some(target) = target {
            cache
                .symbol_alias_targets
                .insert(cache.interner.intern(id), cache.interner.intern(target));
        } else {
            cache.symbol_alias_targets.remove(id.as_str());
        }
    }
    affected_roots.extend(changes.symbol_ids.iter().filter_map(|id| {
        symbols_by_id
            .get(id.as_str())
            .map(|_| alias_root(id, &cache.symbol_alias_targets, &cache.interner))
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
        if let Some(old_root) = cache.aliases.get(id.as_str())
            && let Some(members) = cache.symbols_by_root.get_mut(old_root)
            && let Ok(index) = members.binary_search_by(|member| member.as_str().cmp(id))
        {
            members.remove(index);
        }
    }
    for id in &changes.symbol_ids {
        if let Some(symbol) = symbols_by_id.get(id.as_str()) {
            let compact_id = cache.interner.intern(id);
            let root = alias_root(id, &cache.symbol_alias_targets, &cache.interner);
            cache.aliases.insert(compact_id.clone(), root.clone());
            let members = cache.symbols_by_root.entry(root).or_default();
            if let Err(index) = members.binary_search(&compact_id) {
                members.insert(index, compact_id.clone());
            }
            cache.source_discovery_symbol_semantics.insert(
                compact_id,
                source_discovery_symbol_fingerprint(symbol.alias_target(), symbol.declarations()),
            );
        } else {
            cache.aliases.remove(id.as_str());
            cache.source_discovery_symbol_semantics.remove(id.as_str());
        }
    }
    for root in &affected_roots {
        cache.source_declarations.remove(root);
        cache.symbol_names.remove(root);
        for id in cache.symbols_by_root.get(root).into_iter().flatten() {
            let Some(symbol) = symbols_by_id.get(id.as_str()) else {
                continue;
            };
            if !cache.source_declarations.contains_key(root)
                && let Some(declaration) = symbol
                    .declarations()
                    .iter()
                    .find(|declaration| !declaration.location.path.ends_with(".d.ts"))
            {
                cache
                    .source_declarations
                    .insert(root.clone(), declaration.clone());
            }
            for declaration in symbol.declarations() {
                if solid_primitive_declaration(declaration) {
                    cache
                        .symbol_names
                        .insert(root.clone(), symbol_name(declaration.name.as_ref()));
                }
            }
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
        for entity in table.entities_for_path(path) {
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
                        .unwrap_or_else(|| cache.interner.intern(entity.symbol.as_ref())),
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

pub(super) fn async_symbol_root(symbol: &str, table: &TypeScriptTable) -> String {
    let aliases = table
        .files()
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

pub(super) fn entity_symbols(
    table: &TypeScriptTable,
    roots: &HashMap<SymbolId, SymbolId>,
    interner: &SymbolInterner,
) -> EntitySymbols {
    let mut by_path = HashMap::<String, HashMap<(u64, u64), SymbolId>>::new();
    for entity in table.entities().filter(|entity| !entity.symbol.is_empty()) {
        by_path
            .entry(entity.location.path.to_string())
            .or_default()
            .insert(
                (entity.location.start_byte, entity.location.end_byte),
                roots
                    .get(entity.symbol.as_ref())
                    .cloned()
                    .unwrap_or_else(|| interner.intern(entity.symbol.as_ref())),
            );
    }
    EntitySymbols { by_path }
}

pub(super) fn symbol_names(
    table: &TypeScriptTable,
    roots: &HashMap<SymbolId, SymbolId>,
    interner: &SymbolInterner,
) -> HashMap<SymbolId, SymbolName> {
    let mut names = HashMap::new();
    for symbol in table.symbols() {
        let root = roots
            .get(symbol.id())
            .cloned()
            .unwrap_or_else(|| interner.intern(symbol.id()));
        for declaration in symbol.declarations() {
            if solid_primitive_declaration(declaration) {
                names.insert(root.clone(), symbol_name(declaration.name.as_ref()));
            }
        }
    }
    names
}

pub(super) fn references_for_sources<'a>(
    table: &TypeScriptTable,
    symbols_by_root: &HashMap<SymbolId, Vec<SymbolId>>,
    sources: impl Iterator<Item = &'a SymbolId>,
) -> HashMap<SymbolId, Vec<Location>> {
    let mut references = HashMap::<SymbolId, Vec<Location>>::new();
    for root in sources {
        let locations = references.entry(root.clone()).or_default();
        if let Some(members) = symbols_by_root.get(root.as_str()) {
            for member in members {
                if let Some(symbol) = table.symbol(member.as_str()) {
                    locations.extend(symbol.references().cloned());
                }
            }
        } else if let Some(symbol) = table.symbol(root.as_str()) {
            locations.extend(symbol.references().cloned());
        }
    }
    for locations in references.values_mut() {
        locations.sort_by(location_order);
        locations.dedup();
    }
    references.retain(|_, locations| !locations.is_empty());
    references
}

fn solid_primitive_declaration(declaration: &Declaration) -> bool {
    // Bootstrap analysis of Solid's own implementation, where there is no
    // package import to establish provenance. Require an exact package path
    // component; substring matches would misclassify similarly named projects.
    declaration_path_is_solid_package(declaration.location.path.as_ref())
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

fn declaration_path_is_solid_package(path: &str) -> bool {
    path.replace('\\', "/")
        .split('/')
        .any(|component| matches!(component, "solid-js" | "@solidjs"))
}

#[cfg(test)]
mod tests {
    use super::declaration_path_is_solid_package;

    #[test]
    fn solid_declaration_paths_require_an_exact_package_component() {
        assert!(declaration_path_is_solid_package(
            "/project/node_modules/solid-js/dist/solid.js"
        ));
        assert!(declaration_path_is_solid_package(
            r"C:\project\node_modules\@solidjs\web\dist\web.js"
        ));
        assert!(!declaration_path_is_solid_package(
            "/project/my-solid-js-tools/createSignal.ts"
        ));
        assert!(!declaration_path_is_solid_package(
            "/project/node_modules/not-@solidjs/runtime.ts"
        ));
    }
}
