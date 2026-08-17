#[path = "support/diagnostics.rs"]
mod support;

use std::collections::HashMap;

use support::{assert_rule_findings, diagnostic_fixture, findings_for_rule};

#[test]
fn eslint_plugin_solid_two_corpus_matches_native_rule_semantics() {
    let Some(findings) = diagnostic_fixture("eslint-plugin-corpus") else {
        return;
    };

    for (rule, count) in [
        ("reactive-write-in-owned-scope", 9),
        ("action-called-in-owned-scope", 2),
        ("strict-read-untracked", 25),
        ("reactive-read-after-await", 20),
        ("component-props-destructure", 1),
        ("component-returns-conditionally", 3),
        ("cleanup-in-forbidden-scope", 2),
        ("flush-in-forbidden-scope", 2),
        ("primitive-in-leaf-owner", 3),
    ] {
        assert_rule_findings(&findings, rule, count);
    }

    let expected = HashMap::from([
        (
            "owned-scope-invalid.tsx",
            [
                ("reactive-write-in-owned-scope", 3),
                ("action-called-in-owned-scope", 1),
            ]
            .as_slice(),
        ),
        (
            "effect-apply-invalid.tsx",
            [("strict-read-untracked", 3)].as_slice(),
        ),
        (
            "effect-apply-extended-invalid.tsx",
            [("strict-read-untracked", 5)].as_slice(),
        ),
        (
            "after-await-invalid.tsx",
            [("reactive-read-after-await", 3)].as_slice(),
        ),
        (
            "await-control-flow-invalid.tsx",
            [("reactive-read-after-await", 11)].as_slice(),
        ),
        (
            "after-await-extended-invalid.tsx",
            [("reactive-read-after-await", 6)].as_slice(),
        ),
        (
            "props-invalid.tsx",
            [
                ("strict-read-untracked", 3),
                ("component-props-destructure", 1),
            ]
            .as_slice(),
        ),
        (
            "control-flow-invalid.tsx",
            [("strict-read-untracked", 7)].as_slice(),
        ),
        (
            "props-extended-invalid.tsx",
            [("strict-read-untracked", 3)].as_slice(),
        ),
        (
            "component-return-invalid.tsx",
            [
                ("strict-read-untracked", 3),
                ("component-returns-conditionally", 3),
            ]
            .as_slice(),
        ),
        (
            "leaf-invalid.tsx",
            [
                ("cleanup-in-forbidden-scope", 1),
                ("flush-in-forbidden-scope", 1),
                ("primitive-in-leaf-owner", 3),
            ]
            .as_slice(),
        ),
        (
            "owned-leaf-extended-invalid.tsx",
            [
                ("reactive-write-in-owned-scope", 4),
                ("action-called-in-owned-scope", 1),
                ("cleanup-in-forbidden-scope", 1),
                ("flush-in-forbidden-scope", 1),
            ]
            .as_slice(),
        ),
        (
            "dynamic-tracking-invalid.tsx",
            [("reactive-write-in-owned-scope", 2)].as_slice(),
        ),
    ]);

    for (file, rules) in expected {
        for (rule, count) in rules {
            let actual = findings_for_rule(&findings, rule)
                .into_iter()
                .filter(|finding| {
                    finding["primaryLocation"]["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with(file))
                })
                .count();
            assert_eq!(actual, *count, "{file} / {rule}: {findings:#?}");
        }
    }

    // The corpus files are mechanically extracted from upstream test cases,
    // so a `-valid.tsx` file is valid only for the one upstream rule it was
    // extracted for. The extraction strips the owning component, leaving
    // module-scope effects (real leaks: nothing ever disposes them),
    // returned cleanups the analyzer cannot resolve, and setup-scope reads
    // that register no dependency. Those incidental findings are deliberate;
    // this pins them so a false-positive regression moves an assertion
    // instead of hiding in an unasserted snapshot.
    //
    // The `cleanup-return-unresolved` entries are pinned at 0 rather than
    // deleted: this is the noise that rule produced on *valid* upstream code,
    // and three of those obligations sat on the legal `{ effect, error }`
    // bundle form. Keeping the assertion means reintroducing the rule fails
    // here instead of quietly returning.
    let incidental = HashMap::from([
        (
            "after-await-valid.tsx",
            [("cleanup-return-unresolved", 0)].as_slice(),
        ),
        (
            "component-return-valid.tsx",
            [("strict-read-untracked", 1)].as_slice(),
        ),
        (
            "effect-apply-valid.tsx",
            [("no-owner-effect", 3), ("cleanup-return-unresolved", 0)].as_slice(),
        ),
        (
            "effect-apply-extended-valid.tsx",
            [("no-owner-effect", 4), ("cleanup-return-unresolved", 0)].as_slice(),
        ),
        ("leaf-valid.tsx", [("no-owner-effect", 2)].as_slice()),
        (
            "owned-leaf-extended-valid.tsx",
            [
                ("no-owner-settled-cleanup", 1),
                // The module-scope createTrackedEffect that pins leaf-scope
                // write legality is itself an undisposed effect.
                ("no-owner-effect", 1),
            ]
            .as_slice(),
        ),
        (
            "owned-scope-valid.tsx",
            [("no-owner-effect", 1), ("cleanup-return-unresolved", 0)].as_slice(),
        ),
    ]);
    for (file, rules) in incidental {
        for (rule, count) in rules {
            let actual = findings_for_rule(&findings, rule)
                .into_iter()
                .filter(|finding| {
                    finding["primaryLocation"]["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with(file))
                })
                .count();
            assert_eq!(actual, *count, "incidental {file} / {rule}: {findings:#?}");
        }
    }

    for (file, rules) in [
        (
            "owned-scope-valid.tsx",
            &[
                "reactive-write-in-owned-scope",
                "action-called-in-owned-scope",
            ][..],
        ),
        ("effect-apply-valid.tsx", &["strict-read-untracked"][..]),
        (
            "effect-apply-extended-valid.tsx",
            &["strict-read-untracked"][..],
        ),
        ("after-await-valid.tsx", &["reactive-read-after-await"][..]),
        (
            "await-control-flow-valid.tsx",
            &["reactive-read-after-await"][..],
        ),
        (
            "after-await-extended-valid.tsx",
            &["reactive-read-after-await"][..],
        ),
        (
            "props-valid.tsx",
            &["strict-read-untracked", "component-props-destructure"][..],
        ),
        (
            "control-flow-valid.tsx",
            &["strict-read-untracked", "component-props-destructure"][..],
        ),
        (
            "props-extended-valid.tsx",
            &["strict-read-untracked", "component-props-destructure"][..],
        ),
        (
            "component-return-valid.tsx",
            &["component-returns-conditionally"][..],
        ),
        (
            "leaf-valid.tsx",
            &[
                "cleanup-in-forbidden-scope",
                "flush-in-forbidden-scope",
                "primitive-in-leaf-owner",
            ][..],
        ),
        (
            "owned-leaf-extended-valid.tsx",
            &[
                "reactive-write-in-owned-scope",
                "action-called-in-owned-scope",
                "cleanup-in-forbidden-scope",
                "flush-in-forbidden-scope",
                "primitive-in-leaf-owner",
            ][..],
        ),
        (
            "dynamic-tracking-valid.tsx",
            &["strict-read-untracked", "reactive-write-in-owned-scope"][..],
        ),
    ] {
        for rule in rules {
            assert!(
                findings_for_rule(&findings, rule)
                    .into_iter()
                    .all(|finding| {
                        !finding["primaryLocation"]["path"]
                            .as_str()
                            .is_some_and(|path| path.ends_with(file))
                    }),
                "unexpected {file} / {rule}: {findings:#?}"
            );
        }
    }
}
