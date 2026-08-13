#[path = "support/diagnostics.rs"]
mod support;

use support::{assert_rule_findings, diagnostic_fixture, findings_for_rule};

#[test]
fn write_scope_diagnostics_have_semantic_locations() {
    let Some(findings) = diagnostic_fixture("write-scope") else {
        return;
    };
    assert_eq!(
        (
            findings_for_rule(&findings, "reactive-write-in-owned-scope").len(),
            findings_for_rule(&findings, "action-called-in-owned-scope").len(),
        ),
        (13, 3)
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
                ("cleanup-in-forbidden-scope", 3),
                ("primitive-in-leaf-owner", 3),
                ("flush-in-forbidden-scope", 2),
                ("invalid-cleanup-return", 6),
            ][..],
        ),
        (
            "static-api",
            &[
                ("missing-effect-function", 2),
                ("sync-node-received-async", 6),
                ("invalid-refresh-target", 2),
                ("invalid-affects-target", 2),
                ("affects-keys-on-accessor", 2),
                ("reactive-write-in-owned-scope", 1),
            ],
        ),
        (
            "directive-phases",
            &[
                ("reactive-write-in-owned-scope", 1),
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
                ("pending-async-untracked-read", 2),
                ("pending-async-forbidden-scope", 2),
                ("async-outside-loading-boundary", 11),
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
    assert_rule_findings(&unresolved_findings, "refresh-target-unresolved", 2);
    assert_rule_findings(&unresolved_findings, "affects-target-unresolved", 2);
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
fn control_flow_and_effect_phases_classify_strict_reads() {
    for (fixture, expected) in [("control-flow", 2), ("execution-phases", 1)] {
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
