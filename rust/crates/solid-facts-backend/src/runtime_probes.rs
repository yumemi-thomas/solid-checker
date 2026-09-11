//! Semantic runtime-probe planning and transcript evaluation.
//!
//! Node owns package acquisition for the audit path and the worker
//! implementation. This deep module owns every judgement made from worker
//! output: exact artifact/mode selection, semantic event vocabulary, isolation
//! and drain invariants, repeat consistency, and probe authority. A completed
//! finite run can witness an occurrence or falsify proposed local closure. It
//! can never prove absence, a positive minimum, a finite maximum,
//! exhaustiveness, or accepted closure.
//!
//! Inside a certification transaction the worker process is launched by
//! `contract_certification::probe_harness`, not by the Node driver: Rust
//! resolves and hashes the Node executable and the harness image against
//! compiled-in pins, copies the image and the recipe into a private directory,
//! and binds the process identity. The verdict one target's runs earn is
//! derived here — `ProbeTargetVerdict` — and consumed only by
//! `contract_certification::probe_gates`. A caller cannot construct one.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{
    ArtifactCase, ClaimIdentityError, Digest, NormalizedContract, OperationId, ResourceId,
    SemanticClaimId, SemanticClaimPath, SemanticClaimSubject,
};
use thiserror::Error;

use crate::{
    EnvironmentIdentity, PlannedProposal, ProbeClaimMaterial, ProbeObservationMaterial,
    ProbeOutcome, SandboxKind, ToolIdentity,
};

pub const PROBE_TRANSCRIPT_FORMAT: &str = "solid-checker-runtime-probe-transcript";
pub const PROBE_TRANSCRIPT_VERSION: u16 = 1;

const MAX_MODES: usize = 256;
const MAX_TARGETS: usize = 65_536;
const MAX_REPEATS: u16 = 16;
const MAX_TIMEOUT_MILLIS: u64 = 120_000;
const MAX_MICROTASK_TURNS: u16 = 4_096;
const MAX_MACROTASK_TURNS: u16 = 256;
/// Animation-frame turns are only drainable under the browser profile (ADR
/// 0033); at 60 Hz this bound is about one second of frames.
const MAX_ANIMATION_FRAME_TURNS: u16 = 64;
const MAX_EVENTS: u32 = 65_536;
const MAX_STRING_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProbeMode {
    pub name: String,
    pub artifact_case: String,
    pub environment: EnvironmentIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactModeMatrix {
    modes: Vec<ProbeMode>,
}

impl ArtifactModeMatrix {
    pub fn new(
        contract: &NormalizedContract,
        mut modes: Vec<ProbeMode>,
    ) -> Result<Self, RuntimeProbeError> {
        if modes.is_empty() || modes.len() > MAX_MODES {
            return invalid_plan("artifact mode matrix must be bounded and non-empty");
        }
        for mode in &mut modes {
            validate_string(&mode.name, "probe mode")?;
            if contract.artifact_case(&mode.artifact_case).is_none() {
                return Err(RuntimeProbeError::UnknownArtifactCase {
                    artifact_case: mode.artifact_case.clone(),
                });
            }
            validate_environment(&mut mode.environment)?;
        }
        modes.sort();
        if modes.windows(2).any(|window| {
            window[0].artifact_case == window[1].artifact_case && window[0].name == window[1].name
        }) {
            return invalid_plan("artifact case and mode pairs must be unique");
        }
        Ok(Self { modes })
    }

    #[must_use]
    pub fn modes(&self) -> &[ProbeMode] {
        &self.modes
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProbePolicy {
    pub repeat_runs: u16,
    pub timeout_millis: u64,
    pub max_microtask_turns: u16,
    pub max_macrotask_turns: u16,
    /// ADR 0033: the bound on `animation-frames` drain turns. Zero — the
    /// default for every corpus written before the browser profile — admits no
    /// such step, and a zero bound leaves every digest this policy enters
    /// byte-identical.
    pub max_animation_frame_turns: u16,
    pub max_events: u32,
}

impl ProbePolicy {
    fn validate(self) -> Result<Self, RuntimeProbeError> {
        if !(2..=MAX_REPEATS).contains(&self.repeat_runs) {
            return invalid_plan("probe policy requires between 2 and 16 repeat runs");
        }
        if self.timeout_millis == 0 || self.timeout_millis > MAX_TIMEOUT_MILLIS {
            return invalid_plan("probe timeout is outside the supported bound");
        }
        if self.max_microtask_turns > MAX_MICROTASK_TURNS
            || self.max_macrotask_turns > MAX_MACROTASK_TURNS
            || self.max_animation_frame_turns > MAX_ANIMATION_FRAME_TURNS
            || self.max_events == 0
            || self.max_events > MAX_EVENTS
        {
            return invalid_plan("probe drain or event bound exceeds policy limits");
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProbeAuthority {
    PossiblePositiveWitness,
    ClosureFalsification,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProbeScenario {
    Operation,
    CleanupLifecycle,
    RepeatedAsyncIterable,
    TransitionLifecycle,
    RequestResponseLifecycle,
    RootLifetime,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DrainStep {
    Flush,
    Microtasks {
        max_turns: u16,
    },
    Macrotasks {
        max_turns: u16,
    },
    /// Bounded `requestAnimationFrame` turns. Only the browser profile can
    /// drain these; the Node worker refuses the step as unknown, which refuses
    /// the gate rather than pretending a frame elapsed.
    AnimationFrames {
        max_turns: u16,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeEventClass {
    Call,
    Render,
    Flush,
    Callback,
    Cleanup,
    Settlement,
    Emission,
    Transition,
    Request,
    Response,
    Stream,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProbeEventMatch {
    pub marker: String,
    pub class: ProbeEventClass,
    pub operation: Option<OperationId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeRecipe {
    pub subject: SemanticClaimSubject,
    pub authority: ProbeAuthority,
    pub scenario: ProbeScenario,
    pub construction: Digest,
    pub expected_event: ProbeEventMatch,
    pub drain: Vec<DrainStep>,
    pub coverage_limitations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProbeTarget {
    claim_id: SemanticClaimId,
    subject: SemanticClaimSubject,
    authority: ProbeAuthority,
    scenario: ProbeScenario,
    recipe: Digest,
    expected_event: ProbeEventMatch,
    drain: Vec<DrainStep>,
    coverage_limitations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeSessionRequest {
    id: Digest,
    claim_id: SemanticClaimId,
    subject: SemanticClaimSubject,
    authority: ProbeAuthority,
    scenario: ProbeScenario,
    recipe: Digest,
    expected_event: ProbeEventMatch,
    drain: Vec<DrainStep>,
    mode: ProbeMode,
    repeat: u16,
    policy: ProbePolicy,
}

impl ProbeSessionRequest {
    #[must_use]
    pub const fn id(&self) -> &Digest {
        &self.id
    }

    #[must_use]
    pub const fn claim_id(&self) -> &SemanticClaimId {
        &self.claim_id
    }

    #[must_use]
    pub const fn subject(&self) -> &SemanticClaimSubject {
        &self.subject
    }

    #[must_use]
    pub const fn authority(&self) -> ProbeAuthority {
        self.authority
    }

    #[must_use]
    pub const fn scenario(&self) -> ProbeScenario {
        self.scenario
    }

    #[must_use]
    pub const fn recipe(&self) -> &Digest {
        &self.recipe
    }

    #[must_use]
    pub const fn expected_event(&self) -> &ProbeEventMatch {
        &self.expected_event
    }

    #[must_use]
    pub const fn mode(&self) -> &ProbeMode {
        &self.mode
    }

    #[must_use]
    pub const fn repeat(&self) -> u16 {
        self.repeat
    }

    #[must_use]
    pub const fn policy(&self) -> ProbePolicy {
        self.policy
    }

    #[must_use]
    pub fn drain(&self) -> &[DrainStep] {
        &self.drain
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProbePlan {
    contract: NormalizedContract,
    digest: Digest,
    policy: ProbePolicy,
    targets: Vec<ProbeTarget>,
    sessions: Vec<ProbeSessionRequest>,
}

impl RuntimeProbePlan {
    pub fn for_proposal(
        proposal: &PlannedProposal,
        matrix: ArtifactModeMatrix,
        recipes: Vec<ProbeRecipe>,
        policy: ProbePolicy,
    ) -> Result<Self, RuntimeProbeError> {
        let witness_subjects = proposal
            .plan()
            .probe_candidates()
            .iter()
            .map(|candidate| candidate.operation.semantic_subject())
            .collect();
        let closure_subjects = proposal
            .plan()
            .closure_candidates()
            .iter()
            .map(|claim| claim.semantic_subject())
            .collect();
        Self::build(
            proposal.contract().clone(),
            witness_subjects,
            closure_subjects,
            matrix,
            recipes,
            policy,
        )
    }

    pub(crate) fn build(
        contract: NormalizedContract,
        witness_subjects: BTreeSet<SemanticClaimSubject>,
        closure_subjects: BTreeSet<SemanticClaimSubject>,
        matrix: ArtifactModeMatrix,
        mut recipes: Vec<ProbeRecipe>,
        policy: ProbePolicy,
    ) -> Result<Self, RuntimeProbeError> {
        let policy = policy.validate()?;
        if recipes.is_empty() || recipes.len() > MAX_TARGETS {
            return invalid_plan("runtime probe plan must contain a bounded non-empty target set");
        }
        recipes.sort_by(|left, right| left.subject.cmp(&right.subject));
        if recipes
            .windows(2)
            .any(|window| window[0].subject == window[1].subject)
        {
            return invalid_plan("runtime probe recipes must name unique semantic subjects");
        }
        let mut targets = Vec::with_capacity(recipes.len());
        for mut recipe in recipes {
            let claim_id = contract.claim_id(&recipe.subject)?;
            match recipe.authority {
                ProbeAuthority::PossiblePositiveWitness
                    if witness_subjects.contains(&recipe.subject)
                        && matches!(recipe.subject.path, SemanticClaimPath::Operation(_)) => {}
                ProbeAuthority::ClosureFalsification
                    if closure_subjects.contains(&recipe.subject)
                        && matches!(recipe.subject.path, SemanticClaimPath::Domain(_)) => {}
                _ => {
                    return Err(RuntimeProbeError::UnplannedTarget {
                        claim_id: claim_id.as_str().into(),
                    });
                }
            }
            validate_string(&recipe.expected_event.marker, "probe event marker")?;
            if let Some(operation) = &recipe.expected_event.operation {
                let export = &contract
                    .artifact_case(&recipe.subject.artifact_case)
                    .expect("claim identity validation retained the artifact case")
                    .exports[&recipe.subject.export];
                if export.operation(&operation.0).is_none() {
                    return invalid_plan(
                        "probe event match names an operation outside the exact export",
                    );
                }
            }
            if recipe.authority == ProbeAuthority::PossiblePositiveWitness {
                let SemanticClaimPath::Operation(operation) = &recipe.subject.path else {
                    unreachable!("authority validation checked operation subject")
                };
                if recipe.expected_event.operation.as_ref() != Some(operation) {
                    return invalid_plan(
                        "possible-positive witness marker must name the planned operation",
                    );
                }
            }
            validate_drain(&recipe.drain, policy)?;
            canonicalize_strings(
                &mut recipe.coverage_limitations,
                "probe coverage limitation",
            )?;
            let recipe_digest = recipe_digest(&claim_id, &recipe);
            targets.push(ProbeTarget {
                claim_id,
                subject: recipe.subject,
                authority: recipe.authority,
                scenario: recipe.scenario,
                recipe: recipe_digest,
                expected_event: recipe.expected_event,
                drain: recipe.drain,
                coverage_limitations: recipe.coverage_limitations,
            });
        }

        let covered_cases = matrix
            .modes
            .iter()
            .map(|mode| mode.artifact_case.as_str())
            .collect::<BTreeSet<_>>();
        for target in &targets {
            if !covered_cases.contains(target.subject.artifact_case.as_str()) {
                return Err(RuntimeProbeError::UncoveredArtifactCase {
                    artifact_case: target.subject.artifact_case.clone(),
                });
            }
        }

        let digest = plan_digest(&contract, policy, &matrix, &targets);
        let mut sessions = Vec::new();
        for target in &targets {
            for mode in matrix
                .modes
                .iter()
                .filter(|mode| mode.artifact_case == target.subject.artifact_case)
            {
                for repeat in 0..policy.repeat_runs {
                    sessions.push(ProbeSessionRequest {
                        id: session_digest(&digest, &target.claim_id, mode, repeat),
                        claim_id: target.claim_id.clone(),
                        subject: target.subject.clone(),
                        authority: target.authority,
                        scenario: target.scenario,
                        recipe: target.recipe.clone(),
                        expected_event: target.expected_event.clone(),
                        drain: target.drain.clone(),
                        mode: mode.clone(),
                        repeat,
                        policy,
                    });
                }
            }
        }
        Ok(Self {
            contract,
            digest,
            policy,
            targets,
            sessions,
        })
    }

    #[must_use]
    pub const fn contract(&self) -> &NormalizedContract {
        &self.contract
    }

    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }

    #[must_use]
    pub const fn policy(&self) -> ProbePolicy {
        self.policy
    }

    #[must_use]
    pub fn sessions(&self) -> &[ProbeSessionRequest] {
        &self.sessions
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct IsolationIdentity {
    pub process: String,
    pub realm: String,
    pub module_instance: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoundaryPhase {
    Enter,
    Exit,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CleanupPhase {
    Registered,
    Produced,
    Invoked,
    Disposed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettlementState {
    Settled,
    Rejected,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransitionState {
    Active,
    Settled,
    Reverted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResponseState {
    Uncommitted,
    Committed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamState {
    Opened,
    Chunk,
    Closed,
    Cancelled,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProbeEventKind {
    Call {
        phase: BoundaryPhase,
    },
    Render {
        phase: BoundaryPhase,
    },
    Flush {
        ordinal: u16,
    },
    Callback {
        ordinal: u32,
    },
    Cleanup {
        phase: CleanupPhase,
        root_lifetime: bool,
    },
    Settlement {
        resource: Option<ResourceId>,
        state: SettlementState,
    },
    Emission {
        resource: ResourceId,
        index: u32,
    },
    Transition {
        resource: ResourceId,
        state: TransitionState,
    },
    Request {
        resource: ResourceId,
        phase: BoundaryPhase,
    },
    Response {
        resource: ResourceId,
        state: ResponseState,
    },
    Stream {
        resource: ResourceId,
        state: StreamState,
    },
}

impl ProbeEventKind {
    const fn class(&self) -> ProbeEventClass {
        match self {
            Self::Call { .. } => ProbeEventClass::Call,
            Self::Render { .. } => ProbeEventClass::Render,
            Self::Flush { .. } => ProbeEventClass::Flush,
            Self::Callback { .. } => ProbeEventClass::Callback,
            Self::Cleanup { .. } => ProbeEventClass::Cleanup,
            Self::Settlement { .. } => ProbeEventClass::Settlement,
            Self::Emission { .. } => ProbeEventClass::Emission,
            Self::Transition { .. } => ProbeEventClass::Transition,
            Self::Request { .. } => ProbeEventClass::Request,
            Self::Response { .. } => ProbeEventClass::Response,
            Self::Stream { .. } => ProbeEventClass::Stream,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProbeEvent {
    pub sequence: u32,
    pub marker: String,
    pub operation: Option<OperationId>,
    pub kind: ProbeEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProbeRunOutcome {
    Completed {
        events: Vec<ProbeEvent>,
    },
    Error {
        details: Digest,
        /// The worker's bounded one-line summary of what was thrown, when the
        /// harness reported one. It explains an incomplete veto in the
        /// withheld record and goes nowhere else: evidence keeps the digest.
        summary: Option<String>,
    },
    Timeout,
    Refused {
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeRun {
    pub session: Digest,
    pub environment: EnvironmentIdentity,
    pub isolation: IsolationIdentity,
    pub drained_microtasks: u16,
    pub drained_macrotasks: u16,
    /// Zero for every Node worker frame; the browser bootstrap reports what it
    /// actually drained under an `animation-frames` step.
    pub drained_animation_frames: u16,
    pub outcome: ProbeRunOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeTranscript {
    claim_id: SemanticClaimId,
    mode: String,
    digest: Digest,
    bytes: Vec<u8>,
}

impl ProbeTranscript {
    #[must_use]
    pub const fn claim_id(&self) -> &SemanticClaimId {
        &self.claim_id
    }

    #[must_use]
    pub fn mode(&self) -> &str {
        &self.mode
    }

    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeContradictionRecord {
    pub claim_id: SemanticClaimId,
    pub subject: SemanticClaimSubject,
    pub mode: String,
    pub transcript: Digest,
}

/// What one probe target's complete set of isolated repeat runs, across every
/// covered mode, established about the proposed closure it vetoes.
///
/// This is the *only* channel through which a probe result reaches gate
/// authority, and it is derived here from runs this module validated in the
/// same call. `CleanNonObservation` says exactly that a complete, isolated,
/// deterministic, scenario-satisfying execution did not observe the
/// contradiction the recipe was written to provoke. It is not evidence of
/// absence, and it never establishes closure: the Type Facts
/// `DomainExhaustiveness` witness does that, and this verdict can only veto.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProbeTargetVerdict {
    /// A run observed the behavior the proposed closure denies.
    Contradiction,
    /// Every mode completed cleanly and observed no contradiction.
    CleanNonObservation,
    /// A run was refused, errored, timed out, or was never returned.
    Incomplete,
    /// A possible-positive witness target. Witness targets veto nothing, so
    /// they never satisfy or refuse a mandatory gate.
    NotAGate,
}

impl ProbeTargetVerdict {
    /// Combines the verdicts of one target's modes. A contradiction anywhere
    /// vetoes; otherwise one incomplete mode makes the whole target
    /// incomplete. Silence is never promoted over either.
    fn join(self, other: Self) -> Self {
        match (self, other) {
            // Being a witness rather than a veto is a property of the target,
            // not of its runs, so it survives every mode outcome.
            (Self::NotAGate, _) | (_, Self::NotAGate) => Self::NotAGate,
            (Self::Contradiction, _) | (_, Self::Contradiction) => Self::Contradiction,
            (Self::Incomplete, _) | (_, Self::Incomplete) => Self::Incomplete,
            (Self::CleanNonObservation, Self::CleanNonObservation) => Self::CleanNonObservation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProbeEvaluation {
    materials: Vec<ProbeClaimMaterial>,
    transcripts: Vec<ProbeTranscript>,
    contradictions: Vec<ProbeContradictionRecord>,
    verdicts: BTreeMap<SemanticClaimId, ProbeTargetVerdict>,
    /// Why each `Incomplete` verdict is incomplete, one line per mode that
    /// did not complete: the timeout budget, the worker's failure summary, or
    /// the refusal reason. Read by the certifier to explain a withheld
    /// candidate (ADR 0036); never serialized into evidence.
    incompletions: BTreeMap<SemanticClaimId, String>,
}

impl RuntimeProbeEvaluation {
    #[must_use]
    pub fn claim_material(&self) -> &[ProbeClaimMaterial] {
        &self.materials
    }

    #[must_use]
    pub fn transcripts(&self) -> &[ProbeTranscript] {
        &self.transcripts
    }

    #[must_use]
    pub fn contradictions(&self) -> &[ProbeContradictionRecord] {
        &self.contradictions
    }

    /// The verdict this evaluation established for one exact semantic claim,
    /// or `None` when the claim was not a probe target at all.
    pub(crate) fn verdict(&self, claim_id: &SemanticClaimId) -> Option<ProbeTargetVerdict> {
        self.verdicts.get(claim_id).copied()
    }

    /// Why the claim's verdict is `Incomplete`, or `None` when it is not.
    #[must_use]
    pub fn incompletion(&self, claim_id: &str) -> Option<&str> {
        self.incompletions
            .iter()
            .find(|(id, _)| id.as_str() == claim_id)
            .map(|(_, reason)| reason.as_str())
    }
}

#[derive(Debug, Error)]
pub enum RuntimeProbeError {
    #[error("semantic claim identity is invalid: {0}")]
    Claim(#[from] ClaimIdentityError),
    #[error("runtime probe plan is invalid: {reason}")]
    InvalidPlan { reason: String },
    #[error("runtime probe mode names missing artifact case {artifact_case}")]
    UnknownArtifactCase { artifact_case: String },
    #[error("runtime probe plan does not cover artifact case {artifact_case}")]
    UncoveredArtifactCase { artifact_case: String },
    #[error("runtime probe target {claim_id} is not authorized by the proposal plan")]
    UnplannedTarget { claim_id: String },
    #[error("runtime probe response names an unknown session {session}")]
    UnknownSession { session: String },
    #[error("runtime probe response repeats session {session}")]
    DuplicateSession { session: String },
    #[error("runtime probe transcript emission failed: {message}")]
    Emission { message: String },
}

pub fn evaluate_runtime_probes(
    plan: &RuntimeProbePlan,
    runs: Vec<ProbeRun>,
    producer: ToolIdentity,
) -> Result<RuntimeProbeEvaluation, RuntimeProbeError> {
    validate_tool(&producer)?;
    let requests = plan
        .sessions
        .iter()
        .map(|session| (session.id.clone(), session))
        .collect::<BTreeMap<_, _>>();
    let mut supplied = BTreeMap::new();
    for run in runs {
        let session_id = run.session.clone();
        let Some(_) = requests.get(&run.session) else {
            return Err(RuntimeProbeError::UnknownSession {
                session: run.session.as_str().into(),
            });
        };
        if supplied.insert(session_id.clone(), run).is_some() {
            return Err(RuntimeProbeError::DuplicateSession {
                session: session_id.as_str().into(),
            });
        }
    }

    let isolation_collisions = isolation_collisions(supplied.values());
    let mut materials = Vec::new();
    let mut transcripts = Vec::new();
    let mut contradictions = Vec::new();
    let mut verdicts = BTreeMap::<SemanticClaimId, ProbeTargetVerdict>::new();
    let mut incompletions = BTreeMap::<SemanticClaimId, String>::new();
    for target in &plan.targets {
        let mut observations = Vec::new();
        let modes = plan
            .sessions
            .iter()
            .filter(|session| session.claim_id == target.claim_id)
            .map(|session| session.mode.clone())
            .collect::<BTreeSet<_>>();
        for mode in modes {
            let sessions = plan
                .sessions
                .iter()
                .filter(|session| session.claim_id == target.claim_id && session.mode == mode)
                .collect::<Vec<_>>();
            let mode_runs = sessions
                .iter()
                .map(|session| supplied.get(&session.id).map(|run| (*session, run)))
                .collect::<Vec<_>>();
            let evaluated = evaluate_mode(plan, target, &mode, &mode_runs, &isolation_collisions)?;
            verdicts
                .entry(target.claim_id.clone())
                .and_modify(|existing| *existing = existing.join(evaluated.verdict))
                .or_insert(evaluated.verdict);
            if evaluated.verdict == ProbeTargetVerdict::Incomplete
                && let Some(reason) = &evaluated.incompletion
            {
                let line = format!("{}: {reason}", mode.name);
                incompletions
                    .entry(target.claim_id.clone())
                    .and_modify(|existing| {
                        if !existing.split("; ").any(|known| known == line) {
                            existing.push_str("; ");
                            existing.push_str(&line);
                        }
                    })
                    .or_insert(line);
            }
            if let Some(transcript) = evaluated.transcript {
                if matches!(evaluated.outcome, ProbeOutcome::Falsification { .. }) {
                    contradictions.push(ProbeContradictionRecord {
                        claim_id: target.claim_id.clone(),
                        subject: target.subject.clone(),
                        mode: mode.name.clone(),
                        transcript: transcript.digest.clone(),
                    });
                }
                transcripts.push(transcript);
            }
            observations.push(ProbeObservationMaterial {
                mode: mode.name,
                environment: mode.environment,
                outcome: evaluated.outcome,
            });
        }
        materials.push(ProbeClaimMaterial {
            subject: target.subject.clone(),
            producer: producer.clone(),
            recipe: target.recipe.clone(),
            observations,
            coverage_limitations: target.coverage_limitations.clone(),
        });
    }
    materials.sort_by(|left, right| left.subject.cmp(&right.subject));
    transcripts
        .sort_by(|left, right| (&left.claim_id, &left.mode).cmp(&(&right.claim_id, &right.mode)));
    contradictions
        .sort_by(|left, right| (&left.claim_id, &left.mode).cmp(&(&right.claim_id, &right.mode)));
    Ok(RuntimeProbeEvaluation {
        materials,
        transcripts,
        contradictions,
        verdicts,
        incompletions,
    })
}

struct EvaluatedMode {
    outcome: ProbeOutcome,
    transcript: Option<ProbeTranscript>,
    verdict: ProbeTargetVerdict,
    /// Why `verdict` is `Incomplete`; `None` for every other verdict.
    incompletion: Option<String>,
}

fn evaluate_mode(
    plan: &RuntimeProbePlan,
    target: &ProbeTarget,
    mode: &ProbeMode,
    runs: &[Option<(&ProbeSessionRequest, &ProbeRun)>],
    isolation_collisions: &BTreeSet<Digest>,
) -> Result<EvaluatedMode, RuntimeProbeError> {
    let missing = runs.iter().filter(|run| run.is_none()).count();
    if missing > 0 {
        return Ok(refused_mode(format!(
            "{missing} of {} deterministic repeat runs returned no transcript",
            runs.len()
        )));
    }
    let runs = runs
        .iter()
        .map(|run| run.expect("missing runs returned above"))
        .collect::<Vec<_>>();
    if runs
        .iter()
        .any(|(_, run)| isolation_collisions.contains(&run.session))
    {
        return Ok(refused_mode(
            "repeat runs reused process, realm, or module-instance state",
        ));
    }
    for (session, run) in &runs {
        if canonical_environment(run.environment.clone())?
            != canonical_environment(session.mode.environment.clone())?
        {
            return Ok(refused_mode(
                "worker environment does not match the exact artifact-mode matrix",
            ));
        }
        let (microtask_limit, macrotask_limit, animation_frame_limit) =
            drain_limits(&session.drain);
        if run.drained_microtasks > microtask_limit
            || run.drained_macrotasks > macrotask_limit
            || run.drained_animation_frames > animation_frame_limit
        {
            return Ok(refused_mode(
                "worker exceeded the recipe's bounded semantic drain",
            ));
        }
        validate_isolation(&run.isolation)?;
    }

    if runs
        .iter()
        .any(|(_, run)| matches!(run.outcome, ProbeRunOutcome::Timeout))
    {
        return Ok(EvaluatedMode {
            outcome: ProbeOutcome::Timeout {
                limit_millis: plan.policy.timeout_millis,
            },
            transcript: None,
            verdict: ProbeTargetVerdict::Incomplete,
            incompletion: Some(format!(
                "the worker did not report within the policy budget of {} ms",
                plan.policy.timeout_millis
            )),
        });
    }
    if let Some((details, summary)) = runs.iter().find_map(|(_, run)| match &run.outcome {
        ProbeRunOutcome::Error { details, summary } => Some((details.clone(), summary.clone())),
        _ => None,
    }) {
        if let Some(summary) = &summary {
            validate_string(summary, "probe error summary")?;
        }
        return Ok(EvaluatedMode {
            outcome: ProbeOutcome::Error { details },
            transcript: None,
            verdict: ProbeTargetVerdict::Incomplete,
            incompletion: Some(summary.map_or_else(
                || "the worker threw (no summary reported)".to_owned(),
                |summary| {
                    unresolvable_package_incompletion(&summary)
                        .unwrap_or_else(|| format!("the worker threw: {summary}"))
                },
            )),
        });
    }
    if let Some(reason) = runs.iter().find_map(|(_, run)| match &run.outcome {
        ProbeRunOutcome::Refused { reason } => Some(reason.clone()),
        _ => None,
    }) {
        validate_string(&reason, "probe refusal reason")?;
        return Ok(refused_mode(reason));
    }

    let completed = runs
        .iter()
        .map(|(_, run)| match &run.outcome {
            ProbeRunOutcome::Completed { events } => Ok(events),
            _ => invalid_plan("unclassified runtime probe outcome"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    for events in &completed {
        validate_events(events, plan.policy.max_events)?;
    }
    if completed.windows(2).any(|window| window[0] != window[1]) {
        return Ok(refused_mode(
            "semantic event transcripts differ across isolated repeat runs",
        ));
    }
    let events = completed[0];
    if !scenario_satisfied(target.scenario, events) {
        return Ok(refused_mode(
            "semantic event transcript does not satisfy the scenario lifecycle",
        ));
    }
    if !events
        .iter()
        .any(|event| event_matches(event, &target.expected_event))
    {
        // A closure-falsification recipe's marker *is* the contradiction it
        // was written to provoke. Not seeing it in a complete, isolated,
        // deterministic, scenario-satisfying execution is therefore a clean
        // pass of the mandatory veto — not a refusal of the run. It is still
        // not evidence: the observation stays `Refused` in probe evidence
        // material, because finite silence can never support a claim. Only
        // the separate gate verdict records that nothing contradicted.
        let refused = refused_mode("finite execution did not witness the planned positive marker");
        return Ok(match target.authority {
            ProbeAuthority::ClosureFalsification => EvaluatedMode {
                verdict: ProbeTargetVerdict::CleanNonObservation,
                incompletion: None,
                ..refused
            },
            ProbeAuthority::PossiblePositiveWitness => EvaluatedMode {
                verdict: ProbeTargetVerdict::Incomplete,
                ..refused
            },
        });
    }

    let transcript = emit_transcript(plan, target, mode, &runs)?;
    let (outcome, verdict) = match target.authority {
        ProbeAuthority::PossiblePositiveWitness => (
            ProbeOutcome::Witness {
                transcript: transcript.digest.clone(),
            },
            ProbeTargetVerdict::NotAGate,
        ),
        ProbeAuthority::ClosureFalsification => (
            ProbeOutcome::Falsification {
                transcript: transcript.digest.clone(),
            },
            ProbeTargetVerdict::Contradiction,
        ),
    };
    Ok(EvaluatedMode {
        outcome,
        transcript: Some(transcript),
        verdict,
        incompletion: None,
    })
}

/// A worker throw that is really a missing authenticated dependency, named.
///
/// The private workspace carries this transaction's authenticated dependency
/// closure and nothing else, and `require_authenticated_dependency_closure`
/// checks it against the **analyzed package's own** declared dependencies --
/// one level. A package the artifact case reaches *transitively* is neither
/// declared by the analyzed package nor copied in, so it passes that check and
/// then fails at import as a bare `ERR_MODULE_NOT_FOUND`.
///
/// Measured on six rows whose artifact cases name no conditions: Node's own
/// `node` condition selects `solid-js/web/dist/server.js`, which imports
/// `seroval`, which no one declared (§ 51 of
/// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
///
/// This names it rather than deepening the precheck, and the distinction
/// matters: a precheck walking *declared* dependencies transitively would
/// refuse gates that work today, because what a case imports is a subset of
/// what its closure declares -- `web.js` needs no `seroval` even though
/// `server.js` does. Naming the package that actually failed to resolve
/// cannot over-refuse, because the resolution already failed.
///
/// The outcome is unchanged either way: an incomplete veto withholds its
/// candidate. Only the reason improves.
fn unresolvable_package_incompletion(summary: &str) -> Option<String> {
    let rest = summary.split_once("Cannot find package ")?.1;
    let quote = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let name = rest[quote.len_utf8()..].split(quote).next()?;
    if name.is_empty() || name.len() > 214 {
        return None;
    }
    Some(format!(
        "the probe worker could not resolve {name:?}: the private workspace carries only this \
         transaction's authenticated dependency closure, and {name:?} is reached transitively \
         rather than declared by the analyzed package"
    ))
}

fn refused_mode(reason: impl Into<String>) -> EvaluatedMode {
    let reason = reason.into();
    EvaluatedMode {
        incompletion: Some(format!("the run was refused: {reason}")),
        outcome: ProbeOutcome::Refused { reason },
        transcript: None,
        verdict: ProbeTargetVerdict::Incomplete,
    }
}

fn event_matches(event: &ProbeEvent, expected: &ProbeEventMatch) -> bool {
    event.marker == expected.marker
        && event.kind.class() == expected.class
        && match &expected.operation {
            Some(operation) => event.operation.as_ref() == Some(operation),
            None => true,
        }
}

fn scenario_satisfied(scenario: ProbeScenario, events: &[ProbeEvent]) -> bool {
    match scenario {
        ProbeScenario::Operation => true,
        ProbeScenario::CleanupLifecycle => ordered_cleanup(events),
        ProbeScenario::RepeatedAsyncIterable => repeated_async_iterable(events),
        ProbeScenario::TransitionLifecycle => ordered_transition(events),
        ProbeScenario::RequestResponseLifecycle => ordered_request_response(events),
        ProbeScenario::RootLifetime => events.iter().any(|event| {
            matches!(
                event.kind,
                ProbeEventKind::Cleanup {
                    phase: CleanupPhase::Invoked | CleanupPhase::Disposed,
                    root_lifetime: true
                }
            )
        }),
    }
}

fn ordered_cleanup(events: &[ProbeEvent]) -> bool {
    events.iter().enumerate().any(|(index, event)| {
        if !matches!(
            event.kind,
            ProbeEventKind::Cleanup {
                phase: CleanupPhase::Registered | CleanupPhase::Produced,
                ..
            }
        ) {
            return false;
        }
        events[index + 1..].iter().any(|later| {
            matches!(
                later.kind,
                ProbeEventKind::Cleanup {
                    phase: CleanupPhase::Invoked | CleanupPhase::Disposed,
                    ..
                }
            )
        })
    })
}

fn repeated_async_iterable(events: &[ProbeEvent]) -> bool {
    let mut emissions = BTreeMap::<&ResourceId, Vec<(u32, u32)>>::new();
    for event in events {
        if let ProbeEventKind::Emission { resource, index } = &event.kind {
            emissions
                .entry(resource)
                .or_default()
                .push((event.sequence, *index));
        }
    }
    emissions.into_iter().any(|(resource, values)| {
        values.len() >= 2
            && values
                .iter()
                .enumerate()
                .all(|(expected, (_, index))| usize::try_from(*index) == Ok(expected))
            && events.iter().any(|event| {
                event.sequence > values.last().expect("two emissions exist").0
                    && matches!(
                        &event.kind,
                        ProbeEventKind::Settlement {
                            resource: Some(actual),
                            ..
                        } if actual == resource
                    )
            })
    })
}

fn ordered_transition(events: &[ProbeEvent]) -> bool {
    events.iter().enumerate().any(|(index, event)| {
        let ProbeEventKind::Transition {
            resource,
            state: TransitionState::Active,
        } = &event.kind
        else {
            return false;
        };
        events[index + 1..].iter().any(|later| {
            matches!(
                &later.kind,
                ProbeEventKind::Transition {
                    resource: actual,
                    state: TransitionState::Settled | TransitionState::Reverted,
                } if actual == resource
            )
        })
    })
}

fn ordered_request_response(events: &[ProbeEvent]) -> bool {
    events.iter().enumerate().any(|(index, event)| {
        let ProbeEventKind::Request {
            resource,
            phase: BoundaryPhase::Enter,
        } = &event.kind
        else {
            return false;
        };
        let remainder = &events[index + 1..];
        let Some(uncommitted) = remainder.iter().position(|later| {
            matches!(
                &later.kind,
                ProbeEventKind::Response {
                    resource: actual,
                    state: ResponseState::Uncommitted,
                } if actual == resource
            )
        }) else {
            return false;
        };
        remainder[uncommitted + 1..].iter().any(|later| {
            matches!(
                &later.kind,
                ProbeEventKind::Response {
                    resource: actual,
                    state: ResponseState::Committed,
                } if actual == resource
            )
        })
    })
}

fn validate_events(events: &[ProbeEvent], max_events: u32) -> Result<(), RuntimeProbeError> {
    if events.is_empty() || u32::try_from(events.len()).map_or(true, |len| len > max_events) {
        return invalid_plan("semantic event transcript is empty or exceeds its event bound");
    }
    for (expected, event) in events.iter().enumerate() {
        if usize::try_from(event.sequence) != Ok(expected) {
            return invalid_plan("semantic event sequence must be contiguous and zero-based");
        }
        validate_string(&event.marker, "probe event marker")?;
        if let Some(operation) = &event.operation {
            validate_string(&operation.0, "probe event operation")?;
        }
        match &event.kind {
            ProbeEventKind::Settlement {
                resource: Some(resource),
                ..
            }
            | ProbeEventKind::Emission { resource, .. }
            | ProbeEventKind::Transition { resource, .. }
            | ProbeEventKind::Request { resource, .. }
            | ProbeEventKind::Response { resource, .. }
            | ProbeEventKind::Stream { resource, .. } => {
                validate_string(&resource.0, "probe event resource")?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn isolation_collisions<'a>(runs: impl Iterator<Item = &'a ProbeRun>) -> BTreeSet<Digest> {
    let runs = runs.collect::<Vec<_>>();
    let mut colliding = BTreeSet::new();
    for (index, left) in runs.iter().enumerate() {
        for right in &runs[index + 1..] {
            if left.isolation.process == right.isolation.process
                || left.isolation.realm == right.isolation.realm
                || left.isolation.module_instance == right.isolation.module_instance
            {
                colliding.insert(left.session.clone());
                colliding.insert(right.session.clone());
            }
        }
    }
    colliding
}

fn validate_isolation(identity: &IsolationIdentity) -> Result<(), RuntimeProbeError> {
    validate_string(&identity.process, "probe process identity")?;
    validate_string(&identity.realm, "probe realm identity")?;
    validate_string(&identity.module_instance, "probe module-instance identity")
}

fn validate_drain(drain: &[DrainStep], policy: ProbePolicy) -> Result<(), RuntimeProbeError> {
    if drain.is_empty() || drain.len() > 1_024 {
        return invalid_plan("probe recipe must contain a bounded semantic drain plan");
    }
    let mut microtasks = 0_u32;
    let mut macrotasks = 0_u32;
    let mut animation_frames = 0_u32;
    for step in drain {
        match step {
            DrainStep::Flush => {}
            DrainStep::Microtasks { max_turns } if *max_turns > 0 => {
                microtasks += u32::from(*max_turns);
            }
            DrainStep::Macrotasks { max_turns } if *max_turns > 0 => {
                macrotasks += u32::from(*max_turns);
            }
            DrainStep::AnimationFrames { max_turns } if *max_turns > 0 => {
                animation_frames += u32::from(*max_turns);
            }
            _ => return invalid_plan("drain steps must have non-zero semantic turn bounds"),
        }
    }
    if microtasks > u32::from(policy.max_microtask_turns)
        || macrotasks > u32::from(policy.max_macrotask_turns)
        || animation_frames > u32::from(policy.max_animation_frame_turns)
    {
        return invalid_plan("probe recipe exceeds the configured drain policy");
    }
    Ok(())
}

fn drain_limits(drain: &[DrainStep]) -> (u16, u16, u16) {
    let mut microtasks = 0_u16;
    let mut macrotasks = 0_u16;
    let mut animation_frames = 0_u16;
    for step in drain {
        match step {
            DrainStep::Flush => {}
            DrainStep::Microtasks { max_turns } => {
                microtasks = microtasks.saturating_add(*max_turns);
            }
            DrainStep::Macrotasks { max_turns } => {
                macrotasks = macrotasks.saturating_add(*max_turns);
            }
            DrainStep::AnimationFrames { max_turns } => {
                animation_frames = animation_frames.saturating_add(*max_turns);
            }
        }
    }
    (microtasks, macrotasks, animation_frames)
}

fn validate_environment(environment: &mut EnvironmentIdentity) -> Result<(), RuntimeProbeError> {
    validate_tool(&environment.runtime)?;
    validate_string(&environment.os, "environment operating system")?;
    validate_string(&environment.architecture, "environment architecture")?;
    canonicalize_strings(&mut environment.conditions, "environment condition")?;
    match (environment.sandbox.kind, &environment.sandbox.policy) {
        (SandboxKind::None, None) => Ok(()),
        (SandboxKind::None, Some(_)) => {
            invalid_plan("an unsandboxed environment cannot name a sandbox policy")
        }
        (_, Some(_)) => Ok(()),
        (_, None) => invalid_plan("a sandboxed environment requires a policy digest"),
    }
}

fn canonical_environment(
    mut environment: EnvironmentIdentity,
) -> Result<EnvironmentIdentity, RuntimeProbeError> {
    validate_environment(&mut environment)?;
    Ok(environment)
}

fn validate_tool(tool: &ToolIdentity) -> Result<(), RuntimeProbeError> {
    validate_string(&tool.name, "tool name")?;
    validate_string(&tool.version, "tool version")?;
    if let Some(protocol) = &tool.protocol {
        validate_string(protocol, "tool protocol")?;
    }
    Ok(())
}

fn validate_string(value: &str, field: &str) -> Result<(), RuntimeProbeError> {
    if value.is_empty() || value.len() > MAX_STRING_BYTES {
        invalid_plan(format!(
            "{field} must contain between 1 and {MAX_STRING_BYTES} bytes"
        ))
    } else {
        Ok(())
    }
}

fn canonicalize_strings(values: &mut Vec<String>, field: &str) -> Result<(), RuntimeProbeError> {
    if values.len() > 16_384 {
        return invalid_plan(format!("{field} count exceeds the limit"));
    }
    for value in values.iter() {
        validate_string(value, field)?;
    }
    values.sort();
    values.dedup();
    Ok(())
}

fn invalid_plan<T>(reason: impl Into<String>) -> Result<T, RuntimeProbeError> {
    Err(RuntimeProbeError::InvalidPlan {
        reason: reason.into(),
    })
}

fn hash_field(hasher: &mut Sha256, value: &str) {
    hasher.update(value.len().to_be_bytes());
    hasher.update(value.as_bytes());
}

fn digest_hasher(hasher: Sha256) -> Digest {
    Digest::parse(format!("sha256:{:x}", hasher.finalize()))
        .expect("SHA-256 formatting is canonical")
}

fn recipe_digest(claim_id: &SemanticClaimId, recipe: &ProbeRecipe) -> Digest {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, "solid-checker-probe-recipe-v1");
    hash_field(&mut hasher, claim_id.as_str());
    hash_field(&mut hasher, authority_name(recipe.authority));
    hash_field(&mut hasher, scenario_name(recipe.scenario));
    hash_field(&mut hasher, recipe.construction.as_str());
    hash_field(&mut hasher, &recipe.expected_event.marker);
    hash_field(&mut hasher, event_class_name(recipe.expected_event.class));
    hash_field(
        &mut hasher,
        recipe
            .expected_event
            .operation
            .as_ref()
            .map_or("", |operation| operation.0.as_str()),
    );
    for step in &recipe.drain {
        match step {
            DrainStep::Flush => hash_field(&mut hasher, "flush"),
            DrainStep::Microtasks { max_turns } => {
                hash_field(&mut hasher, "microtasks");
                hash_field(&mut hasher, &max_turns.to_string());
            }
            DrainStep::Macrotasks { max_turns } => {
                hash_field(&mut hasher, "macrotasks");
                hash_field(&mut hasher, &max_turns.to_string());
            }
            DrainStep::AnimationFrames { max_turns } => {
                hash_field(&mut hasher, "animation-frames");
                hash_field(&mut hasher, &max_turns.to_string());
            }
        }
    }
    for limitation in &recipe.coverage_limitations {
        hash_field(&mut hasher, limitation);
    }
    digest_hasher(hasher)
}

fn plan_digest(
    contract: &NormalizedContract,
    policy: ProbePolicy,
    matrix: &ArtifactModeMatrix,
    targets: &[ProbeTarget],
) -> Digest {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, "solid-checker-runtime-probe-plan-v1");
    hash_field(&mut hasher, contract.semantic_digest().as_str());
    hash_field(&mut hasher, &policy.repeat_runs.to_string());
    hash_field(&mut hasher, &policy.timeout_millis.to_string());
    hash_field(&mut hasher, &policy.max_microtask_turns.to_string());
    hash_field(&mut hasher, &policy.max_macrotask_turns.to_string());
    hash_field(&mut hasher, &policy.max_events.to_string());
    // Appended only when nonzero, so every plan digest computed before ADR
    // 0033 — and every receipt binding one — is byte-identical.
    if policy.max_animation_frame_turns > 0 {
        hash_field(&mut hasher, "max-animation-frame-turns");
        hash_field(&mut hasher, &policy.max_animation_frame_turns.to_string());
    }
    for mode in &matrix.modes {
        hash_field(&mut hasher, &mode.artifact_case);
        hash_field(&mut hasher, &mode.name);
        hash_environment(&mut hasher, &mode.environment);
    }
    for target in targets {
        hash_field(&mut hasher, target.claim_id.as_str());
        hash_field(&mut hasher, target.recipe.as_str());
    }
    digest_hasher(hasher)
}

fn session_digest(plan: &Digest, claim: &SemanticClaimId, mode: &ProbeMode, repeat: u16) -> Digest {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, "solid-checker-runtime-probe-session-v1");
    hash_field(&mut hasher, plan.as_str());
    hash_field(&mut hasher, claim.as_str());
    hash_field(&mut hasher, &mode.artifact_case);
    hash_field(&mut hasher, &mode.name);
    hash_field(&mut hasher, &repeat.to_string());
    digest_hasher(hasher)
}

fn hash_environment(hasher: &mut Sha256, environment: &EnvironmentIdentity) {
    hash_field(hasher, &environment.runtime.name);
    hash_field(hasher, &environment.runtime.version);
    hash_field(hasher, environment.runtime.build.as_str());
    hash_field(
        hasher,
        environment.runtime.protocol.as_deref().unwrap_or_default(),
    );
    hash_field(hasher, &environment.os);
    hash_field(hasher, &environment.architecture);
    for condition in &environment.conditions {
        hash_field(hasher, condition);
    }
    hash_field(hasher, sandbox_name(environment.sandbox.kind));
    hash_field(
        hasher,
        environment
            .sandbox
            .policy
            .as_ref()
            .map_or("", Digest::as_str),
    );
}

fn authority_name(authority: ProbeAuthority) -> &'static str {
    match authority {
        ProbeAuthority::PossiblePositiveWitness => "possible-positive-witness",
        ProbeAuthority::ClosureFalsification => "closure-falsification",
    }
}

fn scenario_name(scenario: ProbeScenario) -> &'static str {
    match scenario {
        ProbeScenario::Operation => "operation",
        ProbeScenario::CleanupLifecycle => "cleanup-lifecycle",
        ProbeScenario::RepeatedAsyncIterable => "repeated-async-iterable",
        ProbeScenario::TransitionLifecycle => "transition-lifecycle",
        ProbeScenario::RequestResponseLifecycle => "request-response-lifecycle",
        ProbeScenario::RootLifetime => "root-lifetime",
    }
}

fn event_class_name(class: ProbeEventClass) -> &'static str {
    match class {
        ProbeEventClass::Call => "call",
        ProbeEventClass::Render => "render",
        ProbeEventClass::Flush => "flush",
        ProbeEventClass::Callback => "callback",
        ProbeEventClass::Cleanup => "cleanup",
        ProbeEventClass::Settlement => "settlement",
        ProbeEventClass::Emission => "emission",
        ProbeEventClass::Transition => "transition",
        ProbeEventClass::Request => "request",
        ProbeEventClass::Response => "response",
        ProbeEventClass::Stream => "stream",
    }
}

fn sandbox_name(kind: SandboxKind) -> &'static str {
    match kind {
        SandboxKind::None => "none",
        SandboxKind::Process => "process",
        SandboxKind::Container => "container",
        SandboxKind::VirtualMachine => "virtual-machine",
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireTranscript<'a> {
    format: &'static str,
    transcript_version: u16,
    semantic_model_version: u16,
    semantic_digest: &'a str,
    plan_digest: &'a str,
    claim_id: &'a str,
    artifact: WireTranscriptArtifact<'a>,
    export: &'a str,
    authority: &'static str,
    scenario: &'static str,
    recipe: &'a str,
    mode: &'a str,
    environment: WireTranscriptEnvironment<'a>,
    runs: Vec<WireTranscriptRun<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireTranscriptArtifact<'a> {
    case: &'a str,
    entrypoint: &'a str,
    runtime_path: &'a str,
    runtime_digest: &'a str,
    declaration_path: &'a str,
    declaration_digest: &'a str,
    closure: &'a str,
    transform_path: Option<&'a str>,
    transform_digest: Option<&'a str>,
}

impl<'a> WireTranscriptArtifact<'a> {
    fn new(case: &'a ArtifactCase) -> Self {
        Self {
            case: &case.id,
            entrypoint: &case.entrypoint,
            runtime_path: &case.runtime.path,
            runtime_digest: case.runtime.digest.as_str(),
            declaration_path: &case.declarations.path,
            declaration_digest: case.declarations.digest.as_str(),
            closure: case.dependency_closure.as_str(),
            transform_path: case
                .transform
                .as_ref()
                .map(|artifact| artifact.path.as_str()),
            transform_digest: case
                .transform
                .as_ref()
                .map(|artifact| artifact.digest.as_str()),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireTranscriptEnvironment<'a> {
    runtime_name: &'a str,
    runtime_version: &'a str,
    runtime_build: &'a str,
    runtime_protocol: Option<&'a str>,
    os: &'a str,
    architecture: &'a str,
    conditions: &'a [String],
    sandbox: &'static str,
    sandbox_policy: Option<&'a str>,
}

impl<'a> WireTranscriptEnvironment<'a> {
    fn new(environment: &'a EnvironmentIdentity) -> Self {
        Self {
            runtime_name: &environment.runtime.name,
            runtime_version: &environment.runtime.version,
            runtime_build: environment.runtime.build.as_str(),
            runtime_protocol: environment.runtime.protocol.as_deref(),
            os: &environment.os,
            architecture: &environment.architecture,
            conditions: &environment.conditions,
            sandbox: sandbox_name(environment.sandbox.kind),
            sandbox_policy: environment.sandbox.policy.as_ref().map(Digest::as_str),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireTranscriptRun<'a> {
    session: &'a str,
    repeat: u16,
    process: &'a str,
    realm: &'a str,
    module_instance: &'a str,
    drained_microtasks: u16,
    drained_macrotasks: u16,
    /// Omitted at zero so every transcript written before ADR 0033 keeps its
    /// digest.
    #[serde(skip_serializing_if = "is_zero_u16")]
    drained_animation_frames: u16,
    events: Vec<WireProbeEvent<'a>>,
}

fn is_zero_u16(value: &u16) -> bool {
    *value == 0
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireProbeEvent<'a> {
    sequence: u32,
    marker: &'a str,
    operation: Option<&'a str>,
    #[serde(flatten)]
    kind: WireProbeEventKind<'a>,
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum WireProbeEventKind<'a> {
    Call {
        phase: BoundaryPhase,
    },
    Render {
        phase: BoundaryPhase,
    },
    Flush {
        ordinal: u16,
    },
    Callback {
        ordinal: u32,
    },
    Cleanup {
        phase: CleanupPhase,
        root_lifetime: bool,
    },
    Settlement {
        resource: Option<&'a str>,
        state: SettlementState,
    },
    Emission {
        resource: &'a str,
        index: u32,
    },
    Transition {
        resource: &'a str,
        state: TransitionState,
    },
    Request {
        resource: &'a str,
        phase: BoundaryPhase,
    },
    Response {
        resource: &'a str,
        state: ResponseState,
    },
    Stream {
        resource: &'a str,
        state: StreamState,
    },
}

impl<'a> From<&'a ProbeEvent> for WireProbeEvent<'a> {
    fn from(event: &'a ProbeEvent) -> Self {
        let kind = match &event.kind {
            ProbeEventKind::Call { phase } => WireProbeEventKind::Call { phase: *phase },
            ProbeEventKind::Render { phase } => WireProbeEventKind::Render { phase: *phase },
            ProbeEventKind::Flush { ordinal } => WireProbeEventKind::Flush { ordinal: *ordinal },
            ProbeEventKind::Callback { ordinal } => {
                WireProbeEventKind::Callback { ordinal: *ordinal }
            }
            ProbeEventKind::Cleanup {
                phase,
                root_lifetime,
            } => WireProbeEventKind::Cleanup {
                phase: *phase,
                root_lifetime: *root_lifetime,
            },
            ProbeEventKind::Settlement { resource, state } => WireProbeEventKind::Settlement {
                resource: resource.as_ref().map(|resource| resource.0.as_str()),
                state: *state,
            },
            ProbeEventKind::Emission { resource, index } => WireProbeEventKind::Emission {
                resource: &resource.0,
                index: *index,
            },
            ProbeEventKind::Transition { resource, state } => WireProbeEventKind::Transition {
                resource: &resource.0,
                state: *state,
            },
            ProbeEventKind::Request { resource, phase } => WireProbeEventKind::Request {
                resource: &resource.0,
                phase: *phase,
            },
            ProbeEventKind::Response { resource, state } => WireProbeEventKind::Response {
                resource: &resource.0,
                state: *state,
            },
            ProbeEventKind::Stream { resource, state } => WireProbeEventKind::Stream {
                resource: &resource.0,
                state: *state,
            },
        };
        Self {
            sequence: event.sequence,
            marker: &event.marker,
            operation: event
                .operation
                .as_ref()
                .map(|operation| operation.0.as_str()),
            kind,
        }
    }
}

fn emit_transcript(
    plan: &RuntimeProbePlan,
    target: &ProbeTarget,
    mode: &ProbeMode,
    runs: &[(&ProbeSessionRequest, &ProbeRun)],
) -> Result<ProbeTranscript, RuntimeProbeError> {
    let artifact = plan
        .contract
        .artifact_case(&target.subject.artifact_case)
        .expect("runtime plan validation retained exact artifact case");
    let wire_runs = runs
        .iter()
        .map(|(session, run)| {
            let ProbeRunOutcome::Completed { events } = &run.outcome else {
                unreachable!("only completed runs are emitted as witness transcripts")
            };
            WireTranscriptRun {
                session: session.id.as_str(),
                repeat: session.repeat,
                process: &run.isolation.process,
                realm: &run.isolation.realm,
                module_instance: &run.isolation.module_instance,
                drained_microtasks: run.drained_microtasks,
                drained_macrotasks: run.drained_macrotasks,
                drained_animation_frames: run.drained_animation_frames,
                events: events.iter().map(WireProbeEvent::from).collect(),
            }
        })
        .collect();
    let document = WireTranscript {
        format: PROBE_TRANSCRIPT_FORMAT,
        transcript_version: PROBE_TRANSCRIPT_VERSION,
        semantic_model_version: plan.contract.semantic_model_version(),
        semantic_digest: plan.contract.semantic_digest().as_str(),
        plan_digest: plan.digest.as_str(),
        claim_id: target.claim_id.as_str(),
        artifact: WireTranscriptArtifact::new(artifact),
        export: &target.subject.export,
        authority: authority_name(target.authority),
        scenario: scenario_name(target.scenario),
        recipe: target.recipe.as_str(),
        mode: &mode.name,
        environment: WireTranscriptEnvironment::new(&mode.environment),
        runs: wire_runs,
    };
    let mut bytes =
        serde_json::to_vec_pretty(&document).map_err(|error| RuntimeProbeError::Emission {
            message: error.to_string(),
        })?;
    bytes.push(b'\n');
    let digest = Digest::parse(format!("sha256:{:x}", Sha256::digest(&bytes)))
        .expect("SHA-256 formatting is canonical");
    Ok(ProbeTranscript {
        claim_id: target.claim_id.clone(),
        mode: mode.name.clone(),
        digest,
        bytes,
    })
}

#[cfg(test)]
mod tests;
