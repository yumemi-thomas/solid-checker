//! Per-file owner discovery, target resolution, and the incremental variant of the missing-owner scan.

use super::*;

pub(crate) fn discover_owner_file(
    file: &solid_facts::FileFacts,
    indexes: &ProjectIndexes<'_>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> CachedOwnerFile {
    let dialect = lookup.dialect;
    let OwnerFileIndex {
        call_primitives,
        providing_regions,
    } = OwnerFileIndex::for_file(file, entities, symbol_names, lookup);
    let nodes = file
        .ast
        .functions
        .iter()
        .map(|function| owner_node(file, indexes, entities, function, lookup))
        .collect::<Vec<_>>();
    let nodes_by_path = function_indices_by_path(&nodes);
    let owner_at = |span| {
        containing_function_indexed(&nodes, &nodes_by_path, file.path.as_str(), span)
            .map(|index| nodes[index].span)
    };
    // Keyed by function span: the emission pass resolves it to the node whose
    // propagated context it reads, as it does `owner`.
    let region_chain = |span| {
        providing_region_chain(
            &providing_regions,
            nodes.iter().map(|node| (node.span, node.span, node.body)),
            span,
        )
    };
    let callback_target = |argument: Span| {
        nodes_by_path
            .get(file.path.as_str())
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| argument.contains(nodes[*index].span))
            .max_by_key(|index| nodes[*index].span.end - nodes[*index].span.start)
            .map(|index| OwnerTarget::LocalSpan(nodes[index].span))
            .or_else(|| {
                entities
                    .get(&location(file.path.shared(), argument))
                    .cloned()
                    .map(OwnerTarget::Symbol)
            })
    };
    let mut edges = Vec::new();
    let mut requirements = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let owner = owner_at(call.span);
        let target = lookup
            .object_member_function(file, call.callee)
            .map(|(file, function)| OwnerTarget::Function(file.path.to_string(), function.span))
            .or_else(|| {
                call_edge_symbol(file, call, entities, lookup)
                    .cloned()
                    .map(OwnerTarget::Symbol)
            });
        if let Some(target) = target {
            edges.push(SymbolicOwnerEdge {
                source: owner.map(|span| OwnerSource {
                    path: file.path.to_string(),
                    span,
                }),
                target,
                kind: OwnerEdgeKind::Preserve,
            });
        }
        for edge in owner_callback_edges(file, call, &call_primitives[call_index], lookup) {
            if let Some(target) = call
                .arguments
                .get(edge.argument)
                .and_then(|argument| callback_target(argument.span))
            {
                let source = lookup
                    .files()
                    .iter()
                    .find(|candidate| candidate.path.as_str() == edge.source_path)
                    .and_then(|source_file| {
                        containing_ast_function(&source_file.ast, edge.source).map(|function| {
                            OwnerSource {
                                path: edge.source_path.clone(),
                                span: function.span,
                            }
                        })
                    });
                edges.push(SymbolicOwnerEdge {
                    source,
                    target,
                    kind: edge.kind,
                });
            }
        }
        // Not skipped here: whether the region answers the operation needs the
        // propagated context, so the emission pass decides (`root_owned_at`).
        let providing_region_chain = region_chain(call.span);
        // Same seam as the batch owner pass, for the same reason: these two
        // matched the effect set independently and disagreed about
        // `createRenderEffect`.
        let known = known_primitive(&call_primitives[call_index]);
        let role = known.and_then(|primitive| lookup.dialect.owner_requirement_role(primitive));
        let operation = match (role, known) {
            (Some(OwnerRequirementRole::Effect), Some(primitive)) => {
                let registration =
                    crate::effect_api::classify_effect_call(file, call, primitive, lookup)
                        .owner_registration;
                (registration != ProofStatus::No).then_some((
                    "effect",
                    OWNER_CONTEXT_UNOWNED,
                    None,
                    call.callee,
                    registration == ProofStatus::Uncertain,
                ))
            }
            (Some(OwnerRequirementRole::Cleanup), _) => Some((
                "cleanup",
                OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF,
                None,
                call.callee,
                false,
            )),
            (Some(OwnerRequirementRole::SettledCleanup), _) => Some((
                "settled-cleanup",
                OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF,
                call.arguments
                    .first()
                    .and_then(|argument| callback_target(argument.span)),
                call.arguments
                    .first()
                    .map_or(call.callee, |argument| argument.span),
                false,
            )),
            _ => None,
        };
        if let Some((operation, report_mask, settled_target, operation_span, runtime_uncertain)) =
            operation
        {
            let settled_gate = known_primitive(&call_primitives[call_index])
                .filter(|primitive| dialect.leaf_owner_requires_owned_call_site(*primitive))
                .map(|_| call.span);
            requirements.push(OwnerRequirementCandidate {
                operation,
                operation_span,
                owner,
                report_mask,
                allow_uncertain: true,
                runtime_uncertain,
                settled_target,
                settled_gate,
                providing_region_chain,
            });
        }
    }
    for callback in &file.compiler.callback_roles {
        if matches!(
            callback.role,
            solid_facts::compiler::CallbackRoleKind::EventHandler
                | solid_facts::compiler::CallbackRoleKind::DirectiveApply
        ) && (callback.role != solid_facts::compiler::CallbackRoleKind::DirectiveApply
            || (ref_application_on_intrinsic(file, callback.span)
                && is_identifier_span(file, callback.span)))
            && let Some(target) = callback_target(callback.span)
        {
            // A ref application's callback is resolved against its node: a
            // generator there never runs its body (`RefApplied`).
            let kind = match callback.role {
                solid_facts::compiler::CallbackRoleKind::DirectiveApply => {
                    OwnerEdgeKind::RefApplied
                }
                _ => OwnerEdgeKind::Unowned,
            };
            edges.push(SymbolicOwnerEdge {
                source: None,
                target,
                kind,
            });
        }
    }
    // The function literals a ref value writes at its top level (ADR 0345),
    // placed by position as the batch pass places them.
    for mark in ref_value_marks(file, &call_primitives, lookup) {
        match mark {
            RefValueMark::Applied { function } => edges.push(SymbolicOwnerEdge {
                source: None,
                target: OwnerTarget::LocalSpan(function),
                kind: OwnerEdgeKind::Unowned,
            }),
            RefValueMark::Eager { function, call } => edges.push(SymbolicOwnerEdge {
                source: owner_at(call).map(|span| OwnerSource {
                    path: file.path.to_string(),
                    span,
                }),
                target: OwnerTarget::LocalSpan(function),
                kind: OwnerEdgeKind::Preserve,
            }),
            RefValueMark::Unproven { function } => edges.push(SymbolicOwnerEdge {
                source: None,
                target: OwnerTarget::LocalSpan(function),
                kind: OwnerEdgeKind::MayBeUnowned,
            }),
        }
    }
    for element in &file.ast.jsx_elements {
        let boundary = crate::jsx_primitive_name_cached(
            lookup,
            file,
            element,
            entities,
            symbol_names,
            dialect,
        );
        if boundary
            .as_deref()
            .is_some_and(|tag| dialect.is_async_boundary(tag))
        {
            requirements.push(OwnerRequirementCandidate {
                operation: "boundary",
                operation_span: Span::new(element.span.start, element.name.span.end),
                owner: owner_at(element.span),
                report_mask: OWNER_CONTEXT_UNOWNED,
                allow_uncertain: false,
                runtime_uncertain: false,
                settled_target: None,
                settled_gate: None,
                providing_region_chain: region_chain(element.span),
            });
        }
    }
    CachedOwnerFile {
        source_hash: file.source_hash.clone(),
        cross_file_proofs: lookup.cross_file_proof_digest(),
        compiler: file.compiler.clone(),
        nodes,
        edges,
        requirements,
    }
}

pub(crate) fn resolve_owner_target(
    path: &str,
    target: &OwnerTarget,
    nodes_by_span: &HashMap<String, HashMap<Span, usize>>,
    by_symbol: &HashMap<SymbolId, usize>,
) -> Option<usize> {
    match target {
        OwnerTarget::Symbol(symbol) => by_symbol.get(symbol).copied(),
        OwnerTarget::LocalSpan(span) => nodes_by_span
            .get(path)
            .and_then(|nodes| nodes.get(span))
            .copied(),
        OwnerTarget::Function(path, span) => nodes_by_span
            .get(path)
            .and_then(|nodes| nodes.get(span))
            .copied(),
    }
}

pub(crate) fn find_missing_owners_incremental(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    indexes: &ProjectIndexes<'_>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    retained_source_paths: &HashSet<String>,
    cache: &mut HashMap<SourcePath, CachedOwnerFile>,
) -> (
    Vec<OwnerRequirement>,
    SettledGateDecisions,
    OwnerIncrementalTimings,
) {
    let entities = lookup.entities();
    let mut timings = OwnerIncrementalTimings::default();
    let total_started = Instant::now();
    let current_paths = facts
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    cache.retain(|path, _| current_paths.contains(path.as_str()));
    let mut recomputed = Vec::new();
    for file in &facts.files {
        if retained_source_paths.contains(file.path.as_str())
            && let Some(cached) = cache.get(file.path.as_str())
            && cached.source_hash == file.source_hash
            && cached.cross_file_proofs == lookup.cross_file_proof_digest()
            && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                || same_compiler_semantics(&cached.compiler, &file.compiler))
        {
            timings.reused_files += 1;
            continue;
        }
        recomputed.push(file);
        timings.recomputed_files += 1;
    }
    for (path, discovered) in parallel_slice_results(&recomputed, |file| {
        (
            file.path.clone(),
            discover_owner_file(file, indexes, entities, symbol_names, lookup),
        )
    }) {
        cache.insert(path, discovered);
    }
    let fragment_build = total_started.elapsed();

    let graph_started = Instant::now();
    let mut nodes = Vec::new();
    for file in &facts.files {
        if let Some(fragment) = cache.get(file.path.as_str()) {
            nodes.extend(fragment.nodes.iter().cloned());
        }
    }
    let mut nodes_by_span = HashMap::<String, HashMap<Span, usize>>::default();
    for (index, node) in nodes.iter().enumerate() {
        nodes_by_span
            .entry(node.path.clone())
            .or_default()
            .insert(node.span, index);
    }
    let by_symbol = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.symbol.clone().map(|symbol| (symbol, index)))
        .collect::<HashMap<_, _>>();
    let mut contexts = seed_contexts(&nodes);
    let mut outgoing = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
    for file in &facts.files {
        let Some(fragment) = cache.get(file.path.as_str()) else {
            continue;
        };
        for edge in &fragment.edges {
            let Some(target) =
                resolve_owner_target(file.path.as_str(), &edge.target, &nodes_by_span, &by_symbol)
            else {
                continue;
            };
            // The same answer the fresh pass gives a ref application's
            // callback: a generator's body never runs, so nothing is proven.
            let kind = match edge.kind {
                OwnerEdgeKind::RefApplied if nodes[target].generator => OwnerEdgeKind::MayBeUnowned,
                kind => kind,
            };
            if let Some(source) = edge.source.as_ref().and_then(|source| {
                nodes_by_span
                    .get(&source.path)
                    .and_then(|nodes| nodes.get(&source.span))
                    .copied()
            }) {
                outgoing[source].push((target, kind));
            } else {
                contexts[target] |=
                    owner_edge_context(kind, OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED);
            }
        }
    }
    let graph_assembly = graph_started.elapsed();

    let propagation_started = Instant::now();
    propagate_owner_contexts(&mut contexts, &outgoing);
    let propagation = propagation_started.elapsed();

    let requirements_started = Instant::now();
    let mut requirements = Vec::new();
    let continuations = AwaitContinuations::new(facts, lookup);
    let mut settled_gates = SettledGateDecisions::default();
    let mut seen = HashSet::default();
    for file in &facts.files {
        let Some(fragment) = cache.get(file.path.as_str()) else {
            continue;
        };
        for candidate in &fragment.requirements {
            let owner_index = candidate.owner.and_then(|span| {
                nodes_by_span
                    .get(file.path.as_str())
                    .and_then(|nodes| nodes.get(&span))
                    .copied()
            });
            let context = owner_index.map_or(OWNER_CONTEXT_UNOWNED, |index| contexts[index]);
            // As in the batch pass: effects and cleanups, never a JSX boundary
            // or `onSettled`'s returned cleanup.
            let after_await = matches!(candidate.operation, "effect" | "cleanup")
                && candidate.operation != "settled-cleanup"
                && continuations.contains(file.path.as_str(), candidate.operation_span.start);
            let context = if after_await {
                await_continuation_context(context)
            } else {
                context
            };
            // The batch pass skips these before recording a settled gate too.
            // Every chain span is one of this fragment's own nodes; one that
            // did not resolve would withhold the lexical answer, not grant it.
            if root_owned_at(candidate.providing_region_chain.as_deref(), |span| {
                nodes_by_span
                    .get(file.path.as_str())
                    .and_then(|nodes| nodes.get(span))
                    .map_or(OWNER_CONTEXT_LEAVES_REGION, |index| contexts[*index])
            }) {
                continue;
            }
            if let Some(gate) = candidate.settled_gate {
                settled_gates.insert(
                    (
                        file.path.to_string(),
                        u64::from(gate.start),
                        u64::from(gate.end),
                    ),
                    leaf_gate_decision(owner_index, &nodes, &contexts),
                );
            }
            if candidate.operation == "settled-cleanup" {
                let cleanup_proof = candidate
                    .settled_target
                    .as_ref()
                    .and_then(|target| {
                        resolve_owner_target(file.path.as_str(), target, &nodes_by_span, &by_symbol)
                    })
                    .and_then(|index| {
                        let node = &nodes[index];
                        let callback_file = indexes.files_by_path.get(node.path.as_str())?;
                        let callback = callback_file.ast.function_at(node.span)?;
                        Some(function_cleanup_return_proof(
                            lookup,
                            callback_file,
                            callback,
                        ))
                    })
                    .unwrap_or(CleanupReturnProof::Unresolved);
                let settled_call_valid = file.ast.calls.iter().any(|call| {
                    call.arguments
                        .first()
                        .is_some_and(|argument| argument.span == candidate.operation_span)
                        && lookup
                            .resolved_callee_call(file, call.callee)
                            .is_some_and(|resolved| {
                                resolved.validity == typefacts::ResolvedCallValidity::Valid
                            })
                });
                match cleanup_proof {
                    CleanupReturnProof::NoFunction => continue,
                    CleanupReturnProof::Function => {}
                    CleanupReturnProof::OptionalFunction => {
                        // The callback may return a cleanup. Preserve that as
                        // runtime uncertainty instead of certifying it absent.
                    }
                    CleanupReturnProof::Unresolved if settled_call_valid => {}
                    CleanupReturnProof::Unresolved => continue,
                }
            }
            let proven_unowned = context & OWNER_CONTEXT_PROVEN_UNOWNED != 0;
            let component_uncertain = !proven_unowned
                && (context & OWNER_CONTEXT_COMPONENT_UNCERTAIN != 0
                    || owner_index.is_some_and(|index| nodes[index].component_uncertain));
            let later_run_unowned = !component_uncertain
                && !proven_unowned
                && context & OWNER_CONTEXT_LATER_RUN_UNOWNED != 0;
            let conditional_owner = !component_uncertain
                && !proven_unowned
                && !later_run_unowned
                && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                    == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
            let caller_uncertain = candidate.allow_uncertain
                && !proven_unowned
                && owner_index.is_some_and(|index| {
                    (nodes[index].exported
                        && contexts[index] & OWNER_CONTEXT_UNOWNED != 0
                        && !nodes[index].component
                        && !nodes[index].component_uncertain)
                        || open_world_unowned_only(context, &nodes[index])
                });
            let cleanup_return_uncertain = candidate.operation == "settled-cleanup"
                && candidate
                    .settled_target
                    .as_ref()
                    .and_then(|target| {
                        resolve_owner_target(file.path.as_str(), target, &nodes_by_span, &by_symbol)
                    })
                    .and_then(|index| {
                        let node = &nodes[index];
                        let callback_file = indexes.files_by_path.get(node.path.as_str())?;
                        let callback = callback_file.ast.function_at(node.span)?;
                        Some(function_cleanup_return_proof(
                            lookup,
                            callback_file,
                            callback,
                        ))
                    })
                    .is_none_or(|proof| {
                        matches!(
                            proof,
                            CleanupReturnProof::OptionalFunction | CleanupReturnProof::Unresolved
                        )
                    });
            let runtime_uncertain = candidate.runtime_uncertain || cleanup_return_uncertain;
            let uncertain = runtime_uncertain
                || conditional_owner
                || later_run_unowned
                || caller_uncertain
                || component_uncertain;
            push_owner_requirement(
                &mut requirements,
                &mut seen,
                candidate.operation,
                file,
                candidate.operation_span,
                OwnerRequirementStatus {
                    uncertain,
                    runtime_uncertain,
                    caller_uncertain,
                    conditional_owner,
                    later_run_unowned,
                    component_uncertain,
                    after_await,
                    report: context & candidate.report_mask != 0,
                },
                lookup,
            );
        }
    }
    attribute_dependency_requirements(
        &mut requirements,
        facts,
        lookup,
        &nodes,
        &contexts,
        &outgoing,
    );
    let requirement_emission = requirements_started.elapsed();
    (
        requirements,
        settled_gates,
        OwnerIncrementalTimings {
            fragment_build,
            graph_assembly,
            propagation,
            requirement_emission,
            ..timings
        },
    )
}
