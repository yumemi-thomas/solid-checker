//! The dialect seam, observed end to end: what detection resolves from the
//! installed `solid-js`, and what the selected dialect then reports — with the
//! dialect chosen by detection, never by a flag.
//!
//! This file was built around a *pair* of dialects and the differences between
//! them. With one dialect (ADR 0110) the differential half is gone, and what
//! remains is the selection itself, the refusal of a runtime no vocabulary
//! covers, and the assertions whose scope — not whose subject — was the
//! dialect list.

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// The dialects a *dialect-independent* assertion is run against.
///
/// The tests below that loop this list are not differentials: each asserts the
/// **same** property holds under every dialect, so the list is the claim's
/// scope and not part of the property. Retiring the 1.x dialect narrowed the
/// scope and kept every claim intact — which is exactly what this constant was
/// introduced to make a one-line edit.
///
/// It stays a list of one rather than collapsing into the call sites: the next
/// dialect this checker carries re-widens it here, and a looped assertion says
/// "for every dialect" in a way a hard-coded `"solid-v2"` does not.
const DIALECT_INDEPENDENT: &[&str] = &["solid-v2"];

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
        Some("solid-v2"),
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding["rule"] == "missing-owner"),
        "the enabled control rule should still report: {findings:#?}"
    );
    assert!(
        findings
            .iter()
            .all(|finding| finding["rule"] != "reactive-write-in-owned-scope"),
        "the exact disabled rule still reported: {findings:#?}"
    );
}

#[test]
fn component_ref_callbacks_are_setup_time_outputs() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/component-ref/tsconfig.json");
    for dialect in DIALECT_INDEPENDENT {
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
    for dialect in DIALECT_INDEPENDENT {
        let findings = project_snapshot_findings(project.clone(), Some(dialect));
        // An empty result is only worth something if the project was analyzed.
        // `project_snapshot_findings` already fails on a non-zero exit, which
        // is what separates "traced the handler and found nothing" from "never
        // opened the file"; a fixture that stopped compiling would not reach
        // here quietly.
        assert!(
            findings.is_empty(),
            "{dialect} should trace the returned inner handler to the JSX event: {findings:#?}"
        );
    }
}

#[test]
fn component_identity_comes_from_type_facts() {
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
    // Was a differential over a dialect pair: 2.0 proves the write at
    // `setCount(1)` is in an owned scope and 1.x proved nothing there, so the
    // two arms contrasted dialect compatibility. With one dialect there is no
    // contrast to draw, so the loop is gone rather than iterating a list of
    // one, and what survives is the claim the name now states — component
    // identity is decided by Type Facts, which the SC2001 offset and the exact
    // SC1003 set below both measure.
    let findings = project_snapshot_findings(fixture.join("tsconfig.json"), Some("solid-v2"));
    let mut writes = findings
        .iter()
        .filter(|finding| finding["id"] == "SC2001")
        .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
        .collect::<Vec<_>>();
    writes.sort_unstable();
    assert_eq!(writes, vec![typed_offset], "wrong component identity");
    let mut destructures = findings
        .iter()
        .filter(|finding| finding["id"] == "SC1003")
        .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
        .collect::<Vec<_>>();
    destructures.sort_unstable();
    assert_eq!(
        destructures, component_prop_patterns,
        "callback containment or a JSX render helper distorted component identity"
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

#[test]
fn retired_policy1_async_contract_no_longer_supplies_behavior() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = project_snapshot_findings(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/contract-async/tsconfig.json"),
        Some("solid-v2"),
    );
    assert!(findings.iter().all(|finding| finding["id"] != "SC7002"));
}

#[test]
fn retired_policy1_reactive_reads_no_longer_supply_behavior() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let findings = project_snapshot_findings_with(
        root.join("fixtures/reactive-ir/package-consumer/tsconfig.json"),
        Some("solid-v2"),
        &["--preset", "preferences"],
    );
    assert!(findings.iter().all(|finding| finding["id"] != "SC8014"));
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
    // A store setter directly in a component body is legal on the audited
    // rc.3 triple (the dev component body is a root, which its store setter
    // guard exempts), so the store half reads the one in a memo compute.
    let store_findings = project_snapshot_findings(
        root.join("fixtures/reactive-ir/write-scope-roots-rc3/tsconfig.json"),
        Some("solid-v2"),
    );
    let store = store_findings
        .iter()
        .find(|finding| {
            finding["id"] == "SC2001"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("setState"))
        })
        .expect("the store setter should report in the memo compute");
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
fn run_with_owner_distinguishes_null_definite_and_nullable_owners() {
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

    // One property, asserted for every dialect this build carries; the name no
    // longer says "in both dialects" because the scope is the list, not the
    // claim. See `DIALECT_INDEPENDENT`.
    for dialect in DIALECT_INDEPENDENT.iter().copied() {
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

/// Runs the checker on a backend fixture and returns `(exit code, snapshot)`.
///
/// Distinct from [`project_snapshot_findings_with`], which asserts success:
/// the refusal's exit code is part of what these tests pin, so failure has to
/// be observable rather than an assertion inside the helper.
fn run_checker(fixture: &str, extra_args: &[&str]) -> (i32, serde_json::Value) {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN")
        .expect("guard the calling test on SOLID_TYPEFACTS_BIN before running the checker");
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture)
        .join("tsconfig.json");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .arg("--typefacts")
        .arg(&typefacts)
        .arg("--format")
        .arg("json")
        .args(extra_args)
        .arg("--project")
        .arg(&project)
        .output()
        .expect("run checker");
    let snapshot = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "snapshot JSON from {}: {error}\nstdout: {}\nstderr: {}",
            project.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.code().expect("checker exit code"), snapshot)
}

fn finding_ids(snapshot: &serde_json::Value) -> Vec<String> {
    snapshot["findings"]
        .as_array()
        .expect("findings array")
        .iter()
        .map(|finding| finding["id"].as_str().unwrap().to_owned())
        .collect()
}

/// An installed runtime this build has no dialect for replaces the analysis.
///
/// The fixture is built so this cannot pass vacuously: `App.tsx` destructures
/// component props, so **any build that analyzes this tree reports SC1003**.
/// Seeing SC9013 alone therefore proves the refusal *replaces* the analysis
/// rather than merely preceding it — SC1003 is gone, not accompanied.
///
/// This assertion used to be reachable only under
/// `--no-default-features --features dialect-v2`, because a build carrying the
/// 1.x dialect had nothing to refuse. Retiring that dialect makes it the
/// ordinary case, and this the ordinary test.
#[test]
fn unsupported_runtime_refusal_replaces_the_analysis() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let (code, snapshot) = run_checker("unsupported-runtime-v1", &[]);
    let ids = finding_ids(&snapshot);

    {
        assert_eq!(
            ids,
            vec!["SC9013".to_owned()],
            "the refusal is the whole result: SC1003 is not reported beside it, \
             because the source was never analyzed under the language it runs"
        );
        assert_eq!(snapshot["status"], "uncertifiable");
        let finding = &snapshot["findings"][0];
        assert_eq!(finding["rule"], "unsupported-solid-runtime");
        assert_eq!(finding["kind"], "uncertifiable");
        assert_eq!(finding["severity"], "error");
        let message = finding["message"].as_str().unwrap();
        assert!(
            message.contains("1.9.14"),
            "the refusal quotes the version it read: {message}"
        );
        let path = finding["primaryLocation"]["path"].as_str().unwrap();
        assert!(
            path.ends_with("unsupported-runtime-v1/node_modules/solid-js/package.json"),
            "the deciding manifest is the location, because it is the file to change: {path}"
        );
        assert_eq!(
            code, 0,
            "without --certify the refusal reports and exits 0, exactly as \
             every other uncertifiable result does"
        );
        assert_eq!(
            run_checker("unsupported-runtime-v1", &["--certify"]).0,
            1,
            "an uncertifiable result fails certification"
        );
    }
}

/// A major no `Version` variant names is refused exactly like 1.x is.
///
/// This is the half of ADR 0110 § 1 that 1.x could not pin. `Version::V1`
/// exists, so a 1.x install has a classification to be refused on; an
/// installed `solid-js@3.0.0` has none, and `for_solid_js` used to answer
/// `None` for it — which `detect_detailed` read as "unclassifiable", which is
/// [`Detection::Defaulted`], which is the 2.0 catalog. The hole ADR 0110
/// closed for 1.x was open for every major after 2.
///
/// Same construction as the test above: `App.tsx` destructures component
/// props, so any build that analyzes this tree reports SC1003. SC9013 alone
/// proves the refusal replaced the analysis; SC1003 anywhere in the result
/// would mean a Solid 3 project had just been checked against Solid 2's rules.
#[test]
fn a_major_this_build_does_not_name_is_refused_like_one_it_does() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let (code, snapshot) = run_checker("unsupported-runtime-future-major", &[]);
    let ids = finding_ids(&snapshot);

    assert_eq!(
        ids,
        vec!["SC9013".to_owned()],
        "an uncarried future major refuses; analyzing it as 2.0 is the silent \
         wrong-language outcome, and SC1003 here would be exactly that"
    );
    assert_eq!(snapshot["status"], "uncertifiable");
    let finding = &snapshot["findings"][0];
    assert_eq!(finding["rule"], "unsupported-solid-runtime");
    assert_eq!(finding["kind"], "uncertifiable");
    let message = finding["message"].as_str().unwrap();
    assert!(
        message.contains("3.0.0"),
        "the refusal quotes the version it read, not a classified major: {message}"
    );
    let path = finding["primaryLocation"]["path"].as_str().unwrap();
    assert!(
        path.ends_with("unsupported-runtime-future-major/node_modules/solid-js/package.json"),
        "the deciding manifest is the location, because it is the file to change: {path}"
    );
    assert_eq!(code, 0);
    assert_eq!(
        run_checker("unsupported-runtime-future-major", &["--certify"]).0,
        1,
        "an uncertifiable result fails certification"
    );
}

/// `--check-contracts` under a refused runtime answers with the refusal, and
/// specifically *not* with a contract report.
///
/// The two documents are unrelated shapes: a report is
/// `{missing, packages, stale}`, the refusal is the ordinary findings
/// snapshot. That difference is load bearing rather than incidental, because
/// the sweep in `packages/cli/scripts/generate-missing-contracts.mjs` reads
/// `report.packages` -- and an empty `packages` there means "no package needs
/// a contract", which is the opposite of what an unanalyzed project knows. The
/// report must therefore be absent, not empty, and this pins that.
#[test]
fn a_refused_runtime_answers_check_contracts_with_the_refusal_not_a_report() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let (code, document) = run_checker("unsupported-runtime-v1", &["--check-contracts"]);

    assert!(
        document.get("packages").is_none(),
        "an absent report is the honest answer; an empty one reads as complete coverage: {document}"
    );
    assert!(document.get("missing").is_none() && document.get("stale").is_none());
    assert_eq!(finding_ids(&document), vec!["SC9013".to_owned()]);
    assert_eq!(document["status"], "uncertifiable");
    assert_eq!(
        code, 0,
        "the contract report exits 1 when a package needs action, and this is not that; \
         the sweep's own status guard accepts 0 and 1, so the shape is what has to carry it"
    );
}

/// `--dialect` is the documented escape hatch, and it overrides the refusal
/// too: the tree analyzes under the named dialect in every build.
#[test]
fn unsupported_runtime_refusal_yields_to_an_explicit_dialect() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let (code, snapshot) = run_checker("unsupported-runtime-v1", &["--dialect", "solid-v2"]);
    let ids = finding_ids(&snapshot);
    assert!(
        !ids.contains(&"SC9013".to_owned()),
        "an explicit dialect is a decision, not a detection: {ids:?}"
    );
    assert!(
        ids.contains(&"SC1003".to_owned()),
        "and the analysis actually ran: {ids:?}"
    );
    assert_eq!(code, 0);
}

/// A carried major whose vocabulary refuses the release line is refused like
/// an uncarried major, and the refusal says why.
///
/// `2.0.0-experimental.1` classifies as Solid 2 by its major alone. The
/// construction is the one above: any build that analyzed the tree reports
/// SC1003, so SC9013 alone proves the refusal replaced the analysis.
#[test]
fn the_pre_beta_experiment_is_refused_although_its_major_is_carried() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let (code, snapshot) = run_checker("unsupported-runtime-experimental", &[]);
    assert_eq!(
        finding_ids(&snapshot),
        vec!["SC9013".to_owned()],
        "the pre-beta experiment is refused; SC1003 here would be the rc vocabulary \
         misreading its argument positions"
    );
    assert_eq!(snapshot["status"], "uncertifiable");
    let finding = &snapshot["findings"][0];
    assert_eq!(finding["rule"], "unsupported-solid-runtime");
    let message = finding["message"].as_str().unwrap();
    assert!(
        message.contains("2.0.0-experimental.1")
            && message.contains("2.0.0-experimental.x")
            && message.contains("@solidjs/signals 0.x"),
        "the refusal quotes the version and the vocabulary's reason: {message}"
    );
    assert!(
        !message.contains("carries no dialect"),
        "the dialect is carried; saying otherwise would send the user looking for one: {message}"
    );
    let path = finding["primaryLocation"]["path"].as_str().unwrap();
    assert!(
        path.ends_with("unsupported-runtime-experimental/node_modules/solid-js/package.json"),
        "{path}"
    );
    assert_eq!(code, 0);
    assert_eq!(
        run_checker("unsupported-runtime-experimental", &["--certify"]).0,
        1
    );
}

/// A reviewed release older than the audited one is analyzed, and the
/// analysis carries one project-level uncertifiable notice naming the release,
/// its open gaps, and the review. The audited release beside it carries none.
#[test]
fn a_reviewed_release_is_analyzed_with_one_notice_beside_the_findings() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    // rc.9 is the audited release (ADR 0127): the same project, analyzed, no
    // notice. `App.tsx` imports `createSignal` alone, so the scoped re-export
    // gap is not reached (`rc9_re_export_gap_is_due_only_where_a_project_reaches_it`
    // pins that gap).
    let (code, audited) = run_checker("unaudited-release-rc9", &[]);
    let ids = finding_ids(&audited);
    assert!(ids.contains(&"SC1003".to_owned()), "{ids:?}");
    assert!(!ids.contains(&"SC9014".to_owned()), "{ids:?}");
    assert_eq!(code, 0);

    let (code, snapshot) = run_checker("unaudited-release-rc8", &[]);
    let ids = finding_ids(&snapshot);
    assert!(
        ids.contains(&"SC1003".to_owned()),
        "the analysis ran: the notice accompanies findings, it does not replace them: {ids:?}"
    );
    let notices = snapshot["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["id"] == "SC9014")
        .collect::<Vec<_>>();
    assert_eq!(notices.len(), 1, "exactly one notice per project: {ids:?}");
    let notice = notices[0];
    assert_eq!(notice["rule"], "unaudited-solid-release");
    assert_eq!(notice["kind"], "uncertifiable");
    assert_eq!(notice["severity"], "warning");
    assert_eq!(notice["subjectKind"], "project");
    let message = notice["message"].as_str().unwrap();
    assert!(message.contains("2.0.0-rc.8"), "{message}");
    let hint = notice["hint"].as_str().unwrap();
    assert!(
        hint.contains(
            "docs/package-contract-v2/audits/2026-09-26-solid-2-rc1-rc8-release-review.md"
        ) && hint.contains("to 2.0.0-rc.9, the audited release of each"),
        "the notice points at the review and the audited release: {hint}"
    );
    let gaps = notice["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|step| step["message"].as_str())
        .filter(|message| message.starts_with("known gap "))
        .collect::<Vec<_>>();
    // rc.8's signals has no negative row, and every owner is older than the
    // audited release.
    assert!(
        gaps.len() == 2
            && gaps.iter().any(|gap| gap.contains("negative row"))
            && gaps
                .iter()
                .any(|gap| gap.contains("older than the audited release")),
        "every open gap is named: {gaps:?}"
    );
    let path = notice["primaryLocation"]["path"].as_str().unwrap();
    assert!(
        path.ends_with("unaudited-release-rc8/node_modules/solid-js/package.json"),
        "{path}"
    );
    assert_eq!(code, 0);

    // `--dialect solid-v2` is the decision to analyze under the audited
    // vocabulary; it is not a detection, so it carries no notice.
    let (_, explicit) = run_checker("unaudited-release-rc8", &["--dialect", "solid-v2"]);
    let ids = finding_ids(&explicit);
    assert!(!ids.contains(&"SC9014".to_owned()), "{ids:?}");
    assert!(ids.contains(&"SC1003".to_owned()), "{ids:?}");
}

/// `solid-js@2.0.0-rc.9`'s typings re-export five names their declarations do
/// not declare. That gap is scoped to those exports: the `SC9014` notice
/// carries it only for a project that reaches one of them, names what reached
/// it, and keeps it where a use of `solid-js` does not name what it reaches.
///
/// Asserted on the notice's gap list, not on its presence: the rc.9 triple has
/// release-wide gaps of its own (signals' negative rows), so a notice on the
/// negative fixture is not a failure, and one on a positive is not a pass.
#[test]
fn rc9_re_export_gap_is_due_only_where_a_project_reaches_it() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    // (fixture, the clause the gap must carry, or `None` for no gap).
    let cases: [(&str, Option<&str>); 6] = [
        ("rc9-reexport-gap-none", None),
        (
            "rc9-reexport-gap-named",
            Some("this project uses createErrorBoundary from solid-js"),
        ),
        (
            "rc9-reexport-gap-aliased",
            Some("this project uses sharedConfig from solid-js"),
        ),
        (
            "rc9-reexport-gap-namespace-member",
            Some("this project uses createLoadingBoundary from solid-js"),
        ),
        (
            "rc9-reexport-gap-namespace-escape",
            Some(
                "this project uses solid-js in a way that does not name the exports it reaches \
                 (a namespace import of solid-js used other than as a member name)",
            ),
        ),
        (
            "rc9-reexport-gap-project-reexport",
            Some("this project uses createRevealOrder and $DEVCOMP from solid-js"),
        ),
    ];
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/reactive-ir");
    for (fixture, clause) in cases {
        // Detection chooses the dialect: an explicit `--dialect` gets no notice.
        let findings =
            project_snapshot_findings(fixtures.join(fixture).join("tsconfig.json"), None);
        let notices = findings
            .iter()
            .filter(|finding| finding["id"] == "SC9014")
            .collect::<Vec<_>>();
        assert!(notices.len() <= 1, "{fixture}: one notice per project");
        let evidence = notices
            .first()
            .map(|notice| {
                notice["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|step| Some((step["message"].as_str()?, step.get("location"))))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let re_export_gaps = evidence
            .iter()
            .filter(|(message, _)| {
                message.starts_with("known gap solid-js@2.0.0-rc.9 re-exports createErrorBoundary")
            })
            .collect::<Vec<_>>();
        match clause {
            None => assert!(
                re_export_gaps.is_empty(),
                "{fixture} reaches none of the five names: {evidence:?}"
            ),
            Some(clause) => {
                let [(gap, _)] = re_export_gaps.as_slice() else {
                    panic!("{fixture}: the re-export gap is due exactly once: {evidence:?}");
                };
                assert!(gap.contains(clause), "{fixture}: {gap}");
                // The site that made it due is evidence, located in the
                // project's own source.
                let located = evidence
                    .iter()
                    .filter(|(message, _)| {
                        message.contains(" is reached from solid-js here")
                            || message.contains("so which of its exports the project reaches")
                    })
                    .filter_map(|(_, location)| {
                        location.and_then(|location| location["path"].as_str())
                    })
                    .collect::<Vec<_>>();
                assert!(
                    !located.is_empty()
                        && located
                            .iter()
                            .all(|path| path.contains(fixture) && path.ends_with(".ts")),
                    "{fixture}: the reaching site is located: {evidence:?}"
                );
            }
        }
    }
}
