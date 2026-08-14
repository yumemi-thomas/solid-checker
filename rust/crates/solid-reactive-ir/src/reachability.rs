use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use solid_facts::ProjectFacts;
use solid_facts::core::Span;
use typefacts::Location;

use super::{
    EntitySymbols, FunctionNode, ProjectIndexes, SemanticLookup, SymbolId,
    containing_function_indexed, function_indices_by_path, location, primitive_name,
};
use crate::cache::{
    CachedReachabilityFile, ReachabilityEdge, ReachabilityTarget, SourceDiscoveryTypeScriptDelta,
    same_compiler_semantics,
};
use crate::owners::{function_is_solid_callback, returned_callback_execution_at_call};
use crate::pipeline::parallel_slice_results;
use crate::source_discovery::{source_discovery_identity, source_discovery_identity_matches};

#[derive(Clone, Debug, Eq, PartialEq)]
enum ReachabilityTopologyTarget {
    Symbol(SymbolId),
    Local(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReachabilityTopology {
    function_symbols: Vec<Option<SymbolId>>,
    roots: Vec<ReachabilityTopologyTarget>,
    edges: Vec<(Option<usize>, ReachabilityTopologyTarget)>,
    callback_edges: Vec<(Option<usize>, Vec<ReachabilityTopologyTarget>)>,
}

fn reachability_topology(
    functions: &[FunctionNode],
    roots: &[ReachabilityTarget],
    edges: &[ReachabilityEdge],
    callback_edges: &[(Option<Span>, Vec<ReachabilityTarget>)],
) -> ReachabilityTopology {
    // One map, not a `position` scan per edge: every edge and callback edge
    // resolves an owner and a target span, so scanning made building a file's
    // topology quadratic in its function count. `or_insert` keeps the first
    // index for a repeated span, exactly as the scan did.
    let mut indices_by_span = HashMap::with_capacity(functions.len());
    for (index, function) in functions.iter().enumerate() {
        indices_by_span.entry(function.span).or_insert(index);
    }
    let local = |span: Span| indices_by_span.get(&span).copied();
    let target = |target: &ReachabilityTarget| match target {
        ReachabilityTarget::Symbol(symbol) => {
            Some(ReachabilityTopologyTarget::Symbol(symbol.clone()))
        }
        ReachabilityTarget::LocalSpan(span) => local(*span).map(ReachabilityTopologyTarget::Local),
    };
    ReachabilityTopology {
        function_symbols: functions
            .iter()
            .map(|function| function.symbol.clone())
            .collect(),
        roots: roots.iter().filter_map(target).collect(),
        edges: edges
            .iter()
            .filter_map(|edge| Some((edge.owner.and_then(local), target(&edge.target)?)))
            .collect(),
        callback_edges: callback_edges
            .iter()
            .map(|(owner, targets)| {
                (
                    owner.and_then(local),
                    targets.iter().filter_map(target).collect(),
                )
            })
            .collect(),
    }
}

fn effective_reachability_topology(
    topology: &ReachabilityTopology,
    function_symbols: &HashSet<SymbolId>,
) -> ReachabilityTopology {
    let retained_target = |target: &ReachabilityTopologyTarget| match target {
        ReachabilityTopologyTarget::Local(_) => true,
        ReachabilityTopologyTarget::Symbol(symbol) => function_symbols.contains(symbol),
    };
    ReachabilityTopology {
        function_symbols: topology.function_symbols.clone(),
        roots: topology
            .roots
            .iter()
            .filter(|target| retained_target(target))
            .cloned()
            .collect(),
        edges: topology
            .edges
            .iter()
            .filter(|(_, target)| retained_target(target))
            .cloned()
            .collect(),
        callback_edges: topology
            .callback_edges
            .iter()
            .filter_map(|(owner, targets)| {
                let targets = targets
                    .iter()
                    .filter(|target| retained_target(target))
                    .cloned()
                    .collect::<Vec<_>>();
                (!targets.is_empty()).then_some((*owner, targets))
            })
            .collect(),
    }
}

fn discover_reachability_file(
    file: &solid_facts::FileFacts,
    indexes: &ProjectIndexes<'_>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> CachedReachabilityFile {
    let functions = file
        .ast
        .functions
        .iter()
        .map(|function| {
            let symbol = function.name.as_ref().and_then(|name| {
                entities
                    .get(&location(file.path.shared(), name.span))
                    .cloned()
            });
            FunctionNode {
                path: file.path.to_string(),
                span: function.span,
                body: function.body,
                name: function
                    .name
                    .as_ref()
                    .map(|name| file.source_text(name.span).unwrap_or_default().to_owned()),
                symbol,
            }
        })
        .collect::<Vec<_>>();
    let functions_by_path = function_indices_by_path(&functions);
    let call_owners = file
        .ast
        .calls
        .iter()
        .map(|call| {
            containing_function_indexed(
                &functions,
                &functions_by_path,
                file.path.as_str(),
                call.span,
            )
            .map(|index| functions[index].span)
        })
        .collect::<Vec<_>>();
    let exported_bodies = indexes
        .typescript_file(file.path.as_str())
        .into_iter()
        .flat_map(|file| file.functions.iter())
        .filter(|function| function.exported)
        .map(|function| (function.body.start_byte, function.body.end_byte))
        .collect::<HashSet<_>>();
    let mut roots = functions
        .iter()
        .filter(|function| {
            file.ast
                .functions
                .iter()
                .find(|candidate| candidate.span == function.span)
                .is_some_and(|candidate| {
                    lookup.function_is_component(file, candidate)
                        || function_is_solid_callback(
                            file,
                            candidate,
                            entities,
                            symbol_names,
                            lookup,
                        )
                })
                || exported_bodies
                    .contains(&(u64::from(function.body.start), u64::from(function.body.end)))
                || file
                    .ast
                    .exports
                    .iter()
                    .any(|export| export.span.contains(function.span))
        })
        .map(|function| ReachabilityTarget::LocalSpan(function.span))
        .collect::<Vec<_>>();
    for export in &file.ast.exports {
        if functions
            .iter()
            .any(|function| export.span.contains(function.span))
        {
            continue;
        }
        for entity in indexes.entities_for_path(file.path.as_str()) {
            let Ok(start) = u32::try_from(entity.location.start_byte) else {
                continue;
            };
            let Ok(end) = u32::try_from(entity.location.end_byte) else {
                continue;
            };
            if export.span.contains(Span::new(start, end))
                && let Some(symbol) = entities.get(&entity.location)
            {
                roots.push(ReachabilityTarget::Symbol(symbol.clone()));
            }
        }
    }
    let mut edges = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let owner = call_owners[call_index];
        let callee = location(file.path.shared(), call.callee);
        if let Some(symbol) = entities.get(&callee) {
            edges.push(ReachabilityEdge {
                owner,
                target: ReachabilityTarget::Symbol(symbol.clone()),
            });
        }
        let primitive = primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
            lookup.dialect,
        )
        .as_ref()
        .and_then(super::PrimitiveName::primitive);
        if primitive.is_some()
            || (0..call.arguments.len()).any(|index| {
                returned_callback_execution_at_call(file, call, index, lookup).is_some()
            })
        {
            // Every argument of a matched primitive call, not only the ones
            // carrying a callback-execution fact. The runtime invokes functions
            // it finds in options objects too -- `createMemo(fn, { equals: cmp
            // })` calls `cmp` -- and the dialect's callback table models
            // positional callbacks only, so narrowing this to modelled
            // positions would make every such comparator unreachable and
            // report it as dead code.
            for function in &functions {
                if call
                    .arguments
                    .iter()
                    .any(|argument| argument.span.contains(function.span))
                {
                    edges.push(ReachabilityEdge {
                        owner,
                        target: ReachabilityTarget::LocalSpan(function.span),
                    });
                }
            }
            for property in call
                .arguments
                .iter()
                .flat_map(|argument| &argument.identifier_properties)
            {
                if let Some(symbol) = entities.get(&location(file.path.shared(), property.span)) {
                    edges.push(ReachabilityEdge {
                        owner,
                        target: ReachabilityTarget::Symbol(symbol.clone()),
                    });
                } else if let Some(function) = functions
                    .iter()
                    .find(|function| function.name.as_deref() == file.source_text(property.span))
                {
                    edges.push(ReachabilityEdge {
                        owner,
                        target: ReachabilityTarget::LocalSpan(function.span),
                    });
                }
            }
        }
    }
    let mut callback_edges = Vec::new();
    for callback in &file.compiler.callback_roles {
        let owner = containing_function_indexed(
            &functions,
            &functions_by_path,
            file.path.as_str(),
            callback.span,
        )
        .map(|index| functions[index].span);
        let mut targets = functions
            .iter()
            .filter(|function| callback.span.contains(function.span))
            .map(|function| ReachabilityTarget::LocalSpan(function.span))
            .collect::<Vec<_>>();
        if let Some(symbol) = entities.get(&location(file.path.shared(), callback.span)) {
            targets.push(ReachabilityTarget::Symbol(symbol.clone()));
        }
        callback_edges.push((owner, targets));
    }
    // As in `reachability_topology`: one map rather than a scan per call.
    let mut function_indices_by_span = HashMap::with_capacity(functions.len());
    for (index, function) in functions.iter().enumerate() {
        function_indices_by_span
            .entry(function.span)
            .or_insert(index);
    }
    let call_owner_indices = call_owners
        .iter()
        .map(|owner| owner.and_then(|span| function_indices_by_span.get(&span).copied()))
        .collect();
    let topology = reachability_topology(&functions, &roots, &edges, &callback_edges);
    CachedReachabilityFile {
        identity: source_discovery_identity(file, indexes),
        cross_file_proofs: lookup.cross_file_proof_digest(),
        compiler: file.compiler.clone(),
        functions,
        roots,
        edges,
        callback_edges,
        call_owners,
        call_owner_indices,
        topology,
    }
}

pub(super) struct ReachabilityInputs<'a> {
    pub(super) facts: &'a ProjectFacts,
    pub(super) indexes: &'a ProjectIndexes<'a>,
    pub(super) entities: &'a EntitySymbols,
    pub(super) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(super) typescript_unchanged: bool,
    pub(super) typescript_delta: Option<&'a SourceDiscoveryTypeScriptDelta>,
    pub(super) lookup: &'a SemanticLookup<'a>,
}

pub(super) struct ReachabilityState<'a> {
    pub(super) files: &'a mut HashMap<solid_facts::core::SourcePath, CachedReachabilityFile>,
    pub(super) multiplicity_by_path: &'a mut HashMap<String, Vec<usize>>,
    pub(super) calls: &'a mut HashMap<Location, usize>,
    pub(super) function_symbols: &'a mut HashSet<SymbolId>,
}

/// The reachability stage as the pipeline runs it: decides reuse against the
/// retained cache (patching retained identities on a hit), recomputes
/// incrementally on a miss, and computes fresh when no cache is retained.
/// Returns the owned call table only in the cache-less case; with a cache,
/// the calls live in the slot the caller handed in.
pub(super) fn reachability_stage(
    inputs: ReachabilityInputs<'_>,
    cache: Option<&mut Option<crate::cache::CachedReachability>>,
    build_timings: &mut crate::BuildTimings,
) -> Option<HashMap<Location, usize>> {
    let facts = inputs.facts;
    let Some(cache) = cache else {
        let substage_started = std::time::Instant::now();
        let calls = reachable_call_multiplicity(
            facts,
            inputs.indexes,
            inputs.entities,
            inputs.symbol_names,
            inputs.lookup,
        );
        build_timings.reachability = substage_started.elapsed();
        return Some(calls);
    };
    let can_reuse = inputs.typescript_unchanged
        && cache.as_ref().is_some_and(|cached| {
            cached.inputs.len() == facts.files.len()
                && facts.files.iter().all(|file| {
                    cached
                        .inputs
                        .get(file.path.as_str())
                        .is_some_and(|(source_hash, ast)| {
                            source_hash == &file.source_hash
                                || crate::cache::same_reachability_ast(ast, &file.ast)
                        })
                })
        });
    if can_reuse {
        let cached = cache.as_mut().expect("checked retained reachability");
        for file in &facts.files {
            if let Some((source_hash, _)) = cached.inputs.get_mut(file.path.as_str()) {
                source_hash.clone_from(&file.source_hash);
            }
            if let Some(retained_file) = cached.files.get_mut(file.path.as_str()) {
                retained_file
                    .identity
                    .source_hash
                    .clone_from(&file.source_hash);
            }
        }
        build_timings.reachability_reused = true;
    } else {
        let substage_started = std::time::Instant::now();
        let cached = cache.get_or_insert_with(|| crate::cache::CachedReachability {
            inputs: HashMap::new(),
            files: HashMap::new(),
            calls: HashMap::new(),
            multiplicity_by_path: HashMap::new(),
            function_symbols: HashSet::new(),
        });
        let (reused_files, recomputed_files) = reachable_call_multiplicity_incremental(
            inputs,
            ReachabilityState {
                files: &mut cached.files,
                multiplicity_by_path: &mut cached.multiplicity_by_path,
                calls: &mut cached.calls,
                function_symbols: &mut cached.function_symbols,
            },
        );
        build_timings.reachability = substage_started.elapsed();
        build_timings.reachability_reused_files = reused_files;
        build_timings.reachability_recomputed_files = recomputed_files;
        cached.inputs = facts
            .files
            .iter()
            .map(|file| {
                (
                    file.path.to_string(),
                    (file.source_hash.clone(), file.ast.clone()),
                )
            })
            .collect();
    }
    None
}

pub(super) fn reachable_call_multiplicity_incremental(
    inputs: ReachabilityInputs<'_>,
    state: ReachabilityState<'_>,
) -> (u64, u64) {
    let ReachabilityInputs {
        facts,
        indexes,
        entities,
        symbol_names,
        typescript_unchanged,
        typescript_delta,
        lookup,
    } = inputs;
    let ReachabilityState {
        files: cache,
        multiplicity_by_path,
        calls,
        function_symbols,
    } = state;
    let current_paths = facts
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    let removed_paths = cache
        .keys()
        .filter(|path| !current_paths.contains(path.as_str()))
        .cloned()
        .collect::<HashSet<_>>();
    cache.retain(|path, _| current_paths.contains(path.as_str()));
    multiplicity_by_path.retain(|path, _| current_paths.contains(path.as_str()));
    let mut reused_files = 0;
    let mut recomputed_files = 0;
    let mut recomputed_paths = HashSet::<String>::new();
    let mut topology_unchanged = !multiplicity_by_path.is_empty() && removed_paths.is_empty();
    let mut recomputed = Vec::new();
    for file in &facts.files {
        let reusable = (typescript_unchanged || typescript_delta.is_some())
            && cache.get(file.path.as_str()).is_some_and(|cached| {
                source_discovery_identity_matches(
                    &cached.identity,
                    file.path.as_str(),
                    &file.source_hash,
                    typescript_unchanged,
                    typescript_delta,
                ) && cached.cross_file_proofs == lookup.cross_file_proof_digest()
                    && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                        || same_compiler_semantics(&cached.compiler, &file.compiler))
            });
        if reusable {
            reused_files += 1;
            continue;
        }
        recomputed_files += 1;
        recomputed_paths.insert(file.path.to_string());
        recomputed.push(file);
    }
    for (path, discovered) in parallel_slice_results(&recomputed, |file| {
        (
            file.path.clone(),
            discover_reachability_file(file, indexes, entities, symbol_names, lookup),
        )
    }) {
        topology_unchanged &= cache.get(path.as_str()).is_some_and(|previous| {
            effective_reachability_topology(&previous.topology, function_symbols)
                == effective_reachability_topology(&discovered.topology, function_symbols)
        });
        cache.insert(path, discovered);
    }

    if topology_unchanged {
        calls.retain(|location, _| {
            !removed_paths.contains(location.path.as_ref())
                && !recomputed_paths.contains(location.path.as_ref())
        });
        for file in facts
            .files
            .iter()
            .filter(|file| recomputed_paths.contains(file.path.as_str()))
        {
            let Some(cached) = cache.get(file.path.as_str()) else {
                continue;
            };
            let Some(multiplicity) = multiplicity_by_path.get(file.path.as_str()) else {
                topology_unchanged = false;
                break;
            };
            for ((call, owner), owner_index) in file
                .ast
                .calls
                .iter()
                .zip(&cached.call_owners)
                .zip(&cached.call_owner_indices)
            {
                let Some(owner_index) = owner_index else {
                    continue;
                };
                if multiplicity.get(*owner_index).copied().unwrap_or(0) != 0 {
                    calls.insert(
                        location(file.path.shared(), call.callee),
                        multiplicity[*owner_index],
                    );
                }
                debug_assert_eq!(
                    owner.and_then(|span| {
                        cached
                            .functions
                            .iter()
                            .position(|function| function.span == span)
                    }),
                    Some(*owner_index)
                );
            }
        }
        if topology_unchanged {
            return (reused_files, recomputed_files);
        }
    }

    let mut functions = Vec::new();
    for file in &facts.files {
        if let Some(cached) = cache.get(file.path.as_str()) {
            functions.extend(cached.functions.iter().cloned());
        }
    }
    let functions_by_path = function_indices_by_path(&functions);
    let by_symbol = functions
        .iter()
        .enumerate()
        .filter_map(|(index, function)| function.symbol.clone().map(|symbol| (symbol, index)))
        .collect::<HashMap<_, _>>();
    function_symbols.clear();
    function_symbols.extend(by_symbol.keys().cloned());
    let local_target = |path: &str, span: Span| {
        functions_by_path.get(path).and_then(|indices| {
            indices
                .iter()
                .find(|index| functions[**index].span == span)
                .copied()
        })
    };
    let resolve_target = |path: &str, target: &ReachabilityTarget| match target {
        ReachabilityTarget::Symbol(symbol) => by_symbol.get(symbol).copied(),
        ReachabilityTarget::LocalSpan(span) => local_target(path, *span),
    };
    let mut edges = vec![Vec::new(); functions.len()];
    let mut roots = Vec::new();
    for file in &facts.files {
        let Some(cached) = cache.get(file.path.as_str()) else {
            continue;
        };
        roots.extend(
            cached
                .roots
                .iter()
                .filter_map(|target| resolve_target(file.path.as_str(), target)),
        );
        for edge in &cached.edges {
            let Some(target) = resolve_target(file.path.as_str(), &edge.target) else {
                continue;
            };
            if let Some(owner) = edge
                .owner
                .and_then(|span| local_target(file.path.as_str(), span))
            {
                edges[owner].push(target);
            } else {
                roots.push(target);
            }
        }
        for (owner, targets) in &cached.callback_edges {
            let mut targets = targets
                .iter()
                .filter_map(|target| resolve_target(file.path.as_str(), target))
                .collect::<Vec<_>>();
            targets.sort_unstable();
            targets.dedup();
            for target in targets {
                if let Some(owner) = owner.and_then(|span| local_target(file.path.as_str(), span)) {
                    edges[owner].push(target);
                } else {
                    roots.push(target);
                }
            }
        }
    }
    roots.sort_unstable();
    roots.dedup();
    let mut multiplicity = vec![0_usize; functions.len()];
    for root in roots {
        accumulate_function(root, &edges, &mut HashSet::new(), &mut multiplicity);
    }
    multiplicity_by_path.clear();
    let mut offset = 0;
    for file in &facts.files {
        let count = cache
            .get(file.path.as_str())
            .map_or(0, |cached| cached.functions.len());
        multiplicity_by_path.insert(
            file.path.to_string(),
            multiplicity[offset..offset + count].to_vec(),
        );
        offset += count;
    }
    calls.clear();
    for file in &facts.files {
        let Some(cached) = cache.get(file.path.as_str()) else {
            continue;
        };
        for (call, owner) in file.ast.calls.iter().zip(&cached.call_owners) {
            if let Some(function) = owner.and_then(|span| local_target(file.path.as_str(), span))
                && multiplicity[function] != 0
            {
                calls.insert(
                    location(file.path.shared(), call.callee),
                    multiplicity[function],
                );
            }
        }
    }
    (reused_files, recomputed_files)
}

/// Reachability for a build with no cache to answer from.
///
/// Deliberately not a second implementation of the edge rules. Every rule --
/// which functions are roots, which arguments of a matched primitive call reach
/// a function, how an options object's named callbacks are followed -- used to
/// exist twice, once here and once in [`discover_reachability_file`], and the
/// two copies drifted: the options-object `identifier_properties` edges and the
/// export-declaration root were added to the fragment pass only, so the same
/// project facts produced a different IR depending on which builder ran.
///
/// Running the incremental pass over an empty cache instead keeps one copy of
/// every rule. With no cached fragment to match, no file is reusable and the
/// empty multiplicity table forces the full graph assembly, so this reduces to
/// the from-scratch computation by construction.
pub(super) fn reachable_call_multiplicity<'a>(
    facts: &'a ProjectFacts,
    indexes: &'a ProjectIndexes<'a>,
    entities: &'a EntitySymbols,
    symbol_names: &'a HashMap<SymbolId, SymbolId>,
    lookup: &'a SemanticLookup<'a>,
) -> HashMap<Location, usize> {
    let mut files = HashMap::new();
    let mut multiplicity_by_path = HashMap::new();
    let mut calls = HashMap::new();
    let mut function_symbols = HashSet::new();
    reachable_call_multiplicity_incremental(
        ReachabilityInputs {
            facts,
            indexes,
            entities,
            symbol_names,
            typescript_unchanged: false,
            typescript_delta: None,
            lookup,
        },
        ReachabilityState {
            files: &mut files,
            multiplicity_by_path: &mut multiplicity_by_path,
            calls: &mut calls,
            function_symbols: &mut function_symbols,
        },
    );
    calls
}

fn accumulate_function(
    function: usize,
    edges: &[Vec<usize>],
    visiting: &mut HashSet<usize>,
    multiplicity: &mut [usize],
) {
    if !visiting.insert(function) {
        return;
    }
    multiplicity[function] += 1;
    for target in &edges[function] {
        accumulate_function(*target, edges, visiting, multiplicity);
    }
    visiting.remove(&function);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_reachability_topology_ignores_only_unresolved_symbols() {
        let topology = ReachabilityTopology {
            function_symbols: vec![Some("owner".into())],
            roots: vec![
                ReachabilityTopologyTarget::Symbol("known".into()),
                ReachabilityTopologyTarget::Symbol("unresolved".into()),
                ReachabilityTopologyTarget::Local(0),
            ],
            edges: vec![
                (Some(0), ReachabilityTopologyTarget::Symbol("known".into())),
                (
                    Some(0),
                    ReachabilityTopologyTarget::Symbol("unresolved".into()),
                ),
            ],
            callback_edges: vec![
                (
                    Some(0),
                    vec![
                        ReachabilityTopologyTarget::Symbol("unresolved".into()),
                        ReachabilityTopologyTarget::Local(0),
                    ],
                ),
                (
                    Some(0),
                    vec![ReachabilityTopologyTarget::Symbol("unresolved".into())],
                ),
            ],
        };

        assert_eq!(
            effective_reachability_topology(&topology, &HashSet::from(["known".into()])),
            ReachabilityTopology {
                function_symbols: vec![Some("owner".into())],
                roots: vec![
                    ReachabilityTopologyTarget::Symbol("known".into()),
                    ReachabilityTopologyTarget::Local(0),
                ],
                edges: vec![(Some(0), ReachabilityTopologyTarget::Symbol("known".into()),)],
                callback_edges: vec![(Some(0), vec![ReachabilityTopologyTarget::Local(0)],)],
            }
        );
    }
}
