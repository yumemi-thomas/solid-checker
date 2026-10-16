//! The ordinary check consumes no package contract. The requirement-only
//! index still raises every package boundary the project's own manifests
//! establish, and every flag package contracts took is refused.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir")
        .join(name)
}

fn runtime_arguments(project: &Path) -> Vec<String> {
    let Ok(bytes) = fs::read(project.join(".solid-checker/runtime.json")) else {
        return Vec::new();
    };
    let runtime: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut arguments = Vec::new();
    for (key, flag) in [("target", "--runtime-target"), ("build", "--runtime-build")] {
        if let Some(value) = runtime[key].as_str() {
            arguments.extend([flag.to_owned(), value.to_owned()]);
        }
    }
    for specifier in runtime["dependencySources"]
        .as_array()
        .into_iter()
        .flatten()
    {
        arguments.extend([
            "--dependency-source".to_owned(),
            specifier.as_str().unwrap().to_owned(),
        ]);
    }
    arguments
}

fn run(typefacts: &str, project: &Path, extra: &[&str], daemon: bool) -> Output {
    Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", if daemon { "1" } else { "0" })
        .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "20")
        .env("SOLID_CHECKER_TIMINGS", "1")
        .args(["--format", "json", "--project"])
        .arg(project.join("tsconfig.json"))
        .args(runtime_arguments(project))
        .args(extra)
        .output()
        .unwrap()
}

fn snapshot(output: &Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn findings(snapshot: &serde_json::Value) -> Vec<(String, String, String)> {
    snapshot["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            (
                finding["id"].as_str().unwrap().to_owned(),
                finding["kind"].as_str().unwrap().to_owned(),
                finding["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// A package whose own manifest names Solid stays an open obligation with no
/// contract to answer it: the boundary is the requirement index's, not a
/// tier's.
#[test]
fn the_requirement_boundary_survives_without_any_contract() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let project = fixture("package-contract-absent");
    let disabled = snapshot(&run(
        &typefacts,
        &project,
        &["--no-package-contracts"],
        false,
    ));
    assert_eq!(disabled["status"], "uncertifiable");
    let rows = findings(&disabled);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        (rows[0].0.as_str(), rows[0].1.as_str()),
        ("SC9005", "uncertifiable")
    );
    assert!(
        rows[0].2.contains(
            "uncontracted-package is not analyzed from source here, so the behaviour of imported export readCount is unknown"
        ),
        "{rows:?}"
    );
}

/// Every flag package contracts took is refused with a message, never read
/// as an unknown argument and never silently ignored.
#[test]
fn removed_contract_flags_are_refused() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let project = fixture("package-contract-absent");
    for arguments in [
        vec!["--accepted-contracts", "absent.json"],
        vec!["--receipt-trust-configuration=absent.json"],
        vec!["--check-contracts"],
        vec!["--emit-contract", "out.json"],
        vec!["--no-dependency-source"],
    ] {
        let output = run(&typefacts, &project, &arguments, false);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}: {stderr}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(
            stderr.contains("was removed with package contracts"),
            "{arguments:?}: {stderr}"
        );
    }
}
