//! Owner context lookups: providing regions, callback indexes, await continuations, and requirement pushes.

use super::*;

/// The owner context bits that take a function out of the synchronous extent
/// of an owner-providing region written around it. See [`root_owned_at`].
pub(super) const OWNER_CONTEXT_LEAVES_REGION: u8 = OWNER_CONTEXT_PROVEN_UNOWNED
    | OWNER_CONTEXT_LATER_RUN_UNOWNED
    | OWNER_CONTEXT_LEAF
    | OWNER_CONTEXT_MAY_RESUME_UNOWNED;

/// The owner context bits that prove a function runs on some path with an
/// owner current: an owner edge reached it. See [`region_run_unproven`].
pub(super) const OWNER_CONTEXT_RUNS_OWNED: u8 = OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF_OWNED;

/// The bits that describe a context's unowned part and its provenance: what
/// an owner guard removes ([`OwnerEdgeKind::OwnerGuarded`]) and a no-owner
/// guard keeps ([`OwnerEdgeKind::NoOwnerGuarded`]).
const OWNER_CONTEXT_UNOWNED_PART: u8 = OWNER_CONTEXT_UNOWNED
    | OWNER_CONTEXT_PROVEN_UNOWNED
    | OWNER_CONTEXT_LATER_RUN_UNOWNED
    | OWNER_CONTEXT_MAY_RESUME_UNOWNED
    | OWNER_CONTEXT_COMPONENT_UNCERTAIN;

/// The innermost owner-providing region around `span`: `None` when no region
/// contains it. The innermost region is the only one that needs asking: an
/// outer region's chain is a superset of it, so an outer region answers only
/// where the innermost does.
pub(crate) fn providing_region(providing_regions: &[Span], span: Span) -> Option<Span> {
    providing_regions
        .iter()
        .filter(|region| region.contains(span))
        .min_by_key(|region| region.end - region.start)
        .copied()
}

/// The functions standing between `span` and the innermost owner-providing
/// region around it ([`providing_region`]): every function whose body contains
/// `span` and which is itself written inside that region, the region's own
/// callback included. `None` when no region contains `span`.
///
/// `functions` yields `(key, span, body)` for the file's owner nodes; the key
/// comes back unchanged, so a caller can carry node indices or function spans.
pub(crate) fn providing_region_chain<K>(
    providing_regions: &[Span],
    functions: impl IntoIterator<Item = (K, Span, Span)>,
    span: Span,
) -> Option<Vec<K>> {
    let region = providing_region(providing_regions, span)?;
    Some(
        functions
            .into_iter()
            .filter(|(_, function, body)| body.contains(span) && region.contains(*function))
            .map(|(key, _, _)| key)
            .collect(),
    )
}

/// Whether an operation lexically inside an owner-providing region is answered
/// by that region's owner. `chain` is [`providing_region_chain`]'s answer and
/// `context_of` each chain function's propagated owner context. Both owner
/// passes, every operation kind, ask this one question.
///
/// The region's owner is current only for code that runs in the region's
/// synchronous extent, and a function nested in the region leaves that extent
/// when its context carries one of the bits of [`OWNER_CONTEXT_LEAVES_REGION`]:
///
/// - [`OWNER_CONTEXT_LATER_RUN_UNOWNED`], from a `createRenderEffect` apply
///   ([`solid_dialect::CallbackOwner::InheritsFirstRun`]). `@solidjs/signals`
///   2.0.0-rc.3 and rc.9 (dev and prod client builds) run every apply after
///   the first from the flush with `getOwner() === null`, so an `onCleanup`
///   there under a `createRoot` never runs on dispose and raises
///   `NO_OWNER_CLEANUP`, while the first run's cleanup registers on the root.
/// - [`OWNER_CONTEXT_PROVEN_UNOWNED`], from a callback some invocation of
///   which runs with no owner: a [`solid_dialect::CallbackOwner::None`]
///   position (a `createEffect` apply, a `createReaction` invalidation, a
///   `runWithOwner(null, fn)` callback), a compiler event-handler or
///   directive-apply role, or an `unowned` summary row. Probed on
///   `solid-js`/`@solidjs/signals`/`@solidjs/web` 2.0.0-rc.3 and rc.9, dev and
///   prod client builds, each written inside a `createRoot` (the delegated
///   handler inside a `render` callback): `getOwner()` is `null` in every one
///   of them, the `onCleanup` raises `NO_OWNER_CLEANUP` (dev) and never runs
///   on dispose, and an `onSettled` in the `createEffect` apply runs
///   out-of-band, its returned cleanup raising `SETTLED_CLEANUP_UNOWNED`.
/// - [`OWNER_CONTEXT_LEAF`], from a leaf body. An owned leaf runs its body
///   under the leaf's own owner, which forbids children, so the region's owner
///   is not current there: an `onCleanup` or a primitive in the body throws
///   `CLEANUP_IN_FORBIDDEN_SCOPE` or `PRIMITIVE_IN_FORBIDDEN_SCOPE`, which SC3001
///   claims, and a `createRoot` around the leaf does not answer for it. A
///   queued `onSettled` body has no owner and is withheld by the bit above.
///
/// Such an operation is judged on the owner graph instead, which makes it a
/// violation where the propagated context is proven unowned and uncertifiable
/// where it carries only the later-run bit. A leaf body is judged by the leaf
/// rules instead.
///
/// Only functions *inside* the region are asked. A region written inside the
/// detached function -- a `createRoot` created in a `createEffect` apply, or a
/// compiled JSX child, whose generated render effect owns its expression --
/// answers for what it contains: probed, an `onCleanup` in either runs on that
/// owner's disposal or re-run.
///
/// A callback handed to a reviewed fresh-stack host scheduler (`setTimeout`,
/// `queueMicrotask`, `Promise.then`, an observer constructor, ...) gets the
/// dialect's unowned edge ([`fresh_stack_scheduler_edges`]) and is judged on
/// the graph like the positions above. A nested callback with no owner edge at
/// all, or only a possible-unowned one, carries neither bit; whether the
/// region answers for it is [`region_run_unproven`]'s question (ADR 0534).
pub(crate) fn root_owned_at<K>(chain: Option<&[K]>, context_of: impl Fn(&K) -> u8) -> bool {
    chain.is_some_and(|chain| {
        chain
            .iter()
            .all(|function| context_of(function) & OWNER_CONTEXT_LEAVES_REGION == 0)
    })
}

/// ADR 0534: whether a chain [`root_owned_at`] answers for holds a function
/// whose run inside the region nothing proves. The region's owner is current
/// for an operation only while the region's callback runs it synchronously; a
/// function nested in the region that no owner edge reaches (context zero: a
/// closure handed to a wrapper that stores it or hands it to a scheduler, a
/// class constructor or method, a getter, a function assigned to a member) or
/// that only a possible-unowned edge reaches ([`OwnerEdgeKind::MayBeUnowned`],
/// no bit of [`OWNER_CONTEXT_RUNS_OWNED`]) may run from a timer or a handler
/// instead, with no owner (rc.17 warns `NO_OWNER_CLEANUP`/`NO_OWNER_EFFECT`
/// there). Such an operation is unplaced: an obligation, never certified by
/// position. Two functions are exempt: the region's own callback (`is_region`),
/// which provides the owner whenever it runs, however it was entered; and a
/// function proven never to run (`unreached`, ADR 0458's query), whose
/// operation never happens and keeps the region's answer as before.
pub(crate) fn region_run_unproven<K>(
    chain: &[K],
    context_of: impl Fn(&K) -> u8,
    is_region: impl Fn(&K) -> bool,
    unreached: impl Fn(&K) -> bool,
) -> bool {
    chain.iter().any(|function| {
        context_of(function) & OWNER_CONTEXT_RUNS_OWNED == 0
            && !is_region(function)
            && !unreached(function)
    })
}

/// The function literal a callback argument is, when the argument *is* that
/// literal: the argument with parentheses and TypeScript wrappers removed.
/// Any other expression hands the callback over as a value. A call whose
/// result may be a function, an IIFE, or `untrack(() => …)` runs its inner
/// literals where the surrounding expression runs, not as the callback, so a
/// literal inside one of them is not named (SC4001 review D).
///
/// The one exception is an event handler's tuple: an event attribute may take
/// `[handler, data]`, whose first slot the DOM calls as the handler (the
/// two-slot bound-handler tuple `projection` names, `review-r7-event-array`).
/// `handler_tuple` says the argument sits in an event handler position; no
/// other position reads an array as its callback.
pub(crate) fn callback_literal_span(
    file: &solid_facts::FileFacts,
    argument: Span,
    handler_tuple: bool,
) -> Option<Span> {
    let is_function = |span: Span| {
        file.ast
            .functions
            .iter()
            .any(|function| function.span == span)
    };
    let peeled = file.ast.peel_ts_sugar_span(argument);
    if is_function(peeled) {
        return Some(peeled);
    }
    if !handler_tuple {
        return None;
    }
    let array = file
        .ast
        .array_literal_elements
        .iter()
        .find(|fact| file.ast.peel_ts_sugar_span(fact.span) == peeled)?;
    let first = array
        .elements
        .first()
        .filter(|element| element.kind == solid_facts::ast::ArrayElementKind::Value)?;
    let first = file.ast.peel_ts_sugar_span(first.span);
    is_function(first).then_some(first)
}

/// The callback value a JSX event slot receives, including tuple element 0.
/// Unknown/spread tuple elements keep the original span for refusal.
pub(crate) fn handler_value_span(file: &solid_facts::FileFacts, value: Span) -> Span {
    let value = file.ast.peel_ts_sugar_span(value);
    file.ast
        .array_literal_elements
        .iter()
        .find(|array| file.ast.peel_ts_sugar_span(array.span) == value)
        .and_then(|array| array.elements.first())
        .filter(|element| element.kind == solid_facts::ast::ArrayElementKind::Value)
        .map_or(value, |element| file.ast.peel_ts_sugar_span(element.span))
}

/// The context an `onSettled` callback takes when its call sits outside every
/// leaf body ([`OwnerEdgeKind::QueuedLeaf`]). Under a children-capable owner the
/// callback runs under its leaf owner, [`OWNER_CONTEXT_LEAF_OWNED`]; with no
/// owner it keeps the unowned bits, which make its cleanup SC4001's claim.
/// Inside a leaf body the queue applies instead.
const fn leaf_body_context(source: u8) -> u8 {
    let unowned = source & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED);
    if source & OWNER_CONTEXT_OWNED != 0 {
        OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED | unowned
    } else {
        OWNER_CONTEXT_LEAF | unowned
    }
}

/// A `createTrackedEffect` body always runs under the tracked node, which
/// forbids children, whether or not the effect itself was created under an
/// owner, so it carries [`OWNER_CONTEXT_LEAF_OWNED`] and never the unowned bits:
/// an `onCleanup` there throws `CLEANUP_IN_FORBIDDEN_SCOPE` (SC3001's claim),
/// not `NO_OWNER_CLEANUP`. It is dead only when every path to its creation is
/// inside an owned leaf body: the effect throws before its callback runs, so it
/// carries no context and nothing it contains is claimed (ADR 0347).
const fn tracked_leaf_body_context(source: u8) -> u8 {
    let live = source & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED) != 0
        || source & OWNER_CONTEXT_LEAF_OWNED == 0;
    if live {
        OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED
    } else {
        0
    }
}

pub(crate) const fn owner_edge_context(kind: OwnerEdgeKind, source: u8) -> u8 {
    match kind {
        OwnerEdgeKind::Preserve | OwnerEdgeKind::EagerCall => source,
        OwnerEdgeKind::MayResumeUnowned => {
            if source == 0 || source & OWNER_CONTEXT_PROVEN_UNOWNED != 0 {
                source
            } else {
                source | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_MAY_RESUME_UNOWNED
            }
        }
        OwnerEdgeKind::Owned => OWNER_CONTEXT_OWNED,
        OwnerEdgeKind::Unowned => OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
        // Only a generator that runs resumes: context zero stays zero.
        OwnerEdgeKind::ResumesUnowned => {
            if source == 0 {
                0
            } else {
                OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED
            }
        }
        OwnerEdgeKind::Conditional => OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED,
        // Not proven unowned: a later run happens only if the primitive runs
        // the callback again, which needs a change nothing here proves. A
        // source that is itself proven unowned stays proven: then every run,
        // first or later, is unowned.
        OwnerEdgeKind::InheritsFirstRun => {
            source | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LATER_RUN_UNOWNED
        }
        // A createTrackedEffect callback runs under the tracked node whatever
        // its creation owner, so its onCleanup is SC3001's claim. See
        // [`tracked_leaf_body_context`].
        OwnerEdgeKind::Leaf => tracked_leaf_body_context(source),
        // Inside a leaf body `onSettled` takes the out-of-band queue whatever
        // the body's owner (rc.13), so its callback runs with no owner: proven
        // unowned. A run-owned part of the source still creates a leaf. Outside
        // one it is an ordinary leaf ([`leaf_body_context`]).
        OwnerEdgeKind::QueuedLeaf => {
            if source & OWNER_CONTEXT_LEAF != 0 {
                let run_owned = if source & OWNER_CONTEXT_OWNED != 0 {
                    OWNER_CONTEXT_LEAF_OWNED
                } else {
                    0
                };
                OWNER_CONTEXT_LEAF
                    | OWNER_CONTEXT_UNOWNED
                    | OWNER_CONTEXT_PROVEN_UNOWNED
                    | run_owned
            } else {
                leaf_body_context(source)
            }
        }
        OwnerEdgeKind::QueuedLeafDrained => {
            let queued = if source & OWNER_CONTEXT_LEAF != 0 {
                OWNER_CONTEXT_LEAF | OWNER_CONTEXT_UNOWNED
            } else {
                leaf_body_context(source)
            };
            queued & !OWNER_CONTEXT_PROVEN_UNOWNED
        }
        OwnerEdgeKind::RefApplied => OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
        // ADR 0613: the drain's caller decides, not the creating call's
        // owner: possibly owned, possibly not, never proven.
        OwnerEdgeKind::DrainedByFlush => {
            OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LATER_RUN_UNOWNED
        }
        OwnerEdgeKind::MayBeUnowned => OWNER_CONTEXT_UNOWNED,
        OwnerEdgeKind::MayRun => source & !OWNER_CONTEXT_PROVEN_UNOWNED,
        OwnerEdgeKind::OwnerGuarded => source & !OWNER_CONTEXT_UNOWNED_PART,
        OwnerEdgeKind::NoOwnerGuarded | OwnerEdgeKind::ProvenNoOwnerGuarded => {
            if source & OWNER_CONTEXT_UNOWNED == 0 {
                0
            } else if matches!(kind, OwnerEdgeKind::ProvenNoOwnerGuarded)
                && source & OWNER_CONTEXT_LEAF == 0
            {
                source & OWNER_CONTEXT_UNOWNED_PART
            } else {
                source & OWNER_CONTEXT_UNOWNED_PART & !OWNER_CONTEXT_PROVEN_UNOWNED
            }
        }
    }
}

pub(super) fn event_owner_edge(lookup: &SemanticLookup<'_>, generator: bool) -> OwnerEdgeKind {
    if generator {
        OwnerEdgeKind::MayBeUnowned
    } else if lookup.synchronous_event_dispatch_present() {
        OwnerEdgeKind::Conditional
    } else {
        OwnerEdgeKind::Unowned
    }
}

/// The owner context of an operation that runs after an `await`
/// ([`AwaitContinuations`]): proven unowned on every run, whatever context
/// the enclosing function was entered with.
pub(super) const fn await_continuation_context(context: u8) -> u8 {
    (context | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED)
        & !(OWNER_CONTEXT_OWNED
            | OWNER_CONTEXT_LEAF
            | OWNER_CONTEXT_LEAF_OWNED
            | OWNER_CONTEXT_COMPONENT_UNCERTAIN
            | OWNER_CONTEXT_LATER_RUN_UNOWNED
            | OWNER_CONTEXT_MAY_RESUME_UNOWNED)
}

/// ADR 0357: a skippable suspension adds an unowned possibility; it does not
/// erase the synchronous owner or prove an unowned execution. In particular,
/// a cleanup in an owned leaf may instead resume after the leaf has exited.
/// An unreachable function (context zero) gains no invocation from syntax.
pub(super) fn possible_await_continuation_context(
    file: &solid_facts::FileFacts,
    span: Span,
    context: u8,
) -> Option<u8> {
    (context != 0 && crate::execution_role::may_follow_own_await(file, span))
        .then_some(owner_edge_context(OwnerEdgeKind::MayResumeUnowned, context))
}

pub(super) fn call_owner_edge(
    file: &solid_facts::FileFacts,
    site: Span,
    lookup: &SemanticLookup<'_>,
) -> OwnerEdgeKind {
    if crate::execution_role::may_follow_own_await(file, site) {
        return OwnerEdgeKind::MayResumeUnowned;
    }
    match action_step_resumption(file, site, lookup) {
        Some(StepResumption::Proven) => OwnerEdgeKind::ResumesUnowned,
        Some(StepResumption::Possible) => OwnerEdgeKind::MayResumeUnowned,
        None => OwnerEdgeKind::Preserve,
    }
}

/// Where a site of an action's stepped generator stands to the generator's
/// `yield`s ([`action_step_resumption`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StepResumption {
    /// Every run of the site follows a `yield` whose operand is a promise:
    /// the site runs from that promise's continuation, with no owner.
    Proven,
    /// Some run of the site may follow a `yield` whose operand may be a
    /// thenable: the site may run with no owner, and nothing proves it.
    Possible,
}

/// How an action's stepped generator reaches the site at `span` after one of
/// its own `yield`s, or `None` where no `yield` can run before it (or the
/// site is not in such a generator's body).
///
/// `action(genFn)` runs the generator step by step: `step` calls
/// `it.next(v)` on the stack of whoever called the action for the first
/// step, and settles the yielded value from the result. A yielded thenable
/// is assimilated -- `r.value.then(v => inTransaction(t, q, () => step(v)))`
/// -- so the next step runs from the promise's continuation on an empty
/// stack, where the dialect's fresh-stack answer says no owner is current
/// (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:1618-1636`); any other
/// yielded value steps again synchronously, `inTransaction(t, q, () =>
/// step(r.value))`, under the owner the previous step had (`:1637`). An
/// async generator's `it.next()` returns a promise, so there every `yield`
/// moves the next step to a continuation, whatever it yields (`:1605`).
/// Probed on rc.17 (dev, from a `createTrackedEffect` body): `getOwner()` is
/// the tracked owner before `yield Promise.resolve()` and after `yield 1`,
/// and `null` after the promise.
///
/// Proven: the site is in the block region a `yield` dominates
/// ([`solid_facts::ast::YieldFact::region`]) and that `yield`'s operand is
/// a call the compiler resolved to the standard library's
/// `PromiseConstructor` (`Promise.resolve(…)`, `new Promise(…)`, `Promise.all`
/// and its siblings), or the generator is async. Possible: a `yield` written
/// before the site (not in the other arm of one `if` or `?:`), or in a loop
/// whose repeated part holds the site, whose operand is not a plain value (a
/// number, a string, `true`, `false`, `null`, `undefined`, or absent); a
/// delegating `yield*` hands out whatever the delegate yields. A site in a
/// nested function is that function's: its call edge carries the answer.
pub(crate) fn action_step_resumption(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<StepResumption> {
    let function = containing_body_function(&file.ast, span)?;
    stepped_action_generator(file, function, lookup)?;
    if lookup.dialect.fresh_stack_callback_owner() != Some(solid_dialect::CallbackOwner::None) {
        return None;
    }
    let own = |point: Span| {
        function.body.contains(point)
            && containing_ast_function(&file.ast, point)
                .is_some_and(|owner| owner.span == function.span)
    };
    let mut possible = false;
    for yielded in file.ast.yields.iter().filter(|fact| own(fact.span)) {
        let suspends = function.r#async || yield_operand_promise(file, yielded, lookup);
        if !suspends && !yielded.delegate && yield_operand_plain(file, yielded) {
            continue;
        }
        if suspends && yielded.region.is_some_and(|region| region.contains(span)) {
            return Some(StepResumption::Proven);
        }
        let before = yielded.span.end <= span.start
            && !crate::execution_role::in_exclusive_arms(file, yielded.span, span);
        let repeated = file.ast.loop_repeats.iter().any(|&(looped, repeated)| {
            repeated.contains(span)
                && looped.contains(yielded.span)
                && function.body.contains(looped)
        });
        possible |= before || repeated;
    }
    possible.then_some(StepResumption::Possible)
}

/// Whether the `yield`'s operand is a call the compiler resolved to the
/// standard library's `PromiseConstructor` (a call of `resolve`, `reject`,
/// `all`, `allSettled`, `race` or `any`, or `new Promise(…)`): a native
/// promise, which the action step assimilates. A same-named project
/// declaration, a shadowed `Promise`, or an unresolved call proves nothing.
fn yield_operand_promise(
    file: &solid_facts::FileFacts,
    yielded: &solid_facts::ast::YieldFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(operand) = yielded
        .operand
        .map(|operand| file.ast.peel_ts_sugar_span(operand))
    else {
        return false;
    };
    let Some(call) = file.ast.calls.iter().find(|call| call.span == operand) else {
        return false;
    };
    let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
        return false;
    };
    if resolved.validity != typefacts::ResolvedCallValidity::Valid {
        return false;
    }
    let Some(declaration) = resolved.declaration.as_ref() else {
        return false;
    };
    declaration.standard_library
        && matches!(
            (
                declaration.qualified_name.as_ref(),
                resolved.kind,
                call.construct,
            ),
            (
                "PromiseConstructor.resolve"
                    | "PromiseConstructor.reject"
                    | "PromiseConstructor.all"
                    | "PromiseConstructor.allSettled"
                    | "PromiseConstructor.race"
                    | "PromiseConstructor.any",
                typefacts::CallKind::Call,
                false,
            ) | (
                "PromiseConstructor.construct",
                typefacts::CallKind::Construct,
                true
            )
        )
}

/// Whether the `yield`'s operand is a value that is not a thenable, which the
/// action step passes back synchronously: absent (`undefined`), a numeric or
/// string literal, `true`, `false`, `null`, or the global `undefined`
/// resolved by the absence of a declaration (a binding of that name is a
/// value). Anything else may be a thenable.
fn yield_operand_plain(
    file: &solid_facts::FileFacts,
    yielded: &solid_facts::ast::YieldFact,
) -> bool {
    let Some(operand) = yielded
        .operand
        .map(|operand| file.ast.peel_ts_sugar_span(operand))
    else {
        return true;
    };
    let Some(text) = file.source_text(operand).map(str::trim) else {
        return false;
    };
    matches!(text, "true" | "false" | "null")
        || (text == "undefined" && file.ast.reference_declaration(operand).is_none())
        || text.parse::<f64>().is_ok()
        || (text.len() >= 2
            && (text.starts_with('"') && text.ends_with('"')
                || text.starts_with('\'') && text.ends_with('\'')))
}

pub(super) fn callback_call_owner_edge(
    lookup: &SemanticLookup<'_>,
    source_path: &str,
    site: Span,
    kind: OwnerEdgeKind,
) -> OwnerEdgeKind {
    if kind == OwnerEdgeKind::Preserve {
        lookup
            .file_by_path(source_path)
            .map_or(kind, |file| call_owner_edge(file, site, lookup))
    } else {
        kind
    }
}

/// Whether the operation at `span` runs only after the same invocation of its
/// function has seen a non-null owner: it is the right operand of
/// `getOwner() && …`, or sits in the consequent of `getOwner() ? … : …` or of
/// `if (getOwner()) …`, where the test is a call of Solid's own `getOwner`
/// resolved by symbol (a same-named local function proves nothing).
///
/// Such an operation never executes without an owner, so "no scope's disposal
/// can trigger it" is false on every path that reaches it: probed on the
/// audited 2.0.0-rc.9, dev and prod, an exported helper written this way and
/// called at module scope skips `onCleanup` (no `NO_OWNER_CLEANUP`), while the
/// unguarded helper beside it warns. The guard and the operation must share
/// one function body with no `await` between them -- an owner seen before an
/// `await` is not current after it -- and nothing else is read as a guard:
/// `owner && …` over a binding, `!getOwner() || …`, and early returns keep
/// their requirement.
pub(crate) fn guarded_by_owner_probe(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let owner_probe = |test: Span| {
        let test = file.ast.peel_ts_sugar_span(test);
        file.ast.calls.iter().any(|call| {
            call.span == test
                && call.arguments.is_empty()
                && lookup.primitive_at_call(file, call.span) == Some(Primitive::GetOwner)
        })
    };
    // ADR 0454: the function that owns the operation, so one in a closure's
    // parameter list is not under the owner of the code the closure is written
    // in (it runs when the closure is called without the argument).
    let function = crate::owners::owning_function(&file.ast, span).map(|function| function.span);
    let same_invocation = |test: Span, region: Span| {
        let guard_function =
            crate::owners::owning_function(&file.ast, test).map(|function| function.span);
        // The operation's function is the guard's, or is reached from it through
        // synchronous invoked literals, each invoked inside the guarded region
        // (ADR 0347 amendment): an IIFE in the guarded branch runs under the same owner.
        let mut current = function;
        while current != guard_function {
            let Some(literal) = current else {
                return false;
            };
            let Some(invocation) = file.ast.calls.iter().find(|call| {
                region.contains(call.span)
                    && invoked_function_literal(file, call).is_some_and(|f| f.span == literal)
            }) else {
                return false;
            };
            current = crate::owners::owning_function(&file.ast, invocation.span).map(|f| f.span);
        }
        !file
            .ast
            .awaits
            .iter()
            .any(|awaited| region.contains(*awaited) && awaited.start < span.start)
    };
    file.ast.logical_expressions.iter().any(|logical| {
        logical.operator == solid_facts::ast::LogicalOperatorKind::And
            && logical.right.contains(span)
            && owner_probe(logical.left)
            && same_invocation(logical.left, logical.right)
    }) || file.ast.conditional_expressions.iter().any(|conditional| {
        conditional.consequent.contains(span)
            && owner_probe(conditional.test)
            && same_invocation(conditional.test, conditional.consequent)
    }) || file.ast.if_regions.iter().any(|region| {
        region.consequent.contains(span)
            && owner_probe(region.test)
            && same_invocation(region.test, region.consequent)
    })
}

/// The one place an owner requirement becomes a finding seed — both owner
/// passes, every operation, every uncertainty source — which is why the
/// discarded-region guard is applied here rather than at each push site.
///
/// Candidates come from `discover_owner_file` and the emission in
/// `find_missing_owners_incremental`: the one owner analysis (a build with no
/// retained cache runs it over a scratch fragment table).
pub(crate) fn push_owner_requirement(
    requirements: &mut Vec<OwnerRequirement>,
    seen: &mut HashSet<(String, u64, u64, String)>,
    operation: &str,
    file: &solid_facts::FileFacts,
    span: Span,
    status: OwnerRequirementStatus,
    lookup: &SemanticLookup<'_>,
) {
    // The compiler deleted this operation. There is no owner question to
    // answer and no obligation to record: "this cleanup will never run" is a
    // claim about a call that never happens, and `uncertain` would be wrong too
    // — nothing is unproven here, the two compilers agree the code is gone.
    // Applied at the funnel for the same reason the divergence escalation below
    // is: a new candidate kind cannot forget it.
    if crate::execution_role::discarded_region_contains(file, span) {
        return;
    }
    // The operation runs only once the same invocation has seen an owner, so
    // there is no unowned execution to report and nothing left unproven.
    if guarded_by_owner_probe(file, span, lookup) {
        return;
    }
    let location = location(file.path.as_str(), span);
    if !seen.insert((
        location.path.to_string(),
        location.start_byte,
        location.end_byte,
        operation.into(),
    )) {
        // ADR 0252: a second requirement of the same operation at the same
        // call (a guarded registration beside an unguarded optional one, say)
        // merges into the first. One certain registration proves the call
        // registers; the context is the call's either way, so only the
        // registration's own certainty folds.
        let operation = crate::OwnerRequirementOperation::from_internal(operation);
        let certain = !status.uncertain;
        if certain
            && let Some(existing) = requirements
                .iter_mut()
                .find(|existing| existing.location == location && existing.operation == operation)
        {
            existing.uncertain = existing.missing_jsx_census;
        }
        return;
    }
    {
        let missing_jsx_census = crate::execution_role::missing_jsx_census_region(file, span);
        // An instance field initializer runs at construction: the owner (or
        // leaf scope) of the function the class is written in says nothing
        // about it (review round 2).
        let constructed = crate::execution_role::inside_instance_field(file, span);
        requirements.push(OwnerRequirement {
            operation: crate::OwnerRequirementOperation::from_internal(operation),
            location,
            uncertain: status.uncertain || missing_jsx_census || constructed,
            runtime_uncertain: status.runtime_uncertain,
            caller_uncertain: status.caller_uncertain,
            conditional_owner: status.conditional_owner,
            owner_varies_by_caller: status.owner_varies_by_caller,
            later_run_unowned: status.later_run_unowned,
            component_uncertain: status.component_uncertain,
            unplaced_callback: status.unplaced_callback,
            provider_open: status.provider_open,
            default_unproven: status.default_unproven,
            owned_run_throws: status.owned_run_throws,
            after_await: status.after_await,
            after_yield: status.after_yield,
            missing_jsx_census,
            through_dependency: None,
            report: status.report,
        });
    }
}
/// Which callback arguments of a call get an owner edge, and of what kind.
///
/// Shared by both owner passes on purpose: each used to keep its own literal
/// list, and lists kept separately drift.
pub(crate) fn owner_callback_edges(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: &Option<PrimitiveName>,
    lookup: &SemanticLookup<'_>,
) -> Vec<OwnerCallbackEdge> {
    let Some(primitive) = known_primitive(primitive) else {
        let scheduled = fresh_stack_scheduler_edges(file, call, lookup);
        if !scheduled.is_empty() {
            return scheduled;
        }
        // ADR 0438: a suite body the runner runs inside the registration call
        // runs under the owner of the call's position.
        let inline_suite = inline_suite_body_edges(file, call, lookup);
        if !inline_suite.is_empty() {
            return inline_suite;
        }
        // A call that names no primitive can still be an invocation of a
        // function some primitive returned -- but only where the dialect models
        // such a function. Solid 2.0 models none, so the binding-chain walk
        // below would resolve a primitive and then be told `None` every time.
        if !lookup.models_returned_callbacks() {
            return Vec::new();
        }
        let Some((returned, result_slot)) = returned_primitive_invocation(file, call, lookup)
        else {
            return Vec::new();
        };
        return (0..call.arguments.len())
            .filter_map(|argument| {
                lookup
                    .dialect
                    .returned_callback_semantics_at(
                        returned,
                        result_slot,
                        argument,
                        call.arguments.len(),
                    )
                    .owner
                    .map(|owner| OwnerCallbackEdge {
                        argument,
                        kind: callback_owner_edge_kind_at(lookup, returned, owner),
                        source_path: file.path.to_string(),
                        source: call.span,
                    })
            })
            .collect();
    };
    let mut edges = Vec::new();
    for argument in 0..call.arguments.len() {
        let Some(owner) = callback_owner_at_call(file, call, primitive, argument, lookup) else {
            continue;
        };
        let kind = drained_callback_owner_edge_kind(lookup, primitive, argument, owner);
        if lookup
            .dialect
            .callback_semantics_at(primitive, argument, call.arguments.len())
            .requires_return_invocation
        {
            edges.extend(
                returned_callback_invocation_sites(file, call, lookup)
                    .into_iter()
                    .map(|site| OwnerCallbackEdge {
                        argument,
                        kind: compose_owner_edge(site.inherited_owner, kind),
                        source_path: site.path,
                        source: site.span,
                    }),
            );
        } else {
            edges.push(OwnerCallbackEdge {
                argument,
                kind,
                source_path: file.path.to_string(),
                source: call.span,
            });
        }
    }
    edges
}

/// What the value of an intrinsic `ref` does to the owner graph, per function
/// literal the value writes at its top level (ADR 0345).
///
/// The expression is evaluated while the component renders, and the ref
/// application invokes the values it yields. A yielded function runs with no
/// owner at that application ([`Applied`](Self::Applied)). A function a call in
/// the expression calls directly runs while the expression evaluates, under the
/// owner of the code around it ([`Eager`](Self::Eager)). Any other top-level
/// function there may be the applied callback, or may run with no owner, and no
/// proof says which ([`Unproven`](Self::Unproven)).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RefValueMark {
    /// The function is a value the ref application receives
    /// ([`yielded_value_contains`]).
    Applied { function: Span },
    /// The function is a call the expression makes while the code around it
    /// runs at module initialization ([`runs_at_module_init`]): no owner is
    /// current there, so the call runs with none.
    Unowned { function: Span },
    /// The function is the callee of `call`, which the expression makes.
    Eager { function: Span, call: Span },
    /// The function's owner is unproven: a generator (a call runs none of its
    /// body), a callback handed to a callee no owner edge describes, or a
    /// function the expression evaluates without yielding it (a getter's body,
    /// a tagged template's substitution, an operand of `void`).
    Unproven { function: Span },
}

/// The [`RefValueMark`]s of the function literals inside each DirectiveApply
/// role of an intrinsic `ref`, shared by both owner passes.
///
/// Only a function no other function in the same role contains is marked. A
/// function nested in one runs when its own callers run it, and the owner
/// graph already places it through those calls. A marked generator is
/// [`Unproven`](RefValueMark::Unproven): calling one runs none of its body, and
/// the ref application never iterates it.
pub(crate) fn ref_value_marks(
    file: &solid_facts::FileFacts,
    call_primitives: &[Option<PrimitiveName>],
    lookup: &SemanticLookup<'_>,
) -> Vec<RefValueMark> {
    let mut marks = Vec::new();
    for callback in &file.compiler.callback_roles {
        if callback.role != solid_facts::compiler::CallbackRoleKind::DirectiveApply
            || !ref_application_on_intrinsic(file, callback.span)
        {
            continue;
        }
        let calls = file
            .ast
            .calls
            .iter()
            .enumerate()
            .filter(|(_, call)| callback.span.contains(call.span))
            .collect::<Vec<_>>();
        let functions = file.ast.functions_within(callback.span).collect::<Vec<_>>();
        for function in functions.iter().filter(|function| {
            !functions
                .iter()
                .any(|outer| outer.span != function.span && outer.span.contains(function.span))
        }) {
            if function.generator {
                marks.push(RefValueMark::Unproven {
                    function: function.span,
                });
                continue;
            }
            if let Some((_, call)) = calls
                .iter()
                .find(|(_, call)| file.ast.peel_ts_sugar_span(call.callee) == function.span)
            {
                // The call runs while the expression evaluates, under the owner
                // of the code around it (ADR 0345). A component's children do
                // not run there: they run inside the component's own memo, so
                // the call is unproven. Code that runs at module initialization
                // has no owner, so the call is unowned. Elsewhere the owner walk
                // places it by its enclosing function, and settles a call whose
                // enclosing function has no owner context
                // ([`OwnerEdgeKind::EagerCall`]).
                marks.push(if inside_component_element(file, callback.span) {
                    RefValueMark::Unproven {
                        function: function.span,
                    }
                } else if runs_at_module_init(file, call.span) {
                    RefValueMark::Unowned {
                        function: function.span,
                    }
                } else {
                    RefValueMark::Eager {
                        function: function.span,
                        call: call.span,
                    }
                });
                continue;
            }
            if yielded_value_contains(file, callback.span, function.span) {
                marks.push(RefValueMark::Applied {
                    function: function.span,
                });
                continue;
            }
            // A callback whose callee the dialect describes (a synchronous
            // slot, an effect's apply, a fresh-stack scheduler) is placed by
            // the owner edges its callers already add. Any other function is
            // evaluated by something the expression does (a getter's body, a
            // tagged template's substitution, an operand of `void`): unproven.
            let described = calls.iter().any(|(index, call)| {
                call.span.contains(function.span)
                    && call.arguments.iter().enumerate().any(|(argument, value)| {
                        file.ast.peel_ts_sugar_span(value.span) == function.span
                            && owner_callback_edges(file, call, &call_primitives[*index], lookup)
                                .iter()
                                .any(|edge| edge.argument == argument)
                    })
            });
            if !described {
                marks.push(RefValueMark::Unproven {
                    function: function.span,
                });
            }
        }
        // A two-phase directive (`ref={dir()}`): the call runs while the
        // expression evaluates, and the ref application receives what it
        // returns. When the callee is exactly a function of this file, each
        // function literal one of its own completions returns is applied, with
        // no owner, like a literal written in the attribute. Anything else it
        // returns stays where the owner walk places it.
        for (_, call) in &calls {
            if !yielded_value_contains(file, callback.span, call.span) {
                continue;
            }
            let Some((factory_file, factory)) =
                lookup.function_called_at(file.path.as_str(), call.callee)
            else {
                continue;
            };
            if factory_file.path != file.path {
                continue;
            }
            for returned in returned_functions(file, factory) {
                marks.push(
                    if file.ast.function_at(returned).is_some_and(|f| f.generator) {
                        RefValueMark::Unproven { function: returned }
                    } else {
                        RefValueMark::Applied { function: returned }
                    },
                );
            }
        }
    }
    marks
}

/// The function literals `factory`'s own completions return: an arrow's
/// expression body, or the argument of a `return` no nested function holds.
pub(crate) fn returned_functions(
    file: &solid_facts::FileFacts,
    factory: &solid_facts::ast::FunctionFact,
) -> Vec<Span> {
    let returns = factory.expression_return.as_ref().map_or_else(
        || {
            file.ast
                .returns
                .iter()
                .filter(|returned| {
                    containing_ast_function(&file.ast, returned.span)
                        .is_some_and(|owner| owner.span == factory.span)
                })
                .collect::<Vec<_>>()
        },
        |returned| vec![returned],
    );
    returns
        .into_iter()
        .filter_map(|returned| {
            returned
                .argument
                .map(|span| file.ast.peel_ts_sugar_span(span))
        })
        .filter(|argument| file.ast.function_at(*argument).is_some())
        .collect()
}

/// The values a ref expression yields: the expression's own value
/// (parentheses and type wrappers are transparent), or an element, a branch
/// or an operand of a value it yields, down to the leaves. A ref application
/// receives exactly those values, so only they run in its apply phase. A
/// function anywhere else is evaluated by something the expression does: a
/// getter's body when `.x` reads it, a tagged template's substitution, an
/// operand of `void` (never called at all).
pub(crate) fn yielded_values(file: &solid_facts::FileFacts, value: Span) -> Vec<Span> {
    let value = file.ast.peel_ts_sugar_span(value);
    if let Some(conditional) = file
        .ast
        .conditional_expressions
        .iter()
        .find(|fact| file.ast.peel_ts_sugar_span(fact.span) == value)
    {
        return [conditional.consequent, conditional.alternate]
            .into_iter()
            .flat_map(|branch| yielded_values(file, branch))
            .collect();
    }
    if let Some(logical) = file
        .ast
        .logical_expressions
        .iter()
        .find(|fact| file.ast.peel_ts_sugar_span(fact.span) == value)
    {
        // `a && b` yields `a` only when `a` is falsy, and a function value is
        // always truthy: a function written as the left operand is tested and
        // never applied. `a || b` and `a ?? b` can yield either operand.
        let operands: &[Span] = if logical.operator == solid_facts::ast::LogicalOperatorKind::And {
            &[logical.right]
        } else {
            &[logical.left, logical.right]
        };
        return operands
            .iter()
            .flat_map(|operand| yielded_values(file, *operand))
            .collect();
    }
    // `(a, b)` yields `b`: `a` is evaluated for its effect and discarded.
    if let Some(sequence) = file
        .ast
        .sequences
        .iter()
        .find(|fact| file.ast.peel_ts_sugar_span(fact.span) == value)
    {
        return yielded_values(file, sequence.tail);
    }
    if let Some(array) = file
        .ast
        .array_literal_elements
        .iter()
        .find(|fact| file.ast.peel_ts_sugar_span(fact.span) == value)
    {
        return array
            .elements
            .iter()
            .filter(|element| element.kind == solid_facts::ast::ArrayElementKind::Value)
            .flat_map(|element| yielded_values(file, element.span))
            .collect();
    }
    vec![value]
}

/// Whether `function` is one of the values a ref expression yields
/// ([`yielded_values`]).
pub(crate) fn yielded_value_contains(
    file: &solid_facts::FileFacts,
    value: Span,
    function: Span,
) -> bool {
    yielded_values(file, value).contains(&function)
}

/// The intrinsic element whose `ref` application receives `function`: rc.17's
/// `ref` (`@solidjs/web` `dist/web.dev.js:1016-1022`) resolves the attribute
/// expression under `untrack` and then applies every function it yields to
/// the element, synchronously, under `runWithOwner(null)`, while the element
/// is created. The function is received when it is a value the expression
/// yields ([`yielded_values`]): the literal itself; the function a yielded
/// bare identifier holds, resolved exactly and never reassigned
/// (`ref={apply}`, compiled `ref(() => apply, el)`); or a literal that the
/// exact, same-file callee of a yielded call returns (`ref={dir()}`: the call
/// runs eagerly where the element is created and its result is applied,
/// [`returned_functions`]). A generator yields nothing when applied, and an
/// async factory returns a promise, which the application does not call.
/// Anything else (a member call, an imported binding, a value a getter
/// computes) is unplaced here.
pub(crate) fn ref_applied_element<'f>(
    file: &'f solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<&'f solid_facts::ast::JsxElementFact> {
    if function.generator {
        return None;
    }
    let path = file.path.as_str();
    let same_function = |target_file: &solid_facts::FileFacts,
                         target: &solid_facts::ast::FunctionFact| {
        target_file.path == file.path && target.span == function.span
    };
    let applied = file
        .compiler
        .callback_roles
        .iter()
        .filter(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::DirectiveApply
                && ref_application_on_intrinsic(file, callback.span)
        })
        .find(|callback| {
            yielded_values(file, callback.span).into_iter().any(|leaf| {
                if leaf == function.span {
                    return true;
                }
                if file.ast.identifiers.iter().any(|identifier| {
                    identifier.span == leaf
                        && identifier.role == solid_facts::ast::IdentifierRole::Reference
                }) {
                    return lookup.function_called_at(path, leaf).is_some_and(
                        |(target_file, target)| {
                            same_function(target_file, target)
                                && lookup.function_value_is_current(file, target)
                        },
                    );
                }
                if let Some(call) = file
                    .ast
                    .calls
                    .iter()
                    .find(|call| call.span == leaf && call.direct_callee && !call.construct)
                {
                    return lookup.function_called_at(path, call.callee).is_some_and(
                        |(factory_file, factory)| {
                            factory_file.path == file.path
                                && !factory.r#async
                                && !factory.generator
                                && lookup.function_value_is_current(file, factory)
                                && returned_functions(file, factory).contains(&function.span)
                        },
                    );
                }
                false
            })
        })?;
    file.ast
        .jsx_elements
        .iter()
        .filter(|element| element.span.contains(applied.span))
        .min_by_key(|element| element.span.end - element.span.start)
}

/// Whether a JSX expression the compiler lowers evaluates `span`: an element
/// contains it with no function written in that element between them (an
/// attribute's or child's expression becomes a getter, an effect or a static
/// value, which the compiler roles place), and it is not a value a ref
/// application yields, which runs where the element is created (a two-phase
/// directive's factory call `ref={dir()}` is evaluated eagerly there,
/// [`ref_applied_element`]). Code in a function written in the JSX (a ref
/// callback, a handler) is that function's.
pub(crate) fn jsx_evaluates(file: &solid_facts::FileFacts, span: Span) -> bool {
    let owner = owning_function(&file.ast, span);
    file.ast
        .jsx_containing(span)
        .any(|element| !owner.is_some_and(|function| element.span.contains(function.span)))
        && !file.compiler.callback_roles.iter().any(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::DirectiveApply
                && ref_application_on_intrinsic(file, callback.span)
                && yielded_values(file, callback.span).contains(&span)
        })
}

/// The innermost function whose whole span, parameters included, contains
/// `span`. A call in a default parameter initializer runs in that function's
/// call, under the owner of the code that makes it, so its owner is the
/// function that owns the parameter list, not the enclosing body alone.
pub(crate) fn enclosing_function_span(
    nodes: &[OwnerNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    path: &str,
    span: Span,
) -> Option<usize> {
    nodes_by_path
        .get(path)?
        .iter()
        .copied()
        .filter(|index| nodes[*index].span.contains(span))
        .min_by_key(|index| {
            let function = nodes[*index].span;
            function.end - function.start
        })
}

/// Whether the code around `span` runs at module initialization, where no owner
/// is current. The innermost function containing `span` must be a function
/// literal the file calls directly, and that call must be one module
/// initialization makes: in no function body, outside an instance field (which
/// runs at construction, under the constructor's owner), and outside JSX (a
/// component's children and a compiled child expression run in an owner of
/// their own).
fn runs_at_module_init(file: &solid_facts::FileFacts, span: Span) -> bool {
    let Some(enclosing) = file
        .ast
        .functions
        .iter()
        .filter(|function| function.span.contains(span))
        .min_by_key(|function| function.span.end - function.span.start)
    else {
        return false;
    };
    file.ast.calls.iter().any(|call| {
        file.ast.peel_ts_sugar_span(call.callee) == enclosing.span
            && !file.ast.any_function_body_containing(call.span)
            && !crate::execution_role::inside_instance_field(file, call.span)
            && file.ast.jsx_containing(call.span).next().is_none()
    })
}

/// The function literal an invocation runs synchronously (ADR 0347 amendment).
/// The callee, with parentheses and type assertions peeled, is a function
/// literal, or the last operand of a comma expression (`(0, () => …)()`), or the
/// object of a `.call` / `.apply` on one. A `new` runs only a plain function
/// expression: an arrow, an async function or a method throws before its body
/// runs. A generator is never run by the call, since calling one runs none of
/// its body. An `async` literal qualifies: code before its first `await` runs in
/// the call, and code after it is a continuation, which
/// [`await_continuation_context`] places.
pub(crate) fn invoked_function_literal<'f>(
    file: &'f solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Option<&'f solid_facts::ast::FunctionFact> {
    let mut callee = file.ast.peel_ts_sugar_span(call.callee);
    let mut through_method = false;
    loop {
        if let Some(sequence) = file
            .ast
            .sequences
            .iter()
            .find(|sequence| sequence.span == callee)
        {
            callee = file.ast.peel_ts_sugar_span(sequence.tail);
            continue;
        }
        if !through_method
            && let Some(member) = file.ast.members.iter().find(|member| member.span == callee)
            && file
                .ast
                .computed_members
                .binary_search(&member.span)
                .is_err()
            && matches!(
                file.source
                    .get(member.property.start as usize..member.property.end as usize),
                Some("call" | "apply")
            )
        {
            callee = file.ast.peel_ts_sugar_span(member.object);
            through_method = true;
            continue;
        }
        break;
    }
    let function = file.ast.function_at(callee)?;
    if function.generator
        || (call.construct
            && (through_method
                || function.r#async
                || function.kind != solid_facts::ast::FunctionKind::Expression))
    {
        return None;
    }
    Some(function)
}

/// Whether an invocation at `call` runs under the owner of the code around it
/// (ADR 0347). `owner` is the span of the innermost function holding the call,
/// or `None` at module scope. Two placements run under another owner, so the
/// walk makes no claim there:
///
/// - an instance field initializer, which runs at construction, as
///   `runs_at_module_init` refuses it. A static field or block runs at class
///   evaluation, under the code around the class, so it keeps the edge (ADR
///   0347 amendment). A function inside the initializer is its own owner, and is not
///   refused;
/// - JSX written in the owner's own body. A component runs its children inside
///   its own memo, and an attribute is evaluated as the compiler places it
///   (ADR 0345). JSX that only contains the owner, such as an event handler
///   written in an attribute, is not such a placement.
pub(crate) fn invocation_runs_under_owner(
    file: &solid_facts::FileFacts,
    call: Span,
    owner: Option<Span>,
) -> bool {
    let in_field_initializer = file.ast.classes.iter().any(|class| {
        class.elements.iter().any(|element| {
            let initializer = match element.kind {
                solid_facts::ast::ClassElementKind::Field if !element.r#static => element.value,
                _ => None,
            };
            initializer.is_some_and(|initializer| {
                initializer.contains(call) && owner.is_none_or(|owner| !initializer.contains(owner))
            })
        })
    });
    let in_owner_jsx = file
        .ast
        .jsx_containing(call)
        .any(|element| owner.is_none_or(|owner| owner.contains(element.span)));
    !in_field_initializer && !in_owner_jsx
}

/// Whether `span` is one identifier. A ref application that invokes a named
/// function (`ref={apply}`) resolves its symbol exactly only then: a longer
/// span's start names some other symbol.
pub(crate) fn is_identifier_span(file: &solid_facts::FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    let text = file
        .source
        .get(span.start as usize..span.end as usize)
        .unwrap_or_default();
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// ADR 0468: whether the entity at a call's callee, which names the symbol at
/// its first token, can name the value the call runs. It can for an
/// identifier, a member (whose leading token is its receiver: callers tell
/// the two apart) and a call (`make()()`: the inner call's callee runs too).
/// For any other callee the leading token is some operand: `(f && h)()` and
/// `(x = h)()` call `h`, not `f` or `x`.
pub(crate) fn callee_entity_names_value(file: &solid_facts::FileFacts, callee: Span) -> bool {
    let callee = file.ast.peel_ts_sugar_span(callee);
    is_identifier_span(file, callee)
        || file.ast.member_at(callee).is_some()
        || file.ast.call_at(callee).is_some()
}

/// The owner edges of callbacks handed to a reviewed fresh-stack host
/// scheduler (`runtime_semantics::FRESH_STACK_SCHEDULERS`: `setTimeout`,
/// `queueMicrotask`, `Promise.then`, the observer constructors, ...).
///
/// Two separately reviewed facts compose here. The host fact -- the scheduler
/// invokes the argument only from a task or microtask queue, on an otherwise
/// empty stack -- comes from the compiler-selected standard-library
/// declaration, never from spelling. The owner fact -- what that empty stack
/// means for the owner -- is the dialect's
/// ([`Dialect::fresh_stack_callback_owner`]); a dialect that does not answer
/// gives no edge, and the callback keeps its lexical answer.
///
/// Only an argument that *is* the callback gets the edge: a function literal
/// or an identifier naming one. `setTimeout(wrap(() => ...))` hands the host
/// whatever `wrap` returns, and the arrow inside may run on `wrap`'s own
/// stack, under the scheduling call's owner.
///
/// Reached through [`owner_callback_edges`].
pub(crate) fn fresh_stack_scheduler_edges(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<OwnerCallbackEdge> {
    let Some(owner) = lookup.dialect.fresh_stack_callback_owner() else {
        return Vec::new();
    };
    let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
        return Vec::new();
    };
    call.arguments
        .iter()
        .enumerate()
        .filter(|(_, argument)| {
            matches!(
                argument.value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
                    | solid_facts::ast::ArgumentValueKind::Identifier
            )
            // ADR 0516: an object literal's member that names one function.
            || (!argument.spread
                && lookup
                    .object_member_function(file, file.ast.peel_ts_sugar_span(argument.span))
                    .is_some())
        })
        .filter(|(index, argument)| {
            let exact_member = lookup
                .object_member_function(file, file.ast.peel_ts_sugar_span(argument.span))
                .is_some();
            let callability = lookup
                .entity_at(file.path.as_str(), argument.span)
                .and_then(|entity| entity.callability)
                .or_else(|| exact_member.then_some(typefacts::Callability::Callable));
            crate::runtime_semantics::argument_behavior(resolved, callability, *index).map_or_else(
                // ADR 0438: a reviewed test runner invokes a registered callback
                // from its own scheduler.
                || {
                    lookup.test_runner_callback(file, call)
                        == Some(crate::indexes::TestRunnerCallback::Scheduled)
                },
                crate::runtime_semantics::RuntimeArgumentBehavior::runs_on_fresh_stack,
            )
        })
        .map(|(argument, value)| OwnerCallbackEdge {
            argument,
            // ADR 0516: a generator the scheduler calls runs none of its
            // body there, so nothing is proven about its owner.
            kind: if crate::owners::argument_may_be_generator(file, value.span, lookup) {
                OwnerEdgeKind::MayBeUnowned
            } else {
                callback_owner_edge_kind(owner)
            },
            source_path: file.path.to_string(),
            source: call.span,
        })
        .collect()
}

/// ADR 0561: the arguments of `call` a reviewed standard-library carrier runs
/// before it returns, on the caller's stack: `runtime_semantics`'s audited
/// [`InlineCallback`](crate::runtime_semantics::RuntimeArgumentBehavior)
/// rows (`forEach`, `map`, `sort`, `Array.from`'s map function,
/// `Reflect.apply`'s target, ...), the table ADR 0531's `HandOff::Inline`
/// reads. The carrier is the compiler-selected standard-library declaration,
/// never a spelling: a local or project `forEach` resolves elsewhere and is
/// not one. A spread argument is not the callback, and a primitive's call is
/// its callback table's.
///
/// A synchronous call keeps the current owner (`getOwner()` reads a module
/// global no standard-library call swaps), the fact a direct call's
/// [`OwnerEdgeKind::Preserve`] edge already rests on, so no dialect question
/// is asked: the carrier's run is the direct call's.
pub(crate) fn inline_carrier_arguments(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<usize> {
    inline_carrier_arguments_by(
        file,
        call,
        lookup,
        lookup.resolved_callee_call(file, call.callee),
    )
}

/// ADR 0874: [`inline_carrier_arguments`] for a call the impostor census
/// withholds, read through its placing resolution: the arguments the standard
/// carrier would run in place. An impostor may run them there, later, or
/// never, so this proves no run and no owner; it only keeps the callback
/// from losing every owner context.
pub(crate) fn withheld_carrier_arguments(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<usize> {
    if !lookup.impostor_withheld(file, call.callee) {
        return Vec::new();
    }
    inline_carrier_arguments_by(
        file,
        call,
        lookup,
        lookup.placing_callee_call(file, call.callee),
    )
}

fn inline_carrier_arguments_by(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
    resolved: Option<&typefacts::ResolvedCall>,
) -> Vec<usize> {
    if lookup.primitive_at_call(file, call.span).is_some() {
        return Vec::new();
    }
    let Some(resolved) = resolved else {
        return Vec::new();
    };
    call.arguments
        .iter()
        .enumerate()
        .filter(|(_, argument)| !argument.spread)
        .filter(|(index, argument)| {
            let callability = lookup
                .entity_at(file.path.as_str(), argument.span)
                .and_then(|entity| entity.callability);
            crate::runtime_semantics::is_inline_standard_callback(resolved, callability, *index)
        })
        .map(|(index, _)| index)
        .collect()
}

/// The edges of a reviewed runner's suite registration whose body the runner
/// runs inside the call ([`crate::indexes::TestRunnerCallback::InlineSuiteBody`]):
/// the body is a synchronous function literal, and it inherits the owner at
/// the call. An async body is no edge: its resumptions run elsewhere.
fn inline_suite_body_edges(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<OwnerCallbackEdge> {
    if lookup.test_runner_callback(file, call)
        != Some(crate::indexes::TestRunnerCallback::InlineSuiteBody)
    {
        return Vec::new();
    }
    call.arguments
        .iter()
        .enumerate()
        .filter(|(_, argument)| {
            matches!(
                argument.value,
                solid_facts::ast::ArgumentValueKind::Function
            )
        })
        .map(|(argument, _)| OwnerCallbackEdge {
            argument,
            kind: OwnerEdgeKind::Preserve,
            source_path: file.path.to_string(),
            source: call.span,
        })
        .collect()
}

pub(crate) const fn callback_owner_edge_kind(owner: solid_dialect::CallbackOwner) -> OwnerEdgeKind {
    match owner {
        solid_dialect::CallbackOwner::Creates => OwnerEdgeKind::Owned,
        solid_dialect::CallbackOwner::Conditional => OwnerEdgeKind::Conditional,
        solid_dialect::CallbackOwner::Inherits => OwnerEdgeKind::Preserve,
        solid_dialect::CallbackOwner::None => OwnerEdgeKind::Unowned,
        solid_dialect::CallbackOwner::InheritsFirstRun => OwnerEdgeKind::InheritsFirstRun,
        solid_dialect::CallbackOwner::Leaf => OwnerEdgeKind::Leaf,
    }
}

/// [`callback_owner_edge_kind`] for a callback handed to `primitive`: a leaf
/// owner that the dialect says only an owned call site makes a leaf (`onSettled`)
/// is [`OwnerEdgeKind::QueuedLeaf`].
pub(crate) fn callback_owner_edge_kind_at(
    lookup: &SemanticLookup<'_>,
    primitive: Primitive,
    owner: solid_dialect::CallbackOwner,
) -> OwnerEdgeKind {
    match callback_owner_edge_kind(owner) {
        OwnerEdgeKind::Leaf
            if lookup
                .dialect
                .leaf_owner_requires_owned_call_site(primitive) =>
        {
            OwnerEdgeKind::QueuedLeaf
        }
        kind => kind,
    }
}

/// ADR 0613: [`callback_owner_edge_kind_at`] for the callback at `argument`
/// of `primitive`, where an ownerless answer the queue's drain gives
/// ([`solid_dialect::Dialect::callback_runs_drained_by_flush`]) holds only
/// while no `flush()` of the program may drain under an owner
/// ([`SemanticLookup::flush_drains_under_owner`]); otherwise it is
/// [`OwnerEdgeKind::DrainedByFlush`]. The veto is read only for such a
/// callback.
pub(crate) fn drained_callback_owner_edge_kind(
    lookup: &SemanticLookup<'_>,
    primitive: Primitive,
    argument: usize,
    owner: solid_dialect::CallbackOwner,
) -> OwnerEdgeKind {
    let kind = callback_owner_edge_kind_at(lookup, primitive, owner);
    let drained = matches!(kind, OwnerEdgeKind::Unowned | OwnerEdgeKind::QueuedLeaf)
        && lookup
            .dialect
            .callback_runs_drained_by_flush(primitive, argument)
        && lookup.flush_drains_under_owner();
    match kind {
        OwnerEdgeKind::Unowned if drained => OwnerEdgeKind::DrainedByFlush,
        OwnerEdgeKind::QueuedLeaf if drained => OwnerEdgeKind::QueuedLeafDrained,
        kind => kind,
    }
}

/// Compose the owner supplied by a returned function's invocation with the
/// owner behavior of the factory callback itself. An explicit owner result on
/// the inner callback dominates; an inheriting callback keeps the invocation
/// site's context.
pub(crate) const fn compose_owner_edge(
    invocation: OwnerEdgeKind,
    callback: OwnerEdgeKind,
) -> OwnerEdgeKind {
    match callback {
        OwnerEdgeKind::Preserve => invocation,
        _ => callback,
    }
}

/// The argument whose callback runs under an owner this call creates (or, for
/// `runWithOwner`, supplies). The two owner passes share this answer; each
/// used to keep its own literal list, and the lists disagreed with every
/// primitive outside seven names.
pub(crate) fn owner_providing_argument(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Option<Primitive>,
    lookup: &SemanticLookup<'_>,
) -> Option<usize> {
    let primitive = primitive?;
    (0..call.arguments.len()).find(|index| {
        callback_owner_at_call(file, call, primitive, *index, lookup)
            == Some(solid_dialect::CallbackOwner::Creates)
    })
}

pub(crate) fn containing_leaf_owner(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    lexical_leaf_owner(file, span, entities, symbol_names, lookup)
        .or_else(|| named_leaf_owner(file, span, entities, symbol_names, lookup))
        .or_else(|| action_step_leaf_owner(file, span, entities, symbol_names, lookup))
}

/// The calls among those [`containing_leaf_owner`] reads at `span` whose
/// callback is a leaf only under an owned call site: the argument `span` sits
/// in, and the positions of a callback handed by name.
pub(crate) fn gated_leaf_calls_at(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    let leaf_gated = |call: &solid_facts::ast::CallFact, index: usize| {
        lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| {
                lookup
                    .dialect
                    .leaf_owner_requires_owned_call_site(primitive)
                    && callback_owner_at_call(file, call, primitive, index, lookup)
                        == Some(solid_dialect::CallbackOwner::Leaf)
            })
    };
    let mut calls: Vec<Span> = file
        .ast
        .arguments_containing(span)
        .filter(|(call, index)| leaf_gated(call, *index))
        .map(|(call, _)| call.span)
        .collect();
    let direct = crate::owners::containing_body_function(&file.ast, span);
    let climbed = crate::execution_role::climb_in_place_layers(file, span, lookup)
        .filter(|climb| !climb.exhausted && !climb.in_list && climb.suspended.is_none())
        .and_then(|climb| crate::owners::containing_body_function(&file.ast, climb.at));
    for function in [direct, climbed].into_iter().flatten() {
        let Some((positions, _)) = crate::owners::exact_value_positions(file, function, lookup)
        else {
            continue;
        };
        for (call_span, index) in positions {
            let call = file.ast.calls.iter().find(|call| call.span == call_span);
            if call.is_some_and(|call| leaf_gated(call, index)) {
                calls.push(call_span);
            }
        }
    }
    calls
}

/// ADR 0614: the leaf owner an action's first step runs under: it runs on its
/// caller's stack (ADR 0547, ADR 0550), so the owner of its calls, when every
/// one has it and no `yield` may come first. A step with other callers is not
/// this read's claim.
fn action_step_leaf_owner(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    let step = containing_body_function(&file.ast, span)?;
    stepped_action_generator(file, step, lookup)?;
    if action_step_resumption(file, span, lookup).is_some() {
        return None;
    }
    let calls = crate::execution_role::action_step_call_sites(file, step, lookup)?;
    let mut owners = calls.iter().map(|call| {
        lexical_leaf_owner(file, call.span, entities, symbol_names, lookup)
            .or_else(|| named_leaf_owner(file, call.span, entities, symbol_names, lookup))
    });
    let first = owners.next()??;
    owners.all(|owner| owner.is_some()).then_some(first)
}

/// ADR 0516: the leaf owner a callback handed by name (`onSettled(cb)`, a
/// copy, a slot) runs under: the function around `span`, or the function
/// the in-place climb from `span` lands in, whose every value position is a
/// leaf slot of one owner, with no other use.
fn named_leaf_owner(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    let direct = crate::owners::containing_body_function(&file.ast, span)?;
    let climbed = crate::execution_role::climb_in_place_layers(file, span, lookup)
        .filter(|climb| !climb.exhausted && !climb.in_list && climb.suspended.is_none())
        .and_then(|climb| crate::owners::containing_body_function(&file.ast, climb.at));
    [Some(direct), climbed]
        .into_iter()
        .flatten()
        .find_map(|function| {
            let (positions, other) = crate::owners::exact_value_positions(file, function, lookup)?;
            if other || positions.is_empty() {
                return None;
            }
            let mut owners = positions.iter().map(|(call_span, index)| {
                let call = file.ast.calls.iter().find(|call| call.span == *call_span)?;
                let owner = crate::call_primitive_name_cached(
                    lookup,
                    file,
                    call,
                    entities,
                    symbol_names,
                    lookup.dialect,
                )?;
                let primitive = owner.primitive()?;
                (callback_owner_at_call(file, call, primitive, *index, lookup)
                    == Some(solid_dialect::CallbackOwner::Leaf))
                .then(|| owner.to_string())
            });
            let first = owners.next()??;
            owners
                .all(|owner| owner.as_deref() == Some(first.as_str()))
                .then_some(first)
        })
}

fn lexical_leaf_owner(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            let owner = crate::call_primitive_name_cached(
                lookup,
                file,
                call,
                entities,
                symbol_names,
                lookup.dialect,
            )?;
            owner
                .primitive()
                .is_some_and(|primitive| {
                    callback_owner_at_call(file, call, primitive, index, lookup)
                        == Some(solid_dialect::CallbackOwner::Leaf)
                })
                .then(|| owner.to_string())
        })
}
