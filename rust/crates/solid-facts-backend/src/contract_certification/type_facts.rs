//! Authority-bearing Type Facts acquisition for proof policy 2.
//!
//! The ordinary Type Facts API intentionally exposes serializable invocation
//! transcripts for analysis and audit. This module adds the stricter package-
//! certification boundary: it copies a trusted pinned executable into a
//! private execution directory, verifies the copied bytes and adjacent source
//! manifest, launches that exact path, and accepts evidence only through the
//! resulting live session token.

use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{
    CardinalityScope, ClaimDomain, ClaimPath, OperationKind, Requirement, SemanticClaimPath,
    UpperBound, ValuePathSegment, ValueRoot, ValueShape, ValueSource,
    certification::{
        DemandedCallability, PositiveFactSubject, ProofDemand, ProofDemandGraph,
        ProofDemandSubject, ProofFamily, ProofWitnessVariant, WitnessBinding,
    },
};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use thiserror::Error;
use typefacts::{
    ArgumentBindingDisposition, CallKind, Callability, CertificationInvocationContext,
    ExportValueDemand, ExportValueTranscript, FinitePartition, InvocationConstructability,
    InvocationDemand, InvocationDomain, InvocationTranscript, InvocationValueFact,
    LiveExportValueAnswer, LiveInvocationAnswer, ParameterUseKind, PathPresence, PathSegmentKind,
    Producer, Reachability, ResolvedCallValidity, Session, SourceHash, TranscriptSourceDigest,
};

use super::CertificationPlan;

static EXECUTION_IMAGE_COUNTER: AtomicU64 = AtomicU64::new(1);
static PRIVATE_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(1);

// Retained for the opaque transaction that will derive schedules in Slice 7.
#[allow(dead_code)]
const TYPE_FACTS_FAMILIES: [ProofFamily; 9] = [
    ProofFamily::SelectedSignature,
    ProofFamily::ArgumentBinding,
    ProofFamily::RestSpreadCoverage,
    ProofFamily::CallablePath,
    ProofFamily::OperationReachability,
    ProofFamily::OperationCardinality,
    ProofFamily::RecursiveValueShape,
    ProofFamily::GuardPartition,
    ProofFamily::DomainExhaustiveness,
];

/// Trusted configuration for the exact Type Facts image used by one
/// certification session. This stays crate-private until policy 2 is cut over.
#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct TypeFactsProducerPin {
    path: PathBuf,
    executable_sha256: SourceHash,
    source_manifest_sha256: SourceHash,
}

impl TypeFactsProducerPin {
    /// Loads the producer identity compiled into this verifier build.
    ///
    /// Release/certification builds must set both values. A development build
    /// without them refuses producer authority instead of trusting runtime
    /// strings or an adjacent stamp as its own root of trust.
    pub fn configured(path: impl Into<PathBuf>) -> Result<Self, TypeFactsCertificationError> {
        let executable_sha256 =
            option_env!("SOLID_TYPEFACTS_CERTIFICATION_SHA256").ok_or_else(|| {
                TypeFactsCertificationError::ProducerProvenance(
                    "verifier build has no configured Type Facts executable digest".into(),
                )
            })?;
        let source_manifest_sha256 = option_env!("SOLID_TYPEFACTS_SOURCE_MANIFEST_SHA256")
            .ok_or_else(|| {
                TypeFactsCertificationError::ProducerProvenance(
                    "verifier build has no configured Type Facts source-manifest digest".into(),
                )
            })?;
        Self::new(path, executable_sha256, source_manifest_sha256)
    }

    pub(crate) fn new(
        path: impl Into<PathBuf>,
        executable_sha256: impl Into<String>,
        source_manifest_sha256: impl Into<String>,
    ) -> Result<Self, TypeFactsCertificationError> {
        let path = path.into();
        if !path.is_absolute() {
            return Err(TypeFactsCertificationError::ProducerProvenance(
                "pinned Type Facts path must be absolute".into(),
            ));
        }
        let executable_sha256 = SourceHash::parse(executable_sha256.into())?;
        let source_manifest_sha256 = SourceHash::parse(source_manifest_sha256.into())?;
        Ok(Self {
            path,
            executable_sha256,
            source_manifest_sha256,
        })
    }

    pub(crate) fn executable_sha256(&self) -> &str {
        self.executable_sha256.as_str()
    }

    pub(crate) fn source_manifest_sha256(&self) -> &str {
        self.source_manifest_sha256.as_str()
    }
}

/// One exact invocation demand plus the verifier-derived proof demands it must
/// discharge. Every proof demand is scheduled exactly once; locations are
/// request data, while semantic authority still comes only from the live
/// transcript and family reconciliation.
///
/// The authority transaction derives this schedule from its retained opaque
/// plan. External callers cannot assign proof demands to arbitrary source
/// expressions.
///
/// ```compile_fail
/// use solid_facts_backend::TypeFactsCertificationSchedule;
/// use solid_reactive_ir::contract_semantics::certification::ProofDemandGraph;
/// use typefacts::InvocationDemand;
///
/// fn caller_assigns_demands(
///     graph: &ProofDemandGraph,
///     assignments: Vec<(String, InvocationDemand)>,
/// ) {
///     let _ = TypeFactsCertificationSchedule::new(graph, assignments);
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeFactsCertificationSchedule {
    demand_graph_root: String,
    invocations: Vec<ScheduledInvocation>,
    export_values: Vec<ScheduledExportValue>,
    verifier_sources: Vec<TranscriptSourceDigest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScheduledInvocation {
    demand: InvocationDemand,
    proof_demands: Vec<ScheduledProofDemand>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScheduledExportValue {
    demand: ExportValueDemand,
    proof_demands: Vec<ScheduledProofDemand>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScheduledProofDemand {
    id: String,
    family: ProofFamily,
    subject: ProofDemandSubject,
}

type ExpectedExportValueProofDemands =
    std::collections::BTreeMap<String, (ProofFamily, ProofDemandSubject)>;

fn export_value_schedule_proof_demands(
    graph: &ProofDemandGraph,
) -> Result<ExpectedExportValueProofDemands, TypeFactsCertificationError> {
    Ok(graph
        .demands()
        .iter()
        .filter(|demand| TYPE_FACTS_FAMILIES.contains(&demand.family()))
        .map(|demand| {
            (
                demand.id().as_str().to_owned(),
                (demand.family(), demand.subject().clone()),
            )
        })
        .collect())
}

fn preflight_export_value_schedule_compatibility(
    _graph: &ProofDemandGraph,
) -> Result<(), TypeFactsCertificationError> {
    Ok(())
}

impl TypeFactsCertificationSchedule {
    /// Builds a total schedule for the Type Facts-owned portion of a demand
    /// graph. `assignments` maps each exact proof demand ID to the exact call or
    /// construct expression the verifier will ask Type Facts to inspect.
    #[allow(dead_code)] // Called by the retained-plan transaction added in Slice 7.
    pub(crate) fn new(
        graph: &ProofDemandGraph,
        assignments: impl IntoIterator<Item = (String, InvocationDemand)>,
    ) -> Result<Self, TypeFactsCertificationError> {
        let expected = graph
            .demands()
            .iter()
            .filter(|demand| TYPE_FACTS_FAMILIES.contains(&demand.family()))
            .map(|demand| {
                (
                    demand.id().as_str().to_owned(),
                    (demand.family(), demand.subject().clone()),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut supplied = std::collections::BTreeMap::<String, InvocationDemand>::new();
        for (id, demand) in assignments {
            if !expected.contains_key(&id) {
                return Err(TypeFactsCertificationError::UnknownDemand(id));
            }
            if supplied.insert(id.clone(), demand).is_some() {
                return Err(TypeFactsCertificationError::DuplicateDemand(id));
            }
        }
        if supplied.len() != expected.len() {
            let missing = expected
                .keys()
                .find(|id| !supplied.contains_key(*id))
                .cloned()
                .unwrap_or_else(|| "unknown".into());
            return Err(TypeFactsCertificationError::MissingDemand(missing));
        }

        // Identical invocation requests share one producer transcript, but all
        // proof-demand identities remain individually bound to that response.
        let mut grouped = std::collections::BTreeMap::<InvocationKey, ScheduledInvocation>::new();
        for (id, demand) in supplied {
            let (family, subject) = expected
                .get(&id)
                .expect("supplied IDs were checked against expected demands")
                .clone();
            let proof = ScheduledProofDemand {
                id,
                family,
                subject,
            };
            let key = InvocationKey::from(&demand);
            grouped
                .entry(key)
                .and_modify(|scheduled| scheduled.proof_demands.push(proof.clone()))
                .or_insert_with(|| ScheduledInvocation {
                    demand,
                    proof_demands: vec![proof],
                });
        }
        for scheduled in grouped.values_mut() {
            scheduled
                .proof_demands
                .sort_by(|left, right| left.id.cmp(&right.id));
        }
        Ok(Self {
            demand_graph_root: graph.root().as_str().to_owned(),
            invocations: grouped.into_values().collect(),
            export_values: Vec::new(),
            verifier_sources: Vec::new(),
        })
    }

    /// Builds a total exported-value schedule. Export-root demands use the
    /// declaration expression; function and operation subjects additionally
    /// bind the independently replayed runtime implementation location.
    pub(crate) fn new_export_values(
        graph: &ProofDemandGraph,
        assignments: impl IntoIterator<Item = (String, ExportValueDemand)>,
    ) -> Result<Self, TypeFactsCertificationError> {
        let expected = export_value_schedule_proof_demands(graph)?;
        let mut supplied = std::collections::BTreeMap::<String, ExportValueDemand>::new();
        for (id, demand) in assignments {
            if !expected.contains_key(&id) {
                return Err(TypeFactsCertificationError::UnknownDemand(id));
            }
            if supplied.insert(id.clone(), demand).is_some() {
                return Err(TypeFactsCertificationError::DuplicateDemand(id));
            }
        }
        if supplied.len() != expected.len() {
            let missing = expected
                .keys()
                .find(|id| !supplied.contains_key(*id))
                .cloned()
                .unwrap_or_else(|| "unknown".into());
            return Err(TypeFactsCertificationError::MissingDemand(missing));
        }
        let mut grouped = std::collections::BTreeMap::<ExportValueKey, ScheduledExportValue>::new();
        for (id, demand) in supplied {
            let (family, subject) = expected
                .get(&id)
                .expect("supplied IDs were checked against expected demands")
                .clone();
            let proof = ScheduledProofDemand {
                id,
                family,
                subject,
            };
            let key = ExportValueKey::from(&demand);
            grouped
                .entry(key)
                .and_modify(|scheduled| scheduled.proof_demands.push(proof.clone()))
                .or_insert_with(|| ScheduledExportValue {
                    demand,
                    proof_demands: vec![proof],
                });
        }
        for scheduled in grouped.values_mut() {
            scheduled
                .proof_demands
                .sort_by(|left, right| left.id.cmp(&right.id));
        }
        Ok(Self {
            demand_graph_root: graph.root().as_str().to_owned(),
            invocations: Vec::new(),
            export_values: grouped.into_values().collect(),
            verifier_sources: Vec::new(),
        })
    }

    fn demands(&self) -> Vec<InvocationDemand> {
        self.invocations
            .iter()
            .map(|scheduled| scheduled.demand.clone())
            .collect()
    }

    fn export_value_demands(&self) -> Vec<ExportValueDemand> {
        self.export_values
            .iter()
            .map(|scheduled| scheduled.demand.clone())
            .collect()
    }

    fn proof_demand_ids(&self) -> impl Iterator<Item = String> + '_ {
        self.invocations
            .iter()
            .flat_map(|scheduled| scheduled.proof_demands.iter().map(|proof| proof.id.clone()))
            .chain(
                self.export_values.iter().flat_map(|scheduled| {
                    scheduled.proof_demands.iter().map(|proof| proof.id.clone())
                }),
            )
    }
}

/// Authority-bearing, family-checked Type Facts evidence. The contained
/// bindings can participate in structural coverage only because this value was
/// constructed from a direct live-session answer by [`CertificationPlan`].
pub struct VerifiedTypeFactsEvidence {
    bindings: Vec<WitnessBinding>,
    session_evidence_root: String,
}

impl VerifiedTypeFactsEvidence {
    #[must_use]
    pub fn witness_bindings(&self) -> &[WitnessBinding] {
        &self.bindings
    }

    #[must_use]
    pub fn session_evidence_root(&self) -> &str {
        &self.session_evidence_root
    }
}

#[allow(dead_code)] // Schedule grouping key; construction is intentionally crate-private.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct InvocationKey {
    path: String,
    start: u64,
    end: u64,
    callable_depth: usize,
    census: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ExportValueKey {
    path: String,
    start: u64,
    end: u64,
    callable_depth: usize,
}

impl From<&ExportValueDemand> for ExportValueKey {
    fn from(demand: &ExportValueDemand) -> Self {
        Self {
            path: demand.location.path.to_string(),
            start: demand.location.start_byte,
            end: demand.location.end_byte,
            callable_depth: demand.callable_depth,
        }
    }
}

impl From<&InvocationDemand> for InvocationKey {
    fn from(demand: &InvocationDemand) -> Self {
        Self {
            path: demand.location.path.to_string(),
            start: demand.location.start_byte,
            end: demand.location.end_byte,
            callable_depth: demand.callable_depth,
            census: demand.census,
        }
    }
}

/// Live policy-2 Type Facts acquisition session.
pub(crate) struct TypeFactsCertificationSession {
    _image: PrivateExecutionImage,
    session: Session,
}

impl TypeFactsCertificationSession {
    pub(crate) fn open(
        pin: &TypeFactsProducerPin,
        project_id: &str,
    ) -> Result<Self, TypeFactsCertificationError> {
        let image = PrivateExecutionImage::copy_from_pin(pin)?;
        let producer = Producer::pinned_for_certification(
            image.path(),
            pin.executable_sha256.as_str(),
            pin.source_manifest_sha256.as_str(),
        )?;
        let session = Session::open(producer, project_id, Vec::new())?;
        Ok(Self {
            _image: image,
            session,
        })
    }

    pub(crate) fn acquire(
        &mut self,
        plan: &CertificationPlan,
        schedule: &TypeFactsCertificationSchedule,
    ) -> Result<LiveInvocationAnswer, TypeFactsCertificationError> {
        verify_schedule_identity(
            "invocation acquisition",
            plan.demand_graph().root().as_str(),
            &schedule.demand_graph_root,
            "export_value_count",
            schedule.export_values.len(),
        )?;
        let context = CertificationInvocationContext::new(
            plan.snapshot_root(),
            plan.demand_graph().root().as_str(),
            schedule.proof_demand_ids(),
        )?;
        Ok(self
            .session
            .certification_invocations(context, &schedule.demands())?)
    }

    pub(crate) fn acquire_export_values(
        &mut self,
        plan: &CertificationPlan,
        schedule: &TypeFactsCertificationSchedule,
    ) -> Result<LiveExportValueAnswer, TypeFactsCertificationError> {
        verify_schedule_identity(
            "export-value acquisition",
            plan.demand_graph().root().as_str(),
            &schedule.demand_graph_root,
            "invocation_count",
            schedule.invocations.len(),
        )?;
        let context = CertificationInvocationContext::new(
            plan.snapshot_root(),
            plan.demand_graph().root().as_str(),
            schedule.proof_demand_ids(),
        )?;
        Ok(self
            .session
            .certification_export_values(context, &schedule.export_value_demands())?)
    }
}

/// Performs the export-value acquisition as one opaque native transaction:
/// materialize immutable bytes, derive the harness and schedule, launch the
/// pinned producer, and reconcile the still-live answer before any temporary
/// authority is destroyed.
pub(super) fn acquire_and_verify_export_values(
    plan: &CertificationPlan,
    pin: &TypeFactsProducerPin,
) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
    acquire_and_verify_export_values_batch(&[plan], pin)?
        .pop()
        .ok_or_else(|| {
            TypeFactsCertificationError::identity_mismatch(
                "single export-value acquisition",
                "answer_count",
                "1",
                "0",
            )
        })
}

/// Acquires independently bound answers for a complete alternative-case set
/// while sharing only immutable package materialization and the producer
/// program. Every request retains its own snapshot/demand-graph context and
/// is verified against its own opaque plan before any evidence is returned.
pub(super) fn acquire_and_verify_export_values_batch(
    plans: &[&CertificationPlan],
    pin: &TypeFactsProducerPin,
) -> Result<Vec<VerifiedTypeFactsEvidence>, TypeFactsCertificationError> {
    let sources = union_of_certification_sources(plans)?;
    acquire_and_verify_export_values_batch_with_dependencies(plans, &[], &sources, pin)
}

/// The declaration-only closure every case in this batch already
/// authenticated.
///
/// A batch is one package's alternative artifact cases — the batch identity
/// check already requires one snapshot root, package name, and package root —
/// so the union is that one package row's own authenticated bytes, and one
/// materialized project can serve every case. The union only ever *adds*
/// authenticated copies, never removes one, so it cannot manufacture the
/// partial-name shape that would let a hoisted copy stand in for a withheld
/// nested one. Each source keeps its exact canonical identity, so the census
/// still refuses any producer-consulted file outside one of these snapshots.
///
/// Two union members that project onto the same place are refused rather than
/// dropped. Per-plan retention already withheld every name whose own copies
/// collide, so reaching here means two distinct authenticated identities claim
/// one installed directory — self-contradictory input, not a dependency that
/// merely failed to authenticate.
fn union_of_certification_sources(
    plans: &[&CertificationPlan],
) -> Result<Vec<super::dependencies::VerifiedGraphSourcePackage>, TypeFactsCertificationError> {
    let mut sources = std::collections::BTreeMap::new();
    for plan in plans {
        for source in &plan.certification_sources {
            sources
                .entry(source.identity.clone())
                .or_insert_with(|| (*plan, source.clone()));
        }
    }
    let mut targets = std::collections::BTreeSet::new();
    for (plan, source) in sources.values() {
        let marker = private_project_package_marker(
            plan,
            &source.installed_package_root,
            source.snapshot.package_name(),
        );
        if !targets.insert(marker.clone()) {
            return Err(TypeFactsCertificationError::identity_mismatch(
                "certification-source union",
                "materialization_target",
                format!("unique {}", diagnostic_identity_path(&marker)),
                format!("duplicate {}", diagnostic_identity_path(&marker)),
            ));
        }
    }
    Ok(sources.into_values().map(|(_, source)| source).collect())
}

/// Withholds every source package name whose authenticated copies cannot all
/// occupy distinct places in the private project.
///
/// `private_project_package_target` projects a copy faithfully when it sits
/// under the owner or under the owner's `node_modules` installation root, which
/// is what preserves hoisting and shadowing. Anything else falls back to the
/// project's top-level `node_modules/<name>` — so two such copies of one name,
/// or a copy whose name is the owner package's own, land on one path. That is
/// an `AlreadyExists` write, which the immutable-file writer reports as a source
/// census failure: a hard failure class ordinary root certification must not
/// have, for the same reason it must not refuse on an unacquirable archive.
///
/// The remedy is the F1 rule again, for the same reason: withhold the whole
/// name. Keeping one of the colliding copies would be substitution, and a
/// half-materialized name is exactly what makes a hoisted copy answer for a
/// missing nested one.
pub(super) fn retain_collision_free_source_packages(
    plan: &CertificationPlan,
    sources: Vec<super::dependencies::VerifiedGraphSourcePackage>,
) -> Vec<super::dependencies::VerifiedGraphSourcePackage> {
    let owner_marker = format!(
        "/node_modules/{}/",
        plan.snapshot.package_name().replace('\\', "/")
    );
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    let mut withheld = std::collections::BTreeSet::new();
    for source in &sources {
        let marker = private_project_package_marker(
            plan,
            &source.installed_package_root,
            source.snapshot.package_name(),
        );
        if marker == owner_marker {
            withheld.insert(source.snapshot.package_name().to_owned());
        }
        if seen.insert(marker, 0).is_some() {
            withheld.insert(source.snapshot.package_name().to_owned());
        }
    }
    sources
        .into_iter()
        .filter(|source| !withheld.contains(source.snapshot.package_name()))
        .collect()
}

fn preflight_export_value_plans(
    plans: &[&CertificationPlan],
) -> Result<(), TypeFactsCertificationError> {
    for plan in plans {
        preflight_export_value_schedule_compatibility(plan.demand_graph())?;
    }
    Ok(())
}

/// Reports one certification stage's wall time under `SOLID_CHECKER_TIMINGS`,
/// as a JSON line on stderr like the analyzer's own stage timings.
pub fn report_certification_timing(
    stage: &str,
    started: std::time::Instant,
    detail: serde_json::Value,
) {
    if std::env::var_os("SOLID_CHECKER_TIMINGS").is_none() {
        return;
    }
    let mut line = serde_json::json!({
        "certificationStage": stage,
        "elapsedNs": started.elapsed().as_nanos(),
    });
    if let (Some(object), Some(extra)) = (line.as_object_mut(), detail.as_object()) {
        for (key, value) in extra {
            object.insert(key.clone(), value.clone());
        }
    }
    eprintln!("{line}");
}

fn acquire_and_verify_export_values_batch_with_dependencies(
    plans: &[&CertificationPlan],
    dependencies: &[&CertificationPlan],
    sources: &[super::dependencies::VerifiedGraphSourcePackage],
    pin: &TypeFactsProducerPin,
) -> Result<Vec<VerifiedTypeFactsEvidence>, TypeFactsCertificationError> {
    let first = plans.first().copied().ok_or_else(|| {
        TypeFactsCertificationError::identity_mismatch(
            "export-value batch",
            "plan_count",
            "at least one",
            "0",
        )
    })?;
    if let Some(plan) = plans
        .iter()
        .find(|plan| plan.snapshot_root() != first.snapshot_root())
    {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value batch",
            "snapshot_root",
            first.snapshot_root().to_owned(),
            plan.snapshot_root().to_owned(),
        ));
    }
    if let Some(plan) = plans
        .iter()
        .find(|plan| plan.snapshot.package_name() != first.snapshot.package_name())
    {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value batch",
            "package_name",
            first.snapshot.package_name().to_owned(),
            plan.snapshot.package_name().to_owned(),
        ));
    }
    // This compatibility check consumes only the verifier-retained demand
    // graph and is repeated by schedule construction below. An incompatible
    // graph can never produce an export-value schedule, so reject its exact
    // demand before copying authenticated package bytes into a private project.
    preflight_export_value_plans(plans)
        .map_err(|error| error.at_stage("export-value schedule derivation"))?;
    let started = std::time::Instant::now();
    let project = PrivateTypeFactsProject::materialize(first, dependencies, plans, sources)
        .map_err(|error| error.at_stage("private project materialization"))?;
    report_certification_timing(
        "private-project-materialization",
        started,
        serde_json::json!({ "sources": sources.len(), "dependencies": dependencies.len() }),
    );
    let schedules = derive_export_value_schedules(plans, &project, false)
        .map_err(|error| error.at_stage("export-value schedule derivation"))?;
    let project_id = project.project_id().to_str().ok_or_else(|| {
        TypeFactsCertificationError::ProducerProvenance(
            "private Type Facts project path is not valid UTF-8".into(),
        )
    })?;
    let started = std::time::Instant::now();
    let mut session = TypeFactsCertificationSession::open(pin, project_id)
        .map_err(|error| error.at_stage("pinned producer launch"))?;
    report_certification_timing("pinned-producer-launch", started, serde_json::json!({}));
    let started = std::time::Instant::now();
    let evidence = plans
        .iter()
        .zip(&schedules)
        .map(|(plan, schedule)| {
            let live = session
                .acquire_export_values(plan, schedule)
                .map_err(|error| error.at_stage("live export-value acquisition"))?;
            verify_live_export_value_answer_with_dependencies(
                plan,
                schedule,
                &live,
                dependencies,
                sources,
                &project,
            )
            .map_err(|error| error.at_stage("live export-value verification"))
        })
        .collect::<Result<Vec<_>, _>>();
    report_certification_timing(
        "live-export-value-acquisition-and-verification",
        started,
        serde_json::json!({ "plans": plans.len() }),
    );
    let started = std::time::Instant::now();
    drop(session);
    drop(project);
    report_certification_timing("private-project-removal", started, serde_json::json!({}));
    evidence
}

pub(super) struct GraphExportValueRequest<'a> {
    pub(super) plan: &'a CertificationPlan,
    pub(super) dependencies: Vec<&'a CertificationPlan>,
    pub(super) sources: &'a [super::dependencies::VerifiedGraphSourcePackage],
}

/// Acquires all exported-value answers for one opaque graph through one pinned
/// producer session. The private project contains the authenticated union of
/// graph bytes, while verification still receives only each node's reachable
/// dependencies and source packages; run-wide batching therefore does not
/// widen any node's authority.
pub(super) fn acquire_and_verify_graph_export_values(
    project_root: &CertificationPlan,
    requests: &[GraphExportValueRequest<'_>],
    pin: &TypeFactsProducerPin,
) -> Result<Vec<VerifiedTypeFactsEvidence>, TypeFactsCertificationError> {
    if requests.is_empty() {
        return Ok(Vec::new());
    }
    let plans = requests
        .iter()
        .map(|request| request.plan)
        .collect::<Vec<_>>();
    // Preserve the same graph schedule-derivation error and deterministic
    // request ordering while avoiding graph-wide materialization for a case
    // that the schedule constructor must refuse.
    preflight_export_value_plans(&plans)
        .map_err(|error| error.at_stage("graph export-value schedule derivation"))?;
    let mut materialized = std::collections::BTreeMap::new();
    materialized.insert(private_project_plan_key(project_root), project_root);
    let mut source_packages = std::collections::BTreeMap::new();
    for request in requests {
        materialized.insert(private_project_plan_key(request.plan), request.plan);
        for dependency in &request.dependencies {
            materialized.insert(private_project_plan_key(dependency), *dependency);
        }
        for source in request.sources {
            source_packages.insert(source.identity.clone(), source);
        }
    }
    let root_key = private_project_plan_key(project_root);
    let census_dependencies = materialized.values().copied().collect::<Vec<_>>();
    let dependencies = materialized
        .iter()
        .filter_map(|(key, plan)| (key != &root_key).then_some(*plan))
        .collect::<Vec<_>>();
    let sources = source_packages.into_values().cloned().collect::<Vec<_>>();
    let source_refs = sources.iter().collect::<Vec<_>>();
    // Every plan whose exports this one project must be able to transcribe.
    // Materialization deduplicates by installed identity, so a package's
    // alternative artifact cases collapse to a single `package_roots` entry;
    // the program's file census must still cover each case's own runtime
    // closure. See `PrivateTypeFactsProject::materialize_with_source_refs`.
    let mut program_plans = vec![project_root];
    for request in requests {
        program_plans.push(request.plan);
        program_plans.extend(request.dependencies.iter().copied());
    }
    let started = std::time::Instant::now();
    let project = PrivateTypeFactsProject::materialize_with_source_refs(
        project_root,
        &dependencies,
        &program_plans,
        &source_refs,
    )
    .map_err(|error| error.at_stage("private graph project materialization"))?;
    report_certification_timing(
        "private-project-materialization",
        started,
        serde_json::json!({ "sources": source_refs.len(), "dependencies": dependencies.len(), "graph": true }),
    );
    let schedules = derive_export_value_schedules(&plans, &project, true)
        .map_err(|error| error.at_stage("graph export-value schedule derivation"))?;
    let project_id = project.project_id().to_str().ok_or_else(|| {
        TypeFactsCertificationError::ProducerProvenance(
            "private Type Facts graph project path is not valid UTF-8".into(),
        )
    })?;
    let started = std::time::Instant::now();
    let mut session = TypeFactsCertificationSession::open(pin, project_id)
        .map_err(|error| error.at_stage("pinned graph producer launch"))?;
    report_certification_timing(
        "pinned-producer-launch",
        started,
        serde_json::json!({ "graph": true }),
    );
    let started = std::time::Instant::now();
    let evidence = requests
        .iter()
        .zip(&schedules)
        .map(|(request, schedule)| {
            let live = session
                .acquire_export_values(request.plan, schedule)
                .map_err(|error| {
                    error.at_graph_node(request.plan, "live graph export-value acquisition")
                })?;
            verify_live_export_value_answer_with_project_census(
                request.plan,
                schedule,
                &live,
                &request.dependencies,
                &census_dependencies,
                &sources,
                Some(&project),
            )
            .map_err(|error| {
                error.at_graph_node(request.plan, "live graph export-value verification")
            })
        })
        .collect::<Result<Vec<_>, _>>();
    report_certification_timing(
        "live-export-value-acquisition-and-verification",
        started,
        serde_json::json!({ "plans": requests.len(), "graph": true }),
    );
    let started = std::time::Instant::now();
    drop(session);
    drop(project);
    report_certification_timing(
        "private-project-removal",
        started,
        serde_json::json!({ "graph": true }),
    );
    evidence
}

struct PrivateTypeFactsProject {
    root: PathBuf,
    project_id: PathBuf,
    harness: PathBuf,
    package_roots: std::collections::BTreeMap<(String, String), PathBuf>,
    source_roots: std::collections::BTreeMap<String, PathBuf>,
}

impl PrivateTypeFactsProject {
    fn materialize(
        plan: &CertificationPlan,
        dependencies: &[&CertificationPlan],
        program_plans: &[&CertificationPlan],
        sources: &[super::dependencies::VerifiedGraphSourcePackage],
    ) -> Result<Self, TypeFactsCertificationError> {
        let source_refs = sources.iter().collect::<Vec<_>>();
        Self::materialize_with_source_refs(plan, dependencies, program_plans, &source_refs)
    }

    /// `program_plans` names every plan whose exports this project must be able
    /// to transcribe an implementation for. It is not an authority input: the
    /// bytes come from the snapshots `plan`, `dependencies`, and `sources`
    /// already materialize, and a plan listed here that materialized no package
    /// root is refused rather than guessed at.
    fn materialize_with_source_refs(
        plan: &CertificationPlan,
        dependencies: &[&CertificationPlan],
        program_plans: &[&CertificationPlan],
        sources: &[&super::dependencies::VerifiedGraphSourcePackage],
    ) -> Result<Self, TypeFactsCertificationError> {
        let requested = std::env::temp_dir().join(format!(
            "solid-checker-typefacts-project-{}-{}",
            std::process::id(),
            PRIVATE_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&requested)?;
        set_directory_permissions(&requested)?;
        let root = fs::canonicalize(&requested)?;
        let package_root = root.join("node_modules").join(plan.snapshot.package_name());
        materialize_snapshot(&plan.snapshot, &package_root, &program_root_paths(plan))?;
        let mut package_roots = std::collections::BTreeMap::from([(
            private_project_plan_key(plan),
            package_root.clone(),
        )]);
        let original_package_root = Path::new(&plan.resolved_import.package_root);
        for dependency in dependencies {
            let target = private_project_package_target(
                &root,
                &package_root,
                original_package_root,
                Path::new(&dependency.resolved_import.package_root),
                dependency.snapshot.package_name(),
            );
            materialize_snapshot(
                &dependency.snapshot,
                &target,
                &program_root_paths(dependency),
            )?;
            package_roots.insert(private_project_plan_key(dependency), target);
        }
        let mut source_roots = std::collections::BTreeMap::new();
        // Compiler sources may be linked from the shared materialized store
        // instead of written, one symlink per package. A source whose package
        // directory has another materialized package nested inside it must
        // be written, since nothing may ever be created inside a store entry.
        let source_targets = sources
            .iter()
            .map(|source| {
                private_project_package_target(
                    &root,
                    &package_root,
                    original_package_root,
                    Path::new(&source.installed_package_root),
                    source.snapshot.package_name(),
                )
            })
            .collect::<Vec<_>>();
        let all_targets = package_roots
            .values()
            .cloned()
            .chain(source_targets.iter().cloned())
            .collect::<Vec<_>>();
        let store = materialized_store_root();
        let mut linked_any = false;
        for (source, target) in sources.iter().zip(source_targets) {
            let has_nested_target = all_targets
                .iter()
                .any(|other| other != &target && other.starts_with(&target));
            let linked = match &store {
                Some(store) if !has_nested_target => {
                    link_snapshot_from_store(store, &source.snapshot, &target)?
                }
                _ => false,
            };
            if linked {
                linked_any = true;
            } else {
                materialize_snapshot(
                    &source.snapshot,
                    &target,
                    &std::collections::BTreeSet::new(),
                )?;
            }
            source_roots.insert(source.identity.clone(), target);
        }
        let harness = root.join("solid-checker-export-values.ts");
        let project_id = root.join("tsconfig.json");
        let mut files = vec![harness.to_string_lossy().into_owned()];
        for candidate in std::iter::once(plan)
            .chain(dependencies.iter().copied())
            .chain(program_plans.iter().copied())
        {
            let candidate_root = package_roots
                .get(&private_project_plan_key(candidate))
                .ok_or_else(|| {
                    TypeFactsCertificationError::identity_mismatch(
                        "private project materialization",
                        "package_root",
                        diagnostic_plan_key(&private_project_plan_key(candidate)),
                        format!("{} different known package root(s)", package_roots.len()),
                    )
                })?;
            files.extend(
                candidate
                    .verified_exports
                    .runtime_paths()
                    .map(|path| candidate_root.join(path).to_string_lossy().into_owned()),
            );
            files.extend(
                closure_runtime_modules(candidate)
                    .into_iter()
                    .map(|path| candidate_root.join(path).to_string_lossy().into_owned()),
            );
        }
        files.sort();
        files.dedup();
        let configuration = serde_json::to_vec_pretty(&serde_json::json!({
            "compilerOptions": {
                "strict": true,
                "skipLibCheck": false,
                "module": "esnext",
                "moduleResolution": "bundler",
                "target": "esnext",
                "jsx": "preserve",
                "allowJs": true,
                "checkJs": false,
                "maxNodeModuleJsDepth": 100,
                "allowImportingTsExtensions": true,
                "moduleDetection": "force",
                "types": [],
                // A linked source package is reached through its symlink;
                // without this the producer would realpath every module into
                // the store and the census could not attribute it to its
                // project root. Only set when a link exists, so an unlinked
                // project's configuration is byte-identical to before.
                "preserveSymlinks": linked_any
            },
            "files": files
        }))
        .map_err(|error| {
            TypeFactsCertificationError::ProducerProvenance(format!(
                "could not encode private Type Facts project: {error}"
            ))
        })?;
        write_new_private_project_file(&project_id, &configuration)?;
        Ok(Self {
            root,
            project_id,
            harness,
            package_roots,
            source_roots,
        })
    }

    fn project_id(&self) -> &Path {
        &self.project_id
    }

    fn package_root(&self, plan: &CertificationPlan) -> Result<&Path, TypeFactsCertificationError> {
        self.package_roots
            .get(&private_project_plan_key(plan))
            .map(PathBuf::as_path)
            .ok_or_else(|| {
                TypeFactsCertificationError::identity_mismatch(
                    "private project lookup",
                    "package_root",
                    diagnostic_plan_key(&private_project_plan_key(plan)),
                    format!(
                        "{} different known package root(s)",
                        self.package_roots.len()
                    ),
                )
            })
    }

    fn source_root(
        &self,
        source: &super::dependencies::VerifiedGraphSourcePackage,
    ) -> Result<&Path, TypeFactsCertificationError> {
        self.source_roots
            .get(&source.identity)
            .map(PathBuf::as_path)
            .ok_or_else(|| {
                TypeFactsCertificationError::identity_mismatch(
                    "private project lookup",
                    "source_root",
                    diagnostic_identity_path(&source.installed_package_root),
                    format!("{} different known source root(s)", self.source_roots.len()),
                )
            })
    }
}

fn private_project_plan_key(plan: &CertificationPlan) -> (String, String) {
    (
        plan.snapshot_root().to_owned(),
        plan.resolved_import.package_root.clone(),
    )
}

/// Package-relative runtime modules this plan's independently replayed module
/// closure reaches, entrypoints included.
///
/// The witness program's roots are declaration modules: TypeScript resolves
/// every re-export specifier to the sibling `.d.ts`, so a runtime chunk that
/// only a re-export names is never a program member and the producer has no
/// source file to transcribe its implementation from — the transcript comes
/// back open with `sourceUnavailable` even though the package ships the file.
/// Listing the closure's runtime modules as program roots repairs exactly that
/// and asserts nothing on its own: program membership is not a flow, a fact, or
/// a claim, and every remaining premise is still proved against the snapshot.
///
/// The closure is this package's own module graph — `resolve_local` classifies
/// every bare specifier as external and records it as a dependency edge or an
/// opaque frontier instead of visiting it — so this never widens the program
/// with a dependency's internals beyond what materialization already placed.
/// Non-module resolution inputs (assets) carry a different closure role and are
/// excluded; a path the snapshot does not carry is skipped, so a demand whose
/// module is genuinely absent stays open exactly as before.
fn closure_runtime_modules(plan: &CertificationPlan) -> Vec<&str> {
    closure_runtime_module_paths(&plan.verified_closure.manifest().entries, &|path| {
        plan.snapshot.read(path).is_some()
    })
}

/// The runtime-axis modules of one verified closure, keeping only the paths
/// `materialized` confirms the private project actually carries. Listing a path
/// the snapshot never supplied would ask the producer for a file outside the
/// authenticated bytes, so absence is dropped rather than asserted.
fn closure_runtime_module_paths<'a>(
    entries: &'a [crate::contract_interface::ClosureEntry],
    materialized: &impl Fn(&str) -> bool,
) -> Vec<&'a str> {
    entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.role,
                crate::contract_interface::ClosureFileRole::Runtime
                    | crate::contract_interface::ClosureFileRole::LiteralDynamicChunk
            )
        })
        .map(|entry| entry.path.trim_start_matches("./"))
        .filter(|path| materialized(path))
        .collect()
}

fn private_project_package_target(
    project_root: &Path,
    projected_owner_root: &Path,
    original_owner_root: &Path,
    original_package_root: &Path,
    package_name: &str,
) -> PathBuf {
    if let Ok(relative) = original_package_root.strip_prefix(original_owner_root)
        && relative.starts_with("node_modules")
    {
        return projected_owner_root.join(relative);
    }
    if let Some(installation_root) = original_owner_root
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "node_modules"))
        && let Ok(relative) = original_package_root.strip_prefix(installation_root)
    {
        return project_root.join("node_modules").join(relative);
    }
    project_root.join("node_modules").join(package_name)
}

/// The file extensions a TypeScript program opens: TypeScript and JavaScript
/// modules in every module-format spelling (which covers `.d.ts`, `.d.mts` and
/// `.d.cts`) and JSON, including every `package.json` module resolution reads.
/// TypeScript resolves a specifier by probing these extensions and reads
/// nothing else — not source maps, declaration maps, READMEs, stylesheets,
/// assets, or extensionless files — so a private project that carries only
/// these files gives the producer the same program the whole snapshot would.
const TYPE_FACTS_PROGRAM_EXTENSIONS: [&str; 9] = [
    ".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs", ".json",
];

/// Whether a snapshot file is one a TypeScript program can load. Case-exact:
/// TypeScript's extension probing is, too.
pub(super) fn type_facts_program_can_load(relative_path: &str) -> bool {
    TYPE_FACTS_PROGRAM_EXTENSIONS
        .iter()
        .any(|extension| relative_path.ends_with(extension))
}

/// The snapshot paths a plan names as program roots: its verified runtime
/// entrypoints and the runtime modules of its replayed closure. These are
/// materialized whatever their spelling, so the roots the project lists always
/// exist on disk exactly as before this filter.
fn program_root_paths(plan: &CertificationPlan) -> std::collections::BTreeSet<String> {
    plan.verified_exports
        .runtime_paths()
        .map(|path| path.trim_start_matches("./").to_owned())
        .chain(
            closure_runtime_modules(plan)
                .into_iter()
                .map(|path| path.to_owned()),
        )
        .collect()
}

/// Writes the files of one authenticated snapshot the producer can read.
///
/// Every file of every source snapshot used to be written — for a wide root
/// that is thousands of files per certification, a quarter of them source
/// maps, READMEs and assets — and the private project's creation and removal
/// then dominated the certification's wall time, contended across every
/// concurrent child. Only files with `TYPE_FACTS_PROGRAM_EXTENSIONS` are written
/// now, plus `program_roots` unconditionally. Nothing about authority moves:
/// the producer's transcript still names every file it read with its digest,
/// the source census still verifies each against the in-memory snapshot, and
/// the closure's declaration entries it checks are all `.d.ts`-family files
/// that are kept. A package that ships no loadable file still gets its root
/// directory, so resolution failing into it fails the same way it did.
fn materialize_snapshot(
    snapshot: &super::ArtifactSnapshot,
    package_root: &Path,
    program_roots: &std::collections::BTreeSet<String>,
) -> Result<(), TypeFactsCertificationError> {
    fs::create_dir_all(package_root)?;
    for (relative, bytes) in snapshot.files.iter() {
        if !type_facts_program_can_load(relative) && !program_roots.contains(relative.as_str()) {
            continue;
        }
        let target = package_root.join(relative);
        write_immutable_project_file(&target, bytes)?;
    }
    Ok(())
}

/// Root of the shared materialized store, or `None` when every private project
/// writes its own copy of every source package (the default).
///
/// The store holds, per authenticated snapshot, exactly the files
/// `materialize_snapshot` would write for a compiler source — the loadable
/// files — under a directory named by the snapshot's content root, plus a
/// manifest of their digests. A private project links a source package to its
/// entry with one symlink instead of writing thousands of files, and the
/// producer runs with `preserveSymlinks` so every path it reports stays inside
/// the project. Authority does not depend on the store: before an entry is
/// linked, every file it holds is re-read and its digest compared with the
/// in-memory snapshot (and the manifest with the loadable set), an entry that
/// disagrees is rebuilt from the snapshot, and the source census afterwards
/// still verifies every file the producer reports reading against the snapshot
/// bytes and refuses any it cannot attribute. A tampered or stale entry can
/// therefore cost a refusal, never a wrong receipt.
///
/// The store also holds the verified execution images (`images/<digest>/`):
/// see `PrivateExecutionImage::shared_from_pin` for why launching one
/// long-lived image instead of a fresh copy per certification matters.
pub(super) fn materialized_store_root() -> Option<PathBuf> {
    static ROOT: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        std::env::var_os("SOLID_CHECKER_MATERIALIZED_STORE")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    })
    .clone()
}

const MATERIALIZED_STORE_FORMAT: &str = "v1";
const MATERIALIZED_STORE_MANIFEST: &str = ".solid-checker-materialized.json";

#[derive(serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaterializedStoreManifest {
    format: String,
    snapshot_root: String,
    files: Vec<MaterializedStoreFile>,
}

#[derive(serde::Serialize, Deserialize)]
struct MaterializedStoreFile {
    path: String,
    sha256: String,
}

/// The loadable files of `snapshot` with their digests, in path order: what a
/// store entry must hold, exactly.
fn loadable_snapshot_files(snapshot: &super::ArtifactSnapshot) -> Vec<(&str, &[u8], String)> {
    snapshot
        .files
        .iter()
        .filter(|(path, _)| type_facts_program_can_load(path))
        .map(|(path, bytes)| {
            (
                path.as_str(),
                &bytes[..],
                format!("sha256:{:x}", Sha256::digest(bytes)),
            )
        })
        .collect()
}

fn materialized_store_entry(store: &Path, snapshot: &super::ArtifactSnapshot) -> PathBuf {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker-materialized-store:");
    hash.update(MATERIALIZED_STORE_FORMAT.as_bytes());
    hash.update(b":loadable-v1\0");
    hash.update(snapshot.root().as_bytes());
    let key = format!("{:x}", hash.finalize());
    store
        .join(MATERIALIZED_STORE_FORMAT)
        .join(&key[..2])
        .join(key)
}

/// Whether `entry` holds exactly the loadable files of `snapshot`, byte for
/// byte: the manifest must name the same paths with the same digests, and every
/// file's bytes on disk must hash to its digest.
fn materialized_store_entry_is_exact(
    entry: &Path,
    snapshot: &super::ArtifactSnapshot,
    expected: &[(&str, &[u8], String)],
) -> bool {
    let Ok(manifest_bytes) = fs::read(entry.join(MATERIALIZED_STORE_MANIFEST)) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_slice::<MaterializedStoreManifest>(&manifest_bytes) else {
        return false;
    };
    if manifest.format != MATERIALIZED_STORE_FORMAT
        || manifest.snapshot_root != snapshot.root()
        || manifest.files.len() != expected.len()
    {
        return false;
    }
    for (recorded, (path, _, digest)) in manifest.files.iter().zip(expected) {
        if recorded.path != *path || recorded.sha256 != *digest {
            return false;
        }
        match fs::read(entry.join(path)) {
            Ok(bytes) if format!("sha256:{:x}", Sha256::digest(&bytes)) == *digest => {}
            _ => return false,
        }
    }
    true
}

/// Writes a fresh entry for `snapshot` beside `entry` and moves it into place.
/// A concurrent writer's entry winning the rename is left as it is (it is
/// re-verified by the caller); an entry that failed verification is moved
/// aside first, so a project already linked to its path keeps resolving to
/// byte-identical content.
fn build_materialized_store_entry(
    entry: &Path,
    snapshot: &super::ArtifactSnapshot,
    expected: &[(&str, &[u8], String)],
) -> Result<(), TypeFactsCertificationError> {
    let parent = entry.parent().ok_or_else(|| {
        TypeFactsCertificationError::ProducerProvenance(
            "materialized store entry has no parent directory".into(),
        )
    })?;
    fs::create_dir_all(parent)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let staging = parent.join(format!(
        ".{}.staging-{}-{nonce}",
        entry
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("entry"),
        std::process::id()
    ));
    let result = (|| {
        fs::create_dir(&staging)?;
        set_directory_permissions(&staging)?;
        for (path, bytes, _) in expected {
            write_immutable_project_file(&staging.join(path), bytes)?;
        }
        let manifest = MaterializedStoreManifest {
            format: MATERIALIZED_STORE_FORMAT.into(),
            snapshot_root: snapshot.root().to_owned(),
            files: expected
                .iter()
                .map(|(path, _, digest)| MaterializedStoreFile {
                    path: (*path).to_owned(),
                    sha256: digest.clone(),
                })
                .collect(),
        };
        write_new_private_project_file(
            &staging.join(MATERIALIZED_STORE_MANIFEST),
            &serde_json::to_vec(&manifest).map_err(|error| {
                TypeFactsCertificationError::ProducerProvenance(format!(
                    "could not encode materialized store manifest: {error}"
                ))
            })?,
        )?;
        if entry.exists() {
            let retired = parent.join(format!(
                ".{}.retired-{}-{nonce}",
                entry
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("entry"),
                std::process::id()
            ));
            fs::rename(entry, &retired)?;
            let _ = fs::remove_dir_all(&retired);
        }
        match fs::rename(&staging, entry) {
            Ok(()) => Ok(()),
            // Another certification published the same content-addressed
            // entry first; ours is redundant.
            Err(error)
                if entry.exists()
                    && matches!(
                        error.kind(),
                        std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::DirectoryNotEmpty
                    ) =>
            {
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

/// Links `target` to the store entry for `snapshot`, creating or repairing the
/// entry first. Returns `Ok(false)` when the snapshot cannot use the store (it
/// carries a file at the manifest's reserved name) so the caller writes it.
fn link_snapshot_from_store(
    store: &Path,
    snapshot: &super::ArtifactSnapshot,
    target: &Path,
) -> Result<bool, TypeFactsCertificationError> {
    if snapshot.read(MATERIALIZED_STORE_MANIFEST).is_some() {
        return Ok(false);
    }
    let expected = loadable_snapshot_files(snapshot);
    let entry = materialized_store_entry(store, snapshot);
    if !materialized_store_entry_is_exact(&entry, snapshot, &expected) {
        build_materialized_store_entry(&entry, snapshot, &expected)?;
        if !materialized_store_entry_is_exact(&entry, snapshot, &expected) {
            return Err(TypeFactsCertificationError::SourceCensus(format!(
                "materialized store entry for {} could not be established",
                snapshot.package_name()
            )));
        }
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::symlink_metadata(target) {
        Ok(existing) if existing.file_type().is_symlink() => {
            let Ok(linked) = fs::read_link(target) else {
                return Err(TypeFactsCertificationError::SourceCensus(format!(
                    "distinct authenticated snapshots collide at {}",
                    target.display()
                )));
            };
            if linked == entry {
                return Ok(true);
            }
            // Another snapshot's entry occupies this path. As with written
            // copies, identical loadable bytes are one materialization and
            // anything else is a collision; nothing is ever written through
            // the existing link.
            if materialized_store_entry_is_exact(&linked, snapshot, &expected) {
                return Ok(true);
            }
            return Err(TypeFactsCertificationError::SourceCensus(format!(
                "distinct authenticated snapshots collide at {}",
                target.display()
            )));
        }
        // A package already written here (a dependency plan, or a source that
        // had to be written): `materialize_snapshot` keeps the byte-identical
        // rule that has always governed two snapshots meeting at one path.
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&entry, target)?;
    #[cfg(not(unix))]
    return Ok(false);
    #[cfg(unix)]
    Ok(true)
}

fn write_immutable_project_file(
    target: &Path,
    bytes: &[u8],
) -> Result<(), TypeFactsCertificationError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    match OpenOptions::new().write(true).create_new(true).open(target) {
        Ok(mut file) => {
            file.write_all(bytes)?;
            // This project is private to the still-live certification transaction
            // and is deleted when that transaction drops it. Closing the file makes
            // the exact bytes visible to the producer; the live source census and
            // transcript digest establish authority, so crash durability is neither
            // required nor reused as evidence here.
            drop(file);
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if fs::read(target)? == bytes {
                Ok(())
            } else {
                Err(TypeFactsCertificationError::SourceCensus(format!(
                    "distinct authenticated snapshots collide at {}",
                    target.display()
                )))
            }
        }
        Err(error) => Err(error.into()),
    }
}

impl Drop for PrivateTypeFactsProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

type ExportHarnessSubject = (String, String);
type ExportResolutionVariant = (String, String, String, String);
type ExportResolutionVariants = std::collections::BTreeMap<
    ExportHarnessSubject,
    std::collections::BTreeSet<ExportResolutionVariant>,
>;

fn derive_export_value_schedules(
    plans: &[&CertificationPlan],
    project: &PrivateTypeFactsProject,
    force_exact_subjects: bool,
) -> Result<Vec<TypeFactsCertificationSchedule>, TypeFactsCertificationError> {
    let mut resolution_variants = ExportResolutionVariants::new();
    for plan in plans {
        for demand in plan
            .demand_graph()
            .demands()
            .iter()
            .filter(|demand| TYPE_FACTS_FAMILIES.contains(&demand.family()))
        {
            let (artifact_case, export) = proof_artifact_export(demand.subject());
            let public_specifier =
                public_export_harness_specifier(plan, artifact_case, demand.id().as_str())?;
            let (declaration_path, declaration_selector, declaration_export) = plan
                .verified_exports
                .declaration_binding(export)
                .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
                    demand: demand.id().as_str().to_owned(),
                    reason: "demanded export has no snapshot-verified declaration binding".into(),
                })?;
            resolution_variants
                .entry((public_specifier, export.to_owned()))
                .or_default()
                .insert((
                    declaration_path.to_owned(),
                    declaration_selector.to_owned(),
                    declaration_export.to_owned(),
                    plan.snapshot_root().to_owned(),
                ));
        }
    }
    let mut subjects = std::collections::BTreeMap::<(String, String), String>::new();
    for plan in plans {
        for demand in plan
            .demand_graph()
            .demands()
            .iter()
            .filter(|demand| TYPE_FACTS_FAMILIES.contains(&demand.family()))
        {
            let (artifact_case, export) = proof_artifact_export(demand.subject());
            let subject = export_value_harness_subject(
                project,
                plan,
                artifact_case,
                export,
                demand.id().as_str(),
                &resolution_variants,
                force_exact_subjects,
            )?;
            subjects.entry(subject).or_default();
        }
    }
    if subjects.is_empty() {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: "export-value-schedule".into(),
            reason: "the plan has no Type Facts-owned exported-value demand".into(),
        });
    }

    let declaration_roots = plans
        .iter()
        .map(|plan| {
            snapshot_module_harness_specifier(
                project,
                plan,
                plan.verified_resolution.declarations_path(),
            )
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    let mut harness = String::new();
    for root in declaration_roots {
        let quoted = serde_json::to_string(&root).map_err(|error| {
            TypeFactsCertificationError::ProducerProvenance(format!(
                "could not encode verifier declaration root: {error}"
            ))
        })?;
        harness.push_str(&format!("import {quoted};\n"));
    }
    for (index, ((specifier, declaration_export), local)) in subjects.iter_mut().enumerate() {
        *local = append_exact_harness_import(&mut harness, index, specifier, declaration_export)?;
    }
    let mut locations = std::collections::BTreeMap::new();
    for (subject, local) in &subjects {
        harness.push_str("void ");
        let start = harness.len();
        harness.push_str(local);
        let end = harness.len();
        harness.push_str(";\n");
        locations.insert(subject.clone(), (start, end));
    }
    write_new_private_project_file(&project.harness, harness.as_bytes())?;
    let harness_digest = format!("sha256:{:x}", Sha256::digest(harness.as_bytes()));

    plans
        .iter()
        .map(|plan| {
            let callable_depths = plan
                .demand_graph()
                .demands()
                .iter()
                .filter(|demand| TYPE_FACTS_FAMILIES.contains(&demand.family()))
                .try_fold(
                    std::collections::BTreeMap::<(String, String), usize>::new(),
                    |mut depths, demand| {
                        let (artifact_case, export) = proof_artifact_export(demand.subject());
                        let depth = export_value_callable_depth(plan, demand)?;
                        depths
                            .entry((artifact_case.to_owned(), export.to_owned()))
                            .and_modify(|current| *current = (*current).max(depth))
                            .or_insert(depth);
                        Ok::<_, TypeFactsCertificationError>(depths)
                    },
                )?;
            let assignments = plan
                .demand_graph()
                .demands()
                .iter()
                .filter(|demand| TYPE_FACTS_FAMILIES.contains(&demand.family()))
                .map(|demand| {
                    let (artifact_case, export) = proof_artifact_export(demand.subject());
                    let subject = export_value_harness_subject(
                        project,
                        plan,
                        artifact_case,
                        export,
                        demand.id().as_str(),
                        &resolution_variants,
                        force_exact_subjects,
                    )
                    .expect("artifact case and declaration binding were checked while building the harness");
                    let (start, end) = locations
                        .get(&subject)
                        .expect("every scheduled subject has one harness expression");
                    Ok((
                        demand.id().as_str().to_owned(),
                        ExportValueDemand {
                            location: typefacts::Location {
                                path: project.harness.to_string_lossy().into_owned().into(),
                                start_byte: u64::try_from(*start).unwrap_or(u64::MAX),
                                end_byte: u64::try_from(*end).unwrap_or(u64::MAX),
                            },
                            implementation_location: export_implementation_location(
                                plans,
                                project,
                                plan,
                                export,
                                demand.id().as_str(),
                            )?,
                            callable_depth: *callable_depths
                                .get(&(artifact_case.to_owned(), export.to_owned()))
                                .expect("every scheduled export has an exact callable depth"),
                        },
                    ))
                })
                .collect::<Result<Vec<_>, TypeFactsCertificationError>>()?;
            let mut schedule = TypeFactsCertificationSchedule::new_export_values(
                plan.demand_graph(),
                assignments,
            )?;
            schedule.verifier_sources.push(TranscriptSourceDigest {
                path: project.harness.to_string_lossy().into_owned().into(),
                sha256: harness_digest.clone().into(),
            });
            Ok(schedule)
        })
        .collect()
}

fn append_exact_harness_import(
    harness: &mut String,
    index: usize,
    specifier: &str,
    declaration_export: &str,
) -> Result<String, TypeFactsCertificationError> {
    if !matches!(declaration_export, "default" | "*")
        && !is_ecmascript_identifier(declaration_export)
    {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: "export-value-schedule".into(),
            reason: format!(
                "declaration export name {declaration_export:?} cannot be imported by the exact harness"
            ),
        });
    }
    let local = format!("__solid_checker_export_{index}");
    let quoted = serde_json::to_string(specifier).map_err(|error| {
        TypeFactsCertificationError::ProducerProvenance(format!(
            "could not encode verifier harness specifier: {error}"
        ))
    })?;
    match declaration_export {
        "default" => harness.push_str(&format!("import {local} from {quoted};\n")),
        "*" => harness.push_str(&format!("import * as {local} from {quoted};\n")),
        _ => harness.push_str(&format!(
            "import {{ {declaration_export} as {local} }} from {quoted};\n"
        )),
    }
    Ok(local)
}

fn export_value_callable_depth(
    plan: &CertificationPlan,
    demand: &ProofDemand,
) -> Result<usize, TypeFactsCertificationError> {
    let (artifact_case, export_name) = proof_artifact_export(demand.subject());
    let export = plan
        .candidates()
        .proposal()
        .artifact_case(artifact_case)
        .and_then(|artifact| artifact.exports.get(export_name))
        .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
            demand: demand.id().as_str().to_owned(),
            reason: "scheduled export is absent from the normalized candidate".into(),
        })?;
    let depth = match demand.subject() {
        ProofDemandSubject::PositiveFact(PositiveFactSubject::CallbackBinding {
            ordinal,
            operation,
            ..
        }) => export
            .callbacks()
            .items()
            .get(usize::try_from(*ordinal).unwrap_or(usize::MAX))
            .filter(|callback| callback.operation.0 == *operation)
            .and_then(|callback| match &callback.from {
                ValueSource::Parameter { path, .. } => Some(path.len()),
                _ => None,
            })
            .unwrap_or(0),
        ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
            root,
            path,
            ..
        }) => {
            let prefix = match root {
                ValueRoot::Export | ValueRoot::OperationOutput { .. } => 0,
                ValueRoot::OperationInput { operation, index } => export
                    .operation(&operation.0)
                    .and_then(|operation| operation.inputs.get(usize::from(*index)))
                    .and_then(|input| parameter_source(input).ok())
                    .and_then(|source| match source {
                        ValueSource::Parameter { path, .. } => Some(path.len()),
                        _ => None,
                    })
                    .unwrap_or(0),
            };
            prefix.saturating_add(path.0.len())
        }
        _ => 0,
    };
    if depth > typefacts::MAX_INVOCATION_CALLABLE_DEPTH {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: demand.id().as_str().to_owned(),
            reason: format!(
                "required callable path depth {depth} exceeds the Type Facts limit {}",
                typefacts::MAX_INVOCATION_CALLABLE_DEPTH
            ),
        });
    }
    Ok(depth)
}

fn export_implementation_location(
    plans: &[&CertificationPlan],
    project: &PrivateTypeFactsProject,
    plan: &CertificationPlan,
    export: &str,
    demand: &str,
) -> Result<Option<typefacts::Location>, TypeFactsCertificationError> {
    let Some((runtime_path, _runtime_export, span, snapshot_root)) =
        plan.verified_exports.runtime_binding(export)
    else {
        return Ok(None);
    };
    let mut owners = plans
        .iter()
        .copied()
        .filter(|candidate| candidate.snapshot_root() == snapshot_root)
        .peekable();
    if owners.peek().is_none() {
        return Err(TypeFactsCertificationError::SubjectMismatch {
            demand: demand.to_owned(),
            reason: "runtime export binding belongs to an unplanned snapshot".into(),
        });
    }
    // `snapshot_root` is a content hash over the package name, version, and
    // every file's bytes, so every plan matching it materializes byte-identical
    // sources. The implementation location (path + span) therefore resolves to
    // the same bytes regardless of which matching owner is selected, and the
    // producer's transcript over that span is identical. A batch or graph that
    // legitimately carries the same installation as several plans — a package's
    // alternative artifact cases (all share one snapshot_root by construction),
    // or a dependency reached through multiple graph edges — is not an
    // ambiguity. Select the first matching owner the project actually
    // materialized; refusing on multiplicity was spurious.
    let package_root = owners
        .filter_map(|owner| project.package_root(owner).ok())
        .next()
        .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
            demand: demand.to_owned(),
            reason: "runtime export binding snapshot is not materialized in the project".into(),
        })?;
    let path = package_root.join(runtime_path);
    Ok(Some(typefacts::Location {
        path: path.to_string_lossy().into_owned().into(),
        start_byte: u64::from(span.start),
        end_byte: u64::from(span.end),
    }))
}

/// Test-only entry point: materialize the private witness project for `plans`
/// and return the package-relative program roots its written `tsconfig.json`
/// lists, harness excluded. Used by the sibling module's regression test that a
/// runtime module reachable only through a re-export becomes a program member.
#[cfg(test)]
pub(super) fn private_project_program_files_for_test(
    plans: &[&CertificationPlan],
    owner: &CertificationPlan,
) -> Result<Vec<String>, TypeFactsCertificationError> {
    let project = PrivateTypeFactsProject::materialize(owner, &[], plans, &[])?;
    let configuration: serde_json::Value =
        serde_json::from_slice(&fs::read(project.project_id())?).expect("written tsconfig is JSON");
    let package_root = project.package_root(owner)?.to_string_lossy().into_owned();
    Ok(configuration["files"]
        .as_array()
        .expect("tsconfig files array")
        .iter()
        .map(|value| value.as_str().expect("tsconfig file entry").to_owned())
        .filter_map(|path| {
            path.strip_prefix(&package_root)
                .map(|relative| relative.trim_start_matches('/').to_owned())
        })
        .collect())
}

/// Test-only entry point: materialize `owner`'s package and resolve the
/// implementation location for `export` against the full `plans` set. Used by
/// the sibling module's regression test that a value-only case set carrying
/// several plans of one snapshot_root (its alternative artifact cases) no
/// longer refuses as "multiple installation identities".
#[cfg(test)]
pub(super) fn export_implementation_location_for_test(
    plans: &[&CertificationPlan],
    owner: &CertificationPlan,
    export: &str,
) -> Result<Option<typefacts::Location>, TypeFactsCertificationError> {
    let project = PrivateTypeFactsProject::materialize(owner, &[], plans, &[])?;
    export_implementation_location(plans, &project, owner, export, "test-demand")
}

fn export_value_harness_subject(
    project: &PrivateTypeFactsProject,
    plan: &CertificationPlan,
    artifact_case: &str,
    export: &str,
    demand: &str,
    resolution_variants: &ExportResolutionVariants,
    force_exact_subjects: bool,
) -> Result<(String, String), TypeFactsCertificationError> {
    let public_specifier = public_export_harness_specifier(plan, artifact_case, demand)?;
    let public_subject = (public_specifier, export.to_owned());
    let publicly_addressable = project.package_root(plan)?
        == project
            .root
            .join("node_modules")
            .join(plan.snapshot.package_name());
    if (!force_exact_subjects || publicly_addressable)
        && resolution_variants
            .get(&public_subject)
            .is_some_and(|variants| variants.len() == 1)
    {
        return Ok(public_subject);
    }
    exact_declaration_harness_subject(project, plan, export, demand)
}

fn public_export_harness_specifier(
    plan: &CertificationPlan,
    artifact_case: &str,
    demand: &str,
) -> Result<String, TypeFactsCertificationError> {
    let case = plan
        .candidates
        .proposal()
        .artifact_case(artifact_case)
        .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
            demand: demand.to_owned(),
            reason: "demanded artifact case is absent from the candidate".into(),
        })?;
    Ok(if case.entrypoint == "." {
        plan.snapshot.package_name().to_owned()
    } else {
        format!(
            "{}/{}",
            plan.snapshot.package_name(),
            case.entrypoint.trim_start_matches("./")
        )
    })
}

fn exact_declaration_harness_subject(
    project: &PrivateTypeFactsProject,
    plan: &CertificationPlan,
    export: &str,
    demand: &str,
) -> Result<(String, String), TypeFactsCertificationError> {
    let (declaration_path, declaration_selector, _declaration_export) = plan
        .verified_exports
        .declaration_binding(export)
        .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
            demand: demand.to_owned(),
            reason: "demanded export has no snapshot-verified declaration binding".into(),
        })?;
    // Import the independently replayed declaration target, not the public
    // package specifier. A batch can contain multiple conditional artifact
    // cases for the same public subpath; asking TypeScript to resolve that
    // subpath once would silently bind every case to the host's one active
    // condition set. This relative verifier-owned path selects the immutable
    // snapshot file each opaque plan already authenticated.
    let specifier = snapshot_module_harness_specifier(project, plan, declaration_path)?;
    Ok((specifier, declaration_selector.to_owned()))
}

fn snapshot_module_harness_specifier(
    project: &PrivateTypeFactsProject,
    plan: &CertificationPlan,
    path: &str,
) -> Result<String, TypeFactsCertificationError> {
    let package_root = project.package_root(plan)?;
    let relative = package_root.strip_prefix(&project.root).map_err(|_| {
        let (expected, actual) = diagnostic_identity_path_pair(
            &project.root.to_string_lossy(),
            &package_root.to_string_lossy(),
        );
        TypeFactsCertificationError::identity_mismatch(
            "declaration harness",
            "package_root_prefix",
            expected,
            actual,
        )
    })?;
    Ok(format!(
        "./{}/{}",
        relative.to_string_lossy().replace('\\', "/"),
        declaration_import_path(path.trim_start_matches("./"))
    ))
}

fn declaration_import_path(path: &str) -> String {
    for (declaration, runtime) in [(".d.mts", ".mjs"), (".d.cts", ".cjs"), (".d.ts", ".js")] {
        if let Some(stem) = path.strip_suffix(declaration) {
            return format!("{stem}{runtime}");
        }
    }
    path.to_owned()
}

fn write_new_private_project_file(
    path: &Path,
    bytes: &[u8],
) -> Result<(), TypeFactsCertificationError> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    // See `write_immutable_project_file`: these bytes are consumed only by the
    // current live transaction and never become durable public authority.
    drop(file);
    Ok(())
}

fn is_ecmascript_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && characters.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
}

pub(super) fn verify_live_answer(
    plan: &CertificationPlan,
    schedule: &TypeFactsCertificationSchedule,
    live: &LiveInvocationAnswer,
) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
    verify_schedule_identity(
        "invocation verification",
        plan.demand_graph().root().as_str(),
        &schedule.demand_graph_root,
        "export_value_count",
        schedule.export_values.len(),
    )?;
    let identity = live.identity();
    let answer = live.answer();
    let context = identity.context();
    if context.snapshot_root() != plan.snapshot_root() {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "invocation verification",
            "snapshot_root",
            plan.snapshot_root().to_owned(),
            context.snapshot_root().to_owned(),
        ));
    }
    if context.demand_graph_root() != plan.demand_graph().root().as_str() {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "invocation verification",
            "context_demand_graph_root",
            plan.demand_graph().root().as_str().to_owned(),
            context.demand_graph_root().to_owned(),
        ));
    }
    if identity.generation() != answer.envelope.generation {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "invocation verification",
            "generation",
            identity.generation().to_string(),
            answer.envelope.generation.to_string(),
        ));
    }
    if identity.project_id() != &*answer.envelope.project_id {
        let (expected, actual) =
            diagnostic_identity_path_pair(identity.project_id(), &answer.envelope.project_id);
        return Err(TypeFactsCertificationError::identity_mismatch(
            "invocation verification",
            "project_id",
            expected,
            actual,
        ));
    }
    if identity.demand_sha256() != &*answer.envelope.demand_sha256 {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "invocation verification",
            "demand_sha256",
            identity.demand_sha256().to_owned(),
            answer.envelope.demand_sha256.to_string(),
        ));
    }
    if identity.handshake_protocol() != typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "invocation verification",
            "handshake_protocol",
            typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL.to_string(),
            identity.handshake_protocol().to_string(),
        ));
    }
    if identity.handshake_schema_sha256() != typefacts::v3::TYPE_FACTS_SCHEMA_SHA256 {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "invocation verification",
            "handshake_schema_sha256",
            typefacts::v3::TYPE_FACTS_SCHEMA_SHA256.to_owned(),
            identity.handshake_schema_sha256().to_owned(),
        ));
    }
    if identity.handshake_build() != typefacts::v3::TYPE_FACTS_BUILD_ID {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "invocation verification",
            "handshake_build",
            typefacts::v3::TYPE_FACTS_BUILD_ID.to_owned(),
            identity.handshake_build().to_owned(),
        ));
    }
    let mut expected_ids = schedule.proof_demand_ids().collect::<Vec<_>>();
    expected_ids.sort();
    let actual_ids = identity
        .context()
        .proof_demand_ids()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if expected_ids != actual_ids {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "invocation verification",
            "proof_demand_ids",
            format!("{expected_ids:?}"),
            format!("{actual_ids:?}"),
        ));
    }
    if answer.transcripts.len() != schedule.invocations.len() {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "invocation verification",
            "transcript_count",
            schedule.invocations.len().to_string(),
            answer.transcripts.len().to_string(),
        ));
    }
    if !answer.envelope.open_reasons.is_empty() {
        return Err(TypeFactsCertificationError::FamilyOpen {
            demand: "source-census".into(),
            reason: answer.envelope.open_reasons.join(","),
        });
    }
    let source_sites = verify_snapshot_source_census(
        plan,
        &[],
        &[],
        None,
        &answer.envelope.sources,
        &schedule.verifier_sources,
    )?;

    let mut bindings = Vec::with_capacity(expected_ids.len());
    for (index, scheduled) in schedule.invocations.iter().enumerate() {
        let transcript = &answer.transcripts[index];
        if transcript.location != scheduled.demand.location {
            let (expected, actual) = diagnostic_location_pair(
                Some(&scheduled.demand.location),
                Some(&transcript.location),
            );
            return Err(TypeFactsCertificationError::identity_mismatch(
                "invocation verification",
                "transcript_location",
                expected,
                actual,
            ));
        }
        let transcript_bytes = typefacts::encode(transcript)?;
        let transcript_root = format!("sha256:{:x}", Sha256::digest(&transcript_bytes));
        for proof in &scheduled.proof_demands {
            verify_subject_signature(plan, proof, transcript)?;
            let mut sites = verify_family(proof, transcript)?;
            sites.extend(source_sites.iter().cloned());
            sites.sort();
            sites.dedup();
            let evidence_root = super::certification_evidence_root(
                proof_family_name(proof.family),
                [
                    identity.evidence_root(),
                    proof.id.as_str(),
                    transcript_root.as_str(),
                    plan.snapshot_root(),
                    plan.demand_graph().root().as_str(),
                ],
            );
            bindings.push(WitnessBinding::new(
                witness_variant(proof.family),
                proof.id.clone(),
                evidence_root,
                sites,
            ));
        }
    }
    Ok(VerifiedTypeFactsEvidence {
        bindings,
        session_evidence_root: identity.evidence_root().to_owned(),
    })
}

fn validate_export_envelope_open_reasons(
    open_reasons: &[std::sync::Arc<str>],
) -> Result<(), TypeFactsCertificationError> {
    // Export-value transcripts carry their own type/path completeness. An
    // unresolved module elsewhere in the TypeScript program is not evidence
    // against a locally closed root or path: if it taints the demanded value,
    // the producer emits `openType`, Unknown callability, or an open path and
    // the family verifier below refuses it. Keep the program-level marker in
    // the authenticated answer, while refusing every other envelope defect.
    // This distinction is required by declaration surfaces such as
    // @solidjs/html whose exact callable export is locally declared but whose
    // unused call-result type names an external JSX module.
    if open_reasons
        .iter()
        .all(|reason| reason.as_ref() == "unresolvedModule")
    {
        return Ok(());
    }
    Err(TypeFactsCertificationError::FamilyOpen {
        demand: "source-census".into(),
        reason: open_reasons.join(","),
    })
}

pub(super) fn verify_live_export_value_answer(
    plan: &CertificationPlan,
    schedule: &TypeFactsCertificationSchedule,
    live: &LiveExportValueAnswer,
) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
    verify_live_export_value_answer_with_project_census(plan, schedule, live, &[], &[], &[], None)
}

fn verify_live_export_value_answer_with_dependencies(
    plan: &CertificationPlan,
    schedule: &TypeFactsCertificationSchedule,
    live: &LiveExportValueAnswer,
    dependencies: &[&CertificationPlan],
    sources: &[super::dependencies::VerifiedGraphSourcePackage],
    project: &PrivateTypeFactsProject,
) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
    verify_live_export_value_answer_with_project_census(
        plan,
        schedule,
        live,
        dependencies,
        dependencies,
        sources,
        Some(project),
    )
}

fn verify_live_export_value_answer_with_project_census(
    plan: &CertificationPlan,
    schedule: &TypeFactsCertificationSchedule,
    live: &LiveExportValueAnswer,
    dependencies: &[&CertificationPlan],
    census_dependencies: &[&CertificationPlan],
    census_sources: &[super::dependencies::VerifiedGraphSourcePackage],
    project: Option<&PrivateTypeFactsProject>,
) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
    verify_schedule_identity(
        "export-value verification",
        plan.demand_graph().root().as_str(),
        &schedule.demand_graph_root,
        "invocation_count",
        schedule.invocations.len(),
    )?;
    let identity = live.identity();
    let answer = live.answer();
    let context = identity.context();
    if context.snapshot_root() != plan.snapshot_root() {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value verification",
            "snapshot_root",
            plan.snapshot_root().to_owned(),
            context.snapshot_root().to_owned(),
        ));
    }
    if context.demand_graph_root() != plan.demand_graph().root().as_str() {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value verification",
            "context_demand_graph_root",
            plan.demand_graph().root().as_str().to_owned(),
            context.demand_graph_root().to_owned(),
        ));
    }
    if identity.generation() != answer.envelope.generation {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value verification",
            "generation",
            identity.generation().to_string(),
            answer.envelope.generation.to_string(),
        ));
    }
    if identity.project_id() != &*answer.envelope.project_id {
        let (expected, actual) =
            diagnostic_identity_path_pair(identity.project_id(), &answer.envelope.project_id);
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value verification",
            "project_id",
            expected,
            actual,
        ));
    }
    if identity.demand_sha256() != &*answer.envelope.demand_sha256 {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value verification",
            "demand_sha256",
            identity.demand_sha256().to_owned(),
            answer.envelope.demand_sha256.to_string(),
        ));
    }
    if identity.handshake_protocol() != typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value verification",
            "handshake_protocol",
            typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL.to_string(),
            identity.handshake_protocol().to_string(),
        ));
    }
    if identity.handshake_schema_sha256() != typefacts::v3::TYPE_FACTS_SCHEMA_SHA256 {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value verification",
            "handshake_schema_sha256",
            typefacts::v3::TYPE_FACTS_SCHEMA_SHA256.to_owned(),
            identity.handshake_schema_sha256().to_owned(),
        ));
    }
    if identity.handshake_build() != typefacts::v3::TYPE_FACTS_BUILD_ID {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value verification",
            "handshake_build",
            typefacts::v3::TYPE_FACTS_BUILD_ID.to_owned(),
            identity.handshake_build().to_owned(),
        ));
    }
    let mut expected_ids = schedule.proof_demand_ids().collect::<Vec<_>>();
    expected_ids.sort();
    let actual_ids = identity
        .context()
        .proof_demand_ids()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if expected_ids != actual_ids {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value verification",
            "proof_demand_ids",
            format!("{expected_ids:?}"),
            format!("{actual_ids:?}"),
        ));
    }
    if answer.transcripts.len() != schedule.export_values.len() {
        return Err(TypeFactsCertificationError::identity_mismatch(
            "export-value verification",
            "transcript_count",
            schedule.export_values.len().to_string(),
            answer.transcripts.len().to_string(),
        ));
    }
    validate_export_envelope_open_reasons(&answer.envelope.open_reasons)?;
    let source_sites = verify_snapshot_source_census(
        plan,
        census_dependencies,
        census_sources,
        project,
        &answer.envelope.sources,
        &schedule.verifier_sources,
    )?;
    let certification_sources_root = plan.certification_sources_root();
    let mut bindings = Vec::with_capacity(expected_ids.len());
    for (index, scheduled) in schedule.export_values.iter().enumerate() {
        let transcript = &answer.transcripts[index];
        if transcript.location != scheduled.demand.location {
            let (expected, actual) = diagnostic_location_pair(
                Some(&scheduled.demand.location),
                Some(&transcript.location),
            );
            return Err(TypeFactsCertificationError::identity_mismatch(
                "export-value verification",
                "transcript_location",
                expected,
                actual,
            ));
        }
        verify_implementation_location_identity(
            scheduled.demand.implementation_location.as_ref(),
            transcript
                .implementation
                .as_ref()
                .map(|implementation| &implementation.location),
        )?;
        let transcript_bytes = typefacts::encode(transcript)?;
        let transcript_root = format!("sha256:{:x}", Sha256::digest(&transcript_bytes));
        for proof in &scheduled.proof_demands {
            verify_export_value_subject(plan, proof, transcript, dependencies)?;
            let mut sites = verify_export_value_family(plan, proof, transcript)?;
            sites.extend(source_sites.iter().cloned());
            sites.sort();
            sites.dedup();
            let evidence_root = super::certification_evidence_root(
                proof_family_name(proof.family),
                [
                    identity.evidence_root(),
                    proof.id.as_str(),
                    transcript_root.as_str(),
                    plan.snapshot_root(),
                    plan.demand_graph().root().as_str(),
                    // Which declaration-only closure proved this. Without it the
                    // verdict would depend on an input the receipt does not
                    // name, and a full-closure certification would be
                    // indistinguishable from a partial-closure one.
                    certification_sources_root.as_str(),
                ],
            );
            bindings.push(WitnessBinding::new(
                witness_variant(proof.family),
                proof.id.clone(),
                evidence_root,
                sites,
            ));
        }
    }
    Ok(VerifiedTypeFactsEvidence {
        bindings,
        session_evidence_root: identity.evidence_root().to_owned(),
    })
}

fn verify_subject_signature(
    plan: &CertificationPlan,
    proof: &ScheduledProofDemand,
    transcript: &InvocationTranscript,
) -> Result<(), TypeFactsCertificationError> {
    let (artifact_case, export) = proof_artifact_export(&proof.subject);
    if plan
        .candidates
        .proposal()
        .artifact_case(artifact_case)
        .and_then(|case| case.exports.get(export))
        .is_none()
    {
        return Err(TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "demanded export is absent from the selected candidate".into(),
        });
    }
    let (declaration_path, _declaration_selector, declaration_export) = plan
        .verified_exports
        .declaration_binding(export)
        .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "demanded export has no snapshot-verified declaration binding".into(),
        })?;
    let signature = transcript.selected_signature.as_ref().ok_or_else(|| {
        TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "transcript has no selected signature".into(),
        }
    })?;
    let marker = format!(
        "/node_modules/{}/{}",
        plan.snapshot.package_name(),
        declaration_path
    )
    .replace('\\', "/");
    let actual_path = signature.declaration.location.path.replace('\\', "/");
    if !actual_path.ends_with(&marker) {
        return Err(TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "selected signature is not in the snapshot-verified export declaration".into(),
        });
    }
    verify_declaration_export_identity(proof, declaration_export, signature)
}

fn verify_export_value_subject(
    plan: &CertificationPlan,
    proof: &ScheduledProofDemand,
    transcript: &ExportValueTranscript,
    dependencies: &[&CertificationPlan],
) -> Result<(), TypeFactsCertificationError> {
    let (artifact_case, export) = proof_artifact_export(&proof.subject);
    if plan
        .candidates
        .proposal()
        .artifact_case(artifact_case)
        .and_then(|case| case.exports.get(export))
        .is_none()
    {
        return Err(TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "demanded export is absent from the selected candidate".into(),
        });
    }
    if !transcript.complete
        || transcript.target.is_empty()
        || transcript.query_name.is_empty()
        || !transcript.open_reasons.is_empty()
    {
        let mut causes = Vec::new();
        if !transcript.complete {
            causes.push("transcriptIncomplete".to_owned());
        }
        if transcript.target.is_empty() {
            causes.push("targetMissing".to_owned());
        }
        if transcript.query_name.is_empty() {
            causes.push("queryNameMissing".to_owned());
        }
        causes.extend(
            transcript
                .open_reasons
                .iter()
                .map(|reason| format!("producer:{reason}")),
        );
        return Err(TypeFactsCertificationError::FamilyOpen {
            demand: proof.id.clone(),
            reason: format!(
                "export expression, alias target, or declaration identity is open for artifact case {artifact_case:?} export {export:?} ({})",
                causes.join(",")
            ),
        });
    }
    let declaration = transcript.declaration.as_ref().ok_or_else(|| {
        TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "export-value transcript has no compiler-resolved declaration".into(),
        }
    })?;
    let (declaration_path, _declaration_selector, declaration_export) = plan
        .verified_exports
        .declaration_binding(export)
        .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "demanded export has no snapshot-verified declaration binding".into(),
        })?;
    let marker = format!(
        "/node_modules/{}/{}",
        plan.snapshot.package_name(),
        declaration_path.trim_start_matches("./")
    )
    .replace('\\', "/");
    let actual_path = declaration.location.path.replace('\\', "/");
    let actual_name = if declaration.name.is_empty() {
        declaration
            .qualified_name
            .rsplit('.')
            .next()
            .unwrap_or_default()
    } else {
        &declaration.name
    };
    if !actual_path.ends_with(&marker)
        && !authenticated_dependency_declaration_target(
            plan,
            dependencies,
            actual_name,
            &actual_path,
        )
    {
        return Err(TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: format!(
                "resolved value declaration {actual_name:?} at {actual_path:?} is not the snapshot-selected export declaration suffix {marker:?}"
            ),
        });
    }
    if !actual_path.ends_with(&marker) {
        return Ok(());
    }
    verify_snapshot_declaration_name(&proof.id, declaration_export, actual_name, &actual_path)
}

fn verify_snapshot_declaration_name(
    demand: &str,
    declaration_export: &str,
    actual_name: &str,
    actual_path: &str,
) -> Result<(), TypeFactsCertificationError> {
    if declaration_export == "*" {
        if namespace_declaration_name_matches_path(actual_name, actual_path) {
            return Ok(());
        }
        return Err(TypeFactsCertificationError::SubjectMismatch {
            demand: demand.into(),
            reason: "resolved namespace declaration identity disagrees with its replayed module"
                .into(),
        });
    }
    if declaration_export == "default" {
        // The verifier-authored harness contains an exact default import for
        // this package/subpath, and its bytes are part of the source census.
        // `target` is the compiler-canonicalized alias target, while the
        // declaration path above is independently replayed from the archive.
        // The target's display name is intentionally irrelevant to the export
        // alias, but an anonymous/unidentified declaration is not authority.
        if actual_name.is_empty() {
            return Err(TypeFactsCertificationError::SubjectMismatch {
                demand: demand.into(),
                reason: "canonical default-export target has no declaration identity".into(),
            });
        }
        return Ok(());
    }
    if actual_name != declaration_export {
        return Err(TypeFactsCertificationError::SubjectMismatch {
            demand: demand.into(),
            reason: "resolved value declaration name disagrees with snapshot export replay".into(),
        });
    }
    Ok(())
}

fn namespace_declaration_name_matches_path(name: &str, path: &str) -> bool {
    let Some(name) = name
        .strip_prefix('"')
        .and_then(|name| name.strip_suffix('"'))
    else {
        return false;
    };
    let normalized_name = name.replace('\\', "/");
    let normalized_path = path.replace('\\', "/");
    [
        ".d.mts", ".d.cts", ".d.ts", ".mts", ".cts", ".tsx", ".ts", ".mjs", ".cjs", ".jsx", ".js",
    ]
    .iter()
    .find_map(|suffix| normalized_path.strip_suffix(suffix))
    .is_some_and(|stem| stem == normalized_name)
}

fn authenticated_dependency_declaration_target(
    parent: &CertificationPlan,
    dependencies: &[&CertificationPlan],
    declaration_name: &str,
    declaration_path: &str,
) -> bool {
    dependencies.iter().any(|dependency| {
        let marker = private_project_package_marker(
            parent,
            &dependency.resolved_import.package_root,
            dependency.snapshot.package_name(),
        );
        dependency.snapshot.files.keys().any(|path| {
            declaration_path.ends_with(&format!("{}{}", marker, path.trim_start_matches("./")))
                && dependency
                    .verified_exports
                    .has_declaration_target(path, declaration_name)
        })
    })
}

fn verify_export_value_family(
    plan: &CertificationPlan,
    proof: &ScheduledProofDemand,
    transcript: &ExportValueTranscript,
) -> Result<Vec<String>, TypeFactsCertificationError> {
    let (artifact_case, export_name) = proof_artifact_export(&proof.subject);
    let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
        demand: proof.id.clone(),
        reason: format!(
            "{} ({artifact_case}:{export_name}): {reason}",
            proof_family_name(proof.family)
        ),
    };
    let mut sites = vec![format!(
        "export-value:{}:{}:{}",
        transcript.location.path, transcript.location.start_byte, transcript.location.end_byte
    )];
    match proof.family {
        ProofFamily::SelectedSignature | ProofFamily::RestSpreadCoverage => {
            for signature in require_export_call_signatures(proof, transcript, &open)? {
                sites.push(format!(
                    "export-signature:{}:overload:{}/{}:rest:{}",
                    signature.identity,
                    signature.overload_ordinal,
                    signature.overload_count,
                    signature.has_rest
                ));
            }
        }
        ProofFamily::ArgumentBinding => {
            let (export, implementation) =
                require_export_implementation(plan, proof, transcript, &open)?;
            let source = callback_parameter_source(export, proof)?;
            let floor = callback_reachability_floor(export, proof);
            require_parameter_flow(implementation, &source, floor, &open, &mut sites)?;
        }
        ProofFamily::CallablePath => match &proof.subject {
            ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                root: ValueRoot::Export,
                ..
            }) => require_export_recursive_subject(proof, transcript, &open, &mut sites)?,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue { .. }) => {
                require_operation_recursive_subject(plan, proof, transcript, &open, &mut sites)?
            }
            ProofDemandSubject::PositiveFact(PositiveFactSubject::CallbackBinding { .. }) => {
                let (export, implementation) =
                    require_export_implementation(plan, proof, transcript, &open)?;
                let source = callback_parameter_source(export, proof)?;
                let floor = callback_reachability_floor(export, proof);
                // Every overload has to declare the parameter callable at this
                // path. One overload that does is not the export's promise.
                //
                // The universal check writes into a buffer, not into the
                // witness: it can fail partway, and the sites of the overloads
                // it did clear are not evidence for the weaker proof the
                // fallback then builds. They are committed only when every
                // overload passed.
                let mut typed_sites = Vec::new();
                let typed = require_export_call_signatures(proof, transcript, &open).and_then(
                    |signatures| {
                        signatures.iter().try_for_each(|signature| {
                            require_signature_parameter_callable(
                                signature,
                                &source,
                                &open,
                                &mut typed_sites,
                            )
                        })
                    },
                );
                if typed.is_ok() {
                    sites.append(&mut typed_sites);
                    require_parameter_flow(implementation, &source, floor, &open, &mut sites)?;
                } else {
                    require_parameter_callback_flow(
                        implementation,
                        &source,
                        floor,
                        &open,
                        &mut sites,
                    )?;
                }
            }
            _ => {
                return Err(TypeFactsCertificationError::UnsupportedDemand {
                    demand: proof.id.clone(),
                    reason: "callable-path demand has no exact exported value or callback binding"
                        .into(),
                });
            }
        },
        ProofFamily::OperationReachability => {
            let (export, implementation) =
                require_export_implementation(plan, proof, transcript, &open)?;
            let operation = proof_operation(export, proof)?;
            require_operation_evidence(
                export,
                operation,
                proof,
                implementation,
                &open,
                &mut sites,
            )?;
        }
        ProofFamily::OperationCardinality => {
            let (export, implementation) =
                require_export_implementation(plan, proof, transcript, &open)?;
            let operation = proof_operation(export, proof)?;
            if operation.cardinality.scope != Some(CardinalityScope::Call)
                || operation.cardinality.min != Some(0)
                || operation.cardinality.max != Some(UpperBound::Many)
            {
                return Err(TypeFactsCertificationError::UnsupportedDemand {
                    demand: proof.id.clone(),
                    reason:
                        "runtime implementation census cannot prove a tighter operation cardinality"
                            .into(),
                });
            }
            require_operation_evidence(
                export,
                operation,
                proof,
                implementation,
                &open,
                &mut sites,
            )?;
            sites.push("operation-cardinality:per-call:0..many".into());
        }
        ProofFamily::RecursiveValueShape => {
            if matches!(
                &proof.subject,
                ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                    root: ValueRoot::Export,
                    ..
                })
            ) {
                require_export_recursive_subject(proof, transcript, &open, &mut sites)?;
            } else {
                require_operation_recursive_subject(plan, proof, transcript, &open, &mut sites)?;
            }
        }
        ProofFamily::DomainExhaustiveness => {
            require_closed_value(&transcript.value, &open)?;
            require_export_callable_paths_closed(transcript, &open)?;
            sites.push("typefacts-export-value-domain:complete".into());
        }
        _ => {
            return Err(TypeFactsCertificationError::UnsupportedDemand {
                demand: proof.id.clone(),
                reason: "this proof family requires an invocation or operation transcript".into(),
            });
        }
    }
    Ok(sites)
}

/// The complete set of call signatures a demand about this exported callable
/// must hold for.
///
/// An overload set has no single signature, and demanding "the" one of a
/// two-overload export — `@solid-primitives/cookies` `createServerCookie`,
/// `@corvu/utils`'s default — asks for an object that does not exist. The sound
/// generalization is universal, not existential: a premise that holds for every
/// overload holds for the export, whichever one a caller selects. Callers must
/// therefore require their premise of *all* returned signatures, never the
/// first.
///
/// The producer reports an overload set all-or-nothing, but "the producer
/// promises" is not a premise a verifier may lean on: the completeness of the
/// set is checked here from the set itself. Every signature has to agree that
/// the declared overload count is the number of signatures present, and the
/// ordinals have to be exactly `0..len` with no repeat — the only shape in which
/// no declared overload is missing. The two fields are also mutually exclusive,
/// so a transcript populating both is refused rather than silently answered from
/// one of them.
fn require_export_call_signatures<'a>(
    proof: &ScheduledProofDemand,
    transcript: &'a ExportValueTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<&'a [typefacts::SelectedSignature], TypeFactsCertificationError> {
    if transcript.call_signature.is_some() && !transcript.call_signatures.is_empty() {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "exported callable reports both a single signature and an overload set".into(),
        });
    }
    if let Some(signature) = transcript.call_signature.as_ref() {
        if signature.overload_count != 1 || signature.overload_ordinal != 0 {
            return Err(TypeFactsCertificationError::UnsupportedDemand {
                demand: proof.id.clone(),
                reason: "exported callable proof requires one exact overload".into(),
            });
        }
        return Ok(std::slice::from_ref(signature));
    }
    if transcript.call_signatures.is_empty() {
        return Err(open("exported callable has no compiler signature"));
    }
    require_complete_overload_set(&transcript.call_signatures, open)?;
    Ok(&transcript.call_signatures)
}

/// Refuses an overload set that is not provably the whole declared set.
fn require_complete_overload_set(
    signatures: &[typefacts::SelectedSignature],
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    let mut ordinals = vec![false; signatures.len()];
    for signature in signatures {
        if signature.overload_count != signatures.len()
            || signature.overload_ordinal >= ordinals.len()
            || ordinals[signature.overload_ordinal]
        {
            return Err(open("overload set is not the complete declared set"));
        }
        ordinals[signature.overload_ordinal] = true;
    }
    Ok(())
}

fn require_export_implementation<'a>(
    plan: &'a CertificationPlan,
    proof: &ScheduledProofDemand,
    transcript: &'a ExportValueTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<
    (
        &'a solid_reactive_ir::contract_semantics::ExportSemantics,
        &'a typefacts::ExportImplementationTranscript,
    ),
    TypeFactsCertificationError,
> {
    let (artifact_case, export_name) = proof_artifact_export(&proof.subject);
    let export = plan
        .candidates
        .proposal()
        .artifact_case(artifact_case)
        .and_then(|case| case.exports.get(export_name))
        .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "demanded implementation export is absent from the candidate".into(),
        })?;
    let implementation = transcript
        .implementation
        .as_ref()
        .ok_or_else(|| open("runtime implementation transcript is absent"))?;
    let control_flow_only_open = !implementation.complete
        && !implementation.open_reasons.is_empty()
        && implementation
            .open_reasons
            .iter()
            .all(|reason| reason.as_ref() == "controlFlowUnsupported");
    if (!implementation.complete && !control_flow_only_open)
        || implementation.target.is_empty()
        || implementation.query_name.is_empty()
        || implementation
            .open_reasons
            .iter()
            .any(|reason| reason.as_ref() != "controlFlowUnsupported")
        || implementation.signature.is_none()
        || implementation.declaration.is_none()
        || implementation.control_flow.is_none()
    {
        return Err(open(&format!(
            "runtime implementation transcript is incomplete or open (reasons={:?})",
            implementation.open_reasons
        )));
    }
    let (runtime_path, runtime_export, _, _) =
        plan.verified_exports
            .runtime_binding(export_name)
            .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
                demand: proof.id.clone(),
                reason: "demanded export has no exact identifier runtime binding".into(),
            })?;
    let declaration = implementation
        .declaration
        .as_ref()
        .expect("implementation closure checked the declaration");
    let actual_path = declaration.location.path.replace('\\', "/");
    let expected_suffix = format!("/{}", runtime_path.trim_start_matches("./"));
    if implementation.query_name.as_ref() != runtime_export
        || !actual_path.ends_with(&expected_suffix)
    {
        return Err(TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "runtime implementation does not match the snapshot-replayed export binding"
                .into(),
        });
    }
    Ok((export, implementation))
}

fn callback_parameter_source(
    export: &solid_reactive_ir::contract_semantics::ExportSemantics,
    proof: &ScheduledProofDemand,
) -> Result<ValueSource, TypeFactsCertificationError> {
    let ProofDemandSubject::PositiveFact(PositiveFactSubject::CallbackBinding {
        ordinal,
        operation,
        ..
    }) = &proof.subject
    else {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "callback binding family has no callback subject".into(),
        });
    };
    let callback = export
        .callbacks()
        .items()
        .get(usize::try_from(*ordinal).unwrap_or(usize::MAX))
        .filter(|callback| callback.operation.0 == *operation)
        .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "callback ordinal and normalized operation disagree".into(),
        })?;
    Ok(callback.from.clone())
}

fn proof_operation<'a>(
    export: &'a solid_reactive_ir::contract_semantics::ExportSemantics,
    proof: &ScheduledProofDemand,
) -> Result<&'a solid_reactive_ir::contract_semantics::Operation, TypeFactsCertificationError> {
    let ProofDemandSubject::PositiveFact(PositiveFactSubject::Operation { operation, .. }) =
        &proof.subject
    else {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "operation family has no exact operation subject".into(),
        });
    };
    export
        .operation(operation)
        .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "normalized operation is absent from the selected export".into(),
        })
}

fn parameter_source(value: &ValueShape) -> Result<ValueSource, TypeFactsCertificationError> {
    match value {
        ValueShape::Parameter { index, path } => Ok(ValueSource::Parameter {
            index: *index,
            path: path.clone(),
        }),
        _ => Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: "operation-input".into(),
            reason: "implementation census only binds exact parameter-rooted operation inputs"
                .into(),
        }),
    }
}

fn parameter_value_source_matches(
    actual: &typefacts::ParameterValueSource,
    expected: &ValueSource,
) -> bool {
    parameter_binding_matches(actual.parameter_index, &actual.path, expected)
}

/// Whether an observed parameter-rooted path answers the demanded one.
///
/// The observed path may be *longer*: reading `props.of.keys` is reading
/// `props`, and a use of the destructured `{ children }` is a use of the object
/// it was destructured from. It may never be shorter, and every segment the
/// demand names has to be that exact property — a tuple or computed segment
/// where a property is demanded answers nothing.
fn parameter_binding_matches(
    index: usize,
    path: &[typefacts::PathSegment],
    expected: &ValueSource,
) -> bool {
    let ValueSource::Parameter {
        index: expected_index,
        path: expected_path,
    } = expected
    else {
        return false;
    };
    index == usize::from(*expected_index)
        && path.len() >= expected_path.len()
        && path.iter().zip(expected_path).all(|(actual, expected)| {
            actual.kind == PathSegmentKind::Property && actual.property.as_ref() == expected
        })
}

fn parameter_value_source_exact(
    actual: &typefacts::ParameterValueSource,
    expected: &ValueSource,
) -> bool {
    let ValueSource::Parameter { path, .. } = expected else {
        return false;
    };
    actual.path.len() == path.len() && parameter_value_source_matches(actual, expected)
}

/// How much execution evidence the operation a demand is bound to requires of a
/// call.
///
/// The two floors answer different claims. An operation that asserts an
/// occurrence — `min >= 1` — is witnessed only by a call the implementation
/// provably reaches. An operation that asserts no lower bound (`min: 0`, or no
/// cardinality at all) claims exactly "this may happen": a call in a `for … of`
/// or `do … while` body, which the producer reports as `Unknown` because the
/// body may run zero times, is that claim's own shape and discharges it.
///
/// `Reachability::Unreachable` clears neither floor. Code after a `return` or a
/// `throw` never runs, so it witnesses nothing for any claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReachabilityFloor {
    /// Only a call the implementation provably reaches counts.
    Reachable,
    /// A call the implementation may reach counts as well, because the bound
    /// operation asserts no lower bound.
    MayExecute,
}

impl ReachabilityFloor {
    fn admits(self, reach: Reachability) -> bool {
        match self {
            Self::Reachable => reach == Reachability::Reachable,
            Self::MayExecute => reach_admits_zero_lower_bound(reach),
        }
    }
}

/// Whether this reachability is adequate evidence for a claim whose lower bound
/// is zero. Deliberately spelled out rather than folded into the enum: the
/// premise is that "may execute" is what a zero lower bound asserts, and
/// `Unreachable` is not "may execute".
fn reach_admits_zero_lower_bound(reach: Reachability) -> bool {
    matches!(reach, Reachability::Reachable | Reachability::Unknown)
}

/// The floor an operation sets for the evidence that witnesses it.
///
/// Relaxing the floor takes **positive evidence that the operation asserts no
/// lower bound**, which is an explicit `min: Some(0)` and nothing else. An
/// operation carrying no cardinality at all has said nothing about how often it
/// happens, and silence is not a claim of "zero or more": `has_cardinality` is
/// false for exactly that operation, and the demand builder then requests
/// `operation-reachability` for it *without* an `operation-cardinality` demand,
/// so no bound was ever stated for this floor to read. It keeps the strict
/// floor.
///
/// This is deliberately narrower than `Cardinality::strength`, which folds the
/// absent case in with `min: 0` because it is answering a different question —
/// "may this be assumed guaranteed", where absence and zero agree. Here absence
/// and zero disagree: one is a claim, the other is its lack.
fn operation_reachability_floor(
    operation: &solid_reactive_ir::contract_semantics::Operation,
) -> ReachabilityFloor {
    if operation.cardinality.min == Some(0) {
        return ReachabilityFloor::MayExecute;
    }
    ReachabilityFloor::Reachable
}

/// The floor the operation this callback binding invokes sets.
///
/// Fails closed to the strict floor: a callback whose operation the export does
/// not carry has no bound whose lower end could be read, and an unread bound is
/// never a relaxation.
fn callback_reachability_floor(
    export: &solid_reactive_ir::contract_semantics::ExportSemantics,
    proof: &ScheduledProofDemand,
) -> ReachabilityFloor {
    let ProofDemandSubject::PositiveFact(PositiveFactSubject::CallbackBinding {
        ordinal,
        operation,
        ..
    }) = &proof.subject
    else {
        return ReachabilityFloor::Reachable;
    };
    export
        .callbacks()
        .items()
        .get(usize::try_from(*ordinal).unwrap_or(usize::MAX))
        .filter(|callback| callback.operation.0 == *operation)
        .and_then(|callback| export.operation(&callback.operation.0))
        .map_or(ReachabilityFloor::Reachable, operation_reachability_floor)
}

fn require_parameter_flow(
    implementation: &typefacts::ExportImplementationTranscript,
    source: &ValueSource,
    floor: ReachabilityFloor,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let matching = implementation.calls.iter().filter(|call| {
        is_call_expression(call)
            && implementation_call_is_executed(implementation, call, floor)
            && (call
                .callee_parameter
                .as_ref()
                .is_some_and(|actual| parameter_value_source_matches(actual, source))
                || call.argument_parameters.iter().flatten().any(|actual| {
                    parameter_value_source_matches(actual, source) && !call.target.is_empty()
                }))
    });
    let mut found = false;
    for call in matching {
        found = true;
        sites.push(format!(
            "implementation-flow:{}:{}:{}:{}",
            call.location.path, call.location.start_byte, call.location.end_byte, call.target
        ));
    }
    if !found {
        return Err(open(
            "callback parameter has no exact direct-call or resolved-argument flow",
        ));
    }
    Ok(())
}

fn require_parameter_callback_flow(
    implementation: &typefacts::ExportImplementationTranscript,
    source: &ValueSource,
    floor: ReachabilityFloor,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    for call in &implementation.calls {
        if !is_call_expression(call)
            || !implementation_call_is_executed(implementation, call, floor)
        {
            continue;
        }
        if call
            .callee_parameter
            .as_ref()
            .is_some_and(|actual| parameter_value_source_exact(actual, source))
        {
            sites.push(format!(
                "implementation-direct-callback:{}:{}:{}",
                call.location.path, call.location.start_byte, call.location.end_byte
            ));
            return Ok(());
        }
        // The callee's own body calls the parameter at this slot, and every hop
        // that got it there is a plain identifier forward. That is the same
        // claim an in-body direct call makes, one resolved call away:
        // `createIntervalCounter` forwards its `timeout` to `createPolled`,
        // which forwards it to `createTimer`, whose body calls `delay()`.
        //
        // `callee_strongly_invoked_parameters` and not the weak
        // `callee_invoked_parameters`, deliberately. The weak fact is satisfied
        // by a chain that ends at `addEventListener`, which proves the value
        // runs but says nothing about whether the callee treats the position as
        // a function — and accepting it here would quietly turn this family
        // into `require_parameter_flow`.
        //
        // The pending form is the same claim with its last premise still open.
        // `createTimer` calls `delay()` inside the closure it hands to
        // `createEffect`, so whether that call runs depends on whether
        // `createEffect`'s slot 0 is a callback position — a dialect fact the
        // producer may not decide and this side owns. Only the *strong* pending
        // claims are read here, on the same reasoning: composing through an
        // invoking position changes where the terminal call may sit, never what
        // the chain proves.
        for (argument, actual) in call.argument_parameters.iter().enumerate() {
            if actual
                .as_ref()
                .is_some_and(|actual| parameter_value_source_exact(actual, source))
                && (call.callee_strongly_invoked_parameters.contains(&argument)
                    || callee_pending_invocation_holds(call, argument, true))
            {
                sites.push(format!(
                    "implementation-callee-direct-callback:{}:{}:{}:{}:{}",
                    call.location.path,
                    call.location.start_byte,
                    call.location.end_byte,
                    call.target,
                    argument
                ));
                return Ok(());
            }
        }
        if call.target.is_empty() || call.target_module.as_ref() != "solid-js" {
            continue;
        }
        for (argument, actual) in call.argument_parameters.iter().enumerate() {
            if actual
                .as_ref()
                .is_some_and(|actual| parameter_value_source_exact(actual, source))
                && solid_dialect::unambiguous_callback_argument(
                    &call.target_name,
                    argument,
                    call.argument_parameters.len(),
                )
            {
                sites.push(format!(
                    "implementation-dialect-callback:{}:{}:{}:{}:{}",
                    call.location.path,
                    call.location.start_byte,
                    call.location.end_byte,
                    call.target_module,
                    call.target_name
                ));
                return Ok(());
            }
        }
    }
    Err(open(
        "callback parameter has neither an exact direct call nor an exact dialect callback flow",
    ))
}

/// Whether invoking the export can execute this call.
///
/// A call the implementation makes directly is executed by the call itself. A
/// call *inside a nested callable* is executed only if something invokes that
/// very callable — the one immediately containing it, named by the producer as
/// `enclosingCallable`. Two things can invoke it:
///
///   - the export's caller, when a reachable return site carries that exact
///     callable;
///   - an already-executed call of this same implementation, when it hands that
///     exact callable to a slot proven to be invoked. That premise is what
///     `@solid-primitives/autofocus` needs, whose whole body is
///     `createEffect(() => { const el = ref(); … })` and which returns nothing
///     at all: the closure runs because `createEffect` runs it, not because
///     anything is handed back.
///
/// What "proven to be invoked" may mean is deliberately narrow and is spelled
/// out in [`argument_slot_is_proven_invoking`]. Nothing here infers an invoking
/// position from a callee's name, from the shape of the argument, or from a
/// package being well known.
///
/// **The premise composes; it never nests.** A returned debounced function
/// schedules its callback two callables deep —
/// `return (…) => { setTimeout(() => callback(…), wait) }` — and that call is
/// executed here, but only because two facts meet: the returned closure is
/// carried by the return site, and the arrow immediately containing
/// `callback(…)` is carried at slot 0 of a `setTimeout` whose own enclosing
/// callable is that returned closure. Each link is proven separately, and the
/// recursion is what joins them.
///
/// Byte containment alone would be unsound, and this is the shape that proves
/// it: in
///
/// ```js
/// createEffect(() => {
///   const inner = () => { callback(); };
///   registry.push(inner);
/// });
/// ```
///
/// the `callback()` site lies inside the range `createEffect` invokes, and yet
/// `callback` never runs — the effect only stores `inner`. Containment cannot
/// tell that apart from the debounce shape; requiring the *immediately*
/// enclosing callable to be carried, and every callable above it to be proven
/// in turn, can. A callable that is merely defined, stored, pushed, or assigned
/// breaks the chain, and the demand stays open.
/// Whether this census entry is a call expression rather than a construction.
///
/// Both kinds are in the census, because both run the callables they are handed
/// and the execution premise must be able to compose through either. They are
/// not interchangeable anywhere else: every claim whose witness says the
/// implementation *calls* a value — the invoke flow, the callback flow, the
/// parameter-read evidence, the owner-primitive call, the recursive-parameter
/// call — asks this first. `new Cls(cb)` is a different claim about `cb` and
/// none of those families was reviewed for it.
///
/// An unknown kind answers false. A producer that did not state the kind stated
/// no call. A producer that stated a kind this side does not recognize never
/// reaches here at all: `CallKind` has no catch-all arm, so an unrecognized
/// spelling fails deserialization and the whole transcript is refused.
fn is_call_expression(call: &typefacts::ImplementationCall) -> bool {
    call.kind == typefacts::CallKind::Call
}

/// Whether a conditional callee-parameter claim about `argument` holds, with
/// every premise it defers answered here.
///
/// The producer knows the syntax and refuses to guess the semantics: it reports
/// that the callee calls its parameter from inside a callable handed to slot
/// *s* of `module::name`, and this side decides whether that slot is a callback
/// position — the same dialect gate `argument_slot_is_proven_invoking`'s first
/// tier applies, on the same table, so a package that is not `solid-js` and a
/// slot no dialect agrees about are both refused.
///
/// A claim with no premises proves nothing. The unconditional claims travel in
/// the index lists; an empty requirement list here is a malformed fact, not a
/// fact that needs nothing.
fn callee_pending_invocation_holds(
    call: &typefacts::ImplementationCall,
    argument: usize,
    strong: bool,
) -> bool {
    call.callee_pending_invocations.iter().any(|pending| {
        pending.parameter == argument
            && (!strong || pending.strong)
            && !pending.requires.is_empty()
            && pending.requires.iter().all(|premise| {
                premise.module.as_ref() == "solid-js"
                    && solid_dialect::unambiguous_callback_argument(
                        &premise.name,
                        premise.slot,
                        premise.argument_count,
                    )
            })
    })
}

/// Whether invoking the export reaches this call site.
///
/// `floor` carries what the demand's own bound operation asks of the call
/// site's reachability; see [`ReachabilityFloor`]. It applies to every *call*
/// link of the chain, because the chain states one claim and the claim's
/// strength is the demand's.
///
/// The **return site** that carries a captured call is a separate question and
/// stays strict: a callable only some paths return is not a callable invoking
/// the export reaches, whatever the demanded operation's lower bound is.
fn implementation_call_is_executed(
    implementation: &typefacts::ExportImplementationTranscript,
    call: &typefacts::ImplementationCall,
    floor: ReachabilityFloor,
) -> bool {
    let mut budget = MAX_EXECUTION_PREMISE_NODES;
    implementation_call_is_executed_within(implementation, call, floor, 0, &mut budget)
}

/// Bounds on the invoking-position recursion. A closure inside a closure inside
/// an argument is real, so the walk is transitive; both bounds match the
/// producer's carried-callable descent, and exceeding either answers "not
/// executed" rather than guessing.
const MAX_EXECUTION_PREMISE_DEPTH: usize = 8;
const MAX_EXECUTION_PREMISE_NODES: usize = 256;

fn implementation_call_is_executed_within(
    implementation: &typefacts::ExportImplementationTranscript,
    call: &typefacts::ImplementationCall,
    floor: ReachabilityFloor,
    depth: usize,
    budget: &mut usize,
) -> bool {
    if depth > MAX_EXECUTION_PREMISE_DEPTH || *budget == 0 {
        return false;
    }
    *budget -= 1;
    if !floor.admits(call.reach) {
        return false;
    }
    if !call.captured {
        return true;
    }
    // A captured call whose enclosing callable the producer did not name is a
    // call whose chain cannot be built. Absence is not evidence either way, so
    // the premise refuses.
    let Some(enclosing) = call.enclosing_callable.as_ref() else {
        return false;
    };
    // The call and its immediately enclosing callable are one execution
    // question, not two composition links. Argument/return edges below consume
    // depth; charging this handoff too would halve the pre-existing bound.
    callable_is_executed_within(implementation, enclosing, floor, depth, budget)
}

/// Whether the export invocation can execute one exact nested callable.
///
/// This is the shared node of the execution graph. A callable can enter the
/// graph from the implementation's own return, from an invoking argument slot,
/// or from a reachable return of another callable already in the graph. The
/// last edge is deliberately keyed by exact callable locations on both sides:
/// lexical containment and a merely declared/stored closure carry no weight.
fn callable_is_executed_within(
    implementation: &typefacts::ExportImplementationTranscript,
    callable: &typefacts::Location,
    floor: ReachabilityFloor,
    depth: usize,
    budget: &mut usize,
) -> bool {
    if depth > MAX_EXECUTION_PREMISE_DEPTH || *budget == 0 {
        return false;
    }
    *budget -= 1;
    let carried_by_implementation_return =
        implementation.control_flow.as_ref().is_some_and(|flow| {
            flow.returns
                .iter()
                .filter(|site| site.reach == Reachability::Reachable)
                .filter(|site| site.carry_reach.is_some_and(|reach| floor.admits(reach)))
                .flat_map(|site| site.carried_callables.iter())
                .any(|carried| carried == callable)
        });
    if carried_by_implementation_return {
        return true;
    }
    let carried_by_callable_return = implementation.callable_returns.iter().any(|census| {
        census.returns.iter().any(|site| {
            site.reach == Reachability::Reachable
                && site.carry_reach.is_some_and(|reach| floor.admits(reach))
                && site
                    .carried_callables
                    .iter()
                    .any(|carried| floor.admits(carried.reach) && &carried.location == callable)
                && callable_is_executed_within(
                    implementation,
                    &census.callable,
                    floor,
                    depth + 1,
                    budget,
                )
        })
    });
    if carried_by_callable_return {
        return true;
    }
    implementation.calls.iter().any(|outer| {
        outer
            .argument_callables
            .iter()
            .filter(|carried| {
                carried
                    .locations
                    .iter()
                    .any(|location| location == callable)
            })
            .any(|carried| argument_slot_is_proven_invoking(outer, carried.argument))
            // Every link of the chain answers to the *same* floor, because the
            // chain carries one claim: "invoking the export can reach this
            // call". Under a zero lower bound an outer link that may run is
            // consistent with a callback that may be invoked; under a lower
            // bound of one every link must be reachable, which is exactly the
            // strict premise the argument route was built with. Holding the
            // outer links strict while relaxing the subject was measured and
            // costs `@solid-primitives/marker@0.2.2` the frontier the
            // reachability-floor slice measured for it, with nothing gained.
            && implementation_call_is_executed_within(
                implementation,
                outer,
                floor,
                depth + 1,
                budget,
            )
    })
}

/// Whether argument slot `argument` of `call` is proven to invoke whatever
/// callable it carries.
///
/// Three premises, each an exact fact and none of them a name match:
///
///   - a dialect primitive. The callee resolves to an exact `solid-js` import
///     and every dialect that canonically owns that name agrees the slot is a
///     callback. This is the same gate `require_parameter_callback_flow`
///     already applies, so `createEffect(fn, initialValue)` at slot 1 stays
///     refused and a locally shadowed `createMemo` — whose `target_module` is
///     not `solid-js` — never reaches it.
///   - a reviewed default-library member. The producer resolved the callee by
///     default-library symbol identity, and the verifier re-checks both the
///     member's name against its own closed enum and the slot against its own
///     table. An unrecognized invoker string is refused rather than trusted,
///     and a slot the verifier's table does not list is refused even when the
///     transmitted list carries it.
///   - the callee's own body. The producer proved the resolved local callee
///     sends its parameter at that slot to an invoking position. The verifier
///     consumes that fact and never re-derives semantics from a location.
///
/// An external package's helper satisfies none of the three, and that is the
/// intended answer: `@solid-primitives/until` hands its condition to
/// `createBranch` from `@solid-primitives/rootless`, and nothing in this
/// artifact's transcript can prove what that function does. It needs an
/// accepted dependency contract, not a well-known-name list.
///
/// # Strength: this is a *can execute* premise, deliberately
///
/// Every tier here asserts that invoking the export *can* reach the call, not
/// that it always does or that it does exactly once. `p.then(onOk, onErr)`
/// lists both slots though at most one handler ever runs and neither runs if
/// the promise never settles; `items.forEach(cb)` invokes nothing when `items`
/// is empty, and `.some` / `.find` / `.sort` short-circuit. That matches what
/// the fact is for: the demands it feeds are `Invoke` operations whose
/// inventory records a zero-or-more callback position, so a consumer may read
/// "this argument is a callback this implementation runs" and may not read "it
/// is run at least once". Nothing downstream may derive a lower bound from it,
/// and the tier tables must stay membership-reviewed rather than grown by
/// analogy.
fn argument_slot_is_proven_invoking(call: &typefacts::ImplementationCall, argument: usize) -> bool {
    if !call.target.is_empty()
        && call.target_module.as_ref() == "solid-js"
        && solid_dialect::unambiguous_callback_argument(
            &call.target_name,
            argument,
            call.argument_parameters.len(),
        )
    {
        return true;
    }
    let default_library_invokes = typefacts::DefaultLibraryInvoker::from_wire(
        &call.default_library_invoker,
    )
    .is_some_and(|invoker| invoker.invokes(argument) && call.invoked_arguments.contains(&argument));
    default_library_invokes
        || call.callee_invoked_parameters.contains(&argument)
        || callee_pending_invocation_holds(call, argument, false)
}

fn require_signature_parameter_callable(
    signature: &typefacts::SelectedSignature,
    source: &ValueSource,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let ValueSource::Parameter { index, path } = source else {
        return Err(open(
            "callback source is not rooted in an exported parameter",
        ));
    };
    let parameter = signature
        .parameters
        .get(usize::from(*index))
        .ok_or_else(|| open("callback source names a missing exported parameter"))?;
    if path.is_empty() {
        if parameter.value.callability == Callability::Unknown
            && parameter.declared_type.as_ref().is_some_and(|declared| {
                solid_dialect::unambiguous_callable_type(&declared.module, &declared.name)
            })
        {
            let declared = parameter
                .declared_type
                .as_ref()
                .expect("declared callable type checked above");
            sites.push(format!(
                "callback-parameter:{}:declared-type:{}:{}",
                index, declared.module, declared.name
            ));
            return Ok(());
        }
        require_root_callability(
            &parameter.value,
            DemandedCallability::Callable,
            "callback parameter",
            open,
        )?;
        sites.push(format!("callback-parameter:{}:root", index));
        return Ok(());
    }
    let matches = parameter.callable_paths.iter().filter(|fact| {
        fact.path.len() == path.len()
            && fact.path.iter().zip(path).all(|(actual, expected)| {
                actual.kind == PathSegmentKind::Property && actual.property.as_ref() == expected
            })
    });
    let mut found = false;
    for fact in matches {
        found = true;
        if !callable_path_is_present_and_locally_closed(fact)
            || fact.callability != Callability::Callable
        {
            return Err(open("callback parameter path is not closed and callable"));
        }
        sites.push(callable_path_site(fact));
    }
    if !found {
        return Err(open(
            "callback parameter path is absent from the exact signature census",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn require_operation_evidence(
    export: &solid_reactive_ir::contract_semantics::ExportSemantics,
    operation: &solid_reactive_ir::contract_semantics::Operation,
    proof: &ScheduledProofDemand,
    implementation: &typefacts::ExportImplementationTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let floor = operation_reachability_floor(operation);
    match operation.kind {
        OperationKind::Invoke => {
            let callback = export
                .callbacks()
                .items()
                .iter()
                .find(|callback| callback.operation == operation.id)
                .ok_or_else(|| open("invoke operation has no exact callback source"))?;
            require_parameter_flow(implementation, &callback.from, floor, open, sites)
        }
        OperationKind::Read => {
            let source = operation
                .inputs
                .first()
                .ok_or_else(|| open("read operation has no input"))
                .and_then(parameter_source)?;
            require_parameter_read_evidence(implementation, &source, floor, open, sites)
        }
        OperationKind::Return => {
            let flow = implementation
                .control_flow
                .as_ref()
                .ok_or_else(|| open("implementation control-flow census is absent"))?;
            let reachable = flow
                .returns
                .iter()
                .filter(|site| site.reach == Reachability::Reachable)
                .collect::<Vec<_>>();
            if reachable.is_empty() {
                return Err(open(
                    "return operation has no reachable implementation return",
                ));
            }
            sites.extend(reachable.into_iter().map(|site| {
                format!(
                    "implementation-return:{}:{}:{}",
                    site.location.path, site.location.start_byte, site.location.end_byte
                )
            }));
            Ok(())
        }
        OperationKind::Create => {
            require_owner_operation_call(operation, proof, implementation, floor, open, sites)
        }
        _ => Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "runtime implementation census does not yet bind this operation kind".into(),
        }),
    }?;
    Ok(())
}

/// Evidence that the exact parameter value is the *callee* of a call the
/// implementation itself makes — one of the two witnesses a `Read` operation
/// accepts.
///
/// The kind gate is load-bearing rather than decorative even though today's
/// producer states no `calleeParameter` for a construction: this side does not
/// certify against a producer's habits, and `new cb()` is a different claim
/// about `cb` than `cb()` in every one of the reviewed families. The captured
/// gate is the same discipline about *where* the call sits: a call written
/// inside a nested callable is a call something else runs.
///
/// `floor` is the demand's own bound; see [`ReachabilityFloor`]. A call in a
/// loop body witnesses an operation whose lower bound is zero and nothing else.
fn require_parameter_read_call(
    implementation: &typefacts::ExportImplementationTranscript,
    source: &ValueSource,
    floor: ReachabilityFloor,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let mut found = false;
    for call in &implementation.calls {
        if is_call_expression(call)
            && floor.admits(call.reach)
            && !call.captured
            && call
                .callee_parameter
                .as_ref()
                .is_some_and(|actual| parameter_value_source_matches(actual, source))
        {
            found = true;
            sites.push(format!(
                "implementation-read:{}:{}:{}",
                call.location.path, call.location.start_byte, call.location.end_byte
            ));
        }
    }
    if !found {
        return Err(open(
            "parameter-rooted read has no exact implementation call",
        ));
    }
    Ok(())
}

/// Implementation evidence that the export reads the demanded parameter-rooted
/// value.
///
/// Two witnesses, because reading is not calling. A call whose callee is the
/// value is one — `props.of.keys()` reads `props`; that half is
/// [`require_parameter_read_call`]. The other is the use census's own record of
/// the read: an uncaptured `propertyAccess`, `directCall`, or `aliasCall` rooted
/// at that parameter, which is what `const mapFn = props.children` leaves behind
/// and what the call census, by construction, never sees.
///
/// The kinds this refuses are the ones that are not reads. `storage` and
/// `return` hand the value somewhere without looking into it; `argumentKnown`,
/// `argumentUnknown`, and `capture` do the same through a call or a closure;
/// `unknownEscape` is the census saying it could not classify the use at all,
/// which is never evidence of anything. A captured use is refused for the reason
/// the call census refuses a captured call: nothing here proves the closure
/// holding it runs.
///
/// Both witnesses answer to the same `floor`, and to the same reachability fact:
/// the producer answers a use's position from the very walk that answers a
/// call's. A `props.children` after a `return`, after a `throw`, or in a branch a
/// literal condition excludes is `Unreachable` and witnesses nothing for any
/// claim; a use in a loop body is `Unknown` and witnesses only an operation whose
/// own lower bound is zero. Exempting the use loop from the floor is precisely
/// how a `min >= 1` demand came to be discharged from dead code.
fn require_parameter_read_evidence(
    implementation: &typefacts::ExportImplementationTranscript,
    source: &ValueSource,
    floor: ReachabilityFloor,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let mut found = require_parameter_read_call(implementation, source, floor, open, sites).is_ok();
    for use_site in &implementation.parameter_uses {
        if !floor.admits(use_site.reach)
            || use_site.captured
            || !matches!(
                use_site.kind,
                ParameterUseKind::PropertyAccess
                    | ParameterUseKind::DirectCall
                    | ParameterUseKind::AliasCall
            )
            || !parameter_binding_matches(use_site.parameter_index, &use_site.binding_path, source)
        {
            continue;
        }
        found = true;
        sites.push(format!(
            "implementation-read-use:{}:{}:{}:{}",
            use_site.location.path,
            use_site.location.start_byte,
            use_site.location.end_byte,
            parameter_use_kind_site(use_site.kind)
        ));
    }
    if !found {
        return Err(open(
            "parameter-rooted read has no exact implementation call or use",
        ));
    }
    Ok(())
}

/// The stable spelling of a use kind inside a witness site. Written out rather
/// than derived from `Debug`, which is not a wire format.
const fn parameter_use_kind_site(kind: ParameterUseKind) -> &'static str {
    match kind {
        ParameterUseKind::DirectCall => "directCall",
        ParameterUseKind::AliasCall => "aliasCall",
        ParameterUseKind::ArgumentKnown => "argumentKnown",
        ParameterUseKind::ArgumentUnknown => "argumentUnknown",
        ParameterUseKind::PropertyAccess => "propertyAccess",
        ParameterUseKind::Return => "return",
        ParameterUseKind::Storage => "storage",
        ParameterUseKind::Capture => "capture",
        ParameterUseKind::UnknownEscape => "unknownEscape",
    }
}

fn require_owner_operation_call(
    operation: &solid_reactive_ir::contract_semantics::Operation,
    proof: &ScheduledProofDemand,
    implementation: &typefacts::ExportImplementationTranscript,
    floor: ReachabilityFloor,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let expected = if operation.owner.requirements.cleanup == Requirement::Required {
        solid_dialect::OwnerRequirementRole::Cleanup
    } else if operation.owner.requirements.child_owners == Requirement::Required {
        solid_dialect::OwnerRequirementRole::Effect
    } else {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "create operation has no supported owner requirement".into(),
        });
    };
    let mut found = false;
    for call in &implementation.calls {
        if !is_call_expression(call)
            || !floor.admits(call.reach)
            || call.captured
            || call.target.is_empty()
            || call.target_module.as_ref() != "solid-js"
        {
            continue;
        }
        let name = if !call.target_name.is_empty() {
            call.target_name.as_ref()
        } else if let Some(declaration) = &call.declaration {
            if declaration.name.is_empty() {
                declaration
                    .qualified_name
                    .rsplit('.')
                    .next()
                    .unwrap_or_default()
            } else {
                declaration.name.as_ref()
            }
        } else {
            continue;
        };
        let Some(actual) = solid_dialect::unambiguous_owner_requirement_role(name) else {
            continue;
        };
        if actual == expected {
            found = true;
            sites.push(format!(
                "implementation-owner-call:{}:{}:{}:{name}",
                call.location.path, call.location.start_byte, call.location.end_byte
            ));
        }
    }
    if !found {
        let observed = implementation
            .calls
            .iter()
            .filter(|call| is_call_expression(call))
            .filter_map(|call| {
                if !call.target_name.is_empty() {
                    Some(call.target_name.as_ref())
                } else if let Some(declaration) = &call.declaration {
                    if declaration.name.is_empty() {
                        Some(
                            declaration
                                .qualified_name
                                .rsplit('.')
                                .next()
                                .unwrap_or_default(),
                        )
                    } else {
                        Some(declaration.name.as_ref())
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        return Err(open(&format!(
            "owner requirement has no exact dialect primitive call; observed {observed:?}"
        )));
    }
    Ok(())
}

/// The reachable call whose *callee* is exactly this parameter value, if the
/// implementation makes one.
///
/// Same discipline as [`require_parameter_read_call`], for the witness
/// `recursive-operation-parameter:`: the kind gate keeps a construction of the
/// parameter from answering a claim about calling it, and the captured gate
/// keeps a call written inside a nested callable from answering a claim about
/// what *this* body does.
fn recursive_parameter_call_site<'a>(
    implementation: &'a typefacts::ExportImplementationTranscript,
    source: &ValueSource,
) -> Option<&'a typefacts::ImplementationCall> {
    implementation.calls.iter().find(|call| {
        is_call_expression(call)
            && call.reach == Reachability::Reachable
            && !call.captured
            && !call.target.is_empty()
            && call
                .callee_parameter
                .as_ref()
                .is_some_and(|actual| parameter_value_source_exact(actual, source))
    })
}

/// Which of the two implementation-evidence arms a recursive value-shape demand
/// rooted at an operation input takes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecursiveValueEvidence {
    /// The demand names the input's root and asserts nothing about its
    /// callability. A reachable call whose callee *is* that exact parameter
    /// value answers it.
    RootUnasserted,
    /// The demand names an exact property path and asserts the value there is
    /// callable. The callback-flow census answers it.
    ShapeAsserted,
}

/// Implementation evidence for a recursive value-shape demand rooted at an
/// operation input, split from the plan lookup so the floor it imposes is a
/// decision a test can hold rather than a literal in the middle of a lookup.
///
/// Returns `Ok(true)` when the branch discharged the demand, `Ok(false)` when
/// the evidence is absent but the signature census may still answer, and `Err`
/// when the branch owns the answer and the evidence is not there.
///
/// **Both arms keep the strict floor, deliberately.** A recursive value-shape
/// demand asserts what a position *is*, not how often the implementation
/// reaches it, so the zero-lower-bound reading that relaxes the occurrence
/// families has nothing to stand on here: a call in a loop body proves the
/// parameter is callable no more than it proves the loop runs.
fn operation_input_value_shape_evidence(
    source: &ValueSource,
    evidence: RecursiveValueEvidence,
    implementation: &typefacts::ExportImplementationTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<bool, TypeFactsCertificationError> {
    match evidence {
        RecursiveValueEvidence::RootUnasserted => {
            let Some(call) = recursive_parameter_call_site(implementation, source) else {
                return Ok(false);
            };
            sites.push(format!(
                "recursive-operation-parameter:{}:{}:{}:{}",
                call.location.path, call.location.start_byte, call.location.end_byte, call.target
            ));
            Ok(true)
        }
        RecursiveValueEvidence::ShapeAsserted => {
            require_parameter_callback_flow(
                implementation,
                source,
                ReachabilityFloor::Reachable,
                open,
                sites,
            )?;
            Ok(true)
        }
    }
}

fn require_operation_recursive_subject(
    plan: &CertificationPlan,
    proof: &ScheduledProofDemand,
    transcript: &ExportValueTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
        artifact_case,
        export,
        root,
        path,
        callable,
    }) = &proof.subject
    else {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "recursive operation family has no recursive subject".into(),
        });
    };
    let exported = plan
        .candidates
        .proposal()
        .artifact_case(artifact_case)
        .and_then(|case| case.exports.get(export))
        .ok_or_else(|| open("recursive operation export is absent"))?;
    if let ValueRoot::OperationInput { operation, index } = root {
        let operation = exported
            .operation(&operation.0)
            .ok_or_else(|| open("recursive input operation is absent"))?;
        let mut source = operation
            .inputs
            .get(usize::from(*index))
            .ok_or_else(|| open("recursive input index is absent"))
            .and_then(parameter_source)?;
        let path_is_exact_properties = if let ValueSource::Parameter {
            path: source_path, ..
        } = &mut source
        {
            path.0.iter().all(|segment| match segment {
                ValuePathSegment::ObjectProperty(property) => {
                    source_path.push(property.clone());
                    true
                }
                ValuePathSegment::ChoiceAlternative(_) => true,
                _ => false,
            })
        } else {
            false
        };
        // A root path appends nothing above, so `source` is the same value the
        // unasserted arm has always read.
        let evidence = if path.0.is_empty() && !callable.asserts_callable() {
            Some(RecursiveValueEvidence::RootUnasserted)
        } else if path_is_exact_properties && callable.asserts_callable() {
            Some(RecursiveValueEvidence::ShapeAsserted)
        } else {
            None
        };
        if let Some(evidence) = evidence {
            let (_, implementation) = require_export_implementation(plan, proof, transcript, open)?;
            if operation_input_value_shape_evidence(&source, evidence, implementation, open, sites)?
            {
                return Ok(());
            }
        }
    }
    // An overloaded export is proved by proving every overload. Nothing here
    // may short-circuit on the first: the demand is about the export, and a
    // caller may select any of them.
    for signature in require_export_call_signatures(proof, transcript, open)? {
        require_operation_recursive_signature(
            proof, transcript, exported, root, path, *callable, signature, open, sites,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn require_operation_recursive_signature(
    proof: &ScheduledProofDemand,
    transcript: &ExportValueTranscript,
    exported: &solid_reactive_ir::contract_semantics::ExportSemantics,
    root: &ValueRoot,
    path: &solid_reactive_ir::contract_semantics::ValuePath,
    callable: DemandedCallability,
    signature: &typefacts::SelectedSignature,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let (value, callable_paths, prefix) = match root {
        ValueRoot::OperationInput { operation, index } => {
            let operation = exported
                .operation(&operation.0)
                .ok_or_else(|| open("recursive input operation is absent"))?;
            let source = operation
                .inputs
                .get(usize::from(*index))
                .ok_or_else(|| open("recursive input index is absent"))
                .and_then(parameter_source)?;
            let ValueSource::Parameter { index, path } = source else {
                return Err(open("recursive input is not parameter-rooted"));
            };
            let parameter = signature
                .parameters
                .get(usize::from(index))
                .ok_or_else(|| open("recursive input parameter is absent"))?;
            (parameter.value.clone(), &parameter.callable_paths, path)
        }
        ValueRoot::OperationOutput { operation } => {
            let operation = exported
                .operation(&operation.0)
                .ok_or_else(|| open("recursive output operation is absent"))?;
            if operation.kind != OperationKind::Return || operation.output.is_none() {
                return Err(open(
                    "recursive output is not the exported return operation",
                ));
            }
            (
                signature.result.clone(),
                &signature.result_callable_paths,
                Vec::new(),
            )
        }
        ValueRoot::Export => unreachable!("export roots are handled before this helper"),
    };
    let mut expected = prefix
        .into_iter()
        .map(|property| typefacts::PathSegment {
            kind: PathSegmentKind::Property,
            property: property.into(),
            index: None,
        })
        .collect::<Vec<_>>();
    let (alternative, suffix) = translate_value_path(&path.0).ok_or_else(|| {
        TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "Type Facts cannot address this operation value path exactly".into(),
        }
    })?;
    expected.extend(suffix);
    if expected.is_empty() {
        require_verifiable_root_premise(&value, callable, "operation value", open)?;
        require_root_callability(&value, callable, "operation value root", open)?;
        sites.push("recursive-operation-value:root".into());
        return Ok(());
    }
    let fact = callable_paths
        .iter()
        .find(|fact| fact.alternative == alternative && fact.path == expected)
        .ok_or_else(|| {
            open(&format!(
                "operation value path is absent from the signature census (alternative={alternative}, path={expected:?})"
            ))
        })?;
    let callable_local_closure = callable.asserts_callable()
        && fact.presence == PathPresence::Required
        && fact.callability == Callability::Callable
        && fact
            .open_reasons
            .iter()
            .all(|reason| reason.as_ref() == "openType");
    if callable.asserts_callable()
        && alternative == 0
        && require_return_callable_source(transcript, &expected, sites)
    {
        return Ok(());
    }
    if !callable_path_is_present_and_locally_closed(fact) && !callable_local_closure {
        return Err(open(&format!(
            "operation value path is locally open (complete={}, presence={:?}, callability={:?}, reasons={:?})",
            fact.complete, fact.presence, fact.callability, fact.open_reasons
        )));
    }
    require_path_callability(callable, fact, "operation value path", open)?;
    sites.push(callable_path_site(fact));
    Ok(())
}

fn require_return_callable_source(
    transcript: &ExportValueTranscript,
    expected: &[typefacts::PathSegment],
    sites: &mut Vec<String>,
) -> bool {
    let Some(flow) = transcript
        .implementation
        .as_ref()
        .and_then(|implementation| implementation.control_flow.as_ref())
    else {
        return false;
    };
    if flow
        .returns
        .iter()
        .any(|site| site.reach == Reachability::Unknown)
    {
        return false;
    }
    let mut evidence = Vec::new();
    for site in flow
        .returns
        .iter()
        .filter(|site| site.reach == Reachability::Reachable)
    {
        let source = site.sources.iter().find(|source| {
            source.path == expected
                && match source.kind {
                    typefacts::ImplementationValueSourceKind::DirectCallable => true,
                    typefacts::ImplementationValueSourceKind::CallResult => {
                        source.target_module.as_ref() == "solid-js"
                            && !source.target.is_empty()
                            && source.target_path.len() == 1
                            && source.target_path[0].kind == PathSegmentKind::Tuple
                            && source.target_path[0].index.is_some_and(|index| {
                                solid_dialect::unambiguous_callable_result_tuple_item(
                                    &source.target_name,
                                    index,
                                )
                            })
                    }
                }
        });
        let Some(source) = source else {
            return false;
        };
        evidence.push(format!(
            "implementation-return-source:{}:{}:{}:{}:{}",
            site.location.path,
            site.location.start_byte,
            site.location.end_byte,
            source.target_module,
            source.target_name
        ));
    }
    if evidence.is_empty() {
        return false;
    }
    sites.extend(evidence);
    true
}

fn callable_path_has_closed_local_observation(path: &typefacts::CallablePathFact) -> bool {
    path.complete && path.open_reasons.is_empty() && path.presence != PathPresence::Unknown
}

fn callable_path_is_present_and_locally_closed(path: &typefacts::CallablePathFact) -> bool {
    callable_path_has_closed_local_observation(path)
        && matches!(
            path.presence,
            PathPresence::Required | PathPresence::Optional
        )
}

fn callable_path_census_is_closed(path: &typefacts::CallablePathFact) -> bool {
    callable_path_has_closed_local_observation(path) && path.subtree_enumerated
}

fn require_export_callable_paths_closed(
    transcript: &ExportValueTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    for path in &transcript.callable_paths {
        if !callable_path_census_is_closed(path) {
            return Err(open("export value contains an open callable path"));
        }
    }
    Ok(())
}

fn require_export_recursive_subject(
    proof: &ScheduledProofDemand,
    transcript: &ExportValueTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
        root: ValueRoot::Export,
        path,
        callable,
        ..
    }) = &proof.subject
    else {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "export-value transcript was assigned to a non-export recursive subject".into(),
        });
    };
    if path.0.is_empty() {
        require_verifiable_root_premise(&transcript.value, *callable, "exported value", open)?;
        require_root_callability(&transcript.value, *callable, "export root", open)?;
        sites.push("recursive-export-value:root".into());
        return Ok(());
    }
    let (alternative, expected_path) = translate_value_path(&path.0).ok_or_else(|| {
        TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "Type Facts cannot address this exported-value path exactly".into(),
        }
    })?;
    let fact = transcript
        .callable_paths
        .iter()
        .find(|fact| fact.alternative == alternative && fact.path == expected_path)
        .ok_or_else(|| open("exported-value path is absent from the exact producer census"))?;
    if !callable_path_is_present_and_locally_closed(fact) {
        return Err(open("exported-value path is locally open"));
    }
    require_path_callability(*callable, fact, "exported-value path", open)?;
    sites.push(callable_path_site(fact));
    Ok(())
}

fn verify_declaration_export_identity(
    proof: &ScheduledProofDemand,
    declaration_export: &str,
    signature: &typefacts::SelectedSignature,
) -> Result<(), TypeFactsCertificationError> {
    if declaration_export == "*" {
        let actual_name = if signature.declaration.name.is_empty() {
            signature
                .declaration
                .qualified_name
                .rsplit('.')
                .next()
                .unwrap_or_default()
        } else {
            &signature.declaration.name
        };
        return verify_snapshot_declaration_name(
            &proof.id,
            declaration_export,
            actual_name,
            &signature.declaration.location.path,
        );
    }
    if declaration_export == "default" {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "Type Facts display names cannot authenticate a default-export alias".into(),
        });
    }
    let actual_name = if signature.declaration.name.is_empty() {
        signature
            .declaration
            .qualified_name
            .rsplit('.')
            .next()
            .unwrap_or_default()
    } else {
        &signature.declaration.name
    };
    if actual_name != declaration_export {
        return Err(TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "selected signature name is not the snapshot-verified export declaration"
                .into(),
        });
    }
    Ok(())
}

fn proof_artifact_export(subject: &ProofDemandSubject) -> (&str, &str) {
    match subject {
        ProofDemandSubject::DomainClosure { subject, .. } => {
            (&subject.artifact_case, &subject.export)
        }
        ProofDemandSubject::PositiveFact(positive) => match positive {
            PositiveFactSubject::SelectedCall {
                artifact_case,
                export,
            }
            | PositiveFactSubject::CallbackBinding {
                artifact_case,
                export,
                ..
            }
            | PositiveFactSubject::Operation {
                artifact_case,
                export,
                ..
            }
            | PositiveFactSubject::OperationEdge {
                artifact_case,
                export,
                ..
            }
            | PositiveFactSubject::Resource {
                artifact_case,
                export,
                ..
            }
            | PositiveFactSubject::GuardCase {
                artifact_case,
                export,
                ..
            }
            | PositiveFactSubject::RecursiveValue {
                artifact_case,
                export,
                ..
            } => (artifact_case, export),
        },
        ProofDemandSubject::ArtifactCase(_)
        | ProofDemandSubject::DependencyArtifact { .. }
        | ProofDemandSubject::DependencyClosure { .. } => {
            unreachable!("artifact and dependency demands are not Type Facts")
        }
    }
}

fn verify_snapshot_source_census(
    plan: &CertificationPlan,
    dependencies: &[&CertificationPlan],
    graph_sources: &[super::dependencies::VerifiedGraphSourcePackage],
    project: Option<&PrivateTypeFactsProject>,
    sources: &[typefacts::TranscriptSourceDigest],
    verifier_sources: &[typefacts::TranscriptSourceDigest],
) -> Result<Vec<String>, TypeFactsCertificationError> {
    use crate::contract_interface::ClosureFileRole;

    let package_marker = format!(
        "/node_modules/{}/",
        plan.snapshot.package_name().replace('\\', "/")
    );
    // Every site below names its file by the path it occupies *inside the
    // private project*, never by the absolute path the producer reported. The
    // absolute path is a temporary directory keyed on this process's pid and a
    // counter, so hashing it made every evidence root — and therefore every
    // receipt witness root — unique per run and impossible to compare across
    // certifications of the same bytes. The project-relative path plus the
    // content digest is what the site is actually asserting.
    let mut sites = Vec::new();
    for expected in verifier_sources {
        let matches = sources
            .iter()
            .filter(|source| source.path == expected.path)
            .collect::<Vec<_>>();
        if matches.len() != 1 || matches[0].sha256 != expected.sha256 {
            return Err(TypeFactsCertificationError::SourceCensus(format!(
                "verifier query source {} is absent, duplicated, or stale",
                expected.path
            )));
        }
        // The verifier query harness is written directly at the project root,
        // so its file name is its project-relative path.
        let harness = expected
            .path
            .replace('\\', "/")
            .rsplit('/')
            .next()
            .unwrap_or(&expected.path)
            .to_owned();
        sites.push(format!(
            "typefacts-verifier-source:{harness}:{}",
            expected.sha256
        ));
    }
    for entry in &plan.verified_closure.manifest().entries {
        if entry.role != ClosureFileRole::Declaration {
            continue;
        }
        let relative = entry.path.trim_start_matches("./").replace('\\', "/");
        let suffix = format!("{package_marker}{relative}");
        let matches = sources
            .iter()
            .filter(|source| source.path.replace('\\', "/").ends_with(&suffix))
            .collect::<Vec<_>>();
        if matches.len() != 1 || matches[0].sha256.as_ref() != entry.digest.as_str() {
            let observed = matches
                .iter()
                .take(4)
                .map(|source| format!("{}:{}", source.path, source.sha256))
                .collect::<Vec<_>>();
            return Err(TypeFactsCertificationError::SourceCensus(format!(
                "declaration {relative} is absent, duplicated, or stale; expected {}; observed(count={}, sample={observed:?})",
                entry.digest,
                matches.len(),
            )));
        }
        // `suffix` is exactly this declaration's project-relative path.
        sites.push(format!("typefacts-source:{suffix}:{}", matches[0].sha256));
    }

    let owner_roots = match project {
        Some(project) => vec![project.package_root(plan)?.to_path_buf()],
        None => authenticated_source_root_paths(
            &plan.resolved_import.package_root,
            plan.resolved_import.package_real_root.as_deref(),
        ),
    };
    let mut source_roots = Vec::new();
    for root in owner_roots {
        source_roots.push(SnapshotSourceRoot {
            path: normalized_source_root(&root),
            evidence_prefix: materialized_source_evidence_prefix(project, &root, &package_marker)?,
            snapshot: &plan.snapshot,
            dependency: false,
        });
    }
    for dependency in dependencies {
        let roots = match project {
            Some(project) => vec![project.package_root(dependency)?.to_path_buf()],
            None => authenticated_source_root_paths(
                &dependency.resolved_import.package_root,
                dependency.resolved_import.package_real_root.as_deref(),
            ),
        };
        let fallback_prefix = private_project_package_marker(
            plan,
            &dependency.resolved_import.package_root,
            dependency.snapshot.package_name(),
        );
        for root in roots {
            source_roots.push(SnapshotSourceRoot {
                path: normalized_source_root(&root),
                evidence_prefix: materialized_source_evidence_prefix(
                    project,
                    &root,
                    &fallback_prefix,
                )?,
                snapshot: &dependency.snapshot,
                dependency: true,
            });
        }
    }
    for source in graph_sources {
        let root = project.map_or_else(
            || Ok(PathBuf::from(&source.installed_package_root)),
            |project| project.source_root(source).map(Path::to_path_buf),
        )?;
        let fallback_prefix = private_project_package_marker(
            plan,
            &source.installed_package_root,
            source.snapshot.package_name(),
        );
        source_roots.push(SnapshotSourceRoot {
            path: normalized_source_root(&root),
            evidence_prefix: materialized_source_evidence_prefix(project, &root, &fallback_prefix)?,
            snapshot: &source.snapshot,
            dependency: true,
        });
    }
    deduplicate_snapshot_source_roots(&mut source_roots)?;
    source_roots.sort_by(|left, right| {
        right
            .path
            .len()
            .cmp(&left.path.len())
            .then_with(|| left.path.cmp(&right.path))
    });
    let source_root_paths = source_roots
        .iter()
        .map(|root| root.path.clone())
        .collect::<Vec<_>>();

    reject_unauthenticated_external_sources(&source_root_paths, sources)?;

    // Every source attributed to this package must come from the immutable
    // snapshot. This catches a sibling/ancestor installation silently winning
    // resolution even when the demanded declaration happened to share bytes.
    for source in sources {
        let normalized = source.path.replace('\\', "/");
        let Some((root_index, relative)) =
            strip_materialized_source_root(&normalized, &source_root_paths)
        else {
            continue;
        };
        let root = &source_roots[root_index];
        let snapshot = root.snapshot;
        verify_snapshot_source_digest(snapshot, relative, &source.sha256)?;
        if root.dependency {
            sites.push(format!(
                "typefacts-source-snapshot:{}:{}{}:{}",
                snapshot.provenance_root(),
                root.evidence_prefix,
                relative,
                source.sha256
            ));
        }
    }
    if sites.is_empty() {
        return Err(TypeFactsCertificationError::SourceCensus(
            "verified closure has no declaration source census".into(),
        ));
    }
    Ok(sites)
}

struct SnapshotSourceRoot<'a> {
    path: String,
    evidence_prefix: String,
    snapshot: &'a super::ArtifactSnapshot,
    dependency: bool,
}

fn deduplicate_snapshot_source_roots(
    roots: &mut Vec<SnapshotSourceRoot<'_>>,
) -> Result<(), TypeFactsCertificationError> {
    roots.sort_by(|left, right| left.path.cmp(&right.path));
    let mut unique = Vec::<SnapshotSourceRoot<'_>>::with_capacity(roots.len());
    for root in roots.drain(..) {
        let Some(existing) = unique
            .last_mut()
            .filter(|existing| existing.path == root.path)
        else {
            unique.push(root);
            continue;
        };
        if existing.snapshot.root() != root.snapshot.root()
            || existing.snapshot.provenance_root() != root.snapshot.provenance_root()
            || existing.evidence_prefix != root.evidence_prefix
        {
            return Err(TypeFactsCertificationError::SourceCensus(format!(
                "distinct authenticated snapshots claim one materialized package root: {}",
                diagnostic_identity_path(&root.path)
            )));
        }
        existing.dependency &= root.dependency;
    }
    *roots = unique;
    Ok(())
}

fn normalized_source_root(path: &Path) -> String {
    format!(
        "{}/",
        path.to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
    )
}

fn authenticated_source_root_paths(logical: &str, real: Option<&str>) -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from(logical)];
    if let Some(real) = real {
        roots.push(PathBuf::from(real));
    }
    roots
}

fn materialized_source_evidence_prefix(
    project: Option<&PrivateTypeFactsProject>,
    root: &Path,
    fallback: &str,
) -> Result<String, TypeFactsCertificationError> {
    let Some(project) = project else {
        return Ok(fallback.to_owned());
    };
    let relative = root.strip_prefix(&project.root).map_err(|_| {
        TypeFactsCertificationError::identity_mismatch(
            "source census",
            "materialized_root",
            diagnostic_identity_path(&project.root.to_string_lossy()),
            diagnostic_identity_path(&root.to_string_lossy()),
        )
    })?;
    Ok(format!(
        "/{}/",
        relative
            .to_string_lossy()
            .replace('\\', "/")
            .trim_matches('/')
    ))
}

fn strip_materialized_source_root<'a>(
    source: &'a str,
    roots_longest_first: &[String],
) -> Option<(usize, &'a str)> {
    roots_longest_first
        .iter()
        .enumerate()
        .find_map(|(index, root)| source.strip_prefix(root).map(|relative| (index, relative)))
}

fn verify_snapshot_source_digest(
    snapshot: &super::ArtifactSnapshot,
    relative: &str,
    actual: &str,
) -> Result<(), TypeFactsCertificationError> {
    let bytes = snapshot.read(relative).ok_or_else(|| {
        TypeFactsCertificationError::SourceCensus(format!(
            "producer consulted package source outside the snapshot: {relative}"
        ))
    })?;
    let expected = format!("sha256:{:x}", Sha256::digest(bytes));
    if expected != actual {
        return Err(TypeFactsCertificationError::SourceCensus(format!(
            "producer source digest differs from snapshot: {relative}"
        )));
    }
    Ok(())
}

fn private_project_package_marker(
    parent: &CertificationPlan,
    original_package_root: &str,
    package_name: &str,
) -> String {
    let virtual_root = Path::new("/__solid_checker_private_project__");
    let projected_owner = virtual_root
        .join("node_modules")
        .join(parent.snapshot.package_name());
    let target = private_project_package_target(
        virtual_root,
        &projected_owner,
        Path::new(&parent.resolved_import.package_root),
        Path::new(original_package_root),
        package_name,
    );
    let relative = target
        .strip_prefix(virtual_root)
        .expect("private project target is rooted under its requested project");
    format!("/{}/", relative.to_string_lossy().replace('\\', "/"))
}

fn reject_unauthenticated_external_sources(
    source_roots: &[String],
    sources: &[typefacts::TranscriptSourceDigest],
) -> Result<(), TypeFactsCertificationError> {
    for source in sources {
        let normalized = source.path.replace('\\', "/");
        if normalized.contains("/node_modules/")
            && strip_materialized_source_root(&normalized, source_roots).is_none()
        {
            return Err(TypeFactsCertificationError::SourceCensus(format!(
                "producer consulted unauthenticated external package source: {normalized}"
            )));
        }
    }
    Ok(())
}

fn verify_family(
    proof: &ScheduledProofDemand,
    transcript: &InvocationTranscript,
) -> Result<Vec<String>, TypeFactsCertificationError> {
    let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
        demand: proof.id.clone(),
        reason: reason.into(),
    };
    if transcript.validity != ResolvedCallValidity::Valid
        || transcript.kind == CallKind::Unknown
        || transcript.target.is_empty()
        || transcript.targets.is_some()
        || !transcript.open_reasons.is_empty()
    {
        return Err(open(
            "call target or transcript is unresolved/composite/open",
        ));
    }
    let signature = transcript
        .selected_signature
        .as_ref()
        .ok_or_else(|| open("selected signature is absent"))?;
    let mut sites = vec![format!(
        "call:{}:{}:{}",
        transcript.location.path, transcript.location.start_byte, transcript.location.end_byte
    )];
    match proof.family {
        ProofFamily::SelectedSignature => {
            require_domains(
                transcript,
                &[
                    InvocationDomain::Signature,
                    InvocationDomain::Parameters,
                    InvocationDomain::Result,
                ],
                &open,
            )?;
            require_closed_signature_values(signature, &open)?;
            sites.push(format!(
                "signature:{}:{}:{}:overload:{}/{}",
                signature.identity,
                signature.declaration.location.path,
                signature.declaration.location.start_byte,
                signature.overload_ordinal,
                signature.overload_count
            ));
        }
        ProofFamily::ArgumentBinding => {
            require_domains(
                transcript,
                &[
                    InvocationDomain::Signature,
                    InvocationDomain::Bindings,
                    InvocationDomain::Omissions,
                ],
                &open,
            )?;
            if transcript.bindings.iter().any(|binding| {
                matches!(
                    binding.disposition,
                    ArgumentBindingDisposition::UnknownLengthSpread
                        | ArgumentBindingDisposition::Unmapped
                )
            }) {
                return Err(open("actual-to-formal binding is open"));
            }
            sites.extend(transcript.bindings.iter().flat_map(binding_sites));
        }
        ProofFamily::RestSpreadCoverage => {
            require_domains(transcript, &[InvocationDomain::Bindings], &open)?;
            if transcript.bindings.iter().any(|binding| {
                matches!(
                    binding.disposition,
                    ArgumentBindingDisposition::UnknownLengthSpread
                        | ArgumentBindingDisposition::Unmapped
                )
            }) {
                return Err(open("rest or spread coverage is not exact"));
            }
            sites.extend(transcript.bindings.iter().flat_map(binding_sites));
        }
        ProofFamily::CallablePath => {
            require_domains(
                transcript,
                &[InvocationDomain::Parameters, InvocationDomain::Result],
                &open,
            )?;
            match &proof.subject {
                ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                    callable: DemandedCallability::Callable,
                    ..
                }) => require_recursive_subject(proof, signature, None, &open, &mut sites)?,
                ProofDemandSubject::PositiveFact(PositiveFactSubject::CallbackBinding {
                    ..
                }) => {
                    return Err(TypeFactsCertificationError::UnsupportedDemand {
                        demand: proof.id.clone(),
                        reason: "Type Facts does not yet bind callback operations to an exact callable path"
                            .into(),
                    });
                }
                _ => {
                    return Err(TypeFactsCertificationError::UnsupportedDemand {
                        demand: proof.id.clone(),
                        reason: "callable-path demand has no exact callable-value subject".into(),
                    });
                }
            }
        }
        ProofFamily::OperationReachability => {
            if matches!(
                &proof.subject,
                ProofDemandSubject::PositiveFact(
                    PositiveFactSubject::Operation { .. }
                        | PositiveFactSubject::OperationEdge { .. }
                )
            ) {
                return Err(TypeFactsCertificationError::UnsupportedDemand {
                    demand: proof.id.clone(),
                    reason: "Type Facts control flow is not yet bound to an exact operation or edge subject"
                        .into(),
                });
            }
            require_domains(
                transcript,
                &[InvocationDomain::Uses, InvocationDomain::ControlFlow],
                &open,
            )?;
            let flow = transcript
                .control_flow
                .as_ref()
                .ok_or_else(|| open("control-flow census is absent"))?;
            if !flow.unsupported.is_empty()
                || flow
                    .returns
                    .iter()
                    .any(|site| site.reach == Reachability::Unknown)
                || flow
                    .throws
                    .iter()
                    .any(|site| site.reach == Reachability::Unknown)
                || flow
                    .branches
                    .iter()
                    .any(|site| site.reach == Reachability::Unknown)
                || transcript.parameter_uses.iter().any(|usage| {
                    matches!(
                        usage.kind,
                        ParameterUseKind::ArgumentUnknown
                            | ParameterUseKind::Storage
                            | ParameterUseKind::Capture
                            | ParameterUseKind::UnknownEscape
                    )
                })
            {
                return Err(open(
                    "operation reachability contains an open escape or branch",
                ));
            }
            sites.extend(control_flow_sites(flow));
        }
        ProofFamily::OperationCardinality => {
            return Err(TypeFactsCertificationError::UnsupportedDemand {
                demand: proof.id.clone(),
                reason: "Type Facts has no exact arbitrary runtime loop/reentry bound".into(),
            });
        }
        ProofFamily::RecursiveValueShape => {
            require_recursive_subject(proof, signature, None, &open, &mut sites)?;
        }
        ProofFamily::GuardPartition => {
            if matches!(
                &proof.subject,
                ProofDemandSubject::PositiveFact(PositiveFactSubject::GuardCase { .. })
            ) {
                return Err(TypeFactsCertificationError::UnsupportedDemand {
                    demand: proof.id.clone(),
                    reason: "Type Facts partitions are not yet bound to an exact guard ordinal"
                        .into(),
                });
            }
            let complete = signature
                .parameters
                .iter()
                .flat_map(|parameter| &parameter.value.partitions)
                .chain(&signature.result.partitions)
                .chain(
                    transcript
                        .control_flow
                        .iter()
                        .flat_map(|flow| &flow.branches)
                        .flat_map(|branch| &branch.partitions),
                )
                .filter(|partition| partition.complete)
                .collect::<Vec<_>>();
            if complete.is_empty() {
                return Err(open("no complete finite guard partition was reported"));
            }
            sites.extend(complete.into_iter().map(partition_site));
        }
        ProofFamily::DomainExhaustiveness => {
            if signature.overload_count != 1 {
                return Err(TypeFactsCertificationError::UnsupportedDemand {
                    demand: proof.id.clone(),
                    reason: "export overload domain requires one verifier demand per overload"
                        .into(),
                });
            }
            require_domain_closure(proof, transcript, signature, &open)?;
            sites.push("typefacts-domain-census:complete".into());
        }
        _ => {
            return Err(TypeFactsCertificationError::UnsupportedDemand {
                demand: proof.id.clone(),
                reason: "demand is not Type Facts-owned".into(),
            });
        }
    }
    Ok(sites)
}

fn require_domains(
    transcript: &InvocationTranscript,
    domains: &[InvocationDomain],
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    if domains
        .iter()
        .any(|domain| !transcript.completeness.contains(*domain))
    {
        return Err(open("required producer domain is not complete"));
    }
    Ok(())
}

fn require_closed_signature_values(
    signature: &typefacts::SelectedSignature,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    for value in signature
        .parameters
        .iter()
        .map(|parameter| &parameter.value)
        .chain(std::iter::once(&signature.result))
    {
        require_closed_value(value, open)?;
    }
    Ok(())
}

fn require_all_callable_paths_closed(
    signature: &typefacts::SelectedSignature,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    for path in signature
        .parameters
        .iter()
        .flat_map(|parameter| &parameter.callable_paths)
        .chain(&signature.result_callable_paths)
    {
        if !callable_path_census_is_closed(path) {
            return Err(open("selected signature contains an open callable path"));
        }
    }
    Ok(())
}

fn require_closed_value(
    value: &InvocationValueFact,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    if !value.open_reasons.is_empty()
        || value
            .alternatives
            .iter()
            .any(|alternative| !alternative.open_reasons.is_empty())
        || value.partitions.iter().any(|partition| !partition.complete)
    {
        return Err(open("value shape contains an unresolved recursive leaf"));
    }
    Ok(())
}

/// Supplies the premise for the one recursive-value demand that would otherwise
/// have none: the empty path with no callability assertion.
///
/// Every other combination already carries one. A non-empty path must be found
/// in the producer census, be closed, and match its demanded callability — the
/// sibling premises stand on their own, so an unasserted callability there is
/// merely one premise fewer. At the root there is no census entry to find and,
/// without an assertion, no callability to check either, so
/// `require_root_callability` returns `Ok` on its first arm and the positive
/// fact is recorded as proved by nothing.
///
/// The premise the root does have is the producer's *observation* of it. This
/// requires that observation to be closed: no open reason anywhere in the value
/// or its alternatives, every finite partition complete, the primitive domain
/// not the explicit `unknown` marker, and a callability actually answered. A
/// producer that says "I did not finish looking at this value" proves nothing,
/// and the fact stays open.
///
/// Why that is sound rather than a weakened check. The fact being discharged is
/// the IR's *shape* claim — "this operation returns a Reactive", "this export
/// returns a tuple" — and the demand deliberately asserts nothing about
/// callability, so nothing is being asserted onto the declaration. The keyed
/// class of contradiction (an implementation-derived shape demanding
/// non-callability of a declaration that says otherwise) cannot recur here,
/// because there is no assertion for the producer to disagree with. What the
/// closed answer establishes is the one thing the root needs: that the producer
/// exhaustively observed the value this fact is about.
fn require_verifiable_root_premise(
    value: &InvocationValueFact,
    callable: DemandedCallability,
    label: &str,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    if callable.is_asserted() {
        return Ok(());
    }
    require_closed_value(value, open).map_err(|_| {
        open(&format!(
            "{label} root shape has no verifiable premise: the demand asserts no callability and the producer's root observation is open"
        ))
    })?;
    if value.callability == Callability::Unknown
        || value.constructability == InvocationConstructability::Unknown
        || value.primitive.unknown
    {
        return Err(open(&format!(
            "{label} root shape has no verifiable premise: the demand asserts no callability and the producer did not exhaustively observe the root"
        )));
    }
    Ok(())
}

fn require_root_callability(
    value: &InvocationValueFact,
    callable: DemandedCallability,
    label: &str,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    match (callable, value.callability, value.constructability) {
        (DemandedCallability::Unknown, _, _)
        | (
            DemandedCallability::Callable,
            Callability::Callable | Callability::UntypedCallable,
            _,
        )
        | (DemandedCallability::Callable, _, InvocationConstructability::Constructable)
        | (
            DemandedCallability::NonCallable,
            Callability::NonCallable,
            InvocationConstructability::NonConstructable,
        ) => Ok(()),
        (DemandedCallability::Callable, _, _) => Err(open(&format!(
            "{label} is not compiler-proved callable or constructable"
        ))),
        (DemandedCallability::NonCallable, _, _) => Err(open(&format!(
            "{label} is not compiler-proved non-callable and non-constructable"
        ))),
    }
}

/// Verifies a demanded callability against one exact callable-path census
/// entry. An unasserted callability leaves the entry's own closure premises —
/// checked by the caller — as the whole of the demand.
fn require_path_callability(
    callable: DemandedCallability,
    fact: &typefacts::CallablePathFact,
    label: &str,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    match (callable, fact.callability) {
        (DemandedCallability::Unknown, _)
        | (DemandedCallability::Callable, Callability::Callable)
        | (DemandedCallability::NonCallable, Callability::NonCallable) => Ok(()),
        (DemandedCallability::Callable, _) => {
            Err(open(&format!("{label} is not compiler-proved callable")))
        }
        (DemandedCallability::NonCallable, _) => Err(open(&format!(
            "{label} is not compiler-proved non-callable"
        ))),
    }
}

fn binding_sites(binding: &typefacts::ArgumentBinding) -> Vec<String> {
    if binding.slots.is_empty() {
        return vec![format!(
            "argument:{}:{:?}",
            binding.argument_index, binding.disposition
        )];
    }
    binding
        .slots
        .iter()
        .map(|slot| {
            format!(
                "argument:{}:expanded:{}:formal:{}:tuple:{:?}:rest:{}",
                binding.argument_index,
                slot.expanded_index,
                slot.parameter_index,
                slot.tuple_index,
                slot.rest
            )
        })
        .collect()
}

fn callable_path_site(path: &typefacts::CallablePathFact) -> String {
    let segments = path
        .path
        .iter()
        .map(|segment| match segment.kind {
            PathSegmentKind::Property => format!(".{}", segment.property),
            PathSegmentKind::Tuple => format!("[{}]", segment.index.unwrap_or_default()),
        })
        .collect::<String>();
    format!(
        "callable-path:alternative:{}:{}:{:?}:{:?}",
        path.alternative, segments, path.presence, path.callability
    )
}

fn control_flow_sites(flow: &typefacts::ControlFlowCensus) -> Vec<String> {
    flow.returns
        .iter()
        .map(|site| {
            format!(
                "return:{}:{}:{:?}",
                site.location.path, site.location.start_byte, site.reach
            )
        })
        .chain(flow.throws.iter().map(|site| {
            format!(
                "throw:{}:{}:{:?}",
                site.location.path, site.location.start_byte, site.reach
            )
        }))
        .chain(flow.branches.iter().map(|site| {
            format!(
                "branch:{}:{}:{:?}",
                site.location.path, site.location.start_byte, site.reach
            )
        }))
        .collect()
}

fn partition_site(partition: &FinitePartition) -> String {
    format!(
        "partition:{:?}:cases:{}:complete:{}",
        partition.axis,
        partition.cases.len(),
        partition.complete
    )
}

fn require_recursive_subject(
    proof: &ScheduledProofDemand,
    signature: &typefacts::SelectedSignature,
    bound_operation: Option<&str>,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
        root,
        path,
        callable,
        ..
    }) = &proof.subject
    else {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "recursive-value demand has no exact recursive subject".into(),
        });
    };
    let (value, callable_paths) = match root {
        ValueRoot::Export => {
            return Err(TypeFactsCertificationError::UnsupportedDemand {
                demand: proof.id.clone(),
                reason: "selected-call Type Facts cannot stand in for the exported value root"
                    .into(),
            });
        }
        ValueRoot::OperationOutput { operation } => {
            let Some(bound_operation) = bound_operation else {
                return Err(TypeFactsCertificationError::UnsupportedDemand {
                    demand: proof.id.clone(),
                    reason: "selected-call Type Facts is not yet bound to the named operation root"
                        .into(),
                });
            };
            if operation.0 != bound_operation {
                return Err(TypeFactsCertificationError::SubjectMismatch {
                    demand: proof.id.clone(),
                    reason:
                        "recursive output names a different operation than the producer binding"
                            .into(),
                });
            }
            (&signature.result, &signature.result_callable_paths)
        }
        ValueRoot::OperationInput { operation, index } => {
            let Some(bound_operation) = bound_operation else {
                return Err(TypeFactsCertificationError::UnsupportedDemand {
                    demand: proof.id.clone(),
                    reason: "selected-call Type Facts is not yet bound to the named operation root"
                        .into(),
                });
            };
            if operation.0 != bound_operation {
                return Err(TypeFactsCertificationError::SubjectMismatch {
                    demand: proof.id.clone(),
                    reason: "recursive input names a different operation than the producer binding"
                        .into(),
                });
            }
            let parameter = signature
                .parameters
                .get(usize::from(*index))
                .ok_or_else(|| open("recursive input root names a missing formal parameter"))?;
            (&parameter.value, &parameter.callable_paths)
        }
    };
    if path.0.is_empty() {
        require_root_callability(value, *callable, "recursive value root", open)?;
        sites.push("recursive-value:root".into());
        return Ok(());
    }
    let (alternative, expected_path) = translate_value_path(&path.0).ok_or_else(|| {
        TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "Type Facts cannot address this recursive path exactly".into(),
        }
    })?;
    let fact = callable_paths
        .iter()
        .find(|fact| fact.alternative == alternative && fact.path == expected_path)
        .ok_or_else(|| open("recursive path is absent from the exact producer census"))?;
    if !callable_path_is_present_and_locally_closed(fact) {
        return Err(open("recursive path is locally open"));
    }
    if callable.asserts_callable() && fact.callability != Callability::Callable {
        return Err(open(
            "recursive callable positive is not compiler-proved callable",
        ));
    }
    sites.push(callable_path_site(fact));
    Ok(())
}

fn translate_value_path(path: &[ValuePathSegment]) -> Option<(usize, Vec<typefacts::PathSegment>)> {
    let mut alternative = 0;
    let mut translated = Vec::new();
    for segment in path {
        match segment {
            ValuePathSegment::ChoiceAlternative(index) if translated.is_empty() => {
                alternative = usize::try_from(*index).ok()?;
            }
            ValuePathSegment::TupleItem(index) => translated.push(typefacts::PathSegment {
                kind: PathSegmentKind::Tuple,
                property: "".into(),
                index: Some(usize::try_from(*index).ok()?),
            }),
            ValuePathSegment::ObjectProperty(property) => {
                translated.push(typefacts::PathSegment {
                    kind: PathSegmentKind::Property,
                    property: property.clone().into(),
                    index: None,
                });
            }
            ValuePathSegment::ArrayElement
            | ValuePathSegment::PromiseValue
            | ValuePathSegment::AsyncIterableElement
            | ValuePathSegment::ChoiceAlternative(_) => return None,
        }
    }
    Some((alternative, translated))
}

fn require_domain_closure(
    proof: &ScheduledProofDemand,
    transcript: &InvocationTranscript,
    signature: &typefacts::SelectedSignature,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    let ProofDemandSubject::DomainClosure { subject, .. } = &proof.subject else {
        return Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "domain-exhaustiveness demand has no closure subject".into(),
        });
    };
    let domains: &[InvocationDomain] = match &subject.path {
        SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Callbacks)) => &[
            InvocationDomain::Signature,
            InvocationDomain::Bindings,
            InvocationDomain::Uses,
        ],
        SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Throws | ClaimDomain::Returns))
        | SemanticClaimPath::Domain(ClaimPath::Operation { .. })
        | SemanticClaimPath::Operation(_) => &[
            InvocationDomain::Signature,
            InvocationDomain::Uses,
            InvocationDomain::ControlFlow,
        ],
        SemanticClaimPath::Domain(ClaimPath::Value { .. }) => &[
            InvocationDomain::Signature,
            InvocationDomain::Parameters,
            InvocationDomain::Result,
        ],
        SemanticClaimPath::Domain(ClaimPath::GuardPartition) => &[
            InvocationDomain::Signature,
            InvocationDomain::Parameters,
            InvocationDomain::Result,
            InvocationDomain::ControlFlow,
        ],
        SemanticClaimPath::Domain(ClaimPath::Call(_) | ClaimPath::Resource { .. }) => &[
            InvocationDomain::Signature,
            InvocationDomain::Bindings,
            InvocationDomain::Uses,
            InvocationDomain::ControlFlow,
        ],
    };
    require_domains(transcript, domains, open)?;
    require_closed_signature_values(signature, open)?;
    require_all_callable_paths_closed(signature, open)?;
    if domains.contains(&InvocationDomain::ControlFlow)
        && transcript
            .control_flow
            .as_ref()
            .is_none_or(|flow| !flow.unsupported.is_empty())
    {
        return Err(open("closure control-flow census is absent or unsupported"));
    }
    Ok(())
}

fn proof_family_name(family: ProofFamily) -> &'static str {
    match family {
        ProofFamily::SelectedSignature => "selected-signature",
        ProofFamily::ArgumentBinding => "argument-binding",
        ProofFamily::RestSpreadCoverage => "rest-spread-coverage",
        ProofFamily::CallablePath => "callable-path",
        ProofFamily::OperationReachability => "operation-reachability",
        ProofFamily::OperationCardinality => "operation-cardinality",
        ProofFamily::RecursiveValueShape => "recursive-value-shape",
        ProofFamily::GuardPartition => "guard-partition",
        ProofFamily::DomainExhaustiveness => "domain-exhaustiveness",
        _ => "not-type-facts",
    }
}

fn witness_variant(family: ProofFamily) -> ProofWitnessVariant {
    match family {
        ProofFamily::SelectedSignature => ProofWitnessVariant::SelectedSignature,
        ProofFamily::ArgumentBinding => ProofWitnessVariant::ArgumentBinding,
        ProofFamily::RestSpreadCoverage => ProofWitnessVariant::RestSpreadCoverage,
        ProofFamily::CallablePath => ProofWitnessVariant::CallablePath,
        ProofFamily::OperationReachability => ProofWitnessVariant::OperationReachability,
        ProofFamily::OperationCardinality => ProofWitnessVariant::OperationCardinality,
        ProofFamily::RecursiveValueShape => ProofWitnessVariant::RecursiveValueShape,
        ProofFamily::GuardPartition => ProofWitnessVariant::GuardPartition,
        ProofFamily::DomainExhaustiveness => ProofWitnessVariant::DomainExhaustiveness,
        _ => unreachable!("only Type Facts families reach witness construction"),
    }
}

struct PrivateExecutionImage {
    directory: PathBuf,
    path: PathBuf,
    /// A content-addressed image under the shared store, kept for the next
    /// certification rather than removed with this one.
    shared: bool,
}

impl PrivateExecutionImage {
    fn copy_from_pin(pin: &TypeFactsProducerPin) -> Result<Self, TypeFactsCertificationError> {
        verify_buildinfo(pin)?;
        let metadata = fs::symlink_metadata(&pin.path).map_err(|error| {
            TypeFactsCertificationError::ProducerProvenance(format!(
                "could not inspect pinned Type Facts image: {error}"
            ))
        })?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(TypeFactsCertificationError::ProducerProvenance(
                "pinned Type Facts image must be a regular non-symlink file".into(),
            ));
        }

        if let Some(store) = materialized_store_root()
            && let Some(image) = Self::shared_from_pin(&store, pin)?
        {
            return Ok(image);
        }

        let directory = create_private_directory()?;
        let path = directory.join("solid-typefacts");
        let image = Self {
            directory,
            path,
            shared: false,
        };
        let copied = copy_and_hash(&pin.path, image.path())?;
        if copied != pin.executable_sha256 {
            return Err(TypeFactsCertificationError::ProducerProvenance(
                "pinned Type Facts bytes do not match the configured digest".into(),
            ));
        }
        set_execution_permissions(image.path())?;
        let reopened = hash_file(image.path())?;
        if reopened != pin.executable_sha256 {
            return Err(TypeFactsCertificationError::ProducerProvenance(
                "private Type Facts image changed before launch".into(),
            ));
        }
        Ok(image)
    }

    /// The pinned image under the shared store, at a path named by its own
    /// digest, created once and launched by every certification afterwards.
    ///
    /// Why this exists: macOS assesses a newly created executable the first
    /// time it launches, taking about half a second and serializing those
    /// assessments system-wide, so a private copy per certification made the
    /// producer launch cost ~5 s under twenty concurrent certifications — the
    /// whole of witness acquisition. Launching one long-lived inode is
    /// assessed once. What is verified does not change: the file is hashed
    /// against the pin before every launch here, again by the pinned producer
    /// construction, and again immediately before the spawn, exactly as the
    /// private copy was. Store failures fall back to the private copy; a pin
    /// mismatch is refused.
    fn shared_from_pin(
        store: &Path,
        pin: &TypeFactsProducerPin,
    ) -> Result<Option<Self>, TypeFactsCertificationError> {
        let Some(digest_hex) = pin.executable_sha256.as_str().strip_prefix("sha256:") else {
            return Ok(None);
        };
        let directory = store.join("images").join(digest_hex);
        let path = directory.join("solid-typefacts");
        if path.is_file() && hash_file(&path)? == pin.executable_sha256 {
            return Ok(Some(Self {
                directory,
                path,
                shared: true,
            }));
        }
        let Some(parent) = directory.parent() else {
            return Ok(None);
        };
        if fs::create_dir_all(parent).is_err() {
            return Ok(None);
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let staging = parent.join(format!(
            ".{digest_hex}.staging-{}-{nonce}",
            std::process::id()
        ));
        if fs::create_dir(&staging).is_err() || set_directory_permissions(&staging).is_err() {
            let _ = fs::remove_dir_all(&staging);
            return Ok(None);
        }
        let staged = staging.join("solid-typefacts");
        let copied = match copy_and_hash(&pin.path, &staged) {
            Ok(copied) => copied,
            Err(_) => {
                let _ = fs::remove_dir_all(&staging);
                return Ok(None);
            }
        };
        if copied != pin.executable_sha256 {
            let _ = fs::remove_dir_all(&staging);
            return Err(TypeFactsCertificationError::ProducerProvenance(
                "pinned Type Facts bytes do not match the configured digest".into(),
            ));
        }
        if set_execution_permissions(&staged).is_err() {
            let _ = fs::remove_dir_all(&staging);
            return Ok(None);
        }
        if directory.exists() {
            let retired = parent.join(format!(
                ".{digest_hex}.retired-{}-{nonce}",
                std::process::id()
            ));
            if fs::rename(&directory, &retired).is_ok() {
                let _ = fs::remove_dir_all(&retired);
            }
        }
        match fs::rename(&staging, &directory) {
            Ok(()) => {}
            Err(_) if path.is_file() => {
                // A concurrent certification published the same image first.
                let _ = fs::remove_dir_all(&staging);
            }
            Err(_) => {
                let _ = fs::remove_dir_all(&staging);
                return Ok(None);
            }
        }
        if hash_file(&path)? != pin.executable_sha256 {
            return Err(TypeFactsCertificationError::ProducerProvenance(
                "shared Type Facts image changed before launch".into(),
            ));
        }
        Ok(Some(Self {
            directory,
            path,
            shared: true,
        }))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateExecutionImage {
    fn drop(&mut self) {
        if self.shared {
            return;
        }
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProducerBuildInfo {
    format: u32,
    source_digest: String,
    build_id: String,
}

fn verify_buildinfo(pin: &TypeFactsProducerPin) -> Result<(), TypeFactsCertificationError> {
    let stamp_path = PathBuf::from(format!("{}.buildinfo", pin.path.display()));
    let bytes = fs::read(&stamp_path).map_err(|error| {
        TypeFactsCertificationError::ProducerProvenance(format!(
            "could not read Type Facts source-manifest stamp: {error}"
        ))
    })?;
    let info: ProducerBuildInfo = serde_json::from_slice(&bytes).map_err(|error| {
        TypeFactsCertificationError::ProducerProvenance(format!(
            "invalid Type Facts source-manifest stamp: {error}"
        ))
    })?;
    let source = SourceHash::parse(format!("sha256:{}", info.source_digest))?;
    if info.format != 1
        || source != pin.source_manifest_sha256
        || info.build_id != typefacts::v3::TYPE_FACTS_BUILD_ID
    {
        return Err(TypeFactsCertificationError::ProducerProvenance(
            "Type Facts source-manifest stamp does not match the configured pin".into(),
        ));
    }
    Ok(())
}

fn create_private_directory() -> Result<PathBuf, TypeFactsCertificationError> {
    let base = std::env::temp_dir();
    for _ in 0..128 {
        let candidate = base.join(format!(
            "solid-checker-typefacts-cert-{}-{}",
            std::process::id(),
            EXECUTION_IMAGE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => {
                set_directory_permissions(&candidate)?;
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(TypeFactsCertificationError::ProducerProvenance(
        "could not allocate a private Type Facts execution directory".into(),
    ))
}

/// Copies the pinned image into the private directory and returns the digest
/// of the *destination* bytes. The copy is an APFS clone where the filesystem
/// allows it (`crate::clone_or_copy_file`): a copy-on-write private file that
/// costs no data write. The digest is always taken from the destination, so
/// what is verified is the file that will launch, whichever way it was made.
fn copy_and_hash(
    source: &Path,
    destination: &Path,
) -> Result<SourceHash, TypeFactsCertificationError> {
    crate::clone_or_copy_file(source, destination)?;
    hash_file(destination)
}

fn hash_file(path: &Path) -> Result<SourceHash, TypeFactsCertificationError> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(SourceHash::parse(format!("sha256:{:x}", hash.finalize()))?)
}

#[cfg(unix)]
fn set_directory_permissions(path: &Path) -> Result<(), TypeFactsCertificationError> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_directory_permissions(_path: &Path) -> Result<(), TypeFactsCertificationError> {
    Err(TypeFactsCertificationError::ProducerProvenance(
        "this platform cannot establish private Type Facts execution permissions".into(),
    ))
}

#[cfg(unix)]
fn set_execution_permissions(path: &Path) -> Result<(), TypeFactsCertificationError> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o500))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_execution_permissions(_path: &Path) -> Result<(), TypeFactsCertificationError> {
    Err(TypeFactsCertificationError::ProducerProvenance(
        "this platform cannot establish immutable Type Facts execution permissions".into(),
    ))
}

#[derive(Debug, Error)]
pub enum TypeFactsCertificationError {
    #[error("Type Facts certification failed during {stage}: {source}")]
    TransactionStage {
        stage: &'static str,
        #[source]
        source: Box<TypeFactsCertificationError>,
    },
    #[error("Type Facts certification failed for graph node {package} during {stage}: {source}")]
    GraphNodeStage {
        package: String,
        stage: &'static str,
        #[source]
        source: Box<TypeFactsCertificationError>,
    },
    #[error("Type Facts certification names unknown proof demand {0}")]
    UnknownDemand(String),
    #[error("Type Facts certification repeats proof demand {0}")]
    DuplicateDemand(String),
    #[error("Type Facts certification omits proof demand {0}")]
    MissingDemand(String),
    #[error("Type Facts producer provenance is invalid: {0}")]
    ProducerProvenance(String),
    #[error(
        "Type Facts live-session identity mismatch at {site}: field {field} expected {expected}, actual {actual}"
    )]
    IdentityMismatch {
        site: &'static str,
        field: &'static str,
        expected: Box<str>,
        actual: Box<str>,
    },
    #[error("Type Facts snapshot source census is invalid: {0}")]
    SourceCensus(String),
    #[error("Type Facts demand {demand} is locally open: {reason}")]
    FamilyOpen { demand: String, reason: String },
    #[error("Type Facts demand {demand} is unsupported: {reason}")]
    UnsupportedDemand { demand: String, reason: String },
    #[error("Type Facts demand {demand} does not match its exact export subject: {reason}")]
    SubjectMismatch { demand: String, reason: String },
    #[error(transparent)]
    TypeFacts(#[from] typefacts::TypeFactsError),
    #[error(transparent)]
    Session(#[from] typefacts::SessionError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl TypeFactsCertificationError {
    fn identity_mismatch(
        site: &'static str,
        field: &'static str,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self::IdentityMismatch {
            site,
            field,
            expected: expected.into().into_boxed_str(),
            actual: actual.into().into_boxed_str(),
        }
    }

    fn at_stage(self, stage: &'static str) -> Self {
        Self::TransactionStage {
            stage,
            source: Box::new(self),
        }
    }

    fn at_graph_node(self, plan: &CertificationPlan, stage: &'static str) -> Self {
        Self::GraphNodeStage {
            package: format!(
                "{}@{} ({})",
                plan.resolved_import.package_name,
                plan.resolved_import.package_version,
                plan.resolved_import.requested_entrypoint
            ),
            stage,
            source: Box::new(self),
        }
    }
}

fn diagnostic_identity_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if let Some(index) = normalized.find("/node_modules/") {
        return normalized[index..].to_owned();
    }
    if let Some(index) = normalized.find("solid-checker-typefacts-project-") {
        let suffix = normalized[index..]
            .find('/')
            .map(|offset| &normalized[index + offset..])
            .unwrap_or("");
        return format!("<private-project>{suffix}");
    }
    if Path::new(&normalized).is_absolute() {
        return Path::new(&normalized)
            .file_name()
            .map(|name| format!("<absolute>/{}", name.to_string_lossy()))
            .unwrap_or_else(|| "<absolute>".into());
    }
    normalized
}

fn diagnostic_identity_path_pair(expected: &str, actual: &str) -> (String, String) {
    let mut expected_rendered = diagnostic_identity_path(expected);
    let mut actual_rendered = diagnostic_identity_path(actual);
    if expected != actual && expected_rendered == actual_rendered {
        expected_rendered.push_str(" [expected identity]");
        actual_rendered.push_str(" [different actual identity]");
    }
    (expected_rendered, actual_rendered)
}

fn diagnostic_location(location: Option<&typefacts::Location>) -> String {
    location.map_or_else(
        || "None".into(),
        |location| {
            format!(
                "{}:{}-{}",
                diagnostic_identity_path(&location.path),
                location.start_byte,
                location.end_byte
            )
        },
    )
}

fn diagnostic_location_pair(
    expected: Option<&typefacts::Location>,
    actual: Option<&typefacts::Location>,
) -> (String, String) {
    let mut expected_rendered = diagnostic_location(expected);
    let mut actual_rendered = diagnostic_location(actual);
    if expected != actual && expected_rendered == actual_rendered {
        expected_rendered.push_str(" [expected identity]");
        actual_rendered.push_str(" [different actual identity]");
    }
    (expected_rendered, actual_rendered)
}

fn diagnostic_plan_key(key: &(String, String)) -> String {
    format!("{}@{}", key.0, diagnostic_identity_path(&key.1))
}

fn verify_schedule_identity(
    site: &'static str,
    expected_root: &str,
    actual_root: &str,
    incompatible_count_field: &'static str,
    incompatible_count: usize,
) -> Result<(), TypeFactsCertificationError> {
    if actual_root != expected_root {
        return Err(TypeFactsCertificationError::identity_mismatch(
            site,
            "demand_graph_root",
            expected_root.to_owned(),
            actual_root.to_owned(),
        ));
    }
    if incompatible_count != 0 {
        return Err(TypeFactsCertificationError::identity_mismatch(
            site,
            incompatible_count_field,
            "0",
            incompatible_count.to_string(),
        ));
    }
    Ok(())
}

fn verify_implementation_location_identity(
    expected: Option<&typefacts::Location>,
    actual: Option<&typefacts::Location>,
) -> Result<(), TypeFactsCertificationError> {
    if expected == actual {
        return Ok(());
    }
    let (expected, actual) = diagnostic_location_pair(expected, actual);
    Err(TypeFactsCertificationError::identity_mismatch(
        "export-value verification",
        "implementation_location",
        expected,
        actual,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use solid_reactive_ir::contract_semantics::{
        CallbackInvocation, certification::proof_policy_2, solid2_rc3::conformance_corpus,
    };

    fn digest(bytes: &[u8]) -> String {
        format!("sha256:{:x}", Sha256::digest(bytes))
    }

    fn source_snapshot(
        root: &str,
        provenance_root: &str,
        files: &[(&str, &[u8])],
    ) -> super::super::ArtifactSnapshot {
        super::super::ArtifactSnapshot {
            package_name: "source-package".into(),
            package_version: "1.0.0".into(),
            package_integrity: "sha512:test".into(),
            files: std::sync::Arc::new(
                files
                    .iter()
                    .map(|(path, bytes)| ((*path).to_owned(), std::sync::Arc::<[u8]>::from(*bytes)))
                    .collect(),
            ),
            directories: std::sync::Arc::new(std::collections::BTreeSet::new()),
            root: root.into(),
            provenance_root: provenance_root.into(),
        }
    }

    #[test]
    fn identity_mismatch_names_the_exact_site_field_and_redacted_values() {
        let expected = typefacts::Location {
            path: "/private/tmp/solid-checker-typefacts-project-123-1/node_modules/pkg/a.js".into(),
            start_byte: 10,
            end_byte: 20,
        };
        let actual = typefacts::Location {
            path: "/Users/example/secret/node_modules/pkg/b.js".into(),
            start_byte: 30,
            end_byte: 40,
        };
        let error = verify_implementation_location_identity(Some(&expected), Some(&actual))
            .expect_err("different implementation locations must fail closed");
        assert!(matches!(
            &error,
            TypeFactsCertificationError::IdentityMismatch {
                site: "export-value verification",
                field: "implementation_location",
                expected,
                actual,
            } if expected.as_ref() == "/node_modules/pkg/a.js:10-20"
                && actual.as_ref() == "/node_modules/pkg/b.js:30-40"
        ));
        let rendered = error.to_string();
        assert!(!rendered.contains("/private/tmp"));
        assert!(!rendered.contains("/Users/example"));

        let absent = verify_implementation_location_identity(Some(&expected), None)
            .expect_err("a missing producer implementation must fail closed");
        assert!(matches!(
            absent,
            TypeFactsCertificationError::IdentityMismatch {
                site: "export-value verification",
                field: "implementation_location",
                expected,
                actual,
            } if expected.as_ref() == "/node_modules/pkg/a.js:10-20"
                && actual.as_ref() == "None"
        ));
        assert!(verify_implementation_location_identity(Some(&expected), Some(&expected)).is_ok());
        assert!(verify_implementation_location_identity(None, None).is_ok());
        let unexpected = verify_implementation_location_identity(None, Some(&actual))
            .expect_err("an unexpected producer implementation must fail closed");
        assert!(matches!(
            unexpected,
            TypeFactsCertificationError::IdentityMismatch {
                site: "export-value verification",
                field: "implementation_location",
                expected,
                actual,
            } if expected.as_ref() == "None"
                && actual.as_ref() == "/node_modules/pkg/b.js:30-40"
        ));

        let same_suffix_other_root = typefacts::Location {
            path: "/private/tmp/solid-checker-typefacts-project-999-2/node_modules/pkg/a.js".into(),
            start_byte: 10,
            end_byte: 20,
        };
        let collision =
            verify_implementation_location_identity(Some(&expected), Some(&same_suffix_other_root))
                .expect_err("different private roots must remain visibly distinct");
        assert!(matches!(
            collision,
            TypeFactsCertificationError::IdentityMismatch {
                expected,
                actual,
                ..
            } if expected.as_ref() == "/node_modules/pkg/a.js:10-20 [expected identity]"
                && actual.as_ref()
                    == "/node_modules/pkg/a.js:10-20 [different actual identity]"
        ));
    }

    #[test]
    fn schedule_identity_checks_root_before_the_incompatible_family_count() {
        let root = verify_schedule_identity(
            "export-value verification",
            "sha256:expected",
            "sha256:actual",
            "invocation_count",
            1,
        )
        .expect_err("a wrong graph root must fail closed");
        assert!(matches!(
            root,
            TypeFactsCertificationError::IdentityMismatch {
                site: "export-value verification",
                field: "demand_graph_root",
                expected,
                actual,
            } if expected.as_ref() == "sha256:expected"
                && actual.as_ref() == "sha256:actual"
        ));

        let count = verify_schedule_identity(
            "export-value verification",
            "sha256:same",
            "sha256:same",
            "invocation_count",
            1,
        )
        .expect_err("an invocation schedule cannot enter export-value verification");
        assert!(matches!(
            count,
            TypeFactsCertificationError::IdentityMismatch {
                site: "export-value verification",
                field: "invocation_count",
                expected,
                actual,
            } if expected.as_ref() == "0" && actual.as_ref() == "1"
        ));
        assert!(
            verify_schedule_identity(
                "export-value verification",
                "sha256:same",
                "sha256:same",
                "invocation_count",
                0,
            )
            .is_ok()
        );
    }

    #[test]
    fn diagnostic_paths_remove_private_run_identity_without_losing_package_suffixes() {
        assert_eq!(
            diagnostic_identity_path(
                "/private/tmp/solid-checker-typefacts-project-44-9/tsconfig.json"
            ),
            "<private-project>/tsconfig.json"
        );
        assert_eq!(
            diagnostic_identity_path(
                "/Users/alice/project/node_modules/outer/dist/node_modules/inner/index.d.ts"
            ),
            "/node_modules/outer/dist/node_modules/inner/index.d.ts"
        );
        assert_eq!(
            diagnostic_identity_path("/Users/alice/secret"),
            "<absolute>/secret"
        );
        assert_eq!(
            diagnostic_identity_path_pair(
                "/private/tmp/solid-checker-typefacts-project-1/tsconfig.json",
                "/private/tmp/solid-checker-typefacts-project-2/tsconfig.json",
            ),
            (
                "<private-project>/tsconfig.json [expected identity]".into(),
                "<private-project>/tsconfig.json [different actual identity]".into(),
            )
        );
    }

    #[test]
    fn export_value_preflight_admits_runtime_bound_function_demands_but_requires_total_assignment()
    {
        let candidate = conformance_corpus()
            .into_iter()
            .next()
            .expect("conformance corpus")
            .proposal
            .normalize()
            .expect("normalized candidate");
        let policy = proof_policy_2();
        let candidates = policy
            .inspect_candidates(&candidate)
            .expect("candidate inventory");
        let graph = policy
            .derive_demand_graph(
                &candidates,
                &format!("sha256:{:064x}", 1),
                &format!("sha256:{:064x}", 2),
            )
            .expect("demand graph");

        preflight_export_value_schedule_compatibility(&graph)
            .expect("runtime-bound function subjects are schedulable");
        let refusal = TypeFactsCertificationSchedule::new_export_values(&graph, [])
            .expect_err("the authority schedule still requires every exact demand");
        assert!(matches!(
            refusal,
            TypeFactsCertificationError::MissingDemand(demand) if demand.starts_with("sha256:")
        ));
    }

    #[test]
    fn declaration_harness_uses_runtime_spelling_for_declaration_substitution() {
        assert_eq!(declaration_import_path("dist/index.d.ts"), "dist/index.js");
        assert_eq!(
            declaration_import_path("dist/index.d.mts"),
            "dist/index.mjs"
        );
        assert_eq!(
            declaration_import_path("dist/index.d.cts"),
            "dist/index.cjs"
        );
        assert_eq!(declaration_import_path("src/index.tsx"), "src/index.tsx");
    }

    #[test]
    fn materialized_store_entries_hold_exactly_the_loadable_files_and_are_repaired_when_wrong() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-materialized-store-test-{}-{}",
            std::process::id(),
            PRIVATE_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let store = root.join("store");
        let snapshot = source_snapshot(
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            &[
                ("package.json", b"{\"name\":\"source-package\"}".as_slice()),
                (
                    "dist/index.d.ts",
                    b"export declare const value: 1;".as_slice(),
                ),
                ("dist/index.js", b"export const value = 1;".as_slice()),
                ("dist/index.js.map", b"{}".as_slice()),
                ("README.md", b"# source".as_slice()),
            ],
        );
        let project = root.join("project");
        let target = project.join("node_modules/source-package");
        assert!(link_snapshot_from_store(&store, &snapshot, &target).unwrap());
        assert!(
            fs::symlink_metadata(&target)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let entry = fs::read_link(&target).unwrap();
        assert!(entry.starts_with(&store));
        assert_eq!(
            fs::read(target.join("dist/index.d.ts")).unwrap(),
            b"export declare const value: 1;"
        );
        assert!(
            !entry.join("dist/index.js.map").exists(),
            "source maps are not materialized"
        );
        assert!(
            !entry.join("README.md").exists(),
            "READMEs are not materialized"
        );
        let manifest: MaterializedStoreManifest =
            serde_json::from_slice(&fs::read(entry.join(MATERIALIZED_STORE_MANIFEST)).unwrap())
                .unwrap();
        assert_eq!(
            manifest
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["dist/index.d.ts", "dist/index.js", "package.json"]
        );

        // Linking the same snapshot to the same target again is idempotent.
        assert!(link_snapshot_from_store(&store, &snapshot, &target).unwrap());

        // A tampered entry is rebuilt from the snapshot before it is linked.
        fs::remove_file(entry.join("dist/index.d.ts")).unwrap();
        fs::write(
            entry.join("dist/index.d.ts"),
            b"export declare const value: 2;",
        )
        .unwrap();
        let other = root.join("project-two/node_modules/source-package");
        assert!(link_snapshot_from_store(&store, &snapshot, &other).unwrap());
        assert_eq!(
            fs::read_link(&other).unwrap(),
            entry,
            "the entry keeps its content address"
        );
        assert_eq!(
            fs::read(other.join("dist/index.d.ts")).unwrap(),
            b"export declare const value: 1;"
        );

        // A different snapshot at an occupied target is a collision, as before.
        let different = source_snapshot(
            "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            &[("package.json", b"{}".as_slice())],
        );
        assert!(matches!(
            link_snapshot_from_store(&store, &different, &target),
            Err(TypeFactsCertificationError::SourceCensus(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn type_facts_program_can_load_names_exactly_typescripts_input_extensions() {
        for loadable in [
            "package.json",
            "dist/index.js",
            "dist/index.mjs",
            "dist/index.cjs",
            "dist/index.jsx",
            "dist/index.d.ts",
            "dist/index.d.mts",
            "dist/index.d.cts",
            "src/index.ts",
            "src/index.tsx",
            "src/index.mts",
            "src/index.cts",
            "data/schema.json",
        ] {
            assert!(type_facts_program_can_load(loadable), "{loadable}");
        }
        for unloadable in [
            "dist/index.js.map",
            "dist/index.d.ts.map",
            "README.md",
            "LICENSE",
            "dist/styles.css",
            "dist/logo.svg",
            "dist/font.woff2",
            "dist/index.JS",
            "bin/cli",
        ] {
            assert!(!type_facts_program_can_load(unloadable), "{unloadable}");
        }
    }

    #[test]
    fn execution_image_copies_are_private_and_byte_identical() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-typefacts-clone-test-{}-{}",
            std::process::id(),
            PRIVATE_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.bin");
        let destination = root.join("private.bin");
        let bytes: Vec<u8> = (0..(3 * 1024 * 1024u32))
            .map(|index| (index % 251) as u8)
            .collect();
        fs::write(&source, &bytes).unwrap();
        let expected = hash_file(&source).unwrap();
        assert_eq!(copy_and_hash(&source, &destination).unwrap(), expected);
        assert_eq!(fs::read(&destination).unwrap(), bytes);
        // The private copy shares no future with its source.
        fs::write(&source, b"replaced after the copy").unwrap();
        assert_eq!(hash_file(&destination).unwrap(), expected);
        assert!(
            copy_and_hash(&source, &destination).is_err(),
            "an existing destination is refused, never overwritten"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn private_project_materialization_deduplicates_only_identical_snapshot_files() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-typefacts-materialize-test-{}-{}",
            std::process::id(),
            PRIVATE_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let target = root.join("node_modules/shared/index.d.ts");
        write_immutable_project_file(&target, b"export declare const value: 1;").unwrap();
        write_immutable_project_file(&target, b"export declare const value: 1;").unwrap();
        assert!(matches!(
            write_immutable_project_file(&target, b"export declare const value: 2;"),
            Err(TypeFactsCertificationError::SourceCensus(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    // A witness program's roots are declaration modules, so a runtime chunk
    // that only a re-export names never becomes a program member and its
    // implementation transcript comes back open with `sourceUnavailable`. The
    // closure's runtime-axis entries are what repairs that; declarations,
    // manifests, and non-module resolution inputs are not program roots, and a
    // path the materialized snapshot does not carry must never be listed.
    #[test]
    fn private_project_program_roots_select_materialized_runtime_closure_modules() {
        use crate::contract_interface::{ClosureEntry, ClosureFileRole};

        let entry = |role, path: &str| ClosureEntry {
            role,
            path: format!("./{path}"),
            digest: digest(path.as_bytes()),
            transform_digest: None,
        };
        let entries = vec![
            entry(ClosureFileRole::Manifest, "package.json"),
            entry(ClosureFileRole::ResolutionInput, "dist/data.json"),
            entry(ClosureFileRole::Runtime, "dist/index.js"),
            entry(
                ClosureFileRole::Runtime,
                "dist/create/controllableSignal.js",
            ),
            entry(ClosureFileRole::LiteralDynamicChunk, "dist/lazy/panel.js"),
            entry(ClosureFileRole::Declaration, "dist/index.d.ts"),
            entry(ClosureFileRole::Runtime, "dist/absent.js"),
        ];

        let materialized = |path: &str| path != "dist/absent.js";
        assert_eq!(
            closure_runtime_module_paths(&entries, &materialized),
            vec![
                "dist/index.js",
                "dist/create/controllableSignal.js",
                "dist/lazy/panel.js",
            ],
            "only runtime-axis closure modules the snapshot carries are program roots"
        );

        // Nothing at all is listed when the snapshot carries none of them, so a
        // demand whose module is genuinely absent stays open rather than
        // pointing the producer at a file outside the authenticated bytes.
        assert!(closure_runtime_module_paths(&entries, &|_| false).is_empty());
    }

    #[test]
    fn private_project_preserves_hoisted_and_nested_installed_package_locations() {
        let project = Path::new("/private-project");
        let projected_owner = project.join("node_modules/corvu");
        let original_owner = Path::new("/install/node_modules/corvu");
        assert_eq!(
            private_project_package_target(
                project,
                &projected_owner,
                original_owner,
                Path::new("/install/node_modules/@corvu/utils"),
                "@corvu/utils",
            ),
            project.join("node_modules/@corvu/utils")
        );
        assert_eq!(
            private_project_package_target(
                project,
                &projected_owner,
                original_owner,
                Path::new("/install/node_modules/@corvu/accordion/node_modules/@corvu/utils"),
                "@corvu/utils",
            ),
            project.join("node_modules/@corvu/accordion/node_modules/@corvu/utils")
        );
        assert_eq!(
            private_project_package_target(
                project,
                &projected_owner,
                original_owner,
                Path::new("/install/node_modules/corvu/node_modules/local-child"),
                "local-child",
            ),
            projected_owner.join("node_modules/local-child")
        );
    }

    #[test]
    fn private_image_requires_exact_bytes_and_source_manifest() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-typefacts-pin-test-{}-{}",
            std::process::id(),
            EXECUTION_IMAGE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let executable = root.join("producer");
        fs::write(&executable, b"exact producer bytes").unwrap();
        let source_digest = digest(b"source manifest");
        fs::write(
            PathBuf::from(format!("{}.buildinfo", executable.display())),
            format!(
                "{{\"format\":1,\"sourceDigest\":\"{}\",\"toolchain\":\"test\",\"buildId\":\"{}\"}}",
                source_digest.strip_prefix("sha256:").unwrap(),
                typefacts::v3::TYPE_FACTS_BUILD_ID
            ),
        )
        .unwrap();
        let pin = TypeFactsProducerPin::new(
            executable.clone(),
            digest(b"exact producer bytes"),
            source_digest.clone(),
        )
        .unwrap();
        let image = PrivateExecutionImage::copy_from_pin(&pin).unwrap();
        assert_ne!(image.path(), executable);
        assert_eq!(fs::read(image.path()).unwrap(), b"exact producer bytes");

        let wrong_bytes = TypeFactsProducerPin::new(
            executable.clone(),
            digest(b"different bytes"),
            source_digest.clone(),
        )
        .unwrap();
        assert!(matches!(
            PrivateExecutionImage::copy_from_pin(&wrong_bytes),
            Err(TypeFactsCertificationError::ProducerProvenance(_))
        ));

        let wrong_source = TypeFactsProducerPin::new(
            executable.clone(),
            digest(b"exact producer bytes"),
            digest(b"different source"),
        )
        .unwrap();
        assert!(matches!(
            PrivateExecutionImage::copy_from_pin(&wrong_source),
            Err(TypeFactsCertificationError::ProducerProvenance(_))
        ));
        drop(image);
        fs::remove_file(PathBuf::from(format!("{}.buildinfo", executable.display()))).unwrap();
        fs::remove_file(executable).unwrap();
        fs::remove_dir(root).unwrap();
    }

    fn transcript() -> InvocationTranscript {
        serde_json::from_value(json!({
            "location": {"path": "/project/certification.ts", "startByte": 10, "endByte": 20},
            "validity": "valid",
            "kind": "call",
            "target": "symbol:run",
            "selectedSignature": {
                "identity": format!("sha256:{:064x}", 7),
                "declaration": {
                    "symbol": "symbol:run",
                    "name": "run",
                    "kind": "function",
                    "location": {"path": "/project/node_modules/pkg/index.d.ts", "startByte": 0, "endByte": 30},
                    "originModule": "pkg",
                    "sourceFile": "/project/node_modules/pkg/index.d.ts"
                },
                "overloadOrdinal": 2,
                "overloadCount": 3,
                "minimumArgumentCount": 1,
                "parameters": [{
                    "index": 0,
                    "symbol": "parameter:options",
                    "value": {
                        "type": {"text": "Options", "originModule": "pkg"},
                        "callability": "nonCallable",
                        "constructability": "nonConstructable",
                        "primitive": {"mayBeObject": true},
                        "alternatives": [{"index": 0}],
                        "partitions": [{"axis": "discriminant", "complete": true, "cases": [{"kind": "options"}]}]
                    },
                    "callablePaths": [{
                        "alternative": 0,
                        "path": [{"kind": "property", "property": "callback"}],
                        "presence": "required",
                        "callability": "callable",
                        "constructability": "nonConstructable",
                        "complete": true,
                        "subtreeEnumerated": true
                    }]
                }],
                "result": {
                    "type": {"text": "void"},
                    "callability": "nonCallable",
                    "constructability": "nonConstructable",
                    "primitive": {},
                    "alternatives": [{"index": 0}]
                }
            },
            "bindings": [{
                "argumentIndex": 0,
                "location": {"path": "/project/certification.ts", "startByte": 14, "endByte": 19},
                "disposition": "direct",
                "slots": [{"expandedIndex": 0, "parameterIndex": 0}]
            }],
            "controlFlow": {
                "returns": [{
                    "location": {"path": "/project/node_modules/pkg/index.js", "startByte": 1, "endByte": 2},
                    "reach": "reachable"
                }],
                "branches": [{
                    "location": {"path": "/project/node_modules/pkg/index.js", "startByte": 3, "endByte": 4},
                    "reach": "reachable",
                    "partitions": [{"axis": "literal", "complete": true, "cases": [{"kind": "true"}]}]
                }]
            },
            "complete": ["signature", "bindings", "omissions", "parameters", "result", "uses", "controlFlow"]
        }))
        .unwrap()
    }

    fn proof(family: ProofFamily, subject: ProofDemandSubject) -> ScheduledProofDemand {
        ScheduledProofDemand {
            id: format!("sha256:{:064x}", family as u8 + 1),
            family,
            subject,
        }
    }

    fn selected_subject() -> ProofDemandSubject {
        ProofDemandSubject::PositiveFact(PositiveFactSubject::SelectedCall {
            artifact_case: "browser".into(),
            export: "run".into(),
        })
    }

    fn operation(
        id: &str,
        kind: OperationKind,
        cardinality: solid_reactive_ir::contract_semantics::Cardinality,
    ) -> solid_reactive_ir::contract_semantics::Operation {
        solid_reactive_ir::contract_semantics::Operation {
            id: solid_reactive_ir::contract_semantics::OperationId(id.into()),
            kind,
            guard: None,
            trigger: None,
            at: None,
            schedule: None,
            tracking: solid_reactive_ir::contract_semantics::Tracking::Unknown,
            owner: solid_reactive_ir::contract_semantics::OwnerRelation::default(),
            cardinality,
            inputs: Vec::new(),
            output: None,
            resources: std::collections::BTreeSet::new(),
        }
    }

    fn per_call_cardinality(
        min: Option<u32>,
    ) -> solid_reactive_ir::contract_semantics::Cardinality {
        solid_reactive_ir::contract_semantics::Cardinality {
            scope: Some(CardinalityScope::Call),
            min,
            max: Some(UpperBound::Many),
        }
    }

    fn parameter_source_at(index: u16, path: &[&str]) -> ValueSource {
        ValueSource::Parameter {
            index,
            path: path.iter().map(|segment| (*segment).to_string()).collect(),
        }
    }

    // A demand's own bound decides how much execution evidence its witness owes.
    // "May run zero or more times per call" is exactly what a loop body is, so a
    // call the producer reports as Unknown discharges it; an operation that
    // asserts an occurrence is owed a call the implementation provably reaches.
    // Neither reading admits code that never runs.
    #[test]
    fn reachability_floor_follows_the_bound_operation_lower_bound() {
        assert!(reach_admits_zero_lower_bound(Reachability::Reachable));
        assert!(reach_admits_zero_lower_bound(Reachability::Unknown));
        assert!(!reach_admits_zero_lower_bound(Reachability::Unreachable));

        assert!(ReachabilityFloor::Reachable.admits(Reachability::Reachable));
        assert!(!ReachabilityFloor::Reachable.admits(Reachability::Unknown));
        assert!(!ReachabilityFloor::Reachable.admits(Reachability::Unreachable));
        assert!(ReachabilityFloor::MayExecute.admits(Reachability::Reachable));
        assert!(ReachabilityFloor::MayExecute.admits(Reachability::Unknown));
        assert!(!ReachabilityFloor::MayExecute.admits(Reachability::Unreachable));

        // Relaxing takes positive evidence, and `min: Some(0)` is the whole of
        // it. This is the trap of this tier: an operation carrying *no*
        // cardinality looks like the same relaxation and is not — it stated no
        // bound at all, so there is nothing here to read as "zero or more".
        assert_eq!(
            operation_reachability_floor(&operation(
                "invoke-0",
                OperationKind::Invoke,
                per_call_cardinality(Some(0))
            )),
            ReachabilityFloor::MayExecute,
            "an explicit zero lower bound accepts a call that may execute"
        );
        for cardinality in [
            solid_reactive_ir::contract_semantics::Cardinality::default(),
            per_call_cardinality(None),
        ] {
            assert_eq!(
                operation_reachability_floor(&operation(
                    "invoke-0",
                    OperationKind::Invoke,
                    cardinality.clone()
                )),
                ReachabilityFloor::Reachable,
                "silence about a bound is not a claim of zero: {cardinality:?}"
            );
        }
        for min in [1_u32, 2, 7] {
            assert_eq!(
                operation_reachability_floor(&operation(
                    "invoke-0",
                    OperationKind::Invoke,
                    per_call_cardinality(Some(min))
                )),
                ReachabilityFloor::Reachable,
                "a claim asserting an occurrence keeps the strict floor: min {min}"
            );
        }
    }

    // `callback_reachability_floor` is the floor `argument-binding` and
    // `callable-path` take — the two families that assert no occurrence of their
    // own and therefore read the bound of the operation their callback is bound
    // to. Every arm of that lookup fails closed, and each fail-closed default is
    // pinned here: a mutation flipping either one to `MayExecute` would
    // otherwise pass the whole suite.
    #[test]
    fn callback_floor_reads_the_bound_operation_and_fails_closed_everywhere_else() {
        let read = operation("read-0", OperationKind::Read, per_call_cardinality(Some(0)));
        let export = export_semantics(
            vec![CallbackInvocation {
                from: parameter_source_at(0, &[]),
                operation: read.id.clone(),
            }],
            vec![read.clone()],
        );
        let bound = proof(
            ProofFamily::ArgumentBinding,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::CallbackBinding {
                artifact_case: "browser".into(),
                export: "run".into(),
                ordinal: 0,
                operation: "read-0".into(),
            }),
        );
        assert_eq!(
            callback_reachability_floor(&export, &bound),
            ReachabilityFloor::MayExecute,
            "the bound operation's explicit zero lower bound is the floor"
        );

        // The bound operation's own bound decides, not the callback's position.
        let guaranteed = export_semantics(
            vec![CallbackInvocation {
                from: parameter_source_at(0, &[]),
                operation: read.id.clone(),
            }],
            vec![operation(
                "read-0",
                OperationKind::Read,
                per_call_cardinality(Some(1)),
            )],
        );
        assert_eq!(
            callback_reachability_floor(&guaranteed, &bound),
            ReachabilityFloor::Reachable
        );

        // Fail-closed default 1: a subject that is not a callback binding has no
        // bound operation to read at all.
        assert_eq!(
            callback_reachability_floor(
                &export,
                &proof(ProofFamily::ArgumentBinding, selected_subject())
            ),
            ReachabilityFloor::Reachable,
            "a non-callback subject names no operation whose bound could relax the floor"
        );

        // Fail-closed default 2, three ways to miss: an ordinal past the end,
        // an ordinal naming a callback bound to a different operation, and a
        // callback whose operation the export does not carry.
        let out_of_range = proof(
            ProofFamily::ArgumentBinding,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::CallbackBinding {
                artifact_case: "browser".into(),
                export: "run".into(),
                ordinal: 7,
                operation: "read-0".into(),
            }),
        );
        assert_eq!(
            callback_reachability_floor(&export, &out_of_range),
            ReachabilityFloor::Reachable,
            "an ordinal past the callback list reads no bound"
        );
        let mismatched = proof(
            ProofFamily::ArgumentBinding,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::CallbackBinding {
                artifact_case: "browser".into(),
                export: "run".into(),
                ordinal: 0,
                operation: "read-1".into(),
            }),
        );
        assert_eq!(
            callback_reachability_floor(&export, &mismatched),
            ReachabilityFloor::Reachable,
            "the demand and the callback must name the same operation"
        );
        let absent = export_semantics(
            vec![CallbackInvocation {
                from: parameter_source_at(0, &[]),
                operation: read.id.clone(),
            }],
            Vec::new(),
        );
        assert_eq!(
            callback_reachability_floor(&absent, &bound),
            ReachabilityFloor::Reachable,
            "an operation absent from the export has no bound whose lower end could be read"
        );
    }

    // The recursive value-shape branch keeps the strict floor on purpose: it
    // asserts what a position *is*, not how often the implementation reaches it.
    // Pinned here because nothing else exercises that decision — the mutation
    // that relaxes it passed the whole suite before this test existed.
    #[test]
    fn recursive_value_shape_evidence_keeps_the_strict_floor() {
        let implementation: typefacts::ExportImplementationTranscript =
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "calls": [{
                    "location": {"path": "/project/index.js", "startByte": 30, "endByte": 40},
                    "reach": "unknown",
                    "kind": "call",
                    "target": "symbol:callback",
                    "calleeParameter": {"parameterIndex": 0}
                }]
            }))
            .unwrap();
        let source = parameter_source_at(0, &[]);
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "recursive".into(),
            reason: reason.into(),
        };

        operation_input_value_shape_evidence(
            &source,
            RecursiveValueEvidence::ShapeAsserted,
            &implementation,
            &open,
            &mut Vec::new(),
        )
        .expect_err("a call the implementation may not reach does not fix what a position is");

        let mut reached = implementation.clone();
        reached.calls[0].reach = Reachability::Reachable;
        let mut sites = Vec::new();
        assert!(
            operation_input_value_shape_evidence(
                &source,
                RecursiveValueEvidence::ShapeAsserted,
                &reached,
                &open,
                &mut sites,
            )
            .expect("a reachable direct call of the parameter is the evidence this branch wants"),
            "the branch discharges the demand when it applies and the evidence is there"
        );
        assert_eq!(sites.len(), 1);

        // The unasserted-root arm keeps its own explicit `Reachable` premise,
        // and reports "not discharged" rather than an error so the signature
        // census still gets its turn.
        assert!(
            !operation_input_value_shape_evidence(
                &source,
                RecursiveValueEvidence::RootUnasserted,
                &implementation,
                &open,
                &mut Vec::new(),
            )
            .expect("an absent witness leaves the root to the signature census"),
        );
        let mut root_sites = Vec::new();
        assert!(
            operation_input_value_shape_evidence(
                &source,
                RecursiveValueEvidence::RootUnasserted,
                &reached,
                &open,
                &mut root_sites,
            )
            .expect("a reachable call witnesses the root"),
        );
        assert_eq!(root_sites.len(), 1);
    }

    fn export_semantics(
        callbacks: Vec<CallbackInvocation>,
        operations: Vec<solid_reactive_ir::contract_semantics::Operation>,
    ) -> solid_reactive_ir::contract_semantics::ExportSemantics {
        use solid_reactive_ir::contract_semantics::{
            ArtifactIdentity, CallClaims, CallSemantics, Digest, ExportIdentity, ExportSemantics,
            ExportTargetIdentity, GuardPartition, KnowledgeSet, StabilityKnowledge, ValueShape,
        };
        let module = ArtifactIdentity {
            path: "dist/index.js".into(),
            digest: Digest::parse(format!("sha256:{:064x}", 3)).expect("a valid digest"),
        };
        ExportSemantics {
            identity: ExportIdentity {
                entrypoint: ".".into(),
                public_name: "run".into(),
                runtime: ExportTargetIdentity {
                    module: module.clone(),
                    export_name: "run".into(),
                },
                declarations: ExportTargetIdentity {
                    module,
                    export_name: "run".into(),
                },
            },
            shape: ValueShape::Callable,
            stability: StabilityKnowledge::Unknown,
            call: CallSemantics::new(
                CallClaims {
                    callbacks: KnowledgeSet::complete(callbacks),
                    ..CallClaims::default()
                },
                operations,
                Vec::new(),
                Vec::new(),
                GuardPartition {
                    cases: KnowledgeSet::complete(Vec::new()),
                },
            ),
        }
    }

    // marker's `mapMatch(text)` sits in a `do … while`, so the producer reports
    // it Unknown. Its demand claims 0..many per call, and that claim is
    // discharged by a call that may run — captured inside the returned closure
    // or not. Dead code still discharges nothing.
    #[test]
    fn loop_body_call_executes_only_under_the_zero_lower_bound_floor() {
        let mut implementation: typefacts::ExportImplementationTranscript =
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "controlFlow": {"returns": [{
                    "location": {"path": "/project/index.js", "startByte": 42, "endByte": 52},
                    "reach": "reachable",
                    "carryReach": "reachable",
                    "carriedCallables": [
                        {"path": "/project/index.js", "startByte": 5, "endByte": 40}
                    ]
                }]},
                "calls": [{
                    "location": {"path": "/project/index.js", "startByte": 10, "endByte": 15},
                    "reach": "unknown",
                    "kind": "call",
                    "calleeParameter": {"parameterIndex": 0},
                    "captured": true,
                    "enclosingCallable":
                        {"path": "/project/index.js", "startByte": 5, "endByte": 40}
                }]
            }))
            .unwrap();
        assert!(implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::MayExecute
        ));
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));

        // The carried chain is still required under the relaxed floor: a
        // loop-body call in a closure nothing returns remains unexecuted. The
        // link is the enclosing callable's identity, not the call's own bytes,
        // so this moves the closure rather than the call site.
        let carried = implementation.calls[0]
            .enclosing_callable
            .clone()
            .expect("the fixture states one");
        if let Some(enclosing) = implementation.calls[0].enclosing_callable.as_mut() {
            enclosing.start_byte = 60;
            enclosing.end_byte = 65;
        }
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::MayExecute
        ));
        implementation.calls[0].enclosing_callable = Some(carried);

        implementation.calls[0].captured = false;
        assert!(implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::MayExecute
        ));
        implementation.calls[0].reach = Reachability::Unreachable;
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::MayExecute
        ));
    }

    // `@tanstack/hotkeys::formatHotkey` reads its parameter only from inside a
    // `for … of`. The read is Unknown by construction, and its demand claims no
    // lower bound.
    #[test]
    fn parameter_read_accepts_a_loop_body_call_only_for_a_zero_lower_bound() {
        let implementation: typefacts::ExportImplementationTranscript =
            serde_json::from_value(json!({
                "location": {"path": "/project/format.js", "startByte": 0, "endByte": 4},
                "calls": [{
                    "location": {"path": "/project/format.js", "startByte": 30, "endByte": 60},
                    "reach": "unknown",
                    "kind": "call",
                    "calleeParameter": {
                        "parameterIndex": 0,
                        "path": [
                            {"kind": "property", "property": "modifiers"},
                            {"kind": "property", "property": "includes"}
                        ]
                    }
                }]
            }))
            .unwrap();
        let source = parameter_source_at(0, &[]);
        let open = |reason: &str| TypeFactsCertificationError::UnsupportedDemand {
            demand: "read".into(),
            reason: reason.into(),
        };

        let mut sites = Vec::new();
        require_parameter_read_evidence(
            &implementation,
            &source,
            ReachabilityFloor::MayExecute,
            &open,
            &mut sites,
        )
        .expect("a may-execute read discharges a claim with no lower bound");
        assert_eq!(sites, vec!["implementation-read:/project/format.js:30:60"]);

        let mut sites = Vec::new();
        require_parameter_read_evidence(
            &implementation,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut sites,
        )
        .expect_err("an occurrence claim is not discharged by a loop-body read");
    }

    // `MapEntries` reads `props` at `const mapFn = props.children` — a property
    // access, not a call. The use census records exactly that, and the read
    // branch has to consume it; the call census by construction never will.
    #[test]
    fn parameter_read_accepts_an_uncaptured_property_access_use() {
        let mut implementation: typefacts::ExportImplementationTranscript =
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "parameterUses": [{
                    "parameterIndex": 0,
                    "location": {"path": "/project/index.js", "startByte": 20, "endByte": 25},
                    "reach": "reachable",
                    "kind": "propertyAccess"
                }]
            }))
            .unwrap();
        let source = parameter_source_at(0, &[]);
        let open = |reason: &str| TypeFactsCertificationError::UnsupportedDemand {
            demand: "read".into(),
            reason: reason.into(),
        };

        let mut sites = Vec::new();
        require_parameter_read_evidence(
            &implementation,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut sites,
        )
        .expect("an uncaptured property access is a read");
        assert_eq!(
            sites,
            vec!["implementation-read-use:/project/index.js:20:25:propertyAccess"]
        );

        // A direct or alias call of the parameter reads it too.
        for kind in [ParameterUseKind::DirectCall, ParameterUseKind::AliasCall] {
            implementation.parameter_uses[0].kind = kind;
            let mut sites = Vec::new();
            require_parameter_read_evidence(
                &implementation,
                &source,
                ReachabilityFloor::Reachable,
                &open,
                &mut sites,
            )
            .unwrap_or_else(|_| panic!("{kind:?} is a read of the parameter"));
        }

        // Being handed somewhere is not being read, and a use the census could
        // not classify is evidence of nothing at all.
        for kind in [
            ParameterUseKind::Storage,
            ParameterUseKind::Return,
            ParameterUseKind::ArgumentKnown,
            ParameterUseKind::ArgumentUnknown,
            ParameterUseKind::Capture,
            ParameterUseKind::UnknownEscape,
        ] {
            implementation.parameter_uses[0].kind = kind;
            let mut sites = Vec::new();
            require_parameter_read_evidence(
                &implementation,
                &source,
                ReachabilityFloor::Reachable,
                &open,
                &mut sites,
            )
            .expect_err(&format!("{kind:?} is not a read"));
        }

        // A captured use is refused for the reason a captured call is: nothing
        // here proves the closure holding it runs.
        implementation.parameter_uses[0].kind = ParameterUseKind::PropertyAccess;
        implementation.parameter_uses[0].captured = true;
        let mut sites = Vec::new();
        require_parameter_read_evidence(
            &implementation,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut sites,
        )
        .expect_err("a captured property access proves no read of the export's own call");

        // Another parameter, and a path the demand does not name, answer
        // nothing. A deeper binding path does: destructuring `{ children }` off
        // parameter 0 and using it is a use of parameter 0.
        implementation.parameter_uses[0].captured = false;
        require_parameter_read_evidence(
            &implementation,
            &parameter_source_at(1, &[]),
            ReachabilityFloor::Reachable,
            &open,
            &mut Vec::new(),
        )
        .expect_err("a use of parameter 0 is not a read of parameter 1");
        require_parameter_read_evidence(
            &implementation,
            &parameter_source_at(0, &["children"]),
            ReachabilityFloor::Reachable,
            &open,
            &mut Vec::new(),
        )
        .expect_err("a use of the parameter root does not name the demanded property");
        implementation.parameter_uses[0].binding_path = vec![typefacts::PathSegment {
            kind: PathSegmentKind::Property,
            property: "children".into(),
            index: None,
        }];
        require_parameter_read_evidence(
            &implementation,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut Vec::new(),
        )
        .expect("using a destructured property of the parameter reads the parameter");
    }

    // The hole this closed: the use witness consulted `captured`, `kind`, and
    // the path, and nothing about where the use sits. A `props.children` after a
    // `return`, after a `throw`, or in a branch a literal condition excludes is
    // `Unreachable` and discharged a `min >= 1` read demand, which is exactly
    // what the call witness has always refused. Each row is a producer shape the
    // review reproduced against the real census.
    #[test]
    fn parameter_read_use_answers_to_the_same_floor_as_a_call() {
        let source = parameter_source_at(0, &[]);
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "read".into(),
            reason: reason.into(),
        };
        let use_at = |reach: &str| -> typefacts::ExportImplementationTranscript {
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "parameterUses": [{
                    "parameterIndex": 0,
                    "location": {"path": "/project/index.js", "startByte": 20, "endByte": 25},
                    "reach": reach,
                    "kind": "propertyAccess"
                }]
            }))
            .unwrap()
        };

        // `void props.children;` after a `return` or a `throw`, and the same in
        // an `if (false)` branch: the producer answers `unreachable` for all
        // three, and neither floor admits dead code.
        for floor in [ReachabilityFloor::Reachable, ReachabilityFloor::MayExecute] {
            require_parameter_read_evidence(
                &use_at("unreachable"),
                &source,
                floor,
                &open,
                &mut Vec::new(),
            )
            .expect_err("a property access in dead code reads nothing, for any claim");
        }

        // In a loop body the producer answers `unknown`: that discharges a read
        // whose own lower bound is zero, and never one asserting an occurrence.
        let loop_body = use_at("unknown");
        let mut sites = Vec::new();
        require_parameter_read_evidence(
            &loop_body,
            &source,
            operation_reachability_floor(&operation(
                "read-0",
                OperationKind::Read,
                per_call_cardinality(Some(0)),
            )),
            &open,
            &mut sites,
        )
        .expect("a use that may execute discharges a claim with no lower bound");
        assert_eq!(
            sites,
            vec!["implementation-read-use:/project/index.js:20:25:propertyAccess"]
        );
        require_parameter_read_evidence(
            &loop_body,
            &source,
            operation_reachability_floor(&operation(
                "read-0",
                OperationKind::Read,
                per_call_cardinality(Some(1)),
            )),
            &open,
            &mut Vec::new(),
        )
        .expect_err("a use in a loop body does not witness an asserted occurrence");

        // And the witness still works where it should: a reachable use answers
        // the strict floor, so the gate is not simply refusing every use.
        require_parameter_read_evidence(
            &use_at("reachable"),
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut Vec::new(),
        )
        .expect("a reachable property access is a read under either floor");
    }

    // The owner branch asks the same reachability question the flow branches do,
    // and takes the same answer from the demand's bound: `createRenderEffect`
    // after a `for … of` witnesses a 0..many create claim.
    #[test]
    fn owner_operation_call_follows_the_demanded_lower_bound() {
        let implementation: typefacts::ExportImplementationTranscript =
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "calls": [{
                    "location": {"path": "/project/index.js", "startByte": 30, "endByte": 60},
                    "reach": "unknown",
                    "kind": "call",
                    "target": "symbol:createRenderEffect",
                    "targetName": "createRenderEffect",
                    "targetModule": "solid-js"
                }]
            }))
            .unwrap();
        let mut effect = operation(
            "create-0",
            OperationKind::Create,
            per_call_cardinality(Some(0)),
        );
        effect.owner.requirements.child_owners = Requirement::Required;
        let demand = proof(ProofFamily::OperationReachability, selected_subject());
        let open = |reason: &str| TypeFactsCertificationError::UnsupportedDemand {
            demand: "create".into(),
            reason: reason.into(),
        };

        let mut sites = Vec::new();
        require_owner_operation_call(
            &effect,
            &demand,
            &implementation,
            operation_reachability_floor(&effect),
            &open,
            &mut sites,
        )
        .expect("a may-execute dialect owner call witnesses a claim with no lower bound");
        assert_eq!(
            sites,
            vec!["implementation-owner-call:/project/index.js:30:60:createRenderEffect"]
        );

        let mut guaranteed = effect.clone();
        guaranteed.cardinality = per_call_cardinality(Some(1));
        require_owner_operation_call(
            &guaranteed,
            &demand,
            &implementation,
            operation_reachability_floor(&guaranteed),
            &open,
            &mut Vec::new(),
        )
        .expect_err("an occurrence claim still demands a provably reached owner call");

        // Neither floor admits a call the implementation never reaches, and the
        // module gate is untouched: a same-named local is not the dialect's.
        let mut dead = implementation.clone();
        dead.calls[0].reach = Reachability::Unreachable;
        require_owner_operation_call(
            &effect,
            &demand,
            &dead,
            ReachabilityFloor::MayExecute,
            &open,
            &mut Vec::new(),
        )
        .expect_err("dead code witnesses nothing for any claim");
        let mut local = implementation.clone();
        local.calls[0].target_module = "".into();
        require_owner_operation_call(
            &effect,
            &demand,
            &local,
            ReachabilityFloor::MayExecute,
            &open,
            &mut Vec::new(),
        )
        .expect_err("a call outside the dialect module is not a dialect primitive call");
    }

    #[test]
    fn implementation_flow_requires_direct_or_returned_closure_execution() {
        // The call at bytes 10..15 sits inside the callable at 5..40, which the
        // reachable return site carries.
        let mut implementation: typefacts::ExportImplementationTranscript =
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "controlFlow": {"returns": [{
                    "location": {"path": "/project/index.js", "startByte": 42, "endByte": 52},
                    "reach": "reachable",
                    "carryReach": "reachable",
                    "carriedCallables": [
                        {"path": "/project/index.js", "startByte": 5, "endByte": 40}
                    ]
                }]},
                "calls": [{
                    "location": {"path": "/project/index.js", "startByte": 10, "endByte": 15},
                    "reach": "reachable",
                    "kind": "call",
                    "calleeParameter": {"parameterIndex": 0},
                    "captured": true,
                    "enclosingCallable": {
                        "path": "/project/index.js", "startByte": 5, "endByte": 40
                    }
                }]
            }))
            .unwrap();
        assert!(implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));

        // A call in a closure the implementation never returns is enclosed by a
        // callable no return site carries, and no amount of what the returned
        // closure *mentions* changes which callable contains it.
        implementation.calls[0].enclosing_callable = Some(
            serde_json::from_value(json!({
                "path": "/project/index.js", "startByte": 60, "endByte": 70
            }))
            .unwrap(),
        );
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        // A closure *containing* the carried one is not the carried one either:
        // the match is the exact enclosing callable, never a range that spans it.
        implementation.calls[0].enclosing_callable = Some(
            serde_json::from_value(json!({
                "path": "/project/index.js", "startByte": 4, "endByte": 41
            }))
            .unwrap(),
        );
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        // Same bytes, another file: identity is per file, never a suffix.
        implementation.calls[0].enclosing_callable = Some(
            serde_json::from_value(json!({
                "path": "/project/other.js", "startByte": 5, "endByte": 40
            }))
            .unwrap(),
        );
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        // A captured call the producer did not place inside any callable has no
        // chain to build, so it is refused rather than assumed.
        implementation.calls[0].enclosing_callable = None;
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        implementation.calls[0].enclosing_callable = Some(
            serde_json::from_value(json!({
                "path": "/project/index.js", "startByte": 5, "endByte": 40
            }))
            .unwrap(),
        );

        implementation.control_flow.as_mut().unwrap().returns[0]
            .carried_callables
            .clear();
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        // An unreachable return carries no authority even when it does carry
        // the callable.
        {
            let returns = &mut implementation.control_flow.as_mut().unwrap().returns;
            returns[0].carried_callables = vec![
                serde_json::from_value(
                    json!({"path": "/project/index.js", "startByte": 5, "endByte": 40}),
                )
                .unwrap(),
            ];
            returns[0].reach = Reachability::Unknown;
        }
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));

        implementation.calls[0].captured = false;
        assert!(implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        implementation.calls[0].reach = Reachability::Unknown;
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
    }

    #[test]
    fn second_order_return_carry_composes_exact_callable_identities() {
        let mut implementation: typefacts::ExportImplementationTranscript =
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "controlFlow": {"returns": [{
                    "location": {"path": "/project/index.js", "startByte": 92, "endByte": 100},
                    "reach": "reachable",
                    "carryReach": "reachable",
                    "carriedCallables": [
                        {"path": "/project/index.js", "startByte": 5, "endByte": 90}
                    ]
                }]},
                "callableReturns": [{
                    "callable": {"path": "/project/index.js", "startByte": 5, "endByte": 90},
                    "returns": [{
                        "location": {"path": "/project/index.js", "startByte": 70, "endByte": 82},
                        "reach": "reachable",
                        "carryReach": "reachable",
                        "carriedCallables": [{
                            "location": {
                                "path": "/project/index.js", "startByte": 20, "endByte": 80
                            },
                            "reach": "reachable"
                        }]
                    }]
                }],
                "calls": [{
                    "location": {"path": "/project/index.js", "startByte": 30, "endByte": 40},
                    "reach": "reachable",
                    "kind": "call",
                    "calleeParameter": {"parameterIndex": 0},
                    "captured": true,
                    "enclosingCallable": {
                        "path": "/project/index.js", "startByte": 20, "endByte": 80
                    }
                }]
            }))
            .unwrap();
        assert!(implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));

        // Missing lower-bound strength is absence of authority, never an
        // implicit default. Pin both edges independently under both floors.
        implementation.callable_returns[0].returns[0].carry_reach = None;
        for floor in [ReachabilityFloor::Reachable, ReachabilityFloor::MayExecute] {
            assert!(!implementation_call_is_executed(
                &implementation,
                &implementation.calls[0],
                floor
            ));
        }
        implementation.callable_returns[0].returns[0].carry_reach = Some(Reachability::Reachable);
        implementation.control_flow.as_mut().unwrap().returns[0].carry_reach = None;
        for floor in [ReachabilityFloor::Reachable, ReachabilityFloor::MayExecute] {
            assert!(!implementation_call_is_executed(
                &implementation,
                &implementation.calls[0],
                floor
            ));
        }
        implementation.control_flow.as_mut().unwrap().returns[0].carry_reach =
            Some(Reachability::Reachable);

        // A return site that is not proven reachable cannot add the inner
        // callable to the execution graph.
        implementation.callable_returns[0].returns[0].reach = Reachability::Unknown;
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::MayExecute
        ));
        implementation.callable_returns[0].returns[0].reach = Reachability::Reachable;

        // A branch-controlled return statement is a may-only edge. This is
        // separate from the legacy optimistic site reach above.
        implementation.callable_returns[0].returns[0].carry_reach = Some(Reachability::Unknown);
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        assert!(implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::MayExecute
        ));
        implementation.callable_returns[0].returns[0].carry_reach = Some(Reachability::Reachable);

        // The implementation can likewise return the owner callable only on
        // a possible branch. That first execution edge answers to the same
        // floor as every nested edge.
        implementation.control_flow.as_mut().unwrap().returns[0].carry_reach =
            Some(Reachability::Unknown);
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        assert!(implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::MayExecute
        ));
        implementation.control_flow.as_mut().unwrap().returns[0].carry_reach =
            Some(Reachability::Reachable);

        // A conditional return-carry alternative can satisfy only a may-run
        // operation. It cannot witness a lower bound of one.
        implementation.callable_returns[0].returns[0].carried_callables[0].reach =
            Reachability::Unknown;
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        assert!(implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::MayExecute
        ));
        implementation.callable_returns[0].returns[0].carried_callables[0].reach =
            Reachability::Reachable;

        // The owner of the return must itself execute. Merely naming an edge
        // from a nested callable that the implementation never returns is not
        // enough.
        implementation.callable_returns[0].callable.start_byte = 101;
        implementation.callable_returns[0].callable.end_byte = 120;
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        implementation.callable_returns[0].callable.start_byte = 5;
        implementation.callable_returns[0].callable.end_byte = 90;

        implementation.callable_returns[0].callable.path = "/project/other.js".into();
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        implementation.callable_returns[0].callable.path = "/project/index.js".into();

        // Identity is exact across path and span. Byte containment does not
        // let a neighbouring or same-spanned callable discharge the edge.
        implementation.callable_returns[0].returns[0].carried_callables[0]
            .location
            .path = "/project/other.js".into();
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        implementation.callable_returns[0].returns[0].carried_callables[0]
            .location
            .path = "/project/index.js".into();
        implementation.callable_returns[0].returns[0].carried_callables[0]
            .location
            .start_byte = 21;
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
        implementation.callable_returns[0].returns[0].carried_callables[0]
            .location
            .start_byte = 20;
        implementation.callable_returns[0].returns.clear();
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0],
            ReachabilityFloor::Reachable
        ));
    }

    #[test]
    fn callable_return_recursion_is_transitive_bounded_and_cycle_safe() {
        let location = |index: usize| {
            json!({
                "path": "/project/index.js",
                "startByte": index * 2 + 10,
                "endByte": index * 2 + 11
            })
        };
        let chain = |edges: usize| -> typefacts::ExportImplementationTranscript {
            let callable_returns = (0..edges)
                .map(|index| {
                    json!({
                        "callable": location(index),
                        "returns": [{
                            "location": {
                                "path": "/project/index.js",
                                "startByte": 1000 + index * 2,
                                "endByte": 1001 + index * 2
                            },
                            "reach": "reachable",
                            "carryReach": "reachable",
                            "carriedCallables": [{
                                "location": location(index + 1),
                                "reach": "reachable"
                            }]
                        }]
                    })
                })
                .collect::<Vec<_>>();
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "controlFlow": {"returns": [{
                    "location": {"path": "/project/index.js", "startByte": 5, "endByte": 9},
                    "reach": "reachable",
                    "carryReach": "reachable",
                    "carriedCallables": [location(0)]
                }]},
                "callableReturns": callable_returns,
                "calls": [{
                    "location": {"path": "/project/index.js", "startByte": 2000, "endByte": 2010},
                    "reach": "reachable",
                    "kind": "call",
                    "captured": true,
                    "enclosingCallable": location(edges)
                }]
            }))
            .unwrap()
        };

        let within = chain(MAX_EXECUTION_PREMISE_DEPTH);
        assert!(implementation_call_is_executed(
            &within,
            &within.calls[0],
            ReachabilityFloor::Reachable
        ));
        let beyond = chain(MAX_EXECUTION_PREMISE_DEPTH + 1);
        assert!(!implementation_call_is_executed(
            &beyond,
            &beyond.calls[0],
            ReachabilityFloor::Reachable
        ));

        let cycle: typefacts::ExportImplementationTranscript = serde_json::from_value(json!({
            "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
            "controlFlow": {"returns": []},
            "callableReturns": [
                {
                    "callable": location(0),
                    "returns": [{
                        "location": {"path": "/project/index.js", "startByte": 100, "endByte": 101},
                        "reach": "reachable",
                        "carryReach": "reachable",
                        "carriedCallables": [{"location": location(1), "reach": "reachable"}]
                    }]
                },
                {
                    "callable": location(1),
                    "returns": [{
                        "location": {"path": "/project/index.js", "startByte": 102, "endByte": 103},
                        "reach": "reachable",
                        "carryReach": "reachable",
                        "carriedCallables": [{"location": location(0), "reach": "reachable"}]
                    }]
                }
            ],
            "calls": [{
                "location": {"path": "/project/index.js", "startByte": 200, "endByte": 210},
                "reach": "reachable",
                "kind": "call",
                "captured": true,
                "enclosingCallable": location(1)
            }]
        }))
        .unwrap();
        assert!(!implementation_call_is_executed(
            &cycle,
            &cycle.calls[0],
            ReachabilityFloor::Reachable
        ));
    }

    /// An implementation whose only structure is `outer(<arrow containing the
    /// inner call>)`. The arrow spans 20..80, the captured inner call sits at
    /// 30..40 and names that arrow as its enclosing callable, and nothing is
    /// returned at all — so the returned-closure premise can never fire and the
    /// verdict is decided purely by whether slot 0 of `outer` is proven
    /// invoking.
    fn invoking_argument_implementation(
        outer: serde_json::Value,
    ) -> typefacts::ExportImplementationTranscript {
        let mut outer_call = json!({
            "location": {"path": "/project/index.js", "startByte": 10, "endByte": 90},
            "reach": "reachable",
            "kind": "call",
            "argumentCallables": [{
                "argument": 0,
                "locations": [{"path": "/project/index.js", "startByte": 20, "endByte": 80}]
            }]
        });
        let (serde_json::Value::Object(fields), serde_json::Value::Object(overrides)) =
            (&mut outer_call, outer)
        else {
            unreachable!("both literals are objects");
        };
        fields.extend(overrides);
        serde_json::from_value(json!({
            "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
            "controlFlow": {"returns": []},
            "calls": [
                outer_call,
                {
                    "location": {"path": "/project/index.js", "startByte": 30, "endByte": 40},
                    "reach": "reachable",
                    "kind": "call",
                    "calleeParameter": {"parameterIndex": 0},
                    "captured": true,
                    "enclosingCallable": {
                        "path": "/project/index.js", "startByte": 20, "endByte": 80
                    }
                }
            ]
        }))
        .unwrap()
    }

    fn inner_call_is_executed(implementation: &typefacts::ExportImplementationTranscript) -> bool {
        implementation_call_is_executed(
            implementation,
            &implementation.calls[1],
            ReachabilityFloor::Reachable,
        )
    }

    /// Tier A. `@solid-primitives/autofocus` is the whole case: its body is
    /// `createEffect(() => { const el = ref(); … })` and it returns nothing, so
    /// the call on the parameter is executed because `createEffect` executes
    /// the closure it is handed — not because anything is handed back.
    #[test]
    fn dialect_callback_argument_executes_the_callable_it_carries() {
        let implementation = invoking_argument_implementation(json!({
            "target": "symbol:createEffect",
            "targetName": "createEffect",
            "targetModule": "solid-js",
            "argumentParameters": [null]
        }));
        assert!(inner_call_is_executed(&implementation));

        // `createEffect(fn, initialValue)` at slot 1 is the initial value, not a
        // callback, and the dialect gate already says so.
        let slot_one = invoking_argument_implementation(json!({
            "target": "symbol:createEffect",
            "targetName": "createEffect",
            "targetModule": "solid-js",
            "argumentParameters": [null, null],
            "argumentCallables": [{
                "argument": 1,
                "locations": [{"path": "/project/index.js", "startByte": 20, "endByte": 80}]
            }]
        }));
        assert!(!inner_call_is_executed(&slot_one));

        // A locally defined `createMemo` shadowing the import is a different
        // function with the same name. The module gate is what refuses it.
        let shadowed = invoking_argument_implementation(json!({
            "target": "symbol:localCreateMemo",
            "targetName": "createMemo",
            "targetModule": "",
            "argumentParameters": [null]
        }));
        assert!(!inner_call_is_executed(&shadowed));

        // A namespace member the producer could not resolve to an exact import
        // carries neither target nor module.
        let namespace = invoking_argument_implementation(json!({
            "targetName": "createMemo",
            "argumentParameters": [null]
        }));
        assert!(!inner_call_is_executed(&namespace));

        // And an argument slot that carries nothing proven is not an invoking
        // position no matter what the callee is.
        let uncarried = invoking_argument_implementation(json!({
            "target": "symbol:createEffect",
            "targetName": "createEffect",
            "targetModule": "solid-js",
            "argumentParameters": [null],
            "argumentCallables": []
        }));
        assert!(!inner_call_is_executed(&uncarried));
    }

    /// Tier B. The verifier owns the invoker table too, so a member it does not
    /// recognize and a slot its own table does not list are both refused even
    /// though the producer transmitted them.
    #[test]
    fn default_library_invoker_is_a_closed_table_on_both_sides() {
        let listener = invoking_argument_implementation(json!({
            "target": "symbol:addEventListener",
            "targetName": "addEventListener",
            "defaultLibraryInvoker": "addEventListener",
            "invokedArguments": [1],
            "argumentParameters": [null, null],
            "argumentCallables": [{
                "argument": 1,
                "locations": [{"path": "/project/index.js", "startByte": 20, "endByte": 80}]
            }]
        }));
        assert!(inner_call_is_executed(&listener));

        // `requestAnimationFrame` invokes slot 0 — the `utils::afterPaint` case.
        let frame = invoking_argument_implementation(json!({
            "target": "symbol:requestAnimationFrame",
            "targetName": "requestAnimationFrame",
            "defaultLibraryInvoker": "requestAnimationFrame",
            "invokedArguments": [0],
            "argumentParameters": [null]
        }));
        assert!(inner_call_is_executed(&frame));

        // An invoker string nobody reviewed is not evidence. `watchPosition`
        // really does invoke its callback; it is not on the table, so it stays
        // open, and the same refusal covers a renamed or forged value.
        let unreviewed = invoking_argument_implementation(json!({
            "target": "symbol:watchPosition",
            "targetName": "watchPosition",
            "defaultLibraryInvoker": "watchPosition",
            "invokedArguments": [0],
            "argumentParameters": [null]
        }));
        assert!(!inner_call_is_executed(&unreviewed));

        // A recognized member with a slot its own runtime does not invoke. The
        // transmitted list says 0; `addEventListener` invokes 1, and the
        // verifier's table is what decides.
        let widened = invoking_argument_implementation(json!({
            "target": "symbol:addEventListener",
            "targetName": "addEventListener",
            "defaultLibraryInvoker": "addEventListener",
            "invokedArguments": [0],
            "argumentParameters": [null]
        }));
        assert!(!inner_call_is_executed(&widened));

        // A recognized member whose transmitted list omits the slot. Both
        // halves must agree.
        let omitted = invoking_argument_implementation(json!({
            "target": "symbol:requestAnimationFrame",
            "targetName": "requestAnimationFrame",
            "defaultLibraryInvoker": "requestAnimationFrame",
            "invokedArguments": [],
            "argumentParameters": [null]
        }));
        assert!(!inner_call_is_executed(&omitted));
    }

    /// Tier C. `@solidjs/signals::createRevealOrder` runs its callback inside
    /// `runWithOwner(owner, () => { const value = fn(); … })`, and
    /// `runWithOwner` is package-local — no dialect module, no library symbol.
    /// The producer's fact about that callee's own body is the whole premise.
    #[test]
    fn package_local_invoking_callee_executes_the_callable_it_carries() {
        let local = invoking_argument_implementation(json!({
            "target": "symbol:runWithOwner",
            "targetName": "runWithOwner",
            "argumentParameters": [null],
            "calleeInvokedParameters": [0],
            "calleeStronglyInvokedParameters": [0],
            "calleeDirectlyCalledParameters": [0]
        }));
        assert!(inner_call_is_executed(&local));

        // `function store(fn) { registry.push(fn); }` and
        // `function maybe(fn) { return fn; }` produce no fact, and no fact is
        // no evidence.
        let stores = invoking_argument_implementation(json!({
            "target": "symbol:store",
            "targetName": "store",
            "argumentParameters": [null]
        }));
        assert!(!inner_call_is_executed(&stores));

        // A slot the callee does not invoke, on a callee that invokes another.
        let other_slot = invoking_argument_implementation(json!({
            "target": "symbol:runWithOwner",
            "targetName": "runWithOwner",
            "argumentParameters": [null],
            "calleeInvokedParameters": [1]
        }));
        assert!(!inner_call_is_executed(&other_slot));
    }

    /// The falsifier for byte containment, and the reason the premise composes.
    ///
    /// Both halves below are the *same* nesting: a `createEffect` whose arrow
    /// contains an inner arrow that calls the parameter. They differ only in
    /// what the inner arrow is handed to. Stored in a registry, it never runs
    /// and the call must stay open; handed to `setTimeout`, it runs and the
    /// call is executed. A premise that read containment could not tell them
    /// apart, and would certify "calling this export invokes your callback"
    /// against a package that only ever stores it.
    #[test]
    fn a_merely_stored_inner_closure_breaks_the_invoking_chain() {
        // effect(() => {                        // the arrow, 20..90
        //   const inner = () => { cb(); };      // the inner arrow, 30..60
        //   <second call>(inner);               // 62..85
        // });
        let composition = |second: serde_json::Value| -> typefacts::ExportImplementationTranscript {
            let mut inner_carrier = json!({
                "location": {"path": "/project/index.js", "startByte": 62, "endByte": 85},
                "reach": "reachable",
                "kind": "call",
                "captured": true,
                "enclosingCallable": {"path": "/project/index.js", "startByte": 20, "endByte": 90},
                "argumentCallables": [{
                    "argument": 0,
                    "locations": [{"path": "/project/index.js", "startByte": 30, "endByte": 60}]
                }]
            });
            let (serde_json::Value::Object(fields), serde_json::Value::Object(overrides)) =
                (&mut inner_carrier, second)
            else {
                unreachable!("both literals are objects");
            };
            fields.extend(overrides);
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "controlFlow": {"returns": []},
                "calls": [
                    {
                        "location": {"path": "/project/index.js", "startByte": 10, "endByte": 95},
                        "reach": "reachable",
                        "kind": "call",
                        "target": "symbol:createEffect",
                        "targetName": "createEffect",
                        "targetModule": "solid-js",
                        "argumentParameters": [null],
                        "argumentCallables": [{
                            "argument": 0,
                            "locations": [
                                {"path": "/project/index.js", "startByte": 20, "endByte": 90}
                            ]
                        }]
                    },
                    inner_carrier,
                    {
                        "location": {"path": "/project/index.js", "startByte": 40, "endByte": 52},
                        "reach": "reachable",
                        "kind": "call",
                        "calleeParameter": {"parameterIndex": 0},
                        "captured": true,
                        "enclosingCallable": {
                            "path": "/project/index.js", "startByte": 30, "endByte": 60
                        }
                    }
                ]
            }))
            .unwrap()
        };

        // `registry.push(inner)` carries the inner arrow at a slot no tier
        // proves invoking. The chain breaks there, and byte nesting does not
        // repair it: the call site still lies inside the range `createEffect`
        // runs.
        let stored = composition(json!({
            "target": "symbol:push",
            "targetName": "push",
            "argumentParameters": [null]
        }));
        assert!(!implementation_call_is_executed(
            &stored,
            &stored.calls[2],
            ReachabilityFloor::Reachable
        ));

        // `setTimeout(inner, 0)` is the identical shape with one link proven,
        // and it composes: the inner arrow is carried at a slot the reviewed
        // table invokes, and the call carrying it is itself executed because the
        // arrow enclosing *it* is the one `createEffect` invokes.
        let scheduled = composition(json!({
            "target": "symbol:setTimeout",
            "targetName": "setTimeout",
            "defaultLibraryInvoker": "setTimeout",
            "invokedArguments": [0],
            "argumentParameters": [null, null]
        }));
        assert!(implementation_call_is_executed(
            &scheduled,
            &scheduled.calls[2],
            ReachabilityFloor::Reachable
        ));
    }

    /// The premise is transitive — a closure inside a closure inside an
    /// argument is real code — and it is bounded, because an unbounded walk over
    /// producer-supplied ranges is a denial of service rather than a proof.
    #[test]
    fn invoking_argument_recursion_is_transitive_and_bounded() {
        // Three nested effect closures: the outermost is a direct call, and each
        // inner one is carried by the one above it.
        let mut calls = Vec::new();
        for depth in 0..3usize {
            let start = depth * 10;
            let end = 100 - depth * 10;
            let enclosing = (depth != 0).then(|| {
                json!({
                    "path": "/project/index.js",
                    "startByte": (depth - 1) * 10 + 1,
                    "endByte": 100 - (depth - 1) * 10 - 1
                })
            });
            calls.push(json!({
                "location": {"path": "/project/index.js", "startByte": start, "endByte": end},
                "reach": "reachable",
                "kind": "call",
                "captured": depth != 0,
                "enclosingCallable": enclosing,
                "target": "symbol:createEffect",
                "targetName": "createEffect",
                "targetModule": "solid-js",
                "argumentParameters": [null],
                "argumentCallables": [{
                    "argument": 0,
                    "locations": [{
                        "path": "/project/index.js",
                        "startByte": start + 1,
                        "endByte": end - 1
                    }]
                }]
            }));
        }
        let nested: typefacts::ExportImplementationTranscript = serde_json::from_value(json!({
            "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
            "controlFlow": {"returns": []},
            "calls": calls
        }))
        .unwrap();
        assert!(implementation_call_is_executed(
            &nested,
            &nested.calls[2],
            ReachabilityFloor::Reachable
        ));

        // Real nesting is transitively contained, so genuine source can never
        // make the walk long: the outermost call's own argument range already
        // spans the innermost call. The bound therefore guards against
        // *producer-supplied ranges*, which are just numbers and need not nest
        // at all. This builds the adversarial shape directly — a chain of
        // disjoint one-byte calls where call k carries only call k+1 — and pins
        // that the bound, and not the construction, is what refuses it.
        let chain = |length: usize| -> typefacts::ExportImplementationTranscript {
            let calls = (0..length)
                .map(|link| {
                    json!({
                        "location": {
                            "path": "/project/index.js",
                            "startByte": link * 2,
                            "endByte": link * 2 + 1
                        },
                        "reach": "reachable",
                        "kind": "call",
                        "captured": link != 0,
                        "enclosingCallable": (link != 0).then(|| json!({
                            "path": "/project/index.js",
                            "startByte": link * 2,
                            "endByte": link * 2 + 1
                        })),
                        "target": "symbol:createEffect",
                        "targetName": "createEffect",
                        "targetModule": "solid-js",
                        "argumentParameters": [null],
                        "argumentCallables": [{
                            "argument": 0,
                            "locations": [{
                                "path": "/project/index.js",
                                "startByte": (link + 1) * 2,
                                "endByte": (link + 1) * 2 + 1
                            }]
                        }]
                    })
                })
                .collect::<Vec<_>>();
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "controlFlow": {"returns": []},
                "calls": calls
            }))
            .unwrap()
        };
        let within = chain(MAX_EXECUTION_PREMISE_DEPTH);
        assert!(implementation_call_is_executed(
            &within,
            &within.calls[MAX_EXECUTION_PREMISE_DEPTH - 1],
            ReachabilityFloor::Reachable
        ));
        let beyond = chain(MAX_EXECUTION_PREMISE_DEPTH + 3);
        assert!(!implementation_call_is_executed(
            &beyond,
            &beyond.calls[MAX_EXECUTION_PREMISE_DEPTH + 2],
            ReachabilityFloor::Reachable
        ));
    }

    /// S4-strong. The strong family accepts a chain that terminates in a direct
    /// call of the parameter and refuses the weak transitive variant — otherwise
    /// `callable-path` silently becomes `argument-binding`.
    #[test]
    fn strong_callback_flow_requires_a_terminating_direct_call() {
        let source = ValueSource::Parameter {
            index: 0,
            path: Vec::new(),
        };
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "callable-path".into(),
            reason: reason.to_string(),
        };
        let mut implementation: typefacts::ExportImplementationTranscript =
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "calls": [{
                    "location": {"path": "/project/index.js", "startByte": 10, "endByte": 40},
                    "reach": "reachable",
                    "kind": "call",
                    "target": "symbol:createPolled",
                    "targetName": "createPolled",
                    "argumentParameters": [null, {"parameterIndex": 0}],
                    "calleeDirectlyCalledParameters": [],
                    "calleeInvokedParameters": [1],
                    "calleeStronglyInvokedParameters": [1]
                }]
            }))
            .unwrap();
        let mut sites = Vec::new();
        require_parameter_callback_flow(
            &implementation,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut sites,
        )
        .expect("a chain of plain forwards ending in `delay()` is a direct-call claim");
        assert!(
            sites
                .iter()
                .any(|site| site.starts_with("implementation-callee-direct-callback:")),
            "witness = {sites:?}"
        );

        // The weak fact alone is not the strong claim. A chain that ends at
        // `addEventListener` proves the value runs and says nothing about
        // whether the callee treats the position as a function.
        implementation.calls[0]
            .callee_strongly_invoked_parameters
            .clear();
        let mut weak_sites = Vec::new();
        require_parameter_callback_flow(
            &implementation,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut weak_sites,
        )
        .expect_err("the weak transitive fact must not satisfy the strong claim");

        // And the slot has to be the one the callback occupies.
        implementation.calls[0].callee_strongly_invoked_parameters = vec![0];
        let mut wrong_slot = Vec::new();
        require_parameter_callback_flow(
            &implementation,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut wrong_slot,
        )
        .expect_err("a strong claim about another slot is not about this callback");
    }

    /// A construction runs what it carries, and is still not a call.
    ///
    /// `@solidjs/signals::action` is the whole case:
    /// `return (…args) => new Promise((resolve, reject) => { const it = genFn(…args); … })`.
    /// The `genFn` call runs — the executor runs synchronously — but nothing in
    /// a census of call expressions could say so, because the callable that
    /// immediately contains it is carried by a `new` expression.
    #[test]
    fn a_construction_executes_what_it_carries_and_is_still_not_a_call() {
        // (…args) => {                          // the returned arrow, 20..90
        //   new Promise((resolve, reject) => {  // 30..80, executor 45..75
        //     const it = genFn(…args);          // 50..70
        //   });
        // }
        let implementation = |kind: &str,
                              invoker: &str,
                              slots: serde_json::Value|
         -> typefacts::ExportImplementationTranscript {
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "controlFlow": {"returns": [{
                    "location": {"path": "/project/index.js", "startByte": 12, "endByte": 92},
                    "reach": "reachable",
                    "carryReach": "reachable",
                    "carriedCallables": [
                        {"path": "/project/index.js", "startByte": 20, "endByte": 90}
                    ]
                }]},
                "calls": [
                    {
                        "location": {"path": "/project/index.js", "startByte": 30, "endByte": 80},
                        "reach": "reachable",
                        "kind": kind,
                        "target": "symbol:Promise",
                        "targetName": "Promise",
                        "argumentParameters": [null],
                        "captured": true,
                        "enclosingCallable": {
                            "path": "/project/index.js", "startByte": 20, "endByte": 90
                        },
                        "defaultLibraryInvoker": invoker,
                        "invokedArguments": slots,
                        "argumentCallables": [{
                            "argument": 0,
                            "locations": [
                                {"path": "/project/index.js", "startByte": 45, "endByte": 75}
                            ]
                        }]
                    },
                    {
                        "location": {"path": "/project/index.js", "startByte": 50, "endByte": 70},
                        "reach": "reachable",
                        "kind": "call",
                        "target": "symbol:genFn",
                        "calleeParameter": {"parameterIndex": 0},
                        "captured": true,
                        "enclosingCallable": {
                            "path": "/project/index.js", "startByte": 45, "endByte": 75
                        }
                    }
                ]
            }))
            .unwrap()
        };
        let source = ValueSource::Parameter {
            index: 0,
            path: Vec::new(),
        };
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "operation-cardinality".into(),
            reason: reason.to_string(),
        };

        // The chain: the returned arrow is carried by a reachable return, it
        // encloses the construction, and the construction's slot 0 carries the
        // executor that immediately encloses `genFn(…)`.
        let action = implementation("construct", "promiseConstructor", json!([0]));
        assert!(implementation_call_is_executed(
            &action,
            &action.calls[1],
            ReachabilityFloor::Reachable
        ));
        let mut sites = Vec::new();
        require_parameter_flow(
            &action,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut sites,
        )
        .expect("the executor runs, so the call inside it runs");

        // The construction itself is not a call, and no witness may say it is.
        // `new Cls(cb)` is a different claim about `cb` than `cls(cb)` and this
        // family was not reviewed for it, so the argument branch must not see
        // the construction at all.
        let mut passed = implementation("construct", "promiseConstructor", json!([0]));
        passed.calls[0].argument_parameters = vec![Some(
            serde_json::from_value(json!({"parameterIndex": 0})).unwrap(),
        )];
        passed.calls.truncate(1);
        let mut construct_sites = Vec::new();
        require_parameter_flow(
            &passed,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut construct_sites,
        )
        .expect_err("a parameter handed to a construction is not a called parameter");

        // The verifier owns the table on its own side. An unreviewed invoker
        // string is refused outright; the reviewed one is refused at a slot its
        // own table does not list, however the producer spelled the list.
        let unreviewed = implementation("construct", "promiseAny", json!([0]));
        assert!(!implementation_call_is_executed(
            &unreviewed,
            &unreviewed.calls[1],
            ReachabilityFloor::Reachable
        ));
        let widened = implementation("construct", "promiseConstructor", json!([0, 1]));
        assert!(!argument_slot_is_proven_invoking(&widened.calls[0], 1));
        assert!(argument_slot_is_proven_invoking(&widened.calls[0], 0));
        let omitted = implementation("construct", "promiseConstructor", json!([]));
        assert!(!argument_slot_is_proven_invoking(&omitted.calls[0], 0));

        // A census entry that states no kind states no call. It still carries
        // its executor for the execution premise — that premise is about
        // whether code runs, and the reviewed invoker row already fixes what
        // this site is — but every claim whose witness says "call" refuses it.
        let unstated = implementation("unknown", "promiseConstructor", json!([0]));
        assert!(implementation_call_is_executed(
            &unstated,
            &unstated.calls[1],
            ReachabilityFloor::Reachable
        ));
        let mut unstated_carrier = implementation("unknown", "promiseConstructor", json!([0]));
        unstated_carrier.calls[0].argument_parameters = vec![Some(
            serde_json::from_value(json!({"parameterIndex": 0})).unwrap(),
        )];
        unstated_carrier.calls.truncate(1);
        let mut unstated_sites = Vec::new();
        require_parameter_flow(
            &unstated_carrier,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut unstated_sites,
        )
        .expect_err("a site of unstated kind is not a call");
    }

    /// Every remaining consumer whose witness says *call* refuses a
    /// construction, and each gate is pinned on its own.
    ///
    /// `require_parameter_flow` was the only one with a test. The other five
    /// sites survived a mutation exercise that neutralised each in turn, and
    /// three of them are load-bearing today rather than defensive: `target`,
    /// `targetName`, `targetModule` and `argumentParameters` *are* stated for a
    /// construction, so an ungated `require_parameter_callback_flow` discharges
    /// the `callable-path` family for `new X(cb)` whenever `X` resolves to a
    /// `solid-js` import with an unambiguous callback slot, an ungated
    /// `require_owner_operation_call` reaches `unambiguous_owner_requirement_role`
    /// the same way, and its ungated `observed` list moves a refusal tail.
    ///
    /// The two that are redundant with the producer today — the `Read`
    /// witness and the recursive-parameter witness, both of which need a
    /// `calleeParameter` a construction does not carry — are pinned anyway.
    /// This side certifies against the wire contract, not against a producer's
    /// current habits, and a fact stated by a future or hostile producer must
    /// hit the same wall.
    #[test]
    fn a_construction_satisfies_no_claim_whose_witness_says_call() {
        // One census entry with everything a call of `createEffect(cb)` would
        // carry, parameterized only by the kind. Each consumer below must
        // accept it as `"call"` and refuse it as `"construct"` — a refusal that
        // held for both spellings would pin nothing.
        let dialect_call = |kind: &str| -> typefacts::ExportImplementationTranscript {
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "calls": [{
                    "location": {"path": "/project/index.js", "startByte": 10, "endByte": 40},
                    "reach": "reachable",
                    "kind": kind,
                    "target": "symbol:createEffect",
                    "targetName": "createEffect",
                    "targetModule": "solid-js",
                    "argumentParameters": [{"parameterIndex": 0}]
                }]
            }))
            .unwrap()
        };
        // The same, for the two witnesses that read `calleeParameter`: the
        // parameter value is itself the callee.
        let callee_call = |kind: &str| -> typefacts::ExportImplementationTranscript {
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "calls": [{
                    "location": {"path": "/project/index.js", "startByte": 10, "endByte": 40},
                    "reach": "reachable",
                    "kind": kind,
                    "target": "symbol:cb",
                    "calleeParameter": {"parameterIndex": 0}
                }]
            }))
            .unwrap()
        };
        // And for the owner-primitive witness.
        let owner_call = |kind: &str| -> typefacts::ExportImplementationTranscript {
            serde_json::from_value(json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "calls": [{
                    "location": {"path": "/project/index.js", "startByte": 10, "endByte": 40},
                    "reach": "reachable",
                    "kind": kind,
                    "target": "symbol:onCleanup",
                    "targetName": "onCleanup",
                    "targetModule": "solid-js"
                }]
            }))
            .unwrap()
        };
        let source = ValueSource::Parameter {
            index: 0,
            path: Vec::new(),
        };
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "construct-kind".into(),
            reason: reason.to_string(),
        };

        // `require_parameter_callback_flow` — the `callable-path` family.
        require_parameter_callback_flow(
            &dialect_call("call"),
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut Vec::new(),
        )
        .expect("`createEffect`'s slot 0 is a callback position");
        require_parameter_callback_flow(
            &dialect_call("construct"),
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut Vec::new(),
        )
        .expect_err("`new createEffect(cb)` is not a callback flow");

        // `require_parameter_read_call` — the `Read` operation's witness.
        require_parameter_read_call(
            &callee_call("call"),
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut Vec::new(),
        )
        .expect("the parameter is the callee of a reachable call");
        require_parameter_read_call(
            &callee_call("construct"),
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut Vec::new(),
        )
        .expect_err("`new cb()` is not a read of `cb` as a callee");

        // `recursive_parameter_call_site` — the `recursive-operation-parameter:`
        // witness.
        assert!(recursive_parameter_call_site(&callee_call("call"), &source).is_some());
        assert!(
            recursive_parameter_call_site(&callee_call("construct"), &source).is_none(),
            "a construction of the parameter is not a call of it"
        );

        // `require_owner_operation_call` — both its match loop and the
        // `observed` list it builds when nothing matched, which is a refusal
        // tail a reader compares across runs.
        let cleanup_operation = solid_reactive_ir::contract_semantics::Operation {
            id: solid_reactive_ir::contract_semantics::OperationId("create".into()),
            kind: OperationKind::Create,
            guard: None,
            trigger: None,
            at: None,
            schedule: None,
            tracking: solid_reactive_ir::contract_semantics::Tracking::Unknown,
            owner: solid_reactive_ir::contract_semantics::OwnerRelation {
                requirements: solid_reactive_ir::contract_semantics::OwnerRequirements {
                    cleanup: Requirement::Required,
                    ..Default::default()
                },
                ..Default::default()
            },
            cardinality: solid_reactive_ir::contract_semantics::Cardinality::default(),
            inputs: Vec::new(),
            output: None,
            resources: std::collections::BTreeSet::new(),
        };
        let demand = proof(ProofFamily::OperationReachability, selected_subject());
        let mut owner_sites = Vec::new();
        require_owner_operation_call(
            &cleanup_operation,
            &demand,
            &owner_call("call"),
            ReachabilityFloor::Reachable,
            &open,
            &mut owner_sites,
        )
        .expect("a reachable `onCleanup(…)` call satisfies the cleanup requirement");
        assert!(
            owner_sites
                .iter()
                .any(|site| site.starts_with("implementation-owner-call:")),
            "witness = {owner_sites:?}"
        );
        let refused = require_owner_operation_call(
            &cleanup_operation,
            &demand,
            &owner_call("construct"),
            ReachabilityFloor::Reachable,
            &open,
            &mut Vec::new(),
        )
        .expect_err("`new onCleanup()` is not a call of the owner primitive");
        let TypeFactsCertificationError::FamilyOpen { reason, .. } = &refused else {
            panic!("unexpected refusal: {refused:?}");
        };
        assert!(
            !reason.contains("onCleanup"),
            "the observed list must not name a construction either: {reason}"
        );
    }

    /// What an unrecognized kind actually does, as opposed to what the field's
    /// documentation used to claim.
    ///
    /// An *absent* kind defaults to `Unknown`, which every kind-gated consumer
    /// refuses. An *unrecognized* one is not mapped to `Unknown` at all:
    /// `CallKind` carries no catch-all arm, so deserialization fails and the
    /// whole transcript is rejected. The earlier pin used `"unknown"` — a
    /// *recognized* variant — so nothing tested the sentence as written.
    #[test]
    fn an_unrecognized_call_kind_rejects_the_transcript_rather_than_defaulting() {
        let census = |kind: serde_json::Value| {
            let mut call = json!({
                "location": {"path": "/project/index.js", "startByte": 10, "endByte": 40},
                "reach": "reachable"
            });
            if let (serde_json::Value::Object(fields), serde_json::Value::String(kind)) =
                (&mut call, &kind)
            {
                fields.insert("kind".into(), json!(kind));
            }
            json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "calls": [call]
            })
        };

        // Absent: `Unknown`, and refused by every claim that says "call".
        let absent: typefacts::ExportImplementationTranscript =
            serde_json::from_value(census(serde_json::Value::Null)).unwrap();
        assert_eq!(absent.calls[0].kind, typefacts::CallKind::Unknown);
        assert!(!is_call_expression(&absent.calls[0]));

        // Unrecognized: not a fact this side can read at all.
        for spelling in ["newExpression", "CALL", "invoke", ""] {
            serde_json::from_value::<typefacts::ExportImplementationTranscript>(census(json!(
                spelling
            )))
            .expect_err(&format!("`{spelling}` is not a kind this side recognizes"));
        }

        // And the two it does recognize still deserialize.
        for (spelling, expected) in [
            ("call", typefacts::CallKind::Call),
            ("construct", typefacts::CallKind::Construct),
            ("unknown", typefacts::CallKind::Unknown),
        ] {
            let parsed: typefacts::ExportImplementationTranscript =
                serde_json::from_value(census(json!(spelling))).unwrap();
            assert_eq!(parsed.calls[0].kind, expected);
        }
    }

    /// The deferred dialect premise. The producer states which imported slot a
    /// callee-body chain depends on; this side decides whether that slot is a
    /// callback position, on the same table the first tier already uses.
    ///
    /// `@solid-primitives/timer` is the case: `createTimer` calls `delay()`
    /// inside the closure it hands to `createEffect`, and two plain forwards
    /// carry the claim out to `createIntervalCounter`.
    #[test]
    fn a_deferred_dialect_premise_is_answered_here_or_the_claim_stays_open() {
        let implementation =
            |pending: serde_json::Value| -> typefacts::ExportImplementationTranscript {
                serde_json::from_value(json!({
                    "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                    "calls": [{
                        "location": {"path": "/project/index.js", "startByte": 10, "endByte": 40},
                        "reach": "reachable",
                        "kind": "call",
                        "target": "symbol:createPolled",
                        "targetName": "createPolled",
                        "argumentParameters": [null, {"parameterIndex": 0}],
                        "calleePendingInvocations": pending
                    }]
                }))
                .unwrap()
            };
        let source = ValueSource::Parameter {
            index: 0,
            path: Vec::new(),
        };
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "callable-path".into(),
            reason: reason.to_string(),
        };
        let premise = |module: &str, name: &str, slot: usize, count: usize| json!({"module": module, "name": name, "slot": slot, "argumentCount": count});

        let timer = implementation(json!([{
            "parameter": 1, "strong": true,
            "requires": [premise("solid-js", "createEffect", 0, 1)]
        }]));
        let mut sites = Vec::new();
        require_parameter_callback_flow(
            &timer,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut sites,
        )
        .expect("`createEffect` slot 0 is a callback position in every dialect that owns it");
        assert!(
            sites
                .iter()
                .any(|site| site.starts_with("implementation-callee-direct-callback:")),
            "witness = {sites:?}"
        );
        assert!(argument_slot_is_proven_invoking(&timer.calls[0], 1));

        // Every refusal is a refusal of the *premise*, decided here rather than
        // believed from the wire: another package's function is not a dialect
        // primitive however it is named; `createEffect`'s second argument is a
        // callback in 2.0 and absent in 1.x, so no dialect answer is unanimous
        // and the slot stays open; and a claim that defers nothing is malformed
        // rather than unconditional — the unconditional claims travel in the
        // index lists.
        for refusal in [
            json!([{"parameter": 1, "strong": true,
                    "requires": [premise("@solid-primitives/timer", "createEffect", 0, 1)]}]),
            json!([{"parameter": 1, "strong": true,
                    "requires": [premise("solid-js", "createEffect", 1, 2)]}]),
            json!([{"parameter": 1, "strong": true, "requires": []}]),
            json!([{"parameter": 0, "strong": true,
                    "requires": [premise("solid-js", "createEffect", 0, 1)]}]),
            // Every requirement of a conjunction must hold: one answered
            // premise does not carry an unanswerable one.
            json!([{"parameter": 1, "strong": true, "requires": [
                premise("solid-js", "createEffect", 0, 1),
                premise("mystery", "run", 0, 1)
            ]}]),
        ] {
            let refused = implementation(refusal.clone());
            let mut refused_sites = Vec::new();
            require_parameter_callback_flow(
                &refused,
                &source,
                ReachabilityFloor::Reachable,
                &open,
                &mut refused_sites,
            )
            .expect_err(&format!("this premise is not discharged here: {refusal}"));
            assert!(
                !argument_slot_is_proven_invoking(&refused.calls[0], 1),
                "{refusal} must not prove an invoking slot either"
            );
        }

        // The strong/weak split survives the deferral exactly as it survives an
        // unconditional fact: a chain that only proves the value runs proves an
        // invoking slot and never a callable position.
        let weak = implementation(json!([{
            "parameter": 1,
            "requires": [premise("solid-js", "createEffect", 0, 1)]
        }]));
        assert!(argument_slot_is_proven_invoking(&weak.calls[0], 1));
        let mut weak_sites = Vec::new();
        require_parameter_callback_flow(
            &weak,
            &source,
            ReachabilityFloor::Reachable,
            &open,
            &mut weak_sites,
        )
        .expect_err("a weak deferred claim is not a callable-position claim");
    }

    #[test]
    fn return_callable_source_requires_every_reachable_return() {
        let mut transcript: ExportValueTranscript = serde_json::from_value(json!({
            "location": {"path": "/project/index.d.ts", "startByte": 0, "endByte": 4},
            "value": {
                "callability": "callable",
                "constructability": "nonConstructable",
                "primitive": {"mayBeObject": true}
            },
            "implementation": {
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 4},
                "controlFlow": {"returns": [{
                    "location": {"path": "/project/index.js", "startByte": 20, "endByte": 30},
                    "reach": "reachable",
                    "sources": [{
                        "path": [{"kind": "tuple", "index": 0}],
                        "kind": "directCallable"
                    }]
                }]}
            }
        }))
        .unwrap();
        let expected = vec![typefacts::PathSegment {
            kind: PathSegmentKind::Tuple,
            property: String::new().into(),
            index: Some(0),
        }];
        let mut sites = Vec::new();
        assert!(require_return_callable_source(
            &transcript,
            &expected,
            &mut sites
        ));
        assert_eq!(sites.len(), 1);

        transcript
            .implementation
            .as_mut()
            .unwrap()
            .control_flow
            .as_mut()
            .unwrap()
            .returns
            .push(
                serde_json::from_value(json!({
                    "location": {"path": "/project/index.js", "startByte": 31, "endByte": 40},
                    "reach": "reachable"
                }))
                .unwrap(),
            );
        sites.clear();
        assert!(!require_return_callable_source(
            &transcript,
            &expected,
            &mut sites
        ));
        assert!(sites.is_empty());

        transcript
            .implementation
            .as_mut()
            .unwrap()
            .control_flow
            .as_mut()
            .unwrap()
            .returns[1]
            .reach = Reachability::Unknown;
        assert!(!require_return_callable_source(
            &transcript,
            &expected,
            &mut sites
        ));
    }

    fn export_value_transcript(signatures: serde_json::Value) -> ExportValueTranscript {
        let mut document = json!({
            "location": {"path": "/project/index.d.ts", "startByte": 0, "endByte": 4},
            "value": {
                "callability": "callable",
                "constructability": "nonConstructable",
                "primitive": {"mayBeObject": true}
            },
            "callablePaths": [{
                "alternative": 0,
                "path": [{"kind": "property", "property": "map"}],
                "presence": "required",
                "callability": "callable",
                "constructability": "nonConstructable",
                "complete": true,
                "subtreeEnumerated": true
            }],
            "complete": true
        });
        let object = document.as_object_mut().expect("transcript object");
        for (key, value) in signatures.as_object().expect("signature fields") {
            object.insert(key.clone(), value.clone());
        }
        serde_json::from_value(document).expect("export-value transcript")
    }

    fn export_signature(ordinal: usize, count: usize, result_callable: &str) -> serde_json::Value {
        json!({
            "identity": format!("sha256:{:064x}", 40 + ordinal),
            "declaration": {
                "symbol": "symbol:createServerCookie",
                "name": "createServerCookie",
                "kind": "function",
                "location": {"path": "/project/index.d.ts", "startByte": 0, "endByte": 30},
                "originModule": "pkg",
                "sourceFile": "/project/index.d.ts"
            },
            "overloadOrdinal": ordinal,
            "overloadCount": count,
            "minimumArgumentCount": 1,
            "parameters": [],
            "result": {
                "callability": result_callable,
                "constructability": "nonConstructable",
                "primitive": {"mayBeObject": true}
            }
        })
    }

    fn export_recursive_proof(
        family: ProofFamily,
        path: Vec<ValuePathSegment>,
        callable: DemandedCallability,
    ) -> ScheduledProofDemand {
        proof(
            family,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                artifact_case: "browser".into(),
                export: "run".into(),
                root: ValueRoot::Export,
                path: solid_reactive_ir::contract_semantics::ValuePath(path),
                callable,
            }),
        )
    }

    #[test]
    fn an_unasserted_callability_verifies_the_path_without_a_callability_premise() {
        let transcript = export_value_transcript(json!({}));
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "test".into(),
            reason: reason.into(),
        };
        let path = vec![ValuePathSegment::ObjectProperty("map".into())];

        // `Parameter { path: ["map"] }` is `Array.prototype.map`. The IR never
        // classified its callability, so the demand asserts none -- and the
        // producer's exact `callable` answer can no longer contradict it.
        let mut sites = Vec::new();
        assert!(
            require_export_recursive_subject(
                &export_recursive_proof(
                    ProofFamily::RecursiveValueShape,
                    path.clone(),
                    DemandedCallability::Unknown
                ),
                &transcript,
                &open,
                &mut sites,
            )
            .is_ok(),
            "an unasserted callability must not be refused by the producer's own answer"
        );
        assert_eq!(sites.len(), 1, "the path premise still produces a witness");

        let mut depth_cut = transcript.clone();
        depth_cut.callable_paths[0].subtree_enumerated = false;
        let mut local_sites = Vec::new();
        assert!(
            require_export_recursive_subject(
                &export_recursive_proof(
                    ProofFamily::RecursiveValueShape,
                    path.clone(),
                    DemandedCallability::Unknown,
                ),
                &depth_cut,
                &open,
                &mut local_sites,
            )
            .is_ok(),
            "an exact export path must ignore descendant-census exhaustion"
        );
        assert!(
            require_export_callable_paths_closed(&depth_cut, &open).is_err(),
            "a whole export census must refuse subtreeEnumerated=false"
        );

        let mut absent_alternative = transcript.clone();
        let absent = &mut absent_alternative.callable_paths[0];
        absent.presence = PathPresence::Absent;
        absent.callability = Callability::Unknown;
        absent.constructability = typefacts::InvocationConstructability::Unknown;
        assert!(
            require_export_recursive_subject(
                &export_recursive_proof(
                    ProofFamily::RecursiveValueShape,
                    path.clone(),
                    DemandedCallability::Unknown,
                ),
                &absent_alternative,
                &open,
                &mut Vec::new(),
            )
            .is_err(),
            "a positive exact-path demand must refuse a proven-absent alternative"
        );
        assert!(
            require_export_callable_paths_closed(&absent_alternative, &open).is_ok(),
            "a proven-absent alternative is still a closed whole-census entry"
        );

        // The premise the boolean forced -- and the exact refusal the whole
        // ecosystem row hit.
        let mut refused = Vec::new();
        assert!(matches!(
            require_export_recursive_subject(
                &export_recursive_proof(
                    ProofFamily::RecursiveValueShape,
                    path.clone(),
                    DemandedCallability::NonCallable
                ),
                &transcript,
                &open,
                &mut refused,
            ),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));

        // Unknown drops only the callability premise. An absent path, or one
        // that is locally open, still refuses.
        let mut absent = Vec::new();
        assert!(matches!(
            require_export_recursive_subject(
                &export_recursive_proof(
                    ProofFamily::RecursiveValueShape,
                    vec![ValuePathSegment::ObjectProperty("missing".into())],
                    DemandedCallability::Unknown
                ),
                &transcript,
                &open,
                &mut absent,
            ),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));

        let mut locally_open = export_value_transcript(json!({}));
        locally_open.callable_paths[0].complete = false;
        let mut open_sites = Vec::new();
        assert!(
            matches!(
                require_export_recursive_subject(
                    &export_recursive_proof(
                        ProofFamily::RecursiveValueShape,
                        path,
                        DemandedCallability::Unknown
                    ),
                    &locally_open,
                    &open,
                    &mut open_sites,
                ),
                Err(TypeFactsCertificationError::FamilyOpen { .. })
            ),
            "an unasserted callability must not weaken the path's own closure premise"
        );
    }

    #[test]
    fn callable_path_closure_rejects_unknown_presence_and_open_reasons() {
        let mut fact = export_value_transcript(json!({})).callable_paths.remove(0);
        let mut optional = fact.clone();
        optional.presence = PathPresence::Optional;
        assert!(callable_path_has_closed_local_observation(&optional));
        assert!(callable_path_is_present_and_locally_closed(&optional));
        assert!(callable_path_census_is_closed(&optional));

        fact.presence = PathPresence::Unknown;
        assert!(!callable_path_has_closed_local_observation(&fact));
        assert!(!callable_path_is_present_and_locally_closed(&fact));
        assert!(!callable_path_census_is_closed(&fact));

        fact.presence = PathPresence::Required;
        fact.open_reasons.push("openType".into());
        assert!(!callable_path_has_closed_local_observation(&fact));
        assert!(!callable_path_is_present_and_locally_closed(&fact));
        assert!(!callable_path_census_is_closed(&fact));
    }

    /// The adversarial input for F3: a transcript that concedes everything it
    /// can. Callability unknown, constructability unknown, the primitive domain
    /// unknown, an open reason attached, no callable-path census at all, and the
    /// transcript itself marked incomplete. Nothing in it establishes anything.
    fn maximally_open_export_value_transcript() -> ExportValueTranscript {
        serde_json::from_value(json!({
            "location": {"path": "/project/index.d.ts", "startByte": 0, "endByte": 4},
            "value": {
                "callability": "unknown",
                "constructability": "unknown",
                "primitive": {"unknown": true},
                "openReasons": ["openType"]
            }
        }))
        .expect("export-value transcript")
    }

    #[test]
    fn a_root_shape_without_a_callability_assertion_needs_a_closed_producer_observation() {
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "test".into(),
            reason: reason.into(),
        };
        let transcript = maximally_open_export_value_transcript();

        // Empty path, unasserted callability: there is no census entry to find
        // and no callability to check, so the only premise left is that the
        // producer exhaustively observed the root. This transcript did not.
        let mut vacuous = Vec::new();
        assert!(
            matches!(
                require_export_recursive_subject(
                    &export_recursive_proof(
                        ProofFamily::RecursiveValueShape,
                        Vec::new(),
                        DemandedCallability::Unknown
                    ),
                    &transcript,
                    &open,
                    &mut vacuous,
                ),
                Err(TypeFactsCertificationError::FamilyOpen { .. })
            ),
            "a root demand asserting no callability must not discharge against an open transcript"
        );
        assert!(vacuous.is_empty(), "an open family produces no witness");

        // The recovery: a closed root observation *is* the premise. The shape
        // fact ("this value is what the IR modelled") is retained on the
        // strength of the producer having exhaustively answered for the root,
        // and nothing about callability is asserted onto the declaration.
        let mut closed_root = Vec::new();
        assert!(
            require_export_recursive_subject(
                &export_recursive_proof(
                    ProofFamily::RecursiveValueShape,
                    Vec::new(),
                    DemandedCallability::Unknown
                ),
                &export_value_transcript(json!({})),
                &open,
                &mut closed_root,
            )
            .is_ok(),
            "a closed producer observation discharges the root by evidence"
        );
        assert_eq!(closed_root, vec!["recursive-export-value:root".to_string()]);

        // Each half of "closed" on its own refuses. An answered callability
        // with an open reason attached is not an exhaustive observation, and
        // neither is a silent transcript that never answered.
        for spoiled in [
            json!({"callability": "callable", "constructability": "nonConstructable",
                   "primitive": {"mayBeObject": true}, "openReasons": ["openType"]}),
            json!({"callability": "unknown", "constructability": "nonConstructable",
                   "primitive": {"mayBeObject": true}}),
            json!({"callability": "callable", "constructability": "nonConstructable",
                   "primitive": {"unknown": true}}),
        ] {
            let mut transcript = export_value_transcript(json!({}));
            transcript.value = serde_json::from_value(spoiled.clone()).expect("value fact");
            let mut refused = Vec::new();
            assert!(
                matches!(
                    require_export_recursive_subject(
                        &export_recursive_proof(
                            ProofFamily::RecursiveValueShape,
                            Vec::new(),
                            DemandedCallability::Unknown
                        ),
                        &transcript,
                        &open,
                        &mut refused,
                    ),
                    Err(TypeFactsCertificationError::FamilyOpen { .. })
                ),
                "a partially observed root is not a premise: {spoiled}"
            );
        }

        // The root *with* an assertion still has a premise, and this transcript
        // refuses it because it proves nothing.
        let mut asserted = Vec::new();
        assert!(matches!(
            require_export_recursive_subject(
                &export_recursive_proof(
                    ProofFamily::RecursiveValueShape,
                    Vec::new(),
                    DemandedCallability::Callable
                ),
                &transcript,
                &open,
                &mut asserted,
            ),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));

        // The companion: a *non-empty* path with an unasserted callability
        // keeps verifying its sibling premises — presence, closure, and being
        // in the census at all — and discharges on a transcript that has them.
        let mut sibling = Vec::new();
        assert!(
            require_export_recursive_subject(
                &export_recursive_proof(
                    ProofFamily::RecursiveValueShape,
                    vec![ValuePathSegment::ObjectProperty("map".into())],
                    DemandedCallability::Unknown
                ),
                &export_value_transcript(json!({})),
                &open,
                &mut sibling,
            )
            .is_ok(),
            "a non-empty path verifies its own census premises without a callability assertion"
        );
        assert_eq!(sibling.len(), 1);

        // And a root demand that *does* assert callability discharges against a
        // transcript that proves it, so the guard is not simply refusing roots.
        let mut proved = Vec::new();
        assert!(
            require_export_recursive_subject(
                &export_recursive_proof(
                    ProofFamily::RecursiveValueShape,
                    Vec::new(),
                    DemandedCallability::Callable
                ),
                &export_value_transcript(json!({})),
                &open,
                &mut proved,
            )
            .is_ok()
        );
        assert_eq!(proved, vec!["recursive-export-value:root".to_string()]);
    }

    #[test]
    fn an_overload_set_is_proved_by_every_overload_and_never_by_one() {
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "test".into(),
            reason: reason.into(),
        };
        let unique = proof(ProofFamily::SelectedSignature, selected_subject());

        // No signature at all stays open: the export has none to prove.
        assert!(matches!(
            require_export_call_signatures(&unique, &export_value_transcript(json!({})), &open),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));

        // One signature is still exactly one, and still has to be the only
        // overload; a lone member of an overload set is not "the" signature.
        let single = export_value_transcript(json!({
            "callSignature": export_signature(0, 1, "nonCallable")
        }));
        assert_eq!(
            require_export_call_signatures(&unique, &single, &open)
                .expect("a unique signature")
                .len(),
            1
        );
        let masquerading = export_value_transcript(json!({
            "callSignature": export_signature(1, 2, "nonCallable")
        }));
        assert!(matches!(
            require_export_call_signatures(&unique, &masquerading, &open),
            Err(TypeFactsCertificationError::UnsupportedDemand { .. })
        ));

        // A real overload set: `@solid-primitives/cookies`'s
        // `createServerCookie` has two, and demanding "the" one asked for an
        // object that does not exist. Every overload is returned so a caller
        // can require its premise of all of them.
        let overloaded = export_value_transcript(json!({
            "callSignatures": [
                export_signature(0, 2, "nonCallable"),
                export_signature(1, 2, "nonCallable")
            ]
        }));
        let signatures =
            require_export_call_signatures(&unique, &overloaded, &open).expect("the overload set");
        assert_eq!(signatures.len(), 2);
        assert_eq!(
            signatures
                .iter()
                .map(|signature| signature.overload_ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );

        // Completeness is verified here, not trusted. A set short of the
        // declared overload count is what a dropped signature looks like, and
        // it is exactly the shape that would narrow "every overload" to "every
        // overload the producer could describe".
        let partial = export_value_transcript(json!({
            "callSignatures": [export_signature(0, 2, "nonCallable")]
        }));
        assert!(matches!(
            require_export_call_signatures(&unique, &partial, &open),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));
        let dropped_head = export_value_transcript(json!({
            "callSignatures": [export_signature(1, 2, "nonCallable")]
        }));
        assert!(matches!(
            require_export_call_signatures(&unique, &dropped_head, &open),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));
        let repeated = export_value_transcript(json!({
            "callSignatures": [
                export_signature(0, 2, "nonCallable"),
                export_signature(0, 2, "nonCallable")
            ]
        }));
        assert!(matches!(
            require_export_call_signatures(&unique, &repeated, &open),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));
        let disagreeing = export_value_transcript(json!({
            "callSignatures": [
                export_signature(0, 3, "nonCallable"),
                export_signature(1, 2, "nonCallable")
            ]
        }));
        assert!(matches!(
            require_export_call_signatures(&unique, &disagreeing, &open),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));

        // The two fields are mutually exclusive. A transcript populating both
        // is answered by neither.
        let both = export_value_transcript(json!({
            "callSignature": export_signature(0, 1, "nonCallable"),
            "callSignatures": [
                export_signature(0, 2, "nonCallable"),
                export_signature(1, 2, "nonCallable")
            ]
        }));
        assert!(matches!(
            require_export_call_signatures(&unique, &both, &open),
            Err(TypeFactsCertificationError::UnsupportedDemand { .. })
        ));
    }

    #[test]
    fn runtime_function_shape_accepts_constructable_but_needs_both_negatives_for_values() {
        let mut value: InvocationValueFact = serde_json::from_value(json!({
            "callability": "nonCallable",
            "constructability": "constructable",
            "primitive": {"mayBeObject": true}
        }))
        .unwrap();
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "test".into(),
            reason: reason.into(),
        };
        let callable = DemandedCallability::Callable;
        let non_callable = DemandedCallability::NonCallable;
        assert!(require_root_callability(&value, callable, "export", &open).is_ok());
        assert!(require_root_callability(&value, non_callable, "export", &open).is_err());
        // An unasserted callability has no premise here, in either direction.
        assert!(
            require_root_callability(&value, DemandedCallability::Unknown, "export", &open).is_ok()
        );

        value.constructability = InvocationConstructability::NonConstructable;
        assert!(require_root_callability(&value, non_callable, "export", &open).is_ok());
        value.callability = Callability::Unknown;
        assert!(require_root_callability(&value, non_callable, "export", &open).is_err());
        assert!(
            require_root_callability(&value, DemandedCallability::Unknown, "export", &open).is_ok(),
            "an unknown producer callability cannot refuse a demand that asserts nothing"
        );
    }

    #[test]
    fn unresolved_parameter_callability_requires_an_exact_dialect_type_import() {
        let mut signature = transcript().selected_signature.unwrap();
        let parameter = &mut signature.parameters[0];
        parameter.value.callability = Callability::Unknown;
        parameter.declared_type = Some(typefacts::DeclaredTypeReference {
            name: "Accessor".into(),
            module: "solid-js".into(),
        });
        let source = ValueSource::Parameter {
            index: 0,
            path: Vec::new(),
        };
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "test".into(),
            reason: reason.into(),
        };
        let mut sites = Vec::new();
        assert!(
            require_signature_parameter_callable(&signature, &source, &open, &mut sites).is_ok()
        );

        signature.parameters[0]
            .declared_type
            .as_mut()
            .unwrap()
            .module = "user-module".into();
        assert!(
            require_signature_parameter_callable(&signature, &source, &open, &mut Vec::new())
                .is_err()
        );
    }

    #[test]
    fn exact_callable_parameter_path_ignores_descendant_census_exhaustion() {
        let mut signature = transcript().selected_signature.unwrap();
        signature.parameters[0].callable_paths[0].subtree_enumerated = false;
        let source = ValueSource::Parameter {
            index: 0,
            path: vec!["callback".into()],
        };
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "test".into(),
            reason: reason.into(),
        };
        assert!(
            require_signature_parameter_callable(&signature, &source, &open, &mut Vec::new())
                .is_ok(),
            "the parameter's exact callable shape does not require its descendants"
        );
        assert!(require_all_callable_paths_closed(&signature, &open).is_err());

        let absent = &mut signature.parameters[0].callable_paths[0];
        absent.presence = PathPresence::Absent;
        absent.callability = Callability::Unknown;
        absent.constructability = typefacts::InvocationConstructability::Unknown;
        absent.subtree_enumerated = true;
        assert!(
            require_signature_parameter_callable(&signature, &source, &open, &mut Vec::new())
                .is_err()
        );
        assert!(require_all_callable_paths_closed(&signature, &open).is_ok());
    }

    fn verify_bound_recursive(
        proof: &ScheduledProofDemand,
        transcript: &InvocationTranscript,
        operation: &str,
    ) -> Result<Vec<String>, TypeFactsCertificationError> {
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: proof.id.clone(),
            reason: reason.into(),
        };
        let mut sites = Vec::new();
        require_recursive_subject(
            proof,
            transcript.selected_signature.as_ref().unwrap(),
            Some(operation),
            &open,
            &mut sites,
        )?;
        Ok(sites)
    }

    #[test]
    fn family_checks_keep_open_premises_local_and_refuse_unsupported_cardinality() {
        let complete = transcript();
        let selected = proof(ProofFamily::SelectedSignature, selected_subject());
        let selected_sites = verify_family(&selected, &complete).unwrap();
        assert!(
            selected_sites.iter().any(|site| site.contains(":2")),
            "overload ordinal must survive in the concrete site identity"
        );

        let mut unknown_spread = complete.clone();
        unknown_spread.bindings[0].disposition = ArgumentBindingDisposition::UnknownLengthSpread;
        unknown_spread.bindings[0].slots.clear();
        unknown_spread.bindings[0].possible = Some(typefacts::FormalRange {
            start: 0,
            end_exclusive: None,
            unbounded: true,
        });
        let spread = proof(ProofFamily::RestSpreadCoverage, selected_subject());
        assert!(matches!(
            verify_family(&spread, &unknown_spread),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));

        let mut escaped = complete.clone();
        escaped.parameter_uses.push(typefacts::ParameterUse {
            parameter_index: 0,
            binding_path: Vec::new(),
            location: escaped.location.clone(),
            reach: Reachability::Reachable,
            kind: ParameterUseKind::UnknownEscape,
            alias: false,
            captured: false,
        });
        let reachability = proof(ProofFamily::OperationReachability, selected_subject());
        assert!(matches!(
            verify_family(&reachability, &escaped),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));

        let cardinality = proof(ProofFamily::OperationCardinality, selected_subject());
        assert!(matches!(
            verify_family(&cardinality, &complete),
            Err(TypeFactsCertificationError::UnsupportedDemand { .. })
        ));
    }

    #[test]
    fn generic_control_flow_cannot_certify_an_unbound_operation_subject() {
        let operation = proof(
            ProofFamily::OperationReachability,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::Operation {
                artifact_case: "browser".into(),
                export: "run".into(),
                operation: "different-operation".into(),
                has_cardinality: false,
            }),
        );
        assert!(matches!(
            verify_family(&operation, &transcript()),
            Err(TypeFactsCertificationError::UnsupportedDemand { .. })
        ));
    }

    #[test]
    fn complete_partition_cannot_certify_an_unbound_guard_ordinal() {
        let guard = proof(
            ProofFamily::GuardPartition,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::GuardCase {
                artifact_case: "browser".into(),
                export: "run".into(),
                ordinal: 99,
            }),
        );
        assert!(matches!(
            verify_family(&guard, &transcript()),
            Err(TypeFactsCertificationError::UnsupportedDemand { .. })
        ));
    }

    #[test]
    fn selected_call_result_cannot_certify_an_unbound_operation_output() {
        let operation_output = proof(
            ProofFamily::RecursiveValueShape,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                artifact_case: "browser".into(),
                export: "run".into(),
                root: ValueRoot::OperationOutput {
                    operation: solid_reactive_ir::contract_semantics::OperationId(
                        "different-operation".into(),
                    ),
                },
                path: solid_reactive_ir::contract_semantics::ValuePath(Vec::new()),
                callable: DemandedCallability::NonCallable,
            }),
        );
        assert!(matches!(
            verify_family(&operation_output, &transcript()),
            Err(TypeFactsCertificationError::UnsupportedDemand { .. })
        ));
        assert!(matches!(
            verify_bound_recursive(&operation_output, &transcript(), "invoke"),
            Err(TypeFactsCertificationError::SubjectMismatch { .. })
        ));
    }

    #[test]
    fn exact_operation_path_ignores_descendant_census_exhaustion() {
        let mut operation = operation(
            "invoke",
            OperationKind::Invoke,
            per_call_cardinality(Some(0)),
        );
        operation.inputs.push(
            solid_reactive_ir::contract_semantics::ValueShape::Parameter {
                index: 0,
                path: Vec::new(),
            },
        );
        let exported = export_semantics(Vec::new(), vec![operation]);
        let export_transcript = export_value_transcript(json!({}));
        let mut signature = transcript().selected_signature.unwrap();
        signature.parameters[0].callable_paths[0].subtree_enumerated = false;
        let proof = proof(
            ProofFamily::RecursiveValueShape,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                artifact_case: "browser".into(),
                export: "run".into(),
                root: ValueRoot::OperationInput {
                    operation: solid_reactive_ir::contract_semantics::OperationId("invoke".into()),
                    index: 0,
                },
                path: solid_reactive_ir::contract_semantics::ValuePath(vec![
                    ValuePathSegment::ObjectProperty("callback".into()),
                ]),
                callable: DemandedCallability::Callable,
            }),
        );
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "test".into(),
            reason: reason.into(),
        };
        let mut sites = Vec::new();
        assert!(
            require_operation_recursive_signature(
                &proof,
                &export_transcript,
                &exported,
                &ValueRoot::OperationInput {
                    operation: solid_reactive_ir::contract_semantics::OperationId("invoke".into()),
                    index: 0,
                },
                &solid_reactive_ir::contract_semantics::ValuePath(vec![
                    ValuePathSegment::ObjectProperty("callback".into()),
                ]),
                DemandedCallability::Callable,
                &signature,
                &open,
                &mut sites,
            )
            .is_ok(),
            "the operation path's exact shape does not require its descendants"
        );
        assert!(require_all_callable_paths_closed(&signature, &open).is_err());

        let absent = &mut signature.parameters[0].callable_paths[0];
        absent.presence = PathPresence::Absent;
        absent.callability = Callability::Unknown;
        absent.constructability = typefacts::InvocationConstructability::Unknown;
        absent.subtree_enumerated = true;
        assert!(
            require_operation_recursive_signature(
                &proof,
                &export_transcript,
                &exported,
                &ValueRoot::OperationInput {
                    operation: solid_reactive_ir::contract_semantics::OperationId("invoke".into()),
                    index: 0,
                },
                &solid_reactive_ir::contract_semantics::ValuePath(vec![
                    ValuePathSegment::ObjectProperty("callback".into()),
                ]),
                DemandedCallability::Unknown,
                &signature,
                &open,
                &mut Vec::new(),
            )
            .is_err()
        );
        assert!(require_all_callable_paths_closed(&signature, &open).is_ok());
    }

    #[test]
    fn recursive_open_sibling_does_not_contaminate_an_exact_path() {
        let mut exact = transcript();
        let parameter = &mut exact.selected_signature.as_mut().unwrap().parameters[0];
        parameter.callable_paths.push(typefacts::CallablePathFact {
            alternative: 0,
            path: vec![typefacts::PathSegment {
                kind: PathSegmentKind::Property,
                property: "unknownSibling".into(),
                index: None,
            }],
            presence: PathPresence::Unknown,
            callability: Callability::Unknown,
            constructability: typefacts::InvocationConstructability::Unknown,
            declaration: None,
            complete: false,
            subtree_enumerated: false,
            open_reasons: vec!["unresolvedGeneric".into()],
        });
        let recursive = proof(
            ProofFamily::RecursiveValueShape,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                artifact_case: "browser".into(),
                export: "run".into(),
                root: ValueRoot::OperationInput {
                    operation: solid_reactive_ir::contract_semantics::OperationId("invoke".into()),
                    index: 0,
                },
                path: solid_reactive_ir::contract_semantics::ValuePath(vec![
                    ValuePathSegment::ObjectProperty("callback".into()),
                ]),
                callable: DemandedCallability::Callable,
            }),
        );
        assert!(verify_bound_recursive(&recursive, &exact, "invoke").is_ok());

        let mut depth_cut = transcript();
        depth_cut.selected_signature.as_mut().unwrap().parameters[0].callable_paths[0]
            .subtree_enumerated = false;
        assert!(verify_bound_recursive(&recursive, &depth_cut, "invoke").is_ok());
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "test".into(),
            reason: reason.into(),
        };
        assert!(
            require_all_callable_paths_closed(
                depth_cut.selected_signature.as_ref().unwrap(),
                &open,
            )
            .is_err()
        );

        let mut absent_alternative = transcript();
        let absent = &mut absent_alternative
            .selected_signature
            .as_mut()
            .unwrap()
            .parameters[0]
            .callable_paths[0];
        absent.presence = PathPresence::Absent;
        absent.callability = Callability::Unknown;
        absent.constructability = typefacts::InvocationConstructability::Unknown;
        assert!(verify_bound_recursive(&recursive, &absent_alternative, "invoke").is_err());
        assert!(
            require_all_callable_paths_closed(
                absent_alternative.selected_signature.as_ref().unwrap(),
                &open,
            )
            .is_ok(),
            "a proven-absent alternative is a closed census entry"
        );

        assert!(
            serde_json::from_value::<typefacts::CallablePathFact>(json!({
                "alternative": 0,
                "presence": "required",
                "callability": "callable",
                "constructability": "nonConstructable",
                "complete": true
            }))
            .is_err(),
            "protocol-8 callable paths without subtreeEnumerated must fail closed"
        );

        let unresolved = proof(
            ProofFamily::RecursiveValueShape,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                artifact_case: "browser".into(),
                export: "run".into(),
                root: ValueRoot::OperationInput {
                    operation: solid_reactive_ir::contract_semantics::OperationId("invoke".into()),
                    index: 0,
                },
                path: solid_reactive_ir::contract_semantics::ValuePath(vec![
                    ValuePathSegment::ObjectProperty("unknownSibling".into()),
                ]),
                callable: DemandedCallability::Callable,
            }),
        );
        assert!(matches!(
            verify_bound_recursive(&unresolved, &exact, "invoke"),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));
    }

    #[test]
    fn unrelated_complete_callable_path_cannot_certify_the_demanded_path() {
        let demanded = proof(
            ProofFamily::CallablePath,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                artifact_case: "browser".into(),
                export: "run".into(),
                root: ValueRoot::OperationInput {
                    operation: solid_reactive_ir::contract_semantics::OperationId("invoke".into()),
                    index: 0,
                },
                path: solid_reactive_ir::contract_semantics::ValuePath(vec![
                    ValuePathSegment::ObjectProperty("differentCallback".into()),
                ]),
                callable: DemandedCallability::Callable,
            }),
        );
        assert!(matches!(
            verify_family(&demanded, &transcript()),
            Err(TypeFactsCertificationError::UnsupportedDemand { .. })
        ));
    }

    #[test]
    fn an_export_value_demand_cannot_be_answered_by_the_selected_call_result() {
        let export_value = proof(
            ProofFamily::RecursiveValueShape,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                artifact_case: "browser".into(),
                export: "run".into(),
                root: ValueRoot::Export,
                path: solid_reactive_ir::contract_semantics::ValuePath(Vec::new()),
                callable: DemandedCallability::NonCallable,
            }),
        );
        assert!(matches!(
            verify_family(&export_value, &transcript()),
            Err(TypeFactsCertificationError::UnsupportedDemand { .. })
        ));
    }

    #[test]
    fn unknown_root_callability_cannot_certify_a_non_callable_operation_output() {
        let mut locally_open_but_non_callable = transcript();
        locally_open_but_non_callable
            .selected_signature
            .as_mut()
            .unwrap()
            .result
            .open_reasons
            .push("openIndex".into());
        let operation_output = proof(
            ProofFamily::RecursiveValueShape,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                artifact_case: "browser".into(),
                export: "run".into(),
                root: ValueRoot::OperationOutput {
                    operation: solid_reactive_ir::contract_semantics::OperationId("invoke".into()),
                },
                path: solid_reactive_ir::contract_semantics::ValuePath(Vec::new()),
                callable: DemandedCallability::NonCallable,
            }),
        );
        assert!(
            verify_bound_recursive(&operation_output, &locally_open_but_non_callable, "invoke")
                .is_ok(),
            "an unrelated open value domain cannot contaminate exact root callability"
        );

        let mut unknown = transcript();
        unknown
            .selected_signature
            .as_mut()
            .unwrap()
            .result
            .callability = Callability::Unknown;
        assert!(matches!(
            verify_bound_recursive(&operation_output, &unknown, "invoke"),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));
    }

    #[test]
    fn default_export_display_alias_cannot_stand_in_for_export_identity() {
        let selected = proof(ProofFamily::SelectedSignature, selected_subject());
        let transcript = transcript();
        let signature = transcript.selected_signature.as_ref().unwrap();
        assert!(matches!(
            verify_declaration_export_identity(&selected, "default", signature),
            Err(TypeFactsCertificationError::UnsupportedDemand { .. })
        ));
    }

    #[test]
    fn anonymous_default_declarations_never_gain_synthetic_identity() {
        assert!(matches!(
            verify_snapshot_declaration_name("sha256:test", "default", "", "/pkg/index.d.ts"),
            Err(TypeFactsCertificationError::SubjectMismatch { reason, .. })
                if reason == "canonical default-export target has no declaration identity"
        ));
        assert!(
            verify_snapshot_declaration_name(
                "sha256:test",
                "default",
                "createX",
                "/pkg/index.d.ts",
            )
            .is_ok()
        );
        for path in [
            "/pkg/query/ir.d.mts",
            "/pkg/query/ir.d.cts",
            "/pkg/query/ir.d.ts",
            "/pkg/query/ir.mts",
            "/pkg/query/ir.cts",
            "/pkg/query/ir.tsx",
            "/pkg/query/ir.ts",
            "/pkg/query/ir.mjs",
            "/pkg/query/ir.cjs",
            "/pkg/query/ir.jsx",
            "/pkg/query/ir.js",
        ] {
            assert!(
                verify_snapshot_declaration_name("sha256:test", "*", "\"/pkg/query/ir\"", path,)
                    .is_ok()
            );
        }
        assert!(
            verify_snapshot_declaration_name(
                "sha256:test",
                "createX",
                "createX",
                "/pkg/index.d.ts",
            )
            .is_ok()
        );
        assert!(matches!(
            verify_snapshot_declaration_name(
                "sha256:test",
                "createX",
                "createY",
                "/pkg/index.d.ts",
            ),
            Err(TypeFactsCertificationError::SubjectMismatch { reason, .. })
                if reason == "resolved value declaration name disagrees with snapshot export replay"
        ));
        assert!(
            verify_snapshot_declaration_name(
                "sha256:test",
                "*",
                "\"/pkg/query/ir\"",
                "/pkg/query/ir.d.ts",
            )
            .is_ok()
        );
        for (name, path) in [
            ("ir", "/pkg/query/ir.d.ts"),
            ("\"/pkg/query/other\"", "/pkg/query/ir.d.ts"),
            ("\"/pkg/query/ir\"", "/pkg/query/other.d.ts"),
        ] {
            assert!(matches!(
                verify_snapshot_declaration_name("sha256:test", "*", name, path),
                Err(TypeFactsCertificationError::SubjectMismatch { .. })
            ));
        }
    }

    #[test]
    fn exact_harness_preserves_namespace_import_identity() {
        let mut harness = String::new();
        let namespace = append_exact_harness_import(&mut harness, 0, "./query/ir", "*").unwrap();
        let default = append_exact_harness_import(&mut harness, 1, "./default", "default").unwrap();
        let named = append_exact_harness_import(&mut harness, 2, "./named", "value").unwrap();
        assert_eq!(namespace, "__solid_checker_export_0");
        assert_eq!(default, "__solid_checker_export_1");
        assert_eq!(named, "__solid_checker_export_2");
        assert_eq!(
            harness,
            concat!(
                "import * as __solid_checker_export_0 from \"./query/ir\";\n",
                "import __solid_checker_export_1 from \"./default\";\n",
                "import { value as __solid_checker_export_2 } from \"./named\";\n",
            )
        );

        let before = harness.clone();
        assert!(matches!(
            append_exact_harness_import(&mut harness, 3, "./bad", "not-valid!"),
            Err(TypeFactsCertificationError::UnsupportedDemand { .. })
        ));
        assert_eq!(
            harness, before,
            "an invalid selector must not alter the harness"
        );
    }

    #[test]
    fn mutable_external_declaration_cannot_enter_the_source_census() {
        let sources = vec![typefacts::TranscriptSourceDigest {
            path: "/project/node_modules/dependency/index.d.ts".into(),
            sha256: format!("sha256:{:064x}", 8).try_into().unwrap(),
        }];
        assert!(matches!(
            reject_unauthenticated_external_sources(
                &["/project/node_modules/fixture-package/".into()],
                &sources,
            ),
            Err(TypeFactsCertificationError::SourceCensus(_))
        ));
    }

    #[test]
    fn source_census_uses_longest_materialized_root_prefix() {
        let mut roots = vec![
            "/project/node_modules/solid-recharts/".to_owned(),
            "/project/node_modules/csstype/".to_owned(),
            "/project/node_modules/solid-recharts/node_modules/csstype/".to_owned(),
        ];
        roots.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));

        let vendored =
            "/project/node_modules/solid-recharts/dist/browser/node_modules/csstype/index.d.ts";
        let (owner, relative) = strip_materialized_source_root(vendored, &roots).unwrap();
        assert_eq!(roots[owner], "/project/node_modules/solid-recharts/");
        assert_eq!(relative, "dist/browser/node_modules/csstype/index.d.ts");

        let nested = "/project/node_modules/solid-recharts/node_modules/csstype/index.d.ts";
        let (owner, relative) = strip_materialized_source_root(nested, &roots).unwrap();
        assert_eq!(
            roots[owner],
            "/project/node_modules/solid-recharts/node_modules/csstype/"
        );
        assert_eq!(relative, "index.d.ts");

        let hoisted = "/project/node_modules/csstype/index.d.ts";
        let (owner, relative) = strip_materialized_source_root(hoisted, &roots).unwrap();
        assert_eq!(roots[owner], "/project/node_modules/csstype/");
        assert_eq!(relative, "index.d.ts");

        assert!(
            strip_materialized_source_root("/project/node_modules/unplanned/index.d.ts", &roots,)
                .is_none()
        );

        let normalized = normalized_source_root(Path::new("/project/node_modules/pkg"));
        assert_eq!(normalized, "/project/node_modules/pkg/");
        for outside in [
            "/project/node_modules/pkg-other/index.d.ts",
            "/project/node_modules/pkg2/index.d.ts",
        ] {
            assert!(
                strip_materialized_source_root(outside, std::slice::from_ref(&normalized))
                    .is_none()
            );
        }

        let authenticated_hoisted = normalized_source_root(Path::new("/repo/node_modules/pkg"));
        assert_eq!(authenticated_hoisted, "/repo/node_modules/pkg/");
        assert!(
            strip_materialized_source_root(
                "/repo/node_modules/pkg/index.d.ts",
                std::slice::from_ref(&authenticated_hoisted),
            )
            .is_some()
        );
        assert!(
            strip_materialized_source_root(
                "/repo/packages/app/node_modules/pkg/index.d.ts",
                std::slice::from_ref(&authenticated_hoisted),
            )
            .is_none()
        );

        let pnpm_roots = authenticated_source_root_paths(
            "/repo/node_modules/pkg",
            Some("/repo/.pnpm/pkg@1.0.0/node_modules/pkg"),
        )
        .into_iter()
        .map(|root| normalized_source_root(&root))
        .collect::<Vec<_>>();
        assert!(
            strip_materialized_source_root(
                "/repo/.pnpm/pkg@1.0.0/node_modules/pkg/index.d.ts",
                &pnpm_roots,
            )
            .is_some()
        );
        assert!(
            strip_materialized_source_root(
                "/repo/.pnpm/pkg@2.0.0/node_modules/pkg/index.d.ts",
                &pnpm_roots,
            )
            .is_none()
        );
    }

    #[test]
    fn source_census_refuses_ambiguous_materialized_roots() {
        let first = source_snapshot("sha256:first", "sha256:archive", &[]);
        let same = source_snapshot("sha256:first", "sha256:archive", &[]);
        let different_bytes = source_snapshot("sha256:second", "sha256:archive", &[]);
        let different_archive = source_snapshot("sha256:first", "sha256:other-archive", &[]);

        let root = |snapshot, dependency| SnapshotSourceRoot {
            path: "/project/node_modules/pkg/".into(),
            evidence_prefix: "/node_modules/pkg/".into(),
            snapshot,
            dependency,
        };
        let mut identical = vec![root(&first, false), root(&same, true)];
        deduplicate_snapshot_source_roots(&mut identical).unwrap();
        assert_eq!(identical.len(), 1);
        assert!(!identical[0].dependency);

        for conflicting in [&different_bytes, &different_archive] {
            let mut roots = vec![root(&first, true), root(conflicting, true)];
            assert!(matches!(
                deduplicate_snapshot_source_roots(&mut roots),
                Err(TypeFactsCertificationError::SourceCensus(_))
            ));
        }
    }

    #[test]
    fn source_census_never_falls_back_after_exact_root_selection() {
        let bytes = b"export type Value = string;";
        let nested = source_snapshot("sha256:nested", "sha256:nested-archive", &[]);
        let owner = source_snapshot(
            "sha256:owner",
            "sha256:owner-archive",
            &[("node_modules/dep/index.d.ts", bytes)],
        );
        let roots = vec![
            "/project/node_modules/owner/node_modules/dep/".to_owned(),
            "/project/node_modules/owner/".to_owned(),
        ];
        let source = "/project/node_modules/owner/node_modules/dep/index.d.ts";
        let (selected, relative) = strip_materialized_source_root(source, &roots).unwrap();
        assert_eq!(selected, 0);
        assert!(matches!(
            verify_snapshot_source_digest(&nested, relative, &digest(bytes)),
            Err(TypeFactsCertificationError::SourceCensus(reason))
                if reason.contains("outside the snapshot")
        ));
        assert!(
            verify_snapshot_source_digest(&owner, "node_modules/dep/index.d.ts", &digest(bytes),)
                .is_ok(),
            "matching bytes under the shorter root cannot answer for the selected nested root"
        );
        assert!(matches!(
            verify_snapshot_source_digest(
                &owner,
                "node_modules/dep/index.d.ts",
                &digest(b"different"),
            ),
            Err(TypeFactsCertificationError::SourceCensus(reason))
                if reason.contains("digest differs")
        ));
    }

    #[test]
    fn export_value_envelope_defers_only_unresolved_modules_to_local_completeness() {
        assert!(validate_export_envelope_open_reasons(&[]).is_ok());
        assert!(validate_export_envelope_open_reasons(&["unresolvedModule".into()]).is_ok());
        assert!(matches!(
            validate_export_envelope_open_reasons(&[
                "unresolvedModule".into(),
                "sourceUnavailable".into(),
            ]),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));
    }

    #[test]
    fn composite_namespace_dispatch_and_conditional_guard_stay_open() {
        let mut composite = transcript();
        composite.targets = Some(typefacts::CallTargetSet {
            exhaustive: true,
            candidates: Vec::new().into(),
        });
        let selected = proof(ProofFamily::SelectedSignature, selected_subject());
        assert!(matches!(
            verify_family(&selected, &composite),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));

        let mut conditional = transcript();
        conditional.selected_signature.as_mut().unwrap().parameters[0]
            .value
            .partitions[0]
            .complete = false;
        let guard = proof(ProofFamily::GuardPartition, selected_subject());
        // The exact control-flow partition remains available, so an unrelated
        // open value partition does not contaminate that sibling.
        assert!(verify_family(&guard, &conditional).is_ok());
        conditional.control_flow.as_mut().unwrap().branches[0].partitions[0].complete = false;
        assert!(matches!(
            verify_family(&guard, &conditional),
            Err(TypeFactsCertificationError::FamilyOpen { .. })
        ));
    }
}
