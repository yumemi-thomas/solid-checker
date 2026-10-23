//! Binder-level ids inside the producer (ADR 0366): the merge map, rehydration
//! by key, and the symbols that must keep counters. Order, history and
//! file-order independence run end to end in `tests/binder_ids.rs`.

use super::project_support::{SMALL_CONFIG, Scratch, find, find_last};
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

/// One node declares two symbols: `constructor(public x)` is the parameter `x`
/// (the constructor's locals) and the property `x` (the class members), and
/// both share the declaration a `Decl` key hashes. TypeScript keeps them
/// apart (find-all-references lists both), so the ids must too: the property,
/// which owns the node, keeps the binder id and the parameter takes a counter.
/// A module export and its local half (`localSymbol`) are one symbol and keep
/// sharing an id.
#[test]
fn a_parameter_property_and_its_parameter_never_share_an_id() {
    let source = "export class C {\n  constructor(public x: number) { x }\n  read() { return this.x }\n}\nexport const v = 1\nv\n";
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", source)]);
    let path = scratch.path("a.ts");
    for parameter_first in [true, false] {
        let mut project = scratch.open();
        let ctx = Context::new();
        let declaration = find(&path, source, "x: number");
        let parameter_use = find(&path, source, "x }");
        let property_use = find_last(&path, source, "x }");
        let (parameter, property) = if parameter_first {
            let parameter = project.symbol_at(&ctx, &parameter_use).unwrap();
            (parameter, project.symbol_at(&ctx, &declaration).unwrap())
        } else {
            let property = project.symbol_at(&ctx, &declaration).unwrap();
            (project.symbol_at(&ctx, &parameter_use).unwrap(), property)
        };
        assert_ne!(parameter, property, "parameter and property conflated");
        assert!(property.starts_with("symbol:h:"), "{property}");
        assert!(!parameter.starts_with("symbol:h:"), "{parameter}");
        assert_eq!(
            project.symbol_at(&ctx, &property_use).unwrap(),
            property,
            "this.x"
        );
        project
            .with_env(|env, _| {
                let file = env.program.get_source_file(&path).unwrap();
                let at = u32::try_from(source.rfind("v\n").unwrap()).unwrap();
                let export = env
                    .checker
                    .get_symbol_at_location_exported(crate::nodes::node_at(file, at))
                    .unwrap();
                let local = export.declarations()[0].local_symbol().unwrap();
                assert_ne!(local.addr(), export.addr());
                assert_eq!(env.id_for(local), env.id_for(export));
            })
            .unwrap();
    }
}

/// An `Export` id hashes only (path, name). A module that exports its own
/// `Box` and augments another module's `Box` holds the minimum declaration of
/// both (`a.ts` sorts before `lib.ts`), so both export keys spell one id: the
/// collision check must hand the second one asked a counter, never the first
/// one's id.
#[test]
fn an_augmentation_beside_a_same_named_export_never_shares_its_id() {
    let lib = "export interface Box<T> { value: T }\n";
    let aug = "import \"./lib\"\nexport interface Box { own: number }\ndeclare module \"./lib\" {\n  interface Box<T> { extra: T }\n}\n";
    let main = "import type { Box as LibBox } from \"./lib\"\nimport type { Box as OwnBox } from \"./a\"\nexport type L = LibBox<number>\nexport type O = OwnBox\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", aug),
        ("lib.ts", lib),
        ("main.ts", main),
    ]);
    let (a, l) = (scratch.path("a.ts"), scratch.path("lib.ts"));
    for lib_first in [true, false] {
        let mut project = scratch.open();
        let ctx = Context::new();
        let own = find(&a, aug, "Box {");
        let augmented = find(&l, lib, "Box<T>");
        let (own_id, lib_id) = if lib_first {
            let lib_id = project.symbol_at(&ctx, &augmented).unwrap();
            (project.symbol_at(&ctx, &own).unwrap(), lib_id)
        } else {
            let own_id = project.symbol_at(&ctx, &own).unwrap();
            (own_id, project.symbol_at(&ctx, &augmented).unwrap())
        };
        assert_ne!(own_id, lib_id, "lib first {lib_first}");
        // The collision is real (the case proves something): whichever is
        // asked second fails closed to a counter. Which one that is depends on
        // demand order, a known gap in order independence for this corner.
        assert_eq!(
            [&own_id, &lib_id]
                .iter()
                .filter(|id| id.starts_with("symbol:h:"))
                .count(),
            1,
            "lib first {lib_first}: {own_id} {lib_id}"
        );
        let declarations = project.declarations(&ctx, &lib_id).unwrap().unwrap();
        assert_eq!(
            declarations.len(),
            2,
            "lib first {lib_first}: {declarations:?}"
        );
    }
}
