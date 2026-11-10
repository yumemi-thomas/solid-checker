use super::*;

#[test]
fn callback_positions_are_the_dialects_headline_difference() {
    let two = &Solid2 as &dyn Dialect;

    // 1.x: createEffect(fn, value?) — the callback is first, and index 1 is
    // a seed VALUE. 2.0: createEffect(compute, apply) — index 1 is the
    // apply callback. Reading 1.x's seed as a callback is the single
    // highest-yield way to get this wrong.
    assert_eq!(two.callback_positions(Primitive::CreateEffect), &[1]);
    assert_eq!(two.callback_positions(Primitive::CreateRenderEffect), &[1]);

    // Unchanged across the split.
    assert_eq!(two.callback_positions(Primitive::CreateMemo), &[0]);
    assert_eq!(two.callback_positions(Primitive::Untrack), &[0]);
}

#[test]
fn deferred_execution_is_independent_of_callback_position() {
    let two = &Solid2 as &dyn Dialect;

    // The pair that makes this a separate question. Both put a callback at
    // index 0; one defers, one tracks. A rule that inferred "deferred"
    // from position would stop reporting reads inside every memo compute.
    assert_eq!(two.callback_positions(Primitive::Untrack), &[0]);
    assert_eq!(two.callback_positions(Primitive::CreateMemo), &[0]);
    assert!(two.runs_callback_deferred(Primitive::Untrack));
    assert!(!two.runs_callback_deferred(Primitive::CreateMemo));

    // 2.0 defers these imperative or loader callbacks even though each is
    // reachable from the call.
    let deferred = [
        Primitive::Untrack,
        Primitive::OnSettled,
        Primitive::CreateReaction,
        Primitive::Action,
        Primitive::ClientOnly,
    ];
    for primitive in deferred {
        assert!(two.runs_callback_deferred(primitive), "{primitive:?}");
    }
    for primitive in [
        Primitive::CreateTrackedEffect,
        Primitive::CreateSignal,
        Primitive::CreateStore,
        Primitive::CreateProjection,
        Primitive::CreateOptimistic,
        Primitive::CreateOptimisticStore,
        Primitive::Dynamic,
        // latest(fn) and isPending(fn) catch NotReadyError but do NOT
        // clear tracking: reads inside them subscribe in the caller's
        // scope, so classifying them as deferred would erase those read
        // obligations.
        Primitive::Latest,
        Primitive::IsPending,
        // flush(fn) runs fn() inline and touches neither the owner nor the
        // listener: a read inside it subscribes the caller (probed on
        // rc.0-rc.9, `Solid2::runs_callback_deferred`).
        Primitive::Flush,
    ] {
        assert!(!two.runs_callback_deferred(primitive), "{primitive:?}");
    }
}

/// The set every synchronous-clearing name resolves to, per dialect.
fn synchronous_clearing_names(dialect: &'static dyn Dialect) -> Vec<&'static str> {
    let mut names = dialect_names(dialect)
        .into_iter()
        .filter(|name| {
            dialect
                .primitive(name)
                .is_some_and(|primitive| dialect.runs_callback_synchronously(primitive))
        })
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names
}

/// `runs_callback_synchronously` is derived, so this test is not checking
/// an enumeration against itself: it pins the *concrete* set the derivation
/// produces, which is what callback summaries record as `inline`. A row
/// moving into or out of `callback_executions`, or a primitive joining
/// `runs_callback_deferred`, changes the summary of every package that
/// forwards a callback through it, and has to be a deliberate edit here.
#[test]
fn the_synchronous_clearing_set_is_the_inline_half_of_the_deferred_set() {
    let two = &Solid2 as &dyn Dialect;

    // `createRevealOrder` is here for the same reason `createRoot` is — it
    // clears tracking while establishing an owner and runs its callback
    // immediately. `flush` is not: `@solidjs/signals` `flush(fn)` runs
    // `fn()` inside a `try { return fn() } finally { flush(); syncDepth-- }`,
    // invoked during the call but with the caller's listener still current,
    // so a read inside it subscribes the caller (probed on rc.0-rc.9). It
    // is inline and transparent, as `latest` and `isPending` are.
    assert_eq!(
        synchronous_clearing_names(two),
        vec!["createRevealOrder", "createRoot", "runWithOwner", "untrack"]
    );
    assert!(!two.runs_callback_synchronously(Primitive::Flush));
    // ADR 0183: the slot form names each of them at its callback slot only.
    assert!(two.synchronous_callback_slot(Primitive::CreateRoot, 0, 1));
    assert!(two.synchronous_callback_slot(Primitive::Untrack, 0, 1));
    assert!(two.synchronous_callback_slot(Primitive::RunWithOwner, 1, 2));
    assert!(!two.synchronous_callback_slot(Primitive::RunWithOwner, 0, 2));
    assert!(!two.synchronous_callback_slot(Primitive::Flush, 0, 1));
    assert!(!two.synchronous_callback_slot(Primitive::CreateMemo, 0, 1));

    // The two halves of `runs_callback_deferred` stay separable: a
    // genuinely later callback is never synchronous, and a primitive the
    // dialect models no callback for is never either.
    for primitive in [
        Primitive::OnCleanup,
        Primitive::CreateReaction,
        Primitive::OnSettled,
        Primitive::Action,
        Primitive::Lazy,
    ] {
        assert!(!two.runs_callback_synchronously(primitive), "{primitive:?}");
    }
    // Inline but not listener-clearing: `batch` is transparent to its call
    // site, so it is not in this set either.
    assert!(!two.runs_callback_synchronously(Primitive::Latest));
}

/// Every name whose tracked callback this dialect gives `timing` for, at
/// any argument index a two-argument call could carry.
fn tracked_schedule_names(
    dialect: &'static dyn Dialect,
    timing: TrackedCallbackTiming,
) -> Vec<&'static str> {
    let mut names = dialect_names(dialect)
        .into_iter()
        .filter(|name| {
            dialect.primitive(name).is_some_and(|primitive| {
                (0..2).any(|argument| {
                    dialect.tracked_callback_timing(primitive, argument, 2) == Some(timing)
                })
            })
        })
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names
}

/// The eager/deferring/unestablished partition, pinned per dialect.
///
/// These sets decide what a clearing wrapper *inside* a tracked one
/// records, so a name entering or leaving one changes the summary of
/// every package with that shape. The unestablished side is pinned too: it
/// is the fail-closed arm, and silently promoting a member out of it is how
/// a guessed schedule would ship.
#[test]
fn the_lazy_option_defers_exactly_the_rc17_computed_callers() {
    // ADR 0576: rc.17 `computed` skips its first run under a truthy `lazy`;
    // `createMemo` and the function forms of `createSignal`/`createOptimistic`
    // hand it their second argument unchanged.
    let two = &Solid2 as &dyn Dialect;
    let mut lazy = dialect_names(two)
        .into_iter()
        .filter(|name| {
            two.primitive(name)
                .is_some_and(|primitive| two.lazy_option_argument(primitive) == Some(1))
        })
        .collect::<Vec<_>>();
    lazy.sort_unstable();
    lazy.dedup();
    assert_eq!(lazy, vec!["createMemo", "createOptimistic", "createSignal"]);
    assert_eq!(Silent.lazy_option_argument(Primitive::CreateMemo), None);
}

#[test]
fn the_tracked_callback_schedule_partitions_each_dialect() {
    let two = &Solid2 as &dyn Dialect;

    // 2.0 disagrees with 1.x on `createEffect` — `effect()` recomputes the
    // tracked compute during the call there — and its one deferring member
    // is `createTrackedEffect`, which only enqueues.
    assert_eq!(
        tracked_schedule_names(two, TrackedCallbackTiming::DuringCall),
        vec![
            "createEffect",
            "createMemo",
            "createOptimistic",
            "createProjection",
            "createRenderEffect",
            "createSignal"
        ]
    );
    assert_eq!(
        tracked_schedule_names(two, TrackedCallbackTiming::AfterCall),
        vec!["createTrackedEffect"]
    );

    for primitive in [
        Primitive::CreateStore,
        Primitive::CreateOptimisticStore,
        Primitive::Dynamic,
        Primitive::MapArray,
    ] {
        assert_eq!(
            two.tracked_callback_timing(primitive, 0, 2),
            None,
            "{primitive:?}"
        );
    }

    assert_eq!(
        two.tracked_callback_timing(Primitive::CreateEffect, 1, 2),
        None
    );
    assert_eq!(two.tracked_callback_timing(Primitive::Untrack, 0, 1), None);
}

#[test]
fn the_async_boundary_is_a_role_not_a_name() {
    let two = &Solid2 as &dyn Dialect;

    assert_eq!(two.boundary_kind("Loading"), Some(Boundary::Async));

    // Each dialect refuses the other's spelling.
    assert_eq!(two.boundary_kind("Suspense"), None);
}

#[test]
fn names_removed_in_solid_2_are_not_recognized() {
    let two = &Solid2 as &dyn Dialect;
    for name in ["batch", "createComputed", "createResource", "Suspense"] {
        assert_eq!(two.primitive(name), None, "{name} is gone in Solid 2.0");
    }
}

/// A dialect that states only what the trait *requires*.
///
/// The 26 methods with no default, each answering "nothing": no modules,
/// no primitives, no boundaries, no packages, no audited archives. It is
/// never registered and never analyzes anything. Its whole job is to be
/// asked the 36 *defaulted* questions, so the test below can say what a
/// second dialect gets for free before it has decided anything.
///
/// This is the cheap half of the second-dialect proof. `ALL`, the
/// `Version` variant and the compiler adapter are compile-time decisions
/// whose omission fails the build (`docs/adding-a-dialect.md`); a default
/// that silently hands 2.0's answer to a dialect that never stated one
/// fails nothing at all, which is why it is worth a test.
struct Silent;

impl Dialect for Silent {
    fn version(&self) -> Version {
        // Arbitrary and unused: `Silent` is never registered, and nothing
        // here resolves a dialect by version.
        Version::V1
    }
    fn modules(&self) -> &'static [&'static str] {
        &[]
    }
    fn primitive_defining_packages(&self) -> &'static [&'static str] {
        &[]
    }
    fn ecosystem_scopes(&self) -> &'static [&'static str] {
        &[]
    }
    fn primitive(&self, _name: &str) -> Option<Primitive> {
        None
    }
    fn name_of(&self, _primitive: Primitive) -> Option<&'static str> {
        None
    }
    fn callback_positions(&self, _primitive: Primitive) -> &'static [usize] {
        &[]
    }
    fn runs_callback_deferred(&self, _primitive: Primitive) -> bool {
        false
    }
    fn boundary_kind(&self, _tag: &str) -> Option<Boundary> {
        None
    }
    fn boundary_name(&self, _boundary: Boundary) -> &'static str {
        ""
    }
    fn cleanup_rule(&self, _primitive: Primitive) -> CleanupRule {
        CleanupRule::Never
    }
    fn accepts_cleanup_return(&self, _primitive: Primitive) -> bool {
        false
    }
    fn renders_children_through_callback(&self, _primitive: Primitive) -> bool {
        false
    }
    fn creates_reactive_source(&self, _primitive: Primitive) -> bool {
        false
    }
    fn creates_directive_owner(&self, _primitive: Primitive) -> bool {
        false
    }
    fn merges_props_reactivity(&self, _primitive: Primitive) -> bool {
        false
    }
    fn splits_props(&self, _primitive: Primitive) -> bool {
        false
    }
    fn returns_reactive_tuple(&self, _primitive: Primitive) -> bool {
        false
    }
    fn children_accessor_parameters(
        &self,
        _primitive: Primitive,
        _key: KeyForm,
    ) -> &'static [usize] {
        &[]
    }
    fn returns_store(&self, _primitive: Primitive) -> bool {
        false
    }
    fn options_argument(&self, _primitive: Primitive) -> Option<usize> {
        None
    }
    fn callback_executions(&self, _primitive: Primitive) -> &'static [(usize, Execution)] {
        &[]
    }
    fn export_modules(&self, _name: &str, _position: ExportPosition) -> Vec<&'static str> {
        Vec::new()
    }
    fn namespace_import_primitives(&self, _module: &str) -> &'static [&'static str] {
        &[]
    }
}

#[test]
fn a_resolved_solid_js_version_picks_its_dialect() {
    use Classification::{Modelled, UnmodelledMajor};
    assert_eq!(Version::for_solid_js("1.9.14"), Some(Modelled(Version::V1)));
    // 2.0 is still a prerelease; refusing to classify the RC would leave
    // every current 2.0 project on the caller's fallback.
    assert_eq!(
        Version::for_solid_js("2.0.0-rc.0"),
        Some(Modelled(Version::V2))
    );
    assert_eq!(Version::for_solid_js("^1.8.0"), Some(Modelled(Version::V1)));
    assert_eq!(Version::for_solid_js("v2.0.0"), Some(Modelled(Version::V2)));
    // A major nobody here names is still a major, and saying so is the
    // whole point: `None` would send an installed Solid 3 to the caller's
    // 2.0 default, which is the silent-wrong-language outcome ADR 0110
    // closed for 1.x. Major 0 classifies the same way -- `solid-js@0.x`
    // shipped, and a placeholder `0.0.0` stub is a contradicted answer
    // about a real install rather than an absence.
    assert_eq!(Version::for_solid_js("3.0.0"), Some(UnmodelledMajor(3)));
    assert_eq!(Version::for_solid_js("0.5.0"), Some(UnmodelledMajor(0)));
    // Not a version at all: the caller falls back, as it always has.
    assert_eq!(Version::for_solid_js("workspace:*"), None);
    assert_eq!(Version::for_solid_js(""), None);
}

#[test]
fn a_dialect_that_states_nothing_is_handed_no_vocabulary_by_a_default() {
    let silent = &Silent as &dyn Dialect;
    for primitive in [Primitive::CreateSignal, Primitive::CreateMemo] {
        for slot in [ResultSlot::Whole, ResultSlot::TupleItem(0)] {
            assert_eq!(silent.reactive_result_slot(primitive, slot), None);
        }
    }
    assert!(!silent.callback_inherits_calling_window(Primitive::CreateEffect, 1));
    assert!(!silent.synchronous_callback_slot(Primitive::CreateRoot, 0, 1));
    assert_eq!(
        silent.callback_execution_at(Primitive::CreateMemo, 0, 1),
        None
    );
}

/// The per-argument questions are independent, and the 2.0 answer to each
/// is pinned here.
///
/// This was a differential against the 1.x vocabulary, and every pair in
/// it had been a single hardcoded list in the engine holding 2.0's answer.
/// The contrast is gone with the 1.x dialect; the 2.0 half is kept in
/// full, because these are exactly the API shapes the checker must not
/// drift from — `createEffect(compute, apply)`, `createMemo(compute,
/// options)`, `createStore(value, options)`.
#[test]
fn the_argument_questions_are_independent_and_pinned_to_the_two_zero_api() {
    let two = &Solid2 as &dyn Dialect;

    // createEffect takes a tracked compute arm and a deferred apply arm.
    // 1.x's second argument was a seed, and a read in it was once reported
    // as running in an "apply callback" 1.x did not have.
    assert_eq!(
        two.callback_executions(Primitive::CreateEffect),
        [(0, Execution::Tracked), (1, Execution::Deferred)]
    );

    // createMemo is `(compute, options?)` — 1.x's positional `value`
    // parameter is gone, so the options slot moved from 2 to 1.
    assert_eq!(two.options_argument(Primitive::CreateMemo), Some(1));
    assert!(two.supports_sync_option(Primitive::CreateMemo));
    // createStore keeps a value parameter, so its options slot is 2.
    assert_eq!(two.options_argument(Primitive::CreateStore), Some(2));
    // The store family has an options slot but no `sync` routing: rc.0
    // rebuilds projection node options with only `loadingValue`/`name`,
    // so `sync: true` is inert on all three constructors (probed).
    assert!(!two.supports_sync_option(Primitive::CreateStore));
    assert!(!two.supports_sync_option(Primitive::CreateProjection));
    assert!(!two.supports_sync_option(Primitive::CreateOptimisticStore));

    // createComputed does not exist in 2.0 at all.
    assert!(
        two.callback_executions(Primitive::CreateComputed)
            .is_empty()
    );

    // Stores: createStore and the projection pair are 2.0's; createMutable
    // was 1.x's and 2.0 does not have it.
    assert!(two.returns_store(Primitive::CreateStore));
    assert!(two.returns_store(Primitive::CreateProjection));
    assert!(!two.returns_store(Primitive::CreateMutable));
}

/// The generated tables are binary-searched, so their order is load
/// bearing. A generator that stopped sorting would not fail to compile —
/// it would silently start missing names.
#[test]
fn the_generated_export_index_is_sorted() {
    for (label, table) in [
        ("2.0 values", exports::solid_v2_solid_js::VALUES),
        ("2.0 types", exports::solid_v2_solid_js::TYPES),
        ("web values", exports::solid_v2_solidjs_web::VALUES),
        ("web types", exports::solid_v2_solidjs_web::TYPES),
    ] {
        assert!(!table.is_empty(), "{label} is empty");
        assert!(
            table.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "{label} is not sorted by name"
        );
        for (name, modules) in table {
            assert!(!modules.is_empty(), "{label}: {name} lists no module");
        }
    }
}

#[test]
fn concrete_callback_semantics_cover_overloads_and_both_dialects() {
    let two = &Solid2 as &dyn Dialect;

    assert_eq!(
        two.callback_execution_at(Primitive::CreateSignal, 0, 1),
        Some(Execution::Tracked)
    );
    assert_eq!(
        two.callback_execution_at(Primitive::CreateEffect, 1, 2),
        Some(Execution::Deferred)
    );

    for (primitive, argument, execution) in [
        (Primitive::Action, 0, Execution::Deferred),
        (Primitive::Flush, 0, Execution::Inline),
        (Primitive::CreateErrorBoundary, 1, Execution::Tracked),
        (Primitive::CreateLoadingBoundary, 1, Execution::Tracked),
        (Primitive::RepeatMap, 0, Execution::Tracked),
        (Primitive::RepeatMap, 1, Execution::Inline),
    ] {
        assert_eq!(
            two.callback_execution_at(primitive, argument, 2),
            Some(execution),
            "missing 2.0 callback semantics for {primitive:?}[{argument}]"
        );
    }
}

#[test]
fn each_dialect_knows_its_own_reactive_source_factories() {
    let two = &Solid2 as &dyn Dialect;

    for primitive in [
        Primitive::CreateSignal,
        Primitive::CreateMemo,
        Primitive::CreateStore,
    ] {
        assert!(two.creates_reactive_source(primitive));
    }

    // 1.x-only factories. `createResource` is the one that matters most:
    // it returns a tuple, so a single-value `returns` summary cannot
    // describe it and only this answer finds it.
    for primitive in [
        Primitive::CreateResource,
        Primitive::CreateMutable,
        Primitive::CreateSelector,
    ] {
        assert!(
            !two.creates_reactive_source(primitive),
            "{primitive:?} is not 2.0 vocabulary at all"
        );
    }

    // Returns nothing, so it is not a source however reactive it is.
    // 2.0-only factories, and rc.17's `createDeferred` (a memo).
    for primitive in [
        Primitive::CreateProjection,
        Primitive::CreateOptimistic,
        Primitive::CreateDeferred,
    ] {
        assert!(two.creates_reactive_source(primitive));
    }
}

#[test]
fn module_ownership_follows_the_dialects_package_layout() {
    let two = &Solid2 as &dyn Dialect;

    // 1.x splits stores and DOM into subpaths; importing createStore from
    // "solid-js" is wrong there and right in 2.0.
    assert!(!two.owns_module("solid-js/store"));
    assert!(two.owns_module("@solidjs/web"));
    assert!(two.owns_module("solid-js"));
}

/// Four of the five control-flow components were shared with 1.x; the
/// fifth was the point of the old differential. What matters now is the
/// 2.0 list itself, because a function written inside one of these is a
/// callback, and reading the wrong list means reading it as a component.
#[test]
fn control_flow_components_render_children_through_a_callback() {
    let two = &Solid2 as &dyn Dialect;
    for primitive in [
        Primitive::For,
        Primitive::Show,
        Primitive::Match,
        Primitive::Switch,
        // `Repeat` is 2.0's fifth; 1.x had `Index` here instead.
        Primitive::Repeat,
    ] {
        assert!(two.renders_children_through_callback(primitive));
    }
    assert!(!two.renders_children_through_callback(Primitive::Index));

    // Boundaries render children directly, not through a callback.
    assert!(!two.renders_children_through_callback(Primitive::Loading));
}

/// The aggregate answers a result slot only from rows that exist.
#[test]
fn reactive_result_slots_answer_only_where_a_row_exists() {
    let two = &Solid2 as &dyn Dialect;
    assert_eq!(
        two.reactive_result_slot(Primitive::CreateSignal, ResultSlot::TupleItem(0)),
        Some(ReactiveRole::Accessor)
    );
    assert_eq!(
        two.reactive_result_slot(Primitive::CreateStore, ResultSlot::TupleItem(0)),
        None,
        "a store slot has no accessor row"
    );
}

#[test]
fn callback_result_identity_is_an_explicit_dialect_claim() {
    let two = &Solid2 as &dyn Dialect;
    assert_eq!(two.returned_callback_value(Primitive::CreateRoot), Some(0));
    for primitive in [
        Primitive::Untrack,
        Primitive::RunWithOwner,
        Primitive::CreateMemo,
    ] {
        assert_eq!(two.returned_callback_value(primitive), None);
    }
}

/// ADR 0162: the options argument of `createSignal` and `createMemo` is at
/// index 1 in the audited dialect, and a name the dialect does not export has
/// none.
#[test]
fn the_options_argument_is_the_dialects_own() {
    let two = &Solid2 as &dyn Dialect;
    let options_argument = |name: &str| two.primitive(name).and_then(|p| two.options_argument(p));
    assert_eq!(options_argument("createSignal"), Some(1));
    assert_eq!(options_argument("createMemo"), Some(1));
    assert_eq!(options_argument("createStore"), Some(2));
    assert_eq!(options_argument("onCleanup"), None);
    assert_eq!(options_argument("notADialectName"), None);
}

fn dialect_names(dialect: &'static dyn Dialect) -> Vec<&'static str> {
    match dialect.version() {
        // `Version::V1` survives for classification only; no vocabulary
        // behind it means no name list, and `DIALECTS` never yields one.
        Version::V1 => Vec::new(),
        // A name only some installations export (`until`) is listed for
        // the vocabularies that export it; every other row is listed for
        // all of them, so a row that fails to resolve still fails.
        Version::V2 => solid_2::names()
            .into_iter()
            .filter(|name| {
                !solid_2::RELEASE_GATED_NAMES.contains(name) || dialect.primitive(name).is_some()
            })
            .collect(),
    }
}

#[test]
fn owner_requirement_roles_need_exact_dialect_ownership() {
    let two = &Solid2 as &dyn Dialect;
    // Only the dialect's canonical spelling of a primitive carries its role.
    let role = |name: &str| {
        let primitive = two.primitive(name)?;
        (two.name_of(primitive) == Some(name))
            .then(|| two.owner_requirement_role(primitive))
            .flatten()
    };
    assert_eq!(role("createEffect"), Some(OwnerRequirementRole::Effect));
    assert_eq!(role("onCleanup"), Some(OwnerRequirementRole::Cleanup));
    assert_eq!(
        role("onSettled"),
        Some(OwnerRequirementRole::SettledCleanup)
    );
    // ADR 0352: the read throws without an owner; creating a context does not.
    assert_eq!(role("useContext"), Some(OwnerRequirementRole::ContextRead));
    assert_eq!(role("createContext"), None);
    // ADR 0462: a boundary component called as a function creates its boundary.
    assert_eq!(role("Loading"), Some(OwnerRequirementRole::Boundary));
    assert_eq!(role("Errored"), Some(OwnerRequirementRole::Boundary));
    assert_eq!(role("Show"), None);
    assert_eq!(role("effect"), None);
    assert_eq!(role("createUnknown"), None);
}

#[test]
fn implementation_roles_follow_the_dialects_vocabulary() {
    let two = &Solid2 as &dyn Dialect;
    let callback_argument = |name: &str, argument: usize, argument_count: usize| {
        two.primitive(name)
            .filter(|primitive| two.name_of(*primitive) == Some(name))
            .and_then(|primitive| two.callback_execution_at(primitive, argument, argument_count))
            .is_some()
    };
    assert!(callback_argument("createMemo", 0, 1));
    // `createEffect`'s second argument is the apply callback in 2.0.
    assert!(callback_argument("createEffect", 1, 2));
    assert!(!callback_argument("effect", 0, 1));
    let callable_type = |origin_module: &str, name: &str| {
        matches!(
            two.type_role(origin_module, name),
            Some(TypeRole::Accessor | TypeRole::Setter)
        )
    };
    assert!(callable_type("solid-js", "Accessor"));
    assert!(callable_type("solid-js", "Setter"));
    assert!(!callable_type("user-module", "Accessor"));
    assert!(!callable_type("solid-js", "Signal"));
}

// The reactive result table answers a *slot*: a proof that read a whole-value
// callability answer for reactivity would certify the setter as an accessor.
#[test]
fn reactive_result_slots_are_per_slot() {
    let two = &Solid2 as &dyn Dialect;
    let slot_role = |name: &str, slot: ResultSlot| {
        two.primitive(name)
            .filter(|primitive| two.name_of(*primitive) == Some(name))
            .and_then(|primitive| two.reactive_result_slot(primitive, slot))
    };
    assert_eq!(
        slot_role("createSignal", ResultSlot::TupleItem(0)),
        Some(ReactiveRole::Accessor)
    );
    assert_eq!(
        slot_role("createSignal", ResultSlot::TupleItem(1)),
        Some(ReactiveRole::Setter)
    );
    assert_eq!(
        slot_role("createMemo", ResultSlot::Whole),
        Some(ReactiveRole::Accessor)
    );

    // Silence is not a role. A slot past the audited tuple, the whole of a
    // value whose slots are audited, a slot of a value whose whole is, a
    // primitive with no row at all, and a name the dialect does not export
    // all answer `None` -- and none of those answers may be read as "not
    // reactive".
    assert_eq!(slot_role("createSignal", ResultSlot::TupleItem(2)), None);
    assert_eq!(slot_role("createSignal", ResultSlot::Whole), None);
    assert_eq!(slot_role("createMemo", ResultSlot::TupleItem(0)), None);
    assert_eq!(slot_role("createEffect", ResultSlot::Whole), None);
    assert_eq!(slot_role("createUnknown", ResultSlot::TupleItem(0)), None);

    // A store result is not an accessor result.
    assert_eq!(slot_role("createMutable", ResultSlot::Whole), None);
    assert_eq!(slot_role("createProjection", ResultSlot::Whole), None);
}

// The module premise replaces a `== "solid-js"` literal in the certifier.
// Its whole point is the empty module: a package's own locally declared
// `createSignal` reports no module, and must never answer a question about
// the dialect's.
#[test]
fn value_export_modules_answer_only_for_audited_dialect_modules() {
    let two = &Solid2 as &dyn Dialect;
    let exports_value_from = |origin_module: &str, name: &str| {
        two.export_modules(name, ExportPosition::Value)
            .contains(&origin_module)
    };
    assert!(exports_value_from("solid-js", "createSignal"));
    assert!(exports_value_from("solid-js", "createMemo"));
    // 2.0 folded the store API into core, so `createStore` is exported
    // from `solid-js` and the 1.x `solid-js/store` subpath is not a module
    // any compiled vocabulary owns.
    assert!(exports_value_from("solid-js", "createStore"));
    assert!(!exports_value_from("solid-js/store", "createStore"));
    assert!(!exports_value_from("", "createSignal"));
    assert!(!exports_value_from("solid-js", ""));
    assert!(!exports_value_from("my-signals", "createSignal"));
    assert!(!exports_value_from("solid-js", "createUnknown"));
    // A type-position-only export is not a value export.
    assert!(!exports_value_from("solid-js", "Accessor"));
}

// Exact names, unioned across dialects, and no subpath or prefix reach:
// the predicate names *archives*, and a package that merely lives under
// the `@solidjs` scope is a consumer of these three, not one of them.
#[test]
fn primitive_defining_packages_are_exact_archive_names() {
    assert!(primitive_defining_package("solid-js"));
    assert!(primitive_defining_package("@solidjs/signals"));
    assert!(primitive_defining_package("@solidjs/web"));
    assert!(!primitive_defining_package(""));
    assert!(!primitive_defining_package("solid-js/store"));
    assert!(!primitive_defining_package("solid-js/web"));
    assert!(!primitive_defining_package("@solidjs"));
    assert!(!primitive_defining_package("@solidjs/router"));
    assert!(!primitive_defining_package("@solidjs/meta"));
    assert!(!primitive_defining_package("@solidjs/start"));
    assert!(!primitive_defining_package("@solidjs/element"));
    assert!(!primitive_defining_package("solid-js-signals"));
    assert!(!primitive_defining_package("my-solid-js"));
}

/// The specifier form reaches subpaths, which the archive-name form must
/// not, and it is derived from the same list rather than repeating it.
///
/// This predicate has a use that the archive-name one is documented never
/// to have: `module_closure::record_external` uses it to *not* record an
/// opaque frontier. Its admissibility is argued at that call site — the
/// exemption establishes no claim, it only declines to withdraw every
/// domain — and pinned here so the two forms cannot drift into answering
/// differently about the same package.
#[test]
fn the_core_runtime_specifier_form_reaches_subpaths_of_the_same_archives() {
    for name in ["solid-js", "@solidjs/signals", "@solidjs/web"] {
        assert!(primitive_defining_package(name), "{name}");
        assert!(core_runtime_specifier(name), "{name}");
        assert!(
            core_runtime_specifier(&format!("{name}/store")),
            "{name}/store"
        );
    }
    // The archive-name form refuses a subpath; the specifier form is the
    // one that must accept it.
    assert!(!primitive_defining_package("solid-js/store"));
    assert!(core_runtime_specifier("solid-js/store"));

    // Everything the archive-name form refuses outright, this refuses too:
    // a neighbouring scope member, a prefix that is not a path boundary,
    // and the empty specifier.
    for other in [
        "",
        "@solidjs",
        "@solidjs/router",
        "@solidjs/meta",
        "solid-js-signals",
        "my-solid-js",
        "solid-jsx",
    ] {
        assert!(!core_runtime_specifier(other), "{other:?}");
    }
}

/// Every primitive a dialect can answer for: each exported name it resolves, and the two forms `dynamic`
/// takes from its options (no name maps to them, [`Dialect::call_form`]).
fn profiled_primitives(dialect: &'static dyn Dialect) -> Vec<Primitive> {
    let mut primitives = dialect_names(dialect)
        .into_iter()
        .filter_map(|name| dialect.primitive(name))
        .chain([Primitive::DynamicStatic, Primitive::DynamicUnknownForm])
        .collect::<Vec<_>>();
    primitives.sort_by_key(|primitive| format!("{primitive:?}"));
    primitives.dedup();
    primitives
}

/// What every callback question answers for one slot of one call shape (ADR 0941), grouped by the fact each
/// answers: when and where the callback runs, which owner, which observer, the strict-read window, an action
/// step, the effect phase, and a thrown error. The rules ask these separately, so a slot the table answers
/// in one group and not in another is a slot some rule walks past.
#[derive(Clone, Copy, Debug)]
struct SlotProfile {
    execution: Option<Execution>,
    clears_listener: bool,
    synchronous: bool,
    owner: Option<CallbackOwner>,
    owner_write_preserved: bool,
    created_root: bool,
    created_computation: bool,
    keeps_observer: bool,
    clears_tracking_in_window: bool,
    inherits_window: bool,
    action_steps: bool,
    action_inherits_window: bool,
    in_effect_phase: bool,
    reruns_in_effect_phase: bool,
    swallows_throw: bool,
}

impl SlotProfile {
    fn of(dialect: &dyn Dialect, primitive: Primitive, argument: usize, count: usize) -> Self {
        Self {
            execution: dialect.callback_execution_at(primitive, argument, count),
            clears_listener: dialect.runs_callback_deferred(primitive),
            synchronous: dialect.synchronous_callback_slot(primitive, argument, count),
            owner: dialect.callback_owner_at(primitive, argument, count),
            owner_write_preserved: dialect.callback_preserves_owner_write_context(primitive),
            created_root: dialect.callback_runs_in_created_root(primitive, argument),
            created_computation: dialect.callback_runs_as_created_computation(primitive, argument),
            keeps_observer: dialect.inline_callback_keeps_observer(primitive, argument),
            clears_tracking_in_window: dialect
                .callback_clears_tracking_in_calling_window(primitive, argument),
            inherits_window: dialect.callback_inherits_calling_window(primitive, argument),
            action_steps: dialect.callback_runs_as_action_steps(primitive, argument),
            action_inherits_window: dialect
                .action_step_inherits_calling_window(primitive, argument),
            in_effect_phase: dialect.callback_runs_in_effect_phase(primitive, argument),
            reruns_in_effect_phase: dialect.callback_reruns_in_effect_phase(primitive, argument),
            swallows_throw: dialect.callback_swallows_throw(primitive, argument),
        }
    }

    /// The callback runs on the caller's stack during the call, under the caller's owner: nothing the
    /// primitive does moves the code inside it off the stack of its call.
    fn transparent(&self) -> bool {
        self.execution == Some(Execution::Inline) && self.owner == Some(CallbackOwner::Inherits)
    }

    /// The compact form the golden table pins: the execution word, the owner, and each fact that holds.
    fn spelled(&self) -> String {
        let facts = [
            (self.clears_listener, "clears-listener"),
            (self.owner_write_preserved, "owner-write-preserved"),
            (self.keeps_observer, "keeps-observer"),
            (self.clears_tracking_in_window, "clears-tracking-in-window"),
            (self.inherits_window, "inherits-window"),
            (self.swallows_throw, "swallows-throw"),
            (self.created_root, "created-root"),
        ]
        .into_iter()
        .filter_map(|(holds, name)| holds.then_some(name))
        .collect::<Vec<_>>();
        format!(
            "{:?}/{:?} [{}]",
            self.execution,
            self.owner,
            facts.join(" ")
        )
    }
}

/// The implications that tie the callback questions together, each as `(name, holds)` for one profile.
/// Every one is a statement the questions' own documentation makes, so a table row that breaks one is a row
/// that contradicts another question's answer for the same slot, whichever of the two is wrong.
fn slot_implications(profile: &SlotProfile) -> Vec<(&'static str, bool)> {
    let inline = profile.execution == Some(Execution::Inline);
    let implies = |premise: bool, conclusion: bool| !premise || conclusion;
    vec![
        // Owner. A write meets the guard of the ambient owner, so a callback that runs on the caller's stack
        // under the caller's owner is judged where the call is; the converse and the root rows follow.
        (
            "an inline callback that inherits the owner preserves the owner's write context",
            implies(profile.transparent(), profile.owner_write_preserved),
        ),
        (
            "a primitive that preserves the owner's write context runs its inline callbacks under the caller's owner",
            implies(
                profile.owner_write_preserved && inline,
                profile.owner == Some(CallbackOwner::Inherits),
            ),
        ),
        (
            "a callback that runs in a created root has the owner the call creates, not the caller's write context",
            implies(
                profile.created_root,
                profile.owner == Some(CallbackOwner::Creates) && !profile.owner_write_preserved,
            ),
        ),
        (
            "a callback that runs as a created computation has the owner the call creates",
            implies(
                profile.created_computation,
                profile.owner == Some(CallbackOwner::Creates),
            ),
        ),
        // Observer. An inline callback of a primitive that leaves the listener alone runs under the caller's
        // observer, so a guard that throws on an active observer throws in it; the converse is the claim itself.
        (
            "an inline callback that inherits the owner and leaves the listener alone keeps the caller's observer",
            implies(
                profile.transparent() && !profile.clears_listener,
                profile.keeps_observer,
            ),
        ),
        (
            "a callback that keeps the caller's observer is inline, under the caller's owner, and no listener is cleared",
            implies(
                profile.keeps_observer,
                profile.transparent() && !profile.clears_listener,
            ),
        ),
        (
            "a callback that clears tracking in the calling window is inline and clears the listener",
            implies(
                profile.clears_tracking_in_window,
                inline && profile.clears_listener,
            ),
        ),
        // Strict-read window. The window belongs to the call position; a callback keeps it only by running
        // inline, and these two keep it while clearing the listener.
        (
            "a callback that inherits the calling window is inline and clears the listener",
            implies(profile.inherits_window, inline && profile.clears_listener),
        ),
        (
            "a callback that clears tracking in the calling window inherits that window",
            implies(profile.clears_tracking_in_window, profile.inherits_window),
        ),
        // The derived slot: inline, and clearing the listener.
        (
            "a synchronous slot is inline and clears the listener",
            implies(profile.synchronous, inline && profile.clears_listener),
        ),
        // Action steps run from the action's own driver, never at the call.
        (
            "a stepped callback does not run inline at the call",
            implies(
                profile.action_steps,
                profile.execution == Some(Execution::Deferred),
            ),
        ),
        (
            "a stepped callback that inherits the calling window is a stepped callback",
            implies(profile.action_inherits_window, profile.action_steps),
        ),
        // Effect phase. The first run and the later runs are different claims; neither is an inline callback.
        (
            "a callback is in the effect phase on every run or on the later ones, not both",
            !(profile.in_effect_phase && profile.reruns_in_effect_phase),
        ),
        (
            "an effect-phase callback is not run inline at the call",
            implies(
                profile.in_effect_phase || profile.reruns_in_effect_phase,
                !inline,
            ),
        ),
        // Throws. A primitive can only catch what the callback throws on its own stack.
        (
            "a callback whose throw the call swallows runs inline",
            implies(profile.swallows_throw, inline),
        ),
    ]
}

/// ADR 0941: the callback questions agree for every slot of every primitive, in every vocabulary
/// (`solid-js` rc.21's `latest` and `isPending` were the two that did not: both run `fn()` on the caller's
/// stack, leaving the owner, the listener and the action-step marker alone, `@solidjs/signals@2.0.0-rc.21`
/// `dist/dev.js:1654-1665` and `:1699-1726`, but the owner-write and observer questions answered nothing for
/// them, which is how `latest(() => resolve(...))` in a tracked effect certified clean).
#[test]
fn the_callback_questions_agree_for_every_slot_of_every_primitive() {
    // The vocabularies: the language's own, each read release's triple, and a stride over the other release
    // facets (663,551 combinations: every one of them takes minutes, a stride covers the mixes).
    let two = &Solid2 as &'static dyn Dialect;
    let mut dialects: Vec<&'static dyn Dialect> = vec![
        two,
        &Solid2::RC3,
        &Solid2::RC9,
        &Solid2::RC13,
        &Solid2::RC17,
        &Solid2::RC21,
    ];
    dialects.extend(two.variants().iter().copied().step_by(10_007));
    let mut failures = std::collections::BTreeMap::<String, usize>::new();
    let mut slots = 0;
    for dialect in dialects {
        for primitive in profiled_primitives(dialect) {
            for argument in 0..3 {
                for count in argument + 1..=4 {
                    slots += 1;
                    let profile = SlotProfile::of(dialect, primitive, argument, count);
                    for (name, holds) in slot_implications(&profile) {
                        if !holds {
                            *failures
                                .entry(format!("{primitive:?} argument {argument}: {name}"))
                                .or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(
        slots > 10_000,
        "{slots} slots: the enumeration lost its primitives"
    );
    let report = failures
        .iter()
        .map(|(failure, shapes)| format!("  {failure} ({shapes} vocabulary and call shapes)"))
        .collect::<Vec<_>>();
    assert!(
        report.is_empty(),
        "{} disagreement(s):\n{}",
        report.len(),
        report.join("\n")
    );
}

/// The transparent and the clearing wrappers, pinned slot by slot: a row that moves is a deliberate edit
/// here. `latest` and `isPending` differ in one fact, the throw `isPending` catches (rc.21 `dev.js:1710`: a
/// thunk that errors is simply not pending, except a not-ready read from inside an owner).
#[test]
fn the_wrappers_that_run_a_callback_inline_are_pinned_slot_by_slot() {
    let two = &Solid2 as &dyn Dialect;
    let pinned = [
        (
            Primitive::Untrack,
            0,
            "Some(Inline)/Some(Inherits) [clears-listener owner-write-preserved]",
        ),
        (
            Primitive::Flush,
            0,
            "Some(Inline)/Some(Inherits) [owner-write-preserved keeps-observer]",
        ),
        (
            Primitive::Latest,
            0,
            "Some(Inline)/Some(Inherits) [owner-write-preserved keeps-observer]",
        ),
        (
            Primitive::IsPending,
            0,
            "Some(Inline)/Some(Inherits) [owner-write-preserved keeps-observer swallows-throw]",
        ),
        (
            Primitive::DispatchAsInteraction,
            1,
            "Some(Inline)/Some(Inherits) [owner-write-preserved keeps-observer]",
        ),
        (
            Primitive::RunWithOwner,
            1,
            "Some(Inline)/Some(Conditional) [clears-listener clears-tracking-in-window inherits-window]",
        ),
        (
            Primitive::CreateRoot,
            0,
            "Some(Inline)/Some(Creates) [clears-listener clears-tracking-in-window inherits-window created-root]",
        ),
    ];
    for (primitive, argument, spelled) in pinned {
        assert_eq!(
            SlotProfile::of(two, primitive, argument, argument + 1).spelled(),
            spelled,
            "{primitive:?} argument {argument}"
        );
    }
}

/// ADR 0944: `slot_run` is the one answer the rule walks match, so a row that moves is a deliberate edit here.
/// The slots are the ones the runtime runs on the caller's stack (`@solidjs/signals@2.0.0-rc.21`: `untrack`
/// `dist/dev-shared.js:5842`, `runWithOwner` `:6740`, `latest` `dist/dev.js:1654`, `isPending` `:1699`), a
/// computation's compute, and the callbacks that run later.
#[test]
fn the_slot_run_of_the_primitives_that_take_a_callback_is_pinned() {
    use SlotObserver::{Clears, Keeps, Unstated};
    use SlotOwner::{Creates, Inherits, Supplied};
    use SlotThrows::{Escapes, Swallows};
    let two = &Solid2 as &dyn Dialect;
    let in_place = |owner, observer, throws| {
        SlotRun::InPlace(InPlaceRun {
            owner,
            observer,
            throws,
        })
    };
    let pinned = [
        (Primitive::Untrack, 0, in_place(Inherits, Clears, Escapes)),
        (
            Primitive::DynamicStatic,
            0,
            in_place(Inherits, Clears, Escapes),
        ),
        (Primitive::Latest, 0, in_place(Inherits, Keeps, Escapes)),
        (Primitive::IsPending, 0, in_place(Inherits, Keeps, Swallows)),
        (Primitive::Flush, 0, in_place(Inherits, Keeps, Escapes)),
        (
            Primitive::RunWithOwner,
            1,
            in_place(Supplied { argument: 0 }, Clears, Escapes),
        ),
        (Primitive::CreateRoot, 0, in_place(Creates, Clears, Escapes)),
        (
            Primitive::CreateRevealOrder,
            0,
            in_place(Creates, Clears, Escapes),
        ),
        (Primitive::Render, 0, in_place(Creates, Unstated, Escapes)),
        (Primitive::CreateMemo, 0, SlotRun::Computation),
        (Primitive::CreateTrackedEffect, 0, SlotRun::Computation),
        (Primitive::CreateEffect, 0, SlotRun::Computation),
        (Primitive::CreateEffect, 1, SlotRun::Deferred),
        (Primitive::OnSettled, 0, SlotRun::Deferred),
        (Primitive::OnCleanup, 0, SlotRun::Deferred),
        (Primitive::Action, 0, SlotRun::Deferred),
        // No execution row, no word: nothing is claimed.
        (Primitive::CreateMemo, 2, SlotRun::Unknown),
        (Primitive::GetOwner, 0, SlotRun::Unknown),
    ];
    for (primitive, argument, expected) in pinned {
        assert_eq!(
            two.slot_run(primitive, argument, argument + 1),
            expected,
            "{primitive:?} argument {argument}"
        );
    }
}

/// ADR 0944: `slot_run` says what the questions it is derived from say, for every primitive, slot and call
/// shape in every vocabulary, and an inline slot is never left unclassified by an owner the vocabulary has no
/// word for: that would be a slot a walk silently refuses.
#[test]
fn the_slot_run_is_derived_from_the_callback_questions() {
    let two = &Solid2 as &'static dyn Dialect;
    let mut dialects: Vec<&'static dyn Dialect> = vec![
        two,
        &Solid2::RC3,
        &Solid2::RC9,
        &Solid2::RC13,
        &Solid2::RC17,
        &Solid2::RC21,
    ];
    dialects.extend(two.variants().iter().copied().step_by(10_007));
    let mut in_place = 0;
    for dialect in dialects {
        for primitive in profiled_primitives(dialect) {
            for argument in 0..3 {
                for count in argument + 1..=4 {
                    let profile = SlotProfile::of(dialect, primitive, argument, count);
                    let run = dialect.slot_run(primitive, argument, count);
                    let at = format!("{primitive:?} argument {argument} of {count}");
                    match (profile.execution, run) {
                        (None, SlotRun::Unknown) => {}
                        (Some(Execution::Tracked), SlotRun::Computation) => {}
                        (Some(Execution::Deferred), SlotRun::Deferred) => {}
                        (Some(Execution::Inline), SlotRun::InPlace(run)) => {
                            in_place += 1;
                            assert_eq!(
                                run.observer == SlotObserver::Keeps,
                                profile.keeps_observer,
                                "{at}: the observer"
                            );
                            assert_eq!(
                                run.observer == SlotObserver::Clears,
                                profile.clears_listener && !profile.keeps_observer,
                                "{at}: the listener"
                            );
                            assert_eq!(
                                run.throws == SlotThrows::Swallows,
                                profile.swallows_throw,
                                "{at}: the thrown error"
                            );
                            assert_eq!(
                                run.owner == SlotOwner::Inherits,
                                profile.owner == Some(CallbackOwner::Inherits),
                                "{at}: the inherited owner"
                            );
                            assert_eq!(
                                run.owner == SlotOwner::Creates,
                                profile.owner == Some(CallbackOwner::Creates),
                                "{at}: the created owner"
                            );
                            // The observer the callback keeps is the caller's, so the owner is too.
                            assert!(
                                run.observer != SlotObserver::Keeps
                                    || run.owner == SlotOwner::Inherits,
                                "{at}: a callback that keeps the observer inherits the owner"
                            );
                        }
                        (execution, run) => {
                            panic!("{at}: execution {execution:?} answered {run:?}")
                        }
                    }
                }
            }
        }
    }
    assert!(
        in_place > 100,
        "{in_place} in-place slots: the enumeration lost its primitives"
    );
}
