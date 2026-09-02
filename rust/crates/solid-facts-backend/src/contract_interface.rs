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
        let normalized = contract_document::decode(&document)?.normalize()?;
        let external_targets =
            crate::artifact_resolution::resolved_external_export_targets(&entry.import)?;
        let selected = crate::artifact_resolution::select_and_bind_with_external_targets(
            &normalized,
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
    let (catalog, base) = decode_accepted_contract_catalog(path)?;
    let mut uncertifiable = Vec::with_capacity(catalog.contracts.len());
    let mut accepted = Vec::new();
    for mut entry in catalog.contracts {
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

/// Returns every exact document and receipt referenced by the catalog so a
/// retained analyzer cache cannot survive a content-object replacement.
pub fn accepted_contract_catalog_members(path: &Path) -> Result<Vec<PathBuf>, ContractFailure> {
    let (catalog, base) = decode_accepted_contract_catalog(path)?;
    let mut paths = Vec::with_capacity(catalog.contracts.len());
    for entry in catalog.contracts {
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
        });
    }
    AcceptedContractIndex::new(inputs).map_err(|error| ContractFailure::IdentityMismatch {
        reason: error.to_string(),
    })
}

pub(crate) fn invalid_identity(reason: impl Into<String>) -> ContractFailure {
    ContractFailure::IdentityMismatch {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
