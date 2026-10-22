//! The session's transport half over the real demand closure and a fake
//! compiler: full, delta, sparse, reuse, and the sparse-unavailable rebuild,
//! inline and through the arena, plus symbol-evidence packing and the failure
//! codes around them.

mod common;

use common::request;
use solid_typefacts::arena;
use solid_typefacts::context::Context;
use solid_typefacts::fake::{FakeBackend, symbol_id};
use solid_typefacts::session::Session;
use solid_typefacts::wire::Incoming;
use std::path::PathBuf;
use std::sync::Arc;
use typefacts::Location;
use typefacts::producer::{
    TransitionMode, TransitionSummary, inspect_table_transition, reencode_table_transition,
};
use typefacts::v3::{EntityDemand, FileChange, Operation, Request, Response, SymbolQuery};

const PROJECT: &str = "/p/tsconfig.json";

fn demand(path: &str, start: u64) -> EntityDemand {
    EntityDemand {
        location: Location {
            path: path.into(),
            start_byte: start,
            end_byte: start + 3,
        },
        symbol: true,
        references: true,
        resolved_call: true,
        callability: true,
        ..EntityDemand::default()
    }
}

fn analyze(id: u64, generation: u64, token: &str, demands: Vec<EntityDemand>) -> Request {
    let mut analyze = request(id, Operation::Analyze, generation);
    analyze.state_token = token.to_owned();
    analyze.reset_state = token.is_empty();
    analyze.demands = demands;
    analyze
}

fn update(id: u64, generation: u64, path: &str, source: &str) -> Request {
    let mut update = request(id, Operation::Update, generation);
    update.changes = vec![FileChange {
        path: path.to_owned(),
        source: source.as_bytes().to_vec(),
        deleted: false,
        version: generation,
    }];
    update
}

fn ask(session: &mut Session, request: Request) -> Response {
    ask_into(session, request, None)
}

fn ask_into(
    session: &mut Session,
    request: Request,
    arena: Option<&mut arena::TransitionArena>,
) -> Response {
    let incoming = Incoming {
        request,
        bad_operation: None,
    };
    let ctx = Context::new();
    match arena {
        Some(arena) => {
            arena.begin(incoming.request.request_id).unwrap();
            let mut response = session.lifecycle_into(&ctx, &incoming, Some(&mut *arena));
            if let Some((path, descriptor)) = arena.descriptor() {
                response.source_arena = path;
                response.source_lengths = descriptor;
            }
            response
        }
        None => session.lifecycle(&ctx, &incoming),
    }
}

fn summary(response: &Response) -> TransitionSummary {
    inspect_table_transition(&response.table_transition).unwrap()
}

fn session_over(fake: FakeBackend) -> Session {
    Session::new(Box::new(fake), PROJECT, true).unwrap()
}

fn code(response: &Response) -> Option<&str> {
    response.error.as_ref().map(|error| error.code.as_str())
}

#[test]
fn full_then_sparse_then_reuse() {
    let mut session = session_over(FakeBackend::default());
    let demands = vec![demand("/p/a.ts", 1), demand("/p/b.ts", 5)];

    let first = ask(&mut session, analyze(1, 1, "", demands));
    assert!(first.ok, "{:?}", first.error);
    assert_eq!(first.state_token, "1");
    let timings = first.timings.expect("timings");
    assert!(timings.materialized);
    let full = summary(&first);
    assert_eq!(full.mode, TransitionMode::Full);
    assert_eq!((full.base_generation, full.target_generation), (0, 1));
    assert_eq!(full.path_operations, 2);
    // Symbol closure belongs to the client: the demand table has no symbol rows.
    assert_eq!(full.symbol_operations, 0);
    assert_eq!(full.table_schema, typefacts::v3::TYPE_FACTS_SCHEMA_V1 + 21);
    assert!(reencode_table_transition(&first.table_transition).unwrap() == first.table_transition);

    // Nothing new in the same generation: the retained table answers.
    let reuse = ask(&mut session, analyze(2, 1, "1", Vec::new()));
    assert!(reuse.ok);
    assert_eq!(reuse.state_token, "1");
    assert!(reuse.table_transition.is_empty());
    assert_eq!(reuse.timings.map(|t| t.analyze_ns), Some(0));

    // An edit advances the generation; the closure's exact manifest makes the
    // delta sparse: one unconditional upsert for the edited path.
    assert!(ask(&mut session, update(3, 2, "/p/a.ts", "edited")).ok);
    let delta = ask(&mut session, analyze(4, 2, "1", Vec::new()));
    assert!(delta.ok, "{:?}", delta.error);
    assert_eq!(delta.state_token, "2");
    let sparse = summary(&delta);
    assert_eq!(sparse.mode, TransitionMode::Delta);
    assert_eq!(sparse.base_state_token, "1");
    assert_eq!((sparse.base_generation, sparse.target_generation), (1, 2));
    assert_eq!(sparse.path_operations, 1);
    assert_eq!(sparse.symbol_operations, 0);

    // A generation that changed no path still advances the retained identity:
    // an empty delta frame, not reuse.
    assert!(ask(&mut session, update(5, 3, "/p/zzz.ts", "unrelated")).ok);
    let advance = ask(&mut session, analyze(6, 3, "2", vec![demand("/p/a.ts", 1)]));
    assert!(advance.ok, "{:?}", advance.error);
    let advanced = summary(&advance);
    assert_eq!(advanced.mode, TransitionMode::Delta);
    assert_eq!(advanced.target_generation, 3);

    // A stale token is refused before anything runs.
    let stale = ask(&mut session, analyze(7, 3, "1", Vec::new()));
    assert_eq!(code(&stale), Some("state-mismatch"));
}

#[test]
fn a_missing_manifest_rebuilds_the_table_in_full() {
    let fake = FakeBackend::default();
    let semantic_calls = Arc::clone(&fake.semantic_calls);
    let mut session = session_over(fake);
    let first = ask(&mut session, analyze(1, 1, "", vec![demand("/p/a.ts", 1)]));
    assert_eq!(summary(&first).mode, TransitionMode::Full);

    // Two accepted updates with no analysis between them: the second one's
    // predecessor is no table at all, so the closure cannot prove a manifest.
    assert!(ask(&mut session, update(2, 2, "/p/a.ts", "edited")).ok);
    assert!(ask(&mut session, update(3, 3, "/p/a.ts", "edited again")).ok);
    let rebuilt = ask(&mut session, analyze(4, 3, "1", Vec::new()));
    assert!(rebuilt.ok, "{:?}", rebuilt.error);
    let full = summary(&rebuilt);
    assert_eq!(
        full.mode,
        TransitionMode::Full,
        "no sparse without a manifest"
    );
    assert_eq!(full.base_state_token, "");
    assert_eq!(full.target_generation, 3);
    // The cold analysis, the sparse attempt, and the one full rebuild.
    assert_eq!(*semantic_calls.lock().unwrap(), 3);
    // The token still advances, so the client's next request names it.
    assert_eq!(rebuilt.state_token, "2");
}

fn arena_file(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "solid-typefacts-rs-{name}-{}-arena",
        std::process::id()
    ));
    std::fs::write(&path, b"").unwrap();
    path
}

/// Reads a committed transition the way the client does
/// (`shared_transition_arena.rs`): header, then the descriptor's range.
fn read_arena(response: &Response) -> Vec<u8> {
    use std::io::{Read, Seek, SeekFrom};
    assert!(response.table_transition.is_empty());
    let [request_id, offset, length] = response.source_lengths[..] else {
        panic!("descriptor {:?}", response.source_lengths);
    };
    assert_eq!(request_id, response.request_id);
    let mut file = std::fs::File::open(&response.source_arena).unwrap();
    let mut header = [0_u8; arena::HEADER_SIZE];
    file.read_exact(&mut header).unwrap();
    assert_eq!(header[..8], arena::MAGIC);
    assert_eq!(
        u64::from_le_bytes(header[8..16].try_into().unwrap()),
        request_id
    );
    assert_eq!(
        u64::from_le_bytes(header[16..24].try_into().unwrap()),
        offset
    );
    assert_eq!(
        u64::from_le_bytes(header[24..32].try_into().unwrap()),
        length
    );
    assert!(offset >= arena::HEADER_SIZE as u64);
    let mut bytes = vec![0; usize::try_from(length).unwrap()];
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.read_exact(&mut bytes).unwrap();
    bytes
}

#[test]
fn the_arena_carries_exactly_the_bytes_the_response_would() {
    let demands = vec![demand("/p/a.ts", 1), demand("/p/b.ts", 5)];
    let mut inline = session_over(FakeBackend::default());
    let expected = ask(&mut inline, analyze(1, 1, "", demands.clone()));
    assert!(!expected.table_transition.is_empty());

    let path = arena_file("same-bytes");
    let mut arena = arena::open(path.to_str().unwrap()).unwrap().unwrap();
    let mut streamed = session_over(FakeBackend::default());
    let response = ask_into(&mut streamed, analyze(1, 1, "", demands), Some(&mut arena));
    assert!(response.ok, "{:?}", response.error);
    assert_eq!(response.source_arena, path.to_str().unwrap());
    assert_eq!(read_arena(&response), expected.table_transition);

    // The next request truncates back to the rows offset and rewrites; a reuse
    // commits nothing, so it carries no descriptor.
    let reuse = ask_into(
        &mut streamed,
        analyze(2, 1, "1", Vec::new()),
        Some(&mut arena),
    );
    assert!(reuse.ok && reuse.source_arena.is_empty() && reuse.source_lengths.is_empty());
    assert!(
        ask(&mut inline, update(3, 2, "/p/a.ts", "x")).ok
            && ask_into(&mut streamed, update(3, 2, "/p/a.ts", "x"), None).ok
    );
    let inline_delta = ask(&mut inline, analyze(4, 2, "1", Vec::new()));
    arena.begin(4).unwrap();
    let streamed_delta = ask_into(
        &mut streamed,
        analyze(4, 2, "1", Vec::new()),
        Some(&mut arena),
    );
    assert_eq!(read_arena(&streamed_delta), inline_delta.table_transition);
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        arena::ROWS_OFFSET + {
            let [_, offset, length] = streamed_delta.source_lengths[..] else {
                unreachable!()
            };
            // The rows run from the fixed offset; the prefix sits before it.
            length - (arena::ROWS_OFFSET - offset)
        }
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn symbols_pack_into_a_full_transition_or_stay_inline() {
    let mut session = session_over(FakeBackend::default());
    let first = ask(&mut session, analyze(1, 1, "", vec![demand("/p/a.ts", 1)]));
    assert!(reencode_table_transition(&first.table_transition).unwrap() == first.table_transition);
    let id = symbol_id(&Location {
        path: "/p/a.ts".into(),
        start_byte: 1,
        end_byte: 4,
    })
    .to_string();
    let mut symbols = request(2, Operation::Symbols, 1);
    symbols.state_token = "1".to_owned();
    symbols.symbol_queries = vec![SymbolQuery {
        id: id.clone().into(),
        references: true,
        references_only: false,
    }];
    let packed = ask(&mut session, symbols);
    assert!(packed.ok, "{:?}", packed.error);
    assert!(packed.symbol_evidence.is_empty());
    let summary = summary(&packed);
    assert_eq!(summary.mode, TransitionMode::Full);
    assert_eq!(summary.target_generation, 1);
    assert_eq!(summary.path_operations, 0);
    assert_eq!(summary.symbol_operations, 1);

    // No evidence to pack: the answer is the (empty) inline list.
    let mut none = request(3, Operation::Symbols, 1);
    none.state_token = "1".to_owned();
    let empty = ask(&mut session, none);
    assert!(empty.ok && empty.table_transition.is_empty() && empty.symbol_evidence.is_empty());

    // Through an arena the same frame lands in the file.
    let path = arena_file("symbols");
    let mut arena = arena::open(path.to_str().unwrap()).unwrap().unwrap();
    let mut again = request(4, Operation::Symbols, 1);
    again.state_token = "1".to_owned();
    again.symbol_queries = vec![SymbolQuery {
        id: id.into(),
        references: true,
        references_only: false,
    }];
    let streamed = ask_into(&mut session, again, Some(&mut arena));
    assert_eq!(read_arena(&streamed), packed.table_transition);
    let _ = std::fs::remove_file(path);
}

#[test]
fn reference_changes_and_release_ride_along_with_analyze() {
    let fake = FakeBackend::default();
    let released = Arc::clone(&fake.released);
    let mut session = session_over(fake);
    let mut cold = analyze(1, 1, "", vec![demand("/p/a.ts", 1)]);
    cold.reference_changes = true;
    cold.release_analysis = true;
    let response = ask(&mut session, cold);
    assert!(response.ok, "{:?}", response.error);
    assert!(response.reference_changes_exact);
    assert_eq!(*released.lock().unwrap(), 1);
}

#[test]
fn failures_carry_go_codes() {
    // The backend cannot resolve the demands.
    let fake = FakeBackend {
        fail_semantic: Some("checker unavailable".to_owned()),
        ..FakeBackend::default()
    };
    let mut session = session_over(fake);
    let failed = ask(&mut session, analyze(1, 1, "", vec![demand("/p/a.ts", 1)]));
    assert_eq!(code(&failed), Some("analysis-failed"));
    assert_eq!(failed.error.unwrap().message, "checker unavailable");
    // Nothing was retained: the next analyze still presents no token.
    assert_eq!(
        code(&ask(&mut session, analyze(2, 1, "1", Vec::new()))),
        Some("state-mismatch")
    );

    // A table the wire cannot carry (an inverted span) is an assembly failure,
    // and publishes nothing: the state token stays where it was.
    let mut session = session_over(FakeBackend::default());
    let mut inverted = demand("/p/a.ts", 1);
    inverted.location.end_byte = 0;
    let assembly = ask(&mut session, analyze(1, 1, "", vec![inverted]));
    assert_eq!(code(&assembly), Some("assembly-failed"));
    assert_eq!(
        assembly.state_token, "1",
        "Go reports the token it would have issued"
    );
    let retry = ask(&mut session, analyze(2, 1, "", vec![demand("/p/a.ts", 1)]));
    assert!(retry.ok, "{:?}", retry.error);
    assert_eq!(retry.state_token, "1");

    // A cancelled request never publishes.
    let mut session = session_over(FakeBackend::default());
    let incoming = Incoming {
        request: analyze(1, 1, "", vec![demand("/p/a.ts", 1)]),
        bad_operation: None,
    };
    let ctx = Context::new();
    ctx.cancel();
    let cancelled = session.lifecycle(&ctx, &incoming);
    assert_eq!(code(&cancelled), Some("analysis-cancelled"));
    let retry = ask(&mut session, analyze(2, 1, "", vec![demand("/p/a.ts", 1)]));
    assert!(retry.ok && retry.state_token == "1", "{:?}", retry.error);
}
