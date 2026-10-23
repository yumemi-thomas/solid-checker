//! Closure behaviour the Go package pins only through compiler-backed
//! integration tests (`non_durable_integration_test.go`,
//! `async_function_integration_test.go`, `semantic_retained_integration_test.go`),
//! reproduced over the fake so the materialization branches are exercised
//! without a compiler.

use super::{backend_with, location};
use crate::closure::{DemandClosure, DemandInputGroup};
use crate::context::Context;
use std::collections::BTreeSet;
use std::sync::Arc;
use typefacts::AsyncFunctionFact;
use typefacts::producer::ProducerTable;
use typefacts::v3::{EntityDemand, FileChange};

const A: &str = "/p/a.ts";
const B: &str = "/p/b.ts";

fn group(path: &str, demands: Vec<EntityDemand>) -> DemandInputGroup {
    DemandInputGroup {
        path: path.to_owned(),
        demands,
    }
}

fn edit(closure: &mut DemandClosure, path: &str) {
    closure
        .update(
            &Context::new(),
            &[FileChange {
                path: path.to_owned(),
                source: b"edited".to_vec(),
                deleted: false,
                version: 1,
            }],
        )
        .unwrap();
}

fn analyze(
    closure: &mut DemandClosure,
    groups: &[DemandInputGroup],
    changed: &[&str],
) -> ProducerTable {
    let generation = closure.generation();
    let changed: Vec<String> = changed.iter().map(|path| (*path).to_owned()).collect();
    closure
        .demand_table_for_groups(&Context::new(), generation, groups, &changed)
        .unwrap()
}

fn async_fact(path: &str) -> AsyncFunctionFact {
    AsyncFunctionFact {
        expression: location(path, 3, 9),
        symbol: format!("symbol:h:{path}:fn").into(),
        target: "".into(),
        can_return_async: true,
        calls_after_await: Vec::new(),
    }
}

fn async_demand(path: &str) -> EntityDemand {
    EntityDemand {
        location: location(path, 3, 9),
        r#async: true,
        ..EntityDemand::default()
    }
}

fn async_functions(table: &ProducerTable, path: &str) -> usize {
    table
        .files
        .iter()
        .find(|file| &*file.path == path)
        .map_or(0, |file| file.async_functions.len())
}

#[test]
fn async_facts_are_requeried_after_every_update_and_demand_change() {
    let mut backend = backend_with(&[(A, "a"), (B, "b")]);
    backend.async_facts = vec![async_fact(A)];
    let mut closure = DemandClosure::new(Box::new(backend), None);
    let groups = [group(A, vec![async_demand(A)]), group(B, vec![])];

    let cold = analyze(&mut closure, &groups, &[]);
    assert_eq!(async_functions(&cold, A), 1);

    // Nothing derived from a generation survives into the next one, whether or
    // not the edit touched the file that asked.
    for edited in [B, A] {
        edit(&mut closure, edited);
        let table = analyze(&mut closure, &groups, &[]);
        assert_eq!(
            async_functions(&table, A),
            1,
            "an update must re-query the async facts (edited {edited})"
        );
    }

    // A demand change inside a generation does not reuse them either: the
    // ids of a table do not depend on which demands came first.
    let table = analyze(&mut closure, &groups, &[B]);
    assert_eq!(async_functions(&table, A), 1);
}

#[test]
fn async_facts_the_backend_reports_for_other_paths_are_kept_whole() {
    let mut backend = backend_with(&[(A, "a"), (B, "b")]);
    backend.async_facts = vec![async_fact(A), async_fact(B)];
    backend.async_ignore_locations = true;
    let mut closure = DemandClosure::new(Box::new(backend), None);
    let groups = [
        group(A, vec![async_demand(A)]),
        group(B, vec![async_demand(B)]),
    ];

    analyze(&mut closure, &groups, &[]);
    edit(&mut closure, A);
    let table = analyze(&mut closure, &groups, &[]);
    // The backend answered for B although only A was asked about; both are
    // kept.
    assert_eq!(async_functions(&table, A), 1);
    assert_eq!(async_functions(&table, B), 1);
}

#[test]
fn non_durable_contributions_are_never_retained() {
    let mut backend = backend_with(&[(A, "a")]);
    backend.durable = false;
    let calls = Arc::clone(&backend.semantic_calls);
    let mut closure = DemandClosure::new(Box::new(backend), None);
    let groups = [group(
        A,
        vec![EntityDemand {
            location: location(A, 1, 2),
            symbol: true,
            ..EntityDemand::default()
        }],
    )];

    analyze(&mut closure, &groups, &[]);
    assert_eq!(closure.stats().retention.non_durable_files, 1);
    assert!(closure.retained().get(A).is_none());

    edit(&mut closure, "/p/unrelated.ts");
    analyze(&mut closure, &groups, &[]);
    let retention = closure.stats().retention;
    assert_eq!(
        (retention.retained_files, retention.recomputed_files),
        (0, 1)
    );
    assert_eq!(
        *calls.lock().unwrap(),
        2,
        "the file recomputes every generation"
    );
}

#[test]
fn an_edit_recomputes_every_file_and_the_manifest_names_them() {
    let backend = backend_with(&[(A, "a"), (B, "b"), ("/p/c.ts", "c")]);
    let mut closure = DemandClosure::new(Box::new(backend), None);
    let demand = |path: &str| {
        group(
            path,
            vec![EntityDemand {
                location: location(path, 1, 2),
                symbol: true,
                ..EntityDemand::default()
            }],
        )
    };
    let groups = [demand(A), demand(B), demand("/p/c.ts")];
    analyze(&mut closure, &groups, &[]);

    edit(&mut closure, A);
    let table = analyze(&mut closure, &groups, &[]);
    let retention = closure.stats().retention;
    assert_eq!(
        (retention.retained_files, retention.recomputed_files),
        (0, 3),
        "an update retains nothing: every demanded file recomputes"
    );
    let manifest = table.transport.expect("an exact manifest");
    assert!(manifest.exact);
    let paths: BTreeSet<&str> = manifest.entity_paths.iter().map(String::as_str).collect();
    assert_eq!(paths, BTreeSet::from([A, B, "/p/c.ts"]));
}

/// The closure leaves symbol closure to the client: its table carries entity
/// rows for the demanded paths and no symbol rows.
#[test]
fn sparse_materialization_leaves_symbol_closure_to_client() {
    let mut closure = DemandClosure::new(
        Box::new(backend_with(&[("a.ts", "const value = 1;")])),
        None,
    );
    let table = closure
        .demand_table_for_groups(
            &Context::new(),
            1,
            &[DemandInputGroup {
                path: "a.ts".to_owned(),
                demands: vec![EntityDemand {
                    location: location("a.ts", 6, 11),
                    symbol: true,
                    ..EntityDemand::default()
                }],
            }],
            &[],
        )
        .unwrap();
    assert!(
        table.symbols.is_empty(),
        "producer retained symbol closure rows"
    );
    let typefacts::producer::EntityRows::Runs(runs) = &table.entities else {
        panic!("entity rows are not per-file runs");
    };
    assert!(
        runs.len() == 1 && runs[0].len() == 1,
        "sparse entity runs, want one path row"
    );
}
