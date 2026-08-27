//! The only package-contract boundary exposed to analyzer callers.
//!
//! Schema spellings and compact document mechanics remain private. Phase 2
//! intentionally refuses schema-v2 documents after validating their envelope:
//! normalization and receipt verification land in Phase 5/7, before any
//! producer or consumer is migrated.

use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::AcceptedContract;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};
use thiserror::Error;

const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
const DEVELOPMENT_SCHEMA_VERSION: u16 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionAuthority {
    HostTypeFacts,
    StandalonePackageResolver,
}

/// Exact artifact selected by a real resolver. Friendly environment labels
/// are intentionally absent: selection is identity and trace based.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImport {
    pub specifier: String,
    pub importer: String,
    pub requested_entrypoint: String,
    pub package_name: String,
    pub package_version: String,
    pub package_integrity: String,
    pub package_manifest: String,
    pub runtime: ResolvedFile,
    pub declarations: ResolvedFile,
    pub dependency_closure_digest: String,
    pub transform: Option<ResolvedFile>,
    pub export_trace: Vec<ResolutionTraceStep>,
    pub authority: ResolutionAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFile {
    pub path: String,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionTraceStep {
    pub condition: String,
    pub target: String,
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

/// Adapter for exact resolutions originating in the configured Type Facts
/// session. Duplicate rows are retained as ambiguity rather than overwritten.
#[derive(Clone, Debug, Default)]
pub struct HostResolutionAdapter {
    rows: BTreeMap<ImportRequest, Vec<ResolvedImport>>,
}

impl HostResolutionAdapter {
    #[must_use]
    pub fn from_rows(rows: impl IntoIterator<Item = (ImportRequest, ResolvedImport)>) -> Self {
        Self {
            rows: collect_resolution_rows(rows, ResolutionAuthority::HostTypeFacts),
        }
    }
}

impl ArtifactResolver for HostResolutionAdapter {
    fn resolve(
        &self,
        request: &ImportRequest,
    ) -> Result<ResolvedImport, ArtifactResolutionFailure> {
        exact_resolution(&self.rows, request)
    }
}

/// Adapter for exact resolutions produced by the standalone, standards-
/// compatible package resolver. Resolution itself remains owned by package
/// acquisition; this adapter prevents it from becoming a second contract
/// selector.
#[derive(Clone, Debug, Default)]
pub struct StandaloneResolutionAdapter {
    rows: BTreeMap<ImportRequest, Vec<ResolvedImport>>,
}

impl StandaloneResolutionAdapter {
    #[must_use]
    pub fn from_rows(rows: impl IntoIterator<Item = (ImportRequest, ResolvedImport)>) -> Self {
        Self {
            rows: collect_resolution_rows(rows, ResolutionAuthority::StandalonePackageResolver),
        }
    }
}

impl ArtifactResolver for StandaloneResolutionAdapter {
    fn resolve(
        &self,
        request: &ImportRequest,
    ) -> Result<ResolvedImport, ArtifactResolutionFailure> {
        exact_resolution(&self.rows, request)
    }
}

fn collect_resolution_rows(
    rows: impl IntoIterator<Item = (ImportRequest, ResolvedImport)>,
    authority: ResolutionAuthority,
) -> BTreeMap<ImportRequest, Vec<ResolvedImport>> {
    let mut collected: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (request, mut resolved) in rows {
        resolved.authority = authority;
        collected.entry(request).or_default().push(resolved);
    }
    collected
}

fn exact_resolution(
    rows: &BTreeMap<ImportRequest, Vec<ResolvedImport>>,
    request: &ImportRequest,
) -> Result<ResolvedImport, ArtifactResolutionFailure> {
    match rows.get(request).map(Vec::as_slice) {
        None | Some([]) => Err(ArtifactResolutionFailure::Unattested),
        Some([resolved])
            if resolved.specifier == request.specifier && resolved.importer == request.importer =>
        {
            Ok(resolved.clone())
        }
        Some([_]) => Err(ArtifactResolutionFailure::Invalid {
            reason: "the result does not identify the requested specifier and importer".into(),
        }),
        Some(_) => Err(ArtifactResolutionFailure::Ambiguous),
    }
}

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
        Ok(Self(value))
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
    #[error(
        "schema-v2 normalization is not enabled until the normalized-model implementation phase"
    )]
    NormalizationUnavailable,
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

// These are intentionally private wire types. The shape is only deepened as
// decoder/normalizer work lands; downstream crates cannot name any of it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireDocument {
    schema_version: u16,
    semantic_model_version: u16,
    package: WirePackage,
    #[serde(default)]
    summaries: BTreeMap<String, serde_json::Value>,
    entrypoints: BTreeMap<String, WireEntrypoint>,
    #[serde(default)]
    sidecars: Vec<WireSidecar>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WirePackage {
    name: String,
    version: String,
    integrity: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireEntrypoint {
    #[serde(default)]
    artifact: Option<serde_json::Value>,
    #[serde(default)]
    cases: Vec<serde_json::Value>,
    exports: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireSidecar {
    kind: String,
    digest: String,
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

/// Loads one accepted contract for an already resolved import. Until the
/// normalization phase lands, valid development-schema input fails closed at
/// the explicit `NormalizationUnavailable` boundary.
pub fn load_accepted_contract(
    document_bytes: &[u8],
    receipt: &[u8],
    _import: &ResolvedImport,
) -> Result<AcceptedContract, ContractFailure> {
    if document_bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(ContractFailure::DocumentTooLarge {
            limit: MAX_DOCUMENT_BYTES,
        });
    }
    let document: WireDocument = serde_json::from_slice(document_bytes).map_err(|error| {
        ContractFailure::DocumentDecode {
            message: error.to_string(),
        }
    })?;
    if document.schema_version != DEVELOPMENT_SCHEMA_VERSION {
        return Err(ContractFailure::UnsupportedSchemaVersion {
            expected: DEVELOPMENT_SCHEMA_VERSION,
            actual: document.schema_version,
        });
    }
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
    if receipt.semantic_model_version != document.semantic_model_version {
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

    // Read every envelope field here so additions cannot silently become
    // ignored input while the normalizer is still closed.
    let _ = (
        document.semantic_model_version,
        document.package.name,
        document.package.version,
        document.package.integrity,
        document.summaries,
        document
            .entrypoints
            .into_values()
            .map(|entry| (entry.artifact, entry.cases, entry.exports))
            .collect::<Vec<_>>(),
        document
            .sidecars
            .into_iter()
            .map(|sidecar| (sidecar.kind, sidecar.digest))
            .collect::<Vec<_>>(),
        receipt.verifier.build,
        receipt.verifier.policy,
    );
    Err(ContractFailure::NormalizationUnavailable)
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|payload| {
        payload.len() == 64 && payload.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}
