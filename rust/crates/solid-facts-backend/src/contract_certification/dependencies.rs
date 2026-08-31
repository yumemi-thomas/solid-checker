//! Policy-2 dependency composition planning.
//!
//! This module owns canonical bottom-up ordering, cycle refusal, and exact
//! policy-2 receipt composition. A caller may transport opaque receipts, but
//! cannot turn a policy-1 receipt or a caller-provided digest into dependency
//! authority.

use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::certification::{
    DependencyDemandInput, ProofDemandSubject, ProofFamily,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};
use thiserror::Error;

use super::{
    AuthenticatedPolicy2Receipt, CertificationPlan, CertificationPlanningError,
    CertificationPlanningTransaction, CertificationRequest, ConfiguredReceiptIssuer,
    FinalizedPolicy2Contract, PublishedArchive, TypeFactsProducerPin, UntrustedArtifactEnvelope,
    policy2_resolved_import_root,
};

const POLICY_2_GRAPH_NODE_LIMIT: usize = 256;
const POLICY_2_GRAPH_DEPTH_LIMIT: usize = 64;

/// Package-manager selection compared with independently authenticated
/// registry metadata and archive bytes. These fields are untrusted inputs; the
/// graph planner accepts them only when they describe the exact snapshot it
/// rebuilt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedGraphLockSelection {
    package_manager: String,
    lockfile_digest: String,
    locator: String,
    package_name: String,
    package_version: String,
    integrity: String,
}

/// One compiler-source dependency whose exact published bytes may contribute
/// declarations to Type Facts without claiming any runtime semantics.
pub struct PublishedGraphSourceRequest {
    archive: PublishedArchive,
    lock_selection: PublishedGraphLockSelection,
    installed_package_root: String,
}

impl PublishedGraphSourceRequest {
    #[must_use]
    pub fn new(
        archive: PublishedArchive,
        lock_selection: PublishedGraphLockSelection,
        installed_package_root: impl Into<String>,
    ) -> Self {
        Self {
            archive,
            lock_selection,
            installed_package_root: installed_package_root.into(),
        }
    }
}

#[derive(Clone)]
pub(super) struct VerifiedGraphSourcePackage {
    pub(super) identity: String,
    pub(super) installed_package_root: String,
    pub(super) snapshot: super::ArtifactSnapshot,
}

impl PublishedGraphLockSelection {
    /// Replays an exact Bun text lock selection. The digest binds the original
    /// bytes (including formatting), while selection uses a conservative
    /// trailing-comma normalization matching Bun's JSON-like lock syntax.
    pub fn from_bun_lock(
        lockfile: &[u8],
        locator: impl Into<String>,
        package_name: impl Into<String>,
        package_version: impl Into<String>,
    ) -> Result<Self, super::ArtifactSnapshotError> {
        if lockfile.len() > 8 * 1024 * 1024 {
            return Err(super::ArtifactSnapshotError::ResourceLimit(
                "lockfile bytes exceed graph policy limit".into(),
            ));
        }
        let locator = locator.into();
        let package_name = package_name.into();
        let package_version = package_version.into();
        let source = std::str::from_utf8(lockfile).map_err(|_| {
            super::ArtifactSnapshotError::InvalidProvenance(
                "Bun lockfile is not valid UTF-8".into(),
            )
        })?;
        let normalized = normalize_json_trailing_commas(source);
        let document: serde_json::Value = serde_json::from_str(&normalized).map_err(|error| {
            super::ArtifactSnapshotError::InvalidProvenance(format!(
                "Bun lockfile cannot be decoded: {error}"
            ))
        })?;
        let exact = format!("{package_name}@{package_version}");
        let selections = document
            .get("packages")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flat_map(|packages| packages.iter())
            .filter_map(|(key, record)| {
                if key != &locator && key != &exact {
                    return None;
                }
                let record = record.as_array()?;
                let identifier = record.first()?.as_str()?;
                if key != &exact && identifier != exact {
                    return None;
                }
                record
                    .get(3)?
                    .as_str()
                    .map(|integrity| (key.to_owned(), integrity.to_owned()))
            })
            .collect::<BTreeSet<_>>();
        let (selected_locator, integrity) =
            match selections.into_iter().collect::<Vec<_>>().as_slice() {
                [selection] => selection.clone(),
                [] => {
                    return Err(super::ArtifactSnapshotError::InvalidProvenance(format!(
                        "Bun lockfile has no exact selection for {exact}"
                    )));
                }
                _ => {
                    return Err(super::ArtifactSnapshotError::InvalidProvenance(format!(
                        "Bun lockfile has ambiguous selections for {exact}"
                    )));
                }
            };
        if locator != selected_locator {
            return Err(super::ArtifactSnapshotError::InvalidProvenance(format!(
                "Bun lock locator {locator:?} does not select exact record {selected_locator:?}"
            )));
        }
        Self::new(
            "bun",
            format!("sha256:{:x}", Sha256::digest(lockfile)),
            locator,
            package_name,
            package_version,
            integrity,
        )
    }

    pub(crate) fn new(
        package_manager: impl Into<String>,
        lockfile_digest: impl Into<String>,
        locator: impl Into<String>,
        package_name: impl Into<String>,
        package_version: impl Into<String>,
        integrity: impl Into<String>,
    ) -> Result<Self, super::ArtifactSnapshotError> {
        let value = Self {
            package_manager: package_manager.into(),
            lockfile_digest: lockfile_digest.into(),
            locator: locator.into(),
            package_name: package_name.into(),
            package_version: package_version.into(),
            integrity: integrity.into(),
        };
        for (field, name) in [
            (&value.package_manager, "package manager"),
            (&value.locator, "lock locator"),
            (&value.package_name, "package name"),
            (&value.package_version, "package version"),
        ] {
            super::validate_coordinate(field, name)?;
        }
        super::validate_sha256(&value.lockfile_digest, "lockfile digest")?;
        super::validate_integrity_shape(&value.integrity)?;
        Ok(value)
    }
}

fn normalize_json_trailing_commas(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut normalized = String::with_capacity(source.len());
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            normalized.push(char::from(byte));
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            normalized.push('"');
            index += 1;
            continue;
        }
        if byte == b',' {
            let mut lookahead = index + 1;
            while lookahead < bytes.len() && bytes[lookahead].is_ascii_whitespace() {
                lookahead += 1;
            }
            if lookahead < bytes.len() && matches!(bytes[lookahead], b'}' | b']') {
                index += 1;
                continue;
            }
        }
        normalized.push(char::from(byte));
        index += 1;
    }
    normalized
}

/// One untrusted graph acquisition unit. Planning consumes the registry
/// metadata/archive, independently replays the supplied resolution, and only
/// then compares the package-manager selection.
pub struct PublishedGraphNodeRequest {
    certification: CertificationRequest,
    archive: PublishedArchive,
    lock_selection: PublishedGraphLockSelection,
    source_dependencies: Vec<PublishedGraphSourceRequest>,
}

impl PublishedGraphNodeRequest {
    /// Normalizes an open proposal document inside Rust before it can enter
    /// the graph transaction. The proposal remains comparison material;
    /// snapshot replay owns every artifact identity.
    pub fn from_document(
        document: &[u8],
        import_request: crate::artifact_resolution::ImportRequest,
        resolved_import: crate::artifact_resolution::ResolvedImport,
        archive: PublishedArchive,
        lock_selection: PublishedGraphLockSelection,
    ) -> Result<Self, CertificationPlanningError> {
        Self::from_document_with_sources(
            document,
            import_request,
            resolved_import,
            archive,
            lock_selection,
            [],
        )
    }

    pub fn from_document_with_sources(
        document: &[u8],
        import_request: crate::artifact_resolution::ImportRequest,
        resolved_import: crate::artifact_resolution::ResolvedImport,
        archive: PublishedArchive,
        lock_selection: PublishedGraphLockSelection,
        source_dependencies: impl IntoIterator<Item = PublishedGraphSourceRequest>,
    ) -> Result<Self, CertificationPlanningError> {
        let candidate = crate::contract_document::decode(document)?.normalize()?;
        Ok(Self::new_with_sources(
            CertificationRequest::new(candidate, import_request, resolved_import),
            archive,
            lock_selection,
            source_dependencies,
        ))
    }

    #[doc(hidden)]
    #[must_use]
    pub fn new(
        certification: CertificationRequest,
        archive: PublishedArchive,
        lock_selection: PublishedGraphLockSelection,
    ) -> Self {
        Self::new_with_sources(certification, archive, lock_selection, [])
    }

    #[doc(hidden)]
    #[must_use]
    pub fn new_with_sources(
        certification: CertificationRequest,
        archive: PublishedArchive,
        lock_selection: PublishedGraphLockSelection,
        source_dependencies: impl IntoIterator<Item = PublishedGraphSourceRequest>,
    ) -> Self {
        Self {
            certification,
            archive,
            lock_selection,
            source_dependencies: source_dependencies.into_iter().collect(),
        }
    }
}

/// Complete snapshot-derived identity of one graph node. A package/version is
/// deliberately insufficient: every resolver and byte identity that can
/// change the selected behavior participates in equality and graph hashing.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CanonicalDependencyNodeIdentity {
    pub registry_origin: String,
    pub package_manager: String,
    pub package_name: String,
    pub package_version: String,
    pub integrity: String,
    pub lockfile_digest: String,
    pub lock_locator: String,
    pub entrypoint: String,
    pub conditions: Vec<String>,
    pub importer: String,
    pub resolution_kind: String,
    pub runtime_target: String,
    pub runtime_digest: String,
    pub declarations_target: String,
    pub declarations_digest: String,
    pub closure_root: String,
    pub resolved_import_root: String,
    pub snapshot_root: String,
    pub provenance_root: String,
    pub artifact_case: String,
    pub semantic_digest: String,
    pub source_dependencies_root: String,
    digest: String,
}

impl CanonicalDependencyNodeIdentity {
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

struct PlannedGraphNode {
    identity: CanonicalDependencyNodeIdentity,
    plan: CertificationPlan,
    dependencies: Vec<CanonicalDependencyNodeIdentity>,
    source_dependencies: Vec<VerifiedGraphSourcePackage>,
}

/// Opaque native graph plan. Plans are retained in canonical dependency-first
/// order and never reconstructed from the JavaScript benchmark planner.
pub struct PublishedContractGraphPlan {
    nodes: Vec<PlannedGraphNode>,
    root: CanonicalDependencyNodeIdentity,
    graph_root: String,
}

impl PublishedContractGraphPlan {
    #[must_use]
    pub fn graph_root(&self) -> &str {
        &self.graph_root
    }

    #[must_use]
    pub fn root_identity(&self) -> &CanonicalDependencyNodeIdentity {
        &self.root
    }

    #[must_use]
    pub fn dependency_first_identities(&self) -> Vec<&CanonicalDependencyNodeIdentity> {
        self.nodes.iter().map(|node| &node.identity).collect()
    }

    #[must_use]
    pub fn plan(&self, identity: &CanonicalDependencyNodeIdentity) -> Option<&CertificationPlan> {
        self.nodes
            .iter()
            .find(|node| &node.identity == identity)
            .map(|node| &node.plan)
    }

    fn transitive_dependency_plans(
        &self,
        node: &PlannedGraphNode,
    ) -> Result<Vec<&CertificationPlan>, PublishedGraphCertificationError> {
        let mut reachable = BTreeSet::new();
        let mut pending = node.dependencies.clone();
        while let Some(identity) = pending.pop() {
            if !reachable.insert(identity.clone()) {
                continue;
            }
            let dependency = self
                .nodes
                .iter()
                .find(|candidate| candidate.identity == identity)
                .ok_or_else(|| {
                    PublishedGraphCertificationError::MissingPlannedDependency(
                        identity.digest().into(),
                    )
                })?;
            pending.extend(dependency.dependencies.iter().cloned());
        }
        Ok(self
            .nodes
            .iter()
            .filter(|candidate| reachable.contains(&candidate.identity))
            .map(|candidate| &candidate.plan)
            .collect())
    }

    /// Authenticates every dependency-composition demand for one planned
    /// parent. The caller may transport opaque receipts, but cannot construct
    /// this token from a digest or assign a valid receipt to another edge.
    pub fn authenticate_dependency_receipts(
        &self,
        parent: &CanonicalDependencyNodeIdentity,
        receipts: &[(
            &CanonicalDependencyNodeIdentity,
            &AuthenticatedPolicy2Receipt,
        )],
        issuer: &ConfiguredReceiptIssuer,
        revocation_epoch: u64,
    ) -> Result<VerifiedDependencyComposition, DependencyReceiptCompositionError> {
        let node = self
            .nodes
            .iter()
            .find(|node| &node.identity == parent)
            .ok_or(DependencyReceiptCompositionError::ParentOutsideGraph)?;
        VerifiedDependencyComposition::authenticate(
            &node.plan,
            &node.dependencies,
            &node.source_dependencies,
            self.graph_root(),
            receipts,
            issuer,
            revocation_epoch,
        )
    }

    /// Certifies every node through the same policy-2 transaction, retaining
    /// child authority only as opaque receipts and exposing no partially
    /// finalized root if any node fails.
    pub fn certify_value_only(
        &self,
        pin: &TypeFactsProducerPin,
        issuer: &ConfiguredReceiptIssuer,
        revocation_epoch: u64,
    ) -> Result<FinalizedPolicy2Graph, PublishedGraphCertificationError> {
        let type_facts_requests = self.type_facts_requests()?;
        let root_plan = self.plan(self.root_identity()).ok_or_else(|| {
            PublishedGraphCertificationError::MissingPlannedDependency(
                self.root_identity().digest().into(),
            )
        })?;
        let type_facts_evidence = super::type_facts::acquire_and_verify_graph_export_values(
            root_plan,
            &type_facts_requests
                .iter()
                .map(|(_, request)| super::type_facts::GraphExportValueRequest {
                    plan: request.plan,
                    dependencies: request.dependencies.clone(),
                    sources: request.sources,
                })
                .collect::<Vec<_>>(),
            pin,
        )
        .map_err(
            |source| PublishedGraphCertificationError::TypeFactsForGraph {
                graph: self.graph_root().into(),
                source,
            },
        )?;
        let type_facts_by_node = type_facts_requests
            .into_iter()
            .map(|(identity, _)| identity)
            .zip(type_facts_evidence)
            .collect::<BTreeMap<_, _>>();
        self.finalize_value_only_with_type_facts(&type_facts_by_node, pin, issuer, revocation_epoch)
    }

    fn type_facts_requests(
        &self,
    ) -> Result<
        Vec<(String, super::type_facts::GraphExportValueRequest<'_>)>,
        PublishedGraphCertificationError,
    > {
        self.nodes
            .iter()
            .filter(|node| {
                node.plan
                    .demand_graph()
                    .demands()
                    .iter()
                    .any(|demand| demand.family() == ProofFamily::RecursiveValueShape)
            })
            .map(|node| {
                Ok((
                    node.identity.digest().to_owned(),
                    super::type_facts::GraphExportValueRequest {
                        plan: &node.plan,
                        dependencies: self.transitive_dependency_plans(node)?,
                        sources: &node.source_dependencies,
                    },
                ))
            })
            .collect()
    }

    fn finalize_value_only_with_type_facts(
        &self,
        type_facts_by_node: &BTreeMap<String, super::type_facts::VerifiedTypeFactsEvidence>,
        pin: &TypeFactsProducerPin,
        issuer: &ConfiguredReceiptIssuer,
        revocation_epoch: u64,
    ) -> Result<FinalizedPolicy2Graph, PublishedGraphCertificationError> {
        let mut finalized = Vec::<FinalizedGraphNode>::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let proposal = crate::contract_document::encode(
                &node.plan.selected_candidate,
                &crate::contract_document::SidecarDigests::default(),
                false,
            )?;
            let type_facts = type_facts_by_node.get(node.identity.digest());
            let dependency_evidence =
                if node.dependencies.is_empty() && node.source_dependencies.is_empty() {
                    None
                } else {
                    let receipts = node
                        .dependencies
                        .iter()
                        .map(|dependency| {
                            finalized
                                .iter()
                                .find(|candidate| &candidate.identity == dependency)
                                .map(|candidate| (dependency, candidate.finalized.authenticated()))
                                .ok_or_else(|| {
                                    PublishedGraphCertificationError::MissingFinalizedDependency(
                                        dependency.digest().into(),
                                    )
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Some(self.authenticate_dependency_receipts(
                        &node.identity,
                        &receipts,
                        issuer,
                        revocation_epoch,
                    )?)
                };
            let contract = super::finalization::finalize_value_only_with_dependencies(
                &node.plan,
                &proposal,
                type_facts,
                dependency_evidence.as_ref(),
                pin,
                issuer,
                revocation_epoch,
            )
            .map_err(|source| {
                PublishedGraphCertificationError::FinalizationAtNode {
                    node: node.identity.digest().into(),
                    package: format!(
                        "{}@{}",
                        node.identity.package_name, node.identity.package_version
                    ),
                    source,
                }
            })?;
            finalized.push(FinalizedGraphNode {
                identity: node.identity.clone(),
                finalized: contract,
            });
        }
        Ok(FinalizedPolicy2Graph {
            graph_root: self.graph_root.clone(),
            root: self.root.clone(),
            nodes: finalized,
        })
    }
}

/// Certifies a complete root case-set through one Type Facts session and one
/// native bottom-up transaction. Canonical nodes shared by multiple roots are
/// acquired once for evidence; receipt composition remains graph-root-local,
/// so no child receipt is transplanted between root graphs.
pub fn certify_published_contract_graph_case_set(
    graphs: &[PublishedContractGraphPlan],
    pin: &TypeFactsProducerPin,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
) -> Result<Vec<FinalizedPolicy2Graph>, PublishedGraphCertificationError> {
    let first_graph = graphs
        .first()
        .ok_or(PublishedGraphCertificationError::EmptyCaseSet)?;
    let root_plan = first_graph
        .plan(first_graph.root_identity())
        .ok_or_else(|| {
            PublishedGraphCertificationError::MissingPlannedDependency(
                first_graph.root_identity().digest().into(),
            )
        })?;
    let mut requests = BTreeMap::new();
    let mut identities = BTreeMap::new();
    for graph in graphs {
        for (identity, request) in graph.type_facts_requests()? {
            let node = graph
                .nodes
                .iter()
                .find(|node| node.identity.digest() == identity)
                .expect("Type Facts requests originate from retained graph nodes");
            if identities
                .insert(identity.clone(), node.identity.clone())
                .is_some_and(|previous| previous != node.identity)
            {
                return Err(PublishedGraphCertificationError::CanonicalIdentityCollision(identity));
            }
            requests.entry(identity).or_insert(request);
        }
    }
    let request_keys = requests.keys().cloned().collect::<Vec<_>>();
    let request_values = requests.into_values().collect::<Vec<_>>();
    let evidence =
        super::type_facts::acquire_and_verify_graph_export_values(root_plan, &request_values, pin)
            .map_err(
                |source| PublishedGraphCertificationError::TypeFactsForGraph {
                    graph: "published-graph-case-set".into(),
                    source,
                },
            )?;
    let evidence_by_node = request_keys
        .into_iter()
        .zip(evidence)
        .collect::<BTreeMap<_, _>>();
    graphs
        .iter()
        .map(|graph| {
            graph.finalize_value_only_with_type_facts(
                &evidence_by_node,
                pin,
                issuer,
                revocation_epoch,
            )
        })
        .collect()
}

pub struct FinalizedGraphNode {
    identity: CanonicalDependencyNodeIdentity,
    finalized: FinalizedPolicy2Contract,
}

impl FinalizedGraphNode {
    #[must_use]
    pub const fn identity(&self) -> &CanonicalDependencyNodeIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn finalized(&self) -> &FinalizedPolicy2Contract {
        &self.finalized
    }
}

/// Fully finalized in-memory graph. Publication remains a separate atomic
/// postcondition so a failed root never leaks a partially accepted catalog.
pub struct FinalizedPolicy2Graph {
    graph_root: String,
    root: CanonicalDependencyNodeIdentity,
    nodes: Vec<FinalizedGraphNode>,
}

impl FinalizedPolicy2Graph {
    #[must_use]
    pub fn graph_root(&self) -> &str {
        &self.graph_root
    }

    #[must_use]
    pub fn nodes(&self) -> &[FinalizedGraphNode] {
        &self.nodes
    }

    #[must_use]
    pub fn root(&self) -> &FinalizedPolicy2Contract {
        &self
            .nodes
            .iter()
            .find(|node| node.identity == self.root)
            .expect("finalized graph contains its root")
            .finalized
    }
}

#[derive(Debug, Error)]
pub enum PublishedGraphCertificationError {
    #[error("published graph certification case-set is empty")]
    EmptyCaseSet,
    #[error("canonical published graph identity collision at {0}")]
    CanonicalIdentityCollision(String),
    #[error("dependency node {0} is absent from the opaque graph plan")]
    MissingPlannedDependency(String),
    #[error("dependency node {0} was not finalized before its parent")]
    MissingFinalizedDependency(String),
    #[error("Type Facts certification failed for graph node {node} ({package}): {source}")]
    TypeFactsAtNode {
        node: String,
        package: String,
        #[source]
        source: super::TypeFactsCertificationError,
    },
    #[error("Type Facts certification failed for published graph {graph}: {source}")]
    TypeFactsForGraph {
        graph: String,
        #[source]
        source: super::TypeFactsCertificationError,
    },
    #[error(transparent)]
    Composition(#[from] DependencyReceiptCompositionError),
    #[error("policy-2 finalization failed for graph node {node} ({package}): {source}")]
    FinalizationAtNode {
        node: String,
        package: String,
        #[source]
        source: super::Policy2FinalizationError,
    },
    #[error(transparent)]
    Contract(#[from] crate::contract_interface::ContractFailure),
}

/// Rebuilds a complete finite published-package graph from untrusted registry
/// and lock inputs. Acquisition order is intentionally irrelevant; the result
/// is canonical and dependency-first.
pub fn plan_published_contract_graph(
    root: PublishedGraphNodeRequest,
    dependencies: impl IntoIterator<Item = PublishedGraphNodeRequest>,
) -> Result<PublishedContractGraphPlan, PublishedGraphPlanningError> {
    CertificationPlanningTransaction::new().plan_published_contract_graph(root, dependencies)
}

impl CertificationPlanningTransaction {
    /// Plans one finite graph while reusing only exact verified published
    /// snapshots retained by this transaction.
    pub fn plan_published_contract_graph(
        &mut self,
        root: PublishedGraphNodeRequest,
        dependencies: impl IntoIterator<Item = PublishedGraphNodeRequest>,
    ) -> Result<PublishedContractGraphPlan, PublishedGraphPlanningError> {
        plan_published_contract_graph_with_limits(
            self,
            root,
            dependencies,
            POLICY_2_GRAPH_NODE_LIMIT,
            POLICY_2_GRAPH_DEPTH_LIMIT,
        )
    }
}

fn plan_published_contract_graph_with_limits(
    transaction: &mut CertificationPlanningTransaction,
    root: PublishedGraphNodeRequest,
    dependencies: impl IntoIterator<Item = PublishedGraphNodeRequest>,
    node_limit: usize,
    depth_limit: usize,
) -> Result<PublishedContractGraphPlan, PublishedGraphPlanningError> {
    let mut requests = Vec::from([root]);
    requests.extend(dependencies);
    if requests.len() > node_limit {
        return Err(PublishedGraphPlanningError::NodeLimit {
            actual: requests.len(),
            limit: node_limit,
        });
    }

    let raw_edges = graph_request_edges(&requests)?;
    let mut requests = requests.into_iter().map(Some).collect::<Vec<_>>();
    let mut planned = Vec::with_capacity(requests.len());
    let mut planned_by_request = vec![None; requests.len()];
    let mut visiting = BTreeSet::new();
    for index in 0..requests.len() {
        plan_graph_request_dependency_first(
            transaction,
            index,
            &raw_edges,
            &mut requests,
            &mut planned,
            &mut planned_by_request,
            &mut visiting,
            depth_limit,
            0,
        )?;
    }
    let root_identity = planned[planned_by_request[0].expect("root request was planned")]
        .identity
        .clone();
    let identity_census = planned
        .iter()
        .map(|node| node.identity.clone())
        .collect::<BTreeSet<_>>();
    if identity_census.len() != planned.len() {
        return Err(PublishedGraphPlanningError::DuplicateNode);
    }

    let mut identity_disagreements = Vec::new();
    for parent_index in 0..planned.len() {
        let parent_root = planned[parent_index]
            .plan
            .resolved_import
            .package_root
            .clone();
        let parent_entries = planned[parent_index]
            .plan
            .verified_closure
            .manifest()
            .entries
            .clone();
        let parent_conditions = planned[parent_index].identity.conditions.clone();
        let edges = planned[parent_index]
            .plan
            .verified_closure
            .manifest()
            .dependencies
            .clone();
        let mut resolved = Vec::with_capacity(edges.len());
        for edge in edges {
            let matches = planned
                .iter()
                .filter(|candidate| {
                    candidate.identity.package_name == edge.package_name
                        && candidate.plan.import_request.specifier == edge.specifier
                        && importer_is_closure_entry_module(
                            &candidate.plan.import_request.importer,
                            &parent_root,
                            &parent_entries,
                        )
                        && candidate.identity.conditions == parent_conditions
                })
                .map(|candidate| candidate.identity.clone())
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [identity] => {
                    for (field, supplied, replayed) in [
                        (
                            "artifact case",
                            edge.artifact_case.as_str(),
                            identity.artifact_case.as_str(),
                        ),
                        (
                            "semantic digest",
                            edge.accepted_contract_digest.as_str(),
                            identity.semantic_digest.as_str(),
                        ),
                    ] {
                        if supplied != replayed {
                            identity_disagreements.push((
                                planned[parent_index].identity.digest.clone(),
                                edge.specifier.clone(),
                                field,
                                supplied.to_owned(),
                                replayed.to_owned(),
                            ));
                        }
                    }
                    resolved.push(identity.clone());
                }
                [] => {
                    return Err(PublishedGraphPlanningError::MissingDependency {
                        parent: planned[parent_index].identity.digest.clone(),
                        specifier: edge.specifier,
                    });
                }
                _ => {
                    return Err(PublishedGraphPlanningError::AmbiguousDependency {
                        parent: planned[parent_index].identity.digest.clone(),
                        specifier: edge.specifier,
                    });
                }
            }
        }
        resolved.sort();
        resolved.dedup();
        planned[parent_index].dependencies = resolved;
    }

    let graph = planned
        .iter()
        .map(|node| (node.identity.clone(), node.dependencies.clone()))
        .collect::<BTreeMap<_, _>>();
    let reachable = reachable_nodes(&root_identity, &graph, depth_limit)?;
    if reachable.len() != planned.len() {
        let extras = planned
            .iter()
            .filter(|node| !reachable.contains(&node.identity))
            .map(|node| node.identity.digest.clone())
            .collect();
        return Err(PublishedGraphPlanningError::UnreachableNodes(extras));
    }

    let queue = DependencyCertificationQueue::build(planned.iter().map(|node| {
        DependencyQueueNode::new(
            node.identity.digest.clone(),
            node.identity.artifact_case.clone(),
            node.dependencies
                .iter()
                .map(|dependency| DependencyNodeIdentity {
                    package: dependency.digest.clone(),
                    artifact_case: dependency.artifact_case.clone(),
                })
                .collect(),
        )
    }))?;
    let order = queue
        .order()
        .iter()
        .map(|queued| {
            planned
                .iter()
                .position(|node| {
                    node.identity.digest == queued.package
                        && node.identity.artifact_case == queued.artifact_case
                })
                .expect("queue identities were built from planned nodes")
        })
        .collect::<Vec<_>>();
    if let Some((parent, specifier, field, supplied, replayed)) =
        identity_disagreements.into_iter().next()
    {
        return Err(
            PublishedGraphPlanningError::DependencyIdentityDisagreement {
                parent,
                specifier,
                field,
                supplied,
                replayed,
            },
        );
    }
    let graph_root = graph_root(&root_identity, &graph);
    let mut by_index = planned.into_iter().map(Some).collect::<Vec<_>>();
    let nodes = order
        .into_iter()
        .map(|index| by_index[index].take().expect("queue order is unique"))
        .collect();
    Ok(PublishedContractGraphPlan {
        nodes,
        root: root_identity,
        graph_root,
    })
}

/// True when `importer` lies anywhere inside `package_root`. Node resolves an
/// external import from the importing module, which may be any module of the
/// parent package rather than only its entry, so package-root containment is
/// the sound relation for ordering unplanned graph requests. Comparison is
/// component-wise, so a sibling directory sharing a name prefix does not match.
fn importer_within_package_root(importer: &str, package_root: &str) -> bool {
    Path::new(importer).starts_with(Path::new(package_root))
}

/// True when `importer` is exactly one runtime- or declaration-role module of
/// the parent's replayed, digest-pinned verified closure, reconstructed against
/// its package root. This is the authoritative dependency-edge matcher: it
/// admits a re-export issued from a non-entry module of the parent package
/// while still rejecting any importer that is not a member of the parent's
/// proven closure (for instance one transplanted outside the package root).
fn importer_is_closure_entry_module(
    importer: &str,
    package_root: &str,
    entries: &[crate::artifact_resolution::ClosureEntry],
) -> bool {
    let importer_path = Path::new(importer);
    let root = Path::new(package_root);
    entries.iter().any(|entry| {
        if !matches!(
            entry.role,
            crate::artifact_resolution::ClosureFileRole::Runtime
                | crate::artifact_resolution::ClosureFileRole::Declaration
        ) {
            return false;
        }
        let relative = entry.path.strip_prefix("./").unwrap_or(entry.path.as_str());
        if relative.starts_with("virtual:") {
            return false;
        }
        root.join(relative).as_path() == importer_path
    })
}

fn graph_request_edges(
    requests: &[PublishedGraphNodeRequest],
) -> Result<Vec<Vec<usize>>, PublishedGraphPlanningError> {
    let mut graph = vec![Vec::new(); requests.len()];
    for (parent_index, parent) in requests.iter().enumerate() {
        let parent_root = &parent.certification.resolved_import.package_root;
        let mut parent_conditions = parent
            .certification
            .import_request
            .export_conditions
            .clone();
        parent_conditions.sort();
        parent_conditions.dedup();
        for edge in &parent.certification.resolved_import.closure.dependencies {
            let matches = requests
                .iter()
                .enumerate()
                .filter(|(_, child)| {
                    let mut child_conditions =
                        child.certification.import_request.export_conditions.clone();
                    child_conditions.sort();
                    child_conditions.dedup();
                    child.certification.resolved_import.package_name == edge.package_name
                        && child.certification.import_request.specifier == edge.specifier
                        && importer_within_package_root(
                            &child.certification.import_request.importer,
                            parent_root,
                        )
                        && child_conditions == parent_conditions
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let parent_label = format!(
                "{}@{}:{}",
                parent.certification.resolved_import.package_name,
                parent.certification.resolved_import.package_version,
                parent.certification.resolved_import.requested_entrypoint
            );
            match matches.as_slice() {
                [index] => graph[parent_index].push(*index),
                [] => {
                    return Err(PublishedGraphPlanningError::MissingDependency {
                        parent: parent_label,
                        specifier: edge.specifier.clone(),
                    });
                }
                _ => {
                    return Err(PublishedGraphPlanningError::AmbiguousDependency {
                        parent: parent_label,
                        specifier: edge.specifier.clone(),
                    });
                }
            }
        }
        graph[parent_index].sort_unstable();
        graph[parent_index].dedup();
    }
    Ok(graph)
}

#[allow(clippy::too_many_arguments)]
fn plan_graph_request_dependency_first(
    transaction: &mut CertificationPlanningTransaction,
    index: usize,
    graph: &[Vec<usize>],
    requests: &mut [Option<PublishedGraphNodeRequest>],
    planned: &mut Vec<PlannedGraphNode>,
    planned_by_request: &mut [Option<usize>],
    visiting: &mut BTreeSet<usize>,
    depth_limit: usize,
    depth: usize,
) -> Result<usize, PublishedGraphPlanningError> {
    if let Some(planned_index) = planned_by_request[index] {
        return Ok(planned_index);
    }
    if depth > depth_limit {
        return Err(PublishedGraphPlanningError::DepthLimit { limit: depth_limit });
    }
    if !visiting.insert(index) {
        return Err(PublishedGraphPlanningError::DependencyCycle);
    }
    for dependency in &graph[index] {
        plan_graph_request_dependency_first(
            transaction,
            *dependency,
            graph,
            requests,
            planned,
            planned_by_request,
            visiting,
            depth_limit,
            depth + 1,
        )?;
    }
    visiting.remove(&index);
    // Export identity can traverse more than one accepted re-export edge.
    // Planning remains keyed by direct closure edges, but artifact replay for
    // this node needs every authenticated descendant snapshot that an exact
    // target can terminate in. Keep unrelated graph nodes out of the set.
    let mut dependency_requests = BTreeSet::new();
    collect_graph_descendants(index, graph, &mut dependency_requests);
    let dependency_plans = dependency_requests
        .iter()
        .map(|request_index| {
            &planned[planned_by_request[*request_index]
                .expect("dependency-first recursion planned every descendant")]
            .plan
        })
        .collect::<Vec<_>>();
    let request = requests[index]
        .take()
        .expect("a graph request is consumed only after its dependencies");
    let node = plan_graph_node(transaction, request, &dependency_plans)?;
    let planned_index = planned.len();
    planned.push(node);
    planned_by_request[index] = Some(planned_index);
    Ok(planned_index)
}

fn collect_graph_descendants(index: usize, graph: &[Vec<usize>], output: &mut BTreeSet<usize>) {
    for dependency in &graph[index] {
        if output.insert(*dependency) {
            collect_graph_descendants(*dependency, graph, output);
        }
    }
}

fn plan_graph_node(
    transaction: &mut CertificationPlanningTransaction,
    request: PublishedGraphNodeRequest,
    dependencies: &[&CertificationPlan],
) -> Result<PlannedGraphNode, PublishedGraphPlanningError> {
    let PublishedGraphNodeRequest {
        certification,
        archive,
        lock_selection,
        source_dependencies,
    } = request;
    let registry_origin = archive.registry_origin.clone();
    let plan = super::plan_certification_with_dependencies(
        transaction,
        certification,
        UntrustedArtifactEnvelope::Published(archive),
        dependencies,
    )?;
    for (field, locked, replayed) in [
        (
            "package name",
            lock_selection.package_name.as_str(),
            plan.snapshot.package_name(),
        ),
        (
            "package version",
            lock_selection.package_version.as_str(),
            plan.snapshot.package_version(),
        ),
        (
            "integrity",
            lock_selection.integrity.as_str(),
            plan.snapshot.package_integrity(),
        ),
    ] {
        if locked != replayed {
            return Err(PublishedGraphPlanningError::LockDisagreement {
                field,
                locked: locked.into(),
                replayed: replayed.into(),
            });
        }
    }
    let mut conditions = plan.import_request.export_conditions.clone();
    conditions.sort();
    conditions.dedup();
    let runtime_digest = digest_snapshot_member(&plan, plan.verified_resolution.runtime_path());
    let declarations_digest =
        digest_snapshot_member(&plan, plan.verified_resolution.declarations_path());
    let resolved_import_root = policy2_resolved_import_root(&plan.resolved_import)?;
    let source_dependencies =
        verify_certification_source_packages(transaction, source_dependencies)?;
    let source_dependencies_root = composition_root(
        "source-dependency-snapshots",
        "source-dependency-graph",
        &source_dependencies
            .iter()
            .map(|source| source.identity.as_str())
            .collect::<Vec<_>>(),
    );
    let mut identity = CanonicalDependencyNodeIdentity {
        registry_origin,
        package_manager: lock_selection.package_manager,
        package_name: plan.snapshot.package_name().into(),
        package_version: plan.snapshot.package_version().into(),
        integrity: plan.snapshot.package_integrity().into(),
        lockfile_digest: lock_selection.lockfile_digest,
        lock_locator: lock_selection.locator,
        entrypoint: plan.resolved_import.requested_entrypoint.clone(),
        conditions,
        importer: plan.import_request.importer.clone(),
        resolution_kind: format!("{:?}", plan.resolved_import.authority),
        runtime_target: plan.verified_resolution.runtime_path().into(),
        runtime_digest,
        declarations_target: plan.verified_resolution.declarations_path().into(),
        declarations_digest,
        closure_root: plan.verified_closure.manifest().digest.clone(),
        resolved_import_root,
        snapshot_root: plan.snapshot.root().into(),
        provenance_root: plan.snapshot.provenance_root().into(),
        artifact_case: plan.selected_artifact_case_id().into(),
        semantic_digest: plan
            .demand_graph
            .candidate_semantic_digest()
            .as_str()
            .into(),
        source_dependencies_root,
        digest: String::new(),
    };
    identity.digest = node_identity_digest(&identity);
    Ok(PlannedGraphNode {
        identity,
        plan,
        dependencies: Vec::new(),
        source_dependencies,
    })
}

/// Authenticates a declaration-only source set into canonical, deduplicated
/// order.
///
/// Published-graph nodes and ordinary root certification share this one
/// channel: bytes are accepted only as an integrity-verified published archive
/// whose exact lock selection replays the same name, version, and integrity,
/// installed at an exact `node_modules` coordinate. Nothing here reads an
/// installed tree, and a source that cannot be authenticated is an error rather
/// than a silently trusted package.
#[cfg(test)]
pub(super) fn verify_certification_source_packages_for_test(
    transaction: &mut CertificationPlanningTransaction,
    requests: Vec<PublishedGraphSourceRequest>,
) -> Result<Vec<VerifiedGraphSourcePackage>, PublishedGraphPlanningError> {
    verify_certification_source_packages(transaction, requests)
}

pub(super) fn verify_certification_source_packages(
    transaction: &mut CertificationPlanningTransaction,
    requests: Vec<PublishedGraphSourceRequest>,
) -> Result<Vec<VerifiedGraphSourcePackage>, PublishedGraphPlanningError> {
    let mut sources = requests
        .into_iter()
        .map(|request| plan_graph_source_package(transaction, request))
        .collect::<Result<Vec<_>, _>>()?;
    sources.sort_by(|left, right| left.identity.cmp(&right.identity));
    if sources
        .windows(2)
        .any(|pair| pair[0].identity == pair[1].identity)
    {
        return Err(PublishedGraphPlanningError::DuplicateSourceDependency);
    }
    Ok(sources)
}

/// Authenticates a declaration-only source set for ordinary root
/// certification, withholding whole *package names* rather than single copies.
///
/// A drop here must mean "the witness program cannot resolve this module".
/// Dropping one copy does not mean that. `moduleResolution: "bundler"` walks up
/// `node_modules`, so withholding a nested copy hands the lookup to a hoisted
/// copy of the same name at a *different version* — and the source census
/// accepts those bytes, because they are authentic under their own marker.
/// The determination then comes from authentic-but-wrong-version declarations
/// and can differ from the truth in either direction. That is substitution, not
/// removal, and it is not fail-closed.
///
/// So authentication is all-or-nothing per package name: if any request for a
/// name fails, no copy of that name is materialized, TypeScript reports the
/// module as missing, the reference is `any`, and every demand that needed it
/// stays open exactly as when nothing is supplied. A failing request poisons
/// both the name its lock selection claims and the name its installed root
/// occupies, so a request that disagrees with itself cannot leave either
/// spelling half-materialized.
///
/// Published-graph nodes deliberately do not use this. There a node's canonical
/// identity binds its `source_dependencies_root`, so a source that will not
/// authenticate must refuse the node outright.
pub(super) fn retain_authenticated_source_packages(
    transaction: &mut CertificationPlanningTransaction,
    requests: Vec<PublishedGraphSourceRequest>,
) -> Vec<VerifiedGraphSourcePackage> {
    let mut withheld = BTreeSet::new();
    let mut sources = Vec::with_capacity(requests.len());
    for request in requests {
        let claimed = [
            request.lock_selection.package_name.clone(),
            installed_package_root_name(&request.installed_package_root),
        ];
        match plan_graph_source_package(transaction, request) {
            Ok(source) => sources.push(source),
            Err(_) => withheld.extend(claimed),
        }
    }
    sources.retain(|source| !withheld.contains(source.snapshot.package_name()));
    sources.sort_by(|left, right| left.identity.cmp(&right.identity));
    sources.dedup_by(|left, right| left.identity == right.identity);
    sources
}

/// The package name an installed root occupies, which is the directory name
/// module resolution will find it under. Everything after the last
/// `node_modules/` segment, so a scoped package keeps both of its segments.
fn installed_package_root_name(installed_package_root: &str) -> String {
    installed_package_root
        .replace('\\', "/")
        .rsplit_once("/node_modules/")
        .map_or_else(
            || installed_package_root.to_owned(),
            |(_, name)| name.to_owned(),
        )
}

fn plan_graph_source_package(
    transaction: &mut CertificationPlanningTransaction,
    request: PublishedGraphSourceRequest,
) -> Result<VerifiedGraphSourcePackage, PublishedGraphPlanningError> {
    let PublishedGraphSourceRequest {
        archive,
        lock_selection,
        installed_package_root,
    } = request;
    let registry_origin = archive.registry_origin.clone();
    let snapshot = transaction.published_snapshot(archive)?;
    for (field, locked, replayed) in [
        (
            "source package name",
            lock_selection.package_name.as_str(),
            snapshot.package_name(),
        ),
        (
            "source package version",
            lock_selection.package_version.as_str(),
            snapshot.package_version(),
        ),
        (
            "source package integrity",
            lock_selection.integrity.as_str(),
            snapshot.package_integrity(),
        ),
    ] {
        if locked != replayed {
            return Err(PublishedGraphPlanningError::LockDisagreement {
                field,
                locked: locked.into(),
                replayed: replayed.into(),
            });
        }
    }
    let root = Path::new(&installed_package_root);
    if !root.is_absolute()
        || root
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        || !installed_package_root
            .replace('\\', "/")
            .ends_with(&format!("/node_modules/{}", snapshot.package_name()))
    {
        return Err(PublishedGraphPlanningError::InvalidSourcePackageRoot(
            installed_package_root,
        ));
    }
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:published-source-package:v1\0");
    for value in [
        registry_origin.as_str(),
        lock_selection.package_manager.as_str(),
        lock_selection.lockfile_digest.as_str(),
        lock_selection.locator.as_str(),
        snapshot.package_name(),
        snapshot.package_version(),
        snapshot.package_integrity(),
        installed_package_root.as_str(),
        snapshot.root(),
        snapshot.provenance_root(),
    ] {
        hash_identity_field(&mut hash, value);
    }
    Ok(VerifiedGraphSourcePackage {
        identity: format!("sha256:{:x}", hash.finalize()),
        installed_package_root,
        snapshot,
    })
}

fn digest_snapshot_member(plan: &CertificationPlan, path: &str) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(
            plan.snapshot
                .read(path)
                .expect("verified resolution paths belong to the snapshot")
        )
    )
}

fn node_identity_digest(identity: &CanonicalDependencyNodeIdentity) -> String {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:published-contract-graph-node:v1\0");
    for value in [
        identity.registry_origin.as_str(),
        identity.package_manager.as_str(),
        identity.package_name.as_str(),
        identity.package_version.as_str(),
        identity.integrity.as_str(),
        identity.lockfile_digest.as_str(),
        identity.lock_locator.as_str(),
        identity.entrypoint.as_str(),
        identity.importer.as_str(),
        identity.resolution_kind.as_str(),
        identity.runtime_target.as_str(),
        identity.runtime_digest.as_str(),
        identity.declarations_target.as_str(),
        identity.declarations_digest.as_str(),
        identity.closure_root.as_str(),
        identity.resolved_import_root.as_str(),
        identity.snapshot_root.as_str(),
        identity.provenance_root.as_str(),
        identity.artifact_case.as_str(),
        identity.semantic_digest.as_str(),
        identity.source_dependencies_root.as_str(),
    ] {
        hash_identity_field(&mut hash, value);
    }
    for condition in &identity.conditions {
        hash_identity_field(&mut hash, condition);
    }
    format!("sha256:{:x}", hash.finalize())
}

fn graph_root(
    root: &CanonicalDependencyNodeIdentity,
    graph: &BTreeMap<CanonicalDependencyNodeIdentity, Vec<CanonicalDependencyNodeIdentity>>,
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:published-contract-graph:v1\0");
    hash_identity_field(&mut hash, root.digest());
    for (node, dependencies) in graph {
        hash_identity_field(&mut hash, node.digest());
        for dependency in dependencies {
            hash_identity_field(&mut hash, dependency.digest());
        }
    }
    format!("sha256:{:x}", hash.finalize())
}

fn hash_identity_field(hash: &mut Sha256, value: &str) {
    hash.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hash.update(value.as_bytes());
}

fn reachable_nodes(
    root: &CanonicalDependencyNodeIdentity,
    graph: &BTreeMap<CanonicalDependencyNodeIdentity, Vec<CanonicalDependencyNodeIdentity>>,
    depth_limit: usize,
) -> Result<BTreeSet<CanonicalDependencyNodeIdentity>, PublishedGraphPlanningError> {
    let mut reached = BTreeSet::new();
    let mut stack = vec![(root.clone(), 1_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > depth_limit {
            return Err(PublishedGraphPlanningError::DepthLimit { limit: depth_limit });
        }
        if !reached.insert(node.clone()) {
            continue;
        }
        if let Some(dependencies) = graph.get(&node) {
            stack.extend(
                dependencies
                    .iter()
                    .rev()
                    .cloned()
                    .map(|dependency| (dependency, depth + 1)),
            );
        }
    }
    Ok(reached)
}

#[derive(Debug, Error)]
pub enum PublishedGraphPlanningError {
    #[error(transparent)]
    Certification(#[from] CertificationPlanningError),
    #[error(transparent)]
    ReceiptIdentity(#[from] super::Policy2ReceiptError),
    #[error(transparent)]
    Queue(#[from] DependencyCompositionError),
    #[error(transparent)]
    Snapshot(#[from] super::ArtifactSnapshotError),
    #[error("published dependency graph has {actual} nodes; policy limit is {limit}")]
    NodeLimit { actual: usize, limit: usize },
    #[error("published dependency graph exceeds policy depth limit {limit}")]
    DepthLimit { limit: usize },
    #[error("published dependency graph repeats a complete canonical node identity")]
    DuplicateNode,
    #[error("published graph node repeats an exact declaration-only source dependency")]
    DuplicateSourceDependency,
    #[error(
        "declaration-only installed package root is not an exact node_modules coordinate: {0:?}"
    )]
    InvalidSourcePackageRoot(String),
    #[error("lock {field} {locked:?} disagrees with authenticated archive value {replayed:?}")]
    LockDisagreement {
        field: &'static str,
        locked: String,
        replayed: String,
    },
    #[error("graph node {parent} has no exact dependency node for {specifier:?}")]
    MissingDependency { parent: String, specifier: String },
    #[error("graph node {parent} has multiple exact dependency nodes for {specifier:?}")]
    AmbiguousDependency { parent: String, specifier: String },
    #[error(
        "graph node {parent} dependency {specifier:?} supplied {field} {supplied:?}; replayed child requires {replayed:?}"
    )]
    DependencyIdentityDisagreement {
        parent: String,
        specifier: String,
        field: &'static str,
        supplied: String,
        replayed: String,
    },
    #[error("published dependency graph contains unreachable nodes {0:?}")]
    UnreachableNodes(Vec<String>),
    #[error("published dependency graph contains a cycle before opaque planning")]
    DependencyCycle,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DependencyNodeIdentity {
    pub package: String,
    pub artifact_case: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyQueueNode {
    identity: DependencyNodeIdentity,
    dependencies: Vec<DependencyNodeIdentity>,
}

impl DependencyQueueNode {
    #[must_use]
    pub fn new(
        package: impl Into<String>,
        artifact_case: impl Into<String>,
        dependencies: Vec<DependencyNodeIdentity>,
    ) -> Self {
        Self {
            identity: DependencyNodeIdentity {
                package: package.into(),
                artifact_case: artifact_case.into(),
            },
            dependencies,
        }
    }
}

/// Canonical dependency-first order for a finite certification batch.
#[derive(Debug)]
pub struct DependencyCertificationQueue {
    order: Vec<DependencyNodeIdentity>,
}

impl DependencyCertificationQueue {
    pub fn build(
        nodes: impl IntoIterator<Item = DependencyQueueNode>,
    ) -> Result<Self, DependencyCompositionError> {
        let mut graph = BTreeMap::<DependencyNodeIdentity, Vec<DependencyNodeIdentity>>::new();
        for mut node in nodes {
            validate_node(&node.identity)?;
            node.dependencies.sort();
            node.dependencies.dedup();
            for dependency in &node.dependencies {
                validate_node(dependency)?;
            }
            if graph
                .insert(node.identity.clone(), node.dependencies)
                .is_some()
            {
                return Err(DependencyCompositionError::DuplicateNode(node.identity));
            }
        }
        let mut states = BTreeMap::<DependencyNodeIdentity, VisitState>::new();
        let mut stack = Vec::new();
        let mut order = Vec::with_capacity(graph.len());
        for node in graph.keys() {
            visit(node, &graph, &mut states, &mut stack, &mut order)?;
        }
        Ok(Self { order })
    }

    #[must_use]
    pub fn order(&self) -> &[DependencyNodeIdentity] {
        &self.order
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Visiting,
    Complete,
}

fn visit(
    node: &DependencyNodeIdentity,
    graph: &BTreeMap<DependencyNodeIdentity, Vec<DependencyNodeIdentity>>,
    states: &mut BTreeMap<DependencyNodeIdentity, VisitState>,
    stack: &mut Vec<DependencyNodeIdentity>,
    order: &mut Vec<DependencyNodeIdentity>,
) -> Result<(), DependencyCompositionError> {
    match states.get(node) {
        Some(VisitState::Complete) => return Ok(()),
        Some(VisitState::Visiting) => {
            let start = stack.iter().position(|entry| entry == node).unwrap_or(0);
            let mut cycle = stack[start..].to_vec();
            cycle.push(node.clone());
            return Err(DependencyCompositionError::Cycle(canonical_cycle(cycle)));
        }
        None => {}
    }
    states.insert(node.clone(), VisitState::Visiting);
    stack.push(node.clone());
    if let Some(dependencies) = graph.get(node) {
        for dependency in dependencies {
            // An edge outside this batch is a leaf awaiting an authenticated
            // receipt, not a graph node whose unseen dependencies may be
            // guessed. It is handled by the composition schedule below.
            if graph.contains_key(dependency) {
                visit(dependency, graph, states, stack, order)?;
            }
        }
    }
    stack.pop();
    states.insert(node.clone(), VisitState::Complete);
    order.push(node.clone());
    Ok(())
}

fn canonical_cycle(mut cycle: Vec<DependencyNodeIdentity>) -> Vec<DependencyNodeIdentity> {
    cycle.pop();
    if cycle.is_empty() {
        return cycle;
    }
    let start = cycle
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| left.cmp(right))
        .map_or(0, |(index, _)| index);
    cycle.rotate_left(start);
    cycle.push(cycle[0].clone());
    cycle
}

fn validate_node(node: &DependencyNodeIdentity) -> Result<(), DependencyCompositionError> {
    if node.package.trim().is_empty() || node.artifact_case.trim().is_empty() {
        return Err(DependencyCompositionError::InvalidNode(node.clone()));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DependencyCompositionRequirement {
    demand_id: String,
    dependency: DependencyDemandInput,
    parent_export: Option<String>,
    semantic_claim_id: Option<String>,
}

impl DependencyCompositionRequirement {
    #[must_use]
    pub fn demand_id(&self) -> &str {
        &self.demand_id
    }

    #[must_use]
    pub const fn dependency(&self) -> &DependencyDemandInput {
        &self.dependency
    }

    #[must_use]
    pub fn parent_export(&self) -> Option<&str> {
        self.parent_export.as_deref()
    }

    #[must_use]
    pub fn semantic_claim_id(&self) -> Option<&str> {
        self.semantic_claim_id.as_deref()
    }

    #[must_use]
    pub const fn authenticates_dependency_artifact(&self) -> bool {
        self.parent_export.is_none()
    }
}

pub struct DependencyCompositionSchedule {
    requirements: Vec<DependencyCompositionRequirement>,
}

impl DependencyCompositionSchedule {
    pub(crate) fn from_plan(plan: &CertificationPlan) -> Result<Self, DependencyCompositionError> {
        let mut requirements = plan
            .demand_graph
            .demands()
            .iter()
            .filter(|demand| demand.family() == ProofFamily::AcceptedDependencyComposition)
            .map(|demand| match demand.subject() {
                ProofDemandSubject::DependencyArtifact { dependency } => {
                    Ok(DependencyCompositionRequirement {
                        demand_id: demand.id().as_str().into(),
                        dependency: dependency.clone(),
                        parent_export: None,
                        semantic_claim_id: None,
                    })
                }
                ProofDemandSubject::DependencyClosure {
                    dependency,
                    parent,
                    semantic_claim_id,
                } => Ok(DependencyCompositionRequirement {
                    demand_id: demand.id().as_str().into(),
                    dependency: dependency.clone(),
                    parent_export: Some(parent.export.clone()),
                    semantic_claim_id: Some(semantic_claim_id.clone()),
                }),
                _ => Err(DependencyCompositionError::InvalidDemand),
            })
            .collect::<Result<Vec<_>, _>>()?;
        requirements.sort_by(|left, right| {
            (
                left.dependency.package.as_str(),
                left.dependency.artifact_case.as_str(),
                left.parent_export.as_deref(),
                left.semantic_claim_id.as_deref(),
            )
                .cmp(&(
                    right.dependency.package.as_str(),
                    right.dependency.artifact_case.as_str(),
                    right.parent_export.as_deref(),
                    right.semantic_claim_id.as_deref(),
                ))
        });
        Ok(Self { requirements })
    }

    #[must_use]
    pub fn requirements(&self) -> &[DependencyCompositionRequirement] {
        &self.requirements
    }

    /// Returns the exact canonical first dependency demand absent from a
    /// caller's structural set. This is an ordering helper only: an empty
    /// result is not receipt authority and cannot construct a witness. Slice 8
    /// will feed it IDs obtained from opaque authenticated receipt tokens.
    #[must_use]
    pub fn first_unaccepted<'a>(
        &'a self,
        authenticated_demand_ids: &BTreeSet<String>,
    ) -> Option<&'a DependencyCompositionRequirement> {
        self.requirements
            .iter()
            .find(|requirement| !authenticated_demand_ids.contains(&requirement.demand_id))
    }
}

/// Opaque family evidence produced only after authenticating exact child
/// receipts against one parent plan and the final graph root.
pub struct VerifiedDependencyComposition {
    demand_graph_root: String,
    graph_root: String,
    witnesses: Vec<solid_reactive_ir::contract_semantics::certification::WitnessBinding>,
    receipts_root: String,
    trust_root: String,
    verifier_build_digest: Option<String>,
    semantic_dependency_count: usize,
}

impl VerifiedDependencyComposition {
    fn authenticate(
        parent: &CertificationPlan,
        expected_dependencies: &[CanonicalDependencyNodeIdentity],
        source_dependencies: &[VerifiedGraphSourcePackage],
        graph_root: &str,
        receipts: &[(
            &CanonicalDependencyNodeIdentity,
            &AuthenticatedPolicy2Receipt,
        )],
        issuer: &ConfiguredReceiptIssuer,
        revocation_epoch: u64,
    ) -> Result<Self, DependencyReceiptCompositionError> {
        if expected_dependencies.len() != receipts.len() {
            return Err(DependencyReceiptCompositionError::ReceiptCensus {
                expected: expected_dependencies.len(),
                actual: receipts.len(),
            });
        }
        let receipt_map = receipts
            .iter()
            .map(|(identity, receipt)| (identity.digest(), *receipt))
            .collect::<BTreeMap<_, _>>();
        if receipt_map.len() != receipts.len() {
            return Err(DependencyReceiptCompositionError::DuplicateReceipt);
        }
        let schedule = parent.dependency_composition_schedule()?;
        let mut receipt_rows = Vec::new();
        let mut trust_rows = Vec::new();
        let mut verifier_build_digest = None::<String>;
        let mut witnesses = Vec::with_capacity(schedule.requirements().len());
        for requirement in schedule.requirements() {
            let dependency = expected_dependencies
                .iter()
                .find(|identity| {
                    identity.package_name == requirement.dependency().package
                        && identity.artifact_case == requirement.dependency().artifact_case
                        && identity.semantic_digest
                            == requirement.dependency().accepted_contract_digest
                })
                .ok_or_else(|| DependencyReceiptCompositionError::MissingGraphEdge {
                    demand_id: requirement.demand_id().into(),
                })?;
            let receipt = receipt_map.get(dependency.digest()).ok_or_else(|| {
                DependencyReceiptCompositionError::MissingReceipt {
                    dependency: dependency.digest().into(),
                }
            })?;
            authenticate_dependency_receipt(
                parent,
                requirement,
                dependency,
                receipt,
                issuer,
                revocation_epoch,
            )?;
            match &verifier_build_digest {
                Some(expected) if expected != receipt.verifier_build_digest().as_str() => {
                    return Err(DependencyReceiptCompositionError::VerifierBuildDisagreement);
                }
                None => {
                    verifier_build_digest = Some(receipt.verifier_build_digest().as_str().into());
                }
                _ => {}
            }
            let evidence_root = dependency_composition_evidence_root(
                graph_root,
                parent,
                requirement,
                dependency,
                receipt,
            );
            witnesses.push(
                solid_reactive_ir::contract_semantics::certification::WitnessBinding::new(
                    solid_reactive_ir::contract_semantics::certification::ProofWitnessVariant::AcceptedDependencyComposition,
                    requirement.demand_id(),
                    evidence_root,
                    vec![
                        format!("graph:{graph_root}"),
                        format!("parent-case:{}", parent.selected_artifact_case_id()),
                        format!("dependency-node:{}", dependency.digest()),
                        format!("dependency-receipt:{}", receipt.receipt_digest()),
                    ],
                ),
            );
            receipt_rows.push(format!(
                "{}:{}:{}:{}",
                requirement.demand_id(),
                dependency.digest(),
                receipt.receipt_digest(),
                receipt.main_digest()
            ));
            trust_rows.push(format!(
                "{}:{:?}:{}:{}:{}",
                receipt.trust_store_digest(),
                receipt.issuer_kind(),
                receipt.issuer_scope(),
                receipt.revocation_epoch(),
                receipt.verifier_build_digest().as_str()
            ));
        }
        for source in source_dependencies {
            trust_rows.push(format!(
                "source:{}:{}:{}:{}",
                source.identity,
                source.snapshot.package_integrity(),
                source.snapshot.root(),
                source.snapshot.provenance_root(),
            ));
        }
        receipt_rows.sort();
        receipt_rows.dedup();
        trust_rows.sort();
        trust_rows.dedup();
        Ok(Self {
            demand_graph_root: parent.demand_graph().root().as_str().into(),
            graph_root: graph_root.into(),
            witnesses,
            receipts_root: composition_root("dependency-receipts", graph_root, &receipt_rows),
            trust_root: composition_root("dependency-trust", graph_root, &trust_rows),
            verifier_build_digest,
            semantic_dependency_count: expected_dependencies.len(),
        })
    }

    pub(super) fn verify_plan(
        &self,
        plan: &CertificationPlan,
    ) -> Result<(), DependencyReceiptCompositionError> {
        if self.demand_graph_root != plan.demand_graph().root().as_str() {
            return Err(DependencyReceiptCompositionError::ParentTransplant);
        }
        Ok(())
    }

    pub(super) fn witnesses(
        &self,
    ) -> &[solid_reactive_ir::contract_semantics::certification::WitnessBinding] {
        &self.witnesses
    }

    pub(super) fn receipts_root(&self) -> &str {
        &self.receipts_root
    }

    pub(super) fn trust_root(&self) -> &str {
        &self.trust_root
    }

    pub(super) fn verifier_build_digest(&self) -> Option<&str> {
        self.verifier_build_digest.as_deref()
    }

    pub(super) fn has_semantic_dependencies(&self) -> bool {
        self.semantic_dependency_count != 0
    }

    #[must_use]
    pub fn graph_root(&self) -> &str {
        &self.graph_root
    }
}

fn authenticate_dependency_receipt(
    parent: &CertificationPlan,
    requirement: &DependencyCompositionRequirement,
    dependency: &CanonicalDependencyNodeIdentity,
    receipt: &AuthenticatedPolicy2Receipt,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
) -> Result<(), DependencyReceiptCompositionError> {
    let bindings = receipt.bindings();
    let checks = [
        (
            "semantic digest",
            receipt.semantic_digest().as_str(),
            requirement.dependency().accepted_contract_digest.as_str(),
        ),
        (
            "binding semantic digest",
            bindings.semantic_digest.as_str(),
            dependency.semantic_digest.as_str(),
        ),
        (
            "importer",
            bindings.importer.as_str(),
            dependency.importer.as_str(),
        ),
        (
            "specifier",
            bindings.specifier.as_str(),
            requirement.dependency().specifier.as_str(),
        ),
        (
            "resolved import root",
            bindings.resolved_import_root.as_str(),
            dependency.resolved_import_root.as_str(),
        ),
        (
            "artifact provenance root",
            bindings.artifact_provenance_root.as_str(),
            dependency.provenance_root.as_str(),
        ),
        (
            "snapshot root",
            bindings.snapshot_root.as_str(),
            dependency.snapshot_root.as_str(),
        ),
        (
            "policy digest",
            receipt.policy_digest().as_str(),
            parent.demand_graph().policy_digest().as_str(),
        ),
    ];
    for (field, actual, expected) in checks {
        if actual != expected {
            return Err(DependencyReceiptCompositionError::ReceiptMismatch {
                field,
                actual: actual.into(),
                expected: expected.into(),
            });
        }
    }
    if receipt.issuer_kind() != issuer.kind()
        || receipt.issuer_scope() != issuer.scope()
        || receipt.revocation_epoch() != revocation_epoch
    {
        return Err(DependencyReceiptCompositionError::TrustMismatch);
    }
    Ok(())
}

fn dependency_composition_evidence_root(
    graph_root: &str,
    parent: &CertificationPlan,
    requirement: &DependencyCompositionRequirement,
    dependency: &CanonicalDependencyNodeIdentity,
    receipt: &AuthenticatedPolicy2Receipt,
) -> String {
    let fields = [
        graph_root,
        parent.demand_graph().root().as_str(),
        parent.selected_artifact_case_id(),
        requirement.demand_id(),
        requirement.parent_export().unwrap_or("<artifact>"),
        requirement.semantic_claim_id().unwrap_or("<artifact>"),
        requirement.dependency().specifier.as_str(),
        dependency.digest(),
        dependency.resolved_import_root.as_str(),
        receipt.receipt_digest(),
        receipt.main_digest(),
        receipt.semantic_digest().as_str(),
        receipt.policy_digest().as_str(),
        receipt.verifier_build_digest().as_str(),
        receipt.trust_store_digest(),
    ];
    composition_root("dependency-composition-evidence", graph_root, &fields)
}

fn composition_root(domain: &str, graph_root: &str, values: &[impl AsRef<str>]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:dependency-receipt-composition:v1\0");
    hash_identity_field(&mut hash, domain);
    hash_identity_field(&mut hash, graph_root);
    for value in values {
        hash_identity_field(&mut hash, value.as_ref());
    }
    format!("sha256:{:x}", hash.finalize())
}

#[derive(Debug, Error)]
pub enum DependencyReceiptCompositionError {
    #[error(transparent)]
    Schedule(#[from] DependencyCompositionError),
    #[error("dependency receipt parent is outside the planned graph")]
    ParentOutsideGraph,
    #[error("dependency receipt census has {actual} rows; expected {expected}")]
    ReceiptCensus { expected: usize, actual: usize },
    #[error("dependency receipt census repeats a canonical node")]
    DuplicateReceipt,
    #[error("dependency demand {demand_id} has no matching canonical graph edge")]
    MissingGraphEdge { demand_id: String },
    #[error("canonical dependency node {dependency} has no authenticated receipt")]
    MissingReceipt { dependency: String },
    #[error("dependency receipt {field} {actual:?} does not match {expected:?}")]
    ReceiptMismatch {
        field: &'static str,
        actual: String,
        expected: String,
    },
    #[error("dependency receipt trust identity does not match the graph transaction")]
    TrustMismatch,
    #[error("dependency receipts disagree on verifier build identity")]
    VerifierBuildDisagreement,
    #[error("dependency composition evidence was transplanted to another parent plan")]
    ParentTransplant,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DependencyCompositionError {
    #[error("duplicate dependency certification node {0:?}")]
    DuplicateNode(DependencyNodeIdentity),
    #[error("invalid dependency certification node {0:?}")]
    InvalidNode(DependencyNodeIdentity),
    #[error("dependency certification cycle {0:?}")]
    Cycle(Vec<DependencyNodeIdentity>),
    #[error("dependency-composition demand has the wrong subject")]
    InvalidDemand,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(package: &str) -> DependencyNodeIdentity {
        DependencyNodeIdentity {
            package: package.into(),
            artifact_case: format!("artifact-case:{package}"),
        }
    }

    #[test]
    fn queue_is_dependency_first_and_independent_of_input_order() {
        let nodes = vec![
            DependencyQueueNode::new("root", "artifact-case:root", vec![id("b"), id("a")]),
            DependencyQueueNode::new("b", "artifact-case:b", vec![id("a")]),
            DependencyQueueNode::new("a", "artifact-case:a", vec![]),
        ];
        let forward = DependencyCertificationQueue::build(nodes.clone()).unwrap();
        let reverse = DependencyCertificationQueue::build(nodes.into_iter().rev()).unwrap();
        assert_eq!(forward.order(), reverse.order());
        assert_eq!(forward.order(), &[id("a"), id("b"), id("root")]);
    }

    #[test]
    fn cycle_reporting_rotates_to_the_canonical_first_node() {
        let error = DependencyCertificationQueue::build([
            DependencyQueueNode::new("z", "artifact-case:z", vec![id("b")]),
            DependencyQueueNode::new("b", "artifact-case:b", vec![id("a")]),
            DependencyQueueNode::new("a", "artifact-case:a", vec![id("z")]),
        ])
        .unwrap_err();
        assert_eq!(
            error,
            DependencyCompositionError::Cycle(vec![id("a"), id("z"), id("b"), id("a")])
        );
    }

    #[test]
    fn bun_lock_selection_is_derived_from_exact_bytes_and_rejects_absence() {
        let lock = br#"{
          "packages": {
            "leaf-package@2.0.0": ["leaf-package@2.0.0", "", {}, "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="],
          },
        }"#;
        let selection = PublishedGraphLockSelection::from_bun_lock(
            lock,
            "leaf-package@2.0.0",
            "leaf-package",
            "2.0.0",
        )
        .unwrap();
        assert_eq!(
            selection.integrity,
            "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="
        );
        assert_eq!(
            selection.lockfile_digest,
            format!("sha256:{:x}", Sha256::digest(lock))
        );
        assert!(
            PublishedGraphLockSelection::from_bun_lock(lock, "missing@1.0.0", "missing", "1.0.0")
                .is_err()
        );
        assert!(
            PublishedGraphLockSelection::from_bun_lock(
                lock,
                "transplanted@2.0.0",
                "leaf-package",
                "2.0.0"
            )
            .is_err()
        );
    }

    #[test]
    fn bun_lock_selection_uses_the_installed_locator_to_disambiguate_same_versions() {
        let lock = br#"{
          "packages": {
            "@corvu/utils": ["@corvu/utils@0.3.2", "", {}, "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="],
            "@corvu/accordion/@corvu/utils": ["@corvu/utils@0.3.2", "", {}, "sha512-AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ=="],
          },
        }"#;
        let selection = PublishedGraphLockSelection::from_bun_lock(
            lock,
            "@corvu/accordion/@corvu/utils",
            "@corvu/utils",
            "0.3.2",
        )
        .unwrap();
        assert_eq!(
            selection.integrity,
            "sha512-AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ=="
        );
    }
}
