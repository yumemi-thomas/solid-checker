//! Turns a certified, published catalog into a bundle this checker compiles in.
//!
//! A maintainer tool, built by the Makefile and never packaged. It does one
//! transformation and refuses everything else: take a contract that certified
//! under a configured issuer, and re-issue its receipt as a built-in one over
//! the *same* canonical main and the *same* bindings.
//!
//! **What re-issuing does and does not do.** It changes the receipt's
//! provenance, not a single thing the receipt asserts: the document bytes, the
//! semantic digest, the closed-claims root and every witness root are the
//! certification's own. What changes is who vouches for them — a configured
//! Ed25519 issuer the consumer must be told to trust, or this repository,
//! because the bytes are in it and compiled in. Nothing is re-proven here and
//! this module cannot prove anything: it authenticates the source receipt first
//! (`authenticated_catalog_entries`), so material that is not a real
//! certification by this verifier cannot become a bundle.
//!
//! **What it refuses**, each because the compiled-in tier could not honour it:
//!
//! - a contract about the Solid runtime foundation (ADR 0027);
//! - a receipt that states no `artifactAcceptanceRoot` — bundling is
//!   *entirely* artifact matching, and an importer-only acceptance can never
//!   apply to another project;
//! - a document with more than one artifact case, which
//!   `load_authenticated_policy2_embedded_contract` refuses;
//! - an entrypoint whose runtime or declaration file is not inside the package
//!   root, since the compiled-in tier compares package-relative spellings.

use std::path::Path;

use crate::{
    contract_certification::{
        Policy2TrustConfiguration, issue_builtin_policy2_receipt,
        policy2_artifact_acceptance_root_for_identity,
    },
    contract_interface::{
        AuthenticatedCatalogEntry, authenticated_catalog_entries,
        load_authenticated_policy2_embedded_contract, sha256_digest,
    },
};

/// The issuer scope every bundle in this repository carries. Part of the signed
/// payload, so it is also what a receipt says about where it came from.
pub const BUILT_IN_SCOPE: &str = "solid-checker:bundled-accepted-contract";

/// One bundle, ready to be written into `pkg/contracts/accepted/`.
pub struct BundledAcceptance {
    /// The index entry, in the shape `accepted_bundles::BundleEntry` reads.
    pub entry: serde_json::Value,
    pub document: Vec<u8>,
    pub document_digest: String,
    pub receipt: Vec<u8>,
    pub receipt_digest: String,
}

/// Every entry of one published catalog, re-issued as a bundle.
pub fn bundle_published_catalog(
    catalog: &Path,
    trust: &Policy2TrustConfiguration,
) -> Result<Vec<BundledAcceptance>, String> {
    authenticated_catalog_entries(catalog, trust)
        .map_err(|error| format!("{}: {error}", catalog.display()))?
        .iter()
        .map(|entry| {
            bundle_entry(entry).map_err(|reason| {
                format!(
                    "{}: {} at {}: {reason}",
                    catalog.display(),
                    entry.import.package_name,
                    entry.import.requested_entrypoint
                )
            })
        })
        .collect()
}

fn bundle_entry(entry: &AuthenticatedCatalogEntry) -> Result<BundledAcceptance, String> {
    let import = &entry.import;
    if solid_dialect::primitive_defining_package(&import.package_name)
        || solid_dialect::core_runtime_contract_reference(&import.package_name, &import.specifier)
    {
        return Err(
            "the Solid runtime foundation is the dialect's, not a package contract (ADR 0027)"
                .into(),
        );
    }
    if entry.bindings.artifact_acceptance_root.is_empty() {
        return Err(
            "the receipt states no artifactAcceptanceRoot, so no other project could ever match it"
                .into(),
        );
    }
    // What the index will state has to be what the receipt signed, or admission
    // would recompute one identity and authenticate another.
    let identity = policy2_artifact_acceptance_root_for_identity(
        &import.package_name,
        &import.package_version,
        &import.package_integrity,
        &import.requested_entrypoint,
        &entry.export_conditions,
    );
    if identity != entry.bindings.artifact_acceptance_root {
        return Err(format!(
            "the catalog's identity does not reproduce the signed artifactAcceptanceRoot \
             (derived {identity}, signed {})",
            entry.bindings.artifact_acceptance_root
        ));
    }
    let runtime_target = package_relative(&import.package_root, &import.runtime.path)
        .ok_or("the runtime file is not inside the package root")?;
    let declaration_target =
        package_relative(&import.package_root, &import.declarations.path).unwrap_or_default();

    let receipt =
        issue_builtin_policy2_receipt(&entry.canonical_main, &entry.bindings, BUILT_IN_SCOPE)
            .map_err(|error| format!("built-in receipt issuance refused: {error}"))?;
    let document_digest = sha256_digest(&entry.canonical_main);
    let receipt_digest = sha256_digest(&receipt);
    let built_in = crate::contract_certification::BuiltInReceiptEntry {
        entry_digest: receipt_digest.clone(),
        verifier_build_digest: entry.bindings.verifier_build_digest.clone(),
    };
    // The tool proves its own output loads, with the loader the checker will
    // use, before anything is written. A bundle that only fails at a user's
    // first analysis is the failure mode this whole tier has to avoid.
    load_authenticated_policy2_embedded_contract(
        &entry.canonical_main,
        &receipt,
        &entry.bindings,
        &built_in,
    )
    .map_err(|error| format!("the re-issued bundle does not load: {error}"))?;

    let object = |digest: &str, suffix: &str| {
        format!(
            "objects/{}.{suffix}.json",
            digest.strip_prefix("sha256:").unwrap_or(digest)
        )
    };
    Ok(BundledAcceptance {
        entry: serde_json::json!({
            "packageName": import.package_name,
            "packageVersion": import.package_version,
            "packageIntegrity": import.package_integrity,
            "specifier": import.specifier,
            "requestedEntrypoint": import.requested_entrypoint,
            "exportConditions": entry.export_conditions,
            "runtimeTarget": runtime_target,
            "declarationTarget": declaration_target,
            "document": object(&document_digest, "main"),
            "documentDigest": document_digest,
            "receipt": object(&receipt_digest, "receipt"),
            "receiptDigest": receipt_digest,
            "bindings": entry.bindings,
        }),
        document: entry.canonical_main.clone(),
        document_digest,
        receipt,
        receipt_digest,
    })
}

/// The package-relative spelling of an absolute path under the package root.
/// Both sides of a consumer's comparison are absolute paths on different
/// machines, so this is the only comparable part.
fn package_relative(package_root: &str, path: &str) -> Option<String> {
    let root = package_root.replace('\\', "/");
    path.replace('\\', "/")
        .strip_prefix(root.trim_end_matches('/'))
        .map(|rest| rest.trim_start_matches('/').to_owned())
        .filter(|rest| !rest.is_empty())
}
