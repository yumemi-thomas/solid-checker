//! Package requirements for ordinary analysis. Solid core is supplied by the
//! dialect model; this module reads no core package documents.

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use solid_reactive_ir::package_boundary::{
    BindingForm, BoundaryReason, ImportKey, PackageBoundary, UnanalyzableReason,
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

/// ADR 0322: whether every runtime namespace import of `specifier` in
/// `importer` is used only through static members, each of which a redirect
/// binds to the selected runtime source.
fn namespace_import_redirected(
    facts: &solid_facts::ProjectFacts,
    importer: &str,
    specifier: &str,
) -> bool {
    let Some(file) = facts
        .files
        .iter()
        .find(|file| file.path.as_str() == importer)
    else {
        return false;
    };
    let aliases = crate::source_redirects::canonical_symbol_aliases(facts);
    let entities = facts
        .typescript
        .entities()
        .filter(|entity| entity.location.path.as_ref() == importer)
        .map(|entity| {
            (
                (entity.location.start_byte, entity.location.end_byte),
                entity,
            )
        })
        .collect::<HashMap<_, _>>();
    file.ast
        .imports
        .iter()
        .filter(|import| !import.type_only && import.module.as_str() == specifier)
        .flat_map(|import| import.bindings.iter())
        .filter(|binding| {
            !binding.type_only && binding.kind == solid_facts::ast::ImportKind::Namespace
        })
        .all(|binding| {
            let Some(members) =
                crate::source_redirects::namespace_static_members(file, binding.local.span)
            else {
                return false;
            };
            // Each reference names its member at two spans (the property and
            // the whole member, or a tag's property and name); one of them must
            // carry an entity a redirect binds. At the whole span only a
            // covering symbol is the member: the leading token is the namespace
            // (ADR 0376).
            members.chunks(2).all(|pair| {
                pair.iter().enumerate().any(|(index, (span, _))| {
                    entities
                        .get(&(u64::from(span.start), u64::from(span.end)))
                        .filter(|entity| index == 0 || entity.symbol_covers_span())
                        .is_some_and(|entity| {
                            facts.runtime_symbol_redirects.contains_key(
                                &crate::source_redirects::canonical_symbol(
                                    &entity.symbol,
                                    &aliases,
                                ),
                            )
                        })
                })
            })
        })
}

/// The package boundary of `facts`: every import the requirement discovery
/// below refuses, with each cause recorded at the decision that found it, and
/// `refusals` -- the source selector's own decisions -- attached to the
/// imports of the copy or name they were about. Its legacy keys are exactly
/// the requirement markers this discovery always raised, and nothing else
/// reaches the boundary defects: a recorded cause is explanation.
pub fn external_package_boundary(
    facts: &solid_facts::ProjectFacts,
    refusals: &[crate::dependency_sources::SourceRefusal],
) -> PackageBoundary {
    external_package_boundary_with(&DiskReader, facts, refusals)
}

/// The package boundary of a request that supplies no package files and no
/// import attestation -- the WASM `checkSync` host protocol. Nothing in such a
/// request shows what a package
/// does or which installation a specifier names, so every runtime import or
/// re-export of a bare specifier, Solid core excepted, keeps an open
/// requirement. `node:` built-ins are not packages.
#[must_use]
pub fn unsupplied_package_boundary(facts: &solid_facts::ProjectFacts) -> PackageBoundary {
    let mut boundary = PackageBoundary::default();
    for file in &facts.files {
        let imports = file
            .ast
            .imports
            .iter()
            .filter(|import| !import.type_only)
            .map(|import| import.module.as_str());
        let reexports = file
            .ast
            .exports
            .iter()
            .filter(|export| !export.type_only)
            .filter_map(|export| export.module.as_deref());
        for specifier in imports.chain(reexports) {
            let Some(name) = crate::dependency_sources::package_name(specifier) else {
                continue;
            };
            if name.contains(':') || solid_dialect::primitive_defining_package(&name) {
                continue;
            }
            let key = ImportKey::new(file.path.as_str(), specifier);
            boundary.record(
                key.clone(),
                Some(&name),
                None,
                BoundaryReason::new(
                    UnanalyzableReason::ResolutionUnavailable,
                    "the request supplies no package files or import resolution",
                ),
            );
            boundary.require_legacy(key);
        }
    }
    boundary
}

/// The filesystem lookups the discovery below makes, and nothing else. A read
/// error is an answer the discovery folds into "no manifest" at every site, so
/// it returns the `io::Result` untouched and the boundary stays infallible.
trait ManifestReader {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
}

/// The operational reader: the real filesystem.
struct DiskReader;

impl ManifestReader for DiskReader {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }
}

fn external_package_boundary_with(
    reader: &dyn ManifestReader,
    facts: &solid_facts::ProjectFacts,
    refusals: &[crate::dependency_sources::SourceRefusal],
) -> PackageBoundary {
    let mut boundary = PackageBoundary::default();
    let mut owners = HashMap::new();
    if let Some(imports) = &facts.resolved_imports {
        let imported: HashSet<&str> = imports
            .iter()
            .map(|(_, import)| import.resolved_path.as_ref())
            .collect();
        for (importer, import) in imports.iter() {
            // Vite loads its configuration in Node while building; the file is
            // no part of the program the browser runs, so what its imports
            // do cannot reach a reactive read, an owner or a callback of that
            // program. Only a file nothing in the program imports is exempt:
            // an importer would make the config's exports part of that program.
            if is_build_tool_config(importer) && !imported.contains(importer) {
                continue;
            }
            let package = import
                .resolver_package_name
                .as_deref()
                .or(import.package_name.as_deref());
            if package.is_some_and(solid_dialect::primitive_defining_package) {
                continue;
            }
            if is_package_local_edge(reader, &mut owners, importer, import)
                || is_workspace_project_edge(facts, import)
            {
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
            // ADR 0284: a package analyzed as source is answered by its
            // source, when the import is one its redirects bind. A namespace
            // import is not redirected, so its members stay the declarations'
            // and the import keeps the obligation (review round 1 of ADR 0290,
            // Sol).
            if facts
                .dependency_source_root(&import.resolved_path)
                .is_some()
            {
                if !namespace_import(facts, importer, &import.text)
                    || namespace_import_redirected(facts, importer, &import.text)
                {
                    continue;
                }
                record(
                    UnanalyzableReason::UnsupportedBinding(BindingForm::NamespaceValue),
                    "a runtime namespace import of selected source is not redirected",
                );
            }
            if package.is_some()
                && import.resolution == solid_facts::ImportResolution::NodeModules
                && import
                    .package_manifest
                    .as_deref()
                    .is_some_and(|manifest| manifest_uses_solid(reader, manifest))
            {
                record(
                    UnanalyzableReason::LegacyRequirementFallback,
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
                boundary.require_legacy(key);
            }
        }
    }
    boundary
}

/// Whether `import` is a relative edge between two modules of one package
/// installation, which is no package boundary and so needs no package
/// requirement (ADR 0132).
///
/// The resolver reports a relative specifier as `NodeModules` whenever the
/// path it lands on lies under a `node_modules` directory. That is true of
/// every module of an installed package, so without this test each relative
/// import *inside* the package under analysis reads as an import of an
/// external Solid-using package with no analyzed source. The obligation it
/// raises sits at module level, outside every function, and attribution marks
/// every export of the entrypoint with it. A package analyzed from
/// `node_modules` therefore lost every domain on every export, and the same
/// bytes analyzed from anywhere else did not.
///
/// Exact, not path-shaped: the specifier must be relative, and the importer
/// and the resolved file must have the *same* owning manifest -- the nearest
/// `package.json` above each that declares a `name`. An unnamed nested
/// manifest (`dist/esm/package.json` with only `"type"`) owns nothing. A
/// relative path that climbs into another installation, a nested copy of the
/// same package, or an unresolved or unreadable manifest keeps the marker.
fn is_package_local_edge(
    reader: &dyn ManifestReader,
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
    let Some(importer_owner) = owning_named_manifest(reader, owners, Path::new(importer)) else {
        return false;
    };
    owning_named_manifest(reader, owners, Path::new(import.resolved_path.as_ref()))
        .is_some_and(|target_owner| target_owner == importer_owner)
}

/// Whether `import` is a bare specifier that a workspace link resolved to a
/// file of this project's own program: no package boundary, because the
/// target is project code, analyzed like any other project file. The
/// dependency selection reads such a root the same way ("it is project code,
/// not a dependency"), so a requirement marker raised here claimed that the
/// behaviour of a callee was unknown while its body sat in the facts.
///
/// Exact: the resolver's realpath-normalized file lies under no `node_modules`
/// directory, and the program holds facts for exactly that file. A file the
/// program does not hold (a declaration-only or unselected target) and an
/// installed copy under `node_modules` keep the marker.
fn is_workspace_project_edge(
    facts: &solid_facts::ProjectFacts,
    import: &solid_facts::AttestedImport,
) -> bool {
    let target = import.resolved_path.as_ref();
    !target.is_empty()
        && import.resolution == solid_facts::ImportResolution::NodeModules
        && !Path::new(target)
            .components()
            .any(|component| component.as_os_str() == "node_modules")
        && facts.dependency_source_root(target).is_none()
        && facts.files.iter().any(|file| file.path.as_str() == target)
}

/// Whether `path` is a file Vite itself resolves as its configuration
/// (`vite.config.<ext>`, and `vitest.config.<ext>` for the test runner built on
/// it): the exact names those tools document, not a pattern guessed from a
/// package. [`crate::dependency_sources::is_build_config`] is the prefix test;
/// the extension must also be one of the script extensions, so a helper that
/// merely starts with the name is not a configuration.
fn is_build_tool_config(path: &str) -> bool {
    let path = Path::new(path);
    crate::dependency_sources::is_build_config(path)
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(extension, "js" | "mjs" | "cjs" | "ts" | "mts" | "cts")
            })
        && path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| matches!(stem, "vite.config" | "vitest.config"))
}

/// The nearest `package.json` above `file` whose object declares a string
/// `name`, memoized per directory.
fn owning_named_manifest(
    reader: &dyn ManifestReader,
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
        let named = reader
            .read(&manifest)
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

fn manifest_uses_solid(reader: &dyn ManifestReader, path: &str) -> bool {
    reader
        .read(Path::new(path))
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

#[cfg(test)]
mod tests;
