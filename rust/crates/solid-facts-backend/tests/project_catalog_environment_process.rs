//! ADR 0123's environment rule, applied to the project-catalog tier, through
//! the real checker binary.
//!
//! A project catalog's acceptance reaches the project's own files by artifact:
//! the importer the receipt binds is the certification importer `contract
//! certify` writes beside the package, never a file the project wrote. So
//! every consumer import is admitted -- or not -- by
//! `admitted_project_artifacts`, and that now requires the consumer's installed
//! tree to reproduce the dependency environment the receipt signs.
//!
//! The tree is built the way `contract certify` publishes one -- a receipt
//! issued by a configured issuer, a catalog written by
//! `publish_policy2_catalog`, the environment's entries beside the receipt --
//! with the certification stages replaced by stand-in roots, as
//! `fixture_authorization` does. What is under test is admission, and every
//! input admission reads is the real one. The trust configuration is written
//! outside the project and handed to the checker out of band.

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use solid_facts_backend::{
    ClosureManifest, ClosurePackageIdentity, ConfiguredReceiptIssuer, DependencyEnvironmentEntry,
    EnvironmentImporter, Policy2ReceiptBindings, Policy2ReceiptProvenance,
    RECEIPT_WITNESS_FAMILIES, ResolutionAuthority, ResolutionTrace, ResolvedExportBinding,
    ResolvedExportTarget, ResolvedFile, ResolvedImport, authenticate_policy2_receipt,
    canonicalize_policy2_main, certified_catalog_self_admission,
    encode_policy2_trust_configuration, issue_policy2_receipt, policy2_artifact_acceptance_root,
    policy2_dependency_environment_root, policy2_main_closed_claims_root,
    policy2_main_semantic_digest, policy2_resolved_import_root,
    policy2_trust_configuration_for_issuer, publish_policy2_catalog,
};

use crate::support::temporary_directory;

const PACKAGE: &str = "@solid-primitives/debounce";
const INTEGRITY: &str = "sha512-Cen4ccCPTuEtQM7o9aEKuOJ0LRlAnzKvN7loEBBOQ+zKdu7/7kYKr7HHE/WS8JAI3QeQr5v2ModYRIZLERw5zw==";
/// The one package, besides the certified one, the certification read.
const DEPENDENCY: &str = "environment-dependency";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn stand_in(index: u16) -> String {
    format!("sha256:{index:064x}")
}

fn dependency(version: &str) -> DependencyEnvironmentEntry {
    DependencyEnvironmentEntry {
        name: DEPENDENCY.into(),
        version: version.into(),
        integrity: format!("sha512-{DEPENDENCY}-{version}"),
        resolved_from: None,
    }
}

/// A consumer project: two files that import the package, the package, one
/// dependency at `dependency_version`, and an npm lockfile stating both
/// integrities.
fn consumer_tree(project: &Path, dependency_version: &str) {
    let package = project.join("node_modules").join(PACKAGE);
    fs::create_dir_all(package.join("dist")).unwrap();
    fs::write(
        package.join("package.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "name": PACKAGE,
            "version": "1.3.0",
            "type": "module",
            "types": "./dist/index.d.ts",
            "exports": { ".": { "import": {
                "types": "./dist/index.d.ts",
                "default": "./dist/index.js"
            } } },
            "peerDependencies": { "solid-js": "^1.6.12" }
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        package.join("dist/index.d.ts"),
        "export declare function createDebounce<T>(callback: (value: T) => void, wait?: number): (value: T) => void;\nexport default createDebounce;\n",
    )
    .unwrap();
    fs::write(package.join("dist/index.js"), "export {};\n").unwrap();
    let dependency_root = project.join("node_modules").join(DEPENDENCY);
    fs::create_dir_all(&dependency_root).unwrap();
    fs::write(
        dependency_root.join("package.json"),
        serde_json::to_vec(&serde_json::json!({
            "name": DEPENDENCY,
            "version": dependency_version,
        }))
        .unwrap(),
    )
    .unwrap();
    // The receipt binds the certification importer `contract certify` writes
    // beside the package root (see `resolved_import`), and certify removes that
    // file before it returns: nothing of it is left in the installed tree, so
    // none is written here. The catalog names it as a path identity only.
    fs::create_dir_all(project.join("src")).unwrap();
    for (file, name) in [("App.ts", "save"), ("Other.ts", "search")] {
        fs::write(
            project.join("src").join(file),
            format!(
                "import {{ createDebounce }} from \"{PACKAGE}\";\n\nexport const {name} = createDebounce((value: string) => value.length, 100);\n"
            ),
        )
        .unwrap();
    }
    fs::write(
        project.join("tsconfig.json"),
        r#"{"compilerOptions":{"target":"ES2022","module":"ESNext","moduleResolution":"Bundler","strict":true,"noEmit":true,"skipLibCheck":true},"include":["src"]}"#,
    )
    .unwrap();
    write_lockfile(project, dependency_version);
}

fn write_lockfile(project: &Path, dependency_version: &str) {
    fs::write(
        project.join("package-lock.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "name": "consumer",
            "lockfileVersion": 3,
            "packages": {
                "": { "name": "consumer" },
                format!("node_modules/{PACKAGE}"): { "version": "1.3.0", "integrity": INTEGRITY },
                format!("node_modules/{DEPENDENCY}"): {
                    "version": dependency_version,
                    "integrity": dependency(dependency_version).integrity
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
}

/// The certifier's answer for the package in `project`: the record
/// `policy2_receipt`'s own tests certify the bundled debounce document against,
/// at this tree's absolute paths.
fn resolved_import(project: &Path) -> ResolvedImport {
    // The real path, as `contract certify` resolves it: in a pnpm tree the
    // package's store directory, and in a flat tree the same directory.
    let package_root = project
        .join("node_modules")
        .join(PACKAGE)
        .canonicalize()
        .unwrap();
    let path = |relative: &str| package_root.join(relative).to_string_lossy().into_owned();
    let runtime = ResolvedFile {
        path: path("dist/index.js"),
        real_path: None,
        digest: "sha256:b772f925ca55d55a6ef84c3277ab85f9bff2018c30c4269817327e267c76efe1".into(),
    };
    let declarations = ResolvedFile {
        path: path("dist/index.d.ts"),
        real_path: None,
        digest: "sha256:4fe1834060a02e3a3df804927e4fd7b73eab9496cd6a5e2624dd14f1d2ec382c".into(),
    };
    let target = |export_name: &str| ResolvedExportBinding {
        runtime: ResolvedExportTarget {
            module: runtime.clone(),
            export_name: export_name.into(),
        },
        declarations: ResolvedExportTarget {
            module: declarations.clone(),
            export_name: export_name.into(),
        },
    };
    let census = |name: &str, version: &str, integrity: &str, files: &str| ClosurePackageIdentity {
        name: name.into(),
        version: version.into(),
        integrity: integrity.into(),
        files_manifest_digest: files.into(),
    };
    ResolvedImport {
        specifier: PACKAGE.into(),
        importer: package_root
            .parent()
            .unwrap()
            .join(".solid-checker-certification.mjs")
            .to_string_lossy()
            .into_owned(),
        requested_entrypoint: ".".into(),
        package_name: PACKAGE.into(),
        package_version: "1.3.0".into(),
        package_integrity: INTEGRITY.into(),
        package_root: package_root.to_string_lossy().into_owned(),
        package_real_root: None,
        package_manifest: ResolvedFile {
            path: path("package.json"),
            real_path: None,
            digest: "sha256:19e4b7c252d2650e1d291af601e9fa26cc35cc2f370b14d1c5861cdd008012ab".into(),
        },
        exports: BTreeMap::from([
            ("createDebounce".into(), target("createDebounce")),
            ("default".into(), target("default")),
        ]),
        runtime,
        declarations,
        runtime_trace: ResolutionTrace {
            branch: "/exports/./import".into(),
            steps: Vec::new(),
        },
        declaration_trace: ResolutionTrace {
            branch: "/exports/./import".into(),
            steps: Vec::new(),
        },
        closure: ClosureManifest::from_package_census(vec![
            census(PACKAGE, "1.3.0", INTEGRITY, "sha256:325aec3e2c3e44e50b09cae7d6210c4f36f62d04ac1acdebd7213b3c8964d97d"),
            census("csstype", "3.2.3", "sha512-z1HGKcYy2xA8AGQfwrn0PAy+PB7X/GSj3UVJW9qKyn43xWa+gl5nXmU4qqLMRzWVLFC8KusUX8T/0kCiOYpAIQ==", "sha256:67f35df64a494f8bcebe228c056478858ab201ab6fc0036067d92c006a689108"),
            census("seroval-plugins", "1.5.6", "sha512-HXuLAX2pu/UByPpaeo/TaMfvMIi+1QqIoPJYCcAtU8QkVNwgR6MPlGuCQTErV1JwraaMbYaWVIBX7mppzGLATQ==", "sha256:eb33a8834d4353e4de034bf03799d1c3991922f937161c2418358c23ce041918"),
            census("seroval", "1.5.6", "sha512-rVQVWjjSvlINzaQPZH5JFqsqEsIWdTxY3iJZCnTL/5gQbXIRooVZKI60tVCkOVfzcRPejboxO2t0P89dg5mQaA==", "sha256:9a392773534333f3337edb24d2ad301ef9b3c4c50d1ccdc7beefc3b4de068f80"),
            census("solid-js", "1.9.14", "sha512-sAEXC0Kk0S1EDg+8ysEWJDbYhA3RRoEjwuySUGlKIemeo0I5YZfOyumNjNs9Sv3y2nmhD+0rW66ag2HsMuQiGQ==", "sha256:53190caadda3870b6b66b1334c5913d8208eb0a4d429a006952a28ab49926c76"),
        ])
        .unwrap(),
        transform: None,
        declaration_exports: std::collections::BTreeSet::new(),
        authority: ResolutionAuthority::Host,
    }
}

/// Certifies the package in `project`, stating `environment` (or none), and
/// returns the trust configuration's path, written beside -- not inside --
/// the project.
fn certify(project: &Path, environment: Option<&[DependencyEnvironmentEntry]>) -> PathBuf {
    certify_into(project, &project.join(".solid-checker"), environment)
}

/// [`certify`] for the package installed in `project`, publishing the catalog
/// into `catalog_root` -- the `.solid-checker/` of any directory, which is
/// what a package of a monorepo certified from its own directory holds. The
/// trust configuration is written beside the catalog's directory.
fn certify_into(
    project: &Path,
    catalog_root: &Path,
    environment: Option<&[DependencyEnvironmentEntry]>,
) -> PathBuf {
    let document = fs::read(
        repository_root().join("pkg/contracts/bundled/solid-v1/debounce-root-default.json"),
    )
    .unwrap();
    let main = canonicalize_policy2_main(&document).unwrap();
    let resolved = resolved_import(project);
    let conditions = vec!["import".to_owned()];
    let bindings = Policy2ReceiptBindings {
        importer: resolved.importer.clone(),
        specifier: resolved.specifier.clone(),
        resolved_import_root: policy2_resolved_import_root(&resolved).unwrap(),
        artifact_acceptance_root: policy2_artifact_acceptance_root(&resolved, &conditions).unwrap(),
        semantic_digest: policy2_main_semantic_digest(&main).unwrap(),
        artifact_provenance_root: stand_in(1),
        snapshot_root: stand_in(2),
        package_root: stand_in(3),
        manifest_root: stand_in(4),
        artifacts_root: stand_in(5),
        declarations_root: stand_in(6),
        transform_root: stand_in(7),
        exports_root: stand_in(8),
        closure_root: stand_in(9),
        demand_graph_root: stand_in(10),
        verified_positive_root: stand_in(11),
        witness_roots: RECEIPT_WITNESS_FAMILIES
            .iter()
            .enumerate()
            .map(|(index, family)| {
                (
                    (*family).to_owned(),
                    stand_in(u16::try_from(100 + index).unwrap()),
                )
            })
            .collect(),
        producer_sessions_root: stand_in(12),
        dependency_receipts_root: stand_in(13),
        dependency_trust_root: stand_in(14),
        probe_gate_root: stand_in(15),
        closed_claims_root: policy2_main_closed_claims_root(&main).unwrap(),
        verifier_source_digest: stand_in(17),
        verifier_build_digest: stand_in(18),
        dependency_environment_root: environment
            .map(policy2_dependency_environment_root)
            .unwrap_or_default(),
    };
    let issuer = ConfiguredReceiptIssuer::persistent_local("project-catalog-test", [9; 32])
        .expect("a persistent-local issuer");
    let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
    let trust = policy2_trust_configuration_for_issuer(&issuer, &bindings.verifier_build_digest, 0)
        .unwrap();
    let authenticated = authenticate_policy2_receipt(
        &main,
        &receipt,
        &bindings,
        Policy2ReceiptProvenance::PersistentLocal {
            trust_store: trust.trust_store(),
            scope: issuer.scope(),
        },
    )
    .unwrap();
    let published = publish_policy2_catalog(
        catalog_root,
        &main,
        &receipt,
        &authenticated,
        &resolved,
        &conditions,
        &trust,
    )
    .unwrap();
    // Certification states the entries beside the receipt, which signs only
    // their root; the reader recomputes the root from them. The publication
    // API attaches them only for an environment this process computed, so a
    // test states them the way the catalog carries them.
    if let Some(environment) = environment {
        let mut catalog: serde_json::Value =
            serde_json::from_slice(&fs::read(&published.catalog_path).unwrap()).unwrap();
        catalog["contracts"][0]["dependencyEnvironment"] =
            serde_json::to_value(environment).unwrap();
        fs::write(
            &published.catalog_path,
            serde_json::to_vec(&catalog).unwrap(),
        )
        .unwrap();
    }
    let trust_path = catalog_root
        .parent()
        .unwrap_or(project)
        .with_extension("trust.json");
    fs::write(
        &trust_path,
        encode_policy2_trust_configuration(&trust).unwrap(),
    )
    .unwrap();
    trust_path
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// `contract check` on `project`: the package's status, or the refusal.
fn contract_status(project: &Path, trust: &Path, typefacts: &str) -> Result<String, String> {
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .args([
            "--project",
            &project.join("tsconfig.json").to_string_lossy(),
            "--typefacts",
            typefacts,
            "--check-contracts",
            "--format",
            "json",
            "--receipt-trust-configuration",
            &trust.to_string_lossy(),
            // Only the project's own catalog is under test.
            "--no-bundled-contracts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let Ok(report) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return Err(stderr);
    };
    let row = report["packages"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["name"] == PACKAGE))
        .ok_or_else(|| format!("{PACKAGE} is not reported: {report}"))?;
    Ok(row["status"].as_str().unwrap_or_default().to_owned())
}

/// Ordinary analysis of `project`, as JSON, with the trust configuration when
/// the project has a catalog.
fn analysis(project: &Path, trust: Option<&Path>, typefacts: &str) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command.args([
        "--project",
        &project.join("tsconfig.json").to_string_lossy(),
        "--typefacts",
        typefacts,
        "--format",
        "json",
        "--no-bundled-contracts",
    ]);
    if let Some(trust) = trust {
        command.args(["--receipt-trust-configuration", &trust.to_string_lossy()]);
    }
    let output = command.output().unwrap();
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "analysis produced no snapshot ({error}): {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// The package's `packageSummaries` rows.
fn summaries(snapshot: &serde_json::Value) -> Vec<&serde_json::Value> {
    snapshot["packageSummaries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["name"] == PACKAGE)
        .collect()
}

/// The acceptance-gate `SC9005` for the package: the import matched no
/// accepted contract.
fn acceptance_gate(snapshot: &serde_json::Value) -> Option<&serde_json::Value> {
    snapshot["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| {
            finding["id"] == "SC9005"
                && finding["analysisContext"]
                    == "no receipt-accepted contract matches this exact import"
                && finding["message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains(PACKAGE)
        })
}

/// The evidence messages of one finding.
fn evidence(finding: &serde_json::Value) -> Vec<&str> {
    finding["evidence"]
        .as_array()
        .map(|steps| {
            steps
                .iter()
                .filter_map(|step| step["message"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

const CATALOG_NOTE: &str = "a project catalog entry exists for this package and was not admitted: ";

#[test]
fn package_summaries_and_the_acceptance_gate_report_admission_not_presence() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch = temporary_directory("project-catalog-summaries");
    let certified = [dependency("1.0.0")];

    // No catalog: nothing to summarize, and the acceptance gate says only
    // what it always said.
    let bare = scratch.join("bare");
    consumer_tree(&bare, "1.0.0");
    let snapshot = analysis(&bare, None, &typefacts);
    assert!(summaries(&snapshot).is_empty(), "{snapshot}");
    let bare_gate = acceptance_gate(&snapshot)
        .unwrap_or_else(|| panic!("no acceptance gate without a catalog: {snapshot}"));
    assert!(
        evidence(bare_gate)
            .iter()
            .all(|step| !step.starts_with(CATALOG_NOTE)),
        "{bare_gate}"
    );
    let bare_message = bare_gate["message"].as_str().unwrap().to_owned();

    // Admitted: the one row is `accepted`, and no import reaches the gate.
    let own = scratch.join("own");
    consumer_tree(&own, "1.0.0");
    let trust = certify(&own, Some(&certified));
    let snapshot = analysis(&own, Some(&trust), &typefacts);
    let rows = summaries(&snapshot);
    assert_eq!(rows.len(), 1, "{snapshot}");
    assert_eq!(rows[0]["evidence"], "accepted");
    assert!(rows[0].get("detail").is_none(), "{}", rows[0]);
    assert!(acceptance_gate(&snapshot).is_none(), "{snapshot}");

    // Carried into a tree whose installed dependency differs: present in the
    // catalog, refused by the environment, and both outputs say so.
    let elsewhere = scratch.join("elsewhere");
    copy_tree(&own, &elsewhere);
    fs::write(
        elsewhere
            .join("node_modules")
            .join(DEPENDENCY)
            .join("package.json"),
        serde_json::to_vec(&serde_json::json!({ "name": DEPENDENCY, "version": "2.0.0" })).unwrap(),
    )
    .unwrap();
    write_lockfile(&elsewhere, "2.0.0");
    let snapshot = analysis(&elsewhere, Some(&trust), &typefacts);
    let rows = summaries(&snapshot);
    assert_eq!(rows.len(), 1, "{snapshot}");
    assert_eq!(rows[0]["evidence"], "refused", "{}", rows[0]);
    let detail = rows[0]["detail"].as_str().unwrap();
    assert!(
        detail.starts_with(CATALOG_NOTE) && detail.contains("dependency environment differs"),
        "{detail}"
    );
    let gate = acceptance_gate(&snapshot)
        .unwrap_or_else(|| panic!("a refused entry leaves the import unanswered: {snapshot}"));
    assert_eq!(
        gate["message"], bare_message,
        "the note is evidence; the message is the gate's own"
    );
    let notes = evidence(gate)
        .into_iter()
        .filter(|step| step.starts_with(CATALOG_NOTE))
        .collect::<Vec<_>>();
    assert_eq!(notes, [detail], "{gate}");

    // A receipt that states no environment, in its own tree.
    let unstated = scratch.join("unstated");
    consumer_tree(&unstated, "1.0.0");
    let trust = certify(&unstated, None);
    let snapshot = analysis(&unstated, Some(&trust), &typefacts);
    let rows = summaries(&snapshot);
    assert_eq!(rows.len(), 1, "{snapshot}");
    assert_eq!(rows[0]["evidence"], "refused", "{}", rows[0]);
    let detail = rows[0]["detail"].as_str().unwrap();
    assert!(
        detail.starts_with(CATALOG_NOTE) && detail.contains("states no dependency environment"),
        "{detail}"
    );
    let gate = acceptance_gate(&snapshot).unwrap_or_else(|| panic!("{snapshot}"));
    assert_eq!(gate["message"], bare_message);
    assert!(evidence(gate).contains(&detail), "{gate}");
    let _ = fs::remove_dir_all(scratch);
}

#[test]
fn a_project_catalog_applies_only_in_the_tree_whose_environment_it_certified() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch = temporary_directory("project-catalog-environment");
    let certified = [dependency("1.0.0")];

    // Certified in its own tree: both project files are admitted.
    let own = scratch.join("own");
    consumer_tree(&own, "1.0.0");
    let trust = certify(&own, Some(&certified));
    assert_eq!(
        contract_status(&own, &trust, &typefacts).as_deref(),
        Ok("certified"),
        "a catalog certified in this tree is admitted for every file in it"
    );

    // The same catalog, carried into a tree that installs the same package
    // bytes beside a different dependency: refused.
    let elsewhere = scratch.join("elsewhere");
    copy_tree(&own, &elsewhere);
    fs::write(
        elsewhere
            .join("node_modules")
            .join(DEPENDENCY)
            .join("package.json"),
        serde_json::to_vec(&serde_json::json!({ "name": DEPENDENCY, "version": "2.0.0" })).unwrap(),
    )
    .unwrap();
    write_lockfile(&elsewhere, "2.0.0");
    assert_eq!(
        contract_status(&elsewhere, &trust, &typefacts).as_deref(),
        Ok("missing"),
        "the environment the receipt signs is not the one this tree installs"
    );
    // The control: the copy with the certified dependency restored is admitted,
    // so the refusal above is the environment and nothing else about the copy.
    let restored = scratch.join("restored");
    copy_tree(&own, &restored);
    assert_eq!(
        contract_status(&restored, &trust, &typefacts).as_deref(),
        Ok("certified")
    );

    // A tampered acceptance root: rewritten in the receipt and in the catalog
    // together, with every digest the catalog states recomputed. The
    // signature covers the root, so the catalog is refused.
    let tampered = scratch.join("tampered");
    copy_tree(&own, &tampered);
    let catalog_path = tampered.join(".solid-checker/accepted-contracts.json");
    let mut catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(&catalog_path).unwrap()).unwrap();
    let receipt_member = catalog["contracts"][0]["receipt"]
        .as_str()
        .unwrap()
        .to_owned();
    let original = catalog["contracts"][0]["bindings"]["artifactAcceptanceRoot"]
        .as_str()
        .unwrap()
        .to_owned();
    let repointed = stand_in(4242);
    // A textual edit, so the receipt stays in its canonical encoding and the
    // signature is the only thing left to object.
    let receipt_bytes = String::from_utf8(fs::read(tampered.join(&receipt_member)).unwrap())
        .unwrap()
        .replace(&original, &repointed)
        .into_bytes();
    fs::write(tampered.join(&receipt_member), &receipt_bytes).unwrap();
    catalog["contracts"][0]["bindings"]["artifactAcceptanceRoot"] = repointed.into();
    catalog["contracts"][0]["receiptDigest"] = format!(
        "sha256:{:x}",
        <sha2::Sha256 as sha2::Digest>::digest(&receipt_bytes)
    )
    .into();
    fs::write(&catalog_path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    let refusal = contract_status(&tampered, &trust, &typefacts)
        .expect_err("a re-pointed acceptance root must not authenticate");
    assert!(
        refusal.contains("receipt signature is invalid"),
        "refused for the wrong reason: {refusal}"
    );

    // A receipt that states no environment -- every one issued before ADR
    // 0123 -- in its own tree: it authenticates, and reaches no project file,
    // because which environment it was proven in is not a fact it states.
    let unstated = scratch.join("unstated");
    consumer_tree(&unstated, "1.0.0");
    let trust = certify(&unstated, None);
    assert_eq!(
        contract_status(&unstated, &trust, &typefacts).as_deref(),
        Ok("missing")
    );
    let _ = fs::remove_dir_all(scratch);
}

/// The other package the pnpm-shaped certification read. It never looks
/// [`DEPENDENCY`] up, but from its store directory Node would find the hoisted
/// copy.
const OTHER: &str = "other-dependency";

/// Lockfile integrities in the registry's own shape, so the pnpm reader
/// accepts them.
fn pnpm_integrity(name: &str, version: &str) -> &'static str {
    match (name, version) {
        (DEPENDENCY, "1.0.0") => {
            "sha512-bIWK3pjL6X8xzYeLy7IulQPzWiu9X+STXDJcTdVDwxuRkVpkRxnSxKUnO4zxbNgl+aFNfX2njYMJtmXe1FZLbQ=="
        }
        (DEPENDENCY, "2.0.0") => {
            "sha512-wHGd0HR8SA/seGdNKeXB4t7C1NSq7gfSl7et6Ur/C6Y7ZvEY4+uCqlpgHxCvbEc3LsAKsfp2u8IlWX+GuGnoXQ=="
        }
        (OTHER, "1.0.0") => {
            "sha512-HOE9tF7p7QFJGb7HlR+JuopihXGxn4JRrH7j7NW6Dn8wQf388tajhdpShsKoKk5Pa14dAs6qSXuDDWFj8mm9CA=="
        }
        _ => unreachable!("no such stub package"),
    }
}

/// A pnpm-shaped consumer, the shape measured in kobalte core: the package
/// lives in its store directory beside the two dependencies it resolves
/// (`environment-dependency@1.0.0` and `other-dependency@1.0.0`), while
/// `.pnpm/node_modules` hoists `environment-dependency@2.0.0`, which is what
/// `other-dependency` sees from its own store directory.
#[cfg(unix)]
fn pnpm_consumer_tree(project: &Path) {
    use std::os::unix::fs::symlink;
    consumer_tree(project, "1.0.0");
    // Rebuild node_modules in pnpm's layout from the flat tree just written.
    let flat = project.join("node_modules");
    let staged = project.join("flat-node-modules");
    fs::rename(&flat, &staged).unwrap();
    let store = flat.join(".pnpm");
    let package_store = store.join("@solid-primitives+debounce@1.3.0/node_modules");
    fs::create_dir_all(package_store.join("@solid-primitives")).unwrap();
    fs::rename(staged.join(PACKAGE), package_store.join(PACKAGE)).unwrap();
    let stub = |name: &str, version: &str| {
        let root = store
            .join(format!("{name}@{version}"))
            .join("node_modules")
            .join(name);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            serde_json::to_vec(&serde_json::json!({ "name": name, "version": version })).unwrap(),
        )
        .unwrap();
        root
    };
    let dependency_one = stub(DEPENDENCY, "1.0.0");
    let dependency_two = stub(DEPENDENCY, "2.0.0");
    let other = stub(OTHER, "1.0.0");
    symlink(&dependency_one, package_store.join(DEPENDENCY)).unwrap();
    symlink(&other, package_store.join(OTHER)).unwrap();
    fs::create_dir_all(store.join("node_modules")).unwrap();
    symlink(&dependency_two, store.join("node_modules").join(DEPENDENCY)).unwrap();
    fs::create_dir_all(flat.join("@solid-primitives")).unwrap();
    symlink(package_store.join(PACKAGE), flat.join(PACKAGE)).unwrap();
    // The importer the receipt binds sits beside the package's real root, in
    // the store directory, and -- as in the flat tree -- no longer exists.
    fs::remove_dir_all(&staged).unwrap();
    fs::remove_file(project.join("package-lock.json")).unwrap();
    let entry = |name: &str, version: &str, integrity: &str| {
        format!("  '{name}@{version}':\n    resolution: {{integrity: {integrity}}}\n")
    };
    fs::write(
        project.join("pnpm-lock.yaml"),
        format!(
            "lockfileVersion: '9.0'\n\nsettings:\n  autoInstallPeers: true\n\npackages:\n\n{}\n{}\n{}\n{}",
            entry(PACKAGE, "1.3.0", INTEGRITY),
            entry(DEPENDENCY, "1.0.0", pnpm_integrity(DEPENDENCY, "1.0.0")),
            entry(DEPENDENCY, "2.0.0", pnpm_integrity(DEPENDENCY, "2.0.0")),
            entry(OTHER, "1.0.0", pnpm_integrity(OTHER, "1.0.0")),
        ),
    )
    .unwrap();
}

/// The environment the certification in [`pnpm_consumer_tree`] read: with
/// the lookup that reached each entry, or -- the form every receipt issued
/// before edges were recorded has -- without.
fn pnpm_environment(edges: bool) -> Vec<DependencyEnvironmentEntry> {
    let entry = |name: &str| {
        let entry =
            DependencyEnvironmentEntry::package(name, "1.0.0", pnpm_integrity(name, "1.0.0"));
        if edges {
            entry.resolved_from(EnvironmentImporter::Certified, name)
        } else {
            entry
        }
    };
    let mut environment = vec![entry(DEPENDENCY), entry(OTHER)];
    environment.sort();
    environment
}

/// The measured defect and its repair, through the real binary: a
/// certification made in a pnpm tree is admitted in that tree when its
/// environment records who resolved what, although another package there sees
/// a different, hoisted version of a name the certification read. Stated
/// without edges, the same packages are read by the strict all-lookups rule,
/// which refuses the very tree the certification ran in -- and the
/// self-admission check `contract certify` now runs says so.
#[cfg(unix)]
#[test]
fn a_pnpm_certification_with_resolution_edges_is_admitted_in_its_own_tree() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch = temporary_directory("project-catalog-pnpm-edges");
    let issued = |project: &Path, environment: &[DependencyEnvironmentEntry]| {
        let catalog: serde_json::Value = serde_json::from_slice(
            &fs::read(project.join(".solid-checker/accepted-contracts.json")).unwrap(),
        )
        .unwrap();
        let bindings = &catalog["contracts"][0]["bindings"];
        assert_eq!(
            bindings["dependencyEnvironmentRoot"],
            policy2_dependency_environment_root(environment)
        );
        std::collections::BTreeSet::from([(
            bindings["artifactAcceptanceRoot"]
                .as_str()
                .unwrap()
                .to_owned(),
            bindings["dependencyEnvironmentRoot"]
                .as_str()
                .unwrap()
                .to_owned(),
        )])
    };

    let edged = scratch.join("edged");
    pnpm_consumer_tree(&edged);
    let environment = pnpm_environment(true);
    let trust = certify(&edged, Some(&environment));
    assert_eq!(
        contract_status(&edged, &trust, &typefacts).as_deref(),
        Ok("certified"),
        "an edge-bearing certification is admitted in the pnpm tree it was made in"
    );
    let real_root = edged
        .join("node_modules")
        .join(PACKAGE)
        .canonicalize()
        .unwrap();
    assert_eq!(
        certified_catalog_self_admission(
            &edged.join(".solid-checker"),
            PACKAGE,
            &real_root,
            &issued(&edged, &environment),
        )
        .unwrap(),
        [(PACKAGE.to_owned(), None)],
        "and the certifier's own admission check agrees"
    );

    // The same packages stated without edges: refused in the same tree,
    // naming the hoisted copy another package sees.
    let strict = scratch.join("strict");
    pnpm_consumer_tree(&strict);
    let environment = pnpm_environment(false);
    let trust = certify(&strict, Some(&environment));
    assert_eq!(
        contract_status(&strict, &trust, &typefacts).as_deref(),
        Ok("missing")
    );
    let real_root = strict
        .join("node_modules")
        .join(PACKAGE)
        .canonicalize()
        .unwrap();
    let refusals = certified_catalog_self_admission(
        &strict.join(".solid-checker"),
        PACKAGE,
        &real_root,
        &issued(&strict, &environment),
    )
    .unwrap();
    assert_eq!(refusals.len(), 1);
    assert_eq!(
        refusals[0].1.as_ref().map(ToString::to_string).as_deref(),
        Some(
            "its dependency environment differs: environment-dependency installed 2.0.0, \
             certified 1.0.0"
        )
    );

    // An edge whose importer's own lookup differs is still refused, with the
    // edge named: the package's own copy repointed at the hoisted version.
    let repointed = scratch.join("repointed");
    pnpm_consumer_tree(&repointed);
    let environment = pnpm_environment(true);
    let trust = certify(&repointed, Some(&environment));
    let package_store =
        repointed.join("node_modules/.pnpm/@solid-primitives+debounce@1.3.0/node_modules");
    fs::remove_file(package_store.join(DEPENDENCY)).unwrap();
    std::os::unix::fs::symlink(
        repointed.join(format!(
            "node_modules/.pnpm/{DEPENDENCY}@2.0.0/node_modules/{DEPENDENCY}"
        )),
        package_store.join(DEPENDENCY),
    )
    .unwrap();
    assert_eq!(
        contract_status(&repointed, &trust, &typefacts).as_deref(),
        Ok("missing")
    );
    let real_root = repointed
        .join("node_modules")
        .join(PACKAGE)
        .canonicalize()
        .unwrap();
    let refusals = certified_catalog_self_admission(
        &repointed.join(".solid-checker"),
        PACKAGE,
        &real_root,
        &issued(&repointed, &environment),
    )
    .unwrap();
    assert_eq!(
        refusals[0].1.as_ref().map(ToString::to_string).as_deref(),
        Some(
            "its dependency environment differs: environment-dependency resolved from \
             @solid-primitives/debounce@1.3.0 installed 2.0.0, certified 1.0.0"
        )
    );
    let _ = fs::remove_dir_all(scratch);
}

/// A monorepo whose packages share one hoisted install, npm-workspaces style:
/// the package, its dependency and the lockfile at the root, and two packages
/// importing it -- `packages/a` (`App.ts`, `Other.ts`) and `packages/b`
/// (`Main.ts`). The root `tsconfig.json` covers both packages' sources, and
/// each package has its own.
fn monorepo_tree(root: &Path) {
    consumer_tree(root, "1.0.0");
    fs::remove_dir_all(root.join("src")).unwrap();
    let options = r#""compilerOptions":{"target":"ES2022","module":"ESNext","moduleResolution":"Bundler","strict":true,"noEmit":true,"skipLibCheck":true}"#;
    fs::write(
        root.join("tsconfig.json"),
        format!(r#"{{{options},"include":["packages/a/src","packages/b/src"]}}"#),
    )
    .unwrap();
    for (package, files) in [
        ("a", &[("App.ts", "save"), ("Other.ts", "search")][..]),
        ("b", &[("Main.ts", "submit")][..]),
    ] {
        let directory = root.join("packages").join(package);
        fs::create_dir_all(directory.join("src")).unwrap();
        fs::write(
            directory.join("tsconfig.json"),
            format!(r#"{{{options},"include":["src"]}}"#),
        )
        .unwrap();
        for (file, name) in files {
            fs::write(
                directory.join("src").join(file),
                format!(
                    "import {{ createDebounce }} from \"{PACKAGE}\";\n\nexport const {name} = createDebounce((value: string) => value.length, 100);\n"
                ),
            )
            .unwrap();
        }
    }
}

/// Ordinary analysis of the project whose `tsconfig.json` is in `project`, and
/// what it printed to stderr.
fn analysis_with_notice(
    project: &Path,
    trust: Option<&Path>,
    typefacts: &str,
) -> (serde_json::Value, String) {
    analysis_in(project, trust, typefacts, &[])
}

/// [`analysis_with_notice`] with `environment` set -- the retained daemon's
/// switches, for one.
fn analysis_in(
    project: &Path,
    trust: Option<&Path>,
    typefacts: &str,
    environment: &[(&str, &str)],
) -> (serde_json::Value, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command.envs(environment.iter().copied());
    command.args([
        "--project",
        &project.join("tsconfig.json").to_string_lossy(),
        "--typefacts",
        typefacts,
        "--format",
        "json",
        "--no-bundled-contracts",
    ]);
    if let Some(trust) = trust {
        command.args(["--receipt-trust-configuration", &trust.to_string_lossy()]);
    }
    let output = command.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let snapshot = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("analysis produced no snapshot ({error}): {stderr}"));
    (snapshot, stderr)
}

/// The files, by name, at which the package's import reaches the acceptance
/// gate.
fn gated_files(snapshot: &serde_json::Value) -> Vec<String> {
    let mut files = snapshot["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| {
            finding["id"] == "SC9005"
                && finding["analysisContext"]
                    == "no receipt-accepted contract matches this exact import"
                && finding["message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains(PACKAGE)
        })
        .map(|finding| {
            Path::new(finding["primaryLocation"]["path"].as_str().unwrap())
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

/// Every finding in `packages/a`'s files, spelled independently of which
/// project analysed them.
fn package_a_findings(snapshot: &serde_json::Value) -> Vec<String> {
    let mut findings = snapshot["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|finding| {
            let path = finding["primaryLocation"]["path"].as_str()?;
            let name = Path::new(path).file_name()?.to_string_lossy().into_owned();
            ["App.ts", "Other.ts"].contains(&name.as_str()).then(|| {
                format!(
                    "{name}:{}:{} {} {}",
                    finding["primaryLocation"]["startByte"],
                    finding["primaryLocation"]["endByte"],
                    finding["id"],
                    finding["message"]
                )
            })
        })
        .collect::<Vec<_>>();
    findings.sort();
    findings
}

#[test]
fn a_package_catalog_applies_to_its_files_from_the_monorepo_root_and_not_to_a_sibling() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch = temporary_directory("nested-catalog");
    let certified = [dependency("1.0.0")];
    let root = scratch.join("mono");
    monorepo_tree(&root);
    let package_a = root.join("packages/a");
    let trust = certify_into(&root, &package_a.join(".solid-checker"), Some(&certified));

    // From the package's own directory: its own catalog, as it always was.
    let (own, _) = analysis_with_notice(&package_a, Some(&trust), &typefacts);
    assert!(gated_files(&own).is_empty(), "{own}");
    let rows = summaries(&own);
    assert_eq!(rows.len(), 1, "{own}");
    assert_eq!(rows[0]["evidence"], "accepted");
    assert_eq!(
        contract_status(&package_a, &trust, &typefacts).as_deref(),
        Ok("certified")
    );

    // From the monorepo root: the same files are admitted by the package's
    // catalog, found beside them rather than beside the root tsconfig, and
    // answered exactly as the package's own analysis answers them.
    let (from_root, _) = analysis_with_notice(&root, Some(&trust), &typefacts);
    assert_eq!(
        gated_files(&from_root),
        ["Main.ts"],
        "the sibling package's file has no catalog of its own and the root has none: {from_root}"
    );
    assert_eq!(package_a_findings(&from_root), package_a_findings(&own));
    let rows = summaries(&from_root);
    assert!(
        rows.iter().any(|row| row["evidence"] == "accepted"),
        "{from_root}"
    );
    // `contract check` counts per importer, from the same index: two of the
    // three importers bind, and the sibling's does not.
    assert_eq!(
        contract_status(&root, &trust, &typefacts).as_deref(),
        Ok("unbound")
    );
    assert_eq!(
        contract_status(&root.join("packages/b"), &trust, &typefacts).as_deref(),
        Ok("missing"),
        "a catalog in a sibling package does not apply to this package's files"
    );

    // Without trust the nested catalog is withheld like the project's own:
    // named in the one notice, and the analysis is the no-catalog one.
    let (withheld, stderr) = analysis_with_notice(&root, None, &typefacts);
    let catalog = package_a.join(".solid-checker/accepted-contracts.json");
    assert!(
        stderr.contains(&catalog.display().to_string()) && stderr.contains(PACKAGE),
        "{stderr}"
    );
    // The gate names a specifier once, at its first unanswered import, and
    // with the catalog withheld that is `packages/a`'s again.
    assert_eq!(gated_files(&withheld), ["App.ts"]);
    let _ = fs::remove_dir_all(scratch);
}

/// The release default is the retained daemon, whose cached answer has to be
/// invalidated by a nested catalog appearing -- in a directory that is
/// neither the project's nor a source file's, so no directory stamp it holds
/// moves -- and whose client prints a notice only the daemon can compute.
#[test]
fn the_retained_daemon_sees_a_package_catalog_appear_and_names_it_when_withheld() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let daemon = [
        ("SOLID_CHECKER_DAEMON", "1"),
        ("SOLID_CHECKER_DAEMON_IDLE_SECS", "2"),
    ];
    let scratch = temporary_directory("nested-catalog-daemon");
    let certified = [dependency("1.0.0")];
    let root = scratch.join("mono");
    monorepo_tree(&root);
    // The issuer is fixed, so a certification published anywhere else writes
    // the trust configuration this tree's will need, before it has a catalog.
    let trust = certify_into(
        &root,
        &scratch.join("elsewhere/.solid-checker"),
        Some(&certified),
    );
    let (before, _) = analysis_in(&root, Some(&trust), &typefacts, &daemon);
    assert_eq!(gated_files(&before), ["App.ts"], "{before}");
    certify_into(
        &root,
        &root.join("packages/a/.solid-checker"),
        Some(&certified),
    );
    let (after, _) = analysis_in(&root, Some(&trust), &typefacts, &daemon);
    assert_eq!(gated_files(&after), ["Main.ts"], "{after}");
    let (withheld, stderr) = analysis_in(&root, None, &typefacts, &daemon);
    let catalog = root.join("packages/a/.solid-checker/accepted-contracts.json");
    assert!(stderr.contains(&catalog.display().to_string()), "{stderr}");
    assert_eq!(stderr.matches("was not read").count(), 1, "{stderr}");
    assert_eq!(gated_files(&withheld), ["App.ts"]);
    let _ = fs::remove_dir_all(scratch);
}

#[test]
fn the_nearest_catalog_answers_and_a_farther_one_is_the_fallback() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch = temporary_directory("nested-catalog-nearest");
    let installed = [dependency("1.0.0")];
    let elsewhere = [dependency("2.0.0")];

    // Nearest admitted, farther refused: `packages/a`'s files take the
    // nearest answer, and `packages/b`'s -- below the farther catalog only --
    // are refused with the farther catalog's own reason.
    let root = scratch.join("nearest");
    monorepo_tree(&root);
    let trust = certify_into(
        &root,
        &root.join("packages/a/.solid-checker"),
        Some(&installed),
    );
    certify_into(
        &root,
        &root.join("packages/.solid-checker"),
        Some(&elsewhere),
    );
    let (snapshot, _) = analysis_with_notice(&root, Some(&trust), &typefacts);
    assert_eq!(gated_files(&snapshot), ["Main.ts"], "{snapshot}");
    let gate = acceptance_gate(&snapshot).unwrap();
    assert!(
        evidence(gate)
            .iter()
            .any(|step| step.starts_with(CATALOG_NOTE)
                && step.contains("dependency environment differs")),
        "{gate}"
    );

    // Nearest refused, farther admitted: the farther catalog answers what the
    // nearest does not, for both packages.
    let root = scratch.join("fallback");
    monorepo_tree(&root);
    let trust = certify_into(&root, &root.join("packages/a/.solid-checker"), None);
    certify_into(
        &root,
        &root.join("packages/.solid-checker"),
        Some(&installed),
    );
    let (snapshot, _) = analysis_with_notice(&root, Some(&trust), &typefacts);
    assert!(gated_files(&snapshot).is_empty(), "{snapshot}");
    assert_eq!(
        contract_status(&root, &trust, &typefacts).as_deref(),
        Ok("certified")
    );
    let _ = fs::remove_dir_all(scratch);
}

/// An unhoisted monorepo, pnpm-workspace style without the store: the root
/// `tsconfig.json` covers `packages/a` (`App.ts`, `Other.ts`) and
/// `packages/b` (`Main.ts`), and each package installs its own copy of the
/// package and its dependency under its own `node_modules`, with nothing at
/// the root but the one lockfile governing both installs. `packages/a`'s copy
/// is the certified bytes; `packages/b`'s has another tarball integrity.
fn unhoisted_monorepo_tree(root: &Path) {
    monorepo_tree(root);
    fs::rename(
        root.join("node_modules"),
        root.join("packages/a/node_modules"),
    )
    .unwrap();
    copy_tree(
        &root.join("packages/a/node_modules"),
        &root.join("packages/b/node_modules"),
    );
    fs::write(
        root.join("package-lock.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "name": "mono",
            "lockfileVersion": 3,
            "packages": {
                "": { "name": "mono" },
                format!("packages/a/node_modules/{PACKAGE}"): {
                    "version": "1.3.0", "integrity": INTEGRITY
                },
                format!("packages/a/node_modules/{DEPENDENCY}"): {
                    "version": "1.0.0", "integrity": dependency("1.0.0").integrity
                },
                format!("packages/b/node_modules/{PACKAGE}"): {
                    "version": "1.3.0", "integrity": "sha512-another-tarball"
                },
                format!("packages/b/node_modules/{DEPENDENCY}"): {
                    "version": "1.0.0", "integrity": dependency("1.0.0").integrity
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
}

/// `contract check` on `project`: every row for the package, as JSON.
fn contract_rows(project: &Path, trust: &Path, typefacts: &str) -> Vec<serde_json::Value> {
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .args([
            "--project",
            &project.join("tsconfig.json").to_string_lossy(),
            "--typefacts",
            typefacts,
            "--check-contracts",
            "--format",
            "json",
            "--receipt-trust-configuration",
            &trust.to_string_lossy(),
            "--no-bundled-contracts",
        ])
        .output()
        .unwrap();
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "no contract report ({error}): {}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
    report["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["name"] == PACKAGE)
        .cloned()
        .collect()
}

/// Admission from a monorepo root is evaluated from each importer's own
/// install. The root catalog's acceptance names `packages/a`'s copy, so
/// `packages/a`'s files are admitted from the root exactly as the lookup from
/// their own directory finds that copy -- until this, the root's own lookup
/// found no copy at all and refused every file with "no exact lockfile
/// integrity". `packages/b`'s copy is other bytes, admitted independently and
/// refused, and told why from its own install.
#[test]
fn a_monorepo_root_admits_each_importer_from_the_install_its_resolution_reaches() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let scratch = temporary_directory("importer-admission");
    let certified = [dependency("1.0.0")];
    let root = scratch.join("mono");
    unhoisted_monorepo_tree(&root);
    let trust = certify_into(
        &root.join("packages/a"),
        &root.join(".solid-checker"),
        Some(&certified),
    );

    let (snapshot, _) = analysis_with_notice(&root, Some(&trust), &typefacts);
    assert_eq!(
        gated_files(&snapshot),
        ["Main.ts"],
        "only packages/b's copy differs from the certified artifact: {snapshot}"
    );
    let gate = acceptance_gate(&snapshot).unwrap();
    let notes = evidence(gate)
        .into_iter()
        .filter(|step| step.starts_with(CATALOG_NOTE))
        .collect::<Vec<_>>();
    assert_eq!(
        notes,
        [format!(
            "{CATALOG_NOTE}the installed package's bytes or requested entrypoint are not the \
             certified ones (its acceptance root is not reproduced)"
        )],
        "packages/b's refusal is replayed from its own install: {gate}"
    );
    let rows = summaries(&snapshot);
    assert!(
        rows.iter().any(|row| row["evidence"] == "accepted"),
        "{snapshot}"
    );

    // `contract check` agrees, one row per install, each from its own.
    let rows = contract_rows(&root, &trust, &typefacts);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0]["status"], "certified", "{}", rows[0]);
    assert_eq!(
        rows[0]["importers"],
        serde_json::json!(["packages/a/src/App.ts", "packages/a/src/Other.ts"])
    );
    assert_eq!(rows[1]["status"], "missing", "{}", rows[1]);
    assert_eq!(rows[1]["installedIntegrity"], "sha512-another-tarball");
    assert_eq!(
        rows[1]["importers"],
        serde_json::json!(["packages/b/src/Main.ts"])
    );
    let detail = rows[1]["detail"].as_str().unwrap();
    assert!(
        detail.contains("acceptance root is not reproduced")
            && !detail.contains("no exact lockfile integrity"),
        "{detail}"
    );

    // The negative control: with `packages/a`'s copy repacked too, nothing is
    // admitted anywhere -- the admission above was the install's, not the
    // root's catalog applying by name.
    let repacked = scratch.join("repacked");
    copy_tree(&root, &repacked);
    let lockfile = fs::read_to_string(repacked.join("package-lock.json"))
        .unwrap()
        .replace(INTEGRITY, "sha512-repacked-tarball");
    fs::write(repacked.join("package-lock.json"), lockfile).unwrap();
    let (snapshot, _) = analysis_with_notice(&repacked, Some(&trust), &typefacts);
    assert_eq!(
        gated_files(&snapshot),
        ["App.ts"],
        "the gate names a specifier once, at its first unanswered import: {snapshot}"
    );
    let rows = contract_rows(&repacked, &trust, &typefacts);
    assert!(
        rows.iter().all(|row| row["status"] == "missing"),
        "{rows:?}"
    );
    let _ = fs::remove_dir_all(scratch);
}

/// The retained daemon's cached answer reads a sub-package's install when
/// admission did: repacking `packages/a`'s copy in place -- no source file,
/// and no lockfile or manifest the root's own lookup reads, changes -- must
/// not keep serving the admitted answer.
#[test]
fn the_retained_daemon_rereads_the_sub_package_install_admission_read() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let daemon = [
        ("SOLID_CHECKER_DAEMON", "1"),
        ("SOLID_CHECKER_DAEMON_IDLE_SECS", "2"),
    ];
    let scratch = temporary_directory("importer-admission-daemon");
    let certified = [dependency("1.0.0")];
    let root = scratch.join("mono");
    unhoisted_monorepo_tree(&root);
    let trust = certify_into(
        &root.join("packages/a"),
        &root.join(".solid-checker"),
        Some(&certified),
    );
    let (before, _) = analysis_in(&root, Some(&trust), &typefacts, &daemon);
    assert_eq!(gated_files(&before), ["Main.ts"], "{before}");
    // A patch release installed in place of `packages/a`'s copy: the manifest
    // under `packages/a/node_modules` is the only file that moved.
    let manifest = root
        .join("packages/a/node_modules")
        .join(PACKAGE)
        .join("package.json");
    let rewritten = fs::read_to_string(&manifest)
        .unwrap()
        .replace("\"1.3.0\"", "\"1.3.1\"");
    fs::write(&manifest, rewritten).unwrap();
    let (after, _) = analysis_in(&root, Some(&trust), &typefacts, &daemon);
    assert_eq!(gated_files(&after), ["App.ts"], "{after}");
    let _ = fs::remove_dir_all(scratch);
}
