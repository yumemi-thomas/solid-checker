//! The dialect seam, observed end to end: byte-identical sources, two
//! resolved `solid-js` versions, two different sets of findings — with the
//! dialect chosen by detection, never by a flag.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn dialect_pair_findings(fixture: &str) -> Vec<(String, String, u64)> {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return Vec::new();
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .arg("--typefacts")
        .arg(&typefacts)
        .arg("--format")
        .arg("json")
        .arg("--project")
        .arg(root.join(format!("fixtures/reactive-ir/{fixture}/tsconfig.json")))
        .output()
        .expect("run checker");
    assert!(
        output.status.success(),
        "checker failed on {fixture}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("snapshot JSON");
    snapshot["findings"]
        .as_array()
        .expect("findings array")
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

/// The pair is duplicated source on purpose: the two fixture projects differ
/// only in the `solid-js` version their `node_modules` resolves, so every
/// difference between their findings is the dialect's doing.
#[test]
fn the_dialect_pair_reports_different_findings_from_identical_sources() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
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
