//! The ordinary check consumes no package contract. The requirement-only
//! index still raises every package boundary the project's own manifests
//! establish. `--no-package-contracts` is accepted as a no-op that refuses
//! every option that would read or write a contract tier, and a request for
//! the daemon with it is answered one-shot.

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

/// Every option that reads or writes a contract tier is refused before any
/// analysis, with nothing on standard output.
#[test]
fn options_that_need_a_contract_tier_are_refused() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let project = fixture("package-contract-absent");
    let missing = project.join("absent.json");
    let missing = missing.to_str().unwrap();
    for (arguments, flag) in [
        (
            vec!["--accepted-contracts", missing],
            "--accepted-contracts",
        ),
        (
            vec!["--receipt-trust-configuration", missing],
            "--receipt-trust-configuration",
        ),
        (
            vec!["--proposal-dependencies", missing],
            "--proposal-dependencies",
        ),
    ] {
        let mut extra = vec!["--no-package-contracts"];
        extra.extend(arguments);
        let output = run(&typefacts, &project, &extra, false);
        assert_eq!(output.status.code(), Some(2), "{flag}");
        assert!(output.stdout.is_empty(), "{flag}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim_end(),
            format!("solid-checker-rust: --no-package-contracts cannot be combined with {flag}"),
            "{flag}"
        );
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &to.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

/// A daemon request is answered one-shot, exactly as with the daemon off; the
/// same project without the flag does reach the daemon, so the routing is
/// observed rather than assumed.
#[test]
fn a_daemon_request_is_answered_one_shot() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::env::temp_dir()
        .join("sc-scratch.noindex")
        .join(format!(
            "solid-checker-no-package-contracts-{}",
            std::process::id()
        ));
    let _ = fs::remove_dir_all(&root);
    copy_tree(&fixture("package-contract-absent"), &root.join("project"));
    let project = fs::canonicalize(root.join("project")).unwrap();
    let retained = |output: &Output| {
        String::from_utf8_lossy(&output.stderr).contains("\"mode\":\"retained-daemon\"")
    };
    let routed = run(&typefacts, &project, &["--no-package-contracts"], true);
    let fresh = run(&typefacts, &project, &["--no-package-contracts"], false);
    let control = run(&typefacts, &project, &[], true);
    let _ = fs::remove_dir_all(&root);
    assert!(
        !retained(&routed),
        "{}",
        String::from_utf8_lossy(&routed.stderr)
    );
    assert_eq!(snapshot(&routed), snapshot(&fresh));
    assert!(
        retained(&control),
        "{}",
        String::from_utf8_lossy(&control.stderr)
    );
}
