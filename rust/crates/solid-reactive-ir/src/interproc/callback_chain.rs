//! Callback chains: wrapper classification, composed execution, and forwarded ambient execution.

use super::*;

/// How one callback position schedules the code written inside it, relative to
/// the call that owns that position.
///
/// Four answers rather than the execution word's three, because "runs during
/// the call" and "reads inside it subscribe the caller" are separate facts and
/// the composition in [`compose_callback_chain`] needs both. The execution
/// word collapses them: `untrack` and `batch` are both `inline` there,
/// and only one of them stops an enclosing computation from tracking what runs
/// inside.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CallbackWrapper {
    /// Runs during the wrapping call and leaves the caller's tracking scope in
    /// place: 1.x `batch`, `startTransition`, `catchError`'s protected body,
    /// 2.0 `latest`/`isPending`/`flush`.
    Transparent,
    /// Runs during the wrapping call with the listener cleared: `untrack`,
    /// `createRoot`, `runWithOwner`, 2.0 `createRevealOrder`.
    Detaching,
    /// The wrapping call builds its own tracked computation around the code.
    ///
    /// The payload is when that computation runs relative to the wrapping
    /// call's return, and it is not derivable from "tracked": 1.x `createMemo`
    /// and `createRenderEffect` run it during the call while `createEffect`
    /// queues it, and 2.0 disagrees with 1.x on `createEffect`. `None` is the
    /// dialect refusing to say ([`solid_dialect::Dialect::tracked_callback_timing`]),
    /// or a summary row, whose `execution` word carries no schedule
    /// column at all.
    Tracked(Option<TrackedCallbackTiming>),
    /// The wrapping call schedules the code to run after it returns.
    ///
    /// `clears_tracking` is whether the code, when it does run, is proven to
    /// run with no caller's listener current: a dialect deferred slot the
    /// dialect states untracked ([`solid_dialect::Dialect::reports_untracked_reads_at`]),
    /// or a summary row that states `untracked` for a deferred
    /// invocation. "Runs later" alone is not that proof -- a returned closure
    /// runs inside whatever computation its caller is in -- so `false` is the
    /// default and publishes `ambient-at-execution`.
    Deferred { clears_tracking: bool },
}

/// The wrapper a callback position is, for callback summaries.
///
/// `None` is not "transparent": it is "this analysis cannot say what the
/// wrapping call does with the function it was handed", which is why
/// [`enclosing_callback_chain`] refuses the whole chain on it rather than
/// skipping the link.
pub(super) fn callback_wrapper_at(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<CallbackWrapper> {
    let count = call.arguments.len();
    let primitive = lookup
        .call_index(file, call.span)
        .and_then(|call_index| crate::known_primitive(&lookup.primitives(file).calls[call_index]));
    let primitive_execution =
        primitive_callback_execution(primitive, argument, count, lookup.dialect);
    // Only a primitive answers the slot: no other callee states what it does
    // with the function it is handed.
    let execution = primitive_execution?;
    Some(match execution {
        "deferred" => CallbackWrapper::Deferred {
            clears_tracking: match primitive {
                Some(primitive) => lookup
                    .dialect
                    .reports_untracked_reads_at(primitive, argument, count),
                None => false,
            },
        },
        "tracked" => CallbackWrapper::Tracked(primitive.and_then(|primitive| {
            lookup
                .dialect
                .tracked_callback_timing(primitive, argument, count)
        })),
        // A primitive answers the clearing question from the dialect.
        // `reports_untracked_reads_at` is consulted for the inline entry points
        // that clear the listener without being in the synchronous-clearing set
        // (2.0 `render` and `hydrate`).
        _ if primitive.is_some_and(|primitive| {
            lookup.dialect.runs_callback_synchronously(primitive)
                || lookup
                    .dialect
                    .reports_untracked_reads_at(primitive, argument, count)
        }) =>
        {
            CallbackWrapper::Detaching
        }
        _ => CallbackWrapper::Transparent,
    })
}

/// ADR 0183: whether the call at `span` is reached from `owner`'s own body
/// only through function literals that synchronous dialect slots run during
/// their call (`createRoot(() => createMemo(fn))`,
/// [`solid_dialect::Dialect::synchronous_callback_slot`]), each written as that
/// slot's whole argument. `Some(covered)` when it is, `covered` being whether
/// every level's call covers the normal completions of the body it sits in;
/// `None` for any other enclosure.
pub(super) fn synchronous_enclosure(
    file: &solid_facts::FileFacts,
    span: Span,
    owner: &SummaryNode,
    lookup: &SemanticLookup<'_>,
) -> Option<bool> {
    let cover = |body: Span, call: Span| {
        solid_facts::ast::completion_call_cover(
            std::path::Path::new(file.path.as_str()),
            &file.source,
            body,
            &[call],
            &[],
        ) == Some(true)
    };
    let mut span = span;
    let mut covered = true;
    for _ in 0..8 {
        let literal = crate::owners::containing_ast_function(&file.ast, span)?;
        if literal.body == owner.body {
            return Some(covered && cover(owner.body, span));
        }
        let (outer, index) = file.ast.arguments_containing(span).find(|(outer, index)| {
            let argument = &outer.arguments[*index];
            !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == literal.span
        })?;
        let primitive = lookup.primitive_at_call(file, outer.span)?;
        if !lookup
            .dialect
            .synchronous_callback_slot(primitive, index, outer.arguments.len())
        {
            return None;
        }
        covered = covered && cover(literal.body, span);
        span = outer.span;
    }
    None
}

/// ADR 0183: the eager owned-computation call (`createMemo(fn)`,
/// `createEffect(compute, effect)`, by
/// [`solid_dialect::Dialect::eager_owned_computation_slot`]) whose slot holds,
/// as its whole argument, the synchronous function literal `call` is written
/// directly in, when `call` runs on every normal completion of that literal's
/// body.
pub(super) fn wrapped_owned_computation_call<'f>(
    file: &'f solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<&'f solid_facts::ast::CallFact> {
    let literal = crate::owners::containing_ast_function(&file.ast, call.span)?;
    let (computation, index) =
        file.ast
            .arguments_containing(call.span)
            .find(|(outer, index)| {
                let argument = &outer.arguments[*index];
                !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == literal.span
            })?;
    let primitive = lookup.primitive_at_call(file, computation.span)?;
    (lookup
        .dialect
        .eager_owned_computation_slot(primitive, index, computation.arguments.len())
        && solid_facts::ast::completion_call_cover(
            std::path::Path::new(file.path.as_str()),
            &file.source,
            literal.body,
            &[call.span],
            &[],
        ) == Some(true))
    .then_some(computation)
}

/// The chain of callback positions between `nested` and the body of the
/// function that lexically owns the chain, innermost first.
///
/// `outermost` is the span the walk stopped at -- the last wrapping call, or
/// `nested` itself for an empty chain. Callers check that it sits in the
/// declaring function's own body before believing the composition: a closure
/// that is merely *returned* has an empty chain too, and reading an empty chain
/// as "runs inline" is the promise `callback-execution-boundary` exists to
/// prevent.
pub(super) struct CallbackChain {
    pub(super) wrappers: Vec<CallbackWrapper>,
    pub(super) outermost: Span,
}

/// `None` means a callback position exists that this analysis cannot classify,
/// so no composition over the chain is honest. `Some` with empty `wrappers`
/// means the walk found no callback position at all -- a different answer, and
/// the reason this is not an `Option<Vec<_>>`.
///
/// [`direct_callback_contains`] resolves exactly one level, so the walk is
/// iterative -- the same innermost-outward loop
/// `semantic_write_execution_role_within` uses for write regions.
pub(super) fn enclosing_callback_chain(
    file: &solid_facts::FileFacts,
    nested: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<CallbackChain> {
    let mut wrappers = Vec::new();
    let mut span = nested;
    // Bounded like every other span walk in this module: a malformed or
    // pathologically nested AST must not turn a summary into a hang.
    for _ in 0..32 {
        let mut enclosing = file
            .ast
            .arguments_containing(span)
            .filter(|(call, argument)| {
                direct_callback_contains(file, call.arguments[*argument].span, span)
            })
            .collect::<Vec<_>>();
        if enclosing.is_empty() {
            // An inline function expression can be one branch of the callee
            // itself: `(condition ? value => fn(value) : fn)(arg)`. It is not
            // a call argument, so the ordinary callback-position walk above
            // cannot see it, but selecting that branch invokes the function
            // synchronously and transparently. Continue from the direct call
            // so outer wrappers (for example createRoot/runWithOwner) are
            // still composed instead of misclassifying the nested `fn()` as
            // an escaped/deferred callback.
            let direct_branch_call = file
                .ast
                .functions_body_containing(span)
                .min_by_key(|function| function.body.end - function.body.start)
                .and_then(|function| {
                    file.ast
                        .calls
                        .iter()
                        .filter(|call| call.callee.contains(function.span))
                        .min_by_key(|call| call.callee.end - call.callee.start)
                });
            if let Some(call) = direct_branch_call {
                wrappers.push(CallbackWrapper::Transparent);
                span = call.span;
                continue;
            }
            break;
        }
        // Shortest argument span first: `f(g(() => x))` answers both calls for
        // `x`, and the immediately wrapping one is `g`'s.
        enclosing.sort_by_key(|(call, argument)| {
            let span = call.arguments[*argument].span;
            span.end - span.start
        });
        let (call, argument) = enclosing[0];
        wrappers.push(callback_wrapper_at(file, call, argument, lookup)?);
        span = call.span;
    }
    Some(CallbackChain {
        wrappers,
        outermost: span,
    })
}

/// Whether the chain's outermost wrapping call is written in `owner`'s own
/// body, which is what makes a composition over it a promise about `owner`.
pub(super) fn callback_chain_reaches_owner_body(
    file: &solid_facts::FileFacts,
    chain: &CallbackChain,
    owner: &SummaryNode,
) -> bool {
    crate::execution_role::functions_around(file, chain.outermost)
        .into_iter()
        .min_by_key(|function| function.span.end - function.span.start)
        .is_some_and(|innermost| {
            innermost.body == owner.body && owner.body.contains(chain.outermost)
        })
}

/// The execution word a callback carries when it runs synchronously at the
/// innermost end of `wrappers`.
///
/// Read innermost outward, because the order is the whole answer:
/// `untrack(() => createMemo(fn))` tracks `fn` -- the memo subscribes it and
/// the surrounding `untrack` cannot undo that -- while
/// `createEffect(() => untrack(fn))` does not, and defers instead. That second
/// shape is solid-js's own `onMount`, which the generator used to publish as
/// `tracked` because it read the lexical tracking scope rather than the
/// schedule relative to the export's return.
///
/// `None` is local openness: a wrapper in the chain has no established
/// schedule, so no word is honest. It arises for exactly one shape -- a
/// *detached* callback under a tracked wrapper whose
/// [`solid_dialect::Dialect::tracked_callback_timing`] the dialect does not
/// state. Once tracking is cleared, the tracked wrapper's remaining
/// contribution is only its schedule, and reading `Tracked` as "runs later" is
/// wrong for most of 1.x's tracked primitives: `createMemo(() => untrack(cb))`
/// runs `cb` during the call (`dist/solid.js:244-256`), so `deferred` there was
/// a claim the probe measures and fails. Where tracking is *not* cleared the
/// answer stays `tracked` regardless of schedule -- the attribution is the
/// claim, and every tracked computation eventually runs its compute.
/// The composed word, and whether the chain clears the caller's listener.
///
/// `detached` was computed here and thrown away, which is why every package
/// summary row recorded `tracking: "untracked"` whatever the export did: the
/// one place that knows a `Detaching` wrapper stood in the chain did not say
/// so. It is the difference between `untrack(fn)` and a bare `fn()`, and
/// without it a consumer reading the row cannot tell them apart.
/// The export-relative execution word, and whether a clearing wrapper stood
/// between the export and the callback.
pub(super) type ComposedExecution = (&'static str, bool);

/// A composed chain: what it says, beside the wrappers it composed from. The
/// wrappers are kept because `tracked` carries no schedule column and only the
/// dialect can supply one per wrapper.
pub(super) type ComposedChain = (Option<ComposedExecution>, Vec<CallbackWrapper>);

///
/// The clearing bit is defined per word, and only the wrappers that are still
/// on the callback's stack when it runs can set it:
///
/// - `inline`: a `Detaching` wrapper stood between the export and the callback;
/// - `deferred`: the deferral itself is proven to clear
///   ([`CallbackWrapper::Deferred`]'s `clears_tracking`), or a `Detaching`
///   wrapper sits *inside* it. A clearing wrapper *outside* a deferral has
///   returned by the time the callback runs, so it proves nothing: the old
///   fold set the bit for it, which was harmless only while every `deferred`
///   row published `untracked` whatever the bit said;
/// - `tracked`: never. The word is the attribution, and a clearing wrapper
///   outside the tracking one cannot undo its subscription -- so neither can it
///   leak into a later wrapper's reading, as `[tracked, detaching, tracked]`
///   once composed back to `inline`.
pub(super) fn compose_callback_chain(wrappers: &[CallbackWrapper]) -> Option<ComposedExecution> {
    let mut detached = false;
    let mut execution = "inline";
    for wrapper in wrappers {
        match (execution, wrapper) {
            ("inline", CallbackWrapper::Transparent) => {}
            ("inline", CallbackWrapper::Detaching) => detached = true,
            // A tracked wrapper subscribes what runs inside it -- unless a
            // clearing wrapper already stands between them, in which case what
            // is left of the wrapper is its schedule.
            ("inline", CallbackWrapper::Tracked(_)) if !detached => execution = "tracked",
            // Detached under a tracked wrapper: the schedule is the whole
            // remaining question, and only the dialect can answer it.
            ("inline", CallbackWrapper::Tracked(timing)) => {
                execution = match timing {
                    Some(TrackedCallbackTiming::DuringCall) => "inline",
                    Some(TrackedCallbackTiming::AfterCall) => "deferred",
                    None => return None,
                };
            }
            ("inline", CallbackWrapper::Deferred { clears_tracking }) => {
                execution = "deferred";
                detached |= *clears_tracking;
            }
            // Sticky: no outer wrapper can make a callback that runs later run
            // earlier. What runs later inside a tracked computation is still
            // subscribed by it, so the deferral clears nothing here.
            ("tracked", CallbackWrapper::Deferred { .. }) => execution = "deferred",
            // Attribution is decided, or the callback runs after every outer
            // wrapper has returned: nothing further out moves either answer.
            _ => {}
        }
    }
    Some((execution, detached && execution != "tracked"))
}

/// The export-relative schedule of a chain that composes to `tracked`.
///
/// [`compose_callback_chain`] answers *attribution* -- whose reads the callback
/// subscribes -- and the word `tracked` carries no schedule column, which is
/// why one is computed here instead of read out of it. Every wrapper between
/// the callback and the export body contributes, and only the dialect knows
/// which:
///
/// - a tracked wrapper the dialect says merely *queues* its computation
///   ([`TrackedCallbackTiming::AfterCall`] -- 1.x `createEffect`, 2.0
///   `createTrackedEffect`) puts the callback after the export's return
///   whatever else stands around it, so it wins outright;
/// - a chain whose tracked wrappers all run during their own call
///   ([`TrackedCallbackTiming::DuringCall`] -- 1.x `createMemo`/`mergeProps`,
///   2.0 `createSignal`/`createMemo`/`createOptimistic`) leaves the callback on
///   the export's own stack;
/// - a wrapper the dialect states no timing for
///   ([`solid_dialect::Dialect::tracked_callback_timing`] answering `None`, as
///   2.0 does for `createStore` because no measurement backs a claim) leaves
///   the schedule unestablished. The row survives -- attribution is proven and
///   is a real claim -- but it is emitted with no execution point at all rather
///   than with a guessed one.
///
/// `Transparent`, `Detaching` and `Deferred` wrappers do not appear: the first
/// two run their code during their own call and the third would have made the
/// composed word `deferred`, not `tracked`.
pub(super) fn composed_tracked_schedule(wrappers: &[CallbackWrapper]) -> CallbackSchedule {
    let mut schedule = CallbackSchedule::SameStack;
    for wrapper in wrappers {
        if let CallbackWrapper::Tracked(timing) = wrapper {
            match timing {
                Some(TrackedCallbackTiming::AfterCall) => return CallbackSchedule::Queued,
                Some(TrackedCallbackTiming::DuringCall) => {}
                None => schedule = CallbackSchedule::Unestablished,
            }
        }
    }
    schedule
}

/// What the wrappers around a forwarding call say about a callback the callee's
/// own summary reports as `inline`.
///
/// Three answers, because "no chain fact" and "the chain refuses to answer" are
/// opposites and collapsing them is how a refusal turns back into a claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ForwardedAmbientExecution {
    /// Retain the callee's own answer. Local-helper argument forwarding uses
    /// this only in the parameter owner's body with no enclosing wrappers.
    Callee,
    /// The wrappers between the forwarding call and the export body,
    /// innermost first, which compose to an export-relative execution.
    ///
    /// The wrappers are carried rather than their composition, because the
    /// composition is not complete until the callee's own row is known: the
    /// callee is the innermost wrapper of all. `runUntracked(fn) { return
    /// untrack(fn) }` publishes `inline` + clearing, and under
    /// `createMemo(() => runUntracked(cb))` the composition has to start from
    /// that clearing -- composing the enclosing chain alone and substituting
    /// its answer for the callee's is how the proven clear was dropped and the
    /// row published `tracked`. See [`ForwardedAmbientExecution::restate`].
    Composed { wrappers: Vec<CallbackWrapper> },
    /// The enclosing execution or a wrapper's schedule is unestablished, so no
    /// export-relative word is honest. The callee's `inline` rows must not be
    /// republished and the callback leaf stays open instead.
    Unknown,
}

/// The ambient execution to apply to a callback forwarded into `call` at
/// `argument`, when the callee's own summary says it invokes the callback
/// inline.
///
/// The chain starts at the forwarding call's *own* position, not above it: for
/// `createEffect(() => untrack(fn))` the callee is `untrack`, and "untrack
/// clears the listener" is the fact that stops the enclosing `createEffect`
/// from making `fn` tracked. Without that link the ambient answer was the
/// enclosing computation's lexical role, which is the export's tracking scope
/// rather than the callback's schedule.
pub(super) fn forwarded_callback_ambient_execution(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    argument: usize,
    owner: &SummaryNode,
    lookup: &SemanticLookup<'_>,
) -> ForwardedAmbientExecution {
    let own = callback_wrapper_at(file, call, argument, lookup);
    // An unknown wrapper is not an empty chain. Even an empty, classified
    // chain can stop inside a stored arrow rather than reach the parameter's
    // declaring function. Neither establishes export-relative timing.
    let Some(chain) = enclosing_callback_chain(file, call.span, lookup)
        .filter(|chain| callback_chain_reaches_owner_body(file, chain, owner))
    else {
        return ForwardedAmbientExecution::Unknown;
    };
    let above = chain.wrappers;
    // A local helper invoked from a tracked computation that starts after the
    // wrapping call is not enough to restate the helper's `inline` row as a
    // tracked export callback. Solid 1's first createEffect run can execute
    // the maybe-accessor through the caller's still-active computation (the
    // generic probe observes the caller re-run), while later runs belong to
    // the effect. Schema v1 has one word for the parameter and cannot express
    // that split, so keep it unknown. A callback passed directly to the Solid
    // primitive does not use this forwarding seam and retains its dialect row.
    if matches!(own, None | Some(CallbackWrapper::Transparent))
        && above.iter().any(|wrapper| {
            *wrapper == CallbackWrapper::Tracked(Some(TrackedCallbackTiming::AfterCall))
        })
    {
        return ForwardedAmbientExecution::Unknown;
    }
    if own.is_none() && above.is_empty() {
        return ForwardedAmbientExecution::Callee;
    }
    let mut wrappers = own.into_iter().collect::<Vec<_>>();
    wrappers.extend(above);
    // Composed here once for a transparent callee, which is the answer the
    // sentinel is opened on at the forwarding call. A clearing callee is
    // recomposed in `restate`, where its row is known.
    match compose_callback_chain(&wrappers) {
        Some(_) => ForwardedAmbientExecution::Composed { wrappers },
        None => ForwardedAmbientExecution::Unknown,
    }
}

impl ForwardedAmbientExecution {
    /// A callee's `inline` row, restated relative to the export through the
    /// wrappers around the forwarding call.
    ///
    /// The callee's row is the innermost wrapper: `Detaching` when it proves a
    /// clearing, `Transparent` when it does not. `None` is a composition that
    /// refuses -- a clearing callee under a tracked wrapper whose schedule the
    /// dialect does not state -- and the caller opens the parameter's sentinel
    /// rather than publishing either guess. Only `Composed` restates; the other
    /// two answers are the caller's to handle before asking.
    pub(super) fn restate(
        &self,
        callee_clears_tracking: bool,
    ) -> Option<(String, Option<CallbackSchedule>, bool)> {
        let Self::Composed { wrappers } = self else {
            return None;
        };
        let mut chain = Vec::with_capacity(wrappers.len() + 1);
        chain.push(if callee_clears_tracking {
            CallbackWrapper::Detaching
        } else {
            CallbackWrapper::Transparent
        });
        chain.extend(wrappers.iter().copied());
        let (execution, clears_tracking) = compose_callback_chain(&chain)?;
        Some((
            execution.to_owned(),
            (execution == "tracked").then(|| composed_tracked_schedule(&chain)),
            clears_tracking,
        ))
    }
}

/// The execution recorded for a callback forwarded into a primitive, in the
/// execution-word vocabulary.
///
/// The effect pair derives from the dialect, because its phases are the
/// headline dialect difference: 2.0 has a deferred apply argument, 1.x has a
/// tracked callback and a seed value.
///
/// `untrack` and 2.0's `flush` sit in the `"inline"` arm beside `createRoot`
/// and `runWithOwner` (only `flush` leaves the caller's listener current),
/// which is what the execution-word vocabulary means by the word:
/// `inline` and `deferred` are the *schedule* axis and describe only callbacks
/// the export does not subscribe, while the clearing fact travels separately
/// through [`solid_dialect::Dialect::runs_callback_synchronously`]
/// ("one word over two axes"). This module used to answer `"deferred"` for the pair on the grounds that a consumer reads
/// `"deferred"` as "not tracked here" -- but `"deferred"` is also a promise
/// that the callback does *not* run before the export returns, and every one of
/// these runs it during the call, so the divergence recorded claims the
/// runtime contradicts. The "not tracked" half is now carried by
/// [`callback_wrapper_at`], which reads the clearing fact from the dialect
/// instead of encoding it in the word.
///
/// This table is also the *reach* of the wrapper-chain fold: a primitive with no
/// row here cannot be classified as a wrapper at all, so a chain containing one
/// is refused ([`enclosing_callback_chain`]) and the row falls back to the
/// lexical answer. `startTransition` and `createResource` are deliberately
/// absent for that reason -- their dialect `Execution` states attribution and
/// not a schedule, and restating it as one is the mistake the fold exists to
/// avoid. `batch`, `createComputed`, `onMount`, `catchError`, `children` and the
/// rest are absent because nobody has established their schedule here yet.
///
/// **A row here is not permission to record a callback claim.** This table has
/// two consumers whose polarity is opposite. The wrapper-chain fold needs a row
/// to classify the position at all, and a *missing* row makes the chain refuse
/// so the inner slot's own answer stands -- a stronger claim, not a weaker one.
/// Under Solid 1.x that was the case for `createSignal(fn)`, which *stored* the
/// function. **2.0 has no such shape**: it models `createSignal`'s compute slot
/// with a `Tracked` row, so the chain answers rather than refusing. A genuine
/// refusal needs a primitive this dialect states no timing for --
/// `createStore` and `createOptimisticStore` are the documented candidates.
/// The callback-summary pass in the interprocedural graph wants the opposite
/// reading, so the premises it needs before rooting an `invoke` claim on a
/// forwarded parameter live there, in [`primitive_slot_roots_parameter_invoke`],
/// and never by deleting a row from here.
pub(super) fn primitive_callback_execution(
    primitive: Option<Primitive>,
    parameter: usize,
    argument_count: usize,
    dialect: &dyn solid_dialect::Dialect,
) -> Option<&'static str> {
    let primitive = primitive?;
    dialect
        .callback_execution_word_at(primitive, parameter, argument_count)
        .map(|execution| match execution {
            solid_dialect::Execution::Tracked => "tracked",
            solid_dialect::Execution::Deferred => "deferred",
            solid_dialect::Execution::Inline => "inline",
        })
}

/// Whether a primitive callback slot may *root* an export-level `invoke` claim
/// on a parameter forwarded into it.
///
/// [`primitive_callback_execution`] answers "how would a callback at this slot
/// run relative to the export", which presupposes there is a callback there. The
/// callback summary needs that presupposition established, and it used to
/// record the row without it. Two shapes in the measured ecosystem produced an
/// `invoke` claim about a value the shipped code never invokes -- a demand no
/// evidence can ever discharge, so it stays open forever and correctly:
///
/// - `@solid-primitives/flux-store`'s
///   `createFluxStore(initialState, createMethods)` claimed to invoke
///   `initialState`, because `createStore(initialState)` sits at
///   `(CreateStore, 0)`. Its real callbacks are `createMethods.getters` and
///   `createMethods.actions` -- argument 1 behind a property path, which a
///   [`CallbackSummary`] cannot spell at all.
/// - `@solidjs/meta`'s
///   `Stylesheet = props => createComponent(Link, mergeProps({rel}, props))`,
///   declared `Component<Omit<JSX.LinkHTMLAttributes<HTMLLinkElement>, "rel">>`,
///   claimed to invoke `props`, because `(MergeProps, _)` matches every
///   argument. `@solidjs/router`'s `A` and `Route` are the same shape, and so is
///   every other Solid component that forwards its props through `mergeProps`.
///
/// Three premises, each about the artifact rather than about a package name.
///
/// **The value must be able to hold a function.** A parameter the artifact's
/// types prove non-callable is invoked by no slot. `Callability::Unknown` is the
/// absence of an answer and withdraws nothing, so an untyped JavaScript
/// distribution keeps whatever its shape earns below. The general
/// (non-primitive) branch further down this same loop already requires exactly
/// this; the primitive branch short-circuited past it.
///
/// **A slot whose behavior is conditional on callability needs callability
/// proven, not merely un-refuted.** `mergeProps` memoizes a merge source *if*
/// that source is a function and copies it otherwise; 2.0's `createSignal`,
/// `createStore`, `createOptimistic` and `createOptimisticStore` each have a
/// plain form whose first parameter is declared to exclude functions
/// (`Exclude<T, Function>`, `NoFn<T>`) beside a derived form that takes a
/// compute. For those, an unproven callability is the *missing* premise, and
/// recording the row anyway is manufacturing a positive claim out of absent
/// knowledge. `createMemo`, `createProjection`, `createTrackedEffect` and
/// `dynamic` are unconditional -- argument 0 is the compute in every overload --
/// so they keep the weak premise and keep working on untyped artifacts.
///
/// **The dialect must own the slot.** These are 2.0's names and positions.
/// Solid 1.x has `createStore` too, but only as
/// `createStore(store?, options?)`: no compute form exists, and 1.x's own
/// [`solid_dialect::Dialect::callback_executions`] publishes no row for it, nor
/// for `createSignal`, whose 1.x form *stores* a function argument as the
/// signal's value
/// ([`solid_dialect::Dialect::stores_function_argument_as_value`]).
///
/// `mergeProps` never reaches that check, and not because the vocabulary is
/// silent about it. 2.0 spells its own primitive [`Primitive::Merge`] and
/// carries no `MergeProps` row at all, so the name does not resolve here --
/// which is not the same answer as "resolves, and takes no callback there".
/// What actually decides `mergeProps` is the conditional-callability premise
/// above, which is the runtime's own dispatch.
///
/// (While the 1.x vocabulary was compiled in this paragraph also had to
/// explain its variadic `mergeProps(...sources)` answer, which came ahead of
/// the [`solid_dialect::Dialect::callback_executions`] table. That vocabulary
/// was retired in ADR 0110.)
///
/// **There is deliberately no arity premise.** The 2.0 runtime dispatches
/// `createStore`'s first argument on `typeof first === "function"` alone
/// (`@solidjs/signals` `dist/dev.js:9371`;
/// `solid-js@2.0.0-rc.3 dist/server.js:896` routes the same shape through
/// `createProjection`, and `createOptimisticStore` at `:912` delegates to
/// `createStore`), with no seed-argument condition -- and the published *server*
/// entrypoint declares the plain form `createStore<T extends object>(store: T |
/// Store<T>, options?)` with no `NoFn`, so a one-argument
/// `createStore(compute)` is `tsc`-clean and really is the derived form. An
/// arity clause here therefore dropped a true claim, which is why the premise
/// is callability and only callability: claim exactly when the slot's value is
/// *proven* callable.
pub(super) fn primitive_slot_roots_parameter_invoke(
    dialect: &dyn solid_dialect::Dialect,
    primitive: Option<Primitive>,
    argument_index: usize,
    argument_count: usize,
    callability: Option<Callability>,
) -> bool {
    use Primitive as P;
    if !potentially_callable(callability) {
        return false;
    }
    let Some(primitive) = primitive else {
        return true;
    };
    let conditional_on_callability = matches!(primitive, P::MergeProps)
        || (argument_index == 0
            && matches!(
                primitive,
                P::CreateSignal | P::CreateStore | P::CreateOptimistic | P::CreateOptimisticStore
            ));
    if conditional_on_callability
        && !matches!(
            callability,
            Some(Callability::Callable | Callability::UntypedCallable)
        )
    {
        return false;
    }
    // 1.x owns `createStore` and `createSignal` with no compute form at all, so
    // its own slot table is the authority on whether the slot exists here.
    // `mergeProps` never reaches this check: 1.x answers it above the table and
    // 2.0 does not carry the name at all, so callability alone carries it.
    if matches!(
        primitive,
        P::CreateSignal | P::CreateStore | P::CreateOptimistic | P::CreateOptimisticStore
    ) {
        return dialect
            .callback_execution_at(primitive, argument_index, argument_count)
            .is_some();
    }
    true
}
