use super::*;
use crate::ContractFailure;
use crate::artifact_resolution::{
    ClosureManifest, ClosurePackageIdentity, ResolutionAuthority, ResolutionTrace,
    ResolvedExportBinding, ResolvedExportTarget, ResolvedFile,
};

const MAIN: &[u8] =
    include_bytes!("../../../../../../pkg/contracts/bundled/solid-v1/debounce-root-default.json");
const OTHER_MAIN: &[u8] =
    include_bytes!("../../../../../../pkg/contracts/bundled/solid-v1/solid-root-node.json");
const POLICY1_RECEIPT: &[u8] = include_bytes!("policy1-obsolete.json");

fn canonical_main(input: &[u8]) -> Vec<u8> {
    let decoded = contract_document::decode(input).unwrap();
    let sidecars = decoded.sidecar_digests().unwrap();
    let normalized = decoded.normalize().unwrap();
    contract_document::encode(&normalized, &sidecars, false).unwrap()
}

fn root(label: &str) -> String {
    digest_bytes(label.as_bytes())
}

fn witness_roots() -> BTreeMap<String, String> {
    RECEIPT_WITNESS_FAMILIES
        .iter()
        .map(|family| ((*family).into(), root(family)))
        .collect()
}

fn bindings(main: &[u8]) -> Policy2ReceiptBindings {
    let decoded = contract_document::decode(main).unwrap();
    let semantic_digest = decoded
        .normalize()
        .unwrap()
        .semantic_digest()
        .as_str()
        .into();
    Policy2ReceiptBindings {
        importer: "/workspace/src/App.tsx".into(),
        specifier: "@solid-primitives/debounce".into(),
        resolved_import_root: root("resolved-import"),
        semantic_digest,
        artifact_provenance_root: root("provenance"),
        snapshot_root: root("snapshot"),
        package_root: root("package"),
        manifest_root: root("manifest"),
        artifacts_root: root("artifacts"),
        declarations_root: root("declarations"),
        transform_root: root("transform"),
        exports_root: root("exports"),
        closure_root: root("closure"),
        demand_graph_root: root("demands"),
        verified_positive_root: root("positive"),
        witness_roots: witness_roots(),
        producer_sessions_root: root("sessions"),
        dependency_receipts_root: root("dependency-receipts"),
        dependency_trust_root: root("dependency-trust"),
        probe_gate_root: root("probes"),
        closed_claims_root: root("closed"),
        verifier_source_digest: root("verifier-source"),
        verifier_build_digest: root("verifier-build"),
    }
}

fn resolved_import() -> ResolvedImport {
    let package_root = "/workspace/node_modules/@solid-primitives/debounce";
    let runtime = ResolvedFile {
        path: format!("{package_root}/dist/index.js"),
        real_path: None,
        digest: format!(
            "sha256:{}",
            "b772f925ca55d55a6ef84c3277ab85f9bff2018c30c4269817327e267c76efe1"
        ),
    };
    let declarations = ResolvedFile {
        path: format!("{package_root}/dist/index.d.ts"),
        real_path: None,
        digest: format!(
            "sha256:{}",
            "4fe1834060a02e3a3df804927e4fd7b73eab9496cd6a5e2624dd14f1d2ec382c"
        ),
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
    let exports = BTreeMap::from([
        ("createDebounce".into(), target("createDebounce")),
        ("default".into(), target("default")),
    ]);
    ResolvedImport {
        specifier: "@solid-primitives/debounce".into(),
        importer: "/workspace/src/App.tsx".into(),
        requested_entrypoint: ".".into(),
        package_name: "@solid-primitives/debounce".into(),
        package_version: "1.3.0".into(),
        package_integrity: "sha512-Cen4ccCPTuEtQM7o9aEKuOJ0LRlAnzKvN7loEBBOQ+zKdu7/7kYKr7HHE/WS8JAI3QeQr5v2ModYRIZLERw5zw==".into(),
        package_root: package_root.into(),
        package_real_root: None,
        package_manifest: ResolvedFile {
            path: format!("{package_root}/package.json"),
            real_path: None,
            digest: "sha256:19e4b7c252d2650e1d291af601e9fa26cc35cc2f370b14d1c5861cdd008012ab".into(),
        },
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
        closure: ClosureManifest::new(Vec::new(), Vec::new(), Vec::new()).unwrap(),
        transform: None,
        exports,
        authority: ResolutionAuthority::Host,
    }
}

fn materialized_resolved_import(project: &Path) -> ResolvedImport {
    let mut resolved = resolved_import();
    fs::create_dir_all(project).unwrap();
    let project = project.canonicalize().unwrap();
    let package_root = project.join("node_modules/@solid-primitives/debounce");
    let importer = project.join("src/App.tsx");
    fs::create_dir_all(package_root.join("dist")).unwrap();
    fs::create_dir_all(importer.parent().unwrap()).unwrap();
    for path in [
        package_root.join("package.json"),
        package_root.join("dist/index.js"),
        package_root.join("dist/index.d.ts"),
        importer.clone(),
    ] {
        fs::write(path, b"fixture").unwrap();
    }
    resolved.importer = importer.to_string_lossy().into_owned();
    resolved.package_root = package_root.to_string_lossy().into_owned();
    resolved.package_manifest.path = package_root
        .join("package.json")
        .to_string_lossy()
        .into_owned();
    resolved.runtime.path = package_root
        .join("dist/index.js")
        .to_string_lossy()
        .into_owned();
    resolved.declarations.path = package_root
        .join("dist/index.d.ts")
        .to_string_lossy()
        .into_owned();
    for binding in resolved.exports.values_mut() {
        binding.runtime.module = resolved.runtime.clone();
        binding.declarations.module = resolved.declarations.clone();
    }
    resolved.closure = ClosureManifest::from_package_census(vec![
        ClosurePackageIdentity {
            name: "@solid-primitives/debounce".into(),
            version: "1.3.0".into(),
            integrity: "sha512-Cen4ccCPTuEtQM7o9aEKuOJ0LRlAnzKvN7loEBBOQ+zKdu7/7kYKr7HHE/WS8JAI3QeQr5v2ModYRIZLERw5zw==".into(),
            files_manifest_digest: "sha256:325aec3e2c3e44e50b09cae7d6210c4f36f62d04ac1acdebd7213b3c8964d97d".into(),
        },
        ClosurePackageIdentity {
            name: "csstype".into(),
            version: "3.2.3".into(),
            integrity: "sha512-z1HGKcYy2xA8AGQfwrn0PAy+PB7X/GSj3UVJW9qKyn43xWa+gl5nXmU4qqLMRzWVLFC8KusUX8T/0kCiOYpAIQ==".into(),
            files_manifest_digest: "sha256:67f35df64a494f8bcebe228c056478858ab201ab6fc0036067d92c006a689108".into(),
        },
        ClosurePackageIdentity {
            name: "seroval-plugins".into(),
            version: "1.5.6".into(),
            integrity: "sha512-HXuLAX2pu/UByPpaeo/TaMfvMIi+1QqIoPJYCcAtU8QkVNwgR6MPlGuCQTErV1JwraaMbYaWVIBX7mppzGLATQ==".into(),
            files_manifest_digest: "sha256:eb33a8834d4353e4de034bf03799d1c3991922f937161c2418358c23ce041918".into(),
        },
        ClosurePackageIdentity {
            name: "seroval".into(),
            version: "1.5.6".into(),
            integrity: "sha512-rVQVWjjSvlINzaQPZH5JFqsqEsIWdTxY3iJZCnTL/5gQbXIRooVZKI60tVCkOVfzcRPejboxO2t0P89dg5mQaA==".into(),
            files_manifest_digest: "sha256:9a392773534333f3337edb24d2ad301ef9b3c4c50d1ccdc7beefc3b4de068f80".into(),
        },
        ClosurePackageIdentity {
            name: "solid-js".into(),
            version: "1.9.14".into(),
            integrity: "sha512-sAEXC0Kk0S1EDg+8ysEWJDbYhA3RRoEjwuySUGlKIemeo0I5YZfOyumNjNs9Sv3y2nmhD+0rW66ag2HsMuQiGQ==".into(),
            files_manifest_digest: "sha256:53190caadda3870b6b66b1334c5913d8208eb0a4d429a006952a28ab49926c76".into(),
        },
    ])
    .unwrap();
    resolved
}

fn local_issuer(seed: u8) -> ConfiguredReceiptIssuer {
    ConfiguredReceiptIssuer::persistent_local("machine-a/project-a", [seed; 32]).unwrap()
}

fn portable_issuer(seed: u8) -> ConfiguredReceiptIssuer {
    ConfiguredReceiptIssuer::portable("release-chain-a", [seed; 32]).unwrap()
}

fn trust_store(
    issuer: &ConfiguredReceiptIssuer,
    bindings: &Policy2ReceiptBindings,
    revoked_at_epoch: Option<u64>,
) -> Policy2TrustStore {
    Policy2TrustStore::new(
        [Policy2TrustEntry {
            kind: issuer.kind,
            key_id: issuer.key_id().into(),
            public_key: issuer.public_key(),
            scope: issuer.scope.clone(),
            allowed_policy_digests: vec![proof_policy_2().digest().as_str().into()],
            allowed_verifier_builds: vec![bindings.verifier_build_digest.clone()],
            revoked_at_epoch,
        }],
        7,
    )
    .unwrap()
}

fn canonical_mutation(receipt: &[u8], mutate: impl FnOnce(&mut ReceiptDocument)) -> Vec<u8> {
    let mut document: ReceiptDocument = serde_json::from_slice(receipt).unwrap();
    mutate(&mut document);
    encode_receipt(document).unwrap()
}

type BindingMutation = (&'static str, fn(&mut Policy2ReceiptBindings));

#[test]
fn local_and_portable_receipts_require_configured_external_trust() {
    let main = canonical_main(MAIN);
    let bindings = bindings(&main);
    for issuer in [local_issuer(1), portable_issuer(2)] {
        let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
        let trust = trust_store(&issuer, &bindings, None);
        let provenance = match issuer.kind {
            ReceiptIssuerKind::PersistentLocal => Policy2ReceiptProvenance::PersistentLocal {
                trust_store: &trust,
                scope: &issuer.scope,
            },
            ReceiptIssuerKind::Portable => Policy2ReceiptProvenance::Portable {
                trust_store: &trust,
            },
            ReceiptIssuerKind::BuiltIn => unreachable!(),
        };
        let accepted =
            authenticate_policy2_receipt(&main, &receipt, &bindings, provenance).unwrap();
        assert_eq!(accepted.receipt_digest(), digest_bytes(&receipt));
        assert_eq!(accepted.main_digest(), digest_bytes(&main));
        assert_eq!(accepted.trust_store_digest(), trust.digest());
        assert_eq!(accepted.revocation_epoch(), 7);
        assert_eq!(
            accepted.semantic_digest().as_str(),
            bindings.semantic_digest
        );
    }
}

#[test]
fn authenticated_builtin_receipt_is_consumed_by_the_active_loader() {
    let main = canonical_main(MAIN);
    let normalized = contract_document::decode(&main)
        .unwrap()
        .normalize()
        .unwrap();
    let selected = &normalized.artifact_cases()[0].id;
    let mut bindings = bindings(&main);
    bindings.closed_claims_root =
        solid_reactive_ir::contract_semantics::proof::policy2_closed_claims_root(
            &normalized,
            selected,
        )
        .unwrap()
        .as_str()
        .into();
    let receipt = issue_builtin_policy2_receipt(&main, &bindings, "solid-v1-bundled").unwrap();
    let entry = BuiltInReceiptEntry {
        entry_digest: digest_bytes(&receipt),
        verifier_build_digest: bindings.verifier_build_digest.clone(),
    };
    let accepted = crate::contract_interface::load_authenticated_policy2_embedded_contract(
        &main, &receipt, &bindings, &entry,
    )
    .unwrap();
    assert_eq!(accepted.receipt().receipt_version, 2);
    assert_eq!(accepted.receipt().verifier.policy, 2);
    let authentication = accepted.receipt().authentication.as_ref().unwrap();
    assert_eq!(
        authentication.receipt_digest.as_str(),
        digest_bytes(&receipt)
    );
}

#[test]
fn every_mutable_certification_root_is_rechecked() {
    let main = canonical_main(MAIN);
    let bindings = bindings(&main);
    let issuer = local_issuer(3);
    let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
    let trust = trust_store(&issuer, &bindings, None);
    let verify = |expected: &Policy2ReceiptBindings| {
        authenticate_policy2_receipt(
            &main,
            &receipt,
            expected,
            Policy2ReceiptProvenance::PersistentLocal {
                trust_store: &trust,
                scope: &issuer.scope,
            },
        )
    };

    let mutations: [BindingMutation; 13] = [
        ("resolvedImportRoot", |value| {
            value.resolved_import_root = root("changed-resolved-import")
        }),
        ("artifactProvenanceRoot", |value| {
            value.artifact_provenance_root = root("stale-artifact")
        }),
        ("snapshotRoot", |value| {
            value.snapshot_root = root("changed-snapshot")
        }),
        ("closureRoot", |value| {
            value.closure_root = root("changed-closure")
        }),
        ("demandGraphRoot", |value| {
            value.demand_graph_root = root("changed-facts")
        }),
        ("verifiedPositiveRoot", |value| {
            value.verified_positive_root = root("changed-positive-facts")
        }),
        ("witnessRoots", |value| {
            value
                .witness_roots
                .insert("selected-signature".into(), root("changed-witness"));
        }),
        ("producerSessionsRoot", |value| {
            value.producer_sessions_root = root("changed-session")
        }),
        ("dependencyReceiptsRoot", |value| {
            value.dependency_receipts_root = root("changed-dependency")
        }),
        ("probeGateRoot", |value| {
            value.probe_gate_root = root("changed-probe")
        }),
        ("closedClaimsRoot", |value| {
            value.closed_claims_root = root("changed-closure-claims")
        }),
        ("verifierBuildDigest", |value| {
            value.verifier_build_digest = root("changed-verifier")
        }),
        ("verifierSourceDigest", |value| {
            value.verifier_source_digest = root("changed-verifier-source")
        }),
    ];
    for (field, mutate) in mutations {
        let mut changed = bindings.clone();
        mutate(&mut changed);
        assert_eq!(
            verify(&changed),
            Err(Policy2ReceiptError::BindingMismatch { field })
        );
    }
}

#[test]
fn policy_downgrade_and_policy_digest_substitution_are_obsolete() {
    let main = canonical_main(MAIN);
    let bindings = bindings(&main);
    let issuer = portable_issuer(4);
    let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
    let trust = trust_store(&issuer, &bindings, None);
    for changed in [
        canonical_mutation(&receipt, |document| document.payload.proof_policy = 1),
        canonical_mutation(&receipt, |document| {
            document.payload.policy_digest = root("wrong-policy")
        }),
    ] {
        assert_eq!(
            authenticate_policy2_receipt(
                &main,
                &changed,
                &bindings,
                Policy2ReceiptProvenance::Portable {
                    trust_store: &trust
                },
            ),
            Err(Policy2ReceiptError::ObsoletePolicy)
        );
    }
}

#[test]
fn policy1_receipt_gets_an_obsolete_policy_refusal() {
    let main = canonical_main(MAIN);
    let bindings = bindings(&main);
    let issuer = portable_issuer(44);
    let trust = trust_store(&issuer, &bindings, None);
    assert_eq!(
        authenticate_policy2_receipt(
            &main,
            POLICY1_RECEIPT,
            &bindings,
            Policy2ReceiptProvenance::Portable {
                trust_store: &trust,
            },
        ),
        Err(Policy2ReceiptError::ObsoletePolicy)
    );
}

#[test]
fn wrong_revoked_and_confused_issuers_fail_closed() {
    let main = canonical_main(MAIN);
    let bindings = bindings(&main);
    let issuer = local_issuer(5);
    let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
    let wrong = local_issuer(6);
    let wrong_trust = trust_store(&wrong, &bindings, None);
    assert_eq!(
        authenticate_policy2_receipt(
            &main,
            &receipt,
            &bindings,
            Policy2ReceiptProvenance::PersistentLocal {
                trust_store: &wrong_trust,
                scope: &issuer.scope,
            },
        ),
        Err(Policy2ReceiptError::UntrustedIssuer)
    );

    let revoked = trust_store(&issuer, &bindings, Some(7));
    assert_eq!(
        authenticate_policy2_receipt(
            &main,
            &receipt,
            &bindings,
            Policy2ReceiptProvenance::PersistentLocal {
                trust_store: &revoked,
                scope: &issuer.scope,
            },
        ),
        Err(Policy2ReceiptError::RevokedIssuer)
    );

    let algorithm = canonical_mutation(&receipt, |document| {
        document.payload.signature_algorithm = "ed25519ph".into()
    });
    let trust = trust_store(&issuer, &bindings, None);
    assert_eq!(
        authenticate_policy2_receipt(
            &main,
            &algorithm,
            &bindings,
            Policy2ReceiptProvenance::PersistentLocal {
                trust_store: &trust,
                scope: &issuer.scope,
            },
        ),
        Err(Policy2ReceiptError::UnsupportedAlgorithm)
    );

    let confused = canonical_mutation(&receipt, |document| {
        document.authentication.public_key = Some(STANDARD.encode([9_u8; 32]))
    });
    assert_eq!(
        authenticate_policy2_receipt(
            &main,
            &confused,
            &bindings,
            Policy2ReceiptProvenance::PersistentLocal {
                trust_store: &trust,
                scope: &issuer.scope,
            },
        ),
        Err(Policy2ReceiptError::KeyConfusion)
    );
}

#[test]
fn trust_store_constrains_exact_policy_verifier_kind_and_scope() {
    let main = canonical_main(MAIN);
    let bindings = bindings(&main);
    let issuer = portable_issuer(61);
    let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
    let constrained = |kind, scope: &str, policy: String, verifier: String| {
        Policy2TrustStore::new(
            [Policy2TrustEntry {
                kind,
                key_id: issuer.key_id().into(),
                public_key: issuer.public_key(),
                scope: scope.into(),
                allowed_policy_digests: vec![policy],
                allowed_verifier_builds: vec![verifier],
                revoked_at_epoch: None,
            }],
            3,
        )
        .unwrap()
    };
    for trust in [
        constrained(
            ReceiptIssuerKind::PersistentLocal,
            issuer.scope(),
            proof_policy_2().digest().as_str().into(),
            bindings.verifier_build_digest.clone(),
        ),
        constrained(
            ReceiptIssuerKind::Portable,
            "another-chain",
            proof_policy_2().digest().as_str().into(),
            bindings.verifier_build_digest.clone(),
        ),
        constrained(
            ReceiptIssuerKind::Portable,
            issuer.scope(),
            root("another-policy"),
            bindings.verifier_build_digest.clone(),
        ),
        constrained(
            ReceiptIssuerKind::Portable,
            issuer.scope(),
            proof_policy_2().digest().as_str().into(),
            root("another-verifier"),
        ),
    ] {
        assert_eq!(
            authenticate_policy2_receipt(
                &main,
                &receipt,
                &bindings,
                Policy2ReceiptProvenance::Portable {
                    trust_store: &trust,
                },
            ),
            Err(Policy2ReceiptError::TrustConstraint)
        );
    }
}

#[test]
fn canonical_receipt_signature_and_main_encodings_are_mandatory() {
    let main = canonical_main(MAIN);
    let bindings = bindings(&main);
    let issuer = local_issuer(7);
    let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
    let trust = trust_store(&issuer, &bindings, None);
    let provenance = || Policy2ReceiptProvenance::PersistentLocal {
        trust_store: &trust,
        scope: &issuer.scope,
    };

    let value: serde_json::Value = serde_json::from_slice(&receipt).unwrap();
    let pretty = serde_json::to_vec_pretty(&value).unwrap();
    assert_eq!(
        authenticate_policy2_receipt(&main, &pretty, &bindings, provenance()),
        Err(Policy2ReceiptError::NonCanonicalReceipt)
    );

    let noncanonical_signature = canonical_mutation(&receipt, |document| {
        document.authentication.value = document.authentication.value.trim_end_matches('=').into();
    });
    assert_eq!(
        authenticate_policy2_receipt(&main, &noncanonical_signature, &bindings, provenance(),),
        Err(Policy2ReceiptError::NonCanonicalSignature)
    );

    assert_eq!(
        issue_policy2_receipt(MAIN, &bindings, &issuer),
        Err(Policy2ReceiptError::NonCanonicalMain)
    );
}

#[test]
fn built_in_provenance_cannot_be_copied_into_a_project_catalog() {
    let main = canonical_main(MAIN);
    let bindings = bindings(&main);
    let receipt = issue_builtin_policy2_receipt(&main, &bindings, "compiled-bundle").unwrap();
    let entry = BuiltInReceiptEntry {
        entry_digest: digest_bytes(&receipt),
        verifier_build_digest: bindings.verifier_build_digest.clone(),
    };
    authenticate_policy2_receipt(
        &main,
        &receipt,
        &bindings,
        Policy2ReceiptProvenance::BuiltIn(&entry),
    )
    .unwrap();

    let local = local_issuer(8);
    let trust = trust_store(&local, &bindings, None);
    assert_eq!(
        authenticate_policy2_receipt(
            &main,
            &receipt,
            &bindings,
            Policy2ReceiptProvenance::PersistentLocal {
                trust_store: &trust,
                scope: &local.scope,
            },
        ),
        Err(Policy2ReceiptError::ProvenanceMismatch)
    );
}

#[test]
fn receipt_replay_against_another_accepted_main_is_rejected() {
    let main = canonical_main(MAIN);
    let other = canonical_main(OTHER_MAIN);
    let bindings = bindings(&main);
    let issuer = portable_issuer(9);
    let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
    let trust = trust_store(&issuer, &bindings, None);
    assert!(matches!(
        authenticate_policy2_receipt(
            &other,
            &receipt,
            &bindings,
            Policy2ReceiptProvenance::Portable {
                trust_store: &trust
            },
        ),
        Err(Policy2ReceiptError::BindingMismatch {
            field: "semanticDigest" | "mainDigest"
        })
    ));
}

#[test]
fn publication_commits_one_pointer_after_both_content_objects() {
    let main = canonical_main(MAIN);
    let bindings = bindings(&main);
    let issuer = local_issuer(10);
    let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
    let trust = trust_store(&issuer, &bindings, None);
    let authenticated = authenticate_policy2_receipt(
        &main,
        &receipt,
        &bindings,
        Policy2ReceiptProvenance::PersistentLocal {
            trust_store: &trust,
            scope: issuer.scope(),
        },
    )
    .unwrap();
    let resolved = resolved_import();
    let root = std::env::temp_dir().join(format!(
        "solid-checker-policy2-publication-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut changed_receipt = receipt.clone();
    changed_receipt.push(b' ');
    assert!(matches!(
        publish_policy2_catalog(&root, &main, &changed_receipt, &authenticated, &resolved),
        Err(ReceiptPublicationError::Unauthenticated(_))
    ));
    assert!(!root.exists());
    let mut alternate_value: serde_json::Value = serde_json::from_slice(&main).unwrap();
    alternate_value["sidecars"]["proof"]["sha256"] = "a".repeat(64).into();
    let alternate_main =
        canonicalize_policy2_main(&serde_json::to_vec(&alternate_value).unwrap()).unwrap();
    assert_eq!(
        policy2_main_semantic_digest(&alternate_main).unwrap(),
        bindings.semantic_digest
    );
    assert!(matches!(
        publish_policy2_catalog(&root, &alternate_main, &receipt, &authenticated, &resolved),
        Err(ReceiptPublicationError::Unauthenticated(_))
    ));
    assert!(!root.exists());
    let published =
        publish_policy2_catalog(&root, &main, &receipt, &authenticated, &resolved).unwrap();
    assert_eq!(fs::read(&published.main_path).unwrap(), main);
    assert_eq!(fs::read(&published.receipt_path).unwrap(), receipt);
    let pointer: serde_json::Value =
        serde_json::from_slice(&fs::read(&published.catalog_path).unwrap()).unwrap();
    assert_eq!(pointer["catalogVersion"], 2);
    assert_eq!(
        pointer["contracts"][0]["documentDigest"],
        digest_bytes(&main)
    );
    assert_eq!(
        pointer["contracts"][0]["receiptDigest"],
        digest_bytes(&receipt)
    );
    assert_eq!(
        pointer["contracts"][0]["bindings"]["importer"],
        bindings.importer
    );
    assert_eq!(
        pointer["contracts"][0]["status"],
        "policy2-persistent-local"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn normal_catalog_discovery_authenticates_the_published_policy2_entry() {
    let root = std::env::temp_dir().join(format!(
        "solid-checker-policy2-discovery-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let resolved = materialized_resolved_import(&root);
    let main = canonical_main(MAIN);
    let normalized = contract_document::decode(&main)
        .unwrap()
        .normalize()
        .unwrap();
    let mut bindings = bindings(&main);
    bindings.importer = resolved.importer.clone();
    bindings.specifier = resolved.specifier.clone();
    bindings.resolved_import_root = policy2_resolved_import_root(&resolved).unwrap();
    bindings.closed_claims_root =
        solid_reactive_ir::contract_semantics::proof::policy2_closed_claims_root(
            &normalized,
            &normalized.artifact_cases()[0].id,
        )
        .unwrap()
        .as_str()
        .into();
    let issuer = local_issuer(41);
    let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
    let trust = trust_store(&issuer, &bindings, None);
    let authenticated = authenticate_policy2_receipt(
        &main,
        &receipt,
        &bindings,
        Policy2ReceiptProvenance::PersistentLocal {
            trust_store: &trust,
            scope: issuer.scope(),
        },
    )
    .unwrap();
    let published = publish_policy2_catalog(
        &root.join(".solid-checker"),
        &main,
        &receipt,
        &authenticated,
        &resolved,
    )
    .unwrap();
    let configuration = Policy2TrustConfiguration::new(trust, Some(issuer.scope().into())).unwrap();
    let accepted = crate::contract_interface::read_accepted_contract_catalog_with_trust(
        &published.catalog_path,
        Some(&configuration),
    )
    .unwrap();
    let use_ = accepted
        .resolve_name(&resolved.importer, &resolved.specifier, "createDebounce")
        .unwrap();
    assert_eq!(use_.contract().receipt().receipt_version, 2);
    assert!(use_.contract().receipt().authentication.is_some());

    let catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(&published.catalog_path).unwrap()).unwrap();
    let mut changed_target = catalog.clone();
    changed_target["contracts"][0]["import"]["exports"]["default"]["runtime"]["exportName"] =
        "createDebounce".into();
    fs::write(
        &published.catalog_path,
        serde_json::to_vec(&changed_target).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        crate::contract_interface::read_accepted_contract_catalog_with_trust(
            &published.catalog_path,
            Some(&configuration),
        ),
        Err(ContractFailure::ReceiptMismatch {
            field: "resolvedImportRoot"
        })
    ));

    let mut catalog = catalog;
    let other_importer = root.join("src/Other.tsx");
    fs::write(&other_importer, b"fixture").unwrap();
    catalog["contracts"][0]["import"]["importer"] =
        other_importer.to_string_lossy().into_owned().into();
    fs::write(
        &published.catalog_path,
        serde_json::to_vec(&catalog).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        crate::contract_interface::read_accepted_contract_catalog_with_trust(
            &published.catalog_path,
            Some(&configuration),
        ),
        Err(ContractFailure::ReceiptMismatch { field: "importer" })
    ));
    fs::remove_dir_all(root).unwrap();
}
