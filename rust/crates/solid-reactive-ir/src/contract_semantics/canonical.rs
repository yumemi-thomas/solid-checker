use sha2::{Digest as _, Sha256};

use super::*;

pub(super) fn semantic_digest(
    package: &PackageIdentity,
    artifact_cases: &[ArtifactCase],
) -> Digest {
    let mut writer = CanonicalWriter::new();
    writer.text("solid-checker:normalized-package-contract");
    writer.u16(SEMANTIC_MODEL_VERSION);
    writer.package(package);
    writer.sequence(artifact_cases, CanonicalWriter::artifact_case);
    Digest::from_sha256(writer.finish())
}

struct CanonicalWriter(Sha256);

impl CanonicalWriter {
    fn new() -> Self {
        Self(Sha256::new())
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.0
            .update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
        self.0.update(bytes);
    }

    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn bool(&mut self, value: bool) {
        self.0.update([u8::from(value)]);
    }

    fn u8(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u16(&mut self, value: u16) {
        self.0.update(value.to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn usize(&mut self, value: usize) {
        self.0
            .update(u64::try_from(value).unwrap_or(u64::MAX).to_be_bytes());
    }

    fn option<T>(&mut self, value: Option<&T>, encode: impl FnOnce(&mut Self, &T)) {
        match value {
            Some(value) => {
                self.u8(1);
                encode(self, value);
            }
            None => self.u8(0),
        }
    }

    fn sequence<T>(&mut self, values: &[T], mut encode: impl FnMut(&mut Self, &T)) {
        self.usize(values.len());
        for value in values {
            encode(self, value);
        }
    }

    fn knowledge<T>(&mut self, knowledge: &KnowledgeSet<T>, mut encode: impl FnMut(&mut Self, &T)) {
        match knowledge {
            KnowledgeSet::Unknown => self.u8(0),
            KnowledgeSet::Partial(items) => {
                self.u8(1);
                self.sequence(items, &mut encode);
            }
            KnowledgeSet::Complete(items) if items.is_empty() => {
                self.u8(2);
                self.usize(0);
            }
            KnowledgeSet::Complete(items) => {
                self.u8(3);
                self.sequence(items, &mut encode);
            }
        }
    }

    fn digest(&mut self, digest: &Digest) {
        self.text(digest.as_str());
    }

    fn artifact(&mut self, artifact: &ArtifactIdentity) {
        self.text(&artifact.path);
        self.digest(&artifact.digest);
    }

    fn package(&mut self, package: &PackageIdentity) {
        self.text(&package.name);
        self.text(&package.version);
        self.text(&package.integrity);
        self.artifact(&package.manifest);
    }

    fn artifact_case(&mut self, case: &ArtifactCase) {
        self.text(&case.id);
        self.text(&case.entrypoint);
        self.sequence(&case.resolution_trace, |writer, step| {
            writer.text(&step.condition);
            writer.text(&step.target);
        });
        self.artifact(&case.runtime);
        self.artifact(&case.declarations);
        self.digest(&case.dependency_closure);
        self.option(case.transform.as_ref(), Self::artifact);
        self.stability(case.stability);
        self.usize(case.exports.len());
        for (name, export) in &case.exports {
            self.text(name);
            self.export(export);
        }
    }

    fn export(&mut self, export: &ExportSemantics) {
        self.export_identity(&export.identity);
        self.value(&export.shape);
        self.stability(export.stability);
        self.call(&export.call);
    }

    fn export_identity(&mut self, identity: &ExportIdentity) {
        self.text(&identity.entrypoint);
        self.text(&identity.public_name);
        self.export_target(&identity.runtime);
        self.export_target(&identity.declarations);
    }

    fn export_target(&mut self, target: &ExportTargetIdentity) {
        self.artifact(&target.module);
        self.text(&target.export_name);
    }

    fn stability(&mut self, stability: StabilityKnowledge) {
        self.u8(match stability {
            StabilityKnowledge::Unknown => 0,
            StabilityKnowledge::Experimental => 1,
        });
    }

    fn call(&mut self, call: &CallSemantics) {
        self.call_claims(&call.claims);
        self.sequence(&call.operations, Self::operation);
        self.sequence(&call.edges, Self::edge);
        self.sequence(&call.resources, Self::resource);
        self.guard_partition(&call.guards);
    }

    fn call_claims(&mut self, claims: &CallClaims) {
        self.knowledge(&claims.callbacks, Self::callback);
        self.knowledge(&claims.reads, Self::operation_id);
        self.knowledge(&claims.writes, Self::operation_id);
        self.knowledge(&claims.creates, Self::operation_id);
        self.knowledge(&claims.invalidates, Self::operation_id);
        self.knowledge(&claims.throws, Self::operation_id);
        self.knowledge(&claims.returns, Self::operation_id);
        self.knowledge(&claims.cleanups, Self::operation_id);
        self.knowledge(&claims.disposals, Self::operation_id);
    }

    fn callback(&mut self, callback: &CallbackInvocation) {
        self.value_source(&callback.from);
        self.operation_id(&callback.operation);
    }

    fn value_source(&mut self, source: &ValueSource) {
        match source {
            ValueSource::Parameter { index, path } => {
                self.u8(0);
                self.u16(*index);
                self.sequence(path, |writer, value| writer.text(value));
            }
            ValueSource::OperationOutput { operation, path } => {
                self.u8(1);
                self.operation_id(operation);
                self.sequence(path, |writer, value| writer.text(value));
            }
            ValueSource::Resource { resource, path } => {
                self.u8(2);
                self.resource_id(resource);
                self.sequence(path, |writer, value| writer.text(value));
            }
        }
    }

    fn operation_id(&mut self, id: &OperationId) {
        self.text(&id.0);
    }

    fn resource_id(&mut self, id: &ResourceId) {
        self.text(&id.0);
    }

    fn operation(&mut self, operation: &Operation) {
        self.operation_id(&operation.id);
        self.operation_kind(operation.kind);
        self.option(operation.guard.as_ref(), Self::guard);
        self.option(operation.trigger.as_ref(), Self::trigger);
        self.option(operation.at.as_ref(), |writer, event| writer.event(*event));
        self.option(operation.schedule.as_ref(), |writer, schedule| {
            writer.schedule(*schedule);
        });
        self.tracking(operation.tracking);
        self.owner(&operation.owner);
        self.cardinality(&operation.cardinality);
        self.sequence(&operation.inputs, Self::value);
        self.option(operation.output.as_ref(), Self::value);
        self.usize(operation.resources.len());
        for resource in &operation.resources {
            self.resource_id(resource);
        }
    }

    fn operation_kind(&mut self, kind: OperationKind) {
        self.u8(match kind {
            OperationKind::Invoke => 0,
            OperationKind::Return => 1,
            OperationKind::Read => 2,
            OperationKind::Write => 3,
            OperationKind::Invalidate => 4,
            OperationKind::Create => 5,
            OperationKind::Cleanup => 6,
            OperationKind::Dispose => 7,
        });
    }

    fn event(&mut self, event: Event) {
        self.u8(match event {
            Event::Call => 0,
            Event::Render => 1,
            Event::Flush => 2,
            Event::Settle => 3,
            Event::Transition => 4,
            Event::AsyncEmission => 5,
            Event::Cleanup => 6,
            Event::External => 7,
            Event::Request => 8,
            Event::ResponseCommitment => 9,
        });
    }

    fn trigger(&mut self, trigger: &Trigger) {
        match trigger {
            Trigger::Event(event) => {
                self.u8(0);
                self.event(*event);
            }
            Trigger::Operation(operation) => {
                self.u8(1);
                self.operation_id(operation);
            }
            Trigger::Resource { resource, event } => {
                self.u8(2);
                self.resource_id(resource);
                self.event(*event);
            }
        }
    }

    fn schedule(&mut self, schedule: Schedule) {
        self.u8(match schedule {
            Schedule::SameStack => 0,
            Schedule::Queued => 1,
            Schedule::External => 2,
        });
    }

    fn tracking(&mut self, tracking: Tracking) {
        self.u8(match tracking {
            Tracking::Tracked => 0,
            Tracking::Untracked => 1,
            Tracking::AmbientAtExecution => 2,
            Tracking::Unknown => 3,
        });
    }

    fn owner(&mut self, owner: &OwnerRelation) {
        self.owner_source(&owner.source);
        self.requirement(owner.requirements.owner);
        self.requirement(owner.requirements.child_owners);
        self.requirement(owner.requirements.cleanup);
        self.owner_capabilities(&owner.capabilities);
        self.option(owner.lifetime.as_ref(), Self::lifetime);
        self.knowledge(&owner.productions, |writer, production| {
            writer.resource_id(&production.resource);
            writer.owner_capabilities(&production.capabilities);
            writer.option(production.lifetime.as_ref(), Self::lifetime);
        });
    }

    fn owner_source(&mut self, source: &OwnerSource) {
        match source {
            OwnerSource::None => self.u8(0),
            OwnerSource::AmbientAtCall => self.u8(1),
            OwnerSource::AmbientAtExecution => self.u8(2),
            OwnerSource::Captured(resource) => {
                self.u8(3);
                self.resource_id(resource);
            }
            OwnerSource::Created(resource) => {
                self.u8(4);
                self.resource_id(resource);
            }
            OwnerSource::Unknown => self.u8(5),
        }
    }

    fn requirement(&mut self, requirement: Requirement) {
        self.u8(match requirement {
            Requirement::Required => 0,
            Requirement::Forbidden => 1,
            Requirement::Unconstrained => 2,
        });
    }

    fn owner_capabilities(&mut self, capabilities: &OwnerCapabilities) {
        self.capability_knowledge(capabilities.child_owners);
        self.capability_knowledge(capabilities.cleanup);
    }

    fn capability_knowledge(&mut self, capability: CapabilityKnowledge) {
        self.u8(match capability {
            CapabilityKnowledge::Allowed => 0,
            CapabilityKnowledge::Forbidden => 1,
            CapabilityKnowledge::Unknown => 2,
        });
    }

    fn lifetime(&mut self, lifetime: &Lifetime) {
        match lifetime {
            Lifetime::Call => self.u8(0),
            Lifetime::Resource(resource) => {
                self.u8(1);
                self.resource_id(resource);
            }
            Lifetime::Owner(resource) => {
                self.u8(2);
                self.resource_id(resource);
            }
            Lifetime::Request(resource) => {
                self.u8(3);
                self.resource_id(resource);
            }
            Lifetime::Transition(resource) => {
                self.u8(4);
                self.resource_id(resource);
            }
            Lifetime::AsyncSource(resource) => {
                self.u8(5);
                self.resource_id(resource);
            }
        }
    }

    fn cardinality(&mut self, cardinality: &Cardinality) {
        self.option(cardinality.scope.as_ref(), |writer, scope| match scope {
            CardinalityScope::Trigger => writer.u8(0),
            CardinalityScope::Call => writer.u8(1),
            CardinalityScope::Resource(resource) => {
                writer.u8(2);
                writer.resource_id(resource);
            }
        });
        self.option(cardinality.min.as_ref(), |writer, min| writer.u32(*min));
        self.option(cardinality.max.as_ref(), |writer, max| match max {
            UpperBound::Finite(max) => {
                writer.u8(0);
                writer.u32(*max);
            }
            UpperBound::Many => writer.u8(1),
        });
    }

    fn edge(&mut self, edge: &OperationEdge) {
        self.u8(match edge.kind {
            EdgeKind::Orders => 0,
            EdgeKind::Data => 1,
            EdgeKind::Invalidates => 2,
            EdgeKind::Error => 3,
            EdgeKind::Cleanup => 4,
            EdgeKind::Lifetime => 5,
        });
        self.operation_id(&edge.from);
        self.operation_id(&edge.to);
    }

    fn resource(&mut self, resource: &Resource) {
        self.resource_id(&resource.id);
        self.u8(match resource.kind {
            ResourceKind::Owner => 0,
            ResourceKind::ReactiveSource => 1,
            ResourceKind::AsyncComputation => 2,
            ResourceKind::Transition => 3,
            ResourceKind::Cleanup => 4,
            ResourceKind::Request => 5,
            ResourceKind::Response => 6,
            ResourceKind::Stream => 7,
            ResourceKind::ServerFunctionReference => 8,
        });
        self.knowledge(&resource.states, |writer, state| {
            writer.resource_state(*state)
        });
        self.knowledge(&resource.capabilities, |writer, capability| {
            writer.u8(match capability {
                ResourceCapability::Refreshable => 0,
                ResourceCapability::Writable => 1,
            });
        });
        self.option(resource.lifetime.as_ref(), Self::lifetime);
    }

    fn resource_state(&mut self, state: ResourceState) {
        self.u8(match state {
            ResourceState::OwnerActive => 0,
            ResourceState::OwnerDisposed => 1,
            ResourceState::CleanupInstalled => 2,
            ResourceState::CleanupDisposed => 3,
            ResourceState::AsyncPending => 4,
            ResourceState::AsyncSettled => 5,
            ResourceState::AsyncErrored => 6,
            ResourceState::AsyncCancelled => 7,
            ResourceState::TransitionActive => 8,
            ResourceState::TransitionSettled => 9,
            ResourceState::TransitionReverted => 10,
            ResourceState::ResponseUncommitted => 11,
            ResourceState::ResponseCommitted => 12,
            ResourceState::StreamUnclaimed => 13,
            ResourceState::StreamClaimed => 14,
        });
    }

    fn guard_partition(&mut self, partition: &GuardPartition) {
        self.knowledge(&partition.cases, |writer, case| match case {
            GuardedCase::When { guard, operations } => {
                writer.u8(0);
                writer.guard(guard);
                writer.knowledge(operations, Self::operation_id);
            }
            GuardedCase::Otherwise { operations } => {
                writer.u8(1);
                writer.knowledge(operations, Self::operation_id);
            }
        });
    }

    fn guard(&mut self, guard: &Guard) {
        self.sequence(&guard.0, Self::guard_atom);
    }

    fn guard_atom(&mut self, atom: &GuardAtom) {
        match atom {
            GuardAtom::Signature(signature) => {
                self.u8(0);
                self.text(signature);
            }
            GuardAtom::ArgumentCount { min, max } => {
                self.u8(1);
                self.u16(*min);
                self.option(max.as_ref(), |writer, max| writer.u16(*max));
            }
            GuardAtom::Literal {
                argument,
                path,
                value,
            } => {
                self.u8(2);
                self.u16(*argument);
                self.sequence(path, |writer, value| writer.text(value));
                self.literal(value);
            }
            GuardAtom::ValueKind {
                argument,
                path,
                kind,
            } => {
                self.u8(3);
                self.u16(*argument);
                self.sequence(path, |writer, value| writer.text(value));
                self.value_kind(*kind);
            }
            GuardAtom::Property {
                argument,
                path,
                name,
                callable,
            } => {
                self.u8(4);
                self.u16(*argument);
                self.sequence(path, |writer, value| writer.text(value));
                self.text(name);
                self.option(callable.as_ref(), |writer, callable| writer.bool(*callable));
            }
            GuardAtom::TupleAlternative {
                argument,
                alternative,
            } => {
                self.u8(5);
                self.u16(*argument);
                self.u16(*alternative);
            }
            GuardAtom::ResultProtocol(kind) => {
                self.u8(6);
                self.value_kind(*kind);
            }
            GuardAtom::ArtifactCase(case) => {
                self.u8(7);
                self.text(case);
            }
        }
    }

    fn literal(&mut self, literal: &Literal) {
        match literal {
            Literal::Null => self.u8(0),
            Literal::Bool(value) => {
                self.u8(1);
                self.bool(*value);
            }
            Literal::Number(value) => {
                self.u8(2);
                self.text(value);
            }
            Literal::String(value) => {
                self.u8(3);
                self.text(value);
            }
        }
    }

    fn value_kind(&mut self, kind: ValueKind) {
        self.u8(match kind {
            ValueKind::Plain => 0,
            ValueKind::Callable => 1,
            ValueKind::Promise => 2,
            ValueKind::AsyncIterable => 3,
        });
    }

    fn value(&mut self, value: &ValueShape) {
        match value {
            ValueShape::Unknown => self.u8(0),
            ValueShape::Plain => self.u8(1),
            ValueShape::Parameter { index, path } => {
                self.u8(2);
                self.u16(*index);
                self.sequence(path, |writer, value| writer.text(value));
            }
            ValueShape::Tuple(items) => {
                self.u8(3);
                self.knowledge(items, Self::value);
            }
            ValueShape::Array { element, length } => {
                self.u8(4);
                self.value(element);
                self.option(length.min.as_ref(), |writer, min| writer.u32(*min));
                self.option(length.max.as_ref(), |writer, max| match max {
                    UpperBound::Finite(max) => {
                        writer.u8(0);
                        writer.u32(*max);
                    }
                    UpperBound::Many => writer.u8(1),
                });
            }
            ValueShape::Object(properties) => {
                self.u8(5);
                self.knowledge(properties, |writer, property| {
                    writer.text(&property.name);
                    writer.value(&property.value);
                });
            }
            ValueShape::Choice(alternatives) => {
                self.u8(6);
                self.knowledge(alternatives, Self::value);
            }
            ValueShape::Callable => self.u8(7),
            ValueShape::Promise(value) => {
                self.u8(8);
                self.value(value);
            }
            ValueShape::AsyncIterable(value) => {
                self.u8(9);
                self.value(value);
            }
            ValueShape::Reactive {
                role,
                resource,
                capabilities,
            } => {
                self.u8(10);
                self.u8(match role {
                    ReactiveRole::Accessor => 0,
                    ReactiveRole::Setter => 1,
                });
                self.option(resource.as_ref(), Self::resource_id);
                self.capabilities(capabilities);
            }
            ValueShape::Store {
                resource,
                capabilities,
            } => {
                self.u8(11);
                self.option(resource.as_ref(), Self::resource_id);
                self.capabilities(capabilities);
            }
            ValueShape::Action { transition } => {
                self.u8(12);
                self.option(transition.as_ref(), Self::resource_id);
            }
            ValueShape::Component => self.u8(13),
            ValueShape::Cleanup { resource, lifetime } => {
                self.u8(14);
                self.option(resource.as_ref(), Self::resource_id);
                self.option(lifetime.as_ref(), Self::lifetime);
            }
            ValueShape::RefApplication => self.u8(15),
            ValueShape::ServerFunctionReference { resource } => {
                self.u8(16);
                self.option(resource.as_ref(), Self::resource_id);
            }
        }
    }

    fn capabilities(&mut self, capabilities: &KnowledgeSet<CapabilityClaim>) {
        self.knowledge(capabilities, |writer, claim| {
            writer.u8(match claim.capability {
                ObservableCapability::Readable => 0,
                ObservableCapability::Writable => 1,
                ObservableCapability::Refreshable => 2,
                ObservableCapability::PendingAware => 3,
                ObservableCapability::Optimistic => 4,
            });
            writer.option(claim.resource.as_ref(), Self::resource_id);
        });
    }
}
