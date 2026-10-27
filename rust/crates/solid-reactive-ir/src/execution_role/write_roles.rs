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
    let allowed = write_allowed_spans(file, allowed, lookup);
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
            if callback_owner_at_call(file, call, primitive, index, lookup)
                == Some(solid_dialect::CallbackOwner::InheritsFirstRun)
            {
                return Some(WriteRegionAdjustment::FirstRunAtCallSite(call.span));
            }
            None
        })
        .or_else(|| detached_in_place(file, span, lookup))
        .or_else(|| nested_in_leaf_scope(file, span, lookup))
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
    for _ in 0..16 {
        let Some(function) = crate::owners::owning_function(&file.ast, at) else {
            break;
        };
        if function.generator {
            break;
        }
        in_list |= !function.body.contains(at);
        // An await before the site in a layer: the rest of that layer runs
        // later, on a stack the apply's first run is not on (ADR 0459).
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
        if let Some((call, index, primitive)) = primitive_call {
            let execution = callback_execution_at_call(file, call, primitive, index, lookup);
            if execution == Some(Execution::Inline)
                && lookup
                    .dialect
                    .callback_inherits_calling_window(primitive, index)
            {
                match callback_owner_at_call(file, call, primitive, index, lookup) {
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
            // ADR 0459: the layers run on the stack of a callback that shares
            // its call's owner on its first run (`createRenderEffect`'s apply),
            // so the write is placed as one written directly in it. A site in a
            // list, or a callback a helper also hands on, has runs this climb
            // does not follow, and so no placement.
            if layered
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
                // `write_region_adjustment`'s own question, asked at the call.
                if !layered {
                    break;
                }
                first_hop.get_or_insert(call.span);
                at = call.span;
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
            LayerRun::FreshStack | LayerRun::Elsewhere => break,
        }
        layered = true;
    }
    first_hop
        .filter(|_| !in_list && !among_others)
        .map(WriteRegionAdjustment::CallSite)
}

/// Whether a write under a proven owner runs in a disposal, for the release
/// whose write guards stand down there ([`runs_in_disposal`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DisposalReach {
    /// Written in a callback that runs only while an owner is disposed, on a
    /// stack no await has left.
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
fn runs_in_disposal_within(
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
            LayerRun::Elsewhere => {
                return match lookup.function_component_status(file, function) {
                    ComponentStatus::Proven => DisposalReach::NotIn,
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
                            || is_event_handler_literal(file, function, lookup)
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
/// JSX attribute's value on an element): the event delivers it, and no
/// synchronous dispatch can deliver it inside a cleanup.
fn is_event_handler_literal(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    !lookup.synchronous_event_dispatch_present()
        && file.compiler.callback_roles.iter().any(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && callback.span == function.span
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
    let Some(outer) = crate::owners::owning_function(&file.ast, function.span) else {
        return false;
    };
    if !outer.body.contains(function.span) {
        return false;
    }
    let returned = outer
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
        });
    returned
        && file
            .ast
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
                                || effect_provably_reruns(
                                    file, call, index, outer, function, lookup,
                                ))
                    })
            })
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

/// How the functions between a site and the control-flow children callback
/// around it run (ADR 0459).
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
        control_flow_execution_role(file, at, entities, symbol_names, lookup.dialect)
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
            LayerRun::Elsewhere => return Some(ControlFlowLayers::Unrun),
        }
    }
    Some(ControlFlowLayers::Unrun)
}

/// Whether the callback at `call`'s `index` places the writes written in it by
/// its call's owner: one that keeps that owner and runs during the call
/// (`untrack`, ADR 0455), or one that shares it on its first run
/// (`createRenderEffect`'s apply, ADR 0459). Its lexical role (deferred) is
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
            LayerRun::Elsewhere | LayerRun::InPlaceAmongOthers(_) => {
                unrun = true;
                at = function.span;
            }
        }
    }
    false
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
    );
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
    )
}

/// [`semantic_write_role_walk`], and whether its `Unknown` is final: the write
/// runs under an owner the call gave it, so what its function's other callers
/// run it under places nothing (`decided`).
#[allow(clippy::too_many_arguments)]
fn write_role_walk(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    visiting: &mut HashSet<(String, Span)>,
    decided: &mut bool,
) -> ExecutionRole {
    // ADR 0360: a file the compiler refused places no write.
    if file.compiler.withheld() {
        return ExecutionRole::Unknown;
    }
    // Write legality follows the runtime's *owner* context, which is not
    // always the read-execution context: leaf scopes are write-legal though
    // their reads track, and `untrack` writes answer to the enclosing owner
    // though its reads do not subscribe. Resolve those two adjustments first
    // (innermost outward) and only then classify normally.
    let mut span = span;
    loop {
        match write_region_adjustment(file, span, lookup) {
            Some(WriteRegionAdjustment::LeafScope) => return ExecutionRole::DeferredCallback,
            Some(WriteRegionAdjustment::CallSite(outer)) => span = outer,
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
            Some(
                adjustment @ (WriteRegionAdjustment::CreatedRoot
                | WriteRegionAdjustment::CreatedRootAmongOthers),
            ) => {
                if let Some(role) = suspended_write_role(file, span, lookup) {
                    return role;
                }
                match root_body {
                    RootBodyGuard::Rejects => return ExecutionRole::UntrackedRendering,
                    // Legal in the run under the root; the callback's other runs
                    // are not placed, so a legal write is no claim there.
                    RootBodyGuard::Exempts
                        if matches!(adjustment, WriteRegionAdjustment::CreatedRootAmongOthers) =>
                    {
                        *decided = true;
                        return ExecutionRole::Unknown;
                    }
                    RootBodyGuard::Exempts => return ExecutionRole::UntrackedCallback,
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
                            return ExecutionRole::UntrackedRendering;
                        }
                        // A violation needs the proof that the write is not in a
                        // disposal; a legal write needs the release to say so.
                        // Neither is the call sites' to give: the owner is the
                        // call's, whatever runs the function.
                        return match runs_in_disposal(file, span, lookup) {
                            DisposalReach::NotIn => ExecutionRole::UntrackedRendering,
                            DisposalReach::Only if guard == Guard::StandsDown => {
                                ExecutionRole::UntrackedCallback
                            }
                            DisposalReach::Only | DisposalReach::Unplaced => {
                                *decided = true;
                                ExecutionRole::Unknown
                            }
                        };
                    }
                }
            }
            // Whether a root wraps this callback is not audited on this
            // release, so the write has no placement the checker can prove.
            Some(WriteRegionAdjustment::UnauditedRoot) => return ExecutionRole::Unknown,
            // No owner reaches the write: the guard has nothing to reject.
            Some(WriteRegionAdjustment::Detached) => return ExecutionRole::UntrackedCallback,
            Some(WriteRegionAdjustment::OwnerUnknown) => return ExecutionRole::Unknown,
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
                    ExecutionRole::DiscardedRendering => return first_run,
                    ExecutionRole::Unknown | ExecutionRole::Unreported => return first_run,
                    role if role.reports_disallowed_write() => return ExecutionRole::Unknown,
                    _ => break,
                }
            }
            None => {
                if !visiting.contains(&lexical_only_marker())
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
                        return suspended;
                    }
                    // A call a disposal leaves unplaced is final: no other
                    // entry of the function places what its owner decides.
                    if role == ExecutionRole::Unknown {
                        *decided = true;
                    }
                    return role;
                }
                break;
            }
        }
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
    // ADR 0459: the deferred role a control-flow component's children callback
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
        return ExecutionRole::UntrackedCallback;
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
        return unknown;
    }
    // A tracked role reaches every closure written inside the tracked region
    // or callback, including ones only stored there (`createMemo(() => ({ go:
    // () => set(1) }))`). The role is a proof for the closure only if the
    // closure runs inline in the scope that earned it.
    if direct == ExecutionRole::TrackedJsx
        && enclosed_by_stored_function(file, span, entities, symbol_names, lookup)
    {
        return unknown;
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
    let direct = if tracked_named.is_some() {
        ExecutionRole::Unknown
    } else {
        direct
    };
    // ADR 0455: an anonymous function stored or handed on inside such a
    // callback runs where its holder calls it, not in the callback's pass.
    if direct == ExecutionRole::DeferredCallback
        && unrun_layer_in_owner_keeping_callback(file, span, lookup)
    {
        return ExecutionRole::Unknown;
    }
    // Past an await that dominates it, the write runs on a later task with no
    // owner, whatever its lexical role or its function's callers: legal, and
    // proven so, though in no single role.
    if follows_dominating_await(file, span) || follows_try_head_await(file, span, lookup) {
        return ExecutionRole::Unreported;
    }
    // Code after an `await` in an async function runs on a later task, with no
    // owner; neither the lexical role nor the role at the function's call sites
    // describes it. Not proven to follow an await on every path (an `await`
    // inside a branch), so the answer is no claim rather than a legal write.
    // An implicit suspension (`for await`) is the same, and is not proven to
    // run at all; its callers' roles do not describe the code after it.
    if (direct == ExecutionRole::Unknown || direct.reports_disallowed_write())
        && may_follow_suspension(file, span)
    {
        return unknown;
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
        return unknown;
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
                if role == ExecutionRole::Unknown {
                    *decided = true;
                }
                return role;
            }
            // The entries of the layer decide, but none of them places a write
            // that follows a suspension (as above).
            Some(HandleRole::Unrun) if may_follow_suspension(file, span) => {
                *decided = true;
                return unknown;
            }
            Some(HandleRole::Unrun) => ExecutionRole::Unknown,
            None => direct,
        }
    };
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
        .filter(|_| named_callback_execution_role(file, span, lookup) == Some(direct));
    let named = named_callback.filter(|callback| {
        innermost.is_some_and(|innermost| runs_inline_within(file, innermost, callback, lookup))
    });
    // ADR 0331: in a function its calls enter but some unfollowed entry may
    // render, the rendering role is only a possibility. A call that proves a
    // disallowed write still proves it; otherwise no role is proven.
    let possible_render = direct == ExecutionRole::UntrackedRendering
        && innermost_identity_is_possible(file, span, lookup);
    if direct != ExecutionRole::Unknown && named_callback.is_none() && !possible_render {
        return direct;
    }
    let Some(innermost) = innermost else {
        return unknown;
    };
    // ADR 0452: the named closure built in a tracked scope is entered by its
    // calls, and the write runs with it only through layers that run in place
    // (an IIFE, an inline standard-library callback). Any other layer keeps or
    // schedules the write's function, whose entries are not the closure's.
    let entry = match tracked_named {
        Some(closure) if closure.span == innermost.span => Some(closure),
        Some(closure) if runs_in_place_within(file, innermost, closure, lookup) => Some(closure),
        Some(_) => return unknown,
        None => None,
    };
    // ADR 0327: every invocation of the writing function, followed forward
    // through the closed program, is an event delivery. No other entry exists.
    if event_only_entries(file, entry.unwrap_or(innermost), lookup) {
        return ExecutionRole::EventCallback;
    }
    if visiting.contains(&lexical_only_marker()) {
        return unknown;
    }
    let function = named.or(entry).unwrap_or(innermost);
    // The write is in a closure nested in the named callback: a call of the
    // callback does not run it, so a disallowed entry proves nothing here.
    let nested = entry.is_none() && function.span != innermost.span;
    // A binding that may hold another function (`let reset = …; reset = …`)
    // names more than this one at its positions and calls, so none of them is
    // proof that this function runs in an owned scope.
    let current = lookup.function_value_is_current(file, function)
        || function_binding_name(file, function).is_none();
    // A tracked position (`createMemo(reset)`) runs the callback in an owned
    // computation, whichever role the other positions name it in.
    if named.is_some() && !nested && lookup.named_callback_roles(file).get(function.span).tracked {
        return if current {
            ExecutionRole::TrackedJsx
        } else {
            unknown
        };
    }
    let key = (file.path.to_string(), function.span);
    if !visiting.insert(key.clone()) {
        return unknown;
    }
    let mut imperative = named.map(|_| direct);
    let mut unplaced = false;
    for (caller_file, callee) in lookup.function_call_sites(file.path.as_str(), function.span) {
        // A call written directly in a function this walk is already deriving
        // (the function itself, or one of its callers) runs in that
        // function's role, which its other entries decide -- when it is a
        // plain call outside JSX. A JSX render gives the body a fresh
        // component owner, and JSX the compiler wraps runs in a render
        // effect: such a re-entry has a role of its own the walk cannot
        // derive here.
        if let Some(caller) = crate::owners::containing_body_function(&caller_file.ast, callee)
            .filter(|caller| visiting.contains(&(caller_file.path.to_string(), caller.span)))
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
            visiting.remove(&key);
            return if nested || !current { unknown } else { role };
        }
        if role == ExecutionRole::Unknown {
            // An entry no fact places leaves the function's role unplaced,
            // whatever its other entries prove.
            unplaced = true;
        } else {
            imperative.get_or_insert(role);
        }
    }
    visiting.remove(&key);
    if possible_render {
        return unknown;
    }
    let Some(role) = imperative.filter(|_| !unplaced) else {
        return unknown;
    };
    // A permissive role is inherited only from a complete census of the
    // function's entries: a use the walk did not follow (handed to a child
    // component, stored, aliased, exported) may run it in an owned scope.
    if entry_census_complete(file, function, lookup) || event_only_entries(file, function, lookup) {
        role
    } else {
        unknown
    }
}

/// Whether every way of entering `function` is one the write-role walk sees: a
/// call site [`SemanticLookup::function_call_sites`] resolves to it, or a
/// position of its own file the named-callback index classifies
/// ([`NamedCallbackRoles::positions`]). Every compiler reference to its
/// binding must be one of those, its declaration, or inside a type query, and
/// the census must be bounded at all: a method is entered by member access
/// through any receiver holding it, and an export by modules this build may
/// not see (an open program, or a module whose namespace escapes unnamed).
fn entry_census_complete(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
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
        bindings.iter().any(|binding| at == binding.span)
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
    while layer.span != outer.span {
        if layer.generator || !outer.span.contains(layer.span) {
            return false;
        }
        match layer_run(file, layer, lookup) {
            LayerRun::FreshStack => return fresh_stack_ok,
            LayerRun::InPlace(_) => {}
            LayerRun::Elsewhere | LayerRun::InPlaceAmongOthers(_) => return false,
        }
        // The layer's own span is outside its body, so the function that owns
        // it is the enclosing function, or the function whose parameter list
        // the layer is written in (ADR 0454: a layer in a default value runs
        // only when that function is called without the argument).
        let Some(next) = crate::owners::owning_function(&file.ast, layer.span) else {
            return false;
        };
        layer = next;
    }
    true
}

/// How a function layer is run by the code around it.
pub(super) enum LayerRun {
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
}

pub(super) fn layer_run(
    file: &solid_facts::FileFacts,
    layer: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> LayerRun {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    let iife = file
        .ast
        .calls
        .iter()
        .find(|call| file.ast.peel_ts_sugar_span(call.callee) == layer.span);
    let behavior = file
        .ast
        .arguments_containing(layer.span)
        .filter(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[*index].span) == layer.span
        })
        .find_map(|(call, index)| {
            let resolved = lookup.resolved_callee_call(file, call.callee)?;
            resolved
                .declaration
                .as_ref()
                .is_some_and(|declaration| declaration.standard_library)
                .then_some(())?;
            let callability = lookup
                .entity_at(file.path.as_str(), call.arguments[index].span)
                .and_then(|entity| entity.callability);
            crate::runtime_semantics::argument_behavior(resolved, callability, index)
                .map(|behavior| (call.span, behavior))
        });
    match (behavior, iife) {
        (Some((_, RuntimeArgumentBehavior::FreshStackCallback)), _) => LayerRun::FreshStack,
        (Some((call, RuntimeArgumentBehavior::InlineCallback)), _) => LayerRun::InPlace(call),
        (_, Some(call)) => LayerRun::InPlace(call.span),
        _ => match helper_runs_layer_inline(file, layer, lookup) {
            Some((call, ParameterUses::OnlyCalled)) => LayerRun::InPlace(call),
            Some((call, ParameterUses::AlsoElsewhere)) => LayerRun::InPlaceAmongOthers(call),
            None => LayerRun::Elsewhere,
        },
    }
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
            if lookup.primitive_at_call(file, call.span).is_some() {
                return None;
            }
            let (callee_file, callee, exact) =
                project_callee_at_parameter(file, call, index, lookup)?;
            if !exact {
                return None;
            }
            parameter_uses_during_call(callee_file, callee, index, lookup)
                .map(|uses| (call.span, uses))
        })
}

/// Whether the call at `callee` in `caller` is a plain call expression written
/// outside every JSX element of `caller`'s body.
fn plain_call_outside_jsx(
    file: &solid_facts::FileFacts,
    caller: &solid_facts::ast::FunctionFact,
    callee: Span,
) -> bool {
    file.ast
        .calls
        .iter()
        .any(|call| !call.construct && file.ast.peel_ts_sugar_span(call.callee) == callee)
        && !file
            .ast
            .jsx_containing(callee)
            .any(|element| caller.body.contains(element.span))
}

/// Whether the module publishes `function` itself: an export declaration of
/// it, with no other function between, or Type Facts' export flag on it.
fn module_exports(
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
fn event_only_entries(
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
