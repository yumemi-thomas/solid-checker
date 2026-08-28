//! Process boundary for temporary-v2 runtime-probe orchestration.
//!
//! JavaScript launches isolated workers and transports events. This module
//! owns every semantic read: proposal-plan authorization, exact mode binding,
//! recipe/session identity, event classification, finite-absence refusal, and
//! transcript/contradiction emission.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use solid_reactive_ir::contract_semantics::{Digest, OperationId, ResourceId};
use thiserror::Error;

use crate::{
    EnvironmentIdentity, IsolationIdentity, ProbeAuthority, ProbeEvent, ProbeEventClass,
    ProbeEventKind, ProbeEventMatch, ProbeMode, ProbeOutcome, ProbePolicy, ProbeRecipe, ProbeRun,
    ProbeRunOutcome, ProbeScenario, RuntimeProbePlan, SandboxIdentity, SandboxKind, ToolIdentity,
    contract_document_v2,
    contract_workflow::{ContractWorkflowError, planned_probe_subjects},
    evaluate_runtime_probes,
    evidence_sidecars::WireSemanticClaimSubject,
};

const REQUEST_FORMAT: &str = "solid-checker-runtime-probe-request";
const PLAN_FORMAT: &str = "solid-checker-runtime-probe-plan";
const RUNS_FORMAT: &str = "solid-checker-runtime-probe-runs";
const EVALUATION_FORMAT: &str = "solid-checker-runtime-probe-evaluation";
const SCHEMA_VERSION: u16 = 2;
const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_DOCUMENT_DEPTH: usize = 128;
const MAX_DOCUMENT_NODES: usize = 1_000_000;
const MAX_DOCUMENT_STRING_BYTES: usize = 16 * 1024;

#[derive(Debug, Error)]
pub enum RuntimeProbeWireError {
    #[error(transparent)]
    Contract(#[from] crate::ContractFailure),
    #[error(transparent)]
    Workflow(#[from] ContractWorkflowError),
    #[error(transparent)]
    Probe(#[from] crate::RuntimeProbeError),
    #[error("runtime probe document cannot be decoded: {message}")]
    Decode { message: String },
    #[error("runtime probe document is invalid: {reason}")]
    Invalid { reason: String },
}

pub struct PlannedRuntimeProbes {
    plan: RuntimeProbePlan,
    bytes: Vec<u8>,
}

impl PlannedRuntimeProbes {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRequest {
    format: String,
    schema_version: u16,
    modes: Vec<WireMode>,
    recipes: Vec<WireRecipe>,
    policy: WirePolicy,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireMode {
    name: String,
    artifact_case: String,
    environment: WireEnvironment,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRecipe {
    claim_id: String,
    authority: WireAuthority,
    scenario: WireScenario,
    construction: String,
    module: String,
    expected_event: WireExpectedEvent,
    drain: Vec<WireDrainStep>,
    #[serde(default)]
    coverage_limitations: Vec<String>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WireAuthority {
    PossiblePositiveWitness,
    ClosureFalsification,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WireScenario {
    Operation,
    CleanupLifecycle,
    RepeatedAsyncIterable,
    TransitionLifecycle,
    RequestResponseLifecycle,
    RootLifetime,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireExpectedEvent {
    marker: String,
    class: ProbeEventClass,
    #[serde(default)]
    operation: Option<String>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum WireDrainStep {
    Flush,
    Microtasks { max_turns: u16 },
    Macrotasks { max_turns: u16 },
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WirePolicy {
    repeat_runs: u16,
    timeout_millis: u64,
    max_microtask_turns: u16,
    max_macrotask_turns: u16,
    max_events: u32,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireEnvironment {
    runtime: WireTool,
    os: String,
    architecture: String,
    conditions: Vec<String>,
    sandbox: WireSandbox,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireTool {
    name: String,
    version: String,
    build: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireSandbox {
    kind: WireSandboxKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    policy: Option<String>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WireSandboxKind {
    None,
    Process,
    Container,
    VirtualMachine,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WirePlan {
    format: &'static str,
    schema_version: u16,
    semantic_model_version: u16,
    semantic_digest: String,
    plan_digest: String,
    sessions: Vec<WireSession>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireSession {
    id: String,
    claim_id: String,
    subject: WireSemanticClaimSubject,
    authority: WireAuthority,
    scenario: WireScenario,
    recipe: String,
    construction: String,
    module: String,
    expected_event: WireExpectedEventOutput,
    drain: Vec<WireDrainStep>,
    mode: WireModeOutput,
    repeat: u16,
    policy: WirePolicy,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireExpectedEventOutput {
    marker: String,
    class: ProbeEventClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireModeOutput {
    name: String,
    artifact_case: String,
    environment: WireEnvironment,
}

pub fn plan_runtime_probes(
    proposal_bytes: &[u8],
    proposal_plan_bytes: &[u8],
    request_bytes: &[u8],
) -> Result<PlannedRuntimeProbes, RuntimeProbeWireError> {
    let contract = contract_document_v2::decode(proposal_bytes)?.normalize()?;
    let authorized = planned_probe_subjects(&contract, proposal_plan_bytes)?;
    let request: WireRequest = decode(request_bytes)?;
    if request.format != REQUEST_FORMAT || request.schema_version != SCHEMA_VERSION {
        return invalid(format!(
            "runtime probe request must use format {REQUEST_FORMAT:?} schemaVersion {SCHEMA_VERSION}"
        ));
    }
    let modes = request
        .modes
        .into_iter()
        .map(|mode| {
            Ok(ProbeMode {
                name: mode.name,
                artifact_case: mode.artifact_case,
                environment: environment(mode.environment)?,
            })
        })
        .collect::<Result<Vec<_>, RuntimeProbeWireError>>()?;
    let matrix = crate::ArtifactModeMatrix::new(&contract, modes)?;
    let mut modules = BTreeMap::<String, (String, String)>::new();
    let mut recipes = Vec::new();
    for recipe in request.recipes {
        let subject = match recipe.authority {
            WireAuthority::PossiblePositiveWitness => {
                authorized.possible_operations.get(&recipe.claim_id)
            }
            WireAuthority::ClosureFalsification => {
                authorized.closure_candidates.get(&recipe.claim_id)
            }
        }
        .cloned()
        .ok_or_else(|| RuntimeProbeWireError::Invalid {
            reason: format!(
                "runtime probe recipe names unplanned claim {} for its authority",
                recipe.claim_id
            ),
        })?;
        validate_transport_string(&recipe.module, "probe recipe module")?;
        let construction = parse_digest(&recipe.construction, "probe recipe construction")?;
        if modules
            .insert(
                recipe.claim_id.clone(),
                (construction.as_str().into(), recipe.module),
            )
            .is_some()
        {
            return invalid(format!(
                "runtime probe request repeats recipe for claim {}",
                recipe.claim_id
            ));
        }
        recipes.push(ProbeRecipe {
            subject,
            authority: recipe.authority.into(),
            scenario: recipe.scenario.into(),
            construction,
            expected_event: ProbeEventMatch {
                marker: recipe.expected_event.marker,
                class: recipe.expected_event.class,
                operation: recipe.expected_event.operation.map(OperationId),
            },
            drain: recipe.drain.into_iter().map(Into::into).collect(),
            coverage_limitations: recipe.coverage_limitations,
        });
    }
    let runtime = RuntimeProbePlan::build(
        contract.clone(),
        authorized
            .possible_operations
            .into_values()
            .collect::<BTreeSet<_>>(),
        authorized
            .closure_candidates
            .into_values()
            .collect::<BTreeSet<_>>(),
        matrix,
        recipes,
        request.policy.into(),
    )?;
    let sessions = runtime
        .sessions()
        .iter()
        .map(|session| {
            let (construction, module) = &modules[session.claim_id().as_str()];
            WireSession {
                id: session.id().as_str().into(),
                claim_id: session.claim_id().as_str().into(),
                subject: WireSemanticClaimSubject::from(session.subject()),
                authority: session.authority().into(),
                scenario: session.scenario().into(),
                recipe: session.recipe().as_str().into(),
                construction: construction.clone(),
                module: module.clone(),
                expected_event: WireExpectedEventOutput {
                    marker: session.expected_event().marker.clone(),
                    class: session.expected_event().class,
                    operation: session
                        .expected_event()
                        .operation
                        .as_ref()
                        .map(|operation| operation.0.clone()),
                },
                drain: session.drain().iter().copied().map(Into::into).collect(),
                mode: WireModeOutput {
                    name: session.mode().name.clone(),
                    artifact_case: session.mode().artifact_case.clone(),
                    environment: WireEnvironment::from(&session.mode().environment),
                },
                repeat: session.repeat(),
                policy: session.policy().into(),
            }
        })
        .collect();
    let bytes = emit(&WirePlan {
        format: PLAN_FORMAT,
        schema_version: SCHEMA_VERSION,
        semantic_model_version: contract.semantic_model_version(),
        semantic_digest: contract.semantic_digest().as_str().into(),
        plan_digest: runtime.digest().as_str().into(),
        sessions,
    })?;
    Ok(PlannedRuntimeProbes {
        plan: runtime,
        bytes,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRuns {
    format: String,
    schema_version: u16,
    plan_digest: String,
    producer: WireTool,
    runs: Vec<WireRun>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRun {
    session: String,
    environment: WireEnvironment,
    isolation: WireIsolation,
    drained_microtasks: u16,
    drained_macrotasks: u16,
    outcome: WireRunOutcome,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireIsolation {
    process: String,
    realm: String,
    module_instance: String,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum WireRunOutcome {
    Completed { events: Vec<WireEvent> },
    Error { details: String },
    Timeout,
    Refused { reason: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireEvent {
    sequence: u32,
    marker: String,
    #[serde(default)]
    operation: Option<String>,
    #[serde(flatten)]
    kind: WireEventKind,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum WireEventKind {
    Call {
        phase: crate::BoundaryPhase,
    },
    Render {
        phase: crate::BoundaryPhase,
    },
    Flush {
        ordinal: u16,
    },
    Callback {
        ordinal: u32,
    },
    Cleanup {
        phase: crate::CleanupPhase,
        root_lifetime: bool,
    },
    Settlement {
        #[serde(default)]
        resource: Option<String>,
        state: crate::SettlementState,
    },
    Emission {
        resource: String,
        index: u32,
    },
    Transition {
        resource: String,
        state: crate::TransitionState,
    },
    Request {
        resource: String,
        phase: crate::BoundaryPhase,
    },
    Response {
        resource: String,
        state: crate::ResponseState,
    },
    Stream {
        resource: String,
        state: crate::StreamState,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireEvaluation {
    format: &'static str,
    schema_version: u16,
    semantic_model_version: u16,
    semantic_digest: String,
    plan_digest: String,
    claims: Vec<WireEvaluatedClaim>,
    contradictions: Vec<WireContradiction>,
    transcripts: Vec<WireTranscript>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireEvaluatedClaim {
    claim_id: String,
    subject: WireSemanticClaimSubject,
    producer: WireTool,
    recipe: String,
    observations: Vec<WireObservation>,
    coverage_limitations: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireObservation {
    mode: String,
    environment: WireEnvironment,
    outcome: WireOutcome,
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum WireOutcome {
    Planned,
    Witness { transcript: String },
    Falsification { transcript: String },
    Error { details: String },
    Timeout { limit_millis: u64 },
    Refused { reason: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireContradiction {
    claim_id: String,
    subject: WireSemanticClaimSubject,
    mode: String,
    transcript: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireTranscript {
    claim_id: String,
    mode: String,
    digest: String,
    document: serde_json::Value,
}

pub fn evaluate_runtime_probe_runs(
    planned: &PlannedRuntimeProbes,
    runs_bytes: &[u8],
) -> Result<Vec<u8>, RuntimeProbeWireError> {
    let runs: WireRuns = decode(runs_bytes)?;
    if runs.format != RUNS_FORMAT || runs.schema_version != SCHEMA_VERSION {
        return invalid(format!(
            "runtime probe runs must use format {RUNS_FORMAT:?} schemaVersion {SCHEMA_VERSION}"
        ));
    }
    if runs.plan_digest != planned.plan.digest().as_str() {
        return invalid("runtime probe runs name a stale or different plan digest");
    }
    let producer = tool(runs.producer)?;
    let runs = runs
        .runs
        .into_iter()
        .map(probe_run)
        .collect::<Result<Vec<_>, _>>()?;
    let evaluated = evaluate_runtime_probes(&planned.plan, runs, producer.clone())?;
    let claims = evaluated
        .claim_material()
        .iter()
        .map(|claim| {
            let claim_id = planned
                .plan
                .contract()
                .claim_id(&claim.subject)
                .expect("runtime evaluator retained a validated subject");
            WireEvaluatedClaim {
                claim_id: claim_id.as_str().into(),
                subject: WireSemanticClaimSubject::from(&claim.subject),
                producer: WireTool::from(&claim.producer),
                recipe: claim.recipe.as_str().into(),
                observations: claim
                    .observations
                    .iter()
                    .map(|observation| WireObservation {
                        mode: observation.mode.clone(),
                        environment: WireEnvironment::from(&observation.environment),
                        outcome: WireOutcome::from(&observation.outcome),
                    })
                    .collect(),
                coverage_limitations: claim.coverage_limitations.clone(),
            }
        })
        .collect();
    let contradictions = evaluated
        .contradictions()
        .iter()
        .map(|record| WireContradiction {
            claim_id: record.claim_id.as_str().into(),
            subject: WireSemanticClaimSubject::from(&record.subject),
            mode: record.mode.clone(),
            transcript: record.transcript.as_str().into(),
        })
        .collect();
    let transcripts = evaluated
        .transcripts()
        .iter()
        .map(|transcript| {
            let document = crate::bounded_json::value(
                transcript.bytes(),
                crate::bounded_json::Limits {
                    bytes: MAX_DOCUMENT_BYTES,
                    depth: MAX_DOCUMENT_DEPTH,
                    nodes: MAX_DOCUMENT_NODES,
                    string_bytes: MAX_DOCUMENT_STRING_BYTES,
                },
            )
            .map_err(decode_error)?;
            Ok(WireTranscript {
                claim_id: transcript.claim_id().as_str().into(),
                mode: transcript.mode().into(),
                digest: transcript.digest().as_str().into(),
                document,
            })
        })
        .collect::<Result<Vec<_>, RuntimeProbeWireError>>()?;
    emit(&WireEvaluation {
        format: EVALUATION_FORMAT,
        schema_version: SCHEMA_VERSION,
        semantic_model_version: planned.plan.contract().semantic_model_version(),
        semantic_digest: planned.plan.contract().semantic_digest().as_str().into(),
        plan_digest: planned.plan.digest().as_str().into(),
        claims,
        contradictions,
        transcripts,
    })
}

fn probe_run(run: WireRun) -> Result<ProbeRun, RuntimeProbeWireError> {
    Ok(ProbeRun {
        session: parse_digest(&run.session, "probe session")?,
        environment: environment(run.environment)?,
        isolation: IsolationIdentity {
            process: run.isolation.process,
            realm: run.isolation.realm,
            module_instance: run.isolation.module_instance,
        },
        drained_microtasks: run.drained_microtasks,
        drained_macrotasks: run.drained_macrotasks,
        outcome: match run.outcome {
            WireRunOutcome::Completed { events } => ProbeRunOutcome::Completed {
                events: events
                    .into_iter()
                    .map(probe_event)
                    .collect::<Result<Vec<_>, _>>()?,
            },
            WireRunOutcome::Error { details } => ProbeRunOutcome::Error {
                details: parse_digest(&details, "probe error details")?,
            },
            WireRunOutcome::Timeout => ProbeRunOutcome::Timeout,
            WireRunOutcome::Refused { reason } => ProbeRunOutcome::Refused { reason },
        },
    })
}

fn probe_event(event: WireEvent) -> Result<ProbeEvent, RuntimeProbeWireError> {
    Ok(ProbeEvent {
        sequence: event.sequence,
        marker: event.marker,
        operation: event.operation.map(OperationId),
        kind: match event.kind {
            WireEventKind::Call { phase } => ProbeEventKind::Call { phase },
            WireEventKind::Render { phase } => ProbeEventKind::Render { phase },
            WireEventKind::Flush { ordinal } => ProbeEventKind::Flush { ordinal },
            WireEventKind::Callback { ordinal } => ProbeEventKind::Callback { ordinal },
            WireEventKind::Cleanup {
                phase,
                root_lifetime,
            } => ProbeEventKind::Cleanup {
                phase,
                root_lifetime,
            },
            WireEventKind::Settlement { resource, state } => ProbeEventKind::Settlement {
                resource: resource.map(ResourceId),
                state,
            },
            WireEventKind::Emission { resource, index } => ProbeEventKind::Emission {
                resource: ResourceId(resource),
                index,
            },
            WireEventKind::Transition { resource, state } => ProbeEventKind::Transition {
                resource: ResourceId(resource),
                state,
            },
            WireEventKind::Request { resource, phase } => ProbeEventKind::Request {
                resource: ResourceId(resource),
                phase,
            },
            WireEventKind::Response { resource, state } => ProbeEventKind::Response {
                resource: ResourceId(resource),
                state,
            },
            WireEventKind::Stream { resource, state } => ProbeEventKind::Stream {
                resource: ResourceId(resource),
                state,
            },
        },
    })
}

fn environment(wire: WireEnvironment) -> Result<EnvironmentIdentity, RuntimeProbeWireError> {
    Ok(EnvironmentIdentity {
        runtime: tool(wire.runtime)?,
        os: wire.os,
        architecture: wire.architecture,
        conditions: wire.conditions,
        sandbox: SandboxIdentity {
            kind: wire.sandbox.kind.into(),
            policy: wire
                .sandbox
                .policy
                .map(|value| parse_digest(&value, "sandbox policy"))
                .transpose()?,
        },
    })
}

fn tool(wire: WireTool) -> Result<ToolIdentity, RuntimeProbeWireError> {
    Ok(ToolIdentity {
        name: wire.name,
        version: wire.version,
        build: parse_digest(&wire.build, "tool build")?,
        protocol: wire.protocol,
    })
}

impl From<&ToolIdentity> for WireTool {
    fn from(tool: &ToolIdentity) -> Self {
        Self {
            name: tool.name.clone(),
            version: tool.version.clone(),
            build: tool.build.as_str().into(),
            protocol: tool.protocol.clone(),
        }
    }
}

impl From<&EnvironmentIdentity> for WireEnvironment {
    fn from(environment: &EnvironmentIdentity) -> Self {
        Self {
            runtime: WireTool::from(&environment.runtime),
            os: environment.os.clone(),
            architecture: environment.architecture.clone(),
            conditions: environment.conditions.clone(),
            sandbox: WireSandbox {
                kind: environment.sandbox.kind.into(),
                policy: environment
                    .sandbox
                    .policy
                    .as_ref()
                    .map(|digest| digest.as_str().into()),
            },
        }
    }
}

impl From<WireAuthority> for ProbeAuthority {
    fn from(value: WireAuthority) -> Self {
        match value {
            WireAuthority::PossiblePositiveWitness => Self::PossiblePositiveWitness,
            WireAuthority::ClosureFalsification => Self::ClosureFalsification,
        }
    }
}

impl From<ProbeAuthority> for WireAuthority {
    fn from(value: ProbeAuthority) -> Self {
        match value {
            ProbeAuthority::PossiblePositiveWitness => Self::PossiblePositiveWitness,
            ProbeAuthority::ClosureFalsification => Self::ClosureFalsification,
        }
    }
}

impl From<WireScenario> for ProbeScenario {
    fn from(value: WireScenario) -> Self {
        match value {
            WireScenario::Operation => Self::Operation,
            WireScenario::CleanupLifecycle => Self::CleanupLifecycle,
            WireScenario::RepeatedAsyncIterable => Self::RepeatedAsyncIterable,
            WireScenario::TransitionLifecycle => Self::TransitionLifecycle,
            WireScenario::RequestResponseLifecycle => Self::RequestResponseLifecycle,
            WireScenario::RootLifetime => Self::RootLifetime,
        }
    }
}

impl From<ProbeScenario> for WireScenario {
    fn from(value: ProbeScenario) -> Self {
        match value {
            ProbeScenario::Operation => Self::Operation,
            ProbeScenario::CleanupLifecycle => Self::CleanupLifecycle,
            ProbeScenario::RepeatedAsyncIterable => Self::RepeatedAsyncIterable,
            ProbeScenario::TransitionLifecycle => Self::TransitionLifecycle,
            ProbeScenario::RequestResponseLifecycle => Self::RequestResponseLifecycle,
            ProbeScenario::RootLifetime => Self::RootLifetime,
        }
    }
}

impl From<WireDrainStep> for crate::DrainStep {
    fn from(value: WireDrainStep) -> Self {
        match value {
            WireDrainStep::Flush => Self::Flush,
            WireDrainStep::Microtasks { max_turns } => Self::Microtasks { max_turns },
            WireDrainStep::Macrotasks { max_turns } => Self::Macrotasks { max_turns },
        }
    }
}

impl From<crate::DrainStep> for WireDrainStep {
    fn from(value: crate::DrainStep) -> Self {
        match value {
            crate::DrainStep::Flush => Self::Flush,
            crate::DrainStep::Microtasks { max_turns } => Self::Microtasks { max_turns },
            crate::DrainStep::Macrotasks { max_turns } => Self::Macrotasks { max_turns },
        }
    }
}

impl From<WirePolicy> for ProbePolicy {
    fn from(value: WirePolicy) -> Self {
        Self {
            repeat_runs: value.repeat_runs,
            timeout_millis: value.timeout_millis,
            max_microtask_turns: value.max_microtask_turns,
            max_macrotask_turns: value.max_macrotask_turns,
            max_events: value.max_events,
        }
    }
}

impl From<ProbePolicy> for WirePolicy {
    fn from(value: ProbePolicy) -> Self {
        Self {
            repeat_runs: value.repeat_runs,
            timeout_millis: value.timeout_millis,
            max_microtask_turns: value.max_microtask_turns,
            max_macrotask_turns: value.max_macrotask_turns,
            max_events: value.max_events,
        }
    }
}

impl From<WireSandboxKind> for SandboxKind {
    fn from(value: WireSandboxKind) -> Self {
        match value {
            WireSandboxKind::None => Self::None,
            WireSandboxKind::Process => Self::Process,
            WireSandboxKind::Container => Self::Container,
            WireSandboxKind::VirtualMachine => Self::VirtualMachine,
        }
    }
}

impl From<SandboxKind> for WireSandboxKind {
    fn from(value: SandboxKind) -> Self {
        match value {
            SandboxKind::None => Self::None,
            SandboxKind::Process => Self::Process,
            SandboxKind::Container => Self::Container,
            SandboxKind::VirtualMachine => Self::VirtualMachine,
        }
    }
}

impl From<&ProbeOutcome> for WireOutcome {
    fn from(value: &ProbeOutcome) -> Self {
        match value {
            ProbeOutcome::Planned => Self::Planned,
            ProbeOutcome::Witness { transcript } => Self::Witness {
                transcript: transcript.as_str().into(),
            },
            ProbeOutcome::Falsification { transcript } => Self::Falsification {
                transcript: transcript.as_str().into(),
            },
            ProbeOutcome::Error { details } => Self::Error {
                details: details.as_str().into(),
            },
            ProbeOutcome::Timeout { limit_millis } => Self::Timeout {
                limit_millis: *limit_millis,
            },
            ProbeOutcome::Refused { reason } => Self::Refused {
                reason: reason.clone(),
            },
        }
    }
}

fn parse_digest(value: &str, field: &str) -> Result<Digest, RuntimeProbeWireError> {
    Digest::parse(value).map_err(|error| RuntimeProbeWireError::Invalid {
        reason: format!("{field} is invalid: {error}"),
    })
}

fn validate_transport_string(value: &str, field: &str) -> Result<(), RuntimeProbeWireError> {
    if value.is_empty() || value.len() > 16 * 1024 {
        invalid(format!("{field} must contain between 1 and 16384 bytes"))
    } else {
        Ok(())
    }
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, RuntimeProbeWireError> {
    crate::bounded_json::decode(
        bytes,
        crate::bounded_json::Limits {
            bytes: MAX_DOCUMENT_BYTES,
            depth: MAX_DOCUMENT_DEPTH,
            nodes: MAX_DOCUMENT_NODES,
            string_bytes: MAX_DOCUMENT_STRING_BYTES,
        },
    )
    .map_err(decode_error)
}

fn emit(value: &impl Serialize) -> Result<Vec<u8>, RuntimeProbeWireError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(decode_error)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return invalid("runtime probe document exceeds the 16 MiB resource limit");
    }
    Ok(bytes)
}

fn decode_error(error: impl std::fmt::Display) -> RuntimeProbeWireError {
    RuntimeProbeWireError::Decode {
        message: error.to_string(),
    }
}

fn invalid<T>(reason: impl Into<String>) -> Result<T, RuntimeProbeWireError> {
    Err(RuntimeProbeWireError::Invalid {
        reason: reason.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_workflow::encode_proposal_artifacts;

    #[test]
    fn temporary_v2_plan_and_evaluation_keep_semantic_judgement_in_rust() {
        let bundle = crate::first_party_bundles::solid2_rc3_bundles()
            .unwrap()
            .into_iter()
            .find(|bundle| bundle.file_stem == "solid-js")
            .unwrap();
        let contract = contract_document_v2::decode(&bundle.document)
            .unwrap()
            .normalize()
            .unwrap();
        let proposal = encode_proposal_artifacts(&contract, Vec::new(), false).unwrap();
        let plan: serde_json::Value = serde_json::from_slice(&proposal.plan).unwrap();
        let positive = &plan["positiveOperations"][0];
        let claim_id = positive["claimId"].as_str().unwrap();
        let artifact_case = positive["subject"]["artifactCase"].as_str().unwrap();
        let operation = positive["subject"]["path"]["operation"].as_str().unwrap();
        let digest = |byte: char| format!("sha256:{}", byte.to_string().repeat(64));
        let request = serde_json::json!({
            "format": REQUEST_FORMAT,
            "schemaVersion": 2,
            "modes": [{
                "name": "node-test",
                "artifactCase": artifact_case,
                "environment": {
                    "runtime": {"name": "node", "version": "test", "build": digest('1'), "protocol": "test"},
                    "os": "test",
                    "architecture": "test",
                    "conditions": ["node"],
                    "sandbox": {"kind": "process", "policy": digest('2')}
                }
            }],
            "recipes": [{
                "claimId": claim_id,
                "authority": "possible-positive-witness",
                "scenario": "operation",
                "construction": digest('3'),
                "module": "recipe.mjs",
                "expectedEvent": {"marker": "observed", "class": "callback", "operation": operation},
                "drain": [{"kind": "microtasks", "maxTurns": 1}],
                "coverageLimitations": []
            }],
            "policy": {"repeatRuns": 2, "timeoutMillis": 1000, "maxMicrotaskTurns": 2, "maxMacrotaskTurns": 0, "maxEvents": 16}
        });
        let planned = plan_runtime_probes(
            &proposal.document,
            &proposal.plan,
            &serde_json::to_vec(&request).unwrap(),
        )
        .unwrap();
        let plan: serde_json::Value = serde_json::from_slice(planned.bytes()).unwrap();
        assert_eq!(plan["schemaVersion"], 2);
        assert_eq!(plan["sessions"].as_array().unwrap().len(), 2);
        let runs = plan["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, session)| {
                serde_json::json!({
                    "session": session["id"],
                    "environment": session["mode"]["environment"],
                    "isolation": {"process": format!("p{index}"), "realm": format!("r{index}"), "moduleInstance": format!("m{index}")},
                    "drainedMicrotasks": 1,
                    "drainedMacrotasks": 0,
                    "outcome": {"kind": "completed", "events": [{"sequence": 0, "marker": "observed", "operation": operation, "kind": "callback", "ordinal": 0}]}
                })
            })
            .collect::<Vec<_>>();
        let runs = serde_json::json!({
            "format": RUNS_FORMAT,
            "schemaVersion": 2,
            "planDigest": plan["planDigest"],
            "producer": {"name": "worker", "version": "1", "build": digest('4'), "protocol": "test"},
            "runs": runs
        });
        let evaluation =
            evaluate_runtime_probe_runs(&planned, &serde_json::to_vec(&runs).unwrap()).unwrap();
        let evaluation: serde_json::Value = serde_json::from_slice(&evaluation).unwrap();
        assert_eq!(evaluation["schemaVersion"], 2);
        assert_eq!(
            evaluation["claims"][0]["observations"][0]["outcome"]["kind"],
            "witness"
        );
        assert!(evaluation["contradictions"].as_array().unwrap().is_empty());
        assert_eq!(evaluation["transcripts"].as_array().unwrap().len(), 1);
    }
}
