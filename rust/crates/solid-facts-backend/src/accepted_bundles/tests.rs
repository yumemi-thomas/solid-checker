use super::*;

use crate::contract_certification::{
    RECEIPT_WITNESS_FAMILIES, canonicalize_policy2_main, issue_builtin_policy2_receipt,
    policy2_main_closed_claims_root, policy2_main_semantic_digest,
};

const DOCUMENT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../benchmarks/package-contract-v2/phase6/minimal-unknown.json"
));

const BUILT_IN_SCOPE: &str = "solid-checker:bundled-accepted-contracts";

fn stand_in(index: u16) -> String {
    format!("sha256:{index:064x}")
}

/// One bundle, built the way `scripts/bundle-accepted-contracts.mjs` builds
/// one: canonicalize the certified document, bind the five identity fields,
/// issue a built-in receipt, and record its digest as the compiled-in entry.
fn bundle(
    package_name: &str,
    version: &str,
    integrity: &str,
    specifier: &str,
    conditions: &[&str],
) -> (BundleEntry, Vec<u8>, Vec<u8>) {
    let canonical =
        canonicalize_policy2_main(DOCUMENT).expect("the fixture document canonicalizes");
    let conditions = conditions
        .iter()
        .map(|it| (*it).to_owned())
        .collect::<Vec<_>>();
    let bindings = Policy2ReceiptBindings {
        importer: "/certification/importer.ts".into(),
        specifier: specifier.into(),
        resolved_import_root: stand_in(1),
        artifact_acceptance_root: policy2_artifact_acceptance_root_for_identity(
            package_name,
            version,
            integrity,
            ".",
            &conditions,
        ),
        semantic_digest: policy2_main_semantic_digest(&canonical).expect("semantic digest"),
        artifact_provenance_root: stand_in(2),
        snapshot_root: stand_in(3),
        package_root: stand_in(4),
        manifest_root: stand_in(5),
        artifacts_root: stand_in(6),
        declarations_root: stand_in(7),
        transform_root: stand_in(8),
        exports_root: stand_in(9),
        closure_root: stand_in(10),
        demand_graph_root: stand_in(11),
        verified_positive_root: stand_in(12),
        witness_roots: RECEIPT_WITNESS_FAMILIES
            .iter()
            .enumerate()
            .map(|(index, family)| {
                (
                    (*family).to_owned(),
                    stand_in(u16::try_from(100 + index).unwrap_or(u16::MAX)),
                )
            })
            .collect(),
        producer_sessions_root: stand_in(13),
        dependency_receipts_root: stand_in(14),
        dependency_trust_root: stand_in(15),
        probe_gate_root: stand_in(16),
        closed_claims_root: policy2_main_closed_claims_root(&canonical)
            .expect("closed-claims root"),
        verifier_source_digest: stand_in(17),
        verifier_build_digest: stand_in(18),
    };
    let receipt = issue_builtin_policy2_receipt(&canonical, &bindings, BUILT_IN_SCOPE)
        .expect("a built-in receipt is issuable over a canonical main");
    let entry = BundleEntry {
        package_name: package_name.into(),
        package_version: version.into(),
        package_integrity: integrity.into(),
        specifier: specifier.into(),
        requested_entrypoint: ".".into(),
        export_conditions: conditions,
        runtime_target: "dist/index.js".into(),
        declaration_target: "types/index.d.ts".into(),
        document: "objects/document.json".into(),
        document_digest: crate::contract_interface::sha256_digest(&canonical),
        receipt: "objects/receipt.json".into(),
        receipt_digest: crate::contract_interface::sha256_digest(&receipt),
        bindings,
    };
    (entry, canonical, receipt)
}

fn loaded(
    package_name: &str,
    version: &str,
    integrity: &str,
    specifier: &str,
    conditions: &[&str],
) -> LoadedBundle {
    let (entry, document, receipt) =
        bundle(package_name, version, integrity, specifier, conditions);
    load_bundle(&entry, &document, &receipt).expect("the bundle authenticates")
}

#[test]
fn a_built_in_receipt_authenticates_against_its_compiled_in_entry_digest() {
    let one = loaded(
        "plain-package",
        "1.0.0",
        "sha512-published-integrity",
        "plain-package",
        &["import"],
    );
    assert_eq!(one.specifier, "plain-package");
    assert_eq!(
        one.identity,
        policy2_artifact_acceptance_root_for_identity(
            "plain-package",
            "1.0.0",
            "sha512-published-integrity",
            ".",
            &["import".to_owned()],
        )
    );
}

#[test]
fn a_bundle_whose_stated_identity_is_not_the_one_it_signed_is_refused() {
    // The applicability premise in one assertion. Admission recomputes the
    // acceptance root from the *consumer's* installed name, version and
    // integrity and compares it to this bundle's; if the index could state a
    // version the receipt did not sign, that comparison would admit the
    // contract for an artifact it was never proven about.
    let (mut entry, document, receipt) = bundle(
        "plain-package",
        "1.0.0",
        "sha512-published-integrity",
        "plain-package",
        &["import"],
    );
    entry.package_version = "9.9.9".into();
    assert!(matches!(
        load_bundle(&entry, &document, &receipt),
        Err(ContractFailure::ReceiptMismatch {
            field: "artifactAcceptanceRoot"
        })
    ));
}

#[test]
fn a_tampered_object_is_refused_before_it_is_decoded() {
    let (entry, document, receipt) = bundle(
        "plain-package",
        "1.0.0",
        "sha512-published-integrity",
        "plain-package",
        &["import"],
    );
    let mut altered = receipt.clone();
    altered.extend_from_slice(b" ");
    assert!(matches!(
        load_bundle(&entry, &document, &altered),
        Err(ContractFailure::ReceiptMismatch {
            field: "receiptDigest"
        })
    ));
    let mut altered_document = document.clone();
    altered_document.extend_from_slice(b" ");
    assert!(matches!(
        load_bundle(&entry, &altered_document, &receipt),
        Err(ContractFailure::ReceiptMismatch {
            field: "documentDigest"
        })
    ));
}

#[test]
fn a_contract_about_the_runtime_foundation_cannot_be_bundled() {
    // ADR 0027: ordinary analysis takes `solid-js` from the dialect. A bundle
    // that supplied one would be a second authority over the runtime model,
    // reachable without any project configuration at all.
    let (entry, document, receipt) = bundle(
        "solid-js",
        "1.9.14",
        "sha512-published-integrity",
        "solid-js",
        &["import"],
    );
    assert!(matches!(
        load_bundle(&entry, &document, &receipt),
        Err(ContractFailure::IdentityMismatch { .. })
    ));
}

#[test]
fn admission_needs_the_installed_bytes_to_reproduce_the_signed_root() {
    let bundles = [loaded(
        "plain-package",
        "1.0.0",
        "sha512-published-integrity",
        "plain-package",
        &["import"],
    )];
    let conditions = BTreeSet::from(["import".to_owned()]);
    let resolved = |_: &str| Some("dist/index.js".to_owned());

    let same = |_: &str| {
        Some((
            "plain-package".to_owned(),
            "1.0.0".to_owned(),
            "sha512-published-integrity".to_owned(),
        ))
    };
    assert_eq!(
        admitted_from(&bundles, &conditions, &same, &resolved),
        vec![("plain-package".to_owned(), bundles[0].identity.clone())]
    );

    // A project that installed the same version from different bytes is not the
    // artifact this contract was proven about.
    let repacked = |_: &str| {
        Some((
            "plain-package".to_owned(),
            "1.0.0".to_owned(),
            "sha512-something-else".to_owned(),
        ))
    };
    assert!(admitted_from(&bundles, &conditions, &repacked, &resolved).is_empty());

    // And a project that cannot state its installed identity exactly -- two
    // installs that disagree -- admits nothing rather than picking one.
    let unstated = |_: &str| None;
    assert!(admitted_from(&bundles, &conditions, &unstated, &resolved).is_empty());
}

#[test]
fn admission_needs_this_project_to_have_resolved_the_file_the_contract_is_about() {
    let bundles = [loaded(
        "plain-package",
        "1.0.0",
        "sha512-published-integrity",
        "plain-package",
        &["import"],
    )];
    let conditions = BTreeSet::from(["import".to_owned()]);
    let installed = |_: &str| {
        Some((
            "plain-package".to_owned(),
            "1.0.0".to_owned(),
            "sha512-published-integrity".to_owned(),
        ))
    };
    // The declaration file is accepted as well as the runtime one: TypeScript
    // resolves the former, and both name the same case here.
    for target in ["dist/index.js", "types/index.d.ts"] {
        let resolved = |_: &str| Some(target.to_owned());
        assert_eq!(
            admitted_from(&bundles, &conditions, &installed, &resolved).len(),
            1,
            "{target} is a file this bundle was certified about"
        );
    }
    let elsewhere = |_: &str| Some("dist/other.js".to_owned());
    assert!(admitted_from(&bundles, &conditions, &installed, &elsewhere).is_empty());
}

#[test]
fn a_require_project_does_not_get_a_contract_proven_under_import() {
    // Conditions select the artifact, which is why they are inside the
    // acceptance root. This is the consumer-side half: a declaration that does
    // not cover the case's conditions never reaches it.
    let bundles = [loaded(
        "plain-package",
        "1.0.0",
        "sha512-published-integrity",
        "plain-package",
        &["import"],
    )];
    let installed = |_: &str| {
        Some((
            "plain-package".to_owned(),
            "1.0.0".to_owned(),
            "sha512-published-integrity".to_owned(),
        ))
    };
    let resolved = |_: &str| Some("dist/index.js".to_owned());
    let required = BTreeSet::from(["require".to_owned(), "node".to_owned()]);
    assert!(admitted_from(&bundles, &required, &installed, &resolved).is_empty());
}

#[test]
fn a_bundle_is_reachable_by_its_artifact_and_by_no_importer() {
    let one = loaded(
        "plain-package",
        "1.0.0",
        "sha512-published-integrity",
        "plain-package",
        &["import"],
    );
    let identity = one.identity.clone();
    let index =
        AcceptedContractIndex::from_artifact_acceptances([(identity.clone(), one.contract)]);
    // Nothing about this project's imports: a report that enumerates accepted
    // packages must not name a bundle the project never reached.
    assert!(index.semantic_identity().is_empty());
    assert!(index.contract_for_artifact(&identity).is_some());
    assert!(
        index
            .contract("/certification/importer.ts", "plain-package")
            .is_err()
    );
    let admitted = index.with_admitted_artifacts([("plain-package".to_owned(), identity)]);
    assert!(
        admitted
            .contract("/any/consumer.ts", "plain-package")
            .is_ok()
    );
}

#[test]
fn every_bundle_this_build_carries_authenticates() {
    // The pin on `pkg/contracts/accepted/`: a checked-in bundle set whose
    // objects, digests or bindings drift apart fails here rather than at a
    // user's first analysis.
    bundles().expect("the compiled-in accepted-contract tier loads");
}
