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
                ("invalid-affects-target", 1),
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
                ("settled-cleanup-unowned", 2),
            ],
        ),
        (
            "async-boundary",
            &[
                ("pending-async-untracked-read", 1),
                ("pending-async-forbidden-scope", 1),
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
