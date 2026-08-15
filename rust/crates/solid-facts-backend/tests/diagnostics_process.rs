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
                // Three owner-backed violations plus the exported-helper
                // proof obligation; the out-of-band (event handler) onSettled
                // contributes nothing to any SC3xxx rule.
                ("cleanup-in-forbidden-scope", 4),
                ("primitive-in-leaf-owner", 3),
                ("flush-in-forbidden-scope", 2),
                ("invalid-cleanup-return", 6),
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
                // Wrapper, literal, zero-arg, value-form store, value-form
                // store child record, and a member chain on an accessor.
                // refresh(target, extra) is runtime-legal and not counted.
                ("invalid-refresh-target", 6),
                ("invalid-affects-target", 2),
                ("affects-keys-on-accessor", 2),
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
    // component-body ones stay violations.
    let kinds = cleanup
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "violation").count(),
        3,
        "{cleanup:#?}"
    );
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "uncertifiable").count(),
        1,
        "{cleanup:#?}"
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
        kinds.iter().filter(|kind| **kind == "uncertifiable").count(),
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
        ("uncalled-accessor", 3),
        ("expected-function-got-expression", 2),
        ("untracked-derived-function", 2),
    ] {
        assert_rule_findings(&reactivity_findings, rule, expected);
    }

    let Some(unresolved_findings) = diagnostic_fixture("static-api-unresolved") else {
        return;
    };
    // Two identifier targets each (a plain object and an untraceable
    // parameter), plus one member-chain target each whose root is not a
    // proven source: `refresh(plain.value)` and `affects(state.user, "name")`
    // on a parameter — unresolved, never proven-invalid.
    assert_rule_findings(&unresolved_findings, "refresh-target-unresolved", 3);
    assert_rule_findings(&unresolved_findings, "affects-target-unresolved", 3);
}

#[test]
fn static_violation_evidence_describes_the_actual_proof() {
    let Some(static_api) = diagnostic_fixture("static-api") else {
        return;
    };
    assert!(
        findings_for_rule(&static_api, "invalid-affects-target")
            .into_iter()
            .all(|finding| {
                finding["evidence"][0]["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("affects call"))
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
