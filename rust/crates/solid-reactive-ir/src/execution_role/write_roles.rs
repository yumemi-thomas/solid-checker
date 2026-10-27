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
    /// The write sits directly in an inline callback whose call may create a
    /// root this vocabulary has not audited
    /// ([`solid_dialect::Dialect::callback_root_is_unaudited`]). Whether it runs
    /// under a root is unknown, so the write is unplaced.
    UnauditedRoot,
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
    let role = semantic_write_role_walk(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        root_body,
        visiting,
    );
    if role != ExecutionRole::Unknown || file.compiler.withheld() {
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
            Some(WriteRegionAdjustment::CreatedRoot) => {
                if let Some(role) = suspended_write_role(file, span, lookup) {
                    return role;
                }
                match root_body {
                    RootBodyGuard::Rejects => return ExecutionRole::UntrackedRendering,
                    RootBodyGuard::Exempts => return ExecutionRole::UntrackedCallback,
                }
            }
            // Whether a root wraps this callback is not audited on this
            // release, so the write has no placement the checker can prove.
            Some(WriteRegionAdjustment::UnauditedRoot) => return ExecutionRole::Unknown,
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
    // (ADR 0450): the scope runs its construction, not its body. The closure's
    // own invocations decide, through the walk below.
    let tracked_named = if direct == ExecutionRole::TrackedJsx {
        named_closure_in_tracked_scope(file, span, allowed, entities, symbol_names, lookup)
    } else {
        None
    };
    let direct = if tracked_named.is_some() {
        ExecutionRole::Unknown
    } else {
        direct
    };
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
    // ADR 0450: the named closure built in a tracked scope is entered by its
    // calls, and the write runs with it only through layers that run in place
    // (an IIFE, an inline standard-library callback). Any other layer keeps or
    // schedules the write's function, whose entries are not the closure's.
    //
    // A site in a parameter list (a default value) runs when its function is
    // called without that argument. Such a function holds a call or a read, so
    // its parameter list is not inert and the call graph withholds its call
    // sites (ADR 0339): nothing places the write, which stays unplaced.
    let in_default = functions_around(file, span)
        .into_iter()
        .any(|function| parameter_slot(function, span).is_some());
    let entry = match tracked_named {
        Some(_) if in_default => return unknown,
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
        if let Some(caller) = crate::owners::containing_ast_function(&caller_file.ast, callee)
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
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    let mut layer = inner;
    while layer.span != outer.span {
        if layer.generator || !outer.span.contains(layer.span) {
            return false;
        }
        let iife = file
            .ast
            .calls
            .iter()
            .any(|call| file.ast.peel_ts_sugar_span(call.callee) == layer.span);
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
            });
        match behavior {
            Some(RuntimeArgumentBehavior::FreshStackCallback) => return fresh_stack_ok,
            Some(RuntimeArgumentBehavior::InlineCallback) => {}
            _ if iife => {}
            _ => return false,
        }
        // The layer's own span is outside its body, so the innermost body
        // containing it is the enclosing function's.
        let Some(next) = file.ast.innermost_function_body_containing(layer.span) else {
            return false;
        };
        layer = next;
    }
    true
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
