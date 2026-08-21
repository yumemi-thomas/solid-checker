//! The dialect seam, observed end to end: byte-identical sources, two
//! resolved `solid-js` versions, two different sets of findings — with the
//! dialect chosen by detection, never by a flag.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn dialect_pair_findings(fixture: &str) -> Vec<(String, String, String, u64)> {
    dialect_snapshot_findings(fixture)
        .iter()
        .map(|finding| {
            (
                finding["id"].as_str().unwrap().to_owned(),
                finding["rule"].as_str().unwrap().to_owned(),
                finding["kind"].as_str().unwrap().to_owned(),
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
    project_snapshot_findings_with(project, dialect, &[])
}

fn project_snapshot_findings_with(
    project: PathBuf,
    dialect: Option<&str>,
    extra_args: &[&str],
) -> Vec<serde_json::Value> {
    // Callers skip when the harness is unarmed; reaching this helper without
    // the producer is a test bug, and an empty result here would let every
    // `all(...)`-shaped assertion pass vacuously.
    let typefacts = env::var("SOLID_TYPEFACTS_BIN")
        .expect("guard the calling test on SOLID_TYPEFACTS_BIN before requesting findings");
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .arg("--typefacts")
        .arg(&typefacts)
        .arg("--format")
        .arg("json");
    if let Some(dialect) = dialect {
        command.arg("--dialect").arg(dialect);
    }
    command.args(extra_args);
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
fn preferences_are_default_on_with_explicit_disables_winning() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let v2 = fixture_root.join("preferences-v2/tsconfig.json");
    let preference_findings = |findings: Vec<serde_json::Value>| {
        findings
            .into_iter()
            .filter(|finding| matches!(finding["id"].as_str(), Some("SC8014" | "SC8015")))
            .collect::<Vec<_>>()
    };

    let defaults = preference_findings(project_snapshot_findings_with(
        v2.clone(),
        Some("solid-v2"),
        &[],
    ));
    let preset = preference_findings(project_snapshot_findings_with(
        v2.clone(),
        Some("solid-v2"),
        &["--preset", "preferences"],
    ));
    assert_eq!(
        defaults, preset,
        "the compatibility preset must be redundant once preferences are default-enabled"
    );
    assert_eq!(
        defaults
            .iter()
            .filter(|finding| finding["id"] == "SC8014")
            .count(),
        5,
        "array Type Facts plus direct, prop-accessor, interprocedural, and v2 async facts select five lists: {defaults:#?}"
    );
    assert_eq!(
        defaults
            .iter()
            .filter(|finding| finding["id"] == "SC8015")
            .count(),
        5,
        "per-prop caller facts must keep the static sibling clean: {defaults:#?}"
    );
    assert!(
        defaults
            .iter()
            .all(|finding| finding["kind"] == "violation")
    );
    let v2_source = std::fs::read_to_string(fixture_root.join("preferences-v2/App.tsx"))
        .expect("read v2 preference fixture");
    let starts = defaults
        .iter()
        .map(|finding| finding["primaryLocation"]["startByte"].as_u64().unwrap())
        .collect::<Vec<_>>();
    let marker = |source: &str| {
        u64::try_from(v2_source.find(source).expect("fixture marker")).expect("offset fits u64")
    };
    let accessor_component = v2_source
        .find("function AccessorProps")
        .expect("fixture anchor");
    let accessor_map = accessor_component
        + v2_source[accessor_component..]
            .find("props.items().map")
            .expect("accessor prop marker");
    assert!(starts.contains(&u64::try_from(accessor_map).expect("offset fits u64")));
    assert!(starts.contains(&marker("derivedItems().map")));
    assert!(!starts.contains(&marker("props.staticReady &&")));
    assert!(!starts.contains(&marker("customCollection().map")));
    let async_map = marker("items().map(async");
    assert!(starts.contains(&async_map));
    assert!(defaults.iter().any(|finding| {
        finding["primaryLocation"]["startByte"].as_u64() == Some(async_map)
            && finding["fixes"].as_array().is_none_or(Vec::is_empty)
    }));
    let v2_for_fix_texts = defaults
        .iter()
        .filter(|finding| finding["rule"] == "prefer-for")
        .flat_map(|finding| finding["fixes"].as_array().into_iter().flatten())
        .flat_map(|fix| fix["edits"].as_array().into_iter().flatten())
        .filter_map(|edit| edit["newText"].as_str())
        .collect::<Vec<_>>();
    assert!(!v2_for_fix_texts.is_empty());
    assert!(
        v2_for_fix_texts
            .iter()
            .all(|text| !text.contains("keyed={false}"))
    );
    assert!(
        v2_for_fix_texts
            .iter()
            .any(|text| text.contains("import { For as __SolidCheckerFor"))
    );

    let explicit_enable = preference_findings(project_snapshot_findings_with(
        v2,
        Some("solid-v2"),
        &["--enable-rule", "prefer-show"],
    ));
    assert_eq!(
        explicit_enable, defaults,
        "explicitly enabling an already-default rule must be idempotent"
    );

    let v2_disabled = preference_findings(project_snapshot_findings_with(
        fixture_root.join("preferences-v2-disabled/tsconfig.json"),
        Some("solid-v2"),
        &[],
    ));
    assert!(
        v2_disabled.is_empty(),
        "explicit v2 disables must win over catalog defaults: {v2_disabled:#?}"
    );

    let enabled = preference_findings(project_snapshot_findings_with(
        fixture_root.join("preferences-v1-enabled/tsconfig.json"),
        Some("solid-v1"),
        &[],
    ));
    assert_eq!(
        enabled
            .iter()
            .filter(|finding| finding["id"] == "SC8014")
            .count(),
        2,
        "v1 reports only receivers Type Facts prove are arrays: {enabled:#?}"
    );
    assert_eq!(
        enabled
            .iter()
            .filter(|finding| finding["id"] == "SC8015")
            .count(),
        3,
        "v1 preferences must not promote uncertain prop backing into proof: {enabled:#?}"
    );
    assert!(enabled.iter().all(|finding| finding["kind"] == "violation"));
    let v1_for_fix_texts = enabled
        .iter()
        .filter(|finding| finding["rule"] == "v1/prefer-for")
        .flat_map(|finding| finding["fixes"].as_array().into_iter().flatten())
        .flat_map(|fix| fix["edits"].as_array().into_iter().flatten())
        .filter_map(|edit| edit["newText"].as_str())
        .collect::<Vec<_>>();
    assert!(!v1_for_fix_texts.is_empty());
    assert!(
        v1_for_fix_texts
            .iter()
            .all(|text| !text.contains("keyed={false}"))
    );
    assert!(
        v1_for_fix_texts
            .iter()
            .any(|text| text.contains("import { For as __SolidCheckerFor"))
    );

    let v1_disabled = preference_findings(project_snapshot_findings_with(
        fixture_root.join("preferences-v1-disabled/tsconfig.json"),
        Some("solid-v1"),
        &[],
    ));
    assert!(
        v1_disabled.is_empty(),
        "explicit v1 disables must win over catalog defaults: {v1_disabled:#?}"
    );
}

#[test]
fn disabling_a_specific_owner_restores_its_strict_read_findings() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    type FindingMarker<'a> = (&'a str, &'a str);
    type DisabledOwnerCase<'a> = (&'a str, &'a str, &'a [FindingMarker<'a>]);
    let cases: [DisabledOwnerCase<'_>; 3] = [
        (
            "disabled-component-owner-restores-strict",
            "SC1004",
            &[
                ("function NestedAttrTernary", "cond()"),
                ("function LogicalReturn", "visible()"),
                ("function SwitchReturn", "mode()"),
            ],
        ),
        (
            "disabled-handler-owner-restores-strict",
            "SC1007",
            &[("function ReactiveCard", "props.onSave")],
        ),
        (
            "disabled-pending-owner-restores-strict",
            "SC5001",
            &[
                ("export function BadDirect", "user().name"),
                ("export function BadSignalDirect", "signalUser().name"),
                (
                    "export function BadDeclaredUntracked",
                    "declaredFeed().name",
                ),
                (
                    "export function OpaqueOptionsUntracked",
                    "opaqueUser().name",
                ),
            ],
        ),
    ];

    for (fixture, disabled_owner, markers) in cases {
        let source_path = if disabled_owner == "SC5001" {
            fixture_root.join("../../../../../fixtures/reactive-ir/async-boundary/App.tsx")
        } else {
            fixture_root.join("../../../../../fixtures/reactive-ir/props-callers/App.tsx")
        };
        let source = std::fs::read_to_string(&source_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", source_path.display()));
        let expected_starts = markers.iter().map(|(anchor, marker)| {
            let anchor_start = source.find(anchor).unwrap_or_else(|| {
                panic!("missing anchor {anchor:?} in {}", source_path.display())
            });
            let relative = source[anchor_start..].find(marker).unwrap_or_else(|| {
                panic!(
                    "missing marker {marker:?} after {anchor:?} in {}",
                    source_path.display()
                )
            });
            u64::try_from(anchor_start + relative).expect("source offset fits u64")
        });
        let findings = project_snapshot_findings(
            fixture_root.join(fixture).join("tsconfig.json"),
            Some("solid-v2"),
        );
        assert!(
            findings
                .iter()
                .all(|finding| finding["id"] != disabled_owner),
            "disabled owner {disabled_owner} still reported in {fixture}: {findings:#?}"
        );
        let strict_starts = findings
            .iter()
            .filter(|finding| finding["id"] == "SC1001")
            .map(|finding| finding["primaryLocation"]["startByte"].as_u64().unwrap())
            .collect::<Vec<_>>();
        for expected in expected_starts {
            assert!(
                strict_starts.contains(&expected),
                "disabling {disabled_owner} did not restore SC1001 at {expected} in {fixture}: {findings:#?}"
            );
        }
    }
}

#[test]
fn project_rule_options_disable_one_exact_catalog_rule() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/rule-options-enablement/tsconfig.json"),
        Some("solid-v1"),
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding["rule"] == "v1/missing-owner"),
        "the enabled control rule should still report: {findings:#?}"
    );
    assert!(
        findings
            .iter()
            .all(|finding| finding["rule"] != "v1/reactive-write-in-owned-scope"),
        "the exact disabled rule still reported: {findings:#?}"
    );
}

#[test]
fn solid_one_merge_props_function_sources_are_tracked() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/merge-props-function-v1/tsconfig.json"),
        Some("solid-v1"),
    );
    assert!(
        findings.iter().all(|finding| {
            !matches!(finding["rule"].as_str(), Some("v1/strict-read-untracked"))
        }),
        "mergeProps wraps every function source in a tracked createMemo: {findings:#?}"
    );
}

#[test]
fn component_ref_callbacks_are_setup_time_outputs_in_both_dialects() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/component-ref/tsconfig.json");
    for dialect in ["solid-v1", "solid-v2"] {
        let findings = project_snapshot_findings(project.clone(), Some(dialect));
        assert!(
            findings
                .iter()
                .all(|finding| finding["rule"] != "strict-read-untracked"
                    && finding["rule"] != "v1/strict-read-untracked"),
            "calling a component ref installs an imperative handle in {dialect}: {findings:#?}"
        );
    }
}

#[test]
fn returned_event_handler_factories_preserve_deferred_execution() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/returned-handler-factory/tsconfig.json");
    for dialect in ["solid-v1", "solid-v2"] {
        let findings = project_snapshot_findings(project.clone(), Some(dialect));
        assert!(
            findings.is_empty(),
            "{dialect} should trace the returned inner handler to the JSX event: {findings:#?}"
        );
    }
}

#[test]
fn component_identity_combines_type_facts_with_dialect_compatibility() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/semantic-component-identity");
    let source = std::fs::read_to_string(fixture.join("App.tsx")).unwrap();
    let typed_offset = u64::try_from(source.find("setCount(1)").unwrap()).unwrap();
    let mut component_prop_patterns = ["{ homeName }", "{ nestedName }", "{ spreadName }"]
        .map(|pattern| u64::try_from(source.find(pattern).unwrap()).unwrap())
        .to_vec();
    component_prop_patterns.sort_unstable();
    for (dialect, expected) in [("solid-v2", vec![typed_offset]), ("solid-v1", vec![])] {
        let findings = project_snapshot_findings(fixture.join("tsconfig.json"), Some(dialect));
        let mut writes = findings
            .iter()
            .filter(|finding| finding["id"] == "SC2001")
            .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
            .collect::<Vec<_>>();
        writes.sort_unstable();
        assert_eq!(writes, expected, "wrong component identity in {dialect}");
        let mut destructures = findings
            .iter()
            .filter(|finding| finding["id"] == "SC1003")
            .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
            .collect::<Vec<_>>();
        destructures.sort_unstable();
        assert_eq!(
            destructures, component_prop_patterns,
            "callback containment or a JSX render helper distorted component identity in {dialect}"
        );
        assert!(
            findings.iter().all(|finding| {
                finding["message"]
                    .as_str()
                    .is_none_or(|message| !message.contains("localSameName"))
            }),
            "a user-local type alias became a Solid accessor: {findings:#?}"
        );
    }
}

#[test]
fn package_contract_async_behavior_reaches_async_sensitive_rules() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/contract-async/tsconfig.json"),
        Some("solid-v2"),
    );
    assert!(
        findings.iter().any(|finding| finding["id"] == "SC7002"),
        "the dependency contract's asyncBehavior did not classify the computation: {findings:#?}"
    );
}

#[test]
fn package_contract_reactive_reads_reach_control_flow_preferences() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let findings = project_snapshot_findings_with(
        root.join("fixtures/reactive-ir/package-consumer/tsconfig.json"),
        Some("solid-v2"),
        &["--preset", "preferences"],
    );
    assert!(
        findings.iter().any(|finding| {
            finding["id"] == "SC8014"
                && finding["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("package-consumer/App.tsx"))
        }),
        "the package contract's reactiveReads summary did not reach prefer-for: {findings:#?}"
    );
}

#[test]
fn solid_two_write_wording_follows_source_provenance() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let findings = project_snapshot_findings(
        root.join("fixtures/reactive-ir/write-scope/tsconfig.json"),
        Some("solid-v2"),
    );
    let store = findings
        .iter()
        .find(|finding| {
            finding["id"] == "SC2001"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("setState"))
        })
        .expect("the store setter should report in the component body");
    assert!(
        store["message"]
            .as_str()
            .is_some_and(|message| message.starts_with("store setter")),
        "store provenance was described as another source kind: {store:#?}"
    );
    assert!(
        store["evidence"][0]["message"]
            .as_str()
            .is_some_and(|message| message.ends_with("Solid store")),
        "store evidence lost its provenance: {store:#?}"
    );

    let accessor = findings
        .iter()
        .find(|finding| {
            finding["id"] == "SC2001"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("setCount"))
        })
        .expect("the accessor setter should report in the component body");
    assert!(
        accessor["message"]
            .as_str()
            .is_some_and(|message| message.starts_with("accessor setter")),
        "accessor provenance was described as another source kind: {accessor:#?}"
    );
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
fn solid_one_reaction_callback_uses_its_disposing_computation_owner() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let one = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/solid-1x-resource-overloads/tsconfig.json"),
        Some("solid-v1"),
    );
    assert!(
        one.iter().all(|finding| finding["id"] != "SC3001"),
        "createReaction installs its own owner and disposes callback cleanups and children: {one:#?}"
    );
    assert!(
        one.iter().any(|finding| {
            finding["id"] == "SC2001" && finding["analysisContext"] == "namedTrackingCallback"
        }),
        "the callback passed through the returned reaction tracker must still be analyzed as a tracked computation: {one:#?}"
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
            finding["id"] != "SC4001" || finding["primaryLocation"]["startByte"] != cleanup_start
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
fn solid_one_create_mutable_allows_direct_proxy_writes() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/solid-1x-sources/tsconfig.json"),
        Some("solid-v1"),
    );
    assert!(
        findings.iter().all(|finding| {
            finding["rule"] != "v1/no-direct-mutation"
                || !finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("mutable"))
        }),
        "createMutable is a writable proxy in Solid 1.x: {findings:#?}"
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
        .filter(|finding| finding["id"] == "SC4001")
        .filter(|finding| {
            finding["message"]
                .as_str()
                .is_some_and(|message| message.starts_with("onCleanup"))
        })
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
        .filter(|finding| finding["id"] == "SC4001")
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
        .find("FromProducerInheritsComponentOwner")
        .and_then(|start| {
            source[start..]
                .find("onCleanup")
                .map(|offset| start + offset)
        })
        .expect("component from cleanup") as u64;
    let findings = project_snapshot_findings(fixture.join("tsconfig.json"), Some("solid-v1"));
    let top_level_findings = findings
        .iter()
        .filter(|finding| finding["id"] == "SC4001")
        .filter(|finding| finding["primaryLocation"]["startByte"] == invalid_cleanup)
        .collect::<Vec<_>>();
    assert_eq!(
        top_level_findings.len(),
        1,
        "the top-level producer is certainly unowned: {findings:#?}"
    );
    assert!(
        findings.iter().any(|finding| {
            finding["id"] == "SC4001"
                && finding["primaryLocation"]["startByte"] == valid_cleanup
                && finding["kind"] == "uncertifiable"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("component or an ordinary helper"))
        }),
        "an uppercase-only function may be a component or an ordinary helper, so the producer's inherited owner must remain uncertifiable: {findings:#?}"
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
            finding["id"] != "SC4001" || finding["primaryLocation"]["startByte"] != cleanup_start
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
            finding["id"] != "SC4001"
                || finding["message"]
                    .as_str()
                    .is_none_or(|message| !message.starts_with("onCleanup"))
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
        .filter(|finding| finding["id"] == "SC4001")
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
            finding["id"] != "SC4001"
                || finding["message"]
                    .as_str()
                    .is_none_or(|message| !message.starts_with("onCleanup"))
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
        .filter(|finding| finding["id"] == "SC4001")
        .collect::<Vec<_>>();
    let cleanup_finding_at = |start: u64| {
        cleanup_findings
            .iter()
            .copied()
            .find(|finding| finding["primaryLocation"]["startByte"] == start)
    };
    assert!(
        cleanup_finding_at(cleanup_at("discardedLazySource")).is_none(),
        "a dormant loader cannot register cleanup: {findings:#?}"
    );
    assert!(
        cleanup_finding_at(cleanup_at("immediateLazySource"))
            .is_some_and(|finding| finding["kind"] == "violation"),
        "an immediately invoked top-level lazy component inherits no owner: {findings:#?}"
    );
    assert!(
        cleanup_finding_at(cleanup_at("jsxLazySource"))
            .is_some_and(|finding| finding["kind"] == "uncertifiable"),
        "a JSX invocation inside an uppercase-only function inherits an owner only if that function is actually used as a component: {findings:#?}"
    );
    let cross_file_source = std::fs::read_to_string(fixture.join("lazy-component.tsx"))
        .expect("read cross-file lazy fixture");
    let cross_file_loader = cross_file_source
        .find("export const CrossFileLazy")
        .expect("cross-file lazy loader");
    let cross_file_cleanup = cross_file_source[cross_file_loader..]
        .find("onCleanup")
        .map(|offset| cross_file_loader + offset)
        .expect("cross-file cleanup") as u64;
    assert!(
        findings.iter().any(|finding| {
            finding["id"] == "SC4001"
                && finding["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("lazy-component.tsx"))
                && finding["primaryLocation"]["startByte"] == cross_file_cleanup
                && finding["kind"] == "uncertifiable"
        }),
        "a cross-file JSX invocation inside an uppercase-only function has the same unresolved inherited owner: {findings:#?}"
    );
    assert!(
        cleanup_finding_at(cleanup_at("preloadedLazySource"))
            .is_some_and(|finding| finding["kind"] == "violation"),
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

#[test]
fn run_with_owner_distinguishes_null_definite_and_nullable_owners_in_both_dialects() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/run-with-owner-null");
    let source = std::fs::read_to_string(fixture.join("App.ts")).expect("read fixture");
    let null_region = source.find("runWithOwner(null").unwrap();
    let null_effect = source[null_region..]
        .find("createEffect")
        .map(|offset| (null_region + offset) as u64)
        .unwrap();
    let definite_effect = source[usize::try_from(null_effect).unwrap() + 1..]
        .find("createEffect")
        .map(|offset| null_effect + 1 + offset as u64)
        .unwrap();
    let nullable_effect = source[usize::try_from(definite_effect).unwrap() + 1..]
        .find("createEffect")
        .map(|offset| definite_effect + 1 + offset as u64)
        .unwrap();
    let aliased_nullable_effect = source[usize::try_from(nullable_effect).unwrap() + 1..]
        .find("createEffect")
        .map(|offset| nullable_effect + 1 + offset as u64)
        .unwrap();

    for dialect in ["solid-v1", "solid-v2"] {
        let findings = project_snapshot_findings(fixture.join("tsconfig.json"), Some(dialect));
        let owners = findings
            .iter()
            .filter(|finding| finding["id"] == "SC4001")
            .collect::<Vec<_>>();
        assert!(
            owners.iter().any(|finding| {
                finding["primaryLocation"]["startByte"] == null_effect
                    && finding["kind"] == "violation"
            }),
            "{dialect} missed the definitely detached effect: {findings:#?}"
        );
        assert!(
            owners
                .iter()
                .all(|finding| { finding["primaryLocation"]["startByte"] != definite_effect }),
            "{dialect} rejected a statically non-null owner: {findings:#?}"
        );
        assert!(
            owners.iter().any(|finding| {
                finding["primaryLocation"]["startByte"] == nullable_effect
                    && finding["kind"] == "uncertifiable"
                    && finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("runWithOwner may receive null"))
            }),
            "{dialect} treated a nullable owner as definitely present: {findings:#?}"
        );
        assert!(
            owners.iter().any(|finding| {
                finding["primaryLocation"]["startByte"] == aliased_nullable_effect
                    && finding["kind"] == "uncertifiable"
            }),
            "{dialect} treated an aliased nullable owner as definitely present: {findings:#?}"
        );

        let effect_after = |owner: &str| {
            let owner = source.find(owner).unwrap();
            (owner + source[owner..].find("createEffect").unwrap()) as u64
        };
        let reexported = effect_after("runWithOwner(reExportedOwner");
        let local = effect_after("runWithOwner(localOwner");
        let unresolved = effect_after("runWithOwner(unresolvedOwner");
        assert!(
            owners
                .iter()
                .all(|finding| { finding["primaryLocation"]["startByte"] != reexported }),
            "{dialect} rejected a re-exported Solid Owner: {findings:#?}"
        );
        assert!(
            owners.iter().any(|finding| {
                finding["primaryLocation"]["startByte"] == local
                    && finding["kind"] == "uncertifiable"
            }),
            "{dialect} accepted a user-local type named Owner: {findings:#?}"
        );
        assert!(
            owners.iter().any(|finding| {
                finding["primaryLocation"]["startByte"] == unresolved
                    && finding["kind"] == "uncertifiable"
            }),
            "{dialect} treated an unresolved owner as definitely present: {findings:#?}"
        );
    }
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
    for file in ["App.tsx", "tsconfig.json"] {
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
    for (_, rule, _, _) in &one {
        assert!(
            rule.starts_with("v1/"),
            "1.x finding {rule} must carry the v1/ namespace"
        );
    }
    for (_, rule, _, _) in &two {
        assert!(
            !rule.starts_with("v1/"),
            "2.0 finding {rule} must stay unprefixed"
        );
    }

    // The headline arity difference: `createEffect(() => ...)` is the 1.x
    // signature, so only 2.0 reports the one-argument call. Calls rejected by
    // the published typings stay silent; cast-hidden runtime defects survive.
    let source =
        std::fs::read_to_string(fixtures.join("dialect-solid-1x").join("App.tsx")).unwrap();
    let one_argument_call = source
        .find("createEffect(() => {\n    reader()")
        .expect("one-argument createEffect marker") as u64;
    let escaped_compute_call = source
        .find("createEffect(123 as unknown as () => number)")
        .expect("cast-hidden createEffect compute marker") as u64;
    let server_directive_call = source
        .find("createEffect(456 as unknown as () => number)")
        .expect("server directive spelling marker") as u64;
    let escaped_apply_calls = [
        "createEffect(() => 1, 123 as unknown as (value: number) => void)",
        "createEffect(() => 1, null as unknown as (value: number) => void)",
        "createEffect(() => 1, {} as unknown as (value: number) => void)",
    ]
    .map(|marker| source.find(marker).expect("cast-hidden apply marker") as u64);
    let sc7001 = |findings: &[(String, String, String, u64)]| {
        findings
            .iter()
            .filter(|(code, _, _, _)| code == "SC7001")
            .map(|(_, _, kind, byte)| (*byte, kind.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        sc7001(&one),
        vec![
            (escaped_compute_call, "violation".to_owned()),
            (server_directive_call, "uncertifiable".to_owned()),
        ]
    );
    // No core package reads `"use server"`, so the spelling proves neither
    // client nor server execution. Both dialects preserve that missing fact as
    // an uncertifiable SC7001 obligation: their client entries fail, while
    // their server entries neutralise the call.
    assert_eq!(
        sc7001(&two),
        [
            vec![
                (one_argument_call, "violation".to_owned()),
                (escaped_compute_call, "violation".to_owned()),
            ],
            escaped_apply_calls
                .map(|byte| (byte, "violation".to_owned()))
                .to_vec(),
            vec![(server_directive_call, "uncertifiable".to_owned())],
        ]
        .concat()
    );

    // Neither dialect projects this as a forbidden leaf cleanup: 1.x runs the
    // callback under the reaction's disposing computation, while 2.0's
    // genuinely unowned callback belongs to the missing-owner family.
    assert!(!one.iter().any(|(code, _, _, _)| code == "SC3001"));
    assert!(!two.iter().any(|(code, _, _, _)| code == "SC3001"));
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
