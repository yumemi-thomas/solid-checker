//! Contract requirements for ordinary analysis. Solid core is supplied by the
//! dialect model; this module reads no core contract or receipt documents.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use solid_reactive_ir::{
    contract_semantics::{AcceptedContractIndex, UncertifiableImportReason},
    package_boundary::{
        BindingForm, BoundaryReason, ImportKey, PackageBoundary, UnanalyzableReason,
    },
};

/// Whether `importer` imports `specifier` as a namespace at run time.
fn namespace_import(facts: &solid_facts::ProjectFacts, importer: &str, specifier: &str) -> bool {
    facts
        .files
        .iter()
        .find(|file| file.path.as_str() == importer)
        .is_some_and(|file| {
            file.ast.imports.iter().any(|import| {
                !import.type_only
                    && import.module.as_str() == specifier
                    && import.bindings.iter().any(|binding| {
                        !binding.type_only
                            && binding.kind == solid_facts::ast::ImportKind::Namespace
                    })
            })
        })
}

/// Preserve the existing external-package obligation discovery independently
/// of the historical first-party bundle generator. This index carries only
/// missing-evidence markers; a catalog may fill them with accepted semantics.
pub fn external_package_contract_requirements(
    project: &Path,
    facts: &solid_facts::ProjectFacts,
) -> AcceptedContractIndex {
    requirement_index(&external_package_boundary(project, facts, &[]))
}

/// The contract-era requirement index a boundary's legacy keys produce.
#[must_use]
pub fn requirement_index(boundary: &PackageBoundary) -> AcceptedContractIndex {
    boundary
        .legacy_requirement_index()
        .external_packages()
        .into_owned()
}

/// The package boundary of `facts` (phase P3 of the contract-removal plan):
/// every import the requirement discovery below refuses, with each cause
/// recorded at the decision that found it, and `refusals` -- the source
/// selector's own decisions -- attached to the imports of the copy or name
/// they were about. Its legacy keys are exactly the requirement markers this
/// discovery always raised, and nothing else reaches the acceptance gate:
/// a recorded cause is explanation.
pub fn external_package_boundary(
    project: &Path,
    facts: &solid_facts::ProjectFacts,
    refusals: &[crate::dependency_sources::SourceRefusal],
) -> PackageBoundary {
    let mut boundary = PackageBoundary::default();
    let mut owners = HashMap::new();
    if let Some(imports) = &facts.resolved_imports {
        for (importer, import) in imports.iter() {
            let package = import
                .resolver_package_name
                .as_deref()
                .or(import.package_name.as_deref());
            if package.is_some_and(solid_dialect::primitive_defining_package) {
                continue;
            }
            if is_package_local_edge(&mut owners, importer, import) {
                continue;
            }
            let key = ImportKey::new(importer, import.text.to_string());
            let installed_root = import
                .package_manifest
                .as_deref()
                .and_then(|manifest| Path::new(manifest).parent())
                .map(|root| root.to_string_lossy().into_owned());
            let mut record = |reason: UnanalyzableReason, detail: &str| {
                boundary.record(
                    key.clone(),
                    package,
                    installed_root.as_deref(),
                    BoundaryReason::new(reason, detail),
                );
            };
            // ADR 0284: a package analyzed as source needs no contract, when
            // the import is one its redirects bind. A namespace import is not
            // redirected, so its members stay the declarations' and the
            // import keeps the obligation (review round 1 of ADR 0290, Sol).
            if facts
                .dependency_source_root(&import.resolved_path)
                .is_some()
            {
                if !namespace_import(facts, importer, &import.text) {
                    continue;
                }
                record(
                    UnanalyzableReason::UnsupportedBinding(BindingForm::NamespaceValue),
                    "a runtime namespace import of selected source is not redirected",
                );
            }
            if import.resolution == solid_facts::ImportResolution::Unresolved
                && local_proposal_exists(project, &import.text)
            {
                record(
                    UnanalyzableReason::ResolutionUnavailable,
                    "the import resolves to no file",
                );
                record(
                    UnanalyzableReason::LegacyContractFallback,
                    "a project-local contract proposal names this package",
                );
                boundary.require_legacy(key, UncertifiableImportReason::Unspecified);
                continue;
            }
            if package.is_some()
                && import.resolution == solid_facts::ImportResolution::NodeModules
                && import
                    .package_manifest
                    .as_deref()
                    .is_some_and(manifest_uses_solid)
            {
                record(
                    UnanalyzableReason::LegacyContractFallback,
                    "an installed package whose manifest names a Solid package",
                );
                let name = crate::dependency_sources::package_name(&import.text);
                for refusal in refusals.iter().filter(|refusal| match &refusal.root {
                    Some(root) => Path::new(import.resolved_path.as_ref()).starts_with(root),
                    None => {
                        name.as_deref() == Some(refusal.name.as_str())
                            || package == Some(refusal.name.as_str())
                    }
                }) {
                    record(refusal.kind, &refusal.detail);
                }
                boundary.require_legacy(key, UncertifiableImportReason::Unspecified);
            }
        }
    }
    boundary
}

/// Whether `import` is a relative edge between two modules of one package
/// installation, which is no package boundary and so needs no package
/// contract (ADR 0132).
///
/// The resolver reports a relative specifier as `NodeModules` whenever the
/// path it lands on lies under a `node_modules` directory. That is true of
/// every module of an installed package, so without this test each relative
/// import *inside* the package under analysis reads as an import of an
/// external Solid-using package with no accepted contract. The obligation it
/// raises sits at module level, outside every function, and attribution marks
/// every export of the entrypoint with it. A package generated from
/// `node_modules` therefore lost every domain on every export, and the same
/// bytes generated from anywhere else did not.
///
/// Exact, not path-shaped: the specifier must be relative, and the importer
/// and the resolved file must have the *same* owning manifest -- the nearest
/// `package.json` above each that declares a `name`. An unnamed nested
/// manifest (`dist/esm/package.json` with only `"type"`) owns nothing. A
/// relative path that climbs into another installation, a nested copy of the
/// same package, or an unresolved or unreadable manifest keeps the marker.
fn is_package_local_edge(
    owners: &mut HashMap<PathBuf, Option<PathBuf>>,
    importer: &str,
    import: &solid_facts::AttestedImport,
) -> bool {
    if !(import.text.starts_with("./") || import.text.starts_with("../")) {
        return false;
    }
    if import.resolved_path.is_empty() {
        return false;
    }
    let Some(importer_owner) = owning_named_manifest(owners, Path::new(importer)) else {
        return false;
    };
    owning_named_manifest(owners, Path::new(import.resolved_path.as_ref()))
        .is_some_and(|target_owner| target_owner == importer_owner)
}

/// The nearest `package.json` above `file` whose object declares a string
/// `name`, memoized per directory.
fn owning_named_manifest(
    owners: &mut HashMap<PathBuf, Option<PathBuf>>,
    file: &Path,
) -> Option<PathBuf> {
    let mut visited = Vec::new();
    let mut answer = None;
    for directory in file.ancestors().skip(1) {
        if let Some(known) = owners.get(directory) {
            answer = known.clone();
            break;
        }
        visited.push(directory.to_path_buf());
        let manifest = directory.join("package.json");
        let named = fs::read(&manifest)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .is_some_and(|value| value.get("name").is_some_and(serde_json::Value::is_string));
        if named {
            answer = Some(manifest);
            break;
        }
    }
    for directory in visited {
        owners.insert(directory, answer.clone());
    }
    answer
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
