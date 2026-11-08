//! Execution roles of writes: the regions a write is allowed in and how a leaf scope adjusts them.

use super::*;

/// Classifies a write or action through resolved project call sites when its
/// own source span has no compiler execution fact.
///
/// Any proven tracking-phase invocation makes the operation unsafe. Otherwise
/// a proven imperative role is retained, while cycles with no independently
/// classified call site remain `Unknown`.
///
/// `root_body` says what the operation's guard does directly under a root
/// owner; see [`RootBodyGuard`].
pub(crate) fn semantic_write_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
) -> ExecutionRole {
    crate::role_log::decide("write", file.path.as_str(), span, || {
        let allowed = write_allowed_spans(file, allowed, lookup);
        super::write_components::bounded_walk(lookup, || {
            semantic_write_execution_role_within(
                file,
                span,
                &allowed,
                entities,
                symbol_names,
                lookup,
                root_body,
                &mut HashSet::default(),
            )
        })
    })
}

/// The regions a write may treat as imperative.
///
/// [`allowed_callback_spans`] lists every callback whose *reads* run outside
/// the surrounding tracking pass, and `createRoot`'s body is one: it clears
/// the listener. For a write that is the wrong question. The body runs under
/// the root, a children-capable owner, and containment in an allowed region
/// is tested before a nested memo or effect compute is recognized, so every
/// write anywhere inside a root used to be classified as legal. The root's own
/// body is decided by [`WriteRegionAdjustment::CreatedRoot`] instead, and what
/// is nested in it by its own callback.
pub(super) fn write_allowed_spans(
    file: &solid_facts::FileFacts,
    allowed: &[Span],
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    let roots = file
        .ast
        .calls
        .iter()
        .filter_map(|call| {
            let primitive = lookup.primitive_at_call(file, call.span)?;
            Some(
                call.arguments
                    .iter()
                    .enumerate()
                    .filter(move |(index, _)| {
                        lookup
                            .dialect
                            .callback_runs_in_created_root(primitive, *index)
                    })
                    .map(|(_, argument)| argument.span),
            )
        })
        .flatten()
        .collect::<HashSet<_>>();
    allowed
        .iter()
        .copied()
        .filter(|span| !roots.contains(span))
        .collect()
}

/// How the innermost containing callback changes a write's legality region,
/// when it does. Never consulted for reads: reads keep their own classifier.
pub(super) enum WriteRegionAdjustment {
    /// The write sits in a children-forbidden leaf callback
    /// (`createTrackedEffect`, `onSettled`), which the runtime's write guard
    /// exempts — the write is legal regardless of what surrounds the leaf.
    LeafScope,
    /// The write sits in an owner-transparent inline callback (`untrack`),
    /// which clears tracking but keeps the caller's owner context; the write
    /// is exactly as legal as at the wrapped call itself, so classify there.
    CallSite(Span),
    /// [`WriteRegionAdjustment::CallSite`] for a function with runs the climb
    /// does not follow (a helper also hands it on, a primitive names it by a
    /// binding that has other entries; ADR 0457, 0466): the run at the call
    /// proves a rejection there, and no legal write.
    CallSiteAmongOthers(Span),
    /// The write sits in a callback that shares its call site's owner only on
    /// its first run ([`solid_dialect::CallbackOwner::InheritsFirstRun`], 2.0
    /// `createRenderEffect`'s apply). Later runs come from the flush, where
    /// writes are legal; the first run is as legal as the call site.
    FirstRunAtCallSite(Span),
    /// The write sits directly in a callback that runs under a root owner the
    /// call creates whenever it runs (`createRoot`'s body, rc.17 `hydrate`'s
    /// possibly later `code`). The ambient owner is that root wherever the
    /// call is, so neither the call site nor the timing matters: the
    /// operation's [`RootBodyGuard`] decides, short of a suspension before
    /// the write.
    CreatedRoot,
    /// ADR 0551: the write sits directly in a callback that runs as the
    /// compute of a computation the call creates
    /// ([`solid_dialect::Dialect::callback_runs_as_created_computation`]):
    /// rejected as in a memo's compute, short of a suspension before it.
    CreatedComputation,
    /// [`WriteRegionAdjustment::CreatedRoot`] for a function a helper runs in
    /// place and also hands on or keeps (ADR 0457): one run is under the root,
    /// which proves a rejection and no legal write.
    CreatedRootAmongOthers,
    /// The write sits directly in an inline callback whose call may create a
    /// root this vocabulary has not audited
    /// ([`solid_dialect::Dialect::callback_root_is_unaudited`]). Whether it runs
    /// under a root is unknown, so the write is unplaced.
    UnauditedRoot,
    /// The write runs in a callback the call runs with no owner at all
    /// ([`solid_dialect::CallbackOwner::None`]: `runWithOwner(null, fn)`), or in
    /// a function that callback runs in place (an IIFE, an inline
    /// standard-library callback) or hands to a call that keeps its owner
    /// (`untrack`). The guard keys on the owner, and there is none, so the write
    /// is legal wherever the call is written (ADR 0455).
    Detached,
    /// The write runs in a function a `runWithOwner` callback runs in place,
    /// and the call's owner is data the call site does not settle
    /// ([`solid_dialect::CallbackOwner::Conditional`]): null detaches it, an
    /// owner does not. Neither is proven, so the write is unplaced (ADR 0455).
    OwnerUnknown,
    /// ADR 0532: the climb from the write reached its bound without ending at
    /// a callback or at a layer it could place. A layer past the bound may be
    /// an owner-keeping callback or a created root, so neither a placement
    /// nor the lexical role is proven: the write is unplaced.
    ClimbExhausted,
    /// ADR 0613: the write runs in a callback the queue's drain runs
    /// ([`solid_dialect::Dialect::callback_runs_drained_by_flush`]: a
    /// `createEffect` apply), or in a function it runs in place, while a
    /// `flush()` of the program may drain the queue under an owner
    /// ([`SemanticLookup::flush_drains_under_owner`]). A scheduled drain runs
    /// it with no owner (legal), a synchronous one under its caller's owner
    /// (rejected): unplaced, whatever the function's other callers.
    DrainedByFlush,
}

/// The write-legality adjustment for the innermost callback directly
/// containing `span`, per the dialect's owner-context model.
///
/// Direct containment (the span's own function is the callback function)
/// guarantees at most one candidate: a nested callback's contents are that
/// callback's business on the next recursion step.
pub(super) fn write_region_adjustment(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<WriteRegionAdjustment> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            if !direct_callback_contains(file, call.arguments[index].span, span) {
                return None;
            }
            let primitive = lookup.primitive_at_call(file, call.span)?;
            // ADR 0613: `onSettled`'s queued run is the drain's, whichever owner
            // its call has: the leaf exemption below is no proof then.
            if lookup
                .dialect
                .leaf_owner_requires_owned_call_site(primitive)
                && drained_under_owner(primitive, index, lookup)
            {
                return Some(WriteRegionAdjustment::DrainedByFlush);
            }
            if lookup.dialect.leaf_scopes_allow_writes()
                && callback_owner_at_call(file, call, primitive, index, lookup)
                    == Some(solid_dialect::CallbackOwner::Leaf)
            {
                return Some(WriteRegionAdjustment::LeafScope);
            }
            // The adjustments below place the callback's own body. A site in
            // the parameter list of a function the callback builds
            // (`createRoot(() => ({ f: (x = set(1)) => {} }))`) lies lexically
            // in the callback, but runs when that function is called without
            // the argument, wherever that is (ADR 0455, 0454): no adjustment
            // places it, and the write guard leaves it unplaced.
            if !direct_callback_runs(file, call.arguments[index].span, span) {
                return None;
            }
            if lookup
                .dialect
                .callback_runs_as_created_computation(primitive, index)
            {
                return Some(WriteRegionAdjustment::CreatedComputation);
            }
            // The root is the owner whenever the callback runs, so its
            // timing is not asked (rc.17 `hydrate` states none).
            if lookup
                .dialect
                .callback_runs_in_created_root(primitive, index)
            {
                return Some(WriteRegionAdjustment::CreatedRoot);
            }
            if lookup.dialect.callback_root_is_unaudited(primitive, index)
                && callback_execution_at_call(file, call, primitive, index, lookup)
                    == Some(Execution::Inline)
            {
                return Some(WriteRegionAdjustment::UnauditedRoot);
            }
            if lookup
                .dialect
                .callback_preserves_owner_write_context(primitive)
                && callback_execution_at_call(file, call, primitive, index, lookup).is_some()
            {
                return Some(WriteRegionAdjustment::CallSite(call.span));
            }
            match callback_owner_at_call(file, call, primitive, index, lookup) {
                Some(solid_dialect::CallbackOwner::InheritsFirstRun) => {
                    return Some(WriteRegionAdjustment::FirstRunAtCallSite(call.span));
                }
                // ADR 0613: the apply's "no owner" is the drain's, which a
                // synchronous flush runs under its caller's owner.
                Some(solid_dialect::CallbackOwner::None)
                    if drained_under_owner(primitive, index, lookup) =>
                {
                    return Some(WriteRegionAdjustment::DrainedByFlush);
                }
                // ADR 0502: a render-effect apply whose first run a `schedule`
                // (or unreadable options) may keep on the call: no placement.
                Some(solid_dialect::CallbackOwner::Conditional)
                    if lookup
                        .dialect
                        .effect_apply_schedule_options(primitive, index)
                        .is_some() =>
                {
                    return Some(WriteRegionAdjustment::OwnerUnknown);
                }
                _ => {}
            }
            None
        })
        .or_else(|| drained_returned_cleanup(file, span, lookup))
        .or_else(|| drained_bundle_member(file, span, lookup))
        .or_else(|| detached_in_place(file, span, lookup))
        .or_else(|| nested_in_leaf_scope(file, span, lookup))
}

/// ADR 0613: the cleanup an effect's apply returns runs on a rerun from
/// `runEffect`, in the queue's drain, so its "no owner" is the drain's too
/// (rc.17 `dist/dev.js:2003`, probed: under a `flush()` in a root body a
/// setter there throws `REACTIVE_WRITE_IN_OWNED_SCOPE`).
fn drained_returned_cleanup(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<WriteRegionAdjustment> {
    (inside_rerun_effect_cleanup(file, span, lookup) && lookup.flush_drains_under_owner())
        .then_some(WriteRegionAdjustment::DrainedByFlush)
}

/// ADR 0613: whether the ownerless answer of the callback at `index` of
/// `primitive` is the queue's drain's and some `flush()` of the program may
/// drain under an owner. The veto is read only for such a callback.
fn drained_under_owner(primitive: Primitive, index: usize, lookup: &SemanticLookup<'_>) -> bool {
    lookup
        .dialect
        .callback_runs_drained_by_flush(primitive, index)
        && lookup.flush_drains_under_owner()
}

/// ADR 0613: a write in the `error` or `effect` member of an exact effect
/// bundle, or in a function that member runs in place. The member is no
/// argument the in-place climb stops at, and both run from the queue's drain
/// (`runEffect`, ADR 0548): probed, under a `flush()` in a root body a setter
/// in an `error` arm throws `REACTIVE_WRITE_IN_OWNED_SCOPE`.
fn drained_bundle_member(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<WriteRegionAdjustment> {
    let at = in_place_run_site(file, span, lookup).map_or(span, |(climb, _)| climb.at);
    let function = crate::owners::containing_ast_function(&file.ast, at)?;
    let is_member = |member: Option<Span>| {
        member.is_some_and(|member| file.ast.peel_ts_sugar_span(member) == function.span)
    };
    let held = file
        .ast
        .arguments_containing(function.span)
        .any(|(call, index)| {
            let whole = file.ast.peel_ts_sugar_span(call.arguments[index].span);
            whole != function.span
                && lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| {
                        lookup
                            .dialect
                            .callback_runs_drained_by_flush(primitive, index)
                            && (is_member(super::bundle_error_member(
                                lookup.dialect,
                                primitive,
                                call,
                                index,
                            )) || is_member(super::effect_apply_span(call, index)))
                    })
        });
    (held && lookup.flush_drains_under_owner()).then_some(WriteRegionAdjustment::DrainedByFlush)
}

/// What the owner of the callback that runs `span` says about the write, when
/// the callback is one its call runs during the call, with the call's window
/// ([`solid_dialect::Dialect::callback_inherits_calling_window`]: `runWithOwner`)
/// and `span` is written in it, in a function it runs in place, or in one it
/// hands to a call that keeps the owner and runs it during the call (`untrack`).
/// Every layer between must run on the callback's stack ([`layer_run`]: an IIFE,
/// an inline standard-library callback), so the owner the call gives reaches the
/// write; a layer that stores the function, hands it to an unknown call or runs
/// it on a fresh stack ends the climb.
///
/// - owner `None` (`runWithOwner(null, fn)`): [`WriteRegionAdjustment::Detached`];
/// - owner `Creates` (`createOwner()`, a component's `getOwner()`): the owner is
///   a root whenever the callback runs, as in a created root
///   ([`WriteRegionAdjustment::CreatedRoot`]);
/// - owner `Conditional` (may be null, or not proven non-null):
///   [`WriteRegionAdjustment::OwnerUnknown`].
///
/// When the climb ends at no such callback after crossing a layer and then a
/// call that keeps the owner, the write is as legal as at that call
/// ([`WriteRegionAdjustment::CallSite`]), as it is directly in the callback.
fn detached_in_place(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<WriteRegionAdjustment> {
    let mut at = span;
    let mut layered = false;
    // Whether a layer on the way holds the site in its parameter list: it runs
    // only when its call omits the argument, which nothing here proves.
    let mut in_list = false;
    // The innermost owner-keeping call, when a layer lies between it and `span`.
    let mut first_hop = None;
    // Whether a layer on the way is a helper that also hands its callback on
    // or keeps it ([`LayerRun::InPlaceAmongOthers`]): the callback has runs this
    // climb does not follow, so only a run that proves a violation is read.
    let mut among_others = false;
    // Whether a layer on the way holds the site past an await.
    let mut suspended = false;
    // Bounded: each step moves to the call that holds the previous function.
    // At the bound nothing is proven (ADR 0532): the layers past it are not
    // read, and the lexical role would answer for them.
    let mut steps = 0;
    loop {
        if steps == 16 {
            return Some(WriteRegionAdjustment::ClimbExhausted);
        }
        steps += 1;
        let Some(function) = crate::owners::owning_function(&file.ast, at) else {
            break;
        };
        if function.generator {
            break;
        }
        in_list |= !function.body.contains(at);
        // An await before the site in a layer: the rest of that layer runs
        // later, on a stack the apply's first run is not on (ADR 0460).
        suspended |= follows_dominating_await(file, at)
            || follows_try_head_await(file, at, lookup)
            || may_follow_suspension(file, at);
        let primitive_call = file
            .ast
            .arguments_containing(function.span)
            .filter(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
            })
            .find_map(|(call, index)| {
                lookup
                    .primitive_at_call(file, call.span)
                    .map(|primitive| (call, index, primitive))
            });
        // ADR 0466: a function a primitive call names by its binding runs as an
        // inline callback in that position would. A position that is not the
        // function's only entry is one run among others.
        let mut by_name = false;
        let mut by_name_others = false;
        // An owner-keeping call already crossed is placed at that call, whose own
        // walk climbs on from there.
        let primitive_call = primitive_call.or_else(|| {
            let position =
                by_name_position(file, function, lookup).filter(|_| first_hop.is_none())?;
            by_name = true;
            by_name_others = !position.sole;
            among_others |= by_name_others;
            Some((position.call, position.index, position.primitive))
        });
        if let Some((call, index, primitive)) = primitive_call {
            let execution = callback_execution_at_call(file, call, primitive, index, lookup);
            if execution == Some(Execution::Inline)
                && lookup
                    .dialect
                    .callback_inherits_calling_window(primitive, index)
            {
                match callback_owner_at_call(file, call, primitive, index, lookup) {
                    // A run that proves no rejection leaves a function named by a
                    // binding with other entries to those entries: the position
                    // is not one the entry census counts (the named-callback
                    // index admits no `runWithOwner` callback), so the walk
                    // proves a rejection at another entry or nothing.
                    Some(
                        solid_dialect::CallbackOwner::None
                        | solid_dialect::CallbackOwner::Conditional,
                    ) if by_name_others => return None,
                    // Every run is legal with no owner; another run of a callback a
                    // helper also hands on may be under one.
                    Some(solid_dialect::CallbackOwner::None) if among_others => {
                        return Some(WriteRegionAdjustment::OwnerUnknown);
                    }
                    Some(solid_dialect::CallbackOwner::None) => {
                        return Some(WriteRegionAdjustment::Detached);
                    }
                    // A site in a list is no proof the write runs under the root.
                    Some(solid_dialect::CallbackOwner::Creates) if !in_list && among_others => {
                        return Some(WriteRegionAdjustment::CreatedRootAmongOthers);
                    }
                    Some(solid_dialect::CallbackOwner::Creates) if !in_list => {
                        return Some(WriteRegionAdjustment::CreatedRoot);
                    }
                    Some(solid_dialect::CallbackOwner::Creates) => return None,
                    Some(solid_dialect::CallbackOwner::Conditional) => {
                        return Some(WriteRegionAdjustment::OwnerUnknown);
                    }
                    _ => {}
                }
            }
            // ADR 0540: a callback its call runs with no owner whenever it
            // runs (`createReaction`'s, run as its effect's apply from the
            // flush; `createEffect`'s apply): the guard has no owner to key
            // on, there or in a layer run in place on its stack.
            if execution.is_some()
                && !lookup
                    .dialect
                    .callback_inherits_calling_window(primitive, index)
                && callback_owner_at_call(file, call, primitive, index, lookup)
                    == Some(solid_dialect::CallbackOwner::None)
            {
                if by_name_others {
                    return None;
                }
                if among_others {
                    return Some(WriteRegionAdjustment::OwnerUnknown);
                }
                // ADR 0613: a synchronous flush may drain it under an owner.
                if drained_under_owner(primitive, index, lookup) {
                    return Some(WriteRegionAdjustment::DrainedByFlush);
                }
                return Some(WriteRegionAdjustment::Detached);
            }
            // ADR 0551: the layers run on the stack of a callback that runs
            // as a created computation's compute (`resolve`'s thunk): the
            // computation is the owner there too.
            if (layered || by_name)
                && execution.is_some()
                && lookup
                    .dialect
                    .callback_runs_as_created_computation(primitive, index)
            {
                if suspended {
                    return None;
                }
                if in_list || among_others || by_name_others {
                    return Some(WriteRegionAdjustment::OwnerUnknown);
                }
                return Some(WriteRegionAdjustment::CreatedComputation);
            }
            // ADR 0460: the layers run on the stack of a callback that shares
            // its call's owner on its first run (`createRenderEffect`'s apply),
            // so the write is placed as one written directly in it. A site in a
            // list, or a callback a helper also hands on, has runs this climb
            // does not follow, and so no placement.
            if (layered || by_name)
                && callback_owner_at_call(file, call, primitive, index, lookup)
                    == Some(solid_dialect::CallbackOwner::InheritsFirstRun)
            {
                if suspended {
                    return None;
                }
                if in_list || among_others {
                    return Some(WriteRegionAdjustment::OwnerUnknown);
                }
                return Some(WriteRegionAdjustment::FirstRunAtCallSite(call.span));
            }
            if execution.is_some()
                && lookup
                    .dialect
                    .callback_preserves_owner_write_context(primitive)
            {
                // A write directly in the owner-keeping callback is
                // `write_region_adjustment`'s own question, asked at the call,
                // unless the call names the callback.
                if !layered && !by_name {
                    break;
                }
                first_hop.get_or_insert(call.span);
                at = call.span;
                layered = true;
                continue;
            }
            break;
        }
        match layer_run(file, function, lookup) {
            LayerRun::InPlace(call) => at = call,
            // An owner-keeping call already crossed is as legal as it is, and
            // stays so; the helper's other runs bear on nothing past it.
            LayerRun::InPlaceAmongOthers(_) if first_hop.is_some() => break,
            LayerRun::InPlaceAmongOthers(call) => {
                among_others = true;
                at = call;
            }
            LayerRun::FreshStack | LayerRun::Elsewhere | LayerRun::Withheld(_) => break,
        }
        layered = true;
    }
    first_hop.filter(|_| !in_list).map(|call| {
        if among_others {
            WriteRegionAdjustment::CallSiteAmongOthers(call)
        } else {
            WriteRegionAdjustment::CallSite(call)
        }
    })
}

/// ADR 0521: the call that creates the iterator of the generator `span` is
/// written in, when that generator is stepped there and nowhere else
/// (`(function* () { … })().next()`: the iterator is the call expression
/// itself, so no other code holds it). `None` for a site outside a
/// generator's body, in an async generator (its steps run on later tasks), in
/// a generator that is itself a primitive's callback (placed as that
/// callback), or in one whose iterator is stored or never stepped.
pub(super) struct SteppedGenerator {
    /// The call that creates the iterator, where the first step runs.
    pub(super) created: Span,
    /// Whether the one step reaches the site.
    pub(super) first_step: StepReach,
}

/// Whether the one step of a [`SteppedGenerator`] runs the site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StepReach {
    /// The step is a `next`, and no `yield` of the generator's own body may
    /// run before the site (none written before it outside an exclusive arm,
    /// none in a loop that repeats the site): the first step reaches the
    /// site, on the stack of the creating call.
    Reaches,
    /// The site never runs: the only step is a `return` or `throw` on the
    /// fresh generator, which runs none of the body, or a `yield` in the
    /// body's straight-line flow precedes the site on every path
    /// ([`solid_facts::ast::YieldFact::region`]) and no second step exists.
    Never,
    /// A `yield` may precede the site: the first step may stop before it.
    Unproven,
}

/// [`SteppedGenerator`] for `span`, when its own function is such a generator.
pub(super) fn stepped_generator_in_place(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
) -> Option<SteppedGenerator> {
    let function = crate::owners::containing_ast_function(&file.ast, span)?;
    if !function.generator || function.r#async || !function.body.contains(span) {
        return None;
    }
    if allowed.iter().any(|region| {
        file.ast.peel_ts_sugar_span(*region) == function.span || *region == function.span
    }) {
        return None;
    }
    let created = crate::owners::generator_iterator_used(file, function)?;
    if !crate::owners::iterator_stepped_at(file, created.span) {
        return None;
    }
    let own = |point: Span| {
        function.body.contains(point)
            && crate::owners::containing_ast_function(&file.ast, point)
                .is_some_and(|owner| owner.span == function.span)
    };
    let yield_may_precede = file
        .ast
        .yields
        .iter()
        .filter(|fact| own(fact.span))
        .any(|fact| {
            // Written before the site (or inside it), outside an exclusive arm.
            let before = fact.span.start < span.end && !in_exclusive_arms(file, fact.span, span);
            // In a loop whose repeated part holds the site: a later iteration
            // reaches the site after the `yield`.
            let repeated = file.ast.loop_repeats.iter().any(|&(looped, repeated)| {
                repeated.contains(span)
                    && looped.contains(fact.span)
                    && function.body.contains(looped)
            });
            before || repeated
        });
    let yield_dominates = file
        .ast
        .yields
        .iter()
        .filter(|fact| own(fact.span))
        .any(|fact| fact.region.is_some_and(|region| region.contains(span)));
    let first_step =
        if !crate::owners::iterator_next_stepped_at(file, created.span) || yield_dominates {
            StepReach::Never
        } else if yield_may_precede {
            StepReach::Unproven
        } else {
            StepReach::Reaches
        };
    Some(SteppedGenerator {
        created: created.span,
        first_step,
    })
}

/// Whether a write under a proven owner runs in a disposal, for the release
/// whose write guards stand down there ([`runs_in_disposal`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DisposalReach {
    /// Every run is inside a disposal: written in a callback that runs only
    /// while an owner is disposed, on a stack no await has left, or in a
    /// component only such a callback mounts (ADR 0782).
    Only,
    /// Proven to run outside any disposal: a component body, a fresh stack, a
    /// callback the engine runs on its own schedule, an effect's returned
    /// cleanup (a rerun calls it outside), a function only events invoke, or a
    /// continuation past an await. The guard throws there on every release.
    NotIn,
    /// Neither proven: a function that is named, stored or handed to a call
    /// this does not read may be run by a cleanup, and so may a possible
    /// component or a site in a parameter list.
    Unplaced,
}

/// Where `span` is written relative to a disposal
/// ([`solid_dialect::Dialect::callback_runs_only_in_disposal`]: `onCleanup`'s
/// callback), directly or through functions that run on that callback's stack:
/// a call that runs its callback during the call (`runWithOwner`, `createRoot`,
/// `untrack`), an IIFE, an inline standard-library callback ([`layer_run`]).
/// Only that is [`DisposalReach::Only`], and only when nothing else calls the
/// callback: rc.17 returns it, so each call of the handle runs it too, and the
/// handle's calls answer as well ([`cleanup_handle`], ADR 0456): one outside a
/// disposal is [`DisposalReach::NotIn`], an unread use unplaced. A layer that runs on a fresh stack, is a proven
/// component, a callback the engine schedules, or an effect's returned cleanup
/// is [`DisposalReach::NotIn`]; every other layer is
/// [`DisposalReach::Unplaced`], since a cleanup may be its caller.
///
/// An await that precedes the site in a layer suspends the run past the
/// disposal bracket once the climb has left the callback that gave the write
/// its owner; inside that callback the owner is gone with it, which is not
/// this question's, so the site is unplaced.
pub(super) fn runs_in_disposal(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> DisposalReach {
    runs_in_disposal_within(file, span, lookup, 0)
}

/// [`runs_in_disposal`], `depth` handle calls deep (ADR 0456): a call of a
/// cleanup's handle written in another cleanup asks the question again there.
///
/// Memoized per site and depth, for the same reason as
/// [`component_mount_reach`]: a cleanup handle's calls fan out at each depth.
fn runs_in_disposal_within(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> DisposalReach {
    lookup.disposal_reach_memo(file.path.as_str(), span, depth, || {
        runs_in_disposal_uncached(file, span, lookup, depth)
    })
}

fn runs_in_disposal_uncached(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> DisposalReach {
    let mut at = span;
    // Whether the climb has left the callback of the call that gave the write
    // its owner (`runWithOwner`, `createRoot`).
    let mut owner_given = false;
    // Whether a layer on the way is a helper that also hands its callback on or
    // keeps it ([`LayerRun::InPlaceAmongOthers`]): runs of it are unplaced.
    let mut among_others = false;
    // Whether a callback on the way runs at a time its call does not state: it
    // runs during the call or later, and only the call's place can say that it
    // is not in a disposal.
    let mut timing_unknown = false;
    // Bounded: each step moves to the call that holds the previous function.
    for _ in 0..16 {
        let Some(function) = crate::owners::owning_function(&file.ast, at) else {
            // Module code runs when the module is evaluated, never on the stack
            // of a cleanup.
            return DisposalReach::NotIn;
        };
        // A site in a parameter list runs when its call omits the argument; a
        // generator runs at each `next`.
        if function.generator || !function.body.contains(at) {
            return DisposalReach::Unplaced;
        }
        if follows_dominating_await(file, at) || follows_try_head_await(file, at, lookup) {
            return if owner_given {
                DisposalReach::NotIn
            } else {
                DisposalReach::Unplaced
            };
        }
        if may_follow_suspension(file, at) {
            return DisposalReach::Unplaced;
        }
        let primitive_call = file
            .ast
            .arguments_containing(function.span)
            .filter(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
            })
            .find_map(|(call, index)| {
                lookup
                    .primitive_at_call(file, call.span)
                    .map(|primitive| (call, index, primitive))
            })
            // ADR 0466: a primitive call that names the function by its binding
            // runs it there; other entries are runs this climb does not follow.
            .or_else(|| {
                let position = by_name_position(file, function, lookup)?;
                among_others |= !position.sole;
                Some((position.call, position.index, position.primitive))
            });
        if let Some((call, index, primitive)) = primitive_call {
            if lookup
                .dialect
                .callback_runs_only_in_disposal(primitive, index)
            {
                // A callback on the way that may run after its call returns
                // places nothing at the cleanup's call or its handle's, and
                // neither does one a helper also hands on or keeps
                // ([`LayerRun::InPlaceAmongOthers`], ADR 0457): `Only` needs
                // every run inside the disposal, and the handle's calls are
                // not followed through a callback with runs this does not see.
                if timing_unknown || among_others {
                    return DisposalReach::Unplaced;
                }
                // ADR 0456: rc.17 returns the callback, so each call of the
                // handle also runs it there. One such call outside a disposal
                // runs the write outside one.
                return match cleanup_handle(file, call, primitive, index, lookup) {
                    CleanupHandle::DisposalOnly => DisposalReach::Only,
                    CleanupHandle::Unplaced => DisposalReach::Unplaced,
                    CleanupHandle::Called(_) if depth >= 4 => DisposalReach::Unplaced,
                    CleanupHandle::Called(sites) => {
                        let mut reach = DisposalReach::Only;
                        for site in sites {
                            match runs_in_disposal_within(file, site, lookup, depth + 1) {
                                // Where the backend withdraws a violation, a
                                // call outside a disposal proves nothing.
                                DisposalReach::NotIn
                                    if handle_site_unplaced(file, site, lookup) =>
                                {
                                    reach = DisposalReach::Unplaced;
                                }
                                DisposalReach::NotIn => return DisposalReach::NotIn,
                                DisposalReach::Only => {}
                                DisposalReach::Unplaced => reach = DisposalReach::Unplaced,
                            }
                        }
                        reach
                    }
                };
            }
            match callback_execution_at_call(file, call, primitive, index, lookup) {
                Some(Execution::Inline) => {}
                // The engine runs the callback on its own schedule, which a
                // flush inside a cleanup may put in that cleanup: only a
                // callback created outside every cleanup is proven outside.
                Some(_) if inside_disposal_callback(file, call.span, lookup) => {
                    return DisposalReach::Unplaced;
                }
                Some(_) => return DisposalReach::NotIn,
                None => timing_unknown = true,
            }
            owner_given |= lookup
                .dialect
                .callback_runs_in_created_root(primitive, index)
                || lookup
                    .dialect
                    .callback_inherits_calling_window(primitive, index);
            at = call.span;
            continue;
        }
        match layer_run(file, function, lookup) {
            LayerRun::InPlace(call) => at = call,
            LayerRun::FreshStack => return DisposalReach::NotIn,
            // `NotIn` needs one run outside a disposal, which the in place run
            // is; `Only` needs every run inside, which `among_others` denies.
            LayerRun::InPlaceAmongOthers(call) => {
                among_others = true;
                at = call;
            }
            LayerRun::Elsewhere | LayerRun::Withheld(_) => {
                return match lookup.function_component_status(file, function) {
                    ComponentStatus::Proven => component_mount_reach(file, function, lookup, depth),
                    // ADR 0782: a handler literal is no component, whatever it
                    // renders (`onClick={() => { createRoot(() => <Inner />); }}`).
                    ComponentStatus::Uncertain if is_event_handler_literal(file, function) => {
                        DisposalReach::NotIn
                    }
                    ComponentStatus::Uncertain | ComponentStatus::No
                        if let Some(element) = jsx_child_callback_element(file, function)
                            && !returns_effect_cleanup(file, function, lookup) =>
                    {
                        // A row or a branch runs where the element around it
                        // mounts, and again on its own schedule, which `Only`
                        // cannot read: the climb goes on, as for a timing
                        // unknown.
                        timing_unknown = true;
                        at = element;
                        continue;
                    }
                    ComponentStatus::Uncertain => DisposalReach::Unplaced,
                    // On a release whose guard stands down in a disposal, an
                    // effect's returned cleanup also runs when the effect's
                    // owner is disposed (a memo that reruns, a `Show` turned
                    // off, a removed row, a disposed root), inside a disposal.
                    // A rerun that finds the owner alive is the only run
                    // outside one, and nothing orders the owner's disposal
                    // against it (an owner that tracks what the effect tracks
                    // disposes it first): no proof, unplaced.
                    ComponentStatus::No
                        if lookup.dialect.write_guard_in_disposal()
                            == solid_dialect::WriteGuardInDisposal::StandsDown
                            && returns_effect_cleanup(file, function, lookup) =>
                    {
                        DisposalReach::Unplaced
                    }
                    ComponentStatus::No
                        if returned_by_rerunning_effect(file, function, lookup)
                            || event_only_entries(file, function, lookup)
                            || is_event_handler_literal(file, function)
                            || stored_function_called_only_by_events(file, function, lookup) =>
                    {
                        DisposalReach::NotIn
                    }
                    ComponentStatus::No => DisposalReach::Unplaced,
                };
            }
        }
    }
    DisposalReach::Unplaced
}

/// The element whose child expression holds `function`, the innermost:
/// `<For each={xs}>{() => <Row />}</For>`.
fn jsx_child_callback_element(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Option<Span> {
    file.ast
        .jsx_containing(function.span)
        .filter(|element| {
            element
                .children
                .iter()
                .any(|child| child.contains(function.span))
        })
        .min_by_key(|element| element.span.end - element.span.start)
        .map(|element| element.span)
}

/// Where a proven component's body runs (ADR 0782), from its mount sites: any
/// one outside a disposal is that run; all of them inside one, with no other
/// way to the function, is [`DisposalReach::Only`]. A component with no site
/// here is mounted by what this does not see, outside any disposal as ever
/// before. A value handed on, or an export, may mount it anywhere.
///
/// Memoized per function and depth: every write in a component asks, and a
/// mount site's climb fans out again (`sites^4` without it). The depth is part
/// of the key because the answer depends on it: one kept per function would
/// hand a caller near the bound the answer of one far from it.
fn component_mount_reach(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> DisposalReach {
    lookup.component_mount_reach_memo(file.path.as_str(), function.span, depth, || {
        component_mount_reach_uncached(file, function, lookup, depth)
    })
}

fn component_mount_reach_uncached(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> DisposalReach {
    let sites = lookup.function_call_sites(file.path.as_str(), function.span);
    // A recursive tag runs under a mount this already asks about: it adds no
    // entry, and following it only spends the depth.
    let outside = sites
        .iter()
        .filter(|(caller, site)| caller.path != file.path || !function.span.contains(*site))
        .count();
    if outside == 0 {
        return DisposalReach::NotIn;
    }
    if depth >= 4 {
        return DisposalReach::Unplaced;
    }
    let mut reach = if component_sites_exhaustive(file, function, &sites, lookup) {
        DisposalReach::Only
    } else {
        DisposalReach::Unplaced
    };
    for (caller, site) in sites {
        if caller.path == file.path && function.span.contains(site) {
            continue;
        }
        match runs_in_disposal_within(caller, site, lookup, depth + 1) {
            DisposalReach::NotIn => return DisposalReach::NotIn,
            DisposalReach::Only => {}
            DisposalReach::Unplaced => reach = DisposalReach::Unplaced,
        }
    }
    reach
}

/// Whether every reference of the component's binding is one of its mount
/// sites: nothing exports it, hands it on or stores it.
fn component_sites_exhaustive(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    sites: &[(&solid_facts::FileFacts, Span)],
    lookup: &SemanticLookup<'_>,
) -> bool {
    if lookup.program_has_eval() {
        return false;
    }
    let Some(declaration) = function_binding_name(file, function).map(|binding| binding.span)
    else {
        return false;
    };
    if flow_binding_exported(file, declaration) {
        return false;
    }
    let Some(symbol) = lookup.entities().at(file.path.as_str(), declaration) else {
        return false;
    };
    lookup
        .symbol_references(symbol.as_str())
        .iter()
        .all(|reference| {
            let span = Span::new(reference.start_byte as u32, reference.end_byte as u32);
            span == declaration
                || sites.iter().any(|(caller, site)| {
                    caller.path.as_str() == reference.path.as_ref() && site.contains(span)
                })
        })
}

/// Whether `span` is written inside a callback that runs only in a disposal
/// ([`solid_dialect::Dialect::callback_runs_only_in_disposal`]). An effect
/// created there and flushed there applies inside that disposal.
fn inside_disposal_callback(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast.arguments_containing(span).any(|(call, index)| {
        lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| {
                lookup
                    .dialect
                    .callback_runs_only_in_disposal(primitive, index)
            })
    })
}

/// Whether `function` is a literal the compiler reads as an event handler (a
/// JSX attribute's value on an element) of an event only the host delivers. A
/// user's event runs it outside any disposal, whatever else dispatches:
/// `NotIn` needs that one run (ADR 0782).
fn is_event_handler_literal(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    file.compiler.callback_roles.iter().any(|callback| {
        callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
            && callback.span == function.span
            && !super::handler_event_denied(file, callback.span)
    })
}

/// Whether `function` is stored in one local variable and every use of that
/// variable is a call inside an event handler: a function a cleanup stores for
/// a handler to call runs in no disposal. Any other use of the variable (an
/// alias, an argument, a return, a call anywhere else), an export, an `eval`
/// or a synchronous event dispatch answers nothing.
fn stored_function_called_only_by_events(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_facts::ast::IdentifierRole;
    if lookup.synchronous_event_dispatch_present() || lookup.program_has_eval() {
        return false;
    }
    // The variable: assigned this literal, or declared with it.
    let declaration = file
        .ast
        .assignments
        .iter()
        .find(|assignment| {
            !assignment.reads_target
                && file.ast.peel_ts_sugar_span(assignment.value_span) == function.span
        })
        .and_then(|assignment| file.ast.reference_declaration(assignment.target))
        .or_else(|| function_binding_name(file, function).map(|binding| binding.span));
    let Some(declaration) = declaration else {
        return false;
    };
    if flow_binding_exported(file, declaration) {
        return false;
    }
    let Some(symbol) = lookup.entities().at(file.path.as_str(), declaration) else {
        return false;
    };
    // A reference from another file is a use this census does not read.
    if lookup
        .symbol_references(symbol.as_str())
        .iter()
        .any(|reference| reference.path.as_ref() != file.path.as_str())
    {
        return false;
    }
    let references = file
        .ast
        .identifiers
        .iter()
        .filter(|id| {
            id.role == IdentifierRole::Reference
                && file.ast.reference_declaration(id.span) == Some(declaration)
        })
        .map(|id| id.span)
        .collect::<Vec<_>>();
    !references.is_empty()
        && references.into_iter().all(|reference| {
            if file.ast.write_targets.contains(&reference) {
                return true;
            }
            file.ast.calls.iter().any(|call| {
                !call.construct && file.ast.peel_ts_sugar_span(call.callee) == reference
            }) && crate::owners::owning_function(&file.ast, reference).is_some_and(|caller| {
                file.compiler.callback_roles.iter().any(|callback| {
                    callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                        && callback.span == caller.span
                        && !super::handler_event_denied(file, callback.span)
                })
            })
        })
}

/// Whether `function` is the cleanup an effect's apply returns, which a rerun
/// calls outside a disposal ([`solid_dialect::Dialect::returned_cleanup_runs_outside_disposal`])
/// and the owner's disposal or the final teardown calls inside one, and the
/// effect provably reruns ([`effect_provably_reruns`]). That proof alone places
/// no run outside a disposal where the guard stands down there (see
/// [`returns_effect_cleanup`]); it is the proof for a release nobody read.
fn returned_by_rerunning_effect(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    returned_effect_cleanup(file, function, lookup, true)
}

/// Whether `function` is returned by the apply (or tracked body) of an effect
/// whose returned cleanup a rerun calls outside a disposal, with no claim that
/// the effect reruns.
fn returns_effect_cleanup(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    returned_effect_cleanup(file, function, lookup, false)
}

fn returned_effect_cleanup(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    prove_rerun: bool,
) -> bool {
    let Some(outer) = function_returning(file, function) else {
        return false;
    };
    file.ast
        .arguments_containing(outer.span)
        .filter(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[*index].span) == outer.span
        })
        .any(|(call, index)| {
            lookup
                .primitive_at_call(file, call.span)
                .is_some_and(|primitive| {
                    lookup
                        .dialect
                        .returned_cleanup_runs_outside_disposal(primitive, index)
                        && (!prove_rerun
                            || effect_provably_reruns(file, call, index, outer, function, lookup))
                })
        })
}

/// The function whose body returns `function` itself (an expression body or a
/// `return` of that function, not of a nested one), if any.
fn function_returning<'a>(
    file: &'a solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Option<&'a solid_facts::ast::FunctionFact> {
    let outer = crate::owners::owning_function(&file.ast, function.span)?;
    if !outer.body.contains(function.span) {
        return None;
    }
    outer
        .expression_return
        .as_ref()
        .into_iter()
        .chain(
            file.ast
                .returns
                .iter()
                .filter(|ret| outer.body.contains(ret.span)),
        )
        .any(|ret| {
            ret.argument.is_some_and(|argument| {
                file.ast.peel_ts_sugar_span(argument) == function.span
                    && crate::owners::owning_function(&file.ast, ret.span)
                        .is_some_and(|owner| owner.span == outer.span)
            })
        })
        .then_some(outer)
}

/// ADR 0539: a read in `function`, when `function` is the cleanup an effect's
/// apply returns and a rerun calls it in the apply's strict-read window
/// ([`solid_dialect::Dialect::returned_cleanup_reruns_in_read_window`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WindowedCleanup {
    /// The effect's compute provably reads nothing ([`effect_never_reruns`]):
    /// only a disposal calls the cleanup, outside the window.
    NeverReruns,
    /// A rerun may call it in the window; nothing orders the owner's disposal
    /// (which would call it outside the window) against that rerun.
    MayRerun,
}

/// ADR 0539: whether `span` runs in place in the cleanup an effect's apply
/// returns, a rerun of which may call it in the apply's strict-read window
/// ([`WindowedCleanup::MayRerun`]). An exhausted climb answers `true`.
pub(crate) fn inside_rerun_effect_cleanup(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let at = match in_place_run_site(file, span, lookup) {
        Some((climb, _)) if climb.exhausted => return true,
        Some((climb, _)) => climb.at,
        None => span,
    };
    crate::owners::containing_ast_function(&file.ast, at).is_some_and(|function| {
        windowed_returned_cleanup(file, function, lookup) == Some(WindowedCleanup::MayRerun)
    })
}

pub(super) fn windowed_returned_cleanup(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<WindowedCleanup> {
    let outer = function_returning(file, function)?;
    let mut answer = None;
    for (call, index) in file
        .ast
        .arguments_containing(outer.span)
        .filter(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[*index].span) == outer.span
        })
    {
        let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
            continue;
        };
        if !lookup
            .dialect
            .returned_cleanup_reruns_in_read_window(primitive, index)
        {
            continue;
        }
        if effect_never_reruns(file, call, index) {
            answer.get_or_insert(WindowedCleanup::NeverReruns);
        } else {
            return Some(WindowedCleanup::MayRerun);
        }
    }
    // An apply named at the position (`createEffect(compute, apply)`): its
    // compute is not read here, so a rerun is never excluded.
    let named = lookup.named_callback_roles(file).get(outer.span);
    if answer.is_none()
        && named.effect_apply
        && named
            .effect_apply_primitive
            .single()
            .is_none_or(|primitive| {
                lookup
                    .dialect
                    .returned_cleanup_reruns_in_read_window(primitive, 1)
            })
    {
        return Some(WindowedCleanup::MayRerun);
    }
    answer
}

/// Whether the effect's compute (its first argument, an inline function the
/// apply at `index` 1 follows) provably tracks nothing, so that the effect
/// never reruns: a synchronous function with no call, no identifier
/// reference, no `this`, `super` or `arguments` in its text, whose every
/// `return` gives a primitive or nullish literal. Any other compute may track.
fn effect_never_reruns(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    index: usize,
) -> bool {
    use solid_facts::ast::{IdentifierRole, RuntimeValueKind};
    if index != 1 {
        return false;
    }
    let Some(compute) = call
        .arguments
        .first()
        .filter(|compute| !compute.spread)
        .and_then(|compute| crate::cleanup::callback_argument_literal(file, compute.span))
    else {
        return false;
    };
    if compute.r#async || compute.generator {
        return false;
    }
    let text = usize::try_from(compute.span.start)
        .ok()
        .zip(usize::try_from(compute.span.end).ok())
        .and_then(|(start, end)| file.source.get(start..end));
    let Some(text) = text else {
        return false;
    };
    if ["this", "super", "arguments"]
        .iter()
        .any(|word| text.contains(word))
    {
        return false;
    }
    let returns_literal = |ret: &solid_facts::ast::ReturnFact| {
        ret.argument.is_none()
            || matches!(
                ret.runtime_value_kind,
                RuntimeValueKind::Primitive | RuntimeValueKind::Nullish
            )
    };
    !file
        .ast
        .calls
        .iter()
        .any(|inner| compute.span.contains(inner.span))
        && !file
            .ast
            .identifiers
            .iter()
            .any(|id| id.role == IdentifierRole::Reference && compute.body.contains(id.span))
        && compute
            .expression_return
            .as_ref()
            .is_none_or(returns_literal)
        && file
            .ast
            .returns
            .iter()
            .filter(|ret| compute.body.contains(ret.span))
            .all(returns_literal)
}

/// Whether the effect whose apply (or tracked body) is `outer` provably tracks
/// a source that can change, so that its returned cleanup `cleanup` may be
/// called by a rerun and not only by the final teardown (which runs it inside
/// a disposal). Where the guard stands down in a disposal this is no proof of
/// a run outside one: the owner may be disposed before the rerun. The function that tracks (the compute of `createEffect` and
/// `createRenderEffect`, the body of `createTrackedEffect`) must call, on
/// every completion, the accessor of a `createSignal` declared in this file
/// whose setter is referenced outside `cleanup` or exported. An effect that
/// reads nothing, reads under a condition or `untrack`, reads another kind of
/// source, or reads a signal nothing writes may never rerun: unplaced.
fn effect_provably_reruns(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    index: usize,
    outer: &solid_facts::ast::FunctionFact,
    cleanup: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_facts::ast::IdentifierRole;
    let tracked = if index == 0 {
        Some(outer)
    } else {
        call.arguments
            .first()
            .filter(|compute| !compute.spread)
            .and_then(|compute| crate::cleanup::callback_argument_literal(file, compute.span))
    };
    let Some(tracked) = tracked else {
        return false;
    };
    if tracked.r#async || tracked.generator {
        return false;
    }
    let Some(unconditional) = solid_facts::ast::synchronously_unconditional_calls(
        std::path::Path::new(file.path.as_str()),
        &file.source,
        tracked.span,
        &[],
    ) else {
        return false;
    };
    file.ast.calls.iter().any(|read| {
        if read.construct
            || !read.arguments.is_empty()
            || !unconditional.contains(&read.span)
            || crate::owners::owning_function(&file.ast, read.span)
                .is_none_or(|owner| owner.span != tracked.span)
        {
            return false;
        }
        let callee = file.ast.peel_ts_sugar_span(read.callee);
        let Some(declaration) = file.ast.reference_declaration(callee) else {
            return false;
        };
        let Some((binding_file, binding, _)) =
            lookup.binding_at_reference(file.path.as_str(), callee)
        else {
            return false;
        };
        if binding_file.path != file.path
            || !binding.immutable
            || binding.shape != solid_facts::ast::BindingShape::Array
            || binding
                .array_slots
                .first()
                .and_then(Option::as_ref)
                .is_none_or(|accessor| accessor.span != declaration)
        {
            return false;
        }
        let Some(initializer) = binding
            .call_initializer
            .map(|span| file.ast.peel_ts_sugar_span(span))
            .and_then(|span| file.ast.call_at(span))
        else {
            return false;
        };
        if initializer.construct
            || lookup.primitive_at_call(file, initializer.span)
                != Some(solid_dialect::Primitive::CreateSignal)
        {
            return false;
        }
        let Some(Some(setter)) = binding.array_slots.get(1) else {
            return false;
        };
        flow_binding_exported(file, setter.span)
            || file.ast.identifiers.iter().any(|id| {
                id.role == IdentifierRole::Reference
                    && file.ast.reference_declaration(id.span) == Some(setter.span)
                    && !cleanup.span.contains(id.span)
            })
    })
}

/// ADR 0455: the innermost function around `span` that is bound to a name and
/// written in a callback that keeps its call's owner and runs it during the call
/// (`untrack`, [`solid_dialect::Dialect::callback_preserves_owner_write_context`]).
/// The callback's lexical role covers the closure's body (`allowed`), but
/// constructing the closure runs nothing: it runs where it is called, and the
/// write follows its call sites, as for a closure in a tracked scope
/// ([`named_closure_in_tracked_scope`]). The innermost primitive callback around
/// the closure decides; an anonymous function is judged by the layers
/// ([`runs_in_place_within`]) the caller proves.
fn named_closure_in_owner_keeping_callback<'a>(
    file: &'a solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<&'a solid_facts::ast::FunctionFact> {
    functions_around(file, span)
        .into_iter()
        .filter(|function| {
            function.method_name.is_none()
                && function_binding_name(file, function).is_some()
                && lookup.function_component_status(file, function) == ComponentStatus::No
                && file
                    .ast
                    .arguments_containing(function.span)
                    .filter(|(call, index)| {
                        call.arguments[*index].span != function.span
                            && lookup.primitive_at_call(file, call.span).is_some()
                    })
                    .min_by_key(|(call, index)| {
                        let argument = call.arguments[*index].span;
                        argument.end - argument.start
                    })
                    .is_some_and(|(call, index)| {
                        lookup
                            .primitive_at_call(file, call.span)
                            .is_some_and(|primitive| {
                                callback_takes_call_owner(file, call, primitive, index, lookup)
                            })
                    })
        })
        .min_by_key(|function| function.body.end - function.body.start)
}

/// ADR 0471: the innermost function around `span` that is bound to a name and
/// whose innermost primitive callback is a created root's body, written inline
/// or named by its binding (`createRoot(f1)`). The root's body is not one of
/// the regions a write may treat as imperative ([`write_allowed_spans`]), but a
/// cleanup or an effect apply around the root call is, and its lexical role
/// covers the closure. Constructing the closure runs nothing: it runs where it
/// is called, under the root when that is in the root's body, so its call
/// sites decide. The climb crosses every function that is no primitive's
/// callback (an IIFE, a helper's callback, another named closure, whose own
/// calls the walk then follows).
fn named_closure_in_root_callback<'a>(
    file: &'a solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<&'a solid_facts::ast::FunctionFact> {
    functions_around(file, span)
        .into_iter()
        .filter(|function| {
            function.method_name.is_none()
                && function_binding_name(file, function).is_some()
                && lookup.function_component_status(file, function) == ComponentStatus::No
                && innermost_primitive_callback_is_root(file, function, lookup)
        })
        .min_by_key(|function| function.body.end - function.body.start)
}

/// The call [`handed_to_unreviewed_call`] finds `function` written in.
pub(crate) fn handed_to_unreviewed_call_at(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    file.ast
        .arguments_containing(function.span)
        .filter(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
        })
        .find(|(call, _)| {
            lookup.primitive_at_call(file, call.span).is_none()
                && !lookup
                    .resolved_callee_call(file, call.callee)
                    .and_then(|resolved| resolved.declaration.as_ref())
                    .is_some_and(|declaration| declaration.standard_library)
        })
        .map(|(call, _)| call.span)
}

/// Whether the first function around `function` that a primitive call takes as
/// its callback, by the literal or by its binding, is taken in a position whose
/// callback runs in a root the call creates, or lies past the climb's bound
/// (ADR 0532: unread, so not proven otherwise).
fn innermost_primitive_callback_is_root(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let mut at = function.span;
    // Bounded: each step moves to the function that holds the previous one.
    for _ in 0..16 {
        let Some(outer) = crate::owners::owning_function(&file.ast, at) else {
            return false;
        };
        let positions = primitive_positions(file, outer, lookup);
        if positions.is_empty() {
            at = outer.span;
            continue;
        }
        return positions.into_iter().any(|(primitive, index)| {
            lookup
                .dialect
                .callback_runs_in_created_root(primitive, index)
        });
    }
    // ADR 0532: past the bound the callback is unread, so the permissive role
    // around it is no proof: answer as a root would (the entries decide).
    true
}

/// ADR 0471: whether `span` runs, through layers that run in place (an IIFE,
/// an inline standard-library callback, a helper's callback), in a function a
/// primitive call names by its binding in a created root's position. A
/// position the by-name climb does not admit (one in a nested function, which
/// may run before a `const` is initialized) still runs the function under the
/// root when it runs, so the permissive role of a region around the function
/// is no proof for the site; the site's own entries decide.
fn in_place_in_root_named_callback(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let path = file.path.as_str();
    let mut at = span;
    // Bounded: each step moves to the call that holds the previous function.
    for _ in 0..16 {
        let Some(function) = crate::owners::owning_function(&file.ast, at) else {
            return false;
        };
        if function_binding_name(file, function)
            .filter(|_| function.method_name.is_none())
            .and_then(|name| lookup.entities().at(path, name.span))
            .is_some()
            && primitive_positions(file, function, lookup)
                .into_iter()
                .any(|(primitive, index)| {
                    lookup
                        .dialect
                        .callback_runs_in_created_root(primitive, index)
                })
        {
            return true;
        }
        match layer_run(file, function, lookup) {
            LayerRun::InPlace(call) | LayerRun::InPlaceAmongOthers(call) => at = call,
            LayerRun::FreshStack | LayerRun::Elsewhere | LayerRun::Withheld(_) => return false,
        }
    }
    // ADR 0532: the layers past the bound are unread; the permissive role
    // around them is no proof for the site.
    true
}

/// ADR 0828: whether a function around `span` is handed to a call that is no
/// primitive and no standard-library call with a reviewed behavior
/// (`hc1(() => set(1))`, a helper that forwards its callback to another), and
/// sits, through any layers, in a callback a primitive runs under an owner it
/// supplies: a created root (`createRoot`'s body) or one the call names
/// (`runWithOwner(owner, fn)` with a created or unproven owner). The helper
/// may run the closure on that callback's stack, under that owner, so the
/// permissive role of a region around the callback (the handler that holds
/// the `createRoot` call) is no proof for the site; the site's own entries
/// decide. The same as ADR 0471's named closure, for an anonymous one handed
/// on.
fn handed_closure_in_owner_callback(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    functions_around(file, span).into_iter().any(|function| {
        function.method_name.is_none()
            && lookup.function_component_status(file, function) == ComponentStatus::No
            && handed_to_unreviewed_call(file, function, lookup)
            && owner_supplying_callback_around(file, function, lookup)
    })
}

/// Whether `function` is written as an argument of a call that is no
/// primitive and whose callee no standard-library declaration resolves.
fn handed_to_unreviewed_call(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    handed_to_unreviewed_call_at(file, function, lookup).is_some()
}

/// Whether the first function around `function` that a primitive call takes
/// as its literal callback runs under an owner the call supplies: a created
/// root, or an owner `runWithOwner` names that is created or not proven null
/// ([`solid_dialect::CallbackOwner::Creates`], `Conditional`). Past the bound
/// the layers are unread (ADR 0532): `true`.
fn owner_supplying_callback_around(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let mut at = function.span;
    // Bounded: each step moves to the function that holds the previous one.
    for _ in 0..16 {
        let Some(outer) = crate::owners::owning_function(&file.ast, at) else {
            return false;
        };
        // A handler, a callback prop or a scheduled callback runs later, on
        // its own entries: an owner a call outside it supplies is not its.
        if jsx_attribute_value_function(file, outer.span)
            || matches!(layer_run(file, outer, lookup), LayerRun::FreshStack)
        {
            return false;
        }
        let positions = file
            .ast
            .arguments_containing(outer.span)
            .filter(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == outer.span
            })
            .filter_map(|(call, index)| {
                lookup
                    .primitive_at_call(file, call.span)
                    .map(|primitive| (call, index, primitive))
            })
            .collect::<Vec<_>>();
        if positions.is_empty() {
            at = outer.span;
            continue;
        }
        return positions.into_iter().any(|(call, index, primitive)| {
            lookup
                .dialect
                .callback_runs_in_created_root(primitive, index)
                || (lookup
                    .dialect
                    .callback_inherits_calling_window(primitive, index)
                    && matches!(
                        callback_owner_at_call(file, call, primitive, index, lookup),
                        Some(
                            solid_dialect::CallbackOwner::Creates
                                | solid_dialect::CallbackOwner::Conditional
                        )
                    ))
        });
    }
    true
}

/// The primitive call positions that take `function`: as the literal argument,
/// or by a bare identifier argument whose symbol is its binding's.
fn primitive_positions(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<(Primitive, usize)> {
    let path = file.path.as_str();
    let mut positions = file
        .ast
        .arguments_containing(function.span)
        .filter(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
        })
        .filter_map(|(call, index)| {
            lookup
                .primitive_at_call(file, call.span)
                .map(|primitive| (primitive, index))
        })
        .collect::<Vec<_>>();
    let symbol = function_binding_name(file, function)
        .filter(|_| function.method_name.is_none())
        .and_then(|name| lookup.entities().at(path, name.span));
    if let Some(symbol) = symbol {
        for call in &file.ast.calls {
            for (index, argument) in call.arguments.iter().enumerate() {
                if argument.spread
                    || argument.value != solid_facts::ast::ArgumentValueKind::Identifier
                    || lookup.entities().at(path, argument.span) != Some(symbol)
                {
                    continue;
                }
                if let Some(primitive) = lookup.primitive_at_call(file, call.span) {
                    positions.push((primitive, index));
                }
            }
        }
    }
    positions
}

/// How the functions between a site and the control-flow children callback
/// around it run (ADR 0460).
enum ControlFlowLayers {
    /// Every one runs in place, on the stack of the call at this span, written
    /// in the callback itself; `true` when a helper also hands one on.
    InPlace(Span, bool),
    /// One is bound to a name, stored or handed to a call this does not know:
    /// its entries place it, not the callback.
    Unrun,
}

/// The layers between `span` and the control-flow children callback that
/// gives it its deferred role ([`control_flow_execution_role`] answers
/// deferred only for a function nested in that callback). `None` when that
/// role does not come from such a callback, or when a layer has a role of its
/// own the write keeps: a primitive's callback, a JSX attribute's value (an
/// event handler), or a callback a scheduler runs on a fresh stack.
fn control_flow_nested_layers(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<ControlFlowLayers> {
    let deferred = |at: Span| {
        control_flow_execution_role(file, at, entities, symbol_names, lookup)
            == Some(ExecutionRole::DeferredCallback)
    };
    if !deferred(span) {
        return None;
    }
    let mut at = span;
    let mut among_others = false;
    // Bounded: each step moves to the call that runs the previous function.
    for _ in 0..16 {
        if !deferred(at) {
            return Some(ControlFlowLayers::InPlace(at, among_others));
        }
        let function = crate::owners::owning_function(&file.ast, at)?;
        if function.generator || !function.body.contains(at) {
            return Some(ControlFlowLayers::Unrun);
        }
        // Past an await the rest of the layer runs as a continuation with no
        // owner, where a write is legal; past a possible suspension, unplaced.
        if follows_dominating_await(file, at) || follows_try_head_await(file, at, lookup) {
            return None;
        }
        if may_follow_suspension(file, at) {
            return Some(ControlFlowLayers::Unrun);
        }
        let primitive_argument =
            file.ast
                .arguments_containing(function.span)
                .any(|(call, index)| {
                    file.ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
                        && lookup.primitive_at_call(file, call.span).is_some()
                });
        if primitive_argument || jsx_attribute_value_function(file, function.span) {
            return None;
        }
        match layer_run(file, function, lookup) {
            LayerRun::InPlace(call) => at = call,
            LayerRun::InPlaceAmongOthers(call) => {
                among_others = true;
                at = call;
            }
            LayerRun::FreshStack => return None,
            LayerRun::Elsewhere | LayerRun::Withheld(_) => {
                return Some(ControlFlowLayers::Unrun);
            }
        }
    }
    Some(ControlFlowLayers::Unrun)
}

/// Whether the callback at `call`'s `index` places the writes written in it by
/// its call's owner: one that keeps that owner and runs during the call
/// (`untrack`, ADR 0455), or one that shares it on its first run
/// (`createRenderEffect`'s apply, ADR 0460). Its lexical role (deferred) is
/// then no proof for a function it builds.
fn callback_takes_call_owner(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: solid_dialect::Primitive,
    index: usize,
    lookup: &SemanticLookup<'_>,
) -> bool {
    (lookup
        .dialect
        .callback_preserves_owner_write_context(primitive)
        && callback_execution_at_call(file, call, primitive, index, lookup).is_some())
        || callback_owner_at_call(file, call, primitive, index, lookup)
            == Some(solid_dialect::CallbackOwner::InheritsFirstRun)
}

/// ADR 0455: whether `span` is written in a function that a callback keeping
/// its call's owner (`untrack`) neither runs in place nor on a fresh stack: a
/// literal it returns, stores or hands to a call this does not know. The
/// callback's lexical role says nothing about when that function runs, or under
/// which owner.
fn unrun_layer_in_owner_keeping_callback(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let mut at = span;
    let mut unrun = false;
    for _ in 0..16 {
        let Some(function) = crate::owners::owning_function(&file.ast, at) else {
            return false;
        };
        let primitive_call = file
            .ast
            .arguments_containing(function.span)
            .filter(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
            })
            .find_map(|(call, index)| {
                lookup
                    .primitive_at_call(file, call.span)
                    .map(|primitive| (call, index, primitive))
            });
        if let Some((call, index, primitive)) = primitive_call {
            return unrun && callback_takes_call_owner(file, call, primitive, index, lookup);
        }
        match layer_run(file, function, lookup) {
            LayerRun::InPlace(call) => at = call,
            LayerRun::FreshStack => return false,
            LayerRun::Elsewhere | LayerRun::InPlaceAmongOthers(_) | LayerRun::Withheld(_) => {
                unrun = true;
                at = function.span;
            }
        }
    }
    // ADR 0532: an owner-keeping callback past the bound is unread, so the
    // deferred role is no proof.
    true
}

/// A closure nested in a leaf callback (`createTrackedEffect`, `onSettled`):
/// a timer, a listener, a continuation, a helper the callback defines. It runs
/// later (deferred) or inside the leaf, and the runtime's write guard exempts
/// both. Only the *innermost* primitive callback containing the span decides --
/// an owner-creating primitive nested in the leaf (`createMemo`) is its own
/// scope and keeps its own answer -- and a call that is no primitive (the
/// scheduler a closure is handed to) is looked through.
pub(super) fn nested_in_leaf_scope(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<WriteRegionAdjustment> {
    if !lookup.dialect.leaf_scopes_allow_writes() {
        return None;
    }
    let (call, index) = file
        .ast
        .arguments_containing(span)
        .filter(|(call, _)| lookup.primitive_at_call(file, call.span).is_some())
        .min_by_key(|(call, index)| {
            let argument = call.arguments[*index].span;
            argument.end - argument.start
        })?;
    let primitive = lookup.primitive_at_call(file, call.span)?;
    // ADR 0613: as for the callback itself (`write_region_adjustment`), a layer
    // inside `onSettled`'s queued run is the drain's.
    if lookup
        .dialect
        .leaf_owner_requires_owned_call_site(primitive)
        && drained_under_owner(primitive, index, lookup)
    {
        return Some(WriteRegionAdjustment::DrainedByFlush);
    }
    (callback_owner_at_call(file, call, primitive, index, lookup)
        == Some(solid_dialect::CallbackOwner::Leaf))
    .then_some(WriteRegionAdjustment::LeafScope)
}

/// The write classification of `span`, and where it ends unplaced, ADR 0442:
/// the function that holds the write, or one that runs it in place, is handed
/// to other code (a component's prop, a call's argument, a named function),
/// and every invocation of it is found and placed by the invocation flow
/// ([`write_flow_role`]). A role the walk proves is never replaced.
#[allow(clippy::too_many_arguments)]
pub(super) fn semantic_write_execution_role_within(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    visiting: &mut HashSet<(String, Span)>,
) -> ExecutionRole {
    // ADR 0874: nothing places code an impostor may run, or never run.
    if !super::discarded_region_contains(file, span) && runs_in_withheld_layer(file, span, lookup) {
        return role_arm!(
            "write_roles.semantic_write_execution_role_within.withheld-callback",
            ExecutionRole::Unknown
        );
    }
    let mut decided = false;
    let role = write_role_walk(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        root_body,
        visiting,
        &mut decided,
        None,
    );
    write_role_fallback(file, span, role, decided, root_body, lookup)
}

/// What [`semantic_write_execution_role_within`] answers for the walk's
/// `role` at `span`: an `Unknown` that is not final takes the invocation
/// flow's role ([`write_flow_role`]), when there is one.
pub(super) fn write_role_fallback(
    file: &solid_facts::FileFacts,
    span: Span,
    role: ExecutionRole,
    decided: bool,
    root_body: RootBodyGuard,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    let role = write_role_fallback_unvetoed(file, span, role, decided, root_body, lookup);
    // ADR 0613: an apply role is legal for a write because the queue's drain
    // runs it with no owner; a synchronous flush may run it under one. Every
    // apply role (`createEffect`, `createRenderEffect`, `createReaction`) is
    // the drain's.
    if role == ExecutionRole::EffectApply && lookup.flush_drains_under_owner() {
        return role_arm!("write_roles.effect-apply-drained", ExecutionRole::Unknown);
    }
    role
}

fn write_role_fallback_unvetoed(
    file: &solid_facts::FileFacts,
    span: Span,
    role: ExecutionRole,
    decided: bool,
    root_body: RootBodyGuard,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    if role != ExecutionRole::Unknown || decided || file.compiler.withheld() {
        return role;
    }
    // A write that follows an await that may be skipped has no placement its
    // function's entries give (the walk stopped there on purpose).
    if may_follow_suspension(file, span) {
        return role;
    }
    write_flow_role(file, span, root_body, lookup).unwrap_or(role)
}

/// The `visiting` entry that restricts [`semantic_write_role_walk`] to the
/// role the position itself proves (ADR 0442): the aggregation over every
/// entry of the function that holds it is skipped. The write flow asks it of a
/// call inside a function it entered through one activation, where another
/// call site of that function says nothing about this run.
pub(super) fn lexical_only_marker() -> (String, Span) {
    ("\0lexical-only".to_owned(), Span::new(0, 0))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn semantic_write_role_walk(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    visiting: &mut HashSet<(String, Span)>,
) -> ExecutionRole {
    write_role_walk(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        root_body,
        visiting,
        &mut false,
        None,
    )
}

/// [`semantic_write_role_walk`], and whether its `Unknown` is final: the write
/// runs under an owner the call gave it, so what its function's other callers
/// run it under places nothing (`decided`).
///
/// With `plan`, a walk whose position proves nothing stops where its
/// function's entries would decide: the [`LoopPlan`] goes to `plan` and the
/// answer returned means nothing ([`super::write_components`] folds it).
#[allow(clippy::too_many_arguments)]
pub(super) fn write_role_walk<'f>(
    file: &'f solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    visiting: &mut HashSet<(String, Span)>,
    decided: &mut bool,
    plan: Option<&mut Option<LoopPlan<'f>>>,
) -> ExecutionRole {
    // ADR 0360: a file the compiler refused places no write.
    if file.compiler.withheld() {
        return role_arm!(
            "write_roles.write_role_walk.unknown",
            ExecutionRole::Unknown
        );
    }
    // Write legality follows the runtime's *owner* context, which is not
    // always the read-execution context: leaf scopes are write-legal though
    // their reads track, and `untrack` writes answer to the enclosing owner
    // though its reads do not subscribe. Resolve those two adjustments first
    // (innermost outward) and only then classify normally.
    let mut span = span;
    loop {
        match write_region_adjustment(file, span, lookup) {
            Some(WriteRegionAdjustment::LeafScope) => {
                return role_arm!(
                    "write_roles.write_role_walk.deferred-callback",
                    ExecutionRole::DeferredCallback
                );
            }
            Some(WriteRegionAdjustment::CallSite(outer)) => span = outer,
            Some(WriteRegionAdjustment::CallSiteAmongOthers(outer)) => {
                let role = semantic_write_execution_role_within(
                    file,
                    outer,
                    allowed,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    visiting,
                );
                if role.reports_disallowed_write() {
                    return role_arm!("write_roles.walk-role-1", role);
                }
                *decided = true;
                return role_arm!(
                    "write_roles.write_role_walk.unknown-2",
                    ExecutionRole::Unknown
                );
            }
            // A root body is a one-shot, untracked run under a
            // children-capable owner -- in dev builds exactly what a
            // component body is, since `solid-js` runs every component body
            // in `createRoot(…, { transparent: true })` then `untrack`. So a
            // rejected operation there takes the component body's role; an
            // exempted one takes the inline untracked callback's, which
            // reports nothing.
            //
            // Past an await the body runs on a later task with no owner, so
            // the root is no longer the ambient owner.
            Some(WriteRegionAdjustment::CreatedComputation) => {
                if let Some(role) = suspended_write_role(file, span, lookup) {
                    return role_arm!("write_roles.created-computation-suspended", role);
                }
                return role_arm!("write_roles.created-computation", ExecutionRole::TrackedJsx);
            }
            Some(
                adjustment @ (WriteRegionAdjustment::CreatedRoot
                | WriteRegionAdjustment::CreatedRootAmongOthers),
            ) => {
                if let Some(role) = suspended_write_role(file, span, lookup) {
                    return role_arm!("write_roles.walk-role-2", role);
                }
                match root_body {
                    RootBodyGuard::Rejects => {
                        return role_arm!(
                            "write_roles.write_role_walk.untracked-rendering",
                            ExecutionRole::UntrackedRendering
                        );
                    }
                    // Legal in the run under the root; the callback's other runs
                    // are not placed, so a legal write is no claim there.
                    RootBodyGuard::Exempts
                        if matches!(adjustment, WriteRegionAdjustment::CreatedRootAmongOthers) =>
                    {
                        *decided = true;
                        return role_arm!(
                            "write_roles.write_role_walk.unknown-3",
                            ExecutionRole::Unknown
                        );
                    }
                    RootBodyGuard::Exempts => {
                        return role_arm!(
                            "write_roles.write_role_walk.untracked-callback",
                            ExecutionRole::UntrackedCallback
                        );
                    }
                    // The guard stands down while an owner is disposed, on the
                    // release that says so, and the write is written in a
                    // callback that runs only then (ADR 0455 amendment). A
                    // release nobody read leaves a write that a disposal
                    // decides unplaced; outside a disposal the guard throws on
                    // every release.
                    RootBodyGuard::RejectsOutsideDisposal => {
                        use solid_dialect::WriteGuardInDisposal as Guard;
                        let guard = lookup.dialect.write_guard_in_disposal();
                        if guard == Guard::Rejects {
                            return role_arm!(
                                "write_roles.write_role_walk.untracked-rendering-2",
                                ExecutionRole::UntrackedRendering
                            );
                        }
                        // A violation needs the proof that the write is not in a
                        // disposal; a legal write needs the release to say so.
                        // Neither is the call sites' to give: the owner is the
                        // call's, whatever runs the function.
                        return match runs_in_disposal(file, span, lookup) {
                            DisposalReach::NotIn => role_arm!(
                                "write_roles.created-root-outside-disposal",
                                ExecutionRole::UntrackedRendering
                            ),
                            DisposalReach::Only if guard == Guard::StandsDown => {
                                role_arm!(
                                    "write_roles.created-root-disposal-stands-down",
                                    ExecutionRole::UntrackedCallback
                                )
                            }
                            DisposalReach::Only | DisposalReach::Unplaced => {
                                *decided = true;
                                role_arm!(
                                    "write_roles.created-root-disposal-unplaced",
                                    ExecutionRole::Unknown
                                )
                            }
                        };
                    }
                }
            }
            // Whether a root wraps this callback is not audited on this
            // release, so the write has no placement the checker can prove.
            Some(WriteRegionAdjustment::UnauditedRoot) => {
                return role_arm!(
                    "write_roles.write_role_walk.unknown-4",
                    ExecutionRole::Unknown
                );
            }
            // No owner reaches the write: the guard has nothing to reject.
            Some(WriteRegionAdjustment::Detached) => {
                return role_arm!(
                    "write_roles.write_role_walk.untracked-callback-2",
                    ExecutionRole::UntrackedCallback
                );
            }
            Some(WriteRegionAdjustment::OwnerUnknown) => {
                return role_arm!(
                    "write_roles.write_role_walk.unknown-5",
                    ExecutionRole::Unknown
                );
            }
            // ADR 0532: the bound of the climb proves nothing either way.
            Some(WriteRegionAdjustment::ClimbExhausted) => {
                return role_arm!(
                    "write_roles.write_role_walk.climb-exhausted",
                    ExecutionRole::Unknown
                );
            }
            Some(WriteRegionAdjustment::DrainedByFlush) => {
                *decided = true;
                return role_arm!(
                    "write_roles.write_role_walk.drained-by-flush",
                    ExecutionRole::Unknown
                );
            }
            Some(WriteRegionAdjustment::FirstRunAtCallSite(call)) => {
                // Where the call site forbids the write, the first run throws
                // exactly when it runs during the call -- which needs the
                // compute to settle synchronously on its first pass, with no
                // `defer`/`schedule` option, and on rc.9 outside a staged
                // transaction. None of that is proven here, so the write is
                // neither a violation nor legal: it is unclassified. A call
                // site that allows the write leaves every run legal, and the
                // write keeps its own (apply) role.
                let first_run = semantic_write_execution_role_within(
                    file,
                    call,
                    allowed,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    visiting,
                );
                match first_run {
                    ExecutionRole::DiscardedRendering => {
                        return role_arm!("write_roles.first-run-discarded", first_run);
                    }
                    ExecutionRole::Unknown | ExecutionRole::Unreported => {
                        return role_arm!("write_roles.first-run-unplaced", first_run);
                    }
                    // ADR 0503: a first run proven synchronous throws there,
                    // when nothing in the apply runs before the write (an
                    // earlier call, a pending read among them, may throw
                    // first; ADR 0508 review).
                    role if role.reports_disallowed_write() => {
                        return if first_run_proven_at_call(file, call, lookup)
                            && apply_runs_nothing_before(file, call, span, lookup)
                        {
                            role_arm!("write_roles.first-run-reporting", role)
                        } else {
                            role_arm!("write_roles.first-run-unproven", ExecutionRole::Unknown)
                        };
                    }
                    // ADR 0613: a later run is the drain's, which a
                    // synchronous flush runs under its caller's owner.
                    _ if file.ast.call_at(call).is_some_and(|at| {
                        lookup
                            .primitive_at_call(file, at.span)
                            .is_some_and(|primitive| {
                                (0..at.arguments.len())
                                    .any(|index| drained_under_owner(primitive, index, lookup))
                            })
                    }) =>
                    {
                        *decided = true;
                        return role_arm!(
                            "write_roles.first-run-later-drained",
                            ExecutionRole::Unknown
                        );
                    }
                    _ => break,
                }
            }
            None => {
                if !visit_contains(visiting, &lexical_only_marker())
                    && let Some(role) = named_callback_write_role(
                        file,
                        span,
                        allowed,
                        entities,
                        symbol_names,
                        lookup,
                        root_body,
                        visiting,
                    )
                {
                    if role.reports_disallowed_write()
                        && let Some(suspended) = suspended_write_role(file, span, lookup)
                    {
                        return role_arm!("write_roles.walk-suspended-1", suspended);
                    }
                    // A call a disposal leaves unplaced is final: no other
                    // entry of the function places what its owner decides.
                    if role == ExecutionRole::Unknown {
                        *decided = true;
                    }
                    return role_arm!("write_roles.walk-role-3", role);
                }
                break;
            }
        }
    }
    // ADR 0521: a generator stepped at its own call runs its first step on
    // the stack of that call, under that call's owner, so the write is as
    // legal as one written at the call: a root in a handler rejects it, the
    // handler itself does not. The write walk places the call (the read walk
    // knows no write adjustment, so a root between would be skipped).
    if let Some(step) = stepped_generator_in_place(file, span, allowed) {
        let role = semantic_write_execution_role_within(
            file,
            step.created,
            allowed,
            entities,
            symbol_names,
            lookup,
            root_body,
            visiting,
        );
        if role.reports_disallowed_write() {
            // The step must be a `next` that reaches the write: a `return`
            // or `throw` on a fresh generator runs none of the body, and a
            // write a `yield` may precede runs on no step there is.
            return match step.first_step {
                StepReach::Reaches => role_arm!("write_roles.stepped-generator-reaches", role),
                StepReach::Never => {
                    role_arm!(
                        "write_roles.stepped-generator-never",
                        ExecutionRole::Unreported
                    )
                }
                StepReach::Unproven => {
                    role_arm!(
                        "write_roles.stepped-generator-unproven",
                        ExecutionRole::Unknown
                    )
                }
            };
        }
        // Legal where the call is, or never run: legal. Unplaced stays so;
        // the read walk's answer at the call would claim more than the
        // write's own adjustments allow.
        return role_arm!("write_roles.stepped-generator", role);
    }
    // ADR 0315: write legality keeps the role it had before the use-site arm;
    // that arm places only reads.
    let direct = semantic_execution_role_within(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        &mut HashSet::from_iter([write_classification_marker()]),
    );
    // A proven non-reporting mix walks on exactly as an unknown role does;
    // only where the walk ends without a role does it answer `Unreported`
    // rather than `Unknown`.
    let unreported = direct == ExecutionRole::Unreported;
    let direct = if unreported {
        ExecutionRole::Unknown
    } else {
        direct
    };
    let unknown = if unreported {
        ExecutionRole::Unreported
    } else {
        ExecutionRole::Unknown
    };
    // ADR 0460: the deferred role a control-flow component's children callback
    // gives every function nested in it is no proof for a write. A function
    // the callback's code runs in place (an IIFE, a helper calling it) runs
    // where that code does; any other is placed by its own entries.
    let direct = if direct == ExecutionRole::DeferredCallback {
        match control_flow_nested_layers(file, span, entities, symbol_names, lookup) {
            Some(ControlFlowLayers::InPlace(at, among_others)) => {
                let role = semantic_write_execution_role_within(
                    file,
                    at,
                    allowed,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    visiting,
                );
                return if among_others && !role.reports_disallowed_write() {
                    unknown
                } else {
                    role
                };
            }
            Some(ControlFlowLayers::Unrun) => ExecutionRole::Unknown,
            None => direct,
        }
    } else {
        direct
    };
    // Directly in a component body the dev owner is the component's root, so
    // a guard that exempts roots exempts the body too. Only the body itself:
    // tracked JSX in it runs in a render effect, and a callback nested in it
    // is judged as itself.
    if direct == ExecutionRole::UntrackedRendering
        && root_body == RootBodyGuard::Exempts
        && runs_directly_in_component_root(file, span, lookup)
    {
        return role_arm!(
            "write_roles.write_role_walk.untracked-callback-3",
            ExecutionRole::UntrackedCallback
        );
    }
    // The rendering role places code by where it is written. For a write in a
    // function literal nested in the body that is a proof only when the
    // literal provably runs during the body; otherwise the write's role is
    // what its invocation sites prove, which the walk below derives.
    let direct = if direct == ExecutionRole::UntrackedRendering
        && !nested_literal_runs_during_body(file, span, entities, symbol_names, lookup)
    {
        ExecutionRole::Unknown
    } else {
        direct
    };
    // A function literal that is a JSX attribute's value -- an event handler,
    // a callback prop -- is handed to the element; constructing it runs
    // nothing, and whatever expression wraps the JSX (`cond && <el onClick=… />`,
    // a ternary, a `.map` callback, an argument to a helper call) does not
    // change that. The tracked or rendering role such a wrapper gives the
    // *expression* is not a proof about the function inside it.
    if direct.reports_disallowed_write() && enclosed_by_jsx_attribute_function(file, span) {
        return role_arm!("write_roles.walk-unknown-1", unknown);
    }
    // A tracked role reaches every closure written inside the tracked region
    // or callback, including ones only stored there (`createMemo(() => ({ go:
    // () => set(1) }))`). The role is a proof for the closure only if the
    // closure runs inline in the scope that earned it.
    if direct == ExecutionRole::TrackedJsx
        && enclosed_by_stored_function(file, span, entities, symbol_names, lookup)
    {
        return role_arm!("write_roles.walk-unknown-2", unknown);
    }
    // The same holds for a closure bound to a name inside the tracked scope
    // (ADR 0452): the scope runs its construction, not its body. The closure's
    // own invocations decide, through the walk below.
    let tracked_named = if direct == ExecutionRole::TrackedJsx {
        named_closure_in_tracked_scope(file, span, allowed, entities, symbol_names, lookup)
    } else if direct == ExecutionRole::DeferredCallback {
        // ADR 0455: the same for a closure bound to a name inside a callback that
        // keeps its call's owner (`untrack`): the lexical deferred role is no
        // proof for a closure that runs where it is called.
        named_closure_in_owner_keeping_callback(file, span, lookup)
    } else {
        None
    };
    // ADR 0471: the same for a closure bound to a name in a created root's body
    // that a cleanup, an effect apply or a handler holds: their permissive role
    // is no proof for a closure that runs under the root where it is called.
    // And for a site a function runs in place that a primitive names by its
    // binding as a root's body in a position the by-name climb did not admit:
    // the site's entries decide. Only a guard that rejects in a root asks.
    let permissive_around_root = root_body != RootBodyGuard::Exempts
        && matches!(
            direct,
            ExecutionRole::DeferredCallback
                | ExecutionRole::UntrackedCallback
                | ExecutionRole::UntrackedUnwindowed
                | ExecutionRole::EffectApply
                | ExecutionRole::EventCallback
        );
    let tracked_named = tracked_named.or_else(|| {
        permissive_around_root
            .then(|| named_closure_in_root_callback(file, span, lookup))
            .flatten()
    });
    let tracked_named_or_root = tracked_named.is_some()
        || (permissive_around_root
            && (in_place_in_root_named_callback(file, span, lookup)
                || handed_closure_in_owner_callback(file, span, lookup)));
    let direct = if tracked_named_or_root {
        ExecutionRole::Unknown
    } else {
        direct
    };
    // ADR 0455: an anonymous function stored or handed on inside such a
    // callback runs where its holder calls it, not in the callback's pass.
    if direct == ExecutionRole::DeferredCallback
        && unrun_layer_in_owner_keeping_callback(file, span, lookup)
    {
        return role_arm!(
            "write_roles.write_role_walk.unknown-6",
            ExecutionRole::Unknown
        );
    }
    // Past an await that dominates it, the write runs on a later task with no
    // owner, whatever its lexical role or its function's callers: legal, and
    // proven so, though in no single role.
    if follows_dominating_await(file, span) || follows_try_head_await(file, span, lookup) {
        return role_arm!(
            "write_roles.write_role_walk.unreported",
            ExecutionRole::Unreported
        );
    }
    // ADR 0611: the same past an await of a function the write's own function
    // runs in place inside (an IIFE, a helper calling its callback, a named
    // closure's one call, an inline standard-library callback, each on its
    // caller's stack and owner): `await p; (() => set(1))()` in an async memo
    // compute writes on the later task too, though the lexical role is the
    // compute's.
    let layered_suspension = suspension_through_in_place_layers(file, span, lookup);
    if layered_suspension == Some(true) {
        return role_arm!(
            "write_roles.write_role_walk.unreported-layered",
            ExecutionRole::Unreported
        );
    }
    // ADR 0851: the same past a `yield` of a generator its async caller steps
    // across one of that caller's suspensions (`for await`, or a loop that
    // awaits): the step that runs the write is called from the continuation.
    let generator_resumed = generator_step_resumption(file, span, lookup);
    if generator_resumed == Some(true) {
        return role_arm!(
            "write_roles.write_role_walk.unreported-generator",
            ExecutionRole::Unreported
        );
    }
    // Code after an `await` in an async function runs on a later task, with no
    // owner; neither the lexical role nor the role at the function's call sites
    // describes it. Not proven to follow an await on every path (an `await`
    // inside a branch, a `catch` or `finally` whose `try` may throw before it
    // suspends), the write runs either on the stack its function was entered
    // on, in the role its entries give it, or past the await with no owner
    // (Solid's write guard reads the ambient owner, which every entry point
    // restores before a promise job can run). A rejecting role is no claim
    // then; a permissive one is the write's on both paths, so the walk below
    // places it (`suspendable`). An implicit suspension (`for await`) is the
    // same, and is not proven to run at all.
    // ADR 0611: a layer run in place past a possible await of its caller's
    // function is the same; and whatever role a position names (a named
    // tracked compute, an effect compute, the entries), only a permissive one
    // holds past a possible suspension.
    let suspendable = may_follow_suspension(file, span)
        || layered_suspension == Some(false)
        || generator_resumed == Some(false);
    if direct.reports_disallowed_write() && suspendable {
        return role_arm!("write_roles.walk-unknown-3", unknown);
    }
    // ADR 0853: a named generator handed on as a value has unplaced steps.
    // Its creation calls are not body entries, even if the lexical compute
    // would reject a write. Primitive-stepped and inline generators keep
    // their existing dedicated paths.
    if let Some(generator) = crate::owners::containing_ast_function(&file.ast, span)
        .filter(|function| function.generator && function.body.contains(span))
        && crate::owners::function_binding_name(file, generator).is_some()
        && crate::owners::generator_creations(file, generator, lookup).is_none()
        && crate::owners::stepped_action_generator(file, generator, lookup).is_none()
        && !allowed
            .iter()
            .any(|region| file.ast.peel_ts_sugar_span(*region) == generator.span)
    {
        *decided = true;
        return role_arm!("write_roles.generator-steps-handed", ExecutionRole::Unknown);
    }
    // ADR 0851: a generator's entries are its steps, which the role at the
    // site already took (`callee_roles.generator-steps`); a call of it runs
    // none of its body, so the walk over its callers below does not apply.
    if let Some(generator) = crate::owners::containing_ast_function(&file.ast, span)
        .filter(|function| function.generator && function.body.contains(span))
        && let Some(calls) = crate::owners::generator_creations(file, generator, lookup)
    {
        // The write walk at each step site, which knows the write
        // adjustments the read role at the site does not: one role where
        // every step agrees, none where they differ or a step is unplaced.
        let key = (file.path.to_string(), generator.span);
        // A step runs the site on its own stack only up to the generator's
        // first suspension of its own (a `yield`, or an `await` of an async
        // one), unless a sync generator is drained in place by every
        // creation (a `for…of`, spread, pattern or `Array.from`), whose loop
        // takes each later step on the same stack.
        let own = |point: Span| {
            generator.body.contains(point)
                && crate::owners::containing_ast_function(&file.ast, point)
                    .is_some_and(|owner| owner.span == generator.span)
        };
        let suspensions = file.ast.yields.iter().map(|fact| fact.span).chain(
            generator
                .r#async
                .then(|| {
                    file.ast
                        .awaits
                        .iter()
                        .chain(&file.ast.implicit_suspensions)
                        .copied()
                })
                .into_iter()
                .flatten(),
        );
        let first_segment = !suspensions.filter(|point| own(*point)).any(|point| {
            point.start < span.start
                || file.ast.loop_repeats.iter().any(|&(looped, repeated)| {
                    repeated.contains(span)
                        && looped.contains(point)
                        && generator.body.contains(looped)
                })
        });
        let drained = !generator.r#async
            && calls.iter().all(|call| {
                file.ast.iterated_operands.contains(&call.span)
                    || crate::owners::array_from_source(file, call.span, lookup)
            });
        if (first_segment || drained)
            && !crate::owners::generator_exported(file, generator)
            && visiting.insert(key.clone())
        {
            let mut sites = Vec::new();
            let mut unplaced = false;
            for call in calls {
                match crate::owners::iterator_steps(file, call, lookup) {
                    crate::owners::IteratorSteps::InPlace => sites.push(call.span),
                    crate::owners::IteratorSteps::Held(steps) => sites.extend(steps),
                    crate::owners::IteratorSteps::Unplaced => unplaced = true,
                }
            }
            let mut role: Option<ExecutionRole> = None;
            let mut differ = unplaced;
            for site in sites {
                if differ {
                    break;
                }
                let at = semantic_write_execution_role_within(
                    file,
                    site,
                    allowed,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    visiting,
                );
                differ |= role.is_some_and(|role| role != at);
                role = Some(at);
            }
            visiting.remove(&key);
            match role {
                // A step that may resume on the continuation (ADR 0851) holds
                // only a permissive role there, as a possible await does.
                Some(role)
                    if !differ
                        && role != ExecutionRole::Unknown
                        && !(generator_resumed == Some(false)
                            && role.reports_disallowed_write()) =>
                {
                    return role_arm!("write_roles.generator-steps-walked", role);
                }
                None if !differ => {
                    return role_arm!(
                        "write_roles.generator-never-stepped",
                        ExecutionRole::Unreported
                    );
                }
                _ => {}
            }
        }
        return if direct == ExecutionRole::Unknown {
            role_arm!("write_roles.generator-steps-unknown", unknown)
        } else {
            role_arm!("write_roles.generator-steps", direct)
        };
    }
    // A site in a parameter list (a default value, a pattern, a function written
    // inside either) runs when its function is called without the argument:
    // for a closure bound to a name, whatever its calls omit; for an anonymous
    // callback, whatever the code that invokes it supplies. The function around
    // it (the body that holds the list) does not run it, and nothing here
    // proves an omission: a closure bound to a name has no call sites (its
    // parameter list is not inert, ADR 0339), a lexical tracked or rendering
    // role would claim a run the function's own calls decide, and an invoker
    // such as `forEach` supplies the argument. Nothing places the write (ADR
    // 0452, 0454).
    if functions_around(file, span)
        .into_iter()
        .any(|function| parameter_slot(function, span).is_some())
    {
        return role_arm!("write_roles.walk-unknown-4", unknown);
    }
    // ADR 0456: a disposal callback whose handle the code calls also runs at
    // those calls, outside the disposal, so its lexical role is no proof there.
    let direct = if direct.reports_disallowed_write() {
        direct
    } else {
        match cleanup_handle_write_role(
            file,
            span,
            allowed,
            entities,
            symbol_names,
            lookup,
            root_body,
            visiting,
        ) {
            Some(HandleRole::Decided(role)) => {
                let role = past_possible_suspension(role, suspendable, unknown);
                if role == ExecutionRole::Unknown {
                    *decided = true;
                }
                return role_arm!("write_roles.walk-role-4", role);
            }
            // The entries of the layer decide, but none of them places a write
            // that follows a suspension (as above).
            Some(HandleRole::Unrun) if may_follow_suspension(file, span) => {
                *decided = true;
                return role_arm!("write_roles.walk-unknown-5", unknown);
            }
            Some(HandleRole::Unrun) => ExecutionRole::Unknown,
            None => direct,
        }
    };
    // ADR 0466: an effect's compute named by a binding runs in a computation
    // the effect owns, as the same compute written inline does: a write it runs
    // in place is rejected there, whatever the function's other entries do.
    if !direct.reports_disallowed_write() && named_effect_compute_runs(file, span, lookup) {
        if suspendable {
            return role_arm!("write_roles.walk-suspended-compute", unknown);
        }
        return role_arm!(
            "write_roles.write_role_walk.tracked-jsx",
            ExecutionRole::TrackedJsx
        );
    }
    // ADR 0782: a component a cleanup mounts writes inside that disposal.
    // Only for a guard that stands down there: `action` and `onFlush` test
    // the owner alone and never read the disposal depth (`Rejects`), so a
    // component a cleanup mounts calls them in an owned scope (Chrome).
    if direct == ExecutionRole::UntrackedRendering && root_body != RootBodyGuard::Rejects {
        use solid_dialect::WriteGuardInDisposal as Guard;
        let guard = lookup.dialect.write_guard_in_disposal();
        let reach = (guard != Guard::Rejects)
            .then(|| {
                // The innermost: a component written in another's body mounts
                // where its own tags say.
                functions_around(file, span)
                    .into_iter()
                    .filter(|function| {
                        lookup.function_component_status(file, function) == ComponentStatus::Proven
                    })
                    .min_by_key(|function| function.span.end - function.span.start)
                    .map(|component| component_mount_reach(file, component, lookup, 0))
            })
            .flatten();
        match reach {
            Some(DisposalReach::Only) if guard == Guard::StandsDown => {
                return role_arm!(
                    "write_roles.component-mounted-in-disposal",
                    ExecutionRole::UntrackedCallback
                );
            }
            Some(DisposalReach::Only | DisposalReach::Unplaced) => {
                *decided = true;
                return role_arm!("write_roles.component-mount-unplaced", unknown);
            }
            Some(DisposalReach::NotIn) | None => {}
        }
    }
    let innermost = crate::owners::containing_ast_function(&file.ast, span);
    // A permissive role read off the positions that name the function as a
    // callback (`onClick={reset}`) describes those entries only. Its calls and
    // every other use of it are entries too, so it is kept only as one entry of
    // the walk below. For a closure nested in the named callback it says
    // something only when every layer between them runs inline where it is
    // written; a closure the callback stores or hands on is entered by its own
    // entries.
    let named_callback = (direct != ExecutionRole::Unknown && !direct.reports_disallowed_write())
        .then(|| named_callback_containing(file, span, lookup))
        .flatten()
        .filter(|_| {
            super::named_callbacks::named_callback_write_execution_role(file, span, lookup)
                == Some(direct)
        });
    let named = named_callback.filter(|callback| {
        innermost.is_some_and(|innermost| runs_inline_within(file, innermost, callback, lookup))
    });
    // ADR 0331: in a function its calls enter but some unfollowed entry may
    // render, the rendering role is only a possibility. A call that proves a
    // disallowed write still proves it; otherwise no role is proven.
    let possible_render = direct == ExecutionRole::UntrackedRendering
        && innermost_identity_is_possible(file, span, lookup);
    if direct != ExecutionRole::Unknown && named_callback.is_none() && !possible_render {
        // Past a possible suspension only a permissive role holds on both paths.
        return role_arm!(
            "write_roles.walk-direct-1",
            past_possible_suspension(direct, suspendable, unknown)
        );
    }
    let Some(innermost) = innermost else {
        return role_arm!("write_roles.walk-unknown-6", unknown);
    };
    // ADR 0452: the named closure built in a tracked scope is entered by its
    // calls, and the write runs with it only through layers that run in place
    // (an IIFE, an inline standard-library callback). Any other layer keeps or
    // schedules the write's function, whose entries are not the closure's.
    let entry = match tracked_named {
        Some(closure) if closure.span == innermost.span => Some(closure),
        Some(closure) if runs_in_place_within(file, innermost, closure, lookup) => Some(closure),
        Some(_) => return role_arm!("write_roles.walk-unknown-entry", unknown),
        None => None,
    };
    // ADR 0327: every invocation of the writing function, followed forward
    // through the closed program, is an event delivery. No other entry exists.
    if event_only_entries(file, entry.unwrap_or(innermost), lookup) {
        return role_arm!(
            "write_roles.write_role_walk.event-callback",
            ExecutionRole::EventCallback
        );
    }
    if visit_contains(visiting, &lexical_only_marker()) {
        return role_arm!("write_roles.walk-unknown-7", unknown);
    }
    let function = named.or(entry).unwrap_or(innermost);
    // The write is in a closure nested in the named callback: a call of the
    // callback does not run it, so a disallowed entry proves nothing here.
    // ADR 0466: unless every layer between them runs in place on the
    // callback's stack (an IIFE, an inline standard-library callback, a helper
    // calling it in its own body), as the same layers do in an inline callback:
    // then each run of the callback runs the write, under the callback's owner.
    let nested = entry.is_none()
        && function.span != innermost.span
        && !runs_in_place_within(file, innermost, function, lookup);
    // A binding that may hold another function (`let reset = …; reset = …`)
    // names more than this one at its positions and calls, so none of them is
    // proof that this function runs in an owned scope.
    let current = lookup.function_value_is_current(file, function)
        || function_binding_name(file, function).is_none();
    // A tracked position (`createMemo(reset)`) runs the callback in an owned
    // computation, whichever role the other positions name it in. A leaf
    // position (`createTrackedEffect(reset)`) is tracked too, but its owner
    // is one the write guard exempts (`NamedCallbackRole::leaf`): it is one
    // permissive entry, and the walk below places the rest.
    if named.is_some()
        && !nested
        && lookup
            .named_callback_roles(file)
            .get(function.span)
            .tracked_owned
    {
        return if current && !suspendable {
            role_arm!("write_roles.named-tracked-owned", ExecutionRole::TrackedJsx)
        } else {
            unknown
        };
    }
    // ADR 0516: the permissive role a named position gives is that
    // position's alone. A binding that may hold another function proves
    // nothing about this one; and where the same literal written inline is
    // adjusted to its call site -- a render effect's apply, whose first run is
    // unproven (ADR 0460), and an untracked callback, which runs at the call
    // (ADR 0466) -- a function named at several positions has no one call
    // site to adjust to. Neither is a legal write.
    if named.is_some() && !nested {
        let role = lookup.named_callback_roles(file).get(function.span);
        let several = lookup
            .named_callback_roles(file)
            .positions(function.span)
            .len()
            > 1;
        let adjusted_inline = role.untracked_callback
            || role.effect_apply_primitive.single() == Some(Primitive::CreateRenderEffect)
            || matches!(role.effect_apply_primitive, EffectApplyPrimitive::Several);
        // ADR 0517: an owner-keeping position (`untrack(cb)`) gives a read
        // fact, not a write one: the write there is as legal as the call,
        // which only `named_callback_write_role` judges, for rejections. With
        // several such positions and no rejection found, nothing proved the
        // write legal.
        let owner_keeping =
            primitive_positions(file, function, lookup)
                .into_iter()
                .any(|(primitive, _)| {
                    lookup
                        .dialect
                        .callback_preserves_owner_write_context(primitive)
                });
        let distinct = lookup
            .named_callback_roles(file)
            .positions(function.span)
            .iter()
            .collect::<HashSet<_>>()
            .len();
        if !current || (several && adjusted_inline) || (distinct > 1 && owner_keeping) {
            return role_arm!("write_roles.walk-unknown-8", unknown);
        }
    }
    let loop_plan = LoopPlan {
        function,
        direct,
        named: named.is_some(),
        nested,
        current,
        possible_render,
        unknown,
        suspendable,
    };
    if let Some(plan) = plan {
        *plan = Some(loop_plan);
        return unknown;
    }
    caller_loop(
        file,
        loop_plan,
        entities,
        symbol_names,
        lookup,
        root_body,
        visiting,
    )
}

/// Where a write's walk goes when its position proves nothing: the function
/// whose entries decide (its call sites, followed by [`caller_loop`]) and
/// how the walk folds them.
#[derive(Clone, Copy)]
pub(super) struct LoopPlan<'f> {
    pub(super) function: &'f solid_facts::ast::FunctionFact,
    /// The role the position itself gave, one entry when `named`.
    pub(super) direct: ExecutionRole,
    /// The function is a named callback whose positions gave `direct`.
    pub(super) named: bool,
    /// The write is in a closure nested in the function, which a call of it
    /// does not run: a rejecting entry proves nothing.
    pub(super) nested: bool,
    /// The function's binding holds this function only.
    pub(super) current: bool,
    /// A rendering role is only a possibility here (ADR 0331).
    pub(super) possible_render: bool,
    /// What an unplaced walk answers: `Unknown`, or `Unreported` when the
    /// position proved a non-reporting mix.
    pub(super) unknown: ExecutionRole,
    /// The write may follow a suspension in its own function: its entries'
    /// role holds only on the path that does not suspend, so only a
    /// permissive one is the write's ([`past_possible_suspension`]).
    pub(super) suspendable: bool,
}

impl LoopPlan<'_> {
    /// Whether the loop answers exactly what its entries fold to: a hit
    /// as itself, an unplaced fold as `Unknown`, a placed one as its first
    /// role (or `Unknown` without a complete census).
    pub(super) fn plain(&self) -> bool {
        !self.named
            && !self.nested
            && self.current
            && !self.possible_render
            && self.unknown == ExecutionRole::Unknown
            && !self.suspendable
    }
}

/// The role of a write that may follow a suspension in its own function
/// (`suspendable`), given `role`, the role on the path that does not suspend:
/// the other path runs on a later task with no owner, where the write is
/// legal. So a permissive role holds on both paths, and a rejecting or
/// unplaced one proves nothing (`unknown`).
pub(super) fn past_possible_suspension(
    role: ExecutionRole,
    suspendable: bool,
    unknown: ExecutionRole,
) -> ExecutionRole {
    if suspendable && (role == ExecutionRole::Unknown || role.reports_disallowed_write()) {
        unknown
    } else {
        role
    }
}

/// What the entries of a function fold to ([`caller_loop`]): the first
/// entry that rejects the write, or the first role and whether an entry
/// left it unplaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CallerFold {
    Rejected(ExecutionRole),
    Entries {
        imperative: Option<ExecutionRole>,
        unplaced: bool,
    },
}

/// The role a write takes from the entries of the function `plan` names: a
/// call written in a function this walk is already deriving is that
/// function's business; every other call is placed by its own walk.
#[allow(clippy::too_many_arguments)]
pub(super) fn caller_loop(
    file: &solid_facts::FileFacts,
    plan: LoopPlan<'_>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    visiting: &mut HashSet<(String, Span)>,
) -> ExecutionRole {
    let LoopPlan {
        function,
        direct,
        named,
        nested,
        current,
        possible_render,
        unknown,
        suspendable,
    } = plan;
    let key = (file.path.to_string(), function.span);
    if !visit_insert(visiting, key.clone()) {
        return role_arm!("write_roles.walk-unknown-9", unknown);
    }
    let imperative = named.then_some(direct);
    let folded = super::write_components::region_fold(
        file,
        function,
        imperative,
        entities,
        symbol_names,
        lookup,
        root_body,
        visiting,
    )
    .unwrap_or_else(|| {
        path_fold(
            file,
            function,
            imperative,
            entities,
            symbol_names,
            lookup,
            root_body,
            visiting,
        )
    });
    visiting.remove(&key);
    let (imperative, unplaced) = match folded {
        CallerFold::Rejected(role) => {
            return if nested || !current || suspendable {
                unknown
            } else {
                role
            };
        }
        CallerFold::Entries {
            imperative,
            unplaced,
        } => (imperative, unplaced),
    };
    if possible_render {
        return role_arm!("write_roles.walk-unknown-10", unknown);
    }
    let Some(role) = imperative.filter(|_| !unplaced) else {
        return role_arm!("write_roles.walk-unknown-11", unknown);
    };
    // A permissive role is inherited only from a complete census of the
    // function's entries: a use the walk did not follow (handed to a child
    // component, stored, aliased, exported) may run it in an owned scope.
    if entry_census_complete(file, function, lookup) || event_only_entries(file, function, lookup) {
        past_possible_suspension(role, suspendable, unknown)
    } else {
        unknown
    }
}

/// The reference fold of [`caller_loop`]: every entry of `function` in
/// order, each placed by its own walk, which follows every path through the
/// callers that revisits no function on it.
#[allow(clippy::too_many_arguments)]
fn path_fold(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    mut imperative: Option<ExecutionRole>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    visiting: &mut HashSet<(String, Span)>,
) -> CallerFold {
    let mut unplaced = false;
    for (caller_file, callee) in lookup.function_call_sites(file.path.as_str(), function.span) {
        lookup.count_write_walk_steps(1);
        // ADR 0560: past its bound the walk unwinds; it answers `Unknown`
        // ([`super::write_components::bounded_walk`]).
        if !super::write_components::walk_step() {
            return CallerFold::Entries {
                imperative: None,
                unplaced: true,
            };
        }
        // A call written directly in a function this walk is already deriving
        // (the function itself, or one of its callers) runs in that
        // function's role, which its other entries decide -- when it is a
        // plain call outside JSX. A JSX render gives the body a fresh
        // component owner, and JSX the compiler wraps runs in a render
        // effect: such a re-entry has a role of its own the walk cannot
        // derive here.
        if let Some(caller) = crate::owners::containing_body_function(&caller_file.ast, callee)
            .filter(|caller| visit_contains(visiting, &(caller_file.path.to_string(), caller.span)))
        {
            if !plain_call_outside_jsx(caller_file, caller, callee) {
                unplaced = true;
            }
            continue;
        }
        let caller_allowed = write_allowed_spans(
            caller_file,
            &allowed_callback_spans(caller_file, lookup),
            lookup,
        );
        let role = semantic_write_execution_role_within(
            caller_file,
            callee,
            &caller_allowed,
            entities,
            symbol_names,
            lookup,
            root_body,
            visiting,
        );
        if role.reports_disallowed_write() {
            return CallerFold::Rejected(role);
        }
        if role == ExecutionRole::Unknown {
            // An entry no fact places leaves the function's role unplaced,
            // whatever its other entries prove.
            unplaced = true;
        } else {
            imperative.get_or_insert(role);
        }
    }
    CallerFold::Entries {
        imperative,
        unplaced,
    }
}

/// Whether `span` runs in place ([`runs_in_place_within`]) in a function an
/// effect names, by its binding, as its tracked compute
/// ([`NamedCallbackRole::effect_compute`]): a function that holds that binding's
/// value now and is not a possible component.
fn named_effect_compute_runs(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(innermost) = crate::owners::containing_ast_function(&file.ast, span) else {
        return false;
    };
    if !innermost.body.contains(span) {
        return false;
    }
    let roles = lookup.named_callback_roles(file);
    file.ast
        .functions_body_containing(span)
        .find(|function| roles.get(function.span).effect_compute)
        .is_some_and(|compute| {
            lookup.function_value_is_current(file, compute)
                && lookup.function_component_status(file, compute) == ComponentStatus::No
                && runs_in_place_within(file, innermost, compute, lookup)
        })
}

/// Whether every way of entering `function` is one the write-role walk sees: a
/// call site [`SemanticLookup::function_call_sites`] resolves to it, or a
/// position of its own file the named-callback index classifies
/// ([`NamedCallbackRoles::positions`]). Every compiler reference to its
/// binding must be one of those, its declaration, or inside a type query, and
/// the census must be bounded at all: a method is entered by member access
/// through any receiver holding it, and an export by modules this build may
/// not see (an open program, or a module whose namespace escapes unnamed).
pub(crate) fn entry_census_complete(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    entry_census_complete_except(file, function, lookup, &[])
}

/// [`entry_census_complete`], with the same-file references at `accounted`
/// taken as entries the caller accounts for itself (ADR 0868: a run inline
/// on a carrier whose own placement the caller checks).
pub(crate) fn entry_census_complete_except(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    accounted_here: &[Span],
) -> bool {
    if function.method_name.is_some() {
        return false;
    }
    let path = file.path.as_str();
    // A named function expression is entered through the binding holding it
    // as well as through its own name; with no such binding it is a value
    // handed wherever it is written.
    let mut bindings = Vec::new();
    if function.kind == solid_facts::ast::FunctionKind::Expression
        && let Some(own) = function.name.as_ref()
    {
        let Some(outer) = file
            .ast
            .bindings_initializer_containing(function.span)
            .find(|binding| {
                binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.initializer.is_some_and(|initializer| {
                        file.ast.peel_ts_sugar_span(initializer) == function.span
                    })
            })
            .and_then(|binding| binding.names.first())
        else {
            return false;
        };
        bindings.extend([own, outer]);
    } else {
        let Some(binding) = function_binding_name(file, function) else {
            return false;
        };
        bindings.push(binding);
    }
    let mut symbols = Vec::new();
    for binding in &bindings {
        let Some(symbol) = lookup.entities().at(path, binding.span) else {
            return false;
        };
        let Some(references) = lookup.symbol_references_if_present(symbol.as_str()) else {
            return false;
        };
        symbols.push(references);
    }
    if lookup.hands_out_by_name(path, function.span)
        || (module_exports(file, function, lookup)
            && (!lookup.program_closed || lookup.escapes_unnamed(path)))
    {
        return false;
    }
    let accounted: HashSet<(&str, u32, u32)> = lookup
        .function_call_site_references(path, function.span)
        .into_iter()
        .map(|(caller, callee)| (caller.path.as_str(), callee.start, callee.end))
        .collect();
    let roles = lookup.named_callback_roles(file);
    let positions = roles.positions(function.span);
    symbols.iter().flatten().all(|reference| {
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            return false;
        };
        if accounted.contains(&(reference.path.as_ref(), start, end)) {
            return true;
        }
        if reference.path.as_ref() != path {
            return false;
        }
        let at = Span::new(start, end);
        accounted_here.contains(&at)
            || bindings.iter().any(|binding| at == binding.span)
            || file.ast.type_queries.iter().any(|query| query.contains(at))
            || positions.iter().any(|position| {
                position.contains(at)
                    && !file
                        .ast
                        .functions_within(*position)
                        .any(|literal| literal.span.contains(at))
            })
    })
}

/// Whether `inner`, nested in `outer`, runs only where `outer`'s role places
/// it: every function layer between them is an IIFE or an argument a
/// standard-library call runs inline (`[x].forEach(fn)`), up to the first
/// layer a reviewed scheduler runs on a fresh stack (`setTimeout(fn)`), which
/// no owner reaches wherever it was scheduled from.
fn runs_inline_within(
    file: &solid_facts::FileFacts,
    inner: &solid_facts::ast::FunctionFact,
    outer: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    layers_run_inline(file, inner, outer, lookup, true)
}

/// [`runs_inline_within`] without the fresh-stack exception: every layer
/// between them is an IIFE or an argument a standard-library call runs inline,
/// so `inner` runs wherever `outer` does, on its stack and owner.
pub(super) fn runs_in_place_within(
    file: &solid_facts::FileFacts,
    inner: &solid_facts::ast::FunctionFact,
    outer: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    layers_run_inline(file, inner, outer, lookup, false)
}

fn layers_run_inline(
    file: &solid_facts::FileFacts,
    inner: &solid_facts::ast::FunctionFact,
    outer: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    fresh_stack_ok: bool,
) -> bool {
    let mut layer = inner;
    // Named functions that are each other's one call (ADR 0479: `const f1 =
    // () => f2(); const f2 = () => f1();`) lead the climb round a cycle that
    // never reaches `outer`: nothing proves they run there.
    let mut crossed = HashSet::default();
    while layer.span != outer.span {
        if layer.generator || !outer.span.contains(layer.span) || !crossed.insert(layer.span) {
            return false;
        }
        let call = match layer_run(file, layer, lookup) {
            LayerRun::FreshStack => return fresh_stack_ok,
            LayerRun::InPlace(call) => call,
            LayerRun::Elsewhere | LayerRun::InPlaceAmongOthers(_) | LayerRun::Withheld(_) => {
                return false;
            }
        };
        // The layer runs on its call's stack, so the next layer is the
        // function that owns the call: the enclosing function, or the function
        // whose parameter list the call is written in (ADR 0454: a layer in a
        // default value runs only when that function is called without the
        // argument). For an IIFE, an inline callback and a helper's argument
        // the call is around the layer; a named function's one call (ADR 0479)
        // may be in another function, which must be inside `outer` too.
        if !outer.span.contains(call) {
            return false;
        }
        let Some(next) = crate::owners::owning_function(&file.ast, call) else {
            return false;
        };
        layer = next;
    }
    true
}

/// How a function layer is run by the code around it.
pub(crate) enum LayerRun {
    /// An IIFE, an argument a standard-library call runs inline
    /// (`[x].forEach(fn)`), or one a function calls in its own body
    /// ([`helper_runs_layer_inline`]): it runs on the stack of the call, which
    /// this span is.
    InPlace(Span),
    /// A function that calls the parameter in its own body on every completion
    /// ([`helper_runs_layer_inline`]) and also passes it on, stores it or
    /// closes over it: one run is in place, the others are elsewhere. A
    /// violation needs one run, so it may read the in place call; a legal write
    /// needs every run, so its arms treat this as [`LayerRun::Elsewhere`]
    /// (ADR 0457).
    InPlaceAmongOthers(Span),
    /// An argument a reviewed scheduler runs on a fresh stack (`setTimeout(fn)`),
    /// which no owner reaches wherever it was scheduled from.
    FreshStack,
    /// Anything else: stored, handed on, or run by a call this does not know.
    Elsewhere,
    /// ADR 0874: an argument a call the census withholds would run inline were
    /// it the standard one. It proves no run at all: an impostor may run it
    /// anywhere, or never. Every reader takes this as [`LayerRun::Elsewhere`]
    /// but [`withheld_layer_call`], which only places a row, and a summary
    /// that crosses it undecided ([`in_place_run_site_placing`]). A reader that
    /// matches every arm names this one; one that asks for
    /// [`LayerRun::InPlace`] alone (`if let`, `matches!`) refuses it by that.
    Withheld(Span),
}

/// ADR 0611: whether code at `span` runs past a suspension of a function it
/// runs in place inside ([`climb_in_place_layers`]): `Some(true)` when a
/// layer's call follows an await on every path, `Some(false)` on some path,
/// `None` when none does. Only [`follows_dominating_await`]'s proof counts, not
/// the climb's own `suspended`: code past an await may still run later under
/// an owner (JSX, an instance field), which that proof excludes. A climb cut
/// short proves nothing about the runs it did not follow.
/// ADR 0899: nor does a helper that also hands its callback on, about those
/// other runs; but its in-place run, the one a violation would read, is still
/// past a layer call's await: `Some(false)`, a suspension on some path, so no
/// role that rejects stands (`await p; w(() => set(1))` with a recursive `w`).
pub(crate) fn suspension_through_in_place_layers(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<bool> {
    let mut calls = Vec::new();
    let climb = climb_in_place_layers_recording(file, span, lookup, false, None, &mut calls)?;
    if climb.exhausted {
        return None;
    }
    let mut answer = None;
    for call in calls {
        if follows_dominating_await(file, call) || follows_try_head_await(file, call, lookup) {
            return Some(!climb.among_others);
        }
        if may_follow_suspension(file, call) {
            answer = Some(false);
        }
    }
    answer
}

/// [`suspension_through_in_place_layers`] with `span`'s own function: whether
/// code at `span` runs on a later task than the one its enclosing callback was
/// entered on, with no ambient owner or observer (ADRs 0333, 0611, 0612).
pub(crate) fn past_suspension(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<bool> {
    let layered = suspension_through_in_place_layers(file, span, lookup);
    if layered == Some(true)
        || follows_dominating_await(file, span)
        || follows_try_head_await(file, span, lookup)
    {
        Some(true)
    } else if layered == Some(false) || may_follow_suspension(file, span) {
        Some(false)
    } else {
        None
    }
}

pub(crate) fn layer_run(
    file: &solid_facts::FileFacts,
    layer: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> LayerRun {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    let iife = file.ast.calls_with_peeled_callee(layer.span).next();
    let behavior = file
        .ast
        .arguments_containing(layer.span)
        .filter(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[*index].span) == layer.span
        })
        .find_map(|(call, index)| {
            let resolved = lookup.placing_callee_call(file, call.callee)?;
            resolved
                .declaration
                .as_ref()
                .is_some_and(|declaration| declaration.standard_library)
                .then_some(())?;
            let callability = lookup
                .entity_at(file.path.as_str(), call.arguments[index].span)
                .and_then(|entity| entity.callability);
            crate::runtime_semantics::argument_behavior(resolved, callability, index)
                .map(|behavior| (call, behavior))
        });
    // ADR 0874: a call the census withholds places its inline callback's rows
    // as a standard one would, for one run among others, never the sole run:
    // a site there has no role (`runs_in_withheld_layer`), and a scheduler's
    // fresh stack is no fact. Callers that take a run here as proof ask
    // `standard_call_withheld` themselves (after_await's ADR 0565 arm).
    let withheld = behavior
        .as_ref()
        .is_some_and(|(call, _)| lookup.standard_call_withheld(file, call.callee));
    if withheld {
        return match behavior {
            Some((call, RuntimeArgumentBehavior::InlineCallback)) => LayerRun::Withheld(call.span),
            _ => LayerRun::Elsewhere,
        };
    }
    let behavior = behavior.map(|(call, behavior)| (call.span, behavior));
    match (behavior, iife) {
        (Some((_, RuntimeArgumentBehavior::FreshStackCallback)), _) => LayerRun::FreshStack,
        (Some((call, RuntimeArgumentBehavior::InlineCallback)), _) => LayerRun::InPlace(call),
        (_, Some(call)) => LayerRun::InPlace(call.span),
        _ => match helper_runs_layer_inline(file, layer, lookup) {
            Some((call, ParameterUses::OnlyCalled)) => LayerRun::InPlace(call),
            Some((call, ParameterUses::AlsoElsewhere)) => LayerRun::InPlaceAmongOthers(call),
            None => sole_call_of_named_function(file, layer, lookup)
                .map_or(LayerRun::Elsewhere, LayerRun::InPlace),
        },
    }
}

/// ADR 0874: whether the site at `span` runs only as a function handed to a
/// call the census withholds does: directly in that function, or in layers
/// that run in place in it. A function a primitive runs, or a JSX expression
/// the compiler wraps, between the two keeps its own role: it does not depend
/// on who runs the function around it.
pub(crate) fn runs_in_withheld_layer(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    withheld_layer_call(file, span, lookup).is_some()
}

/// [`runs_in_withheld_layer`]'s withheld call, by span: where the standard
/// carrier would run the site, which only places a row and proves nothing.
pub(crate) fn withheld_layer_call(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    let mut at = span;
    for _ in 0..32 {
        let function = crate::owners::owning_function(&file.ast, at)?;
        if let Some(call) = handed_to_withheld_call(file, function, lookup) {
            return Some(call);
        }
        match layer_run(file, function, lookup) {
            LayerRun::InPlace(call)
            | LayerRun::InPlaceAmongOthers(call)
            | LayerRun::Withheld(call)
                if call != at =>
            {
                if super::callee_roles::own_jsx_compiler_role(file, call).is_some() {
                    return None;
                }
                at = call;
            }
            _ => return None,
        }
    }
    None
}

/// Whether the call nearest around `function` is a withheld one with the
/// function in its arguments (under a condition or an assertion too), or a
/// reference to the function's name is such an argument.
fn handed_to_withheld_call(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    let ast = &file.ast;
    let nearest = |span: Span| {
        ast.arguments_containing(span)
            .min_by_key(|(call, _)| call.span.end - call.span.start)
    };
    if let Some((call, _)) =
        nearest(function.span).filter(|(call, _)| lookup.standard_call_withheld(file, call.callee))
    {
        return Some(call.span);
    }
    let name = crate::owners::function_binding_name(file, function)?;
    ast.references_to(name.span)
        .iter()
        .filter(|reference| **reference != name.span)
        .find_map(|reference| {
            nearest(*reference)
                .filter(|(call, index)| {
                    ast.peel_ts_sugar_span(call.arguments[*index].span) == *reference
                        && lookup.standard_call_withheld(file, call.callee)
                })
                .map(|(call, _)| call.span)
        })
}

/// ADR 0479: the one call that runs a function bound to a name, when it is the
/// function's only run (`const f = () => …; f();`): the binding holds this
/// function at every call (ADR 0221, `function_value_is_current`), the
/// function has exactly one call site (ADR 0448 credits only calls that follow
/// the declarator), it is in this file, and every other reference to the
/// binding is the declaration, so nothing aliases, passes, stores or exports
/// it. Its module neither exports it nor hands it out through a namespace
/// (`module_exports`, `hands_out_by_name`): a consumer outside the analysed
/// files would leave no reference here. A method, a generator, a named
/// function expression (also entered by its own name) and a function with no
/// binding answer `None`, as does a symbol whose references are not known.
fn sole_call_of_named_function(
    file: &solid_facts::FileFacts,
    layer: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    use solid_facts::ast::FunctionKind;
    if layer.method_name.is_some()
        || layer.generator
        || (layer.kind == FunctionKind::Expression && layer.name.is_some())
    {
        return None;
    }
    let binding = function_binding_name(file, layer)?;
    let path = file.path.as_str();
    let symbol = lookup.entities().at(path, binding.span)?;
    if !lookup.function_value_is_current(file, layer)
        || lookup.hands_out_by_name(path, layer.span)
        || module_exports(file, layer, lookup)
    {
        return None;
    }
    let sites = lookup.function_call_sites(path, layer.span);
    let [(caller, callee)] = sites.as_slice() else {
        return None;
    };
    if caller.path.as_str() != path {
        return None;
    }
    let call = file.ast.calls_with_peeled_callee(*callee).next()?;
    let references = lookup.symbol_references_if_present(symbol.as_str())?;
    let accounted = |start: u32, end: u32| {
        (start == binding.span.start && end == binding.span.end)
            || (start == callee.start && end == callee.end)
    };
    let escapes = references.iter().any(|reference| {
        let start = u32::try_from(reference.start_byte).unwrap_or(u32::MAX);
        let end = u32::try_from(reference.end_byte).unwrap_or(u32::MAX);
        reference.path.as_ref() != path || !accounted(start, end)
    });
    (!escapes).then_some(call.span)
}

/// Where a site runs once the function layers around it that run in place
/// ([`layer_run`]) are crossed (ADR 0469).
pub(crate) struct InPlaceClimb {
    /// The bound left outer layers unread: no placement role is proven.
    pub(crate) exhausted: bool,
    /// The call of the outermost layer crossed: the site runs on its stack.
    pub(crate) at: Span,
    /// A layer holds the climbed site in its parameter list (ADR 0454): it
    /// runs only when its call omits the argument.
    pub(crate) in_list: bool,
    /// A helper on the way also hands its callback on or keeps it
    /// ([`LayerRun::InPlaceAmongOthers`]): one run is in place.
    pub(crate) among_others: bool,
    /// A site on the way follows an await in its layer: on every path
    /// (`Some(true)`), or possibly (`Some(false)`).
    pub(crate) suspended: Option<bool>,
    /// ADR 0874: the climb crossed a call the census withholds, which only
    /// [`in_place_run_site_placing`] does: a row it places is undecided.
    pub(crate) withheld: bool,
}

/// ADR 0493 review: whether the climb from `span` to `to` crosses a layer that
/// is an inline callback of a standard-library or dependency call
/// (`[x].forEach(fn)`, `[1].reduce(fn)`): such a callback runs during the
/// call but may run zero times, so a claim that it runs is no proof. An IIFE,
/// a project helper's callback and a function bound to a name and called once
/// run on every completion. `true` where a step cannot be retraced.
pub(crate) fn climb_crosses_library_callback(
    file: &solid_facts::FileFacts,
    span: Span,
    to: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let mut at = span;
    for _ in 0..16 {
        if at == to {
            return false;
        }
        let Some(function) = crate::owners::owning_function(&file.ast, at) else {
            return true;
        };
        let library = file
            .ast
            .arguments_containing(function.span)
            .filter(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
            })
            .any(|(call, _)| {
                lookup
                    .resolved_callee_call(file, call.callee)
                    .and_then(|resolved| resolved.declaration.as_ref())
                    .is_none_or(|declaration| {
                        declaration.standard_library
                            || declaration.source_file.contains("/node_modules/")
                    })
            });
        if library {
            return true;
        }
        at = match layer_run(file, function, lookup) {
            LayerRun::InPlace(call) | LayerRun::InPlaceAmongOthers(call) => call,
            LayerRun::FreshStack | LayerRun::Elsewhere | LayerRun::Withheld(_) => return true,
        };
    }
    true
}

/// ADR 0503: the first run of the effect created at `call` happens before the
/// call returns, under the caller's owner. rc.17's `effect()` runs it on the
/// call unless `defer` or `schedule` is set (ADR 0502), the first pass is born
/// staged or seated in a lane, the compute errors or goes pending, or a
/// boundary above shows its fallback (`@solidjs/signals@2.0.0-rc.17`
/// `dist/dev.js:1851-1869`, `dist/dev-shared.js:4227-4240`, `:4388-4396`,
/// `:4438-4462`, `dist/dev.js:1957`). Proven where:
/// - the call has no options argument;
/// - its compute is a function literal, not async or a generator, that calls,
///   reads, references, awaits, renders and throws nothing, so it settles on
///   its first pass with a value no thenable can be;
/// - the call is on every completion of its component's body
///   (`site_on_every_completion`), and that component is rendered by a
///   module-scope `render` ([`crate::owners::mounted_by_module_render`]): no
///   flush, lane, transition or boundary is above that body.
fn first_run_proven_at_call(
    file: &solid_facts::FileFacts,
    call: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let Some(fact) = ast.calls.iter().find(|candidate| candidate.span == call) else {
        return false;
    };
    let [compute, _apply] = fact.arguments.as_slice() else {
        return false;
    };
    if compute.spread || !compute_settles_synchronously(file, compute.span) {
        return false;
    }
    let Some(component) = crate::owners::containing_ast_function(ast, call) else {
        return false;
    };
    lookup.function_may_be_component(file, component)
        && crate::interproc::site_on_every_completion(lookup, file, component.span, call)
        && crate::owners::mounted_by_module_render(lookup, file, component)
}

/// No call is written in the apply of the effect created at `call` before the
/// position of `span` in it (the outermost function or site around `span` the
/// apply's own body holds); a call around that position is the one that runs
/// it, not one before it.
fn apply_runs_nothing_before(
    file: &solid_facts::FileFacts,
    call: Span,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let Some(apply) = ast
        .call_at(call)
        .and_then(|fact| fact.arguments.get(1))
        .and_then(|argument| ast.function_at(ast.peel_ts_sugar_span(argument.span)))
    else {
        return false;
    };
    let own_calls = || {
        ast.calls.iter().filter(move |other| {
            apply.body.contains(other.span)
                && crate::owners::containing_ast_function(ast, other.span)
                    .is_some_and(|function| function.span == apply.span)
        })
    };
    // A write outside the apply (a closure of the body the apply calls) runs
    // through one of the apply's calls, which one this does not know: only an
    // apply with one call of its own runs nothing before it.
    if !apply.body.contains(span) {
        return own_calls().count() <= 1;
    }
    let mut position = span;
    let mut reached = false;
    for _ in 0..32 {
        match crate::owners::containing_ast_function(ast, position) {
            Some(function) if function.span == apply.span => {
                reached = true;
                break;
            }
            Some(function) => position = function.span,
            None => return false,
        }
    }
    // Past the bound the apply is not reached: no proof (ADR 0532).
    if !reached {
        return false;
    }
    !own_calls().any(|other| other.span.end <= position.start)
        && runs_nothing_before_in_layers(file, span, apply.span, lookup)
}

/// ADR 0510 (a defect of ADRs 0503-0505): nothing the first run calls before
/// `site` may throw first. From `site` up to the apply (`apply`), each
/// function on the way runs no call of its own body written before the
/// position, and its run is in place ([`layer_run`]): an IIFE, an inline
/// standard-library callback, the one call of a named local function, or a
/// project helper that calls the argument before anything else
/// ([`parameter_called_first`], ADR 0495); and the call that runs it
/// evaluates no other call in its callee or arguments, nor the site in its
/// own. Before this,
/// `(() => { JSON.parse("{"); a1(); })()` in an apply was SC5001 (and a write
/// there ADR 0503's), though the run throws a `SyntaxError` first.
pub(crate) fn runs_nothing_before_in_layers(
    file: &solid_facts::FileFacts,
    site: Span,
    apply: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    // The site's own arguments are evaluated before it (and a member among
    // them may throw; the callee's own member is the read).
    let callee = ast
        .call_at(site)
        .map(|call| ast.peel_ts_sugar_span(call.callee));
    if ast
        .calls
        .iter()
        .any(|other| other.span != site && site.contains(other.span))
        || ast.members.iter().any(|member| {
            Some(member.span) != callee && site.contains(member.span) && member.span != site
        })
        || ast
            .computed_members
            .iter()
            .any(|member| site.contains(*member))
        || ast.spreads.iter().any(|spread| site.contains(spread.span))
    {
        return false;
    }
    let mut at = site;
    for _ in 0..16 {
        let Some(function) = crate::owners::containing_ast_function(ast, at) else {
            return false;
        };
        let earlier = ast.calls.iter().any(|other| {
            function.body.contains(other.span)
                && other.span.end <= at.start
                && crate::owners::containing_ast_function(ast, other.span)
                    .is_some_and(|owner| owner.span == function.span)
        });
        // A pattern parameter takes its argument apart first, which may throw.
        let plain_parameters = function
            .parameters
            .iter()
            .all(|parameter| parameter.shape == solid_facts::ast::BindingShape::Identifier);
        if earlier
            || !plain_parameters
            || !throws_nothing_before(
                file,
                function,
                function.span.start,
                at.start,
                BodyAdmits::default(),
                lookup,
            )
        {
            return false;
        }
        if function.span == apply {
            return true;
        }
        let call = match layer_run(file, function, lookup) {
            LayerRun::InPlace(call) | LayerRun::InPlaceAmongOthers(call) => call,
            LayerRun::FreshStack | LayerRun::Elsewhere | LayerRun::Withheld(_) => return false,
        };
        let Some(fact) = ast.call_at(call) else {
            return false;
        };
        // The callee and every argument are evaluated before the call runs
        // the function: a call among them, outside the function, runs first.
        let evaluated_first = ast.calls.iter().any(|other| {
            other.span != fact.span
                && fact.span.contains(other.span)
                && !function.span.contains(other.span)
        });
        let helper_first = match fact
            .arguments
            .iter()
            .position(|argument| ast.peel_ts_sugar_span(argument.span) == function.span)
        {
            Some(index)
                if lookup
                    .resolved_callee_call(file, fact.callee)
                    .and_then(|resolved| resolved.declaration.as_ref())
                    .is_none_or(|declaration| !declaration.standard_library) =>
            {
                parameter_called_first(file, fact, index, lookup)
            }
            _ => true,
        };
        if evaluated_first || !helper_first {
            return false;
        }
        at = call;
    }
    false
}

/// ADR 0504: the read at `site` happens in the first run of a render-effect
/// apply that runs while its async source is still pending, so rc.17's
/// `read()` throws `PENDING_ASYNC_UNTRACKED_READ` before it would warn the
/// strict read (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:5320-5325`,
/// `:5441-5446`). Answers the apply's span. Proven where:
/// - the site is in the apply of a `createRenderEffect`, a function literal,
///   directly or through in-place layers that cross no library callback,
///   parameter list, await or helper that also hands its callback on, and no
///   call in that apply is written before it (an earlier throw ends the run);
/// - the call's first run is on the call ([`first_run_proven_at_call`]: no
///   options, a compute that is one primitive literal, on every completion of
///   a component rendered by a module-scope `render`): it runs at creation
///   (`dist/dev.js:1869`), which a halt does not stop (`schedule` alone checks
///   it, `dist/dev-shared.js:2038-2042`);
/// - the source (`declaration`) is a `const` of that body, declared on every
///   completion before the call, initialized by `createMemo` with one
///   argument, an async function literal: pending from its creation until a
///   microtask (`:4626`, `:3642-3660`), with no `loadingValue` to mask it;
/// - nothing in the body between the source and the call runs code: each call
///   there creates a signal or memo, or calls a local function that calls
///   nothing.
///
/// A user effect's first apply (`createEffect`) is in `render`'s tail flush,
/// which a pending render effect anywhere in the mount (a fragment, a helper's
/// JSX, a module-level effect) holds until the source settles: proven only
/// under [`user_apply_runs_in_mount_flush`] (ADR 0505).
pub(crate) fn pending_apply_first_read(
    file: &solid_facts::FileFacts,
    site: Span,
    declaration: &typefacts::Location,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    pending_apply_first_read_from(file, site, site, declaration, lookup)
}

/// [`pending_apply_first_read`] at `site`, a call that runs the read at
/// `origin` (the read itself, or ADR 0511: a read in a named local function
/// the call runs, `const wrapUse = () => { props.k(); }`): a source the mount
/// hands over is named by the origin's own callee.
pub(crate) fn pending_apply_first_read_from(
    file: &solid_facts::FileFacts,
    site: Span,
    origin: Span,
    declaration: &typefacts::Location,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    use solid_dialect::Primitive;
    let ast = &file.ast;
    let at = match climb_in_place_layers(file, site, lookup) {
        Some(climb)
            if !climb.exhausted
                && !climb.in_list
                && !climb.among_others
                && climb.suspended.is_none()
                && !climb_crosses_library_callback(file, site, climb.at, lookup) =>
        {
            climb.at
        }
        Some(_) => return None,
        None => site,
    };
    let (call, _) = ast.arguments_containing(at).find(|(container, index)| {
        *index == 1 && super::direct_callback_runs(file, container.arguments[*index].span, at)
    })?;
    let apply = ast.function_at(ast.peel_ts_sugar_span(call.arguments.get(1)?.span))?;
    if !apply.body.contains(at)
        || !read_on_every_completion_of_apply(file, site, apply.span, lookup)
        || !matches!(
            lookup.primitive_at_call(file, call.span),
            Some(Primitive::CreateRenderEffect | Primitive::CreateEffect)
        )
        || !first_run_proven_at_call(file, call.span, lookup)
        || ast.calls.iter().any(|other| {
            // A call in a function nested in the apply runs only when that
            // function is called, and that call is the apply's own.
            apply.body.contains(other.span)
                && other.span.end <= at.start
                && crate::owners::containing_ast_function(ast, other.span)
                    .is_some_and(|function| function.span == apply.span)
        })
        || !runs_nothing_before_in_layers(file, site, apply.span, lookup)
    {
        return None;
    }
    let component = crate::owners::containing_ast_function(ast, call.span)?;
    let declared = Span::new(
        u32::try_from(declaration.start_byte).ok()?,
        u32::try_from(declaration.end_byte).ok()?,
    );
    let source_file = lookup.file_by_path(declaration.path.as_ref())?;
    let (binding, memo) = async_memo_at(source_file, declared, lookup)?;
    let own_body = source_file.path == file.path
        && crate::owners::containing_ast_function(ast, binding.declaration)
            .is_some_and(|function| function.span == component.span);
    let ok = if own_body {
        binding.declaration.end <= call.span.start
            && crate::interproc::site_on_every_completion(lookup, file, component.span, memo.span)
            && body_runs_nothing_between(
                file,
                component,
                memo.span.end,
                call.span.start,
                BodyAdmits::default(),
                lookup,
            )
            && (lookup.primitive_at_call(file, call.span) == Some(Primitive::CreateRenderEffect)
                || user_apply_runs_in_mount_flush(file, component, call, memo, lookup))
    } else if crate::indexes::at_module_scope(&source_file.ast, binding.declaration) {
        // ADR 0510: a module-scope memo of a module that only creates.
        module_source_pending(file, component, call, source_file, binding, memo, lookup)
    } else {
        // ADR 0510: a memo the mounting parent hands over; ADR 0511: one an
        // inert project hook the body calls creates.
        parent_source_pending(
            file,
            origin,
            component,
            call,
            source_file,
            binding,
            memo,
            lookup,
        ) || hook_source_pending(
            file,
            origin,
            component,
            call,
            source_file,
            binding,
            memo,
            lookup,
        )
    };
    ok.then_some(apply.span)
}

/// ADR 0505: a user effect's first apply (`createEffect`) runs in the tail
/// flush of the `render` that mounted `component`, while the source is still
/// pending. The apply is queued at the call and run by that flush
/// (`@solidjs/web@2.0.0-rc.17` `dist/web.dev.js:542-568`); a pending render
/// effect anywhere in the flush, or a transaction an earlier pass left, would
/// hold it until the source settles (`@solidjs/signals@2.0.0-rc.17`
/// `dist/dev-shared.js:1690-1733`, `:2180`). Proven where:
/// - the component renders a static tree ([`static_intrinsic_tree`]): no
///   component, fragment, expression, attribute or custom element, so the
///   mount creates no render effect that can go pending;
/// - the body runs nothing else ([`body_runs_nothing_else`]), and the
///   source's async compute is one primitive literal;
/// - a module-scope `render` mounting it is the first code to run that can
///   create a reactive node ([`render_runs_first`]); a parent between them
///   (ADR 0507) runs nothing but that render (`static_child_tag`).
fn user_apply_runs_in_mount_flush(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    effect: &solid_facts::ast::CallFact,
    memo: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    memo_compute_is_literal(file, memo)
        && user_apply_flush_premises(
            file,
            component,
            effect,
            Some(memo),
            BodyAdmits::default(),
            lookup,
        )
        && crate::owners::module_render_sites(lookup, file, component)
            .into_iter()
            .any(|site| render_runs_first(lookup, site.entry, site.render))
}

/// ADR 0510: what a component body may name or call beyond ADR 0505's inert
/// code: the `useContext` of a context its mount provides (`context`, the
/// context's declaration), and the module-scope source's own name (`source`,
/// its import or, in its own module, its declaration), initialized before
/// the mount.
#[derive(Clone, Copy, Default)]
pub(super) struct BodyAdmits {
    pub(super) context: Option<Span>,
    pub(super) source: Option<Span>,
    /// ADR 0511: the call of an inert project hook in the body
    /// ([`super::pending_sources::inert_hook`]): it creates nodes, runs
    /// nothing else and returns an object literal of data properties, so the
    /// call and a destructuring of its result cannot throw.
    pub(super) hook: Option<Span>,
    /// ADR 0511: the declaration of the object the mount hands over (the
    /// one plain props parameter, or the `useContext` result of the provided
    /// context): a named member read of it reads a data property of the
    /// object the proven mount passes, and cannot throw.
    pub(super) handed: Option<Span>,
}

/// ADR 0511: `member` reads a named, non-computed, non-optional member of the
/// handed object ([`BodyAdmits::handed`]) directly.
fn handed_member_read(
    file: &solid_facts::FileFacts,
    member: &solid_facts::ast::MemberFact,
    admits: BodyAdmits,
) -> bool {
    let ast = &file.ast;
    let object = ast.peel_ts_sugar_span(member.object);
    admits.handed.is_some()
        && !ast.computed_members.contains(&member.span)
        && ast.optional_members.binary_search(&member.span).is_err()
        && ast
            .identifiers
            .iter()
            .any(|identifier| identifier.span == object)
        && ast.reference_declaration(object) == admits.handed
}

/// ADR 0505's premises on the component itself, its mount aside: plain
/// parameters, a static tree ([`static_intrinsic_tree`]) and a body that runs
/// nothing else ([`body_runs_nothing_else`]); `memo` is the source's creation
/// when the body writes it, and `provided` a context whose `useContext` the
/// body may call (ADR 0510: the mount provides it).
pub(super) fn user_apply_flush_premises(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    effect: &solid_facts::ast::CallFact,
    memo: Option<&solid_facts::ast::CallFact>,
    admits: BodyAdmits,
    lookup: &SemanticLookup<'_>,
) -> bool {
    plain_component(component)
        && static_intrinsic_tree(file, component, lookup)
        && body_runs_nothing_else(file, component, effect, memo, admits, lookup)
}

/// Every JSX element in `component` is a lowercase intrinsic tag (no custom
/// element, member or namespaced name) written in its own body, with no
/// attribute but string ones, no spread and no expression child but a settled
/// one ([`settled_expression_child`], ADR 0510), no fragment is written there,
/// and every return of the body (or an expression body) is one of those
/// elements.
fn static_intrinsic_tree(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let own = |span: Span| {
        crate::owners::containing_ast_function(ast, span)
            .is_some_and(|function| function.span == component.span)
    };
    let elements = ast
        .jsx_elements
        .iter()
        .filter(|element| component.span.contains(element.span))
        .collect::<Vec<_>>();
    let static_element = |element: &solid_facts::ast::JsxElementFact| {
        let intrinsic = file.source_text(element.name.span).is_some_and(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
        let children = element.children.iter().all(|child| {
            let text = file.source_text(*child).unwrap_or("{");
            if text.starts_with('<') {
                elements.iter().any(|inner| inner.span == *child)
            } else if text.starts_with('{') {
                element.empty_children.contains(child)
                    || settled_expression_child(file, component, *child, lookup)
            } else {
                true
            }
        });
        // ADR 0510: a string attribute is written into the template.
        let static_attributes = element.attributes.iter().all(|attribute| {
            attribute.namespace.is_none()
                && attribute.expression.is_none()
                && attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::String
        });
        own(element.span)
            && intrinsic
            && element.member_object.is_none()
            && static_attributes
            && element.spreads.is_empty()
            && element.boolean_properties.is_empty()
            && children
    };
    let is_element = |span: Span| {
        let span = ast.peel_ts_sugar_span(span);
        elements.iter().any(|element| element.span == span)
    };
    let returns = ast
        .returns
        .iter()
        .filter(|fact| own(fact.span))
        .collect::<Vec<_>>();
    let returns_tree = if component.expression_body {
        is_element(component.body)
    } else {
        !returns.is_empty()
            && returns
                .iter()
                .all(|fact| fact.argument.is_some_and(is_element))
    };
    !ast.jsx_fragments
        .iter()
        .any(|fragment| component.span.contains(*fragment))
        && elements.iter().all(|element| static_element(element))
        && returns_tree
}

/// Nothing written in `component`'s body outside the effect's and the
/// source's arguments, and its static tree's settled expressions, runs code
/// while the body runs, or throws: each call there is one of those two, a
/// signal or memo created from primitive literals or an inert creation
/// ([`inert_creation`], ADR 0510), a `useContext` of the context the mount
/// provides, or a call of an inert local function ([`inert_local_function`])
/// with primitive-literal arguments; no member is read, no template is
/// tagged, no class is written, no `throw` is written; a destructuring
/// declaration takes a local object or array literal (or a signal's tuple)
/// apart with no default; each name the body references outside those calls'
/// callees is its own, the provided context's, or the module source's.
fn body_runs_nothing_else(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    effect: &solid_facts::ast::CallFact,
    memo: Option<&solid_facts::ast::CallFact>,
    admits: BodyAdmits,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_dialect::Primitive;
    let ast = &file.ast;
    let provided = admits.context;
    let excluded = effect
        .arguments
        .iter()
        .chain(memo.into_iter().flat_map(|memo| &memo.arguments))
        .map(|argument| argument.span)
        .collect::<Vec<_>>();
    // The tree's expressions are [`static_intrinsic_tree`]'s to admit (ADR
    // 0510: settled children), and run when it renders.
    let in_tree = |span: Span| {
        ast.jsx_elements
            .iter()
            .any(|element| component.span.contains(element.span) && element.span.contains(span))
    };
    let in_code = |span: Span| {
        component.span.contains(span)
            && !excluded.iter().any(|region| region.contains(span))
            && !in_tree(span)
    };
    // Code of the body itself, not of a function nested in it.
    let own = |span: Span| {
        in_code(span)
            && crate::owners::containing_ast_function(ast, span)
                .is_some_and(|function| function.span == component.span)
    };
    let literal_arguments = |call: &solid_facts::ast::CallFact| {
        call.arguments.iter().all(|argument| {
            !argument.spread
                && file
                    .source_text(ast.peel_ts_sugar_span(argument.span))
                    .is_some_and(primitive_literal_body)
        })
    };
    let primitive_callee = |call: &solid_facts::ast::CallFact| {
        call.span == effect.span
            || memo.is_some_and(|memo| call.span == memo.span)
            || provided_context_call(file, call, provided, lookup)
            || admits.hook == Some(call.span)
            || (inert_creation(file, call, lookup)
                && lookup.primitive_at_call(file, call.span) != Some(Primitive::CreateContext))
            || matches!(
                lookup.primitive_at_call(file, call.span),
                Some(Primitive::CreateSignal | Primitive::CreateMemo)
            ) && literal_arguments(call)
    };
    // The argument of a `useContext` of the provided context names it.
    let context_arguments = ast
        .calls
        .iter()
        .filter(|call| own(call.span) && provided_context_call(file, call, provided, lookup))
        .filter_map(|call| call.arguments.first())
        .map(|argument| ast.peel_ts_sugar_span(argument.span))
        .collect::<Vec<_>>();
    let calls = ast.calls.iter().filter(|call| own(call.span)).all(|call| {
        primitive_callee(call)
            || (literal_arguments(call)
                && local_function_of_callee(file, call.callee).is_some_and(|callee| {
                    own(callee.span) && inert_local_function(file, component, callee, admits)
                }))
    });
    // A nested function runs only when called: by a call above (inert), or
    // from the excluded regions. An inert creation's compute (ADR 0510) runs
    // one literal.
    let computes = ast
        .calls
        .iter()
        .filter(|call| own(call.span) && inert_creation(file, call, lookup))
        .filter_map(|call| call.arguments.first())
        .map(|argument| ast.peel_ts_sugar_span(argument.span))
        .collect::<Vec<_>>();
    let nested = ast
        .functions
        .iter()
        .filter(|function| function.span != component.span && in_code(function.span))
        .all(|function| {
            !own(function.span)
                || crate::indexes::declared_function(ast, function)
                || computes.contains(&function.span)
        });
    let quiet = !ast
        .members
        .iter()
        .any(|member| own(member.span) && !handed_member_read(file, member, admits))
        && !ast.computed_members.iter().any(|member| own(*member))
        && !ast.tagged_template_tags.iter().any(|tag| in_code(*tag))
        && !ast.classes.iter().any(|class| in_code(class.span))
        && !ast.spreads.iter().any(|spread| own(spread.span))
        && !ast
            .object_properties
            .iter()
            .any(|property| own(property.span) && property.computed)
        && !ast
            .throw_statements
            .iter()
            .any(|statement| in_code(*statement));
    let callees = ast
        .calls
        .iter()
        .filter(|call| own(call.span) && primitive_callee(call))
        .map(|call| ast.peel_ts_sugar_span(call.callee))
        .collect::<Vec<_>>();
    let references = ast.identifiers.iter().all(|identifier| {
        !crate::indexes::runtime_reference(ast, identifier)
            || !own(identifier.span)
            || callees.contains(&identifier.span)
            || context_arguments.contains(&identifier.span)
            || ast
                .reference_declaration(identifier.span)
                .is_some_and(|declaration| {
                    component.span.contains(declaration) || admits.source == Some(declaration)
                })
    });
    let destructuring = ast
        .bindings
        .iter()
        .filter(|binding| {
            own(binding.declaration) && binding.shape != solid_facts::ast::BindingShape::Identifier
        })
        .all(|binding| {
            local_literal_destructuring(file, component, binding, admits, lookup)
                || hook_result_destructuring(file, binding, admits)
                // A signal's tuple (ADR 0510, `inert_creation`).
                || binding
                    .call_initializer
                    .and_then(|span| ast.call_at(span))
                    .is_some_and(|call| inert_creation(file, call, lookup))
        });
    calls && nested && quiet && references && destructuring
}

/// A local `const` function literal whose body runs nothing: it calls, reads
/// a member of, tags, renders, spreads, awaits and throws nothing, nests no
/// function, and each name it references is declared in `component`.
fn inert_local_function(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    function: &solid_facts::ast::FunctionFact,
    admits: BodyAdmits,
) -> bool {
    let ast = &file.ast;
    let inside = |span: Span| function.span.contains(span);
    !function.r#async
        && !function.generator
        && function.parameters.is_empty()
        && !ast.calls.iter().any(|call| inside(call.span))
        && !ast.members.iter().any(|member| inside(member.span))
        && !ast.computed_members.iter().any(|member| inside(*member))
        && !ast.tagged_template_tags.iter().any(|tag| inside(*tag))
        && !ast.jsx_elements.iter().any(|element| inside(element.span))
        && !ast.jsx_fragments.iter().any(|fragment| inside(*fragment))
        && !ast.spreads.iter().any(|spread| inside(spread.span))
        && !ast.awaits.iter().any(|awaited| inside(*awaited))
        && !ast
            .object_properties
            .iter()
            .any(|property| inside(property.span) && property.computed)
        && !ast
            .functions
            .iter()
            .any(|nested| nested.span != function.span && inside(nested.span))
        && !ast
            .throw_statements
            .iter()
            .any(|statement| inside(*statement))
        && ast.identifiers.iter().all(|identifier| {
            !crate::indexes::runtime_reference(ast, identifier)
                || !inside(identifier.span)
                || ast
                    .reference_declaration(identifier.span)
                    .is_some_and(|declaration| {
                        component.span.contains(declaration) || admits.source == Some(declaration)
                    })
        })
}

/// ADR 0511: an object pattern with no default taking apart the result of
/// the body's inert hook call ([`BodyAdmits::hook`]), which is an object
/// literal of data properties: it reads plain properties and cannot throw.
fn hook_result_destructuring(
    file: &solid_facts::FileFacts,
    binding: &solid_facts::ast::BindingFact,
    admits: BodyAdmits,
) -> bool {
    binding.shape == solid_facts::ast::BindingShape::Object
        && admits.hook.is_some()
        && binding.call_initializer == admits.hook
        && file
            .source_text(binding.pattern)
            .is_some_and(|pattern| !pattern.contains('=') && !pattern.contains("..."))
}

/// A destructuring declaration with no default that takes apart an object
/// literal (written there, bound to a local `const`, or returned by an inert
/// local function's expression body) or an array literal (written there or
/// bound to a local `const`, with the array iterator intact, ADR 0349): it
/// reads plain properties and cannot throw.
fn local_literal_destructuring(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    binding: &solid_facts::ast::BindingFact,
    admits: BodyAdmits,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_facts::ast::BindingShape;
    let ast = &file.ast;
    let Some(initializer) = binding.initializer.map(|span| ast.peel_ts_sugar_span(span)) else {
        return false;
    };
    let opens = |span: Span, bracket: char| {
        file.source_text(ast.peel_ts_sugar_span(span))
            .is_some_and(|text| text.starts_with(bracket))
    };
    let literal = |span: Span, bracket: char| {
        if opens(span, bracket) {
            return true;
        }
        // A local `const` holding the literal.
        ast.reference_declaration(span)
            .and_then(|declaration| {
                ast.bindings.iter().find(|candidate| {
                    candidate.immutable
                        && candidate.shape == BindingShape::Identifier
                        && candidate.names.iter().any(|name| name.span == declaration)
                })
            })
            .is_some_and(|candidate| {
                component.span.contains(candidate.declaration)
                    && candidate
                        .initializer
                        .is_some_and(|value| opens(value, bracket))
            })
    };
    let returned_object = || {
        ast.call_at(initializer).is_some_and(|call| {
            call.arguments.is_empty()
                && local_function_of_callee(file, call.callee).is_some_and(|callee| {
                    callee.expression_body
                        && component.span.contains(callee.span)
                        && inert_local_function(file, component, callee, admits)
                        && opens(callee.body, '{')
                })
        })
    };
    let no_default = file
        .source_text(binding.pattern)
        .is_some_and(|pattern| !pattern.contains('='));
    no_default
        && match binding.shape {
            BindingShape::Object => literal(initializer, '{') || returned_object(),
            BindingShape::Array => lookup.array_iteration_intact() && literal(initializer, '['),
            BindingShape::Identifier => true,
        }
}

/// ADR 0505: no code that can create a reactive node runs before the module-
/// scope `render` call at `render` in `entry`, so its tail flush is the first
/// and holds nothing an earlier pass left: no project module's scope runs
/// code except `entry`'s, every module loaded is the dialect's own or an
/// analyzed project file ([`crate::indexes::module_scope_census`]), and before
/// the call `entry` itself runs only standard-library calls and local `const`
/// functions that run only those, references no import but the call's own
/// callee, and writes no class, tagged template, JSX outside the call, or
/// top-level `await`.
pub(super) fn render_runs_first(
    lookup: &SemanticLookup<'_>,
    entry: &solid_facts::FileFacts,
    render: Span,
) -> bool {
    let (loud, creating, _, unseen) = lookup.module_scope_census();
    // ADR 0510: a module whose scope only creates nodes (a memo of a literal,
    // a signal, a context) creates none that can hold the flush.
    if unseen
        || loud
            .iter()
            .any(|path| path != entry.path.as_str() && !creating.contains(path))
    {
        return false;
    }
    let ast = &entry.ast;
    let Some(call) = ast.call_at(render) else {
        return false;
    };
    let Some(mounted) = call.arguments.first().map(|argument| argument.span) else {
        return false;
    };
    // What runs before the mount: the code ahead of the call, and the call's
    // other arguments.
    let before = |span: Span| span.end <= render.end && !mounted.contains(span) && span != render;
    let standard = |call: &solid_facts::ast::CallFact| {
        let kind = if call.construct {
            typefacts::CallKind::Construct
        } else {
            typefacts::CallKind::Call
        };
        lookup
            .standard_library_declaration(entry, call, kind)
            .is_some()
    };
    let local_runs_standard = |call: &solid_facts::ast::CallFact| {
        local_function_of_callee(entry, call.callee).is_some_and(|function| {
            function.span.end <= render.start
                && crate::indexes::at_module_scope(ast, function.span)
                && ast
                    .calls
                    .iter()
                    .filter(|inner| function.span.contains(inner.span))
                    .all(&standard)
        })
    };
    let imports = ast
        .imports
        .iter()
        .flat_map(|import| import.bindings.iter().map(|binding| binding.local.span))
        .collect::<Vec<_>>();
    let callee = ast.peel_ts_sugar_span(call.callee);
    ast.module_hazards.is_empty()
        && ast
            .calls
            .iter()
            .filter(|candidate| before(candidate.span))
            .all(|candidate| standard(candidate) || local_runs_standard(candidate))
        && ast
            .functions
            .iter()
            .filter(|function| before(function.span))
            .all(|function| {
                crate::indexes::at_module_scope(ast, function.span)
                    && crate::indexes::declared_function(ast, function)
            })
        && ast.identifiers.iter().all(|identifier| {
            !crate::indexes::runtime_reference(ast, identifier)
                || !before(identifier.span)
                || identifier.span == callee
                || ast
                    .reference_declaration(identifier.span)
                    .is_none_or(|declaration| !imports.contains(&declaration))
        })
        && !ast.classes.iter().any(|class| before(class.span))
        && !ast.tagged_template_tags.iter().any(|tag| before(*tag))
        && !ast.jsx_elements.iter().any(|element| before(element.span))
        && !ast.jsx_fragments.iter().any(|fragment| before(*fragment))
        // ADR 0518 review: `for await` and `await using` suspend as `await` does.
        && !ast
            .awaits
            .iter()
            .chain(&ast.implicit_suspensions)
            .any(|awaited| before(*awaited) && crate::indexes::at_module_scope(ast, *awaited))
        && !ast.module_loads.iter().any(|load| before(load.span))
}

/// ADR 0504 review: the read at `site` runs on every normal completion of the
/// apply's first run: on every completion of its own function, and each
/// in-place layer's call on every completion of the function holding it, up to
/// the apply. A guard false on the first run (`if (v === 1) return;`, an empty
/// loop) skips it.
fn read_on_every_completion_of_apply(
    file: &solid_facts::FileFacts,
    site: Span,
    apply: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let mut at = site;
    for _ in 0..16 {
        let Some(function) = crate::owners::containing_body_function(&file.ast, at) else {
            return false;
        };
        if !crate::interproc::site_on_every_completion(lookup, file, function.span, at) {
            return false;
        }
        if function.span == apply {
            return true;
        }
        at = match layer_run(file, function, lookup) {
            LayerRun::InPlace(call) | LayerRun::InPlaceAmongOthers(call) => call,
            LayerRun::FreshStack | LayerRun::Elsewhere | LayerRun::Withheld(_) => return false,
        };
    }
    false
}

/// Every call written in `component`'s own body between `from` and `until`
/// (not inside a function nested there) creates a signal or memo, or calls a
/// local function whose body calls nothing: nothing there can read the source
/// or throw before the effect is created.
pub(super) fn body_runs_nothing_between(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    from: u32,
    until: u32,
    admits: BodyAdmits,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_dialect::Primitive;
    let ast = &file.ast;
    ast.calls
        .iter()
        .filter(|call| {
            call.span.start >= from
                && call.span.end <= until
                && crate::owners::containing_ast_function(ast, call.span)
                    .is_some_and(|function| function.span == component.span)
        })
        .all(|call| {
            matches!(
                lookup.primitive_at_call(file, call.span),
                Some(Primitive::CreateSignal | Primitive::CreateMemo)
            ) || provided_context_call(file, call, admits.context, lookup)
                || admits.hook == Some(call.span)
                || local_function_of_callee(file, call.callee).is_some_and(|callee| {
                    component.span.contains(callee.span)
                        && !ast
                            .calls
                            .iter()
                            .any(|inner| callee.span.contains(inner.span))
                        && throws_nothing_before(
                            file,
                            callee,
                            callee.span.start,
                            callee.span.end,
                            admits,
                            lookup,
                        )
                })
        })
        && throws_nothing_before(file, component, from, until, admits, lookup)
}

/// ADR 0510 review: no expression of `function`'s own code (not of a function
/// nested in it) written between `from` and `until` may throw without a call:
/// a member read (`props.cfg.x` of an `any` that is `undefined`, a getter), a
/// computed member, a spread or iteration, an operator (`'k' in 1`,
/// `instanceof`, arithmetic that coerces), a `delete`, a destructuring of
/// anything but a signal's or memo's tuple or a local literal (`const { a } =
/// null`), a tagged template, a
/// class, a `throw` or a loop. Calls are the callers' to check. Before this,
/// `const v = props.cfg.x;` before the effect, or `(v as any).a.b;` before the
/// read in the apply, left the claim standing though the run throws first.
pub(super) fn throws_nothing_before(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    from: u32,
    until: u32,
    admits: BodyAdmits,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_dialect::Primitive;
    let ast = &file.ast;
    let own = |span: Span| {
        span.start >= from
            && span.end <= until
            && function.span.contains(span)
            && crate::owners::containing_ast_function(ast, span)
                .is_some_and(|owner| owner.span == function.span)
    };
    // A signal's or memo's tuple, or a local literal taken apart (ADR 0505's
    // [`local_literal_destructuring`]): nothing there can throw.
    let safe_pattern = |binding: &solid_facts::ast::BindingFact| {
        local_literal_destructuring(file, function, binding, admits, lookup)
            || hook_result_destructuring(file, binding, admits)
            || (binding.shape == solid_facts::ast::BindingShape::Array
                && binding
                    .call_initializer
                    .and_then(|span| ast.call_at(span))
                    .is_some_and(|call| {
                        matches!(
                            lookup.primitive_at_call(file, call.span),
                            Some(Primitive::CreateSignal | Primitive::CreateMemo)
                        )
                    }))
    };
    !ast.members
        .iter()
        .any(|member| own(member.span) && !handed_member_read(file, member, admits))
        && !ast.computed_members.iter().any(|member| own(*member))
        && !ast.spreads.iter().any(|spread| own(spread.span))
        && !ast.iterated_operands.iter().any(|operand| {
            own(*operand)
                && !ast.bindings.iter().any(|binding| {
                    safe_pattern(binding)
                        && binding
                            .initializer
                            .is_some_and(|initializer| initializer.contains(*operand))
                })
        })
        && !ast.primitive_results.iter().any(|operator| own(*operator))
        && !ast.coercing_operands.iter().any(|operand| own(*operand))
        && !ast.deleted_targets.iter().any(|target| own(*target))
        && !ast.tagged_template_tags.iter().any(|tag| own(*tag))
        && !ast.classes.iter().any(|class| own(class.span))
        && !ast.throw_statements.iter().any(|statement| own(*statement))
        && !ast.loop_statements.iter().any(|statement| own(*statement))
        && ast
            .bindings
            .iter()
            .filter(|binding| {
                own(binding.declaration)
                    && binding.shape != solid_facts::ast::BindingShape::Identifier
            })
            .all(safe_pattern)
}

/// The function literal a call's callee names: an identifier bound by a
/// `const` whose initializer is exactly that literal (not a primitive's
/// result, such as a memo accessor).
fn local_function_of_callee(
    file: &solid_facts::FileFacts,
    callee: Span,
) -> Option<&solid_facts::ast::FunctionFact> {
    let ast = &file.ast;
    let declaration = ast.reference_declaration(ast.peel_ts_sugar_span(callee))?;
    let binding = ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.iter().any(|name| name.span == declaration)
    })?;
    ast.function_at(ast.peel_ts_sugar_span(binding.initializer?))
}

/// A compute function literal that settles on its first pass with a plain
/// value: not async or a generator, with no parameter or one plain parameter
/// with no default, whose whole body is one primitive literal or returns one
/// (`() => 1`, `() => { return 1; }`). Anything else may run code, throw, or
/// return a thenable the runtime parks (a dynamic `import()`, an object with
/// `then`; ADR 0503 review).
fn compute_settles_synchronously(file: &solid_facts::FileFacts, argument: Span) -> bool {
    let ast = &file.ast;
    let Some(function) = ast.function_at(ast.peel_ts_sugar_span(argument)) else {
        return false;
    };
    let plain_parameters = match function.parameters.as_slice() {
        [] => true,
        [parameter] => {
            parameter.shape == solid_facts::ast::BindingShape::Identifier
                && parameter.initializer.is_none()
        }
        _ => false,
    };
    !function.r#async
        && !function.generator
        && plain_parameters
        && file
            .source_text(function.body)
            .is_some_and(primitive_literal_body)
}

/// The text of a function body that is one primitive literal, or a block that
/// only returns one.
pub(crate) fn primitive_literal_body(text: &str) -> bool {
    let text = text.trim();
    let literal = match text
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
    {
        Some(inner) => {
            let Some(rest) = inner.trim().strip_prefix("return") else {
                return false;
            };
            if !rest.starts_with(char::is_whitespace) {
                return false;
            }
            let rest = rest.trim();
            rest.strip_suffix(';').unwrap_or(rest).trim()
        }
        None => text,
    };
    let quoted = |quote: char| {
        literal.len() >= 2
            && literal.starts_with(quote)
            && literal.ends_with(quote)
            && !literal[1..literal.len() - 1].contains([quote, '\\', '\n'])
    };
    matches!(literal, "true" | "false" | "null")
        || quoted('"')
        || quoted('\'')
        || (!literal.is_empty()
            && literal.trim_start_matches('-').split('.').count() <= 2
            && literal
                .trim_start_matches('-')
                .split('.')
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit())))
}

/// Climbs from `span` through the function layers that run in place: an IIFE,
/// an inline standard-library callback, a helper's callback it calls in its own
/// body ([`layer_run`], the walk the write roles take). Stops at the first
/// function that is not such a layer (a primitive's callback, a stored or
/// handed-on function, a generator); `None` when `span` is in none.
pub(crate) fn climb_in_place_layers(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<InPlaceClimb> {
    climb_in_place_layers_within(file, span, lookup, None)
}

/// [`climb_in_place_layers`] that does not cross the function at `within`:
/// the climb ends at the call in that function's body which runs the site,
/// however that function itself is run. A summary claims the site at that
/// call (`interproc::reads`, ADR 0522).
pub(crate) fn climb_in_place_layers_within(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    within: Option<Span>,
) -> Option<InPlaceClimb> {
    climb_in_place_layers_recording(file, span, lookup, false, within, &mut Vec::new())
}

/// [`climb_in_place_layers_within`], appending to `passed` the call of each
/// layer crossed, innermost first.
fn climb_in_place_layers_recording(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    crossing: bool,
    within: Option<Span>,
    passed: &mut Vec<Span>,
) -> Option<InPlaceClimb> {
    let mut climb = InPlaceClimb {
        exhausted: false,
        at: span,
        in_list: false,
        among_others: false,
        suspended: None,
        withheld: false,
    };
    // Bounded: each step moves to the call that holds the previous function.
    for step in 0..16 {
        let Some(function) = crate::owners::owning_function(&file.ast, climb.at) else {
            break;
        };
        if function.generator || within == Some(function.span) {
            break;
        }
        let call = match layer_run(file, function, lookup) {
            LayerRun::InPlace(call) => call,
            LayerRun::InPlaceAmongOthers(call) => {
                climb.among_others = true;
                call
            }
            LayerRun::Withheld(call) if crossing => {
                climb.among_others = true;
                climb.withheld = true;
                call
            }
            LayerRun::FreshStack | LayerRun::Elsewhere | LayerRun::Withheld(_) => break,
        };
        // A layer called in a JSX expression runs where the compiler places
        // that expression (an insert's tracked computation), which the
        // compiler roles decide, not the code around the element. A call in
        // the body of a function written inside JSX (a flow component's
        // children callback, `<For>{() => { h(() => r()); … }}</For>`, a ref
        // callback, a handler) is in no JSX expression: the element around it is outside the call's own
        // function, and the call runs when that function does.
        let own = crate::owners::owning_function(&file.ast, call);
        if file
            .ast
            .jsx_containing(call)
            .any(|element| own.is_none_or(|function| function.span.contains(element.span)))
        {
            break;
        }
        climb.in_list |= !function.body.contains(climb.at);
        if follows_unconditional_await(file, climb.at)
            || follows_dominating_await(file, climb.at)
            || follows_try_head_await(file, climb.at, lookup)
        {
            climb.suspended = Some(true);
        } else if climb.suspended.is_none()
            && (follows_any_suspension(file, climb.at) || may_follow_own_await(file, climb.at))
        {
            climb.suspended = Some(false);
        }
        climb.at = call;
        passed.push(call);
        climb.exhausted = step == 15;
    }
    (climb.at != span).then_some(climb)
}

/// ADR 0628: the call that runs `function` in place, one layer of
/// [`climb_in_place_layers`].
pub(crate) fn in_place_layer_call(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    match layer_run(file, function, lookup) {
        LayerRun::InPlace(call) => Some(call),
        _ => None,
    }
}

/// [`climb_in_place_layers`] continued through the generators stepped at
/// their own call (ADR 0516): `(function* () { r(); yield 1; })().next()`
/// runs some of its body, or none, on the stack of that call, so the climb
/// goes on from there. `through_generator` records that such a step was
/// crossed, where a proven run is no proof of the site's run. `None` when
/// `span` crosses no layer.
pub(crate) fn in_place_run_site(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<(InPlaceClimb, bool)> {
    in_place_run_site_within(file, span, lookup, None)
}

/// [`in_place_run_site`] that does not cross the function at `within`
/// ([`climb_in_place_layers_within`]).
pub(crate) fn in_place_run_site_within(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    within: Option<Span>,
) -> Option<(InPlaceClimb, bool)> {
    in_place_run_site_recording(file, span, lookup, false, within, &mut Vec::new())
}

/// ADR 0874: [`in_place_run_site_within`] across a call the census withholds,
/// for a summary that places rows: it proves no run, so a row through
/// [`InPlaceClimb::withheld`] is undecided. Nothing else may cross one.
pub(crate) fn in_place_run_site_placing(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    within: Option<Span>,
) -> Option<(InPlaceClimb, bool)> {
    in_place_run_site_recording(file, span, lookup, true, within, &mut Vec::new())
}

/// The calls [`in_place_run_site`] passes from `span` outwards, innermost
/// first: each layer's call and each stepped generator's creating call. Empty
/// when `span` crosses no layer. ADR 0874: past a call the census withholds
/// too, for [`in_place_run_site_placing`]'s reader.
pub(crate) fn in_place_run_calls(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    let mut passed = Vec::new();
    in_place_run_site_recording(file, span, lookup, true, None, &mut passed);
    passed
}

fn in_place_run_site_recording(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    crossing: bool,
    within: Option<Span>,
    passed: &mut Vec<Span>,
) -> Option<(InPlaceClimb, bool)> {
    let mut climb = InPlaceClimb {
        exhausted: false,
        at: span,
        in_list: false,
        among_others: false,
        suspended: None,
        withheld: false,
    };
    let mut through_generator = false;
    for step in 0..8 {
        if let Some(next) =
            climb_in_place_layers_recording(file, climb.at, lookup, crossing, within, passed)
        {
            climb.in_list |= next.in_list;
            climb.among_others |= next.among_others;
            climb.withheld |= next.withheld;
            climb.suspended = match (climb.suspended, next.suspended) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), _) | (_, Some(false)) => Some(false),
                (None, None) => None,
            };
            climb.at = next.at;
            if next.exhausted {
                climb.exhausted = true;
                break;
            }
        }
        let Some(created) = crate::owners::owning_function(&file.ast, climb.at)
            .filter(|function| {
                function.generator && !function.r#async && within != Some(function.span)
            })
            .and_then(|function| crate::owners::generator_iterator_used(file, function))
            .filter(|created| crate::owners::iterator_stepped_at(file, created.span))
        else {
            break;
        };
        through_generator = true;
        climb.at = created.span;
        passed.push(created.span);
        climb.exhausted = step == 7;
    }
    (climb.at != span).then_some((climb, through_generator))
}

/// ADR 0466: a primitive call that names a function as its callback by its
/// binding (`createRoot(init)`, `untrack(write)`, `runWithOwner(null, run)`),
/// which runs it as it runs an inline callback in the same position.
pub(super) struct ByNamePosition<'f> {
    pub(super) call: &'f solid_facts::ast::CallFact,
    pub(super) index: usize,
    pub(super) primitive: Primitive,
    /// The position is the function's only entry: every reference to its
    /// binding is the declaration or this argument, and nothing hands it out
    /// or exports it. Otherwise the function has runs this does not follow:
    /// one run is at the position, which proves a violation and no legal write
    /// ([`LayerRun::InPlaceAmongOthers`]).
    pub(super) sole: bool,
}

/// The one primitive call position that names `function` by its binding: a
/// bare identifier argument whose symbol is exactly the binding's, which holds
/// this function now ([`SemanticLookup::function_value_is_current`]). A `const`
/// named before its declarator is evaluated throws instead of passing the
/// function, so the argument must follow the declarator in the same function.
/// A method, a generator, a named function expression entered by its own name
/// too (ADR 0515: one its own name never references is named by its binding,
/// `named_expression_binding`) and a function two primitive positions name
/// answer nothing.
pub(super) fn by_name_position<'f>(
    file: &'f solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<ByNamePosition<'f>> {
    use solid_facts::ast::FunctionKind;
    let named_expression = function.kind == FunctionKind::Expression && function.name.is_some();
    if function.method_name.is_some()
        || function.generator
        || !lookup.function_value_is_current(file, function)
    {
        return None;
    }
    let name = if named_expression {
        super::named_callbacks::named_expression_binding(file, function, lookup)?
    } else {
        function_binding_name(file, function)?
    };
    let path = file.path.as_str();
    let symbol = lookup.entities().at(path, name.span)?;
    let mut found = None;
    for call in &file.ast.calls {
        for (index, argument) in call.arguments.iter().enumerate() {
            if argument.spread
                || argument.value != solid_facts::ast::ArgumentValueKind::Identifier
                || lookup.entities().at(path, argument.span) != Some(symbol)
            {
                continue;
            }
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            if found.is_some() {
                return None;
            }
            found = Some((call, index, primitive, argument.span));
        }
    }
    let (call, index, primitive, argument) = found?;
    if function.kind != FunctionKind::Declaration {
        let scope = |span: Span| {
            crate::owners::containing_ast_function(&file.ast, span).map(|scope| scope.span)
        };
        // ADR 0471: an argument in layers that run in place (an IIFE, an inline
        // standard-library callback, a helper's callback) is evaluated when the
        // outermost layer's call runs, which must follow the declarator.
        let target = scope(name.span);
        let mut at = argument;
        for _ in 0..16 {
            if scope(at) == target {
                break;
            }
            let layer = crate::owners::containing_ast_function(&file.ast, at)?;
            if layer.generator
                || !layer.body.contains(at)
                || may_follow_suspension(file, at)
                || follows_dominating_await(file, at)
                || follows_try_head_await(file, at, lookup)
            {
                return None;
            }
            let LayerRun::InPlace(call) = layer_run(file, layer, lookup) else {
                return None;
            };
            if file.ast.jsx_containing(call).next().is_some() {
                return None;
            }
            at = call;
        }
        if at.start < function.span.end || scope(at) != target {
            return None;
        }
    }
    let sole = !lookup.hands_out_by_name(path, function.span)
        && !module_exports(file, function, lookup)
        && lookup
            .symbol_references_if_present(symbol.as_str())
            .is_some_and(|references| {
                references.iter().all(|reference| {
                    let (Ok(start), Ok(end)) = (
                        u32::try_from(reference.start_byte),
                        u32::try_from(reference.end_byte),
                    ) else {
                        return false;
                    };
                    let at = Span::new(start, end);
                    reference.path.as_ref() == path
                        && (at == name.span
                            || at == argument
                            || file.ast.type_queries.iter().any(|query| query.contains(at)))
                })
            });
    Some(ByNamePosition {
        call,
        index,
        primitive,
        sole,
    })
}

/// ADR 0457: the call that runs `layer` in place because `layer` is its direct
/// argument and the exact callee, a project function or a dependency function
/// read from its source (ADR 0266, 0300), is synchronous and calls that
/// parameter in its own body on every completion
/// ([`invokes_parameter_during_call`]), the proof [`callee_callback_timing`]
/// takes for a read. A call in the callee's own body is on the call's stack
/// and under its owner: a callee that hands the parameter on (`runWithOwner(o,
/// cb)`) or calls it in a closure (`createRoot(() => cb())`) proves nothing.
/// A primitive's callback is the dialect's question, never this one.
///
/// The proof says the parameter is called at least once, not that every run of
/// the callback is that call: [`ParameterUses::AlsoElsewhere`] says the callee
/// has other uses of it.
fn helper_runs_layer_inline(
    file: &solid_facts::FileFacts,
    layer: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<(Span, ParameterUses)> {
    file.ast
        .arguments_containing(layer.span)
        .filter(|(call, index)| {
            !call.arguments[*index].spread
                && file.ast.peel_ts_sugar_span(call.arguments[*index].span) == layer.span
        })
        .find_map(|(call, index)| {
            helper_argument_run(file, call, index, lookup)
                .filter(|run| run.every_completion)
                .map(|run| (call.span, run.uses))
        })
}

/// Whether the call at `callee` in `caller` is a plain call expression written
/// outside every JSX element of `caller`'s body.
pub(super) fn plain_call_outside_jsx(
    file: &solid_facts::FileFacts,
    caller: &solid_facts::ast::FunctionFact,
    callee: Span,
) -> bool {
    file.ast
        .calls_with_peeled_callee(callee)
        .any(|call| !call.construct)
        && !file
            .ast
            .jsx_containing(callee)
            .any(|element| caller.body.contains(element.span))
}

/// Whether the module publishes `function` itself: an export declaration of
/// it, with no other function between, or Type Facts' export flag on it.
pub(crate) fn module_exports(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast.exports.iter().any(|export| {
        export.span.contains(function.span)
            && !file.ast.functions.iter().any(|outer| {
                outer.span != function.span
                    && export.span.contains(outer.span)
                    && outer.span.contains(function.span)
            })
    }) || lookup
        .typescript_file(file.path.as_str())
        .is_some_and(|facts| {
            facts.functions.iter().any(|candidate| {
                candidate.exported
                    && candidate.body.start_byte == u64::from(function.body.start)
                    && candidate.body.end_byte == u64::from(function.body.end)
            })
        })
}

/// ADR 0327's census: every invocation of the literal, followed forward
/// through a closed program, is an event delivery, and no synchronous dispatch
/// can hand a delivery its caller's owner. Such a function runs with no owner
/// however it is passed around.
pub(super) fn event_only_entries(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    !lookup.synchronous_event_dispatch_present()
        && lookup
            .event_invoked_literals()
            .contains(&(file.path.to_string(), function.span))
}

/// The role of a write its own function may reach only past a suspension,
/// where the owner a callback slot gives the function is gone: legal past a
/// dominating await, unplaced past one that may be skipped. `None` when no
/// suspension precedes it.
pub(super) fn suspended_write_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    if follows_dominating_await(file, span) || follows_try_head_await(file, span, lookup) {
        Some(ExecutionRole::Unreported)
    } else if may_follow_suspension(file, span) {
        Some(ExecutionRole::Unknown)
    } else {
        None
    }
}

/// Whether `span` sits in a function literal that is the value of a JSX
/// attribute, or in a function nested inside one.
pub(super) fn enclosed_by_jsx_attribute_function(
    file: &solid_facts::FileFacts,
    span: Span,
) -> bool {
    functions_around(file, span)
        .into_iter()
        .any(|function| jsx_attribute_value_function(file, function.span))
}

/// Whether `span` sits in a function literal, written strictly inside `region`,
/// that the region's own evaluation is not proven to run: a JSX attribute's
/// value (an event handler, a callback prop), an argument handed to a call that
/// is not proven to invoke it during the call, or a stored literal. What such a
/// function does runs when something calls it, not in the region's pass.
pub(super) fn attribute_function_within(
    file: &solid_facts::FileFacts,
    region: Span,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    functions_around(file, span).into_iter().any(|function| {
        region.contains(function.span)
            && region != function.span
            && (jsx_attribute_value_function(file, function.span)
                || call_argument_invocation_unproven(file, function, lookup))
    })
}

/// ADR 0331: whether the innermost function around `span` that has a
/// component identity at all is a possible component rather than a proven
/// one. An enclosing component renders its own body, not a function nested
/// in it, so it proves nothing about how that function is entered.
fn innermost_identity_is_possible(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast
        .functions_body_containing(span)
        .filter(|function| lookup.function_component_status(file, function) != ComponentStatus::No)
        .min_by_key(|function| function.body.end - function.body.start)
        .is_some_and(|function| !lookup.function_is_component(file, function))
}

/// ADR 0510: `call` is the dialect's `useContext` of the context declared at
/// `provided`, which the mount provides: it returns the provider's value and
/// cannot throw (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:1811-1824`).
fn provided_context_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    provided: Option<Span>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    provided.is_some_and(|context| {
        lookup.primitive_at_call(file, call.span) == Some(solid_dialect::Primitive::UseContext)
            && matches!(call.arguments.as_slice(), [argument] if !argument.spread
                && ast.reference_declaration(ast.peel_ts_sugar_span(argument.span)) == Some(context))
    })
}

/// ADR 0510: an expression child of a static tree whose render cannot go
/// pending: a primitive literal (the compiler writes it into the template or
/// inserts the value, which `insert` places with no effect,
/// `@solidjs/web@2.0.0-rc.17` `dist/web.dev.js:1135-1141`; any lowering of
/// it computes the literal), or a call with no argument of a `const` of the
/// component's own body that an inert creation initialized
/// ([`inert_creation`]: a signal of a literal, or a synchronous memo of one,
/// computed at creation, `@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:4913-4932`):
/// the render effect reading it reads a settled value.
fn settled_expression_child(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    child: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let Some(inner) = file
        .source_text(child)
        .and_then(|text| text.strip_prefix('{')?.strip_suffix('}'))
        .map(str::trim)
    else {
        return false;
    };
    if primitive_literal_body(inner) {
        return true;
    }
    let calls = ast
        .calls
        .iter()
        .filter(|call| child.contains(call.span))
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        return false;
    };
    let callee = ast.peel_ts_sugar_span(call.callee);
    file.source_text(call.span).map(str::trim) == Some(inner)
        && call.arguments.is_empty()
        && !call.construct
        && ast.identifiers.iter().any(|identifier| {
            identifier.span == callee
                && identifier.role == solid_facts::ast::IdentifierRole::Reference
        })
        && ast
            .reference_declaration(callee)
            .and_then(|declaration| {
                ast.bindings.iter().find(|binding| {
                    binding.immutable
                        && match binding.shape {
                            solid_facts::ast::BindingShape::Identifier => {
                                binding.names.iter().any(|name| name.span == declaration)
                            }
                            // The accessor, the tuple's first slot.
                            solid_facts::ast::BindingShape::Array => binding
                                .array_slots
                                .first()
                                .and_then(Option::as_ref)
                                .is_some_and(|slot| slot.span == declaration),
                            solid_facts::ast::BindingShape::Object => false,
                        }
                })
            })
            .is_some_and(|binding| {
                crate::owners::containing_ast_function(ast, binding.declaration)
                    .is_some_and(|function| function.span == component.span)
                    && binding
                        .call_initializer
                        .and_then(|span| ast.call_at(span))
                        .is_some_and(|creation| {
                            // An async memo's render goes pending and holds
                            // the flush (the `expression-child` variant).
                            let synchronous = creation.arguments.first().is_none_or(|argument| {
                                ast.function_at(ast.peel_ts_sugar_span(argument.span))
                                    .is_none_or(|compute| !compute.r#async)
                            });
                            synchronous
                                && inert_creation(file, creation, lookup)
                                && lookup.primitive_at_call(file, creation.span)
                                    != Some(solid_dialect::Primitive::CreateContext)
                        })
            })
}
