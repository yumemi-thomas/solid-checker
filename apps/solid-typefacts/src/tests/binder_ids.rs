//! Binder-level ids inside the producer (ADR 0366): the merge map, rehydration
//! by key, and the symbols that must not take a binder or `Set` key (they
//! take an exact derived key, or no id). Order, history and file-order
//! independence run end to end in `tests/binder_ids.rs`.

use super::project_support::{SMALL_CONFIG, Scratch, find, find_last};
use crate::backend::ClosureBackend;
use crate::binder_key::Key;
use crate::context::Context;
use crate::identity::Identities;
use crate::model::durable_symbol_id;
use crate::project::Project;
use tsrs_ast::SymbolFlags;
use typefacts::Location;

/// What `id` was minted from: `binder` (ADR 0366), `set` (ADR 0371),
/// `derived` (an exact derived key, `derived_key.rs`), or `none` (no exact
/// key: the empty id).
fn key_kind(ids: &Identities, id: &str) -> &'static str {
    if id.is_empty() {
        return "none";
    }
    match ids.key(&id.into()).map(|key| &**key) {
        Some(Key::Derived { .. }) => "derived",
        Some(Key::Set { .. }) => "set",
        Some(_) => "binder",
        None => panic!("{id} was minted from no key"),
    }
}

/// The id at `location`, the empty id when the symbol has no exact key.
fn id_at(project: &mut Project, location: &Location) -> String {
    match project.symbol_at(&Context::new(), location) {
        Ok(id) => id.to_string(),
        Err(error) if error.0.contains("has no exact id") => String::new(),
        Err(error) => panic!("{error:?}"),
    }
}

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
/// which owns the node, keeps the binder id and the parameter does not take
/// one.
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
        assert_eq!(key_kind(project.identities(), &property), "binder");
        assert_ne!(
            key_kind(project.identities(), &parameter),
            "binder",
            "{parameter}"
        );
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
/// collision check must never hand the second one asked the first one's id.
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
            let lib_id = id_at(&mut project, &augmented);
            (id_at(&mut project, &own), lib_id)
        } else {
            let own_id = id_at(&mut project, &own);
            (own_id, id_at(&mut project, &augmented))
        };
        assert_ne!(own_id, lib_id, "lib first {lib_first}");
        // The collision is real (the case proves something): whichever is
        // asked second does not take the binder id. Which one that is depends
        // on demand order, a known gap in order independence for this corner.
        assert_eq!(
            [&own_id, &lib_id]
                .iter()
                .filter(|id| key_kind(project.identities(), id) == "binder")
                .count(),
            1,
            "lib first {lib_first}: {own_id} {lib_id}"
        );
        // The binder id answers both of `lib.ts`'s merged declarations; a
        // derived id names a symbol of the pass that minted it only, so asked
        // later it fails closed.
        let declarations = project.declarations(&ctx, &lib_id).unwrap();
        if key_kind(project.identities(), &lib_id) == "binder" {
            let declarations = declarations.unwrap();
            assert_eq!(
                declarations.len(),
                2,
                "lib first {lib_first}: {declarations:?}"
            );
        } else {
            assert!(declarations.is_none(), "lib first {lib_first}");
        }
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
                match set_key(env.checker, symbol, |_| false, &env.ids.local_chains) {
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
            assert!(set_key(env.checker, mixed, |_| false, &env.ids.local_chains).is_none());
            let id = env.id_for(mixed);
            let kind = key_kind(env.ids, &id);
            assert!(kind == "derived" || kind == "none", "{id}: {kind}");
            // A declared property keeps its binder key.
            let declared = symbol_in(env, &path, "kind", 0);
            assert!(set_key(env.checker, declared, |_| false, &env.ids.local_chains).is_none());
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
        // A `Set` id names a property made inside a pass: its row is read in
        // a pass that mints it again.
        let mut row = Vec::new();
        project
            .with_env(|env, _| {
                let file = env.program.get_source_file(&path).unwrap();
                for n in [first, 1 - first] {
                    let at = u32::try_from(reads[n].start_byte).unwrap();
                    let node = crate::nodes::node_at(file, at);
                    let symbol = env.checker.get_symbol_at_location_exported(node).unwrap();
                    assert_eq!(env.id_for(symbol), ids[0]);
                }
                let id: crate::model::SymbolId = ids[0].clone();
                row = crate::per_file::row_for(env, &id).declarations.to_vec();
            })
            .unwrap();
        assert!(!row.is_empty());
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
/// has an index signature, take a derived key or no id.
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
        id_at(&mut project, &at)
    };
    // `k` does not read `T`: one type under every instantiation.
    let k = id("n.k");
    let opens: Vec<(&str, String)> = ["n.v", "w.v", "r.v", "cd.v"]
        .into_iter()
        .map(|open| (open, id(open)))
        .collect();
    let _ = &ctx;
    assert_eq!(key_kind(project.identities(), &k), "set");
    for (open, got) in opens {
        let kind = key_kind(project.identities(), &got);
        assert!(kind == "derived" || kind == "none", "{open}: {got} {kind}");
    }
}

/// PR #75 review: properties of different types never share a `Set` id.
/// Each pair below has the same origins; the second property's type differs
/// (inherited mapped members, a nested spread, an inferred polymorphic
/// `this`), so it takes a derived key or no id while the first keeps its
/// `Set` key.
#[test]
fn transformed_or_inferred_origins_never_share_a_set_id() {
    let source = "interface P { v: number }\n\
interface Q { v: string }\n\
type Wrap<T> = { [K in keyof T]: () => T[K] }\n\
interface WP extends Wrap<P> {}\n\
interface WQ extends Wrap<Q> {}\n\
declare const plain: P | Q\n\
declare const wrapped: WP | WQ\n\
declare const iplain: P & Q\n\
declare const iwrapped: WP & WQ\n\
interface S { s: number }\n\
interface T2 { s?: string }\n\
interface U { s: boolean }\n\
declare const p: S\n\
declare const q: T2\n\
const spread = { ...p, ...q }\n\
declare const three: S | T2 | U\n\
declare const nested: typeof spread | U\n\
class Base { m() { return this; } }\n\
class A extends Base { v = super.m(); n() { return super.m(); } }\n\
class B extends Base { v = super.m(); n() { return super.m(); } }\n\
class A1 extends A { extraA = 1; }\n\
class B1 extends B { extraB = 2; }\n\
declare const ab: A | B\n\
declare const ab1: A1 | B1\n\
class C { w: number = 1; k(x: number): number { return x; } }\n\
class D { w: number = 2; k(x: number): number { return x; } }\n\
class C1 extends C { extraC = 1; }\n\
class D1 extends D { extraD = 2; }\n\
declare const cd: C | D\n\
declare const cd1: C1 | D1\n\
class E { u = 1; j(x: number) { return x; } }\n\
class F { u = 2; j(x: number) { return x; } }\n\
declare const ef: E | F\n\
export const r = [plain.v, wrapped.v, iplain.v, iwrapped.v, three.s, nested.s, ab.v, ab1.v, ab.n, ab1.n, cd.w, cd1.w, cd.k, cd1.k, ef.u, ef.j]\n";
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", source)]);
    let path = scratch.path("a.ts");
    let ctx = Context::new();
    let mut project = scratch.open();
    let mut id = |needle: &str| {
        let mut at = find(&path, source, needle);
        at.start_byte += u64::try_from(needle.len()).unwrap() - 1;
        at.end_byte = at.start_byte + 1;
        id_at(&mut project, &at)
    };
    let keyed: Vec<(&str, String)> = [
        "plain.v", "iplain.v", "three.s", "cd.w", "cd.k", "cd1.w", "cd1.k",
    ]
    .into_iter()
    .map(|needle| (needle, id(needle)))
    .collect();
    let open: Vec<(&str, String)> = [
        "wrapped.v",
        "iwrapped.v",
        "nested.s",
        "ab.v",
        "ab1.v",
        "ab.n",
        "ab1.n",
        "ef.u",
        "ef.j",
    ]
    .into_iter()
    .map(|needle| (needle, id(needle)))
    .collect();
    let (w, w1, k, k1) = (id("cd.w"), id("cd1.w"), id("cd.k"), id("cd1.k"));
    let _ = &ctx;
    let ids = project.identities();
    // The plain readers keep their keys (each case proves something).
    for (needle, got) in &keyed {
        assert_eq!(key_kind(ids, got), "set", "{needle}: {got}");
    }
    // Written types read through subclasses do not change: one id.
    assert_eq!(w, w1);
    assert_eq!(k, k1);
    // A transformed or inferred member takes no `Set` key, so it shares no
    // id with another container's; so does an inferred one whose type
    // happens to be fixed (`u = 1`, a method without a return annotation):
    // only written types qualify.
    for (needle, got) in &open {
        let kind = key_kind(ids, got);
        assert!(
            kind == "derived" || kind == "none",
            "{needle}: {got} {kind}"
        );
    }
}

/// PR #75 review sweep: a written annotation resolves, in its declaration's
/// scope, to one program-level type, so `typeof` another declaration,
/// `ReturnType<...>`, a defaulted generic and a member added by a global or
/// module augmentation are keyed and read the same type through every
/// container; only an annotation reading the container's type parameter (an
/// `Array<T>` augmentation's `T`) is refused.
#[test]
fn annotations_reading_other_declarations_keep_one_type_per_key() {
    let lib = "export interface M { mv: number }\nexport interface N { mv: number }\n";
    let source = "import type { M, N } from \"./lib\"\n\
declare module \"./lib\" { interface M { added: string } interface N { added: string } }\n\
declare global { interface Array<T> { gw: number; gt: T } }\n\
declare const seed: { s: number }\n\
declare function make(): boolean\n\
interface Dflt<X = string> { d: number }\n\
interface G1 { q: typeof seed; r: ReturnType<typeof make>; e: Dflt }\n\
interface G2 { q: typeof seed; r: ReturnType<typeof make>; e: Dflt<number> }\n\
interface G1b extends G1 { more: 1 }\n\
declare const g: G1 | G2\n\
declare const gb: G1b | G2\n\
declare const arrs: number[] | string[]\n\
declare const arrs2: boolean[] | string[]\n\
declare const mn: M | N\n\
export const r = [g.q, gb.q, g.r, gb.r, g.e, gb.e, arrs.gw, arrs2.gw, arrs.gt, mn.added, mn.mv]\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("lib.ts", lib),
        ("a.ts", source),
    ]);
    let path = scratch.path("a.ts");
    let ctx = Context::new();
    let mut project = scratch.open();
    let mut id = |needle: &str| {
        let mut at = find(&path, source, needle);
        at.start_byte += u64::try_from(needle.len()).unwrap() - 1;
        at.end_byte = at.start_byte + 1;
        id_at(&mut project, &at)
    };
    let (gw, gw2) = (id("arrs.gw"), id("arrs2.gw"));
    let pairs: Vec<(&str, String, String)> = [("g.q", "gb.q"), ("g.r", "gb.r"), ("g.e", "gb.e")]
        .into_iter()
        .map(|(left, right)| (left, id(left), id(right)))
        .collect();
    let keyed: Vec<(&str, String)> = ["mn.added", "mn.mv"]
        .into_iter()
        .map(|needle| (needle, id(needle)))
        .collect();
    let gt = id("arrs.gt");
    let _ = &ctx;
    let ids = project.identities();
    let unset = |id: &str| matches!(key_kind(ids, id), "derived" | "none");
    // `number[] | string[]` reads `gw` through two instantiations of one
    // member with one type: the checker's merged-instantiation clone, not a
    // union property. `gw` reads no type variable, so every instantiation
    // answers with the declaration itself and takes its binder id
    // (`derived_key.rs`).
    assert_eq!(gw, gw2);
    assert_eq!(key_kind(ids, &gw), "binder", "{gw}");
    for (left, a, b) in &pairs {
        assert_eq!(key_kind(ids, a), "set", "{left}: {a}");
        assert_eq!(a, b, "{left}");
    }
    for (needle, got) in &keyed {
        assert_eq!(key_kind(ids, got), "set", "{needle}: {got}");
    }
    assert!(unset(&gt), "arrs.gt: {gt}");
}

/// A `Set` id names a checker-made property, which no position re-resolves:
/// after a release it fails closed until a pass mints it again (to the same
/// id).
#[test]
fn a_set_id_fails_closed_after_a_release_and_is_minted_again() {
    let (scratch, path) = set_scratch();
    let ctx = Context::new();
    let mut project = scratch.open();
    let read = find(&path, SET_SOURCE, "kind\n");
    let id = project.symbol_at(&ctx, &read).unwrap();
    project
        .with_env(|env, _| assert!(env.ids.is_set_id(&id)))
        .unwrap();
    project.release_analysis_state_for_test();
    project
        .with_env(|env, _| {
            assert!(
                env.symbol_for(&id).is_none(),
                "{id} resolved after a release"
            );
        })
        .unwrap();
    assert_eq!(project.symbol_at(&ctx, &read).unwrap(), id);
}

/// A source file's unexported declarations take a `Local` key, whose chain
/// names no span (ADR 03XX-stable-locals): a statement inserted before them,
/// at the top or mid-file, keeps every id; ids stay distinct for
/// same-named declarations in sibling scopes and in one scope; an exported
/// symbol keeps its `Export` id.
#[test]
fn local_ids_survive_an_insert_before_them_and_stay_distinct() {
    let body = "function hidden(props: { v: number }) {\n  const items = [1, 2];\n  return items.map((item) => item + props.v);\n}\n\
function other(props: { v: number }) {\n  { const x = 1; void x; }\n  { const x = 2; void x; }\n  return props.v;\n}\n\
export const out = hidden({ v: 1 }) + other({ v: 2 });\n";
    let needles = [
        "hidden(",
        "items",
        "item)",
        "props: { v: number }) {\n  const",
        "other(",
        "props.v;",
        "x = 1",
        "x = 2",
        "out",
    ];
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", body)]);
    let path = scratch.path("a.ts");
    let ids_of = |source: &str| -> Vec<String> {
        scratch.write_all(&[("a.ts", source)]);
        let mut project = scratch.open();
        let ids: Vec<String> = needles
            .iter()
            .map(|needle| {
                let at = find(&path, source, needle);
                let word = needle
                    .find(|c: char| !c.is_alphanumeric())
                    .unwrap_or(needle.len());
                let at = Location {
                    end_byte: at.start_byte + u64::try_from(word).unwrap(),
                    ..at
                };
                id_at(&mut project, &at)
            })
            .collect();
        project
            .with_env(|env, _| {
                for (needle, id) in needles.iter().zip(&ids) {
                    let kind = env.ids.key(&id.as_str().into()).map(|key| match **key {
                        Key::Local { .. } => "local",
                        Key::Export(_) => "export",
                        _ => "other",
                    });
                    let want = if *needle == "out" { "export" } else { "local" };
                    assert_eq!(kind, Some(want), "{needle}: {id}");
                }
            })
            .unwrap();
        ids
    };
    let before = ids_of(body);
    let distinct: std::collections::HashSet<&String> = before.iter().collect();
    assert_eq!(distinct.len(), before.len(), "{before:?}");
    let top = ids_of(&format!(
        "import {{}} from \"./a\";\nconst inserted = 0; void inserted;\n{body}"
    ));
    assert_eq!(before, top, "an insert at the top moved an id");
    let at = body.find("function other").unwrap();
    let mid = ids_of(&format!(
        "{}const inserted = 0; void inserted;\n{}",
        &body[..at],
        &body[at..]
    ));
    assert_eq!(before, mid, "a mid-file insert moved an id");
}

/// A `Local` id minted before a release re-resolves from its chain alone.
#[test]
fn a_local_id_rehydrates_after_a_release() {
    let source =
        "function hidden() { const inner = 1; return inner; }\nexport const v = hidden();\n";
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", source)]);
    let path = scratch.path("a.ts");
    let mut project = scratch.open();
    let ctx = Context::new();
    let ids: Vec<_> = ["hidden", "inner"]
        .iter()
        .map(|needle| {
            project
                .symbol_at(&ctx, &find(&path, source, needle))
                .unwrap()
        })
        .collect();
    project.release_analysis_state_for_test();
    project
        .with_env(|env, _| {
            for id in &ids {
                assert!(
                    matches!(env.ids.key(id).map(|key| &**key), Some(Key::Local { .. })),
                    "{id}"
                );
                let symbol = env.symbol_for(id).unwrap_or_else(|| panic!("{id} lost"));
                assert_eq!(&env.id_for(symbol), id, "re-minted");
            }
        })
        .unwrap();
}

/// The ids of the first `(needle, width)` of `source`, each checked to be a
/// `Local` key of the program `source` makes.
fn local_ids(scratch: &Scratch, path: &str, source: &str, names: &[(&str, u64)]) -> Vec<String> {
    scratch.write_all(&[("a.ts", source)]);
    let mut project = scratch.open();
    let ids: Vec<String> = names
        .iter()
        .map(|(needle, width)| {
            let at = find(path, source, needle);
            let at = Location {
                end_byte: at.start_byte + width,
                ..at
            };
            id_at(&mut project, &at)
        })
        .collect();
    project
        .with_env(|env, _| {
            for id in &ids {
                let key = env.ids.key(&id.as_str().into()).map(|key| &**key);
                assert!(matches!(key, Some(Key::Local { .. })), "{id}: {key:?}");
            }
        })
        .unwrap();
    ids
}

/// An id kept across an edit names the same declaration (ADR 03XX-stable-locals):
/// an inserted declaration of the same kind and name takes the rank of the one
/// after it, and every declaration it displaces takes a new id, so no id of the
/// new program is an id of the old one.
#[test]
fn an_inserted_same_named_declaration_hands_out_no_id() {
    let before =
        "export function run() {\n  { const x = 1; void x; }\n  { const x = 2; void x; }\n}\n";
    let after = "export function run() {\n  { const x = 0; void x; }\n  { const x = 1; void x; }\n  { const x = 2; void x; }\n}\n";
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", before)]);
    let path = scratch.path("a.ts");
    let before_ids = local_ids(&scratch, &path, before, &[("x = 1", 1), ("x = 2", 1)]);
    let after_ids = local_ids(
        &scratch,
        &path,
        after,
        &[("x = 0", 1), ("x = 1", 1), ("x = 2", 1)],
    );
    for ids in [&before_ids, &after_ids] {
        let distinct: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(distinct.len(), ids.len(), "{ids:?}");
    }
    for id in &after_ids {
        assert!(
            !before_ids.contains(id),
            "{id} names a declaration of the old program"
        );
    }
}

/// Two declarations with the same kind, name and text in one scope differ in
/// rank, so their `Local` ids differ (ADR 03XX-stable-locals).
#[test]
fn identical_declarations_in_one_scope_get_distinct_ids() {
    let source = "function f() {\n  { const y = 1; void y; }\n  { const y = 1; void y; }\n}\nexport const out = f();\n";
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", source)]);
    let path = scratch.path("a.ts");
    scratch.write_all(&[("a.ts", source)]);
    let mut project = scratch.open();
    let ids: Vec<String> = [
        find(&path, source, "y = 1"),
        find_last(&path, source, "y = 1"),
    ]
    .into_iter()
    .map(|at| {
        let at = Location {
            end_byte: at.start_byte + 1,
            ..at
        };
        id_at(&mut project, &at)
    })
    .collect();
    assert_ne!(ids[0], ids[1], "two declarations share an id");
    project
        .with_env(|env, _| {
            for id in &ids {
                let key = env.ids.key(&id.as_str().into()).map(|key| &**key);
                assert!(matches!(key, Some(Key::Local { .. })), "{id}: {key:?}");
            }
        })
        .unwrap();
}

/// An edit inside a declaration moves that declaration's id and no nested
/// declaration's: a nested declaration keeps its own text and position.
#[test]
fn an_edit_inside_a_declaration_moves_only_its_own_id() {
    let before =
        "function outer() {\n  const inner = 1;\n  return inner;\n}\nexport const v = outer();\n";
    let after = "function outer() {\n  void 0;\n  const inner = 1;\n  return inner;\n}\nexport const v = outer();\n";
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", before)]);
    let path = scratch.path("a.ts");
    let before_ids = local_ids(&scratch, &path, before, &[("outer", 5), ("inner", 5)]);
    let after_ids = local_ids(&scratch, &path, after, &[("outer", 5), ("inner", 5)]);
    assert_ne!(
        before_ids[0], after_ids[0],
        "the edited declaration kept its id"
    );
    assert_eq!(before_ids[1], after_ids[1], "a nested declaration moved");
}
