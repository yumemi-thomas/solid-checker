#[path = "support/diagnostics.rs"]
mod support;

use support::{assert_rule_findings, diagnostic_fixture, findings_for_rule};

#[test]
fn write_scope_diagnostics_have_semantic_locations() {
    let Some(findings) = diagnostic_fixture("write-scope") else {
        return;
    };
    // 14 writes / 2 actions: the untrack-wrapped writes in the component body
    // and in a memo count (the rc.0 guard keys on the owner, not tracking),
    // while writes and the action inside createTrackedEffect no longer do
    // (children-forbidden leaf scopes are legal write regions).
    assert_eq!(
        (
            findings_for_rule(&findings, "reactive-write-in-owned-scope").len(),
            findings_for_rule(&findings, "action-called-in-owned-scope").len(),
        ),
        (14, 2)
    );
    assert!(
        findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding["rule"].as_str(),
                    Some("reactive-write-in-owned-scope" | "action-called-in-owned-scope")
                )
            })
            .all(|finding| {
                finding["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with(".tsx"))
                    && finding["message"].as_str().is_some_and(|message| {
                        message.contains("owned scope") || message.contains("action")
                    })
            })
    );
}

#[test]
fn diagnostic_domains_match_the_solid_two_matrix() {
    for (fixture, rules) in [
        (
            "leaf-owner",
            &[
                // Three owner-backed inline violations, two dynamic-extent
                // violations reached through exact helpers (registerTeardown
                // and the transitive indirectTeardown), plus the
                // exported-helper proof obligation. Silent, each for its own
                // reason: the out-of-band (event handler) onSettled, the
                // helper call written inside the event handler, the helper
                // that only builds a nested function, and — because the
                // argument is not a function literal the owner receives —
                // both `createTrackedEffect(makeTeardownCallback())` and
                // `createTrackedEffect(wrapCallback(() => …))`.
                ("cleanup-in-forbidden-scope", 6),
                // Three inline (the function-seeded createSignal, createMemo,
                // createRoot) plus the dynamic-extent trackDouble() reached
                // through its helper.
                ("primitive-in-leaf-owner", 4),
                // Two inline plus flushNow() reached through its helper from
                // a block-bodied and an expression-bodied leaf callback.
                ("flush-in-forbidden-scope", 4),
                // The six returns that used to be reported here are the
                // domain `EffectFunction`'s `(() => void) | void` return type
                // rejects, so the value-legality rules are gone; the fixture
                // keeps them as sources because the ownership rules still
                // classify them.
                ("invalid-cleanup-return", 0),
                ("cleanup-return-unresolved", 0),
            ][..],
        ),
        (
            "static-api",
            &[
                // Absent, `undefined`, `null`, `5`, and `"apply"` second
                // arguments: the last three are proven non-functions that
                // crash the effect queue. The `{ effect, error }` object and
                // the plain apply function stay silent.
                ("missing-effect-function", 5),
                // Signal-family only: the store constructors never route
                // options.sync into their node, so their three sync: true
                // async derives are negative cases now.
                ("sync-node-received-async", 3),
                // The refresh/affects target rules were removed on
                // 2026-08-17: `Refreshable<T>` brands the target in the type
                // system, so every spelling this fixture writes -- wrapper,
                // literal, zero-arg, value-form store, store child record,
                // accessor member chain, and a key on an accessor -- is a
                // TS2345. Pinned at 0 so a reintroduction fails here.
                ("invalid-refresh-target", 0),
                ("invalid-affects-target", 0),
                ("affects-keys-on-accessor", 0),
                ("reactive-write-in-owned-scope", 1),
            ],
        ),
        (
            "directive-phases",
            &[
                // Building a directive value happens while the component
                // renders; only invoking the returned directive is in the
                // compiler's directive-application phase.
                ("reactive-write-in-owned-scope", 2),
                // Owner-attaching creations only: the direct createMemo, the
                // forwarded createEffect, and the function-form createSignal.
                // The three value-form createSignal(element) calls allocate
                // plain state that needs no owner and stay silent.
                ("primitive-in-directive-application", 3),
            ],
        ),
        (
            "owner-presence",
            &[
                ("no-owner-effect", 7),
                ("no-owner-cleanup", 2),
                ("no-owner-boundary", 3),
                ("no-owner-settled-cleanup", 2),
            ],
        ),
        (
            "async-boundary",
            &[
                // Four untracked reads: two plain async sources, the
                // declared-loadingValue source (still reported — the declared
                // window ends at the first real answer, so later re-asks
                // throw; conditional wording), and the opaque-options source
                // (downgraded to uncertifiable, asserted below).
                ("pending-async-untracked-read", 4),
                ("pending-async-forbidden-scope", 3),
                // The declared sources (loadingValue memo, seedLoadingValue
                // projection and store) render bare without any SC5003: their
                // first flight never trips a Loading boundary. The
                // opaque-options render keeps the informational warning.
                ("async-outside-loading-boundary", 12),
            ],
        ),
        (
            "ssr-client-boundary",
            &[
                // Only the bare ssrSource: "client" read outside Loading in a
                // server-rendering project; the bounded, loadingValue, and
                // seedLoadingValue reads stay silent.
                ("ssr-client-source-outside-loading-boundary", 1),
            ],
        ),
    ] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            return;
        };
        for (rule, expected) in rules {
            assert_rule_findings(&findings, rule, *expected);
        }
    }
}

#[test]
fn settled_leaf_rules_follow_call_site_ownership() {
    let Some(findings) = diagnostic_fixture("leaf-owner") else {
        return;
    };
    let cleanup = findings_for_rule(&findings, "cleanup-in-forbidden-scope");
    // The out-of-band onSettled (event handler) must not carry any leaf-scope
    // finding: the runtime enqueues a plain callback there.
    assert!(
        findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding["rule"].as_str(),
                    Some(
                        "cleanup-in-forbidden-scope"
                            | "primitive-in-leaf-owner"
                            | "flush-in-forbidden-scope"
                    )
                )
            })
            .all(|finding| {
                finding["message"]
                    .as_str()
                    .is_some_and(|message| !message.contains("OutOfBand"))
            }),
        "{findings:#?}"
    );
    // The exported helper's call sites are unknowable, so its onSettled leaf
    // finding is a proof obligation, not a proven violation; the owner-backed
    // component-body ones stay violations — three inline, two reached through
    // the exactly-resolved dynamic-extent helpers.
    let kinds = cleanup
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "violation").count(),
        5,
        "{cleanup:#?}"
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == "uncertifiable")
            .count(),
        1,
        "{cleanup:#?}"
    );
}

#[test]
fn solid2_precision_corrections_are_end_to_end() {
    let Some(findings) = diagnostic_fixture("solid2-precision") else {
        return;
    };
    let source = include_str!("../../../../fixtures/reactive-ir/solid2-precision/App.tsx");
    let start_of = |needle: &str| {
        source
            .find(needle)
            .unwrap_or_else(|| panic!("fixture landmark {needle:?}")) as u64
    };
    let starts = |rule: &str| {
        findings_for_rule(&findings, rule)
            .iter()
            .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
            .collect::<Vec<_>>()
    };

    // Each count pins one side of a proof. SC1002: the accessor call and the
    // store member read inside the exact `Array#filter` callback, and nothing
    // for the Promise, shadowed, unresolved, or wrapper-built callbacks.
    // The cleanup-return *value* rules are gone (every illegal return is a
    // TypeScript error against the real `EffectFunction` signature), so the
    // contextual, explicit, parenthesized, `as`-cast, member, and returned-call
    // spellings this fixture still writes are pinned through the ownership
    // rules below instead of through a legality finding.
    // SC1001/SC2003: a plain store write is a write only,
    // while the compound and update forms also read their target and a
    // computed key stays a read. SC3001/SC4002/SC4004: the one owner-backed
    // settled cleanup written as a literal callback reports SC3001 without a
    // duplicate SC4002; the wrapper-built, identifier-referenced, and
    // out-of-band cleanups are SC4002 only. The returned-call block adds three
    // SC3004 (a produced `number` in both return spellings, plus the unowned
    // one), one SC9002 (`any`), and one SC4004 (a produced function is a real
    // cleanup); a produced function, `(() => void) | undefined`, and `void`
    // are legal and silent.
    for (rule, expected) in [
        ("reactive-read-after-await", 2),
        ("invalid-cleanup-return", 0),
        ("cleanup-return-unresolved", 0),
        ("strict-read-untracked", 5),
        ("no-direct-mutation", 4),
        ("cleanup-in-forbidden-scope", 1),
        ("no-owner-cleanup", 3),
        ("no-owner-settled-cleanup", 2),
    ] {
        assert_rule_findings(&findings, rule, expected);
    }

    // The leaf rules need the literal callback *and* the call's place in its
    // own synchronous extent. Past this landmark every leaf-owner call in the
    // fixture is wrapper-built, handed over as an identifier reference, or
    // has its onCleanup in a nested function the callback merely builds — no
    // leaf scope is proven at any of them, so SC3001 stops here while the
    // genuinely unowned SC4002 continues.
    let non_literal_leaf = start_of("// `wrap` may stash");
    assert!(
        starts("cleanup-in-forbidden-scope")
            .iter()
            .all(|start| *start < non_literal_leaf),
        "a leaf callback that is not the literal argument proves no leaf scope"
    );
    assert_eq!(
        starts("no-owner-cleanup")
            .iter()
            .filter(|start| **start > non_literal_leaf)
            .count(),
        3,
        "the wrapped, referenced, and out-of-band cleanups stay unowned"
    );

    assert!(
        !starts("strict-read-untracked").contains(&start_of("profile.name =")),
        "a plain assignment target is a write, not a read"
    );
    assert!(
        starts("strict-read-untracked").contains(&start_of("props.index")),
        "a computed key inside an assignment target is still a read"
    );

    // `filter(makePredicate(post => …))` hands the arrow to a wrapper, so the
    // accessor call inside it is not proven to run before the await resumes.
    let wrapped_callback = start_of("makePredicate((post)");
    assert!(
        starts("reactive-read-after-await")
            .iter()
            .all(|start| *start < wrapped_callback),
        "a wrapper-built filter callback is not a proven synchronous read"
    );

    // A returned call is still classified from its result, not from its
    // callable callee — the ownership half of that work is what survives the
    // removal of the legality rules. `return makeCount()` produces a number,
    // so no cleanup is handed over and SC4004 must not fire; the neighbouring
    // `onSettled(() => makeThunk())` produces a function and must.
    // Anchored on the module-level spellings: the returned expression is the
    // reported span, and both callees also appear inside the component above.
    let returned_call = start_of("makeCount();\n});");
    let unowned_thunk =
        start_of("onSettled(() => makeThunk())") + u64::try_from("onSettled(".len()).unwrap();
    assert!(
        !starts("no-owner-settled-cleanup").contains(&returned_call),
        "a call producing a number hands the owner no cleanup to leak"
    );
    assert!(
        starts("no-owner-settled-cleanup").contains(&unowned_thunk),
        "a call producing a function is an unowned returned cleanup"
    );

    // `return nothing` where `nothing: undefined` is a legal cleanup return
    // that hands the owner nothing, so it is not a returned cleanup that would
    // make these unowned callbacks SC4004.
    let typed_undefined = start_of("// `nothing` is provably");
    assert!(
        findings
            .iter()
            .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
            .all(|start| start < typed_undefined),
        "a proven-`undefined` cleanup return is silent in every cleanup rule"
    );
}

#[test]
fn solid_one_missing_wording_paths_are_end_to_end() {
    let Some(findings) = diagnostic_fixture("no-owner-v1") else {
        return;
    };

    for (rule, expected) in [
        ("v1/no-owner-effect", 1),
        ("v1/no-owner-boundary", 1),
        ("v1/primitive-in-directive-application", 1),
    ] {
        assert_rule_findings(&findings, rule, expected);
    }
    assert_eq!(
        findings_for_rule(&findings, "v1/package-contract-missing").len(),
        1,
        "v1 package-contract wording path must run end to end: {findings:#?}"
    );
}

#[test]
fn declared_first_paint_and_opaque_options_split_the_async_rules() {
    let Some(findings) = diagnostic_fixture("async-boundary") else {
        return;
    };
    // Fail-honest policy: SC5001 stays a proven violation when the options
    // argument is absent or readable, and downgrades to a proof obligation
    // when an unreadable options argument could declare a loadingValue.
    let untracked = findings_for_rule(&findings, "pending-async-untracked-read");
    let kinds = untracked
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "violation").count(),
        3,
        "{untracked:#?}"
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == "uncertifiable")
            .count(),
        1,
        "{untracked:#?}"
    );
    // The declared source keeps SC5001/SC5002 with conditional wording: the
    // first flight cannot throw, later re-asks can (probed against rc.0).
    assert!(
        untracked.iter().any(|finding| {
            finding["message"].as_str().is_some_and(|message| {
                message.contains("declares a loadingValue")
                    && message.contains("after the first real answer lands")
            })
        }),
        "{untracked:#?}"
    );
    // No declared source may carry the boundary warning: the declared first
    // paint is exactly what makes a Loading boundary unnecessary.
    assert!(
        findings_for_rule(&findings, "async-outside-loading-boundary")
            .iter()
            .all(|finding| {
                finding["message"].as_str().is_some_and(|message| {
                    !message.contains("declaredFeed")
                        && !message.contains("seededUser")
                        && !message.contains("seededStoreUser")
                })
            }),
        "{findings:#?}"
    );
}

#[test]
fn ssr_client_hole_requires_a_server_rendering_project() {
    let Some(findings) = diagnostic_fixture("ssr-client-boundary") else {
        return;
    };
    let holes = findings_for_rule(&findings, "ssr-client-source-outside-loading-boundary");
    assert_eq!(holes.len(), 1, "{findings:#?}");
    assert_eq!(holes[0]["kind"], "violation", "{holes:#?}");
    // The server throw is unconditional, so the rule mirrors it as an error.
    assert_eq!(holes[0]["severity"], "error", "{holes:#?}");
    assert!(
        holes[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("ssrSource: \"client\"")),
        "{holes:#?}"
    );
    // The same bare client source read in a CSR-only project must stay
    // silent: the throwing code path lives in the server runtime.
    let Some(csr_findings) = diagnostic_fixture("ssr-client-boundary-csr") else {
        return;
    };
    assert!(csr_findings.is_empty(), "{csr_findings:#?}");
}

/// The wave-6 server-surface and resolve rules, pinned at their probed
/// gates: SC7005's server-render + Loading-children dominance, SC7006's
/// module-directive export shapes, SC7007's enableRichArguments silence, and
/// SC2004's observer-keyed scope split.
#[test]
fn server_surface_and_resolve_rules_pin_their_probed_gates() {
    if let Some(findings) = diagnostic_fixture("http-response-flush") {
        let drops = findings_for_rule(&findings, "http-response-after-flush");
        // The two component-body calls below the Loading boundary plus the
        // lexical call in its children; the shell, fallback, and
        // event-handler calls stay silent.
        assert_eq!(drops.len(), 3, "{findings:#?}");
        assert!(
            drops
                .iter()
                .all(|finding| finding["severity"] == "warning" && finding["kind"] == "violation"),
            "the post-flush drop is conditional and must stay a warning: {drops:#?}"
        );
        // CSR twin: both exports are client no-ops everywhere, no drop to
        // report.
        if let Some(csr) = diagnostic_fixture("http-response-flush-csr") {
            assert!(csr.is_empty(), "{csr:#?}");
        }
    }
    if let Some(findings) = diagnostic_fixture("server-function-directive") {
        // Two wrapped exports, one named re-export, one star re-export, one
        // wrapped default export; the direct function exports stay silent.
        assert_rule_findings(&findings, "server-function-module-directive", 5);
        assert!(
            findings.iter().all(|finding| {
                !finding["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("plain.ts"))
            }),
            "a module without the directive puts nothing at risk: {findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("server-function-rich-args") {
        // Date, Set, Map, RegExp (module-level directive), Float64Array, and
        // the Set of the mixed call; lone/trailing Uint8Array, plain JSON
        // shapes, and the unresolvable inline `new Date()` stay silent.
        assert_rule_findings(&findings, "server-function-rich-argument", 6);
        if let Some(enabled) = diagnostic_fixture("server-function-rich-args-enabled") {
            assert!(
                enabled.is_empty(),
                "enableRichArguments installs the codec and removes the throw everywhere: {enabled:#?}"
            );
        }
    }
    if let Some(findings) = diagnostic_fixture("resolve-scope") {
        // Memo compute, effect compute, createTrackedEffect, tracked JSX;
        // untrack, component body, event handler, apply, and module scope
        // are observer-free and stay silent (probed, rc.0).
        assert_rule_findings(&findings, "resolve-in-reactive-scope", 4);
        assert!(
            findings
                .iter()
                .all(|finding| finding["rule"] == "resolve-in-reactive-scope"),
            "{findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("uncalled-accessor-v2") {
        // The three positions TypeScript permits: a string-concatenation
        // operand, a unary operand, and a template interpolation. The typed
        // positions the 2026-08-17 narrowing dropped -- a class object value, a
        // native attribute, and a computed key -- are each a diagnostic of
        // TypeScript's own, and the children attribute and the called and
        // passed-on accessors were already silent.
        assert_rule_findings(&findings, "uncalled-accessor", 3);
        for position in ["binary operator", "unary operator", "template literal"] {
            assert!(
                findings.iter().any(|finding| {
                    finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains(position))
                }),
                "{position} is a position TypeScript permits and must stay reported: {findings:#?}"
            );
        }
        for typed in [
            "class object value",
            "native JSX attribute",
            "computed property access",
        ] {
            assert!(
                findings.iter().all(|finding| {
                    !finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains(typed))
                }),
                "{typed} is TypeScript's; reporting it duplicates a diagnostic: {findings:#?}"
            );
        }
    }
}

#[test]
fn broadened_rule_surfaces_pin_distinct_semantic_branches() {
    let Some(async_findings) = diagnostic_fixture("async-boundary") else {
        return;
    };
    assert!(
        async_findings.iter().all(|finding| {
            !finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("Good.tsx"))
        }),
        "nested Loading and inline pending observers must stay clean: {async_findings:#?}"
    );

    let Some(reactivity_findings) = diagnostic_fixture("shared-reactivity-v2") else {
        return;
    };
    for (rule, expected) in [
        // Two template interpolations. The third was a native JSX attribute,
        // which TypeScript rejects on its own (TS2322) and the 2026-08-17
        // narrowing dropped.
        ("uncalled-accessor", 2),
        // One: the reactive `props.onSave` read. The call-result arm went on
        // 2026-08-17 -- a handler expression proven non-callable, or a
        // non-callable accessor call, is TS2322 at that same attribute.
        ("expected-function-got-expression", 1),
        ("untracked-derived-function", 2),
    ] {
        assert_rule_findings(&reactivity_findings, rule, expected);
    }

    let Some(unresolved_findings) = diagnostic_fixture("static-api-unresolved") else {
        return;
    };
    // These six obligations asked whether the target carries the source
    // brand, and the brand is a type (`Refreshable<T>`), so TypeScript answers
    // the question outright: an unbranded target is TS2345, a branded one type
    // checks. The obligations were removed on 2026-08-17 and are pinned at 0;
    // the fixture keeps its unresolved targets so a reintroduction is caught
    // by the same cases that used to justify them.
    assert_rule_findings(&unresolved_findings, "refresh-target-unresolved", 0);
    assert_rule_findings(&unresolved_findings, "affects-target-unresolved", 0);
}

#[test]
fn static_violation_evidence_describes_the_actual_proof() {
    let Some(static_api) = diagnostic_fixture("static-api") else {
        return;
    };
    assert!(
        findings_for_rule(&static_api, "sync-node-received-async")
            .into_iter()
            .all(|finding| {
                finding["evidence"][0]["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("sync computation"))
            })
    );

    let Some(stylistic) = diagnostic_fixture("upstream-divergences") else {
        return;
    };
    assert!(
        findings_for_rule(&stylistic, "v1/prefer-show")
            .into_iter()
            .all(|finding| {
                finding["evidence"][0]["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("conditional JSX expression"))
            })
    );
}

#[test]
fn interprocedural_diagnostics_point_to_the_calling_component() {
    for (fixture, expected_count, message) in [
        ("interprocedural", 1, "readCount"),
        ("callback-forwarding", 1, "invoke"),
        ("polymorphic", 2, "readGeneric"),
        ("recursive", 1, "readA"),
        ("returned-closure", 1, "readCount"),
        ("store-flow", 1, "\"state.count\""),
        // Four: the class, object, and generic-function calls, plus
        // `invoke(objectReader, …)` whose receiver is exactly one object at
        // that site. The sibling `invoke(cond ? a : b, …)` stays silent.
        ("interprocedural-methods-v2", 4, "count"),
    ] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            return;
        };
        let strict = findings_for_rule(&findings, "strict-read-untracked");
        assert_eq!(
            strict.len(),
            expected_count,
            "fixture {fixture}: {findings:#?}"
        );
        assert!(strict.iter().any(|finding| {
            finding["message"]
                .as_str()
                .is_some_and(|text| text.contains(message))
        }));
        assert!(strict.iter().all(|finding| {
            finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("App.tsx"))
        }));
    }
}

#[test]
fn unknown_callback_diagnostic_contains_actionable_contract_stub() {
    let Some(findings) = diagnostic_fixture("package-unknown-callback-producer") else {
        return;
    };
    let callback_findings = findings_for_rule(&findings, "package-contract-callback-missing");
    assert_eq!(callback_findings.len(), 1, "{findings:#?}");
    let finding = &callback_findings[0];
    let message = finding["message"].as_str().unwrap_or_default();
    let hint = finding["hint"].as_str().unwrap_or_default();
    assert!(message.contains("current project.:schedule"), "{message}");
    assert!(message.contains("parameter 0 (() => void)"), "{message}");
    assert!(hint.contains("schemaVersion\":1"), "{hint}");
    assert!(hint.contains("choose exactly one audited mode"), "{hint}");
    assert!(hint.contains("solid-checker contract generate"), "{hint}");
}

#[test]
fn prefer_component_syntax_follows_conditional_cross_file_returns() {
    let Some(findings) = diagnostic_fixture("prefer-component-syntax-v2") else {
        return;
    };
    let preferred = findings_for_rule(&findings, "prefer-component-syntax");
    assert_eq!(preferred.len(), 1, "{findings:#?}");
    assert!(
        preferred[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("renderCard"))
    );
}

#[test]
fn control_flow_and_effect_phases_classify_strict_reads() {
    // control-flow: the two frozen Show-callback reads, plus the two frozen
    // reads under <For keyed={byId}> — a named key function proven callable
    // through type facts keeps the custom-key accessor claims. The dynamic
    // boolean `keyed={flag()}` functions contribute nothing: the callback
    // shape is ambiguous, so neither parameter is claimed as an accessor.
    for (fixture, expected) in [("control-flow", 4), ("execution-phases", 1)] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            return;
        };
        assert_eq!(
            findings_for_rule(&findings, "strict-read-untracked").len(),
            expected,
            "fixture {fixture}: {findings:#?}"
        );
    }
}
