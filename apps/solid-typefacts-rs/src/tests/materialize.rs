//! Closure behaviour the Go package pins only through compiler-backed
//! integration tests (`suppression_delta_integration_test.go`,
//! `non_durable_integration_test.go`, `async_function_integration_test.go`,
//! `semantic_retained_integration_test.go`), reproduced over the fake so the
//! retained-materialization branches are exercised without a compiler.

use super::{backend_with, location};
use crate::closure::{DemandClosure, DemandInputGroup};
use crate::context::Context;
use crate::fake::ScopeSeen;
use crate::model::SymbolId;
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
fn async_facts_are_cached_per_file_across_unrelated_edits() {
    let mut backend = backend_with(&[(A, "a"), (B, "b")]);
    backend.async_facts = vec![async_fact(A)];
    let mut closure = DemandClosure::new(Box::new(backend), None);
    let groups = [group(A, vec![async_demand(A)]), group(B, vec![])];

    let cold = analyze(&mut closure, &groups, &[]);
    assert_eq!(async_functions(&cold, A), 1);
    let retention = closure.stats().retention;
    assert_eq!(
        (
            retention.retained_async_files,
            retention.recomputed_async_files
        ),
        (0, 1)
    );

    edit(&mut closure, B);
    let after_unrelated = analyze(&mut closure, &groups, &[]);
    assert_eq!(async_functions(&after_unrelated, A), 1);
    let retention = closure.stats().retention;
    assert_eq!(
        (
            retention.retained_async_files,
            retention.recomputed_async_files
        ),
        (1, 0),
        "an unrelated edit must not re-query the async facts"
    );

    edit(&mut closure, A);
    let after_edit = analyze(&mut closure, &groups, &[]);
    assert_eq!(async_functions(&after_edit, A), 1);
    let retention = closure.stats().retention;
    assert_eq!(
        (
            retention.retained_async_files,
            retention.recomputed_async_files
        ),
        (0, 1),
        "an edit to the file re-queries its async facts"
    );
}

#[test]
fn cross_path_async_facts_fall_back_to_a_full_batch_and_are_not_cached() {
    let mut backend = backend_with(&[(A, "a"), (B, "b")]);
    backend.async_facts = vec![async_fact(A), async_fact(B)];
    backend.async_ignore_locations = true;
    let mut closure = DemandClosure::new(Box::new(backend), None);
    let groups = [
        group(A, vec![async_demand(A)]),
        group(B, vec![async_demand(B)]),
    ];

    analyze(&mut closure, &groups, &[]);
    assert!(closure.async_files.is_some(), "a clean batch is cacheable");

    edit(&mut closure, A);
    let table = analyze(&mut closure, &groups, &[]);
    // The backend answered for B although only A was asked about: the whole
    // batch is redone and nothing is cached from it.
    let retention = closure.stats().retention;
    assert_eq!(
        (
            retention.retained_async_files,
            retention.recomputed_async_files
        ),
        (0, 2)
    );
    assert!(closure.async_files.is_none());
    assert_eq!(async_functions(&table, A), 1);
    assert_eq!(async_functions(&table, B), 1);
}

fn scopes(seen: &Arc<std::sync::Mutex<Vec<ScopeSeen>>>) -> Vec<ScopeSeen> {
    seen.lock().unwrap().clone()
}

#[test]
fn a_changed_suppression_union_refreshes_only_the_descriptor_users() {
    let mut backend = backend_with(&[(A, "a"), (B, "b")]);
    let shared: SymbolId = "symbol:h:shared".into();
    backend
        .symbol_overrides
        .insert(location(A, 1, 2), shared.clone());
    backend
        .symbol_overrides
        .insert(location(B, 1, 2), shared.clone());
    let seen = Arc::clone(&backend.scopes_seen);
    let mut closure = DemandClosure::new(Box::new(backend), None);

    let accessor = EntityDemand {
        location: location(A, 1, 2),
        symbol: true,
        structural_accessor: true,
        ..EntityDemand::default()
    };
    let user = EntityDemand {
        location: location(B, 1, 2),
        symbol: true,
        type_descriptor: true,
        ..EntityDemand::default()
    };
    analyze(
        &mut closure,
        &[group(A, vec![accessor]), group(B, vec![user.clone()])],
        &[],
    );
    assert_eq!(
        closure.last_suppression,
        Some([shared.clone()].into_iter().collect())
    );
    assert_eq!(scopes(&seen).len(), 1);

    // A stops asking for the structural accessor: the union loses the shared
    // symbol, so B, whose descriptor demand touches it, is refreshed under the
    // new union while A is recomputed by its own demand change.
    let plain = EntityDemand {
        location: location(A, 1, 2),
        symbol: true,
        ..EntityDemand::default()
    };
    analyze(
        &mut closure,
        &[group(A, vec![plain]), group(B, vec![user])],
        &[A],
    );
    let retention = closure.stats().retention;
    assert!(retention.suppression_recompute);
    assert_eq!(
        (retention.retained_files, retention.recomputed_files),
        (0, 2)
    );
    let calls = scopes(&seen);
    assert_eq!(
        calls.len(),
        3,
        "cold, A's recompute, B's refresh: {calls:?}"
    );
    assert_eq!(calls[1].paths, vec![A.to_owned()]);
    assert_eq!(
        calls[2],
        ScopeSeen {
            paths: vec![B.to_owned()],
            suppression: BTreeSet::new(),
            descriptor_seed: BTreeSet::new(),
        },
        "the refresh runs under the exact current union and no seed"
    );
    assert_eq!(closure.last_suppression, Some(Default::default()));

    // An unchanged union refreshes nothing.
    let unchanged = [
        group(
            A,
            vec![EntityDemand {
                location: location(A, 1, 2),
                symbol: true,
                ..EntityDemand::default()
            }],
        ),
        group(
            B,
            vec![EntityDemand {
                location: location(B, 1, 2),
                symbol: true,
                type_descriptor: true,
                ..EntityDemand::default()
            }],
        ),
    ];
    edit(&mut closure, "/p/unrelated.ts");
    analyze(&mut closure, &unchanged, &[]);
    assert!(!closure.stats().retention.suppression_recompute);
    assert_eq!(closure.stats().retention.retained_files, 2);
    assert_eq!(scopes(&seen).len(), 3);
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
fn an_edit_invalidates_direct_dependents_and_the_manifest_names_them() {
    let mut backend = backend_with(&[(A, "a"), (B, "b"), ("/p/c.ts", "c")]);
    backend
        .dependencies
        .insert(B.to_owned(), vec![A.to_owned()]);
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
        (1, 2),
        "the edited file and its direct dependent recompute; c.ts is retained"
    );
    let manifest = table.transport.expect("an exact manifest");
    assert!(manifest.exact);
    let paths: BTreeSet<&str> = manifest.entity_paths.iter().map(String::as_str).collect();
    assert_eq!(paths, BTreeSet::from([A, B]));
}
