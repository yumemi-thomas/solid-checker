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
        self.markers.iter().any(|marker| {
            matches!(
                marker.as_str(),
                "return-value"
                    | "return-outside-identity"
                    | "return-not-primitive"
                    | "return-outside-containers"
            )
        })
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
        protocol: None,
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
    // Item B round 2 of ways-to-improve § 3.3: one literal member of the
    // argument is the container observation's, never the identity one; a
    // longer path is no observation's.
    let mut member = operation.clone();
    member.output = Some(ValueShape::Parameter {
        index: 1,
        path: vec!["value".into()],
    });
    assert!(matches!(
        candidate_observation("returns", &export_with_returns(claim.clone(), vec![member])),
        Some(Observation::ArgumentContainers(_))
    ));
    let mut deep_member = operation.clone();
    deep_member.output = Some(ValueShape::Parameter {
        index: 1,
        path: vec!["value".into(), "inner".into()],
    });
    // ADR 0113: a `plain` output is its own observation, never the identity
    // one -- `Object.is` against an argument would contradict every primitive
    // the claim permits.
    let mut plain = operation.clone();
    plain.output = Some(ValueShape::Plain);
    assert_eq!(
        candidate_observation(
            "returns",
            &export_with_returns(claim.clone(), vec![plain.clone()])
        ),
        Some(Observation::PrimitiveReturn),
    );
    let mut plain_read = plain;
    plain_read.kind = OperationKind::Read;
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
    for unsupported in [wrong_kind, deep_member, plain_read, missing_output] {
        assert!(
            candidate_observation(
                "returns",
                &export_with_returns(claim.clone(), vec![unsupported])
            )
            .is_none()
        );
    }
}

/// ADR 0113: the primitive-return module stays quiet on every primitive, `null`
/// included, and fires on anything a primitive completion rules out: an
/// object, an array, a function, a proxy, and the promise or iterator an
/// `async` function or a generator hands back whatever its body returns.
#[test]
fn the_primitive_return_module_emits_only_for_an_object_or_a_function() {
    let signatures = [signature(&[value_fact(json!({"mayBeNumber": true}))])];
    for implementation in [
        "export function subject(value) { return Math.min(Math.max(value, 0), 1); }",
        "export function subject() { return 'text'; }",
        "export function subject() { return true; }",
        "export function subject() { return null; }",
        "export function subject() { return void 0; }",
        "export function subject() {}",
        "export function subject() { return 1n; }",
        "export function subject() { return Symbol('s'); }",
        "export function subject(value) { return value !== null && typeof value === 'object'; }",
    ] {
        let observed = execute(implementation, Observation::PrimitiveReturn, &signatures);
        assert!(!observed.contradicted(), "{implementation}: {observed:?}");
        assert!(observed.error.is_none(), "{implementation}: {observed:?}");
    }
    for implementation in [
        "export function subject() { return {}; }",
        "export function subject() { return []; }",
        "export function subject() { return () => 1; }",
        "export function subject() { return new Proxy({}, {}); }",
        "export function subject() { return new Number(1); }",
        "export function subject(value) { return value === 1 ? {} : value; }",
        "export async function subject() { return 1; }",
        "export function* subject() { return 1; }",
    ] {
        let observed = execute(implementation, Observation::PrimitiveReturn, &signatures);
        assert!(observed.contradicted(), "{implementation}: {observed:?}");
        assert!(observed.error.is_none(), "{implementation}: {observed:?}");
    }
}

/// ADR 0115: returns of argument containers select their own observation --
/// one whole parameter stays ADR 0096's identity veto -- and the module stays
/// quiet on every claimed container and fires on anything else, a different
/// object in a claimed position included.
#[test]
fn the_argument_container_module_emits_only_outside_the_claimed_containers() {
    let returned = |id: &str, output: ValueShape| Operation {
        id: OperationId(id.into()),
        output: Some(output),
        ..return_operation()
    };
    let operations = vec![
        returned(
            "return-0",
            ValueShape::Parameter {
                index: 0,
                path: vec![],
            },
        ),
        returned("return-1", ValueShape::ArgumentArray { items: vec![] }),
        returned("return-2", ValueShape::ArgumentArray { items: vec![0] }),
    ];
    let claim = KnowledgeSet::complete(
        operations
            .iter()
            .map(|operation| operation.id.clone())
            .collect(),
    );
    let Some(observation @ Observation::ArgumentContainers(_)) =
        candidate_observation("returns", &export_with_returns(claim, operations.clone()))
    else {
        panic!("argument containers select their own observation");
    };
    assert_eq!(
        candidate_observation(
            "returns",
            &export_with_returns(
                KnowledgeSet::complete(vec![operations[0].id.clone()]),
                vec![operations[0].clone()]
            )
        ),
        Some(Observation::ParameterReturn(0)),
        "a lone whole parameter is the identity veto's"
    );
    let signatures = [signature(&[value_fact(
        json!({"mayBeObject": true, "mayBeUndefined": true}),
    )])];
    for implementation in [
        "export function subject(value) { return Array.isArray(value) ? value : value ? [value] : []; }",
        "export function subject(value) { return [value]; }",
        "export function subject(value) { return value; }",
        "export function subject() { return []; }",
    ] {
        let observed = execute(implementation, observation, &signatures);
        assert!(!observed.contradicted(), "{implementation}: {observed:?}");
        assert!(observed.error.is_none(), "{implementation}: {observed:?}");
    }
    for implementation in [
        "export function subject(value) { return { value }; }",
        "export function subject(value) { return [value, value]; }",
        "export function subject() { return [{}]; }",
        "export function subject() { return {}; }",
        "export function subject() { return 1; }",
    ] {
        let observed = execute(implementation, observation, &signatures);
        assert!(observed.contradicted(), "{implementation}: {observed:?}");
        assert!(observed.error.is_none(), "{implementation}: {observed:?}");
    }
}

/// ADR 0116 on the same module: what an invocation of the argument returned is
/// a token the slot's recording function handed back during that call, and
/// nothing else is -- not a value the caller's callable would have returned,
/// and not a token of another call.
#[test]
fn the_invocation_result_module_admits_only_what_the_invocation_returned() {
    let returned = |id: &str, output: ValueShape| Operation {
        id: OperationId(id.into()),
        output: Some(output),
        ..return_operation()
    };
    let operations = vec![
        returned("return-0", ValueShape::InvocationResult { parameter: 0 }),
        returned(
            "return-1",
            ValueShape::Parameter {
                index: 0,
                path: vec![],
            },
        ),
    ];
    let claim = KnowledgeSet::complete(
        operations
            .iter()
            .map(|operation| operation.id.clone())
            .collect(),
    );
    let Some(observation @ Observation::ArgumentContainers(_)) =
        candidate_observation("returns", &export_with_returns(claim, operations.clone()))
    else {
        panic!("an invocation result selects the container observation");
    };
    assert!(
        matches!(
            candidate_observation(
                "returns",
                &export_with_returns(
                    KnowledgeSet::complete(vec![operations[0].id.clone()]),
                    vec![operations[0].clone()]
                )
            ),
            Some(Observation::ArgumentContainers(_))
        ),
        "a lone invocation result is this observation's, not the identity veto's"
    );
    let mut maybe_callable = value_fact(json!({"mayBeObject": true, "mayBeUndefined": true}));
    maybe_callable["callability"] = json!("callable");
    let signatures = [signature(&[maybe_callable])];
    for implementation in [
        // `accessWith` and `access`.
        "export function subject(value) { return typeof value === 'function' ? value() : value; }",
        "export function subject(value) { return typeof value === 'function' && !value.length ? value() : value; }",
        "export function subject(value) { return value; }",
    ] {
        let observed = execute(implementation, observation, &signatures);
        assert!(!observed.contradicted(), "{implementation}: {observed:?}");
        assert!(observed.error.is_none(), "{implementation}: {observed:?}");
    }
    for implementation in [
        // Calling the argument and handing back something else.
        "export function subject(value) { if (typeof value === 'function') { value(); return {}; } return value; }",
        // A token of an earlier call is not this call's invocation result.
        "let last; export function subject(value) { if (typeof value === 'function') { const previous = last; last = value(); return previous ?? last; } return value; }",
        "export function subject(value) { return typeof value === 'function' ? [value()] : value; }",
    ] {
        let observed = execute(implementation, observation, &signatures);
        assert!(observed.contradicted(), "{implementation}: {observed:?}");
        assert!(observed.error.is_none(), "{implementation}: {observed:?}");
    }
}

/// Item B round 2 of ways-to-improve § 3.3 on the same module: a member of the
/// argument is what the argument holds at the claimed key when the call has
/// returned, and `undefined` is exactly `undefined`. `callHandler`'s
/// `event?.defaultPrevented` holds on every sample -- including a handler that
/// writes the member before the return reads it -- and a value read before a
/// write, another member, and an `undefined` the claim does not enumerate
/// fire. A set that names neither shape keeps its module byte for byte.
#[test]
fn the_member_module_admits_what_the_argument_holds_at_return_and_undefined() {
    let returned = |id: &str, output: ValueShape| Operation {
        id: OperationId(id.into()),
        output: Some(output),
        ..return_operation()
    };
    let member = |key: &str| ValueShape::Parameter {
        index: 0,
        path: vec![key.into()],
    };
    let claim_of = |operations: &[Operation]| {
        KnowledgeSet::complete(
            operations
                .iter()
                .map(|operation| operation.id.clone())
                .collect(),
        )
    };
    let optional = vec![
        returned("return-0", member("defaultPrevented")),
        returned("return-1", ValueShape::Undefined),
    ];
    let Some(chain @ Observation::ArgumentContainers(_)) = candidate_observation(
        "returns",
        &export_with_returns(claim_of(&optional), optional.clone()),
    ) else {
        panic!("a member and undefined select the container observation");
    };
    let lone = vec![returned("return-0", member("key"))];
    let Some(read @ Observation::ArgumentContainers(_)) =
        candidate_observation("returns", &export_with_returns(claim_of(&lone), lone))
    else {
        panic!("a lone member is this observation's, not the identity veto's");
    };
    // A key the inline table cannot hold is not synthesized.
    let long = vec![returned("return-0", member(&"k".repeat(33)))];
    assert_eq!(
        candidate_observation("returns", &export_with_returns(claim_of(&long), long)),
        None
    );
    let object = || value_fact(json!({"mayBeObject": true}));
    let mut handler = value_fact(json!({"mayBeObject": true, "mayBeUndefined": true}));
    handler["callability"] = json!("callable");
    let two = [signature(&[object(), handler])];
    for implementation in [
        // `@kobalte/utils@2.0.0-alpha.0`'s `callHandler`, byte for byte.
        "export function subject(event, handler) {\n\tif (handler) if (typeof handler === \"function\") handler(event);\n\telse handler[0](handler[1], event);\n\treturn event?.defaultPrevented;\n}",
        // The member written before the return reads it is still the member.
        "export function subject(event) { if (event) event.defaultPrevented = 1; return event?.defaultPrevented; }",
        "export function subject(event) { return event == null ? undefined : event.defaultPrevented; }",
    ] {
        let observed = execute(implementation, chain, &two);
        assert!(!observed.contradicted(), "{implementation}: {observed:?}");
        assert!(observed.error.is_none(), "{implementation}: {observed:?}");
    }
    for implementation in [
        // The value read before a write is not what the argument holds when
        // the call returns.
        "export function subject(event) { const before = event?.defaultPrevented; if (event) event.defaultPrevented = {}; return before; }",
        "export function subject(event) { return { value: event?.defaultPrevented }; }",
        "export function subject(event) { return event; }",
        "export function subject() { return null; }",
    ] {
        let observed = execute(implementation, chain, &two);
        assert!(observed.contradicted(), "{implementation}: {observed:?}");
        assert!(observed.error.is_none(), "{implementation}: {observed:?}");
    }
    let one = [signature(&[object()])];
    let quiet = execute("export function subject(p) { return p.key; }", read, &one);
    assert!(!quiet.contradicted(), "{quiet:?}");
    assert!(quiet.error.is_none(), "{quiet:?}");
    // The object sample whose member holds a fresh token is what tells the
    // member from anything else. (A different member the samples leave
    // unset reads `undefined` on each of them, which the lone member claim
    // admits whenever the claimed member is unset too: that is the census's
    // to refuse, not this finite sample's.)
    for implementation in [
        "export function subject(p) { return {}; }",
        "export function subject(p) { return p; }",
    ] {
        let observed = execute(implementation, read, &one);
        assert!(observed.contradicted(), "{implementation}: {observed:?}");
    }
    // A set naming no member and no undefined is synthesized as it was.
    let unchanged = vec![
        returned(
            "return-0",
            ValueShape::Parameter {
                index: 0,
                path: vec![],
            },
        ),
        returned("return-1", ValueShape::ArgumentArray { items: vec![0] }),
    ];
    let Some(unchanged @ Observation::ArgumentContainers(_)) = candidate_observation(
        "returns",
        &export_with_returns(claim_of(&unchanged), unchanged.clone()),
    ) else {
        panic!("argument containers select their own observation");
    };
    let source = module_source("subject-module", "subject", unchanged, &one);
    assert!(
        source.contains(
            "const containers = [{ array: false, invocation: false, items: [0] }, { array: true, invocation: false, items: [0] }];"
        ),
        "{source}"
    );
    assert!(!source.contains("container.member"), "{source}");
}

/// A throwing sample observes nothing, and a run in which no sample completes
/// normally is incomplete rather than satisfied by silence.
#[test]
fn the_primitive_return_module_requires_a_normal_completion() {
    let signatures = [signature(&[value_fact(json!({"mayBeNumber": true}))])];
    let nothing = execute(
        "export function subject() { throw new Error('sample rejected'); }",
        Observation::PrimitiveReturn,
        &signatures,
    );
    assert!(!nothing.contradicted(), "{nothing:?}");
    assert!(nothing.error.is_some(), "{nothing:?}");
    assert!(
        nothing
            .markers
            .iter()
            .any(|marker| marker == "sample-threw")
    );

    // Two samples, `1` and `"x"`: the first throws, the second completes.
    let partial = execute(
        "export function subject(value) { if (typeof value === 'number') throw new Error('sample rejected'); return {}; }",
        Observation::PrimitiveReturn,
        &[signature(&[value_fact(
            json!({"mayBeNumber": true, "mayBeString": true}),
        )])],
    );
    assert!(
        partial.contradicted(),
        "a contradiction survives other throws: {partial:?}"
    );
    assert!(
        partial
            .markers
            .iter()
            .any(|marker| marker == "sample-threw")
    );
    assert!(partial.error.is_none(), "{partial:?}");
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

/// ADR 0101: the described-reads module is quiet for exactly the described
/// read -- a member of a described slot's argument invoked inside its sample
/// call -- and loud for a member of an undescribed slot at any time, or a
/// member of a described slot after the call returned. A plain property read,
/// a bare call of the argument, and the engine's own protocol members record
/// nothing.
#[test]
fn the_described_reads_module_emits_outside_the_description_only() {
    let object = value_fact(json!({"mayBeObject": true}));
    let signatures = [signature(&[object.clone(), object])];
    let read = |observed: &ObservationResult| {
        observed
            .markers
            .iter()
            .any(|marker| marker == "read-operation")
    };
    for implementation in [
        "export function subject(a, b) { return a.of.values(); }",
        "export function subject(a, b) { a.of.values(); a.other(); }",
        // Reads that are not member invocations.
        "export function subject(a, b) { return a.of.values; }",
        "export function subject(a, b) { return b.value; }",
        // A bare call of the argument is the callbacks domain's.
        "export function subject(a, b) { return b(); }",
        // The engine's own protocol members.
        "export function subject(a, b) { return `${b}` + String(b) + JSON.stringify({ b }); }",
        // A walk along a member chain ends: the tripwire is bounded in depth.
        "export function subject(a, b) { let node = b; while (node) node = node.parentNode; }",
        // Nothing invoked: the veto is one-sided, the census proves the item.
        "export function subject(a, b) {}",
    ] {
        let quiet = execute(
            implementation,
            Observation::DescribedReads(0b01),
            &signatures,
        );
        assert_eq!(quiet.error, None, "{implementation}");
        assert!(!read(&quiet), "{implementation}: {quiet:?}");
    }
    let both = execute(
        "export function subject(a, b) { a.x(); b.y.z(); }",
        Observation::DescribedReads(0b11),
        &signatures,
    );
    assert!(!read(&both), "{both:?}");
    for implementation in [
        // A member of an undescribed slot.
        "export function subject(a, b) { b.of.values(); }",
        "export function subject(a, b) { const values = b.of.values; values(); }",
        // The described slot's member, after the sample call has returned.
        "export function subject(a, b) { queueMicrotask(() => a.of.values()); }",
        "export function subject(a, b) { Promise.resolve().then(() => a.of.values()); }",
    ] {
        let loud = execute(
            implementation,
            Observation::DescribedReads(0b01),
            &signatures,
        );
        assert_eq!(loud.error, None, "{implementation}");
        assert!(read(&loud), "{implementation}: {loud:?}");
    }
    // Every object slot carries its own tripwire, described or not, so the
    // module can tell the slots apart; a primitive slot stays a primitive.
    let source = module_source(
        "data:text/javascript,",
        "subject",
        Observation::DescribedReads(0b01),
        &signatures,
    );
    assert!(
        source.contains("[tripwireAt(0), tripwireAt(1)]"),
        "{source}"
    );
    assert!(
        source.contains("const described = new Set([0]);"),
        "{source}"
    );
    let mixed = [signature(&[
        value_fact(json!({"mayBeObject": true})),
        value_fact(json!({"mayBeNumber": true})),
    ])];
    let source = module_source(
        "data:text/javascript,",
        "subject",
        Observation::DescribedReads(0b01),
        &mixed,
    );
    assert!(source.contains("[tripwireAt(0), 1]"), "{source}");
    // A described slot is a tripwire whatever the signature says about it.
    let source = module_source(
        "data:text/javascript,",
        "subject",
        Observation::DescribedReads(0b10),
        &mixed,
    );
    assert!(
        source.contains("[tripwireAt(0), tripwireAt(1)]"),
        "{source}"
    );
}

/// ADR 0101: only an enumeration of call-time member reads of caller
/// parameters is observed; the empty enumeration keeps its hand recipes, and
/// an owned, composed, deferred or guarded item keeps the candidate
/// recipe-less.
#[test]
fn the_described_reads_observation_is_selected_from_the_exact_enumeration() {
    use solid_reactive_ir::contract_semantics::{ComposedFrom, ReactiveRole};
    let read = |id: &str, input: ValueShape| Operation {
        id: OperationId(id.into()),
        kind: OperationKind::Read,
        inputs: vec![input],
        output: None,
        ..return_operation()
    };
    let parameter = |index: u16, path: &[&str]| ValueShape::Parameter {
        index,
        path: path.iter().map(|segment| (*segment).to_owned()).collect(),
    };
    let export = |items: &[&str], operations: Vec<Operation>| {
        let mut export = export_with_returns(KnowledgeSet::Unknown, operations);
        export.call = CallSemantics::new(
            CallClaims {
                reads: KnowledgeSet::complete(
                    items.iter().map(|id| OperationId((*id).into())).collect(),
                ),
                ..CallClaims::default()
            },
            export.call.operations.clone(),
            vec![],
            vec![],
            GuardPartition::default(),
        );
        export
    };
    assert_eq!(
        candidate_observation("reads", &export(&[], vec![])),
        None,
        "the empty enumeration is served by hand recipes, deliberately"
    );
    assert_eq!(
        candidate_observation(
            "reads",
            &export(
                &["a", "b"],
                vec![
                    read("a", parameter(0, &["of", "values"])),
                    read("b", parameter(2, &[]))
                ]
            )
        ),
        Some(Observation::DescribedReads(0b101))
    );
    let owned = read(
        "a",
        ValueShape::Reactive {
            role: ReactiveRole::Accessor,
            resource: None,
            capabilities: KnowledgeSet::Unknown,
        },
    );
    assert_eq!(
        candidate_observation("reads", &export(&["a"], vec![owned])),
        None
    );
    let mut queued = read("a", parameter(0, &["of"]));
    queued.schedule = Some(Schedule::Queued);
    assert_eq!(
        candidate_observation("reads", &export(&["a"], vec![queued])),
        None
    );
    let mut composed = read("a", parameter(0, &["of"]));
    composed.composed_from = Some(ComposedFrom {
        export: "helper".into(),
        operation: OperationId("helper-read".into()),
    });
    assert_eq!(
        candidate_observation("reads", &export(&["a"], vec![composed])),
        None
    );
    assert_eq!(
        candidate_observation(
            "reads",
            &export(&["a"], vec![read("a", parameter(64, &[]))])
        ),
        None,
        "an index the mask cannot hold is not synthesized"
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

fn masks(call: u64, get: u64, coerce: u64) -> ProtocolMasks {
    ProtocolMasks {
        call,
        get,
        coerce,
        ..ProtocolMasks::default()
    }
}

/// Item A of ways-to-improve § 3.3: the per-protocol module is quiet for
/// exactly the described uses -- `access`'s `.length` read and call of slot 0,
/// `compare`'s coercions of slots 0 and 1, inside the sample call -- and loud
/// for a use by an undescribed protocol, of an undescribed slot, or after the
/// call returned. Its Proxy answers as the target does, so `.length` reads 0
/// and `access` reaches its call branch.
#[test]
fn the_described_protocols_module_emits_outside_the_description_only() {
    let invoked = |observed: &ObservationResult| {
        observed
            .markers
            .iter()
            .any(|marker| marker == "callback-invocation")
    };
    let any = value_fact(json!({"unknown": true}));
    let callable = [signature(&[callable_fact()])];
    let pair = [signature(&[any.clone(), any])];
    let object = value_fact(json!({"mayBeObject": true}));
    let objects = [signature(&[object.clone(), object])];

    let access = "export const subject = (v) => typeof v === \"function\" && !v.length ? v() : v;";
    let quiet = execute(
        access,
        Observation::DescribedProtocols(masks(1, 1, 0)),
        &callable,
    );
    assert_eq!(quiet.error, None);
    assert!(!invoked(&quiet), "{quiet:?}");
    // The Proxy's `.length` is the zero-arity target's own, so the call branch
    // runs: describing the read alone is contradicted by that call.
    let call_undescribed = execute(
        access,
        Observation::DescribedProtocols(masks(0, 1, 0)),
        &callable,
    );
    assert!(invoked(&call_undescribed), "{call_undescribed:?}");

    let compare = "export const subject = (a, b) => a < b ? -1 : a > b ? 1 : 0;";
    let quiet = execute(
        compare,
        Observation::DescribedProtocols(masks(0, 0, 0b11)),
        &pair,
    );
    assert_eq!(quiet.error, None);
    assert!(!invoked(&quiet), "{quiet:?}");
    let one_slot = execute(
        compare,
        Observation::DescribedProtocols(masks(0, 0, 0b01)),
        &pair,
    );
    assert!(invoked(&one_slot), "slot 1 is coerced too: {one_slot:?}");

    for (implementation, described) in [
        // The falsifying variant: `get 0` described, and the export also
        // coerces parameter 0.
        (
            "export const subject = (v) => v.length + v;",
            masks(0, 1, 0),
        ),
        // A read deferred past the sample call.
        (
            "export const subject = (v) => { queueMicrotask(() => v.length); };",
            masks(0, 1, 0),
        ),
        // A read of an undescribed slot.
        (
            "export const subject = (a, b) => a.x + b.y;",
            masks(0, 1, 0),
        ),
        // An iteration no item describes.
        (
            "export const subject = (v) => { for (const x of v) {} };",
            masks(0, 1, 0),
        ),
    ] {
        let observed = execute(
            implementation,
            Observation::DescribedProtocols(described),
            &objects,
        );
        assert!(invoked(&observed), "{implementation}: {observed:?}");
    }

    let source = module_source(
        "data:text/javascript,",
        "subject",
        Observation::DescribedProtocols(masks(1, 1, 0)),
        &callable,
    );
    assert!(source.contains("slotAt(0, \"function\")"), "{source}");
    assert!(
        source.contains(
            "const described = { call: new Set([0]), get: new Set([0]), iterate: new Set([]), coerce: new Set([]), \"has-instance\": new Set([]) };"
        ),
        "{source}"
    );
    assert!(source.contains("Reflect.get(t, key, receiver)"), "{source}");
}

/// A described enumeration selects the per-protocol observation exactly when
/// one of its items is a non-call use, and never counts that item as a call
/// slot; a call-only enumeration keeps ADR 0100's module.
#[test]
fn a_non_call_item_selects_the_protocol_observation_and_is_no_call_slot() {
    use solid_reactive_ir::contract_semantics::{CallbackInvocation, InvokeProtocol, ValueSource};
    let invoke = |id: &str, protocol: Option<InvokeProtocol>| Operation {
        id: OperationId(id.into()),
        kind: OperationKind::Invoke,
        output: None,
        tracking: Tracking::AmbientAtExecution,
        protocol,
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
        candidate_observation(
            "callbacks",
            &export(
                vec![item(0, "a"), item(0, "b"), item(2, "c")],
                vec![
                    invoke("a", None),
                    invoke("b", Some(InvokeProtocol::Get)),
                    invoke("c", Some(InvokeProtocol::Coerce)),
                ]
            )
        ),
        Some(Observation::DescribedProtocols(ProtocolMasks {
            call: 0b001,
            get: 0b001,
            coerce: 0b100,
            ..ProtocolMasks::default()
        }))
    );
    assert_eq!(
        candidate_observation(
            "callbacks",
            &export(vec![item(1, "a")], vec![invoke("a", None)])
        ),
        Some(Observation::DescribedCallbacks(0b10)),
        "a call-only enumeration is ADR 0100's observation, unchanged"
    );
}

fn members(items: &[(u16, u8)]) -> MemberCalls {
    let mut members = MemberCalls::default();
    for (slot, index) in items {
        members.push(*slot, *index).expect("within capacity");
    }
    members
}

/// Item B of ways-to-improve § 3.3: the member module is quiet for exactly
/// the described uses -- `callHandler`'s call of slot 1, call of slot 1's
/// member 0, reads of both slots, inside the sample call -- and loud for a
/// call of an undescribed member, a described member called after the call
/// returned, or a member call no item describes at all. A slot a member item
/// names is sampled with the array Proxy even where its signature offers only
/// a callable.
#[test]
fn the_described_members_module_emits_outside_the_description_only() {
    let invoked = |observed: &ObservationResult| {
        observed
            .markers
            .iter()
            .any(|marker| marker == "callback-invocation")
    };
    let object = value_fact(json!({"mayBeObject": true, "mayBeUndefined": true}));
    let handler = [signature(&[object.clone(), callable_fact()])];
    let call_handler = "export function subject(event, handler) {\n\
         \tif (handler) if (typeof handler === \"function\") handler(event);\n\
         \telse handler[0](handler[1], event);\n\
         \treturn event?.defaultPrevented;\n\
         }";
    let described = members(&[(1, 0)]);
    let quiet = execute(
        call_handler,
        Observation::DescribedMembers(masks(0b10, 0b11, 0), described),
        &handler,
    );
    assert_eq!(quiet.error, None);
    assert!(!invoked(&quiet), "{quiet:?}");
    let source = module_source(
        "data:text/javascript,",
        "subject",
        Observation::DescribedMembers(masks(0b10, 0b11, 0), described),
        &handler,
    );
    assert!(source.contains("slotAt(1, \"array\")"), "{source}");
    assert!(source.contains("slotAt(1, \"function\")"), "{source}");
    assert!(
        source.contains("const members = new Set([\"1:0\"]);"),
        "{source}"
    );
    assert!(source.contains("const memberTops = { 1: 0 };"), "{source}");

    for (implementation, described) in [
        // The member call is the item; describing the rest without it is
        // contradicted by the call of member 0.
        (call_handler, members(&[(1, 1)])),
        // A call of member 1 beside the described member 0.
        (
            "export function subject(event, handler) { if (typeof handler !== \"function\") { handler[0](); handler[1](); } }",
            members(&[(1, 0)]),
        ),
        // A described member called after the sample call returned.
        (
            "export function subject(event, handler) { if (typeof handler !== \"function\") queueMicrotask(() => handler[0]()); }",
            members(&[(1, 0)]),
        ),
    ] {
        let observed = execute(
            implementation,
            Observation::DescribedMembers(masks(0b10, 0b11, 0), described),
            &handler,
        );
        assert!(invoked(&observed), "{implementation}: {observed:?}");
    }
}

/// A member-path call item selects the member observation, at an index only;
/// an enumeration without one keeps the per-protocol or ADR 0100 observation,
/// and a member at a property key, a member of a member, or a non-call use of
/// a member is not synthesized.
#[test]
fn a_member_call_item_selects_the_member_observation_at_an_index_only() {
    use solid_reactive_ir::contract_semantics::{CallbackInvocation, InvokeProtocol, ValueSource};
    let invoke = |id: &str, protocol: Option<InvokeProtocol>| Operation {
        id: OperationId(id.into()),
        kind: OperationKind::Invoke,
        output: None,
        tracking: Tracking::AmbientAtExecution,
        protocol,
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
    let item = |index: u16, path: &[&str], operation: &str| CallbackInvocation {
        from: ValueSource::Parameter {
            index,
            path: path.iter().map(|key| (*key).to_owned()).collect(),
        },
        operation: OperationId(operation.into()),
    };
    assert_eq!(
        candidate_observation(
            "callbacks",
            &export(
                vec![item(1, &[], "a"), item(1, &["0"], "b"), item(1, &[], "c")],
                vec![
                    invoke("a", None),
                    invoke("b", None),
                    invoke("c", Some(InvokeProtocol::Get)),
                ]
            )
        ),
        Some(Observation::DescribedMembers(
            ProtocolMasks {
                call: 0b10,
                get: 0b10,
                ..ProtocolMasks::default()
            },
            members(&[(1, 0)])
        ))
    );
    for (path, protocol) in [
        (&["run"][..], None),
        (&["0", "1"][..], None),
        (&["01"][..], None),
        (&["16"][..], None),
        (&["0"][..], Some(InvokeProtocol::Get)),
    ] {
        assert_eq!(
            candidate_observation(
                "callbacks",
                &export(vec![item(0, path, "a")], vec![invoke("a", protocol)])
            ),
            None,
            "{path:?} {protocol:?}"
        );
    }
}

/// ADR 0139: a `result-access` item is never a call slot -- it leaves the mask,
/// so an enumeration of kept items alone observes every slot -- and a class
/// export is sampled with `new`. The module hands the constructed value to
/// nothing, so a kept callable that runs at any time is the contradiction, and
/// a described call slot that runs inside the construction is the item.
#[test]
fn the_constructed_callbacks_module_samples_new_and_never_runs_a_kept_slot() {
    use solid_reactive_ir::contract_semantics::{
        CallbackInvocation, CardinalityScope, OwnerSource, UpperBound, ValueSource,
    };
    let kept = |id: &str| Operation {
        id: OperationId(id.into()),
        kind: OperationKind::Invoke,
        output: None,
        trigger: Some(Trigger::Event(Event::ResultAccess)),
        at: Some(Event::ResultAccess),
        schedule: Some(Schedule::External),
        tracking: Tracking::AmbientAtExecution,
        owner: OwnerRelation {
            source: OwnerSource::AmbientAtExecution,
            ..OwnerRelation::default()
        },
        cardinality: Cardinality {
            scope: Some(CardinalityScope::Trigger),
            min: Some(0),
            max: Some(UpperBound::Many),
        },
        ..return_operation()
    };
    let call = |id: &str| Operation {
        id: OperationId(id.into()),
        kind: OperationKind::Invoke,
        output: None,
        ..return_operation()
    };
    let item = |index: u16, operation: &str| CallbackInvocation {
        from: ValueSource::Parameter {
            index,
            path: vec![],
        },
        operation: OperationId(operation.into()),
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
    assert_eq!(
        candidate_observation("callbacks", &export(vec![item(0, "k")], vec![kept("k")])),
        Some(Observation::DescribedCallbacks(0)),
        "a kept slot is outside every mask"
    );
    assert_eq!(
        candidate_observation(
            "callbacks",
            &export(vec![item(0, "c"), item(0, "k")], vec![call("c"), kept("k")])
        ),
        Some(Observation::DescribedCallbacks(0b1)),
        "the call item alone is the described slot"
    );

    let signatures = [signature(&[callable_fact()])];
    let invoked = |observed: &ObservationResult| {
        observed
            .markers
            .iter()
            .any(|marker| marker == "callback-invocation")
    };
    for (implementation, mask) in [
        (
            "export class subject { constructor(a) { this.a = a; } run() { this.a(); } }",
            0,
        ),
        (
            "export class subject { constructor(a) { a(); this.a = a; } run() { this.a(); } }",
            0b1,
        ),
    ] {
        let quiet = execute(
            implementation,
            Observation::ConstructedCallbacks(mask),
            &signatures,
        );
        assert_eq!(quiet.error, None, "{implementation}");
        assert!(!invoked(&quiet), "{implementation}: {quiet:?}");
    }
    for implementation in [
        // Invoked at construction, which no call item describes.
        "export class subject { constructor(a) { a(); } }",
        // Kept somewhere other than the instance and run later.
        "export class subject { constructor(a) { queueMicrotask(a); } }",
    ] {
        let loud = execute(
            implementation,
            Observation::ConstructedCallbacks(0),
            &signatures,
        );
        assert_eq!(loud.error, None, "{implementation}");
        assert!(invoked(&loud), "{implementation}: {loud:?}");
    }
    let source = module_source(
        "data:text/javascript,",
        "subject",
        Observation::ConstructedCallbacks(0),
        &signatures,
    );
    assert!(source.contains("new subject(...args);"), "{source}");
    let called = module_source(
        "data:text/javascript,",
        "subject",
        Observation::DescribedCallbacks(0),
        &signatures,
    );
    assert!(!called.contains("new subject"), "{called}");
}
