//! Ports `session_transaction_regression_test.go`.

use super::{
    backend_with, lifecycle_request, lifecycle_with, location, regression_demand, session_over,
};
use crate::context::Context;
use crate::fake::{FakeBackend, symbol_id};
use crate::session::Session;
use std::sync::{Arc, Mutex};
use typefacts::Location;
use typefacts::v3::{Operation, SymbolQuery};

/// Go's `postMaterializationCancelBackend`, as the handles the fake exposes.
struct Cancelling {
    session: Session,
    cancel_after_references: Arc<Mutex<Option<Context>>>,
    released: Arc<Mutex<u32>>,
    reference_before_release: Arc<Mutex<bool>>,
}

fn cancelling_session(files: &[(&str, &str)]) -> Cancelling {
    let mut backend: FakeBackend = backend_with(files);
    backend.strict_release = true;
    let cancel_after_references = Arc::clone(&backend.cancel_after_references);
    let released = Arc::clone(&backend.released);
    let reference_before_release = Arc::clone(&backend.reference_before_release);
    Cancelling {
        session: session_over(backend),
        cancel_after_references,
        released,
        reference_before_release,
    }
}

fn evidence_query(location: &Location) -> SymbolQuery {
    SymbolQuery {
        id: symbol_id(location),
        references: true,
        references_only: false,
    }
}

fn code(response: &typefacts::v3::Response) -> Option<&str> {
    response.error.as_ref().map(|error| error.code.as_str())
}

#[test]
fn cancelled_materialization_cannot_publish_a_closure_generation() {
    const PATH: &str = "/project/source.ts";
    let old_location = location(PATH, 1, 2);
    let mut harness = cancelling_session(&[(PATH, "export const value = 1\n")]);
    let session = &mut harness.session;

    let mut initial = lifecycle_request(1, Operation::Analyze, 1);
    initial.reset_state = true;
    initial.demands = vec![regression_demand(old_location.clone())];
    let initial_response = lifecycle_with(session, &Context::new(), initial);
    assert!(
        initial_response.ok && !initial_response.state_token.is_empty(),
        "initial analyze = {initial_response:?}"
    );

    let mut removed = lifecycle_request(2, Operation::Analyze, 1);
    removed
        .state_token
        .clone_from(&initial_response.state_token);
    removed.removed_demand_paths = vec![PATH.to_owned()];
    removed.symbol_queries = vec![evidence_query(&old_location)];
    let cancelled_context = Context::new();
    *harness.cancel_after_references.lock().unwrap() = Some(cancelled_context.clone());
    let cancelled = lifecycle_with(session, &cancelled_context, removed.clone());
    assert_eq!(
        code(&cancelled),
        Some("analysis-cancelled"),
        "post-materialization cancellation = {cancelled:?}"
    );
    let (published, token) = session.published();
    assert!(
        published.is_some() && token == initial_response.state_token,
        "cancelled analyze changed the session-published state"
    );
    assert!(
        session.closure().table().is_none() && session.closure().retained().get(PATH).is_none(),
        "cancelled materialization remained published inside the closure"
    );

    let mut retry = removed;
    retry.request_id = 3;
    let retry_response = lifecycle_with(session, &Context::new(), retry);
    assert!(
        retry_response.ok && retry_response.state_token != initial_response.state_token,
        "retry analyze = {retry_response:?}"
    );
}

#[test]
fn cancelled_demand_contribution_cannot_leak_when_another_path_changes() {
    const FIRST: &str = "/project/first.ts";
    const SECOND: &str = "/project/second.ts";
    let first_old = location(FIRST, 1, 2);
    let first_cancelled = location(FIRST, 5, 6);
    let second_old = location(SECOND, 1, 2);
    let second_next = location(SECOND, 5, 6);
    let mut harness = cancelling_session(&[
        (FIRST, "export const first = 1\n"),
        (SECOND, "export const second = 1\n"),
    ]);
    let session = &mut harness.session;

    let mut initial = lifecycle_request(1, Operation::Analyze, 1);
    initial.reset_state = true;
    initial.demands = vec![
        regression_demand(first_old.clone()),
        regression_demand(second_old),
    ];
    let initial_response = lifecycle_with(session, &Context::new(), initial);
    assert!(
        initial_response.ok && !initial_response.state_token.is_empty(),
        "initial analyze = {initial_response:?}"
    );

    let mut change_first = lifecycle_request(2, Operation::Analyze, 1);
    change_first
        .state_token
        .clone_from(&initial_response.state_token);
    change_first.demands = vec![regression_demand(first_cancelled)];
    change_first.symbol_queries = vec![evidence_query(&first_old)];
    let cancelled_context = Context::new();
    *harness.cancel_after_references.lock().unwrap() = Some(cancelled_context.clone());
    let cancelled = lifecycle_with(session, &cancelled_context, change_first);
    assert_eq!(
        code(&cancelled),
        Some("analysis-cancelled"),
        "post-materialization cancellation = {cancelled:?}"
    );
    assert!(
        session.closure().table().is_none() && session.closure().retained().get(FIRST).is_none(),
        "cancelled demand contribution remained published inside the closure"
    );

    let mut change_second = lifecycle_request(3, Operation::Analyze, 1);
    change_second
        .state_token
        .clone_from(&initial_response.state_token);
    change_second.demands = vec![regression_demand(second_next.clone())];
    let accepted = lifecycle_with(session, &Context::new(), change_second);
    assert!(accepted.ok, "second-path analyze = {accepted:?}");
    let retained_first = session.demands().at(FIRST).expect("first-path run");
    assert!(
        retained_first.len() == 1 && retained_first[0].location == first_old,
        "session demand state no longer represents the last accepted first-path run"
    );
    let retained_second = session.demands().at(SECOND).expect("second-path run");
    assert!(
        retained_second.len() == 1 && retained_second[0].location == second_next,
        "accepted second-path demand was not published"
    );
}

#[test]
fn symbol_evidence_precedes_analysis_release() {
    const PATH: &str = "/project/source.ts";
    let location = location(PATH, 1, 2);
    let mut harness = cancelling_session(&[(PATH, "export const value = 1\n")]);
    let mut request = lifecycle_request(1, Operation::Analyze, 1);
    request.reset_state = true;
    request.demands = vec![regression_demand(location.clone())];
    request.symbol_queries = vec![evidence_query(&location)];
    request.release_analysis = true;
    let response = lifecycle_with(&mut harness.session, &Context::new(), request);
    assert!(response.ok, "analysis with symbol evidence = {response:?}");
    assert!(
        *harness.reference_before_release.lock().unwrap() && *harness.released.lock().unwrap() == 1,
        "evidence/release order"
    );
}
