use std::collections::BTreeMap;

use solid_reactive_ir::{
    ContractClaim, ContractEntrypoint, ContractExport, ContractPackage, ContractReactiveRead,
    PackageContract,
    contract_semantics::{ClaimDomain, KnowledgeSet, KnowledgeState, OperationKind},
};

use super::*;
use crate::artifact_resolution::{
    ClosureManifest, ResolutionAuthority, ResolutionTrace, ResolvedExportBinding,
    ResolvedExportTarget, ResolvedFile,
};

fn sha(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn resolution(exports: impl IntoIterator<Item = String>) -> ResolvedImport {
    resolution_for_package("package", exports)
}

fn resolution_for_package(
    package_name: &str,
    exports: impl IntoIterator<Item = String>,
) -> ResolvedImport {
    ResolvedImport {
        specifier: package_name.into(),
        package_name: package_name.into(),
        ..resolution_at_default_paths(exports)
    }
}

fn resolution_at_default_paths(exports: impl IntoIterator<Item = String>) -> ResolvedImport {
    let manifest = ResolvedFile {
        path: "/project/node_modules/package/package.json".into(),
        real_path: None,
        digest: sha('a'),
    };
    let runtime = ResolvedFile {
        path: "/project/node_modules/package/dist/index.js".into(),
        real_path: None,
        digest: sha('b'),
    };
    let declarations = ResolvedFile {
        path: "/project/node_modules/package/dist/index.d.ts".into(),
        real_path: None,
        digest: sha('c'),
    };
    ResolvedImport {
        specifier: "package".into(),
        importer: "/project/src/app.ts".into(),
        requested_entrypoint: ".".into(),
        package_name: "package".into(),
        package_version: "1.0.0".into(),
        package_integrity: "sha512:test".into(),
        package_root: "/project/node_modules/package".into(),
        package_real_root: None,
        package_manifest: manifest,
        runtime: runtime.clone(),
        declarations: declarations.clone(),
        runtime_trace: ResolutionTrace::default(),
        declaration_trace: ResolutionTrace::default(),
        closure: ClosureManifest::new(Vec::new(), Vec::new(), Vec::new()).unwrap(),
        transform: None,
        exports: exports
            .into_iter()
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
                            export_name: name,
                        },
                    },
                )
            })
            .collect(),
        declaration_exports: std::collections::BTreeSet::new(),
        authority: ResolutionAuthority::Host,
    }
}

fn inferred(summary: ContractExport) -> PackageContract {
    PackageContract {
        package: ContractPackage {
            name: "package".into(),
            version: "1.0.0".into(),
            integrity: "sha512:test".into(),
        },
        entrypoints: BTreeMap::from([(
            ".".into(),
            ContractEntrypoint {
                exports: BTreeMap::from([("read".into(), summary)]),
            },
        )]),
        source_path: String::new(),
    }
}

#[test]
fn unknown_runtime_kind_normalizes_without_negative_claims() {
    let normalized = normalize_inferred_contract_with_candidates(
        &inferred(ContractExport::unknown_runtime_kind()),
        &resolution(["read".into()]),
    )
    .unwrap();
    let export = &normalized.contract.artifact_cases()[0].exports["read"];
    assert_eq!(export.shape, ValueShape::Unknown);
    for domain in ClaimDomain::ALL {
        assert_eq!(export.claim_state(domain), KnowledgeState::Unknown);
    }
    assert!(normalized.closure_candidates.is_empty());
}

#[test]
fn inferred_normalization_keeps_unknowns_local_and_emits_only_open_proposals() {
    let summary = ContractExport {
        kind: "function".into(),
        reactive_reads: ContractClaim::Known(vec![ContractReactiveRead {
            kind: "parameter".into(),
            label: String::new(),
            parameter: Some(0),
            path: None,
            composed_owner: None,
            composed_from: None,
        }]),
        returns: ContractClaim::Open,
        ..ContractExport::default()
    };
    let normalized = normalize_inferred_contract_with_candidates(
        &inferred(summary),
        &resolution(["read".into()]),
    )
    .unwrap();
    let candidates = normalized.closure_candidates;
    let export = normalized.contract.artifact_cases()[0]
        .exports
        .get("read")
        .unwrap();

    assert!(matches!(
        export.call.claims().returns,
        KnowledgeSet::Unknown
    ));
    assert_eq!(export.call.operations.len(), 1);
    assert_eq!(export.call.operations[0].kind, OperationKind::Read);
    assert!(
        candidates.iter().any(|candidate| matches!(
            candidate.path,
            SemanticClaimPath::Domain(solid_reactive_ir::contract_semantics::ClaimPath::Call(
                ClaimDomain::Reads
            ))
        )),
        "the generator may propose read closure but cannot finalize it"
    );
}

/// One export carrying exactly the two domains the path bootstrap fabricates
/// inside a dialect's own archive: a reactive read and an owner requirement.
fn bootstrapped_reactive_summary() -> ContractExport {
    owner_requirement_summary(solid_reactive_ir::OwnerRequirementOperation::Effect)
}

/// The same summary with the owner requirement's role chosen by the caller.
fn owner_requirement_summary(
    operation: solid_reactive_ir::OwnerRequirementOperation,
) -> ContractExport {
    ContractExport {
        kind: "function".into(),
        reactive_reads: ContractClaim::Known(vec![ContractReactiveRead {
            kind: "accessor".into(),
            label: String::new(),
            parameter: None,
            path: None,
            composed_owner: None,
            composed_from: None,
        }]),
        owner_requirements: ContractClaim::Known(vec![
            solid_reactive_ir::ContractOwnerRequirement { operation },
        ]),
        ..ContractExport::default()
    }
}

/// One normalization of `owner_requirement_summary(role)` for an ordinary
/// consuming package, which is the only scope that publishes the domain at all.
fn normalized_owner_requirement(
    operation: solid_reactive_ir::OwnerRequirementOperation,
) -> super::NormalizedInference {
    normalize_inferred_contract_with_candidates(
        &inferred(owner_requirement_summary(operation)),
        &resolution(["read".into()]),
    )
    .unwrap()
}

// The generator's scope decision, both directions, at the demand owner. The
// name is the only input, so the pair differs in nothing else.
#[test]
fn a_dialects_own_archive_publishes_neither_bootstrapped_reads_nor_owner_creates() {
    for package_name in ["solid-js", "@solidjs/signals", "@solidjs/web"] {
        let proposal = normalize_inferred_contract(
            &inferred(bootstrapped_reactive_summary()),
            &resolution_for_package(package_name, ["read".into()]),
        )
        .unwrap();
        let export = proposal.artifact_cases()[0].exports.get("read").unwrap();
        // Open, never closed-empty: `KnowledgeSet::Complete(vec![])` would be
        // the negative claim only the hand audits may assert.
        assert!(
            matches!(export.call.claims().reads, KnowledgeSet::Unknown),
            "{package_name} must leave reads open, never closed-empty"
        );
        assert!(
            matches!(export.call.claims().creates, KnowledgeSet::Unknown),
            "{package_name} must leave creates open, never closed-empty"
        );
        assert!(
            matches!(export.call.claims().cleanups, KnowledgeSet::Unknown),
            "{package_name} must leave cleanups open, never closed-empty"
        );
        assert!(
            export.call.operations.iter().all(|operation| !matches!(
                operation.kind,
                OperationKind::Read | OperationKind::Create | OperationKind::Cleanup
            )),
            "{package_name} must emit no read or owner-requirement operation"
        );
    }
}

#[test]
fn self_bootstrapped_callbacks_are_unknown_while_consuming_callbacks_survive() {
    for package_name in [
        "solid-js",
        "@solidjs/signals",
        "@solidjs/web",
        "@solidjs/router",
        "package",
    ] {
        let summary = ContractExport {
            kind: "function".into(),
            callbacks: ContractClaim::Known(vec![solid_reactive_ir::ContractCallback {
                parameter: 0,
                execution: "inline".into(),
                schedule: None,
                arguments: Vec::new(),
                owner: None,
            }]),
            ..ContractExport::default()
        };
        let normalized = normalize_inferred_contract_with_candidates(
            &inferred(summary),
            &resolution_for_package(package_name, ["read".into()]),
        )
        .unwrap();
        let export = &normalized.contract.artifact_cases()[0].exports["read"];
        if solid_dialect::primitive_defining_package(package_name) {
            assert!(matches!(
                export.call.claims().callbacks,
                KnowledgeSet::Unknown
            ));
            assert!(export.call.operations.is_empty());
            assert!(
                !normalized
                    .closure_candidates
                    .iter()
                    .any(|candidate| matches!(
                        candidate.path,
                        SemanticClaimPath::Domain(
                            solid_reactive_ir::contract_semantics::ClaimPath::Call(
                                ClaimDomain::Callbacks
                            )
                        )
                    ))
            );
        } else {
            assert!(
                matches!(&export.call.claims().callbacks, KnowledgeSet::Partial(callbacks) if callbacks.len() == 1)
            );
            assert_eq!(export.call.operations.len(), 1);
        }
    }
}

/// An ordinary consuming package publishes what it derived. The *cleanup* role
/// is the only owner-requirement role that has a home in schema version 1, so
/// this pair is a read operation and a `kind: cleanup` operation -- never a
/// `create`.
#[test]
fn an_ordinary_consuming_package_still_publishes_reads_and_owner_cleanups() {
    for package_name in [
        "package",
        "solid-js-signals",
        "@solidjs/router",
        "@solid-primitives/utils",
    ] {
        let proposal = normalize_inferred_contract(
            &inferred(owner_requirement_summary(
                solid_reactive_ir::OwnerRequirementOperation::Cleanup,
            )),
            &resolution_for_package(package_name, ["read".into()]),
        )
        .unwrap();
        let export = proposal.artifact_cases()[0].exports.get("read").unwrap();
        // The *claims* are reopened for every package by
        // `ContractProposal::normalize` -- the generator proposes closure and
        // cannot finalize it. What the scope decision changes is whether the
        // operations themselves are published at all.
        assert_eq!(
            export
                .call
                .operations
                .iter()
                .filter(|operation| matches!(
                    operation.kind,
                    OperationKind::Read | OperationKind::Cleanup
                ))
                .count(),
            2,
            "{package_name} must emit both operations"
        );
        assert!(
            export
                .call
                .operations
                .iter()
                .all(|operation| operation.kind != OperationKind::Create),
            "{package_name} must emit no create for an owner requirement"
        );
    }
}

/// The published cleanup shape, field by field: `kind: cleanup` in the
/// `cleanups` domain, `source: ambient-at-call` (the caller's owner, not one
/// this operation made), `requires: required`, `requiresCleanup: required`,
/// no owner production, and **no resource** -- a resource declaration is a
/// positive fact (`PositiveFactSubject::Resource`) with no witness on any
/// axis today, so declaring one would assert what nothing can prove.
#[test]
fn a_cleanup_owner_requirement_publishes_a_resourceless_cleanup_operation() {
    for role in [
        solid_reactive_ir::OwnerRequirementOperation::Cleanup,
        solid_reactive_ir::OwnerRequirementOperation::SettledCleanup,
    ] {
        let normalized = normalized_owner_requirement(role);
        assert!(
            normalized.withheld.is_empty(),
            "{role:?} is published, so nothing is withheld: {:?}",
            normalized.withheld
        );
        let export = normalized.contract.artifact_cases()[0]
            .exports
            .get("read")
            .unwrap();
        let cleanups = export.call.claims().cleanups.items().to_vec();
        assert_eq!(cleanups.len(), 1, "{role:?} must publish one cleanup item");
        let operation = export.operation(&cleanups[0].0).unwrap();
        assert_eq!(operation.kind, OperationKind::Cleanup);
        assert_eq!(
            operation.owner.source,
            solid_reactive_ir::contract_semantics::OwnerSource::AmbientAtCall
        );
        assert_eq!(
            operation.owner.requirements.owner,
            solid_reactive_ir::contract_semantics::Requirement::Required
        );
        assert_eq!(
            operation.owner.requirements.cleanup,
            solid_reactive_ir::contract_semantics::Requirement::Required
        );
        assert!(
            operation.resources.is_empty(),
            "{role:?} must name no resource: {:?}",
            operation.resources
        );
        assert!(
            export.call.resources.is_empty(),
            "{role:?} must declare no summary resource: {:?}",
            export.call.resources
        );
    }
}

/// The two roles this generation refuses. Each leaves no operation, no
/// `Creates` closure candidate, and a *named* withholding record carrying the
/// export, the role, and the reason -- which is what makes the refusal
/// distinguishable from a census that found nothing.
#[test]
fn a_withheld_owner_requirement_publishes_nothing_and_is_named() {
    for (role, name) in [
        (
            solid_reactive_ir::OwnerRequirementOperation::Effect,
            "effect",
        ),
        (
            solid_reactive_ir::OwnerRequirementOperation::Boundary,
            "boundary",
        ),
    ] {
        let normalized = normalized_owner_requirement(role);
        let export = normalized.contract.artifact_cases()[0]
            .exports
            .get("read")
            .unwrap();
        assert!(
            export.call.operations.iter().all(|operation| !matches!(
                operation.kind,
                OperationKind::Create | OperationKind::Cleanup
            )),
            "{role:?} must publish no owner-requirement operation"
        );
        assert!(
            matches!(export.call.claims().creates, KnowledgeSet::Unknown),
            "{role:?} must leave creates open"
        );
        assert!(
            matches!(export.call.claims().cleanups, KnowledgeSet::Unknown),
            "{role:?} must leave cleanups open"
        );
        assert!(
            !normalized
                .closure_candidates
                .iter()
                .any(|candidate| matches!(
                    candidate.path,
                    SemanticClaimPath::Domain(
                        solid_reactive_ir::contract_semantics::ClaimPath::Call(
                            ClaimDomain::Creates | ClaimDomain::Cleanups
                        )
                    )
                )),
            "{role:?} must propose no owner-requirement closure: {:?}",
            normalized.closure_candidates
        );
        assert_eq!(normalized.withheld.len(), 1, "{role:?} must be named once");
        assert_eq!(normalized.withheld[0].export, "read");
        assert_eq!(normalized.withheld[0].role.role(), name);
        assert!(
            !normalized.withheld[0].role.reason().is_empty(),
            "{role:?} must carry a reason"
        );
    }
}

/// `semantic-model.md` § creates' mechanical separator, asserted over the
/// generator's own output rather than trusted: a `create` operation names what
/// it registered. The generator now emits no `create` at all, which is the
/// strongest form of the same guarantee, so the assertion is written over
/// every operation of every role.
#[test]
fn the_generator_emits_no_resourceless_create() {
    for role in [
        solid_reactive_ir::OwnerRequirementOperation::Cleanup,
        solid_reactive_ir::OwnerRequirementOperation::SettledCleanup,
        solid_reactive_ir::OwnerRequirementOperation::Effect,
        solid_reactive_ir::OwnerRequirementOperation::Boundary,
    ] {
        let normalized = normalized_owner_requirement(role);
        for artifact_case in normalized.contract.artifact_cases() {
            for (name, export) in &artifact_case.exports {
                for operation in &export.call.operations {
                    assert!(
                        operation.kind != OperationKind::Create || !operation.resources.is_empty(),
                        "{role:?}: {name}'s create {} names no resource",
                        operation.id.0
                    );
                }
            }
        }
    }
}

/// The domains a normalization *proposes closure for*, which is the list the
/// certifier turns into demands. Withholding the operations has to withhold
/// these too: a proposed `Reads`/`Creates` closure with no operation behind it
/// would ask the certifier to prove a domain the document does not describe.
fn proposed_closure_domains(
    package_name: &str,
) -> Vec<solid_reactive_ir::contract_semantics::ClaimPath> {
    normalize_inferred_contract_with_candidates(
        &inferred(bootstrapped_reactive_summary()),
        &resolution_for_package(package_name, ["read".into()]),
    )
    .unwrap()
    .closure_candidates
    .into_iter()
    .map(|candidate| match candidate.path {
        SemanticClaimPath::Domain(path) => path,
        other => panic!("unexpected claim path {other:?}"),
    })
    .collect()
}

/// The whole generate-then-certify seam for a `creates` closure candidate, in
/// one test, because losing it here is what made the census unreachable.
///
/// The generator's walk cleared this export, so the normalization proposes a
/// `creates` closure. Weakening alone dropped the candidacy: the certifier
/// rebuilds its candidate universe by weakening the emitted document's own
/// closed claims, and the canonical main its receipt binds is that same
/// document, so a withdrawn closure reaches no demand, no probe gate and no
/// census. The document therefore states the closure and labels it proposed —
/// this generator's inference, not a reviewed claim.
#[test]
fn a_cleared_creates_walk_reaches_the_certifiers_candidate_universe_through_the_document() {
    use solid_reactive_ir::contract_semantics::{
        ClaimPath, KnowledgeState, SemanticClaimPath, certification::ProofPolicy2,
    };

    let summary = ContractExport {
        kind: "function".into(),
        creates_walk_clean: true,
        ..ContractExport::default()
    };
    let normalized = normalize_inferred_contract_with_candidates(
        &inferred(summary),
        &resolution(["read".into()]),
    )
    .unwrap();
    let creates = SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Creates));
    assert!(
        normalized
            .closure_candidates
            .iter()
            .any(|candidate| candidate.path == creates),
        "the walk cleared this export, so the plan must carry its candidate: {:?}",
        normalized.closure_candidates
    );

    let export = &normalized.contract.artifact_cases()[0].exports["read"];
    assert_eq!(
        export.claim_state(ClaimDomain::Creates),
        KnowledgeState::CompleteNegative,
        "the candidate has to state the closure it offers"
    );
    // `Reads` joins `Creates` here since 2026-09-10: this fixture's closure
    // installs no accessor at run time, so the reads census may decide it and
    // the generator proposes it. A fixture that did install one would carry a
    // `runtime-accessor-installation` hazard and this set would be `{Creates}`
    // again — that pair is what `implementation-census-reads` pins.
    assert_eq!(
        export.call.proposed_closures(),
        &std::collections::BTreeSet::from([ClaimDomain::Creates, ClaimDomain::Reads]),
        "labelled as proposed, so it stays distinguishable from a reviewed claim"
    );
    // Every other domain the walk cleared stays withdrawn: no census can
    // decide them, so publishing their closure would refuse the row.
    for domain in ClaimDomain::ALL {
        assert!(
            matches!(domain, ClaimDomain::Creates | ClaimDomain::Reads)
                || export.claim_state(domain).is_open(),
            "{domain:?} must stay open"
        );
    }

    // Through the encoder and back: the marker is a wire field, so the
    // candidate has to survive the canonicalization the emit boundary performs
    // before anything reads the document again.
    let bytes = crate::contract_document::encode(
        &normalized.contract,
        &crate::contract_document::SidecarDigests::default(),
        false,
    )
    .unwrap();
    let rendered = String::from_utf8_lossy(&bytes);
    assert!(
        rendered.contains("\"closed\":[\"reads\",\"creates\"]")
            && rendered.contains("\"creates\":[]"),
        "the emitted document must state the closure: {rendered}"
    );
    assert!(
        rendered.contains("\"proposedClosures\":[\"reads\",\"creates\"]"),
        "and must label it as proposed: {rendered}"
    );
    let decoded = crate::contract_document::decode(&bytes)
        .unwrap()
        .normalize()
        .unwrap();

    let candidates = ProofPolicy2.inspect_candidates(&decoded).unwrap();
    assert!(
        candidates
            .closure_candidates()
            .iter()
            .any(|candidate| candidate.path == creates && candidate.export == "read"),
        "the certifier must rebuild the candidate from the document alone: {:?}",
        candidates.closure_candidates()
    );
    // And having read it, the certifier's own proposal no longer offers it:
    // one candidate, planned once.
    let planned = &candidates.proposal().artifact_cases()[0].exports["read"];
    assert!(planned.call.proposed_closures().is_empty());
    assert_eq!(
        planned.claim_state(ClaimDomain::Creates),
        KnowledgeState::Unknown,
        "the planning proposal withdraws the closure it is about to demand"
    );
}

#[test]
fn a_dialects_own_archive_proposes_no_read_or_create_closure_candidate() {
    let carries = |domains: &[solid_reactive_ir::contract_semantics::ClaimPath], domain| {
        domains.iter().any(|path| {
            matches!(
                path,
                solid_reactive_ir::contract_semantics::ClaimPath::Call(found) if *found == domain
            )
        })
    };

    let withheld = proposed_closure_domains("@solidjs/signals");
    assert!(
        !carries(&withheld, ClaimDomain::Reads),
        "@solidjs/signals must propose no read closure: {withheld:?}"
    );
    assert!(
        !carries(&withheld, ClaimDomain::Creates),
        "@solidjs/signals must propose no owner-requirement closure: {withheld:?}"
    );

    // The same bytes under any other name: the *read* census is published, so
    // its closure is proposed. `Creates` is not, and no longer can be for any
    // package -- the generator derives no `create` at all, and an owner census
    // that found no owner requirement is not a census of registrations into an
    // outside runtime (`semantic-model.md` § creates).
    let published = proposed_closure_domains("package");
    assert!(
        carries(&published, ClaimDomain::Reads),
        "a consuming package must propose its read closure: {published:?}"
    );
    assert!(
        !carries(&published, ClaimDomain::Creates),
        "no package may propose a create closure the generator cannot derive: {published:?}"
    );
}

#[test]
fn parameter_indexes_outside_the_normalized_limit_are_refused_not_clamped() {
    let summary = ContractExport {
        kind: "function".into(),
        reactive_reads: ContractClaim::Known(vec![ContractReactiveRead {
            kind: "parameter".into(),
            label: String::new(),
            parameter: Some(usize::MAX),
            path: None,
            composed_owner: None,
            composed_from: None,
        }]),
        ..ContractExport::default()
    };
    let error =
        normalize_inferred_contract(&inferred(summary), &resolution(["read".into()])).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("exceeds the normalized model limit")
    );
}
