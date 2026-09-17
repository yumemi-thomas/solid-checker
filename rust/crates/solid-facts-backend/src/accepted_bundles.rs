//! The compiled-in accepted-contract tier.
//!
//! A user who never runs `contract certify` has no accepted contract for any
//! dependency, so every import of an external Solid package stops at the
//! acceptance gate and nothing the contract says about it is ever read. This
//! module is the other supply: contracts this repository certified, reviewed
//! and compiled into the checker.
//!
//! **What makes a bundle applicable is the artifact, not the importer.**
//! `policy2_artifact_acceptance_root` commits to a package's name, version,
//! tarball integrity, requested entrypoint and sorted export conditions — and
//! to no importer and no path. A project whose installed bytes reproduce that
//! root demonstrably resolved the same published artifact the contract was
//! proven about, whatever file imported it. So a bundle feeds
//! `AcceptedContractIndex` through `from_artifact_acceptances`, is reachable
//! only through [`admitted_bundle_artifacts`], and appears in no report about
//! what this project imported.
//!
//! **Where this is not.** `first_party_bundles` is the retired policy-1 seam
//! for the Solid runtime foundation, and ADR 0027 keeps that foundation out of
//! package contracts entirely: ordinary analysis takes `solid-js`,
//! `@solidjs/signals` and `@solidjs/web` from the dialect. A contract about one
//! of those is refused here rather than loaded.
//!
//! **What the compiled-in authority actually is.** The receipt is issued with
//! `ReceiptIssuerKind::BuiltIn`, whose authentication compares the receipt's
//! own digest against a compiled-in entry digest — so the authority is that
//! these bytes are in this repository and were reviewed, exactly as it is for
//! any other `include_bytes!`. The entry digest is a consistency check on the
//! pair, not an independent second witness, and this module does not claim
//! otherwise. What it does establish independently is applicability: the
//! acceptance root is recomputed from the consumer's *own* installed tree, and
//! a bundle whose root does not reproduce is not admitted.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use serde::Deserialize;
use solid_reactive_ir::contract_semantics::AcceptedContractIndex;

use crate::{
    contract_certification::{
        BuiltInReceiptEntry, Policy2ReceiptBindings, policy2_artifact_acceptance_root_for_identity,
    },
    contract_interface::{
        AuthenticCase, ContractFailure, InstalledArtifactIdentity, ResolvedTargetIdentity,
        admissible_cases, declared_conditions, load_authenticated_policy2_embedded_contract,
    },
};

mod embedded;

#[cfg(test)]
mod tests;

const BUNDLE_INDEX_FORMAT: &str = "solid-checker-accepted-contract-bundle-index";
const BUNDLE_INDEX_VERSION: u16 = 1;

/// The reviewed index. Compiled in beside the objects it names, so a build
/// carries exactly the bundles the repository does.
const INDEX_BYTES: &[u8] = include_bytes!("../../../../pkg/contracts/accepted/index.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BundleIndexDocument {
    format: String,
    bundle_index_version: u16,
    bundles: Vec<BundleEntry>,
}

/// One compiled-in acceptance, described by exactly what a consumer can
/// recompute about its own installed tree.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BundleEntry {
    package_name: String,
    package_version: String,
    package_integrity: String,
    /// The specifier a consumer writes. Admission is keyed by it, because that
    /// is what an import states and what the installed-tree lookups take.
    specifier: String,
    requested_entrypoint: String,
    export_conditions: Vec<String>,
    /// Package-relative, because the certifier's absolute paths are on another
    /// machine. These are what [`AuthenticCase::reaches`] compares against the
    /// file this project resolved.
    runtime_target: String,
    #[serde(default)]
    declaration_target: String,
    document: String,
    document_digest: String,
    receipt: String,
    receipt_digest: String,
    bindings: Policy2ReceiptBindings,
}

struct LoadedBundle {
    specifier: String,
    requested_entrypoint: String,
    export_conditions: Vec<String>,
    runtime_target: String,
    declaration_target: String,
    identity: String,
    contract: solid_reactive_ir::contract_semantics::AcceptedContract,
}

fn bundles() -> Result<&'static [LoadedBundle], ContractFailure> {
    static LOADED: OnceLock<Result<Vec<LoadedBundle>, ContractFailure>> = OnceLock::new();
    match LOADED.get_or_init(load_bundles) {
        Ok(bundles) => Ok(bundles.as_slice()),
        Err(error) => Err(error.clone()),
    }
}

fn load_bundles() -> Result<Vec<LoadedBundle>, ContractFailure> {
    let index: BundleIndexDocument =
        serde_json::from_slice(INDEX_BYTES).map_err(|error| ContractFailure::DocumentDecode {
            message: format!("decode the compiled-in accepted-contract index: {error}"),
        })?;
    if index.format != BUNDLE_INDEX_FORMAT || index.bundle_index_version != BUNDLE_INDEX_VERSION {
        return Err(ContractFailure::DocumentDecode {
            message: format!(
                "the compiled-in accepted-contract index must use format \
                 {BUNDLE_INDEX_FORMAT:?} version {BUNDLE_INDEX_VERSION}"
            ),
        });
    }
    index
        .bundles
        .iter()
        .map(|entry| {
            let document = object(&entry.document)?;
            let receipt = object(&entry.receipt)?;
            load_bundle(entry, document, receipt)
        })
        .collect()
}

fn load_bundle(
    entry: &BundleEntry,
    document: &[u8],
    receipt: &[u8],
) -> Result<LoadedBundle, ContractFailure> {
    // ADR 0027: the runtime foundation is the dialect's, and a package contract
    // about it must never reach ordinary analysis. Refused here rather than
    // filtered later, so a bundle set that names one fails the build's own
    // check instead of being quietly dropped.
    if solid_dialect::primitive_defining_package(&entry.package_name)
        || solid_dialect::core_runtime_contract_reference(&entry.package_name, &entry.specifier)
    {
        return Err(ContractFailure::IdentityMismatch {
            reason: format!(
                "{} is the Solid runtime foundation, which ordinary analysis takes from the \
                 dialect (ADR 0027); it cannot be supplied as a bundled package contract",
                entry.package_name
            ),
        });
    }
    // The specifier a consumer writes and the specifier the receipt binds are
    // the same string, or admission would key on one and authenticate the
    // other.
    if entry.specifier != entry.bindings.specifier {
        return Err(ContractFailure::ReceiptMismatch { field: "specifier" });
    }
    verify_object_digest(document, &entry.document_digest, "documentDigest")?;
    verify_object_digest(receipt, &entry.receipt_digest, "receiptDigest")?;
    let built_in = BuiltInReceiptEntry {
        entry_digest: entry.receipt_digest.clone(),
        verifier_build_digest: entry.bindings.verifier_build_digest.clone(),
    };
    let contract = load_authenticated_policy2_embedded_contract(
        document,
        receipt,
        &entry.bindings,
        &built_in,
    )?;
    // The whole applicability premise: the five identity fields the index
    // states must be the ones the receipt signed. A bundle whose stated
    // identity does not reproduce its own signed root could be admitted for an
    // artifact it was never proven about.
    let identity = policy2_artifact_acceptance_root_for_identity(
        &entry.package_name,
        &entry.package_version,
        &entry.package_integrity,
        &entry.requested_entrypoint,
        &entry.export_conditions,
    );
    if identity != entry.bindings.artifact_acceptance_root {
        return Err(ContractFailure::ReceiptMismatch {
            field: "artifactAcceptanceRoot",
        });
    }
    Ok(LoadedBundle {
        specifier: entry.specifier.clone(),
        requested_entrypoint: entry.requested_entrypoint.clone(),
        export_conditions: entry.export_conditions.clone(),
        runtime_target: entry.runtime_target.clone(),
        declaration_target: entry.declaration_target.clone(),
        identity,
        contract,
    })
}

fn object(member: &str) -> Result<&'static [u8], ContractFailure> {
    embedded::OBJECTS
        .iter()
        .find_map(|(name, bytes)| (*name == member).then_some(*bytes))
        .ok_or_else(|| ContractFailure::DocumentDecode {
            message: format!(
                "the compiled-in accepted-contract index names {member:?}, which this build does \
                 not carry; regenerate with `bun scripts/bundle-accepted-contracts.mjs`"
            ),
        })
}

fn verify_object_digest(
    bytes: &[u8],
    expected: &str,
    field: &'static str,
) -> Result<(), ContractFailure> {
    (crate::contract_interface::sha256_digest(bytes) == expected)
        .then_some(())
        .ok_or(ContractFailure::ReceiptMismatch { field })
}

/// Every compiled-in acceptance, reachable only by the artifact it was proven
/// about.
///
/// Fold it in below the project's own catalogs with
/// [`AcceptedContractIndex::with_fallback`]: a project that certified a package
/// itself must keep its own answer, and this tier fills only what it left
/// absent.
pub fn compiled_in_accepted_contracts() -> Result<AcceptedContractIndex, ContractFailure> {
    Ok(AcceptedContractIndex::from_artifact_acceptances(
        bundles()?
            .iter()
            .map(|bundle| (bundle.identity.clone(), bundle.contract.clone())),
    ))
}

/// Which specifiers this project may import under a compiled-in acceptance.
///
/// The twin of `contract_interface::admitted_project_artifacts`, and
/// deliberately the same rule: recompute the acceptance root from the installed
/// tree, keep the cases that reproduce it, and let `select_case` decide which
/// one this project's declaration reaches. The only difference is where the
/// cases come from — a compiled-in index rather than catalogs on disk — because
/// "does this acceptance apply here" must not have two answers.
pub fn admitted_bundle_artifacts(
    conditions: &BTreeSet<String>,
    installed_integrity: &InstalledArtifactIdentity,
    resolved_target: &ResolvedTargetIdentity,
) -> Result<Vec<(String, String)>, ContractFailure> {
    Ok(admitted_from(
        bundles()?,
        conditions,
        installed_integrity,
        resolved_target,
    ))
}

fn admitted_from(
    loaded: &[LoadedBundle],
    conditions: &BTreeSet<String>,
    installed_integrity: &InstalledArtifactIdentity,
    resolved_target: &ResolvedTargetIdentity,
) -> Vec<(String, String)> {
    let declared = declared_conditions(conditions);
    let mut authentic: BTreeMap<String, Vec<AuthenticCase>> = BTreeMap::new();
    for bundle in loaded {
        let Some((name, version, integrity)) = installed_integrity(&bundle.specifier) else {
            continue;
        };
        let derived = policy2_artifact_acceptance_root_for_identity(
            &name,
            &version,
            &integrity,
            &bundle.requested_entrypoint,
            &bundle.export_conditions,
        );
        if derived != bundle.identity {
            continue;
        }
        authentic
            .entry(bundle.specifier.clone())
            .or_default()
            .push(AuthenticCase::from_relative(
                derived,
                bundle.runtime_target.clone(),
                bundle.declaration_target.clone(),
                bundle.export_conditions.clone(),
            ));
    }
    let mut admitted = Vec::new();
    for (specifier, cases) in authentic {
        let Some(target) = resolved_target(&specifier) else {
            continue;
        };
        let reaching = cases
            .iter()
            .filter(|case| case.reaches(&target))
            .collect::<Vec<_>>();
        admitted.extend(
            admissible_cases(&reaching, &declared)
                .into_iter()
                .map(|case| (specifier.clone(), case.identity.clone())),
        );
    }
    admitted
}
