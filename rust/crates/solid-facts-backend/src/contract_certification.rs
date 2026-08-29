//! Policy-2 package certification boundary.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::read::GzDecoder;
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, SeqAccess},
};
use serde_json::Value;
use sha2::{Digest as _, Sha256, Sha512};
use solid_reactive_ir::contract_semantics::{
    NormalizedContract,
    certification::{
        CertificationCandidates, DemandPlanningError, ProofDemandGraph, ProofFamily,
        ProofWitnessVariant, WitnessBinding, WitnessCoverage, proof_policy_2,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
    path::{Component, Path},
    sync::Arc,
};
use thiserror::Error;

use crate::artifact_resolution::{
    ImportRequest, ResolutionTrace, ResolutionTraceStep, ResolvedFile, ResolvedImport,
};
use crate::contract_interface::ContractFailure;

#[cfg(feature = "dialect-v2")]
mod compiler_facts;
mod export_bindings;
mod module_closure;
mod type_facts;
mod witness_wire;

#[cfg(feature = "dialect-v2")]
pub use compiler_facts::{
    CompilerCertificationConfiguration, CompilerCertificationError, CompilerCertificationSchedule,
    LiveCompilerEvidenceBatch, VerifiedCompilerEvidence,
};
pub use export_bindings::SnapshotVerifiedExports;
pub use module_closure::SnapshotVerifiedClosure;
pub use type_facts::{
    TypeFactsCertificationError, TypeFactsCertificationSchedule, TypeFactsProducerPin,
    VerifiedTypeFactsEvidence,
};
pub use witness_wire::WitnessWireError;

const SNAPSHOT_HASH_DOMAIN: &[u8] = b"solid-checker:artifact-snapshot:v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotLimits {
    pub registry_metadata_bytes: usize,
    pub archive_bytes: usize,
    pub expanded_archive_bytes: usize,
    pub archive_members: usize,
    pub package_path_bytes: usize,
}

impl SnapshotLimits {
    #[must_use]
    pub fn policy_2() -> Self {
        let limits = proof_policy_2().artifact_snapshot_limits();
        Self {
            registry_metadata_bytes: limits.registry_metadata_bytes,
            archive_bytes: limits.archive_bytes,
            expanded_archive_bytes: limits.expanded_archive_bytes,
            archive_members: limits.archive_members,
            package_path_bytes: limits.package_path_bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedArchive {
    registry_origin: String,
    package_name: String,
    package_version: String,
    registry_metadata: Vec<u8>,
    archive: Vec<u8>,
}

impl PublishedArchive {
    pub fn new(
        registry_origin: impl Into<String>,
        package_name: impl Into<String>,
        package_version: impl Into<String>,
        registry_metadata: Vec<u8>,
        archive: Vec<u8>,
    ) -> Result<Self, ArtifactSnapshotError> {
        let value = Self {
            registry_origin: registry_origin.into(),
            package_name: package_name.into(),
            package_version: package_version.into(),
            registry_metadata,
            archive,
        };
        validate_registry_origin(&value.registry_origin)?;
        validate_coordinate(&value.package_name, "package name")?;
        validate_coordinate(&value.package_version, "package version")?;
        Ok(value)
    }
}

/// A package-manager-selected archive. This is deliberately not convertible
/// to [`PublishedArchive`]: a lock selection proves pinned bytes, not that a
/// registry currently publishes those bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockPinnedArchive {
    package_manager: String,
    lockfile_digest: String,
    locator: String,
    package_name: String,
    package_version: String,
    integrity: String,
    archive: Vec<u8>,
}

impl LockPinnedArchive {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        package_manager: impl Into<String>,
        lockfile_digest: impl Into<String>,
        locator: impl Into<String>,
        package_name: impl Into<String>,
        package_version: impl Into<String>,
        integrity: impl Into<String>,
        archive: Vec<u8>,
    ) -> Result<Self, ArtifactSnapshotError> {
        let value = Self {
            package_manager: package_manager.into(),
            lockfile_digest: lockfile_digest.into(),
            locator: locator.into(),
            package_name: package_name.into(),
            package_version: package_version.into(),
            integrity: integrity.into(),
            archive,
        };
        for (field, name) in [
            (&value.package_manager, "package manager"),
            (&value.locator, "lock locator"),
            (&value.package_name, "package name"),
            (&value.package_version, "package version"),
        ] {
            validate_coordinate(field, name)?;
        }
        validate_sha256(&value.lockfile_digest, "lockfile digest")?;
        validate_integrity_shape(&value.integrity)?;
        Ok(value)
    }
}

/// Workspace/link acquisition has a separate input type so it cannot acquire
/// registry provenance by filling in name/version/integrity fields. Semantic
/// model 1 has no accepted local provenance identity, so policy 2 currently
/// refuses this input explicitly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalArtifact {
    root: String,
}

impl LocalArtifact {
    pub fn new(root: impl Into<String>) -> Result<Self, ArtifactSnapshotError> {
        let root = root.into();
        if !Path::new(&root).is_absolute() {
            return Err(ArtifactSnapshotError::InvalidProvenance(
                "local artifact root must be absolute".into(),
            ));
        }
        Ok(Self { root })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UntrustedArtifactEnvelope {
    Published(PublishedArchive),
    LockPinned(LockPinnedArchive),
    Local(LocalArtifact),
}

#[derive(Clone, Debug)]
pub struct CertificationRequest {
    candidate: NormalizedContract,
    import_request: ImportRequest,
    resolved_import: ResolvedImport,
}

impl CertificationRequest {
    #[must_use]
    pub fn new(
        candidate: NormalizedContract,
        import_request: ImportRequest,
        resolved_import: ResolvedImport,
    ) -> Self {
        Self {
            candidate,
            import_request,
            resolved_import,
        }
    }
}

/// Opaque output of policy-owned planning. Keeping the snapshot and candidate
/// inventory private prevents issuance from swapping in a caller-supplied plan
/// or rereading mutable acquisition paths.
pub struct CertificationPlan {
    snapshot: ArtifactSnapshot,
    verified_resolution: SnapshotVerifiedResolution,
    verified_closure: SnapshotVerifiedClosure,
    verified_exports: SnapshotVerifiedExports,
    candidates: CertificationCandidates,
    demand_graph: ProofDemandGraph,
    artifact_witnesses: Vec<WitnessBinding>,
}

impl CertificationPlan {
    #[must_use]
    pub const fn demand_graph(&self) -> &ProofDemandGraph {
        &self.demand_graph
    }

    #[must_use]
    pub const fn candidates(&self) -> &CertificationCandidates {
        &self.candidates
    }

    #[must_use]
    pub fn snapshot_root(&self) -> &str {
        self.snapshot.root()
    }

    #[must_use]
    pub fn verified_resolution(&self) -> &SnapshotVerifiedResolution {
        &self.verified_resolution
    }

    #[must_use]
    pub const fn verified_closure(&self) -> &SnapshotVerifiedClosure {
        &self.verified_closure
    }

    #[must_use]
    pub const fn verified_exports(&self) -> &SnapshotVerifiedExports {
        &self.verified_exports
    }

    /// Snapshot-derived bindings for the six artifact-wide demands. These are
    /// generated inside the opaque plan and are never accepted from proof
    /// wire. Other family adapters must still satisfy every remaining demand.
    #[must_use]
    pub fn artifact_witness_bindings(&self) -> &[WitnessBinding] {
        &self.artifact_witnesses
    }

    /// Decodes and checks a proof-v2 audit document against this exact plan.
    /// The returned value proves structural coverage only; it cannot replace
    /// direct family-adapter authentication.
    pub fn inspect_witness_document(
        &self,
        bytes: &[u8],
    ) -> Result<WitnessCoverage, WitnessWireError> {
        witness_wire::decode_witness_coverage(bytes, &self.demand_graph)
    }

    /// Acquires Type Facts evidence through the policy-2 live-session adapter.
    ///
    /// `TypeFactsProducerPin` has no public constructor. Policy orchestration
    /// obtains it only from trusted built-in or configured producer policy, so
    /// callers cannot mint authority by hashing an arbitrary executable.
    pub fn acquire_type_facts(
        &self,
        pin: &TypeFactsProducerPin,
        project_id: &str,
        schedule: &TypeFactsCertificationSchedule,
    ) -> Result<typefacts::LiveInvocationAnswer, TypeFactsCertificationError> {
        let mut session = type_facts::TypeFactsCertificationSession::open(pin, project_id)?;
        session.acquire(self, schedule)
    }

    /// Reconciles a direct live producer answer against every scheduled Type
    /// Facts family and the exact immutable snapshot source census.
    pub fn verify_type_facts(
        &self,
        schedule: &TypeFactsCertificationSchedule,
        answer: &typefacts::LiveInvocationAnswer,
    ) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
        type_facts::verify_live_answer(self, schedule, answer)
    }

    /// Launches a fresh private verifier child for every fully materialized
    /// compiler demand and retains authority only in opaque live-session
    /// tokens. Current schema-v1 transform cases are refused by schedule
    /// construction until the output/tool materialization sidecar exists.
    #[cfg(feature = "dialect-v2")]
    pub fn acquire_compiler_facts(
        &self,
        schedule: &CompilerCertificationSchedule,
    ) -> Result<LiveCompilerEvidenceBatch, CompilerCertificationError> {
        compiler_facts::acquire(self, schedule)
    }

    /// Reconciles live compiler sessions with exact materialized source/output
    /// bytes and the complete normalized source-site census.
    #[cfg(feature = "dialect-v2")]
    pub fn verify_compiler_facts(
        &self,
        schedule: &CompilerCertificationSchedule,
        evidence: &LiveCompilerEvidenceBatch,
    ) -> Result<VerifiedCompilerEvidence, CompilerCertificationError> {
        compiler_facts::verify(self, schedule, evidence)
    }
}

/// Hidden child-mode entrypoint used only by the policy-2 compiler adapter.
#[doc(hidden)]
#[cfg(feature = "dialect-v2")]
pub fn serve_compiler_certification_session() -> Result<(), CompilerCertificationError> {
    compiler_facts::serve_compiler_certification_session()
}

/// Exact argv discriminator for the hidden compiler child. Keeping the test in
/// the backend avoids teaching public CLI parsing about this protocol.
#[doc(hidden)]
#[cfg(feature = "dialect-v2")]
pub fn is_compiler_certification_session_argument(argument: &str) -> bool {
    argument == compiler_facts::SESSION_ARGUMENT
}

pub fn plan_certification(
    request: CertificationRequest,
    artifact: UntrustedArtifactEnvelope,
) -> Result<CertificationPlan, CertificationPlanningError> {
    let limits = SnapshotLimits::policy_2();
    let snapshot = match artifact {
        UntrustedArtifactEnvelope::Published(archive) => {
            ArtifactSnapshot::from_published(&archive, limits)?
        }
        UntrustedArtifactEnvelope::LockPinned(archive) => {
            ArtifactSnapshot::from_lock_pinned(&archive, limits)?
        }
        UntrustedArtifactEnvelope::Local(artifact) => {
            ArtifactSnapshot::from_local(&artifact, limits)?
        }
    };
    let verified_resolution =
        snapshot.verify_resolved_import(&request.import_request, &request.resolved_import)?;
    let verified_closure = module_closure::verify_snapshot_closure(
        &snapshot,
        &verified_resolution,
        &request.resolved_import.closure,
    )?;
    let verified_exports = export_bindings::verify_snapshot_exports(
        &snapshot,
        &verified_resolution,
        &request.resolved_import,
    )?;
    let selected =
        crate::artifact_resolution::select_and_bind(&request.candidate, &request.resolved_import)?;
    let policy = proof_policy_2();
    let candidates = policy
        .inspect_candidates(&selected)
        .map_err(|error| CertificationPlanningError::InvalidCandidate(error.to_string()))?;
    let demand_graph =
        policy.derive_demand_graph(&candidates, snapshot.root(), snapshot.provenance_root())?;
    let artifact_witnesses = artifact_witness_bindings(
        &snapshot,
        &verified_resolution,
        &verified_closure,
        &verified_exports,
        &demand_graph,
    );
    Ok(CertificationPlan {
        snapshot,
        verified_resolution,
        verified_closure,
        verified_exports,
        candidates,
        demand_graph,
        artifact_witnesses,
    })
}

#[derive(Debug, Error)]
pub enum CertificationPlanningError {
    #[error(transparent)]
    Artifact(#[from] ArtifactSnapshotError),
    #[error(transparent)]
    Contract(#[from] ContractFailure),
    #[error(transparent)]
    Demand(#[from] DemandPlanningError),
    #[error("invalid certification candidate: {0}")]
    InvalidCandidate(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SnapshotProvenance {
    Published {
        registry_origin: String,
        metadata_digest: String,
        tarball_url: String,
        integrity: String,
    },
    LockPinned {
        package_manager: String,
        lockfile_digest: String,
        locator: String,
        integrity: String,
    },
}

#[derive(Deserialize)]
struct RegistryMetadata {
    versions: RegistryVersions,
}

struct RegistryVersions(BTreeMap<String, RegistryVersion>);

#[derive(Deserialize)]
struct RegistryVersion {
    name: String,
    version: String,
    dist: RegistryDistribution,
}

#[derive(Deserialize)]
struct RegistryDistribution {
    integrity: String,
    tarball: String,
}

struct RegistrySelection {
    integrity: String,
    tarball: String,
}

#[derive(Deserialize)]
struct SnapshotPackageManifest {
    name: String,
    version: String,
    #[serde(default)]
    exports: ExportField,
    #[serde(default)]
    main: Option<String>,
    #[serde(default)]
    types: Option<String>,
    #[serde(default)]
    typings: Option<String>,
}

#[derive(Default)]
enum ExportField {
    #[default]
    Missing,
    Present(ExportTarget),
}

impl<'de> Deserialize<'de> for ExportField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ExportTarget::deserialize(deserializer).map(Self::Present)
    }
}

#[derive(Clone, Debug)]
enum ExportTarget {
    Null,
    String(String),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
    Invalid,
}

impl<'de> Deserialize<'de> for ExportTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TargetVisitor;

        impl<'de> serde::de::Visitor<'de> for TargetVisitor {
            type Value = ExportTarget;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a package exports target")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(ExportTarget::Null)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(ExportTarget::Null)
            }

            fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
                Ok(ExportTarget::Invalid)
            }

            fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
                Ok(ExportTarget::Invalid)
            }

            fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
                Ok(ExportTarget::Invalid)
            }

            fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
                Ok(ExportTarget::Invalid)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(ExportTarget::String(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(ExportTarget::String(value))
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut items = Vec::new();
                while let Some(item) = sequence.next_element()? {
                    items.push(item);
                }
                Ok(ExportTarget::Array(items))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut fields = Vec::<(String, ExportTarget)>::new();
                while let Some((name, target)) = map.next_entry()? {
                    if fields.iter().any(|(existing, _)| existing == &name) {
                        return Err(serde::de::Error::custom(format!(
                            "duplicate package exports key {name:?}"
                        )));
                    }
                    fields.push((name, target));
                }
                Ok(ExportTarget::Object(fields))
            }
        }

        deserializer.deserialize_any(TargetVisitor)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolutionAxis {
    Runtime,
    Declarations,
}

struct SelectedTarget {
    path: String,
    trace: ResolutionTrace,
}

enum TargetSelectionError {
    InvalidTarget(String),
    Refusal(String),
}

impl TargetSelectionError {
    fn into_snapshot_error(self) -> ArtifactSnapshotError {
        let reason = match self {
            Self::InvalidTarget(reason) | Self::Refusal(reason) => reason,
        };
        ArtifactSnapshotError::ResolutionMismatch(reason)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotVerifiedResolution {
    snapshot_root: String,
    provenance_root: String,
    runtime_path: String,
    declarations_path: String,
    evidence_root: String,
}

impl SnapshotVerifiedResolution {
    #[must_use]
    pub fn snapshot_root(&self) -> &str {
        &self.snapshot_root
    }

    #[must_use]
    pub fn provenance_root(&self) -> &str {
        &self.provenance_root
    }

    #[must_use]
    pub fn runtime_path(&self) -> &str {
        &self.runtime_path
    }

    #[must_use]
    pub fn declarations_path(&self) -> &str {
        &self.declarations_path
    }

    #[must_use]
    pub fn evidence_root(&self) -> &str {
        &self.evidence_root
    }
}

fn artifact_witness_bindings(
    snapshot: &ArtifactSnapshot,
    resolution: &SnapshotVerifiedResolution,
    closure: &SnapshotVerifiedClosure,
    exports: &SnapshotVerifiedExports,
    graph: &ProofDemandGraph,
) -> Vec<WitnessBinding> {
    let runtime_digest = snapshot
        .read(resolution.runtime_path())
        .map(|bytes| format!("sha256:{:x}", Sha256::digest(bytes)))
        .expect("verified runtime path belongs to the snapshot");
    let declaration_digest = snapshot
        .read(resolution.declarations_path())
        .map(|bytes| format!("sha256:{:x}", Sha256::digest(bytes)))
        .expect("verified declaration path belongs to the snapshot");
    graph
        .demands()
        .iter()
        .filter_map(|demand| {
            let (variant, root, sites) = match demand.family() {
                ProofFamily::PackageIdentity => (
                    ProofWitnessVariant::PackageIdentity,
                    certification_evidence_root(
                        "package-identity",
                        [
                            snapshot.package_name(),
                            snapshot.package_version(),
                            snapshot.package_integrity(),
                            snapshot.root(),
                            snapshot.provenance_root(),
                        ],
                    ),
                    vec![
                        format!("package:{}", snapshot.package_name()),
                        format!("version:{}", snapshot.package_version()),
                        format!("integrity:{}", snapshot.package_integrity()),
                        format!("snapshot:{}", snapshot.root()),
                        format!("provenance:{}", snapshot.provenance_root()),
                    ],
                ),
                ProofFamily::ManifestEntrypoint => (
                    ProofWitnessVariant::ManifestEntrypoint,
                    certification_evidence_root(
                        "manifest-entrypoint",
                        [resolution.evidence_root()],
                    ),
                    vec![
                        format!("runtime-entrypoint:{}", resolution.runtime_path()),
                        format!("declaration-entrypoint:{}", resolution.declarations_path()),
                    ],
                ),
                ProofFamily::ExportResolution => (
                    ProofWitnessVariant::ExportResolution,
                    certification_evidence_root(
                        "export-resolution",
                        [
                            resolution.runtime_path(),
                            resolution.declarations_path(),
                            resolution.evidence_root(),
                        ],
                    ),
                    vec![
                        format!("runtime-resolution:{}", resolution.runtime_path()),
                        format!("declaration-resolution:{}", resolution.declarations_path()),
                    ],
                ),
                ProofFamily::ArtifactDeclarations => (
                    ProofWitnessVariant::ArtifactDeclarations,
                    certification_evidence_root(
                        "artifact-declarations",
                        [
                            resolution.runtime_path(),
                            runtime_digest.as_str(),
                            resolution.declarations_path(),
                            declaration_digest.as_str(),
                        ],
                    ),
                    vec![
                        format!(
                            "runtime-artifact:{}:{runtime_digest}",
                            resolution.runtime_path()
                        ),
                        format!(
                            "declaration-artifact:{}:{declaration_digest}",
                            resolution.declarations_path()
                        ),
                    ],
                ),
                ProofFamily::ExportIdentity => (
                    ProofWitnessVariant::ExportIdentity,
                    certification_evidence_root("export-identity", [exports.evidence_root()]),
                    {
                        let sites = exports.site_ids();
                        if sites.is_empty() {
                            vec!["export-set:empty".into()]
                        } else {
                            sites
                        }
                    },
                ),
                ProofFamily::ModuleClosure => (
                    ProofWitnessVariant::ModuleClosure,
                    certification_evidence_root(
                        "module-closure",
                        [closure.manifest().digest.as_str()],
                    ),
                    {
                        let mut sites = closure
                            .manifest()
                            .entries
                            .iter()
                            .map(|entry| {
                                format!("file:{:?}:{}:{}", entry.role, entry.path, entry.digest)
                            })
                            .chain(
                                closure
                                    .manifest()
                                    .dependencies
                                    .iter()
                                    .map(|dependency| dependency.specifier.clone()),
                            )
                            .chain(
                                closure
                                    .manifest()
                                    .hazards
                                    .iter()
                                    .map(|hazard| hazard.source.clone()),
                            )
                            .collect::<Vec<_>>();
                        if sites.is_empty() {
                            sites.push("module-closure:empty".into());
                        }
                        sites
                    },
                ),
                _ => return None,
            };
            Some(WitnessBinding::new(
                variant,
                demand.id().as_str(),
                root,
                sites,
            ))
        })
        .collect()
}

fn resolution_evidence_root(
    entrypoint: &str,
    conditions: &BTreeSet<&str>,
    runtime: &ResolutionTrace,
    declarations: &ResolutionTrace,
) -> String {
    let mut fields = vec![entrypoint.to_owned()];
    fields.extend(
        conditions
            .iter()
            .map(|condition| format!("condition:{condition}")),
    );
    for (axis, trace) in [("runtime", runtime), ("declarations", declarations)] {
        fields.push(format!("{axis}:branch:{}", trace.branch));
        fields.extend(
            trace
                .steps
                .iter()
                .map(|step| format!("{axis}:{}:{}", step.condition, step.target)),
        );
    }
    certification_evidence_root("resolution", fields.iter().map(String::as_str))
}

pub(super) fn certification_evidence_root<'a>(
    family: &str,
    fields: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:contract-proof-family-evidence:v2");
    hash.update(
        u64::try_from(family.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hash.update(family.as_bytes());
    for field in fields {
        hash.update(u64::try_from(field.len()).unwrap_or(u64::MAX).to_be_bytes());
        hash.update(field.as_bytes());
    }
    format!("sha256:{:x}", hash.finalize())
}

impl<'de> Deserialize<'de> for RegistryVersions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VersionsVisitor;

        impl<'de> serde::de::Visitor<'de> for VersionsVisitor {
            type Value = RegistryVersions;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a registry versions object without duplicate versions")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut versions = BTreeMap::new();
                while let Some((version, record)) = map.next_entry::<String, RegistryVersion>()? {
                    if versions.insert(version.clone(), record).is_some() {
                        return Err(serde::de::Error::custom(format!(
                            "duplicate registry version {version:?}"
                        )));
                    }
                }
                Ok(RegistryVersions(versions))
            }
        }

        deserializer.deserialize_map(VersionsVisitor)
    }
}

/// Read-only, content-addressed view of one fully validated package archive.
/// It owns every byte used by later certification stages; no later read is
/// allowed to fall back to the acquisition archive or host filesystem.
#[derive(Clone, Debug)]
pub struct ArtifactSnapshot {
    package_name: String,
    package_version: String,
    package_integrity: String,
    files: BTreeMap<String, Arc<[u8]>>,
    directories: BTreeSet<String>,
    root: String,
    provenance_root: String,
}

impl ArtifactSnapshot {
    pub fn from_published(
        archive: &PublishedArchive,
        limits: SnapshotLimits,
    ) -> Result<Self, ArtifactSnapshotError> {
        let selection = select_registry_metadata(archive, limits)?;
        verify_sri(&archive.archive, &selection.integrity)?;
        Self::from_archive(
            &archive.archive,
            archive.package_name.clone(),
            archive.package_version.clone(),
            SnapshotProvenance::Published {
                registry_origin: archive.registry_origin.clone(),
                metadata_digest: format!("sha256:{:x}", Sha256::digest(&archive.registry_metadata)),
                tarball_url: selection.tarball,
                integrity: selection.integrity.clone(),
            },
            selection.integrity,
            limits,
        )
    }

    pub fn from_lock_pinned(
        archive: &LockPinnedArchive,
        limits: SnapshotLimits,
    ) -> Result<Self, ArtifactSnapshotError> {
        verify_sri(&archive.archive, &archive.integrity)?;
        Self::from_archive(
            &archive.archive,
            archive.package_name.clone(),
            archive.package_version.clone(),
            SnapshotProvenance::LockPinned {
                package_manager: archive.package_manager.clone(),
                lockfile_digest: archive.lockfile_digest.clone(),
                locator: archive.locator.clone(),
                integrity: archive.integrity.clone(),
            },
            archive.integrity.clone(),
            limits,
        )
    }

    pub fn from_local(
        artifact: &LocalArtifact,
        _limits: SnapshotLimits,
    ) -> Result<Self, ArtifactSnapshotError> {
        Err(ArtifactSnapshotError::UnsupportedProvenance(format!(
            "policy 2 semantic model 1 cannot certify local artifact {}",
            artifact.root
        )))
    }

    fn from_archive(
        archive: &[u8],
        package_name: String,
        package_version: String,
        provenance: SnapshotProvenance,
        package_integrity: String,
        limits: SnapshotLimits,
    ) -> Result<Self, ArtifactSnapshotError> {
        if archive.len() > limits.archive_bytes {
            return Err(ArtifactSnapshotError::ResourceLimit(
                "archive bytes exceed policy limit".into(),
            ));
        }

        let decoder = GzDecoder::new(Cursor::new(archive));
        let mut tar = tar::Archive::new(decoder);
        let mut files = BTreeMap::<String, Arc<[u8]>>::new();
        let mut seen = BTreeSet::new();
        let mut casefolded = BTreeMap::<String, String>::new();
        let mut expanded_bytes = 0usize;
        let entries = tar.entries().map_err(archive_error)?;
        for (index, entry) in entries.enumerate() {
            if index >= limits.archive_members {
                return Err(ArtifactSnapshotError::ResourceLimit(
                    "archive members exceed policy limit".into(),
                ));
            }
            let mut entry = entry.map_err(archive_error)?;
            let package_path = canonical_member_path(&entry, limits.package_path_bytes)?;
            if !seen.insert(package_path.clone()) {
                return Err(ArtifactSnapshotError::DuplicateMember(package_path));
            }
            let folded = package_path.to_lowercase();
            if let Some(first) = casefolded.insert(folded, package_path.clone())
                && first != package_path
            {
                return Err(ArtifactSnapshotError::CaseCollision {
                    first,
                    second: package_path,
                });
            }

            let kind = entry.header().entry_type();
            if kind.is_dir() {
                continue;
            }
            if !kind.is_file() {
                return Err(ArtifactSnapshotError::UnsupportedMember {
                    path: package_path,
                    kind: format!("{kind:?}"),
                });
            }
            let declared = usize::try_from(entry.size()).map_err(|_| {
                ArtifactSnapshotError::ResourceLimit("archive member size exceeds usize".into())
            })?;
            let remaining = limits
                .expanded_archive_bytes
                .checked_sub(expanded_bytes)
                .ok_or_else(|| {
                    ArtifactSnapshotError::ResourceLimit(
                        "expanded archive bytes exceed policy limit".into(),
                    )
                })?;
            if declared > remaining {
                return Err(ArtifactSnapshotError::ResourceLimit(
                    "expanded archive bytes exceed policy limit".into(),
                ));
            }
            let mut bytes = Vec::with_capacity(declared);
            entry.read_to_end(&mut bytes).map_err(archive_error)?;
            if bytes.len() != declared {
                return Err(ArtifactSnapshotError::InvalidArchive(format!(
                    "member {package_path} length disagrees with its header"
                )));
            }
            expanded_bytes += bytes.len();
            files.insert(package_path, Arc::from(bytes));
        }
        if files.is_empty() {
            return Err(ArtifactSnapshotError::InvalidArchive(
                "archive contains no package files".into(),
            ));
        }
        validate_topology(&files)?;
        let directories = derive_directories(files.keys());
        validate_manifest_identity(&files, &package_name, &package_version)?;
        let root = snapshot_root(&package_name, &package_version, &files, &directories);
        let provenance_root = provenance_root(&provenance, &root);
        Ok(Self {
            package_name,
            package_version,
            package_integrity,
            files,
            directories,
            root,
            provenance_root,
        })
    }

    #[must_use]
    pub fn read(&self, package_relative_path: &str) -> Option<&[u8]> {
        self.files.get(package_relative_path).map(AsRef::as_ref)
    }

    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }

    #[must_use]
    pub fn provenance_root(&self) -> &str {
        &self.provenance_root
    }

    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    #[must_use]
    pub fn package_version(&self) -> &str {
        &self.package_version
    }

    #[must_use]
    pub fn package_integrity(&self) -> &str {
        &self.package_integrity
    }

    /// Re-resolves the exact import from snapshot-owned manifest and file
    /// bytes. The supplied record is comparison material only; none of its
    /// paths, digests, or condition traces become authority by being nonempty.
    pub fn verify_resolved_import(
        &self,
        request: &ImportRequest,
        resolved: &ResolvedImport,
    ) -> Result<SnapshotVerifiedResolution, ArtifactSnapshotError> {
        resolved.validate().map_err(|error| {
            ArtifactSnapshotError::ResolutionMismatch(format!(
                "supplied resolution is structurally invalid: {error}"
            ))
        })?;
        if resolved.transform.is_some() {
            return resolution_mismatch(
                "policy 2 cannot certify a transform without separately bound output and tool bytes",
            );
        }
        if resolved.specifier != request.specifier || resolved.importer != request.importer {
            return resolution_mismatch("supplied resolution does not answer the exact request");
        }
        let expected_entrypoint = requested_entrypoint(&request.specifier, &self.package_name)?;
        for (field, expected, actual) in [
            (
                "entrypoint",
                expected_entrypoint.as_str(),
                resolved.requested_entrypoint.as_str(),
            ),
            (
                "package name",
                self.package_name.as_str(),
                resolved.package_name.as_str(),
            ),
            (
                "package version",
                self.package_version.as_str(),
                resolved.package_version.as_str(),
            ),
            (
                "package integrity",
                self.package_integrity.as_str(),
                resolved.package_integrity.as_str(),
            ),
        ] {
            if expected != actual {
                return resolution_mismatch(format!(
                    "{field} is {actual:?}; snapshot requires {expected:?}"
                ));
            }
        }

        verify_resolved_file(self, resolved, &resolved.package_manifest, "package.json")?;
        let manifest: SnapshotPackageManifest = serde_json::from_slice(
            self.read("package.json")
                .expect("snapshot creation requires package.json"),
        )
        .map_err(|error| {
            ArtifactSnapshotError::ResolutionMismatch(format!(
                "snapshot package manifest cannot drive resolution: {error}"
            ))
        })?;
        if manifest.name != self.package_name || manifest.version != self.package_version {
            return resolution_mismatch(
                "snapshot package manifest changed after identity validation",
            );
        }
        let conditions = request
            .export_conditions
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let runtime = resolve_snapshot_export(
            self,
            &manifest,
            &expected_entrypoint,
            &conditions,
            ResolutionAxis::Runtime,
        )?;
        let declarations = resolve_snapshot_export(
            self,
            &manifest,
            &expected_entrypoint,
            &conditions,
            ResolutionAxis::Declarations,
        )?;
        verify_resolved_file(self, resolved, &resolved.runtime, &runtime.path)?;
        verify_resolved_file(self, resolved, &resolved.declarations, &declarations.path)?;
        if resolved.runtime_trace != runtime.trace {
            return resolution_mismatch("runtime resolution trace disagrees with snapshot replay");
        }
        if resolved.declaration_trace != declarations.trace {
            return resolution_mismatch(
                "declaration resolution trace disagrees with snapshot replay",
            );
        }

        let evidence_root = resolution_evidence_root(
            &expected_entrypoint,
            &conditions,
            &runtime.trace,
            &declarations.trace,
        );
        Ok(SnapshotVerifiedResolution {
            snapshot_root: self.root.clone(),
            provenance_root: self.provenance_root.clone(),
            runtime_path: runtime.path,
            declarations_path: declarations.path,
            evidence_root,
        })
    }

    #[must_use]
    pub fn member_count(&self) -> usize {
        self.files.len() + self.directories.len()
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ArtifactSnapshotError {
    #[error("invalid artifact provenance: {0}")]
    InvalidProvenance(String),
    #[error("unsupported artifact provenance: {0}")]
    UnsupportedProvenance(String),
    #[error("artifact integrity mismatch")]
    IntegrityMismatch,
    #[error("invalid package archive: {0}")]
    InvalidArchive(String),
    #[error("archive member path is unsafe: {0}")]
    UnsafePath(String),
    #[error("duplicate archive member: {0}")]
    DuplicateMember(String),
    #[error("case-folding archive collision between {first} and {second}")]
    CaseCollision { first: String, second: String },
    #[error("unsupported archive member {path}: {kind}")]
    UnsupportedMember { path: String, kind: String },
    #[error("artifact snapshot resource limit: {0}")]
    ResourceLimit(String),
    #[error("package manifest identity mismatch: {0}")]
    ManifestIdentity(String),
    #[error("artifact resolution mismatch: {0}")]
    ResolutionMismatch(String),
    #[error("artifact module closure mismatch: {0}")]
    ModuleClosure(String),
    #[error("artifact export binding mismatch: {0}")]
    ExportBindings(String),
}

fn requested_entrypoint(
    specifier: &str,
    package_name: &str,
) -> Result<String, ArtifactSnapshotError> {
    if specifier == package_name {
        return Ok(".".into());
    }
    let suffix = specifier.strip_prefix(package_name).ok_or_else(|| {
        ArtifactSnapshotError::ResolutionMismatch(
            "specifier does not belong to the snapshot package".into(),
        )
    })?;
    if !suffix.starts_with('/') {
        return resolution_mismatch("specifier does not name a package subpath");
    }
    let entrypoint = format!(".{suffix}");
    if entrypoint.contains('\\')
        || entrypoint
            .trim_start_matches("./")
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return resolution_mismatch("specifier contains a noncanonical package subpath");
    }
    Ok(entrypoint)
}

fn verify_resolved_file(
    snapshot: &ArtifactSnapshot,
    resolved: &ResolvedImport,
    file: &ResolvedFile,
    expected_path: &str,
) -> Result<(), ArtifactSnapshotError> {
    let actual_path = resolved_package_path(resolved, file)?;
    if actual_path != expected_path {
        return resolution_mismatch(format!(
            "resolved file path is {actual_path:?}; snapshot selected {expected_path:?}"
        ));
    }
    let bytes = snapshot.read(expected_path).ok_or_else(|| {
        ArtifactSnapshotError::ResolutionMismatch(format!(
            "snapshot has no regular file {expected_path:?}"
        ))
    })?;
    let digest = format!("sha256:{:x}", Sha256::digest(bytes));
    if file.digest != digest {
        return resolution_mismatch(format!("resolved digest for {expected_path:?} is stale"));
    }
    Ok(())
}

fn resolved_package_path(
    resolved: &ResolvedImport,
    file: &ResolvedFile,
) -> Result<String, ArtifactSnapshotError> {
    let logical = Path::new(&file.path)
        .strip_prefix(Path::new(&resolved.package_root))
        .map_err(|_| {
            ArtifactSnapshotError::ResolutionMismatch(
                "resolved file is outside the logical package root".into(),
            )
        })?;
    let logical = canonical_relative_path(logical)?;
    match (&resolved.package_real_root, &file.real_path) {
        (None, None) => {}
        (Some(root), Some(path)) => {
            let real = Path::new(path).strip_prefix(Path::new(root)).map_err(|_| {
                ArtifactSnapshotError::ResolutionMismatch(
                    "resolved real file is outside the real package root".into(),
                )
            })?;
            if canonical_relative_path(real)? != logical {
                return resolution_mismatch(
                    "resolved file changes package-relative identity through a symlink",
                );
            }
        }
        _ => {
            return resolution_mismatch(
                "resolved package root and file real-path identities are incomplete",
            );
        }
    }
    Ok(logical)
}

fn canonical_relative_path(path: &Path) -> Result<String, ArtifactSnapshotError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str().ok_or_else(|| {
                ArtifactSnapshotError::ResolutionMismatch(
                    "resolved package path is not UTF-8".into(),
                )
            })?),
            _ => return resolution_mismatch("resolved package path contains traversal"),
        }
    }
    if parts.is_empty() {
        return resolution_mismatch("resolved package path does not name a file");
    }
    Ok(parts.join("/"))
}

fn resolve_snapshot_export(
    snapshot: &ArtifactSnapshot,
    manifest: &SnapshotPackageManifest,
    entrypoint: &str,
    conditions: &BTreeSet<&str>,
    axis: ResolutionAxis,
) -> Result<SelectedTarget, ArtifactSnapshotError> {
    match &manifest.exports {
        ExportField::Present(exports) => {
            let (target, pointer, capture) = select_subpath(exports, entrypoint)?;
            let mut active = conditions.clone();
            active.insert("default");
            if axis == ResolutionAxis::Declarations {
                active.insert("types");
            } else {
                active.remove("types");
            }
            let mut selected = select_target(
                target,
                snapshot,
                entrypoint,
                capture.as_deref(),
                &active,
                &pointer,
                vec![ResolutionTraceStep {
                    condition: "subpath".into(),
                    target: entrypoint.into(),
                }],
            )
            .map_err(TargetSelectionError::into_snapshot_error)?;
            if axis == ResolutionAxis::Declarations {
                selected.path =
                    declaration_candidate(snapshot, &selected.path).ok_or_else(|| {
                        ArtifactSnapshotError::ResolutionMismatch(format!(
                            "no declaration target exists for {:?}",
                            selected.path
                        ))
                    })?;
            } else if snapshot.read(&selected.path).is_none() {
                return resolution_mismatch(format!(
                    "runtime target {:?} is not a snapshot file",
                    selected.path
                ));
            }
            Ok(selected)
        }
        ExportField::Missing => resolve_legacy(snapshot, manifest, entrypoint, axis),
    }
}

fn select_subpath<'a>(
    exports: &'a ExportTarget,
    entrypoint: &str,
) -> Result<(&'a ExportTarget, String, Option<String>), ArtifactSnapshotError> {
    let fields = match exports {
        ExportTarget::Object(fields) => Some(fields.as_slice()),
        _ => None,
    };
    if let Some(fields) = fields {
        let has_subpath = fields.iter().any(|(key, _)| key.starts_with('.'));
        let has_condition = fields.iter().any(|(key, _)| !key.starts_with('.'));
        if has_subpath && has_condition {
            return resolution_mismatch("package exports mixes subpath and condition keys");
        }
        if has_subpath {
            if let Some((_, target)) = fields.iter().find(|(key, _)| key == entrypoint) {
                return Ok((
                    target,
                    format!("/exports/{}", pointer_segment(entrypoint)),
                    None,
                ));
            }
            let mut matches = fields
                .iter()
                .filter_map(|(key, target)| {
                    pattern_capture(key, entrypoint).map(|capture| (key, target, capture))
                })
                .collect::<Vec<_>>();
            matches.sort_by(|(left, _, _), (right, _, _)| pattern_key_compare(left, right));
            let Some((key, target, capture)) = matches.into_iter().next() else {
                return resolution_mismatch(format!("entrypoint {entrypoint:?} is not exported"));
            };
            return Ok((
                target,
                format!("/exports/{}", pointer_segment(key)),
                Some(capture),
            ));
        }
    }
    if entrypoint != "." {
        return resolution_mismatch(format!("entrypoint {entrypoint:?} is not exported"));
    }
    Ok((exports, "/exports/.".into(), None))
}

#[allow(clippy::too_many_arguments)]
fn select_target(
    target: &ExportTarget,
    snapshot: &ArtifactSnapshot,
    entrypoint: &str,
    capture: Option<&str>,
    conditions: &BTreeSet<&str>,
    pointer: &str,
    steps: Vec<ResolutionTraceStep>,
) -> Result<SelectedTarget, TargetSelectionError> {
    match target {
        ExportTarget::Null => Err(TargetSelectionError::Refusal(format!(
            "entrypoint {entrypoint:?} is blocked by a null target"
        ))),
        ExportTarget::String(target) => {
            let selected = match capture {
                Some(capture) => target.replace('*', capture),
                None => target.clone(),
            };
            let path =
                validate_target_string(&selected).map_err(TargetSelectionError::InvalidTarget)?;
            if snapshot.read(&path).is_none() {
                return Err(TargetSelectionError::Refusal(format!(
                    "package target {selected:?} is not a snapshot file"
                )));
            }
            let mut steps = steps;
            steps.push(ResolutionTraceStep {
                condition: "target".into(),
                target: selected,
            });
            Ok(SelectedTarget {
                path,
                trace: ResolutionTrace {
                    branch: pointer.into(),
                    steps,
                },
            })
        }
        ExportTarget::Array(items) => {
            let mut last = None;
            for (index, item) in items.iter().enumerate() {
                let mut next_steps = steps.clone();
                next_steps.push(ResolutionTraceStep {
                    condition: "array".into(),
                    target: index.to_string(),
                });
                match select_target(
                    item,
                    snapshot,
                    entrypoint,
                    capture,
                    conditions,
                    &format!("{pointer}/{index}"),
                    next_steps,
                ) {
                    Ok(selected) => return Ok(selected),
                    Err(error @ TargetSelectionError::InvalidTarget(_)) => last = Some(error),
                    Err(error @ TargetSelectionError::Refusal(_)) => return Err(error),
                }
            }
            Err(last.unwrap_or_else(|| {
                TargetSelectionError::InvalidTarget("package target array is empty".into())
            }))
        }
        ExportTarget::Object(fields) => {
            if fields.iter().any(|(key, _)| key.starts_with('.')) {
                return Err(TargetSelectionError::InvalidTarget(
                    "subpath keys cannot be nested inside a conditional target".into(),
                ));
            }
            for (condition, nested) in fields {
                if condition != "default" && !conditions.contains(condition.as_str()) {
                    continue;
                }
                let mut next_steps = steps.clone();
                next_steps.push(ResolutionTraceStep {
                    condition: condition.clone(),
                    target: pointer.into(),
                });
                return select_target(
                    nested,
                    snapshot,
                    entrypoint,
                    capture,
                    conditions,
                    &format!("{pointer}/{}", pointer_segment(condition)),
                    next_steps,
                );
            }
            Err(TargetSelectionError::Refusal(format!(
                "entrypoint {entrypoint:?} selects no active condition"
            )))
        }
        ExportTarget::Invalid => Err(TargetSelectionError::InvalidTarget(
            "package target is not a string, object, array, or null".into(),
        )),
    }
}

fn resolve_legacy(
    snapshot: &ArtifactSnapshot,
    manifest: &SnapshotPackageManifest,
    entrypoint: &str,
    axis: ResolutionAxis,
) -> Result<SelectedTarget, ArtifactSnapshotError> {
    if entrypoint != "." {
        return resolution_mismatch("legacy manifest has no package subpath entrypoint");
    }
    let (field, target) = if axis == ResolutionAxis::Declarations {
        if let Some(target) = &manifest.types {
            ("types", target.as_str())
        } else if let Some(target) = &manifest.typings {
            ("typings", target.as_str())
        } else if let Some(target) = &manifest.main {
            ("main", target.as_str())
        } else {
            ("index", "index.js")
        }
    } else if let Some(target) = &manifest.main {
        ("main", target.as_str())
    } else {
        ("index", "index.js")
    };
    let path = validate_legacy_target(target)?;
    let path = if axis == ResolutionAxis::Declarations {
        declaration_candidate(snapshot, &path).ok_or_else(|| {
            ArtifactSnapshotError::ResolutionMismatch(format!(
                "no declaration target exists for {path:?}"
            ))
        })?
    } else {
        if snapshot.read(&path).is_none() {
            return resolution_mismatch(format!("legacy target {path:?} does not exist"));
        }
        path
    };
    Ok(SelectedTarget {
        path,
        trace: ResolutionTrace {
            branch: format!("legacy:{field}"),
            steps: vec![ResolutionTraceStep {
                condition: field.into(),
                target: target.into(),
            }],
        },
    })
}

fn validate_target_string(target: &str) -> Result<String, String> {
    let relative = target
        .strip_prefix("./")
        .ok_or_else(|| "package target must start with ./".to_owned())?;
    validate_target_segments(relative, target).map_err(|error| error.to_string())?;
    Ok(relative.into())
}

fn validate_legacy_target(target: &str) -> Result<String, ArtifactSnapshotError> {
    let relative = target.trim_start_matches("./");
    validate_target_segments(relative, target)?;
    Ok(relative.into())
}

fn validate_target_segments(relative: &str, rendered: &str) -> Result<(), ArtifactSnapshotError> {
    if relative.split('/').any(|part| {
        let lowercase = part.to_ascii_lowercase();
        part.is_empty()
            || matches!(part, "." | ".." | "node_modules")
            || lowercase.contains("%2e")
            || lowercase.contains("%2f")
            || lowercase.contains("%5c")
            || part.contains('\\')
    }) {
        return resolution_mismatch(format!(
            "package target {rendered:?} contains an invalid segment"
        ));
    }
    Ok(())
}

fn declaration_candidate(snapshot: &ArtifactSnapshot, path: &str) -> Option<String> {
    const DECLARATIONS: [&str; 3] = [".d.ts", ".d.mts", ".d.cts"];
    const RUNTIME: [&str; 8] = [".js", ".mjs", ".cjs", ".jsx", ".ts", ".mts", ".cts", ".tsx"];
    if DECLARATIONS
        .iter()
        .any(|extension| path.ends_with(extension))
    {
        return snapshot.read(path).is_some().then(|| path.into());
    }
    let slash = path.rfind('/');
    let dot = path
        .rfind('.')
        .filter(|dot| slash.is_none_or(|slash| dot > &slash));
    let stem = dot.map_or(path, |dot| &path[..dot]);
    for candidate in DECLARATIONS
        .iter()
        .map(|extension| format!("{stem}{extension}"))
        .chain(
            DECLARATIONS
                .iter()
                .map(|extension| format!("{path}/index{extension}")),
        )
    {
        if snapshot.read(&candidate).is_some() {
            return Some(candidate);
        }
    }
    (RUNTIME.iter().any(|extension| path.ends_with(extension)) && snapshot.read(path).is_some())
        .then(|| path.into())
}

fn pattern_capture(pattern: &str, candidate: &str) -> Option<String> {
    let star = pattern.find('*')?;
    let (prefix, suffix_with_star) = pattern.split_at(star);
    let suffix = &suffix_with_star[1..];
    candidate
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
        .map(str::to_owned)
}

fn pattern_key_compare(left: &str, right: &str) -> std::cmp::Ordering {
    let left_star = left.find('*');
    let right_star = right.find('*');
    let left_base = left_star.map_or(left.len(), |index| index + 1);
    let right_base = right_star.map_or(right.len(), |index| index + 1);
    right_base
        .cmp(&left_base)
        .then_with(|| left_star.is_none().cmp(&right_star.is_none()).reverse())
        .then_with(|| right.len().cmp(&left.len()))
}

fn pointer_segment(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn resolution_mismatch<T>(reason: impl Into<String>) -> Result<T, ArtifactSnapshotError> {
    Err(ArtifactSnapshotError::ResolutionMismatch(reason.into()))
}

fn validate_registry_origin(origin: &str) -> Result<(), ArtifactSnapshotError> {
    let authority = origin.strip_prefix("https://").ok_or_else(|| {
        ArtifactSnapshotError::InvalidProvenance(
            "registry origin must use canonical https://".into(),
        )
    })?;
    if authority.is_empty()
        || !authority.is_ascii()
        || authority.contains(['/', '?', '#', '@'])
        || authority != authority.to_ascii_lowercase()
    {
        return Err(ArtifactSnapshotError::InvalidProvenance(
            "registry origin must contain only a canonical lowercase authority".into(),
        ));
    }
    Ok(())
}

fn select_registry_metadata(
    archive: &PublishedArchive,
    limits: SnapshotLimits,
) -> Result<RegistrySelection, ArtifactSnapshotError> {
    crate::bounded_json::value(
        &archive.registry_metadata,
        crate::bounded_json::Limits {
            bytes: limits.registry_metadata_bytes,
            depth: 128,
            nodes: 1_000_000,
            string_bytes: 16 * 1024,
        },
    )
    .map_err(|error| {
        ArtifactSnapshotError::InvalidProvenance(format!(
            "registry metadata is invalid or exceeds policy limits: {error}"
        ))
    })?;
    // Deserialize the original bytes after the structural pass. Going through
    // serde_json::Value here would silently collapse duplicate version or
    // distribution fields before the typed visitor can reject them.
    let metadata: RegistryMetadata =
        serde_json::from_slice(&archive.registry_metadata).map_err(|error| {
            ArtifactSnapshotError::InvalidProvenance(format!(
                "registry metadata selection is invalid: {error}"
            ))
        })?;
    let selected = metadata
        .versions
        .0
        .get(&archive.package_version)
        .ok_or_else(|| {
            ArtifactSnapshotError::InvalidProvenance(format!(
                "registry metadata does not contain selected version {:?}",
                archive.package_version
            ))
        })?;
    if selected.name != archive.package_name || selected.version != archive.package_version {
        return Err(ArtifactSnapshotError::InvalidProvenance(
            "selected registry record identity disagrees with its version key".into(),
        ));
    }
    validate_integrity_shape(&selected.dist.integrity)?;
    let tarball_prefix = format!("{}/", archive.registry_origin);
    if !selected.dist.tarball.starts_with(&tarball_prefix)
        || selected.dist.tarball.contains(['?', '#'])
    {
        return Err(ArtifactSnapshotError::InvalidProvenance(
            "selected tarball URL is outside the canonical registry origin".into(),
        ));
    }
    Ok(RegistrySelection {
        integrity: selected.dist.integrity.clone(),
        tarball: selected.dist.tarball.clone(),
    })
}

fn validate_coordinate(value: &str, field: &str) -> Result<(), ArtifactSnapshotError> {
    if value.is_empty() || value.len() > 16 * 1024 || value.chars().any(char::is_control) {
        return Err(ArtifactSnapshotError::InvalidProvenance(format!(
            "{field} is empty, oversized, or contains controls"
        )));
    }
    Ok(())
}

fn validate_sha256(value: &str, field: &str) -> Result<(), ArtifactSnapshotError> {
    let hex = value.strip_prefix("sha256:").ok_or_else(|| {
        ArtifactSnapshotError::InvalidProvenance(format!("{field} must use sha256:"))
    })?;
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ArtifactSnapshotError::InvalidProvenance(format!(
            "{field} is not a canonical SHA-256 digest"
        )));
    }
    Ok(())
}

fn validate_integrity_shape(integrity: &str) -> Result<(), ArtifactSnapshotError> {
    let encoded = integrity.strip_prefix("sha512-").ok_or_else(|| {
        ArtifactSnapshotError::InvalidProvenance("archive integrity must use sha512 SRI".into())
    })?;
    let decoded = STANDARD.decode(encoded).map_err(|_| {
        ArtifactSnapshotError::InvalidProvenance("archive integrity is not canonical base64".into())
    })?;
    if decoded.len() != 64 || STANDARD.encode(&decoded) != encoded {
        return Err(ArtifactSnapshotError::InvalidProvenance(
            "archive integrity is not a canonical SHA-512 SRI".into(),
        ));
    }
    Ok(())
}

fn verify_sri(bytes: &[u8], integrity: &str) -> Result<(), ArtifactSnapshotError> {
    validate_integrity_shape(integrity)?;
    let computed = format!("sha512-{}", STANDARD.encode(Sha512::digest(bytes)));
    if computed != integrity {
        return Err(ArtifactSnapshotError::IntegrityMismatch);
    }
    Ok(())
}

fn canonical_member_path<R: Read>(
    entry: &tar::Entry<'_, R>,
    path_limit: usize,
) -> Result<String, ArtifactSnapshotError> {
    let path = entry.path().map_err(archive_error)?;
    let text = path.to_str().ok_or_else(|| {
        ArtifactSnapshotError::UnsafePath("member path is not valid UTF-8".into())
    })?;
    if text.len() > path_limit || text.contains('\\') || text.contains('\0') {
        return Err(ArtifactSnapshotError::UnsafePath(text.into()));
    }
    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(root)) if root == "package" => {}
        _ => return Err(ArtifactSnapshotError::UnsafePath(text.into())),
    }
    let mut normalized = Vec::new();
    for component in components {
        match component {
            Component::Normal(value) => normalized.push(value.to_str().ok_or_else(|| {
                ArtifactSnapshotError::UnsafePath("member path is not valid UTF-8".into())
            })?),
            _ => return Err(ArtifactSnapshotError::UnsafePath(text.into())),
        }
    }
    if normalized.is_empty() {
        return Ok(String::new());
    }
    Ok(normalized.join("/"))
}

fn validate_topology(files: &BTreeMap<String, Arc<[u8]>>) -> Result<(), ArtifactSnapshotError> {
    for path in files.keys() {
        if path.is_empty() {
            return Err(ArtifactSnapshotError::InvalidArchive(
                "package root cannot be a regular file".into(),
            ));
        }
        let mut prefix = String::new();
        let components = path.split('/').collect::<Vec<_>>();
        for component in &components[..components.len().saturating_sub(1)] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            if files.contains_key(&prefix) {
                return Err(ArtifactSnapshotError::InvalidArchive(format!(
                    "{prefix} is both a file and a directory"
                )));
            }
        }
    }
    Ok(())
}

fn derive_directories<'a>(paths: impl Iterator<Item = &'a String>) -> BTreeSet<String> {
    let mut directories = BTreeSet::new();
    for path in paths {
        let mut prefix = String::new();
        let mut components = path.split('/').peekable();
        while let Some(component) = components.next() {
            if components.peek().is_none() {
                break;
            }
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            directories.insert(prefix.clone());
        }
    }
    directories
}

fn validate_manifest_identity(
    files: &BTreeMap<String, Arc<[u8]>>,
    expected_name: &str,
    expected_version: &str,
) -> Result<(), ArtifactSnapshotError> {
    let bytes = files
        .get("package.json")
        .ok_or_else(|| ArtifactSnapshotError::ManifestIdentity("package.json is missing".into()))?;
    let manifest: Value = serde_json::from_slice(bytes).map_err(|error| {
        ArtifactSnapshotError::ManifestIdentity(format!("package.json is invalid: {error}"))
    })?;
    let actual_name = manifest.get("name").and_then(Value::as_str);
    let actual_version = manifest.get("version").and_then(Value::as_str);
    if actual_name != Some(expected_name) || actual_version != Some(expected_version) {
        return Err(ArtifactSnapshotError::ManifestIdentity(format!(
            "expected {expected_name}@{expected_version}, found {}@{}",
            actual_name.unwrap_or("<missing>"),
            actual_version.unwrap_or("<missing>")
        )));
    }
    Ok(())
}

fn snapshot_root(
    package_name: &str,
    package_version: &str,
    files: &BTreeMap<String, Arc<[u8]>>,
    directories: &BTreeSet<String>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SNAPSHOT_HASH_DOMAIN);
    hash_field(&mut hasher, package_name.as_bytes());
    hash_field(&mut hasher, package_version.as_bytes());
    for directory in directories {
        hash_field(&mut hasher, b"directory");
        hash_field(&mut hasher, directory.as_bytes());
    }
    for (path, bytes) in files {
        hash_field(&mut hasher, b"file");
        hash_field(&mut hasher, path.as_bytes());
        hash_field(&mut hasher, bytes);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn provenance_root(provenance: &SnapshotProvenance, snapshot_root: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"solid-checker:artifact-provenance:v1\0");
    match provenance {
        SnapshotProvenance::Published {
            registry_origin,
            metadata_digest,
            tarball_url,
            integrity,
        } => {
            hash_field(&mut hasher, b"published");
            hash_field(&mut hasher, registry_origin.as_bytes());
            hash_field(&mut hasher, metadata_digest.as_bytes());
            hash_field(&mut hasher, tarball_url.as_bytes());
            hash_field(&mut hasher, integrity.as_bytes());
        }
        SnapshotProvenance::LockPinned {
            package_manager,
            lockfile_digest,
            locator,
            integrity,
        } => {
            hash_field(&mut hasher, b"lock-pinned");
            hash_field(&mut hasher, package_manager.as_bytes());
            hash_field(&mut hasher, lockfile_digest.as_bytes());
            hash_field(&mut hasher, locator.as_bytes());
            hash_field(&mut hasher, integrity.as_bytes());
        }
    }
    hash_field(&mut hasher, snapshot_root.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn archive_error(error: impl std::fmt::Display) -> ArtifactSnapshotError {
    ArtifactSnapshotError::InvalidArchive(error.to_string())
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "dialect-v2")]
    use super::CompilerCertificationSchedule;
    use super::{
        ArtifactSnapshot, ArtifactSnapshotError, CertificationRequest, LocalArtifact,
        LockPinnedArchive, PublishedArchive, SnapshotLimits, SnapshotVerifiedResolution,
        UntrustedArtifactEnvelope, plan_certification,
    };
    use crate::artifact_resolution::{
        ClosureEntry, ClosureFileRole, ClosureHazardKind, ClosureManifest, ImportRequest,
        ResolutionAuthority, ResolutionTrace, ResolutionTraceStep, ResolvedExportBinding,
        ResolvedExportTarget, ResolvedFile, ResolvedImport,
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use flate2::{Compression, write::GzEncoder};
    use sha2::{Digest as _, Sha256, Sha512};
    use solid_reactive_ir::contract_semantics::ContractProposal;
    use std::collections::BTreeMap;

    use std::io::Write as _;

    fn archive_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut builder = tar::Builder::new(&mut encoder);
            for (path, bytes) in files {
                let mut header = tar::Header::new_gnu();
                header.set_size(bytes.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append_data(&mut header, *path, *bytes)
                    .expect("test archive member");
            }
            builder.finish().expect("test archive");
        }
        encoder.finish().expect("test gzip")
    }

    fn published_from_bytes(archive: Vec<u8>) -> PublishedArchive {
        let integrity = format!("sha512-{}", STANDARD.encode(Sha512::digest(&archive)));
        let metadata = registry_metadata(
            &integrity,
            "fixture-package",
            "1.2.3",
            "https://registry.npmjs.org/fixture-package/-/fixture-package-1.2.3.tgz",
        );
        PublishedArchive::new(
            "https://registry.npmjs.org",
            "fixture-package",
            "1.2.3",
            metadata,
            archive,
        )
        .expect("published coordinates")
    }

    fn registry_metadata(integrity: &str, name: &str, version: &str, tarball: &str) -> Vec<u8> {
        format!(
            r#"{{"versions":{{"1.2.3":{{"name":"{name}","version":"{version}","dist":{{"integrity":"{integrity}","tarball":"{tarball}"}}}}}}}}"#
        )
        .into_bytes()
    }

    fn published_archive(files: &[(&str, &[u8])]) -> PublishedArchive {
        published_from_bytes(archive_bytes(files))
    }

    fn raw_archive(members: &[(&str, u8, &str, &[u8])]) -> Vec<u8> {
        let mut tar = Vec::new();
        for (path, kind, link, bytes) in members {
            let mut header = [0_u8; 512];
            header[..path.len()].copy_from_slice(path.as_bytes());
            write_octal(&mut header[100..108], 0o644);
            write_octal(&mut header[108..116], 0);
            write_octal(&mut header[116..124], 0);
            write_octal(&mut header[124..136], bytes.len() as u64);
            write_octal(&mut header[136..148], 0);
            header[148..156].fill(b' ');
            header[156] = *kind;
            header[157..157 + link.len()].copy_from_slice(link.as_bytes());
            header[257..263].copy_from_slice(b"ustar\0");
            header[263..265].copy_from_slice(b"00");
            let checksum = header.iter().map(|byte| u64::from(*byte)).sum();
            write_checksum(&mut header[148..156], checksum);
            tar.extend_from_slice(&header);
            tar.extend_from_slice(bytes);
            let padding = (512 - bytes.len() % 512) % 512;
            tar.resize(tar.len() + padding, 0);
        }
        tar.resize(tar.len() + 1024, 0);
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar).expect("raw tar gzip");
        encoder.finish().expect("raw test gzip")
    }

    fn write_octal(slot: &mut [u8], value: u64) {
        slot.fill(b'0');
        let encoded = format!("{value:o}");
        let start = slot.len() - 1 - encoded.len();
        slot[start..start + encoded.len()].copy_from_slice(encoded.as_bytes());
        slot[slot.len() - 1] = 0;
    }

    fn write_checksum(slot: &mut [u8], value: u64) {
        let encoded = format!("{value:06o}\0 ");
        slot.copy_from_slice(encoded.as_bytes());
    }

    fn fixture_files() -> [(&'static str, &'static [u8]); 2] {
        [
            (
                "package/package.json",
                br#"{"name":"fixture-package","version":"1.2.3"}"#,
            ),
            ("package/dist/index.js", b"export const answer = 42;"),
        ]
    }

    fn resolved_file(root: &str, path: &str, bytes: &[u8]) -> ResolvedFile {
        ResolvedFile {
            path: format!("{root}/{path}"),
            real_path: None,
            digest: format!("sha256:{:x}", Sha256::digest(bytes)),
        }
    }

    fn closure_entry(role: ClosureFileRole, path: &str, bytes: &[u8]) -> ClosureEntry {
        ClosureEntry {
            role,
            path: format!("./{path}"),
            digest: format!("sha256:{:x}", Sha256::digest(bytes)),
            transform_digest: None,
        }
    }

    #[test]
    fn published_archive_becomes_an_immutable_package_snapshot() {
        let archive = published_archive(&fixture_files());

        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();

        assert_eq!(
            snapshot.read("dist/index.js"),
            Some(&b"export const answer = 42;"[..])
        );
        assert_eq!(snapshot.package_name(), "fixture-package");
        assert_eq!(snapshot.package_version(), "1.2.3");
        assert!(snapshot.root().starts_with("sha256:"));
    }

    #[test]
    fn snapshot_root_ignores_archive_order_but_provenance_does_not() {
        let files = fixture_files();
        let forward = published_archive(&files);
        let reverse = published_archive(&[files[1], files[0]]);
        let forward_snapshot =
            ArtifactSnapshot::from_published(&forward, SnapshotLimits::policy_2()).unwrap();
        let reverse_snapshot =
            ArtifactSnapshot::from_published(&reverse, SnapshotLimits::policy_2()).unwrap();

        assert_ne!(forward.registry_metadata, reverse.registry_metadata);
        assert_eq!(forward_snapshot.root(), reverse_snapshot.root());
        assert_ne!(
            forward_snapshot.provenance_root(),
            reverse_snapshot.provenance_root()
        );
    }

    #[test]
    fn snapshot_survives_mutation_of_the_acquisition_archive() {
        let mut archive = published_archive(&fixture_files());
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();

        archive.archive.fill(0);

        assert_eq!(
            snapshot.read("dist/index.js"),
            Some(&b"export const answer = 42;"[..])
        );
    }

    #[test]
    fn sri_and_manifest_identity_are_recomputed() {
        let mut corrupt = published_archive(&fixture_files());
        corrupt.archive[0] ^= 1;
        assert_eq!(
            ArtifactSnapshot::from_published(&corrupt, SnapshotLimits::policy_2()).unwrap_err(),
            ArtifactSnapshotError::IntegrityMismatch
        );

        let mixed = published_archive(&[(
            "package/package.json",
            br#"{"name":"other-package","version":"1.2.3"}"#,
        )]);
        assert!(matches!(
            ArtifactSnapshot::from_published(&mixed, SnapshotLimits::policy_2()),
            Err(ArtifactSnapshotError::ManifestIdentity(_))
        ));
    }

    #[test]
    fn registry_metadata_selects_integrity_and_origin_without_caller_shortcuts() {
        let bytes = archive_bytes(&fixture_files());
        let integrity = format!("sha512-{}", STANDARD.encode(Sha512::digest(&bytes)));
        for metadata in [
            registry_metadata(
                &integrity,
                "other-package",
                "1.2.3",
                "https://registry.npmjs.org/fixture-package/-/fixture-package-1.2.3.tgz",
            ),
            registry_metadata(
                &integrity,
                "fixture-package",
                "1.2.3",
                "https://evil.invalid/fixture-package-1.2.3.tgz",
            ),
            format!(
                r#"{{"versions":{{"1.2.3":{{"name":"fixture-package","version":"1.2.3","dist":{{"integrity":"{integrity}","tarball":"https://registry.npmjs.org/a.tgz"}}}},"1.2.3":{{"name":"fixture-package","version":"1.2.3","dist":{{"integrity":"{integrity}","tarball":"https://registry.npmjs.org/b.tgz"}}}}}}}}"#
            )
            .into_bytes(),
        ] {
            let input = PublishedArchive::new(
                "https://registry.npmjs.org",
                "fixture-package",
                "1.2.3",
                metadata,
                bytes.clone(),
            )
            .unwrap();
            assert!(matches!(
                ArtifactSnapshot::from_published(&input, SnapshotLimits::policy_2()),
                Err(ArtifactSnapshotError::InvalidProvenance(_))
            ));
        }

        let mut oversized = published_from_bytes(bytes);
        oversized.registry_metadata =
            vec![b' '; SnapshotLimits::policy_2().registry_metadata_bytes + 1];
        assert!(matches!(
            ArtifactSnapshot::from_published(&oversized, SnapshotLimits::policy_2()),
            Err(ArtifactSnapshotError::InvalidProvenance(_))
        ));
    }

    #[test]
    fn archive_topology_attacks_are_rejected() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3"}"#;
        let duplicate = published_from_bytes(raw_archive(&[
            ("package/package.json", b'0', "", manifest),
            ("package/package.json", b'0', "", manifest),
        ]));
        assert!(matches!(
            ArtifactSnapshot::from_published(&duplicate, SnapshotLimits::policy_2()),
            Err(ArtifactSnapshotError::DuplicateMember(_))
        ));

        let collision = published_from_bytes(raw_archive(&[
            ("package/package.json", b'0', "", manifest),
            ("package/Dist/index.js", b'0', "", b"a"),
            ("package/dist/index.js", b'0', "", b"b"),
        ]));
        assert!(matches!(
            ArtifactSnapshot::from_published(&collision, SnapshotLimits::policy_2()),
            Err(ArtifactSnapshotError::CaseCollision { .. })
        ));

        for hostile in [
            raw_archive(&[
                ("package/package.json", b'0', "", manifest),
                ("package/../escape.js", b'0', "", b"bad"),
            ]),
            raw_archive(&[
                ("package/package.json", b'0', "", manifest),
                ("package/link", b'2', "../../escape", b""),
            ]),
            raw_archive(&[
                ("package/package.json", b'0', "", manifest),
                ("package/link", b'1', "../../escape", b""),
            ]),
        ] {
            assert!(
                ArtifactSnapshot::from_published(
                    &published_from_bytes(hostile),
                    SnapshotLimits::policy_2()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn snapshot_resource_limits_fail_closed() {
        let archive = published_archive(&fixture_files());
        let mut limits = SnapshotLimits::policy_2();
        limits.archive_bytes = archive.archive.len() - 1;
        assert!(matches!(
            ArtifactSnapshot::from_published(&archive, limits),
            Err(ArtifactSnapshotError::ResourceLimit(_))
        ));

        let mut limits = SnapshotLimits::policy_2();
        limits.expanded_archive_bytes = 8;
        assert!(matches!(
            ArtifactSnapshot::from_published(&archive, limits),
            Err(ArtifactSnapshotError::ResourceLimit(_))
        ));

        let mut limits = SnapshotLimits::policy_2();
        limits.archive_members = 1;
        assert!(matches!(
            ArtifactSnapshot::from_published(&archive, limits),
            Err(ArtifactSnapshotError::ResourceLimit(_))
        ));

        let mut limits = SnapshotLimits::policy_2();
        limits.package_path_bytes = 12;
        assert!(matches!(
            ArtifactSnapshot::from_published(&archive, limits),
            Err(ArtifactSnapshotError::UnsafePath(_))
        ));
    }

    #[test]
    fn provenance_kinds_cannot_impersonate_each_other() {
        assert!(
            PublishedArchive::new(
                "https://REGISTRY.npmjs.org",
                "fixture-package",
                "1.2.3",
                b"{}".to_vec(),
                Vec::new(),
            )
            .is_err()
        );

        let published = published_archive(&fixture_files());
        let published_snapshot =
            ArtifactSnapshot::from_published(&published, SnapshotLimits::policy_2()).unwrap();
        let lock = LockPinnedArchive::new(
            "bun",
            format!("sha256:{:064x}", 1),
            "fixture-package@1.2.3",
            "fixture-package",
            "1.2.3",
            published_snapshot.package_integrity().to_owned(),
            published.archive.clone(),
        )
        .unwrap();
        let lock_snapshot =
            ArtifactSnapshot::from_lock_pinned(&lock, SnapshotLimits::policy_2()).unwrap();
        assert_eq!(published_snapshot.root(), lock_snapshot.root());
        assert_ne!(
            published_snapshot.provenance_root(),
            lock_snapshot.provenance_root()
        );
        assert!(published_snapshot.member_count() >= 2);
        let local = LocalArtifact::new("/workspace/fixture-package").unwrap();
        assert!(matches!(
            ArtifactSnapshot::from_local(&local, SnapshotLimits::policy_2()),
            Err(ArtifactSnapshotError::UnsupportedProvenance(_))
        ));
    }

    #[test]
    fn resolved_import_is_replayed_from_snapshot_manifest_and_bytes() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3","exports":{".":{"types":"./types/index.d.ts","development":"./dist/dev.js","import":"./dist/index.js","default":"./dist/default.js"}}}"#;
        let runtime = b"export const mode = 'development';";
        let declarations = b"export declare const mode: 'development';";
        let archive = published_archive(&[
            ("package/package.json", manifest),
            ("package/dist/dev.js", runtime),
            ("package/dist/index.js", b"export const mode = 'import';"),
            ("package/dist/default.js", b"export const mode = 'default';"),
            ("package/types/index.d.ts", declarations),
        ]);
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let root = "/project/node_modules/fixture-package";
        let package_manifest = resolved_file(root, "package.json", manifest);
        let runtime_file = resolved_file(root, "dist/dev.js", runtime);
        let declaration_file = resolved_file(root, "types/index.d.ts", declarations);
        let closure = ClosureManifest::new(
            vec![
                ClosureEntry {
                    role: ClosureFileRole::Manifest,
                    path: "./package.json".into(),
                    digest: package_manifest.digest.clone(),
                    transform_digest: None,
                },
                ClosureEntry {
                    role: ClosureFileRole::ResolutionInput,
                    path: "./package.json".into(),
                    digest: package_manifest.digest.clone(),
                    transform_digest: None,
                },
                ClosureEntry {
                    role: ClosureFileRole::Runtime,
                    path: "./dist/dev.js".into(),
                    digest: runtime_file.digest.clone(),
                    transform_digest: None,
                },
                ClosureEntry {
                    role: ClosureFileRole::Declaration,
                    path: "./types/index.d.ts".into(),
                    digest: declaration_file.digest.clone(),
                    transform_digest: None,
                },
            ],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let request = ImportRequest {
            specifier: "fixture-package".into(),
            importer: "/project/src/app.ts".into(),
            export_conditions: vec!["import".into(), "development".into()],
        };
        let exports = BTreeMap::from([(
            "mode".into(),
            ResolvedExportBinding {
                runtime: ResolvedExportTarget {
                    module: runtime_file.clone(),
                    export_name: "mode".into(),
                },
                declarations: ResolvedExportTarget {
                    module: declaration_file.clone(),
                    export_name: "mode".into(),
                },
            },
        )]);
        let resolved = ResolvedImport {
            specifier: request.specifier.clone(),
            importer: request.importer.clone(),
            requested_entrypoint: ".".into(),
            package_name: "fixture-package".into(),
            package_version: "1.2.3".into(),
            package_integrity: snapshot.package_integrity().into(),
            package_root: root.into(),
            package_real_root: None,
            package_manifest,
            runtime: runtime_file,
            declarations: declaration_file,
            runtime_trace: ResolutionTrace {
                branch: "/exports/./development".into(),
                steps: vec![
                    ResolutionTraceStep {
                        condition: "subpath".into(),
                        target: ".".into(),
                    },
                    ResolutionTraceStep {
                        condition: "development".into(),
                        target: "/exports/.".into(),
                    },
                    ResolutionTraceStep {
                        condition: "target".into(),
                        target: "./dist/dev.js".into(),
                    },
                ],
            },
            declaration_trace: ResolutionTrace {
                branch: "/exports/./types".into(),
                steps: vec![
                    ResolutionTraceStep {
                        condition: "subpath".into(),
                        target: ".".into(),
                    },
                    ResolutionTraceStep {
                        condition: "types".into(),
                        target: "/exports/.".into(),
                    },
                    ResolutionTraceStep {
                        condition: "target".into(),
                        target: "./types/index.d.ts".into(),
                    },
                ],
            },
            closure,
            transform: None,
            exports,
            authority: ResolutionAuthority::Host,
        };

        let verified = snapshot
            .verify_resolved_import(&request, &resolved)
            .unwrap();
        assert_eq!(verified.snapshot_root(), snapshot.root());
        assert_eq!(verified.runtime_path(), "dist/dev.js");
        assert_eq!(verified.declarations_path(), "types/index.d.ts");

        let mut stale = resolved.clone();
        stale.runtime.digest = format!("sha256:{:064x}", 0);
        assert!(matches!(
            snapshot.verify_resolved_import(&request, &stale),
            Err(ArtifactSnapshotError::ResolutionMismatch(_))
        ));

        let mut copied_trace = resolved.clone();
        copied_trace.runtime_trace = copied_trace.declaration_trace.clone();
        assert!(matches!(
            snapshot.verify_resolved_import(&request, &copied_trace),
            Err(ArtifactSnapshotError::ResolutionMismatch(_))
        ));

        let mut unmaterialized_transform = resolved.clone();
        unmaterialized_transform.transform = Some(resolved_file(
            "/toolchain",
            "loader.js",
            b"untrusted loader bytes",
        ));
        assert!(matches!(
            snapshot.verify_resolved_import(&request, &unmaterialized_transform),
            Err(ArtifactSnapshotError::ResolutionMismatch(_))
        ));

        let (package, artifact_case) =
            crate::artifact_resolution::proposal_identity(&resolved).unwrap();
        let candidate = ContractProposal::new(package, vec![artifact_case])
            .normalize()
            .unwrap();
        let plan = plan_certification(
            CertificationRequest::new(candidate, request.clone(), resolved.clone()),
            UntrustedArtifactEnvelope::Published(archive.clone()),
        )
        .unwrap();
        assert_eq!(plan.snapshot_root(), snapshot.root());
        assert_eq!(plan.demand_graph().demands().len(), 6);
        assert_eq!(plan.verified_closure().manifest(), &resolved.closure);
        assert_eq!(plan.verified_exports().binding_count(), 1);
        assert_eq!(plan.artifact_witness_bindings().len(), 6);
        #[cfg(feature = "dialect-v2")]
        assert_eq!(
            CompilerCertificationSchedule::new(&plan, [])
                .unwrap()
                .demand_count(),
            0,
            "an artifact with no compiler-owned site must not fabricate an empty witness"
        );
        assert!(
            plan.demand_graph()
                .verify_witness_coverage(plan.artifact_witness_bindings().iter().cloned())
                .is_ok()
        );

        let request_without_development = ImportRequest {
            export_conditions: vec!["import".into()],
            ..request
        };
        assert!(matches!(
            snapshot.verify_resolved_import(&request_without_development, &resolved),
            Err(ArtifactSnapshotError::ResolutionMismatch(_))
        ));
    }

    #[test]
    fn module_closure_is_recomputed_with_exact_roles_edges_and_hazards() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3"}"#;
        let runtime = br#"
            import "./shared.js";
            import "./asset.json";
            import "external-package";
            import(dynamicName);
            import("./chunk.js");
            eval(source);
            WebAssembly.instantiate(bytes);
            leaked = 1;
        "#;
        let shared = b"export const shared = true;";
        let chunk = b"export const chunk = true;";
        let asset = br#"{"value":true}"#;
        let declarations = b"export * from './surface.js';";
        let surface = b"export declare const shared: boolean;";
        let archive = published_archive(&[
            ("package/package.json", manifest),
            ("package/dist/index.js", runtime),
            ("package/dist/shared.js", shared),
            ("package/dist/shared-copy.js", shared),
            ("package/dist/chunk.js", chunk),
            ("package/dist/asset.json", asset),
            ("package/types/index.d.ts", declarations),
            ("package/types/surface.d.ts", surface),
        ]);
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let resolution = SnapshotVerifiedResolution {
            snapshot_root: snapshot.root().into(),
            provenance_root: snapshot.provenance_root().into(),
            runtime_path: "dist/index.js".into(),
            declarations_path: "types/index.d.ts".into(),
            evidence_root: format!("sha256:{:064x}", 0),
        };

        let replayed =
            super::module_closure::replay_snapshot_closure(&snapshot, &resolution, &[]).unwrap();
        for expected in [
            closure_entry(ClosureFileRole::Manifest, "package.json", manifest),
            closure_entry(ClosureFileRole::ResolutionInput, "package.json", manifest),
            closure_entry(ClosureFileRole::Runtime, "dist/index.js", runtime),
            closure_entry(ClosureFileRole::Runtime, "dist/shared.js", shared),
            closure_entry(ClosureFileRole::LiteralDynamicChunk, "dist/chunk.js", chunk),
            closure_entry(ClosureFileRole::ResolutionInput, "dist/asset.json", asset),
            closure_entry(
                ClosureFileRole::Declaration,
                "types/index.d.ts",
                declarations,
            ),
            closure_entry(ClosureFileRole::Declaration, "types/surface.d.ts", surface),
        ] {
            assert!(replayed.entries.contains(&expected), "missing {expected:?}");
        }
        assert!(
            !replayed
                .entries
                .iter()
                .any(|entry| entry.path == "./dist/shared-copy.js")
        );
        assert_eq!(
            replayed
                .hazards
                .iter()
                .map(|hazard| hazard.kind)
                .collect::<std::collections::BTreeSet<_>>(),
            [
                ClosureHazardKind::NonliteralDynamicLoading,
                ClosureHazardKind::Eval,
                ClosureHazardKind::OpaqueWasm,
                ClosureHazardKind::MutableUnboundGlobal,
                ClosureHazardKind::UnacceptedExternalDependency,
            ]
            .into_iter()
            .collect()
        );
        let verified =
            super::module_closure::verify_snapshot_closure(&snapshot, &resolution, &replayed)
                .unwrap();
        assert_eq!(verified.manifest(), &replayed);

        let mut missing_edge_entries = replayed.entries.clone();
        missing_edge_entries.retain(|entry| entry.path != "./dist/shared.js");
        let missing_edge = ClosureManifest::new(
            missing_edge_entries,
            replayed.dependencies.clone(),
            replayed.hazards.clone(),
        )
        .unwrap();
        assert!(matches!(
            super::module_closure::verify_snapshot_closure(&snapshot, &resolution, &missing_edge),
            Err(ArtifactSnapshotError::ModuleClosure(_))
        ));

        let mut stale_entries = replayed.entries.clone();
        stale_entries
            .iter_mut()
            .find(|entry| entry.path == "./dist/shared.js")
            .unwrap()
            .digest = format!("sha256:{:064x}", 0);
        let stale = ClosureManifest::new(
            stale_entries,
            replayed.dependencies.clone(),
            replayed.hazards.clone(),
        )
        .unwrap();
        assert!(matches!(
            super::module_closure::verify_snapshot_closure(&snapshot, &resolution, &stale),
            Err(ArtifactSnapshotError::ModuleClosure(_))
        ));

        let mut aliased_entries = replayed.entries.clone();
        aliased_entries
            .iter_mut()
            .find(|entry| entry.path == "./dist/shared.js")
            .unwrap()
            .path = "./dist/shared-copy.js".into();
        let same_bytes_different_path =
            ClosureManifest::new(aliased_entries, replayed.dependencies, replayed.hazards).unwrap();
        assert!(matches!(
            super::module_closure::verify_snapshot_closure(
                &snapshot,
                &resolution,
                &same_bytes_different_path
            ),
            Err(ArtifactSnapshotError::ModuleClosure(_))
        ));
    }

    #[test]
    fn export_bindings_are_replayed_across_reexports_stars_and_namespaces() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3"}"#;
        let runtime = br#"
            export { runtimeName as publicName } from "./runtime.js";
            export * from "./star.js";
            export * as ns from "./namespace.js";
        "#;
        let runtime_target = b"export const runtimeName = 1;";
        let runtime_star = b"export const shared = 1;";
        let runtime_namespace = b"export const inner = 1;";
        let declarations = br#"
            export { declarationName as publicName } from "./surface.js";
            export * from "./star.js";
            export * as ns from "./namespace.js";
        "#;
        let declaration_target = b"export declare const declarationName: number;";
        let declaration_star = b"export declare const shared: number;";
        let declaration_namespace = b"export declare const inner: number;";
        let archive = published_archive(&[
            ("package/package.json", manifest),
            ("package/dist/index.js", runtime),
            ("package/dist/runtime.js", runtime_target),
            ("package/dist/runtime-copy.js", runtime_target),
            ("package/dist/star.js", runtime_star),
            ("package/dist/namespace.js", runtime_namespace),
            ("package/types/index.d.ts", declarations),
            ("package/types/surface.d.ts", declaration_target),
            ("package/types/star.d.ts", declaration_star),
            ("package/types/namespace.d.ts", declaration_namespace),
        ]);
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let root = "/project/node_modules/fixture-package";
        let resolution = SnapshotVerifiedResolution {
            snapshot_root: snapshot.root().into(),
            provenance_root: snapshot.provenance_root().into(),
            runtime_path: "dist/index.js".into(),
            declarations_path: "types/index.d.ts".into(),
            evidence_root: format!("sha256:{:064x}", 0),
        };
        let closure =
            super::module_closure::replay_snapshot_closure(&snapshot, &resolution, &[]).unwrap();
        let binding = |runtime_path: &str,
                       runtime_bytes: &[u8],
                       runtime_name: &str,
                       declaration_path: &str,
                       declaration_bytes: &[u8],
                       declaration_name: &str| ResolvedExportBinding {
            runtime: ResolvedExportTarget {
                module: resolved_file(root, runtime_path, runtime_bytes),
                export_name: runtime_name.into(),
            },
            declarations: ResolvedExportTarget {
                module: resolved_file(root, declaration_path, declaration_bytes),
                export_name: declaration_name.into(),
            },
        };
        let exports = BTreeMap::from([
            (
                "ns".into(),
                binding(
                    "dist/namespace.js",
                    runtime_namespace,
                    "*",
                    "types/namespace.d.ts",
                    declaration_namespace,
                    "*",
                ),
            ),
            (
                "publicName".into(),
                binding(
                    "dist/runtime.js",
                    runtime_target,
                    "runtimeName",
                    "types/surface.d.ts",
                    declaration_target,
                    "declarationName",
                ),
            ),
            (
                "shared".into(),
                binding(
                    "dist/star.js",
                    runtime_star,
                    "shared",
                    "types/star.d.ts",
                    declaration_star,
                    "shared",
                ),
            ),
        ]);
        let resolved = ResolvedImport {
            specifier: "fixture-package".into(),
            importer: "/project/src/app.ts".into(),
            requested_entrypoint: ".".into(),
            package_name: "fixture-package".into(),
            package_version: "1.2.3".into(),
            package_integrity: snapshot.package_integrity().into(),
            package_root: root.into(),
            package_real_root: None,
            package_manifest: resolved_file(root, "package.json", manifest),
            runtime: resolved_file(root, "dist/index.js", runtime),
            declarations: resolved_file(root, "types/index.d.ts", declarations),
            runtime_trace: ResolutionTrace::default(),
            declaration_trace: ResolutionTrace::default(),
            closure,
            transform: None,
            exports,
            authority: ResolutionAuthority::Host,
        };

        let verified =
            super::export_bindings::verify_snapshot_exports(&snapshot, &resolution, &resolved)
                .unwrap();
        assert_eq!(verified.binding_count(), 3);

        let mut omitted = resolved.clone();
        omitted.exports.remove("shared");
        assert!(matches!(
            super::export_bindings::verify_snapshot_exports(&snapshot, &resolution, &omitted),
            Err(ArtifactSnapshotError::ExportBindings(_))
        ));

        let mut aliased = resolved.clone();
        aliased
            .exports
            .get_mut("publicName")
            .unwrap()
            .runtime
            .module = resolved_file(root, "dist/runtime-copy.js", runtime_target);
        assert!(matches!(
            super::export_bindings::verify_snapshot_exports(&snapshot, &resolution, &aliased),
            Err(ArtifactSnapshotError::ResolutionMismatch(_))
        ));

        let mut renamed = resolved;
        renamed
            .exports
            .get_mut("publicName")
            .unwrap()
            .declarations
            .export_name = "runtimeName".into();
        assert!(matches!(
            super::export_bindings::verify_snapshot_exports(&snapshot, &resolution, &renamed),
            Err(ArtifactSnapshotError::ExportBindings(_))
        ));
    }

    #[test]
    fn wildcard_entrypoint_and_condition_order_are_replayed_from_manifest_bytes() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3","exports":{"./features/*":{"types":"./types/*.d.ts","import":"./dist/*.js","default":"./dist/default.js"}}}"#;
        let runtime = b"export const feature = true;";
        let declarations = b"export declare const feature: true;";
        let archive = published_archive(&[
            ("package/package.json", manifest),
            ("package/dist/a.js", runtime),
            ("package/dist/default.js", b"export const feature = false;"),
            ("package/types/a.d.ts", declarations),
        ]);
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let root = "/project/node_modules/fixture-package";
        let package_manifest = resolved_file(root, "package.json", manifest);
        let runtime_file = resolved_file(root, "dist/a.js", runtime);
        let declaration_file = resolved_file(root, "types/a.d.ts", declarations);
        let closure = ClosureManifest::new(
            vec![
                closure_entry(ClosureFileRole::Manifest, "package.json", manifest),
                closure_entry(ClosureFileRole::Runtime, "dist/a.js", runtime),
                closure_entry(ClosureFileRole::Declaration, "types/a.d.ts", declarations),
            ],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let request = ImportRequest {
            specifier: "fixture-package/features/a".into(),
            importer: "/project/src/app.ts".into(),
            export_conditions: vec!["import".into()],
        };
        let resolved = ResolvedImport {
            specifier: request.specifier.clone(),
            importer: request.importer.clone(),
            requested_entrypoint: "./features/a".into(),
            package_name: "fixture-package".into(),
            package_version: "1.2.3".into(),
            package_integrity: snapshot.package_integrity().into(),
            package_root: root.into(),
            package_real_root: None,
            package_manifest,
            runtime: runtime_file,
            declarations: declaration_file,
            runtime_trace: ResolutionTrace {
                branch: "/exports/.~1features~1*/import".into(),
                steps: vec![
                    ResolutionTraceStep {
                        condition: "subpath".into(),
                        target: "./features/a".into(),
                    },
                    ResolutionTraceStep {
                        condition: "import".into(),
                        target: "/exports/.~1features~1*".into(),
                    },
                    ResolutionTraceStep {
                        condition: "target".into(),
                        target: "./dist/a.js".into(),
                    },
                ],
            },
            declaration_trace: ResolutionTrace {
                branch: "/exports/.~1features~1*/types".into(),
                steps: vec![
                    ResolutionTraceStep {
                        condition: "subpath".into(),
                        target: "./features/a".into(),
                    },
                    ResolutionTraceStep {
                        condition: "types".into(),
                        target: "/exports/.~1features~1*".into(),
                    },
                    ResolutionTraceStep {
                        condition: "target".into(),
                        target: "./types/a.d.ts".into(),
                    },
                ],
            },
            closure,
            transform: None,
            exports: BTreeMap::new(),
            authority: ResolutionAuthority::Host,
        };

        let verified = snapshot
            .verify_resolved_import(&request, &resolved)
            .unwrap();
        assert_eq!(verified.runtime_path(), "dist/a.js");
        assert_eq!(verified.declarations_path(), "types/a.d.ts");

        let mut reordered = resolved;
        reordered.runtime_trace.steps.swap(1, 2);
        assert!(matches!(
            snapshot.verify_resolved_import(&request, &reordered),
            Err(ArtifactSnapshotError::ResolutionMismatch(_))
        ));
    }
}
