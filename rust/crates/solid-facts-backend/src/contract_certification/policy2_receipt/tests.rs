use super::*;

const MAIN: &[u8] =
    include_bytes!("../../../../../../pkg/contracts/bundled/solid-v1/debounce-root-default.json");
const OTHER_MAIN: &[u8] =
    include_bytes!("../../../../../../pkg/contracts/bundled/solid-v1/solid-root-node.json");
const POLICY1_RECEIPT: &[u8] = include_bytes!(
    "../../../../../../pkg/contracts/bundled/solid-v1/debounce-root-default.receipt.json"
);

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

    let mutations: [BindingMutation; 12] = [
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
        publish_policy2_catalog(&root, &main, &changed_receipt, &authenticated),
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
        publish_policy2_catalog(&root, &alternate_main, &receipt, &authenticated),
        Err(ReceiptPublicationError::Unauthenticated(_))
    ));
    assert!(!root.exists());
    let published = publish_policy2_catalog(&root, &main, &receipt, &authenticated).unwrap();
    assert_eq!(fs::read(&published.main_path).unwrap(), main);
    assert_eq!(fs::read(&published.receipt_path).unwrap(), receipt);
    let pointer: serde_json::Value =
        serde_json::from_slice(&fs::read(&published.catalog_path).unwrap()).unwrap();
    assert_eq!(pointer["catalogVersion"], 2);
    assert_eq!(pointer["main"]["digest"], digest_bytes(&main));
    assert_eq!(pointer["receipt"]["digest"], digest_bytes(&receipt));
    fs::remove_dir_all(root).unwrap();
}
