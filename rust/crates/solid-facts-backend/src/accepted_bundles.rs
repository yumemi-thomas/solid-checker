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
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::OnceLock,
};

use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::AcceptedContractIndex;

use crate::{
    contract_certification::{
        BuiltInReceiptEntry, DependencyEnvironmentEntry, EnvironmentImporter,
        Policy2ReceiptBindings, dependency_environment_states_edges,
        policy2_artifact_acceptance_root_for_identity, policy2_dependency_environment_root,
        policy2_receipt_payload_bindings, validate_dependency_environment,
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
/// Version 3: what version 2 states, each fact stored once.
///
/// - **Environments** live in one `environments` table keyed by their
///   `dependencyEnvironmentRoot` -- the content address the receipt already
///   signs -- and a bundle names its environment by that root. A per-host tier
///   proves one artifact in the same environment for every host, and 729
///   bundles read 120 distinct environments.
/// - **Bindings** are not restated. Version 2 carried a copy of each receipt
///   payload's bindings beside the receipt's own digest, and authentication
///   refused any copy that was not byte-equal to the payload, so the copy
///   carried nothing the pinned receipt did not. Version 3 reads them from the
///   receipt the index names by digest.
///
/// Every table key is recomputed from its entries and every reference must
/// resolve, before any bundle is authenticated; either failure refuses the
/// whole index. What is admitted, and where, is exactly what version 2 admits.
const BUNDLE_INDEX_VERSION: u16 = 3;
/// Version 2: every bundle states the dependency environment its receipt
/// binds inline, beside a copy of its receipt's bindings, and bundles are
/// unique by artifact *and* environment. Still read, with unchanged meaning, so
/// a tier generated before version 3 loads; the bundler writes version 3 only.
const INLINE_BUNDLE_INDEX_VERSION: u16 = 2;
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

/// Enough of any index to choose the decoder for its version.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleIndexHeader {
    format: String,
    bundle_index_version: u16,
}

/// Version 3 (see [`BUNDLE_INDEX_VERSION`]).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BundleIndexDocument {
    /// Read by [`BundleIndexHeader`]; stated here so `deny_unknown_fields`
    /// admits them.
    #[serde(rename = "format")]
    _format: String,
    #[serde(rename = "bundleIndexVersion")]
    _bundle_index_version: u16,
    /// Every distinct environment the bundles were proven in, keyed by its
    /// `dependencyEnvironmentRoot`. A key is a claim about its entries, checked
    /// by recomputing it before any bundle is read.
    environments: BTreeMap<String, Vec<DependencyEnvironmentEntry>>,
    bundles: Vec<IndexedBundleEntry>,
}

/// One version-3 bundle: a [`BundleEntry`] whose bindings are read from its
/// receipt and whose environment is named by root.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IndexedBundleEntry {
    package_name: String,
    package_version: String,
    package_integrity: String,
    specifier: String,
    requested_entrypoint: String,
    export_conditions: Vec<String>,
    runtime_target: String,
    #[serde(default)]
    declaration_target: String,
    document: String,
    document_digest: String,
    receipt: String,
    receipt_digest: String,
    /// The key of this bundle's environment in the index's `environments`
    /// table, which must also be the root its receipt signs.
    dependency_environment_root: String,
}

/// Versions 1 and 2, which state each bundle whole.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InlineBundleIndexDocument {
    #[serde(rename = "format")]
    _format: String,
    #[serde(rename = "bundleIndexVersion")]
    _bundle_index_version: u16,
    bundles: Vec<BundleEntry>,
}

/// One compiled-in acceptance, described by exactly what a consumer can
/// recompute about its own installed tree: a version-2 index entry as it is
/// written, and every version's entry once [`indexed_entries`] resolves it.
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
    /// Stated by a version-2 index and compared with the receipt payload by
    /// authentication; a version-3 index states none, and they are read from
    /// the receipt `receipt_digest` pins.
    bindings: Policy2ReceiptBindings,
    /// The entries behind `bindings.dependencyEnvironmentRoot`, in canonical
    /// order. Unauthenticated on their own: the loader admits them only when
    /// they hash to the root the receipt binds.
    #[serde(default)]
    dependency_environment: Option<Vec<DependencyEnvironmentEntry>>,
}

struct LoadedBundle {
    specifier: String,
    /// The certified version, so a refusal report can name it.
    package_version: String,
    requested_entrypoint: String,
    export_conditions: Vec<String>,
    runtime_target: String,
    declaration_target: String,
    /// `policy2_artifact_acceptance_root` of the five identity fields, which is
    /// what a consumer's installed identity must reproduce.
    acceptance_root: String,
    /// The signed `snapshotRoot`: the archive the proof read, which a
    /// consumer's installed files must reproduce (ADR 0131).
    snapshot_root: String,
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
    load_index(INDEX_BYTES, object)
}

/// Every bundle an index names, authenticated, with `object` supplying the
/// bytes of each member it names. Any failure refuses the whole index.
fn load_index<'a>(
    index_bytes: &[u8],
    object: impl Fn(&str) -> Result<&'a [u8], ContractFailure>,
) -> Result<Vec<LoadedBundle>, ContractFailure> {
    let header: BundleIndexHeader = decode_index(index_bytes)?;
    let inert = match header.bundle_index_version {
        BUNDLE_INDEX_VERSION | INLINE_BUNDLE_INDEX_VERSION => false,
        INERT_BUNDLE_INDEX_VERSION => true,
        _ => {
            return Err(unsupported_index());
        }
    };
    if header.format != BUNDLE_INDEX_FORMAT {
        return Err(unsupported_index());
    }
    let entries = if header.bundle_index_version == BUNDLE_INDEX_VERSION {
        indexed_entries(decode_index(index_bytes)?, &object)?
    } else {
        decode_index::<InlineBundleIndexDocument>(index_bytes)?.bundles
    };
    entries
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

fn decode_index<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, ContractFailure> {
    serde_json::from_slice(bytes).map_err(|error| ContractFailure::DocumentDecode {
        message: format!("decode the compiled-in accepted-contract index: {error}"),
    })
}

/// A version-3 index's bundles, each with the environment its reference names
/// and the bindings its receipt states: the shape version 2 wrote whole.
///
/// The table is checked first and as a whole. A key must be the
/// `dependencyEnvironmentRoot` of its own entries, which must be canonical
/// ([`validate_dependency_environment`] refuses an unsorted or repeating list),
/// so a key determines its entries exactly; a bundle whose reference is not a
/// key refuses too. Neither is a bundle-level skip: an index that misstates
/// one environment has misstated what was reviewed, and the tier refuses to
/// load rather than serve the rest of it.
fn indexed_entries<'a>(
    index: BundleIndexDocument,
    object: &impl Fn(&str) -> Result<&'a [u8], ContractFailure>,
) -> Result<Vec<BundleEntry>, ContractFailure> {
    for (root, entries) in &index.environments {
        if validate_dependency_environment(entries).is_err()
            || policy2_dependency_environment_root(entries) != *root
        {
            return Err(ContractFailure::DocumentDecode {
                message: format!(
                    "the compiled-in accepted-contract index's environment {root} is not the \
                     environment that root names"
                ),
            });
        }
    }
    index
        .bundles
        .into_iter()
        .map(|bundle| {
            let environment = index
                .environments
                .get(&bundle.dependency_environment_root)
                .ok_or_else(|| ContractFailure::DocumentDecode {
                    message: format!(
                        "the compiled-in accepted-contract index names environment {} for {}, \
                         which its environments table does not carry",
                        bundle.dependency_environment_root, bundle.package_name
                    ),
                })?;
            // The bindings are read from the pinned bytes, never from bytes
            // the index has not vouched for.
            let receipt = object(&bundle.receipt)?;
            verify_object_digest(receipt, &bundle.receipt_digest, "receiptDigest")?;
            let bindings = policy2_receipt_payload_bindings(receipt).map_err(|error| {
                ContractFailure::ReceiptDecode {
                    message: error.to_string(),
                }
            })?;
            if bindings.dependency_environment_root != bundle.dependency_environment_root {
                return Err(ContractFailure::ReceiptMismatch {
                    field: "dependencyEnvironment",
                });
            }
            Ok(BundleEntry {
                package_name: bundle.package_name,
                package_version: bundle.package_version,
                package_integrity: bundle.package_integrity,
                specifier: bundle.specifier,
                requested_entrypoint: bundle.requested_entrypoint,
                export_conditions: bundle.export_conditions,
                runtime_target: bundle.runtime_target,
                declaration_target: bundle.declaration_target,
                document: bundle.document,
                document_digest: bundle.document_digest,
                receipt: bundle.receipt,
                receipt_digest: bundle.receipt_digest,
                bindings,
                dependency_environment: Some(environment.clone()),
            })
        })
        .collect()
}

fn unsupported_index() -> ContractFailure {
    ContractFailure::DocumentDecode {
        message: format!(
            "the compiled-in accepted-contract index must use format {BUNDLE_INDEX_FORMAT:?} \
             version {BUNDLE_INDEX_VERSION} or {INLINE_BUNDLE_INDEX_VERSION} (or the inert \
             version {INERT_BUNDLE_INDEX_VERSION})"
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
        package_version: entry.package_version.clone(),
        requested_entrypoint: entry.requested_entrypoint.clone(),
        export_conditions: entry.export_conditions.clone(),
        runtime_target: entry.runtime_target.clone(),
        declaration_target: entry.declaration_target.clone(),
        identity: bundle_identity(&identity, environment.as_deref()),
        acceptance_root: identity,
        snapshot_root: entry.bindings.snapshot_root.clone(),
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

/// The artifact snapshot root of the files installed for the package the
/// specifier names (`installed_package_snapshot_root`), or why this tree
/// cannot state that its files are the published archive's: a patch the tree
/// records, a member no archive installs, an unreadable file.
///
/// Step 2's acceptance root reproduces from the lockfile's integrity, which a
/// package manager keeps when it patches the installed files, so the files
/// themselves are asked too (ADR 0131). The native answer walks the installed
/// directory; a host with no filesystem has no answer and admits nothing that
/// needs one.
pub type InstalledArtifactBytes<'a> = dyn Fn(&str) -> Result<String, String> + 'a;

/// Whether an installed tree reproduces `environment`, starting from `root`,
/// the imported package's installed location.
///
/// `resolve(from, name)` is Node's lookup of the bare package `name` from the
/// package installed at `from`: `Ok(None)` when nothing is installed under any
/// `node_modules` it walks, `Err` when the tree cannot state the answer
/// exactly. `identity(at)` is the name, manifest version and lockfile integrity
/// of the package installed at `at`, or `None` when those are not all stated.
///
/// **An environment that states its resolution edges** (the `edges:v3` root)
/// is replayed edge by edge. Each entry names the package that looked it up and
/// the bare name it looked up; the lookup is repeated from that importer's own
/// installed location -- `root` for the certified package, and for any other
/// importer every location an earlier edge reached it at -- and must reach
/// exactly that entry's name, manifest version and lockfile integrity.
///
/// - What the certification never looked up is not asked. A package elsewhere
///   in the tree that sees another, hoisted version of the same name is not a
///   premise of the proof: in a pnpm tree `.pnpm/node_modules` hoists some
///   version of nearly everything into every package's view, and a rule that
///   read those lookups would refuse the tree the certification ran in.
/// - One name may appear under two importers at two versions, because each
///   edge is its own lookup.
/// - Missing, unresolvable or different refuses, as does an entry whose
///   importer is never reached.
///
/// **An environment stated without edges** (the `v1` root, which every receipt
/// issued before edges were recorded binds) cannot say which package read each
/// entry, so it keeps the strict rule, which is sound for it:
///
/// - Every entry must be found from at least one located package: the root, or
///   a package an earlier entry resolved to. An entry nothing reaches is a
///   premise this tree cannot supply.
/// - **Every** resolution of an entry's name, from **every** located package,
///   must reach exactly that entry's identity. A nested copy under one
///   dependency that differs from the hoisted one another dependency sees is
///   exactly the swap this cannot tell apart from the certified tree. It is
///   refused rather than guessed.
/// - Two entries with the same name -- two copies of one package in the
///   certified environment -- can therefore never both hold, and refuse.
///
/// **Under either rule, a copy that matches but is patched refuses** (ADR
/// 0131). `patched(at)` names the evidence that the package installed at `at`
/// is not the published archive its lockfile integrity names -- a package
/// manager's patch record, a `patch-package` patch -- or `None` when the tree
/// shows none. A lockfile keeps the published integrity for a patched package,
/// so name, version and integrity alone would admit a contract about bytes the
/// consumer does not run.
pub(crate) fn environment_is_installed<L: Clone + Ord>(
    environment: &[DependencyEnvironmentEntry],
    root: L,
    resolve: impl Fn(&L, &str) -> Result<Option<L>, ()>,
    identity: impl Fn(&L) -> Option<DependencyEnvironmentEntry>,
    patched: impl Fn(&L) -> Option<String>,
) -> bool {
    environment_difference(environment, root, resolve, identity, patched, |_| None).is_none()
}

/// The first way an installed tree fails to reproduce `environment`, by the
/// rule [`environment_is_installed`] states; `None` when it reproduces it.
///
/// `version_of(at)` is the manifest version of the package at `at` when it can
/// be read even though its full identity cannot, so a report can name the
/// installed version beside the certified one. It never decides anything.
pub(crate) fn environment_difference<L: Clone + Ord>(
    environment: &[DependencyEnvironmentEntry],
    root: L,
    resolve: impl Fn(&L, &str) -> Result<Option<L>, ()>,
    identity: impl Fn(&L) -> Option<DependencyEnvironmentEntry>,
    patched: impl Fn(&L) -> Option<String>,
    version_of: impl Fn(&L) -> Option<String>,
) -> Option<EnvironmentDifference> {
    if dependency_environment_states_edges(environment) {
        return edge_environment_difference(
            environment,
            root,
            resolve,
            identity,
            patched,
            version_of,
        );
    }
    let mut names = BTreeSet::new();
    if let Some(entry) = environment
        .iter()
        .find(|entry| !names.insert(entry.name.as_str()))
    {
        return Some(EnvironmentDifference::DuplicateName {
            name: entry.name.clone(),
        });
    }
    let mut located = BTreeSet::from([root.clone()]);
    let mut pending = vec![root];
    let mut found = BTreeSet::new();
    while let Some(from) = pending.pop() {
        for entry in environment {
            let at = match resolve(&from, &entry.name) {
                Ok(Some(at)) => at,
                Ok(None) => continue,
                Err(()) => {
                    return Some(EnvironmentDifference::Unresolvable {
                        name: entry.name.clone(),
                        from: None,
                    });
                }
            };
            let installed = identity(&at);
            if !installed
                .as_ref()
                .is_some_and(|installed| installed.same_package(entry))
            {
                return Some(EnvironmentDifference::Differs {
                    installed_version: installed
                        .as_ref()
                        .map(|installed| installed.version.clone())
                        .or_else(|| version_of(&at)),
                    installed,
                    certified: entry.clone(),
                    from: None,
                });
            }
            if let Some(evidence) = patched(&at) {
                return Some(EnvironmentDifference::Patched {
                    certified: entry.clone(),
                    from: None,
                    evidence,
                });
            }
            found.insert(entry.name.as_str());
            if located.insert(at.clone()) {
                pending.push(at);
            }
        }
    }
    environment
        .iter()
        .find(|entry| !found.contains(entry.name.as_str()))
        .map(|entry| EnvironmentDifference::NotInstalled {
            certified: entry.clone(),
            from: None,
        })
}

/// The edge-rooted rule [`environment_is_installed`] states for an environment
/// that records who resolved what. Breadth-first from the certified package,
/// so the first failing edge reported is the one nearest the root.
fn edge_environment_difference<L: Clone + Ord>(
    environment: &[DependencyEnvironmentEntry],
    root: L,
    resolve: impl Fn(&L, &str) -> Result<Option<L>, ()>,
    identity: impl Fn(&L) -> Option<DependencyEnvironmentEntry>,
    patched: impl Fn(&L) -> Option<String>,
    version_of: impl Fn(&L) -> Option<String>,
) -> Option<EnvironmentDifference> {
    let certified_label = identity(&root).map_or_else(
        || "the certified package".to_owned(),
        |certified| format!("{}@{}", certified.name, certified.version),
    );
    let label = |importer: &EnvironmentImporter| match importer {
        EnvironmentImporter::Certified => certified_label.clone(),
        EnvironmentImporter::Package(package) => format!("{}@{}", package.name, package.version),
    };
    let mut located = BTreeMap::<EnvironmentImporter, BTreeSet<L>>::new();
    located
        .entry(EnvironmentImporter::Certified)
        .or_default()
        .insert(root.clone());
    // Each importer's edges, in canonical order, so every location is asked
    // only the lookups made from it.
    let mut by_importer = BTreeMap::<&EnvironmentImporter, Vec<usize>>::new();
    for (index, entry) in environment.iter().enumerate() {
        if let Some(edge) = &entry.resolved_from {
            by_importer.entry(&edge.importer).or_default().push(index);
        }
    }
    let mut queue = VecDeque::from([(EnvironmentImporter::Certified, root)]);
    let mut reached = vec![false; environment.len()];
    while let Some((importer, from)) = queue.pop_front() {
        for &index in by_importer.get(&importer).map_or(&[][..], Vec::as_slice) {
            let entry = &environment[index];
            let Some(edge) = &entry.resolved_from else {
                continue;
            };
            let at = match resolve(&from, &edge.specifier) {
                Ok(Some(at)) => at,
                Ok(None) => {
                    return Some(EnvironmentDifference::NotInstalled {
                        certified: entry.clone(),
                        from: Some(label(&importer)),
                    });
                }
                Err(()) => {
                    return Some(EnvironmentDifference::Unresolvable {
                        name: edge.specifier.clone(),
                        from: Some(label(&importer)),
                    });
                }
            };
            let installed = identity(&at);
            if !installed
                .as_ref()
                .is_some_and(|installed| installed.same_package(entry))
            {
                return Some(EnvironmentDifference::Differs {
                    installed_version: installed
                        .as_ref()
                        .map(|installed| installed.version.clone())
                        .or_else(|| version_of(&at)),
                    installed,
                    certified: entry.clone(),
                    from: Some(label(&importer)),
                });
            }
            if let Some(evidence) = patched(&at) {
                return Some(EnvironmentDifference::Patched {
                    certified: entry.clone(),
                    from: Some(label(&importer)),
                    evidence,
                });
            }
            reached[index] = true;
            let key = entry.as_importer();
            if located.entry(key.clone()).or_default().insert(at.clone()) {
                queue.push_back((key, at));
            }
        }
    }
    // Validation refuses an environment whose edges are not rooted, so this is
    // reached only for an entry whose importer this tree never located.
    environment
        .iter()
        .zip(reached)
        .find(|(_, reached)| !reached)
        .map(|(entry, _)| EnvironmentDifference::NotInstalled {
            certified: entry.clone(),
            from: entry
                .resolved_from
                .as_ref()
                .map(|edge| label(&edge.importer)),
        })
}

/// Why an installed tree does not reproduce a certified environment: the
/// first entry that differs, for a report that has to name it.
///
/// `from` names the importer whose lookup failed (`name@version`) when the
/// environment states edges; `None` under the strict rule, which has no edge
/// to name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EnvironmentDifference {
    /// The certified environment names one package twice, which no tree can
    /// reproduce under the "every resolution" rule.
    DuplicateName { name: String },
    /// The tree cannot state exactly which copy of this package resolves.
    Unresolvable { name: String, from: Option<String> },
    /// A copy resolves, and it is not the certified one.
    Differs {
        /// Its full identity, when the tree states one.
        installed: Option<DependencyEnvironmentEntry>,
        /// Its manifest version, when readable.
        installed_version: Option<String>,
        certified: DependencyEnvironmentEntry,
        from: Option<String>,
    },
    /// Nothing resolves this certified package from anywhere the rule looks.
    NotInstalled {
        certified: DependencyEnvironmentEntry,
        from: Option<String>,
    },
    /// The certified copy resolves -- name, version and lockfile integrity all
    /// match -- and the tree shows it patched, so its bytes are not the
    /// published archive that integrity names (ADR 0131).
    Patched {
        certified: DependencyEnvironmentEntry,
        from: Option<String>,
        /// What shows the patch, e.g. `pnpm-lock.yaml patchedDependencies`.
        evidence: String,
    },
}

impl std::fmt::Display for EnvironmentDifference {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let via = |from: &Option<String>| {
            from.as_ref()
                .map_or_else(String::new, |from| format!(" resolved from {from}"))
        };
        match self {
            Self::DuplicateName { name } => {
                write!(formatter, "the certified environment names {name} twice")
            }
            Self::Unresolvable { name, from } => write!(
                formatter,
                "{name}{}: this tree cannot state exactly which installed copy resolves",
                via(from)
            ),
            Self::Differs {
                installed: Some(installed),
                certified,
                from,
                ..
            } if installed.version == certified.version => write!(
                formatter,
                "{}{} {} installed with integrity {}, certified with {}",
                certified.name,
                via(from),
                certified.version,
                installed.integrity,
                certified.integrity
            ),
            Self::Differs {
                installed_version: Some(version),
                certified,
                from,
                ..
            } => write!(
                formatter,
                "{}{} installed {version}, certified {}",
                certified.name,
                via(from),
                certified.version
            ),
            Self::Differs {
                certified, from, ..
            } => write!(
                formatter,
                "{}{} installed with no exact version and lockfile integrity, certified {}",
                certified.name,
                via(from),
                certified.version
            ),
            Self::NotInstalled { certified, from } => write!(
                formatter,
                "{}{} not installed, certified {}",
                certified.name,
                via(from),
                certified.version
            ),
            Self::Patched {
                certified,
                from,
                evidence,
            } => write!(
                formatter,
                "{}@{}{} is patched ({evidence}), so its installed bytes are not the published \
                 archive the certification read",
                certified.name,
                certified.version,
                via(from)
            ),
        }
    }
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
    installed_bytes: &InstalledArtifactBytes,
    resolved_target: &ResolvedTargetIdentity,
    installed_environment: &InstalledEnvironment,
) -> Result<Vec<(String, String)>, ContractFailure> {
    Ok(admitted_from(
        bundles()?,
        conditions,
        installed_integrity,
        installed_bytes,
        resolved_target,
        installed_environment,
    ))
}

fn admitted_from(
    loaded: &[LoadedBundle],
    conditions: &BTreeSet<String>,
    installed_integrity: &InstalledArtifactIdentity,
    installed_bytes: &InstalledArtifactBytes,
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
            snapshot_root: &bundle.snapshot_root,
            environment: bundle.environment.as_deref(),
            identity: &bundle.identity,
        }),
        conditions,
        installed_integrity,
        installed_bytes,
        resolved_target,
        installed_environment,
    )
}

/// Why [`admit_by_artifact`] did not admit an acceptance, in the order its
/// steps are checked. A report states it so `missing` is not the whole answer
/// when an acceptance for the installed package does exist.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionRefusal {
    /// Step 1: the receipt states no dependency environment -- issued before
    /// ADR 0123, by a certifier that could not acquire one, or binding the
    /// retired ambiguous empty root.
    NoEnvironmentStated,
    /// Step 2: this project's installed identity does not reproduce the signed
    /// acceptance root. `installed` is `(version, integrity)` when the project
    /// states them; `None` when it cannot (no exact lockfile integrity).
    AcceptanceRootNotReproduced {
        installed: Option<(String, String)>,
        certified_version: String,
    },
    /// Step 2b: the installed package's files are not the archive the receipt
    /// signs (`snapshotRoot`) -- patched, rebuilt or edited after install; the
    /// text says what shows it (ADR 0131).
    InstalledBytesDiffer(String),
    /// Step 3: the installed tree differs from the certified environment; the
    /// text names the first differing package.
    EnvironmentDiffers(String),
}

impl std::fmt::Display for AdmissionRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoEnvironmentStated => formatter.write_str(
                "its receipt states no dependency environment, so it is admitted nowhere",
            ),
            Self::AcceptanceRootNotReproduced {
                installed: Some((version, _)),
                certified_version,
            } if version != certified_version => write!(
                formatter,
                "the installed package is {version}, not the certified {certified_version}"
            ),
            Self::AcceptanceRootNotReproduced {
                installed: Some(_), ..
            } => formatter.write_str(
                "the installed package's bytes or requested entrypoint are not the certified ones \
                 (its acceptance root is not reproduced)",
            ),
            Self::AcceptanceRootNotReproduced {
                installed: None, ..
            } => formatter.write_str(
                "the installed package has no exact lockfile integrity, so its acceptance root \
                 cannot be reproduced",
            ),
            Self::InstalledBytesDiffer(difference) => write!(
                formatter,
                "the installed package's files are not the certified archive's: {difference}"
            ),
            Self::EnvironmentDiffers(difference) => {
                write!(
                    formatter,
                    "its dependency environment differs: {difference}"
                )
            }
        }
    }
}

/// The first way this tree differs from a certified environment, rendered, or
/// `None` when it reproduces it. The diagnostic twin of
/// [`InstalledEnvironment`]; it never decides admission.
pub type InstalledEnvironmentDifference<'a> =
    dyn Fn(&str, &[DependencyEnvironmentEntry]) -> Option<String> + 'a;

/// Steps 1-3 of [`admit_by_artifact`] for each acceptance, reporting the first
/// that fails: `Some(refusal)`, or `None` when the acceptance passes all three
/// and admission then turns only on the resolved file and case selection.
///
/// Diagnostic only, and deliberately a replay of the same checks rather than a
/// second rule: nothing admits from it.
pub(crate) fn admission_refusals<'a>(
    acceptances: impl IntoIterator<Item = (ArtifactAcceptance<'a>, &'a str)>,
    installed_integrity: &InstalledArtifactIdentity,
    installed_bytes: &InstalledArtifactBytes,
    installed_difference: &InstalledEnvironmentDifference,
) -> Vec<(String, Option<AdmissionRefusal>)> {
    acceptances
        .into_iter()
        .map(|(acceptance, certified_version)| {
            let refusal = (|| {
                let Some(environment) = acceptance.environment else {
                    return Some(AdmissionRefusal::NoEnvironmentStated);
                };
                let Some((name, version, integrity)) = installed_integrity(acceptance.specifier)
                else {
                    return Some(AdmissionRefusal::AcceptanceRootNotReproduced {
                        installed: None,
                        certified_version: certified_version.to_owned(),
                    });
                };
                if policy2_artifact_acceptance_root_for_identity(
                    &name,
                    &version,
                    &integrity,
                    acceptance.requested_entrypoint,
                    acceptance.export_conditions,
                ) != acceptance.acceptance_root
                {
                    return Some(AdmissionRefusal::AcceptanceRootNotReproduced {
                        installed: Some((version, integrity)),
                        certified_version: certified_version.to_owned(),
                    });
                }
                if let Some(difference) = bytes_difference(&acceptance, installed_bytes) {
                    return Some(AdmissionRefusal::InstalledBytesDiffer(difference));
                }
                installed_difference(acceptance.specifier, environment)
                    .map(AdmissionRefusal::EnvironmentDiffers)
            })();
            (acceptance.specifier.to_owned(), refusal)
        })
        .collect()
}

/// [`admission_refusals`] over the compiled-in tier.
pub fn bundle_admission_refusals(
    installed_integrity: &InstalledArtifactIdentity,
    installed_bytes: &InstalledArtifactBytes,
    installed_difference: &InstalledEnvironmentDifference,
) -> Result<Vec<(String, Option<AdmissionRefusal>)>, ContractFailure> {
    Ok(admission_refusals(
        bundles()?.iter().map(|bundle| {
            (
                ArtifactAcceptance {
                    specifier: &bundle.specifier,
                    requested_entrypoint: &bundle.requested_entrypoint,
                    export_conditions: &bundle.export_conditions,
                    runtime_target: &bundle.runtime_target,
                    declaration_target: &bundle.declaration_target,
                    acceptance_root: &bundle.acceptance_root,
                    snapshot_root: &bundle.snapshot_root,
                    environment: bundle.environment.as_deref(),
                    identity: &bundle.identity,
                },
                bundle.package_version.as_str(),
            )
        }),
        installed_integrity,
        installed_bytes,
        installed_difference,
    ))
}

/// Why step 2b refuses `acceptance`, or `None` when the installed files
/// reproduce the signed snapshot root. The one reading of that step, for both
/// admission and the refusal report.
fn bytes_difference(
    acceptance: &ArtifactAcceptance,
    installed_bytes: &InstalledArtifactBytes,
) -> Option<String> {
    if acceptance.snapshot_root.is_empty() {
        return Some("its receipt signs no snapshot root to compare them with".into());
    }
    match installed_bytes(acceptance.specifier) {
        Ok(root) if root == acceptance.snapshot_root => None,
        Ok(_) => Some(
            "they do not reproduce the signed snapshot root, so something changed them after \
             install"
                .into(),
        ),
        Err(reason) => Some(reason),
    }
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
    /// The signed `snapshotRoot`, which the installed files must reproduce.
    pub(crate) snapshot_root: &'a str,
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
///    2b. the files installed for that package reproduce the signed
///    `snapshotRoot`, so they are the archive the proof read and not a patched
///    copy that kept its lockfile integrity (ADR 0131);
/// 3. the project's installed tree, resolved from that package's own location,
///    reproduces the environment exactly, and no package in it is patched
///    ([`environment_is_installed`]);
/// 4. the file this project resolved is one the acceptance was proven about,
///    and [`admissible_cases`] selects it under the host's declaration.
///
/// Anything missing, different or unstatable skips the acceptance, so this
/// adds acceptances on proof and never removes one.
pub(crate) fn admit_by_artifact<'a>(
    acceptances: impl IntoIterator<Item = ArtifactAcceptance<'a>>,
    conditions: &BTreeSet<String>,
    installed_integrity: &InstalledArtifactIdentity,
    installed_bytes: &InstalledArtifactBytes,
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
        // The integrity is the lockfile's record of what was fetched, not of
        // what is on disk: a patched package keeps the published one. The
        // files themselves have to be the archive the proof read.
        if bytes_difference(&acceptance, installed_bytes).is_some() {
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
