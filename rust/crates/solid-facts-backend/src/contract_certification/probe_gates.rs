//! Verifier-derived mandatory runtime-probe vetoes for policy 2.
//!
//! Probe gates are deliberately separate from proof witnesses. A contradiction
//! vetoes the exact proposed closure. A successful finite observation is only
//! audit material and cannot establish absence, completeness, or closure. The
//! current runtime-probe evaluator is exact, but its executable/Node image is
//! not yet launched by an authority-bearing adapter, so this module refuses to
//! authenticate even a structurally complete successful batch.

use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::SemanticClaimSubject;
use std::collections::BTreeMap;
use thiserror::Error;

use super::CertificationPlan;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProbeGate {
    id: String,
    subject: SemanticClaimSubject,
    semantic_claim_id: String,
}

impl ProbeGate {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn subject(&self) -> &SemanticClaimSubject {
        &self.subject
    }

    #[must_use]
    pub fn semantic_claim_id(&self) -> &str {
        &self.semantic_claim_id
    }
}

pub struct ProbeGateSchedule {
    gates: Vec<ProbeGate>,
}

impl ProbeGateSchedule {
    pub(crate) fn from_plan(plan: &CertificationPlan) -> Result<Self, ProbeGateError> {
        let mut gates = plan
            .candidates
            .closure_candidates()
            .iter()
            .map(|subject| {
                let claim = plan
                    .candidates
                    .proposal()
                    .claim_id(subject)
                    .map_err(|_| ProbeGateError::InvalidSubject)?;
                let semantic_claim_id = claim.as_str().to_owned();
                Ok(ProbeGate {
                    id: probe_gate_id(
                        plan.snapshot.root(),
                        plan.demand_graph.root().as_str(),
                        &semantic_claim_id,
                    ),
                    subject: subject.clone(),
                    semantic_claim_id,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        gates.sort();
        if gates.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(ProbeGateError::IdCollision);
        }
        Ok(Self { gates })
    }

    #[must_use]
    pub fn gates(&self) -> &[ProbeGate] {
        &self.gates
    }

    /// Checks coverage and contradiction semantics for untrusted/audit
    /// outcomes. The returned value is intentionally not authority-bearing.
    pub fn inspect_outcomes(
        &self,
        outcomes: impl IntoIterator<Item = ProbeGateOutcome>,
    ) -> Result<InspectedProbeGateBatch, ProbeGateError> {
        let expected = self
            .gates
            .iter()
            .map(|gate| (gate.id.as_str(), gate))
            .collect::<BTreeMap<_, _>>();
        let mut supplied = BTreeMap::<String, ProbeGateOutcomeKind>::new();
        for outcome in outcomes {
            if !expected.contains_key(outcome.gate_id.as_str()) {
                return Err(ProbeGateError::UnknownGate(outcome.gate_id));
            }
            if supplied
                .insert(outcome.gate_id.clone(), outcome.kind)
                .is_some()
            {
                return Err(ProbeGateError::DuplicateGate(outcome.gate_id));
            }
        }
        if supplied.len() != expected.len() {
            let missing = expected
                .keys()
                .find(|id| !supplied.contains_key(**id))
                .map_or_else(|| "unknown".into(), |id| (*id).into());
            return Err(ProbeGateError::MissingGate(missing));
        }
        for gate in &self.gates {
            match supplied
                .get(gate.id.as_str())
                .expect("complete gate coverage was checked")
            {
                ProbeGateOutcomeKind::Contradiction => {
                    return Err(ProbeGateError::Contradiction {
                        gate_id: gate.id.clone(),
                        semantic_claim_id: gate.semantic_claim_id.clone(),
                    });
                }
                ProbeGateOutcomeKind::ErrorOrTimeout => {
                    return Err(ProbeGateError::IncompleteGate(gate.id.clone()));
                }
                ProbeGateOutcomeKind::NoContradictionObserved => {}
            }
        }
        Ok(InspectedProbeGateBatch {
            gate_ids: supplied.into_keys().collect(),
        })
    }

    /// Successful observations cannot be promoted while the harness image and
    /// Node runtime are not independently pinned and directly launched.
    pub fn authenticate(
        &self,
        _inspected: InspectedProbeGateBatch,
    ) -> Result<VerifiedProbeGateBatch, ProbeGateError> {
        Err(ProbeGateError::HarnessBindingRequired)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeGateOutcomeKind {
    NoContradictionObserved,
    Contradiction,
    ErrorOrTimeout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeGateOutcome {
    gate_id: String,
    kind: ProbeGateOutcomeKind,
}

impl ProbeGateOutcome {
    #[must_use]
    pub fn new(gate_id: impl Into<String>, kind: ProbeGateOutcomeKind) -> Self {
        Self {
            gate_id: gate_id.into(),
            kind,
        }
    }
}

#[derive(Debug)]
pub struct InspectedProbeGateBatch {
    gate_ids: Vec<String>,
}

impl InspectedProbeGateBatch {
    #[must_use]
    pub fn gate_ids(&self) -> &[String] {
        &self.gate_ids
    }
}

/// Reserved authority type for the directly launched harness adapter. There is
/// intentionally no constructor in this slice.
pub struct VerifiedProbeGateBatch {
    _private: (),
}

fn probe_gate_id(snapshot_root: &str, demand_graph_root: &str, claim_id: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:contract-probe-gate:v2");
    for value in [snapshot_root, demand_graph_root, claim_id] {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
    format!("sha256:{:x}", hash.finalize())
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProbeGateError {
    #[error("probe gate subject is absent from normalized meaning")]
    InvalidSubject,
    #[error("probe gate ID collision")]
    IdCollision,
    #[error("unknown probe gate {0}")]
    UnknownGate(String),
    #[error("duplicate probe gate {0}")]
    DuplicateGate(String),
    #[error("missing mandatory probe gate {0}")]
    MissingGate(String),
    #[error("mandatory probe gate {0} did not complete")]
    IncompleteGate(String),
    #[error("probe contradiction at {gate_id} for {semantic_claim_id}")]
    Contradiction {
        gate_id: String,
        semantic_claim_id: String,
    },
    #[error("probe harness executable and Node runtime are not authority-bound")]
    HarnessBindingRequired,
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_reactive_ir::contract_semantics::{ClaimDomain, ClaimPath, SemanticClaimPath};

    #[test]
    fn gate_ids_bind_snapshot_graph_and_claim() {
        assert_ne!(
            probe_gate_id("snapshot-a", "graph", "claim"),
            probe_gate_id("snapshot-b", "graph", "claim")
        );
        assert_ne!(
            probe_gate_id("snapshot", "graph-a", "claim"),
            probe_gate_id("snapshot", "graph-b", "claim")
        );
    }

    fn schedule() -> ProbeGateSchedule {
        ProbeGateSchedule {
            gates: vec![ProbeGate {
                id: "sha256:gate".into(),
                subject: SemanticClaimSubject {
                    artifact_case: "artifact-case:fixture".into(),
                    export: "run".into(),
                    path: SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Callbacks)),
                },
                semantic_claim_id: "sha256:claim".into(),
            }],
        }
    }

    #[test]
    fn missing_incomplete_and_contradictory_gates_fail_closed() {
        let schedule = schedule();
        assert_eq!(
            schedule.inspect_outcomes([]).unwrap_err(),
            ProbeGateError::MissingGate("sha256:gate".into())
        );
        assert!(matches!(
            schedule.inspect_outcomes([ProbeGateOutcome::new(
                "sha256:gate",
                ProbeGateOutcomeKind::ErrorOrTimeout,
            )]),
            Err(ProbeGateError::IncompleteGate(_))
        ));
        assert!(matches!(
            schedule.inspect_outcomes([ProbeGateOutcome::new(
                "sha256:gate",
                ProbeGateOutcomeKind::Contradiction,
            )]),
            Err(ProbeGateError::Contradiction { .. })
        ));
    }

    #[test]
    fn successful_nonobservation_remains_non_authoritative() {
        let schedule = schedule();
        let inspected = schedule
            .inspect_outcomes([ProbeGateOutcome::new(
                "sha256:gate",
                ProbeGateOutcomeKind::NoContradictionObserved,
            )])
            .unwrap();
        assert_eq!(inspected.gate_ids(), &["sha256:gate".to_owned()]);
        assert!(matches!(
            schedule.authenticate(inspected),
            Err(ProbeGateError::HarnessBindingRequired)
        ));
    }
}
