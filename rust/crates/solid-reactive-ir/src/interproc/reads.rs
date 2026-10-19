//! Direct and typed reference contributions and the interprocedural read solver.

use super::*;

pub(super) fn cached_reactive_source(
    symbol: &str,
    display: &str,
    declaration: &Location,
    source_phases: &HashMap<SymbolId, u8>,
) -> CachedReactiveSource {
    CachedReactiveSource {
        symbol: SymbolId::from(symbol),
        display: SymbolId::from(display),
        declaration: declaration.clone(),
        phase: source_phases.get(symbol).copied().unwrap_or(1),
    }
}

pub(super) fn reactive_source_order(
    left: &CachedReactiveSource,
    right: &CachedReactiveSource,
) -> std::cmp::Ordering {
    left.phase
        .cmp(&right.phase)
        .then_with(|| location_order(&left.declaration, &right.declaration))
}

pub(super) fn retained_reactive_sources(
    cache: &mut Option<Arc<Vec<CachedReactiveSource>>>,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
    summary_source_symbols: &HashSet<SymbolId>,
    source_phases: &HashMap<SymbolId, u8>,
) -> Arc<Vec<CachedReactiveSource>> {
    let eligible = |symbol: &str| summary_source_symbols.contains(symbol);
    let matches = |source: &CachedReactiveSource| {
        eligible(source.symbol.as_str())
            && accessors
                .get(source.symbol.as_str())
                .is_some_and(|(display, declaration)| {
                    display == &source.display
                        && declaration == &source.declaration
                        && source.phase
                            == source_phases
                                .get(source.symbol.as_str())
                                .copied()
                                .unwrap_or(1)
                })
    };
    let eligible_count = accessors
        .keys()
        .filter(|symbol| eligible(symbol.as_str()))
        .count();
    if let Some(retained) = cache.as_ref()
        && retained.len() == eligible_count
        && retained.iter().all(matches)
    {
        return retained.clone();
    }

    if cache.is_none() {
        let mut sources = accessors
            .iter()
            .filter(|(symbol, _)| eligible(symbol.as_str()))
            .map(|(symbol, (display, declaration))| {
                cached_reactive_source(symbol, display, declaration, source_phases)
            })
            .collect::<Vec<_>>();
        sources.sort_by(reactive_source_order);
        let sources = Arc::new(sources);
        *cache = Some(sources.clone());
        return sources;
    }

    let retained = Arc::make_mut(cache.as_mut().expect("reactive sources initialized"));
    retained.retain(&matches);
    let mut retained_symbols = retained
        .iter()
        .map(|source| source.symbol.clone())
        .collect::<HashSet<_>>();
    for (symbol, (display, declaration)) in accessors {
        if !eligible(symbol.as_str()) || retained_symbols.contains(symbol.as_str()) {
            continue;
        }
        let source = cached_reactive_source(symbol, display, declaration, source_phases);
        let insert_at = retained.partition_point(|current| {
            reactive_source_order(current, &source) != std::cmp::Ordering::Greater
        });
        retained.insert(insert_at, source);
        retained_symbols.insert(symbol.clone());
    }
    cache.as_ref().expect("reactive sources retained").clone()
}

pub(super) fn direct_reference_contributions(
    source: &CachedReactiveSource,
    context: &InterproceduralContext<'_, '_>,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
) -> Vec<DirectReferenceContribution> {
    let InterproceduralContext {
        references_by_source,
        project_indexes,
        entities,
        symbol_names,
        source_kinds,
        lookup,
        ..
    } = context;
    let mut contributions = Vec::new();
    for reference in references_by_source
        .get(source.symbol.as_str())
        .into_iter()
        .flatten()
    {
        let Some(&file) = project_indexes.files_by_path.get(reference.path.as_ref()) else {
            continue;
        };
        let Ok(start) = u32::try_from(reference.start_byte) else {
            continue;
        };
        let Ok(end) = u32::try_from(reference.end_byte) else {
            continue;
        };
        let reference_span = Span::new(start, end);
        let Some(owner) = containing_summary_function_indexed(
            nodes,
            nodes_by_path,
            file.path.as_str(),
            reference_span,
        ) else {
            continue;
        };
        if read_escapes_synchronous_extent(
            file,
            reference_span,
            entities,
            symbol_names,
            lookup.dialect,
        ) || read_escapes_runtime_stack(file, reference_span, nodes[owner].body, lookup)
            || runs_in_retained_value_literal(file, reference_span, &nodes[owner])
            || runs_outside_owner_call(file, reference_span, nodes[owner].body, lookup)
        {
            continue;
        }
        if let Some(call) = project_indexes
            .ast_files_by_path
            .get(file.path.as_str())
            .and_then(|index| index.direct_call_by_callee(reference_span))
        {
            let read = SummaryRead {
                transported: false,
                symbol: source.symbol.clone(),
                display: source.display.clone(),
                kind: Some(
                    match source_kinds.get(source.symbol.as_str()) {
                        Some(ReactiveSourceKind::Store) => "store-path",
                        Some(ReactiveSourceKind::Accessor) | None => "accessor",
                    }
                    .into(),
                ),
                declaration: source.declaration.clone(),
                origin: location(file.path.shared(), call.span),
                origin_context: nodes[owner].name.clone().unwrap_or_default(),
                owner: nodes[owner].symbol.clone(),
                local: !site_on_every_completion(lookup, file, nodes[owner].span, call.span),
                conditional: false,
                undecided: false,
            };
            contributions.push(DirectReferenceContribution {
                owner,
                read,
                unique: false,
            });
            continue;
        }
        if source_kinds.get(source.symbol.as_str()) == Some(&ReactiveSourceKind::Store) {
            contributions.extend(
                file.ast
                    .members
                    .iter()
                    .filter(|member| member.object == reference_span)
                    // ADR 0293: only a head Get whose key warns is a read. An
                    // exempt Get whose backing runs nothing is no row; an
                    // undecided one is an undecided row (review round 1),
                    // uncertifiable at every caller and never a contract read.
                    .filter_map(|member| {
                        match crate::local_access::store_get_decision(
                            file,
                            member.span,
                            &source.declaration,
                            lookup,
                        ) {
                            crate::local_access::StoreHeadGet::Warns => Some((member, false)),
                            crate::local_access::StoreHeadGet::Exempt => None,
                            crate::local_access::StoreHeadGet::Undecided(_) => Some((member, true)),
                        }
                    })
                    .map(|(member, undecided)| DirectReferenceContribution {
                        owner,
                        read: SummaryRead {
                            transported: false,
                            symbol: source.symbol.clone(),
                            display: SymbolId::from(format!(
                                "{}.{}",
                                source.display,
                                file.source_text(member.property).unwrap_or_default()
                            )),
                            kind: Some("store-path".into()),
                            declaration: source.declaration.clone(),
                            origin: location(file.path.shared(), member.span),
                            origin_context: nodes[owner].name.clone().unwrap_or_default(),
                            owner: nodes[owner].symbol.clone(),
                            local: !site_on_every_completion(
                                lookup,
                                file,
                                nodes[owner].span,
                                member.span,
                            ),
                            conditional: false,
                            undecided,
                        },
                        unique: false,
                    }),
            );
        }
    }
    contributions
}

/// Review round 1 of ADR 0294: an undecided row for each call, in a summary
/// node's own synchronous extent, of a typed candidate whose member holds
/// function literals no summary follows
/// ([`crate::source_discovery::TypedSource::held_body_unfollowed`]). Its
/// type is no evidence of a read and the body is not followed, so a caller
/// reports the call as an uncertifiable read, never a proven one. A call in
/// a component's own body is `local_access`'s obligation instead.
pub(super) fn typed_candidate_contributions(
    file: &solid_facts::FileFacts,
    context: &InterproceduralContext<'_, '_>,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
) -> Vec<DirectReferenceContribution> {
    let InterproceduralContext {
        accessors,
        entities,
        symbol_names,
        typed_sources,
        lookup,
        ..
    } = context;
    let mut contributions = Vec::new();
    for call in &file.ast.calls {
        if lookup
            .object_member_function(file, call.callee)
            .is_some_and(|(target_file, target)| {
                nodes
                    .iter()
                    .any(|node| node.path == target_file.path.as_str() && node.span == target.span)
            })
        {
            continue;
        }
        // The candidate is keyed by the entity root its reads carry; the
        // callee's resolved declaration symbol may be another identity.
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let property = file
            .ast
            .members
            .iter()
            .find(|member| member.span == callee)
            .map(|member| member.property);
        let Some((symbol, typed)) = [
            lookup.callee_symbol(file, call.callee),
            entities
                .at(file.path.as_str(), callee)
                .map(SymbolId::as_str),
            property.and_then(|property| {
                entities
                    .at(file.path.as_str(), property)
                    .map(SymbolId::as_str)
            }),
        ]
        .into_iter()
        .flatten()
        .find_map(|symbol| {
            typed_sources
                .get(symbol)
                .filter(|typed| {
                    typed.held_body_unfollowed && typed.kind == ReactiveSourceKind::Accessor
                })
                .map(|typed| (symbol, typed))
        }) else {
            continue;
        };
        if accessors.contains_key(symbol) {
            continue;
        }
        let Some(owner) = containing_summary_function_indexed(
            nodes,
            nodes_by_path,
            file.path.as_str(),
            call.callee,
        ) else {
            continue;
        };
        if read_escapes_synchronous_extent(
            file,
            call.callee,
            entities,
            symbol_names,
            lookup.dialect,
        ) || read_escapes_runtime_stack(file, call.callee, nodes[owner].body, lookup)
            || runs_in_retained_value_literal(file, call.callee, &nodes[owner])
            || runs_outside_owner_call(file, call.callee, nodes[owner].body, lookup)
            || enclosing_render_function(file, call.callee, lookup)
        {
            continue;
        }
        let call_location = location(file.path.shared(), call.span);
        contributions.push(DirectReferenceContribution {
            owner,
            read: SummaryRead {
                transported: false,
                symbol: SymbolId::from(format!(
                    "typed:{}\0{}\0{}",
                    call_location.path, call_location.start_byte, call_location.end_byte
                )),
                display: SymbolId::from(file.source_text(call.callee).unwrap_or("accessor")),
                kind: Some("accessor".into()),
                declaration: typed.declaration.clone(),
                origin: call_location,
                origin_context: nodes[owner].name.clone().unwrap_or_default(),
                owner: nodes[owner].symbol.clone(),
                local: !site_on_every_completion(lookup, file, nodes[owner].span, call.span),
                conditional: false,
                undecided: true,
            },
            unique: false,
        });
    }
    contributions
}

pub(crate) struct InterproceduralContext<'a, 'facts> {
    pub(crate) facts: &'a ProjectFacts,
    pub(crate) project_indexes: &'a ProjectIndexes<'facts>,
    pub(crate) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) returned_source_symbols: &'a HashSet<SymbolId>,
    pub(crate) summary_source_symbols: &'a HashSet<SymbolId>,
    pub(crate) source_phases: &'a HashMap<SymbolId, u8>,
    pub(crate) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) entities: &'a EntitySymbols,
    pub(crate) references_by_source: &'a HashMap<SymbolId, Vec<Location>>,
    pub(crate) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) retained_source_paths: &'a HashSet<String>,
    /// ADR 0294's typed candidates, for [`typed_candidate_contributions`].
    pub(crate) typed_sources: &'a HashMap<SymbolId, crate::source_discovery::TypedSource>,
    pub(crate) lookup: &'a SemanticLookup<'facts>,
}

impl InterproceduralContext<'_, '_> {
    pub(crate) fn build(
        &self,
        interprocedural_graph_cache: Option<
            &mut HashMap<solid_facts::core::SourcePath, CachedInterproceduralGraph>,
        >,
        interprocedural_result_cache: Option<&mut CachedInterproceduralResults>,
    ) -> InterproceduralResult {
        interprocedural_reads(
            self,
            InterproceduralCaches {
                graph: interprocedural_graph_cache,
                results: interprocedural_result_cache,
            },
        )
    }
}

pub(super) struct InterproceduralCaches<'a> {
    pub(super) graph:
        Option<&'a mut HashMap<solid_facts::core::SourcePath, CachedInterproceduralGraph>>,
    pub(super) results: Option<&'a mut CachedInterproceduralResults>,
}

pub(super) fn interprocedural_reads(
    context: &InterproceduralContext<'_, '_>,
    caches: InterproceduralCaches<'_>,
) -> InterproceduralResult {
    let InterproceduralContext {
        facts,
        project_indexes,
        accessors,
        returned_source_symbols,
        summary_source_symbols,
        source_phases,
        source_kinds,
        entities,
        references_by_source: _,
        symbol_names,
        retained_source_paths,
        typed_sources,
        lookup,
    } = context;
    let InterproceduralCaches {
        graph: mut interprocedural_graph_cache,
        results: mut interprocedural_result_cache,
    } = caches;
    let mut phase_started = Instant::now();
    let mut nodes = Vec::new();
    let mut graph_node_reused_paths = HashSet::new();
    if let Some(cache) = interprocedural_graph_cache.as_deref_mut() {
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
                (retained_source_paths.contains(file.path.as_str())
                    && cache.get(file.path.as_str()).is_some_and(|cached| {
                        cached.member_resolution == lookup.member_resolution_digest()
                            && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                                || same_compiler_semantics(&cached.compiler, &file.compiler))
                    }))
                .then_some(file.path.as_str())
            })
            .collect::<HashSet<_>>();
        let discovered = parallel_file_results(&facts.files, |file| {
            (!reusable_paths.contains(file.path.as_str()))
                .then(|| discover_summary_nodes(file, project_indexes, entities))
        });
        for (file, discovered) in facts.files.iter().zip(discovered) {
            if let Some(file_nodes) = discovered {
                nodes.extend(file_nodes.iter().cloned());
                cache.insert(
                    file.path.clone(),
                    CachedInterproceduralGraph {
                        nodes: file_nodes,
                        contribution: InterproceduralGraphContribution::default(),
                        compiler: file.compiler.clone(),
                        member_resolution: lookup.member_resolution_digest(),
                    },
                );
            } else {
                let cached = cache
                    .get(file.path.as_str())
                    .expect("reusable graph path has a cached contribution");
                nodes.extend(cached.nodes.iter().cloned());
                graph_node_reused_paths.insert(file.path.as_str());
            }
        }
    } else {
        for file_nodes in parallel_file_results(&facts.files, |file| {
            discover_summary_nodes(file, project_indexes, entities)
        }) {
            nodes.extend(file_nodes);
        }
    }
    let provenance = accessor_provenance::Resolver::new(
        facts,
        lookup,
        accessors,
        source_kinds,
        symbol_names,
        nodes
            .iter()
            .map(|node| (node.path.clone(), node.span))
            .collect(),
    );
    let (mut transported_calls, unavailable_calls) = provenance.calls();
    // ADR 0299 review: accessor transport predates the shared exact object
    // target map. An escaped or uninitialized literal must not regain its
    // read summary through that older path after the owner edge is withheld.
    transported_calls.retain(|site, _| {
        let Some(file) = lookup.file_by_path(site.path.as_ref()) else {
            return false;
        };
        let (Ok(start), Ok(end)) = (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
        else {
            return false;
        };
        !crate::object_members::literal_receiver_call(file, Span::new(start, end), lookup)
            || lookup.object_member_targets().contains_key(site)
    });
    for (site, (path, span)) in lookup.object_member_targets() {
        transported_calls.insert(
            site.clone(),
            accessor_provenance::Callable::Function {
                path: path.clone(),
                span: *span,
            },
        );
    }
    let array_source_origins = if transported_calls
        .values()
        .any(|target| matches!(target, accessor_provenance::Callable::ArrayMembers(_)))
    {
        provenance.proven_source_origins().cloned().collect()
    } else {
        HashSet::new()
    };
    let transport_identity_matches = interprocedural_result_cache.as_ref().is_none_or(|cache| {
        cache.transported_calls == transported_calls
            && cache.unavailable_calls == unavailable_calls
            && cache.array_source_origins == array_source_origins
    });
    let mut transported_bodies = HashSet::new();
    let mut array_member_bodies = HashSet::new();
    for target in transported_calls.values() {
        match target {
            accessor_provenance::Callable::Function { path, span } => {
                transported_bodies.insert((path.as_str(), *span));
            }
            accessor_provenance::Callable::ArrayMembers(targets) => {
                array_member_bodies.extend(targets.iter().cloned());
                transported_bodies
                    .extend(targets.iter().map(|(path, span)| (path.as_str(), *span)));
            }
            accessor_provenance::Callable::Accessor(_) => {}
        }
    }
    let nodes_by_path = function_indices_by_path(&nodes);
    let indexes = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| ((node.path.clone(), node.span), index))
        .collect::<HashMap<_, _>>();
    let by_symbol = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.symbol.clone().map(|symbol| (symbol, index)))
        .collect::<HashMap<_, _>>();
    let mut summaries = vec![SummaryReads::default(); nodes.len()];
    let mut callback_summaries = vec![Vec::<CallbackSummary>::new(); nodes.len()];
    let mut callback_forwardings = Vec::new();
    let mut dispatches = Vec::<(usize, Vec<usize>)>::new();
    let mut contract_generation_obligations =
        vec![Vec::<ContractGenerationObligation>::new(); nodes.len()];
    let mut dispatch_obligations = Vec::new();
    let mut edges = vec![Vec::<usize>::new(); nodes.len()];
    let mut guaranteed_edges = HashSet::<(usize, usize)>::new();
    let mut invoked_parameters = vec![Vec::<usize>::new(); nodes.len()];
    let mut direct_callback_parameters = vec![Vec::<usize>::new(); nodes.len()];
    let mut guaranteed_callback_parameters = vec![Vec::<usize>::new(); nodes.len()];
    let mut direct_protocol_parameters =
        vec![Vec::<(crate::summary_semantics::InvokeProtocol, usize)>::new(); nodes.len()];
    let mut direct_member_callback_parameters =
        vec![Vec::<(usize, Vec<String>)>::new(); nodes.len()];
    let mut escaped_parameters = vec![Vec::<usize>::new(); nodes.len()];
    let mut invoked_parameter_members = vec![Vec::<ParameterMemberInvocation>::new(); nodes.len()];
    let mut returned_binding_candidates = Vec::new();
    let mut factory_call_candidates = Vec::new();
    let mut graph_reused_files = 0;
    let mut graph_recomputed_files = 0;
    {
        let mut graph = InterproceduralGraphAssembly {
            nodes: &nodes,
            nodes_by_path: &nodes_by_path,
            by_symbol: &by_symbol,
            summaries: &mut summaries,
            callback_summaries: &mut callback_summaries,
            callback_forwardings: &mut callback_forwardings,
            dispatches: &mut dispatches,
            contract_generation_obligations: &mut contract_generation_obligations,
            contract_consumer_obligations: &mut dispatch_obligations,
            edges: &mut edges,
            guaranteed_edges: &mut guaranteed_edges,
            invoked_parameters: &mut invoked_parameters,
            direct_callback_parameters: &mut direct_callback_parameters,
            guaranteed_callback_parameters: &mut guaranteed_callback_parameters,
            direct_protocol_parameters: &mut direct_protocol_parameters,
            direct_member_callback_parameters: &mut direct_member_callback_parameters,
            escaped_parameters: &mut escaped_parameters,
            invoked_parameter_members: &mut invoked_parameter_members,
            returned_bindings: &mut returned_binding_candidates,
            factory_calls: &mut factory_call_candidates,
        };
        match interprocedural_graph_cache {
            None => {
                let contributions = parallel_file_results(&facts.files, |file| {
                    discover_interprocedural_graph(
                        file,
                        &nodes,
                        &nodes_by_path,
                        InterproceduralGraphSymbols {
                            entities,
                            names: symbol_names,
                        },
                        lookup,
                        accessors,
                    )
                });
                for (file, contribution) in facts.files.iter().zip(contributions) {
                    graph_recomputed_files += 1;
                    graph.merge(file.path.as_str(), &contribution);
                }
            }
            Some(cache) => {
                let contributions = parallel_file_results(&facts.files, |file| {
                    (!graph_node_reused_paths.contains(file.path.as_str())).then(|| {
                        discover_interprocedural_graph(
                            file,
                            &nodes,
                            &nodes_by_path,
                            InterproceduralGraphSymbols {
                                entities,
                                names: symbol_names,
                            },
                            lookup,
                            accessors,
                        )
                    })
                });
                for (file, contribution) in facts.files.iter().zip(contributions) {
                    if graph_node_reused_paths.contains(file.path.as_str())
                        && let Some(cached) = cache.get(file.path.as_str())
                    {
                        graph_reused_files += 1;
                        graph.merge(file.path.as_str(), &cached.contribution);
                        continue;
                    }
                    graph_recomputed_files += 1;
                    let contribution =
                        contribution.expect("recomputed graph path has a fresh contribution");
                    graph.merge(file.path.as_str(), &contribution);
                    cache.insert(
                        file.path.clone(),
                        CachedInterproceduralGraph {
                            nodes: nodes_by_path
                                .get(file.path.as_str())
                                .into_iter()
                                .flatten()
                                .map(|index| nodes[*index].clone())
                                .collect(),
                            contribution,
                            compiler: file.compiler.clone(),
                            member_resolution: lookup.member_resolution_digest(),
                        },
                    );
                }
            }
        }
    }
    // A callback consumer can change in another file while its forwarding
    // helper's cached graph contribution remains reusable. Refresh invocation
    // context from the current source census before dependency states are read.
    let mut member_consumers = crate::execution_role::ReadConsumerSummaries::new(lookup);
    for file in &facts.files {
        let allowed = allowed_callback_spans(file, lookup);
        for owner in nodes_by_path.get(file.path.as_str()).into_iter().flatten() {
            for invocation in &mut invoked_parameter_members[*owner] {
                let site = invocation.site;
                let default =
                    file.ast.functions.iter().any(|function| {
                        function.span.contains(site) && !function.body.contains(site)
                    });
                let role =
                    semantic_execution_role(file, site, &allowed, entities, symbol_names, lookup);
                invocation.dispatch_read_excluded = !default
                    && (crate::execution_role::excluded_role_holds(file, site, role, lookup)
                        || member_consumers.proves_tracked_or_fresh(file, site)
                        || crate::execution_role::await_continuation_decides_nothing(
                            file, site, lookup,
                        ));
            }
        }
    }
    // ADR 0320: a function that hands its own parameter, as the whole
    // argument, to an exact project callee invoking a member of it invokes
    // that member too: the dispatch is decided by this function's callers.
    // Without this, a call written in a non-rendering helper raises its
    // obligation nowhere. Recomputed from the merged graph every build; a
    // forwarded invocation's context is the forwarding call's.
    let mut site_excluded = |file: &solid_facts::FileFacts, site: Span| {
        let allowed = allowed_callback_spans(file, lookup);
        let default = file
            .ast
            .functions
            .iter()
            .any(|function| function.span.contains(site) && !function.body.contains(site));
        let role = semantic_execution_role(file, site, &allowed, entities, symbol_names, lookup);
        !default
            && (crate::execution_role::excluded_role_holds(file, site, role, lookup)
                || member_consumers.proves_tracked_or_fresh(file, site)
                || crate::execution_role::await_continuation_decides_nothing(file, site, lookup))
    };
    unmodeled_parameter_uses(
        facts,
        &nodes,
        &nodes_by_path,
        lookup,
        &mut invoked_parameter_members,
        &mut site_excluded,
    );
    forward_parameter_member_invocations(
        facts,
        &nodes,
        &nodes_by_path,
        lookup,
        &mut invoked_parameter_members,
        |file, site| {
            let allowed = allowed_callback_spans(file, lookup);
            let default = file
                .ast
                .functions
                .iter()
                .any(|function| function.span.contains(site) && !function.body.contains(site));
            let role =
                semantic_execution_role(file, site, &allowed, entities, symbol_names, lookup);
            !default
                && (crate::execution_role::excluded_role_holds(file, site, role, lookup)
                    || member_consumers.proves_tracked_or_fresh(file, site)
                    || crate::execution_role::await_continuation_decides_nothing(
                        file, site, lookup,
                    ))
        },
    );
    // These edges depend on other files' returned literals and the entire
    // provider census. Recompute them after merging file-local cached graph
    // contributions, so a provider/factory edit cannot retain a stale edge.
    let mut transported_edges = HashSet::new();
    for file in &facts.files {
        for call in &file.ast.calls {
            let Some(owner) = containing_summary_function_indexed(
                &nodes,
                &nodes_by_path,
                file.path.as_str(),
                call.span,
            ) else {
                continue;
            };
            let selected_body =
                transported_bodies.contains(&(nodes[owner].path.as_str(), nodes[owner].span));
            let source = selected_body
                .then(|| provenance.source_call(file, call.callee))
                .flatten();
            let source_target = source.map(accessor_provenance::Callable::Accessor);
            let Some(target) = transported_calls
                .get(&location(file.path.shared(), call.callee))
                .or(source_target.as_ref())
            else {
                continue;
            };
            // Transported targets obey the same callback listener boundary
            // as direct source references and ordinary helper edges.
            if read_escapes_synchronous_extent(
                file,
                call.span,
                entities,
                symbol_names,
                lookup.dialect,
            ) || read_escapes_runtime_stack(file, call.span, nodes[owner].body, lookup)
                || runs_in_retained_value_literal(file, call.span, &nodes[owner])
                || runs_outside_owner_call(file, call.span, nodes[owner].body, lookup)
            {
                continue;
            }
            match target {
                accessor_provenance::Callable::Accessor(symbol) => {
                    if let Some(mut read) = provenance.read(symbol, file, call.span) {
                        read.owner = nodes[owner].symbol.clone();
                        read.local =
                            !site_on_every_completion(lookup, file, nodes[owner].span, call.span);
                        summaries[owner].push_unique(read);
                    }
                }
                accessor_provenance::Callable::Function { path, span } => {
                    if let Some(target) = indexes.get(&(path.clone(), *span))
                        && !edges[owner].contains(target)
                    {
                        transported_edges.insert((owner, *target));
                        edges[owner].push(*target);
                        if site_on_every_completion(lookup, file, nodes[owner].span, call.span) {
                            guaranteed_edges.insert((owner, *target));
                        }
                    }
                }
                accessor_provenance::Callable::ArrayMembers(targets) => {
                    for target in targets {
                        if let Some(target) = indexes.get(target)
                            && !edges[owner].contains(target)
                        {
                            transported_edges.insert((owner, *target));
                            edges[owner].push(*target);
                            if site_on_every_completion(lookup, file, nodes[owner].span, call.span)
                            {
                                guaranteed_edges.insert((owner, *target));
                            }
                        }
                    }
                }
            }
        }
    }
    // Extra memo-wrapper source origins also enter through ordinary discovery.
    // Tag those rows before any normal summary hop can copy them.
    if provenance.has_extra_source_origins() {
        for summary in &mut summaries {
            for read in &mut summary.ordered {
                read.transported |= provenance.has_source_origin(&read.symbol);
            }
        }
    }
    let mut contract_generation_obligation_keys = contract_generation_obligations
        .iter()
        .map(|obligations| {
            obligations
                .iter()
                .map(|obligation| (obligation.parameter, obligation.location.clone()))
                .collect::<HashSet<_>>()
        })
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for (owner, target, target_parameter, owner_parameter, ambient_execution) in
            &callback_forwardings
        {
            // Forwarding a callback into a local callee inherits that callee's
            // *whole* answer for the slot, and "the callee never accounted for
            // this parameter" is part of it. Without this hop the forwarding
            // silently upgrades an unknown into the empty (negative) claim.
            let target_escaped = escaped_parameters[*target].contains(target_parameter)
                || (escaped_parameters[*target].contains(&REST_PARAMETER)
                    && *target_parameter >= nodes[*target].parameters.len());
            if target_escaped && !escaped_parameters[*owner].contains(owner_parameter) {
                escaped_parameters[*owner].push(*owner_parameter);
                changed = true;
            }
            for callback in callback_summaries[*target]
                .iter()
                .filter(|callback| callback.parameter == *target_parameter)
                .cloned()
                .collect::<Vec<_>>()
            {
                // ADR 0139: a `result-access` row says the *callee* keeps the
                // callable only in what it returns. That is not a fact about
                // the owner, which may keep that value anywhere, so the row is
                // never restated: the owner's slot is opened instead.
                if callback.is_result_access() {
                    if !escaped_parameters[*owner].contains(owner_parameter) {
                        escaped_parameters[*owner].push(*owner_parameter);
                        changed = true;
                    }
                    continue;
                }
                // Only an `inline` callee row is relative to the callee's own
                // call and therefore needs restating; `tracked` and `deferred`
                // survive any wrapper. `Unknown` refuses to restate it, and the
                // row is dropped rather than republished -- the obligation that
                // came with the forwarding opens the sentinel for the export.
                if callback.execution == "inline"
                    && *ambient_execution == ForwardedAmbientExecution::Unknown
                {
                    continue;
                }
                let restated = match (callback.execution.as_str(), ambient_execution) {
                    ("inline", ForwardedAmbientExecution::Composed { .. }) => {
                        let Some(restated) = ambient_execution.restate(callback.clears_tracking)
                        else {
                            // The callee's clearing meets a tracked wrapper
                            // with no stated schedule: no word is honest. The
                            // forwarding call opened no sentinel for this --
                            // it could not know the callee cleared -- so it is
                            // opened here, and the row is not republished.
                            if !escaped_parameters[*owner].contains(owner_parameter) {
                                escaped_parameters[*owner].push(*owner_parameter);
                                changed = true;
                            }
                            continue;
                        };
                        Some(restated)
                    }
                    _ => None,
                };
                let (execution, schedule, clears_tracking) = restated.unwrap_or((
                    callback.execution,
                    callback.schedule,
                    callback.clears_tracking,
                ));
                let forwarded = CallbackSummary {
                    parameter: *owner_parameter,
                    execution,
                    schedule,
                    clears_tracking,
                    arguments: callback.arguments.clone(),
                    owner: callback.owner.clone(),
                    protocol: callback.protocol,
                    path: callback.path.clone(),
                };
                if !callback_summaries[*owner].contains(&forwarded) {
                    callback_summaries[*owner].push(forwarded);
                    changed = true;
                }
            }
            for obligation in contract_generation_obligations[*target]
                .iter()
                .filter(|obligation| obligation.parameter == *target_parameter)
                .cloned()
                .collect::<Vec<_>>()
            {
                let forwarded = ContractGenerationObligation {
                    function: nodes[*owner]
                        .name
                        .clone()
                        .unwrap_or_else(|| "<anonymous>".into()),
                    function_symbol: nodes[*owner]
                        .symbol
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                    function_identity: nodes[*owner].runtime_identity.clone(),
                    parameter: *owner_parameter,
                    package: obligation.package.clone(),
                    entrypoint: obligation.entrypoint.clone(),
                    parameter_type: obligation.parameter_type.clone(),
                    required_execution: obligation.required_execution.clone(),
                    claim_context: obligation.claim_context.clone(),
                    location: obligation.location,
                    message: format!(
                        "parameter {owner_parameter} reaches unresolved behavior through {}; {}",
                        nodes[*target].name.as_deref().unwrap_or("<anonymous>"),
                        obligation.message
                    ),
                };
                let key = (forwarded.parameter, forwarded.location.clone());
                if contract_generation_obligation_keys[*owner].insert(key) {
                    contract_generation_obligations[*owner].push(forwarded);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let graph = phase_started.elapsed();
    phase_started = Instant::now();
    let owned_reactive_sources;
    let reactive_sources = if let Some(cache) = interprocedural_result_cache.as_deref_mut() {
        owned_reactive_sources = retained_reactive_sources(
            &mut cache.reactive_sources,
            accessors,
            summary_source_symbols,
            source_phases,
        );
        owned_reactive_sources.as_slice()
    } else {
        owned_reactive_sources = {
            let mut sources = accessors
                .iter()
                .filter(|(symbol, _)| summary_source_symbols.contains(*symbol))
                .map(|(symbol, (display, declaration))| {
                    cached_reactive_source(symbol, display, declaration, source_phases)
                })
                .collect::<Vec<_>>();
            sources.sort_by(reactive_source_order);
            Arc::new(sources)
        };
        owned_reactive_sources.as_slice()
    };
    let direct_index = phase_started.elapsed();
    let direct_references_started = Instant::now();
    for contributions in parallel_slice_results(reactive_sources, |source| {
        direct_reference_contributions(source, context, &nodes, &nodes_by_path)
    }) {
        for contribution in contributions {
            if contribution.unique {
                summaries[contribution.owner].push_unique(contribution.read);
            } else {
                summaries[contribution.owner].push(contribution.read);
            }
        }
    }
    if typed_sources
        .values()
        .any(|typed| typed.held_body_unfollowed)
    {
        for file in &facts.files {
            for contribution in typed_candidate_contributions(file, context, &nodes, &nodes_by_path)
            {
                summaries[contribution.owner].push(contribution.read);
            }
        }
    }
    let direct_references = direct_references_started.elapsed();
    let direct_summaries = phase_started.elapsed();
    phase_started = Instant::now();
    let mut returned = vec![SummaryReads::default(); nodes.len()];
    let mut returned_edges = Vec::<(usize, usize)>::new();
    let returns_by_owner = items_by_containing_function(
        &nodes,
        &nodes_by_path,
        facts.files.iter().flat_map(|file| {
            file.ast
                .returns
                .iter()
                .map(move |returned| (file.path.as_str(), returned))
        }),
        |returned| returned.span,
    );
    let mut returns_by_ast_owner = HashMap::new();
    for file in &facts.files {
        for returned in &file.ast.returns {
            if let Some(function) = containing_ast_function(&file.ast, returned.span) {
                returns_by_ast_owner
                    .entry((file.path.as_str(), function.span))
                    .or_insert_with(Vec::new)
                    .push(returned);
            }
        }
    }
    for (index, node) in nodes.iter().enumerate() {
        let Some(&file) = project_indexes.files_by_path.get(node.path.as_str()) else {
            continue;
        };
        let returned_closures = returns_by_owner[index]
            .iter()
            .copied()
            .flat_map(|returned| {
                returned
                    .argument
                    .into_iter()
                    .flat_map(|argument| returned_function_spans(file, argument, entities))
            })
            .collect::<Vec<_>>();
        for closure in &returned_closures {
            if let Some(target) = indexes.get(&(node.path.clone(), *closure)) {
                returned_edges.push((index, *target));
            }
        }
        for returned_value in &returns_by_owner[index] {
            match returned_value.value {
                solid_facts::ast::ReturnValueKind::Identifier => {
                    let returned_location = location(file.path.shared(), returned_value.span);
                    if let Some(symbol) = entities.get(&returned_location) {
                        if returned_source_symbols.contains(symbol)
                            && let Some((display, declaration)) = accessors.get(symbol)
                        {
                            returned[index].push_unique(SummaryRead {
                                transported: false,
                                symbol: symbol.clone(),
                                display: display.clone(),
                                kind: Some(
                                    match source_kinds.get(symbol.as_str()) {
                                        Some(ReactiveSourceKind::Store) => "store-path",
                                        Some(ReactiveSourceKind::Accessor) | None => "accessor",
                                    }
                                    .into(),
                                ),
                                declaration: declaration.clone(),
                                origin: returned_location,
                                origin_context: node.name.clone().unwrap_or_default(),
                                owner: None,
                                local: false,
                                conditional: false,
                                undecided: false,
                            });
                        } else if let Some(target) = by_symbol.get(symbol).copied()
                            && target != index
                        {
                            for read in summaries[target].iter() {
                                returned[index].push_unique(read.clone());
                            }
                        }
                    }
                }
                solid_facts::ast::ReturnValueKind::Call => {
                    let Some(callee) = returned_value.callee else {
                        continue;
                    };
                    let Some(call) = project_indexes
                        .ast_files_by_path
                        .get(file.path.as_str())
                        .into_iter()
                        .flat_map(|index| index.calls_by_callee(callee))
                        .find(|call| {
                            returned_value.argument.is_some_and(|argument| {
                                file.ast.peel_ts_sugar_span(argument) == call.span
                            })
                        })
                    else {
                        continue;
                    };
                    let callee_location = location(file.path.shared(), call.callee);
                    if let Some(symbol) = entities.get(&callee_location)
                        && let Some(target) = by_symbol.get(symbol).copied()
                    {
                        returned_edges.push((index, target));
                    }
                }
                _ => {}
            }
        }
        if returned_closures.is_empty() {
            continue;
        }
        let mut direct = Vec::with_capacity(summaries[index].len());
        for read in summaries[index].take() {
            let in_returned_closure = returned_closures.iter().any(|closure| {
                read.origin.path == node.path.clone().into()
                    && u64::from(closure.start) <= read.origin.start_byte
                    && read.origin.end_byte <= u64::from(closure.end)
            });
            if in_returned_closure {
                returned[index].push(read);
            } else {
                direct.push(read);
            }
        }
        summaries[index].replace(direct);
    }
    let returned_direct = phase_started.elapsed();
    let returned_delta_started = Instant::now();
    propagate_returned_summary_deltas(&mut returned, &returned_edges);
    let returned_delta = returned_delta_started.elapsed();
    let call_summary_delta_started = Instant::now();
    let mut reverse_edges = vec![Vec::new(); nodes.len()];
    for (owner, targets) in edges.iter_mut().enumerate() {
        targets.sort_unstable();
        targets.dedup();
        for target in targets.iter().copied() {
            reverse_edges[target].push(owner);
        }
    }
    let mut propagated_lengths = vec![0; summaries.len()];
    propagate_summary_deltas(
        &mut summaries,
        &reverse_edges,
        &mut propagated_lengths,
        &transported_edges,
        &guaranteed_edges,
    );

    // Composite structural dispatch is intentionally a second phase. The
    // first propagation computes each exact implementation's own summary;
    // only an equal set of summaries may then be attached to the caller. If
    // one candidate is missing or differs in reads, callbacks, or async
    // shape, the call remains uncertifiable and contributes no edge.
    let mut dispatch_edges_added = false;
    for (owner, candidates) in &dispatches {
        let Some(first) = candidates.first().copied() else {
            continue;
        };
        let equivalent = candidates.iter().all(|candidate| {
            contract_generation_obligations[*candidate].is_empty()
                && equivalent_summary_reads(&summaries[*candidate], &summaries[first])
                && equivalent_callbacks(&callback_summaries[*candidate], &callback_summaries[first])
                && invoked_parameters[*candidate] == invoked_parameters[first]
                && direct_callback_parameters[*candidate] == direct_callback_parameters[first]
                && guaranteed_callback_parameters[*candidate]
                    == guaranteed_callback_parameters[first]
                && direct_protocol_parameters[*candidate] == direct_protocol_parameters[first]
                && direct_member_callback_parameters[*candidate]
                    == direct_member_callback_parameters[first]
                && equivalent_member_invocations(
                    &invoked_parameter_members[*candidate],
                    &invoked_parameter_members[first],
                )
                && nodes[*candidate].r#async == nodes[first].r#async
        });
        if !equivalent {
            continue;
        }
        for target in candidates.iter().copied() {
            if !edges[*owner].contains(&target) {
                edges[*owner].push(target);
                reverse_edges[target].push(*owner);
                let target_reads = summaries[target].to_vec();
                for read in target_reads {
                    if summaries[*owner].push_unique(read) {
                        dispatch_edges_added = true;
                    }
                }
            }
        }
    }
    if dispatch_edges_added {
        propagate_summary_deltas(
            &mut summaries,
            &reverse_edges,
            &mut propagated_lengths,
            &transported_edges,
            &guaranteed_edges,
        );
    }

    for (index, node) in nodes.iter().enumerate() {
        let Some(&file) = project_indexes.files_by_path.get(node.path.as_str()) else {
            continue;
        };
        let Some(function) = project_indexes
            .ast_files_by_path
            .get(file.path.as_str())
            .and_then(|index| index.function_by_span(node.span))
        else {
            continue;
        };
        for value in function.expression_return.iter().chain(
            returns_by_ast_owner
                .get(&(node.path.as_str(), function.span))
                .into_iter()
                .flatten()
                .copied(),
        ) {
            if value.value == solid_facts::ast::ReturnValueKind::Function
                && let Some(target) = indexes.get(&(node.path.clone(), value.span))
            {
                for read in summaries[*target].iter() {
                    returned[index].push_unique(read.clone());
                }
            }
        }
    }
    let call_summary_delta = call_summary_delta_started.elapsed();
    let factory_propagation_started = Instant::now();
    let mut returned_bindings = HashMap::<SymbolId, Vec<SummaryRead>>::new();
    if returned.iter().any(|summary| !summary.is_empty()) {
        for (binding_symbol, target_symbol) in &returned_binding_candidates {
            let Some(target) = by_symbol.get(target_symbol).copied() else {
                continue;
            };
            let mut summary = returned[target].to_vec();
            for read in &mut summary {
                if read.origin_context.is_empty() {
                    read.origin_context = nodes[target].name.clone().unwrap_or_default();
                }
            }
            returned_bindings.insert(binding_symbol.clone(), summary);
        }
        let mut factory_reads_added = false;
        if !returned_bindings.is_empty() {
            for (owner, symbol) in &factory_call_candidates {
                if accessors.contains_key(symbol) {
                    continue;
                }
                let Some(factory_reads) = returned_bindings.get(symbol) else {
                    continue;
                };
                for read in factory_reads {
                    let previous_len = summaries[*owner].len();
                    summaries[*owner].push_unique(read.clone());
                    factory_reads_added |= summaries[*owner].len() != previous_len;
                }
            }
        }
        if factory_reads_added {
            propagate_summary_deltas(
                &mut summaries,
                &reverse_edges,
                &mut propagated_lengths,
                &transported_edges,
                &guaranteed_edges,
            );
        }
    }

    let factory_propagation = factory_propagation_started.elapsed();
    let propagation = phase_started.elapsed();
    phase_started = Instant::now();
    let result_capacity = interprocedural_result_cache.as_ref().map_or(0, |cache| {
        cache.files.values().map(|file| file.reads.len()).sum()
    });
    let mut result = Vec::with_capacity(result_capacity);
    let mut result_reused_files = 0;
    let mut result_recomputed_files = 0;
    let result_view = InterproceduralResultView {
        nodes: &nodes,
        indexes: &indexes,
        by_symbol: &by_symbol,
        summaries: &summaries,
        invoked_parameters: &invoked_parameters,
        invoked_parameter_members: &invoked_parameter_members,
        returned_bindings: &returned_bindings,
    };
    let result_read_context = InterproceduralResultReadContext {
        result: result_view,
        source_kinds,
        accessors,
        entities,
        symbol_names,
        lookup: context.lookup,
        transported_calls: &transported_calls,
        unavailable_calls: &unavailable_calls,
        array_member_bodies: &array_member_bodies,
        provenance: &provenance,
    };
    if let Some(cache) = interprocedural_result_cache {
        cache.transported_calls.clone_from(&transported_calls);
        cache.unavailable_calls.clone_from(&unavailable_calls);
        cache.array_source_origins.clone_from(&array_source_origins);
        // Call-role proofs inspect callee syntax beyond its read summary:
        // moving an await/default/edge can preserve that summary byte for
        // byte. Until syntax dependencies are retained per hop, a changed,
        // added or removed file holding such syntax, before or after the
        // edit, invalidates result-file reuse, including callers.
        let all_result_sources_retained = facts
            .files
            .iter()
            .filter(|file| !retained_source_paths.contains(file.path.as_str()))
            .all(|file| {
                !call_role_syntax(file)
                    && cache
                        .files
                        .get(file.path.as_str())
                        .is_none_or(|cached| !cached.call_role_syntax)
            })
            && cache.files.iter().all(|(path, cached)| {
                !cached.call_role_syntax
                    || facts
                        .files
                        .iter()
                        .any(|file| file.path.as_str() == path.as_str())
            });
        if cache.files.is_empty()
            && cache.dependency_states.is_empty()
            && cache.dependency_users.is_empty()
        {
            let per_file = parallel_file_results(&facts.files, |file| {
                interprocedural_result_reads_for_file(file, &result_read_context)
            });
            for (file, (reads, obligations, dependencies)) in facts.files.iter().zip(per_file) {
                result_recomputed_files += 1;
                result.extend(reads.iter().cloned());
                dispatch_obligations.extend(obligations.iter().cloned());
                for dependency in &dependencies {
                    add_interprocedural_dependency_user(&mut cache.dependency_users, dependency);
                }
                cache.files.insert(
                    file.path.clone(),
                    CachedInterproceduralResultFile {
                        dependencies,
                        reads,
                        dispatch_obligations: obligations,
                        compiler: file.compiler.clone(),
                        call_role_syntax: call_role_syntax(file),
                    },
                );
            }
        } else {
            let current_paths = facts
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<HashSet<_>>();
            let removed_paths = cache
                .files
                .keys()
                .filter(|path| !current_paths.contains(path.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            for path in removed_paths {
                let Some(removed) = cache.files.remove(path.as_str()) else {
                    continue;
                };
                for dependency in &removed.dependencies {
                    remove_interprocedural_dependency_user(
                        &mut cache.dependency_users,
                        &mut cache.dependency_states,
                        dependency,
                    );
                }
            }
            let changed_dependencies = cache
                .dependency_states
                .iter()
                .filter(|(dependency, retained)| {
                    !result_view.dependency_matches(retained, dependency)
                })
                .map(|(dependency, _)| dependency.clone())
                .collect::<HashSet<_>>();
            for file in &facts.files {
                if all_result_sources_retained
                    && transport_identity_matches
                    && retained_source_paths.contains(file.path.as_str())
                    && let Some(cached) = cache.files.get(file.path.as_str())
                    && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                        || same_compiler_semantics(&cached.compiler, &file.compiler))
                    && cached.dependencies.is_disjoint(&changed_dependencies)
                {
                    result_reused_files += 1;
                    result.extend(cached.reads.iter().cloned());
                    dispatch_obligations.extend(cached.dispatch_obligations.iter().cloned());
                    continue;
                }
                result_recomputed_files += 1;
                let (reads, obligations, dependencies) =
                    interprocedural_result_reads_for_file(file, &result_read_context);
                result.extend(reads.iter().cloned());
                dispatch_obligations.extend(obligations.iter().cloned());
                if let Some(previous) = cache.files.remove(file.path.as_str()) {
                    for dependency in previous.dependencies.difference(&dependencies) {
                        remove_interprocedural_dependency_user(
                            &mut cache.dependency_users,
                            &mut cache.dependency_states,
                            dependency,
                        );
                    }
                    for dependency in dependencies.difference(&previous.dependencies) {
                        add_interprocedural_dependency_user(
                            &mut cache.dependency_users,
                            dependency,
                        );
                    }
                } else {
                    for dependency in &dependencies {
                        add_interprocedural_dependency_user(
                            &mut cache.dependency_users,
                            dependency,
                        );
                    }
                }
                cache.files.insert(
                    file.path.clone(),
                    CachedInterproceduralResultFile {
                        dependencies,
                        reads,
                        dispatch_obligations: obligations,
                        compiler: file.compiler.clone(),
                        call_role_syntax: call_role_syntax(file),
                    },
                );
            }
            for dependency in cache.dependency_users.keys() {
                if changed_dependencies.contains(dependency)
                    || !cache.dependency_states.contains_key(dependency)
                {
                    cache
                        .dependency_states
                        .insert(dependency.clone(), result_view.dependency_state(dependency));
                }
            }
        }
        if cache.dependency_states.is_empty() {
            for dependency in cache.dependency_users.keys() {
                cache
                    .dependency_states
                    .insert(dependency.clone(), result_view.dependency_state(dependency));
            }
        }
    } else {
        for file in &facts.files {
            result_recomputed_files += 1;
            let (reads, obligations, _) =
                interprocedural_result_reads_for_file(file, &result_read_context);
            result.extend(reads);
            dispatch_obligations.extend(obligations);
        }
    }
    // ADR 0289: computed fresh on every analysis, never retained per file:
    // the derivation reads other files' literals and the provider census.
    if !facts.dependency_source_roots.is_empty() {
        let (getter_reads, getter_obligations) = dependency_getter_obligations(
            facts,
            context.lookup,
            &provenance,
            entities,
            symbol_names,
        );
        result.extend(getter_reads);
        dispatch_obligations.extend(getter_obligations);
    }
    // Keep definition-site obligations even when one consumer is proven.
    // Another instance or opaque consumer of the same callable can remain
    // unresolved; a projected origin is not an exhaustive invocation census.
    // An exported helper that invokes a member supplied by a parameter has
    // callers outside the analyzed project. Normal project analysis must keep
    // that boundary explicit. Contract emission can discharge this specific
    // obligation because `contract_export_function` serializes the same exact
    // parameter provenance as a `parameter-member` read. A closed program has
    // no outside caller, and the pipeline discharges it there for a helper
    // entered only through call expressions
    // (`attribution::discharge_closed_program_export_dispatch`, ADR 0203).
    let mut allowed_by_path: HashMap<&str, Vec<Span>> = HashMap::new();
    for node in nodes.iter().filter(|node| node.exported) {
        let Some(&file) = project_indexes.files_by_path.get(node.path.as_str()) else {
            continue;
        };
        let Some(function) = project_indexes
            .ast_files_by_path
            .get(file.path.as_str())
            .and_then(|index| index.function_by_span(node.span))
        else {
            continue;
        };
        let allowed = allowed_by_path
            .entry(file.path.as_str())
            .or_insert_with(|| allowed_callback_spans(file, context.lookup));
        let direct_member = file.ast.calls.iter().find_map(|call| {
            if !function.body.contains(call.span)
                || !containing_ast_function(&file.ast, call.span)
                    .is_some_and(|owner| owner.span == function.span)
            {
                return None;
            }
            if semantic_execution_role(
                file,
                call.callee,
                allowed,
                entities,
                symbol_names,
                context.lookup,
            ) == ExecutionRole::TrackedJsx
            {
                return None;
            }
            if context
                .lookup
                .resolved_callee_call(file, call.callee)
                .and_then(|resolved| resolved.declaration.as_ref())
                .is_some_and(|declaration| declaration.standard_library)
            {
                return None;
            }
            let (receiver, path) = context.lookup.member_callee_receiver(file, call.callee)?;
            node.parameters
                .iter()
                .any(|parameter| parameter == &receiver)
                .then_some(path)
        });
        if let Some(path) = direct_member {
            dispatch_obligations.push(StaticDefect {
                kind: StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: node
                        .name
                        .clone()
                        .unwrap_or_else(|| "exported helper".into()),
                    // The whole chain names the dispatch: `reader.source.read`
                    // is not a dispatch on `read`. An empty path named no
                    // segment exactly, which is what `None` already spells --
                    // the obligation still stands either way.
                    member: (!path.is_empty()).then(|| path.join(".")),
                },
                location: location(Arc::from(node.path.as_str()), node.span),
                analysis_context: crate::EXPORTED_PARAMETER_MEMBER_DISPATCH.into(),
                fixes: vec![],
                uncertain: true,
            });
        }
    }
    let factory_instances = returned_bindings
        .values()
        .filter(|summary| !summary.is_empty())
        .count();
    let result_reads = phase_started.elapsed();
    let export_started = Instant::now();
    let mut structured_returns = vec![None; nodes.len()];
    let has_structured_return_seed = facts.files.iter().any(|file| {
        !file.ast.object_properties.is_empty()
            || file
                .ast
                .functions
                .iter()
                .filter_map(|function| function.expression_return.as_ref())
                .chain(file.ast.returns.iter())
                .any(|returned| {
                    !returned.elements().is_empty() || !returned.properties().is_empty()
                })
    });
    if has_structured_return_seed {
        for _ in 0..=nodes.len() {
            let discovered = discover_structured_returns(&StructuredReturnDiscovery {
                facts,
                nodes: &nodes,
                indexes: &indexes,
                by_symbol: &by_symbol,
                summaries: &summaries,
                returned: &returned,
                structured_returns: &structured_returns,
                accessors,
                source_kinds,
                entities,
                symbol_names,
                lookup,
            });
            let mut changed = false;
            for (target, returned) in structured_returns.iter_mut().zip(discovered) {
                if *target != returned {
                    *target = returned;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
    let structured_discovery = StructuredReturnDiscovery {
        facts,
        nodes: &nodes,
        indexes: &indexes,
        by_symbol: &by_symbol,
        summaries: &summaries,
        returned: &returned,
        structured_returns: &structured_returns,
        accessors,
        source_kinds,
        entities,
        symbol_names,
        lookup,
    };
    for node in nodes.iter().filter(|node| node.exported) {
        let Some(file) = lookup.file_by_path(node.path.as_str()) else {
            continue;
        };
        let Some(function) = file
            .ast
            .functions
            .iter()
            .find(|function| function.span == node.span)
        else {
            continue;
        };
        for property in function
            .expression_return
            .iter()
            .chain(file.ast.returns.iter().filter(|returned| {
                containing_ast_function(&file.ast, returned.span)
                    .is_some_and(|owner| owner.span == function.span)
            }))
            .flat_map(|returned| returned.properties())
        {
            let Some(reason) =
                structured_discovery.unresolved_shorthand_reason(file, property.value)
            else {
                continue;
            };
            dispatch_obligations.push(StaticDefect {
                kind: StaticDefectKind::StructuredReturnUnresolved {
                    function: node
                        .name
                        .clone()
                        .unwrap_or_else(|| "exported function".into()),
                    property: property.name.to_string(),
                    reason,
                },
                location: location(file.path.shared(), property.value),
                analysis_context: "exported-structured-return".into(),
                fixes: vec![],
                uncertain: true,
            });
        }
    }
    let export_summaries = export_started.elapsed();
    let results_and_exports = phase_started.elapsed();
    InterproceduralResult {
        reads: result.into(),
        source_call_targets: Arc::new(transported_calls),
        dispatch_obligations: dispatch_obligations.into(),
        factory_instances,
        timings: InterproceduralTimings {
            graph,
            direct_summaries,
            direct_index,
            direct_references,
            propagation,
            returned_direct,
            returned_delta,
            call_summary_delta,
            factory_propagation,
            results_and_exports,
            result_reads,
            export_summaries,
            graph_reused_files,
            graph_recomputed_files,
            result_reused_files,
            result_recomputed_files,
        },
    }
}
