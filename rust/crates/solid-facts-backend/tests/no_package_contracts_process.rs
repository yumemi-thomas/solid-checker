//! `--no-package-contracts`: the ordinary check consuming no package contract
//! from any tier, the native reference for the owned WASM check (P5 S1).
//!
//! Disabled consumption is not an empty boundary: the requirement-only index
//! still raises every package boundary the project's own manifests establish,
//! and keeps open every import a tier the check does not consume covers (a
//! project catalog entry, the authored tier) without reading that tier's
//! claims. Dependency-source programs are still analyzed, and the flag
//! refuses every option that would read or write a contract tier. A request
//! for the daemon is answered one-shot.

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

/// Every fixture whose imports a project catalog names. No check reads a
/// catalog's contract (P7); each keeps the same imports open for the reason it
/// has: no contract is consumed.
const CATALOGED: &[&str] = &[
    "package-contract-install-shapes",
    "package-contract-paths-shadow",
    "package-return-consumer",
    "package-unknown-callback-consumer",
];

/// Every distinct finding site, as `(id, kind, start, end)`.
type Sites = std::collections::BTreeSet<(String, String, u64, u64)>;

/// The status, the two obligation counts, and every distinct finding site.
fn outcome(snapshot: &serde_json::Value) -> (String, u64, u64, Sites) {
    let sites = snapshot["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            (
                finding["id"].as_str().unwrap().to_owned(),
                finding["kind"].as_str().unwrap().to_owned(),
                finding["primaryLocation"]["startByte"].as_u64().unwrap(),
                finding["primaryLocation"]["endByte"].as_u64().unwrap(),
            )
        })
        .collect();
    (
        snapshot["status"].as_str().unwrap().to_owned(),
        snapshot["metrics"]["proofObligations"].as_u64().unwrap(),
        snapshot["metrics"]["unresolvedObligations"]
            .as_u64()
            .unwrap(),
        sites,
    )
}

/// The catalog is read for which imports it names, never for its contract:
/// every cataloged import stays exactly as open as the default leaves it.
#[test]
fn a_cataloged_import_stays_open_without_its_contract() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for name in CATALOGED {
        let project = fixture(name);
        let default = snapshot(&run(&typefacts, &project, &[], false));
        let disabled = snapshot(&run(
            &typefacts,
            &project,
            &["--no-package-contracts"],
            false,
        ));
        assert!(
            !findings(&default)
                .iter()
                .any(|(_, _, message)| message.contains("discovered reactivity contract")),
            "{name}: {default}"
        );
        assert!(
            !findings(&disabled)
                .iter()
                .any(|(_, _, message)| message.contains("discovered reactivity contract")),
            "{name}: {disabled}"
        );
        assert_eq!(outcome(&disabled), outcome(&default), "{name}");
        assert_eq!(disabled["status"], "uncertifiable", "{name}");
    }
}

/// The review's counterexample: a local reactive callback passed to an
/// installed package that names no Solid dependency and whose implementation
/// nothing analyzes. Its callback behavior is unknown, so the check stays
/// uncertifiable with the obligation open, and proves no violation.
#[test]
fn an_unknown_callback_into_a_plain_install_is_not_certified() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let project = fixture("package-unknown-callback-consumer");
    let disabled = snapshot(&run(
        &typefacts,
        &project,
        &["--no-package-contracts"],
        false,
    ));
    assert_eq!(disabled["status"], "uncertifiable", "{disabled}");
    assert_eq!(disabled["metrics"]["proofObligations"], 2, "{disabled}");
    assert_eq!(
        disabled["metrics"]["unresolvedObligations"], 2,
        "{disabled}"
    );
    let rows = disabled["findings"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{disabled}");
    let row = &rows[0];
    assert_eq!(
        (row["id"].as_str(), row["kind"].as_str()),
        (Some("SC9005"), Some("uncertifiable"))
    );
    // The import of runUnknown, App.ts:2.
    assert_eq!(row["primaryLocation"]["startByte"], 41);
    assert_eq!(row["primaryLocation"]["endByte"], 98);
    assert!(
        row["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["message"].as_str().unwrap().starts_with(
                "package contracts are not consumed in this check (--no-package-contracts)"
            )),
        "{row}"
    );
}

/// A specifier `paths` maps onto project source is analyzed as that source:
/// only the import that resolves into the cataloged install stays open.
#[test]
fn a_paths_shadowed_specifier_stays_project_source() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let project = fixture("package-contract-paths-shadow");
    let disabled = snapshot(&run(
        &typefacts,
        &project,
        &["--no-package-contracts"],
        false,
    ));
    let rows = disabled["findings"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{disabled}");
    assert!(
        rows[0]["message"]
            .as_str()
            .unwrap()
            .contains("other-reactive-package"),
        "{disabled}"
    );
    assert_eq!(rows[0]["primaryLocation"]["line"], 2, "{disabled}");
}

/// Dependency-source programs do not depend on a contract tier: the selected
/// package's own JavaScript answers the same violation either way.
#[test]
fn dependency_sources_answer_the_same_without_contracts() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let project = fixture("dependency-source-program");
    let default = snapshot(&run(&typefacts, &project, &[], false));
    let disabled = snapshot(&run(
        &typefacts,
        &project,
        &["--no-package-contracts"],
        false,
    ));
    assert_eq!(disabled, default);
    assert!(
        findings(&disabled)
            .iter()
            .any(|(id, kind, _)| id == "SC1001" && kind == "violation"),
        "{disabled}"
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
        (vec!["--check-contracts"], "--check-contracts"),
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

/// A private copy of a fixture, removed when dropped.
struct Copy {
    root: PathBuf,
    project: PathBuf,
}

impl Copy {
    fn of(name: &str, tag: &str) -> Self {
        let root = std::env::temp_dir()
            .join("sc-scratch.noindex")
            .join(format!(
                "solid-checker-no-package-contracts-{tag}-{}",
                std::process::id()
            ));
        let _ = fs::remove_dir_all(&root);
        copy_tree(&fixture(name), &root.join("project"));
        let project = fs::canonicalize(root.join("project")).unwrap();
        Self { root, project }
    }

    /// Rewrites every catalog entry's importer to `importer`.
    fn respell_importer(&self, importer: &str) {
        let path = self.project.join(".solid-checker/accepted-contracts.json");
        let mut catalog: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for entry in catalog["contracts"].as_array_mut().unwrap() {
            entry["import"]["importer"] = importer.into();
        }
        fs::write(&path, serde_json::to_vec_pretty(&catalog).unwrap()).unwrap();
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The exact P1 outcome: uncertifiable, both obligations open, one SC9005 at
/// the `partial-package` import with the Disabled note, and no violation.
fn assert_p1_open(disabled: &serde_json::Value, label: &str) {
    assert_eq!(disabled["status"], "uncertifiable", "{label}: {disabled}");
    assert_eq!(disabled["metrics"]["proofObligations"], 2, "{label}");
    assert_eq!(disabled["metrics"]["unresolvedObligations"], 2, "{label}");
    let rows = disabled["findings"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{label}: {disabled}");
    assert_eq!(rows[0]["id"], "SC9005", "{label}");
    assert_eq!(rows[0]["kind"], "uncertifiable", "{label}");
    assert_eq!(rows[0]["primaryLocation"]["startByte"], 41, "{label}");
    assert_eq!(rows[0]["primaryLocation"]["endByte"], 98, "{label}");
    assert!(
        rows[0]["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["message"].as_str().unwrap().starts_with(
                "package contracts are not consumed in this check (--no-package-contracts)"
            )),
        "{label}: {}",
        rows[0]
    );
}

/// Claim C on the review's counterexample: every supported spelling of the
/// catalog importer keeps its canonical key and both open obligations --
/// relative, absolute, absolute through `node_modules/..`, and through a
/// symlinked directory. None is refused and none is certified.
#[cfg(unix)]
#[test]
fn every_supported_importer_spelling_keeps_the_callback_obligation() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for (tag, spelling) in [
        ("relative", None),
        ("absolute", Some("{project}/App.ts")),
        ("dotdot", Some("{project}/node_modules/../App.ts")),
        ("symlink", Some("alias/App.ts")),
    ] {
        let copy = Copy::of("package-unknown-callback-consumer", tag);
        std::os::unix::fs::symlink(".", copy.project.join("alias")).unwrap();
        if let Some(spelling) = spelling {
            copy.respell_importer(&spelling.replace("{project}", copy.project.to_str().unwrap()));
        }
        let output = run(
            &typefacts,
            &copy.project,
            &["--no-package-contracts"],
            false,
        );
        assert_p1_open(&snapshot(&output), tag);
    }
}

/// Provenance that cannot be established ends the check: exit 2, one stderr
/// line naming the refusal, and no snapshot.
#[cfg(unix)]
#[test]
fn a_dangling_catalog_root_refuses_the_check() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let copy = Copy::of("package-unknown-callback-consumer", "dangling-root");
    fs::remove_dir_all(copy.project.join(".solid-checker")).unwrap();
    std::os::unix::fs::symlink("missing-target", copy.project.join(".solid-checker")).unwrap();
    let output = run(
        &typefacts,
        &copy.project,
        &["--no-package-contracts"],
        false,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("package-contract provenance cannot be established")
            && stderr.contains("dangling symlink"),
        "{stderr}"
    );
}

/// A covered unresolved import (the ambiently typed `ambient-package`) whose
/// installation is a dangling link refuses the check rather than losing its
/// requirement; with the package genuinely absent, the check answers and that
/// import carries no requirement -- the missing module is TypeScript's.
#[cfg(unix)]
#[test]
fn a_covered_unresolved_import_needs_its_installation_answered() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let sites = |snapshot: &serde_json::Value| {
        snapshot["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|finding| finding["primaryLocation"]["startByte"].as_u64().unwrap())
            .collect::<Vec<_>>()
    };
    let intact = Copy::of("package-contract-install-shapes", "install-intact");
    let open = snapshot(&run(
        &typefacts,
        &intact.project,
        &["--no-package-contracts"],
        false,
    ));
    assert!(
        sites(&open).contains(&0),
        "the ambient import is open: {open}"
    );

    let dangling = Copy::of("package-contract-install-shapes", "install-dangling");
    fs::remove_dir_all(dangling.project.join("node_modules/ambient-package")).unwrap();
    std::os::unix::fs::symlink(
        "missing-target",
        dangling.project.join("node_modules/ambient-package"),
    )
    .unwrap();
    let output = run(
        &typefacts,
        &dangling.project,
        &["--no-package-contracts"],
        false,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("dangling symlink"), "{stderr}");

    let absent = Copy::of("package-contract-install-shapes", "install-absent");
    fs::remove_dir_all(absent.project.join("node_modules/ambient-package")).unwrap();
    let answered = snapshot(&run(
        &typefacts,
        &absent.project,
        &["--no-package-contracts"],
        false,
    ));
    assert!(
        !sites(&answered).contains(&0),
        "no requirement at the missing package: {answered}"
    );
    assert!(
        sites(&answered).contains(&122),
        "the subpath import stays open: {answered}"
    );
}
