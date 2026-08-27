use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::contract_semantics::*;

fn digest(byte: char) -> Digest {
    Digest::parse(format!("sha256:{}", byte.to_string().repeat(64))).unwrap()
}

fn artifact(path: &str, byte: char) -> ArtifactIdentity {
    ArtifactIdentity {
        path: path.into(),
        digest: digest(byte),
    }
}

fn closed_claims() -> CallClaims {
    CallClaims {
        callbacks: KnowledgeSet::complete(vec![]),
        reads: KnowledgeSet::complete(vec![]),
        writes: KnowledgeSet::complete(vec![]),
        creates: KnowledgeSet::complete(vec![]),
        invalidates: KnowledgeSet::complete(vec![]),
        throws: KnowledgeSet::complete(vec![]),
        returns: KnowledgeSet::complete(vec![]),
        cleanups: KnowledgeSet::complete(vec![]),
        disposals: KnowledgeSet::complete(vec![]),
    }
}

fn closed_contract(runtime_digest: char) -> NormalizedContract {
    let runtime = artifact("dist/solid.js", runtime_digest);
    let declarations = artifact("types/index.d.ts", 'c');
    let mut case = ArtifactCase {
        id: "browser-import".into(),
        entrypoint: ".".into(),
        resolution_trace: vec![ResolutionStep {
            condition: "import".into(),
            target: "./dist/solid.js".into(),
        }],
        runtime: runtime.clone(),
        declarations: declarations.clone(),
        dependency_closure: digest('d'),
        transform: None,
        stability: StabilityKnowledge::Unknown,
        exports: BTreeMap::new(),
    };
    case.exports.insert(
        "version".into(),
        ExportSemantics {
            identity: ExportIdentity {
                entrypoint: ".".into(),
                public_name: "version".into(),
                runtime: ExportTargetIdentity {
                    module: runtime,
                    export_name: "version".into(),
                },
                declarations: ExportTargetIdentity {
                    module: declarations,
                    export_name: "version".into(),
                },
            },
            shape: ValueShape::Object(KnowledgeSet::complete(vec![ObjectProperty {
                name: "nested".into(),
                value: ValueShape::Tuple(KnowledgeSet::complete(vec![ValueShape::Plain])),
            }])),
            stability: StabilityKnowledge::Unknown,
            call: CallSemantics::new(
                closed_claims(),
                vec![],
                vec![],
                vec![],
                GuardPartition {
                    cases: KnowledgeSet::complete(vec![]),
                },
            ),
        },
    );
    ContractProposal::new(
        PackageIdentity {
            name: "solid-js".into(),
            version: "2.0.0-rc.3".into(),
            integrity: "sha512-authoritative".into(),
            manifest: artifact("package.json", 'a'),
        },
        vec![case],
    )
    .normalize()
    .unwrap()
}

fn open_contract(runtime_digest: char) -> NormalizedContract {
    let normalized = closed_contract(runtime_digest);
    let package = normalized.package().clone();
    let mut cases = normalized.artifact_cases().to_vec();
    for case in &mut cases {
        for export in case.exports.values_mut() {
            export.open_proposed_closure();
        }
    }
    ContractProposal::new(package, cases).normalize().unwrap()
}

fn reads_subject() -> SemanticClaimSubject {
    SemanticClaimSubject {
        artifact_case: "browser-import".into(),
        export: "version".into(),
        path: SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Reads)),
    }
}

fn nested_tuple_subject() -> SemanticClaimSubject {
    SemanticClaimSubject {
        artifact_case: "browser-import".into(),
        export: "version".into(),
        path: SemanticClaimPath::Domain(ClaimPath::Value {
            root: ValueRoot::Export,
            path: ValuePath(vec![ValuePathSegment::ObjectProperty("nested".into())]),
            domain: ValueClaimDomain::TupleItems,
        }),
    }
}

fn valid_proofs(
    contract: &NormalizedContract,
    subject: &SemanticClaimSubject,
) -> Vec<ReplayedProof> {
    CLOSURE_PROOF_FAMILIES
        .into_iter()
        .enumerate()
        .map(|(index, family)| {
            replay_proof_rule(
                contract,
                family,
                subject.clone(),
                ProofRuleInput {
                    authority: family_authority(family),
                    transcript: format!("proof transcript {}", index + 1).into_bytes(),
                    observed_scope: proof_scope_digest(contract, family, subject).unwrap(),
                    enumerated: vec![],
                    classified: vec![],
                    unresolved: vec![],
                    completeness: CensusCompleteness::Complete,
                },
            )
            .unwrap()
        })
        .collect()
}

fn request(
    contract: NormalizedContract,
    subject: SemanticClaimSubject,
    proofs: Vec<ReplayedProof>,
) -> AcceptanceRequest {
    AcceptanceRequest {
        contract,
        selected_artifact_case: "browser-import".into(),
        wire_bytes: b"temporary main contract".to_vec(),
        closed_claims: vec![subject],
        proofs,
        contradictions: vec![],
        verifier: VerifierIdentity {
            build: "phase-11-test".into(),
            policy: PROOF_POLICY_VERSION,
        },
    }
}

#[test]
fn every_proof_family_is_required_against_false_closure() {
    for missing in CLOSURE_PROOF_FAMILIES {
        let contract = open_contract('b');
        let subject = reads_subject();
        let proofs = valid_proofs(&contract, &subject)
            .into_iter()
            .filter(|proof| proof.family() != missing)
            .collect();
        assert!(matches!(
            verify_and_accept(request(contract, subject, proofs)),
            Err(ProofError::MissingProof { family, .. }) if family == missing
        ));
    }
}

#[test]
fn rule_replay_refuses_incomplete_unresolved_and_mismatched_censuses() {
    let contract = open_contract('b');
    let subject = reads_subject();
    let scope = proof_scope_digest(&contract, ProofFamily::DomainExhaustiveness, &subject).unwrap();
    let base = ProofRuleInput {
        authority: family_authority(ProofFamily::DomainExhaustiveness),
        transcript: b"complete Type Facts census".to_vec(),
        observed_scope: scope,
        enumerated: vec![digest('2')],
        classified: vec![digest('2')],
        unresolved: vec![],
        completeness: CensusCompleteness::Complete,
    };

    let mut incomplete = base.clone();
    incomplete.completeness = CensusCompleteness::Incomplete;
    assert!(matches!(
        replay_proof_rule(
            &contract,
            ProofFamily::DomainExhaustiveness,
            subject.clone(),
            incomplete
        ),
        Err(ProofError::IncompleteCensus { .. })
    ));

    let mut unresolved = base.clone();
    unresolved.unresolved.push(digest('3'));
    assert!(matches!(
        replay_proof_rule(
            &contract,
            ProofFamily::DomainExhaustiveness,
            subject.clone(),
            unresolved
        ),
        Err(ProofError::UnresolvedPremises { .. })
    ));

    let mut omitted = base;
    omitted.classified.clear();
    assert!(matches!(
        replay_proof_rule(
            &contract,
            ProofFamily::DomainExhaustiveness,
            subject,
            omitted
        ),
        Err(ProofError::CensusMismatch { .. })
    ));

    let wrong_authority = ProofRuleInput {
        authority: ProofAuthority::RuntimeProbe,
        transcript: b"not a Type Facts transcript".to_vec(),
        observed_scope: proof_scope_digest(
            &contract,
            ProofFamily::DomainExhaustiveness,
            &reads_subject(),
        )
        .unwrap(),
        enumerated: vec![],
        classified: vec![],
        unresolved: vec![],
        completeness: CensusCompleteness::Complete,
    };
    assert!(matches!(
        replay_proof_rule(
            &contract,
            ProofFamily::DomainExhaustiveness,
            reads_subject(),
            wrong_authority
        ),
        Err(ProofError::WrongAuthority { .. })
    ));
}

#[test]
fn changed_artifact_or_semantics_cannot_reuse_proof_replay() {
    let original = open_contract('b');
    let subject = reads_subject();
    let proofs = valid_proofs(&original, &subject);
    let mutated = open_contract('e');
    assert!(matches!(
        verify_and_accept(request(mutated, subject, proofs)),
        Err(ProofError::StaleReplay)
    ));
}

#[test]
fn probe_contradictions_block_the_exact_closed_claim() {
    let contract = open_contract('b');
    let subject = reads_subject();
    let claim = contract.claim_id(&subject).unwrap();
    let proofs = valid_proofs(&contract, &subject);
    let mut request = request(contract, subject, proofs);
    request.contradictions.push(ProofContradiction {
        claim,
        transcript: digest('9'),
    });
    assert!(matches!(
        verify_and_accept(request),
        Err(ProofError::ProbeContradiction { .. })
    ));
}

#[test]
fn verified_recursive_closure_stays_local_to_the_exact_leaf() {
    let contract = open_contract('b');
    let subject = nested_tuple_subject();
    let proofs = valid_proofs(&contract, &subject);
    let accepted = verify_and_accept(request(contract, subject, proofs)).unwrap();
    let export = accepted.export("version").unwrap();
    let ValueShape::Object(properties) = &export.shape else {
        panic!("expected object");
    };
    assert_eq!(properties.state(), KnowledgeState::PartialPositive);
    let ValueShape::Tuple(items) = &properties.items()[0].value else {
        panic!("expected nested tuple");
    };
    assert_eq!(items.state(), KnowledgeState::CompletePositive);
    assert_eq!(
        export.claim_state(ClaimDomain::Reads),
        KnowledgeState::Unknown
    );
}

#[test]
fn proof_and_closed_claim_roots_are_deterministic() {
    let contract = open_contract('b');
    let subject = reads_subject();
    let proofs = valid_proofs(&contract, &subject);
    let first =
        verify_and_accept(request(contract.clone(), subject.clone(), proofs.clone())).unwrap();
    let mut reversed = proofs;
    reversed.reverse();
    let second = verify_and_accept(request(contract, subject, reversed)).unwrap();
    assert_eq!(first.receipt().proof_root, second.receipt().proof_root);
    assert_eq!(
        first.receipt().closed_claims_root,
        second.receipt().closed_claims_root
    );
    assert_eq!(
        first.receipt().semantic_digest,
        second.receipt().semantic_digest
    );
}

#[test]
fn policy_downgrades_are_rejected_before_acceptance() {
    let contract = open_contract('b');
    let subject = reads_subject();
    let proofs = valid_proofs(&contract, &subject);
    let mut request = request(contract, subject, proofs);
    request.verifier.policy = 0;
    assert_eq!(
        verify_and_accept(request),
        Err(ProofError::PolicyDowngrade {
            required: PROOF_POLICY_VERSION,
            actual: 0,
        })
    );
}

#[test]
fn stored_receipt_replay_recomputes_every_available_binding() {
    let contract = open_contract('b');
    let subject = reads_subject();
    let proofs = valid_proofs(&contract, &subject);
    let issued = verify_and_accept(request(contract, subject, proofs)).unwrap();
    let finalized = ContractProposal::new(
        issued.package().clone(),
        vec![issued.artifact_case().clone()],
    )
    .normalize()
    .unwrap();

    let loaded = validate_receipt_and_accept(
        finalized.clone(),
        &issued.artifact_case().id,
        issued.receipt().clone(),
    )
    .unwrap();
    assert_eq!(loaded, issued);

    for field in [
        "semanticDigest",
        "artifactsDigest",
        "closureDigest",
        "closedClaimsRoot",
    ] {
        let mut receipt = issued.receipt().clone();
        match field {
            "semanticDigest" => receipt.semantic_digest = digest('7'),
            "artifactsDigest" => receipt.artifacts_digest = digest('7'),
            "closureDigest" => receipt.closure_digest = digest('7'),
            "closedClaimsRoot" => receipt.closed_claims_root = digest('7'),
            _ => unreachable!(),
        }
        assert_eq!(
            validate_receipt_and_accept(finalized.clone(), "browser-import", receipt),
            Err(ReceiptValidationError::Mismatch { field })
        );
    }
}

#[test]
fn stored_receipt_replay_refuses_policy_drift() {
    let contract = open_contract('b');
    let subject = reads_subject();
    let proofs = valid_proofs(&contract, &subject);
    let issued = verify_and_accept(request(contract, subject, proofs)).unwrap();
    let finalized = ContractProposal::new(
        issued.package().clone(),
        vec![issued.artifact_case().clone()],
    )
    .normalize()
    .unwrap();
    let mut receipt = issued.receipt().clone();
    receipt.verifier.policy += 1;
    assert_eq!(
        validate_receipt_and_accept(finalized, "browser-import", receipt),
        Err(ReceiptValidationError::ProofPolicy {
            expected: PROOF_POLICY_VERSION,
            actual: PROOF_POLICY_VERSION + 1,
        })
    );
}

#[test]
fn stored_receipt_cannot_invent_an_acceptance_with_no_closed_claim() {
    let contract = open_contract('b');
    let selected = contract.artifact_cases()[0].clone();
    let receipt = AcceptanceReceipt {
        receipt_version: ACCEPTANCE_RECEIPT_VERSION,
        wire_digest: digest('1'),
        semantic_model_version: contract.semantic_model_version(),
        semantic_digest: contract.semantic_digest().clone(),
        artifacts_digest: artifacts_digest(contract.package(), &selected),
        closure_digest: selected.dependency_closure.clone(),
        proof_root: digest('2'),
        closed_claims_root: closed_claims_root(std::iter::empty()),
        verifier: VerifierIdentity {
            build: "phase-12-test".into(),
            policy: PROOF_POLICY_VERSION,
        },
    };
    assert_eq!(
        validate_receipt_and_accept(contract, &selected.id, receipt),
        Err(ReceiptValidationError::NoClosedClaims)
    );
}

#[test]
fn complete_census_order_and_duplicates_normalize_equivalently() {
    let contract = open_contract('b');
    let subject = reads_subject();
    let family = ProofFamily::SelectedSignature;
    let scope = proof_scope_digest(&contract, family, &subject).unwrap();
    let input = |values: Vec<Digest>| ProofRuleInput {
        authority: family_authority(family),
        transcript: b"selected signature transcript".to_vec(),
        observed_scope: scope.clone(),
        enumerated: values.clone(),
        classified: values,
        unresolved: vec![],
        completeness: CensusCompleteness::Complete,
    };
    let first = replay_proof_rule(
        &contract,
        family,
        subject.clone(),
        input(vec![digest('2'), digest('1'), digest('2')]),
    )
    .unwrap();
    let second = replay_proof_rule(
        &contract,
        family,
        subject,
        input(vec![digest('1'), digest('2')]),
    )
    .unwrap();
    assert_eq!(first, second);
}

#[test]
fn operation_claims_cannot_be_smuggled_in_as_closed_domains() {
    let contract = open_contract('b');
    let subject = SemanticClaimSubject {
        artifact_case: "browser-import".into(),
        export: "version".into(),
        path: SemanticClaimPath::Operation(OperationId("missing".into())),
    };
    let result = verify_and_accept(request(contract, subject, vec![]));
    assert!(matches!(
        result,
        Err(ProofError::OperationIsNotClosure | ProofError::Claim(_))
    ));
}

#[test]
fn all_family_names_are_unique_and_stable() {
    assert_eq!(
        CLOSURE_PROOF_FAMILIES
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        CLOSURE_PROOF_FAMILIES.len()
    );
}
