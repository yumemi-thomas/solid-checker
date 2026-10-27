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
    // ADR 0454: the function that owns the site. A call in a parameter list is
    // the activation of the function whose list it is, not of the function
    // around it (`containing_function_indexed` is body-only). Every function
    // has a node, so the owner's span names one.
    let owner_at = |span| {
        file.ast
            .innermost_function_owning(span)
            .map(|function| function.span)
    };
    // Whether the site runs only when its function is called without the
    // argument. Nothing here proves a call omits it, so the site has no proven
    // owner context: an edge it makes is a root that may be unowned
    // ([`OwnerEdgeKind::MayBeUnowned`]), and a requirement it holds is
    // uncertifiable, never a violation and never silent.
    let list_site = |span| crate::owners::in_parameter_list(&file.ast, span);
    // Keyed by function span: the emission pass resolves it to the node whose
    // propagated context it reads, as it does `owner`.
    let region_chain = |span| {
        providing_region_chain(
            &providing_regions,
            nodes.iter().map(|node| (node.span, node.span, node.body)),
            span,
        )
    };
    // The callback an argument names, exactly as `owner_callback_index` reads
    // it: the function literal itself (or an event tuple's handler slot), or
    // the function a bare identifier names. A function literal inside any other
    // expression is not the callback (SC4001 review D).
    let callback_target = |argument: Span, handler_tuple: bool| {
        callback_literal_span(file, argument, handler_tuple)
            .or_else(|| crate::indexes::identity_wrapped_literal_span(lookup, file, argument))
            .and_then(|span| {
                nodes_by_path
                    .get(file.path.as_str())
                    .into_iter()
                    .flatten()
                    .copied()
                    .find(|index| nodes[*index].span == span)
            })
            .map(|index| OwnerTarget::LocalSpan(nodes[index].span))
            .or_else(|| {
                if !is_identifier_span(file, argument) {
                    return None;
                }
                entities
                    .value_symbol(file, argument)
                    .cloned()
                    .map(OwnerTarget::Symbol)
            })
    };
    let mut edges = Vec::new();
    let mut requirements = Vec::new();
    // ADR 0358: an event attribute's member read, and the handlers an intrinsic
    // element's spread supplies, name their functions exactly; the host runs
    // them with no owner.
    for callback in &file.compiler.callback_roles {
        if callback.role != solid_facts::compiler::CallbackRoleKind::EventHandler {
            continue;
        }
        if let Some((target, function)) = lookup.object_member_function(file, callback.span) {
            edges.push(SymbolicOwnerEdge {
                source: None,
                target: OwnerTarget::Function(target.path.to_string(), function.span),
                kind: event_owner_edge(lookup, function.generator),
            });
        }
    }
    for element in &file.ast.jsx_elements {
        if !crate::object_members::intrinsic_element(file, element) {
            continue;
        }
        for spread in &element.spreads {
            let key = crate::location(file.path.shared(), spread.argument);
            let Some(Some(targets)) = lookup.spread_handler_targets().get(&key) else {
                continue;
            };
            for (path, span) in targets {
                edges.push(SymbolicOwnerEdge {
                    source: None,
                    target: OwnerTarget::Function(path.clone(), *span),
                    kind: event_owner_edge(
                        lookup,
                        lookup
                            .file_by_path(path)
                            .and_then(|file| file.ast.function_at(*span))
                            .is_some_and(|function| function.generator),
                    ),
                });
            }
        }
    }
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
        let in_list = list_site(call.span);
        if let Some(target) = target {
            edges.push(SymbolicOwnerEdge {
                source: owner.filter(|_| !in_list).map(|span| OwnerSource {
                    path: file.path.to_string(),
                    span,
                }),
                target,
                kind: if in_list {
                    OwnerEdgeKind::MayBeUnowned
                } else {
                    call_owner_edge(file, call.callee)
                },
            });
        }
        // ADR 0347: an invoked function literal runs in the call, under the
        // owner of the code around it (`invocation_runs_under_owner`).
        if let Some(function) = invoked_function_literal(file, call) {
            let invoker =
                enclosing_function_span(&nodes, &nodes_by_path, file.path.as_str(), call.span);
            if invocation_runs_under_owner(file, call.span, invoker.map(|index| nodes[index].span))
            {
                edges.push(SymbolicOwnerEdge {
                    source: invoker.filter(|_| !in_list).map(|index| OwnerSource {
                        path: file.path.to_string(),
                        span: nodes[index].span,
                    }),
                    target: OwnerTarget::LocalSpan(function.span),
                    kind: if in_list {
                        OwnerEdgeKind::MayBeUnowned
                    } else {
                        call_owner_edge(file, call.callee)
                    },
                });
            }
        }
        for edge in owner_callback_edges(file, call, &call_primitives[call_index], lookup) {
            let kind = callback_call_owner_edge(lookup, &edge.source_path, edge.source, edge.kind);
            if let Some(target) = call
                .arguments
                .get(edge.argument)
                .and_then(|argument| callback_target(argument.span, false))
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
                // A callback created in a parameter list exists only if the
                // default runs: it is owned where its primitive owns it, and
                // otherwise may be unowned.
                let (source, kind) = if list_site(call.span) {
                    (
                        None,
                        if kind == OwnerEdgeKind::Owned {
                            kind
                        } else {
                            OwnerEdgeKind::MayBeUnowned
                        },
                    )
                } else {
                    (source, kind)
                };
                edges.push(SymbolicOwnerEdge {
                    source,
                    target,
                    kind,
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
            // A leaf owner exists here, and the runtime throws
            // CLEANUP_IN_FORBIDDEN_SCOPE (SC3001's claim), not an unowned
            // registration: the mask is unowned only (ADR 0347 amendment).
            (Some(OwnerRequirementRole::Cleanup), _) => {
                Some(("cleanup", OWNER_CONTEXT_UNOWNED, None, call.callee, false))
            }
            (Some(OwnerRequirementRole::SettledCleanup), _) => Some((
                "settled-cleanup",
                OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF,
                call.arguments
                    .first()
                    .and_then(|argument| callback_target(argument.span, false)),
                call.arguments
                    .first()
                    .map_or(call.callee, |argument| argument.span),
                false,
            )),
            // ADR 0352: `useContext` with no owner throws `NoOwnerError`; the
            // batch pass asks the same unowned question.
            (Some(OwnerRequirementRole::ContextRead), _) => Some((
                "context-read",
                OWNER_CONTEXT_UNOWNED,
                None,
                call.callee,
                false,
            )),
            // ADR 0462: `Loading(props)`/`Errored(props)` called as a function
            // creates its boundary in the call, with the caller's owner. Its
            // own operation name, so the call is placed after an `await` as
            // an effect is, and a JSX boundary keeps its placement.
            (Some(OwnerRequirementRole::Boundary), _) => Some((
                "boundary-call",
                OWNER_CONTEXT_UNOWNED,
                None,
                call.callee,
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
            // The context argument of a `useContext` read, for the closed
            // provider census (ADR 0352). Only a read with exactly one argument.
            let context_argument = (operation == "context-read")
                .then(|| call.arguments.first())
                .flatten()
                .filter(|_| call.arguments.len() == 1 && !call.arguments[0].spread)
                .map(|argument| argument.span);
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
                context_argument,
                in_parameter_list: in_list,
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
            && let Some(target) = callback_target(
                callback.span,
                callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler,
            )
        {
            // A ref application's callback is resolved against its node: a
            // generator there never runs its body (`RefApplied`).
            let kind = match callback.role {
                solid_facts::compiler::CallbackRoleKind::DirectiveApply => {
                    OwnerEdgeKind::RefApplied
                }
                _ if lookup.synchronous_event_dispatch_present() => OwnerEdgeKind::Conditional,
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
            RefValueMark::Applied { function } | RefValueMark::Unowned { function } => {
                edges.push(SymbolicOwnerEdge {
                    source: None,
                    target: OwnerTarget::LocalSpan(function),
                    kind: OwnerEdgeKind::Unowned,
                })
            }
            RefValueMark::Eager { function, call } => edges.push(SymbolicOwnerEdge {
                source: nodes
                    .iter()
                    .filter(|node| node.span.contains(call))
                    .min_by_key(|node| node.span.end - node.span.start)
                    .map(|node| OwnerSource {
                        path: file.path.to_string(),
                        span: node.span,
                    }),
                target: OwnerTarget::LocalSpan(function),
                kind: OwnerEdgeKind::EagerCall,
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
                context_argument: None,
                in_parameter_list: list_site(element.span),
            });
        }
    }
    CachedOwnerFile {
        source_hash: file.source_hash.clone(),
        proof: crate::base::reads::ReadProof::default(),
        compiler: file.compiler.clone(),
        nodes,
        edges,
        requirements,
    }
}

/// What the owner stage keeps between builds: the graph with its fragments
/// and contexts, and each file's emission ([`owner_state`], [`owner_emission`]).
#[derive(Default)]
pub(crate) struct OwnerState {
    graph: OwnerGraph,
    emissions: HashMap<PathId, Emission>,
}

/// The owner requirements of the project, and the call-site gates they
/// settled. `state` holds the previous build's graph (a one-shot build's is
/// empty).
pub(crate) fn find_missing_owners_incremental(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    indexes: &ProjectIndexes<'_>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    unchanged_paths: &HashSet<String>,
    state: &mut OwnerState,
) -> (
    Vec<OwnerRequirement>,
    SettledGateDecisions,
    OwnerIncrementalTimings,
) {
    let entities = lookup.entities();
    let mut timings = OwnerIncrementalTimings::default();
    let total_started = Instant::now();
    // The files of the build, each path once: a path listed twice is the
    // later slot's.
    let mut last_slot = HashMap::<&str, usize>::default();
    for (slot, file) in facts.files.iter().enumerate() {
        last_slot.insert(file.path.as_str(), slot);
    }
    let files = facts
        .files
        .iter()
        .enumerate()
        .filter(|(slot, file)| last_slot.get(file.path.as_str()) == Some(slot))
        .map(|(_, file)| file)
        .collect::<Vec<_>>();
    let OwnerState { graph, emissions } = state;
    let order = files
        .iter()
        .map(|file| graph.intern(file.path.as_str()))
        .collect::<Vec<_>>();
    let current = order.iter().copied().collect::<HashSet<_>>();
    for held in graph.held() {
        if !current.contains(&held) {
            graph.remove_fragment(held);
            emissions.remove(&held);
        }
    }
    let mut recomputed = Vec::new();
    for (file, pid) in files.iter().zip(&order) {
        if unchanged_paths.contains(file.path.as_str())
            && let Some(entry) = graph.file(*pid)
            && entry.fragment.source_hash == file.source_hash
            && entry.fragment.proof.holds(lookup)
            && (Arc::ptr_eq(&entry.fragment.compiler, &file.compiler)
                || same_compiler_semantics(&entry.fragment.compiler, &file.compiler))
        {
            timings.reused_files += 1;
            continue;
        }
        recomputed.push(*file);
        timings.recomputed_files += 1;
    }
    let discovered = parallel_slice_results(&recomputed, |file| {
        // What the fragment read of other files is its proof.
        let (mut discovered, reads) = crate::base::reads::record(|| {
            discover_owner_file(file, indexes, entities, symbol_names, lookup)
        });
        discovered.proof = crate::base::reads::ReadProof::new(reads, lookup);
        (file.path.clone(), discovered)
    });
    let fragment_build = total_started.elapsed();

    let graph_started = Instant::now();
    for (path, fragment) in discovered {
        let pid = graph.intern(path.as_str());
        graph.set_fragment(path.as_str(), fragment);
        // The fragment's lookup reads were recorded again: so are the
        // emission's, which fold into its proof.
        emissions.remove(&pid);
    }
    let graph_assembly = graph_started.elapsed();

    let propagation_started = Instant::now();
    let moved = graph.settle(
        &order,
        &HandOuts {
            named: &|path, span| lookup.hands_out_by_name(path, span),
            escapes_unnamed: &|path| lookup.escapes_unnamed(path),
        },
    );
    let propagation = propagation_started.elapsed();

    let requirements_started = Instant::now();
    // Program-wide: a context's provider census spans every file (ADR 0352).
    let census_started = Instant::now();
    let provider_free = provider_free_contexts(facts, lookup, symbol_names);
    let context_census = census_started.elapsed();
    let inputs = EmissionInputs {
        facts,
        lookup,
        indexes,
        provider_free: &provider_free,
    };
    let mut dirty = moved.files.clone();
    dirty.extend(moved.contexts.iter().map(|node| node.path));
    let mut requirements = Vec::new();
    let mut settled_gates = SettledGateDecisions::default();
    for (file, pid) in files.iter().zip(&order) {
        let reusable = !dirty.contains(pid)
            && emissions
                .get(pid)
                .is_some_and(|emission| inputs.holds(graph, *pid, emission));
        if !reusable {
            let (emission, reads) =
                crate::base::reads::record(|| emit_file(&inputs, graph, *pid, file));
            // The emission's reads join the fragment's: one proof per file.
            if let Some(entry) = graph.file_mut(*pid) {
                let mut total = entry.fragment.proof.reads.clone();
                total.extend(&reads);
                entry.fragment.proof = crate::base::reads::ReadProof::new(total, lookup);
            }
            emissions.insert(*pid, emission);
        }
        if let Some(emission) = emissions.get(pid) {
            requirements.extend(emission.requirements.iter().cloned());
            settled_gates.extend(emission.gates.iter().cloned());
        }
    }
    if !facts.dependency_source_roots.is_empty() {
        let (nodes, contexts, outgoing) = graph.dense_view(&order);
        attribute_dependency_requirements(
            &mut requirements,
            facts,
            lookup,
            &nodes,
            &contexts,
            &outgoing,
        );
    }
    let requirement_emission = requirements_started.elapsed();
    (
        requirements,
        settled_gates,
        OwnerIncrementalTimings {
            fragment_build,
            graph_assembly,
            propagation,
            requirement_emission,
            context_census,
            ..timings
        },
    )
}
