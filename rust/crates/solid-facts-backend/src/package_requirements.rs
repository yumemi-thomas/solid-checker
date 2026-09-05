//! Contract requirements for ordinary analysis. Solid core is supplied by the
//! dialect model; this module reads no core contract or receipt documents.

use std::{collections::BTreeSet, fs, path::Path};

use solid_reactive_ir::contract_semantics::AcceptedContractIndex;

/// Preserve the existing external-package obligation discovery independently
/// of the historical first-party bundle generator. This index carries only
/// missing-evidence markers; a catalog may fill them with accepted semantics.
pub fn external_package_contract_requirements(
    dialect_id: &str,
    project: &Path,
    facts: &solid_facts::ProjectFacts,
) -> AcceptedContractIndex {
    let mut missing = BTreeSet::new();
    if let Some(imports) = &facts.resolved_imports {
        for (importer, import) in imports.iter() {
            let package = import
                .resolver_package_name
                .as_deref()
                .or(import.package_name.as_deref());
            if package.is_some_and(solid_dialect::primitive_defining_package) {
                continue;
            }
            if import.resolution == solid_facts::ImportResolution::Unresolved
                && local_proposal_exists(project, &import.text)
            {
                missing.insert((importer.to_owned(), import.text.to_string()));
                continue;
            }
            // Preserve the previous discovery behavior for these external
            // Solid 1 helpers. Their catalog/refusal entries still apply;
            // changing their obligation discovery is a separate migration.
            if dialect_id == "solid-v1"
                && matches!(
                    package,
                    Some(
                        "@solid-primitives/scheduled"
                            | "@solid-primitives/debounce"
                            | "@solid-primitives/rootless"
                    )
                )
            {
                continue;
            }
            if package.is_some()
                && import.resolution == solid_facts::ImportResolution::NodeModules
                && import
                    .package_manifest
                    .as_deref()
                    .is_some_and(manifest_uses_solid)
            {
                missing.insert((importer.to_owned(), import.text.to_string()));
            }
        }
    }
    AcceptedContractIndex::default()
        .with_uncertifiable_imports(missing)
        .external_packages()
        .into_owned()
}

fn local_proposal_exists(project: &Path, specifier: &str) -> bool {
    let mut segments = specifier.split('/');
    let Some(first) = segments.next() else {
        return false;
    };
    let package = if first.starts_with('@') {
        let Some(second) = segments.next() else {
            return false;
        };
        format!("{first}/{second}")
    } else {
        first.to_owned()
    };
    project
        .join(".solid-checker/contracts")
        .join(package)
        .join("solid-reactivity.json")
        .is_file()
}

fn manifest_uses_solid(path: &str) -> bool {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|manifest| {
            ["dependencies", "peerDependencies", "optionalDependencies"]
                .into_iter()
                .filter_map(|field| manifest[field].as_object())
                .flat_map(|dependencies| dependencies.keys())
                .any(|name| name == "solid-js" || name.starts_with("@solidjs/"))
        })
}
