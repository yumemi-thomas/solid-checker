//! Ports `session_test.go`.

use super::{
    PROJECT, backend_with, demand_at, lifecycle, lifecycle_request, location, session_over,
};
use crate::context::Context;
use crate::fake::FakeBackend;
use crate::session::Session;
use crate::wire::Incoming;
use std::sync::Arc;
use typefacts::producer::inspect_table_transition;
use typefacts::v3::{
    EntityDemand, FileChange, ModuleGraphRequest, Operation, TYPE_FACTS_SCHEMA_V1, compact_demands,
};
use typefacts::{ModuleFact, ModuleImportFact, ModuleResolution};

const SOURCE: &str = "/project/source.ts";

fn source_backend() -> FakeBackend {
    backend_with(&[(SOURCE, "export const value = 1\n")])
}

fn source_session() -> Session {
    session_over(source_backend())
}

#[test]
fn demand_preparation_keeps_every_group() {
    // The Go benchmark `BenchmarkSessionDemandPreparation`, as a correctness
    // check: one edit among 1,000 retained paths leaves 1,000 groups.
    const PATHS: u64 = 1_000;
    let initial: Vec<EntityDemand> = (0..PATHS)
        .map(|index| EntityDemand {
            location: location(&format!("/project/file-{index:04}.ts"), 1, 2),
            symbol: true,
            ..EntityDemand::default()
        })
        .collect();
    let mut retained = crate::demands::RetainedDemandStore::default();
    retained.begin(&initial, &[], true).commit();
    let change = [EntityDemand {
        location: location("/project/file-0500.ts", 2, 3),
        symbol: true,
        ..EntityDemand::default()
    }];
    let transaction = retained.begin(&change, &[], false);
    assert_eq!(usize::try_from(PATHS).unwrap(), transaction.groups().len());
    transaction.commit();
}

#[test]
fn session_owns_retained_lifecycle_state() {
    let mut session = source_session();

    let open = lifecycle(&mut session, lifecycle_request(1, Operation::Open, 1));
    assert!(open.ok && open.generation == 1, "{open:?}");

    let mut first_request = lifecycle_request(2, Operation::Analyze, 1);
    first_request.reset_state = true;
    let first = lifecycle(&mut session, first_request);
    assert!(
        first.ok && first.state_token == "1" && !first.table_transition.is_empty(),
        "initial analyze response = {first:?}"
    );
    let mut reuse_request = lifecycle_request(3, Operation::Analyze, 1);
    reuse_request.state_token.clone_from(&first.state_token);
    let reuse = lifecycle(&mut session, reuse_request);
    assert!(
        reuse.ok && reuse.table_transition.is_empty() && reuse.state_token == first.state_token,
        "warm analyze response = {reuse:?}"
    );

    let mut stale_request = lifecycle_request(4, Operation::Analyze, 1);
    stale_request.state_token = "stale".to_owned();
    let stale = lifecycle(&mut session, stale_request);
    assert_eq!(
        stale.error.as_ref().map(|e| e.code.as_str()),
        Some("state-mismatch")
    );

    // An accepted no-op update still advances exactly one protocol generation.
    let update = lifecycle(&mut session, lifecycle_request(5, Operation::Update, 2));
    assert!(
        update.ok && update.generation == 2,
        "no-op update = {update:?}"
    );
    let mut next_request = lifecycle_request(6, Operation::Analyze, 2);
    next_request.state_token.clone_from(&first.state_token);
    let next = lifecycle(&mut session, next_request);
    assert!(
        next.ok && next.state_token == "2" && !next.table_transition.is_empty(),
        "post-update analyze response = {next:?}"
    );
    // The frame's expansion and application are the client's obligation,
    // pinned by the delta golden; here the producer's promise is the mode, the
    // token, and a non-empty frame for the advanced generation.
    assert_eq!(next.generation, 2);
}

#[test]
fn session_transfers_expanded_rows_and_keeps_sparse_incremental_proof() {
    let demand = EntityDemand {
        location: location(SOURCE, 7, 12),
        symbol: true,
        references: true,
        ..EntityDemand::default()
    };
    let mut session = source_session();

    let analyze = |session: &mut Session| {
        let mut request = lifecycle_request(1, Operation::Analyze, 1);
        request.reset_state = true;
        request.demands = vec![demand.clone()];
        lifecycle(session, request)
    };
    let cold = analyze(&mut session);
    assert!(cold.ok, "cold response: {:?}", cold.error);
    {
        let contribution = session.closure().retained().get(SOURCE).expect("retained");
        let contribution = contribution.borrow();
        assert!(
            contribution.entities.is_none() && !contribution.roots.is_empty(),
            "retained contribution did not transfer rows: {contribution:?}"
        );
    }
    let table = session.closure().table().expect("closure table");
    assert!(
        table.entities.is_empty() && table.files.is_empty() && table.sources.is_empty(),
        "retained an expanded transport table after publication"
    );

    let update = lifecycle(&mut session, lifecycle_request(2, Operation::Update, 2));
    assert!(update.ok, "update: {:?}", update.error);
    let mut next = lifecycle_request(3, Operation::Analyze, 2);
    next.state_token.clone_from(&cold.state_token);
    let response = lifecycle(&mut session, next);
    assert!(
        response.ok && !response.table_transition.is_empty(),
        "sparse successor: {:?}",
        response.error
    );
    let contribution = session.closure().retained().get(SOURCE).expect("retained");
    assert!(
        contribution.borrow().entities.is_none(),
        "sparse successor reacquired expanded retained rows"
    );
}

#[test]
fn latest_semantic_domains_are_available_in_v1() {
    let demand = EntityDemand {
        location: location(SOURCE, 7, 12),
        constructability: true,
        runtime_value_domain: true,
        primitive_value_domain: true,
        call_result_domain: true,
        constant_value: true,
        array_shape: true,
        tuple_shape: true,
        library_types: true,
        ..EntityDemand::default()
    };
    let mut session = source_session();
    let mut request = lifecycle_request(1, Operation::Analyze, 1);
    request.reset_state = true;
    request.demands = vec![demand.clone()];
    let response = lifecycle(&mut session, request);
    assert!(
        response.ok,
        "v1 rejected semantic domains: {:?}",
        response.error
    );
    let summary = inspect_table_transition(&response.table_transition).unwrap();
    assert_eq!(
        summary.table_schema,
        TYPE_FACTS_SCHEMA_V1 + 21,
        "active wire table schema"
    );

    let mut compact_session = source_session();
    let mut request = lifecycle_request(1, Operation::Analyze, 1);
    request.reset_state = true;
    request.compact_demands = Some(compact_demands(&[demand]));
    let response = lifecycle(&mut compact_session, request);
    assert!(
        response.ok,
        "v1 rejected compact semantic domains: {:?}",
        response.error
    );
}

fn arena_session() -> Session {
    Session::new(Box::new(source_backend()), PROJECT, false).unwrap()
}

#[test]
fn session_returns_sources_through_one_shared_arena() {
    let mut session = arena_session();

    let initial = lifecycle(&mut session, lifecycle_request(1, Operation::Sources, 1));
    assert!(initial.ok && initial.sources.len() == 1, "{initial:?}");
    let arena = std::fs::read(&initial.source_arena).unwrap();
    let length = usize::try_from(initial.source_lengths[0]).unwrap();
    assert_eq!(&arena[..length], b"export const value = 1\n");

    let mut update_request = lifecycle_request(2, Operation::Update, 2);
    update_request.changes = vec![FileChange {
        path: SOURCE.to_owned(),
        version: 1,
        source: b"export const value = 2\n".to_vec(),
        deleted: false,
    }];
    let update = lifecycle(&mut session, update_request);
    assert!(update.ok, "{update:?}");
    let updated = lifecycle(&mut session, lifecycle_request(3, Operation::Sources, 2));
    assert!(updated.ok && updated.sources.len() == 1, "{updated:?}");
    let arena = std::fs::read(&updated.source_arena).unwrap();
    let length = usize::try_from(updated.source_lengths[0]).unwrap();
    // Go's double ignores the edit, so its arena still holds the original
    // bytes; what the test pins is that the answer is read from the backend at
    // the new generation through the (replaced) arena.
    assert_eq!(&arena[..length], b"export const value = 1\n");
}

#[test]
fn session_cancellation_does_not_commit_retained_state() {
    let mut session = source_session();

    let cancelled = Context::new();
    cancelled.cancel();
    let mut request = lifecycle_request(1, Operation::Analyze, 1);
    request.reset_state = true;
    let response = session.lifecycle(
        &cancelled,
        &Incoming {
            request: request.clone(),
            bad_operation: None,
        },
    );
    assert_eq!(
        response.error.as_ref().map(|e| e.code.as_str()),
        Some("analysis-cancelled"),
        "{response:?}"
    );

    let retry = lifecycle(&mut session, request);
    assert!(
        retry.ok && retry.state_token == "1" && !retry.table_transition.is_empty(),
        "retry response = {retry:?}"
    );
}

#[test]
fn session_owns_project_closure() {
    let backend = source_backend();
    let closed = Arc::clone(&backend.closed);
    let mut session = session_over(backend);

    let stale_close = lifecycle(&mut session, lifecycle_request(1, Operation::Close, 2));
    assert_eq!(
        stale_close.error.as_ref().map(|e| e.code.as_str()),
        Some("generation-mismatch")
    );
    assert_eq!(*closed.lock().unwrap(), 0, "stale close closed the backend");

    let close_request = lifecycle_request(1, Operation::Close, 1);
    let first = lifecycle(&mut session, close_request.clone());
    let second = lifecycle(&mut session, close_request);
    assert!(
        first.ok && second.ok,
        "close responses = {first:?}, {second:?}"
    );
    session.close().unwrap();
    assert_eq!(*closed.lock().unwrap(), 1, "backend close count");

    let analyze = lifecycle(&mut session, lifecycle_request(2, Operation::Analyze, 1));
    assert_eq!(
        analyze.error.as_ref().map(|e| e.code.as_str()),
        Some("session-closed"),
        "{analyze:?}"
    );
}

/// Pins that the doubles drive the branches the producer actually ships. A
/// double that only satisfied the unscoped surface would send every fast unit
/// test through a fallback no release runs, and the retention machinery this
/// asserts on would go untested outside one integration test.
#[test]
fn session_analysis_traverses_the_retained_path() {
    const FIRST: &str = "/project/source.ts";
    const SECOND: &str = "/project/other.ts";
    let mut session = session_over(backend_with(&[
        (FIRST, "export const value = 1\n"),
        (SECOND, "export const other = 2\n"),
    ]));

    let demands = vec![
        EntityDemand {
            location: location(FIRST, 13, 18),
            symbol: true,
            references: true,
            ..EntityDemand::default()
        },
        EntityDemand {
            location: location(SECOND, 13, 18),
            symbol: true,
            references: true,
            ..EntityDemand::default()
        },
    ];
    let mut cold = lifecycle_request(1, Operation::Analyze, 1);
    cold.reset_state = true;
    cold.demands = demands;
    let response = lifecycle(&mut session, cold);
    assert!(
        response.ok && !response.table_transition.is_empty(),
        "cold analyze = {response:?}"
    );
    let token = "1";

    let mut update = lifecycle_request(2, Operation::Update, 2);
    update.changes = vec![FileChange {
        path: FIRST.to_owned(),
        version: 1,
        source: b"export const value = 11\n".to_vec(),
        deleted: false,
    }];
    assert!(lifecycle(&mut session, update).ok);

    let mut warm = lifecycle_request(3, Operation::Analyze, 2);
    warm.state_token = token.to_owned();
    let warm_response = lifecycle(&mut session, warm);
    assert!(warm_response.ok, "warm analyze = {warm_response:?}");

    let retention = session.closure().stats().retention;
    assert!(
        retention.retained_files != 0,
        "no file was retained across the update; retention = {retention:?}"
    );
    // The active V1 session always uses the latest semantic transport path;
    // retained files are the stable contract here.
    assert!(
        !warm_response.table_transition.is_empty(),
        "warm analyze omitted its generation-advancing transition; retention = {retention:?}"
    );
}

#[test]
fn session_answers_the_module_graph_without_touching_retained_state() {
    let mut backend = source_backend();
    backend.module_inventory = Some(crate::backend::ModuleInventory {
        modules: vec![
            ModuleFact {
                path: "/project/shape.d.ts".into(),
                declaration_file: true,
                format: typefacts::ModuleFormat::Esm,
                project_reference: None,
                redirect_targets: Arc::from(Vec::new()),
            },
            ModuleFact {
                path: SOURCE.into(),
                declaration_file: false,
                format: typefacts::ModuleFormat::Esm,
                project_reference: None,
                redirect_targets: Arc::from(Vec::new()),
            },
        ],
        imports: vec![ModuleImportFact {
            specifier: location(SOURCE, 20, 29),
            text: "./shape".into(),
            resolution: ModuleResolution::Relative,
            resolved_path: "/project/shape.d.ts".into(),
            included_path: "".into(),
            symlink_path: "".into(),
            extension: ".d.ts".into(),
            ts_extension: false,
            paths_pattern: "".into(),
            package: None,
            resolver_package: None,
        }],
        unknown_import_paths: vec!["/project/absent.ts".into()],
    });
    let demands = Arc::clone(&backend.module_demands);
    let mut session = session_over(backend);

    let mut analyze_request = lifecycle_request(1, Operation::Analyze, 1);
    analyze_request.reset_state = true;
    analyze_request.demands = vec![demand_at(SOURCE, 13)];
    let analyze = lifecycle(&mut session, analyze_request);
    assert!(analyze.ok, "analyze response = {analyze:?}");
    let token = analyze.state_token;

    let mut modules_request = lifecycle_request(2, Operation::Modules, 1);
    modules_request.module_graph = Some(ModuleGraphRequest {
        imports: true,
        import_paths: vec![SOURCE.to_owned()],
        packages: true,
    });
    let modules = lifecycle(&mut session, modules_request);
    assert!(modules.ok, "modules response = {modules:?}");
    assert!(
        modules.modules.len() == 2 && modules.module_imports.len() == 1,
        "modules answer = {modules:?}"
    );
    assert_eq!(
        modules.unknown_import_paths,
        vec![Arc::<str>::from("/project/absent.ts")],
        "the one requested path the program lacks"
    );
    let seen = demands.lock().unwrap();
    assert!(
        seen.len() == 1 && seen[0].imports && seen[0].packages,
        "backend saw demands {seen:?}, want the request's own"
    );
    // The read carried no state token and issued none, so the analysis the
    // session is holding is exactly where it was.
    assert!(
        modules.state_token.is_empty(),
        "modules issued a state token"
    );
    let mut reuse_request = lifecycle_request(3, Operation::Analyze, 1);
    reuse_request.state_token.clone_from(&token);
    let reuse = lifecycle(&mut session, reuse_request);
    assert!(
        reuse.ok && reuse.state_token == token,
        "the retained analysis did not survive a modules read: {reuse:?}"
    );
}

/// A backend with no compiler resolution cannot answer a partial graph and be
/// believed, so it fails the request instead.
#[test]
fn session_fails_closed_when_the_backend_has_no_module_graph() {
    let mut session = source_session();
    let response = lifecycle(&mut session, lifecycle_request(1, Operation::Modules, 1));
    assert!(!response.ok, "modules answered {response:?}");
    let error = response.error.expect("error");
    assert_eq!(error.code, "modules-failed");
    assert_eq!(
        error.message,
        "type facts backend reports no resolved module graph"
    );
    assert!(
        response.modules.is_empty(),
        "a failed modules response carried modules"
    );
}
