//! Receipt-issued first-party contracts derived from the checked RC.3 corpus.
//!
//! The conformance model owns semantics. The checked machine reports own the
//! published package census and export-map branch identities. This adapter
//! joins those authorities and feeds the ordinary proposal/proof/finalizer
//! path; it contains no API-name semantic table.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use sha2::Digest as _;
use solid_reactive_ir::contract_semantics::{
    AcceptedContractIndex, AcceptedContractInput, ArtifactCase, CallClaims, CallSemantics,
    ContractProposal, ExportSemantics, GuardPartition, KnowledgeSet, ResolutionStep,
    solid2_rc3::{conformance_corpus, reviewed_support_corpus},
};
use solid_reactive_ir::{RuntimeBuild, RuntimeEnvironment, RuntimeRendering, RuntimeTarget};
use thiserror::Error;

use crate::{
    artifact_resolution::{ClosureManifest, ClosurePackageIdentity},
    contract_workflow::{AcceptedArtifacts, ContractWorkflowError, accept_checked_corpus_case},
    diagnostics::installed_package_integrity,
};

const CONFORMANCE_BYTES: &[u8] =
    include_bytes!("../../../../benchmarks/package-contract-v2/phase13/conformance.json");
const REVIEWED_SUPPORT_BYTES: &[u8] = include_bytes!(
    "../../../../benchmarks/package-contract-v2/phase14/solid2-reviewed-support.json"
);
const AUDIT_BYTES: &[u8] =
    include_bytes!("../../../../benchmarks/package-contract-v2/phase0/rc3/audit.json");

#[derive(Debug, Error)]
pub enum FirstPartyBundleError {
    #[error("checked RC.3 authority cannot be decoded: {0}")]
    Authority(#[from] serde_json::Error),
    #[error("checked RC.3 authority is inconsistent: {0}")]
    Inconsistent(String),
    #[error("checked RC.3 semantic model is invalid: {0}")]
    Model(#[from] solid_reactive_ir::contract_semantics::ModelError),
    #[error("checked RC.3 closure is invalid: {0}")]
    Closure(#[from] crate::ArtifactResolutionFailure),
    #[error("checked RC.3 proof workflow failed: {0}")]
    Workflow(#[from] ContractWorkflowError),
}

pub struct FirstPartyBundle {
    pub file_stem: String,
    pub package: String,
    pub artifact_case: String,
    pub selector: Option<BundleSelector>,
    pub document: Vec<u8>,
    pub receipt: Vec<u8>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleSelector {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<RuntimeTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<RuntimeBuild>,
}

struct EmbeddedBundle {
    document: &'static [u8],
    receipt: &'static [u8],
}

const EMBEDDED_BUNDLES: &[EmbeddedBundle] = &[
    EmbeddedBundle {
        document: include_bytes!("../../../../pkg/contracts/bundled/solid-v2/solid-js.json"),
        receipt: include_bytes!("../../../../pkg/contracts/bundled/solid-v2/solid-js.receipt.json"),
    },
    EmbeddedBundle {
        document: include_bytes!("../../../../pkg/contracts/bundled/solid-v2/solidjs-signals.json"),
        receipt: include_bytes!(
            "../../../../pkg/contracts/bundled/solid-v2/solidjs-signals.receipt.json"
        ),
    },
    EmbeddedBundle {
        document: include_bytes!("../../../../pkg/contracts/bundled/solid-v2/solidjs-web.json"),
        receipt: include_bytes!(
            "../../../../pkg/contracts/bundled/solid-v2/solidjs-web.receipt.json"
        ),
    },
    EmbeddedBundle {
        document: include_bytes!(
            "../../../../pkg/contracts/bundled/solid-v2/solidjs-web--server-functions-browser-client.json"
        ),
        receipt: include_bytes!(
            "../../../../pkg/contracts/bundled/solid-v2/solidjs-web--server-functions-browser-client.receipt.json"
        ),
    },
    EmbeddedBundle {
        document: include_bytes!(
            "../../../../pkg/contracts/bundled/solid-v2/solidjs-web--server-functions-node-server.json"
        ),
        receipt: include_bytes!(
            "../../../../pkg/contracts/bundled/solid-v2/solidjs-web--server-functions-node-server.receipt.json"
        ),
    },
    EmbeddedBundle {
        document: include_bytes!(
            "../../../../pkg/contracts/bundled/solid-v2/solidjs-web--web-node-server.json"
        ),
        receipt: include_bytes!(
            "../../../../pkg/contracts/bundled/solid-v2/solidjs-web--web-node-server.receipt.json"
        ),
    },
];

struct NamedDocument {
    name: &'static str,
    document: &'static [u8],
}

macro_rules! solid1_authority_document {
    ($stem:literal) => {
        NamedDocument {
            name: concat!($stem, ".json"),
            document: include_bytes!(concat!(
                "../../../../benchmarks/package-contract-v2/phase14/solid-v1-authority/",
                $stem,
                ".json"
            )),
        }
    };
}

macro_rules! solid1_embedded_bundle {
    ($stem:literal) => {
        EmbeddedBundle {
            document: include_bytes!(concat!(
                "../../../../pkg/contracts/bundled/solid-v1/",
                $stem,
                ".json"
            )),
            receipt: include_bytes!(concat!(
                "../../../../pkg/contracts/bundled/solid-v1/",
                $stem,
                ".receipt.json"
            )),
        }
    };
}

const SOLID1_AUTHORITY_INDEX_BYTES: &[u8] = include_bytes!(
    "../../../../benchmarks/package-contract-v2/phase14/solid-v1-authority/authority-index.json"
);

const SOLID1_AUTHORITY_DOCUMENTS: &[NamedDocument] = &[
    solid1_authority_document!("solid-root-browser-development"),
    solid1_authority_document!("solid-root-browser-production"),
    solid1_authority_document!("solid-root-node"),
    solid1_authority_document!("solid-store-browser-development"),
    solid1_authority_document!("solid-store-browser-production"),
    solid1_authority_document!("solid-store-node"),
    solid1_authority_document!("solid-web-browser-development"),
    solid1_authority_document!("solid-web-browser-production"),
    solid1_authority_document!("solid-web-node"),
    solid1_authority_document!("solid-jsx-runtime-default"),
    solid1_authority_document!("solid-jsx-dev-runtime-default"),
    solid1_authority_document!("solid-universal-development"),
    solid1_authority_document!("solid-universal-production"),
    solid1_authority_document!("solid-h-jsx-runtime-default"),
    solid1_authority_document!("solid-h-jsx-dev-runtime-default"),
    solid1_authority_document!("solid-web-storage-default"),
    solid1_authority_document!("scheduled-root-browser"),
    solid1_authority_document!("scheduled-root-node"),
    solid1_authority_document!("debounce-root-default"),
    solid1_authority_document!("rootless-root-default"),
];

const EMBEDDED_SOLID1_BUNDLES: &[EmbeddedBundle] = &[
    solid1_embedded_bundle!("solid-root-browser-development"),
    solid1_embedded_bundle!("solid-root-browser-production"),
    solid1_embedded_bundle!("solid-root-node"),
    solid1_embedded_bundle!("solid-store-browser-development"),
    solid1_embedded_bundle!("solid-store-browser-production"),
    solid1_embedded_bundle!("solid-store-node"),
    solid1_embedded_bundle!("solid-web-browser-development"),
    solid1_embedded_bundle!("solid-web-browser-production"),
    solid1_embedded_bundle!("solid-web-node"),
    solid1_embedded_bundle!("solid-universal-development"),
    solid1_embedded_bundle!("solid-universal-production"),
    solid1_embedded_bundle!("solid-h-jsx-runtime-default"),
    solid1_embedded_bundle!("solid-h-jsx-dev-runtime-default"),
    solid1_embedded_bundle!("solid-web-storage-default"),
    solid1_embedded_bundle!("scheduled-root-browser"),
    solid1_embedded_bundle!("scheduled-root-node"),
    solid1_embedded_bundle!("debounce-root-default"),
    solid1_embedded_bundle!("rootless-root-default"),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Solid1Authority {
    schema_version: u16,
    format: String,
    cases: Vec<Solid1AuthorityCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Solid1AuthorityCase {
    stem: String,
    package: String,
    selector: BundleSelector,
    entrypoint: String,
    document: String,
    closure: ClosureManifest,
    legacy_authority: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConformanceReport {
    closure_identities: BTreeMap<String, CheckedClosure>,
}

#[derive(Deserialize)]
struct CheckedClosure {
    digest: String,
    components: Vec<CheckedClosurePackage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckedClosurePackage {
    name: String,
    version: String,
    integrity: String,
    files_manifest_sha256: String,
}

#[derive(Deserialize)]
struct PublishedAudit {
    packages: Vec<AuditPackage>,
}

#[derive(Deserialize)]
struct AuditPackage {
    name: String,
    #[serde(rename = "exportTargets")]
    export_targets: Vec<AuditTarget>,
}

#[derive(Deserialize)]
struct AuditTarget {
    trace: Vec<String>,
    target: String,
    kind: String,
    #[serde(default)]
    sha256: Option<String>,
}

/// Builds every exact artifact case modeled for the three first-party RC.3
/// packages. One document/receipt pair owns one artifact case because receipt
/// roots are selected-case identities.
pub fn solid2_rc3_bundles() -> Result<Vec<FirstPartyBundle>, FirstPartyBundleError> {
    let report: ConformanceReport = serde_json::from_slice(CONFORMANCE_BYTES)?;
    let audit: PublishedAudit = serde_json::from_slice(AUDIT_BYTES)?;
    let closures = closure_manifests(report)?;
    let mut grouped = BTreeMap::<
        (String, String),
        (
            ArtifactCase,
            solid_reactive_ir::contract_semantics::PackageIdentity,
        ),
    >::new();
    let proposals = conformance_corpus()
        .into_iter()
        .map(|row| row.proposal)
        .chain(reviewed_support_corpus());
    for proposal in proposals {
        let normalized = proposal.normalize()?;
        if !matches!(
            normalized.package().name.as_str(),
            "solid-js" | "@solidjs/signals" | "@solidjs/web"
        ) {
            continue;
        }
        for mut artifact in normalized.artifact_cases().iter().cloned() {
            let original_conditions = artifact
                .resolution_trace
                .iter()
                .map(|step| step.condition.clone())
                .collect::<Vec<_>>();
            artifact.resolution_trace = exact_trace(
                &audit,
                normalized.package().name.as_str(),
                &artifact,
                &original_conditions,
            )?;
            let closure = closures
                .get(artifact.dependency_closure.as_str())
                .ok_or_else(|| {
                    inconsistent(format!(
                        "artifact {} names an unrecorded closure {}",
                        artifact.id,
                        artifact.dependency_closure.as_str()
                    ))
                })?;
            artifact.dependency_closure =
                solid_reactive_ir::contract_semantics::Digest::parse(closure.digest.clone())
                    .map_err(|error| inconsistent(error.to_string()))?;
            let key = (normalized.package().name.clone(), artifact.id.clone());
            match grouped.entry(key) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((artifact, normalized.package().clone()));
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    let (existing, package) = entry.get_mut();
                    let mut left = existing.clone();
                    let mut right = artifact.clone();
                    left.exports.clear();
                    right.exports.clear();
                    if left != right || package != normalized.package() {
                        return Err(inconsistent(format!(
                            "artifact case {} has contradictory checked identities",
                            artifact.id
                        )));
                    }
                    for (name, semantics) in artifact.exports {
                        if let Some(current) = existing.exports.remove(&name) {
                            existing.exports.insert(
                                name,
                                merge_checked_export(current, semantics, &existing.id)?,
                            );
                        } else {
                            existing.exports.insert(name, semantics);
                        }
                    }
                }
            }
        }
    }

    let checked_authority = [
        (CONFORMANCE_BYTES.len() as u64).to_be_bytes().as_slice(),
        CONFORMANCE_BYTES,
        (REVIEWED_SUPPORT_BYTES.len() as u64)
            .to_be_bytes()
            .as_slice(),
        REVIEWED_SUPPORT_BYTES,
    ]
    .concat();
    let mut bundles = Vec::new();
    for ((package_name, artifact_id), (artifact, package)) in grouped {
        let complete = ContractProposal::new(package, vec![artifact]).normalize()?;
        if !has_local_closure(&complete) {
            // An all-open case has no behavior that a receipt can authorize.
            // The conformance corpus still records the exact open domain, but
            // publishing it as an accepted analyzer input would grant no fact.
            continue;
        }
        let AcceptedArtifacts { document, receipt } = accept_checked_corpus_case(
            &complete,
            "solid-checker-phase14-rc3-bundle-authority",
            &checked_authority,
            true,
        )
        .map_err(|error| {
            inconsistent(format!(
                "cannot issue {package_name}:{artifact_id} from checked corpus: {error}"
            ))
        })?;
        let accepted_case = crate::contract_document_v2::decode(&document)
            .and_then(|proposal| proposal.normalize())
            .map_err(|error| {
                inconsistent(format!("generated accepted document is invalid: {error}"))
            })?
            .artifact_cases()[0]
            .id
            .clone();
        bundles.push(FirstPartyBundle {
            file_stem: bundle_stem(&package_name, &artifact_id),
            package: package_name,
            artifact_case: accepted_case,
            selector: None,
            document,
            receipt,
        });
    }
    bundles.sort_by(|left, right| left.file_stem.cmp(&right.file_stem));
    Ok(bundles)
}

/// Replays the normalized Solid 1 authority captured during the atomic Phase
/// 14 migration and issues ordinary proof-bound receipts. The source documents
/// already use the internal semantic model and temporary-v2 wire format; no
/// legacy summary IDs, variants, evidence tiers, or unknown sentinels are read.
pub fn solid1_bundles() -> Result<Vec<FirstPartyBundle>, FirstPartyBundleError> {
    let authority: Solid1Authority = serde_json::from_slice(SOLID1_AUTHORITY_INDEX_BYTES)?;
    if authority.schema_version != 2
        || authority.format != "solid-checker-phase14-solid1-normalized-authority"
    {
        return Err(inconsistent(
            "Solid 1 authority has the wrong format or version",
        ));
    }
    let sources = SOLID1_AUTHORITY_DOCUMENTS
        .iter()
        .map(|source| (source.name, source.document))
        .collect::<BTreeMap<_, _>>();
    if authority.cases.len() != sources.len() {
        return Err(inconsistent(
            "Solid 1 authority index and document census disagree",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut bundles = Vec::new();
    for case in authority.cases {
        if case.legacy_authority.trim().is_empty() || !seen.insert(case.document.clone()) {
            return Err(inconsistent(
                "Solid 1 authority repeats a document or omits provenance",
            ));
        }
        case.closure.validate()?;
        let source = sources.get(case.document.as_str()).ok_or_else(|| {
            inconsistent(format!(
                "Solid 1 authority names missing document {}",
                case.document
            ))
        })?;
        let normalized = crate::contract_document_v2::decode(source)
            .and_then(|proposal| proposal.normalize())
            .map_err(|error| {
                inconsistent(format!(
                    "Solid 1 authority {} cannot be decoded: {error}",
                    case.document
                ))
            })?;
        let [artifact] = normalized.artifact_cases() else {
            return Err(inconsistent(format!(
                "Solid 1 authority {} must contain exactly one artifact case",
                case.document
            )));
        };
        if normalized.package().name != case.package
            || artifact.entrypoint != case.entrypoint
            || artifact.dependency_closure
                != solid_reactive_ir::contract_semantics::Digest::parse(
                    case.closure.digest.clone(),
                )?
        {
            return Err(inconsistent(format!(
                "Solid 1 authority metadata disagrees with {}",
                case.document
            )));
        }
        if !has_local_closure(&normalized) {
            // Exact acquisition can prove an exported subpath while the
            // runtime/declaration pair exposes no common value binding. Keep
            // that case in the authority census, but an empty semantic case
            // has nothing a receipt may authorize for analysis.
            continue;
        }
        let AcceptedArtifacts { document, receipt } = accept_checked_corpus_case(
            &normalized,
            "solid-checker-phase14-solid1-normalized-authority",
            source,
            true,
        )
        .map_err(|error| {
            inconsistent(format!(
                "Solid 1 authority {} cannot issue a receipt: {error}",
                case.document
            ))
        })?;
        bundles.push(FirstPartyBundle {
            file_stem: case.stem,
            package: case.package,
            artifact_case: artifact.id.clone(),
            selector: Some(case.selector),
            document,
            receipt,
        });
    }
    if seen.len() != sources.len() {
        return Err(inconsistent(
            "Solid 1 authority carries an unindexed document",
        ));
    }
    bundles.sort_by(|left, right| left.file_stem.cmp(&right.file_stem));
    Ok(bundles)
}

/// Builds receipt-validated analyzer inputs for exact first-party imports
/// whose installed package closure reproduces the checked published-file
/// census for the selected dialect.
/// A missing runtime/build selection, mutated file, unattested lock identity,
/// or open-only artifact case refuses only that import.
pub fn bundled_first_party_contract_index(
    dialect_id: &str,
    project_directory: &Path,
    facts: &solid_facts::ProjectFacts,
    runtime: &RuntimeEnvironment,
) -> Result<AcceptedContractIndex, crate::ContractFailure> {
    let (sources, closures, packages): (
        &[EmbeddedBundle],
        BTreeMap<String, ClosureManifest>,
        BTreeSet<&str>,
    ) = match dialect_id {
        "solid-v1" => {
            let authority: Solid1Authority = serde_json::from_slice(SOLID1_AUTHORITY_INDEX_BYTES)
                .map_err(|error| {
                crate::ContractFailure::DocumentDecode {
                    message: format!("decode checked Solid 1 closure census: {error}"),
                }
            })?;
            let mut closures = BTreeMap::new();
            for case in authority.cases {
                case.closure.validate().map_err(|error| {
                    crate::ContractFailure::InvalidSemanticModel {
                        reason: error.to_string(),
                    }
                })?;
                if let Some(existing) =
                    closures.insert(case.closure.digest.clone(), case.closure.clone())
                    && existing != case.closure
                {
                    return Err(crate::ContractFailure::InvalidSemanticModel {
                        reason: format!(
                            "Solid 1 authority assigns different closures to digest {:?}",
                            case.closure.digest
                        ),
                    });
                }
            }
            (
                EMBEDDED_SOLID1_BUNDLES,
                closures,
                [
                    "solid-js",
                    "@solid-primitives/scheduled",
                    "@solid-primitives/debounce",
                    "@solid-primitives/rootless",
                ]
                .into_iter()
                .collect(),
            )
        }
        "solid-v2" => {
            let report: ConformanceReport =
                serde_json::from_slice(CONFORMANCE_BYTES).map_err(|error| {
                    crate::ContractFailure::DocumentDecode {
                        message: format!("decode checked RC.3 closure census: {error}"),
                    }
                })?;
            let closures = closure_manifests(report).map_err(|error| {
                crate::ContractFailure::InvalidSemanticModel {
                    reason: error.to_string(),
                }
            })?;
            (
                EMBEDDED_BUNDLES,
                closures,
                ["solid-js", "@solidjs/signals", "@solidjs/web"]
                    .into_iter()
                    .collect(),
            )
        }
        other => {
            return Err(crate::ContractFailure::InvalidSemanticModel {
                reason: format!("unsupported first-party bundle dialect {other:?}"),
            });
        }
    };
    let mut bundles = Vec::new();
    for source in sources {
        bundles.push(
            crate::contract_interface::load_receipt_issued_embedded_contract(
                source.document,
                source.receipt,
            )?,
        );
    }
    let solid1_selectors = if dialect_id == "solid-v1" {
        let authority: Solid1Authority = serde_json::from_slice(SOLID1_AUTHORITY_INDEX_BYTES)
            .map_err(|error| crate::ContractFailure::DocumentDecode {
                message: format!("decode checked Solid 1 selectors: {error}"),
            })?;
        let mut selectors = BTreeMap::new();
        for case in authority.cases {
            let source = SOLID1_AUTHORITY_DOCUMENTS
                .iter()
                .find(|source| source.name == case.document)
                .ok_or_else(|| crate::ContractFailure::DocumentDecode {
                    message: format!(
                        "Solid 1 authority references missing document {:?}",
                        case.document
                    ),
                })?;
            let contract = crate::contract_document_v2::decode(source.document)
                .and_then(|proposal| proposal.normalize())?;
            if !has_local_closure(&contract) {
                continue;
            }
            let digest = contract.semantic_digest().as_str().to_owned();
            if let Some(existing) = selectors.insert(digest.clone(), case.selector.clone())
                && existing != case.selector
            {
                return Err(crate::ContractFailure::InvalidSemanticModel {
                    reason: format!(
                        "Solid 1 semantic digest {digest:?} has contradictory selectors"
                    ),
                });
            }
        }
        selectors
    } else {
        BTreeMap::new()
    };
    let Some(attested) = &facts.resolved_imports else {
        return Ok(AcceptedContractIndex::default());
    };
    let mut checked_roots = BTreeMap::<PathBuf, bool>::new();
    let mut inputs = Vec::new();
    let mut seen = BTreeSet::new();
    let mut uncertifiable = BTreeSet::new();
    for (importer, import) in attested.iter() {
        if import.resolution == solid_facts::ImportResolution::Unresolved
            && local_contract_exists(project_directory, &import.text)
        {
            uncertifiable.insert((importer.to_owned(), import.text.to_string()));
            continue;
        }
        let Some(package_name) = import
            .resolver_package_name
            .as_deref()
            .or(import.package_name.as_deref())
        else {
            continue;
        };
        if !packages.contains(package_name) {
            if import.resolution == solid_facts::ImportResolution::NodeModules
                && import
                    .package_manifest
                    .as_deref()
                    .is_some_and(manifest_uses_solid)
            {
                uncertifiable.insert((importer.to_owned(), import.text.to_string()));
            }
            continue;
        }
        let Some(package_version) = import
            .resolver_package_version
            .as_deref()
            .or(import.package_version.as_deref())
        else {
            continue;
        };
        let Some(manifest) = import.package_manifest.as_deref() else {
            continue;
        };
        let Some(resolved_package_root) = Path::new(manifest).parent() else {
            continue;
        };
        let Some(package_root) =
            lexical_install_root(project_directory, resolved_package_root, package_name)
        else {
            continue;
        };
        let entrypoint = match requested_entrypoint(package_name, &import.text) {
            Some(entrypoint) => entrypoint,
            None => continue,
        };
        let Some(contract) = bundles.iter().find(|contract| {
            contract.package().name == package_name
                && contract.package().version == package_version
                && contract.artifact_case().entrypoint == entrypoint
                && if dialect_id == "solid-v1" {
                    solid1_selectors
                        .get(contract.receipt().semantic_digest.as_str())
                        .is_some_and(|selector| selector_selects(selector, runtime))
                } else {
                    environment_selects(contract.artifact_case(), runtime)
                }
        }) else {
            continue;
        };
        let Some(closure) = closures.get(contract.receipt().closure_digest.as_str()) else {
            continue;
        };
        if !checked_closure_matches(
            project_directory,
            &package_root,
            closure,
            &mut checked_roots,
        ) {
            continue;
        }
        if !artifact_files_match(&package_root, contract.artifact_case()) {
            continue;
        }
        let key = (importer.to_owned(), import.text.to_string());
        if seen.insert(key.clone()) {
            inputs.push(AcceptedContractInput {
                importer: key.0,
                specifier: key.1,
                contract: contract.clone(),
            });
        }
    }
    AcceptedContractIndex::new(inputs)
        .map(|index| index.with_uncertifiable_imports(uncertifiable))
        .map_err(|error| crate::ContractFailure::IdentityMismatch {
            reason: error.to_string(),
        })
}

fn local_contract_exists(project: &Path, specifier: &str) -> bool {
    let mut segments = specifier.split('/');
    let Some(first) = segments.next() else {
        return false;
    };
    let package = if first.starts_with('@') {
        let Some(second) = segments.next() else {
            return false;
        };
        format!("{first}/{second}")
    } else {
        first.to_owned()
    };
    project
        .join(".solid-checker/contracts")
        .join(package)
        .join("solid-reactivity.json")
        .is_file()
}

fn manifest_uses_solid(path: &str) -> bool {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|manifest| {
            ["dependencies", "peerDependencies", "optionalDependencies"]
                .into_iter()
                .filter_map(|field| manifest[field].as_object())
                .flat_map(|dependencies| dependencies.keys())
                .any(|name| name == "solid-js" || name.starts_with("@solidjs/"))
        })
}

fn requested_entrypoint(package: &str, specifier: &str) -> Option<String> {
    if specifier == package {
        return Some(".".into());
    }
    specifier
        .strip_prefix(package)
        .and_then(|suffix| suffix.strip_prefix('/'))
        .filter(|suffix| !suffix.is_empty())
        .map(|suffix| format!("./{suffix}"))
}

/// Recover the lockfile-owned lexical install from a resolver-realpathed
/// package manifest.
///
/// TypeScript deliberately realpaths packages reached through a node_modules
/// symlink. Registry integrity, however, belongs to the exact lexical install
/// named by the project's lockfile. A candidate is usable only when its
/// canonical directory is exactly the resolver's package directory; package
/// name or path shape alone never selects a bundle.
fn lexical_install_root(
    project: &Path,
    resolved_package_root: &Path,
    package_name: &str,
) -> Option<PathBuf> {
    let resolved = fs::canonicalize(resolved_package_root).ok()?;
    let relative = package_name.split('/').collect::<PathBuf>();
    project.ancestors().find_map(|ancestor| {
        let candidate = ancestor.join("node_modules").join(&relative);
        (candidate.is_dir()
            && fs::canonicalize(&candidate)
                .ok()
                .is_some_and(|canonical| canonical == resolved))
        .then_some(candidate)
    })
}

fn selector_selects(selector: &BundleSelector, runtime: &RuntimeEnvironment) -> bool {
    selector
        .target
        .is_none_or(|target| runtime.target.unwrap_or(RuntimeTarget::Browser) == target)
        && selector
            .build
            .is_none_or(|build| runtime.build.unwrap_or(RuntimeBuild::Development) == build)
}

fn environment_selects(artifact: &ArtifactCase, runtime: &RuntimeEnvironment) -> bool {
    let target = runtime.target.unwrap_or(RuntimeTarget::Browser);
    let build = runtime.build.unwrap_or(RuntimeBuild::Development);
    if build != RuntimeBuild::Development {
        return false;
    }
    let server = target == RuntimeTarget::Node
        || matches!(
            runtime.rendering,
            Some(RuntimeRendering::StringSsr | RuntimeRendering::StreamingSsr)
        );
    let path = artifact.runtime.path.as_str();
    if artifact.entrypoint == "./server-functions" {
        return server == path.contains("server.dev");
    }
    if artifact.entrypoint == "." && artifact.runtime.path.contains("server") {
        server
    } else {
        !server
    }
}

fn artifact_files_match(package_root: &Path, artifact: &ArtifactCase) -> bool {
    [
        (&artifact.runtime.path, artifact.runtime.digest.as_str()),
        (
            &artifact.declarations.path,
            artifact.declarations.digest.as_str(),
        ),
    ]
    .into_iter()
    .all(|(path, digest)| file_digest(&package_root.join(path)) == Some(digest.to_owned()))
}

fn checked_closure_matches(
    project_directory: &Path,
    selected_package_root: &Path,
    closure: &ClosureManifest,
    cache: &mut BTreeMap<PathBuf, bool>,
) -> bool {
    closure.packages.iter().all(|package| {
        let Some(root) = locate_package(project_directory, selected_package_root, &package.name)
        else {
            return false;
        };
        if let Some(answer) = cache.get(&root) {
            return *answer;
        }
        let answer = package_manifest_matches(&root, &package.name, &package.version)
            && installed_package_integrity(project_directory, &root)
                .ok()
                .flatten()
                .as_deref()
                == Some(package.integrity.as_str())
            && package_files_manifest(&root).as_deref()
                == package.files_manifest_digest.strip_prefix("sha256:");
        cache.insert(root, answer);
        answer
    })
}

fn locate_package(project: &Path, selected_root: &Path, name: &str) -> Option<PathBuf> {
    if fs::read(selected_root.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|manifest| manifest["name"].as_str() == Some(name))
    {
        return Some(selected_root.to_owned());
    }
    let relative = name.split('/').collect::<PathBuf>();
    let mut candidates = vec![selected_root.join("node_modules").join(&relative)];
    candidates.extend(
        selected_root
            .ancestors()
            .chain(project.ancestors())
            .map(|ancestor| ancestor.join("node_modules").join(&relative)),
    );
    candidates.into_iter().find(|candidate| candidate.is_dir())
}

fn package_manifest_matches(root: &Path, name: &str, version: &str) -> bool {
    fs::read(root.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|manifest| {
            manifest["name"].as_str() == Some(name) && manifest["version"].as_str() == Some(version)
        })
}

fn package_files_manifest(root: &Path) -> Option<String> {
    fn visit(directory: &Path, files: &mut Vec<PathBuf>) -> Option<()> {
        for entry in fs::read_dir(directory).ok()? {
            let entry = entry.ok()?;
            if entry.file_name() == "node_modules" {
                continue;
            }
            let file_type = entry.file_type().ok()?;
            if file_type.is_dir() {
                visit(&entry.path(), files)?;
            } else if file_type.is_file() {
                files.push(entry.path());
            } else {
                return None;
            }
        }
        Some(())
    }
    let mut files = Vec::new();
    visit(root, &mut files)?;
    files.sort_by_key(|path| {
        let relative = path
            .strip_prefix(root)
            .expect("visited path is below root")
            .to_string_lossy()
            .replace('\\', "/");
        // Phase 13's published-file authority used localeCompare over ASCII
        // package paths. Spell its effective order explicitly so replay does
        // not depend on the host locale and the Rust consumer hashes the same
        // census: ASCII case-fold first, original bytes as the tie-breaker.
        (relative.to_ascii_lowercase(), relative)
    });
    let mut manifest = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(root)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&path).ok()?;
        manifest.extend_from_slice(
            format!("{:x}  {relative}\n", sha2::Sha256::digest(bytes)).as_bytes(),
        );
    }
    Some(format!("{:x}", sha2::Sha256::digest(manifest)))
}

fn file_digest(path: &Path) -> Option<String> {
    fs::read(path)
        .ok()
        .map(|bytes| format!("sha256:{:x}", sha2::Sha256::digest(bytes)))
}

fn has_local_closure(contract: &solid_reactive_ir::contract_semantics::NormalizedContract) -> bool {
    contract.artifact_cases().iter().any(|artifact| {
        artifact.exports.values().any(|export| {
            let mut export = export.clone();
            !export.open_proposed_closure().is_empty()
        })
    })
}

fn merge_checked_export(
    left: ExportSemantics,
    right: ExportSemantics,
    artifact_case: &str,
) -> Result<ExportSemantics, FirstPartyBundleError> {
    if left.identity != right.identity
        || left.shape != right.shape
        || left.stability != right.stability
    {
        return Err(inconsistent(format!(
            "artifact case {artifact_case} export {:?} has contradictory identity, value shape, or stability",
            left.identity.public_name
        )));
    }
    let name = left.identity.public_name.clone();
    let left_claims = left.call.claims();
    let right_claims = right.call.claims();
    let claims = CallClaims {
        callbacks: KnowledgeSet::join([
            left_claims.callbacks.clone(),
            right_claims.callbacks.clone(),
        ]),
        reads: KnowledgeSet::join([left_claims.reads.clone(), right_claims.reads.clone()]),
        writes: KnowledgeSet::join([left_claims.writes.clone(), right_claims.writes.clone()]),
        creates: KnowledgeSet::join([left_claims.creates.clone(), right_claims.creates.clone()]),
        invalidates: KnowledgeSet::join([
            left_claims.invalidates.clone(),
            right_claims.invalidates.clone(),
        ]),
        throws: KnowledgeSet::join([left_claims.throws.clone(), right_claims.throws.clone()]),
        returns: KnowledgeSet::join([left_claims.returns.clone(), right_claims.returns.clone()]),
        cleanups: KnowledgeSet::join([left_claims.cleanups.clone(), right_claims.cleanups.clone()]),
        disposals: KnowledgeSet::join([
            left_claims.disposals.clone(),
            right_claims.disposals.clone(),
        ]),
    };
    let mut operations = left.call.operations;
    merge_named_rows(
        &mut operations,
        right.call.operations,
        |operation| operation.id.0.as_str(),
        artifact_case,
        &name,
        "operation",
    )?;
    let mut resources = left.call.resources;
    merge_named_rows(
        &mut resources,
        right.call.resources,
        |resource| resource.id.0.as_str(),
        artifact_case,
        &name,
        "resource",
    )?;
    let mut edges = left.call.edges;
    edges.extend(right.call.edges);
    edges.sort();
    edges.dedup();
    let guards = GuardPartition {
        cases: KnowledgeSet::join([left.call.guards.cases, right.call.guards.cases]),
    };
    Ok(ExportSemantics {
        identity: left.identity,
        shape: left.shape,
        stability: left.stability,
        call: CallSemantics::new(claims, operations, edges, resources, guards),
    })
}

fn merge_named_rows<T: Eq>(
    output: &mut Vec<T>,
    incoming: Vec<T>,
    key: impl Fn(&T) -> &str,
    artifact_case: &str,
    export: &str,
    kind: &str,
) -> Result<(), FirstPartyBundleError> {
    for row in incoming {
        match output.iter().find(|current| key(current) == key(&row)) {
            Some(current) if current != &row => {
                return Err(inconsistent(format!(
                    "artifact case {artifact_case} export {export:?} has contradictory {kind} {:?}",
                    key(&row)
                )));
            }
            Some(_) => {}
            None => output.push(row),
        }
    }
    Ok(())
}

fn closure_manifests(
    report: ConformanceReport,
) -> Result<BTreeMap<String, ClosureManifest>, FirstPartyBundleError> {
    let mut output = BTreeMap::new();
    for closure in report.closure_identities.into_values() {
        let expected = format!("sha256:{}", closure.digest);
        let manifest = ClosureManifest::from_package_census(
            closure
                .components
                .into_iter()
                .map(|package| ClosurePackageIdentity {
                    name: package.name,
                    version: package.version,
                    integrity: package.integrity,
                    files_manifest_digest: format!("sha256:{}", package.files_manifest_sha256),
                })
                .collect(),
        )?;
        if manifest.digest != expected {
            return Err(inconsistent(format!(
                "package census digest {} does not reproduce checked digest {expected}",
                manifest.digest
            )));
        }
        output.insert(expected, manifest);
    }
    Ok(output)
}

fn exact_trace(
    audit: &PublishedAudit,
    package: &str,
    artifact: &ArtifactCase,
    runtime_conditions: &[String],
) -> Result<Vec<ResolutionStep>, FirstPartyBundleError> {
    let package = audit
        .packages
        .iter()
        .find(|candidate| candidate.name == package)
        .ok_or_else(|| inconsistent(format!("published audit has no package {package:?}")))?;
    let runtime = select_target(
        package,
        artifact,
        "runtime",
        &artifact.runtime.path,
        artifact.runtime.digest.as_str(),
        runtime_conditions,
    )?;
    let declarations = select_target(
        package,
        artifact,
        "declaration",
        &artifact.declarations.path,
        artifact.declarations.digest.as_str(),
        runtime_conditions,
    )?;
    Ok(vec![
        ResolutionStep {
            condition: "runtime".into(),
            target: trace_pointer(&runtime.trace),
        },
        ResolutionStep {
            condition: "types".into(),
            target: trace_pointer(&declarations.trace),
        },
    ])
}

fn select_target<'a>(
    package: &'a AuditPackage,
    artifact: &ArtifactCase,
    kind: &str,
    path: &str,
    digest: &str,
    runtime_conditions: &[String],
) -> Result<&'a AuditTarget, FirstPartyBundleError> {
    let digest = digest.trim_start_matches("sha256:");
    let path = path.trim_start_matches("./");
    let mut candidates = package
        .export_targets
        .iter()
        .filter(|target| target.kind == kind)
        .filter(|target| target.target.trim_start_matches("./") == path)
        .filter(|target| target.sha256.as_deref() == Some(digest))
        .filter(|target| {
            target
                .trace
                .first()
                .is_some_and(|value| value == &artifact.entrypoint)
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|target| {
        std::cmp::Reverse(common_prefix(
            target.trace.get(1..).unwrap_or_default(),
            runtime_conditions,
        ))
    });
    let Some(selected) = candidates.first().copied() else {
        return Err(inconsistent(format!(
            "published audit has no exact {kind} branch for {}:{} ({path})",
            package.name, artifact.id
        )));
    };
    let score = common_prefix(
        selected.trace.get(1..).unwrap_or_default(),
        runtime_conditions,
    );
    if candidates.get(1).is_some_and(|candidate| {
        common_prefix(
            candidate.trace.get(1..).unwrap_or_default(),
            runtime_conditions,
        ) == score
    }) {
        return Err(inconsistent(format!(
            "published audit has ambiguous {kind} branches for {}:{}",
            package.name, artifact.id
        )));
    }
    Ok(selected)
}

fn common_prefix(left: &[String], right: &[String]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

fn trace_pointer(trace: &[String]) -> String {
    format!(
        "/exports/{}",
        trace
            .iter()
            .map(|segment| segment.replace('~', "~0").replace('/', "~1"))
            .collect::<Vec<_>>()
            .join("/")
    )
}

fn bundle_stem(package: &str, artifact: &str) -> String {
    match (package, artifact) {
        ("solid-js", "solid-browser-development") => "solid-js".into(),
        ("@solidjs/signals", "signals-development") => "solidjs-signals".into(),
        ("@solidjs/web", "web-browser-development") => "solidjs-web".into(),
        _ => format!(
            "{}--{artifact}",
            package.trim_start_matches('@').replace(['/', '@'], "-")
        ),
    }
}

fn inconsistent(message: impl Into<String>) -> FirstPartyBundleError {
    FirstPartyBundleError::Inconsistent(message.into())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::contract_document_v2;

    #[test]
    fn checked_rc3_corpus_produces_receipt_issued_single_case_bundles() {
        let bundles = solid2_rc3_bundles().unwrap();
        assert_eq!(
            bundles
                .iter()
                .map(|bundle| bundle.file_stem.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "solid-js",
                "solidjs-signals",
                "solidjs-web",
                "solidjs-web--server-functions-browser-client",
                "solidjs-web--server-functions-node-server",
                "solidjs-web--web-node-server",
            ])
        );
        for bundle in bundles {
            let contract = contract_document_v2::decode(&bundle.document)
                .unwrap()
                .normalize()
                .unwrap();
            assert_eq!(contract.package().name, bundle.package);
            assert_eq!(contract.artifact_cases().len(), 1);
            assert_eq!(contract.artifact_cases()[0].id, bundle.artifact_case);
            assert!(!bundle.receipt.is_empty());
        }
    }

    #[cfg(unix)]
    #[test]
    fn resolver_realpath_recovers_only_the_exact_lockfile_owned_symlink_install() {
        use std::os::unix::fs::symlink;
        use std::time::{SystemTime, UNIX_EPOCH};

        let temporary = std::env::temp_dir().join(format!(
            "solid-checker-first-party-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let project = temporary.join("project");
        let actual = temporary.join("store/signals");
        let wrong = temporary.join("store/wrong-signals");
        fs::create_dir_all(project.join("node_modules/@solidjs")).unwrap();
        fs::create_dir_all(&actual).unwrap();
        fs::create_dir_all(&wrong).unwrap();
        symlink(&actual, project.join("node_modules/@solidjs/signals")).unwrap();

        assert_eq!(
            lexical_install_root(&project, &actual, "@solidjs/signals"),
            Some(project.join("node_modules/@solidjs/signals"))
        );
        assert_eq!(
            lexical_install_root(&project, &wrong, "@solidjs/signals"),
            None
        );
        fs::remove_dir_all(temporary).unwrap();
    }
}
