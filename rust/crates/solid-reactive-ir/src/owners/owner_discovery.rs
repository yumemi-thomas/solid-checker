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
    // ADR 0714: whether the call runs on every run of the function around it,
    // or on every evaluation of the module (one parse per file). An edge from
    // a call that may not run proves no run of its target: it keeps its
    // source's owner without the proof (`MayRun`), or for a primitive's
    // unowned callback the possible bit alone. In a package analyzed as
    // source the project call's attribution asks each hop instead, with the
    // call's arguments (ADR 0290, `attribute_dependency_requirements`).
    let every_run = std::cell::OnceCell::new();
    let in_dependency = lookup.in_dependency_source(file.path.as_str());
    let runs_every_time = |span: Span| {
        in_dependency
            || every_run
                .get_or_init(|| {
                    solid_facts::ast::every_unconditional_call(
                        std::path::Path::new(file.path.as_str()),
                        &file.source,
                    )
                })
                .as_ref()
                .is_some_and(|calls| calls.contains(&span))
    };
    // Keyed by function span: the emission pass resolves it to the node whose
    // propagated context it reads, as it does `owner`.
    // A region never answers for the stepped generator of an action: its
    // body runs where the action is called, never where it is written, so an
    // operation under one is judged on the graph (its callers' edges).
    let region_chain = |span| {
        providing_region_chain(
            &providing_regions,
            nodes.iter().map(|node| (node.span, node.span, node.body)),
            span,
        )
        .filter(|chain: &Vec<Span>| {
            !chain.iter().any(|function| {
                file.ast.function_at(*function).is_some_and(|function| {
                    stepped_action_generator(file, function, lookup).is_some()
                })
            })
        })
    };
    let region_of = |span| providing_region(&providing_regions, span);
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
            // ADR 0516: a copy, a `const` holding a function expression, or an
            // object literal's slot names exactly one function.
            .or_else(|| {
                crate::owners::exact_function_argument(file, argument, lookup)
                    .map(|(path, span)| OwnerTarget::Function(path, span))
            })
            // The stepped generator of an action a `const` holds: the action
            // runs its first step where it is called.
            .or_else(|| named_action_generator(file, argument, lookup).map(OwnerTarget::LocalSpan))
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
                kind: if crate::execution_role::handler_event_denied(file, callback.span) {
                    OwnerEdgeKind::Conditional
                } else {
                    event_owner_edge(lookup, function.generator)
                },
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
                    // The key the literal holds it under names the event; a
                    // value no literal key names has no measured event.
                    kind: match lookup.file_by_path(path).filter(|file| {
                        file.ast.object_properties.iter().any(|property| {
                            file.ast.peel_ts_sugar_span(property.value) == *span
                                && file.source_text(property.key).is_some_and(|key| {
                                    crate::runtime_semantics::event_keeps_delivery_proof(
                                        key.trim_matches(['\'', '"']),
                                    )
                                })
                        })
                    }) {
                        Some(file) => event_owner_edge(
                            lookup,
                            file.ast
                                .function_at(*span)
                                .is_some_and(|function| function.generator),
                        ),
                        None => OwnerEdgeKind::Conditional,
                    },
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
                named_action_generator(file, call.callee, lookup).map(OwnerTarget::LocalSpan)
            })
            .or_else(|| {
                call_edge_symbol(file, call, entities, lookup)
                    .cloned()
                    .map(OwnerTarget::Symbol)
            });
        let in_list = list_site(call.span);
        let source = owner.filter(|_| !in_list).map(|span| OwnerSource {
            path: file.path.to_string(),
            span,
        });
        let kind = if in_list {
            OwnerEdgeKind::MayBeUnowned
        } else {
            call_owner_edge(file, call.callee, lookup)
        };
        // ADR 0468: an edge to a function the call may run, not one it does:
        // the source's owner without its proof ([`OwnerEdgeKind::MayRun`]).
        // In a parameter list or after the function's own `await` the owner
        // is not the source's, so it is the bare uncertain bit there.
        let may_run = || {
            if kind == OwnerEdgeKind::Preserve {
                (source.clone(), OwnerEdgeKind::MayRun)
            } else {
                (None, OwnerEdgeKind::MayBeUnowned)
            }
        };
        // ADR 0468: a comma, conditional or logical callee (`(c && f)()`)
        // names no target, so it had no edge, and a function it may evaluate
        // to no owner context: proof that its body never runs. Each identifier
        // or object member the callee may evaluate to may run here
        // (`(0, o.m)()` calls `o.m` without its receiver).
        if target.is_none() {
            for operand in file.ast.composite_callee_operands(call.callee) {
                let target = lookup
                    .object_member_function(file, operand)
                    .map(|(file, function)| {
                        OwnerTarget::Function(file.path.to_string(), function.span)
                    })
                    .or_else(|| {
                        crate::owners::is_identifier_span(file, operand)
                            .then(|| entities.get(&location(file.path.shared(), operand)))
                            .flatten()
                            .cloned()
                            .map(OwnerTarget::Symbol)
                    });
                if let Some(target) = target {
                    let (source, kind) = may_run();
                    edges.push(SymbolicOwnerEdge {
                        source,
                        target,
                        kind,
                    });
                }
            }
        }
        // ADR 0468: a binding written anywhere may hold another value at this
        // call (`let k = () => …; k = other; k()`), and a mention that may run
        // before its binding's declarator throws instead of calling (ADR 0339),
        // so the function the binding was declared with may run here.
        let unproven_callee = matches!(target, Some(OwnerTarget::Symbol(_)))
            && (lookup.may_precede_initialization(file.path.as_str(), call.callee)
                || file
                    .ast
                    .reference_declaration(file.ast.peel_ts_sugar_span(call.callee))
                    .is_some_and(|declaration| crate::indexes::binding_written(file, declaration)));
        // ADR 0851: calling a generator runs none of its body: the edge is a
        // step's, from where the step runs. A literal called in place is the
        // ADR 0516 arm's below; an action's stepped generator is its own.
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let steps = (!call.construct
            && file.ast.function_at(callee).is_none()
            && matches!(
                target,
                Some(OwnerTarget::Symbol(_) | OwnerTarget::Function(..))
            )
            && lookup
                .function_called_at(file.path.as_str(), callee)
                .or_else(|| lookup.object_member_function(file, call.callee))
                .is_some_and(|(_, function)| function.generator))
        .then(|| crate::owners::iterator_steps(file, call, lookup));
        if let Some(target) = target {
            match steps {
                Some(crate::owners::IteratorSteps::Held(sites)) => {
                    for site in sites {
                        let in_list = list_site(site);
                        let source = owner_at(site).filter(|_| !in_list).map(|span| OwnerSource {
                            path: file.path.to_string(),
                            span,
                        });
                        let kind = if in_list {
                            OwnerEdgeKind::MayBeUnowned
                        } else {
                            call_owner_edge(file, site, lookup)
                        };
                        let kind = if kind == OwnerEdgeKind::Preserve && !runs_every_time(site) {
                            OwnerEdgeKind::MayRun
                        } else {
                            kind
                        };
                        edges.push(SymbolicOwnerEdge {
                            source,
                            target: target.clone(),
                            kind,
                        });
                    }
                }
                Some(crate::owners::IteratorSteps::Unplaced) => edges.push(SymbolicOwnerEdge {
                    source: None,
                    target,
                    kind: OwnerEdgeKind::MayBeUnowned,
                }),
                Some(crate::owners::IteratorSteps::InPlace) | None => {
                    let (source, kind) = if unproven_callee
                        || (kind == OwnerEdgeKind::Preserve && !runs_every_time(call.span))
                    {
                        may_run()
                    } else {
                        (source.clone(), kind)
                    };
                    edges.push(SymbolicOwnerEdge {
                        source,
                        target,
                        kind,
                    });
                }
            }
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
                        match call_owner_edge(file, call.callee, lookup) {
                            OwnerEdgeKind::Preserve if !runs_every_time(call.span) => {
                                OwnerEdgeKind::MayRun
                            }
                            kind => kind,
                        }
                    },
                });
            }
        }
        // ADR 0561: a function a reviewed standard-library carrier runs before
        // it returns (`[1].forEach(hf)`, `Reflect.apply(hf, …)`) runs on the
        // call's stack, under the owner of the code around it, as a direct
        // call of it would. A generator runs none of its body there, and a
        // placement that runs under another owner (an instance field, the
        // owner's own JSX) gets no edge, as for an invoked literal.
        if known_primitive(&call_primitives[call_index]).is_none()
            && invocation_runs_under_owner(file, call.span, owner)
        {
            // ADR 0563: a carrier that may run its callback zero times (an
            // empty or unknown-length receiver) proves no run: the possible
            // unowned bit alone, so what the callback does stays an
            // obligation, never a violation.
            let runs = crate::carrier_runs::carrier_provably_runs(file, call.span, lookup);
            for index in inline_carrier_arguments(file, call, lookup) {
                let argument = call.arguments[index].span;
                if crate::owners::argument_may_be_generator(file, argument, lookup) {
                    continue;
                }
                let Some(target) = callback_target(argument, false) else {
                    continue;
                };
                let (source, kind) = if !runs {
                    (None, OwnerEdgeKind::MayBeUnowned)
                } else if unproven_handed_function(file, &target, argument, lookup)
                    || !runs_every_time(call.span)
                {
                    may_run()
                } else {
                    (source.clone(), kind)
                };
                edges.push(SymbolicOwnerEdge {
                    source,
                    target,
                    kind,
                });
            }
        }
        // ADR 0516: a generator called in place whose iterator is stepped at
        // the call runs its body there, up to a `yield`, under the owner of
        // the code around it: it may run, never a proof that it does.
        if let Some(function) = file
            .ast
            .function_at(file.ast.peel_ts_sugar_span(call.callee))
            .filter(|function| function.generator && !call.construct)
            && crate::owners::generator_iterator_used(file, function)
                .is_some_and(|created| created.span == call.span)
            && crate::owners::iterator_stepped_at(file, call.span)
        {
            let invoker =
                enclosing_function_span(&nodes, &nodes_by_path, file.path.as_str(), call.span);
            let under_owner = invocation_runs_under_owner(
                file,
                call.span,
                invoker.map(|index| nodes[index].span),
            );
            edges.push(SymbolicOwnerEdge {
                source: invoker
                    .filter(|_| !in_list && under_owner)
                    .map(|index| OwnerSource {
                        path: file.path.to_string(),
                        span: nodes[index].span,
                    }),
                target: OwnerTarget::LocalSpan(function.span),
                // After a suspension the step runs with whatever owner the
                // continuation has, which the source's context does not say.
                kind: if in_list
                    || !under_owner
                    || invoker.is_none()
                    || crate::execution_role::may_follow_own_await(file, call.span)
                    || crate::execution_role::follows_any_suspension(file, call.span)
                {
                    OwnerEdgeKind::MayBeUnowned
                } else {
                    OwnerEdgeKind::MayRun
                },
            });
        }
        let callback_edges = owner_callback_edges(file, call, &call_primitives[call_index], lookup);
        let helper_call =
            callback_edges.is_empty() && known_primitive(&call_primitives[call_index]).is_none();
        for edge in callback_edges {
            let kind = callback_call_owner_edge(lookup, &edge.source_path, edge.source, edge.kind);
            // ADR 0548: an exact effect bundle names its members: the `effect`
            // member runs as the slot's callback, the `error` member from the
            // flush's error branch with no owner. ADR 0613: a drain a
            // synchronous `flush()` makes runs it under that call's owner.
            let bundle_targets = || {
                let primitive = known_primitive(&call_primitives[call_index])?;
                let error = crate::execution_role::bundle_error_member(
                    lookup.dialect,
                    primitive,
                    call,
                    edge.argument,
                )?;
                let error_kind = drained_callback_owner_edge_kind(
                    lookup,
                    primitive,
                    edge.argument,
                    solid_dialect::CallbackOwner::None,
                );
                let effect = crate::execution_role::effect_apply_span(call, edge.argument)
                    .filter(|effect| *effect != call.arguments[edge.argument].span);
                Some(
                    effect
                        .and_then(|effect| callback_target(effect, false))
                        .map(|target| (target, kind))
                        .into_iter()
                        .chain(callback_target(error, false).map(|target| (target, error_kind)))
                        .collect::<Vec<_>>(),
                )
            };
            let targets = match call
                .arguments
                .get(edge.argument)
                .and_then(|argument| callback_target(argument.span, false))
            {
                Some(target) => vec![(target, kind)],
                None => bundle_targets().unwrap_or_default(),
            };
            for (target, kind) in targets {
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
                } else if !runs_every_time(call.span) {
                    // ADR 0714: a callback whose call may not run is not
                    // proven to run.
                    match kind {
                        OwnerEdgeKind::Preserve => (source, OwnerEdgeKind::MayRun),
                        OwnerEdgeKind::Unowned => (None, OwnerEdgeKind::MayBeUnowned),
                        kind => (source, kind),
                    }
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
        // ADR 0474: a function literal an exact helper calls in its own body
        // runs in the helper's call, on its stack and under the caller's owner
        // (ADR 0457's census, `helper_argument_run`). A call on some paths
        // only may run it; a helper that also stores or hands it on may run it
        // elsewhere too, with no owner this proves.
        if helper_call {
            for index in 0..call.arguments.len() {
                let Some(target) = callback_target(call.arguments[index].span, false)
                    .filter(|target| matches!(target, OwnerTarget::LocalSpan(_)))
                else {
                    continue;
                };
                let Some(run) =
                    crate::execution_role::helper_argument_owner_runs(file, call, index, lookup)
                else {
                    continue;
                };
                if run.unguarded {
                    let (in_place_source, in_place_kind) =
                        if run.every_completion && !unproven_callee {
                            (source.clone(), kind)
                        } else {
                            may_run()
                        };
                    edges.push(SymbolicOwnerEdge {
                        source: in_place_source,
                        target: target.clone(),
                        kind: in_place_kind,
                    });
                }
                // Amendment: a call behind `getOwner()` runs only under an
                // owner, one behind `!getOwner()` only with none; a hand-on to
                // an owner-creating position (`createRoot(fn)`) runs under the
                // owner that call creates.
                let guarded = |guarded_kind| {
                    if kind == OwnerEdgeKind::Preserve && !unproven_callee {
                        (source.clone(), guarded_kind)
                    } else {
                        may_run()
                    }
                };
                let guarded_edges = [
                    run.owner_guarded
                        .then(|| guarded(OwnerEdgeKind::OwnerGuarded)),
                    run.no_owner_guarded.map(|proven| {
                        guarded(if proven {
                            OwnerEdgeKind::ProvenNoOwnerGuarded
                        } else {
                            OwnerEdgeKind::NoOwnerGuarded
                        })
                    }),
                    run.rooted.then(|| (source.clone(), OwnerEdgeKind::Owned)),
                ];
                for (guarded_source, guarded_kind) in guarded_edges.into_iter().flatten() {
                    edges.push(SymbolicOwnerEdge {
                        source: guarded_source,
                        target: target.clone(),
                        kind: guarded_kind,
                    });
                }
                if run.elsewhere {
                    edges.push(SymbolicOwnerEdge {
                        source: None,
                        target,
                        kind: OwnerEdgeKind::MayBeUnowned,
                    });
                }
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
        // ADR 0549: the creating call of such a function whose result escapes
        // where no invocation can be named (exported, handed whole to a call,
        // a JSX attribute's value, or held by no binding): some invocation
        // may run its work under this call's owner, so an obligation here.
        if !in_list
            && let Some(primitive) = known
            && dialect.returned_invocation_owner_requirement_role(primitive)
                == Some(OwnerRequirementRole::Effect)
            && returned_function_escapes(file, call)
        {
            requirements.push(OwnerRequirementCandidate {
                operation: "effect",
                operation_span: call.callee,
                owner,
                report_mask: OWNER_CONTEXT_UNOWNED,
                allow_uncertain: true,
                runtime_uncertain: true,
                settled_target: None,
                settled_gate: None,
                providing_region_chain: providing_region_chain.clone(),
                providing_region: region_of(call.span),
                context_argument: None,
                in_parameter_list: false,
            });
        }
        // ADR 0549: an invocation of a function a primitive returned that
        // creates its work under the owner captured where the primitive was
        // called (`createReaction`'s `track()`): the requirement is placed at
        // the invocation and judged at the creating call. A chain through a
        // `let` is an obligation; a creating call
        // in another file is placed only at its module's top level, where no
        // owner is current.
        if known.is_none()
            && !in_list
            && let Some((placed_here, answer)) = lookup.returned_factory_at(file, call)
            && answer.slot.is_none()
            && dialect.returned_invocation_owner_requirement_role(answer.primitive)
                == Some(OwnerRequirementRole::Effect)
        {
            // Placed at the call whose owner created the function, in this
            // file; another module's top level has none. Anything else is the
            // creating call's escape obligation (`returned_function_escapes`).
            let placement = match placed_here {
                Some(at) => Some((owner_at(at), region_chain(at), region_of(at))),
                None => answer.factory_at_module_scope.then_some((None, None, None)),
            };
            if let Some((owner, providing_region_chain, providing_region)) = placement {
                requirements.push(OwnerRequirementCandidate {
                    operation: "effect",
                    operation_span: call.callee,
                    owner,
                    report_mask: OWNER_CONTEXT_UNOWNED,
                    allow_uncertain: true,
                    // A `let` on the walk may hold another function by the
                    // time it is called: an obligation, never a proof.
                    runtime_uncertain: !answer.exact,
                    settled_target: None,
                    settled_gate: None,
                    providing_region_chain,
                    providing_region,
                    context_argument: None,
                    in_parameter_list: false,
                });
            }
        }
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
                providing_region: region_of(call.span),
                context_argument,
                in_parameter_list: in_list,
            });
        }
    }
    // ADR 0562: an arrow written as a prop of a same-file project component
    // that only ever calls the prop (ADR 0538's proof,
    // `component_prop_invocations`) runs at each such call, under the owner
    // of the component's body, as a direct call there would. Only a call
    // written in the component's own body or its JSX: one in a nested
    // function (a handler, an `onSettled` callback) runs under that
    // function's owner, which this does not follow. A forwarded, spread,
    // defaulted or copied prop is not followed, and a component in another
    // file is not either (the edge would rest on another file's facts).
    for literal in &file.ast.functions {
        if literal.kind != solid_facts::ast::FunctionKind::Arrow {
            continue;
        }
        let Some((element, _)) = crate::execution_role::component_prop_literal(file, literal.span)
        else {
            continue;
        };
        let Some((callee_file, calls)) =
            crate::execution_role::component_prop_invocations(file, literal, lookup)
        else {
            continue;
        };
        let Some((_, component)) = lookup.function_called_at(file.path.as_str(), element.name.span)
        else {
            continue;
        };
        if callee_file.path != file.path {
            continue;
        }
        for at in calls {
            let Some(call) = file.ast.call_at(at) else {
                continue;
            };
            if owner_at(call.span) != Some(component.span) {
                continue;
            }
            let in_list = list_site(call.span);
            let source = owner_at(call.span)
                .filter(|_| !in_list)
                .map(|span| OwnerSource {
                    path: file.path.to_string(),
                    span,
                });
            let kind = if in_list {
                OwnerEdgeKind::MayBeUnowned
            } else {
                call_owner_edge(file, call.callee, lookup)
            };
            edges.push(SymbolicOwnerEdge {
                source,
                target: OwnerTarget::LocalSpan(literal.span),
                kind,
            });
        }
    }
    // ADR 0561: a tagged template calls its tag in place, on the caller's
    // stack, as a direct call does.
    for template in &file.ast.tagged_templates {
        let owner = owner_at(template.span);
        if !invocation_runs_under_owner(file, template.span, owner)
            || crate::owners::argument_may_be_generator(file, template.tag, lookup)
        {
            continue;
        }
        let Some(target) = callback_target(template.tag, false) else {
            continue;
        };
        let in_list = list_site(template.span);
        let kind = if in_list {
            OwnerEdgeKind::MayBeUnowned
        } else {
            call_owner_edge(file, template.span, lookup)
        };
        let source = owner.filter(|_| !in_list).map(|span| OwnerSource {
            path: file.path.to_string(),
            span,
        });
        let (source, kind) = if unproven_handed_function(file, &target, template.tag, lookup) {
            if kind == OwnerEdgeKind::Preserve {
                (source, OwnerEdgeKind::MayRun)
            } else {
                (None, OwnerEdgeKind::MayBeUnowned)
            }
        } else {
            (source, kind)
        };
        edges.push(SymbolicOwnerEdge {
            source,
            target,
            kind,
        });
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
                _ if lookup.synchronous_event_dispatch_present()
                    || crate::execution_role::handler_event_denied(file, callback.span) =>
                {
                    OwnerEdgeKind::Conditional
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
                providing_region: region_of(element.span),
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

/// ADR 0561: whether a handed function named through a binding may not be the
/// function that binding was declared with when the carrier runs it: a
/// mention that may precede the declarator, or a binding written anywhere
/// (ADR 0468's `unproven_callee`, for a direct call). An exact target (a
/// literal, a `const` holding one, an object slot) is already proven.
fn unproven_handed_function(
    file: &solid_facts::FileFacts,
    target: &OwnerTarget,
    mention: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    matches!(target, OwnerTarget::Symbol(_))
        && (lookup.may_precede_initialization(file.path.as_str(), mention)
            || file
                .ast
                .reference_declaration(file.ast.peel_ts_sugar_span(mention))
                .is_none_or(|declaration| crate::indexes::binding_written(file, declaration)))
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
    Vec<crate::StaticViolation>,
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
    let mut static_violations = Vec::new();
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
            static_violations.extend(emission.static_violations.iter().cloned());
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
        static_violations,
    )
}

/// ADR 0549: whether the function `call` returns reaches code no returned-
/// invocation walk names. Followed: a call's callee, the object of a member
/// that is a call's callee, the initializer of a single-name binding (a copy),
/// and a value of a closed object literal or a tuple literal that is a
/// binding's whole initializer (that binding's names are followed in turn).
/// Anything else escapes: the call is no binding's whole initializer, a holder
/// is exported, or a reference is handed to a call, written as a JSX
/// attribute's value, returned, assigned, or written in any other position.
fn returned_function_escapes(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> bool {
    let ast = &file.ast;
    let peel = |span: Span| ast.peel_ts_sugar_span(span);
    let Some(name) = ast
        .bindings_initializer_containing(call.span)
        .find(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.names.len() == 1
                && binding
                    .initializer
                    .is_some_and(|initializer| peel(initializer) == call.span)
        })
        .and_then(|binding| binding.names.first())
    else {
        return true;
    };
    // The binding whose whole initializer is the literal directly holding
    // `value` (its property's value or its element), when that literal is a
    // closed object literal or a tuple literal.
    let literal_binding = |value: Span| {
        let literal = ast
            .object_get_shapes
            .iter()
            .filter(|shape| shape.closed && shape.span.contains(value))
            .map(|shape| shape.span)
            .chain(
                ast.array_literal_elements
                    .iter()
                    .filter(|array| {
                        array.elements.iter().any(|element| {
                            element.kind == solid_facts::ast::ArrayElementKind::Value
                                && peel(element.span) == value
                        }) && !array.elements.iter().any(|element| {
                            element.kind == solid_facts::ast::ArrayElementKind::Spread
                        })
                    })
                    .map(|array| array.span),
            )
            .min_by_key(|span| span.end - span.start)?;
        // The value must be the literal's own: a property value or element,
        // not something nested deeper.
        let own = ast.array_literal_at(literal).is_some()
            || crate::execution_role::closed_literal_value_is_own(file, literal, value);
        if !own {
            return None;
        }
        ast.bindings.iter().find(|binding| {
            binding
                .initializer
                .is_some_and(|initializer| peel(initializer) == literal)
        })
    };
    let mut holders = vec![name.span];
    let mut index = 0;
    while index < holders.len() {
        let holder = holders[index];
        index += 1;
        if index > 16 || crate::execution_role::flow_binding_exported(file, holder) {
            return true;
        }
        for &(reference, declaration) in &ast.reference_declarations {
            if declaration != holder {
                continue;
            }
            if ast
                .calls
                .iter()
                .any(|inner| peel(inner.callee) == reference)
            {
                continue;
            }
            if ast.members.iter().any(|member| {
                peel(member.object) == reference
                    && ast
                        .calls
                        .iter()
                        .any(|inner| peel(inner.callee) == member.span)
            }) {
                continue;
            }
            if let Some(copy) = ast.bindings.iter().find(|binding| {
                binding
                    .initializer
                    .is_some_and(|initializer| peel(initializer) == reference)
            }) {
                holders.extend(copy.names.iter().map(|name| name.span));
                continue;
            }
            match literal_binding(reference) {
                Some(binding) => holders.extend(binding.names.iter().map(|name| name.span)),
                None => return true,
            }
        }
    }
    false
}
