//! Ports `session_demands_test.go`.

use super::{demand_at, location};
use crate::demands::RetainedDemandStore;
use typefacts::v3::{CompactDemandGroup, CompactDemands, EntityDemand, compact_demands};

#[test]
fn transaction_rolls_back_replace_insert_and_remove() {
    let mut store = RetainedDemandStore::default();
    store
        .begin(&[demand_at("a.ts", 1), demand_at("c.ts", 1)], &[], true)
        .commit();

    let mut transaction = store.begin(
        &[demand_at("a.ts", 2), demand_at("b.ts", 1)],
        &["c.ts".to_owned()],
        false,
    );
    assert!(
        transaction.groups().len() == 2
            && transaction.groups()[0].path == "a.ts"
            && transaction.groups()[1].path == "b.ts",
        "candidate groups = {:?}",
        transaction.groups()
    );
    transaction.rollback();
    drop(transaction);

    assert!(
        store.groups.len() == 2
            && store.groups[0].path == "a.ts"
            && store.groups[0].demands.as_ref().unwrap()[0]
                .location
                .start_byte
                == 1
            && store.groups[1].path == "c.ts",
        "rolled-back groups = {:?}",
        store.groups
    );
}

#[test]
fn an_uncommitted_transaction_rolls_back_when_dropped() {
    let mut store = RetainedDemandStore::default();
    store.begin(&[demand_at("a.ts", 1)], &[], true).commit();
    drop(store.begin(&[demand_at("a.ts", 2), demand_at("b.ts", 1)], &[], true));
    assert_eq!(store.groups.len(), 1);
    assert_eq!(store.groups[0].path, "a.ts");
    assert_eq!(
        store.groups[0].demands.as_ref().unwrap()[0]
            .location
            .start_byte,
        1
    );
}

#[test]
fn transaction_canonicalizes_only_unordered_changes() {
    let mut store = RetainedDemandStore::default();
    store
        .begin(
            &[
                demand_at("b.ts", 1),
                demand_at("a.ts", 2),
                demand_at("a.ts", 1),
            ],
            &[],
            true,
        )
        .commit();

    assert!(
        store.groups.len() == 2 && store.groups[0].path == "a.ts" && store.groups[1].path == "b.ts",
        "canonical groups = {:?}",
        store.groups
    );
    let first = store.groups[0].demands.as_ref().unwrap();
    assert!(
        first.len() == 2 && first[0].location.start_byte == 1 && first[1].location.start_byte == 2,
        "canonical a.ts run = {first:?}"
    );
}

const CLEAN: &str = "project/source.ts";
const DIRTY: &str = "project/./nested/../source.ts";

#[test]
fn transaction_cleans_paths_without_mutating_request() {
    let query = location(DIRTY, 2, 3);
    let request = vec![EntityDemand {
        location: location(DIRTY, 1, 2),
        query_location: Some(query),
        symbol: true,
        ..EntityDemand::default()
    }];

    let mut store = RetainedDemandStore::default();
    store.begin(&request, &[], true).commit();

    let run = store.groups[0].demands.as_ref().unwrap();
    assert!(
        store.groups.len() == 1
            && store.groups[0].path == CLEAN
            && *run[0].location.path == *CLEAN
            && run[0]
                .query_location
                .as_ref()
                .is_some_and(|q| *q.path == *CLEAN),
        "canonical groups = {:?}",
        store.groups
    );
    assert!(
        *request[0].location.path == *DIRTY
            && request[0]
                .query_location
                .as_ref()
                .is_some_and(|q| *q.path == *DIRTY),
        "request was mutated: {request:?}"
    );
}

#[test]
fn store_owns_compact_runs_without_expanded_rows() {
    let compact = compact_demands(&[
        demand_at("a.ts", 1),
        demand_at("a.ts", 2),
        demand_at("b.ts", 3),
    ]);
    let mut store = RetainedDemandStore::default();
    store.begin_compact(&compact, &[], true).unwrap().commit();

    assert_eq!(store.groups.len(), 2, "retained groups");
    for group in &store.groups {
        assert!(
            group.is_compact() && group.demands.is_none(),
            "group {:?} retained expanded demands",
            group.path
        );
    }
    assert_eq!(store.at("a.ts").map(|demands| demands.len()), Some(2));
}

#[test]
fn store_canonicalizes_expanded_compact_paths() {
    let query = location(DIRTY, 2, 3);
    let compact = compact_demands(&[EntityDemand {
        location: location(DIRTY, 1, 2),
        query_location: Some(query),
        symbol: true,
        r#async: true,
        ..EntityDemand::default()
    }]);

    let mut store = RetainedDemandStore::default();
    store.begin_compact(&compact, &[], true).unwrap().commit();

    let demands = store.at(CLEAN).expect("expanded demands");
    assert!(
        demands.len() == 1
            && *demands[0].location.path == *CLEAN
            && demands[0]
                .query_location
                .as_ref()
                .is_some_and(|q| *q.path == *CLEAN),
        "expanded compact demands = {demands:?}"
    );
    assert_eq!(compact.strings[1], DIRTY, "compact request was mutated");
    let mut asynchronous = Vec::new();
    store.groups[0]
        .append_async_demands(&mut asynchronous)
        .unwrap();
    assert!(
        asynchronous.len() == 1
            && *asynchronous[0].location.path == *CLEAN
            && asynchronous[0]
                .query_location
                .as_ref()
                .is_some_and(|q| *q.path == *CLEAN),
        "expanded compact async demands = {asynchronous:?}"
    );
}

#[test]
fn malformed_compact_demand_does_not_mutate_retained_state() {
    let mut store = RetainedDemandStore::default();
    store.begin(&[demand_at("a.ts", 1)], &[], true).commit();

    let rejected = store.begin_compact(
        &CompactDemands {
            strings: vec![String::new(), "b.ts".to_owned()],
            groups: vec![CompactDemandGroup(1, vec![0x80])],
        },
        &[],
        false,
    );
    assert!(rejected.is_err(), "malformed compact demand was accepted");
    drop(rejected);
    assert!(
        store.groups.len() == 1
            && store.groups[0].path == "a.ts"
            && store.groups[0].demands.as_ref().unwrap().len() == 1,
        "retained state mutated after rejected compact demand: {:?}",
        store.groups
    );
}

#[test]
fn compact_groups_must_name_each_path_once() {
    let mut compact = compact_demands(&[demand_at("a.ts", 1), demand_at("a.ts", 2)]);
    // A second group for the same path (an out-of-order start starts one).
    let duplicate = compact.groups[0].clone();
    compact.groups.push(duplicate);
    let mut store = RetainedDemandStore::default();
    let error = store.begin_compact(&compact, &[], true).unwrap_err();
    assert_eq!(
        error,
        "compact demand groups name \"a.ts\" twice; each path may appear once"
    );
}
