//! Internal authenticated receipt-v2 protocol.
//!
//! Receipt bytes gain authority only through an explicit configured or
//! immutable built-in provenance. The authenticated token is consumed by the
//! analyzer loader; serialized proof evidence cannot manufacture it.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{
    Digest, NormalizedContract, certification::proof_policy_2,
};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

use crate::{artifact_resolution::ResolvedImport, bounded_json, contract_document};

const RECEIPT_FORMAT: &str = "solid-checker-authenticated-acceptance-receipt";
const SIGNATURE_ALGORITHM: &str = "ed25519";
const BUILTIN_ALGORITHM: &str = "builtin-entry-sha256";
const PAYLOAD_DOMAIN: &[u8] = b"solid-checker:acceptance-receipt:v2";
const TRUST_STORE_DOMAIN: &[u8] = b"solid-checker:receipt-trust-store:v2";
const MAX_RECEIPT_BYTES: usize = 64 * 1024;
const TRUST_CONFIGURATION_FORMAT: &str = "solid-checker-policy2-trust-configuration";
const TRUST_CONFIGURATION_VERSION: u16 = 1;
const MAX_STRING_BYTES: usize = 16 * 1024;
const MAX_ROOTS: usize = 256;
const RECEIPT_WITNESS_FAMILIES: [&str; 17] = [
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
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReceiptIssuerKind {
    BuiltIn,
    PersistentLocal,
    Portable,
}

impl ReceiptIssuerKind {
    const fn code(self) -> u8 {
        match self {
            Self::BuiltIn => 1,
            Self::PersistentLocal => 2,
            Self::Portable => 3,
        }
    }
}

/// Every certification root the receipt must freeze. Construction validates
/// shape only: authority comes from the opaque verifier sessions that supply
/// these roots and from the configured issuer that signs the final payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy2ReceiptBindings {
    pub importer: String,
    pub specifier: String,
    pub resolved_import_root: String,
    pub semantic_digest: String,
    pub artifact_provenance_root: String,
    pub snapshot_root: String,
    pub package_root: String,
    pub manifest_root: String,
    pub artifacts_root: String,
    pub declarations_root: String,
    pub transform_root: String,
    pub exports_root: String,
    pub closure_root: String,
    pub demand_graph_root: String,
    pub verified_positive_root: String,
    pub witness_roots: BTreeMap<String, String>,
    pub producer_sessions_root: String,
    pub dependency_receipts_root: String,
    pub dependency_trust_root: String,
    pub probe_gate_root: String,
    pub closed_claims_root: String,
    pub verifier_source_digest: String,
    pub verifier_build_digest: String,
}

impl Policy2ReceiptBindings {
    fn validate(&self) -> Result<(), Policy2ReceiptError> {
        for (field, value) in [
            ("importer", self.importer.as_str()),
            ("specifier", self.specifier.as_str()),
        ] {
            if value.is_empty()
                || value.len() > MAX_STRING_BYTES
                || value.bytes().any(|byte| byte.is_ascii_control())
            {
                return Err(Policy2ReceiptError::InvalidBinding { field });
            }
        }
        for (field, value) in [
            ("semanticDigest", &self.semantic_digest),
            ("resolvedImportRoot", &self.resolved_import_root),
            ("artifactProvenanceRoot", &self.artifact_provenance_root),
            ("snapshotRoot", &self.snapshot_root),
            ("packageRoot", &self.package_root),
            ("manifestRoot", &self.manifest_root),
            ("artifactsRoot", &self.artifacts_root),
            ("declarationsRoot", &self.declarations_root),
            ("transformRoot", &self.transform_root),
            ("exportsRoot", &self.exports_root),
            ("closureRoot", &self.closure_root),
            ("demandGraphRoot", &self.demand_graph_root),
            ("verifiedPositiveRoot", &self.verified_positive_root),
            ("producerSessionsRoot", &self.producer_sessions_root),
            ("dependencyReceiptsRoot", &self.dependency_receipts_root),
            ("dependencyTrustRoot", &self.dependency_trust_root),
            ("probeGateRoot", &self.probe_gate_root),
            ("closedClaimsRoot", &self.closed_claims_root),
            ("verifierSourceDigest", &self.verifier_source_digest),
            ("verifierBuildDigest", &self.verifier_build_digest),
        ] {
            validate_digest(value).map_err(|_| Policy2ReceiptError::InvalidBinding { field })?;
        }
        if self.witness_roots.len() != RECEIPT_WITNESS_FAMILIES.len()
            || !RECEIPT_WITNESS_FAMILIES
                .iter()
                .all(|family| self.witness_roots.contains_key(*family))
        {
            return Err(Policy2ReceiptError::InvalidBinding {
                field: "witnessRoots",
            });
        }
        for (family, root) in &self.witness_roots {
            if family.is_empty()
                || family.len() > MAX_STRING_BYTES
                || !family
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
                || validate_digest(root).is_err()
            {
                return Err(Policy2ReceiptError::InvalidBinding {
                    field: "witnessRoots",
                });
            }
        }
        Ok(())
    }
}

pub struct ConfiguredReceiptIssuer {
    kind: ReceiptIssuerKind,
    scope: String,
    signing_key: SigningKey,
    key_id: String,
}

impl ConfiguredReceiptIssuer {
    pub fn persistent_local(
        scope: impl Into<String>,
        seed: [u8; 32],
    ) -> Result<Self, Policy2ReceiptError> {
        Self::new(ReceiptIssuerKind::PersistentLocal, scope.into(), seed)
    }

    pub fn portable(
        trust_chain: impl Into<String>,
        seed: [u8; 32],
    ) -> Result<Self, Policy2ReceiptError> {
        Self::new(ReceiptIssuerKind::Portable, trust_chain.into(), seed)
    }

    fn new(
        kind: ReceiptIssuerKind,
        scope: String,
        seed: [u8; 32],
    ) -> Result<Self, Policy2ReceiptError> {
        validate_scope(&scope)?;
        let signing_key = SigningKey::from_bytes(&seed);
        let key_id = key_id(&signing_key.verifying_key());
        Ok(Self {
            kind,
            scope,
            signing_key,
            key_id,
        })
    }

    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    #[must_use]
    pub const fn kind(&self) -> ReceiptIssuerKind {
        self.kind
    }

    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    #[must_use]
    pub fn public_key(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }
}

/// Immutable compiled-bundle authority. The entry digest is over the complete
/// canonical receipt bytes, not a key or label copied from those bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltInReceiptEntry {
    pub entry_digest: String,
    pub verifier_build_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Policy2TrustEntry {
    pub kind: ReceiptIssuerKind,
    pub key_id: String,
    pub public_key: [u8; 32],
    pub scope: String,
    pub allowed_policy_digests: Vec<String>,
    pub allowed_verifier_builds: Vec<String>,
    pub revoked_at_epoch: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Policy2TrustStore {
    entries: BTreeMap<String, Policy2TrustEntry>,
    revocation_epoch: u64,
    digest: String,
}

/// External analyzer trust configuration. The analyzed project may point at
/// this object, but it may not author it: callers must acquire the bytes from
/// a separately configured path and pass the authenticated result into catalog
/// discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Policy2TrustConfiguration {
    trust_store: Policy2TrustStore,
    persistent_local_scope: Option<String>,
}

impl Policy2TrustConfiguration {
    pub fn new(
        trust_store: Policy2TrustStore,
        persistent_local_scope: Option<String>,
    ) -> Result<Self, Policy2ReceiptError> {
        if let Some(scope) = persistent_local_scope.as_deref() {
            validate_scope(scope)?;
        }
        Ok(Self {
            trust_store,
            persistent_local_scope,
        })
    }

    #[must_use]
    pub const fn trust_store(&self) -> &Policy2TrustStore {
        &self.trust_store
    }

    #[must_use]
    pub fn persistent_local_scope(&self) -> Option<&str> {
        self.persistent_local_scope.as_deref()
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TrustConfigurationDocument {
    format: String,
    trust_configuration_version: u16,
    revocation_epoch: u64,
    #[serde(default)]
    persistent_local_scope: Option<String>,
    entries: Vec<TrustConfigurationEntry>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TrustConfigurationEntry {
    kind: ReceiptIssuerKind,
    key_id: String,
    public_key: String,
    scope: String,
    allowed_policy_digests: Vec<String>,
    allowed_verifier_builds: Vec<String>,
    #[serde(default)]
    revoked_at_epoch: Option<u64>,
}

pub fn decode_policy2_trust_configuration(
    bytes: &[u8],
) -> Result<Policy2TrustConfiguration, Policy2ReceiptError> {
    let document: TrustConfigurationDocument = bounded_json::decode(
        bytes,
        bounded_json::Limits {
            bytes: MAX_RECEIPT_BYTES,
            depth: 128,
            nodes: 4096,
            string_bytes: MAX_STRING_BYTES,
        },
    )
    .map_err(|message| Policy2ReceiptError::Decode { message })?;
    if document.format != TRUST_CONFIGURATION_FORMAT
        || document.trust_configuration_version != TRUST_CONFIGURATION_VERSION
    {
        return Err(Policy2ReceiptError::InvalidTrustStore);
    }
    let entries = document
        .entries
        .into_iter()
        .map(|entry| {
            Ok(Policy2TrustEntry {
                kind: entry.kind,
                key_id: entry.key_id,
                public_key: decode_public_key(&entry.public_key)?,
                scope: entry.scope,
                allowed_policy_digests: entry.allowed_policy_digests,
                allowed_verifier_builds: entry.allowed_verifier_builds,
                revoked_at_epoch: entry.revoked_at_epoch,
            })
        })
        .collect::<Result<Vec<_>, Policy2ReceiptError>>()?;
    Policy2TrustConfiguration::new(
        Policy2TrustStore::new(entries, document.revocation_epoch)?,
        document.persistent_local_scope,
    )
}

/// Constructs the least-privilege trust configuration for one configured
/// issuer and verifier build. The signing seed is not retained or serialized.
pub fn policy2_trust_configuration_for_issuer(
    issuer: &ConfiguredReceiptIssuer,
    verifier_build_digest: &str,
    revocation_epoch: u64,
) -> Result<Policy2TrustConfiguration, Policy2ReceiptError> {
    validate_digest(verifier_build_digest).map_err(|_| Policy2ReceiptError::InvalidTrustStore)?;
    Policy2TrustConfiguration::new(
        Policy2TrustStore::new(
            [Policy2TrustEntry {
                kind: issuer.kind(),
                key_id: issuer.key_id().to_owned(),
                public_key: issuer.public_key(),
                scope: issuer.scope().to_owned(),
                allowed_policy_digests: vec![proof_policy_2().digest().as_str().to_owned()],
                allowed_verifier_builds: vec![verifier_build_digest.to_owned()],
                revoked_at_epoch: None,
            }],
            revocation_epoch,
        )?,
        (issuer.kind() == ReceiptIssuerKind::PersistentLocal).then(|| issuer.scope().to_owned()),
    )
}

/// Canonical external trust bytes for fresh-process discovery.
pub fn encode_policy2_trust_configuration(
    configuration: &Policy2TrustConfiguration,
) -> Result<Vec<u8>, Policy2ReceiptError> {
    let entries = configuration
        .trust_store
        .entries
        .values()
        .map(|entry| TrustConfigurationEntry {
            kind: entry.kind,
            key_id: entry.key_id.clone(),
            public_key: STANDARD.encode(entry.public_key),
            scope: entry.scope.clone(),
            allowed_policy_digests: entry.allowed_policy_digests.clone(),
            allowed_verifier_builds: entry.allowed_verifier_builds.clone(),
            revoked_at_epoch: entry.revoked_at_epoch,
        })
        .collect();
    let document = TrustConfigurationDocument {
        format: TRUST_CONFIGURATION_FORMAT.into(),
        trust_configuration_version: TRUST_CONFIGURATION_VERSION,
        revocation_epoch: configuration.trust_store.revocation_epoch,
        persistent_local_scope: configuration.persistent_local_scope.clone(),
        entries,
    };
    let mut bytes = serde_json::to_vec(&document)
        .map_err(|error| Policy2ReceiptError::Encode(error.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

impl Policy2TrustStore {
    pub fn new(
        entries: impl IntoIterator<Item = Policy2TrustEntry>,
        revocation_epoch: u64,
    ) -> Result<Self, Policy2ReceiptError> {
        let mut entries_by_id = BTreeMap::new();
        for mut entry in entries {
            if entry.kind == ReceiptIssuerKind::BuiltIn
                || entry.key_id != key_id_from_bytes(&entry.public_key)
            {
                return Err(Policy2ReceiptError::KeyConfusion);
            }
            validate_scope(&entry.scope)?;
            canonicalize_digests(&mut entry.allowed_policy_digests)?;
            canonicalize_digests(&mut entry.allowed_verifier_builds)?;
            if entry.allowed_policy_digests.is_empty()
                || entry.allowed_verifier_builds.is_empty()
                || entries_by_id.insert(entry.key_id.clone(), entry).is_some()
            {
                return Err(Policy2ReceiptError::InvalidTrustStore);
            }
        }
        if entries_by_id.is_empty() {
            return Err(Policy2ReceiptError::InvalidTrustStore);
        }
        let digest = trust_store_digest(&entries_by_id, revocation_epoch);
        Ok(Self {
            entries: entries_by_id,
            revocation_epoch,
            digest,
        })
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    #[must_use]
    pub const fn revocation_epoch(&self) -> u64 {
        self.revocation_epoch
    }
}

pub enum Policy2ReceiptProvenance<'a> {
    BuiltIn(&'a BuiltInReceiptEntry),
    PersistentLocal {
        trust_store: &'a Policy2TrustStore,
        scope: &'a str,
    },
    Portable {
        trust_store: &'a Policy2TrustStore,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedPolicy2Receipt {
    receipt_digest: String,
    main_digest: String,
    trust_store_digest: String,
    revocation_epoch: u64,
    semantic_digest: Digest,
    policy_digest: Digest,
    closed_claims_root: Digest,
    verifier_build_digest: Digest,
    issuer_kind: ReceiptIssuerKind,
    issuer_scope: String,
    bindings: Policy2ReceiptBindings,
    contract: NormalizedContract,
}

impl AuthenticatedPolicy2Receipt {
    #[must_use]
    pub fn receipt_digest(&self) -> &str {
        &self.receipt_digest
    }

    #[must_use]
    pub fn main_digest(&self) -> &str {
        &self.main_digest
    }

    #[must_use]
    pub fn trust_store_digest(&self) -> &str {
        &self.trust_store_digest
    }

    #[must_use]
    pub const fn revocation_epoch(&self) -> u64 {
        self.revocation_epoch
    }

    #[must_use]
    pub const fn semantic_digest(&self) -> &Digest {
        &self.semantic_digest
    }

    #[must_use]
    pub const fn policy_digest(&self) -> &Digest {
        &self.policy_digest
    }

    #[must_use]
    pub const fn closed_claims_root(&self) -> &Digest {
        &self.closed_claims_root
    }

    #[must_use]
    pub const fn verifier_build_digest(&self) -> &Digest {
        &self.verifier_build_digest
    }

    #[must_use]
    pub const fn issuer_kind(&self) -> ReceiptIssuerKind {
        self.issuer_kind
    }

    #[must_use]
    pub fn issuer_scope(&self) -> &str {
        &self.issuer_scope
    }

    #[must_use]
    pub(super) fn contains_closed_claim_id(&self, semantic_claim_id: &str) -> bool {
        self.contract.contains_closed_claim_id(semantic_claim_id)
    }

    #[must_use]
    pub const fn bindings(&self) -> &Policy2ReceiptBindings {
        &self.bindings
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptDocument {
    format: String,
    receipt_version: u16,
    payload: ReceiptPayload,
    authentication: ReceiptAuthentication,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptVersionProbe {
    receipt_version: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptPayload {
    semantic_model_version: u16,
    proof_version: u16,
    proof_policy: u32,
    policy_digest: String,
    main_digest: String,
    importer: String,
    specifier: String,
    resolved_import_root: String,
    semantic_digest: String,
    artifact_provenance_root: String,
    snapshot_root: String,
    package_root: String,
    manifest_root: String,
    artifacts_root: String,
    declarations_root: String,
    transform_root: String,
    exports_root: String,
    closure_root: String,
    demand_graph_root: String,
    verified_positive_root: String,
    witness_roots: BTreeMap<String, String>,
    producer_sessions_root: String,
    dependency_receipts_root: String,
    dependency_trust_root: String,
    probe_gate_root: String,
    closed_claims_root: String,
    verifier_source_digest: String,
    verifier_build_digest: String,
    issuer_kind: ReceiptIssuerKind,
    issuer_scope: String,
    key_id: String,
    signature_algorithm: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptAuthentication {
    public_key: Option<String>,
    value: String,
}

/// Creates a configured Ed25519 receipt. It intentionally accepts only the
/// finalized binding set; proof JSON and caller-supplied success booleans are
/// absent from the interface.
pub fn issue_policy2_receipt(
    canonical_main: &[u8],
    bindings: &Policy2ReceiptBindings,
    issuer: &ConfiguredReceiptIssuer,
) -> Result<Vec<u8>, Policy2ReceiptError> {
    let (_, normalized) = validate_canonical_main(canonical_main)?;
    let semantic_digest = normalized.semantic_digest().as_str();
    bindings.validate()?;
    if semantic_digest != bindings.semantic_digest {
        return Err(Policy2ReceiptError::BindingMismatch {
            field: "semanticDigest",
        });
    }
    let payload = payload(
        canonical_main,
        bindings,
        issuer.kind,
        &issuer.scope,
        &issuer.key_id,
        SIGNATURE_ALGORITHM,
    );
    let signature = issuer.signing_key.sign(&canonical_payload(&payload));
    encode_receipt(ReceiptDocument {
        format: RECEIPT_FORMAT.into(),
        receipt_version: proof_policy_2().receipt_version(),
        payload,
        authentication: ReceiptAuthentication {
            public_key: Some(STANDARD.encode(issuer.public_key())),
            value: STANDARD.encode(signature.to_bytes()),
        },
    })
}

/// Produces the one stable-v1 byte encoding that receipt v2 is allowed to
/// authenticate. This rendering function carries no acceptance authority.
pub fn canonicalize_policy2_main(document: &[u8]) -> Result<Vec<u8>, Policy2ReceiptError> {
    let decoded = contract_document::decode(document)
        .map_err(|error| Policy2ReceiptError::MainDocument(error.to_string()))?;
    let sidecars = decoded
        .sidecar_digests()
        .map_err(|error| Policy2ReceiptError::MainDocument(error.to_string()))?;
    let normalized = decoded
        .normalize()
        .map_err(|error| Policy2ReceiptError::MainDocument(error.to_string()))?;
    contract_document::encode(&normalized, &sidecars, false)
        .map_err(|error| Policy2ReceiptError::MainDocument(error.to_string()))
}

/// Recomputes the semantic digest from an already canonical policy-2 main.
pub fn policy2_main_semantic_digest(canonical_main: &[u8]) -> Result<String, Policy2ReceiptError> {
    validate_canonical_main(canonical_main)
        .map(|(_, contract)| contract.semantic_digest().as_str().to_owned())
}

/// Canonical identity of the complete resolver answer selected for one
/// importer/specifier pair. The resolved record is already path-normalized by
/// the host boundary; stable struct order plus BTreeMap export order makes the
/// encoded census deterministic.
pub fn policy2_resolved_import_root(
    resolved: &ResolvedImport,
) -> Result<String, Policy2ReceiptError> {
    resolved
        .validate()
        .map_err(|error| Policy2ReceiptError::ResolvedImport(error.to_string()))?;
    let encoded = serde_json::to_vec(resolved)
        .map_err(|error| Policy2ReceiptError::Encode(error.to_string()))?;
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:policy2-resolved-import:v1");
    hash.update(
        u64::try_from(encoded.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hash.update(encoded);
    Ok(format!("sha256:{:x}", hash.finalize()))
}

#[must_use]
pub fn policy2_policy_digest() -> &'static str {
    proof_policy_2().digest().as_str()
}

/// Constructs a compiled-bundle receipt. Authentication still depends on the
/// caller retaining the resulting bytes in the immutable bundle map and
/// passing their independently compiled entry digest to the loader.
#[doc(hidden)]
pub fn issue_builtin_policy2_receipt(
    canonical_main: &[u8],
    bindings: &Policy2ReceiptBindings,
    built_in_scope: &str,
) -> Result<Vec<u8>, Policy2ReceiptError> {
    let (_, normalized) = validate_canonical_main(canonical_main)?;
    let semantic_digest = normalized.semantic_digest().as_str();
    bindings.validate()?;
    validate_scope(built_in_scope)?;
    if semantic_digest != bindings.semantic_digest {
        return Err(Policy2ReceiptError::BindingMismatch {
            field: "semanticDigest",
        });
    }
    let payload = payload(
        canonical_main,
        bindings,
        ReceiptIssuerKind::BuiltIn,
        built_in_scope,
        "builtin",
        BUILTIN_ALGORITHM,
    );
    let value = digest_bytes(&canonical_payload(&payload));
    encode_receipt(ReceiptDocument {
        format: RECEIPT_FORMAT.into(),
        receipt_version: proof_policy_2().receipt_version(),
        payload,
        authentication: ReceiptAuthentication {
            public_key: None,
            value,
        },
    })
}

/// Policy-2 authentication boundary shared by native and WASM loaders.
pub fn authenticate_policy2_receipt(
    canonical_main: &[u8],
    receipt_bytes: &[u8],
    expected: &Policy2ReceiptBindings,
    provenance: Policy2ReceiptProvenance<'_>,
) -> Result<AuthenticatedPolicy2Receipt, Policy2ReceiptError> {
    let (_, normalized) = validate_canonical_main(canonical_main)?;
    let semantic_digest = normalized.semantic_digest().as_str();
    expected.validate()?;
    let limits = bounded_json::Limits {
        bytes: MAX_RECEIPT_BYTES,
        depth: 128,
        nodes: 4096,
        string_bytes: MAX_STRING_BYTES,
    };
    let version: ReceiptVersionProbe = bounded_json::decode(receipt_bytes, limits)
        .map_err(|message| Policy2ReceiptError::Decode { message })?;
    if version.receipt_version != proof_policy_2().receipt_version() {
        return Err(Policy2ReceiptError::ObsoletePolicy);
    }
    let document: ReceiptDocument = bounded_json::decode(receipt_bytes, limits)
        .map_err(|message| Policy2ReceiptError::Decode { message })?;
    if encode_receipt(document.clone())? != receipt_bytes {
        return Err(Policy2ReceiptError::NonCanonicalReceipt);
    }
    if document.format != RECEIPT_FORMAT
        || document.receipt_version != proof_policy_2().receipt_version()
    {
        return Err(Policy2ReceiptError::ObsoletePolicy);
    }
    validate_payload(&document.payload, canonical_main, semantic_digest, expected)?;
    let signed = canonical_payload(&document.payload);
    let (trust_store_digest, revocation_epoch) = match provenance {
        Policy2ReceiptProvenance::BuiltIn(entry) => {
            if document.payload.issuer_kind != ReceiptIssuerKind::BuiltIn
                || document.payload.signature_algorithm != BUILTIN_ALGORITHM
                || document.authentication.public_key.is_some()
                || document.authentication.value != digest_bytes(&signed)
                || entry.entry_digest != digest_bytes(receipt_bytes)
                || entry.verifier_build_digest != document.payload.verifier_build_digest
            {
                return Err(Policy2ReceiptError::ProvenanceMismatch);
            }
            (digest_bytes(b"solid-checker:built-in-receipt-trust:v2"), 0)
        }
        Policy2ReceiptProvenance::PersistentLocal { trust_store, scope } => {
            if document.payload.issuer_kind != ReceiptIssuerKind::PersistentLocal
                || document.payload.issuer_scope != scope
            {
                return Err(Policy2ReceiptError::ProvenanceMismatch);
            }
            verify_configured(&document, &signed, trust_store)?;
            (trust_store.digest.clone(), trust_store.revocation_epoch)
        }
        Policy2ReceiptProvenance::Portable { trust_store } => {
            if document.payload.issuer_kind != ReceiptIssuerKind::Portable {
                return Err(Policy2ReceiptError::ProvenanceMismatch);
            }
            verify_configured(&document, &signed, trust_store)?;
            (trust_store.digest.clone(), trust_store.revocation_epoch)
        }
    };
    Ok(AuthenticatedPolicy2Receipt {
        receipt_digest: digest_bytes(receipt_bytes),
        main_digest: document.payload.main_digest,
        trust_store_digest,
        revocation_epoch,
        semantic_digest: normalized.semantic_digest().clone(),
        policy_digest: Digest::parse(document.payload.policy_digest)
            .expect("validated policy digest is canonical"),
        closed_claims_root: Digest::parse(document.payload.closed_claims_root)
            .expect("validated closed-claims root is canonical"),
        verifier_build_digest: Digest::parse(document.payload.verifier_build_digest)
            .expect("validated verifier-build digest is canonical"),
        issuer_kind: document.payload.issuer_kind,
        issuer_scope: document.payload.issuer_scope,
        bindings: expected.clone(),
        contract: normalized,
    })
}

fn verify_configured(
    document: &ReceiptDocument,
    signed: &[u8],
    trust_store: &Policy2TrustStore,
) -> Result<(), Policy2ReceiptError> {
    if document.payload.signature_algorithm != SIGNATURE_ALGORITHM {
        return Err(Policy2ReceiptError::UnsupportedAlgorithm);
    }
    let entry = trust_store
        .entries
        .get(&document.payload.key_id)
        .ok_or(Policy2ReceiptError::UntrustedIssuer)?;
    if entry.kind != document.payload.issuer_kind
        || entry.scope != document.payload.issuer_scope
        || !entry
            .allowed_policy_digests
            .contains(&document.payload.policy_digest)
        || !entry
            .allowed_verifier_builds
            .contains(&document.payload.verifier_build_digest)
    {
        return Err(Policy2ReceiptError::TrustConstraint);
    }
    if entry
        .revoked_at_epoch
        .is_some_and(|epoch| epoch <= trust_store.revocation_epoch)
    {
        return Err(Policy2ReceiptError::RevokedIssuer);
    }
    let carried_key = document
        .authentication
        .public_key
        .as_deref()
        .ok_or(Policy2ReceiptError::KeyConfusion)
        .and_then(decode_public_key)?;
    if carried_key != entry.public_key || document.payload.key_id != key_id_from_bytes(&carried_key)
    {
        return Err(Policy2ReceiptError::KeyConfusion);
    }
    let signature_bytes = decode_canonical_base64(&document.authentication.value)?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| Policy2ReceiptError::NonCanonicalSignature)?;
    VerifyingKey::from_bytes(&entry.public_key)
        .map_err(|_| Policy2ReceiptError::KeyConfusion)?
        .verify_strict(signed, &signature)
        .map_err(|_| Policy2ReceiptError::InvalidSignature)
}

fn validate_payload(
    payload: &ReceiptPayload,
    main: &[u8],
    semantic_digest: &str,
    expected: &Policy2ReceiptBindings,
) -> Result<(), Policy2ReceiptError> {
    let policy = proof_policy_2();
    if payload.semantic_model_version != policy.semantic_model_version()
        || payload.proof_version != policy.proof_version()
        || payload.proof_policy != policy.policy_version()
        || payload.policy_digest != policy.digest().as_str()
    {
        return Err(Policy2ReceiptError::ObsoletePolicy);
    }
    let actual = payload_bindings(payload);
    if &actual != expected {
        let field = binding_mismatch(&actual, expected);
        return Err(Policy2ReceiptError::BindingMismatch { field });
    }
    if payload.main_digest != digest_bytes(main) {
        return Err(Policy2ReceiptError::BindingMismatch {
            field: "mainDigest",
        });
    }
    if payload.semantic_digest != semantic_digest {
        return Err(Policy2ReceiptError::BindingMismatch {
            field: "semanticDigest",
        });
    }
    Ok(())
}

fn payload_bindings(payload: &ReceiptPayload) -> Policy2ReceiptBindings {
    Policy2ReceiptBindings {
        importer: payload.importer.clone(),
        specifier: payload.specifier.clone(),
        resolved_import_root: payload.resolved_import_root.clone(),
        semantic_digest: payload.semantic_digest.clone(),
        artifact_provenance_root: payload.artifact_provenance_root.clone(),
        snapshot_root: payload.snapshot_root.clone(),
        package_root: payload.package_root.clone(),
        manifest_root: payload.manifest_root.clone(),
        artifacts_root: payload.artifacts_root.clone(),
        declarations_root: payload.declarations_root.clone(),
        transform_root: payload.transform_root.clone(),
        exports_root: payload.exports_root.clone(),
        closure_root: payload.closure_root.clone(),
        demand_graph_root: payload.demand_graph_root.clone(),
        verified_positive_root: payload.verified_positive_root.clone(),
        witness_roots: payload.witness_roots.clone(),
        producer_sessions_root: payload.producer_sessions_root.clone(),
        dependency_receipts_root: payload.dependency_receipts_root.clone(),
        dependency_trust_root: payload.dependency_trust_root.clone(),
        probe_gate_root: payload.probe_gate_root.clone(),
        closed_claims_root: payload.closed_claims_root.clone(),
        verifier_source_digest: payload.verifier_source_digest.clone(),
        verifier_build_digest: payload.verifier_build_digest.clone(),
    }
}

fn binding_mismatch(
    actual: &Policy2ReceiptBindings,
    expected: &Policy2ReceiptBindings,
) -> &'static str {
    for (field, matches) in [
        ("importer", actual.importer == expected.importer),
        ("specifier", actual.specifier == expected.specifier),
        (
            "resolvedImportRoot",
            actual.resolved_import_root == expected.resolved_import_root,
        ),
        (
            "semanticDigest",
            actual.semantic_digest == expected.semantic_digest,
        ),
        (
            "artifactProvenanceRoot",
            actual.artifact_provenance_root == expected.artifact_provenance_root,
        ),
        (
            "snapshotRoot",
            actual.snapshot_root == expected.snapshot_root,
        ),
        ("packageRoot", actual.package_root == expected.package_root),
        (
            "manifestRoot",
            actual.manifest_root == expected.manifest_root,
        ),
        (
            "artifactsRoot",
            actual.artifacts_root == expected.artifacts_root,
        ),
        (
            "declarationsRoot",
            actual.declarations_root == expected.declarations_root,
        ),
        (
            "transformRoot",
            actual.transform_root == expected.transform_root,
        ),
        ("exportsRoot", actual.exports_root == expected.exports_root),
        ("closureRoot", actual.closure_root == expected.closure_root),
        (
            "demandGraphRoot",
            actual.demand_graph_root == expected.demand_graph_root,
        ),
        (
            "verifiedPositiveRoot",
            actual.verified_positive_root == expected.verified_positive_root,
        ),
        (
            "witnessRoots",
            actual.witness_roots == expected.witness_roots,
        ),
        (
            "producerSessionsRoot",
            actual.producer_sessions_root == expected.producer_sessions_root,
        ),
        (
            "dependencyReceiptsRoot",
            actual.dependency_receipts_root == expected.dependency_receipts_root,
        ),
        (
            "dependencyTrustRoot",
            actual.dependency_trust_root == expected.dependency_trust_root,
        ),
        (
            "probeGateRoot",
            actual.probe_gate_root == expected.probe_gate_root,
        ),
        (
            "closedClaimsRoot",
            actual.closed_claims_root == expected.closed_claims_root,
        ),
        (
            "verifierSourceDigest",
            actual.verifier_source_digest == expected.verifier_source_digest,
        ),
        (
            "verifierBuildDigest",
            actual.verifier_build_digest == expected.verifier_build_digest,
        ),
    ] {
        if !matches {
            return field;
        }
    }
    "unknown"
}

fn payload(
    main: &[u8],
    bindings: &Policy2ReceiptBindings,
    issuer_kind: ReceiptIssuerKind,
    issuer_scope: &str,
    key_id: &str,
    signature_algorithm: &str,
) -> ReceiptPayload {
    ReceiptPayload {
        semantic_model_version: proof_policy_2().semantic_model_version(),
        proof_version: proof_policy_2().proof_version(),
        proof_policy: proof_policy_2().policy_version(),
        policy_digest: proof_policy_2().digest().as_str().into(),
        main_digest: digest_bytes(main),
        importer: bindings.importer.clone(),
        specifier: bindings.specifier.clone(),
        resolved_import_root: bindings.resolved_import_root.clone(),
        semantic_digest: bindings.semantic_digest.clone(),
        artifact_provenance_root: bindings.artifact_provenance_root.clone(),
        snapshot_root: bindings.snapshot_root.clone(),
        package_root: bindings.package_root.clone(),
        manifest_root: bindings.manifest_root.clone(),
        artifacts_root: bindings.artifacts_root.clone(),
        declarations_root: bindings.declarations_root.clone(),
        transform_root: bindings.transform_root.clone(),
        exports_root: bindings.exports_root.clone(),
        closure_root: bindings.closure_root.clone(),
        demand_graph_root: bindings.demand_graph_root.clone(),
        verified_positive_root: bindings.verified_positive_root.clone(),
        witness_roots: bindings.witness_roots.clone(),
        producer_sessions_root: bindings.producer_sessions_root.clone(),
        dependency_receipts_root: bindings.dependency_receipts_root.clone(),
        dependency_trust_root: bindings.dependency_trust_root.clone(),
        probe_gate_root: bindings.probe_gate_root.clone(),
        closed_claims_root: bindings.closed_claims_root.clone(),
        verifier_source_digest: bindings.verifier_source_digest.clone(),
        verifier_build_digest: bindings.verifier_build_digest.clone(),
        issuer_kind,
        issuer_scope: issuer_scope.into(),
        key_id: key_id.into(),
        signature_algorithm: signature_algorithm.into(),
    }
}

fn canonical_payload(payload: &ReceiptPayload) -> Vec<u8> {
    let mut bytes = Vec::new();
    frame(&mut bytes, PAYLOAD_DOMAIN);
    number(&mut bytes, payload.semantic_model_version as u64);
    number(&mut bytes, payload.proof_version as u64);
    number(&mut bytes, payload.proof_policy as u64);
    for value in [
        &payload.policy_digest,
        &payload.main_digest,
        &payload.importer,
        &payload.specifier,
        &payload.resolved_import_root,
        &payload.semantic_digest,
        &payload.artifact_provenance_root,
        &payload.snapshot_root,
        &payload.package_root,
        &payload.manifest_root,
        &payload.artifacts_root,
        &payload.declarations_root,
        &payload.transform_root,
        &payload.exports_root,
        &payload.closure_root,
        &payload.demand_graph_root,
        &payload.verified_positive_root,
    ] {
        frame(&mut bytes, value.as_bytes());
    }
    number(&mut bytes, payload.witness_roots.len() as u64);
    for (family, root) in &payload.witness_roots {
        frame(&mut bytes, family.as_bytes());
        frame(&mut bytes, root.as_bytes());
    }
    for value in [
        &payload.producer_sessions_root,
        &payload.dependency_receipts_root,
        &payload.dependency_trust_root,
        &payload.probe_gate_root,
        &payload.closed_claims_root,
        &payload.verifier_source_digest,
        &payload.verifier_build_digest,
    ] {
        frame(&mut bytes, value.as_bytes());
    }
    bytes.push(payload.issuer_kind.code());
    frame(&mut bytes, payload.issuer_scope.as_bytes());
    frame(&mut bytes, payload.key_id.as_bytes());
    frame(&mut bytes, payload.signature_algorithm.as_bytes());
    bytes
}

fn validate_canonical_main(
    bytes: &[u8],
) -> Result<(Vec<u8>, NormalizedContract), Policy2ReceiptError> {
    let decoded = contract_document::decode(bytes)
        .map_err(|error| Policy2ReceiptError::MainDocument(error.to_string()))?;
    let sidecars = decoded
        .sidecar_digests()
        .map_err(|error| Policy2ReceiptError::MainDocument(error.to_string()))?;
    let normalized = decoded
        .normalize()
        .map_err(|error| Policy2ReceiptError::MainDocument(error.to_string()))?;
    let canonical = contract_document::encode(&normalized, &sidecars, false)
        .map_err(|error| Policy2ReceiptError::MainDocument(error.to_string()))?;
    if canonical != bytes {
        return Err(Policy2ReceiptError::NonCanonicalMain);
    }
    Ok((canonical, normalized))
}

fn encode_receipt(document: ReceiptDocument) -> Result<Vec<u8>, Policy2ReceiptError> {
    let mut bytes = serde_json::to_vec(&document).map_err(|error| Policy2ReceiptError::Decode {
        message: error.to_string(),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn decode_public_key(value: &str) -> Result<[u8; 32], Policy2ReceiptError> {
    let bytes = decode_canonical_base64(value).map_err(|_| Policy2ReceiptError::KeyConfusion)?;
    bytes
        .try_into()
        .map_err(|_| Policy2ReceiptError::KeyConfusion)
}

fn decode_canonical_base64(value: &str) -> Result<Vec<u8>, Policy2ReceiptError> {
    let decoded = STANDARD
        .decode(value)
        .map_err(|_| Policy2ReceiptError::NonCanonicalSignature)?;
    if STANDARD.encode(&decoded) != value {
        return Err(Policy2ReceiptError::NonCanonicalSignature);
    }
    Ok(decoded)
}

fn key_id(key: &VerifyingKey) -> String {
    key_id_from_bytes(&key.to_bytes())
}

fn key_id_from_bytes(bytes: &[u8; 32]) -> String {
    format!("ed25519:{}", digest_bytes(bytes))
}

fn validate_scope(scope: &str) -> Result<(), Policy2ReceiptError> {
    if scope.is_empty()
        || scope.len() > MAX_STRING_BYTES
        || scope.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(Policy2ReceiptError::InvalidIssuerScope);
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), ()> {
    Digest::parse(value).map(|_| ()).map_err(|_| ())
}

fn canonicalize_digests(values: &mut Vec<String>) -> Result<(), Policy2ReceiptError> {
    if values.len() > MAX_ROOTS || values.iter().any(|value| validate_digest(value).is_err()) {
        return Err(Policy2ReceiptError::InvalidTrustStore);
    }
    values.sort();
    values.dedup();
    Ok(())
}

fn trust_store_digest(entries: &BTreeMap<String, Policy2TrustEntry>, epoch: u64) -> String {
    let mut bytes = Vec::new();
    frame(&mut bytes, TRUST_STORE_DOMAIN);
    number(&mut bytes, epoch);
    number(&mut bytes, entries.len() as u64);
    for entry in entries.values() {
        bytes.push(entry.kind.code());
        frame(&mut bytes, entry.key_id.as_bytes());
        frame(&mut bytes, &entry.public_key);
        frame(&mut bytes, entry.scope.as_bytes());
        number(&mut bytes, entry.allowed_policy_digests.len() as u64);
        for digest in &entry.allowed_policy_digests {
            frame(&mut bytes, digest.as_bytes());
        }
        number(&mut bytes, entry.allowed_verifier_builds.len() as u64);
        for digest in &entry.allowed_verifier_builds {
            frame(&mut bytes, digest.as_bytes());
        }
        match entry.revoked_at_epoch {
            Some(value) => {
                bytes.push(1);
                number(&mut bytes, value);
            }
            None => bytes.push(0),
        }
    }
    digest_bytes(&bytes)
}

fn frame(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value);
}

fn number(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum Policy2ReceiptError {
    #[error("policy-2 receipt cannot be decoded: {message}")]
    Decode { message: String },
    #[error("policy-2 receipt or trust configuration cannot be encoded: {0}")]
    Encode(String),
    #[error("accepted main cannot be decoded: {0}")]
    MainDocument(String),
    #[error("resolved import cannot be bound: {0}")]
    ResolvedImport(String),
    #[error("policy-2 acceptance requires the canonical compact stable-v1 main encoding")]
    NonCanonicalMain,
    #[error("policy-2 receipt JSON is not canonical")]
    NonCanonicalReceipt,
    #[error("policy-2 receipt is obsolete or attempts a policy downgrade")]
    ObsoletePolicy,
    #[error("invalid certification binding {field}")]
    InvalidBinding { field: &'static str },
    #[error("receipt does not bind the current {field}")]
    BindingMismatch { field: &'static str },
    #[error("receipt issuer scope is invalid")]
    InvalidIssuerScope,
    #[error("receipt provenance does not match its acquisition channel")]
    ProvenanceMismatch,
    #[error("receipt signature algorithm is unsupported")]
    UnsupportedAlgorithm,
    #[error("receipt issuer is not in the configured trust store")]
    UntrustedIssuer,
    #[error("receipt issuer is revoked")]
    RevokedIssuer,
    #[error("receipt violates configured policy, verifier, kind, or scope constraints")]
    TrustConstraint,
    #[error("receipt key ID, public key, or issuer kind is confused")]
    KeyConfusion,
    #[error("receipt signature encoding is noncanonical")]
    NonCanonicalSignature,
    #[error("receipt signature is invalid")]
    InvalidSignature,
    #[error("receipt trust store is empty, duplicate, or invalid")]
    InvalidTrustStore,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedPolicy2Catalog {
    pub main_path: PathBuf,
    pub receipt_path: PathBuf,
    pub catalog_path: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogDocument<'a> {
    format: &'static str,
    catalog_version: u16,
    contracts: [CatalogEntry<'a>; 1],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogEntry<'a> {
    document: &'a str,
    document_digest: &'a str,
    receipt: &'a str,
    receipt_digest: &'a str,
    bindings: &'a Policy2ReceiptBindings,
    status: CatalogStatus,
    import: &'a ResolvedImport,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum CatalogStatus {
    Policy2PersistentLocal,
    Policy2Portable,
}

/// Publishes content-addressed objects first and commits visibility with one
/// atomic catalog-pointer rename. Existing objects are reused only after an
/// exact content check; a crash can leave unreachable blobs but never a
/// pointer to a partial pair.
pub fn publish_policy2_catalog(
    root: &Path,
    canonical_main: &[u8],
    receipt: &[u8],
    authenticated: &AuthenticatedPolicy2Receipt,
    resolved_import: &ResolvedImport,
) -> Result<PublishedPolicy2Catalog, ReceiptPublicationError> {
    let (_, normalized) = validate_canonical_main(canonical_main)
        .map_err(|error| ReceiptPublicationError::Unauthenticated(error.to_string()))?;
    if authenticated.receipt_digest != digest_bytes(receipt)
        || authenticated.main_digest != digest_bytes(canonical_main)
        || authenticated.semantic_digest.as_str() != normalized.semantic_digest().as_str()
    {
        return Err(ReceiptPublicationError::Unauthenticated(
            "authenticated receipt token does not bind the publication bytes".into(),
        ));
    }
    if authenticated.bindings.importer != resolved_import.importer
        || authenticated.bindings.specifier != resolved_import.specifier
    {
        return Err(ReceiptPublicationError::Unauthenticated(
            "authenticated receipt token does not bind the published importer/specifier".into(),
        ));
    }
    let status = match authenticated.issuer_kind {
        ReceiptIssuerKind::PersistentLocal => CatalogStatus::Policy2PersistentLocal,
        ReceiptIssuerKind::Portable => CatalogStatus::Policy2Portable,
        ReceiptIssuerKind::BuiltIn => {
            return Err(ReceiptPublicationError::Unauthenticated(
                "built-in receipts cannot be published through a project catalog".into(),
            ));
        }
    };
    fs::create_dir_all(root).map_err(publication_io)?;
    let objects = root.join("objects");
    fs::create_dir_all(&objects).map_err(publication_io)?;
    let main_digest = digest_bytes(canonical_main);
    let receipt_digest = digest_bytes(receipt);
    let main_name = format!("{}.main.json", digest_hex(&main_digest));
    let receipt_name = format!("{}.receipt.json", digest_hex(&receipt_digest));
    let main_path = objects.join(&main_name);
    let receipt_path = objects.join(&receipt_name);
    store_content_object(&main_path, canonical_main)?;
    store_content_object(&receipt_path, receipt)?;

    let object_prefix = if root
        .file_name()
        .is_some_and(|name| name == ".solid-checker")
    {
        ".solid-checker/objects"
    } else {
        "objects"
    };
    let main_relative = format!("{object_prefix}/{main_name}");
    let receipt_relative = format!("{object_prefix}/{receipt_name}");
    let mut pointer = serde_json::to_vec(&CatalogDocument {
        format: "solid-checker-accepted-contract-catalog",
        catalog_version: 2,
        contracts: [CatalogEntry {
            document: &main_relative,
            document_digest: &main_digest,
            receipt: &receipt_relative,
            receipt_digest: &receipt_digest,
            bindings: &authenticated.bindings,
            status,
            import: resolved_import,
        }],
    })
    .map_err(|error| ReceiptPublicationError::Io(error.to_string()))?;
    pointer.push(b'\n');
    let catalog_path = root.join("accepted-contracts.json");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ReceiptPublicationError::Io(error.to_string()))?
        .as_nanos();
    let temporary = root.join(format!(
        ".accepted-contracts.{}.{}.tmp",
        std::process::id(),
        nonce
    ));
    write_new_synced(&temporary, &pointer)?;
    fs::rename(&temporary, &catalog_path).map_err(publication_io)?;
    File::open(root)
        .and_then(|directory| directory.sync_all())
        .map_err(publication_io)?;
    Ok(PublishedPolicy2Catalog {
        main_path,
        receipt_path,
        catalog_path,
    })
}

fn store_content_object(path: &Path, bytes: &[u8]) -> Result<(), ReceiptPublicationError> {
    match fs::read(path) {
        Ok(existing) if existing == bytes => return Ok(()),
        Ok(_) => return Err(ReceiptPublicationError::ContentAddressCollision),
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(publication_io(error));
        }
        Err(_) => {}
    }
    match write_new_synced(path, bytes) {
        Err(ReceiptPublicationError::Io(_)) if path.exists() => match fs::read(path) {
            Ok(existing) if existing == bytes => Ok(()),
            Ok(_) => Err(ReceiptPublicationError::ContentAddressCollision),
            Err(error) => Err(publication_io(error)),
        },
        result => result,
    }
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), ReceiptPublicationError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(publication_io)?;
    file.write_all(bytes).map_err(publication_io)?;
    file.sync_all().map_err(publication_io)
}

fn digest_hex(value: &str) -> &str {
    value
        .strip_prefix("sha256:")
        .expect("local digest is SHA-256")
}

fn publication_io(error: std::io::Error) -> ReceiptPublicationError {
    ReceiptPublicationError::Io(error.to_string())
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ReceiptPublicationError {
    #[error("policy-2 receipt publication failed: {0}")]
    Io(String),
    #[error("content-addressed policy-2 object path contains different bytes")]
    ContentAddressCollision,
    #[error("policy-2 publication lacks an authenticated exact receipt: {0}")]
    Unauthenticated(String),
}

#[cfg(test)]
mod tests;
