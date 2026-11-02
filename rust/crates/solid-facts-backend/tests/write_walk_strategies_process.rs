//! ADR 0546: the write walk's region fold, the site-reach fixed point, the
//! helper climb's kept answers and the ordering proof's visited set answer
//! what the walks that follow every path answer. With
//! `solid_reactive_ir::follow_every_path` on (and, for the write walk and the
//! helper climb, with the role log on), each follows every path as written,
//! so that build is the reference: on randomized programs of mutually
//! recursive local functions (random call forms, entries and declaration
//! orders; ADR 0560: one in six a chain longer than the helper climb's
//! depth), the IR built without it must equal it.
//!
//! ADR 0560: the walks whose answer is path-dependent end at a deterministic
//! bound ([`solid_reactive_ir::WALK_STEP_BOUND`]) and answer nothing they
//! found there; `path_dependent_walks_end_at_their_bound_and_fail_closed`
//! builds shapes that reach it.
//!
//! The switches are process-wide, so these tests are their own binary and
//! take [`SWITCHES`] in turn.

use std::{env, fs, path::PathBuf, time::Instant};

use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};
use solid_reactive_ir::{AnalysisInputs, ExecutionRole, RoleLogTarget, RuleOptions};

/// Held by each test while it sets the process-wide switches.
static SWITCHES: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Declarations transcribed from solid-js@2.0.0-rc.17, reduced to what the
/// programs call.
const STUB: &str = r#"declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: MouseEvent) => void; children?: any };
    div: { children?: any };
  }
  interface Element {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: (prev: T | undefined) => T): Accessor<T>;
  export function createEffect<T>(compute: () => T, effect: (value: T) => void | (() => void)): void;
  export function untrack<T>(fn: () => T): T;
}

declare module "hands" {
  /** Code the program does not see: what it does with `cb` is unknown. */
  export function hand(cb: (n: number) => void): void;
}
"#;

const TSCONFIG: &str = r#"{"compilerOptions":{"jsx":"preserve","module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"include":["*.ts","*.tsx"]}"#;

const PROGRAMS: u64 = 500;

/// A small deterministic generator (xorshift64*).
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// A call of `f{j}` written in another function's body, in one of the forms
/// the walks treat differently.
fn call(random: &mut Random, j: usize) -> String {
    match random.below(13) {
        0 => format!("[1].forEach(() => f{j}(n - 1));"),
        1 => format!("untrack(() => f{j}(n - 1));"),
        2 => format!("const g{j} = () => f{j}(n - 1); g{j}();"),
        3 => format!("return void (<div>{{f{j}(n - 1)}}</div>);"),
        4 => format!("[1].forEach(f{j});"),
        5 => format!("hand(f{j});"),
        // ADR 0560: a call in a function written as a member's value (the
        // helper climb's unplaced call), and in a named function handed to
        // an inline carrier (a call with a role of its own).
        6 => format!("box.m = (k: number) => f{j}(k - 1);"),
        7 => format!("const c{j} = () => f{j}(n - 1); [1].forEach(c{j});"),
        _ => format!("if (n > {j}) f{j}(n - 1);"),
    }
}

/// An entry of `f{i}` in the component.
fn entry(random: &mut Random, i: usize) -> (String, String) {
    match random.below(14) {
        0 => (format!("  f{i}(3);\n"), String::new()),
        1 => (
            format!("  createMemo(() => {{ f{i}(3); return 1; }});\n"),
            String::new(),
        ),
        2 => (
            format!("  createEffect(() => r(), () => {{ f{i}(3); }});\n"),
            String::new(),
        ),
        3 => (String::new(), format!(" onClick={{() => f{i}(3)}}")),
        4 => (format!("  untrack(() => f{i}(3));\n"), String::new()),
        // Inline carriers, which run a function handed by name in place.
        5 => (format!("  [1].forEach(f{i});\n"), String::new()),
        6 => (
            format!("  createMemo(() => {{ [1].forEach(f{i}); return 1; }});\n"),
            String::new(),
        ),
        7 => (
            String::new(),
            format!(" onClick={{() => [1].forEach(f{i})}}"),
        ),
        8 => (format!("  hand(f{i});\n"), String::new()),
        // A named function handed to an inline carrier calls it: in an owned
        // scope that call rejects a helper's write (the climb's rejection).
        9 => (
            format!("  const e{i} = () => f{i}(3); [1].forEach(e{i});\n"),
            String::new(),
        ),
        10 => (
            String::new(),
            format!(" onClick={{() => {{ const e = () => f{i}(3); [1].forEach(e); }}}}"),
        ),
        // The function handed by name as an effect's apply: a position of
        // its own, which with another entry is a mix of roles.
        11 => (format!("  createEffect(() => r(), f{i});\n"), String::new()),
        _ => (String::new(), String::new()),
    }
}

fn program(random: &mut Random) -> String {
    // ADR 0560: one program in six is a chain of 9 to 12 functions, each
    // calling the one before it, with a few other calls: longer than the
    // helper climb's depth (8 calls), so a climb can run out of calls.
    let chain = random.chance(16);
    let k = if chain {
        9 + random.below(4) as usize
    } else {
        2 + random.below(4) as usize
    };
    let density = if chain { 5 } else { 30 + random.below(71) };
    let signal_at = random.below(k as u64 + 1) as usize;
    let mut body = String::from("  const box = { m: (k: number): void => {} };\n");
    let mut module = String::new();
    for i in 1..=k {
        if i - 1 == signal_at {
            body.push_str("  const [r, set] = createSignal(0);\n");
        }
        let mut statements = Vec::new();
        if random.chance(80) || (chain && i == 1) {
            statements.push(format!("set({i});"));
        }
        if chain && i > 1 {
            statements.push(format!("if (n > 0) f{}(n - 1);", i - 1));
        }
        for j in 1..=k {
            if j != i && random.chance(density) {
                statements.push(call(random, j));
            }
        }
        let statements = statements.join(" ");
        let declaration = if random.chance(50) {
            format!("function f{i}(n: number): void {{ {statements} }}")
        } else {
            format!("const f{i} = (n: number): void => {{ {statements} }};")
        };
        body.push_str(&format!("  {declaration}\n"));
    }
    if signal_at == k {
        body.push_str("  const [r, set] = createSignal(0);\n");
    }
    let mut handler = String::new();
    for i in 1..=k {
        // A chain is entered at its last function, and rarely elsewhere.
        if chain && i < k && !random.chance(10) {
            continue;
        }
        // Some functions are entered twice.
        for _ in 0..(1 + usize::from(random.chance(30))) {
            let (statement, attribute) = entry(random, i);
            body.push_str(&statement);
            if handler.is_empty() {
                handler = attribute;
            }
        }
    }
    if random.chance(20) {
        module.push_str("export function outside() { App(); }\n");
    }
    format!(
        "import {{ createEffect, createMemo, createSignal, untrack }} from \"solid-js\";\nimport {{ hand }} from \"hands\";\n\n{module}export function App() {{\n{body}  return <button{handler}>{{r()}}</button>;\n}}\n"
    )
}

#[test]
fn every_path_and_the_folds_build_the_same_program() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let _switches = SWITCHES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = env::temp_dir()
        .join("sc-scratch.noindex")
        .join(format!("write-walk-strategies-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let _scratch = Scratch(root.clone());
    fs::write(root.join("solid-js.d.ts"), STUB).unwrap();
    fs::write(root.join("tsconfig.json"), TSCONFIG).unwrap();
    let mut random = Random(0x0546_d1ce_5eed_0001);
    let first = program(&mut random);
    fs::write(root.join("App.tsx"), &first).unwrap();
    let root = root.canonicalize().unwrap();
    let app = root.join("App.tsx");
    let log = RoleLogTarget {
        path: root.join("roles.jsonl"),
        program: None,
    };
    let project_id = root.join("tsconfig.json").to_string_lossy().into_owned();
    let typefacts = PathBuf::from(typefacts)
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let solid_v2 = dialect::by_id("solid-v2").unwrap();
    let mut session = NativeIncrementalSession::open(
        solid_v2,
        project_id,
        vec![SourceFile {
            path: app.to_string_lossy().into_owned(),
            source: first.clone().into(),
            compiler_options: CompilerOptions::default(),
        }],
        typescript,
    )
    .unwrap();
    let mut source = first;
    let mut writes = 0;
    for version in 1..=PROGRAMS {
        if version > 1 {
            source = program(&mut random);
        }
        fs::write(&app, &source).unwrap();
        let facts = session
            .edit(
                vec![SourceChange {
                    path: app.to_string_lossy().into_owned(),
                    version,
                    source: Some(source.clone()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let build = || {
            solid_reactive_ir::build_with_analysis_inputs_measured(
                &facts,
                solid_v2.vocabulary,
                AnalysisInputs::empty(),
                &RuleOptions::default(),
            )
            .unwrap()
            .0
        };
        solid_reactive_ir::follow_every_path(true);
        let reference = build();
        solid_reactive_ir::follow_every_path(false);
        let folded = build();
        // Every few programs, the role log's own reference too.
        if version % 8 == 0 {
            solid_reactive_ir::configure_role_log(Some(&log));
            let logged = build();
            solid_reactive_ir::configure_role_log(None);
            assert_eq!(
                logged, reference,
                "program {version} builds differently with the role log:\n{source}"
            );
        }
        writes += reference.writes.len();
        assert_eq!(
            folded, reference,
            "program {version} builds differently without the role log:\n{source}"
        );
    }
    // The premise: the programs hold writes for the walks to place.
    assert!(writes > PROGRAMS as usize, "{writes} writes");
}

/// ADR 0560: the helper climb answers none at once where no call it reaches
/// rejects, none runs out of calls and none is unplaced (ADR 0546). Dropping
/// the last condition answers the same: a call the climb reaches whose own
/// answer is unplaced is written in a function stored as a member's value
/// (`handed_tail`'s escape), which no call site or position places, and the
/// write walk reaches it through the same call sites, so the write's own
/// role is already `Unknown` and the climb's `Unknown` changes nothing. These
/// programs pin that premise: each write must stay `Unknown`.
const UNPLACED_CLIMB_CALLS: [&str; 3] = [
    "  function f1(n: number): void { set(n); }\n  box.m = (k: number) => f1(k - 1);\n  createEffect(() => r(), f1);\n",
    "  function f1(n: number): void { set(n); }\n  function f2(n: number): void { f1(n); }\n  box.m = (k: number) => f2(k - 1);\n  createEffect(() => r(), f2);\n",
    "  const f1 = (n: number): void => { set(n); };\n  const f2 = (n: number): void => { f1(n); };\n  box.m = function (k: number) { f2(k - 1); };\n",
];

#[test]
fn an_unplaced_call_the_climb_reaches_leaves_the_write_unplaced() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let _switches = SWITCHES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = env::temp_dir()
        .join("sc-scratch.noindex")
        .join(format!("unplaced-climb-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let _scratch = Scratch(root.clone());
    fs::write(root.join("solid-js.d.ts"), STUB).unwrap();
    fs::write(root.join("tsconfig.json"), TSCONFIG).unwrap();
    let program = |functions: &str| {
        format!(
            "import {{ createEffect, createSignal }} from \"solid-js\";\n\nexport function App() {{\n  const [r, set] = createSignal(0);\n  const box = {{ m: (k: number): void => {{}} }};\n{functions}  return <button onClick={{() => f1(3)}}>{{r()}}</button>;\n}}\n"
        )
    };
    let first = program(UNPLACED_CLIMB_CALLS[0]);
    fs::write(root.join("App.tsx"), &first).unwrap();
    let root = root.canonicalize().unwrap();
    let app = root.join("App.tsx");
    let project_id = root.join("tsconfig.json").to_string_lossy().into_owned();
    let typefacts = PathBuf::from(typefacts)
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let solid_v2 = dialect::by_id("solid-v2").unwrap();
    let mut session = NativeIncrementalSession::open(
        solid_v2,
        project_id,
        vec![SourceFile {
            path: app.to_string_lossy().into_owned(),
            source: first.into(),
            compiler_options: CompilerOptions::default(),
        }],
        typescript,
    )
    .unwrap();
    for (index, functions) in UNPLACED_CLIMB_CALLS.iter().enumerate() {
        let source = program(functions);
        fs::write(&app, &source).unwrap();
        let facts = session
            .edit(
                vec![SourceChange {
                    path: app.to_string_lossy().into_owned(),
                    version: index as u64 + 1,
                    source: Some(source.clone()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let (built, _) = solid_reactive_ir::build_with_analysis_inputs_measured(
            &facts,
            solid_v2.vocabulary,
            AnalysisInputs::empty(),
            &RuleOptions::default(),
        )
        .unwrap();
        assert_eq!(built.writes.len(), 1, "{source}");
        assert_eq!(
            built.writes[0].execution,
            ExecutionRole::Unknown,
            "the write is placed though a call its climb reaches is not:\n{source}"
        );
    }
    eprintln!(
        "unplaced climb calls: {} programs",
        UNPLACED_CLIMB_CALLS.len()
    );
}

/// ADR 0560's bounded shapes, built with the bound lowered for this process
/// ([`solid_reactive_ir::bound_walks`]) so they reach it with small
/// programs. On each, a series' work must stay linear in the program at a
/// fixed bound, every walk within it, and no write may come out clean.
///
/// - `untrack`, `alias`: `k` local functions that all call each other
///   through `untrack(() => fj(n - 1))` or `const gj = () => fj(n - 1);
///   gj();`, entered from the component body. The write walk's region fold
///   (ADR 0546) does not apply to those edges; following the paths is
///   factorial (at ed30c5738, release: untrack k = 8 38.7 s, alias k = 7
///   5.6 s).
/// - `reach-dead`: the same functions calling each other through
///   `[1].forEach(() => fj(n - 1))`, entered only from dead code: `k * k`
///   functions whose answers read each other, which the least fixed point
///   finds never run, and which derive depth first.
/// - `climb`: the complexity ladder's `climb` shape with
///   [`solid_reactive_ir::follow_every_path`] on (the reference climb that
///   the role log, a probe and a proof's deferred check take).
fn bounded_shapes() -> [&'static str; 4] {
    ["untrack", "alias", "reach-dead", "climb"]
}

/// The bound this process takes: the shapes pass it by k = 6..8.
const BOUND: u64 = 1 << 10;

/// The largest component each series builds.
const MAX_K: usize = 9;

fn bounded_shape_program(shape: &str, k: usize) -> String {
    let head = "import { createSignal, untrack } from \"solid-js\";\n\n";
    if shape == "climb" {
        let helpers = (1..=k)
            .map(|i| {
                let calls = (1..=k)
                    .filter(|&j| j != i)
                    .map(|j| format!("if (n > {j}) h{j}(n - 1);"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let write = if i == 1 { "set(1); " } else { "" };
                format!("  const h{i} = (n: number): void => {{ {write}{calls} }};\n")
            })
            .collect::<String>();
        return format!(
            "{head}export function App() {{\n  const [r, set] = createSignal(0);\n{helpers}  const hf = () => {{ h1(3); }}; [1].forEach(hf);\n  return <button>{{r()}}</button>;\n}}\n"
        );
    }
    let call = |j: usize| match shape {
        "untrack" => format!("untrack(() => f{j}(n - 1));"),
        "alias" => format!("const g{j} = () => f{j}(n - 1); g{j}();"),
        _ => format!("[1].forEach(() => f{j}(n - 1));"),
    };
    let functions = (1..=k)
        .map(|i| {
            let calls = (1..=k)
                .filter(|&j| j != i)
                .map(call)
                .collect::<Vec<_>>()
                .join(" ");
            format!("  function f{i}(n: number): void {{ set({i}); {calls} }}\n")
        })
        .collect::<String>();
    let entry = if shape == "reach-dead" {
        "  if (false as boolean) f1(3);\n"
    } else {
        "  f1(3);\n"
    };
    format!(
        "{head}export function App() {{\n  const [r, set] = createSignal(0);\n{functions}{entry}  return <button>{{r()}}</button>;\n}}\n"
    )
}

#[test]
fn path_dependent_walks_end_at_their_bound_and_fail_closed() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let _switches = SWITCHES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    solid_reactive_ir::bound_walks(Some(BOUND));
    let root = env::temp_dir()
        .join("sc-scratch.noindex")
        .join(format!("walk-bounds-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let _scratch = Scratch(root.clone());
    fs::write(root.join("solid-js.d.ts"), STUB).unwrap();
    fs::write(root.join("tsconfig.json"), TSCONFIG).unwrap();
    let first = bounded_shape_program("untrack", 2);
    fs::write(root.join("App.tsx"), &first).unwrap();
    let root = root.canonicalize().unwrap();
    let app = root.join("App.tsx");
    let project_id = root.join("tsconfig.json").to_string_lossy().into_owned();
    let typefacts = PathBuf::from(typefacts)
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let solid_v2 = dialect::by_id("solid-v2").unwrap();
    let mut session = NativeIncrementalSession::open(
        solid_v2,
        project_id,
        vec![SourceFile {
            path: app.to_string_lossy().into_owned(),
            source: first.clone().into(),
            compiler_options: CompilerOptions::default(),
        }],
        typescript,
    )
    .unwrap();
    let mut version = 0;
    let mut failures = Vec::new();
    for shape in bounded_shapes() {
        solid_reactive_ir::follow_every_path(shape == "climb");
        let mut curve = Vec::new();
        let mut exhausted_at = None;
        for k in 2..=MAX_K {
            let source = bounded_shape_program(shape, k);
            fs::write(&app, &source).unwrap();
            version += 1;
            let facts = session
                .edit(
                    vec![SourceChange {
                        path: app.to_string_lossy().into_owned(),
                        version,
                        source: Some(source.clone()),
                        compiler_options: CompilerOptions::default(),
                    }],
                    None,
                )
                .unwrap();
            let started = Instant::now();
            let (built, timings) = solid_reactive_ir::build_with_analysis_inputs_measured(
                &facts,
                solid_v2.vocabulary,
                AnalysisInputs::empty(),
                &RuleOptions::default(),
            )
            .unwrap();
            let elapsed = started.elapsed();
            let writes = if shape == "climb" { 1 } else { k };
            assert_eq!(
                built.writes.len(),
                writes,
                "{shape} at k = {k} holds {} writes:\n{source}",
                built.writes.len()
            );
            curve.push(format!(
                "k{k}: walk {}, reach {}, peak {}, exhausted {}, {:.0} ms",
                timings.write_walk_steps,
                timings.reach_evaluations,
                timings.bounded_walk_peak,
                timings.bounded_walks_exhausted,
                elapsed.as_secs_f64() * 1000.0
            ));
            // Every walk stops within a few steps of its bound (the steps
            // its enclosing folds take while it unwinds), and a series'
            // work is linear in the program at a fixed bound: each of the
            // k writes, its nested walks and the site queries do at most a
            // bound's work each.
            if timings.bounded_walk_peak > 2 * BOUND {
                failures.push(format!(
                    "{shape} at k = {k}: a walk did {} steps past a bound of {BOUND}",
                    timings.bounded_walk_peak
                ));
            }
            let work = timings.write_walk_steps + timings.reach_evaluations;
            let ceiling = 8 * (k * k) as u64 * BOUND;
            if work > ceiling {
                failures.push(format!(
                    "{shape} at k = {k}: work {work} is over 8 * k^2 * bound ({ceiling})"
                ));
            }
            if timings.bounded_walks_exhausted > 0 {
                exhausted_at.get_or_insert(k);
                // Fail closed: a walk that ended at its bound proves no
                // write legal. In the component body each write is a
                // violation where it is proven, and unproven elsewhere.
                for write in &built.writes {
                    if !write.execution.reports_disallowed_write()
                        && write.execution != ExecutionRole::Unknown
                    {
                        failures.push(format!(
                            "{shape} at k = {k}: a write at {} is {:?} past the bound",
                            write.location.start_byte, write.execution
                        ));
                    }
                }
                // An undecided site-reach query proves no violation.
                if shape == "reach-dead" && !built.unreached_sites.iter().any(|site| site.undecided)
                {
                    failures.push(format!(
                        "{shape} at k = {k}: the site-reach query ended at its bound and no site is undecided"
                    ));
                }
            }
        }
        eprintln!("bounded {shape}: {}", curve.join("; "));
        // The premise: the shape reaches the bound (else an exact fold now
        // answers it, and the complexity ladder should build it instead).
        if exhausted_at.is_none() {
            failures.push(format!("{shape}: no walk reached the bound by k = {MAX_K}"));
        }
    }
    solid_reactive_ir::follow_every_path(false);
    solid_reactive_ir::bound_walks(None);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
