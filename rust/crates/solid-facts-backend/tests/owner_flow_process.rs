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
