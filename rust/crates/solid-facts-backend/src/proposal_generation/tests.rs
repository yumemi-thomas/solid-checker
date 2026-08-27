use super::*;
use solid_reactive_ir::contract_semantics::{
    ArtifactIdentity, CallClaims, CallSemantics, CapabilityKnowledge, Cardinality,
    CardinalityScope, ClaimDomain, ExportIdentity, ExportSemantics, ExportTargetIdentity,
    GuardPartition, KnowledgeSet, KnowledgeState, Lifetime, ObjectProperty, Operation, OperationId,
    OwnerCapabilities, OwnerRelation, OwnerRequirements, OwnerSource, Requirement, ResolutionStep,
    Schedule, StabilityKnowledge, Tracking, Trigger, UpperBound, ValueShape,
};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ClosureManifest, ResolutionAuthority, ResolutionTrace, ResolvedExportBinding,
    ResolvedExportTarget, ResolvedFile, ResolvedImport,
};

fn digest(byte: char) -> Digest {
    Digest::parse(format!("sha256:{}", byte.to_string().repeat(64))).unwrap()
}

fn artifact(path: &str, byte: char) -> ArtifactIdentity {
    ArtifactIdentity {
        path: path.into(),
        digest: digest(byte),
    }
}

fn package() -> PackageIdentity {
    PackageIdentity {
        name: "example".into(),
        version: "1.0.0".into(),
        integrity: "sha512:exact".into(),
        manifest: artifact("package.json", 'a'),
    }
}

fn operation(id: &str, min: u32) -> Operation {
    Operation {
        id: OperationId(id.into()),
        kind: OperationKind::Read,
        guard: None,
        trigger: Some(Trigger::Event(
            solid_reactive_ir::contract_semantics::Event::Call,
        )),
        at: Some(solid_reactive_ir::contract_semantics::Event::Call),
        schedule: Some(Schedule::SameStack),
        tracking: Tracking::Untracked,
        owner: OwnerRelation {
            source: OwnerSource::None,
            requirements: OwnerRequirements {
                owner: Requirement::Forbidden,
                child_owners: Requirement::Unconstrained,
                cleanup: Requirement::Unconstrained,
            },
            capabilities: OwnerCapabilities {
                child_owners: CapabilityKnowledge::Forbidden,
                cleanup: CapabilityKnowledge::Forbidden,
            },
            lifetime: Some(Lifetime::Call),
            productions: KnowledgeSet::Complete(vec![]),
        },
        cardinality: Cardinality {
            scope: Some(CardinalityScope::Call),
            min: Some(min),
            max: Some(UpperBound::Finite(1)),
        },
        inputs: vec![],
        output: None,
        resources: BTreeSet::new(),
    }
}

fn claims(
    read: &OperationId,
    callbacks: KnowledgeSet<solid_reactive_ir::contract_semantics::CallbackInvocation>,
) -> CallClaims {
    CallClaims {
        callbacks,
        reads: KnowledgeSet::Complete(vec![read.clone()]),
        writes: KnowledgeSet::Complete(vec![]),
        creates: KnowledgeSet::Complete(vec![]),
        invalidates: KnowledgeSet::Complete(vec![]),
        throws: KnowledgeSet::Complete(vec![]),
        returns: KnowledgeSet::Complete(vec![]),
        cleanups: KnowledgeSet::Complete(vec![]),
        disposals: KnowledgeSet::Complete(vec![]),
    }
}

fn export(
    case: &ArtifactCase,
    name: &str,
    callbacks: KnowledgeSet<solid_reactive_ir::contract_semantics::CallbackInvocation>,
    min: u32,
) -> ExportSemantics {
    let read = operation(&format!("{name}-read"), min);
    ExportSemantics {
        identity: ExportIdentity {
            entrypoint: case.entrypoint.clone(),
            public_name: name.into(),
            runtime: ExportTargetIdentity {
                module: case.runtime.clone(),
                export_name: name.into(),
            },
            declarations: ExportTargetIdentity {
                module: case.declarations.clone(),
                export_name: name.into(),
            },
        },
        shape: ValueShape::Object(KnowledgeSet::Complete(vec![
            ObjectProperty {
                name: "known".into(),
                value: ValueShape::Plain,
            },
            ObjectProperty {
                name: "open".into(),
                value: ValueShape::Unknown,
            },
        ])),
        stability: StabilityKnowledge::Unknown,
        call: CallSemantics::new(
            claims(&read.id, callbacks),
            vec![read],
            vec![],
            vec![],
            GuardPartition {
                cases: KnowledgeSet::Complete(vec![]),
            },
        ),
    }
}

fn analysis() -> ProposalAnalysis {
    let closure = ClosureManifest::new(vec![], vec![], vec![]).unwrap();
    let mut case = ArtifactCase {
        id: "import-case".into(),
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
        runtime: artifact("dist/index.js", 'b'),
        declarations: artifact("types/index.d.ts", 'c'),
        dependency_closure: Digest::parse(closure.digest.clone()).unwrap(),
        transform: None,
        stability: StabilityKnowledge::Unknown,
        exports: BTreeMap::new(),
    };
    case.exports.insert(
        "partial".into(),
        export(&case, "partial", KnowledgeSet::Unknown, 0),
    );
    case.exports.insert(
        "sibling".into(),
        export(&case, "sibling", KnowledgeSet::Complete(vec![]), 1),
    );
    let root = "/project/node_modules/example";
    let runtime = ResolvedFile {
        path: format!("{root}/dist/index.js"),
        real_path: None,
        digest: format!("sha256:{}", "b".repeat(64)),
    };
    let declarations = ResolvedFile {
        path: format!("{root}/types/index.d.ts"),
        real_path: None,
        digest: format!("sha256:{}", "c".repeat(64)),
    };
    let exports = case
        .exports
        .keys()
        .map(|name| {
            (
                name.clone(),
                ResolvedExportBinding {
                    runtime: ResolvedExportTarget {
                        module: runtime.clone(),
                        export_name: name.clone(),
                    },
                    declarations: ResolvedExportTarget {
                        module: declarations.clone(),
                        export_name: name.clone(),
                    },
                },
            )
        })
        .collect();
    let resolution = ResolvedImport {
        specifier: "example".into(),
        importer: "/project/src/app.ts".into(),
        requested_entrypoint: ".".into(),
        package_name: "example".into(),
        package_version: "1.0.0".into(),
        package_integrity: "sha512:exact".into(),
        package_root: root.into(),
        package_real_root: None,
        package_manifest: ResolvedFile {
            path: format!("{root}/package.json"),
            real_path: None,
            digest: format!("sha256:{}", "a".repeat(64)),
        },
        runtime,
        declarations,
        runtime_trace: ResolutionTrace {
            branch: "/exports/./import".into(),
            steps: vec![],
        },
        declaration_trace: ResolutionTrace {
            branch: "/exports/./types".into(),
            steps: vec![],
        },
        closure,
        transform: None,
        exports,
        authority: ResolutionAuthority::StandalonePackageResolver,
    };
    ProposalAnalysis {
        package: package(),
        artifact_cases: vec![case],
        resolutions: vec![resolution],
    }
}

fn planned() -> PlannedProposal {
    plan_probes(plan_proofs(construct_proposal(analysis()).unwrap()))
}

#[test]
fn construction_withdraws_false_closure_but_keeps_partial_positive_operations() {
    let proposal = construct_proposal(analysis()).unwrap();
    let case = proposal.contract().artifact_case("import-case").unwrap();
    let partial = &case.exports["partial"];
    assert_eq!(
        partial.claim_state(ClaimDomain::Reads),
        KnowledgeState::PartialPositive
    );
    assert_eq!(
        partial.claim_state(ClaimDomain::Writes),
        KnowledgeState::Unknown
    );
    assert_eq!(
        partial.claim_state(ClaimDomain::Callbacks),
        KnowledgeState::Unknown
    );
    assert!(partial.operation("partial-read").is_some());
    assert!(proposal.closure_candidates().iter().any(|candidate| {
        candidate.export == "sibling" && candidate.claim == ClaimPath::Call(ClaimDomain::Callbacks)
    }));
    assert!(proposal.closure_candidates().iter().any(|candidate| {
        candidate.export == "partial" && candidate.claim == ClaimPath::Call(ClaimDomain::Writes)
    }));
}

#[test]
fn local_unknowns_do_not_erase_unrelated_closure_candidates_or_known_siblings() {
    let proposal = construct_proposal(analysis()).unwrap();
    let sibling = &proposal
        .contract()
        .artifact_case("import-case")
        .unwrap()
        .exports["sibling"];
    assert_eq!(
        sibling.claim_state(ClaimDomain::Callbacks),
        KnowledgeState::Unknown
    );
    assert!(proposal.closure_candidates().iter().any(|candidate| {
        candidate.export == "sibling" && candidate.claim == ClaimPath::Call(ClaimDomain::Reads)
    }));
    let unresolved = sibling.unresolved_claims();
    assert!(unresolved.iter().any(|claim| matches!(
        claim,
        ClaimPath::Value { path, .. }
            if path.0 == vec![solid_reactive_ir::contract_semantics::ValuePathSegment::ObjectProperty("open".into())]
    )));
    assert!(!unresolved.iter().any(|claim| matches!(
        claim,
        ClaimPath::Value { path, .. }
            if path.0 == vec![solid_reactive_ir::contract_semantics::ValuePathSegment::ObjectProperty("known".into())]
    )));
}

#[test]
fn proof_and_probe_plans_keep_authorities_separate() {
    let planned = planned();
    assert!(
        planned
            .plan()
            .proof_obligations
            .iter()
            .any(|obligation| { obligation.kind == ProofObligationKind::ProveClosure })
    );
    assert!(
        planned
            .plan()
            .proof_obligations
            .iter()
            .any(|obligation| { obligation.kind == ProofObligationKind::ResolveOpenClaim })
    );
    assert!(
        planned
            .plan()
            .probe_candidates
            .iter()
            .any(|candidate| { candidate.operation.operation == "partial-read" })
    );
    assert!(
        !planned
            .plan()
            .probe_candidates
            .iter()
            .any(|candidate| { candidate.operation.operation == "sibling-read" })
    );
}

#[test]
fn proposal_emission_is_deterministic_and_has_no_accepted_closure_field() {
    let first = emit_proposal(&planned()).unwrap();
    let second = emit_proposal(&planned()).unwrap();
    assert_eq!(first, second);
    let value: serde_json::Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(value["acceptance"], "unaccepted");
    assert!(
        !first
            .windows(b"\"closed\"".len())
            .any(|window| window == b"\"closed\"")
    );
    assert!(
        value["positiveOperations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|operation| {
                operation["operation"] == "partial-read" && operation["strength"] == "Possible"
            })
    );
}

#[test]
fn proposal_plan_reaches_an_order_independent_idempotent_fixed_point() {
    let plan = planned().plan().clone();
    let forward = ProposalPlan::fixed_point([plan.clone(), plan.clone()]).unwrap();
    let reverse = ProposalPlan::fixed_point([plan.clone(), forward.clone()]).unwrap();
    assert_eq!(forward, reverse);
    assert_eq!(forward, plan);
    assert!(matches!(
        ProposalPlan::fixed_point(std::iter::empty()),
        Err(ProposalGenerationError::NoRounds)
    ));

    let mut other = plan;
    other.semantic_digest = digest('9');
    assert!(matches!(
        ProposalPlan::fixed_point([forward, other]),
        Err(ProposalGenerationError::MixedSemanticDigests)
    ));
}

#[test]
fn proposal_construction_refuses_missing_and_duplicate_artifact_selection() {
    let mut missing = analysis();
    missing.resolutions.clear();
    assert!(matches!(
        construct_proposal(missing),
        Err(ProposalGenerationError::MissingArtifactResolutions)
    ));

    let mut duplicate = analysis();
    duplicate.resolutions.push(duplicate.resolutions[0].clone());
    assert!(matches!(
        construct_proposal(duplicate),
        Err(ProposalGenerationError::DuplicateArtifactResolution { .. })
    ));
}
