//! Symbol ids are a pure function of the program (ADR 0366, STABLE-IDS V1,
//! V2, V5, V8; ADR 03XX-p6): demand order, file order, checker history and the
//! process's symbol counters do not change them. A binder-level symbol takes
//! its binder key, a multi-origin property its `Set` key (ADR 0371), and a
//! derived symbol never takes either: it takes its exact derived key, or no
//! id.
//! Its own process: the private-name test depends on the process's symbol
//! counters.

use solid_typefacts::context::Context;
use solid_typefacts::project::Project;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use typefacts::Location;

/// Fixtures alive in the process: the shared root is removed when the last one
/// drops, also when its test panics.
static LIVE: AtomicUsize = AtomicUsize::new(0);

/// Serialises the tests: the private-name test counts process-wide symbols.
static SERIAL: Mutex<()> = Mutex::new(());

const LIB: &str = r#"export interface Box<T> { value: T }
export class Counter {
  #count = 0
  #id = 1
  inc() { return ++this.#count }
  get id() { return this.#id }
}
export function make<T>(v: T): Box<T> { return { value: v } }
export const sym: unique symbol = Symbol()
export class WithSym { [sym] = 1 }
"#;

const AUG: &str = r#"import "./lib"
declare module "./lib" {
  interface Box<T> { extra: T }
}
export {}
"#;

const MAIN: &str = r#"import { make, Counter, type Box } from "./lib"
const b = make(1)
export const read = b.value
export const extra = (b as Box<number>).extra
const c = new Counter()
export const n = c.inc()
export const j = JSON.stringify(read)
export const s = new Set<number>().size
export const a = Array.isArray(read)
export const u = read === undefined
export const m = [1].map((x) => x)
export const q: Box<string> = make("s")
export const qv = q.value
"#;

/// Multi-origin properties (ADR 0371): union, intersection and spread
/// properties, over interfaces, instantiated aliases and parameter properties.
const SHAPES: &str = r#"export interface Circle { kind: "circle"; r: number; tag?: string }
export interface Square { kind: "square"; s: number; tag?: string }
export interface Circle2 extends Circle { more: number }
export type Shape = Circle | Square
export type Res<T> = { ok: true; value: T } | { ok: false; error: string }
export type L = { a: number }
export type R = { a: number; b: string }
export class PA { constructor(public x: number) {} }
export class PB { constructor(public x: number) {} }
const k = "c"
export interface CX { [k]: number }
export interface CY { [k]: number }
"#;

const USES: &str = r#"import type { Shape, Circle, Square, Circle2, Res, L, R, PA, PB, CX, CY } from "./shapes"
declare const s1: Shape
declare const s2: Circle | Square | Circle2
declare const r1: Res<number>
declare const r2: Res<string>
declare const i: L & R
declare const u: L | R
declare const x: { p: number }
declare const y: { p?: number }
declare const pab: PA | PB
declare const cxy: CX | CY
export const k1 = s1.kind
export const k2 = s2.kind
export const t1 = s1.tag
export const o1 = r1.ok
export const o2 = r2.ok
export const ia = i.a
export const ua = u.a
export const sp = ({ ...x, ...y }).p
export const px = pab.x
export const cc = cxy.c
"#;

const SCRIPT_A: &str = "interface Merged { a: number }\nconst scriptValue = 1\n";
const SCRIPT_B: &str = "interface Merged { b: number }\ndeclare const mergedUse: Merged\n";

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    /// `order` is the `files` order of the tsconfig.
    fn new(order: &[&str], extra: &[(&str, &str)]) -> Self {
        // One root for every fixture of the process (the tests are serialised):
        // ids hash paths, so projects compared with each other share them.
        let root = std::env::temp_dir().join(format!("binder-ids-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let files: Vec<String> = order.iter().map(|name| format!("\"{name}\"")).collect();
        std::fs::write(
            root.join("tsconfig.json"),
            format!(
                r#"{{"compilerOptions":{{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"}},"files":[{}]}}"#,
                files.join(",")
            ),
        )
        .unwrap();
        for (name, text) in [
            ("lib.ts", LIB),
            ("aug.ts", AUG),
            ("main.ts", MAIN),
            ("sa.ts", SCRIPT_A),
            ("sb.ts", SCRIPT_B),
            ("shapes.ts", SHAPES),
            ("uses.ts", USES),
        ]
        .into_iter()
        .chain(extra.iter().copied())
        {
            std::fs::write(root.join(name), text).unwrap();
        }
        LIVE.fetch_add(1, Ordering::SeqCst);
        Self { root }
    }

    fn path(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }

    fn open(&self) -> Project {
        Project::open(&Context::new(), &self.path("tsconfig.json"), None, None).unwrap()
    }
}

const ALL: [&str; 7] = [
    "lib.ts",
    "aug.ts",
    "main.ts",
    "sa.ts",
    "sb.ts",
    "shapes.ts",
    "uses.ts",
];

/// (label, file, source, needle, offset into the needle, binder?): a
/// binder-level symbol or a keyed multi-origin property, else a derived
/// symbol.
type Probe = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    usize,
    bool,
);

fn probes() -> Vec<Probe> {
    vec![
        ("make call", "main.ts", MAIN, "make(1)", 0, true),
        ("Counter ctor", "main.ts", MAIN, "Counter()", 0, true),
        // A class instance's members are `this`-instantiated symbols.
        ("inc call", "main.ts", MAIN, "inc()", 0, false),
        ("JSON", "main.ts", MAIN, "JSON", 0, true),
        ("Set", "main.ts", MAIN, "Set<number>", 0, true),
        ("Array", "main.ts", MAIN, "Array.isArray", 0, true),
        ("isArray", "main.ts", MAIN, "isArray", 0, true),
        ("undefined", "main.ts", MAIN, "undefined", 0, true),
        ("map", "main.ts", MAIN, "map(", 0, false),
        ("Box decl", "lib.ts", LIB, "Box<T>", 0, true),
        ("Box aug", "aug.ts", AUG, "Box<T>", 0, true),
        ("value decl", "lib.ts", LIB, "value: T", 0, true),
        ("#count decl", "lib.ts", LIB, "#count", 0, true),
        ("#id decl", "lib.ts", LIB, "#id", 0, true),
        ("#count use", "lib.ts", LIB, "this.#count", 5, true),
        ("sym decl", "lib.ts", LIB, "sym: unique", 0, true),
        ("Merged a", "sa.ts", SCRIPT_A, "Merged", 0, true),
        ("Merged b", "sb.ts", SCRIPT_B, "Merged", 0, true),
        ("scriptValue", "sa.ts", SCRIPT_A, "scriptValue", 0, true),
        // Properties read through an instantiation are derived symbols.
        ("b.value", "main.ts", MAIN, "b.value", 2, false),
        ("q.value", "main.ts", MAIN, "q.value", 2, false),
        // Multi-origin properties (ADR 0371).
        ("Circle.kind", "shapes.ts", SHAPES, "kind", 0, true),
        ("s1.kind", "uses.ts", USES, "s1.kind", 3, true),
        ("s2.kind", "uses.ts", USES, "s2.kind", 3, true),
        ("s1.tag", "uses.ts", USES, "s1.tag", 3, true),
        ("r1.ok", "uses.ts", USES, "r1.ok", 3, true),
        ("r2.ok", "uses.ts", USES, "r2.ok", 3, true),
        ("i.a", "uses.ts", USES, "i.a", 2, true),
        ("u.a", "uses.ts", USES, "u.a", 2, true),
        ("spread.p", "uses.ts", USES, "}).p", 3, true),
        ("pab.x", "uses.ts", USES, "pab.x", 4, true),
        ("PA.x", "shapes.ts", SHAPES, "x: number", 0, true),
        // Its origins are late-bound members, which have no binder key.
        ("cxy.c", "uses.ts", USES, "cxy.c", 4, false),
    ]
}

fn location(fixture: &Fixture, probe: &Probe) -> Location {
    let (_, file, source, needle, offset, _) = *probe;
    let start = source.find(needle).unwrap_or_else(|| panic!("{needle}")) + offset;
    Location {
        path: fixture.path(file).into(),
        start_byte: start as u64,
        end_byte: (start + 1) as u64,
    }
}

fn is_hash(id: &str) -> bool {
    id.starts_with("symbol:h:")
}

/// Label to whether its id was minted from an exact derived key.
type Derived = BTreeMap<String, bool>;

/// Label to id, asking in `order` (indexes into the probes), optionally after
/// forcing every probe's type first (a different checker history).
fn ids(
    project: &mut Project,
    fixture: &Fixture,
    order: &[usize],
    warm: bool,
) -> BTreeMap<String, String> {
    ids_with_kinds(project, fixture, order, warm).0
}

/// [`ids`], and which ids are derived keys. A symbol with no exact key has
/// the empty id.
fn ids_with_kinds(
    project: &mut Project,
    fixture: &Fixture,
    order: &[usize],
    warm: bool,
) -> (BTreeMap<String, String>, Derived) {
    let ctx = Context::new();
    let probes = probes();
    if warm {
        for index in order.iter().rev() {
            let _ = project.describe_type_at(&ctx, &location(fixture, &probes[*index]));
        }
    }
    let mut out = BTreeMap::new();
    for index in order {
        let probe = &probes[*index];
        let id = match project.symbol_at(&ctx, &location(fixture, probe)) {
            Ok(id) => id.to_string(),
            Err(error) if error.0.contains("has no exact id") => String::new(),
            Err(error) => panic!("{}: {error:?}", probe.0),
        };
        out.insert(probe.0.to_owned(), id);
    }
    let derived = out
        .iter()
        .map(|(label, id)| {
            let derived = project.identities().is_derived_id(&id.as_str().into());
            (label.clone(), derived)
        })
        .collect();
    (out, derived)
}

fn shuffled(count: usize, mut seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..count).collect();
    for index in (1..count).rev() {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        order.swap(index, ((seed >> 33) as usize) % (index + 1));
    }
    order
}

fn baseline() -> BTreeMap<String, String> {
    baseline_with_kinds().0
}

fn baseline_with_kinds() -> (BTreeMap<String, String>, Derived) {
    let fixture = Fixture::new(&ALL, &[]);
    let order: Vec<usize> = (0..probes().len()).collect();
    ids_with_kinds(&mut fixture.open(), &fixture, &order, false)
}

#[test]
fn binder_ids_are_binder_keys_and_derived_symbols_take_derived_keys() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (base, derived) = baseline_with_kinds();
    for probe in probes() {
        let id = &base[probe.0];
        if probe.5 {
            assert!(is_hash(id) && !derived[probe.0], "{}: {id}", probe.0);
        } else {
            // No counter: an exact derived key, or no id at all.
            assert!(
                id.is_empty() || (is_hash(id) && derived[probe.0]),
                "{}: {id}",
                probe.0
            );
        }
    }
    // Every hashed id is a distinct symbol, except the declarations of one
    // merged symbol: Box in lib.ts and aug.ts, Merged in sa.ts and sb.ts,
    // #count at its declaration and at its use.
    assert_eq!(base["Box decl"], base["Box aug"]);
    assert_eq!(base["Merged a"], base["Merged b"]);
    assert_eq!(base["#count decl"], base["#count use"]);
    assert_ne!(base["#count decl"], base["#id decl"]);
    // A derived property never takes its declaration's id, whichever is asked
    // first.
    for label in ["b.value", "q.value"] {
        assert_ne!(base[label], base["value decl"], "{label}");
        assert!(derived[label], "{label}");
    }
    assert_ne!(base["b.value"], base["q.value"]);
    // Library globals are named by their name (one id for the merged symbol).
    for global in ["JSON", "Set", "Array"] {
        assert!(is_hash(&base[global]), "{global}");
    }
}

/// ADR 0371: a multi-origin property's id is its combination, name and origin
/// set. The same origins under another containing type (a union with an
/// extra constituent that inherits `kind`, another instantiation of `Res`)
/// are the same property; another name, combination or origin set is not,
/// and no property shares an id with one of its origins.
#[test]
fn multi_origin_properties_are_keyed_by_their_origin_set() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let base = baseline();
    assert_eq!(base["s1.kind"], base["s2.kind"]);
    assert_eq!(base["r1.ok"], base["r2.ok"]);
    let distinct = [
        "Circle.kind",
        "s1.kind",
        "s1.tag",
        "r1.ok",
        "i.a",
        "u.a",
        "spread.p",
        "pab.x",
        "PA.x",
        "value decl",
    ];
    for (n, left) in distinct.iter().enumerate() {
        for right in &distinct[n + 1..] {
            assert_ne!(base[*left], base[*right], "{left} and {right} share an id");
        }
    }
}

#[test]
fn ids_do_not_depend_on_demand_order() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let base = baseline();
    let count = probes().len();
    let reversed: Vec<usize> = (0..count).rev().collect();
    let mut orders = vec![reversed];
    orders.extend((1..=4).map(|seed| shuffled(count, seed)));
    for (n, order) in orders.iter().enumerate() {
        let fixture = Fixture::new(&ALL, &[]);
        let got = ids(&mut fixture.open(), &fixture, order, false);
        assert_eq!(got, base, "demand order {n}: {order:?}");
    }
}

#[test]
fn ids_do_not_depend_on_file_order() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let base = baseline();
    let order: Vec<usize> = (0..probes().len()).collect();
    let mut file_orders: Vec<Vec<&str>> = vec![ALL.iter().rev().copied().collect()];
    for seed in 1..=4u64 {
        file_orders.push(
            shuffled(ALL.len(), seed)
                .into_iter()
                .map(|i| ALL[i])
                .collect(),
        );
    }
    for files in &file_orders {
        let fixture = Fixture::new(files, &[]);
        let got = ids(&mut fixture.open(), &fixture, &order, false);
        assert_eq!(got, base, "file order {files:?}");
    }
}

#[test]
fn ids_do_not_depend_on_checker_history() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let base = baseline();
    let count = probes().len();
    let order: Vec<usize> = (0..count).collect();
    let reversed: Vec<usize> = (0..count).rev().collect();
    // Cold, warmed by forcing every probe's type first (instantiations are
    // created before any id is asked for), and warmed + reversed.
    for (warm, ask) in [(false, &order), (true, &order), (true, &reversed)] {
        let fixture = Fixture::new(&ALL, &[]);
        let got = ids(&mut fixture.open(), &fixture, ask, warm);
        assert_eq!(got, base, "warm {warm}");
    }
}

#[test]
fn private_name_ids_survive_the_checker_counters() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // A class symbol's process-wide id is embedded in its private names'
    // symbol names (`#<classId>@count`). Classes bound before it, in this
    // project and in earlier projects of the process, shift it.
    let filler =
        "export class F1 { #a = 1 }\nexport class F2 { #a = 2 }\nexport class F3 { #a = 3 }\n";
    let base = baseline();
    let mut with_filler_first: Vec<&str> = vec!["aaa.ts"];
    with_filler_first.extend(ALL);
    let fixture = Fixture::new(&with_filler_first, &[("aaa.ts", filler)]);
    let order: Vec<usize> = (0..probes().len()).collect();
    let got = ids(&mut fixture.open(), &fixture, &order, false);
    for label in ["#count decl", "#id decl", "#count use"] {
        assert_eq!(got[label], base[label], "{label}");
    }
    assert_eq!(got, base);
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if LIVE.fetch_sub(1, Ordering::SeqCst) == 1 {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}
