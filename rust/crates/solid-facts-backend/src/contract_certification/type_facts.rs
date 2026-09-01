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
        if schedule.demand_graph_root != plan.demand_graph().root().as_str() {
            return Err(TypeFactsCertificationError::IdentityMismatch);
        }
        if !schedule.export_values.is_empty() {
            return Err(TypeFactsCertificationError::IdentityMismatch);
        }
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
        if schedule.demand_graph_root != plan.demand_graph().root().as_str()
            || !schedule.invocations.is_empty()
        {
            return Err(TypeFactsCertificationError::IdentityMismatch);
        }
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
        .ok_or(TypeFactsCertificationError::IdentityMismatch)
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
        if !targets.insert(marker) {
            return Err(TypeFactsCertificationError::IdentityMismatch);
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

fn acquire_and_verify_export_values_batch_with_dependencies(
    plans: &[&CertificationPlan],
    dependencies: &[&CertificationPlan],
    sources: &[super::dependencies::VerifiedGraphSourcePackage],
    pin: &TypeFactsProducerPin,
) -> Result<Vec<VerifiedTypeFactsEvidence>, TypeFactsCertificationError> {
    let first = plans
        .first()
        .copied()
        .ok_or(TypeFactsCertificationError::IdentityMismatch)?;
    if plans.iter().any(|plan| {
        plan.snapshot_root() != first.snapshot_root()
            || plan.snapshot.package_name() != first.snapshot.package_name()
    }) {
        return Err(TypeFactsCertificationError::IdentityMismatch);
    }
    // This compatibility check consumes only the verifier-retained demand
    // graph and is repeated by schedule construction below. An incompatible
    // graph can never produce an export-value schedule, so reject its exact
    // demand before copying authenticated package bytes into a private project.
    preflight_export_value_plans(plans)
        .map_err(|error| error.at_stage("export-value schedule derivation"))?;
    let project = PrivateTypeFactsProject::materialize(first, dependencies, plans, sources)
        .map_err(|error| error.at_stage("private project materialization"))?;
    let schedules = derive_export_value_schedules(plans, &project, false)
        .map_err(|error| error.at_stage("export-value schedule derivation"))?;
    let project_id = project.project_id().to_str().ok_or_else(|| {
        TypeFactsCertificationError::ProducerProvenance(
            "private Type Facts project path is not valid UTF-8".into(),
        )
    })?;
    let mut session = TypeFactsCertificationSession::open(pin, project_id)
        .map_err(|error| error.at_stage("pinned producer launch"))?;
    plans
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
            )
            .map_err(|error| error.at_stage("live export-value verification"))
        })
        .collect()
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
    let project = PrivateTypeFactsProject::materialize_with_source_refs(
        project_root,
        &dependencies,
        &program_plans,
        &source_refs,
    )
    .map_err(|error| error.at_stage("private graph project materialization"))?;
    let schedules = derive_export_value_schedules(&plans, &project, true)
        .map_err(|error| error.at_stage("graph export-value schedule derivation"))?;
    let project_id = project.project_id().to_str().ok_or_else(|| {
        TypeFactsCertificationError::ProducerProvenance(
            "private Type Facts graph project path is not valid UTF-8".into(),
        )
    })?;
    let mut session = TypeFactsCertificationSession::open(pin, project_id)
        .map_err(|error| error.at_stage("pinned graph producer launch"))?;
    requests
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
            )
            .map_err(|error| {
                error.at_graph_node(request.plan, "live graph export-value verification")
            })
        })
        .collect()
}

struct PrivateTypeFactsProject {
    root: PathBuf,
    project_id: PathBuf,
    harness: PathBuf,
    package_roots: std::collections::BTreeMap<(String, String), PathBuf>,
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
        materialize_snapshot(&plan.snapshot, &package_root)?;
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
            materialize_snapshot(&dependency.snapshot, &target)?;
            package_roots.insert(private_project_plan_key(dependency), target);
        }
        for source in sources {
            let target = private_project_package_target(
                &root,
                &package_root,
                original_package_root,
                Path::new(&source.installed_package_root),
                source.snapshot.package_name(),
            );
            materialize_snapshot(&source.snapshot, &target)?;
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
                .ok_or(TypeFactsCertificationError::IdentityMismatch)?;
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
                "types": []
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
        })
    }

    fn project_id(&self) -> &Path {
        &self.project_id
    }

    fn package_root(&self, plan: &CertificationPlan) -> Result<&Path, TypeFactsCertificationError> {
        self.package_roots
            .get(&private_project_plan_key(plan))
            .map(PathBuf::as_path)
            .ok_or(TypeFactsCertificationError::IdentityMismatch)
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

fn materialize_snapshot(
    snapshot: &super::ArtifactSnapshot,
    package_root: &Path,
) -> Result<(), TypeFactsCertificationError> {
    fs::create_dir_all(package_root)?;
    for (relative, bytes) in snapshot.files.iter() {
        let target = package_root.join(relative);
        write_immutable_project_file(&target, bytes)?;
    }
    Ok(())
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
type ExportResolutionVariant = (String, String, String);
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
            let (declaration_path, declaration_export) = plan
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
        if declaration_export != "default" && !is_ecmascript_identifier(declaration_export) {
            return Err(TypeFactsCertificationError::UnsupportedDemand {
                demand: "export-value-schedule".into(),
                reason: format!(
                    "declaration export name {declaration_export:?} cannot be imported by the exact harness"
                ),
            });
        }
        *local = format!("__solid_checker_export_{index}");
        let quoted = serde_json::to_string(specifier).map_err(|error| {
            TypeFactsCertificationError::ProducerProvenance(format!(
                "could not encode verifier harness specifier: {error}"
            ))
        })?;
        if declaration_export == "default" {
            harness.push_str(&format!("import {} from {quoted};\n", local));
        } else {
            harness.push_str(&format!(
                "import {{ {declaration_export} as {} }} from {quoted};\n",
                local
            ));
        }
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
    let (declaration_path, declaration_export) = plan
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
    Ok((specifier, declaration_export.to_owned()))
}

fn snapshot_module_harness_specifier(
    project: &PrivateTypeFactsProject,
    plan: &CertificationPlan,
    path: &str,
) -> Result<String, TypeFactsCertificationError> {
    let relative = project
        .package_root(plan)?
        .strip_prefix(&project.root)
        .map_err(|_| TypeFactsCertificationError::IdentityMismatch)?;
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
    if schedule.demand_graph_root != plan.demand_graph().root().as_str()
        || !schedule.export_values.is_empty()
    {
        return Err(TypeFactsCertificationError::IdentityMismatch);
    }
    let identity = live.identity();
    let answer = live.answer();
    if identity.context().snapshot_root() != plan.snapshot_root()
        || identity.context().demand_graph_root() != plan.demand_graph().root().as_str()
        || identity.generation() != answer.envelope.generation
        || identity.project_id() != &*answer.envelope.project_id
        || identity.demand_sha256() != &*answer.envelope.demand_sha256
        || identity.handshake_protocol() != typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL
        || identity.handshake_schema_sha256() != typefacts::v3::TYPE_FACTS_SCHEMA_SHA256
        || identity.handshake_build() != typefacts::v3::TYPE_FACTS_BUILD_ID
    {
        return Err(TypeFactsCertificationError::IdentityMismatch);
    }
    let mut expected_ids = schedule.proof_demand_ids().collect::<Vec<_>>();
    expected_ids.sort();
    let actual_ids = identity
        .context()
        .proof_demand_ids()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if expected_ids != actual_ids || answer.transcripts.len() != schedule.invocations.len() {
        return Err(TypeFactsCertificationError::IdentityMismatch);
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
        &answer.envelope.sources,
        &schedule.verifier_sources,
    )?;

    let mut bindings = Vec::with_capacity(expected_ids.len());
    for (index, scheduled) in schedule.invocations.iter().enumerate() {
        let transcript = &answer.transcripts[index];
        if transcript.location != scheduled.demand.location {
            return Err(TypeFactsCertificationError::IdentityMismatch);
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
    verify_live_export_value_answer_with_dependencies(plan, schedule, live, &[], &[])
}

fn verify_live_export_value_answer_with_dependencies(
    plan: &CertificationPlan,
    schedule: &TypeFactsCertificationSchedule,
    live: &LiveExportValueAnswer,
    dependencies: &[&CertificationPlan],
    sources: &[super::dependencies::VerifiedGraphSourcePackage],
) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
    verify_live_export_value_answer_with_project_census(
        plan,
        schedule,
        live,
        dependencies,
        dependencies,
        sources,
    )
}

fn verify_live_export_value_answer_with_project_census(
    plan: &CertificationPlan,
    schedule: &TypeFactsCertificationSchedule,
    live: &LiveExportValueAnswer,
    dependencies: &[&CertificationPlan],
    census_dependencies: &[&CertificationPlan],
    census_sources: &[super::dependencies::VerifiedGraphSourcePackage],
) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
    if schedule.demand_graph_root != plan.demand_graph().root().as_str()
        || !schedule.invocations.is_empty()
    {
        return Err(TypeFactsCertificationError::IdentityMismatch);
    }
    let identity = live.identity();
    let answer = live.answer();
    if identity.context().snapshot_root() != plan.snapshot_root()
        || identity.context().demand_graph_root() != plan.demand_graph().root().as_str()
        || identity.generation() != answer.envelope.generation
        || identity.project_id() != &*answer.envelope.project_id
        || identity.demand_sha256() != &*answer.envelope.demand_sha256
        || identity.handshake_protocol() != typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL
        || identity.handshake_schema_sha256() != typefacts::v3::TYPE_FACTS_SCHEMA_SHA256
        || identity.handshake_build() != typefacts::v3::TYPE_FACTS_BUILD_ID
    {
        return Err(TypeFactsCertificationError::IdentityMismatch);
    }
    let mut expected_ids = schedule.proof_demand_ids().collect::<Vec<_>>();
    expected_ids.sort();
    let actual_ids = identity
        .context()
        .proof_demand_ids()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if expected_ids != actual_ids || answer.transcripts.len() != schedule.export_values.len() {
        return Err(TypeFactsCertificationError::IdentityMismatch);
    }
    validate_export_envelope_open_reasons(&answer.envelope.open_reasons)?;
    let source_sites = verify_snapshot_source_census(
        plan,
        census_dependencies,
        census_sources,
        &answer.envelope.sources,
        &schedule.verifier_sources,
    )?;
    let certification_sources_root = plan.certification_sources_root();
    let mut bindings = Vec::with_capacity(expected_ids.len());
    for (index, scheduled) in schedule.export_values.iter().enumerate() {
        let transcript = &answer.transcripts[index];
        if transcript.location != scheduled.demand.location {
            return Err(TypeFactsCertificationError::IdentityMismatch);
        }
        match (
            scheduled.demand.implementation_location.as_ref(),
            transcript.implementation.as_ref(),
        ) {
            (Some(expected), Some(actual)) if expected == &actual.location => {}
            (None, None) => {}
            _ => return Err(TypeFactsCertificationError::IdentityMismatch),
        }
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
    let (declaration_path, declaration_export) = plan
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
    let (declaration_path, declaration_export) = plan
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
    if declaration_export == "default" {
        // The verifier-authored harness contains an exact default import for
        // this package/subpath, and its bytes are part of the source census.
        // `target` is the compiler-canonicalized alias target, while the
        // declaration path above is independently replayed from the archive.
        // The target's display name is intentionally irrelevant to the export
        // alias, but an anonymous/unidentified declaration is not authority.
        if actual_name.is_empty() {
            return Err(TypeFactsCertificationError::SubjectMismatch {
                demand: proof.id.clone(),
                reason: "canonical default-export target has no declaration identity".into(),
            });
        }
        return Ok(());
    }
    if actual_name != declaration_export {
        return Err(TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "resolved value declaration name disagrees with snapshot export replay".into(),
        });
    }
    Ok(())
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
            require_parameter_flow(implementation, &source, &open, &mut sites)?;
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
                    require_parameter_flow(implementation, &source, &open, &mut sites)?;
                } else {
                    require_parameter_callback_flow(implementation, &source, &open, &mut sites)?;
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
    let ValueSource::Parameter { index, path } = expected else {
        return false;
    };
    actual.parameter_index == usize::from(*index)
        && actual.path.len() >= path.len()
        && actual.path.iter().zip(path).all(|(actual, expected)| {
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

fn require_parameter_flow(
    implementation: &typefacts::ExportImplementationTranscript,
    source: &ValueSource,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let matching = implementation.calls.iter().filter(|call| {
        implementation_call_is_executed(implementation, call)
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
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    for call in &implementation.calls {
        if !implementation_call_is_executed(implementation, call) {
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
/// callable, and the one such thing the census proves is the value the export
/// returns. So the premise is containment, not mention: the call site must lie
/// within the source range of a callable that a reachable return site provably
/// carries.
///
/// Containment rather than an immediately-enclosing match, because a returned
/// debounced function schedules the callback from an arrow it hands to
/// `setTimeout`; invoking the returned value still reaches that call. What
/// containment refuses is the shape a parameter-index union could not: a call in
/// a closure the implementation never returns, discharged by a returned closure
/// that merely names the same parameter.
fn implementation_call_is_executed(
    implementation: &typefacts::ExportImplementationTranscript,
    call: &typefacts::ImplementationCall,
) -> bool {
    if call.reach != Reachability::Reachable {
        return false;
    }
    if !call.captured {
        return true;
    }
    implementation.control_flow.as_ref().is_some_and(|flow| {
        flow.returns
            .iter()
            .filter(|site| site.reach == Reachability::Reachable)
            .flat_map(|site| site.carried_callables.iter())
            .any(|carried| location_contains(carried, &call.location))
    })
}

/// Whether `outer` spans `inner` in the same file. Byte ranges come from one
/// producer census, so an exact path match is required rather than a suffix.
fn location_contains(outer: &typefacts::Location, inner: &typefacts::Location) -> bool {
    outer.path == inner.path
        && outer.start_byte <= inner.start_byte
        && inner.end_byte <= outer.end_byte
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
        if !fact.complete
            || !fact.open_reasons.is_empty()
            || fact.presence == PathPresence::Unknown
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
    match operation.kind {
        OperationKind::Invoke => {
            let callback = export
                .callbacks()
                .items()
                .iter()
                .find(|callback| callback.operation == operation.id)
                .ok_or_else(|| open("invoke operation has no exact callback source"))?;
            require_parameter_flow(implementation, &callback.from, open, sites)
        }
        OperationKind::Read => {
            let source = operation
                .inputs
                .first()
                .ok_or_else(|| open("read operation has no input"))
                .and_then(parameter_source)?;
            let mut found = false;
            for call in &implementation.calls {
                if call.reach == Reachability::Reachable
                    && !call.captured
                    && call
                        .callee_parameter
                        .as_ref()
                        .is_some_and(|actual| parameter_value_source_matches(actual, &source))
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
            require_owner_operation_call(operation, proof, implementation, open, sites)
        }
        _ => Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: proof.id.clone(),
            reason: "runtime implementation census does not yet bind this operation kind".into(),
        }),
    }?;
    Ok(())
}

fn require_owner_operation_call(
    operation: &solid_reactive_ir::contract_semantics::Operation,
    proof: &ScheduledProofDemand,
    implementation: &typefacts::ExportImplementationTranscript,
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
        if call.reach != Reachability::Reachable
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
        // Implementation evidence that the exact parameter value is itself the
        // callee of a reachable call. That answers a demand which does not
        // assert callability; a demand that does assert it is answered by the
        // callback-flow branch below or by the signature census.
        if path.0.is_empty() && !callable.asserts_callable() {
            let (_, implementation) = require_export_implementation(plan, proof, transcript, open)?;
            if let Some(call) = implementation.calls.iter().find(|call| {
                call.reach == Reachability::Reachable
                    && !call.captured
                    && !call.target.is_empty()
                    && call
                        .callee_parameter
                        .as_ref()
                        .is_some_and(|actual| parameter_value_source_exact(actual, &source))
            }) {
                sites.push(format!(
                    "recursive-operation-parameter:{}:{}:{}:{}",
                    call.location.path,
                    call.location.start_byte,
                    call.location.end_byte,
                    call.target
                ));
                return Ok(());
            }
        }
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
        if path_is_exact_properties && callable.asserts_callable() {
            let (_, implementation) = require_export_implementation(plan, proof, transcript, open)?;
            require_parameter_callback_flow(implementation, &source, open, sites)?;
            return Ok(());
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
    if (!fact.complete || !fact.open_reasons.is_empty() || fact.presence == PathPresence::Unknown)
        && !callable_local_closure
    {
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

fn require_export_callable_paths_closed(
    transcript: &ExportValueTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    for path in &transcript.callable_paths {
        if !path.complete || !path.open_reasons.is_empty() || path.presence == PathPresence::Unknown
        {
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
    if !fact.complete || !fact.open_reasons.is_empty() || fact.presence == PathPresence::Unknown {
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

    let mut dependency_markers = dependencies
        .iter()
        .map(|dependency| {
            (
                private_project_package_marker(
                    plan,
                    &dependency.resolved_import.package_root,
                    dependency.snapshot.package_name(),
                ),
                &dependency.snapshot,
            )
        })
        .chain(graph_sources.iter().map(|source| {
            (
                private_project_package_marker(
                    plan,
                    &source.installed_package_root,
                    source.snapshot.package_name(),
                ),
                &source.snapshot,
            )
        }))
        .collect::<Vec<_>>();
    dependency_markers.sort_by(|(left, _), (right, _)| {
        right.len().cmp(&left.len()).then_with(|| left.cmp(right))
    });
    reject_unauthenticated_external_sources(&package_marker, &dependency_markers, sources)?;

    // Every source attributed to this package must come from the immutable
    // snapshot. This catches a sibling/ancestor installation silently winning
    // resolution even when the demanded declaration happened to share bytes.
    for source in sources {
        let normalized = source.path.replace('\\', "/");
        let matched = dependency_markers
            .iter()
            .map(|(marker, snapshot)| (marker, *snapshot))
            .chain(std::iter::once((&package_marker, &plan.snapshot)))
            .find_map(|(marker, snapshot)| {
                normalized
                    .rsplit_once(marker)
                    .map(|(_, relative)| (snapshot, relative))
            });
        let Some((snapshot, relative)) = matched else {
            continue;
        };
        let bytes = snapshot.read(relative).ok_or_else(|| {
            TypeFactsCertificationError::SourceCensus(format!(
                "producer consulted package source outside the snapshot: {relative}"
            ))
        })?;
        let expected = format!("sha256:{:x}", Sha256::digest(bytes));
        if expected != source.sha256.as_ref() {
            return Err(TypeFactsCertificationError::SourceCensus(format!(
                "producer source digest differs from snapshot: {relative}"
            )));
        }
        if let Some((marker, snapshot)) = dependency_markers
            .iter()
            .find(|(marker, _)| normalized.contains(marker))
        {
            // `marker` is this package's project-relative root and `relative`
            // its path inside the snapshot, so together they are the file's
            // project-relative path.
            sites.push(format!(
                "typefacts-source-snapshot:{}:{marker}{relative}:{}",
                snapshot.provenance_root(),
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
    package_marker: &str,
    dependencies: &[(String, &super::ArtifactSnapshot)],
    sources: &[typefacts::TranscriptSourceDigest],
) -> Result<(), TypeFactsCertificationError> {
    for source in sources {
        let normalized = source.path.replace('\\', "/");
        if normalized.contains("/node_modules/")
            && !normalized.contains(package_marker)
            && !dependencies
                .iter()
                .any(|(marker, _)| normalized.contains(marker))
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
        if !path.complete || !path.open_reasons.is_empty() || path.presence == PathPresence::Unknown
        {
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
    if !fact.complete || !fact.open_reasons.is_empty() || fact.presence == PathPresence::Unknown {
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

        let directory = create_private_directory()?;
        let path = directory.join("solid-typefacts");
        let image = Self { directory, path };
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

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateExecutionImage {
    fn drop(&mut self) {
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

fn copy_and_hash(
    source: &Path,
    destination: &Path,
) -> Result<SourceHash, TypeFactsCertificationError> {
    let mut source = File::open(source)?;
    let mut destination = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
        destination.write_all(&buffer[..read])?;
    }
    destination.sync_all()?;
    Ok(SourceHash::parse(format!("sha256:{:x}", hash.finalize()))?)
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
    #[error("Type Facts live-session identity does not match the certification plan")]
    IdentityMismatch,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use solid_reactive_ir::contract_semantics::{
        certification::proof_policy_2, solid2_rc3::conformance_corpus,
    };

    fn digest(bytes: &[u8]) -> String {
        format!("sha256:{:x}", Sha256::digest(bytes))
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
                        "complete": true
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
                    "carriedCallables": [
                        {"path": "/project/index.js", "startByte": 5, "endByte": 40}
                    ]
                }]},
                "calls": [{
                    "location": {"path": "/project/index.js", "startByte": 10, "endByte": 15},
                    "reach": "reachable",
                    "calleeParameter": {"parameterIndex": 0},
                    "captured": true
                }]
            }))
            .unwrap();
        assert!(implementation_call_is_executed(
            &implementation,
            &implementation.calls[0]
        ));

        // A call in a closure the implementation never returns is outside every
        // carried range, and no amount of what the returned closure *mentions*
        // puts it back inside one.
        implementation.calls[0].location.start_byte = 60;
        implementation.calls[0].location.end_byte = 65;
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0]
        ));
        // Same bytes, another file: containment is per file, never a suffix.
        implementation.calls[0].location.start_byte = 10;
        implementation.calls[0].location.end_byte = 15;
        implementation.calls[0].location.path = "/project/other.js".into();
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0]
        ));
        implementation.calls[0].location.path = "/project/index.js".into();

        implementation.control_flow.as_mut().unwrap().returns[0]
            .carried_callables
            .clear();
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0]
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
            &implementation.calls[0]
        ));

        implementation.calls[0].captured = false;
        assert!(implementation_call_is_executed(
            &implementation,
            &implementation.calls[0]
        ));
        implementation.calls[0].reach = Reachability::Unknown;
        assert!(!implementation_call_is_executed(
            &implementation,
            &implementation.calls[0]
        ));
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
                "complete": true
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
    fn mutable_external_declaration_cannot_enter_the_source_census() {
        let sources = vec![typefacts::TranscriptSourceDigest {
            path: "/project/node_modules/dependency/index.d.ts".into(),
            sha256: format!("sha256:{:064x}", 8).try_into().unwrap(),
        }];
        assert!(matches!(
            reject_unauthenticated_external_sources(
                "/node_modules/fixture-package/",
                &[],
                &sources,
            ),
            Err(TypeFactsCertificationError::SourceCensus(_))
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
