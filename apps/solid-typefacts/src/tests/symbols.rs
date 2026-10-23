//! Ports `symbol_store_test.go`, `symbol_intern_test.go` and the live test of
//! `symbol_incremental_adversarial_test.go`.

use super::{backend_with, location};
use crate::closure::{DemandClosure, DemandInputGroup};
use crate::context::Context;
use crate::model::{SymbolId, blank_symbol};
use crate::symbols::intern::{SymbolHandleSet, SymbolInterner};
use crate::symbols::store::SymbolFactStore;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use typefacts::v3::EntityDemand;
use typefacts::{Declaration, SymbolFact};

fn fact(id: &str) -> SymbolFact {
    blank_symbol(id.into())
}

fn handle_set(interner: &crate::symbols::intern::SharedInterner, ids: &[&str]) -> SymbolHandleSet {
    let mut set = SymbolHandleSet::new(interner, Vec::new());
    for id in ids {
        set.add_id(&SymbolId::from(*id));
    }
    set
}

#[test]
fn store_shares_untouched_chunks_across_patch() {
    let facts: Vec<SymbolFact> = (0..600)
        .map(|index| fact(&format!("symbol-{index:04}")))
        .collect();
    let previous = SymbolFactStore::new(facts.clone());
    let interner = SymbolInterner::new_shared();
    let mut present = SymbolHandleSet::new(&interner, Vec::new());
    for fact in &facts {
        present.add_id(&fact.id);
    }
    let mut changed = facts[300].clone();
    changed.alias_target = "updated".into();

    let (next, shared, removed, complete) = previous.patch(
        &present,
        &HashMap::from([(changed.id.clone(), changed.clone())]),
        &HashSet::new(),
    );

    assert!(
        complete,
        "copy-on-write patch did not preserve the complete symbol universe"
    );
    assert!(
        removed.is_empty(),
        "removed symbols = {removed:?}, want none"
    );
    assert_eq!(shared, 2, "shared chunks");
    assert!(
        Arc::ptr_eq(&previous.chunks[0], &next.chunks[0]),
        "untouched leading chunk was copied"
    );
    assert!(
        Arc::ptr_eq(&previous.chunks[2], &next.chunks[2]),
        "untouched trailing chunk was copied"
    );
    let mut want = facts;
    want[300] = changed;
    assert_eq!(
        next.facts(),
        want,
        "copy-on-write store differs from canonical patched rows"
    );
}

#[test]
fn store_rejects_incomplete_removal_candidates() {
    let previous = SymbolFactStore::new(vec![fact("a"), fact("b")]);
    let interner = SymbolInterner::new_shared();
    let present = handle_set(&interner, &["a"]);

    let (_, _, _, complete) = previous.patch(&present, &HashMap::new(), &HashSet::new());

    assert!(
        !complete,
        "patch accepted an incomplete removal-candidate set"
    );
}

/// The interner is the one symbol structure with no eviction of its own: a
/// session minting fresh non-durable ids every generation grows it without
/// bound unless the reset policy replaces it once dead identities outnumber
/// the live universe.
#[test]
fn maybe_reset_interner_bounds_dead_identities() {
    let mut closure = DemandClosure::new(Box::new(backend_with(&[])), None);
    closure.maybe_reset_interner(10);
    let healthy = Rc::clone(closure.interner.as_ref().expect("no interner was created"));
    for index in 0..100 {
        healthy
            .borrow_mut()
            .handle(&SymbolId::from(format!("symbol:h:{index:024}")));
    }
    closure.maybe_reset_interner(10);
    assert!(
        Rc::ptr_eq(&healthy, closure.interner.as_ref().unwrap()),
        "an interner within bounds was reset"
    );
    for generation in 0..20 {
        for index in 0..100 {
            healthy
                .borrow_mut()
                .handle(&SymbolId::from(format!("symbol:{generation}:{index}")));
        }
    }
    closure.maybe_reset_interner(10);
    assert!(
        !Rc::ptr_eq(&healthy, closure.interner.as_ref().unwrap()),
        "an interner holding {} identities for a 10-symbol universe survived",
        healthy.borrow().size()
    );
    assert_eq!(
        closure.interner.as_ref().unwrap().borrow().size(),
        0,
        "the replacement interner is not empty"
    );
}

#[test]
fn symbol_handle_set_grows_geometrically_during_cold_interning() {
    let interner = SymbolInterner::new_shared();
    let mut set = SymbolHandleSet::new(&interner, Vec::new());
    let mut previous_capacity = set.members.len();
    let mut growths = 0;
    for index in 0..50_000 {
        set.add_id(&SymbolId::from(format!("symbol:h:{index:024}")));
        let capacity = set.members.len();
        if capacity != previous_capacity {
            growths += 1;
            assert!(
                previous_capacity == 0 || capacity >= previous_capacity * 2,
                "growth {previous_capacity} -> {capacity} is not geometric"
            );
            previous_capacity = capacity;
        }
    }
    assert!(
        growths <= 11,
        "50,000 cold symbols caused {growths} backing allocations, want at most 11"
    );
    assert_eq!(set.len(), 50_000);
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

#[test]
fn declaration_index_tracks_facts_by_path() {
    // `indexSymbolFact` / `unindexSymbolFact` keep Update's eviction lookup in
    // sync with the canonical store.
    let mut closure = DemandClosure::new(Box::new(backend_with(&[])), None);
    let mut symbol = fact("symbol:h:a");
    symbol.declarations = Arc::from(vec![
        Declaration {
            name: "a".into(),
            kind: "variable".into(),
            location: location("/p/a.ts", 0, 1),
        },
        Declaration {
            name: "a".into(),
            kind: "variable".into(),
            location: location("/p/b.ts", 0, 1),
        },
    ]);
    closure.index_symbol_fact(&symbol);
    closure.index_symbol_fact(&symbol);
    assert_eq!(closure.symbols_by_path["/p/a.ts"], vec![symbol.id.clone()]);
    assert_eq!(closure.symbols_by_path["/p/b.ts"], vec![symbol.id.clone()]);
    closure.unindex_symbol_fact(&symbol);
    assert!(closure.symbols_by_path.is_empty());
}
