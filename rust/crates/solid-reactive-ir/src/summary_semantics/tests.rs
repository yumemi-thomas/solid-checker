//! Characterization pins for the shared summary vocabulary. Every expected
//! value is a literal fixed at the baseline commit, before the vocabulary
//! moved into this module; none is computed by the code under test.

use std::collections::BTreeMap;

use super::{CallbackSchedule, CallbackSummary, InvokeProtocol, ReturnSummary};
use crate::contract_semantics::{
    PrototypeInstanceRecipe, PrototypeMember, PrototypeMemberKind, PrototypePopulation,
    PrototypeTrack,
};

fn leaf(kind: &str, label: &str) -> ReturnSummary {
    ReturnSummary {
        kind: kind.into(),
        label: label.into(),
        ..ReturnSummary::default()
    }
}

fn recipe(member: &str) -> PrototypeInstanceRecipe {
    PrototypeInstanceRecipe {
        members: vec![PrototypeMember {
            name: member.into(),
            kind: PrototypeMemberKind::Getter,
            tracks: vec![PrototypeTrack {
                cache: "signals".into(),
                argument: Some(0),
                shared: None,
            }],
        }],
        population: PrototypePopulation::Values,
    }
}

// T1: `interproc::equivalent_callbacks` uses this JSON as an equality key, so
// the key names, their order, `null` for `None`, the ordered map and the
// skipped prototype recipe (at every depth) are all part of the contract.
#[test]
fn return_summary_serializes_with_a_fixed_key_order_and_no_prototype() {
    let inner = ReturnSummary {
        kind: "plain".into(),
        label: String::new(),
        parameter: None,
        elements: Vec::new(),
        properties: BTreeMap::new(),
        prototype: Some(recipe("deep")),
    };
    let value = ReturnSummary {
        kind: "accessor".into(),
        label: "a\"b".into(),
        parameter: Some(2),
        elements: vec![None, Some(inner.clone())],
        properties: BTreeMap::from([
            ("zeta".to_owned(), leaf("k", "z")),
            ("alpha".to_owned(), inner),
        ]),
        prototype: Some(recipe("shallow")),
    };
    let expected = concat!(
        r#"{"kind":"accessor","label":"a\"b","parameter":2,"#,
        r#""elements":[null,{"kind":"plain","label":"","parameter":null,"elements":[],"properties":{}}],"#,
        r#""properties":{"alpha":{"kind":"plain","label":"","parameter":null,"elements":[],"properties":{}},"#,
        r#""zeta":{"kind":"k","label":"z","parameter":null,"elements":[],"properties":{}}}}"#,
    );
    assert_eq!(serde_json::to_string(&value).unwrap(), expected);
    assert_eq!(
        serde_json::to_string(&ReturnSummary::default()).unwrap(),
        r#"{"kind":"","label":"","parameter":null,"elements":[],"properties":{}}"#
    );
}

// T2: declaration order is derived `Ord`; the wire and canonical encodings
// do not depend on it, but `contracts.rs` and the `BTreeSet`s of protocols do.
#[test]
fn invoke_protocol_orders_by_declaration_and_is_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<InvokeProtocol>();
    let declared = [
        InvokeProtocol::Call,
        InvokeProtocol::Get,
        InvokeProtocol::Iterate,
        InvokeProtocol::Coerce,
        InvokeProtocol::HasInstance,
        InvokeProtocol::GetEnumerableStringValues,
        InvokeProtocol::GetOwnEnumerableValues,
    ];
    let mut reversed = declared;
    reversed.reverse();
    reversed.sort();
    assert_eq!(reversed, declared);
    assert!(InvokeProtocol::Call < InvokeProtocol::Get);
    assert!(InvokeProtocol::HasInstance < InvokeProtocol::GetEnumerableStringValues);
    assert!(InvokeProtocol::GetEnumerableStringValues < InvokeProtocol::GetOwnEnumerableValues);
    assert_eq!(
        declared.map(InvokeProtocol::is_value_enumeration),
        [false, false, false, false, false, true, true]
    );
}

fn row(
    protocol: InvokeProtocol,
    path: &[&str],
    execution: &str,
    schedule: Option<CallbackSchedule>,
) -> CallbackSummary {
    CallbackSummary {
        parameter: 0,
        execution: execution.into(),
        schedule,
        arguments: Vec::new(),
        owner: None,
        clears_tracking: false,
        protocol,
        path: path.iter().map(|segment| (*segment).to_owned()).collect(),
    }
}

// T3: a few discriminating rows with literal outcomes, in the order
// (is_invocation, is_result_access, invokes_argument, invokes_member).
#[test]
fn callback_predicates_separate_call_from_non_call_and_member_from_argument() {
    let outcome = |callback: &CallbackSummary| {
        (
            callback.is_invocation(),
            callback.is_result_access(),
            callback.invokes_argument(),
            callback.invokes_member(),
        )
    };
    let call = InvokeProtocol::Call;
    let get = InvokeProtocol::Get;
    assert_eq!(
        outcome(&row(call, &[], "inline", None)),
        (true, false, true, false)
    );
    assert_eq!(
        outcome(&row(call, &["0"], "inline", None)),
        (true, false, false, true)
    );
    assert_eq!(
        outcome(&row(get, &[], "inline", None)),
        (false, false, false, false)
    );
    assert_eq!(
        outcome(&row(get, &["a"], "inline", None)),
        (false, false, false, false)
    );
    assert_eq!(
        outcome(&row(
            InvokeProtocol::GetOwnEnumerableValues,
            &[],
            "inline",
            None
        )),
        (false, false, false, false)
    );
    // `ResultAccess` is a `deferred` word's retention event, read independent
    // of the protocol; no other execution or schedule is one.
    let deferred_result = Some(CallbackSchedule::ResultAccess);
    assert_eq!(
        outcome(&row(call, &[], "deferred", deferred_result)),
        (true, true, true, false)
    );
    assert_eq!(
        outcome(&row(get, &[], "deferred", deferred_result)),
        (false, true, false, false)
    );
    assert!(!row(call, &[], "tracked", deferred_result).is_result_access());
    assert!(!row(call, &[], "inline", deferred_result).is_result_access());
    assert!(!row(call, &[], "deferred", Some(CallbackSchedule::Queued)).is_result_access());
    assert!(!row(call, &[], "deferred", None).is_result_access());
}

#[test]
fn clearing_follows_only_inline_and_deferred_untracked() {
    use crate::contract_semantics::Tracking;
    assert!(CallbackSummary::clears_tracking_from(
        "inline",
        Tracking::Untracked
    ));
    assert!(CallbackSummary::clears_tracking_from(
        "deferred",
        Tracking::Untracked
    ));
    assert!(!CallbackSummary::clears_tracking_from(
        "tracked",
        Tracking::Untracked
    ));
    assert!(!CallbackSummary::clears_tracking_from(
        "inline",
        Tracking::Tracked
    ));
}

// T4: `contracts::project_return` orders returns by their `Debug` text and
// then dedups. Corresponding rows share type names, so the first differing
// byte is value text; this pins the order for pairs that agree on kind, label
// and parameter and differ only in children, vector or map termination,
// optional-child presence, prototype presence, and escaped strings that
// quote either type name. Rows are named by test metadata, outside the shape.
fn shape(
    elements: Vec<Option<ReturnSummary>>,
    properties: Vec<(&str, ReturnSummary)>,
) -> ReturnSummary {
    ReturnSummary {
        elements,
        properties: properties
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
        ..leaf("accessor", "x")
    }
}

#[test]
fn debug_sorted_returns_keep_the_baseline_order() {
    let a = || leaf("a", "a");
    let b = || leaf("a", "b");
    let nested = |child: ReturnSummary| shape(vec![Some(child)], vec![]);
    let mut rows: Vec<(&str, ReturnSummary)> = vec![
        ("map-two", shape(vec![], vec![("k", a()), ("l", a())])),
        ("vec-then-none", shape(vec![Some(a()), None], vec![])),
        (
            "label-quote-both",
            leaf("accessor", "ReturnSummary\", ContractReturn"),
        ),
        ("nested-deep-some", nested(nested(a()))),
        (
            "prototype-b",
            ReturnSummary {
                prototype: Some(recipe("b")),
                ..shape(vec![], vec![])
            },
        ),
        ("vec-b", shape(vec![Some(b())], vec![])),
        ("label-ab", leaf("accessor", "ab")),
        ("empty", shape(vec![], vec![])),
        ("map-one", shape(vec![], vec![("k", a())])),
        ("label-ab-escaped-quote", leaf("accessor", "ab\"c")),
        ("vec-none", shape(vec![None], vec![])),
        ("nested-deep-none", nested(shape(vec![None], vec![]))),
        (
            "parameter-one",
            ReturnSummary {
                parameter: Some(1),
                ..shape(vec![], vec![])
            },
        ),
        ("label-new-name", leaf("accessor", "ReturnSummary")),
        ("label-old-name", leaf("accessor", "ContractReturn")),
        ("label-abc", leaf("accessor", "abc")),
        ("vec-a", shape(vec![Some(a())], vec![])),
        (
            "prototype-a",
            ReturnSummary {
                prototype: Some(recipe("a")),
                ..shape(vec![], vec![])
            },
        ),
        ("kind-old-name", leaf("ContractReturn", "x")),
        ("kind-new-name", leaf("ReturnSummary", "x")),
        ("empty-again", shape(vec![], vec![])),
        (
            "label-struct-text",
            leaf("accessor", "ReturnSummary { kind: \"a\" }"),
        ),
        ("map-other-key", shape(vec![], vec![("j", a())])),
    ];
    rows.sort_by(|left, right| format!("{:?}", left.1).cmp(&format!("{:?}", right.1)));
    let order = rows.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert_eq!(
        order, EXPECTED_ORDER,
        "baseline Debug order of the summary rows moved"
    );
    let mut values = rows.into_iter().map(|(_, value)| value).collect::<Vec<_>>();
    values.dedup();
    assert_eq!(values.len(), EXPECTED_ORDER.len() - 1);
}

// Derived by hand from the byte order of the `Debug` text, then confirmed by
// running at the baseline commit: kind and label strings first (uppercase
// before lowercase, a space and `"` before the escape's `\`, a shorter string
// before its extension), then `parameter: None` before `Some(`, then
// `elements: [None`, `[Some(` and `[]` in that byte order, then a map entry
// before the closing `}`, and an absent prototype before a present one.
const EXPECTED_ORDER: &[&str] = &[
    "kind-old-name",
    "kind-new-name",
    "label-old-name",
    "label-struct-text",
    "label-new-name",
    "label-quote-both",
    "label-ab",
    "label-ab-escaped-quote",
    "label-abc",
    "vec-none",
    "vec-then-none",
    "vec-a",
    "vec-b",
    "nested-deep-none",
    "nested-deep-some",
    "map-other-key",
    "map-two",
    "map-one",
    "empty",
    "empty-again",
    "prototype-a",
    "prototype-b",
    "parameter-one",
];
