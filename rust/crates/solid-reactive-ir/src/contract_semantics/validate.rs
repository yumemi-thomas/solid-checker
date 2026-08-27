use std::collections::{BTreeMap, BTreeSet};

use super::*;

pub(super) fn normalize(mut proposal: ContractProposal) -> Result<NormalizedContract, ModelError> {
    if proposal.semantic_model_version != SEMANTIC_MODEL_VERSION {
        return Err(ModelError::SemanticModelVersion {
            expected: SEMANTIC_MODEL_VERSION,
            actual: proposal.semantic_model_version,
        });
    }
    validate_package(&proposal.package)?;
    if proposal.artifact_cases.is_empty() {
        return Err(ModelError::MissingArtifactCase { selected: 0 });
    }

    let mut case_ids = BTreeSet::new();
    let mut selections = BTreeMap::new();
    for case in &mut proposal.artifact_cases {
        require_text(&case.id, "artifact case id")?;
        require_text(&case.entrypoint, "artifact case entrypoint")?;
        validate_artifact(&case.runtime, "artifact case runtime")?;
        validate_artifact(&case.declarations, "artifact case declarations")?;
        if let Some(transform) = &case.transform {
            validate_artifact(transform, "artifact case transform")?;
        }
        for step in &case.resolution_trace {
            require_text(&step.condition, "resolution condition")?;
            require_text(&step.target, "resolution target")?;
        }
        if !case_ids.insert(case.id.clone()) {
            return Err(ModelError::DuplicateIdentity {
                kind: "artifact case",
                id: case.id.clone(),
            });
        }

        let selection = (
            case.entrypoint.clone(),
            case.resolution_trace.clone(),
            case.runtime.clone(),
            case.declarations.clone(),
            case.dependency_closure.clone(),
            case.transform.clone(),
        );
        if let Some(first) = selections.insert(selection, case.id.clone()) {
            return Err(ModelError::DuplicateArtifactSelection {
                first,
                second: case.id.clone(),
            });
        }

        for (name, export) in &mut case.exports {
            validate_export_identity(&case.id, &case.entrypoint, name, &export.identity)?;
            normalize_call(&mut export.call, &format!("case {} export {name}", case.id))?;
            let resources = resource_map(&export.call.resources);
            normalize_value(
                &mut export.shape,
                &resources,
                &format!("case {} export {name} value", case.id),
            )?;
        }
    }

    for case in &proposal.artifact_cases {
        validate_artifact_guard_references(case, &case_ids)?;
    }

    proposal
        .artifact_cases
        .sort_by(|left, right| left.id.cmp(&right.id));
    let semantic_digest =
        super::canonical::semantic_digest(&proposal.package, &proposal.artifact_cases);
    Ok(NormalizedContract {
        semantic_model_version: proposal.semantic_model_version,
        package: proposal.package,
        artifact_cases: proposal.artifact_cases,
        semantic_digest,
    })
}

fn validate_package(package: &PackageIdentity) -> Result<(), ModelError> {
    require_text(&package.name, "package name")?;
    require_text(&package.version, "package version")?;
    require_text(&package.integrity, "package integrity")?;
    validate_artifact(&package.manifest, "package manifest")
}

fn validate_artifact(artifact: &ArtifactIdentity, field: &str) -> Result<(), ModelError> {
    require_text(&artifact.path, field)
}

fn require_text(value: &str, field: &str) -> Result<(), ModelError> {
    if value.is_empty() {
        Err(ModelError::EmptyIdentity {
            field: field.into(),
        })
    } else {
        Ok(())
    }
}

fn validate_export_identity(
    case_id: &str,
    entrypoint: &str,
    name: &str,
    identity: &ExportIdentity,
) -> Result<(), ModelError> {
    if name.is_empty()
        || identity.public_name != name
        || identity.entrypoint != entrypoint
        || identity.runtime.export_name.is_empty()
        || identity.declarations.export_name.is_empty()
        || validate_artifact(&identity.runtime.module, "runtime export target").is_err()
        || validate_artifact(&identity.declarations.module, "declaration export target").is_err()
    {
        return Err(ModelError::ExportIdentity {
            case: case_id.into(),
            export: name.into(),
        });
    }
    Ok(())
}

fn validate_artifact_guard_references(
    case: &ArtifactCase,
    case_ids: &BTreeSet<String>,
) -> Result<(), ModelError> {
    for (name, export) in &case.exports {
        let path = format!("case {} export {name}", case.id);
        for operation in &export.call.operations {
            if let Some(guard) = &operation.guard {
                validate_artifact_guard(guard, case_ids, &path)?;
            }
        }
        for guarded in export.call.guards.cases.items() {
            if let GuardedCase::When { guard, .. } = guarded {
                validate_artifact_guard(guard, case_ids, &path)?;
            }
        }
    }
    Ok(())
}

fn validate_artifact_guard(
    guard: &Guard,
    case_ids: &BTreeSet<String>,
    path: &str,
) -> Result<(), ModelError> {
    for atom in &guard.0 {
        if let GuardAtom::ArtifactCase(case) = atom
            && !case_ids.contains(case)
        {
            return Err(ModelError::InvalidGuard {
                path: path.into(),
                reason: format!("artifact case {case} does not exist"),
            });
        }
    }
    Ok(())
}

fn normalize_call(call: &mut CallSemantics, path: &str) -> Result<(), ModelError> {
    normalize_knowledge(&mut call.claims.callbacks, &format!("{path}.callbacks"))?;
    normalize_knowledge(&mut call.claims.reads, &format!("{path}.reads"))?;
    normalize_knowledge(&mut call.claims.writes, &format!("{path}.writes"))?;
    normalize_knowledge(&mut call.claims.creates, &format!("{path}.creates"))?;
    normalize_knowledge(&mut call.claims.invalidates, &format!("{path}.invalidates"))?;
    normalize_knowledge(&mut call.claims.throws, &format!("{path}.throws"))?;
    normalize_knowledge(&mut call.claims.returns, &format!("{path}.returns"))?;
    normalize_knowledge(&mut call.claims.cleanups, &format!("{path}.cleanups"))?;
    normalize_knowledge(&mut call.claims.disposals, &format!("{path}.disposals"))?;

    let mut resource_ids = BTreeSet::new();
    for resource in &mut call.resources {
        require_text(&resource.id.0, "resource id")?;
        if !resource_ids.insert(resource.id.clone()) {
            return Err(ModelError::DuplicateIdentity {
                kind: "resource",
                id: resource.id.0.clone(),
            });
        }
        normalize_knowledge(
            &mut resource.states,
            &format!("{path}.resource.{}.states", resource.id.0),
        )?;
        normalize_knowledge(
            &mut resource.capabilities,
            &format!("{path}.resource.{}.capabilities", resource.id.0),
        )?;
        validate_resource_local(resource, path)?;
    }
    call.resources.sort_by(|left, right| left.id.cmp(&right.id));
    let resources = resource_map(&call.resources);
    for resource in &call.resources {
        if let Some(lifetime) = &resource.lifetime {
            validate_lifetime(
                lifetime,
                &resources,
                &format!("{path}.resource.{}", resource.id.0),
            )?;
        }
    }

    let mut operation_ids = BTreeSet::new();
    for operation in &call.operations {
        require_text(&operation.id.0, "operation id")?;
        if !operation_ids.insert(operation.id.clone()) {
            return Err(ModelError::DuplicateIdentity {
                kind: "operation",
                id: operation.id.0.clone(),
            });
        }
    }
    for operation in &mut call.operations {
        normalize_operation(operation, &operation_ids, &resources, path)?;
    }
    call.operations
        .sort_by(|left, right| left.id.cmp(&right.id));

    validate_call_claims(&call.claims, &call.operations, &resources, path)?;
    normalize_edges(&mut call.edges, &operation_ids, path)?;
    normalize_guard_partition(&mut call.guards, &operation_ids, path)?;
    Ok(())
}

fn normalize_knowledge<T: Ord>(
    knowledge: &mut KnowledgeSet<T>,
    path: &str,
) -> Result<(), ModelError> {
    let items = match knowledge {
        KnowledgeSet::Unknown => return Ok(()),
        KnowledgeSet::Partial(items) if items.is_empty() => {
            return Err(ModelError::InvalidKnowledge {
                path: path.into(),
                reason: "partial knowledge must contain positive evidence".into(),
            });
        }
        KnowledgeSet::Partial(items) | KnowledgeSet::Complete(items) => items,
    };
    items.sort();
    if items.windows(2).any(|items| items[0] == items[1]) {
        return Err(ModelError::InvalidKnowledge {
            path: path.into(),
            reason: "duplicate positive claim".into(),
        });
    }
    Ok(())
}

#[derive(Clone)]
struct ResourceInfo {
    kind: ResourceKind,
    capabilities: KnowledgeSet<ResourceCapability>,
}

fn resource_map(resources: &[Resource]) -> BTreeMap<ResourceId, ResourceInfo> {
    resources
        .iter()
        .map(|resource| {
            (
                resource.id.clone(),
                ResourceInfo {
                    kind: resource.kind,
                    capabilities: resource.capabilities.clone(),
                },
            )
        })
        .collect()
}

fn operation_map(operations: &[Operation]) -> BTreeMap<OperationId, OperationKind> {
    operations
        .iter()
        .map(|operation| (operation.id.clone(), operation.kind))
        .collect()
}

fn validate_resource_local(resource: &Resource, path: &str) -> Result<(), ModelError> {
    let stateful = matches!(
        resource.kind,
        ResourceKind::Owner
            | ResourceKind::Cleanup
            | ResourceKind::AsyncComputation
            | ResourceKind::Transition
            | ResourceKind::Response
            | ResourceKind::Stream
    );
    if stateful && resource.states.proves_absence() {
        return contradiction(
            format!("{path}.resource.{}.states", resource.id.0),
            format!(
                "stateful resource {:?} cannot prove no states",
                resource.kind
            ),
        );
    }
    for state in resource.states.items() {
        let compatible = matches!(
            (resource.kind, state),
            (
                ResourceKind::Owner,
                ResourceState::OwnerActive | ResourceState::OwnerDisposed
            ) | (
                ResourceKind::Cleanup,
                ResourceState::CleanupInstalled | ResourceState::CleanupDisposed
            ) | (
                ResourceKind::AsyncComputation,
                ResourceState::AsyncPending
                    | ResourceState::AsyncSettled
                    | ResourceState::AsyncErrored
                    | ResourceState::AsyncCancelled
            ) | (
                ResourceKind::Transition,
                ResourceState::TransitionActive
                    | ResourceState::TransitionSettled
                    | ResourceState::TransitionReverted
            ) | (
                ResourceKind::Response,
                ResourceState::ResponseUncommitted | ResourceState::ResponseCommitted
            ) | (
                ResourceKind::Stream,
                ResourceState::StreamUnclaimed | ResourceState::StreamClaimed
            )
        );
        if !compatible {
            return contradiction(
                format!("{path}.resource.{}.states", resource.id.0),
                format!("state {state:?} is incompatible with {:?}", resource.kind),
            );
        }
    }
    for capability in resource.capabilities.items() {
        let compatible = match capability {
            ResourceCapability::Refreshable => matches!(
                resource.kind,
                ResourceKind::ReactiveSource | ResourceKind::AsyncComputation
            ),
            ResourceCapability::Writable => matches!(
                resource.kind,
                ResourceKind::ReactiveSource | ResourceKind::Transition
            ),
        };
        if !compatible {
            return contradiction(
                format!("{path}.resource.{}.capabilities", resource.id.0),
                format!(
                    "capability {capability:?} is incompatible with {:?}",
                    resource.kind
                ),
            );
        }
    }
    Ok(())
}

fn normalize_operation(
    operation: &mut Operation,
    operations: &BTreeSet<OperationId>,
    resources: &BTreeMap<ResourceId, ResourceInfo>,
    path: &str,
) -> Result<(), ModelError> {
    let op_path = format!("{path}.operation.{}", operation.id.0);
    if let Some(guard) = &mut operation.guard {
        super::guards::normalize_guard(guard, &format!("{op_path}.guard"))?;
    }
    match &operation.trigger {
        Some(Trigger::Operation(trigger)) => require_operation(trigger, operations, &op_path)?,
        Some(Trigger::Resource { resource, .. }) => {
            require_resource(resource, resources, &op_path)?;
        }
        Some(Trigger::Event(_)) | None => {}
    }
    for resource in &operation.resources {
        require_resource(resource, resources, &op_path)?;
    }
    if let Some(CardinalityScope::Resource(resource)) = &operation.cardinality.scope {
        require_resource(resource, resources, &format!("{op_path}.cardinality"))?;
    }
    validate_cardinality(&operation.cardinality, &op_path)?;
    normalize_owner(&mut operation.owner, resources, &op_path)?;
    for (index, input) in operation.inputs.iter_mut().enumerate() {
        normalize_value(input, resources, &format!("{op_path}.input.{index}"))?;
    }
    if let Some(output) = &mut operation.output {
        normalize_value(output, resources, &format!("{op_path}.output"))?;
    }
    Ok(())
}

fn validate_cardinality(cardinality: &Cardinality, path: &str) -> Result<(), ModelError> {
    if cardinality.scope.is_none() && (cardinality.min.is_some() || cardinality.max.is_some()) {
        return contradiction(
            format!("{path}.cardinality"),
            "bounds require an explicit scope",
        );
    }
    if let Some(UpperBound::Finite(max)) = cardinality.max {
        if max == 0 {
            return contradiction(
                format!("{path}.cardinality"),
                "an explicit operation cannot have a maximum of zero",
            );
        }
        if cardinality.min.is_some_and(|min| min > max) {
            return contradiction(format!("{path}.cardinality"), "minimum exceeds maximum");
        }
    }
    Ok(())
}

fn normalize_owner(
    owner: &mut OwnerRelation,
    resources: &BTreeMap<ResourceId, ResourceInfo>,
    path: &str,
) -> Result<(), ModelError> {
    match &owner.source {
        OwnerSource::Captured(resource) | OwnerSource::Created(resource) => {
            require_resource_kind(resource, ResourceKind::Owner, resources, path)?;
        }
        _ => {}
    }
    if owner.requirements.owner == Requirement::Required && owner.source == OwnerSource::None {
        return contradiction(format!("{path}.owner"), "required owner has source None");
    }
    if owner.requirements.owner == Requirement::Forbidden
        && !matches!(owner.source, OwnerSource::None | OwnerSource::Unknown)
    {
        return contradiction(
            format!("{path}.owner"),
            "forbidden owner has a positive source",
        );
    }
    validate_required_capability(
        owner.requirements.child_owners,
        owner.capabilities.child_owners,
        &format!("{path}.owner.child-owners"),
    )?;
    validate_required_capability(
        owner.requirements.cleanup,
        owner.capabilities.cleanup,
        &format!("{path}.owner.cleanup"),
    )?;
    if owner.source == OwnerSource::None
        && (owner.capabilities.child_owners == CapabilityKnowledge::Allowed
            || owner.capabilities.cleanup == CapabilityKnowledge::Allowed)
    {
        return contradiction(
            format!("{path}.owner"),
            "source None cannot provide owner capabilities",
        );
    }
    if owner.source == OwnerSource::None
        && owner
            .lifetime
            .as_ref()
            .is_some_and(|lifetime| *lifetime != Lifetime::Call)
    {
        return contradiction(
            format!("{path}.owner"),
            "source None cannot claim a resource-bound owner lifetime",
        );
    }
    if let Some(lifetime) = &owner.lifetime {
        validate_lifetime(lifetime, resources, &format!("{path}.owner"))?;
    }
    normalize_knowledge(&mut owner.productions, &format!("{path}.owner.productions"))?;
    for production in owner.productions.items() {
        require_resource_kind(
            &production.resource,
            ResourceKind::Owner,
            resources,
            &format!("{path}.owner.production"),
        )?;
        if let Some(lifetime) = &production.lifetime {
            validate_lifetime(lifetime, resources, &format!("{path}.owner.production"))?;
        }
    }
    if let OwnerSource::Created(created) = &owner.source {
        let produced = owner
            .productions
            .items()
            .iter()
            .any(|production| &production.resource == created);
        if !produced {
            return contradiction(
                format!("{path}.owner"),
                format!(
                    "created owner {} is not named by a positive ownership-production claim",
                    created.0
                ),
            );
        }
    }
    Ok(())
}

fn validate_required_capability(
    requirement: Requirement,
    capability: CapabilityKnowledge,
    path: &str,
) -> Result<(), ModelError> {
    let contradictory = matches!(
        (requirement, capability),
        (Requirement::Required, CapabilityKnowledge::Forbidden)
            | (Requirement::Forbidden, CapabilityKnowledge::Allowed)
    );
    if contradictory {
        contradiction(path, "requirement contradicts the claimed capability")
    } else {
        Ok(())
    }
}

fn validate_lifetime(
    lifetime: &Lifetime,
    resources: &BTreeMap<ResourceId, ResourceInfo>,
    path: &str,
) -> Result<(), ModelError> {
    let expected = match lifetime {
        Lifetime::Call => return Ok(()),
        Lifetime::Resource(resource) => {
            require_resource(resource, resources, path)?;
            return Ok(());
        }
        Lifetime::Owner(resource) => (resource, ResourceKind::Owner),
        Lifetime::Request(resource) => (resource, ResourceKind::Request),
        Lifetime::Transition(resource) => (resource, ResourceKind::Transition),
        Lifetime::AsyncSource(resource) => (resource, ResourceKind::AsyncComputation),
    };
    require_resource_kind(expected.0, expected.1, resources, path)
}

fn require_operation(
    operation: &OperationId,
    operations: &BTreeSet<OperationId>,
    path: &str,
) -> Result<(), ModelError> {
    if operations.contains(operation) {
        Ok(())
    } else {
        Err(ModelError::MissingOperation {
            path: path.into(),
            operation: operation.0.clone(),
        })
    }
}

fn require_resource(
    resource: &ResourceId,
    resources: &BTreeMap<ResourceId, ResourceInfo>,
    path: &str,
) -> Result<(), ModelError> {
    if resources.contains_key(resource) {
        Ok(())
    } else {
        Err(ModelError::MissingResource {
            path: path.into(),
            resource: resource.0.clone(),
        })
    }
}

fn require_resource_kind(
    resource: &ResourceId,
    expected: ResourceKind,
    resources: &BTreeMap<ResourceId, ResourceInfo>,
    path: &str,
) -> Result<(), ModelError> {
    require_resource(resource, resources, path)?;
    let actual = resources[resource].kind;
    if actual != expected {
        contradiction(
            path,
            format!(
                "resource {} has kind {actual:?}, expected {expected:?}",
                resource.0
            ),
        )
    } else {
        Ok(())
    }
}

fn contradiction<T>(path: impl Into<String>, reason: impl Into<String>) -> Result<T, ModelError> {
    Err(ModelError::Contradiction {
        path: path.into(),
        reason: reason.into(),
    })
}

fn normalize_value(
    value: &mut ValueShape,
    resources: &BTreeMap<ResourceId, ResourceInfo>,
    path: &str,
) -> Result<(), ModelError> {
    match value {
        ValueShape::Unknown
        | ValueShape::Plain
        | ValueShape::Parameter { .. }
        | ValueShape::Callable
        | ValueShape::Component
        | ValueShape::RefApplication => {}
        ValueShape::Tuple(items) => {
            validate_open_nonempty(items, &format!("{path}.tuple-items"))?;
            if !matches!(items, KnowledgeSet::Unknown) {
                for (index, item) in items_mut(items).iter_mut().enumerate() {
                    normalize_value(item, resources, &format!("{path}.tuple.{index}"))?;
                }
            }
        }
        ValueShape::Array { element, length } => {
            if let (Some(min), Some(UpperBound::Finite(max))) = (length.min, length.max)
                && min > max
            {
                return contradiction(format!("{path}.array-length"), "minimum exceeds maximum");
            }
            normalize_value(element, resources, &format!("{path}.array-element"))?;
        }
        ValueShape::Object(properties) => {
            validate_open_nonempty(properties, &format!("{path}.object-properties"))?;
            if !matches!(properties, KnowledgeSet::Unknown) {
                for property in items_mut(properties) {
                    require_text(&property.name, "object property")?;
                    normalize_value(
                        &mut property.value,
                        resources,
                        &format!("{path}.property.{}", property.name),
                    )?;
                }
            }
            normalize_knowledge(properties, &format!("{path}.object-properties"))?;
            let names = properties
                .items()
                .iter()
                .map(|property| &property.name)
                .collect::<BTreeSet<_>>();
            if names.len() != properties.items().len() {
                return Err(ModelError::DuplicateIdentity {
                    kind: "object property",
                    id: path.into(),
                });
            }
        }
        ValueShape::Choice(alternatives) => {
            validate_open_nonempty(alternatives, &format!("{path}.choice-alternatives"))?;
            if !matches!(alternatives, KnowledgeSet::Unknown) {
                for (index, alternative) in items_mut(alternatives).iter_mut().enumerate() {
                    normalize_value(alternative, resources, &format!("{path}.choice.{index}"))?;
                }
            }
            normalize_knowledge(alternatives, &format!("{path}.choice-alternatives"))?;
        }
        ValueShape::Promise(inner) => {
            normalize_value(inner, resources, &format!("{path}.promise"))?;
        }
        ValueShape::AsyncIterable(inner) => {
            normalize_value(inner, resources, &format!("{path}.async-iterable"))?;
        }
        ValueShape::Reactive {
            role,
            resource,
            capabilities,
        } => {
            normalize_capabilities(capabilities, resources, path)?;
            if let Some(resource) = resource {
                require_resource_any(
                    resource,
                    &[ResourceKind::ReactiveSource, ResourceKind::AsyncComputation],
                    resources,
                    path,
                )?;
            }
            for capability in capabilities.items() {
                if (*role == ReactiveRole::Accessor
                    && capability.capability == ObservableCapability::Writable)
                    || (*role == ReactiveRole::Setter
                        && capability.capability == ObservableCapability::Readable)
                {
                    return contradiction(
                        format!("{path}.capabilities"),
                        format!("{role:?} cannot claim {:?}", capability.capability),
                    );
                }
            }
            let required = match role {
                ReactiveRole::Accessor => ObservableCapability::Readable,
                ReactiveRole::Setter => ObservableCapability::Writable,
            };
            require_closed_capability(capabilities, required, path)?;
        }
        ValueShape::Store {
            resource,
            capabilities,
        } => {
            normalize_capabilities(capabilities, resources, path)?;
            if let Some(resource) = resource {
                require_resource_any(
                    resource,
                    &[ResourceKind::ReactiveSource, ResourceKind::AsyncComputation],
                    resources,
                    path,
                )?;
            }
            require_closed_capability(capabilities, ObservableCapability::Readable, path)?;
        }
        ValueShape::Action { transition } => {
            if let Some(transition) = transition {
                require_resource_kind(transition, ResourceKind::Transition, resources, path)?;
            }
        }
        ValueShape::Cleanup { resource, lifetime } => {
            if resource.is_none() && lifetime.is_none() {
                return contradiction(
                    path,
                    "cleanup callable must bind a cleanup resource or lifetime",
                );
            }
            if let Some(resource) = resource {
                require_resource_kind(resource, ResourceKind::Cleanup, resources, path)?;
            }
            if let Some(lifetime) = lifetime {
                validate_lifetime(lifetime, resources, path)?;
            }
        }
        ValueShape::ServerFunctionReference { resource } => {
            if let Some(resource) = resource {
                require_resource_kind(
                    resource,
                    ResourceKind::ServerFunctionReference,
                    resources,
                    path,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_open_nonempty<T>(knowledge: &KnowledgeSet<T>, path: &str) -> Result<(), ModelError> {
    if matches!(knowledge, KnowledgeSet::Partial(items) if items.is_empty()) {
        return Err(ModelError::InvalidKnowledge {
            path: path.into(),
            reason: "partial knowledge must contain positive evidence".into(),
        });
    }
    Ok(())
}

fn items_mut<T>(knowledge: &mut KnowledgeSet<T>) -> &mut Vec<T> {
    match knowledge {
        KnowledgeSet::Unknown => {
            unreachable!("unknown knowledge has no items and is handled before traversal")
        }
        KnowledgeSet::Partial(items) | KnowledgeSet::Complete(items) => items,
    }
}

fn normalize_capabilities(
    capabilities: &mut KnowledgeSet<CapabilityClaim>,
    resources: &BTreeMap<ResourceId, ResourceInfo>,
    path: &str,
) -> Result<(), ModelError> {
    normalize_knowledge(capabilities, &format!("{path}.capabilities"))?;
    for claim in capabilities.items() {
        match claim.capability {
            ObservableCapability::Readable | ObservableCapability::Writable => {
                if claim.resource.is_some() {
                    return contradiction(
                        format!("{path}.capabilities"),
                        format!(
                            "{:?} is intrinsic and must not name a resource",
                            claim.capability
                        ),
                    );
                }
            }
            ObservableCapability::Refreshable
            | ObservableCapability::PendingAware
            | ObservableCapability::Optimistic => {
                let resource =
                    claim
                        .resource
                        .as_ref()
                        .ok_or_else(|| ModelError::Contradiction {
                            path: format!("{path}.capabilities"),
                            reason: format!("{:?} must name its exact resource", claim.capability),
                        })?;
                let info = resources
                    .get(resource)
                    .ok_or_else(|| ModelError::MissingResource {
                        path: format!("{path}.capabilities"),
                        resource: resource.0.clone(),
                    })?;
                let kind = info.kind;
                match claim.capability {
                    ObservableCapability::Refreshable => {
                        if !matches!(
                            kind,
                            ResourceKind::ReactiveSource | ResourceKind::AsyncComputation
                        ) {
                            return contradiction(
                                format!("{path}.capabilities"),
                                "refreshable capability requires a reactive or async resource",
                            );
                        }
                        require_positive_resource_capability(
                            resource,
                            ResourceCapability::Refreshable,
                            resources,
                            path,
                        )?;
                    }
                    ObservableCapability::PendingAware => {
                        if kind != ResourceKind::AsyncComputation {
                            return contradiction(
                                format!("{path}.capabilities"),
                                "pending-aware capability requires an async resource",
                            );
                        }
                    }
                    ObservableCapability::Optimistic => {
                        if kind != ResourceKind::Transition {
                            return contradiction(
                                format!("{path}.capabilities"),
                                "optimistic capability requires a transition resource",
                            );
                        }
                        require_positive_resource_capability(
                            resource,
                            ResourceCapability::Writable,
                            resources,
                            path,
                        )?;
                    }
                    ObservableCapability::Readable | ObservableCapability::Writable => {
                        unreachable!()
                    }
                }
            }
        }
    }
    if capabilities
        .items()
        .iter()
        .any(|claim| claim.capability == ObservableCapability::Optimistic)
        && !capabilities
            .items()
            .iter()
            .any(|claim| claim.capability == ObservableCapability::Writable)
    {
        return contradiction(
            format!("{path}.capabilities"),
            "optimistic capability requires an explicit writable capability",
        );
    }
    Ok(())
}

fn require_positive_resource_capability(
    resource: &ResourceId,
    capability: ResourceCapability,
    resources: &BTreeMap<ResourceId, ResourceInfo>,
    path: &str,
) -> Result<(), ModelError> {
    let info = &resources[resource];
    let kind = info.kind;
    let compatible = matches!(
        (capability, kind),
        (
            ResourceCapability::Refreshable,
            ResourceKind::ReactiveSource | ResourceKind::AsyncComputation
        ) | (
            ResourceCapability::Writable,
            ResourceKind::ReactiveSource | ResourceKind::Transition
        )
    );
    if !compatible {
        contradiction(
            format!("{path}.capabilities"),
            "value capability is incompatible with its resource",
        )
    } else if !info.capabilities.items().contains(&capability) {
        contradiction(
            format!("{path}.capabilities"),
            format!(
                "value capability {capability:?} lacks a positive claim on resource {}",
                resource.0
            ),
        )
    } else {
        Ok(())
    }
}

fn require_closed_capability(
    capabilities: &KnowledgeSet<CapabilityClaim>,
    required: ObservableCapability,
    path: &str,
) -> Result<(), ModelError> {
    if capabilities.is_closed()
        && !capabilities
            .items()
            .iter()
            .any(|claim| claim.capability == required)
    {
        contradiction(
            format!("{path}.capabilities"),
            format!("closed capability set omits required {required:?}"),
        )
    } else {
        Ok(())
    }
}

fn require_resource_any(
    resource: &ResourceId,
    expected: &[ResourceKind],
    resources: &BTreeMap<ResourceId, ResourceInfo>,
    path: &str,
) -> Result<ResourceKind, ModelError> {
    require_resource(resource, resources, path)?;
    let actual = resources[resource].kind;
    if expected.contains(&actual) {
        Ok(actual)
    } else {
        contradiction(
            path,
            format!("resource {} has incompatible kind {actual:?}", resource.0),
        )
    }
}

fn validate_call_claims(
    claims: &CallClaims,
    operations: &[Operation],
    resources: &BTreeMap<ResourceId, ResourceInfo>,
    path: &str,
) -> Result<(), ModelError> {
    let operation_kinds = operation_map(operations);
    for callback in claims.callbacks.items() {
        require_operation_kind(
            &callback.operation,
            OperationKind::Invoke,
            &operation_kinds,
            &format!("{path}.callbacks"),
        )?;
        match &callback.from {
            ValueSource::Parameter { .. } => {}
            ValueSource::OperationOutput { operation, .. } => {
                if !operation_kinds.contains_key(operation) {
                    return Err(ModelError::MissingOperation {
                        path: format!("{path}.callbacks.source"),
                        operation: operation.0.clone(),
                    });
                }
            }
            ValueSource::Resource { resource, .. } => {
                require_resource(resource, resources, &format!("{path}.callbacks.source"))?;
            }
        }
    }
    validate_operation_claim(
        &claims.reads,
        Some(OperationKind::Read),
        &operation_kinds,
        &format!("{path}.reads"),
    )?;
    validate_operation_claim(
        &claims.writes,
        Some(OperationKind::Write),
        &operation_kinds,
        &format!("{path}.writes"),
    )?;
    validate_operation_claim(
        &claims.creates,
        Some(OperationKind::Create),
        &operation_kinds,
        &format!("{path}.creates"),
    )?;
    validate_operation_claim(
        &claims.invalidates,
        Some(OperationKind::Invalidate),
        &operation_kinds,
        &format!("{path}.invalidates"),
    )?;
    validate_operation_claim(
        &claims.throws,
        None,
        &operation_kinds,
        &format!("{path}.throws"),
    )?;
    validate_operation_claim(
        &claims.returns,
        Some(OperationKind::Return),
        &operation_kinds,
        &format!("{path}.returns"),
    )?;
    validate_operation_claim(
        &claims.cleanups,
        Some(OperationKind::Cleanup),
        &operation_kinds,
        &format!("{path}.cleanups"),
    )?;
    validate_operation_claim(
        &claims.disposals,
        Some(OperationKind::Dispose),
        &operation_kinds,
        &format!("{path}.disposals"),
    )?;
    for operation in operations {
        let represented = match operation.kind {
            OperationKind::Invoke => claims
                .callbacks
                .items()
                .iter()
                .any(|callback| callback.operation == operation.id),
            OperationKind::Return => claims.returns.items().contains(&operation.id),
            OperationKind::Read => claims.reads.items().contains(&operation.id),
            OperationKind::Write => claims.writes.items().contains(&operation.id),
            OperationKind::Invalidate => claims.invalidates.items().contains(&operation.id),
            OperationKind::Create => claims.creates.items().contains(&operation.id),
            OperationKind::Cleanup => claims.cleanups.items().contains(&operation.id),
            OperationKind::Dispose => claims.disposals.items().contains(&operation.id),
        };
        if !represented {
            return contradiction(
                format!("{path}.operation.{}", operation.id.0),
                "operation node lacks its corresponding positive call claim",
            );
        }
    }
    Ok(())
}

fn validate_operation_claim(
    claims: &KnowledgeSet<OperationId>,
    expected: Option<OperationKind>,
    operations: &BTreeMap<OperationId, OperationKind>,
    path: &str,
) -> Result<(), ModelError> {
    for operation in claims.items() {
        let actual = operations
            .get(operation)
            .ok_or_else(|| ModelError::MissingOperation {
                path: path.into(),
                operation: operation.0.clone(),
            })?;
        if expected.is_some_and(|expected| expected != *actual) {
            return contradiction(
                path,
                format!(
                    "operation {} has kind {actual:?}, expected {:?}",
                    operation.0, expected
                ),
            );
        }
    }
    Ok(())
}

fn require_operation_kind(
    operation: &OperationId,
    expected: OperationKind,
    operations: &BTreeMap<OperationId, OperationKind>,
    path: &str,
) -> Result<(), ModelError> {
    let actual = operations
        .get(operation)
        .ok_or_else(|| ModelError::MissingOperation {
            path: path.into(),
            operation: operation.0.clone(),
        })?;
    if *actual == expected {
        Ok(())
    } else {
        contradiction(
            path,
            format!(
                "operation {} has kind {actual:?}, expected {expected:?}",
                operation.0
            ),
        )
    }
}

fn normalize_edges(
    edges: &mut [OperationEdge],
    operations: &BTreeSet<OperationId>,
    path: &str,
) -> Result<(), ModelError> {
    for edge in edges.iter() {
        require_operation(&edge.from, operations, &format!("{path}.edges"))?;
        require_operation(&edge.to, operations, &format!("{path}.edges"))?;
        if edge.from == edge.to {
            return Err(ModelError::OperationCycle {
                operation: edge.from.0.clone(),
            });
        }
    }
    edges.sort();
    if edges.windows(2).any(|edges| edges[0] == edges[1]) {
        return Err(ModelError::DuplicateIdentity {
            kind: "operation edge",
            id: path.into(),
        });
    }

    let mut adjacency = BTreeMap::<&OperationId, Vec<&OperationId>>::new();
    for edge in edges.iter() {
        adjacency.entry(&edge.from).or_default().push(&edge.to);
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for operation in operations {
        visit_operation(operation, &adjacency, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn visit_operation<'a>(
    operation: &'a OperationId,
    adjacency: &BTreeMap<&'a OperationId, Vec<&'a OperationId>>,
    visiting: &mut BTreeSet<&'a OperationId>,
    visited: &mut BTreeSet<&'a OperationId>,
) -> Result<(), ModelError> {
    if visited.contains(operation) {
        return Ok(());
    }
    if !visiting.insert(operation) {
        return Err(ModelError::OperationCycle {
            operation: operation.0.clone(),
        });
    }
    if let Some(next) = adjacency.get(operation) {
        for next in next {
            visit_operation(next, adjacency, visiting, visited)?;
        }
    }
    visiting.remove(operation);
    visited.insert(operation);
    Ok(())
}

fn normalize_guard_partition(
    partition: &mut GuardPartition,
    operations: &BTreeSet<OperationId>,
    path: &str,
) -> Result<(), ModelError> {
    validate_open_nonempty(&partition.cases, &format!("{path}.guard-partition"))?;
    if matches!(partition.cases, KnowledgeSet::Unknown) {
        return Ok(());
    }
    let complete = partition.cases.is_closed();
    let cases = items_mut(&mut partition.cases);
    let mut otherwise = None;
    let mut when = Vec::new();
    for (index, mut case) in std::mem::take(cases).into_iter().enumerate() {
        match &mut case {
            GuardedCase::When {
                guard,
                operations: selected,
            } => {
                super::guards::normalize_guard(guard, &format!("{path}.guard-partition.{index}"))?;
                normalize_knowledge(
                    selected,
                    &format!("{path}.guard-partition.{index}.operations"),
                )?;
                validate_selected_operations(selected, operations, path)?;
                when.push(case);
            }
            GuardedCase::Otherwise {
                operations: selected,
            } => {
                if otherwise.is_some() {
                    return Err(ModelError::InvalidGuard {
                        path: format!("{path}.guard-partition"),
                        reason: "at most one otherwise case is allowed".into(),
                    });
                }
                normalize_knowledge(
                    selected,
                    &format!("{path}.guard-partition.otherwise.operations"),
                )?;
                validate_selected_operations(selected, operations, path)?;
                otherwise = Some(case);
            }
        }
    }
    if complete && !when.is_empty() && otherwise.is_none() {
        return Err(ModelError::InvalidGuard {
            path: format!("{path}.guard-partition"),
            reason: "a complete non-empty partition requires an otherwise case".into(),
        });
    }
    if !complete && otherwise.is_some() {
        return Err(ModelError::InvalidGuard {
            path: format!("{path}.guard-partition"),
            reason: "an open partition cannot claim an exhaustive otherwise case".into(),
        });
    }
    when.sort();
    for left in 0..when.len() {
        for right in left + 1..when.len() {
            let GuardedCase::When {
                guard: left_guard, ..
            } = &when[left]
            else {
                unreachable!()
            };
            let GuardedCase::When {
                guard: right_guard, ..
            } = &when[right]
            else {
                unreachable!()
            };
            if super::guards::guards_overlap(left_guard, right_guard) {
                return Err(ModelError::OverlappingGuards {
                    path: format!("{path}.guard-partition"),
                    left,
                    right,
                });
            }
        }
    }
    cases.extend(when);
    if let Some(otherwise) = otherwise {
        cases.push(otherwise);
    }
    Ok(())
}

fn validate_selected_operations(
    selected: &KnowledgeSet<OperationId>,
    operations: &BTreeSet<OperationId>,
    path: &str,
) -> Result<(), ModelError> {
    for operation in selected.items() {
        require_operation(operation, operations, &format!("{path}.guard-partition"))?;
    }
    Ok(())
}

pub(super) fn unresolved_claims(export: &ExportSemantics) -> Vec<ClaimPath> {
    let mut claims = export.unresolved_call_claims();
    visit_value(
        &export.shape,
        ValueRoot::Export,
        ValuePath::default(),
        &mut claims,
    );
    for operation in &export.call.operations {
        let id = operation.id.clone();
        if operation.trigger.is_none() {
            push_operation(&mut claims, &id, OperationClaimDomain::Trigger);
        }
        if operation.at.is_none() {
            push_operation(&mut claims, &id, OperationClaimDomain::ExecutionPoint);
        }
        if operation.schedule.is_none() {
            push_operation(&mut claims, &id, OperationClaimDomain::Schedule);
        }
        if operation.tracking == Tracking::Unknown {
            push_operation(&mut claims, &id, OperationClaimDomain::Tracking);
        }
        if operation.owner.source == OwnerSource::Unknown {
            push_operation(&mut claims, &id, OperationClaimDomain::OwnerSource);
        }
        if operation.owner.capabilities.child_owners == CapabilityKnowledge::Unknown {
            push_operation(&mut claims, &id, OperationClaimDomain::OwnerChildCapability);
        }
        if operation.owner.capabilities.cleanup == CapabilityKnowledge::Unknown {
            push_operation(
                &mut claims,
                &id,
                OperationClaimDomain::OwnerCleanupCapability,
            );
        }
        if operation.owner.lifetime.is_none() {
            push_operation(&mut claims, &id, OperationClaimDomain::OwnerLifetime);
        }
        if operation.owner.productions.state().is_open() {
            push_operation(&mut claims, &id, OperationClaimDomain::OwnerProductions);
        }
        if operation.cardinality.scope.is_none() {
            push_operation(&mut claims, &id, OperationClaimDomain::CardinalityScope);
        }
        if operation.cardinality.min.is_none() {
            push_operation(&mut claims, &id, OperationClaimDomain::CardinalityMinimum);
        }
        if operation.cardinality.max.is_none() {
            push_operation(&mut claims, &id, OperationClaimDomain::CardinalityMaximum);
        }
        for (index, input) in operation.inputs.iter().enumerate() {
            visit_value(
                input,
                ValueRoot::OperationInput {
                    operation: id.clone(),
                    index: u16::try_from(index).unwrap_or(u16::MAX),
                },
                ValuePath::default(),
                &mut claims,
            );
        }
        if let Some(output) = &operation.output {
            visit_value(
                output,
                ValueRoot::OperationOutput {
                    operation: id.clone(),
                },
                ValuePath::default(),
                &mut claims,
            );
        }
    }
    for resource in &export.call.resources {
        if resource.states.state().is_open() {
            claims.push(ClaimPath::Resource {
                resource: resource.id.clone(),
                domain: ResourceClaimDomain::States,
            });
        }
        if resource.capabilities.state().is_open() {
            claims.push(ClaimPath::Resource {
                resource: resource.id.clone(),
                domain: ResourceClaimDomain::Capabilities,
            });
        }
        if resource.lifetime.is_none() {
            claims.push(ClaimPath::Resource {
                resource: resource.id.clone(),
                domain: ResourceClaimDomain::Lifetime,
            });
        }
    }
    if export.call.guards.cases.state().is_open() {
        claims.push(ClaimPath::GuardPartition);
    }
    claims.sort();
    claims.dedup();
    claims
}

pub(super) fn claim_subject_exists(export: &ExportSemantics, subject: &SemanticClaimPath) -> bool {
    match subject {
        SemanticClaimPath::Operation(operation) => export.operation(&operation.0).is_some(),
        SemanticClaimPath::Domain(ClaimPath::Call(_))
        | SemanticClaimPath::Domain(ClaimPath::GuardPartition) => true,
        SemanticClaimPath::Domain(ClaimPath::Operation { operation, .. }) => {
            export.operation(&operation.0).is_some()
        }
        SemanticClaimPath::Domain(ClaimPath::Resource { resource, .. }) => export
            .call
            .resources
            .iter()
            .any(|candidate| candidate.id == *resource),
        SemanticClaimPath::Domain(ClaimPath::Value { root, path, domain }) => {
            claim_value_root(export, root)
                .and_then(|value| claim_value_path(value, &path.0))
                .is_some_and(|value| value_claim_domain_exists(value, *domain))
        }
    }
}

fn claim_value_root<'a>(export: &'a ExportSemantics, root: &ValueRoot) -> Option<&'a ValueShape> {
    match root {
        ValueRoot::Export => Some(&export.shape),
        ValueRoot::OperationInput { operation, index } => export
            .operation(&operation.0)?
            .inputs
            .get(usize::from(*index)),
        ValueRoot::OperationOutput { operation } => export.operation(&operation.0)?.output.as_ref(),
    }
}

fn claim_value_path<'a>(
    mut value: &'a ValueShape,
    path: &[ValuePathSegment],
) -> Option<&'a ValueShape> {
    for segment in path {
        value = match (value, segment) {
            (ValueShape::Tuple(items), ValuePathSegment::TupleItem(index)) => {
                items.items().get(usize::try_from(*index).ok()?)?
            }
            (ValueShape::Array { element, .. }, ValuePathSegment::ArrayElement) => element,
            (ValueShape::Object(properties), ValuePathSegment::ObjectProperty(name)) => {
                &properties
                    .items()
                    .iter()
                    .find(|property| property.name == *name)?
                    .value
            }
            (ValueShape::Choice(alternatives), ValuePathSegment::ChoiceAlternative(index)) => {
                alternatives.items().get(usize::try_from(*index).ok()?)?
            }
            (ValueShape::Promise(inner), ValuePathSegment::PromiseValue)
            | (ValueShape::AsyncIterable(inner), ValuePathSegment::AsyncIterableElement) => inner,
            _ => return None,
        };
    }
    Some(value)
}

fn value_claim_domain_exists(value: &ValueShape, domain: ValueClaimDomain) -> bool {
    match domain {
        ValueClaimDomain::Shape => true,
        ValueClaimDomain::TupleItems => matches!(value, ValueShape::Tuple(_)),
        ValueClaimDomain::ObjectProperties => matches!(value, ValueShape::Object(_)),
        ValueClaimDomain::ChoiceAlternatives => matches!(value, ValueShape::Choice(_)),
        ValueClaimDomain::ArrayMinimumLength | ValueClaimDomain::ArrayMaximumLength => {
            matches!(value, ValueShape::Array { .. })
        }
        ValueClaimDomain::Capabilities => {
            matches!(
                value,
                ValueShape::Reactive { .. } | ValueShape::Store { .. }
            )
        }
    }
}

pub(super) fn open_proposed_closure(export: &mut ExportSemantics) -> Vec<ClaimPath> {
    let mut candidates = Vec::new();
    if export.call.claims.callbacks.open_proposed_closure() {
        candidates.push(ClaimPath::Call(ClaimDomain::Callbacks));
    }
    for domain in ClaimDomain::ALL
        .into_iter()
        .filter(|domain| *domain != ClaimDomain::Callbacks)
    {
        let knowledge = match domain {
            ClaimDomain::Callbacks => unreachable!("callbacks are opened separately"),
            ClaimDomain::Reads => &mut export.call.claims.reads,
            ClaimDomain::Writes => &mut export.call.claims.writes,
            ClaimDomain::Creates => &mut export.call.claims.creates,
            ClaimDomain::Invalidates => &mut export.call.claims.invalidates,
            ClaimDomain::Throws => &mut export.call.claims.throws,
            ClaimDomain::Returns => &mut export.call.claims.returns,
            ClaimDomain::Cleanups => &mut export.call.claims.cleanups,
            ClaimDomain::Disposals => &mut export.call.claims.disposals,
        };
        if knowledge.open_proposed_closure() {
            candidates.push(ClaimPath::Call(domain));
        }
    }
    open_value_closure(
        &mut export.shape,
        ValueRoot::Export,
        ValuePath::default(),
        &mut candidates,
    );
    for operation in &mut export.call.operations {
        let id = operation.id.clone();
        if operation.owner.productions.open_proposed_closure() {
            push_operation(&mut candidates, &id, OperationClaimDomain::OwnerProductions);
        }
        for (index, input) in operation.inputs.iter_mut().enumerate() {
            open_value_closure(
                input,
                ValueRoot::OperationInput {
                    operation: id.clone(),
                    index: u16::try_from(index).unwrap_or(u16::MAX),
                },
                ValuePath::default(),
                &mut candidates,
            );
        }
        if let Some(output) = &mut operation.output {
            open_value_closure(
                output,
                ValueRoot::OperationOutput {
                    operation: id.clone(),
                },
                ValuePath::default(),
                &mut candidates,
            );
        }
    }
    for resource in &mut export.call.resources {
        if resource.states.open_proposed_closure() {
            candidates.push(ClaimPath::Resource {
                resource: resource.id.clone(),
                domain: ResourceClaimDomain::States,
            });
        }
        if resource.capabilities.open_proposed_closure() {
            candidates.push(ClaimPath::Resource {
                resource: resource.id.clone(),
                domain: ResourceClaimDomain::Capabilities,
            });
        }
    }
    if export.call.guards.cases.open_proposed_closure() {
        candidates.push(ClaimPath::GuardPartition);
    }
    for guarded in export.call.guards.cases.items_mut() {
        let operations = match guarded {
            GuardedCase::When { operations, .. } | GuardedCase::Otherwise { operations } => {
                operations
            }
        };
        if operations.open_proposed_closure() {
            candidates.push(ClaimPath::GuardPartition);
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

pub(super) fn close_verified_claim(
    export: &mut ExportSemantics,
    claim: &ClaimPath,
) -> Result<(), ModelError> {
    let closed = match claim {
        ClaimPath::Call(domain) => match domain {
            ClaimDomain::Callbacks => export.call.claims.callbacks.close_verified(),
            ClaimDomain::Reads => export.call.claims.reads.close_verified(),
            ClaimDomain::Writes => export.call.claims.writes.close_verified(),
            ClaimDomain::Creates => export.call.claims.creates.close_verified(),
            ClaimDomain::Invalidates => export.call.claims.invalidates.close_verified(),
            ClaimDomain::Throws => export.call.claims.throws.close_verified(),
            ClaimDomain::Returns => export.call.claims.returns.close_verified(),
            ClaimDomain::Cleanups => export.call.claims.cleanups.close_verified(),
            ClaimDomain::Disposals => export.call.claims.disposals.close_verified(),
        },
        ClaimPath::Value { root, path, domain } => {
            let value =
                value_root_mut(export, root).and_then(|value| value_at_path_mut(value, &path.0));
            match (value, domain) {
                (Some(ValueShape::Tuple(items)), ValueClaimDomain::TupleItems) => {
                    items.close_verified()
                }
                (Some(ValueShape::Object(properties)), ValueClaimDomain::ObjectProperties) => {
                    properties.close_verified()
                }
                (Some(ValueShape::Choice(alternatives)), ValueClaimDomain::ChoiceAlternatives) => {
                    alternatives.close_verified()
                }
                (
                    Some(
                        ValueShape::Reactive { capabilities, .. }
                        | ValueShape::Store { capabilities, .. },
                    ),
                    ValueClaimDomain::Capabilities,
                ) => capabilities.close_verified(),
                _ => false,
            }
        }
        ClaimPath::Operation { operation, domain } => export
            .call
            .operations
            .iter_mut()
            .find(|candidate| candidate.id == *operation)
            .is_some_and(|operation| match domain {
                OperationClaimDomain::OwnerProductions => {
                    operation.owner.productions.close_verified()
                }
                _ => false,
            }),
        ClaimPath::Resource { resource, domain } => export
            .call
            .resources
            .iter_mut()
            .find(|candidate| candidate.id == *resource)
            .is_some_and(|resource| match domain {
                ResourceClaimDomain::States => resource.states.close_verified(),
                ResourceClaimDomain::Capabilities => resource.capabilities.close_verified(),
                ResourceClaimDomain::Lifetime => false,
            }),
        ClaimPath::GuardPartition => {
            let mut changed = export.call.guards.cases.close_verified();
            for guarded in export.call.guards.cases.items_mut() {
                let operations = match guarded {
                    GuardedCase::When { operations, .. }
                    | GuardedCase::Otherwise { operations } => operations,
                };
                changed |= operations.close_verified();
            }
            changed
        }
    };
    if closed {
        Ok(())
    } else {
        Err(ModelError::InvalidKnowledge {
            path: format!("{claim:?}"),
            reason: "claim is not an open, closable local knowledge leaf".into(),
        })
    }
}

fn value_root_mut<'a>(
    export: &'a mut ExportSemantics,
    root: &ValueRoot,
) -> Option<&'a mut ValueShape> {
    match root {
        ValueRoot::Export => Some(&mut export.shape),
        ValueRoot::OperationInput { operation, index } => export
            .call
            .operations
            .iter_mut()
            .find(|candidate| candidate.id == *operation)?
            .inputs
            .get_mut(usize::from(*index)),
        ValueRoot::OperationOutput { operation } => export
            .call
            .operations
            .iter_mut()
            .find(|candidate| candidate.id == *operation)?
            .output
            .as_mut(),
    }
}

fn value_at_path_mut<'a>(
    mut value: &'a mut ValueShape,
    path: &[ValuePathSegment],
) -> Option<&'a mut ValueShape> {
    for segment in path {
        value = match (value, segment) {
            (ValueShape::Tuple(items), ValuePathSegment::TupleItem(index)) => {
                items.items_mut().get_mut(usize::try_from(*index).ok()?)?
            }
            (ValueShape::Array { element, .. }, ValuePathSegment::ArrayElement) => element,
            (ValueShape::Object(properties), ValuePathSegment::ObjectProperty(name)) => {
                &mut properties
                    .items_mut()
                    .iter_mut()
                    .find(|property| property.name == *name)?
                    .value
            }
            (ValueShape::Choice(alternatives), ValuePathSegment::ChoiceAlternative(index)) => {
                alternatives
                    .items_mut()
                    .get_mut(usize::try_from(*index).ok()?)?
            }
            (ValueShape::Promise(inner), ValuePathSegment::PromiseValue)
            | (ValueShape::AsyncIterable(inner), ValuePathSegment::AsyncIterableElement) => inner,
            _ => return None,
        };
    }
    Some(value)
}

fn open_value_closure(
    value: &mut ValueShape,
    root: ValueRoot,
    path: ValuePath,
    candidates: &mut Vec<ClaimPath>,
) {
    match value {
        ValueShape::Tuple(items) => {
            if items.open_proposed_closure() {
                push_value(
                    candidates,
                    root.clone(),
                    path.clone(),
                    ValueClaimDomain::TupleItems,
                );
            }
            for (index, item) in items.items_mut().iter_mut().enumerate() {
                let mut nested = path.clone();
                nested.0.push(ValuePathSegment::TupleItem(
                    u32::try_from(index).unwrap_or(u32::MAX),
                ));
                open_value_closure(item, root.clone(), nested, candidates);
            }
        }
        ValueShape::Array { element, .. } => {
            let mut nested = path;
            nested.0.push(ValuePathSegment::ArrayElement);
            open_value_closure(element, root, nested, candidates);
        }
        ValueShape::Object(properties) => {
            if properties.open_proposed_closure() {
                push_value(
                    candidates,
                    root.clone(),
                    path.clone(),
                    ValueClaimDomain::ObjectProperties,
                );
            }
            for property in properties.items_mut() {
                let mut nested = path.clone();
                nested
                    .0
                    .push(ValuePathSegment::ObjectProperty(property.name.clone()));
                open_value_closure(&mut property.value, root.clone(), nested, candidates);
            }
        }
        ValueShape::Choice(alternatives) => {
            if alternatives.open_proposed_closure() {
                push_value(
                    candidates,
                    root.clone(),
                    path.clone(),
                    ValueClaimDomain::ChoiceAlternatives,
                );
            }
            for (index, alternative) in alternatives.items_mut().iter_mut().enumerate() {
                let mut nested = path.clone();
                nested.0.push(ValuePathSegment::ChoiceAlternative(
                    u32::try_from(index).unwrap_or(u32::MAX),
                ));
                open_value_closure(alternative, root.clone(), nested, candidates);
            }
        }
        ValueShape::Promise(inner) => {
            let mut nested = path;
            nested.0.push(ValuePathSegment::PromiseValue);
            open_value_closure(inner, root, nested, candidates);
        }
        ValueShape::AsyncIterable(inner) => {
            let mut nested = path;
            nested.0.push(ValuePathSegment::AsyncIterableElement);
            open_value_closure(inner, root, nested, candidates);
        }
        ValueShape::Reactive { capabilities, .. } | ValueShape::Store { capabilities, .. } => {
            if capabilities.open_proposed_closure() {
                push_value(candidates, root, path, ValueClaimDomain::Capabilities);
            }
        }
        ValueShape::Unknown
        | ValueShape::Plain
        | ValueShape::Parameter { .. }
        | ValueShape::Callable
        | ValueShape::Action { .. }
        | ValueShape::Component
        | ValueShape::Cleanup { .. }
        | ValueShape::RefApplication
        | ValueShape::ServerFunctionReference { .. } => {}
    }
}

fn push_operation(
    claims: &mut Vec<ClaimPath>,
    operation: &OperationId,
    domain: OperationClaimDomain,
) {
    claims.push(ClaimPath::Operation {
        operation: operation.clone(),
        domain,
    });
}

fn visit_value(value: &ValueShape, root: ValueRoot, path: ValuePath, claims: &mut Vec<ClaimPath>) {
    match value {
        ValueShape::Unknown => push_value(claims, root, path, ValueClaimDomain::Shape),
        ValueShape::Plain
        | ValueShape::Parameter { .. }
        | ValueShape::Callable
        | ValueShape::Action { .. }
        | ValueShape::Component
        | ValueShape::Cleanup { .. }
        | ValueShape::RefApplication
        | ValueShape::ServerFunctionReference { .. } => {}
        ValueShape::Tuple(items) => {
            if items.state().is_open() {
                push_value(
                    claims,
                    root.clone(),
                    path.clone(),
                    ValueClaimDomain::TupleItems,
                );
            }
            for (index, item) in items.items().iter().enumerate() {
                let mut nested = path.clone();
                nested.0.push(ValuePathSegment::TupleItem(
                    u32::try_from(index).unwrap_or(u32::MAX),
                ));
                visit_value(item, root.clone(), nested, claims);
            }
        }
        ValueShape::Array { element, length } => {
            if length.min.is_none() {
                push_value(
                    claims,
                    root.clone(),
                    path.clone(),
                    ValueClaimDomain::ArrayMinimumLength,
                );
            }
            if length.max.is_none() {
                push_value(
                    claims,
                    root.clone(),
                    path.clone(),
                    ValueClaimDomain::ArrayMaximumLength,
                );
            }
            let mut nested = path;
            nested.0.push(ValuePathSegment::ArrayElement);
            visit_value(element, root, nested, claims);
        }
        ValueShape::Object(properties) => {
            if properties.state().is_open() {
                push_value(
                    claims,
                    root.clone(),
                    path.clone(),
                    ValueClaimDomain::ObjectProperties,
                );
            }
            for property in properties.items() {
                let mut nested = path.clone();
                nested
                    .0
                    .push(ValuePathSegment::ObjectProperty(property.name.clone()));
                visit_value(&property.value, root.clone(), nested, claims);
            }
        }
        ValueShape::Choice(alternatives) => {
            if alternatives.state().is_open() {
                push_value(
                    claims,
                    root.clone(),
                    path.clone(),
                    ValueClaimDomain::ChoiceAlternatives,
                );
            }
            for (index, alternative) in alternatives.items().iter().enumerate() {
                let mut nested = path.clone();
                nested.0.push(ValuePathSegment::ChoiceAlternative(
                    u32::try_from(index).unwrap_or(u32::MAX),
                ));
                visit_value(alternative, root.clone(), nested, claims);
            }
        }
        ValueShape::Promise(inner) => {
            let mut nested = path;
            nested.0.push(ValuePathSegment::PromiseValue);
            visit_value(inner, root, nested, claims);
        }
        ValueShape::AsyncIterable(inner) => {
            let mut nested = path;
            nested.0.push(ValuePathSegment::AsyncIterableElement);
            visit_value(inner, root, nested, claims);
        }
        ValueShape::Reactive { capabilities, .. } | ValueShape::Store { capabilities, .. } => {
            if capabilities.state().is_open() {
                push_value(claims, root, path, ValueClaimDomain::Capabilities);
            }
        }
    }
}

fn push_value(
    claims: &mut Vec<ClaimPath>,
    root: ValueRoot,
    path: ValuePath,
    domain: ValueClaimDomain,
) {
    claims.push(ClaimPath::Value { root, path, domain });
}
