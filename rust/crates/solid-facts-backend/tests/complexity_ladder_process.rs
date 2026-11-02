//! The complexity ladder: one program per layer kind and depth (1..=20), a
//! write, an action, a refresh and a read nested under that many layers in
//! three hosts (the component body and a memo, which own the scope, and an
//! event handler, which does not). The checker's work must grow with the
//! program, near-linearly in the depth, however many reachable paths the
//! layers multiply: each in-place layer is reached by two edges, so a call at
//! depth N lies on about 2^(N+1) paths.
//!
//! Work is counted, never timed: the rows the IR holds for each host
//! (writes, actions, reads) and the reachability count's visits
//! ([`solid_reactive_ir::BuildTimings::reachability_visits`]). Wall time is
//! printed with `--nocapture` and gates nothing.

use std::{env, fs, path::PathBuf, time::Instant};

use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};
use solid_reactive_ir::{AnalysisInputs, RuleOptions};

/// Declarations transcribed from solid-js@2.0.0-rc.17
/// (`fixtures/reactive-ir/local-alias-member-reads/solid-js.d.ts`), reduced
/// to what the ladder calls.
const STUB: &str = r#"declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: MouseEvent) => void; children?: any };
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
  export function refresh<T>(target: Accessor<T>): Promise<T>;
  export function action<Args extends any[], Y, R>(
    genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>
  ): (...args: Args) => Promise<R>;
}
"#;

const TSCONFIG: &str = r#"{"compilerOptions":{"jsx":"preserve","module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"include":["*.ts","*.tsx"]}"#;

const MAX_DEPTH: usize = 20;

/// The deepest program built for `layer`. Named closures stop at 12: their
/// write-role queries still cost time exponential in the depth (debug,
/// whole build: 286 ms at depth 10, 8.1 s at 15, 32 s at 17, 69 s at 20),
/// though rows and visits stay flat. That is role work this ladder does not
/// count.
fn max_depth(layer: Layer) -> usize {
    match layer {
        Layer::Closure => 12,
        _ => MAX_DEPTH,
    }
}

/// How a layer wraps what it holds; every one runs its body once, in place.
#[derive(Clone, Copy, Debug)]
enum Layer {
    /// `(() => { … })();`
    Iife,
    /// `const fN = () => { … }; fN();`
    Closure,
    /// `hN(() => { … });` with `const hN = (cb) => { cb(); };` in the body.
    Helper,
    /// `mN(() => { … });` with `function mN(cb) { cb(); }` in the module.
    ModuleHelper,
    /// `[1].forEach(() => { … });`, an inline carrier.
    Carrier,
}

const LAYERS: [Layer; 5] = [
    Layer::Iife,
    Layer::Closure,
    Layer::Helper,
    Layer::ModuleHelper,
    Layer::Carrier,
];

/// Where the layers stand. `B` and `M` own the scope (a write, an action and
/// a refresh there are violations; so is the body's read); `H` is an event
/// handler, where every payload is legal.
const HOSTS: [char; 3] = ['B', 'M', 'H'];

fn wrap(layer: Layer, host: char, depth: usize, payload: &str, module: &mut String) -> String {
    let mut code = payload.to_string();
    let mut locals = String::new();
    for level in (1..=depth).rev() {
        code = match layer {
            Layer::Iife => format!("(() => {{ {code} }})();"),
            Layer::Closure => {
                format!("const f{host}{level} = () => {{ {code} }}; f{host}{level}();")
            }
            Layer::Helper => {
                locals.push_str(&format!(
                    "  const h{host}{level} = (cb: () => void) => {{ cb(); }};\n"
                ));
                format!("h{host}{level}(() => {{ {code} }});")
            }
            Layer::ModuleHelper => {
                module.push_str(&format!(
                    "function m{host}{level}(cb: () => void) {{ cb(); }}\n"
                ));
                format!("m{host}{level}(() => {{ {code} }});")
            }
            Layer::Carrier => format!("[1].forEach(() => {{ {code} }});"),
        };
    }
    format!("{locals}  {code}\n")
}

/// The program for `layer` at `depth`, with the hosts in `hosts`, each
/// between `/*X{*/` and `/*}X*/` markers.
fn program(layer: Layer, depth: usize, hosts: &[char]) -> String {
    let mut module = String::new();
    let mut body = String::new();
    let mut handler = String::new();
    for &host in hosts {
        let payload = format!("set{host}(1); a{host}(); refresh(m{host}); r{host}();");
        body.push_str(&format!(
            "  const [r{host}, set{host}] = createSignal(0);\n  const a{host} = action(function* () {{}});\n  const m{host} = createMemo(() => 1);\n"
        ));
        let layered = wrap(layer, host, depth, &payload, &mut module);
        match host {
            'B' => body.push_str(&format!("  /*B{{*/\n{layered}  /*}}B*/\n")),
            'M' => body.push_str(&format!(
                "  createMemo(() => {{\n  /*M{{*/\n{layered}  /*}}M*/\n    return 1;\n  }});\n"
            )),
            _ => handler = format!(" onClick={{() => {{\n  /*H{{*/\n{layered}  /*}}H*/\n  }}}}"),
        }
    }
    format!(
        "import {{ action, createMemo, createSignal, refresh }} from \"solid-js\";\n\n{module}\nexport function App() {{\n{body}  return <button{handler}>x</button>;\n}}\n"
    )
}

/// The affine envelope a series must stay under: `2 * depth * work(1)`, and
/// at least `2 * depth`. Near-linear growth passes; a doubling per layer
/// leaves it by depth 8 or so.
fn within_envelope(series: &[usize], depth: usize) -> bool {
    let first = series[0].max(1);
    series[depth - 1] <= 2 * depth * first
}

/// The `(layer, host)` series whose rows still grow exponentially. Each must
/// leave the envelope, so a fix that makes one near-linear fails here until
/// it is removed. Empty since rows carry their path count: at b38255c8b
/// every series doubled per layer (body 10, 22, 46, 94 rows at depths 1-4;
/// memo and handler 25, 55, 115, 235).
const KNOWN_EXPONENTIAL_ROWS: &[(&str, char)] = &[];

/// The layers whose reachability visits still grow exponentially. Empty
/// since the count is arithmetic on the acyclic graph: at b38255c8b the walk
/// enumerated every path (44, 87, 178, 365 visits at depths 1-4).
const KNOWN_EXPONENTIAL_VISITS: &[&str] = &[];

#[test]
fn checker_work_grows_near_linearly_with_nested_layers() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = env::temp_dir()
        .join("sc-scratch.noindex")
        .join(format!("complexity-ladder-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let _scratch = Scratch(root.clone());
    fs::write(root.join("solid-js.d.ts"), STUB).unwrap();
    fs::write(root.join("tsconfig.json"), TSCONFIG).unwrap();
    let app = root.join("App.tsx");
    let first = program(Layer::Iife, 1, &HOSTS);
    fs::write(&app, &first).unwrap();
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
    for layer in LAYERS {
        let name = format!("{layer:?}");
        let mut rows = HOSTS.map(|_| Vec::<usize>::new());
        let mut visits = Vec::<u64>::new();
        let mut alive = HOSTS.to_vec();
        let mut curve = Vec::new();
        let deepest = max_depth(layer);
        for depth in 1..=deepest {
            let source = program(layer, depth, &alive);
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
            let region = |host: char| {
                let open = source.find(&format!("/*{host}{{*/")).unwrap() as u64;
                let close = source.find(&format!("/*}}{host}*/")).unwrap() as u64;
                move |start: u64, end: u64| start >= open && end <= close
            };
            let mut total = 0;
            for (index, &host) in HOSTS.iter().enumerate() {
                if !alive.contains(&host) {
                    continue;
                }
                let inside = region(host);
                let held = built
                    .writes
                    .iter()
                    .filter(|row| inside(row.location.start_byte, row.location.end_byte))
                    .count()
                    + built
                        .actions
                        .iter()
                        .filter(|row| inside(row.location.start_byte, row.location.end_byte))
                        .count()
                    + built
                        .reads
                        .iter()
                        .filter(|row| inside(row.location.start_byte, row.location.end_byte))
                        .count();
                // The premise: the layered payload is analyzed at all.
                assert!(
                    held > 0,
                    "{name}/{host} at depth {depth} holds no row:\n{source}"
                );
                total += held;
                rows[index].push(held);
            }
            visits.push(timings.reachability_visits);
            curve.push(format!(
                "d{depth}: rows {total}, visits {}, {:.0} ms",
                timings.reachability_visits,
                elapsed.as_secs_f64() * 1000.0
            ));
            // A series that left the envelope is done; drop its host so the
            // ladder never builds the exponential tail.
            for (index, &host) in HOSTS.iter().enumerate() {
                if alive.contains(&host) && !within_envelope(&rows[index], depth) {
                    alive.retain(|alive| *alive != host);
                    let known = KNOWN_EXPONENTIAL_ROWS.contains(&(name.as_str(), host));
                    if !known {
                        failures.push(format!(
                            "{name}/{host}: rows {:?} leave 2*depth*rows(1) at depth {depth}",
                            rows[index]
                        ));
                    }
                }
            }
            let visits_out = !within_envelope(
                &visits
                    .iter()
                    .map(|&value| value as usize)
                    .collect::<Vec<_>>(),
                depth,
            );
            if visits_out && !KNOWN_EXPONENTIAL_VISITS.contains(&name.as_str()) {
                failures.push(format!(
                    "{name}: reachability visits {visits:?} leave 2*depth*visits(1) at depth {depth}"
                ));
            }
            if visits_out || alive.is_empty() {
                break;
            }
        }
        eprintln!("ladder {name}: {}", curve.join("; "));
        for (index, &host) in HOSTS.iter().enumerate() {
            let known = KNOWN_EXPONENTIAL_ROWS.contains(&(name.as_str(), host));
            let polynomial = rows[index].len() == deepest
                && (1..=deepest).all(|depth| within_envelope(&rows[index], depth));
            if known && polynomial {
                failures.push(format!(
                    "{name}/{host}: rows {:?} are near-linear now; remove it from KNOWN_EXPONENTIAL_ROWS",
                    rows[index]
                ));
            }
        }
        let visits_polynomial = visits.len() == deepest
            && (1..=deepest).all(|depth| {
                within_envelope(
                    &visits
                        .iter()
                        .map(|&value| value as usize)
                        .collect::<Vec<_>>(),
                    depth,
                )
            });
        if KNOWN_EXPONENTIAL_VISITS.contains(&name.as_str()) && visits_polynomial {
            failures.push(format!(
                "{name}: visits {visits:?} are near-linear now; remove it from KNOWN_EXPONENTIAL_VISITS"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
