//! Policy-2 dependency composition planning.
//!
//! This module owns canonical bottom-up ordering, cycle refusal, and exact
//! policy-2 receipt composition. A caller may transport opaque receipts, but
//! cannot turn a policy-1 receipt or a caller-provided digest into dependency
//! authority.

use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::NormalizedContract;
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

// 1024, from 256: a node is one (artifact, importing module) pair, so a package
// with many entrypoints multiplies its dependencies by its importers and an
// umbrella package (`corvu`) crossed 256 without adding an artifact. Importer
// variants share generation and exported-value acquisition, so the cost this
// bounds grows with distinct artifacts, not with the node count. The CLI's
// discovery bound (`certify-contract.mjs`, `published-contract-graph.mjs`) is
// the same number.
const POLICY_2_GRAPH_NODE_LIMIT: usize = 1024;
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

    /// Every field of the identity except the importing module and the two
    /// values derived from it (`resolved_import_root`, which hashes the
    /// resolved import that names the importer, and the digest), for
    /// [`select_importer_variant`]: two identities with equal keys are the same
    /// archive, lock selection, resolution result, closure, proposal, and
    /// source set, reached from different modules of the same consuming
    /// package.
    fn importer_invariant_key(&self) -> Vec<&str> {
        let mut key = vec![
            self.registry_origin.as_str(),
            self.package_manager.as_str(),
            self.package_name.as_str(),
            self.package_version.as_str(),
            self.integrity.as_str(),
            self.lockfile_digest.as_str(),
            self.lock_locator.as_str(),
            self.entrypoint.as_str(),
            self.resolution_kind.as_str(),
            self.runtime_target.as_str(),
            self.runtime_digest.as_str(),
            self.declarations_target.as_str(),
            self.declarations_digest.as_str(),
            self.closure_root.as_str(),
            self.snapshot_root.as_str(),
            self.provenance_root.as_str(),
            self.artifact_case.as_str(),
            self.semantic_digest.as_str(),
            self.source_dependencies_root.as_str(),
        ];
        key.extend(self.conditions.iter().map(String::as_str));
        key
    }
}

#[derive(Clone)]
struct PlannedGraphNode {
    identity: CanonicalDependencyNodeIdentity,
    plan: CertificationPlan,
    dependencies: Vec<CanonicalDependencyNodeIdentity>,
    source_dependencies: Vec<VerifiedGraphSourcePackage>,
    /// What recipe-gated planning withheld from this node's plan; empty until
    /// [`PublishedContractGraphPlan::recipe_gated`] derives the gated graph.
    withheld: Vec<super::WithheldClosure>,
    /// The proposal this node was **planned** with — the one its identity's
    /// `semantic_digest` names and every parent's closure edge accepted.
    /// Recipe gating replaces `plan` with a plan over a weakened proposal; this
    /// stays, so composition can re-derive that weakening from the accepted
    /// proposal and the withheld records and compare it against what the
    /// dependency's receipt actually certified (see
    /// [`authenticate_dependency_receipt`]).
    accepted_candidate: NormalizedContract,
}

/// What recipe gating did to one dependency node, as composition needs it:
/// the proposal every parent accepted, the proposal the node's receipt
/// certifies, and the records that separate them.
struct DependencyGating<'a> {
    accepted_candidate: &'a NormalizedContract,
    certified_candidate: &'a NormalizedContract,
    withheld: &'a [super::WithheldClosure],
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
        self.authenticate_dependency_receipts_with_census(
            parent,
            receipts,
            issuer,
            revocation_epoch,
            None,
        )
    }

    fn authenticate_dependency_receipts_with_census(
        &self,
        parent: &CanonicalDependencyNodeIdentity,
        receipts: &[(
            &CanonicalDependencyNodeIdentity,
            &AuthenticatedPolicy2Receipt,
        )],
        issuer: &ConfiguredReceiptIssuer,
        revocation_epoch: u64,
        type_facts: Option<&super::type_facts::VerifiedTypeFactsEvidence>,
    ) -> Result<VerifiedDependencyComposition, DependencyReceiptCompositionError> {
        let node = self
            .nodes
            .iter()
            .find(|node| &node.identity == parent)
            .ok_or(DependencyReceiptCompositionError::ParentOutsideGraph)?;
        let gating = node
            .dependencies
            .iter()
            .map(|dependency| {
                let planned = self
                    .nodes
                    .iter()
                    .find(|candidate| &candidate.identity == dependency)
                    .ok_or_else(
                        || DependencyReceiptCompositionError::DependencyOutsideGraph {
                            dependency: dependency.digest().into(),
                        },
                    )?;
                Ok((
                    dependency.digest().to_owned(),
                    DependencyGating {
                        accepted_candidate: &planned.accepted_candidate,
                        certified_candidate: &planned.plan.selected_candidate,
                        withheld: &planned.withheld,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, DependencyReceiptCompositionError>>()?;
        VerifiedDependencyComposition::authenticate(
            &node.plan,
            &node.dependencies,
            &node.source_dependencies,
            &gating,
            self.graph_root(),
            receipts,
            issuer,
            revocation_epoch,
            type_facts,
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
        probes: Option<&super::ProbeHarnessConfiguration>,
    ) -> Result<FinalizedPolicy2Graph, PublishedGraphCertificationError> {
        let mut finalized = certify_graphs_with_recipe_gating(
            std::slice::from_ref(self),
            pin,
            issuer,
            revocation_epoch,
            probes,
        )?;
        finalized
            .pop()
            .ok_or(PublishedGraphCertificationError::EmptyCaseSet)
    }

    fn node(&self, digest: &str) -> Option<&PlannedGraphNode> {
        self.nodes
            .iter()
            .find(|node| node.identity.digest() == digest)
    }

    /// The graph with every node's plan recipe-gated
    /// (`CertificationPlan::recipe_gated`), node identities and graph root
    /// unchanged.
    ///
    /// Node identity binds the snapshot, the resolution and the proposal's
    /// semantic digest as *planned*; the graph root is derived from those
    /// identities alone. Gating re-derives a node's demand graph from a
    /// weakened proposal, which the receipt then binds through its own
    /// `semantic_digest` and `demand_graph_root` fields — so the graph root a
    /// case set is keyed by is the same before and after, and the receipt is
    /// the one telling the truth about what was certified.
    ///
    /// Identities are kept rather than rebound to the gated digest on purpose.
    /// A parent's closure edge names the dependency proposal it was generated
    /// against (`accepted_contract_digest`), that digest is hashed into every
    /// dependency demand of the parent's demand graph, and the edge lives in
    /// the parent's authenticated closure manifest — none of which a gate on
    /// the *dependency* may rewrite. So the accepted digest stays the identity,
    /// and composition instead proves that what the dependency's receipt
    /// certifies is exactly the accepted proposal with the withheld domains
    /// opened ([`authenticate_dependency_receipt`]).
    /// One gating pass with a single corpus and nothing already withdrawn:
    /// what the certification loop's first pass does, kept for the tests that
    /// pin gating on its own.
    #[cfg(test)]
    pub(super) fn recipe_gated(
        &self,
        recipe_corpus: Option<&Path>,
    ) -> Result<Self, PublishedGraphCertificationError> {
        self.recipe_gated_per_node(&BTreeMap::new(), recipe_corpus, &BTreeMap::new())
    }

    /// [`Self::recipe_gated`] with ADR 0036's per-node inputs: the corpus and
    /// the already-withdrawn candidates are chosen *per node*, by canonical
    /// identity digest, so one node's synthesized corpus and one node's census
    /// or veto withdrawals never reach another node's gate.
    pub(super) fn recipe_gated_per_node(
        &self,
        synthesized: &BTreeMap<String, super::synthesized_vetoes::SynthesizedCorpus>,
        base_corpus: Option<&Path>,
        already_withheld: &BTreeMap<String, Vec<super::WithheldClosure>>,
    ) -> Result<Self, PublishedGraphCertificationError> {
        let nodes = self
            .nodes
            .iter()
            .map(|node| {
                let digest = node.identity.digest();
                let corpus = synthesized
                    .get(digest)
                    .map(|corpus| corpus.configuration().recipe_corpus())
                    .or(base_corpus);
                let (plan, withheld) = node
                    .plan
                    .recipe_gated_with(
                        corpus,
                        already_withheld.get(digest).map_or(&[], Vec::as_slice),
                    )
                    .map_err(
                        |source| PublishedGraphCertificationError::RecipeGatingAtNode {
                            node: node.identity.digest().into(),
                            package: format!(
                                "{}@{}",
                                node.identity.package_name, node.identity.package_version
                            ),
                            source: Box::new(source),
                        },
                    )?
                    .into_parts();
                Ok(PlannedGraphNode {
                    identity: node.identity.clone(),
                    plan,
                    dependencies: node.dependencies.clone(),
                    source_dependencies: node.source_dependencies.clone(),
                    withheld,
                    accepted_candidate: node.accepted_candidate.clone(),
                })
            })
            .collect::<Result<Vec<_>, PublishedGraphCertificationError>>()?;
        Ok(Self {
            nodes,
            root: self.root.clone(),
            graph_root: self.graph_root.clone(),
        })
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
                        acquire: true,
                    },
                ))
            })
            .collect()
    }

    /// Finalizes every node bottom-up against the pass's evidence and the
    /// gates the pre-pass authenticated. `Ok(Err(withdrawals))` is a pass that
    /// could not finalize as gated: a parent's dependency-closure demand
    /// required a child claim the child withheld, and the parent's own
    /// candidate is withdrawn by name (ADR 0036, graph lanes); every node
    /// above a withdrawn one is skipped this pass, so one pass collects every
    /// such withdrawal the graph can reach.
    fn finalize_value_only_with_type_facts(
        &self,
        type_facts_by_node: &BTreeMap<String, super::type_facts::VerifiedTypeFactsEvidence>,
        gates_by_node: &BTreeMap<String, (String, super::probe_gates::VerifiedProbeGateBatch)>,
        pin: &TypeFactsProducerPin,
        issuer: &ConfiguredReceiptIssuer,
        revocation_epoch: u64,
    ) -> Result<
        Result<FinalizedPolicy2Graph, Vec<(String, super::WithheldClosure)>>,
        PublishedGraphCertificationError,
    > {
        let mut finalized = Vec::<FinalizedGraphNode>::with_capacity(self.nodes.len());
        let mut withdrawals = Vec::<(String, super::WithheldClosure)>::new();
        let mut skipped = BTreeSet::<String>::new();
        for node in &self.nodes {
            let digest = node.identity.digest();
            let package = format!(
                "{}@{}",
                node.identity.package_name, node.identity.package_version
            );
            if node
                .dependencies
                .iter()
                .any(|dependency| skipped.contains(dependency.digest()))
            {
                skipped.insert(digest.to_owned());
                continue;
            }
            let proposal = crate::contract_document::encode(
                &node.plan.selected_candidate,
                &crate::contract_document::SidecarDigests::default(),
                false,
            )?;
            let type_facts = type_facts_by_node.get(digest);
            let dependency_evidence = if node.dependencies.is_empty()
                && node.source_dependencies.is_empty()
            {
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
                match self.authenticate_dependency_receipts_with_census(
                    &node.identity,
                    &receipts,
                    issuer,
                    revocation_epoch,
                    type_facts,
                ) {
                    Ok(evidence) => Some(evidence),
                    Err(DependencyReceiptCompositionError::MissingClosedClaim {
                        demand_id,
                        semantic_claim_id,
                    }) => {
                        let Some(record) =
                            composed_from_withheld_dependency(node, &demand_id, &semantic_claim_id)
                        else {
                            return Err(DependencyReceiptCompositionError::MissingClosedClaim {
                                demand_id,
                                semantic_claim_id,
                            }
                            .into());
                        };
                        withdrawals.push((digest.to_owned(), record));
                        skipped.insert(digest.to_owned());
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                }
            };
            let (_, probe_gates) = gates_by_node.get(digest).ok_or_else(|| {
                PublishedGraphCertificationError::MissingProbeGates(digest.to_owned())
            })?;
            let contract = super::finalization::finalize_value_only_with_dependencies(
                &node.plan,
                &proposal,
                type_facts,
                dependency_evidence.as_ref(),
                probe_gates,
                pin,
                issuer,
                revocation_epoch,
            )
            .map_err(|source| {
                PublishedGraphCertificationError::FinalizationAtNode {
                    node: digest.to_owned(),
                    package: package.clone(),
                    source,
                }
            })?;
            finalized.push(FinalizedGraphNode {
                identity: node.identity.clone(),
                finalized: contract.with_withheld_closures(node.withheld.clone()),
            });
        }
        if !withdrawals.is_empty() {
            return Ok(Err(withdrawals));
        }
        Ok(Ok(FinalizedPolicy2Graph {
            graph_root: self.graph_root.clone(),
            root: self.root.clone(),
            nodes: finalized,
        }))
    }
}

/// The parent candidate a `MissingClosedClaim` composition refusal is about,
/// withheld by name: the parent's dependency-closure demand `demand_id` asked
/// for `child_claim` closed in the dependency's certified contract, and the
/// dependency withheld it, so the parent's own closure -- composed from that
/// claim -- cannot be certified either. Withholding the parent leaves its
/// domain open, which is exactly what is known. `None` when the demand is not
/// a proposable dependency-closure demand of this node, in which case the
/// refusal stands.
fn composed_from_withheld_dependency(
    node: &PlannedGraphNode,
    demand_id: &str,
    child_claim: &str,
) -> Option<super::WithheldClosure> {
    let demand = node
        .plan
        .demand_graph()
        .demands()
        .iter()
        .find(|demand| demand.id().as_str() == demand_id)?;
    let ProofDemandSubject::DependencyClosure {
        dependency, parent, ..
    } = demand.subject()
    else {
        return None;
    };
    let super::SemanticClaimPath::Domain(super::ClaimPath::Call(domain)) = &parent.path else {
        return None;
    };
    if !domain.is_proposable() {
        return None;
    }
    let semantic_claim_id = node.plan.candidates.proposal().claim_id(parent).ok()?;
    Some(super::WithheldClosure {
        artifact_case: parent.artifact_case.clone(),
        export: parent.export.clone(),
        domain: super::type_facts::call_claim_domain_name(*domain).to_owned(),
        semantic_claim_id: semantic_claim_id.as_str().to_owned(),
        reason: format!(
            "{}{child_claim} of {}",
            super::WITHHELD_CLOSURE_DEPENDENCY_WITHHELD_PREFIX,
            dependency.package
        ),
    })
}

/// Certifies a complete root case-set through one native bottom-up transaction.
/// Type Facts acquisition shares a session with bounded context isolation for
/// duplicate installations. Canonical nodes shared by multiple roots are
/// acquired once for evidence; receipt composition remains graph-root-local,
/// so no child receipt is transplanted between root graphs.
pub fn certify_published_contract_graph_case_set(
    graphs: &[PublishedContractGraphPlan],
    pin: &TypeFactsProducerPin,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
    probes: Option<&super::ProbeHarnessConfiguration>,
) -> Result<Vec<FinalizedPolicy2Graph>, PublishedGraphCertificationError> {
    certify_graphs_with_recipe_gating(graphs, pin, issuer, revocation_epoch, probes)
}

/// ADR 0036 for the graph lanes: recipe gating, census-refusal and
/// incomplete-veto withholding, and synthesized vetoes, per node.
///
/// The value-only lane's bounded loop (`CertificationPlan::certify_value_only`)
/// applied to every node of every graph in the case set at once. Each pass
/// re-gates every node with the corpus and the already-withdrawn candidates
/// chosen for *that* node, acquires exported-value evidence for the nodes whose
/// gating changed since their evidence was taken, and then either withdraws one
/// more candidate by name -- a census that could not decide it, a veto run that
/// did not complete -- and goes again, or synthesizes a veto for every node's
/// recipe-less candidate that stated a call signature (once, before the first
/// gate runs), or finalizes. Every pass withdraws at least one candidate or is
/// the one synthesis pass, so the loop is bounded by the number of closure
/// candidates in the case set plus two.
///
/// A node's synthesized corpus and a node's withdrawals are keyed by its
/// canonical identity digest, so nothing a child withheld or synthesized
/// reaches a parent's gate, and canonical nodes two roots share are gated and
/// acquired once. Evidence is re-acquired only for nodes whose own gating moved:
/// a gate changes a node's demand graph, never its resolution, and a parent's
/// dependency demands hash the accepted proposal digest the gate leaves alone.
fn certify_graphs_with_recipe_gating(
    graphs: &[PublishedContractGraphPlan],
    pin: &TypeFactsProducerPin,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
    probes: Option<&super::ProbeHarnessConfiguration>,
) -> Result<Vec<FinalizedPolicy2Graph>, PublishedGraphCertificationError> {
    let first_graph = graphs
        .first()
        .ok_or(PublishedGraphCertificationError::EmptyCaseSet)?;
    let label = if graphs.len() == 1 {
        first_graph.graph_root().to_owned()
    } else {
        "published-graph-case-set".to_owned()
    };
    let candidate_count = graphs
        .iter()
        .flat_map(|graph| graph.nodes.iter())
        .map(|node| node.plan.candidates.closure_candidates().len())
        .sum::<usize>();
    let passes = candidate_count + 2;
    let mut already_withheld = BTreeMap::<String, Vec<super::WithheldClosure>>::new();
    let mut synthesized = BTreeMap::<String, super::synthesized_vetoes::SynthesizedCorpus>::new();
    let mut synthesis_attempted = false;
    let base_corpus = probes.map(super::ProbeHarnessConfiguration::recipe_corpus);
    // Evidence and gates persist across passes, each entry keyed by the gating
    // it was taken under: a node whose demand-graph root (and, for gates, whose
    // corpus) did not move keeps both, so a pass costs only the nodes it moved.
    let mut evidence_by_node =
        BTreeMap::<String, super::type_facts::VerifiedTypeFactsEvidence>::new();
    let mut evidence_roots = BTreeMap::<String, String>::new();
    let mut gates_by_node =
        BTreeMap::<String, (String, super::probe_gates::VerifiedProbeGateBatch)>::new();
    let emit_timings = std::env::var_os("SOLID_CHECKER_TIMINGS").is_some();
    for pass in 0..passes {
        let mut timing = GraphGatingPassTiming {
            pass,
            ..GraphGatingPassTiming::default()
        };
        let pass_started = std::time::Instant::now();
        let gated = graphs
            .iter()
            .map(|graph| graph.recipe_gated_per_node(&synthesized, base_corpus, &already_withheld))
            .collect::<Result<Vec<_>, _>>()?;
        // Every Type Facts node stays in the request set on every pass, and
        // only the nodes whose demand-graph root moved are acquired again.
        // Acquiring a subset of the *plans* was tried and is unsound: an
        // export's runtime binding may belong to another node's snapshot (a
        // re-export), and `export_implementation_location` finds that owner
        // among the plans being acquired, so a subset left such an export
        // bound to "an unplanned snapshot". Keeping every plan in the request
        // while acquiring only the moved ones keeps the owner lookup whole.
        timing.nodes = gated.iter().map(|graph| graph.nodes.len()).sum();
        let acquisition_started = std::time::Instant::now();
        let acquired = acquire_case_set_evidence(&gated, pin, &evidence_roots);
        timing.acquisition_ns = elapsed_ns(acquisition_started);
        match acquired {
            Ok(fresh) => {
                timing.acquired = fresh.len();
                for (digest, evidence) in fresh {
                    let root = gated
                        .iter()
                        .find_map(|graph| graph.node(&digest))
                        .map(|node| node.plan.demand_graph().root().as_str().to_owned())
                        .expect("fresh evidence names a gated node");
                    evidence_roots.insert(digest.clone(), root);
                    evidence_by_node.insert(digest, evidence);
                }
            }
            Err(PublishedGraphCertificationError::TypeFactsForGraph { source, .. }) => {
                // Every node's census refusals at once (`CensusRefused`
                // carries them all), each withheld at its own node.
                let mut seen = BTreeSet::new();
                let mut withdrawn = 0_usize;
                for node in gated.iter().flat_map(|graph| graph.nodes.iter()) {
                    let digest = node.identity.digest();
                    if !seen.insert(digest.to_owned()) {
                        continue;
                    }
                    let records = super::census_refusal_withholding(&node.plan, &source);
                    withdrawn += records.len();
                    if !records.is_empty() {
                        already_withheld
                            .entry(digest.to_owned())
                            .or_default()
                            .extend(records);
                    }
                }
                if withdrawn == 0 {
                    return Err(PublishedGraphCertificationError::TypeFactsForGraph {
                        graph: label,
                        source,
                    });
                }
                timing.withdrawn = withdrawn;
                timing.emit(emit_timings, pass_started);
                continue;
            }
            Err(error) => return Err(error),
        };
        if !synthesis_attempted {
            synthesis_attempted = true;
            if let Some(base) = probes {
                let synthesis_started = std::time::Instant::now();
                let mut changed = false;
                for node in gated.iter().flat_map(|graph| graph.nodes.iter()) {
                    let digest = node.identity.digest();
                    if synthesized.contains_key(digest) {
                        continue;
                    }
                    let Some(evidence) = evidence_by_node.get(digest) else {
                        continue;
                    };
                    let corpus = super::synthesized_vetoes::synthesize(
                        &node.plan,
                        evidence,
                        base,
                        &node.withheld,
                    )
                    .map_err(|error| {
                        PublishedGraphCertificationError::FinalizationAtNode {
                            node: digest.to_owned(),
                            package: format!(
                                "{}@{}",
                                node.identity.package_name, node.identity.package_version
                            ),
                            source: super::Policy2FinalizationError::VetoSynthesis(
                                error.to_string(),
                            ),
                        }
                    })?;
                    if let Some(corpus) = corpus {
                        synthesized.insert(digest.to_owned(), corpus);
                        changed = true;
                        timing.synthesized += 1;
                    }
                }
                timing.synthesis_ns = elapsed_ns(synthesis_started);
                if changed {
                    timing.emit(emit_timings, pass_started);
                    continue;
                }
            }
        }
        // Gate pre-pass: every node's veto set runs now, so one pass collects
        // every incomplete veto and every synthesized veto the interpreter
        // cannot run, instead of one per pass. The batches are independent —
        // each has its own private workspace, corpus, and evidence — so they
        // run side by side (`parallel::run_each`) and are applied afterwards
        // in node order, which keeps every withdrawal, every cache entry, and
        // the first reported error where the sequential loop put them.
        let mut withdrawals = Vec::<(String, super::WithheldClosure)>::new();
        let mut dropped_corpora = BTreeSet::<String>::new();
        let mut gated_this_pass = BTreeSet::<String>::new();
        let mut jobs = Vec::new();
        for graph in &gated {
            for node in &graph.nodes {
                let digest = node.identity.digest();
                if !gated_this_pass.insert(digest.to_owned()) {
                    continue;
                }
                let node_probes = synthesized
                    .get(digest)
                    .map(super::synthesized_vetoes::SynthesizedCorpus::configuration)
                    .or(probes);
                let gating_key = format!(
                    "{}\0{}",
                    node.plan.demand_graph().root().as_str(),
                    node_probes.map_or_else(String::new, |configuration| {
                        configuration.recipe_corpus().to_string_lossy().into_owned()
                    })
                );
                if gates_by_node
                    .get(digest)
                    .is_some_and(|(key, _)| *key == gating_key)
                {
                    continue;
                }
                gates_by_node.remove(digest);
                jobs.push(GateJob {
                    digest: digest.to_owned(),
                    package: format!(
                        "{}@{}",
                        node.identity.package_name, node.identity.package_version
                    ),
                    node,
                    node_probes,
                    gating_key,
                    dependencies: graph.transitive_dependency_plans(node)?,
                });
            }
        }
        let gate_started = std::time::Instant::now();
        timing.gate_workers = super::parallel::workers_for(jobs.len());
        let outcomes = super::parallel::run_each(&jobs, timing.gate_workers, |job| {
            super::finalization::authenticate_probe_gates_with_dependencies(
                &job.node.plan,
                job.node_probes,
                pin,
                &job.dependencies,
            )
        });
        timing.gate_runs = jobs.len();
        timing.gate_sessions = jobs
            .iter()
            .map(|job| {
                job.node
                    .plan
                    .probe_gate_schedule()
                    .map_or(0, |schedule| schedule.gates().len())
            })
            .sum();
        timing.gate_ns = elapsed_ns(gate_started);
        for (job, authenticated) in jobs.into_iter().zip(outcomes) {
            let GateJob {
                digest,
                package,
                node,
                gating_key,
                ..
            } = job;
            let digest = digest.as_str();
            match authenticated {
                Ok(gates) => {
                    gates_by_node.insert(digest.to_owned(), (gating_key, gates));
                }
                Err(source) => {
                    if let Some(record) = super::incomplete_gate_withholding(&node.plan, &source) {
                        withdrawals.push((digest.to_owned(), record));
                        continue;
                    }
                    // A synthesized veto the pinned interpreter cannot run
                    // for this artifact case -- an export condition it
                    // cannot be given (`@tanstack/custom-condition`), or
                    // one under which it would load a different file than
                    // the witness read (`solid` selecting `dist/solid.js`
                    // where Node selects `dist/server.js`). The hand corpus
                    // named nothing for these candidates and the checker's
                    // own veto cannot be executed, so they are withheld
                    // with that reason and the node keeps the hand corpus.
                    // A hand recipe that hits the same binding refuses as
                    // it always did.
                    let cannot_run = matches!(
                        &source,
                        super::Policy2FinalizationError::ProbeHarness(
                            super::probe_harness::ProbeHarnessError::Configuration(_)
                                | super::probe_harness::ProbeHarnessError::ConditionMismatch(_)
                        )
                    );
                    if cannot_run && synthesized.contains_key(digest) {
                        let served = graphs
                            .iter()
                            .find_map(|original| original.node(digest))
                            .map(|original| {
                                original.plan.recipe_gated_with(
                                    base_corpus,
                                    already_withheld.get(digest).map_or(&[], Vec::as_slice),
                                )
                            })
                            .transpose()
                            .map_err(|gating| {
                                PublishedGraphCertificationError::RecipeGatingAtNode {
                                    node: digest.to_owned(),
                                    package: package.clone(),
                                    source: Box::new(gating),
                                }
                            })?
                            .map(|gated| gated.into_parts().1)
                            .unwrap_or_default();
                        let schedule = node.plan.probe_gate_schedule().ok();
                        let records = served
                        .into_iter()
                        .filter(|record| record.reason == super::WITHHELD_CLOSURE_NO_RECIPE)
                        .map(|record| {
                            let gate_id = schedule
                                .as_ref()
                                .and_then(|schedule| {
                                    schedule.gates().iter().find(|gate| {
                                        gate.semantic_claim_id() == record.semantic_claim_id
                                    })
                                })
                                .map_or_else(
                                    || "unscheduled".to_owned(),
                                    |gate| gate.id().to_owned(),
                                );
                            (
                                digest.to_owned(),
                                super::WithheldClosure {
                                    reason: format!(
                                        "{}{gate_id} (synthesized veto cannot run for this artifact case: {source})",
                                        super::WITHHELD_CLOSURE_VETO_INCOMPLETE_PREFIX
                                    ),
                                    ..record
                                },
                            )
                        })
                        .collect::<Vec<_>>();
                        if !records.is_empty() {
                            withdrawals.extend(records);
                            dropped_corpora.insert(digest.to_owned());
                            continue;
                        }
                    }
                    return Err(PublishedGraphCertificationError::FinalizationAtNode {
                        node: digest.to_owned(),
                        package,
                        source,
                    });
                }
            }
        }
        for digest in &dropped_corpora {
            synthesized.remove(digest);
        }
        if !withdrawals.is_empty() {
            timing.withdrawn = withdrawals.len();
            for (digest, record) in withdrawals {
                already_withheld.entry(digest).or_default().push(record);
            }
            timing.emit(emit_timings, pass_started);
            continue;
        }
        let mut finalized = Vec::with_capacity(gated.len());
        let mut composition_withdrawals = Vec::new();
        for graph in &gated {
            match graph.finalize_value_only_with_type_facts(
                &evidence_by_node,
                &gates_by_node,
                pin,
                issuer,
                revocation_epoch,
            )? {
                Ok(contract) => finalized.push(contract),
                Err(withdrawn) => composition_withdrawals.extend(withdrawn),
            }
        }
        timing.withdrawn = composition_withdrawals.len();
        timing.emit(emit_timings, pass_started);
        if composition_withdrawals.is_empty() {
            return Ok(finalized);
        }
        for (digest, record) in composition_withdrawals {
            already_withheld.entry(digest).or_default().push(record);
        }
    }
    Err(PublishedGraphCertificationError::WithholdingDidNotConverge { passes })
}

/// One node's probe-gate batch to run in the gate pre-pass of
/// [`certify_graphs_with_recipe_gating`]: everything the run reads, resolved
/// before any batch starts so the batches share no lookups.
struct GateJob<'a> {
    digest: String,
    package: String,
    node: &'a PlannedGraphNode,
    node_probes: Option<&'a super::ProbeHarnessConfiguration>,
    gating_key: String,
    dependencies: Vec<&'a CertificationPlan>,
}

/// What one pass of [`certify_graphs_with_recipe_gating`] cost and moved,
/// reported under `SOLID_CHECKER_TIMINGS` so a slow graph row is attributable
/// to acquisition, synthesis, or gate launches rather than guessed at.
#[derive(Debug, Default)]
struct GraphGatingPassTiming {
    pass: usize,
    nodes: usize,
    acquired: usize,
    acquisition_ns: u64,
    synthesized: usize,
    synthesis_ns: u64,
    gate_runs: usize,
    gate_sessions: usize,
    gate_workers: usize,
    gate_ns: u64,
    withdrawn: usize,
}

impl GraphGatingPassTiming {
    fn emit(&self, enabled: bool, pass_started: std::time::Instant) {
        if !enabled {
            return;
        }
        eprintln!(
            "{}",
            serde_json::json!({
                "mode": "graph-recipe-gating",
                "pass": self.pass,
                "nodes": self.nodes,
                "acquired": self.acquired,
                "acquisitionNs": self.acquisition_ns,
                "synthesized": self.synthesized,
                "synthesisNs": self.synthesis_ns,
                "gateRuns": self.gate_runs,
                "gateSessions": self.gate_sessions,
                "gateWorkers": self.gate_workers,
                "gateNs": self.gate_ns,
                "withdrawn": self.withdrawn,
                "passNs": elapsed_ns(pass_started),
            })
        );
    }
}

fn elapsed_ns(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Exported-value evidence for the Type Facts nodes of the case set whose
/// gating moved since `held` was taken, keyed by canonical identity digest;
/// `held` maps a digest to the demand-graph root its evidence was acquired
/// under, and a node whose root is unchanged is not acquired again. Every node
/// still takes part in schedule derivation, so an export whose runtime binding
/// belongs to another node's snapshot finds its owner. Canonical nodes shared
/// by several roots are acquired once; the same digest naming two different
/// identities is refused.
fn acquire_case_set_evidence(
    graphs: &[PublishedContractGraphPlan],
    pin: &TypeFactsProducerPin,
    held: &BTreeMap<String, String>,
) -> Result<
    BTreeMap<String, super::type_facts::VerifiedTypeFactsEvidence>,
    PublishedGraphCertificationError,
> {
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
        for (identity, mut request) in graph.type_facts_requests()? {
            let node = graph
                .nodes
                .iter()
                .find(|node| node.identity.digest() == identity)
                .expect("Type Facts requests originate from retained graph nodes");
            request.acquire =
                held.get(&identity) != Some(&node.plan.demand_graph().root().as_str().to_owned());
            if identities
                .insert(identity.clone(), node.identity.clone())
                .is_some_and(|previous| previous != node.identity)
            {
                return Err(PublishedGraphCertificationError::CanonicalIdentityCollision(identity));
            }
            // The canonical identity travels with its request rather than
            // being looked up again when the order is derived. A lookup would
            // need an answer for a key it cannot find, and every such answer
            // is wrong: `None` coordinates sort the node first, and a panic
            // turns an ordering decision into a crash.
            requests
                .entry(identity)
                .or_insert((node.identity.clone(), request));
        }
    }
    let mut request_entries = requests.into_iter().collect::<Vec<_>>();
    request_entries.sort_by(|(_, (left, _)), (_, (right, _))| {
        compare_type_facts_request_coordinates(
            type_facts_request_order_key(left),
            type_facts_request_order_key(right),
        )
    });
    let (request_keys, request_values): (Vec<_>, Vec<_>) = request_entries
        .into_iter()
        .map(|(digest, (_, request))| (digest, request))
        .unzip();
    let evidence =
        super::type_facts::acquire_and_verify_graph_export_values(root_plan, &request_values, pin)
            .map_err(
                |source| PublishedGraphCertificationError::TypeFactsForGraph {
                    graph: "published-graph-case-set".into(),
                    source,
                },
            )?;
    Ok(request_keys
        .into_iter()
        .zip(evidence)
        .filter_map(|(digest, evidence)| evidence.map(|evidence| (digest, evidence)))
        .collect())
}

/// The coordinates one case-set Type Facts request is ordered by, most
/// significant first. See `type_facts_request_order_key`.
type TypeFactsRequestOrderKey<'a> = (
    &'a str,
    &'a str,
    &'a str,
    usize,
    &'a [String],
    &'a str,
    &'a str,
    &'a str,
);

/// The order one case set's Type Facts requests are acquired and verified in —
/// and therefore, since verification stops at the first open demand, which
/// node's refusal a failing case set reports.
///
/// Package name, version and requested entrypoint first, so a case set stays
/// grouped by package; then **fewest conditions first**, the conditions
/// themselves, and the package-relative runtime and declaration targets those
/// conditions resolve to. Every coordinate is a property of the packages; the
/// canonical identity digest is only the last resort, and it is never reached
/// in practice (see the residual below).
///
/// Condition *count* precedes the conditions because the list alone orders
/// them backwards. `certify-contract.mjs` adds `import` to
/// every case's condition set, so the unconditional case is `["import"]` and
/// the opt-in cases are supersets of it — and *lexicographically* a superset
/// starting with a lower-sorting word comes first:
/// `["@tanstack/custom-condition", "import"]` and `["development", "import"]`
/// both sort before `["import"]`, because `@` and `d` precede `i`. Ordering by
/// the condition list alone therefore put the publisher-private TypeScript
/// source case first and the ordinary consumer's case last, the exact
/// inversion of the intent. The count restores it: `["import"]` is the
/// shortest set any case can have.
///
/// The digest used to be the *third* tie-break, and it is salted by absolute
/// paths — the canonical identity binds `importer` and the resolved import
/// root, which under any harness that installs into a fresh temporary
/// directory differ on every run. Alternative artifact cases of one package
/// share name, version and entrypoint, so the digest alone decided their
/// relative order, and the case whose refusal a failing set reported changed
/// run to run from identical inputs and identical binaries:
/// `@tanstack/query-persist-client-core`'s plain `import` case
/// (`build/modern/createPersister.js`) one run and its
/// `@tanstack/custom-condition` case (`src/createPersister.ts`) the next, with
/// the demand digest in the report flipping with it.
///
/// Residual, unobservable within one run: two nodes agreeing on every
/// coordinate above and differing only in `importer` would still tie down to
/// the path-salted digest. One `name@version` resolves to one integrity in a
/// lockfile, so two such nodes are byte-identical installations of the same
/// package reached from different importers, and every premise proved over
/// them is the same. Nothing in the corpus produces the pair.
///
/// The sibling lane orders differently and deliberately:
/// `certify_value_only` consumes `type_facts_requests()` in `self.nodes`
/// order, which is the retained dependency-first planning order. That is
/// deterministic and path-independent too, but it is not this order, and a
/// single-graph run and a case-set run can report different first refusals for
/// the same node set.
fn type_facts_request_order_key(
    identity: &CanonicalDependencyNodeIdentity,
) -> TypeFactsRequestOrderKey<'_> {
    (
        identity.package_name.as_str(),
        identity.package_version.as_str(),
        identity.entrypoint.as_str(),
        identity.conditions.len(),
        identity.conditions.as_slice(),
        identity.runtime_target.as_str(),
        identity.declarations_target.as_str(),
        identity.digest(),
    )
}

fn compare_type_facts_request_coordinates(
    left: TypeFactsRequestOrderKey<'_>,
    right: TypeFactsRequestOrderKey<'_>,
) -> std::cmp::Ordering {
    left.cmp(&right)
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
    #[error("recipe-gated planning failed for graph node {node} ({package}): {source}")]
    RecipeGatingAtNode {
        node: String,
        package: String,
        // Boxed: the gating error carries a whole planning error, and an
        // unboxed one would grow every `Result` in this module past the size
        // Clippy's `result_large_err` accepts.
        #[source]
        source: Box<super::RecipeGatingError>,
    },
    /// ADR 0036's withdraw-and-re-plan passes are bounded by the number of
    /// closure candidates in the graph plus one synthesis pass; exceeding that
    /// is a defect in the bookkeeping, not a property of the package.
    #[error("recipe gating of the published graph did not converge within {passes} passes")]
    WithholdingDidNotConverge { passes: usize },
    #[error("graph node {0} reached finalization without an authenticated probe gate set")]
    MissingProbeGates(String),
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
            let identity = match matches.as_slice() {
                [identity] => identity.clone(),
                [] => {
                    return Err(PublishedGraphPlanningError::MissingDependency {
                        parent: planned[parent_index].identity.digest.clone(),
                        specifier: edge.specifier,
                    });
                }
                _ => {
                    let variants = matches
                        .iter()
                        .map(|identity| {
                            (
                                identity.importer_invariant_key(),
                                identity.importer.as_str(),
                            )
                        })
                        .collect::<Vec<_>>();
                    match select_importer_variant(&variants) {
                        Some(position) => matches[position].clone(),
                        None => {
                            return Err(PublishedGraphPlanningError::AmbiguousDependency {
                                parent: planned[parent_index].identity.digest.clone(),
                                specifier: edge.specifier,
                            });
                        }
                    }
                }
            };
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
            resolved.push(identity);
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

/// Selects, among several nodes that all match one dependency edge of a parent,
/// the node the parent's discovery bound to that edge.
///
/// Discovery keys a dependency node by the module that imports it, and a
/// package with many entrypoints reaches the same dependency from many of its
/// modules, so a parent's closure can contain the importers of several nodes
/// that are the same artifact, resolution, and proposal and differ only in
/// which of the package's modules imported them. Those nodes are
/// interchangeable as a dependency, and the one discovery bound to *this*
/// parent is the one whose importer sorts first: discovery binds a parent to
/// the first of its importing modules in the same byte order, and every node
/// whose importer is a member of the parent's closure is one of those modules.
/// Nodes that differ in anything beyond the importer are not interchangeable,
/// and the tie stays refused. Returns the position of the selected variant.
fn select_importer_variant<K: PartialEq>(variants: &[(K, &str)]) -> Option<usize> {
    let (first_key, _) = variants.first()?;
    if variants.iter().any(|(key, _)| key != first_key) {
        return None;
    }
    variants
        .iter()
        .enumerate()
        .min_by_key(|(_, (_, importer))| *importer)
        .map(|(position, _)| position)
}

/// Everything an unplanned graph request's replayed resolution names except
/// the importing module, for [`select_importer_variant`].
fn request_importer_invariant_key(certification: &CertificationRequest) -> Vec<String> {
    let resolved = &certification.resolved_import;
    let mut conditions = certification.import_request.export_conditions.clone();
    conditions.sort();
    conditions.dedup();
    let mut key = vec![
        certification.import_request.specifier.clone(),
        resolved.package_name.clone(),
        resolved.package_version.clone(),
        resolved.package_integrity.clone(),
        resolved.package_root.clone(),
        resolved.requested_entrypoint.clone(),
        resolved.runtime.path.clone(),
        resolved.runtime.digest.clone(),
        resolved.declarations.path.clone(),
        resolved.declarations.digest.clone(),
        resolved.closure.digest.clone(),
        format!("{:?}", resolved.authority),
    ];
    key.extend(conditions);
    key
}

/// True when `importer` is exactly one runtime- or declaration-role module of
/// the parent's replayed, digest-pinned verified closure, reconstructed against
/// its package root. This is the authoritative dependency-edge matcher: it
/// admits a re-export issued from a non-entry module of the parent package
/// while still rejecting any importer that is not a member of the parent's
/// proven closure (for instance one transplanted outside the package root).
pub(super) fn importer_is_closure_entry_module(
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
        // The supplied closure is untrusted here and only orders planning;
        // `plan_published_contract_graph_with_limits` re-derives every edge
        // against the replayed, digest-pinned closure with the same matcher.
        let parent_entries = &parent.certification.resolved_import.closure.entries;
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
                        && importer_is_closure_entry_module(
                            &child.certification.import_request.importer,
                            parent_root,
                            parent_entries,
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
            let selected = match matches.as_slice() {
                [index] => *index,
                [] => {
                    return Err(PublishedGraphPlanningError::MissingDependency {
                        parent: parent_label,
                        specifier: edge.specifier.clone(),
                    });
                }
                _ => {
                    let variants = matches
                        .iter()
                        .map(|index| {
                            let certification = &requests[*index].certification;
                            (
                                request_importer_invariant_key(certification),
                                certification.import_request.importer.as_str(),
                            )
                        })
                        .collect::<Vec<_>>();
                    match select_importer_variant(&variants) {
                        Some(position) => matches[position],
                        None => {
                            return Err(PublishedGraphPlanningError::AmbiguousDependency {
                                parent: parent_label,
                                specifier: edge.specifier.clone(),
                            });
                        }
                    }
                }
            };
            graph[parent_index].push(selected);
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
    let accepted_candidate = plan.selected_candidate.clone();
    Ok(PlannedGraphNode {
        identity,
        plan,
        dependencies: Vec::new(),
        source_dependencies,
        withheld: Vec::new(),
        accepted_candidate,
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
    /// The **parent's** own closure-candidate claim id, which is what
    /// `creates_census` and `dependency_creates_claims` resolve against the
    /// parent's `DomainClosure` demand.
    semantic_claim_id: Option<String>,
    /// The **dependency's** claim this requirement demands closed in the
    /// dependency's receipt. Demand planning names none -- see the condition
    /// in `authenticate_dependency_receipt` for why that is correct rather
    /// than missing -- so today only a caller that has a real dependency
    /// claim in hand sets it.
    dependency_semantic_claim_id: Option<String>,
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
    pub fn dependency_semantic_claim_id(&self) -> Option<&str> {
        self.dependency_semantic_claim_id.as_deref()
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
                        dependency_semantic_claim_id: None,
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
                    dependency_semantic_claim_id: None,
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
    census_requirements_root: Option<String>,
    factory_requirements_root: Option<String>,
}

impl VerifiedDependencyComposition {
    #[allow(clippy::too_many_arguments)]
    fn authenticate(
        parent: &CertificationPlan,
        expected_dependencies: &[CanonicalDependencyNodeIdentity],
        source_dependencies: &[VerifiedGraphSourcePackage],
        gating: &BTreeMap<String, DependencyGating<'_>>,
        graph_root: &str,
        receipts: &[(
            &CanonicalDependencyNodeIdentity,
            &AuthenticatedPolicy2Receipt,
        )],
        issuer: &ConfiguredReceiptIssuer,
        revocation_epoch: u64,
        type_facts: Option<&super::type_facts::VerifiedTypeFactsEvidence>,
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
        let factory_claims = type_facts
            .map(|facts| facts.factory_return_claims())
            .unwrap_or_default();
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
            let dependency_gating = gating.get(dependency.digest()).ok_or_else(|| {
                DependencyReceiptCompositionError::DependencyOutsideGraph {
                    dependency: dependency.digest().into(),
                }
            })?;
            let census = requirement
                .semantic_claim_id()
                .and_then(|claim| type_facts?.creates_census(parent, claim));
            authenticate_dependency_receipt(
                parent,
                requirement,
                dependency,
                dependency_gating,
                receipt,
                issuer,
                revocation_epoch,
                census,
            )?;
            let census_claims = requirement
                .semantic_claim_id()
                .and_then(|claim| {
                    type_facts.map(|facts| facts.dependency_creates_claims(parent, claim))
                })
                .unwrap_or_default();
            let mut census_sites = Vec::new();
            for claim in census_claims.iter().filter(|claim| {
                claim.package == requirement.dependency().package
                    && claim.artifact_case == requirement.dependency().artifact_case
                    && claim.accepted_contract_digest
                        == requirement.dependency().accepted_contract_digest
            }) {
                let empty = dependency_gating
                    .certified_candidate
                    .artifact_case(&claim.artifact_case)
                    .and_then(|case| case.exports.get(&claim.export))
                    .and_then(|export| {
                        export.operation_claim(
                            solid_reactive_ir::contract_semantics::ClaimDomain::Creates,
                        )
                    })
                    .is_some_and(|creates| creates.is_closed() && creates.items().is_empty());
                if !empty || !receipt.contains_closed_claim_id(&claim.semantic_claim_id) {
                    return Err(DependencyReceiptCompositionError::MissingClosedClaim {
                        demand_id: requirement.demand_id().into(),
                        semantic_claim_id: claim.semantic_claim_id.clone(),
                    });
                }
                census_sites.push(format!(
                    "census-dependency-creates:{}:{}:{}",
                    claim.export,
                    claim.semantic_claim_id,
                    receipt.receipt_digest()
                ));
            }
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
            let evidence_root = census.map_or(evidence_root.clone(), |census| {
                composition_root(
                    if census_claims.is_empty() {
                        "independent-creates-census-composition"
                    } else {
                        "dependency-creates-census-composition"
                    },
                    graph_root,
                    &[evidence_root.as_str(), census],
                )
            });
            let mut sites = vec![
                format!("graph:{graph_root}"),
                format!("parent-case:{}", parent.selected_artifact_case_id()),
                format!("dependency-node:{}", dependency.digest()),
                format!("dependency-receipt:{}", receipt.receipt_digest()),
            ];
            if let Some(census) = census {
                let kind = if census_claims.is_empty() {
                    "independent-creates-census"
                } else {
                    "dependency-creates-census"
                };
                sites.push(format!("{kind}:{census}"));
            }
            sites.extend(census_sites);
            witnesses.push(
                solid_reactive_ir::contract_semantics::certification::WitnessBinding::new(
                    solid_reactive_ir::contract_semantics::certification::ProofWitnessVariant::AcceptedDependencyComposition,
                    requirement.demand_id(),
                    evidence_root,
                    sites,
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
        // A positive factory proof names its importing module explicitly.
        // Discharge each claim against that exact node, independently of the
        // representative an ordinary dependency-artifact demand selected.
        // No token is returned if even one conditional proof is unfulfilled.
        for (demand_id, claim) in &factory_claims {
            let missing = || DependencyReceiptCompositionError::MissingGraphEdge {
                demand_id: demand_id.clone(),
            };
            let requirement = schedule
                .requirements()
                .iter()
                .find(|requirement| {
                    requirement.authenticates_dependency_artifact()
                        && requirement.dependency().package == claim.package
                        && requirement.dependency().artifact_case == claim.artifact_case
                        && requirement.dependency().accepted_contract_digest
                            == claim.accepted_contract_digest
                        && requirement.dependency().specifier == claim.specifier
                })
                .ok_or_else(missing)?;
            let mut selected = expected_dependencies.iter().filter(|identity| {
                identity.package_name == claim.package
                    && identity.artifact_case == claim.artifact_case
                    && identity.semantic_digest == claim.accepted_contract_digest
                    && identity.importer == claim.importer
                    && identity.resolved_import_root == claim.resolved_import_root
            });
            let dependency = selected.next().ok_or_else(missing)?;
            if selected.next().is_some() {
                return Err(missing());
            }
            let receipt = receipt_map.get(dependency.digest()).ok_or_else(|| {
                DependencyReceiptCompositionError::MissingReceipt {
                    dependency: dependency.digest().into(),
                }
            })?;
            let dependency_gating = gating.get(dependency.digest()).ok_or_else(|| {
                DependencyReceiptCompositionError::DependencyOutsideGraph {
                    dependency: dependency.digest().into(),
                }
            })?;
            authenticate_dependency_receipt(
                parent,
                requirement,
                dependency,
                dependency_gating,
                receipt,
                issuer,
                revocation_epoch,
                None,
            )?;
            if !claim.is_closed_in(dependency_gating.certified_candidate)
                || !receipt.contains_closed_claim_id(&claim.semantic_claim_id)
            {
                return Err(DependencyReceiptCompositionError::MissingClosedClaim {
                    demand_id: demand_id.clone(),
                    semantic_claim_id: claim.semantic_claim_id.clone(),
                });
            }
            if verifier_build_digest.as_deref() != Some(receipt.verifier_build_digest().as_str()) {
                return Err(DependencyReceiptCompositionError::VerifierBuildDisagreement);
            }
            receipt_rows.push(format!(
                "factory-return-discharge:{demand_id}:{}:{}:{}:{}:{}",
                claim.export,
                claim.semantic_claim_id,
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
            census_requirements_root: type_facts
                .and_then(super::type_facts::VerifiedTypeFactsEvidence::dependency_census_root),
            factory_requirements_root: type_facts
                .and_then(super::type_facts::VerifiedTypeFactsEvidence::factory_requirements_root),
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

    pub(super) fn verify_type_facts_requirements(
        &self,
        type_facts: &super::type_facts::VerifiedTypeFactsEvidence,
    ) -> Result<(), DependencyReceiptCompositionError> {
        if self.census_requirements_root != type_facts.dependency_census_root()
            || self.factory_requirements_root != type_facts.factory_requirements_root()
        {
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

/// Authenticates one dependency receipt against the edge the parent accepted.
///
/// # The gated dependency
///
/// The parent accepted the dependency's proposal as *planned* — the edge's
/// `accepted_contract_digest`, which is also the node identity's
/// `semantic_digest`. Recipe gating may then have withheld `creates` closure
/// candidates from that node, so the contract its receipt certifies is a
/// **weakening** of the accepted proposal: the same document with those domains
/// opened. Requiring the receipt's digest to equal the accepted digest would
/// refuse every such graph; accepting any digest would let a receipt for some
/// other document compose. What is required instead is that the certified
/// document be *exactly* that weakening, established three ways:
///
/// 1. the accepted proposal's digest is the edge's digest (the parent accepted
///    what this node was planned with);
/// 2. the weakening is re-derived here, independently of gating, from the
///    accepted proposal and the node's withheld records
///    (`super::withheld_weakening`), and its digest is what the receipt — and
///    the receipt's own bindings — carry;
/// 3. the plan that was actually certified is that same document, so nothing
///    between gating and issuance substituted another.
///
/// A parent demand that *relied* on a withheld closure still refuses on its
/// own, and `creates` is the domain where that can happen: its census follows
/// callees, so `dependency_creates_claims` names the dependency claims the
/// parent composed from and the caller checks each against this receipt
/// (`MissingClosedClaim`). The claim-id condition inside this function is a
/// second, narrower guard for callers that name a dependency claim directly;
/// see the comment at it for why a planning-built `DependencyClosure`
/// requirement deliberately does not reach it.
/// ADR 0020 adds one independent premise: a live-verified creates census of
/// the exact parent demand can prove that claim without any dependency
/// semantic assumption. Receipt identity and weakening still authenticate,
/// and the census evidence root is bound into the composition witness.
#[allow(clippy::too_many_arguments)]
fn authenticate_dependency_receipt(
    parent: &CertificationPlan,
    requirement: &DependencyCompositionRequirement,
    dependency: &CanonicalDependencyNodeIdentity,
    gating: &DependencyGating<'_>,
    receipt: &AuthenticatedPolicy2Receipt,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
    independent_creates_census: Option<&str>,
) -> Result<(), DependencyReceiptCompositionError> {
    let bindings = receipt.bindings();
    if dependency.semantic_digest != requirement.dependency().accepted_contract_digest {
        return Err(DependencyReceiptCompositionError::ReceiptMismatch {
            field: "accepted contract digest",
            actual: dependency.semantic_digest.clone(),
            expected: requirement.dependency().accepted_contract_digest.clone(),
        });
    }
    if gating.accepted_candidate.semantic_digest().as_str() != dependency.semantic_digest {
        return Err(DependencyReceiptCompositionError::ReceiptMismatch {
            field: "accepted candidate digest",
            actual: gating.accepted_candidate.semantic_digest().as_str().into(),
            expected: dependency.semantic_digest.clone(),
        });
    }
    let certified_digest = if gating.withheld.is_empty() {
        dependency.semantic_digest.clone()
    } else {
        let weakened = super::withheld_weakening(gating.accepted_candidate, gating.withheld)
            .map_err(
                |error| DependencyReceiptCompositionError::WithheldWeakening {
                    dependency: dependency.digest().into(),
                    reason: error.to_string(),
                },
            )?;
        if weakened.semantic_digest() != gating.certified_candidate.semantic_digest() {
            return Err(DependencyReceiptCompositionError::ReceiptMismatch {
                field: "gated candidate digest",
                actual: gating.certified_candidate.semantic_digest().as_str().into(),
                expected: weakened.semantic_digest().as_str().into(),
            });
        }
        weakened.semantic_digest().as_str().to_owned()
    };
    let checks = [
        (
            "semantic digest",
            receipt.semantic_digest().as_str(),
            certified_digest.as_str(),
        ),
        (
            "binding semantic digest",
            bindings.semantic_digest.as_str(),
            certified_digest.as_str(),
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
    // Reads the *dependency's* claim, which is a different field from the
    // parent's claim the census lookups use.
    //
    // They used to be one field, and that was the defect measured in § 44-45
    // of `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`.
    // Demand planning fills a `DependencyClosure` subject's claim id from the
    // *parent's* own proposal over the parent's own closure candidate, which
    // is what `creates_census` and `dependency_creates_claims` both want --
    // and this condition then asked the *dependency's* receipt to contain it.
    // `NormalizedContract::claim_id` digests package identity, so that
    // comparison had no satisfying assignment: every closure candidate on a
    // node with a dependency was refused here, and the refusal named the
    // parent's own claim as the dependency claim it was waiting for.
    //
    // Splitting the field is what makes each reader's meaning explicit.
    // Planning sets no dependency claim, so it does not reach this condition
    // -- and nothing is lost by that, which is a fact about the domains
    // rather than a concession. `creates` is the one domain whose census
    // follows callees, so it is the one whose closure a dependency can
    // contradict, and its dependency claims are named exactly by
    // `dependency_creates_claims` and checked against this same receipt by
    // the caller. `reads` and `returns` have no callee walk by construction
    // (`census_reads_domain`, `census_returns_domain`): a `reads` claim is
    // about accesses in the export's own body, and a read reached through a
    // caller-supplied value is the caller's (ADR 0034), so a dependency's own
    // reads cannot contradict it. There is no dependency claim for those
    // domains to name, because the semantics create none.
    if let Some(semantic_claim_id) = requirement.dependency_semantic_claim_id()
        && !receipt.contains_closed_claim_id(semantic_claim_id)
        && independent_creates_census.is_none()
    {
        return Err(DependencyReceiptCompositionError::MissingClosedClaim {
            demand_id: requirement.demand_id().into(),
            semantic_claim_id: semantic_claim_id.into(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn authenticate_dependency_claim_for_test(
    parent: &CertificationPlan,
    requirement: &DependencyCompositionRequirement,
    dependency: &CanonicalDependencyNodeIdentity,
    dependency_plan: &CertificationPlan,
    receipt: &AuthenticatedPolicy2Receipt,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
    semantic_claim_id: &str,
) -> Result<(), DependencyReceiptCompositionError> {
    let mut requirement = requirement.clone();
    requirement.parent_export = Some("test-parent".into());
    requirement.dependency_semantic_claim_id = Some(semantic_claim_id.into());
    authenticate_dependency_receipt(
        parent,
        &requirement,
        dependency,
        &DependencyGating {
            accepted_candidate: &dependency_plan.selected_candidate,
            certified_candidate: &dependency_plan.selected_candidate,
            withheld: &[],
        },
        receipt,
        issuer,
        revocation_epoch,
        None,
    )
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
    #[error(
        "dependency demand {demand_id} requires closed semantic claim {semantic_claim_id}, which the authenticated dependency contract does not contain"
    )]
    MissingClosedClaim {
        demand_id: String,
        semantic_claim_id: String,
    },
    #[error("dependency receipts disagree on verifier build identity")]
    VerifierBuildDisagreement,
    #[error("dependency composition evidence was transplanted to another parent plan")]
    ParentTransplant,
    #[error("canonical dependency node {dependency} is outside the planned graph")]
    DependencyOutsideGraph { dependency: String },
    #[error(
        "the withheld closures of dependency node {dependency} do not re-derive a weakening of its accepted proposal: {reason}"
    )]
    WithheldWeakening { dependency: String, reason: String },
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

    /// A node identity whose digest is computed the way production computes
    /// it, over every field including the absolute `importer`. The digest is
    /// what the old order tie-broke on, so a plaintext stand-in whose salt is
    /// a shared prefix would make the digest order degenerate to the field
    /// order and the salt-independence test vacuous.
    fn order_identity(
        package: &str,
        version: &str,
        entrypoint: &str,
        conditions: &[&str],
        runtime_target: &str,
        importer: &str,
    ) -> CanonicalDependencyNodeIdentity {
        let mut identity = CanonicalDependencyNodeIdentity {
            registry_origin: "https://registry.npmjs.org".into(),
            package_manager: "bun".into(),
            package_name: package.into(),
            package_version: version.into(),
            integrity: "sha512-test".into(),
            lockfile_digest: "sha256:lock".into(),
            lock_locator: format!("{package}@{version}"),
            entrypoint: entrypoint.into(),
            conditions: conditions.iter().map(|value| (*value).to_owned()).collect(),
            // The one path-salted field the old order tie-broke on.
            importer: importer.into(),
            resolution_kind: "Exports".into(),
            runtime_target: runtime_target.into(),
            runtime_digest: "sha256:runtime".into(),
            declarations_target: "build/index.d.ts".into(),
            declarations_digest: "sha256:declarations".into(),
            closure_root: "sha256:closure".into(),
            resolved_import_root: format!("sha256:{importer}"),
            snapshot_root: "sha256:snapshot".into(),
            provenance_root: "sha256:provenance".into(),
            artifact_case: "artifact-case:test".into(),
            semantic_digest: "sha256:semantic".into(),
            source_dependencies_root: "sha256:sources".into(),
            digest: String::new(),
        };
        identity.digest = node_identity_digest(&identity);
        identity
    }

    #[test]
    fn type_facts_request_order_is_package_coordinate_first_and_digest_last() {
        let left = order_identity("@scope/a", "2.0.0", ".", &["import"], "build/index.js", "z");
        let right = order_identity("@scope/b", "1.0.0", ".", &["import"], "build/index.js", "a");
        assert!(
            compare_type_facts_request_coordinates(order_key(&left), order_key(&right)).is_lt()
        );
        let left = order_identity("pkg", "1.0.0", "./a", &["import"], "build/a.js", "z");
        let right = order_identity("pkg", "1.0.0", "./b", &["import"], "build/b.js", "a");
        assert!(
            compare_type_facts_request_coordinates(order_key(&left), order_key(&right)).is_lt()
        );
        let left = order_identity("pkg", "1.0.0", ".", &["import"], "build/index.js", "z");
        let right = order_identity("pkg", "2.0.0", ".", &["import"], "build/index.js", "a");
        assert!(
            compare_type_facts_request_coordinates(order_key(&left), order_key(&right)).is_lt()
        );
        // Fewest conditions first, and only then the condition list itself.
        // `["import"]` is what an ordinary consumer selects; every opt-in case
        // is a superset of it, and several of those sort *before* it
        // lexicographically.
        let plain = order_identity("pkg", "1.0.0", ".", &["import"], "build/index.js", "z");
        for extra in ["@scope/private-condition", "development", "solid", "zzz"] {
            let opt_in = order_identity(
                "pkg",
                "1.0.0",
                ".",
                &sorted_conditions(&["import", extra]),
                "src/index.ts",
                "a",
            );
            assert!(
                compare_type_facts_request_coordinates(order_key(&plain), order_key(&opt_in))
                    .is_lt(),
                "the unconditional case must precede an opt-in case adding {extra:?}"
            );
        }
        let left = order_identity("pkg", "1.0.0", ".", &["a", "import"], "build/index.js", "z");
        let right = order_identity("pkg", "1.0.0", ".", &["b", "import"], "build/index.js", "a");
        assert!(
            compare_type_facts_request_coordinates(order_key(&left), order_key(&right)).is_lt()
        );
    }

    fn sorted_conditions<'a>(conditions: &[&'a str]) -> Vec<&'a str> {
        let mut sorted = conditions.to_vec();
        sorted.sort_unstable();
        sorted
    }

    fn order_key(identity: &CanonicalDependencyNodeIdentity) -> TypeFactsRequestOrderKey<'_> {
        type_facts_request_order_key(identity)
    }

    /// The regression: alternative artifact cases of one package share name,
    /// version and entrypoint, and their canonical identity digests are salted
    /// by the absolute installed paths of the run. Deriving the order over
    /// every input permutation, under two different salts, must give one
    /// answer — with the unconditional case, the runtime an ordinary consumer
    /// resolves, first and the publisher-private TypeScript source last.
    ///
    /// This fails on both of the orders it replaces: on the digest tie-break
    /// (the answer changes with the salt) and on the bare condition-list
    /// tie-break (`src/index.ts` first, `build/modern/index.js` last).
    #[test]
    fn type_facts_request_order_of_alternative_cases_is_independent_of_path_salt() {
        let case = |conditions: &[&str], runtime_target: &str, salt: &str| {
            order_identity(
                "@tanstack/query-persist-client-core",
                "5.102.5",
                ".",
                &sorted_conditions(conditions),
                runtime_target,
                salt,
            )
        };
        let derive = |salt: &str, permutation: [usize; 3]| {
            let published = [
                (
                    ["@tanstack/custom-condition", "import"].as_slice(),
                    "src/index.ts",
                ),
                (["development", "import"].as_slice(), "build/dev.js"),
                (["import"].as_slice(), "build/modern/index.js"),
            ];
            let mut cases = permutation
                .iter()
                .map(|index| {
                    let (conditions, runtime_target) = published[*index];
                    case(conditions, runtime_target, salt)
                })
                .collect::<Vec<_>>();
            cases.sort_by(|left, right| {
                compare_type_facts_request_coordinates(order_key(left), order_key(right))
            });
            cases
                .into_iter()
                .map(|identity| identity.runtime_target)
                .collect::<Vec<_>>()
        };
        let expected = vec![
            "build/modern/index.js".to_owned(),
            "src/index.ts".to_owned(),
            "build/dev.js".to_owned(),
        ];
        // Every permutation of the three cases, under two salts that differ
        // the way two runs' temporary install roots differ.
        for permutation in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            for salt in [
                "/private/tmp/solid-checker-ecosystem-aaaaaa/node_modules",
                "/private/tmp/solid-checker-ecosystem-zzzzzz/node_modules",
            ] {
                assert_eq!(
                    derive(salt, permutation),
                    expected,
                    "{permutation:?} {salt}"
                );
            }
        }
    }

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
