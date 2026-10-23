//! ADR 0327: whether a callback runs only from an event dispatch is decided
//! by the helper's file. A retained session must answer what a cold build
//! answers after the helper stops and starts returning the callback as an
//! event handler, with the boundary closed and open (open: no walk, no seed).
use std::{env, fs};

use crate::invocation_flow_process::{cold, project, warm};
use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{NativeIncrementalSession, SourceChange, TypeFactsSession, dialect};

const APP: &str = r#"import { onCleanup } from "solid-js";
import { handlers } from "./lib";
export function App() {
  const h = handlers(() => {
    onCleanup(() => undefined);
  });
  return <button {...h}>run</button>;
}
"#;
const LIB_EVENT: &str = r#"export function handlers(cb: () => void) {
  return { onClick: () => cb() };
}
"#;
const LIB_NOT_EVENT: &str = r#"export function handlers(cb: () => void) {
  return { run: () => cb() };
}
"#;
const LIB_BODY: &str = r#"export function handlers(cb: () => void) {
  cb();
  return { onClick: () => cb() };
}
"#;
const LIB_DISPATCH: &str = r#"export function handlers(cb: () => void) {
  new EventTarget().dispatchEvent(new Event("ping"));
  return { onClick: () => cb() };
}
"#;
const LIB_HOMONYM: &str = r#"export function handlers(cb: () => void) {
  const bus = { dispatchEvent() { return true; }, click() {} };
  bus.dispatchEvent();
  bus.click();
  return { onClick: () => cb() };
}
"#;

/// Whether `findings` holds a proven `SC4001` at the `onCleanup` call.
fn owner_violation(findings: &[solid_reactive_ir::Finding], path: &str) -> bool {
    let start = APP.find("onCleanup(() =>").unwrap() as u64;
    findings.iter().any(|finding| {
        finding.id == "SC4001"
            && finding.kind == "violation"
            && finding.primary_location.path.as_ref() == path
            && finding.primary_location.start_byte == start
    })
}

#[test]
fn helper_event_edits_match_cold() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let (directory, paths) = project("owner-flow", &[("App.tsx", APP), ("lib.ts", LIB_EVENT)]);
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
    let mut closed_builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut open_builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut check = |generation: &str, facts: &solid_facts::ProjectFacts, expected: bool| {
        for (closed, builder) in [(true, &mut closed_builder), (false, &mut open_builder)] {
            let retained = warm(builder, facts, selected.vocabulary, closed);
            let fresh = cold(facts, selected.vocabulary, closed);
            assert_eq!(retained, fresh, "{generation} closed={closed}");
            let findings = selected.solve(&retained);
            assert_eq!(
                findings,
                selected.solve(&fresh),
                "{generation} closed={closed}: findings"
            );
            assert_eq!(
                owner_violation(&findings, &paths["App.tsx"]),
                closed && expected,
                "{generation} closed={closed}: the onCleanup owner finding"
            );
        }
    };
    let first = session.analyze().unwrap();
    check("initial", &first, true);
    // The helper stops returning an event handler, calls the callback in its
    // own body as well (an owned run: mixed, refused), and goes back.
    for (version, (text, expected)) in [
        (LIB_NOT_EVENT, false),
        (LIB_EVENT, true),
        (LIB_DISPATCH, false),
        (LIB_EVENT, true),
        (LIB_HOMONYM, true),
        (LIB_BODY, false),
        (LIB_EVENT, true),
    ]
    .into_iter()
    .enumerate()
    {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: paths["lib.ts"].clone(),
                    version: version as u64 + 1,
                    source: Some(text.into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        check(&format!("edit {}", version + 1), &facts, expected);
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

/// A remote helper switches between synchronous, possible and definite
/// suspension. Its cleanup must gain an obligation, never a silent clean,
/// while the retained and fresh owner passes agree in both directions.
#[test]
fn possible_cleanup_suspension_edits_match_cold() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    const APP: &str = "import { onSettled } from 'solid-js'; import { cleanup } from './lib'; export function App() { onSettled(() => { void cleanup(); }); return <button />; }";
    const SYNC: &str = "import { onCleanup } from 'solid-js'; export async function cleanup() { onCleanup(() => {}); }";
    const POSSIBLE: &str = "import { onCleanup } from 'solid-js'; declare const flag: boolean; export async function cleanup() { if (flag) await 0; onCleanup(() => {}); }";
    const DEFINITE: &str = "import { onCleanup } from 'solid-js'; export async function cleanup() { await 0; onCleanup(() => {}); }";
    const WRAPPED: &str = "import { onCleanup } from 'solid-js'; declare const flag: boolean; function register() { onCleanup(() => {}); } export async function cleanup() { if (flag) await 0; register(); }";
    const CALLBACK: &str = "import { onCleanup, untrack } from 'solid-js'; declare const flag: boolean; export async function cleanup() { if (flag) await 0; untrack(() => onCleanup(() => {})); }";
    let (directory, paths) = project(
        "possible-cleanup-owner",
        &[("App.tsx", APP), ("lib.ts", SYNC)],
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
    let mut builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut check = |facts: &solid_facts::ProjectFacts, expected: Option<&str>| {
        let retained = warm(&mut builder, facts, selected.vocabulary, true);
        let fresh = cold(facts, selected.vocabulary, true);
        assert_eq!(retained, fresh);
        let findings = selected.solve(&retained);
        assert_eq!(findings, selected.solve(&fresh));
        let kinds = findings
            .iter()
            .filter(|finding| {
                finding.id == "SC4001" && finding.primary_location.path.as_ref() == paths["lib.ts"]
            })
            .map(|finding| finding.kind.as_str())
            .collect::<Vec<_>>();
        assert_eq!(kinds, expected.into_iter().collect::<Vec<_>>());
    };
    check(&session.analyze().unwrap(), None);
    for (version, (text, expected)) in [
        (POSSIBLE, Some("uncertifiable")),
        (SYNC, None),
        (DEFINITE, Some("violation")),
        (WRAPPED, Some("uncertifiable")),
        (CALLBACK, Some("uncertifiable")),
        (POSSIBLE, Some("uncertifiable")),
    ]
    .into_iter()
    .enumerate()
    {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: paths["lib.ts"].clone(),
                    version: version as u64 + 1,
                    source: Some(text.into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        check(&facts, expected);
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

/// Exact remote member/spread placements must invalidate retained owner edges
/// when the exported receiver changes, even though its JSX consumer is unchanged.
#[test]
fn handler_member_and_spread_edits_match_cold() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    const APP: &str = "import { handlers } from './lib'; export function App() { return <><button onClick={handlers.onClick} /><button {...handlers} /></>; }";
    const EXACT: &str = "import { onCleanup } from 'solid-js'; export const handlers = { onClick: () => { onCleanup(() => {}); } };";
    const SHIFTED: &str = "import { onCleanup } from 'solid-js'; export const handlers = { title: 'shift', onClick: () => { onCleanup(() => {}); } };";
    const MUTATED: &str = "import { onCleanup } from 'solid-js'; export const handlers = { onClick: () => { onCleanup(() => {}); } }; handlers.onClick = () => {};";
    const GETTER: &str = "import { onCleanup } from 'solid-js'; export const handlers = { get title(): string { this.onClick = () => {}; return ''; }, onClick: () => { onCleanup(() => {}); } };";
    let (directory, paths) = project("handler-placements", &[("App.tsx", APP), ("lib.ts", EXACT)]);
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
    let mut closed_builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut open_builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut check = |facts: &solid_facts::ProjectFacts, exact: bool| {
        for (closed, builder) in [(true, &mut closed_builder), (false, &mut open_builder)] {
            let retained = warm(builder, facts, selected.vocabulary, closed);
            let fresh = cold(facts, selected.vocabulary, closed);
            assert_eq!(retained, fresh, "closed={closed}");
            let findings = selected.solve(&retained);
            assert_eq!(findings, selected.solve(&fresh));
            let violation = findings.iter().any(|finding| {
                finding.id == "SC4001"
                    && finding.kind == "violation"
                    && finding.primary_location.path.as_ref() == paths["lib.ts"]
            });
            assert_eq!(violation, exact, "closed={closed}: {findings:?}");
        }
    };
    check(&session.analyze().unwrap(), true);
    for (version, (source, exact)) in [
        (SHIFTED, true),
        (MUTATED, false),
        (EXACT, true),
        (GETTER, false),
        (SHIFTED, true),
    ]
    .into_iter()
    .enumerate()
    {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: paths["lib.ts"].clone(),
                    version: version as u64 + 1,
                    source: Some(source.into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        check(&facts, exact);
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

/// ADR 0361: a supported dispatch withholds the ownerless proof only from the
/// handlers it delivers to, and a remote handler's finding follows the
/// dispatching file's edits. Retained and fresh builds agree on each.
#[test]
fn supported_dispatch_edits_match_cold() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    const LIB: &str = "import { onCleanup } from 'solid-js'; export const handlers = { onScroll: () => { onCleanup(() => {}); } };";
    const NONE: &str =
        "import { handlers } from './lib'; export function App() { return <div {...handlers} />; }";
    const DELIVERED: &str = "import { handlers } from './lib'; export function App() { const el = (<div {...handlers} />) as HTMLDivElement; el.dispatchEvent(new Event('scroll')); return <p />; }";
    const ELSEWHERE: &str = "import { handlers } from './lib'; export function App() { const el = (<div />) as HTMLDivElement; el.dispatchEvent(new Event('scroll')); return <p {...handlers} />; }";
    const BUBBLES: &str = "import { handlers } from './lib'; export function App() { const el = (<div />) as HTMLDivElement; el.dispatchEvent(new Event('scroll', { bubbles: true })); return <p {...handlers} />; }";
    const DELEGATED: &str = "import { handlers } from './lib'; export function App() { const el = (<div />) as HTMLDivElement; el.dispatchEvent(new Event('click')); return <p {...handlers} />; }";
    const ESCAPES: &str = "import { handlers } from './lib'; export function App() { const el = (<div />) as HTMLDivElement; el.dispatchEvent(new Event('scroll')); return <p {...handlers}>{el}</p>; }";
    const SHADOWED: &str = "import { handlers } from './lib'; class Event { constructor(readonly type: string) {} } export function App() { const el = (<div />) as HTMLDivElement; el.dispatchEvent(new Event('scroll') as unknown as globalThis.Event); return <p {...handlers} />; }";
    let (directory, paths) = project(
        "supported-dispatch",
        &[("App.tsx", DELIVERED), ("lib.ts", LIB)],
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
    let mut closed_builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut open_builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut check = |generation: &str, facts: &solid_facts::ProjectFacts, violation: bool| {
        for (closed, builder) in [(true, &mut closed_builder), (false, &mut open_builder)] {
            let retained = warm(builder, facts, selected.vocabulary, closed);
            let fresh = cold(facts, selected.vocabulary, closed);
            assert_eq!(retained, fresh, "{generation} closed={closed}");
            let findings = selected.solve(&retained);
            assert_eq!(
                findings,
                selected.solve(&fresh),
                "{generation} closed={closed}: findings"
            );
            let proven = findings.iter().any(|finding| {
                finding.id == "SC4001"
                    && finding.kind == "violation"
                    && finding.primary_location.path.as_ref() == paths["lib.ts"]
            });
            assert_eq!(
                proven, violation,
                "{generation} closed={closed}: {findings:?}"
            );
        }
    };
    check("delivered", &session.analyze().unwrap(), false);
    for (version, (name, source, violation)) in [
        ("no dispatch", NONE, true),
        ("delivered", DELIVERED, false),
        ("another element", ELSEWHERE, true),
        ("bubbling", BUBBLES, false),
        ("another element", ELSEWHERE, true),
        ("delegated type", DELEGATED, false),
        ("escaping receiver", ESCAPES, false),
        ("shadowed constructor", SHADOWED, false),
        ("another element", ELSEWHERE, true),
    ]
    .into_iter()
    .enumerate()
    {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: paths["App.tsx"].clone(),
                    version: version as u64 + 1,
                    source: Some(source.into()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        check(name, &facts, violation);
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}
