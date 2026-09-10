#[path = "support/process.rs"]
mod support;

#[path = "contract_closure_process.rs"]
mod contract_closure_process;

use std::{env, fs, path::PathBuf, process::Command};

use support::{decode_findings, temporary_directory};

fn checker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn core_runtime_model_needs_no_contract_and_is_not_reported_as_certified() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    for fixture_name in ["dialect-solid-1x", "dialect-solid-2"] {
        let fixture = root().join("fixtures/reactive-ir").join(fixture_name);
        let output = checker()
            .args([
                "--project",
                &fixture.join("tsconfig.json").to_string_lossy(),
                "--typefacts",
                &typefacts,
                "--check-contracts",
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let core = report["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == "solid-js")
            .expect("core import is reported");
        assert_eq!(core["status"], "builtin");
        assert!(core["remedy"].is_null());
        assert_eq!(core["contractPath"], "");
        assert_eq!(report["missing"], 0);
        assert!(
            core["detail"]
                .as_str()
                .unwrap()
                .contains("not installed-artifact authentication")
        );
    }
}

#[test]
fn an_installed_core_alias_does_not_require_a_package_contract() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let directory = temporary_directory("core-foundation-installed-alias");
    let package = directory.join("node_modules/core-alias");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        r#"{"name":"@solidjs/signals","version":"2.0.0-rc.3","types":"index.d.ts"}"#,
    )
    .unwrap();
    // No behavioral signatures: this fixture tests resolution/reporting only.
    fs::write(package.join("index.d.ts"), "export {};\n").unwrap();
    fs::write(
        directory.join("main.ts"),
        "import * as core from 'core-alias'; export { core };\n",
    )
    .unwrap();
    fs::write(directory.join("tsconfig.json"), r#"{"compilerOptions":{"module":"ESNext","moduleResolution":"Bundler"},"include":["main.ts"]}"#).unwrap();
    let run = || {
        checker()
            .args([
                "--project",
                &directory.join("tsconfig.json").to_string_lossy(),
                "--typefacts",
                &typefacts,
                "--dialect",
                "solid-v2",
                "--check-contracts",
                "--format",
                "json",
            ])
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["packages"][0]["name"], "core-alias");
    assert_eq!(report["packages"][0]["status"], "builtin");
    // A similarly named external package retains its contract requirement.
    fs::write(package.join("package.json"), r#"{"name":"@solidjs/signals-extra","version":"2.0.0-rc.3","types":"index.d.ts","peerDependencies":{"solid-js":"*"}}"#).unwrap();
    let output = run();
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["packages"][0]["status"], "missing");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn an_incompatible_core_package_requires_a_dialect_change_not_a_receipt() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let project = root().join("fixtures/reactive-ir/rendering-csr-selected/tsconfig.json");
    let output = checker()
        .args([
            "--project",
            &project.to_string_lossy(),
            "--typefacts",
            &typefacts,
            "--dialect",
            "solid-v1",
            "--check-contracts",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let web = report["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "@solidjs/web")
        .expect("web is reported");
    assert_eq!(web["status"], "unsupported-runtime");
    assert!(
        web["remedy"]
            .as_str()
            .unwrap()
            .contains("a core package contract cannot extend")
    );
}

#[test]
fn missing_core_contract_objects_cannot_change_ordinary_findings() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let directory = temporary_directory("core-foundation-no-contracts");
    let template: serde_json::Value = serde_json::from_slice(
        &fs::read(root().join(
            "fixtures/reactive-ir/package-return-consumer/.solid-checker/accepted-contracts.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let mut catalog = template.clone();
    catalog["contracts"] = serde_json::Value::Array(
        ["solid-js", "@solidjs/signals", "@solidjs/web"]
            .into_iter()
            .flat_map(|package| {
                [package, "core-alias"]
                    .into_iter()
                    .map(|specifier| {
                        let mut entry = template["contracts"][0].clone();
                        entry["document"] = "absent-core-document.json".into();
                        entry["import"]["packageName"] = package.into();
                        entry["import"]["specifier"] = specifier.into();
                        entry
                    })
                    .collect::<Vec<_>>()
            })
            .collect(),
    );
    let catalog_path = directory.join("catalog.json");
    fs::write(&catalog_path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    assert!(
        solid_facts_backend::accepted_contract_catalog_members(&catalog_path)
            .unwrap()
            .is_empty()
    );
    for fixture_name in ["dialect-solid-1x", "dialect-solid-2"] {
        let project = root()
            .join("fixtures/reactive-ir")
            .join(fixture_name)
            .join("tsconfig.json");
        let run = |catalog: bool| {
            let mut command = checker();
            command.args([
                "--project",
                &project.to_string_lossy(),
                "--typefacts",
                &typefacts,
                "--format",
                "json",
            ]);
            if catalog {
                command.args(["--accepted-contracts", &catalog_path.to_string_lossy()]);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success() || output.status.code() == Some(1),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            decode_findings(&output.stdout)
        };
        let baseline = run(false);
        assert!(
            !baseline.is_empty(),
            "the native model must exercise real findings"
        );
        assert_eq!(baseline, run(true));
    }
    catalog["contracts"][0]["import"]["packageName"] = "solid-js-extra".into();
    catalog["contracts"][0]["import"]["specifier"] = "solid-js-extra".into();
    fs::write(&catalog_path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    assert!(
        solid_facts_backend::read_external_contract_catalog_with_trust(&catalog_path, None)
            .is_err()
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_validation_accepts_every_retired_bundle_main_as_a_proposal() {
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
fn retired_fixture_catalog_no_longer_supplies_package_semantics() {
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
        finding["rule"] == "package-contract-incomplete"
            && finding["analysisContext"]
                .as_str()
                .is_some_and(|context| context.starts_with("obsolete-policy1-receipt:"))
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

/// A real gzipped npm tarball with matching registry metadata, written where a
/// certification request can name them.
fn published_artifact(
    directory: &std::path::Path,
    members: &[(&str, &[u8])],
) -> (PathBuf, PathBuf) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use sha2::{Digest as _, Sha512};

    let mut archive = Vec::new();
    {
        let encoder = flate2::write::GzEncoder::new(&mut archive, flate2::Compression::none());
        let mut builder = tar::Builder::new(encoder);
        for (path, bytes) in members {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_cksum();
            builder.append_data(&mut header, path, *bytes).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap();
    }
    let integrity = format!("sha512-{}", STANDARD.encode(Sha512::digest(&archive)));
    let metadata = format!(
        r#"{{"versions":{{"1.0.0":{{"name":"root-package","version":"1.0.0","dist":{{"integrity":"{integrity}","tarball":"https://registry.npmjs.org/root-package/-/root-package-1.0.0.tgz"}}}}}}}}"#
    );
    let archive_path = directory.join("root-package-1.0.0.tgz");
    let metadata_path = directory.join("root-package.json");
    fs::write(&archive_path, &archive).unwrap();
    fs::write(&metadata_path, metadata.as_bytes()).unwrap();
    (archive_path, metadata_path)
}

fn applicability_planning_request(
    directory: &std::path::Path,
    archive: &std::path::Path,
    metadata: &std::path::Path,
    claims: serde_json::Value,
) -> PathBuf {
    let request = serde_json::json!({
        "schemaVersion": 1,
        // Deliberately absent: a declared applicability claim is re-proved
        // before the proposal document is even read, so a refuted claim must
        // refuse here and a proved one must fall through to this path.
        "proposal": directory.join("absent-proposal.json").to_string_lossy(),
        "resolution": {
            "specifier": "root-package",
            "importer": "/project/src/app.ts",
            "requestedEntrypoint": ".",
            "packageName": "root-package",
            "packageVersion": "1.0.0",
            "packageIntegrity": "sha512-AA==",
            "packageRoot": "/project/node_modules/root-package",
            "packageManifest": { "path": "/p/package.json", "digest": "sha256:00" },
            "runtime": { "path": "/p/dist/index.js", "digest": "sha256:00" },
            "declarations": { "path": "/p/types/index.d.ts", "digest": "sha256:00" },
            "closure": { "digest": "sha256:00", "entries": [], "dependencies": [], "hazards": [] },
            "authority": "host"
        },
        "exportConditions": ["import"],
        "registryOrigin": "https://registry.npmjs.org",
        "registryMetadata": metadata.to_string_lossy(),
        "archive": archive.to_string_lossy(),
        "inapplicableCases": claims
    });
    let path = directory.join("certification-request.json");
    fs::write(&path, format!("{request:#}\n")).unwrap();
    path
}

const APPLICABILITY_MEMBERS: &[(&str, &[u8])] = &[
    (
        "package/package.json",
        br#"{"name":"root-package","version":"1.0.0","exports":{".":"./dist/index.js","./types/*":"./types/*"}}"#,
    ),
    ("package/dist/index.js", b"export const answer = 42;"),
    (
        "package/types/effects.d.ts",
        b"import { start } from \"./dep.js\";\nstart();\n",
    ),
    (
        "package/types/kinds.d.ts",
        b"export type Kind = 1;\n",
    ),
];

/// The claim a proposal carries for an omitted artifact case is re-proved
/// against the authenticated archive by the *planning* request path. Deleting
/// that call site makes this test fail: a refuted claim would reach the
/// (deliberately absent) proposal instead of refusing.
#[test]
fn a_planning_request_refuses_a_declared_applicability_the_archive_refutes() {
    let directory = temporary_directory("phase21-applicability-planning");
    let (archive, metadata) = published_artifact(&directory, APPLICABILITY_MEMBERS);
    let request = applicability_planning_request(
        &directory,
        &archive,
        &metadata,
        serde_json::json!([{
            "entrypoint": "./types/effects.d.ts",
            "conditions": [],
            "class": "non-emitting-module-target",
            "reason": "runtime target emits no JavaScript (declaration-file): 2 module-level statement(s)"
        }]),
    );
    let output = checker()
        .args([
            "--plan-contract-certification",
            &request.to_string_lossy(),
            "--certification-plan-output",
            &directory.join("plan.json").to_string_lossy(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("artifact case ./types/effects.d.ts"),
        "{stderr}"
    );
    assert!(stderr.contains("non-emitting-module-target"), "{stderr}");
    assert!(stderr.contains("emits JavaScript"), "{stderr}");
    assert!(stderr.contains("value import at bytes 0..33"), "{stderr}");
    assert!(
        !stderr.contains("could not read certification proposal"),
        "the claim must be refused before the proposal is read: {stderr}"
    );
}

/// The control that makes the test above a proof rather than a coincidence: a
/// claim the archive *proves* gets past the same call site and fails on the
/// absent proposal instead.
#[test]
fn a_planning_request_proves_a_declared_applicability_and_continues() {
    let directory = temporary_directory("phase21-applicability-planning-proved");
    let (archive, metadata) = published_artifact(&directory, APPLICABILITY_MEMBERS);
    for claims in [
        serde_json::json!([]),
        serde_json::json!([{
            "entrypoint": "./types/kinds.d.ts",
            "conditions": [],
            "class": "non-emitting-module-target",
            "reason": "runtime target emits no JavaScript (declaration-file): 1 module-level statement(s)"
        }]),
    ] {
        let request = applicability_planning_request(&directory, &archive, &metadata, claims);
        let output = checker()
            .args([
                "--plan-contract-certification",
                &request.to_string_lossy(),
                "--certification-plan-output",
                &directory.join("plan.json").to_string_lossy(),
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("could not read certification proposal"),
            "{stderr}"
        );
        assert!(
            !stderr.contains("declared artifact-case applicability"),
            "{stderr}"
        );
    }
}
