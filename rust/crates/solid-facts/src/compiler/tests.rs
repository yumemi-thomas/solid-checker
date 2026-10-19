use super::*;

fn producer(source: &str) -> CompilerProducerInput {
    let source_hash = SourceHash::of(source);
    CompilerProducerInput {
        dialect: "test".into(),
        trace_version: 1,
        package_version: None,
        upstream_revision: None,
        implementation_revision: "test-revision".into(),
        source_sha256: source_hash.as_str().strip_prefix("sha256:").unwrap().into(),
        output: "output".into(),
        source_map: None,
        claimed_output_sha256: None,
        claimed_source_map_sha256: None,
        configuration_json: "{}".into(),
        identity_complete: false,
    }
}

fn facts(source: &str, kind: CompilerOperationKind) -> ExecutionMap {
    ExecutionMap::normalize(
        CompilerTraceInput {
            producer: producer(source),
            source_operations_complete: true,
            generated_operations_complete: false,
            operations: vec![CompilerOperationInput::legacy(
                Span::new(0, 5),
                kind,
                LegacyCompilerDecision::ReactiveRerun,
                true,
            )],
            generated_operations: Vec::new(),
        },
        source,
    )
    .unwrap()
}

fn encoded(source: &str) -> String {
    serde_json::to_string(&facts(source, CompilerOperationKind::JsxChild)).unwrap()
}

#[test]
fn validates_execution_map_and_completeness() {
    let source = "value";
    let facts = ExecutionMap::from_json(&encoded(source), source).unwrap();
    assert_eq!(facts.seed_spans(), vec![Span::new(0, 5)]);
}

#[test]
fn rejects_a_compatibility_projection_that_disagrees_with_operations() {
    let source = "value";
    let mut facts = ExecutionMap::from_json(&encoded(source), source).unwrap();
    facts.tracked_regions.clear();
    assert!(matches!(
        facts.validate(source),
        Err(CompilerFactsError::CompatibilityProjection)
    ));
}

#[test]
fn all_component_value_operations_cover_jsx_expressions() {
    for kind in [
        CompilerOperationKind::ComponentProperty,
        CompilerOperationKind::ComponentSpread,
        CompilerOperationKind::ComponentChild,
    ] {
        let source = "value";
        let facts = facts(source, kind);
        assert!(facts.classifies(Span::new(1, 2)), "kind {kind:?}");
    }
}

#[test]
fn unfamiliar_ownership_kinds_cannot_override_protocol_two_operations() {
    let source = "value";
    let encoded = encoded(source).replace(r#""kind":"owned""#, r#""kind":"future-state""#);
    assert!(ExecutionMap::from_json(&encoded, source).is_err());
}

#[test]
fn rejects_stale_source() {
    assert!(matches!(
        ExecutionMap::from_json(&encoded("value"), "other"),
        Err(CompilerFactsError::SourceHash { .. })
    ));
}

#[test]
fn normalization_recomputes_and_checks_generated_output_identity() {
    let source = "value";
    let mut producer = producer(source);
    producer.claimed_output_sha256 = Some("0".repeat(64));
    let error = ExecutionMap::normalize(
        CompilerTraceInput {
            producer,
            source_operations_complete: true,
            generated_operations_complete: true,
            operations: Vec::new(),
            generated_operations: Vec::new(),
        },
        source,
    )
    .expect_err("a producer cannot certify bytes it did not emit");
    assert!(matches!(error, CompilerFactsError::ProducerIdentity(_)));
}

#[test]
fn generated_operation_references_are_foreign_key_checked() {
    let source = "value";
    let error = ExecutionMap::normalize(
        CompilerTraceInput {
            producer: producer(source),
            source_operations_complete: true,
            generated_operations_complete: true,
            operations: vec![CompilerOperationInput {
                id: "source:0".into(),
                span: Span::new(0, 5),
                kind: CompilerOperationKind::JsxChild,
                evidence: CompilerOperationEvidence::Rich(CompilerExecutionSemantics {
                    disposition: CompilerExecutionDisposition::Unknown,
                    trigger: CompilerExecutionTrigger::Unknown,
                    schedule: CompilerExecutionSchedule::Unknown,
                    tracking: CompilerTrackingRelation::Unknown,
                    cardinality: CompilerExecutionCardinality::Unknown,
                    owner: CompilerOwnerRelation::Unknown,
                    generated_operations: vec!["missing:0".into()],
                }),
            }],
            generated_operations: Vec::new(),
        },
        source,
    )
    .expect_err("a source operation cannot cite invented generated code");
    assert!(matches!(
        error,
        CompilerFactsError::MissingGeneratedOperation { .. }
    ));
}

#[test]
fn uncertainty_is_local_to_each_execution_axis() {
    let source = "value";
    let facts = ExecutionMap::normalize(
        CompilerTraceInput {
            producer: producer(source),
            source_operations_complete: true,
            generated_operations_complete: true,
            operations: vec![CompilerOperationInput {
                id: "source:0".into(),
                span: Span::new(0, 5),
                kind: CompilerOperationKind::Ref,
                evidence: CompilerOperationEvidence::Rich(CompilerExecutionSemantics {
                    disposition: CompilerExecutionDisposition::RefFactory,
                    trigger: CompilerExecutionTrigger::Render,
                    schedule: CompilerExecutionSchedule::Inline,
                    tracking: CompilerTrackingRelation::Untracked,
                    cardinality: CompilerExecutionCardinality::ZeroOrOne,
                    owner: CompilerOwnerRelation::Unknown,
                    generated_operations: Vec::new(),
                }),
            }],
            generated_operations: Vec::new(),
        },
        source,
    )
    .expect("an unknown owner must not erase known timing or tracking");
    let execution = &facts.semantic_model.operations[0].execution;
    assert_eq!(execution.schedule, CompilerExecutionSchedule::Inline);
    assert_eq!(execution.tracking, CompilerTrackingRelation::Untracked);
    assert_eq!(execution.owner, CompilerOwnerRelation::Unknown);
    assert!(facts.untracked_regions.is_empty());
}

#[test]
fn contradictory_generated_operation_axes_are_refused() {
    let source = "value";
    let error = ExecutionMap::normalize(
        CompilerTraceInput {
            producer: producer(source),
            source_operations_complete: true,
            generated_operations_complete: false,
            operations: Vec::new(),
            generated_operations: vec![GeneratedCompilerOperation {
                id: "g0".into(),
                source_id: "s:0:5:event".into(),
                source_span: Span::new(0, 5),
                kind: GeneratedCompilerOperationKind::DelegatedEvent,
                trigger: CompilerExecutionTrigger::Event,
                schedule: CompilerExecutionSchedule::Inline,
                tracking: CompilerTrackingRelation::Untracked,
                cardinality: CompilerExecutionCardinality::ZeroOrMore,
                owner: CompilerOwnerRelation::None,
                receiver_span: None,
                group_id: None,
                wrapper: Some("delegated".into()),
            }],
        },
        source,
    )
    .expect_err("an event operation cannot claim inline scheduling");
    assert!(matches!(
        error,
        CompilerFactsError::ContradictoryGeneratedOperation(_)
    ));
}

#[test]
fn component_child_creation_keeps_its_deferred_body_projection() {
    let source = "value";
    let facts = ExecutionMap::normalize(
        CompilerTraceInput {
            producer: producer(source),
            source_operations_complete: true,
            generated_operations_complete: false,
            operations: vec![CompilerOperationInput {
                id: "source:0".into(),
                span: Span::new(0, 5),
                kind: CompilerOperationKind::ComponentChild,
                evidence: CompilerOperationEvidence::Rich(CompilerExecutionSemantics {
                    disposition: CompilerExecutionDisposition::EagerOnce,
                    trigger: CompilerExecutionTrigger::Render,
                    schedule: CompilerExecutionSchedule::Inline,
                    tracking: CompilerTrackingRelation::Untracked,
                    cardinality: CompilerExecutionCardinality::ExactlyOnce,
                    owner: CompilerOwnerRelation::AmbientAtTransformSite,
                    generated_operations: Vec::new(),
                }),
            }],
            generated_operations: Vec::new(),
        },
        source,
    )
    .expect("component child facts");
    assert_eq!(
        facts.callback_roles,
        vec![CallbackRole {
            span: Span::new(0, 5),
            role: CallbackRoleKind::Deferred,
        }]
    );
    assert!(facts.untracked_regions.is_empty());
}
