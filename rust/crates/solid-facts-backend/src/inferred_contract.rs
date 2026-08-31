//! Projection of the analyzer's existing inference result into the normalized
//! proposal model.
//!
//! `PackageContract` remains an internal inference accumulator while the rule
//! engine is migrated. No legacy document is decoded or emitted here: exact
//! artifact identity arrives independently and the only output is normalized
//! semantics suitable for the stable package-contract encoder.

use std::collections::BTreeSet;

use solid_reactive_ir::{
    ContractCallback, ContractClaim, ContractExport, ContractOwnerRequirement, ContractReturn,
    OwnerRequirementOperation, PackageContract,
    contract_semantics::{
        ArrayLength, ArtifactCase, CallClaims, CallSemantics, CallbackInvocation,
        CapabilityKnowledge, Cardinality, CardinalityScope, ContractProposal, Event,
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

pub(crate) fn normalize_inferred_contract(
    inferred: &PackageContract,
    resolved: &ResolvedImport,
) -> Result<NormalizedContract, ContractFailure> {
    normalize_inferred_contract_with_candidates(inferred, resolved).map(|(contract, _)| contract)
}

pub(crate) fn normalize_inferred_contract_with_candidates(
    inferred: &PackageContract,
    resolved: &ResolvedImport,
) -> Result<(NormalizedContract, Vec<SemanticClaimSubject>), ContractFailure> {
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
) -> Result<(NormalizedContract, Vec<SemanticClaimSubject>), ContractFailure> {
    let selected = normalize_inferred_contract_identity(inferred, resolved, external_targets)?;
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
    Ok((contract, candidates))
}

fn normalize_inferred_contract_identity(
    inferred: &PackageContract,
    resolved: &ResolvedImport,
    external_targets: &BTreeSet<(String, String)>,
) -> Result<NormalizedContract, ContractFailure> {
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
    for (name, summary) in &entrypoint.exports {
        artifact_case.exports.insert(
            name.clone(),
            normalize_export(&artifact_case, name, summary)?,
        );
    }
    let normalized = ContractProposal::new(package, vec![artifact_case])
        .normalize()
        .map_err(model_failure)?;
    if external_targets.is_empty() {
        select_and_bind(&normalized, resolved)
    } else {
        select_and_bind_with_external_targets(&normalized, resolved, external_targets)
    }
}

fn normalize_export(
    artifact_case: &ArtifactCase,
    name: &str,
    summary: &ContractExport,
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
        ContractClaim::Open => KnowledgeSet::Unknown,
        ContractClaim::Known(reads) => KnowledgeSet::Complete(
            reads
                .iter()
                .enumerate()
                .map(|(index, read)| {
                    let id = OperationId(format!("{prefix}read-{index}"));
                    let input = match (read.parameter, read.member.as_ref()) {
                        (Some(parameter), member) => ValueShape::Parameter {
                            index: u16::try_from(parameter).map_err(|_| {
                                ContractFailure::InvalidSemanticModel {
                                    reason: format!(
                                        "reactive-read parameter {parameter} exceeds the normalized model limit"
                                    ),
                                }
                            })?,
                            path: member.cloned().into_iter().collect(),
                        },
                        (None, _) => ValueShape::Reactive {
                            role: ReactiveRole::Accessor,
                            resource: None,
                            capabilities: KnowledgeSet::Unknown,
                        },
                    };
                    operations.push(operation(
                        id.clone(),
                        OperationKind::Read,
                        vec![input],
                        None,
                    ));
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

    let creates = match &summary.owner_requirements {
        ContractClaim::Open => KnowledgeSet::Unknown,
        ContractClaim::Known(requirements) => KnowledgeSet::Complete(
            requirements
                .iter()
                .enumerate()
                .map(|(index, requirement)| {
                    let id = OperationId(format!("{prefix}owner-requirement-{index}"));
                    let mut created =
                        operation(id.clone(), OperationKind::Create, Vec::new(), None);
                    apply_owner_requirement(&mut created, requirement);
                    operations.push(created);
                    id
                })
                .collect(),
        ),
    };

    let claims = CallClaims {
        callbacks,
        reads,
        writes: KnowledgeSet::Unknown,
        creates,
        invalidates: KnowledgeSet::Unknown,
        throws: KnowledgeSet::Unknown,
        returns,
        cleanups: KnowledgeSet::Unknown,
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
    let (schedule, tracking) = match callback.execution.as_str() {
        "inline" => (Some(Schedule::SameStack), Tracking::Untracked),
        "deferred" => (Some(Schedule::Queued), Tracking::Untracked),
        "tracked" => (Some(Schedule::Queued), Tracking::Tracked),
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

fn apply_owner_requirement(operation: &mut Operation, requirement: &ContractOwnerRequirement) {
    operation.owner.requirements.owner = Requirement::Required;
    operation.owner.source = OwnerSource::AmbientAtCall;
    operation.owner.productions = KnowledgeSet::Complete(Vec::new());
    match requirement.operation {
        OwnerRequirementOperation::Cleanup | OwnerRequirementOperation::SettledCleanup => {
            operation.owner.requirements.cleanup = Requirement::Required;
        }
        OwnerRequirementOperation::Effect | OwnerRequirementOperation::Boundary => {
            operation.owner.requirements.child_owners = Requirement::Required;
        }
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
