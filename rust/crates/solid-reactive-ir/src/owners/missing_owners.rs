//! Missing-owner discovery: the project-wide scan for reactive reads and creations that run with no owner.

use super::*;

pub(crate) fn find_missing_owners(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    indexes: &ProjectIndexes<'_>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> (Vec<OwnerRequirement>, SettledGateDecisions) {
    let entities = lookup.entities();
    let owner_file_indexes = facts
        .files
        .iter()
        .map(|file| OwnerFileIndex::for_file(file, entities, symbol_names, lookup))
        .collect::<Vec<_>>();
    let mut nodes = Vec::new();
    for file in &facts.files {
        for function in &file.ast.functions {
            nodes.push(owner_node(file, indexes, entities, function, lookup));
        }
    }
    let nodes_by_path = function_indices_by_path(&nodes);
    let by_symbol = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.symbol.clone().map(|symbol| (symbol, index)))
        .collect::<HashMap<_, _>>();
    let mut contexts = seed_contexts(&nodes);
    let mut edges = Vec::<(usize, usize, OwnerEdgeKind)>::new();
    let mut requirements = Vec::new();
    let mut seen = HashSet::default();
    for (file_index, file) in facts.files.iter().enumerate() {
        for (call_index, call) in file.ast.calls.iter().enumerate() {
            let owner =
                containing_function_indexed(&nodes, &nodes_by_path, file.path.as_str(), call.span);
            if let Some(target_index) = lookup
                .object_member_function(file, call.callee)
                .and_then(|(file, function)| {
                    nodes.iter().position(|node| {
                        node.path == file.path.as_str() && node.span == function.span
                    })
                })
                .or_else(|| {
                    call_edge_symbol(file, call, entities, lookup)
                        .and_then(|symbol| by_symbol.get(symbol))
                        .copied()
                })
            {
                if let Some(owner) = owner {
                    edges.push((owner, target_index, OwnerEdgeKind::Preserve));
                } else {
                    contexts[target_index] |= OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED;
                }
            }
            for edge in owner_callback_edges(
                file,
                call,
                &owner_file_indexes[file_index].call_primitives[call_index],
                lookup,
            ) {
                let Some(argument) = call.arguments.get(edge.argument) else {
                    continue;
                };
                let Some(target_index) = owner_callback_index(
                    &nodes,
                    &nodes_by_path,
                    &by_symbol,
                    file,
                    argument.span,
                    entities,
                    false,
                    lookup,
                ) else {
                    continue;
                };
                let invocation_owner = containing_function_indexed(
                    &nodes,
                    &nodes_by_path,
                    &edge.source_path,
                    edge.source,
                );
                if let Some(owner) = invocation_owner {
                    edges.push((owner, target_index, edge.kind));
                } else {
                    contexts[target_index] |= owner_edge_context(
                        edge.kind,
                        OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
                    );
                }
            }
        }
        for callback in &file.compiler.callback_roles {
            if !matches!(
                callback.role,
                solid_facts::compiler::CallbackRoleKind::EventHandler
                    | solid_facts::compiler::CallbackRoleKind::DirectiveApply
            ) || (callback.role == solid_facts::compiler::CallbackRoleKind::DirectiveApply
                && (!ref_application_on_intrinsic(file, callback.span)
                    || !is_identifier_span(file, callback.span)))
            {
                continue;
            }
            if let Some(index) = owner_callback_index(
                &nodes,
                &nodes_by_path,
                &by_symbol,
                file,
                callback.span,
                entities,
                callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler,
                lookup,
            ) {
                let kind = match callback.role {
                    solid_facts::compiler::CallbackRoleKind::DirectiveApply
                        if nodes[index].generator =>
                    {
                        OwnerEdgeKind::MayBeUnowned
                    }
                    solid_facts::compiler::CallbackRoleKind::DirectiveApply => {
                        OwnerEdgeKind::RefApplied
                    }
                    _ => OwnerEdgeKind::Unowned,
                };
                contexts[index] |=
                    owner_edge_context(kind, OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED);
            }
        }
        // The function literals a ref value writes at its top level (ADR 0345),
        // placed by position: a value-position function runs with no owner at
        // the application, a call made at module initialization runs with none,
        // an eagerly called one runs under the owner of the code around the
        // expression (settled once propagation has run), and any other one is
        // unproven.
        for mark in ref_value_marks(
            file,
            &owner_file_indexes[file_index].call_primitives,
            lookup,
        ) {
            let function = match mark {
                RefValueMark::Applied { function }
                | RefValueMark::Unowned { function }
                | RefValueMark::Eager { function, .. }
                | RefValueMark::Unproven { function } => function,
            };
            let Some(target) = nodes_by_path
                .get(file.path.as_str())
                .into_iter()
                .flatten()
                .copied()
                .find(|index| nodes[*index].span == function)
            else {
                continue;
            };
            match mark {
                RefValueMark::Applied { .. } | RefValueMark::Unowned { .. } => {
                    contexts[target] |= OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED;
                }
                RefValueMark::Eager { call, .. } => {
                    if let Some(owner) =
                        enclosing_function_span(&nodes, &nodes_by_path, file.path.as_str(), call)
                    {
                        edges.push((owner, target, OwnerEdgeKind::EagerCall));
                    } else {
                        contexts[target] |= OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED;
                    }
                }
                RefValueMark::Unproven { .. } => {
                    contexts[target] |= owner_edge_context(OwnerEdgeKind::MayBeUnowned, 0);
                }
            }
        }
    }
    let mut outgoing = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
    for (source, target, kind) in edges {
        outgoing[source].push((target, kind));
    }
    propagate_owner_contexts(&mut contexts, &outgoing);
    settle_eager_owners(&mut contexts, &outgoing);

    let continuations = AwaitContinuations::new(facts, lookup);
    let mut settled_gates = SettledGateDecisions::default();
    for (file_index, file) in facts.files.iter().enumerate() {
        let file_nodes = nodes_by_path
            .get(file.path.as_str())
            .map_or(&[][..], Vec::as_slice);
        let root_owned_span = |span: Span| {
            root_owned_at(
                providing_region_chain(
                    &owner_file_indexes[file_index].providing_regions,
                    file_nodes
                        .iter()
                        .map(|index| (*index, nodes[*index].span, nodes[*index].body)),
                    span,
                )
                .as_deref(),
                |index| contexts[*index],
            )
        };
        for (call_index, call) in file.ast.calls.iter().enumerate() {
            let primitive =
                known_primitive(&owner_file_indexes[file_index].call_primitives[call_index]);
            let context = owner_context_at(
                &nodes,
                &nodes_by_path,
                &contexts,
                file.path.as_str(),
                call.span,
            );
            // `onSettled`'s returned cleanup is a different owner question
            // (whether an unowned settle registers at all), so it keeps the
            // enclosing context.
            let after_await = continuations.contains(file.path.as_str(), call.callee.start)
                && !matches!(
                    primitive
                        .and_then(|primitive| lookup.dialect.owner_requirement_role(primitive)),
                    Some(OwnerRequirementRole::SettledCleanup)
                );
            let context = if after_await {
                await_continuation_context(context)
            } else {
                context
            };
            let root_owned = root_owned_span(call.span);
            if !root_owned
                && primitive.is_some_and(|primitive| {
                    lookup
                        .dialect
                        .leaf_owner_requires_owned_call_site(primitive)
                })
            {
                settled_gates.insert(
                    (
                        file.path.to_string(),
                        u64::from(call.span.start),
                        u64::from(call.span.end),
                    ),
                    leaf_gate_decision(
                        containing_function_indexed(
                            &nodes,
                            &nodes_by_path,
                            file.path.as_str(),
                            call.span,
                        ),
                        &nodes,
                        &contexts,
                    ),
                );
            }
            // Which primitives carry which owner-requirement role is the
            // dialect's answer (`owner_requirement_role`), not a set spelled
            // out here. The two owner passes used to spell it out separately
            // and drifted: this one omitted `createRenderEffect`, so a render
            // effect outside any owner leaked with nothing reported.
            let role =
                primitive.and_then(|primitive| lookup.dialect.owner_requirement_role(primitive));
            let operation = match (role, primitive) {
                (Some(OwnerRequirementRole::Effect), Some(primitive)) if !root_owned => {
                    let registration =
                        crate::effect_api::classify_effect_call(file, call, primitive, lookup)
                            .owner_registration;
                    (registration != ProofStatus::No).then_some((
                        "effect",
                        context & OWNER_CONTEXT_UNOWNED != 0,
                        registration == ProofStatus::Uncertain,
                    ))
                }
                (Some(OwnerRequirementRole::Cleanup), _) if !root_owned => Some((
                    "cleanup",
                    context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                    false,
                )),
                (Some(OwnerRequirementRole::SettledCleanup), _) if !root_owned => {
                    let proof = call
                        .arguments
                        .first()
                        .and_then(|argument| {
                            owner_callback_index(
                                &nodes,
                                &nodes_by_path,
                                &by_symbol,
                                file,
                                argument.span,
                                entities,
                                false,
                                lookup,
                            )
                        })
                        .and_then(|index| {
                            let node = &nodes[index];
                            let callback_file = facts
                                .files
                                .iter()
                                .find(|candidate| candidate.path.as_str() == node.path)?;
                            let callback = callback_file.ast.function_at(node.span)?;
                            Some(function_cleanup_return_proof(
                                lookup,
                                callback_file,
                                callback,
                            ))
                        })
                        .unwrap_or(CleanupReturnProof::Unresolved);
                    match proof {
                        CleanupReturnProof::Function => Some((
                            "settled-cleanup",
                            context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                            false,
                        )),
                        CleanupReturnProof::OptionalFunction => Some((
                            "settled-cleanup",
                            context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                            true,
                        )),
                        CleanupReturnProof::Unresolved
                            if lookup.resolved_callee_call(file, call.callee).is_some_and(
                                |resolved| {
                                    resolved.validity == typefacts::ResolvedCallValidity::Valid
                                },
                            ) =>
                        {
                            Some((
                                "settled-cleanup",
                                context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                                true,
                            ))
                        }
                        CleanupReturnProof::Unresolved => None,
                        CleanupReturnProof::NoFunction => None,
                    }
                }
                _ => None,
            };
            if let Some((operation, report, runtime_uncertain)) = operation {
                let owner_index = containing_function_indexed(
                    &nodes,
                    &nodes_by_path,
                    file.path.as_str(),
                    call.span,
                );
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
                let caller_uncertain = !proven_unowned
                    && owner_index.is_some_and(|index| {
                        (nodes[index].exported
                            && contexts[index] & OWNER_CONTEXT_UNOWNED != 0
                            && !nodes[index].component
                            && !nodes[index].component_uncertain)
                            || open_world_unowned_only(context, &nodes[index])
                    });
                let uncertain = runtime_uncertain
                    || conditional_owner
                    || later_run_unowned
                    || caller_uncertain
                    || component_uncertain;
                let operation_span = if operation == "settled-cleanup" {
                    call.arguments
                        .first()
                        .map_or(call.callee, |argument| argument.span)
                } else {
                    call.callee
                };
                push_owner_requirement(
                    &mut requirements,
                    &mut seen,
                    operation,
                    file,
                    operation_span,
                    OwnerRequirementStatus {
                        uncertain,
                        runtime_uncertain,
                        caller_uncertain,
                        conditional_owner,
                        later_run_unowned,
                        component_uncertain,
                        after_await,
                        report,
                    },
                    lookup,
                );
            }
        }
        for element in &file.ast.jsx_elements {
            let boundary = crate::jsx_primitive_name_cached(
                lookup,
                file,
                element,
                entities,
                symbol_names,
                lookup.dialect,
            );
            if !boundary
                .as_deref()
                .is_some_and(|tag| lookup.dialect.is_async_boundary(tag))
            {
                continue;
            }
            let context = owner_context_at(
                &nodes,
                &nodes_by_path,
                &contexts,
                file.path.as_str(),
                element.span,
            );
            if root_owned_span(element.span) {
                continue;
            }
            let proven_unowned = context & OWNER_CONTEXT_PROVEN_UNOWNED != 0;
            let component_uncertain = !proven_unowned
                && (context & OWNER_CONTEXT_COMPONENT_UNCERTAIN != 0
                    || containing_function_indexed(
                        &nodes,
                        &nodes_by_path,
                        file.path.as_str(),
                        element.span,
                    )
                    .is_some_and(|index| nodes[index].component_uncertain));
            let later_run_unowned = !component_uncertain
                && !proven_unowned
                && context & OWNER_CONTEXT_LATER_RUN_UNOWNED != 0;
            let conditional_owner = !component_uncertain
                && !proven_unowned
                && !later_run_unowned
                && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                    == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
            push_owner_requirement(
                &mut requirements,
                &mut seen,
                "boundary",
                file,
                Span::new(element.span.start, element.name.span.end),
                OwnerRequirementStatus {
                    uncertain: conditional_owner || later_run_unowned || component_uncertain,
                    runtime_uncertain: false,
                    caller_uncertain: false,
                    conditional_owner,
                    later_run_unowned,
                    component_uncertain,
                    after_await: false,
                    report: context & OWNER_CONTEXT_UNOWNED != 0,
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
    (requirements, settled_gates)
}

/// The function symbol a call runs, for an owner edge. An entity at a member
/// callee's composite span can name its receiver (`f.reset()`, `f.bind(…)`
/// carry `f`'s symbol), and such a call does not run the receiver
/// (ADR 0290 review round 1): it gets no edge, unless the member is the
/// standard library's `call` or `apply`, which run it before they return.
pub(super) fn call_edge_symbol<'e>(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &'e EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> Option<&'e SymbolId> {
    let symbol = entities.get(&location(file.path.shared(), call.callee))?;
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let receiver = file
        .ast
        .member_at(callee)
        .map(|member| file.ast.peel_ts_sugar_span(member.object));
    if receiver
        .is_some_and(|object| entities.get(&location(file.path.shared(), object)) == Some(symbol))
    {
        let invokes_receiver = lookup
            .resolved_callee_call(file, call.callee)
            .and_then(|resolved| resolved.declaration.as_ref())
            .is_some_and(|declaration| {
                declaration.standard_library
                    && matches!(
                        declaration.qualified_name.as_ref(),
                        "Function.call"
                            | "Function.apply"
                            | "CallableFunction.call"
                            | "CallableFunction.apply"
                            | "NewableFunction.call"
                            | "NewableFunction.apply"
                    )
            });
        return invokes_receiver.then_some(symbol);
    }
    Some(symbol)
}

/// ADR 0290: report an owner requirement located in a package analyzed as
/// source (ADR 0284) at the project's call that runs it there.
///
/// A requirement inside a package is reported because some owner context
/// reached its function; when that context came from a project caller, the
/// defect is the project's call, and dropping it with the package's own
/// findings would leave the project silent. The walk goes back from the
/// operation's function along the edges that carry the reporting bit, through
/// package functions only, and stops at:
///
/// - an edge from a project function: each call written directly in it whose
///   callee is the package function it enters becomes a requirement at that
///   call;
/// - an edge a package function creates the bit on itself (it schedules the
///   callback): that context is the package's own, not a caller's.
///
/// Only a requirement proven unowned (`OWNER_CONTEXT_PROVEN_UNOWNED`, not
/// uncertain) walks the proven bit and can stay a violation at the call, and
/// only when the call is on its project function's every-completion path:
/// that function's unowned context reaches a call behind a branch or an owner
/// guard on some invocations only. Every other one walks the plain unowned
/// bit and becomes an obligation there: the call may run it unowned.
///
/// An operation after an `await` inside the package is unowned whatever the
/// caller did, so it walks every edge. A project call at module scope has no
/// edge; it is found by scanning and attributed as an obligation. A bit
/// seeded inside the package (its own event handlers or module scope) has no
/// project call and is not attributed.
pub(crate) fn attribute_dependency_requirements(
    requirements: &mut Vec<OwnerRequirement>,
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    nodes: &[OwnerNode],
    contexts: &[u8],
    outgoing: &[Vec<(usize, OwnerEdgeKind)>],
) {
    if facts.dependency_source_roots.is_empty() {
        return;
    }
    let mut incoming = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
    for (source, edges) in outgoing.iter().enumerate() {
        for &(target, kind) in edges {
            incoming[target].push((source, kind));
        }
    }
    let in_dependency = |index: usize| facts.dependency_source_root(&nodes[index].path).is_some();
    let mut seen = requirements
        .iter()
        .map(|requirement| {
            (
                requirement.location.path.to_string(),
                requirement.location.start_byte,
                requirement.location.end_byte,
                operation_name(requirement.operation).to_owned(),
            )
        })
        .collect::<HashSet<_>>();
    let mut attributed = Vec::new();
    for requirement in requirements.iter() {
        if !requirement.report
            || requirement.through_dependency.is_some()
            || facts
                .dependency_source_root(&requirement.location.path)
                .is_none()
        {
            continue;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(requirement.location.start_byte),
            u32::try_from(requirement.location.end_byte),
        ) else {
            continue;
        };
        let span = Span::new(start, end);
        let Some(owner) = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                *node.path == *requirement.location.path && node.body.contains(span)
            })
            .min_by_key(|(_, node)| node.body.end - node.body.start)
            .map(|(index, _)| index)
        else {
            continue;
        };
        // After an `await` the operation is proven unowned on its own; its
        // function's entry context does not matter.
        let proven = !requirement.uncertain
            && (requirement.after_await || contexts[owner] & OWNER_CONTEXT_PROVEN_UNOWNED != 0);
        let mask = if proven {
            OWNER_CONTEXT_PROVEN_UNOWNED
        } else {
            OWNER_CONTEXT_UNOWNED
        };
        let Some(owner_file) = lookup.file_by_path(&nodes[owner].path) else {
            continue;
        };
        // ADR 0275 at the call boundary: the operation counts for a caller
        // only on its function's every-completion path, and each hop's call
        // on its caller's. The operation's own site is its call.
        let operation_site = owner_file
            .ast
            .calls
            .iter()
            .find(|call| call.callee == span || call.span == span)
            .map(|call| call.span);
        // Per package function on a path to the operation: the site in it
        // that leads there, whether that site is on the function's
        // every-completion path, and whether every hop below it is.
        // Per package function on a path to the operation: the sites in it
        // that lead there, whether one of them is on the function's
        // every-completion path, and whether every hop below it is.
        let mut best = HashMap::<usize, (Vec<Span>, bool, bool)>::default();
        best.insert(
            owner,
            (
                vec![operation_site.unwrap_or(span)],
                operation_site.is_some_and(|site| {
                    crate::interproc::site_on_every_completion(
                        lookup,
                        owner_file,
                        nodes[owner].span,
                        site,
                    )
                }),
                true,
            ),
        );
        // An operation after an `await` in the package runs unowned whatever
        // context its function was entered with (review round 1 of ADR 0290,
        // Sol): every project call that runs it is attributed.
        let carries = |kind: OwnerEdgeKind, caller: usize| {
            requirement.after_await || owner_edge_context(kind, contexts[caller]) & mask != 0
        };
        let enters =
            |file: &solid_facts::FileFacts, call: &solid_facts::ast::CallFact, node: usize| {
                // An exact invocation of the function: not a member of it
                // (`f.reset()`, `f.bind(…)`) nor a replaced binding.
                crate::execution_role::project_callee_at_parameter(file, call, 0, lookup)
                    .is_some_and(|(callee_file, callee, exact)| {
                        exact
                            && *callee_file.path.as_str() == *nodes[node].path
                            && callee.span == nodes[node].span
                    })
            };
        // ADR 0280 on the first hop: what the call's arguments decide about
        // the callee's sites. Reached when one site is guaranteed, dropped
        // when every site is unreachable, possible otherwise.
        let first_hop = |file: &solid_facts::FileFacts,
                         call: &solid_facts::ast::CallFact,
                         node_file: &'_ solid_facts::FileFacts,
                         node_function: &solid_facts::ast::FunctionFact,
                         sites: &[Span]| {
            crate::argument_values::parameter_values(file, call, node_file, node_function, lookup)
                .filter(|values| !values.is_empty())
                .and_then(|values| {
                    solid_facts::ast::argument_site_standings(
                        std::path::Path::new(node_file.path.as_str()),
                        &node_file.source,
                        node_function.span,
                        &values,
                        &lookup.never_returning_calls_within(node_file, node_function),
                        sites,
                    )
                })
                .map(|standings| {
                    if standings.contains(&solid_facts::ast::SiteStanding::Guaranteed) {
                        solid_facts::ast::SiteStanding::Guaranteed
                    } else if standings
                        .iter()
                        .all(|standing| *standing == solid_facts::ast::SiteStanding::Unreachable)
                    {
                        solid_facts::ast::SiteStanding::Unreachable
                    } else {
                        solid_facts::ast::SiteStanding::Possible
                    }
                })
        };
        let mut queue = VecDeque::from([owner]);
        while let Some(node) = queue.pop_front() {
            let (sites, own, below) = best[&node].clone();
            let Some(node_file) = lookup.file_by_path(&nodes[node].path) else {
                continue;
            };
            let Some(node_function) = node_file.ast.function_at(nodes[node].span) else {
                continue;
            };
            for &(caller, kind) in &incoming[node] {
                if !carries(kind, caller) {
                    continue;
                }
                let created = owner_edge_context(kind, 0) & mask != 0;
                let Some(file) = lookup.file_by_path(&nodes[caller].path) else {
                    continue;
                };
                // Only calls written directly in the caller that enter this
                // function: a callback edge names no call to report at.
                let calls = file
                    .ast
                    .calls
                    .iter()
                    .filter(|call| {
                        containing_ast_function(&file.ast, call.span)
                            .is_some_and(|function| function.span == nodes[caller].span)
                            && enters(file, call, node)
                    })
                    .collect::<Vec<_>>();
                if calls.is_empty() {
                    continue;
                }
                if in_dependency(caller) {
                    if created {
                        continue;
                    }
                    let completing = calls.iter().any(|call| {
                        crate::interproc::site_on_every_completion(
                            lookup,
                            file,
                            nodes[caller].span,
                            call.span,
                        )
                    });
                    let entry = (
                        calls.iter().map(|call| call.span).collect::<Vec<_>>(),
                        completing,
                        own && below,
                    );
                    let improves = best
                        .get(&caller)
                        .is_none_or(|previous| !(previous.1 && previous.2) && entry.1 && entry.2);
                    if improves {
                        best.insert(caller, entry);
                        queue.push_back(caller);
                    }
                    continue;
                }
                // The project caller creates the bit itself (it schedules the
                // package function, `setTimeout(f)`): the unowned run is that
                // reference's, not a direct call's (review round 1).
                if created && !requirement.after_await {
                    continue;
                }
                for call in calls {
                    let reached = match first_hop(file, call, node_file, node_function, &sites) {
                        Some(solid_facts::ast::SiteStanding::Unreachable) => continue,
                        Some(solid_facts::ast::SiteStanding::Guaranteed) => below,
                        Some(solid_facts::ast::SiteStanding::Possible) => false,
                        None => own && below,
                    };
                    // The unowned context is the project function's; it
                    // reaches this call only when the call is on that
                    // function's every-completion path (review round 1, Sol):
                    // a call behind a branch or an owner guard may run only on
                    // owned invocations.
                    let call_completes =
                        frontier_call_completes(lookup, file, nodes[caller].span, call.span);
                    attributed.push((
                        file,
                        call.span,
                        requirement.clone(),
                        proven && reached && call_completes,
                    ));
                }
            }
        }
        // A project call at module scope has no owner and no function node,
        // so no edge records it. It is attributed as an obligation: whether a
        // module-scope statement runs is not proven here.
        for &node in best.keys() {
            let (sites, _, _) = &best[&node];
            let Some(node_file) = lookup.file_by_path(&nodes[node].path) else {
                continue;
            };
            let Some(node_function) = node_file.ast.function_at(nodes[node].span) else {
                continue;
            };
            for file in &facts.files {
                if facts.dependency_source_root(file.path.as_str()).is_some() {
                    continue;
                }
                for call in &file.ast.calls {
                    if containing_ast_function(&file.ast, call.span).is_some()
                        || !enters(file, call, node)
                        || first_hop(file, call, node_file, node_function, sites)
                            == Some(solid_facts::ast::SiteStanding::Unreachable)
                    {
                        continue;
                    }
                    attributed.push((file, call.span, requirement.clone(), false));
                }
            }
        }
    }
    for (file, span, requirement, proven) in attributed {
        let before = requirements.len();
        push_owner_requirement(
            requirements,
            &mut seen,
            operation_name(requirement.operation),
            file,
            span,
            OwnerRequirementStatus {
                uncertain: !proven,
                runtime_uncertain: requirement.runtime_uncertain,
                caller_uncertain: requirement.caller_uncertain,
                conditional_owner: requirement.conditional_owner,
                later_run_unowned: requirement.later_run_unowned,
                component_uncertain: requirement.component_uncertain,
                after_await: false,
                report: true,
            },
            lookup,
        );
        if requirements.len() > before
            && let Some(pushed) = requirements.last_mut()
        {
            pushed.through_dependency = Some(requirement.location.clone());
        }
    }
}

/// Whether `site`, a call written directly in the project function at
/// `function_span`, runs on that function's every completion (ADR 0276's
/// completion sites, minus an earlier `never` guard). Like
/// `interproc::site_on_every_completion`, except that only JSX, a class or a
/// pattern written inside this function defers the site: an inline handler
/// sits inside its component's JSX, and that JSX does not defer its own body.
pub(super) fn frontier_call_completes(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function_span: Span,
    site: Span,
) -> bool {
    let Some(function) = file.ast.function_at(function_span) else {
        return false;
    };
    let inside = |span: Span| function.body.contains(span) && span.contains(site);
    file.ast.completion_sites.binary_search(&site).is_ok()
        && !function.generator
        && written_directly_in(&file.ast, function, site)
        && !file
            .ast
            .jsx_elements
            .iter()
            .any(|element| inside(element.span))
        && !file.ast.classes.iter().any(|class| inside(class.span))
        && !file
            .ast
            .bindings
            .iter()
            .any(|binding| inside(binding.pattern))
        && !file
            .ast
            .assignments
            .iter()
            .any(|assignment| inside(assignment.target))
        && !lookup.never_returning_call_before(file, function, site)
}

pub(super) const fn operation_name(operation: OwnerRequirementOperation) -> &'static str {
    match operation {
        OwnerRequirementOperation::Effect => "effect",
        OwnerRequirementOperation::Cleanup => "cleanup",
        OwnerRequirementOperation::Boundary => "boundary",
        OwnerRequirementOperation::SettledCleanup => "settled-cleanup",
    }
}
