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
//! **And the environment the proof read.** An artifact is proven about in one
//! installed environment: a closure a dialect axiom discharged holds for the
//! `@solidjs/signals` archive whose audited rows answered, and a claim composed
//! from a dependency's receipt holds only where that dependency is the one
//! certified. Two certifications of the same bytes against different
//! environments are two different acceptances. So a bundle states the
//! environment its receipt binds (`dependencyEnvironmentRoot`, reproduced from
//! the entries the index publishes), and admission resolves every entry from
//! the imported package's own installed location, the way Node would, and
//! compares name, manifest version and lockfile integrity. A missing, different
//! or unstatable dependency refuses the bundle; the import then behaves as if
//! no bundle existed. The dialect chosen for the analysis is never consulted:
//! it answers which language the project is written in, not which archive this
//! package resolves.
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
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::AcceptedContractIndex;

use crate::{
    contract_certification::{
        BuiltInReceiptEntry, DependencyEnvironmentEntry, Policy2ReceiptBindings,
        policy2_artifact_acceptance_root_for_identity, policy2_dependency_environment_root,
    },
    contract_interface::{
        AuthenticCase, ContractFailure, InstalledArtifactIdentity, ResolvedTargetIdentity,
        admissible_cases, declared_conditions, load_authenticated_policy2_embedded_contract,
        verified_dependency_environment,
    },
};

mod embedded;

#[cfg(test)]
mod tests;

const BUNDLE_INDEX_FORMAT: &str = "solid-checker-accepted-contract-bundle-index";
/// Version 2: every bundle states the dependency environment its receipt
/// binds, and bundles are unique by artifact *and* environment.
const BUNDLE_INDEX_VERSION: u16 = 2;
/// The version before environments were stated. Still read, and still
/// authenticated byte for byte, but **inert**: its receipts bind no
/// environment, so no consumer tree can be shown to be the one the proof read,
/// and none of its bundles is ever admitted. Kept loadable only so a build
/// carrying such an index still starts; it is provably sound because it
/// supplies nothing.
const INERT_BUNDLE_INDEX_VERSION: u16 = 1;

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
    /// The entries behind `bindings.dependencyEnvironmentRoot`, in canonical
    /// order. Unauthenticated on their own: the loader admits them only when
    /// they hash to the root the receipt binds.
    #[serde(default)]
    dependency_environment: Option<Vec<DependencyEnvironmentEntry>>,
}

struct LoadedBundle {
    specifier: String,
    requested_entrypoint: String,
    export_conditions: Vec<String>,
    runtime_target: String,
    declaration_target: String,
    /// `policy2_artifact_acceptance_root` of the five identity fields, which is
    /// what a consumer's installed identity must reproduce.
    acceptance_root: String,
    /// The key this bundle is indexed and admitted under: the acceptance root
    /// and the environment together, because two certifications of one
    /// artifact in different environments are two acceptances, and keyed by
    /// the artifact alone `AcceptedContractIndex` would drop both.
    identity: String,
    /// `None` for an inert bundle (see [`INERT_BUNDLE_INDEX_VERSION`]).
    environment: Option<Vec<DependencyEnvironmentEntry>>,
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
    let inert = match index.bundle_index_version {
        BUNDLE_INDEX_VERSION => false,
        INERT_BUNDLE_INDEX_VERSION => true,
        _ => {
            return Err(unsupported_index());
        }
    };
    if index.format != BUNDLE_INDEX_FORMAT {
        return Err(unsupported_index());
    }
    index
        .bundles
        .iter()
        .map(|entry| {
            let document = object(&entry.document)?;
            let receipt = object(&entry.receipt)?;
            let mut bundle = load_bundle(entry, document, receipt)?;
            if inert {
                bundle.environment = None;
                bundle.identity.clone_from(&bundle.acceptance_root);
            } else if bundle.environment.is_none() {
                return Err(ContractFailure::ReceiptMismatch {
                    field: "dependencyEnvironment",
                });
            }
            Ok(bundle)
        })
        .collect()
}

fn unsupported_index() -> ContractFailure {
    ContractFailure::DocumentDecode {
        message: format!(
            "the compiled-in accepted-contract index must use format {BUNDLE_INDEX_FORMAT:?} \
             version {BUNDLE_INDEX_VERSION} (or the inert version {INERT_BUNDLE_INDEX_VERSION})"
        ),
    }
}

/// The key one bundle is indexed under: its artifact and the environment its
/// proof read. An environment-less bundle is never admitted, and keeps the bare
/// acceptance root only so it has a key at all.
fn bundle_identity(
    acceptance_root: &str,
    environment: Option<&[DependencyEnvironmentEntry]>,
) -> String {
    match environment {
        Some(environment) => environment_acceptance_identity(acceptance_root, environment),
        None => acceptance_root.to_owned(),
    }
}

/// The key an acceptance is indexed and admitted under by artifact, in either
/// tier: the acceptance root and the environment its proof read, together.
///
/// Two certifications of one artifact in different environments are two
/// acceptances; keyed by the artifact alone, `AcceptedContractIndex` would drop
/// both as a conflict, or -- across tiers -- hand one tier's environment to the
/// other's consumer. A project catalog's entry and a compiled-in bundle about
/// the same artifact in the same environment get the same key, which is right:
/// they are the same statement.
pub(crate) fn environment_acceptance_identity(
    acceptance_root: &str,
    environment: &[DependencyEnvironmentEntry],
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:accepted-bundle-identity:v1");
    for field in [
        acceptance_root,
        policy2_dependency_environment_root(environment).as_str(),
    ] {
        hash.update(u64::try_from(field.len()).unwrap_or(u64::MAX).to_be_bytes());
        hash.update(field.as_bytes());
    }
    format!("sha256:{:x}", hash.finalize())
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
    // The environment the index states has to be the one the receipt signed,
    // for the same reason: admission checks the consumer's tree against these
    // entries, and an entry nobody signed would be an environment nobody
    // proved anything in.
    let environment =
        verified_dependency_environment(&entry.bindings, entry.dependency_environment.as_deref())?;
    Ok(LoadedBundle {
        specifier: entry.specifier.clone(),
        requested_entrypoint: entry.requested_entrypoint.clone(),
        export_conditions: entry.export_conditions.clone(),
        runtime_target: entry.runtime_target.clone(),
        declaration_target: entry.declaration_target.clone(),
        identity: bundle_identity(&identity, environment.as_deref()),
        acceptance_root: identity,
        environment,
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
            // An inert bundle can never be admitted, so it is not offered.
            .filter(|bundle| bundle.environment.is_some())
            .map(|bundle| (bundle.identity.clone(), bundle.contract.clone())),
    ))
}

/// Whether this project's installed tree reproduces a bundle's dependency
/// environment, resolved from the installed location of the package the
/// specifier (the first argument) names.
///
/// The native answer is `diagnostics`' filesystem walk over
/// [`environment_is_installed`]. A host with no filesystem answers `true` only
/// for the empty environment.
pub type InstalledEnvironment<'a> = dyn Fn(&str, &[DependencyEnvironmentEntry]) -> bool + 'a;

/// Whether an installed tree reproduces `environment`, starting from `root`,
/// the imported package's installed location.
///
/// `resolve(from, name)` is Node's lookup of the bare package `name` from the
/// package installed at `from`: `Ok(None)` when nothing is installed under any
/// `node_modules` it walks, `Err` when the tree cannot state the answer
/// exactly. `identity(at)` is the name, manifest version and lockfile integrity
/// of the package installed at `at`, or `None` when those are not all stated.
///
/// The rule, and why it is this strict:
///
/// - Every entry must be found from at least one located package: the root, or
///   a package an earlier entry resolved to. An entry nothing reaches is a
///   premise this tree cannot supply.
/// - **Every** resolution of an entry's name, from **every** located package,
///   must reach exactly that entry's identity. The certification states which
///   packages it read, not which package read each one, so a nested copy under
///   one dependency that differs from the hoisted one another dependency sees
///   is exactly the swap this cannot tell apart from the certified tree. It is
///   refused rather than guessed.
/// - Two entries with the same name -- two copies of one package in the
///   certified environment -- can therefore never both hold, and refuse.
pub(crate) fn environment_is_installed<L: Clone + Ord>(
    environment: &[DependencyEnvironmentEntry],
    root: L,
    resolve: impl Fn(&L, &str) -> Result<Option<L>, ()>,
    identity: impl Fn(&L) -> Option<DependencyEnvironmentEntry>,
) -> bool {
    let mut names = BTreeSet::new();
    if !environment
        .iter()
        .all(|entry| names.insert(entry.name.as_str()))
    {
        return false;
    }
    let mut located = BTreeSet::from([root.clone()]);
    let mut pending = vec![root];
    let mut found = BTreeSet::new();
    while let Some(from) = pending.pop() {
        for entry in environment {
            let at = match resolve(&from, &entry.name) {
                Ok(Some(at)) => at,
                Ok(None) => continue,
                Err(()) => return false,
            };
            if identity(&at).as_ref() != Some(entry) {
                return false;
            }
            found.insert(entry.name.as_str());
            if located.insert(at.clone()) {
                pending.push(at);
            }
        }
    }
    found.len() == names.len()
}

/// Which specifiers this project may import under a compiled-in acceptance.
///
/// The twin of `contract_interface::admitted_project_artifacts`, and not a
/// second rule: both hand their candidates to [`admit_by_artifact`]. The only
/// difference is where the candidates come from — a compiled-in index rather
/// than catalogs on disk — because "does this acceptance apply here" must not
/// have two answers.
pub fn admitted_bundle_artifacts(
    conditions: &BTreeSet<String>,
    installed_integrity: &InstalledArtifactIdentity,
    resolved_target: &ResolvedTargetIdentity,
    installed_environment: &InstalledEnvironment,
) -> Result<Vec<(String, String)>, ContractFailure> {
    Ok(admitted_from(
        bundles()?,
        conditions,
        installed_integrity,
        resolved_target,
        installed_environment,
    ))
}

fn admitted_from(
    loaded: &[LoadedBundle],
    conditions: &BTreeSet<String>,
    installed_integrity: &InstalledArtifactIdentity,
    resolved_target: &ResolvedTargetIdentity,
    installed_environment: &InstalledEnvironment,
) -> Vec<(String, String)> {
    admit_by_artifact(
        loaded.iter().map(|bundle| ArtifactAcceptance {
            specifier: &bundle.specifier,
            requested_entrypoint: &bundle.requested_entrypoint,
            export_conditions: &bundle.export_conditions,
            runtime_target: &bundle.runtime_target,
            declaration_target: &bundle.declaration_target,
            acceptance_root: &bundle.acceptance_root,
            environment: bundle.environment.as_deref(),
            identity: &bundle.identity,
        }),
        conditions,
        installed_integrity,
        resolved_target,
        installed_environment,
    )
}

/// One acceptance a consumer could reach by artifact rather than by importer,
/// from whichever tier supplied it, described by exactly what the consumer's
/// own installed tree can be compared against.
pub(crate) struct ArtifactAcceptance<'a> {
    /// The specifier a consumer writes; admission is keyed by it.
    pub(crate) specifier: &'a str,
    pub(crate) requested_entrypoint: &'a str,
    /// The conditions the signed acceptance root was computed over.
    pub(crate) export_conditions: &'a [String],
    /// Package-relative; see [`AuthenticCase::reaches`].
    pub(crate) runtime_target: &'a str,
    pub(crate) declaration_target: &'a str,
    /// The signed `artifactAcceptanceRoot`.
    pub(crate) acceptance_root: &'a str,
    /// The entries behind the signed `dependencyEnvironmentRoot`, already
    /// reproduced against it. `None` when the receipt states no environment
    /// or its entries were not published: such an acceptance is never admitted.
    pub(crate) environment: Option<&'a [DependencyEnvironmentEntry]>,
    /// The key the acceptance is indexed under
    /// ([`environment_acceptance_identity`]).
    pub(crate) identity: &'a str,
}

/// The one rule for admitting an acceptance by artifact (ADR 0123), whichever
/// tier it came from.
///
/// An acceptance applies to this project only when all of these hold:
///
/// 1. the receipt states the environment its proof read;
/// 2. the project's installed identity for the specifier -- name, manifest
///    version, lockfile integrity -- reproduces the signed acceptance root under
///    the conditions it was computed over;
/// 3. the project's installed tree, resolved from that package's own location,
///    reproduces the environment exactly ([`environment_is_installed`]);
/// 4. the file this project resolved is one the acceptance was proven about,
///    and [`admissible_cases`] selects it under the host's declaration.
///
/// Anything missing, different or unstatable skips the acceptance, so this
/// adds acceptances on proof and never removes one.
pub(crate) fn admit_by_artifact<'a>(
    acceptances: impl IntoIterator<Item = ArtifactAcceptance<'a>>,
    conditions: &BTreeSet<String>,
    installed_integrity: &InstalledArtifactIdentity,
    resolved_target: &ResolvedTargetIdentity,
    installed_environment: &InstalledEnvironment,
) -> Vec<(String, String)> {
    let declared = declared_conditions(conditions);
    let mut authentic: BTreeMap<String, Vec<AuthenticCase>> = BTreeMap::new();
    for acceptance in acceptances {
        let Some(environment) = acceptance.environment else {
            continue;
        };
        let Some((name, version, integrity)) = installed_integrity(acceptance.specifier) else {
            continue;
        };
        // Recomputed against the *installed* identity rather than the
        // certifier's record of it: the record states what certification
        // resolved, and the question here is whether this project resolved the
        // same thing.
        let derived = policy2_artifact_acceptance_root_for_identity(
            &name,
            &version,
            &integrity,
            acceptance.requested_entrypoint,
            acceptance.export_conditions,
        );
        if derived != acceptance.acceptance_root {
            continue;
        }
        // The same artifact, and now the same environment, or it is a
        // different acceptance this project never reproduced.
        if !installed_environment(acceptance.specifier, environment) {
            continue;
        }
        authentic
            .entry(acceptance.specifier.to_owned())
            .or_default()
            .push(AuthenticCase::from_relative(
                acceptance.identity.to_owned(),
                acceptance.runtime_target.to_owned(),
                acceptance.declaration_target.to_owned(),
                acceptance.export_conditions.to_vec(),
            ));
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
        admitted.extend(
            admissible_cases(&reaching, &declared)
                .into_iter()
                .map(|case| (specifier.clone(), case.identity.clone())),
        );
    }
    admitted
}
