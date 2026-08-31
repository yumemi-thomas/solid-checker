//! Exact package-artifact resolution and canonical closure identity.
//!
//! This module is the resolver seam shared by host/Type Facts attestations and
//! standalone package acquisition. Callers supply exact resolver results; the
//! implementation validates, materializes, hashes, selects, and binds them.
//! Analyzer consumers never inspect export-map syntax, filesystem spellings,
//! closure manifests, or hazard records.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{
    ArtifactCase, ArtifactIdentity, ClaimDomain, ContractProposal, Digest, ExportTargetIdentity,
    NormalizedContract, PackageIdentity, ResolutionStep, StabilityKnowledge,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

use crate::contract_interface::{ContractFailure, invalid_identity};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResolutionAuthority {
    Host,
    TypeFacts,
    StandalonePackageResolver,
}

/// One materialized file selected by a resolver. `path` is the host-visible
/// spelling and `real_path`, when present, is the resolver's symlink target.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_path: Option<String>,
    pub digest: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolutionTrace {
    /// Exact JSON pointer (or an explicitly named legacy branch) selected by
    /// the resolver. This is compared to the contract case; `steps` are
    /// retained provenance and never used as a second matcher.
    #[serde(default)]
    pub branch: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<ResolutionTraceStep>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolutionTraceStep {
    pub condition: String,
    pub target: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClosureFileRole {
    Runtime,
    Declaration,
    Manifest,
    ResolutionInput,
    LiteralDynamicChunk,
    Generated,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClosureEntry {
    pub role: ClosureFileRole,
    /// Canonical package-relative path, or `virtual:<stable-id>` for
    /// materialized compiler/loader output.
    pub path: String,
    pub digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_digest: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptedDependencyEdge {
    pub specifier: String,
    pub package_name: String,
    pub artifact_case: String,
    pub accepted_contract_digest: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClosureHazardKind {
    NonliteralDynamicLoading,
    Eval,
    NativeCode,
    OpaqueWasm,
    MutableUnboundGlobal,
    UnmaterializedTransform,
    UnacceptedExternalDependency,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AffectedClaimDomain {
    Callbacks,
    Reads,
    Writes,
    Creates,
    Invalidates,
    Throws,
    Returns,
    Cleanups,
    Disposals,
}

impl From<AffectedClaimDomain> for ClaimDomain {
    fn from(value: AffectedClaimDomain) -> Self {
        match value {
            AffectedClaimDomain::Callbacks => Self::Callbacks,
            AffectedClaimDomain::Reads => Self::Reads,
            AffectedClaimDomain::Writes => Self::Writes,
            AffectedClaimDomain::Creates => Self::Creates,
            AffectedClaimDomain::Invalidates => Self::Invalidates,
            AffectedClaimDomain::Throws => Self::Throws,
            AffectedClaimDomain::Returns => Self::Returns,
            AffectedClaimDomain::Cleanups => Self::Cleanups,
            AffectedClaimDomain::Disposals => Self::Disposals,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClosureHazard {
    pub kind: ClosureHazardKind,
    pub source: String,
    /// Empty means every modeled export in the selected case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_exports: Vec<String>,
    pub affected_domains: Vec<AffectedClaimDomain>,
}

/// Exact published package instance included in a finite dependency census.
///
/// This is distinct from an accepted semantic dependency edge: it proves the
/// bytes that were classified even when no behavior from that dependency is
/// queried. `files_manifest_digest` hashes the sorted `sha256  path` manifest
/// of every regular file in the installed package.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClosurePackageIdentity {
    pub name: String,
    pub version: String,
    pub integrity: String,
    pub files_manifest_digest: String,
}

/// Canonical, replayable identity of one artifact case's local files,
/// accepted external edges, and exact open frontiers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClosureManifest {
    pub entries: Vec<ClosureEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<AcceptedDependencyEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hazards: Vec<ClosureHazard>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<ClosurePackageIdentity>,
    pub digest: String,
}

/// Input to canonical closure materialization. Local files are read and
/// hashed here; generated/virtual output must supply both bytes and the exact
/// transform identity that produced them.
#[derive(Clone, Debug)]
pub enum ClosureInput {
    File {
        role: ClosureFileRole,
        path: PathBuf,
    },
    Generated {
        stable_id: String,
        bytes: Vec<u8>,
        transform: ResolvedFile,
    },
}

impl ClosureManifest {
    pub fn materialize(
        package_root: &Path,
        inputs: impl IntoIterator<Item = ClosureInput>,
        dependencies: Vec<AcceptedDependencyEdge>,
        hazards: Vec<ClosureHazard>,
    ) -> Result<Self, ArtifactResolutionFailure> {
        let canonical_root =
            fs::canonicalize(package_root).map_err(|error| ArtifactResolutionFailure::Invalid {
                reason: format!(
                    "package root {} cannot be canonicalized: {error}",
                    package_root.display()
                ),
            })?;
        let mut entries = Vec::new();
        for input in inputs {
            match input {
                ClosureInput::File { role, path } => {
                    let canonical = fs::canonicalize(&path).map_err(|error| {
                        ArtifactResolutionFailure::Invalid {
                            reason: format!(
                                "closure file {} cannot be read: {error}",
                                path.display()
                            ),
                        }
                    })?;
                    if !canonical.starts_with(&canonical_root) {
                        return Err(ArtifactResolutionFailure::Invalid {
                            reason: format!(
                                "closure file {} escapes package root {} through a symlink",
                                path.display(),
                                package_root.display()
                            ),
                        });
                    }
                    let relative = path
                        .strip_prefix(package_root)
                        .or_else(|_| canonical.strip_prefix(&canonical_root))
                        .map_err(|_| ArtifactResolutionFailure::Invalid {
                            reason: format!(
                                "closure file {} is outside package root {}",
                                path.display(),
                                package_root.display()
                            ),
                        })?;
                    let relative = canonical_package_path(relative)?;
                    let bytes = fs::read(&canonical).map_err(|error| {
                        ArtifactResolutionFailure::Invalid {
                            reason: format!(
                                "closure file {} cannot be read: {error}",
                                path.display()
                            ),
                        }
                    })?;
                    entries.push(ClosureEntry {
                        role,
                        path: relative,
                        digest: sha256_digest(&bytes),
                        transform_digest: None,
                    });
                }
                ClosureInput::Generated {
                    stable_id,
                    bytes,
                    transform,
                } => {
                    validate_identifier(&stable_id, "generated output identity")?;
                    validate_resolved_file(&transform)?;
                    entries.push(ClosureEntry {
                        role: ClosureFileRole::Generated,
                        path: format!("virtual:{stable_id}"),
                        digest: sha256_digest(&bytes),
                        transform_digest: Some(normalize_digest(&transform.digest)?),
                    });
                }
            }
        }
        Self::new(entries, dependencies, hazards)
    }

    pub fn new(
        mut entries: Vec<ClosureEntry>,
        mut dependencies: Vec<AcceptedDependencyEdge>,
        mut hazards: Vec<ClosureHazard>,
    ) -> Result<Self, ArtifactResolutionFailure> {
        for entry in &mut entries {
            validate_closure_path(&entry.path)?;
            entry.digest = normalize_digest(&entry.digest)?;
            if entry.role == ClosureFileRole::Generated {
                let transform = entry.transform_digest.as_mut().ok_or_else(|| {
                    ArtifactResolutionFailure::Invalid {
                        reason: format!(
                            "generated closure entry {:?} has no transform identity",
                            entry.path
                        ),
                    }
                })?;
                *transform = normalize_digest(transform)?;
            } else if entry.transform_digest.is_some() {
                return Err(ArtifactResolutionFailure::Invalid {
                    reason: format!(
                        "non-generated closure entry {:?} carries a transform identity",
                        entry.path
                    ),
                });
            }
        }
        entries.sort();
        reject_conflicting_entries(&entries)?;
        entries.dedup();

        for dependency in &mut dependencies {
            validate_identifier(&dependency.specifier, "dependency specifier")?;
            validate_identifier(&dependency.package_name, "dependency package name")?;
            validate_identifier(&dependency.artifact_case, "dependency artifact case")?;
            dependency.accepted_contract_digest =
                normalize_digest(&dependency.accepted_contract_digest)?;
        }
        dependencies.sort();
        reject_conflicting_dependencies(&dependencies)?;
        dependencies.dedup();

        for hazard in &mut hazards {
            validate_identifier(&hazard.source, "closure hazard source")?;
            if hazard.affected_domains.is_empty() {
                return Err(ArtifactResolutionFailure::Invalid {
                    reason: format!(
                        "closure hazard {:?} at {:?} names no affected claim domain",
                        hazard.kind, hazard.source
                    ),
                });
            }
            hazard.affected_exports.sort();
            hazard.affected_exports.dedup();
            hazard.affected_domains.sort();
            hazard.affected_domains.dedup();
        }
        hazards.sort();
        hazards.dedup();

        let digest = closure_digest(&entries, &dependencies, &hazards);
        Ok(Self {
            entries,
            dependencies,
            hazards,
            packages: Vec::new(),
            digest,
        })
    }

    /// Builds the exact finite published-package closure used by first-party
    /// conformance. It cannot be mixed with a file-edge closure: the two
    /// shapes have different independently replayable census authorities.
    pub fn from_package_census(
        mut packages: Vec<ClosurePackageIdentity>,
    ) -> Result<Self, ArtifactResolutionFailure> {
        if packages.is_empty() {
            return invalid_resolution("package closure census must not be empty");
        }
        for package in &mut packages {
            validate_identifier(&package.name, "closure package name")?;
            validate_identifier(&package.version, "closure package version")?;
            if !package.integrity.starts_with("sha512-") {
                return invalid_resolution("closure package integrity must be an exact SRI");
            }
            package.files_manifest_digest = normalize_digest(&package.files_manifest_digest)?;
        }
        // Preserve the checked Phase 13 census ordering exactly: the authority
        // sorts the complete rendered identity line, not the package-name
        // field in isolation (notably `seroval-plugins` sorts before
        // `seroval` because `-` precedes `@`).
        packages.sort_by_cached_key(package_census_line);
        if packages.windows(2).any(|pair| pair[0].name == pair[1].name) {
            return invalid_resolution("package closure census repeats a package name");
        }
        let digest = package_census_digest(&packages);
        Ok(Self {
            entries: Vec::new(),
            dependencies: Vec::new(),
            hazards: Vec::new(),
            packages,
            digest,
        })
    }

    pub fn validate(&self) -> Result<(), ArtifactResolutionFailure> {
        let rebuilt = if self.packages.is_empty() {
            Self::new(
                self.entries.clone(),
                self.dependencies.clone(),
                self.hazards.clone(),
            )?
        } else {
            if !self.entries.is_empty() || !self.dependencies.is_empty() || !self.hazards.is_empty()
            {
                return invalid_resolution(
                    "package closure census cannot be mixed with file, dependency, or hazard rows",
                );
            }
            Self::from_package_census(self.packages.clone())?
        };
        if rebuilt.digest != normalize_digest(&self.digest)? {
            return Err(ArtifactResolutionFailure::Invalid {
                reason: "dependency closure digest does not match its canonical manifest".into(),
            });
        }
        if rebuilt.entries != self.entries
            || rebuilt.dependencies != self.dependencies
            || rebuilt.hazards != self.hazards
            || rebuilt.packages != self.packages
        {
            return Err(ArtifactResolutionFailure::Invalid {
                reason: "dependency closure manifest is not in canonical order".into(),
            });
        }
        Ok(())
    }

    fn contains(&self, role: ClosureFileRole, path: &str, digest: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.role == role && entry.path == path && entry.digest == digest)
    }

    fn open_domains(&self, export: &str) -> BTreeSet<ClaimDomain> {
        self.hazards
            .iter()
            .filter(|hazard| {
                hazard.affected_exports.is_empty()
                    || hazard.affected_exports.iter().any(|name| name == export)
            })
            .flat_map(|hazard| hazard.affected_domains.iter().copied().map(Into::into))
            .collect()
    }
}

fn package_census_digest(packages: &[ClosurePackageIdentity]) -> String {
    let mut input = String::from("solid-checker:phase13-rc3-closure:v1\n");
    for package in packages {
        input.push_str(&package_census_line(package));
        input.push('\n');
    }
    format!("sha256:{:x}", Sha256::digest(input.as_bytes()))
}

fn package_census_line(package: &ClosurePackageIdentity) -> String {
    format!(
        "{}@{}\t{}\t{}",
        package.name,
        package.version,
        package.integrity,
        package
            .files_manifest_digest
            .strip_prefix("sha256:")
            .expect("package census digests are normalized")
    )
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedExportTarget {
    pub module: ResolvedFile,
    pub export_name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedExportBinding {
    pub runtime: ResolvedExportTarget,
    pub declarations: ResolvedExportTarget,
}

/// Complete exact resolution record consumed by contract selection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedImport {
    pub specifier: String,
    pub importer: String,
    pub requested_entrypoint: String,
    pub package_name: String,
    pub package_version: String,
    pub package_integrity: String,
    pub package_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_real_root: Option<String>,
    pub package_manifest: ResolvedFile,
    pub runtime: ResolvedFile,
    pub declarations: ResolvedFile,
    #[serde(default)]
    pub runtime_trace: ResolutionTrace,
    #[serde(default)]
    pub declaration_trace: ResolutionTrace,
    pub closure: ClosureManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<ResolvedFile>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub exports: BTreeMap<String, ResolvedExportBinding>,
    pub authority: ResolutionAuthority,
}

impl ResolvedImport {
    pub fn validate(&self) -> Result<(), ArtifactResolutionFailure> {
        for (value, field) in [
            (&self.specifier, "specifier"),
            (&self.importer, "importer"),
            (&self.requested_entrypoint, "requested entrypoint"),
            (&self.package_name, "package name"),
            (&self.package_version, "package version"),
            (&self.package_integrity, "package integrity"),
            (&self.package_root, "package root"),
        ] {
            validate_identifier(value, field)?;
        }
        if !Path::new(&self.package_root).is_absolute() {
            return invalid_resolution("package root must be absolute");
        }
        validate_import_coordinates(
            &self.specifier,
            &self.package_name,
            &self.requested_entrypoint,
        )?;
        if let Some(real_root) = &self.package_real_root
            && !Path::new(real_root).is_absolute()
        {
            return invalid_resolution("package real root must be absolute");
        }
        validate_resolved_file(&self.package_manifest)?;
        validate_resolved_file(&self.runtime)?;
        validate_resolved_file(&self.declarations)?;
        if let Some(transform) = &self.transform {
            validate_resolved_file(transform)?;
        }
        validate_trace(&self.runtime_trace, "runtime")?;
        validate_trace(&self.declaration_trace, "declaration")?;
        self.closure.validate()?;
        for (name, binding) in &self.exports {
            validate_identifier(name, "export name")?;
            validate_identifier(&binding.runtime.export_name, "runtime export target")?;
            validate_identifier(
                &binding.declarations.export_name,
                "declaration export target",
            )?;
            validate_resolved_file(&binding.runtime.module)?;
            validate_resolved_file(&binding.declarations.module)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ImportRequest {
    pub specifier: String,
    pub importer: String,
    pub export_conditions: Vec<String>,
}

pub trait ArtifactResolver {
    fn resolve(&self, request: &ImportRequest)
    -> Result<ResolvedImport, ArtifactResolutionFailure>;
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ArtifactResolutionFailure {
    #[error("the resolver did not attest this exact import")]
    Unattested,
    #[error("the resolver returned more than one result for this exact import")]
    Ambiguous,
    #[error("the resolved import is structurally invalid: {reason}")]
    Invalid { reason: String },
}

#[derive(Clone, Debug, Default)]
pub struct HostResolutionAdapter(ResolutionRows);

#[derive(Clone, Debug, Default)]
pub struct TypeFactsResolutionAdapter(ResolutionRows);

#[derive(Clone, Debug, Default)]
pub struct StandaloneResolutionAdapter(ResolutionRows);

#[derive(Clone, Debug, Default)]
struct ResolutionRows(BTreeMap<ImportRequest, Vec<ResolvedImport>>);

macro_rules! resolution_adapter {
    ($adapter:ty, $authority:expr) => {
        impl $adapter {
            #[must_use]
            pub fn from_rows(
                rows: impl IntoIterator<Item = (ImportRequest, ResolvedImport)>,
            ) -> Self {
                Self(collect_resolution_rows(rows, $authority))
            }
        }

        impl ArtifactResolver for $adapter {
            fn resolve(
                &self,
                request: &ImportRequest,
            ) -> Result<ResolvedImport, ArtifactResolutionFailure> {
                exact_resolution(&self.0, request)
            }
        }
    };
}

resolution_adapter!(HostResolutionAdapter, ResolutionAuthority::Host);
resolution_adapter!(TypeFactsResolutionAdapter, ResolutionAuthority::TypeFacts);
resolution_adapter!(
    StandaloneResolutionAdapter,
    ResolutionAuthority::StandalonePackageResolver
);

/// Resolver precedence is structural: an invalid or ambiguous higher-authority
/// result is refused, not silently replaced by a friendlier lower-authority
/// answer. Only an unattested result falls through.
pub struct ArtifactResolverChain<'a> {
    pub host: Option<&'a dyn ArtifactResolver>,
    pub typefacts: Option<&'a dyn ArtifactResolver>,
    pub standalone: Option<&'a dyn ArtifactResolver>,
}

impl ArtifactResolver for ArtifactResolverChain<'_> {
    fn resolve(
        &self,
        request: &ImportRequest,
    ) -> Result<ResolvedImport, ArtifactResolutionFailure> {
        for resolver in [self.host, self.typefacts, self.standalone]
            .into_iter()
            .flatten()
        {
            match resolver.resolve(request) {
                Err(ArtifactResolutionFailure::Unattested) => {}
                answer => return answer,
            }
        }
        Err(ArtifactResolutionFailure::Unattested)
    }
}

fn collect_resolution_rows(
    rows: impl IntoIterator<Item = (ImportRequest, ResolvedImport)>,
    authority: ResolutionAuthority,
) -> ResolutionRows {
    let mut collected: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (request, mut resolved) in rows {
        resolved.authority = authority;
        collected.entry(request).or_default().push(resolved);
    }
    ResolutionRows(collected)
}

fn exact_resolution(
    rows: &ResolutionRows,
    request: &ImportRequest,
) -> Result<ResolvedImport, ArtifactResolutionFailure> {
    match rows.0.get(request).map(Vec::as_slice) {
        None | Some([]) => Err(ArtifactResolutionFailure::Unattested),
        Some([resolved])
            if resolved.specifier == request.specifier && resolved.importer == request.importer =>
        {
            resolved.validate()?;
            Ok(resolved.clone())
        }
        Some([_]) => {
            invalid_resolution("the result does not identify the requested specifier and importer")
        }
        Some(_) => Err(ArtifactResolutionFailure::Ambiguous),
    }
}

/// Selects exactly one normalized artifact case and replaces its provisional
/// Phase 6 export identities with independently resolved runtime/declaration
/// targets. Opaque closure hazards weaken only their named call domains.
pub(crate) fn select_and_bind(
    contract: &NormalizedContract,
    resolved: &ResolvedImport,
) -> Result<NormalizedContract, ContractFailure> {
    select_and_bind_with_external_targets(contract, resolved, &BTreeSet::new())
}

pub(crate) fn select_and_bind_with_external_targets(
    contract: &NormalizedContract,
    resolved: &ResolvedImport,
    external_targets: &BTreeSet<(String, String)>,
) -> Result<NormalizedContract, ContractFailure> {
    resolved
        .validate()
        .map_err(|error| invalid_identity(error.to_string()))?;
    validate_package_identity(contract, resolved)?;

    let matches = contract
        .artifact_cases()
        .iter()
        .filter(|case| artifact_case_matches(case, resolved))
        .collect::<Vec<_>>();
    let selected = match matches.as_slice() {
        [] => return Err(ContractFailure::NoArtifactCase),
        [selected] => (*selected).clone(),
        _ => return Err(ContractFailure::MultipleArtifactCases),
    };
    let selected = bind_exports(selected, resolved, external_targets)?;
    ContractProposal::new(contract.package().clone(), vec![selected])
        .normalize()
        .map_err(|error| ContractFailure::InvalidSemanticModel {
            reason: error.to_string(),
        })
}

pub(crate) fn resolved_external_export_targets(
    resolved: &ResolvedImport,
) -> Result<BTreeSet<(String, String)>, ContractFailure> {
    let mut targets = BTreeSet::new();
    for target in resolved
        .exports
        .values()
        .flat_map(|binding| [&binding.runtime, &binding.declarations])
    {
        let relative = package_relative_path(&target.module, resolved);
        let nested_package = relative.as_deref().is_some_and(|path| {
            Path::new(path)
                .components()
                .any(|component| component.as_os_str() == "node_modules")
        });
        if relative.is_none() || nested_package {
            targets.insert((
                target.module.path.clone(),
                normalize_digest(&target.module.digest)
                    .map_err(|error| invalid_identity(error.to_string()))?,
            ));
        }
    }
    Ok(targets)
}

/// Creates the exact package and empty artifact-case identities used by the
/// Rust proposal generator. Export semantics are added by the analysis owner
/// and rebound through [`select_and_bind`] before emission.
pub(crate) fn proposal_identity(
    resolved: &ResolvedImport,
) -> Result<(PackageIdentity, ArtifactCase), ContractFailure> {
    resolved
        .validate()
        .map_err(|error| invalid_identity(error.to_string()))?;
    let manifest = semantic_artifact(&resolved.package_manifest, resolved)?;
    let runtime = semantic_artifact(&resolved.runtime, resolved)?;
    let declarations = semantic_artifact(&resolved.declarations, resolved)?;
    let transform = resolved
        .transform
        .as_ref()
        .map(|file| semantic_artifact(file, resolved))
        .transpose()?;
    let resolution_trace = if resolved.runtime_trace.branch.is_empty()
        && resolved.declaration_trace.branch.is_empty()
    {
        Vec::new()
    } else {
        if resolved.runtime_trace.branch.is_empty() || resolved.declaration_trace.branch.is_empty()
        {
            return Err(invalid_identity(
                "runtime and declaration resolution branches must both be present",
            ));
        }
        vec![
            ResolutionStep {
                condition: "runtime".into(),
                target: resolved.runtime_trace.branch.clone(),
            },
            ResolutionStep {
                condition: "types".into(),
                target: resolved.declaration_trace.branch.clone(),
            },
        ]
    };
    let identity = format!(
        "proposal:{}:{}:{}",
        resolved.package_name,
        resolved.requested_entrypoint,
        resolved.closure.digest.trim_start_matches("sha256:")
    );
    Ok((
        PackageIdentity {
            name: resolved.package_name.clone(),
            version: resolved.package_version.clone(),
            integrity: resolved.package_integrity.clone(),
            manifest,
        },
        ArtifactCase {
            id: identity,
            entrypoint: resolved.requested_entrypoint.clone(),
            resolution_trace,
            runtime,
            declarations,
            dependency_closure: Digest::parse(&resolved.closure.digest)
                .map_err(|error| invalid_identity(error.to_string()))?,
            transform,
            stability: StabilityKnowledge::Unknown,
            exports: BTreeMap::new(),
        },
    ))
}

fn semantic_artifact(
    file: &ResolvedFile,
    resolved: &ResolvedImport,
) -> Result<ArtifactIdentity, ContractFailure> {
    let path = package_relative_path(file, resolved)
        .ok_or_else(|| invalid_identity("resolved artifact is outside its package root"))?;
    Ok(ArtifactIdentity {
        path,
        digest: Digest::parse(
            normalize_digest(&file.digest).map_err(|error| invalid_identity(error.to_string()))?,
        )
        .map_err(|error| invalid_identity(error.to_string()))?,
    })
}

fn validate_package_identity(
    contract: &NormalizedContract,
    resolved: &ResolvedImport,
) -> Result<(), ContractFailure> {
    let package = contract.package();
    for (field, expected, actual) in [
        (
            "package name",
            package.name.as_str(),
            resolved.package_name.as_str(),
        ),
        (
            "package version",
            package.version.as_str(),
            resolved.package_version.as_str(),
        ),
        (
            "package integrity",
            package.integrity.as_str(),
            resolved.package_integrity.as_str(),
        ),
    ] {
        if expected != actual {
            return Err(invalid_identity(format!(
                "{field} is {actual:?}; contract requires {expected:?}"
            )));
        }
    }
    if !artifact_matches(&package.manifest, &resolved.package_manifest, resolved) {
        return Err(invalid_identity(
            "package manifest path or digest does not match the resolved package",
        ));
    }
    Ok(())
}

fn artifact_case_matches(case: &ArtifactCase, resolved: &ResolvedImport) -> bool {
    case.entrypoint == resolved.requested_entrypoint
        && artifact_matches(&case.runtime, &resolved.runtime, resolved)
        && artifact_matches(&case.declarations, &resolved.declarations, resolved)
        && case.dependency_closure.as_str() == resolved.closure.digest
        && match (&case.transform, &resolved.transform) {
            (None, None) => true,
            (Some(expected), Some(actual)) => artifact_matches(expected, actual, resolved),
            _ => false,
        }
        && trace_matches(case, resolved)
}

fn trace_matches(case: &ArtifactCase, resolved: &ResolvedImport) -> bool {
    if case.resolution_trace.is_empty() {
        return true;
    }
    let runtime = case
        .resolution_trace
        .iter()
        .find(|step| step.condition == "runtime")
        .map(|step| step.target.as_str());
    let declarations = case
        .resolution_trace
        .iter()
        .find(|step| step.condition == "types")
        .map(|step| step.target.as_str());
    runtime == Some(resolved.runtime_trace.branch.as_str())
        && declarations == Some(resolved.declaration_trace.branch.as_str())
}

fn bind_exports(
    mut case: ArtifactCase,
    resolved: &ResolvedImport,
    external_targets: &BTreeSet<(String, String)>,
) -> Result<ArtifactCase, ContractFailure> {
    for (name, export) in &mut case.exports {
        let binding = resolved.exports.get(name).ok_or_else(|| {
            invalid_identity(format!(
                "resolved artifact has no exact runtime/declaration binding for export {name:?}"
            ))
        })?;
        export.identity.runtime = bind_export_target(
            &binding.runtime,
            ClosureFileRole::Runtime,
            resolved,
            name,
            external_targets,
        )?;
        export.identity.declarations = bind_export_target(
            &binding.declarations,
            ClosureFileRole::Declaration,
            resolved,
            name,
            external_targets,
        )?;
        export.open_call_domains(resolved.closure.open_domains(name));
    }
    Ok(case)
}

fn bind_export_target(
    target: &ResolvedExportTarget,
    role: ClosureFileRole,
    resolved: &ResolvedImport,
    public_name: &str,
    external_targets: &BTreeSet<(String, String)>,
) -> Result<ExportTargetIdentity, ContractFailure> {
    let digest = normalize_digest(&target.module.digest)
        .map_err(|error| invalid_identity(error.to_string()))?;
    let root = match role {
        ClosureFileRole::Runtime => &resolved.runtime,
        ClosureFileRole::Declaration => &resolved.declarations,
        _ => unreachable!("export targets are runtime or declarations"),
    };
    if external_targets.contains(&(target.module.path.clone(), digest.clone())) {
        // An installed nested dependency is lexically below the parent
        // package root, but it is not a member of the parent's authenticated
        // archive. Exact child target identity takes precedence over that
        // filesystem prefix.
        let module = semantic_artifact(root, resolved)?;
        return Ok(ExportTargetIdentity {
            module,
            export_name: target.export_name.clone(),
        });
    }
    let path = package_relative_path(&target.module, resolved).ok_or_else(|| {
        invalid_identity(format!(
            "{role:?} target for export {public_name:?} is outside the resolved package"
        ))
    })?;
    let is_root = package_relative_path(root, resolved).as_deref() == Some(path.as_str())
        && normalize_digest(&root.digest).ok().as_deref() == Some(digest.as_str());
    // A checked whole-package census binds every regular file in the exact
    // published closure, so re-export targets do not need duplicate per-file
    // rows. File-edge closures still require every non-root binding to appear
    // explicitly.
    if !is_root
        && resolved.closure.packages.is_empty()
        && !resolved.closure.contains(role, &path, &digest)
    {
        return Err(invalid_identity(format!(
            "{role:?} target for export {public_name:?} is not present in the canonical closure"
        )));
    }
    Ok(ExportTargetIdentity {
        module: ArtifactIdentity {
            path,
            digest: Digest::parse(digest).map_err(|error| invalid_identity(error.to_string()))?,
        },
        export_name: target.export_name.clone(),
    })
}

fn artifact_matches(
    expected: &ArtifactIdentity,
    actual: &ResolvedFile,
    resolved: &ResolvedImport,
) -> bool {
    package_relative_path(actual, resolved)
        .is_some_and(|path| normalize_contract_path(&expected.path) == path)
        && normalize_digest(&actual.digest).is_ok_and(|digest| expected.digest.as_str() == digest)
}

fn package_relative_path(file: &ResolvedFile, resolved: &ResolvedImport) -> Option<String> {
    let logical_root = Path::new(&resolved.package_root);
    let logical = Path::new(&file.path);
    if let Ok(relative) = logical.strip_prefix(logical_root) {
        return canonical_package_path(relative).ok();
    }
    let real_root = resolved.package_real_root.as_deref().map(Path::new);
    let real_file = file.real_path.as_deref().map(Path::new);
    real_root
        .zip(real_file)
        .and_then(|(root, file)| file.strip_prefix(root).ok())
        .and_then(|relative| canonical_package_path(relative).ok())
}

fn canonical_package_path(path: &Path) -> Result<String, ArtifactResolutionFailure> {
    if path.as_os_str().is_empty() {
        return invalid_resolution("package-relative path must not be empty");
    }
    let mut segments = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => segments.push(segment.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return invalid_resolution("package-relative path contains traversal"),
        }
    }
    if segments.is_empty() {
        return invalid_resolution("package-relative path must name a file");
    }
    Ok(format!("./{}", segments.join("/")))
}

fn normalize_contract_path(path: &str) -> String {
    format!("./{}", path.trim_start_matches("./").replace('\\', "/"))
}

fn validate_resolved_file(file: &ResolvedFile) -> Result<(), ArtifactResolutionFailure> {
    validate_identifier(&file.path, "resolved file path")?;
    if !Path::new(&file.path).is_absolute() {
        return invalid_resolution("resolved file paths must be absolute");
    }
    if let Some(real_path) = &file.real_path
        && !Path::new(real_path).is_absolute()
    {
        return invalid_resolution("resolved real paths must be absolute");
    }
    normalize_digest(&file.digest)?;
    Ok(())
}

fn validate_trace(trace: &ResolutionTrace, axis: &str) -> Result<(), ArtifactResolutionFailure> {
    if !trace.branch.is_empty()
        && !trace.branch.starts_with('/')
        && !trace.branch.starts_with("legacy:")
    {
        return invalid_resolution(format!(
            "{axis} resolution branch must be a JSON pointer or legacy branch"
        ));
    }
    for step in &trace.steps {
        validate_identifier(&step.condition, "resolution condition")?;
        validate_identifier(&step.target, "resolution target")?;
    }
    Ok(())
}

fn validate_import_coordinates(
    specifier: &str,
    package_name: &str,
    requested_entrypoint: &str,
) -> Result<(), ArtifactResolutionFailure> {
    let expected = if specifier == package_name {
        ".".to_owned()
    } else if let Some(suffix) = specifier.strip_prefix(package_name)
        && suffix.starts_with('/')
    {
        format!(".{suffix}")
    } else {
        return invalid_resolution("specifier does not belong to the resolved package name");
    };
    if requested_entrypoint != expected {
        return invalid_resolution(
            "requested entrypoint does not match the exact package specifier",
        );
    }
    if requested_entrypoint != "."
        && requested_entrypoint.strip_prefix("./").is_none_or(|path| {
            path.is_empty()
                || path.contains('\\')
                || path
                    .split('/')
                    .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        })
    {
        return invalid_resolution("requested entrypoint contains traversal or is non-canonical");
    }
    Ok(())
}

fn validate_closure_path(path: &str) -> Result<(), ArtifactResolutionFailure> {
    if let Some(id) = path.strip_prefix("virtual:") {
        return validate_identifier(id, "virtual output identity");
    }
    if !path.starts_with("./") {
        return invalid_resolution("closure paths must be package-relative");
    }
    let relative = path.strip_prefix("./").expect("prefix checked above");
    if path.contains('\\')
        || relative
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return invalid_resolution("closure paths must be canonical and traversal-free");
    }
    canonical_package_path(Path::new(relative)).map(|_| ())
}

fn reject_conflicting_entries(entries: &[ClosureEntry]) -> Result<(), ArtifactResolutionFailure> {
    for pair in entries.windows(2) {
        if pair[0].role == pair[1].role && pair[0].path == pair[1].path && pair[0] != pair[1] {
            return invalid_resolution(format!(
                "closure entry {:?} has contradictory identities",
                pair[0].path
            ));
        }
    }
    Ok(())
}

fn reject_conflicting_dependencies(
    dependencies: &[AcceptedDependencyEdge],
) -> Result<(), ArtifactResolutionFailure> {
    for pair in dependencies.windows(2) {
        if pair[0].specifier == pair[1].specifier
            && pair[0].package_name == pair[1].package_name
            && pair[0] != pair[1]
        {
            return invalid_resolution(format!(
                "dependency edge {:?} has contradictory accepted contract identities",
                pair[0].specifier
            ));
        }
    }
    Ok(())
}

fn closure_digest(
    entries: &[ClosureEntry],
    dependencies: &[AcceptedDependencyEdge],
    hazards: &[ClosureHazard],
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:artifact-closure:v1");
    hash_u64(&mut hash, entries.len());
    for entry in entries {
        hash_text(&mut hash, &format!("{:?}", entry.role));
        hash_text(&mut hash, &entry.path);
        hash_text(&mut hash, &entry.digest);
        hash_optional(&mut hash, entry.transform_digest.as_deref());
    }
    hash_u64(&mut hash, dependencies.len());
    for dependency in dependencies {
        hash_text(&mut hash, &dependency.specifier);
        hash_text(&mut hash, &dependency.package_name);
        hash_text(&mut hash, &dependency.artifact_case);
        hash_text(&mut hash, &dependency.accepted_contract_digest);
    }
    hash_u64(&mut hash, hazards.len());
    for hazard in hazards {
        hash_text(&mut hash, &format!("{:?}", hazard.kind));
        hash_text(&mut hash, &hazard.source);
        hash_u64(&mut hash, hazard.affected_exports.len());
        for export in &hazard.affected_exports {
            hash_text(&mut hash, export);
        }
        hash_u64(&mut hash, hazard.affected_domains.len());
        for domain in &hazard.affected_domains {
            hash_text(&mut hash, &format!("{domain:?}"));
        }
    }
    format!("sha256:{:x}", hash.finalize())
}

fn hash_optional(hash: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hash.update([1]);
            hash_text(hash, value);
        }
        None => hash.update([0]),
    }
}

fn hash_text(hash: &mut Sha256, value: &str) {
    hash_u64(hash, value.len());
    hash.update(value.as_bytes());
}

fn hash_u64(hash: &mut Sha256, value: usize) {
    hash.update(u64::try_from(value).unwrap_or(u64::MAX).to_be_bytes());
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn normalize_digest(value: &str) -> Result<String, ArtifactResolutionFailure> {
    let payload =
        value
            .strip_prefix("sha256:")
            .ok_or_else(|| ArtifactResolutionFailure::Invalid {
                reason: "digest must start with sha256:".into(),
            })?;
    if payload.len() != 64 || !payload.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid_resolution("digest must contain exactly 64 hexadecimal digits");
    }
    Ok(format!("sha256:{}", payload.to_ascii_lowercase()))
}

fn validate_identifier(value: &str, field: &str) -> Result<(), ArtifactResolutionFailure> {
    if value.is_empty() {
        invalid_resolution(format!("{field} must not be empty"))
    } else if value.len() > 16 * 1024 {
        invalid_resolution(format!("{field} exceeds the 16 KiB limit"))
    } else {
        Ok(())
    }
}

fn invalid_resolution<T>(reason: impl Into<String>) -> Result<T, ArtifactResolutionFailure> {
    Err(ArtifactResolutionFailure::Invalid {
        reason: reason.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_reactive_ir::contract_semantics::{KnowledgeState, ResolutionStep};
    use std::time::SystemTime;

    fn temporary_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "solid-checker-phase7-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn repeated_digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn repeated_wire_digest(byte: char) -> String {
        byte.to_string().repeat(64)
    }

    fn resolved_import(closure: ClosureManifest) -> ResolvedImport {
        let root = "/project/node_modules/example";
        let runtime = ResolvedFile {
            path: format!("{root}/dist/index.js"),
            real_path: None,
            digest: repeated_digest('b'),
        };
        let declarations = ResolvedFile {
            path: format!("{root}/types/index.d.ts"),
            real_path: None,
            digest: repeated_digest('d'),
        };
        let binding = ResolvedExportBinding {
            runtime: ResolvedExportTarget {
                module: runtime.clone(),
                export_name: "value".into(),
            },
            declarations: ResolvedExportTarget {
                module: declarations.clone(),
                export_name: "value".into(),
            },
        };
        ResolvedImport {
            specifier: "example".into(),
            importer: "/project/src/app.ts".into(),
            requested_entrypoint: ".".into(),
            package_name: "example".into(),
            package_version: "1.0.0".into(),
            package_integrity: "sha512:example".into(),
            package_root: root.into(),
            package_real_root: None,
            package_manifest: ResolvedFile {
                path: format!("{root}/package.json"),
                real_path: None,
                digest: repeated_digest('a'),
            },
            runtime,
            declarations,
            runtime_trace: ResolutionTrace::default(),
            declaration_trace: ResolutionTrace::default(),
            closure,
            transform: None,
            exports: BTreeMap::from([("value".into(), binding.clone()), ("other".into(), binding)]),
            authority: ResolutionAuthority::Host,
        }
    }

    fn normalized_contract(resolved: &ResolvedImport) -> NormalizedContract {
        let document = serde_json::json!({
            "format": "solid-reactivity-contract",
            "schemaVersion": 1,
            "semanticModelVersion": 1,
            "package": {
                "name": "example",
                "version": "1.0.0",
                "integrity": "sha512:example",
                "manifest": { "path": "package.json", "sha256": repeated_wire_digest('a') }
            },
            "summaries": {
                "closed-call": {
                    "shape": "callable",
                    "call": {
                        "closed": ["reads", "writes"],
                        "reads": [],
                        "writes": []
                    }
                }
            },
            "entrypoints": {
                ".": {
                    "artifact": {
                        "path": "dist/index.js",
                        "sha256": repeated_wire_digest('b'),
                        "closureSha256": resolved.closure.digest.trim_start_matches("sha256:")
                    },
                    "declarations": {
                        "path": "types/index.d.ts",
                        "sha256": repeated_wire_digest('d')
                    },
                    "exports": { "value": "closed-call", "other": "closed-call" }
                }
            },
            "sidecars": {}
        });
        crate::contract_document::decode(&serde_json::to_vec(&document).unwrap())
            .unwrap()
            .normalize()
            .unwrap()
    }

    #[test]
    fn canonical_closure_is_order_independent_and_binds_paths_and_roles() {
        let root = temporary_root("closure");
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::write(root.join("dist/index.js"), b"export const value = 1;\n").unwrap();
        fs::write(
            root.join("dist/index.d.ts"),
            b"export declare const value: 1;\n",
        )
        .unwrap();
        let runtime = ClosureInput::File {
            role: ClosureFileRole::Runtime,
            path: root.join("dist/index.js"),
        };
        let declarations = ClosureInput::File {
            role: ClosureFileRole::Declaration,
            path: root.join("dist/index.d.ts"),
        };
        let first = ClosureManifest::materialize(
            &root,
            [runtime.clone(), declarations.clone()],
            vec![],
            vec![],
        )
        .unwrap();
        let second =
            ClosureManifest::materialize(&root, [declarations, runtime], vec![], vec![]).unwrap();
        assert_eq!(first, second);
        assert_ne!(first.entries[0].role, first.entries[1].role);
        assert_eq!(
            ClosureManifest::new(vec![], vec![], vec![]).unwrap().digest,
            "sha256:19575d19c2fadca45b8704b31f09949362bfb667a45fe12b9708825bb4aad020"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn same_bytes_with_a_different_closure_or_dependency_do_not_collide() {
        let root = temporary_root("identity");
        fs::write(root.join("a.js"), b"same").unwrap();
        fs::write(root.join("b.js"), b"same").unwrap();
        let a = ClosureManifest::materialize(
            &root,
            [ClosureInput::File {
                role: ClosureFileRole::Runtime,
                path: root.join("a.js"),
            }],
            vec![],
            vec![],
        )
        .unwrap();
        let b = ClosureManifest::materialize(
            &root,
            [ClosureInput::File {
                role: ClosureFileRole::Runtime,
                path: root.join("b.js"),
            }],
            vec![],
            vec![],
        )
        .unwrap();
        assert_ne!(a.digest, b.digest);

        let dependency = AcceptedDependencyEdge {
            specifier: "dep".into(),
            package_name: "dep".into(),
            artifact_case: "artifact-case:dep".into(),
            accepted_contract_digest: format!("sha256:{}", "1".repeat(64)),
        };
        let with_dependency =
            ClosureManifest::new(a.entries.clone(), vec![dependency], vec![]).unwrap();
        assert_ne!(a.digest, with_dependency.digest);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generated_output_requires_transform_identity_and_affects_the_digest() {
        let transform = ResolvedFile {
            path: "/package/compiler.json".into(),
            real_path: None,
            digest: format!("sha256:{}", "2".repeat(64)),
        };
        let first = ClosureManifest::materialize(
            Path::new("/"),
            [ClosureInput::Generated {
                stable_id: "server-function:entry".into(),
                bytes: b"first".to_vec(),
                transform: transform.clone(),
            }],
            vec![],
            vec![],
        )
        .unwrap();
        assert_eq!(
            first.digest,
            "sha256:fc5aa068103c10e1e89193af111113541668254cd82440497dbe8cb72e48f961"
        );
        let second = ClosureManifest::materialize(
            Path::new("/"),
            [ClosureInput::Generated {
                stable_id: "server-function:entry".into(),
                bytes: b"second".to_vec(),
                transform,
            }],
            vec![],
            vec![],
        )
        .unwrap();
        assert_ne!(first.digest, second.digest);
        assert!(matches!(
            ClosureManifest::new(
                vec![ClosureEntry {
                    role: ClosureFileRole::Generated,
                    path: "virtual:missing-transform".into(),
                    digest: format!("sha256:{}", "3".repeat(64)),
                    transform_digest: None,
                }],
                vec![],
                vec![]
            ),
            Err(ArtifactResolutionFailure::Invalid { .. })
        ));
    }

    #[test]
    fn exact_selection_refuses_zero_multiple_and_stale_identities() {
        let closure = ClosureManifest::new(vec![], vec![], vec![]).unwrap();
        let resolved = resolved_import(closure);
        let contract = normalized_contract(&resolved);
        assert!(select_and_bind(&contract, &resolved).is_ok());

        let mut stale_runtime = resolved.clone();
        stale_runtime.runtime.digest = repeated_digest('9');
        assert!(matches!(
            select_and_bind(&contract, &stale_runtime),
            Err(ContractFailure::NoArtifactCase)
        ));

        let mut stale_declarations = resolved.clone();
        stale_declarations.declarations.digest = repeated_digest('8');
        assert!(matches!(
            select_and_bind(&contract, &stale_declarations),
            Err(ContractFailure::NoArtifactCase)
        ));

        let mut different_closure = resolved.clone();
        different_closure.closure = ClosureManifest::new(
            vec![],
            vec![],
            vec![ClosureHazard {
                kind: ClosureHazardKind::Eval,
                source: "./dist/index.js:0-4".into(),
                affected_exports: vec![],
                affected_domains: vec![AffectedClaimDomain::Writes],
            }],
        )
        .unwrap();
        assert!(matches!(
            select_and_bind(&contract, &different_closure),
            Err(ContractFailure::NoArtifactCase)
        ));

        let mut stale_manifest = resolved.clone();
        stale_manifest.package_manifest.digest = repeated_digest('7');
        assert!(matches!(
            select_and_bind(&contract, &stale_manifest),
            Err(ContractFailure::IdentityMismatch { .. })
        ));

        let mut conditional = contract.artifact_cases()[0].clone();
        conditional.id = "conditional-case".into();
        conditional.resolution_trace = vec![
            ResolutionStep {
                condition: "runtime".into(),
                target: "/exports/./import".into(),
            },
            ResolutionStep {
                condition: "types".into(),
                target: "/exports/./types".into(),
            },
        ];
        let ambiguous = ContractProposal::new(
            contract.package().clone(),
            vec![contract.artifact_cases()[0].clone(), conditional],
        )
        .normalize()
        .unwrap();
        let mut traced = resolved.clone();
        traced.runtime_trace.branch = "/exports/./import".into();
        traced.declaration_trace.branch = "/exports/./types".into();
        assert!(matches!(
            select_and_bind(&ambiguous, &traced),
            Err(ContractFailure::MultipleArtifactCases)
        ));

        let mut traversing = resolved.clone();
        traversing.specifier = "example/../other".into();
        traversing.requested_entrypoint = "./../other".into();
        assert!(matches!(
            traversing.validate(),
            Err(ArtifactResolutionFailure::Invalid { .. })
        ));

        let mut substituted = resolved;
        substituted.specifier = "other-framework".into();
        assert!(matches!(
            substituted.validate(),
            Err(ArtifactResolutionFailure::Invalid { .. })
        ));
    }

    #[test]
    fn exact_export_binding_and_hazard_weakening_stay_local() {
        let runtime_leaf = ResolvedFile {
            path: "/project/node_modules/example/dist/impl.js".into(),
            real_path: None,
            digest: repeated_digest('e'),
        };
        let declaration_leaf = ResolvedFile {
            path: "/project/node_modules/example/types/impl.d.ts".into(),
            real_path: None,
            digest: repeated_digest('f'),
        };
        let closure = ClosureManifest::new(
            vec![
                ClosureEntry {
                    role: ClosureFileRole::Runtime,
                    path: "./dist/impl.js".into(),
                    digest: runtime_leaf.digest.clone(),
                    transform_digest: None,
                },
                ClosureEntry {
                    role: ClosureFileRole::Declaration,
                    path: "./types/impl.d.ts".into(),
                    digest: declaration_leaf.digest.clone(),
                    transform_digest: None,
                },
            ],
            vec![],
            vec![ClosureHazard {
                kind: ClosureHazardKind::Eval,
                source: "./dist/index.js:0-4".into(),
                affected_exports: vec!["value".into()],
                affected_domains: vec![AffectedClaimDomain::Writes],
            }],
        )
        .unwrap();
        let mut resolved = resolved_import(closure);
        resolved.exports.get_mut("value").unwrap().runtime = ResolvedExportTarget {
            module: runtime_leaf,
            export_name: "internal".into(),
        };
        resolved.exports.get_mut("value").unwrap().declarations = ResolvedExportTarget {
            module: declaration_leaf,
            export_name: "Declared".into(),
        };
        let contract = normalized_contract(&resolved);
        let selected = select_and_bind(&contract, &resolved).unwrap();
        let value = &selected.artifact_cases()[0].exports["value"];
        let other = &selected.artifact_cases()[0].exports["other"];
        assert_eq!(value.identity.runtime.module.path, "./dist/impl.js");
        assert_eq!(value.identity.runtime.export_name, "internal");
        assert_eq!(value.identity.declarations.module.path, "./types/impl.d.ts");
        assert_eq!(value.identity.declarations.export_name, "Declared");
        assert_eq!(
            value.claim_state(ClaimDomain::Writes),
            KnowledgeState::Unknown
        );
        assert_eq!(
            value.claim_state(ClaimDomain::Reads),
            KnowledgeState::CompleteNegative
        );
        assert_eq!(
            other.claim_state(ClaimDomain::Writes),
            KnowledgeState::CompleteNegative
        );

        let mut stale_binding = resolved.clone();
        stale_binding
            .exports
            .get_mut("value")
            .unwrap()
            .runtime
            .module
            .digest = repeated_digest('0');
        assert!(matches!(
            select_and_bind(&contract, &stale_binding),
            Err(ContractFailure::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn resolver_chain_falls_through_only_unattested_authorities() {
        let request = ImportRequest {
            specifier: "example".into(),
            importer: "/project/src/app.ts".into(),
            export_conditions: vec!["import".into()],
        };
        let resolved = resolved_import(ClosureManifest::new(vec![], vec![], vec![]).unwrap());
        let standalone =
            StandaloneResolutionAdapter::from_rows([(request.clone(), resolved.clone())]);
        let empty_host = HostResolutionAdapter::default();
        let empty_typefacts = TypeFactsResolutionAdapter::default();
        let chain = ArtifactResolverChain {
            host: Some(&empty_host),
            typefacts: Some(&empty_typefacts),
            standalone: Some(&standalone),
        };
        assert_eq!(
            chain.resolve(&request).unwrap().authority,
            ResolutionAuthority::StandalonePackageResolver
        );

        let mut wrong = resolved;
        wrong.importer = "/different/importer.ts".into();
        let invalid_host = HostResolutionAdapter::from_rows([(request.clone(), wrong)]);
        let refusing_chain = ArtifactResolverChain {
            host: Some(&invalid_host),
            typefacts: None,
            standalone: Some(&standalone),
        };
        assert!(matches!(
            refusing_chain.resolve(&request),
            Err(ArtifactResolutionFailure::Invalid { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_inside_the_package_are_canonical_and_escaping_links_are_refused() {
        use std::os::unix::fs::symlink;

        let root = temporary_root("symlink");
        fs::write(root.join("target.js"), b"export {};\n").unwrap();
        symlink(root.join("target.js"), root.join("alias.js")).unwrap();
        let manifest = ClosureManifest::materialize(
            &root,
            [ClosureInput::File {
                role: ClosureFileRole::Runtime,
                path: root.join("alias.js"),
            }],
            vec![],
            vec![],
        )
        .unwrap();
        assert_eq!(manifest.entries[0].path, "./alias.js");

        let outside = root.with_extension("outside.js");
        fs::write(&outside, b"outside").unwrap();
        symlink(&outside, root.join("escape.js")).unwrap();
        assert!(matches!(
            ClosureManifest::materialize(
                &root,
                [ClosureInput::File {
                    role: ClosureFileRole::Runtime,
                    path: root.join("escape.js"),
                }],
                vec![],
                vec![]
            ),
            Err(ArtifactResolutionFailure::Invalid { .. })
        ));
        fs::remove_file(outside).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
