use super::*;
use solid_reactive_ir::contract_semantics::{
    ArtifactCase, ArtifactIdentity, CallClaims, CallSemantics, ContractProposal, ExportIdentity,
    ExportSemantics, ExportTargetIdentity, GuardPartition, KnowledgeSet, PackageIdentity,
    ResolutionStep, StabilityKnowledge, ValueShape,
};
use std::collections::BTreeMap;

#[test]
fn proof_family_names_cover_the_complete_policy() {
    let names = [
        "package-identity",
        "manifest-entrypoint",
        "export-resolution",
        "artifact-declarations",
        "export-identity",
        "module-closure",
        "selected-signature",
        "argument-binding",
        "rest-spread-coverage",
        "callable-path",
        "operation-reachability",
        "operation-cardinality",
        "recursive-value-shape",
        "guard-partition",
        "compiler-reconciliation",
        "accepted-dependency-composition",
        "domain-exhaustiveness",
        "probe-consistency",
    ];
    assert_eq!(names.len(), CLOSURE_PROOF_FAMILIES.len());
    for (name, expected) in names.into_iter().zip(CLOSURE_PROOF_FAMILIES) {
        assert_eq!(parse_family(name).unwrap(), expected);
    }
}

fn family_name(family: ProofFamily) -> &'static str {
    match family {
        ProofFamily::PackageIdentity => "package-identity",
        ProofFamily::ManifestEntrypoint => "manifest-entrypoint",
        ProofFamily::ExportResolution => "export-resolution",
        ProofFamily::ArtifactDeclarations => "artifact-declarations",
        ProofFamily::ExportIdentity => "export-identity",
        ProofFamily::ModuleClosure => "module-closure",
        ProofFamily::SelectedSignature => "selected-signature",
        ProofFamily::ArgumentBinding => "argument-binding",
        ProofFamily::RestSpreadCoverage => "rest-spread-coverage",
        ProofFamily::CallablePath => "callable-path",
        ProofFamily::OperationReachability => "operation-reachability",
        ProofFamily::OperationCardinality => "operation-cardinality",
        ProofFamily::RecursiveValueShape => "recursive-value-shape",
        ProofFamily::GuardPartition => "guard-partition",
        ProofFamily::CompilerReconciliation => "compiler-reconciliation",
        ProofFamily::AcceptedDependencyComposition => "accepted-dependency-composition",
        ProofFamily::DomainExhaustiveness => "domain-exhaustiveness",
        ProofFamily::ProbeConsistency => "probe-consistency",
    }
}

#[test]
fn proof_transcript_is_the_only_workflow_path_to_receipt_bytes() {
    let digest =
        |byte: char| Digest::parse(format!("sha256:{}", byte.to_string().repeat(64))).unwrap();
    let runtime = ArtifactIdentity {
        path: "dist/index.js".into(),
        digest: digest('1'),
    };
    let declarations = ArtifactIdentity {
        path: "dist/index.d.ts".into(),
        digest: digest('2'),
    };
    let mut artifact = ArtifactCase {
        id: "case".into(),
        entrypoint: ".".into(),
        resolution_trace: vec![
            ResolutionStep {
                condition: "runtime".into(),
                target: "/exports/./import".into(),
            },
            ResolutionStep {
                condition: "types".into(),
                target: "/exports/./types".into(),
            },
        ],
        runtime: runtime.clone(),
        declarations: declarations.clone(),
        dependency_closure: digest('3'),
        transform: None,
        stability: StabilityKnowledge::Unknown,
        exports: BTreeMap::from([(
            "read".into(),
            ExportSemantics {
                identity: ExportIdentity {
                    entrypoint: ".".into(),
                    public_name: "read".into(),
                    runtime: ExportTargetIdentity {
                        module: runtime,
                        export_name: "read".into(),
                    },
                    declarations: ExportTargetIdentity {
                        module: declarations,
                        export_name: "read".into(),
                    },
                },
                shape: ValueShape::Callable,
                stability: StabilityKnowledge::Unknown,
                call: CallSemantics::new(
                    CallClaims {
                        callbacks: KnowledgeSet::Unknown,
                        reads: KnowledgeSet::Complete(Vec::new()),
                        writes: KnowledgeSet::Unknown,
                        creates: KnowledgeSet::Unknown,
                        invalidates: KnowledgeSet::Unknown,
                        throws: KnowledgeSet::Unknown,
                        returns: KnowledgeSet::Unknown,
                        cleanups: KnowledgeSet::Unknown,
                        disposals: KnowledgeSet::Unknown,
                    },
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    GuardPartition::default(),
                ),
            },
        )]),
    };
    let package = PackageIdentity {
        name: "package".into(),
        version: "1.0.0".into(),
        integrity: "sha512:test".into(),
        manifest: ArtifactIdentity {
            path: "package.json".into(),
            digest: digest('0'),
        },
    };
    let mut candidates = Vec::new();
    for (export_name, export) in &mut artifact.exports {
        candidates.extend(export.open_proposed_closure().into_iter().map(|path| {
            SemanticClaimSubject {
                artifact_case: artifact.id.clone(),
                export: export_name.clone(),
                path: SemanticClaimPath::Domain(path),
            }
        }));
    }
    let proposal = ContractProposal::new(package, vec![artifact])
        .normalize()
        .unwrap();
    let artifacts = encode_proposal_artifacts(&proposal, candidates, false).unwrap();
    let document = artifacts.document;
    let plan_bytes = artifacts.plan;
    let proposal = contract_document_v2::decode(&document)
        .unwrap()
        .normalize()
        .unwrap();
    let plan = decode_plan(&plan_bytes).unwrap();
    let selected = proposal.artifact_cases()[0].id.clone();
    let claim = plan.closure_candidates.into_iter().next().unwrap();
    let proof = ProofDocument {
        format: PROOF_FORMAT.into(),
        proof_version: PROOF_VERSION,
        semantic_model_version: proposal.semantic_model_version(),
        semantic_digest: proposal.semantic_digest().as_str().into(),
        verifier_build: "contract-workflow-test".into(),
        claims: vec![ProofClaim {
            claim_id: claim.claim_id,
            subject: claim.subject,
            families: CLOSURE_PROOF_FAMILIES
                .into_iter()
                .map(|family| ProofFamilyInput {
                    family: family_name(family).into(),
                    transcript: format!("independent {family:?} replay"),
                    enumerated: Vec::new(),
                    classified: Vec::new(),
                    unresolved: Vec::new(),
                    complete: true,
                })
                .collect(),
        }],
        probe_contradictions: Vec::new(),
        probe_sidecar: None,
    };
    let proof_bytes = emit(&proof).unwrap();
    let accepted = verify(&document, &plan_bytes, &proof_bytes, &selected, false).unwrap();
    contract_document_v2::decode(&accepted.document)
        .unwrap()
        .normalize()
        .unwrap();
    let receipt: serde_json::Value = serde_json::from_slice(&accepted.receipt).unwrap();
    assert_eq!(receipt["semanticModelVersion"], 1);
}
