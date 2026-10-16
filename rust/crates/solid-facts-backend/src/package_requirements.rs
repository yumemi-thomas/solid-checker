//! Contract requirements for ordinary analysis. Solid core is supplied by the
//! dialect model; this module reads no core contract or receipt documents.

use std::{
    collections::HashMap,
    fs, io,
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
    external_package_boundary_with(&DiskReader, project, facts, refusals)
}

/// The package boundary of a request that supplies no package files and no
/// import attestation -- the WASM `checkSync` host protocol (P5 of the
/// contract-removal plan). Nothing in such a request shows what a package
/// does or which installation a specifier names, so every runtime import or
/// re-export of a bare specifier, Solid core excepted, keeps an open
/// requirement. `node:` built-ins are not packages. `answered` names the
/// `(importer, specifier)` pairs something else in the request answers.
#[must_use]
pub fn unsupplied_package_boundary(
    facts: &solid_facts::ProjectFacts,
    answered: &std::collections::BTreeSet<(String, String)>,
) -> PackageBoundary {
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
            let pair = (file.path.as_str().to_owned(), specifier.to_owned());
            if answered.contains(&pair) {
                continue;
            }
            let key = ImportKey::new(&pair.0, pair.1);
            boundary.record(
                key.clone(),
                Some(&name),
                None,
                BoundaryReason::new(
                    UnanalyzableReason::ResolutionUnavailable,
                    "the request supplies no package files or import resolution",
                ),
            );
            boundary.require_legacy(key, UncertifiableImportReason::Unspecified);
        }
    }
    boundary
}

/// The filesystem lookups the discovery below makes, and nothing else. A read
/// error is an answer the discovery folds into "no manifest" at every site, so
/// it returns the `io::Result` untouched and the boundary stays infallible.
///
/// `canonicalize` and `kind` are asked only by the fallible provenance
/// acquisition in [`unconsumed`], which folds no error into absence.
trait ManifestReader {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn is_file(&self, path: &Path) -> bool;
    /// The real path, as [`fs::canonicalize`] answers it.
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
    /// What `path` is, following symlinks as [`Path::is_file`] does, with
    /// every failure kept.
    fn kind(&self, path: &Path) -> io::Result<NodeKind>;
    /// What the directory entry `path` itself is, without following a final
    /// symlink, with every failure kept. Asked only to establish that a
    /// followed `NotFound` is a genuinely absent entry.
    fn entry_kind(&self, path: &Path) -> io::Result<NodeKind>;
}

/// One metadata answer. `Symlink` only ever comes from `entry_kind`; a
/// followed answer is the link's target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NodeKind {
    File { len: u64 },
    Dir,
    Symlink,
    Other,
}

/// The operational reader: the real filesystem.
struct DiskReader;

impl ManifestReader for DiskReader {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        fs::canonicalize(path)
    }

    fn kind(&self, path: &Path) -> io::Result<NodeKind> {
        let metadata = fs::metadata(path)?;
        Ok(if metadata.is_file() {
            NodeKind::File {
                len: metadata.len(),
            }
        } else if metadata.is_dir() {
            NodeKind::Dir
        } else {
            NodeKind::Other
        })
    }

    fn entry_kind(&self, path: &Path) -> io::Result<NodeKind> {
        let file_type = fs::symlink_metadata(path)?.file_type();
        Ok(if file_type.is_symlink() {
            NodeKind::Symlink
        } else if file_type.is_file() {
            NodeKind::File { len: 0 }
        } else if file_type.is_dir() {
            NodeKind::Dir
        } else {
            NodeKind::Other
        })
    }
}

fn external_package_boundary_with(
    reader: &dyn ManifestReader,
    project: &Path,
    facts: &solid_facts::ProjectFacts,
    refusals: &[crate::dependency_sources::SourceRefusal],
) -> PackageBoundary {
    match boundary_with(reader, project, facts, refusals, None) {
        Ok((boundary, _)) => boundary,
        Err(refusal) => unreachable!("only provenance coverage inspects fallibly: {refusal}"),
    }
}

/// The acceptance-gate note at each `(importer, specifier)` an unconsumed
/// tier covers, in the shape `AcceptedContractIndex::with_admission_refusals_for`
/// takes.
pub(crate) type UnconsumedTierNotes = Vec<((String, String), Option<String>)>;

/// [`external_package_boundary`] for a check that consumes no package
/// contract: every import it requires, plus each attested occurrence an
/// unconsumed tier covers (see [`UnconsumedContractProvenance`]), under the
/// same exemptions -- Solid core, package-local edges and selected dependency
/// source. Returns the note the acceptance gate adds at each covered
/// occurrence, or a refusal when a covered occurrence's installation cannot be
/// inspected (see `UnconsumedContractProvenance::covers`).
pub(crate) fn external_package_boundary_without_contracts(
    project: &Path,
    facts: &solid_facts::ProjectFacts,
    refusals: &[crate::dependency_sources::SourceRefusal],
    provenance: &UnconsumedContractProvenance,
) -> Result<(PackageBoundary, UnconsumedTierNotes), ProvenanceRefusal> {
    boundary_with(&DiskReader, project, facts, refusals, Some(provenance))
}

fn boundary_with(
    reader: &dyn ManifestReader,
    project: &Path,
    facts: &solid_facts::ProjectFacts,
    refusals: &[crate::dependency_sources::SourceRefusal],
    provenance: Option<&UnconsumedContractProvenance>,
) -> Result<(PackageBoundary, UnconsumedTierNotes), ProvenanceRefusal> {
    let mut boundary = PackageBoundary::default();
    let mut notes = Vec::new();
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
            if is_package_local_edge(reader, &mut owners, importer, import) {
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
                && local_proposal_exists(reader, project, &import.text)
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
                    .is_some_and(|manifest| manifest_uses_solid(reader, manifest))
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
            } else if let Some(provenance) = provenance
                && let Some(covered) = provenance.covers(reader, project, importer, import)?
            {
                record(UnanalyzableReason::LegacyContractFallback, &covered);
                notes.push((
                    (key.importer.clone(), key.specifier.clone()),
                    Some(format!(
                        "package contracts are not consumed in this check (--no-package-contracts), and {covered}; nothing else establishes this import's reactive behavior"
                    )),
                ));
                boundary.require_legacy(key, UncertifiableImportReason::Unspecified);
            }
        }
    }
    Ok((boundary, notes))
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

fn local_proposal_exists(reader: &dyn ManifestReader, project: &Path, specifier: &str) -> bool {
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
    reader.is_file(
        &project
            .join(".solid-checker/contracts")
            .join(package)
            .join("solid-reactivity.json"),
    )
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

/// Provenance for a check that consumes no package contract (P5 S1b-0, owner
/// checkpoint): which imports a contract tier this build would otherwise
/// answer covers, as identity only. Its consumer is the Disabled arm of the
/// shared check (`crate::check`), through
/// [`external_package_boundary_without_contracts`].
///
/// **The contract.** [`UnconsumedContractProvenance::acquire`] returns every
/// non-core `(importer, specifier)` that any discovered project catalog names,
/// with an established importer identity, and every authored
/// `(specifier, name, version)` -- or a [`ProvenanceRefusal`]. It never returns
/// a partial index, and no error becomes absence: a later entry's failure
/// discards everything acquired before it. Only a genuinely absent catalog file
/// is absence.
///
/// **Identity.** A catalog importer is keyed by its real path, by the rule the
/// catalog loader's `catalog_importer_path` uses, so a key equals the one the
/// default loader computes for the same entry. Two failure cases are
/// deliberately stricter than that loader, which tries the parent after *any*
/// failure:
/// - only `NotFound` falls back to the canonical parent plus the file name;
/// - every other error refuses, including a host with no realpath capability
///   (`Unsupported`), which gets no lexical fallback at all.
///
/// A non-UTF-8 identity refuses rather than being converted lossily.
///
/// **Scope.** Discovery covers every catalog the project's own `.solid-checker/`
/// and each nested candidate directory hold, plain and case-set alike, with
/// the case-set pointer and documents digest-checked exactly as
/// [`crate::contract_interface::catalog_paths_in`] checks them. Refusal is
/// eager: an entry no import would match still refuses when its identity
/// cannot be established.
///
/// **Absence.** A followed metadata `NotFound` does not prove absence: a
/// dangling symlink, at the entry or at any ancestor, answers the same. So an
/// optional site (the `.solid-checker` root, the plain catalog, the case-set
/// pointer) is absent only when, without following the final component, the
/// entry itself is missing *and* its parent resolves to a directory -- or the
/// parent is itself proved absent, up the chain. A dangling link refuses, as
/// does every other answer; a valid symlink resolving to the expected kind is
/// followed exactly as before.
///
/// **Coverage** (the consumer, `covers`). A requirement the acquisition keyed
/// cannot disappear because inspection fails. A catalog key covers its exact
/// attested occurrence when that occurrence resolves into `node_modules`, or is
/// unresolved while its package is installed where Node looks from the
/// importer. Installation is a followed regular `package.json`; genuine absence
/// at every ancestor is proved as discovery absence is proved, and then the
/// occurrence is TypeScript's missing module, not an obligation here. Every
/// other inspection answer refuses the operation. The authored tier covers the
/// exact specifier resolving into the resolver's exact name and version.
/// `Relative` and `NonRelative` occurrences are never covered.
mod unconsumed {
    use std::{
        collections::{BTreeMap, BTreeSet},
        fmt, io,
        path::{Path, PathBuf},
    };

    use super::{DiskReader, ManifestReader, NodeKind};
    use crate::contract_interface::{
        self, ContractFailure, MAX_CATALOG_BYTES, case_set_document_cases,
        case_set_pointer_references, catalog_member_path, verify_catalog_digest,
    };

    /// The acquired keys. Nothing else in it is read by analysis.
    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    pub(crate) struct UnconsumedContractProvenance {
        /// `(importer real path, exact specifier)` to the first catalog naming
        /// it. Every naming entry was validated, duplicates included.
        pub(crate) catalogs: BTreeMap<(String, String), PathBuf>,
        /// `(exact specifier, package name, package version)`.
        pub(crate) authored: BTreeSet<(String, String, String)>,
    }

    /// Why provenance could not be established. The operation that asked must
    /// stop with no success result.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(crate) struct ProvenanceRefusal {
        pub(crate) source: PathBuf,
        pub(crate) reason: String,
    }

    impl fmt::Display for ProvenanceRefusal {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "package-contract provenance cannot be established from {}: {}",
                self.source.display(),
                self.reason
            )
        }
    }

    impl std::error::Error for ProvenanceRefusal {}

    fn refusal(source: &Path, reason: impl fmt::Display) -> ProvenanceRefusal {
        ProvenanceRefusal {
            source: source.to_path_buf(),
            reason: reason.to_string(),
        }
    }

    impl UnconsumedContractProvenance {
        /// The project's provenance, read from the real filesystem.
        pub(crate) fn acquire(
            project_directory: &Path,
            facts: &solid_facts::ProjectFacts,
        ) -> Result<Self, ProvenanceRefusal> {
            let nested = crate::nested_catalog_candidates(
                project_directory,
                facts.files.iter().map(|file| file.path.as_str()),
            );
            acquire_with(
                &DiskReader,
                project_directory,
                &nested,
                crate::authored_contracts::embedded_index(),
            )
        }
    }

    pub(super) fn acquire_with(
        reader: &dyn ManifestReader,
        project_directory: &Path,
        nested: &[PathBuf],
        authored_index: &[u8],
    ) -> Result<UnconsumedContractProvenance, ProvenanceRefusal> {
        let mut provenance = UnconsumedContractProvenance::default();
        for root in std::iter::once(project_directory).chain(nested.iter().map(PathBuf::as_path)) {
            for (catalog, bytes) in discovered_catalogs(reader, root)? {
                let (base, entries) = contract_interface::catalog_named_entries(&catalog, &bytes)
                    .map_err(|error| refusal(&catalog, error))?;
                for (spelling, specifier) in entries {
                    let importer = importer_key(reader, &base, &spelling).map_err(|reason| {
                        refusal(&catalog, format!("entry importer {spelling:?}: {reason}"))
                    })?;
                    provenance
                        .catalogs
                        .entry((importer, specifier))
                        .or_insert_with(|| catalog.clone());
                }
            }
        }
        provenance.authored = crate::authored_contracts::authored_coverage_index(authored_index)
            .map_err(|error| refusal(Path::new("authored contract index"), error))?
            .into_iter()
            .collect();
        Ok(provenance)
    }

    /// Every catalog `root` holds, as `(path, bytes)`: the plain catalog and
    /// every case catalog the case-set pointer names. A missing `.solid-checker`
    /// directory, plain catalog or pointer is absence; any other answer about
    /// them, and any failure reading what the pointer names, refuses.
    fn discovered_catalogs(
        reader: &dyn ManifestReader,
        root: &Path,
    ) -> Result<Vec<(PathBuf, Vec<u8>)>, ProvenanceRefusal> {
        let catalog_root = root.join(".solid-checker");
        match reader.kind(&catalog_root) {
            Ok(NodeKind::Dir) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                established_absent(reader, &catalog_root)?;
                return Ok(Vec::new());
            }
            Ok(kind) => {
                return Err(refusal(
                    &catalog_root,
                    format!("is not a directory ({kind:?})"),
                ));
            }
            Err(error) => return Err(refusal(&catalog_root, error)),
        }
        let mut catalogs = Vec::new();
        let plain = catalog_root.join("accepted-contracts.json");
        if let Some(bytes) = read_catalog_file(reader, &plain, true)? {
            catalogs.push((plain, bytes));
        }
        let pointer = catalog_root.join("accepted-contract-case-set.json");
        let Some(pointer_bytes) = read_catalog_file(reader, &pointer, true)? else {
            return Ok(catalogs);
        };
        let references = case_set_pointer_references(&pointer_bytes)
            .map_err(|error: ContractFailure| refusal(&pointer, error))?;
        for (document, document_digest) in references {
            let document_path = catalog_member_path(&catalog_root, &document)
                .map_err(|error| refusal(&pointer, error))?;
            let document_bytes = required_catalog_file(reader, &document_path)?;
            let cases = case_set_document_cases(&document_bytes, &document_digest)
                .map_err(|error| refusal(&document_path, error))?;
            let base = document_path
                .parent()
                .ok_or_else(|| refusal(&document_path, "has no directory"))?;
            for (case, case_digest) in cases {
                let path = catalog_member_path(base, &case)
                    .map_err(|error| refusal(&document_path, error))?;
                let bytes = required_catalog_file(reader, &path)?;
                verify_catalog_digest(&bytes, Some(case_digest.as_str()), "catalogDigest")
                    .map_err(|error| refusal(&path, error))?;
                catalogs.push((path, bytes));
            }
        }
        Ok(catalogs)
    }

    /// A file a case-set pointer or document names: its absence refuses.
    fn required_catalog_file(
        reader: &dyn ManifestReader,
        path: &Path,
    ) -> Result<Vec<u8>, ProvenanceRefusal> {
        read_catalog_file(reader, path, false)?.ok_or_else(|| refusal(path, "is named but absent"))
    }

    /// The bytes of one catalog-tier file under the catalog byte limit. `None`
    /// only when `optional` and the file is genuinely absent.
    fn read_catalog_file(
        reader: &dyn ManifestReader,
        path: &Path,
        optional: bool,
    ) -> Result<Option<Vec<u8>>, ProvenanceRefusal> {
        match reader.kind(path) {
            Ok(NodeKind::File { len }) => {
                if len > u64::try_from(MAX_CATALOG_BYTES).unwrap_or(u64::MAX) {
                    return Err(refusal(
                        path,
                        format!("exceeds the {MAX_CATALOG_BYTES}-byte resource limit"),
                    ));
                }
                let bytes = reader.read(path).map_err(|error| refusal(path, error))?;
                if bytes.len() > MAX_CATALOG_BYTES {
                    return Err(refusal(
                        path,
                        format!("exceeds the {MAX_CATALOG_BYTES}-byte resource limit"),
                    ));
                }
                Ok(Some(bytes))
            }
            Err(error) if optional && error.kind() == io::ErrorKind::NotFound => {
                established_absent(reader, path)?;
                Ok(None)
            }
            Ok(kind) => Err(refusal(path, format!("is not a regular file ({kind:?})"))),
            Err(error) => Err(refusal(path, error)),
        }
    }

    /// Establishes that `path`, whose followed metadata answered `NotFound`, is
    /// a genuinely absent directory entry (see the module's **Absence**).
    /// Anything short of that refuses: an ambiguous path is never absence.
    fn established_absent(
        reader: &dyn ManifestReader,
        path: &Path,
    ) -> Result<(), ProvenanceRefusal> {
        match reader.entry_kind(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(NodeKind::Symlink) => {
                return Err(refusal(path, "is a dangling symlink, not an absent entry"));
            }
            Ok(kind) => {
                return Err(refusal(
                    path,
                    format!("exists ({kind:?}) but does not resolve"),
                ));
            }
            Err(error) => {
                return Err(refusal(
                    path,
                    format!("its absence cannot be established: {error}"),
                ));
            }
        }
        let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        else {
            return Err(refusal(
                path,
                "has no parent to establish its absence against",
            ));
        };
        match reader.kind(parent) {
            Ok(NodeKind::Dir) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                established_absent(reader, parent)
            }
            Ok(kind) => Err(refusal(parent, format!("is not a directory ({kind:?})"))),
            Err(error) => Err(refusal(
                parent,
                format!("its absence cannot be established: {error}"),
            )),
        }
    }

    /// The real-path identity of one catalog importer spelling (see the module
    /// contract). `base` is the catalog's own base.
    pub(super) fn importer_key(
        reader: &dyn ManifestReader,
        base: &Path,
        spelling: &str,
    ) -> Result<String, String> {
        let candidate =
            contract_interface::catalog_path(base, spelling).map_err(|error| error.to_string())?;
        let canonical = match reader.canonicalize(&candidate) {
            Ok(canonical) => canonical,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let (Some(parent), Some(name)) = (candidate.parent(), candidate.file_name()) else {
                    return Err(format!("{} names no file", candidate.display()));
                };
                let parent = reader
                    .canonicalize(parent)
                    .map_err(|error| capability(parent, &error))?;
                parent.join(name)
            }
            Err(error) => return Err(capability(&candidate, &error)),
        };
        canonical
            .into_os_string()
            .into_string()
            .map_err(|path| format!("{} is not UTF-8", Path::new(&path).display()))
    }

    fn capability(path: &Path, error: &io::Error) -> String {
        if error.kind() == io::ErrorKind::Unsupported {
            format!(
                "this host has no realpath capability, so the identity of {} cannot be established",
                path.display()
            )
        } else {
            format!("cannot resolve {}: {error}", path.display())
        }
    }

    impl UnconsumedContractProvenance {
        /// Why `importer`'s attested `import` is covered by an unconsumed tier,
        /// `None` when it is not, or a refusal when covering it would need an
        /// installation answer that cannot be established (see the module's
        /// **Coverage**). Only a catalog-keyed unresolved occurrence is ever
        /// inspected; every other occurrence is decided from the facts alone.
        pub(super) fn covers(
            &self,
            reader: &dyn ManifestReader,
            project: &Path,
            importer: &str,
            import: &solid_facts::AttestedImport,
        ) -> Result<Option<String>, ProvenanceRefusal> {
            use solid_facts::ImportResolution;
            let key = (importer.to_owned(), import.text.to_string());
            let catalog = self.catalogs.get(&key);
            match import.resolution {
                ImportResolution::NodeModules => {}
                ImportResolution::Unresolved => {
                    if catalog.is_none() || !installed_from(reader, importer, &import.text)? {
                        return Ok(None);
                    }
                }
                _ => return Ok(None),
            }
            if let Some(catalog) = catalog {
                let shown = catalog.strip_prefix(project).unwrap_or(catalog);
                return Ok(Some(format!(
                    "the project contract catalog {} names this import",
                    shown.display()
                )));
            }
            if import.resolution != ImportResolution::NodeModules {
                return Ok(None);
            }
            let (Some(name), Some(version)) = (
                import
                    .resolver_package_name
                    .as_deref()
                    .or(import.package_name.as_deref()),
                import
                    .resolver_package_version
                    .as_deref()
                    .or(import.package_version.as_deref()),
            ) else {
                return Ok(None);
            };
            Ok(self
                .authored
                .contains(&(import.text.to_string(), name.to_owned(), version.to_owned()))
                .then(|| format!("this build's authored contract tier covers {name}@{version}")))
        }
    }

    /// Whether `specifier`'s package is installed where Node looks for it from
    /// `importer`: a followed regular `node_modules/<package>/package.json` in
    /// an ancestor of the importer's directory, nearest first. A candidate is
    /// passed over only when it is proved genuinely absent; any other answer --
    /// a dangling link, the wrong kind, an unreadable entry, any error --
    /// refuses, and so does a specifier that names no package.
    pub(super) fn installed_from(
        reader: &dyn ManifestReader,
        importer: &str,
        specifier: &str,
    ) -> Result<bool, ProvenanceRefusal> {
        let Some(package) = crate::dependency_sources::package_name(specifier) else {
            return Err(refusal(
                Path::new(importer),
                format!("the catalog-covered specifier {specifier:?} names no package"),
            ));
        };
        for directory in Path::new(importer).ancestors().skip(1) {
            let candidate = directory
                .join("node_modules")
                .join(&package)
                .join("package.json");
            match reader.kind(&candidate) {
                Ok(NodeKind::File { .. }) => return Ok(true),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    established_absent(reader, &candidate)?;
                }
                Ok(kind) => {
                    return Err(refusal(
                        &candidate,
                        format!("is not a regular file ({kind:?})"),
                    ));
                }
                Err(error) => return Err(refusal(&candidate, error)),
            }
        }
        Ok(false)
    }
}

pub(crate) use unconsumed::{ProvenanceRefusal, UnconsumedContractProvenance};

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        collections::BTreeMap,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use solid_facts::{
        AttestedImport, AttestedImportIndex, FileFacts, ImportResolution, ProjectFacts,
        TypeScriptTable,
        compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap},
        core::{Generation, Span},
    };

    use super::*;
    use crate::dependency_sources::SourceRefusal;

    // characterization:begin
    //
    // These tests pin the public wrapper against real unique temporary trees.
    // Every tree has a named root manifest, so an ownership walk always ends
    // inside it and never reads a host ancestor.

    /// A real temporary project tree and the facts that import from it.
    struct Fixture {
        root: PathBuf,
        written: Vec<(PathBuf, Vec<u8>)>,
        dirs: Vec<PathBuf>,
        absent: Vec<PathBuf>,
        rows: Vec<(String, AttestedImport)>,
        sources: Vec<(String, String)>,
        source_roots: Vec<String>,
        refusals: Vec<SourceRefusal>,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "solid-checker-package-requirements-{}-{}-{tag}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            let mut fixture = Self {
                root,
                written: Vec::new(),
                dirs: Vec::new(),
                absent: Vec::new(),
                rows: Vec::new(),
                sources: Vec::new(),
                source_roots: Vec::new(),
                refusals: Vec::new(),
            };
            fixture.file("package.json", r#"{"name":"fixture-root"}"#);
            fixture.absent("src/package.json");
            fixture
        }

        fn path(&self, rel: &str) -> String {
            self.root.join(rel).to_string_lossy().into_owned()
        }

        fn file(&mut self, rel: &str, contents: &str) {
            let path = self.root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
            self.written.push((path, contents.as_bytes().to_vec()));
        }

        fn dir(&mut self, rel: &str) {
            let path = self.root.join(rel);
            fs::create_dir_all(&path).unwrap();
            self.dirs.push(path);
        }

        /// A path the owner queries that holds nothing on disk. The fake
        /// reader must be told so; it never infers absence.
        fn absent(&mut self, rel: &str) {
            self.absent.push(self.root.join(rel));
        }

        /// One import row of `importer`, resolved to `resolved` (empty for
        /// none), with `manifest` as its nearest installed manifest.
        fn import(
            &mut self,
            importer: &str,
            text: &str,
            resolution: ImportResolution,
            resolved: &str,
            package: Option<&str>,
            manifest: Option<&str>,
        ) -> usize {
            let start = u32::try_from(self.rows.len()).unwrap() * 10;
            let row = AttestedImport {
                span: Span::new(start, start + 5),
                text: text.into(),
                resolution,
                resolved_path: if resolved.is_empty() {
                    "".into()
                } else {
                    self.path(resolved).into()
                },
                included_path: "".into(),
                symlink_path: "".into(),
                extension: ".js".into(),
                package_name: package.map(Into::into),
                package_version: None,
                package_manifest: manifest.map(|rel| self.path(rel).into()),
                resolver_package_name: None,
                resolver_package_version: None,
            };
            self.rows.push((self.path(importer), row));
            self.rows.len() - 1
        }

        fn source(&mut self, importer: &str, source: &str) {
            self.sources.push((self.path(importer), source.to_owned()));
        }

        fn facts(&self) -> ProjectFacts {
            let mut by_importer: BTreeMap<&str, Vec<AttestedImport>> = BTreeMap::new();
            for (importer, row) in &self.rows {
                by_importer
                    .entry(importer.as_str())
                    .or_default()
                    .push(row.clone());
            }
            let mut index = AttestedImportIndex::default();
            for (importer, rows) in by_importer {
                index.insert_file(importer, rows);
            }
            let files = self
                .sources
                .iter()
                .map(|(path, source)| {
                    let ast = solid_facts::ast::extract(path.as_str(), source).unwrap();
                    let compiler = ExecutionMap {
                        compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                        source_hash: ast.source.hash.clone(),
                        semantic_model: Default::default(),
                        tracked_regions: vec![],
                        untracked_regions: vec![],
                        discarded_regions: vec![],
                        ownership_regions: vec![],
                        callback_roles: vec![],
                        jsx_operations: vec![],
                    };
                    FileFacts::new(Generation::new(1).unwrap(), source.as_str(), ast, compiler)
                        .unwrap()
                })
                .collect();
            let project_id = self.path("tsconfig.json");
            ProjectFacts {
                generation: Generation::new(1).unwrap(),
                project_id: project_id.clone(),
                files,
                typescript: TypeScriptTable::from_parts(
                    3,
                    1,
                    project_id,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
                typescript_changes: None,
                resolved_imports: Some(index),
                runtime_resolutions: None,
                runtime_symbol_redirects: Default::default(),
                dependency_source_roots: self.source_roots.clone(),
            }
        }

        fn boundary(&self) -> PackageBoundary {
            external_package_boundary(&self.root, &self.facts(), &self.refusals)
        }

        fn importer(&self, row: usize) -> &str {
            &self.rows[row].0
        }

        fn text(&self, row: usize) -> &str {
            &self.rows[row].1.text
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn kinds(boundary: &PackageBoundary, fixture: &Fixture, row: usize) -> Vec<UnanalyzableReason> {
        boundary
            .import(fixture.importer(row), fixture.text(row))
            .map(|import| import.reasons().iter().map(|reason| reason.kind).collect())
            .unwrap_or_default()
    }

    fn marker(boundary: &PackageBoundary, fixture: &Fixture, row: usize) -> bool {
        boundary
            .legacy_reason(fixture.importer(row), fixture.text(row))
            .is_some()
    }

    /// The import raised the contract-era marker and its explanation.
    fn kept(boundary: &PackageBoundary, fixture: &Fixture, row: usize) -> bool {
        marker(boundary, fixture, row)
            && kinds(boundary, fixture, row).contains(&UnanalyzableReason::LegacyContractFallback)
    }

    /// The import left no trace in the boundary at all.
    fn untouched(boundary: &PackageBoundary, fixture: &Fixture, row: usize) -> bool {
        !marker(boundary, fixture, row)
            && boundary
                .import(fixture.importer(row), fixture.text(row))
                .is_none()
    }

    const SOLID_DEPENDENCY: &str = r#"{"name":"pkg","dependencies":{"solid-js":"^2"}}"#;

    /// Relative edges between files of one installation. Each row would raise
    /// a marker through its installed manifest unless the edge is local.
    fn local_edges() -> (Fixture, Vec<(usize, bool)>) {
        let mut f = Fixture::new("local-edges");
        f.file("node_modules/pkg/package.json", SOLID_DEPENDENCY);
        f.file("node_modules/pkg/esm/package.json", r#"{"type":"module"}"#);
        f.file("node_modules/pkg/bad/package.json", "{not json");
        f.file("node_modules/pkg/array/package.json", "[1]");
        f.file("node_modules/pkg/numeric/package.json", r#"{"name":5}"#);
        f.file(
            "node_modules/pkg/vendor/package.json",
            r#"{"name":"vendored"}"#,
        );
        f.file("node_modules/other/package.json", r#"{"name":"other"}"#);
        f.absent("node_modules/pkg/dist/package.json");
        let manifest = "node_modules/pkg/package.json";
        let row = |f: &mut Fixture, importer: &str, text: &str, resolved: &str, local| {
            (
                f.import(
                    importer,
                    text,
                    ImportResolution::NodeModules,
                    resolved,
                    Some("pkg"),
                    Some(manifest),
                ),
                local,
            )
        };
        let rows = vec![
            row(
                &mut f,
                "node_modules/pkg/dist/a.js",
                "./b.js",
                "node_modules/pkg/dist/b.js",
                true,
            ),
            row(
                &mut f,
                "node_modules/pkg/esm/a.js",
                "../dist/b.js",
                "node_modules/pkg/dist/b.js",
                true,
            ),
            row(
                &mut f,
                "node_modules/pkg/bad/a.js",
                "../dist/b.js",
                "node_modules/pkg/dist/b.js",
                true,
            ),
            row(
                &mut f,
                "node_modules/pkg/array/a.js",
                "../dist/b.js",
                "node_modules/pkg/dist/b.js",
                true,
            ),
            row(
                &mut f,
                "node_modules/pkg/numeric/a.js",
                "../dist/b.js",
                "node_modules/pkg/dist/b.js",
                true,
            ),
            row(
                &mut f,
                "node_modules/pkg/dist/a.js",
                "../vendor/c.js",
                "node_modules/pkg/vendor/c.js",
                false,
            ),
            row(
                &mut f,
                "node_modules/pkg/dist/a.js",
                "../../other/x.js",
                "node_modules/other/x.js",
                false,
            ),
            row(
                &mut f,
                "node_modules/pkg/dist/a.js",
                "pkg/dist/b.js",
                "node_modules/pkg/dist/b.js",
                false,
            ),
            row(
                &mut f,
                "node_modules/pkg/dist/a.js",
                "./empty.js",
                "",
                false,
            ),
        ];
        (f, rows)
    }

    #[test]
    fn a_relative_edge_inside_one_installation_raises_nothing() {
        let (f, rows) = local_edges();
        let boundary = f.boundary();
        for (row, local) in rows {
            if local {
                assert!(untouched(&boundary, &f, row), "{}", f.text(row));
            } else {
                assert!(kept(&boundary, &f, row), "{}", f.text(row));
            }
        }
    }

    /// Package identity comes from the resolver's own name first; the
    /// primitive-defining packages raise nothing.
    fn primitive_precedence() -> (Fixture, Vec<(usize, bool)>) {
        let mut f = Fixture::new("primitive-precedence");
        f.file("node_modules/dep/package.json", SOLID_DEPENDENCY);
        let manifest = "node_modules/dep/package.json";
        let importer = "src/main.ts";
        let add = |f: &mut Fixture,
                   text: &str,
                   resolution,
                   package: Option<&str>,
                   resolver: Option<&str>,
                   kept| {
            let row = f.import(
                importer,
                text,
                resolution,
                "node_modules/dep/index.js",
                package,
                Some(manifest),
            );
            f.rows[row].1.resolver_package_name = resolver.map(Into::into);
            (row, kept)
        };
        let nm = ImportResolution::NodeModules;
        let rows = vec![
            add(&mut f, "a", nm, Some("lib"), Some("solid-js"), false),
            add(&mut f, "b", nm, Some("solid-js"), Some("lib"), true),
            add(&mut f, "c", nm, Some("@solidjs/web"), None, false),
            add(&mut f, "d", nm, Some("@solidjs/router"), None, true),
            add(&mut f, "e", nm, None, None, false),
            add(
                &mut f,
                "f",
                ImportResolution::NonRelative,
                Some("lib"),
                None,
                false,
            ),
        ];
        (f, rows)
    }

    #[test]
    fn primitive_packages_are_skipped_before_anything_else() {
        let (f, rows) = primitive_precedence();
        let boundary = f.boundary();
        for (row, expect_kept) in rows {
            if expect_kept {
                assert!(kept(&boundary, &f, row), "{}", f.text(row));
            } else {
                assert!(untouched(&boundary, &f, row), "{}", f.text(row));
            }
        }
    }

    /// One row per installed-manifest shape, with and without source refusals.
    fn manifest_fields(populated: bool) -> (Fixture, Vec<(usize, bool)>) {
        let mut f = Fixture::new(if populated {
            "manifest-fields-populated"
        } else {
            "manifest-fields"
        });
        let manifests = [
            ("m-deps", r#"{"dependencies":{"solid-js":"2"}}"#, true),
            (
                "m-peer",
                r#"{"peerDependencies":{"@solidjs/web":"2"}}"#,
                true,
            ),
            (
                "m-optional",
                r#"{"optionalDependencies":{"@solidjs/signals":"2"}}"#,
                true,
            ),
            (
                "m-scope",
                r#"{"dependencies":{"@solidjs/router":"1"}}"#,
                true,
            ),
            ("m-dev", r#"{"devDependencies":{"solid-js":"2"}}"#, false),
            ("m-other", r#"{"dependencies":{"left-pad":"1"}}"#, false),
            ("m-bad", "{", false),
            ("m-array", "[]", false),
        ];
        let mut rows = Vec::new();
        for (name, manifest, expect_kept) in manifests {
            f.file(&format!("node_modules/{name}/package.json"), manifest);
            rows.push((
                f.import(
                    "src/main.ts",
                    name,
                    ImportResolution::NodeModules,
                    &format!("node_modules/{name}/index.js"),
                    Some(name),
                    Some(&format!("node_modules/{name}/package.json")),
                ),
                expect_kept,
            ));
        }
        f.absent("node_modules/m-missing/package.json");
        rows.push((
            f.import(
                "src/main.ts",
                "m-missing",
                ImportResolution::NodeModules,
                "node_modules/m-missing/index.js",
                Some("m-missing"),
                Some("node_modules/m-missing/package.json"),
            ),
            false,
        ));
        rows.push((
            f.import(
                "src/main.ts",
                "m-unmanifested",
                ImportResolution::NodeModules,
                "node_modules/m-unmanifested/index.js",
                Some("m-unmanifested"),
                None,
            ),
            false,
        ));
        if populated {
            f.refusals = vec![
                SourceRefusal {
                    name: "m-deps".into(),
                    root: None,
                    kind: UnanalyzableReason::ManifestUnavailable,
                    detail: "by name".into(),
                },
                SourceRefusal {
                    name: "other".into(),
                    root: Some(f.root.join("node_modules/m-peer")),
                    kind: UnanalyzableReason::ClosureUnavailable,
                    detail: "by root".into(),
                },
                SourceRefusal {
                    name: "m-optional".into(),
                    root: Some(f.root.join("node_modules/elsewhere")),
                    kind: UnanalyzableReason::ClosureLimit,
                    detail: "another copy".into(),
                },
            ];
        }
        (f, rows)
    }

    #[test]
    fn an_installed_manifest_naming_solid_raises_the_marker() {
        for populated in [false, true] {
            let (f, rows) = manifest_fields(populated);
            let boundary = f.boundary();
            for &(row, expect_kept) in &rows {
                if expect_kept {
                    assert!(kept(&boundary, &f, row), "{}", f.text(row));
                } else {
                    assert!(untouched(&boundary, &f, row), "{}", f.text(row));
                }
            }
            let has = |row: usize, kind| kinds(&boundary, &f, row).contains(&kind);
            assert_eq!(
                has(0, UnanalyzableReason::ManifestUnavailable),
                populated,
                "a refusal about the package name joins its import"
            );
            assert_eq!(
                has(1, UnanalyzableReason::ClosureUnavailable),
                populated,
                "a refusal about the installed root joins imports under it"
            );
            assert!(
                !has(2, UnanalyzableReason::ClosureLimit),
                "a refusal about another copy joins nothing"
            );
        }
    }

    #[test]
    fn the_requirement_index_carries_exactly_the_boundary_markers() {
        let (f, rows) = manifest_fields(true);
        let facts = f.facts();
        let index = external_package_contract_requirements(&f.root, &facts);
        let boundary = f.boundary();
        assert_eq!(index, requirement_index(&boundary));
        for (row, expect_kept) in rows {
            assert_eq!(
                index.is_uncertifiable(f.importer(row), f.text(row)),
                expect_kept
            );
        }
    }

    /// Unresolved imports raise through a project-local proposal file.
    fn proposals() -> (Fixture, Vec<(usize, bool)>) {
        let mut f = Fixture::new("proposals");
        f.file(
            ".solid-checker/contracts/has-file/solid-reactivity.json",
            "{}",
        );
        f.file(
            ".solid-checker/contracts/@scope/pkg/solid-reactivity.json",
            "{}",
        );
        f.dir(".solid-checker/contracts/is-dir/solid-reactivity.json");
        f.absent(".solid-checker/contracts/missing/solid-reactivity.json");
        f.file(
            ".solid-checker/contracts/resolved/solid-reactivity.json",
            "{}",
        );
        f.file(
            "node_modules/resolved/package.json",
            r#"{"name":"resolved"}"#,
        );
        let mut rows = Vec::new();
        for (text, expect_kept) in [
            ("has-file", true),
            ("has-file/sub/path", true),
            ("@scope/pkg/deep", true),
            ("is-dir", false),
            ("missing", false),
            ("@scope", false),
        ] {
            rows.push((
                f.import(
                    "src/main.ts",
                    text,
                    ImportResolution::Unresolved,
                    "",
                    None,
                    None,
                ),
                expect_kept,
            ));
        }
        rows.push((
            f.import(
                "src/main.ts",
                "resolved",
                ImportResolution::NodeModules,
                "node_modules/resolved/index.js",
                Some("resolved"),
                Some("node_modules/resolved/package.json"),
            ),
            false,
        ));
        (f, rows)
    }

    #[test]
    fn a_local_proposal_is_a_regular_file_for_an_unresolved_import() {
        let (f, rows) = proposals();
        let boundary = f.boundary();
        for (row, expect_kept) in rows {
            if expect_kept {
                assert!(kept(&boundary, &f, row), "{}", f.text(row));
                let reasons = kinds(&boundary, &f, row);
                assert!(reasons.contains(&UnanalyzableReason::ResolutionUnavailable));
            } else {
                assert!(untouched(&boundary, &f, row), "{}", f.text(row));
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_proposal_symlink_is_followed_like_a_file() {
        let mut f = Fixture::new("proposal-symlinks");
        f.file("real-proposal.json", "{}");
        for (name, target) in [
            ("linked", f.root.join("real-proposal.json")),
            ("dangling", f.root.join("no-such-proposal.json")),
        ] {
            let directory = f.root.join(".solid-checker/contracts").join(name);
            fs::create_dir_all(&directory).unwrap();
            std::os::unix::fs::symlink(target, directory.join("solid-reactivity.json")).unwrap();
        }
        let linked = f.import(
            "src/main.ts",
            "linked",
            ImportResolution::Unresolved,
            "",
            None,
            None,
        );
        let dangling = f.import(
            "src/main.ts",
            "dangling",
            ImportResolution::Unresolved,
            "",
            None,
            None,
        );
        let boundary = f.boundary();
        assert!(kept(&boundary, &f, linked));
        assert!(untouched(&boundary, &f, dangling));
    }

    /// Imports of packages analyzed as source: only a runtime namespace
    /// import keeps an obligation, and the checks after it still run.
    fn source_roots() -> (Fixture, Vec<usize>) {
        let mut f = Fixture::new("source-roots");
        let importers = [
            ("srcpkg-ns", "import * as ns from \"srcpkg-ns\";\n", false),
            (
                "srcpkg-ns-solid",
                "import * as ns from \"srcpkg-ns-solid\";\n",
                true,
            ),
            (
                "srcpkg-named",
                "import { a } from \"srcpkg-named\";\n",
                true,
            ),
            (
                "srcpkg-type",
                "import type * as ns from \"srcpkg-type\";\n",
                true,
            ),
        ];
        let mut rows = Vec::new();
        for (name, source, solid) in importers {
            let importer = format!("src/{name}.ts");
            f.source(&importer, source);
            f.file(
                &format!("node_modules/{name}/package.json"),
                if solid {
                    SOLID_DEPENDENCY
                } else {
                    r#"{"name":"quiet"}"#
                },
            );
            f.source_roots.push(f.path(&format!("node_modules/{name}")));
            rows.push(f.import(
                &importer,
                name,
                ImportResolution::NodeModules,
                &format!("node_modules/{name}/index.js"),
                Some(name),
                Some(&format!("node_modules/{name}/package.json")),
            ));
        }
        (f, rows)
    }

    #[test]
    fn selected_source_is_redirected_except_a_runtime_namespace_import() {
        let (f, rows) = source_roots();
        let boundary = f.boundary();
        let namespace = UnanalyzableReason::UnsupportedBinding(BindingForm::NamespaceValue);
        // A quiet manifest: the namespace explanation alone, with no marker.
        assert_eq!(kinds(&boundary, &f, rows[0]), vec![namespace]);
        assert!(!marker(&boundary, &f, rows[0]));
        // A solid-naming manifest: the explanation and the later marker.
        assert!(kinds(&boundary, &f, rows[1]).contains(&namespace));
        assert!(kept(&boundary, &f, rows[1]));
        // Named and type-only namespace imports are redirected: no trace.
        assert!(untouched(&boundary, &f, rows[2]));
        assert!(untouched(&boundary, &f, rows[3]));
    }

    /// Every scenario of this section, for the differential below.
    fn scenarios() -> Vec<(&'static str, Fixture)> {
        vec![
            ("local-edges", local_edges().0),
            ("primitive-precedence", primitive_precedence().0),
            ("manifest-fields", manifest_fields(false).0),
            ("manifest-fields-populated", manifest_fields(true).0),
            ("proposals", proposals().0),
            ("source-roots", source_roots().0),
        ]
    }
    // characterization:end

    // fake-reader:begin
    //
    // An in-memory reader answers exactly the lookups the discovery makes. The
    // discovery folds every read error into "no manifest", so an `Err` could
    // not tell a test that a lookup was never tabled; a queried path outside
    // the table therefore panics. That is a test harness fault, not a
    // production policy for missing input, and there is no disk fallback.

    enum Read {
        Bytes(Vec<u8>),
        Absent,
        Unreadable,
    }

    #[derive(Default)]
    struct FakeReader {
        reads: BTreeMap<PathBuf, Read>,
        files: BTreeMap<PathBuf, bool>,
        /// `canonicalize` answers; a path outside the table panics.
        canonical: BTreeMap<PathBuf, Result<PathBuf, io::ErrorKind>>,
        /// `kind` answers; a path outside the table panics.
        kinds: BTreeMap<PathBuf, Result<NodeKind, io::ErrorKind>>,
        /// `entry_kind` (no-follow) answers; a path outside the table panics,
        /// so a test shows exactly which absence queries a branch makes.
        entries: BTreeMap<PathBuf, Result<NodeKind, io::ErrorKind>>,
        read_queries: RefCell<BTreeMap<PathBuf, usize>>,
        file_queries: RefCell<BTreeMap<PathBuf, usize>>,
    }

    impl FakeReader {
        fn read(mut self, path: &str, entry: Read) -> Self {
            self.reads.insert(PathBuf::from(path), entry);
            self
        }

        fn bytes(self, path: &str, contents: &str) -> Self {
            self.read(path, Read::Bytes(contents.as_bytes().to_vec()))
        }

        /// The table a fixture's real tree would answer from.
        fn of_tree(fixture: &Fixture) -> Self {
            let mut reader = Self::default();
            for (path, contents) in &fixture.written {
                reader
                    .reads
                    .insert(path.clone(), Read::Bytes(contents.clone()));
                reader.files.insert(path.clone(), true);
            }
            for path in &fixture.absent {
                reader.reads.insert(path.clone(), Read::Absent);
                reader.files.insert(path.clone(), false);
            }
            for path in &fixture.dirs {
                reader.files.insert(path.clone(), false);
            }
            reader
        }

        fn queried_reads(&self) -> BTreeMap<PathBuf, usize> {
            self.read_queries.borrow().clone()
        }

        fn queried_files(&self) -> Vec<PathBuf> {
            self.file_queries.borrow().keys().cloned().collect()
        }

        /// A genuinely absent entry: followed and no-follow both NotFound.
        fn absent_entry(&mut self, path: &str) {
            self.kinds
                .insert(PathBuf::from(path), Err(io::ErrorKind::NotFound));
            self.entries
                .insert(PathBuf::from(path), Err(io::ErrorKind::NotFound));
        }
    }

    impl ManifestReader for FakeReader {
        fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
            *self
                .read_queries
                .borrow_mut()
                .entry(path.to_path_buf())
                .or_default() += 1;
            match self.reads.get(path) {
                Some(Read::Bytes(bytes)) => Ok(bytes.clone()),
                Some(Read::Absent) => Err(io::ErrorKind::NotFound.into()),
                Some(Read::Unreadable) => Err(io::ErrorKind::PermissionDenied.into()),
                None => panic!("test reader: no table entry for read of {}", path.display()),
            }
        }

        fn is_file(&self, path: &Path) -> bool {
            *self
                .file_queries
                .borrow_mut()
                .entry(path.to_path_buf())
                .or_default() += 1;
            *self.files.get(path).unwrap_or_else(|| {
                panic!(
                    "test reader: no table entry for is_file of {}",
                    path.display()
                )
            })
        }

        fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
            self.canonical
                .get(path)
                .unwrap_or_else(|| {
                    panic!(
                        "test reader: no table entry for canonicalize of {}",
                        path.display()
                    )
                })
                .clone()
                .map_err(io::Error::from)
        }

        fn kind(&self, path: &Path) -> io::Result<NodeKind> {
            (*self.kinds.get(path).unwrap_or_else(|| {
                panic!("test reader: no table entry for kind of {}", path.display())
            }))
            .map_err(io::Error::from)
        }

        fn entry_kind(&self, path: &Path) -> io::Result<NodeKind> {
            (*self.entries.get(path).unwrap_or_else(|| {
                panic!(
                    "test reader: no table entry for entry_kind of {}",
                    path.display()
                )
            }))
            .map_err(io::Error::from)
        }
    }

    fn virtual_row(
        text: &str,
        resolution: ImportResolution,
        resolved: &str,
        package: Option<&str>,
        manifest: Option<&str>,
    ) -> AttestedImport {
        AttestedImport {
            span: Span::new(0, 5),
            text: text.into(),
            resolution,
            resolved_path: resolved.into(),
            included_path: "".into(),
            symlink_path: "".into(),
            extension: ".js".into(),
            package_name: package.map(Into::into),
            package_version: None,
            package_manifest: manifest.map(Into::into),
            resolver_package_name: None,
            resolver_package_version: None,
        }
    }

    /// Facts whose rows live at virtual absolute paths, for controls that
    /// must not touch the disk (no named owner, the filesystem root).
    fn virtual_facts(rows: Vec<(&str, AttestedImport)>) -> ProjectFacts {
        let mut by_importer: BTreeMap<&str, Vec<AttestedImport>> = BTreeMap::new();
        for (position, (importer, mut row)) in rows.into_iter().enumerate() {
            let start = u32::try_from(position).unwrap() * 10;
            row.span = Span::new(start, start + 5);
            by_importer.entry(importer).or_default().push(row);
        }
        let mut index = AttestedImportIndex::default();
        for (importer, rows) in by_importer {
            index.insert_file(importer, rows);
        }
        ProjectFacts {
            generation: Generation::new(1).unwrap(),
            project_id: "/virtual/tsconfig.json".into(),
            files: Vec::new(),
            typescript: TypeScriptTable::from_parts(
                3,
                1,
                "/virtual/tsconfig.json",
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            typescript_changes: None,
            resolved_imports: Some(index),
            runtime_resolutions: None,
            runtime_symbol_redirects: Default::default(),
            dependency_source_roots: Vec::new(),
        }
    }

    fn boundary_of(reader: &FakeReader, rows: Vec<(&str, AttestedImport)>) -> PackageBoundary {
        external_package_boundary_with(reader, Path::new("/virtual"), &virtual_facts(rows), &[])
    }

    /// Each queried path and how often, as `path=count`. Path sets and counts
    /// only: the discovery's query order is not defined.
    fn counts(reader: &FakeReader) -> Vec<String> {
        reader
            .queried_reads()
            .into_iter()
            .map(|(path, count)| format!("{}={count}", path.display()))
            .collect()
    }

    fn named(name: &str) -> String {
        format!(r#"{{"name":"{name}"}}"#)
    }

    #[test]
    fn the_disk_reader_and_a_table_of_the_same_tree_agree_on_every_scenario() {
        for (name, fixture) in scenarios() {
            let reader = FakeReader::of_tree(&fixture);
            let fake = external_package_boundary_with(
                &reader,
                &fixture.root,
                &fixture.facts(),
                &fixture.refusals,
            );
            let disk = fixture.boundary();
            assert!(!disk.is_empty(), "{name} must exercise the discovery");
            assert_eq!(disk, fake, "{name}");
        }
    }

    #[test]
    fn a_proposal_is_looked_up_only_for_an_unresolved_import() {
        let (fixture, _) = proposals();
        let reader = FakeReader::of_tree(&fixture);
        external_package_boundary_with(&reader, &fixture.root, &fixture.facts(), &fixture.refusals);
        let proposal = |package: &str| {
            fixture
                .root
                .join(".solid-checker/contracts")
                .join(package)
                .join("solid-reactivity.json")
        };
        let mut expected = vec![
            proposal("has-file"),
            proposal("@scope/pkg"),
            proposal("is-dir"),
            proposal("missing"),
        ];
        expected.sort();
        assert_eq!(reader.queried_files(), expected);
    }

    #[test]
    fn an_importer_without_a_named_owner_never_walks_the_target() {
        let reader = FakeReader::default()
            .read("/a/package.json", Read::Absent)
            .read("/package.json", Read::Absent)
            .bytes("/b/package.json", &named("b"));
        let boundary = boundary_of(
            &reader,
            vec![(
                "/a/i.ts",
                virtual_row(
                    "../b/t.ts",
                    ImportResolution::Relative,
                    "/b/t.ts",
                    None,
                    None,
                ),
            )],
        );
        assert!(boundary.is_empty());
        assert_eq!(counts(&reader), ["/a/package.json=1", "/package.json=1"]);
    }

    #[test]
    fn the_filesystem_root_manifest_is_queried_and_can_own() {
        let row = || {
            vec![(
                "/x/i.ts",
                virtual_row(
                    "./j.ts",
                    ImportResolution::NodeModules,
                    "/x/j.ts",
                    Some("pkg"),
                    Some("/m/package.json"),
                ),
            )]
        };
        let solid = r#"{"dependencies":{"solid-js":"2"}}"#;
        // A named root owns both files: the edge is local and the installed
        // manifest is never read.
        let reader = FakeReader::default()
            .read("/x/package.json", Read::Absent)
            .bytes("/package.json", &named("root"))
            .bytes("/m/package.json", solid);
        assert!(boundary_of(&reader, row()).is_empty());
        assert_eq!(counts(&reader), ["/package.json=1", "/x/package.json=1"]);
        // An unnamed root owns nothing: the edge is not local and the
        // installed manifest is read.
        let reader = FakeReader::default()
            .read("/x/package.json", Read::Absent)
            .bytes("/package.json", "{}")
            .bytes("/m/package.json", solid);
        let boundary = boundary_of(&reader, row());
        assert!(boundary.legacy_reason("/x/i.ts", "./j.ts").is_some());
        assert_eq!(
            counts(&reader),
            ["/m/package.json=1", "/package.json=1", "/x/package.json=1"]
        );
    }

    #[test]
    fn owner_lookups_are_memoized_per_directory_not_per_import() {
        let reader = FakeReader::default()
            .read("/x/package.json", Read::Absent)
            .read("/y/package.json", Read::Absent)
            .bytes("/package.json", &named("root"));
        let relative = |text: &str, resolved: &str| {
            (
                "/x/i.ts",
                virtual_row(text, ImportResolution::Relative, resolved, None, None),
            )
        };
        boundary_of(
            &reader,
            vec![
                relative("./a.ts", "/x/a.ts"),
                relative("./b.ts", "/x/b.ts"),
                relative("../y/c.ts", "/y/c.ts"),
            ],
        );
        assert_eq!(
            counts(&reader),
            ["/package.json=1", "/x/package.json=1", "/y/package.json=1"]
        );
    }

    #[test]
    fn a_primitive_package_is_skipped_before_any_lookup() {
        let reader = FakeReader::default();
        let boundary = boundary_of(
            &reader,
            vec![(
                "/x/i.ts",
                virtual_row(
                    "./a.ts",
                    ImportResolution::NodeModules,
                    "/x/a.ts",
                    Some("solid-js"),
                    Some("/m/package.json"),
                ),
            )],
        );
        assert!(boundary.is_empty());
        assert!(reader.queried_reads().is_empty());
        assert!(reader.queried_files().is_empty());
    }

    #[test]
    fn an_unreadable_manifest_is_folded_like_an_absent_one() {
        let solid = r#"{"dependencies":{"solid-js":"2"}}"#;
        // Unreadable importer directory: the walk continues to the root.
        let run = |importer_dir: Read, manifest: Read| {
            let reader = FakeReader::default()
                .read("/x/package.json", importer_dir)
                .bytes("/package.json", &named("root"))
                .bytes("/n/package.json", &named("n"))
                .read("/m/package.json", manifest);
            boundary_of(
                &reader,
                vec![(
                    "/x/i.ts",
                    virtual_row(
                        "../n/j.ts",
                        ImportResolution::NodeModules,
                        "/n/j.ts",
                        Some("pkg"),
                        Some("/m/package.json"),
                    ),
                )],
            )
        };
        let solid_manifest = || Read::Bytes(solid.as_bytes().to_vec());
        let absent = run(Read::Absent, solid_manifest());
        assert!(!absent.is_empty());
        assert_eq!(run(Read::Unreadable, solid_manifest()), absent);
        // An unreadable installed manifest raises nothing, as an absent one.
        let quiet = run(Read::Absent, Read::Absent);
        assert!(quiet.is_empty());
        assert_eq!(run(Read::Absent, Read::Unreadable), quiet);
    }

    #[test]
    #[should_panic(expected = "test reader: no table entry for read of /x/package.json")]
    fn a_read_outside_the_table_is_a_harness_fault() {
        boundary_of(
            &FakeReader::default(),
            vec![(
                "/x/i.ts",
                virtual_row("./a.ts", ImportResolution::Relative, "/x/a.ts", None, None),
            )],
        );
    }

    #[test]
    #[should_panic(expected = "test reader: no table entry for is_file of")]
    fn a_proposal_lookup_outside_the_table_is_a_harness_fault() {
        boundary_of(
            &FakeReader::default(),
            vec![(
                "/x/i.ts",
                virtual_row("pkg", ImportResolution::Unresolved, "", None, None),
            )],
        );
    }
    // fake-reader:end

    // provenance:begin
    //
    // The S1b-0 acquisition contract (module `unconsumed`): every catalog key
    // and authored triple with an established identity, or a refusal; never a
    // partial result and never an error read as absence.

    use super::unconsumed::{acquire_with, importer_key};

    /// A real catalog entry from a checked-in fixture, retargeted.
    fn catalog_bytes(entries: &[(&str, &str, &str)]) -> Vec<u8> {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../../fixtures/reactive-ir/package-unknown-callback-consumer/.solid-checker/accepted-contracts.json",
        );
        let mut catalog: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture).unwrap()).unwrap();
        let template = catalog["contracts"][0].clone();
        catalog["contracts"] = entries
            .iter()
            .map(|(importer, specifier, package)| {
                let mut entry = template.clone();
                entry["import"]["importer"] = (*importer).into();
                entry["import"]["specifier"] = (*specifier).into();
                entry["import"]["packageName"] = (*package).into();
                entry
            })
            .collect();
        serde_json::to_vec(&catalog).unwrap()
    }

    fn file_kind(bytes: &[u8]) -> Result<NodeKind, io::ErrorKind> {
        Ok(NodeKind::File {
            len: bytes.len() as u64,
        })
    }

    /// A fake project at `/v` with one plain catalog holding `bytes`, no
    /// case-set pointer, and `App.ts`/`Other.ts` resolving to `/real/...`.
    fn plain_catalog_reader(bytes: Vec<u8>) -> FakeReader {
        let mut reader = FakeReader::default();
        reader
            .kinds
            .insert(PathBuf::from("/v/.solid-checker"), Ok(NodeKind::Dir));
        reader.kinds.insert(
            PathBuf::from("/v/.solid-checker/accepted-contracts.json"),
            file_kind(&bytes),
        );
        reader.absent_entry("/v/.solid-checker/accepted-contract-case-set.json");
        reader.reads.insert(
            PathBuf::from("/v/.solid-checker/accepted-contracts.json"),
            Read::Bytes(bytes),
        );
        for name in ["App.ts", "Other.ts"] {
            reader.canonical.insert(
                Path::new("/v").join(name),
                Ok(Path::new("/real").join(name)),
            );
        }
        reader
    }

    fn acquire(reader: &FakeReader) -> Result<UnconsumedContractProvenance, ProvenanceRefusal> {
        acquire_with(
            reader,
            Path::new("/v"),
            &[],
            crate::authored_contracts::embedded_index(),
        )
    }

    use super::unconsumed::{ProvenanceRefusal, UnconsumedContractProvenance};

    /// A unique real directory, removed when dropped.
    struct Tree(PathBuf);

    impl Tree {
        fn new(tag: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "solid-checker-provenance-{}-{}-{tag}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Self(fs::canonicalize(root).unwrap())
        }

        fn file(&self, rel: &str) -> String {
            let path = self.0.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "export {};\n").unwrap();
            path.to_string_lossy().into_owned()
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Natively a key is the default loader's own key: the real path of an
    /// existing importer however it is spelled (relative, absolute inside or
    /// outside the base, through `..`, through a symlink), and the canonical
    /// parent plus the name of a missing final file.
    #[cfg(unix)]
    #[test]
    fn native_keys_equal_the_default_loaders_keys() {
        let tree = Tree::new("keys");
        let outside = Tree::new("outside");
        let app = tree.file("App.ts");
        let source = tree.file("src/x.ts");
        let out = outside.file("Out.ts");
        fs::create_dir_all(tree.0.join("node_modules")).unwrap();
        std::os::unix::fs::symlink("src", tree.0.join("alias")).unwrap();
        let root = tree.0.to_string_lossy().into_owned();
        let gone = tree.0.join("gone.ts").to_string_lossy().into_owned();
        for (spelling, expected) in [
            ("App.ts".to_owned(), &app),
            ("src/x.ts".to_owned(), &source),
            (app.clone(), &app),
            (out.clone(), &out),
            (format!("{root}/node_modules/../App.ts"), &app),
            ("alias/x.ts".to_owned(), &source),
            (format!("{root}/alias/x.ts"), &source),
            ("gone.ts".to_owned(), &gone),
        ] {
            let key = importer_key(&DiskReader, &tree.0, &spelling);
            assert_eq!(key.as_ref(), Ok(expected), "{spelling}");
            assert_eq!(
                crate::contract_interface::catalog_importer_path(&tree.0, &spelling)
                    .map_err(|error| error.to_string())
                    .as_ref(),
                Ok(expected),
                "{spelling}: the default loader's key"
            );
        }
    }

    /// Invalid relative members refuse on every path, as the catalog member
    /// validator refuses them; a missing directory refuses as the default
    /// loader does; and a non-directory intermediate refuses where the default
    /// loader would join a name onto a file -- the deliberately stricter
    /// NotFound-only parent rule.
    #[cfg(unix)]
    #[test]
    fn native_identity_failures_refuse() {
        let tree = Tree::new("refusals");
        tree.file("App.ts");
        for spelling in ["./App.ts", "", "src//x.ts", "../App.ts", "src/../App.ts"] {
            assert!(
                importer_key(&DiskReader, &tree.0, spelling).is_err(),
                "{spelling:?}"
            );
        }
        assert!(importer_key(&DiskReader, &tree.0, "missing-dir/x.ts").is_err());
        assert!(
            crate::contract_interface::catalog_importer_path(&tree.0, "missing-dir/x.ts").is_err()
        );
        assert!(importer_key(&DiskReader, &tree.0, "App.ts/child").is_err());
        assert!(
            crate::contract_interface::catalog_importer_path(&tree.0, "App.ts/child").is_ok(),
            "the default loader joins the name onto the file; acquisition is stricter"
        );
    }

    /// Only NotFound reaches the parent rule. Unsupported -- no realpath on
    /// this host -- refuses, for every spelling, with no lexical fallback;
    /// every other error refuses as itself.
    #[test]
    fn capability_and_io_errors_never_fall_back() {
        let base = Path::new("/v");
        for spelling in [
            "App.ts",
            "alias/x.ts",
            "/v/App.ts",
            "/v/node_modules/../App.ts",
            "/v/./App.ts",
            "/v//App.ts",
            "/outside/Out.ts",
        ] {
            let candidate = crate::contract_interface::catalog_path(base, spelling).unwrap();
            for (kind, message) in [
                (io::ErrorKind::Unsupported, "no realpath capability"),
                (io::ErrorKind::PermissionDenied, "cannot resolve"),
                (io::ErrorKind::NotADirectory, "cannot resolve"),
                (io::ErrorKind::Other, "cannot resolve"),
            ] {
                let mut reader = FakeReader::default();
                reader.canonical.insert(candidate.clone(), Err(kind));
                let error = importer_key(&reader, base, spelling).unwrap_err();
                assert!(error.contains(message), "{spelling} {kind:?}: {error}");
            }
        }
        let parent = |answer: Result<PathBuf, io::ErrorKind>| {
            let mut reader = FakeReader::default();
            reader
                .canonical
                .insert(PathBuf::from("/v/sub/App.ts"), Err(io::ErrorKind::NotFound));
            reader.canonical.insert(PathBuf::from("/v/sub"), answer);
            importer_key(&reader, base, "sub/App.ts")
        };
        assert_eq!(
            parent(Ok(PathBuf::from("/real/sub"))),
            Ok("/real/sub/App.ts".to_owned())
        );
        for kind in [
            io::ErrorKind::Unsupported,
            io::ErrorKind::NotFound,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::NotADirectory,
            io::ErrorKind::Other,
        ] {
            assert!(parent(Err(kind)).is_err(), "{kind:?}");
        }
    }

    /// Every entry is named and keyed; a core entry is left out; no contract
    /// document or receipt is read (the reader panics on any read outside
    /// its table, and the entries name documents it does not hold).
    #[test]
    fn every_catalog_entry_is_keyed_without_reading_a_document() {
        let reader = plain_catalog_reader(catalog_bytes(&[
            ("App.ts", "partial-package", "partial-package"),
            ("Other.ts", "other-package", "other-package"),
            ("App.ts", "solid-js", "solid-js"),
        ]));
        let provenance = acquire(&reader).unwrap();
        let catalog = PathBuf::from("/v/.solid-checker/accepted-contracts.json");
        assert_eq!(
            provenance.catalogs,
            BTreeMap::from([
                (
                    ("/real/App.ts".to_owned(), "partial-package".to_owned()),
                    catalog.clone()
                ),
                (
                    ("/real/Other.ts".to_owned(), "other-package".to_owned()),
                    catalog
                ),
            ])
        );
        assert!(!provenance.authored.is_empty());
        assert_eq!(
            reader.queried_reads().keys().collect::<Vec<_>>(),
            [&PathBuf::from("/v/.solid-checker/accepted-contracts.json")]
        );
    }

    /// A later entry's failure, or an invalid entry no import would ever
    /// match, refuses the whole acquisition: nothing partial escapes.
    #[test]
    fn a_late_or_unused_invalid_entry_refuses_everything() {
        let mut late = plain_catalog_reader(catalog_bytes(&[
            ("App.ts", "partial-package", "partial-package"),
            ("Stale.ts", "partial-package", "partial-package"),
        ]));
        late.canonical.insert(
            PathBuf::from("/v/Stale.ts"),
            Err(io::ErrorKind::PermissionDenied),
        );
        let error = acquire(&late).unwrap_err();
        assert!(error.reason.contains("Stale.ts"), "{error}");
        let unused = plain_catalog_reader(catalog_bytes(&[
            ("App.ts", "partial-package", "partial-package"),
            ("./never.ts", "unimported-package", "unimported-package"),
        ]));
        assert!(acquire(&unused).is_err());
    }

    /// A missing `.solid-checker`, plain catalog or pointer is absence; every
    /// other answer about them refuses -- a permission error, the wrong kind,
    /// a catalog over the byte limit (refused before any read), a read error.
    #[test]
    fn discovery_tells_absence_from_failure() {
        let catalog_root = PathBuf::from("/v/.solid-checker");
        let plain = catalog_root.join("accepted-contracts.json");
        let pointer = catalog_root.join("accepted-contract-case-set.json");
        let with = |root: Result<NodeKind, io::ErrorKind>,
                    plain_kind: Option<Result<NodeKind, io::ErrorKind>>,
                    pointer_kind: Option<Result<NodeKind, io::ErrorKind>>| {
            let mut reader = FakeReader::default();
            reader.kinds.insert(PathBuf::from("/v"), Ok(NodeKind::Dir));
            for (path, kind) in [
                (&catalog_root, Some(root)),
                (&plain, plain_kind),
                (&pointer, pointer_kind),
            ] {
                let Some(kind) = kind else { continue };
                reader.kinds.insert(path.clone(), kind);
                // A NotFound here is a genuinely absent entry.
                if kind == Err(io::ErrorKind::NotFound) {
                    reader.entries.insert(path.clone(), kind);
                }
            }
            reader
        };
        let absent = Err(io::ErrorKind::NotFound);
        let provenance = acquire(&with(absent, None, None)).unwrap();
        assert!(provenance.catalogs.is_empty() && !provenance.authored.is_empty());
        assert!(
            acquire(&with(Ok(NodeKind::Dir), Some(absent), Some(absent)))
                .unwrap()
                .catalogs
                .is_empty()
        );
        for reader in [
            with(Err(io::ErrorKind::PermissionDenied), None, None),
            with(Ok(NodeKind::File { len: 1 }), None, None),
            with(Ok(NodeKind::Other), None, None),
            with(Ok(NodeKind::Dir), Some(Ok(NodeKind::Dir)), None),
            with(
                Ok(NodeKind::Dir),
                Some(Err(io::ErrorKind::PermissionDenied)),
                None,
            ),
            with(
                Ok(NodeKind::Dir),
                Some(Ok(NodeKind::File {
                    len: crate::contract_interface::MAX_CATALOG_BYTES as u64 + 1,
                })),
                None,
            ),
            with(
                Ok(NodeKind::Dir),
                Some(absent),
                Some(Err(io::ErrorKind::NotADirectory)),
            ),
        ] {
            assert!(acquire(&reader).is_err());
        }
        let mut unreadable = with(
            Ok(NodeKind::Dir),
            Some(Ok(NodeKind::File { len: 10 })),
            None,
        );
        unreadable.reads.insert(plain, Read::Unreadable);
        assert!(acquire(&unreadable).is_err());
    }

    /// Case-set catalogs are discovered beside the plain one, digest-checked
    /// as the default discovery checks them, and every entry of every catalog
    /// is validated even when its key duplicates an earlier one.
    #[test]
    fn case_set_catalogs_are_complete_and_checked() {
        let digest = crate::contract_interface::sha256_digest;
        let plain = catalog_bytes(&[("App.ts", "partial-package", "partial-package")]);
        let case = catalog_bytes(&[
            ("App.ts", "partial-package", "partial-package"),
            ("Other.ts", "other-package", "other-package"),
        ]);
        let set = |case_digest: &str| {
            serde_json::to_vec(&serde_json::json!({
                "format": "solid-checker-accepted-contract-case-set",
                "caseSetVersion": 1,
                "cases": [{"catalog": "case.json", "catalogDigest": case_digest}],
            }))
            .unwrap()
        };
        let reader_for = |set_bytes: Vec<u8>, case_bytes: Vec<u8>, set_present: bool| {
            let mut reader = plain_catalog_reader(plain.clone());
            let pointer = serde_json::to_vec(&serde_json::json!({
                "format": "solid-checker-accepted-contract-case-set-pointer",
                "caseSetVersion": 1,
                "document": "sets/set.json",
                "documentDigest": digest(&set_bytes),
            }))
            .unwrap();
            for (path, bytes) in [
                ("/v/.solid-checker/accepted-contract-case-set.json", pointer),
                ("/v/.solid-checker/sets/set.json", set_bytes),
                ("/v/.solid-checker/sets/case.json", case_bytes),
            ] {
                let kind = if !set_present
                    && path.ends_with("set.json")
                    && !path.ends_with("case-set.json")
                {
                    Err(io::ErrorKind::NotFound)
                } else {
                    file_kind(&bytes)
                };
                reader.kinds.insert(PathBuf::from(path), kind);
                reader.reads.insert(PathBuf::from(path), Read::Bytes(bytes));
            }
            reader.canonical.insert(
                PathBuf::from("/v/.solid-checker/sets/App.ts"),
                Ok(PathBuf::from("/real/App.ts")),
            );
            reader.canonical.insert(
                PathBuf::from("/v/.solid-checker/sets/Other.ts"),
                Ok(PathBuf::from("/real/Other.ts")),
            );
            reader
        };
        let provenance = acquire(&reader_for(set(&digest(&case)), case.clone(), true)).unwrap();
        assert_eq!(
            provenance.catalogs.keys().cloned().collect::<Vec<_>>(),
            [
                ("/real/App.ts".to_owned(), "partial-package".to_owned()),
                ("/real/Other.ts".to_owned(), "other-package".to_owned()),
            ]
        );
        assert_eq!(
            provenance.catalogs[&("/real/App.ts".to_owned(), "partial-package".to_owned())],
            PathBuf::from("/v/.solid-checker/accepted-contracts.json"),
            "the first catalog naming a key is kept"
        );
        assert!(acquire(&reader_for(set("sha256:00"), case.clone(), true)).is_err());
        // An independently wrong pointer `documentDigest`, over a valid set.
        let mut bad_document_digest = reader_for(set(&digest(&case)), case.clone(), true);
        let pointer = serde_json::to_vec(&serde_json::json!({
            "format": "solid-checker-accepted-contract-case-set-pointer",
            "caseSetVersion": 1,
            "document": "sets/set.json",
            "documentDigest": "sha256:00",
        }))
        .unwrap();
        let pointer_path = PathBuf::from("/v/.solid-checker/accepted-contract-case-set.json");
        bad_document_digest
            .kinds
            .insert(pointer_path.clone(), file_kind(&pointer));
        bad_document_digest
            .reads
            .insert(pointer_path, Read::Bytes(pointer));
        assert!(acquire(&bad_document_digest).is_err());
        assert!(acquire(&reader_for(set(&digest(&case)), case.clone(), false)).is_err());
        let mut duplicate_invalid = reader_for(set(&digest(&case)), case, true);
        duplicate_invalid.canonical.insert(
            PathBuf::from("/v/.solid-checker/sets/App.ts"),
            Err(io::ErrorKind::PermissionDenied),
        );
        assert!(
            acquire(&duplicate_invalid).is_err(),
            "a duplicate key's entry is still validated"
        );
    }

    /// Malformed catalog or authored-index metadata refuses.
    #[test]
    fn malformed_metadata_refuses() {
        let mut wrong_format: serde_json::Value =
            serde_json::from_slice(&catalog_bytes(&[("App.ts", "p", "p")])).unwrap();
        wrong_format["catalogVersion"] = 99.into();
        let mut unknown: serde_json::Value =
            serde_json::from_slice(&catalog_bytes(&[("App.ts", "p", "p")])).unwrap();
        unknown["contracts"][0]["import"]["unexpected"] = true.into();
        for bytes in [
            b"{".to_vec(),
            serde_json::to_vec(&wrong_format).unwrap(),
            serde_json::to_vec(&unknown).unwrap(),
        ] {
            assert!(acquire(&plain_catalog_reader(bytes)).is_err());
        }
        let reader = plain_catalog_reader(catalog_bytes(&[("App.ts", "p", "p")]));
        assert!(acquire_with(&reader, Path::new("/v"), &[], b"{").is_err());
    }

    /// A nested candidate's catalog is read too, and its relative importers
    /// resolve against that candidate.
    #[test]
    fn a_nested_catalog_resolves_against_its_own_directory() {
        let bytes = catalog_bytes(&[("App.ts", "partial-package", "partial-package")]);
        let mut reader = FakeReader::default();
        reader.kinds.insert(PathBuf::from("/v"), Ok(NodeKind::Dir));
        reader.absent_entry("/v/.solid-checker");
        reader
            .kinds
            .insert(PathBuf::from("/v/pkg/.solid-checker"), Ok(NodeKind::Dir));
        reader.kinds.insert(
            PathBuf::from("/v/pkg/.solid-checker/accepted-contracts.json"),
            file_kind(&bytes),
        );
        reader.absent_entry("/v/pkg/.solid-checker/accepted-contract-case-set.json");
        reader.reads.insert(
            PathBuf::from("/v/pkg/.solid-checker/accepted-contracts.json"),
            Read::Bytes(bytes),
        );
        reader.canonical.insert(
            PathBuf::from("/v/pkg/App.ts"),
            Ok(PathBuf::from("/real/pkg/App.ts")),
        );
        let provenance = acquire_with(
            &reader,
            Path::new("/v"),
            &[PathBuf::from("/v/pkg")],
            crate::authored_contracts::embedded_index(),
        )
        .unwrap();
        assert!(
            provenance
                .catalogs
                .contains_key(&("/real/pkg/App.ts".to_owned(), "partial-package".to_owned()))
        );
    }

    /// The real filesystem end to end: the checked-in catalog of the review's
    /// counterexample fixture, read where it is, keys its import by the
    /// importer's real path, through the operational entry point.
    #[test]
    fn the_counterexample_fixture_catalog_is_acquired_from_disk() {
        let project = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/package-unknown-callback-consumer")
            .canonicalize()
            .unwrap();
        let provenance =
            UnconsumedContractProvenance::acquire(&project, &virtual_facts(Vec::new())).unwrap();
        let importer = project.join("App.ts").to_string_lossy().into_owned();
        assert_eq!(
            provenance.catalogs.keys().collect::<Vec<_>>(),
            [&(importer, "partial-package".to_owned())]
        );
        assert!(provenance.authored.contains(&(
            "@tanstack/virtual-core".to_owned(),
            "@tanstack/virtual-core".to_owned(),
            "3.17.8".to_owned()
        )));
    }

    /// A followed NotFound at an optional site is absence only when the entry
    /// itself is missing (no-follow) and its parent resolves to a directory,
    /// or is itself proved absent up the chain. Every other no-follow or
    /// parent answer refuses, and names where it stopped.
    #[test]
    fn optional_absence_is_established_or_refused() {
        let root = PathBuf::from("/v/.solid-checker");
        let base = || {
            let mut reader = FakeReader::default();
            reader
                .kinds
                .insert(root.clone(), Err(io::ErrorKind::NotFound));
            reader
        };
        // The entry answers: only NotFound continues to the parent.
        for (answer, reason) in [
            (Ok(NodeKind::Symlink), "dangling symlink"),
            (Ok(NodeKind::Dir), "does not resolve"),
            (Ok(NodeKind::File { len: 0 }), "does not resolve"),
            (
                Err(io::ErrorKind::PermissionDenied),
                "cannot be established",
            ),
            (Err(io::ErrorKind::NotADirectory), "cannot be established"),
            (Err(io::ErrorKind::Other), "cannot be established"),
            (Err(io::ErrorKind::Unsupported), "cannot be established"),
        ] {
            let mut reader = base();
            reader.entries.insert(root.clone(), answer);
            let error = acquire(&reader).unwrap_err();
            assert_eq!(error.source, root, "{answer:?}");
            assert!(error.reason.contains(reason), "{answer:?}: {error}");
        }
        // The parent answers, once the entry is missing.
        for answer in [
            Ok(NodeKind::File { len: 1 }),
            Ok(NodeKind::Other),
            Err(io::ErrorKind::PermissionDenied),
            Err(io::ErrorKind::NotADirectory),
            Err(io::ErrorKind::Other),
            Err(io::ErrorKind::Unsupported),
        ] {
            let mut reader = base();
            reader
                .entries
                .insert(root.clone(), Err(io::ErrorKind::NotFound));
            reader.kinds.insert(PathBuf::from("/v"), answer);
            let error = acquire(&reader).unwrap_err();
            assert_eq!(error.source, PathBuf::from("/v"), "{answer:?}");
        }
        // A dangling ancestor of a nested candidate refuses at that ancestor.
        let mut dangling_ancestor = FakeReader::default();
        dangling_ancestor
            .kinds
            .insert(PathBuf::from("/v"), Ok(NodeKind::Dir));
        dangling_ancestor.absent_entry("/v/.solid-checker");
        dangling_ancestor.absent_entry("/v/pkg/.solid-checker");
        dangling_ancestor
            .kinds
            .insert(PathBuf::from("/v/pkg"), Err(io::ErrorKind::NotFound));
        dangling_ancestor
            .entries
            .insert(PathBuf::from("/v/pkg"), Ok(NodeKind::Symlink));
        let error = acquire_with(
            &dangling_ancestor,
            Path::new("/v"),
            &[PathBuf::from("/v/pkg")],
            crate::authored_contracts::embedded_index(),
        )
        .unwrap_err();
        assert_eq!(error.source, PathBuf::from("/v/pkg"));
        assert!(error.reason.contains("dangling symlink"), "{error}");
        // A genuinely missing regular ancestor chain, ending at a directory,
        // is established absence.
        let mut missing_chain = FakeReader::default();
        missing_chain
            .kinds
            .insert(PathBuf::from("/v"), Ok(NodeKind::Dir));
        missing_chain.absent_entry("/v/.solid-checker");
        missing_chain.absent_entry("/v/gone/sub/.solid-checker");
        missing_chain.absent_entry("/v/gone/sub");
        missing_chain.absent_entry("/v/gone");
        let provenance = acquire_with(
            &missing_chain,
            Path::new("/v"),
            &[PathBuf::from("/v/gone/sub")],
            crate::authored_contracts::embedded_index(),
        )
        .unwrap();
        assert!(provenance.catalogs.is_empty() && !provenance.authored.is_empty());
        // A dangling plain catalog or pointer refuses at that file.
        for dangling in [
            "/v/.solid-checker/accepted-contracts.json",
            "/v/.solid-checker/accepted-contract-case-set.json",
        ] {
            let mut reader = plain_catalog_reader(catalog_bytes(&[(
                "App.ts",
                "partial-package",
                "partial-package",
            )]));
            reader
                .kinds
                .insert(PathBuf::from(dangling), Err(io::ErrorKind::NotFound));
            reader
                .entries
                .insert(PathBuf::from(dangling), Ok(NodeKind::Symlink));
            let error = acquire(&reader).unwrap_err();
            assert_eq!(error.source, PathBuf::from(dangling));
        }
    }

    /// A catalog that grows past the byte limit between its metadata and its
    /// read refuses after the read.
    #[test]
    fn a_catalog_over_the_limit_after_its_read_refuses() {
        let mut reader = plain_catalog_reader(catalog_bytes(&[("App.ts", "p", "p")]));
        reader.reads.insert(
            PathBuf::from("/v/.solid-checker/accepted-contracts.json"),
            Read::Bytes(vec![b' '; crate::contract_interface::MAX_CATALOG_BYTES + 1]),
        );
        let error = acquire(&reader).unwrap_err();
        assert!(error.reason.contains("resource limit"), "{error}");
    }

    fn write_catalog(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn acquire_disk(
        root: &Path,
        nested: &[PathBuf],
    ) -> Result<UnconsumedContractProvenance, ProvenanceRefusal> {
        acquire_with(
            &DiskReader,
            root,
            nested,
            crate::authored_contracts::embedded_index(),
        )
    }

    /// On the real filesystem: a truly absent `.solid-checker` is absence; a
    /// dangling root link, plain catalog, pointer, or nested-candidate
    /// ancestor refuses at that path with no partial result; a genuinely
    /// missing regular ancestor chain is absence.
    #[cfg(unix)]
    #[test]
    fn disk_discovery_tells_absence_from_dangling_links() {
        use std::os::unix::fs::symlink;
        let catalog = catalog_bytes(&[("App.ts", "partial-package", "partial-package")]);

        let absent = Tree::new("absent");
        let provenance = acquire_disk(&absent.0, &[]).unwrap();
        assert!(provenance.catalogs.is_empty() && !provenance.authored.is_empty());
        let provenance = acquire_disk(&absent.0, &[absent.0.join("gone/sub")]).unwrap();
        assert!(
            provenance.catalogs.is_empty(),
            "a missing regular ancestor chain"
        );

        let dangling_root = Tree::new("dangling-root");
        symlink("missing-target", dangling_root.0.join(".solid-checker")).unwrap();
        let error = acquire_disk(&dangling_root.0, &[]).unwrap_err();
        assert_eq!(error.source, dangling_root.0.join(".solid-checker"));
        assert!(error.reason.contains("dangling symlink"), "{error}");

        let dangling_plain = Tree::new("dangling-plain");
        fs::create_dir_all(dangling_plain.0.join(".solid-checker")).unwrap();
        let plain = dangling_plain
            .0
            .join(".solid-checker/accepted-contracts.json");
        symlink("missing-target", &plain).unwrap();
        let error = acquire_disk(&dangling_plain.0, &[]).unwrap_err();
        assert_eq!(error.source, plain);

        let dangling_pointer = Tree::new("dangling-pointer");
        write_catalog(
            &dangling_pointer
                .0
                .join(".solid-checker/accepted-contracts.json"),
            &catalog,
        );
        let pointer = dangling_pointer
            .0
            .join(".solid-checker/accepted-contract-case-set.json");
        symlink("missing-target", &pointer).unwrap();
        let error = acquire_disk(&dangling_pointer.0, &[]).unwrap_err();
        assert_eq!(
            error.source, pointer,
            "the valid plain catalog does not escape"
        );

        let dangling_ancestor = Tree::new("dangling-ancestor");
        symlink("missing-target", dangling_ancestor.0.join("pkg")).unwrap();
        let error =
            acquire_disk(&dangling_ancestor.0, &[dangling_ancestor.0.join("pkg")]).unwrap_err();
        assert_eq!(error.source, dangling_ancestor.0.join("pkg"));
        assert!(error.reason.contains("dangling symlink"), "{error}");
    }

    /// Valid symlinks resolving to the expected kind are followed as before,
    /// at every discovery site, and the default discovery finds the same
    /// catalogs; a normal plain plus case-set catalog on disk keys both.
    #[cfg(unix)]
    #[test]
    fn disk_discovery_follows_valid_symlinks_and_case_sets() {
        use std::os::unix::fs::symlink;
        let digest = crate::contract_interface::sha256_digest;
        let catalog = catalog_bytes(&[("App.ts", "partial-package", "partial-package")]);

        // `.solid-checker` itself, and the plain catalog, as links.
        let linked = Tree::new("linked");
        linked.file("App.ts");
        write_catalog(&linked.0.join("store/accepted-contracts.json"), &catalog);
        symlink("store", linked.0.join(".solid-checker")).unwrap();
        let app = linked.0.join("App.ts").to_string_lossy().into_owned();
        let key = (app.clone(), "partial-package".to_owned());
        assert!(
            acquire_disk(&linked.0, &[])
                .unwrap()
                .catalogs
                .contains_key(&key)
        );
        assert_eq!(
            crate::contract_interface::discovered_catalog_paths(&linked.0)
                .unwrap()
                .len(),
            1
        );
        let file_link = Tree::new("file-link");
        file_link.file("App.ts");
        write_catalog(&file_link.0.join("elsewhere/catalog.json"), &catalog);
        fs::create_dir_all(file_link.0.join(".solid-checker")).unwrap();
        symlink(
            "../elsewhere/catalog.json",
            file_link.0.join(".solid-checker/accepted-contracts.json"),
        )
        .unwrap();
        let file_key = (
            file_link.0.join("App.ts").to_string_lossy().into_owned(),
            "partial-package".to_owned(),
        );
        assert!(
            acquire_disk(&file_link.0, &[])
                .unwrap()
                .catalogs
                .contains_key(&file_key)
        );

        // A nested candidate reached through a valid directory link.
        let nested = Tree::new("nested-link");
        nested.file("real-pkg/App.ts");
        write_catalog(
            &nested
                .0
                .join("real-pkg/.solid-checker/accepted-contracts.json"),
            &catalog,
        );
        symlink("real-pkg", nested.0.join("pkg")).unwrap();
        let nested_key = (
            nested
                .0
                .join("real-pkg/App.ts")
                .to_string_lossy()
                .into_owned(),
            "partial-package".to_owned(),
        );
        assert!(
            acquire_disk(&nested.0, &[nested.0.join("pkg")])
                .unwrap()
                .catalogs
                .contains_key(&nested_key)
        );

        // A plain catalog plus a case-set pointer (itself a valid link) whose
        // case catalog names a second import.
        let sets = Tree::new("case-set");
        sets.file("App.ts");
        write_catalog(
            &sets.0.join(".solid-checker/accepted-contracts.json"),
            &catalog,
        );
        let case = catalog_bytes(&[("Other.ts", "other-package", "other-package")]);
        write_catalog(&sets.0.join(".solid-checker/sets/case.json"), &case);
        let set = serde_json::to_vec(&serde_json::json!({
            "format": "solid-checker-accepted-contract-case-set",
            "caseSetVersion": 1,
            "cases": [{"catalog": "case.json", "catalogDigest": digest(&case)}],
        }))
        .unwrap();
        write_catalog(&sets.0.join(".solid-checker/sets/set.json"), &set);
        let pointer = serde_json::to_vec(&serde_json::json!({
            "format": "solid-checker-accepted-contract-case-set-pointer",
            "caseSetVersion": 1,
            "document": "sets/set.json",
            "documentDigest": digest(&set),
        }))
        .unwrap();
        write_catalog(&sets.0.join(".solid-checker/pointer.json"), &pointer);
        symlink(
            "pointer.json",
            sets.0
                .join(".solid-checker/accepted-contract-case-set.json"),
        )
        .unwrap();
        let provenance = acquire_disk(&sets.0, &[]).unwrap();
        assert_eq!(
            provenance.catalogs.keys().cloned().collect::<Vec<_>>(),
            [
                (
                    sets.0
                        .join(".solid-checker/sets/Other.ts")
                        .to_string_lossy()
                        .into_owned(),
                    "other-package".to_owned()
                ),
                (
                    sets.0.join("App.ts").to_string_lossy().into_owned(),
                    "partial-package".to_owned()
                ),
            ]
        );
        assert_eq!(
            crate::contract_interface::discovered_catalog_paths(&sets.0)
                .unwrap()
                .len(),
            2
        );
    }
    // provenance:end

    // consumer:begin
    //
    // Claim C: every requirement acquisition keyed becomes an open requirement
    // at each attested occurrence it covers, and cannot disappear because an
    // installation inspection fails.

    fn provenance_of(
        catalog: &[(&str, &str)],
        authored: &[(&str, &str, &str)],
    ) -> UnconsumedContractProvenance {
        UnconsumedContractProvenance {
            catalogs: catalog
                .iter()
                .map(|(importer, specifier)| {
                    (
                        ((*importer).to_owned(), (*specifier).to_owned()),
                        PathBuf::from("/v/.solid-checker/accepted-contracts.json"),
                    )
                })
                .collect(),
            authored: authored
                .iter()
                .map(|(specifier, name, version)| {
                    (
                        (*specifier).to_owned(),
                        (*name).to_owned(),
                        (*version).to_owned(),
                    )
                })
                .collect(),
        }
    }

    fn consume(
        reader: &FakeReader,
        facts: &ProjectFacts,
        provenance: &UnconsumedContractProvenance,
    ) -> Result<(PackageBoundary, UnconsumedTierNotes), ProvenanceRefusal> {
        boundary_with(reader, Path::new("/v"), facts, &[], Some(provenance))
    }

    fn required(boundary: &PackageBoundary, importer: &str, specifier: &str) -> bool {
        requirement_index(boundary)
            .uncertifiable_reason(importer, specifier)
            .is_some()
    }

    /// An installed package that names no Solid package.
    fn installed_row(text: &str, name: &str) -> AttestedImport {
        virtual_row(
            text,
            ImportResolution::NodeModules,
            &format!("/v/node_modules/{name}/index.d.ts"),
            Some(name),
            Some(&format!("/v/node_modules/{name}/package.json")),
        )
    }

    fn plain_manifest(mut reader: FakeReader, name: &str) -> FakeReader {
        reader.reads.insert(
            PathBuf::from(format!("/v/node_modules/{name}/package.json")),
            Read::Bytes(format!(r#"{{"name":"{name}","version":"1.0.0"}}"#).into_bytes()),
        );
        reader
    }

    fn unresolved(text: &str) -> AttestedImport {
        virtual_row(text, ImportResolution::Unresolved, "", None, None)
    }

    /// A fake reader for one unresolved `/v/App.ts` import of `package`, with
    /// no local proposal, and `/v` and `/` as directories.
    fn unresolved_reader(package: &str) -> FakeReader {
        let mut reader = FakeReader::default();
        reader.files.insert(
            PathBuf::from(format!(
                "/v/.solid-checker/contracts/{package}/solid-reactivity.json"
            )),
            false,
        );
        reader.kinds.insert(PathBuf::from("/v"), Ok(NodeKind::Dir));
        reader.kinds.insert(PathBuf::from("/"), Ok(NodeKind::Dir));
        reader
    }

    /// `package` genuinely absent at `directory`: the candidate, the package
    /// directory and `node_modules` are all missing entries, ending at
    /// `directory`, a directory.
    fn absent_at(reader: &mut FakeReader, directory: &str, package: &str) {
        let base = if directory == "/" { "" } else { directory };
        reader.absent_entry(&format!("{base}/node_modules/{package}/package.json"));
        reader.absent_entry(&format!("{base}/node_modules/{package}"));
        reader.absent_entry(&format!("{base}/node_modules"));
    }

    #[test]
    fn a_cataloged_installed_import_stays_open_without_its_contract() {
        let reader = plain_manifest(FakeReader::default(), "partial-package");
        let facts = virtual_facts(vec![(
            "/v/App.ts",
            installed_row("partial-package", "partial-package"),
        )]);
        let (boundary, notes) = consume(
            &reader,
            &facts,
            &provenance_of(&[("/v/App.ts", "partial-package")], &[]),
        )
        .unwrap();
        assert!(required(&boundary, "/v/App.ts", "partial-package"));
        assert_eq!(
            notes,
            [(
                ("/v/App.ts".to_owned(), "partial-package".to_owned()),
                Some("package contracts are not consumed in this check (--no-package-contracts), and the project contract catalog .solid-checker/accepted-contracts.json names this import; nothing else establishes this import's reactive behavior".to_owned())
            )]
        );
        // The control: no provenance, no requirement for this plain install.
        let (plain, notes) = consume(&reader, &facts, &provenance_of(&[], &[])).unwrap();
        assert!(plain.is_empty() && notes.is_empty());
    }

    #[test]
    fn project_source_and_unkeyed_occurrences_are_never_covered() {
        // The relative row's package-local test walks the importer's owners.
        let reader = FakeReader::default()
            .read("/v/package.json", Read::Absent)
            .read("/package.json", Read::Absent);
        let facts = virtual_facts(vec![
            (
                "/v/App.ts",
                virtual_row(
                    "reactive-package",
                    ImportResolution::NonRelative,
                    "/v/src/local-impl.ts",
                    None,
                    None,
                ),
            ),
            (
                "/v/Other.ts",
                virtual_row(
                    "./local",
                    ImportResolution::Relative,
                    "/v/local.ts",
                    None,
                    None,
                ),
            ),
        ]);
        let (boundary, notes) = consume(
            &reader,
            &facts,
            &provenance_of(
                &[
                    ("/v/App.ts", "reactive-package"),
                    ("/v/Other.ts", "./local"),
                ],
                &[],
            ),
        )
        .unwrap();
        assert!(boundary.is_empty() && notes.is_empty());
    }

    /// An unresolved catalog-covered occurrence: installed raises the
    /// requirement; genuinely absent at every ancestor raises nothing (that is
    /// TypeScript's missing module); an uncovered unresolved import is never
    /// inspected at all.
    #[test]
    fn an_unresolved_covered_occurrence_needs_installation_or_proved_absence() {
        let mut installed = unresolved_reader("ambient-package");
        installed.kinds.insert(
            PathBuf::from("/v/node_modules/ambient-package/package.json"),
            Ok(NodeKind::File { len: 2 }),
        );
        let facts = virtual_facts(vec![("/v/App.ts", unresolved("ambient-package"))]);
        let (boundary, _) = consume(
            &installed,
            &facts,
            &provenance_of(&[("/v/App.ts", "ambient-package")], &[]),
        )
        .unwrap();
        assert!(required(&boundary, "/v/App.ts", "ambient-package"));

        let mut missing = unresolved_reader("missing-package");
        absent_at(&mut missing, "/v", "missing-package");
        absent_at(&mut missing, "/", "missing-package");
        let facts = virtual_facts(vec![("/v/App.ts", unresolved("missing-package"))]);
        let (boundary, notes) = consume(
            &missing,
            &facts,
            &provenance_of(&[("/v/App.ts", "missing-package")], &[]),
        )
        .unwrap();
        assert!(boundary.is_empty() && notes.is_empty());

        // Not keyed: the reader holds no installation answer, and none is asked.
        let uncovered = unresolved_reader("other-package");
        let facts = virtual_facts(vec![("/v/App.ts", unresolved("other-package"))]);
        assert!(
            consume(&uncovered, &facts, &provenance_of(&[], &[]))
                .unwrap()
                .0
                .is_empty()
        );
    }

    /// Every installation answer that is neither a regular file nor proved
    /// absence refuses the operation; nothing becomes a clean success.
    #[test]
    fn an_uninspectable_installation_refuses() {
        let candidate = PathBuf::from("/v/node_modules/ambient-package/package.json");
        let facts = virtual_facts(vec![("/v/App.ts", unresolved("ambient-package"))]);
        let provenance = provenance_of(&[("/v/App.ts", "ambient-package")], &[]);
        let mut cases = Vec::new();
        for answer in [
            Ok(NodeKind::Dir),
            Ok(NodeKind::Other),
            Err(io::ErrorKind::PermissionDenied),
            Err(io::ErrorKind::NotADirectory),
            Err(io::ErrorKind::Other),
            Err(io::ErrorKind::Unsupported),
        ] {
            let mut reader = unresolved_reader("ambient-package");
            reader.kinds.insert(candidate.clone(), answer);
            cases.push((format!("{answer:?}"), reader, candidate.clone()));
        }
        // A dangling package link: the candidate is missing, and so is the
        // package directory, which is a symlink to nothing.
        let mut dangling = unresolved_reader("ambient-package");
        dangling.absent_entry("/v/node_modules/ambient-package/package.json");
        dangling.kinds.insert(
            PathBuf::from("/v/node_modules/ambient-package"),
            Err(io::ErrorKind::NotFound),
        );
        dangling.entries.insert(
            PathBuf::from("/v/node_modules/ambient-package"),
            Ok(NodeKind::Symlink),
        );
        cases.push((
            "dangling package link".to_owned(),
            dangling,
            PathBuf::from("/v/node_modules/ambient-package"),
        ));
        // A dangling `package.json` link itself.
        let mut dangling_manifest = unresolved_reader("ambient-package");
        dangling_manifest
            .kinds
            .insert(candidate.clone(), Err(io::ErrorKind::NotFound));
        dangling_manifest
            .entries
            .insert(candidate.clone(), Ok(NodeKind::Symlink));
        cases.push((
            "dangling manifest link".to_owned(),
            dangling_manifest,
            candidate.clone(),
        ));
        for (label, reader, source) in cases {
            let error = consume(&reader, &facts, &provenance).unwrap_err();
            assert_eq!(error.source, source, "{label}: {error}");
        }
        // A covered unresolved specifier that names no package refuses too.
        let facts = virtual_facts(vec![("/v/App.ts", unresolved("./nowhere"))]);
        let mut reader = unresolved_reader("./nowhere");
        reader.files.insert(
            PathBuf::from("/v/.solid-checker/contracts/./solid-reactivity.json"),
            false,
        );
        assert!(
            consume(
                &reader,
                &facts,
                &provenance_of(&[("/v/App.ts", "./nowhere")], &[])
            )
            .is_err()
        );
    }

    #[test]
    fn the_authored_tier_covers_only_its_exact_specifier_name_and_version() {
        let reader = plain_manifest(FakeReader::default(), "@tanstack/virtual-core");
        let resolved = |text: &str, version: &str| {
            let mut row = installed_row(text, "@tanstack/virtual-core");
            row.resolver_package_name = Some("@tanstack/virtual-core".into());
            row.resolver_package_version = Some(version.into());
            row
        };
        let facts = virtual_facts(vec![
            ("/v/a.ts", resolved("@tanstack/virtual-core", "3.17.8")),
            ("/v/b.ts", resolved("@tanstack/virtual-core", "3.0.0")),
            ("/v/c.ts", resolved("@tanstack/virtual-core/sub", "3.17.8")),
        ]);
        let (boundary, notes) = consume(
            &reader,
            &facts,
            &provenance_of(
                &[],
                &[("@tanstack/virtual-core", "@tanstack/virtual-core", "3.17.8")],
            ),
        )
        .unwrap();
        assert!(required(&boundary, "/v/a.ts", "@tanstack/virtual-core"));
        assert!(!required(&boundary, "/v/b.ts", "@tanstack/virtual-core"));
        assert!(!required(
            &boundary,
            "/v/c.ts",
            "@tanstack/virtual-core/sub"
        ));
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn selected_source_and_solid_core_stay_exempt() {
        let reader = plain_manifest(FakeReader::default(), "partial-package");
        let mut facts = virtual_facts(vec![
            (
                "/v/App.ts",
                installed_row("partial-package", "partial-package"),
            ),
            ("/v/App.ts", installed_row("solid-js", "solid-js")),
        ]);
        facts.dependency_source_roots = vec!["/v/node_modules/partial-package".into()];
        let (boundary, notes) = consume(
            &reader,
            &facts,
            &provenance_of(
                &[("/v/App.ts", "partial-package"), ("/v/App.ts", "solid-js")],
                &[],
            ),
        )
        .unwrap();
        assert!(boundary.is_empty() && notes.is_empty());
    }
    // consumer:end
}
