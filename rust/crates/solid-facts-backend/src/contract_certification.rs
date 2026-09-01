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
        CertificationCandidates, DemandPlanningError, DependencyDemandInput, ProofDemandGraph,
        ProofFamily, ProofWitnessVariant, WitnessBinding, WitnessCoverage, proof_policy_2,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
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
mod dependencies;
mod export_bindings;
mod finalization;
mod module_closure;
mod policy2_receipt;
mod probe_gates;
mod type_facts;
mod witness_wire;

#[cfg(feature = "dialect-v2")]
pub use compiler_facts::{
    CompilerCertificationConfiguration, CompilerCertificationError, CompilerCertificationSchedule,
    LiveCompilerEvidenceBatch, VerifiedCompilerEvidence,
};
pub use dependencies::{
    CanonicalDependencyNodeIdentity, DependencyCompositionError, DependencyCompositionRequirement,
    DependencyCompositionSchedule, DependencyNodeIdentity, DependencyQueueNode,
    DependencyReceiptCompositionError, FinalizedGraphNode, FinalizedPolicy2Graph,
    PublishedContractGraphPlan, PublishedGraphCertificationError, PublishedGraphLockSelection,
    PublishedGraphNodeRequest, PublishedGraphPlanningError, PublishedGraphSourceRequest,
    VerifiedDependencyComposition, certify_published_contract_graph_case_set,
    plan_published_contract_graph,
};
pub use export_bindings::SnapshotVerifiedExports;
pub use finalization::{FinalizedPolicy2Contract, Policy2FinalizationError};
pub use module_closure::SnapshotVerifiedClosure;
#[doc(hidden)]
pub use policy2_receipt::{
    AuthenticatedPolicy2Receipt, BuiltInReceiptEntry, ConfiguredReceiptIssuer,
    Policy2ReceiptBindings, Policy2ReceiptError, Policy2ReceiptProvenance,
    Policy2TrustConfiguration, Policy2TrustEntry, Policy2TrustStore, PublishedPolicy2Catalog,
    ReceiptIssuerKind, ReceiptPublicationError, authenticate_policy2_receipt,
    canonicalize_policy2_main, decode_policy2_trust_configuration,
    encode_policy2_trust_configuration, issue_builtin_policy2_receipt, issue_policy2_receipt,
    policy2_main_semantic_digest, policy2_policy_digest, policy2_resolved_import_root,
    policy2_trust_configuration_for_issuer, publish_policy2_catalog,
};
pub use probe_gates::{
    InspectedProbeGateBatch, ProbeGate, ProbeGateError, ProbeGateOutcome, ProbeGateOutcomeKind,
    ProbeGateSchedule, VerifiedProbeGateBatch,
};
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PublishedArchive {
    registry_origin: String,
    package_name: String,
    package_version: String,
    registry_metadata: Vec<u8>,
    archive: Vec<u8>,
}

/// Transaction-local reuse of fully verified immutable published snapshots.
///
/// Equality covers the complete acquisition identity and bytes: registry
/// origin, package coordinates, registry metadata, and archive. The cache
/// deliberately retains no resolution, closure, export, demand, Type Facts,
/// or receipt state, so every request still replays all proof-bearing work
/// against its own source program and graph root.
#[derive(Default)]
pub struct CertificationPlanningTransaction {
    published_snapshots: HashMap<PublishedArchive, ArtifactSnapshot>,
}

impl CertificationPlanningTransaction {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn plan_certification(
        &mut self,
        request: CertificationRequest,
        artifact: UntrustedArtifactEnvelope,
    ) -> Result<CertificationPlan, CertificationPlanningError> {
        plan_certification_with_dependencies(self, request, artifact, &[])
    }

    pub fn plan_contract_document(
        &mut self,
        document: &[u8],
        import_request: ImportRequest,
        resolved_import: ResolvedImport,
        artifact: UntrustedArtifactEnvelope,
    ) -> Result<CertificationPlan, CertificationPlanningError> {
        let candidate = crate::contract_document::decode(document)?.normalize()?;
        self.plan_certification(
            CertificationRequest::new(candidate, import_request, resolved_import),
            artifact,
        )
    }

    /// Plans an ordinary root certification together with the declaration-only
    /// packages whose typings its own declarations reference.
    ///
    /// The sources travel through the same authenticated channel a published
    /// graph node uses: each one is an integrity-verified published archive
    /// replayed against an exact lock selection. They are evidence material
    /// only — the demand graph, snapshot root, selected artifact case, and
    /// every semantic claim are identical to [`Self::plan_contract_document`]
    /// with the same document.
    ///
    /// Because they are only evidence, a source that will not authenticate is
    /// dropped rather than refused, and supplying none is not a failure. In
    /// both cases the witness program simply cannot resolve those references
    /// and the demands that need them stay open, which is exactly the answer
    /// this path gives today.
    pub fn plan_contract_document_with_sources(
        &mut self,
        document: &[u8],
        import_request: ImportRequest,
        resolved_import: ResolvedImport,
        artifact: UntrustedArtifactEnvelope,
        sources: Vec<PublishedGraphSourceRequest>,
    ) -> Result<CertificationPlan, CertificationPlanningError> {
        let mut plan =
            self.plan_contract_document(document, import_request, resolved_import, artifact)?;
        let authenticated = dependencies::retain_authenticated_source_packages(self, sources);
        plan.certification_sources =
            type_facts::retain_collision_free_source_packages(&plan, authenticated);
        Ok(plan)
    }

    fn published_snapshot(
        &mut self,
        archive: PublishedArchive,
    ) -> Result<ArtifactSnapshot, ArtifactSnapshotError> {
        if let Some(snapshot) = self.published_snapshots.get(&archive) {
            return Ok(snapshot.clone());
        }
        let snapshot = ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2())?;
        self.published_snapshots.insert(archive, snapshot.clone());
        Ok(snapshot)
    }
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
    selected_candidate: NormalizedContract,
    candidates: CertificationCandidates,
    demand_graph: ProofDemandGraph,
    artifact_witnesses: Vec<WitnessBinding>,
    import_request: ImportRequest,
    resolved_import: ResolvedImport,
    /// Declaration-only packages this plan already authenticated. They supply
    /// the type-providing closure the witness program needs to resolve a
    /// cross-package reference; they never contribute a semantic claim, a
    /// dependency receipt, or a runtime module to this plan.
    certification_sources: Vec<dependencies::VerifiedGraphSourcePackage>,
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

    /// Exact artifact case retained after independent resolution replay.
    /// Batch orchestration may use this identity only for census checks; it
    /// cannot replace the opaque plan or affect receipt finalization.
    #[must_use]
    pub fn selected_artifact_case_id(&self) -> &str {
        &self.selected_candidate.artifact_cases()[0].id
    }

    #[must_use]
    pub fn snapshot_root(&self) -> &str {
        self.snapshot.root()
    }

    /// Canonical root over the declaration-only closure this plan will
    /// materialize into its witness program.
    ///
    /// Every Type Facts witness binding folds this in, so a receipt records
    /// which closure proved it and an auditor can tell a full-closure
    /// certification from a partial-closure one. A plan with no sources has its
    /// own well-defined root, so "nothing was supplied" is a statement the
    /// receipt makes rather than an absence. The composition is over the
    /// canonical `VerifiedGraphSourcePackage` identities, which already bind
    /// registry origin, package manager, lockfile digest, lock locator, name,
    /// version, integrity, installed root, snapshot root, and provenance root —
    /// mirroring the graph's `source_dependencies_root`.
    pub(super) fn certification_sources_root(&self) -> String {
        certification_evidence_root(
            "policy2-certification-sources",
            std::iter::once("solid-checker:certification-source-closure:v1").chain(
                self.certification_sources
                    .iter()
                    .map(|source| source.identity.as_str()),
            ),
        )
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

    /// Canonical dependency-composition requirements derived from the exact
    /// snapshot-replayed external edges and every proposed parent closure.
    pub fn dependency_composition_schedule(
        &self,
    ) -> Result<DependencyCompositionSchedule, DependencyCompositionError> {
        dependencies::DependencyCompositionSchedule::from_plan(self)
    }

    /// Mandatory probe vetoes derived from every proposed closure. A complete
    /// successful audit batch still cannot authenticate until the harness and
    /// Node runtime image are directly bound.
    pub fn probe_gate_schedule(&self) -> Result<ProbeGateSchedule, ProbeGateError> {
        probe_gates::ProbeGateSchedule::from_plan(self)
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

    /// Acquires exact exported-value evidence without manufacturing a call.
    pub fn acquire_export_value_type_facts(
        &self,
        pin: &TypeFactsProducerPin,
        project_id: &str,
        schedule: &TypeFactsCertificationSchedule,
    ) -> Result<typefacts::LiveExportValueAnswer, TypeFactsCertificationError> {
        let mut session = type_facts::TypeFactsCertificationSession::open(pin, project_id)?;
        session.acquire_export_values(self, schedule)
    }

    /// Reconciles a live exported-value answer with its exact proof subjects.
    pub fn verify_export_value_type_facts(
        &self,
        schedule: &TypeFactsCertificationSchedule,
        answer: &typefacts::LiveExportValueAnswer,
    ) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
        type_facts::verify_live_export_value_answer(self, schedule, answer)
    }

    /// Executes the complete opaque exported-value Type Facts transaction.
    /// No harness path, source location, schedule, or serialized answer is
    /// accepted from the caller.
    pub fn acquire_and_verify_export_value_type_facts(
        &self,
        pin: &TypeFactsProducerPin,
    ) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
        type_facts::acquire_and_verify_export_values(self, pin)
    }

    /// Certifies the supported value-only cohort in one native transaction.
    pub fn certify_value_only(
        &self,
        canonical_proposal: &[u8],
        pin: &TypeFactsProducerPin,
        issuer: &ConfiguredReceiptIssuer,
        revocation_epoch: u64,
    ) -> Result<FinalizedPolicy2Contract, Policy2FinalizationError> {
        let evidence = type_facts::acquire_and_verify_export_values(self, pin)?;
        finalization::finalize_value_only(
            self,
            canonical_proposal,
            &evidence,
            pin,
            issuer,
            revocation_epoch,
        )
    }

    /// Atomically publishes a final result against the exact resolved import
    /// retained by this opaque plan.
    pub fn publish_finalized_policy2(
        &self,
        catalog_root: &Path,
        finalized: &FinalizedPolicy2Contract,
    ) -> Result<PublishedPolicy2Catalog, ReceiptPublicationError> {
        publish_policy2_catalog(
            catalog_root,
            finalized.canonical_main(),
            finalized.receipt(),
            finalized.authenticated(),
            &self.resolved_import,
        )
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

/// Finalizes a complete set of alternative artifact cases while sharing only
/// immutable Type Facts setup. Evidence and receipts remain one-per-plan and
/// each is checked against its own demand graph before this returns anything.
pub fn certify_value_only_case_set(
    plans: &[&CertificationPlan],
    canonical_proposal: &[u8],
    pin: &TypeFactsProducerPin,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
) -> Result<Vec<FinalizedPolicy2Contract>, Policy2FinalizationError> {
    let evidence = type_facts::acquire_and_verify_export_values_batch(plans, pin)?;
    plans
        .iter()
        .zip(evidence)
        .map(|(plan, evidence)| {
            finalization::finalize_value_only(
                plan,
                canonical_proposal,
                &evidence,
                pin,
                issuer,
                revocation_epoch,
            )
        })
        .collect()
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
    CertificationPlanningTransaction::new().plan_certification(request, artifact)
}

fn plan_certification_with_dependencies(
    transaction: &mut CertificationPlanningTransaction,
    request: CertificationRequest,
    artifact: UntrustedArtifactEnvelope,
    dependencies: &[&CertificationPlan],
) -> Result<CertificationPlan, CertificationPlanningError> {
    let snapshot = match artifact {
        UntrustedArtifactEnvelope::Published(archive) => transaction.published_snapshot(archive)?,
        UntrustedArtifactEnvelope::LockPinned(archive) => {
            ArtifactSnapshot::from_lock_pinned(&archive, SnapshotLimits::policy_2())?
        }
        UntrustedArtifactEnvelope::Local(artifact) => {
            ArtifactSnapshot::from_local(&artifact, SnapshotLimits::policy_2())?
        }
    };
    let verified_resolution =
        snapshot.verify_resolved_import(&request.import_request, &request.resolved_import)?;
    let verified_closure = module_closure::verify_snapshot_closure(
        &snapshot,
        &verified_resolution,
        &request.resolved_import.closure,
    )?;
    let verified_exports = export_bindings::verify_snapshot_exports_with_dependencies(
        &snapshot,
        &verified_resolution,
        &request.resolved_import,
        dependencies,
    )?;
    let external_targets = dependencies
        .iter()
        .flat_map(|dependency| dependency.resolved_import.exports.values())
        .flat_map(|binding| [&binding.runtime.module, &binding.declarations.module])
        .map(|module| (module.path.clone(), module.digest.clone()))
        .collect();
    let selected = crate::artifact_resolution::select_and_bind_with_external_targets(
        &request.candidate,
        &request.resolved_import,
        &external_targets,
    )?;
    // Exact per-export re-export targets are intentionally receipt evidence,
    // not stable-v1 main-document fields. Round-trip the independently bound
    // selection through the public document boundary before deriving its
    // semantic digest, while `verified_exports` and artifact witnesses retain
    // the exact target declarations and digests. Otherwise planning hashes an
    // internal identity that ordinary discovery can never reconstruct.
    let selected = crate::contract_document::decode(&crate::contract_document::encode(
        &selected,
        &crate::contract_document::SidecarDigests::default(),
        false,
    )?)?
    .normalize()?;
    let policy = proof_policy_2();
    let candidates = policy
        .inspect_candidates(&selected)
        .map_err(|error| CertificationPlanningError::InvalidCandidate(error.to_string()))?;
    let demand_graph = policy.derive_demand_graph_with_dependencies(
        &candidates,
        snapshot.root(),
        snapshot.provenance_root(),
        verified_closure
            .manifest()
            .dependencies
            .iter()
            .map(|dependency| DependencyDemandInput {
                specifier: dependency.specifier.clone(),
                package: dependency.package_name.clone(),
                artifact_case: dependency.artifact_case.clone(),
                accepted_contract_digest: dependency.accepted_contract_digest.clone(),
            }),
    )?;
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
        selected_candidate: selected,
        candidates,
        demand_graph,
        artifact_witnesses,
        import_request: request.import_request,
        resolved_import: request.resolved_import,
        certification_sources: Vec::new(),
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
    /// The bundler ESM entry of a legacy dual package. It is not a Node field,
    /// so a package that declares it as anything other than a string is still
    /// resolvable through `main`: a non-string value degrades to "not declared"
    /// instead of refusing the package.
    #[serde(default, deserialize_with = "optional_module_target")]
    module: Option<String>,
    #[serde(default)]
    types: Option<String>,
    #[serde(default)]
    typings: Option<String>,
}

fn optional_module_target<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(target) => Some(target),
        _ => None,
    })
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
    /// A conditional target selected no active condition. Split out from
    /// `Refusal` because it is the one outcome Node's PACKAGE_TARGET_RESOLVE
    /// backtracks over: an enclosing conditional object continues to its next
    /// key rather than refusing. Every other refusal is a property of the
    /// package and still stops resolution where it happens.
    ConditionsUnmatched(String),
}

impl TargetSelectionError {
    fn into_snapshot_error(self) -> ArtifactSnapshotError {
        let reason = match self {
            Self::InvalidTarget(reason)
            | Self::Refusal(reason)
            | Self::ConditionsUnmatched(reason) => reason,
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
    files: Arc<BTreeMap<String, Arc<[u8]>>>,
    directories: Arc<BTreeSet<String>>,
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
        let mut explicit_directories = BTreeSet::new();
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
                if entry.size() != 0 {
                    return Err(ArtifactSnapshotError::InvalidArchive(format!(
                        "directory member {package_path} has a nonzero payload"
                    )));
                }
                if files.contains_key(&package_path) {
                    return Err(ArtifactSnapshotError::DuplicateMember(package_path));
                }
                explicit_directories.insert(package_path);
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
            if explicit_directories.contains(&package_path) {
                return Err(ArtifactSnapshotError::DuplicateMember(package_path));
            }
            if let Some(first) = files.get(&package_path) {
                if first.as_ref() == bytes.as_slice() {
                    continue;
                }
                return Err(ArtifactSnapshotError::DuplicateMember(package_path));
            }
            files.insert(package_path, Arc::from(bytes));
        }
        if files.is_empty() {
            return Err(ArtifactSnapshotError::InvalidArchive(
                "archive contains no package files".into(),
            ));
        }
        validate_topology(&files, &explicit_directories)?;
        let directories = derive_directories(files.keys());
        validate_manifest_identity(&files, &package_name, &package_version)?;
        let root = snapshot_root(&package_name, &package_version, &files, &directories);
        let provenance_root = provenance_root(&provenance, &root);
        Ok(Self {
            package_name,
            package_version,
            package_integrity,
            files: Arc::new(files),
            directories: Arc::new(directories),
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
                    Err(
                        error @ (TargetSelectionError::Refusal(_)
                        | TargetSelectionError::ConditionsUnmatched(_)),
                    ) => return Err(error),
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
            // Node's PACKAGE_TARGET_RESOLVE continues to the next key when a
            // key's own target resolves to nothing, so `{"vendor": {"browser":
            // "./a.js"}, "default": "./index.js"}` under conditions ["vendor"]
            // resolves to ./index.js. Taking the first *matching* key and
            // refusing there instead would reject a package every real consumer
            // resolves fine. Only `ConditionsUnmatched` backtracks; a null
            // (blocked) target, a missing snapshot file, and an invalid target
            // are properties of the package and still refuse immediately.
            //
            // The abandoned branch's steps are discarded, never merged: each key
            // clones `steps` rather than extending a shared vector, so the trace
            // that survives describes exactly the branch actually taken. This
            // mirrors `selectTarget` in packages/cli/scripts/artifact-resolution.mjs
            // step for step -- the generator and this replay must select the
            // same target and produce the same trace.
            for (condition, nested) in fields {
                if condition != "default" && !conditions.contains(condition.as_str()) {
                    continue;
                }
                let mut next_steps = steps.clone();
                next_steps.push(ResolutionTraceStep {
                    condition: condition.clone(),
                    target: pointer.into(),
                });
                match select_target(
                    nested,
                    snapshot,
                    entrypoint,
                    capture,
                    conditions,
                    &format!("{pointer}/{}", pointer_segment(condition)),
                    next_steps,
                ) {
                    Err(TargetSelectionError::ConditionsUnmatched(_)) => continue,
                    other => return other,
                }
            }
            Err(TargetSelectionError::ConditionsUnmatched(format!(
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
    } else if let Some(target) = legacy_module_target(snapshot, manifest) {
        ("module", target)
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

/// The runtime target a legacy `module` field names, when it names a snapshot
/// file. `module` is the bundler's ESM entry of a dual package whose `main` is
/// usually the CJS transpile of the same source, so preferring it on the
/// runtime axis analyzes the ESM build instead of refusing the package. A
/// declared-but-absent (or unresolvable) target is not a refusal: Node
/// consumers never read `module`, so the `main` surface is still real.
fn legacy_module_target<'a>(
    snapshot: &ArtifactSnapshot,
    manifest: &'a SnapshotPackageManifest,
) -> Option<&'a str> {
    let target = manifest.module.as_deref()?;
    let path = validate_legacy_target(target).ok()?;
    snapshot.read(&path).is_some().then_some(target)
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
    if DECLARATIONS
        .iter()
        .any(|extension| path.ends_with(extension))
    {
        return snapshot.read(path).is_some().then(|| path.into());
    }
    let extension = node_path_extension(path);
    let stem = &path[..path.len() - extension.len()];
    if let Some((declaration_extension, source_fallback)) = match extension {
        ".mjs" | ".mts" => Some((".d.mts", false)),
        ".cjs" | ".cts" => Some((".d.cts", false)),
        ".js" | ".jsx" | ".ts" | ".tsx" => Some((".d.ts", true)),
        _ => None,
    } {
        let candidate = format!("{stem}{declaration_extension}");
        if snapshot.read(&candidate).is_some() {
            return Some(candidate);
        }
        return (source_fallback && snapshot.read(path).is_some()).then(|| path.into());
    }

    if extension.is_empty() {
        for candidate in DECLARATIONS
            .iter()
            .map(|extension| format!("{path}{extension}"))
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
    }
    None
}

/// Mirrors `node:path.extname` for validated package-relative POSIX paths.
/// A basename made of one leading dot plus non-dot characters is extensionless
/// (`.mjs`), while a second dot starts an extension (`.index.mjs`). Snapshot
/// replay must classify the selected target exactly as the JavaScript generator.
fn node_path_extension(path: &str) -> &str {
    let basename = path.rsplit('/').next().unwrap_or(path);
    if matches!(basename, "." | "..") {
        return "";
    }
    let Some(dot) = basename.rfind('.') else {
        return "";
    };
    if dot == 0 {
        return "";
    }
    &basename[dot..]
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

fn validate_topology(
    files: &BTreeMap<String, Arc<[u8]>>,
    explicit_directories: &BTreeSet<String>,
) -> Result<(), ArtifactSnapshotError> {
    let folded_files: BTreeMap<String, &String> = files
        .keys()
        .map(|path| (path.to_lowercase(), path))
        .collect();
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
            if let Some(file) = folded_files.get(&prefix.to_lowercase()) {
                return Err(ArtifactSnapshotError::InvalidArchive(format!(
                    "{file} is both a file and a directory"
                )));
            }
        }
    }
    for directory in explicit_directories {
        let mut prefix = String::new();
        for component in directory
            .split('/')
            .filter(|component| !component.is_empty())
        {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            if let Some(file) = folded_files.get(&prefix.to_lowercase()) {
                return Err(ArtifactSnapshotError::InvalidArchive(format!(
                    "{file} is both a file and a directory"
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
        ArtifactSnapshot, ArtifactSnapshotError, CertificationPlan,
        CertificationPlanningTransaction, CertificationRequest, ConfiguredReceiptIssuer,
        DependencyReceiptCompositionError, LocalArtifact, LockPinnedArchive,
        Policy2ReceiptBindings, Policy2ReceiptProvenance, PublishedArchive,
        PublishedGraphLockSelection, PublishedGraphNodeRequest, PublishedGraphPlanningError,
        PublishedGraphSourceRequest, ResolutionAxis, SnapshotLimits, SnapshotPackageManifest,
        SnapshotVerifiedResolution, UntrustedArtifactEnvelope, authenticate_policy2_receipt,
        declaration_candidate, issue_policy2_receipt, plan_certification,
        plan_published_contract_graph, policy2_main_semantic_digest,
        policy2_trust_configuration_for_issuer, resolve_snapshot_export,
    };
    use crate::artifact_resolution::{
        AcceptedDependencyEdge, AffectedClaimDomain, ClosureEntry, ClosureFileRole, ClosureHazard,
        ClosureHazardKind, ClosureManifest, ImportRequest, ResolutionAuthority, ResolutionTrace,
        ResolutionTraceStep, ResolvedExportBinding, ResolvedExportTarget, ResolvedFile,
        ResolvedImport,
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use flate2::{Compression, write::GzEncoder};
    use sha2::{Digest as _, Sha256, Sha512};
    use solid_reactive_ir::contract_semantics::{
        CallClaims, CallSemantics, ClaimDomain, ClaimPath, ContractProposal, ExportIdentity,
        ExportSemantics, ExportTargetIdentity, GuardPartition, KnowledgeSet, SemanticClaimPath,
        SemanticClaimSubject, StabilityKnowledge, ValueShape,
    };
    use std::collections::{BTreeMap, BTreeSet};

    use std::io::Write as _;
    use std::sync::Arc;

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
            r#"{{"versions":{{"{version}":{{"name":"{name}","version":"{version}","dist":{{"integrity":"{integrity}","tarball":"{tarball}"}}}}}}}}"#
        )
        .into_bytes()
    }

    fn published_archive(files: &[(&str, &[u8])]) -> PublishedArchive {
        published_from_bytes(archive_bytes(files))
    }

    fn published_archive_for(
        name: &str,
        version: &str,
        files: &[(&str, &[u8])],
    ) -> PublishedArchive {
        let archive = archive_bytes(files);
        let integrity = format!("sha512-{}", STANDARD.encode(Sha512::digest(&archive)));
        let tarball = format!("https://registry.npmjs.org/{name}/-/{name}-{version}.tgz");
        PublishedArchive::new(
            "https://registry.npmjs.org",
            name,
            version,
            registry_metadata(&integrity, name, version, &tarball),
            archive,
        )
        .expect("published coordinates")
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
    fn planning_transaction_reuses_only_the_exact_verified_published_archive() {
        let archive = published_archive(&fixture_files());
        let mut transaction = CertificationPlanningTransaction::new();

        let first = transaction.published_snapshot(archive.clone()).unwrap();
        let reused = transaction.published_snapshot(archive.clone()).unwrap();
        assert_eq!(first.root(), reused.root());
        assert_eq!(first.provenance_root(), reused.provenance_root());
        assert!(Arc::ptr_eq(&first.files, &reused.files));
        assert!(Arc::ptr_eq(&first.directories, &reused.directories));
        assert!(Arc::ptr_eq(
            first.files.get("dist/index.js").unwrap(),
            reused.files.get("dist/index.js").unwrap(),
        ));
        assert_eq!(transaction.published_snapshots.len(), 1);

        let mut independent_transaction = CertificationPlanningTransaction::new();
        let independently_verified = independent_transaction
            .published_snapshot(archive.clone())
            .unwrap();
        assert_eq!(first.root(), independently_verified.root());
        assert_eq!(
            first.provenance_root(),
            independently_verified.provenance_root()
        );
        assert!(!Arc::ptr_eq(&first.files, &independently_verified.files));
        assert!(!Arc::ptr_eq(
            &first.directories,
            &independently_verified.directories
        ));
        assert!(!Arc::ptr_eq(
            first.files.get("dist/index.js").unwrap(),
            independently_verified.files.get("dist/index.js").unwrap(),
        ));
        assert_eq!(independent_transaction.published_snapshots.len(), 1);

        let mut changed_metadata = archive.clone();
        changed_metadata.registry_metadata.push(b'\n');
        let metadata_snapshot = transaction.published_snapshot(changed_metadata).unwrap();
        assert_eq!(first.root(), metadata_snapshot.root());
        assert_ne!(first.provenance_root(), metadata_snapshot.provenance_root());
        assert!(!Arc::ptr_eq(&first.files, &metadata_snapshot.files));
        assert!(!Arc::ptr_eq(
            &first.directories,
            &metadata_snapshot.directories
        ));
        assert!(!Arc::ptr_eq(
            first.files.get("dist/index.js").unwrap(),
            metadata_snapshot.files.get("dist/index.js").unwrap(),
        ));

        let changed_archive = published_archive(&[
            fixture_files()[0],
            ("package/dist/index.js", b"export const answer = 43;"),
        ]);
        let changed_snapshot = transaction.published_snapshot(changed_archive).unwrap();
        assert_ne!(first.root(), changed_snapshot.root());
        assert!(!Arc::ptr_eq(&first.files, &changed_snapshot.files));
        assert!(!Arc::ptr_eq(
            &first.directories,
            &changed_snapshot.directories
        ));

        let alternate_origin = "https://registry.example.invalid";
        let alternate = PublishedArchive::new(
            alternate_origin,
            "fixture-package",
            "1.2.3",
            registry_metadata(
                first.package_integrity(),
                "fixture-package",
                "1.2.3",
                &format!("{alternate_origin}/fixture-package/-/fixture-package-1.2.3.tgz"),
            ),
            archive.archive.clone(),
        )
        .unwrap();
        let alternate_snapshot = transaction.published_snapshot(alternate).unwrap();
        assert_eq!(first.root(), alternate_snapshot.root());
        assert_ne!(
            first.provenance_root(),
            alternate_snapshot.provenance_root()
        );
        assert!(!Arc::ptr_eq(&first.files, &alternate_snapshot.files));
        assert!(!Arc::ptr_eq(
            &first.directories,
            &alternate_snapshot.directories
        ));
        assert_eq!(transaction.published_snapshots.len(), 4);

        let mismatched_origin = PublishedArchive::new(
            alternate_origin,
            "fixture-package",
            "1.2.3",
            archive.registry_metadata.clone(),
            archive.archive.clone(),
        )
        .unwrap();
        assert!(matches!(
            transaction.published_snapshot(mismatched_origin),
            Err(ArtifactSnapshotError::InvalidProvenance(_))
        ));
        let mut corrupt = archive;
        corrupt.archive[0] ^= 1;
        assert_eq!(
            transaction.published_snapshot(corrupt).unwrap_err(),
            ArtifactSnapshotError::IntegrityMismatch,
        );
        assert_eq!(transaction.published_snapshots.len(), 4);
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
            (
                "package/package.json",
                b'0',
                "",
                br#"{"name":"fixture-package","version":"9.9.9"}"#,
            ),
        ]));
        assert!(matches!(
            ArtifactSnapshot::from_published(&duplicate, SnapshotLimits::policy_2()),
            Err(ArtifactSnapshotError::DuplicateMember(_))
        ));

        let collision = published_from_bytes(raw_archive(&[
            ("package/package.json", b'0', "", manifest),
            ("package/Dist/index.js", b'0', "", b"same"),
            ("package/dist/index.js", b'0', "", b"same"),
        ]));
        assert!(matches!(
            ArtifactSnapshot::from_published(&collision, SnapshotLimits::policy_2()),
            Err(ArtifactSnapshotError::CaseCollision { .. })
        ));

        for members in [
            vec![
                ("package/package.json", b'0', "", manifest.as_slice()),
                ("package/dist", b'5', "", b"".as_slice()),
                ("package/dist", b'0', "", b"file".as_slice()),
            ],
            vec![
                ("package/package.json", b'0', "", manifest.as_slice()),
                ("package/dist", b'0', "", b"file".as_slice()),
                ("package/dist", b'5', "", b"".as_slice()),
            ],
            vec![
                ("package/package.json", b'0', "", manifest.as_slice()),
                ("package/dist", b'0', "", b"file".as_slice()),
                ("package/dist/child", b'5', "", b"".as_slice()),
            ],
            vec![
                ("package/package.json", b'0', "", manifest.as_slice()),
                ("package/dist/child", b'5', "", b"".as_slice()),
                ("package/dist", b'0', "", b"file".as_slice()),
            ],
        ] {
            let kind_collision = published_from_bytes(raw_archive(&members));
            assert!(
                ArtifactSnapshot::from_published(&kind_collision, SnapshotLimits::policy_2())
                    .is_err()
            );
        }

        for kind in *b"12" {
            for members in [
                vec![
                    ("package/package.json", b'0', "", manifest.as_slice()),
                    ("package/link", b'0', "", b"".as_slice()),
                    ("package/link", kind, "target", b"".as_slice()),
                ],
                vec![
                    ("package/package.json", b'0', "", manifest.as_slice()),
                    ("package/link", kind, "target", b"".as_slice()),
                    ("package/link", b'0', "", b"".as_slice()),
                ],
            ] {
                let unsupported_collision = published_from_bytes(raw_archive(&members));
                assert!(
                    ArtifactSnapshot::from_published(
                        &unsupported_collision,
                        SnapshotLimits::policy_2()
                    )
                    .is_err()
                );
            }
        }

        for members in [
            vec![
                ("package/package.json", b'0', "", manifest.as_slice()),
                ("package/Dist", b'0', "", b"file".as_slice()),
                ("package/dist/child", b'0', "", b"child".as_slice()),
            ],
            vec![
                ("package/package.json", b'0', "", manifest.as_slice()),
                ("package/dist/child", b'0', "", b"child".as_slice()),
                ("package/Dist", b'0', "", b"file".as_slice()),
            ],
            vec![
                ("package/package.json", b'0', "", manifest.as_slice()),
                ("package/Dist", b'0', "", b"file".as_slice()),
                ("package/dist/child", b'5', "", b"".as_slice()),
            ],
            vec![
                ("package/package.json", b'0', "", manifest.as_slice()),
                ("package/dist/child", b'5', "", b"".as_slice()),
                ("package/Dist", b'0', "", b"file".as_slice()),
            ],
        ] {
            let folded_topology = published_from_bytes(raw_archive(&members));
            assert!(
                ArtifactSnapshot::from_published(&folded_topology, SnapshotLimits::policy_2())
                    .is_err()
            );
        }

        let directory_payload = published_from_bytes(raw_archive(&[
            ("package/package.json", b'0', "", manifest),
            ("package/dist", b'5', "", b"payload"),
        ]));
        let mut limits = SnapshotLimits::policy_2();
        limits.expanded_archive_bytes = manifest.len();
        assert!(
            ArtifactSnapshot::from_published(&directory_payload, limits).is_err(),
            "a directory payload must not bypass the expanded-byte limit"
        );

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
    fn byte_identical_duplicate_archive_files_are_idempotent_and_still_counted() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3"}"#;
        let single = published_from_bytes(raw_archive(&[
            ("package/package.json", b'0', "", manifest),
            ("package/dist/index.js", b'0', "", b"same"),
        ]));
        let duplicate = published_from_bytes(raw_archive(&[
            ("package/package.json", b'0', "", manifest),
            ("package/./dist/index.js", b'0', "", b"same"),
            ("package/dist/index.js", b'0', "", b"same"),
            ("package/dist", b'5', "", b""),
            ("package/./dist", b'5', "", b""),
        ]));

        let single_snapshot =
            ArtifactSnapshot::from_published(&single, SnapshotLimits::policy_2()).unwrap();
        let duplicate_snapshot =
            ArtifactSnapshot::from_published(&duplicate, SnapshotLimits::policy_2()).unwrap();
        assert_eq!(single_snapshot.root(), duplicate_snapshot.root());
        assert_eq!(
            single_snapshot.member_count(),
            duplicate_snapshot.member_count()
        );

        let conflicting = published_from_bytes(raw_archive(&[
            ("package/package.json", b'0', "", manifest),
            ("package/./dist/index.js", b'0', "", b"first"),
            ("package/dist/index.js", b'0', "", b"second"),
        ]));
        assert!(matches!(
            ArtifactSnapshot::from_published(&conflicting, SnapshotLimits::policy_2()),
            Err(ArtifactSnapshotError::DuplicateMember(_))
        ));

        let mut limits = SnapshotLimits::policy_2();
        limits.expanded_archive_bytes = manifest.len() + b"same".len();
        assert!(matches!(
            ArtifactSnapshot::from_published(&duplicate, limits),
            Err(ArtifactSnapshotError::ResourceLimit(_))
        ));
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

    fn exports_resolution(
        manifest: &[u8],
        files: &[(&str, &[u8])],
        entrypoint: &str,
        conditions: &[&str],
        axis: ResolutionAxis,
    ) -> Result<(String, ResolutionTrace), ArtifactSnapshotError> {
        let mut members = vec![("package/package.json", manifest)];
        members.extend(files.iter().copied());
        let snapshot = ArtifactSnapshot::from_published(
            &published_archive(&members),
            SnapshotLimits::policy_2(),
        )
        .unwrap();
        let parsed: SnapshotPackageManifest = serde_json::from_slice(manifest).unwrap();
        let active: BTreeSet<&str> = conditions.iter().copied().collect();
        let selected = resolve_snapshot_export(&snapshot, &parsed, entrypoint, &active, axis)?;
        Ok((selected.path, selected.trace))
    }

    fn declaration_snapshot(files: &[(&str, &[u8])]) -> ArtifactSnapshot {
        let mut members = vec![(
            "package/package.json",
            br#"{"name":"fixture-package","version":"1.2.3"}"# as &[u8],
        )];
        members.extend(files.iter().copied());
        ArtifactSnapshot::from_published(&published_archive(&members), SnapshotLimits::policy_2())
            .unwrap()
    }

    #[test]
    fn declaration_candidates_follow_the_selected_module_format() {
        for (runtime_extension, declaration_extension) in [
            (".mjs", ".d.mts"),
            (".mts", ".d.mts"),
            (".cjs", ".d.cts"),
            (".cts", ".d.cts"),
            (".js", ".d.ts"),
            (".jsx", ".d.ts"),
            (".ts", ".d.ts"),
            (".tsx", ".d.ts"),
        ] {
            let runtime = format!("dist/index{runtime_extension}");
            let matching = format!("dist/index{declaration_extension}");
            let package_runtime = format!("package/{runtime}");
            let files = [
                (
                    package_runtime.as_str(),
                    b"export const value = 1;" as &[u8],
                ),
                (
                    "package/dist/index.d.ts",
                    b"export declare const value: 'd.ts';",
                ),
                (
                    "package/dist/index.d.mts",
                    b"export declare const value: 'd.mts';",
                ),
                (
                    "package/dist/index.d.cts",
                    b"export declare const value: 'd.cts';",
                ),
            ];
            let snapshot = declaration_snapshot(&files);

            assert_eq!(declaration_candidate(&snapshot, &runtime), Some(matching));
        }
    }

    #[test]
    fn declaration_candidates_preserve_source_fallbacks() {
        for extension in [".js", ".jsx", ".ts", ".tsx"] {
            let path = format!("dist/fallback{extension}");
            let package_path = format!("package/{path}");
            let snapshot =
                declaration_snapshot(&[(package_path.as_str(), b"export const value = 1;")]);

            assert_eq!(declaration_candidate(&snapshot, &path), Some(path));
        }
    }

    #[test]
    fn declaration_candidates_do_not_cross_module_formats() {
        for extension in [".mjs", ".mts"] {
            let path = format!("dist/index{extension}");
            let package_path = format!("package/{path}");
            let snapshot = declaration_snapshot(&[
                (package_path.as_str(), b"export const value = 1;"),
                ("package/dist/index.d.ts", b"export declare const value: 1;"),
                (
                    "package/dist/index.d.cts",
                    b"export declare const value: 1;",
                ),
            ]);
            assert_eq!(declaration_candidate(&snapshot, &path), None);
        }
        for extension in [".cjs", ".cts"] {
            let path = format!("dist/index{extension}");
            let package_path = format!("package/{path}");
            let snapshot = declaration_snapshot(&[
                (package_path.as_str(), b"exports.value = 1;"),
                ("package/dist/index.d.ts", b"export declare const value: 1;"),
                (
                    "package/dist/index.d.mts",
                    b"export declare const value: 1;",
                ),
            ]);
            assert_eq!(declaration_candidate(&snapshot, &path), None);
        }
    }

    #[test]
    fn declaration_candidates_keep_direct_multidot_and_extensionless_behavior() {
        let direct_snapshot = declaration_snapshot(&[
            (
                "package/dist/direct.d.mts",
                b"export declare const esm: true;",
            ),
            (
                "package/dist/direct.d.cts",
                b"export declare const cjs: true;",
            ),
            ("package/dist/index.browser.mjs", b"export const value = 1;"),
            (
                "package/dist/index.browser.d.mts",
                b"export declare const value: 1;",
            ),
        ]);

        assert_eq!(
            declaration_candidate(&direct_snapshot, "dist/direct.d.mts"),
            Some("dist/direct.d.mts".into())
        );
        assert_eq!(
            declaration_candidate(&direct_snapshot, "dist/direct.d.cts"),
            Some("dist/direct.d.cts".into())
        );
        assert_eq!(
            declaration_candidate(&direct_snapshot, "dist/index.browser.mjs"),
            Some("dist/index.browser.d.mts".into())
        );

        let candidates = [
            "dist/index.d.ts",
            "dist/index.d.mts",
            "dist/index.d.cts",
            "dist/index/index.d.ts",
            "dist/index/index.d.mts",
            "dist/index/index.d.cts",
        ];
        for first_present in 0..candidates.len() {
            let package_paths: Vec<String> = candidates[first_present..]
                .iter()
                .map(|path| format!("package/{path}"))
                .collect();
            let files: Vec<(&str, &[u8])> = package_paths
                .iter()
                .map(|path| {
                    (
                        path.as_str(),
                        b"export declare const selected: true;" as &[u8],
                    )
                })
                .collect();
            let snapshot = declaration_snapshot(&files);

            assert_eq!(
                declaration_candidate(&snapshot, "dist/index"),
                Some(candidates[first_present].into())
            );
        }
    }

    #[test]
    fn declaration_candidates_treat_leading_dot_basenames_as_extensionless() {
        for (basename, incorrectly_formatted_sibling) in [(".mjs", ".d.mts"), (".js", ".d.ts")] {
            let runtime = format!("dist/{basename}");
            let extensionless_declaration = format!("dist/{basename}.d.ts");
            let package_runtime = format!("package/{runtime}");
            let package_extensionless = format!("package/{extensionless_declaration}");
            let wrong_format = format!("package/dist/{incorrectly_formatted_sibling}");
            let snapshot = declaration_snapshot(&[
                (package_runtime.as_str(), b"export const value = 1;"),
                (
                    package_extensionless.as_str(),
                    b"export declare const selected: true;",
                ),
                (wrong_format.as_str(), b"export declare const wrong: true;"),
            ]);

            assert_eq!(
                declaration_candidate(&snapshot, &runtime),
                Some(extensionless_declaration)
            );
        }
    }

    /// Node's PACKAGE_TARGET_RESOLVE continues to the next key when a matched
    /// key's own nested object selects nothing. Taking the first matching key
    /// and refusing there would reject a package every real consumer resolves
    /// fine -- and, worse, disagree with the generator, whose `selectTarget`
    /// backtracks the same way.
    #[test]
    fn an_unmatched_nested_condition_backtracks_to_the_next_sibling_key() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3","exports":{"./a":{"vendor":{"browser":"./missing-a.js"},"default":"./index.js"},"./b":{"vendor":{"browser":"./b-browser.js"},"default":"./index.js"},"./none":{"vendor":{"browser":"./b-browser.js"}}}}"#;
        let files: &[(&str, &[u8])] = &[
            ("package/index.js", b"export const value = 1;"),
            ("package/b-browser.js", b"export const value = 2;"),
        ];

        // `vendor` matches, its nested object selects nothing, so resolution
        // continues to `default` instead of refusing.
        for entrypoint in ["./a", "./b"] {
            let (path, trace) = exports_resolution(
                manifest,
                files,
                entrypoint,
                &["import", "vendor"],
                ResolutionAxis::Runtime,
            )
            .unwrap();
            assert_eq!(path, "index.js");
            // The abandoned `vendor` branch leaves no trace: the steps describe
            // exactly the branch actually taken.
            assert_eq!(
                trace.steps,
                vec![
                    ResolutionTraceStep {
                        condition: "subpath".into(),
                        target: entrypoint.into(),
                    },
                    ResolutionTraceStep {
                        condition: "default".into(),
                        target: format!("/exports/{}", super::pointer_segment(entrypoint)),
                    },
                    ResolutionTraceStep {
                        condition: "target".into(),
                        target: "./index.js".into(),
                    },
                ]
            );
        }

        // With `browser` active the nested branch does select, and wins.
        let (path, trace) = exports_resolution(
            manifest,
            files,
            "./b",
            &["import", "vendor", "browser"],
            ResolutionAxis::Runtime,
        )
        .unwrap();
        assert_eq!(path, "b-browser.js");
        assert_eq!(trace.branch, "/exports/.~1b/vendor/browser");

        // A matched branch naming a target the artifact does not contain is a
        // property of the package, not an unmatched condition: it refuses where
        // it happens rather than silently falling through to `default`.
        assert!(matches!(
            exports_resolution(
                manifest,
                files,
                "./a",
                &["import", "vendor", "browser"],
                ResolutionAxis::Runtime,
            ),
            Err(ArtifactSnapshotError::ResolutionMismatch(_))
        ));

        // And an object that yields nothing at all still refuses.
        assert!(matches!(
            exports_resolution(
                manifest,
                files,
                "./none",
                &["import", "vendor"],
                ResolutionAxis::Runtime,
            ),
            Err(ArtifactSnapshotError::ResolutionMismatch(_))
        ));
    }

    fn legacy_resolution(
        manifest: &[u8],
        files: &[(&str, &[u8])],
        axis: ResolutionAxis,
    ) -> Result<(String, ResolutionTrace), ArtifactSnapshotError> {
        let mut members = vec![("package/package.json", manifest)];
        members.extend(files.iter().copied());
        let snapshot = ArtifactSnapshot::from_published(
            &published_archive(&members),
            SnapshotLimits::policy_2(),
        )
        .unwrap();
        let parsed: SnapshotPackageManifest = serde_json::from_slice(manifest).unwrap();
        let selected = resolve_snapshot_export(&snapshot, &parsed, ".", &BTreeSet::new(), axis)?;
        Ok((selected.path, selected.trace))
    }

    #[test]
    fn legacy_runtime_resolution_prefers_a_present_module_target_over_main() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3","type":"module","main":"dist/index.cjs","module":"dist/index.js","types":"dist/index.d.ts"}"#;
        let files: &[(&str, &[u8])] = &[
            ("package/dist/index.cjs", b"exports.observe = () => {};"),
            ("package/dist/index.js", b"export const observe = () => {};"),
            (
                "package/dist/index.d.ts",
                b"export declare const observe: () => void;",
            ),
        ];

        let (path, trace) = legacy_resolution(manifest, files, ResolutionAxis::Runtime).unwrap();
        assert_eq!(path, "dist/index.js");
        assert_eq!(trace.branch, "legacy:module");
        assert_eq!(
            trace.steps,
            vec![ResolutionTraceStep {
                condition: "module".into(),
                target: "dist/index.js".into(),
            }]
        );

        // The declarations axis is untouched: `module` never names a typing.
        let (path, trace) =
            legacy_resolution(manifest, files, ResolutionAxis::Declarations).unwrap();
        assert_eq!(path, "dist/index.d.ts");
        assert_eq!(trace.branch, "legacy:types");
    }

    #[test]
    fn legacy_runtime_resolution_falls_back_to_main_when_module_is_unusable() {
        let present = br#"{"name":"fixture-package","version":"1.2.3","type":"module","main":"dist/index.js"}"#;
        let runtime: &[u8] = b"export const observe = () => {};";
        let files: &[(&str, &[u8])] = &[("package/dist/index.js", runtime)];
        let (baseline, baseline_trace) =
            legacy_resolution(present, files, ResolutionAxis::Runtime).unwrap();
        assert_eq!(baseline, "dist/index.js");
        assert_eq!(baseline_trace.branch, "legacy:main");

        // A declared `module` target that the artifact does not contain is not a
        // refusal: Node consumers never read `module`, so `main` is still real.
        for manifest in [
            br#"{"name":"fixture-package","version":"1.2.3","type":"module","main":"dist/index.js","module":"dist/absent.js"}"#.as_slice(),
            // Neither is a traversal, an escaping, or a non-string `module`.
            br#"{"name":"fixture-package","version":"1.2.3","type":"module","main":"dist/index.js","module":"../outside/index.js"}"#.as_slice(),
            br#"{"name":"fixture-package","version":"1.2.3","type":"module","main":"dist/index.js","module":true}"#.as_slice(),
            br#"{"name":"fixture-package","version":"1.2.3","type":"module","main":"dist/index.js","module":null}"#.as_slice(),
        ] {
            let (path, trace) =
                legacy_resolution(manifest, files, ResolutionAxis::Runtime).unwrap();
            assert_eq!(path, "dist/index.js");
            assert_eq!(trace.branch, "legacy:main");
            assert_eq!(
                trace.steps,
                vec![ResolutionTraceStep {
                    condition: "main".into(),
                    target: "dist/index.js".into(),
                }]
            );
        }

        // With neither field usable the index fallback is still the last resort.
        let indexed = br#"{"name":"fixture-package","version":"1.2.3","type":"module","module":"dist/absent.js"}"#;
        let (path, trace) = legacy_resolution(
            indexed,
            &[("package/index.js", runtime)],
            ResolutionAxis::Runtime,
        )
        .unwrap();
        assert_eq!(path, "index.js");
        assert_eq!(trace.branch, "legacy:index");
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
            declaration_exports: BTreeSet::new(),
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
        let candidate_document = crate::contract_document::encode(
            &candidate,
            &crate::contract_document::SidecarDigests::default(),
            false,
        )
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
        let document_plan = crate::plan_contract_document_certification(
            &candidate_document,
            request.clone(),
            resolved.clone(),
            UntrustedArtifactEnvelope::Published(archive),
        )
        .unwrap();
        assert_eq!(document_plan.snapshot_root(), snapshot.root());
        assert_eq!(document_plan.demand_graph().demands().len(), 6);

        let request_without_development = ImportRequest {
            export_conditions: vec!["import".into()],
            ..request
        };
        assert!(matches!(
            snapshot.verify_resolved_import(&request_without_development, &resolved),
            Err(ArtifactSnapshotError::ResolutionMismatch(_))
        ));
    }

    // Regression: a value-only case set carries one plan per alternative
    // artifact case of a single package, and every such plan shares one
    // `snapshot_root` (the batch identity check requires it). The
    // implementation-location resolver used to refuse the moment more than one
    // plan matched that shared root ("multiple installation identities"), which
    // sank every multi-case package (corvu, corvu-next, @solid-devtools/logger,
    // and every multi-case solid-primitives). `snapshot_root` is a content hash,
    // so all matching plans materialize byte-identical sources: the resolver
    // must bind the first materialized owner, not refuse.
    #[test]
    fn implementation_location_binds_first_owner_for_shared_snapshot_root() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3","exports":{".":{"types":"./types/index.d.ts","import":"./dist/index.js","default":"./dist/index.js"}}}"#;
        let runtime = b"export function make(callback) { callback(); return () => {}; }";
        let declarations = b"export declare function make(callback: () => void): () => void;";
        let archive = published_archive(&[
            ("package/package.json", manifest),
            ("package/dist/index.js", runtime),
            ("package/types/index.d.ts", declarations),
        ]);
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let root = "/project/node_modules/fixture-package";
        let package_manifest = resolved_file(root, "package.json", manifest);
        let runtime_file = resolved_file(root, "dist/index.js", runtime);
        let declaration_file = resolved_file(root, "types/index.d.ts", declarations);
        let closure = ClosureManifest::new(
            vec![
                closure_entry(ClosureFileRole::Manifest, "package.json", manifest),
                closure_entry(ClosureFileRole::ResolutionInput, "package.json", manifest),
                closure_entry(ClosureFileRole::Runtime, "dist/index.js", runtime),
                closure_entry(
                    ClosureFileRole::Declaration,
                    "types/index.d.ts",
                    declarations,
                ),
            ],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let request = ImportRequest {
            specifier: "fixture-package".into(),
            importer: "/project/src/app.ts".into(),
            export_conditions: vec!["import".into()],
        };
        let exports = BTreeMap::from([(
            "make".into(),
            ResolvedExportBinding {
                runtime: ResolvedExportTarget {
                    module: runtime_file.clone(),
                    export_name: "make".into(),
                },
                declarations: ResolvedExportTarget {
                    module: declaration_file.clone(),
                    export_name: "make".into(),
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
                branch: "/exports/./import".into(),
                steps: vec![
                    ResolutionTraceStep {
                        condition: "subpath".into(),
                        target: ".".into(),
                    },
                    ResolutionTraceStep {
                        condition: "import".into(),
                        target: "/exports/.".into(),
                    },
                    ResolutionTraceStep {
                        condition: "target".into(),
                        target: "./dist/index.js".into(),
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
            declaration_exports: BTreeSet::new(),
            authority: ResolutionAuthority::Host,
        };

        let (package, artifact_case) =
            crate::artifact_resolution::proposal_identity(&resolved).unwrap();
        let candidate = ContractProposal::new(package, vec![artifact_case])
            .normalize()
            .unwrap();
        let plan = plan_certification(
            CertificationRequest::new(candidate, request, resolved),
            UntrustedArtifactEnvelope::Published(archive),
        )
        .unwrap();
        // The runtime export must expose a span for the implementation-location
        // resolver to have anything to bind; otherwise this test would trivially
        // pass on the early `Ok(None)` and never reach the multiplicity path.
        assert!(
            plan.verified_exports().runtime_binding("make").is_some(),
            "the function export must carry a runtime span"
        );

        // Two references to the same plan model a value-only case set whose
        // alternative artifact cases share one snapshot_root and one package
        // root — exactly the shape that used to refuse.
        let location = super::type_facts::export_implementation_location_for_test(
            &[&plan, &plan],
            &plan,
            "make",
        )
        .expect("shared snapshot_root must bind, not refuse as multiple identities")
        .expect("the resolved function export has an implementation span");
        assert!(
            location.path.ends_with("dist/index.js"),
            "implementation location must point at the runtime module, got {}",
            location.path
        );

        // A single plan still resolves; the fix did not narrow the ordinary path.
        assert!(
            super::type_facts::export_implementation_location_for_test(&[&plan], &plan, "make")
                .unwrap()
                .is_some()
        );
    }

    // Regression: the private witness project listed only the export bindings'
    // own runtime paths as program roots. Its harness imports declaration
    // modules, and TypeScript resolves every declaration re-export specifier to
    // the sibling `.d.ts`, so a runtime chunk named only by a re-export never
    // became a program member: the producer found no source file for it and
    // returned `sourceUnavailable` for an implementation the package ships
    // (`@tanstack/devtools-ui`'s `dist/esm/styles/semantic-theme.js` reached
    // through `dist/esm/internal.js`). The plan's independently replayed module
    // closure already names those modules; the tsconfig must list them.
    #[test]
    fn private_project_lists_reexported_runtime_closure_modules_as_program_roots() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3","exports":{".":{"types":"./dist/index.d.ts","import":"./dist/index.js","default":"./dist/index.js"}}}"#;
        let runtime = b"export { make } from \"./create/make.js\";\n";
        let implementation = b"import { helper } from \"./helper.js\";\nexport function make(callback) { helper(callback); return () => {}; }\n";
        let helper = b"export function helper(callback) { callback(); }\n";
        let declarations = b"export { make } from \"./create/make.js\";\n";
        let implementation_declarations =
            b"export declare function make(callback: () => void): () => void;";
        let archive = published_archive(&[
            ("package/package.json", manifest),
            ("package/dist/index.js", runtime),
            ("package/dist/create/make.js", implementation),
            ("package/dist/create/helper.js", helper),
            ("package/dist/index.d.ts", declarations),
            ("package/dist/create/make.d.ts", implementation_declarations),
        ]);
        let root = "/project/node_modules/fixture-package";
        let request = ImportRequest {
            specifier: "fixture-package".into(),
            importer: "/project/src/app.ts".into(),
            export_conditions: vec!["import".into()],
        };
        let resolved = ResolvedImport {
            specifier: request.specifier.clone(),
            importer: request.importer.clone(),
            requested_entrypoint: ".".into(),
            package_name: "fixture-package".into(),
            package_version: "1.2.3".into(),
            package_integrity: ArtifactSnapshot::from_published(
                &archive,
                SnapshotLimits::policy_2(),
            )
            .unwrap()
            .package_integrity()
            .into(),
            package_root: root.into(),
            package_real_root: None,
            package_manifest: resolved_file(root, "package.json", manifest),
            runtime: resolved_file(root, "dist/index.js", runtime),
            declarations: resolved_file(root, "dist/index.d.ts", declarations),
            runtime_trace: ResolutionTrace {
                branch: "/exports/./import".into(),
                steps: vec![
                    ResolutionTraceStep {
                        condition: "subpath".into(),
                        target: ".".into(),
                    },
                    ResolutionTraceStep {
                        condition: "import".into(),
                        target: "/exports/.".into(),
                    },
                    ResolutionTraceStep {
                        condition: "target".into(),
                        target: "./dist/index.js".into(),
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
                        target: "./dist/index.d.ts".into(),
                    },
                ],
            },
            // The implementation module is not the entrypoint: only a re-export
            // in `dist/index.js` names it.
            closure: ClosureManifest::new(
                vec![
                    closure_entry(ClosureFileRole::Manifest, "package.json", manifest),
                    closure_entry(ClosureFileRole::ResolutionInput, "package.json", manifest),
                    closure_entry(ClosureFileRole::Runtime, "dist/index.js", runtime),
                    closure_entry(
                        ClosureFileRole::Runtime,
                        "dist/create/make.js",
                        implementation,
                    ),
                    closure_entry(ClosureFileRole::Runtime, "dist/create/helper.js", helper),
                    closure_entry(
                        ClosureFileRole::Declaration,
                        "dist/index.d.ts",
                        declarations,
                    ),
                    closure_entry(
                        ClosureFileRole::Declaration,
                        "dist/create/make.d.ts",
                        implementation_declarations,
                    ),
                ],
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
            transform: None,
            exports: BTreeMap::from([(
                "make".into(),
                ResolvedExportBinding {
                    runtime: ResolvedExportTarget {
                        module: resolved_file(root, "dist/create/make.js", implementation),
                        export_name: "make".into(),
                    },
                    declarations: ResolvedExportTarget {
                        module: resolved_file(
                            root,
                            "dist/create/make.d.ts",
                            implementation_declarations,
                        ),
                        export_name: "make".into(),
                    },
                },
            )]),
            declaration_exports: BTreeSet::new(),
            authority: ResolutionAuthority::Host,
        };

        let (package, artifact_case) =
            crate::artifact_resolution::proposal_identity(&resolved).unwrap();
        let candidate = ContractProposal::new(package, vec![artifact_case])
            .normalize()
            .unwrap();
        let plan = plan_certification(
            CertificationRequest::new(candidate, request, resolved),
            UntrustedArtifactEnvelope::Published(archive),
        )
        .unwrap();

        let files =
            super::type_facts::private_project_program_files_for_test(&[&plan], &plan).unwrap();
        assert!(
            files.contains(&"dist/create/make.js".to_owned()),
            "the re-exported implementation module must be a program root, got {files:?}"
        );
        assert!(
            files.contains(&"dist/index.js".to_owned()),
            "the entrypoint runtime module must remain a program root, got {files:?}"
        );
        // No export binding names this one at all: only the replayed runtime
        // closure reaches it, and an implementation that calls into it needs it
        // in the program.
        assert!(
            files.contains(&"dist/create/helper.js".to_owned()),
            "a runtime module reached only through the closure must be a program root, got {files:?}"
        );
        // Declarations stay out: TypeScript resolves them itself, and the
        // declaration source census already pins them.
        assert!(
            !files.iter().any(|path| path.ends_with(".d.ts")),
            "declaration modules must not be added as program roots, got {files:?}"
        );
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
        let chunk = b"import './chunk-leaf.js'; export const chunk = true;";
        let chunk_leaf = b"export const leaf = true;";
        let asset = br#"{"value":true}"#;
        let declarations = b"export * from './surface.js';";
        let surface = b"export declare const shared: boolean;";
        let archive = published_archive(&[
            ("package/package.json", manifest),
            ("package/dist/index.js", runtime),
            ("package/dist/shared.js", shared),
            ("package/dist/shared-copy.js", shared),
            ("package/dist/chunk.js", chunk),
            ("package/dist/chunk-leaf.js", chunk_leaf),
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
            closure_entry(
                ClosureFileRole::LiteralDynamicChunk,
                "dist/chunk-leaf.js",
                chunk_leaf,
            ),
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
        let mismatch =
            super::module_closure::verify_snapshot_closure(&snapshot, &resolution, &missing_edge)
                .unwrap_err();
        assert!(matches!(mismatch, ArtifactSnapshotError::ModuleClosure(_)));
        assert!(mismatch.to_string().contains("diff={\"replayedOnly\""));

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
    fn module_closure_resolves_extensionless_and_multi_dot_source_modules_on_both_axes() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3"}"#;
        let entry = b"export { first } from './array'; export { second } from './HeadContent.dev';";
        let array = b"export const first = true;";
        let multi_dot = b"export const second = true;";
        let archive = published_archive(&[
            ("package/package.json", manifest),
            ("package/src/index.ts", entry),
            ("package/src/array.ts", array),
            ("package/src/HeadContent.dev.tsx", multi_dot),
        ]);
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let resolution = SnapshotVerifiedResolution {
            snapshot_root: snapshot.root().into(),
            provenance_root: snapshot.provenance_root().into(),
            runtime_path: "src/index.ts".into(),
            declarations_path: "src/index.ts".into(),
            evidence_root: format!("sha256:{:064x}", 0),
        };

        let replayed =
            super::module_closure::replay_snapshot_closure(&snapshot, &resolution, &[]).unwrap();
        for role in [ClosureFileRole::Runtime, ClosureFileRole::Declaration] {
            assert!(
                replayed
                    .entries
                    .contains(&closure_entry(role, "src/array.ts", array))
            );
            assert!(replayed.entries.contains(&closure_entry(
                role,
                "src/HeadContent.dev.tsx",
                multi_dot,
            )));
        }
    }

    #[test]
    fn module_closure_holds_query_suffixed_asset_imports_opaque_without_refusing() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3"}"#;
        // The generator's `bundlerResourceSuffix` classifies exactly these five
        // as bundler-mediated, and `./shipped.js` as an ordinary module edge.
        // This replay must agree specifier for specifier or every such closure
        // diverges from the supplied one.
        // `br##` because the source contains `"#`, which would close `br#`.
        let entry = br##"
            import source from "./shipped.js?raw";
            import { run } from "./shipped.js";
            import "./absent.js?url";
            import "./shipped.js#fragment";
            import "#platform?raw";
            import "external-pkg/theme.css?inline";
            export const thing = [source, run];
        "##;
        let shipped = b"export const run = callback => callback();";
        let archive = published_archive(&[
            ("package/package.json", manifest),
            ("package/dist/index.js", entry),
            ("package/dist/shipped.js", shipped),
        ]);
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let resolution = SnapshotVerifiedResolution {
            snapshot_root: snapshot.root().into(),
            provenance_root: snapshot.provenance_root().into(),
            runtime_path: "dist/index.js".into(),
            declarations_path: "dist/index.js".into(),
            evidence_root: format!("sha256:{:064x}", 0),
        };

        let replayed =
            super::module_closure::replay_snapshot_closure(&snapshot, &resolution, &[]).unwrap();
        // The module import still reaches the shipped file; no suffixed
        // specifier contributes an entry, and none refuses the replay.
        for role in [ClosureFileRole::Runtime, ClosureFileRole::Declaration] {
            assert!(
                replayed
                    .entries
                    .contains(&closure_entry(role, "dist/shipped.js", shipped))
            );
        }
        let mut opaque = replayed
            .hazards
            .iter()
            .filter(|hazard| hazard.kind == ClosureHazardKind::UnacceptedExternalDependency)
            .map(|hazard| hazard.source.clone())
            .collect::<Vec<_>>();
        opaque.sort();
        opaque.dedup();
        assert_eq!(
            opaque,
            [
                "./dist/index.js:#platform?raw",
                "./dist/index.js:./absent.js?url",
                "./dist/index.js:./shipped.js#fragment",
                "./dist/index.js:./shipped.js?raw",
                "./dist/index.js:external-pkg/theme.css?inline",
            ]
        );
        assert!(replayed.dependencies.is_empty());

        // An unsuffixed relative specifier with no file still refuses.
        let missing = published_archive(&[
            ("package/package.json", manifest),
            ("package/dist/index.js", b"import './absent.js';"),
        ]);
        let missing_snapshot =
            ArtifactSnapshot::from_published(&missing, SnapshotLimits::policy_2()).unwrap();
        let missing_resolution = SnapshotVerifiedResolution {
            snapshot_root: missing_snapshot.root().into(),
            provenance_root: missing_snapshot.provenance_root().into(),
            runtime_path: "dist/index.js".into(),
            declarations_path: "dist/index.js".into(),
            evidence_root: format!("sha256:{:064x}", 0),
        };
        let refusal = super::module_closure::replay_snapshot_closure(
            &missing_snapshot,
            &missing_resolution,
            &[],
        )
        .unwrap_err();
        assert!(refusal.to_string().contains("was not found"));
    }

    #[test]
    fn module_closure_maps_explicit_source_suffix_to_declaration_file() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3"}"#;
        let runtime = b"export const value = true;";
        let declarations = b"export { value } from './main.ts';";
        let declaration_leaf = b"export declare const value: true;";
        let archive = published_archive(&[
            ("package/package.json", manifest),
            ("package/dist/index.js", runtime),
            ("package/dist/index.d.ts", declarations),
            ("package/dist/main.d.ts", declaration_leaf),
        ]);
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let resolution = SnapshotVerifiedResolution {
            snapshot_root: snapshot.root().into(),
            provenance_root: snapshot.provenance_root().into(),
            runtime_path: "dist/index.js".into(),
            declarations_path: "dist/index.d.ts".into(),
            evidence_root: format!("sha256:{:064x}", 0),
        };

        let replayed =
            super::module_closure::replay_snapshot_closure(&snapshot, &resolution, &[]).unwrap();
        assert!(replayed.entries.contains(&closure_entry(
            ClosureFileRole::Declaration,
            "dist/main.d.ts",
            declaration_leaf,
        )));
    }

    #[test]
    fn module_closure_resolves_only_unshadowed_literal_require_edges() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3"}"#;
        let runtime = br#"
            require("./loaded");
            require(dynamicName);
            function local(require) { require("./shadowed-parameter"); }
            { const require = value => value; require("./shadowed-block"); }
            export const value = true;
        "#;
        let loaded = b"export const loaded = true;";
        let declarations = b"export declare const value: true;";
        let archive = published_archive(&[
            ("package/package.json", manifest),
            ("package/dist/index.js", runtime),
            ("package/dist/loaded.js", loaded),
            (
                "package/dist/shadowed-parameter.js",
                b"throw new Error('must not load')",
            ),
            (
                "package/dist/shadowed-block.js",
                b"throw new Error('must not load')",
            ),
            ("package/dist/index.d.ts", declarations),
        ]);
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let resolution = SnapshotVerifiedResolution {
            snapshot_root: snapshot.root().into(),
            provenance_root: snapshot.provenance_root().into(),
            runtime_path: "dist/index.js".into(),
            declarations_path: "dist/index.d.ts".into(),
            evidence_root: format!("sha256:{:064x}", 0),
        };

        let replayed =
            super::module_closure::replay_snapshot_closure(&snapshot, &resolution, &[]).unwrap();
        assert!(replayed.entries.contains(&closure_entry(
            ClosureFileRole::Runtime,
            "dist/loaded.js",
            loaded,
        )));
        assert!(!replayed.entries.iter().any(|entry| {
            entry.path.contains("shadowed-parameter") || entry.path.contains("shadowed-block")
        }));
        assert_eq!(
            replayed
                .hazards
                .iter()
                .filter(|hazard| hazard.kind == ClosureHazardKind::NonliteralDynamicLoading)
                .count(),
            1
        );
    }

    #[test]
    fn export_bindings_are_replayed_across_reexports_stars_and_namespaces() {
        let manifest = br#"{"name":"fixture-package","version":"1.2.3"}"#;
        let runtime = br#"
            export { runtimeName as publicName } from "./runtime.js";
            export * from "./star.js";
            export * as ns from "./namespace.js";
            export const Config = {};
        "#;
        let runtime_target = b"export const runtimeName = 1;";
        let runtime_star = b"export const shared = 1;";
        let runtime_namespace = b"export const inner = 1;";
        let declarations = br#"
            export { declarationName as publicName } from "./surface.js";
            export * from "./star.js";
            export * as ns from "./namespace.js";
            export declare namespace Config { const inner: number; }
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
            declaration_exports: BTreeSet::from([
                "Config".into(),
                "ns".into(),
                "publicName".into(),
                "shared".into(),
            ]),
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

        let mut omitted_declaration = resolved.clone();
        omitted_declaration.declaration_exports.remove("shared");
        assert!(matches!(
            super::export_bindings::verify_snapshot_exports(
                &snapshot,
                &resolution,
                &omitted_declaration
            ),
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
            declaration_exports: BTreeSet::new(),
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

    fn synthetic_graph_certification_request(
        name: &str,
        version: &str,
        package_root: &str,
        importer: &str,
        runtime: &[u8],
        declarations: &[u8],
        dependencies: Vec<AcceptedDependencyEdge>,
    ) -> (CertificationRequest, PublishedArchive, String) {
        let manifest = format!(
            r#"{{"name":"{name}","version":"{version}","exports":{{".":{{"types":"./types/index.d.ts","import":"./dist/index.js"}}}}}}"#
        );
        let archive = published_archive_for(
            name,
            version,
            &[
                ("package/package.json", manifest.as_bytes()),
                ("package/dist/index.js", runtime),
                ("package/types/index.d.ts", declarations),
            ],
        );
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let package_manifest = resolved_file(package_root, "package.json", manifest.as_bytes());
        let runtime_file = resolved_file(package_root, "dist/index.js", runtime);
        let declaration_file = resolved_file(package_root, "types/index.d.ts", declarations);
        let closure = ClosureManifest::new(
            vec![
                closure_entry(
                    ClosureFileRole::Manifest,
                    "package.json",
                    manifest.as_bytes(),
                ),
                closure_entry(
                    ClosureFileRole::ResolutionInput,
                    "package.json",
                    manifest.as_bytes(),
                ),
                closure_entry(ClosureFileRole::Runtime, "dist/index.js", runtime),
                closure_entry(
                    ClosureFileRole::Declaration,
                    "types/index.d.ts",
                    declarations,
                ),
            ],
            dependencies,
            Vec::new(),
        )
        .unwrap();
        let request = ImportRequest {
            specifier: name.into(),
            importer: importer.into(),
            export_conditions: vec!["import".into()],
        };
        let exports = BTreeMap::from([(
            "value".into(),
            ResolvedExportBinding {
                runtime: ResolvedExportTarget {
                    module: runtime_file.clone(),
                    export_name: "value".into(),
                },
                declarations: ResolvedExportTarget {
                    module: declaration_file.clone(),
                    export_name: "value".into(),
                },
            },
        )]);
        let resolved = ResolvedImport {
            specifier: name.into(),
            importer: importer.into(),
            requested_entrypoint: ".".into(),
            package_name: name.into(),
            package_version: version.into(),
            package_integrity: snapshot.package_integrity().into(),
            package_root: package_root.into(),
            package_real_root: None,
            package_manifest,
            runtime: runtime_file,
            declarations: declaration_file,
            runtime_trace: ResolutionTrace {
                branch: "/exports/./import".into(),
                steps: vec![
                    ResolutionTraceStep {
                        condition: "subpath".into(),
                        target: ".".into(),
                    },
                    ResolutionTraceStep {
                        condition: "import".into(),
                        target: "/exports/.".into(),
                    },
                    ResolutionTraceStep {
                        condition: "target".into(),
                        target: "./dist/index.js".into(),
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
            declaration_exports: BTreeSet::new(),
            authority: ResolutionAuthority::Host,
        };
        let (package, mut artifact_case) =
            crate::artifact_resolution::proposal_identity(&resolved).unwrap();
        artifact_case.exports.insert(
            "value".into(),
            ExportSemantics {
                identity: ExportIdentity {
                    entrypoint: artifact_case.entrypoint.clone(),
                    public_name: "value".into(),
                    runtime: ExportTargetIdentity {
                        module: artifact_case.runtime.clone(),
                        export_name: "value".into(),
                    },
                    declarations: ExportTargetIdentity {
                        module: artifact_case.declarations.clone(),
                        export_name: "value".into(),
                    },
                },
                shape: ValueShape::Plain,
                stability: StabilityKnowledge::Unknown,
                call: CallSemantics::new(
                    CallClaims::default(),
                    vec![],
                    vec![],
                    vec![],
                    GuardPartition {
                        cases: KnowledgeSet::Unknown,
                    },
                ),
            },
        );
        let candidate = ContractProposal::new(package, vec![artifact_case])
            .normalize()
            .unwrap();
        (
            CertificationRequest::new(candidate, request, resolved),
            archive,
            snapshot.package_integrity().into(),
        )
    }

    /// Builds a published root package whose *entry* module imports only a
    /// local sibling, and whose non-entry sibling (`dist/re-export.js` /
    /// `types/re-export.d.ts`) is the module that re-exports the external
    /// dependency. Node resolves an external import from the importing module,
    /// so this leaf's importer is the non-entry sibling, not the package entry.
    /// Every artifact identity is still replayed from the archive bytes.
    fn synthetic_reexport_module_root_request(
        name: &str,
        version: &str,
        package_root: &str,
        importer: &str,
        dependency_specifier: &str,
        dependencies: Vec<AcceptedDependencyEdge>,
    ) -> (CertificationRequest, PublishedArchive, String) {
        let manifest = format!(
            r#"{{"name":"{name}","version":"{version}","exports":{{".":{{"types":"./types/index.d.ts","import":"./dist/index.js"}}}}}}"#
        );
        let entry_runtime = b"export * from './re-export.js';".to_vec();
        let entry_declarations = b"export * from './re-export.js';".to_vec();
        let reexport_runtime = format!("export * from '{dependency_specifier}';").into_bytes();
        let reexport_declarations = format!("export * from '{dependency_specifier}';").into_bytes();
        let archive = published_archive_for(
            name,
            version,
            &[
                ("package/package.json", manifest.as_bytes()),
                ("package/dist/index.js", &entry_runtime),
                ("package/dist/re-export.js", &reexport_runtime),
                ("package/types/index.d.ts", &entry_declarations),
                ("package/types/re-export.d.ts", &reexport_declarations),
            ],
        );
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let package_manifest = resolved_file(package_root, "package.json", manifest.as_bytes());
        let runtime_file = resolved_file(package_root, "dist/index.js", &entry_runtime);
        let declaration_file = resolved_file(package_root, "types/index.d.ts", &entry_declarations);
        let closure = ClosureManifest::new(
            vec![
                closure_entry(
                    ClosureFileRole::Manifest,
                    "package.json",
                    manifest.as_bytes(),
                ),
                closure_entry(
                    ClosureFileRole::ResolutionInput,
                    "package.json",
                    manifest.as_bytes(),
                ),
                closure_entry(ClosureFileRole::Runtime, "dist/index.js", &entry_runtime),
                closure_entry(
                    ClosureFileRole::Runtime,
                    "dist/re-export.js",
                    &reexport_runtime,
                ),
                closure_entry(
                    ClosureFileRole::Declaration,
                    "types/index.d.ts",
                    &entry_declarations,
                ),
                closure_entry(
                    ClosureFileRole::Declaration,
                    "types/re-export.d.ts",
                    &reexport_declarations,
                ),
            ],
            dependencies,
            Vec::new(),
        )
        .unwrap();
        let request = ImportRequest {
            specifier: name.into(),
            importer: importer.into(),
            export_conditions: vec!["import".into()],
        };
        let resolved = ResolvedImport {
            specifier: name.into(),
            importer: importer.into(),
            requested_entrypoint: ".".into(),
            package_name: name.into(),
            package_version: version.into(),
            package_integrity: snapshot.package_integrity().into(),
            package_root: package_root.into(),
            package_real_root: None,
            package_manifest,
            runtime: runtime_file,
            declarations: declaration_file,
            runtime_trace: ResolutionTrace {
                branch: "/exports/./import".into(),
                steps: vec![
                    ResolutionTraceStep {
                        condition: "subpath".into(),
                        target: ".".into(),
                    },
                    ResolutionTraceStep {
                        condition: "import".into(),
                        target: "/exports/.".into(),
                    },
                    ResolutionTraceStep {
                        condition: "target".into(),
                        target: "./dist/index.js".into(),
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
            exports: BTreeMap::new(),
            declaration_exports: BTreeSet::new(),
            authority: ResolutionAuthority::Host,
        };
        let (package, mut artifact_case) =
            crate::artifact_resolution::proposal_identity(&resolved).unwrap();
        artifact_case.exports.insert(
            "value".into(),
            ExportSemantics {
                identity: ExportIdentity {
                    entrypoint: artifact_case.entrypoint.clone(),
                    public_name: "value".into(),
                    runtime: ExportTargetIdentity {
                        module: artifact_case.runtime.clone(),
                        export_name: "value".into(),
                    },
                    declarations: ExportTargetIdentity {
                        module: artifact_case.declarations.clone(),
                        export_name: "value".into(),
                    },
                },
                shape: ValueShape::Plain,
                stability: StabilityKnowledge::Unknown,
                call: CallSemantics::new(
                    CallClaims::default(),
                    vec![],
                    vec![],
                    vec![],
                    GuardPartition {
                        cases: KnowledgeSet::Unknown,
                    },
                ),
            },
        );
        let candidate = ContractProposal::new(package, vec![artifact_case])
            .normalize()
            .unwrap();
        (
            CertificationRequest::new(candidate, request, resolved),
            archive,
            snapshot.package_integrity().into(),
        )
    }

    /// A dependency edge whose external re-export is issued from a *non-entry*
    /// module of the parent package must still resolve to its exact graph node.
    /// Node resolution is per-importing-module, so the child's importer is the
    /// re-exporting sibling, not the parent entry; the authoritative matcher
    /// admits it because that sibling is a runtime/declaration module of the
    /// parent's replayed, digest-pinned closure. The transplanted-leaf case in
    /// `native_published_graph_is_dependency_first_and_rejects_missing_or_transplanted_nodes`
    /// remains rejected: an importer outside the parent package root is not one
    /// of those closure modules.
    #[test]
    fn native_published_graph_matches_a_non_entry_module_reexport() {
        let reexport_importer = "/project/node_modules/root-package/dist/re-export.js";
        let (leaf_request, leaf_archive, leaf_integrity) = synthetic_graph_certification_request(
            "leaf-package",
            "2.0.0",
            "/project/node_modules/root-package/node_modules/leaf-package",
            reexport_importer,
            b"export const value = 1;",
            b"export declare const value: number;",
            Vec::new(),
        );
        let leaf_plan = plan_certification(
            leaf_request.clone(),
            UntrustedArtifactEnvelope::Published(leaf_archive.clone()),
        )
        .unwrap();
        let edge = AcceptedDependencyEdge {
            specifier: "leaf-package".into(),
            package_name: "leaf-package".into(),
            artifact_case: leaf_plan.selected_artifact_case_id().into(),
            accepted_contract_digest: leaf_plan
                .demand_graph()
                .candidate_semantic_digest()
                .as_str()
                .into(),
        };
        let (mut root_request, root_archive, root_integrity) =
            synthetic_reexport_module_root_request(
                "root-package",
                "1.0.0",
                "/project/node_modules/root-package",
                "/project/src/app.ts",
                "leaf-package",
                vec![edge],
            );
        root_request.resolved_import.exports.insert(
            "value".into(),
            leaf_request.resolved_import.exports["value"].clone(),
        );

        let graph = plan_published_contract_graph(
            PublishedGraphNodeRequest::new(
                root_request,
                root_archive,
                graph_lock("root-package", "1.0.0", &root_integrity),
            ),
            [PublishedGraphNodeRequest::new(
                leaf_request,
                leaf_archive,
                graph_lock("leaf-package", "2.0.0", &leaf_integrity),
            )],
        )
        .unwrap();
        let order = graph.dependency_first_identities();
        assert_eq!(order.len(), 2);
        assert_eq!(order[0].package_name, "leaf-package");
        assert_eq!(order[1].package_name, "root-package");
        assert_eq!(graph.root_identity().package_name, "root-package");
    }

    #[test]
    fn planning_transaction_shares_snapshot_members_but_rebuilds_request_authority() {
        let (request, archive, _) = synthetic_graph_certification_request(
            "fixture-package",
            "1.2.3",
            "/project/node_modules/fixture-package",
            "/project/src/app.ts",
            b"export const value = 1;",
            b"export declare const value: number;",
            Vec::new(),
        );
        let mut transaction = CertificationPlanningTransaction::new();

        let first = transaction
            .plan_certification(
                request.clone(),
                UntrustedArtifactEnvelope::Published(archive.clone()),
            )
            .unwrap();
        let second = transaction
            .plan_certification(request, UntrustedArtifactEnvelope::Published(archive))
            .unwrap();

        assert_eq!(first.snapshot_root(), second.snapshot_root());
        assert_eq!(
            first.snapshot.provenance_root(),
            second.snapshot.provenance_root()
        );
        assert!(Arc::ptr_eq(&first.snapshot.files, &second.snapshot.files));
        assert!(Arc::ptr_eq(
            &first.snapshot.directories,
            &second.snapshot.directories
        ));
        assert!(Arc::ptr_eq(
            first.snapshot.files.get("dist/index.js").unwrap(),
            second.snapshot.files.get("dist/index.js").unwrap(),
        ));
        assert!(!std::ptr::eq(
            first.verified_closure.manifest(),
            second.verified_closure.manifest(),
        ));
        assert!(!std::ptr::eq(first.demand_graph(), second.demand_graph()));
        assert_eq!(transaction.published_snapshots.len(), 1);
    }

    fn graph_lock(name: &str, version: &str, integrity: &str) -> PublishedGraphLockSelection {
        let lock = format!(
            r#"{{"packages":{{"{name}@{version}":["{name}@{version}","",{{}},"{integrity}"],}},}}"#
        );
        PublishedGraphLockSelection::from_bun_lock(
            lock.as_bytes(),
            format!("{name}@{version}"),
            name,
            version,
        )
        .unwrap()
    }

    fn two_node_published_graph(
        transplanted_leaf: bool,
        substituted_leaf_lock: bool,
        forged_dependency_digest: bool,
    ) -> (PublishedGraphNodeRequest, PublishedGraphNodeRequest) {
        two_node_published_graph_with_root_callbacks(
            transplanted_leaf,
            substituted_leaf_lock,
            forged_dependency_digest,
            false,
            false,
        )
    }

    fn close_candidate_callbacks(request: &mut CertificationRequest) {
        let mut artifact_cases = request.candidate.artifact_cases().to_vec();
        artifact_cases[0].exports.get_mut("value").unwrap().call = CallSemantics::new(
            CallClaims {
                callbacks: KnowledgeSet::complete(vec![]),
                ..CallClaims::default()
            },
            vec![],
            vec![],
            vec![],
            GuardPartition::default(),
        );
        request.candidate =
            ContractProposal::new(request.candidate.package().clone(), artifact_cases)
                .normalize()
                .unwrap();
    }

    fn two_node_published_graph_with_root_callbacks(
        transplanted_leaf: bool,
        substituted_leaf_lock: bool,
        forged_dependency_digest: bool,
        close_root_callbacks: bool,
        close_leaf_callbacks: bool,
    ) -> (PublishedGraphNodeRequest, PublishedGraphNodeRequest) {
        let root_runtime_path = "/project/node_modules/root-package/dist/index.js";
        let leaf_importer = if transplanted_leaf {
            "/other/node_modules/root-package/dist/index.js"
        } else {
            root_runtime_path
        };
        let (mut leaf_request, leaf_archive, leaf_integrity) =
            synthetic_graph_certification_request(
                "leaf-package",
                "2.0.0",
                "/project/node_modules/root-package/node_modules/leaf-package",
                leaf_importer,
                b"export const value = 1;",
                b"export declare const value: number;",
                Vec::new(),
            );
        if close_leaf_callbacks {
            close_candidate_callbacks(&mut leaf_request);
        }
        let leaf_plan = plan_certification(
            leaf_request.clone(),
            UntrustedArtifactEnvelope::Published(leaf_archive.clone()),
        )
        .unwrap();
        let edge = AcceptedDependencyEdge {
            specifier: "leaf-package".into(),
            package_name: "leaf-package".into(),
            artifact_case: leaf_plan.selected_artifact_case_id().into(),
            accepted_contract_digest: if forged_dependency_digest {
                format!("sha256:{:064x}", 99)
            } else {
                leaf_plan
                    .demand_graph()
                    .candidate_semantic_digest()
                    .as_str()
                    .into()
            },
        };
        let (mut root_request, root_archive, root_integrity) =
            synthetic_graph_certification_request(
            "root-package",
            "1.0.0",
            "/project/node_modules/root-package",
            "/project/src/app.ts",
            b"import { value as leafValue } from 'leaf-package'; export const value = leafValue;",
            b"import { value as leafValue } from 'leaf-package'; export declare const value: typeof leafValue;",
            vec![edge],
        );
        if close_root_callbacks {
            close_candidate_callbacks(&mut root_request);
        }
        (
            PublishedGraphNodeRequest::new(
                root_request,
                root_archive,
                graph_lock("root-package", "1.0.0", &root_integrity),
            ),
            PublishedGraphNodeRequest::new(
                leaf_request,
                leaf_archive,
                graph_lock(
                    "leaf-package",
                    "2.0.0",
                    if substituted_leaf_lock {
                        "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="
                    } else {
                        &leaf_integrity
                    },
                ),
            ),
        )
    }

    #[test]
    fn graph_planning_transaction_reuses_exact_node_snapshots_with_equal_roots() {
        let mut transaction = CertificationPlanningTransaction::new();
        let (root, leaf) = two_node_published_graph(false, false, false);
        let first = transaction
            .plan_published_contract_graph(root, [leaf])
            .unwrap();
        assert_eq!(transaction.published_snapshots.len(), 2);

        let (root, leaf) = two_node_published_graph(false, false, false);
        let second = transaction
            .plan_published_contract_graph(root, [leaf])
            .unwrap();
        assert_eq!(first.graph_root(), second.graph_root());
        assert_eq!(first.root_identity(), second.root_identity());
        assert_eq!(transaction.published_snapshots.len(), 2);

        let (root, substituted_leaf) = two_node_published_graph(false, true, false);
        assert!(matches!(
            transaction.plan_published_contract_graph(root, [substituted_leaf]),
            Err(PublishedGraphPlanningError::LockDisagreement {
                field: "integrity",
                ..
            })
        ));
        assert_eq!(transaction.published_snapshots.len(), 2);
    }

    #[test]
    fn native_published_graph_is_dependency_first_and_rejects_missing_or_transplanted_nodes() {
        let (root, leaf) = two_node_published_graph(false, false, false);
        let graph = plan_published_contract_graph(root, [leaf]).unwrap();
        let order = graph.dependency_first_identities();
        assert_eq!(order.len(), 2);
        assert_eq!(order[0].package_name, "leaf-package");
        assert_eq!(order[1].package_name, "root-package");
        assert_eq!(graph.root_identity(), order[1]);
        assert!(graph.graph_root().starts_with("sha256:"));

        let (root, _) = two_node_published_graph(false, false, false);
        assert!(matches!(
            plan_published_contract_graph(root, []),
            Err(PublishedGraphPlanningError::MissingDependency { .. })
        ));

        let (root, transplanted_leaf) = two_node_published_graph(true, false, false);
        assert!(matches!(
            plan_published_contract_graph(root, [transplanted_leaf]),
            Err(PublishedGraphPlanningError::MissingDependency { .. })
        ));

        let (root, substituted_leaf) = two_node_published_graph(false, true, false);
        assert!(matches!(
            plan_published_contract_graph(root, [substituted_leaf]),
            Err(PublishedGraphPlanningError::LockDisagreement {
                field: "integrity",
                ..
            })
        ));

        let (root, leaf) = two_node_published_graph(false, false, true);
        assert!(matches!(
            plan_published_contract_graph(root, [leaf]),
            Err(
                PublishedGraphPlanningError::DependencyIdentityDisagreement {
                    field: "semantic digest",
                    ..
                }
            )
        ));
    }

    #[test]
    fn native_published_graph_authenticates_an_external_export_all_target() {
        let root_runtime_path = "/project/node_modules/root-package/dist/index.js";
        let (leaf_request, leaf_archive, leaf_integrity) = synthetic_graph_certification_request(
            "leaf-package",
            "2.0.0",
            "/project/node_modules/root-package/node_modules/leaf-package",
            root_runtime_path,
            b"export const value = 1;",
            b"export declare const value: number;",
            Vec::new(),
        );
        let leaf_plan = plan_certification(
            leaf_request.clone(),
            UntrustedArtifactEnvelope::Published(leaf_archive.clone()),
        )
        .unwrap();
        let edge = AcceptedDependencyEdge {
            specifier: "leaf-package".into(),
            package_name: "leaf-package".into(),
            artifact_case: leaf_plan.selected_artifact_case_id().into(),
            accepted_contract_digest: leaf_plan
                .demand_graph()
                .candidate_semantic_digest()
                .as_str()
                .into(),
        };
        let (mut root_request, root_archive, root_integrity) =
            synthetic_graph_certification_request(
                "root-package",
                "1.0.0",
                "/project/node_modules/root-package",
                "/project/src/app.ts",
                b"export * from 'leaf-package';",
                b"export * from 'leaf-package';",
                vec![edge],
            );
        root_request.resolved_import.exports.insert(
            "value".into(),
            leaf_request.resolved_import.exports["value"].clone(),
        );

        assert!(
            plan_certification(
                root_request.clone(),
                UntrustedArtifactEnvelope::Published(root_archive.clone()),
            )
            .is_err()
        );
        let graph = plan_published_contract_graph(
            PublishedGraphNodeRequest::new(
                root_request,
                root_archive,
                graph_lock("root-package", "1.0.0", &root_integrity),
            ),
            [PublishedGraphNodeRequest::new(
                leaf_request,
                leaf_archive,
                graph_lock("leaf-package", "2.0.0", &leaf_integrity),
            )],
        )
        .unwrap();
        assert_eq!(graph.dependency_first_identities().len(), 2);
        assert_eq!(graph.root_identity().package_name, "root-package");
    }

    #[test]
    fn native_published_graph_authenticates_a_transitive_external_export_target() {
        let root_runtime = "/project/node_modules/root-package/dist/index.js";
        let middle_runtime =
            "/project/node_modules/root-package/node_modules/middle-package/dist/index.js";
        let (leaf_request, leaf_archive, leaf_integrity) = synthetic_graph_certification_request(
            "leaf-package",
            "3.0.0",
            "/project/node_modules/root-package/node_modules/middle-package/node_modules/leaf-package",
            middle_runtime,
            b"export const value = 1;",
            b"export declare const value: number;",
            Vec::new(),
        );
        let leaf_plan = plan_certification(
            leaf_request.clone(),
            UntrustedArtifactEnvelope::Published(leaf_archive.clone()),
        )
        .unwrap();
        let leaf_edge = AcceptedDependencyEdge {
            specifier: "leaf-package".into(),
            package_name: "leaf-package".into(),
            artifact_case: leaf_plan.selected_artifact_case_id().into(),
            accepted_contract_digest: leaf_plan
                .demand_graph()
                .candidate_semantic_digest()
                .as_str()
                .into(),
        };
        let (mut middle_request, middle_archive, middle_integrity) =
            synthetic_graph_certification_request(
                "middle-package",
                "2.0.0",
                "/project/node_modules/root-package/node_modules/middle-package",
                root_runtime,
                b"export * from 'leaf-package';",
                b"export * from 'leaf-package';",
                vec![leaf_edge],
            );
        middle_request.resolved_import.exports.insert(
            "value".into(),
            leaf_request.resolved_import.exports["value"].clone(),
        );
        let middle_graph = plan_published_contract_graph(
            PublishedGraphNodeRequest::new(
                middle_request.clone(),
                middle_archive.clone(),
                graph_lock("middle-package", "2.0.0", &middle_integrity),
            ),
            [PublishedGraphNodeRequest::new(
                leaf_request.clone(),
                leaf_archive.clone(),
                graph_lock("leaf-package", "3.0.0", &leaf_integrity),
            )],
        )
        .unwrap();
        let middle_plan = middle_graph
            .dependency_first_identities()
            .into_iter()
            .find(|identity| identity.package_name == "middle-package")
            .and_then(|identity| middle_graph.plan(identity))
            .unwrap();
        let middle_edge = AcceptedDependencyEdge {
            specifier: "middle-package".into(),
            package_name: "middle-package".into(),
            artifact_case: middle_plan.selected_artifact_case_id().into(),
            accepted_contract_digest: middle_plan
                .demand_graph()
                .candidate_semantic_digest()
                .as_str()
                .into(),
        };
        let (mut root_request, root_archive, root_integrity) =
            synthetic_graph_certification_request(
                "root-package",
                "1.0.0",
                "/project/node_modules/root-package",
                "/project/src/app.ts",
                b"export * from 'middle-package';",
                b"export * from 'middle-package';",
                vec![middle_edge],
            );
        root_request.resolved_import.exports.insert(
            "value".into(),
            leaf_request.resolved_import.exports["value"].clone(),
        );

        let graph = plan_published_contract_graph(
            PublishedGraphNodeRequest::new(
                root_request,
                root_archive,
                graph_lock("root-package", "1.0.0", &root_integrity),
            ),
            [
                PublishedGraphNodeRequest::new(
                    middle_request,
                    middle_archive,
                    graph_lock("middle-package", "2.0.0", &middle_integrity),
                ),
                PublishedGraphNodeRequest::new(
                    leaf_request,
                    leaf_archive,
                    graph_lock("leaf-package", "3.0.0", &leaf_integrity),
                ),
            ],
        )
        .unwrap();
        assert_eq!(graph.dependency_first_identities().len(), 3);
        assert_eq!(graph.root_identity().package_name, "root-package");
    }

    fn authenticated_graph_test_receipt(
        plan: &CertificationPlan,
        importer: &str,
        issuer: &ConfiguredReceiptIssuer,
        revocation_epoch: u64,
    ) -> super::AuthenticatedPolicy2Receipt {
        let canonical_main = crate::contract_document::encode(
            &plan.selected_candidate,
            &crate::contract_document::SidecarDigests::default(),
            false,
        )
        .unwrap();
        let root = |value: u8| format!("sha256:{:064x}", value);
        let witness_roots = [
            "package-identity",
            "manifest-entrypoint",
            "export-resolution",
            "artifact-declarations",
            "export-identity",
            "module-closure",
            "selected-signature",
            "argument-binding",
            "rest-spread-coverage",
            "callable-path",
            "operation-reachability",
            "operation-cardinality",
            "recursive-value-shape",
            "guard-partition",
            "compiler-reconciliation",
            "accepted-dependency-composition",
            "domain-exhaustiveness",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, family)| (family.into(), root(u8::try_from(index + 1).unwrap())))
        .collect();
        let semantic_digest = policy2_main_semantic_digest(&canonical_main).unwrap();
        let bindings = Policy2ReceiptBindings {
            importer: importer.into(),
            specifier: plan.import_request.specifier.clone(),
            resolved_import_root: super::policy2_resolved_import_root(&plan.resolved_import)
                .unwrap(),
            semantic_digest,
            artifact_provenance_root: plan.snapshot.provenance_root().into(),
            snapshot_root: plan.snapshot.root().into(),
            package_root: root(20),
            manifest_root: root(21),
            artifacts_root: root(22),
            declarations_root: root(23),
            transform_root: root(24),
            exports_root: root(25),
            closure_root: root(26),
            demand_graph_root: plan.demand_graph().root().as_str().into(),
            verified_positive_root: root(27),
            witness_roots,
            producer_sessions_root: root(28),
            dependency_receipts_root: root(29),
            dependency_trust_root: root(30),
            probe_gate_root: root(31),
            closed_claims_root: root(32),
            verifier_source_digest: root(33),
            verifier_build_digest: root(34),
        };
        let receipt = issue_policy2_receipt(&canonical_main, &bindings, issuer).unwrap();
        let trust = policy2_trust_configuration_for_issuer(
            issuer,
            &bindings.verifier_build_digest,
            revocation_epoch,
        )
        .unwrap();
        authenticate_policy2_receipt(
            &canonical_main,
            &receipt,
            &bindings,
            Policy2ReceiptProvenance::PersistentLocal {
                trust_store: trust.trust_store(),
                scope: issuer.scope(),
            },
        )
        .unwrap()
    }

    #[test]
    fn dependency_composition_requires_an_exact_opaque_receipt() {
        let (root, leaf) = two_node_published_graph(false, false, false);
        let graph = plan_published_contract_graph(root, [leaf]).unwrap();
        let root_identity = graph.root_identity().clone();
        let leaf_identity = graph
            .dependency_first_identities()
            .into_iter()
            .find(|identity| identity.package_name == "leaf-package")
            .unwrap()
            .clone();
        let leaf_plan = graph.plan(&leaf_identity).unwrap();
        assert_eq!(
            leaf_plan.candidates().proposal().artifact_cases()[0]
                .exports
                .len(),
            1
        );
        let issuer = ConfiguredReceiptIssuer::persistent_local("phase21-graph", [17; 32]).unwrap();
        let receipt =
            authenticated_graph_test_receipt(leaf_plan, &leaf_identity.importer, &issuer, 7);
        let composition = graph
            .authenticate_dependency_receipts(
                &root_identity,
                &[(&leaf_identity, &receipt)],
                &issuer,
                7,
            )
            .unwrap();
        assert_eq!(composition.graph_root(), graph.graph_root());
        assert_eq!(composition.witnesses().len(), 1);
        assert!(matches!(
            composition.verify_plan(leaf_plan),
            Err(DependencyReceiptCompositionError::ParentTransplant)
        ));

        let stale_epoch = graph.authenticate_dependency_receipts(
            &root_identity,
            &[(&leaf_identity, &receipt)],
            &issuer,
            8,
        );
        assert!(matches!(
            stale_epoch,
            Err(DependencyReceiptCompositionError::TrustMismatch)
        ));

        let transplanted = authenticated_graph_test_receipt(
            leaf_plan,
            "/other/node_modules/root-package/dist/index.js",
            &issuer,
            7,
        );
        assert!(matches!(
            graph.authenticate_dependency_receipts(
                &root_identity,
                &[(&leaf_identity, &transplanted)],
                &issuer,
                7,
            ),
            Err(DependencyReceiptCompositionError::ReceiptMismatch {
                field: "importer",
                ..
            })
        ));
    }

    #[test]
    fn dependency_composition_requires_the_receipt_to_close_the_exact_claim() {
        let (root, leaf) =
            two_node_published_graph_with_root_callbacks(false, false, false, true, false);
        let graph = plan_published_contract_graph(root, [leaf]).unwrap();
        let root_identity = graph.root_identity().clone();
        let leaf_identity = graph
            .dependency_first_identities()
            .into_iter()
            .find(|identity| identity.package_name == "leaf-package")
            .unwrap()
            .clone();
        let leaf_plan = graph.plan(&leaf_identity).unwrap();
        let issuer = ConfiguredReceiptIssuer::persistent_local("phase21-graph", [17; 32]).unwrap();
        let receipt =
            authenticated_graph_test_receipt(leaf_plan, &leaf_identity.importer, &issuer, 7);

        let result = graph.authenticate_dependency_receipts(
            &root_identity,
            &[(&leaf_identity, &receipt)],
            &issuer,
            7,
        );
        assert!(matches!(
            result,
            Err(DependencyReceiptCompositionError::MissingClosedClaim { .. })
        ));

        let transplanted = authenticated_graph_test_receipt(
            leaf_plan,
            "/other/node_modules/root-package/dist/index.js",
            &issuer,
            7,
        );
        assert!(matches!(
            graph.authenticate_dependency_receipts(
                &root_identity,
                &[(&leaf_identity, &transplanted)],
                &issuer,
                7,
            ),
            Err(DependencyReceiptCompositionError::ReceiptMismatch {
                field: "importer",
                ..
            })
        ));
        assert!(matches!(
            graph.authenticate_dependency_receipts(
                &root_identity,
                &[(&leaf_identity, &receipt)],
                &issuer,
                8,
            ),
            Err(DependencyReceiptCompositionError::TrustMismatch)
        ));
    }

    #[test]
    fn one_dependency_receipt_cannot_exchange_callbacks_for_throws() {
        let (root, leaf) =
            two_node_published_graph_with_root_callbacks(false, false, false, false, true);
        let graph = plan_published_contract_graph(root, [leaf]).unwrap();
        let root_identity = graph.root_identity().clone();
        let root_plan = graph.plan(&root_identity).unwrap();
        let leaf_identity = graph
            .dependency_first_identities()
            .into_iter()
            .find(|identity| identity.package_name == "leaf-package")
            .unwrap()
            .clone();
        let leaf_plan = graph.plan(&leaf_identity).unwrap();
        let issuer = ConfiguredReceiptIssuer::persistent_local("phase21-graph", [17; 32]).unwrap();
        let receipt =
            authenticated_graph_test_receipt(leaf_plan, &leaf_identity.importer, &issuer, 7);
        let schedule = root_plan.dependency_composition_schedule().unwrap();
        let requirement = &schedule.requirements()[0];
        assert!(requirement.authenticates_dependency_artifact());
        let claim_id = |domain| {
            leaf_plan
                .selected_candidate
                .claim_id(&SemanticClaimSubject {
                    artifact_case: leaf_plan.selected_artifact_case_id().into(),
                    export: "value".into(),
                    path: SemanticClaimPath::Domain(ClaimPath::Call(domain)),
                })
                .unwrap()
        };
        let callbacks = claim_id(ClaimDomain::Callbacks);
        let throws = claim_id(ClaimDomain::Throws);

        super::dependencies::authenticate_dependency_claim_for_test(
            root_plan,
            requirement,
            &leaf_identity,
            &receipt,
            &issuer,
            7,
            callbacks.as_str(),
        )
        .unwrap();
        assert!(matches!(
            super::dependencies::authenticate_dependency_claim_for_test(
                root_plan,
                requirement,
                &leaf_identity,
                &receipt,
                &issuer,
                7,
                throws.as_str(),
            ),
            Err(DependencyReceiptCompositionError::MissingClosedClaim { .. })
        ));
    }

    fn pinned_producer_for_test() -> Option<super::TypeFactsProducerPin> {
        let typefacts_path =
            std::fs::canonicalize(std::env::var_os("SOLID_TYPEFACTS_BIN")?).ok()?;
        let executable = std::fs::read(&typefacts_path).unwrap();
        let buildinfo: serde_json::Value = serde_json::from_slice(
            &std::fs::read(format!("{}.buildinfo", typefacts_path.display())).unwrap(),
        )
        .unwrap();
        let source_digest = buildinfo["sourceDigest"].as_str().unwrap();
        Some(
            super::TypeFactsProducerPin::new(
                typefacts_path,
                format!("sha256:{:x}", Sha256::digest(executable)),
                format!("sha256:{source_digest}"),
            )
            .unwrap(),
        )
    }

    /// A root package whose only claim — that `value` is callable — is provable
    /// exclusively from a *different* package's declarations. Its own `.d.ts`
    /// says nothing more than "`value` has the type `source-types` calls
    /// `Callback`"; a witness program built without `source-types` sees that
    /// name as `any`, which the producer correctly refuses to call callable.
    fn callable_through_external_declaration_root()
    -> (Vec<u8>, ImportRequest, ResolvedImport, PublishedArchive) {
        let package_root = "/project/node_modules/root-package";
        let manifest = br#"{"name":"root-package","version":"1.0.0","exports":{".":{"types":"./types/index.d.ts","import":"./dist/index.js"}}}"#;
        let runtime = b"export const value = () => true;";
        let declarations = b"import type { Callback } from \"source-types\";\nexport declare const value: Callback;\n";
        let archive = published_archive_for(
            "root-package",
            "1.0.0",
            &[
                ("package/package.json", manifest),
                ("package/dist/index.js", runtime),
                ("package/types/index.d.ts", declarations),
            ],
        );
        let snapshot =
            ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2()).unwrap();
        let runtime_file = resolved_file(package_root, "dist/index.js", runtime);
        let declaration_file = resolved_file(package_root, "types/index.d.ts", declarations);
        let closure = ClosureManifest::new(
            vec![
                closure_entry(ClosureFileRole::Manifest, "package.json", manifest),
                closure_entry(ClosureFileRole::ResolutionInput, "package.json", manifest),
                closure_entry(ClosureFileRole::Runtime, "dist/index.js", runtime),
                closure_entry(
                    ClosureFileRole::Declaration,
                    "types/index.d.ts",
                    declarations,
                ),
            ],
            Vec::new(),
            // The declaration import of `source-types` is an opaque frontier
            // for the closure replay whether or not its bytes are supplied as
            // evidence. Declaring it here is what a real resolver's manifest
            // does; it is not what makes the type resolvable.
            vec![ClosureHazard {
                kind: ClosureHazardKind::UnacceptedExternalDependency,
                source: "./types/index.d.ts:source-types".into(),
                affected_exports: Vec::new(),
                affected_domains: vec![
                    AffectedClaimDomain::Callbacks,
                    AffectedClaimDomain::Reads,
                    AffectedClaimDomain::Writes,
                    AffectedClaimDomain::Creates,
                    AffectedClaimDomain::Invalidates,
                    AffectedClaimDomain::Throws,
                    AffectedClaimDomain::Returns,
                    AffectedClaimDomain::Cleanups,
                    AffectedClaimDomain::Disposals,
                ],
            }],
        )
        .unwrap();
        let import_request = ImportRequest {
            specifier: "root-package".into(),
            importer: "/project/src/app.ts".into(),
            export_conditions: vec!["import".into()],
        };
        let resolved = ResolvedImport {
            specifier: "root-package".into(),
            importer: "/project/src/app.ts".into(),
            requested_entrypoint: ".".into(),
            package_name: "root-package".into(),
            package_version: "1.0.0".into(),
            package_integrity: snapshot.package_integrity().into(),
            package_root: package_root.into(),
            package_real_root: None,
            package_manifest: resolved_file(package_root, "package.json", manifest),
            runtime: runtime_file,
            declarations: declaration_file,
            runtime_trace: ResolutionTrace {
                branch: "/exports/./import".into(),
                steps: vec![
                    ResolutionTraceStep {
                        condition: "subpath".into(),
                        target: ".".into(),
                    },
                    ResolutionTraceStep {
                        condition: "import".into(),
                        target: "/exports/.".into(),
                    },
                    ResolutionTraceStep {
                        condition: "target".into(),
                        target: "./dist/index.js".into(),
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
            exports: BTreeMap::from([(
                "value".into(),
                ResolvedExportBinding {
                    runtime: ResolvedExportTarget {
                        module: resolved_file(package_root, "dist/index.js", runtime),
                        export_name: "value".into(),
                    },
                    declarations: ResolvedExportTarget {
                        module: resolved_file(package_root, "types/index.d.ts", declarations),
                        export_name: "value".into(),
                    },
                },
            )]),
            declaration_exports: BTreeSet::new(),
            authority: ResolutionAuthority::Host,
        };
        let (package, mut artifact_case) =
            crate::artifact_resolution::proposal_identity(&resolved).unwrap();
        artifact_case.exports.insert(
            "value".into(),
            ExportSemantics {
                identity: ExportIdentity {
                    entrypoint: artifact_case.entrypoint.clone(),
                    public_name: "value".into(),
                    runtime: ExportTargetIdentity {
                        module: artifact_case.runtime.clone(),
                        export_name: "value".into(),
                    },
                    declarations: ExportTargetIdentity {
                        module: artifact_case.declarations.clone(),
                        export_name: "value".into(),
                    },
                },
                shape: ValueShape::Callable,
                stability: StabilityKnowledge::Unknown,
                call: CallSemantics::new(
                    CallClaims::default(),
                    vec![],
                    vec![],
                    vec![],
                    GuardPartition {
                        cases: KnowledgeSet::Unknown,
                    },
                ),
            },
        );
        let document = crate::contract_document::encode(
            &ContractProposal::new(package, vec![artifact_case])
                .normalize()
                .unwrap(),
            &crate::contract_document::SidecarDigests::default(),
            false,
        )
        .unwrap();

        (document, import_request, resolved, archive)
    }

    /// One copy of `source-types` for the root above, at an exact installed
    /// location. `lock_integrity` overrides the lock selection's integrity so a
    /// copy can be made to fail authentication while remaining well-formed.
    fn external_declaration_source(
        version: &str,
        declarations: &[u8],
        installed_package_root: &str,
        lock_integrity: Option<&str>,
    ) -> PublishedGraphSourceRequest {
        let manifest = format!(
            r#"{{"name":"source-types","version":"{version}","exports":{{".":{{"types":"./types/index.d.ts","import":"./dist/index.js"}}}}}}"#
        );
        let archive = published_archive_for(
            "source-types",
            version,
            &[
                ("package/package.json", manifest.as_bytes()),
                ("package/dist/index.js", b"export {};"),
                ("package/types/index.d.ts", declarations),
            ],
        );
        let integrity = ArtifactSnapshot::from_published(&archive, SnapshotLimits::policy_2())
            .unwrap()
            .package_integrity()
            .to_owned();
        let lock = graph_lock(
            "source-types",
            version,
            lock_integrity.unwrap_or(&integrity),
        );
        PublishedGraphSourceRequest::new(archive, lock, installed_package_root)
    }

    fn dependencies_verify_for_test(
        transaction: &mut CertificationPlanningTransaction,
        requests: Vec<PublishedGraphSourceRequest>,
    ) -> Result<Vec<super::dependencies::VerifiedGraphSourcePackage>, PublishedGraphPlanningError>
    {
        super::dependencies::verify_certification_source_packages_for_test(transaction, requests)
    }

    fn callable_source(installed_package_root: &str) -> PublishedGraphSourceRequest {
        external_declaration_source(
            "3.0.0",
            b"export type Callback = () => boolean;\n",
            installed_package_root,
            None,
        )
    }

    /// A well-formed archive whose lock selection claims different bytes, so
    /// `plan_graph_source_package` refuses it on integrity.
    const SUBSTITUTED_INTEGRITY: &str = "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";

    /// Plans the cross-package root with `sources` and asks the pinned producer
    /// whether the callable claim is now provable. `Ok(())` means the demand
    /// closed; `Err` carries the refusal text.
    fn certify_cross_package_root_with(
        pin: &super::TypeFactsProducerPin,
        sources: Vec<PublishedGraphSourceRequest>,
    ) -> Result<(), String> {
        let (document, import_request, resolved, archive) =
            callable_through_external_declaration_root();
        let plan = CertificationPlanningTransaction::new()
            .plan_contract_document_with_sources(
                &document,
                import_request,
                resolved,
                UntrustedArtifactEnvelope::Published(archive),
                sources,
            )
            .unwrap();
        plan.acquire_and_verify_export_value_type_facts(pin)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    #[test]
    fn root_certification_withholds_every_copy_of_a_name_one_copy_could_not_authenticate() {
        let Some(pin) = pinned_producer_for_test() else {
            return;
        };
        // The installed truth: a nested copy of `source-types` shadows the
        // hoisted one, and it is the nested copy that types `Callback` as a
        // non-callable object. Only the hoisted copy authenticates.
        //
        // Materializing just the hoisted copy is not "removing evidence":
        // `moduleResolution: "bundler"` walks up, finds it, and proves the
        // export callable from a version that never governed this import. The
        // whole name must be withheld so the module is simply missing.
        let refusal = certify_cross_package_root_with(
            &pin,
            vec![
                callable_source("/project/node_modules/source-types"),
                external_declaration_source(
                    "4.0.0",
                    b"export type Callback = { notCallable: true };\n",
                    "/project/node_modules/root-package/node_modules/source-types",
                    Some(SUBSTITUTED_INTEGRITY),
                ),
            ],
        )
        .expect_err("a partially authenticated name must not prove anything");
        assert!(
            refusal.contains("is not compiler-proved callable or constructable"),
            "{refusal}"
        );
    }

    #[test]
    fn root_certification_drops_a_source_whose_lock_selection_claims_other_bytes() {
        let Some(pin) = pinned_producer_for_test() else {
            return;
        };
        let refusal = certify_cross_package_root_with(
            &pin,
            vec![external_declaration_source(
                "3.0.0",
                b"export type Callback = () => boolean;\n",
                "/project/node_modules/source-types",
                Some(SUBSTITUTED_INTEGRITY),
            )],
        )
        .expect_err("an archive the lock does not select must not become evidence");
        assert!(
            refusal.contains("is not compiler-proved callable or constructable"),
            "{refusal}"
        );
    }

    #[test]
    fn root_certification_drops_a_source_installed_outside_an_exact_node_modules_coordinate() {
        let Some(pin) = pinned_producer_for_test() else {
            return;
        };
        let refusal = certify_cross_package_root_with(
            &pin,
            vec![callable_source("/project/vendor/source-types")],
        )
        .expect_err("a source outside an exact node_modules coordinate is not authenticated");
        assert!(
            refusal.contains("is not compiler-proved callable or constructable"),
            "{refusal}"
        );
    }

    #[test]
    fn root_certification_withholds_a_name_whose_copies_collide_in_the_private_project() {
        let Some(pin) = pinned_producer_for_test() else {
            return;
        };
        // Neither copy sits under the owner or under the owner's installation
        // root, so `private_project_package_target` projects both onto the
        // project's top-level `node_modules/source-types`. Materializing them
        // would be an `AlreadyExists` write — a hard source-census failure this
        // path must not have — and keeping only one would be substitution.
        let refusal = certify_cross_package_root_with(
            &pin,
            vec![
                callable_source("/elsewhere/node_modules/source-types"),
                external_declaration_source(
                    "4.0.0",
                    b"export type Callback = () => boolean;\n",
                    "/other-place/node_modules/source-types",
                    None,
                ),
            ],
        )
        .expect_err("colliding copies of one name must be withheld, not half-materialized");
        assert!(
            refusal.contains("is not compiler-proved callable or constructable"),
            "{refusal}"
        );
    }

    #[test]
    fn certification_sources_root_names_the_exact_closure_a_receipt_was_proved_against() {
        let (document, import_request, resolved, archive) =
            callable_through_external_declaration_root();
        let plan_with = |sources| {
            CertificationPlanningTransaction::new()
                .plan_contract_document_with_sources(
                    &document,
                    import_request.clone(),
                    resolved.clone(),
                    UntrustedArtifactEnvelope::Published(archive.clone()),
                    sources,
                )
                .unwrap()
        };
        let empty = plan_with(Vec::new());
        let full = plan_with(vec![callable_source("/project/node_modules/source-types")]);
        // A dropped source must be indistinguishable from one never supplied,
        // so that a receipt states the closure it really used.
        let dropped = plan_with(vec![callable_source("/project/vendor/source-types")]);

        assert_ne!(
            empty.certification_sources_root(),
            full.certification_sources_root(),
            "an auditor must be able to tell a full closure from an empty one"
        );
        assert_eq!(
            empty.certification_sources_root(),
            dropped.certification_sources_root(),
            "a withheld source is not part of the closure the receipt claims"
        );
        assert_eq!(
            empty.demand_graph().root(),
            full.demand_graph().root(),
            "the closure is evidence, not a semantic claim"
        );
    }

    #[test]
    fn graph_nodes_still_refuse_a_source_they_cannot_authenticate() {
        let mut transaction = CertificationPlanningTransaction::new();
        // The root path drops; a graph node must not, because its canonical
        // identity binds `source_dependencies_root`.
        assert!(matches!(
            dependencies_verify_for_test(
                &mut transaction,
                vec![callable_source("/project/vendor/source-types")]
            ),
            Err(PublishedGraphPlanningError::InvalidSourcePackageRoot(_))
        ));
        assert!(matches!(
            dependencies_verify_for_test(
                &mut transaction,
                vec![external_declaration_source(
                    "3.0.0",
                    b"export type Callback = () => boolean;\n",
                    "/project/node_modules/source-types",
                    Some(SUBSTITUTED_INTEGRITY),
                )]
            ),
            Err(PublishedGraphPlanningError::LockDisagreement {
                field: "source package integrity",
                ..
            })
        ));
        assert!(matches!(
            dependencies_verify_for_test(
                &mut transaction,
                vec![
                    callable_source("/project/node_modules/source-types"),
                    callable_source("/project/node_modules/source-types"),
                ]
            ),
            Err(PublishedGraphPlanningError::DuplicateSourceDependency)
        ));
    }

    #[test]
    fn root_certification_proves_a_cross_package_type_only_from_authenticated_sources() {
        let Some(pin) = pinned_producer_for_test() else {
            return;
        };
        let (document, import_request, resolved, archive) =
            callable_through_external_declaration_root();
        let source = callable_source("/project/node_modules/source-types");
        let mut transaction = CertificationPlanningTransaction::new();

        // Exactly today's behaviour when the type-providing package is outside
        // the authenticated set: `Callback` is `any`, so the producer refuses to
        // call the export callable and the demand stays open.
        let without_sources = transaction
            .plan_contract_document_with_sources(
                &document,
                import_request.clone(),
                resolved.clone(),
                UntrustedArtifactEnvelope::Published(archive.clone()),
                Vec::new(),
            )
            .unwrap();
        let refusal = match without_sources.acquire_and_verify_export_value_type_facts(&pin) {
            Ok(_) => panic!("an unresolved cross-package type cannot prove callability"),
            Err(error) => error.to_string(),
        };
        assert!(
            refusal.contains("is not compiler-proved callable or constructable"),
            "{refusal}"
        );

        // The same plan, the same claim, the same demand graph — with the one
        // package whose authenticated declarations make the claim provable.
        let with_sources = transaction
            .plan_contract_document_with_sources(
                &document,
                import_request,
                resolved,
                UntrustedArtifactEnvelope::Published(archive),
                vec![source],
            )
            .unwrap();
        assert_eq!(
            with_sources.demand_graph().root(),
            without_sources.demand_graph().root(),
            "supplying evidence must not move the demand graph"
        );
        if let Err(error) = with_sources.acquire_and_verify_export_value_type_facts(&pin) {
            panic!("the authenticated declaration must prove the callable root: {error}");
        }
    }

    #[test]
    fn published_graph_certifies_bottom_up_with_the_pinned_producer() {
        let Some(pin) = pinned_producer_for_test() else {
            return;
        };
        let (root, leaf) = two_node_published_graph(false, false, false);
        let graph = plan_published_contract_graph(root, [leaf]).unwrap();
        let issuer = ConfiguredReceiptIssuer::persistent_local("phase21-graph", [19; 32]).unwrap();
        let finalized = graph.certify_value_only(&pin, &issuer, 9).unwrap();
        assert_eq!(finalized.nodes().len(), 2);
        assert_eq!(finalized.graph_root(), graph.graph_root());
        assert_ne!(
            finalized.root().bindings().dependency_receipts_root,
            finalized.nodes()[0]
                .finalized()
                .bindings()
                .dependency_receipts_root
        );
    }
}
