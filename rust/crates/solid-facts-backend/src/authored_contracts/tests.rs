use std::collections::BTreeSet;

use super::*;

const SNAPSHOT: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000003";

/// The certified rc.4 router document the tier carries for `browser`/`import`,
/// read from disk, as the pilot's authored documents start from (ADR 0198 § 6).
fn router_document() -> (serde_json::Value, Vec<u8>) {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../pkg/contracts/accepted");
    let index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("index.json")).unwrap()).unwrap();
    let bundle = index["bundles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bundle| {
            bundle["packageName"] == "@tanstack/solid-router"
                && bundle["packageVersion"] == "2.0.0-rc.4"
                && bundle["exportConditions"] == serde_json::json!(["browser", "import"])
                && bundle["runtimeTarget"] == "dist/esm/index.js"
        })
        .expect("the tier carries the rc.4 router browser case")
        .clone();
    let document = std::fs::read(root.join(bundle["document"].as_str().unwrap())).unwrap();
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
