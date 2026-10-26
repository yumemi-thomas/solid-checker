use super::*;

#[test]
fn browser_callback_triggers_keep_registration_separate_from_delivery() {
    use crate::BrowserCallbackTrigger as Trigger;
    let dialect = Solid2::default();
    assert_eq!(
        dialect.browser_callback_trigger(Primitive::Untrack, 0, 1),
        Some(Trigger::DuringCall)
    );
    assert_eq!(
        dialect.browser_callback_trigger(Primitive::CreateRoot, 0, 1),
        Some(Trigger::DuringCall)
    );
    assert_eq!(
        dialect.browser_callback_trigger(Primitive::CreateMemo, 0, 1),
        Some(Trigger::DuringCall)
    );
    assert_eq!(
        dialect.browser_callback_trigger(Primitive::CreateMemo, 0, 2),
        Some(Trigger::ComputationDemand)
    );
    assert_eq!(
        dialect.browser_callback_trigger(Primitive::CreateEffect, 0, 2),
        Some(Trigger::DuringCall)
    );
    assert_eq!(
        dialect.browser_callback_trigger(Primitive::CreateEffect, 1, 2),
        Some(Trigger::EffectFlush)
    );
    assert_eq!(
        dialect.browser_callback_trigger(Primitive::OnCleanup, 0, 1),
        Some(Trigger::OwnerDisposal)
    );
    assert_eq!(
        dialect.browser_callback_defer_options(Primitive::CreateEffect, 1),
        Some(2)
    );
    assert_eq!(
        dialect.browser_callback_defer_options(Primitive::CreateEffect, 0),
        None
    );
    for primitive in [Primitive::CreateEffect, Primitive::CreateRenderEffect] {
        assert_eq!(dialect.effect_apply_defer_options(primitive, 1), Some(2));
        assert_eq!(dialect.effect_apply_defer_options(primitive, 0), None);
    }
    for primitive in [
        Primitive::CreateTrackedEffect,
        Primitive::CreateReaction,
        Primitive::CreateMemo,
    ] {
        assert_eq!(dialect.effect_apply_defer_options(primitive, 0), None);
    }
    assert_eq!(
        dialect.browser_callback_member_trigger(Primitive::CreateEffect, 1, 2, "error"),
        Some(Trigger::ComputationError)
    );
    assert_eq!(
        dialect.browser_callback_member_trigger(Primitive::CreateEffect, 1, 2, "effect"),
        Some(Trigger::EffectFlush)
    );
    assert_eq!(
        dialect.browser_callback_trigger(Primitive::OnSettled, 0, 1),
        Some(Trigger::Settlement)
    );
    assert_eq!(
        dialect.browser_callback_trigger(Primitive::Lazy, 0, 1),
        None
    );
    assert_eq!(
        dialect.browser_callback_trigger(Primitive::CreateEffect, 2, 2),
        None
    );
    for primitive in [
        Primitive::For,
        Primitive::Repeat,
        Primitive::Show,
        Primitive::Switch,
        Primitive::Match,
        Primitive::Loading,
        Primitive::Errored,
    ] {
        assert_eq!(
            dialect.browser_children_trigger(primitive),
            Some(Trigger::RenderSelection)
        );
    }
    assert!(dialect.browser_children_dead(Primitive::Show, true, false, false));
    assert!(!dialect.browser_children_dead(Primitive::Show, false, false, false));
    assert!(!dialect.browser_children_dead(Primitive::For, true, false, false));
    assert!(dialect.browser_children_dead(Primitive::For, false, true, false));
    assert!(dialect.browser_children_dead(Primitive::Repeat, false, false, true));
}

/// The execution words this dialect states, pinned exactly.
///
/// These lived in `solid-reactive-ir`'s `interproc.rs` as a hardcoded
/// `match` until ADR 0111, where shared code decided per-version
/// behaviour and a new dialect could not state a different answer. Moving
/// them here changed no summary in the corpus; this test is what keeps
/// the move honest.
///
/// The execution word is **not** derivable from
/// [`Dialect::callback_execution_at`], which is why the two are asserted
/// apart: `createRenderEffect`'s apply is attributed `Deferred` and
/// promises no execution word at all.
#[test]
fn the_execution_words_are_stated_by_this_dialect_not_by_shared_code() {
    let two = &Solid2 as &dyn Dialect;
    let word =
        |primitive, argument, count| two.callback_execution_word_at(primitive, argument, count);

    for primitive in [
        Primitive::CreateMemo,
        Primitive::CreateTrackedEffect,
        Primitive::CreateSignal,
        Primitive::CreateStore,
        Primitive::CreateProjection,
        Primitive::CreateOptimistic,
        Primitive::CreateOptimisticStore,
        Primitive::Dynamic,
    ] {
        assert_eq!(
            word(primitive, 0, 1),
            Some(Execution::Tracked),
            "{primitive:?}"
        );
    }
    for primitive in [
        Primitive::OnSettled,
        Primitive::Action,
        Primitive::CreateReaction,
        Primitive::OnCleanup,
    ] {
        assert_eq!(
            word(primitive, 0, 1),
            Some(Execution::Deferred),
            "{primitive:?}"
        );
    }
    for primitive in [Primitive::CreateRoot, Primitive::Untrack, Primitive::Flush] {
        assert_eq!(
            word(primitive, 0, 1),
            Some(Execution::Inline),
            "{primitive:?}"
        );
    }
    // The clearing wrapper whose callback slot is not index 0.
    assert_eq!(word(Primitive::RunWithOwner, 1, 2), Some(Execution::Inline));
    assert_eq!(word(Primitive::RunWithOwner, 0, 2), None);

    // `createEffect` defers to the attribution answer for both arguments:
    // a tracked compute and a queued effect function.
    assert_eq!(
        word(Primitive::CreateEffect, 0, 2),
        Some(Execution::Tracked)
    );
    assert_eq!(
        word(Primitive::CreateEffect, 1, 2),
        Some(Execution::Deferred)
    );
    assert_eq!(
        word(Primitive::CreateRenderEffect, 0, 2),
        Some(Execution::Tracked)
    );
    // `createRenderEffect`'s apply runs before the call returns on the
    // plain path and after it under `defer`/`schedule`, an async or
    // pending compute, or (rc.9) a staged transaction, so neither `inline`
    // nor `deferred` is a true promise at any arity. The attribution word
    // stays `Deferred`; only the execution word refuses to restate it.
    for count in 2..=3 {
        assert_eq!(word(Primitive::CreateRenderEffect, 1, count), None);
    }
    assert_eq!(
        two.callback_execution_at(Primitive::CreateRenderEffect, 1, 2),
        Some(Execution::Deferred)
    );

    // `onCleanup` agrees in both vocabularies: attributed `Deferred` (a
    // read in its callback is outside the registering body's strict-read
    // window) and promised `deferred` to a consumer.
    assert_eq!(
        two.callback_execution_at(Primitive::OnCleanup, 0, 1),
        Some(Execution::Deferred)
    );
    assert_eq!(word(Primitive::OnCleanup, 0, 1), Some(Execution::Deferred));
    assert!(two.wraps_function_arguments_in_memo(Primitive::Merge));
    assert!(!two.wraps_function_arguments_in_memo(Primitive::Omit));

    // Silence is an answer a summary must keep as "unknown": the
    // execution *word* for `children` stays unstated. ADR 0323 gives it an
    // *attribution* answer, which is a different question: its callback is
    // a lazy memo's computation, so reads in it are tracked.
    assert_eq!(word(Primitive::Children, 0, 1), None);
    assert_eq!(
        two.callback_execution_at(Primitive::Children, 0, 1),
        Some(Execution::Tracked)
    );
    assert_eq!(
        two.callback_owner_at(Primitive::Children, 0, 1),
        Some(CallbackOwner::Creates)
    );
    assert!(!two.runs_callback_deferred(Primitive::Children));
    assert_eq!(two.tracked_callback_timing(Primitive::Children, 0, 1), None);
}

/// ADR 0324: the audited router release table is exact-version.
#[test]
fn only_the_audited_router_release_hands_out_url_strings() {
    let two: &dyn Dialect = &Solid2::RC9;
    assert!(two.router_params_are_url_strings("2.0.0-next.26"));
    assert!(two.router_params_are_url_strings("2.0.0-next.41"));
    for version in [
        "2.0.0-next.18",
        "2.0.0-next.27",
        "2.0.0-next.40",
        "2.0.0",
        "0.15.3",
        "2.0.0-next.26-rc",
        "",
    ] {
        assert!(!two.router_params_are_url_strings(version), "{version}");
    }
    assert_eq!(
        two.audited_router_releases(),
        &["2.0.0-next.26", "2.0.0-next.41"]
    );
    // Every audited release has exactly one digest, and no other has.
    for version in two.audited_router_releases() {
        assert!(two.audited_router_digest(version).is_some_and(|digest| {
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        }));
    }
    assert_eq!(two.audited_router_digest("2.0.0-next.18"), None);
}

/// ADR 0335: route definitions are audited by exact version, each with a
/// digest; next.26's and next.41's equal ADR 0324's (the same registry bytes).
#[test]
fn only_audited_router_releases_render_route_definitions() {
    let two: &dyn Dialect = &Solid2::RC9;
    for version in [
        "2.0.0-next.16",
        "2.0.0-next.18",
        "2.0.0-next.26",
        "2.0.0-next.41",
    ] {
        assert!(
            two.audited_route_config_digest(version).is_some(),
            "{version}"
        );
    }
    for version in ["2.0.0-next.26", "2.0.0-next.41"] {
        assert_eq!(
            two.audited_route_config_digest(version),
            two.audited_router_digest(version)
        );
    }
    for version in ["2.0.0-next.21", "2.0.0-next.17", "0.15.3", "2.0.0", ""] {
        assert_eq!(two.audited_route_config_digest(version), None, "{version}");
    }
}

/// ADR 0334: the audited route-component release table is exact-version, and
/// each release carries both digests.
#[test]
fn only_audited_tanstack_releases_render_route_components() {
    let two: &dyn Dialect = &Solid2::RC9;
    let hex =
        |digest: &str| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit());
    for version in [
        "2.0.0-beta.29",
        "2.0.0-rc.0",
        "2.0.0-rc.4",
        "2.0.0-rc.6",
        "2.0.0-rc.8",
        "2.0.0-rc.9",
    ] {
        let release = two.audited_route_components(version).expect(version);
        assert_eq!(release.version, version);
        assert!(
            hex(release.router_digest) && hex(release.core_digest),
            "{version}"
        );
    }
    for version in [
        "2.0.0-rc.5",
        "2.0.0-rc.7",
        "2.0.0",
        "1.120.0",
        "2.0.0-rc.8-x",
        "",
    ] {
        assert_eq!(two.audited_route_components(version), None, "{version}");
    }
}

/// ADR 0323 (review round 10): which callbacks run in their call's own
/// strict-read window. `untrack` clears it, and the rest are queued, tracked
/// or reporting slots.
#[test]
fn only_synchronous_owner_swappers_inherit_the_calling_window() {
    let two: &dyn Dialect = &Solid2::RC9;
    assert!(two.callback_inherits_calling_window(Primitive::CreateRoot, 0));
    assert!(two.callback_inherits_calling_window(Primitive::RunWithOwner, 1));
    assert!(two.callback_inherits_calling_window(Primitive::CreateRevealOrder, 0));
    assert!(!two.callback_inherits_calling_window(Primitive::RunWithOwner, 0));
    for primitive in [
        Primitive::Untrack,
        Primitive::Flush,
        Primitive::OnSettled,
        Primitive::Action,
        Primitive::CreateMemo,
        Primitive::CreateReaction,
        Primitive::Resolve,
        Primitive::Lazy,
    ] {
        assert!(
            !two.callback_inherits_calling_window(primitive, 0),
            "{primitive:?}"
        );
    }
}

/// The three `<For>` overloads from rc.0's `flow.d.ts`, plus the two
/// forms that must claim nothing: a dynamic boolean flag (either overload
/// may run — RFC 03 tells authors to prefer a literal or key function),
/// and — for `Show`/`Match`, whose `keyed` is boolean-only — any
/// expression form at all.
/// ADR 0401: only the map function of `mapArray` and `repeat` is untracked
/// without a window of its own, and only `runWithOwner` clears tracking in the
/// window of its call. `createRoot` shares the code read but not a probe, and
/// the list and count thunks stay tracked computations.
#[test]
fn only_map_functions_and_runwithowner_are_untracked_reads_the_runtime_leaves_silent() {
    let two = Solid2::CONSERVATIVE;
    for primitive in [Primitive::MapArray, Primitive::RepeatMap] {
        assert!(two.callback_runs_untracked_without_window(primitive, 1));
        assert!(!two.callback_runs_untracked_without_window(primitive, 0));
        assert!(two.reports_untracked_reads_at(primitive, 1, 2));
        assert!(
            !two.callback_semantics_at(primitive, 1, 2).tracks_reads,
            "{primitive:?}: the map function subscribes nothing"
        );
        assert!(!two.callback_clears_tracking_in_calling_window(primitive, 1));
    }
    assert!(two.callback_clears_tracking_in_calling_window(Primitive::RunWithOwner, 1));
    assert!(!two.callback_clears_tracking_in_calling_window(Primitive::RunWithOwner, 0));
    assert!(!two.callback_runs_untracked_without_window(Primitive::RunWithOwner, 1));
    for primitive in [
        Primitive::CreateRoot,
        Primitive::Untrack,
        Primitive::CreateMemo,
        Primitive::CreateEffect,
    ] {
        assert!(!two.callback_runs_untracked_without_window(primitive, 0));
        assert!(!two.callback_clears_tracking_in_calling_window(primitive, 0));
    }
}

#[test]
fn keyed_forms_claim_only_proven_callback_shapes() {
    use crate::KeyForm;
    assert_eq!(
        Solid2.children_accessor_parameters(Primitive::For, KeyForm::Absent),
        &[1]
    );
    assert_eq!(
        Solid2.children_accessor_parameters(Primitive::For, KeyForm::Keyed),
        &[1]
    );
    assert_eq!(
        Solid2.children_accessor_parameters(Primitive::For, KeyForm::Unkeyed),
        &[0]
    );
    assert_eq!(
        Solid2.children_accessor_parameters(Primitive::For, KeyForm::CustomKey),
        &[0, 1]
    );
    assert!(
        Solid2
            .children_accessor_parameters(Primitive::For, KeyForm::DynamicFlag)
            .is_empty()
    );
    for primitive in [Primitive::Show, Primitive::Match] {
        assert_eq!(
            Solid2.children_accessor_parameters(primitive, KeyForm::Absent),
            &[0]
        );
        for form in [KeyForm::Keyed, KeyForm::CustomKey, KeyForm::DynamicFlag] {
            assert!(
                Solid2
                    .children_accessor_parameters(primitive, form)
                    .is_empty()
            );
        }
    }
}

/// The vocabulary, pinned by name rather than by count.
///
/// This test used to assert `TABLE.len() == 30` on the grounds that the
/// table was an extraction of what `solid-reactive-ir` hardcoded, so a
/// moved count meant drift. That premise expired: the engine now reads
/// this table rather than the other way round, and the table is sourced
/// from the published package. A count cannot say which name changed, and
/// bumping one is not review. A list can and is.
///
/// Adding a name here is a behaviour change -- it starts resolving, and
/// the fixture snapshots will say what moved.
#[test]
fn the_vocabulary_is_pinned_by_name() {
    assert_eq!(
        names(),
        [
            "action",
            "affects",
            "children",
            "clientOnly",
            "createContext",
            "createDeferred",
            "createEffect",
            "createErrorBoundary",
            "createLoadingBoundary",
            "createMemo",
            "createOptimistic",
            "createOptimisticStore",
            "createOwner",
            "createProjection",
            "createReaction",
            "createRenderEffect",
            "createRevealOrder",
            "createRoot",
            "createSignal",
            "createStore",
            "createTrackedEffect",
            "deep",
            "dispatchAsInteraction",
            "dynamic",
            "dynamicComponent",
            "Errored",
            "flush",
            "For",
            "getOwner",
            "httpHeader",
            "httpStatus",
            "hydrate",
            "isPending",
            "latest",
            "lazy",
            "lazyModule",
            "Loading",
            "mapArray",
            "Match",
            "merge",
            "omit",
            "onCleanup",
            "onHydrationEnd",
            "onSettled",
            "reconcile",
            "refresh",
            "render",
            "Repeat",
            "repeat",
            "resolve",
            "runWithOwner",
            "Show",
            "snapshot",
            "Switch",
            "until",
            "untrack",
            "useContext",
            "useHead",
        ]
    );
}

/// The `dynamic` call forms, pinned against `@solidjs/web@2.0.0-rc.9`'s
/// `if (options?.static)` (`dist/web.dev.js:2199`): only a proven literal
/// picks a form, a proven falsy or absent option keeps the default, and an
/// unproven one reaches the form that states nothing.
#[test]
fn dynamic_call_forms_follow_the_static_option_literal() {
    use crate::OptionLiteral;
    let form = |vocabulary: Solid2, literal: OptionLiteral| {
        vocabulary.call_form(Primitive::Dynamic, &|argument, key| {
            assert_eq!((argument, key), (1, "static"));
            literal
        })
    };
    let rc9 = Solid2::RC9;
    assert_eq!(form(rc9, OptionLiteral::True), Primitive::DynamicStatic);
    assert_eq!(form(rc9, OptionLiteral::False), Primitive::Dynamic);
    assert_eq!(form(rc9, OptionLiteral::Absent), Primitive::Dynamic);
    assert_eq!(
        form(rc9, OptionLiteral::Unknown),
        Primitive::DynamicUnknownForm
    );
    // rc.0-rc.8's web never reads the option: every call is the default,
    // which is what those runtimes build.
    for literal in [
        OptionLiteral::True,
        OptionLiteral::False,
        OptionLiteral::Absent,
        OptionLiteral::Unknown,
    ] {
        assert_eq!(form(Solid2, literal), Primitive::Dynamic);
    }
    // A web release nobody read: an option-bearing call states nothing.
    let unread = Solid2 {
        dynamic_options: releases::DynamicOptions::Unread,
        ..Solid2
    };
    assert_eq!(
        form(unread, OptionLiteral::True),
        Primitive::DynamicUnknownForm
    );
    assert_eq!(
        form(unread, OptionLiteral::Unknown),
        Primitive::DynamicUnknownForm
    );
    assert_eq!(form(unread, OptionLiteral::False), Primitive::Dynamic);
    assert_eq!(form(unread, OptionLiteral::Absent), Primitive::Dynamic);
    // Nothing else has a form, and nothing else is asked about options.
    assert_eq!(
        Solid2.call_form(Primitive::Untrack, &|_, _| panic!("not asked")),
        Primitive::Untrack
    );
    // A form spells as the export it is a call of, and is never a name.
    for primitive in [Primitive::DynamicStatic, Primitive::DynamicUnknownForm] {
        assert_eq!(Solid2.name_of(primitive), Some("dynamic"));
    }
    assert_eq!(Solid2.primitive("dynamic"), Some(Primitive::Dynamic));
}

/// The static form answers exactly as `untrack` does, because it *is*
/// `untrack(source)` at the call; the default keeps its rc.3 rows; the
/// unknown form states nothing a consumer could rely on.
#[test]
fn dynamic_forms_answer_their_own_runtime() {
    let two: &dyn Dialect = &Solid2::RC9;
    for primitive in [Primitive::DynamicStatic, Primitive::Untrack] {
        assert_eq!(
            two.callback_execution_at(primitive, 0, 2),
            Some(Execution::Inline)
        );
        assert!(two.runs_callback_deferred(primitive));
        assert!(two.runs_callback_synchronously(primitive));
        assert!(two.callback_preserves_owner_write_context(primitive));
        assert_eq!(
            two.callback_owner_at(primitive, 0, 2),
            Some(CallbackOwner::Inherits)
        );
        assert_eq!(
            two.callback_execution_word_at(primitive, 0, 2),
            Some(Execution::Inline)
        );
    }
    assert_eq!(
        two.callback_execution_at(Primitive::Dynamic, 0, 1),
        Some(Execution::Tracked)
    );
    assert_eq!(
        two.callback_owner_at(Primitive::Dynamic, 0, 1),
        Some(CallbackOwner::Creates)
    );
    assert_eq!(
        two.callback_execution_word_at(Primitive::Dynamic, 0, 1),
        Some(Execution::Tracked)
    );
    let unknown = Primitive::DynamicUnknownForm;
    assert_eq!(two.callback_execution_at(unknown, 0, 2), None);
    assert_eq!(two.callback_owner_at(unknown, 0, 2), None);
    assert_eq!(two.callback_execution_word_at(unknown, 0, 2), None);
    assert!(!two.runs_callback_deferred(unknown));
}

/// rc.9's `until` is `resolve`'s shape (`dist/dev.js:2717-2785`), so it
/// answers `resolve`'s rows, execution-word silence included -- on an
/// installation that exports it. On the audited rc.3 triple the name is
/// not an export at all (TS2305), so the vocabulary does not know it.
#[test]
fn until_answers_as_resolve_does() {
    let audited: &dyn Dialect = &Solid2;
    assert_eq!(audited.primitive("until"), None);
    assert_eq!(audited.name_of(Primitive::Until), None);
    assert!(
        audited
            .export_modules("until", crate::ExportPosition::Value)
            .is_empty()
    );
    let two: &dyn Dialect = &Solid2::RC9;
    assert_eq!(two.primitive("until"), Some(Primitive::Until));
    assert_eq!(two.name_of(Primitive::Until), Some("until"));
    for primitive in [Primitive::Until, Primitive::Resolve] {
        assert_eq!(two.callback_positions(primitive), &[0]);
        assert_eq!(
            two.callback_execution_at(primitive, 0, 2),
            Some(Execution::Deferred)
        );
        assert!(two.runs_callback_deferred(primitive));
        assert_eq!(
            two.callback_owner_at(primitive, 0, 2),
            Some(CallbackOwner::Creates)
        );
        assert_eq!(two.callback_execution_word_at(primitive, 0, 2), None);
    }
    assert_eq!(
        two.export_modules("until", crate::ExportPosition::Value),
        vec!["solid-js"]
    );
    assert!(
        two.namespace_import_primitives("solid-js")
            .contains(&"until")
    );
}

/// Only a two-argument `omit` can carry rc.9's predicate
/// (`keys.length === 1 && typeof keys[0] === "function"`,
/// `dist/dev.js:4380`), and only at argument 1. rc.3 has no predicate
/// form at all.
#[test]
fn only_the_two_argument_omit_slot_runs_on_result_access() {
    let audited: &dyn Dialect = &Solid2;
    for argument_count in 0..4 {
        for argument in 0..argument_count {
            assert!(!audited.callback_runs_on_result_access(
                Primitive::Omit,
                argument,
                argument_count
            ));
        }
    }
    assert!(!Solid2::RC3.callback_runs_on_result_access(Primitive::Omit, 1, 2));
    assert!(!Solid2::default().callback_runs_on_result_access(Primitive::Omit, 1, 2));
    let two: &dyn Dialect = &Solid2::RC9;
    assert!(two.callback_runs_on_result_access(Primitive::Omit, 1, 2));
    assert!(!two.callback_runs_on_result_access(Primitive::Omit, 0, 2));
    assert!(!two.callback_runs_on_result_access(Primitive::Omit, 1, 3));
    assert!(!two.callback_runs_on_result_access(Primitive::Omit, 2, 3));
    assert!(!two.callback_runs_on_result_access(Primitive::Merge, 1, 2));
    // The split row itself is unchanged: the exception is the slot's.
    assert!(two.splits_props(Primitive::Omit));
    assert_eq!(two.callback_execution_at(Primitive::Omit, 1, 2), None);
}

#[test]
fn the_table_is_sorted_and_free_of_duplicates() {
    let mut sorted = TABLE.to_vec();
    sorted.sort_by_key(|(name, _)| name.to_lowercase());
    let actual: Vec<_> = TABLE.iter().map(|(name, _)| name.to_lowercase()).collect();
    let expected: Vec<_> = sorted.iter().map(|(name, _)| name.to_lowercase()).collect();
    assert_eq!(actual, expected, "keep the table sorted for review");

    let mut seen = std::collections::HashSet::new();
    for (name, primitive) in TABLE {
        assert!(seen.insert(*name), "{name} is listed twice");
        assert_eq!(
            reverse(TABLE, *primitive),
            Some(*name),
            "{name} maps to a primitive another name already claims"
        );
    }
}

/// The namespace invariant, which the retired `solid_1x.rs` also held: a
/// namespace import must
/// retain every modelled runtime obligation the module exports, so a
/// primitive cannot be reachable as `import { x }` but invisible as
/// `Solid.x`.
#[test]
fn every_modelled_export_resolves_through_its_namespace_module() {
    // The namespace lists are static and name every release's exports; a
    // name an installation lacks is then simply not a primitive there.
    for vocabulary in [Solid2, Solid2::RC9] {
        for module in vocabulary.modules() {
            let mut expected = TABLE
                .iter()
                .filter_map(|(name, _)| {
                    vocabulary
                        .export_modules(name, crate::ExportPosition::Value)
                        .contains(module)
                        .then_some(*name)
                })
                .collect::<Vec<_>>();
            expected.sort_unstable();
            let mut actual = vocabulary
                .namespace_import_primitives(module)
                .iter()
                .copied()
                .filter(|name| {
                    !RELEASE_GATED_NAMES.contains(name) || vocabulary.primitive(name).is_some()
                })
                .collect::<Vec<_>>();
            actual.sort_unstable();
            assert_eq!(
                actual, expected,
                "namespace imports from {module} must retain every modelled runtime obligation"
            );
        }
    }
}

#[test]
fn named_and_namespace_imports_can_resolve_run_with_owner() {
    assert!(
        Solid2
            .namespace_import_primitives("solid-js")
            .contains(&"runWithOwner")
    );
}

#[test]
fn rc_0_web_primitives_have_distinct_callback_roles() {
    assert_eq!(Solid2.primitive("clientOnly"), Some(Primitive::ClientOnly));
    assert_eq!(Solid2.primitive("useHead"), Some(Primitive::UseHead));
    assert_eq!(
        Solid2.callback_executions(Primitive::ClientOnly),
        &[(0, Execution::Deferred)]
    );
    assert_eq!(
        Solid2.callback_executions(Primitive::UseHead),
        &[(0, Execution::Tracked)]
    );
    assert!(Solid2.runs_callback_deferred(Primitive::ClientOnly));
    assert!(!Solid2.runs_callback_deferred(Primitive::UseHead));
    assert_eq!(
        Solid2.export_modules("clientOnly", crate::ExportPosition::Value),
        vec!["@solidjs/web"]
    );
    assert_eq!(
        Solid2.export_modules("useHead", crate::ExportPosition::Value),
        vec!["@solidjs/web"]
    );
    assert!(
        Solid2
            .namespace_import_primitives("@solidjs/web")
            .contains(&"clientOnly")
    );
    assert!(
        Solid2
            .namespace_import_primitives("@solidjs/web")
            .contains(&"useHead")
    );
}

#[test]
fn only_callback_bearing_primitives_report_positions() {
    // A tag is not a call; asking for its callback positions is a caller
    // error that should answer empty rather than guess index 0.
    assert!(Solid2.callback_positions(Primitive::For).is_empty());
    assert!(Solid2.callback_positions(Primitive::Loading).is_empty());
    // `children(fn)` takes its thunk at 0 (ADR 0323).
    assert_eq!(Solid2.callback_positions(Primitive::Children), &[0]);
}

/// `dynamicComponent` is `dynamicCore` with no tag arm (`solid-js@2.0.0-rc.17`
/// `dist/solid.dev.js:1545`): `dynamic`'s forms, decided from the call alone,
/// whatever web's `dynamic` answers; a third argument (the tag arm) states
/// nothing.
#[test]
fn dynamic_component_calls_take_dynamics_forms() {
    use crate::OptionLiteral;
    let form = |vocabulary: Solid2, third: OptionLiteral, literal: OptionLiteral| {
        vocabulary.call_form(
            Primitive::DynamicComponent,
            &|argument, key| match argument {
                1 => {
                    assert_eq!(key, "static");
                    literal
                }
                2 => third,
                _ => panic!("argument {argument} is not asked"),
            },
        )
    };
    // Web's own `dynamic` answer does not decide `dynamicComponent`'s.
    for vocabulary in [
        Solid2::RC17,
        Solid2 {
            dynamic_options: releases::DynamicOptions::Unread,
            ..Solid2::RC17
        },
    ] {
        let absent = OptionLiteral::Absent;
        assert_eq!(form(vocabulary, absent, absent), Primitive::Dynamic);
        assert_eq!(
            form(vocabulary, absent, OptionLiteral::False),
            Primitive::Dynamic
        );
        assert_eq!(
            form(vocabulary, absent, OptionLiteral::True),
            Primitive::DynamicStatic
        );
        assert_eq!(
            form(vocabulary, absent, OptionLiteral::Unknown),
            Primitive::DynamicUnknownForm
        );
        for third in [
            OptionLiteral::Unknown,
            OptionLiteral::True,
            OptionLiteral::False,
        ] {
            assert_eq!(
                form(vocabulary, third, absent),
                Primitive::DynamicUnknownForm
            );
        }
    }
    assert_eq!(
        Solid2::RC17.name_of(Primitive::DynamicComponent),
        Some("dynamicComponent")
    );
    assert_eq!(Solid2::RC13.name_of(Primitive::DynamicComponent), None);
    // A name only: no row answers for it before its call form is taken.
    let two: &dyn Dialect = &Solid2::RC17;
    assert!(
        two.callback_executions(Primitive::DynamicComponent)
            .is_empty()
    );
    assert!(two.callback_owners(Primitive::DynamicComponent).is_empty());
}

/// `dispatchAsInteraction(event, fn)` runs `fn()` inline, once, under the
/// caller's owner and listener (`@solidjs/web@2.0.0-rc.17`
/// `dist/web.js:484`, `dist/web.dev.js:653-660`): `flush(fn)`'s attribution
/// without its scheduler.
#[test]
fn dispatch_as_interaction_runs_its_callback_in_place() {
    let two: &dyn Dialect = &Solid2::RC17;
    let primitive = Primitive::DispatchAsInteraction;
    assert_eq!(two.callback_positions(primitive), &[1]);
    assert_eq!(
        two.callback_execution_at(primitive, 1, 2),
        Some(Execution::Inline)
    );
    assert_eq!(two.callback_execution_at(primitive, 0, 2), None);
    assert_eq!(
        two.callback_execution_word_at(primitive, 1, 2),
        Some(Execution::Inline)
    );
    assert_eq!(
        two.callback_owner_at(primitive, 1, 2),
        Some(CallbackOwner::Inherits)
    );
    assert!(!two.runs_callback_deferred(primitive));
    assert!(two.callback_preserves_owner_write_context(primitive));
    assert!(!two.callback_inherits_calling_window(primitive, 1));
    assert!(!two.reports_untracked_reads_at(primitive, 1, 2));
    assert!(!two.throws_inside_action_step(primitive));
    assert!(!two.warns_inside_effect_phase(primitive));
    assert_eq!(two.owner_requirement_role(primitive), None);
}

/// rc.17's `createDeferred(fn, options?)` is `accessor(computed(wrapper,
/// options))` whose wrapper calls `fn(prev)` (`@solidjs/signals@2.0.0-rc.17`
/// `dist/dev.js:8897-8909`), and the server's is `createMemo`
/// (`solid-js` `dist/server.js:2035`): every answer about `fn` and the
/// accessor is `createMemo`'s. What differs (refetches after the first answer
/// do not hold readers) is no answer this vocabulary gives.
#[test]
fn create_deferred_answers_as_create_memo() {
    let two: &dyn Dialect = &Solid2::RC17;
    let (deferred, memo) = (Primitive::CreateDeferred, Primitive::CreateMemo);
    assert_eq!(
        two.callback_positions(deferred),
        two.callback_positions(memo)
    );
    assert_eq!(two.callback_owners(deferred), two.callback_owners(memo));
    assert_eq!(
        two.callback_executions(deferred),
        two.callback_executions(memo)
    );
    assert_eq!(
        two.runs_callback_deferred(deferred),
        two.runs_callback_deferred(memo)
    );
    assert_eq!(two.options_argument(deferred), two.options_argument(memo));
    assert_eq!(two.cleanup_rule(deferred), two.cleanup_rule(memo));
    assert!(two.creates_reactive_source(deferred));
    assert!(two.creates_directive_owner(deferred));
    for count in [1, 2] {
        assert_eq!(
            two.callback_execution_word_at(deferred, 0, count),
            two.callback_execution_word_at(memo, 0, count)
        );
        assert_eq!(
            two.tracked_callback_timing(deferred, 0, count),
            two.tracked_callback_timing(memo, 0, count)
        );
        assert_eq!(
            two.eager_owned_computation_slot(deferred, 0, count),
            two.eager_owned_computation_slot(memo, 0, count)
        );
        assert_eq!(
            two.browser_callback_trigger(deferred, 0, count),
            two.browser_callback_trigger(memo, 0, count)
        );
    }
    assert_eq!(
        two.reactive_result_slot(deferred, ResultSlot::Whole),
        Some(ReactiveRole::Accessor)
    );
    assert_eq!(
        two.accessor_getter_length(deferred, ResultSlot::Whole),
        Some(0)
    );
    // Not measured for `createDeferred`, so not stated.
    assert!(!two.supports_sync_option(deferred));
    assert!(!two.accessor_yields_only_its_compute(deferred));
}

/// rc.17's `lazyModule(fn)`: `lazy`'s loader without the component. `fn` runs
/// only from the returned `load`, on its caller's stack (`solid-js@2.0.0-rc.17`
/// `dist/solid.dev.js:1408-1431`), so it is never during the call, and its
/// reads are not a reporting slot: they meet the invoker's window.
#[test]
fn lazy_module_runs_its_loader_only_from_the_result() {
    let two: &dyn Dialect = &Solid2::RC17;
    let primitive = Primitive::LazyModule;
    assert_eq!(two.callback_positions(primitive), &[0]);
    assert_eq!(
        two.callback_execution_at(primitive, 0, 1),
        Some(Execution::Deferred)
    );
    assert_eq!(two.callback_execution_word_at(primitive, 0, 1), None);
    assert_eq!(
        two.callback_owner_at(primitive, 0, 1),
        Some(CallbackOwner::Inherits)
    );
    assert!(two.callback_requires_return_invocation(primitive, 0));
    assert!(two.runs_callback_deferred(primitive));
    assert!(!two.reports_untracked_reads_at(primitive, 0, 1));
    assert!(!two.callback_inherits_calling_window(primitive, 0));
    assert_eq!(two.renders_loaded_default_argument(primitive, 1), None);
    assert!(!two.creates_reactive_source(primitive));
}

/// rc.17's `onHydrationEnd(callback)`: a microtask, or the hydration drain on
/// `hydrate`'s caller's stack (`solid-js@2.0.0-rc.17`
/// `dist/solid.dev.js:385-404`; `server.js:2731-2733`), so never during the
/// call and with an owner only when the drain has one.
#[test]
fn on_hydration_end_runs_its_callback_later_with_a_conditional_owner() {
    let two: &dyn Dialect = &Solid2::RC17;
    let primitive = Primitive::OnHydrationEnd;
    assert_eq!(two.callback_positions(primitive), &[0]);
    assert_eq!(
        two.callback_execution_at(primitive, 0, 1),
        Some(Execution::Deferred)
    );
    assert_eq!(
        two.callback_execution_word_at(primitive, 0, 1),
        Some(Execution::Deferred)
    );
    assert_eq!(
        two.callback_owner_at(primitive, 0, 1),
        Some(CallbackOwner::Conditional)
    );
    assert!(two.runs_callback_deferred(primitive));
    assert!(!two.callback_requires_return_invocation(primitive, 0));
    assert!(!two.reports_untracked_reads_at(primitive, 0, 1));
    assert!(!two.callback_preserves_owner_write_context(primitive));
}
