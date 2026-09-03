use std::collections::BTreeMap;

use solid_reactive_ir::{
    ContractClaim, ContractEntrypoint, ContractExport, ContractPackage, ContractReactiveRead,
    PackageContract,
    contract_semantics::{ClaimDomain, KnowledgeSet, OperationKind},
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
    let (proposal, candidates) = normalize_inferred_contract_with_candidates(
        &inferred(summary),
        &resolution(["read".into()]),
    )
    .unwrap();
    let export = proposal.artifact_cases()[0].exports.get("read").unwrap();

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
/// inside a dialect's own archive: a reactive read and an owner-requirement
/// create.
fn bootstrapped_reactive_summary() -> ContractExport {
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
            solid_reactive_ir::ContractOwnerRequirement {
                operation: solid_reactive_ir::OwnerRequirementOperation::Effect,
            },
        ]),
        ..ContractExport::default()
    }
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
            export.call.operations.iter().all(|operation| !matches!(
                operation.kind,
                OperationKind::Read | OperationKind::Create
            )),
            "{package_name} must emit no read or owner-requirement operation"
        );
    }
}

#[test]
fn an_ordinary_consuming_package_still_publishes_reads_and_owner_creates() {
    for package_name in [
        "package",
        "solid-js-signals",
        "@solidjs/router",
        "@solid-primitives/utils",
    ] {
        let proposal = normalize_inferred_contract(
            &inferred(bootstrapped_reactive_summary()),
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
                    OperationKind::Read | OperationKind::Create
                ))
                .count(),
            2,
            "{package_name} must emit both operations"
        );
    }
}

/// The domains a normalization *proposes closure for*, which is the list the
/// certifier turns into demands. Withholding the operations has to withhold
/// these too: a proposed `Reads`/`Creates` closure with no operation behind it
/// would ask the certifier to prove a domain the document does not describe.
fn proposed_closure_domains(
    package_name: &str,
) -> Vec<solid_reactive_ir::contract_semantics::ClaimPath> {
    let (_, candidates) = normalize_inferred_contract_with_candidates(
        &inferred(bootstrapped_reactive_summary()),
        &resolution_for_package(package_name, ["read".into()]),
    )
    .unwrap();
    candidates
        .into_iter()
        .map(|candidate| match candidate.path {
            SemanticClaimPath::Domain(path) => path,
            other => panic!("unexpected claim path {other:?}"),
        })
        .collect()
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

    // The same bytes under any other name: both domains are described, so both
    // are proposed for closure.
    let published = proposed_closure_domains("package");
    assert!(
        carries(&published, ClaimDomain::Reads),
        "a consuming package must propose its read closure: {published:?}"
    );
    assert!(
        carries(&published, ClaimDomain::Creates),
        "a consuming package must propose its owner-requirement closure: {published:?}"
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
