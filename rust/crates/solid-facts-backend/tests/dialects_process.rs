//! The dialect seam, observed end to end: byte-identical sources, two
//! resolved `solid-js` versions, two different sets of findings — with the
//! dialect chosen by detection, never by a flag.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn dialect_pair_findings(fixture: &str) -> Vec<(String, String, u64)> {
    dialect_snapshot_findings(fixture)
        .iter()
        .map(|finding| {
            (
                finding["id"].as_str().unwrap().to_owned(),
                finding["rule"].as_str().unwrap().to_owned(),
                finding["primaryLocation"]["startByte"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn dialect_snapshot_findings(fixture: &str) -> Vec<serde_json::Value> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    project_snapshot_findings(
        root.join(format!("fixtures/reactive-ir/{fixture}/tsconfig.json")),
        None,
    )
}

fn project_snapshot_findings(project: PathBuf, dialect: Option<&str>) -> Vec<serde_json::Value> {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return Vec::new();
    };
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .arg("--typefacts")
        .arg(&typefacts)
        .arg("--format")
        .arg("json");
    if let Some(dialect) = dialect {
        command.arg("--dialect").arg(dialect);
    }
    let output = command
        .arg("--project")
        .arg(&project)
        .output()
        .expect("run checker");
    assert!(
        output.status.success(),
        "checker failed on {}: {}",
        project.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("snapshot JSON");
    snapshot["findings"]
        .as_array()
        .expect("findings array")
        .clone()
}

#[test]
fn solid_one_function_signal_values_are_not_analyzed_as_callbacks() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let one = dialect_snapshot_findings("dialect-solid-1x");
    assert!(
        one.iter().all(|finding| {
            !finding["message"]
                .as_str()
                .is_some_and(|message| message.contains("storedFunctionSource"))
        }),
        "a stored function was treated as invoked: {one:#?}"
    );
}

#[test]
fn solid_one_resource_overloads_classify_source_and_fetcher_separately() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let one = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-resource-overloads/tsconfig.json"),
        Some("solid-v1"),
    );
    let strict_messages = one
        .iter()
        .filter(|finding| finding["rule"] == "v1/strict-read-untracked")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();
    assert!(
        strict_messages
            .iter()
            .any(|message| message.contains("fetcherDependency")),
        "the one-argument fetcher read was treated as tracked: {one:#?}"
    );
    assert!(
        strict_messages
            .iter()
            .all(|message| !message.contains("sourceDependency")),
        "the two-argument source read was treated as untracked: {one:#?}"
    );
}

#[test]
fn solid_one_array_callbacks_track_lists_but_not_mappers() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let one = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-resource-overloads/tsconfig.json"),
        Some("solid-v1"),
    );
    let strict_messages = one
        .iter()
        .filter(|finding| finding["rule"] == "v1/strict-read-untracked")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();
    for expected in ["mappingStore", "mapIndex", "indexedItem"] {
        assert!(
            strict_messages
                .iter()
                .any(|message| message.contains(expected)),
            "missing untracked mapper source {expected}: {one:#?}"
        );
    }
    assert!(
        strict_messages
            .iter()
            .all(|message| !message.contains("trackedListStore")),
        "the tracked list accessor was treated as an untracked mapper: {one:#?}"
    );
    for dormant in [
        "discardedMapperStore",
        "discardedMapIndex",
        "discardedIndexedItem",
        "memberMapperStore",
    ] {
        assert!(
            strict_messages
                .iter()
                .all(|message| !message.contains(dormant)),
            "a callback of a discarded array adapter was treated as reachable for {dormant}: {one:#?}"
        );
    }
    assert_eq!(
        strict_messages
            .iter()
            .filter(|message| message.contains("mapped array"))
            .count(),
        3,
        "direct, immediate, and mixed untracked adapter calls must each read the contracted mapped-array accessor: {one:#?}"
    );
}

#[test]
fn solid_one_reaction_leaf_owner_requires_invoking_returned_tracker() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let one = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-resource-overloads/tsconfig.json"),
        Some("solid-v1"),
    );
    let leaf_owner_findings = one
        .iter()
        .filter(|finding| finding["id"] == "SC3001")
        .collect::<Vec<_>>();
    assert_eq!(
        leaf_owner_findings.len(),
        1,
        "only the invoked reaction tracker can reach its invalidation callback: {one:#?}"
    );
}

#[test]
fn solid_one_reaction_tracker_argument_is_a_tracked_computation() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let one = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-resource-overloads/tsconfig.json"),
        Some("solid-v1"),
    );
    let strict_messages = one
        .iter()
        .filter(|finding| finding["rule"] == "v1/strict-read-untracked")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();
    assert!(
        ["reactionSource", "aliasedReactionSource"]
            .iter()
            .all(|source| strict_messages
                .iter()
                .all(|message| !message.contains(source))),
        "the returned reaction tracker must track its callback argument: {one:#?}"
    );
    assert!(
        one.iter().any(|finding| {
            finding["id"] == "SC2001"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("setAliasedReactionSource"))
        }),
        "the named callback passed through a tracker alias was not analyzed as tracked: {one:#?}"
    );
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-resource-overloads/App.tsx"),
    )
    .unwrap();
    let cleanup_start = source
        .find("onCleanup(() => {});\n  };\n  aliasedTracker")
        .unwrap() as u64;
    assert!(
        one.iter().all(|finding| {
            finding["id"] != "SC4002" || finding["primaryLocation"]["startByte"] != cleanup_start
        }),
        "the returned reaction tracker creates the computation owner for its tracking callback: {one:#?}"
    );
}

#[test]
fn solid_one_value_function_proof_distinguishes_dormant_values_from_iifes() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let one = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-resource-overloads/tsconfig.json"),
        Some("solid-v1"),
    );
    let strict_messages = one
        .iter()
        .filter(|finding| finding["rule"] == "v1/strict-read-untracked")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();
    assert!(
        strict_messages
            .iter()
            .all(|message| !message.contains("nestedDormantSource")),
        "a nested closure inside a stored function was treated as reachable: {one:#?}"
    );
    assert!(
        strict_messages
            .iter()
            .any(|message| message.contains("evaluatedSource")),
        "an immediately invoked function was mistaken for a stored value: {one:#?}"
    );
}

#[test]
fn solid_one_never_emits_a_two_only_sync_option_identity() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let one = dialect_snapshot_findings("dialect-solid-1x");
    assert!(
        one.iter().all(|finding| finding["id"] != "SC7002"),
        "the 1.x engine emitted a 2.0-only diagnostic: {one:#?}"
    );
}

#[test]
fn solid_one_context_provider_values_follow_the_resolved_runtime_contract() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/tsconfig.json"),
        Some("solid-v1"),
    );
    let strict_messages = findings
        .iter()
        .filter(|finding| finding["rule"] == "v1/strict-read-untracked")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();

    for expected in ["contextValue", "contextSignal"] {
        assert!(
            strict_messages
                .iter()
                .any(|message| message.contains(expected)),
            "missing untracked Context.Provider value read {expected}: {findings:#?}"
        );
    }
    assert!(
        strict_messages
            .iter()
            .all(|message| !message.contains("ordinaryValue")),
        "a component was treated as a context provider from its name: {findings:#?}"
    );
}

#[test]
fn solid_one_context_provider_contract_follows_cross_file_symbols() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/tsconfig.json"),
        Some("solid-v1"),
    );
    let strict_messages = findings
        .iter()
        .filter(|finding| finding["rule"] == "v1/strict-read-untracked")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();
    for expected in [
        "sharedContextValue",
        "reexportContextValue",
        "namespaceContextValue",
    ] {
        assert!(
            strict_messages
                .iter()
                .any(|message| message.contains(expected)),
            "a cross-file createContext binding lost its Provider runtime contract for {expected}: {findings:#?}"
        );
    }
    assert!(
        strict_messages
            .iter()
            .all(|message| !message.contains("dormantContextSignal")),
        "a function stored as a context value was treated as immediately invoked: {findings:#?}"
    );
}

#[test]
fn solid_one_directive_names_use_compiler_scope_resolution() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/tsconfig.json"),
        Some("solid-v1"),
    );
    let undefined_messages = findings
        .iter()
        .filter(|finding| finding["rule"] == "v1/jsx-no-undef")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();

    assert!(
        undefined_messages
            .iter()
            .any(|message| message.contains("missingDirective")),
        "an unresolved custom directive was not reported: {findings:#?}"
    );
    assert!(
        undefined_messages
            .iter()
            .all(|message| !message.contains("definedDirective")),
        "a compiler-resolved custom directive was reported undefined: {findings:#?}"
    );
}

#[test]
fn solid_one_direct_imports_fall_back_to_reviewed_package_contracts() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/tsconfig.json"),
        Some("solid-v1"),
    );
    for expected in ["externalValue", "renderDependency"] {
        assert!(
            findings.iter().any(|finding| {
                finding["rule"] == "v1/strict-read-untracked"
                    && finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains(expected))
            }),
            "a reviewed direct-import contract fact was discarded for {expected}: {findings:#?}"
        );
    }
}

#[test]
fn solid_one_reviewed_package_contracts_follow_local_reexports() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/tsconfig.json"),
        Some("solid-v1"),
    );
    assert!(
        findings.iter().any(|finding| {
            finding["rule"] == "v1/strict-read-untracked"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("reexportedMemoValue"))
        }),
        "the reviewed `memo` return contract did not follow its TypeScript symbol through a local re-export: {findings:#?}"
    );
}

#[test]
fn solid_one_native_tuple_contracts_discover_their_accessor_slot() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/tsconfig.json"),
        Some("solid-v1"),
    );
    assert!(
        findings.iter().any(|finding| {
            finding["rule"] == "v1/strict-read-untracked"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("transitionPending"))
        }),
        "useTransition's first tuple slot is a runtime accessor even though the flat package return schema cannot express it: {findings:#?}"
    );
}

#[test]
fn solid_one_transition_starter_restores_the_callers_tracking_and_owner() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/tsconfig.json"),
        Some("solid-v1"),
    );
    assert!(
        findings.iter().any(|finding| {
            finding["rule"] == "v1/strict-read-untracked"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("topLevelTransitionRead"))
        }),
        "a top-level transition callback must inherit the certainly untracked caller: {findings:#?}"
    );
    assert!(
        findings.iter().any(|finding| {
            finding["id"] == "SC2001"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("setTrackedTransitionRead"))
        }),
        "the aliased transition starter must restore its enclosing effect's listener: {findings:#?}"
    );
    assert!(
        findings.iter().all(|finding| {
            finding["message"]
                .as_str()
                .is_none_or(|message| !message.contains("trackedTransitionRead()"))
        }),
        "the transition callback's tracked read was classified as untracked: {findings:#?}"
    );

    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/App.tsx"),
    )
    .unwrap();
    let region_start = source.find("const [topLevelTransitionRead").unwrap() as u64;
    let region_end = source
        .find("export function renderOwnsMountedWork")
        .unwrap() as u64;
    let cleanup_findings = findings
        .iter()
        .filter(|finding| finding["id"] == "SC4002")
        .filter(|finding| {
            finding["primaryLocation"]["startByte"]
                .as_u64()
                .is_some_and(|start| start >= region_start && start < region_end)
        })
        .count();
    assert_eq!(
        cleanup_findings, 1,
        "only the top-level transition callback is unowned; the effect-contained alias inherits its owner: {findings:#?}"
    );
}

#[test]
fn solid_one_web_mount_callbacks_run_under_their_disposal_root() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/solid-1x-evidence-contracts");
    let source = std::fs::read_to_string(fixture.join("App.tsx")).expect("read fixture");
    let region_start = source
        .find("// Top-level execution is certainly unowned")
        .expect("render owner tracer") as u64;
    let region_end = source
        .find("// Top-level from producers")
        .expect("from owner tracer") as u64;
    let findings = project_snapshot_findings(fixture.join("tsconfig.json"), Some("solid-v1"));
    let impossible = findings
        .iter()
        .filter(|finding| matches!(finding["id"].as_str(), Some("SC4001" | "SC4002")))
        .filter(|finding| {
            finding["primaryLocation"]["startByte"]
                .as_u64()
                .is_some_and(|start| start >= region_start && start < region_end)
        })
        .collect::<Vec<_>>();
    assert!(
        impossible.is_empty(),
        "render/hydrate create the root that owns work in their callbacks: {impossible:#?}"
    );
}

#[test]
fn solid_one_from_producer_inherits_its_callers_owner() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/solid-1x-evidence-contracts");
    let source = std::fs::read_to_string(fixture.join("App.tsx")).expect("read fixture");
    let invalid_cleanup = source
        .find("// Top-level from producers")
        .and_then(|start| {
            source[start..]
                .find("onCleanup")
                .map(|offset| start + offset)
        })
        .expect("top-level from cleanup") as u64;
    let valid_cleanup = source
        .find("export function FromProducerInheritsComponentOwner")
        .and_then(|start| {
            source[start..]
                .find("onCleanup")
                .map(|offset| start + offset)
        })
        .expect("component from cleanup") as u64;
    let findings = project_snapshot_findings(fixture.join("tsconfig.json"), Some("solid-v1"));
    let owner_findings = findings
        .iter()
        .filter(|finding| finding["id"] == "SC4002")
        .filter(|finding| finding["primaryLocation"]["startByte"] == invalid_cleanup)
        .collect::<Vec<_>>();
    assert_eq!(
        owner_findings.len(),
        1,
        "the top-level producer is certainly unowned, while the component producer inherits its owner: {findings:#?}"
    );
    assert!(
        findings.iter().all(|finding| {
            finding["id"] != "SC4002" || finding["primaryLocation"]["startByte"] != valid_cleanup
        }),
        "the component producer must retain its caller's owner: {findings:#?}"
    );
}

#[test]
fn solid_one_web_effect_alias_retains_render_effect_ownership() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/solid-1x-evidence-contracts");
    let source = std::fs::read_to_string(fixture.join("App.tsx")).expect("read fixture");
    let effect_start = source
        .find("effect(() => {\n  onCleanup")
        .expect("top-level web effect") as u64;
    let cleanup_start = source[usize::try_from(effect_start).unwrap()..]
        .find("onCleanup")
        .map(|offset| effect_start + offset as u64)
        .expect("effect cleanup");
    let findings = project_snapshot_findings(fixture.join("tsconfig.json"), Some("solid-v1"));
    assert!(
        findings.iter().any(|finding| {
            finding["id"] == "SC4001" && finding["primaryLocation"]["startByte"] == effect_start
        }),
        "the web effect alias created an ownerless top-level effect without a diagnostic: {findings:#?}"
    );
    assert!(
        findings.iter().all(|finding| {
            finding["id"] != "SC4002" || finding["primaryLocation"]["startByte"] != cleanup_start
        }),
        "the effect computation owns cleanup registered inside its callback: {findings:#?}"
    );
}

#[test]
fn solid_one_web_derived_helpers_retain_their_computation_contracts() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/solid-1x-evidence-contracts");
    let source = std::fs::read_to_string(fixture.join("App.tsx")).expect("read fixture");
    let region_start = source
        .find("const [webMemoSource")
        .expect("web memo tracer") as u64;
    let region_end = source
        .find("const [topLevelBatchSource")
        .expect("end of web helper tracer") as u64;
    let findings = project_snapshot_findings(fixture.join("tsconfig.json"), Some("solid-v1"));
    for setter in ["setWebMemoSource", "setDynamicComponentSource"] {
        assert!(
            findings.iter().any(|finding| {
                finding["id"] == "SC2001"
                    && finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains(setter))
            }),
            "{setter} runs in a tracked memo computation: {findings:#?}"
        );
    }
    assert!(
        findings.iter().all(|finding| {
            finding["id"] != "SC4002"
                || finding["primaryLocation"]["startByte"]
                    .as_u64()
                    .is_none_or(|start| start < region_start || start >= region_end)
        }),
        "memo/createDynamic computations own cleanup in their callbacks: {findings:#?}"
    );
}

#[test]
fn solid_one_higher_order_helpers_compose_tracking_reachability_and_owner() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/solid-1x-evidence-contracts");
    let source = std::fs::read_to_string(fixture.join("App.tsx")).expect("read fixture");
    let findings = project_snapshot_findings(fixture.join("tsconfig.json"), Some("solid-v1"));
    let strict_messages = findings
        .iter()
        .filter(|finding| finding["id"] == "SC1001")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();
    assert!(
        strict_messages
            .iter()
            .any(|message| message.contains("topLevelBatchSource")),
        "batch preserves rather than manufactures a tracking Listener: {findings:#?}"
    );
    for non_strict in [
        "trackedBatchSource",
        "childrenSource",
        "onDependency",
        "onBodySource",
        "discardedOnSource",
        "discardedProduceSource",
    ] {
        assert!(
            strict_messages
                .iter()
                .all(|message| !message.contains(non_strict)),
            "unexpected strict read for {non_strict}: {findings:#?}"
        );
    }
    assert!(
        strict_messages
            .iter()
            .any(|message| message.contains("invokedProduceSource")),
        "modifyMutable invokes the producer in its top-level caller context: {findings:#?}"
    );

    let write_messages = findings
        .iter()
        .filter(|finding| finding["id"] == "SC2001")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();
    for tracked_setter in [
        "setTrackedBatchSource",
        "setChildrenSource",
        "setOnDependency",
    ] {
        assert!(
            write_messages
                .iter()
                .any(|message| message.contains(tracked_setter)),
            "missing tracked write for {tracked_setter}: {findings:#?}"
        );
    }
    assert!(
        write_messages
            .iter()
            .all(|message| !message.contains("setOnBodySource")),
        "on's body is intentionally untracked after its explicit dependency read: {findings:#?}"
    );

    let cleanup_at = |source_name: &str| {
        let start = source
            .find(source_name)
            .expect("higher-order source marker");
        source[start..]
            .find("onCleanup")
            .map(|offset| (start + offset) as u64)
            .expect("higher-order cleanup")
    };
    let cleanup_findings = findings
        .iter()
        .filter(|finding| finding["id"] == "SC4002")
        .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
        .collect::<Vec<_>>();
    for owned in ["childrenSource", "onBodySource"] {
        assert!(
            !cleanup_findings.contains(&cleanup_at(owned)),
            "{owned} runs under a created computation owner: {findings:#?}"
        );
    }
    assert!(
        cleanup_findings.contains(&cleanup_at("invokedProduceSource")),
        "a top-level producer invocation inherits the absent caller owner: {findings:#?}"
    );
}

#[test]
fn solid_one_selector_distinguishes_its_eager_source_from_its_lazy_comparator() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/tsconfig.json"),
        Some("solid-v1"),
    );
    let messages = findings
        .iter()
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();

    let dormant = "discardedSelectorComparatorRead";
    assert!(
        messages.iter().all(|message| !message.contains(dormant)),
        "a discarded selector made its comparator reachable: {findings:#?}"
    );
    assert!(
        messages
            .iter()
            .all(|message| !message.contains("selectorSource")),
        "the selector source must remain tracked even though its comparator is lazy: {findings:#?}"
    );
    assert!(
        findings.iter().any(|finding| {
            finding["rule"] == "v1/strict-read-untracked"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("topLevelSelectorComparatorRead"))
        }),
        "a proven top-level selector call must expose the comparator's untracked read: {findings:#?}"
    );
    assert!(
        findings.iter().any(|finding| {
            finding["id"] == "SC2001"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("setTrackedSelectorComparatorRead"))
        }),
        "a selector comparator invoked through an alias in an effect must inherit tracking: {findings:#?}"
    );
    assert!(
        findings
            .iter()
            .filter(|finding| finding["rule"] == "v1/strict-read-untracked")
            .all(|finding| {
                finding["message"]
                    .as_str()
                    .is_none_or(|message| !message.contains("trackedSelectorComparatorRead"))
            }),
        "the tracked selector comparator read was classified as untracked: {findings:#?}"
    );
}

#[test]
fn solid_one_catch_error_body_preserves_tracking_under_its_created_owner() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/tsconfig.json"),
        Some("solid-v1"),
    );
    assert!(
        findings.iter().any(|finding| {
            finding["rule"] == "v1/strict-read-untracked"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("topLevelCatchSource"))
        }),
        "catchError must not fabricate tracking around a top-level protected body: {findings:#?}"
    );
    assert!(
        findings.iter().all(|finding| {
            finding["message"]
                .as_str()
                .is_none_or(|message| !message.contains("trackedCatchSource()"))
        }),
        "catchError must preserve the enclosing effect's tracking listener: {findings:#?}"
    );
    assert!(
        findings.iter().any(|finding| {
            finding["id"] == "SC2001"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("setTrackedCatchSource"))
        }),
        "a write in a catchError body nested in an effect is still a tracked write: {findings:#?}"
    );

    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/App.tsx"),
    )
    .unwrap();
    let region_start = source.find("const [topLevelCatchSource").unwrap() as u64;
    let region_end = source.find("const [childrenSource").unwrap() as u64;
    assert!(
        findings.iter().all(|finding| {
            finding["id"] != "SC4002"
                || finding["primaryLocation"]["startByte"]
                    .as_u64()
                    .is_none_or(|start| start < region_start || start >= region_end)
        }),
        "catchError's protected body runs under the computation owner the runtime creates: {findings:#?}"
    );
}

#[test]
fn solid_one_lazy_loader_requires_a_proven_component_or_preload_invocation() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/solid-1x-evidence-contracts");
    let source = std::fs::read_to_string(fixture.join("App.tsx")).expect("read fixture");
    let findings = project_snapshot_findings(fixture.join("tsconfig.json"), Some("solid-v1"));

    let strict_messages = findings
        .iter()
        .filter(|finding| finding["id"] == "SC1001")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();
    assert!(
        strict_messages
            .iter()
            .all(|message| !message.contains("discardedLazySource")),
        "constructing and discarding a lazy component cannot execute its loader: {findings:#?}"
    );
    for invoked in [
        "immediateLazySource",
        "jsxLazySource",
        "crossFileLazySource",
        "preloadedLazySource",
    ] {
        assert!(
            strict_messages
                .iter()
                .any(|message| message.contains(invoked)),
            "the proven lazy invocation must expose the untracked loader read {invoked}: {findings:#?}"
        );
    }

    let cleanup_at = |source_name: &str| {
        let source_start = source.find(source_name).expect("lazy source marker");
        source[source_start..]
            .find("onCleanup")
            .map(|offset| (source_start + offset) as u64)
            .expect("lazy cleanup")
    };
    let cleanup_findings = findings
        .iter()
        .filter(|finding| finding["id"] == "SC4002")
        .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
        .collect::<Vec<_>>();
    assert!(
        !cleanup_findings.contains(&cleanup_at("discardedLazySource")),
        "a dormant loader cannot register cleanup: {findings:#?}"
    );
    assert!(
        cleanup_findings.contains(&cleanup_at("immediateLazySource")),
        "an immediately invoked top-level lazy component inherits no owner: {findings:#?}"
    );
    assert!(
        !cleanup_findings.contains(&cleanup_at("jsxLazySource")),
        "a JSX component invocation supplies the owner inherited by its lazy loader: {findings:#?}"
    );
    let cross_file_source = std::fs::read_to_string(fixture.join("lazy-component.tsx"))
        .expect("read cross-file lazy fixture");
    let cross_file_cleanup = cross_file_source
        .find("onCleanup")
        .expect("cross-file cleanup") as u64;
    assert!(
        findings.iter().all(|finding| {
            finding["id"] != "SC4002"
                || finding["primaryLocation"]["path"]
                    .as_str()
                    .is_none_or(|path| !path.ends_with("lazy-component.tsx"))
                || finding["primaryLocation"]["startByte"] != cross_file_cleanup
        }),
        "a cross-file JSX invocation must preserve its component owner: {findings:#?}"
    );
    assert!(
        cleanup_findings.contains(&cleanup_at("preloadedLazySource")),
        "top-level preload invokes the loader without an owner: {findings:#?}"
    );
}

/// An adapter invoked inside its own factory callback makes execution
/// classification cyclic; before the classification stack existed, this
/// fixture overflowed the checker's stack. The cyclic site contributes no
/// context, so the read still classifies from the acyclic invocations.
#[test]
fn solid_one_cyclic_adapter_invocations_terminate_and_classify() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-evidence-contracts/tsconfig.json"),
        Some("solid-v1"),
    );
    let strict_messages = findings
        .iter()
        .filter(|finding| finding["rule"] == "v1/strict-read-untracked")
        .filter_map(|finding| finding["message"].as_str())
        .collect::<Vec<_>>();
    assert!(
        strict_messages
            .iter()
            .any(|message| message.contains("cyclicAdapterSource")),
        "the self-invoking adapter's acyclic top-level call proves its dependency read untracked: {findings:#?}"
    );
    assert!(
        strict_messages
            .iter()
            .all(|message| !message.contains("mutualAdapterSource")),
        "the mutual adapters' only acyclic execution context is a tracked effect: {findings:#?}"
    );
}

/// The pair is duplicated source on purpose: the two fixture projects differ
/// only in the `solid-js` version their `node_modules` resolves, so every
/// difference between their findings is the dialect's doing.
#[test]
fn the_dialect_pair_reports_different_findings_from_identical_sources() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    // Pin the premise before consuming it: if the pair's sources drift, the
    // finding diff below stops meaning "the dialect changed the answer".
    // (`scripts/coverage.mjs` enforces the same identity; this assert keeps
    // the test self-contained when run without the snapshot gate.)
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/reactive-ir");
    for file in ["App.tsx", "solid-js.d.ts", "tsconfig.json"] {
        assert_eq!(
            std::fs::read_to_string(fixtures.join("dialect-solid-1x").join(file)).unwrap(),
            std::fs::read_to_string(fixtures.join("dialect-solid-2").join(file)).unwrap(),
            "{file} drifted between the dialect pair projects"
        );
    }

    let one = dialect_pair_findings("dialect-solid-1x");
    let two = dialect_pair_findings("dialect-solid-2");
    assert_ne!(one, two, "the dialects agreed on everything");

    // Every 1.x finding names the dialect that produced it.
    for (_, rule, _) in &one {
        assert!(
            rule.starts_with("v1/"),
            "1.x finding {rule} must carry the v1/ namespace"
        );
    }
    for (_, rule, _) in &two {
        assert!(
            !rule.starts_with("v1/"),
            "2.0 finding {rule} must stay unprefixed"
        );
    }

    // The headline arity difference: `createEffect(() => ...)` is the 1.x
    // signature, so only 2.0 reports the one-argument call (byte 1197); both
    // report `createEffect(undefined)` (byte 1541).
    let sc7001 = |findings: &[(String, String, u64)]| {
        findings
            .iter()
            .filter(|(code, _, _)| code == "SC7001")
            .map(|(_, _, byte)| *byte)
            .collect::<Vec<_>>()
    };
    assert_eq!(sc7001(&one), vec![1541]);
    assert_eq!(sc7001(&two), vec![1197, 1541]);

    // createReaction is a leaf owner only in 1.x: onCleanup inside its
    // callback is a 1.x finding and 2.0 silence.
    assert!(one.iter().any(|(code, _, _)| code == "SC3001"));
    assert!(!two.iter().any(|(code, _, _)| code == "SC3001"));
}

/// An explicit `--dialect` beats detection: the 1.x fixture analyzed as 2.0
/// reports the 2.0 shape.
#[test]
fn an_explicit_dialect_overrides_detection() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .arg("--typefacts")
        .arg(&typefacts)
        .arg("--dialect")
        .arg("solid-v2")
        .arg("--format")
        .arg("json")
        .arg("--project")
        .arg(root.join("fixtures/reactive-ir/dialect-solid-1x/tsconfig.json"))
        .output()
        .expect("run checker");
    assert!(output.status.success());
    let snapshot: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("snapshot JSON");
    for finding in snapshot["findings"].as_array().unwrap() {
        let rule = finding["rule"].as_str().unwrap();
        assert!(!rule.starts_with("v1/"), "explicit v2 produced {rule}");
    }
}
