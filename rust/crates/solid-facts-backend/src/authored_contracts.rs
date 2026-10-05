//! The authored contract tier (ADR 0198, implementing ADR 0189).
//!
//! An authored contract is a one-case contract document the repository ships
//! and reviews, plus an index entry. It carries no certification receipt: its
//! authority is repository review, and every claim it states has a probe case
//! run on the published package at the listed version and Solid runtime.
//!
//! Applicability is the one admission rule, [`admit_by_artifact`], over the
//! entry's package identity and published-files snapshot, with an environment
//! of exactly the Solid runtime the claims were probed on. The rest of the
//! dependency tree is not compared (ADR 0189 § 3).
//!
//! [`admit_by_artifact`]: crate::accepted_bundles::admit_by_artifact

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::Deserialize;
use solid_reactive_ir::contract_semantics::proof::accept_authored;
use solid_reactive_ir::contract_semantics::{AcceptedContract, AcceptedContractIndex, Digest};

use crate::accepted_bundles::{
    ArtifactAcceptance, EnvironmentRule, InstalledArtifactBytes, InstalledEnvironment,
    admit_by_artifact,
};
use crate::contract_certification::{
    DependencyEnvironmentEntry, policy2_artifact_acceptance_root_for_identity,
};
use crate::contract_interface::{
    ContractFailure, InstalledArtifactIdentity, ResolvedTargetIdentity,
};

mod embedded;

/// The verifier build an authored receipt names. An authored contract is not
/// verified by a build; the string only keeps the identity stable.
const AUTHORED_VERIFIER: &str = "solid-checker:authored-contracts";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuthoredIndex {
    format: u32,
    entries: Vec<AuthoredEntry>,
}

/// One authored contract: one artifact case of one package version.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AuthoredEntry {
    pub(crate) package_name: String,
    pub(crate) specifier: String,
    pub(crate) package_version: String,
    pub(crate) package_integrity: String,
    pub(crate) requested_entrypoint: String,
    pub(crate) export_conditions: Vec<String>,
    pub(crate) runtime_target: String,
    pub(crate) declaration_target: String,
    /// The snapshot root of the published files, which the installed files
    /// must reproduce.
    pub(crate) snapshot_root: String,
    /// The Solid runtime the claims were probed on, which the consumer's tree
    /// must install as stated (resolved from the package's own location).
    pub(crate) solid_runtime: Vec<SolidRuntimeEntry>,
    /// The document's member path among the embedded objects.
    pub(crate) document: String,
    pub(crate) document_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SolidRuntimeEntry {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) integrity: String,
}

/// One loaded authored contract.
pub(crate) struct LoadedAuthored {
    pub(crate) entry: AuthoredEntry,
    pub(crate) acceptance_root: String,
    pub(crate) environment: Vec<DependencyEnvironmentEntry>,
    pub(crate) identity: String,
    pub(crate) contract: AcceptedContract,
}

/// The index entry identity an authored contract is keyed and admitted by.
/// Prefixed so it can never collide with a certified acceptance's.
fn authored_identity(acceptance_root: &str, environment: &[DependencyEnvironmentEntry]) -> String {
    format!(
        "authored:{}",
        crate::accepted_bundles::environment_acceptance_identity(acceptance_root, environment)
    )
}

pub(crate) fn load_authored(
    entry: &AuthoredEntry,
    document: &[u8],
) -> Result<LoadedAuthored, ContractFailure> {
    // ADR 0027: the runtime foundation is the dialect's, never a package
    // contract's.
    if solid_dialect::primitive_defining_package(&entry.package_name)
        || solid_dialect::core_runtime_contract_reference(&entry.package_name, &entry.specifier)
    {
        return Err(ContractFailure::IdentityMismatch {
            reason: format!(
                "{} is the Solid runtime foundation, which ordinary analysis takes from the \
                 dialect (ADR 0027); it cannot be an authored package contract",
                entry.package_name
            ),
        });
    }
    if crate::contract_interface::sha256_digest(document) != entry.document_digest {
        return Err(ContractFailure::ReceiptMismatch {
            field: "documentDigest",
        });
    }
    if entry.solid_runtime.is_empty() {
        return Err(ContractFailure::IdentityMismatch {
            reason: format!(
                "the authored contract for {}@{} states no Solid runtime its claims were probed on",
                entry.package_name, entry.package_version
            ),
        });
    }
    let normalized = crate::contract_document::decode(document)?.normalize()?;
    let package = normalized.package();
    if package.name != entry.package_name
        || package.version != entry.package_version
        || package.integrity != entry.package_integrity
    {
        return Err(ContractFailure::IdentityMismatch {
            reason: format!(
                "the authored document for {}@{} is about {}@{}",
                entry.package_name, entry.package_version, package.name, package.version
            ),
        });
    }
    let [case] = normalized.artifact_cases() else {
        return Err(ContractFailure::MultipleArtifactCases);
    };
    let case_id = case.id.clone();
    let digest = Digest::parse(entry.document_digest.clone()).map_err(|_| {
        ContractFailure::ReceiptMismatch {
            field: "documentDigest",
        }
    })?;
    let contract =
        accept_authored(normalized, &case_id, digest, AUTHORED_VERIFIER).map_err(|error| {
            ContractFailure::IdentityMismatch {
                reason: format!(
                    "the authored contract for {}@{} cannot be accepted: {error:?}",
                    entry.package_name, entry.package_version
                ),
            }
        })?;
    let acceptance_root = policy2_artifact_acceptance_root_for_identity(
        &entry.package_name,
        &entry.package_version,
        &entry.package_integrity,
        &entry.requested_entrypoint,
        &entry.export_conditions,
    );
    let environment = entry
        .solid_runtime
        .iter()
        .map(|runtime| {
            DependencyEnvironmentEntry::package(&runtime.name, &runtime.version, &runtime.integrity)
        })
        .collect::<Vec<_>>();
    Ok(LoadedAuthored {
        identity: authored_identity(&acceptance_root, &environment),
        entry: entry.clone(),
        acceptance_root,
        environment,
        contract,
    })
}

fn parse_index(bytes: &[u8]) -> Result<AuthoredIndex, ContractFailure> {
    let index: AuthoredIndex =
        serde_json::from_slice(bytes).map_err(|error| ContractFailure::DocumentDecode {
            message: format!("the authored contract index does not parse: {error}"),
        })?;
    if index.format != 1 {
        return Err(ContractFailure::DocumentDecode {
            message: format!(
                "the authored contract index is format {}, not 1",
                index.format
            ),
        });
    }
    Ok(index)
}

pub(crate) fn load_index(
    index: &[u8],
    object: impl Fn(&str) -> Option<&'static [u8]>,
) -> Result<Vec<LoadedAuthored>, ContractFailure> {
    let index = parse_index(index)?;
    let mut seen = BTreeSet::new();
    index
        .entries
        .iter()
        .map(|entry| {
            let document =
                object(&entry.document).ok_or_else(|| ContractFailure::DocumentDecode {
                    message: format!(
                        "the authored contract index names {:?}, which this build does not carry",
                        entry.document
                    ),
                })?;
            let loaded = load_authored(entry, document)?;
            if !seen.insert(loaded.identity.clone()) {
                return Err(ContractFailure::IdentityMismatch {
                    reason: format!(
                        "two authored contracts for {}@{} share one artifact and runtime",
                        entry.package_name, entry.package_version
                    ),
                });
            }
            Ok(loaded)
        })
        .collect()
}

fn authored() -> Result<&'static [LoadedAuthored], ContractFailure> {
    static LOADED: OnceLock<Result<Vec<LoadedAuthored>, String>> = OnceLock::new();
    LOADED
        .get_or_init(|| {
            load_index(embedded::INDEX, |member| {
                embedded::OBJECTS
                    .iter()
                    .find_map(|(name, bytes)| (*name == member).then_some(*bytes))
            })
            .map_err(|error| error.to_string())
        })
        .as_deref()
        .map_err(|message| ContractFailure::DocumentDecode {
            message: message.clone(),
        })
}

/// Every compiled-in authored contract, reachable only by the artifact and
/// Solid runtime it states. Fold it in above the certified tier.
pub fn compiled_in_authored_contracts() -> Result<AcceptedContractIndex, ContractFailure> {
    Ok(AcceptedContractIndex::from_artifact_acceptances(
        authored()?
            .iter()
            .map(|loaded| (loaded.identity.clone(), loaded.contract.clone())),
    ))
}

pub(crate) fn admitted_from(
    loaded: &[LoadedAuthored],
    conditions: &BTreeSet<String>,
    installed_integrity: &InstalledArtifactIdentity,
    installed_bytes: &InstalledArtifactBytes,
    resolved_target: &ResolvedTargetIdentity,
    installed_environment: &InstalledEnvironment,
) -> Vec<(String, String)> {
    admit_by_artifact(
        loaded.iter().map(|authored| ArtifactAcceptance {
            specifier: &authored.entry.specifier,
            requested_entrypoint: &authored.entry.requested_entrypoint,
            export_conditions: &authored.entry.export_conditions,
            runtime_target: &authored.entry.runtime_target,
            declaration_target: &authored.entry.declaration_target,
            acceptance_root: &authored.acceptance_root,
            snapshot_root: &authored.entry.snapshot_root,
            environment: Some(&authored.environment),
            identity: &authored.identity,
            citations: &[],
        }),
        conditions,
        installed_integrity,
        installed_bytes,
        resolved_target,
        installed_environment,
        EnvironmentRule::Exact,
    )
}

/// Which specifiers this project may import under an authored contract.
pub fn admitted_authored_artifacts(
    conditions: &BTreeSet<String>,
    installed_integrity: &InstalledArtifactIdentity,
    installed_bytes: &InstalledArtifactBytes,
    resolved_target: &ResolvedTargetIdentity,
    installed_environment: &InstalledEnvironment,
) -> Result<Vec<(String, String)>, ContractFailure> {
    Ok(admitted_from(
        authored()?,
        conditions,
        installed_integrity,
        installed_bytes,
        resolved_target,
        installed_environment,
    ))
}

#[cfg(test)]
mod tests;
