use std::{env, path::PathBuf, process::Command};

fn decode_findings(output: &[u8]) -> Vec<serde_json::Value> {
    let snapshot: serde_json::Value = serde_json::from_slice(output).expect("decode snapshot");
    snapshot["findings"]
        .as_array()
        .expect("snapshot findings")
        .clone()
}

pub fn diagnostic_fixture(name: &str) -> Option<Vec<serde_json::Value>> {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").ok()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let project = root.join(format!("fixtures/reactive-ir/{name}/tsconfig.json"));
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .args(["--format", "json", "--project"])
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
