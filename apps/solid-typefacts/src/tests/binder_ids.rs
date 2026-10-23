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

/// Multi-origin properties (ADR 0371): a union, an intersection and a spread
/// property over two declared properties each, and a union whose origins
/// include a late-bound member (no binder key).
const SET_SOURCE: &str = "interface A { kind: \"a\"; tag?: string }\n\
interface B { kind: \"b\"; tag?: string }\n\
declare const u: A | B\n\
export const k = u.kind\n\
declare const i: A & { kind: \"a\" }\n\
export const ik = i.kind\n\
declare const x: { p: number }\n\
declare const y: { p?: number }\n\
export const sp = ({ ...x, ...y }).p\n\
const c = \"c\"\n\
export interface CX { [c]: number }\n\
export interface D { c: number }\n\
declare const cd: CX | D\n\
export const cc = cd.c\n";

fn set_scratch() -> (Scratch, String) {
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", SET_SOURCE)]);
    let path = scratch.path("a.ts");
    (scratch, path)
}

/// The symbol the checker answers at the start of `needle` + `offset`.
fn symbol_in(
    env: &mut crate::identity::Env<'_>,
    path: &str,
    needle: &str,
    offset: usize,
) -> tsrs_core::P<tsrs_ast::Symbol> {
    let file = env.program.get_source_file(path).unwrap();
    let at = u32::try_from(SET_SOURCE.find(needle).unwrap() + offset).unwrap();
    env.checker
        .get_symbol_at_location_exported(crate::nodes::node_at(file, at))
        .unwrap_or_else(|| panic!("{needle}"))
}

/// Each construct is recognised as what it is, with one origin per declared
/// property; one late-bound origin (whose node holds the binder's
/// `__computed` placeholder) leaves the property unkeyed.
#[test]
fn multi_origin_properties_have_set_keys_of_their_combination() {
    use crate::binder_key::{Combination, Key, set_key};
    let (scratch, path) = set_scratch();
    let mut project = scratch.open();
    project
        .with_env(|env, _| {
            for (needle, offset, want) in [
                ("u.kind", 2, Combination::Union),
                ("i.kind", 2, Combination::Intersection),
                ("}).p", 3, Combination::Spread),
            ] {
                let symbol = symbol_in(env, &path, needle, offset);
                match set_key(env.checker, symbol, |_| false) {
                    Some(Key::Set {
                        combination,
                        origins,
                        ..
                    }) => {
                        assert_eq!(combination, want, "{needle}");
                        assert_eq!(origins.len(), 2, "{needle}: {origins:?}");
                        assert!(origins.iter().all(Key::is_binder), "{needle}");
                    }
                    other => panic!("{needle}: {other:?}"),
                }
                let id = env.id_for(symbol);
                assert!(id.starts_with("symbol:h:"), "{needle}: {id}");
                assert!(env.ids.is_set_id(&id), "{needle}");
            }
            let mixed = symbol_in(env, &path, "cd.c", 3);
            assert_eq!(mixed.declarations().len(), 2, "the case proves nothing");
            assert!(set_key(env.checker, mixed, |_| false).is_none());
            let id = env.id_for(mixed);
            assert!(!id.starts_with("symbol:h:"), "{id}");
            // A declared property keeps its binder key.
            let declared = symbol_in(env, &path, "kind", 0);
            assert!(set_key(env.checker, declared, |_| false).is_none());
        })
        .unwrap();
}

/// The checker lists a multi-origin property's declarations in constituent
/// order: a union's by type name (type ids as the last tie-break, history),
/// an intersection's as written. `P | Q` lists `v` as P's then Q's; `AQ | P`
/// (`AQ` inherits Q's `v` and sorts first) as Q's then P's: one union
/// property, one id and one declarations row, whichever is minted first. An
/// intersection resolves overloaded members in its written order, so
/// `P & Q` and `Q & P` keep two ids.
#[test]
fn set_ids_do_not_depend_on_constituent_order() {
    let source = "interface P { v: number }\n\
interface Q { v: number }\n\
interface AQ extends Q { w: number }\n\
declare const pq: P | Q\n\
declare const qp: AQ | P\n\
declare const ipq: P & Q\n\
declare const iqp: Q & P\n\
export const a = pq.v\n\
export const b = qp.v\n\
export const c = ipq.v\n\
export const d = iqp.v\n";
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", source)]);
    let path = scratch.path("a.ts");
    let ctx = Context::new();
    let at = |needle: &str| {
        let mut at = find(&path, source, needle);
        at.start_byte += u64::try_from(needle.len()).unwrap() - 1;
        at.end_byte = at.start_byte + 1;
        at
    };
    let reads = [at("pq.v"), at("qp.v")];
    let mut seen = Vec::new();
    for first in [0, 1] {
        let mut project = scratch.open();
        let ids = [first, 1 - first].map(|n| project.symbol_at(&ctx, &reads[n]).unwrap());
        assert_eq!(ids[0], ids[1], "first {first}");
        assert!(ids[0].starts_with("symbol:h:"), "{}", ids[0]);
        let mut raw = Vec::new();
        project
            .with_env(|env, _| {
                let file = env.program.get_source_file(&path).unwrap();
                for read in &reads {
                    let node = crate::nodes::node_at(file, u32::try_from(read.start_byte).unwrap());
                    let symbol = env.checker.get_symbol_at_location_exported(node).unwrap();
                    let order: Vec<u32> = symbol
                        .declarations()
                        .iter()
                        .map(|node| node.pos().source_offset().unwrap())
                        .collect();
                    raw.push(order);
                }
            })
            .unwrap();
        assert_ne!(
            raw[0], raw[1],
            "one constituent order: the case proves nothing"
        );
        let row = project.declarations(&ctx, &ids[0]).unwrap().unwrap();
        let starts: Vec<u64> = row.iter().map(|d| d.location.start_byte).collect();
        assert!(
            starts.windows(2).all(|pair| pair[0] < pair[1]),
            "{starts:?}"
        );
        seen.push((ids[0].clone(), row));
        let pq = project.symbol_at(&ctx, &at("ipq.v")).unwrap();
        let qp = project.symbol_at(&ctx, &at("iqp.v")).unwrap();
        assert!(pq.starts_with("symbol:h:") && qp.starts_with("symbol:h:"));
        assert_ne!(pq, qp, "intersections in two orders share an id");
        assert_ne!(pq, ids[0]);
    }
    assert_eq!(seen[0], seen[1]);
}

/// A key is only given when every property sharing it has one type: the
/// same origins under a generic container whose member reads the type
/// parameter, under a mapped constituent, or with a constituent that only
/// has an index signature, keep counters.
#[test]
fn a_set_key_needs_one_type_for_every_container() {
    let source = "interface Box<T> { v: T; k: \"box\" }\n\
interface Bag<T> { v: T; k: \"bag\" }\n\
declare const n: Box<number> | Bag<number>\n\
type Wrap<T> = { [K in keyof T]: () => T[K] }\n\
interface P { v: number }\n\
interface Q { v: number }\n\
declare const w: Wrap<P> | Wrap<Q>\n\
declare const r: P | { [key: string]: number }\n\
class C { v = this }\n\
class D { v = this }\n\
declare const cd: C | D\n\
export const a = n.v\n\
export const b = n.k\n\
export const c = w.v\n\
export const d = r.v\n\
export const e = cd.v\n";
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", source)]);
    let path = scratch.path("a.ts");
    let ctx = Context::new();
    let mut project = scratch.open();
    let mut id = |needle: &str| {
        let mut at = find(&path, source, needle);
        at.start_byte += u64::try_from(needle.len()).unwrap() - 1;
        at.end_byte = at.start_byte + 1;
        project.symbol_at(&ctx, &at).unwrap()
    };
    // `k` does not read `T`: one type under every instantiation.
    assert!(id("n.k").starts_with("symbol:h:"));
    for open in ["n.v", "w.v", "r.v", "cd.v"] {
        let got = id(open);
        assert!(!got.starts_with("symbol:h:"), "{open}: {got}");
    }
}

/// A `Set` id names a checker-made property, which no position re-resolves:
/// after a release it fails closed until the generation mints it again (to
/// the same id), and no retained binder-level answer may hold it.
#[test]
fn a_set_id_fails_closed_after_a_release_and_is_never_retained() {
    let (scratch, path) = set_scratch();
    let ctx = Context::new();
    let mut project = scratch.open();
    let read = find(&path, SET_SOURCE, "kind\n");
    let id = project.symbol_at(&ctx, &read).unwrap();
    project
        .with_env(|env, _| {
            assert!(env.ids.is_set_id(&id));
            assert!(
                env.ids.key_of_id(&id).is_none(),
                "a binder answer could hold it"
            );
        })
        .unwrap();
    project.release_analysis_state_for_test();
    project
        .with_env(|env, _| {
            assert!(
                env.symbol_for(&id).is_none(),
                "{id} resolved after a release"
            );
            let key = std::sync::Arc::new(crate::binder_key::Key::Set {
                combination: crate::binder_key::Combination::Union,
                name: "kind".into(),
                origins: Vec::new(),
            });
            assert!(!env.ids.register_key(&key.id().as_str().into(), &key));
        })
        .unwrap();
    assert_eq!(project.symbol_at(&ctx, &read).unwrap(), id);
}
