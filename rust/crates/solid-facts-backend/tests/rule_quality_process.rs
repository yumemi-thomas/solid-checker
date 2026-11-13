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
        // rc.17's `createOptimistic` setter meets no owned-scope guard, so
        // `owned-leaf-extended-invalid.tsx`'s optimistic write is legal there.
        // ADR 0613: six more, the uncertifiable apply writes pinned below.
        ("no-write-in-pure-computation", 16),
        ("action-called-in-owned-scope", 2),
        // Review round 4: two zero-parameter `Show` children are tracked
        // inserts, not strict-read callbacks.
        // Two of them are uncertifiable reads in the closures the two `nested`
        // memos return (ADR 0452), pinned below. One more since ADR 0535,
        // pinned as incidental below. ADR 0920: one more, the cleanup a named
        // apply returns, pinned below.
        ("strict-read-untracked", 22),
        // ADR 0516: one is the uncertifiable read after `loopAwait`'s loop,
        // pinned below. ADR 0870: two more, the reads after
        // `conditionalAwait`'s and `shortCircuitAwait`'s skippable awaits.
        ("reactive-read-after-await", 22),
        // ADR 0294: the exported `install(read: Accessor<number>)` shapes.
        // ADR 0646: two more, `owned-scope-valid.tsx:28`, a tuple's function
        // slots from a local hook: its transport does not resolve.
        // ADR 0692: eight more, the census's internal uncertifiables, pinned
        // exactly below (`census_rows`).
        ("reactive-dispatch-unresolved", 13),
        ("no-destructure", 1),
        ("components-return-once", 3),
        // ADR 0966 (batch 11, slice 5): two more, the uncertifiable
        // `leaf-operation-unaccounted` rows at the two `flush` calls in
        // `owned-scope-valid.tsx`'s memo body, pinned below. No violation moved.
        // ADR 0969 (batch 11, slice 6): six more, the uncertifiable `leaf-operation-unaccounted` rows of the owner-call universe at a creation in an exported
        // function of this open program (every export is handed: B3), in `after-await-valid.tsx`, `await-control-flow-valid.tsx`, `effect-apply-valid.tsx`,
        // `effect-apply-invalid.tsx`, `effect-apply-extended-invalid.tsx` and `props-valid.tsx`, pinned below. No violation moved.
        // ADR 0971 (batch 11, B2): 21 more, the uncertifiable `leaf-operation-unaccounted` rows at the JSX creation sites (a component element, an
        // expression container's insert) of the corpus's exported functions, in seven files, pinned per file below. No violation moved.
        ("leaf-owner-forbidden-call", 36),
    ] {
        assert_rule_findings(&findings, rule, count);
    }

    let expected = HashMap::from([
        (
            "owned-scope-invalid.tsx",
            [
                ("no-write-in-pure-computation", 3),
                ("action-called-in-owned-scope", 1),
            ]
            .as_slice(),
        ),
        (
            "effect-apply-invalid.tsx",
            [
                ("strict-read-untracked", 2),
                ("reactive-dispatch-unresolved", 1),
            ]
            .as_slice(),
        ),
        (
            "effect-apply-extended-invalid.tsx",
            [
                ("strict-read-untracked", 4),
                ("reactive-dispatch-unresolved", 2),
            ]
            .as_slice(),
        ),
        (
            "after-await-invalid.tsx",
            [
                ("reactive-read-after-await", 2),
                ("reactive-dispatch-unresolved", 2),
            ]
            .as_slice(),
        ),
        (
            "await-control-flow-invalid.tsx",
            [("reactive-read-after-await", 11)].as_slice(),
        ),
        (
            "after-await-extended-invalid.tsx",
            [("reactive-read-after-await", 6)].as_slice(),
        ),
        // ADR 0971: each file's JSX creation sites (`leaf-operation-unaccounted`, uncertifiable; checked below).
        (
            "props-invalid.tsx",
            [
                ("strict-read-untracked", 3),
                ("no-destructure", 1),
                ("leaf-owner-forbidden-call", 1),
            ]
            .as_slice(),
        ),
        (
            "control-flow-invalid.tsx",
            [
                ("strict-read-untracked", 5),
                ("leaf-owner-forbidden-call", 6),
            ]
            .as_slice(),
        ),
        (
            "props-extended-invalid.tsx",
            [
                ("strict-read-untracked", 3),
                ("leaf-owner-forbidden-call", 1),
            ]
            .as_slice(),
        ),
        (
            "component-return-invalid.tsx",
            [("components-return-once", 3)].as_slice(),
        ),
        (
            "leaf-invalid.tsx",
            [
                ("leaf-owner-forbidden-call", 5),
                ("reactive-dispatch-unresolved", 2),
            ]
            .as_slice(),
        ),
        (
            "owned-leaf-extended-invalid.tsx",
            [
                ("no-write-in-pure-computation", 3),
                ("action-called-in-owned-scope", 1),
                ("leaf-owner-forbidden-call", 2),
                ("reactive-dispatch-unresolved", 1),
            ]
            .as_slice(),
        ),
        (
            "dynamic-tracking-invalid.tsx",
            [("no-write-in-pure-computation", 2)].as_slice(),
        ),
    ]);

    // ADR 0692: the census's rows, exactly (file, line, blocker). Each is an
    // internal uncertifiable; no violation moved.
    let mut census_rows = findings_for_rule(&findings, "reactive-dispatch-unresolved")
        .into_iter()
        .filter(|finding| {
            finding["analysisContext"]
                .as_str()
                .is_some_and(|context| context.starts_with("value-"))
        })
        .map(|finding| {
            assert_eq!(finding["kind"], "uncertifiable", "{finding:#?}");
            let path = finding["primaryLocation"]["path"]
                .as_str()
                .unwrap_or_default();
            (
                std::path::Path::new(path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_owned(),
                finding["primaryLocation"]["line"]
                    .as_u64()
                    .unwrap_or_default(),
                finding["analysisContext"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect::<Vec<_>>();
    census_rows.sort();
    let pinned = [
        ("after-await-invalid.tsx", 22, "value-forwarded"),
        (
            "effect-apply-extended-invalid.tsx",
            17,
            "value-from-primitive-callback",
        ),
        ("leaf-invalid.tsx", 12, "value-from-primitive-callback"),
        ("leaf-invalid.tsx", 14, "value-from-primitive-callback"),
        ("leaf-valid.tsx", 9, "value-to-unresolved-callee"),
        ("owned-leaf-extended-invalid.tsx", 14, "value-in-carrier"),
        ("owned-leaf-extended-valid.tsx", 19, "value-in-carrier"),
        ("owned-leaf-extended-valid.tsx", 31, "value-in-carrier"),
    ]
    .map(|(file, line, context)| (file.to_owned(), line, context.to_owned()));
    assert_eq!(census_rows, pinned, "census rows: {findings:#?}");

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
        // ADR 0971: the JSX creation site of the returned expression (`leaf-operation-unaccounted`, uncertifiable; checked below).
        (
            "component-return-valid.tsx",
            [
                ("strict-read-untracked", 1),
                ("leaf-owner-forbidden-call", 1),
            ]
            .as_slice(),
        ),
        // ADR 0971: the JSX creation sites of these files (`leaf-operation-unaccounted`, uncertifiable; checked below). The match is a suffix:
        // `control-flow-valid.tsx` is seven rows of its own and the one of `await-control-flow-valid.tsx` (ADR 0969).
        (
            "control-flow-valid.tsx",
            [("leaf-owner-forbidden-call", 8)].as_slice(),
        ),
        (
            "props-extended-valid.tsx",
            [("leaf-owner-forbidden-call", 2)].as_slice(),
        ),
        (
            "props-valid.tsx",
            [("leaf-owner-forbidden-call", 4)].as_slice(),
        ),
        // ADR 0452: the closure an async memo returns is built in the memo, so
        // its read has no placement (an uncertifiable result, no violation).
        (
            "after-await-extended-valid.tsx",
            [("strict-read-untracked", 1)].as_slice(),
        ),
        // ADR 0516: the read after `loopAwait`'s `while` loop may follow an
        // await the loop ran (an uncertifiable result, no violation). ADR
        // 0870: so may the reads after `conditionalAwait`'s `if` and
        // `shortCircuitAwait`'s `&&` await: wrong when the branch awaits.
        (
            "await-control-flow-valid.tsx",
            [
                ("strict-read-untracked", 1),
                ("reactive-read-after-await", 3),
            ]
            .as_slice(),
        ),
        (
            "effect-apply-valid.tsx",
            [
                // Three, and since ADR 0534 the exported `install`'s effect,
                // whose callers are unknown: an obligation.
                ("missing-owner", 4),
                ("cleanup-return-unresolved", 0),
                // ADR 0535: the cleanup the apply returns (`later`) runs on a
                // rerun, where rc.17 can warn (the fuzzer's render-cleanup-rerun
                // cell): an obligation, never a violation.
                ("strict-read-untracked", 1),
            ]
            .as_slice(),
        ),
        (
            "effect-apply-extended-valid.tsx",
            [
                ("missing-owner", 4),
                ("cleanup-return-unresolved", 0),
                // ADR 0613: the corpus's own `flush()` calls (a component
                // body, a memo) may drain an apply under an owner, so its
                // writes are uncertifiable, not legal.
                ("no-write-in-pure-computation", 4),
                // ADR 0920: `click`, the cleanup the named `applyLater`
                // returns, runs on a rerun in the apply's window (ADR 0539):
                // an obligation, as `effect-apply-valid.tsx`'s `later`.
                ("strict-read-untracked", 1),
            ]
            .as_slice(),
        ),
        (
            "leaf-valid.tsx",
            [("missing-owner", 2), ("reactive-dispatch-unresolved", 1)].as_slice(),
        ),
        (
            "owned-leaf-extended-valid.tsx",
            [
                // One unowned settled cleanup plus the module-scope
                // createTrackedEffect that pins leaf-scope write legality.
                ("missing-owner", 2),
                // ADR 0692: `refresh(value)` twice, census rows.
                ("reactive-dispatch-unresolved", 2),
            ]
            .as_slice(),
        ),
        (
            "owned-scope-valid.tsx",
            [
                ("missing-owner", 1),
                ("cleanup-return-unresolved", 0),
                // Not incidental: a deliberate divergence from upstream, which
                // lists `flush(fn)` inside a memo as valid. `flush` keeps the
                // memo's owner, so the inline write and the `flushWrite` it is
                // passed throw REACTIVE_WRITE_IN_OWNED_SCOPE on every
                // rc.0-rc.9 dev build (ownership case
                // no-write-in-pure-computation/positive/002). Two more,
                // uncertifiable: an apply's write and a module-scope
                // `onSettled`'s, which that flush may drain (ADR 0613).
                ("no-write-in-pure-computation", 4),
                // ADR 0966: the two `flush` calls in the memo body. The
                // program runs a callback under a leaf owner (`onSettled`)
                // and no census class places the calls off one: uncertifiable
                // rows, never a violation (pinned below).
                ("leaf-owner-forbidden-call", 2),
            ]
            .as_slice(),
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

    // ADR 0966, 0969, 0971: the leaf rows of these files are internal uncertifiables (`leaf-operation-unaccounted`: the flush accounting's, the owner-call
    // universe's, the JSX creation sites'), none a violation; the violations are the two `leaf-invalid` and `owned-leaf-extended-invalid` files'.
    for finding in findings_for_rule(&findings, "leaf-owner-forbidden-call")
        .into_iter()
        .filter(|finding| {
            finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| {
                    [
                        "owned-scope-valid.tsx",
                        "after-await-valid.tsx",
                        "await-control-flow-valid.tsx",
                        "effect-apply-valid.tsx",
                        "effect-apply-invalid.tsx",
                        "effect-apply-extended-invalid.tsx",
                        "props-valid.tsx",
                        // ADR 0971: the JSX creation sites.
                        "component-return-valid.tsx",
                        "control-flow-invalid.tsx",
                        "control-flow-valid.tsx",
                        "props-extended-invalid.tsx",
                        "props-extended-valid.tsx",
                        "props-invalid.tsx",
                    ]
                    .iter()
                    .any(|file| path.ends_with(file))
                })
        })
    {
        assert_eq!(finding["kind"], "uncertifiable", "{finding:#?}");
        assert_eq!(
            finding["analysisContext"], "leaf-operation-unaccounted",
            "{finding:#?}"
        );
    }

    for (file, rules) in [
        // SC2001 is pinned at two above: the `flush(fn)` divergence.
        (
            "owned-scope-valid.tsx",
            &["action-called-in-owned-scope"][..],
        ),
        ("after-await-valid.tsx", &["reactive-read-after-await"][..]),
        (
            "after-await-extended-valid.tsx",
            &["reactive-read-after-await"][..],
        ),
        (
            "props-valid.tsx",
            &["strict-read-untracked", "no-destructure"][..],
        ),
        (
            "control-flow-valid.tsx",
            &["strict-read-untracked", "no-destructure"][..],
        ),
        (
            "props-extended-valid.tsx",
            &["strict-read-untracked", "no-destructure"][..],
        ),
        (
            "component-return-valid.tsx",
            &["components-return-once"][..],
        ),
        (
            "leaf-valid.tsx",
            &[
                "leaf-owner-forbidden-call",
                "leaf-owner-forbidden-call",
                "leaf-owner-forbidden-call",
            ][..],
        ),
        (
            "owned-leaf-extended-valid.tsx",
            &[
                "no-write-in-pure-computation",
                "action-called-in-owned-scope",
                "leaf-owner-forbidden-call",
                "leaf-owner-forbidden-call",
                "leaf-owner-forbidden-call",
            ][..],
        ),
        (
            "dynamic-tracking-valid.tsx",
            &["strict-read-untracked", "no-write-in-pure-computation"][..],
        ),
    ] {
        for rule in rules {
            assert!(
                findings_for_rule(&findings, rule)
                    .into_iter()
                    .all(|finding| {
                        // The file name, not a suffix: `await-control-flow-valid.tsx`
                        // ends with `control-flow-valid.tsx`.
                        !finding["primaryLocation"]["path"]
                            .as_str()
                            .is_some_and(|path| {
                                std::path::Path::new(path)
                                    .file_name()
                                    .is_some_and(|name| name == file)
                            })
                    }),
                "unexpected {file} / {rule}: {findings:#?}"
            );
        }
    }
}
