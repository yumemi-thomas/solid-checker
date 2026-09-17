//! Authorizing a contract that a *fixture* supplies, so a corpus can analyze a
//! consumer against an **accepted** contract.
//!
//! # Why this exists
//!
//! Every catalog committed under `fixtures/` carries `status:
//! "obsolete-policy1"`. A policy-1 catalog is rejected before a single claim is
//! read, so every contract-consumer fixture in the repository pins the
//! *rejection* path and nothing downstream of it. Three separate claims now
//! rest on that gap — a parameter-relative `returns`, ADR 0109's `merged-props`
//! consumer arm, and the ownership filter's `SC4001`-versus-`source: created`
//! pair — and each was recorded as correct-by-construction because no fixture
//! could supply the premise.
//!
//! # What is minted, and what is not
//!
//! Only the **authorization** is minted here. The *resolution* — the closure
//! manifest, the export bindings, the package integrity — is the fixture's own
//! hand-written answer, exactly as it has always been, and it is reused byte for
//! byte. This replaces a rejected receipt with an authenticated one; it does not
//! invent a resolution.
//!
//! The issuer is test-scoped in the one way that matters: **the trust
//! configuration is returned to the caller, not written into the project.** A
//! project cannot nominate its own issuer (`contract_interface.rs`: "Trust bytes
//! are deliberately not referenced by the project catalog"), so an authorized
//! tree analyzed without `--receipt-trust-configuration` still refuses the
//! catalog. That is what keeps a fixed signing seed from being a forgery: the
//! signature proves nothing until a verifier is separately told to trust the key,
//! and nothing in a checked-in fixture can do the telling.
//!
//! Every receipt binding below a `closed_claims_root` is a shape-valid stand-in
//! for a root whose authority would come from the verifier sessions of a real
//! certification. `Policy2ReceiptBindings` validates shape only, by design. The
//! closed-claims root is the exception and is rebound from the canonical
//! document itself, so an issuer here cannot assert a closure the contract does
//! not carry.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

use crate::{
    ConfiguredReceiptIssuer, Policy2ReceiptBindings, Policy2ReceiptProvenance,
    RECEIPT_WITNESS_FAMILIES, ResolvedImport, authenticate_policy2_receipt,
    canonicalize_policy2_main, encode_policy2_trust_configuration, issue_policy2_receipt,
    policy2_artifact_acceptance_root, policy2_main_closed_claims_root,
    policy2_main_semantic_digest, policy2_resolved_import_root,
    policy2_trust_configuration_for_issuer, publish_policy2_catalog,
};

/// The request a fixture writes when it wants its contract authorized.
pub const AUTHORIZATION_REQUEST: &str = ".solid-checker/authorize-contract.json";
/// The catalog the checker reads.
pub const CATALOG: &str = ".solid-checker/accepted-contracts.json";

/// Scope of the issuer minted here. It names the corpus, not a vendor: a trust
/// configuration carrying it is a statement that the reader is a fixture gate.
pub const FIXTURE_ISSUER_SCOPE: &str = "solid-checker-fixture";
/// The signing seed. Fixed, and safe to be fixed — see the module comment: a
/// signature is inert until a verifier is told out of band to trust the key.
pub const FIXTURE_ISSUER_SEED: [u8; 32] = [7u8; 32];

#[derive(Debug, thiserror::Error)]
pub enum FixtureAuthorizationError {
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Malformed(String),
    /// The tree states no contract to authorize. Distinct from `Malformed` so a
    /// corpus can tell "this fixture did not ask" from "this fixture asked
    /// wrongly", and never treat the second as the first.
    #[error("no contract to authorize: neither {AUTHORIZATION_REQUEST} nor a policy-1 {CATALOG}")]
    NoRequest,
    #[error("{0}")]
    Refused(String),
}

fn io(error: &std::io::Error, path: &Path) -> FixtureAuthorizationError {
    FixtureAuthorizationError::Io(format!("{}: {error}", path.display()))
}

/// A fixture's own answer to "what does this import resolve to", read from the
/// tree and rebased onto it, plus the document that answer selects.
///
/// Separated from [`authorize_fixture_contract`] so a caller can edit the
/// document between reading the request and signing over it — which is how the
/// closure-process tests ask what a *partially* closed contract is worth.
pub struct FixtureContractRequest {
    /// Absolute path to the contract document the catalog will point at.
    pub document: PathBuf,
    resolved: ResolvedImport,
}

impl FixtureContractRequest {
    #[must_use]
    pub fn specifier(&self) -> &str {
        &self.resolved.specifier
    }

    #[must_use]
    pub fn importer(&self) -> &str {
        &self.resolved.importer
    }
}

/// What a caller needs to analyze the authorized project.
pub struct FixtureAuthorization {
    pub document: PathBuf,
    pub specifier: String,
    pub importer: String,
    pub issuer_scope: String,
    pub key_id: String,
    /// Encoded trust configuration. The caller writes this **outside** the
    /// project and passes it to the checker as `--receipt-trust-configuration`;
    /// writing it into the analyzed tree would not help, because the catalog
    /// never references trust bytes.
    pub trust_configuration: Vec<u8>,
}

/// Reads a fixture's authorization request.
///
/// Two spellings are accepted, and they mean different things:
///
/// - `.solid-checker/authorize-contract.json` — `{"document": …, "import": …}`.
///   A fixture that ships this has **no** catalog, so un-authorized it behaves
///   as a project with no accepted contract, which is the truthful baseline for
///   a new fixture.
/// - a one-entry `.solid-checker/accepted-contracts.json` whose status is
///   `obsolete-policy1`. This is the sixteen fixtures that predate the policy-2
///   cut; their `import` block is a valid resolver answer and only the
///   authorization is obsolete.
///
/// A catalog carrying more than one contract is refused rather than
/// hand-assembled: [`publish_policy2_catalog`] writes a catalog holding exactly
/// one, and assembling a multi-entry one here would be this code asserting its
/// own idea of the on-disk shape.
pub fn read_fixture_contract_request(
    project: &Path,
) -> Result<FixtureContractRequest, FixtureAuthorizationError> {
    let request = project.join(AUTHORIZATION_REQUEST);
    let (document, mut import) = if request.is_file() {
        let bytes = fs::read(&request).map_err(|error| io(&error, &request))?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
            FixtureAuthorizationError::Malformed(format!("{request:?}: {error}"))
        })?;
        let document = value
            .get("document")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                FixtureAuthorizationError::Malformed(format!(
                    "{}: \"document\" must be a package-relative path",
                    request.display()
                ))
            })?
            .to_owned();
        let import = value.get("import").cloned().ok_or_else(|| {
            FixtureAuthorizationError::Malformed(format!(
                "{}: \"import\" must be a resolved-import object",
                request.display()
            ))
        })?;
        (document, import)
    } else {
        let catalog_path = project.join(CATALOG);
        if !catalog_path.is_file() {
            return Err(FixtureAuthorizationError::NoRequest);
        }
        let bytes = fs::read(&catalog_path).map_err(|error| io(&error, &catalog_path))?;
        let catalog: Value = serde_json::from_slice(&bytes).map_err(|error| {
            FixtureAuthorizationError::Malformed(format!("{}: {error}", catalog_path.display()))
        })?;
        let contracts = catalog["contracts"].as_array().ok_or_else(|| {
            FixtureAuthorizationError::Malformed(format!(
                "{}: \"contracts\" must be an array",
                catalog_path.display()
            ))
        })?;
        if contracts.len() != 1 {
            return Err(FixtureAuthorizationError::Refused(format!(
                "catalog publishes {} contracts; publication writes one",
                contracts.len()
            )));
        }
        let entry = &contracts[0];
        if entry["status"] != "obsolete-policy1" {
            return Err(FixtureAuthorizationError::Refused(format!(
                "catalog status is {}",
                entry["status"]
            )));
        }
        let document = entry["document"]
            .as_str()
            .ok_or_else(|| {
                FixtureAuthorizationError::Malformed(format!(
                    "{}: \"document\" must be a path",
                    catalog_path.display()
                ))
            })?
            .to_owned();
        (document, entry["import"].clone())
    };

    absolutize(&mut import, project);
    let resolved: ResolvedImport = serde_json::from_value(import).map_err(|error| {
        FixtureAuthorizationError::Malformed(format!(
            "\"import\" is not a resolved import: {error}"
        ))
    })?;
    resolved
        .validate()
        .map_err(|error| FixtureAuthorizationError::Malformed(format!("{error}")))?;
    Ok(FixtureContractRequest {
        document: project.join(document),
        resolved,
    })
}

/// Mints a policy-2 receipt over the requested document and publishes it as the
/// project's catalog, replacing whatever pointer was there.
///
/// The returned trust configuration is the other half: without it the published
/// catalog authenticates against nothing and the analysis refuses, which is the
/// property `an_authorized_catalog_still_refuses_without_the_trust_configuration`
/// pins.
pub fn authorize_fixture_contract(
    project: &Path,
    request: &FixtureContractRequest,
) -> Result<FixtureAuthorization, FixtureAuthorizationError> {
    let bytes = fs::read(&request.document).map_err(|error| io(&error, &request.document))?;
    let canonical_main = canonicalize_policy2_main(&bytes)
        .map_err(|error| FixtureAuthorizationError::Malformed(format!("{error}")))?;

    let resolved = &request.resolved;
    let refused =
        |error: crate::Policy2ReceiptError| FixtureAuthorizationError::Refused(format!("{error}"));
    let bindings = Policy2ReceiptBindings {
        importer: resolved.importer.clone(),
        specifier: resolved.specifier.clone(),
        resolved_import_root: policy2_resolved_import_root(resolved).map_err(refused)?,
        artifact_acceptance_root: policy2_artifact_acceptance_root(resolved, &["import".into()])
            .map_err(refused)?,
        semantic_digest: policy2_main_semantic_digest(&canonical_main).map_err(refused)?,
        artifact_provenance_root: stand_in(1),
        snapshot_root: stand_in(2),
        package_root: stand_in(3),
        manifest_root: stand_in(4),
        artifacts_root: stand_in(5),
        declarations_root: stand_in(6),
        transform_root: stand_in(7),
        exports_root: stand_in(8),
        closure_root: stand_in(9),
        demand_graph_root: stand_in(10),
        verified_positive_root: stand_in(11),
        witness_roots: RECEIPT_WITNESS_FAMILIES
            .iter()
            .enumerate()
            .map(|(index, family)| {
                (
                    (*family).to_owned(),
                    stand_in(u16::try_from(100 + index).unwrap_or(u16::MAX)),
                )
            })
            .collect(),
        producer_sessions_root: stand_in(12),
        dependency_receipts_root: stand_in(13),
        dependency_trust_root: stand_in(14),
        probe_gate_root: stand_in(15),
        // Rebound from the document: a fixture issuer cannot assert a closure
        // the contract it signs does not carry.
        closed_claims_root: policy2_main_closed_claims_root(&canonical_main).map_err(refused)?,
        verifier_source_digest: stand_in(17),
        verifier_build_digest: stand_in(18),
    };

    let issuer =
        ConfiguredReceiptIssuer::persistent_local(FIXTURE_ISSUER_SCOPE, FIXTURE_ISSUER_SEED)
            .map_err(refused)?;
    let receipt = issue_policy2_receipt(&canonical_main, &bindings, &issuer).map_err(refused)?;
    let trust = policy2_trust_configuration_for_issuer(&issuer, &bindings.verifier_build_digest, 0)
        .map_err(refused)?;
    let authenticated = authenticate_policy2_receipt(
        &canonical_main,
        &receipt,
        &bindings,
        Policy2ReceiptProvenance::PersistentLocal {
            trust_store: trust.trust_store(),
            scope: issuer.scope(),
        },
    )
    .map_err(refused)?;

    // Publication replaces the catalog atomically; an obsolete pointer must be
    // gone rather than merged with.
    let catalog_path = project.join(CATALOG);
    if catalog_path.exists() {
        fs::remove_file(&catalog_path).map_err(|error| io(&error, &catalog_path))?;
    }
    publish_policy2_catalog(
        &project.join(".solid-checker"),
        &canonical_main,
        &receipt,
        &authenticated,
        resolved,
        // The same set `artifact_acceptance_root` was computed over above.
        std::slice::from_ref(&"import".to_owned()),
    )
    .map_err(|error| FixtureAuthorizationError::Refused(format!("{error}")))?;

    Ok(FixtureAuthorization {
        document: request.document.clone(),
        specifier: resolved.specifier.clone(),
        importer: resolved.importer.clone(),
        issuer_scope: issuer.scope().to_owned(),
        key_id: issuer.key_id().to_owned(),
        trust_configuration: encode_policy2_trust_configuration(&trust).map_err(refused)?,
    })
}

/// Shape-valid stand-in for a root whose authority a real certification would
/// carry from a verifier session.
fn stand_in(index: u16) -> String {
    format!("sha256:{index:064x}")
}

/// The stored request spells every path relative to the project, which is what
/// makes a fixture relocatable; `ResolvedImport::validate` requires absolute
/// ones, and the receipt binds the importer the consumer will itself compute.
/// Rewrite exactly the project-relative fields, leaving package-relative closure
/// entries alone.
fn absolutize(import: &mut Value, project: &Path) {
    let at = |value: &Value| -> Option<String> {
        Some(project.join(value.as_str()?).to_string_lossy().into_owned())
    };
    for key in ["importer", "packageRoot"] {
        if let Some(absolute) = at(&import[key]) {
            import[key] = absolute.into();
        }
    }
    for key in ["packageManifest", "runtime", "declarations"] {
        if let Some(absolute) = at(&import[key]["path"]) {
            import[key]["path"] = absolute.into();
        }
    }
    let Some(exports) = import["exports"].as_object_mut() else {
        return;
    };
    for binding in exports.values_mut() {
        for axis in ["runtime", "declarations"] {
            if let Some(absolute) = at(&binding[axis]["module"]["path"]) {
                binding[axis]["module"]["path"] = absolute.into();
            }
        }
    }
}
