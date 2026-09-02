//! Bounded, non-authoritative proof-witness-v2 decoder.
//!
//! Decoding can establish only that a document names every verifier-derived
//! demand exactly once with the matching closed family variant. Producer
//! adapters must still authenticate and semantically verify the referenced
//! evidence before certification can use it.

use serde::Deserialize;
use solid_reactive_ir::contract_semantics::certification::{
    ProofDemandGraph, ProofWitnessVariant, WitnessBinding, WitnessCoverage, WitnessCoverageError,
    proof_policy_2,
};
use thiserror::Error;

const FORMAT: &str = "solid-checker-contract-proof-witnesses";

pub(super) fn decode_witness_coverage(
    bytes: &[u8],
    graph: &ProofDemandGraph,
) -> Result<WitnessCoverage, WitnessWireError> {
    let policy = proof_policy_2();
    let document: WireDocument = crate::bounded_json::decode(
        bytes,
        crate::bounded_json::Limits {
            bytes: policy.proof_document_bytes_limit(),
            depth: policy.proof_json_depth_limit(),
            nodes: policy.proof_json_nodes_limit(),
            string_bytes: policy.proof_string_bytes_limit(),
        },
    )
    .map_err(WitnessWireError::Decode)?;
    if document.format != FORMAT
        || document.proof_version != policy.proof_version()
        || document.proof_policy != policy.policy_version()
        || document.policy_digest != graph.policy_digest().as_str()
        || document.demand_graph_root != graph.root().as_str()
    {
        return Err(WitnessWireError::Identity);
    }
    let bindings = document.witnesses.into_iter().map(|witness| {
        WitnessBinding::new(
            witness.kind.into(),
            witness.demand_id,
            witness.evidence_root,
            witness.sites,
        )
    });
    graph
        .verify_witness_coverage(bindings)
        .map_err(WitnessWireError::Coverage)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireDocument {
    format: String,
    proof_version: u16,
    proof_policy: u32,
    policy_digest: String,
    demand_graph_root: String,
    witnesses: Vec<WireWitness>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireWitness {
    kind: WireWitnessKind,
    demand_id: String,
    evidence_root: String,
    sites: Vec<String>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireWitnessKind {
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

impl From<WireWitnessKind> for ProofWitnessVariant {
    fn from(value: WireWitnessKind) -> Self {
        match value {
            WireWitnessKind::PackageIdentity => Self::PackageIdentity,
            WireWitnessKind::ManifestEntrypoint => Self::ManifestEntrypoint,
            WireWitnessKind::ExportResolution => Self::ExportResolution,
            WireWitnessKind::ArtifactDeclarations => Self::ArtifactDeclarations,
            WireWitnessKind::ExportIdentity => Self::ExportIdentity,
            WireWitnessKind::ModuleClosure => Self::ModuleClosure,
            WireWitnessKind::SelectedSignature => Self::SelectedSignature,
            WireWitnessKind::ArgumentBinding => Self::ArgumentBinding,
            WireWitnessKind::RestSpreadCoverage => Self::RestSpreadCoverage,
            WireWitnessKind::CallablePath => Self::CallablePath,
            WireWitnessKind::OperationReachability => Self::OperationReachability,
            WireWitnessKind::OperationCardinality => Self::OperationCardinality,
            WireWitnessKind::RecursiveValueShape => Self::RecursiveValueShape,
            WireWitnessKind::GuardPartition => Self::GuardPartition,
            WireWitnessKind::CompilerReconciliation => Self::CompilerReconciliation,
            WireWitnessKind::AcceptedDependencyComposition => Self::AcceptedDependencyComposition,
            WireWitnessKind::DomainExhaustiveness => Self::DomainExhaustiveness,
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum WitnessWireError {
    #[error("invalid proof-witness-v2 document: {0}")]
    Decode(String),
    #[error("proof-witness-v2 document identity does not match the verifier-derived plan")]
    Identity,
    #[error(transparent)]
    Coverage(#[from] WitnessCoverageError),
}

#[cfg(test)]
mod tests {
    use super::{WitnessWireError, decode_witness_coverage};
    use serde_json::{Value, json};
    use solid_reactive_ir::contract_semantics::{
        certification::{ProofDemandGraph, ProofFamily, proof_policy_2},
        solid2_rc3::conformance_corpus,
    };

    fn graph() -> ProofDemandGraph {
        let candidate = conformance_corpus()
            .into_iter()
            .next()
            .unwrap()
            .proposal
            .normalize()
            .unwrap();
        let policy = proof_policy_2();
        let candidates = policy.inspect_candidates(&candidate).unwrap();
        policy
            .derive_demand_graph(
                &candidates,
                &format!("sha256:{:064x}", 1),
                &format!("sha256:{:064x}", 2),
            )
            .unwrap()
    }

    fn document(graph: &ProofDemandGraph) -> Value {
        json!({
            "format": "solid-checker-contract-proof-witnesses",
            "proofVersion": 2,
            "proofPolicy": 2,
            "policyDigest": graph.policy_digest().as_str(),
            "demandGraphRoot": graph.root().as_str(),
            "witnesses": graph.demands().iter().enumerate().map(|(index, demand)| json!({
                "kind": family_name(demand.family()),
                "demandId": demand.id().as_str(),
                "evidenceRoot": format!("sha256:{index:064x}"),
                "sites": [format!("site:{index}")]
            })).collect::<Vec<_>>()
        })
    }

    #[test]
    fn bounded_wire_decoder_rejects_unknown_variants_fields_and_plan_substitution() {
        let graph = graph();
        let valid = serde_json::to_vec(&document(&graph)).unwrap();
        let coverage = decode_witness_coverage(&valid, &graph).unwrap();
        assert_eq!(coverage.len(), graph.demands().len());

        let mut unknown_variant = document(&graph);
        unknown_variant["witnesses"][0]["kind"] = json!("inapplicable");
        assert!(matches!(
            decode_witness_coverage(&serde_json::to_vec(&unknown_variant).unwrap(), &graph),
            Err(WitnessWireError::Decode(_))
        ));

        let mut unknown_field = document(&graph);
        unknown_field["witnesses"][0]["complete"] = json!(true);
        assert!(matches!(
            decode_witness_coverage(&serde_json::to_vec(&unknown_field).unwrap(), &graph),
            Err(WitnessWireError::Decode(_))
        ));

        let mut missing = document(&graph);
        missing["witnesses"].as_array_mut().unwrap().pop();
        assert_eq!(
            decode_witness_coverage(&serde_json::to_vec(&missing).unwrap(), &graph),
            Err(WitnessWireError::Coverage(
                solid_reactive_ir::contract_semantics::certification::WitnessCoverageError::MissingWitness
            ))
        );

        let mut wrong_graph = document(&graph);
        wrong_graph["demandGraphRoot"] = json!(format!("sha256:{:064x}", 999));
        assert_eq!(
            decode_witness_coverage(&serde_json::to_vec(&wrong_graph).unwrap(), &graph),
            Err(WitnessWireError::Identity)
        );

        let mut family_mismatch = document(&graph);
        family_mismatch["witnesses"][0]["kind"] = json!("module-closure");
        assert_eq!(
            decode_witness_coverage(&serde_json::to_vec(&family_mismatch).unwrap(), &graph),
            Err(WitnessWireError::Coverage(
                solid_reactive_ir::contract_semantics::certification::WitnessCoverageError::FamilyMismatch
            ))
        );
    }

    fn family_name(family: ProofFamily) -> &'static str {
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
            ProofFamily::ProbeConsistency => panic!("probe consistency is not a proof demand"),
        }
    }
}
