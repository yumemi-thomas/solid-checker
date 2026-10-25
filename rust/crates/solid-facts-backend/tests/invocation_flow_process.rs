//! ADR 0319: the invocation flow reads other files' contents (callee bodies,
//! consumers, forwarded props, module/script status). A retained session must
//! answer exactly what a cold build answers after every edit, in both
//! directions; with the program boundary open the flow is off.
use std::{collections::BTreeMap, env, fs, path::PathBuf};

use crate::process_support::temporary_directory;
use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{NativeIncrementalSession, SourceChange, TypeFactsSession, dialect};

fn options(closed: bool) -> solid_reactive_ir::RuleOptions {
    let mut options = solid_reactive_ir::RuleOptions::default();
    options.runtime.program_boundary = Some(if closed {
        solid_reactive_ir::ProgramBoundary::Closed
    } else {
        solid_reactive_ir::ProgramBoundary::Open
    });
    options
}

pub(crate) fn cold(
    facts: &solid_facts::ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
    closed: bool,
) -> solid_reactive_ir::Program {
    solid_reactive_ir::build_with_analysis_inputs_measured(
        facts,
        dialect,
        solid_reactive_ir::AnalysisInputs::empty(),
        &options(closed),
    )
    .unwrap()
    .0
}

pub(crate) fn warm(
    incremental: &mut solid_reactive_ir::IncrementalBuilder,
    facts: &solid_facts::ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
    closed: bool,
) -> solid_reactive_ir::Program {
    (*incremental
        .build_with_analysis_inputs_shared(
            facts,
            dialect,
            solid_reactive_ir::AnalysisInputs::empty(),
            &options(closed),
        )
        .unwrap()
        .0)
        .clone()
}

/// A scratch closed application: the `invocation-flow` fixture's stubs, a
/// private manifest, and `files`.
pub(crate) fn project(label: &str, files: &[(&str, &str)]) -> (PathBuf, BTreeMap<String, String>) {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/invocation-flow");
    let directory = temporary_directory(label);
    for name in ["tsconfig.json", "solid-js.d.ts", "package.json"] {
        fs::copy(fixture.join(name), directory.join(name)).unwrap();
    }
    for (package, files) in [
        ("solid-js", &["package.json"][..]),
        ("@solidjs/signals", &["package.json", "index.d.ts"][..]),
        ("@solidjs/web", &["package.json"][..]),
    ] {
        let stub = directory.join("node_modules").join(package);
        fs::create_dir_all(&stub).unwrap();
        for name in files {
            fs::copy(
                fixture.join("node_modules").join(package).join(name),
                stub.join(name),
            )
            .unwrap();
        }
    }
    let mut paths = BTreeMap::new();
    for (name, text) in files {
        let path = directory.join(name);
        fs::write(&path, text).unwrap();
        paths.insert(
            (*name).to_owned(),
            path.canonicalize().unwrap().to_string_lossy().into_owned(),
        );
    }
    (directory, paths)
}

/// Whether `program` reports `SC1001` uncertifiable at the first `needle` in
/// `path` (the read inside the flowed callback).
fn read_open(
    findings: &[solid_reactive_ir::Finding],
    path: &str,
    source: &str,
    needle: &str,
) -> bool {
    finding_open(findings, path, source, needle, "SC1001")
}

/// Whether `findings` holds an uncertifiable `id` at the first `needle`.
pub(crate) fn finding_open(
    findings: &[solid_reactive_ir::Finding],
    path: &str,
    source: &str,
    needle: &str,
    id: &str,
) -> bool {
    let start = source.find(needle).unwrap() as u64;
    findings.iter().any(|finding| {
        finding.id == id
            && finding.kind == "uncertifiable"
            && finding.primary_location.path.as_ref() == path
            && finding.primary_location.start_byte == start
    })
}

/// Runs the edits on one retained session and checks each generation against
/// cold builds: closed (retained == cold, finding as `expected`) and open
/// (retained == cold, the read always uncertifiable: no flow).
fn twin(
    label: &str,
    files: &[(&str, &str)],
    read: (&str, &str),
    initially: bool,
    edits: &[(&str, &str, bool)],
) {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let (directory, paths) = project(label, files);
    let tsconfig = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let read_path = paths[read.0].clone();
    let mut read_source = files
        .iter()
        .find(|(name, _)| *name == read.0)
        .unwrap()
        .1
        .to_owned();
    let selected = dialect::default_dialect();
    let typescript = TypeFactsSession::open(&typefacts, &tsconfig, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(selected, tsconfig, typescript).unwrap();
    let mut closed_builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut open_builder = solid_reactive_ir::IncrementalBuilder::default();
    let check = |generation: &str,
                 read_source: &str,
                 facts: &solid_facts::ProjectFacts,
                 closed_builder: &mut solid_reactive_ir::IncrementalBuilder,
                 open_builder: &mut solid_reactive_ir::IncrementalBuilder,
                 expected: bool| {
        for (closed, builder) in [(true, closed_builder), (false, open_builder)] {
            let retained = warm(builder, facts, selected.vocabulary, closed);
            let fresh = cold(facts, selected.vocabulary, closed);
            assert_eq!(retained, fresh, "{label} {generation} closed={closed}");
            let retained_findings = selected.solve(&retained);
            assert_eq!(
                retained_findings,
                selected.solve(&fresh),
                "{label} {generation} closed={closed}: findings"
            );
            let open = read_open(&retained_findings, &read_path, read_source, read.1);
            assert_eq!(
                open,
                if closed { expected } else { true },
                "{label} {generation} closed={closed}: read finding"
            );
        }
    };
    let first = session.analyze().unwrap();
    check(
        "initial",
        &read_source,
        &first,
        &mut closed_builder,
        &mut open_builder,
        initially,
    );
    for (version, (name, text, expected)) in edits.iter().enumerate() {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: paths[*name].clone(),
                    version: version as u64 + 1,
                    source: Some((*text).into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        if *name == read.0 {
            (*text).clone_into(&mut read_source);
        }
        check(
            &format!("edit {} ({name})", version + 1),
            &read_source,
            &facts,
            &mut closed_builder,
            &mut open_builder,
            *expected,
        );
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

const RETAIN_APP: &str = r#"import { createSignal } from "solid-js";
import { retain } from "./lib";
export function App() {
  const [count] = createSignal(0);
  const run = retain({ onRun: () => console.log(count()) });
  return <button onClick={() => run()}>run</button>;
}
"#;
const RETAIN_LIB: &str = r#"export function retain(options: { onRun: () => void }) {
  return () => options.onRun();
}
"#;
const RETAIN_LIB_BODY: &str = r#"export function retain(options: { onRun: () => void }) {
  options.onRun();
  return () => options.onRun();
}
"#;

/// (1) The callee file starts invoking the callback in its body, and stops.
#[test]
fn callee_body_invocation_edits_match_cold() {
    twin(
        "flow-callee-edit",
        &[("App.tsx", RETAIN_APP), ("lib.ts", RETAIN_LIB)],
        ("App.tsx", "count()"),
        false,
        &[
            ("lib.ts", RETAIN_LIB_BODY, true),
            ("lib.ts", RETAIN_LIB, false),
        ],
    );
}

const SPREAD_APP: &str = r#"import { createSignal } from "solid-js";
import { rowProps } from "./rows";
import { Row } from "./Row";
export function App() {
  const [count] = createSignal(0);
  const handlers = rowProps(() => console.log(count()));
  return <Row handlers={handlers} />;
}
"#;
const SPREAD_ROWS: &str = r#"export function rowProps(callback: () => void) {
  return { onKeyDown: () => callback() };
}
"#;
const SPREAD_ROW: &str = r#"export function Row(props: { handlers: Record<string, unknown> }) {
  return <div {...props.handlers}>row</div>;
}
"#;
const SPREAD_ROW_BODY: &str = r#"export function Row(props: { handlers: Record<string, unknown> }) {
  const run = props.handlers.onKeyDown as () => void;
  run();
  return <div>row</div>;
}
"#;

/// (2) The consumer stops spreading the handlers onto an intrinsic and calls
/// one in its body, and goes back.
#[test]
fn consumer_spread_edits_match_cold() {
    twin(
        "flow-spread-edit",
        &[
            ("App.tsx", SPREAD_APP),
            ("rows.ts", SPREAD_ROWS),
            ("Row.tsx", SPREAD_ROW),
        ],
        ("App.tsx", "count()"),
        false,
        &[
            ("Row.tsx", SPREAD_ROW_BODY, true),
            ("Row.tsx", SPREAD_ROW, false),
        ],
    );
}

const TREE_APP: &str = r#"import { createSignal } from "solid-js";
import { Branch } from "./Branch";
export function App() {
  const [count] = createSignal(0);
  return <Branch a={() => console.log(count())} depth={2} />;
}
"#;
const TREE_BRANCH: &str = r#"type P = { a?: () => void; b?: () => void; depth: number };
export function Branch(props: P) {
  return (
    <div onClick={props.a}>
      {props.depth > 0 && <Branch a={props.a} depth={props.depth - 1} />}
    </div>
  );
}
"#;
const TREE_BRANCH_RENAMED: &str = r#"type P = { a?: () => void; b?: () => void; depth: number };
export function Branch(props: P) {
  const run = props.b;
  if (run) run();
  return (
    <div onClick={props.a}>
      {props.depth > 0 && <Branch b={props.a} depth={props.depth - 1} />}
    </div>
  );
}
"#;

/// (3) A recursive component's forwarded prop is renamed and the renamed
/// prop is called in the body, and back.
#[test]
fn forwarded_prop_cycle_edits_match_cold() {
    twin(
        "flow-cycle-edit",
        &[("App.tsx", TREE_APP), ("Branch.tsx", TREE_BRANCH)],
        ("App.tsx", "count()"),
        false,
        &[
            ("Branch.tsx", TREE_BRANCH_RENAMED, true),
            ("Branch.tsx", TREE_BRANCH, false),
        ],
    );
}

const MODULE_APP: &str = r#"import { createSignal } from "solid-js";
import { relay } from "./lib";
export function App() {
  const [count] = createSignal(0);
  const handlers = relay(() => console.log(count()));
  return <button onClick={handlers.onClick}>x</button>;
}
"#;
const MODULE_LIB: &str = r#"export function relay(callback: () => void) {
  return { onClick: () => callback(), later: () => fire(callback) };
}
function fire(run: () => void) {
  run();
}
"#;
const MODULE_LIB_GLOBAL: &str = r#"export function relay(callback: () => void) {
  return { onClick: () => callback(), later: () => fire(callback) };
}
function fire(run: () => void) {
  run();
}
(globalThis as unknown as { fire?: typeof fire }).fire = fire;
"#;
const SIDE_MODULE: &str = "export {};\n";
const SIDE_SCRIPT: &str = "function sideGlobal() {\n  return 1;\n}\n";
const SIDE_GLOBAL: &str = r#"import { relay } from "./lib";
(globalThis as unknown as { relay?: typeof relay }).relay = relay;
export {};
"#;

/// (4) A third file flips between script and module and attaches an export
/// to `globalThis`; the callee file attaches its unexported helper. Only the
/// last changes the answer: `fire` becomes callable from anywhere, so its
/// census, which found no caller, is no longer complete.
#[test]
fn script_module_flip_and_global_attachment_match_cold() {
    twin(
        "flow-script-flip",
        &[
            ("App.tsx", MODULE_APP),
            ("lib.ts", MODULE_LIB),
            ("side.ts", SIDE_MODULE),
        ],
        ("App.tsx", "count()"),
        false,
        &[
            ("side.ts", SIDE_SCRIPT, false),
            ("side.ts", SIDE_MODULE, false),
            ("side.ts", SIDE_GLOBAL, false),
            ("lib.ts", MODULE_LIB_GLOBAL, true),
            ("lib.ts", MODULE_LIB, false),
            ("side.ts", SIDE_MODULE, false),
        ],
    );
}

const KEY_APP: &str = r#"import { createSignal } from "solid-js";
import { retainLogged } from "./lib";
const noop = () => undefined;
export function App() {
  const [count] = createSignal(0);
  const run = retainLogged({ onRun: () => console.log(count()), log: noop });
  return <button onClick={() => run()}>run</button>;
}
"#;
const KEY_APP_QUOTED: &str = r#"import { createSignal } from "solid-js";
import { retainLogged } from "./lib";
const noop = () => undefined;
export function App() {
  const [count] = createSignal(0);
  const run = retainLogged({ "onRun": () => console.log(count()), "log": noop });
  return <button onClick={() => run()}>run</button>;
}
"#;
const KEY_APP_PROTO: &str = r#"import { createSignal } from "solid-js";
import { retainLogged } from "./lib";
const noop = () => undefined;
const logger = { log: noop };
export function App() {
  const [count] = createSignal(0);
  const run = retainLogged({ "onRun": () => console.log(count()), "__proto__": logger });
  return <button onClick={() => run()}>run</button>;
}
"#;
const KEY_LIB: &str = r#"export function retainLogged(options: { onRun: () => void; log?: () => void }) {
  return () => {
    options.log?.();
    options.onRun();
  };
}
"#;
const KEY_LIB_QUOTED_BODY: &str = r#"export function retainLogged(options: { onRun: () => void; log?: () => void }) {
  options["onRun"]();
  return () => {
    options.log?.();
    options.onRun();
  };
}
"#;

/// (5) The options literal's keys are quoted (same answer), its `log` is
/// replaced by a `__proto__` setter (the member is inherited: open), and the
/// callee calls the callback through a quoted computed member in its body.
#[test]
fn options_key_quoting_edits_match_cold() {
    twin(
        "flow-key-edit",
        &[("App.tsx", KEY_APP), ("lib.ts", KEY_LIB)],
        ("App.tsx", "count()"),
        false,
        &[
            ("App.tsx", KEY_APP_QUOTED, false),
            ("App.tsx", KEY_APP_PROTO, true),
            ("App.tsx", KEY_APP_QUOTED, false),
            ("lib.ts", KEY_LIB_QUOTED_BODY, true),
            ("lib.ts", KEY_LIB, false),
            ("App.tsx", KEY_APP, false),
        ],
    );
}

/// One retained builder asked under alternating runtime environments (open
/// and closed boundary, browser and node targets, CSR and SSR) across an edit
/// that moves the flow's answer: every answer equals a cold build under the
/// same environment, so no cached fragment carries one environment's flow
/// role into another's.
#[test]
fn alternating_runtime_environments_match_cold() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let (directory, paths) = project(
        "flow-runtime-alternation",
        &[("App.tsx", RETAIN_APP), ("lib.ts", RETAIN_LIB)],
    );
    let tsconfig = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let selected = dialect::default_dialect();
    let typescript = TypeFactsSession::open(&typefacts, &tsconfig, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(selected, tsconfig, typescript).unwrap();
    let environments = {
        use solid_reactive_ir::{ProgramBoundary, RuntimeRendering, RuntimeTarget};
        let mut list = Vec::new();
        for (boundary, target, rendering) in [
            (ProgramBoundary::Closed, None, None),
            (ProgramBoundary::Open, None, None),
            (
                ProgramBoundary::Closed,
                Some(RuntimeTarget::Browser),
                Some(RuntimeRendering::Csr),
            ),
            (
                ProgramBoundary::Closed,
                Some(RuntimeTarget::Node),
                Some(RuntimeRendering::StringSsr),
            ),
            (ProgramBoundary::Closed, None, None),
        ] {
            let mut options = solid_reactive_ir::RuleOptions::default();
            options.runtime.program_boundary = Some(boundary);
            options.runtime.target = target;
            options.runtime.rendering = rendering;
            list.push(options);
        }
        list
    };
    let mut builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut facts = session.analyze().unwrap();
    for (version, text) in [
        (0_u64, None),
        (1, Some(RETAIN_LIB_BODY)),
        (2, Some(RETAIN_LIB)),
    ] {
        if let Some(text) = text {
            facts = session
                .edit(
                    vec![SourceChange {
                        path: paths["lib.ts"].clone(),
                        version,
                        source: Some(text.into()),
                        compiler_options: CompilerOptions::default(),
                    }],
                    None,
                )
                .unwrap();
        }
        for (index, options) in environments.iter().enumerate() {
            let retained = (*builder
                .build_with_analysis_inputs_shared(
                    &facts,
                    selected.vocabulary,
                    solid_reactive_ir::AnalysisInputs::empty(),
                    options,
                )
                .unwrap()
                .0)
                .clone();
            let fresh = solid_reactive_ir::build_with_analysis_inputs_measured(
                &facts,
                selected.vocabulary,
                solid_reactive_ir::AnalysisInputs::empty(),
                options,
            )
            .unwrap()
            .0;
            assert_eq!(retained, fresh, "generation {version}, environment {index}");
            assert_eq!(
                selected.solve(&retained),
                selected.solve(&fresh),
                "generation {version}, environment {index}: findings"
            );
        }
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

/// One CLI check, through the daemon or one-shot; its findings' JSON and
/// whether the daemon answered from its cached snapshot.
fn cli_check(
    typefacts: &str,
    tsconfig: &std::path::Path,
    sockets: &std::path::Path,
    retained: bool,
) -> (serde_json::Value, Option<bool>) {
    cli_check_retaining(typefacts, tsconfig, sockets, retained, "performance")
}

/// [`cli_check`] with the daemon's idle cache retention selected.
pub(crate) fn cli_check_retaining(
    typefacts: &str,
    tsconfig: &std::path::Path,
    sockets: &std::path::Path,
    retained: bool,
    retention: &str,
) -> (serde_json::Value, Option<bool>) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_CACHE_RETENTION", retention)
        .env("SOLID_CHECKER_DAEMON", if retained { "1" } else { "0" })
        .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "10")
        .env("SOLID_CHECKER_TIMINGS", "1")
        .env("TMPDIR", sockets)
        .args(["--format", "json", "--project"])
        .arg(tsconfig)
        .args([
            "--runtime-target",
            "browser",
            "--runtime-build",
            "development",
            "--rendering",
            "csr",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let snapshot: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("{error}: {stderr}"));
    let hit = stderr.lines().find_map(|line| {
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        (value["mode"] == "retained-daemon").then(|| value["cacheHit"] == true)
    });
    (snapshot["findings"].clone(), hit)
}

/// ADR 0193's boundary is read from the nearest manifest, which is no
/// program source. A retained daemon check with a declared host (so no host
/// inference inputs) must still notice the manifest flipping the program
/// between closed and open, which turns the flow on and off; and it must
/// follow a callee edit like a one-shot check.
#[test]
fn daemon_follows_the_boundary_manifest() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let (directory, _) = project(
        "flow-daemon-boundary",
        &[("App.tsx", RETAIN_APP), ("lib.ts", RETAIN_LIB)],
    );
    let directory = directory.canonicalize().unwrap();
    let tsconfig = directory.join("tsconfig.json");
    let sockets = PathBuf::from(format!("/tmp/scf-{}", std::process::id()));
    let _ = fs::remove_dir_all(&sockets);
    fs::create_dir_all(&sockets).unwrap();
    let manifest = directory.join("package.json");
    let private = fs::read_to_string(&manifest).unwrap();
    let library = r#"{"name":"invocation-flow","main":"index.js"}"#;
    let reads = |findings: &serde_json::Value| {
        findings
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["id"] == "SC1001")
            .count()
    };
    let (closed, _) = cli_check(&typefacts, &tsconfig, &sockets, true);
    let (repeat, hit) = cli_check(&typefacts, &tsconfig, &sockets, true);
    assert_eq!(repeat, closed);
    assert_eq!(hit, Some(true), "the repeat is a cached answer");
    assert_eq!(reads(&closed), 0, "closed: the flow proves the read");
    for (text, open) in [(library, true), (private.as_str(), false)] {
        fs::write(&manifest, text).unwrap();
        let (retained, _) = cli_check(&typefacts, &tsconfig, &sockets, true);
        let (fresh, _) = cli_check(&typefacts, &tsconfig, &sockets, false);
        assert_eq!(retained, fresh, "manifest open={open}");
        assert_eq!(reads(&fresh) > 0, open, "manifest open={open}");
    }
    // The same daemon across a callee edit and back.
    let lib = directory.join("lib.ts");
    for (text, open) in [(RETAIN_LIB_BODY, true), (RETAIN_LIB, false)] {
        fs::write(&lib, text).unwrap();
        let (retained, _) = cli_check(&typefacts, &tsconfig, &sockets, true);
        let (fresh, _) = cli_check(&typefacts, &tsconfig, &sockets, false);
        assert_eq!(retained, fresh, "callee edit open={open}");
        assert_eq!(reads(&fresh) > 0, open, "callee edit open={open}");
    }
    fs::remove_dir_all(&directory).unwrap();
    let _ = fs::remove_dir_all(&sockets);
}

const OTHER: &str = r#"import { createSignal } from "solid-js";
export function Other() {
  const [count] = createSignal(0);
  console.log(count());
  return <p />;
}
"#;

/// A retained daemon check must follow an edit to a config the project's
/// tsconfig `extends`: here the base config's `include` adds and drops a file
/// holding a strict-read violation.
#[test]
fn daemon_follows_an_extended_tsconfig() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let (directory, _) = project(
        "daemon-extends",
        &[
            ("App.tsx", RETAIN_APP),
            ("lib.ts", RETAIN_LIB),
            ("Other.tsx", OTHER),
        ],
    );
    let directory = directory.canonicalize().unwrap();
    let tsconfig = directory.join("tsconfig.json");
    let base = directory.join("base.json");
    let options = r#""compilerOptions": {"jsx": "preserve", "module": "ESNext", "moduleResolution": "Bundler", "strict": true, "target": "ES2022"}"#;
    let narrow = format!(r#"{{ {options}, "include": ["App.tsx", "lib.ts", "*.d.ts"] }}"#);
    let wide = format!(r#"{{ {options}, "include": ["*.ts", "*.tsx"] }}"#);
    fs::write(&tsconfig, r#"{ "extends": "./base" }"#).unwrap();
    fs::write(&base, &narrow).unwrap();
    let sockets = PathBuf::from(format!("/tmp/sce-{}", std::process::id()));
    let _ = fs::remove_dir_all(&sockets);
    fs::create_dir_all(&sockets).unwrap();
    let violations = |findings: &serde_json::Value| {
        findings
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["id"] == "SC1001" && finding["kind"] == "violation")
            .count()
    };
    let (first, _) = cli_check(&typefacts, &tsconfig, &sockets, true);
    let (repeat, hit) = cli_check(&typefacts, &tsconfig, &sockets, true);
    assert_eq!(repeat, first);
    assert_eq!(hit, Some(true), "the repeat is a cached answer");
    assert_eq!(violations(&first), 0, "Other.tsx is outside the program");
    for (text, inside) in [(&wide, true), (&narrow, false), (&wide, true)] {
        fs::write(&base, text).unwrap();
        let (retained, _) = cli_check(&typefacts, &tsconfig, &sockets, true);
        let (fresh, _) = cli_check(&typefacts, &tsconfig, &sockets, false);
        assert_eq!(retained, fresh, "base include wide={inside}");
        assert_eq!(violations(&fresh) > 0, inside, "base include wide={inside}");
    }
    fs::remove_dir_all(&directory).unwrap();
    let _ = fs::remove_dir_all(&sockets);
}

/// Review round 1 of the extends tracking: a package config's resolution
/// changes with no change to any file the chain names. A pnpm-style symlink
/// is retargeted, the package's `tsconfig` field moves, and a nearer copy of
/// the package appears; each time the retained answer must be the fresh one.
#[test]
fn daemon_follows_a_package_extends_resolution() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let (root, _) = project("daemon-extends-package", &[]);
    let root = root.canonicalize().unwrap();
    let app = root.join("app");
    fs::create_dir_all(&app).unwrap();
    for (name, text) in [
        ("App.tsx", RETAIN_APP),
        ("lib.ts", RETAIN_LIB),
        ("Other.tsx", OTHER),
    ] {
        fs::write(app.join(name), text).unwrap();
    }
    fs::copy(root.join("solid-js.d.ts"), app.join("solid-js.d.ts")).unwrap();
    fs::copy(root.join("package.json"), app.join("package.json")).unwrap();
    let options = r#""compilerOptions": {"jsx": "preserve", "module": "ESNext", "moduleResolution": "Bundler", "strict": true, "target": "ES2022"}"#;
    let narrow = format!(
        r#"{{ {options}, "include": ["${{configDir}}/App.tsx", "${{configDir}}/lib.ts", "${{configDir}}/*.d.ts"] }}"#
    );
    let wide =
        format!(r#"{{ {options}, "include": ["${{configDir}}/*.ts", "${{configDir}}/*.tsx"] }}"#);
    let store = root.join("store");
    for (version, text) in [("v1", &narrow), ("v2", &wide)] {
        fs::create_dir_all(store.join(version)).unwrap();
        fs::write(store.join(version).join("tsconfig.json"), text).unwrap();
        fs::write(store.join(version).join("narrow.json"), &narrow).unwrap();
        fs::write(store.join(version).join("wide.json"), &wide).unwrap();
        fs::write(
            store.join(version).join("package.json"),
            r#"{"name":"@cfg/base"}"#,
        )
        .unwrap();
    }
    let scope = root.join("node_modules").join("@cfg");
    fs::create_dir_all(&scope).unwrap();
    let link = scope.join("base");
    std::os::unix::fs::symlink(store.join("v1"), &link).unwrap();
    let tsconfig = app.join("tsconfig.json");
    fs::write(&tsconfig, r#"{ "extends": "@cfg/base" }"#).unwrap();
    let sockets = PathBuf::from(format!("/tmp/scp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&sockets);
    fs::create_dir_all(&sockets).unwrap();
    let violations = |findings: &serde_json::Value| {
        findings
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["id"] == "SC1001" && finding["kind"] == "violation")
            .count()
    };
    let check = |label: &str, inside: bool| {
        let (retained, _) = cli_check(&typefacts, &tsconfig, &sockets, true);
        let (fresh, _) = cli_check(&typefacts, &tsconfig, &sockets, false);
        assert_eq!(retained, fresh, "{label}");
        assert_eq!(violations(&fresh) > 0, inside, "{label}");
    };
    check("narrow v1", false);
    let (repeat, hit) = cli_check(&typefacts, &tsconfig, &sockets, true);
    assert_eq!(hit, Some(true), "the repeat is a cached answer");
    assert_eq!(violations(&repeat), 0);
    // A retargeted symlink, both ways.
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(store.join("v2"), &link).unwrap();
    check("relinked to v2", true);
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(store.join("v1"), &link).unwrap();
    check("relinked to v1", false);
    // The package's `tsconfig` field moves, both ways.
    let manifest = store.join("v1").join("package.json");
    fs::write(&manifest, r#"{"name":"@cfg/base","tsconfig":"wide.json"}"#).unwrap();
    check("tsconfig field wide", true);
    fs::write(
        &manifest,
        r#"{"name":"@cfg/base","tsconfig":"narrow.json"}"#,
    )
    .unwrap();
    check("tsconfig field narrow", false);
    // A nearer copy appears, and goes away.
    let nearer = app.join("node_modules").join("@cfg").join("base");
    fs::create_dir_all(&nearer).unwrap();
    fs::write(nearer.join("tsconfig.json"), &wide).unwrap();
    fs::write(nearer.join("package.json"), r#"{"name":"@cfg/base"}"#).unwrap();
    check("nearer copy", true);
    fs::remove_dir_all(app.join("node_modules")).unwrap();
    check("nearer copy removed", false);
    fs::remove_dir_all(&root).unwrap();
    let _ = fs::remove_dir_all(&sockets);
}

// ---- Module resolution under a retained daemon (review round 5).

const WS_APP: &str = r#"import { createSignal } from "solid-js";
import { keep } from "@ws/lib";
export function App() {
  const [count] = createSignal(0);
  const run = keep(() => console.log(count()));
  return <button onClick={run}>x</button>;
}
"#;
/// `keep` runs the callback only from the returned click handler.
const WS_EVENT: &str =
    "export function keep(callback: () => void) {\n  return () => callback();\n}\n";
/// `keep` also runs it in the caller's body.
const WS_BODY: &str =
    "export function keep(callback: () => void) {\n  callback();\n  return () => callback();\n}\n";

fn write(path: &std::path::Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn relink(link: &std::path::Path, target: &std::path::Path) {
    let _ = fs::remove_file(link);
    let _ = fs::remove_dir_all(link);
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(target, link).unwrap();
}

/// A package directory holding both implementations and `manifest`.
fn package(directory: &std::path::Path, manifest: &str) {
    write(&directory.join("event.ts"), WS_EVENT);
    write(&directory.join("body.ts"), WS_BODY);
    write(&directory.join("package.json"), manifest);
}

type Step = (&'static str, Box<dyn Fn(&std::path::Path)>, bool);

/// The scratch app, `setup`, then each step: a retained daemon check equals a
/// one-shot check, and the read is reported exactly when the step says the
/// program now resolves `@ws/lib` to the body-calling `keep`. Run once per
/// idle cache retention level, each with its own daemon.
fn resolution_case(label: &str, setup: impl Fn(&std::path::Path), steps: Vec<Step>) {
    for retention in ["performance", "compact"] {
        resolution_case_retaining(label, retention, &setup, &steps);
    }
}

fn resolution_case_retaining(
    label: &str,
    retention: &str,
    setup: &impl Fn(&std::path::Path),
    steps: &[Step],
) {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let label = &format!("{label}-{retention}");
    let (directory, _) = project(label, &[("App.tsx", WS_APP)]);
    let directory = directory.canonicalize().unwrap();
    let tsconfig = directory.join("tsconfig.json");
    let sockets = PathBuf::from(format!("/tmp/scr-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&sockets);
    fs::create_dir_all(&sockets).unwrap();
    setup(&directory);
    let reads = |findings: &serde_json::Value| {
        findings
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["id"] == "SC1001")
    };
    for (name, step, body) in steps {
        step(&directory);
        let check =
            |retained| cli_check_retaining(&typefacts, &tsconfig, &sockets, retained, retention);
        let (retained, _) = check(true);
        let (fresh, _) = check(false);
        assert_eq!(
            retained, fresh,
            "{label}: {name}: retained and one-shot differ"
        );
        assert_eq!(reads(&fresh), *body, "{label}: {name}");
        let (repeat, hit) = check(true);
        assert_eq!(repeat, fresh, "{label}: {name}: repeat");
        assert_eq!(
            hit,
            Some(true),
            "{label}: {name}: an unchanged repeat is cached"
        );
    }
    fs::remove_dir_all(&directory).unwrap();
    let _ = fs::remove_dir_all(&sockets);
}

fn no_op() -> Box<dyn Fn(&std::path::Path)> {
    Box::new(|_| {})
}

#[test]
fn a_relinked_workspace_package_is_resolved_again() {
    let manifest = r#"{"name":"@ws/lib","private":true,"types":"index.ts"}"#;
    resolution_case(
        "ws-relink",
        |root| {
            write(&root.join("packages/one/index.ts"), WS_EVENT);
            write(&root.join("packages/one/package.json"), manifest);
            write(&root.join("packages/two/index.ts"), WS_BODY);
            write(&root.join("packages/two/package.json"), manifest);
            relink(
                &root.join("node_modules/@ws/lib"),
                std::path::Path::new("../../packages/one"),
            );
        },
        vec![
            ("linked to the event package", no_op(), false),
            (
                "relinked to the body package",
                Box::new(|root| {
                    relink(
                        &root.join("node_modules/@ws/lib"),
                        std::path::Path::new("../../packages/two"),
                    );
                }),
                true,
            ),
            (
                "relinked back",
                Box::new(|root| {
                    relink(
                        &root.join("node_modules/@ws/lib"),
                        std::path::Path::new("../../packages/one"),
                    );
                }),
                false,
            ),
        ],
    );
}

#[test]
fn a_redirected_types_main_or_exports_field_is_resolved_again() {
    let lib = |root: &std::path::Path| root.join("node_modules/@ws/lib");
    resolution_case(
        "ws-manifest",
        move |root| {
            package(
                &lib(root),
                r#"{"name":"@ws/lib","private":true,"types":"event.ts"}"#,
            );
        },
        vec![
            ("types: event", no_op(), false),
            (
                "types: body",
                Box::new(move |root| {
                    write(
                        &lib(root).join("package.json"),
                        r#"{"name":"@ws/lib","private":true,"types":"body.ts"}"#,
                    );
                }),
                true,
            ),
            (
                "exports: event",
                Box::new(move |root| {
                    write(
                        &lib(root).join("package.json"),
                        r#"{"name":"@ws/lib","private":true,"exports":{".":{"types":"./event.ts"}}}"#,
                    );
                }),
                false,
            ),
            (
                "exports: body",
                Box::new(move |root| {
                    write(
                        &lib(root).join("package.json"),
                        r#"{"name":"@ws/lib","private":true,"exports":{".":{"types":"./body.ts"}}}"#,
                    );
                }),
                true,
            ),
            (
                "main: event",
                Box::new(move |root| {
                    write(
                        &lib(root).join("package.json"),
                        r#"{"name":"@ws/lib","private":true,"main":"./event.ts"}"#,
                    );
                }),
                false,
            ),
            (
                "main: body",
                Box::new(move |root| {
                    write(
                        &lib(root).join("package.json"),
                        r#"{"name":"@ws/lib","private":true,"main":"./body.ts"}"#,
                    );
                }),
                true,
            ),
        ],
    );
}

#[test]
fn a_package_reinstalled_at_another_version_is_resolved_again() {
    let lib = |root: &std::path::Path| root.join("node_modules/@ws/lib");
    resolution_case(
        "ws-reinstall",
        move |root| {
            write(&lib(root).join("index.ts"), WS_EVENT);
            write(
                &lib(root).join("package.json"),
                r#"{"name":"@ws/lib","version":"1.0.0","types":"index.ts"}"#,
            );
        },
        vec![
            ("version 1", no_op(), false),
            (
                "removed and reinstalled at version 2",
                Box::new(move |root| {
                    fs::remove_dir_all(lib(root)).unwrap();
                    write(&lib(root).join("index.ts"), WS_BODY);
                    write(
                        &lib(root).join("package.json"),
                        r#"{"name":"@ws/lib","version":"2.0.0","types":"index.ts"}"#,
                    );
                }),
                true,
            ),
            (
                "the installed file edited in place",
                Box::new(move |root| write(&lib(root).join("index.ts"), WS_EVENT)),
                false,
            ),
        ],
    );
}

#[test]
fn a_relinked_pnpm_store_entry_is_resolved_again() {
    // node_modules/@ws/lib -> .pnpm/@ws+lib@1/node_modules/@ws/lib, which is
    // itself a link into a content store, as pnpm lays out an install.
    let manifest = r#"{"name":"@ws/lib","version":"1.0.0","types":"index.ts"}"#;
    resolution_case(
        "ws-pnpm",
        move |root| {
            write(&root.join("store/a/index.ts"), WS_EVENT);
            write(&root.join("store/a/package.json"), manifest);
            write(&root.join("store/b/index.ts"), WS_BODY);
            write(&root.join("store/b/package.json"), manifest);
            relink(
                &root.join("node_modules/.pnpm/@ws+lib@1/node_modules/@ws/lib"),
                &root.join("store/a"),
            );
            relink(
                &root.join("node_modules/@ws/lib"),
                std::path::Path::new("../.pnpm/@ws+lib@1/node_modules/@ws/lib"),
            );
        },
        vec![
            ("store a", no_op(), false),
            (
                "the inner store link moved to b",
                Box::new(|root| {
                    relink(
                        &root.join("node_modules/.pnpm/@ws+lib@1/node_modules/@ws/lib"),
                        &root.join("store/b"),
                    );
                }),
                true,
            ),
            (
                "moved back",
                Box::new(|root| {
                    relink(
                        &root.join("node_modules/.pnpm/@ws+lib@1/node_modules/@ws/lib"),
                        &root.join("store/a"),
                    );
                }),
                false,
            ),
        ],
    );
}

// ---- ADR 0320: member resolution across a retained session.

/// Like [`twin`] for an `SC9012` dispatch obligation at `needle`, under both
/// boundaries (member resolution does not depend on it).
fn member_twin(
    label: &str,
    files: &[(&str, &str)],
    site: (&str, &str),
    initially: bool,
    edits: &[(&str, &str, bool)],
) {
    member_twin_with(label, files, site, initially, edits, false);
}

/// [`member_twin`], where an open program keeps the obligation in every
/// generation: the discharge it checks needs the closed program's census.
fn member_twin_with(
    label: &str,
    files: &[(&str, &str)],
    site: (&str, &str),
    initially: bool,
    edits: &[(&str, &str, bool)],
    open_keeps: bool,
) {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let (directory, paths) = project(label, files);
    let tsconfig = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let site_path = paths[site.0].clone();
    let mut site_source = files
        .iter()
        .find(|(name, _)| *name == site.0)
        .unwrap()
        .1
        .to_owned();
    let selected = dialect::default_dialect();
    let typescript = TypeFactsSession::open(&typefacts, &tsconfig, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(selected, tsconfig, typescript).unwrap();
    let mut builders = [
        solid_reactive_ir::IncrementalBuilder::default(),
        solid_reactive_ir::IncrementalBuilder::default(),
    ];
    let mut check =
        |generation: &str, source: &str, facts: &solid_facts::ProjectFacts, expected: bool| {
            for (closed, builder) in [true, false].into_iter().zip(builders.iter_mut()) {
                let retained = warm(builder, facts, selected.vocabulary, closed);
                let fresh = cold(facts, selected.vocabulary, closed);
                assert_eq!(retained, fresh, "{label} {generation} closed={closed}");
                let findings = selected.solve(&retained);
                assert_eq!(
                    findings,
                    selected.solve(&fresh),
                    "{label} {generation}: findings"
                );
                assert_eq!(
                    finding_open(&findings, &site_path, source, site.1, "SC9012"),
                    expected || (open_keeps && !closed),
                    "{label} {generation} closed={closed}: dispatch obligation"
                );
            }
        };
    let first = session.analyze().unwrap();
    check("initial", &site_source, &first, initially);
    for (version, (name, text, expected)) in edits.iter().enumerate() {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: paths[*name].clone(),
                    version: version as u64 + 1,
                    source: Some((*text).into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        if *name == site.0 {
            (*text).clone_into(&mut site_source);
        }
        check(
            &format!("edit {} ({name})", version + 1),
            &site_source,
            &facts,
            *expected,
        );
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

const MEMBER_APP: &str = r#"import { helper } from "./helper";
export function App() {
  helper({ run() {} });
  return <main />;
}
"#;
const MEMBER_HELPER: &str = r#"export function helper(o: { run: () => void }) {
  o.run();
}
"#;
const MEMBER_HELPER_LEAKS: &str = r#"declare function mutate(target: object): void;
export function helper(o: { run: () => void }) {
  mutate(o);
  o.run();
}
"#;
const MEMBER_THIRD: &str = "export {};\n";
const MEMBER_THIRD_COPIES: &str =
    "export function copy(target: object, source: object) {\n  Object.assign(target, source);\n}\n";
const MEMBER_THIRD_WRITES: &str =
    "export function write(target: { run: () => void }) {\n  target.run = () => undefined;\n}\n";

/// The callee starts handing its parameter to unknown code, and stops; a
/// third file starts copying keys into objects, or assigning `run`, and
/// stops. The literal's `run` is trusted exactly while the callee keeps it
/// closed: neither third-file edit can reach a literal written as the argument
/// (ADR 0321).
#[test]
fn member_resolution_edits_match_cold() {
    member_twin(
        "member-value-edit",
        &[
            ("App.tsx", MEMBER_APP),
            ("helper.ts", MEMBER_HELPER),
            ("third.ts", MEMBER_THIRD),
        ],
        ("App.tsx", "{ run() {} }"),
        false,
        &[
            ("helper.ts", MEMBER_HELPER_LEAKS, true),
            ("helper.ts", MEMBER_HELPER, false),
            ("third.ts", MEMBER_THIRD_COPIES, false),
            // ADR 0321: a write of `run` elsewhere cannot reach a literal
            // only the closed callee's parameter holds.
            ("third.ts", MEMBER_THIRD_WRITES, false),
            ("third.ts", MEMBER_THIRD, false),
        ],
    );
}

const WITHHELD_APP: &str = r#"import "./mutate";
import { api } from "./api";
export function App() {
  const value = api.read();
  return <main>{value}</main>;
}
"#;
const WITHHELD_API: &str = "export const api = {\n  read() {\n    return 1;\n  },\n};\n";
const WITHHELD_MUTATE: &str = "import { api } from \"./api\";\napi.read = () => 2;\n";
const WITHHELD_INERT: &str = "export {};\n";

/// A module starts and stops assigning the literal's member: the call in an
/// unchanged file is a dispatch obligation exactly while the assignment
/// exists.
#[test]
fn withheld_member_edits_match_cold() {
    member_twin(
        "withheld-member-edit",
        &[
            ("App.tsx", WITHHELD_APP),
            ("api.ts", WITHHELD_API),
            ("mutate.ts", WITHHELD_INERT),
        ],
        ("App.tsx", "api.read()"),
        false,
        &[
            ("mutate.ts", WITHHELD_MUTATE, true),
            ("mutate.ts", WITHHELD_INERT, false),
        ],
    );
}

const FORMS_APP: &str = r#"import { forward } from "./forward";
declare const opaque: { run: () => void };
export function App() {
  forward(opaque);
  return <main />;
}
"#;
const FORMS_HELPER: &str = "export function helper(o: { run: () => void }) {\n  o.run();\n}\n";
const FORMS_DIRECT: &str = "import { helper } from \"./helper\";\nexport function forward(o: { run: () => void }) {\n  helper(o);\n}\n";
const FORMS_ALIAS: &str = "import { helper } from \"./helper\";\nexport function forward(o: { run: () => void }) {\n  const alias = o;\n  helper(alias);\n}\n";
const FORMS_TEST: &str =
    "export function forward(o: { run: () => void }) {\n  return typeof o === \"object\";\n}\n";

/// The forwarding function changes how it uses its parameter: forwarded
/// whole, through an alias, or only tested. The obligation at the
/// unchanged caller follows.
#[test]
fn parameter_use_edits_match_cold() {
    member_twin(
        "parameter-use-edit",
        &[
            ("App.tsx", FORMS_APP),
            ("helper.ts", FORMS_HELPER),
            ("forward.ts", FORMS_DIRECT),
        ],
        ("App.tsx", "opaque)"),
        true,
        &[
            ("forward.ts", FORMS_ALIAS, true),
            ("forward.ts", FORMS_TEST, false),
            ("forward.ts", FORMS_DIRECT, true),
        ],
    );
}

const NAMED_EVENT_ONLY: &str = r#"import { helper } from "./helper";
declare const opaque: { run: () => void };
export function App() {
  function press() {
    helper(opaque);
  }
  return <button onClick={press} />;
}
"#;
const NAMED_ALSO_CALLED: &str = r#"import { helper } from "./helper";
declare const opaque: { run: () => void };
export function App() {
  function press() {
    helper(opaque);
  }
  press();
  return <button onClick={press} />;
}
"#;

/// ADR 0320: a function named at an event position lends that role to its
/// body only while no direct call runs it too. Adding and removing the
/// direct call moves the obligation in a retained session as in a cold one.
#[test]
fn named_callback_census_edits_match_cold() {
    member_twin(
        "named-callback-census-edit",
        &[("App.tsx", NAMED_EVENT_ONLY), ("helper.ts", FORMS_HELPER)],
        ("App.tsx", "opaque)"),
        false,
        &[
            ("App.tsx", NAMED_ALSO_CALLED, true),
            ("App.tsx", NAMED_EVENT_ONLY, false),
        ],
    );
}

const AWAIT_APP: &str = r#"import { forward } from "./forward";
declare const opaque: { run: () => void };
export function App() {
  void forward(opaque);
  return <main />;
}
"#;
const AWAIT_UNCONDITIONAL: &str = "import { helper } from \"./helper\";\nexport async function forward(o: { run: () => void }) {\n  await 0;\n  helper(o);\n}\n";
const AWAIT_CONDITIONAL: &str = "import { helper } from \"./helper\";\ndeclare const flag: boolean;\nexport async function forward(o: { run: () => void }) {\n  if (flag) await 0;\n  helper(o);\n}\n";

const AWAIT_LOGICAL_ASSIGNMENT: &str = "import { helper } from \"./helper\";\ndeclare let seed: number | undefined;\nexport async function forward(o: { run: () => void }) {\n  seed ??= await Promise.resolve(1);\n  helper(o);\n}\n";
const AWAIT_LABELLED: &str = "import { helper } from \"./helper\";\ndeclare const flag: boolean;\nexport async function forward(o: { run: () => void }) {\n  skip: {\n    if (flag) break skip;\n    await 0;\n  }\n  helper(o);\n}\n";

/// ADR 0320: a forwarding after an await reached on every path decides
/// nothing at the caller; one after a skippable await does. The edit is in
/// another file than the obligation.
#[test]
fn await_continuation_edits_match_cold() {
    member_twin(
        "await-continuation-edit",
        &[
            ("App.tsx", AWAIT_APP),
            ("helper.ts", FORMS_HELPER),
            ("forward.ts", AWAIT_UNCONDITIONAL),
        ],
        ("App.tsx", "opaque)"),
        false,
        &[
            ("forward.ts", AWAIT_CONDITIONAL, true),
            ("forward.ts", AWAIT_UNCONDITIONAL, false),
            ("forward.ts", AWAIT_LOGICAL_ASSIGNMENT, true),
            ("forward.ts", AWAIT_LABELLED, true),
            ("forward.ts", AWAIT_UNCONDITIONAL, false),
        ],
    );
}

const DESTRUCTURED_APP: &str = r#"import { forward } from "./forward";
declare const opaque: { run: () => void };
export function App() {
  forward({ inner: opaque });
  return <main />;
}
"#;
const DESTRUCTURED_CALL: &str =
    "export function forward({ inner }: { inner: { run: () => void } }) {\n  inner.run();\n}\n";
const DESTRUCTURED_DATA: &str = "export function forward({ inner }: { inner: { run: () => void } }) {\n  return typeof inner;\n}\n";
const DESTRUCTURED_BODY: &str = "export function forward(box: { inner: { run: () => void } }) {\n  const { inner } = box;\n  inner.run();\n}\n";

/// ADR 0320: a member called on a local destructured from the parameter is
/// an obligation at the caller in another file, in both directions.
#[test]
fn destructured_parameter_edits_match_cold() {
    member_twin(
        "destructured-parameter-edit",
        &[
            ("App.tsx", DESTRUCTURED_APP),
            ("forward.ts", DESTRUCTURED_CALL),
        ],
        ("App.tsx", "{ inner: opaque }"),
        true,
        &[
            ("forward.ts", DESTRUCTURED_DATA, false),
            ("forward.ts", DESTRUCTURED_BODY, true),
            ("forward.ts", DESTRUCTURED_DATA, false),
            ("forward.ts", DESTRUCTURED_CALL, true),
        ],
    );
}

const SIGNAL_PARENT_PRIMITIVE: &str = r#"import { createSignal } from "solid-js";
import { Child } from "./Child";
export function Parent() {
  const [value, setValue] = createSignal<number | { run(): void }>(0);
  setValue(1);
  return <Child value={value()} />;
}
"#;
const SIGNAL_PARENT_OBJECT: &str = r#"import { createSignal } from "solid-js";
import { Child } from "./Child";
declare const opaque: { run(): void };
export function Parent() {
  const [value, setValue] = createSignal<number | { run(): void }>(0);
  setValue(opaque);
  return <Child value={value()} />;
}
"#;
const SIGNAL_CHILD: &str = r#"import { forward } from "./forward";
export function Child(props: { value: number | { run(): void } }) {
  forward(props.value);
  return <main />;
}
"#;
const SIGNAL_FORWARD: &str = "declare function sink(value: unknown): void;\nexport function forward(value: unknown) {\n  sink({ value });\n}\n";

/// ADR 0320: a prop that every tag passes from a signal every write of which
/// is a proven primitive settles the forwarded use in the child. Writing an
/// object through the setter in the parent's file brings the child's
/// obligation back, in a retained session as in a cold one.
#[test]
fn signal_writer_edits_match_cold() {
    member_twin_with(
        "signal-writer-edit",
        &[
            ("Parent.tsx", SIGNAL_PARENT_PRIMITIVE),
            ("Child.tsx", SIGNAL_CHILD),
            ("forward.ts", SIGNAL_FORWARD),
        ],
        ("Child.tsx", "props.value)"),
        false,
        &[
            ("Parent.tsx", SIGNAL_PARENT_OBJECT, true),
            ("Parent.tsx", SIGNAL_PARENT_PRIMITIVE, false),
        ],
        true,
    );
}

const LITERAL_APP_INERT: &str = r#"import { useThing } from "./helper";
export function App() {
  useThing({ onClose: () => undefined, label: "a" });
  return <main />;
}
"#;
const LITERAL_APP_SPREAD: &str = r#"import { useThing } from "./helper";
declare const extra: { onClose?: () => void };
export function App() {
  useThing({ onClose: () => undefined, label: "a", ...extra });
  return <main />;
}
"#;
const LITERAL_HELPER_CLOSED: &str = "export function useThing(options: { onClose: () => void; label: string }) {\n  options.onClose();\n}\n";
const LITERAL_HELPER_HANDS_ON: &str = "declare function sink(value: unknown): void;\nexport function useThing(options: { onClose: () => void; label: string }) {\n  options.onClose();\n  sink(options);\n}\n";

/// ADR 0321: an argument literal's arrow property is the dispatch target
/// while the literal is fresh and the callee closed. A later spread in the
/// caller, or a callee that hands the object on (another file), brings the
/// obligation back, in a retained session as in a cold one.
#[test]
fn literal_property_dispatch_edits_match_cold() {
    member_twin(
        "literal-property-dispatch-edit",
        &[
            ("App.tsx", LITERAL_APP_INERT),
            ("helper.ts", LITERAL_HELPER_CLOSED),
        ],
        ("App.tsx", "{ onClose: () => undefined, label"),
        false,
        &[
            ("App.tsx", LITERAL_APP_SPREAD, true),
            ("App.tsx", LITERAL_APP_INERT, false),
            ("helper.ts", LITERAL_HELPER_HANDS_ON, true),
            ("helper.ts", LITERAL_HELPER_CLOSED, false),
        ],
    );
}

/// The kind of the `SC2001` finding at the first `needle` in `path`:
/// `"violation"`, `"uncertifiable"`, or `"clean"` with none.
fn write_kind(
    findings: &[solid_reactive_ir::Finding],
    path: &str,
    source: &str,
    needle: &str,
) -> String {
    let start = source.find(needle).unwrap() as u64;
    findings
        .iter()
        .find(|finding| {
            finding.id == "SC2001"
                && finding.primary_location.path.as_ref() == path
                && finding.primary_location.start_byte == start
        })
        .map_or_else(|| "clean".to_owned(), |finding| finding.kind.to_string())
}

/// ADR 0400: a write in a function handed to project code takes the role of the
/// callee's invocations of it, which another file decides. Runs the edits on
/// one retained session and checks each generation against a cold build, for
/// a closed and an open program alike (the answer needs no closed program for
/// a prop; an exported helper needs one, so `open_expected` can differ).
fn write_twin(
    label: &str,
    files: &[(&str, &str)],
    write: (&str, &str),
    initially: (&str, &str),
    edits: &[(&str, &str, &str, &str)],
) {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let (directory, paths) = project(label, files);
    let tsconfig = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let write_path = paths[write.0].clone();
    let mut write_source = files
        .iter()
        .find(|(name, _)| *name == write.0)
        .unwrap()
        .1
        .to_owned();
    let selected = dialect::default_dialect();
    let typescript = TypeFactsSession::open(&typefacts, &tsconfig, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(selected, tsconfig, typescript).unwrap();
    let mut closed_builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut open_builder = solid_reactive_ir::IncrementalBuilder::default();
    let check = |generation: &str,
                 write_source: &str,
                 facts: &solid_facts::ProjectFacts,
                 closed_builder: &mut solid_reactive_ir::IncrementalBuilder,
                 open_builder: &mut solid_reactive_ir::IncrementalBuilder,
                 expected: (&str, &str)| {
        for (closed, builder, expected) in [
            (true, closed_builder, expected.0),
            (false, open_builder, expected.1),
        ] {
            let retained = warm(builder, facts, selected.vocabulary, closed);
            let fresh = cold(facts, selected.vocabulary, closed);
            assert_eq!(retained, fresh, "{label} {generation} closed={closed}");
            let retained_findings = selected.solve(&retained);
            assert_eq!(
                retained_findings,
                selected.solve(&fresh),
                "{label} {generation} closed={closed}: findings"
            );
            assert_eq!(
                write_kind(&retained_findings, &write_path, write_source, write.1),
                expected,
                "{label} {generation} closed={closed}: write finding"
            );
        }
    };
    let first = session.analyze().unwrap();
    check(
        "initial",
        &write_source,
        &first,
        &mut closed_builder,
        &mut open_builder,
        initially,
    );
    for (version, (name, text, closed, open)) in edits.iter().enumerate() {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: paths[*name].clone(),
                    version: version as u64 + 1,
                    source: Some((*text).into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        if *name == write.0 {
            (*text).clone_into(&mut write_source);
        }
        check(
            &format!("edit {} ({name})", version + 1),
            &write_source,
            &facts,
            &mut closed_builder,
            &mut open_builder,
            (closed, open),
        );
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

const PROP_WRITE_APP: &str = r#"import { createSignal } from "solid-js";
import { Select } from "./Select";
export function App() {
  const [, setOpen] = createSignal(false);
  return <Select onSelect={() => setOpen(true)} />;
}
"#;
const PROP_WRITE_EVENT: &str = r#"export function Select(props: { onSelect: () => void }) {
  return <button onClick={props.onSelect}>select</button>;
}
"#;
const PROP_WRITE_BODY: &str = r#"export function Select(props: { onSelect: () => void }) {
  props.onSelect();
  return <button>select</button>;
}
"#;
const PROP_WRITE_SPREAD: &str = r#"export function Select(props: { onSelect: () => void }) {
  return <button {...props}>select</button>;
}
"#;
const PROP_WRITE_DESTRUCTURED: &str = r#"export function Select({ onSelect }: { onSelect: () => void }) {
  return <button onClick={onSelect}>select</button>;
}
"#;

/// A component stops handing its prop to a handler and calls it in its body,
/// then destructures it, then goes back: the caller's write follows each edit.
#[test]
fn prop_write_role_edits_match_cold() {
    write_twin(
        "flow-write-prop-edit",
        &[
            ("App.tsx", PROP_WRITE_APP),
            ("Select.tsx", PROP_WRITE_EVENT),
        ],
        ("App.tsx", "setOpen(true)"),
        ("clean", "clean"),
        &[
            ("Select.tsx", PROP_WRITE_BODY, "violation", "violation"),
            (
                "Select.tsx",
                PROP_WRITE_DESTRUCTURED,
                "uncertifiable",
                "uncertifiable",
            ),
            ("Select.tsx", PROP_WRITE_SPREAD, "clean", "clean"),
            ("Select.tsx", PROP_WRITE_EVENT, "clean", "clean"),
        ],
    );
}

const HELPER_WRITE_APP: &str = r#"import { createSignal } from "solid-js";
import { withTx } from "./tx";
export function App() {
  const [, setX] = createSignal(0);
  withTx(() => setX(1));
  return <main />;
}
"#;
const HELPER_WRITE_CALLS: &str = "export function withTx(fn: () => void): void {\n  fn();\n}\n";
const HELPER_WRITE_KEEPS: &str = "const queue: Array<() => void> = [];\nexport function withTx(fn: () => void): void {\n  queue.push(fn);\n}\n";

/// A helper in another file starts keeping its callback and goes back: the
/// caller's write is a violation only while the helper calls it.
#[test]
fn helper_write_role_edits_match_cold() {
    write_twin(
        "flow-write-helper-edit",
        &[("App.tsx", HELPER_WRITE_APP), ("tx.ts", HELPER_WRITE_CALLS)],
        ("App.tsx", "setX(1)"),
        ("violation", "violation"),
        &[
            (
                "tx.ts",
                HELPER_WRITE_KEEPS,
                "uncertifiable",
                "uncertifiable",
            ),
            ("tx.ts", HELPER_WRITE_CALLS, "violation", "violation"),
        ],
    );
}

const SUMMARY_WRITE_APP: &str = r#"import { createSignal } from "solid-js";
import { track } from "./lib";
export function App() {
  const [, setN] = createSignal(0);
  const bump = () => setN(1);
  track(() => bump());
  return <main />;
}
"#;
const SUMMARY_WRITE_OWNED: &str = "import { createMemo } from \"solid-js\";\nexport function track(fn: () => void): void {\n  createMemo(() => fn());\n}\n";
const SUMMARY_WRITE_KEEPS: &str = "const queue: Array<() => void> = [];\nexport function track(fn: () => void): void {\n  queue.push(fn);\n}\n";

/// The write sits in a function of its own file (`bump`), whose only call is in
/// a literal handed to another file's helper: the write's role is decided by
/// what that helper does with the literal; the flow answer for `bump` enters
/// the helper's module, so the proof digest binds it.
#[test]
fn callee_summary_write_role_edits_match_cold() {
    write_twin(
        "flow-write-summary-edit",
        &[
            ("App.tsx", SUMMARY_WRITE_APP),
            ("lib.ts", SUMMARY_WRITE_OWNED),
        ],
        ("App.tsx", "setN(1)"),
        ("violation", "violation"),
        &[
            (
                "lib.ts",
                SUMMARY_WRITE_KEEPS,
                "uncertifiable",
                "uncertifiable",
            ),
            ("lib.ts", SUMMARY_WRITE_OWNED, "violation", "violation"),
        ],
    );
}

const COMPONENT_WRITE_APP: &str = r#"import { createSignal } from "solid-js";
export function maybe() {
  const [, setN] = createSignal(0);
  const bump = () => setN(1);
  bump();
  return null;
}
"#;
const COMPONENT_WRITE_PLAIN: &str =
    "import { maybe } from \"./App\";\nexport const value = maybe;\n";
const COMPONENT_WRITE_RENDERED: &str =
    "import { maybe } from \"./App\";\nconst Maybe = maybe;\nexport const view = <Maybe />;\n";

/// The write sits in a function of its own file whose only call is in the
/// body of another function of that file; whether that body is a component's
/// (an owned scope) is decided by a use in a third file, an input of the
/// classifier that the write-flow answer does not record as a module touched.
#[test]
fn use_site_write_role_edits_match_cold() {
    write_twin(
        "flow-write-use-site-edit",
        &[
            ("App.tsx", COMPONENT_WRITE_APP),
            ("use.tsx", COMPONENT_WRITE_PLAIN),
        ],
        ("App.tsx", "setN(1)"),
        ("uncertifiable", "uncertifiable"),
        &[
            (
                "use.tsx",
                COMPONENT_WRITE_RENDERED,
                "violation",
                "violation",
            ),
            (
                "use.tsx",
                COMPONENT_WRITE_PLAIN,
                "uncertifiable",
                "uncertifiable",
            ),
        ],
    );
}
