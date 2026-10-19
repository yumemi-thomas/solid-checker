#[path = "support/diagnostics.rs"]
mod support;

// ADR 0296: retained dependency-source checks against one-shot ones.
#[path = "dependency_daemon_process.rs"]
mod dependency_daemon_process;

// ADR 0311: a dependency-source check is a fresh one-shot check.
#[path = "dependency_fresh_native_process.rs"]
mod dependency_fresh_native_process;

// The ordinary check consumes no package document, and the flags that once
// supplied one are refused.
#[path = "removed_package_flags_process.rs"]
mod removed_package_flags_process;

use support::{assert_rule_findings, diagnostic_fixture, findings_for_rule};

#[test]
fn conditional_package_getters_require_every_alternative() {
    let Ok(producer) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", producer)
        .env("SOLID_CHECKER_DAEMON", "0")
        .args([
            "--format",
            "json",
            "--runtime-target",
            "browser",
            "--runtime-build",
            "development",
            "--dependency-source",
            "dep-router",
            "--project",
        ])
        .arg(root.join("fixtures/reactive-ir/dependency-source-integrity-getter/tsconfig.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let source = include_str!(
        "../../../../fixtures/reactive-ir/dependency-source-integrity-getter/Conditional.tsx"
    );
    let rows = report["findings"].as_array().unwrap();
    for (name, expected) in [
        ("BothKnown", Some(("SC1001", "violation"))),
        ("UnknownBranch", Some(("SC9012", "uncertifiable"))),
        ("InertBranch", Some(("SC9012", "uncertifiable"))),
        ("TrackedConditional", None),
        ("DestructuredObject", Some(("SC1001", "violation"))),
        ("RenamedObject", Some(("SC1001", "violation"))),
        ("MutableObject", Some(("SC9012", "uncertifiable"))),
        ("TrackedObject", None),
    ] {
        let function = source.find(&format!("export function {name}(")).unwrap();
        let expression = if name == "RenamedObject" {
            "current.search"
        } else {
            "location.search"
        };
        let site = function + source[function..].find(expression).unwrap();
        let selected = rows
            .iter()
            .filter(|row| {
                row["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|p| p.ends_with("/Conditional.tsx"))
                    && row["primaryLocation"]["startByte"].as_u64() == Some(site as u64)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            selected.len(),
            usize::from(expected.is_some()),
            "{name}: {selected:#?}"
        );
        if let Some((code, kind)) = expected {
            assert_eq!(selected[0]["id"], code, "{name}");
            assert_eq!(selected[0]["kind"], kind, "{name}");
        }
    }
}

#[test]
fn immediate_leaf_helpers_follow_bodies_and_async_prefixes() {
    let Some(findings) = diagnostic_fixture("leaf-scope-helper-completion") else {
        return;
    };
    let source =
        include_str!("../../../../fixtures/reactive-ir/leaf-scope-helper-completion/Immediate.tsx");
    for (name, expected) in [
        ("CleanImmediate", None),
        ("ForbiddenImmediate", Some(("SC3001", "violation"))),
        ("AsyncPrefix", Some(("SC3001", "violation"))),
        ("ConditionalImmediate", Some(("SC3001", "uncertifiable"))),
        ("KnownArgument", None),
        ("KnownTrueArgument", Some(("SC3001", "violation"))),
        ("SpreadArgument", Some(("SC3001", "uncertifiable"))),
        ("StableTransportFalse", None),
        ("StableTransportTrue", Some(("SC3001", "violation"))),
        ("StableTransportSpread", Some(("SC3001", "uncertifiable"))),
        ("StableMemberReceiver", Some(("SC3001", "uncertifiable"))),
        ("UnknownImmediate", Some(("SC9012", "uncertifiable"))),
        ("DefaultImmediate", Some(("SC9012", "uncertifiable"))),
        ("ConditionalSuspension", Some(("SC3001", "uncertifiable"))),
        ("OpaqueSelection", Some(("SC9012", "uncertifiable"))),
        ("WrappedImmediate", Some(("SC3001", "violation"))),
    ] {
        let start = source.find(&format!("export function {name}(")).unwrap();
        let end = source[start + 1..]
            .find("export function ")
            .map_or(source.len(), |offset| start + 1 + offset);
        let rows = findings
            .iter()
            .filter(|row| {
                row["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/Immediate.tsx"))
                    && row["primaryLocation"]["startByte"]
                        .as_u64()
                        .is_some_and(|byte| (start as u64..end as u64).contains(&byte))
                    && matches!(row["id"].as_str(), Some("SC3001" | "SC9012"))
            })
            .collect::<Vec<_>>();
        if let Some((code, kind)) = expected {
            let count = if name == "DefaultImmediate" { 2 } else { 1 };
            assert_eq!(rows.len(), count, "{name}: {rows:#?}");
            assert_eq!(rows[0]["id"], code, "{name}");
            assert_eq!(rows[0]["kind"], kind, "{name}");
            if name == "WrappedImmediate" {
                let begin = rows[0]["primaryLocation"]["startByte"].as_u64().unwrap() as usize;
                let end = rows[0]["primaryLocation"]["endByte"].as_u64().unwrap() as usize;
                assert!(source[begin..end].contains("as () => void"));
                continue;
            }
            if name == "DefaultImmediate" {
                assert_eq!(rows[1]["id"], "SC9012");
                assert_eq!(rows[1]["kind"], "uncertifiable");
                assert_eq!(
                    rows[1]["primaryLocation"]["startByte"],
                    (start + source[start..end].find("unknown()").unwrap()) as u64
                );
            }
            let callee = match name {
                "StableTransportFalse" | "StableTransportTrue" | "StableTransportSpread" => {
                    start + source[start..end].find("register(").unwrap()
                }
                "StableMemberReceiver" => {
                    start + source[start..end].find("target.register(").unwrap()
                }
                _ => {
                    let anchor = source[start..end]
                        .find("    (")
                        .or_else(|| source[start..end].find("    void ("))
                        .unwrap();
                    let line = &source[start + anchor..end];
                    start + anchor + line.find('(').unwrap() + 1
                }
            };
            assert_eq!(
                rows[0]["primaryLocation"]["startByte"], callee as u64,
                "{name}"
            );
        } else {
            assert!(rows.is_empty(), "{name}: {rows:#?}");
        }
    }
}

#[test]
fn leaf_scopes_follow_exact_source_objects_without_trusting_receivers() {
    let Some(findings) = diagnostic_fixture("leaf-source-call-targets") else {
        return;
    };
    let source = include_str!("../../../../fixtures/reactive-ir/leaf-source-call-targets/App.tsx");
    for (method, expected) in [
        ("cleanLeaf", None),
        ("arrowLeaf", None),
        ("forbiddenLeaf", Some(("SC3001", "violation"))),
        ("nestedLeaf", Some(("SC3001", "violation"))),
        ("conditionalLeaf", Some(("SC3001", "uncertifiable"))),
        ("defaultLeaf", Some(("SC9012", "uncertifiable"))),
    ] {
        let start = source.find(&format!("service.{method}(")).unwrap();
        let rows = findings
            .iter()
            .filter(|row| {
                row["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/App.tsx"))
                    && row["primaryLocation"]["startByte"].as_u64() == Some(start as u64)
            })
            .collect::<Vec<_>>();
        if let Some((code, kind)) = expected {
            assert_eq!(rows.len(), 1, "{method}: {rows:#?}");
            assert_eq!(rows[0]["id"], code, "{method}");
            assert_eq!(rows[0]["kind"], kind, "{method}");
        } else {
            assert!(rows.is_empty(), "{method}: {rows:#?}");
        }
    }
    let selected = source.rfind("cluster.selected()").unwrap() as u64;
    assert!(findings.iter().any(|row| {
        row["id"] == "SC1001"
            && row["kind"] == "violation"
            && row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/App.tsx"))
            && row["primaryLocation"]["startByte"].as_u64() == Some(selected)
    }));
    let findings = diagnostic_fixture("leaf-source-call-targets-negative").unwrap();
    assert!(
        findings
            .iter()
            .all(|row| row["id"] != "SC3001" || row["kind"] != "violation"),
        "a constructor assertion cannot make the method body execute: {findings:#?}"
    );
    let source =
        include_str!("../../../../fixtures/reactive-ir/leaf-source-call-targets-negative/App.tsx");
    for method in [
        "receiverLeaf",
        "mutableLeaf",
        "escapedLeaf",
        "untouchedLeaf",
    ] {
        let start = source.find(&format!("service.{method}(")).unwrap() as u64;
        assert!(
            findings.iter().any(|row| row["id"] == "SC9012"
                && row["kind"] == "uncertifiable"
                && row["primaryLocation"]["path"].as_str().is_some_and(
                    |path| path.ends_with("/leaf-source-call-targets-negative/App.tsx")
                )
                && row["primaryLocation"]["startByte"].as_u64() == Some(start)),
            "{method}: {findings:#?}"
        );
    }
}

#[test]
fn merged_global_timers_keep_audited_timing_without_trusting_lookalikes() {
    let Some(findings) = diagnostic_fixture("leaf-scope-host-callbacks") else {
        return;
    };
    let source = include_str!(
        "../../../../fixtures/reactive-ir/leaf-scope-host-callbacks/AugmentedTimers.tsx"
    );
    let rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/AugmentedTimers.tsx"))
        })
        .collect::<Vec<_>>();
    let native_end = source.find("export function OpaqueTimer").unwrap() as u64;
    assert!(
        rows.iter()
            .all(|row| row["primaryLocation"]["startByte"].as_u64().unwrap() >= native_end),
        "native timer callbacks run on a fresh stack: {rows:#?}"
    );
    let opaque = source.find("    unknownTimer(").unwrap() + 4;
    let shadow = source.rfind("    setInterval(").unwrap() + 4;
    for (start, code, kind) in [
        (opaque, "SC9012", "uncertifiable"),
        (shadow, "SC3001", "violation"),
    ] {
        let row = rows
            .iter()
            .find(|row| {
                row["id"] == code
                    && row["primaryLocation"]["startByte"].as_u64() == Some(start as u64)
            })
            .unwrap_or_else(|| panic!("missing {code} at {start}: {rows:#?}"));
        assert_eq!(row["kind"], kind);
    }
}

#[test]
fn eager_read_summaries_require_exact_fresh_stack_global_bindings() {
    let Some(findings) = diagnostic_fixture("accessor-provenance") else {
        return;
    };
    let source =
        include_str!("../../../../fixtures/reactive-ir/accessor-provenance/RuntimeStack.tsx");
    for (name, uncertain) in [
        ("Timer", false),
        ("Microtask", false),
        ("Wrapped", false),
        ("Helper", false),
        ("NestedInline", false),
        ("NestedOpaque", true),
        ("Inline", true),
        ("Listener", true),
        ("SynchronousThenable", true),
        ("ReceiverCatch", true),
        ("Unknown", true),
        ("Shadowed", true),
        ("ScheduledAndReturned", true),
    ] {
        let start = source.find(&format!("export function {name}(")).unwrap();
        let end = source[start..]
            .find("\n\n")
            .map_or(source.len(), |offset| start + offset);
        let rows = findings
            .iter()
            .filter(|row| {
                row["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/RuntimeStack.tsx"))
                    && row["primaryLocation"]["startByte"]
                        .as_u64()
                        .is_some_and(|byte| (start as u64..end as u64).contains(&byte))
            })
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), usize::from(uncertain), "{name}: {rows:#?}");
        if uncertain {
            assert_eq!(rows[0]["id"], "SC1001", "{name}");
            assert_eq!(rows[0]["kind"], "uncertifiable", "{name}");
        }
    }
}

#[test]
fn factory_read_summaries_respect_dialect_callback_boundaries() {
    let Some(findings) = diagnostic_fixture("accessor-provenance") else {
        return;
    };
    let source = include_str!("../../../../fixtures/reactive-ir/accessor-provenance/Scope.tsx");
    for (name, expected) in [
        ("MemoAccessor", None),
        ("MemoClosure", None),
        ("MemoHelper", None),
        ("MemoReturned", None),
        ("NamespaceMemo", None),
        ("MemoArray", None),
        ("EffectCompute", None),
        ("SettledRead", None),
        ("ShadowMemo", Some("uncertifiable")),
        ("EagerRead", Some("violation")),
        ("UnknownRead", Some("uncertifiable")),
        ("RenderApply", Some("uncertifiable")),
    ] {
        let start = source.find(&format!("export function {name}(")).unwrap();
        let end = source[start + 1..].find("\n}").unwrap() + start + 3;
        let rows = findings
            .iter()
            .filter(|row| {
                row["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/Scope.tsx"))
                    && row["primaryLocation"]["startByte"]
                        .as_u64()
                        .is_some_and(|byte| (start as u64..end as u64).contains(&byte))
            })
            .collect::<Vec<_>>();
        if let Some(kind) = expected {
            assert_eq!(rows.len(), 1, "{name}: {rows:#?}");
            assert_eq!(rows[0]["id"], "SC1001", "{name}");
            assert_eq!(rows[0]["kind"], kind, "{name}");
        } else {
            assert!(rows.is_empty(), "{name}: {rows:#?}");
        }
    }
}

#[test]
fn anonymous_summary_changes_preserve_captured_and_unknown_callback_obligations() {
    let Some(findings) = diagnostic_fixture("accessor-provenance") else {
        return;
    };
    let source = include_str!("../../../../fixtures/reactive-ir/accessor-provenance/Anonymous.tsx");
    for (name, expected) in [
        ("InvokedInline", Some(("SC1001", "violation"))),
        ("CapturedOnly", None),
        ("InvokedReturned", Some(("SC1001", "violation"))),
        ("UnknownConsumer", Some(("SC1001", "uncertifiable"))),
        ("UnknownFactoryCall", Some(("SC1001", "uncertifiable"))),
        (
            "CapturedParameterDispatch",
            Some(("SC9012", "uncertifiable")),
        ),
    ] {
        let start = source.find(&format!("export function {name}(")).unwrap();
        let end = source[start + 1..]
            .find("\n}")
            .map_or(source.len(), |offset| start + 1 + offset + 2);
        let rows = findings
            .iter()
            .filter(|row| {
                row["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/Anonymous.tsx"))
                    && row["primaryLocation"]["startByte"]
                        .as_u64()
                        .is_some_and(|byte| (start as u64..end as u64).contains(&byte))
            })
            .collect::<Vec<_>>();
        if let Some((code, kind)) = expected {
            assert_eq!(rows.len(), 1, "{name}: {rows:#?}");
            assert_eq!(rows[0]["id"], code, "{name}");
            assert_eq!(rows[0]["kind"], kind, "{name}");
        } else {
            assert!(rows.is_empty(), "{name}: {rows:#?}");
        }
    }
}

#[test]
fn merged_global_fetch_preserves_opaque_and_getter_obligations() {
    let Some(findings) = diagnostic_fixture("leaf-scope-host-callbacks") else {
        return;
    };
    let source =
        include_str!("../../../../fixtures/reactive-ir/leaf-scope-host-callbacks/Fetch.tsx");
    for (name, expected) in [
        ("NativeFetch", None),
        ("AsyncFetch", None),
        ("OpaqueFetch", Some(("SC9012", "uncertifiable"))),
        ("ShadowedFetch", Some(("SC3001", "violation"))),
        ("FetchWithGetter", Some(("SC9012", "uncertifiable"))),
    ] {
        let start = source.find(&format!("export function {name}(")).unwrap();
        let end = source[start + 1..]
            .find("export function ")
            .map_or(source.len(), |offset| start + 1 + offset);
        let rows = findings
            .iter()
            .filter(|row| {
                row["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/Fetch.tsx"))
                    && row["primaryLocation"]["startByte"]
                        .as_u64()
                        .is_some_and(|byte| (start as u64..end as u64).contains(&byte))
                    && matches!(row["id"].as_str(), Some("SC3001" | "SC9012"))
            })
            .collect::<Vec<_>>();
        if let Some((code, kind)) = expected {
            assert_eq!(rows.len(), 1, "{name}: {rows:#?}");
            assert_eq!(rows[0]["id"], code, "{name}");
            assert_eq!(rows[0]["kind"], kind, "{name}");
        } else {
            assert!(rows.is_empty(), "{name}: {rows:#?}");
        }
    }
}

#[test]
fn optional_primitive_signatures_preserve_dispatch_and_callback_boundaries() {
    let Some(findings) = diagnostic_fixture("literal-argument-member-dispatch") else {
        return;
    };
    let rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/Optional.tsx"))
        })
        .collect::<Vec<_>>();
    let source = include_str!(
        "../../../../fixtures/reactive-ir/literal-argument-member-dispatch/Optional.tsx"
    );
    for (expression, code, kind) in [
        ("date);", "SC9012", "uncertifiable"),
        ("name());", "SC1001", "uncertifiable"),
    ] {
        let start = source.find(expression).unwrap();
        let row = rows
            .iter()
            .find(|row| row["primaryLocation"]["startByte"].as_u64() == Some(start as u64))
            .unwrap_or_else(|| panic!("missing {expression}: {rows:#?}"));
        assert_eq!(row["id"], code);
        assert_eq!(row["kind"], kind);
    }
    assert_eq!(
        rows.len(),
        2,
        "the primitive chain must stay clean: {rows:#?}"
    );
}

#[test]
fn formatted_primitive_receivers_demand_exact_builtin_declarations() {
    let Some(findings) = diagnostic_fixture("literal-argument-member-dispatch") else {
        return;
    };
    let rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/Formatted.tsx"))
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1, "{rows:#?}");
    let row = rows[0];
    assert_eq!(row["id"], "SC9012");
    assert_eq!(row["kind"], "uncertifiable");
    let source = include_str!(
        "../../../../fixtures/reactive-ir/literal-argument-member-dispatch/Formatted.tsx"
    );
    let start = source.find("formattedObject(when)").unwrap() + "formattedObject(".len();
    assert_eq!(
        row["primaryLocation"]["startByte"].as_u64(),
        Some(start as u64)
    );
    assert_eq!(
        row["primaryLocation"]["endByte"].as_u64(),
        Some((start + "when".len()) as u64)
    );
}

#[test]
fn callback_proof_budget_is_per_root_and_stays_bounded() {
    use std::{fs, process::Command, time::SystemTime};
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/project-callback-uses");
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch = std::env::temp_dir().join(format!(
        "solid-checker-callback-budget-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(scratch.join("node_modules/solid-js")).unwrap();
    fs::write(
        scratch.join("node_modules/solid-js/package.json"),
        r#"{"name":"solid-js","version":"2.0.0-rc.13"}"#,
    )
    .unwrap();
    for name in ["tsconfig.json", "solid-js.d.ts"] {
        fs::copy(fixture.join(name), scratch.join(name)).unwrap();
    }
    let mut source =
        String::from("import { createSignal, createMemo, untrack } from 'solid-js';\n");
    for index in 0..140 {
        // Mixed safe modes require the closed consumer proof, rather than
        // the earlier all-tracked execution-role classifier.
        source.push_str(&format!(
            "function consume{index}(read: () => number) {{ createMemo(read); untrack(read); }}\n\
             export function Independent{index}() {{ const [count] = createSignal(0); consume{index}(() => count()); return <div />; }}\n"
        ));
    }
    source.push_str("function oversized(read: () => number) {\n");
    for index in 0..129 {
        source.push_str(&format!("consume{index}(read);\n"));
    }
    source.push_str(
        "}\nexport function Oversized() { const [count] = createSignal(0); oversized(() => count()); return <div />; }\n\
         declare function unknown(read: () => number): void;\n\
         export function Unknown() { const [count] = createSignal(0); unknown(() => count()); return <div />; }\n",
    );
    fs::write(scratch.join("Budget.tsx"), &source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .args(["--format", "json", "--project"])
        .arg(scratch.join("tsconfig.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let rows = snapshot["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["id"] == "SC1001")
        .collect::<Vec<_>>();
    for name in ["Oversized", "Unknown"] {
        let owner = source.find(&format!("export function {name}()")).unwrap();
        let start = owner + source[owner..].find("() => count()").unwrap() + "() => ".len();
        let row = rows
            .iter()
            .find(|row| row["primaryLocation"]["startByte"].as_u64() == Some(start as u64))
            .unwrap_or_else(|| panic!("{name} must stay uncertifiable"));
        assert_eq!(row["kind"], "uncertifiable");
    }
    assert_eq!(
        rows.len(),
        2,
        "unrelated small proofs must not exhaust each other's allowance; scratch: {}",
        scratch.display()
    );
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn routed_options_callbacks_compose_source_contexts() {
    let Some(findings) = diagnostic_fixture("forwarded-tracked-compute") else {
        return;
    };
    let source = include_str!(
        "../../../../fixtures/reactive-ir/forwarded-tracked-compute/RoutedMembers.tsx"
    );
    let rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/RoutedMembers.tsx"))
        })
        .collect::<Vec<_>>();
    // ADR 0321: each `read` arrow is written in the argument literal, so its
    // dispatch resolves to it; the read keeps the source context proven here.
    assert_eq!(rows.len(), 4, "{rows:#?}");
    for (name, helper, _routed_dispatch, strict) in [
        ("RoutedTracked", "routedMember", false, false),
        ("RoutedMixed", "mixedRoutedMember", true, true),
        ("RoutedFresh", "freshRoutedMember", false, false),
        ("RoutedSampled", "sampledRoutedMember", true, false),
        ("RoutedEscaped", "escapedRoutedMember", true, true),
        ("RoutedSiblingArrow", "siblingRoutedMember", false, false),
        ("RoutedSiblingReceiver", "receiverRoutedMember", true, true),
        (
            "RoutedSubjectReceiver",
            "subjectReceiverRoutedMember",
            true,
            true,
        ),
    ] {
        let owner = source.find(&format!("export function {name}(")).unwrap();
        let argument =
            owner + source[owner..].find(&format!("{helper}(")).unwrap() + helper.len() + 1;
        let read = argument + "{ read: () => ".len();
        for (code, expected, start, width) in [
            ("SC9012", false, argument, "{ read: () => count() }".len()),
            ("SC1001", strict, read, "count()".len()),
        ] {
            let row = rows.iter().find(|row| {
                row["id"] == code
                    && row["primaryLocation"]["startByte"].as_u64() == Some(start as u64)
                    && row["primaryLocation"]["endByte"].as_u64() == Some((start + width) as u64)
            });
            assert_eq!(row.is_some(), expected, "{name}/{code}: {rows:#?}");
            if let Some(row) = row {
                assert_eq!(row["kind"], "uncertifiable");
            }
        }
    }
}

#[test]
fn returned_literal_body_facts_project_reads_only_at_invocation() {
    let Some(findings) = diagnostic_fixture("fp-summary-returned-closure") else {
        return;
    };
    let source = include_str!(
        "../../../../fixtures/reactive-ir/fp-summary-returned-closure/DirectReturns.tsx"
    );
    let rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/DirectReturns.tsx"))
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1, "{rows:#?}");
    let start = source.find("const value = read()").unwrap() + "const value = ".len();
    assert_eq!(
        rows[0]["primaryLocation"]["startByte"].as_u64(),
        Some(start as u64)
    );
    assert_eq!(
        rows[0]["primaryLocation"]["endByte"].as_u64(),
        Some((start + 6) as u64)
    );
    assert_eq!(rows[0]["id"], "SC1001");
    assert_eq!(rows[0]["kind"], "violation");
}

#[test]
fn local_declaration_dispatch_rejects_escapes() {
    let Some(findings) = diagnostic_fixture("literal-argument-member-dispatch") else {
        return;
    };
    let source = include_str!(
        "../../../../fixtures/reactive-ir/literal-argument-member-dispatch/DeclarationDispatch.tsx"
    );
    let rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/DeclarationDispatch.tsx"))
                && row["analysisContext"] == "parameter-member-target-unresolved"
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1, "{rows:#?}");
    for (marker, expression, unresolved) in [
        ("function LocalDeclaration", "pageSlice(props.items)", false),
        (
            "function EscapedDeclaration",
            "pageSlice(props.items)",
            true,
        ),
    ] {
        let definition = source.find(marker).unwrap();
        let argument =
            definition + source[definition..].find(expression).unwrap() + "pageSlice(".len();
        let row = rows
            .iter()
            .find(|row| row["primaryLocation"]["startByte"].as_u64() == Some(argument as u64));
        assert_eq!(row.is_some(), unresolved, "{marker}: {rows:#?}");
        if let Some(row) = row {
            assert_eq!(row["id"], "SC9012");
            assert_eq!(row["kind"], "uncertifiable");
        }
    }
}

#[test]
fn nested_parameter_members_require_closed_values_at_every_level() {
    let Some(findings) = diagnostic_fixture("literal-argument-member-dispatch") else {
        return;
    };
    let source = include_str!(
        "../../../../fixtures/reactive-ir/literal-argument-member-dispatch/Nested.tsx"
    );
    let rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/Nested.tsx"))
        })
        .collect::<Vec<_>>();
    for (name, expected) in [
        ("NestedLiteral", None),
        ("NestedBinding", None),
        ("NestedAliases", None),
        ("NestedNamespace", None),
        ("NestedMethod", None),
        ("NestedReactiveRead", Some("SC1001")),
        ("NestedWrongLevel", Some("SC1001")),
        ("NestedUnknown", Some("SC9012")),
        ("NestedGetter", Some("SC9012")),
        ("NestedSpread", Some("SC9012")),
        ("NestedMutable", None),
        ("NestedReassignedLet", Some("SC9012")),
        ("NestedEscaped", Some("SC9012")),
        ("NestedReplaced", Some("SC9012")),
        ("NestedReceiver", Some("SC9012")),
        ("NestedCalleeEscape", Some("SC9012")),
        ("NestedCalleeWrite", Some("SC9012")),
        ("NestedDefault", None),
        ("NestedDefaultUndefined", Some("SC9012")),
        ("NestedDefaultUnknown", Some("SC9012")),
        ("NestedConditional", Some("SC9012")),
        ("NestedFactory", Some("SC9012")),
        ("NestedMethodArguments", Some("SC9012")),
        ("NestedComputedPrefix", Some("SC9012")),
    ] {
        let start = source.find(&format!("export function {name}(")).unwrap();
        let end = source[start..].find("\n}").unwrap() + start;
        let selected = rows
            .iter()
            .filter(|row| {
                row["primaryLocation"]["startByte"]
                    .as_u64()
                    .is_some_and(|offset| offset >= start as u64 && offset < end as u64)
                    && (row["id"] == "SC9012"
                        || row["id"] == "SC1001"
                            && row["message"]
                                .as_str()
                                .is_some_and(|message| message.contains("read through")))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            selected.len(),
            usize::from(expected.is_some()),
            "{name}: {selected:#?}"
        );
        if let Some(code) = expected {
            assert_eq!(selected[0]["id"], code, "{name}: {selected:#?}");
            assert_eq!(
                selected[0]["kind"], "uncertifiable",
                "{name}: {selected:#?}"
            );
            if code == "SC9012" {
                // ADR 0320: `undefined` selects the callee's default.
                let context = if name == "NestedDefaultUndefined" {
                    "parameter-default-unresolved"
                } else {
                    "parameter-member-path-unresolved"
                };
                assert_eq!(selected[0]["analysisContext"], context);
            }
        }
    }
}

#[test]
fn member_dispatch_keeps_the_source_invocation_context() {
    let Some(findings) = diagnostic_fixture("forwarded-tracked-compute") else {
        return;
    };
    let source = include_str!(
        "../../../../fixtures/reactive-ir/forwarded-tracked-compute/MemberContexts.tsx"
    );
    let rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/MemberContexts.tsx"))
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3, "{rows:#?}");
    for (name, helper, unresolved) in [
        ("TrackedMember", "trackedMember", false),
        ("MixedMember", "mixedMember", true),
        ("DeferredMember", "deferredMember", false),
        ("UnknownMember", "unknownMember", true),
        ("DefaultMember", "defaultMember", true),
    ] {
        let function = source.find(&format!("export function {name}(")).unwrap();
        let call = function + source[function..].find(&format!("{helper}(")).unwrap();
        let argument = call + helper.len() + 1;
        let argument_end = argument + "{ read: () => count() }".len();
        let row = rows.iter().find(|row| {
            row["id"] == "SC9012"
                && row["primaryLocation"]["startByte"].as_u64() == Some(argument as u64)
                && row["primaryLocation"]["endByte"].as_u64() == Some(argument_end as u64)
        });
        // ADR 0321: the dispatch resolves to the argument literal's arrow.
        assert!(row.is_none(), "{name}: {rows:#?}");
        if let Some(row) = row {
            assert_eq!(row["kind"], "uncertifiable", "{name}: {row:#?}");
        }
        let read = argument + "{ read: () => ".len();
        let row = rows.iter().find(|row| {
            row["id"] == "SC1001"
                && row["primaryLocation"]["startByte"].as_u64() == Some(read as u64)
                && row["primaryLocation"]["endByte"].as_u64() == Some((read + 7) as u64)
        });
        assert_eq!(row.is_some(), unresolved, "{name}: {rows:#?}");
    }
}

#[test]
fn named_event_prop_forwarding_requires_an_exclusive_reference_census() {
    let Some(findings) = diagnostic_fixture("forwarded-event-prop") else {
        return;
    };
    let source =
        include_str!("../../../../fixtures/reactive-ir/forwarded-event-prop/NamedHandlers.tsx");
    let rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/NamedHandlers.tsx"))
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 7, "{rows:#?}");
    for (name, unresolved) in [
        ("UsesNamed", false),
        ("UsesWrappedNamed", false),
        ("UsesEagerNamed", true),
        ("UsesEscapedNamed", true),
        ("UsesAliasedNamed", true),
        ("UsesMutableNamed", true),
        ("UsesShadowedNamed", false),
        ("UsesShadowedEagerNamed", true),
        ("UsesCallbackDefault", true),
        ("UsesImportedNamed", false),
        ("UsesNamedThroughComponent", false),
        ("UsesRepeatedNamed", false),
        ("UsesRepeatedInline", false),
        ("UsesNamedCycle", true),
    ] {
        let function = source.find(&format!("export function {name}(")).unwrap();
        let start = function + source[function..].find("count();").unwrap();
        let row = rows.iter().find(|row| {
            row["primaryLocation"]["startByte"].as_u64() == Some(start as u64)
                && row["primaryLocation"]["endByte"].as_u64() == Some((start + 7) as u64)
        });
        assert_eq!(row.is_some(), unresolved, "{name}: {rows:#?}");
        if let Some(row) = row {
            assert_eq!(row["id"], "SC1001", "{name}: {row:#?}");
            assert_eq!(row["kind"], "uncertifiable", "{name}: {row:#?}");
        }
    }
    let eval_rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/EvalHandler.tsx"))
        })
        .collect::<Vec<_>>();
    assert_eq!(eval_rows.len(), 1, "{eval_rows:#?}");
    assert_eq!(eval_rows[0]["id"], "SC1001");
    assert_eq!(eval_rows[0]["kind"], "uncertifiable");
}

#[test]
fn leaf_focus_requires_runtime_body_and_event_dispatch_evidence() {
    let Some(findings) = diagnostic_fixture("leaf-scope-host-callbacks") else {
        return;
    };
    let source =
        include_str!("../../../../fixtures/reactive-ir/leaf-scope-host-callbacks/Focus.tsx");
    let rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/Focus.tsx"))
        })
        .collect::<Vec<_>>();
    for (name, expression, expected) in [
        (
            "OptionalDomFocus",
            "props.dialog?.focus",
            Some(("SC9012", "uncertifiable")),
        ),
        (
            "FocusWithListener",
            "props.dialog?.focus",
            Some(("SC9012", "uncertifiable")),
        ),
        (
            "UnknownFocus",
            "props.target.focus",
            Some(("SC9012", "uncertifiable")),
        ),
        ("ExactInertFocus", "target.focus", None),
        (
            "ExactForbiddenFocus",
            "target.focus",
            Some(("SC3001", "violation")),
        ),
    ] {
        let function = source.find(&format!("export function {name}(")).unwrap();
        let start = function + source[function..].find(expression).unwrap();
        let actual = rows.iter().find(|row| {
            row["primaryLocation"]["startByte"].as_u64() == Some(start as u64)
                && row["primaryLocation"]["endByte"].as_u64()
                    == Some((start + expression.len()) as u64)
        });
        assert_eq!(actual.is_some(), expected.is_some(), "{name}: {rows:#?}");
        if let (Some(row), Some((code, kind))) = (actual, expected) {
            assert_eq!(row["id"], code, "{name}: {row:#?}");
            assert_eq!(row["kind"], kind, "{name}: {row:#?}");
        }
    }
}

#[test]
fn local_member_dispatch_requires_only_tracked_exact_callers() {
    let Some(findings) = diagnostic_fixture("literal-argument-member-dispatch") else {
        return;
    };
    let source = include_str!(
        "../../../../fixtures/reactive-ir/literal-argument-member-dispatch/TrackedDispatch.tsx"
    );
    let rows = findings
        .iter()
        .filter(|row| {
            row["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/TrackedDispatch.tsx"))
                && row["analysisContext"] == "parameter-member-target-unresolved"
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 10, "{rows:#?}");
    for (name, occurrence, unresolved) in [
        ("DirectTracked", 0, false),
        ("LocalTracked", 0, false),
        ("ComponentProperty", 0, false),
        ("ChainedTracked", 0, false),
        ("MixedCalls", 0, true),
        ("EscapedHelper", 0, true),
        // ADR 0320: the call follows an await reached on every path.
        ("AsyncHelper", 0, false),
        ("RecursiveHelper", 0, true),
        ("ClearedTracking", 0, false),
        ("ShadowedHelper", 0, true),
        ("ShadowedHelper", 1, false),
        ("WrappedCallee", 0, false),
        ("MutableHelper", 0, true),
        ("StoredCaller", 0, true),
        ("GetterDefaultCaller", 0, true),
        ("JsxDefaultCaller", 0, true),
        ("LocalEvent", 0, false),
        ("ChainedEvent", 0, false),
        ("LocalTimer", 0, false),
        ("MixedEvent", 0, true),
        ("EventDefaultCaller", 0, true),
    ] {
        let function = source.find(&format!("export function {name}(")).unwrap();
        let call = source[function..]
            .match_indices("pageSlice(props.items)")
            .nth(occurrence)
            .unwrap()
            .0;
        let start = function + call + "pageSlice(".len();
        let row = rows
            .iter()
            .find(|row| row["primaryLocation"]["startByte"].as_u64() == Some(start as u64));
        assert_eq!(row.is_some(), unresolved, "{name}/{occurrence}: {rows:#?}");
        if let Some(row) = row {
            assert_eq!(row["id"], "SC9012", "{name}: {row:#?}");
            assert_eq!(row["kind"], "uncertifiable", "{name}: {row:#?}");
        }
    }
}

#[test]
fn local_dispatch_eval_and_depth_limits_keep_obligations() {
    use std::{fs, process::Command, time::SystemTime};
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/literal-argument-member-dispatch");
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch = std::env::temp_dir().join(format!(
        "solid-checker-tracked-dispatch-eval-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(scratch.join("node_modules/solid-js")).unwrap();
    fs::write(
        scratch.join("node_modules/solid-js/package.json"),
        r#"{"name":"solid-js","version":"2.0.0-rc.13"}"#,
    )
    .unwrap();
    for name in ["tsconfig.json", "solid-js.d.ts"] {
        fs::copy(fixture.join(name), scratch.join(name)).unwrap();
    }
    let source = fs::read_to_string(fixture.join("TrackedDispatch.tsx")).unwrap();
    let source = source.replacen(
        "  return <div>{visible().length}</div>;",
        "  eval(\"visible()\");\n  return <div>{visible().length}</div>;",
        1,
    );
    let function = source.find("export function LocalTracked(").unwrap();
    let start =
        function + source[function..].find("pageSlice(props.items)").unwrap() + "pageSlice(".len();
    fs::write(scratch.join("TrackedDispatch.tsx"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .args(["--format", "json", "--project"])
        .arg(scratch.join("tsconfig.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let row = snapshot["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["primaryLocation"]["startByte"].as_u64() == Some(start as u64))
        .expect("direct eval must keep the member dispatch uncertifiable");
    assert_eq!(row["id"], "SC9012", "{row:#?}");
    assert_eq!(row["kind"], "uncertifiable", "{row:#?}");

    let mut bounded = String::from(
        "function pageSlice(items: readonly number[]) { return items.slice(0, 2); }\n",
    );
    for depth in [32, 33] {
        bounded.push_str(&format!(
            "export function Depth{depth}(props: {{ items: readonly number[] }}) {{\n\
             const h1 = () => {{ pageSlice(props.items); return h0(); }};\n\
             const h0 = () => pageSlice(props.items);\n"
        ));
        for index in 2..depth {
            bounded.push_str(&format!("const h{index} = () => h{}();\n", index - 1));
        }
        bounded.push_str(&format!(
            "return <div>{{h{}().length}}</div>;\n}}\n",
            depth - 1
        ));
    }
    fs::write(scratch.join("TrackedDispatch.tsx"), &bounded).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .args(["--format", "json", "--project"])
        .arg(scratch.join("tsconfig.json"))
        .output()
        .unwrap();
    fs::remove_dir_all(scratch).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    for depth in [32, 33] {
        let function = bounded
            .find(&format!("export function Depth{depth}("))
            .unwrap();
        let start = function
            + bounded[function..]
                .find("const h0 = () => pageSlice(")
                .unwrap()
            + "const h0 = () => pageSlice(".len();
        let row = snapshot["findings"].as_array().unwrap().iter().find(|row| {
            row["primaryLocation"]["startByte"].as_u64() == Some(start as u64)
                && row["id"] == "SC9012"
        });
        assert_eq!(row.is_some(), depth == 33, "depth {depth}: {snapshot:#?}");
        if let Some(row) = row {
            assert_eq!(row["kind"], "uncertifiable", "{row:#?}");
        }
    }
}

#[test]
fn typed_options_wrappers_preserve_source_callback_execution() {
    let Some(findings) = diagnostic_fixture("forwarded-tracked-compute") else {
        return;
    };
    let source =
        include_str!("../../../../fixtures/reactive-ir/forwarded-tracked-compute/QueryOptions.tsx");
    let rows = findings
        .iter()
        .filter(|finding| {
            finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/QueryOptions.tsx"))
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3, "{rows:#?}");
    for (wrapper, expected) in [
        ("wrapped", None),
        ("wrappedSnapshot", None),
        ("wrappedAndEager", Some("violation")),
        ("wrappedLater", Some("uncertifiable")),
        ("wrappedUnknown", Some("uncertifiable")),
    ] {
        let expression = format!("{wrapper}(() => ({{ enabled: open() }}))");
        let start =
            source.find(&expression).expect("callback call") + expression.find("open()").unwrap();
        let row = rows
            .iter()
            .find(|row| row["primaryLocation"]["startByte"].as_u64() == Some(start as u64));
        match expected {
            Some(kind) => {
                let row = row.unwrap_or_else(|| panic!("{wrapper}: {rows:#?}"));
                assert_eq!(row["id"], "SC1001", "{wrapper}: {row:#?}");
                assert_eq!(row["kind"], kind, "{wrapper}: {row:#?}");
                assert_eq!(
                    row["primaryLocation"]["endByte"].as_u64(),
                    Some((start + "open()".len()) as u64),
                    "{wrapper}: {row:#?}"
                );
            }
            None => assert!(row.is_none(), "{wrapper}: {row:#?}"),
        }
    }
}

#[test]
fn returned_tuple_iteration_mutations_withdraw_only_the_proven_read() {
    use std::{fs, process::Command};
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/tuple-iteration-veto");
    let scratch = std::path::PathBuf::from("/tmp/sc-scratch.noindex")
        .join(format!("tuple-iteration-{}", std::process::id()));
    for package in ["solid-js", "@solidjs/signals", "@solidjs/web"] {
        fs::create_dir_all(scratch.join("node_modules").join(package)).unwrap();
    }
    for name in [
        "App.tsx",
        "factory.ts",
        "gateways.ts",
        "tsconfig.json",
        "solid-js.d.ts",
        "node_modules/solid-js/package.json",
        "node_modules/@solidjs/signals/package.json",
        "node_modules/@solidjs/web/package.json",
    ] {
        fs::copy(fixture.join(name), scratch.join(name)).unwrap();
    }
    let cases: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.join("cases.json")).unwrap()).unwrap();
    for case in cases.as_array().unwrap() {
        fs::write(
            scratch.join("mutations.ts"),
            format!("{}\nexport {{}};\n", case["source"].as_str().unwrap()),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env("SOLID_CHECKER_DAEMON", "0")
            .args(["--format", "json", "--project"])
            .arg(scratch.join("tsconfig.json"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            case["name"],
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let rows = snapshot["findings"].as_array().unwrap();
        let read = rows
            .iter()
            .filter(|row| {
                row["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("App.tsx"))
            })
            .collect::<Vec<_>>();
        assert_eq!(read.len(), 2, "{}: {rows:#?}", case["name"]);
        let (kind, code) = if case["veto"] == true {
            ("uncertifiable", "SC9012")
        } else {
            ("violation", "SC1001")
        };
        assert_eq!(read[0]["kind"], kind, "{}: {read:#?}", case["name"]);
        assert_eq!(read[0]["id"], code, "{}: {read:#?}", case["name"]);
        assert_eq!(read[1]["kind"], kind, "{}: {read:#?}", case["name"]);
        assert_eq!(read[1]["id"], code, "{}: {read:#?}", case["name"]);
        assert!(
            rows.iter()
                .all(|row| row["kind"] != "violation" || !case["veto"].as_bool().unwrap()),
            "{}: {rows:#?}",
            case["name"]
        );
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn returned_store_tuple_veto_preserves_uncertifiable_candidates() {
    use std::process::Command;
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for (fixture, expected, kind, code, flags) in [
        (
            "returned-store-proxy",
            4,
            "uncertifiable",
            "SC9012",
            Vec::new(),
        ),
        (
            "dependency-source-proxy-store",
            1,
            "violation",
            "SC1001",
            vec![
                "--runtime-target",
                "browser",
                "--runtime-build",
                "development",
                "--dependency-source",
                "dep-query",
            ],
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env("SOLID_CHECKER_DAEMON", "0")
            .args(["--format", "json", "--project"])
            .arg(root.join(format!("fixtures/reactive-ir/{fixture}/tsconfig.json")))
            .args(flags)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let rows = snapshot["findings"].as_array().unwrap();
        let reads = rows
            .iter()
            .filter(|row| {
                row["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("App.tsx"))
            })
            .collect::<Vec<_>>();
        assert_eq!(reads.len(), expected, "{fixture}: {rows:#?}");
        assert!(
            reads
                .iter()
                .all(|row| row["kind"] == kind && row["id"] == code),
            "{fixture}: {rows:#?}"
        );
    }
}

#[test]
fn write_scope_diagnostics_have_semantic_locations() {
    let Some(findings) = diagnostic_fixture("write-scope") else {
        return;
    };
    // 12 writes / 2 actions: the untrack-wrapped writes in the component body
    // and in a memo count (the rc.0 guard keys on the owner, not tracking),
    // while writes and the action inside createTrackedEffect no longer do
    // (children-forbidden leaf scopes are legal write regions). The two store
    // setters directly in the component body count too: the fixture resolves
    // no solid-js, so it is analyzed as the audited release, whose store
    // setter guard rejects a root owner (ADR 0127; rc.13 keeps rc.9's
    // answer, ADR 0194).
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
                // exported-helper proof obligation. The two closed local
                // callback adapters are now included: their exact returned
                // function values reach the leaf scope and report SC3001 at
                // registerTeardown(). Silent, each for its own reason: the
                // out-of-band (event handler) onSettled, the helper call
                // written inside the event handler, and the helper that only
                // builds a nested function.
                ("leaf-owner-forbidden-call", 17),
                // Three inline (the function-seeded createSignal, createMemo,
                // createRoot) plus the dynamic-extent trackDouble() reached
                // through its helper.
                // Two inline plus flushNow() reached through its helper from
                // a block-bodied and an expression-bodied leaf callback.
                // The six returns that used to be reported here are the
                // domain `EffectFunction`'s `(() => void) | void` return type
                // rejects, so the value-legality rules are gone; the fixture
                // keeps them as sources because the ownership rules still
                // classify them.
                ("invalid-cleanup-return", 0),
                ("cleanup-return-unresolved", 0),
                // The local callback adapters have exact closed returns and
                // are no longer dispatch obligations.
                ("reactive-dispatch-unresolved", 0),
            ][..],
        ),
        (
            "static-api",
            &[
                // The valid deprecated one-argument overload, the same absent
                // apply slot behind an exact one-element tuple spread, plus
                // five cast-hidden non-callable runtime values (including a
                // bad EffectBundle.effect field). Raw invalid apply arguments
                // are TypeScript's diagnostics and stay silent.
                ("missing-effect-function", 7),
                // Signal-family only: the store constructors never route
                // options.sync into their node, so their three sync: true
                // async derives are negative cases now.
                ("sync-computation-received-async", 3),
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
        ("owner-presence", &[("missing-owner", 14)]),
        (
            "async-boundary",
            &[
                // Four untracked reads: two plain async sources, the
                // declared-loadingValue source (still reported — the declared
                // window ends at the first real answer, so later re-asks
                // throw; conditional wording), and the opaque-options source
                // (downgraded to uncertifiable, asserted below).
                ("pending-async-unsuspendable-read", 7),
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
                ("async-outside-loading-boundary", 1),
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
    let cleanup = findings_for_rule(&findings, "leaf-owner-forbidden-call")
        .into_iter()
        .filter(|finding| {
            finding["message"]
                .as_str()
                .is_some_and(|message| message.starts_with("onCleanup"))
        })
        .collect::<Vec<_>>();
    // The out-of-band onSettled (event handler) must not carry any leaf-scope
    // finding: the runtime enqueues a plain callback there.
    assert!(
        findings
            .iter()
            .filter(|finding| finding["rule"] == "leaf-owner-forbidden-call")
            .all(|finding| {
                finding["message"]
                    .as_str()
                    .is_some_and(|message| !message.contains("OutOfBand"))
            }),
        "{findings:#?}"
    );
    // The exported helper's call sites are unknowable, so its onSettled leaf
    // finding is a proof obligation, not a proven violation; the owner-backed
    // component-body ones stay violations — three inline, three reached
    // through exactly-resolved dynamic-extent helpers, and two through the
    // closed local callback adapters.
    let kinds = cleanup
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "violation").count(),
        8,
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
    let owner_starts = |message_prefix: &str| {
        findings_for_rule(&findings, "missing-owner")
            .iter()
            .filter(|finding| {
                finding["message"]
                    .as_str()
                    .is_some_and(|message| message.starts_with(message_prefix))
            })
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
    // computed key stays a read. SC3001/SC4001/SC4001: the one owner-backed
    // settled cleanup written as a literal callback reports SC3001 without a
    // duplicate SC4001; the wrapper-built, identifier-referenced, and
    // out-of-band cleanups are SC4001 only. The returned-call block adds three
    // SC3004 (a produced `number` in both return spellings, plus the unowned
    // one), one SC9002 (`any`), and one SC4001 (a produced function is a real
    // cleanup); a produced function, `(() => void) | undefined`, and `void`
    // are legal and silent.
    for (rule, expected) in [
        ("reactive-read-after-await", 2),
        ("invalid-cleanup-return", 0),
        ("cleanup-return-unresolved", 0),
        ("strict-read-untracked", 5),
        // Still four, but not the same four: the 2026-08-17 narrowing traded the
        // three root-record writes for the nested, cast, and props writes
        // asserted by span below. A count alone cannot tell those apart.
        ("no-direct-mutation", 4),
        ("leaf-owner-forbidden-call", 1),
        // One proven returned cleanup plus four callbacks whose runtime
        // return may be a cleanup and therefore cannot be certified safe.
        ("missing-owner", 8),
    ] {
        assert_rule_findings(&findings, rule, expected);
    }

    // The leaf rules need the literal callback *and* the call's place in its
    // own synchronous extent. Past this landmark every leaf-owner call in the
    // fixture is wrapper-built, handed over as an identifier reference, or
    // has its onCleanup in a nested function the callback merely builds — no
    // leaf scope is proven at any of them, so SC3001 stops here while the
    // genuinely unowned SC4001 continues.
    let non_literal_leaf = start_of("// `wrap` may stash");
    assert!(
        starts("leaf-owner-forbidden-call")
            .iter()
            .all(|start| *start < non_literal_leaf),
        "a leaf callback that is not the literal argument proves no leaf scope"
    );
    assert_eq!(
        owner_starts("onCleanup")
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

    // 2.0's store proxy is shallowly `Readonly`, so a write to a *root*
    // property is TS2540 and no longer this checker's. The shallowness and the
    // cast escape hatch are what remain, and each is pinned by span -- the
    // count above is satisfied by four findings either way, so only these
    // assertions distinguish the narrowing from a regression that dropped the
    // wrong three.
    for root_write in ["profile.name = ", "profile.count += ", "profile.count++"] {
        assert!(
            !starts("no-direct-mutation").contains(&start_of(root_write)),
            "a root store property is TS2540, so {root_write:?} is TypeScript's to report"
        );
    }
    for kept in [
        // The readonly-ness stops at the top level.
        "profile.user.name = ",
        // A cast erases it, so TypeScript falls silent and the write is still
        // dropped. `member_root` resolves through the cast, which is why the
        // narrowing demands a bare-identifier root.
        "(profile as { user: { name: string } }).user = ",
        // A props member is not readonly at all.
        "props.n = ",
    ] {
        assert!(
            starts("no-direct-mutation").contains(&start_of(kept)),
            "{kept:?} is a write TypeScript accepts and the checker must still report"
        );
    }
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
    // so no cleanup is handed over and SC4001 must not fire; the neighbouring
    // `onSettled(() => makeThunk())` produces a function and must.
    // Anchored on the module-level spellings: the returned expression is the
    // reported span, and both callees also appear inside the component above.
    let returned_call = start_of("makeCount();\n});");
    let unowned_thunk =
        start_of("onSettled(() => makeThunk())") + u64::try_from("onSettled(".len()).unwrap();
    assert!(
        !owner_starts("onSettled").contains(&returned_call),
        "a call producing a number hands the owner no cleanup to leak"
    );
    assert!(
        owner_starts("onSettled").contains(&unowned_thunk),
        "a call producing a function is an unowned returned cleanup"
    );

    // `return nothing` where `nothing: undefined` is a legal cleanup return
    // that hands the owner nothing, so it is not a returned cleanup that would
    // make these unowned callbacks SC4001.
    let cleanup_starts = owner_starts("onSettled");
    for typed_undefined in [
        start_of("return nothing;") + u64::try_from("return ".len()).unwrap(),
        start_of("=> nothing);") + u64::try_from("=> ".len()).unwrap(),
    ] {
        assert!(
            !cleanup_starts.contains(&typed_undefined),
            "a proven-`undefined` return is not an unowned cleanup"
        );
    }
}

#[test]
fn declared_first_paint_and_opaque_options_split_the_async_rules() {
    let Some(findings) = diagnostic_fixture("async-boundary") else {
        return;
    };
    // Fail-honest policy: SC5001 stays a proven violation when the options
    // argument is absent or readable, and downgrades to a proof obligation
    // when an unreadable options argument could declare a loadingValue.
    let untracked = findings_for_rule(&findings, "pending-async-unsuspendable-read")
        .into_iter()
        .filter(|finding| finding["severity"] == "error")
        .collect::<Vec<_>>();
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
    // The declared source keeps SC5001/SC5001 with conditional wording: the
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
fn props_container_and_callback_creation_do_not_prove_nested_behavior() {
    let Some(findings) = diagnostic_fixture("callee-callback-timing") else {
        return;
    };
    let props = findings
        .iter()
        .filter(|finding| {
            finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/Props.tsx"))
        })
        .cloned()
        .collect::<Vec<_>>();
    let reads = findings_for_rule(&props, "strict-read-untracked");
    assert_eq!(reads.len(), 1, "{props:#?}");
    // ADR 0199: `Handler` invokes the prop only inside a `<button>` click
    // handler, so both callback-prop literals run on dispatch and report
    // nothing. The consumers that leave such a read uncertifiable are
    // pinned by `forwarded-event-prop`.
    for line in [10, 15] {
        assert!(
            !reads
                .iter()
                .any(|read| read["primaryLocation"]["line"] == line),
            "{reads:#?}"
        );
    }
    let eager = reads
        .iter()
        .find(|read| read["primaryLocation"]["line"] == 21)
        .expect("eager component-body read");
    assert_eq!(eager["kind"], "violation", "{eager:#?}");
    let mutations = findings_for_rule(&props, "no-direct-mutation");
    assert_eq!(mutations.len(), 1, "{props:#?}");
    assert_eq!(mutations[0]["primaryLocation"]["line"], 36);
    // A write through the props container is an obligation: the props object
    // may be the compiler's plain object, where the write succeeds.
    assert_eq!(mutations[0]["kind"], "uncertifiable");
}

#[test]
fn pending_callback_reads_preserve_callee_invocation_uncertainty() {
    let Some(findings) = diagnostic_fixture("callee-callback-timing") else {
        return;
    };
    let pending = findings_for_rule(&findings, "pending-async-unsuspendable-read");
    assert_eq!(pending.len(), 7, "{pending:#?}");
    assert_eq!(
        pending
            .iter()
            .filter(|finding| finding["kind"] == "violation")
            .count(),
        4,
        "{pending:#?}"
    );
    let open = pending
        .iter()
        .filter(|finding| finding["kind"] == "uncertifiable")
        .collect::<Vec<_>>();
    assert_eq!(open.len(), 3, "{pending:#?}");
    assert!(
        open.iter().all(|finding| {
            finding["message"].as_str().is_some_and(|message| {
                (message.contains("pending handling are unproven")
                    || message.contains("strict-read window is unproven"))
                    && !message.contains("throws PENDING_ASYNC_UNTRACKED_READ")
            })
        }),
        "{open:#?}"
    );
}

#[test]
fn development_models_bind_exact_sources_and_primitive_symbols() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/callee-callback-timing");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .args(["--format", "json", "--feedback-facts", "--project"])
        .arg(fixture.join("tsconfig.json"))
        .output()
        .expect("run development model analysis");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let models = snapshot["feedbackFacts"]
        .as_array()
        .expect("requested source models");
    let source = models
        .iter()
        .find(|model| model["path"].as_str().unwrap().ends_with("/Async.tsx"))
        .unwrap();
    let text = std::fs::read_to_string(fixture.join("Async.tsx")).unwrap();
    assert_eq!(
        source["sourceSha256"],
        solid_facts::core::SourceHash::of(&text).to_string()
    );
    let origins = source["functions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|function| !function["derivedOrigin"].is_null())
        .collect::<Vec<_>>();
    assert_eq!(origins.len(), 8, "{origins:#?}");
    for function in origins {
        let origin = &function["derivedOrigin"];
        let call = &text
            [origin["start"].as_u64().unwrap() as usize..origin["end"].as_u64().unwrap() as usize];
        assert!(
            call.starts_with("createMemo(") || call.starts_with("Solid.createMemo("),
            "{call}"
        );
        assert_eq!(function["asynchronous"], true);
    }
    let shadowed = models
        .iter()
        .find(|model| model["path"].as_str().unwrap().ends_with("/Feedback.ts"))
        .unwrap();
    assert!(
        shadowed["functions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|function| function["derivedOrigin"].is_null())
    );
    let text = std::fs::read_to_string(fixture.join("Feedback.ts")).unwrap();
    for (name, expected) in [("discarded", "open"), ("used", "return-expression")] {
        let child = shadowed["functions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|function| {
                let parent = &function["parent"];
                parent["start"]
                    .as_u64()
                    .zip(parent["end"].as_u64())
                    .is_some_and(|(start, end)| {
                        text[start as usize..end as usize].starts_with(&format!("function {name}("))
                    })
            })
            .unwrap();
        assert_eq!(child["allocationRelevance"], expected);
    }
}

#[test]
fn ssr_client_hole_distinguishes_proven_and_unresolved_server_rendering() {
    let Some(findings) = diagnostic_fixture("ssr-client-boundary") else {
        return;
    };
    let holes = findings_for_rule(&findings, "async-outside-loading-boundary");
    assert_eq!(holes.len(), 1, "{findings:#?}");
    // The visible server entry proves the application server-renders, but no
    // render chain is traced from a mount root to `BadWidget` (server
    // renderers are not modelled as mount roots), so the missing boundary
    // above it is not proven: uncertifiable, never a violation.
    assert_eq!(holes[0]["kind"], "uncertifiable", "{holes:#?}");
    // The server throw is unconditional, so the rule mirrors it as an error.
    assert_eq!(holes[0]["severity"], "error", "{holes:#?}");
    assert!(
        holes[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("ssrSource: \"client\"")),
        "{holes:#?}"
    );
    // With no visible server entry, the same source is not certified safe:
    // the entry may live in another tsconfig/package.
    let Some(csr_findings) = diagnostic_fixture("ssr-client-boundary-csr") else {
        return;
    };
    let unresolved = findings_for_rule(&csr_findings, "async-outside-loading-boundary");
    assert_eq!(unresolved.len(), 1, "{csr_findings:#?}");
    assert_eq!(unresolved[0]["kind"], "uncertifiable", "{unresolved:#?}");
}

/// The wave-6 server-surface and resolve rules, pinned at their probed
/// gates: SC7005's server-render + Loading-children dominance, SC7006's
/// module-directive export shapes, SC7007's enableRichArguments silence, and
/// SC2004's and SC2005's observer-keyed scope split.
#[test]
fn server_surface_and_resolve_rules_pin_their_probed_gates() {
    if let Some(findings) = diagnostic_fixture("http-response-flush") {
        let drops = findings_for_rule(&findings, "http-response-after-flush");
        // The two component-body calls below the Loading boundary plus the
        // lexical call in its children; the shell, fallback, and
        // event-handler calls stay silent.
        assert_eq!(drops.len(), 3, "{findings:#?}");
        assert!(
            drops.iter().all(|finding| {
                finding["severity"] == "warning" && finding["kind"] == "uncertifiable"
            }),
            "the post-flush race must remain explicitly uncertifiable: {drops:#?}"
        );
        // With no visible server entry, the rendering mode is unresolved;
        // absence of the import is not proof that the app is CSR-only.
        if let Some(csr) = diagnostic_fixture("http-response-flush-csr") {
            let unresolved = findings_for_rule(&csr, "http-response-after-flush");
            assert_eq!(unresolved.len(), 2, "{csr:#?}");
            assert!(
                unresolved
                    .iter()
                    .all(|finding| finding["kind"] == "uncertifiable"),
                "{unresolved:#?}"
            );
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
        // Fifteen proven rich/non-JSON primitive values -- thirteen at the
        // top level plus two witnessed inside a closed object literal, since
        // JSON.stringify reaches nested values -- and four explicit
        // obligations. Lone/trailing Uint8Array, compiler-proven JSON-safe
        // primitive domains, and object graphs closed against spreads,
        // computed and duplicate keys, and accessors are all certified.
        //
        // The four obligations are each irreducible for a different reason,
        // and every one is pinned: a broad `number` may be non-finite; a
        // getter's body runs on access, so no written value proves what the
        // property yields; a binding referenced twice could be mutated before
        // the call; and a spread could overwrite the witness.
        assert_rule_findings(&findings, "server-function-rich-argument", 19);
        assert_eq!(
            findings_for_rule(&findings, "server-function-rich-argument")
                .iter()
                .filter(|finding| finding["kind"] == "uncertifiable")
                .count(),
            4,
            "{findings:#?}"
        );
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
        assert_rule_findings(&findings, "resolve-in-tracked-scope", 4);
        assert!(
            findings
                .iter()
                .all(|finding| finding["rule"] == "resolve-in-tracked-scope"),
            "{findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("rc9-until-scope") {
        // rc.9's `until` carries resolve's observer guard, so the same four
        // tracked scopes throw and the same observer-free ones -- plus the
        // action step rc.9 documents -- stay silent.
        assert_rule_findings(&findings, "until-in-tracked-scope", 4);
        // rc.9 is older than the audited rc.13 (ADR 0194): one SC9014 notice
        // beside them.
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            1,
            "{findings:#?}"
        );
        assert!(
            findings.iter().all(|finding| {
                (finding["rule"] == "until-in-tracked-scope" && finding["kind"] == "violation")
                    || finding["rule"] == "unaudited-solid-release"
            }),
            "{findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("rc9-static-dynamic-async") {
        // rc.9's static dynamic() form throws in dev on a thenable source and
        // renders nothing in production. Four inline async sources, the
        // standard library's Promise.resolve and Promise construct signature,
        // and two identifiers resolved to same-file async functions; the
        // synchronous, default-form, unknown-form, shadowed, parameter, `let`
        // and block-bodied sources stay silent.
        assert_rule_findings(&findings, "static-dynamic-async-source", 8);
        assert!(
            findings.iter().all(|finding| {
                (finding["rule"] == "static-dynamic-async-source" && finding["kind"] == "violation")
                    || finding["rule"] == "unaudited-solid-release"
            }),
            "{findings:#?}"
        );
        // rc.3 has no static form: the same calls are the memo's async
        // compute, which settles the Promise.
        if let Some(rc3) = diagnostic_fixture("release-triple-static-dynamic-async-rc3") {
            // Only the older-release notice (ADR 0127).
            assert!(
                rc3.iter()
                    .all(|finding| finding["rule"] == "unaudited-solid-release"),
                "{rc3:#?}"
            );
            assert_eq!(rc3.len(), 1, "{rc3:#?}");
        }
    }
    // SC2006 is keyed on the resolved @solidjs/signals: the FLUSH_IN_ACTION
    // guard ships from rc.8, so the same positives report on rc.8 and rc.9
    // and nothing reports on rc.3, where flush drains inside a
    // step as anywhere else.
    let line = |finding: &serde_json::Value| finding["primaryLocation"]["line"].as_u64();
    for (fixture, lines, notice) in [
        ("rc9-flush-in-action", &[14_u64, 20, 27, 32, 39, 44][..], 1),
        ("release-triple-flush-rc8", &[9, 15][..], 1),
        ("release-triple-flush-rc3", &[][..], 1),
    ] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            continue;
        };
        let flushes = findings_for_rule(&findings, "flush-in-action");
        assert_eq!(
            flushes
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            lines.iter().copied().map(Some).collect::<Vec<_>>(),
            "{fixture}: {findings:#?}"
        );
        assert!(
            flushes
                .iter()
                .all(|finding| finding["id"] == "SC2006" && finding["kind"] == "violation"),
            "{fixture}: {findings:#?}"
        );
        // Beside them only the release notice, which every one of these
        // triples gets: all are older than the audited rc.13 (ADR 0194).
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            notice,
            "{fixture}: {findings:#?}"
        );
        assert_eq!(
            findings.len(),
            lines.len() + notice,
            "{fixture}: {findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("uncalled-accessor-v2") {
        // The positions TypeScript permits: a string-concatenation operand, a
        // logical-not operand, the two unary numeric coercions (`-count` and
        // `~count`, both clean against the published typings), and a template
        // interpolation. The typed positions the 2026-08-17 narrowing dropped
        // -- a class object value, a native attribute, and a computed key --
        // are each a diagnostic of TypeScript's own, and the children
        // attribute and the called and passed-on accessors were already
        // silent. Binary arithmetic and bitwise operands stay silent because
        // TypeScript rejects a function there (TS2365/TS2362).
        assert_rule_findings(&findings, "uncalled-accessor", 5);
        for position in [
            "string concatenation",
            "logical-not operator",
            "numeric coercion",
            "template literal",
        ] {
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

/// rc.9's two new callback forms whose runtime the call shape selects.
///
/// `dynamic(source, { static: true })` is `untrack(source)` at the call, so it
/// owns and writes exactly as `untrack` does; an unproven options value claims
/// nothing. `omit(props, hidden)`'s predicate runs on reads of the returned
/// view, so nothing written inside it is placed in the component body, and a
/// predicate not proven inert is uncertifiable at the predicate argument.
#[test]
fn rc9_call_forms_follow_the_runtime_they_select() {
    let line = |finding: &serde_json::Value| finding["primaryLocation"]["line"].as_u64();
    if let Some(findings) = diagnostic_fixture("rc9-dynamic-static") {
        let owners = findings_for_rule(&findings, "missing-owner");
        // StaticEffect, NamespaceStaticEffect, and the `untrack` reference.
        assert_eq!(
            owners
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            [Some(20), Some(27), Some(33)],
            "{findings:#?}"
        );
        let writes = findings_for_rule(&findings, "reactive-write-in-owned-scope");
        // DefaultWrite, FalseWrite, DeferStreamWrite, BodyStaticWrite; the
        // module-scope static write and both unknown-form writes are silent.
        assert_eq!(
            writes
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            [Some(50), Some(57), Some(62), Some(71)],
            "{findings:#?}"
        );
        // Seven findings, and the SC9014 notice: rc.9 is older than the
        // audited rc.13 (ADR 0194).
        assert_eq!(findings.len(), 8, "{findings:#?}");
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            1,
            "{findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("rc9-omit-predicate") {
        // The only violation is the control: the body read beside a
        // predicate. No predicate body is placed in a component.
        let reads = findings_for_rule(&findings, "strict-read-untracked");
        assert_eq!(
            reads
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            [Some(48)],
            "{findings:#?}"
        );
        // Every predicate not proven inert, each at its argument and each
        // uncertifiable, with the reason it is not proven: a reactive
        // operation (inline read, inline write, named, const arrow, props,
        // forwarded through `hideBy`), a call out of the predicate, or no
        // inspectable body. The inert literal, the standard-library-only
        // literal, the key list and the forwarded parameter itself are silent.
        let predicates = findings_for_rule(&findings, "reactive-dispatch-unresolved");
        fn context(finding: &serde_json::Value) -> Option<&str> {
            finding["analysisContext"]
                .as_str()
                .map(|context| context.trim_start_matches("result-access-callback-"))
        }
        assert_eq!(
            predicates
                .iter()
                .map(|finding| (line(finding), context(finding)))
                .collect::<Vec<_>>(),
            [
                (Some(20), Some("reactive-operation")),
                (Some(27), Some("reactive-operation")),
                (Some(39), Some("reactive-operation")),
                (Some(67), Some("reactive-operation")),
                (Some(73), Some("reactive-operation")),
                (Some(84), Some("opaque-call")),
                (Some(91), Some("body-unresolved")),
                (Some(107), Some("reactive-operation")),
            ],
            "{findings:#?}"
        );
        assert!(
            predicates
                .iter()
                .all(|finding| finding["kind"] == "uncertifiable"),
            "{findings:#?}"
        );
        // The control and the eight predicates, and the SC9014 notice: rc.9
        // is older than the audited rc.13 (ADR 0194). The exported wrapper
        // owes no package obligation: nothing is generated.
        assert!(
            findings_for_rule(&findings, "package-behavior-unavailable").is_empty(),
            "{findings:#?}"
        );
        assert_eq!(findings.len(), 10, "{findings:#?}");
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            1,
            "{findings:#?}"
        );
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
        // The reactive `props.onSave` read, plus the two hyphenated attributes
        // TypeScript deliberately declines to check: one proven invalid and
        // one callable/non-callable proof obligation. Ordinary `onClick`
        // non-callable values remain TS2322-owned.
        ("reactive-handler-frozen", 3),
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
        findings_for_rule(&static_api, "sync-computation-received-async")
            .into_iter()
            .all(|finding| {
                finding["evidence"][0]["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("sync computation"))
            })
    );
}

#[test]
fn create_effect_owner_findings_require_runtime_allocation() {
    let Some(static_api) = diagnostic_fixture("static-api") else {
        return;
    };
    let lines = findings_for_rule(&static_api, "missing-owner")
        .into_iter()
        .map(|finding| finding["primaryLocation"]["line"].as_u64().unwrap())
        .collect::<Vec<_>>();
    // Solid 2 throws before allocating an effect node for an absent apply slot
    // (including the exact one-element tuple spread), undefined, or null apply
    // argument (including cast-hidden null). Other non-callable values
    // allocate first and still create an owner leak.
    assert_eq!(lines, [12, 13, 17, 19, 20, 21, 27, 40]);
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
        // Two: class and generic-function. The object passed to `invoke` and
        // the conditional over two readers name `objectReader`, an exported
        // `const`, which ADR 0320 no longer resolves through its initializer
        // (other files' references to it are outside a retained answer), so
        // those are SC9012 obligations; so are divergent and computed
        // dispatch. This count agrees with the checked-in snapshot.
        ("interprocedural-methods-v2", 2, "count"),
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
    // control-flow: one frozen Show-callback read (the other Show callback
    // takes no parameter, so rc.13 inserts it as a tracked child -- review
    // round 4), plus the two frozen reads under <For keyed={byId}> — a named
    // key function proven callable through type facts keeps the custom-key
    // accessor claims. The dynamic boolean `keyed={flag()}` functions
    // contribute nothing: the callback shape is ambiguous, so neither
    // parameter is claimed as an accessor.
    for (fixture, expected) in [("control-flow", 3), ("execution-phases", 1)] {
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

/// ADR 0202: the default output lists findings about the user's code in full
/// and folds the analysis-coverage gaps into one grouped section; a closed
/// program reports no package obligation for its own exports.
#[test]
fn default_output_groups_coverage_gaps_after_findings_to_review() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    // ADR 0328: the tiers are the maintainer view (`--internal`).
    let run_with = |fixture: &str, format: &str, internal: bool| {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env("SOLID_CHECKER_DAEMON", "0")
            .args(if internal { &["--internal"][..] } else { &[] })
            .args(["--format", format, "--project"])
            .arg(
                root.join("fixtures/reactive-ir")
                    .join(fixture)
                    .join("tsconfig.json"),
            )
            .output()
            .expect("run the checker");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let run = |fixture: &str, format: &str| run_with(fixture, format, true);

    // Users see neither tier: both hold only uncertifiable findings.
    let user = run_with("feedback-tiers-open", "default", false);
    assert!(!user.contains("Needs review"), "{user}");
    assert!(!user.contains("Analysis coverage"), "{user}");
    assert!(!user.contains("SC9012"), "{user}");

    let open = run("feedback-tiers-open", "default");
    let review = open.find("Needs review (1)").expect("review heading");
    let coverage = open
        .find("Analysis coverage: 1 site in 1 group")
        .expect("coverage heading");
    assert!(review < coverage, "{open}");
    // A coverage gap is grouped by family and subject, with its first site,
    // not a frame (ADR 0205).
    assert!(
        open.contains(
            "[SC9012] Helpers that call a method on a value their caller supplies: 1 site in 1 helper"
        ),
        "{open}"
    );
    assert!(open.contains("      ageOf (1)\n"), "{open}");
    assert!(open.contains("first at App.tsx:18:21"), "{open}");
    // Nothing is generated for a package, so an export owes none (D2).
    assert!(!open.contains("SC9005"), "{open}");
    assert!(!open[coverage..].contains(",-["), "{open}");
    // `--format full` keeps every finding in place.
    let full = run("feedback-tiers-open", "full");
    assert!(!full.contains("Analysis coverage"), "{full}");
    assert!(!full.contains("SC9005"), "{full}");

    // The same source in a closed program: no package obligation.
    let closed = run("feedback-tiers", "default");
    assert!(
        closed.contains("Analysis coverage: 1 site in 1 group"),
        "{closed}"
    );
    assert!(!closed.contains("SC9005"), "{closed}");
}
