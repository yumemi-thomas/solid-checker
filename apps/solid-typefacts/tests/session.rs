//! Request validation and the exact error codes, against a fake backend.

mod common;

use common::request;
use solid_typefacts::context::Context;
use solid_typefacts::fake::FakeBackend;
use solid_typefacts::session::Session;
use solid_typefacts::wire::{Incoming, decode_request};
use std::sync::{Arc, Mutex};
use typefacts::v3::{FileChange, Operation, Response};

fn session(inline: bool) -> (Session, Arc<Mutex<u32>>) {
    let fake = FakeBackend::default();
    let closed = Arc::clone(&fake.closed);
    (
        Session::new(Box::new(fake), "/p/tsconfig.json", inline).unwrap(),
        closed,
    )
}

fn ask(session: &mut Session, request: typefacts::v3::Request) -> Response {
    let incoming = Incoming {
        request,
        bad_operation: None,
    };
    session.lifecycle(&Context::new(), &incoming)
}

fn code(response: &Response) -> Option<(&str, &str)> {
    response
        .error
        .as_ref()
        .map(|e| (e.code.as_str(), e.message.as_str()))
}

#[test]
fn validation_precedes_everything_in_go_order() {
    let (mut session, _) = session(true);
    let mut bad_schema = request(1, Operation::Open, 1);
    bad_schema.schema = 2;
    assert_eq!(
        code(&ask(&mut session, bad_schema)),
        Some(("invalid-request", "unsupported TypeFacts schema 2"))
    );
    assert_eq!(
        code(&ask(&mut session, request(0, Operation::Open, 1))),
        Some(("invalid-request", "type facts generation mismatch"))
    );
    assert_eq!(
        code(&ask(&mut session, request(1, Operation::Open, 0))),
        Some(("invalid-request", "type facts generation mismatch"))
    );
    let mut foreign = request(1, Operation::Open, 1);
    foreign.project_id = "/other/tsconfig.json".to_owned();
    assert_eq!(
        code(&ask(&mut session, foreign)),
        Some(("project-mismatch", "type facts generation mismatch"))
    );
    // The project id is compared cleaned.
    let mut unclean = request(1, Operation::Open, 1);
    unclean.project_id = "/p/./src/../tsconfig.json".to_owned();
    assert!(ask(&mut session, unclean).ok);
}

#[test]
fn an_unknown_operation_is_an_invalid_request_not_a_decode_failure() {
    let (mut session, _) = session(true);
    let bytes = typefacts::encode(&request(5, Operation::Open, 1)).unwrap();
    // Rewrite "open" to "boom" in the frame: same length keeps it canonical.
    let mut bytes = bytes;
    let at = bytes
        .windows(4)
        .position(|window| window == b"open")
        .unwrap();
    bytes[at..at + 4].copy_from_slice(b"boom");
    let incoming = decode_request(&bytes).unwrap();
    let response = session.lifecycle(&Context::new(), &incoming);
    assert_eq!(response.request_id, 5);
    assert_eq!(
        code(&response),
        Some((
            "invalid-request",
            "unsupported lifecycle operation \"boom\""
        ))
    );
}

#[test]
fn open_update_and_generation_rules() {
    let (mut session, _) = session(true);
    assert!(ask(&mut session, request(1, Operation::Open, 1)).ok);
    assert_eq!(
        code(&ask(&mut session, request(2, Operation::Open, 2))),
        Some(("generation-mismatch", "type facts generation mismatch"))
    );
    assert_eq!(
        code(&ask(&mut session, request(3, Operation::Update, 1))),
        Some(("generation-mismatch", "type facts generation mismatch"))
    );
    let mut update = request(4, Operation::Update, 2);
    update.changes = vec![FileChange {
        path: "/p/a.ts".into(),
        source: b"x".to_vec(),
        deleted: false,
        version: 1,
    }];
    let response = ask(&mut session, update);
    assert!(response.ok);
    assert_eq!(response.generation, 2);
    assert_eq!(response.affected, vec!["/p/a.ts".to_owned()]);
    assert_eq!(session.generation(), 2);
}

#[test]
fn a_failed_update_leaves_the_generation() {
    let fake = FakeBackend {
        fail_update: true,
        ..FakeBackend::default()
    };
    let mut session = Session::new(Box::new(fake), "/p/tsconfig.json", true).unwrap();
    let response = ask(&mut session, request(1, Operation::Update, 2));
    assert_eq!(code(&response), Some(("update-failed", "boom")));
    assert_eq!(response.generation, 1);
}

#[test]
fn analyze_and_symbols_run_go_pre_checks() {
    let (mut session, _) = session(true);
    let mut both = request(1, Operation::Analyze, 1);
    both.compact_demands = Some(typefacts::v3::CompactDemands {
        groups: Vec::new(),
        strings: Vec::new(),
    });
    both.demands = vec![typefacts::v3::EntityDemand::default()];
    assert_eq!(
        code(&ask(&mut session, both)).map(|c| c.0),
        Some("invalid-demands")
    );
    let mut stale = request(2, Operation::Analyze, 1);
    stale.state_token = "9".to_owned();
    assert_eq!(
        code(&ask(&mut session, stale)).map(|c| c.0),
        Some("state-mismatch")
    );
    let response = ask(&mut session, request(3, Operation::Symbols, 1));
    assert_eq!(
        code(&response),
        Some((
            "state-mismatch",
            "type facts generation mismatch: symbol evidence token \"\", retained token \"\""
        ))
    );
}

#[test]
fn sources_inline_and_through_an_arena() {
    let (mut inline, _) = session(true);
    let response = ask(&mut inline, request(1, Operation::Sources, 1));
    assert!(response.ok);
    assert_eq!(response.sources.len(), 2);
    assert_eq!(response.sources[0].source, b"let a");
    assert!(response.source_arena.is_empty() && response.source_lengths.is_empty());

    let (mut arena, _) = session(false);
    let response = ask(&mut arena, request(1, Operation::Sources, 1));
    assert!(response.ok);
    assert_eq!(response.source_lengths, vec![5, 0]);
    assert!(
        response
            .sources
            .iter()
            .all(|source| source.source.is_empty())
    );
    assert_eq!(std::fs::read(&response.source_arena).unwrap(), b"let a");
    // A second answer replaces the first file; close removes the last.
    let second = ask(&mut arena, request(2, Operation::Sources, 1));
    assert!(
        !std::path::Path::new(&response.source_arena).exists()
            || second.source_arena == response.source_arena
    );
    assert!(ask(&mut arena, request(3, Operation::Close, 1)).ok);
    assert!(!std::path::Path::new(&second.source_arena).exists());
}

#[test]
fn modules_reports_cancellation_distinctly() {
    let fake = FakeBackend {
        interrupt_modules: true,
        ..FakeBackend::default()
    };
    let mut session = Session::new(Box::new(fake), "/p/tsconfig.json", true).unwrap();
    let response = ask(&mut session, request(1, Operation::Modules, 1));
    assert_eq!(
        code(&response),
        Some(("analysis-cancelled", "context canceled"))
    );
}

#[test]
fn close_is_idempotent_and_final() {
    let (mut session, closed) = session(true);
    assert!(ask(&mut session, request(1, Operation::Close, 1)).ok);
    assert!(ask(&mut session, request(2, Operation::Close, 1)).ok);
    assert_eq!(
        code(&ask(&mut session, request(3, Operation::Open, 1))),
        Some(("session-closed", "Type Facts session is closed"))
    );
    assert_eq!(*closed.lock().unwrap(), 1);
    drop(session);
    assert_eq!(*closed.lock().unwrap(), 1);
}

#[test]
fn cancel_acknowledges_but_needs_a_valid_request() {
    let (mut session, _) = session(true);
    assert!(ask(&mut session, request(2, Operation::Cancel, 1)).ok);
    assert_eq!(
        code(&ask(&mut session, request(2, Operation::Cancel, 0))).map(|c| c.0),
        Some("invalid-request")
    );
}
