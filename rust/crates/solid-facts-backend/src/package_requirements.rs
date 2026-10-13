//! Contract requirements for ordinary analysis. Solid core is supplied by the
//! dialect model; this module reads no core contract or receipt documents.

use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

use solid_reactive_ir::contract_semantics::AcceptedContractIndex;

/// Preserve the existing external-package obligation discovery independently
/// of the historical first-party bundle generator. This index carries only
/// missing-evidence markers; a catalog may fill them with accepted semantics.
pub fn external_package_contract_requirements(
    project: &Path,
    facts: &solid_facts::ProjectFacts,
) -> AcceptedContractIndex {
    let mut missing = BTreeSet::new();
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
            if import.resolution == solid_facts::ImportResolution::Unresolved
                && local_proposal_exists(project, &import.text)
            {
                missing.insert((importer.to_owned(), import.text.to_string()));
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
