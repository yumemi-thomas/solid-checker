//! Execute the generated observation against real JavaScript counterexamples.
//! These tests never turn finite non-observation into an implementation proof.

use std::io::Write as _;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use solid_reactive_ir::contract_semantics::{
    ArtifactIdentity, CallClaims, CallSemantics, Cardinality, Digest, Event, ExportIdentity,
    ExportSemantics, ExportTargetIdentity, Guard, GuardPartition, KnowledgeSet, Operation,
    OperationId, OperationKind, OwnerRelation, Schedule, StabilityKnowledge, Tracking, Trigger,
    ValueShape,
};

use super::*;

fn value_fact(primitive: Value) -> Value {
    json!({
        "callability": "nonCallable",
        "constructability": "nonConstructable",
        "primitive": primitive,
    })
}

fn signature(values: &[Value]) -> typefacts::SelectedSignature {
    serde_json::from_value(json!({
        "identity": "identity-signature",
        "declaration": {
            "kind": "FunctionDeclaration",
            "location": {"path": "/package/index.d.ts", "startByte": 0, "endByte": 80},
        },
        "overloadOrdinal": 0,
        "overloadCount": 1,
        "minimumArgumentCount": values.len(),
        "parameters": values.iter().enumerate().map(|(index, value)| {
            json!({"index": index, "value": value})
        }).collect::<Vec<_>>(),
        "result": value_fact(json!({"unknown": true})),
    }))
    .unwrap()
}

fn data_module(source: &str) -> String {
    let encoded = source
        .bytes()
        .map(|byte| format!("%{byte:02X}"))
        .collect::<String>();
    format!("data:text/javascript,{encoded}")
}

#[derive(Debug)]
struct ObservationResult {
    markers: Vec<String>,
    error: Option<String>,
}

impl ObservationResult {
    fn contradicted(&self) -> bool {
        self.markers
            .iter()
            .any(|marker| matches!(marker.as_str(), "return-value" | "return-outside-identity"))
    }
}

fn execute(
    implementation: &str,
    observation: Observation,
    signatures: &[typefacts::SelectedSignature],
) -> ObservationResult {
    let mut source = module_source(
        &data_module(implementation),
        "subject",
        observation,
        signatures,
    );
    source.push_str(
        r#"
const observedEvents = [];
let observedError = null;
try {
  await runProbeSession({}, { emit(event) { observedEvents.push(event); } });
} catch (error) {
  observedError = String(error);
}
process.stdout.write(JSON.stringify({ events: observedEvents, error: observedError }));
"#,
    );
    let mut child = Command::new("node")
        .arg("--input-type=module")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("generated-veto observation tests require Node");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "generated module failed to execute: {}\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    ObservationResult {
        markers: result["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["marker"].as_str().unwrap().to_owned())
            .collect(),
        error: result["error"].as_str().map(str::to_owned),
    }
}

#[test]
fn identity_veto_accepts_original_object_but_detects_copies_and_replacement() {
    let signatures = [signature(&[value_fact(json!({"mayBeObject": true}))])];
    for implementation in [
        "export function subject(value) { return value; }",
        "export function subject(value) { value.changed = true; return value; }",
    ] {
        let observed = execute(implementation, Observation::ParameterReturn(0), &signatures);
        assert!(!observed.contradicted(), "{implementation}: {observed:?}");
        assert!(observed.error.is_none(), "{observed:?}");
    }
    for implementation in [
        "export function subject(value) { return { ...value }; }",
        "export function subject(value) { value = {}; return value; }",
        "export function subject() {}",
    ] {
        let observed = execute(implementation, Observation::ParameterReturn(0), &signatures);
        assert!(observed.contradicted(), "{implementation}: {observed:?}");
        assert!(observed.error.is_none(), "{observed:?}");
    }
}

#[test]
fn identity_veto_ignores_object_is_replaced_during_subject_import() {
    let signatures = [signature(&[value_fact(json!({"mayBeObject": true}))])];
    for (implementation, contradiction) in [
        (
            "Object.is = () => true; export function subject(value) { return { ...value }; }",
            true,
        ),
        (
            "Object.is = () => false; export function subject(value) { return value; }",
            false,
        ),
    ] {
        let observed = execute(implementation, Observation::ParameterReturn(0), &signatures);
        assert_eq!(observed.contradicted(), contradiction, "{observed:?}");
        assert!(observed.error.is_none(), "{observed:?}");
    }
}

#[test]
fn identity_veto_distinguishes_parameter_slots_for_each_sampled_value_kind() {
    let mut callable = value_fact(json!({"mayBeObject": true}));
    callable["callability"] = json!("callable");
    for value in [
        value_fact(json!({"mayBeObject": true})),
        callable,
        value_fact(json!({"mayBeString": true})),
        value_fact(json!({"mayBeNumber": true})),
        value_fact(json!({"mayBeBoolean": true})),
        value_fact(json!({"mayBeBigInt": true})),
        value_fact(json!({"mayBeSymbol": true})),
    ] {
        let signatures = [signature(&[value.clone(), value.clone()])];
        let implementation = "export function subject(first, second) { return second; }";
        let wrong = execute(implementation, Observation::ParameterReturn(0), &signatures);
        assert!(
            wrong.contradicted(),
            "wrong argument for {value}: {wrong:?}"
        );
        let correct = execute(implementation, Observation::ParameterReturn(1), &signatures);
        assert!(!correct.contradicted(), "{value}: {correct:?}");
        assert!(correct.error.is_none(), "{value}: {correct:?}");
    }
}

#[test]
fn identity_veto_samples_only_symbols_for_a_symbol_only_parameter() {
    let signatures = [signature(&[value_fact(json!({"mayBeSymbol": true}))])];
    for implementation in [
        "export function subject(value) { if (typeof value !== 'symbol') throw new Error('requires symbol'); return value; }",
        "export function subject(value) { return typeof value === 'symbol' ? value : {}; }",
    ] {
        let observed = execute(implementation, Observation::ParameterReturn(0), &signatures);
        assert!(!observed.contradicted(), "{implementation}: {observed:?}");
        assert!(observed.error.is_none(), "{observed:?}");
        assert!(
            !observed
                .markers
                .iter()
                .any(|marker| marker == "sample-threw"),
            "every sample must belong to the symbol-only input domain: {observed:?}",
        );
    }
}

#[test]
fn identity_veto_preserves_nan_identity_and_distinguishes_signed_zero() {
    let signatures = [signature(&[value_fact(json!({"mayBeNumber": true}))])];
    let nan = execute(
        "export function subject(value) { if (!Number.isNaN(value)) throw new Error('not NaN'); return value; }",
        Observation::ParameterReturn(0),
        &signatures,
    );
    assert!(!nan.contradicted(), "Object.is(NaN, NaN) is true: {nan:?}");
    assert!(nan.error.is_none(), "a NaN sample must complete: {nan:?}");
    for implementation in [
        "export function subject(value) { return Object.is(value, -0) ? 0 : value; }",
        "export function subject(value) { return Number.isNaN(value) ? 0 : value; }",
    ] {
        let observed = execute(implementation, Observation::ParameterReturn(0), &signatures);
        assert!(observed.contradicted(), "{implementation}: {observed:?}");
    }
}

#[test]
fn identity_veto_requires_a_normal_completion_but_tolerates_throwing_samples() {
    let signatures = [signature(&[value_fact(json!({"mayBeNumber": true}))])];
    let no_observation = execute(
        "export function subject() { throw new Error('sample rejected'); }",
        Observation::ParameterReturn(0),
        &signatures,
    );
    assert!(!no_observation.contradicted(), "{no_observation:?}");
    assert!(no_observation.error.is_some(), "{no_observation:?}");
    assert!(no_observation.markers.iter().any(|m| m == "sample-threw"));

    let partial = execute(
        "export function subject(value) { if (Number.isNaN(value)) throw new Error('sample rejected'); return value; }",
        Observation::ParameterReturn(0),
        &signatures,
    );
    assert!(!partial.contradicted(), "{partial:?}");
    assert!(partial.error.is_none(), "{partial:?}");
    assert!(partial.markers.iter().any(|m| m == "sample-threw"));

    let contradiction = execute(
        "export function subject(value) { if (Number.isNaN(value)) throw new Error('sample rejected'); return {}; }",
        Observation::ParameterReturn(0),
        &signatures,
    );
    assert!(contradiction.contradicted(), "{contradiction:?}");
    assert!(
        contradiction.error.is_none(),
        "a contradiction survives other throws: {contradiction:?}"
    );
}

#[test]
fn identity_veto_does_not_unwrap_async_or_generator_results() {
    let signatures = [signature(&[value_fact(json!({"mayBeObject": true}))])];
    for implementation in [
        "export async function subject(value) { return value; }",
        "export function* subject(value) { return value; }",
    ] {
        let observed = execute(implementation, Observation::ParameterReturn(0), &signatures);
        assert!(observed.contradicted(), "{implementation}: {observed:?}");
    }
}

#[test]
fn identity_veto_samples_every_overload() {
    let signatures = [
        signature(&[value_fact(json!({"mayBeObject": true}))]),
        signature(&[value_fact(json!({"mayBeString": true}))]),
    ];
    let observed = execute(
        "export function subject(value) { return typeof value === 'string' ? {} : value; }",
        Observation::ParameterReturn(0),
        &signatures,
    );
    assert!(
        observed.contradicted(),
        "the second overload must be sampled: {observed:?}"
    );
}

#[test]
fn identity_samples_respect_complete_literals_and_finite_numbers() {
    let mut literal = value_fact(json!({"mayBeString": true}));
    literal["partitions"] = json!([{
        "axis": "literal",
        "complete": true,
        "cases": [{"kind": "literal", "literal": {"kind": "string", "string": "only"}}],
    }]);
    let observed = execute(
        "export function subject(value) { return value === 'only' ? value : {}; }",
        Observation::ParameterReturn(0),
        &[signature(&[literal])],
    );
    assert!(!observed.contradicted(), "{observed:?}");
    assert!(observed.error.is_none(), "{observed:?}");

    let observed = execute(
        "export function subject(value) { return Number.isFinite(value) ? value : {}; }",
        Observation::ParameterReturn(0),
        &[signature(&[value_fact(json!({
            "mayBeNumber": true,
            "numbersFinite": true,
        }))])],
    );
    assert!(!observed.contradicted(), "{observed:?}");
    assert!(observed.error.is_none(), "{observed:?}");
}

#[test]
fn an_unsampleable_overload_cannot_be_hidden_by_an_observable_one() {
    let mut empty_literal = value_fact(json!({"mayBeString": true}));
    empty_literal["partitions"] = json!([{
        "axis": "literal", "complete": true, "cases": [],
    }]);
    let empty_signature = signature(&[empty_literal]);
    let observed = execute(
        "export function subject(value) { return value; }",
        Observation::ParameterReturn(0),
        std::slice::from_ref(&empty_signature),
    );
    assert!(!observed.contradicted(), "{observed:?}");
    assert!(observed.error.is_some(), "{observed:?}");
    assert!(!identity_signatures_supported(
        &[
            signature(&[value_fact(json!({"mayBeObject": true}))]),
            empty_signature
        ],
        0,
    ));
}

#[test]
fn optional_identity_observes_undefined_and_detects_a_replacing_default() {
    let mut optional = signature(&[value_fact(json!({"mayBeObject": true}))]);
    optional.parameters[0].optional = true;
    for (implementation, contradiction) in [
        ("export function subject(value) { return value; }", false),
        (
            "export function subject(value = {}) { return value; }",
            true,
        ),
    ] {
        let observed = execute(
            implementation,
            Observation::ParameterReturn(0),
            std::slice::from_ref(&optional),
        );
        assert_eq!(observed.contradicted(), contradiction, "{observed:?}");
        assert!(observed.error.is_none(), "{observed:?}");
    }
}

#[test]
fn empty_return_observation_remains_distinct_from_parameter_identity() {
    let signatures = [signature(&[])];
    for (implementation, contradiction) in [
        ("export function subject() {}", false),
        ("export function subject() { return null; }", true),
        ("export function subject() { return 0; }", true),
    ] {
        let observed = execute(implementation, Observation::EmptyReturns, &signatures);
        assert_eq!(observed.contradicted(), contradiction, "{observed:?}");
        assert!(observed.error.is_none(), "{observed:?}");
    }
}

#[test]
fn parameter_sampling_needs_the_target_slot_in_every_overload() {
    let value = value_fact(json!({"mayBeObject": true}));
    let mut first = signature(&[value.clone(), value.clone()]);
    assert!(identity_signatures_supported(&[first.clone()], 1));
    assert!(!identity_signatures_supported(&[], 0));
    assert!(!identity_signatures_supported(&[first.clone()], 2));
    assert!(!identity_signatures_supported(
        &[first.clone(), signature(&[value])],
        1,
    ));
    first.parameters[1].rest = true;
    assert!(!identity_signatures_supported(&[first.clone()], 1));
    first.parameters[1].rest = false;
    first.parameters[0].rest = true;
    assert!(!identity_signatures_supported(&[first.clone()], 1));
    first.parameters[0].rest = false;
    first.parameters[1].optional = true;
    first.parameters[1].defaulted = true;
    assert!(
        identity_signatures_supported(&[first], 1),
        "synthesis observes defaults; the independent census decides their identity proof",
    );
}

fn return_operation() -> Operation {
    Operation {
        id: OperationId("returned".into()),
        kind: OperationKind::Return,
        guard: None,
        trigger: Some(Trigger::Event(Event::Call)),
        at: Some(Event::Call),
        schedule: Some(Schedule::SameStack),
        tracking: Tracking::Untracked,
        owner: OwnerRelation::default(),
        cardinality: Cardinality::default(),
        inputs: vec![],
        output: Some(ValueShape::Parameter {
            index: 1,
            path: vec![],
        }),
        resources: Default::default(),
        composed_from: None,
    }
}

fn export_with_returns(
    claim: KnowledgeSet<OperationId>,
    operations: Vec<Operation>,
) -> ExportSemantics {
    let target = ExportTargetIdentity {
        module: ArtifactIdentity {
            path: "index.js".into(),
            digest: Digest::parse(format!("sha256:{}", "0".repeat(64))).unwrap(),
        },
        export_name: "subject".into(),
    };
    ExportSemantics {
        identity: ExportIdentity {
            entrypoint: ".".into(),
            public_name: "subject".into(),
            runtime: target.clone(),
            declarations: target,
        },
        shape: ValueShape::Callable,
        stability: StabilityKnowledge::Unknown,
        call: CallSemantics::new(
            CallClaims {
                returns: claim,
                ..CallClaims::default()
            },
            operations,
            vec![],
            vec![],
            GuardPartition::default(),
        ),
    }
}

#[test]
fn claim_filter_selects_only_one_whole_parameter_return() {
    let operation = return_operation();
    let claim = KnowledgeSet::complete(vec![operation.id.clone()]);
    let export = export_with_returns(claim.clone(), vec![operation.clone()]);
    assert_eq!(
        candidate_observation("returns", &export),
        Some(Observation::ParameterReturn(1))
    );
    assert!(candidate_observation("reads", &export).is_none());
    assert_eq!(
        candidate_observation(
            "returns",
            &export_with_returns(KnowledgeSet::complete(vec![]), vec![])
        ),
        Some(Observation::EmptyReturns),
    );
    // Withholding the candidate opens its knowledge before synthesis; the
    // exact withheld-domain record supplies eligibility, not this helper.
    assert_eq!(
        candidate_observation(
            "returns",
            &export_with_returns(
                KnowledgeSet::Partial(vec![operation.id.clone()]),
                vec![operation.clone()]
            )
        ),
        Some(Observation::ParameterReturn(1)),
    );
    for unsupported in [
        export_with_returns(claim.clone(), vec![]),
        export_with_returns(
            KnowledgeSet::complete(vec![operation.id.clone(), operation.id.clone()]),
            vec![operation.clone()],
        ),
    ] {
        assert!(candidate_observation("returns", &unsupported).is_none());
    }
    let mut wrong_kind = operation.clone();
    wrong_kind.kind = OperationKind::Read;
    let mut member = operation.clone();
    member.output = Some(ValueShape::Parameter {
        index: 1,
        path: vec!["value".into()],
    });
    let mut plain = operation.clone();
    plain.output = Some(ValueShape::Plain);
    let mut missing_output = operation.clone();
    missing_output.output = None;
    let mut guarded = operation;
    guarded.guard = Some(Guard(vec![]));
    assert_eq!(
        candidate_observation(
            "returns",
            &export_with_returns(claim.clone(), vec![guarded]),
        ),
        Some(Observation::ParameterReturn(1)),
        "the independent census still requires this identity at every possible return",
    );
    for unsupported in [wrong_kind, member, plain, missing_output] {
        assert!(
            candidate_observation(
                "returns",
                &export_with_returns(claim.clone(), vec![unsupported])
            )
            .is_none()
        );
    }
}

/// ADR 0099: the not-callable module samples nothing and emits only when the
/// runtime value is a function after all.
#[test]
fn the_not_callable_module_emits_only_when_the_runtime_value_is_callable() {
    let quiet = execute(
        "export const subject = { equals: false };",
        Observation::NotCallable,
        &[],
    );
    assert_eq!(quiet.error, None);
    assert!(
        !quiet
            .markers
            .iter()
            .any(|marker| marker == "callable-value"),
        "{quiet:?}"
    );
    let loud = execute(
        "export const subject = () => 1;",
        Observation::NotCallable,
        &[],
    );
    assert_eq!(loud.error, None);
    assert!(
        loud.markers.iter().any(|marker| marker == "callable-value"),
        "{loud:?}"
    );
    let class = execute("export class subject {}", Observation::NotCallable, &[]);
    assert!(
        class
            .markers
            .iter()
            .any(|marker| marker == "callable-value"),
        "a class is typeof function and the veto sees it: {class:?}"
    );
}

fn callable_fact() -> Value {
    json!({
        "callability": "callable",
        "constructability": "nonConstructable",
        "primitive": {"mayBeObject": true},
    })
}

/// ADR 0100: the described-callbacks module is quiet for exactly the described
/// invocation — a described slot run inside its sample call — and loud for an
/// undescribed slot at any time or a described slot after the call returned.
#[test]
fn the_described_callbacks_module_emits_outside_the_description_only() {
    let signatures = [signature(&[callable_fact(), callable_fact()])];
    let invoked = |observed: &ObservationResult| {
        observed
            .markers
            .iter()
            .any(|marker| marker == "callback-invocation")
    };
    for implementation in [
        "export function subject(a, b) { a(); return 1; }",
        "export function subject(a, b) { a(); a(); }",
        // Nothing invoked: the veto is one-sided, the census proves the item.
        "export function subject(a, b) {}",
    ] {
        let quiet = execute(
            implementation,
            Observation::DescribedCallbacks(0b01),
            &signatures,
        );
        assert_eq!(quiet.error, None, "{implementation}");
        assert!(!invoked(&quiet), "{implementation}: {quiet:?}");
    }
    let both = execute(
        "export function subject(a, b) { a(); b(); }",
        Observation::DescribedCallbacks(0b11),
        &signatures,
    );
    assert!(!invoked(&both), "{both:?}");
    for implementation in [
        // An undescribed slot.
        "export function subject(a, b) { b(); }",
        // The described slot, after the sample call has returned.
        "export function subject(a, b) { queueMicrotask(a); }",
        "export function subject(a, b) { Promise.resolve().then(a); }",
    ] {
        let loud = execute(
            implementation,
            Observation::DescribedCallbacks(0b01),
            &signatures,
        );
        assert_eq!(loud.error, None, "{implementation}");
        assert!(invoked(&loud), "{implementation}: {loud:?}");
    }
    // Every callable slot carries its own recording callable, so the module
    // can tell the slots apart; the shared `callback` of ADR 0036 cannot.
    let source = module_source(
        "data:text/javascript,",
        "subject",
        Observation::DescribedCallbacks(0b01),
        &signatures,
    );
    assert!(
        source.contains("[callbackAt(0), callbackAt(1)]"),
        "{source}"
    );
    assert!(
        source.contains("const described = new Set([0]);"),
        "{source}"
    );
}

/// ADR 0100: only an enumeration of call-time bare-parameter invocations is
/// observed; anything else keeps the candidate recipe-less.
#[test]
fn the_described_callbacks_observation_is_selected_from_the_exact_enumeration() {
    use solid_reactive_ir::contract_semantics::{CallbackInvocation, ValueSource};
    let invoke = |id: &str| Operation {
        id: OperationId(id.into()),
        kind: OperationKind::Invoke,
        output: None,
        ..return_operation()
    };
    let export = |items: Vec<CallbackInvocation>, operations: Vec<Operation>| {
        let mut export = export_with_returns(KnowledgeSet::Unknown, operations);
        export.call = CallSemantics::new(
            CallClaims {
                callbacks: KnowledgeSet::complete(items),
                ..CallClaims::default()
            },
            export.call.operations.clone(),
            vec![],
            vec![],
            GuardPartition::default(),
        );
        export
    };
    let item = |index: u16, operation: &str| CallbackInvocation {
        from: ValueSource::Parameter {
            index,
            path: vec![],
        },
        operation: OperationId(operation.into()),
    };
    assert_eq!(
        candidate_observation("callbacks", &export(vec![], vec![])),
        Some(Observation::EmptyCallbacks)
    );
    assert_eq!(
        candidate_observation(
            "callbacks",
            &export(
                vec![item(0, "a"), item(3, "b")],
                vec![invoke("a"), invoke("b")]
            )
        ),
        Some(Observation::DescribedCallbacks(0b1001))
    );
    let mut queued = invoke("a");
    queued.schedule = Some(Schedule::Queued);
    assert_eq!(
        candidate_observation("callbacks", &export(vec![item(0, "a")], vec![queued])),
        None
    );
    assert_eq!(
        candidate_observation(
            "callbacks",
            &export(
                vec![CallbackInvocation {
                    from: ValueSource::Parameter {
                        index: 0,
                        path: vec!["onChange".into()],
                    },
                    operation: OperationId("a".into()),
                }],
                vec![invoke("a")]
            )
        ),
        None
    );
    assert_eq!(
        candidate_observation("callbacks", &export(vec![item(64, "a")], vec![invoke("a")])),
        None,
        "an index the mask cannot hold is not synthesized"
    );
}
