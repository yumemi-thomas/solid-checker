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
