use std::collections::BTreeSet;

use super::*;

const SNAPSHOT: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000003";

/// The rc.4 router's `browser`/`import` identity case, as the authored spec's
/// identity.json carries it (ADR 0207), and its document's bytes.
fn router_document() -> (serde_json::Value, Vec<u8>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../pkg/contracts/authored/specs/@tanstack+solid-router@2.0.0-rc.4/identity.json",
    );
    let identity: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut bundle = identity["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| {
            case["exportConditions"] == serde_json::json!(["browser", "import"])
                && case["runtimeTarget"] == "dist/esm/index.js"
        })
        .expect("the spec's identity carries the rc.4 router browser case")
        .clone();
    let document = serde_json::to_vec_pretty(&bundle["identityDocument"]).unwrap();
    bundle["packageName"] = "@tanstack/solid-router".into();
    bundle["packageVersion"] = "2.0.0-rc.4".into();
    (bundle, document)
}

fn runtime(version: &str, integrity: &str) -> Vec<SolidRuntimeEntry> {
    ["solid-js", "@solidjs/signals", "@solidjs/web"]
        .into_iter()
        .map(|name| SolidRuntimeEntry {
            name: name.to_owned(),
            version: version.to_owned(),
            integrity: format!("{integrity}-{name}"),
        })
        .collect()
}

fn entry(bundle: &serde_json::Value, document: &[u8]) -> AuthoredEntry {
    AuthoredEntry {
        package_name: bundle["packageName"].as_str().unwrap().to_owned(),
        specifier: bundle["specifier"].as_str().unwrap().to_owned(),
        package_version: bundle["packageVersion"].as_str().unwrap().to_owned(),
        package_integrity: bundle["packageIntegrity"].as_str().unwrap().to_owned(),
        requested_entrypoint: bundle["requestedEntrypoint"].as_str().unwrap().to_owned(),
        export_conditions: vec!["browser".to_owned(), "import".to_owned()],
        runtime_target: bundle["runtimeTarget"].as_str().unwrap().to_owned(),
        declaration_target: bundle["declarationTarget"].as_str().unwrap().to_owned(),
        snapshot_root: SNAPSHOT.to_owned(),
        patched_install: false,
        solid_runtime: runtime("2.0.0-rc.13", "sha512-rc13"),
        document: "objects/router.json".to_owned(),
        document_digest: crate::contract_interface::sha256_digest(document),
        premises: Vec::new(),
    }
}

#[test]
fn an_authored_document_loads_with_its_own_identity_and_no_receipt() {
    let (bundle, document) = router_document();
    let authored = load_authored(&entry(&bundle, &document), &document).expect("loads");
    let receipt = authored.contract.receipt();
    assert_eq!(
        receipt.verifier.policy,
        solid_reactive_ir::contract_semantics::proof::AUTHORED_POLICY
    );
    assert!(receipt.authentication.is_none());
    assert_eq!(
        receipt.proof_root.as_str(),
        crate::contract_interface::sha256_digest(&document)
    );
    assert!(authored.identity.starts_with("authored:"));
    assert_eq!(authored.environment.len(), 3);
    // The document is the router's, so it describes `useSearch`.
    assert!(authored.contract.export("useSearch").is_some());
}

#[test]
fn an_authored_document_is_refused_when_its_index_entry_lies() {
    let (bundle, document) = router_document();
    let mut wrong_digest = entry(&bundle, &document);
    wrong_digest.document_digest =
        "sha256:1111111111111111111111111111111111111111111111111111111111111111".into();
    assert!(load_authored(&wrong_digest, &document).is_err(), "digest");
    let mut wrong_version = entry(&bundle, &document);
    wrong_version.package_version = "2.0.0-rc.5".into();
    assert!(load_authored(&wrong_version, &document).is_err(), "version");
    let mut no_runtime = entry(&bundle, &document);
    no_runtime.solid_runtime.clear();
    assert!(load_authored(&no_runtime, &document).is_err(), "no runtime");
    let mut foundation = entry(&bundle, &document);
    foundation.package_name = "solid-js".into();
    foundation.specifier = "solid-js".into();
    assert!(
        load_authored(&foundation, &document).is_err(),
        "the runtime foundation"
    );
}

#[test]
fn an_authored_contract_is_admitted_on_package_bytes_and_its_solid_runtime() {
    let (bundle, document) = router_document();
    let loaded = [load_authored(&entry(&bundle, &document), &document).unwrap()];
    let identity = loaded[0].identity.clone();
    let conditions = BTreeSet::from(["browser".to_owned(), "import".to_owned()]);
    let resolved = |_: &str| Some("dist/esm/index.d.ts".to_owned());
    let installed = |_: &str| {
        Some((
            "@tanstack/solid-router".to_owned(),
            "2.0.0-rc.4".to_owned(),
            bundle["packageIntegrity"].as_str().unwrap().to_owned(),
        ))
    };
    let published = |_: &str, _: bool| -> Result<String, String> { Ok(SNAPSHOT.to_owned()) };
    let admit = |runtime: Vec<SolidRuntimeEntry>,
                 bytes: &InstalledArtifactBytes,
                 installed: &InstalledArtifactIdentity| {
        let expected = runtime
            .into_iter()
            .map(|entry| {
                DependencyEnvironmentEntry::package(entry.name, entry.version, entry.integrity)
            })
            .collect::<Vec<_>>();
        // Only the Solid runtime is ever asked about: the environment handed
        // to the tree is exactly the stated runtime entries.
        let environment = move |specifier: &str, environment: &[DependencyEnvironmentEntry]| {
            assert_eq!(specifier, "@tanstack/solid-router");
            environment == expected.as_slice()
        };
        admitted_from(
            &loaded,
            &conditions,
            installed,
            bytes,
            &resolved,
            &environment,
        )
    };

    assert_eq!(
        admit(
            runtime("2.0.0-rc.13", "sha512-rc13"),
            &published,
            &installed
        ),
        vec![("@tanstack/solid-router".to_owned(), identity)],
        "the probed runtime and the published bytes"
    );
    assert!(
        admit(runtime("2.0.0-rc.9", "sha512-rc9"), &published, &installed).is_empty(),
        "another Solid runtime"
    );
    let patched = |_: &str, _: bool| -> Result<String, String> { Err("patched".to_owned()) };
    assert!(
        admit(runtime("2.0.0-rc.13", "sha512-rc13"), &patched, &installed).is_empty(),
        "other installed bytes"
    );
    let other_integrity = |_: &str| {
        Some((
            "@tanstack/solid-router".to_owned(),
            "2.0.0-rc.4".to_owned(),
            "sha512-republished".to_owned(),
        ))
    };
    assert!(
        admit(
            runtime("2.0.0-rc.13", "sha512-rc13"),
            &published,
            &other_integrity
        )
        .is_empty(),
        "another integrity"
    );
}

/// ADR 0208: an entry probed on a patched install admits that install by its
/// snapshot root, and only an entry that states so asks past the patch.
#[test]
fn a_patched_install_is_admitted_only_by_an_entry_probed_on_it() {
    let (bundle, document) = router_document();
    let conditions = BTreeSet::from(["browser".to_owned(), "import".to_owned()]);
    let resolved = |_: &str| Some("dist/esm/index.d.ts".to_owned());
    let installed = |_: &str| {
        Some((
            "@tanstack/solid-router".to_owned(),
            "2.0.0-rc.4".to_owned(),
            bundle["packageIntegrity"].as_str().unwrap().to_owned(),
        ))
    };
    let expected = runtime("2.0.0-rc.13", "sha512-rc13")
        .into_iter()
        .map(|entry| {
            DependencyEnvironmentEntry::package(entry.name, entry.version, entry.integrity)
        })
        .collect::<Vec<_>>();
    let environment = move |_: &str, environment: &[DependencyEnvironmentEntry]| {
        environment == expected.as_slice()
    };
    // The tree records a patch: its files answer only a caller that admits one.
    let patched_tree = |_: &str, admits_patch: bool| -> Result<String, String> {
        if admits_patch {
            Ok(SNAPSHOT.to_owned())
        } else {
            Err("patched".to_owned())
        }
    };
    let admit = |entry: AuthoredEntry, bytes: &InstalledArtifactBytes| {
        let loaded = [load_authored(&entry, &document).unwrap()];
        admitted_from(
            &loaded,
            &conditions,
            &installed,
            bytes,
            &resolved,
            &environment,
        )
    };

    assert!(
        admit(entry(&bundle, &document), &patched_tree).is_empty(),
        "an entry about the published bytes"
    );
    let mut probed_on_patch = entry(&bundle, &document);
    probed_on_patch.patched_install = true;
    assert_eq!(
        admit(probed_on_patch.clone(), &patched_tree).len(),
        1,
        "the patched install it was probed on"
    );
    // Another patch: the root no longer matches.
    let other_patch =
        |_: &str, _: bool| -> Result<String, String> { Ok("sha256:other".to_owned()) };
    assert!(
        admit(probed_on_patch, &other_patch).is_empty(),
        "another patch"
    );
}

#[test]
fn the_shipped_index_loads() {
    authored().expect("the compiled-in authored index loads");
}

#[test]
fn router_getter_returns_are_premised_and_optional_callbacks_remain_open() {
    use solid_reactive_ir::contract_semantics::{
        ClaimDomain, Event, Schedule, Trigger, ValueShape,
    };

    let entries = authored().expect("the compiled-in authored index loads");
    let mut versions = BTreeSet::new();
    for loaded in entries.iter().filter(|loaded| {
        loaded.entry.package_name == "@solidjs/router" && loaded.entry.requested_entrypoint == "."
    }) {
        versions.insert(loaded.entry.package_version.as_str());
        let location = loaded.contract.export("useLocation").unwrap();
        let returns = location.operation_claim(ClaimDomain::Returns).unwrap();
        if loaded.entry.package_version == "2.0.0-next.26" {
            let withheld = loaded
                .withheld
                .as_ref()
                .expect("premised location has an open weakening");
            let withheld_returns = withheld
                .export("useLocation")
                .unwrap()
                .operation_claim(ClaimDomain::Returns)
                .unwrap();
            assert!(!withheld_returns.is_closed());
            assert!(
                withheld_returns.items().is_empty(),
                "veto withholds every getter and data member positive"
            );
            assert_ne!(
                loaded.contract.semantic_identity(),
                withheld.semantic_identity()
            );
            assert!(returns.is_closed());
            assert_eq!(returns.items().len(), 1);
            let output = location
                .operation(&returns.items()[0].0)
                .unwrap()
                .output
                .as_ref()
                .unwrap();
            let ValueShape::Object(properties) = output else {
                panic!("plain location object");
            };
            assert!(properties.is_closed());
            assert_eq!(properties.items().len(), 6);
            for name in ["pathname", "search", "hash", "state"] {
                assert_eq!(
                    properties
                        .items()
                        .iter()
                        .find(|member| member.name == name)
                        .unwrap()
                        .value,
                    ValueShape::Getter { reactive: true }
                );
            }
            assert_eq!(
                properties
                    .items()
                    .iter()
                    .find(|member| member.name == "key")
                    .unwrap()
                    .value,
                ValueShape::Getter { reactive: false }
            );
            assert!(matches!(
                properties
                    .items()
                    .iter()
                    .find(|member| member.name == "query")
                    .unwrap()
                    .value,
                ValueShape::Store { .. }
            ));
            for name in ["useHref", "useResolvedPath"] {
                let hook = loaded.contract.export(name).unwrap();
                assert!(!hook.callbacks().is_closed(), "{name}");
                assert_eq!(hook.callbacks().items().len(), 1, "initial only: {name}");
                let callback = &hook.callbacks().items()[0];
                let operation = hook.operation(&callback.operation.0).unwrap();
                assert_eq!(
                    operation.trigger,
                    Some(Trigger::Event(Event::Call)),
                    "{name}"
                );
                assert_eq!(operation.at, Some(Event::Call), "{name}");
                assert_eq!(operation.schedule, Some(Schedule::SameStack), "{name}");
                assert_eq!(operation.cardinality.min, Some(0), "{name}");
                assert!(
                    hook.operation_claim(ClaimDomain::Returns)
                        .unwrap()
                        .is_closed(),
                    "{name}"
                );
            }
        } else {
            assert!(!returns.is_closed());
            assert!(
                returns.items().is_empty(),
                "older context substitution stays open"
            );
        }
    }
    assert_eq!(
        versions,
        BTreeSet::from([
            "2.0.0-next.16",
            "2.0.0-next.17",
            "2.0.0-next.18",
            "2.0.0-next.26"
        ])
    );
}

#[test]
fn integration_veto_opens_only_premised_exports_and_changes_semantic_identity() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/package-contracts/standard-router-integration");
    let bytes = std::fs::read(fixture.join("solid-reactivity.json")).unwrap();
    let identity: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            root.join("pkg/contracts/authored/specs/@solidjs+router@2.0.0-next.26/identity.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut bundle = identity["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| {
            case["requestedEntrypoint"] == "."
                && case["exportConditions"] == serde_json::json!(["browser", "import"])
        })
        .unwrap()
        .clone();
    bundle["packageName"] = "@solidjs/router".into();
    bundle["packageVersion"] = "2.0.0-next.26".into();
    let mut entry = entry(&bundle, &bytes);
    entry.premises = vec![
        serde_json::from_slice(&std::fs::read(fixture.join("premise.json")).unwrap()).unwrap(),
    ];
    let loaded = load_authored(&entry, &bytes).unwrap();
    let open = loaded.withheld.as_ref().unwrap();
    for name in ["useLocation", "useParams"] {
        assert_ne!(loaded.contract.export(name), open.export(name), "{name}");
    }
    assert_eq!(
        loaded.contract.export("defineRoutes"),
        open.export("defineRoutes")
    );
    assert_ne!(
        loaded.contract.receipt().closed_claims_root,
        open.receipt().closed_claims_root
    );
    assert_ne!(
        loaded.contract.semantic_identity(),
        open.semantic_identity()
    );
    for invalid in [
        serde_json::json!({"name":"","exports":["useLocation"],"valueReferences":[],"configCalls":[]}),
        serde_json::json!({"name":"p","exports":["absent"],"valueReferences":[],"configCalls":[]}),
        serde_json::json!({"name":"p","exports":["useLocation","useLocation"],"valueReferences":[{"path":"dist/routing.d.ts","start":1101,"end":1117}],"configCalls":[]}),
        serde_json::json!({"name":"p","exports":["useLocation"],"valueReferences":[{"path":"dist\\routing.d.ts","start":1101,"end":1117}],"configCalls":[]}),
    ] {
        entry.premises = vec![serde_json::from_value(invalid).unwrap()];
        assert!(load_authored(&entry, &bytes).is_err());
    }
}

/// The triples an index names, extracted independently of `AuthoredIndex`.
fn independent_triples(index: &[u8]) -> BTreeSet<(String, String, String)> {
    let value: serde_json::Value = serde_json::from_slice(index).unwrap();
    value["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let field = |name: &str| entry[name].as_str().unwrap().to_owned();
            (
                field("specifier"),
                field("packageName"),
                field("packageVersion"),
            )
        })
        .collect()
}

#[test]
fn coverage_is_exactly_the_embedded_index_triples() {
    let coverage = authored_coverage_index(embedded_index()).unwrap();
    let unique = coverage.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(unique, independent_triples(embedded_index()));
    assert_eq!(
        coverage.len(),
        serde_json::from_slice::<serde_json::Value>(embedded_index()).unwrap()["entries"]
            .as_array()
            .unwrap()
            .len()
    );
    assert!(unique.contains(&(
        "@tanstack/virtual-core".to_owned(),
        "@tanstack/virtual-core".to_owned(),
        "3.17.8".to_owned()
    )));
}

/// Coverage reads no document object: an index whose objects this build does
/// not carry, or whose digests match nothing, answers the same triples, and
/// there is no object accessor to call.
#[test]
fn coverage_never_reads_a_document_object() {
    let mut value: serde_json::Value = serde_json::from_slice(embedded_index()).unwrap();
    for entry in value["entries"].as_array_mut().unwrap() {
        entry["document"] = "objects/absent.json".into();
        entry["documentDigest"] = "sha256:00".into();
    }
    let altered = serde_json::to_vec(&value).unwrap();
    assert!(
        load_index(&altered, |_| None).is_err(),
        "the semantic loader needs the objects"
    );
    assert_eq!(
        authored_coverage_index(&altered).unwrap(),
        authored_coverage_index(embedded_index()).unwrap()
    );
}

#[test]
fn malformed_index_metadata_refuses() {
    let mut wrong_format: serde_json::Value = serde_json::from_slice(embedded_index()).unwrap();
    wrong_format["format"] = 2.into();
    let mut unknown_field: serde_json::Value = serde_json::from_slice(embedded_index()).unwrap();
    unknown_field["entries"][0]["unexpected"] = true.into();
    let mut missing_field: serde_json::Value = serde_json::from_slice(embedded_index()).unwrap();
    missing_field["entries"][0]
        .as_object_mut()
        .unwrap()
        .remove("packageVersion");
    let mut core: serde_json::Value = serde_json::from_slice(embedded_index()).unwrap();
    core["entries"][0]["packageName"] = "solid-js".into();
    core["entries"][0]["specifier"] = "solid-js".into();
    for bytes in [
        b"{".to_vec(),
        serde_json::to_vec(&wrong_format).unwrap(),
        serde_json::to_vec(&unknown_field).unwrap(),
        serde_json::to_vec(&missing_field).unwrap(),
        serde_json::to_vec(&core).unwrap(),
    ] {
        assert!(authored_coverage_index(&bytes).is_err());
    }
}
