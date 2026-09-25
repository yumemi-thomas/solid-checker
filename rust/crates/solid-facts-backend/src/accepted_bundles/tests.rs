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

fn entry(name: &str, version: &str, integrity: &str) -> DependencyEnvironmentEntry {
    DependencyEnvironmentEntry {
        name: name.into(),
        version: version.into(),
        integrity: integrity.into(),
    }
}

/// The floor and head rows of the ecosystem corpus in miniature: the same
/// package bytes, certified once against `@solidjs/signals@2.0.0-rc.0` and once
/// against rc.6, whose audited rows are what closed the head certification.
fn floor() -> Vec<DependencyEnvironmentEntry> {
    vec![entry(
        "@solidjs/signals",
        "2.0.0-rc.0",
        "sha512-signals-rc0",
    )]
}

fn head() -> Vec<DependencyEnvironmentEntry> {
    vec![entry(
        "@solidjs/signals",
        "2.0.0-rc.6",
        "sha512-signals-rc6",
    )]
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
    bundle_in(
        package_name,
        version,
        integrity,
        specifier,
        conditions,
        Some(&[]),
    )
}

/// [`bundle`] certified in a stated environment, or in none (`None`, the shape
/// every receipt issued before the binding existed has).
fn bundle_in(
    package_name: &str,
    version: &str,
    integrity: &str,
    specifier: &str,
    conditions: &[&str],
    environment: Option<&[DependencyEnvironmentEntry]>,
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
        dependency_environment_root: environment
            .map(policy2_dependency_environment_root)
            .unwrap_or_default(),
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
        dependency_environment: environment.map(<[_]>::to_vec),
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

fn loaded_in(environment: &[DependencyEnvironmentEntry]) -> LoadedBundle {
    let (entry, document, receipt) = bundle_in(
        "plain-package",
        "1.0.0",
        "sha512-published-integrity",
        "plain-package",
        &["import"],
        Some(environment),
    );
    load_bundle(&entry, &document, &receipt).expect("the bundle authenticates")
}

/// Any environment is installed. For the tests about the artifact half, which
/// the environment half must not change.
fn any_environment(_: &str, _: &[DependencyEnvironmentEntry]) -> bool {
    true
}

/// An installed tree, as the location of every package in it: `(location,
/// name) -> location` is what Node would resolve, and each location's identity.
struct Tree {
    resolves: BTreeMap<(&'static str, &'static str), &'static str>,
    identities: BTreeMap<&'static str, DependencyEnvironmentEntry>,
}

impl Tree {
    fn installs(&self, environment: &[DependencyEnvironmentEntry]) -> bool {
        environment_is_installed(
            environment,
            "root",
            |from: &&str, name: &str| Ok(self.resolves.get(&(*from, name)).copied()),
            |at: &&str| self.identities.get(at).cloned(),
        )
    }
}

/// `root` depends on signals, hoisted beside it: the flat tree every package
/// manager writes for a single consumer.
fn flat_tree(signals: DependencyEnvironmentEntry) -> Tree {
    Tree {
        resolves: BTreeMap::from([(("root", "@solidjs/signals"), "signals")]),
        identities: BTreeMap::from([("signals", signals)]),
    }
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
        one.acceptance_root,
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
        admitted_from(&bundles, &conditions, &same, &resolved, &any_environment),
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
    assert!(
        admitted_from(
            &bundles,
            &conditions,
            &repacked,
            &resolved,
            &any_environment
        )
        .is_empty()
    );

    // And a project that cannot state its installed identity exactly -- two
    // installs that disagree -- admits nothing rather than picking one.
    let unstated = |_: &str| None;
    assert!(
        admitted_from(
            &bundles,
            &conditions,
            &unstated,
            &resolved,
            &any_environment
        )
        .is_empty()
    );
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
            admitted_from(
                &bundles,
                &conditions,
                &installed,
                &resolved,
                &any_environment
            )
            .len(),
            1,
            "{target} is a file this bundle was certified about"
        );
    }
    let elsewhere = |_: &str| Some("dist/other.js".to_owned());
    assert!(
        admitted_from(
            &bundles,
            &conditions,
            &installed,
            &elsewhere,
            &any_environment
        )
        .is_empty()
    );
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
    assert!(admitted_from(&bundles, &required, &installed, &resolved, &any_environment).is_empty());
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

#[test]
fn a_matching_environment_admits_and_a_different_one_refuses() {
    let bundles = [loaded_in(&head())];
    let conditions = BTreeSet::from(["import".to_owned()]);
    let resolved = |_: &str| Some("dist/index.js".to_owned());
    let installed = |_: &str| {
        Some((
            "plain-package".to_owned(),
            "1.0.0".to_owned(),
            "sha512-published-integrity".to_owned(),
        ))
    };
    let admit = |tree: &Tree| {
        let environment = |specifier: &str, environment: &[DependencyEnvironmentEntry]| {
            assert_eq!(
                specifier, "plain-package",
                "resolved from the imported package"
            );
            tree.installs(environment)
        };
        admitted_from(&bundles, &conditions, &installed, &resolved, &environment)
    };

    let same = flat_tree(entry(
        "@solidjs/signals",
        "2.0.0-rc.6",
        "sha512-signals-rc6",
    ));
    assert_eq!(
        admit(&same),
        vec![("plain-package".to_owned(), bundles[0].identity.clone())],
        "the environment the proof read is installed, so the proof applies"
    );

    // The floor row's tree. The package bytes are the ones certified, and the
    // closure is still not true here: it was discharged by rc.6's rows.
    let floor_signals = flat_tree(entry(
        "@solidjs/signals",
        "2.0.0-rc.0",
        "sha512-signals-rc6",
    ));
    assert!(
        admit(&floor_signals).is_empty(),
        "a different signals version"
    );

    let repacked = flat_tree(entry("@solidjs/signals", "2.0.0-rc.6", "sha512-repacked"));
    assert!(
        admit(&repacked).is_empty(),
        "a different dependency integrity"
    );

    let missing = Tree {
        resolves: BTreeMap::new(),
        identities: BTreeMap::new(),
    };
    assert!(admit(&missing).is_empty(), "a dependency nothing resolves");

    let unstated = Tree {
        resolves: BTreeMap::from([(("root", "@solidjs/signals"), "signals")]),
        identities: BTreeMap::new(),
    };
    assert!(
        admit(&unstated).is_empty(),
        "a dependency whose identity the tree cannot state, a lockfile with no integrity"
    );
}

#[test]
fn the_environment_is_resolved_from_every_package_that_reads_it() {
    // `utils` is the semantic dependency and reads signals itself. The hoisted
    // signals is the certified one, and a nested copy under utils is not: the
    // package utils actually loads is the nested one.
    let environment = vec![
        entry("@solid-primitives/utils", "7.0.0-next.4", "sha512-utils"),
        entry("@solidjs/signals", "2.0.0-rc.6", "sha512-signals-rc6"),
    ];
    let flat = Tree {
        resolves: BTreeMap::from([
            (("root", "@solid-primitives/utils"), "utils"),
            (("root", "@solidjs/signals"), "signals"),
            (("utils", "@solidjs/signals"), "signals"),
            (("utils", "@solid-primitives/utils"), "utils"),
            (("signals", "@solid-primitives/utils"), "utils"),
            (("signals", "@solidjs/signals"), "signals"),
        ]),
        identities: BTreeMap::from([
            ("utils", environment[0].clone()),
            ("signals", environment[1].clone()),
        ]),
    };
    assert!(flat.installs(&environment));

    let mut nested = flat;
    nested
        .resolves
        .insert(("utils", "@solidjs/signals"), "utils/signals");
    nested.identities.insert(
        "utils/signals",
        entry("@solidjs/signals", "2.0.0-rc.0", "sha512-signals-rc0"),
    );
    assert!(
        !nested.installs(&environment),
        "a copy only utils sees is still a copy the proof never read"
    );

    // A transitive entry reached only through the dependency, never from the
    // root, is still reached.
    let transitive = Tree {
        resolves: BTreeMap::from([
            (("root", "@solid-primitives/utils"), "utils"),
            (("utils", "@solidjs/signals"), "utils/signals"),
            (("utils", "@solid-primitives/utils"), "utils"),
        ]),
        identities: BTreeMap::from([
            ("utils", environment[0].clone()),
            ("utils/signals", environment[1].clone()),
        ]),
    };
    assert!(transitive.installs(&environment));
}

#[test]
fn an_environment_that_read_two_copies_of_one_package_never_applies() {
    // The receipt states which packages were read, not which package read
    // each, so two copies cannot be told apart from a swap of them.
    let environment = vec![
        entry("@solidjs/signals", "2.0.0-rc.0", "sha512-signals-rc0"),
        entry("@solidjs/signals", "2.0.0-rc.6", "sha512-signals-rc6"),
    ];
    let tree = flat_tree(environment[1].clone());
    assert!(!tree.installs(&environment));
    assert!(flat_tree(entry("x", "1", "sha512-x")).installs(&[]));
}

#[test]
fn a_tree_that_cannot_answer_a_lookup_refuses() {
    let environment = head();
    assert!(!environment_is_installed(
        &environment,
        "root",
        |_: &&str, _: &str| Err(()),
        |_: &&str| None,
    ));
}

#[test]
fn a_receipt_that_states_no_environment_is_never_admitted() {
    // Every bundle in an index written before the binding existed. It still
    // authenticates -- its bytes are exactly what was reviewed -- and it is
    // inert: which environment it was proven in is not a fact it states.
    let (entry, document, receipt) = bundle_in(
        "plain-package",
        "1.0.0",
        "sha512-published-integrity",
        "plain-package",
        &["import"],
        None,
    );
    let bundle = load_bundle(&entry, &document, &receipt).expect("the bundle authenticates");
    assert!(bundle.environment.is_none());
    let installed = |_: &str| {
        Some((
            "plain-package".to_owned(),
            "1.0.0".to_owned(),
            "sha512-published-integrity".to_owned(),
        ))
    };
    let resolved = |_: &str| Some("dist/index.js".to_owned());
    let conditions = BTreeSet::from(["import".to_owned()]);
    assert!(
        admitted_from(
            &[bundle],
            &conditions,
            &installed,
            &resolved,
            &any_environment
        )
        .is_empty()
    );
}

#[test]
fn an_index_that_states_an_environment_its_receipt_did_not_sign_is_refused() {
    // Admission compares the consumer's tree against the index's entries, so
    // an entry the receipt's root does not reproduce would be an environment
    // nobody proved anything in.
    let (mut entry, document, receipt) = bundle_in(
        "plain-package",
        "1.0.0",
        "sha512-published-integrity",
        "plain-package",
        &["import"],
        Some(&head()),
    );
    entry.dependency_environment = Some(floor());
    assert!(matches!(
        load_bundle(&entry, &document, &receipt),
        Err(ContractFailure::ReceiptMismatch {
            field: "dependencyEnvironment"
        })
    ));
    // And entries beside a receipt that binds no environment at all.
    let (mut entry, document, receipt) = bundle_in(
        "plain-package",
        "1.0.0",
        "sha512-published-integrity",
        "plain-package",
        &["import"],
        None,
    );
    entry.dependency_environment = Some(head());
    assert!(matches!(
        load_bundle(&entry, &document, &receipt),
        Err(ContractFailure::ReceiptMismatch {
            field: "dependencyEnvironment"
        })
    ));
}

#[test]
fn floor_and_head_certifications_of_one_artifact_are_two_bundles() {
    let floor_bundle = loaded_in(&floor());
    let head_bundle = loaded_in(&head());
    assert_eq!(floor_bundle.acceptance_root, head_bundle.acceptance_root);
    assert_ne!(
        floor_bundle.identity, head_bundle.identity,
        "one artifact in two environments is two acceptances"
    );
    // Keyed by the artifact alone, two different contracts under one key
    // would be dropped as two answers about the same bytes.
    let index = AcceptedContractIndex::from_artifact_acceptances([
        (floor_bundle.identity.clone(), floor_bundle.contract.clone()),
        (head_bundle.identity.clone(), head_bundle.contract.clone()),
    ]);
    assert!(
        index
            .contract_for_artifact(&floor_bundle.identity)
            .is_some()
    );
    assert!(index.contract_for_artifact(&head_bundle.identity).is_some());

    let bundles = [floor_bundle, head_bundle];
    let conditions = BTreeSet::from(["import".to_owned()]);
    let resolved = |_: &str| Some("dist/index.js".to_owned());
    let installed = |_: &str| {
        Some((
            "plain-package".to_owned(),
            "1.0.0".to_owned(),
            "sha512-published-integrity".to_owned(),
        ))
    };
    for (tree, expected) in [
        (flat_tree(floor()[0].clone()), &bundles[0].identity),
        (flat_tree(head()[0].clone()), &bundles[1].identity),
    ] {
        let environment =
            |_: &str, environment: &[DependencyEnvironmentEntry]| tree.installs(environment);
        assert_eq!(
            admitted_from(&bundles, &conditions, &installed, &resolved, &environment),
            vec![("plain-package".to_owned(), expected.clone())],
            "each tree gets the certification of its own environment, and only that one"
        );
    }
}

/// The whole native admission path over a real installed tree: the package's
/// own identity from its manifest and lockfile, then the environment resolved
/// from its installed copy. The consumer that installed the certified bytes
/// beside `@solidjs/signals@2.0.0-rc.0` does not get the contract proven
/// against rc.6, and the one beside rc.6 does -- each gets exactly the
/// certification of its own environment.
#[test]
fn a_consumer_tree_gets_only_the_certification_of_its_own_environment() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-bundle-admission-{}",
        std::process::id()
    ));
    let install = |signals_version: &str, signals_integrity: &str| {
        let _ = std::fs::remove_dir_all(&directory);
        let write = |relative: &str, text: String| {
            let path = directory.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(
            "node_modules/plain-package/package.json",
            r#"{"name":"plain-package","version":"1.0.0"}"#.to_owned(),
        );
        write(
            "node_modules/@solidjs/signals/package.json",
            format!(r#"{{"name":"@solidjs/signals","version":"{signals_version}"}}"#),
        );
        write(
            "package-lock.json",
            format!(
                r#"{{"lockfileVersion":3,"packages":{{
                    "node_modules/plain-package":
                        {{"version":"1.0.0","integrity":"sha512-published-integrity"}},
                    "node_modules/@solidjs/signals":
                        {{"version":"{signals_version}","integrity":"{signals_integrity}"}}}}}}"#
            ),
        );
    };
    let bundles = [loaded_in(&floor()), loaded_in(&head())];
    let conditions = BTreeSet::from(["import".to_owned()]);
    let resolved = |_: &str| Some("dist/index.js".to_owned());
    let admit = || {
        let installed = |specifier: &str| {
            crate::diagnostics::installed_artifact_identity(&directory, specifier)
        };
        let environment = |specifier: &str, environment: &[DependencyEnvironmentEntry]| {
            crate::diagnostics::installed_environment_matches(&directory, specifier, environment)
        };
        admitted_from(&bundles, &conditions, &installed, &resolved, &environment)
    };

    install("2.0.0-rc.6", "sha512-signals-rc6");
    assert_eq!(
        admit(),
        vec![("plain-package".to_owned(), bundles[1].identity.clone())],
        "the head tree gets the head certification"
    );
    install("2.0.0-rc.0", "sha512-signals-rc0");
    assert_eq!(
        admit(),
        vec![("plain-package".to_owned(), bundles[0].identity.clone())],
        "the floor tree gets the floor certification, never the head one"
    );
    // A signals no certification read: the same package bytes, and nothing.
    install("2.0.0-rc.3", "sha512-signals-rc3");
    assert!(admit().is_empty());
    // The right version from different bytes.
    install("2.0.0-rc.6", "sha512-signals-repacked");
    assert!(admit().is_empty());
    let _ = std::fs::remove_dir_all(&directory);
}
