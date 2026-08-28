//! The only package-contract boundary exposed to analyzer callers.
//!
//! Schema spellings and compact document mechanics remain private. Temporary
//! schema-v2 documents are decoded and normalized by the sibling deep module;
//! this analyzer-loading interface still refuses exposure until Phase 12 can
//! validate proof-issued receipts and migrate consumers atomically.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{
    AcceptanceReceipt, AcceptedContract, AcceptedContractIndex, AcceptedContractInput, Digest,
    VerifierIdentity, proof::validate_receipt_and_accept,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

use crate::{artifact_resolution::select_and_bind, contract_document_v2};

const MAX_CONTRACT_DOCUMENT_BYTES: usize = 1024 * 1024;
const MAX_RECEIPT_BYTES: usize = 64 * 1024;
const MAX_CATALOG_BYTES: usize = 16 * 1024 * 1024;
const MAX_CATALOG_CONTRACTS: usize = 65_536;
const MAX_BOUNDARY_DEPTH: usize = 128;
const MAX_BOUNDARY_STRING_BYTES: usize = 16 * 1024;
const MAX_RECEIPT_NODES: usize = 4_096;
const MAX_CATALOG_NODES: usize = 1_000_000;

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireReceipt {
    receipt_version: u16,
    wire_digest: String,
    semantic_model_version: u16,
    semantic_digest: String,
    artifacts_digest: String,
    closure_digest: String,
    proof_root: String,
    closed_claims_root: String,
    verifier: WireVerifier,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireVerifier {
    build: String,
    policy: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptDocument<'a> {
    receipt_version: u16,
    wire_digest: &'a str,
    semantic_model_version: u16,
    semantic_digest: &'a str,
    artifacts_digest: &'a str,
    closure_digest: &'a str,
    proof_root: &'a str,
    closed_claims_root: &'a str,
    verifier: ReceiptVerifier<'a>,
}

#[derive(Serialize)]
struct ReceiptVerifier<'a> {
    build: &'a str,
    policy: u32,
}

pub fn encode_acceptance_receipt(receipt: &AcceptanceReceipt) -> Result<Vec<u8>, ContractFailure> {
    let mut bytes = serde_json::to_vec(&ReceiptDocument {
        receipt_version: receipt.receipt_version,
        wire_digest: receipt.wire_digest.as_str(),
        semantic_model_version: receipt.semantic_model_version,
        semantic_digest: receipt.semantic_digest.as_str(),
        artifacts_digest: receipt.artifacts_digest.as_str(),
        closure_digest: receipt.closure_digest.as_str(),
        proof_root: receipt.proof_root.as_str(),
        closed_claims_root: receipt.closed_claims_root.as_str(),
        verifier: ReceiptVerifier {
            build: &receipt.verifier.build,
            policy: receipt.verifier.policy,
        },
    })
    .map_err(|error| ContractFailure::ReceiptDecode {
        message: error.to_string(),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
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
const ACCEPTED_CATALOG_VERSION: u16 = 1;

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
    receipt: String,
    import: ResolvedImport,
}

/// Reads an explicit host-acquisition catalog and terminates every file and
/// wire-format concept at this boundary. Paths are relative to the catalog;
/// analyzer consumers receive only the accepted semantic index.
pub fn read_accepted_contract_catalog(
    path: &Path,
) -> Result<AcceptedContractIndex, ContractFailure> {
    let (catalog, base) = decode_accepted_contract_catalog(path)?;
    let mut loaded = Vec::with_capacity(catalog.contracts.len());
    for mut entry in catalog.contracts {
        let document_path = catalog_member_path(&base, &entry.document)?;
        let receipt_path = catalog_member_path(&base, &entry.receipt)?;
        let document = read_boundary_file(
            &document_path,
            MAX_CONTRACT_DOCUMENT_BYTES,
            "contract",
            false,
        )?;
        let receipt = read_boundary_file(&receipt_path, MAX_RECEIPT_BYTES, "receipt", true)?;
        rebase_catalog_import(&base, &mut entry.import)?;
        loaded.push((document, receipt, entry.import));
    }
    load_accepted_contract_index(loaded.iter().map(|(document, receipt, import)| {
        AcceptedContractSource {
            document,
            receipt,
            import,
        }
    }))
}

/// Returns the exact main documents and receipts referenced by an accepted
/// catalog. Retained hosts hash these paths together with the catalog so an
/// in-place proof or semantic edit cannot reuse an older accepted analysis.
pub fn accepted_contract_catalog_members(path: &Path) -> Result<Vec<PathBuf>, ContractFailure> {
    let (catalog, base) = decode_accepted_contract_catalog(path)?;
    let mut paths = Vec::with_capacity(catalog.contracts.len() * 2);
    for entry in catalog.contracts {
        paths.push(catalog_member_path(&base, &entry.document)?);
        paths.push(catalog_member_path(&base, &entry.receipt)?);
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
    let document = contract_document_v2::decode(document_bytes)?;
    let receipt =
        decode_and_validate_receipt(document_bytes, document.semantic_model_version(), receipt)?;

    let normalized = document.normalize()?;
    let selected = select_and_bind(&normalized, import)?;
    accept_selected(selected, receipt)
}

/// Validates a compile-time embedded, already single-case bundle and its
/// receipt. The caller remains responsible for independently proving that the
/// installed package census and selected artifacts match this checked bundle;
/// ordinary host documents must use [`load_accepted_contract`] instead.
pub(crate) fn load_receipt_issued_embedded_contract(
    document_bytes: &[u8],
    receipt_bytes: &[u8],
) -> Result<AcceptedContract, ContractFailure> {
    let document = contract_document_v2::decode(document_bytes)?;
    let receipt = decode_and_validate_receipt(
        document_bytes,
        document.semantic_model_version(),
        receipt_bytes,
    )?;
    let normalized = document.normalize()?;
    if normalized.artifact_cases().len() != 1 {
        return Err(ContractFailure::MultipleArtifactCases);
    }
    accept_selected(normalized, receipt)
}

fn decode_and_validate_receipt(
    document_bytes: &[u8],
    semantic_model_version: u16,
    receipt_bytes: &[u8],
) -> Result<AcceptanceReceipt, ContractFailure> {
    let receipt: WireReceipt = crate::bounded_json::decode(
        receipt_bytes,
        crate::bounded_json::Limits {
            bytes: MAX_RECEIPT_BYTES,
            depth: MAX_BOUNDARY_DEPTH,
            nodes: MAX_RECEIPT_NODES,
            string_bytes: MAX_BOUNDARY_STRING_BYTES,
        },
    )
    .map_err(|message| ContractFailure::ReceiptDecode { message })?;

    if receipt.receipt_version != 1 {
        return Err(ContractFailure::UnsupportedReceiptVersion {
            expected: 1,
            actual: receipt.receipt_version,
        });
    }
    if receipt.wire_digest != sha256_digest(document_bytes) {
        return Err(ContractFailure::ReceiptMismatch {
            field: "wireDigest",
        });
    }
    if receipt.semantic_model_version != semantic_model_version {
        return Err(ContractFailure::ReceiptMismatch {
            field: "semanticModelVersion",
        });
    }
    for (field, digest) in [
        ("semanticDigest", &receipt.semantic_digest),
        ("artifactsDigest", &receipt.artifacts_digest),
        ("closureDigest", &receipt.closure_digest),
        ("proofRoot", &receipt.proof_root),
        ("closedClaimsRoot", &receipt.closed_claims_root),
    ] {
        if !is_sha256_digest(digest) {
            return Err(ContractFailure::ReceiptMismatch { field });
        }
    }

    Ok(AcceptanceReceipt {
        receipt_version: receipt.receipt_version,
        wire_digest: parse_receipt_digest(receipt.wire_digest, "wireDigest")?,
        semantic_model_version: receipt.semantic_model_version,
        semantic_digest: parse_receipt_digest(receipt.semantic_digest, "semanticDigest")?,
        artifacts_digest: parse_receipt_digest(receipt.artifacts_digest, "artifactsDigest")?,
        closure_digest: parse_receipt_digest(receipt.closure_digest, "closureDigest")?,
        proof_root: parse_receipt_digest(receipt.proof_root, "proofRoot")?,
        closed_claims_root: parse_receipt_digest(receipt.closed_claims_root, "closedClaimsRoot")?,
        verifier: VerifierIdentity {
            build: receipt.verifier.build,
            policy: receipt.verifier.policy,
        },
    })
}

fn accept_selected(
    selected: solid_reactive_ir::contract_semantics::NormalizedContract,
    receipt: AcceptanceReceipt,
) -> Result<AcceptedContract, ContractFailure> {
    let selected_case = selected
        .artifact_cases()
        .first()
        .expect("artifact selection returns exactly one case")
        .id
        .clone();
    validate_receipt_and_accept(selected, &selected_case, receipt).map_err(|error| {
        use solid_reactive_ir::contract_semantics::proof::ReceiptValidationError;
        match error {
            ReceiptValidationError::ReceiptVersion { expected, actual } => {
                ContractFailure::UnsupportedReceiptVersion { expected, actual }
            }
            ReceiptValidationError::Mismatch { field } => {
                ContractFailure::ReceiptMismatch { field }
            }
            other => ContractFailure::ReceiptMismatch {
                field: match other {
                    ReceiptValidationError::EmptyVerifierBuild => "verifier.build",
                    ReceiptValidationError::ProofPolicy { .. } => "verifier.policy",
                    ReceiptValidationError::MissingArtifactCase { .. } => "selectedArtifactCase",
                    ReceiptValidationError::NoClosedClaims => "closedClaimsRoot",
                    ReceiptValidationError::Claim(_) => "closedClaimsRoot",
                    ReceiptValidationError::ReceiptVersion { .. }
                    | ReceiptValidationError::Mismatch { .. } => unreachable!(),
                },
            },
        }
    })
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

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|payload| {
        payload.len() == 64 && payload.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn parse_receipt_digest(value: String, field: &'static str) -> Result<Digest, ContractFailure> {
    let digest = Digest::parse(&value).map_err(|_| ContractFailure::ReceiptMismatch { field })?;
    if digest.as_str() == value {
        Ok(digest)
    } else {
        Err(ContractFailure::ReceiptMismatch { field })
    }
}

#[cfg(test)]
mod tests {
    use solid_reactive_ir::contract_semantics::{
        ClaimDomain, ClaimPath, ContractProposal, SemanticClaimPath, SemanticClaimSubject,
        proof::{
            AcceptanceRequest, CLOSURE_PROOF_FAMILIES, CensusCompleteness, PROOF_POLICY_VERSION,
            ProofRuleInput, family_authority, proof_scope_digest, replay_proof_rule,
            verify_and_accept,
        },
    };

    use super::*;

    fn resolved_import() -> ResolvedImport {
        let root = "/project/node_modules/solid-js";
        let runtime = ResolvedFile {
            path: format!("{root}/dist/solid.js"),
            real_path: None,
            digest: format!("sha256:{}", "b".repeat(64)),
        };
        let declarations = ResolvedFile {
            path: format!("{root}/types/index.d.ts"),
            real_path: None,
            digest: format!("sha256:{}", "d".repeat(64)),
        };
        ResolvedImport {
            specifier: "solid-js".into(),
            importer: "/project/src/app.tsx".into(),
            requested_entrypoint: ".".into(),
            package_name: "solid-js".into(),
            package_version: "2.0.0-rc.3".into(),
            package_integrity: "sha512:test".into(),
            package_root: root.into(),
            package_real_root: None,
            package_manifest: ResolvedFile {
                path: format!("{root}/package.json"),
                real_path: None,
                digest: format!("sha256:{}", "a".repeat(64)),
            },
            runtime: runtime.clone(),
            declarations: declarations.clone(),
            runtime_trace: ResolutionTrace::default(),
            declaration_trace: ResolutionTrace::default(),
            closure: ClosureManifest::new(vec![], vec![], vec![]).unwrap(),
            transform: None,
            exports: BTreeMap::from([(
                "version".into(),
                ResolvedExportBinding {
                    runtime: ResolvedExportTarget {
                        module: runtime,
                        export_name: "version".into(),
                    },
                    declarations: ResolvedExportTarget {
                        module: declarations,
                        export_name: "version".into(),
                    },
                },
            )]),
            authority: ResolutionAuthority::Host,
        }
    }

    #[test]
    fn proof_issued_receipt_is_the_only_path_into_the_analyzer_index() {
        let import = resolved_import();
        let closure = import.closure.digest.as_str().trim_start_matches("sha256:");
        let document = format!(
            "{{\"format\":\"solid-reactivity-contract\",\"schemaVersion\":2,\"semanticModelVersion\":1,\"package\":{{\"name\":\"solid-js\",\"version\":\"2.0.0-rc.3\",\"integrity\":\"sha512:test\",\"manifest\":{{\"path\":\"package.json\",\"sha256\":\"{}\"}}}},\"summaries\":{{\"callable\":{{\"shape\":\"callable\",\"call\":{{\"closed\":[\"reads\"],\"reads\":[]}}}}}},\"entrypoints\":{{\".\":{{\"artifact\":{{\"path\":\"dist/solid.js\",\"sha256\":\"{}\",\"closureSha256\":\"{}\"}},\"declarations\":{{\"path\":\"types/index.d.ts\",\"sha256\":\"{}\"}},\"exports\":{{\"version\":\"callable\"}}}}}},\"sidecars\":{{}}}}",
            "a".repeat(64),
            "b".repeat(64),
            closure,
            "d".repeat(64),
        );
        let final_contract = contract_document_v2::decode(document.as_bytes())
            .unwrap()
            .normalize()
            .unwrap();
        let selected = select_and_bind(&final_contract, &import).unwrap();
        let selected_case = selected.artifact_cases()[0].id.clone();
        let mut cases = selected.artifact_cases().to_vec();
        let export_name = "version".to_owned();
        cases[0]
            .exports
            .get_mut(&export_name)
            .unwrap()
            .open_proposed_closure();
        let proposal = ContractProposal::new(selected.package().clone(), cases)
            .normalize()
            .unwrap();
        let subject = SemanticClaimSubject {
            artifact_case: selected_case.clone(),
            export: export_name,
            path: SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Reads)),
        };
        let proofs = CLOSURE_PROOF_FAMILIES
            .into_iter()
            .enumerate()
            .map(|(index, family)| {
                replay_proof_rule(
                    &proposal,
                    family,
                    subject.clone(),
                    ProofRuleInput {
                        authority: family_authority(family),
                        transcript: format!("phase-12 replay {index}").into_bytes(),
                        observed_scope: proof_scope_digest(&proposal, family, &subject).unwrap(),
                        enumerated: vec![],
                        classified: vec![],
                        unresolved: vec![],
                        completeness: CensusCompleteness::Complete,
                    },
                )
                .unwrap()
            })
            .collect();
        let issued = verify_and_accept(AcceptanceRequest {
            contract: proposal,
            selected_artifact_case: selected_case,
            wire_bytes: document.as_bytes().to_vec(),
            closed_claims: vec![subject],
            proofs,
            contradictions: vec![],
            verifier: VerifierIdentity {
                build: "phase-12-test".into(),
                policy: PROOF_POLICY_VERSION,
            },
        })
        .unwrap();
        let receipt = encode_acceptance_receipt(issued.receipt()).unwrap();

        let index = load_accepted_contract_index([AcceptedContractSource {
            document: document.as_bytes(),
            receipt: &receipt,
            import: &import,
        }])
        .unwrap();
        let identity = issued.export("version").unwrap().identity.clone();
        assert!(
            index
                .resolve(&import.importer, &import.specifier, &identity)
                .is_ok()
        );

        let mut reformatted = document.as_bytes().to_vec();
        reformatted.push(b' ');
        assert!(matches!(
            load_accepted_contract(&reformatted, &receipt, &import),
            Err(ContractFailure::ReceiptMismatch {
                field: "wireDigest"
            })
        ));

        let mut noncanonical: serde_json::Value = serde_json::from_slice(&receipt).unwrap();
        noncanonical["semanticDigest"] = serde_json::json!(
            noncanonical["semanticDigest"]
                .as_str()
                .unwrap()
                .to_ascii_uppercase()
        );
        assert!(matches!(
            load_accepted_contract(
                document.as_bytes(),
                &serde_json::to_vec(&noncanonical).unwrap(),
                &import,
            ),
            Err(ContractFailure::ReceiptMismatch {
                field: "semanticDigest"
            })
        ));

        let oversized = vec![b' '; MAX_RECEIPT_BYTES + 1];
        assert!(matches!(
            load_accepted_contract(document.as_bytes(), &oversized, &import),
            Err(ContractFailure::ReceiptDecode { .. })
        ));
    }
}
