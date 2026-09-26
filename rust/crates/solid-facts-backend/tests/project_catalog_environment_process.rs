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
    Policy2ReceiptBindings, Policy2ReceiptProvenance, RECEIPT_WITNESS_FAMILIES,
    ResolutionAuthority, ResolutionTrace, ResolvedExportBinding, ResolvedExportTarget,
    ResolvedFile, ResolvedImport, authenticate_policy2_receipt, canonicalize_policy2_main,
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
    // The file `contract certify` writes beside the package root and binds the
    // receipt to. No project file is it.
    fs::write(
        project.join("node_modules/@solid-primitives/.solid-checker-certification.mjs"),
        "import \"@solid-primitives/debounce\";\n",
    )
    .unwrap();
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
    let project = project.canonicalize().unwrap();
    let package_root = project.join("node_modules").join(PACKAGE);
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
        importer: project
            .join("node_modules/@solid-primitives/.solid-checker-certification.mjs")
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
        &project.join(".solid-checker"),
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
    let trust_path = project.with_extension("trust.json");
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
