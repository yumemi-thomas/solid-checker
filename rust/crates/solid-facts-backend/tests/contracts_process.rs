#[path = "support/process.rs"]
mod support;

use std::{env, fs, path::PathBuf, process::Command};

use support::{decode_findings, temporary_directory};

fn checker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn cli_validation_accepts_every_receipt_issued_stable_v1_bundle() {
    let directory = root().join("pkg/contracts/bundled/solid-v2");
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|value| value.to_str()) != Some("json")
            || path.to_string_lossy().contains("receipt")
            || path.file_name().and_then(|value| value.to_str()) == Some("bundle-index.json")
        {
            continue;
        }
        let output = checker()
            .args(["--validate-contract", &path.to_string_lossy()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn cli_validation_refuses_the_retired_public_schema() {
    let directory = temporary_directory("retired-contract-schema");
    let legacy = directory.join("legacy.json");
    fs::write(
        &legacy,
        br#"{"schemaVersion":1,"package":{"name":"legacy","version":"1.0.0"},"entrypoints":{}}"#,
    )
    .unwrap();
    let output = checker()
        .args(["--validate-contract", &legacy.to_string_lossy()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("contract document cannot be decoded")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn checked_bundle_generator_is_reproducible_in_both_physical_locations() {
    let output = Command::new(env!("CARGO_BIN_EXE_solid-contract-bundles"))
        .args(["--root", &root().to_string_lossy(), "--check"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn accepted_fixture_catalog_reaches_the_process_analyzer() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let fixture = root().join("fixtures/reactive-ir/package-return-consumer");
    let output = checker()
        .args([
            "--project",
            &fixture.join("tsconfig.json").to_string_lossy(),
            "--typefacts",
            &typefacts,
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let findings = decode_findings(&output.stdout);
    assert!(findings.iter().any(|finding| {
        finding["rule"] == "strict-read-untracked"
            && finding["message"]
                .as_str()
                .is_some_and(|message| message.contains("count"))
    }));
}

#[test]
fn proposal_emission_requires_exact_resolution_and_a_separate_plan() {
    let directory = temporary_directory("phase14-proposal-requires-resolution");
    let fixture = root().join("fixtures/reactive-ir/package-return-consumer");
    let output = checker()
        .args([
            "--project",
            &fixture.join("tsconfig.json").to_string_lossy(),
            "--emit-contract",
            &directory.join("proposal.json").to_string_lossy(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--contract-resolution"));
    assert!(stderr.contains("--emit-proposal-plan"));
}
