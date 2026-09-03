//! Projection of the analyzer's existing inference result into the normalized
//! proposal model.
//!
//! `PackageContract` remains an internal inference accumulator while the rule
//! engine is migrated. No legacy document is decoded or emitted here: exact
//! artifact identity arrives independently and the only output is normalized
//! semantics suitable for the stable package-contract encoder.

use std::collections::BTreeSet;

use solid_reactive_ir::{
    CallbackSchedule, ContractCallback, ContractClaim, ContractExport, ContractOwnerRequirement,
    ContractReturn, OwnerRequirementOperation, PackageContract,
    contract_semantics::{
        ArrayLength, ArtifactCase, CallClaims, CallSemantics, CallbackInvocation,
        CapabilityKnowledge, Cardinality, CardinalityScope, ComposedFrom, ContractProposal, Event,
        ExportIdentity, ExportSemantics, ExportTargetIdentity, GuardPartition, KnowledgeSet,
        Lifetime, NormalizedContract, ObjectProperty, Operation, OperationId, OperationKind,
        OwnerCapabilities, OwnerProduction, OwnerRelation, OwnerRequirements, OwnerSource,
        ReactiveRole, Requirement, Resource, ResourceId, ResourceKind, ResourceState, Schedule,
        SemanticClaimPath, SemanticClaimSubject, StabilityKnowledge, Tracking, Trigger, UpperBound,
        ValueShape, ValueSource,
    },
};

use crate::{
    artifact_resolution::{
        ResolvedImport, proposal_identity, select_and_bind, select_and_bind_with_external_targets,
    },
    contract_interface::ContractFailure,
};

/// One normalization's outputs: the proposal, the closure candidates it may
/// propose but not finalize, and the claims it refused to publish by name.
pub(crate) struct NormalizedInference {
    pub(crate) contract: NormalizedContract,
    pub(crate) closure_candidates: Vec<SemanticClaimSubject>,
    pub(crate) withheld: Vec<WithheldOwnerRequirementRecord>,
}

pub(crate) fn normalize_inferred_contract(
    inferred: &PackageContract,
    resolved: &ResolvedImport,
) -> Result<NormalizedContract, ContractFailure> {
    normalize_inferred_contract_with_candidates(inferred, resolved)
        .map(|normalized| normalized.contract)
}

pub(crate) fn normalize_inferred_contract_with_candidates(
    inferred: &PackageContract,
    resolved: &ResolvedImport,
) -> Result<NormalizedInference, ContractFailure> {
    normalize_inferred_contract_with_candidates_and_external_targets(
        inferred,
        resolved,
        &BTreeSet::new(),
    )
}

pub(crate) fn normalize_inferred_contract_with_candidates_and_external_targets(
    inferred: &PackageContract,
    resolved: &ResolvedImport,
    external_targets: &BTreeSet<(String, String)>,
) -> Result<NormalizedInference, ContractFailure> {
    let (selected, withheld) =
        normalize_inferred_contract_identity(inferred, resolved, external_targets)?;
    let package = selected.package().clone();
    let mut cases = selected.artifact_cases().to_vec();
    let mut candidates = Vec::new();
    for artifact_case in &mut cases {
        for (name, export) in &mut artifact_case.exports {
            candidates.extend(export.open_proposed_closure().into_iter().map(|path| {
                SemanticClaimSubject {
                    artifact_case: artifact_case.id.clone(),
                    export: name.clone(),
                    path: SemanticClaimPath::Domain(path),
                }
            }));
        }
    }
    let contract = ContractProposal::new(package, cases)
        .normalize()
        .map_err(model_failure)?;
    Ok(NormalizedInference {
        contract,
        closure_candidates: candidates,
        withheld,
    })
}

fn normalize_inferred_contract_identity(
    inferred: &PackageContract,
    resolved: &ResolvedImport,
    external_targets: &BTreeSet<(String, String)>,
) -> Result<(NormalizedContract, Vec<WithheldOwnerRequirementRecord>), ContractFailure> {
    let entrypoint = inferred
        .entrypoints
        .get(&resolved.requested_entrypoint)
        .ok_or_else(|| ContractFailure::InvalidSemanticModel {
            reason: format!(
                "inference has no entrypoint {:?}",
                resolved.requested_entrypoint
            ),
        })?;
    let (package, mut artifact_case) = proposal_identity(resolved)?;
    // See [`GenerationScope`]: the archive under generation being one of the
    // dialects' own primitive-defining packages is decided once, from the
    // resolved package name, and applies to every export of the case.
    let scope = GenerationScope::for_package(&resolved.package_name);
    let mut withheld = Vec::new();
    for (name, summary) in &entrypoint.exports {
        artifact_case.exports.insert(
            name.clone(),
            normalize_export(&artifact_case, name, summary, scope, &mut withheld)?,
        );
    }
    let normalized = ContractProposal::new(package, vec![artifact_case])
        .normalize()
        .map_err(model_failure)?;
    let selected = if external_targets.is_empty() {
        select_and_bind(&normalized, resolved)?
    } else {
        select_and_bind_with_external_targets(&normalized, resolved, external_targets)?
    };
    Ok((selected, withheld))
}

/// What this generation may claim about the archive it is describing.
///
/// The one distinction it carries is whether the archive is a dialect's own
/// primitive-defining package. Inside such an archive the analyzer's primitive
/// recognition is granted by declaration *path*
/// (`solid-reactive-ir`'s `declaration_path_is_solid_package`), so the
/// package's own local `createSignal`, `createTrackedEffect`, `onCleanup`, …
/// are read as dialect primitive calls, and the owner census and the reactive
/// read census then attribute *the runtime's own internals* to the export as
/// consumer-visible operations. The audited bundled contracts for those exact
/// bytes close both domains as absent
/// (`pkg/contracts/bundled/solid-v1/solid-root-browser-production.json`,
/// `pkg/contracts/bundled/solid-v2/solidjs-signals.json`), so publishing those
/// operations asserts what the audit denies — see ADR 0005 and
/// `docs/package-contract-v2/phase21/2026-09-03-solid-js-self-certification-diagnosis.md`.
///
/// This is a **generation-scope decision, not a proof**: the predicate is an
/// exact package-name match with no version and no integrity behind it, which
/// is not identity. That is admissible only because the decision can act in
/// exactly one direction — it *withholds* claims, turning the two domains
/// open, and can never establish one. The generator cannot answer the other
/// way either: emitting `reads: []`/`creates: []` *closed*, as the audits do,
/// would manufacture a negative claim from a derivation that produced nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GenerationScope {
    /// An ordinary consuming package: every derived domain is published.
    ConsumingPackage,
    /// A dialect's own primitive-defining package.
    DialectDefiningPackage,
}

impl GenerationScope {
    pub(crate) fn for_package(package_name: &str) -> Self {
        if solid_dialect::primitive_defining_package(package_name) {
            Self::DialectDefiningPackage
        } else {
            Self::ConsumingPackage
        }
    }

    /// Whether path-bootstrapped primitive recognition inside this archive may
    /// reach the published document as owner-requirement cleanups and reactive
    /// reads.
    fn publishes_bootstrapped_reactive_domains(self) -> bool {
        matches!(self, Self::ConsumingPackage)
    }
}

fn normalize_export(
    artifact_case: &ArtifactCase,
    name: &str,
    summary: &ContractExport,
    scope: GenerationScope,
    withheld: &mut Vec<WithheldOwnerRequirementRecord>,
) -> Result<ExportSemantics, ContractFailure> {
    let prefix = format!("{}:{name}:operation:", artifact_case.id);
    let mut operations = Vec::new();
    let mut resources = Vec::new();

    let callbacks = match &summary.callbacks {
        ContractClaim::Open => KnowledgeSet::Unknown,
        ContractClaim::Known(callbacks) => KnowledgeSet::Complete(
            callbacks
                .iter()
                .enumerate()
                .map(|(index, callback)| {
                    let id = OperationId(format!("{prefix}callback-{index}"));
                    operations.push(callback_operation(id.clone(), callback, &mut resources)?);
                    Ok(CallbackInvocation {
                        from: ValueSource::Parameter {
                            index: u16::try_from(callback.parameter).map_err(|_| {
                                ContractFailure::InvalidSemanticModel {
                                    reason: format!(
                                        "callback parameter {} exceeds the normalized model limit",
                                        callback.parameter
                                    ),
                                }
                            })?,
                            path: Vec::new(),
                        },
                        operation: id,
                    })
                })
                .collect::<Result<Vec<_>, ContractFailure>>()?,
        ),
    };

    let reads = match &summary.reactive_reads {
        // Withheld, not emptied: the domain is *open*, which says only that
        // this generation does not describe it.
        _ if !scope.publishes_bootstrapped_reactive_domains() => KnowledgeSet::Unknown,
        ContractClaim::Open => KnowledgeSet::Unknown,
        ContractClaim::Known(reads) => KnowledgeSet::Complete(
            reads
                .iter()
                .enumerate()
                .map(|(index, read)| {
                    let id = OperationId(format!("{prefix}read-{index}"));
                    let input = match (read.parameter, read.path.as_ref()) {
                        (Some(parameter), path) => ValueShape::Parameter {
                            index: u16::try_from(parameter).map_err(|_| {
                                ContractFailure::InvalidSemanticModel {
                                    reason: format!(
                                        "reactive-read parameter {parameter} exceeds the normalized model limit"
                                    ),
                                }
                            })?,
                            // The whole access path from the parameter. An
                            // absent path named no segment exactly and stays
                            // empty, which claims only "read through this
                            // parameter" -- the weakest claim the model has.
                            path: path.cloned().unwrap_or_default(),
                        },
                        (None, _) => ValueShape::Reactive {
                            role: ReactiveRole::Accessor,
                            resource: None,
                            capabilities: KnowledgeSet::Unknown,
                        },
                    };
                    let mut read_operation =
                        operation(id.clone(), OperationKind::Read, vec![input], None);
                    // The provenance the IR could name exactly. `read-<n>` is
                    // this same naming rule applied to the *other* export's
                    // own list, which is why the IR carries an ordinal rather
                    // than a label: the ordinal is the operation's name.
                    read_operation.composed_from =
                        read.composed_from
                            .as_ref()
                            .map(|composed| ComposedFrom {
                                export: composed.export.clone(),
                                operation: OperationId(format!(
                                    "{}:{}:operation:read-{}",
                                    artifact_case.id, composed.export, composed.read
                                )),
                            });
                    operations.push(read_operation);
                    Ok(id)
                })
                .collect::<Result<Vec<_>, ContractFailure>>()?,
        ),
    };

    let returns = match &summary.returns {
        ContractClaim::Open => KnowledgeSet::Unknown,
        ContractClaim::Known(None) => KnowledgeSet::Complete(Vec::new()),
        ContractClaim::Known(Some(returned)) => {
            let id = OperationId(format!("{prefix}return"));
            let mut output = return_shape(returned)?;
            if let ContractClaim::Known(protocol) = &summary.async_behavior {
                output = match protocol.as_str() {
                    "promise" => ValueShape::Promise(Box::new(output)),
                    "async-iterable" => ValueShape::AsyncIterable(Box::new(output)),
                    _ => output,
                };
            }
            operations.push(operation(
                id.clone(),
                OperationKind::Return,
                Vec::new(),
                Some(output),
            ));
            KnowledgeSet::Complete(vec![id])
        }
    };

    let owner_requirements = match &summary.owner_requirements {
        // Same withholding as `reads`, for the same reason: the requirement's
        // only evidence inside this archive is the path heuristic naming the
        // primitive's own guarded implementation as a consumer obligation.
        // This one is not a *named* withholding: nothing was derived to
        // withhold. The scope decision refuses the whole derivation, and the
        // audits for these exact bytes state the domains themselves.
        _ if !scope.publishes_bootstrapped_reactive_domains() => None,
        ContractClaim::Open => None,
        ContractClaim::Known(requirements) => Some(requirements.as_slice()),
    };
    let mut requirement_cleanups = Vec::new();
    for (index, requirement) in owner_requirements.unwrap_or_default().iter().enumerate() {
        let id = OperationId(format!("{prefix}owner-requirement-{index}"));
        match owner_requirement_operation(&id, requirement) {
            Ok(operation) => {
                requirement_cleanups.push(id);
                operations.push(operation);
            }
            Err(role) => withheld.push(WithheldOwnerRequirementRecord {
                export: name.to_owned(),
                role,
            }),
        }
    }
    // `creates` carries no owner requirement any more, and the generator
    // derives nothing else that is a `create`: registering a computation on
    // the caller's owner is not a registration into a runtime outside the
    // invocation (`semantic-model.md` § creates), so this generation has
    // *nothing* to say about the domain. Open, never closed-empty -- an owner
    // census that walked this body proves which obligations it found, never
    // that no `create` exists, and `Complete(vec![])` is the negative claim
    // only the hand audits may assert.
    let creates = KnowledgeSet::Unknown;
    // `cleanups` is never *closed* here either, for the same reason: the owner
    // census establishes the cleanup obligations it walked, not that no other
    // cleanup exists. `partial` collapses an empty list back to `Unknown`.
    //
    // It is published `Partial` rather than left `Unknown` because the items
    // are real positive facts a consumer reads: `project_owner_requirements`
    // (`solid-reactive-ir`'s `contracts.rs`) reads this domain for its items
    // *only* and deliberately does not insert `ClaimDomain::Cleanups` into the
    // export's open claims -- which is why publishing items here opens nothing
    // for a consumer. See the scoping note in
    // `phase21/2026-09-03-implementation-census-plan.md` § 2.2 item 5.
    let cleanups = KnowledgeSet::partial(requirement_cleanups).unwrap_or(KnowledgeSet::Unknown);

    let claims = CallClaims {
        callbacks,
        reads,
        writes: KnowledgeSet::Unknown,
        creates,
        invalidates: KnowledgeSet::Unknown,
        throws: KnowledgeSet::Unknown,
        returns,
        cleanups,
        disposals: KnowledgeSet::Unknown,
    };
    let root = ExportTargetIdentity {
        module: artifact_case.runtime.clone(),
        export_name: name.into(),
    };
    Ok(ExportSemantics {
        identity: ExportIdentity {
            entrypoint: artifact_case.entrypoint.clone(),
            public_name: name.into(),
            runtime: root,
            declarations: ExportTargetIdentity {
                module: artifact_case.declarations.clone(),
                export_name: name.into(),
            },
        },
        shape: match summary.kind.as_str() {
            "function" => ValueShape::Callable,
            "component" => ValueShape::Component,
            _ => ValueShape::Plain,
        },
        stability: StabilityKnowledge::Unknown,
        call: CallSemantics::new(
            claims,
            operations,
            Vec::new(),
            resources,
            GuardPartition::default(),
        ),
    })
}

fn callback_operation(
    id: OperationId,
    callback: &ContractCallback,
    resources: &mut Vec<Resource>,
) -> Result<Operation, ContractFailure> {
    // `inline` and `deferred` carry their schedule in the word. `tracked` does
    // not: it is an attribution word, and 1.x `createMemo`/`mergeProps` have
    // already run the callback when the export returns while 1.x `createEffect`
    // has not. Reading `queued` out of `tracked` published that falsehood for
    // every retained row; the schedule now travels with the row, from
    // [`solid_dialect::Dialect::tracked_callback_timing`] by way of
    // `composed_tracked_schedule`. `Unestablished` is the dialect refusing to
    // say, and emits no execution point rather than a guessed one -- the
    // attribution claim survives on its own.
    let (schedule, tracking) = match callback.execution.as_str() {
        "inline" => (Some(Schedule::SameStack), Tracking::Untracked),
        "deferred" => (Some(Schedule::Queued), Tracking::Untracked),
        "tracked" => (
            match callback.schedule {
                Some(CallbackSchedule::SameStack) => Some(Schedule::SameStack),
                Some(CallbackSchedule::External) => Some(Schedule::External),
                Some(CallbackSchedule::Unestablished) => None,
                // A producer that stated no schedule for the row keeps the
                // consumer's historical default. Recorded in
                // docs/precision-backlog.md: the compiler-lowering callback
                // roles are the remaining path that reaches here.
                Some(CallbackSchedule::Queued) | None => Some(Schedule::Queued),
            },
            Tracking::Tracked,
        ),
        other => return invalid(format!("unknown callback execution {other:?}")),
    };
    let mut operation = operation(
        id.clone(),
        OperationKind::Invoke,
        callback
            .arguments
            .iter()
            .map(|argument| {
                argument
                    .as_ref()
                    .map_or(Ok(ValueShape::Unknown), return_shape)
            })
            .collect::<Result<Vec<_>, ContractFailure>>()?,
        None,
    );
    operation.schedule = schedule;
    // The execution point and the schedule are one fact in the document format
    // and must be known together, so an unestablished schedule drops both.
    if schedule.is_none() {
        operation.at = None;
    }
    operation.tracking = tracking;
    operation.owner = match callback.owner.as_deref() {
        Some("none" | "unowned") => owner_none(),
        Some("inherited") => owner_ambient(),
        Some("created" | "leaf") => {
            let resource = ResourceId(format!("{}owner", id.0));
            resources.push(Resource {
                id: resource.clone(),
                kind: ResourceKind::Owner,
                states: KnowledgeSet::Complete(vec![
                    ResourceState::OwnerActive,
                    ResourceState::OwnerDisposed,
                ]),
                capabilities: KnowledgeSet::Complete(Vec::new()),
                lifetime: Some(Lifetime::Owner(resource.clone())),
            });
            owner_created(resource, callback.owner.as_deref() == Some("leaf"))
        }
        Some("conditional") | None => OwnerRelation::default(),
        Some(other) => return invalid(format!("unknown callback owner {other:?}")),
    };
    Ok(operation)
}

fn operation(
    id: OperationId,
    kind: OperationKind,
    inputs: Vec<ValueShape>,
    output: Option<ValueShape>,
) -> Operation {
    Operation {
        id,
        kind,
        guard: None,
        trigger: Some(Trigger::Event(Event::Call)),
        at: Some(Event::Call),
        schedule: Some(Schedule::SameStack),
        tracking: Tracking::Untracked,
        owner: OwnerRelation::default(),
        cardinality: Cardinality {
            scope: Some(CardinalityScope::Call),
            min: Some(0),
            max: Some(UpperBound::Many),
        },
        inputs,
        output,
        resources: BTreeSet::new(),
        composed_from: None,
    }
}

fn owner_none() -> OwnerRelation {
    OwnerRelation {
        source: OwnerSource::None,
        requirements: OwnerRequirements {
            owner: Requirement::Forbidden,
            child_owners: Requirement::Unconstrained,
            cleanup: Requirement::Unconstrained,
        },
        capabilities: OwnerCapabilities {
            child_owners: CapabilityKnowledge::Forbidden,
            cleanup: CapabilityKnowledge::Forbidden,
        },
        lifetime: Some(Lifetime::Call),
        productions: KnowledgeSet::Complete(Vec::new()),
    }
}

fn owner_ambient() -> OwnerRelation {
    OwnerRelation {
        source: OwnerSource::AmbientAtExecution,
        productions: KnowledgeSet::Complete(Vec::new()),
        ..OwnerRelation::default()
    }
}

fn owner_created(resource: ResourceId, leaf: bool) -> OwnerRelation {
    let capabilities = OwnerCapabilities {
        child_owners: if leaf {
            CapabilityKnowledge::Forbidden
        } else {
            CapabilityKnowledge::Allowed
        },
        cleanup: if leaf {
            CapabilityKnowledge::Forbidden
        } else {
            CapabilityKnowledge::Allowed
        },
    };
    OwnerRelation {
        source: OwnerSource::Created(resource.clone()),
        requirements: OwnerRequirements {
            owner: Requirement::Required,
            child_owners: if leaf {
                Requirement::Forbidden
            } else {
                Requirement::Unconstrained
            },
            cleanup: if leaf {
                Requirement::Forbidden
            } else {
                Requirement::Unconstrained
            },
        },
        capabilities: capabilities.clone(),
        lifetime: Some(Lifetime::Owner(resource.clone())),
        productions: KnowledgeSet::Complete(vec![OwnerProduction {
            resource: resource.clone(),
            capabilities,
            lifetime: Some(Lifetime::Owner(resource)),
        }]),
    }
}

/// One withheld owner requirement, named by the export it belongs to.
///
/// A withholding that leaves only an absence behind is indistinguishable from
/// "the census found nothing", which is a different claim. This record is what
/// makes the generator's refusal a *named* one: it travels out of
/// normalization, through [`crate::ProposalArtifacts`], and onto the emit
/// boundary's machine-readable record so the generator's refusal audit can
/// carry the export, the role, and the reason.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WithheldOwnerRequirementRecord {
    pub export: String,
    pub role: WithheldOwnerRequirement,
}

/// The owner-requirement role this generation withheld, named so the
/// withholding can be recorded rather than inferred from an absence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WithheldOwnerRequirement {
    /// An export that must be called under an ambient owner because it
    /// registers a computation on that owner.
    Effect,
    /// An async-boundary JSX element the archive ships, which needs an owner
    /// at the consumer's call site.
    Boundary,
}

impl WithheldOwnerRequirement {
    /// The stable role name a withholding record carries.
    pub const fn role(self) -> &'static str {
        match self {
            Self::Effect => "effect",
            Self::Boundary => "boundary",
        }
    }

    /// Why this generation cannot publish the role, in one line.
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Effect => {
                "a free-standing owner requirement -- an export that must be called under an owner because it registers a computation on it -- has no operation kind that can carry it in schema version 1"
            }
            Self::Boundary => {
                "an async-boundary JSX element's owner requirement is a compiler-lowering fact the implementation census does not record"
            }
        }
    }
}

/// The operation one owner requirement publishes, in the shape the audited
/// documents use for that act -- `Err(role)` when the requirement is one this
/// generation refuses to publish at all.
///
/// An owner requirement is *not* a resourceless `create`: `creates` is the
/// domain of published `create` operations, and a `create` registers a
/// version-1 resource into a runtime outside the invocation, naming what it
/// registered (`docs/package-contract-v2/semantic-model.md` § creates, decision
/// 2026-09-03). Exactly one of the three roles has a home in the model:
///
/// - A `Cleanup`/`SettledCleanup` requirement registers a cleanup on the
///   caller's owner: `kind: cleanup` in the `cleanups` domain, carrying
///   `source: ambient-at-call`, `requires: required`, and
///   `requiresCleanup: required`. **There is no audited precedent for that
///   shape** -- every `kind: cleanup` operation in the bundled corpus
///   (`solid-js`'s `replace-cleanup`, `@solidjs/signals`'s `returned-cleanup`,
///   `@solidjs/web`'s `ref-cleanup`, `--web-node-server`'s
///   `retract-declaration`) is `requires: forbidden`, `source: none`, because
///   each describes a cleanup the *runtime* runs rather than one the export
///   installs on its caller's owner. The shape is chosen for the fact rather
///   than copied: the requirement is a `Requirement` triple on the operation
///   that needs the owner (§ creates' third "not a `creates` item"), the
///   installing act is a cleanup, and `require_owner_operation_call` witnesses
///   it from the archive's own `onCleanup` call. It names **no resource**, as
///   `returned-cleanup` and `ref-cleanup` also do: a resource declaration is a
///   positive fact of its own -- `PositiveFactSubject::Resource`, demanded as
///   `ProofFamily::RecursiveValueShape` -- and no witness exists for a
///   resource axis today, so declaring one would assert what this generation
///   cannot prove.
/// - An `Effect` requirement is **withheld**. It registers a computation on
///   the caller's owner, which is what the audits publish beside
///   `creates: []` *closed*, so it is not a `create`; naming a child owner
///   resource on a `create` would additionally contradict § creates' rule that
///   a `create` names what it registered into an outside runtime. Version 1
///   has no domain that carries a free-standing owner requirement, and the
///   audits record no consumer-level owner requirement anywhere, so the
///   requirement is withheld by name and `creates` stays open. See
///   `docs/package-contract-v2/phase21/2026-09-03-implementation-census-plan.md`
///   § 2.2 item 1.
/// - A `Boundary` requirement is withheld for a second, independent reason: it
///   is a *compiler lowering* fact (the JSX loop at
///   `rust/crates/solid-reactive-ir/src/owners.rs:1213-1231`), and the
///   producer's implementation census records neither JSX elements nor their
///   lowering, so nothing could discharge it.
fn owner_requirement_operation(
    id: &OperationId,
    requirement: &ContractOwnerRequirement,
) -> Result<Operation, WithheldOwnerRequirement> {
    match requirement.operation {
        OwnerRequirementOperation::Cleanup | OwnerRequirementOperation::SettledCleanup => {
            let mut cleanup = operation(id.clone(), OperationKind::Cleanup, Vec::new(), None);
            cleanup.owner = OwnerRelation {
                source: OwnerSource::AmbientAtCall,
                requirements: OwnerRequirements {
                    owner: Requirement::Required,
                    child_owners: Requirement::Unconstrained,
                    cleanup: Requirement::Required,
                },
                capabilities: OwnerCapabilities::default(),
                lifetime: None,
                productions: KnowledgeSet::Complete(Vec::new()),
            };
            Ok(cleanup)
        }
        OwnerRequirementOperation::Effect => Err(WithheldOwnerRequirement::Effect),
        OwnerRequirementOperation::Boundary => Err(WithheldOwnerRequirement::Boundary),
    }
}

fn return_shape(returned: &ContractReturn) -> Result<ValueShape, ContractFailure> {
    Ok(match returned.kind.as_str() {
        "accessor" => ValueShape::Reactive {
            role: ReactiveRole::Accessor,
            resource: None,
            capabilities: KnowledgeSet::Unknown,
        },
        "store-path" => ValueShape::Store {
            resource: None,
            capabilities: KnowledgeSet::Unknown,
        },
        "argument" => ValueShape::Parameter {
            index: u16::try_from(returned.parameter.ok_or_else(|| {
                ContractFailure::InvalidSemanticModel {
                    reason: "argument return shape requires a parameter index".into(),
                }
            })?)
            .map_err(|_| ContractFailure::InvalidSemanticModel {
                reason: format!(
                    "return parameter {} exceeds the normalized model limit",
                    returned.parameter.expect("checked above")
                ),
            })?,
            path: Vec::new(),
        },
        "tuple" => ValueShape::Tuple(KnowledgeSet::Complete(
            returned
                .elements
                .iter()
                .map(|item| item.as_ref().map_or(Ok(ValueShape::Unknown), return_shape))
                .collect::<Result<Vec<_>, ContractFailure>>()?,
        )),
        "object" => ValueShape::Object(KnowledgeSet::Complete(
            returned
                .properties
                .iter()
                .map(|(name, value)| {
                    Ok(ObjectProperty {
                        name: name.clone(),
                        value: return_shape(value)?,
                    })
                })
                .collect::<Result<Vec<_>, ContractFailure>>()?,
        )),
        "array" => ValueShape::Array {
            element: Box::new(ValueShape::Unknown),
            length: ArrayLength::default(),
        },
        _ => ValueShape::Unknown,
    })
}

fn invalid<T>(reason: impl Into<String>) -> Result<T, ContractFailure> {
    Err(ContractFailure::InvalidSemanticModel {
        reason: reason.into(),
    })
}

fn model_failure(error: solid_reactive_ir::contract_semantics::ModelError) -> ContractFailure {
    ContractFailure::InvalidSemanticModel {
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests;
