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
            // ADR 0874: a withheld carrier's callback may run here, later with
            // no owner, or never. As for a carrier that may run zero times
            // (ADR 0563), the possible unowned bit alone: nothing proves a run,
            // so what the callback does stays an obligation, never a violation.
            for index in withheld_carrier_arguments(file, call, lookup) {
                let argument = call.arguments[index].span;
                if crate::owners::argument_may_be_generator(file, argument, lookup) {
                    continue;
                }
                let Some(target) = callback_target(argument, false) else {
                    continue;
                };
                edges.push(SymbolicOwnerEdge {
                    source: None,
                    target,
                    kind: OwnerEdgeKind::MayBeUnowned,
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
                    .first_file_by_path(&edge.source_path)
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
        // ADR 0884: `onSettled` through a name no rule reads: SETTLED_CLEANUP_UNOWNED
        // has no site line, so only the owner rule's own row covers it.
        let alias_settled = known.is_none()
            && aliased_primitive(file, call, lookup)
                .and_then(|name| lookup.dialect.primitive(&name))
                == Some(Primitive::OnSettled);
        let operation = match (role, known) {
            (None, None) if alias_settled => Some((
                "settled-cleanup",
                OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF,
                None,
                // ADR 0902: `f.call(this, cb)` and `f.apply(this, [cb])` hold
                // the callback past the receiver: the row goes at the callee.
                if file
                    .ast
                    .member_at(file.ast.peel_ts_sugar_span(call.callee))
                    .is_some_and(|member| {
                        matches!(file.source_text(member.property), Some("call" | "apply"))
                    })
                {
                    call.callee
                } else {
                    call.arguments
                        .first()
                        .map_or(call.callee, |argument| argument.span)
                },
                true,
            )),
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
            // an effect is (ADR 0869: a JSX boundary is no site).
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
    // ADR 0869: a JSX `<Loading>` or `<Errored>` is no owner site. rc.21's dev
    // `createComponent` runs every component under its own transparent root,
    // so NO_OWNER_BOUNDARY comes only from the call form (`boundary-call`).
    // ADR 0883: a control-flow tag runs its function child under an owner of its
    // own, whoever owns the tag: For's and Repeat's row, Show's and Match's memo.
    for element in &file.ast.jsx_elements {
        let Some((expression, run)) = control_flow_child(file, element, lookup) else {
            continue;
        };
        let Some(target) = callback_target(expression, false) else {
            continue;
        };
        // Only a function written in place (ADR 0883): 33 written names in the
        // corpus at most, and a name keeps its other entries.
        let function = match &target {
            OwnerTarget::LocalSpan(span) if *span == file.ast.peel_ts_sugar_span(expression) => {
                file.ast.function_at(*span)
            }
            _ => None,
        };
        if function.is_some_and(|function| runs_as_child(function, run)) {
            edges.push(SymbolicOwnerEdge {
                source: None,
                target,
                kind: OwnerEdgeKind::Owned,
            });
        }
    }
    // ADR 0883: JSX creates a render effect with no call to name it.
    for site in jsx_sites(file, lookup) {
        if !matches!(
            site.kind,
            JsxSiteKind::RenderEffect | JsxSiteKind::MaybeRenderEffect
        ) {
            continue;
        }
        requirements.push(OwnerRequirementCandidate {
            operation: "jsx-effect",
            operation_span: site.span,
            owner: owner_at(site.anchor),
            report_mask: OWNER_CONTEXT_UNOWNED,
            // A value that may hold no function, and a getter read wherever
            // the component reads it, prove no effect at this owner.
            runtime_uncertain: site.in_getter || site.kind == JsxSiteKind::MaybeRenderEffect,
            settled_target: None,
            settled_gate: None,
            providing_region_chain: region_chain(site.anchor),
            providing_region: region_of(site.anchor),
            context_argument: None,
            in_parameter_list: list_site(site.anchor),
        });
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
    leaf_operations: &[crate::LeafOwnerOperation],
) -> (
    Vec<OwnerRequirement>,
    SettledGateDecisions,
    OwnerIncrementalTimings,
    Vec<crate::StaticViolation>,
    Option<OwnerView>,
) {
    let entities = lookup.entities();
    let mut timings = OwnerIncrementalTimings::default();
    let total_started = Instant::now();
    // The files of the build, each path once: a path listed twice is the
    // later slot's.
    let mut last_slot = HashMap::<&str, usize>::default();
    for (slot, file) in crate::base::reads::file_slots(facts).iter().enumerate() {
        last_slot.insert(file.path.as_str(), slot);
    }
    let files = crate::base::reads::file_slots(facts)
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
    // One check pass: each memo is asked once (`reads::PassAnswers`).
    let answers = crate::base::reads::PassAnswers::new(lookup);
    for (file, pid) in files.iter().zip(&order) {
        if unchanged_paths.contains(file.path.as_str())
            && let Some(entry) = graph.file(*pid)
            && entry.fragment.source_hash == file.source_hash
            && entry.fragment.proof.holds(&answers)
            && (Arc::ptr_eq(&entry.fragment.compiler, &file.compiler)
                || same_compiler_semantics(&entry.fragment.compiler, &file.compiler))
        {
            timings.reused_files += 1;
            continue;
        }
        recomputed.push(*file);
        timings.recomputed_files += 1;
    }
    let discovered = crate::pipeline::parallel_balanced_results_from(2, &recomputed, |file| {
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
    // ADR 0886: check once, over a universe built from the calls, not from
    // the candidates emission chose to see (reviewer D34).
    unaccounted_owner_sites(
        &files,
        &order,
        emissions,
        &mut requirements,
        lookup,
        leaf_operations,
    );
    let mut owner_view = None;
    if !facts.dependency_source_roots.is_empty() {
        let (nodes, contexts, outgoing) = graph.dense_view(&order);
        let mut walked = HashSet::default();
        attribute_dependency_requirements(
            &mut requirements,
            facts,
            lookup,
            &nodes,
            &contexts,
            &outgoing,
            &mut walked,
        );
        // ADR 0949: the project sites that enter package code and no call names.
        let package_reads = order
            .iter()
            .filter_map(|pid| emissions.get(pid))
            .flat_map(|emission| emission.package_reads.iter().cloned())
            .collect::<Vec<_>>();
        attribute_package_entries(
            &mut requirements,
            facts,
            lookup,
            &nodes,
            &contexts,
            &outgoing,
            &walked,
            &provider_free,
            &package_reads,
        );
        // ADR 0949: kept for the package entry table over the final rows.
        owner_view = Some(OwnerView {
            nodes,
            contexts,
            outgoing,
            provider_free: provider_free.clone(),
        });
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
        owner_view,
    )
}

/// ADR 0549: whether the function `call` returns reaches code no returned-
/// invocation walk names. Followed: a call's callee (the creating call's own
/// too, ADR 0931), the object of a member
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
    // ADR 0931: the call is itself a call's callee (`createReaction(f)(…)`):
    // that call is the function's one use, and the walk names it.
    if ast
        .calls
        .iter()
        .any(|outer| peel(outer.callee) == call.span)
    {
        return false;
    }
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

/// ADR 0933: how a site of SC4001 that is no obligation ends (ADR 0886's list).
#[derive(Clone, Copy)]
enum SiteEnd {
    /// A reported row stands at the site.
    Row,
    /// The file's emission recorded this proof.
    Proof(super::owner_emission::ContextProof),
    /// ADR 0942: the site is another rule's claim, and that rule has a row at it (the span is where).
    Delegated(crate::site_accounting::Delegate, Span),
    /// The compiler deleted the call.
    Discarded,
    /// An owner probe guards the call.
    OwnerProbe,
    /// An effect call proven to register nothing.
    RegistersNothing,
}

/// ADR 0933: writes the typed ending of a site that is no obligation. A row
/// is its own account (the row ledger writes it); the others are silences.
fn log_site_end(file: &solid_facts::FileFacts, site: Span, ending: SiteEnd) {
    if !crate::proof_log::enabled() {
        return;
    }
    let proof = match ending {
        SiteEnd::Row => return,
        SiteEnd::Proof(proof) => format!("ContextProof::{proof:?}"),
        SiteEnd::Delegated(delegate, row) => {
            crate::proof_log::silent(
                "SC4001",
                &site.location(file.path.shared()),
                delegate.id(),
                &[crate::proof_log::Premise::at(
                    crate::proof_log::PremiseKind::OwningFindingAtSite,
                    Some(row),
                )],
            );
            return;
        }
        SiteEnd::Discarded => "Discarded".to_owned(),
        SiteEnd::OwnerProbe => "OwnerProbe".to_owned(),
        SiteEnd::RegistersNothing => "EffectRegistersNothing".to_owned(),
    };
    crate::proof_log::silent("SC4001", &site.location(file.path.shared()), &proof, &[]);
}

/// ADR 0886: every call this build resolves to an owner-requiring primitive
/// (outside dependency source, whose sites are attributed to the project's
/// calls) is one site of SC4001. A site ends with a reported row, a named
/// proof its file's emission recorded ([`super::owner_emission::ContextProof`]),
/// the compiler's discard, an owner probe, or an effect call proven to
/// register nothing. Anything else, a file the graph has no entry for
/// included, gets `execution-context-unaccounted`. A call through an alias is
/// ADR 0884's, which claims the rule at the call.
fn unaccounted_owner_sites(
    files: &[&solid_facts::FileFacts],
    order: &[super::owner_state::PathId],
    emissions: &HashMap<super::owner_state::PathId, super::owner_emission::Emission>,
    requirements: &mut Vec<OwnerRequirement>,
    lookup: &SemanticLookup<'_>,
    leaf_operations: &[crate::LeafOwnerOperation],
) {
    let rows = requirements
        .iter()
        .filter(|row| {
            row.report && row.operation != crate::OwnerRequirementOperation::ContextProvider
        })
        .map(|row| {
            (
                row.location.path.to_string(),
                row.location.start_byte,
                row.location.end_byte,
            )
        })
        .collect::<HashSet<_>>();
    let mut seen = HashSet::default();
    for (file, pid) in files.iter().zip(order) {
        let path = file.path.as_str();
        if lookup.in_dependency_source(path) {
            continue;
        }
        let accounted = emissions.get(pid).map(|emission| &emission.accounted);
        for call in &file.ast.calls {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let Some(role) = lookup.dialect.owner_requirement_role(primitive) else {
                continue;
            };
            let (operation, site) = match role {
                OwnerRequirementRole::Effect => ("effect", call.callee),
                OwnerRequirementRole::Cleanup => ("cleanup", call.callee),
                OwnerRequirementRole::SettledCleanup => (
                    "settled-cleanup",
                    call.arguments
                        .first()
                        .map_or(call.callee, |argument| argument.span),
                ),
                OwnerRequirementRole::ContextRead => ("context-read", call.callee),
                OwnerRequirementRole::Boundary => ("boundary-call", call.callee),
            };
            // ADR 0933: the same endings in the same order, each named.
            let ending =
                if rows.contains(&(path.to_owned(), u64::from(site.start), u64::from(site.end))) {
                    Some(SiteEnd::Row)
                } else if let Some((_, proof)) =
                    accounted.and_then(|proofs| proofs.iter().find(|(span, _)| *span == site))
                {
                    // ADR 0942: a leaf owner's refusal is SC3001's claim; the hand-off
                    // ends the site only where SC3001 has a row inside this call, else
                    // the site is unaccounted for (an uncertifiable row below).
                    if *proof == super::owner_emission::ContextProof::LeafOwned {
                        crate::site_accounting::delegate_row(
                            crate::site_accounting::Delegate::LeafOwnerForbiddenCall,
                            leaf_operations,
                            path,
                            call.span,
                        )
                        .map(|row| {
                            SiteEnd::Delegated(
                                crate::site_accounting::Delegate::LeafOwnerForbiddenCall,
                                row,
                            )
                        })
                    } else {
                        Some(SiteEnd::Proof(*proof))
                    }
                } else if crate::execution_role::discarded_region_contains(file, site) {
                    Some(SiteEnd::Discarded)
                } else if guarded_by_owner_probe(file, site, lookup) {
                    Some(SiteEnd::OwnerProbe)
                } else if role == OwnerRequirementRole::Effect
                    && crate::effect_api::classify_effect_call(file, call, primitive, lookup)
                        .owner_registration
                        == ProofStatus::No
                {
                    Some(SiteEnd::RegistersNothing)
                } else {
                    None
                };
            if let Some(ending) = ending {
                log_site_end(file, site, ending);
                continue;
            }
            push_owner_requirement(
                requirements,
                &mut seen,
                operation,
                file,
                site,
                OwnerRequirementStatus {
                    uncertain: true,
                    runtime_uncertain: false,
                    caller_uncertain: false,
                    unplaced_callback: false,
                    conditional_owner: false,
                    owner_varies_by_caller: false,
                    later_run_unowned: false,
                    component_uncertain: false,
                    provider_open: false,
                    default_unproven: false,
                    owned_run_throws: false,
                    after_await: false,
                    after_yield: false,
                    flight_error_unobserved: false,
                    context_unaccounted: true,
                    report: true,
                },
                lookup,
            );
        }
    }
}
