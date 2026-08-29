//! Authority-bearing Solid 2 compiler reconciliation for proof policy 2.
//!
//! Ordinary [`solid_facts::compiler::ExecutionMap`] values are serializable
//! analysis data. This module accepts compiler evidence only from a fresh
//! private copy of the running verifier image, launched in the hidden compiler
//! session mode. The parent verifies the child PID, executable bytes, nonce,
//! demand/snapshot roots, exact source and configuration, materialized output,
//! and the normalized compiler-owned site census before creating witnesses.
//! Schema-v1 currently stores transform-tool identity separately from virtual
//! generated-output digests but carries neither output bytes nor an immutable
//! pairing sidecar. Schedule construction therefore refuses every non-empty
//! compiler demand until that neighboring protocol exists; the live child
//! protocol is implemented and tested without treating it as closure authority.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solid_facts::compiler::{
    AnalysisRequest, CompilerExecutionCardinality, CompilerExecutionDisposition,
    CompilerExecutionSchedule, CompilerExecutionTrigger, CompilerOptions, CompilerOwnerRelation,
    CompilerTrackingRelation, ExecutionMap,
};
use solid_reactive_ir::contract_semantics::certification::{
    ProofDemandSubject, ProofFamily, ProofWitnessVariant, WitnessBinding,
};
#[cfg(unix)]
use std::fs::OpenOptions;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

use super::{CertificationPlan, certification_evidence_root};

const SESSION_PROTOCOL: u32 = 1;
pub(crate) const SESSION_ARGUMENT: &str = "--internal-compiler-certification-session";
const MAX_SESSION_BYTES: u64 = 64 * 1024 * 1024;
static SESSION_NONCE_COUNTER: AtomicU64 = AtomicU64::new(1);
#[cfg(unix)]
static EXECUTION_IMAGE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Policy-owned configuration for one transformed artifact case.
///
/// The demand ID and source/output paths are not caller inputs: the schedule
/// derives them from the opaque certification plan. Configuration remains an
/// explicit recipe because schema-v1 artifact identity does not encode
/// compiler flags; accepting the recipe still requires byte-for-byte equality
/// with the plan's materialized transform output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerCertificationConfiguration {
    demand_id: String,
    options: CompilerOptions,
}

impl CompilerCertificationConfiguration {
    #[must_use]
    pub fn new(demand_id: impl Into<String>, options: CompilerOptions) -> Self {
        Self {
            demand_id: demand_id.into(),
            options,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerCertificationSchedule {
    demand_graph_root: String,
    snapshot_root: String,
    units: Vec<CompilerCertificationUnit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompilerCertificationUnit {
    demand_id: String,
    artifact_case: String,
    source_path: String,
    output_path: String,
    request: AnalysisRequest,
}

impl CompilerCertificationSchedule {
    #[must_use]
    pub fn demand_count(&self) -> usize {
        self.units.len()
    }

    pub fn new(
        plan: &CertificationPlan,
        configurations: impl IntoIterator<Item = CompilerCertificationConfiguration>,
    ) -> Result<Self, CompilerCertificationError> {
        let expected = plan
            .demand_graph
            .demands()
            .iter()
            .filter(|demand| demand.family() == ProofFamily::CompilerReconciliation)
            .map(|demand| {
                let ProofDemandSubject::ArtifactCase(artifact_case) = demand.subject() else {
                    return Err(CompilerCertificationError::InvalidDemandSubject);
                };
                Ok((demand.id().as_str().to_owned(), artifact_case.clone()))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let mut supplied = BTreeMap::new();
        for configuration in configurations {
            if !expected.contains_key(&configuration.demand_id) {
                return Err(CompilerCertificationError::UnknownDemand(
                    configuration.demand_id,
                ));
            }
            if supplied
                .insert(configuration.demand_id.clone(), configuration.options)
                .is_some()
            {
                return Err(CompilerCertificationError::DuplicateDemand(
                    configuration.demand_id,
                ));
            }
        }
        if supplied.len() != expected.len() {
            let missing = expected
                .keys()
                .find(|id| !supplied.contains_key(*id))
                .cloned()
                .unwrap_or_else(|| "unknown".into());
            return Err(CompilerCertificationError::MissingDemand(missing));
        }
        if !expected.is_empty() {
            return Err(CompilerCertificationError::MaterializationSidecarRequired);
        }
        Ok(Self {
            demand_graph_root: plan.demand_graph.root().as_str().to_owned(),
            snapshot_root: plan.snapshot.root().to_owned(),
            units: Vec::new(),
        })
    }
}

/// Non-serializable authority token for one directly launched compiler child.
pub struct LiveCompilerAnswer {
    response: SessionResponse,
    identity: LiveCompilerSessionIdentity,
}

struct LiveCompilerSessionIdentity {
    executable_sha256: String,
    source_manifest_sha256: String,
    process_id: u32,
    nonce: String,
    snapshot_root: String,
    demand_graph_root: String,
    demand_id: String,
    evidence_root: String,
}

pub struct LiveCompilerEvidenceBatch {
    answers: Vec<LiveCompilerAnswer>,
}

pub struct VerifiedCompilerEvidence {
    bindings: Vec<WitnessBinding>,
    session_evidence_root: String,
}

impl VerifiedCompilerEvidence {
    #[must_use]
    pub fn witness_bindings(&self) -> &[WitnessBinding] {
        &self.bindings
    }

    #[must_use]
    pub fn session_evidence_root(&self) -> &str {
        &self.session_evidence_root
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionRequest {
    protocol: u32,
    nonce: String,
    snapshot_root: String,
    demand_graph_root: String,
    demand_id: String,
    analysis: AnalysisRequest,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionResponse {
    protocol: u32,
    nonce: String,
    process_id: u32,
    compiler_identity: String,
    compiler_source_manifest_sha256: String,
    request_sha256: String,
    execution_map: ExecutionMap,
    output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_map: Option<String>,
}

pub(crate) fn acquire(
    plan: &CertificationPlan,
    schedule: &CompilerCertificationSchedule,
) -> Result<LiveCompilerEvidenceBatch, CompilerCertificationError> {
    if schedule.snapshot_root != plan.snapshot.root()
        || schedule.demand_graph_root != plan.demand_graph.root().as_str()
    {
        return Err(CompilerCertificationError::ScheduleSubstitution);
    }
    let image = PrivateExecutionImage::copy_current_verifier()?;
    let mut answers = Vec::with_capacity(schedule.units.len());
    for unit in &schedule.units {
        answers.push(run_unit(&image, schedule, unit)?);
    }
    Ok(LiveCompilerEvidenceBatch { answers })
}

fn run_unit(
    image: &PrivateExecutionImage,
    schedule: &CompilerCertificationSchedule,
    unit: &CompilerCertificationUnit,
) -> Result<LiveCompilerAnswer, CompilerCertificationError> {
    if sha256_file(&image.path)? != image.executable_sha256 {
        return Err(CompilerCertificationError::ExecutableMutation);
    }
    let nonce = new_nonce(image.executable_sha256.as_str(), &unit.demand_id);
    let request = SessionRequest {
        protocol: SESSION_PROTOCOL,
        nonce: nonce.clone(),
        snapshot_root: schedule.snapshot_root.clone(),
        demand_graph_root: schedule.demand_graph_root.clone(),
        demand_id: unit.demand_id.clone(),
        analysis: unit.request.clone(),
    };
    let mut child = Command::new(&image.path)
        .arg(SESSION_ARGUMENT)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
    let process_id = child.id();
    if sha256_file(&image.path)? != image.executable_sha256 {
        let _ = child.kill();
        let _ = child.wait();
        return Err(CompilerCertificationError::ExecutableMutation);
    }
    child
        .stdin
        .take()
        .ok_or_else(|| CompilerCertificationError::Process("compiler child stdin missing".into()))?
        .write_all(&serde_json::to_vec(&request)?)
        .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
    let output = child
        .wait_with_output()
        .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
    if !output.status.success() {
        return Err(CompilerCertificationError::Process(format!(
            "compiler child exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    if output.stdout.len() as u64 > MAX_SESSION_BYTES {
        return Err(CompilerCertificationError::ResourceLimit);
    }
    let response: SessionResponse = serde_json::from_slice(&output.stdout)?;
    if response.protocol != SESSION_PROTOCOL
        || response.nonce != nonce
        || response.process_id != process_id
        || response.compiler_identity != solid_v2_compiler::COMPILER_FACTS_IDENTITY
        || response.compiler_source_manifest_sha256
            != solid_v2_compiler::COMPILER_SOURCE_MANIFEST_SHA256
        || response.request_sha256 != analysis_request_sha256(&unit.request)?
    {
        return Err(CompilerCertificationError::SessionIdentity);
    }
    let evidence_root = session_evidence_root(
        image.executable_sha256.as_str(),
        solid_v2_compiler::COMPILER_SOURCE_MANIFEST_SHA256,
        process_id,
        &nonce,
        schedule,
        unit,
        &response,
    );
    Ok(LiveCompilerAnswer {
        response,
        identity: LiveCompilerSessionIdentity {
            executable_sha256: image.executable_sha256.clone(),
            source_manifest_sha256: solid_v2_compiler::COMPILER_SOURCE_MANIFEST_SHA256.into(),
            process_id,
            nonce,
            snapshot_root: schedule.snapshot_root.clone(),
            demand_graph_root: schedule.demand_graph_root.clone(),
            demand_id: unit.demand_id.clone(),
            evidence_root,
        },
    })
}

pub(crate) fn verify(
    plan: &CertificationPlan,
    schedule: &CompilerCertificationSchedule,
    batch: &LiveCompilerEvidenceBatch,
) -> Result<VerifiedCompilerEvidence, CompilerCertificationError> {
    if schedule.snapshot_root != plan.snapshot.root()
        || schedule.demand_graph_root != plan.demand_graph.root().as_str()
        || batch.answers.len() != schedule.units.len()
    {
        return Err(CompilerCertificationError::ScheduleSubstitution);
    }
    let answers = batch
        .answers
        .iter()
        .map(|answer| (answer.identity.demand_id.as_str(), answer))
        .collect::<BTreeMap<_, _>>();
    if answers.len() != batch.answers.len() {
        return Err(CompilerCertificationError::MixedSession);
    }
    let mut bindings = Vec::with_capacity(schedule.units.len());
    let mut session_roots = Vec::with_capacity(schedule.units.len());
    for unit in &schedule.units {
        let answer = answers
            .get(unit.demand_id.as_str())
            .ok_or(CompilerCertificationError::MixedSession)?;
        verify_unit(plan, schedule, unit, answer)?;
        let mut sites = answer
            .response
            .execution_map
            .semantic_model
            .operations
            .iter()
            .map(|operation| format!("source:{}", operation.id))
            .chain(
                answer
                    .response
                    .execution_map
                    .semantic_model
                    .generated_operations
                    .iter()
                    .map(|operation| format!("generated:{}", operation.id)),
            )
            .collect::<Vec<_>>();
        sites.sort();
        sites.dedup();
        if sites.is_empty() {
            return Err(CompilerCertificationError::EmptySiteCensus);
        }
        bindings.push(WitnessBinding::new(
            ProofWitnessVariant::CompilerReconciliation,
            unit.demand_id.clone(),
            answer.identity.evidence_root.clone(),
            sites,
        ));
        session_roots.push(answer.identity.evidence_root.as_str());
    }
    session_roots.sort_unstable();
    Ok(VerifiedCompilerEvidence {
        session_evidence_root: certification_evidence_root("compiler-session-batch", session_roots),
        bindings,
    })
}

fn verify_unit(
    plan: &CertificationPlan,
    schedule: &CompilerCertificationSchedule,
    unit: &CompilerCertificationUnit,
    answer: &LiveCompilerAnswer,
) -> Result<(), CompilerCertificationError> {
    let identity = &answer.identity;
    if identity.snapshot_root != schedule.snapshot_root
        || identity.demand_graph_root != schedule.demand_graph_root
        || identity.demand_id != unit.demand_id
        || identity.process_id != answer.response.process_id
        || identity.nonce != answer.response.nonce
        || identity.source_manifest_sha256 != solid_v2_compiler::COMPILER_SOURCE_MANIFEST_SHA256
        || answer.response.compiler_source_manifest_sha256 != identity.source_manifest_sha256
    {
        return Err(CompilerCertificationError::MixedSession);
    }
    let current = session_evidence_root(
        &identity.executable_sha256,
        &identity.source_manifest_sha256,
        identity.process_id,
        &identity.nonce,
        schedule,
        unit,
        &answer.response,
    );
    if current != identity.evidence_root {
        return Err(CompilerCertificationError::MixedSession);
    }
    if plan.snapshot.read(&unit.source_path) != Some(unit.request.source.as_bytes()) {
        return Err(CompilerCertificationError::SnapshotMutation(
            unit.source_path.clone(),
        ));
    }
    if plan.snapshot.read(&unit.output_path) != Some(answer.response.output.as_bytes()) {
        return Err(CompilerCertificationError::OutputMismatch(
            unit.output_path.clone(),
        ));
    }
    answer
        .response
        .execution_map
        .validate(&unit.request.source)?;
    if answer.response.execution_map.source_hash != unit.request.source_hash {
        return Err(CompilerCertificationError::SessionIdentity);
    }
    let model = &answer.response.execution_map.semantic_model;
    let producer = model
        .producer
        .as_ref()
        .ok_or(CompilerCertificationError::IncompleteProducer)?;
    if !producer.identity_complete
        || producer.dialect != "solid-v2"
        || producer.trace_version != 3
        || producer.output_sha256 != sha256_hex(answer.response.output.as_bytes())
        || producer.source_map_sha256
            != answer
                .response
                .source_map
                .as_deref()
                .map(|source_map| sha256_hex(source_map.as_bytes()))
    {
        return Err(CompilerCertificationError::IncompleteProducer);
    }
    if !model.source_operations_complete {
        return Err(CompilerCertificationError::OpenSourceCensus);
    }
    for operation in &model.operations {
        if operation.execution.disposition == CompilerExecutionDisposition::Unknown
            || operation.execution.trigger == CompilerExecutionTrigger::Unknown
            || operation.execution.schedule == CompilerExecutionSchedule::Unknown
            || operation.execution.tracking == CompilerTrackingRelation::Unknown
            || operation.execution.cardinality == CompilerExecutionCardinality::Unknown
            || operation.execution.owner == CompilerOwnerRelation::Unknown
        {
            return Err(CompilerCertificationError::OpenOperation(
                operation.id.clone(),
            ));
        }
    }
    for operation in &model.generated_operations {
        if operation.trigger == CompilerExecutionTrigger::Unknown
            || operation.schedule == CompilerExecutionSchedule::Unknown
            || operation.tracking == CompilerTrackingRelation::Unknown
            || operation.cardinality == CompilerExecutionCardinality::Unknown
            || operation.owner == CompilerOwnerRelation::Unknown
        {
            return Err(CompilerCertificationError::OpenOperation(
                operation.id.clone(),
            ));
        }
    }
    // Trace v3 has no independent complete generated-wrapper census. Exact
    // positive generated rows are retained, but their absence remains open.
    if model.generated_operations_complete {
        return Err(CompilerCertificationError::UnexpectedGeneratedClosure);
    }
    Ok(())
}

/// Hidden child entrypoint. Only the parent-created opaque token can turn this
/// ordinary JSON response into proof authority.
pub fn serve_compiler_certification_session() -> Result<(), CompilerCertificationError> {
    let mut encoded = Vec::new();
    std::io::stdin()
        .take(MAX_SESSION_BYTES + 1)
        .read_to_end(&mut encoded)
        .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
    if encoded.len() as u64 > MAX_SESSION_BYTES {
        return Err(CompilerCertificationError::ResourceLimit);
    }
    let request: SessionRequest = serde_json::from_slice(&encoded)?;
    if request.protocol != SESSION_PROTOCOL
        || request.nonce.trim().is_empty()
        || request.snapshot_root.trim().is_empty()
        || request.demand_graph_root.trim().is_empty()
        || request.demand_id.trim().is_empty()
    {
        return Err(CompilerCertificationError::SessionIdentity);
    }
    let compilation = solid_v2_compiler::analyze_with_materialized_output(&request.analysis)
        .map_err(|error| CompilerCertificationError::Compiler(error.to_string()))?;
    let request_sha256 = analysis_request_sha256(&request.analysis)?;
    let response = SessionResponse {
        protocol: SESSION_PROTOCOL,
        nonce: request.nonce,
        process_id: std::process::id(),
        compiler_identity: solid_v2_compiler::COMPILER_FACTS_IDENTITY.into(),
        compiler_source_manifest_sha256: solid_v2_compiler::COMPILER_SOURCE_MANIFEST_SHA256.into(),
        request_sha256,
        execution_map: compilation.execution_map,
        output: compilation.output,
        source_map: compilation.source_map,
    };
    serde_json::to_writer(std::io::stdout().lock(), &response)?;
    Ok(())
}

struct PrivateExecutionImage {
    root: PathBuf,
    path: PathBuf,
    executable_sha256: String,
}

impl PrivateExecutionImage {
    fn copy_current_verifier() -> Result<Self, CompilerCertificationError> {
        #[cfg(not(unix))]
        return Err(CompilerCertificationError::UnsupportedPlatform);

        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

            let source = std::env::current_exe()
                .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
            let source_digest = sha256_file(&source)?;
            let root = std::env::temp_dir().join(format!(
                "solid-checker-compiler-session-{}-{}",
                std::process::id(),
                EXECUTION_IMAGE_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root)
                .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
            let path = root.join("solid-checker-compiler-session");
            let result = (|| {
                let mut input = File::open(&source)
                    .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
                let mut output = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o500)
                    .open(&path)
                    .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
                std::io::copy(&mut input, &mut output)
                    .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
                output
                    .sync_all()
                    .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o500))
                    .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
                if sha256_file(&path)? != source_digest {
                    return Err(CompilerCertificationError::ExecutableMutation);
                }
                Ok(())
            })();
            if let Err(error) = result {
                let _ = fs::remove_file(&path);
                let _ = fs::remove_dir(&root);
                return Err(error);
            }
            Ok(Self {
                root,
                path,
                executable_sha256: source_digest,
            })
        }
    }
}

impl Drop for PrivateExecutionImage {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.root);
    }
}

fn new_nonce(executable_sha256: &str, demand_id: &str) -> String {
    certification_evidence_root(
        "compiler-session-nonce",
        [
            executable_sha256,
            demand_id,
            &std::process::id().to_string(),
            &SESSION_NONCE_COUNTER
                .fetch_add(1, Ordering::Relaxed)
                .to_string(),
            &SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .to_string(),
        ],
    )
}

fn session_evidence_root(
    executable_sha256: &str,
    source_manifest_sha256: &str,
    process_id: u32,
    nonce: &str,
    schedule: &CompilerCertificationSchedule,
    unit: &CompilerCertificationUnit,
    response: &SessionResponse,
) -> String {
    let producer = response.execution_map.semantic_model.producer.as_ref();
    certification_evidence_root(
        "compiler-live-session",
        [
            executable_sha256,
            source_manifest_sha256,
            &process_id.to_string(),
            nonce,
            &schedule.snapshot_root,
            &schedule.demand_graph_root,
            &unit.demand_id,
            &unit.artifact_case,
            &unit.source_path,
            unit.request.source_hash.as_str(),
            &unit.output_path,
            response.compiler_identity.as_str(),
            response.request_sha256.as_str(),
            producer.map_or("missing", |identity| identity.output_sha256.as_str()),
            producer.map_or("missing", |identity| identity.configuration_sha256.as_str()),
        ],
    )
}

fn sha256_file(path: &Path) -> Result<String, CompilerCertificationError> {
    let mut file =
        File::open(path).map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| CompilerCertificationError::Process(error.to_string()))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn analysis_request_sha256(
    request: &AnalysisRequest,
) -> Result<String, CompilerCertificationError> {
    Ok(format!(
        "sha256:{}",
        sha256_hex(&serde_json::to_vec(request)?)
    ))
}

#[derive(Debug, Error)]
pub enum CompilerCertificationError {
    #[error(
        "compiler demand requires a materialization sidecar that separately binds transform-tool and virtual-output bytes"
    )]
    MaterializationSidecarRequired,
    #[error("unknown compiler proof demand {0}")]
    UnknownDemand(String),
    #[error("duplicate compiler proof demand {0}")]
    DuplicateDemand(String),
    #[error("missing compiler proof demand {0}")]
    MissingDemand(String),
    #[error("compiler proof demand does not name one transformed artifact case")]
    InvalidDemandSubject,
    #[error("snapshot path is missing: {0}")]
    SnapshotPath(String),
    #[error("compiler source is not UTF-8: {0}")]
    SnapshotSourceUtf8(String),
    #[error("compiler mode is not certifiable: {0}")]
    UnsupportedMode(String),
    #[error("compiler certification schedule was substituted")]
    ScheduleSubstitution,
    #[error("compiler certification process failed: {0}")]
    Process(String),
    #[error("compiler certification response exceeded its resource limit")]
    ResourceLimit,
    #[error("compiler certification session identity is invalid")]
    SessionIdentity,
    #[error("compiler certification evidence mixes process sessions")]
    MixedSession,
    #[error("compiler execution image changed during private snapshot creation")]
    ExecutableMutation,
    #[error("compiler certification is unsupported on this platform")]
    UnsupportedPlatform,
    #[error("compiler output does not match snapshot artifact {0}")]
    OutputMismatch(String),
    #[error("compiler source changed in snapshot at {0}")]
    SnapshotMutation(String),
    #[error("compiler producer identity is incomplete or mismatched")]
    IncompleteProducer,
    #[error("compiler source-operation census is open")]
    OpenSourceCensus,
    #[error("compiler operation has an unknown semantic axis: {0}")]
    OpenOperation(String),
    #[error("trace v3 unexpectedly claimed complete generated-operation closure")]
    UnexpectedGeneratedClosure,
    #[error("compiler reconciliation demand has no concrete compiler-owned site")]
    EmptySiteCensus,
    #[error("compiler failed: {0}")]
    Compiler(String),
    #[error(transparent)]
    Facts(#[from] solid_facts::compiler::CompilerFactsError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_axes_are_never_silently_closed() {
        let source = "const view = <div>{count()}</div>;";
        let request = AnalysisRequest::new("input.tsx", source, CompilerOptions::default());
        let compilation =
            solid_v2_compiler::analyze_with_materialized_output(&request).expect("compile");
        assert!(
            compilation
                .execution_map
                .semantic_model
                .source_operations_complete
        );
        assert!(
            compilation
                .execution_map
                .semantic_model
                .operations
                .iter()
                .all(|operation| operation.execution.disposition
                    != CompilerExecutionDisposition::Unknown)
        );
        assert!(
            !compilation
                .execution_map
                .semantic_model
                .generated_operations_complete
        );
    }

    #[test]
    fn serialized_session_response_is_not_an_authority_token() {
        fn assert_not_serde<T>() {}
        assert_not_serde::<LiveCompilerAnswer>();
        assert_not_serde::<LiveCompilerEvidenceBatch>();
    }

    #[test]
    fn private_execution_image_preserves_the_exact_running_verifier_bytes() {
        let current = std::env::current_exe().unwrap();
        let expected = sha256_file(&current).unwrap();
        let image = PrivateExecutionImage::copy_current_verifier().unwrap();
        assert_eq!(image.executable_sha256, expected);
        assert_eq!(sha256_file(&image.path).unwrap(), expected);
        assert!(image.root.is_dir());
    }

    #[test]
    fn request_digest_is_canonical_over_normalized_options() {
        let left = AnalysisRequest::new(
            "input.tsx",
            "const view = <div />;",
            CompilerOptions {
                built_ins: vec!["For".into(), "Show".into()],
                ..CompilerOptions::default()
            },
        );
        let right = left.clone();
        assert_eq!(
            analysis_request_sha256(&left).unwrap(),
            analysis_request_sha256(&right).unwrap()
        );
    }
}
