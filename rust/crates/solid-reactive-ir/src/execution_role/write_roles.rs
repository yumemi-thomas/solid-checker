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
        &mut HashSet::new(),
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
    /// The write sits directly in a callback that runs during the call under
    /// a root owner the call creates (`createRoot`'s body). The ambient owner
    /// is that root wherever the call is, so the call site does not matter:
    /// the operation's [`RootBodyGuard`] decides.
    CreatedRoot,
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
            if lookup
                .dialect
                .callback_runs_in_created_root(primitive, index)
                && callback_execution_at_call(file, call, primitive, index, lookup)
                    == Some(Execution::Inline)
            {
                return Some(WriteRegionAdjustment::CreatedRoot);
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
        .or_else(|| nested_in_leaf_scope(file, span, lookup))
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
            Some(WriteRegionAdjustment::CreatedRoot) => match root_body {
                RootBodyGuard::Rejects => return ExecutionRole::UntrackedRendering,
                RootBodyGuard::Exempts => return ExecutionRole::UntrackedCallback,
            },
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
                if let Some(role) = named_callback_write_role(
                    file,
                    span,
                    allowed,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    visiting,
                ) {
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
        &mut HashSet::from([write_classification_marker()]),
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
    // Code after an `await` in an async function runs on a later task, with no
    // owner; neither the lexical role nor the role at the function's call sites
    // describes it. Not proven to follow an await on every path (an `await`
    // inside a branch), so the answer is no claim rather than a legal write.
    // An implicit suspension (`for await`) is the same, and is not proven to
    // run at all; its callers' roles do not describe the code after it.
    if (direct == ExecutionRole::Unknown || direct.reports_disallowed_write())
        && (follows_await_in_async_function(file, span)
            || super::body_roles::follows_any_suspension(file, span))
    {
        return unknown;
    }
    if direct != ExecutionRole::Unknown {
        return direct;
    }
    let Some(function) = crate::owners::containing_ast_function(&file.ast, span) else {
        return unknown;
    };
    let key = (file.path.to_string(), function.span);
    if !visiting.insert(key.clone()) {
        return unknown;
    }
    let mut imperative = None;
    for (caller_file, callee) in lookup.function_call_sites(file.path.as_str(), function.span) {
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
            return role;
        }
        if role != ExecutionRole::Unknown {
            imperative.get_or_insert(role);
        }
    }
    visiting.remove(&key);
    imperative.unwrap_or(unknown)
}

/// Whether `span` sits in a function literal that is the value of a JSX
/// attribute, or in a function nested inside one.
pub(super) fn enclosed_by_jsx_attribute_function(
    file: &solid_facts::FileFacts,
    span: Span,
) -> bool {
    file.ast
        .functions_body_containing(span)
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
    file.ast.functions_body_containing(span).any(|function| {
        region.contains(function.span)
            && region != function.span
            && (jsx_attribute_value_function(file, function.span)
                || call_argument_invocation_unproven(file, function, lookup))
    })
}
