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
    DependencyEnvironmentEntry::package(name, version, integrity)
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

/// The installed files are the archive every test bundle signs
/// (`snapshotRoot` is `stand_in(3)`). For the tests about everything but the
/// bytes, which the bytes half must not change.
#[allow(clippy::unnecessary_wraps)]
fn signed_bytes(_: &str) -> Result<String, String> {
    Ok(stand_in(3))
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
            |_: &&str| None,
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
        admitted_from(
            &bundles,
            &conditions,
            &same,
            &signed_bytes,
            &resolved,
            &any_environment
        ),
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
            &signed_bytes,
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
            &signed_bytes,
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
                &signed_bytes,
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
            &signed_bytes,
            &elsewhere,
            &any_environment
        )
        .is_empty()
    );
}

/// A case certified under `browser` may rest on a dialect row scoped to that
/// host (`solid-js`' `createSignal`), so a host that declared nothing — every
/// ESLint and Oxlint run — never receives it, even as the lone candidate and
/// even beside an unscoped case it would otherwise be compared with. A host
/// that declared `--runtime-target browser` does, by the ordinary subset rule.
#[test]
fn an_undeclared_host_never_receives_a_browser_scoped_case() {
    let installed = |_: &str| {
        Some((
            "plain-package".to_owned(),
            "1.0.0".to_owned(),
            "sha512-published-integrity".to_owned(),
        ))
    };
    let resolved = |_: &str| Some("dist/index.js".to_owned());
    let case = |conditions: &[&str]| {
        loaded(
            "plain-package",
            "1.0.0",
            "sha512-published-integrity",
            "plain-package",
            conditions,
        )
    };
    // `[browser case, unscoped case]`, fresh each time: a loaded bundle owns
    // its decoded contract.
    let both = || [case(&["browser", "import"]), case(&["import"])];
    let identities = both().map(|bundle| bundle.identity);
    let admitted = |bundles: &[LoadedBundle], conditions: &BTreeSet<String>| {
        admitted_from(
            bundles,
            conditions,
            &installed,
            &signed_bytes,
            &resolved,
            &any_environment,
        )
        .into_iter()
        .map(|(specifier, identity)| {
            assert_eq!(specifier, "plain-package");
            identity
        })
        .collect::<Vec<_>>()
    };

    let undeclared = BTreeSet::new();
    assert!(
        admitted(&[case(&["browser", "import"])], &undeclared).is_empty(),
        "a lone browser-scoped candidate is not admitted unchecked"
    );
    assert_eq!(
        admitted(&both(), &undeclared),
        vec![identities[1].clone()],
        "the browser case is dropped before any comparison, the unscoped one stays"
    );

    let host = |target| {
        solid_reactive_ir::RuntimeEnvironment {
            target: Some(target),
            ..Default::default()
        }
        .selected_conditions()
    };
    assert_eq!(
        admitted(&both(), &host(solid_reactive_ir::RuntimeTarget::Browser)),
        vec![identities[0].clone()],
        "a `--runtime-target browser` host gets the most specific applicable case"
    );
    assert_eq!(
        admitted(&both(), &host(solid_reactive_ir::RuntimeTarget::Node)),
        vec![identities[1].clone()],
        "a node host never gets the browser case"
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
    assert!(
        admitted_from(
            &bundles,
            &required,
            &installed,
            &signed_bytes,
            &resolved,
            &any_environment
        )
        .is_empty()
    );
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
        admitted_from(
            &bundles,
            &conditions,
            &installed,
            &signed_bytes,
            &resolved,
            &environment,
        )
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
            &signed_bytes,
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
            admitted_from(
                &bundles,
                &conditions,
                &installed,
                &signed_bytes,
                &resolved,
                &environment
            ),
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
        admitted_from(
            &bundles,
            &conditions,
            &installed,
            &signed_bytes,
            &resolved,
            &environment,
        )
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

/// The diagnostic twin of `environment_is_installed` names the first entry
/// that differs, with the installed version beside the certified one, and
/// agrees with it on every tree.
#[test]
fn an_environment_difference_names_the_first_differing_package() {
    let certified = entry("@solidjs/signals", "2.0.0-rc.6", "sha512-rc6");
    let differs = |tree: &Tree, version_of: Option<&str>| {
        let found = environment_difference(
            std::slice::from_ref(&certified),
            "root",
            |from: &&str, name: &str| Ok(tree.resolves.get(&(*from, name)).copied()),
            |at: &&str| tree.identities.get(at).cloned(),
            |_: &&str| None,
            |_: &&str| version_of.map(str::to_owned),
        );
        assert_eq!(
            found.is_none(),
            tree.installs(std::slice::from_ref(&certified)),
            "the two readings agree"
        );
        found.map(|difference| difference.to_string())
    };
    assert_eq!(differs(&flat_tree(certified.clone()), None), None);
    assert_eq!(
        differs(
            &flat_tree(entry("@solidjs/signals", "2.0.0-rc.0", "sha512-rc0")),
            None
        )
        .as_deref(),
        Some("@solidjs/signals installed 2.0.0-rc.0, certified 2.0.0-rc.6")
    );
    assert_eq!(
        differs(
            &flat_tree(entry("@solidjs/signals", "2.0.0-rc.6", "sha512-other")),
            None
        )
        .as_deref(),
        Some(
            "@solidjs/signals 2.0.0-rc.6 installed with integrity sha512-other, certified with sha512-rc6"
        )
    );
    let unidentified = Tree {
        resolves: BTreeMap::from([(("root", "@solidjs/signals"), "signals")]),
        identities: BTreeMap::new(),
    };
    assert_eq!(
        differs(&unidentified, Some("2.0.0-rc.3")).as_deref(),
        Some("@solidjs/signals installed 2.0.0-rc.3, certified 2.0.0-rc.6")
    );
    let absent = Tree {
        resolves: BTreeMap::new(),
        identities: BTreeMap::new(),
    };
    assert_eq!(
        differs(&absent, None).as_deref(),
        Some("@solidjs/signals not installed, certified 2.0.0-rc.6")
    );
}

/// `missing` says why: the admission steps replayed, first failure reported.
#[test]
fn admission_refusals_name_the_step_that_failed() {
    let certified = [entry("@solidjs/signals", "2.0.0-rc.6", "sha512-rc6")];
    let bundle = loaded_in(&certified);
    let stated = || ArtifactAcceptance {
        specifier: &bundle.specifier,
        requested_entrypoint: &bundle.requested_entrypoint,
        export_conditions: &bundle.export_conditions,
        runtime_target: &bundle.runtime_target,
        declaration_target: &bundle.declaration_target,
        acceptance_root: &bundle.acceptance_root,
        snapshot_root: &bundle.snapshot_root,
        environment: bundle.environment.as_deref(),
        identity: &bundle.identity,
    };
    let installed = |version: &'static str| {
        move |_: &str| {
            Some((
                "plain-package".to_owned(),
                version.to_owned(),
                "sha512-published-integrity".to_owned(),
            ))
        }
    };
    let same = |_: &str, _: &[DependencyEnvironmentEntry]| None;
    let rc0 = |_: &str, _: &[DependencyEnvironmentEntry]| {
        Some("@solidjs/signals installed 2.0.0-rc.0, certified 2.0.0-rc.6".to_owned())
    };
    let refusal = |acceptance,
                   installed: &InstalledArtifactIdentity,
                   difference: &InstalledEnvironmentDifference| {
        admission_refusals(
            [(acceptance, "1.0.0")],
            installed,
            &signed_bytes,
            difference,
        )
        .pop()
        .unwrap()
        .1
    };
    assert_eq!(refusal(stated(), &installed("1.0.0"), &same), None);
    assert_eq!(
        refusal(stated(), &installed("1.0.0"), &rc0),
        Some(AdmissionRefusal::EnvironmentDiffers(
            "@solidjs/signals installed 2.0.0-rc.0, certified 2.0.0-rc.6".into()
        ))
    );
    let wrong_version = refusal(stated(), &installed("1.0.1"), &rc0).unwrap();
    assert!(
        matches!(
            wrong_version,
            AdmissionRefusal::AcceptanceRootNotReproduced { .. }
        ),
        "the artifact step is checked before the environment"
    );
    assert_eq!(
        wrong_version.to_string(),
        "the installed package is 1.0.1, not the certified 1.0.0"
    );
    let mut unstated = stated();
    unstated.environment = None;
    assert_eq!(
        refusal(unstated, &installed("1.0.1"), &rc0),
        Some(AdmissionRefusal::NoEnvironmentStated)
    );
    assert_eq!(
        refusal(stated(), &|_: &str| None, &same).map(|refusal| refusal.to_string()),
        Some(
            "the installed package has no exact lockfile integrity, so its acceptance root \
             cannot be reproduced"
                .into()
        )
    );
}

/// A pnpm tree, as measured in kobalte core: the certified
/// `vite-plugin-solid` resolves its own `merge-anything@5.1.7` from its store
/// directory, while `.pnpm/node_modules` hoists 6.0.6, which every other store
/// package sees -- `solid-refresh` among them, though it never looks
/// `merge-anything` up for the certification.
fn pnpm_tree() -> Tree {
    Tree {
        resolves: BTreeMap::from([
            (("root", "merge-anything"), "vps/merge-anything"),
            (("root", "solid-refresh"), "sr"),
            (("vps/merge-anything", "is-what"), "ma/is-what"),
            // Hoisted: what every other store package sees.
            (("sr", "merge-anything"), "hoisted/merge-anything"),
            (("sr", "is-what"), "hoisted/is-what"),
            (("ma/is-what", "merge-anything"), "hoisted/merge-anything"),
        ]),
        identities: BTreeMap::from([
            (
                "root",
                entry("vite-plugin-solid", "3.0.0-next.5", "sha512-vps"),
            ),
            ("vps/merge-anything", merge_anything("5.1.7")),
            ("hoisted/merge-anything", merge_anything("6.0.6")),
            ("sr", entry("solid-refresh", "0.8.0-next.7", "sha512-sr")),
            ("ma/is-what", entry("is-what", "4.1.8", "sha512-iw4")),
            ("hoisted/is-what", entry("is-what", "5.0.0", "sha512-iw5")),
        ]),
    }
}

fn merge_anything(version: &str) -> DependencyEnvironmentEntry {
    entry("merge-anything", version, &format!("sha512-ma-{version}"))
}

/// The environment the certification in [`pnpm_tree`] read, with the lookup
/// that reached each entry.
fn pnpm_environment() -> Vec<DependencyEnvironmentEntry> {
    let merge = merge_anything("5.1.7");
    let mut environment = vec![
        entry("is-what", "4.1.8", "sha512-iw4").resolved_from(merge.as_importer(), "is-what"),
        merge.resolved_from(EnvironmentImporter::Certified, "merge-anything"),
        entry("solid-refresh", "0.8.0-next.7", "sha512-sr")
            .resolved_from(EnvironmentImporter::Certified, "solid-refresh"),
    ];
    environment.sort();
    environment
}

impl Tree {
    fn difference(&self, environment: &[DependencyEnvironmentEntry]) -> Option<String> {
        let found = environment_difference(
            environment,
            "root",
            |from: &&str, name: &str| Ok(self.resolves.get(&(*from, name)).copied()),
            |at: &&str| self.identities.get(at).cloned(),
            |_: &&str| None,
            |_: &&str| None,
        );
        assert_eq!(
            found.is_none(),
            self.installs(environment),
            "the two readings agree"
        );
        found.map(|difference| difference.to_string())
    }
}

#[test]
fn a_hoisted_different_version_elsewhere_in_the_tree_is_irrelevant_to_edges() {
    let tree = pnpm_tree();
    let environment = pnpm_environment();
    crate::contract_certification::validate_dependency_environment(&environment)
        .expect("a canonical, rooted edge-bearing environment");
    assert_eq!(tree.difference(&environment), None);
    // The same packages stated without edges keep the strict rule, and the
    // strict rule refuses the very tree the certification ran in -- the
    // measured defect.
    let strict = {
        let mut strict = environment
            .iter()
            .map(DependencyEnvironmentEntry::without_edge)
            .collect::<Vec<_>>();
        strict.sort();
        strict
    };
    assert_eq!(
        tree.difference(&strict).as_deref(),
        Some("is-what installed 5.0.0, certified 4.1.8")
    );
}

#[test]
fn the_importers_own_resolution_differing_refuses_and_names_the_edge() {
    let mut tree = pnpm_tree();
    tree.resolves
        .insert(("root", "merge-anything"), "hoisted/merge-anything");
    assert_eq!(
        tree.difference(&pnpm_environment()).as_deref(),
        Some(
            "merge-anything resolved from vite-plugin-solid@3.0.0-next.5 installed 6.0.6, certified 5.1.7"
        )
    );
    // A transitive importer's own lookup is checked from its own location.
    let mut tree = pnpm_tree();
    tree.resolves
        .insert(("vps/merge-anything", "is-what"), "hoisted/is-what");
    assert_eq!(
        tree.difference(&pnpm_environment()).as_deref(),
        Some("is-what resolved from merge-anything@5.1.7 installed 5.0.0, certified 4.1.8")
    );
}

#[test]
fn a_missing_or_unresolvable_edge_target_refuses() {
    let mut tree = pnpm_tree();
    tree.resolves.remove(&("vps/merge-anything", "is-what"));
    assert_eq!(
        tree.difference(&pnpm_environment()).as_deref(),
        Some("is-what resolved from merge-anything@5.1.7 not installed, certified 4.1.8")
    );
    let tree = pnpm_tree();
    assert!(!environment_is_installed(
        &pnpm_environment(),
        "root",
        |from: &&str, name: &str| {
            if *from == "vps/merge-anything" {
                Err(())
            } else {
                Ok(tree.resolves.get(&(*from, name)).copied())
            }
        },
        |at: &&str| tree.identities.get(at).cloned(),
        |_: &&str| None,
    ));
}

#[test]
fn two_importers_resolving_two_versions_of_one_name_are_admitted_when_both_match() {
    let tree = pnpm_tree();
    let solid_refresh = entry("solid-refresh", "0.8.0-next.7", "sha512-sr");
    let mut environment = pnpm_environment();
    // solid-refresh really did look merge-anything up, and read the hoisted
    // 6.0.6; vite-plugin-solid read its own 5.1.7. Both are premises now.
    environment
        .push(merge_anything("6.0.6").resolved_from(solid_refresh.as_importer(), "merge-anything"));
    environment.sort();
    crate::contract_certification::validate_dependency_environment(&environment)
        .expect("two copies of one name are expressible with edges");
    assert_eq!(tree.difference(&environment), None);

    // And each copy is still checked from its own importer.
    let mut swapped = pnpm_tree();
    swapped
        .resolves
        .insert(("sr", "merge-anything"), "vps/merge-anything");
    assert_eq!(
        swapped.difference(&environment).as_deref(),
        Some(
            "merge-anything resolved from solid-refresh@0.8.0-next.7 installed 5.1.7, certified 6.0.6"
        )
    );
}

#[test]
fn an_edge_bearing_environment_is_keyed_apart_from_the_same_packages_without_edges() {
    let edged = pnpm_environment();
    let mut strict = edged
        .iter()
        .map(DependencyEnvironmentEntry::without_edge)
        .collect::<Vec<_>>();
    strict.sort();
    assert_ne!(
        environment_acceptance_identity("sha256:artifact", &edged),
        environment_acceptance_identity("sha256:artifact", &strict)
    );
}

/// A consumer tree for ADR 0131, on disk: the certified `plain-package`, the
/// `@solidjs/signals` it resolves, and the `leaf-dep` signals resolves in turn
/// -- plus an `unrelated` package the certification never read. Integrities
/// are registry-shaped (the SHA-512 of each name), which every lockfile reader
/// accepts.
struct PatchTree {
    directory: std::path::PathBuf,
}

const PLAIN_INTEGRITY: &str = "sha512-zN09qfKZ0Rt7CBzVPnscSx/q1ZRTI/cCJ0JKxvJiMU8m4p628jVm+HFVK02ykmfcEJHl7Ar5CaytDctctAmeKg==";
const SIGNALS_INTEGRITY: &str = "sha512-k5Fd5w/wyBqMYIWiZD3WSP75Ijd4fCh5A6qZT+L7KotX02Q3VrgWwaAohd2oh/rLa5m4fTkabZI/NuKDqWa+OA==";
const LEAF_INTEGRITY: &str = "sha512-7l4YxsJgDWOVNoJlPkbDTv4Y2p81Uq3obLYj4PgeA/QaxqJ2kjvNyVKRBsDruFsX6/tYWcFdPcrSGWqo11OrUw==";
const UNRELATED_INTEGRITY: &str = "sha512-z/GtFuGju3yAqg2912t2YS3/bZTyKtVwzNRlgk5n48ICUe5wclrln9wa3y7kXw7NMnbEb1Kgd4JR4se5roHOaQ==";

impl PatchTree {
    fn new(label: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-patched-admission-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        let tree = Self { directory };
        tree.write("package.json", r#"{"name":"consumer","private":true}"#);
        tree.write(
            "node_modules/plain-package/package.json",
            r#"{"name":"plain-package","version":"1.0.0"}"#,
        );
        tree.write(
            "node_modules/plain-package/dist/index.js",
            "export const value = 1;\n",
        );
        tree.write(
            "node_modules/@solidjs/signals/package.json",
            r#"{"name":"@solidjs/signals","version":"2.0.0-rc.6"}"#,
        );
        tree.write(
            "node_modules/leaf-dep/package.json",
            r#"{"name":"leaf-dep","version":"1.0.0"}"#,
        );
        tree.write(
            "node_modules/unrelated/package.json",
            r#"{"name":"unrelated","version":"1.0.0"}"#,
        );
        tree
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.directory.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A pnpm v9 lockfile for the tree, with `patched` (a whole
    /// `patchedDependencies:` block, or empty) spliced in.
    fn pnpm_lock(&self, patched: &str) {
        self.write(
            "pnpm-lock.yaml",
            &format!(
                "lockfileVersion: '9.0'\n\n{patched}importers:\n  .:\n    dependencies:\n      plain-package:\n        specifier: 1.0.0\n        version: 1.0.0\n\npackages:\n  plain-package@1.0.0:\n    resolution: {{integrity: {PLAIN_INTEGRITY}}}\n  '@solidjs/signals@2.0.0-rc.6':\n    resolution: {{integrity: {SIGNALS_INTEGRITY}}}\n  leaf-dep@1.0.0:\n    resolution: {{integrity: {LEAF_INTEGRITY}}}\n  unrelated@1.0.0:\n    resolution: {{integrity: {UNRELATED_INTEGRITY}}}\n"
            ),
        );
    }

    fn npm_lock(&self) {
        self.write(
            "package-lock.json",
            &format!(
                r#"{{"lockfileVersion":3,"packages":{{
                    "node_modules/plain-package":{{"version":"1.0.0","integrity":"{PLAIN_INTEGRITY}"}},
                    "node_modules/@solidjs/signals":{{"version":"2.0.0-rc.6","integrity":"{SIGNALS_INTEGRITY}"}},
                    "node_modules/leaf-dep":{{"version":"1.0.0","integrity":"{LEAF_INTEGRITY}"}},
                    "node_modules/unrelated":{{"version":"1.0.0","integrity":"{UNRELATED_INTEGRITY}"}}}}}}"#
            ),
        );
    }

    fn bun_lock(&self, patched: &str) {
        self.write(
            "bun.lock",
            &format!(
                r#"{{
                  "lockfileVersion": 2,
                  {patched}
                  "packages": {{
                    "plain-package": ["plain-package@1.0.0", "", {{}}, "{PLAIN_INTEGRITY}"],
                    "@solidjs/signals": ["@solidjs/signals@2.0.0-rc.6", "", {{}}, "{SIGNALS_INTEGRITY}"],
                    "leaf-dep": ["leaf-dep@1.0.0", "", {{}}, "{LEAF_INTEGRITY}"],
                    "unrelated": ["unrelated@1.0.0", "", {{}}, "{UNRELATED_INTEGRITY}"],
                  }},
                }}"#
            ),
        );
    }

    /// The environment the certification read, with its lookups: signals from
    /// the certified package, and `leaf-dep` from signals.
    fn environment() -> Vec<DependencyEnvironmentEntry> {
        let signals = entry("@solidjs/signals", "2.0.0-rc.6", SIGNALS_INTEGRITY);
        let mut environment = vec![
            entry("leaf-dep", "1.0.0", LEAF_INTEGRITY)
                .resolved_from(signals.as_importer(), "leaf-dep"),
            signals.resolved_from(EnvironmentImporter::Certified, "@solidjs/signals"),
        ];
        environment.sort();
        environment
    }

    /// The bundle certified in this tree as first written: its signed
    /// snapshot root is the root of the files installed now.
    fn bundle(&self) -> LoadedBundle {
        let environment = Self::environment();
        let (entry, document, receipt) = bundle_in(
            "plain-package",
            "1.0.0",
            PLAIN_INTEGRITY,
            "plain-package",
            &["import"],
            Some(&environment),
        );
        let mut bundle =
            load_bundle(&entry, &document, &receipt).expect("the bundle authenticates");
        bundle.snapshot_root = crate::installed_package_snapshot_root(
            &self.directory.join("node_modules/plain-package"),
            "plain-package",
            "1.0.0",
        )
        .unwrap();
        bundle
    }

    /// Whether the native admission path admits `bundle` here, and the refusal
    /// `contract check` reports when it does not.
    fn admit(&self, bundle: &LoadedBundle) -> Result<(), String> {
        let directory = &self.directory;
        let installed =
            |specifier: &str| crate::diagnostics::installed_artifact_identity(directory, specifier);
        let bytes =
            |specifier: &str| crate::diagnostics::installed_artifact_snapshot(directory, specifier);
        let environment = |specifier: &str, environment: &[DependencyEnvironmentEntry]| {
            crate::diagnostics::installed_environment_matches(directory, specifier, environment)
        };
        let difference = |specifier: &str, environment: &[DependencyEnvironmentEntry]| {
            crate::diagnostics::installed_environment_difference(
                directory,
                specifier,
                environment,
                &crate::installed_patches::InstalledPatches::read(directory),
            )
        };
        let admitted = admitted_from(
            std::slice::from_ref(bundle),
            &BTreeSet::from(["import".to_owned()]),
            &installed,
            &bytes,
            &|_: &str| Some("dist/index.js".to_owned()),
            &environment,
        );
        let refusal = admission_refusals(
            [(
                ArtifactAcceptance {
                    specifier: &bundle.specifier,
                    requested_entrypoint: &bundle.requested_entrypoint,
                    export_conditions: &bundle.export_conditions,
                    runtime_target: &bundle.runtime_target,
                    declaration_target: &bundle.declaration_target,
                    acceptance_root: &bundle.acceptance_root,
                    snapshot_root: &bundle.snapshot_root,
                    environment: bundle.environment.as_deref(),
                    identity: &bundle.identity,
                },
                "1.0.0",
            )],
            &installed,
            &bytes,
            &difference,
        )
        .pop()
        .unwrap()
        .1;
        assert_eq!(
            admitted.is_empty(),
            refusal.is_some(),
            "admission and its report agree: {refusal:?}"
        );
        refusal.map_or(Ok(()), |refusal| Err(refusal.to_string()))
    }
}

impl Drop for PatchTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn a_pnpm_patch_of_an_environment_package_refuses_and_names_the_patch() {
    let tree = PatchTree::new("pnpm");
    tree.pnpm_lock("");
    let bundle = tree.bundle();
    assert_eq!(
        tree.admit(&bundle),
        Ok(()),
        "the unpatched tree is admitted"
    );

    // The lock keeps the published integrity for the patched copy, so without
    // the patch record this tree is indistinguishable from the certified one.
    tree.pnpm_lock(
        "patchedDependencies:\n  '@solidjs/signals@2.0.0-rc.6':\n    hash: 0123abcd\n    path: patches/@solidjs__signals@2.0.0-rc.6.patch\n\n",
    );
    assert_eq!(
        tree.admit(&bundle),
        Err(
            "its dependency environment differs: @solidjs/signals@2.0.0-rc.6 resolved from \
             plain-package@1.0.0 is patched (pnpm-lock.yaml patchedDependencies), so its \
             installed bytes are not the published archive the certification read"
                .into()
        )
    );

    // A patch of a package the certification never read is irrelevant.
    tree.pnpm_lock("patchedDependencies:\n  unrelated@1.0.0: 0123abcd\n\n");
    assert_eq!(tree.admit(&bundle), Ok(()));

    // A patch deeper in the environment -- a package signals resolves -- is a
    // patch of the environment all the same, and the edge is named.
    tree.pnpm_lock("patchedDependencies:\n  leaf-dep@1.0.0: 0123abcd\n\n");
    assert_eq!(
        tree.admit(&bundle),
        Err(
            "its dependency environment differs: leaf-dep@1.0.0 resolved from \
             @solidjs/signals@2.0.0-rc.6 is patched (pnpm-lock.yaml \
             patchedDependencies), so its installed bytes are not the published archive the \
             certification read"
                .into()
        )
    );
}

#[test]
fn a_patched_certified_package_is_refused_by_its_record_or_by_its_bytes() {
    let tree = PatchTree::new("certified");
    tree.pnpm_lock("");
    let bundle = tree.bundle();
    assert_eq!(tree.admit(&bundle), Ok(()));
    tree.pnpm_lock("patchedDependencies:\n  plain-package@1.0.0: 0123abcd\n\n");
    assert_eq!(
        tree.admit(&bundle),
        Err(
            "the installed package's files are not the certified archive's: \
             plain-package@1.0.0 is patched (pnpm-lock.yaml patchedDependencies)"
                .into()
        )
    );
    // No record at all -- a hand edit, a postinstall script -- and the files
    // still are not the signed archive's.
    tree.pnpm_lock("");
    tree.write(
        "node_modules/plain-package/dist/index.js",
        "export const value = 2;\n",
    );
    assert_eq!(
        tree.admit(&bundle),
        Err(
            "the installed package's files are not the certified archive's: they do not \
             reproduce the signed snapshot root, so something changed them after install"
                .into()
        )
    );
    // A dependency nested under the package is not its archive.
    tree.write(
        "node_modules/plain-package/dist/index.js",
        "export const value = 1;\n",
    );
    tree.write(
        "node_modules/plain-package/node_modules/nested/package.json",
        r#"{"name":"nested","version":"1.0.0"}"#,
    );
    assert_eq!(tree.admit(&bundle), Ok(()));
}

#[test]
fn bun_and_patch_package_patches_refuse_the_packages_they_patch_only() {
    let tree = PatchTree::new("bun");
    tree.bun_lock("");
    let bundle = tree.bundle();
    assert_eq!(tree.admit(&bundle), Ok(()));
    tree.bun_lock(
        r#""patchedDependencies": { "@solidjs/signals@2.0.0-rc.6": "patches/signals.patch" },"#,
    );
    let refusal = tree.admit(&bundle).unwrap_err();
    assert!(
        refusal.contains(
            "@solidjs/signals@2.0.0-rc.6 resolved from plain-package@1.0.0 is patched (bun.lock \
             patchedDependencies)"
        ),
        "{refusal}"
    );
    tree.bun_lock(r#""patchedDependencies": { "unrelated@1.0.0": "patches/unrelated.patch" },"#);
    assert_eq!(tree.admit(&bundle), Ok(()));

    // npm has no patch record: patch-package rewrites the files at
    // postinstall, and its patch files are the only trace.
    let tree = PatchTree::new("patch-package");
    tree.npm_lock();
    let bundle = tree.bundle();
    assert_eq!(tree.admit(&bundle), Ok(()));
    tree.write("patches/unrelated+1.0.0.patch", "");
    assert_eq!(tree.admit(&bundle), Ok(()));
    tree.write("patches/leaf-dep+1.0.0.patch", "");
    let refusal = tree.admit(&bundle).unwrap_err();
    assert!(
        refusal.contains(
            "leaf-dep@1.0.0 resolved from @solidjs/signals@2.0.0-rc.6 is patched (patch-package \
             patch patches/leaf-dep+1.0.0.patch)"
        ),
        "{refusal}"
    );
}
