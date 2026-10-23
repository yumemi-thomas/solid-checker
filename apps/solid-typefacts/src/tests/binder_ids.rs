//! Binder-level ids inside the producer (ADR 0366): the merge map, rehydration
//! by key, and the symbols that must keep counters. Order, history and
//! file-order independence run end to end in `tests/binder_ids.rs`.

use super::project_support::{SMALL_CONFIG, Scratch, find_last};
use crate::backend::ClosureBackend;
use crate::context::Context;
use crate::model::durable_symbol_id;
use tsrs_ast::SymbolFlags;

const SOURCE: &str = "export interface Box<T> { value: T }\n\
export const box: Box<number> = { value: 1 }\n\
export const read = box.value\n\
export const json = JSON.stringify(read)\n";

/// A pre-merge binder symbol (what `declaration.symbol()` holds) and its merged
/// symbol are one logical symbol: without the merge map `JSON`, `Set` and
/// `Array` would split in two ids, one per lib declaration.
#[test]
fn pre_merge_binder_symbols_share_the_merged_symbols_id() {
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", SOURCE)]);
    for reverse in [false, true] {
        let mut project = scratch.open();
        project
            .with_env(|env, _| {
                let mut merged = 0;
                for name in ["JSON", "Set", "Array", "Math", "Promise", "Object"] {
                    let global = env.checker.globals.lookup(name).expect(name);
                    let mut binders: Vec<_> = global
                        .declarations()
                        .iter()
                        .filter_map(|declaration| declaration.symbol())
                        .collect();
                    if reverse {
                        binders.reverse();
                    }
                    let mut ids = Vec::new();
                    for binder in &binders {
                        if binder.addr() != global.addr() {
                            merged += 1;
                        }
                        ids.push(env.id_for(*binder));
                    }
                    ids.push(env.id_for(global));
                    assert!(
                        ids.iter().all(|id| id.starts_with("symbol:h:")),
                        "{name}: {ids:?}"
                    );
                    assert!(
                        ids.windows(2).all(|pair| pair[0] == pair[1]),
                        "{name} split into {ids:?}"
                    );
                }
                assert!(
                    merged > 0,
                    "no lib global was merged: the case proves nothing"
                );
            })
            .unwrap();
    }
}

/// An id minted before `ReleaseAnalysisState` rehydrates from its key alone,
/// for every key kind, and a re-minted symbol gets the same id.
#[test]
fn every_key_kind_rehydrates_after_a_release() {
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", SOURCE)]);
    let path = scratch.path("a.ts");
    let mut project = scratch.open();
    let ctx = Context::new();
    let exported = project
        .symbol_at(&ctx, &find_last(&path, SOURCE, "Box"))
        .unwrap();
    let local = project
        .symbol_at(&ctx, &find_last(&path, SOURCE, "read"))
        .unwrap();
    let global = project
        .symbol_at(&ctx, &find_last(&path, SOURCE, "JSON"))
        .unwrap();
    let mut intrinsic = None;
    project
        .with_env(|env, _| {
            let id = env.id_for(env.checker.undefined_symbol);
            assert!(durable_symbol_id(&id) && !id.is_empty(), "{id}");
            intrinsic = Some(id);
        })
        .unwrap();
    let intrinsic = intrinsic.unwrap();
    let ids = [exported, local, global, intrinsic];
    assert!(ids.iter().all(|id| id.starts_with("symbol:h:")), "{ids:?}");
    project.release_analysis_state_for_test();
    let mut project_ids = Vec::new();
    project
        .with_env(|env, _| {
            for id in &ids {
                let symbol = env.symbol_for(id).unwrap_or_else(|| panic!("{id} lost"));
                assert_eq!(&env.id_for(symbol), id, "re-minted");
                project_ids.push(symbol.flags().intersects(SymbolFlags::Transient));
            }
        })
        .unwrap();
    // Declarations still answer for the declared ones.
    for id in &ids[..3] {
        let declarations = project.declarations(&ctx, id).unwrap().unwrap();
        assert!(!declarations.is_empty(), "{id}");
    }
}

/// An id no key names fails closed.
#[test]
fn an_unknown_hash_id_resolves_to_nothing() {
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", SOURCE)]);
    let mut project = scratch.open();
    project
        .with_env(|env, _| {
            let unknown: crate::model::SymbolId = "symbol:h:000000000000000000000000".into();
            assert!(env.symbol_for(&unknown).is_none());
        })
        .unwrap();
}
