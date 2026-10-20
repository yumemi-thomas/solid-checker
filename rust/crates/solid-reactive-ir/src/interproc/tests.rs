use crate::{HashMap, HashSet};
use std::sync::Arc;

use solid_dialect::{Primitive, TrackedCallbackTiming};
use typefacts::Location;

use super::{
    CallbackSchedule, CallbackSummary, CallbackWrapper, ForwardedAmbientExecution, SummaryRead,
    SummaryReads, SymbolId, add_interprocedural_dependency_user, cached_reactive_source,
    compose_callback_chain, composed_tracked_schedule, equivalent_callbacks,
    equivalent_summary_reads, primitive_callback_execution, primitive_slot_roots_parameter_invoke,
    reactive_source_order, remove_interprocedural_dependency_user, retained_reactive_sources,
};
use super::{ParameterMemberInvocation, equivalent_member_invocations};
use crate::cache::InterproceduralResultDependency;
use solid_facts::core::Span;

#[test]
fn source_sites_do_not_change_dispatch_equivalence_but_contexts_do() {
    let first = ParameterMemberInvocation {
        parameter: 0,
        path: vec!["read".into()],
        in_owner_body: false,
        site: Span::new(10, 14),
        primitive_builtin: false,
        dispatch_read_excluded: true,
        unmodeled: false,
    };
    let second = ParameterMemberInvocation {
        site: Span::new(30, 34),
        ..first.clone()
    };
    assert!(equivalent_member_invocations(
        std::slice::from_ref(&first),
        &[
            second.clone(),
            ParameterMemberInvocation {
                site: Span::new(50, 54),
                ..second.clone()
            }
        ]
    ));
    let eager = ParameterMemberInvocation {
        dispatch_read_excluded: false,
        unmodeled: false,
        ..second
    };
    assert!(!equivalent_member_invocations(
        std::slice::from_ref(&first),
        std::slice::from_ref(&eager)
    ));
    assert!(!equivalent_member_invocations(
        std::slice::from_ref(&first),
        &[first.clone(), eager]
    ));
}

fn location(start: u64) -> Location {
    Location {
        path: Arc::from("app.tsx"),
        start_byte: start,
        end_byte: start + 1,
    }
}

fn read(symbol: &str, display: &str, origin: u64) -> SummaryRead {
    SummaryRead {
        transported: false,
        symbol: SymbolId::from(symbol),
        display: SymbolId::from(display),
        kind: None,
        declaration: location(0),
        origin: location(origin),
        origin_context: "test".to_owned(),
        owner: None,
        local: false,
        conditional: false,
        undecided: false,
    }
}

fn reads(entries: &[SummaryRead]) -> SummaryReads {
    let mut summary = SummaryReads::default();
    for entry in entries {
        summary.push(entry.clone());
    }
    summary
}

#[test]
fn captured_body_read_proof_requires_the_exact_synchronous_body() {
    use solid_facts::{
        FileFacts, ast,
        compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap},
        core::Generation,
    };
    let source = "const callback = () => { read(); const nested = () => later(); return <div data-value={jsx()} />; };";
    let ast = ast::extract("app.tsx", source).unwrap();
    let compiler = ExecutionMap {
        compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
        source_hash: ast.source.hash.clone(),
        semantic_model: Default::default(),
        tracked_regions: vec![],
        untracked_regions: vec![],
        discarded_regions: vec![],
        ownership_regions: vec![],
        callback_roles: vec![],
        jsx_operations: vec![],
    };
    let file = FileFacts::new(Generation::new(1).unwrap(), source, ast, compiler).unwrap();
    let function = file
        .ast
        .functions
        .iter()
        .find(|function| function.body.contains(file.ast.calls[0].span))
        .unwrap();
    for (callee, expected) in [("read", true), ("later", false), ("jsx", false)] {
        let call = file
            .ast
            .calls
            .iter()
            .find(|call| file.source_text(call.callee) == Some(callee))
            .unwrap();
        let mut row = read("accessor", callee, u64::from(call.span.start));
        row.origin.end_byte = u64::from(call.span.end);
        assert_eq!(
            super::captured_body_read_runs_during_call(&file, function, &row),
            expected
        );
        row.origin.path = "other.tsx".into();
        assert!(!super::captured_body_read_runs_during_call(
            &file, function, &row
        ));
        row.origin.path = file.path.shared();
        row.origin.start_byte = u64::MAX;
        assert!(!super::captured_body_read_runs_during_call(
            &file, function, &row
        ));
    }
}

/// The dispatch gate unions every candidate's reads once it returns true,
/// so a read only one candidate performs must fail it. Same-length
/// containment does not catch that: `[count, count]` and `[count, other]`
/// are the same length and every member of the first is in the second.
#[test]
fn dispatch_equivalence_preserves_transport_provenance() {
    let ordinary = read("sym-a", "count", 10);
    let mut transported = ordinary.clone();
    transported.transported = true;
    assert!(!equivalent_summary_reads(
        &reads(&[ordinary]),
        &reads(&[transported])
    ));
}

#[test]
fn dispatch_equivalence_rejects_a_read_only_one_candidate_performs() {
    let repeated = reads(&[read("sym-a", "count", 10), read("sym-a", "count", 11)]);
    let distinct = reads(&[read("sym-a", "count", 12), read("sym-b", "other", 13)]);
    assert!(!equivalent_summary_reads(&repeated, &distinct));
    assert!(!equivalent_summary_reads(&distinct, &repeated));
}

/// Reading one accessor twice is the same effect as reading it once, so a
/// differing repeat count -- or a differing origin line -- must not make
/// the dispatch fail closed.
#[test]
fn dispatch_equivalence_ignores_repeat_count_and_origin() {
    let twice = reads(&[read("sym-a", "count", 10), read("sym-a", "count", 11)]);
    let once = reads(&[read("sym-a", "count", 99)]);
    assert!(equivalent_summary_reads(&twice, &once));
    assert!(equivalent_summary_reads(&once, &twice));
}

#[test]
fn dispatch_equivalence_accepts_the_same_read_set() {
    let left = reads(&[read("sym-a", "count", 10), read("sym-b", "other", 11)]);
    let right = reads(&[read("sym-b", "other", 20), read("sym-a", "count", 21)]);
    assert!(equivalent_summary_reads(&left, &right));
}

#[test]
fn callback_equivalence_rejects_timing_only_one_candidate_declares() {
    let callback = |parameter, execution: &str| CallbackSummary {
        parameter,
        execution: execution.to_owned(),
        schedule: None,
        clears_tracking: false,
        arguments: Vec::new(),
        owner: None,
        protocol: crate::summary_semantics::InvokeProtocol::Call,
        path: Vec::new(),
    };
    let repeated = [callback(0, "deferred"), callback(0, "deferred")];
    let distinct = [callback(0, "deferred"), callback(1, "inline")];
    assert!(!equivalent_callbacks(&repeated, &distinct));
    assert!(!equivalent_callbacks(&distinct, &repeated));
    // The same timing set in a different order stays equivalent.
    let reordered = [callback(1, "inline"), callback(0, "deferred")];
    assert!(equivalent_callbacks(&distinct, &reordered));
}

#[test]
fn callback_equivalence_rejects_different_owner_contexts() {
    let inherited = CallbackSummary {
        parameter: 0,
        execution: "deferred".into(),
        schedule: None,
        clears_tracking: false,
        arguments: Vec::new(),
        owner: Some("inherited".into()),
        protocol: crate::summary_semantics::InvokeProtocol::Call,
        path: Vec::new(),
    };
    let leaf = CallbackSummary {
        owner: Some("leaf".into()),
        ..inherited.clone()
    };
    assert!(!equivalent_callbacks(&[inherited], &[leaf]));
}

#[test]
fn summary_reads_dedupe_on_display_origin_and_declaration_only() {
    let mut reads = SummaryReads::default();
    assert!(reads.push_unique(read("sym-a", "count", 10)));
    // A different underlying symbol with the same display, origin, and
    // declaration is the same read for summary purposes.
    assert!(!reads.push_unique(read("sym-b", "count", 10)));
    // Any differing key component makes the read new again.
    assert!(reads.push_unique(read("sym-a", "count", 11)));
    assert!(reads.push_unique(read("sym-a", "other", 10)));
    assert_eq!(reads.len(), 3);
}

#[test]
fn unconditional_push_appends_duplicates_but_still_records_the_key() {
    let mut reads = SummaryReads::default();
    reads.push(read("sym", "count", 10));
    reads.push(read("sym", "count", 10));
    // `push` never dedupes its own input...
    assert_eq!(reads.len(), 2);
    // ...but it seeds the key, so a later `push_unique` is refused.
    assert!(!reads.push_unique(read("sym", "count", 10)));
}

#[test]
fn replacing_the_reads_resets_the_dedupe_state() {
    let mut reads = SummaryReads::default();
    reads.push(read("sym", "count", 10));
    reads.replace(vec![read("sym", "name", 20)]);
    // The old key is forgotten, the replacement's key is live.
    assert!(reads.push_unique(read("sym", "count", 10)));
    assert!(!reads.push_unique(read("sym", "name", 20)));
    assert_eq!(
        reads.to_vec(),
        vec![read("sym", "name", 20), read("sym", "count", 10)]
    );
}

#[test]
fn effect_callback_executions_come_from_the_dialect() {
    let solid2 = solid_dialect::Solid2;
    assert_eq!(
        primitive_callback_execution(Some(Primitive::CreateEffect), 0, 2, &solid2),
        Some("tracked")
    );
    // 2.0's second effect argument is the deferred apply callback.
    assert_eq!(
        primitive_callback_execution(Some(Primitive::CreateEffect), 1, 2, &solid2),
        Some("deferred")
    );
    assert_eq!(
        primitive_callback_execution(Some(Primitive::CreateEffect), 2, 2, &solid2),
        None
    );
}

/// The table above answers "how would a callback here run"; this answers
/// "is there one". `@solid-primitives/flux-store`'s `createFluxStore` and
/// `@solidjs/meta`'s `Stylesheet` published `invoke` claims because the
/// second question was never asked.
#[test]
fn a_primitive_slot_roots_an_invoke_claim_only_with_its_premises() {
    use typefacts::Callability as C;
    let solid2 = solid_dialect::Solid2;
    let roots = |dialect: &dyn solid_dialect::Dialect, primitive, argument, count, callability| {
        primitive_slot_roots_parameter_invoke(
            dialect,
            Some(primitive),
            argument,
            count,
            callability,
        )
    };

    // A parameter the types prove non-callable is invoked by no slot, not
    // even an unconditional one.
    assert!(!roots(
        &solid2,
        Primitive::CreateMemo,
        0,
        1,
        Some(C::NonCallable)
    ));
    // ... and an unconditional slot keeps working with no answer at all,
    // which is every argument of an untyped JavaScript distribution.
    assert!(roots(&solid2, Primitive::CreateMemo, 0, 1, None));
    assert!(roots(
        &solid2,
        Primitive::CreateMemo,
        0,
        1,
        Some(C::Unknown)
    ));

    // 2.0's store and signal pairs are separated by callability and by
    // nothing else. The runtime dispatches on `typeof first === "function"`
    // with no arity condition (`@solidjs/signals` `dist/dev.js:9371`), and
    // the published server entrypoint's plain form carries no `NoFn`, so a
    // one-argument `createStore(compute)` is `tsc`-clean *and* derived. An
    // arity clause here dropped that true claim.
    for count in [1, 2, 3] {
        assert!(roots(
            &solid2,
            Primitive::CreateStore,
            0,
            count,
            Some(C::Callable)
        ));
        assert!(roots(
            &solid2,
            Primitive::CreateOptimisticStore,
            0,
            count,
            Some(C::Callable)
        ));
        // ... and the plain form of the same pair is withdrawn at the same
        // arities, which is `createFluxStore` under 1.0.0-next.2: the
        // proof is the callability answer, never the argument count.
        assert!(!roots(
            &solid2,
            Primitive::CreateStore,
            0,
            count,
            Some(C::Unknown)
        ));
        assert!(!roots(&solid2, Primitive::CreateStore, 0, count, None));
        assert!(!roots(
            &solid2,
            Primitive::CreateOptimisticStore,
            0,
            count,
            Some(C::Unknown)
        ));
    }
    // 2.0's `createSignal(fn, options?)` and `createOptimistic(fn)` are the
    // same shape one primitive over.
    for primitive in [Primitive::CreateSignal, Primitive::CreateOptimistic] {
        assert!(roots(&solid2, primitive, 0, 1, Some(C::Callable)));
        assert!(!roots(&solid2, primitive, 0, 1, None));
        assert!(!roots(&solid2, primitive, 0, 1, Some(C::Unknown)));
    }

    // `UntypedCallable` -- the signature-less `Function` supertype -- is a
    // *positive* callability proof with nothing to read from the
    // signature, so it satisfies the conditional premise exactly as
    // `Callable` does. This is the answer an artifact whose declaration
    // types the slot `Function` yields.
    for (dialect, primitive) in [
        (
            &solid2 as &dyn solid_dialect::Dialect,
            Primitive::CreateStore,
        ),
        (
            &solid2 as &dyn solid_dialect::Dialect,
            Primitive::CreateSignal,
        ),
    ] {
        assert!(roots(dialect, primitive, 0, 2, Some(C::UntypedCallable)));
    }
    assert!(!roots(
        &solid2,
        Primitive::CreateStore,
        0,
        2,
        Some(C::Mixed)
    ));
    assert!(!roots(
        &solid2,
        Primitive::CreateSignal,
        0,
        1,
        Some(C::Mixed)
    ));
    assert!(roots(&solid2, Primitive::CreateMemo, 0, 1, Some(C::Mixed)));

    // An unresolved callee is not a primitive at all; the general branch
    // downstream owns it, and only the non-callable proof applies here.
    assert!(primitive_slot_roots_parameter_invoke(
        &solid2, None, 0, 1, None
    ));
    assert!(!primitive_slot_roots_parameter_invoke(
        &solid2,
        None,
        0,
        1,
        Some(C::NonCallable)
    ));
}

#[test]
fn non_effect_callback_executions_use_the_module_classification() {
    let dialect = solid_dialect::Solid2;
    assert_eq!(
        primitive_callback_execution(Some(Primitive::CreateMemo), 0, 1, &dialect),
        Some("tracked")
    );
    // `untrack` and `flush` run their callback before returning, so the
    // execution word for both is `inline`. The
    // listener-clearing half is a separate dialect fact, not this word
    // (`untrack` clears the listener, `flush` does not).
    assert_eq!(
        primitive_callback_execution(Some(Primitive::Untrack), 0, 1, &dialect),
        Some("inline")
    );
    assert_eq!(
        primitive_callback_execution(Some(Primitive::Flush), 0, 1, &dialect),
        Some("inline")
    );
    assert_eq!(
        primitive_callback_execution(Some(Primitive::OnCleanup), 0, 1, &dialect),
        Some("deferred")
    );
    assert_eq!(
        primitive_callback_execution(Some(Primitive::CreateRoot), 0, 1, &dialect),
        Some("inline")
    );
    assert_eq!(
        primitive_callback_execution(Some(Primitive::RunWithOwner), 1, 2, &dialect),
        Some("inline")
    );
    assert_eq!(
        primitive_callback_execution(Some(Primitive::RunWithOwner), 0, 2, &dialect),
        None
    );
    assert_eq!(primitive_callback_execution(None, 0, 0, &dialect), None);
}

/// The composition rule, innermost wrapper first. Each row is a real shape
/// the corpus measurement produced a wrong claim for.
#[test]
fn a_callback_chain_composes_detachment_and_schedule_in_order() {
    use CallbackWrapper::{Detaching, Tracked, Transparent};
    const DEFERRED: CallbackWrapper = CallbackWrapper::Deferred {
        clears_tracking: false,
    };
    const DEFERRED_CLEARING: CallbackWrapper = CallbackWrapper::Deferred {
        clears_tracking: true,
    };
    // The tracked wrapper's schedule, which decides what a *detached*
    // callback under it composes to. 1.x `createEffect` is the deferring
    // one; 1.x `createMemo`/`createRenderEffect`/`mergeProps` and every
    // 2.0 effect are eager; a summary row has neither.
    const EAGER: CallbackWrapper = Tracked(Some(TrackedCallbackTiming::DuringCall));
    const LATER: CallbackWrapper = Tracked(Some(TrackedCallbackTiming::AfterCall));
    const UNKNOWN: CallbackWrapper = Tracked(None);

    // `use(fn, el, arg) { return untrack(() => fn(el, arg)) }` --
    // @solidjs/web. Runs before the export returns.
    assert_eq!(compose_callback_chain(&[Detaching]), Some(("inline", true)));
    // `createSubRoot(fn) { return createRoot(d => fn(d)) }` --
    // @solid-primitives/rootless. Same shape, same answer.
    assert_eq!(
        compose_callback_chain(&[Detaching, Transparent]),
        Some(("inline", true))
    );
    // `onMount(fn) { createEffect(() => untrack(fn)) }` -- solid-js. The
    // clearing wrapper stops `tracked`; the effect still schedules.
    assert_eq!(
        compose_callback_chain(&[Detaching, LATER]),
        Some(("deferred", true))
    );
    // `createMemo(() => untrack(fn))`, and its `createRenderEffect` and
    // `mergeProps` twins: the same chain shape with an *eager* tracked
    // wrapper runs `fn` during the call. Measured against solid-js@1.9.14
    // under `--conditions browser`: `ranDuringCall`, so a `deferred` claim
    // here is one the probe fails.
    assert_eq!(
        compose_callback_chain(&[Detaching, EAGER]),
        Some(("inline", true))
    );
    // No established schedule for the tracked wrapper: no word is honest,
    // and local openness is the answer rather than either guess.
    assert_eq!(compose_callback_chain(&[Detaching, UNKNOWN]), None);
    // Order is the answer: `untrack(() => createMemo(fn))` still tracks
    // `fn`, because the memo subscribes it and the outer untrack cannot
    // undo that. The wrapper's schedule is irrelevant once attribution
    // decides, so even the unknown one answers `tracked` here -- and the
    // clearing bit is never set on a `tracked` word.
    assert_eq!(
        compose_callback_chain(&[EAGER, Detaching]),
        Some(("tracked", false))
    );
    assert_eq!(
        compose_callback_chain(&[UNKNOWN, Detaching]),
        Some(("tracked", false))
    );
    // Nor does that outer clearing leak into a later tracked wrapper: the
    // inner memo still subscribes `fn`, so a second eager memo outside the
    // `untrack` cannot turn the answer back into `inline`.
    assert_eq!(
        compose_callback_chain(&[EAGER, Detaching, EAGER]),
        Some(("tracked", false))
    );
    // No clearing wrapper: the tracked claim survives untouched.
    assert_eq!(compose_callback_chain(&[LATER]), Some(("tracked", false)));
    assert_eq!(compose_callback_chain(&[UNKNOWN]), Some(("tracked", false)));
    assert_eq!(
        compose_callback_chain(&[Transparent, EAGER]),
        Some(("tracked", false))
    );
    // A transparent wrapper is exactly its call site.
    assert_eq!(
        compose_callback_chain(&[Transparent]),
        Some(("inline", false))
    );
    // Deferral is sticky in both directions, and it outranks the sentinel:
    // an inner wrapper that already runs later cannot be made earlier by
    // anything above it, so the outer schedule is not asked for.
    assert_eq!(
        compose_callback_chain(&[DEFERRED]),
        Some(("deferred", false))
    );
    assert_eq!(
        compose_callback_chain(&[DEFERRED, EAGER]),
        Some(("deferred", false))
    );
    // A clearing wrapper *outside* a deferral has returned by the time the
    // callback runs: `untrack(() => onCleanup(fn))` clears nothing for
    // `fn`. The old fold said `true` here, which was invisible only while
    // every deferred row published `untracked` whatever the bit said.
    assert_eq!(
        compose_callback_chain(&[DEFERRED, Detaching, UNKNOWN]),
        Some(("deferred", false))
    );
    assert_eq!(
        compose_callback_chain(&[LATER, DEFERRED]),
        Some(("deferred", false))
    );
    // A clearing *inside* the deferral is on the callback's stack when it
    // runs: `onCleanup(() => untrack(fn))`.
    assert_eq!(
        compose_callback_chain(&[Detaching, DEFERRED]),
        Some(("deferred", true))
    );
    // A deferral the dialect states untracked clears on its own, and no
    // wrapper outside it moves that.
    assert_eq!(
        compose_callback_chain(&[DEFERRED_CLEARING]),
        Some(("deferred", true))
    );
    assert_eq!(
        compose_callback_chain(&[DEFERRED_CLEARING, EAGER, Transparent]),
        Some(("deferred", true))
    );
    // ... but not what runs inside a tracked computation it defers: the
    // memo still subscribes `fn` when the deferred code builds it.
    assert_eq!(
        compose_callback_chain(&[EAGER, DEFERRED_CLEARING]),
        Some(("deferred", false))
    );
    assert_eq!(
        compose_callback_chain(&[Detaching, LATER, Transparent]),
        Some(("deferred", true))
    );
    // Two tracked wrappers above a clearing one: the outer one's schedule
    // is asked for too, so an unknown outer wrapper refuses even when the
    // inner one is established.
    assert_eq!(compose_callback_chain(&[Detaching, EAGER, UNKNOWN]), None);
}

/// G3 of ways-to-improve step 7: a local helper's own `inline` row is the
/// innermost wrapper of a forwarded callback. `runUntracked(fn) { return
/// untrack(fn) }` under `createMemo(() => runUntracked(cb))` runs `cb`
/// during the call with the listener cleared; composing the enclosing chain
/// alone published `tracked`.
#[test]
fn a_forwarded_inline_row_composes_from_the_callee_outward() {
    const EAGER: CallbackWrapper =
        CallbackWrapper::Tracked(Some(TrackedCallbackTiming::DuringCall));
    const UNKNOWN: CallbackWrapper = CallbackWrapper::Tracked(None);
    let under = |wrappers: Vec<CallbackWrapper>| ForwardedAmbientExecution::Composed { wrappers };

    assert_eq!(
        under(vec![EAGER]).restate(true),
        Some(("inline".to_owned(), None, true))
    );
    // The negative control: a helper that calls `fn` bare
    // (`trackedThroughLocalHelper`'s `runNow`) is transparent, and the memo
    // subscribes the callback during the call.
    assert_eq!(
        under(vec![EAGER]).restate(false),
        Some((
            "tracked".to_owned(),
            Some(CallbackSchedule::SameStack),
            false
        ))
    );
    // A clearing callee under a tracked wrapper that states no schedule:
    // no word is honest, and the caller opens the sentinel.
    assert_eq!(under(vec![UNKNOWN]).restate(true), None);
    assert_eq!(
        under(vec![UNKNOWN]).restate(false),
        Some((
            "tracked".to_owned(),
            Some(CallbackSchedule::Unestablished),
            false
        ))
    );
    // With nothing around the forwarding call, the callee's own answer is
    // the answer.
    assert_eq!(
        under(Vec::new()).restate(true),
        Some(("inline".to_owned(), None, true))
    );
    assert_eq!(
        under(Vec::new()).restate(false),
        Some(("inline".to_owned(), None, false))
    );
    // Only `Composed` restates.
    assert_eq!(ForwardedAmbientExecution::Callee.restate(true), None);
    assert_eq!(ForwardedAmbientExecution::Unknown.restate(true), None);
}

/// The word `tracked` says who owns the reads, never when the callback
/// runs. Emission asked the word and got `queued` for every retained row,
/// which 1.x `createMemo` and `mergeProps` -- both `DuringCall` -- break
/// before the export even returns.
#[test]
fn a_tracked_chain_takes_its_schedule_from_the_dialect_not_the_word() {
    use CallbackWrapper::{Detaching, Tracked, Transparent};
    const EAGER: CallbackWrapper = Tracked(Some(TrackedCallbackTiming::DuringCall));
    const LATER: CallbackWrapper = Tracked(Some(TrackedCallbackTiming::AfterCall));
    const UNKNOWN: CallbackWrapper = Tracked(None);

    // 1.x `createMemo(compute)` and `mergeProps(base, extras)`, and 2.0's
    // `createSignal(compute)` / `createOptimistic(compute)`: the compute has
    // already run when the creating call returns, so the export's own
    // stack is where it happened.
    assert_eq!(
        composed_tracked_schedule(&[EAGER]),
        CallbackSchedule::SameStack
    );
    assert_eq!(
        composed_tracked_schedule(&[Transparent, EAGER]),
        CallbackSchedule::SameStack
    );
    // 1.x `createEffect` and 2.0 `createTrackedEffect` only queue, and a
    // queueing wrapper anywhere in the chain puts the callback after the
    // export's return whatever else stands around it.
    assert_eq!(
        composed_tracked_schedule(&[LATER]),
        CallbackSchedule::Queued
    );
    assert_eq!(
        composed_tracked_schedule(&[EAGER, LATER]),
        CallbackSchedule::Queued
    );
    assert_eq!(
        composed_tracked_schedule(&[UNKNOWN, LATER]),
        CallbackSchedule::Queued
    );
    // 2.0's `createStore`/`createOptimisticStore` state no timing -- their
    // derived overloads never accepted the probe's call shape, so no
    // measurement backs a claim. The attribution row survives; the schedule
    // does not, and the emitted operation carries no execution point.
    assert_eq!(
        composed_tracked_schedule(&[UNKNOWN]),
        CallbackSchedule::Unestablished
    );
    assert_eq!(
        composed_tracked_schedule(&[EAGER, UNKNOWN]),
        CallbackSchedule::Unestablished
    );
    // Wrappers that run their code during their own call contribute
    // nothing: an empty chain and a purely transparent one are the
    // export's own stack.
    assert_eq!(composed_tracked_schedule(&[]), CallbackSchedule::SameStack);
    assert_eq!(
        composed_tracked_schedule(&[Detaching, Transparent]),
        CallbackSchedule::SameStack
    );
}

#[test]
fn reactive_sources_order_by_phase_before_location() {
    let phases = HashMap::from_iter([(SymbolId::from("late"), 2_u8)]);
    let early = cached_reactive_source("early", "early", &location(90), &phases);
    let late = cached_reactive_source("late", "late", &location(10), &phases);
    // `late` sits at an earlier byte but a later phase.
    assert_eq!(
        reactive_source_order(&early, &late),
        std::cmp::Ordering::Less
    );
    let sibling = cached_reactive_source("sibling", "sibling", &location(95), &phases);
    assert_eq!(
        reactive_source_order(&early, &sibling),
        std::cmp::Ordering::Less
    );
}

#[test]
fn retained_reactive_sources_build_sorted_and_reuse_the_cache() {
    let accessors = HashMap::from_iter([
        (SymbolId::from("a"), (SymbolId::from("count"), location(50))),
        (SymbolId::from("b"), (SymbolId::from("name"), location(10))),
        (SymbolId::from("c"), (SymbolId::from("gone"), location(5))),
    ]);
    // `c` never appears in the summaries, so it is ineligible.
    let summary_sources = HashSet::from_iter([SymbolId::from("a"), SymbolId::from("b")]);
    let phases = HashMap::from_iter([(SymbolId::from("a"), 2_u8)]);

    let mut cache = None;
    let first = retained_reactive_sources(&mut cache, &accessors, &summary_sources, &phases);
    assert_eq!(
        first
            .iter()
            .map(|source| (source.symbol.as_ref(), source.phase))
            .collect::<Vec<_>>(),
        [("b", 1), ("a", 2)]
    );

    let second = retained_reactive_sources(&mut cache, &accessors, &summary_sources, &phases);
    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn retained_reactive_sources_drop_stale_entries_and_insert_new_ones_in_order() {
    let mut accessors = HashMap::from_iter([
        (SymbolId::from("a"), (SymbolId::from("count"), location(50))),
        (SymbolId::from("b"), (SymbolId::from("name"), location(10))),
    ]);
    let mut summary_sources = HashSet::from_iter([SymbolId::from("a"), SymbolId::from("b")]);
    let phases = HashMap::default();

    let mut cache = None;
    retained_reactive_sources(&mut cache, &accessors, &summary_sources, &phases);

    // `b` stops being a summary source; `c` appears between the others.
    summary_sources.remove("b");
    summary_sources.insert(SymbolId::from("c"));
    accessors.insert(SymbolId::from("c"), (SymbolId::from("mid"), location(30)));
    let retained = retained_reactive_sources(&mut cache, &accessors, &summary_sources, &phases);
    assert_eq!(
        retained
            .iter()
            .map(|source| source.display.as_ref())
            .collect::<Vec<_>>(),
        ["mid", "count"]
    );
}

#[test]
fn dependency_users_are_reference_counted() {
    let dependency = InterproceduralResultDependency::Symbol(SymbolId::from("f"));
    let mut users = HashMap::default();
    let mut states = HashMap::default();
    states.insert(
        dependency.clone(),
        crate::cache::InterproceduralResultDependencyState::Missing,
    );
    add_interprocedural_dependency_user(&mut users, &dependency);
    add_interprocedural_dependency_user(&mut users, &dependency);

    remove_interprocedural_dependency_user(&mut users, &mut states, &dependency);
    // One user remains: both maps keep the dependency.
    assert!(users.contains_key(&dependency));
    assert!(states.contains_key(&dependency));

    remove_interprocedural_dependency_user(&mut users, &mut states, &dependency);
    assert!(users.is_empty());
    assert!(states.is_empty());
}
