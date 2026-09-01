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

#[test]
fn parameter_indexes_outside_the_normalized_limit_are_refused_not_clamped() {
    let summary = ContractExport {
        kind: "function".into(),
        reactive_reads: ContractClaim::Known(vec![ContractReactiveRead {
            kind: "parameter".into(),
            label: String::new(),
            parameter: Some(usize::MAX),
            path: None,
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
