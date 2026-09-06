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
    CardinalityScope, ClaimDomain, ClaimPath, OperationKind, ReactiveRole, Requirement,
    SemanticClaimPath, UpperBound, ValueClaimDomain, ValuePathSegment, ValueRoot, ValueShape,
    ValueSource,
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
/// How many leftover `(pid, counter)` directories a process skips past before
/// giving up on naming its private project.
const PRIVATE_PROJECT_NAME_ATTEMPTS: u32 = 1024;

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

    /// The pinned producer image on disk. The probe harness watches these
    /// bytes across a probe run so a run that altered a producer input refuses
    /// its gate instead of being absorbed into the transaction.
    pub(crate) fn path(&self) -> &Path {
        &self.path
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

/// The export-value transcripts of *this* answer, addressable by the export
/// they were scheduled for.
///
/// Composition needs a second export's census, and only this answer carries
/// one: a transcript is scheduled per exported value, and the scheduled
/// demands name which export each one is for. Nothing here reaches outside the
/// answer — a composition premise that could not find the target's transcript
/// refuses rather than looking anywhere else, so the evidence for a composed
/// row is always the same session's, under the same producer identity, bound
/// into the same evidence root.
#[derive(Clone, Copy)]
struct ExportTranscripts<'a> {
    scheduled: &'a [ScheduledExportValue],
    transcripts: &'a [ExportValueTranscript],
}

impl<'a> ExportTranscripts<'a> {
    /// The transcript scheduled for exactly this artifact case and export.
    ///
    /// Resolved through the *scheduled demands*, which carry the subject
    /// identity, rather than through the transcript's own query name: the
    /// schedule is what the producer was asked, and the answer's shape was
    /// already checked against it position by position before this is reached.
    fn find(&self, artifact_case: &str, export: &str) -> Option<&'a ExportValueTranscript> {
        self.scheduled
            .iter()
            .zip(self.transcripts)
            .find_map(|(scheduled, transcript)| {
                scheduled
                    .proof_demands
                    .iter()
                    .any(|proof| proof_artifact_export(&proof.subject) == (artifact_case, export))
                    .then_some(transcript)
            })
    }
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
// `Clone` exists for importer variants of one graph node, which share their
// representative's live-verified evidence inside the same transaction; it never
// crosses a process or a session boundary.
#[derive(Clone)]
pub struct VerifiedTypeFactsEvidence {
    bindings: Vec<WitnessBinding>,
    session_evidence_root: String,
    /// ADR 0036: the unique call signature of every scheduled export whose
    /// transcript stated one, by export name — one signature, or the complete
    /// declared overload set. Read only by veto synthesis; a signature here
    /// proves nothing and binds nothing.
    call_signatures: std::collections::BTreeMap<String, Vec<typefacts::SelectedSignature>>,
}

impl VerifiedTypeFactsEvidence {
    /// A completed creates census proves this parent claim without consulting
    /// dependency semantic claims. Only this live-verified token can expose
    /// the premise; a wire WitnessBinding alone has no such authority.
    pub(super) fn independent_creates_census(
        &self,
        plan: &CertificationPlan,
        claim_id: &str,
    ) -> Option<&str> {
        let demand = plan.demand_graph().demands().iter().find(|demand| {
            demand.family() == ProofFamily::DomainExhaustiveness
                && matches!(demand.subject(), ProofDemandSubject::DomainClosure {
                    subject,
                    semantic_claim_id,
                } if semantic_claim_id == claim_id && matches!(
                    subject.path,
                    SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Creates))
                ))
        })?;
        self.bindings
            .iter()
            .find(|binding| binding.demand_id() == demand.id().as_str())
            .map(WitnessBinding::evidence_root)
    }

    #[must_use]
    /// The unique call signature the export-value transcript stated for
    /// `export`, when it stated exactly one, or its complete declared overload
    /// set (ADR 0036). Never a partial set: an overload the producer did not
    /// report would be a call shape synthesis never samples, and the sample is
    /// only as honest as the set it was drawn from.
    pub(super) fn call_signatures(&self, export: &str) -> Option<&[typefacts::SelectedSignature]> {
        self.call_signatures.get(export).map(Vec::as_slice)
    }

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

    /// Asks the same pinned live process for the implementation transcripts of
    /// module-local declarations the `creates` census reached.
    ///
    /// These are not certification evidence in their own right, which is why
    /// they carry no session identity envelope: the receipt binds each one
    /// through the census witness site that names the declaration's exact span
    /// and the SHA-256 of its canonical transcript
    /// (`census-local-declaration:…`), and the census refuses the domain when a
    /// declaration it needs is missing. The producer's own binding still holds
    /// — [`typefacts::Session::export_values`] refuses an answer whose resolved
    /// declaration does not lie inside the demanded span.
    pub(crate) fn acquire_local_declarations(
        &mut self,
        demands: &[ExportValueDemand],
    ) -> Result<typefacts::ExportValueAnswer, TypeFactsCertificationError> {
        Ok(self.session.export_values(demands)?)
    }
}

/// Acquires, through the still-open session, every module-local declaration
/// transcript the `creates` implementation census of this plan's demands
/// needs, one batch per recursion depth.
///
/// The census is run in its acquisition mode against the transcripts already
/// in hand: a pass that reaches a local declaration nobody has transcribed yet
/// records the declaration and stops short of a verdict
/// ([`CensusOutcome::NeedsTranscripts`]), and this loop demands exactly those
/// declarations and runs the pass again. A pass that decides — or refuses —
/// asks for nothing more; verification then states the verdict. The loop is
/// bounded by [`MAX_COMPOSITION_DEPTH`] rounds because each round resolves one
/// more hop, and the census itself refuses a chain longer than that.
fn acquire_census_local_transcripts(
    session: &mut TypeFactsCertificationSession,
    plan: &CertificationPlan,
    schedule: &TypeFactsCertificationSchedule,
    live: &LiveExportValueAnswer,
    roots: &[SnapshotSourceRoot<'_>],
) -> Result<Vec<LocalDeclarationTranscript>, TypeFactsCertificationError> {
    let mut locals = Vec::<LocalDeclarationTranscript>::new();
    for _round in 0..=MAX_COMPOSITION_DEPTH {
        // (anchor expression the producer evaluates, declaration span asked for)
        let mut requested = Vec::<(typefacts::Location, typefacts::Location)>::new();
        for (index, scheduled) in schedule.export_values.iter().enumerate() {
            let Some(transcript) = live.answer().transcripts.get(index) else {
                continue;
            };
            let Some(implementation) = transcript.implementation.as_ref() else {
                continue;
            };
            for proof in &scheduled.proof_demands {
                let ProofDemandSubject::DomainClosure { subject, .. } = &proof.subject else {
                    continue;
                };
                if proof.family != ProofFamily::DomainExhaustiveness
                    || !matches!(
                        &subject.path,
                        SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Creates))
                    )
                {
                    continue;
                }
                let Some(export) = plan
                    .candidates
                    .proposal()
                    .artifact_case(&subject.artifact_case)
                    .and_then(|case| case.exports.get(&subject.export))
                else {
                    continue;
                };
                let evidence = CensusEvidence {
                    roots,
                    locals: &locals,
                };
                if let Ok((CensusOutcome::NeedsTranscripts, _, wanted)) =
                    census_creates_domain(plan, proof, export, implementation, evidence)
                {
                    for location in wanted {
                        let known = locals.iter().any(|local| local.location == location)
                            || requested.iter().any(|(_, pending)| *pending == location);
                        if !known {
                            requested.push((scheduled.demand.location.clone(), location));
                        }
                    }
                }
            }
        }
        if requested.is_empty() {
            break;
        }
        let demands = requested
            .iter()
            .map(|(anchor, location)| ExportValueDemand {
                location: anchor.clone(),
                implementation_location: None,
                local_declaration_location: Some(location.clone()),
                callable_depth: 0,
            })
            .collect::<Vec<_>>();
        let answer = session.acquire_local_declarations(&demands)?;
        for ((_, location), transcript) in requested.into_iter().zip(answer.transcripts) {
            // `Session::export_values` already refused an answer that omitted
            // the local declaration or bound it to another span; an absent one
            // here is therefore unreachable, and is simply not recorded, so the
            // census refuses the demand by name rather than this loop guessing.
            if let Some(local) = transcript.local_declaration {
                locals.push(LocalDeclarationTranscript {
                    location,
                    transcript: local,
                });
            }
        }
    }
    Ok(locals)
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
            let package_marker = snapshot_package_marker(plan);
            let roots =
                snapshot_source_roots(plan, dependencies, sources, Some(&project), &package_marker)
                    .map_err(|error| error.at_stage("census source-root derivation"))?;
            let locals =
                acquire_census_local_transcripts(&mut session, plan, schedule, &live, &roots)
                    .map_err(|error| error.at_stage("census local-declaration acquisition"))?;
            verify_live_export_value_answer_with_dependencies(
                plan,
                schedule,
                &live,
                dependencies,
                sources,
                &project,
                &locals,
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
    /// Whether this node's exported values are acquired and verified in this
    /// call. A node whose gating did not move since its evidence was taken
    /// keeps that evidence (ADR 0036's graph loop); its plan still takes part
    /// in schedule derivation, so an export whose runtime binding belongs to
    /// this node's snapshot still finds its owner.
    pub(super) acquire: bool,
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
) -> Result<Vec<Option<VerifiedTypeFactsEvidence>>, TypeFactsCertificationError> {
    if requests.is_empty() {
        return Ok(Vec::new());
    }
    // Importer variants share one acquisition. Discovery keys a graph node by
    // the module that imported it, so one package's many entrypoints put the
    // same `solid-js` into the graph once per importing module
    // (`@corvu/drawer`'s graph carries it about forty times). Two such nodes
    // are the same snapshot, the same proposal, and the same materialized
    // root, and the demand graph hashes none of the importer -- the roots are
    // equal -- so the evidence one acquisition binds to that (snapshot,
    // demand-graph) pair is the evidence every variant's receipt needs. The
    // group also requires the same dependency set and source set up to the
    // same importer-invariant key, so a variant whose authenticated
    // descendants differ is acquired on its own.
    let all_requests = requests;
    let mut variant_keys: Vec<Vec<String>> = Vec::new();
    let mut representatives: Vec<&GraphExportValueRequest<'_>> = Vec::new();
    let mut representative_of = Vec::with_capacity(all_requests.len());
    // A group is acquired when any of its variants asks to be.
    let mut group_acquire: Vec<bool> = Vec::new();
    for request in all_requests {
        let key = importer_invariant_request_key(request);
        let position = match variant_keys.iter().position(|known| *known == key) {
            Some(position) => {
                group_acquire[position] |= request.acquire;
                position
            }
            None => {
                variant_keys.push(key);
                representatives.push(request);
                group_acquire.push(request.acquire);
                representatives.len() - 1
            }
        };
        representative_of.push(position);
    }
    let requests = representatives.as_slice();
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
        .enumerate()
        .map(|(position, (request, schedule))| {
            if !group_acquire[position] {
                return Ok(None);
            }
            let live = session
                .acquire_export_values(request.plan, schedule)
                .map_err(|error| {
                    error.at_graph_node(request.plan, "live graph export-value acquisition")
                })?;
            let package_marker = snapshot_package_marker(request.plan);
            let roots = snapshot_source_roots(
                request.plan,
                &census_dependencies,
                &sources,
                Some(&project),
                &package_marker,
            )
            .map_err(|error| {
                error.at_graph_node(request.plan, "graph census source-root derivation")
            })?;
            let locals = acquire_census_local_transcripts(
                &mut session,
                request.plan,
                schedule,
                &live,
                &roots,
            )
            .map_err(|error| {
                error.at_graph_node(request.plan, "graph census local-declaration acquisition")
            })?;
            verify_live_export_value_answer_with_project_census(
                request.plan,
                schedule,
                &live,
                &request.dependencies,
                &census_dependencies,
                &sources,
                Some(&project),
                &locals,
            )
            .map_err(|error| {
                error.at_graph_node(request.plan, "live graph export-value verification")
            })
            .map(Some)
        })
        .collect::<Vec<Result<_, _>>>();
    let evidence = merge_graph_census_refusals(evidence);
    report_certification_timing(
        "live-export-value-acquisition-and-verification",
        started,
        serde_json::json!({
            "plans": requests.len(),
            "importerVariants": all_requests.len(),
            "graph": true
        }),
    );
    let started = std::time::Instant::now();
    drop(session);
    drop(project);
    report_certification_timing(
        "private-project-removal",
        started,
        serde_json::json!({ "graph": true }),
    );
    // Back to one answer per requested node, importer variants sharing their
    // representative's evidence.
    evidence.map(|evidence| {
        representative_of
            .iter()
            .map(|position| evidence[*position].clone())
            .collect()
    })
}

/// Every node's verification result as one result: the evidence of every
/// node when every node verified; otherwise the first error that is not a
/// census refusal; otherwise one `CensusRefused` carrying every node's census
/// refusals, so the caller withholds them all in one pass.
fn merge_graph_census_refusals<T>(
    results: Vec<Result<T, TypeFactsCertificationError>>,
) -> Result<Vec<T>, TypeFactsCertificationError> {
    let mut evidence = Vec::with_capacity(results.len());
    let mut refusals = Vec::new();
    for result in results {
        match result {
            Ok(verified) => evidence.push(verified),
            Err(error) => {
                let mut inner = &error;
                while let TypeFactsCertificationError::TransactionStage { source, .. }
                | TypeFactsCertificationError::GraphNodeStage { source, .. } = inner
                {
                    inner = source;
                }
                match inner {
                    TypeFactsCertificationError::CensusRefused {
                        refusals: node_refusals,
                    } => {
                        refusals.extend(node_refusals.iter().cloned());
                    }
                    _ => return Err(error),
                }
            }
        }
    }
    if refusals.is_empty() {
        Ok(evidence)
    } else {
        Err(TypeFactsCertificationError::CensusRefused { refusals })
    }
}

/// What decides whether two graph nodes may share one exported-value
/// acquisition: the snapshot, the demand graph, the materialized root, and the
/// same for every authenticated descendant and source. The importing module is
/// deliberately absent -- it is the one coordinate importer variants differ in.
fn importer_invariant_request_key(request: &GraphExportValueRequest<'_>) -> Vec<String> {
    fn plan_key(plan: &CertificationPlan) -> String {
        format!(
            "{}\0{}\0{}",
            plan.snapshot_root(),
            plan.demand_graph().root().as_str(),
            plan.resolved_import.package_root
        )
    }
    let mut dependencies = request
        .dependencies
        .iter()
        .map(|dependency| plan_key(dependency))
        .collect::<Vec<_>>();
    dependencies.sort();
    dependencies.dedup();
    let mut sources = request
        .sources
        .iter()
        .map(|source| source.identity.clone())
        .collect::<Vec<_>>();
    sources.sort();
    vec![
        plan_key(request.plan),
        dependencies.join("\n"),
        sources.join("\n"),
    ]
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
        // The name is (pid, counter), and a pid is reused: a certification the
        // runner killed on its timeout never dropped its project, so its
        // directory is still there when a later process draws the same pid and
        // `create_dir` reports "File exists" for a directory this process never
        // made. Skip past such a leftover rather than fail; nothing is ever
        // written into a directory this process did not create.
        let requested = {
            let mut attempts = 0;
            loop {
                let candidate = std::env::temp_dir().join(format!(
                    "solid-checker-typefacts-project-{}-{}",
                    std::process::id(),
                    PRIVATE_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&candidate) {
                    Ok(()) => break candidate,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::AlreadyExists
                            && attempts < PRIVATE_PROJECT_NAME_ATTEMPTS =>
                    {
                        attempts += 1;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };
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
        // Another certification may have published this same content-addressed
        // entry while ours was staged. An entry that is exact *now* is the one
        // to keep -- a sibling process may already have linked its project to
        // it, and retiring it from under that link would cost that
        // certification its sources -- so re-check before replacing, and
        // replace only an entry that is still not exact.
        if entry.exists() && materialized_store_entry_is_exact(entry, snapshot, expected) {
            return Ok(());
        }
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

/// How many distinct declaration resolutions each public `(specifier, export)`
/// subject has across `plans`.
///
/// A variant is `(declaration path, selector, declaration export, owning
/// snapshot root)`. The owner is the fourth coordinate — not the *plan's*
/// snapshot root, which is what it used to be — because a declaration path is
/// only an identity together with the package it is relative to. Two cases whose
/// paths, selectors and export names agree inside two different dependency
/// copies are two resolutions of one public subpath, and collapsing them let a
/// case be bound to whichever copy the host's active condition set resolves.
///
/// Both directions of the change are deliberate. Within a single-package batch
/// every plan shares `plan.snapshot_root()`, so the old coordinate distinguished
/// nothing there. Across a graph, where plans of one package name at different
/// versions do differ in it, the old coordinate *split* subjects that agreed on
/// the binding in every respect that decides what the harness imports; the new
/// one merges them, and merging is safe because the public specifier is only
/// ever taken when `publicly_addressable` holds, which means it resolves inside
/// that plan's own materialized copy.
fn export_resolution_variants(
    plans: &[&CertificationPlan],
) -> Result<ExportResolutionVariants, TypeFactsCertificationError> {
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
                    declaration_binding_owner(plan, export, demand.id().as_str())?.to_owned(),
                ));
        }
    }
    Ok(resolution_variants)
}

/// Test-only entry point: the declaration-resolution variants `plans` produce,
/// as `(public specifier, export, variants)` with each variant rendered as
/// `(declaration path, selector, declaration export, owning snapshot root)`.
/// Used by the sibling module's regression tests that the owning snapshot root
/// is the coordinate, not the plan's.
#[cfg(test)]
pub(super) fn export_resolution_variants_for_test(
    plans: &[&CertificationPlan],
) -> Result<Vec<(String, String, Vec<ExportResolutionVariant>)>, TypeFactsCertificationError> {
    Ok(export_resolution_variants(plans)?
        .into_iter()
        .map(|((specifier, export), variants)| (specifier, export, variants.into_iter().collect()))
        .collect())
}

/// Test-only entry point: the harness subject `derive_export_value_schedules`
/// would give `subject_plan`'s `export`, with the variant map computed over
/// `plans` and the private project materialized from `owner` plus
/// `dependencies`. `force_exact_subjects` mirrors the graph lane's argument.
#[cfg(test)]
pub(super) fn export_value_harness_subject_for_test(
    owner: &CertificationPlan,
    dependencies: &[&CertificationPlan],
    plans: &[&CertificationPlan],
    subject_plan: &CertificationPlan,
    export: &str,
    force_exact_subjects: bool,
) -> Result<(String, String), TypeFactsCertificationError> {
    let project = PrivateTypeFactsProject::materialize(owner, dependencies, plans, &[])?;
    let variants = export_resolution_variants(plans)?;
    export_value_harness_subject(
        &project,
        subject_plan,
        subject_plan.selected_artifact_case_id(),
        export,
        "test-demand",
        &variants,
        force_exact_subjects,
    )
}

fn derive_export_value_schedules(
    plans: &[&CertificationPlan],
    project: &PrivateTypeFactsProject,
    force_exact_subjects: bool,
) -> Result<Vec<TypeFactsCertificationSchedule>, TypeFactsCertificationError> {
    let resolution_variants = export_resolution_variants(plans)?;
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
                project.package_root(plan)?,
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
                            // No scheduled demand asks for a module-local
                            // declaration yet. The implementation census that
                            // will is not wired here, and a demand that named
                            // a helper without a census to consume it would
                            // pay for a transcript nothing reads.
                            local_declaration_location: None,
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
                    .and_then(parameter_root)
                    .map_or(0, |(_, path)| path.len()),
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

/// Test-only entry point: materialize `owner` together with `dependencies` the
/// way one graph project does, then build the exact declaration harness subject
/// for each of `exports` and report `(specifier, selector, specifier resolves to
/// a materialized module)`.
///
/// A specifier resolves when the literal path exists or when the declaration
/// file TypeScript maps it back to does — `declaration_import_path` writes the
/// runtime extension, and a declaration-only module is addressed through it.
/// Used by the sibling module's regression test that a declaration binding
/// re-exported from a dependency is imported from *that dependency's*
/// materialized package root.
#[cfg(test)]
pub(super) fn exact_declaration_harness_subjects_for_test(
    owner: &CertificationPlan,
    dependencies: &[&CertificationPlan],
    exports: &[&str],
) -> Result<Vec<(String, String, bool)>, TypeFactsCertificationError> {
    let mut plans = vec![owner];
    plans.extend(dependencies.iter().copied());
    let project = PrivateTypeFactsProject::materialize(owner, dependencies, &plans, &[])?;
    exports
        .iter()
        .map(|export| {
            let (specifier, selector) =
                exact_declaration_harness_subject(&project, owner, export, "test-demand")?;
            let path = project.root.join(specifier.trim_start_matches("./"));
            let resolves = path.is_file()
                || ["d.ts", "d.mts", "d.cts"].iter().any(|extension| {
                    let literal = path.to_string_lossy();
                    let stem = literal
                        .strip_suffix(".js")
                        .or_else(|| literal.strip_suffix(".mjs"))
                        .or_else(|| literal.strip_suffix(".cjs"))
                        .unwrap_or(&literal);
                    Path::new(&format!("{stem}.{extension}")).is_file()
                });
            Ok((specifier, selector, resolves))
        })
        .collect()
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
    //
    // `declaration_path` is relative to the package that *owns* the binding,
    // which for an export re-exported from a dependency is that dependency and
    // not this plan. See `declaration_owner_package_root`.
    let owner = declaration_binding_owner(plan, export, demand)?;
    let package_root = declaration_owner_package_root(project, plan, owner)?;
    let specifier = snapshot_module_harness_specifier(project, package_root, declaration_path)?;
    Ok((specifier, declaration_selector.to_owned()))
}

fn declaration_binding_owner<'plan>(
    plan: &'plan CertificationPlan,
    export: &str,
    demand: &str,
) -> Result<&'plan str, TypeFactsCertificationError> {
    plan.verified_exports
        .declaration_binding_snapshot_root(export)
        .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
            demand: demand.to_owned(),
            reason: "demanded export has no snapshot-verified declaration binding".into(),
        })
}

/// Materialized package root of the copy whose authenticated snapshot root is
/// `owner`.
///
/// A re-exported export's declaration binding names a file relative to the
/// package that owns it. Joining a dependency-owned path onto this plan's own
/// package root names a file the plan does not ship: the harness import never
/// resolves, the alias target comes back as the checker's `unknown` symbol, and
/// the producer answers a declaration openness about a file the demand never
/// named — a witness-program defect reported as a producer limit.
///
/// The owner is selected by the same authenticated snapshot root the export
/// replay recorded and `verify_target` matches a planned dependency by, never
/// by package name or by path arithmetic.
///
/// Several materialized copies of one snapshot root are not an ambiguity, for
/// the reason `export_implementation_location` already records at its own call
/// site: `snapshot_root` is a content hash over the package name, version, and
/// every file's bytes, so every copy carrying it holds byte-identical sources at
/// the same package-relative path. Importing the declaration through any of them
/// selects the same bytes and the same producer transcript. This binds the
/// *first* such copy in `package_roots` order — the map is a `BTreeMap` keyed by
/// `(snapshot root, installed package root)`, so among the copies of one
/// snapshot root that order is the installed package root's, and the selection
/// is stable across runs and independent of plan discovery order. Refusing
/// instead would escape `derive_export_value_schedules`, which the graph lane
/// calls once for the whole graph: one duplicated install of one owning
/// dependency would refuse every node of that published graph.
///
/// An owner this project did not materialize is a different matter and still
/// fails closed — there is no copy to address, and guessing one would be
/// substitution. That arm is unreachable from the batch lane, which cannot carry
/// a foreign declaration owner at all (see the `(None, _)` note below).
fn declaration_owner_package_root<'project>(
    project: &'project PrivateTypeFactsProject,
    plan: &CertificationPlan,
    owner: &str,
) -> Result<&'project Path, TypeFactsCertificationError> {
    if owner == plan.snapshot_root() {
        return project.package_root(plan);
    }
    project
        .package_roots
        .iter()
        .find(|((snapshot_root, _), _)| snapshot_root == owner)
        .map(|(_, root)| root.as_path())
        .ok_or_else(|| {
            TypeFactsCertificationError::identity_mismatch(
                "declaration harness",
                "declaration_owner_package_root",
                format!("a materialized package of snapshot root {owner}"),
                format!(
                    "{} materialized package root(s), none of that snapshot root",
                    project.package_roots.len()
                ),
            )
        })
}

fn snapshot_module_harness_specifier(
    project: &PrivateTypeFactsProject,
    package_root: &Path,
    path: &str,
) -> Result<String, TypeFactsCertificationError> {
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
        call_signatures: std::collections::BTreeMap::new(),
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
    // No local-declaration transcripts: a `creates` census that has to recurse
    // into a module-local helper refuses here, because this entry point has no
    // session to ask. The opaque acquisition transactions below are the only
    // callers that can supply them.
    verify_live_export_value_answer_with_project_census(
        plan,
        schedule,
        live,
        &[],
        &[],
        &[],
        None,
        &[],
    )
}

fn verify_live_export_value_answer_with_dependencies(
    plan: &CertificationPlan,
    schedule: &TypeFactsCertificationSchedule,
    live: &LiveExportValueAnswer,
    dependencies: &[&CertificationPlan],
    sources: &[super::dependencies::VerifiedGraphSourcePackage],
    project: &PrivateTypeFactsProject,
    locals: &[LocalDeclarationTranscript],
) -> Result<VerifiedTypeFactsEvidence, TypeFactsCertificationError> {
    verify_live_export_value_answer_with_project_census(
        plan,
        schedule,
        live,
        dependencies,
        dependencies,
        sources,
        Some(project),
        locals,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "one authenticated evidence set per fact domain, each already bound to this plan"
)]
fn verify_live_export_value_answer_with_project_census(
    plan: &CertificationPlan,
    schedule: &TypeFactsCertificationSchedule,
    live: &LiveExportValueAnswer,
    dependencies: &[&CertificationPlan],
    census_dependencies: &[&CertificationPlan],
    census_sources: &[super::dependencies::VerifiedGraphSourcePackage],
    project: Option<&PrivateTypeFactsProject>,
    locals: &[LocalDeclarationTranscript],
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
    // The same authenticated roots the source census just checked every
    // consulted file against, now read the other way round: given a resolved
    // callee's declaration, which archive is it a member of. Derived once, by
    // the one function that derives them, so the two censuses cannot disagree.
    let package_marker = snapshot_package_marker(plan);
    let source_roots = snapshot_source_roots(
        plan,
        census_dependencies,
        census_sources,
        project,
        &package_marker,
    )?;
    let census = CensusEvidence {
        roots: &source_roots,
        locals,
    };
    let certification_sources_root = plan.certification_sources_root();
    let mut bindings = Vec::with_capacity(expected_ids.len());
    let mut call_signatures = std::collections::BTreeMap::new();
    let mut census_refusals = Vec::<CensusRefusal>::new();
    for (index, scheduled) in schedule.export_values.iter().enumerate() {
        let transcript = &answer.transcripts[index];
        if let Some(proof) = scheduled.proof_demands.first()
            && let Some(signatures) = stated_call_signatures(transcript)
        {
            let (_, export) = proof_artifact_export(&proof.subject);
            call_signatures
                .entry(export.to_owned())
                .or_insert(signatures);
        }
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
        verify_scheduled_implementation_location(
            transcript,
            scheduled.demand.implementation_location.as_ref(),
        )?;
        let transcript_bytes = typefacts::encode(transcript)?;
        let transcript_root = format!("sha256:{:x}", Sha256::digest(&transcript_bytes));
        for proof in &scheduled.proof_demands {
            verify_export_value_subject(plan, proof, transcript, dependencies)?;
            let mut sites = match verify_export_value_family(
                plan,
                proof,
                transcript,
                ExportTranscripts {
                    scheduled: &schedule.export_values,
                    transcripts: &answer.transcripts,
                },
                census,
            ) {
                Ok(sites) => sites,
                // A census that could not decide a proposable closure
                // candidate is recorded and the next demand is read: the
                // caller withholds every recorded candidate at once and
                // re-plans, and the evidence of this answer is not returned.
                Err(error) => {
                    if let Some(refusal) = proposable_census_refusal(proof, &error) {
                        census_refusals.push(refusal);
                        continue;
                    }
                    return Err(error);
                }
            };
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
    if !census_refusals.is_empty() {
        return Err(TypeFactsCertificationError::CensusRefused {
            refusals: census_refusals,
        });
    }
    Ok(VerifiedTypeFactsEvidence {
        bindings,
        session_evidence_root: identity.evidence_root().to_owned(),
        call_signatures,
    })
}

/// `error` as the census refusal of `proof`, when `proof` is a
/// domain-exhaustiveness demand on a proposable call domain and the error is
/// the census saying it cannot decide it -- the pair ADR 0036 withholds on.
/// Anything else is `None` and refuses as before.
fn proposable_census_refusal(
    proof: &ScheduledProofDemand,
    error: &TypeFactsCertificationError,
) -> Option<CensusRefusal> {
    if proof.family != ProofFamily::DomainExhaustiveness {
        return None;
    }
    let ProofDemandSubject::DomainClosure { subject, .. } = &proof.subject else {
        return None;
    };
    let SemanticClaimPath::Domain(ClaimPath::Call(domain)) = &subject.path else {
        return None;
    };
    if !domain.is_proposable() {
        return None;
    }
    let (demand, reason) = match error {
        TypeFactsCertificationError::UnsupportedDemand { demand, reason }
        | TypeFactsCertificationError::FamilyOpen { demand, reason } => (demand, reason),
        _ => return None,
    };
    if demand != &proof.id {
        return None;
    }
    Some(CensusRefusal {
        demand: demand.clone(),
        reason: reason.clone(),
        rendered: error.to_string(),
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
    transcripts: ExportTranscripts<'_>,
    census: CensusEvidence<'_>,
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
                require_operation_recursive_subject(
                    plan,
                    proof,
                    transcript,
                    transcripts,
                    &open,
                    &mut sites,
                )?
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
                plan,
                export,
                operation,
                proof,
                implementation,
                transcripts,
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
                plan,
                export,
                operation,
                proof,
                implementation,
                transcripts,
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
                require_operation_recursive_subject(
                    plan,
                    proof,
                    transcript,
                    transcripts,
                    &open,
                    &mut sites,
                )?;
            }
        }
        ProofFamily::DomainExhaustiveness => {
            let ProofDemandSubject::DomainClosure { subject, .. } = &proof.subject else {
                return Err(TypeFactsCertificationError::UnsupportedDemand {
                    demand: proof.id.clone(),
                    reason: "domain-exhaustiveness demand has no closure subject".into(),
                });
            };
            if matches!(
                &subject.path,
                SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Creates))
            ) {
                // The implementation census. The demanded export's own
                // runtime implementation transcript has to clear the same
                // completeness and authenticated runtime binding every other
                // implementation-reading family clears, and then every call it
                // reaches — transitively, through the local declarations this
                // session transcribed — is dispositioned by its callee.
                let (export, implementation) =
                    require_export_implementation(plan, proof, transcript, &open)?;
                require_census_decides_closure(
                    proof,
                    &subject.path,
                    ClosureCensus::Implementation,
                )?;
                let (outcome, census_sites, requested) =
                    census_creates_domain(plan, proof, export, implementation, census)?;
                if outcome == CensusOutcome::NeedsTranscripts {
                    // Verification has no session left to ask. Acquisition
                    // batches every local declaration the census names; one
                    // still missing here is a declaration that acquisition
                    // could not transcribe, and the census refuses rather
                    // than reading its absence as harmless.
                    return Err(TypeFactsCertificationError::UnsupportedDemand {
                        demand: proof.id.clone(),
                        reason: format!(
                            "implementation-census premise required: no implementation \
                             transcript was acquired for the local declaration(s) {}",
                            requested
                                .iter()
                                .map(|location| format!(
                                    "{}:{}..{}",
                                    location.path, location.start_byte, location.end_byte
                                ))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    });
                }
                sites.extend(census_sites);
            } else if matches!(
                &subject.path,
                SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Returns))
            ) {
                // ADR 0035: the same implementation transcript, read for its
                // completions instead of its calls. No callee is dispositioned
                // and no local declaration is recursed into: a callee's value
                // reaches this export's caller only through a return site of
                // this export's own, which is what the census reads.
                let (export, implementation) =
                    require_export_implementation(plan, proof, transcript, &open)?;
                require_census_decides_closure(
                    proof,
                    &subject.path,
                    ClosureCensus::Implementation,
                )?;
                sites.extend(census_returns_domain(proof, export, implementation)?);
            } else {
                // This census is the exported value's own observation and
                // nothing else: no control-flow branch census, so no guard
                // partition, and no implementation census, so no other
                // behavioral call domain.
                require_census_decides_closure(proof, &subject.path, ClosureCensus::ExportedValue)?;
                require_closed_value(&transcript.value, &open)?;
                require_export_callable_paths_closed(transcript, &open)?;
                let alternatives = require_export_value_enumeration_matches_census(
                    plan, proof, transcript, &open,
                )?;
                sites.push(format!(
                    "typefacts-export-value-domain:choice-alternatives:{alternatives}"
                ));
            }
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
/// The call signatures an export-value transcript states for veto synthesis:
/// its one signature, or its overload set when that set is provably the whole
/// declared one. `None` for a value with no signature, for a transcript that
/// states both forms, and for a partial overload set.
fn stated_call_signatures(
    transcript: &ExportValueTranscript,
) -> Option<Vec<typefacts::SelectedSignature>> {
    match (
        transcript.call_signature.as_ref(),
        transcript.call_signatures.as_slice(),
    ) {
        (Some(signature), []) => Some(vec![signature.clone()]),
        (None, overloads) if !overloads.is_empty() => {
            let incomplete = |reason: &str| TypeFactsCertificationError::UnsupportedDemand {
                demand: String::new(),
                reason: reason.to_owned(),
            };
            require_complete_overload_set(overloads, &incomplete)
                .ok()
                .map(|()| overloads.to_vec())
        }
        _ => None,
    }
}

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
    require_named_export_implementation(plan, proof, artifact_case, export_name, transcript, open)
}

/// The same closure and snapshot-binding checks for an export the demand does
/// not name.
///
/// The demand still owns the refusal, because a composition premise that fails
/// is a refusal of the *demand*, not of the export it was trying to compose
/// from. Everything else is identical, deliberately: a composed row's target
/// has to clear exactly the transcript completeness and the authenticated
/// runtime binding that the demanded export cleared, or the composition would
/// rest on a census this side never validated.
fn require_named_export_implementation<'a>(
    plan: &'a CertificationPlan,
    proof: &ScheduledProofDemand,
    artifact_case: &str,
    export_name: &str,
    transcript: &'a ExportValueTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<
    (
        &'a solid_reactive_ir::contract_semantics::ExportSemantics,
        &'a typefacts::ExportImplementationTranscript,
    ),
    TypeFactsCertificationError,
> {
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

/// The exact parameter root of an operation input, or `None` for every other
/// shape. Never an error: the callers that own an answer build the attributable
/// refusal themselves through [`operation_input_parameter_root`], and the one
/// caller that is only measuring a path depth wants the absence.
fn parameter_root(value: &ValueShape) -> Option<(u16, &[String])> {
    match value {
        ValueShape::Parameter { index, path } => Some((*index, path.as_slice())),
        _ => None,
    }
}

/// The parameter root of an operation input, with the refusal that names the
/// exact input and the shape the implementation census cannot bind.
///
/// The refusal carries the demand's own id, the operation input's identity, and
/// the demand family, because without them every unsupported input in a package
/// renders as the same sentence: the audit sidecar reports `demandId: null,
/// family: null` and six different packages become one indistinguishable class.
/// The operation identity is the one the emitted document carries, which in a
/// normalized contract is already qualified by artifact case and export
/// (`artifact-case:<digest>:<export>:operation:<id>`), so the input is named
/// once rather than prefixed with the same two fields again.
/// `value_shape_constructor` is a stable spelling rather than `{:?}` for the
/// same reason `parameter_use_kind_site` is — a refusal reason is read by
/// tooling and pinned by tests, so it may not move when a shape gains a field.
fn operation_input_parameter_root(
    value: &ValueShape,
    proof: &ScheduledProofDemand,
    operation: &str,
    index: usize,
) -> Result<(u16, Vec<String>), TypeFactsCertificationError> {
    if let Some((parameter, path)) = parameter_root(value) {
        return Ok((parameter, path.to_vec()));
    }
    Err(TypeFactsCertificationError::UnsupportedDemand {
        demand: proof.id.clone(),
        reason: format!(
            "operation input {operation}[{index}] is {}, and the implementation census binds only parameter-rooted operation inputs (family={})",
            value_shape_constructor(value),
            proof_family_name(proof.family)
        ),
    })
}

fn operation_input_parameter_source(
    value: &ValueShape,
    proof: &ScheduledProofDemand,
    operation: &str,
    index: usize,
) -> Result<ValueSource, TypeFactsCertificationError> {
    let (parameter, path) = operation_input_parameter_root(value, proof, operation, index)?;
    Ok(ValueSource::Parameter {
        index: parameter,
        path,
    })
}

/// The stable spelling of a value shape's constructor, for refusal text.
///
/// A shape's *constructor* only: a refusal names which shape the census cannot
/// bind, and reproducing a shape's contents there would put a package's
/// semantics into an error string that tests pin. `Reactive` is the one
/// exception, and only for its role, because "reactive" without the role does
/// not distinguish the two inputs this refusal class is actually made of.
fn value_shape_constructor(value: &ValueShape) -> &'static str {
    match value {
        ValueShape::Unknown => "unknown",
        ValueShape::Plain => "plain",
        ValueShape::Parameter { .. } => "parameter",
        ValueShape::Tuple(_) => "tuple",
        ValueShape::Array { .. } => "array",
        ValueShape::Object(_) => "object",
        ValueShape::Choice(_) => "choice",
        ValueShape::Callable => "callable",
        ValueShape::Promise(_) => "promise",
        ValueShape::AsyncIterable(_) => "async-iterable",
        ValueShape::Reactive {
            role: ReactiveRole::Accessor,
            ..
        } => "reactive/accessor",
        ValueShape::Reactive {
            role: ReactiveRole::Setter,
            ..
        } => "reactive/setter",
        ValueShape::Store { .. } => "store",
        ValueShape::Action { .. } => "action",
        ValueShape::Component => "component",
        ValueShape::Cleanup { .. } => "cleanup",
        ValueShape::RefApplication => "ref-application",
        ValueShape::ServerFunctionReference { .. } => "server-function-reference",
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

/// A terminator for an implementation census of one call claim domain: an
/// audited dialect authority answers, for this exact callee, that it publishes
/// no operation of the domain's kind.
///
/// Carries only its witness site, because that is the whole of what it
/// contributes: the census's own refusal-by-name is what happens without one,
/// and a terminator is never a positive fact about anything.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CensusTerminator {
    /// `census-dialect-axiom:<pkg>@<ver>#<sri-prefix>:<export>:<domain>`.
    witness_site: String,
}

/// Which identity field disagreed when an authenticated snapshot matched an
/// audited archive's *name* but not the rest of its tuple.
///
/// Precondition 1 of `docs/adr/0005-dialect-axioms-about-the-dialects-own-package.md`
/// requires the gate to say which field disagreed, mirroring the lock replay in
/// `contract_certification/dependencies.rs`. The terminator itself answers
/// `Option`, because a disagreement makes the census refuse the domain by name
/// rather than report a certification error of its own; this is where the
/// information exists, so this is where it is named.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuditedArchiveDisagreement {
    /// No dialect audited an archive under this name at all.
    Name,
    Version,
    Integrity,
    /// The archive's own `package.json` digest, re-derived from the
    /// authenticated snapshot rather than read from a resolver's report.
    Manifest,
}

/// The audited archive tuple this authenticated snapshot *is*, or the field
/// that disagreed.
///
/// Every field is compared, and the manifest digest is computed from the
/// snapshot's own bytes. A snapshot that matches the coordinate but not the
/// integrity refuses here, which is the failure mode ADR 0005 objection 1
/// records: a registry serving a self-consistent `name@version` would otherwise
/// receive the answer on bytes nobody in this repository ever read.
fn audited_archive_for_snapshot(
    snapshot: &super::ArtifactSnapshot,
) -> Result<&'static solid_dialect::AuditedArchive, AuditedArchiveDisagreement> {
    let candidates = solid_dialect::audited_archives(snapshot.package_name());
    if candidates.is_empty() {
        return Err(AuditedArchiveDisagreement::Name);
    }
    let manifest_sha256 = snapshot
        .read("package.json")
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)));
    let mut disagreement = AuditedArchiveDisagreement::Version;
    for archive in candidates {
        if archive.version != snapshot.package_version() {
            continue;
        }
        disagreement = AuditedArchiveDisagreement::Integrity;
        if archive.integrity != snapshot.package_integrity() {
            continue;
        }
        disagreement = AuditedArchiveDisagreement::Manifest;
        if manifest_sha256.as_deref() != Some(archive.manifest_sha256) {
            continue;
        }
        return Ok(archive);
    }
    Err(disagreement)
}

/// The first 23 characters of an SRI — `sha512-` plus sixteen base64 digits.
///
/// A witness site is compared and stored, not re-verified, so it carries a
/// prefix rather than the whole 95-character digest. The identity gate already
/// bound the full integrity; this is a label that makes two witnesses over
/// different bytes visibly different.
fn sri_prefix(integrity: &str) -> &str {
    integrity
        .char_indices()
        .nth(23)
        .map_or(integrity, |(index, _)| &integrity[..index])
}

/// A peer of [`argument_slot_is_proven_invoking`]'s Tier A: same position in
/// the tier list, same membership-reviewed discipline, same refusal of anything
/// unlisted — applied to a **callee** and to a **negative** question.
///
/// Answers `Some` only when an audited dialect authority denies, for exactly
/// these published bytes, that this callee publishes any operation of
/// `domain`'s kind. Every one of the following must hold, and each is a
/// separate `None`:
///
/// 1. `floor == MayExecute`, and the call's own reachability is admitted by it.
///    A closed domain asserts a zero *upper* bound, so anything that may
///    execute bears on it; a `Reachable` floor asserts a lower bound above
///    zero, which no negative authority can answer
///    (`phase21/2026-09-03-implementation-census-plan.md` § 3 item 2, and ADR
///    0005 precondition 3). Refusing a `Reachable` floor is correct polarity,
///    not conservatism: a negative table can never discharge a min-≥-1 claim,
///    whatever the archive or the domain.
/// 2. The site is a call or a construction. An absent kind deserializes to
///    `CallKind::Unknown` and is refused: absence is never read as "call".
/// 3. The callee is resolved — a symbol, a name, and a `declaration` with a
///    source file — and the declaration's own name is the resolved name, so the
///    export key the table is asked about is the declaration's identity rather
///    than an import spelling.
/// 4. The resolved name is a canonical primitive spelling of some dialect.
/// 5. The declaration's source file strips to an authenticated **dependency**
///    snapshot source root, and the remainder is a member of that archive. This
///    is the half that makes the premise an identity claim: `target_module` is
///    the *written import specifier* and is never consulted here, so
///    `import { render } from "solid-js"` resolving to a hoisted sibling
///    installation cannot reach the table, and neither can a file placed under
///    the root that the archive does not contain.
/// 6. That root's archive is not the archive under certification — `certified`
///    is the caller's `plan.snapshot`, taken as a snapshot rather than as a
///    plan so this gate is reachable from a unit test; ADR 0005 precondition 4
///    records that nothing in this repository builds a `CertificationPlan`. A
///    callee
///    inside the artifact being certified is the *self* axiom ADR 0005 defers,
///    whose objection 5 — the demand and the discharge deriving from the same
///    dialect rows — is fatal. Answering about a callee raises no such
///    circularity: the axiom discharges nothing about the export under
///    certification.
/// 7. The artifact under certification is not *itself* an audited archive of
///    any dialect, whatever the callee resolves into. Gate 6 alone lets
///    `solid-js@2.0.0-rc.3` certify with a callee resolving into
///    `@solidjs/signals@2.0.0-rc.3` through: the two are different snapshots
///    with different roots, both audited by this dialect, and that is exactly
///    the live phase-21 self-certification configuration ADR 0005 objection 5
///    is about. This gate is a consumer-side restriction: inside the
///    dialect-defining archives the runtime *is* the definition, and only the
///    tracking-state proof mode may decide there.
/// 8. That root's snapshot equals an audited archive tuple in **all four**
///    fields — name, version, integrity, and the archive's own `package.json`
///    digest re-derived from the snapshot.
/// 9. The dialects' negative table denies the domain for `(archive, export)`,
///    with cross-dialect agreement, and silence is never "no".
///
/// # Where it is consumed
///
/// [`census_call_disposition`] consults it as the `DialectAxiom` disposition of
/// the `creates` implementation census, after the parameter-rooted and
/// standard-library dispositions and before local recursion
/// (`docs/adr/0008-implementation-census-for-creates.md`). Every other
/// behavioral call domain still refuses by name in
/// `require_census_decides_closure`.
fn census_dialect_axiom_for_callee(
    call: &typefacts::ImplementationCall,
    domain: solid_dialect::CallClaimDomain,
    floor: ReachabilityFloor,
    certified: &super::ArtifactSnapshot,
    roots_longest_first: &[SnapshotSourceRoot<'_>],
) -> Option<CensusTerminator> {
    if floor != ReachabilityFloor::MayExecute || !floor.admits(call.reach) {
        return None;
    }
    if !matches!(call.kind, CallKind::Call | CallKind::Construct) {
        return None;
    }
    if call.target.is_empty() || call.target_name.is_empty() {
        return None;
    }
    let declaration = call.declaration.as_ref()?;
    if declaration.source_file.is_empty()
        || declaration.name.as_ref() != call.target_name.as_ref()
        || !solid_dialect::canonical_primitive_name(&call.target_name)
    {
        return None;
    }

    let source_file = declaration.source_file.replace('\\', "/");
    // Clones every root's path on every call, because
    // `strip_materialized_source_root` takes `&[String]` rather than
    // `&[&str]`. This tier runs once per resolved callee per domain, so a
    // future allocation concern if the census turns out hot; not addressed
    // here since it changes a shared helper's signature.
    let root_paths = roots_longest_first
        .iter()
        .map(|root| root.path.clone())
        .collect::<Vec<_>>();
    let (root_index, relative) = strip_materialized_source_root(&source_file, &root_paths)?;
    let root = roots_longest_first.get(root_index)?;
    if !root.dependency {
        return None;
    }
    let snapshot = root.snapshot;
    // The declaration has to be a member of the authenticated archive, not
    // merely a path under its root. The source census already refuses a
    // producer-consulted file that is not in the snapshot; re-asking here keeps
    // this tier's own premise complete rather than inherited.
    snapshot.read(relative)?;
    if snapshot.root() == certified.root()
        || snapshot.provenance_root() == certified.provenance_root()
    {
        return None;
    }
    // The certified artifact itself may not be answered *about* by this axiom,
    // even from a different archive's rows. Certifying `solid-js@2.0.0-rc.3`
    // with a callee resolving into `@solidjs/signals@2.0.0-rc.3` — both audited
    // by this dialect, but different snapshots and different roots — passes
    // the root-equality gate above and is exactly the live phase-21
    // self-certification configuration ADR 0005 objection 5 is about. Inside
    // the dialect-defining archives the runtime IS the definition; only the
    // tracking-state proof mode may decide there, never this negative table.
    if audited_archive_for_snapshot(certified).is_ok() {
        return None;
    }

    let archive = audited_archive_for_snapshot(snapshot).ok()?;
    if !solid_dialect::primitive_performs_no_operation(archive, &call.target_name, domain) {
        return None;
    }
    Some(CensusTerminator {
        witness_site: format!(
            "census-dialect-axiom:{}@{}#{}:{}:{}",
            archive.name,
            archive.version,
            sri_prefix(archive.integrity),
            call.target_name,
            domain.wire_name()
        ),
    })
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
    plan: &CertificationPlan,
    export: &solid_reactive_ir::contract_semantics::ExportSemantics,
    operation: &solid_reactive_ir::contract_semantics::Operation,
    proof: &ScheduledProofDemand,
    implementation: &typefacts::ExportImplementationTranscript,
    transcripts: ExportTranscripts<'_>,
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
            let input = operation
                .inputs
                .first()
                .ok_or_else(|| open("read operation has no input"))?;
            // The locally-created-accessor arm is answered *before* the
            // parameter root is required, because this input is deliberately
            // not parameter-rooted: the IR knows the value is a dialect
            // accessor the export created itself, and the shape is all it has
            // left to say so with. A parameter-rooted input still goes down the
            // parameter path below, untouched.
            // Provenance decides, when the row carries it. There is no
            // fallback to the export's own census: the dedup key separates a
            // read the export performs itself from one it performs through a
            // call, so a row that carries provenance *is* the composed row,
            // and answering it from this export's own body would be answering
            // a different row.
            if let Some(composed) = &operation.composed_from {
                return require_composed_operation(
                    plan,
                    proof,
                    operation,
                    composed,
                    implementation,
                    transcripts,
                    open,
                    sites,
                );
            }
            if let Some(role) = reactive_read_operation_input_role(operation, input) {
                return require_reactive_read_operation_input(
                    operation,
                    role,
                    implementation,
                    floor,
                    open,
                    sites,
                );
            }
            let source =
                operation_input_parameter_source(input, proof, operation.id.0.as_str(), 0)?;
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
        // Both kinds arrive here for exactly one fact: an owner requirement
        // this archive imposes on its caller, witnessed by the archive's own
        // reachable call of the dialect primitive that installs it.
        // `require_owner_operation_call` keys off the operation's own
        // `Requirement` triple, not off the kind, so the `cleanup`-role
        // requirement -- published as `kind: cleanup` in the `cleanups` domain
        // (`inferred_contract.rs`'s `owner_requirement_operation`), witnessed
        // by an `onCleanup` call -- is the same demand it has always answered.
        // Without this arm the re-kinding silently made every such row
        // unsupported at witness acquisition.
        OperationKind::Create | OperationKind::Cleanup => {
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
            reason: "operation has no supported owner requirement".into(),
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

/// The reactive role a recursive value-shape demand asserts of an `invoke`
/// operation's input, or `None` when the demand is not that one.
///
/// Every premise is required, not defaulted. The kind gate is `Invoke` because
/// this witness is the callback's own call sites and a `read` operation has
/// none (its shape carries no span at all — see the backlog entry). The path
/// must be the root, because the census traces the argument *expression* and
/// says nothing about a property inside the value it traces. And the demand
/// must assert no callability: a demand that also claims the value is callable
/// is asking for something a value trace does not answer.
fn reactive_operation_input_role(
    operation: &solid_reactive_ir::contract_semantics::Operation,
    input: &ValueShape,
    path: &solid_reactive_ir::contract_semantics::ValuePath,
    callable: DemandedCallability,
) -> Option<ReactiveRole> {
    if operation.kind != OperationKind::Invoke
        || !path.0.is_empty()
        || callable != DemandedCallability::Unknown
    {
        return None;
    }
    match input {
        ValueShape::Reactive { role, .. } => Some(*role),
        _ => None,
    }
}

/// The reactive role a `read` operation asserts of its own input, or `None`
/// when the operation is not that one.
///
/// The companion of [`reactive_operation_input_role`] for the *other* half of
/// the operation-input class. A `read` operation's input is not a parameter of
/// anything: the IR discovered a call whose callee is a dialect accessor the
/// export created itself, and `ValueShape::Reactive` is the only column the
/// projection has left to state it in. Every other shape — `Parameter` above
/// all — keeps its existing path, so a parameter-rooted read is still answered
/// by [`require_parameter_read_evidence`] and never by this arm.
///
/// Unlike mechanism A there is no path or callability premise to check: a read
/// operation carries exactly one input and the demands that reach here name it
/// as a whole, either implicitly (the reachability and cardinality families,
/// which have no path at all) or explicitly at the root (the recursive family,
/// whose caller checks the path).
fn reactive_read_operation_input_role(
    operation: &solid_reactive_ir::contract_semantics::Operation,
    input: &ValueShape,
) -> Option<ReactiveRole> {
    if operation.kind != OperationKind::Read {
        return None;
    }
    match input {
        ValueShape::Reactive { role, .. } => Some(*role),
        _ => None,
    }
}

/// Implementation evidence that a `read` operation reads a dialect accessor of
/// the demanded role that the export created itself.
///
/// **This rule is weaker than mechanism A, and deliberately so.** A `read`
/// operation carries **no span**: `ContractReactiveRead` drops the read's
/// origin location and `ValueShape::Reactive` carries nothing, so the operation
/// cannot be matched to *a* census call the way an `invoke` operation's input is
/// matched to the callback's own call sites. Universal quantification over the
/// census is not available either — it would be vacuously false, because every
/// ordinary `arr.push(x)` in the body has an empty `callee_sources` and would
/// refuse the demand. So the strongest sound claim the demand as inventoried
/// supports is **existential plus no counterexample**:
///
/// > there is at least one call, admitted by the floor and sitting in the
/// > export's own body, whose callee traces to an unambiguous dialect result of
/// > exactly this role.
///
/// Every call is examined and only a matching one witnesses. A call whose
/// callee traces to nothing is silence, not a negative fact (see
/// `ImplementationCall::callee_sources`), and a call whose callee traces to a
/// dialect result of the *other* role — `setPolled(v)` against an accessor row
/// — witnesses nothing either, because `traced_source_proves_role` is asked
/// about the demanded role and answers no.
///
/// **A no-counterexample clause was implemented here and removed.** It refused
/// the demand when any admitted call proved the *other* role, on the reasoning
/// that a rule ignoring `setPolled(v)` would certify a setter write as an
/// accessor read. That reasoning was wrong: the role is compared, so such a
/// call can never be a witness in the first place, and the clause therefore
/// added no soundness. What it did add was a false refusal, because writing a
/// signal and reading it in one body is ordinary —
/// `const [g, setG] = createSignal(3); setG(4); return g();` publishes an
/// accessor read row that is honestly witnessed by `g()` and was refused by
/// `setG(4)` sitting beside it.
///
/// What this cannot distinguish is *which* read. Two reads of the same
/// `(kind, label)` collapse into one row in
/// `contract_export_function`'s dedup, so a row that says "this export performs
/// one reactive read of an accessor" is witnessed by a set rather than by a
/// site. Every matching call is therefore recorded in the witness list, so the
/// receipt states the set it was proved against. Recorded in
/// `docs/precision-backlog.md`.
///
/// **`captured` stays a veto here, and that is the load-bearing difference from
/// mechanism A.** These operations are stamped `at: call / schedule:
/// same-stack` by `inferred_contract.rs`'s shared constructor, and
/// `ContractReactiveRead` has no execution or schedule column in which a
/// different schedule could ever be stated — so the row means unconditionally
/// "this export reads that accessor when you call it", and the uncaptured gate
/// is the *only* enforcement of it anywhere. A read inside a closure the export
/// hands away or stores does not establish it. Mechanism A relaxes `captured`
/// because its claim is about what a call *passes*, which does not depend on
/// the closure running; this claim is about what the export *does*, which does.
fn require_reactive_read_operation_input(
    operation: &solid_reactive_ir::contract_semantics::Operation,
    role: ReactiveRole,
    implementation: &typefacts::ExportImplementationTranscript,
    floor: ReachabilityFloor,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let mut witnesses = Vec::new();
    for call in &implementation.calls {
        // The kind gate is the same discipline as everywhere else in this
        // module: `new Accessor()` is a different claim about the value than
        // `accessor()`, and the producer's present habits are not what this
        // side certifies against. The captured gate is the schedule premise
        // documented above; the floor is the operation's own stated bound.
        if !is_call_expression(call) || !floor.admits(call.reach) || call.captured {
            continue;
        }
        let rooted = call
            .callee_sources
            .iter()
            .filter(|source| source.path.is_empty())
            .collect::<Vec<_>>();
        if rooted.is_empty() {
            continue;
        }
        for source in rooted {
            if !traced_source_proves_role(source, role) {
                continue;
            }
            witnesses.push(format!(
                "reactive-read-operation-input:{}:{}:{}:{}:{}:{}",
                call.location.path,
                call.location.start_byte,
                call.location.end_byte,
                source.target_module,
                source.target_name,
                traced_result_slot_site(source)
            ));
        }
    }
    if witnesses.is_empty() {
        return Err(open(&format!(
            "read operation {} has no reachable uncaptured call whose callee traces to an unambiguous dialect {}",
            operation.id.0,
            reactive_role_name(role)
        )));
    }
    sites.extend(witnesses);
    Ok(())
}

/// Whether a composed row and the row it names state the *same* claim.
///
/// Every field of the operation except its id and its own provenance. A
/// composed row publishes the target's behaviour as this export's, so a target
/// whose schedule, execution point, cardinality, tracking, owner relation,
/// guard, trigger, inputs or output differ is not the row being published, and
/// composing it would put a promise into this export's contract that the
/// target never made. The provenance is excluded because the recursion, not
/// this comparison, is what follows it.
///
/// **Resources are not compared — the composing row is required to have
/// none.** A resource id is qualified by the export that owns it, so two
/// exports' resource sets can never be equal and comparing them would refuse
/// every composition; but *ignoring* the composing row's would let it publish
/// a resource relation of its own that this premise proves nothing about. The
/// honest reading is the fail-closed one: a row that claims resources is not a
/// row composition can discharge. The target's own resources stay its own
/// claim, proved by its own demands.
fn composed_operation_states_the_same_claim(
    composing: &solid_reactive_ir::contract_semantics::Operation,
    target: &solid_reactive_ir::contract_semantics::Operation,
) -> bool {
    composing.resources.is_empty()
        && composing.kind == target.kind
        && composing.inputs == target.inputs
        && composing.output == target.output
        && composing.at == target.at
        && composing.schedule == target.schedule
        && composing.cardinality == target.cardinality
        && composing.tracking == target.tracking
        && composing.owner == target.owner
        && composing.guard == target.guard
        && composing.trigger == target.trigger
}

/// Whether one census call is the composing call: a reachable, uncaptured
/// *call* whose callee resolves to exactly the named export's own declaration.
///
/// Identity, on four facts at once — the declaration symbol, its source file,
/// its exact byte range, and the export name the snapshot replay bound it to.
/// The symbol alone would be enough for a producer that always states one, so
/// an empty symbol is refused rather than treated as a wildcard; the rest are
/// there because a premise this cheap should not depend on the producer's
/// habits. What is deliberately *not* enough is the name: another module's
/// `createPolled` carries the same `declaration.name`, and clearing a call to
/// it is the scoping study's "some other export proves it" restated.
///
/// The kind, floor and captured gates are the same three this module applies
/// everywhere. Uncaptured is the load-bearing one here: a direct call is the
/// same stack, so the composed row's `schedule: same-stack` stamp survives the
/// hop, and a call from inside a closure this export hands away does not.
fn composing_call_resolves_to_declaration(
    call: &typefacts::ImplementationCall,
    target: &typefacts::ResolvedDeclaration,
    target_runtime_export: &str,
    floor: ReachabilityFloor,
) -> bool {
    if !is_call_expression(call) || !floor.admits(call.reach) || call.captured {
        return false;
    }
    let Some(declaration) = &call.declaration else {
        return false;
    };
    !declaration.symbol.is_empty()
        && declaration.symbol == target.symbol
        && declaration.source_file == target.source_file
        && declaration.location.start_byte == target.location.start_byte
        && declaration.location.end_byte == target.location.end_byte
        && declaration.name.as_ref() == target_runtime_export
}

/// The furthest a composed operation's provenance chain may be followed.
///
/// Summary reads propagate transitively across call edges with no bound at the
/// propagation site, so a published chain can be arbitrarily deep. Eight is
/// the bound the execution premises already use
/// ([`MAX_EXECUTION_PREMISE_DEPTH`]); reaching it refuses rather than
/// approximating.
///
/// A cycle back through the *demanded* export is refused earlier and by name:
/// the recursion keeps the original `proof`, so `composing_export` stays the
/// export the demand is about, and a provenance that names it again takes the
/// self-composition refusal. A cycle among other exports runs out of depth
/// instead, which is the same verdict by a longer route.
const MAX_COMPOSITION_DEPTH: usize = 8;

/// One resolved composition target: the named export's semantics, its own
/// implementation census, and the export name the snapshot replay bound it to.
///
/// Everything the composition premise needs about a target and nothing about
/// how it was found, so the premise can be driven by a test resolver as well
/// as by the certification plan.
#[derive(Clone, Copy)]
struct ComposedTarget<'a> {
    export: &'a solid_reactive_ir::contract_semantics::ExportSemantics,
    implementation: &'a typefacts::ExportImplementationTranscript,
    runtime_export: &'a str,
}

/// Implementation evidence that a composed operation's claim holds: this
/// export performs the named export's named operation, through its own call to
/// it.
///
/// Two premises, both required, and neither is a search.
///
/// **(a) The composing call.** Some call in *this* export's census must be a
/// `call` (never a construction), admitted by the operation's own floor,
/// **uncaptured**, and resolve *by declaration identity* to the named export.
/// Identity means the declaration symbol and source range of the call's callee
/// equal those of the named export's own implementation declaration — the one
/// the caller already matched against the snapshot-replayed runtime binding for
/// that name. A name comparison would clear a call to a *different*
/// `createPolled` in another module, which is the scoping study's warning
/// restated as code: provenance names an exact target, and "some export of
/// this artifact case proves it" is not a premise.
///
/// The uncaptured gate is what keeps the composed row's `at: call /
/// schedule: same-stack` stamp honest across the hop. A direct call is the
/// same stack; a call inside a closure this export hands to `createEffect` is
/// not, and a read reached only that way must refuse rather than compose.
/// `Unreachable` clears nothing for the same reason it clears nothing
/// anywhere else.
///
/// **(b) The target's own claim.** The named operation must exist in the named
/// export, must be the *same claim* — see
/// [`composed_operation_states_the_same_claim`] — and must itself be
/// discharged from the named export's own census: recursively, when the target
/// is composed in turn, and by
/// [`require_reactive_read_operation_input`] when it is not. A composed row
/// therefore never states anything the target does not already state and
/// prove, and a target whose own demand refuses refuses the composed row too.
///
/// **Cycles are refused by the visited set, not by running out of depth.**
/// `visited` is seeded with the export the demand is about and grows by one
/// export per hop, so a provenance naming an export already on the chain — its
/// own included — refuses as a cycle. The depth bound is the separate,
/// blunter limit for an *acyclic* chain: summary reads propagate transitively
/// across call edges with no bound at the propagation site, so a document can
/// state a chain of arbitrarily many distinct exports, and
/// [`MAX_COMPOSITION_DEPTH`] refuses at the length the execution premises
/// already use.
///
/// Everything fails closed. A provenance the resolver cannot answer, an
/// operation absent from the target, a claim that differs in any field, a
/// composing row that claims resources of its own, a captured or unreachable
/// composing call, a cycle, and the depth bound each refuse with the target
/// named. The refusal is a `String` rather than a
/// `TypeFactsCertificationError` because the demand owns the error: a
/// composition premise that fails is a refusal of the *demand*, not of the
/// export it was trying to compose from.
fn require_composed_operation_chain<'a>(
    composing_export: &str,
    composing_operation: &'a solid_reactive_ir::contract_semantics::Operation,
    composed: &'a solid_reactive_ir::contract_semantics::ComposedFrom,
    implementation: &'a typefacts::ExportImplementationTranscript,
    resolve: &dyn Fn(&str) -> Result<ComposedTarget<'a>, String>,
    visited: &mut Vec<String>,
    depth: usize,
) -> Result<Vec<String>, String> {
    if depth >= MAX_COMPOSITION_DEPTH {
        return Err(format!(
            "composed operation chain exceeds {MAX_COMPOSITION_DEPTH} hops at {}:{}",
            composed.export, composed.operation.0
        ));
    }
    // A self-composition is a cycle, not a proof. The model refuses to publish
    // one (`normalize_operation`), and this refuses to read one: the two
    // checks are on opposite sides of the wire and a document this side did
    // not generate reaches only the second.
    if composed.export == composing_export {
        return Err(format!(
            "composed operation names its own export {composing_export}"
        ));
    }
    if visited.contains(&composed.export) {
        return Err(format!(
            "composed operation chain revisits {} at {}",
            composed.export, composed.operation.0
        ));
    }
    let target = resolve(&composed.export)?;
    let target_operation = target
        .export
        .operation(&composed.operation.0)
        .ok_or_else(|| {
            format!(
                "composed operation {} is absent from {}",
                composed.operation.0, composed.export
            )
        })?;
    if !composed_operation_states_the_same_claim(composing_operation, target_operation) {
        return Err(format!(
            "composed operation {}:{} does not state the same claim as the composing row",
            composed.export, composed.operation.0
        ));
    }
    // (a) The composing call, by declaration identity.
    let target_declaration = target
        .implementation
        .declaration
        .as_ref()
        .ok_or_else(|| "composed operation target states no declaration".to_owned())?;
    let floor = operation_reachability_floor(composing_operation);
    let mut composing = implementation
        .calls
        .iter()
        .filter(|call| {
            composing_call_resolves_to_declaration(
                call,
                target_declaration,
                target.runtime_export,
                floor,
            )
        })
        .map(|call| {
            format!(
                "composed-operation-call:{}:{}:{}:{}:{}",
                call.location.path,
                call.location.start_byte,
                call.location.end_byte,
                composed.export,
                composed.operation.0
            )
        })
        .collect::<Vec<_>>();
    if composing.is_empty() {
        return Err(format!(
            "composed operation {}:{} has no reachable uncaptured call of {} resolving to its own declaration",
            composed.export, composed.operation.0, composed.export
        ));
    }
    // (b) The target's own evidence, from the target's own census.
    visited.push(composed.export.clone());
    let mut target_sites = Vec::new();
    if let Some(next) = &target_operation.composed_from {
        target_sites = require_composed_operation_chain(
            &composed.export,
            target_operation,
            next,
            target.implementation,
            resolve,
            visited,
            depth + 1,
        )?;
    } else {
        let input = target_operation
            .inputs
            .first()
            .ok_or_else(|| "composed operation target has no input".to_owned())?;
        let role =
            reactive_read_operation_input_role(target_operation, input).ok_or_else(|| {
                format!(
                    "composed operation {}:{} is not a read of a locally created accessor",
                    composed.export, composed.operation.0
                )
            })?;
        let refuse = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: String::new(),
            reason: reason.to_owned(),
        };
        require_reactive_read_operation_input(
            target_operation,
            role,
            target.implementation,
            operation_reachability_floor(target_operation),
            &refuse,
            &mut target_sites,
        )
        .map_err(|error| match error {
            TypeFactsCertificationError::FamilyOpen { reason, .. } => reason,
            other => other.to_string(),
        })?;
    }
    composing.extend(target_sites);
    Ok(composing)
}

/// The plan-bound half of the composition premise: resolve each target out of
/// this plan and this session's transcripts, and let
/// [`require_composed_operation_chain`] decide.
///
/// Nothing here reaches outside the answer. A transcript is scheduled per
/// exported value, so the evidence for a composed row is always the same
/// session's, under the same producer identity, bound into the same evidence
/// root; a target the schedule holds no transcript for refuses rather than
/// being looked for anywhere else. Each target additionally clears the
/// transcript completeness and the authenticated runtime binding that the
/// demanded export cleared, through
/// [`require_named_export_implementation`] — otherwise the composition would
/// rest on a census this side never validated.
#[allow(clippy::too_many_arguments)]
fn require_composed_operation<'a>(
    plan: &'a CertificationPlan,
    proof: &ScheduledProofDemand,
    operation: &'a solid_reactive_ir::contract_semantics::Operation,
    composed: &'a solid_reactive_ir::contract_semantics::ComposedFrom,
    implementation: &'a typefacts::ExportImplementationTranscript,
    transcripts: ExportTranscripts<'a>,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let (artifact_case, composing_export) = proof_artifact_export(&proof.subject);
    let resolve = |export_name: &str| -> Result<ComposedTarget<'a>, String> {
        let transcript = transcripts.find(artifact_case, export_name).ok_or_else(|| {
            format!(
                "composed operation names {export_name}, for which this session scheduled no export-value transcript"
            )
        })?;
        let (export, implementation) = require_named_export_implementation(
            plan,
            proof,
            artifact_case,
            export_name,
            transcript,
            open,
        )
        .map_err(|error| error.to_string())?;
        let (_, runtime_export, _, _) = plan
            .verified_exports
            .runtime_binding(export_name)
            .ok_or_else(|| {
                format!(
                    "composed operation names {export_name}, which has no exact identifier runtime binding"
                )
            })?;
        Ok(ComposedTarget {
            export,
            implementation,
            runtime_export,
        })
    };
    let mut visited = vec![composing_export.to_owned()];
    let witnesses = require_composed_operation_chain(
        composing_export,
        operation,
        composed,
        implementation,
        &resolve,
        &mut visited,
        0,
    )
    .map_err(|reason| open(&reason))?;
    sites.extend(witnesses);
    Ok(())
}

/// The stable spelling of a reactive role, for refusal text.
const fn reactive_role_name(role: ReactiveRole) -> &'static str {
    match role {
        ReactiveRole::Accessor => "reactive/accessor",
        ReactiveRole::Setter => "reactive/setter",
    }
}

/// The role the dialect vocabulary would have to agree on for this shape.
const fn dialect_reactive_role(role: ReactiveRole) -> solid_dialect::ReactiveRole {
    match role {
        ReactiveRole::Accessor => solid_dialect::ReactiveRole::Accessor,
        ReactiveRole::Setter => solid_dialect::ReactiveRole::Setter,
    }
}

/// The result slot a traced call-result source names, when it names one
/// exactly.
///
/// An empty target path is the whole result; one tuple segment carrying an
/// index is that slot. Every other path — a property, a deeper path, a tuple
/// segment with no index — names something this table does not answer, and
/// answering it anyway is how a slot claim becomes a guess.
fn traced_result_slot(
    source: &typefacts::ImplementationValueSource,
) -> Option<solid_dialect::ResultSlot> {
    match source.target_path.as_slice() {
        [] => Some(solid_dialect::ResultSlot::Whole),
        [segment] if segment.kind == PathSegmentKind::Tuple => {
            segment.index.map(solid_dialect::ResultSlot::TupleItem)
        }
        _ => None,
    }
}

/// Whether one traced argument source proves the demanded reactive role.
///
/// Three premises, each owned here rather than by the producer, and each
/// enforced once: the caller selects the sources whose `path` is empty, so this
/// does not restate that.
///
/// The trace has to be of a call *result* with a resolved callee. The kind gate
/// stays even though today's producer emits a `DirectCallable` with no target
/// — a function literal is not a dialect call, and
/// `reactive_operation_input_refuses_every_unproven_provenance` pins what
/// deleting the gate would cost by handing this a `DirectCallable` carrying a
/// complete, plausible `createSignal` target. The `(name, slot)` pair has to be
/// one every dialect that exports the name agrees is exactly this role, which
/// is where a setter handed to an accessor position is refused. And the module
/// the producer states has to be one a dialect exports that name from in value
/// position, which is what keeps a package's own `function createSignal()` —
/// traced with an empty module — from answering for the dialect's.
fn traced_source_proves_role(
    source: &typefacts::ImplementationValueSource,
    role: ReactiveRole,
) -> bool {
    source.kind == typefacts::ImplementationValueSourceKind::CallResult
        && !source.target.is_empty()
        && solid_dialect::exports_value_from(&source.target_module, &source.target_name)
        && traced_result_slot(source).is_some_and(|slot| {
            solid_dialect::unambiguous_reactive_result_slot(&source.target_name, slot)
                == Some(dialect_reactive_role(role))
        })
}

/// Implementation evidence that an `invoke` operation's input carries the
/// demanded reactive role.
///
/// The claim is universally quantified over the callback's call sites, and it
/// has to be: `cb(accessor); cb(plainValue);` makes "there is a call that hands
/// an accessor" true and the operation's input shape false. So every admitted
/// call must prove it, and a call whose slot traces to nothing refuses the
/// whole demand rather than being skipped — an empty trace is the producer's
/// silence, never a plain value.
///
/// **The floor is `MayExecute`, deliberately, and this is the one branch here
/// where that is not a relaxation.** The other operation-input branches assert
/// what a position *is* and keep the strict floor, because a call in a loop
/// body proves a parameter callable no more than it proves the loop runs. This
/// demand's claim is conditional in the same shape a conditional call is:
/// *whenever* this invoke happens, input `index` has this shape. Whether the
/// invoke happens at all is the `operation-reachability` demand's question, and
/// that one is answered separately through `callback.from`. `Unreachable` still
/// clears nothing: a call after a `return` never happens, so it neither
/// witnesses the shape nor contradicts it, and a census whose only matching
/// call is unreachable leaves the set empty and refuses.
///
/// `captured` is not a veto for the same reason — marker's `mapMatch(text)`
/// sits inside a `createRoot(dispose => …)` callback, and a claim about what
/// that call passes does not depend on proving the closure runs. But the
/// enclosing callable is recorded in the witness, so the audit shows which body
/// each site sits in; a producer that says `captured` without naming one is
/// refused rather than recorded as if it sat in the export's own body.
///
/// **Quantifying over the call census alone is not universal quantification,
/// and that gap was a false-certification route.** The census states
/// `calleeParameter` only for a call whose callee resolves to the parameter
/// *exactly*, so five shapes that run the callback are invisible to it, each
/// measured against the real producer: `const f = cb; f(plain)` (the call's
/// callee is nil), `cb.call(null, plain)` and `cb.apply(…)` (the callee is the
/// parameter at path length 1, which
/// [`parameter_value_source_exact`] refuses), `Reflect.apply(cb, …)` and
/// `holder.cb(plain)` (nil again), and `new cb(plain)` — a construction, for
/// which the producer states no `calleeParameter` *by design*. A census of
/// `[cb(accessor), <any of those>]` satisfied "every matching call proves the
/// role" while the export handed the callback a plain value.
///
/// So the premise is **the use census accounts for the callback exactly as the
/// calls this proof proved**: every use rooted at the callback parameter that
/// the floor admits must *be the callee of* one of those calls. Not "lie
/// inside" one — byte containment is the wrong relation, and reading it as
/// coverage was a second false-certification route: `cb(text, cb)` and
/// `cb(text, (held = cb))` put an `argumentKnown` and an `unknownEscape` use
/// *inside* the proved call's own span, so a containment premise cleared them
/// while the callback was handed to something else in the same call.
///
/// Identity is positional, and exactly so: a simple identifier callee is the
/// call expression's first token, so the callee use and the call start at the
/// same byte and the use ends inside the call. `cb?.(x)` also starts at `cb`.
/// A parenthesized `(cb)(x)` starts the call one byte earlier than the use and
/// is refused; that is an accepted over-refusal, pinned as one.
///
/// This subsumes a use-kind gate rather than restating it: `storage`,
/// `aliasCall`, `propertyAccess`, `argumentKnown`, `return` and
/// `unknownEscape` uses are the callee of no proved call, and neither is the
/// `directCall` use inside `new cb(plain)`, because a construction is not one
/// of the calls examined. It keeps the shape the proof must accept: marker's
/// captured `cb(text)` is a `capture` use that *is* the callee of its proved
/// call.
///
/// Two premises the census has to meet before any of that means anything. The
/// transcript must be **complete with no open reason at all** — not even
/// `controlFlowUnsupported`, which `require_export_implementation` otherwise
/// admits: the use census withholds rows inside an unsafe-jump region, and a
/// withheld escape is exactly what this premise cannot afford. And the
/// callback must be bound to a *whole* parameter: the census is rooted at
/// parameters, so for a callback at `Parameter{0, ["cb"]}` a use of `props` is
/// indistinguishable from a use of `props.cb`, and a premise that skipped
/// those rows would be vacuous precisely where the value can escape.
fn require_reactive_operation_input(
    export: &solid_reactive_ir::contract_semantics::ExportSemantics,
    operation: &solid_reactive_ir::contract_semantics::Operation,
    index: usize,
    role: ReactiveRole,
    implementation: &typefacts::ExportImplementationTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let callback = export
        .callbacks()
        .items()
        .iter()
        .find(|callback| callback.operation == operation.id)
        .ok_or_else(|| open("reactive operation input has no exact callback source"))?;
    let source = &callback.from;
    if !matches!(source, ValueSource::Parameter { path, .. } if path.is_empty()) {
        return Err(open(
            "reactive operation input's callback is not bound to a whole parameter",
        ));
    }
    if !implementation.complete || !implementation.open_reasons.is_empty() {
        return Err(open(&format!(
            "implementation use census is not exhaustive (complete={}, reasons={:?})",
            implementation.complete, implementation.open_reasons
        )));
    }
    let floor = ReachabilityFloor::MayExecute;
    let mut witnesses = Vec::new();
    let mut proved = Vec::new();
    for call in &implementation.calls {
        let names_callback = call.callee_parameter.as_ref().is_some_and(|actual| {
            parameter_binding_matches(actual.parameter_index, &actual.path, source)
        });
        // The kind gate is load-bearing rather than decorative, for the reason
        // spelled out on `require_parameter_read_call`: this side does not
        // certify against the producer's habits. Today a construction states no
        // `calleeParameter` at all — which is why the coverage premise below is
        // what catches `new cb(plain)` — but a producer that did state one must
        // not have it read as a call, at any path length.
        if names_callback && !is_call_expression(call) {
            return Err(open(&format!(
                "callback parameter is constructed rather than called at {}:{}:{}",
                call.location.path, call.location.start_byte, call.location.end_byte
            )));
        }
        if !is_call_expression(call)
            || !call
                .callee_parameter
                .as_ref()
                .is_some_and(|actual| parameter_value_source_exact(actual, source))
        {
            continue;
        }
        if !floor.admits(call.reach) {
            continue;
        }
        if call.captured && call.enclosing_callable.is_none() {
            return Err(open(
                "reactive operation input has a captured call site with no enclosing callable",
            ));
        }
        let traced = call
            .argument_sources
            .get(index)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let rooted = traced
            .iter()
            .filter(|source| source.path.is_empty())
            .collect::<Vec<_>>();
        if rooted.is_empty() {
            return Err(open(&format!(
                "reactive operation input {index} has no traced value at {}:{}:{}",
                call.location.path, call.location.start_byte, call.location.end_byte
            )));
        }
        if !rooted
            .iter()
            .all(|source| traced_source_proves_role(source, role))
        {
            return Err(open(&format!(
                "reactive operation input {index} at {}:{}:{} does not trace to an unambiguous dialect {} result; observed {:?}",
                call.location.path,
                call.location.start_byte,
                call.location.end_byte,
                reactive_role_name(role),
                rooted
                    .iter()
                    .map(|source| (
                        source.target_module.as_ref(),
                        source.target_name.as_ref(),
                        traced_result_slot(source)
                    ))
                    .collect::<Vec<_>>()
            )));
        }
        for source in rooted {
            witnesses.push(format!(
                "reactive-operation-input:{}:{}:{}:{}:{}:{}:{}:{}",
                call.location.path,
                call.location.start_byte,
                call.location.end_byte,
                index,
                source.target_module,
                source.target_name,
                traced_result_slot_site(source),
                enclosing_callable_site(call)
            ));
        }
        proved.push(&call.location);
    }
    if witnesses.is_empty() {
        return Err(open(&format!(
            "reactive operation input {index} has no call of the exact callback parameter the implementation may reach"
        )));
    }
    for usage in &implementation.parameter_uses {
        if !parameter_binding_matches(usage.parameter_index, &usage.binding_path, source)
            || !floor.admits(usage.reach)
        {
            continue;
        }
        if !proved
            .iter()
            .any(|proved| location_is_callee_of(&usage.location, proved))
        {
            return Err(open(&format!(
                "callback parameter use {} at {}:{}:{} is not the callee of a proved call of it",
                parameter_use_kind_site(usage.kind),
                usage.location.path,
                usage.location.start_byte,
                usage.location.end_byte
            )));
        }
    }
    sites.extend(witnesses);
    Ok(())
}

/// Whether `use_site` is the *callee* of the call at `call`, positionally.
///
/// A call expression begins at its callee, so an identifier callee starts at
/// exactly the call's own start byte and ends inside it — `cb(x)` and
/// `cb?.(x)` both. Deliberately not containment: an argument of the same call
/// is contained by it and is not its callee, which is the whole distinction
/// this premise turns on. A callee written with parentheses, `(cb)(x)`, starts
/// one byte later than its call and is refused here; over-refusing that is the
/// safe side of a positional rule.
///
/// It is never a claim about *execution*. That a use is a call's callee says
/// nothing about whether the call runs; the floor answers that separately.
fn location_is_callee_of(use_site: &typefacts::Location, call: &typefacts::Location) -> bool {
    use_site.path == call.path
        && use_site.start_byte == call.start_byte
        && use_site.end_byte <= call.end_byte
}

/// The stable spelling of a traced result slot for a witness site.
fn traced_result_slot_site(source: &typefacts::ImplementationValueSource) -> String {
    match traced_result_slot(source) {
        Some(solid_dialect::ResultSlot::Whole) => "whole".to_owned(),
        Some(solid_dialect::ResultSlot::TupleItem(index)) => format!("tuple:{index}"),
        None => "unaddressed".to_owned(),
    }
}

/// The body a call sits in, named exactly: the innermost callable containing
/// it, or the implementation's own body.
fn enclosing_callable_site(call: &typefacts::ImplementationCall) -> String {
    match &call.enclosing_callable {
        Some(location) => format!(
            "enclosing:{}:{}:{}",
            location.path, location.start_byte, location.end_byte
        ),
        None => "enclosing:implementation-body".to_owned(),
    }
}

fn require_operation_recursive_subject(
    plan: &CertificationPlan,
    proof: &ScheduledProofDemand,
    transcript: &ExportValueTranscript,
    transcripts: ExportTranscripts<'_>,
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
        let index = usize::from(*index);
        let input = operation
            .inputs
            .get(index)
            .ok_or_else(|| open("recursive input index is absent"))?;
        // The reactive-input arm is answered *before* the parameter root is
        // required, because this input is deliberately not parameter-rooted:
        // the IR knows the value is a dialect accessor the export created
        // itself, and the shape is what it has left to say so with.
        if let Some(role) = reactive_operation_input_role(operation, input, path, *callable) {
            let (export_semantics, implementation) =
                require_export_implementation(plan, proof, transcript, open)?;
            require_reactive_operation_input(
                export_semantics,
                operation,
                index,
                role,
                implementation,
                open,
                sites,
            )?;
            return Ok(());
        }
        // The `read` half of the same class, at the root path only: the demand
        // asks about the whole value, and the census traces the callee
        // *expression* and says nothing about a property inside the value it
        // traces. The floor is the operation's own stated lower bound, the same
        // derivation `require_operation_evidence` uses for the two families
        // that reach the read arm there — this family carries no bound of its
        // own to read.
        if path.0.is_empty() && *callable == DemandedCallability::Unknown {
            if let Some(composed) = &operation.composed_from {
                let (_, implementation) =
                    require_export_implementation(plan, proof, transcript, open)?;
                require_composed_operation(
                    plan,
                    proof,
                    operation,
                    composed,
                    implementation,
                    transcripts,
                    open,
                    sites,
                )?;
                return Ok(());
            }
            if let Some(role) = reactive_read_operation_input_role(operation, input) {
                let (_, implementation) =
                    require_export_implementation(plan, proof, transcript, open)?;
                require_reactive_read_operation_input(
                    operation,
                    role,
                    implementation,
                    operation_reachability_floor(operation),
                    open,
                    sites,
                )?;
                return Ok(());
            }
        }
        let (parameter, mut source_path) =
            operation_input_parameter_root(input, proof, operation.id.0.as_str(), index)?;
        let path_is_exact_properties = path.0.iter().all(|segment| match segment {
            ValuePathSegment::ObjectProperty(property) => {
                source_path.push(property.clone());
                true
            }
            ValuePathSegment::ChoiceAlternative(_) => true,
            _ => false,
        });
        let source = ValueSource::Parameter {
            index: parameter,
            path: source_path,
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
    if let ValueRoot::OperationOutput { operation } = root {
        let operation = exported
            .operation(&operation.0)
            .ok_or_else(|| open("recursive output operation is absent"))?;
        if let Some(ValueShape::Parameter {
            index,
            path: parameter_path,
        }) = &operation.output
        {
            if operation.kind != OperationKind::Return
                || !path.0.is_empty()
                || !parameter_path.is_empty()
                || *callable != DemandedCallability::Unknown
            {
                return Err(open(
                    "returned parameter identity requires an unasserted whole parameter root",
                ));
            }
            let (_, implementation) = require_export_implementation(plan, proof, transcript, open)?;
            return require_returned_parameter_identity(implementation, *index, open, sites);
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

fn require_returned_parameter_identity(
    implementation: &typefacts::ExportImplementationTranscript,
    index: u16,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
    sites: &mut Vec<String>,
) -> Result<(), TypeFactsCertificationError> {
    let flow = implementation
        .control_flow
        .as_ref()
        .ok_or_else(|| open("returned parameter identity has no control-flow census"))?;
    // Which incompleteness the identity fact can stand beside (ADR 0016,
    // amendment of 2026-09-05). The fact itself is flow-insensitive: the
    // producer emits it only when the parameter's symbol is assigned nowhere
    // in the implementation and the body mentions neither `eval` nor
    // `arguments`, so no construct's flow can change *which value* a return
    // site carries. What flow decides is only whether a return is reached, and
    // a `reachability-lower-bound` construct -- a loop, `try`, or `switch` the
    // census walked in full -- leaves that answered for every site outside it:
    // the producer keeps `reachable` across a construct that completes
    // normally and marks the sites inside `unknown`, which the agreement check
    // below still reads (`unknown` is not `unreachable`). A `flow-unaccounted`
    // construct answers neither question, and an unclassified marker is a
    // producer that did not say which, so both keep the fact open.
    let unaccounted = flow
        .incompleteness
        .iter()
        .any(|row| row.class != typefacts::ControlFlowIncompletenessClass::ReachabilityLowerBound);
    let unclassified = !flow.unsupported.is_empty() && flow.incompleteness.is_empty();
    if unaccounted
        || unclassified
        || !flow.returns.iter().any(|site| {
            site.reach == Reachability::Reachable
                && site.carry_reach == Some(Reachability::Reachable)
        })
    {
        return Err(open(
            "returned parameter identity needs complete flow and an unconditional return",
        ));
    }
    let returns = flow
        .returns
        .iter()
        .filter(|site| site.reach != Reachability::Unreachable)
        .collect::<Vec<_>>();
    if returns.iter().any(|site| {
        !site.parameter.as_ref().is_some_and(|source| {
            source.parameter_index == usize::from(index) && source.path.is_empty()
        })
    }) {
        return Err(open(
            "returned parameter identity is absent or does not match the demanded parameter",
        ));
    }
    sites.extend(returns.into_iter().map(|site| {
        format!(
            "implementation-returned-parameter:{index}:{}:{}:{}",
            site.location.path, site.location.start_byte, site.location.end_byte,
        )
    }));
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
            let index = usize::from(*index);
            let input = operation
                .inputs
                .get(index)
                .ok_or_else(|| open("recursive input index is absent"))?;
            let (parameter_index, path) =
                operation_input_parameter_root(input, proof, operation.id.0.as_str(), index)?;
            let parameter = signature
                .parameters
                .get(usize::from(parameter_index))
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

/// Whether a path fact belongs to the declared-member census that closure is a
/// claim about.
///
/// An apparent fact does not. It describes a member the value carries through
/// the compiler's global-`Function` augmentation — `bind`, `call`, `prototype`
/// — which is library-owned, never caller-supplied, and emitted as a leaf the
/// producer does not descend into. Two of those members are declared `any`, so
/// requiring them to be closed would refuse every callable value on the
/// strength of a `Function.prototype` the package under analysis never wrote.
/// Closure is therefore a claim about the members some declaration in the
/// package's own surface writes down; an exact path lookup still reads the
/// apparent facts, so nothing is hidden from a demand that names one.
fn callable_path_is_declared_census_member(path: &typefacts::CallablePathFact) -> bool {
    !path.apparent
}

fn require_export_callable_paths_closed(
    transcript: &ExportValueTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    for path in transcript
        .callable_paths
        .iter()
        .filter(|path| callable_path_is_declared_census_member(path))
    {
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

    let package_marker = snapshot_package_marker(plan);
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
    let source_roots =
        snapshot_source_roots(plan, dependencies, graph_sources, project, &package_marker)?;
    let source_root_paths = source_roots
        .iter()
        .map(|root| root.path.clone())
        .collect::<Vec<_>>();

    // A declaration of this package is looked up under the roots materialized
    // from *this node's snapshot*, never by path suffix. A graph materializes
    // one package name at several coordinates: a nested copy of another version
    // under some dependency's `node_modules` ends in the same
    // `/node_modules/<name>/<relative>` suffix and the suffix census read it as a
    // duplicate; and the same snapshot can stand at a hoisted coordinate and a
    // nested one at once, with the program reading whichever the specifier
    // resolves to, so the node's own root alone is too narrow. Every root of
    // the same snapshot holds the same bytes, and each match is still held to
    // the closure's digest. The site string keeps the package-marker form, so
    // every evidence root that already passed is unchanged.
    let snapshot_roots = source_roots
        .iter()
        .filter(|root| root.snapshot.root() == plan.snapshot.root())
        .map(|root| root.path.as_str())
        .collect::<Vec<_>>();
    for entry in &plan.verified_closure.manifest().entries {
        if entry.role != ClosureFileRole::Declaration {
            continue;
        }
        let relative = entry.path.trim_start_matches("./").replace('\\', "/");
        let suffix = format!("{package_marker}{relative}");
        let matches = declaration_sources_under_roots(sources, &snapshot_roots, &relative);
        if matches.is_empty()
            || matches
                .iter()
                .any(|source| source.sha256.as_ref() != entry.digest.as_str())
        {
            let observed = matches
                .iter()
                .take(4)
                .map(|source| format!("{}:{}", source.path, source.sha256))
                .collect::<Vec<_>>();
            return Err(TypeFactsCertificationError::SourceCensus(format!(
                "declaration {relative} is absent or stale under {snapshot_roots:?}; expected {}; observed(count={}, sample={observed:?})",
                entry.digest,
                matches.len(),
            )));
        }
        // `suffix` is this declaration's path in package-marker form; for a
        // hoisted copy that is also its project-relative path.
        sites.push(format!("typefacts-source:{suffix}:{}", matches[0].sha256));
    }

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

/// Every authenticated snapshot root the producer may have read source from,
/// longest path first, each carrying the snapshot it belongs to and whether it
/// is a dependency rather than the artifact under certification.
///
/// Extracted from [`verify_snapshot_source_census`] because two independent
/// consumers need the same list: that census, which checks every reported
/// source against it, and the `creates` implementation census, which asks it
/// which archive a resolved callee's declaration belongs to. Deriving it twice
/// would be two chances to derive it differently.
///
/// `dependency` is `false` only for the artifact under certification's own
/// roots. `deduplicate_snapshot_source_roots` refuses two distinct
/// authenticated snapshots claiming one materialized root, and narrows the flag
/// with `&=` where the same root is claimed twice, so the answer stays the
/// conservative one.
/// The path prefix a source of the artifact under certification occupies
/// inside the private project, and the fallback evidence prefix for it.
/// The reported sources that stand at `relative` directly under one of
/// `roots` (each a normalized root ending in `/`). Membership is by exact
/// root, so a copy of the same package name materialized at another
/// coordinate -- nested under some other dependency, or hoisted beside it --
/// is never mistaken for this one by the suffix its path shares.
fn declaration_sources_under_roots<'a>(
    sources: &'a [typefacts::TranscriptSourceDigest],
    roots: &[&str],
    relative: &str,
) -> Vec<&'a typefacts::TranscriptSourceDigest> {
    sources
        .iter()
        .filter(|source| {
            let normalized = source.path.replace('\\', "/");
            roots
                .iter()
                .any(|root| normalized.strip_prefix(root) == Some(relative))
        })
        .collect()
}

fn snapshot_package_marker(plan: &CertificationPlan) -> String {
    format!(
        "/node_modules/{}/",
        plan.snapshot.package_name().replace('\\', "/")
    )
}

fn snapshot_source_roots<'a>(
    plan: &'a CertificationPlan,
    dependencies: &[&'a CertificationPlan],
    graph_sources: &'a [super::dependencies::VerifiedGraphSourcePackage],
    project: Option<&PrivateTypeFactsProject>,
    package_marker: &str,
) -> Result<Vec<SnapshotSourceRoot<'a>>, TypeFactsCertificationError> {
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
            evidence_prefix: materialized_source_evidence_prefix(project, &root, package_marker)?,
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
    Ok(source_roots)
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
        .filter(|path| callable_path_is_declared_census_member(path))
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

/// Which Type Facts census is in hand when a domain-closure demand is verified.
///
/// The two carry different evidence, so they decide different claims, and
/// neither decides most of them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClosureCensus {
    /// The exported value's own observed shape: its alternative enumeration,
    /// its recursive open reasons, its finite partitions, and its declared
    /// callable-path census with every subtree enumerated.
    ExportedValue,
    /// One selected call signature's parameter and result values plus the
    /// implementation's control-flow branch census.
    Invocation,
    /// The demanded export's own `ExportImplementationTranscript`, its
    /// uncensused-invoking-form census, and the transcripts of every local
    /// declaration it reaches: the census of the **implementation**, which is
    /// the only census that decides a behavioral call domain
    /// (`docs/adr/0008-implementation-census-for-creates.md`).
    Implementation,
}

/// Refuses a domain-closure demand whose claim the census in hand does not
/// decide, naming the premise it lacks.
///
/// `DomainExhaustiveness` is one proof family over many different claims, and
/// both censuses above are censuses of the **declaration**. What a declaration
/// decides is narrow:
///
/// * A **root choice-alternatives** domain of the exported value. The producer
///   enumerates that value's alternatives itself, so the premise is its own
///   enumeration — which is why the proposal's enumeration is then required to
///   *be* that enumeration, in
///   [`require_export_value_enumeration_matches_census`]. Nothing weaker would
///   do: a closed declared type says the alternative set is finite and fully
///   observed, not that a proposal listing three of them is right.
/// * A **guard partition**, whose premise is a complete finite partition in a
///   control-flow census with no unsupported branch, alongside closed parameter
///   and result values. Only the invocation census carries branches.
///
/// * The **`creates` call domain**, and only when the census in hand is the
///   [`ClosureCensus::Implementation`] one. Its premise is the implementation
///   census of [`census_creates_domain`]: the export's own transcript at the
///   `MayExecute` floor with every invoking form enumerated, every call
///   dispositioned by its callee, and every local declaration recursed into
///   (`docs/adr/0008-implementation-census-for-creates.md`). The declaration
///   censuses still refuse it, because two exports with identical declarations
///   have identical declaration censuses.
/// * The **`returns` call domain**, under the same census, for the empty
///   closure only. Its premise is [`census_returns_domain`]: a plain (neither
///   `async` nor generator) implementation whose classified control-flow
///   census carries no value-carrying completion at the `MayExecute` floor
///   (`docs/adr/0035-returns-census-for-valueless-completion.md`).
///
/// Everything else refuses. Two cases are worth naming because they look close:
///
/// * The **other behavioral call domains** — `reads`, `writes`, `callbacks`,
///   `cleanups`, `disposals`, `invalidates`, and equally `throws` — are
///   decided by the implementation too, but no census of them exists
///   yet: `reads` additionally needs the proxy property-access forms, and
///   `throws` is not a census target under version 1 at all
///   (`phase21/2026-09-03-implementation-census-plan.md` § 4.4-4.5). Admitting
///   one for the declaration census would leave the probe gate's finite
///   non-observation as the only thing separating a certified row from a
///   refused one — closure decided by non-observation, which a veto may never
///   do.
/// * The **other value domains** — object properties, tuple items, array
///   bounds, capabilities, and any non-root path — are decided by a declaration
///   too, but this census hands over no enumeration of them to compare a
///   proposal against, so admitting them would certify the proposal's own word.
///
/// Both refusals are proof-mode work, recorded in `docs/precision-backlog.md`
/// and `docs/adr/0006-probe-harness-binding.md`.
fn require_census_decides_closure(
    proof: &ScheduledProofDemand,
    path: &SemanticClaimPath,
    census: ClosureCensus,
) -> Result<(), TypeFactsCertificationError> {
    let unsupported = |reason: String| TypeFactsCertificationError::UnsupportedDemand {
        demand: proof.id.clone(),
        reason,
    };
    match (census, path) {
        (
            ClosureCensus::ExportedValue,
            SemanticClaimPath::Domain(ClaimPath::Value {
                root: ValueRoot::Export,
                path: value_path,
                domain: ValueClaimDomain::ChoiceAlternatives,
            }),
        ) if value_path.0.is_empty() => Ok(()),
        (ClosureCensus::Invocation, SemanticClaimPath::Domain(ClaimPath::GuardPartition)) => Ok(()),
        // The behavioral call domains with a census: `creates` (ADR 0008) and
        // `returns` (ADR 0035), decided by the implementation census and
        // nothing else. Every other call domain keeps refusing by name below,
        // including under this census, because no census of *it* exists yet.
        (
            ClosureCensus::Implementation,
            SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Creates | ClaimDomain::Returns)),
        ) => Ok(()),
        (_, SemanticClaimPath::Domain(ClaimPath::Value { root, domain, .. })) => {
            Err(unsupported(format!(
                "value-enumeration premise required: closing the {} domain of a {} value needs a \
                 producer enumeration of that domain to compare the proposal against, and this \
                 census carries one only for the exported value's own choice alternatives",
                value_claim_domain_name(*domain),
                value_root_name(root)
            )))
        }
        (_, SemanticClaimPath::Domain(ClaimPath::GuardPartition)) => Err(unsupported(
            "control-flow-census premise required: closing a guard partition needs the branch \
             census, which an export-value transcript does not carry"
                .into(),
        )),
        (ClosureCensus::Implementation, SemanticClaimPath::Domain(ClaimPath::Call(domain))) => {
            Err(unsupported(format!(
                "implementation-census premise required: the implementation census decides the \
                 creates and returns call domains only; closing the {} call domain needs its own \
                 census of every invoking form that reaches it, which does not exist yet",
                call_claim_domain_name(*domain)
            )))
        }
        (_, SemanticClaimPath::Domain(ClaimPath::Call(domain))) => Err(unsupported(format!(
            "implementation-census premise required: closing the {} call domain needs a \
             complete ExportImplementationTranscript, every `calls` target resolved, and no \
             resolved target able to perform the domain's operation. A declaration census \
             cannot decide it, because two exports with identical declarations have identical \
             censuses",
            call_claim_domain_name(*domain)
        ))),
        (
            _,
            SemanticClaimPath::Domain(ClaimPath::Operation { .. } | ClaimPath::Resource { .. }),
        ) => Err(unsupported(
            "implementation-census premise required: closing an operation or resource \
                 domain needs the operation graph's own census, which no declaration decides"
                .into(),
        )),
        (_, SemanticClaimPath::Operation(_)) => Err(unsupported(
            "a domain-closure demand must name a claim domain, not one operation".into(),
        )),
    }
}

/// The one reason a censused call cannot perform a `create`.
///
/// Exactly one is assigned per call admitted by the `MayExecute` floor, in the
/// order the census tries them, and a call none of them fits refuses the domain
/// **by name**. The set is closed on purpose: adding a disposition is adding a
/// premise, and each of these six is a premise with an owner.
///
/// * `Unreachable` — the producer's control-flow census proves the call never
///   runs. `MayExecute` admits `Reachable` *and* `Unknown`, so an
///   uncertain-reach call is **in** the census and must be dispositioned like
///   any other; only `Unreachable` is excused
///   (`phase21/2026-09-03-implementation-census-plan.md` § 3 item 2).
/// * `ParameterRooted` — the callee is proven to be a caller-supplied callable
///   ([`typefacts::ImplementationCall::callee_parameter`]). What that callable
///   does is the caller's behavior, in the caller's artifact, under the
///   caller's own contract; this export's act is the *invocation*, which is a
///   `callbacks` item (§ 3.2). An **empty `callee_sources` is not this**: it
///   means the producer traced nothing, and an untraced callee is unresolved.
/// * `StandardLibrary` — the callee resolved, by default-library symbol
///   identity rather than by spelling, to a declaration the producer marks
///   [`typefacts::ResolvedDeclaration::standard_library`]. **The premise is
///   that a default-library member cannot register a version-1 resource into a
///   Solid runtime.** The reviewed set is the engine's own description of
///   itself (`lib.*.d.ts`); a `create` is the export registering a version-1
///   resource — an owner, a reactive source, an async computation, a
///   transition, a browser root, a server reference — into a runtime outside
///   the invocation (`semantic-model.md` § creates); and the engine has no such
///   kind to register, nor any Solid runtime to register it with.
///   `Array.prototype.map` invoking its callback is not a counterexample: the
///   callback's body is code written elsewhere, dispositioned where it is
///   written and not here.
/// * `DialectAxiom` — [`census_dialect_axiom_for_callee`] answered, which is
///   the identity-bound negative table of ADR 0007. It carries its own witness
///   site, so this disposition emits the tier's site instead of a generic one.
/// * `LocalRecursion` — the callee's declaration resolves inside the certified
///   artifact's **own runtime source set**, so the census recurses into that
///   declaration's own transcript. Identity is symbol + source file + exact
///   span, never name, and depth [`MAX_COMPOSITION_DEPTH`] refuses rather than
///   approximating.
/// * `LocalRecursionBackedge` — the same exact, stable local declaration is
///   already on the current census stack. For this zero-upper-bound claim the
///   edge closes a finite call-graph cycle: the target frame has already
///   passed its transcript premises, and every non-cycle edge in every frame
///   is still dispositioned normally. Re-entering the frame can repeat those
///   bodies or diverge, but cannot introduce an unenumerated `create`.
/// * `ParameterRootedAccessor` — ADR 0034. A *read accessor* form — a
///   `get-accessor` or a `property-access-unknown-accessor` on a property or
///   element access — whose subject the producer states is rooted at a plain,
///   unwritten parameter of this very declaration, or a `.call`/`.apply` of a
///   reviewed this-protocol member ([`CENSUS_THIS_PROTOCOL_MEMBERS`]) whose
///   `this` argument is so rooted. The code such a read can run — a getter, a
///   Proxy trap, a `Symbol.toStringTag` getter — sits on an object the caller
///   handed to this invocation, so it is the caller's behavior exactly as a
///   parameter-rooted *call* is; this export's act is the read. The producer
///   states the fact only under the premises its `subjectParameter` doc lists,
///   and only from handshake protocol
///   [`CENSUS_PARAMETER_ROOTED_SUBJECTS_PROTOCOL`] on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CensusDisposition {
    Unreachable,
    ParameterRooted,
    ParameterRootedAccessor,
    StandardLibrary,
    DialectAxiom,
    LocalRecursion,
    LocalRecursionBackedge,
}

impl CensusDisposition {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Unreachable => "unreachable",
            Self::ParameterRooted => "parameter-rooted",
            Self::ParameterRootedAccessor => "parameter-rooted-accessor",
            Self::StandardLibrary => "standard-library",
            Self::DialectAxiom => "dialect-axiom",
            Self::LocalRecursion => "local-recursion",
            Self::LocalRecursionBackedge => "local-recursion-backedge",
        }
    }
}

/// The handshake protocol at which
/// [`typefacts::ExportImplementationTranscript::uncensused_invoking_forms`]
/// became a positive claim rather than an absence.
///
/// Serde cannot separate an absent list from a present empty one — the field
/// defaults to empty — so the discriminator is the **protocol**, exactly as
/// that field's own documentation says. Verification already refuses an answer
/// whose `handshake_protocol` is not
/// [`typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL`], so this constant is the
/// census naming the dependency it rests on: against a producer predating the
/// field every empty list would read as "no uncensused form", which is exactly
/// the conclusion-from-silence this census exists to prevent.
const CENSUS_UNCENSUSED_FORMS_PROTOCOL: u64 = 14;

/// The handshake protocol at which the producer stopped **dropping** the `calls`
/// rows a `break`/`continue` region covers, and began classifying each
/// control-flow `unsupported` marker into
/// [`typefacts::ControlFlowIncompletenessClass`].
///
/// Both halves are premises of [`census_transcript_is_censusable`], and neither
/// can be established from the transcript's own bytes. A protocol-14 producer
/// silently withholds a row whose absence this census cannot detect — the whole
/// of ADR 0008 item 0 — and its `incompleteness` list decodes as empty, which is
/// indistinguishable from "nothing about this body is unmodelled". Reading
/// either as the admissible arm is the unsound direction, so an older producer's
/// transcript refuses **here**, by protocol number, before any marker is read.
const CENSUS_CONTROL_FLOW_CLASSES_PROTOCOL: u64 = 15;

/// The handshake protocol at which an uncensused form and a `.call`/`.apply`
/// row state the parameter their subject is rooted at (ADR 0034). Below it an
/// absent `subjectParameter` is a producer with no opinion, and reading it as
/// "not rooted" would only refuse — but reading a *present* one from a producer
/// that never promised the premise behind it would admit; the check is here for
/// the same reason the two constants above are.
const CENSUS_PARAMETER_ROOTED_SUBJECTS_PROTOCOL: u64 = 18;

/// The handshake protocol at which an implementation transcript states its
/// completion form (ADR 0035). A `returns` census on an older producer would
/// read an absent form as plain, which is the one reading the fact exists to
/// forbid.
const CENSUS_COMPLETION_FORM_PROTOCOL: u64 = 19;

/// One local declaration's own implementation transcript, keyed by the exact
/// span it was demanded at.
#[derive(Clone, Debug)]
struct LocalDeclarationTranscript {
    location: typefacts::Location,
    transcript: typefacts::ExportImplementationTranscript,
}

/// A function-like declaration inside the certified artifact's own runtime
/// source, by the identity the census matches on.
///
/// Symbol, source file and exact byte range — never the name. Another module's
/// `helper` carries the same `declaration.name`, and treating the two as one
/// declaration is the "some other export proves it" shortcut
/// [`composing_call_resolves_to_declaration`] already refuses. An empty symbol
/// is refused rather than treated as a wildcard.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CensusDeclarationIdentity {
    symbol: String,
    path: String,
    start: u64,
    end: u64,
}

impl CensusDeclarationIdentity {
    fn of(declaration: &typefacts::ResolvedDeclaration) -> Option<Self> {
        (!declaration.symbol.is_empty() && !declaration.location.path.is_empty()).then(|| Self {
            symbol: declaration.symbol.to_string(),
            path: declaration.location.path.replace('\\', "/"),
            start: declaration.location.start_byte,
            end: declaration.location.end_byte,
        })
    }
}

/// Everything the `creates` implementation census needs that the demanded
/// export's own transcript does not carry.
///
/// `roots` is [`snapshot_source_roots`]' answer, which is how a resolved
/// callee's declaration is attributed to an archive at all. `locals` carries
/// the local-declaration transcripts this same session answered for; an empty
/// slice can decide nothing recursive, and refuses rather than passing.
#[derive(Clone, Copy)]
struct CensusEvidence<'a> {
    roots: &'a [SnapshotSourceRoot<'a>],
    locals: &'a [LocalDeclarationTranscript],
}

impl CensusEvidence<'_> {
    fn local(
        &self,
        location: &typefacts::Location,
    ) -> Option<&typefacts::ExportImplementationTranscript> {
        self.locals
            .iter()
            .find(|local| {
                local.location.path == location.path
                    && local.location.start_byte == location.start_byte
                    && local.location.end_byte == location.end_byte
            })
            .map(|local| &local.transcript)
    }
}

/// Whether one census pass reached a verdict.
///
/// `NeedsTranscripts` is the acquisition phase's signal, never a verdict: the
/// walk reached a local declaration whose transcript this session has not asked
/// for, recorded it, and stopped short of deciding. The acquisition loop
/// demands the recorded declarations and runs the pass again; verification,
/// which has no session left to ask, refuses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CensusStep {
    Decided,
    NeedsTranscripts,
}

/// What a whole census concluded, with the totals its witness records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CensusOutcome {
    Decided { calls: usize, depth: usize },
    NeedsTranscripts,
}

/// One `creates` census in progress.
struct CensusRun<'a> {
    /// The artifact under certification, which is what a local-recursion
    /// disposition binds a callee's declaration to and what the dialect tier
    /// refuses to answer *about*. A snapshot rather than the whole plan so the
    /// dispositions are testable against a synthesized archive.
    certified: &'a super::ArtifactSnapshot,
    evidence: CensusEvidence<'a>,
    runtime_sources: std::collections::BTreeSet<String>,
    visited: Vec<CensusDeclarationIdentity>,
    requested: Vec<typefacts::Location>,
    sites: Vec<String>,
    calls: usize,
    deepest: usize,
    /// Each runtime source this census has had to read, keyed by
    /// snapshot-relative path. Parsed once per file from the authenticated
    /// bytes; `None` records a file that did not parse.
    sources: std::collections::BTreeMap<String, Option<CensusSourceFacts>>,
    /// The declaration node the transcript being walked belongs to, as a
    /// producer-side location: the file the producer names and the node's
    /// exact span. A callable literal handed to a standard-library invoker is
    /// admissible only when it lies inside this frame, because that is the
    /// span whose calls are rows of the transcript under the walk.
    frame: Option<typefacts::Location>,
}

/// One runtime source as the certifier reads it from the authenticated
/// snapshot bytes: the text a leading UTF-8 byte-order mark removed from, and
/// its own Oxc facts over that text.
///
/// The byte-order mark is stripped **before** the parse, and every offset this
/// census compares against the producer's is taken over the stripped text.
/// That is the producer's own convention: typescript-go's file decoder
/// (`internal/vfs/internal/internal.go`, `decodeBytes`) removes a UTF-8 BOM
/// before the source text exists, so every `Location` it emits counts bytes
/// from the first byte after the mark. Parsing the raw bytes instead would
/// leave the verifier's spans three bytes behind the producer's on a BOM'd
/// file, and the identifier-to-node binding below would then bind — or refuse
/// — the wrong bytes.
struct CensusSourceFacts {
    text: String,
    facts: solid_facts::ast::AstFacts,
}

/// One function-like declaration node of a runtime source, as the certifier
/// reads it from the authenticated snapshot bytes: its whole span, its body,
/// and the span of its name when it has one.
#[derive(Clone, Copy, Debug)]
struct CensusFunctionNode {
    /// The Oxc function node itself. Oxc excludes an enclosing `export`
    /// keyword from a function declaration's span.
    span: solid_facts::core::Span,
    /// The exact declaration span typescript-go accepts for a local
    /// declaration demand. For an inline exported declaration this is the
    /// enclosing export span; otherwise it is [`Self::span`].
    demand_span: solid_facts::core::Span,
    body: solid_facts::core::Span,
    name: Option<solid_facts::core::Span>,
}

impl CensusSourceFacts {
    fn function_nodes(&self) -> impl Iterator<Item = CensusFunctionNode> + '_ {
        self.facts.functions.iter().map(|function| {
            let name = function.name.as_ref().map(|name| name.span);
            // Oxc models `export function helper() {}` as an export node
            // wrapping a function whose own span starts at `function`.
            // typescript-go's FunctionDeclaration node starts at
            // `export`. Bind the two parsers through the export fact's
            // direct declaration name and shared end, rather than by
            // scanning source text for a modifier spelling.
            let demand_span = name
                .and_then(|name| {
                    self.facts
                        .exports
                        .iter()
                        .filter(|export| {
                            export.module.is_none()
                                && export.span.start <= function.span.start
                                && export.span.end == function.span.end
                                && export
                                    .declarations
                                    .iter()
                                    .any(|declaration| declaration.local.span == name)
                        })
                        .min_by_key(|export| export.span.end - export.span.start)
                        .map(|export| export.span)
                })
                .unwrap_or(function.span);
            CensusFunctionNode {
                span: function.span,
                demand_span,
                body: function.body,
                name,
            }
        })
    }

    fn text_at(&self, span: solid_facts::core::Span) -> Option<&str> {
        self.text.get(span.start as usize..span.end as usize)
    }
}

/// The text of an authenticated runtime source as the producer read it: UTF-8,
/// a leading byte-order mark removed. `None` when the bytes are not UTF-8.
fn census_source_text(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    Some(text.strip_prefix('\u{feff}').unwrap_or(text))
}

/// Whether `(start, end)` lies inside `span`, inclusive of its bounds.
fn census_span_contains(span: solid_facts::core::Span, start: u64, end: u64) -> bool {
    u64::from(span.start) <= start && end <= u64::from(span.end)
}

/// The implementation census that decides a `creates: []` closure for one
/// export of a consuming package.
///
/// The predicate, stated as the negation it establishes
/// (`phase21/2026-09-03-implementation-census-plan.md` § 3):
///
/// > `creates: []` certifies only if every invoking form in the export's
/// > transitive census, taken at the `MayExecute` reachability floor, is
/// > enumerated and resolved, and no resolved target performs a `create`
/// > operation.
///
/// Both halves are load-bearing. **Enumerated** is what
/// [`typefacts::ExportImplementationTranscript::uncensused_invoking_forms`]
/// answers: the calls census records `CallExpression` and `NewExpression` only,
/// so a tagged template, an accessor behind a property access, the iteration
/// protocol, a JSX lowering — everything else that reaches a callable — arrives
/// as a marker, and one marker at the floor refuses the domain by name. Silence
/// is never enumeration. **Performs a `create` operation** is the decidable
/// question — does any resolved target publish an operation of `kind: create`,
/// per `semantic-model.md` § creates — and deliberately *not* "brings a
/// version-1 resource into existence", which the audited corpus refutes as a
/// predicate: `createSignal`, `createStore`, `createMemo`, `action`,
/// `createEffect`, `createTrackedEffect` and `onSettled` all establish version-1
/// resources and all close `creates: []`. So a consumer that calls
/// `createSignal` certifies; a consumer that calls `@solidjs/web`'s `render`
/// does not, because `render` publishes `register-delegation`.
///
/// This family is **universal, not existential**: every call gets a disposition
/// and a witness site, and the first call with none refuses. A passing probe
/// gate, a finite non-observation, and an empty list are not evidence here and
/// never reach this function.
///
/// See `docs/adr/0008-implementation-census-for-creates.md` for the decision
/// and for what still refuses.
fn census_creates_domain(
    plan: &CertificationPlan,
    proof: &ScheduledProofDemand,
    export: &solid_reactive_ir::contract_semantics::ExportSemantics,
    implementation: &typefacts::ExportImplementationTranscript,
    evidence: CensusEvidence<'_>,
) -> Result<(CensusOutcome, Vec<String>, Vec<typefacts::Location>), TypeFactsCertificationError> {
    let refuse = |reason: String| TypeFactsCertificationError::UnsupportedDemand {
        demand: proof.id.clone(),
        reason,
    };
    if typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL < CENSUS_UNCENSUSED_FORMS_PROTOCOL {
        return Err(refuse(format!(
            "implementation-census premise required: the uncensused-invoking-form census arrived \
             at handshake protocol {CENSUS_UNCENSUSED_FORMS_PROTOCOL} and this build speaks {}, \
             so an empty form list would be an absence read as an enumeration",
            typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL
        )));
    }
    if typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL < CENSUS_PARAMETER_ROOTED_SUBJECTS_PROTOCOL {
        return Err(refuse(format!(
            "implementation-census premise required: the parameter-rooted subject fact arrived \
             at handshake protocol {CENSUS_PARAMETER_ROOTED_SUBJECTS_PROTOCOL} and this build \
             speaks {}, so a stated subject parameter would carry a premise this build never \
             reviewed",
            typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL
        )));
    }
    if typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL < CENSUS_CONTROL_FLOW_CLASSES_PROTOCOL {
        return Err(refuse(format!(
            "implementation-census premise required: unwithheld call rows and classified \
             control-flow incompleteness arrived at handshake protocol \
             {CENSUS_CONTROL_FLOW_CLASSES_PROTOCOL} and this build speaks {}, so an empty \
             incompleteness list would be an absence read as an admissible construct while rows \
             a jump region covers were still being dropped",
            typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL
        )));
    }
    // The census derives its own enumeration of this export's `create`
    // operations and requires the proposal's to *be* it, exactly as
    // `require_export_value_enumeration_matches_census` does for root choice
    // alternatives. A candidate's set is empty by construction — the generator
    // proposes `Complete(vec![])` and `normalize_knowledge` weakens it into the
    // candidate — so a nonempty one is a proposal claiming a closure over
    // operations this walk never enumerated, and admitting it would certify the
    // proposal's own word (objection 5 of ADR 0006).
    let proposed = export
        .operation_claim(ClaimDomain::Creates)
        .ok_or_else(|| refuse("creates is not an operation domain of this export".into()))?;
    if !proposed.items().is_empty() {
        return Err(refuse(format!(
            "a creates closure candidate must enumerate no operation, but the proposal names {}",
            proposed.items().len()
        )));
    }
    let mut run = CensusRun {
        certified: &plan.snapshot,
        evidence,
        runtime_sources: certified_runtime_sources(plan),
        visited: Vec::new(),
        requested: Vec::new(),
        sites: Vec::new(),
        calls: 0,
        deepest: 0,
        sources: std::collections::BTreeMap::new(),
        frame: None,
    };
    // Seeded with the demanded export, so a helper calling back into it refuses
    // as a cycle rather than running out of depth.
    if let Some(identity) = implementation
        .declaration
        .as_ref()
        .and_then(CensusDeclarationIdentity::of)
    {
        run.visited.push(identity);
    }
    let step = census_transcript(&mut run, implementation, 0).map_err(refuse)?;
    let mut sites = std::mem::take(&mut run.sites);
    let outcome = match step {
        CensusStep::Decided => {
            // One line for the whole census, and it is a *claim*: the producer
            // classified every invoking form it walked in every transcript this
            // census read, and none of them was a form the calls census does
            // not record. A census that refused would never reach here.
            sites.push("census-uncensused-forms:0".into());
            sites.push(format!("census-total:{}:{}", run.calls, run.deepest));
            CensusOutcome::Decided {
                calls: run.calls,
                depth: run.deepest,
            }
        }
        CensusStep::NeedsTranscripts => CensusOutcome::NeedsTranscripts,
    };
    sites.sort();
    sites.dedup();
    Ok((outcome, sites, run.requested))
}

/// The `returns` implementation census (ADR 0035): whether the demanded
/// export's own implementation completes without yielding a value on every
/// path at the `MayExecute` floor.
///
/// Decides the **empty** closure only. A `returns` proposal that enumerates
/// operations is refused exactly as a nonempty `creates` enumeration is: this
/// census derives no return shapes to compare it against, and admitting it
/// would certify the proposal's own word (objection 5 of ADR 0006).
///
/// No callee is dispositioned. A helper's return value reaches this export's
/// caller only through a return site of this export's own, and a return site
/// carrying an expression refuses whatever the expression is.
fn census_returns_domain(
    proof: &ScheduledProofDemand,
    export: &solid_reactive_ir::contract_semantics::ExportSemantics,
    implementation: &typefacts::ExportImplementationTranscript,
) -> Result<Vec<String>, TypeFactsCertificationError> {
    let refuse = |reason: String| TypeFactsCertificationError::UnsupportedDemand {
        demand: proof.id.clone(),
        reason,
    };
    if typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL < CENSUS_COMPLETION_FORM_PROTOCOL {
        return Err(refuse(format!(
            "returns-census premise required: the completion-form fact arrived at handshake \
             protocol {CENSUS_COMPLETION_FORM_PROTOCOL} and this build speaks {}, so an absent \
             form would be read as a plain callable",
            typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL
        )));
    }
    if typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL < CENSUS_CONTROL_FLOW_CLASSES_PROTOCOL {
        return Err(refuse(format!(
            "returns-census premise required: classified control-flow incompleteness arrived at \
             handshake protocol {CENSUS_CONTROL_FLOW_CLASSES_PROTOCOL} and this build speaks {}",
            typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL
        )));
    }
    let proposed = export
        .operation_claim(ClaimDomain::Returns)
        .ok_or_else(|| refuse("returns is not an operation domain of this export".into()))?;
    if !proposed.items().is_empty() {
        return Err(refuse(format!(
            "a returns closure candidate must enumerate no operation, but the proposal names {}",
            proposed.items().len()
        )));
    }
    census_returns_transcript(implementation).map_err(refuse)
}

/// The per-transcript half of [`census_returns_domain`]: the premises read off
/// the implementation transcript alone, so they can be pinned against a
/// synthesized transcript. `Err` is the refusal, by name and location.
fn census_returns_transcript(
    implementation: &typefacts::ExportImplementationTranscript,
) -> Result<Vec<String>, String> {
    let at = format!(
        "{}:{}..{}",
        implementation.location.path,
        implementation.location.start_byte,
        implementation.location.end_byte
    );
    // Premise 1: a plain callable. An `async` function hands its caller a
    // promise on every completion, a generator an iterator, whatever the body
    // does; an unclassified or unstated form is refused, never read as plain.
    match implementation.completion_form {
        Some(typefacts::ImplementationCompletionForm::Plain) => {}
        Some(typefacts::ImplementationCompletionForm::Async) => {
            return Err(format!(
                "returns census refuses an async implementation at {at}: every completion hands \
                 the caller a promise"
            ));
        }
        Some(typefacts::ImplementationCompletionForm::Generator) => {
            return Err(format!(
                "returns census refuses a generator implementation at {at}: every completion \
                 hands the caller an iterator"
            ));
        }
        Some(typefacts::ImplementationCompletionForm::AsyncGenerator) => {
            return Err(format!(
                "returns census refuses an async generator implementation at {at}: every \
                 completion hands the caller an async iterator"
            ));
        }
        Some(typefacts::ImplementationCompletionForm::Unclassified) => {
            return Err(format!(
                "returns census refuses an implementation at {at} whose completion form the \
                 producer did not classify"
            ));
        }
        None => {
            return Err(format!(
                "returns census refuses an implementation transcript at {at} that states no \
                 completion form"
            ));
        }
    }
    // Premise 5: the census is present. An absence is never zero returns.
    let Some(control_flow) = implementation.control_flow.as_ref() else {
        return Err(format!(
            "returns census refuses an implementation transcript with no control-flow census at \
             {at}"
        ));
    };
    // Premise 4: every incompleteness is the one admissible class. A loop, a
    // `switch` or a `try` whose lower bound alone is unmodelled still has every
    // return inside it on the wire, with reach `unknown`, and premise 3 reads
    // that row like any other; a construct the census cannot account for at
    // all has no such guarantee.
    if let Some(row) = control_flow
        .incompleteness
        .iter()
        .find(|row| row.class != typefacts::ControlFlowIncompletenessClass::ReachabilityLowerBound)
    {
        return Err(format!(
            "returns census refuses an implementation transcript whose control-flow census cannot \
             account for a construct ({}, {}) at {}:{}..{}, for {at}",
            row.marker,
            control_flow_incompleteness_class_name(row.class),
            row.location.path,
            row.location.start_byte,
            row.location.end_byte
        ));
    }
    if let Some(marker) = control_flow.unsupported.iter().find(|marker| {
        !control_flow
            .incompleteness
            .iter()
            .any(|row| row.marker == **marker)
    }) {
        return Err(format!(
            "returns census refuses an implementation transcript with an unclassified \
             control-flow marker {marker} at {at}"
        ));
    }
    // Premises 2 and 3: every return site at the floor is bare. An expression
    // body arrives as a value-carrying site over the body itself. A
    // value-carrying site the producer proved unreachable performs nothing and
    // is admitted with its own witness, exactly as an unreachable call is in
    // the `creates` census.
    let mut sites = Vec::new();
    for site in &control_flow.returns {
        let disposition = match (site.value.is_some(), site.reach) {
            (false, _) => "bare",
            (true, Reachability::Unreachable) => "value-unreachable",
            (true, reach) => {
                return Err(format!(
                    "returns census refuses a value-carrying completion at {}:{}..{}, reach {}",
                    site.location.path,
                    site.location.start_byte,
                    site.location.end_byte,
                    reachability_name(reach)
                ));
            }
        };
        sites.push(format!(
            "census-return:{}:{}:{}:{}:{}",
            site.location.path,
            site.location.start_byte,
            site.location.end_byte,
            reachability_name(site.reach),
            disposition
        ));
    }
    sites.push(format!(
        "census-returns-total:{}",
        control_flow.returns.len()
    ));
    sites.sort();
    sites.dedup();
    Ok(sites)
}

/// Every path the verified closure manifest calls **runtime** source of the
/// artifact under certification, normalized the way a producer path strips.
///
/// The local-recursion disposition rests on this set rather than on "the file
/// is under the certified root": a declaration file the archive also ships is
/// under the same root and is a description of code, not code, and recursing
/// into one would census a description.
fn certified_runtime_sources(plan: &CertificationPlan) -> std::collections::BTreeSet<String> {
    plan.verified_closure
        .manifest()
        .entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.role,
                crate::contract_interface::ClosureFileRole::Runtime
                    | crate::contract_interface::ClosureFileRole::LiteralDynamicChunk
            )
        })
        .map(|entry| entry.path.trim_start_matches("./").replace('\\', "/"))
        .collect()
}

/// One transcript's own calls and uncensused forms, and the recursion its
/// local-recursion dispositions require.
///
/// Refusals are `String` because the *demand* owns the error: a census premise
/// that fails is a refusal of the closure demand, not of the helper it was
/// walking.
fn census_transcript(
    run: &mut CensusRun<'_>,
    implementation: &typefacts::ExportImplementationTranscript,
    depth: usize,
) -> Result<CensusStep, String> {
    run.deepest = run.deepest.max(depth);
    census_transcript_is_censusable(implementation, depth)?;
    // The declaration node this transcript describes, bound from the
    // authenticated bytes, and the one premise the producer's own transcript
    // cannot state about it: that those bytes are this artifact's runtime
    // source at all.
    let frame = census_transcript_frame(run, implementation, depth)?;
    let previous_frame = run.frame.replace(frame);
    let step = census_transcript_calls(run, implementation, depth);
    run.frame = previous_frame;
    step
}

/// The per-call half of [`census_transcript`], run with `run.frame` set to the
/// transcript's own declaration node.
fn census_transcript_calls(
    run: &mut CensusRun<'_>,
    implementation: &typefacts::ExportImplementationTranscript,
    depth: usize,
) -> Result<CensusStep, String> {
    // Every uncensused invoking form the floor admits refuses, by kind and
    // location — except the one shape ADR 0034 dispositions: a read accessor
    // whose subject the producer roots at an unwritten parameter of this very
    // declaration, which is the caller's object and the caller's code. This is
    // otherwise the enumeration guarantee: the calls census records
    // `CallExpression` and `NewExpression` only, and a form it does not record
    // reaches a callable the walk below can say nothing about.
    for form in &implementation.uncensused_invoking_forms {
        if form.reach == Reachability::Unreachable {
            continue;
        }
        if census_form_is_parameter_rooted_accessor(form) {
            run.sites.push(census_form_site(
                form,
                CensusDisposition::ParameterRootedAccessor,
            ));
            continue;
        }
        return Err(format!(
            "creates census refuses an uncensused invoking form: {} ({}) at {}:{}..{}, reach {}",
            uncensused_invoking_form_kind_name(form.kind),
            form.node_kind,
            form.location.path,
            form.location.start_byte,
            form.location.end_byte,
            reachability_name(form.reach)
        ));
    }
    let mut step = CensusStep::Decided;
    for call in &implementation.calls {
        run.calls += 1;
        match census_call_disposition(run, call, depth)? {
            Some((disposition, site)) => {
                run.sites.push(site);
                if disposition == CensusDisposition::LocalRecursion
                    && census_local_recursion(run, call, depth)? == CensusStep::NeedsTranscripts
                {
                    step = CensusStep::NeedsTranscripts;
                }
            }
            None => step = CensusStep::NeedsTranscripts,
        }
    }
    Ok(step)
}

/// The transcript-level premise, applied to every transcript the census reads,
/// the demanded export's own included: a local declaration arrives through a
/// second acquisition that no scheduled demand checked, so its completeness is
/// this function's obligation.
///
/// # The incompleteness rule, and why it is sound
///
/// A negative call domain asserts a **zero upper bound**, so the premise this
/// census needs from a transcript is not that the producer modelled the body's
/// control flow but that the transcript *enumerates every callable the body can
/// reach* and never calls one of them `unreachable` when it can run. Those are
/// different requirements, and until protocol 15 the producer's wire form could
/// not tell them apart:
///
/// - It **dropped** every `calls` row lying in a region a `break` or `continue`
///   makes non-universal, to keep an over-optimistic `reachable` off the wire.
///   For the positive families that is the safe direction — an absent row lends
///   authority to nothing — but for a zero upper bound it is the exact failure
///   mode. The dropped call is a `CallExpression`, so no uncensused-form row
///   appeared either, and `switch (kind) { case "mount": render(App, el);
///   break; }` published nothing about `render` beyond a marker.
/// - And `unsupported` absorbed three different situations under four marker
///   strings: a construct whose *reachability* the census does not model, a jump
///   whose target it cannot resolve, and — because the first of those covers
///   every loop, `switch` and `try` — essentially every real function body.
///
/// So this function refused any nonempty `unsupported` at any depth, which
/// refused almost every export that had a body worth censusing.
///
/// **The rule now.** Rows are no longer dropped: a call in a jump region
/// arrives with `reach: unknown`, which the `MayExecute` floor admits and which
/// cannot make a negative claim over-optimistic. What remains is to separate the
/// two meanings of "incomplete", and the producer states it per construct:
///
/// - [`typefacts::ControlFlowIncompletenessClass::ReachabilityLowerBound`] — a
///   construct the producer walked in full. Every site inside it is recorded by
///   the shared body walk, and no site is called `unreachable` on its account;
///   what is missing is the *lower* bound, because control may not enter a loop
///   body, a `catch`, or a selected clause. **This census admits it**, and that
///   is sound precisely because the census only ever reads reach to ask "may
///   this run?": it disposes every row the floor admits and refuses on the
///   first it cannot, so an unmodelled guarantee costs it nothing while a
///   missing row would cost it everything.
/// - [`typefacts::ControlFlowIncompletenessClass::FlowUnaccounted`] — a
///   construct whose flow the producer cannot account for in either direction.
///   **This census refuses it**, by marker and location. It is also the
///   producer's classifier default, so a construct a future revision marks
///   unsupported without classifying refuses on arrival rather than passing as
///   the admissible arm.
///
/// A body containing an ordinary `for…of`, or a `switch` whose `break` the
/// enclosing construct owns, therefore carries lower-bound rows only and is
/// censusable; a labelled jump whose target no enclosing construct of the frame
/// owns is `flow-unaccounted` and still refuses.
///
/// Two further premises make the marker set trustworthy rather than merely
/// readable. `validate_control_flow_incompleteness` (in the client) refuses a
/// transcript whose two lists name different markers, so a classified row can
/// never be missing for a stated marker. And
/// [`CENSUS_CONTROL_FLOW_CLASSES_PROTOCOL`] is checked before any of this, in
/// [`census_creates_domain`], because an older producer both withholds rows and
/// decodes as an empty `incompleteness` list — an absence that would read
/// exactly like the admissible arm.
///
/// The `controlFlowUnsupported` open reason is admitted for the same reason the
/// positive families admit it, and only that one: it is the producer's seventh
/// completeness gate and says nothing this function has not just decided for
/// itself. Every other open reason still refuses.
fn census_transcript_is_censusable(
    implementation: &typefacts::ExportImplementationTranscript,
    depth: usize,
) -> Result<(), String> {
    let at = || {
        format!(
            "{}:{}..{}",
            implementation.location.path,
            implementation.location.start_byte,
            implementation.location.end_byte
        )
    };
    let Some(control_flow) = implementation.control_flow.as_ref() else {
        return Err(format!(
            "creates census refuses an implementation transcript with no control-flow census at \
             depth {depth} for {}",
            at()
        ));
    };
    // The classification first, so a transcript the producer left open for the
    // one relaxable reason is refused by the construct it could not account for
    // rather than by a bare "incomplete".
    if let Some(row) = control_flow
        .incompleteness
        .iter()
        .find(|row| row.class != typefacts::ControlFlowIncompletenessClass::ReachabilityLowerBound)
    {
        return Err(format!(
            "creates census refuses an implementation transcript whose control-flow census cannot \
             account for a construct ({}, {}) at {}:{}..{}, at depth {depth} for {}: only a \
             construct whose reachability lower bound alone is unmodelled leaves every call this \
             body can reach on the wire",
            row.marker,
            control_flow_incompleteness_class_name(row.class),
            row.location.path,
            row.location.start_byte,
            row.location.end_byte,
            at()
        ));
    }
    // Belt beside the client's own check: a stated marker with no classified
    // construct is an unclassified incompleteness, and reading it as the
    // admissible arm is what the class exists to prevent.
    if let Some(marker) = control_flow.unsupported.iter().find(|marker| {
        !control_flow
            .incompleteness
            .iter()
            .any(|row| row.marker == **marker)
    }) {
        return Err(format!(
            "creates census refuses an implementation transcript reporting {marker} unsupported \
             with no classified construct for it, at depth {depth} for {}",
            at()
        ));
    }
    if implementation.declaration.is_none() {
        return Err(format!(
            "creates census refuses an implementation transcript with no resolved declaration at \
             depth {depth} for {}",
            at()
        ));
    }
    if !implementation.complete
        && implementation
            .open_reasons
            .iter()
            .any(|reason| reason.as_ref() != "controlFlowUnsupported")
    {
        return Err(format!(
            "creates census refuses an incomplete implementation transcript at depth {depth} for \
             {} (reasons={:?})",
            at(),
            implementation.open_reasons
        ));
    }
    // An open transcript whose only reason is the relaxable one must actually
    // carry the construct that produced it. Without this, a producer that
    // appended `controlFlowUnsupported` for something else entirely would pass.
    if !implementation.complete && control_flow.incompleteness.is_empty() {
        return Err(format!(
            "creates census refuses a transcript open on controlFlowUnsupported that classifies \
             no construct, at depth {depth} for {}",
            at()
        ));
    }
    Ok(())
}

/// The wire spelling of an incompleteness class, for a refusal reason.
fn control_flow_incompleteness_class_name(
    class: typefacts::ControlFlowIncompletenessClass,
) -> &'static str {
    match class {
        typefacts::ControlFlowIncompletenessClass::ReachabilityLowerBound => {
            "reachability-lower-bound"
        }
        typefacts::ControlFlowIncompletenessClass::FlowUnaccounted => "flow-unaccounted",
    }
}

/// The declaration node a transcript describes, as a producer-side location,
/// bound from the authenticated runtime bytes.
///
/// The node, not the identifier: the producer resolves a named function to its
/// identifier and a `const helper = () => …` to the arrow itself.
///
/// # The jump premise moved to the producer, which is where it belongs
///
/// This function used to additionally refuse a node containing a `break` or
/// `continue` anywhere inside it, nested callables included, using the
/// verifier's own Oxc parse. That check existed for one reason: the producer's
/// control-flow census never enters a nested callable, so a jump *there* left no
/// `unsupported` marker on this transcript while still making the producer
/// withhold the rows of the region it targeted — an invisible withholding the
/// marker premise could not catch.
///
/// It is gone because both halves of its reason are gone. The producer no longer
/// withholds a row at all (it states `reach: unknown`), and its jump regions are
/// keyed by *flow owner*, so a jump inside a nested callable reduces the rows of
/// that callable and covers the whole of it whenever the jump's target cannot be
/// bound to an enclosing construct. The producer's own facts now answer the
/// question the syntactic scan was approximating, which is strictly better:
/// `AstFacts::jump_statements` could see the jump but never which rows it
/// touched, and it refused every `break` in the frame including the ones that
/// withheld nothing.
fn census_transcript_frame(
    run: &mut CensusRun<'_>,
    implementation: &typefacts::ExportImplementationTranscript,
    depth: usize,
) -> Result<typefacts::Location, String> {
    let declaration = implementation
        .declaration
        .as_ref()
        .expect("census_transcript_is_censusable required a declaration");
    let Some((_, relative)) = census_local_declaration_identity(run, declaration) else {
        return Err(format!(
            "creates census refuses a transcript at depth {depth} whose declaration {:?} at \
             {}:{}..{} is not in this artifact's own runtime source: the census walks \
             authenticated runtime bytes and nothing else",
            declaration.name,
            declaration.location.path,
            declaration.location.start_byte,
            declaration.location.end_byte
        ));
    };
    census_local_declaration_node(run, &relative, declaration)
}

/// The parsed facts of one runtime source of the artifact under certification,
/// parsed once from the authenticated bytes and cached on the run.
fn census_source<'r>(
    run: &'r mut CensusRun<'_>,
    relative: &str,
) -> Result<&'r CensusSourceFacts, String> {
    if !run.sources.contains_key(relative) {
        let parsed = run
            .certified
            .read(relative)
            .and_then(census_source_text)
            .and_then(|text| {
                solid_facts::ast::extract(relative, text)
                    .ok()
                    .map(|facts| CensusSourceFacts {
                        text: text.to_owned(),
                        facts,
                    })
            });
        run.sources.insert(relative.to_owned(), parsed);
    }
    match run.sources.get(relative) {
        Some(Some(source)) => Ok(source),
        _ => Err(format!(
            "creates census cannot parse the runtime source {relative} of this artifact"
        )),
    }
}

/// The one disposition this call carries, and its witness site.
///
/// `Ok(None)` is "a local declaration this session has not transcribed yet",
/// which the caller turns into [`CensusStep::NeedsTranscripts`]; the demand has
/// already been recorded in `run.requested`.
fn census_call_disposition(
    run: &mut CensusRun<'_>,
    call: &typefacts::ImplementationCall,
    depth: usize,
) -> Result<Option<(CensusDisposition, String)>, String> {
    let at = || {
        format!(
            "{}:{}..{}",
            call.location.path, call.location.start_byte, call.location.end_byte
        )
    };
    let floor = ReachabilityFloor::MayExecute;
    if !floor.admits(call.reach) {
        // `MayExecute` admits `Reachable` and `Unknown`, so this is exactly
        // `Unreachable`: a call the implementation provably never reaches
        // performs nothing.
        return Ok(Some((
            CensusDisposition::Unreachable,
            census_call_site(call, CensusDisposition::Unreachable),
        )));
    }
    // A construction runs its callee too — `new F(x)` evaluates `F`'s body — so
    // both kinds are dispositioned the same way. An *absent* kind deserializes
    // to `CallKind::Unknown` and refuses: absence is never read as "call".
    if !matches!(call.kind, CallKind::Call | CallKind::Construct) {
        return Err(format!(
            "creates census refuses a call of unknown kind at {}",
            at()
        ));
    }
    // Arguments do not matter for `creates`. A call is dispositioned by its
    // *callee*: what a spread carries, what a slot proves, and whether a
    // callable was handed over are questions the `callbacks` domain asks. This
    // is why a spread-carrying call whose slots trace nothing still certifies
    // here — and why a callee rooted at a parameter is excused while the
    // arguments beside it are simply not consulted.
    if call.callee_parameter.is_some() {
        return Ok(Some((
            CensusDisposition::ParameterRooted,
            census_call_site(call, CensusDisposition::ParameterRooted),
        )));
    }
    if let Some(declaration) = call
        .declaration
        .as_ref()
        .filter(|declaration| declaration.standard_library)
    {
        let disposition = census_standard_library_admits(run, call, declaration)
            .map_err(|reason| format!("{reason}, called at {}", at()))?;
        return Ok(Some((disposition, census_call_site(call, disposition))));
    }
    if let Some(terminator) = census_dialect_axiom_for_callee(
        call,
        solid_dialect::CallClaimDomain::Creates,
        floor,
        run.certified,
        run.evidence.roots,
    ) {
        // The tier's own site, not a generic one: it names the archive tuple,
        // the SRI prefix, the export and the domain, which is the whole premise.
        return Ok(Some((
            CensusDisposition::DialectAxiom,
            terminator.witness_site,
        )));
    }
    let Some(declaration) = call.declaration.as_ref() else {
        // The producer names nothing for an unbound identifier — `targetName`
        // is empty — so the call is named by its own authenticated bytes.
        return Err(format!(
            "creates census refuses an unresolved callee at {} ({})",
            at(),
            census_call_source_text(run, call).map_or_else(
                || format!("target={:?}", call.target_name),
                |text| format!("`{text}`")
            )
        ));
    };
    let Some((identity, relative)) = census_local_declaration_identity(run, declaration) else {
        return Err(format!(
            "creates census refuses a resolved callee that is neither a default-library member, a \
             dialect primitive under the negative authority, nor a declaration in this artifact's \
             own runtime source: {:?} declared at {}:{}..{}, called at {}",
            declaration.name,
            declaration.location.path,
            declaration.location.start_byte,
            declaration.location.end_byte,
            at()
        ));
    };
    // The producer resolves a named function to its *identifier*, and answers
    // a local-declaration demand only for the exact span of the declaration
    // node. The node is bound from the authenticated bytes the identifier sits
    // in; the answer is then bound back by the producer, which refuses one
    // whose resolved declaration does not lie inside the demanded node.
    let node = census_local_declaration_node(run, &relative, declaration)
        .map_err(|reason| format!("{reason}, called at {}", at()))?;
    // The producer resolved the *identifier* to this declaration, which is what
    // the binding named when the file was bound; the bytes that run are what
    // the binding holds when the call executes. The two agree only when
    // nothing writes the binding, so any write — an assignment, an update, a
    // destructuring target, a `for…in`/`for…of` head — or a second
    // declaration of the same name refuses, and so does a declaration with no
    // binding identifier of its own.
    census_local_binding_is_stable(run, &relative, &node, declaration)
        .map_err(|reason| format!("{reason}, called at {}", at()))?;
    // The target has been re-bound from the authenticated bytes before the
    // cycle is closed. That keeps a reassigned recursive name or another
    // declaration from borrowing this fixed-point disposition.
    if run.visited.contains(&identity) {
        return Ok(Some((
            CensusDisposition::LocalRecursionBackedge,
            census_call_site(call, CensusDisposition::LocalRecursionBackedge),
        )));
    }
    // The demanded export sits at depth 0, so a helper at depth `d` is `d`
    // distinct declarations away. A back-edge was closed above and consumes
    // no new hop; reaching the bound refuses rather than approximating.
    if depth + 1 > MAX_COMPOSITION_DEPTH {
        return Err(format!(
            "creates census exceeds {MAX_COMPOSITION_DEPTH} local-recursion hops at {}",
            at()
        ));
    }
    if run.evidence.local(&node).is_none() {
        if !run.requested.contains(&node) {
            run.requested.push(node);
        }
        return Ok(None);
    }
    Ok(Some((
        CensusDisposition::LocalRecursion,
        census_call_site(call, CensusDisposition::LocalRecursion),
    )))
}

/// The source text of a call the census refuses, read from the authenticated
/// snapshot bytes at the call's own location, or `None` when the location is
/// not inside the artifact under certification. Diagnostic only: it lets a
/// refusal name a callee the producer could resolve to nothing.
fn census_call_source_text(
    run: &CensusRun<'_>,
    call: &typefacts::ImplementationCall,
) -> Option<String> {
    let (_, relative) = census_certified_relative_path(run, &call.location.path)?;
    // The same text the producer counted offsets over: a leading byte-order
    // mark removed (see `CensusSourceFacts`).
    let source = census_source_text(run.certified.read(&relative)?)?;
    let start = usize::try_from(call.location.start_byte).ok()?;
    let end = usize::try_from(call.location.end_byte).ok()?;
    let text = source.get(start..end)?;
    const LIMIT: usize = 80;
    Some(if text.chars().count() > LIMIT {
        format!("{}…", text.chars().take(LIMIT).collect::<String>())
    } else {
        text.to_owned()
    })
}

/// The snapshot-relative path of `path` when it strips to one of the
/// artifact under certification's own roots, with that root's index.
fn census_certified_relative_path(run: &CensusRun<'_>, path: &str) -> Option<(usize, String)> {
    let normalized = path.replace('\\', "/");
    let root_paths = run
        .evidence
        .roots
        .iter()
        .map(|root| root.path.clone())
        .collect::<Vec<_>>();
    let (root_index, relative) = strip_materialized_source_root(&normalized, &root_paths)?;
    let root = run.evidence.roots.get(root_index)?;
    if root.dependency || root.snapshot.root() != run.certified.root() {
        return None;
    }
    Some((root_index, relative.to_owned()))
}

/// The exact span of the function-like declaration node a resolved local
/// declaration names, as a location the producer's `localDeclarationLocation`
/// demand accepts.
///
/// A named function resolves to its identifier while an anonymous
/// `const helper = () => …` resolves to the arrow itself, so two readings are
/// tried against the certifier's own parse of the authenticated runtime bytes:
/// a node whose span *is* the resolved span, else the one node whose **name**
/// span is the resolved span. Anything else — no node, two nodes, a source that
/// does not parse — refuses by name. This only chooses what to *ask*; the
/// producer binds its answer by refusing one whose resolved declaration does not
/// lie inside the demanded span (`declarationIdentityUnbound`), and the census
/// keys every lookup by the node it demanded.
fn census_local_declaration_node(
    run: &mut CensusRun<'_>,
    relative: &str,
    declaration: &typefacts::ResolvedDeclaration,
) -> Result<typefacts::Location, String> {
    let describe = || {
        format!(
            "{:?} declared at {}:{}..{}",
            declaration.name,
            declaration.location.path,
            declaration.location.start_byte,
            declaration.location.end_byte
        )
    };
    // A callee that is a *parameter* — of a nested callable, since a parameter
    // of the transcript's own declaration is dispositioned `parameter-rooted`
    // before any recursion — is caller-supplied code. It has no body this
    // census could walk, and saying so names the domain that owns the fact.
    if declaration.kind.as_ref() == "Parameter" {
        return Err(format!(
            "creates census refuses a call through {}: a parameter of a nested callable is \
             caller-supplied code, which the callbacks domain owns and this census cannot see",
            describe()
        ));
    }
    let source = census_source(run, relative).map_err(|_| {
        format!(
            "creates census cannot parse the runtime source declaring {}",
            describe()
        )
    })?;
    let nodes = source.function_nodes().collect::<Vec<_>>();
    let resolved = (
        u32::try_from(declaration.location.start_byte).map_err(|_| "declaration span overflows")?,
        u32::try_from(declaration.location.end_byte).map_err(|_| "declaration span overflows")?,
    );
    let mut matches = nodes
        .iter()
        .filter(|node| (node.span.start, node.span.end) == resolved)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        matches = nodes
            .iter()
            .filter(|node| {
                node.name
                    .is_some_and(|name| (name.start, name.end) == resolved)
                    && node.span.start <= resolved.0
                    && resolved.1 <= node.span.end
                    && !(node.body.start <= resolved.0 && resolved.1 <= node.body.end)
            })
            .collect();
    }
    if matches.is_empty() {
        // Third reading (2026-09-06): the resolved span is the binding
        // identifier of a variable declarator — `const helper = (el) => …`
        // resolved from another module of the same artifact names `helper`,
        // not the arrow — and the declarator's initializer is a function or
        // arrow literal. That literal is the node to ask about; whether the
        // binding is proven to still hold it when the call runs is
        // `census_local_binding_is_stable`'s question, asked next. An
        // initializer that is anything else — a call, a conditional, another
        // identifier — refuses by name: the census does not trace values.
        if let Some(binding) = census_plain_binding_named_at(&source.facts, resolved) {
            let initializer = binding
                .initializer
                .filter(|_| binding.initializer_function)
                .ok_or_else(|| {
                    format!(
                        "creates census refuses {}: the binding is initialized by an expression \
                         that is not a function or arrow literal, and the census does not trace \
                         values",
                        describe()
                    )
                })?;
            matches = nodes
                .iter()
                .filter(|node| node.span == initializer)
                .collect();
        }
    }
    match matches.as_slice() {
        [node] => Ok(typefacts::Location {
            path: declaration.location.path.clone(),
            start_byte: u64::from(node.demand_span.start),
            end_byte: u64::from(node.demand_span.end),
        }),
        [] => Err(format!(
            "creates census finds no function-like declaration node for {}",
            describe()
        )),
        _ => Err(format!(
            "creates census finds more than one function-like declaration node for {}",
            describe()
        )),
    }
}

/// The variable declarator binding exactly one plain identifier at `name`, if
/// any: no destructuring pattern, so the identifier is the whole binding.
fn census_plain_binding_named_at(
    facts: &solid_facts::ast::AstFacts,
    name: (u32, u32),
) -> Option<&solid_facts::ast::BindingFact> {
    facts.bindings.iter().find(|binding| {
        binding.array_slots.is_empty()
            && binding.object_slots.is_empty()
            && matches!(binding.names.as_slice(), [named] if (named.span.start, named.span.end) == name)
    })
}

/// The variable declarator whose initializer is exactly the function-like node
/// at `initializer`, binding one plain identifier.
fn census_plain_binding_initialized_by(
    facts: &solid_facts::ast::AstFacts,
    initializer: solid_facts::core::Span,
) -> Option<&solid_facts::ast::BindingFact> {
    facts.bindings.iter().find(|binding| {
        binding.initializer == Some(initializer)
            && binding.initializer_function
            && binding.array_slots.is_empty()
            && binding.object_slots.is_empty()
            && binding.names.len() == 1
    })
}

/// Recurses into the local declaration a `LocalRecursion` call named, adding
/// that declaration's own transcript identity to the witness.
fn census_local_recursion(
    run: &mut CensusRun<'_>,
    call: &typefacts::ImplementationCall,
    depth: usize,
) -> Result<CensusStep, String> {
    let declaration = call
        .declaration
        .as_ref()
        .expect("a local-recursion disposition resolved a declaration");
    let (identity, relative) = census_local_declaration_identity(run, declaration)
        .expect("a local-recursion disposition bound a declaration identity");
    let node = census_local_declaration_node(run, &relative, declaration)
        .expect("a local-recursion disposition bound the declaration node");
    let transcript = run
        .evidence
        .local(&node)
        .expect("a local-recursion disposition found the declaration's transcript")
        .clone();
    // The transcript's own bytes, so the receipt names the evidence the census
    // read rather than only the call that reached it. It travels as a witness
    // *site*: the evidence-root envelope is deliberately unchanged, because a
    // new envelope field would move every receipt's witness root.
    let transcript_bytes = typefacts::encode(&transcript).map_err(|error| {
        format!("creates census could not canonicalize a local transcript: {error}")
    })?;
    run.sites.push(format!(
        "census-local-declaration:{}:{}:{}:{}:sha256:{:x}",
        identity.path,
        identity.start,
        identity.end,
        identity.symbol,
        Sha256::digest(&transcript_bytes)
    ));
    run.visited.push(identity);
    let step = census_transcript(run, &transcript, depth + 1);
    run.visited.pop();
    step
}

/// Standard-library members that transfer control to a callable **reference**
/// they are handed, or evaluate source text as code. A call to one of these is
/// a call to something the census never dispositioned, whatever the arguments
/// prove, so the member itself refuses, by qualified name.
///
/// This is a **denylist beside a reviewed allowlist**, and the split is
/// deliberate. The allowlist is the producer's own reviewed invoker table
/// (`invoking_positions.go`, read back through
/// [`typefacts::DefaultLibraryInvoker::from_wire`]): it answers *which slot* a
/// reviewed member invokes, so the census can demand a proof for exactly that
/// slot. It cannot be the whole answer, because the members below invoke
/// something that is not an argument slot at all — `fn.call(…)` runs its
/// *receiver*, `Reflect.apply(target, …)` runs whatever value sits in slot 0
/// however it got there, and `eval`/`Function` run text — and a per-slot table
/// has no row shape for "the receiver" or "the text". A full allowlist of every
/// default-library member that transfers control to *nothing* would be a
/// review of the whole of `lib.*.d.ts`, which nobody has done, and pretending
/// otherwise would certify on an unreviewed row. So: reviewed invoker rows
/// prove slots, these names refuse outright, and every other standard-library
/// member is admitted only when no argument slot is proven or suspected to
/// carry a callable (see [`census_standard_library_admits`]).
const CENSUS_CONTROL_TRANSFER_MEMBERS: &[&str] = &[
    "eval",
    "Function",
    "FunctionConstructor",
    "Reflect.apply",
    "Reflect.construct",
];

/// Default-library members whose only reach into user code is a protocol read
/// on their `this` value, each with the ECMAScript step that names the reach
/// (ADR 0034). A `.call`/`.apply` whose receiver resolves — by default-library
/// symbol identity, never by spelling — to one of these is dispositioned as an
/// invocation of that member with slot 0 as its subject; the subject then has
/// to be parameter-rooted, or the site refuses as any by-reference transfer
/// does. Grown by review, one row at a time; never derived from `lib.*.d.ts`.
///
/// | member | reach on `this` |
/// | --- | --- |
/// | `Object.prototype.toString` (qualified `Object.toString`) | ES2024 §20.1.3.6 step 15: `Get(O, @@toStringTag)` — one getter or trap on the subject, nothing else |
const CENSUS_THIS_PROTOCOL_MEMBERS: &[&str] = &["Object.toString"];

/// Owners every one of whose members transfers control by reference:
/// `Function.prototype.{call,apply,bind}` under each of the interface names the
/// default library declares them on.
const CENSUS_CONTROL_TRANSFER_OWNERS: &[&str] = &[
    "Function.",
    "FunctionConstructor.",
    "CallableFunction.",
    "NewableFunction.",
];

/// Whether a standard-library callee may take the `standard-library`
/// disposition: it transfers control to no callable the census cannot see.
///
/// The disposition's premise — a default-library member registers no version-1
/// resource into a Solid runtime — is about the member's *own* body. It says
/// nothing about user code the member runs on the census's behalf, and three
/// ways a member can do that are refused here:
///
/// 1. **By reference or by text.** The member is one of
///    [`CENSUS_CONTROL_TRANSFER_MEMBERS`] or belongs to one of
///    [`CENSUS_CONTROL_TRANSFER_OWNERS`]: `render.call(null, App, el)`,
///    `Reflect.apply(render, …)`, `eval(source)`, `new Function(source)`.
/// 2. **Through a reviewed invoking slot.** The producer named the member in
///    its reviewed invoker table (`default_library_invoker`), so every slot
///    that table says the member invokes — `queue.forEach(render)`,
///    `setTimeout(render, 0)`, `promise.then(render)`, `new Promise(render)` —
///    must be proven: either rooted at a parameter of this implementation
///    (`argument_parameters[slot]`, the caller's code under § 3.2) or a
///    callable literal whose every location lies inside the transcript's own
///    frame, where its calls are rows of this very walk. A slot the producer
///    traced to nothing — an imported `render`, a module-local `function
///    helper` (the tracer follows a `const` binding only), a member read — is
///    not proven and refuses. An unrecognized invoker string refuses too: it
///    names a row this side never reviewed.
/// 3. **Through any slot the producer saw a callable in.** `argument_callables`
///    lists the callables a slot provably carries, and a standard-library
///    member handed one — `Array.from(items, mapFn)`, `JSON.parse(text,
///    reviver)`, `text.replace(pattern, fn)` — invokes it or stores it, and
///    the census can prove neither. The same two proofs admit the slot; nothing
///    else does.
///
/// What this does **not** close, and says so: a standard-library member may
/// reach user code through a *protocol method* on a value it is handed or
/// receives — `JSON.stringify(o)` calls `o.toJSON`, `Array.from(iterable)`
/// drives `iterable[Symbol.iterator]`, `arr.sort()` calls `toString` on its
/// elements, `Promise.resolve(thenable)` calls `then`. The producer records the
/// operator and template spellings of that reach as `coercion` and
/// `iteration-protocol` forms, but not the call spellings, and this side has
/// no fact about a non-callable argument's shape to refuse on. That is a
/// producer-side gap recorded in `docs/precision-backlog.md`, not a premise
/// this function claims.
fn census_standard_library_admits(
    run: &CensusRun<'_>,
    call: &typefacts::ImplementationCall,
    declaration: &typefacts::ResolvedDeclaration,
) -> Result<CensusDisposition, String> {
    let qualified: &str = if declaration.qualified_name.is_empty() {
        &declaration.name
    } else {
        &declaration.qualified_name
    };
    if qualified.is_empty() {
        return Err(
            "creates census refuses a standard-library callee with no name to review".into(),
        );
    }
    let last_segment = qualified.rsplit('.').next().unwrap_or(qualified);
    // ADR 0034: `.call`/`.apply` of a reviewed this-protocol member on a
    // parameter-rooted `this` is that member's invocation on the caller's own
    // object, decided before the by-reference rule refuses the owner.
    if matches!(last_segment, "call" | "apply")
        && CENSUS_CONTROL_TRANSFER_OWNERS
            .iter()
            .any(|owner| qualified.starts_with(owner))
        && let Some(receiver) = call.call_receiver.as_ref()
        && receiver.standard_library
    {
        let receiver_name: &str = if receiver.qualified_name.is_empty() {
            &receiver.name
        } else {
            &receiver.qualified_name
        };
        if CENSUS_THIS_PROTOCOL_MEMBERS.contains(&receiver_name) {
            if call.this_parameter.is_some() {
                return Ok(CensusDisposition::ParameterRootedAccessor);
            }
            return Err(format!(
                "creates census refuses `{qualified}` on the this-protocol member `{receiver_name}`: \
                 its `this` argument is not rooted at an unwritten parameter of this declaration, \
                 so the protocol read it performs may run code this export owns"
            ));
        }
    }
    if CENSUS_CONTROL_TRANSFER_MEMBERS.contains(&qualified)
        || CENSUS_CONTROL_TRANSFER_OWNERS
            .iter()
            .any(|owner| qualified.starts_with(owner))
        || last_segment == "eval"
    {
        return Err(format!(
            "creates census refuses the standard-library member `{qualified}`: it transfers \
             control to a callable by reference (or evaluates text as code), which the census \
             cannot disposition"
        ));
    }
    let invoker = if call.default_library_invoker.is_empty() {
        None
    } else {
        Some(
            typefacts::DefaultLibraryInvoker::from_wire(&call.default_library_invoker).ok_or_else(
                || {
                    format!(
                        "creates census refuses the standard-library member `{qualified}`: the \
                     producer names it a default-library invoker {:?}, which is not a member \
                     of the reviewed table",
                        call.default_library_invoker
                    )
                },
            )?,
        )
    };
    let frame = run
        .frame
        .as_ref()
        .ok_or("creates census has no transcript frame to admit a callable literal against")?;
    let slots = call
        .argument_parameters
        .len()
        .max(
            call.argument_callables
                .iter()
                .map(|carried| carried.argument + 1)
                .max()
                .unwrap_or(0),
        )
        .max(
            call.invoked_arguments
                .iter()
                .map(|slot| slot + 1)
                .max()
                .unwrap_or(0),
        );
    for slot in 0..slots {
        let invoked = invoker.is_some_and(|invoker| invoker.invokes(slot))
            || call.invoked_arguments.contains(&slot);
        let carried = call
            .argument_callables
            .iter()
            .find(|carried| carried.argument == slot);
        if !invoked && carried.is_none() {
            continue;
        }
        if call
            .argument_parameters
            .get(slot)
            .is_some_and(Option::is_some)
        {
            // Rooted at a parameter of this implementation: the caller's code,
            // under the caller's own contract (census plan § 3.2).
            continue;
        }
        if carried.is_some_and(|carried| {
            !carried.locations.is_empty()
                && carried.locations.iter().all(|location| {
                    location.path == frame.path
                        && frame.start_byte <= location.start_byte
                        && location.end_byte <= frame.end_byte
                })
        }) {
            // A callable literal inside the frame under the walk: its calls are
            // rows of this transcript, dispositioned like every other.
            continue;
        }
        return Err(format!(
            "creates census refuses the standard-library member `{qualified}`: argument {slot} \
             is {} and is neither rooted at a parameter of this implementation nor a callable \
             literal inside the transcript being censused, so control reaches a callable the \
             census cannot see",
            if invoked {
                "a slot the reviewed invoker table says the member invokes"
            } else {
                "a slot the producer saw a callable in"
            }
        ));
    }
    Ok(CensusDisposition::StandardLibrary)
}

/// Whether the binding a local-recursion callee resolved through holds, at
/// every point of the implementation, the declaration the producer resolved.
///
/// The producer answers "which declaration does this identifier's symbol
/// have", which is a fact about the file as bound. The census then reads that
/// declaration's body as *the code the call runs*, which is a fact about the
/// binding's value when the call executes. `function helper() {} … helper =
/// (el) => render(App, el); … helper()` separates the two: the symbol still
/// resolves to the declaration, and the declaration is not what runs. Four
/// things therefore refuse, each by name and location, all read from the
/// verifier's own Oxc facts over the authenticated bytes:
///
/// * a callable expression with **no binding identifier** that is not the
///   whole initializer of a variable declarator binding one plain identifier —
///   an argument, an element, an operand. What runs is whatever holds it, and
///   this census does not trace values. `const helper = () => …` (and the
///   `let`/`var` spellings) is the one indirection taken, since 2026-09-06:
///   the declarator's identifier is the binding, and it is held to the same
///   two checks below;
/// * any **write** to the binding: an assignment or update expression whose
///   target contains a reference to it, or a `for…in`/`for…of` head that
///   assigns it;
/// * a **second declaration** of the same name in the file — another function
///   declaration, a variable declarator, or a class — which the binder merges
///   into one symbol whose declaration list this census does not see.
fn census_local_binding_is_stable(
    run: &mut CensusRun<'_>,
    relative: &str,
    node: &typefacts::Location,
    declaration: &typefacts::ResolvedDeclaration,
) -> Result<(), String> {
    let describe = || {
        format!(
            "{:?} declared at {}:{}..{}",
            declaration.name, node.path, node.start_byte, node.end_byte
        )
    };
    let source = census_source(run, relative)?;
    let function = source
        .function_nodes()
        .find(|candidate| {
            (
                u64::from(candidate.demand_span.start),
                u64::from(candidate.demand_span.end),
            ) == (node.start_byte, node.end_byte)
        })
        .ok_or_else(|| {
            format!(
                "creates census lost the declaration node it bound for {}",
                describe()
            )
        })?;
    let facts = &source.facts;
    // An arrow or function expression has no name of its own. When it is the
    // whole initializer of a variable declarator binding one plain identifier
    // (2026-09-06), that identifier is the binding the call runs through, and
    // the same two questions decide it as decide a named declaration: is it
    // written anywhere in the file, and is it declared again. A `let` or `var`
    // is admitted on the same terms as a `const` — an unwritten, once-declared
    // module binding holds its initializer at every call, whatever keyword
    // declared it, and no other module can write it. A callable expression
    // that is not such an initializer — an argument, an element, an operand —
    // still refuses: it runs through whatever holds it, which the census does
    // not trace.
    let name_span = match function.name {
        Some(span) => span,
        None => match census_plain_binding_initialized_by(facts, function.span) {
            Some(binding) => binding.names[0].span,
            None => {
                return Err(format!(
                    "creates census refuses a local declaration with no binding identifier of \
                     its own ({}): a callable expression runs through whatever variable holds \
                     it, and the census does not trace variables",
                    describe()
                ));
            }
        },
    };
    let name = source.text_at(name_span).ok_or_else(|| {
        format!(
            "creates census cannot read the binding name of {} from the authenticated bytes",
            describe()
        )
    })?;
    for (reference, resolved) in &facts.reference_declarations {
        if *resolved != name_span {
            continue;
        }
        let start = u64::from(reference.start);
        let end = u64::from(reference.end);
        let write = facts
            .assignments
            .iter()
            .map(|assignment| assignment.target)
            .chain(facts.iteration_targets.iter().copied())
            .find(|target| census_span_contains(*target, start, end));
        if let Some(write) = write {
            return Err(format!(
                "creates census refuses the local declaration `{name}` ({}): its binding is \
                 written at {}:{}..{}, so the declaration the producer resolved is not proven \
                 to be the code the call runs",
                describe(),
                node.path,
                write.start,
                write.end
            ));
        }
    }
    let redeclared = facts
        .function_declarations
        .iter()
        .map(|function| function.name.span)
        .chain(
            facts
                .bindings
                .iter()
                .flat_map(|binding| binding.names.iter().map(|named| named.span)),
        )
        .chain(
            facts
                .classes
                .iter()
                .filter_map(|class| class.name.as_ref().map(|named| named.span)),
        )
        .find(|span| *span != name_span && source.text_at(*span) == Some(name));
    if let Some(other) = redeclared {
        return Err(format!(
            "creates census refuses the local declaration `{name}` ({}): the same name is \
             declared again at {}:{}..{}, and the binder merges the two into one symbol whose \
             running declaration the census cannot choose",
            describe(),
            node.path,
            other.start,
            other.end
        ));
    }
    Ok(())
}

/// The declaration identity of a callee that resolves inside the certified
/// artifact's own runtime source set, or `None` when it does not.
///
/// Three premises, all required. The declaration's source file must strip to an
/// authenticated snapshot root that is **not** a dependency and **is** this
/// plan's own snapshot; the remainder must be a path the verified closure
/// manifest calls runtime source, so a declaration file the archive ships is
/// excluded; and the remainder must be a member of the snapshot, re-asked here
/// rather than inherited from the source census.
fn census_local_declaration_identity(
    run: &CensusRun<'_>,
    declaration: &typefacts::ResolvedDeclaration,
) -> Option<(CensusDeclarationIdentity, String)> {
    if declaration.source_file.is_empty() {
        return None;
    }
    let (_, relative) = census_certified_relative_path(run, &declaration.source_file)?;
    if !run.runtime_sources.contains(&relative) {
        return None;
    }
    run.certified.read(&relative)?;
    Some((CensusDeclarationIdentity::of(declaration)?, relative))
}

/// One call's witness line. Universal, not existential: every censused call
/// contributes one, so a receipt records the whole census rather than the
/// subset that happened to be interesting.
/// Whether an uncensused form is the one shape ADR 0034 dispositions instead of
/// refusing: a read accessor on a property or element access whose subject the
/// producer rooted at a parameter of the transcript's own declaration. Every
/// other kind — a setter, an iteration, a coercion, an `instanceof`, a spread,
/// a destructuring pattern — refuses whatever the producer stated.
fn census_form_is_parameter_rooted_accessor(form: &typefacts::UncensusedInvokingForm) -> bool {
    use typefacts::UncensusedInvokingFormKind as Kind;
    matches!(
        form.kind,
        Kind::GetAccessor | Kind::PropertyAccessUnknownAccessor
    ) && matches!(
        form.node_kind.as_ref(),
        "PropertyAccessExpression" | "ElementAccessExpression"
    ) && form.subject_parameter.is_some()
}

fn census_form_site(
    form: &typefacts::UncensusedInvokingForm,
    disposition: CensusDisposition,
) -> String {
    format!(
        "census-form:{}:{}:{}:{}:{}:{}",
        form.location.path,
        form.location.start_byte,
        form.location.end_byte,
        uncensused_invoking_form_kind_name(form.kind),
        reachability_name(form.reach),
        disposition.wire_name()
    )
}

fn census_call_site(
    call: &typefacts::ImplementationCall,
    disposition: CensusDisposition,
) -> String {
    format!(
        "census-call:{}:{}:{}:{}:{}:{}",
        call.location.path,
        call.location.start_byte,
        call.location.end_byte,
        call_kind_name(call.kind),
        reachability_name(call.reach),
        disposition.wire_name()
    )
}

const fn call_kind_name(kind: CallKind) -> &'static str {
    match kind {
        CallKind::Call => "call",
        CallKind::Construct => "construct",
        CallKind::Unknown => "unknown",
    }
}

const fn reachability_name(reach: Reachability) -> &'static str {
    match reach {
        Reachability::Reachable => "reachable",
        Reachability::Unknown => "unknown",
        Reachability::Unreachable => "unreachable",
    }
}

const fn uncensused_invoking_form_kind_name(
    kind: typefacts::UncensusedInvokingFormKind,
) -> &'static str {
    use typefacts::UncensusedInvokingFormKind as Kind;

    match kind {
        Kind::TaggedTemplate => "tagged-template",
        Kind::GetAccessor => "get-accessor",
        Kind::SetAccessor => "set-accessor",
        Kind::PropertyAccessUnknownAccessor => "property-access-unknown-accessor",
        Kind::Decorator => "decorator",
        Kind::IterationProtocol => "iteration-protocol",
        Kind::UsingDispose => "using-dispose",
        Kind::InstanceOf => "instanceof",
        Kind::AwaitThen => "await-then",
        Kind::Coercion => "coercion",
        Kind::JsxElement => "jsx-element",
        Kind::UnclassifiedInvokingForm => "unclassified-invoking-form",
    }
}

/// Requires the proposal's root alternative enumeration to *be* the producer's:
/// the same number of alternatives, and at every index the kind the census
/// observed there.
///
/// This is the closure premise for a root choice-alternatives domain, and it is
/// the part that makes the claim the census's rather than the proposal's.
/// `require_closed_value` already proves the producer observed the value and
/// every alternative exhaustively — no open reason anywhere, every finite
/// partition complete — so an enumeration of the same size as that observation
/// is the observation's *cardinality*. A proposal naming a different number of
/// alternatives is not a weaker claim about the same value; it is a claim the
/// producer's exhaustive observation contradicts.
///
/// Cardinality is not identity, though, and the difference is not academic. The
/// sibling per-index `recursive-value-shape` demands this shape inventories
/// carry a `DemandedCallability` that is `Unknown` for every structural kind
/// (`Object`, `Tuple`, `Promise`, `Reactive`, `Store`, `Action`, `Cleanup`, …),
/// so on their own they would let a proposal name two alternatives of the wrong
/// kinds and still "equal" a census of two. Each index is therefore compared
/// here as well — against the only classification this census carries, which is
/// callability — and any proposed kind the census cannot classify refuses.
fn require_export_value_enumeration_matches_census(
    plan: &CertificationPlan,
    proof: &ScheduledProofDemand,
    transcript: &ExportValueTranscript,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<usize, TypeFactsCertificationError> {
    let (artifact_case, export_name) = proof_artifact_export(&proof.subject);
    let export = plan
        .candidates
        .proposal()
        .artifact_case(artifact_case)
        .and_then(|case| case.exports.get(export_name))
        .ok_or_else(|| TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "closure subject names an export absent from the selected candidate".into(),
        })?;
    let ValueShape::Choice(alternatives) = &export.shape else {
        return Err(TypeFactsCertificationError::SubjectMismatch {
            demand: proof.id.clone(),
            reason: "a root choice-alternatives closure requires a proposed choice shape".into(),
        });
    };
    let proposed = alternatives.items().len();
    let observed = transcript.value.alternatives.len();
    if proposed != observed {
        return Err(open(&format!(
            "proposed {proposed} root choice alternatives, but the producer exhaustively \
             observed {observed}"
        )));
    }
    if proposed == 0 {
        return Err(open(
            "a root choice-alternatives closure over no alternative enumerates nothing",
        ));
    }
    for (index, alternative) in alternatives.items().iter().enumerate() {
        require_export_alternative_kind_matches_census(
            proof,
            transcript,
            index,
            alternative,
            open,
        )?;
    }
    Ok(proposed)
}

/// Requires the proposal's alternative at one index to be the kind the census
/// observed at that index.
///
/// The census classifies an alternative by **callability** and nothing else:
/// [`typefacts::ValueAlternative`] carries an index, discriminants, and open
/// reasons, and the per-alternative root [`typefacts::CallablePathFact`] adds
/// presence and callability. So exactly two proposed kinds can be compared
/// against it — `Callable`, which must be observed callable, and `Plain`, which
/// must be observed non-callable — and every other kind refuses for want of a
/// producer *kind* fact at that index. That refusal is the honest one:
/// admitting, say, a proposed `Object` alternative would certify the proposal's
/// own word about a structure the census never described. The remaining gap is
/// recorded in `docs/precision-backlog.md`.
///
/// **`Component` refuses too**, and it is the one kind where that is not
/// obvious. `recursive_value_callability` groups it with `Callable`, which is
/// correct for the question *that* function asks — a component is callable —
/// but this function decides whether a **closed** claim may be certified, and
/// the artifact a receipt binds then names the alternative `component`. The
/// only evidence in hand is bare callability, which every function has, so
/// `component` would be a strictly stronger name than the census can support:
/// nothing here observes a props parameter, a JSX or element result, or a
/// render-time owner. A publisher who wants that closure can propose
/// `Callable`, which is the claim the evidence actually makes.
///
/// The empty path names the alternative's own root, of which the census has one
/// per alternative.
///
/// # What this comparison contributes, and what the sibling demand already does
///
/// Comparing proposal index *i* against census alternative *i* presumes the two
/// enumerations are ordered alike, and neither side chose to be: the proposal's
/// order is `normalize_knowledge`'s canonical sort and the producer's is its
/// own. The correspondence is a *checked* property rather than an assumption,
/// and — this is the part an earlier version of this comment got wrong — it is
/// checked independently for **both** callability kinds:
/// `inventory_value_shape` emits one `recursive-value-shape` demand per
/// alternative carrying `recursive_value_callability(item)`, which is
/// `Callable` for a `Callable` alternative and `NonCallable` for a `Plain` one,
/// and `require_export_recursive_subject` verifies that against the census fact
/// whose `alternative` field *is* `i` (see `translate_value_path`) through
/// `require_path_callability`, which refuses `NonCallable` against an observed
/// callable exactly as it refuses the converse. A permuted enumeration
/// therefore refuses through those demands whichever of the two kinds sits at
/// the permuted index, and every scheduled demand has to be discharged for the
/// closure to certify.
///
/// What *this* comparison contributes on its own is the refusal of every kind
/// the census cannot classify: each structural shape carries
/// `DemandedCallability::Unknown`, which the sibling demand accepts without
/// asserting anything, so a proposal naming two alternatives of the wrong
/// structural kinds would otherwise have "equalled" a census of two. It also
/// requires the index to appear in the producer's own enumeration and that
/// alternative's root observation to be present and locally closed.
///
/// A multiset comparison — matching the *counts* of callable and non-callable
/// alternatives instead of their positions — was considered and rejected on
/// that ground. It keeps the unclassifiable-kind refusal, so it buys no
/// reachable certification: every permutation it would newly accept is already
/// refused by the sibling per-index demand. What it would cost is coherence —
/// two halves of one premise disagreeing about what an index means, with this
/// half no longer stating the reading the other half enforces.
fn require_export_alternative_kind_matches_census(
    proof: &ScheduledProofDemand,
    transcript: &ExportValueTranscript,
    index: usize,
    alternative: &ValueShape,
    open: &impl Fn(&str) -> TypeFactsCertificationError,
) -> Result<(), TypeFactsCertificationError> {
    let expected = match alternative {
        ValueShape::Callable => Callability::Callable,
        ValueShape::Plain => Callability::NonCallable,
        other => {
            return Err(TypeFactsCertificationError::UnsupportedDemand {
                demand: proof.id.clone(),
                reason: format!(
                    "alternative-kind premise required: this census classifies an alternative \
                     only by callability, so it cannot decide that root choice alternative \
                     {index} is a {} value",
                    value_shape_kind_name(other)
                ),
            });
        }
    };
    if !transcript
        .value
        .alternatives
        .iter()
        .any(|observed| observed.index == index)
    {
        return Err(open(&format!(
            "root choice alternative {index} is absent from the producer's own enumeration"
        )));
    }
    let fact = transcript
        .callable_paths
        .iter()
        .find(|fact| fact.alternative == index && fact.path.is_empty())
        .ok_or_else(|| {
            open(&format!(
                "root choice alternative {index} has no root observation in the exact producer \
                 census"
            ))
        })?;
    if !callable_path_is_present_and_locally_closed(fact) {
        return Err(open(&format!(
            "root choice alternative {index} is locally open in the producer census"
        )));
    }
    if fact.callability != expected {
        return Err(open(&format!(
            "root choice alternative {index} is proposed as a {} value, which requires \
             {expected:?}, but the producer census observed {:?}",
            value_shape_kind_name(alternative),
            fact.callability
        )));
    }
    Ok(())
}

/// The proposal-side name of one value shape's kind, for a refusal that says
/// which kind had no census classification.
const fn value_shape_kind_name(shape: &ValueShape) -> &'static str {
    match shape {
        ValueShape::Unknown => "unknown",
        ValueShape::Plain => "plain",
        ValueShape::Parameter { .. } => "parameter",
        ValueShape::Tuple(_) => "tuple",
        ValueShape::Array { .. } => "array",
        ValueShape::Object(_) => "object",
        ValueShape::Choice(_) => "choice",
        ValueShape::Callable => "callable",
        ValueShape::Promise(_) => "promise",
        ValueShape::AsyncIterable(_) => "async-iterable",
        ValueShape::Reactive { .. } => "reactive",
        ValueShape::Store { .. } => "store",
        ValueShape::Action { .. } => "action",
        ValueShape::Component => "component",
        ValueShape::Cleanup { .. } => "cleanup",
        ValueShape::RefApplication => "ref-application",
        ValueShape::ServerFunctionReference { .. } => "server-function-reference",
    }
}

const fn value_root_name(root: &ValueRoot) -> &'static str {
    match root {
        ValueRoot::Export => "exported",
        ValueRoot::OperationInput { .. } => "operation-input",
        ValueRoot::OperationOutput { .. } => "operation-output",
    }
}

const fn value_claim_domain_name(domain: ValueClaimDomain) -> &'static str {
    match domain {
        ValueClaimDomain::Shape => "shape",
        ValueClaimDomain::TupleItems => "tuple-items",
        ValueClaimDomain::ObjectProperties => "object-properties",
        ValueClaimDomain::ChoiceAlternatives => "choice-alternatives",
        ValueClaimDomain::ArrayMinimumLength => "array-minimum-length",
        ValueClaimDomain::ArrayMaximumLength => "array-maximum-length",
        ValueClaimDomain::Capabilities => "capabilities",
    }
}

pub(super) const fn call_claim_domain_name(domain: ClaimDomain) -> &'static str {
    match domain {
        ClaimDomain::Reads => "reads",
        ClaimDomain::Writes => "writes",
        ClaimDomain::Creates => "creates",
        ClaimDomain::Invalidates => "invalidates",
        ClaimDomain::Throws => "throws",
        ClaimDomain::Returns => "returns",
        ClaimDomain::Cleanups => "cleanups",
        ClaimDomain::Disposals => "disposals",
        ClaimDomain::Callbacks => "callbacks",
    }
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
    // The invocation census carries the control-flow branch census, so a guard
    // partition is the one closure it can decide.
    require_census_decides_closure(proof, &subject.path, ClosureCensus::Invocation)?;
    // Unreachable by construction: the gate above admits exactly
    // `Domain(GuardPartition)` for this census and refuses every other claim
    // path by name, so a per-path domain table here would be dead code that
    // reads as if other paths were supported. When the implementation-census
    // premise lands, the gate is what widens, and this is what follows it.
    debug_assert!(
        matches!(
            &subject.path,
            SemanticClaimPath::Domain(ClaimPath::GuardPartition)
        ),
        "require_census_decides_closure admits only a guard partition here"
    );
    let domains: &[InvocationDomain] = &[
        InvocationDomain::Signature,
        InvocationDomain::Parameters,
        InvocationDomain::Result,
        InvocationDomain::ControlFlow,
    ];
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
    // A guard-partition closure's premise is a *complete* finite partition, not
    // merely a branch census with nothing unsupported in it. The
    // `GuardPartition` family already requires one for a positive guard case;
    // closing the domain cannot ask for less.
    if matches!(
        &subject.path,
        SemanticClaimPath::Domain(ClaimPath::GuardPartition)
    ) && !signature
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
        .any(|partition| partition.complete)
    {
        return Err(open(
            "guard-partition closure has no complete finite partition to enumerate",
        ));
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

/// One proposable closure candidate's census refusal: the demand, the reason
/// the withheld record carries, and the refusal as it would have rendered on
/// its own.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CensusRefusal {
    pub demand: String,
    pub reason: String,
    pub rendered: String,
}

fn render_census_refusals(refusals: &[CensusRefusal]) -> String {
    let mut rendered = format!(
        "Type Facts census refused {} proposable closure candidate(s): ",
        refusals.len()
    );
    rendered.push_str(
        &refusals
            .iter()
            .map(|refusal| refusal.rendered.as_str())
            .collect::<Vec<_>>()
            .join("; "),
    );
    rendered
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
    /// Every proposable closure candidate whose implementation census could
    /// not decide it in this acquisition, collected rather than stopping at
    /// the first (ADR 0036): a census refusal withholds the candidate by name
    /// and the transaction re-plans, so one refusal per acquisition made a
    /// graph with dozens of such candidates take dozens of passes.
    #[error("{}", render_census_refusals(refusals))]
    CensusRefused { refusals: Vec<CensusRefusal> },
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

/// Holds one export-value transcript to the implementation location its
/// schedule asked about — but only when the producer got far enough to answer.
///
/// An export observation that refused early (the demanded span was not the
/// exact identifier, the alias did not resolve, the target had no declaration)
/// returns before it ever reads `implementation_location`, so its absent
/// implementation *is* that refusal, not a producer answering about a
/// different span. Comparing anyway turned every such transcript into
/// "field implementation_location expected <path>, actual None": a location
/// identity failure naming a location the schedule chose correctly, for a
/// demand whose real defect is one of the transcript's own open reasons.
/// `@tanstack/solid-query-persist-client@5.102.5` reported exactly that, and
/// the reason it hid is `PERSISTER_KEY_PREFIX` coming back
/// `declarationUnavailable`.
///
/// Deferring accepts nothing. Every scheduled export value carries at least
/// one proof demand by construction (`new_export_values` groups demands and
/// never creates an empty group), and `verify_export_value_subject` refuses an
/// incomplete transcript for each of them, naming `transcriptIncomplete` and
/// every producer reason.
fn verify_scheduled_implementation_location(
    transcript: &ExportValueTranscript,
    expected: Option<&typefacts::Location>,
) -> Result<(), TypeFactsCertificationError> {
    if !transcript.complete {
        return Ok(());
    }
    verify_implementation_location_identity(
        expected,
        transcript
            .implementation
            .as_ref()
            .map(|implementation| &implementation.location),
    )
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

    /// An incomplete transcript never answers about a span, so holding it to
    /// the scheduled implementation location reported a location identity
    /// failure in place of the producer's actual open reason. A complete one
    /// is still held to it exactly as before.
    #[test]
    fn an_incomplete_transcript_is_not_held_to_the_scheduled_implementation_location() {
        let scheduled = typefacts::Location {
            path: "/private/tmp/solid-checker-typefacts-project-1-1/node_modules/pkg/impl.js"
                .into(),
            start_byte: 6160,
            end_byte: 6180,
        };
        let mut open = maximally_open_export_value_transcript();
        assert!(!open.complete);
        assert!(open.implementation.is_none());
        assert!(verify_scheduled_implementation_location(&open, Some(&scheduled)).is_ok());

        // Completeness is the whole difference: the same absent implementation
        // on a transcript the producer finished is still a hard mismatch.
        open.complete = true;
        let error = verify_scheduled_implementation_location(&open, Some(&scheduled))
            .expect_err("a finished transcript owes an answer about the scheduled span");
        assert!(matches!(
            error,
            TypeFactsCertificationError::IdentityMismatch {
                field: "implementation_location",
                actual,
                ..
            } if actual.as_ref() == "None"
        ));
        assert!(verify_scheduled_implementation_location(&open, None).is_ok());
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

        // A sibling certification that staged the same entry and finds it
        // already published exact keeps the published one: nothing is retired
        // from under a project that may already link to it.
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let published = fs::metadata(&entry).unwrap().ino();
            let expected = loadable_snapshot_files(&snapshot);
            build_materialized_store_entry(&entry, &snapshot, &expected).unwrap();
            assert_eq!(fs::metadata(&entry).unwrap().ino(), published);
            assert_eq!(
                fs::read(target.join("dist/index.d.ts")).unwrap(),
                b"export declare const value: 1;"
            );
        }

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
                        "apparent": false,
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
            composed_from: None,
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
            composed_from: None,
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
                "apparent": false,
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

    /// A two-alternative root choice census: alternative 0 observed
    /// non-callable, alternative 1 observed callable, both locally closed.
    ///
    /// That is the whole classification this census carries about an
    /// alternative, which is what makes the kind comparison in
    /// `require_export_alternative_kind_matches_census` decide exactly two
    /// proposed kinds and refuse the rest.
    fn choice_export_value_transcript() -> ExportValueTranscript {
        let root_path = |alternative: usize, callability: &str| {
            json!({
                "alternative": alternative,
                "path": [],
                "presence": "required",
                "callability": callability,
                "constructability": "nonConstructable",
                "complete": true,
                "apparent": false,
                "subtreeEnumerated": true
            })
        };
        serde_json::from_value(json!({
            "location": {"path": "/project/index.d.ts", "startByte": 0, "endByte": 4},
            "value": {
                "callability": "mixed",
                "constructability": "nonConstructable",
                "primitive": {"mayBeObject": true},
                "alternatives": [{"index": 0}, {"index": 1}]
            },
            "callablePaths": [
                root_path(0, "nonCallable"),
                root_path(1, "callable")
            ],
            "complete": true
        }))
        .expect("a two-alternative export-value transcript")
    }

    #[test]
    fn the_alternative_kind_premise_admits_only_what_callability_decides() {
        let transcript = choice_export_value_transcript();
        // Only `proof.id` is read here; the subject shape is irrelevant to the
        // kind comparison.
        let demand = proof(ProofFamily::DomainExhaustiveness, selected_subject());
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "test".into(),
            reason: reason.into(),
        };
        let verify = |index: usize, alternative: &ValueShape| {
            require_export_alternative_kind_matches_census(
                &demand,
                &transcript,
                index,
                alternative,
                &open,
            )
        };

        // The two kinds callability decides, at the indices the census observed
        // them.
        verify(0, &ValueShape::Plain).expect("a non-callable observation decides a plain value");
        verify(1, &ValueShape::Callable).expect("a callable observation decides a callable value");

        // `Component` is *not* one of them, and this is the assertion that
        // pins it. `recursive_value_callability` groups it with `Callable`
        // because a component is callable, but the receipt-bound artifact then
        // names the alternative `component` — a strictly stronger claim than
        // bare callability, which every function satisfies. Nothing in this
        // census observes props, an element result, or a render-time owner, so
        // the honest answer is the same refusal every unclassifiable kind gets.
        for (index, unclassifiable) in [
            (1, ValueShape::Component),
            (
                0,
                ValueShape::Object(solid_reactive_ir::contract_semantics::KnowledgeSet::Unknown),
            ),
            (0, ValueShape::Unknown),
            (
                0,
                ValueShape::Tuple(solid_reactive_ir::contract_semantics::KnowledgeSet::Unknown),
            ),
        ] {
            let error = verify(index, &unclassifiable)
                .expect_err("a kind this census cannot classify must refuse");
            assert!(
                matches!(&error, TypeFactsCertificationError::UnsupportedDemand { reason, .. }
                    if reason.contains("alternative-kind premise required")),
                "unexpected error for {unclassifiable:?}: {error}"
            );
        }

        // The index correspondence itself: a proposal whose canonical order
        // disagrees with the producer's enumeration refuses rather than
        // certifying. That is the safe direction. It is *not* the half no
        // sibling demand covers: the per-index `recursive-value-shape` demand
        // carries `NonCallable` for a `Plain` alternative and `Callable` for a
        // `Callable` one, and `require_path_callability` verifies both against
        // the census, so a permutation of either kind refuses there too. What
        // this comparison alone refuses is a kind the census cannot classify,
        // which the loop above covers.
        for (index, permuted) in [(0, ValueShape::Callable), (1, ValueShape::Plain)] {
            let error = verify(index, &permuted)
                .expect_err("a kind the census contradicts at that index must refuse");
            assert!(
                matches!(&error, TypeFactsCertificationError::FamilyOpen { reason, .. }
                    if reason.contains("but the producer census observed")),
                "unexpected error at index {index}: {error}"
            );
        }

        // An index the producer never enumerated is not a weaker claim about
        // the same value; it is one the census cannot speak to.
        let error =
            verify(2, &ValueShape::Callable).expect_err("an unenumerated index must refuse");
        assert!(
            matches!(&error, TypeFactsCertificationError::FamilyOpen { reason, .. }
                if reason.contains("absent from the producer's own enumeration")),
            "unexpected error: {error}"
        );
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

    /// Declared-member census closure is a claim about what the package's own
    /// surface writes down. An apparent leaf is a member the compiler's
    /// global-`Function` augmentation supplies -- and two of those, `prototype`
    /// and `arguments`, are declared `any` -- so requiring them to be closed
    /// would refuse every callable value on the strength of a
    /// `Function.prototype` no package wrote. A *declared* open leaf still
    /// refuses, which is what keeps the skip from being a hole.
    #[test]
    fn census_closure_skips_apparent_leaves_and_still_refuses_declared_open_ones() {
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "test".into(),
            reason: reason.into(),
        };
        let open_leaf = |apparent: bool| typefacts::CallablePathFact {
            alternative: 0,
            path: vec![typefacts::PathSegment {
                kind: PathSegmentKind::Property,
                property: "prototype".into(),
                index: None,
            }],
            presence: PathPresence::Required,
            callability: Callability::Unknown,
            constructability: typefacts::InvocationConstructability::Unknown,
            declaration: None,
            complete: false,
            apparent,
            subtree_enumerated: false,
            open_reasons: vec!["openType".into()],
        };

        let base = export_value_transcript(json!({}));
        assert!(
            require_export_callable_paths_closed(&base, &open).is_ok(),
            "the baseline export census is closed"
        );
        let mut with_apparent = base.clone();
        with_apparent.callable_paths.push(open_leaf(true));
        assert!(
            require_export_callable_paths_closed(&with_apparent, &open).is_ok(),
            "an apparent open leaf is outside the declared-member census"
        );
        let mut with_declared = base;
        with_declared.callable_paths.push(open_leaf(false));
        assert!(
            require_export_callable_paths_closed(&with_declared, &open).is_err(),
            "a declared open leaf must still refuse the census"
        );

        let mut signature = transcript()
            .selected_signature
            .expect("selected signature")
            .clone();
        assert!(
            require_all_callable_paths_closed(&signature, &open).is_ok(),
            "the baseline signature census is closed"
        );
        signature.result_callable_paths.push(open_leaf(true));
        assert!(
            require_all_callable_paths_closed(&signature, &open).is_ok(),
            "an apparent open leaf on a result is outside the census too"
        );
        signature.parameters[0]
            .callable_paths
            .push(open_leaf(false));
        assert!(
            require_all_callable_paths_closed(&signature, &open).is_err(),
            "a declared open leaf on a parameter must still refuse the census"
        );
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
            apparent: false,
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
    fn declaration_census_selects_by_exact_snapshot_root_not_by_suffix() {
        let source = |path: &str, seed: u64| typefacts::TranscriptSourceDigest {
            path: path.into(),
            sha256: format!("sha256:{seed:064x}").try_into().unwrap(),
        };
        // `@corvu/drawer`'s graph: `@corvu/utils@0.4.2` hoisted and
        // `@corvu/utils@0.3.2` nested under `solid-transition-size`. Both end
        // in `/node_modules/@corvu/utils/dist/dom/index.d.ts`.
        let sources = vec![
            source("/p/node_modules/@corvu/utils/dist/dom/index.d.ts", 1),
            source(
                "/p/node_modules/solid-transition-size/node_modules/@corvu/utils/dist/dom/index.d.ts",
                2,
            ),
            source("/p/node_modules/@corvu/utils/dist/dom/index.js", 3),
        ];
        let hoisted = ["/p/node_modules/@corvu/utils/"];
        let nested = ["/p/node_modules/solid-transition-size/node_modules/@corvu/utils/"];
        let hoisted_matches =
            declaration_sources_under_roots(&sources, &hoisted, "dist/dom/index.d.ts");
        assert_eq!(hoisted_matches.len(), 1);
        assert_eq!(hoisted_matches[0].path, sources[0].path);
        let nested_matches =
            declaration_sources_under_roots(&sources, &nested, "dist/dom/index.d.ts");
        assert_eq!(nested_matches.len(), 1);
        assert_eq!(nested_matches[0].path, sources[1].path);
        // The same snapshot at two coordinates yields both files, and the
        // census then holds every match to the closure's digest rather than
        // refusing the count.
        let both = [hoisted[0], nested[0]];
        assert_eq!(
            declaration_sources_under_roots(&sources, &both, "dist/dom/index.d.ts").len(),
            2
        );
        // A root that shares a name prefix never matches.
        let prefix_sibling = ["/p/node_modules/@corvu/util/"];
        assert!(
            declaration_sources_under_roots(&sources, &prefix_sibling, "dist/dom/index.d.ts")
                .is_empty()
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

    fn reactive_input_operation(
        kind: OperationKind,
        role: ReactiveRole,
    ) -> solid_reactive_ir::contract_semantics::Operation {
        let mut operation = operation("callback-0", kind, per_call_cardinality(Some(0)));
        operation.inputs = vec![ValueShape::Reactive {
            role,
            resource: None,
            capabilities: solid_reactive_ir::contract_semantics::KnowledgeSet::Unknown,
        }];
        operation
    }

    fn reactive_input_export(
        operation: &solid_reactive_ir::contract_semantics::Operation,
    ) -> solid_reactive_ir::contract_semantics::ExportSemantics {
        export_semantics(
            vec![CallbackInvocation {
                from: parameter_source_at(0, &[]),
                operation: operation.id.clone(),
            }],
            vec![operation.clone()],
        )
    }

    /// One `cb(argument)` census row: the callback parameter is the callee, and
    /// slot 0 carries whatever provenance the row states.
    fn callback_call(
        start: usize,
        reach: &str,
        captured: bool,
        sources: serde_json::Value,
    ) -> serde_json::Value {
        let mut call = json!({
            "location": {"path": "/pkg/dist/index.js", "startByte": start, "endByte": start + 10},
            "reach": reach,
            "kind": "call",
            "target": "symbol:callback",
            "calleeParameter": {"parameterIndex": 0},
            "argumentParameters": [null],
            "argumentSources": [sources],
        });
        if captured {
            call["captured"] = json!(true);
            call["enclosingCallable"] =
                json!({"path": "/pkg/dist/index.js", "startByte": 1, "endByte": 900});
        }
        call
    }

    #[test]
    fn returned_parameter_identity_requires_every_return_and_complete_flow() {
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "test".into(),
            reason: reason.into(),
        };
        let good = json!({
            "location": {"path": "/pkg/dist/index.js", "startByte": 20, "endByte": 30},
            "reach": "reachable", "carryReach": "reachable", "parameter": {"parameterIndex": 0},
        });
        let cases = [
            (json!({"returns": [good.clone()]}), true),
            (json!({"returns": []}), false),
        ];
        // Build variants explicitly: absence, mismatch, conditional return,
        // unknown/unreachable edges, and an incomplete census cannot prove identity.
        let mut variants = cases.to_vec();
        for (field, value) in [
            ("parameter", json!({"parameterIndex": 1})),
            (
                "parameter",
                json!({"parameterIndex": 0, "path": [{"kind":"property", "property":"child"}]}),
            ),
            ("carryReach", json!("unknown")),
            ("reach", json!("unreachable")),
        ] {
            let mut spoiled = good.clone();
            spoiled[field] = value;
            variants.push((json!({"returns": [spoiled]}), false));
        }
        let mut absent = good.clone();
        absent.as_object_mut().unwrap().remove("parameter");
        variants.push((json!({"returns": [absent.clone()]}), false));
        variants.push((json!({"returns": [good.clone(), absent]}), false));
        // Incompleteness by class (ADR 0016, amendment of 2026-09-05). A
        // walked loop or `try` -- `@solidjs/web`'s `claimElement`, which loops
        // over its handlers and then returns its parameter -- leaves the
        // identity fact provable: the fact is flow-insensitive and the return
        // after the construct stays reachable. A construct whose flow is
        // unaccounted, or a marker the producer never classified, keeps it open.
        let loop_row = |class: &str| {
            json!({
                "marker": "iterationReachability",
                "class": class,
                "location": {"path": "/pkg/dist/index.js", "startByte": 8, "endByte": 18},
            })
        };
        variants.push((
            json!({
                "returns": [good.clone()],
                "unsupported": ["iterationReachability"],
                "incompleteness": [loop_row("reachability-lower-bound")],
            }),
            true,
        ));
        variants.push((
            json!({
                "returns": [good.clone()],
                "unsupported": ["iterationReachability"],
                "incompleteness": [loop_row("flow-unaccounted")],
            }),
            false,
        ));
        variants.push((
            json!({"returns": [good.clone()], "unsupported": ["iterationReachability"]}),
            false,
        ));
        // A return inside the walked construct is `unknown`, not `unreachable`,
        // so it still has to agree on the parameter.
        let mut inside = good.clone();
        inside["reach"] = json!("unknown");
        inside["carryReach"] = json!("unknown");
        inside["location"] = json!({"path": "/pkg/dist/index.js", "startByte": 10, "endByte": 16});
        variants.push((
            json!({
                "returns": [inside.clone(), good.clone()],
                "unsupported": ["iterationReachability"],
                "incompleteness": [loop_row("reachability-lower-bound")],
            }),
            true,
        ));
        inside["parameter"] = json!({"parameterIndex": 1});
        variants.push((
            json!({
                "returns": [inside, good],
                "unsupported": ["iterationReachability"],
                "incompleteness": [loop_row("reachability-lower-bound")],
            }),
            false,
        ));
        for (flow, expected) in variants {
            let mut implementation = implementation_with(Vec::new());
            implementation.control_flow = Some(serde_json::from_value(flow.clone()).unwrap());
            let mut sites = Vec::new();
            let result = require_returned_parameter_identity(&implementation, 0, &open, &mut sites);
            assert_eq!(result.is_ok(), expected, "{flow}: {result:?}");
            assert_eq!(sites.is_empty(), !expected);
        }
    }

    fn implementation_with(
        calls: Vec<serde_json::Value>,
    ) -> typefacts::ExportImplementationTranscript {
        implementation_with_uses(calls, Vec::new())
    }

    fn implementation_with_uses(
        calls: Vec<serde_json::Value>,
        parameter_uses: Vec<serde_json::Value>,
    ) -> typefacts::ExportImplementationTranscript {
        serde_json::from_value(json!({
            "location": {"path": "/pkg/dist/index.js", "startByte": 0, "endByte": 4},
            "calls": calls,
            "parameterUses": parameter_uses,
            "complete": true,
        }))
        .expect("a valid implementation transcript")
    }

    /// One use-census row for the callback parameter, at `start`.
    fn callback_use(start: usize, kind: &str, captured: bool) -> serde_json::Value {
        json!({
            "parameterIndex": 0,
            "location": {"path": "/pkg/dist/index.js", "startByte": start, "endByte": start + 2},
            "reach": "reachable",
            "kind": kind,
            "captured": captured,
        })
    }

    /// A call the census records with no callee parameter at all: an alias
    /// call, a reflective apply, or a member call of a value the callback was
    /// stored into.
    fn callback_call_of_other_value(start: usize, sources: serde_json::Value) -> serde_json::Value {
        json!({
            "location": {"path": "/pkg/dist/index.js", "startByte": start, "endByte": start + 10},
            "reach": "reachable",
            "kind": "call",
            "target": "symbol:other",
            "argumentSources": [sources],
        })
    }

    fn dialect_signal_source(slot: usize) -> serde_json::Value {
        json!([{
            "kind": "callResult",
            "target": "symbol:createSignal",
            "targetName": "createSignal",
            "targetModule": "solid-js",
            "targetPath": [{"kind": "tuple", "index": slot}],
        }])
    }

    fn reactive_input_result(
        role: ReactiveRole,
        calls: Vec<serde_json::Value>,
    ) -> Result<Vec<String>, TypeFactsCertificationError> {
        reactive_input_result_with_uses(role, calls, Vec::new())
    }

    fn reactive_input_result_with_uses(
        role: ReactiveRole,
        calls: Vec<serde_json::Value>,
        uses: Vec<serde_json::Value>,
    ) -> Result<Vec<String>, TypeFactsCertificationError> {
        let operation = reactive_input_operation(OperationKind::Invoke, role);
        let export = reactive_input_export(&operation);
        let implementation = implementation_with_uses(calls, uses);
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "recursive".into(),
            reason: reason.into(),
        };
        let mut sites = Vec::new();
        require_reactive_operation_input(
            &export,
            &operation,
            0,
            role,
            &implementation,
            &open,
            &mut sites,
        )?;
        Ok(sites)
    }

    // Mechanism A: the export created the accessor itself and handed it to the
    // callback, so the input's shape is proved by the callback's own call sites
    // rather than by binding the input to a parameter it does not come from.
    // This is `@solid-primitives/marker`'s `mapMatch(text)` exactly — reach
    // `Unknown`, captured inside a `createRoot(dispose => …)` callback.
    #[test]
    fn reactive_operation_input_is_proved_by_traced_dialect_provenance() {
        let sites = reactive_input_result(
            ReactiveRole::Accessor,
            vec![callback_call(
                100,
                "unknown",
                true,
                dialect_signal_source(0),
            )],
        )
        .expect("a traced dialect accessor proves the input shape");
        assert_eq!(sites.len(), 1);
        assert!(
            sites[0].starts_with("reactive-operation-input:/pkg/dist/index.js:100:110:0:solid-js:createSignal:tuple:0:"),
            "the witness names the call, the slot and the traced result: {}",
            sites[0]
        );
        assert!(
            sites[0].ends_with("enclosing:/pkg/dist/index.js:1:900"),
            "a captured call site records the body it sits in: {}",
            sites[0]
        );

        // The uncaptured case records the export's own body rather than
        // nothing, so a witness never omits where it was observed.
        let own_body = reactive_input_result(
            ReactiveRole::Accessor,
            vec![callback_call(
                100,
                "reachable",
                false,
                dialect_signal_source(0),
            )],
        )
        .expect("an uncaptured reachable call is evidence too");
        assert!(
            own_body[0].ends_with("enclosing:implementation-body"),
            "{}",
            own_body[0]
        );

        // The setter slot proves the setter role, on the same table.
        reactive_input_result(
            ReactiveRole::Setter,
            vec![callback_call(
                100,
                "reachable",
                false,
                dialect_signal_source(1),
            )],
        )
        .expect("slot 1 is the setter in every dialect that exports createSignal");
    }

    // Every trap in the design that must not clear, each as its own row. A
    // mutation that drops any one premise turns exactly one of these green.
    #[test]
    fn reactive_operation_input_refuses_every_unproven_provenance() {
        let local_factory = json!([{
            "kind": "callResult",
            "target": "symbol:myOwnAccessorFactory",
            "targetName": "myOwnAccessorFactory",
            "targetPath": [{"kind": "tuple", "index": 0}],
        }]);
        let local_same_name = json!([{
            "kind": "callResult",
            "target": "symbol:local-createSignal",
            "targetName": "createSignal",
            "targetPath": [{"kind": "tuple", "index": 0}],
        }]);
        let unresolved_callee = json!([{
            "kind": "callResult",
            "targetName": "createSignal",
            "targetModule": "solid-js",
            "targetPath": [{"kind": "tuple", "index": 0}],
        }]);
        let direct_callable = json!([{"kind": "directCallable"}]);
        let property_path = json!([{
            "kind": "callResult",
            "target": "symbol:createSignal",
            "targetName": "createSignal",
            "targetModule": "solid-js",
            "targetPath": [{"kind": "property", "property": "read"}],
        }]);
        let foreign_module = json!([{
            "kind": "callResult",
            "target": "symbol:createSignal",
            "targetName": "createSignal",
            "targetModule": "my-signals",
            "targetPath": [{"kind": "tuple", "index": 0}],
        }]);

        for (label, calls) in [
            // A non-dialect helper: traced, resolved, and answered by no
            // dialect table.
            (
                "a locally declared factory",
                vec![callback_call(100, "reachable", false, local_factory)],
            ),
            // The same, named `createSignal`. The name is not the premise; the
            // module the producer states is, and a local declaration states
            // none.
            (
                "a local function named createSignal",
                vec![callback_call(100, "reachable", false, local_same_name)],
            ),
            // A dialect name from a module no dialect exports it from.
            (
                "a foreign module",
                vec![callback_call(100, "reachable", false, foreign_module)],
            ),
            // `(options.storage || createSignal)(…)` resolves to no callee, so
            // the producer traces nothing at all.
            (
                "an untraced argument",
                vec![callback_call(100, "reachable", false, json!([]))],
            ),
            // The slot is missing from the census entirely.
            (
                "an absent slot",
                vec![json!({
                    "location": {"path": "/pkg/dist/index.js", "startByte": 100, "endByte": 110},
                    "reach": "reachable",
                    "kind": "call",
                    "target": "symbol:callback",
                    "calleeParameter": {"parameterIndex": 0},
                })],
            ),
            // A resolved trace with no callee symbol is not a resolved callee.
            (
                "an unresolved callee symbol",
                vec![callback_call(100, "reachable", false, unresolved_callee)],
            ),
            // A function literal is not a dialect call result.
            (
                "a direct callable",
                vec![callback_call(100, "reachable", false, direct_callable)],
            ),
            // A property of a result is a slot this table does not answer.
            (
                "an unaddressed result path",
                vec![callback_call(100, "reachable", false, property_path)],
            ),
            // The setter, where the accessor was demanded. This is the trap a
            // rule proving "traced to createSignal slot n" without comparing
            // roles would clear.
            (
                "the wrong role",
                vec![callback_call(
                    100,
                    "reachable",
                    false,
                    dialect_signal_source(1),
                )],
            ),
            // A second call hands a value the producer traced nothing for. The
            // claim is about the input of every invoke, so one counterexample
            // refuses -- this is what makes "some witnessing call" unsound.
            (
                "a second call with an untraced value",
                vec![
                    callback_call(100, "reachable", false, dialect_signal_source(0)),
                    callback_call(200, "reachable", false, json!([])),
                ],
            ),
            // Code after a `return` never runs, so it witnesses nothing, and a
            // census whose only matching call is unreachable proves nothing.
            (
                "only an unreachable call",
                vec![callback_call(
                    100,
                    "unreachable",
                    false,
                    dialect_signal_source(0),
                )],
            ),
            // A call of something else is not a call of the callback.
            (
                "a call of another value",
                vec![json!({
                    "location": {"path": "/pkg/dist/index.js", "startByte": 100, "endByte": 110},
                    "reach": "reachable",
                    "kind": "call",
                    "target": "symbol:other",
                    "argumentSources": [dialect_signal_source(0)],
                })],
            ),
            // A construction is not a call.
            (
                "a construction",
                vec![json!({
                    "location": {"path": "/pkg/dist/index.js", "startByte": 100, "endByte": 110},
                    "reach": "reachable",
                    "kind": "construct",
                    "target": "symbol:callback",
                    "calleeParameter": {"parameterIndex": 0},
                    "argumentSources": [dialect_signal_source(0)],
                })],
            ),
            // A producer that says `captured` without naming the callable it
            // is captured in leaves the witness unable to say where the call
            // sits, which is a fact this proof records rather than assumes.
            (
                "a captured call with no enclosing callable",
                vec![json!({
                    "location": {"path": "/pkg/dist/index.js", "startByte": 100, "endByte": 110},
                    "reach": "reachable",
                    "kind": "call",
                    "target": "symbol:callback",
                    "calleeParameter": {"parameterIndex": 0},
                    "captured": true,
                    "argumentSources": [dialect_signal_source(0)],
                })],
            ),
        ] {
            assert!(
                matches!(
                    reactive_input_result(ReactiveRole::Accessor, calls),
                    Err(TypeFactsCertificationError::FamilyOpen { .. })
                ),
                "{label} must not clear a reactive operation input"
            );
        }

        // A callback bound to a different operation is no callback for this
        // one.
        let operation = reactive_input_operation(OperationKind::Invoke, ReactiveRole::Accessor);
        let export = export_semantics(
            vec![CallbackInvocation {
                from: parameter_source_at(0, &[]),
                operation: solid_reactive_ir::contract_semantics::OperationId("callback-9".into()),
            }],
            vec![operation.clone()],
        );
        let implementation = implementation_with(vec![callback_call(
            100,
            "reachable",
            false,
            dialect_signal_source(0),
        )]);
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "recursive".into(),
            reason: reason.into(),
        };
        assert!(
            require_reactive_operation_input(
                &export,
                &operation,
                0,
                ReactiveRole::Accessor,
                &implementation,
                &open,
                &mut Vec::new(),
            )
            .is_err(),
            "the callback bound to this exact operation is the only one that answers"
        );
    }

    // The use census is the universal quantifier the call census cannot be.
    // Every shape here runs the callback with a value the call census does not
    // show, measured against the real producer, and every one of them returned
    // `Ok` before this premise existed.
    #[test]
    fn reactive_operation_input_requires_the_use_census_to_hold_no_other_route() {
        // The accepted baseline: the proved call at 100..110, its own
        // `directCall` use inside it, and nothing else.
        let proved = callback_call(100, "reachable", false, dialect_signal_source(0));
        reactive_input_result_with_uses(
            ReactiveRole::Accessor,
            vec![proved.clone()],
            vec![callback_use(100, "directCall", false)],
        )
        .expect("a call whose only use is the call itself is fully accounted for");

        // marker's real shape: the call sits inside a `createRoot(dispose =>
        // …)` callback, so the producer classifies the use as `capture` with
        // `captured: true`. It is still covered by the proved call, and a
        // premise that demanded `directCall` would refuse all three marker
        // rows.
        reactive_input_result_with_uses(
            ReactiveRole::Accessor,
            vec![callback_call(
                100,
                "unknown",
                true,
                dialect_signal_source(0),
            )],
            vec![callback_use(100, "capture", true)],
        )
        .expect("a captured call of the callback is covered by that call");

        // `cb?.(x)` is an ordinary call whose callee still starts at `cb`, so
        // the identity relation holds for it unchanged.
        reactive_input_result_with_uses(
            ReactiveRole::Accessor,
            vec![proved.clone()],
            vec![callback_use(100, "directCall", false)],
        )
        .expect("an optional call is a call");

        for (label, calls, uses) in [
            // `cb(text, cb)`: the callback is its own second argument. The
            // `argumentKnown` use sits *inside* the proved call's span, which
            // is why containment was the wrong relation and identity is the
            // right one.
            (
                "the callback as an argument of its own call",
                vec![proved.clone()],
                vec![
                    callback_use(100, "directCall", false),
                    callback_use(105, "argumentKnown", false),
                ],
            ),
            // `cb(text, (held = cb))`: the same, through an assignment the
            // producer cannot classify.
            (
                "an assignment escape inside the proved call",
                vec![proved.clone()],
                vec![
                    callback_use(100, "directCall", false),
                    callback_use(106, "unknownEscape", false),
                ],
            ),
            // `cb(text)(cb)`: the outer call's callee is the *result*, so the
            // census states no callee parameter for it, and the callback is
            // its argument -- inside the outer span, outside the proved one.
            (
                "the callback as an argument of a call of its result",
                vec![proved.clone(), callback_call_of_other_value(100, json!([]))],
                vec![
                    callback_use(100, "directCall", false),
                    callback_use(108, "argumentKnown", false),
                ],
            ),
            // `cb(text, () => cb(plain))`: the nested arrow's own call is a
            // matching call whose slot traces nothing, so the per-call rule
            // refuses first -- but the `capture` use is also not the callee of
            // the outer proved call, so either premise alone catches it.
            (
                "a nested closure that calls the callback",
                vec![proved.clone()],
                vec![
                    callback_use(100, "directCall", false),
                    callback_use(104, "capture", true),
                ],
            ),
            // `(cb)(x)`: the call starts at the parenthesis and the callee one
            // byte later. Refusing this is an accepted over-refusal of the
            // positional rule, pinned so it stays a decision.
            (
                "a parenthesized callee",
                vec![callback_call(
                    100,
                    "reachable",
                    false,
                    dialect_signal_source(0),
                )],
                vec![callback_use(101, "directCall", false)],
            ),
            // `const f = cb; f(plain)`: the alias call's callee is nil, so the
            // call census never sees it. The `storage` use is the evidence.
            (
                "an alias binding",
                vec![proved.clone(), callback_call_of_other_value(200, json!([]))],
                vec![
                    callback_use(100, "directCall", false),
                    callback_use(150, "storage", false),
                    callback_use(200, "aliasCall", false),
                ],
            ),
            // `cb.call(null, plain)` / `cb.apply(…)`: the callee is the
            // parameter at path length 1, which the exact match refuses, so
            // the call is skipped. The `propertyAccess` use is the evidence.
            (
                "a call through Function.prototype.call",
                vec![
                    proved.clone(),
                    json!({
                        "location": {"path": "/pkg/dist/index.js", "startByte": 200, "endByte": 220},
                        "reach": "reachable",
                        "kind": "call",
                        "target": "symbol:call",
                        "calleeParameter": {"parameterIndex": 0, "path": [{"kind": "property", "property": "call"}]},
                        "argumentSources": [json!([]), json!([])],
                    }),
                ],
                vec![
                    callback_use(100, "directCall", false),
                    callback_use(201, "propertyAccess", false),
                ],
            ),
            // `Reflect.apply(cb, null, [plain])`: the callback is an argument.
            (
                "a reflective apply",
                vec![proved.clone(), callback_call_of_other_value(200, json!([]))],
                vec![
                    callback_use(100, "directCall", false),
                    callback_use(205, "argumentKnown", false),
                ],
            ),
            // `holder.cb = cb; holder.cb(plain)`: the producer cannot classify
            // the escape at all.
            (
                "an escape into a property",
                vec![proved.clone(), callback_call_of_other_value(300, json!([]))],
                vec![
                    callback_use(100, "directCall", false),
                    callback_use(200, "unknownEscape", false),
                ],
            ),
            // `const holder = { cb }; holder.cb(plain)`: the object-literal
            // shorthand. Its use row exists only because the producer stopped
            // treating a shorthand name as a declaration name -- before that
            // the census recorded nothing at all here and this premise was
            // vacuous for the one spelling that names the value once.
            (
                "an object-literal shorthand escape",
                vec![proved.clone(), callback_call_of_other_value(300, json!([]))],
                vec![
                    callback_use(100, "directCall", false),
                    callback_use(210, "unknownEscape", false),
                ],
            ),
            // `return cb`: the value leaves, and what the caller passes it is
            // not in this census.
            (
                "a returned callback",
                vec![proved.clone()],
                vec![
                    callback_use(100, "directCall", false),
                    callback_use(200, "return", false),
                ],
            ),
            // `new cb(plain)`: a construction states no `calleeParameter` at
            // all, so neither the kind gate nor an exact-callee match sees it,
            // and the producer classifies its callee as a `directCall` use.
            // Only coverage catches it.
            (
                "a construction of the callback",
                vec![
                    proved.clone(),
                    json!({
                        "location": {"path": "/pkg/dist/index.js", "startByte": 200, "endByte": 215},
                        "reach": "reachable",
                        "kind": "construct",
                        "argumentSources": [json!([])],
                    }),
                ],
                vec![
                    callback_use(100, "directCall", false),
                    callback_use(205, "directCall", false),
                ],
            ),
            // The same construction from a producer that *did* state the
            // callee. The kind gate owns this one, at any path length.
            (
                "a construction naming the callback as its callee",
                vec![
                    proved.clone(),
                    json!({
                        "location": {"path": "/pkg/dist/index.js", "startByte": 200, "endByte": 215},
                        "reach": "reachable",
                        "kind": "construct",
                        "target": "symbol:callback",
                        "calleeParameter": {"parameterIndex": 0},
                        "argumentSources": [json!([])],
                    }),
                ],
                vec![callback_use(100, "directCall", false)],
            ),
            // A use in another file is not covered by a call in this one.
            (
                "a use in another file",
                vec![proved.clone()],
                vec![json!({
                    "parameterIndex": 0,
                    "location": {"path": "/pkg/dist/other.js", "startByte": 100, "endByte": 102},
                    "reach": "reachable",
                    "kind": "directCall",
                })],
            ),
        ] {
            assert!(
                matches!(
                    reactive_input_result_with_uses(ReactiveRole::Accessor, calls, uses),
                    Err(TypeFactsCertificationError::FamilyOpen { .. })
                ),
                "{label} must not clear a reactive operation input"
            );
        }

        // An unreachable use runs no more than an unreachable call, so it is
        // no route either -- and it does not refuse a proof the reachable
        // calls carry.
        reactive_input_result_with_uses(
            ReactiveRole::Accessor,
            vec![proved],
            vec![
                callback_use(100, "directCall", false),
                json!({
                    "parameterIndex": 0,
                    "location": {"path": "/pkg/dist/index.js", "startByte": 300, "endByte": 302},
                    "reach": "unreachable",
                    "kind": "storage",
                }),
            ],
        )
        .expect("a use the implementation cannot reach is not a route to the callback");
    }

    // The premises the coverage rule itself rests on. Each is a way for the
    // use census to be *unable* to answer, and each fails closed.
    #[test]
    fn reactive_operation_input_requires_an_answerable_use_census() {
        let operation = reactive_input_operation(OperationKind::Invoke, ReactiveRole::Accessor);
        let export = reactive_input_export(&operation);
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "recursive".into(),
            reason: reason.into(),
        };
        let calls = vec![callback_call(
            100,
            "reachable",
            false,
            dialect_signal_source(0),
        )];
        let uses = vec![callback_use(100, "directCall", false)];
        let attempt = |implementation: typefacts::ExportImplementationTranscript| {
            require_reactive_operation_input(
                &export,
                &operation,
                0,
                ReactiveRole::Accessor,
                &implementation,
                &open,
                &mut Vec::new(),
            )
        };

        // An incomplete transcript, and a transcript open only for
        // `controlFlowUnsupported` -- which `require_export_implementation`
        // otherwise admits. The use census withholds rows inside an
        // unsafe-jump region, so an open transcript is one whose escapes this
        // premise cannot enumerate.
        for open_reasons in [json!([]), json!(["controlFlowUnsupported"])] {
            let incomplete: typefacts::ExportImplementationTranscript =
                serde_json::from_value(json!({
                    "location": {"path": "/pkg/dist/index.js", "startByte": 0, "endByte": 4},
                    "calls": calls.clone(),
                    "parameterUses": uses.clone(),
                    "complete": open_reasons.as_array().is_some_and(|reasons| !reasons.is_empty()),
                    "openReasons": open_reasons,
                }))
                .expect("a valid implementation transcript");
            assert!(
                attempt(incomplete).is_err(),
                "a census that is incomplete or open cannot be read as exhaustive"
            );
        }

        // A callback bound to a property of a parameter, with a census that
        // would otherwise prove it: the call states that exact callee path and
        // hands it the accessor. What the census cannot state is whether
        // `props.cb` *escaped* -- its use rows are rooted at `props`, and a
        // `storage` use of `props` neither is nor is not a use of the
        // callback. Skipping those rows would leave the premise vacuous
        // exactly where the value can leave, so the demand is refused instead.
        let nested = export_semantics(
            vec![CallbackInvocation {
                from: parameter_source_at(0, &["cb"]),
                operation: operation.id.clone(),
            }],
            vec![operation.clone()],
        );
        let mut nested_call = callback_call(100, "reachable", false, dialect_signal_source(0));
        nested_call["calleeParameter"] = json!({
            "parameterIndex": 0,
            "path": [{"kind": "property", "property": "cb"}],
        });
        let mut nested_callee_use = callback_use(100, "propertyAccess", false);
        nested_callee_use["bindingPath"] = json!([{"kind": "property", "property": "cb"}]);
        let error = require_reactive_operation_input(
            &nested,
            &operation,
            0,
            ReactiveRole::Accessor,
            &implementation_with_uses(
                vec![nested_call],
                vec![
                    nested_callee_use,
                    // `const held = props;` -- rooted at the parameter, with
                    // no path, so it says nothing about the property.
                    callback_use(300, "storage", false),
                ],
            ),
            &open,
            &mut Vec::new(),
        )
        .expect_err("a callback at a property path has no use-census row of its own");
        let TypeFactsCertificationError::FamilyOpen { reason, .. } = &error else {
            panic!("unexpected error {error:?}");
        };
        assert!(
            reason.contains("not bound to a whole parameter"),
            "the refusal names the premise it fails, not a downstream symptom: {reason}"
        );

        // The rooted match is over the demanded parameter, not over "index 0
        // with an empty binding path". A use of a *different* parameter is
        // irrelevant and must not refuse; a use of this one through a
        // destructured binding path is relevant and must.
        let mut other_parameter = callback_use(300, "storage", false);
        other_parameter["parameterIndex"] = json!(1);
        reactive_input_result_with_uses(
            ReactiveRole::Accessor,
            calls.clone(),
            vec![callback_use(100, "directCall", false), other_parameter],
        )
        .expect("a use of another parameter is not a use of the callback");

        let mut bound_path = callback_use(300, "storage", false);
        bound_path["bindingPath"] = json!([{"kind": "property", "property": "handler"}]);
        assert!(
            matches!(
                reactive_input_result_with_uses(
                    ReactiveRole::Accessor,
                    calls,
                    vec![callback_use(100, "directCall", false), bound_path],
                ),
                Err(TypeFactsCertificationError::FamilyOpen { .. })
            ),
            "a use rooted at the callback through a binding path is still a use of it"
        );
    }

    // The two premises a reviewer's mutation survived. Each is pinned by a
    // transcript that only that premise refuses.
    #[test]
    fn reactive_operation_input_premises_are_each_load_bearing() {
        // `all` versus `any`: one slot, two rooted sources, one of which is a
        // setter. A rule that accepted "some source proves the role" would
        // certify this.
        let mut disagreeing = dialect_signal_source(0);
        disagreeing
            .as_array_mut()
            .expect("an array of sources")
            .push(json!({
                "kind": "callResult",
                "target": "symbol:createSignal",
                "targetName": "createSignal",
                "targetModule": "solid-js",
                "targetPath": [{"kind": "tuple", "index": 1}],
            }));
        assert!(
            matches!(
                reactive_input_result(
                    ReactiveRole::Accessor,
                    vec![callback_call(100, "reachable", false, disagreeing)]
                ),
                Err(TypeFactsCertificationError::FamilyOpen { .. })
            ),
            "two rooted sources that disagree about the role refuse the demand"
        );

        // The rooted-path selection. `cb([text])` traces slot 0 of the
        // *argument* to `createSignal` slot 0, and the argument is an array
        // holding the accessor rather than the accessor. The demand names the
        // input's root, so only a source at the root answers it -- this is the
        // premise the caller's `path.is_empty()` filter carries, and the one
        // that must not be widened into "any source anywhere in the argument".
        assert!(
            matches!(
                reactive_input_result(
                    ReactiveRole::Accessor,
                    vec![callback_call(
                        100,
                        "reachable",
                        false,
                        json!([{
                            "path": [{"kind": "tuple", "index": 0}],
                            "kind": "callResult",
                            "target": "symbol:createSignal",
                            "targetName": "createSignal",
                            "targetModule": "solid-js",
                            "targetPath": [{"kind": "tuple", "index": 0}],
                        }])
                    )]
                ),
                Err(TypeFactsCertificationError::FamilyOpen { .. })
            ),
            "an accessor inside the argument is not the argument"
        );

        // The `CallResult` kind gate. Today's producer emits a
        // `DirectCallable` with no target at all, so the gate looks
        // redundant with the target premises -- this is what deleting it
        // would cost: a function literal with a complete, plausible
        // `createSignal` provenance beside it.
        assert!(
            matches!(
                reactive_input_result(
                    ReactiveRole::Accessor,
                    vec![callback_call(
                        100,
                        "reachable",
                        false,
                        json!([{
                            "kind": "directCallable",
                            "target": "symbol:createSignal",
                            "targetName": "createSignal",
                            "targetModule": "solid-js",
                            "targetPath": [{"kind": "tuple", "index": 0}],
                        }])
                    )]
                ),
                Err(TypeFactsCertificationError::FamilyOpen { .. })
            ),
            "a function literal is not a dialect call result, whatever else the row carries"
        );
    }

    // The whole of a returned value is a slot too, and it is reachable:
    // `cb(createMemo(fn))` traces through the call-expression arm, which emits
    // an empty target path. Measured against the real producer, which reports
    // `callResult:createMemo@"solid-js"` with no target-path segments.
    #[test]
    fn reactive_operation_input_accepts_a_whole_result_accessor() {
        let sites = reactive_input_result(
            ReactiveRole::Accessor,
            vec![callback_call(
                100,
                "reachable",
                false,
                json!([{
                    "kind": "callResult",
                    "target": "symbol:createMemo",
                    "targetName": "createMemo",
                    "targetModule": "solid-js",
                }]),
            )],
        )
        .expect("createMemo returns an accessor whole in both dialects");
        assert!(
            sites[0].contains(":solid-js:createMemo:whole:"),
            "the witness names the whole result rather than a slot: {}",
            sites[0]
        );
    }

    // Which demands the arm claims at all. Every premise is required, and a
    // demand missing one falls through to the parameter-rooted path rather
    // than being answered by a value trace.
    #[test]
    fn reactive_operation_input_arm_claims_only_root_invoke_inputs() {
        let empty = solid_reactive_ir::contract_semantics::ValuePath::default();
        let invoke = reactive_input_operation(OperationKind::Invoke, ReactiveRole::Accessor);
        assert_eq!(
            reactive_operation_input_role(
                &invoke,
                &invoke.inputs[0],
                &empty,
                DemandedCallability::Unknown
            ),
            Some(ReactiveRole::Accessor)
        );

        // A `read` operation's input carries no span at all, so the callback
        // call sites this arm reads do not exist for it.
        let read = reactive_input_operation(OperationKind::Read, ReactiveRole::Accessor);
        assert_eq!(
            reactive_operation_input_role(
                &read,
                &read.inputs[0],
                &empty,
                DemandedCallability::Unknown
            ),
            None
        );

        // A callability claim is not something a value trace answers.
        for callable in [
            DemandedCallability::Callable,
            DemandedCallability::NonCallable,
        ] {
            assert_eq!(
                reactive_operation_input_role(&invoke, &invoke.inputs[0], &empty, callable),
                None,
                "{callable:?} asserts something this witness does not prove"
            );
        }

        // A property inside the traced value is not what the census traced.
        let property = solid_reactive_ir::contract_semantics::ValuePath(vec![
            ValuePathSegment::ObjectProperty("read".into()),
        ]);
        assert_eq!(
            reactive_operation_input_role(
                &invoke,
                &invoke.inputs[0],
                &property,
                DemandedCallability::Unknown
            ),
            None
        );

        // Every other shape stays unsupported. `Plain` carries a negative
        // callability claim the census cannot make about an argument value,
        // and `Object` needs a property path the tracer does not produce.
        for shape in [
            ValueShape::Plain,
            ValueShape::Callable,
            ValueShape::Object(solid_reactive_ir::contract_semantics::KnowledgeSet::Unknown),
            ValueShape::Store {
                resource: None,
                capabilities: solid_reactive_ir::contract_semantics::KnowledgeSet::Unknown,
            },
        ] {
            assert_eq!(
                reactive_operation_input_role(
                    &invoke,
                    &shape,
                    &empty,
                    DemandedCallability::Unknown
                ),
                None,
                "{shape:?} is not a reactive input this arm proves"
            );
        }
    }

    /// One census row whose callee carries `callee_sources`, and nothing else:
    /// mechanism B reads the callee's provenance, never a parameter root.
    fn callee_traced_call(
        start: usize,
        reach: &str,
        captured: bool,
        sources: serde_json::Value,
    ) -> serde_json::Value {
        let mut call = json!({
            "location": {"path": "/pkg/dist/index.js", "startByte": start, "endByte": start + 12},
            "reach": reach,
            "kind": "call",
            "target": "symbol:binding",
            "targetName": "depSignal",
            "declaration": {
                "kind": "BindingElement",
                "name": "depSignal",
                "sourceFile": "/pkg/dist/index.js",
                "location": {"path": "/pkg/dist/index.js", "startByte": 10, "endByte": 20},
            },
            "calleeSources": sources,
        });
        if captured {
            call["captured"] = json!(true);
            call["enclosingCallable"] =
                json!({"path": "/pkg/dist/index.js", "startByte": 1, "endByte": 900});
        }
        call
    }

    fn read_input_result(
        role: ReactiveRole,
        calls: Vec<serde_json::Value>,
    ) -> Result<Vec<String>, TypeFactsCertificationError> {
        let operation = reactive_input_operation(OperationKind::Read, role);
        let implementation = implementation_with(calls);
        let open = |reason: &str| TypeFactsCertificationError::FamilyOpen {
            demand: "read".into(),
            reason: reason.into(),
        };
        let mut sites = Vec::new();
        require_reactive_read_operation_input(
            &operation,
            role,
            &implementation,
            operation_reachability_floor(&operation),
            &open,
            &mut sites,
        )
        .map(|()| sites)
    }

    // Mechanism B's positive case, and the shape it was built for:
    // `@solid-primitives/timer`'s `createPolled` reads `depSignal()`, bound by
    // `const [depSignal] = createSignal(...)`, in its own body.
    #[test]
    fn reactive_read_operation_input_is_proved_by_a_traced_dialect_callee() {
        let sites = read_input_result(
            ReactiveRole::Accessor,
            vec![callee_traced_call(
                4105,
                "reachable",
                false,
                dialect_signal_source(0),
            )],
        )
        .expect("a traced dialect accessor callee proves the read");
        assert_eq!(
            sites,
            vec![
                "reactive-read-operation-input:/pkg/dist/index.js:4105:4117:solid-js:createSignal:tuple:0"
                    .to_owned()
            ]
        );

        // The witness list carries *every* matching call, because the row is
        // existential over the export's census: two reads of the same
        // `(kind, label)` collapse into one operation, so the receipt has to
        // record the set the row was proved against rather than one site.
        let both = read_input_result(
            ReactiveRole::Accessor,
            vec![
                callee_traced_call(4105, "reachable", false, dialect_signal_source(0)),
                callee_traced_call(4200, "unknown", false, dialect_signal_source(0)),
                // An ordinary call in the same body: an empty trace is the
                // producer's silence, not a counterexample, so it neither
                // witnesses nor refuses. Universal quantification here would
                // be vacuously false for every real body.
                callee_traced_call(4300, "reachable", false, json!([])),
            ],
        )
        .expect("the arm is existential over the census");
        assert_eq!(both.len(), 2, "{both:?}");

        // Writing a signal and reading it in one body is ordinary, and the
        // accessor row is honestly witnessed by the read. A
        // no-counterexample clause was implemented here and removed for
        // exactly this case: `setG(4)` proves the *setter* role, which
        // `traced_source_proves_role` already declines to accept for an
        // accessor demand, so refusing on it added a false refusal and no
        // soundness.
        let beside_a_setter = read_input_result(
            ReactiveRole::Accessor,
            vec![
                callee_traced_call(100, "reachable", false, dialect_signal_source(1)),
                callee_traced_call(200, "reachable", false, dialect_signal_source(0)),
            ],
        )
        .expect("a setter call beside the read does not contradict the read");
        assert_eq!(beside_a_setter.len(), 1, "{beside_a_setter:?}");
        assert!(
            beside_a_setter[0].ends_with("tuple:0"),
            "{beside_a_setter:?}"
        );

        // A setter read is a different row, and the same census proves it
        // through slot 1 rather than slot 0.
        let setter = read_input_result(
            ReactiveRole::Setter,
            vec![callee_traced_call(
                4105,
                "reachable",
                false,
                dialect_signal_source(1),
            )],
        )
        .expect("slot 1 is the setter");
        assert!(setter[0].ends_with("tuple:1"), "{setter:?}");
    }

    // Every premise of mechanism B, one row each. These are the traps a rule
    // that read "the callee traces to something" would clear.
    #[test]
    fn reactive_read_operation_input_refuses_every_unproven_callee() {
        let non_dialect_helper = json!([{
            "kind": "callResult",
            "target": "symbol:myOwnAccessorFactory",
            "targetName": "myOwnAccessorFactory",
            "targetPath": [{"kind": "tuple", "index": 0}],
        }]);
        let locally_declared_creator = json!([{
            "kind": "callResult",
            "target": "symbol:localCreateSignal",
            "targetName": "createSignal",
            "targetPath": [{"kind": "tuple", "index": 0}],
        }]);
        let unresolved_callee = json!([{
            "kind": "callResult",
            "targetName": "createSignal",
            "targetModule": "solid-js",
            "targetPath": [{"kind": "tuple", "index": 0}],
        }]);
        let direct_callable = json!([{
            "kind": "directCallable",
            "target": "symbol:createSignal",
            "targetName": "createSignal",
            "targetModule": "solid-js",
            "targetPath": [{"kind": "tuple", "index": 0}],
        }]);
        let nested_path = json!([{
            "kind": "callResult",
            "target": "symbol:createSignal",
            "targetName": "createSignal",
            "targetModule": "solid-js",
            "targetPath": [{"kind": "tuple", "index": 0}],
            "path": [{"kind": "property", "property": "inner"}],
        }]);

        for (why, calls) in [
            // The reads that trace to nothing: a computed callee
            // (`(options.storage || createSignal)(…)`), a reassigned `let`,
            // and every ordinary member call in the body. All three reach the
            // arm as an empty list, and an empty list is silence.
            (
                "nothing traced",
                vec![callee_traced_call(100, "reachable", false, json!([]))],
            ),
            // A helper the dialect does not export the name from. The empty
            // module is what refuses it, and a *locally declared* function
            // named `createSignal` is the same refusal for the same reason.
            (
                "a non-dialect helper",
                vec![callee_traced_call(
                    100,
                    "reachable",
                    false,
                    non_dialect_helper,
                )],
            ),
            (
                "a locally declared creator of the dialect's name",
                vec![callee_traced_call(
                    100,
                    "reachable",
                    false,
                    locally_declared_creator,
                )],
            ),
            // A trace with no callee symbol is not a resolved callee.
            (
                "an unresolved callee symbol",
                vec![callee_traced_call(
                    100,
                    "reachable",
                    false,
                    unresolved_callee,
                )],
            ),
            // A function literal is not a dialect call result.
            (
                "a direct callable",
                vec![callee_traced_call(100, "reachable", false, direct_callable)],
            ),
            // A property *of* the traced value is not the traced value.
            (
                "a non-root source path",
                vec![callee_traced_call(100, "reachable", false, nested_path)],
            ),
            // The `captured` veto, which is load-bearing rather than
            // decorative: these operations publish `at: call / schedule:
            // same-stack` and `ContractReactiveRead` has no schedule column,
            // so a read inside a closure the export hands away does not
            // witness the row. `createPolled`'s second `depSignal()` — inside
            // `createEffect(() => depSignal(), …)` — is exactly this.
            (
                "only a captured call",
                vec![callee_traced_call(
                    100,
                    "reachable",
                    true,
                    dialect_signal_source(0),
                )],
            ),
            // Code after a `return` never runs.
            (
                "only an unreachable call",
                vec![callee_traced_call(
                    100,
                    "unreachable",
                    false,
                    dialect_signal_source(0),
                )],
            ),
            // A construction is a different claim about the value.
            (
                "only a construction",
                vec![json!({
                    "location": {"path": "/pkg/dist/index.js", "startByte": 100, "endByte": 112},
                    "reach": "reachable",
                    "kind": "construct",
                    "calleeSources": dialect_signal_source(0),
                })],
            ),
            // An absent kind is never read as "call".
            (
                "only a call of unstated kind",
                vec![json!({
                    "location": {"path": "/pkg/dist/index.js", "startByte": 100, "endByte": 112},
                    "reach": "reachable",
                    "calleeSources": dialect_signal_source(0),
                })],
            ),
            // The wrong role: `setPolled(v)` traces to `createSignal` slot 1,
            // and a rule proving "traced to createSignal slot n" without
            // comparing roles would certify a setter write as an accessor
            // read.
            (
                "the wrong role",
                vec![callee_traced_call(
                    100,
                    "reachable",
                    false,
                    dialect_signal_source(1),
                )],
            ),
            ("an empty census", Vec::new()),
        ] {
            assert!(
                read_input_result(ReactiveRole::Accessor, calls).is_err(),
                "{why} proved a reactive read"
            );
        }
    }

    /// The composing call, and every shape that must not be read as one.
    ///
    /// The premise is declaration *identity*, so the traps are the ones a
    /// weaker relation would clear: a call to a same-named declaration in
    /// another module, a call the export makes from inside a closure it hands
    /// away, a call after a `return`, and a construction.
    #[test]
    fn composing_call_is_resolved_by_declaration_identity() {
        let target: typefacts::ResolvedDeclaration = serde_json::from_value(json!({
            "symbol": "symbol:createPolled",
            "name": "createPolled",
            "kind": "FunctionDeclaration",
            "sourceFile": "/pkg/dist/index.js",
            "location": {"path": "/pkg/dist/index.js", "startByte": 3810, "endByte": 3822},
        }))
        .expect("a valid declaration");
        let call = |overrides: serde_json::Value| -> typefacts::ImplementationCall {
            let mut value = json!({
                "location": {"path": "/pkg/dist/index.js", "startByte": 4655, "endByte": 4709},
                "reach": "reachable",
                "kind": "call",
                "target": "symbol:createPolled",
                "targetName": "createPolled",
                "declaration": {
                    "symbol": "symbol:createPolled",
                    "name": "createPolled",
                    "kind": "FunctionDeclaration",
                    "sourceFile": "/pkg/dist/index.js",
                    "location": {"path": "/pkg/dist/index.js", "startByte": 3810, "endByte": 3822},
                },
            });
            let object = value.as_object_mut().expect("an object");
            for (key, replacement) in overrides.as_object().expect("an object") {
                if replacement.is_null() {
                    object.remove(key);
                } else {
                    object.insert(key.clone(), replacement.clone());
                }
            }
            serde_json::from_value(value).expect("a valid call")
        };
        let floor = ReachabilityFloor::MayExecute;
        assert!(composing_call_resolves_to_declaration(
            &call(json!({})),
            &target,
            "createPolled",
            floor
        ));

        for (why, overrides, runtime_export) in [
            // Another module's declaration of the same name. This is the
            // must-not-clear the whole premise exists for.
            (
                "a same-named declaration elsewhere",
                json!({"declaration": {
                    "symbol": "symbol:otherCreatePolled",
                    "name": "createPolled",
                    "kind": "FunctionDeclaration",
                    "sourceFile": "/pkg/dist/other.js",
                    "location": {"path": "/pkg/dist/other.js", "startByte": 100, "endByte": 112},
                }}),
                "createPolled",
            ),
            // The same symbol at a different range is a producer this side
            // does not understand, not a match to be salvaged.
            (
                "the same symbol at a different range",
                json!({"declaration": {
                    "symbol": "symbol:createPolled",
                    "name": "createPolled",
                    "kind": "FunctionDeclaration",
                    "sourceFile": "/pkg/dist/index.js",
                    "location": {"path": "/pkg/dist/index.js", "startByte": 9000, "endByte": 9012},
                }}),
                "createPolled",
            ),
            // An empty symbol is refused rather than treated as a wildcard.
            (
                "an unstated declaration symbol",
                json!({"declaration": {
                    "name": "createPolled",
                    "kind": "FunctionDeclaration",
                    "sourceFile": "/pkg/dist/index.js",
                    "location": {"path": "/pkg/dist/index.js", "startByte": 3810, "endByte": 3822},
                }}),
                "createPolled",
            ),
            // No declaration at all.
            (
                "no declaration",
                json!({"declaration": null}),
                "createPolled",
            ),
            // The snapshot-replayed export name disagrees, so the declaration
            // this call resolves to is not the one bound to that name.
            (
                "a runtime export name that disagrees",
                json!({}),
                "createIntervalCounter",
            ),
            // Inside a closure the export hands away: not the same stack, so
            // the composed row's `same-stack` stamp would not survive.
            (
                "a captured call",
                json!({
                    "captured": true,
                    "enclosingCallable": {"path": "/pkg/dist/index.js", "startByte": 1, "endByte": 900},
                }),
                "createPolled",
            ),
            // Code after a `return` never runs.
            (
                "an unreachable call",
                json!({"reach": "unreachable"}),
                "createPolled",
            ),
            // A construction is a different claim about the value.
            (
                "a construction",
                json!({"kind": "construct"}),
                "createPolled",
            ),
            // An absent kind is never read as "call".
            ("an unstated kind", json!({"kind": null}), "createPolled"),
        ] {
            assert!(
                !composing_call_resolves_to_declaration(
                    &call(overrides),
                    &target,
                    runtime_export,
                    floor
                ),
                "{why} was read as the composing call"
            );
        }
    }

    /// The composition premise, end to end and plan-free.
    ///
    /// The chain is driven by a test resolver over an in-memory map of
    /// exports, so every premise `require_composed_operation_chain` owns is
    /// reachable: the composing call, the same-claim comparison, the
    /// cycle refusals, and the depth bound. Each of the three guards a
    /// mutation could delete has its own row below, and each row fails if the
    /// guard is removed.
    struct ComposedFixture {
        exports: Vec<(
            String,
            solid_reactive_ir::contract_semantics::ExportSemantics,
            typefacts::ExportImplementationTranscript,
        )>,
    }

    impl ComposedFixture {
        /// One export: a `read-0` operation, optionally composed from
        /// `composed`, and a census that calls each named target directly and
        /// reads its own accessor when it composes nothing.
        fn push(
            &mut self,
            name: &str,
            composed: Option<(&str, &str)>,
            targets: &[&str],
        ) -> &mut Self {
            let mut operation =
                reactive_input_operation(OperationKind::Read, ReactiveRole::Accessor);
            operation.id = solid_reactive_ir::contract_semantics::OperationId("read-0".into());
            operation.composed_from =
                composed.map(
                    |(export, id)| solid_reactive_ir::contract_semantics::ComposedFrom {
                        export: export.into(),
                        operation: solid_reactive_ir::contract_semantics::OperationId(id.into()),
                    },
                );
            let mut calls = targets
                .iter()
                .map(|target| Self::call_of(target))
                .collect::<Vec<_>>();
            if composed.is_none() {
                calls.push(callee_traced_call(
                    9000,
                    "reachable",
                    false,
                    dialect_signal_source(0),
                ));
            }
            let mut export = export_semantics(Vec::new(), vec![operation]);
            export.identity.public_name = name.into();
            let mut implementation = implementation_with(calls);
            implementation.declaration = Some(Self::declaration(name));
            self.exports.push((name.to_owned(), export, implementation));
            self
        }

        fn declaration(name: &str) -> typefacts::ResolvedDeclaration {
            serde_json::from_value(json!({
                "symbol": format!("symbol:{name}"),
                "name": name,
                "kind": "FunctionDeclaration",
                "sourceFile": "/pkg/dist/index.js",
                "location": {
                    "path": "/pkg/dist/index.js",
                    "startByte": 100 + name.len(),
                    "endByte": 200 + name.len(),
                },
            }))
            .expect("a valid declaration")
        }

        fn call_of(name: &str) -> serde_json::Value {
            json!({
                "location": {"path": "/pkg/dist/index.js", "startByte": 4000, "endByte": 4050},
                "reach": "reachable",
                "kind": "call",
                "target": format!("symbol:{name}"),
                "targetName": name,
                "declaration": {
                    "symbol": format!("symbol:{name}"),
                    "name": name,
                    "kind": "FunctionDeclaration",
                    "sourceFile": "/pkg/dist/index.js",
                    "location": {
                        "path": "/pkg/dist/index.js",
                        "startByte": 100 + name.len(),
                        "endByte": 200 + name.len(),
                    },
                },
            })
        }

        fn get(
            &self,
            name: &str,
        ) -> &(
            String,
            solid_reactive_ir::contract_semantics::ExportSemantics,
            typefacts::ExportImplementationTranscript,
        ) {
            self.exports
                .iter()
                .find(|(candidate, ..)| candidate == name)
                .expect("a fixture export")
        }

        fn resolve(&self, name: &str) -> Result<ComposedTarget<'_>, String> {
            let (stored, export, implementation) = self
                .exports
                .iter()
                .find(|(candidate, ..)| candidate == name)
                .ok_or_else(|| format!("no transcript for {name}"))?;
            Ok(ComposedTarget {
                export,
                implementation,
                runtime_export: stored.as_str(),
            })
        }

        /// Discharge `from`'s own composed `read-0` through the chain.
        fn chain(&self, from: &str) -> Result<Vec<String>, String> {
            let (_, export, implementation) = self.get(from);
            let operation = export.operation("read-0").expect("the read operation");
            let composed = operation
                .composed_from
                .as_ref()
                .expect("a composed operation");
            let resolve = |name: &str| self.resolve(name);
            let mut visited = vec![from.to_owned()];
            require_composed_operation_chain(
                from,
                operation,
                composed,
                implementation,
                &resolve,
                &mut visited,
                0,
            )
        }
    }

    #[test]
    fn composed_operation_chain_proves_one_hop_and_refuses_every_broken_premise() {
        // One hop: `composes` calls `reads` directly, and `reads` reads its own
        // accessor. Both witness kinds appear, so the receipt records the call
        // that composed and the read that was composed.
        let mut fixture = ComposedFixture {
            exports: Vec::new(),
        };
        fixture
            .push("composes", Some(("reads", "read-0")), &["reads"])
            .push("reads", None, &[]);
        let sites = fixture.chain("composes").expect("a one-hop composition");
        assert_eq!(
            sites,
            vec![
                "composed-operation-call:/pkg/dist/index.js:4000:4050:reads:read-0".to_owned(),
                "reactive-read-operation-input:/pkg/dist/index.js:9000:9012:solid-js:createSignal:tuple:0"
                    .to_owned(),
            ]
        );

        // Two hops, and the chain records every one of them.
        let mut chained = ComposedFixture {
            exports: Vec::new(),
        };
        chained
            .push("outer", Some(("middle", "read-0")), &["middle"])
            .push("middle", Some(("inner", "read-0")), &["inner"])
            .push("inner", None, &[]);
        let two = chained.chain("outer").expect("a two-hop composition");
        assert_eq!(two.len(), 3, "{two:?}");

        // The target's own demand refuses, so the composed row refuses: this
        // is the premise that makes provenance a proof obligation rather than
        // an assertion.
        let mut unproven = ComposedFixture {
            exports: Vec::new(),
        };
        unproven
            .push("composes", Some(("reads", "read-0")), &["reads"])
            .push("reads", None, &[]);
        unproven.exports[1].2 = implementation_with(Vec::new());
        unproven.exports[1].2.declaration = Some(ComposedFixture::declaration("reads"));
        let refusal = unproven.chain("composes").expect_err("an unproven target");
        assert!(
            refusal.contains("no reachable uncaptured call"),
            "{refusal}"
        );

        // The composing export makes no call to the target at all.
        let mut uncalled = ComposedFixture {
            exports: Vec::new(),
        };
        uncalled
            .push("composes", Some(("reads", "read-0")), &[])
            .push("reads", None, &[]);
        let refusal = uncalled.chain("composes").expect_err("no composing call");
        assert!(
            refusal.contains("has no reachable uncaptured call of reads"),
            "{refusal}"
        );

        // A target the resolver cannot answer.
        let mut absent = ComposedFixture {
            exports: Vec::new(),
        };
        absent.push("composes", Some(("missing", "read-0")), &["missing"]);
        let refusal = absent.chain("composes").expect_err("an absent target");
        assert!(refusal.contains("no transcript for missing"), "{refusal}");

        // An operation absent from a target that does exist.
        let mut wrong_id = ComposedFixture {
            exports: Vec::new(),
        };
        wrong_id
            .push("composes", Some(("reads", "read-9")), &["reads"])
            .push("reads", None, &[]);
        let refusal = wrong_id
            .chain("composes")
            .expect_err("an out-of-range foreign ordinal");
        assert!(refusal.contains("read-9 is absent from reads"), "{refusal}");
    }

    /// The same-claim guard, driven through the chain.
    ///
    /// Deleting `composed_operation_states_the_same_claim`'s call site makes
    /// this pass a composition whose target promises something else.
    #[test]
    fn composed_operation_chain_refuses_a_target_with_a_different_claim() {
        let mut fixture = ComposedFixture {
            exports: Vec::new(),
        };
        fixture
            .push("composes", Some(("reads", "read-0")), &["reads"])
            .push("reads", None, &[]);
        let target = fixture.exports[1]
            .1
            .call
            .operations
            .get_mut(0)
            .expect("the target operation");
        target.schedule = Some(solid_reactive_ir::contract_semantics::Schedule::Queued);
        let refusal = fixture
            .chain("composes")
            .expect_err("a target with a different schedule");
        assert!(
            refusal.contains("does not state the same claim"),
            "{refusal}"
        );
    }

    /// A composing row that claims resources of its own is not a row
    /// composition can discharge, because this premise proves nothing about
    /// them.
    #[test]
    fn composed_operation_chain_refuses_a_composing_row_that_claims_resources() {
        let mut fixture = ComposedFixture {
            exports: Vec::new(),
        };
        fixture
            .push("composes", Some(("reads", "read-0")), &["reads"])
            .push("reads", None, &[]);
        let composing = fixture.exports[0]
            .1
            .call
            .operations
            .get_mut(0)
            .expect("the composing operation");
        composing
            .resources
            .insert(solid_reactive_ir::contract_semantics::ResourceId(
                "case:composes:resource:owner".into(),
            ));
        let refusal = fixture
            .chain("composes")
            .expect_err("a composing row claiming a resource");
        assert!(
            refusal.contains("does not state the same claim"),
            "{refusal}"
        );
    }

    /// The two cycle refusals and the depth bound, each on its own.
    ///
    /// Deleting the self-composition check makes the alias row pass; deleting
    /// the visited set makes the two-export cycle spin until the depth bound
    /// catches it with the wrong reason; deleting the depth bound makes the
    /// nine-export chain pass.
    #[test]
    fn composed_operation_chain_refuses_cycles_and_bounds_its_depth() {
        // A provenance naming its own export, by alias.
        let mut itself = ComposedFixture {
            exports: Vec::new(),
        };
        itself.push("composes", Some(("composes", "read-0")), &["composes"]);
        let refusal = itself.chain("composes").expect_err("a self-composition");
        assert_eq!(
            refusal, "composed operation names its own export composes",
            "{refusal}"
        );

        // A cycle through a second export: refused as a cycle, not by running
        // out of depth.
        let mut cycle = ComposedFixture {
            exports: Vec::new(),
        };
        cycle
            .push("composes", Some(("reads", "read-0")), &["reads"])
            .push("reads", Some(("composes", "read-0")), &["composes"]);
        let refusal = cycle.chain("composes").expect_err("a two-export cycle");
        assert_eq!(
            refusal, "composed operation chain revisits composes at read-0",
            "{refusal}"
        );

        // An acyclic chain of distinct exports, longer than the bound. Eight
        // hops is the last accepted length, so nine refuses.
        let chain_of = |length: usize| {
            let mut fixture = ComposedFixture {
                exports: Vec::new(),
            };
            let names = (0..length)
                .map(|index| format!("e{index}"))
                .collect::<Vec<_>>();
            for (index, name) in names.iter().enumerate() {
                match names.get(index + 1) {
                    Some(next) => {
                        fixture.push(name, Some((next.as_str(), "read-0")), &[next.as_str()]);
                    }
                    None => {
                        fixture.push(name, None, &[]);
                    }
                }
            }
            fixture.chain("e0")
        };
        assert!(
            chain_of(MAX_COMPOSITION_DEPTH + 1).is_ok(),
            "a chain of exactly the bound must still prove"
        );
        let refusal = chain_of(MAX_COMPOSITION_DEPTH + 2).expect_err("an over-long chain");
        assert!(
            refusal.contains(&format!("exceeds {MAX_COMPOSITION_DEPTH} hops")),
            "{refusal}"
        );
    }

    /// A composed row may only name a target that states the *same* claim.
    ///
    /// One field at a time, because each of them is a promise: a target with a
    /// tighter cardinality or a different schedule is a different row, and
    /// publishing this export's row against it would put a promise into this
    /// contract that the target never made.
    #[test]
    fn composed_provenance_requires_the_target_to_state_the_same_claim() {
        let base = reactive_input_operation(OperationKind::Read, ReactiveRole::Accessor);
        let mut target = base.clone();
        // The id and the provenance are deliberately not compared: the target
        // is a different export's operation, and following its own provenance
        // is the recursion's job.
        target.id = solid_reactive_ir::contract_semantics::OperationId("read-0".into());
        target.composed_from = Some(solid_reactive_ir::contract_semantics::ComposedFrom {
            export: "deeper".into(),
            operation: solid_reactive_ir::contract_semantics::OperationId("case:deeper".into()),
        });
        assert!(composed_operation_states_the_same_claim(&base, &target));

        type Mutation = (
            &'static str,
            fn(&mut solid_reactive_ir::contract_semantics::Operation),
        );
        let mutate: [Mutation; 8] = [
            ("kind", |operation| {
                operation.kind = OperationKind::Write;
            }),
            ("inputs", |operation| {
                operation.inputs = vec![ValueShape::Plain];
            }),
            ("output", |operation| {
                operation.output = Some(ValueShape::Plain);
            }),
            ("execution point", |operation| {
                operation.at = Some(solid_reactive_ir::contract_semantics::Event::Flush);
            }),
            ("schedule", |operation| {
                operation.schedule = Some(solid_reactive_ir::contract_semantics::Schedule::Queued);
            }),
            ("cardinality", |operation| {
                operation.cardinality.min = Some(1);
            }),
            ("tracking", |operation| {
                operation.tracking = solid_reactive_ir::contract_semantics::Tracking::Tracked;
            }),
            ("trigger", |operation| {
                operation.trigger = Some(solid_reactive_ir::contract_semantics::Trigger::Event(
                    solid_reactive_ir::contract_semantics::Event::Render,
                ));
            }),
        ];
        for (field, apply) in mutate {
            let mut divergent = target.clone();
            apply(&mut divergent);
            assert!(
                !composed_operation_states_the_same_claim(&base, &divergent),
                "a target differing in {field} was accepted as the same claim"
            );
        }
    }

    // The arm's own gate: only a `read` operation whose input is `Reactive`.
    // A parameter-rooted read keeps going through the parameter path, which is
    // a different witness (`require_parameter_read_evidence`) against a
    // different fact.
    #[test]
    fn reactive_read_operation_input_arm_claims_only_reactive_read_inputs() {
        let read = reactive_input_operation(OperationKind::Read, ReactiveRole::Accessor);
        assert_eq!(
            reactive_read_operation_input_role(&read, &read.inputs[0]),
            Some(ReactiveRole::Accessor)
        );

        // An `invoke` operation's input is mechanism A's, quantified over the
        // callback's own call sites; reading it here would drop that
        // quantification for a weaker existential one.
        let invoke = reactive_input_operation(OperationKind::Invoke, ReactiveRole::Accessor);
        assert_eq!(
            reactive_read_operation_input_role(&invoke, &invoke.inputs[0]),
            None
        );
        for kind in [
            OperationKind::Return,
            OperationKind::Create,
            OperationKind::Write,
        ] {
            let other = reactive_input_operation(kind, ReactiveRole::Accessor);
            assert_eq!(
                reactive_read_operation_input_role(&other, &other.inputs[0]),
                None,
                "{kind:?} is not a read"
            );
        }

        // Every other input shape stays unsupported, `Parameter` above all:
        // that one has a census witness of its own and must not be answered
        // by a callee trace.
        for shape in [
            ValueShape::Parameter {
                index: 0,
                path: Vec::new(),
            },
            ValueShape::Plain,
            ValueShape::Callable,
            ValueShape::Unknown,
            ValueShape::Object(solid_reactive_ir::contract_semantics::KnowledgeSet::Unknown),
            ValueShape::Store {
                resource: None,
                capabilities: solid_reactive_ir::contract_semantics::KnowledgeSet::Unknown,
            },
        ] {
            assert_eq!(
                reactive_read_operation_input_role(&read, &shape),
                None,
                "{shape:?} is not a reactive read input this arm proves"
            );
        }
    }

    // The refusal every unsupported operation input takes, and the whole
    // reason it was rewritten: the message names the demand, the exact input,
    // the shape and the family, so the audit sidecar can attribute it instead
    // of reporting `demandId: null, family: null` for six different packages.
    #[test]
    fn unsupported_operation_input_refusal_names_the_demand_and_the_shape() {
        // The operation identity an emitted document carries: already
        // qualified by artifact case and export, which is why the refusal
        // names the input once instead of repeating those two fields.
        const OPERATION: &str = "artifact-case:097ee468:createMarker:operation:callback-0";
        let proof = proof(
            ProofFamily::RecursiveValueShape,
            ProofDemandSubject::PositiveFact(PositiveFactSubject::RecursiveValue {
                artifact_case: "artifact-case:097ee468".into(),
                export: "createMarker".into(),
                root: ValueRoot::OperationInput {
                    operation: solid_reactive_ir::contract_semantics::OperationId(OPERATION.into()),
                    index: 0,
                },
                path: solid_reactive_ir::contract_semantics::ValuePath::default(),
                callable: DemandedCallability::Unknown,
            }),
        );
        let error = operation_input_parameter_root(
            &ValueShape::Reactive {
                role: ReactiveRole::Accessor,
                resource: None,
                capabilities: solid_reactive_ir::contract_semantics::KnowledgeSet::Unknown,
            },
            &proof,
            OPERATION,
            0,
        )
        .expect_err("a reactive input has no parameter root");
        let TypeFactsCertificationError::UnsupportedDemand { demand, reason } = &error else {
            panic!("unexpected error {error:?}");
        };
        assert_eq!(demand, &proof.id);
        assert_eq!(
            reason,
            "operation input artifact-case:097ee468:createMarker:operation:callback-0[0] is reactive/accessor, and the implementation census binds only parameter-rooted operation inputs (family=recursive-value-shape)"
        );

        // The shape spelling is stable and per-constructor, so a test may pin
        // it and a shape gaining a field cannot move it.
        assert_eq!(value_shape_constructor(&ValueShape::Plain), "plain");
        assert_eq!(
            value_shape_constructor(&ValueShape::Object(
                solid_reactive_ir::contract_semantics::KnowledgeSet::Unknown
            )),
            "object"
        );
        assert_eq!(
            value_shape_constructor(&ValueShape::Reactive {
                role: ReactiveRole::Setter,
                resource: None,
                capabilities: solid_reactive_ir::contract_semantics::KnowledgeSet::Unknown,
            }),
            "reactive/setter"
        );

        // A parameter-rooted input still answers, with its exact path.
        let (index, path) = operation_input_parameter_root(
            &ValueShape::Parameter {
                index: 1,
                path: vec!["of".into()],
            },
            &proof,
            OPERATION,
            0,
        )
        .expect("a parameter-rooted input has a root");
        assert_eq!((index, path), (1, vec!["of".to_owned()]));
    }

    /// The exact audited `package.json` bytes of one rc.3 archive.
    ///
    /// Checked in under `benchmarks/package-contract-v2/phase0/rc3/`, and
    /// digest-equal to the `package.manifest.sha256` the bundled contract for
    /// the same archive carries. Reading them is what makes the four-field
    /// identity gate reachable from a unit test at all: the fourth field is a
    /// digest of these bytes, and no synthesised payload can produce it.
    fn audited_rc3_manifest(directory: &str) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("benchmarks/package-contract-v2/phase0/rc3")
            .join(directory)
            .join("package.json");
        std::fs::read(&path).unwrap_or_else(|error| {
            panic!(
                "the audited rc.3 manifest {} is not readable: {error}",
                path.display()
            )
        })
    }

    /// A snapshot carrying the archive's `package.json` and the two
    /// declaration files these tests resolve callees into. Both declaration
    /// members are present because the tier requires the stripped declaration
    /// path to be a member of the authenticated archive.
    fn archive_snapshot(
        name: &str,
        version: &str,
        integrity: &str,
        manifest: &[u8],
        root: &str,
    ) -> super::super::ArtifactSnapshot {
        super::super::ArtifactSnapshot {
            package_name: name.into(),
            package_version: version.into(),
            package_integrity: integrity.into(),
            files: std::sync::Arc::new(
                [
                    (
                        "package.json".to_owned(),
                        std::sync::Arc::<[u8]>::from(manifest),
                    ),
                    (
                        "dist/types/signals.d.ts".to_owned(),
                        std::sync::Arc::<[u8]>::from(
                            &b"export declare function createTrackedEffect(): void;"[..],
                        ),
                    ),
                    (
                        "types/index.d.ts".to_owned(),
                        std::sync::Arc::<[u8]>::from(
                            &b"export declare function render(): void;"[..],
                        ),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            directories: std::sync::Arc::new(std::collections::BTreeSet::new()),
            root: root.into(),
            provenance_root: format!("{root}-archive"),
        }
    }

    const SIGNALS_INTEGRITY: &str = "sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==";
    const WEB_INTEGRITY: &str = "sha512-5ckKgOjem1pN5ADycOk6TjHmTtjbbN2fukqxo6RW3Oe3H7z0gaXWAdt8dLISto5/O4Nn8VxprFXFWpfy31+DUg==";
    const SOLID_JS_INTEGRITY: &str = "sha512-pmW6bRoTvfp/rN4jN7JmLvSaoIpFt7wm0Hi3j508S/smuJqUbRg3dQEjOPTkAwHW+McYnXrMG7cJ4AMNpLevtQ==";

    /// A call whose callee resolves into `@solidjs/signals`' installed tree.
    fn signals_call(export: &str, overrides: serde_json::Value) -> typefacts::ImplementationCall {
        let source_file = "/project/node_modules/@solidjs/signals/dist/types/signals.d.ts";
        let mut value = json!({
            "location": {"path": "/project/node_modules/consumer/dist/index.js", "startByte": 100, "endByte": 140},
            "reach": "reachable",
            "kind": "call",
            "target": format!("symbol:{export}"),
            "targetName": export,
            "targetModule": "solid-js",
            "declaration": {
                "symbol": format!("symbol:{export}"),
                "name": export,
                "kind": "FunctionDeclaration",
                "sourceFile": source_file,
                "location": {"path": source_file, "startByte": 10, "endByte": 20},
            },
        });
        let object = value.as_object_mut().expect("an object");
        for (key, replacement) in overrides.as_object().expect("an object") {
            if replacement.is_null() {
                object.remove(key);
            } else {
                object.insert(key.clone(), replacement.clone());
            }
        }
        serde_json::from_value(value).expect("a valid call")
    }

    /// The audited-tuple gate compares every field and names the one that
    /// disagreed, mirroring the lock replay's discipline.
    #[test]
    fn audited_archive_lookup_names_the_disagreeing_identity_field() {
        let manifest = audited_rc3_manifest("solidjs-signals");
        let exact = archive_snapshot(
            "@solidjs/signals",
            "2.0.0-rc.3",
            SIGNALS_INTEGRITY,
            &manifest,
            "/snapshot/signals",
        );
        assert_eq!(
            audited_archive_for_snapshot(&exact).map(|archive| archive.name),
            Ok("@solidjs/signals")
        );

        for (why, snapshot, expected) in [
            (
                "an unaudited package name",
                archive_snapshot(
                    "@solidjs/router",
                    "2.0.0-rc.3",
                    SIGNALS_INTEGRITY,
                    &manifest,
                    "/snapshot/router",
                ),
                AuditedArchiveDisagreement::Name,
            ),
            (
                "another prerelease of the audited archive",
                archive_snapshot(
                    "@solidjs/signals",
                    "2.0.0-rc.5",
                    SIGNALS_INTEGRITY,
                    &manifest,
                    "/snapshot/rc5",
                ),
                AuditedArchiveDisagreement::Version,
            ),
            (
                "the audited coordinate over other bytes",
                archive_snapshot(
                    "@solidjs/signals",
                    "2.0.0-rc.3",
                    "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==",
                    &manifest,
                    "/snapshot/mirror",
                ),
                AuditedArchiveDisagreement::Integrity,
            ),
            (
                "a rewritten package manifest",
                archive_snapshot(
                    "@solidjs/signals",
                    "2.0.0-rc.3",
                    SIGNALS_INTEGRITY,
                    b"{\"name\":\"@solidjs/signals\"}",
                    "/snapshot/rewritten",
                ),
                AuditedArchiveDisagreement::Manifest,
            ),
        ] {
            assert_eq!(
                audited_archive_for_snapshot(&snapshot).map(|archive| archive.name),
                Err(expected),
                "{why} must refuse and name its field"
            );
        }
    }

    /// The terminator, and every gate that must refuse it.
    ///
    /// The one that matters most is the last: `targetModule` says `solid-js`
    /// for every case here, and it is never what the gate consults. A callee
    /// whose declaration does not lie under an authenticated dependency root
    /// gets nothing, however the import was spelled.
    #[test]
    fn census_dialect_axiom_answers_only_for_an_exact_audited_dependency_archive() {
        let manifest = audited_rc3_manifest("solidjs-signals");
        let dependency = archive_snapshot(
            "@solidjs/signals",
            "2.0.0-rc.3",
            SIGNALS_INTEGRITY,
            &manifest,
            "/snapshot/signals",
        );
        let certified = archive_snapshot(
            "consumer",
            "1.0.0",
            "sha512-consumer",
            b"{\"name\":\"consumer\"}",
            "/snapshot/consumer",
        );
        fn root<'a>(
            snapshot: &'a super::super::ArtifactSnapshot,
            dependency: bool,
        ) -> SnapshotSourceRoot<'a> {
            SnapshotSourceRoot {
                path: "/project/node_modules/@solidjs/signals/".to_owned(),
                evidence_prefix: "/node_modules/@solidjs/signals/".to_owned(),
                snapshot,
                dependency,
            }
        }
        let roots = vec![root(&dependency, true)];
        let terminator = census_dialect_axiom_for_callee(
            &signals_call("createTrackedEffect", json!({})),
            solid_dialect::CallClaimDomain::Creates,
            ReachabilityFloor::MayExecute,
            &certified,
            &roots,
        )
        .expect("the audited archive denies createTrackedEffect's creates");
        assert_eq!(
            terminator.witness_site,
            "census-dialect-axiom:@solidjs/signals@2.0.0-rc.3#sha512-/yPhTf3xS1FRR4MX:createTrackedEffect:creates"
        );

        // Identity: the coordinate alone is never enough.
        for (why, snapshot) in [
            (
                "a mirror serving the coordinate over other bytes",
                archive_snapshot(
                    "@solidjs/signals",
                    "2.0.0-rc.3",
                    "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==",
                    &manifest,
                    "/snapshot/mirror",
                ),
            ),
            (
                "another prerelease",
                archive_snapshot(
                    "@solidjs/signals",
                    "2.0.0-rc.5",
                    SIGNALS_INTEGRITY,
                    &manifest,
                    "/snapshot/rc5",
                ),
            ),
            (
                "a rewritten manifest under the audited coordinate",
                archive_snapshot(
                    "@solidjs/signals",
                    "2.0.0-rc.3",
                    SIGNALS_INTEGRITY,
                    b"{\"name\":\"@solidjs/signals\"}",
                    "/snapshot/rewritten",
                ),
            ),
        ] {
            let roots = vec![root(&snapshot, true)];
            assert!(
                census_dialect_axiom_for_callee(
                    &signals_call("createTrackedEffect", json!({})),
                    solid_dialect::CallClaimDomain::Creates,
                    ReachabilityFloor::MayExecute,
                    &certified,
                    &roots,
                )
                .is_none(),
                "{why} must not receive the terminator"
            );
        }

        // The root has to be a dependency, and it has to be another archive.
        let owner = vec![root(&dependency, false)];
        assert!(
            census_dialect_axiom_for_callee(
                &signals_call("createTrackedEffect", json!({})),
                solid_dialect::CallClaimDomain::Creates,
                ReachabilityFloor::MayExecute,
                &certified,
                &owner,
            )
            .is_none(),
            "the artifact's own source root is not a callee root"
        );
        assert!(
            census_dialect_axiom_for_callee(
                &signals_call("createTrackedEffect", json!({})),
                solid_dialect::CallClaimDomain::Creates,
                ReachabilityFloor::MayExecute,
                &dependency,
                &roots,
            )
            .is_none(),
            "the archive under certification may not answer about itself"
        );

        // `targetModule` is `solid-js` on every one of these, and it never
        // reaches the table: the declaration is what is stripped.
        for (why, overrides) in [
            (
                "a declaration outside every authenticated root",
                json!({"declaration": {
                    "symbol": "symbol:createTrackedEffect",
                    "name": "createTrackedEffect",
                    "kind": "FunctionDeclaration",
                    "sourceFile": "/project/vendor/@solidjs/signals/dist/types/signals.d.ts",
                    "location": {"path": "/project/vendor/@solidjs/signals/dist/types/signals.d.ts", "startByte": 10, "endByte": 20},
                }}),
            ),
            (
                "a hoisted sibling installation",
                json!({"declaration": {
                    "symbol": "symbol:createTrackedEffect",
                    "name": "createTrackedEffect",
                    "kind": "FunctionDeclaration",
                    "sourceFile": "/other/node_modules/@solidjs/signals/dist/types/signals.d.ts",
                    "location": {"path": "/other/node_modules/@solidjs/signals/dist/types/signals.d.ts", "startByte": 10, "endByte": 20},
                }}),
            ),
            (
                "a declaration path under the root that the archive does not contain",
                json!({"declaration": {
                    "symbol": "symbol:createTrackedEffect",
                    "name": "createTrackedEffect",
                    "kind": "FunctionDeclaration",
                    "sourceFile": "/project/node_modules/@solidjs/signals/dist/types/injected.d.ts",
                    "location": {"path": "/project/node_modules/@solidjs/signals/dist/types/injected.d.ts", "startByte": 10, "endByte": 20},
                }}),
            ),
            ("no declaration at all", json!({"declaration": null})),
            ("no resolved symbol", json!({"target": ""})),
            ("an empty target name", json!({"targetName": ""})),
            (
                "an empty declaration source file",
                json!({"declaration": {
                    "symbol": "symbol:createTrackedEffect",
                    "name": "createTrackedEffect",
                    "kind": "FunctionDeclaration",
                    "sourceFile": "",
                    "location": {"path": "/project/node_modules/@solidjs/signals/dist/types/signals.d.ts", "startByte": 10, "endByte": 20},
                }}),
            ),
            (
                "a declaration naming another export",
                json!({"declaration": {
                    "symbol": "symbol:createTrackedEffect",
                    "name": "createEffect",
                    "kind": "FunctionDeclaration",
                    "sourceFile": "/project/node_modules/@solidjs/signals/dist/types/signals.d.ts",
                    "location": {"path": "/project/node_modules/@solidjs/signals/dist/types/signals.d.ts", "startByte": 10, "endByte": 20},
                }}),
            ),
        ] {
            let call = signals_call("createTrackedEffect", overrides);
            assert_eq!(call.target_module.as_ref(), "solid-js");
            assert!(
                census_dialect_axiom_for_callee(
                    &call,
                    solid_dialect::CallClaimDomain::Creates,
                    ReachabilityFloor::MayExecute,
                    &certified,
                    &roots,
                )
                .is_none(),
                "{why} must not receive the terminator"
            );
        }
    }

    /// Gate 7: the artifact under certification may not itself be an audited
    /// archive of *any* dialect, even when the callee resolves into a
    /// *different* audited archive. Certifying `solid-js@2.0.0-rc.3` with a
    /// callee resolving into `@solidjs/signals@2.0.0-rc.3` is the live
    /// phase-21 self-certification configuration ADR 0005 objection 5 is
    /// about — both audited, but different snapshots and different roots, so
    /// gate 6 (root equality) alone does not refuse it.
    #[test]
    fn census_dialect_axiom_refuses_when_the_certified_artifact_is_itself_audited() {
        let signals_manifest = audited_rc3_manifest("solidjs-signals");
        let dependency = archive_snapshot(
            "@solidjs/signals",
            "2.0.0-rc.3",
            SIGNALS_INTEGRITY,
            &signals_manifest,
            "/snapshot/signals",
        );
        let roots = vec![SnapshotSourceRoot {
            path: "/project/node_modules/@solidjs/signals/".to_owned(),
            evidence_prefix: "/node_modules/@solidjs/signals/".to_owned(),
            snapshot: &dependency,
            dependency: true,
        }];

        // `certified` equals the audited `solid-js@2.0.0-rc.3` tuple exactly
        // — a different snapshot and a different root than `dependency`.
        let solid_js_manifest = audited_rc3_manifest("solid-js");
        let certified_audited = archive_snapshot(
            "solid-js",
            "2.0.0-rc.3",
            SOLID_JS_INTEGRITY,
            &solid_js_manifest,
            "/snapshot/solid-js",
        );
        assert!(
            census_dialect_axiom_for_callee(
                &signals_call("createTrackedEffect", json!({})),
                solid_dialect::CallClaimDomain::Creates,
                ReachabilityFloor::MayExecute,
                &certified_audited,
                &roots,
            )
            .is_none(),
            "certifying one audited archive must not borrow another audited archive's negative rows"
        );

        // An ordinary consumer snapshot sits beside it, unaffected — the same
        // case `census_dialect_axiom_answers_only_for_an_exact_audited_dependency_archive`
        // pins as a positive.
        let certified_consumer = archive_snapshot(
            "consumer",
            "1.0.0",
            "sha512-consumer",
            b"{\"name\":\"consumer\"}",
            "/snapshot/consumer",
        );
        assert!(
            census_dialect_axiom_for_callee(
                &signals_call("createTrackedEffect", json!({})),
                solid_dialect::CallClaimDomain::Creates,
                ReachabilityFloor::MayExecute,
                &certified_consumer,
                &roots,
            )
            .is_some(),
            "a consumer snapshot must still receive the terminator"
        );
    }

    /// Gate 6's two arms are independent: a callee root can share the
    /// certified artifact's `provenance_root` without sharing its `root` —
    /// two different published coordinates materialized from one shared
    /// provenance origin — and either arm alone must refuse.
    #[test]
    fn census_dialect_axiom_refuses_on_the_provenance_root_arm_alone() {
        let manifest = audited_rc3_manifest("solidjs-signals");
        let dependency = super::super::ArtifactSnapshot {
            package_name: "@solidjs/signals".into(),
            package_version: "2.0.0-rc.3".into(),
            package_integrity: SIGNALS_INTEGRITY.into(),
            files: std::sync::Arc::new(
                [
                    (
                        "package.json".to_owned(),
                        std::sync::Arc::<[u8]>::from(manifest.as_slice()),
                    ),
                    (
                        "dist/types/signals.d.ts".to_owned(),
                        std::sync::Arc::<[u8]>::from(
                            &b"export declare function createTrackedEffect(): void;"[..],
                        ),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            directories: std::sync::Arc::new(std::collections::BTreeSet::new()),
            root: "/snapshot/signals".into(),
            provenance_root: "/shared-provenance-origin".into(),
        };
        let certified = super::super::ArtifactSnapshot {
            package_name: "consumer".into(),
            package_version: "1.0.0".into(),
            package_integrity: "sha512-consumer".into(),
            files: std::sync::Arc::new(std::collections::BTreeMap::new()),
            directories: std::sync::Arc::new(std::collections::BTreeSet::new()),
            root: "/snapshot/consumer".into(),
            provenance_root: "/shared-provenance-origin".into(),
        };
        assert_ne!(dependency.root(), certified.root());
        assert_eq!(dependency.provenance_root(), certified.provenance_root());
        let roots = vec![SnapshotSourceRoot {
            path: "/project/node_modules/@solidjs/signals/".to_owned(),
            evidence_prefix: "/node_modules/@solidjs/signals/".to_owned(),
            snapshot: &dependency,
            dependency: true,
        }];
        assert!(
            census_dialect_axiom_for_callee(
                &signals_call("createTrackedEffect", json!({})),
                solid_dialect::CallClaimDomain::Creates,
                ReachabilityFloor::MayExecute,
                &certified,
                &roots,
            )
            .is_none(),
            "a shared provenance root alone must refuse even though root() differs"
        );
    }

    /// The export has to be a canonical primitive the audited document
    /// actually covers.
    #[test]
    fn census_dialect_axiom_refuses_a_non_canonical_or_unaudited_export() {
        let manifest = audited_rc3_manifest("solidjs-signals");
        let dependency = archive_snapshot(
            "@solidjs/signals",
            "2.0.0-rc.3",
            SIGNALS_INTEGRITY,
            &manifest,
            "/snapshot/signals",
        );
        let certified = archive_snapshot(
            "consumer",
            "1.0.0",
            "sha512-consumer",
            b"{\"name\":\"consumer\"}",
            "/snapshot/consumer",
        );
        let roots = vec![SnapshotSourceRoot {
            path: "/project/node_modules/@solidjs/signals/".to_owned(),
            evidence_prefix: "/node_modules/@solidjs/signals/".to_owned(),
            snapshot: &dependency,
            dependency: true,
        }];
        for (why, export) in [
            // A real export of the audited archive whose audited summary
            // closes `creates: []` — but no dialect models the name, so the
            // census has no primitive identity to terminate on.
            ("a non-canonical export the audit closes", "isEqual"),
            // A canonical primitive of the archive with neither an audited
            // summary nor a hand implementation census. `createSignal` used to
            // stand here and no longer can: the 2026-09-04 census closed it,
            // along with `createRoot`, `getOwner`, `onCleanup` and `untrack`.
            ("an export outside the audited document", "createContext"),
            ("an export of no dialect at all", "notAPrimitive"),
        ] {
            assert!(
                census_dialect_axiom_for_callee(
                    &signals_call(export, json!({})),
                    solid_dialect::CallClaimDomain::Creates,
                    ReachabilityFloor::MayExecute,
                    &certified,
                    &roots,
                )
                .is_none(),
                "{why} must not receive the terminator"
            );
        }
    }

    /// `render` publishes a `create`, so its `creates` question has no
    /// negative answer; its audited sibling `clientOnly` does.
    ///
    /// The per-domain half of this — the same export answering `None` for
    /// `creates` and `Some` for a domain it does close — is not writable yet:
    /// the table ships only `CallClaimDomain::Creates`, and the seven other
    /// kinded domains are withheld with their reasons in
    /// `solid-dialect`'s `NEGATIVE_ROWS`. `render`'s closed domains are
    /// `reads`, `writes`, `invalidates`, `returns` and `cleanups`, all of them
    /// in that withheld set, so there is no domain it both closes and this
    /// table admits.
    #[test]
    fn census_dialect_axiom_refuses_an_export_that_publishes_the_domains_operation() {
        let manifest = audited_rc3_manifest("solidjs-web");
        let dependency = archive_snapshot(
            "@solidjs/web",
            "2.0.0-rc.3",
            WEB_INTEGRITY,
            &manifest,
            "/snapshot/web",
        );
        let certified = archive_snapshot(
            "consumer",
            "1.0.0",
            "sha512-consumer",
            b"{\"name\":\"consumer\"}",
            "/snapshot/consumer",
        );
        let roots = vec![SnapshotSourceRoot {
            path: "/project/node_modules/@solidjs/web/".to_owned(),
            evidence_prefix: "/node_modules/@solidjs/web/".to_owned(),
            snapshot: &dependency,
            dependency: true,
        }];
        let call = |export: &str| -> typefacts::ImplementationCall {
            let source_file = "/project/node_modules/@solidjs/web/types/index.d.ts";
            serde_json::from_value(json!({
                "location": {"path": "/project/node_modules/consumer/dist/index.js", "startByte": 1, "endByte": 9},
                "reach": "unknown",
                "kind": "call",
                "target": format!("symbol:{export}"),
                "targetName": export,
                "targetModule": "@solidjs/web",
                "declaration": {
                    "symbol": format!("symbol:{export}"),
                    "name": export,
                    "kind": "FunctionDeclaration",
                    "sourceFile": source_file,
                    "location": {"path": source_file, "startByte": 10, "endByte": 20},
                },
            }))
            .expect("a valid call")
        };
        let terminator = |export: &str| {
            census_dialect_axiom_for_callee(
                &call(export),
                solid_dialect::CallClaimDomain::Creates,
                ReachabilityFloor::MayExecute,
                &certified,
                &roots,
            )
        };
        assert!(terminator("render").is_none());
        // Withheld because rc.3's `hydrate` reaches `render`, whose first
        // statement is `registerDelegatedRoot`; the audit's own `creates: []`
        // for it is not followed.
        assert!(terminator("hydrate").is_none());
        assert_eq!(
            terminator("clientOnly")
                .expect("clientOnly's creates is denied in both audited conditions")
                .witness_site,
            "census-dialect-axiom:@solidjs/web@2.0.0-rc.3#sha512-5ckKgOjem1pN5ADy:clientOnly:creates"
        );
    }

    /// A closed domain asserts a zero upper bound, so the premise is the
    /// `MayExecute` floor and nothing stronger — and an absent call kind is
    /// never read as a call.
    #[test]
    fn census_dialect_axiom_holds_only_at_the_may_execute_floor() {
        let manifest = audited_rc3_manifest("solidjs-signals");
        let dependency = archive_snapshot(
            "@solidjs/signals",
            "2.0.0-rc.3",
            SIGNALS_INTEGRITY,
            &manifest,
            "/snapshot/signals",
        );
        let certified = archive_snapshot(
            "consumer",
            "1.0.0",
            "sha512-consumer",
            b"{\"name\":\"consumer\"}",
            "/snapshot/consumer",
        );
        let roots = vec![SnapshotSourceRoot {
            path: "/project/node_modules/@solidjs/signals/".to_owned(),
            evidence_prefix: "/node_modules/@solidjs/signals/".to_owned(),
            snapshot: &dependency,
            dependency: true,
        }];
        let answer = |overrides: serde_json::Value, floor: ReachabilityFloor| {
            census_dialect_axiom_for_callee(
                &signals_call("createTrackedEffect", overrides),
                solid_dialect::CallClaimDomain::Creates,
                floor,
                &certified,
                &roots,
            )
        };
        assert!(answer(json!({}), ReachabilityFloor::MayExecute).is_some());
        // An `unknown`-reach call is in the census at this floor.
        assert!(answer(json!({"reach": "unknown"}), ReachabilityFloor::MayExecute).is_some());
        assert!(
            answer(json!({}), ReachabilityFloor::Reachable).is_none(),
            "a lower bound above zero is not something a negative table can answer"
        );
        assert!(
            answer(
                json!({"reach": "unreachable"}),
                ReachabilityFloor::MayExecute
            )
            .is_none()
        );
        assert!(answer(json!({"kind": null}), ReachabilityFloor::MayExecute).is_none());
        // A construction runs the callee too.
        assert!(answer(json!({"kind": "construct"}), ReachabilityFloor::MayExecute).is_some());
    }

    // ----------------------------------------------------------------------
    // The `creates` implementation census: one disposition per call
    // ----------------------------------------------------------------------

    /// An implementation transcript the census can walk: complete, with a
    /// resolved declaration inside the consumer's own runtime and the calls
    /// supplied, and every other field at its default.
    fn census_transcript_with(
        calls: Vec<typefacts::ImplementationCall>,
        uncensused_invoking_forms: serde_json::Value,
    ) -> typefacts::ExportImplementationTranscript {
        let source = "/project/node_modules/consumer/dist/index.js";
        let mut value = json!({
            "location": {"path": source, "startByte": 0, "endByte": 400},
            "queryName": "useThing",
            "target": "symbol:useThing",
            "declaration": {
                "symbol": "symbol:useThing",
                "name": "useThing",
                "kind": "FunctionDeclaration",
                "sourceFile": source,
                "location": {"path": source, "startByte": 16, "endByte": 24},
            },
            "controlFlow": {},
            "uncensusedInvokingForms": uncensused_invoking_forms,
            "complete": true,
        });
        let mut transcript: typefacts::ExportImplementationTranscript =
            serde_json::from_value(value.take()).expect("a valid transcript");
        transcript.calls = calls;
        transcript
    }

    fn census_run<'a>(
        certified: &'a super::super::ArtifactSnapshot,
        roots: &'a [SnapshotSourceRoot<'a>],
    ) -> CensusRun<'a> {
        CensusRun {
            certified,
            evidence: CensusEvidence { roots, locals: &[] },
            // The consumer's runtime module, which is where every synthesized
            // transcript below declares itself; a test that wants the
            // declaration-file refusal clears this.
            runtime_sources: std::iter::once("dist/index.js".to_owned()).collect(),
            visited: Vec::new(),
            requested: Vec::new(),
            sites: Vec::new(),
            calls: 0,
            deepest: 0,
            sources: std::collections::BTreeMap::new(),
            frame: None,
        }
    }

    /// The consumer's runtime module every synthesized transcript declares
    /// itself in: `useThing` sits at bytes 16..24, which is the declaration
    /// [`census_transcript_with`] names, and the body contains no jump.
    const CONSUMER_SOURCE: &str =
        "export function useThing(items, callback) {\n  return callback(items);\n}\n";

    fn consumer_snapshot() -> super::super::ArtifactSnapshot {
        consumer_snapshot_with(CONSUMER_SOURCE)
    }

    /// A consumer archive whose `dist/index.js` is `source`, so the census can
    /// bind declarations and read jumps and writes from authenticated bytes.
    fn consumer_snapshot_with(source: &str) -> super::super::ArtifactSnapshot {
        super::super::ArtifactSnapshot {
            package_name: "consumer".into(),
            package_version: "1.0.0".into(),
            package_integrity: "sha512-consumer".into(),
            files: std::sync::Arc::new(
                [
                    (
                        "package.json".to_owned(),
                        std::sync::Arc::<[u8]>::from(&b"{\"name\":\"consumer\"}"[..]),
                    ),
                    (
                        "dist/index.js".to_owned(),
                        std::sync::Arc::<[u8]>::from(source.as_bytes()),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            directories: std::sync::Arc::new(std::collections::BTreeSet::new()),
            root: "/snapshot/consumer".into(),
            provenance_root: "/snapshot/consumer".into(),
        }
    }

    /// The consumer's own (non-dependency) source root, which is what lets the
    /// census attribute a transcript's declaration to the artifact under
    /// certification.
    fn consumer_root<'a>(certified: &'a super::super::ArtifactSnapshot) -> SnapshotSourceRoot<'a> {
        SnapshotSourceRoot {
            path: "/project/node_modules/consumer/".to_owned(),
            evidence_prefix: "/node_modules/consumer/".to_owned(),
            snapshot: certified,
            dependency: false,
        }
    }

    fn signals_root<'a>(dependency: &'a super::super::ArtifactSnapshot) -> SnapshotSourceRoot<'a> {
        SnapshotSourceRoot {
            path: "/project/node_modules/@solidjs/signals/".to_owned(),
            evidence_prefix: "/node_modules/@solidjs/signals/".to_owned(),
            snapshot: dependency,
            dependency: true,
        }
    }

    /// Oxc excludes the `export` modifier from a function node while
    /// typescript-go includes it in the FunctionDeclaration demand range. The
    /// verifier binds the two parsers through the direct export declaration,
    /// without changing the ordinary local-function range.
    #[test]
    fn probe_creates_census_demands_the_typescript_span_for_an_exported_local_helper() {
        let source = "export function caller() { return helper(); }\n\
                      export function helper() { return 1; }";
        let offset = |needle: &str| u64::try_from(source.find(needle).unwrap()).unwrap();
        let helper_export = offset("export function helper");
        let helper_function = offset("function helper");
        let helper_name = offset("helper() { return 1");
        let certified = consumer_snapshot_with(source);
        let roots = vec![consumer_root(&certified)];
        let mut run = census_run(&certified, &roots);
        let declaration: typefacts::ResolvedDeclaration = serde_json::from_value(json!({
            "symbol": "symbol:helper",
            "name": "helper",
            "kind": "FunctionDeclaration",
            "sourceFile": "/project/node_modules/consumer/dist/index.js",
            "location": {
                "path": "/project/node_modules/consumer/dist/index.js",
                "startByte": helper_name,
                "endByte": helper_name + 6,
            },
        }))
        .expect("a valid declaration");

        let demanded = census_local_declaration_node(&mut run, "dist/index.js", &declaration)
            .expect("the exported helper binds to one declaration");
        assert_eq!(demanded.start_byte, helper_export);
        assert_eq!(demanded.end_byte, u64::try_from(source.len()).unwrap());
        assert_ne!(demanded.start_byte, helper_function);
        census_local_binding_is_stable(&mut run, "dist/index.js", &demanded, &declaration)
            .expect("the same expanded demand still binds the Oxc function node");
    }

    /// Every call the `MayExecute` floor admits gets exactly one disposition
    /// and one witness site, in the order the census tries them; the dialect
    /// disposition carries the tier's own site. A synthesized root matching the
    /// audited `@solidjs/signals@2.0.0-rc.3` tuple is what lets the tier
    /// answer, exactly as the tier's own tests bind it.
    #[test]
    fn creates_census_dispositions_every_admitted_call_by_its_callee() {
        let manifest = audited_rc3_manifest("solidjs-signals");
        let dependency = archive_snapshot(
            "@solidjs/signals",
            "2.0.0-rc.3",
            SIGNALS_INTEGRITY,
            &manifest,
            "/snapshot/signals",
        );
        let certified = consumer_snapshot();
        let roots = vec![consumer_root(&certified), signals_root(&dependency)];
        let lib = "/toolchain/lib/lib.es5.d.ts";
        let implementation = census_transcript_with(
            vec![
                // dialect-axiom, at an *unknown* reach: admitted by the floor and
                // dispositioned like any other.
                signals_call("createTrackedEffect", json!({"reach": "unknown"})),
                // parameter-rooted: the callee is proven to be parameter 0.
                signals_call(
                    "callback",
                    json!({
                        "location": {"path": "/project/node_modules/consumer/dist/index.js", "startByte": 150, "endByte": 160},
                        "target": "",
                        "targetModule": "",
                        "declaration": null,
                        "calleeParameter": {"parameterIndex": 0},
                    }),
                ),
                // standard-library: resolved by default-library symbol identity.
                signals_call(
                    "map",
                    json!({
                        "location": {"path": "/project/node_modules/consumer/dist/index.js", "startByte": 170, "endByte": 190},
                        "targetModule": "",
                        "declaration": {
                            "symbol": "symbol:Array.map",
                            "name": "map",
                            "kind": "MethodSignature",
                            "sourceFile": lib,
                            "location": {"path": lib, "startByte": 10, "endByte": 20},
                            "standardLibrary": true,
                        },
                    }),
                ),
                // unreachable: excused by the floor, and still recorded.
                signals_call(
                    "never",
                    json!({
                        "location": {"path": "/project/node_modules/consumer/dist/index.js", "startByte": 200, "endByte": 210},
                        "reach": "unreachable",
                        "target": "",
                        "targetModule": "",
                        "declaration": null,
                    }),
                ),
                // parameter-rooted-accessor (ADR 0034): `Object.prototype
                // .toString.call(value)` — the by-reference owner, a reviewed
                // this-protocol receiver, and a `this` rooted at parameter 0.
                signals_call(
                    "call",
                    json!({
                        "location": {"path": "/project/node_modules/consumer/dist/index.js", "startByte": 220, "endByte": 250},
                        "targetModule": "",
                        "declaration": {
                            "symbol": "symbol:CallableFunction.call",
                            "name": "call",
                            "qualifiedName": "CallableFunction.call",
                            "kind": "MethodSignature",
                            "sourceFile": lib,
                            "location": {"path": lib, "startByte": 30, "endByte": 34},
                            "standardLibrary": true,
                        },
                        "callReceiver": {
                            "symbol": "symbol:Object.toString",
                            "name": "toString",
                            "qualifiedName": "Object.toString",
                            "kind": "MethodSignature",
                            "sourceFile": lib,
                            "location": {"path": lib, "startByte": 40, "endByte": 48},
                            "standardLibrary": true,
                        },
                        "thisParameter": 0,
                    }),
                ),
            ],
            // parameter-rooted-accessor (ADR 0034): a read whose subject the
            // producer rooted at parameter 0 is dispositioned, not refused.
            json!([{
                "kind": "property-access-unknown-accessor",
                "nodeKind": "ElementAccessExpression",
                "location": {"path": "/project/node_modules/consumer/dist/index.js", "startByte": 260, "endByte": 270},
                "reach": "reachable",
                "subjectParameter": 0,
            }]),
        );
        let mut run = census_run(&certified, &roots);
        assert_eq!(
            census_transcript(&mut run, &implementation, 0),
            Ok(CensusStep::Decided)
        );
        assert_eq!(run.calls, 5);
        run.sites.sort();
        assert_eq!(
            run.sites,
            vec![
                "census-call:/project/node_modules/consumer/dist/index.js:150:160:call:reachable:parameter-rooted",
                "census-call:/project/node_modules/consumer/dist/index.js:170:190:call:reachable:standard-library",
                "census-call:/project/node_modules/consumer/dist/index.js:200:210:call:unreachable:unreachable",
                "census-call:/project/node_modules/consumer/dist/index.js:220:250:call:reachable:parameter-rooted-accessor",
                "census-dialect-axiom:@solidjs/signals@2.0.0-rc.3#sha512-/yPhTf3xS1FRR4MX:createTrackedEffect:creates",
                "census-form:/project/node_modules/consumer/dist/index.js:260:270:property-access-unknown-accessor:reachable:parameter-rooted-accessor",
            ]
        );
    }

    /// ADR 0035: the `returns` census reads the implementation's completion
    /// form and its return sites, and nothing else. Every premise refuses by
    /// name; a value-carrying site is admitted only when the producer proved it
    /// unreachable.
    #[test]
    fn returns_census_admits_only_bare_or_unreachable_completions() {
        let transcript = |form: serde_json::Value,
                          returns: serde_json::Value,
                          incompleteness: serde_json::Value|
         -> typefacts::ExportImplementationTranscript {
            let mut value = json!({
                "location": {"path": "/project/index.js", "startByte": 0, "endByte": 40},
                "controlFlow": {"returns": returns}
            });
            if !form.is_null() {
                value["completionForm"] = form;
            }
            if !incompleteness.is_null() {
                value["controlFlow"]["unsupported"] = json!(["iterationReachability"]);
                value["controlFlow"]["incompleteness"] = incompleteness;
            }
            serde_json::from_value(value).unwrap()
        };
        let site = |start: u64, end: u64, reach: &str, value: bool| {
            let mut row = json!({
                "location": {"path": "/project/index.js", "startByte": start, "endByte": end},
                "reach": reach
            });
            if value {
                row["value"] = json!({
                    "callability": "nonCallable",
                    "constructability": "nonConstructable",
                    "primitive": {"mayBeNumber": true}
                });
            }
            row
        };

        // Admitted: no return site at all, bare returns at every reach, and a
        // value-carrying return the producer proved unreachable.
        assert_eq!(
            census_returns_transcript(&transcript(json!("plain"), json!([]), json!(null))).unwrap(),
            vec!["census-returns-total:0".to_owned()]
        );
        let admitted = census_returns_transcript(&transcript(
            json!("plain"),
            json!([
                site(10, 17, "reachable", false),
                site(20, 27, "unknown", false),
                site(30, 39, "unreachable", true)
            ]),
            json!(null),
        ))
        .unwrap();
        assert_eq!(
            admitted,
            vec![
                "census-return:/project/index.js:10:17:reachable:bare".to_owned(),
                "census-return:/project/index.js:20:27:unknown:bare".to_owned(),
                "census-return:/project/index.js:30:39:unreachable:value-unreachable".to_owned(),
                "census-returns-total:3".to_owned(),
            ]
        );
        // A bare return under a lower-bound-only construct is admitted.
        census_returns_transcript(&transcript(
            json!("plain"),
            json!([site(10, 17, "unknown", false)]),
            json!([{"marker": "iterationReachability", "class": "reachability-lower-bound",
                    "location": {"path": "/project/index.js", "startByte": 5, "endByte": 30}}]),
        ))
        .unwrap();

        // Refused, each by name.
        for (form, returns, incompleteness, needle) in [
            (
                json!(null),
                json!([]),
                json!(null),
                "states no completion form",
            ),
            (
                json!("unclassified"),
                json!([]),
                json!(null),
                "did not classify",
            ),
            (
                json!("async"),
                json!([]),
                json!(null),
                "async implementation",
            ),
            (
                json!("generator"),
                json!([]),
                json!(null),
                "generator implementation",
            ),
            (
                json!("async-generator"),
                json!([]),
                json!(null),
                "async generator implementation",
            ),
            (
                json!("plain"),
                json!([site(10, 17, "reachable", true)]),
                json!(null),
                "value-carrying completion at /project/index.js:10..17, reach reachable",
            ),
            (
                json!("plain"),
                json!([site(10, 17, "unknown", true)]),
                json!(null),
                "value-carrying completion at /project/index.js:10..17, reach unknown",
            ),
            (
                json!("plain"),
                json!([]),
                json!([{"marker": "iterationReachability", "class": "flow-unaccounted",
                        "location": {"path": "/project/index.js", "startByte": 5, "endByte": 30}}]),
                "cannot account for a construct (iterationReachability, flow-unaccounted)",
            ),
        ] {
            let refusal = census_returns_transcript(&transcript(form, returns, incompleteness))
                .expect_err("the returns census must refuse this transcript");
            assert!(refusal.contains(needle), "{refusal}");
        }
        // No control-flow census at all is an absence, not zero returns.
        let mut absent: typefacts::ExportImplementationTranscript = serde_json::from_value(json!({
            "location": {"path": "/project/index.js", "startByte": 0, "endByte": 40},
            "completionForm": "plain"
        }))
        .unwrap();
        absent.control_flow = None;
        assert!(
            census_returns_transcript(&absent)
                .expect_err("no census")
                .contains("no control-flow census")
        );
    }

    /// ADR 0034's boundary, form by form and call by call: a stated subject
    /// parameter admits only the two read-accessor kinds on a property or
    /// element access; a `.call` admits only a reviewed this-protocol receiver
    /// with a rooted `this`.
    #[test]
    fn creates_census_parameter_rooted_accessor_stops_at_its_stated_boundary() {
        let certified = consumer_snapshot();
        let roots = vec![consumer_root(&certified)];
        let lib = "/toolchain/lib/lib.es5.d.ts";
        let form = |kind: &str, node_kind: &str, subject: serde_json::Value| {
            json!([{
                "kind": kind,
                "nodeKind": node_kind,
                "location": {"path": "/project/node_modules/consumer/dist/index.js", "startByte": 260, "endByte": 270},
                "reach": "reachable",
                "subjectParameter": subject,
            }])
        };
        // Admitted: both read-accessor kinds, on either access node kind.
        for (kind, node_kind) in [
            ("get-accessor", "PropertyAccessExpression"),
            (
                "property-access-unknown-accessor",
                "PropertyAccessExpression",
            ),
            (
                "property-access-unknown-accessor",
                "ElementAccessExpression",
            ),
        ] {
            assert_eq!(
                census_transcript(
                    &mut census_run(&certified, &roots),
                    &census_transcript_with(vec![], form(kind, node_kind, json!(1))),
                    0,
                ),
                Ok(CensusStep::Decided),
                "{kind} on {node_kind}"
            );
        }
        // Refused: a rooted subject on any other kind or node, and an unrooted
        // read accessor.
        for (kind, node_kind, subject) in [
            ("set-accessor", "PropertyAccessExpression", json!(0)),
            ("iteration-protocol", "ForOfStatement", json!(0)),
            ("coercion", "BinaryExpression", json!(0)),
            ("instanceof", "BinaryExpression", json!(0)),
            (
                "property-access-unknown-accessor",
                "SpreadAssignment",
                json!(0),
            ),
            (
                "property-access-unknown-accessor",
                "BindingElement",
                json!(0),
            ),
            (
                "property-access-unknown-accessor",
                "PropertyAccessExpression",
                serde_json::Value::Null,
            ),
            (
                "get-accessor",
                "PropertyAccessExpression",
                serde_json::Value::Null,
            ),
        ] {
            let refusal = census_transcript(
                &mut census_run(&certified, &roots),
                &census_transcript_with(vec![], form(kind, node_kind, subject)),
                0,
            )
            .expect_err("a rooted subject on an unadmitted kind or node, or an unrooted read accessor, refuses");
            assert!(
                refusal.contains("refuses an uncensused invoking form"),
                "{kind} on {node_kind}: {refusal}"
            );
        }
        for (kind, node_kind, subject) in [
            ("set-accessor", "PropertyAccessExpression", json!(0)),
            (
                "property-access-unknown-accessor",
                "SpreadAssignment",
                json!(0),
            ),
            (
                "get-accessor",
                "PropertyAccessExpression",
                serde_json::Value::Null,
            ),
        ] {
            let refusal = census_transcript(
                &mut census_run(&certified, &roots),
                &census_transcript_with(vec![], form(kind, node_kind, subject)),
                0,
            )
            .expect_err("refuses");
            assert!(
                refusal.contains(&format!("uncensused invoking form: {kind}")),
                "{refusal}"
            );
        }

        let call_of = |receiver: serde_json::Value, this_parameter: serde_json::Value| {
            signals_call(
                "call",
                json!({
                    "location": {"path": "/project/node_modules/consumer/dist/index.js", "startByte": 220, "endByte": 250},
                    "targetModule": "",
                    "declaration": {
                        "symbol": "symbol:CallableFunction.call",
                        "name": "call",
                        "qualifiedName": "CallableFunction.call",
                        "kind": "MethodSignature",
                        "sourceFile": lib,
                        "location": {"path": lib, "startByte": 30, "endByte": 34},
                        "standardLibrary": true,
                    },
                    "callReceiver": receiver,
                    "thisParameter": this_parameter,
                }),
            )
        };
        let library_receiver = |qualified: &str| {
            json!({
                "symbol": format!("symbol:{qualified}"),
                "name": qualified.rsplit('.').next().unwrap(),
                "qualifiedName": qualified,
                "kind": "MethodSignature",
                "sourceFile": lib,
                "location": {"path": lib, "startByte": 40, "endByte": 48},
                "standardLibrary": true,
            })
        };
        // A reviewed receiver whose `this` is not rooted refuses by name.
        let refusal = census_transcript(
            &mut census_run(&certified, &roots),
            &census_transcript_with(
                vec![call_of(
                    library_receiver("Object.toString"),
                    serde_json::Value::Null,
                )],
                json!([]),
            ),
            0,
        )
        .expect_err("an unrooted this refuses");
        assert!(
            refusal.contains("this-protocol member `Object.toString`")
                && refusal.contains("not rooted at an unwritten parameter"),
            "{refusal}"
        );
        // A library receiver outside the reviewed table refuses as the
        // by-reference transfer it is, rooted `this` or not.
        let refusal = census_transcript(
            &mut census_run(&certified, &roots),
            &census_transcript_with(
                vec![call_of(library_receiver("Array.slice"), json!(0))],
                json!([]),
            ),
            0,
        )
        .expect_err("an unreviewed receiver refuses");
        assert!(
            refusal.contains("transfers control to a callable by reference"),
            "{refusal}"
        );
        // A receiver that is not a default-library member refuses the same way.
        let refusal = census_transcript(
            &mut census_run(&certified, &roots),
            &census_transcript_with(
                vec![call_of(
                    json!({
                        "symbol": "symbol:helper",
                        "name": "helper",
                        "kind": "FunctionDeclaration",
                        "sourceFile": "/project/node_modules/consumer/dist/index.js",
                        "location": {"path": "/project/node_modules/consumer/dist/index.js", "startByte": 5, "endByte": 11},
                    }),
                    json!(0),
                )],
                json!([]),
            ),
            0,
        )
        .expect_err("a local receiver refuses");
        assert!(
            refusal.contains("transfers control to a callable by reference"),
            "{refusal}"
        );
        // No receiver stated at all: the old behavior, unchanged.
        let refusal = census_transcript(
            &mut census_run(&certified, &roots),
            &census_transcript_with(vec![call_of(serde_json::Value::Null, json!(0))], json!([])),
            0,
        )
        .expect_err("a receiverless by-reference transfer refuses");
        assert!(
            refusal.contains("transfers control to a callable by reference"),
            "{refusal}"
        );
    }

    /// What has no disposition refuses the domain by name, and the tier does
    /// not answer about an audited archive's own certification.
    #[test]
    fn creates_census_refuses_by_name_what_no_disposition_covers() {
        let manifest = audited_rc3_manifest("solidjs-signals");
        let dependency = archive_snapshot(
            "@solidjs/signals",
            "2.0.0-rc.3",
            SIGNALS_INTEGRITY,
            &manifest,
            "/snapshot/signals",
        );
        let certified = consumer_snapshot();
        let roots = vec![consumer_root(&certified), signals_root(&dependency)];

        // An untraced callee is not a parameter-rooted one: an empty
        // `calleeSources` and no `calleeParameter` is unresolved.
        let unresolved = census_transcript_with(
            vec![signals_call(
                "callback",
                json!({"target": "", "targetModule": "", "declaration": null}),
            )],
            json!([]),
        );
        let refusal = census_transcript(&mut census_run(&certified, &roots), &unresolved, 0)
            .expect_err("an unresolved callee refuses");
        assert!(
            refusal.contains("refuses an unresolved callee") && refusal.contains("callback"),
            "{refusal}"
        );

        // A dependency export with no audited negative row, in a root that is
        // not this artifact's own runtime, is neither local nor excused.
        let unaudited = census_transcript_with(vec![signals_call("render", json!({}))], json!([]));
        let refusal = census_transcript(&mut census_run(&certified, &roots), &unaudited, 0)
            .expect_err("an unaudited dependency callee refuses");
        assert!(
            refusal.contains("neither a default-library member") && refusal.contains("\"render\""),
            "{refusal}"
        );

        // The artifact under certification being some other archive than the
        // one the transcript declares itself in refuses before any call is
        // dispositioned: the census walks this artifact's authenticated runtime
        // bytes and nothing else. (That the tier refuses to answer about the
        // audited archive's own certification is pinned by the tier's tests.)
        let refusal = census_transcript(
            &mut census_run(&dependency, &roots),
            &census_transcript_with(
                vec![signals_call("createTrackedEffect", json!({}))],
                json!([]),
            ),
            0,
        )
        .expect_err("a transcript outside the certified artifact's runtime source refuses");
        assert!(
            refusal.contains("not in this artifact's own runtime source"),
            "{refusal}"
        );

        // An uncensused invoking form at the floor refuses before any call is
        // dispositioned, by kind and location; an unreachable one is excused.
        let forms = |reach: &str| {
            json!([{
                "kind": "tagged-template",
                "nodeKind": "TaggedTemplateExpression",
                "location": {"path": "/project/node_modules/consumer/dist/index.js", "startByte": 300, "endByte": 320},
                "reach": reach,
            }])
        };
        let refusal = census_transcript(
            &mut census_run(&certified, &roots),
            &census_transcript_with(vec![], forms("unknown")),
            0,
        )
        .expect_err("an uncensused form at the floor refuses");
        assert!(
            refusal.contains("uncensused invoking form: tagged-template")
                && refusal.contains(":300..320"),
            "{refusal}"
        );
        assert_eq!(
            census_transcript(
                &mut census_run(&certified, &roots),
                &census_transcript_with(vec![], forms("unreachable")),
                0,
            ),
            Ok(CensusStep::Decided)
        );

        // A call of unknown kind — an absent `kind` on the wire — refuses.
        let refusal = census_transcript(
            &mut census_run(&certified, &roots),
            &census_transcript_with(
                vec![signals_call("createTrackedEffect", json!({"kind": null}))],
                json!([]),
            ),
            0,
        )
        .expect_err("an absent call kind is never read as a call");
        assert!(refusal.contains("call of unknown kind"), "{refusal}");
    }

    /// A callee declared in this artifact's own runtime source is a local
    /// recursion: without its transcript the census asks for it and decides
    /// nothing; with it the census recurses, and a revisit closes the finite
    /// graph while every other edge remains subject to its ordinary premise.
    #[test]
    fn probe_creates_census_recurses_into_local_declarations_and_closes_a_cycle() {
        let source_path = "/project/node_modules/consumer/dist/index.js";
        // Real runtime bytes, because the census binds a local declaration to
        // its *node* by parsing them: the producer resolves `helper` to its
        // identifier, and only the exact node span is a demand it answers.
        let source = "export function useThing(callback) {\n  return helper(callback);\n}\n\
                      function helper(callback) {\n  callback();\n}";
        let offset = |needle: &str| u64::try_from(source.find(needle).unwrap()).unwrap();
        let export_name = (offset("useThing"), offset("useThing") + 8);
        let helper_node = (
            offset("function helper"),
            u64::try_from(source.len()).unwrap(),
        );
        let helper_name = (
            offset("function helper") + 9,
            offset("function helper") + 15,
        );
        let call_helper_at = (offset("helper(callback)"), offset("helper(callback)") + 16);
        let call_back_at = (offset("  callback();") + 2, offset("  callback();") + 12);
        let certified = super::super::ArtifactSnapshot {
            package_name: "consumer".into(),
            package_version: "1.0.0".into(),
            package_integrity: "sha512-consumer".into(),
            files: std::sync::Arc::new(
                [
                    (
                        "package.json".to_owned(),
                        std::sync::Arc::<[u8]>::from(&b"{\"name\":\"consumer\"}"[..]),
                    ),
                    (
                        "dist/index.js".to_owned(),
                        std::sync::Arc::<[u8]>::from(source.as_bytes()),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            directories: std::sync::Arc::new(std::collections::BTreeSet::new()),
            root: "/snapshot/consumer".into(),
            provenance_root: "/snapshot/consumer".into(),
        };
        let roots = vec![SnapshotSourceRoot {
            path: "/project/node_modules/consumer/".to_owned(),
            evidence_prefix: "/node_modules/consumer/".to_owned(),
            snapshot: &certified,
            dependency: false,
        }];
        let helper_location = typefacts::Location {
            path: source_path.into(),
            start_byte: helper_node.0,
            end_byte: helper_node.1,
        };
        let call_helper = signals_call(
            "helper",
            json!({
                "location": {"path": source_path, "startByte": call_helper_at.0, "endByte": call_helper_at.1},
                "targetModule": "",
                "declaration": {
                    "symbol": "symbol:helper",
                    "name": "helper",
                    "kind": "FunctionDeclaration",
                    "sourceFile": source_path,
                    "location": {"path": source_path, "startByte": helper_name.0, "endByte": helper_name.1},
                },
            }),
        );
        let mut export = census_transcript_with(vec![call_helper.clone()], json!([]));
        export.declaration = Some(
            serde_json::from_value(json!({
                "symbol": "symbol:useThing",
                "name": "useThing",
                "kind": "FunctionDeclaration",
                "sourceFile": source_path,
                "location": {"path": source_path, "startByte": export_name.0, "endByte": export_name.1},
            }))
            .expect("a valid declaration"),
        );

        // Without the helper's transcript: no verdict, and exactly one request,
        // for the helper's whole declaration node rather than its identifier.
        let mut run = census_run(&certified, &roots);
        run.runtime_sources.insert("dist/index.js".into());
        assert_eq!(
            census_transcript(&mut run, &export, 0),
            Ok(CensusStep::NeedsTranscripts)
        );
        assert_eq!(run.requested, vec![helper_location.clone()]);

        // A file the verified closure manifest does not call runtime source is
        // not walked at all, the export's own declaration included: a
        // declaration file the archive also ships is a description of code.
        let mut run = census_run(&certified, &roots);
        run.runtime_sources.clear();
        let refusal = census_transcript(&mut run, &export, 0)
            .expect_err("a declaration outside the runtime source set is not walked");
        assert!(
            refusal.contains("not in this artifact's own runtime source"),
            "{refusal}"
        );

        // A name span the parse binds to no node refuses by name rather than
        // asking the producer about bytes the census never read.
        let mut unbound = export.clone();
        unbound.calls[0]
            .declaration
            .as_mut()
            .unwrap()
            .location
            .end_byte += 1;
        let mut run = census_run(&certified, &roots);
        run.runtime_sources.insert("dist/index.js".into());
        let refusal = census_transcript(&mut run, &unbound, 0)
            .expect_err("an identifier no node owns is refused");
        assert!(
            refusal.contains("finds no function-like declaration node"),
            "{refusal}"
        );

        // With the helper's transcript, whose one call is back into the export,
        // the exact stable declaration is a finite-graph back-edge. The root
        // frame already passed its transcript premises, so repeating it adds
        // no unenumerated operation.
        let mut helper = census_transcript_with(
            vec![signals_call(
                "useThing",
                json!({
                    "location": {"path": source_path, "startByte": call_back_at.0, "endByte": call_back_at.1},
                    "targetModule": "",
                    "declaration": {
                        "symbol": "symbol:useThing",
                        "name": "useThing",
                        "kind": "FunctionDeclaration",
                        "sourceFile": source_path,
                        "location": {"path": source_path, "startByte": export_name.0, "endByte": export_name.1},
                    },
                }),
            )],
            json!([]),
        );
        helper.location = helper_location.clone();
        helper.query_name = "helper".into();
        helper.declaration = Some(
            serde_json::from_value(json!({
                "symbol": "symbol:helper",
                "name": "helper",
                "kind": "FunctionDeclaration",
                "sourceFile": source_path,
                "location": {"path": source_path, "startByte": helper_name.0, "endByte": helper_name.1},
            }))
            .expect("a valid declaration"),
        );
        let locals = vec![LocalDeclarationTranscript {
            location: helper_location.clone(),
            transcript: helper.clone(),
        }];
        let mut run = census_run(&certified, &roots);
        run.runtime_sources.insert("dist/index.js".into());
        run.evidence = CensusEvidence {
            roots: &roots,
            locals: &locals,
        };
        // Seeded with the demanded export, as `census_creates_domain` does.
        run.visited
            .push(CensusDeclarationIdentity::of(export.declaration.as_ref().unwrap()).unwrap());
        assert_eq!(
            census_transcript(&mut run, &export, 0),
            Ok(CensusStep::Decided)
        );
        assert!(run.sites.contains(&format!(
            "census-call:{source_path}:{}:{}:call:reachable:local-recursion-backedge",
            call_back_at.0, call_back_at.1
        )));

        // The back-edge does not excuse another edge in the same recursive
        // frame. An unresolved call beside it still refuses the whole census.
        helper.calls.push(signals_call(
            "escape",
            json!({
                "location": {"path": source_path, "startByte": call_back_at.0, "endByte": call_back_at.1},
                "target": "",
                "targetModule": "",
                "declaration": null,
            }),
        ));
        let locals = vec![LocalDeclarationTranscript {
            location: helper_location.clone(),
            transcript: helper.clone(),
        }];
        let mut run = census_run(&certified, &roots);
        run.runtime_sources.insert("dist/index.js".into());
        run.evidence = CensusEvidence {
            roots: &roots,
            locals: &locals,
        };
        run.visited
            .push(CensusDeclarationIdentity::of(export.declaration.as_ref().unwrap()).unwrap());
        let refusal = census_transcript(&mut run, &export, 0)
            .expect_err("a cycle does not hide an unresolved sibling edge");
        assert!(
            refusal.contains("refuses an unresolved callee"),
            "{refusal}"
        );

        // The same helper with a parameter-rooted body decides, and the witness
        // names both the call and the helper transcript it read.
        helper.calls = vec![signals_call(
            "callback",
            json!({
                "location": {"path": source_path, "startByte": call_back_at.0, "endByte": call_back_at.1},
                "target": "",
                "targetModule": "",
                "declaration": null,
                "calleeParameter": {"parameterIndex": 0},
            }),
        )];
        let locals = vec![LocalDeclarationTranscript {
            location: helper_location,
            transcript: helper,
        }];
        let mut run = census_run(&certified, &roots);
        run.runtime_sources.insert("dist/index.js".into());
        run.evidence = CensusEvidence {
            roots: &roots,
            locals: &locals,
        };
        assert_eq!(
            census_transcript(&mut run, &export, 0),
            Ok(CensusStep::Decided)
        );
        assert_eq!((run.calls, run.deepest), (2, 1));
        assert!(run.sites.contains(&format!(
            "census-call:{source_path}:{}:{}:call:reachable:local-recursion",
            call_helper_at.0, call_helper_at.1
        )));
        assert!(run.sites.contains(&format!(
            "census-call:{source_path}:{}:{}:call:reachable:parameter-rooted",
            call_back_at.0, call_back_at.1
        )));
        assert!(run.sites.iter().any(|site| site.starts_with(&format!(
            "census-local-declaration:{source_path}:{}:{}:symbol:helper:sha256:",
            helper_name.0, helper_name.1
        ))));
    }

    /// The consumer source `source`, a transcript for `export_name` declared in
    /// it, and the roots that attribute the transcript to the consumer.
    fn census_source_case(
        source: &str,
        export_name: &str,
        calls: Vec<typefacts::ImplementationCall>,
    ) -> (
        super::super::ArtifactSnapshot,
        typefacts::ExportImplementationTranscript,
    ) {
        let source_path = "/project/node_modules/consumer/dist/index.js";
        let needle = format!("function {export_name}");
        let name_start =
            u64::try_from(source.find(&needle).expect("the export is declared") + 9).unwrap();
        let name_end = name_start + u64::try_from(export_name.len()).unwrap();
        let mut transcript = census_transcript_with(calls, json!([]));
        transcript.query_name = export_name.into();
        transcript.declaration = Some(
            serde_json::from_value(json!({
                "symbol": format!("symbol:{export_name}"),
                "name": export_name,
                "kind": "FunctionDeclaration",
                "sourceFile": source_path,
                "location": {"path": source_path, "startByte": name_start, "endByte": name_end},
            }))
            .expect("a valid declaration"),
        );
        (consumer_snapshot_with(source), transcript)
    }

    /// A call whose callee resolved, by default-library symbol identity, to the
    /// member `qualified`, with `overrides` applied on top.
    fn library_call(
        qualified: &str,
        overrides: serde_json::Value,
    ) -> typefacts::ImplementationCall {
        let lib = "/toolchain/lib/lib.es5.d.ts";
        let name = qualified.rsplit('.').next().unwrap_or(qualified);
        let mut base = json!({
            "targetModule": "",
            "declaration": {
                "symbol": format!("symbol:{qualified}"),
                "name": name,
                "qualifiedName": qualified,
                "kind": "MethodSignature",
                "sourceFile": lib,
                "location": {"path": lib, "startByte": 10, "endByte": 20},
                "standardLibrary": true,
            },
        });
        for (key, value) in overrides.as_object().expect("an object") {
            base.as_object_mut()
                .unwrap()
                .insert(key.clone(), value.clone());
        }
        signals_call(name, base)
    }

    /// The incompleteness rule, at every depth: a construct whose *lower
    /// bound* alone is unmodelled is admitted, a construct the producer cannot
    /// account for refuses by marker and location, and a marker with no
    /// classified construct refuses as an unclassified incompleteness.
    ///
    /// This is the load-bearing relaxation of ADR 0008 item 0, and what makes
    /// it sound is not stated here but in the producer: the `mount(el)` row
    /// below is on the wire with `unknown` reach, so the census disposes the
    /// call rather than reading a marker as a proxy for an absent row.
    #[test]
    fn creates_census_admits_only_a_lower_bound_control_flow_incompleteness() {
        let source = "export function useThing(kind, el) {\n  switch (kind) {\n    case \"mount\":\n      mount(el);\n      break;\n  }\n}\nfunction mount(el) {\n  return el;\n}\n";
        let source_path = "/project/node_modules/consumer/dist/index.js";
        let at = |needle: &str| u64::try_from(source.find(needle).unwrap()).unwrap();
        let switch_span = (at("switch (kind)"), at("}\n}\nfunction") + 1);
        let mount_name = (at("function mount") + 9, at("function mount") + 14);
        let mount_node = (
            at("function mount"),
            u64::try_from(source.trim_end().len()).unwrap(),
        );
        // The producer's own shape now: the `mount(el)` row is stated with
        // `unknown` reach, and the `switch` is classified rather than merely
        // named.
        let call = signals_call(
            "mount",
            json!({
                "reach": "unknown",
                "location": {"path": source_path, "startByte": at("mount(el)"), "endByte": at("mount(el)") + 9},
                "targetModule": "",
                "declaration": {
                    "symbol": "symbol:mount",
                    "name": "mount",
                    "kind": "FunctionDeclaration",
                    "sourceFile": source_path,
                    "location": {"path": source_path, "startByte": mount_name.0, "endByte": mount_name.1},
                },
            }),
        );
        let incompleteness = |marker: &str, class: &str, span: (u64, u64)| {
            json!({
                "unsupported": [marker],
                "incompleteness": [{
                    "marker": marker,
                    "class": class,
                    "location": {"path": source_path, "startByte": span.0, "endByte": span.1},
                }],
            })
        };
        let (certified, mut export) = census_source_case(source, "useThing", vec![call]);
        let roots = vec![consumer_root(&certified)];
        let mut mount = census_transcript_with(vec![], json!([]));
        mount.location = typefacts::Location {
            path: source_path.into(),
            start_byte: mount_node.0,
            end_byte: mount_node.1,
        };
        mount.query_name = "mount".into();
        mount.declaration = Some(
            serde_json::from_value(json!({
                "symbol": "symbol:mount",
                "name": "mount",
                "kind": "FunctionDeclaration",
                "sourceFile": source_path,
                "location": {"path": source_path, "startByte": mount_name.0, "endByte": mount_name.1},
            }))
            .unwrap(),
        );
        let locals = vec![LocalDeclarationTranscript {
            location: mount.location.clone(),
            transcript: mount.clone(),
        }];
        let decide = |export: &typefacts::ExportImplementationTranscript,
                      locals: &[LocalDeclarationTranscript]| {
            let mut run = census_run(&certified, &roots);
            run.evidence = CensusEvidence {
                roots: &roots,
                locals,
            };
            census_transcript(&mut run, export, 0)
        };

        // A `switch` whose `break` the construct itself owns: the lower bound
        // is unmodelled, every call inside is on the wire, and the domain is
        // decided by disposing `mount(el)` — not by relaxing a marker.
        export.control_flow = Some(
            serde_json::from_value(incompleteness(
                "switchReachability",
                "reachability-lower-bound",
                switch_span,
            ))
            .expect("a control-flow census"),
        );
        export.complete = false;
        export.open_reasons = vec!["controlFlowUnsupported".into()];
        assert_eq!(decide(&export, &locals), Ok(CensusStep::Decided));

        // The same construct classified `flow-unaccounted` refuses, by marker
        // and location. That is the class a labelled jump whose target no
        // enclosing construct of the frame owns arrives under.
        let mut unaccounted = export.clone();
        unaccounted.control_flow = Some(
            serde_json::from_value(incompleteness(
                "jumpReachability",
                "flow-unaccounted",
                switch_span,
            ))
            .unwrap(),
        );
        let refusal = decide(&unaccounted, &locals).expect_err("an unaccounted construct refuses");
        assert!(
            refusal.contains("cannot account for a construct (jumpReachability, flow-unaccounted)")
                && refusal.contains(&format!("{}..{}", switch_span.0, switch_span.1)),
            "{refusal}"
        );

        // A marker with no classified construct is an unclassified
        // incompleteness — the shape a protocol-14 producer's transcript
        // decodes to — and refuses.
        let mut unclassified = export.clone();
        unclassified.control_flow =
            Some(serde_json::from_value(json!({"unsupported": ["switchReachability"]})).unwrap());
        let refusal = decide(&unclassified, &locals).expect_err("an unclassified marker refuses");
        assert!(
            refusal.contains(
                "reporting switchReachability unsupported with no classified \
                              construct"
            ),
            "{refusal}"
        );

        // And `controlFlowUnsupported` is the *only* open reason the census
        // relaxes: an open transcript that classifies nothing refuses too,
        // because the reason then describes something this census never saw.
        let mut open_for_nothing = export.clone();
        open_for_nothing.control_flow = Some(serde_json::from_value(json!({})).unwrap());
        let refusal = decide(&open_for_nothing, &locals)
            .expect_err("an open transcript classifying no construct refuses");
        assert!(refusal.contains("classifies no construct"), "{refusal}");

        // Every premise applies at each depth of the recursion, not only at the
        // demanded export: a local declaration arrives through a second
        // acquisition no scheduled demand checked.
        let mut deep_export = export.clone();
        deep_export.control_flow = Some(serde_json::from_value(json!({})).unwrap());
        deep_export.complete = true;
        deep_export.open_reasons = Vec::new();
        let mut unaccounted_local = mount;
        unaccounted_local.control_flow = Some(
            serde_json::from_value(incompleteness(
                "jumpReachability",
                "flow-unaccounted",
                mount_node,
            ))
            .unwrap(),
        );
        unaccounted_local.complete = false;
        unaccounted_local.open_reasons = vec!["controlFlowUnsupported".into()];
        let deep_locals = vec![LocalDeclarationTranscript {
            location: unaccounted_local.location.clone(),
            transcript: unaccounted_local,
        }];
        let refusal = decide(&deep_export, &deep_locals)
            .expect_err("an unaccounted construct one hop down refuses");
        assert!(
            refusal.contains("flow-unaccounted") && refusal.contains("at depth 1"),
            "{refusal}"
        );
    }

    /// A jump inside a *nested* callable leaves no `unsupported` marker on the
    /// transcript — the producer's control-flow census never enters one — and it
    /// no longer needs to.
    ///
    /// The verifier used to answer this with its own Oxc parse, refusing any
    /// node containing a `break` or `continue` anywhere inside it. That check is
    /// gone because both halves of its reason are: the producer withholds no
    /// row, and its jump regions are keyed by flow owner, so the rows of the
    /// nested callable are stated with `unknown` reach. What the census reads is
    /// therefore the row, and a body whose only jump is a nested one is decided
    /// by disposing the call the jump used to hide.
    #[test]
    fn creates_census_decides_a_frame_whose_nested_callable_carries_a_jump() {
        let source = "export function useThing(kind, el) {\n  return () => {\n    while (el) {\n      mount(el);\n      break;\n    }\n  };\n}\nfunction mount(el) {\n  return el;\n}\n";
        let source_path = "/project/node_modules/consumer/dist/index.js";
        let at = |needle: &str| u64::try_from(source.find(needle).unwrap()).unwrap();
        let arrow = (at("() =>"), at("};\n}") + 1);
        let mount_name = (at("function mount") + 9, at("function mount") + 14);
        let mount_node = (
            at("function mount"),
            u64::try_from(source.trim_end().len()).unwrap(),
        );
        let call = signals_call(
            "mount",
            json!({
                "reach": "unknown",
                "captured": true,
                "enclosingCallable": {"path": source_path, "startByte": arrow.0, "endByte": arrow.1},
                "location": {"path": source_path, "startByte": at("mount(el)"), "endByte": at("mount(el)") + 9},
                "targetModule": "",
                "declaration": {
                    "symbol": "symbol:mount",
                    "name": "mount",
                    "kind": "FunctionDeclaration",
                    "sourceFile": source_path,
                    "location": {"path": source_path, "startByte": mount_name.0, "endByte": mount_name.1},
                },
            }),
        );
        let (certified, export) = census_source_case(source, "useThing", vec![call]);
        let roots = vec![consumer_root(&certified)];
        let mut mount = census_transcript_with(vec![], json!([]));
        mount.location = typefacts::Location {
            path: source_path.into(),
            start_byte: mount_node.0,
            end_byte: mount_node.1,
        };
        mount.query_name = "mount".into();
        mount.declaration = Some(
            serde_json::from_value(json!({
                "symbol": "symbol:mount",
                "name": "mount",
                "kind": "FunctionDeclaration",
                "sourceFile": source_path,
                "location": {"path": source_path, "startByte": mount_name.0, "endByte": mount_name.1},
            }))
            .unwrap(),
        );
        let locals = vec![LocalDeclarationTranscript {
            location: mount.location.clone(),
            transcript: mount,
        }];
        let mut run = census_run(&certified, &roots);
        run.evidence = CensusEvidence {
            roots: &roots,
            locals: &locals,
        };
        assert_eq!(
            census_transcript(&mut run, &export, 0),
            Ok(CensusStep::Decided)
        );
    }

    /// A standard-library callee is admitted only when it transfers control to
    /// no callable the census cannot see: every reviewed invoking slot and every
    /// slot the producer saw a callable in must be parameter-rooted or a literal
    /// inside the frame; the by-reference members refuse outright; a
    /// construction is dispositioned like a call.
    #[test]
    fn creates_census_admits_a_standard_library_callee_only_without_an_unseen_callable() {
        let source = "export function useThing(items, callback) {\n  items.forEach(callback);\n  items.forEach(function inline() {});\n  items.forEach(work);\n  return new Map();\n}\nfunction work() {}\n";
        let source_path = "/project/node_modules/consumer/dist/index.js";
        let at = |needle: &str| u64::try_from(source.find(needle).unwrap()).unwrap();
        let inline = (at("function inline"), at("function inline") + 21);
        let (certified, export) = census_source_case(source, "useThing", vec![]);
        let roots = vec![consumer_root(&certified)];
        let mut run = census_run(&certified, &roots);
        // Establish the frame exactly as `census_transcript` does.
        run.frame = Some(census_transcript_frame(&mut run, &export, 0).unwrap());
        let admitted = |run: &mut CensusRun<'_>, call: &typefacts::ImplementationCall| {
            census_call_disposition(run, call, 0).map(|verdict| verdict.map(|(kind, _)| kind))
        };
        let for_each = |overrides: serde_json::Value| {
            let mut base = json!({
                "defaultLibraryInvoker": "arrayIteration",
                "invokedArguments": [0],
            });
            for (key, value) in overrides.as_object().unwrap() {
                base.as_object_mut()
                    .unwrap()
                    .insert(key.clone(), value.clone());
            }
            library_call("Array.forEach", base)
        };

        // Parameter-rooted slot: the caller's code.
        assert_eq!(
            admitted(
                &mut run,
                &for_each(json!({"argumentParameters": [{"parameterIndex": 1}]}))
            ),
            Ok(Some(CensusDisposition::StandardLibrary))
        );
        // A callable literal inside the frame: its calls are rows of this walk.
        assert_eq!(
            admitted(
                &mut run,
                &for_each(json!({
                    "argumentParameters": [null],
                    "argumentCallables": [{"argument": 0, "locations": [
                        {"path": source_path, "startByte": inline.0, "endByte": inline.1}
                    ]}],
                }))
            ),
            Ok(Some(CensusDisposition::StandardLibrary))
        );
        // A reference the producer traced to nothing: refused.
        let refusal = admitted(&mut run, &for_each(json!({"argumentParameters": [null]})))
            .expect_err("an invoked slot with no proof refuses");
        assert!(
            refusal.contains("`Array.forEach`")
                && refusal.contains("argument 0")
                && refusal.contains("reviewed invoker table"),
            "{refusal}"
        );
        // A callable the producer saw outside the frame — the module-local
        // `work` — is not a literal of this transcript.
        let work = (
            at("function work"),
            u64::try_from(source.len()).unwrap() - 1,
        );
        let refusal = admitted(
            &mut run,
            &for_each(json!({
                "argumentParameters": [null],
                "argumentCallables": [{"argument": 0, "locations": [
                    {"path": source_path, "startByte": work.0, "endByte": work.1}
                ]}],
            })),
        )
        .expect_err("a callable outside the frame refuses");
        assert!(refusal.contains("argument 0"), "{refusal}");
        // A callable in a slot no invoker row names still refuses: the census
        // cannot say what the member does with it.
        let refusal = admitted(
            &mut run,
            &library_call(
                "ArrayConstructor.from",
                json!({
                    "argumentParameters": [{"parameterIndex": 0}, null],
                    "argumentCallables": [{"argument": 1, "locations": [
                        {"path": source_path, "startByte": work.0, "endByte": work.1}
                    ]}],
                }),
            ),
        )
        .expect_err("a carried callable in an unreviewed slot refuses");
        assert!(
            refusal.contains("argument 1") && refusal.contains("saw a callable in"),
            "{refusal}"
        );
        // An invoker string outside the reviewed table refuses.
        let refusal = admitted(
            &mut run,
            &for_each(json!({
                "defaultLibraryInvoker": "arrayIterationLater",
                "argumentParameters": [{"parameterIndex": 1}],
            })),
        )
        .expect_err("an unrecognized invoker refuses");
        assert!(
            refusal.contains("not a member of the reviewed table"),
            "{refusal}"
        );
        // By-reference and by-text members refuse whatever the slots prove.
        for qualified in [
            "Reflect.apply",
            "Reflect.construct",
            "CallableFunction.call",
            "Function.apply",
            "NewableFunction.bind",
            "eval",
            "FunctionConstructor",
        ] {
            let refusal = admitted(
                &mut run,
                &library_call(
                    qualified,
                    json!({"argumentParameters": [{"parameterIndex": 1}, {"parameterIndex": 0}]}),
                ),
            )
            .expect_err("a control-transfer member refuses whatever its slots prove");
            assert!(refusal.contains("by reference"), "{qualified}: {refusal}");
        }
        for qualified in ["Reflect.apply", "CallableFunction.call", "eval"] {
            let refusal = admitted(&mut run, &library_call(qualified, json!({})))
                .expect_err("a control-transfer member refuses");
            assert!(
                refusal.contains(&format!("`{qualified}`")) && refusal.contains("by reference"),
                "{refusal}"
            );
        }
        // A construction of a default-library value is dispositioned like a
        // call, and a construction that runs its executor needs the same proof.
        assert_eq!(
            admitted(
                &mut run,
                &library_call("MapConstructor", json!({"kind": "construct"}))
            ),
            Ok(Some(CensusDisposition::StandardLibrary))
        );
        let refusal = admitted(
            &mut run,
            &library_call(
                "PromiseConstructor",
                json!({
                    "kind": "construct",
                    "defaultLibraryInvoker": "promiseConstructor",
                    "invokedArguments": [0],
                    "argumentParameters": [null],
                }),
            ),
        )
        .expect_err("a constructed executor the census cannot see refuses");
        assert!(
            refusal.contains("`PromiseConstructor`") && refusal.contains("argument 0"),
            "{refusal}"
        );
        assert_eq!(
            admitted(
                &mut run,
                &library_call(
                    "PromiseConstructor",
                    json!({
                        "kind": "construct",
                        "defaultLibraryInvoker": "promiseConstructor",
                        "invokedArguments": [0],
                        "argumentParameters": [{"parameterIndex": 1}],
                    })
                )
            ),
            Ok(Some(CensusDisposition::StandardLibrary))
        );
        // The witness site names the construction as one.
        let (_, site) = census_call_disposition(
            &mut run,
            &library_call("MapConstructor", json!({"kind": "construct"})),
            0,
        )
        .unwrap()
        .unwrap();
        assert!(
            site.ends_with(":construct:reachable:standard-library"),
            "{site}"
        );
    }

    /// A local-recursion callee is walked only when the binding the producer
    /// resolved through is proven to hold that declaration: a written binding, a
    /// redeclared name, and a callable expression nothing binds each refuse by
    /// name, while an arrow or function expression that is the whole
    /// initializer of a plain declarator is followed through that binding.
    #[test]
    fn creates_census_follows_an_initializer_binding_and_refuses_a_written_redeclared_or_unbound_one()
     {
        let source_path = "/project/node_modules/consumer/dist/index.js";
        let local_call = |source: &str, callee: &str, name_offset: u64| {
            let at = |needle: &str| u64::try_from(source.find(needle).unwrap()).unwrap();
            let call_at = at(&format!("{callee}(el)"));
            signals_call(
                callee,
                json!({
                    "location": {"path": source_path, "startByte": call_at, "endByte": call_at + u64::try_from(callee.len()).unwrap() + 4},
                    "targetModule": "",
                    "declaration": {
                        "symbol": format!("symbol:{callee}"),
                        "name": callee,
                        "kind": "FunctionDeclaration",
                        "sourceFile": source_path,
                        "location": {"path": source_path, "startByte": name_offset, "endByte": name_offset + u64::try_from(callee.len()).unwrap()},
                    },
                }),
            )
        };
        let refusal_for = |source: &str, callee: &str, name_offset: u64| {
            let (certified, export) = census_source_case(
                source,
                "useThing",
                vec![local_call(source, callee, name_offset)],
            );
            let roots = vec![consumer_root(&certified)];
            let mut run = census_run(&certified, &roots);
            census_transcript(&mut run, &export, 0)
        };
        let name_at = |source: &str, needle: &str| {
            u64::try_from(source.find(needle).unwrap() + needle.len() - "helper".len()).unwrap()
        };

        // Plain reassignment.
        let source = "export function useThing(el) {\n  return helper(el);\n}\nfunction helper(el) {\n  return el;\n}\nhelper = (el) => mount(el);\n";
        let refusal = refusal_for(source, "helper", name_at(source, "function helper"))
            .expect_err("a reassigned binding refuses");
        let write = source.find("helper = (el)").unwrap();
        assert!(
            refusal.contains("local declaration `helper`")
                && refusal.contains("written at")
                && refusal.contains(&format!(":{write}..")),
            "{refusal}"
        );
        // A destructuring assignment target and an update expression are writes.
        let source = "export function useThing(el) {\n  return helper(el);\n}\nfunction helper(el) {\n  return el;\n}\n[helper] = el;\n";
        let refusal = refusal_for(source, "helper", name_at(source, "function helper"))
            .expect_err("a destructuring write refuses");
        assert!(refusal.contains("written at"), "{refusal}");
        // A `for…of` head that assigns the binding is a write on every
        // iteration.
        let source = "export function useThing(el) {\n  return helper(el);\n}\nfunction helper(el) {\n  return el;\n}\nfor (helper of el) {}\n";
        let refusal = refusal_for(source, "helper", name_at(source, "function helper"))
            .expect_err("an iteration head write refuses");
        assert!(refusal.contains("written at"), "{refusal}");
        // A second declaration of the same name.
        let source = "export function useThing(el) {\n  return helper(el);\n}\nfunction helper(el) {\n  return el;\n}\nvar helper = 1;\n";
        let refusal = refusal_for(source, "helper", name_at(source, "function helper"))
            .expect_err("a redeclared binding refuses");
        assert!(refusal.contains("declared again at"), "{refusal}");
        // An arrow a `const` holds resolves to the arrow node itself and has no
        // binding identifier of its own.
        let source =
            "export function useThing(el) {\n  return helper(el);\n}\nconst helper = (el) => el;\n";
        let arrow = u64::try_from(source.find("(el) => el").unwrap()).unwrap();
        let (certified, export) = census_source_case(
            source,
            "useThing",
            vec![signals_call(
                "helper",
                json!({
                    "location": {"path": source_path, "startByte": source.find("helper(el)").unwrap(), "endByte": source.find("helper(el)").unwrap() + 10},
                    "targetModule": "",
                    "declaration": {
                        "symbol": "symbol:helper",
                        "name": "helper",
                        "kind": "ArrowFunction",
                        "sourceFile": source_path,
                        "location": {"path": source_path, "startByte": arrow, "endByte": arrow + 10},
                    },
                }),
            )],
        );
        let roots = vec![consumer_root(&certified)];
        // Since 2026-09-06 the arrow's declarator binds it: `helper` is
        // unwritten and declared once, so the arrow is asked for like a named
        // declaration.
        assert_eq!(
            census_transcript(&mut census_run(&certified, &roots), &export, 0),
            Ok(CensusStep::NeedsTranscripts)
        );
        // The same declarator resolved from another module names the binding
        // identifier rather than the arrow (the third reading).
        let declared = |source: &str, kind: &str, name_offset: u64, len: u64| {
            signals_call(
                "helper",
                json!({
                    "location": {"path": source_path, "startByte": source.find("helper(el)").unwrap(), "endByte": source.find("helper(el)").unwrap() + 10},
                    "targetModule": "",
                    "declaration": {
                        "symbol": "symbol:helper",
                        "name": "helper",
                        "kind": kind,
                        "sourceFile": source_path,
                        "location": {"path": source_path, "startByte": name_offset, "endByte": name_offset + len},
                    },
                }),
            )
        };
        let outcome_for = |source: &str, call: typefacts::ImplementationCall| {
            let (certified, export) = census_source_case(source, "useThing", vec![call]);
            let roots = vec![consumer_root(&certified)];
            census_transcript(&mut census_run(&certified, &roots), &export, 0)
        };
        let const_name = name_at(source, "const helper");
        assert_eq!(
            outcome_for(
                source,
                declared(source, "VariableDeclaration", const_name, 6)
            ),
            Ok(CensusStep::NeedsTranscripts)
        );
        // A `let` is admitted on the same terms, and a write refuses it on the
        // same terms too.
        let source =
            "export function useThing(el) {\n  return helper(el);\n}\nlet helper = (el) => el;\n";
        assert_eq!(
            outcome_for(
                source,
                declared(
                    source,
                    "VariableDeclaration",
                    name_at(source, "let helper"),
                    6
                )
            ),
            Ok(CensusStep::NeedsTranscripts)
        );
        let source = "export function useThing(el) {\n  return helper(el);\n}\nlet helper = (el) => el;\nhelper = (el) => mount(el);\n";
        let refusal = outcome_for(
            source,
            declared(
                source,
                "VariableDeclaration",
                name_at(source, "let helper"),
                6,
            ),
        )
        .expect_err("a written let binding refuses");
        assert!(
            refusal.contains("local declaration `helper`") && refusal.contains("written at"),
            "{refusal}"
        );
        // An initializer that is not a function or arrow literal refuses by
        // name: the census does not trace values.
        let source = "export function useThing(el) {\n  return helper(el);\n}\nconst helper = memo(() => el);\n";
        let refusal = outcome_for(
            source,
            declared(
                source,
                "VariableDeclaration",
                name_at(source, "const helper"),
                6,
            ),
        )
        .expect_err("a call-initialized binding refuses");
        assert!(
            refusal.contains("not a function or arrow literal"),
            "{refusal}"
        );
        // A destructured binding is not a plain one.
        let source =
            "export function useThing(el) {\n  return helper(el);\n}\nconst { helper } = el;\n";
        let refusal = outcome_for(
            source,
            declared(
                source,
                "VariableDeclaration",
                name_at(source, "{ helper"),
                6,
            ),
        )
        .expect_err("a destructured binding refuses");
        assert!(
            refusal.contains("finds no function-like declaration node"),
            "{refusal}"
        );
        // A callable expression that is not a declarator's initializer still
        // has no binding identifier.
        let source = "export function useThing(el) {\n  return helper(el);\n}\nconst helper = [(el) => el][0];\n";
        let arrow = u64::try_from(source.find("(el) => el").unwrap()).unwrap();
        let refusal = outcome_for(source, declared(source, "ArrowFunction", arrow, 10))
            .expect_err("an element callable refuses");
        assert!(
            refusal.contains("no binding identifier of its own"),
            "{refusal}"
        );
        // A parameter of a nested callable is caller-supplied code, and the
        // refusal names the domain that owns it.
        let source =
            "export function useThing(el) {\n  return [el].map((helper) => helper(el));\n}\n";
        let refusal = outcome_for(
            source,
            declared(source, "Parameter", name_at(source, "(helper)"), 6),
        )
        .expect_err("a nested callable's parameter refuses");
        assert!(refusal.contains("callbacks domain"), "{refusal}");
        // The unwritten, once-declared helper is asked for.
        let source = "export function useThing(el) {\n  return helper(el);\n}\nfunction helper(el) {\n  return el;\n}\n";
        assert_eq!(
            refusal_for(source, "helper", name_at(source, "function helper")),
            Ok(CensusStep::NeedsTranscripts)
        );
    }

    /// The producer counts bytes from the first byte after a UTF-8 byte-order
    /// mark, so the verifier parses the same text: a BOM'd runtime source binds
    /// the declaration the producer's offsets name, and does not refuse.
    #[test]
    fn creates_census_reads_a_runtime_source_past_its_byte_order_mark() {
        let body = "export function useThing(items, callback) {\n  return callback(items);\n}\n";
        let source = format!("\u{feff}{body}");
        // Offsets are the producer's: over `body`, not over `source`.
        let (_, export) = census_source_case(body, "useThing", vec![]);
        let certified = consumer_snapshot_with(&source);
        let roots = vec![consumer_root(&certified)];
        assert_eq!(
            census_transcript(&mut census_run(&certified, &roots), &export, 0),
            Ok(CensusStep::Decided)
        );
        // Without the mark removed, the same offsets would name bytes three
        // positions off: the identifier span would bind to no node.
        let mut shifted = export.clone();
        let declaration = shifted.declaration.as_mut().unwrap();
        declaration.location.start_byte += 3;
        declaration.location.end_byte += 3;
        let refusal = census_transcript(&mut census_run(&certified, &roots), &shifted, 0)
            .expect_err("offsets counted over the mark bind nothing");
        assert!(
            refusal.contains("finds no function-like declaration node"),
            "{refusal}"
        );
    }
}
