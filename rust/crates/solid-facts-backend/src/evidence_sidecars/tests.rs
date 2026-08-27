use super::*;

const SIGNAL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../benchmarks/package-contract-v2/phase6/signal-pair-complete.json"
));

fn digest(byte: char) -> Digest {
    Digest::parse(format!("sha256:{}", byte.to_string().repeat(64))).unwrap()
}

fn tool(name: &str, byte: char) -> ToolIdentity {
    ToolIdentity {
        name: name.into(),
        version: "1.0.0".into(),
        build: digest(byte),
        protocol: Some("1".into()),
    }
}

fn fixture_contract() -> NormalizedContract {
    contract_document_v2::decode(SIGNAL)
        .unwrap()
        .normalize()
        .unwrap()
}

fn fixture_subjects(contract: &NormalizedContract) -> (SemanticClaimSubject, SemanticClaimSubject) {
    let case = &contract.artifact_cases()[0];
    let export = &case.exports["createSignal"];
    (
        SemanticClaimSubject {
            artifact_case: case.id.clone(),
            export: "createSignal".into(),
            path: SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Reads)),
        },
        SemanticClaimSubject {
            artifact_case: case.id.clone(),
            export: "createSignal".into(),
            path: SemanticClaimPath::Operation(export.call.operations[0].id.clone()),
        },
    )
}

fn proof_material(subject: SemanticClaimSubject) -> ProofClaimMaterial {
    ProofClaimMaterial {
        subject,
        producer: tool("proposal-proof-planner", '1'),
        fact_transcripts: vec![
            FactTranscriptIdentity {
                domain: FactDomainIdentity::TypeFacts,
                transcript: digest('3'),
                generation: Some(7),
                producer: tool("solid-typefacts", '4'),
            },
            FactTranscriptIdentity {
                domain: FactDomainIdentity::OxcSyntax,
                transcript: digest('2'),
                generation: None,
                producer: tool("solid-checker", '5'),
            },
        ],
        proof_inputs: vec![ProofInputIdentity {
            rule: "operation-reachability".into(),
            input: digest('6'),
            tool: tool("proof-planner", '7'),
        }],
        coverage_limitations: vec!["open dependency edge".into(), "dynamic loader".into()],
    }
}

fn probe_material(subject: SemanticClaimSubject) -> ProbeClaimMaterial {
    ProbeClaimMaterial {
        subject,
        producer: tool("probe-planner", '8'),
        recipe: digest('9'),
        environment: EnvironmentIdentity {
            runtime: tool("bun", 'a'),
            os: "darwin".into(),
            architecture: "arm64".into(),
            conditions: vec!["production".into(), "browser".into()],
            sandbox: SandboxIdentity {
                kind: SandboxKind::Container,
                policy: Some(digest('b')),
            },
        },
        outcome: ProbeOutcome::Falsification {
            transcript: digest('c'),
        },
        coverage_limitations: vec!["one exact artifact mode".into()],
    }
}

fn fixture() -> (
    EvidenceCatalog,
    EvidenceSidecarDocuments,
    SemanticClaimSubject,
    SemanticClaimSubject,
) {
    let contract = fixture_contract();
    let (proof_subject, probe_subject) = fixture_subjects(&contract);
    let catalog =
        EvidenceCatalog::new(contract, [proof_subject.clone()], [probe_subject.clone()]).unwrap();
    let documents = emit_evidence_sidecars(
        &catalog,
        tool("solid-contract-evidence", 'd'),
        vec![proof_material(proof_subject.clone())],
        vec![probe_material(probe_subject.clone())],
    )
    .unwrap();
    (catalog, documents, proof_subject, probe_subject)
}

fn main_with_references(references: &EvidenceSidecarReferences) -> Vec<u8> {
    let mut value: serde_json::Value = serde_json::from_slice(SIGNAL).unwrap();
    if let Some(proof) = &references.proof {
        value["sidecars"]["proof"] = serde_json::json!({
            "sha256": proof.as_str().strip_prefix("sha256:").unwrap()
        });
    }
    if let Some(probes) = &references.probes {
        value["sidecars"]["probes"] = serde_json::json!({
            "sha256": probes.as_str().strip_prefix("sha256:").unwrap()
        });
    }
    serde_json::to_vec(&value).unwrap()
}

fn replace_reference(main: &[u8], kind: &str, bytes: &[u8]) -> Vec<u8> {
    let mut value: serde_json::Value = serde_json::from_slice(main).unwrap();
    value["sidecars"][kind]["sha256"] = serde_json::json!(
        content_digest(bytes)
            .as_str()
            .strip_prefix("sha256:")
            .unwrap()
    );
    serde_json::to_vec(&value).unwrap()
}

#[test]
fn claim_ids_survive_summary_renaming_and_wire_reformatting() {
    let expected = fixture_contract();
    let (subject, _) = fixture_subjects(&expected);
    let expected_id = expected.claim_id(&subject).unwrap();

    let mut value: serde_json::Value = serde_json::from_slice(SIGNAL).unwrap();
    let summary = value["summaries"]
        .as_object_mut()
        .unwrap()
        .remove("signal-pair")
        .unwrap();
    value["summaries"]["wire-only-renaming"] = summary;
    value["entrypoints"]["."]["exports"]["createSignal"] = serde_json::json!("wire-only-renaming");
    let actual = contract_document_v2::decode(&serde_json::to_vec_pretty(&value).unwrap())
        .unwrap()
        .normalize()
        .unwrap();
    assert_eq!(expected.semantic_digest(), actual.semantic_digest());
    assert_eq!(expected_id, actual.claim_id(&subject).unwrap());
}

#[test]
fn checked_in_schema_pins_both_document_discriminators() {
    let schema: serde_json::Value = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../schema/solid-reactivity-evidence-sidecars-v1.schema.json"
    )))
    .unwrap();
    assert_eq!(
        schema["$defs"]["proofDocument"]["properties"]["format"]["const"],
        PROOF_EVIDENCE_FORMAT
    );
    assert_eq!(
        schema["$defs"]["probeDocument"]["properties"]["format"]["const"],
        PROBE_EVIDENCE_FORMAT
    );
    assert_eq!(
        schema["$defs"]["proofDocument"]["properties"]["sidecarVersion"]["const"],
        EVIDENCE_SIDECAR_VERSION
    );
    assert_eq!(
        schema["$defs"]["probeDocument"]["properties"]["sidecarVersion"]["const"],
        EVIDENCE_SIDECAR_VERSION
    );
}

#[test]
fn sidecar_families_emit_deterministically_and_keep_material_separate() {
    let (catalog, first, proof_subject, probe_subject) = fixture();
    let mut proof = proof_material(proof_subject);
    proof.fact_transcripts.reverse();
    proof.coverage_limitations.reverse();
    let mut probe = probe_material(probe_subject);
    probe.environment.conditions.reverse();
    let second = emit_evidence_sidecars(
        &catalog,
        tool("solid-contract-evidence", 'd'),
        vec![proof],
        vec![probe],
    )
    .unwrap();
    assert_eq!(first, second);

    let proof: serde_json::Value = serde_json::from_slice(first.proof().unwrap()).unwrap();
    let probes: serde_json::Value = serde_json::from_slice(first.probes().unwrap()).unwrap();
    assert_eq!(proof["format"], PROOF_EVIDENCE_FORMAT);
    assert_eq!(probes["format"], PROBE_EVIDENCE_FORMAT);
    assert_eq!(proof["sidecarVersion"], EVIDENCE_SIDECAR_VERSION);
    assert_eq!(probes["sidecarVersion"], EVIDENCE_SIDECAR_VERSION);
    assert!(proof["claims"][0].get("environment").is_none());
    assert!(probes["claims"][0].get("factTranscripts").is_none());
    assert!(
        proof["claims"][0]["claimId"]
            .as_str()
            .unwrap()
            .starts_with("claim:v1:sha256:")
    );
}

#[test]
fn main_hashes_and_sidecar_contract_identity_bind_both_directions() {
    let (catalog, documents, _, _) = fixture();
    let main = main_with_references(documents.references());
    let validated =
        validate_evidence_sidecars(&main, &catalog, documents.proof(), documents.probes()).unwrap();
    assert_eq!(validated.proof_claims().len(), 1);
    assert_eq!(validated.probe_claims().len(), 1);

    // Ordinary normalization consumes only the main document's hash
    // references. Deleting both raw sidecars cannot change semantic meaning.
    let without_raw_sidecars = contract_document_v2::decode(&main)
        .unwrap()
        .normalize()
        .unwrap();
    assert_eq!(
        without_raw_sidecars.semantic_digest(),
        catalog.contract().semantic_digest()
    );
    assert!(matches!(
        validate_evidence_sidecars(&main, &catalog, None, None),
        Err(EvidenceSidecarError::MissingDocument { .. })
    ));
}

#[test]
fn stale_cross_package_cross_artifact_and_orphan_sidecars_are_rejected() {
    let (catalog, documents, _, _) = fixture();
    let main = main_with_references(documents.references());

    let mut stale = documents.proof().unwrap().to_vec();
    stale.push(b' ');
    assert!(matches!(
        validate_evidence_sidecars(&main, &catalog, Some(&stale), documents.probes()),
        Err(EvidenceSidecarError::ContentMismatch { kind: "proof" })
    ));

    assert!(matches!(
        validate_evidence_sidecars(SIGNAL, &catalog, documents.proof(), documents.probes()),
        Err(EvidenceSidecarError::OrphanDocument { .. })
    ));

    let mut stale_main: serde_json::Value = serde_json::from_slice(&main).unwrap();
    stale_main["package"]["version"] = serde_json::json!("2.0.0-rc.4");
    let stale_main = serde_json::to_vec(&stale_main).unwrap();
    assert!(matches!(
        validate_evidence_sidecars(&stale_main, &catalog, documents.proof(), documents.probes()),
        Err(EvidenceSidecarError::MainContractMismatch)
    ));

    let mut cross_package: serde_json::Value =
        serde_json::from_slice(documents.proof().unwrap()).unwrap();
    cross_package["contract"]["package"]["name"] = serde_json::json!("other-package");
    let cross_package = serde_json::to_vec(&cross_package).unwrap();
    let cross_package_main = replace_reference(&main, "proof", &cross_package);
    assert!(matches!(
        validate_evidence_sidecars(
            &cross_package_main,
            &catalog,
            Some(&cross_package),
            documents.probes()
        ),
        Err(EvidenceSidecarError::ContractBindingMismatch { kind: "proof" })
    ));

    let mut cross_artifact: serde_json::Value =
        serde_json::from_slice(documents.proof().unwrap()).unwrap();
    cross_artifact["claims"][0]["artifact"]["runtime"]["digest"] =
        serde_json::json!(digest('e').as_str());
    let cross_artifact = serde_json::to_vec(&cross_artifact).unwrap();
    let cross_artifact_main = replace_reference(&main, "proof", &cross_artifact);
    assert!(matches!(
        validate_evidence_sidecars(
            &cross_artifact_main,
            &catalog,
            Some(&cross_artifact),
            documents.probes()
        ),
        Err(EvidenceSidecarError::ArtifactMismatch)
    ));
}

#[test]
fn wrong_document_kind_version_claim_id_and_unplanned_claim_fail_closed() {
    let (catalog, documents, _, _) = fixture();
    let main = main_with_references(documents.references());

    for (field, value, expected) in [
        (
            "format",
            serde_json::json!(PROBE_EVIDENCE_FORMAT),
            "document-kind",
        ),
        ("sidecarVersion", serde_json::json!(2), "version"),
    ] {
        let mut sidecar: serde_json::Value =
            serde_json::from_slice(documents.proof().unwrap()).unwrap();
        sidecar[field] = value;
        let sidecar = serde_json::to_vec(&sidecar).unwrap();
        let changed_main = replace_reference(&main, "proof", &sidecar);
        let error =
            validate_evidence_sidecars(&changed_main, &catalog, Some(&sidecar), documents.probes())
                .unwrap_err();
        match expected {
            "document-kind" => assert!(matches!(error, EvidenceSidecarError::DocumentKind { .. })),
            "version" => assert!(matches!(error, EvidenceSidecarError::Version { .. })),
            _ => unreachable!(),
        }
    }

    let mut wrong_id: serde_json::Value =
        serde_json::from_slice(documents.proof().unwrap()).unwrap();
    wrong_id["claims"][0]["claimId"] =
        serde_json::json!(format!("claim:v1:{}", digest('f').as_str()));
    let wrong_id = serde_json::to_vec(&wrong_id).unwrap();
    let wrong_id_main = replace_reference(&main, "proof", &wrong_id);
    assert!(matches!(
        validate_evidence_sidecars(
            &wrong_id_main,
            &catalog,
            Some(&wrong_id),
            documents.probes()
        ),
        Err(EvidenceSidecarError::ClaimIdMismatch)
    ));

    let writes = SemanticClaimSubject {
        artifact_case: catalog.contract().artifact_cases()[0].id.clone(),
        export: "createSignal".into(),
        path: SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Writes)),
    };
    let mut orphan: serde_json::Value = serde_json::from_slice(documents.proof().unwrap()).unwrap();
    orphan["claims"][0]["subject"]["path"] =
        serde_json::json!({"kind": "call", "domain": "writes"});
    orphan["claims"][0]["claimId"] =
        serde_json::json!(catalog.contract().claim_id(&writes).unwrap().as_str());
    let orphan = serde_json::to_vec(&orphan).unwrap();
    let orphan_main = replace_reference(&main, "proof", &orphan);
    assert!(matches!(
        validate_evidence_sidecars(&orphan_main, &catalog, Some(&orphan), documents.probes()),
        Err(EvidenceSidecarError::OrphanClaim { kind: "proof" })
    ));

    let mut duplicate: serde_json::Value =
        serde_json::from_slice(documents.proof().unwrap()).unwrap();
    let repeated = duplicate["claims"][0].clone();
    duplicate["claims"].as_array_mut().unwrap().push(repeated);
    let duplicate = serde_json::to_vec(&duplicate).unwrap();
    let duplicate_main = replace_reference(&main, "proof", &duplicate);
    assert!(matches!(
        validate_evidence_sidecars(
            &duplicate_main,
            &catalog,
            Some(&duplicate),
            documents.probes()
        ),
        Err(EvidenceSidecarError::DuplicateClaim { .. })
    ));

    let mut empty: serde_json::Value = serde_json::from_slice(documents.proof().unwrap()).unwrap();
    empty["claims"][0]["factTranscripts"] = serde_json::json!([]);
    empty["claims"][0]["proofInputs"] = serde_json::json!([]);
    empty["claims"][0]["coverageLimitations"] = serde_json::json!([]);
    let empty = serde_json::to_vec(&empty).unwrap();
    let empty_main = replace_reference(&main, "proof", &empty);
    assert!(matches!(
        validate_evidence_sidecars(&empty_main, &catalog, Some(&empty), documents.probes()),
        Err(EvidenceSidecarError::InvalidMaterial { .. })
    ));
}

#[test]
fn probe_outcomes_record_limits_and_never_create_acceptance_authority() {
    let contract = fixture_contract();
    let (_, probe_subject) = fixture_subjects(&contract);
    let catalog = EvidenceCatalog::new(contract, [], [probe_subject.clone()]).unwrap();
    for outcome in [
        ProbeOutcome::Planned,
        ProbeOutcome::Witness {
            transcript: digest('1'),
        },
        ProbeOutcome::Falsification {
            transcript: digest('2'),
        },
        ProbeOutcome::Error {
            details: digest('3'),
        },
        ProbeOutcome::Timeout { limit_millis: 500 },
        ProbeOutcome::Refused {
            reason: "package execution is disabled".into(),
        },
    ] {
        let mut material = probe_material(probe_subject.clone());
        material.outcome = outcome;
        let documents = emit_evidence_sidecars(
            &catalog,
            tool("solid-contract-evidence", 'd'),
            vec![],
            vec![material],
        )
        .unwrap();
        let text = std::str::from_utf8(documents.probes().unwrap()).unwrap();
        assert!(!text.contains("accepted"));
        assert!(!text.contains("receipt"));
        if text.contains("\"kind\": \"timeout\"") {
            assert!(text.contains("\"limitMillis\": 500"));
            assert!(!text.contains("limit_millis"));
        }
        assert!(documents.proof().is_none());
    }
}
