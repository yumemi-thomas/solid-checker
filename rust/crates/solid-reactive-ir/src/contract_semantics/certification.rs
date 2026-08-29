//! Policy-owned package-contract certification.

use std::{collections::BTreeSet, sync::LazyLock};

use serde::Serialize;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::{
    CallSemantics, ContractProposal, Digest, EdgeKind, ModelError, NormalizedContract,
    ResourceKind, SEMANTIC_MODEL_VERSION, SemanticClaimPath, SemanticClaimSubject, ValuePath,
    ValuePathSegment, ValueRoot, ValueShape,
};

const POLICY_VERSION: u32 = 2;
const PROOF_VERSION: u16 = 2;
const RECEIPT_VERSION: u16 = 2;
const POLICY_DIGEST_DOMAIN: &str = "solid-checker:contract-proof-policy:v2";

/// Active policy-2 authority for package-contract certification and loading.
pub struct ProofPolicy2;

/// Archive budgets consumed by the backend snapshot boundary. These values
/// come from the same Rust-owned policy definition that renders the audit
/// manifest; adapters must not load replacement limits from JSON.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSnapshotLimits {
    pub registry_metadata_bytes: usize,
    pub archive_bytes: usize,
    pub expanded_archive_bytes: usize,
    pub archive_members: usize,
    pub package_path_bytes: usize,
}

/// Verifier-derived facts that must survive certification planning.
///
/// The source candidate list is deliberately not an argument. This typestate
/// is constructed only by walking the normalized candidate, so a proof
/// document cannot omit a proposed closure or positive operation from the
/// planner's universe.
pub struct CertificationCandidates {
    candidate_semantic_digest: Digest,
    proposal: NormalizedContract,
    closure_candidates: Vec<SemanticClaimSubject>,
    positive_operations: Vec<SemanticClaimSubject>,
    positive_facts: Vec<PositiveFactSubject>,
}

/// Exact analyzer-visible positive facts retained when proposed completeness
/// is withdrawn. Each variant has a distinct proof subject; no generic claim
/// ID can be reused to stand for a structurally different fact.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PositiveFactSubject {
    SelectedCall {
        artifact_case: String,
        export: String,
    },
    CallbackBinding {
        artifact_case: String,
        export: String,
        ordinal: u32,
        operation: String,
    },
    Operation {
        artifact_case: String,
        export: String,
        operation: String,
        has_cardinality: bool,
    },
    OperationEdge {
        artifact_case: String,
        export: String,
        kind: String,
        from: String,
        to: String,
    },
    Resource {
        artifact_case: String,
        export: String,
        resource: String,
        kind: String,
    },
    GuardCase {
        artifact_case: String,
        export: String,
        ordinal: u32,
    },
    RecursiveValue {
        artifact_case: String,
        export: String,
        root: ValueRoot,
        path: ValuePath,
        callable: bool,
    },
}

impl CertificationCandidates {
    #[must_use]
    pub const fn candidate_semantic_digest(&self) -> &Digest {
        &self.candidate_semantic_digest
    }

    #[must_use]
    pub const fn proposal(&self) -> &NormalizedContract {
        &self.proposal
    }

    #[must_use]
    pub fn closure_candidates(&self) -> &[SemanticClaimSubject] {
        &self.closure_candidates
    }

    #[must_use]
    pub fn positive_operations(&self) -> &[SemanticClaimSubject] {
        &self.positive_operations
    }

    #[must_use]
    pub fn positive_facts(&self) -> &[PositiveFactSubject] {
        &self.positive_facts
    }
}

static POLICY: ProofPolicy2 = ProofPolicy2;
static CANONICAL_MANIFEST: LazyLock<Vec<u8>> = LazyLock::new(|| {
    serde_json::to_vec(&manifest()).expect("the static proof-policy manifest must serialize")
});
static AUDIT_MANIFEST: LazyLock<Vec<u8>> = LazyLock::new(|| {
    serde_json::to_vec_pretty(&manifest())
        .expect("the static proof-policy audit manifest must serialize")
});
static POLICY_DIGEST: LazyLock<Digest> = LazyLock::new(|| {
    let manifest = CANONICAL_MANIFEST.as_slice();
    let mut hash = Sha256::new();
    hash.update((POLICY_DIGEST_DOMAIN.len() as u64).to_be_bytes());
    hash.update(POLICY_DIGEST_DOMAIN.as_bytes());
    hash.update((manifest.len() as u64).to_be_bytes());
    hash.update(manifest);
    Digest::from_sha256(hash.finalize().into())
});

#[must_use]
pub fn proof_policy_2() -> &'static ProofPolicy2 {
    &POLICY
}

impl ProofPolicy2 {
    #[must_use]
    pub const fn policy_version(&self) -> u32 {
        POLICY_VERSION
    }

    #[must_use]
    pub const fn proof_version(&self) -> u16 {
        PROOF_VERSION
    }

    #[must_use]
    pub const fn receipt_version(&self) -> u16 {
        RECEIPT_VERSION
    }

    #[must_use]
    pub const fn semantic_model_version(&self) -> u16 {
        SEMANTIC_MODEL_VERSION
    }

    /// Canonical bytes hashed into every policy-2 demand and receipt.
    ///
    /// The checked-in JSON is an audit rendering of these Rust-owned values;
    /// it is never loaded as runtime policy.
    #[must_use]
    pub fn canonical_manifest(&self) -> &'static [u8] {
        CANONICAL_MANIFEST.as_slice()
    }

    /// Human-readable rendering generated from the same Rust-owned policy.
    #[must_use]
    pub fn audit_manifest(&self) -> &'static [u8] {
        AUDIT_MANIFEST.as_slice()
    }

    #[must_use]
    pub fn digest(&self) -> &'static Digest {
        &POLICY_DIGEST
    }

    #[must_use]
    pub const fn artifact_snapshot_limits(&self) -> ArtifactSnapshotLimits {
        ArtifactSnapshotLimits {
            registry_metadata_bytes: 16 * 1024 * 1024,
            archive_bytes: 256 * 1024 * 1024,
            expanded_archive_bytes: 1024 * 1024 * 1024,
            archive_members: 100_000,
            package_path_bytes: 4 * 1024,
        }
    }

    #[must_use]
    pub const fn demand_limit(&self) -> usize {
        65_536
    }

    #[must_use]
    pub const fn witness_items_per_demand_limit(&self) -> usize {
        16_384
    }

    #[must_use]
    pub const fn proof_document_bytes_limit(&self) -> usize {
        16 * 1024 * 1024
    }

    #[must_use]
    pub const fn proof_json_depth_limit(&self) -> usize {
        128
    }

    #[must_use]
    pub const fn proof_json_nodes_limit(&self) -> usize {
        1_000_000
    }

    #[must_use]
    pub const fn proof_string_bytes_limit(&self) -> usize {
        16 * 1024
    }

    /// Rebuilds the certification candidate universe from normalized meaning.
    ///
    /// Closure is withdrawn in the returned proposal; known positive
    /// operations remain present and independently inventoried for later
    /// witness demands.
    pub fn inspect_candidates(
        &self,
        candidate: &NormalizedContract,
    ) -> Result<CertificationCandidates, ModelError> {
        let package = candidate.package().clone();
        let mut artifact_cases = candidate.artifact_cases().to_vec();
        let mut closure_candidates = Vec::new();
        let mut positive_operations = Vec::new();
        let mut positive_facts = Vec::new();

        for artifact in &mut artifact_cases {
            for (export_name, export) in &mut artifact.exports {
                inventory_export_facts(
                    &artifact.id,
                    export_name,
                    &export.shape,
                    &export.call,
                    &mut positive_facts,
                );
                positive_operations.extend(export.call.operations.iter().map(|operation| {
                    SemanticClaimSubject {
                        artifact_case: artifact.id.clone(),
                        export: export_name.clone(),
                        path: SemanticClaimPath::Operation(operation.id.clone()),
                    }
                }));
                closure_candidates.extend(export.open_proposed_closure().into_iter().map(|path| {
                    SemanticClaimSubject {
                        artifact_case: artifact.id.clone(),
                        export: export_name.clone(),
                        path: SemanticClaimPath::Domain(path),
                    }
                }));
            }
        }
        closure_candidates.sort();
        closure_candidates.dedup();
        positive_operations.sort();
        positive_operations.dedup();
        positive_facts.sort();
        positive_facts.dedup();

        Ok(CertificationCandidates {
            candidate_semantic_digest: candidate.semantic_digest().clone(),
            proposal: ContractProposal::new(package, artifact_cases).normalize()?,
            closure_candidates,
            positive_operations,
            positive_facts,
        })
    }

    pub fn derive_demand_graph(
        &self,
        candidates: &CertificationCandidates,
        snapshot_root: &str,
        provenance_root: &str,
    ) -> Result<ProofDemandGraph, DemandPlanningError> {
        self.derive_demand_graph_with_dependencies(
            candidates,
            snapshot_root,
            provenance_root,
            std::iter::empty(),
        )
    }

    /// Derives the same verifier-owned graph plus one dependency-composition
    /// demand for every exact external edge and proposed parent closure.
    ///
    /// The backend supplies edges only after replaying the immutable module
    /// closure. This input is structural planning material, not receipt
    /// authority; the family verifier must still authenticate each policy-2
    /// dependency receipt before constructing a witness.
    pub fn derive_demand_graph_with_dependencies(
        &self,
        candidates: &CertificationCandidates,
        snapshot_root: &str,
        provenance_root: &str,
        dependencies: impl IntoIterator<Item = DependencyDemandInput>,
    ) -> Result<ProofDemandGraph, DemandPlanningError> {
        let snapshot_root =
            Digest::parse(snapshot_root).map_err(|_| DemandPlanningError::InvalidArtifactRoot)?;
        let provenance_root =
            Digest::parse(provenance_root).map_err(|_| DemandPlanningError::InvalidArtifactRoot)?;
        let mut requested = BTreeSet::<(ProofFamily, ProofDemandSubject)>::new();
        for artifact in candidates.proposal.artifact_cases() {
            for rule in ARTIFACT_PREREQUISITES {
                requested.insert((
                    rule.family,
                    ProofDemandSubject::ArtifactCase(artifact.id.clone()),
                ));
            }
        }
        for positive in &candidates.positive_facts {
            match positive {
                PositiveFactSubject::SelectedCall { .. } => {
                    insert_positive(&mut requested, ProofFamily::SelectedSignature, positive);
                }
                PositiveFactSubject::CallbackBinding { .. } => {
                    insert_positive(&mut requested, ProofFamily::ArgumentBinding, positive);
                    insert_positive(&mut requested, ProofFamily::CallablePath, positive);
                }
                PositiveFactSubject::Operation {
                    has_cardinality, ..
                } => {
                    insert_positive(&mut requested, ProofFamily::OperationReachability, positive);
                    if *has_cardinality {
                        insert_positive(
                            &mut requested,
                            ProofFamily::OperationCardinality,
                            positive,
                        );
                    }
                }
                PositiveFactSubject::OperationEdge { .. } => {
                    insert_positive(&mut requested, ProofFamily::OperationReachability, positive)
                }
                PositiveFactSubject::Resource { .. } => {
                    insert_positive(&mut requested, ProofFamily::RecursiveValueShape, positive)
                }
                PositiveFactSubject::GuardCase { .. } => {
                    insert_positive(&mut requested, ProofFamily::GuardPartition, positive);
                }
                PositiveFactSubject::RecursiveValue { callable, .. } => {
                    insert_positive(&mut requested, ProofFamily::RecursiveValueShape, positive);
                    if *callable {
                        insert_positive(&mut requested, ProofFamily::CallablePath, positive);
                    }
                }
            }
        }
        for closure in &candidates.closure_candidates {
            let semantic_claim_id = candidates
                .proposal
                .claim_id(closure)
                .map_err(|_| DemandPlanningError::InvalidCandidate)?;
            requested.insert((
                ProofFamily::DomainExhaustiveness,
                ProofDemandSubject::DomainClosure {
                    subject: closure.clone(),
                    semantic_claim_id: semantic_claim_id.as_str().into(),
                },
            ));
        }
        let dependencies = dependencies
            .into_iter()
            .map(DependencyDemandInput::validate)
            .collect::<Result<BTreeSet<_>, _>>()?;
        for dependency in dependencies {
            for closure in &candidates.closure_candidates {
                let semantic_claim_id = candidates
                    .proposal
                    .claim_id(closure)
                    .map_err(|_| DemandPlanningError::InvalidCandidate)?;
                requested.insert((
                    ProofFamily::AcceptedDependencyComposition,
                    ProofDemandSubject::DependencyClosure {
                        dependency: dependency.clone(),
                        parent: closure.clone(),
                        semantic_claim_id: semantic_claim_id.as_str().into(),
                    },
                ));
            }
        }
        if requested.len() > self.demand_limit() {
            return Err(DemandPlanningError::DemandLimit);
        }
        let policy_digest = self.digest().clone();
        let candidate_semantic_digest = candidates.candidate_semantic_digest.clone();
        let mut demands = requested
            .into_iter()
            .map(|(family, subject)| ProofDemand {
                id: demand_id(
                    &policy_digest,
                    &candidate_semantic_digest,
                    &snapshot_root,
                    &provenance_root,
                    family,
                    &subject,
                ),
                family,
                subject,
            })
            .collect::<Vec<_>>();
        demands.sort_by(|left, right| left.id.cmp(&right.id));
        if demands
            .windows(2)
            .any(|pair| pair[0].id == pair[1].id && pair[0] != pair[1])
        {
            return Err(DemandPlanningError::DemandIdCollision);
        }
        let root = demand_graph_root(&demands);
        Ok(ProofDemandGraph {
            policy_digest,
            candidate_semantic_digest,
            snapshot_root,
            provenance_root,
            demands,
            root,
        })
    }
}

fn insert_positive(
    demands: &mut BTreeSet<(ProofFamily, ProofDemandSubject)>,
    family: ProofFamily,
    positive: &PositiveFactSubject,
) {
    demands.insert((family, ProofDemandSubject::PositiveFact(positive.clone())));
}

fn inventory_export_facts(
    artifact_case: &str,
    export: &str,
    shape: &ValueShape,
    call: &CallSemantics,
    facts: &mut Vec<PositiveFactSubject>,
) {
    let has_call_facts = !call.operations.is_empty()
        || !call.edges.is_empty()
        || !call.resources.is_empty()
        || !call.claims().callbacks.items().is_empty()
        || !call.guards.cases.items().is_empty();
    if has_call_facts {
        facts.push(PositiveFactSubject::SelectedCall {
            artifact_case: artifact_case.into(),
            export: export.into(),
        });
    }
    for (ordinal, callback) in call.claims().callbacks.items().iter().enumerate() {
        facts.push(PositiveFactSubject::CallbackBinding {
            artifact_case: artifact_case.into(),
            export: export.into(),
            ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
            operation: callback.operation.0.clone(),
        });
    }
    for operation in &call.operations {
        facts.push(PositiveFactSubject::Operation {
            artifact_case: artifact_case.into(),
            export: export.into(),
            operation: operation.id.0.clone(),
            has_cardinality: operation.cardinality.scope.is_some()
                || operation.cardinality.min.is_some()
                || operation.cardinality.max.is_some(),
        });
        for (index, input) in operation.inputs.iter().enumerate() {
            inventory_value_shape(
                artifact_case,
                export,
                &ValueRoot::OperationInput {
                    operation: operation.id.clone(),
                    index: u16::try_from(index).unwrap_or(u16::MAX),
                },
                &ValuePath::default(),
                input,
                facts,
            );
        }
        if let Some(output) = &operation.output {
            inventory_value_shape(
                artifact_case,
                export,
                &ValueRoot::OperationOutput {
                    operation: operation.id.clone(),
                },
                &ValuePath::default(),
                output,
                facts,
            );
        }
    }
    for edge in &call.edges {
        facts.push(PositiveFactSubject::OperationEdge {
            artifact_case: artifact_case.into(),
            export: export.into(),
            kind: edge_kind_name(edge.kind).into(),
            from: edge.from.0.clone(),
            to: edge.to.0.clone(),
        });
    }
    for resource in &call.resources {
        facts.push(PositiveFactSubject::Resource {
            artifact_case: artifact_case.into(),
            export: export.into(),
            resource: resource.id.0.clone(),
            kind: resource_kind_name(resource.kind).into(),
        });
    }
    for (ordinal, _) in call.guards.cases.items().iter().enumerate() {
        facts.push(PositiveFactSubject::GuardCase {
            artifact_case: artifact_case.into(),
            export: export.into(),
            ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
        });
    }
    inventory_value_shape(
        artifact_case,
        export,
        &ValueRoot::Export,
        &ValuePath::default(),
        shape,
        facts,
    );
}

const fn edge_kind_name(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Orders => "orders",
        EdgeKind::Data => "data",
        EdgeKind::Invalidates => "invalidates",
        EdgeKind::Error => "error",
        EdgeKind::Cleanup => "cleanup",
        EdgeKind::Lifetime => "lifetime",
    }
}

const fn resource_kind_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Owner => "owner",
        ResourceKind::ReactiveSource => "reactive-source",
        ResourceKind::AsyncComputation => "async-computation",
        ResourceKind::Transition => "transition",
        ResourceKind::Cleanup => "cleanup",
        ResourceKind::Request => "request",
        ResourceKind::Response => "response",
        ResourceKind::Stream => "stream",
        ResourceKind::ServerFunctionReference => "server-function-reference",
    }
}

fn inventory_value_shape(
    artifact_case: &str,
    export: &str,
    root: &ValueRoot,
    path: &ValuePath,
    shape: &ValueShape,
    facts: &mut Vec<PositiveFactSubject>,
) {
    if matches!(shape, ValueShape::Unknown) {
        return;
    }
    facts.push(PositiveFactSubject::RecursiveValue {
        artifact_case: artifact_case.into(),
        export: export.into(),
        root: root.clone(),
        path: path.clone(),
        callable: matches!(shape, ValueShape::Callable),
    });
    match shape {
        ValueShape::Tuple(items) | ValueShape::Choice(items) => {
            for (index, item) in items.items().iter().enumerate() {
                let segment = if matches!(shape, ValueShape::Tuple(_)) {
                    ValuePathSegment::TupleItem(u32::try_from(index).unwrap_or(u32::MAX))
                } else {
                    ValuePathSegment::ChoiceAlternative(u32::try_from(index).unwrap_or(u32::MAX))
                };
                inventory_value_child(artifact_case, export, root, path, segment, item, facts);
            }
        }
        ValueShape::Array { element, .. } => inventory_value_child(
            artifact_case,
            export,
            root,
            path,
            ValuePathSegment::ArrayElement,
            element,
            facts,
        ),
        ValueShape::Object(properties) => {
            for property in properties.items() {
                inventory_value_child(
                    artifact_case,
                    export,
                    root,
                    path,
                    ValuePathSegment::ObjectProperty(property.name.clone()),
                    &property.value,
                    facts,
                );
            }
        }
        ValueShape::Promise(value) => inventory_value_child(
            artifact_case,
            export,
            root,
            path,
            ValuePathSegment::PromiseValue,
            value,
            facts,
        ),
        ValueShape::AsyncIterable(value) => inventory_value_child(
            artifact_case,
            export,
            root,
            path,
            ValuePathSegment::AsyncIterableElement,
            value,
            facts,
        ),
        ValueShape::Unknown
        | ValueShape::Plain
        | ValueShape::Parameter { .. }
        | ValueShape::Callable
        | ValueShape::Reactive { .. }
        | ValueShape::Store { .. }
        | ValueShape::Action { .. }
        | ValueShape::Component
        | ValueShape::Cleanup { .. }
        | ValueShape::RefApplication
        | ValueShape::ServerFunctionReference { .. } => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn inventory_value_child(
    artifact_case: &str,
    export: &str,
    root: &ValueRoot,
    path: &ValuePath,
    segment: ValuePathSegment,
    shape: &ValueShape,
    facts: &mut Vec<PositiveFactSubject>,
) {
    let mut child = path.clone();
    child.0.push(segment);
    inventory_value_shape(artifact_case, export, root, &child, shape, facts);
}

fn demand_id(
    policy_digest: &Digest,
    candidate_digest: &Digest,
    snapshot_root: &Digest,
    provenance_root: &Digest,
    family: ProofFamily,
    subject: &ProofDemandSubject,
) -> ProofDemandId {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:contract-proof-demand:v2");
    hash_demand_text(&mut hash, policy_digest.as_str());
    hash_demand_text(&mut hash, candidate_digest.as_str());
    hash_demand_text(&mut hash, snapshot_root.as_str());
    hash_demand_text(&mut hash, provenance_root.as_str());
    hash_demand_text(&mut hash, proof_family_name(family));
    hash_demand_subject(&mut hash, subject);
    ProofDemandId(Digest::from_sha256(hash.finalize().into()))
}

fn demand_graph_root(demands: &[ProofDemand]) -> Digest {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:contract-proof-demand-graph:v2");
    hash.update(
        u64::try_from(demands.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for demand in demands {
        hash_demand_text(&mut hash, demand.id.as_str());
    }
    Digest::from_sha256(hash.finalize().into())
}

fn hash_demand_subject(hash: &mut Sha256, subject: &ProofDemandSubject) {
    match subject {
        ProofDemandSubject::ArtifactCase(artifact_case) => {
            hash_demand_text(hash, "artifact-case");
            hash_demand_text(hash, artifact_case);
        }
        ProofDemandSubject::PositiveFact(positive) => {
            hash_demand_text(hash, "positive-fact");
            hash_positive_subject(hash, positive);
        }
        ProofDemandSubject::DomainClosure {
            semantic_claim_id, ..
        } => {
            hash_demand_text(hash, "domain-closure");
            hash_demand_text(hash, semantic_claim_id);
        }
        ProofDemandSubject::DependencyClosure {
            dependency,
            semantic_claim_id,
            ..
        } => {
            hash_demand_text(hash, "dependency-closure");
            hash_demand_text(hash, &dependency.package);
            hash_demand_text(hash, &dependency.artifact_case);
            hash_demand_text(hash, &dependency.specifier);
            hash_demand_text(hash, &dependency.accepted_contract_digest);
            hash_demand_text(hash, semantic_claim_id);
        }
    }
}

fn hash_positive_subject(hash: &mut Sha256, subject: &PositiveFactSubject) {
    match subject {
        PositiveFactSubject::SelectedCall {
            artifact_case,
            export,
        } => {
            hash_demand_text(hash, "selected-call");
            hash_artifact_export(hash, artifact_case, export);
        }
        PositiveFactSubject::CallbackBinding {
            artifact_case,
            export,
            ordinal,
            operation,
        } => {
            hash_demand_text(hash, "callback-binding");
            hash_artifact_export(hash, artifact_case, export);
            hash.update(ordinal.to_be_bytes());
            hash_demand_text(hash, operation);
        }
        PositiveFactSubject::Operation {
            artifact_case,
            export,
            operation,
            has_cardinality,
        } => {
            hash_demand_text(hash, "operation");
            hash_artifact_export(hash, artifact_case, export);
            hash_demand_text(hash, operation);
            hash.update([u8::from(*has_cardinality)]);
        }
        PositiveFactSubject::OperationEdge {
            artifact_case,
            export,
            kind,
            from,
            to,
        } => {
            hash_demand_text(hash, "operation-edge");
            hash_artifact_export(hash, artifact_case, export);
            hash_demand_text(hash, kind);
            hash_demand_text(hash, from);
            hash_demand_text(hash, to);
        }
        PositiveFactSubject::Resource {
            artifact_case,
            export,
            resource,
            kind,
        } => {
            hash_demand_text(hash, "resource");
            hash_artifact_export(hash, artifact_case, export);
            hash_demand_text(hash, resource);
            hash_demand_text(hash, kind);
        }
        PositiveFactSubject::GuardCase {
            artifact_case,
            export,
            ordinal,
        } => {
            hash_demand_text(hash, "guard-case");
            hash_artifact_export(hash, artifact_case, export);
            hash.update(ordinal.to_be_bytes());
        }
        PositiveFactSubject::RecursiveValue {
            artifact_case,
            export,
            root,
            path,
            callable,
        } => {
            hash_demand_text(hash, "recursive-value");
            hash_artifact_export(hash, artifact_case, export);
            hash_value_root(hash, root);
            hash_value_path(hash, path);
            hash.update([u8::from(*callable)]);
        }
    }
}

fn hash_artifact_export(hash: &mut Sha256, artifact_case: &str, export: &str) {
    hash_demand_text(hash, artifact_case);
    hash_demand_text(hash, export);
}

fn hash_value_root(hash: &mut Sha256, root: &ValueRoot) {
    match root {
        ValueRoot::Export => hash_demand_text(hash, "export"),
        ValueRoot::OperationInput { operation, index } => {
            hash_demand_text(hash, "operation-input");
            hash_demand_text(hash, &operation.0);
            hash.update(index.to_be_bytes());
        }
        ValueRoot::OperationOutput { operation } => {
            hash_demand_text(hash, "operation-output");
            hash_demand_text(hash, &operation.0);
        }
    }
}

fn hash_value_path(hash: &mut Sha256, path: &ValuePath) {
    hash.update(
        u64::try_from(path.0.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for segment in &path.0 {
        match segment {
            ValuePathSegment::TupleItem(index) => {
                hash_demand_text(hash, "tuple-item");
                hash.update(index.to_be_bytes());
            }
            ValuePathSegment::ArrayElement => hash_demand_text(hash, "array-element"),
            ValuePathSegment::ObjectProperty(name) => {
                hash_demand_text(hash, "object-property");
                hash_demand_text(hash, name);
            }
            ValuePathSegment::ChoiceAlternative(index) => {
                hash_demand_text(hash, "choice-alternative");
                hash.update(index.to_be_bytes());
            }
            ValuePathSegment::PromiseValue => hash_demand_text(hash, "promise-value"),
            ValuePathSegment::AsyncIterableElement => {
                hash_demand_text(hash, "async-iterable-element");
            }
        }
    }
}

fn hash_demand_text(hash: &mut Sha256, value: &str) {
    hash.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hash.update(value.as_bytes());
}

const fn proof_family_name(family: ProofFamily) -> &'static str {
    match family {
        ProofFamily::PackageIdentity => "package-identity",
        ProofFamily::ManifestEntrypoint => "manifest-entrypoint",
        ProofFamily::ExportResolution => "export-resolution",
        ProofFamily::ArtifactDeclarations => "artifact-declarations",
        ProofFamily::ExportIdentity => "export-identity",
        ProofFamily::ModuleClosure => "module-closure",
        ProofFamily::SelectedSignature => "selected-signature",
        ProofFamily::ArgumentBinding => "argument-binding",
        ProofFamily::RestSpreadCoverage => "rest-spread-coverage",
        ProofFamily::CallablePath => "callable-path",
        ProofFamily::OperationReachability => "operation-reachability",
        ProofFamily::OperationCardinality => "operation-cardinality",
        ProofFamily::RecursiveValueShape => "recursive-value-shape",
        ProofFamily::GuardPartition => "guard-partition",
        ProofFamily::CompilerReconciliation => "compiler-reconciliation",
        ProofFamily::AcceptedDependencyComposition => "accepted-dependency-composition",
        ProofFamily::DomainExhaustiveness => "domain-exhaustiveness",
        ProofFamily::ProbeConsistency => "probe-consistency",
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyManifest {
    format: &'static str,
    policy_version: u32,
    proof_version: u16,
    receipt_version: u16,
    semantic_model_version: u16,
    status: &'static str,
    canonical_encoding: CanonicalEncoding,
    digests: &'static [DigestRule],
    coverage: Coverage,
    witness_envelope: WitnessEnvelope,
    applicability: Applicability,
    producer_constraints: ProducerConstraints,
    resource_budgets: ResourceBudgets,
    receipt_trust: ReceiptTrust,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalEncoding {
    name: &'static str,
    version: u16,
}

#[derive(Serialize)]
struct DigestRule {
    purpose: &'static str,
    algorithm: &'static str,
    domain: &'static str,
    framing: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Coverage {
    positive_facts: &'static str,
    closures: &'static str,
    inapplicability: &'static str,
    unproved_positive: &'static str,
    unproved_closure: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WitnessEnvelope {
    variants: &'static [&'static str],
    exactly_one_per_demand: bool,
    concrete_site_identities_required: bool,
    empty_witness_allowed: bool,
    duplicate_sites_allowed: bool,
    unknown_variants_allowed: bool,
    serialized_authority: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Applicability {
    artifact_prerequisites: &'static [DemandRule],
    claim_families: &'static [DemandRule],
    compiler_reconciliation: DemandRule,
    dependency_composition: DemandRule,
    probe_consistency: ProbeRule,
}

#[derive(Clone, Copy, Serialize)]
struct DemandRule {
    family: ProofFamily,
    authority: ProofAuthority,
    #[serde(rename = "appliesTo")]
    applies_to: DemandTarget,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeRule {
    family: ProofFamily,
    authority: ProofAuthority,
    mode: &'static str,
    applies_to: DemandTarget,
    missing_required_probe: &'static str,
    absence_proves_negative: bool,
    successful_observation: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProofFamily {
    PackageIdentity,
    ManifestEntrypoint,
    ExportResolution,
    ArtifactDeclarations,
    ExportIdentity,
    ModuleClosure,
    SelectedSignature,
    ArgumentBinding,
    RestSpreadCoverage,
    CallablePath,
    OperationReachability,
    OperationCardinality,
    RecursiveValueShape,
    GuardPartition,
    CompilerReconciliation,
    AcceptedDependencyComposition,
    DomainExhaustiveness,
    ProbeConsistency,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ProofAuthority {
    PackageArtifacts,
    TypeFacts,
    CompilerExecutionFacts,
    AcceptedDependencyContract,
    RuntimeProbe,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DemandTarget {
    ArtifactCase,
    SelectedCallSignature,
    ArgumentOrCallbackBinding,
    RestOrSpreadSite,
    CallableValuePath,
    OperationOrOperationEdge,
    OperationCardinality,
    RecursiveValueItem,
    GuardOrPartition,
    ProposedDomainClosure,
    CompilerOwnedSite,
    RelevantExternalDependencyEdge,
    VerifierScheduledProbe,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProofDemandSubject {
    ArtifactCase(String),
    PositiveFact(PositiveFactSubject),
    DomainClosure {
        subject: SemanticClaimSubject,
        semantic_claim_id: String,
    },
    DependencyClosure {
        dependency: DependencyDemandInput,
        parent: SemanticClaimSubject,
        semantic_claim_id: String,
    },
}

/// Exact replayed external edge used only to plan dependency composition.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DependencyDemandInput {
    pub specifier: String,
    pub package: String,
    pub artifact_case: String,
    pub accepted_contract_digest: String,
}

impl DependencyDemandInput {
    fn validate(self) -> Result<Self, DemandPlanningError> {
        if self.specifier.trim().is_empty()
            || self.package.trim().is_empty()
            || self.artifact_case.trim().is_empty()
            || Digest::parse(&self.accepted_contract_digest).is_err()
        {
            return Err(DemandPlanningError::InvalidDependency);
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProofDemandId(Digest);

impl ProofDemandId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProofDemand {
    id: ProofDemandId,
    family: ProofFamily,
    subject: ProofDemandSubject,
}

impl ProofDemand {
    #[must_use]
    pub const fn id(&self) -> &ProofDemandId {
        &self.id
    }

    #[must_use]
    pub const fn family(&self) -> ProofFamily {
        self.family
    }

    #[must_use]
    pub const fn subject(&self) -> &ProofDemandSubject {
        &self.subject
    }
}

/// Complete policy-2 demand universe for one candidate and immutable artifact
/// snapshot. The graph is constructed from normalized meaning; proof-wire
/// documents can reference its IDs but cannot add, remove, or reclassify them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofDemandGraph {
    policy_digest: Digest,
    candidate_semantic_digest: Digest,
    snapshot_root: Digest,
    provenance_root: Digest,
    demands: Vec<ProofDemand>,
    root: Digest,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DemandPlanningError {
    #[error("artifact snapshot or provenance root is invalid")]
    InvalidArtifactRoot,
    #[error("proof demand graph exceeds the policy limit")]
    DemandLimit,
    #[error("proof demand ID collision")]
    DemandIdCollision,
    #[error("the verifier-derived demand subject is absent from normalized meaning")]
    InvalidCandidate,
    #[error("the replayed external dependency edge is invalid")]
    InvalidDependency,
}

impl ProofDemandGraph {
    #[must_use]
    pub fn demands(&self) -> &[ProofDemand] {
        &self.demands
    }

    #[must_use]
    pub const fn root(&self) -> &Digest {
        &self.root
    }

    #[must_use]
    pub const fn policy_digest(&self) -> &Digest {
        &self.policy_digest
    }

    #[must_use]
    pub const fn candidate_semantic_digest(&self) -> &Digest {
        &self.candidate_semantic_digest
    }

    #[must_use]
    pub const fn snapshot_root(&self) -> &Digest {
        &self.snapshot_root
    }

    #[must_use]
    pub const fn provenance_root(&self) -> &Digest {
        &self.provenance_root
    }

    /// Checks the exact one-witness-per-demand envelope before any witness can
    /// be considered by family-specific semantic verifiers.
    ///
    /// This establishes structural coverage only. An evidence digest is not
    /// authority: backend adapters still have to authenticate the producing
    /// snapshot, process session, compiler run, dependency receipt, or probe
    /// transcript before constructing the family result consumed later.
    pub fn verify_witness_coverage(
        &self,
        witnesses: impl IntoIterator<Item = WitnessBinding>,
    ) -> Result<WitnessCoverage, WitnessCoverageError> {
        let demands = self
            .demands
            .iter()
            .map(|demand| (demand.id.as_str(), demand))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut covered = std::collections::BTreeMap::<String, CoveredWitness>::new();
        for witness in witnesses {
            let demand = demands
                .get(witness.demand_id.as_str())
                .ok_or(WitnessCoverageError::OrphanWitness)?;
            if demand.family != witness.variant.family() {
                return Err(WitnessCoverageError::FamilyMismatch);
            }
            if witness.site_ids.is_empty() {
                return Err(WitnessCoverageError::EmptyWitness);
            }
            if witness.site_ids.len() > proof_policy_2().witness_items_per_demand_limit() {
                return Err(WitnessCoverageError::ItemLimit);
            }
            let mut site_ids = witness.site_ids;
            if site_ids.iter().any(|site| {
                site.is_empty() || site.len() > proof_policy_2().proof_string_bytes_limit()
            }) {
                return Err(WitnessCoverageError::InvalidSiteIdentity);
            }
            site_ids.sort();
            if site_ids.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(WitnessCoverageError::DuplicateSiteIdentity);
            }
            let evidence_root = Digest::parse(&witness.evidence_root)
                .map_err(|_| WitnessCoverageError::InvalidEvidenceRoot)?;
            let id = witness.demand_id;
            if covered
                .insert(
                    id,
                    CoveredWitness {
                        family: demand.family,
                        evidence_root,
                        site_ids,
                    },
                )
                .is_some()
            {
                return Err(WitnessCoverageError::DuplicateWitness);
            }
        }
        if covered.len() != self.demands.len() {
            return Err(WitnessCoverageError::MissingWitness);
        }

        let evidence_root = witness_evidence_root(self.root(), &covered);
        Ok(WitnessCoverage {
            demand_graph_root: self.root.clone(),
            evidence_root,
            covered,
        })
    }
}

/// Closed wire-independent witness discriminator. There is intentionally no
/// `inapplicable`, `other`, or caller-defined variant.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProofWitnessVariant {
    PackageIdentity,
    ManifestEntrypoint,
    ExportResolution,
    ArtifactDeclarations,
    ExportIdentity,
    ModuleClosure,
    SelectedSignature,
    ArgumentBinding,
    RestSpreadCoverage,
    CallablePath,
    OperationReachability,
    OperationCardinality,
    RecursiveValueShape,
    GuardPartition,
    CompilerReconciliation,
    AcceptedDependencyComposition,
    DomainExhaustiveness,
}

impl ProofWitnessVariant {
    const fn family(self) -> ProofFamily {
        match self {
            Self::PackageIdentity => ProofFamily::PackageIdentity,
            Self::ManifestEntrypoint => ProofFamily::ManifestEntrypoint,
            Self::ExportResolution => ProofFamily::ExportResolution,
            Self::ArtifactDeclarations => ProofFamily::ArtifactDeclarations,
            Self::ExportIdentity => ProofFamily::ExportIdentity,
            Self::ModuleClosure => ProofFamily::ModuleClosure,
            Self::SelectedSignature => ProofFamily::SelectedSignature,
            Self::ArgumentBinding => ProofFamily::ArgumentBinding,
            Self::RestSpreadCoverage => ProofFamily::RestSpreadCoverage,
            Self::CallablePath => ProofFamily::CallablePath,
            Self::OperationReachability => ProofFamily::OperationReachability,
            Self::OperationCardinality => ProofFamily::OperationCardinality,
            Self::RecursiveValueShape => ProofFamily::RecursiveValueShape,
            Self::GuardPartition => ProofFamily::GuardPartition,
            Self::CompilerReconciliation => ProofFamily::CompilerReconciliation,
            Self::AcceptedDependencyComposition => ProofFamily::AcceptedDependencyComposition,
            Self::DomainExhaustiveness => ProofFamily::DomainExhaustiveness,
        }
    }
}

/// Non-authoritative binding decoded from proof wire or produced by an
/// adapter. Coverage validation authenticates none of its claimed evidence;
/// it only prevents demand-set substitution before the family verifier runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WitnessBinding {
    variant: ProofWitnessVariant,
    demand_id: String,
    evidence_root: String,
    site_ids: Vec<String>,
}

impl WitnessBinding {
    #[must_use]
    pub fn new(
        variant: ProofWitnessVariant,
        demand_id: impl Into<String>,
        evidence_root: impl Into<String>,
        site_ids: Vec<String>,
    ) -> Self {
        Self {
            variant,
            demand_id: demand_id.into(),
            evidence_root: evidence_root.into(),
            site_ids,
        }
    }

    #[must_use]
    pub fn demand_id(&self) -> &str {
        &self.demand_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoveredWitness {
    family: ProofFamily,
    evidence_root: Digest,
    site_ids: Vec<String>,
}

/// Exact structural coverage of a verifier-derived demand graph. This is an
/// input to family verification, never an accepted semantic verdict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WitnessCoverage {
    demand_graph_root: Digest,
    evidence_root: Digest,
    covered: std::collections::BTreeMap<String, CoveredWitness>,
}

impl WitnessCoverage {
    #[must_use]
    pub const fn demand_graph_root(&self) -> &Digest {
        &self.demand_graph_root
    }

    #[must_use]
    pub const fn evidence_root(&self) -> &Digest {
        &self.evidence_root
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.covered.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.covered.is_empty()
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WitnessCoverageError {
    #[error("proof witness names no verifier-derived demand")]
    OrphanWitness,
    #[error("proof demand has more than one witness")]
    DuplicateWitness,
    #[error("proof witness family does not match its demand")]
    FamilyMismatch,
    #[error("proof witness is empty; inapplicability is verifier-derived")]
    EmptyWitness,
    #[error("proof witness exceeds the per-demand item limit")]
    ItemLimit,
    #[error("proof witness contains an invalid site identity")]
    InvalidSiteIdentity,
    #[error("proof witness repeats a site identity")]
    DuplicateSiteIdentity,
    #[error("proof witness evidence root is invalid")]
    InvalidEvidenceRoot,
    #[error("one or more verifier-derived demands have no witness")]
    MissingWitness,
}

fn witness_evidence_root(
    graph_root: &Digest,
    covered: &std::collections::BTreeMap<String, CoveredWitness>,
) -> Digest {
    let mut hash = Sha256::new();
    hash_demand_text(&mut hash, "solid-checker:contract-proof-evidence:v2");
    hash_demand_text(&mut hash, graph_root.as_str());
    hash.update(
        u64::try_from(covered.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for (demand_id, witness) in covered {
        hash_demand_text(&mut hash, demand_id);
        hash_demand_text(&mut hash, proof_family_name(witness.family));
        hash_demand_text(&mut hash, witness.evidence_root.as_str());
        hash.update(
            u64::try_from(witness.site_ids.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        for site in &witness.site_ids {
            hash_demand_text(&mut hash, site);
        }
    }
    Digest::from_sha256(hash.finalize().into())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProducerConstraints {
    package_artifacts: ArtifactProducerConstraints,
    type_facts: ProcessProducerConstraints,
    compiler_execution_facts: ProcessProducerConstraints,
    accepted_dependency_contract: DependencyProducerConstraints,
    runtime_probe: ProbeProducerConstraints,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactProducerConstraints {
    session: &'static str,
    registry_or_lock_provenance_required: bool,
    lifecycle_scripts_allowed: bool,
    caller_serialized_authority_accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessProducerConstraints {
    session: &'static str,
    executable_digest_required: bool,
    source_manifest_digest_required: bool,
    runtime_digest_required: bool,
    build_and_protocol_identity_required: bool,
    private_execution_snapshot_required: bool,
    process_and_restart_epoch_required: bool,
    project_generation_required: bool,
    snapshot_and_demand_binding_required: bool,
    caller_serialized_authority_accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyProducerConstraints {
    receipt: &'static str,
    exact_dependency_edge_binding_required: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeProducerConstraints {
    harness: &'static str,
    harness_digest_required: bool,
    environment_identity_required: bool,
    contradiction_is_veto: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceBudgets {
    contract_document_bytes: usize,
    proof_document_bytes: usize,
    receipt_bytes: usize,
    json_depth: usize,
    json_nodes: usize,
    string_bytes: usize,
    demands: usize,
    witness_items_per_demand: usize,
    registry_metadata_bytes: usize,
    archive_bytes: usize,
    expanded_archive_bytes: usize,
    archive_members: usize,
    package_path_bytes: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptTrust {
    canonical_main_encoding_required: bool,
    policy_digest_constraint_required: bool,
    receipt_carried_key_is_trust_root: bool,
    built_in: BuiltInReceiptTrust,
    persistent_local: SignedReceiptTrust,
    portable: PortableReceiptTrust,
}

#[derive(Serialize)]
struct BuiltInReceiptTrust {
    provenance: &'static str,
    #[serde(rename = "externalSignatureRequired")]
    external_signature_required: bool,
}

#[derive(Serialize)]
struct SignedReceiptTrust {
    algorithm: &'static str,
    #[serde(rename = "configuredIssuerAndTrustRootRequired")]
    configured_issuer_and_trust_root_required: bool,
}

#[derive(Serialize)]
struct PortableReceiptTrust {
    algorithm: &'static str,
    #[serde(rename = "explicitTrustStoreChainRequired")]
    explicit_trust_store_chain_required: bool,
}

const DIGESTS: [DigestRule; 5] = [
    DigestRule {
        purpose: "policy",
        algorithm: "sha256",
        domain: POLICY_DIGEST_DOMAIN,
        framing: "u64be-length-prefixed-domain-and-payload",
    },
    DigestRule {
        purpose: "demand-id",
        algorithm: "sha256",
        domain: "solid-checker:contract-proof-demand:v2",
        framing: "typed-length-prefixed-fields",
    },
    DigestRule {
        purpose: "demand-graph",
        algorithm: "sha256",
        domain: "solid-checker:contract-proof-demand-graph:v2",
        framing: "sorted-demand-id-sequence",
    },
    DigestRule {
        purpose: "evidence-root",
        algorithm: "sha256",
        domain: "solid-checker:contract-proof-evidence:v2",
        framing: "typed-length-prefixed-fields",
    },
    DigestRule {
        purpose: "receipt-payload",
        algorithm: "sha256",
        domain: "solid-checker:acceptance-receipt:v2",
        framing: "typed-length-prefixed-fields",
    },
];

const WITNESS_VARIANTS: [&str; 17] = [
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

const ARTIFACT_PREREQUISITES: [DemandRule; 6] = [
    artifact_rule(ProofFamily::PackageIdentity),
    artifact_rule(ProofFamily::ManifestEntrypoint),
    artifact_rule(ProofFamily::ExportResolution),
    artifact_rule(ProofFamily::ArtifactDeclarations),
    artifact_rule(ProofFamily::ExportIdentity),
    artifact_rule(ProofFamily::ModuleClosure),
];

const CLAIM_FAMILIES: [DemandRule; 9] = [
    type_facts_rule(
        ProofFamily::SelectedSignature,
        DemandTarget::SelectedCallSignature,
    ),
    type_facts_rule(
        ProofFamily::ArgumentBinding,
        DemandTarget::ArgumentOrCallbackBinding,
    ),
    type_facts_rule(
        ProofFamily::RestSpreadCoverage,
        DemandTarget::RestOrSpreadSite,
    ),
    type_facts_rule(ProofFamily::CallablePath, DemandTarget::CallableValuePath),
    type_facts_rule(
        ProofFamily::OperationReachability,
        DemandTarget::OperationOrOperationEdge,
    ),
    type_facts_rule(
        ProofFamily::OperationCardinality,
        DemandTarget::OperationCardinality,
    ),
    type_facts_rule(
        ProofFamily::RecursiveValueShape,
        DemandTarget::RecursiveValueItem,
    ),
    type_facts_rule(ProofFamily::GuardPartition, DemandTarget::GuardOrPartition),
    type_facts_rule(
        ProofFamily::DomainExhaustiveness,
        DemandTarget::ProposedDomainClosure,
    ),
];

const fn artifact_rule(family: ProofFamily) -> DemandRule {
    DemandRule {
        family,
        authority: ProofAuthority::PackageArtifacts,
        applies_to: DemandTarget::ArtifactCase,
    }
}

const fn type_facts_rule(family: ProofFamily, applies_to: DemandTarget) -> DemandRule {
    DemandRule {
        family,
        authority: ProofAuthority::TypeFacts,
        applies_to,
    }
}

const fn manifest() -> PolicyManifest {
    PolicyManifest {
        format: "solid-checker-contract-proof-policy",
        policy_version: POLICY_VERSION,
        proof_version: PROOF_VERSION,
        receipt_version: RECEIPT_VERSION,
        semantic_model_version: SEMANTIC_MODEL_VERSION,
        status: "active",
        canonical_encoding: CanonicalEncoding {
            name: "utf8-minified-ordered-json",
            version: 1,
        },
        digests: &DIGESTS,
        coverage: Coverage {
            positive_facts: "all-analyzer-visible-candidates",
            closures: "all-proposed-closures",
            inapplicability: "verifier-derived",
            unproved_positive: "remove-or-refuse",
            unproved_closure: "leave-open-or-refuse",
        },
        witness_envelope: WitnessEnvelope {
            variants: &WITNESS_VARIANTS,
            exactly_one_per_demand: true,
            concrete_site_identities_required: true,
            empty_witness_allowed: false,
            duplicate_sites_allowed: false,
            unknown_variants_allowed: false,
            serialized_authority: false,
        },
        applicability: Applicability {
            artifact_prerequisites: &ARTIFACT_PREREQUISITES,
            claim_families: &CLAIM_FAMILIES,
            compiler_reconciliation: DemandRule {
                family: ProofFamily::CompilerReconciliation,
                authority: ProofAuthority::CompilerExecutionFacts,
                applies_to: DemandTarget::CompilerOwnedSite,
            },
            dependency_composition: DemandRule {
                family: ProofFamily::AcceptedDependencyComposition,
                authority: ProofAuthority::AcceptedDependencyContract,
                applies_to: DemandTarget::RelevantExternalDependencyEdge,
            },
            probe_consistency: ProbeRule {
                family: ProofFamily::ProbeConsistency,
                authority: ProofAuthority::RuntimeProbe,
                mode: "separate-veto",
                applies_to: DemandTarget::VerifierScheduledProbe,
                missing_required_probe: "refuse",
                absence_proves_negative: false,
                successful_observation: "possible-positive-only",
            },
        },
        producer_constraints: ProducerConstraints {
            package_artifacts: ArtifactProducerConstraints {
                session: "immutable-snapshot",
                registry_or_lock_provenance_required: true,
                lifecycle_scripts_allowed: false,
                caller_serialized_authority_accepted: false,
            },
            type_facts: ProcessProducerConstraints {
                session: "direct-fresh-process",
                executable_digest_required: true,
                source_manifest_digest_required: true,
                runtime_digest_required: true,
                build_and_protocol_identity_required: true,
                private_execution_snapshot_required: true,
                process_and_restart_epoch_required: true,
                project_generation_required: true,
                snapshot_and_demand_binding_required: true,
                caller_serialized_authority_accepted: false,
            },
            compiler_execution_facts: ProcessProducerConstraints {
                session: "direct-fresh-process",
                executable_digest_required: true,
                source_manifest_digest_required: true,
                runtime_digest_required: true,
                build_and_protocol_identity_required: true,
                private_execution_snapshot_required: true,
                process_and_restart_epoch_required: true,
                project_generation_required: true,
                snapshot_and_demand_binding_required: true,
                caller_serialized_authority_accepted: false,
            },
            accepted_dependency_contract: DependencyProducerConstraints {
                receipt: "authenticated-policy-2",
                exact_dependency_edge_binding_required: true,
            },
            runtime_probe: ProbeProducerConstraints {
                harness: "isolated-bounded",
                harness_digest_required: true,
                environment_identity_required: true,
                contradiction_is_veto: true,
            },
        },
        resource_budgets: ResourceBudgets {
            contract_document_bytes: 1024 * 1024,
            proof_document_bytes: 16 * 1024 * 1024,
            receipt_bytes: 64 * 1024,
            json_depth: 128,
            json_nodes: 1_000_000,
            string_bytes: 16 * 1024,
            demands: 65_536,
            witness_items_per_demand: 16_384,
            registry_metadata_bytes: 16 * 1024 * 1024,
            archive_bytes: 256 * 1024 * 1024,
            expanded_archive_bytes: 1024 * 1024 * 1024,
            archive_members: 100_000,
            package_path_bytes: 4 * 1024,
        },
        receipt_trust: ReceiptTrust {
            canonical_main_encoding_required: true,
            policy_digest_constraint_required: true,
            receipt_carried_key_is_trust_root: false,
            built_in: BuiltInReceiptTrust {
                provenance: "binary-embedded",
                external_signature_required: false,
            },
            persistent_local: SignedReceiptTrust {
                algorithm: "ed25519",
                configured_issuer_and_trust_root_required: true,
            },
            portable: PortableReceiptTrust {
                algorithm: "ed25519",
                explicit_trust_store_chain_required: true,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DependencyDemandInput, ProofDemandSubject, ProofFamily, ProofWitnessVariant,
        WitnessBinding, WitnessCoverageError, proof_policy_2,
    };
    use crate::contract_semantics::{
        KnowledgeState, SEMANTIC_MODEL_VERSION, SemanticClaimPath,
        proof::{ACCEPTANCE_RECEIPT_VERSION, PROOF_POLICY_VERSION},
        solid2_rc3::conformance_corpus,
    };

    const AUDIT_MANIFEST: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/package-contract-v2/phase19/proof-policy-v2.json"
    ));

    #[test]
    fn policy_2_is_canonical_rust_owned_and_active() {
        let policy = proof_policy_2();

        assert_eq!(policy.policy_version(), 2);
        assert_eq!(policy.proof_version(), 2);
        assert_eq!(policy.receipt_version(), 2);
        assert_eq!(policy.semantic_model_version(), SEMANTIC_MODEL_VERSION);
        assert_eq!(
            policy.digest().as_str(),
            "sha256:23d11125ffeb3c57cace0f898f46895dfb383113ff3637d409eb719eaaf088fd"
        );
        // Policy-1 replay types remain only for historical internal tests;
        // the backend loader no longer accepts their receipts.
        assert_eq!(PROOF_POLICY_VERSION, 1);
        assert_eq!(ACCEPTANCE_RECEIPT_VERSION, 1);

        assert_eq!(
            policy.audit_manifest(),
            AUDIT_MANIFEST.strip_suffix(b"\n").unwrap_or(AUDIT_MANIFEST)
        );
    }

    #[test]
    fn candidate_inventory_is_verifier_derived_and_preserves_partial_positives() {
        let complete = conformance_corpus()
            .into_iter()
            .next()
            .unwrap()
            .proposal
            .normalize()
            .unwrap();

        let candidates = proof_policy_2().inspect_candidates(&complete).unwrap();

        assert_eq!(
            candidates.candidate_semantic_digest(),
            complete.semantic_digest()
        );
        assert_eq!(candidates.positive_operations().len(), 6);
        assert!(candidates.positive_operations().iter().any(|subject| {
            subject.export == "createEffect"
                && matches!(
                    &subject.path,
                    SemanticClaimPath::Operation(operation) if operation.0 == "initial-compute"
                )
        }));
        assert!(
            candidates
                .closure_candidates()
                .iter()
                .all(|subject| matches!(subject.path, SemanticClaimPath::Domain(_)))
        );

        let reopened = candidates
            .proposal()
            .artifact_case("solid-browser-development")
            .unwrap()
            .exports
            .get("createEffect")
            .unwrap();
        assert_eq!(
            reopened.claim_state(crate::contract_semantics::ClaimDomain::Callbacks),
            KnowledgeState::PartialPositive
        );
        assert_eq!(reopened.call.operations.len(), 6);
        assert_ne!(
            candidates.proposal().semantic_digest(),
            complete.semantic_digest()
        );
    }

    #[test]
    fn demand_graph_covers_every_positive_and_closure_without_caller_plan_input() {
        let complete = conformance_corpus()
            .into_iter()
            .next()
            .unwrap()
            .proposal
            .normalize()
            .unwrap();
        let policy = proof_policy_2();
        let candidates = policy.inspect_candidates(&complete).unwrap();
        let graph = policy
            .derive_demand_graph(
                &candidates,
                &format!("sha256:{:064x}", 1),
                &format!("sha256:{:064x}", 2),
            )
            .unwrap();

        assert!(candidates.positive_facts().iter().all(|positive| {
            graph.demands().iter().any(|demand| {
                demand.subject() == &ProofDemandSubject::PositiveFact(positive.clone())
            })
        }));
        assert!(candidates.closure_candidates().iter().all(|closure| {
            graph.demands().iter().any(|demand| {
                matches!(
                    demand.subject(),
                    ProofDemandSubject::DomainClosure { subject, .. } if subject == closure
                )
            })
        }));
        assert_eq!(
            graph
                .demands()
                .iter()
                .map(|demand| demand.id().as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            graph.demands().len()
        );

        let changed_snapshot = policy
            .derive_demand_graph(
                &candidates,
                &format!("sha256:{:064x}", 3),
                &format!("sha256:{:064x}", 2),
            )
            .unwrap();
        assert_ne!(graph.root(), changed_snapshot.root());
        assert!(
            graph
                .demands()
                .iter()
                .zip(changed_snapshot.demands())
                .all(|(left, right)| left.id() != right.id())
        );
    }

    #[test]
    fn dependency_demands_cover_every_parent_closure_and_bind_the_exact_edge() {
        let complete = conformance_corpus()
            .into_iter()
            .next()
            .unwrap()
            .proposal
            .normalize()
            .unwrap();
        let policy = proof_policy_2();
        let candidates = policy.inspect_candidates(&complete).unwrap();
        let edge = DependencyDemandInput {
            specifier: "dependency/subpath".into(),
            package: "dependency".into(),
            artifact_case: "artifact-case:dependency:browser".into(),
            accepted_contract_digest: format!("sha256:{:064x}", 7),
        };
        let graph = policy
            .derive_demand_graph_with_dependencies(
                &candidates,
                &format!("sha256:{:064x}", 1),
                &format!("sha256:{:064x}", 2),
                [edge.clone(), edge.clone()],
            )
            .unwrap();
        let dependency_demands = graph
            .demands()
            .iter()
            .filter(|demand| demand.family() == ProofFamily::AcceptedDependencyComposition)
            .collect::<Vec<_>>();
        assert_eq!(
            dependency_demands.len(),
            candidates.closure_candidates().len()
        );
        assert!(dependency_demands.iter().all(|demand| matches!(
            demand.subject(),
            ProofDemandSubject::DependencyClosure { dependency, parent, .. }
                if dependency == &edge && candidates.closure_candidates().contains(parent)
        )));

        let changed = policy
            .derive_demand_graph_with_dependencies(
                &candidates,
                &format!("sha256:{:064x}", 1),
                &format!("sha256:{:064x}", 2),
                [DependencyDemandInput {
                    accepted_contract_digest: format!("sha256:{:064x}", 8),
                    ..edge
                }],
            )
            .unwrap();
        assert_ne!(graph.root(), changed.root());
    }

    #[test]
    fn witness_coverage_rejects_substitution_omission_and_fabricated_empty_evidence() {
        let complete = conformance_corpus()
            .into_iter()
            .next()
            .unwrap()
            .proposal
            .normalize()
            .unwrap();
        let policy = proof_policy_2();
        let candidates = policy.inspect_candidates(&complete).unwrap();
        let graph = policy
            .derive_demand_graph(
                &candidates,
                &format!("sha256:{:064x}", 1),
                &format!("sha256:{:064x}", 2),
            )
            .unwrap();
        let bindings = graph
            .demands()
            .iter()
            .enumerate()
            .map(|(index, demand)| {
                WitnessBinding::new(
                    witness_variant(demand.family()),
                    demand.id().as_str(),
                    format!("sha256:{index:064x}"),
                    vec![format!("site:{index}")],
                )
            })
            .collect::<Vec<_>>();

        let coverage = graph.verify_witness_coverage(bindings.clone()).unwrap();
        let reversed = graph
            .verify_witness_coverage(bindings.iter().cloned().rev())
            .unwrap();
        assert_eq!(coverage.len(), graph.demands().len());
        assert_eq!(coverage.demand_graph_root(), graph.root());
        assert_eq!(coverage.evidence_root(), reversed.evidence_root());

        assert_eq!(
            graph.verify_witness_coverage(bindings[..bindings.len() - 1].iter().cloned()),
            Err(WitnessCoverageError::MissingWitness)
        );
        assert_eq!(
            graph.verify_witness_coverage(
                bindings
                    .iter()
                    .cloned()
                    .chain(std::iter::once(bindings[0].clone()))
            ),
            Err(WitnessCoverageError::DuplicateWitness)
        );
        let mut orphan = bindings.clone();
        orphan[0] = WitnessBinding::new(
            witness_variant(graph.demands()[0].family()),
            format!("sha256:{:064x}", 999),
            format!("sha256:{:064x}", 1),
            vec!["orphan-site".into()],
        );
        assert_eq!(
            graph.verify_witness_coverage(orphan),
            Err(WitnessCoverageError::OrphanWitness)
        );
        let mut wrong_family = bindings.clone();
        wrong_family[0] = WitnessBinding::new(
            ProofWitnessVariant::ModuleClosure,
            graph.demands()[0].id().as_str(),
            format!("sha256:{:064x}", 1),
            vec!["wrong-family-site".into()],
        );
        assert_eq!(
            graph.verify_witness_coverage(wrong_family),
            Err(WitnessCoverageError::FamilyMismatch)
        );
        for (site_ids, expected) in [
            (Vec::new(), WitnessCoverageError::EmptyWitness),
            (
                vec!["site".to_owned(); policy.witness_items_per_demand_limit() + 1],
                WitnessCoverageError::ItemLimit,
            ),
        ] {
            let mut invalid = bindings.clone();
            invalid[0] = WitnessBinding::new(
                witness_variant(graph.demands()[0].family()),
                graph.demands()[0].id().as_str(),
                format!("sha256:{:064x}", 1),
                site_ids,
            );
            assert_eq!(graph.verify_witness_coverage(invalid), Err(expected));
        }
        let mut invalid_root = bindings;
        invalid_root[0] = WitnessBinding::new(
            witness_variant(graph.demands()[0].family()),
            graph.demands()[0].id().as_str(),
            "caller-says-complete",
            vec!["invalid-root-site".into()],
        );
        assert_eq!(
            graph.verify_witness_coverage(invalid_root),
            Err(WitnessCoverageError::InvalidEvidenceRoot)
        );

        let mut duplicate_site = graph
            .demands()
            .iter()
            .enumerate()
            .map(|(index, demand)| {
                WitnessBinding::new(
                    witness_variant(demand.family()),
                    demand.id().as_str(),
                    format!("sha256:{index:064x}"),
                    if index == 0 {
                        vec!["same-site".into(), "same-site".into()]
                    } else {
                        vec![format!("site:{index}")]
                    },
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            graph.verify_witness_coverage(duplicate_site.drain(..)),
            Err(WitnessCoverageError::DuplicateSiteIdentity)
        );
    }

    fn witness_variant(family: ProofFamily) -> ProofWitnessVariant {
        match family {
            ProofFamily::PackageIdentity => ProofWitnessVariant::PackageIdentity,
            ProofFamily::ManifestEntrypoint => ProofWitnessVariant::ManifestEntrypoint,
            ProofFamily::ExportResolution => ProofWitnessVariant::ExportResolution,
            ProofFamily::ArtifactDeclarations => ProofWitnessVariant::ArtifactDeclarations,
            ProofFamily::ExportIdentity => ProofWitnessVariant::ExportIdentity,
            ProofFamily::ModuleClosure => ProofWitnessVariant::ModuleClosure,
            ProofFamily::SelectedSignature => ProofWitnessVariant::SelectedSignature,
            ProofFamily::ArgumentBinding => ProofWitnessVariant::ArgumentBinding,
            ProofFamily::RestSpreadCoverage => ProofWitnessVariant::RestSpreadCoverage,
            ProofFamily::CallablePath => ProofWitnessVariant::CallablePath,
            ProofFamily::OperationReachability => ProofWitnessVariant::OperationReachability,
            ProofFamily::OperationCardinality => ProofWitnessVariant::OperationCardinality,
            ProofFamily::RecursiveValueShape => ProofWitnessVariant::RecursiveValueShape,
            ProofFamily::GuardPartition => ProofWitnessVariant::GuardPartition,
            ProofFamily::CompilerReconciliation => ProofWitnessVariant::CompilerReconciliation,
            ProofFamily::AcceptedDependencyComposition => {
                ProofWitnessVariant::AcceptedDependencyComposition
            }
            ProofFamily::DomainExhaustiveness => ProofWitnessVariant::DomainExhaustiveness,
            ProofFamily::ProbeConsistency => panic!("probe consistency is not a proof demand"),
        }
    }
}
