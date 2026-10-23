//! Binder-level symbol ids are a pure function of the program (ADR 0366,
//! STABLE-IDS V1, V2, V5, V8): demand order, file order, checker history and
//! the process's symbol counters do not change them, and a derived symbol
//! never receives one. Its own process, because the history tests flip a
//! process-wide checker flag.

use solid_typefacts::context::Context;
use solid_typefacts::project::Project;
use solid_typefacts::unsafe_boundary::use_canonical_history_for_tests;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use typefacts::Location;

/// Serialises the tests: they open programs under different history flags.
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
        ]
        .into_iter()
        .chain(extra.iter().copied())
        {
            std::fs::write(root.join(name), text).unwrap();
        }
        Self { root }
    }

    fn path(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }

    fn open(&self) -> Project {
        Project::open(&Context::new(), &self.path("tsconfig.json"), None, None).unwrap()
    }
}

const ALL: [&str; 5] = ["lib.ts", "aug.ts", "main.ts", "sa.ts", "sb.ts"];

/// (label, file, source, needle, offset into the needle, binder-level?)
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

/// Label to id, asking in `order` (indexes into the probes), optionally after
/// forcing every probe's type first (a different checker history).
fn ids(
    project: &mut Project,
    fixture: &Fixture,
    order: &[usize],
    warm: bool,
) -> BTreeMap<String, String> {
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
        let id = project
            .symbol_at(&ctx, &location(fixture, probe))
            .unwrap_or_else(|error| panic!("{}: {error:?}", probe.0));
        out.insert(probe.0.to_owned(), id.to_string());
    }
    out
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

/// Only the binder-level probes (a derived probe's counter depends on demand
/// order by design).
fn binder(map: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let kinds: BTreeMap<_, _> = probes().into_iter().map(|p| (p.0, p.5)).collect();
    map.iter()
        .filter(|(label, _)| kinds[label.as_str()])
        .map(|(l, i)| (l.clone(), i.clone()))
        .collect()
}

fn baseline() -> BTreeMap<String, String> {
    let fixture = Fixture::new(&ALL, &[]);
    let order: Vec<usize> = (0..probes().len()).collect();
    ids(&mut fixture.open(), &fixture, &order, false)
}

#[test]
fn binder_ids_are_hashes_and_derived_symbols_are_not() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    use_canonical_history_for_tests(false);
    let base = baseline();
    for probe in probes() {
        let id = &base[probe.0];
        assert_eq!(is_hash(id), probe.5, "{}: {id}", probe.0);
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
    for derived in ["b.value", "q.value"] {
        assert_ne!(base[derived], base["value decl"], "{derived}");
        assert!(!is_hash(&base[derived]), "{derived}");
    }
    assert_ne!(base["b.value"], base["q.value"]);
    // Library globals are named by their name (one id for the merged symbol).
    for global in ["JSON", "Set", "Array"] {
        assert!(is_hash(&base[global]), "{global}");
    }
}

#[test]
fn ids_do_not_depend_on_demand_order() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    use_canonical_history_for_tests(false);
    let base = binder(&baseline());
    let count = probes().len();
    let reversed: Vec<usize> = (0..count).rev().collect();
    let mut orders = vec![reversed];
    orders.extend((1..=4).map(|seed| shuffled(count, seed)));
    for (n, order) in orders.iter().enumerate() {
        let fixture = Fixture::new(&ALL, &[]);
        let got = ids(&mut fixture.open(), &fixture, order, false);
        assert_eq!(binder(&got), base, "demand order {n}: {order:?}");
    }
}

#[test]
fn ids_do_not_depend_on_file_order() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    use_canonical_history_for_tests(false);
    let base = binder(&baseline());
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
        assert_eq!(binder(&got), base, "file order {files:?}");
    }
}

#[test]
fn ids_do_not_depend_on_checker_history() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let base = binder(&baseline());
    let count = probes().len();
    let order: Vec<usize> = (0..count).collect();
    let reversed: Vec<usize> = (0..count).rev().collect();
    for canonical in [false, true] {
        use_canonical_history_for_tests(canonical);
        // Cold, warmed by forcing every probe's type first (instantiations are
        // created before any id is asked for), and warmed + reversed.
        for (warm, ask) in [(false, &order), (true, &order), (true, &reversed)] {
            let fixture = Fixture::new(&ALL, &[]);
            let got = ids(&mut fixture.open(), &fixture, ask, warm);
            assert_eq!(
                binder(&got),
                base,
                "canonical history {canonical}, warm {warm}"
            );
        }
    }
    use_canonical_history_for_tests(false);
}

#[test]
fn private_name_ids_survive_the_checker_counters() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    use_canonical_history_for_tests(false);
    // A class symbol's process-wide id is embedded in its private names'
    // symbol names (`#<classId>@count`). Classes bound before it, in this
    // project and in earlier projects of the process, shift it.
    let filler =
        "export class F1 { #a = 1 }\nexport class F2 { #a = 2 }\nexport class F3 { #a = 3 }\n";
    let base = binder(&baseline());
    let mut with_filler_first: Vec<&str> = vec!["aaa.ts"];
    with_filler_first.extend(ALL);
    let fixture = Fixture::new(&with_filler_first, &[("aaa.ts", filler)]);
    let order: Vec<usize> = (0..probes().len()).collect();
    let got = binder(&ids(&mut fixture.open(), &fixture, &order, false));
    for label in ["#count decl", "#id decl", "#count use"] {
        assert_eq!(got[label], base[label], "{label}");
    }
    assert_eq!(got, base);
}
