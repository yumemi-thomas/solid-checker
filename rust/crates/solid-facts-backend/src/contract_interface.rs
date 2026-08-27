//! The only package-contract boundary exposed to analyzer callers.
//!
//! Schema spellings and compact document mechanics remain private. Temporary
//! schema-v2 documents are decoded and normalized by the sibling deep module;
//! this analyzer-loading interface still refuses exposure until Phase 12 can
//! validate proof-issued receipts and migrate consumers atomically.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{AcceptanceReceipt, AcceptedContract};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

use crate::{artifact_resolution::select_and_bind, contract_document_v2};

pub use crate::artifact_resolution::{
    AcceptedDependencyEdge, AffectedClaimDomain, ArtifactResolutionFailure, ArtifactResolver,
    ArtifactResolverChain, ClosureEntry, ClosureFileRole, ClosureHazard, ClosureHazardKind,
    ClosureInput, ClosureManifest, HostResolutionAdapter, ImportRequest, ResolutionAuthority,
    ResolutionTrace, ResolutionTraceStep, ResolvedExportBinding, ResolvedExportTarget,
    ResolvedFile, ResolvedImport, StandaloneResolutionAdapter, TypeFactsResolutionAdapter,
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
        match fs::read(path) {
            Ok(bytes) => verify_evidence_content(key, bytes.into()).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(EvidenceStoreFailure::Io {
                message: error.to_string(),
            }),
        }
    }
}

impl ReceiptStore for LocalEvidenceStore {
    fn store_receipt(&self, bytes: &[u8]) -> Result<EvidenceKey, EvidenceStoreFailure> {
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
    #[error("accepted-contract loading is reserved for the Phase 12 analyzer integration")]
    AcceptanceUnavailable,
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

/// Loads one accepted contract for an already resolved import. Temporary-v2
/// wire mechanics are fully normalized here, but analyzer exposure remains
/// disabled until the Phase 12 consumer migration validates proof-issued
/// receipts through this boundary.
pub fn load_accepted_contract(
    document_bytes: &[u8],
    receipt: &[u8],
    import: &ResolvedImport,
) -> Result<AcceptedContract, ContractFailure> {
    let document = contract_document_v2::decode(document_bytes)?;
    let receipt: WireReceipt =
        serde_json::from_slice(receipt).map_err(|error| ContractFailure::ReceiptDecode {
            message: error.to_string(),
        })?;

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
    if receipt.semantic_model_version != document.semantic_model_version() {
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

    let normalized = document.normalize()?;
    let selected = select_and_bind(&normalized, import)?;
    let _ = (
        selected.semantic_digest(),
        receipt.verifier.build,
        receipt.verifier.policy,
    );
    Err(ContractFailure::AcceptanceUnavailable)
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
