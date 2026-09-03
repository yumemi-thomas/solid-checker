//! Verifier-derived mandatory runtime-probe vetoes for policy 2.
//!
//! Probe gates are deliberately separate from proof witnesses. A contradiction
//! vetoes the exact proposed closure. A successful finite observation is only
//! audit material and cannot establish absence, completeness, or closure —
//! the Type Facts `DomainExhaustiveness` witness is what discharges a closed
//! claim domain, and a passing gate only declines to veto it.
//!
//! Two things are authority here, and nothing else is:
//!
//! * The schedule. One gate per proposed closure, its id bound to the artifact
//!   snapshot root, the demand-graph root, and the exact semantic claim. A
//!   caller cannot add, drop, or rename one.
//! * The verdict. `ProbeGateOutcome` is crate-private and has no public
//!   constructor: outcomes are derived from a [`RuntimeProbeEvaluation`]
//!   produced by the harness adapter in the same certification transaction,
//!   never handed in.
//!
//! An empty schedule authenticates on its own — there is nothing to launch,
//! and the verifier derived the emptiness itself. A nonempty schedule
//! authenticates only against a [`BoundProbeHarnessIdentity`], which exists
//! only after `probe_harness` resolved and hashed the Node executable and the
//! harness image against this build's compiled-in pins, ran the worker from a
//! private directory, and proved that neither the snapshot copy nor any
//! producer image changed across the run.

use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{SemanticClaimId, SemanticClaimSubject};
use std::collections::BTreeMap;
use thiserror::Error;

use super::CertificationPlan;
use super::probe_harness::BoundProbeHarnessIdentity;
use crate::runtime_probes::{ProbeTargetVerdict, RuntimeProbeEvaluation};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProbeGate {
    id: String,
    subject: SemanticClaimSubject,
    semantic_claim_id: SemanticClaimId,
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
        self.semantic_claim_id.as_str()
    }
}

pub struct ProbeGateSchedule {
    snapshot_root: String,
    demand_graph_root: String,
    gates: Vec<ProbeGate>,
}

impl ProbeGateSchedule {
    pub(crate) fn from_plan(plan: &CertificationPlan) -> Result<Self, ProbeGateError> {
        let snapshot_root = plan.snapshot.root().to_owned();
        let demand_graph_root = plan.demand_graph.root().as_str().to_owned();
        let mut gates = plan
            .candidates
            .closure_candidates()
            .iter()
            .map(|subject| {
                let semantic_claim_id = plan
                    .candidates
                    .proposal()
                    .claim_id(subject)
                    .map_err(|_| ProbeGateError::InvalidSubject)?;
                Ok(ProbeGate {
                    id: probe_gate_id(
                        &snapshot_root,
                        &demand_graph_root,
                        semantic_claim_id.as_str(),
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
        Ok(Self {
            snapshot_root,
            demand_graph_root,
            gates,
        })
    }

    #[must_use]
    pub fn gates(&self) -> &[ProbeGate] {
        &self.gates
    }

    /// Derives exactly one outcome per scheduled gate from an evaluation this
    /// transaction produced.
    ///
    /// This is the only way an outcome comes into existence. The evaluator
    /// already validated session coverage, isolation, determinism, drain
    /// bounds, environment identity, and scenario lifecycle; the mapping here
    /// is total and refuses a gate the evaluation says nothing about.
    pub(crate) fn outcomes_from_evaluation(
        &self,
        evaluation: &RuntimeProbeEvaluation,
    ) -> Result<Vec<ProbeGateOutcome>, ProbeGateError> {
        self.gates
            .iter()
            .map(|gate| {
                let kind = match evaluation.verdict(&gate.semantic_claim_id) {
                    Some(ProbeTargetVerdict::Contradiction) => ProbeGateOutcomeKind::Contradiction,
                    Some(ProbeTargetVerdict::CleanNonObservation) => {
                        ProbeGateOutcomeKind::NoContradictionObserved
                    }
                    Some(ProbeTargetVerdict::Incomplete) => ProbeGateOutcomeKind::ErrorOrTimeout,
                    // A witness-authority target cannot veto a closure, and a
                    // claim the run never covered has no verdict at all.
                    // Neither satisfies a mandatory gate.
                    Some(ProbeTargetVerdict::NotAGate) | None => {
                        return Err(ProbeGateError::MissingGate(gate.id.clone()));
                    }
                };
                Ok(ProbeGateOutcome {
                    gate_id: gate.id.clone(),
                    kind,
                })
            })
            .collect()
    }

    /// Checks coverage and contradiction semantics for the derived outcomes.
    /// The returned value is intentionally not authority-bearing.
    pub(crate) fn inspect_outcomes(
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
                        semantic_claim_id: gate.semantic_claim_id.as_str().to_owned(),
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

    /// Authenticates the verifier-derived *empty* schedule.
    ///
    /// There is nothing to launch and no harness to bind: the certifier walked
    /// the normalized artifact case itself and found no proposed closure, so
    /// the empty veto set is its own conclusion. A nonempty schedule refuses
    /// here and must go through [`Self::authenticate_with_harness`].
    pub(crate) fn authenticate(
        &self,
        inspected: InspectedProbeGateBatch,
    ) -> Result<VerifiedProbeGateBatch, ProbeGateError> {
        if !self.gates.is_empty() || !inspected.gate_ids.is_empty() {
            return Err(ProbeGateError::HarnessBindingRequired);
        }
        Ok(VerifiedProbeGateBatch {
            snapshot_root: self.snapshot_root.clone(),
            demand_graph_root: self.demand_graph_root.clone(),
            gate_ids: Vec::new(),
            harness: None,
        })
    }

    /// Authenticates a complete nonempty batch against the harness image and
    /// Node runtime the adapter actually launched.
    ///
    /// The harness identity is refused unless it was bound for this exact
    /// schedule: the same artifact snapshot root, the same demand-graph root,
    /// and the same gate ids. A harness bound for another plan, another
    /// snapshot, or a different veto set is not authority here.
    pub(crate) fn authenticate_with_harness(
        &self,
        inspected: InspectedProbeGateBatch,
        harness: &BoundProbeHarnessIdentity,
    ) -> Result<VerifiedProbeGateBatch, ProbeGateError> {
        if self.gates.is_empty() {
            return Err(ProbeGateError::EmptyScheduleClaimsHarness);
        }
        let scheduled = self
            .gates
            .iter()
            .map(|gate| gate.id.clone())
            .collect::<Vec<_>>();
        if inspected.gate_ids != scheduled {
            return Err(ProbeGateError::ScheduleMismatch);
        }
        if harness.snapshot_root() != self.snapshot_root
            || harness.demand_graph_root() != self.demand_graph_root
            || harness.gate_ids() != scheduled.as_slice()
        {
            return Err(ProbeGateError::HarnessScheduleMismatch);
        }
        Ok(VerifiedProbeGateBatch {
            snapshot_root: self.snapshot_root.clone(),
            demand_graph_root: self.demand_graph_root.clone(),
            gate_ids: scheduled,
            harness: Some(harness.clone()),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeGateOutcomeKind {
    NoContradictionObserved,
    Contradiction,
    ErrorOrTimeout,
}

/// One gate's derived verdict. Crate-private with no public constructor: the
/// only producer is [`ProbeGateSchedule::outcomes_from_evaluation`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProbeGateOutcome {
    gate_id: String,
    kind: ProbeGateOutcomeKind,
}

#[derive(Debug)]
pub(crate) struct InspectedProbeGateBatch {
    gate_ids: Vec<String>,
}

/// Authority that every mandatory probe veto for one plan was scheduled,
/// executed where a launch was required, and declined to veto.
///
/// Non-serializable and constructible only by [`ProbeGateSchedule`]. It never
/// asserts that a claim domain is closed; finalization consumes it purely as
/// the receipt's probe-gate binding, alongside the independent Type Facts
/// witness that actually discharges `DomainExhaustiveness`.
#[derive(Debug)]
pub struct VerifiedProbeGateBatch {
    snapshot_root: String,
    demand_graph_root: String,
    gate_ids: Vec<String>,
    harness: Option<BoundProbeHarnessIdentity>,
}

impl VerifiedProbeGateBatch {
    /// Refuses a batch verified for a different snapshot, demand graph, or veto
    /// set than the plan being finalized.
    ///
    /// The schedule is re-derived from the plan rather than taken on trust, and
    /// the gate ids are compared as well as the two roots. The roots alone were
    /// nearly enough — a gate id is a digest over them plus the claim, and the
    /// demand-graph root moves with the closure demands — but "nearly" is not a
    /// property this check should rest on: the batch's authority is exactly the
    /// gates that were run, so that is what is compared.
    pub(crate) fn verify_plan(&self, plan: &CertificationPlan) -> Result<(), ProbeGateError> {
        self.verify_schedule(&ProbeGateSchedule::from_plan(plan)?)
    }

    fn verify_schedule(&self, schedule: &ProbeGateSchedule) -> Result<(), ProbeGateError> {
        if self.snapshot_root != schedule.snapshot_root
            || self.demand_graph_root != schedule.demand_graph_root
        {
            return Err(ProbeGateError::PlanMismatch);
        }
        let scheduled = schedule
            .gates
            .iter()
            .map(|gate| gate.id.clone())
            .collect::<Vec<_>>();
        if self.gate_ids != scheduled {
            return Err(ProbeGateError::PlanMismatch);
        }
        Ok(())
    }

    pub(crate) fn gate_ids(&self) -> &[String] {
        &self.gate_ids
    }

    /// The stable harness and runtime identity fields bound into the receipt's
    /// probe-gate root. Deliberately excludes the per-launch nonce, process id,
    /// and launch epoch: those bind the live worker inside the transaction, and
    /// a receipt root must stay reproducible from the same inputs.
    pub(crate) fn harness_identity_fields(&self) -> Vec<&str> {
        self.harness
            .as_ref()
            .map(BoundProbeHarnessIdentity::root_fields)
            .unwrap_or_default()
    }
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
    #[error("an empty probe schedule cannot claim a launched harness")]
    EmptyScheduleClaimsHarness,
    #[error("inspected probe outcomes do not cover exactly the scheduled gates")]
    ScheduleMismatch,
    #[error("the bound probe harness was launched for a different gate schedule")]
    HarnessScheduleMismatch,
    #[error("a verified probe batch was presented to a different certification plan")]
    PlanMismatch,
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
            snapshot_root: "sha256:snapshot".into(),
            demand_graph_root: "sha256:graph".into(),
            gates: vec![ProbeGate {
                id: "sha256:gate".into(),
                subject: SemanticClaimSubject {
                    artifact_case: "artifact-case:fixture".into(),
                    export: "run".into(),
                    path: SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Callbacks)),
                },
                semantic_claim_id: SemanticClaimId::parse(format!(
                    "claim:v1:sha256:{}",
                    "c".repeat(64)
                ))
                .expect("test claim id is canonical"),
            }],
        }
    }

    fn outcome(kind: ProbeGateOutcomeKind) -> ProbeGateOutcome {
        ProbeGateOutcome {
            gate_id: "sha256:gate".into(),
            kind,
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
            schedule.inspect_outcomes([outcome(ProbeGateOutcomeKind::ErrorOrTimeout)]),
            Err(ProbeGateError::IncompleteGate(_))
        ));
        assert!(matches!(
            schedule.inspect_outcomes([outcome(ProbeGateOutcomeKind::Contradiction)]),
            Err(ProbeGateError::Contradiction { .. })
        ));
    }

    #[test]
    fn successful_nonobservation_still_requires_the_bound_harness() {
        let schedule = schedule();
        let inspected = schedule
            .inspect_outcomes([outcome(ProbeGateOutcomeKind::NoContradictionObserved)])
            .unwrap();
        assert_eq!(inspected.gate_ids, ["sha256:gate".to_owned()]);
        assert!(matches!(
            schedule.authenticate(inspected),
            Err(ProbeGateError::HarnessBindingRequired)
        ));
    }

    #[test]
    fn the_verifier_derived_empty_schedule_authenticates_without_a_harness() {
        let schedule = ProbeGateSchedule {
            snapshot_root: "sha256:snapshot".into(),
            demand_graph_root: "sha256:graph".into(),
            gates: Vec::new(),
        };
        let inspected = schedule.inspect_outcomes([]).unwrap();
        assert!(inspected.gate_ids.is_empty());
        let verified = schedule.authenticate(inspected).unwrap();
        assert!(verified.gate_ids().is_empty());
        assert!(verified.harness_identity_fields().is_empty());
    }

    #[test]
    fn an_empty_schedule_cannot_borrow_a_harness_binding() {
        let empty = ProbeGateSchedule {
            snapshot_root: "sha256:snapshot".into(),
            demand_graph_root: "sha256:graph".into(),
            gates: Vec::new(),
        };
        let harness = BoundProbeHarnessIdentity::for_test(
            "sha256:snapshot",
            "sha256:graph",
            vec!["sha256:gate".into()],
        );
        assert_eq!(
            empty
                .authenticate_with_harness(
                    InspectedProbeGateBatch {
                        gate_ids: Vec::new()
                    },
                    &harness
                )
                .unwrap_err(),
            ProbeGateError::EmptyScheduleClaimsHarness
        );
    }

    #[test]
    fn a_harness_bound_to_another_schedule_is_not_authority() {
        let schedule = schedule();
        let inspected = schedule
            .inspect_outcomes([outcome(ProbeGateOutcomeKind::NoContradictionObserved)])
            .unwrap();
        let elsewhere = BoundProbeHarnessIdentity::for_test(
            "sha256:other-snapshot",
            "sha256:graph",
            vec!["sha256:gate".into()],
        );
        assert_eq!(
            schedule
                .authenticate_with_harness(inspected, &elsewhere)
                .unwrap_err(),
            ProbeGateError::HarnessScheduleMismatch
        );
    }

    #[test]
    fn a_verified_batch_is_refused_by_a_schedule_with_another_veto_set() {
        // Finalization re-derives the schedule from the plan it is finalizing.
        // Matching roots are not enough: the batch's authority is the gates
        // that were actually run, so a schedule whose veto set differs — even
        // over the same snapshot and demand graph — is a different plan.
        let ran = schedule();
        let inspected = ran
            .inspect_outcomes([outcome(ProbeGateOutcomeKind::NoContradictionObserved)])
            .unwrap();
        let harness = BoundProbeHarnessIdentity::for_test(
            "sha256:snapshot",
            "sha256:graph",
            vec!["sha256:gate".into()],
        );
        let verified = ran.authenticate_with_harness(inspected, &harness).unwrap();
        verified
            .verify_schedule(&ran)
            .expect("the schedule it ran accepts it");

        let mut renamed = schedule();
        renamed.gates[0].id = "sha256:another-gate".into();
        assert_eq!(
            verified.verify_schedule(&renamed).unwrap_err(),
            ProbeGateError::PlanMismatch
        );

        let mut elsewhere = schedule();
        elsewhere.snapshot_root = "sha256:other-snapshot".into();
        assert_eq!(
            verified.verify_schedule(&elsewhere).unwrap_err(),
            ProbeGateError::PlanMismatch
        );

        let mut empty = schedule();
        empty.gates.clear();
        assert_eq!(
            verified.verify_schedule(&empty).unwrap_err(),
            ProbeGateError::PlanMismatch
        );
    }

    #[test]
    fn a_bound_harness_authenticates_the_exact_schedule_it_ran() {
        let schedule = schedule();
        let inspected = schedule
            .inspect_outcomes([outcome(ProbeGateOutcomeKind::NoContradictionObserved)])
            .unwrap();
        let harness = BoundProbeHarnessIdentity::for_test(
            "sha256:snapshot",
            "sha256:graph",
            vec!["sha256:gate".into()],
        );
        let verified = schedule
            .authenticate_with_harness(inspected, &harness)
            .unwrap();
        assert_eq!(verified.gate_ids(), &["sha256:gate".to_owned()]);
        assert!(!verified.harness_identity_fields().is_empty());
    }
}
