//! The only package-contract boundary exposed to analyzer callers.
//!
//! Schema spellings and compact document mechanics remain private. Stable-v1
//! documents are decoded and normalized by the sibling deep module; this
//! analyzer-loading interface exposes semantics only after validating a
//! proof-issued receipt and exact artifact selection.

use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{
    AcceptedContract, AcceptedContractIndex, AcceptedContractInput, UncertifiableImportReason,
    proof::{
        AuthenticatedPolicy2Acceptance, accept_authenticated_policy2,
        project_untrusted_proposal_for_generation,
    },
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

use crate::{
    contract_certification::{
        AuthenticatedPolicy2Receipt, BuiltInReceiptEntry, Policy2ReceiptBindings,
        Policy2ReceiptProvenance, Policy2TrustConfiguration, authenticate_policy2_receipt,
        canonicalize_policy2_main, decode_policy2_trust_configuration,
        policy2_resolved_import_root,
    },
    contract_document,
};

const MAX_CONTRACT_DOCUMENT_BYTES: usize = 1024 * 1024;
const MAX_RECEIPT_BYTES: usize = 64 * 1024;
const MAX_CATALOG_BYTES: usize = 16 * 1024 * 1024;
const MAX_CATALOG_CONTRACTS: usize = 65_536;
const MAX_BOUNDARY_DEPTH: usize = 128;
const MAX_BOUNDARY_STRING_BYTES: usize = 16 * 1024;
const MAX_RECEIPT_NODES: usize = 4_096;
const MAX_CATALOG_NODES: usize = 1_000_000;
const MAX_TRUST_CONFIGURATION_BYTES: usize = 64 * 1024;

pub use crate::artifact_resolution::{
    AcceptedDependencyEdge, AffectedClaimDomain, ArtifactResolutionFailure, ArtifactResolver,
    ArtifactResolverChain, ClosureEntry, ClosureFileRole, ClosureHazard, ClosureHazardKind,
    ClosureInput, ClosureManifest, ClosurePackageIdentity, HostResolutionAdapter, ImportRequest,
    ResolutionAuthority, ResolutionTrace, ResolutionTraceStep, ResolvedExportBinding,
    ResolvedExportTarget, ResolvedFile, ResolvedImport, StandaloneResolutionAdapter,
    TypeFactsResolutionAdapter,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EvidenceKey(String);

impl EvidenceKey {
    pub fn parse(value: impl Into<String>) -> Result<Self, EvidenceStoreFailure> {
        let value = value.into();
        let digest = value
            .strip_prefix("sha256:")
            .ok_or(EvidenceStoreFailure::InvalidKey)?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(EvidenceStoreFailure::InvalidKey);
        }
        Ok(Self(format!("sha256:{}", digest.to_ascii_lowercase())))
    }

    #[must_use]
    pub fn for_content(bytes: &[u8]) -> Self {
        Self(sha256_digest(bytes))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn filename(&self) -> &str {
        self.0
            .strip_prefix("sha256:")
            .expect("validated evidence key")
    }
}

pub trait EvidenceStore {
    fn receipt(&self, key: &EvidenceKey) -> Result<Option<Arc<[u8]>>, EvidenceStoreFailure>;
}

pub trait ReceiptStore: EvidenceStore {
    fn store_receipt(&self, bytes: &[u8]) -> Result<EvidenceKey, EvidenceStoreFailure>;
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum EvidenceStoreFailure {
    #[error("invalid content-addressed evidence key")]
    InvalidKey,
    #[error("evidence store I/O failed: {message}")]
    Io { message: String },
    #[error("evidence content does not match its content-addressed key")]
    ContentMismatch,
    #[error("evidence content exceeds the {limit}-byte resource limit")]
    ResourceLimit { limit: usize },
}

#[derive(Clone, Debug, Default)]
pub struct BundledEvidenceStore {
    receipts: BTreeMap<EvidenceKey, Arc<[u8]>>,
}

impl BundledEvidenceStore {
    #[must_use]
    pub fn new(receipts: impl IntoIterator<Item = (EvidenceKey, Arc<[u8]>)>) -> Self {
        Self {
            receipts: receipts.into_iter().collect(),
        }
    }
}

impl EvidenceStore for BundledEvidenceStore {
    fn receipt(&self, key: &EvidenceKey) -> Result<Option<Arc<[u8]>>, EvidenceStoreFailure> {
        self.receipts
            .get(key)
            .cloned()
            .map(|bytes| verify_evidence_content(key, bytes))
            .transpose()
    }
}

#[derive(Clone, Debug)]
pub struct LocalEvidenceStore {
    root: PathBuf,
}

impl LocalEvidenceStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn receipt_path(&self, key: &EvidenceKey) -> PathBuf {
        self.root.join("receipts").join(key.filename())
    }
}

impl EvidenceStore for LocalEvidenceStore {
    fn receipt(&self, key: &EvidenceKey) -> Result<Option<Arc<[u8]>>, EvidenceStoreFailure> {
        let path = self.receipt_path(key);
        match fs::metadata(&path) {
            Ok(metadata) if metadata.len() > MAX_RECEIPT_BYTES as u64 => {
                return Err(EvidenceStoreFailure::ResourceLimit {
                    limit: MAX_RECEIPT_BYTES,
                });
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_failure(error)),
        }
        match fs::read(path) {
            Ok(bytes) => verify_evidence_content(key, bytes.into()).map(Some),
            Err(error) => Err(EvidenceStoreFailure::Io {
                message: error.to_string(),
            }),
        }
    }
}

impl ReceiptStore for LocalEvidenceStore {
    fn store_receipt(&self, bytes: &[u8]) -> Result<EvidenceKey, EvidenceStoreFailure> {
        if bytes.len() > MAX_RECEIPT_BYTES {
            return Err(EvidenceStoreFailure::ResourceLimit {
                limit: MAX_RECEIPT_BYTES,
            });
        }
        let key = EvidenceKey::for_content(bytes);
        let path = self.receipt_path(&key);
        if let Some(existing) = self.receipt(&key)? {
            if existing.as_ref() == bytes {
                return Ok(key);
            }
            return Err(EvidenceStoreFailure::ContentMismatch);
        }
        let directory = path.parent().expect("receipt path has a parent");
        fs::create_dir_all(directory).map_err(io_failure)?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary = directory.join(format!(
            ".{}.{}.{nonce}.tmp",
            key.filename(),
            std::process::id()
        ));
        fs::write(&temporary, bytes).map_err(io_failure)?;
        if let Err(error) = fs::rename(&temporary, &path) {
            let _ = fs::remove_file(&temporary);
            return Err(io_failure(error));
        }
        verify_evidence_content(&key, fs::read(path).map_err(io_failure)?.into())?;
        Ok(key)
    }
}

fn io_failure(error: std::io::Error) -> EvidenceStoreFailure {
    EvidenceStoreFailure::Io {
        message: error.to_string(),
    }
}

fn verify_evidence_content(
    key: &EvidenceKey,
    bytes: Arc<[u8]>,
) -> Result<Arc<[u8]>, EvidenceStoreFailure> {
    if sha256_digest(&bytes) == key.0 {
        Ok(bytes)
    } else {
        Err(EvidenceStoreFailure::ContentMismatch)
    }
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Error)]
pub enum ContractFailure {
    #[error("contract document exceeds the {limit}-byte resource limit")]
    DocumentTooLarge { limit: usize },
    #[error("contract document cannot be decoded: {message}")]
    DocumentDecode { message: String },
    #[error("acceptance receipt cannot be decoded: {message}")]
    ReceiptDecode { message: String },
    #[error("unsupported acceptance receipt version {actual}; expected {expected}")]
    UnsupportedReceiptVersion { expected: u16, actual: u16 },
    #[error("policy-2 acceptance receipt requires authenticated issuer provenance")]
    ReceiptAuthenticationRequired,
    #[error("policy-2 acceptance receipt authentication failed: {message}")]
    ReceiptAuthentication { message: String },
    #[error("unsupported contract schema version {actual}; expected {expected}")]
    UnsupportedSchemaVersion { expected: u16, actual: u16 },
    #[error("no artifact case matches the exact resolved import")]
    NoArtifactCase,
    #[error("multiple artifact cases match the exact resolved import")]
    MultipleArtifactCases,
    #[error("contract identity does not match the resolved import: {reason}")]
    IdentityMismatch { reason: String },
    #[error("acceptance receipt does not bind the selected contract: {field}")]
    ReceiptMismatch { field: &'static str },
    #[error("normalized operation graph is invalid: {reason}")]
    InvalidSemanticModel { reason: String },
}

/// One contract document, proof-issued receipt, and exact host resolution to
/// load at the analyzer boundary.
#[derive(Clone, Copy, Debug)]
pub struct AcceptedContractSource<'a> {
    pub document: &'a [u8],
    pub receipt: &'a [u8],
    pub import: &'a ResolvedImport,
}

const ACCEPTED_CATALOG_FORMAT: &str = "solid-checker-accepted-contract-catalog";
const ACCEPTED_CATALOG_VERSION: u16 = 2;
const PROPOSAL_DEPENDENCY_CATALOG_FORMAT: &str = "solid-checker-proposal-dependency-catalog";
const PROPOSAL_DEPENDENCY_CATALOG_VERSION: u16 = 1;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AcceptedCatalogDocument {
    format: String,
    catalog_version: u16,
    contracts: Vec<AcceptedCatalogEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AcceptedCatalogEntry {
    document: String,
    #[serde(default)]
    document_digest: Option<String>,
    #[serde(default)]
    receipt: Option<String>,
    #[serde(default)]
    receipt_digest: Option<String>,
    #[serde(default)]
    bindings: Option<Policy2ReceiptBindings>,
    status: AcceptedCatalogStatus,
    import: ResolvedImport,
    /// The export conditions `artifactAcceptanceRoot` was computed over.
    ///
    /// `None` for a catalog published before this was recorded. Those fall back
    /// to the `["import"]` guess below, which is what every consumer did
    /// unconditionally until now — so an older catalog keeps exactly the
    /// behaviour it had, and a newer one stops needing the guess.
    #[serde(default)]
    export_conditions: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProposalDependencyCatalogDocument {
    format: String,
    catalog_version: u16,
    contracts: Vec<ProposalDependencyCatalogEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProposalDependencyCatalogEntry {
    document: String,
    document_digest: String,
    import: ResolvedImport,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum AcceptedCatalogStatus {
    ObsoletePolicy1,
    Policy2PersistentLocal,
    Policy2Portable,
}

/// Decodes and normalizes one catalog document, once per distinct
/// `documentDigest`.
///
/// A digest-duplicated entry is the ordinary shape now that the catalog names
/// every module of a package that re-exports a dependency: N entries share one
/// document object and differ only in the importer the generation-time index
/// is keyed by. Every entry still reads its own file and is digest-checked
/// against it before it gets here, so nothing is admitted on a neighbour's
/// evidence; only the decode and normalization is shared, and that is a pure
/// function of exactly the bytes the digest pins.
fn normalized_catalog_document<'memo>(
    memo: &'memo mut BTreeMap<String, solid_reactive_ir::contract_semantics::NormalizedContract>,
    document_digest: &str,
    document: &[u8],
) -> Result<&'memo solid_reactive_ir::contract_semantics::NormalizedContract, ContractFailure> {
    if !memo.contains_key(document_digest) {
        let normalized = contract_document::decode(document)?.normalize()?;
        memo.insert(document_digest.to_owned(), normalized);
    }
    Ok(memo
        .get(document_digest)
        .expect("the entry was just inserted when absent"))
}

/// Loads open child proposals for one private graph-generation process. The
/// resulting semantics are explicitly unauthenticated projection material;
/// this reader is never used by ordinary discovery, diagnostics, or catalog
/// publication. The final native graph transaction replays every archive,
/// resolution, closure edge, semantic digest, receipt, and graph root before
/// any generated parent contract can become authoritative.
#[doc(hidden)]
pub fn read_proposal_dependency_catalog_for_generation(
    path: &Path,
) -> Result<AcceptedContractIndex, ContractFailure> {
    let bytes = read_boundary_file(
        path,
        MAX_CATALOG_BYTES,
        "proposal dependency catalog",
        false,
    )?;
    let catalog: ProposalDependencyCatalogDocument = crate::bounded_json::decode(
        &bytes,
        crate::bounded_json::Limits {
            bytes: MAX_CATALOG_BYTES,
            depth: MAX_BOUNDARY_DEPTH,
            nodes: MAX_CATALOG_NODES,
            string_bytes: MAX_BOUNDARY_STRING_BYTES,
        },
    )
    .map_err(|message| ContractFailure::DocumentDecode {
        message: format!(
            "decode proposal dependency catalog {}: {message}",
            path.display()
        ),
    })?;
    if catalog.format != PROPOSAL_DEPENDENCY_CATALOG_FORMAT
        || catalog.catalog_version != PROPOSAL_DEPENDENCY_CATALOG_VERSION
    {
        return Err(ContractFailure::DocumentDecode {
            message: format!(
                "proposal dependency catalog must use format {PROPOSAL_DEPENDENCY_CATALOG_FORMAT:?} version {PROPOSAL_DEPENDENCY_CATALOG_VERSION}"
            ),
        });
    }
    if catalog.contracts.len() > MAX_CATALOG_CONTRACTS {
        return Err(ContractFailure::DocumentDecode {
            message: format!(
                "proposal dependency catalog exceeds the {MAX_CATALOG_CONTRACTS} contract resource limit"
            ),
        });
    }
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let mut projected = Vec::with_capacity(catalog.contracts.len());
    let mut normalized_documents = BTreeMap::new();
    for mut entry in catalog.contracts {
        let document = read_boundary_file(
            &catalog_member_path(base, &entry.document)?,
            MAX_CONTRACT_DOCUMENT_BYTES,
            "proposal dependency",
            false,
        )?;
        if sha256_digest(&document) != entry.document_digest {
            return Err(ContractFailure::ReceiptMismatch {
                field: "documentDigest",
            });
        }
        rebase_catalog_import(base, &mut entry.import)?;
        let normalized = normalized_catalog_document(
            &mut normalized_documents,
            &entry.document_digest,
            &document,
        )?;
        let external_targets =
            crate::artifact_resolution::resolved_external_export_targets(&entry.import)?;
        let selected = crate::artifact_resolution::select_and_bind_with_external_targets(
            normalized,
            &entry.import,
            &external_targets,
        )?;
        let selected_case = selected
            .artifact_cases()
            .first()
            .ok_or(ContractFailure::NoArtifactCase)?
            .id
            .clone();
        let contract = project_untrusted_proposal_for_generation(selected, &selected_case)
            .map_err(|error| ContractFailure::InvalidSemanticModel {
                reason: error.to_string(),
            })?;
        projected.push(AcceptedContractInput {
            importer: entry.import.importer,
            specifier: entry.import.specifier,
            contract,
            // Unauthenticated projection material for one generation process;
            // it carries no receipt, so there is no signed artifact identity
            // to match and this stays importer-only.
            artifact_identity: None,
        });
    }
    AcceptedContractIndex::new(projected).map_err(|error| ContractFailure::IdentityMismatch {
        reason: error.to_string(),
    })
}

/// Reads an explicit host-acquisition catalog and terminates every file and
/// wire-format concept at this boundary. Paths are relative to the catalog;
/// analyzer consumers receive only the accepted semantic index.
pub fn read_accepted_contract_catalog(
    path: &Path,
) -> Result<AcceptedContractIndex, ContractFailure> {
    read_accepted_contract_catalog_with_trust(path, None)
}

/// Reads the separately configured trust authority for ordinary policy-2
/// discovery. The path is selected by the host process, never by the analyzed
/// project catalog.
pub fn read_policy2_trust_configuration(
    path: &Path,
) -> Result<Policy2TrustConfiguration, ContractFailure> {
    let bytes = read_boundary_file(
        path,
        MAX_TRUST_CONFIGURATION_BYTES,
        "policy-2 trust configuration",
        false,
    )?;
    decode_policy2_trust_configuration(&bytes).map_err(authentication_error)
}

/// Loads a normal discovery catalog with separately acquired policy-2 trust.
/// Trust bytes are deliberately not referenced by the project catalog: doing
/// so would let an analyzed project nominate its own issuer.
pub fn read_accepted_contract_catalog_with_trust(
    path: &Path,
    trust: Option<&Policy2TrustConfiguration>,
) -> Result<AcceptedContractIndex, ContractFailure> {
    read_catalog_with_trust(path, trust, false)
}

/// Ordinary analysis does not read core documents or receipts. Their catalog
/// entries are withheld before opening any content object; this grants no
/// premise about what the actual import resolves to.
pub fn read_external_contract_catalog_with_trust(
    path: &Path,
    trust: Option<&Policy2TrustConfiguration>,
) -> Result<AcceptedContractIndex, ContractFailure> {
    read_catalog_with_trust(path, trust, true)
}

fn read_catalog_with_trust(
    path: &Path,
    trust: Option<&Policy2TrustConfiguration>,
    external_only: bool,
) -> Result<AcceptedContractIndex, ContractFailure> {
    let (catalog, base) = decode_accepted_contract_catalog(path)?;
    let mut uncertifiable = Vec::with_capacity(catalog.contracts.len());
    let mut accepted = Vec::new();
    for mut entry in catalog.contracts {
        if external_only
            && solid_dialect::core_runtime_contract_reference(
                &entry.import.package_name,
                &entry.import.specifier,
            )
        {
            continue;
        }
        let document_path = catalog_member_path(&base, &entry.document)?;
        let document = read_boundary_file(
            &document_path,
            MAX_CONTRACT_DOCUMENT_BYTES,
            "contract",
            false,
        )?;
        rebase_catalog_import(&base, &mut entry.import)?;
        match entry.status {
            AcceptedCatalogStatus::ObsoletePolicy1 => uncertifiable.push((
                entry.import.importer.clone(),
                entry.import.specifier.clone(),
            )),
            AcceptedCatalogStatus::Policy2PersistentLocal
            | AcceptedCatalogStatus::Policy2Portable => {
                let trust = trust.ok_or(ContractFailure::ReceiptAuthenticationRequired)?;
                let receipt_path = entry
                    .receipt
                    .as_deref()
                    .ok_or_else(|| catalog_field("policy-2 entry has no receipt path"))
                    .and_then(|path| catalog_member_path(&base, path))?;
                let receipt =
                    read_boundary_file(&receipt_path, MAX_RECEIPT_BYTES, "receipt", true)?;
                let bindings = entry
                    .bindings
                    .as_ref()
                    .ok_or_else(|| catalog_field("policy-2 entry has no receipt bindings"))?;
                verify_catalog_digest(
                    &document,
                    entry.document_digest.as_deref(),
                    "documentDigest",
                )?;
                verify_catalog_digest(&receipt, entry.receipt_digest.as_deref(), "receiptDigest")?;
                if bindings.importer != entry.import.importer
                    || bindings.specifier != entry.import.specifier
                {
                    return Err(ContractFailure::ReceiptMismatch {
                        field: if bindings.importer != entry.import.importer {
                            "importer"
                        } else {
                            "specifier"
                        },
                    });
                }
                let provenance = match entry.status {
                    AcceptedCatalogStatus::Policy2PersistentLocal => {
                        let scope = trust
                            .persistent_local_scope()
                            .ok_or(ContractFailure::ReceiptAuthenticationRequired)?;
                        Policy2ReceiptProvenance::PersistentLocal {
                            trust_store: trust.trust_store(),
                            scope,
                        }
                    }
                    AcceptedCatalogStatus::Policy2Portable => Policy2ReceiptProvenance::Portable {
                        trust_store: trust.trust_store(),
                    },
                    AcceptedCatalogStatus::ObsoletePolicy1 => unreachable!(),
                };
                let contract = load_authenticated_policy2_contract(
                    &document,
                    &receipt,
                    &entry.import,
                    bindings,
                    provenance,
                )?;
                accepted.push(AcceptedContractInput {
                    importer: entry.import.importer.clone(),
                    specifier: entry.import.specifier.clone(),
                    contract,
                    artifact_identity: default_condition_artifact_identity(
                        &entry.import,
                        bindings,
                        entry.export_conditions.as_deref(),
                    ),
                });
            }
        }
    }
    Ok(AcceptedContractIndex::new(accepted)
        .map_err(|error| ContractFailure::IdentityMismatch {
            reason: error.to_string(),
        })?
        .with_uncertifiable_import_reasons(
            uncertifiable
                .into_iter()
                .map(|key| (key, UncertifiableImportReason::ObsoletePolicy1)),
        ))
}

/// The receipt's importer-free artifact identity, but only when this
/// acceptance was issued under the default single `import` condition.
///
/// Conditions select the artifact, and they are not written in the catalog's
/// import record — only folded into the signed root. So the check is a
/// recomputation: derive the identity assuming `["import"]` and keep it only if
/// it reproduces what the receipt signed. A multi-condition acceptance does not
/// reproduce it, yields `None`, and stays importer-only, which is the
/// fail-closed direction — those imports keep raising the obligation they raise
/// today rather than matching on a condition set nobody checked.
///
/// The narrowing exists because the analyzer has no condition facts at all
/// (`2026-09-14-acceptance-identity-spike.md` § 5). When it has them, this
/// becomes a comparison against the consumer's own conditions and the
/// restriction lifts.
fn default_condition_artifact_identity(
    import: &crate::artifact_resolution::ResolvedImport,
    bindings: &crate::contract_certification::Policy2ReceiptBindings,
    export_conditions: Option<&[String]>,
) -> Option<String> {
    if bindings.artifact_acceptance_root.is_empty() {
        return None;
    }
    // The set the catalog recorded, and only `["import"]` as a fallback for a
    // catalog published before it was recorded. Guessing was the defect: an
    // entry certified under `node, import` derived a root for `import`, matched
    // nothing, and the case was unreachable however the consumer declared
    // itself. The equality below is still the whole check — a recorded set that
    // does not reproduce the signed root states nothing.
    let fallback = ["import".to_owned()];
    let conditions = export_conditions.unwrap_or(&fallback);
    let derived =
        crate::contract_certification::policy2_artifact_acceptance_root(import, conditions).ok()?;
    (derived == bindings.artifact_acceptance_root).then_some(derived)
}

/// What the caller can state about a specifier's installed package: its name,
/// its version, and the registry integrity its lockfile selected. `None` for
/// anything the project cannot state exactly — an absent lockfile entry, or two
/// installs that disagree.
pub type InstalledArtifactIdentity<'a> = dyn Fn(&str) -> Option<(String, String, String)> + 'a;

/// Derives, for every specifier this catalog accepts, whether *this* project's
/// installed artifact is the one the acceptance was issued for.
///
/// The identity is recomputed from the installed tree — the package's registry
/// integrity from its lockfile, the entrypoint the specifier names, and the
/// host's declared export conditions — and admitted only when it reproduces the
/// signed root. Anything the project cannot state exactly is skipped, so this
/// adds acceptances and never removes one.
///
/// `conditions` is the host's declaration, not a guess: the analyzer has no
/// condition facts of its own, and conditions select the artifact, so an empty
/// set admits nothing rather than assuming `import`.
pub fn admitted_project_artifacts(
    catalogs: &[PathBuf],
    trust: Option<&Policy2TrustConfiguration>,
    project_directory: &Path,
    conditions: &std::collections::BTreeSet<String>,
    installed_integrity: &InstalledArtifactIdentity,
    resolved_target: &ResolvedTargetIdentity,
) -> Result<Vec<(String, String)>, ContractFailure> {
    let _ = (project_directory, trust);
    // The host's set, plus the module format the analyzer actually resolved
    // with. `--runtime-target browser` describes an *environment*; it says
    // nothing about `import` versus `require`, and every export map splits on
    // that first. Without this an SSR app declaring its target derived
    // `{browser}`, no case's `["import"]` was a subset of it, and declaring the
    // environment made things *worse* than declaring nothing — measured.
    //
    // Added only when the host named neither format itself: a project that
    // explicitly declares `require` is describing a build whose resolution this
    // analyzer did not perform, and overriding that would be inventing a fact.
    let mut declared = conditions.iter().cloned().collect::<Vec<_>>();
    if !declared.is_empty() && !declared.iter().any(|it| it == "import" || it == "require") {
        declared.push("import".to_owned());
    }
    declared.sort();
    // Every authentic case, across every catalog. A case set publishes one
    // catalog *per case*, so a per-catalog decision would never see two cases
    // of the same package together and could not tell an unambiguous artifact
    // from an ambiguous one -- it would admit both, which is the unsound
    // direction.
    let mut authentic: BTreeMap<String, Vec<AuthenticCase>> = BTreeMap::new();
    for path in catalogs {
        let (catalog, _) = decode_accepted_contract_catalog(path)?;
        for entry in catalog.contracts {
            if !matches!(
                entry.status,
                AcceptedCatalogStatus::Policy2PersistentLocal
                    | AcceptedCatalogStatus::Policy2Portable
            ) {
                continue;
            }
            let Some(bindings) = entry
                .bindings
                .as_ref()
                .filter(|bindings| !bindings.artifact_acceptance_root.is_empty())
            else {
                continue;
            };
            let Some((name, version, integrity)) = installed_integrity(&entry.import.specifier)
            else {
                continue;
            };
            // Recompute against the *installed* identity rather than the
            // catalog's record of it: the catalog states what certification
            // resolved, and the question here is whether this project resolved
            // the same thing.
            let mut installed = entry.import.clone();
            installed.package_name = name;
            installed.package_version = version;
            installed.package_integrity = integrity;
            // The conditions the entry recorded, not the ones this host
            // declared. Reproducing the signed root is a statement about the
            // *case*; whether it applies to this project is decided below.
            let fallback = ["import".to_owned()];
            let case_conditions = entry
                .export_conditions
                .clone()
                .unwrap_or_else(|| fallback.to_vec());
            let Ok(derived) = crate::contract_certification::policy2_artifact_acceptance_root(
                &installed,
                &case_conditions,
            ) else {
                continue;
            };
            if derived != bindings.artifact_acceptance_root {
                continue;
            }
            let Some(case) = AuthenticCase::new(derived, &entry.import, case_conditions) else {
                continue;
            };
            authentic
                .entry(entry.import.specifier.clone())
                .or_default()
                .push(case);
        }
    }

    let mut admitted = Vec::new();
    for (specifier, cases) in authentic {
        // What this project resolved the specifier to. Without it nothing is
        // admitted.
        let Some(target) = resolved_target(&specifier) else {
            continue;
        };
        // TypeScript resolves the *declaration* file, and one `.d.ts` is
        // routinely shared by several export-condition branches. So the
        // resolved file selects a set of candidate cases, not one case.
        let reaching = cases
            .iter()
            .filter(|case| case.reaches(&target))
            .collect::<Vec<_>>();
        let Some(selected) = select_case(&reaching, &declared) else {
            continue;
        };
        admitted.push((specifier, selected.identity.clone()));
    }
    Ok(admitted)
}

/// Chooses which acceptance applies to this project, among those certified
/// about a file it actually resolved.
///
/// Two regimes, because the honest answer differs:
///
/// - **No declaration** — the linter case. ESLint and Oxlint hosts do not know
///   their export conditions, and a wrong guess is worse than none: the guess
///   this replaced was the constant `["import"]`, which refused every project
///   that declared its real conditions and admitted only ones that declared
///   that exact set. With nothing declared, admit only when every candidate was
///   proven about the *same runtime file* — they then describe the same bytes,
///   and which branch reached them changes nothing about what is true of them.
///   Candidates that disagree cannot both be what this project runs, and
///   nothing here can choose, so refuse.
/// - **A declaration** — authoritative, with Node's own selection semantics. A
///   case applies when every condition it was certified under is one this host
///   declares, and the most specific such case wins. Set *equality* would be
///   wrong in both directions: a host declaring `node, import, development`
///   must still match a case certified under `node, import`, and a host
///   declaring `require` must not match one certified under `import` however
///   many declaration files the two branches share.
fn select_case<'a>(
    reaching: &[&'a AuthenticCase],
    declared: &[String],
) -> Option<&'a AuthenticCase> {
    let first = reaching.first()?;
    if declared.is_empty() {
        return reaching
            .iter()
            .all(|case| case.runtime_target == first.runtime_target)
            .then_some(*first);
    }
    let applicable = reaching
        .iter()
        .filter(|case| case.conditions.iter().all(|it| declared.contains(it)))
        .collect::<Vec<_>>();
    let best = applicable.iter().map(|case| case.conditions.len()).max()?;
    let mut most_specific = applicable
        .iter()
        .filter(|case| case.conditions.len() == best);
    match (most_specific.next(), most_specific.next()) {
        (Some(one), None) => Some(**one),
        // Two equally specific cases under different condition sets is not
        // something a declaration can resolve. Refuse.
        _ => None,
    }
}

/// One acceptance that reproduced its signed artifact root against this
/// project's installed bytes.
struct AuthenticCase {
    identity: String,
    /// The runtime file the contract was proven about, package-relative.
    runtime_target: String,
    /// The declaration file paired with it, package-relative. Empty when the
    /// entry names none.
    declaration_target: String,
    conditions: Vec<String>,
}

impl AuthenticCase {
    /// Both sides of the comparison are absolute paths on different machines,
    /// so the package-relative spelling is the only comparable part.
    fn new(
        identity: String,
        import: &crate::artifact_resolution::ResolvedImport,
        conditions: Vec<String>,
    ) -> Option<Self> {
        let relative = |path: &str| -> Option<String> {
            let root = import.package_root.replace('\\', "/");
            path.replace('\\', "/")
                .strip_prefix(root.trim_end_matches('/'))
                .map(|rest| rest.trim_start_matches('/').to_owned())
                .filter(|rest| !rest.is_empty())
        };
        Some(Self {
            identity,
            runtime_target: relative(&import.runtime.path)?,
            declaration_target: relative(&import.declarations.path).unwrap_or_default(),
            conditions,
        })
    }

    /// Whether this project's resolved file is one this case was certified
    /// about. The analyzer resolves TypeScript's answer, which is the
    /// declaration file; the runtime spelling is accepted too, so a host that
    /// resolves the runtime target directly is not excluded.
    fn reaches(&self, target: &str) -> bool {
        self.runtime_target == target || self.declaration_target == target
    }
}

/// What the caller can state about a specifier's *resolved runtime file*,
/// relative to the installed package root. `None` when the project cannot state
/// one exactly — an unresolved import, or two importers that disagree.
pub type ResolvedTargetIdentity<'a> = dyn Fn(&str) -> Option<String> + 'a;

/// The project's local accepted-contract tier: every catalog it holds.
///
/// Discovery has always opened exactly one path,
/// `.solid-checker/accepted-contracts.json`. Certification does not always
/// write that path. When a package resolves to more than one artifact case —
/// which `@solid-primitives/debounce@1.3.0` already does, on its two export
/// conditions — `contract certify` publishes a **case set** instead: a pointer
/// at `.solid-checker/accepted-contract-case-set.json`, a content-addressed
/// case-set document, and one ordinary single-contract catalog per case.
///
/// The producer and the consumer of the same tier therefore disagreed on the
/// filename, and the whole delivery path ended there: a correctly signed,
/// correctly trusted contract sat on disk and nothing ever opened it. Measured
/// on a lockfile-pinned project importing `createDebounce` — certification
/// succeeded, published, and `contract check` still answered "none of the 1
/// exact imported artifact case(s) has a matching receipt" and told the user to
/// start over with `contract generate`.
///
/// Every hop is digest-verified, because a case set is three files rather than
/// one and each is a place to substitute bytes: the pointer names the case-set
/// document's digest, and the document names each case catalog's. Member paths
/// go through [`catalog_member_path`], so a case cannot name `../` out of the
/// case-set directory.
///
/// Both spellings are read, not one or the other, and a plain
/// `accepted-contracts.json` only takes *precedence* — it is first in the
/// returned order, so it wins a conflict over the same import. Treating it as
/// exclusive was a defect with an immediate symptom: certifying a second
/// package writes the plain catalog, which then hid the first package's case
/// set entirely. Measured — `debounce: missing`, `scheduled: certified`, in a
/// project where both had just been certified. A real project has many
/// dependencies, so the exclusive reading loses a contract per certification
/// after the first.
pub fn discovered_catalog_paths(directory: &Path) -> Result<Vec<PathBuf>, ContractFailure> {
    let mut paths = Vec::new();
    let catalog = directory.join(".solid-checker/accepted-contracts.json");
    if catalog.is_file() {
        paths.push(catalog);
    }
    let pointer_path = directory.join(".solid-checker/accepted-contract-case-set.json");
    if !pointer_path.is_file() {
        return Ok(paths);
    }
    let pointer_bytes = read_boundary_file(
        &pointer_path,
        MAX_CATALOG_BYTES,
        "accepted contract case-set pointer",
        false,
    )?;
    let pointer: CaseSetPointerDocument = decode_case_set_json(&pointer_bytes)?;
    if pointer.format != "solid-checker-accepted-contract-case-set-pointer"
        || pointer.case_set_version != 1
    {
        return Err(catalog_field(
            "accepted contract case-set pointer has an unsupported format",
        ));
    }
    let pointer_base = pointer_path
        .parent()
        .ok_or_else(|| catalog_field("accepted contract case-set pointer has no directory"))?;
    let document_path = catalog_member_path(pointer_base, &pointer.document)?;
    let document_bytes = read_boundary_file(
        &document_path,
        MAX_CATALOG_BYTES,
        "accepted contract case set",
        false,
    )?;
    verify_catalog_digest(
        &document_bytes,
        Some(pointer.document_digest.as_str()),
        "caseSetDocumentDigest",
    )?;
    let document: CaseSetDocument = decode_case_set_json(&document_bytes)?;
    if document.format != "solid-checker-accepted-contract-case-set"
        || document.case_set_version != 1
    {
        return Err(catalog_field(
            "accepted contract case set has an unsupported format",
        ));
    }
    let base = document_path
        .parent()
        .ok_or_else(|| catalog_field("accepted contract case set has no directory"))?;
    paths.reserve(document.cases.len());
    for case in &document.cases {
        let path = catalog_member_path(base, &case.catalog)?;
        let bytes =
            read_boundary_file(&path, MAX_CATALOG_BYTES, "accepted contract catalog", false)?;
        verify_catalog_digest(&bytes, Some(case.catalog_digest.as_str()), "catalogDigest")?;
        paths.push(path);
    }
    Ok(paths)
}

fn decode_case_set_json<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
) -> Result<T, ContractFailure> {
    crate::bounded_json::decode(
        bytes,
        crate::bounded_json::Limits {
            bytes: MAX_CATALOG_BYTES,
            depth: MAX_BOUNDARY_DEPTH,
            nodes: MAX_CATALOG_NODES,
            string_bytes: MAX_BOUNDARY_STRING_BYTES,
        },
    )
    .map_err(|message| ContractFailure::DocumentDecode { message })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaseSetPointerDocument {
    format: String,
    case_set_version: u16,
    document: String,
    document_digest: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaseSetDocument {
    format: String,
    case_set_version: u16,
    #[serde(default)]
    cases: Vec<CaseSetCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaseSetCase {
    catalog: String,
    catalog_digest: String,
}

fn catalog_field(message: impl Into<String>) -> ContractFailure {
    ContractFailure::DocumentDecode {
        message: message.into(),
    }
}

fn verify_catalog_digest(
    bytes: &[u8],
    expected: Option<&str>,
    field: &'static str,
) -> Result<(), ContractFailure> {
    let expected =
        expected.ok_or_else(|| catalog_field(format!("policy-2 entry has no {field}")))?;
    if sha256_digest(bytes) != expected {
        return Err(ContractFailure::ReceiptMismatch { field });
    }
    Ok(())
}

/// Returns external documents and receipts referenced by the catalog so a
/// retained analyzer cache cannot survive a content-object replacement.
/// Core objects are not ordinary-analysis inputs and are not opened or watched.
pub fn accepted_contract_catalog_members(path: &Path) -> Result<Vec<PathBuf>, ContractFailure> {
    let (catalog, base) = decode_accepted_contract_catalog(path)?;
    let mut paths = Vec::with_capacity(catalog.contracts.len());
    for entry in catalog.contracts {
        if solid_dialect::core_runtime_contract_reference(
            &entry.import.package_name,
            &entry.import.specifier,
        ) {
            continue;
        }
        paths.push(catalog_member_path(&base, &entry.document)?);
        if let Some(receipt) = entry.receipt {
            paths.push(catalog_member_path(&base, &receipt)?);
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn decode_accepted_contract_catalog(
    path: &Path,
) -> Result<(AcceptedCatalogDocument, PathBuf), ContractFailure> {
    let bytes = read_boundary_file(path, MAX_CATALOG_BYTES, "accepted contract catalog", false)?;
    let catalog: AcceptedCatalogDocument = crate::bounded_json::decode(
        &bytes,
        crate::bounded_json::Limits {
            bytes: MAX_CATALOG_BYTES,
            depth: MAX_BOUNDARY_DEPTH,
            nodes: MAX_CATALOG_NODES,
            string_bytes: MAX_BOUNDARY_STRING_BYTES,
        },
    )
    .map_err(|message| ContractFailure::DocumentDecode {
        message: format!(
            "decode accepted contract catalog {}: {message}",
            path.display()
        ),
    })?;
    if catalog.format != ACCEPTED_CATALOG_FORMAT
        || catalog.catalog_version != ACCEPTED_CATALOG_VERSION
    {
        return Err(ContractFailure::DocumentDecode {
            message: format!(
                "accepted contract catalog must use format {ACCEPTED_CATALOG_FORMAT:?} version {ACCEPTED_CATALOG_VERSION}"
            ),
        });
    }
    if catalog.contracts.len() > MAX_CATALOG_CONTRACTS {
        return Err(ContractFailure::DocumentDecode {
            message: format!(
                "accepted contract catalog exceeds the {MAX_CATALOG_CONTRACTS} contract resource limit"
            ),
        });
    }
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let base = if directory
        .file_name()
        .is_some_and(|name| name == ".solid-checker")
    {
        directory.parent().unwrap_or(directory)
    } else {
        directory
    };
    Ok((catalog, base.to_path_buf()))
}

fn read_boundary_file(
    path: &Path,
    limit: usize,
    label: &str,
    receipt: bool,
) -> Result<Vec<u8>, ContractFailure> {
    let failure = |message: String| {
        if receipt {
            ContractFailure::ReceiptDecode { message }
        } else {
            ContractFailure::DocumentDecode { message }
        }
    };
    let metadata = fs::metadata(path)
        .map_err(|error| failure(format!("read {label} {}: {error}", path.display())))?;
    if metadata.len() > u64::try_from(limit).unwrap_or(u64::MAX) {
        return Err(failure(format!(
            "{label} {} exceeds the {limit}-byte resource limit",
            path.display()
        )));
    }
    fs::read(path).map_err(|error| failure(format!("read {label} {}: {error}", path.display())))
}

fn rebase_catalog_import(base: &Path, import: &mut ResolvedImport) -> Result<(), ContractFailure> {
    import.importer = catalog_absolute_path(base, &import.importer)?;
    import.package_root = catalog_absolute_path(base, &import.package_root)?;
    if let Some(real_root) = &mut import.package_real_root {
        *real_root = catalog_absolute_path(base, real_root)?;
    }
    rebase_catalog_file(base, &mut import.package_manifest)?;
    rebase_catalog_file(base, &mut import.runtime)?;
    rebase_catalog_file(base, &mut import.declarations)?;
    if let Some(transform) = &mut import.transform {
        rebase_catalog_file(base, transform)?;
    }
    for binding in import.exports.values_mut() {
        rebase_catalog_file(base, &mut binding.runtime.module)?;
        rebase_catalog_file(base, &mut binding.declarations.module)?;
    }
    Ok(())
}

fn rebase_catalog_file(base: &Path, file: &mut ResolvedFile) -> Result<(), ContractFailure> {
    file.path = catalog_absolute_path(base, &file.path)?;
    if let Some(real_path) = &mut file.real_path {
        *real_path = catalog_absolute_path(base, real_path)?;
    }
    Ok(())
}

fn catalog_absolute_path(base: &Path, value: &str) -> Result<String, ContractFailure> {
    let path = Path::new(value);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        catalog_member_path(base, value)?
    };
    path.canonicalize()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| ContractFailure::DocumentDecode {
            message: format!("accepted contract catalog path {}: {error}", path.display()),
        })
}

fn catalog_member_path(base: &Path, member: &str) -> Result<PathBuf, ContractFailure> {
    let member = Path::new(member);
    let spelling = member.to_string_lossy();
    let windows_absolute = spelling.as_bytes().get(1) == Some(&b':')
        && spelling
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
    if member.as_os_str().is_empty()
        || member.is_absolute()
        || spelling.starts_with('\\')
        || windows_absolute
        || spelling
            .split(['/', '\\'])
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        || member.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
    {
        return Err(ContractFailure::DocumentDecode {
            message: "accepted contract catalog contains an invalid member path".into(),
        });
    }
    Ok(base.join(member))
}

/// Loads one accepted contract for an already resolved import. Temporary-v2
/// wire mechanics terminate here; only receipt-validated normalized semantics
/// can cross into analyzer queries.
pub fn load_accepted_contract(
    document_bytes: &[u8],
    receipt: &[u8],
    import: &ResolvedImport,
) -> Result<AcceptedContract, ContractFailure> {
    contract_document::decode(document_bytes)?;
    require_policy2_receipt(receipt)?;
    let _ = import;
    Err(ContractFailure::ReceiptAuthenticationRequired)
}

/// Loads one policy-2 contract after authenticating its exact signed binding
/// set and issuer provenance. Ordinary discovery cannot infer either input and
/// therefore uses [`load_accepted_contract`] only to report a typed refusal.
pub fn load_authenticated_policy2_contract(
    document_bytes: &[u8],
    receipt_bytes: &[u8],
    import: &ResolvedImport,
    expected: &Policy2ReceiptBindings,
    provenance: Policy2ReceiptProvenance<'_>,
) -> Result<AcceptedContract, ContractFailure> {
    let canonical_main = canonicalize_policy2_main(document_bytes).map_err(authentication_error)?;
    if canonical_main != document_bytes {
        return Err(ContractFailure::ReceiptMismatch {
            field: "mainDigest",
        });
    }
    let authenticated =
        authenticate_policy2_receipt(&canonical_main, receipt_bytes, expected, provenance)
            .map_err(authentication_error)?;
    let actual_import_root = policy2_resolved_import_root(import).map_err(authentication_error)?;
    if actual_import_root != expected.resolved_import_root {
        return Err(ContractFailure::ReceiptMismatch {
            field: "resolvedImportRoot",
        });
    }
    let normalized = contract_document::decode(&canonical_main)?.normalize()?;
    // Replay every selected artifact, trace, closure, and export target. The
    // rebound object is validation-only because per-export targets are signed
    // through `resolvedImportRoot` and deliberately are not stable-v1 fields.
    let external_targets = crate::artifact_resolution::resolved_external_export_targets(import)?;
    if !external_targets.is_empty() && import.closure.dependencies.is_empty() {
        return Err(ContractFailure::IdentityMismatch {
            reason: "an external export target has no receipt-bound dependency edge".into(),
        });
    }
    let _rebound = crate::artifact_resolution::select_and_bind_with_external_targets(
        &normalized,
        import,
        &external_targets,
    )?;
    accept_policy2_selected(normalized, authenticated)
}

/// Loads an immutable compiled single-case bundle. Its receipt becomes
/// authoritative only when its independently compiled entry digest matches.
#[doc(hidden)]
pub fn load_authenticated_policy2_embedded_contract(
    document_bytes: &[u8],
    receipt_bytes: &[u8],
    expected: &Policy2ReceiptBindings,
    entry: &BuiltInReceiptEntry,
) -> Result<AcceptedContract, ContractFailure> {
    let canonical_main = canonicalize_policy2_main(document_bytes).map_err(authentication_error)?;
    if canonical_main != document_bytes {
        return Err(ContractFailure::ReceiptMismatch {
            field: "mainDigest",
        });
    }
    let authenticated = authenticate_policy2_receipt(
        &canonical_main,
        receipt_bytes,
        expected,
        Policy2ReceiptProvenance::BuiltIn(entry),
    )
    .map_err(authentication_error)?;
    let normalized = contract_document::decode(&canonical_main)?.normalize()?;
    if normalized.artifact_cases().len() != 1 {
        return Err(ContractFailure::MultipleArtifactCases);
    }
    accept_policy2_selected(normalized, authenticated)
}

fn accept_policy2_selected(
    selected: solid_reactive_ir::contract_semantics::NormalizedContract,
    authenticated: AuthenticatedPolicy2Receipt,
) -> Result<AcceptedContract, ContractFailure> {
    let selected_case = selected
        .artifact_cases()
        .first()
        .expect("policy-2 selection retains exactly one artifact case")
        .id
        .clone();
    accept_authenticated_policy2(
        selected,
        &selected_case,
        AuthenticatedPolicy2Acceptance {
            main_digest: solid_reactive_ir::contract_semantics::Digest::parse(
                authenticated.main_digest(),
            )
            .expect("authenticated main digest is canonical"),
            semantic_digest: authenticated.semantic_digest().clone(),
            receipt_digest: solid_reactive_ir::contract_semantics::Digest::parse(
                authenticated.receipt_digest(),
            )
            .expect("authenticated receipt digest is canonical"),
            policy_digest: authenticated.policy_digest().clone(),
            closed_claims_root: authenticated.closed_claims_root().clone(),
            verifier_build_digest: authenticated.verifier_build_digest().clone(),
            trust_store_digest: solid_reactive_ir::contract_semantics::Digest::parse(
                authenticated.trust_store_digest(),
            )
            .expect("authenticated trust-store digest is canonical"),
            revocation_epoch: authenticated.revocation_epoch(),
        },
    )
    .map_err(|error| ContractFailure::ReceiptAuthentication {
        message: error.to_string(),
    })
}

fn authentication_error(error: impl std::fmt::Display) -> ContractFailure {
    ContractFailure::ReceiptAuthentication {
        message: error.to_string(),
    }
}

/// Validates a compile-time embedded, already single-case bundle and its
/// receipt. The caller remains responsible for independently proving that the
/// installed package census and selected artifacts match this checked bundle;
/// ordinary host documents must use [`load_accepted_contract`] instead.
pub(crate) fn load_receipt_issued_embedded_contract(
    document_bytes: &[u8],
    receipt_bytes: &[u8],
) -> Result<AcceptedContract, ContractFailure> {
    contract_document::decode(document_bytes)?;
    require_policy2_receipt(receipt_bytes)?;
    Err(ContractFailure::ReceiptAuthenticationRequired)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptVersionProbe {
    receipt_version: u16,
}

fn require_policy2_receipt(receipt_bytes: &[u8]) -> Result<(), ContractFailure> {
    let receipt: ReceiptVersionProbe = crate::bounded_json::decode(
        receipt_bytes,
        crate::bounded_json::Limits {
            bytes: MAX_RECEIPT_BYTES,
            depth: MAX_BOUNDARY_DEPTH,
            nodes: MAX_RECEIPT_NODES,
            string_bytes: MAX_BOUNDARY_STRING_BYTES,
        },
    )
    .map_err(|message| ContractFailure::ReceiptDecode { message })?;
    if receipt.receipt_version != 2 {
        return Err(ContractFailure::UnsupportedReceiptVersion {
            expected: 2,
            actual: receipt.receipt_version,
        });
    }
    Ok(())
}

/// Loads the complete analyzer-facing index. The exact importer/specifier pair
/// is retained so nested installations cannot alias each other, and duplicate
/// answers are refused before any consumer can query them.
pub fn load_accepted_contract_index<'a>(
    sources: impl IntoIterator<Item = AcceptedContractSource<'a>>,
) -> Result<AcceptedContractIndex, ContractFailure> {
    let mut inputs = Vec::new();
    for source in sources {
        inputs.push(AcceptedContractInput {
            importer: source.import.importer.clone(),
            specifier: source.import.specifier.clone(),
            contract: load_accepted_contract(source.document, source.receipt, source.import)?,
            artifact_identity: None,
        });
    }
    AcceptedContractIndex::new(inputs).map_err(|error| ContractFailure::IdentityMismatch {
        reason: error.to_string(),
    })
}

/// Host/WASM counterpart of external-only native catalog discovery.
pub fn load_external_contract_index<'a>(
    sources: impl IntoIterator<Item = AcceptedContractSource<'a>>,
) -> Result<AcceptedContractIndex, ContractFailure> {
    load_accepted_contract_index(sources.into_iter().filter(|source| {
        !solid_dialect::core_runtime_contract_reference(
            &source.import.package_name,
            &source.import.specifier,
        )
    }))
}

pub(crate) fn invalid_identity(reason: impl Into<String>) -> ContractFailure {
    ContractFailure::IdentityMismatch {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds the three files `contract certify` publishes for a package with
    /// more than one artifact case, and returns the project directory.
    fn published_case_set(label: &str, cases: usize) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-case-set-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let catalog_root = root.join(".solid-checker");
        let mut entries = Vec::new();
        let mut case_dirs = Vec::new();
        for index in 0..cases {
            let catalog = serde_json::json!({
                "format": "solid-checker-accepted-contract-catalog",
                "catalogVersion": 2,
                "contracts": [],
                "case": index,
            });
            let bytes = serde_json::to_vec(&catalog).unwrap();
            let digest = sha256_digest(&bytes);
            let name = digest.trim_start_matches("sha256:").to_owned();
            case_dirs.push((name.clone(), bytes.clone()));
            entries.push(serde_json::json!({
                "catalog": format!("cases/{name}/accepted-contracts.json"),
                "catalogDigest": digest,
            }));
        }
        let document = serde_json::to_vec(&serde_json::json!({
            "format": "solid-checker-accepted-contract-case-set",
            "caseSetVersion": 1,
            "cases": entries,
        }))
        .unwrap();
        let document_digest = sha256_digest(&document);
        let key = document_digest.trim_start_matches("sha256:").to_owned();
        let case_set_dir = catalog_root.join("case-sets").join(&key);
        for (name, bytes) in case_dirs {
            let dir = case_set_dir.join("cases").join(name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("accepted-contracts.json"), bytes).unwrap();
        }
        fs::create_dir_all(&case_set_dir).unwrap();
        fs::write(
            case_set_dir.join("accepted-contract-case-set.json"),
            &document,
        )
        .unwrap();
        fs::write(
            catalog_root.join("accepted-contract-case-set.json"),
            serde_json::to_vec(&serde_json::json!({
                "format": "solid-checker-accepted-contract-case-set-pointer",
                "caseSetVersion": 1,
                "document": format!("case-sets/{key}/accepted-contract-case-set.json"),
                "documentDigest": document_digest,
            }))
            .unwrap(),
        )
        .unwrap();
        root
    }

    /// The gap this closes: certification publishes a case set, discovery only
    /// ever opened `accepted-contracts.json`, and a correctly signed contract
    /// was therefore written and never read.
    #[test]
    fn discovery_opens_the_case_set_certification_actually_publishes() {
        let project = published_case_set("published", 2);
        let found = discovered_catalog_paths(&project).expect("the case set resolves");
        assert_eq!(
            found.len(),
            2,
            "both case catalogs are discovered: {found:?}"
        );
        for path in &found {
            assert!(path.is_file(), "{path:?} is a real catalog");
        }
        let _ = fs::remove_dir_all(&project);
    }

    /// A project with neither spelling discovers nothing, rather than erroring.
    #[test]
    fn discovery_of_a_project_with_no_local_tier_is_empty() {
        let project = std::env::temp_dir().join(format!(
            "solid-checker-case-set-absent-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&project);
        fs::create_dir_all(&project).unwrap();
        assert!(
            discovered_catalog_paths(&project).unwrap().is_empty(),
            "no local tier discovers nothing"
        );
        let _ = fs::remove_dir_all(&project);
    }

    fn case(identity: &str, runtime: &str, conditions: &[&str]) -> AuthenticCase {
        AuthenticCase {
            identity: identity.to_owned(),
            runtime_target: runtime.to_owned(),
            declaration_target: "dist/index.d.ts".to_owned(),
            conditions: conditions.iter().map(|it| (*it).to_owned()).collect(),
        }
    }

    fn selected(cases: &[AuthenticCase], declared: &[&str]) -> Option<String> {
        let reaching = cases.iter().collect::<Vec<_>>();
        let declared = declared
            .iter()
            .map(|it| (*it).to_owned())
            .collect::<Vec<_>>();
        select_case(&reaching, &declared).map(|case| case.identity.clone())
    }

    /// The defect this replaced: `artifactAcceptanceRoot` is a digest over a
    /// condition set the catalog never recorded, so a consumer could only guess
    /// it, and the guess was the constant `["import"]`. A project declaring its
    /// real conditions was refused; one declaring that exact set was admitted.
    /// Declaring honestly broke it.
    ///
    /// Measured against `@solid-primitives/debounce@1.3.0`, whose two certified
    /// cases reach the same `dist/index.js` through `/exports/./import` and
    /// `/exports/./node/import`.
    #[test]
    fn a_declaration_selects_by_node_condition_semantics() {
        let cases = [
            case("root-import", "dist/index.js", &["import"]),
            case("root-node", "dist/index.js", &["node", "import"]),
        ];
        // A superset of a case's conditions selects it, and the most specific
        // applicable case wins.
        assert_eq!(
            selected(&cases, &["import"]).as_deref(),
            Some("root-import")
        );
        assert_eq!(
            selected(&cases, &["node", "import"]).as_deref(),
            Some("root-node"),
            "the most specific applicable case wins, not the first"
        );
        assert_eq!(
            selected(&cases, &["browser", "import", "development"]).as_deref(),
            Some("root-import"),
            "a host declaring more than a case needs still matches it"
        );
        // A host whose conditions contain none of a case's is not that case.
        assert_eq!(selected(&cases, &["require"]), None);
        assert_eq!(selected(&cases, &["solid"]), None);
    }

    /// The linter case. ESLint and Oxlint hosts do not know their export
    /// conditions, so requiring a declaration would make delivery a no-op for
    /// them -- silently, which is the worst failure mode a linter can have.
    #[test]
    fn no_declaration_admits_only_an_unambiguous_artifact() {
        let same = [
            case("root-import", "dist/index.js", &["import"]),
            case("root-node", "dist/index.js", &["node", "import"]),
        ];
        assert_eq!(
            selected(&same, &[]).as_deref(),
            Some("root-import"),
            "candidates proven about the same runtime file describe the same bytes"
        );
        // One `.d.ts` shared by branches that run *different* files is exactly
        // where a guess would be unsound, and there is nothing here to choose
        // with.
        let differing = [
            case("root-import", "dist/index.js", &["import"]),
            case("root-require", "dist/index.cjs", &["require"]),
        ];
        assert_eq!(
            selected(&differing, &[]),
            None,
            "candidates that disagree about the runtime file must refuse"
        );
        // With a declaration the same pair is decidable.
        assert_eq!(
            selected(&differing, &["require"]).as_deref(),
            Some("root-require")
        );
    }

    /// The shape a Solid app with SSR actually has: one `.d.ts`, two runtime
    /// files, and *both* of them real — the server bundle runs one and the
    /// browser bundle the other. There is no single answer for such a project,
    /// which is why the environment is declared per analysis run rather than
    /// derived, and why declaring nothing has to refuse instead of picking.
    #[test]
    fn an_ssr_package_is_selected_by_the_declared_environment() {
        let cases = [
            case("server", "dist/server.js", &["node", "import"]),
            case("client", "dist/index.js", &["browser", "import"]),
        ];
        assert_eq!(
            selected(&cases, &["import", "node"]).as_deref(),
            Some("server"),
            "the server pass selects the artifact the server actually runs"
        );
        assert_eq!(
            selected(&cases, &["browser", "import"]).as_deref(),
            Some("client")
        );
        // `--runtime-target node --rendering string-ssr` folds to this.
        assert_eq!(
            selected(&cases, &["import", "node", "string-ssr"]).as_deref(),
            Some("server"),
            "a host declaring more than the case needs still matches it"
        );
        assert_eq!(
            selected(&cases, &[]),
            None,
            "two real artifacts and no declaration is not something to guess at"
        );
    }

    /// Nothing resolved, or nothing certified about what was resolved.
    #[test]
    fn no_candidate_admits_nothing() {
        assert_eq!(selected(&[], &[]), None);
        assert_eq!(selected(&[], &["import"]), None);
    }

    /// Both spellings are read. The older one only takes *precedence*.
    ///
    /// Exclusivity was a defect with an immediate symptom: `contract certify`
    /// writes the plain catalog for a single-case package and a case set for a
    /// multi-case one, so certifying a second dependency hid the first.
    #[test]
    fn a_plain_catalog_takes_precedence_without_hiding_a_case_set() {
        let project = published_case_set("precedence", 2);
        let plain = project.join(".solid-checker/accepted-contracts.json");
        fs::write(&plain, b"{\"contracts\":[]}").unwrap();
        let found = discovered_catalog_paths(&project).unwrap();
        assert_eq!(
            found.len(),
            3,
            "the plain catalog and both cases: {found:?}"
        );
        assert_eq!(found[0], plain, "the plain catalog is consulted first");
        let _ = fs::remove_dir_all(&project);
    }

    /// Both digest-verified hops refuse a substitution, and neither refusal is
    /// silent.
    ///
    /// The pointer itself is deliberately *not* on this list: nothing above it
    /// names its digest, because it is the root of the local tier. Its
    /// authority comes from the receipt each catalog carries, not from a hash
    /// chain that would have to terminate in the same directory an attacker
    /// already wrote to.
    #[test]
    fn every_digest_bound_case_set_hop_refuses_a_substitution() {
        // The case-set document, named by the pointer's `documentDigest`.
        let project = published_case_set("tamper-document", 1);
        let pointer: serde_json::Value = serde_json::from_slice(
            &fs::read(project.join(".solid-checker/accepted-contract-case-set.json")).unwrap(),
        )
        .unwrap();
        let document = project
            .join(".solid-checker")
            .join(pointer["document"].as_str().unwrap());
        let mut body: serde_json::Value =
            serde_json::from_slice(&fs::read(&document).unwrap()).unwrap();
        body["tampered"] = serde_json::json!(true);
        fs::write(&document, serde_json::to_vec(&body).unwrap()).unwrap();
        assert!(
            matches!(
                discovered_catalog_paths(&project),
                Err(ContractFailure::ReceiptMismatch {
                    field: "caseSetDocumentDigest"
                })
            ),
            "a substituted case-set document must refuse"
        );
        let _ = fs::remove_dir_all(&project);

        // A case catalog, named by the document's `catalogDigest`.
        let project = published_case_set("tamper-catalog", 1);
        let catalog = discovered_catalog_paths(&project).unwrap()[0].clone();
        let mut body: serde_json::Value =
            serde_json::from_slice(&fs::read(&catalog).unwrap()).unwrap();
        body["tampered"] = serde_json::json!(true);
        fs::write(&catalog, serde_json::to_vec(&body).unwrap()).unwrap();
        assert!(
            matches!(
                discovered_catalog_paths(&project),
                Err(ContractFailure::ReceiptMismatch {
                    field: "catalogDigest"
                })
            ),
            "a substituted case catalog must refuse"
        );
        let _ = fs::remove_dir_all(&project);
    }

    /// A case may not name its way out of the case-set directory.
    #[test]
    fn a_case_cannot_escape_the_case_set_directory() {
        let project = published_case_set("escape", 1);
        let pointer = project.join(".solid-checker/accepted-contract-case-set.json");
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&pointer).unwrap()).unwrap();
        document["document"] = serde_json::json!("../../../etc/passwd");
        fs::write(&pointer, serde_json::to_vec(&document).unwrap()).unwrap();
        assert!(
            discovered_catalog_paths(&project).is_err(),
            "a traversing member path must refuse"
        );
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn host_core_contract_payloads_are_withheld_without_decoding() {
        let catalog: serde_json::Value = serde_json::from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/reactive-ir/package-return-consumer/.solid-checker/accepted-contracts.json"
        ))).unwrap();
        let mut import: ResolvedImport =
            serde_json::from_value(catalog["contracts"][0]["import"].clone()).unwrap();
        import.specifier = "aliased-core".into();
        for package in ["solid-js", "@solidjs/signals", "@solidjs/web"] {
            import.package_name = package.into();
            let index = load_external_contract_index([AcceptedContractSource {
                document: b"not JSON",
                receipt: b"not a receipt",
                import: &import,
            }])
            .unwrap();
            assert!(index.semantic_identity().is_empty());
        }
        import.package_name = "@solidjs/web-extra".into();
        assert!(
            load_external_contract_index([AcceptedContractSource {
                document: b"not JSON",
                receipt: b"not a receipt",
                import: &import,
            }])
            .is_err()
        );
    }

    #[test]
    fn policy1_receipts_are_obsolete_at_the_active_boundary() {
        assert!(matches!(
            require_policy2_receipt(br#"{"receiptVersion":1}"#),
            Err(ContractFailure::UnsupportedReceiptVersion {
                expected: 2,
                actual: 1
            })
        ));
    }

    #[test]
    fn policy2_receipts_require_authenticated_provenance() {
        assert!(require_policy2_receipt(br#"{"receiptVersion":2}"#).is_ok());
    }

    #[test]
    fn one_document_digest_normalizes_once_for_every_catalog_entry() {
        const DOCUMENT: &[u8] = br#"{"format":"solid-reactivity-contract","schemaVersion":1,"semanticModelVersion":1,"package":{"name":"solid-js","version":"2.0.0-rc.3","integrity":"sha512:test","manifest":{"path":"package.json","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}},"summaries":{"plain":{"shape":"plain"}},"entrypoints":{".":{"artifact":{"path":"dist/solid.js","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","closureSha256":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"},"declarations":{"path":"types/index.d.ts","sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"},"exports":{"version":"plain"}}},"sidecars":{}}"#;
        let digest = sha256_digest(DOCUMENT);
        let mut memo = BTreeMap::new();
        let first = normalized_catalog_document(&mut memo, &digest, DOCUMENT)
            .unwrap()
            .clone();
        assert_eq!(memo.len(), 1);
        // Two catalog entries naming one document object -- the shape every
        // multi-module re-export now produces.
        let second = normalized_catalog_document(&mut memo, &digest, DOCUMENT)
            .unwrap()
            .clone();
        assert_eq!(
            memo.len(),
            1,
            "a repeated documentDigest must not decode and normalize again"
        );
        assert_eq!(
            first, second,
            "both entries bind the same normalized document"
        );
    }
}
