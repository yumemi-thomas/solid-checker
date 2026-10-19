//! ADR 0323: whether a props read inside an anonymous callback is reported
//! depends on what the callback reaches, and that is decided by other files
//! (the callee's body, whether the callee resolves to a project function at
//! all). A retained session must answer exactly what a cold build answers
//! after every edit, in both directions, with the program boundary closed and
//! open.
use std::{env, fs};

use crate::invocation_flow_process::{cold, finding_open, project, warm};
use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{NativeIncrementalSession, SourceChange, TypeFactsSession, dialect};

/// What the read's finding is: `(closed, open)`.
type Expected = (bool, bool);

fn twin(
    label: &str,
    files: &[(&str, &str)],
    needle: &str,
    initially: Expected,
    edits: &[(&str, &str, Expected)],
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
    let read_path = paths["App.tsx"].clone();
    let mut read_source = files
        .iter()
        .find(|(name, _)| *name == "App.tsx")
        .unwrap()
        .1
        .to_owned();
    let selected = dialect::default_dialect();
    let typescript = TypeFactsSession::open(&typefacts, &tsconfig, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(selected, tsconfig, typescript).unwrap();
    let mut closed_builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut open_builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut check = |generation: &str,
                     read_source: &str,
                     facts: &solid_facts::ProjectFacts,
                     expected: Expected| {
        for (closed, builder) in [(true, &mut closed_builder), (false, &mut open_builder)] {
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
                finding_open(
                    &retained_findings,
                    &read_path,
                    read_source,
                    needle,
                    "SC1001"
                ),
                if closed { expected.0 } else { expected.1 },
                "{label} {generation} closed={closed}: read finding"
            );
        }
    };
    let first = session.analyze().unwrap();
    check("initial", &read_source, &first, initially);
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
        if *name == "App.tsx" {
            (*text).clone_into(&mut read_source);
        }
        check(
            &format!("edit {} ({name})", version + 1),
            &read_source,
            &facts,
            *expected,
        );
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

const APP: &str = r#"import { run } from "./lib";
export function App(props: { x: number }) {
  run(() => console.log(props.x));
  return <button />;
}
"#;
const LIB_DEFERRED: &str = r#"export function run(f: () => void) {
  setTimeout(f, 0);
}
"#;
const LIB_BODY: &str = r#"export function run(f: () => void) {
  f();
}
"#;
const LIB_NO_BODY: &str = r#"export declare function run(f: () => void): void;
"#;

/// The callee file starts calling the callback in its body, stops, and then
/// loses its body altogether (an unknown callee keeps the old silence).
#[test]
fn callee_edits_turn_a_props_read_on_and_off() {
    twin(
        "callback-props-callee",
        &[("App.tsx", APP), ("lib.ts", LIB_DEFERRED)],
        "props.x",
        // A callee that hands the callback to a timer places it there
        // (ADR 0315's use-site role), with the boundary open or closed.
        (false, false),
        &[
            ("lib.ts", LIB_BODY, (true, true)),
            ("lib.ts", LIB_DEFERRED, (false, false)),
            // A callee with no body is an unknown consumer: since the ADR
            // 0323 amendment the read is uncertifiable, like its signal twin.
            ("lib.ts", LIB_NO_BODY, (true, true)),
            ("lib.ts", LIB_BODY, (true, true)),
        ],
    );
}

const APP_FOR_EACH: &str = r#"export function App(props: { x: number }) {
  [1].forEach((n) => console.log(n + props.x));
  return <button />;
}
"#;
const APP_TIMER: &str = r#"export function App(props: { x: number }) {
  setTimeout(() => console.log(props.x), 0);
  return <button />;
}
"#;
const APP_IIFE: &str = r#"export function App(props: { x: number }) {
  (() => console.log(props.x))();
  return <button />;
}
"#;
const APP_TIMER_IIFE: &str = r#"export function App(props: { x: number }) {
  setTimeout(() => {
    (() => console.log(props.x))();
  }, 0);
  return <button />;
}
"#;

/// The same file moves a read between an inline standard-library callback, a
/// timer, an IIFE and an IIFE inside a timer.
#[test]
fn local_callback_shape_edits_match_cold() {
    twin(
        "callback-props-local",
        &[("App.tsx", APP_FOR_EACH)],
        "props.x",
        (true, true),
        &[
            ("App.tsx", APP_TIMER, (false, false)),
            ("App.tsx", APP_IIFE, (true, true)),
            // Closed: the walk places the IIFE in the timer, which does not
            // report. Open: an obligation, like the signal twin (ADR 0323
            // amendment).
            ("App.tsx", APP_TIMER_IIFE, (false, true)),
            ("App.tsx", APP_FOR_EACH, (true, true)),
        ],
    );
}

const APP_HOOK: &str = r#"import { useKeyed } from "./hook";
export function App(props: { id: string }) {
  const q = useKeyed({ key: () => `k:${props.id}` });
  return <button>{q.state()}</button>;
}
"#;
const HOOK_MIX: &str = r#"import { createMemo, untrack } from "solid-js";
export function useKeyed(options: { key: () => string }) {
  const bound = createMemo(() => options.key(), { lazy: true });
  return { state: () => bound(), reload: () => untrack(options.key) };
}
"#;
const HOOK_MIX_BODY: &str = r#"import { createMemo, untrack } from "solid-js";
export function useKeyed(options: { key: () => string }) {
  const bound = createMemo(() => options.key(), { lazy: true });
  const first = options.key();
  return { state: () => bound() + first, reload: () => untrack(options.key) };
}
"#;
const HOOK_TRACKED_ONLY: &str = r#"import { createMemo } from "solid-js";
export function useKeyed(options: { key: () => string }) {
  const bound = createMemo(() => options.key(), { lazy: true });
  return { state: () => bound() };
}
"#;

/// ADR 0323: invocations in different non-reporting roles are accepted as a
/// mix; one invocation in the render window (the hook's own body) refuses.
/// With the boundary open the flow is off, so the read is an obligation
/// throughout.
#[test]
fn a_mixed_role_callback_follows_the_hook_file() {
    twin(
        "callback-props-mix",
        &[("App.tsx", APP_HOOK), ("hook.ts", HOOK_MIX)],
        "props.id",
        (false, true),
        &[
            ("hook.ts", HOOK_MIX_BODY, (true, true)),
            ("hook.ts", HOOK_MIX, (false, true)),
            // One tracked role needs no flow: ADR 0315 places it with the boundary open too.
            ("hook.ts", HOOK_TRACKED_ONLY, (false, false)),
            ("hook.ts", HOOK_MIX_BODY, (true, true)),
        ],
    );
}

const APP_OMIT_Y: &str = r#"import { omit } from "solid-js";
export function App(props: { x: number; y: number }) {
  const o = omit(props, "y");
  console.log(o.x);
  return <button />;
}
"#;
const APP_OMIT_X: &str = r#"import { omit } from "solid-js";
export function App(props: { x: number; y: number }) {
  const o = omit(props, "x");
  console.log(o.x);
  return <button />;
}
"#;
const APP_OMIT_NONE: &str = r#"import { omit } from "solid-js";
export function App(props: { x: number; y: number }) {
  const o = omit(props, (key) => key === "x");
  console.log(o.x);
  return <button />;
}
"#;

/// ADR 0323: an `omit` view reads a key it keeps and nothing of a key it hides
/// by string literal; a predicate hides nothing nameable.
#[test]
fn omit_view_edits_match_cold() {
    twin(
        "callback-props-omit",
        &[("App.tsx", APP_OMIT_Y)],
        "o.x",
        (true, true),
        &[
            ("App.tsx", APP_OMIT_X, (false, false)),
            ("App.tsx", APP_OMIT_NONE, (true, true)),
            ("App.tsx", APP_OMIT_Y, (true, true)),
        ],
    );
}

const APP_BEFORE_AWAIT: &str = r#"export function App(props: { x: number }) {
  [1].forEach(async () => {
    console.log(props.x);
    await 0;
  });
  return <button />;
}
"#;
const APP_AFTER_AWAIT: &str = r#"export function App(props: { x: number }) {
  [1].forEach(async () => {
    await 0;
    console.log(props.x);
  });
  return <button />;
}
"#;
const APP_SKIPPABLE_AWAIT: &str = r#"declare const p: Promise<number>;
export function App(props: { x: number; y?: number }) {
  [1].forEach(async () => {
    const y = props.y;
    let z = y;
    z ??= await p;
    console.log(props.x, z);
  });
  return <button />;
}
"#;

/// ADR 0323: a read after an unconditional await runs on a later task.
#[test]
fn await_continuation_edits_match_cold() {
    twin(
        "callback-props-await",
        &[("App.tsx", APP_BEFORE_AWAIT)],
        "props.x",
        (true, true),
        &[
            ("App.tsx", APP_AFTER_AWAIT, (false, false)),
            ("App.tsx", APP_SKIPPABLE_AWAIT, (true, true)),
            ("App.tsx", APP_BEFORE_AWAIT, (true, true)),
        ],
    );
}

const HOOK_ROOT: &str = r#"import { createRoot } from "solid-js";
export function useKeyed(options: { key: () => string }) {
  createRoot(() => options.key());
  return { state: () => "" };
}
"#;
const HOOK_RUN_WITH_OWNER: &str = r#"import { getOwner, runWithOwner } from "solid-js";
export function useKeyed(options: { key: () => string }) {
  runWithOwner(getOwner(), () => options.key());
  return { state: () => "" };
}
"#;

/// ADR 0323 (review round 10): a callback the hook runs in a root or under
/// `runWithOwner` runs in its caller's window; a tracked slot does not.
#[test]
fn hook_slot_edits_match_cold() {
    twin(
        "callback-props-slot",
        &[("App.tsx", APP_HOOK), ("hook.ts", HOOK_TRACKED_ONLY)],
        "props.id",
        (false, false),
        &[
            ("hook.ts", HOOK_ROOT, (true, true)),
            ("hook.ts", HOOK_MIX, (false, true)),
            ("hook.ts", HOOK_RUN_WITH_OWNER, (true, true)),
            ("hook.ts", HOOK_TRACKED_ONLY, (false, false)),
            ("hook.ts", HOOK_ROOT, (true, true)),
        ],
    );
}

const APP_ROOT_BODY: &str = r#"import { createRoot } from "solid-js";
export function App(props: { x: number }) {
  createRoot(() => console.log(props.x));
  return <button />;
}
"#;
const APP_ROOT_EVENT: &str = r#"import { createRoot } from "solid-js";
export function App(props: { x: number }) {
  return <button onClick={() => createRoot(() => console.log(props.x))} />;
}
"#;
const APP_ROOT_MEMO: &str = r#"import { createMemo, createRoot } from "solid-js";
export function App(props: { x: number }) {
  const m = createMemo(() => createRoot(() => props.x));
  return <button>{m()}</button>;
}
"#;
const APP_ROOT_THEN: &str = r#"import { createRoot } from "solid-js";
export function App(props: { x: number }) {
  void Promise.resolve().then(() => createRoot(() => console.log(props.x)));
  return <button />;
}
"#;

/// ADR 0323 (review round 10): a root's read is in the window of the call.
#[test]
fn root_window_edits_match_cold() {
    twin(
        "callback-props-root",
        &[("App.tsx", APP_ROOT_BODY)],
        "props.x",
        (true, true),
        &[
            ("App.tsx", APP_ROOT_EVENT, (false, false)),
            ("App.tsx", APP_ROOT_MEMO, (false, false)),
            ("App.tsx", APP_ROOT_THEN, (false, false)),
            ("App.tsx", APP_ROOT_BODY, (true, true)),
        ],
    );
}

const APP_INLINE_KEPT: &str = r#"import { omit } from "solid-js";
type P = { x: number; y: number };
export function App(props: P) {
  console.log(omit(props, "y").x);
  return <button />;
}
"#;
const APP_INLINE_HIDDEN: &str = r#"import { omit } from "solid-js";
type P = { x: number; y: number };
export function App(props: P) {
  console.log(omit(props, "y").y);
  return <button />;
}
"#;
const APP_INLINE_PREDICATE: &str = r#"import { omit } from "solid-js";
type P = { x: number; y: number };
export function App(props: P) {
  console.log(omit(props, (key) => key === "y").y);
  return <button />;
}
"#;
const APP_INLINE_NESTED_HIDDEN: &str = r#"import { omit } from "solid-js";
type P = { x: number; y: number };
export function App(props: P) {
  console.log(omit(omit(props, "x"), "y").y);
  return <button />;
}
"#;

/// ADR 0323 (review round 10): an inline `omit` view is the bound one.
#[test]
fn inline_view_edits_match_cold() {
    twin(
        "callback-props-inline-view",
        &[("App.tsx", APP_INLINE_KEPT)],
        "omit(",
        (true, true),
        &[
            ("App.tsx", APP_INLINE_HIDDEN, (false, false)),
            ("App.tsx", APP_INLINE_PREDICATE, (true, true)),
            ("App.tsx", APP_INLINE_NESTED_HIDDEN, (false, false)),
            ("App.tsx", APP_INLINE_KEPT, (true, true)),
        ],
    );
}
