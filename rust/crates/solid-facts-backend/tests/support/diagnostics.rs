use std::{env, path::PathBuf, process::Command};

fn decode_findings(output: &[u8]) -> Vec<serde_json::Value> {
    let snapshot: serde_json::Value = serde_json::from_slice(output).expect("decode snapshot");
    snapshot["findings"]
        .as_array()
        .expect("snapshot findings")
        .clone()
}

pub fn diagnostic_fixture(name: &str) -> Option<Vec<serde_json::Value>> {
    diagnostic_fixture_with(name, &[])
}

/// [`diagnostic_fixture`] with the options a fixture's `.solid-checker/runtime.json` selects (`--dependency-source`, the runtime target and build), which the CLI does not read.
#[allow(dead_code)] // the test binaries that include this module and take no option leave it unused
pub fn diagnostic_fixture_with(name: &str, options: &[&str]) -> Option<Vec<serde_json::Value>> {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").ok()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let project = root.join(format!("fixtures/reactive-ir/{name}/tsconfig.json"));
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .args(["--format", "json"])
        .args(options)
        .arg("--project")
        .arg(project)
        .output()
        .expect("run Rust diagnostic CLI");
    assert!(
        output.status.success(),
        "fixture {name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(decode_findings(&output.stdout))
}

/// ADR 0970: the owner emission takes a function a reference enters for a layer like any other, so an owner operation or a render effect in an exported component no
/// tag renders is an uncertifiable SC4001 row (`owner-callers-unenumerated`). A test whose subject is another rule leaves those rows out of its count.
#[allow(dead_code)] // the other test binaries that include this module leave those rows in
pub fn owner_entry_row(finding: &serde_json::Value) -> bool {
    finding["id"] == "SC4001"
        && finding["kind"] == "uncertifiable"
        && finding["blockedBy"]["kind"] == "owner-callers-unenumerated"
}

pub fn findings_for_rule<'a>(
    findings: &'a [serde_json::Value],
    rule: &str,
) -> Vec<&'a serde_json::Value> {
    findings
        .iter()
        .filter(|finding| finding["rule"] == rule)
        .collect()
}

pub fn assert_rule_findings<'a>(
    findings: &'a [serde_json::Value],
    rule: &str,
    expected: usize,
) -> Vec<&'a serde_json::Value> {
    let selected = findings_for_rule(findings, rule);
    assert_eq!(selected.len(), expected, "rule {rule}: {selected:#?}");
    for finding in &selected {
        assert!(
            finding["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("SC")),
            "rule {rule} must have a stable diagnostic code"
        );
        assert!(
            finding["message"]
                .as_str()
                .is_some_and(|message| !message.is_empty()),
            "rule {rule} must explain the violation"
        );
        assert!(
            finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with(".ts") || path.ends_with(".tsx")),
            "rule {rule} must point to source"
        );
        assert!(
            finding["evidence"]
                .as_array()
                .is_some_and(|evidence| !evidence.is_empty()),
            "rule {rule} must carry evidence"
        );
    }
    selected
}

/// ADR 0971 (batch 11, B2): the uncertifiable rows of the owner-call universe at a JSX creation site (a component element, an expression container's insert): an
/// SC3001 `leaf-operation-unaccounted` and an SC6002 `effect-creation-unaccounted` per site. Split from `rows` so a test that pins the rest keeps its meaning and
/// pins these by count; only a fixture with no such row of its own before B2 (a creation call's, ADR 0969) may use it.
#[allow(dead_code)] // the test binaries that include this module and keep those rows leave it unused
pub fn partition_creation_rows(
    rows: Vec<&serde_json::Value>,
) -> (Vec<&serde_json::Value>, Vec<&serde_json::Value>) {
    rows.into_iter().partition(|row| {
        row["kind"] == "uncertifiable"
            && matches!(
                (row["id"].as_str(), row["analysisContext"].as_str()),
                (Some("SC3001"), Some("leaf-operation-unaccounted"))
                    | (Some("SC6002"), Some("effect-creation-unaccounted"))
            )
    })
}
