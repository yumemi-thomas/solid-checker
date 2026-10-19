use std::{
    fs::{create_dir_all, remove_dir_all, write},
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

use solid_facts_backend::{Metrics, Snapshot, SnapshotFinding, SourceLocation};

use super::emit;

fn snapshot(status: &str) -> Snapshot {
    Snapshot {
        premises: Vec::new(),
        feedback_facts: Vec::new(),
        status: status.into(),
        findings: Vec::new(),
        metrics: Metrics {
            files_analyzed: 1,
            functions_analyzed: 2,
            proof_obligations: 3,
            cached_summaries: 4,
            unresolved_obligations: 0,
        },
    }
}

#[test]
fn every_report_format_states_the_runs_premises() {
    let mut snapshot = snapshot("certified");
    snapshot.premises = vec![
        "standard runtime configuration (ADRs 0266/0268)".into(),
        "package integrity (ADR 0300)".into(),
    ];
    for format in ["json", "text", "default", "full"] {
        let output = emit(
            solid_facts_backend::dialect::default_dialect(),
            format,
            "project",
            &snapshot,
            false,
            false,
            Duration::ZERO,
        )
        .unwrap()
        .output;
        let output = String::from_utf8(output).unwrap();
        for premise in &snapshot.premises {
            assert!(output.contains(premise), "{format}: {output}");
        }
        if format == "json" {
            let decoded: Snapshot = serde_json::from_str(&output).unwrap();
            assert_eq!(decoded.premises, snapshot.premises);
        }
    }
}

#[test]
fn text_and_json_share_certification_semantics() {
    for format in ["text", "json"] {
        assert_eq!(
            emit(
                solid_facts_backend::dialect::default_dialect(),
                format,
                "project",
                &snapshot("violation"),
                true,
                false,
                Duration::ZERO,
            )
            .unwrap()
            .exit_code,
            1
        );
        assert_eq!(
            emit(
                solid_facts_backend::dialect::default_dialect(),
                format,
                "project",
                &snapshot("certified"),
                true,
                false,
                Duration::ZERO,
            )
            .unwrap()
            .exit_code,
            0
        );
        assert_eq!(
            emit(
                solid_facts_backend::dialect::default_dialect(),
                format,
                "project",
                &snapshot("violation"),
                false,
                false,
                Duration::ZERO,
            )
            .unwrap()
            .exit_code,
            0
        );
    }
}

#[test]
fn wire_snapshot_with_omitted_optional_fields_round_trips() {
    // The daemon client re-parses emitted snapshots; findings serialize
    // without their empty optional fields, so deserialization must
    // default them instead of failing (which silently downgraded every
    // daemon check with findings to a cold one-shot run).
    let wire = r#"{
            "status": "violation",
            "findings": [{
                "id": "SC1003",
                "rule": "strict-read-untracked",
                "kind": "violation",
                "severity": "error",
                "message": "reactive read outside tracked scope",
                "primaryLocation": {
                    "path": "App.tsx", "startByte": 1, "endByte": 2,
                    "line": 1, "column": 1
                }
            }],
            "metrics": {
                "filesAnalyzed": 1, "functionsAnalyzed": 1,
                "proofObligations": 1, "cachedSummaries": 0,
                "unresolvedObligations": 0
            }
        }"#;
    let decoded: Snapshot = serde_json::from_str(wire).unwrap();
    assert_eq!(decoded.findings[0].subject_kind, "");
    assert!(decoded.findings[0].fixes.is_empty());
    let reemitted = emit(
        solid_facts_backend::dialect::default_dialect(),
        "json",
        "project",
        &decoded,
        true,
        false,
        Duration::ZERO,
    )
    .unwrap();
    assert_eq!(reemitted.exit_code, 1);
}

#[test]
fn formats_snapshot_without_adapter_specific_logic() {
    let text = emit(
        solid_facts_backend::dialect::default_dialect(),
        "text",
        "project",
        &snapshot("certified"),
        false,
        false,
        Duration::ZERO,
    )
    .unwrap();
    // Users see `clean`; the internal view keeps the certification status.
    assert_eq!(text.output, b"project: clean\n");
    let internal = emit(
        solid_facts_backend::dialect::default_dialect(),
        "text",
        "project",
        &snapshot("certified"),
        false,
        true,
        Duration::ZERO,
    )
    .unwrap();
    assert_eq!(internal.output, b"project: certified\n");

    let json = emit(
        solid_facts_backend::dialect::default_dialect(),
        "json",
        "project",
        &snapshot("certified"),
        false,
        false,
        Duration::ZERO,
    )
    .unwrap();
    let decoded: serde_json::Value = serde_json::from_slice(&json.output).unwrap();
    assert_eq!(decoded["status"], "certified");
    assert!(json.output.ends_with(b"\n"));
}

#[test]
fn default_format_renders_oxc_source_frames() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-default-format-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    create_dir_all(&directory).unwrap();
    let source_path = directory.join("App.tsx");
    write(
        &source_path,
        "export function Bad({ title }) {\n  return <h2>{title}</h2>;\n}\n",
    )
    .unwrap();
    let mut snapshot = snapshot("violation");
    snapshot.findings.push(SnapshotFinding {
        id: "SC1003".into(),
        rule: "no-destructure".into(),
        kind: "violation".into(),
        severity: "error".into(),
        message: "destructuring component props reads them outside tracking".into(),
        hint: "Keep the props object intact and read props.<name> inside JSX.".into(),
        analysis_context: String::new(),
        subject_kind: "component-props".into(),
        coverage_family: String::new(),
        coverage_subject: String::new(),
        primary_location: SourceLocation {
            path: source_path.to_string_lossy().into_owned(),
            start_byte: 20,
            end_byte: 29,
            line: 1,
            column: 21,
        },
        related_locations: Vec::new(),
        evidence: Vec::new(),
        fixes: Vec::new(),
    });

    let rendered = emit(
        solid_facts_backend::dialect::default_dialect(),
        "default",
        &directory.join("tsconfig.json").to_string_lossy(),
        &snapshot,
        true,
        false,
        Duration::from_millis(295),
    )
    .unwrap();
    let text = String::from_utf8(rendered.output).unwrap();
    assert!(text.contains("solid-checker(certification)"));
    assert!(text.contains("[SC1003]"));
    assert!(text.contains("App.tsx"));
    assert!(text.contains("destructuring component props"));
    assert!(text.contains("Keep the props object intact"));
    assert!(text.contains("docs/rules/no-destructure.md"));
    assert!(text.contains(&format!(
        "Finished in 295ms on 1 files with {} rules using",
        solid_facts_backend::dialect::default_dialect().rule_count
    )));
    assert_eq!(rendered.exit_code, 1);

    // ADR 0328: an uncertifiable finding is internal. Users see neither it
    // nor a failing `--certify` for it; `--internal` shows it and fails.
    let mut undecided = snapshot.clone();
    undecided.status = "uncertifiable".into();
    undecided.findings[0].kind = "uncertifiable".into();
    undecided.findings[0].message = "could not prove this either way".into();
    let user = emit(
        solid_facts_backend::dialect::default_dialect(),
        "default",
        &directory.join("tsconfig.json").to_string_lossy(),
        &undecided,
        true,
        false,
        Duration::ZERO,
    )
    .unwrap();
    assert!(
        !String::from_utf8(user.output)
            .unwrap()
            .contains("could not prove")
    );
    assert_eq!(user.exit_code, 0);
    let internal = emit(
        solid_facts_backend::dialect::default_dialect(),
        "default",
        &directory.join("tsconfig.json").to_string_lossy(),
        &undecided,
        true,
        true,
        Duration::ZERO,
    )
    .unwrap();
    assert!(
        String::from_utf8(internal.output)
            .unwrap()
            .contains("could not prove")
    );
    assert_eq!(internal.exit_code, 1);

    // A project-level notice stays visible and fails `--certify`.
    undecided.findings[0].subject_kind = "project".into();
    let notice = emit(
        solid_facts_backend::dialect::default_dialect(),
        "default",
        &directory.join("tsconfig.json").to_string_lossy(),
        &undecided,
        true,
        false,
        Duration::ZERO,
    )
    .unwrap();
    assert!(
        String::from_utf8(notice.output)
            .unwrap()
            .contains("Notices (1)")
    );
    assert_eq!(notice.exit_code, 1);
    remove_dir_all(directory).unwrap();
}
