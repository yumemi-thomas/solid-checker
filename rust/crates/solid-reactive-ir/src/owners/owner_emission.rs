//! Per-file requirement emission over the kept owner graph (S7f).
//!
//! A file's requirements are a function of its candidates (the fragment), the
//! final contexts of its own functions, the file's `await` continuations, the
//! lookup answers it read (recorded: they are folded into the fragment's
//! proof, so a moved answer reruns the fragment and with it the emission), and
//! two answers kept beside the rows because a file's reuse cannot see them
//! move:
//!
//! - the closed provider census's verdict for each context a `useContext`
//!   read names ([`Emission::provider`]), and
//! - for an `onSettled` cleanup, the callback it resolves to and that file's
//!   source hash ([`Emission::callbacks`]): the cleanup proof reads the
//!   callback's body. It also reads that file's entity rows, which another
//!   file's edit can move under an unchanged text, so the emission records
//!   the callback file as read (`note_file_read`).
//!
//! The dedup of `push_owner_requirement` keys on the requirement's path, so a
//! file's emission is the same whichever other files are emitted around it,
//! and the project's requirements are the files' in path order.

use super::owner_state::{NodeId, OwnerGraph, PathId};
use super::*;
use crate::role_log::arm_id;
use typefacts::Location;

/// One file's emitted requirements and what they were emitted under.
#[derive(Default)]
pub(crate) struct Emission {
    pub(super) requirements: Vec<OwnerRequirement>,
    pub(super) gates: Vec<((String, u64, u64), LeafGateDecision)>,
    /// The census's verdict for each context a `useContext` read named
    /// (`None`: no project context with no default).
    pub(super) provider: Vec<(Location, Option<ContextVerdict>)>,
    /// ADR 0713: reads of an open context no provider is proven above.
    pub(super) static_violations: Vec<crate::StaticViolation>,
    /// Each `onSettled` cleanup's resolved callback node and its file's hash.
    pub(super) callbacks: Vec<(OwnerTarget, Option<NodeId>, Option<SourceHash>)>,
}

pub(super) struct EmissionInputs<'a, 'f, 'i> {
    pub(super) facts: &'a ProjectFacts,
    pub(super) lookup: &'a SemanticLookup<'f>,
    pub(super) indexes: &'a ProjectIndexes<'i>,
    pub(super) provider_free: &'a HashMap<Location, ContextVerdict>,
}

impl EmissionInputs<'_, '_, '_> {
    /// Whether the answers `emission` was made under still hold.
    pub(super) fn holds(&self, graph: &OwnerGraph, pid: PathId, emission: &Emission) -> bool {
        emission
            .provider
            .iter()
            .all(|(key, verdict)| self.provider_free.get(key).copied() == *verdict)
            && emission.callbacks.iter().all(|(target, node, hash)| {
                graph.resolve_target(pid, target) == *node
                    && self.callback_hash(graph, *node) == *hash
            })
    }

    /// The source hash of the file holding the callback `node`.
    fn callback_hash(&self, graph: &OwnerGraph, node: Option<NodeId>) -> Option<SourceHash> {
        let node = graph.node(node?)?;
        let file = self.indexes.files_by_path.get(node.path.as_str())?;
        Some(file.source_hash.clone())
    }
}

/// The starts of the calls in `path` that run after an `await` on every path
/// through their async function's own body (the producer's
/// `calls_after_await` dominance fact, the one `reactive-read-after-await`
/// proves reads with): branches, `&&`, `try`/`catch`, loops and `switch` merge
/// conservatively, and a nested closure is never scanned. Such a call runs
/// from a promise continuation on an empty stack, where the dialect's
/// fresh-stack answer says whether an owner is current; Solid 2.0's owner is
/// a synchronous dynamic scope, so none is. Empty for a dialect that does not
/// answer [`solid_dialect::CallbackOwner::None`]. A file's own set: the
/// producer files each fact under the path of its call.
fn await_starts(inputs: &EmissionInputs<'_, '_, '_>, path: &str) -> HashSet<u64> {
    let mut starts = HashSet::default();
    if inputs.lookup.dialect.fresh_stack_callback_owner()
        == Some(solid_dialect::CallbackOwner::None)
        && let Some(file) = inputs.facts.typescript.file(path)
    {
        for function in file.async_functions.iter() {
            for call in &function.calls_after_await {
                if call.path.as_ref() == path {
                    starts.insert(call.start_byte);
                }
            }
        }
    }
    starts
}

pub(super) fn emit_file(
    inputs: &EmissionInputs<'_, '_, '_>,
    graph: &OwnerGraph,
    pid: PathId,
    file: &solid_facts::FileFacts,
) -> Emission {
    let lookup = inputs.lookup;
    let mut emission = Emission::default();
    let Some(entry) = graph.file(pid) else {
        return emission;
    };
    let path = file.path.as_str();
    let starts = await_starts(inputs, path);
    let mut seen = HashSet::default();
    // ADR 0534: whether a chain function nothing places ever runs (ADR 0458's
    // query, memoized across this file's candidates). Its lookups are recorded
    // with the emission's reads, so a moved answer reruns the emission.
    let reach = crate::execution_role::SiteReach::new(lookup);
    let context_of = |span: &Span| {
        graph
            .context(pid, *span)
            .unwrap_or(OWNER_CONTEXT_LEAVES_REGION)
    };
    // ADR 0868: the function making a run at `at` holds context zero (no
    // edge reaches it) and is not proven never to run.
    let caller_unplaced = |at: Span| {
        file.ast
            .innermost_function_owning(at)
            .is_some_and(|caller| {
                graph
                    .node_context(pid, caller.span)
                    .is_some_and(|(_, context)| context == 0)
                    && reach.function_runs(file, caller)
            })
    };
    for candidate in &entry.fragment.requirements {
        let owner = candidate
            .owner
            .and_then(|span| graph.node_context(pid, span));
        let mut context = owner.map_or(OWNER_CONTEXT_UNOWNED, |(_, context)| context);
        // ADR 0347's closed-program export census is stronger than SiteReach,
        // which deliberately treats module-scope declarations as possible entries.
        // Do not invent a caller for a dormant export or its nested callbacks.
        if file.ast.functions.iter().any(|function| {
            function.body.contains(candidate.operation_span)
                && graph
                    .node_context(pid, function.span)
                    .is_some_and(|(node, context)| node.namespace_candidate && context == 0)
        }) {
            // Gate decisions are consumed by leaf-owner discovery too. A dormant
            // registration must keep its no-entry answer, not leave that gate
            // undecided and turn its callback into a new leaf violation.
            if let Some(gate) = candidate.settled_gate {
                emission.gates.push((
                    (path.to_owned(), u64::from(gate.start), u64::from(gate.end)),
                    leaf_gate_decision(owner),
                ));
            }
            continue;
        }
        // The role log's answer for this candidate (ADR 0533): which arm set
        // its context last. Write-only; nothing below reads it.
        let mut arm = if owner.is_none() {
            arm_id!("owner_emission.no-owner-node")
        } else {
            arm_id!("owner_emission.region-context")
        };
        // A function no owner edge reaches holds context zero: its body never
        // runs from syntax. Its parameter list is no such claim (ADR 0454).
        if candidate.in_parameter_list && context == 0 {
            context = OWNER_CONTEXT_UNOWNED;
            arm = arm_id!("owner_emission.parameter-list");
        }
        // ADR 0472: context zero is no proof for a function whose value its
        // function returns: the caller of that function may call it later, on
        // a timer or in a handler, and no owner edge follows the returned
        // value (through `runWithOwner`, `createRoot`, `untrack`, a memo or a
        // plain factory call). The operation is unplaced, never certified.
        let returned_unplaced = context == 0
            && !candidate.in_parameter_list
            && candidate
                .owner
                .is_some_and(|span| returned_unreached_function(graph, pid, file, span));
        if returned_unplaced {
            context = OWNER_CONTEXT_UNOWNED;
            arm = arm_id!("owner_emission.returned-unplaced");
        }
        // ADR 0534: a function proven never to run (ADR 0458's query) has no
        // operation to place.
        let unreached = |span: &Span| {
            file.ast
                .function_at(*span)
                .is_some_and(|function| !reach.function_runs(file, function))
        };
        // ADR 0534: the region answers only for functions proven to run in
        // its synchronous extent. Every chain span is one of this file's own
        // nodes; one that did not resolve would withhold the lexical answer,
        // not grant it.
        let region_answers = candidate
            .providing_region_chain
            .as_deref()
            .is_some_and(|chain| {
                root_owned_at(Some(chain), context_of)
                    && !region_run_unproven(
                        chain,
                        context_of,
                        |span| Some(*span) == candidate.providing_region,
                        unreached,
                    )
            });
        // The operation's own function is an export holding the unknown-caller
        // seed (its open-world seed, or ADR 0347's namespace hand-out): that
        // seed is its owner edge, a caller this build cannot see, so its
        // unowned bit is the exported-caller cause below, not ADR 0534's
        // unplaced callback. The verdict is the same obligation either way.
        let exported_seed = owner.is_some_and(|(node, node_context)| {
            node.exported
                && node_context & OWNER_CONTEXT_UNOWNED != 0
                && !node.component
                && !node.component_uncertain
        });
        // ADR 0534: context zero is no proof for a function that may run
        // either: nothing calls it from syntax, so whoever runs it (a wrapper
        // it was handed to, the holder of a member it was assigned to, a
        // timer constructing its class, a getter's reader) is not followed,
        // and no region it is written in answers for it. The operation is
        // unplaced, never certified.
        let unplaced_callback = context
            & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF_OWNED | OWNER_CONTEXT_PROVEN_UNOWNED)
            == 0
            && !candidate.in_parameter_list
            && !returned_unplaced
            && !region_answers
            && !exported_seed
            && candidate.owner.is_some_and(|span| !unreached(&span));
        if unplaced_callback {
            // The caller nothing follows may supply no owner, as for an
            // open-world seed; a context-zero function holds that bit alone.
            context = OWNER_CONTEXT_UNOWNED;
        }
        // As in the fragment: effects, cleanups, context reads and boundary
        // calls, never `onSettled`'s returned cleanup. A
        // context read after an `await` is unowned (ADR 0352), and so is a
        // `Loading(props)` call (ADR 0462).
        let continuation_placed = matches!(
            candidate.operation,
            "effect" | "cleanup" | "context-read" | "boundary-call"
        );
        // ADR 0488: a settled cleanup's operation span is its callback; the
        // `onSettled` call that takes it runs after the await when its callee
        // does, and with no owner the callback's returned cleanup throws
        // SETTLED_CLEANUP_UNOWNED (rc.17) as anywhere unowned.
        let settled_call = (candidate.operation == "settled-cleanup")
            .then(|| {
                file.ast.calls.iter().find(|call| {
                    call.arguments
                        .first()
                        .is_some_and(|argument| argument.span == candidate.operation_span)
                })
            })
            .flatten();
        // ADR 0493: an operation in function layers that run in place (an
        // IIFE, a helper's callback, a function bound to a name called once;
        // ADR 0469's climb) runs on the stack of the outermost layer's call,
        // so where that call follows an await on every path, so does it.
        // Not through a library callback, which may never run, and not where
        // the function the climb ends in probes `getOwner()` at all: a guard
        // around a layer (`if (getOwner()) run(() => …)`) skips it with no
        // owner, and the push-time guard test follows only IIFEs.
        let climbed_after_await = |site: Span| {
            crate::execution_role::climb_in_place_layers(file, site, lookup).is_some_and(|climb| {
                !climb.exhausted
                    && !climb.in_list
                    && climb.suspended == Some(true)
                    && !crate::execution_role::climb_crosses_library_callback(
                        file, site, climb.at, lookup,
                    )
                    && !crate::owners::owning_function(&file.ast, climb.at).is_some_and(|outer| {
                        file.ast.calls.iter().any(|call| {
                            outer.span.contains(call.span)
                                && lookup.primitive_at_call(file, call.span)
                                    == Some(solid_dialect::Primitive::GetOwner)
                        })
                    })
            })
        };
        // ADR 0488: a settled cleanup's `onSettled` call runs after the await
        // when its callee does, also through in-place layers (ADR 0493).
        let settled_after_await = settled_call.is_some_and(|call| {
            starts.contains(&u64::from(call.callee.start)) || climbed_after_await(call.span)
        });
        // ADR 0612: a `catch` or `finally` runs unsuspended where the `try` head
        // throws before it suspends, under the owner of the function's stack;
        // only a head that cannot (`follows_try_head_await`) proves it past.
        let try_tail_unproven = |site: Span| {
            file.ast
                .try_head_awaits
                .iter()
                .any(|head| head.regions.iter().any(|region| region.contains(site)))
                && !crate::execution_role::follows_try_head_await(file, site, lookup)
        };
        let after_await = settled_after_await
            || (continuation_placed
                && ((starts.contains(&u64::from(candidate.operation_span.start))
                    && !try_tail_unproven(candidate.operation_span))
                    || climbed_after_await(candidate.operation_span)));
        // ADR 0516: a settled cleanup's `onSettled` call that may follow an
        // await (a later loop iteration, a skippable await) may run with no
        // owner: its possible continuation is the call's, not the callback's.
        let possible_site = if continuation_placed {
            Some(candidate.operation_span)
        } else {
            settled_call.map(|call| call.span)
        };
        // An operation of an action's stepped generator after a `yield` of a
        // promise runs from that promise's continuation, with no owner, as
        // one after an `await` does; after a `yield` of a possible thenable
        // it may (`action_step_resumption`). A generator that never runs
        // (context zero) resumes nothing.
        let step_resumption = possible_site
            .filter(|_| !after_await && context != 0)
            .and_then(|site| action_step_resumption(file, site, lookup));
        let after_yield = step_resumption == Some(StepResumption::Proven);
        let after_await = after_await || after_yield;
        let possible_continuation = possible_site.filter(|_| !after_await).and_then(|site| {
            possible_await_continuation_context(file, site, context).or_else(|| {
                (step_resumption == Some(StepResumption::Possible))
                    .then(|| owner_edge_context(OwnerEdgeKind::MayResumeUnowned, context))
            })
        });
        let context = if after_await {
            arm = if after_yield {
                arm_id!("owner_emission.after-yield")
            } else {
                arm_id!("owner_emission.after-await")
            };
            await_continuation_context(context)
        } else {
            if possible_continuation.is_some() {
                arm = arm_id!("owner_emission.possible-continuation");
            }
            possible_continuation.unwrap_or(context)
        };
        if context == 0 {
            // No owner edge reaches the function: its body never runs from
            // syntax, so nothing is reported (the saturation report's class 1).
            arm = arm_id!(default "owner_emission.context-0");
        }
        // A read of a provider-free context under an owner throws
        // `ContextNotFoundError` (ADR 0352).
        // `Some(None)`: the argument names no binding exactly (ADR 0713).
        let context_key = (candidate.operation == "context-read")
            .then_some(candidate.context_argument)
            .flatten()
            .map(|argument| context_key_at(lookup, file, argument));
        let verdict = context_key.as_ref().and_then(|key| {
            let key = key.as_ref()?;
            let verdict = inputs.provider_free.get(key).copied();
            emission.provider.push((key.clone(), verdict));
            verdict
        });
        let provider_verdict = match verdict {
            Some(ContextVerdict::Free(proven)) => Some(proven),
            _ => None,
        };
        let provider_free_read = provider_verdict.is_some();
        // ADR 0713: a read of a context a provider may supply somewhere, run
        // under an owner, throws `ContextNotFoundError` unless a provider is
        // above it: proven only by every render of its function sitting
        // under one (`provided_above`), else an obligation.
        // A key the census does not know (an alias, a parameter, an
        // unresolved import) is open too: nothing proves a default.
        let open_key = match &context_key {
            Some(None) => Some(None),
            Some(Some(key)) if matches!(verdict, None | Some(ContextVerdict::Open)) => {
                Some(Some(key))
            }
            _ => None,
        };
        if let Some(key) = open_key
            && !after_await
            && possible_continuation.is_none()
            && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF_OWNED) != 0
            && !key.is_some_and(|key| {
                // A `let` may hold another context at the provider than at
                // the read: the binding's identity proves nothing then.
                let constant = candidate.context_argument.is_some_and(|argument| {
                    lookup
                        .binding_at_reference(
                            file.path.as_str(),
                            file.ast.peel_ts_sugar_span(argument),
                        )
                        .is_some_and(|(_, binding, _)| binding.immutable)
                });
                constant
                    && containing_ast_function(&file.ast, candidate.operation_span)
                        .is_some_and(|reading| provided_above(lookup, file, reading, key))
            })
        {
            let here = location(file.path.shared(), candidate.operation_span);
            if !emission
                .static_violations
                .iter()
                .any(|known| known.location == here)
            {
                emission.static_violations.push(crate::StaticViolation {
                    id: "SC4002".into(),
                    rule: "missing-context-provider".into(),
                    // ADR 0792: true of every open key. An open context may
                    // have a default the syntax cannot rule out, or be only
                    // exported or handed on, and an unknown key names none.
                    message: "this read runs under an owner, and nothing proves a default for \
                              its context or a provider above every render of the function \
                              reading it; Solid throws `ContextNotFoundError` where a context \
                              with no default has no provider above"
                        .into(),
                    hint: "Render a provider above every use of this component, or give the \
                           context a default."
                        .into(),
                    location: here,
                    analysis_context: crate::blockers::PROVIDER_NOT_PROVEN_ABOVE.into(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
        }
        // ADR 0575: a default the syntax does not prove is not `undefined`.
        let default_unproven = provider_verdict == Some(false);
        // ADR 0612: a region's owner is the synchronous run's: past an await
        // of its compute (`createMemo(async () => { await p; createEffect(…) })`)
        // none is current, whatever the region provides.
        let root_owned = !after_await
            && possible_continuation.is_none()
            && !candidate.in_parameter_list
            && !returned_unplaced
            && region_answers;
        // A providing region owns its reads, so a provider-free read there is
        // a provider finding, not an owner question (ADR 0352).
        // ADR 0866 amendment: a read under `if (getOwner())` in the same
        // invocation runs only with an owner, past any suspension or pass.
        let probe_owned = provider_free_read
            && crate::owners::guarded_by_owner_probe(file, candidate.operation_span, lookup);
        let provider =
            provider_free_read && (root_owned || owned_on_every_path(context) || probe_owned);
        // ADR 0577: a provider-free read whose function runs under an owner
        // on some path and with none on another throws `ContextNotFoundError`
        // on every owned run. That is SC4002's claim; SC4001's unowned run is
        // not proven to be reached past it (a body call that throws before the
        // component's JSX, and so its handlers, exists).
        let owned_run_throws = provider_free_read
            && !provider
            && !after_await
            && possible_continuation.is_none()
            && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF_OWNED) != 0;
        if root_owned && !provider {
            // ADR 0868: the region answers for the run its chain proves, not
            // for a run through a named layer's other entries.
            let entry_open = named_layer_entry_open(
                file,
                candidate.operation_span,
                candidate.providing_region,
                lookup,
                caller_unplaced,
            );
            crate::role_log::owner(
                candidate.operation,
                path,
                candidate.operation_span,
                if entry_open {
                    arm_id!("owner_emission.root-owned-entry-open")
                } else {
                    arm_id!("owner_emission.root-owned")
                },
                context,
                !entry_open,
            );
            if entry_open {
                push_owner_requirement(
                    &mut emission.requirements,
                    &mut seen,
                    candidate.operation,
                    file,
                    candidate.operation_span,
                    OwnerRequirementStatus {
                        uncertain: true,
                        runtime_uncertain: false,
                        caller_uncertain: true,
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
                        report: true,
                    },
                    lookup,
                );
            }
            continue;
        }
        if provider {
            arm = arm_id!("owner_emission.provider-free-read");
        }
        let (operation, report_hit) = if provider {
            ("context-provider", true)
        } else {
            (candidate.operation, context & candidate.report_mask != 0)
        };
        if let Some(gate) = candidate.settled_gate {
            emission.gates.push((
                (path.to_owned(), u64::from(gate.start), u64::from(gate.end)),
                leaf_gate_decision(owner),
            ));
        }
        let mut cleanup_proof = None;
        if candidate.operation == "settled-cleanup" {
            let target = candidate
                .settled_target
                .as_ref()
                .and_then(|target| graph.resolve_target(pid, target));
            cleanup_proof = target.and_then(|target| {
                let node = graph.node(target)?;
                let callback_file = inputs.indexes.files_by_path.get(node.path.as_str())?;
                lookup.note_file_read(callback_file);
                let callback = callback_file.ast.function_at(node.span)?;
                Some(function_cleanup_return_proof(
                    lookup,
                    callback_file,
                    callback,
                ))
            });
            if let Some(symbolic) = &candidate.settled_target {
                emission.callbacks.push((
                    symbolic.clone(),
                    target,
                    inputs.callback_hash(graph, target),
                ));
            }
            // ADR 0867: only a call tsc rejects has nothing to place. A call
            // with no resolved fact (an `any` callee, a held value) still runs.
            let settled_call_rejected = file.ast.calls.iter().any(|call| {
                call.arguments
                    .first()
                    .is_some_and(|argument| argument.span == candidate.operation_span)
                    && lookup
                        .resolved_callee_call(file, call.callee)
                        .is_some_and(|resolved| {
                            resolved.validity == typefacts::ResolvedCallValidity::Recovery
                        })
            });
            let cleanup_skipped = |arm| {
                crate::role_log::owner(
                    candidate.operation,
                    path,
                    candidate.operation_span,
                    arm,
                    context,
                    true,
                );
            };
            match cleanup_proof.unwrap_or(CleanupReturnProof::Unresolved) {
                CleanupReturnProof::NoFunction => {
                    cleanup_skipped(arm_id!("owner_emission.cleanup-no-function"));
                    continue;
                }
                CleanupReturnProof::Function => {}
                CleanupReturnProof::OptionalFunction => {
                    // The callback may return a cleanup. Preserve that as
                    // runtime uncertainty instead of certifying it absent.
                }
                CleanupReturnProof::Unresolved if !settled_call_rejected => {}
                CleanupReturnProof::Unresolved => {
                    cleanup_skipped(arm_id!("owner_emission.cleanup-unresolved-call"));
                    continue;
                }
            }
        }
        let proven_unowned = context & OWNER_CONTEXT_PROVEN_UNOWNED != 0;
        let component_uncertain = !proven_unowned
            && (context & OWNER_CONTEXT_COMPONENT_UNCERTAIN != 0
                || owner.is_some_and(|(node, _)| node.component_uncertain));
        let later_run_unowned = !component_uncertain
            && !proven_unowned
            && context & OWNER_CONTEXT_LATER_RUN_UNOWNED != 0;
        let conditional_owner = !component_uncertain
            && !proven_unowned
            && !later_run_unowned
            && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF_OWNED) != 0
            && context & OWNER_CONTEXT_UNOWNED != 0;
        // ADR 0687: a `runWithOwner` callback has a nullable owner to name;
        // anywhere else the callers differ.
        let owner_varies_by_caller = conditional_owner
            && !file
                .ast
                .arguments_containing(candidate.operation_span)
                .any(|(call, index)| {
                    inputs
                        .lookup
                        .primitive_at_call(file, call.span)
                        .is_some_and(|primitive| {
                            inputs
                                .lookup
                                .dialect
                                .callback_semantics_at(primitive, index, call.arguments.len())
                                .owner
                                == Some(solid_dialect::CallbackOwner::Conditional)
                        })
                });
        let caller_uncertain = !proven_unowned
            && !unplaced_callback
            && (exported_seed
                || owner.is_some_and(|(node, _)| open_world_unowned_only(context, node)));
        let cleanup_return_uncertain = candidate.operation == "settled-cleanup"
            && cleanup_proof.is_none_or(|proof| {
                matches!(
                    proof,
                    CleanupReturnProof::OptionalFunction | CleanupReturnProof::Unresolved
                )
            });
        let runtime_uncertain = candidate.runtime_uncertain
            || cleanup_return_uncertain
            || candidate.in_parameter_list
            || returned_unplaced;
        // ADR 0517: in an open program a provider outside the analyzed code
        // may supply the context; the census proves only what it sees.
        let provider_open = provider && !inputs.lookup.program_closed;
        let default_unproven = provider && default_unproven;
        let unplaced_callback = unplaced_callback && !proven_unowned;
        let uncertain = runtime_uncertain
            || conditional_owner
            || later_run_unowned
            || caller_uncertain
            || component_uncertain
            || unplaced_callback
            || provider_open
            || default_unproven
            || owned_run_throws;
        if owned_run_throws {
            // ADR 0577: the owned runs' claim, beside the owner obligation.
            let provider_open = !inputs.lookup.program_closed;
            let default_unproven = provider_verdict == Some(false);
            let component_uncertain = context & OWNER_CONTEXT_COMPONENT_UNCERTAIN != 0
                || owner.is_some_and(|(node, _)| node.component_uncertain);
            let uncertain =
                runtime_uncertain || component_uncertain || provider_open || default_unproven;
            crate::role_log::owner(
                "context-provider",
                path,
                candidate.operation_span,
                arm_id!("owner_emission.owned-run-throws"),
                context,
                false,
            );
            push_owner_requirement(
                &mut emission.requirements,
                &mut seen,
                "context-provider",
                file,
                candidate.operation_span,
                OwnerRequirementStatus {
                    uncertain,
                    runtime_uncertain,
                    caller_uncertain: false,
                    unplaced_callback: false,
                    conditional_owner: false,
                    owner_varies_by_caller: false,
                    later_run_unowned: false,
                    component_uncertain,
                    provider_open,
                    default_unproven,
                    owned_run_throws: false,
                    after_await: false,
                    after_yield: false,
                    report: true,
                },
                lookup,
            );
        }
        // A cleanup in a leaf that is not owned is reported; in an owned leaf
        // it is SC3001's claim (ADR 0347 amendment).
        let report = report_hit
            || (candidate.operation == "cleanup"
                && context & OWNER_CONTEXT_LEAF != 0
                && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF_OWNED) == 0);
        // ADR 0868: an owned context is the edges the graph holds. A named
        // layer entered some other way (assigned, stored, handed on, a
        // method) may run with no owner, and that run has no edge.
        let entry_open = !report
            && !uncertain
            && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF_OWNED) != 0
            && named_layer_entry_open(
                file,
                candidate.operation_span,
                candidate.providing_region,
                lookup,
                caller_unplaced,
            );
        if entry_open {
            arm = arm_id!("owner_emission.named-layer-entry-open");
        }
        let caller_uncertain = caller_uncertain || entry_open;
        let uncertain = uncertain || entry_open;
        // An obligation is a reported requirement, as every other one.
        let report = report || entry_open;
        crate::role_log::owner(
            operation,
            path,
            candidate.operation_span,
            arm,
            context,
            !report && !uncertain,
        );
        push_owner_requirement(
            &mut emission.requirements,
            &mut seen,
            operation,
            file,
            candidate.operation_span,
            OwnerRequirementStatus {
                uncertain,
                runtime_uncertain,
                caller_uncertain,
                unplaced_callback,
                conditional_owner,
                owner_varies_by_caller,
                later_run_unowned,
                component_uncertain,
                provider_open,
                default_unproven,
                owned_run_throws,
                after_await: after_await && !after_yield,
                after_yield,
                report,
            },
            lookup,
        );
    }
    emission
}

/// Whether the function at `span`, or a function around it that also holds
/// context zero and so runs it only from where it runs, is the value its
/// enclosing function returns (a `return` argument or a concise arrow body).
fn returned_unreached_function(
    graph: &OwnerGraph,
    pid: PathId,
    file: &solid_facts::FileFacts,
    span: Span,
) -> bool {
    let returned = |function: &solid_facts::ast::FunctionFact| {
        let target = function.span;
        file.ast
            .returns
            .iter()
            .filter_map(|returned| returned.argument)
            .chain(
                file.ast
                    .functions
                    .iter()
                    .filter_map(|outer| outer.expression_return.as_ref()?.argument),
            )
            .any(|argument| file.ast.peel_ts_sugar_span(argument) == target)
    };
    let mut at = span;
    // Bounded: each step moves to the function that holds the previous one.
    for _ in 0..16 {
        let Some(function) = file.ast.function_at(at) else {
            return false;
        };
        if returned(function) {
            return true;
        }
        let Some(outer) = crate::owners::owning_function(&file.ast, function.span) else {
            return false;
        };
        if graph.context(pid, outer.span) != Some(0) {
            return false;
        }
        at = outer.span;
    }
    // ADR 0532: the functions past the bound are unread; context zero there
    // is no proof, so the operation stays unplaced.
    true
}

/// ADR 0868: whether a function between `span` and the component around it
/// may be entered by a way the owner graph holds no edge for, so that the
/// owned context its edges give is no proof of every run:
///
/// - a method, or a function value no binding names (an object slot, an array
///   element, a class field), unless it is invoked in place or is a whole
///   argument of a call (that call gives it its edge, or leaves it unplaced);
/// - a named function whose entry census is open
///   ([`crate::execution_role::entry_census_complete`]), or a reference to
///   which, here or in another file, is anything but a direct call or an
///   inline run. A value-only argument counts too: the callee may keep it
///   (`keep.call(f)`, an `object` parameter, an `EventTarget` subclass's
///   `removeEventListener`);
/// - any layer at all when the program calls direct `eval`, which reaches a
///   binding by name.
///
/// The walk ends at a function a tag is proven to render, whose entries are
/// renders, and at the innermost owner-creating region around `span`. A function that only may be a component is a layer like any other.
/// `caller_unplaced` answers whether the function making a run at a span holds
/// no owner context and may run (the emission's ADR 0534 question).
fn named_layer_entry_open(
    file: &solid_facts::FileFacts,
    span: Span,
    region: Option<Span>,
    lookup: &SemanticLookup<'_>,
    caller_unplaced: impl Fn(Span) -> bool,
) -> bool {
    let mut walked = HashSet::default();
    layer_entry_open(
        file,
        span,
        region,
        lookup,
        &caller_unplaced,
        &mut walked,
        LAYER_CALLER_DEPTH,
    )
}

/// How many calls [`layer_entry_open`] follows out through a closed layer's
/// callers. Past it the entries are unread: open.
const LAYER_CALLER_DEPTH: usize = 8;

/// Whether `function` is the whole argument the dialect gives an owner it
/// creates (`CallbackOwner::Creates`): who enters what is around that call
/// changes nothing inside.
fn creates_owner_for(
    file: &solid_facts::FileFacts,
    function: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast
        .arguments_containing(function)
        .any(|(call, index)| {
            !call.arguments[index].spread
                && file.ast.peel_ts_sugar_span(call.arguments[index].span) == function
                && lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| {
                        lookup
                            .dialect
                            .callback_owner_at(primitive, index, call.arguments.len())
                            == Some(solid_dialect::CallbackOwner::Creates)
                    })
        })
}

fn layer_entry_open(
    file: &solid_facts::FileFacts,
    span: Span,
    region: Option<Span>,
    lookup: &SemanticLookup<'_>,
    caller_unplaced: &dyn Fn(Span) -> bool,
    walked: &mut HashSet<(String, Span)>,
    depth: usize,
) -> bool {
    use crate::local_access::HandOff;
    let escapes = |hand_off: &HandOff| !matches!(hand_off, HandOff::Call | HandOff::Inline(_));
    // Innermost first: the span index yields its owners in span order.
    let mut layers = file.ast.functions_owning(span).collect::<Vec<_>>();
    layers.sort_by_key(|function| function.span.end - function.span.start);
    for function in layers {
        // The innermost region whose primitive creates the owner its
        // callback runs under (`CallbackOwner::Creates`: a memo, a root, an
        // effect's compute, a tracked JSX expression): who enters a layer
        // around it changes nothing inside.
        if region.is_some_and(|region| !region.contains(function.span)) {
            return false;
        }
        if lookup.function_proven_component(file, function) {
            return false;
        }
        // Reached through a caller, no candidate names the region.
        if creates_owner_for(file, function.span, lookup) {
            return false;
        }
        if lookup.program_has_eval() || function.method_name.is_some() {
            return true;
        }
        if function_binding_name(file, function).is_none() {
            let in_place = file
                .ast
                .calls_with_peeled_callee(function.span)
                .next()
                .is_some();
            let argument = file.ast.calls.iter().any(|call| {
                call.arguments.iter().any(|argument| {
                    !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == function.span
                })
            });
            if in_place || argument {
                continue;
            }
            return true;
        }
        let Some(local) = crate::local_access::function_reference_hand_offs(file, function, lookup)
        else {
            return true;
        };
        // A whole argument the dialect gives an owner is a position the owner
        // graph places (`createRoot(helper)`): its edge is in the context.
        // `action(gen)` keeps its argument and `createContext(f)` holds a
        // value: no edge follows either (R29).
        let primitive_position = |at: Span| {
            file.ast.arguments_containing(at).any(|(call, index)| {
                !call.arguments[index].spread
                    && file.ast.peel_ts_sugar_span(call.arguments[index].span) == at
                    && lookup
                        .primitive_at_call(file, call.span)
                        .is_some_and(|primitive| {
                            lookup
                                .dialect
                                .callback_owner_at(primitive, index, call.arguments.len())
                                .is_some()
                        })
            })
        };
        if local
            .iter()
            .any(|(at, hand_off)| escapes(hand_off) && !primitive_position(*at))
            || crate::local_access::foreign_reference_hand_offs(file, function, lookup)
                .iter()
                .any(|(_, _, hand_off)| escapes(hand_off))
        {
            return true;
        }
        // A call or an inline run is an entry only as placed as the code
        // making it: from a function no edge reaches that may still run
        // (assigned to `onclick`, say), the run has no owner edge either.
        let run_at = |(at, hand_off): &(Span, HandOff)| match hand_off {
            HandOff::Inline(carrier) => *carrier,
            _ => *at,
        };
        if local
            .iter()
            .any(|reference| caller_unplaced(run_at(reference)))
        {
            return true;
        }
        let inline = local
            .iter()
            .filter(|(_, hand_off)| matches!(hand_off, HandOff::Inline(_)))
            .map(|(at, _)| *at)
            .collect::<Vec<_>>();
        if !crate::execution_role::entry_census_complete_except(file, function, lookup, &inline) {
            return true;
        }
        // A closed layer is entered only by its callers, and each is entered
        // as it is: `wrapUse` called from an `f` that escapes runs with no
        // owner on `f`'s other entries too.
        if !walked.insert((file.path.as_str().to_owned(), function.span)) {
            continue;
        }
        if depth == 0 {
            return true;
        }
        for reference in &local {
            let run = run_at(reference);
            if layer_entry_open(file, run, None, lookup, caller_unplaced, walked, depth - 1) {
                return true;
            }
        }
        for (caller, callee) in
            lookup.function_call_site_references(file.path.as_str(), function.span)
        {
            if caller.path != file.path
                && layer_entry_open(caller, callee, None, lookup, &|_| false, walked, depth - 1)
            {
                return true;
            }
        }
    }
    false
}
