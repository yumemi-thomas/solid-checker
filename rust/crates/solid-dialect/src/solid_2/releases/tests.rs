use super::*;

fn triple<'a>(
    solid_js: Option<&'a str>,
    signals: Option<&'a str>,
    web: Option<&'a str>,
) -> [InstalledRelease<'a>; 3] {
    [
        InstalledRelease {
            package: SOLID_JS,
            version: solid_js,
        },
        InstalledRelease {
            package: SIGNALS,
            version: signals,
        },
        InstalledRelease {
            package: WEB,
            version: web,
        },
    ]
}

fn same(release: &str) -> [InstalledRelease<'_>; 3] {
    triple(Some(release), Some(release), Some(release))
}

/// The vocabulary and gaps an installation is analyzed with.
fn analyzed(installed: &[InstalledRelease<'_>]) -> (Solid2, Vec<InstallationGap>) {
    match review(installed) {
        InstallationReview::Analyzed {
            vocabulary: Some(vocabulary),
            gaps,
        } => {
            let variant = VARIANTS
                .iter()
                .find(|candidate| {
                    std::ptr::addr_eq(
                        *candidate as *const Solid2,
                        vocabulary as *const dyn Dialect,
                    )
                })
                .copied()
                .expect("the vocabulary is one of the interned variants");
            (variant, gaps)
        }
        other => panic!("{installed:?} is analyzed: {other:?}"),
    }
}

/// The classification table, one same-release triple per row.
#[test]
fn every_reviewed_triple_answers_as_the_reviews_measured() {
    // (release, store root, omit predicate, until, dynamic options, store setter under a
    // root, flush in action, gap count)
    let rows = [
        (
            "2.0.0-rc.0",
            StoreRootTyping::Readonly,
            false,
            false,
            DynamicOptions::Ignored,
            StoreSetterRootGuard::Exempt,
            false,
            1,
        ),
        (
            "2.0.0-rc.1",
            StoreRootTyping::Readonly,
            false,
            false,
            DynamicOptions::Ignored,
            StoreSetterRootGuard::Exempt,
            false,
            1,
        ),
        (
            "2.0.0-rc.2",
            StoreRootTyping::Readonly,
            false,
            false,
            DynamicOptions::Ignored,
            StoreSetterRootGuard::Exempt,
            false,
            1,
        ),
        (
            "2.0.0-rc.3",
            StoreRootTyping::Readonly,
            false,
            false,
            DynamicOptions::Ignored,
            StoreSetterRootGuard::Exempt,
            false,
            1,
        ),
        (
            "2.0.0-rc.4",
            StoreRootTyping::Readonly,
            false,
            false,
            DynamicOptions::Ignored,
            StoreSetterRootGuard::Exempt,
            false,
            3,
        ),
        (
            "2.0.0-rc.5",
            StoreRootTyping::Readonly,
            false,
            true,
            DynamicOptions::Ignored,
            StoreSetterRootGuard::Exempt,
            false,
            3,
        ),
        (
            "2.0.0-rc.6",
            StoreRootTyping::Readonly,
            false,
            true,
            DynamicOptions::Ignored,
            StoreSetterRootGuard::Exempt,
            false,
            3,
        ),
        (
            "2.0.0-rc.7",
            StoreRootTyping::Mutable,
            false,
            true,
            DynamicOptions::Ignored,
            StoreSetterRootGuard::Exempt,
            false,
            2,
        ),
        (
            "2.0.0-rc.8",
            StoreRootTyping::Mutable,
            false,
            true,
            DynamicOptions::Ignored,
            StoreSetterRootGuard::Exempt,
            true,
            2,
        ),
        // rc.9's re-export gap, and the notice that it is older than the
        // audited rc.13.
        (
            "2.0.0-rc.9",
            StoreRootTyping::Mutable,
            true,
            true,
            DynamicOptions::StaticForm,
            StoreSetterRootGuard::Guarded,
            true,
            2,
        ),
        // rc.9's answers (the rc.13 review § 0), older than the audited
        // release.
        (
            "2.0.0-rc.13",
            StoreRootTyping::Mutable,
            true,
            true,
            DynamicOptions::StaticForm,
            StoreSetterRootGuard::Guarded,
            true,
            1,
        ),
        // The audited release: these answers are rc.13's (the rc.17
        // review); its two gaps are scoped to the new callback exports.
        (
            "2.0.0-rc.17",
            StoreRootTyping::Mutable,
            true,
            true,
            DynamicOptions::StaticForm,
            StoreSetterRootGuard::Guarded,
            true,
            2,
        ),
    ];
    for (
        release,
        store_root,
        omit_predicate_form,
        until,
        dynamic_options,
        store_setter_roots,
        flush_in_action,
        gap_count,
    ) in rows
    {
        let (vocabulary, gaps) = analyzed(&same(release));
        assert_eq!(
            (
                vocabulary.store_root,
                vocabulary.omit_predicate_form,
                vocabulary.until,
                vocabulary.dynamic_options,
                vocabulary.store_setter_roots,
                vocabulary.flush_in_action
            ),
            (
                store_root,
                omit_predicate_form,
                until,
                dynamic_options,
                store_setter_roots,
                flush_in_action
            ),
            "{release}"
        );
        assert_eq!(gaps.len(), gap_count, "{release}: {gaps:?}");
    }
    assert_eq!(analyzed(&same("2.0.0-rc.3")).0.index(), Solid2::RC3.index());
    assert_eq!(analyzed(&same("2.0.0-rc.9")).0.index(), Solid2::RC9.index());
    assert_eq!(
        analyzed(&same("2.0.0-rc.13")).0.index(),
        Solid2::RC13.index()
    );
    assert_eq!(
        analyzed(&same("2.0.0-rc.17")).0.index(),
        Solid2::AUDITED.index()
    );
    // rc.10 and rc.11 were published and never read: the conservative
    // answers, under the notice.
    for unread in ["2.0.0-rc.10", "2.0.0-rc.11", "2.0.0-rc.12", "2.0.0-rc.14"] {
        let vocabulary = analyzed(&same(unread)).0;
        assert_eq!(
            (
                vocabulary.store_root,
                vocabulary.omit_predicate_form,
                vocabulary.until,
                vocabulary.dynamic_options,
                vocabulary.store_setter_roots,
                vocabulary.flush_in_action
            ),
            (
                StoreRootTyping::Readonly,
                false,
                false,
                DynamicOptions::Unread,
                StoreSetterRootGuard::Exempt,
                false
            ),
            "{unread}"
        );
    }
    // rc.4-rc.6 name all five patch-channel exports between them.
    let names = analyzed(&same("2.0.0-rc.5"))
        .1
        .iter()
        .map(|gap| gap.gap.clone())
        .collect::<Vec<_>>()
        .join(" ");
    for export in [
        "registerPatch",
        "registerRowOps",
        "registerSlotPatch",
        "installListDriver",
        "driveList",
    ] {
        assert!(names.contains(export), "{export}: {names}");
    }
}

/// New strict-key witnesses follow signals, including mixed triples.
#[test]
fn strict_store_key_witness_follows_exact_signals_release() {
    for release in ["2.0.0-rc.3", "2.0.0-rc.9", "2.0.0-rc.14"] {
        assert!(
            !analyzed(&same(release))
                .0
                .store_key_warns_strict_read("value")
        );
    }
    let (audited, _) = analyzed(&same("2.0.0-rc.13"));
    assert!(audited.store_key_warns_strict_read("value"));
    assert!(!audited.store_key_warns_strict_read("then"));
    assert!(!Solid2::CONSERVATIVE.store_key_warns_strict_read("value"));
    // ADR 0293: the refusal half holds in every vocabulary.
    for vocabulary in [Solid2::CONSERVATIVE, Solid2::RC3, Solid2::RC9, audited] {
        assert!(vocabulary.store_key_may_skip_strict_read("then"));
        assert!(!vocabulary.store_key_may_skip_strict_read("value"));
    }
    assert!(
        vocabulary_for(Release::Read(9), Release::Read(13), Release::Read(9))
            .store_key_warns_strict_read("value")
    );
    assert!(
        !vocabulary_for(Release::Read(13), Release::Read(9), Release::Read(13))
            .store_key_warns_strict_read("value")
    );
}

/// Defect 1 of the rc.1-rc.8 review § 5: a fresh install of the audited
/// `solid-js` resolves signals rc.9, whose store root is mutable.
#[test]
fn the_store_typing_follows_the_resolved_signals() {
    let fresh = triple(Some("2.0.0-rc.3"), Some("2.0.0-rc.9"), Some("2.0.0-rc.3"));
    let (vocabulary, gaps) = analyzed(&fresh);
    assert_eq!(vocabulary.store_root, StoreRootTyping::Mutable);
    // `omit` is signals' too, so its predicate form comes with rc.9, and
    // so is the store setter's guard, which no longer exempts roots.
    assert!(vocabulary.omit_predicate_form);
    assert_eq!(vocabulary.store_setter_roots, StoreSetterRootGuard::Guarded);
    // `flush` is signals' as well, so its action-step throw comes along.
    assert!(vocabulary.flush_in_action);
    // rc.3's root does not re-export until (TS2305 on that tree), and
    // rc.3's web has no static option.
    assert!(!vocabulary.until);
    assert_eq!(vocabulary.dynamic_options, DynamicOptions::Ignored);
    assert!(
        gaps.iter()
            .any(|gap| gap.gap.contains("not one reviewed release")),
        "{gaps:?}"
    );
    // And the other way: rc.9's solid-js over an older signals.
    let (vocabulary, _) = analyzed(&triple(
        Some("2.0.0-rc.9"),
        Some("2.0.0-rc.6"),
        Some("2.0.0-rc.9"),
    ));
    assert_eq!(vocabulary.store_root, StoreRootTyping::Readonly);
    assert!(vocabulary.until);
    assert_eq!(vocabulary.dynamic_options, DynamicOptions::StaticForm);
    assert_eq!(vocabulary.store_setter_roots, StoreSetterRootGuard::Exempt);
    assert!(!vocabulary.flush_in_action);
}

/// N4: the `FLUSH_IN_ACTION` throw is signals' alone, from rc.8, and is
/// never claimed on a signals release nobody read.
#[test]
fn the_flush_throw_follows_the_resolved_signals() {
    for (signals, throws) in [
        (Some("2.0.0-rc.3"), false),
        (Some("2.0.0-rc.7"), false),
        (Some("2.0.0-rc.8"), true),
        (Some("2.0.0-rc.9"), true),
        (Some("2.0.0-rc.10"), false),
        (Some("2.0.0-beta.2"), false),
        (None, false),
    ] {
        let (vocabulary, _) = analyzed(&triple(Some("2.0.0-rc.3"), signals, Some("2.0.0-rc.3")));
        assert_eq!(vocabulary.flush_in_action, throws, "{signals:?}");
        assert_eq!(
            vocabulary.throws_inside_action_step(Primitive::Flush),
            throws,
            "{signals:?}"
        );
        // The stepping is the same on every release; only the throw moves.
        assert!(vocabulary.callback_runs_as_action_steps(Primitive::Action, 0));
        assert!(!vocabulary.callback_runs_as_action_steps(Primitive::Action, 1));
        assert!(!vocabulary.throws_inside_action_step(Primitive::Untrack));
    }
}

/// ADR 0341: `flush` from an effect's apply reports `FLUSH_IN_EFFECT_CALLBACK`
/// on every signals release some review read from rc.1, and on none before it,
/// none unread and none unresolved. The effect phase is `createEffect`'s apply
/// alone: `createRenderEffect`'s first run is not queued.
#[test]
fn the_effect_phase_report_follows_the_resolved_signals() {
    for (signals, reports) in [
        (Some("2.0.0-rc.0"), false),
        (Some("2.0.0-rc.1"), true),
        (Some("2.0.0-rc.3"), true),
        (Some("2.0.0-rc.8"), true),
        (Some("2.0.0-rc.9"), true),
        (Some("2.0.0-rc.13"), true),
        (Some("2.0.0-rc.10"), false),
        (Some("2.0.0-rc.15"), false),
        (Some("2.0.0-beta.2"), false),
        (None, false),
    ] {
        let (vocabulary, _) = analyzed(&triple(Some("2.0.0-rc.3"), signals, Some("2.0.0-rc.3")));
        assert_eq!(vocabulary.flush_in_effect_callback, reports, "{signals:?}");
        assert_eq!(
            vocabulary.warns_inside_effect_phase(Primitive::Flush),
            reports,
            "{signals:?}"
        );
        assert!(!vocabulary.warns_inside_effect_phase(Primitive::Untrack));
        assert!(vocabulary.callback_runs_in_effect_phase(Primitive::CreateEffect, 1));
        assert!(!vocabulary.callback_runs_in_effect_phase(Primitive::CreateEffect, 0));
        assert!(!vocabulary.callback_runs_in_effect_phase(Primitive::CreateRenderEffect, 1));
        assert!(!vocabulary.callback_runs_in_effect_phase(Primitive::CreateMemo, 1));
    }
}

/// ADR 0136: `createComponent` renders its first argument on every
/// release the reviews read (rc.0-rc.9 all call `Comp` in place), so the
/// answer is the same in every vocabulary, and no other name renders.
#[test]
fn create_component_renders_its_first_argument_in_every_vocabulary() {
    for vocabulary in std::iter::once(&Solid2::CONSERVATIVE).chain(VARIANTS.iter()) {
        assert_eq!(
            vocabulary.renders_component_argument("createComponent"),
            Some(0)
        );
        for other in ["createMemo", "Dynamic", "lazy", "untrack", "createRoot"] {
            assert_eq!(
                vocabulary.renders_component_argument(other),
                None,
                "{other}"
            );
        }
    }
}

/// ADR 0292: only rc.13's `solid-js` renders `lazy`'s loaded `default`
/// through `createComponent`, and only for the one-argument form, so only
/// that answer names a loader slot; mixed triples follow `solid-js`.
#[test]
fn lazy_renders_its_loaded_default_only_on_solid_js_rc13() {
    let (audited, _) = analyzed(&same("2.0.0-rc.13"));
    assert_eq!(
        audited.renders_loaded_default_argument(Primitive::Lazy, 1),
        Some(0)
    );
    assert_eq!(
        audited.renders_loaded_default_argument(Primitive::Lazy, 2),
        None
    );
    assert_eq!(
        audited.renders_loaded_default_argument(Primitive::CreateMemo, 1),
        None
    );
    for release in ["2.0.0-rc.3", "2.0.0-rc.9", "2.0.0-rc.14"] {
        assert_eq!(
            analyzed(&same(release))
                .0
                .renders_loaded_default_argument(Primitive::Lazy, 1),
            None,
            "{release}"
        );
    }
    assert_eq!(
        Solid2::CONSERVATIVE.renders_loaded_default_argument(Primitive::Lazy, 1),
        None
    );
    assert!(
        vocabulary_for(Release::Read(13), Release::Read(9), Release::Read(9))
            .lazy_renders_component
    );
    assert!(
        !vocabulary_for(Release::Read(9), Release::Read(13), Release::Read(13))
            .lazy_renders_component
    );
}

/// ADR 0353: `render` and `hydrate` run `code` under the root they create on
/// `@solidjs/web` rc.13 alone. Only the web owner decides it, whatever
/// `solid-js` and `@solidjs/signals` resolve to.
#[test]
fn render_and_hydrate_create_a_root_only_on_web_rc13() {
    for release in ["2.0.0-rc.3", "2.0.0-rc.9", "2.0.0-rc.14"] {
        assert!(!analyzed(&same(release)).0.render_creates_root, "{release}");
    }
    assert!(analyzed(&same("2.0.0-rc.13")).0.render_creates_root);
    for primitive in [Primitive::Render, Primitive::Hydrate] {
        assert!(
            vocabulary_for(Release::Read(13), Release::Read(13), Release::Read(13))
                .callback_runs_in_created_root(primitive, 0),
            "{primitive:?}"
        );
        assert!(
            !vocabulary_for(Release::Read(13), Release::Read(13), Release::Read(9))
                .callback_runs_in_created_root(primitive, 0),
            "{primitive:?} with rc.9 web"
        );
        assert!(
            vocabulary_for(Release::Read(9), Release::Read(9), Release::Read(13))
                .callback_runs_in_created_root(primitive, 0),
            "{primitive:?} with rc.13 web and rc.9 solid-js"
        );
    }
    // Off rc.13 the callback's root is unaudited, so its writes are unplaced.
    for primitive in [Primitive::Render, Primitive::Hydrate] {
        assert!(Solid2::CONSERVATIVE.callback_root_is_unaudited(primitive, 0));
        assert!(Solid2::RC9.callback_root_is_unaudited(primitive, 0));
        assert!(!Solid2::RC13.callback_root_is_unaudited(primitive, 0));
        assert!(!Solid2::RC13.callback_root_is_unaudited(primitive, 1));
    }
    assert!(!Solid2::RC9.callback_root_is_unaudited(Primitive::CreateRoot, 0));
}

/// ADR 0138: `Dynamic` renders `component` on every release the reviews
/// read, and only a one-argument `createMemo` is a holder a rendered value
/// may be followed through. `dynamic`'s returned component, the store and
/// signal constructors, and the optimistic memos are not.
#[test]
fn dynamic_renders_its_component_prop_in_every_vocabulary() {
    for vocabulary in std::iter::once(&Solid2::CONSERVATIVE).chain(VARIANTS.iter()) {
        assert_eq!(
            vocabulary.component_prop_renderers(),
            &[("Dynamic", "component")]
        );
        // ADR 0313: and a string `component` renders that intrinsic element.
        assert_eq!(
            vocabulary.intrinsic_tag_renderers(),
            &[("Dynamic", "component")]
        );
        assert!(vocabulary.accessor_yields_only_its_compute(Primitive::CreateMemo));
        for other in [
            Primitive::Dynamic,
            Primitive::CreateSignal,
            Primitive::CreateStore,
            Primitive::CreateOptimistic,
            Primitive::CreateProjection,
        ] {
            assert!(
                !vocabulary.accessor_yields_only_its_compute(other),
                "{other:?}"
            );
        }
    }
}

/// N5: only rc.0's optimistic-store setter meets no owned-scope guard, and
/// the answer is signals' alone; an unread or unresolved signals keeps the
/// audited guard. rc.0's `createStore` answer (N3) is unchanged.
#[test]
fn the_optimistic_store_setter_guard_follows_the_resolved_signals() {
    for (signals, guarded) in [
        (Some("2.0.0-rc.0"), false),
        (Some("2.0.0-rc.1"), true),
        (Some("2.0.0-rc.3"), true),
        (Some("2.0.0-rc.9"), true),
        (Some("2.0.0-rc.10"), true),
        (Some("2.0.0-beta.2"), true),
        (None, true),
    ] {
        let (vocabulary, _) = analyzed(&triple(Some("2.0.0-rc.3"), signals, Some("2.0.0-rc.3")));
        assert_eq!(
            vocabulary.optimistic_store_setter_guarded(),
            guarded,
            "{signals:?}"
        );
    }
    let (rc0, _) = analyzed(&same("2.0.0-rc.0"));
    assert!(!rc0.optimistic_store_setter_guarded());
    assert!(rc0.store_setter_guard_exempts_roots());
    assert_eq!(rc0.store_setter_roots, StoreSetterRootGuard::Exempt);
    assert_eq!(
        rc0.key(),
        Some(
            "optimistic-store-setter-unguarded+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline"
        )
    );
    assert!(Solid2::RC3.optimistic_store_setter_guarded());
    assert!(Solid2::RC9.optimistic_store_setter_guarded());
    // Rows rc.1-rc.3 still share rc.3's vocabulary.
    for release in ["2.0.0-rc.1", "2.0.0-rc.2", "2.0.0-rc.3"] {
        assert_eq!(
            analyzed(&same(release)).0.index(),
            Solid2::RC3.index(),
            "{release}"
        );
    }
}

/// Defect 2: B2 and B4 answer only where the owner has the feature.
#[test]
fn until_and_the_static_form_need_their_owners() {
    for (solid_js, signals, until) in [
        ("2.0.0-rc.4", "2.0.0-rc.9", false),
        ("2.0.0-rc.5", "2.0.0-rc.4", false),
        ("2.0.0-rc.5", "2.0.0-rc.5", true),
        ("2.0.0-rc.9", "2.0.0-rc.10", false),
    ] {
        let (vocabulary, _) = analyzed(&triple(Some(solid_js), Some(signals), None));
        assert_eq!(vocabulary.until, until, "{solid_js} over {signals}");
    }
    for (web, options) in [
        ("2.0.0-rc.3", DynamicOptions::Ignored),
        ("2.0.0-rc.7", DynamicOptions::Ignored),
        ("2.0.0-rc.8", DynamicOptions::Ignored),
        ("2.0.0-rc.9", DynamicOptions::StaticForm),
        ("2.0.0-rc.10", DynamicOptions::Unread),
    ] {
        let (vocabulary, _) = analyzed(&triple(Some("2.0.0-rc.9"), Some("2.0.0-rc.9"), Some(web)));
        assert_eq!(vocabulary.dynamic_options, options, "{web}");
    }
}

/// The conservative answers, each with a gap naming it.
#[test]
fn an_unknown_or_missing_owner_takes_the_conservative_answers() {
    for signals in [
        None,
        Some("2.0.0-rc.10"),
        Some("2.0.0-beta.2"),
        Some("^2.0.0-rc.3"),
    ] {
        let (vocabulary, gaps) = analyzed(&triple(Some("2.0.0-rc.9"), signals, Some("2.0.0-rc.9")));
        assert_eq!(
            vocabulary.store_root,
            StoreRootTyping::Readonly,
            "{signals:?}"
        );
        assert!(!vocabulary.until, "{signals:?}");
        assert_eq!(
            vocabulary.store_setter_roots,
            StoreSetterRootGuard::Exempt,
            "{signals:?}"
        );
        assert!(
            gaps.iter()
                .any(|gap| gap.gap.contains("store root") && gap.review.is_none()),
            "{signals:?}: {gaps:?}"
        );
    }
    let (vocabulary, gaps) = analyzed(&triple(
        Some("2.0.0-rc.3"),
        Some("2.0.0-rc.3"),
        Some("2.0.1"),
    ));
    assert_eq!(vocabulary.dynamic_options, DynamicOptions::Unread);
    // The unread web, and the rc.3 pair's being older than the audited
    // release.
    assert_eq!(gaps.len(), 2, "{gaps:?}");
    // A web that does not resolve is not asked about, so it adds no gap;
    // the rc.3 pair beside it keeps its reviewed answers and only the
    // older-release gap.
    let (vocabulary, gaps) = analyzed(&triple(Some("2.0.0-rc.3"), Some("2.0.0-rc.3"), None));
    // Only the web-owned `hydrate` answer differs from rc.3's: nothing
    // imports it, and the unresolved owner keeps the conservative one.
    assert_eq!(
        vocabulary.index(),
        Solid2 {
            hydrate_code_inline: false,
            ..Solid2::RC3
        }
        .index()
    );
    assert_eq!(gaps.len(), 1, "{gaps:?}");
    assert!(
        gaps[0].gap.contains("older than the audited release"),
        "{gaps:?}"
    );
    // An unknown solid-js keeps the conservative answers it owns, with a
    // gap, beside the rc.3 pair's older-release gap.
    let (vocabulary, gaps) = analyzed(&triple(
        Some("2.0.0"),
        Some("2.0.0-rc.3"),
        Some("2.0.0-rc.3"),
    ));
    assert!(!vocabulary.until);
    assert_eq!(gaps.len(), 2, "{gaps:?}");
}

#[test]
fn releases_read_only_their_exact_spelling() {
    for (version, release) in [
        ("2.0.0-rc.3", Release::Read(3)),
        (" 2.0.0-rc.3\n", Release::Read(3)),
        ("2.0.0-rc.0", Release::Read(0)),
        ("2.0.0-rc.9", Release::Read(9)),
        ("2.0.0-rc.10", Release::Unread("2.0.0-rc.10")),
        ("2.0.0-rc.03", Release::Unread("2.0.0-rc.03")),
        ("2.0.0-rc.3.1", Release::Unread("2.0.0-rc.3.1")),
        ("2.0.0-rc.3+local", Release::Unread("2.0.0-rc.3+local")),
        ("^2.0.0-rc.3", Release::Unread("^2.0.0-rc.3")),
        ("v2.0.0-rc.3", Release::Unread("v2.0.0-rc.3")),
        ("2.0.0", Release::Unread("2.0.0")),
        ("2.0.0-beta.19", Release::Unread("2.0.0-beta.19")),
    ] {
        assert_eq!(Release::of(Some(version)), release, "{version:?}");
    }
    assert_eq!(Release::of(None), Release::Unresolved);
}

#[test]
fn the_pre_beta_experiment_is_refused_by_its_solid_js() {
    for refused in [
        "2.0.0-experimental.1",
        "2.0.0-experimental.0",
        "2.0.0-experimental",
    ] {
        assert!(
            matches!(
                review(&same(refused)),
                InstallationReview::Refused(refusal) if std::ptr::eq(refusal, &PRE_BETA_EXPERIMENT)
            ),
            "{refused}"
        );
    }
    assert!(matches!(
        review(&triple(
            Some("2.1.0-experimental.1"),
            Some("2.0.0-rc.3"),
            None
        )),
        InstallationReview::Analyzed { .. }
    ));
}

/// Every variant round-trips through its index, and no two share a key.
#[test]
fn every_variant_has_its_own_key() {
    let mut keys = std::collections::BTreeSet::new();
    for (index, vocabulary) in VARIANTS.iter().enumerate() {
        assert_eq!(vocabulary.index(), index);
        assert!(
            keys.insert(vocabulary.key()),
            "{index}: {:?}",
            vocabulary.key()
        );
        assert_eq!(vocabulary.key().is_none(), index == 0);
        // The key decodes back to the same interned vocabulary.
        if let Some(key) = vocabulary.key() {
            let decoded = Solid2::of_key(key).expect("every key decodes");
            assert!(std::ptr::eq(decoded, vocabulary), "{key}");
        }
    }
    for malformed in [
        "",
        "until+until",
        "omit-predicate+store-root-mutable",
        "nope",
        "until+",
    ] {
        assert!(Solid2::of_key(malformed).is_none(), "{malformed:?}");
    }
    assert_eq!(OTHER_VARIANTS.len(), Solid2::VARIANT_COUNT - 1);
    assert_eq!(
        Solid2::RC9.key(),
        Some(
            "store-root-mutable+omit-predicate+until+flush-in-action+flush-in-effect-callback+dynamic-static+store-setter-guards-roots+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline"
        )
    );
}

/// The conservative answers are the enum defaults and the unresolved
/// owner's, and they are not read from the audited release: nothing here
/// names [`Solid2::AUDITED`] or [`AUDITED_INSTALLATION`], so pointing
/// either at another release leaves this test, and the plain `solid-v2`
/// id it anchors, as they are.
#[test]
fn the_conservative_answers_do_not_depend_on_the_audited_release() {
    let conservative = Solid2::CONSERVATIVE;
    assert_eq!(conservative.index(), 0);
    assert_eq!(Solid2::default().index(), conservative.index());
    assert_eq!(conservative.store_root, StoreRootTyping::Readonly);
    assert!(!conservative.omit_predicate_form);
    assert!(!conservative.until);
    assert_eq!(conservative.dynamic_options, DynamicOptions::Ignored);
    assert_eq!(
        conservative.store_setter_roots,
        StoreSetterRootGuard::Exempt
    );
    assert!(!conservative.flush_in_action);
    assert!(!conservative.flush_in_effect_callback);
    assert_eq!(
        conservative.optimistic_store_setter,
        OptimisticStoreSetterGuard::Guarded
    );
    assert!(!conservative.lazy_renders_component);
    // It is exactly what an installation with nothing resolved reads as.
    assert_eq!(
        vocabulary_for(
            Release::Unresolved,
            Release::Unresolved,
            Release::Unresolved
        )
        .index(),
        conservative.index()
    );
    // Every answer `@solidjs/signals` owns falls back to it, over the
    // newest read `solid-js` and `@solidjs/web`.
    let rc9 = Release::Read(RC9);
    for signals in [Release::Unresolved, Release::Unread("2.0.0-rc.10")] {
        let vocabulary = vocabulary_for(rc9, signals, rc9);
        assert_eq!(vocabulary.store_root, conservative.store_root);
        assert_eq!(
            vocabulary.omit_predicate_form,
            conservative.omit_predicate_form
        );
        assert_eq!(vocabulary.until, conservative.until);
        assert_eq!(
            vocabulary.store_setter_roots,
            conservative.store_setter_roots
        );
        assert_eq!(vocabulary.flush_in_action, conservative.flush_in_action);
        assert_eq!(
            vocabulary.flush_in_effect_callback,
            conservative.flush_in_effect_callback
        );
        assert_eq!(
            vocabulary.optimistic_store_setter,
            conservative.optimistic_store_setter
        );
    }
    // Ids are spelled against it. rc.3 is no longer the vocabulary that
    // names nothing: it reports `flush` in an effect's apply (N6), which an
    // installation whose signals nobody read does not, so its id carries that
    // answer (ADR 0341).
    assert_eq!(conservative.key(), None);
    assert_eq!(
        Solid2::RC3.key(),
        Some(
            "flush-in-effect-callback+optimistic-setter-guarded+latest-rejects-pending+hydrate-inline"
        )
    );
    assert!(Solid2::RC9.key().is_some());
}

/// The audited vocabulary is the one its installation reviews to, with no
/// gap, and every sentence naming the audited release reads it from
/// [`AUDITED_INSTALLATION`].
#[test]
fn the_audited_vocabulary_is_what_the_audited_installation_reviews_to() {
    let installed = OWNERS
        .iter()
        .zip(AUDITED_INSTALLATION)
        .map(|(owner, (package, version))| {
            assert_eq!(owner.package, *package, "in owner order");
            InstalledRelease {
                package,
                version: Some(version),
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(installed.len(), OWNERS.len());
    let (vocabulary, gaps) = analyzed(&installed);
    assert_eq!(vocabulary.index(), Solid2::AUDITED.index());
    // The audited release carries no gap but import-scoped ones, which
    // are due only where a project reaches the exports they name.
    assert!(gaps.iter().all(|gap| gap.scope.is_some()), "{gaps:?}");
    assert!(std::ptr::eq(
        Solid2::DEFAULTED.interned(),
        &VARIANTS[Solid2::DEFAULTED.index()]
    ));
    assert_eq!(PRE_BETA_EXPERIMENT.audited, AUDITED_INSTALLATION);
    for package in [SOLID_JS, WEB] {
        assert!(
            unread_consequence(package)
                .contains(&format!("read as {} declares it", audited_release(package))),
            "{package}"
        );
    }
}

/// What the engine asks, through the trait, follows the variant; the
/// review does not depend on which variant is asked.
#[test]
fn the_trait_answers_from_the_variant() {
    assert!(Solid2::RC3.store_root_properties_are_readonly());
    assert!(!Solid2::RC9.store_root_properties_are_readonly());
    // The value the engine holds by name is the conservative vocabulary.
    assert!(Solid2.store_root_properties_are_readonly());
    assert!(Solid2::default().store_root_properties_are_readonly());
    assert_eq!(Solid2::default().index(), Solid2::CONSERVATIVE.index());
    assert!(Solid2::RC9.store_setter_callback_enables_proxy_writes());
    assert!(Solid2::RC3.store_setter_guard_exempts_roots());
    assert!(!Solid2::RC9.store_setter_guard_exempts_roots());
    for release in 0..=8 {
        let release = format!("2.0.0-rc.{release}");
        assert!(
            analyzed(&same(&release))
                .0
                .store_setter_guard_exempts_roots(),
            "{release}"
        );
    }
    // Only `createRoot`'s own body is a created root on the conservative
    // vocabulary. `render` and `hydrate` join it on the audited rc.13 web
    // alone (ADR 0353).
    assert!(Solid2.callback_runs_in_created_root(Primitive::CreateRoot, 0));
    for (primitive, argument) in [
        (Primitive::CreateRoot, 1),
        (Primitive::RunWithOwner, 1),
        (Primitive::Untrack, 0),
        (Primitive::Flush, 0),
        (Primitive::CreateMemo, 0),
        (Primitive::Render, 0),
        (Primitive::Hydrate, 0),
    ] {
        assert!(
            !Solid2.callback_runs_in_created_root(primitive, argument),
            "{primitive:?} {argument}"
        );
    }
    assert!(Solid2::RC13.callback_runs_in_created_root(Primitive::CreateRoot, 0));
    for primitive in [Primitive::Render, Primitive::Hydrate] {
        assert!(
            Solid2::RC13.callback_runs_in_created_root(primitive, 0),
            "{primitive:?}"
        );
        assert!(
            !Solid2::RC13.callback_runs_in_created_root(primitive, 1),
            "{primitive:?} 1"
        );
    }
    // `flush(fn)` creates no root: it keeps the caller's owner, as
    // `untrack` does, on every release (probed rc.0-rc.9).
    for vocabulary in [Solid2::RC3, Solid2::RC9] {
        assert!(vocabulary.callback_preserves_owner_write_context(Primitive::Flush));
        assert!(vocabulary.callback_preserves_owner_write_context(Primitive::Untrack));
        assert!(!vocabulary.callback_preserves_owner_write_context(Primitive::CreateRoot));
        // The dev component body runs under `createRoot(…, { transparent: true })`.
        assert!(vocabulary.component_body_runs_under_root());
    }
    for gated in super::super::RELEASE_GATED_NAMES {
        assert_eq!(Solid2::RC3.primitive(gated), None, "{gated}");
        assert!(Solid2::RC17.primitive(gated).is_some(), "{gated}");
    }
    assert!(Solid2::RC9.primitive("until").is_some());
    // New in rc.17 (the rc.17 review): no earlier vocabulary has them.
    for name in ["dynamicComponent", "dispatchAsInteraction"] {
        for vocabulary in [Solid2::RC9, Solid2::RC13] {
            assert_eq!(vocabulary.primitive(name), None, "{name}");
            assert!(
                vocabulary
                    .export_modules(name, crate::ExportPosition::Value)
                    .is_empty(),
                "{name}"
            );
        }
    }
    let expected = Solid2::RC9.interned() as *const Solid2;
    for vocabulary in &VARIANTS {
        let InstallationReview::Analyzed {
            vocabulary: Some(chosen),
            ..
        } = vocabulary.review_installation(&same("2.0.0-rc.9"))
        else {
            panic!("rc.9 is analyzed");
        };
        assert!(std::ptr::addr_eq(chosen as *const dyn Dialect, expected));
    }
}

/// rc.9's unresolvable re-exports are the one gap scoped to the exports a
/// project reaches; every other gap stays release-wide.
#[test]
fn only_the_rc9_re_export_gap_is_scoped_to_the_names_it_loses() {
    let (_, gaps) = analyzed(&same("2.0.0-rc.9"));
    let scoped = gaps
        .iter()
        .filter_map(|gap| gap.scope.map(|scope| (gap, scope)))
        .collect::<Vec<_>>();
    let [(gap, scope)] = scoped.as_slice() else {
        panic!("exactly one rc.9 gap is scoped: {gaps:?}");
    };
    assert!(
        gap.gap.starts_with("solid-js@2.0.0-rc.9 re-exports"),
        "{gap:?}"
    );
    assert_eq!(scope.specifier, SOLID_JS);
    assert_eq!(
        scope.exports,
        [
            "createErrorBoundary",
            "createLoadingBoundary",
            "createRevealOrder",
            "sharedConfig",
            "$DEVCOMP",
        ]
    );
    for export in scope.exports {
        assert!(gap.gap.contains(export), "the gap names {export}");
    }
    // The signals gap, and every gap of another release or a mixed
    // installation, is release-wide.
    for installed in [
        same("2.0.0-rc.4"),
        same("2.0.0-rc.7"),
        same("2.0.0-rc.10"),
        triple(Some("2.0.0-rc.9"), Some("2.0.0-rc.3"), None),
    ] {
        let (_, gaps) = analyzed(&installed);
        assert!(
            gaps.iter()
                .filter(|gap| gap.scope.is_some())
                .all(|gap| gap.gap.starts_with("solid-js@2.0.0-rc.9 ")),
            "{installed:?}: {gaps:?}"
        );
    }
}

#[test]
fn every_review_names_a_document_that_exists() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for document in [
        RC17_REVIEW,
        RC13_REVIEW,
        RC9_REVIEW,
        RC1_RC8_REVIEW,
        PRE_BETA_EXPERIMENT.review,
    ] {
        assert!(root.join(document).is_file(), "{document} is missing");
    }
    for known in KNOWN_GAPS {
        assert!(known.from <= known.through && READ_RELEASES.contains(&known.through));
        assert!(OWNERS.iter().any(|owner| owner.package == known.package));
    }
}

/// ADR 0358: spread listener grammar belongs to the audited web runtime.
#[test]
fn spread_event_keys_require_web_rc13() {
    let audited = vocabulary_for(Release::Read(9), Release::Read(9), Release::Read(13));
    for key in ["onClick", "onclick", "onpointerdown", "on"] {
        assert_eq!(audited.spread_key_is_event(key), Some(true), "{key}");
    }
    for key in ["title", "OnClick", "on:click", "prop:onClick"] {
        assert_eq!(audited.spread_key_is_event(key), Some(false), "{key}");
    }
    let older_web = vocabulary_for(Release::Read(13), Release::Read(13), Release::Read(9));
    assert_eq!(older_web.spread_key_is_event("onClick"), None);
    assert_eq!(Solid2::CONSERVATIVE.spread_key_is_event("onClick"), None);
}

/// The rc.17 review: `on` + an uppercase letter is a spread listener on web
/// rc.17, `createOptimistic`'s setter is unguarded and `latest` absorbs a
/// pending read on signals rc.17, `hydrate` may defer `code` on web rc.17,
/// and `dynamic`'s static form needs `solid-js` rc.17 under web rc.17.
#[test]
fn rc17_answers_follow_their_owners() {
    let audited = analyzed(&same("2.0.0-rc.17")).0;
    for key in ["onClick", "onPointerDown"] {
        assert_eq!(audited.spread_key_is_event(key), Some(true), "{key}");
    }
    for key in ["onclick", "on", "on-foo", "on:click", "title"] {
        assert_eq!(audited.spread_key_is_event(key), Some(false), "{key}");
    }
    assert!(!audited.optimistic_signal_setter_guarded());
    assert!(Solid2::RC13.optimistic_signal_setter_guarded());
    assert!(audited.callback_handles_pending_accessor_read(Primitive::Latest, 0));
    assert!(!Solid2::RC13.callback_handles_pending_accessor_read(Primitive::Latest, 0));
    assert!(audited.callback_handles_pending_accessor_read(Primitive::IsPending, 0));
    assert!(audited.callback_executions(Primitive::Hydrate).is_empty());
    assert!(
        !Solid2::RC13
            .callback_executions(Primitive::Hydrate)
            .is_empty()
    );
    assert!(!audited.callback_executions(Primitive::Render).is_empty());
    assert!(audited.callback_runs_in_created_root(Primitive::Hydrate, 0));
    assert!(audited.lazy_renders_component && audited.store_strict_keys_audited);
    assert_eq!(audited.dynamic_options, DynamicOptions::StaticForm);
    assert_eq!(
        vocabulary_for(Release::Read(13), Release::Read(13), Release::Read(17)).dynamic_options,
        DynamicOptions::Unread
    );
    // An unread signals claims neither guard nor rejection.
    let unread = analyzed(&same("2.0.0-rc.16")).0;
    assert!(!unread.optimistic_signal_setter_guarded());
    assert!(unread.callback_handles_pending_accessor_read(Primitive::Latest, 0));
}

/// The rc.17 review's new callback-taking exports: `dynamicComponent` and
/// `dispatchAsInteraction` are modelled on the rc.17 triple, and stay a
/// scoped gap on any other installation that has them; `setLinkClaim` stays
/// a gap everywhere.
#[test]
fn rc17_callback_exports_are_modelled_only_on_the_rc17_triple() {
    let (audited, gaps) = analyzed(&same("2.0.0-rc.17"));
    assert!(audited.rc17_callback_exports);
    let scoped = |gaps: &[InstallationGap]| {
        let mut names = gaps
            .iter()
            .filter_map(|gap| gap.scope.map(|scope| (scope.specifier, scope.exports)))
            .flat_map(|(specifier, exports)| exports.iter().map(move |name| (specifier, *name)))
            .collect::<Vec<_>>();
        names.sort_unstable();
        names
    };
    assert_eq!(
        scoped(&gaps),
        [
            (WEB, "setLinkClaim"),
            (SOLID_JS, "createDeferred"),
            (SOLID_JS, "lazyModule"),
            (SOLID_JS, "onHydrationEnd"),
        ]
    );
    for gap in &gaps {
        for name in ["dynamicComponent", "dispatchAsInteraction"] {
            assert!(!gap.gap.contains(name), "{gap:?}");
        }
    }
    // A mixed rc.17 installation keeps both unread and names them.
    for installed in [
        triple(
            Some("2.0.0-rc.17"),
            Some("2.0.0-rc.13"),
            Some("2.0.0-rc.17"),
        ),
        triple(
            Some("2.0.0-rc.17"),
            Some("2.0.0-rc.17"),
            Some("2.0.0-rc.13"),
        ),
    ] {
        let (vocabulary, gaps) = analyzed(&installed);
        assert!(!vocabulary.rc17_callback_exports, "{installed:?}");
        assert_eq!(vocabulary.primitive("dynamicComponent"), None);
        assert_eq!(vocabulary.primitive("dispatchAsInteraction"), None);
        assert!(
            scoped(&gaps).contains(&(SOLID_JS, "dynamicComponent")),
            "{installed:?}"
        );
    }
    let (_, gaps) = analyzed(&triple(
        Some("2.0.0-rc.17"),
        Some("2.0.0-rc.13"),
        Some("2.0.0-rc.17"),
    ));
    assert!(scoped(&gaps).contains(&(WEB, "dispatchAsInteraction")));
    assert!(scoped(&gaps).contains(&(WEB, "dynamicComponent")));
    // Both names resolve, from the modules that export them.
    assert_eq!(
        audited.primitive("dynamicComponent"),
        Some(Primitive::DynamicComponent)
    );
    assert_eq!(
        audited.export_modules("dynamicComponent", crate::ExportPosition::Value),
        ["solid-js", "@solidjs/web"]
    );
    assert_eq!(
        audited.export_modules("dispatchAsInteraction", crate::ExportPosition::Value),
        ["@solidjs/web"]
    );
    assert_eq!(audited.primitive("setLinkClaim"), None);
}
