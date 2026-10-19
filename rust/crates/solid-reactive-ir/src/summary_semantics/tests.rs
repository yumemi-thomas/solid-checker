//! Characterization pins for the shared summary vocabulary. Every expected
//! value is a literal fixed at the baseline commit, before the vocabulary
//! moved into this module; none is computed by the code under test.

use std::collections::BTreeMap;

use super::{CallbackSchedule, CallbackSummary, InvokeProtocol, ReturnSummary};

fn leaf(kind: &str, label: &str) -> ReturnSummary {
    ReturnSummary {
        kind: kind.into(),
        label: label.into(),
        ..ReturnSummary::default()
    }
}

// T1: `interproc::equivalent_callbacks` uses this JSON as an equality key, so
// the key names, their order, `null` for `None` and the ordered map are all
// part of the contract.
#[test]
fn return_summary_serializes_with_a_fixed_key_order() {
    let inner = ReturnSummary {
        kind: "plain".into(),
        label: String::new(),
        parameter: None,
        elements: Vec::new(),
        properties: BTreeMap::new(),
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
