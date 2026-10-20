//! ADR 0325: an imported constant's primitive-ness is another file's fact.
//! A retained session must answer what a cold build answers after the
//! constant's file is edited in both directions, and after the import is
//! rewritten to something the proof refuses, with the boundary closed and
//! open.
use std::{env, fs};

use crate::invocation_flow_process::{cold, finding_open, project, warm};
use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{NativeIncrementalSession, SourceChange, TypeFactsSession, dialect};

const APP: &str = r#"import { CLUSTER } from "./constants";
declare function lookup(value: unknown): number;
function fetchThing(id: unknown) {
  return lookup(id);
}
function Screen(props: { id: string }) {
  const value = fetchThing(props.id);
  return <button>{value}</button>;
}
export function Root() {
  return <Screen id={CLUSTER} />;
}
export function Direct() {
  const value = fetchThing(CLUSTER);
  return <button>{value}</button>;
}
"#;
const CONSTANTS_STRING: &str = "export const CLUSTER = \"quickstart\";\n";
const CONSTANTS_OBJECT: &str = "export const CLUSTER = { id: \"quickstart\" };\n";
const CONSTANTS_LET: &str = "export let CLUSTER = \"quickstart\";\n";
const CONSTANTS_ALIASED: &str = "const HIDDEN = \"quickstart\";\nexport { HIDDEN as CLUSTER };\n";
const CONSTANTS_CJS: &str =
    "export const CLUSTER = \"quickstart\";\nmodule.exports.CLUSTER = { id: \"other\" };\n";
const CONSTANTS_RENAMED: &str = "export const OTHER = \"quickstart\";\n";
const BARREL: &str = "export { CLUSTER } from \"./constants\";\n";
const APP_VIA_BARREL: &str = r#"import { CLUSTER } from "./barrel";
declare function lookup(value: unknown): number;
function fetchThing(id: unknown) {
  return lookup(id);
}
function Screen(props: { id: string }) {
  const value = fetchThing(props.id);
  return <button>{value}</button>;
}
export function Root() {
  return <Screen id={CLUSTER} />;
}
export function Direct() {
  const value = fetchThing(CLUSTER);
  return <button>{value}</button>;
}
"#;

/// `(closed, open)`: the unmodeled use at `props.id` is an obligation. With
/// the boundary open the obligation is never discharged (it stands for
/// callers outside the files), whatever the constant is.
type Expected = (bool, bool);

fn twin(initially: Expected, edits: &[(&str, &str, Expected)]) {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let files = [("App.tsx", APP), ("constants.ts", CONSTANTS_STRING)];
    let (directory, paths) = project("imported-const", &files);
    let tsconfig = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let read_path = paths["App.tsx"].clone();
    let mut read_source = APP.to_owned();
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
            assert_eq!(retained, fresh, "{generation} closed={closed}");
            let findings = selected.solve(&retained);
            assert_eq!(
                findings,
                selected.solve(&fresh),
                "{generation} closed={closed}: findings"
            );
            for needle in ["props.id", "CLUSTER);"] {
                if read_source.contains(needle) {
                    // A direct argument is settled at its call whatever the
                    // boundary; a component prop's obligation stands for
                    // callers outside the files while it is open.
                    let wanted = if needle == "props.id" && !closed {
                        expected.1
                    } else {
                        expected.0
                    };
                    assert_eq!(
                        finding_open(&findings, &read_path, read_source, needle, "SC9012"),
                        wanted,
                        "{generation} closed={closed}: the unmodeled use at {needle}"
                    );
                }
            }
        }
    };
    let first = session.analyze().unwrap();
    check("initial", &read_source, &first, initially);
    for (version, (name, text, expected)) in edits.iter().enumerate() {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: paths.get(*name).cloned().unwrap_or_else(|| {
                        directory
                            .join(name)
                            .canonicalize()
                            .unwrap()
                            .to_string_lossy()
                            .into_owned()
                    }),
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

/// The constant's file turns the value into an object, a `let` and back, and
/// is exported through an alias and under another name.
#[test]
fn constant_file_edits_match_cold() {
    twin(
        (false, true),
        &[
            ("constants.ts", CONSTANTS_OBJECT, (true, true)),
            ("constants.ts", CONSTANTS_STRING, (false, true)),
            ("constants.ts", CONSTANTS_LET, (true, true)),
            ("constants.ts", CONSTANTS_ALIASED, (false, true)),
            ("constants.ts", CONSTANTS_RENAMED, (true, true)),
            ("constants.ts", CONSTANTS_STRING, (false, true)),
            ("constants.ts", CONSTANTS_CJS, (true, true)),
            ("constants.ts", CONSTANTS_STRING, (false, true)),
        ],
    );
}

/// The import is rewritten to go through a barrel, whose re-export hop the
/// proof refuses, and back.
#[test]
fn import_through_a_barrel_matches_cold() {
    // The barrel exists from the start so the program's membership does not
    // change with the edit; only the import's target does.
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let files = [
        ("App.tsx", APP),
        ("constants.ts", CONSTANTS_STRING),
        ("barrel.ts", BARREL),
    ];
    let (directory, paths) = project("imported-const-barrel", &files);
    let tsconfig = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let read_path = paths["App.tsx"].clone();
    let selected = dialect::default_dialect();
    let typescript = TypeFactsSession::open(&typefacts, &tsconfig, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(selected, tsconfig, typescript).unwrap();
    let mut builder = solid_reactive_ir::IncrementalBuilder::default();
    let mut source = APP.to_owned();
    let facts = session.analyze().unwrap();
    for (step, (text, open)) in [(APP, false), (APP_VIA_BARREL, true), (APP, false)]
        .into_iter()
        .enumerate()
    {
        let facts = if step == 0 {
            facts.clone()
        } else {
            session
                .edit(
                    vec![SourceChange {
                        path: read_path.clone(),
                        version: step as u64,
                        source: Some(text.into()),
                        compiler_options: CompilerOptions::default(),
                    }],
                    None,
                )
                .unwrap()
        };
        text.clone_into(&mut source);
        let retained = warm(&mut builder, &facts, selected.vocabulary, true);
        let fresh = cold(&facts, selected.vocabulary, true);
        assert_eq!(retained, fresh, "barrel step {step}");
        assert_eq!(
            finding_open(
                &selected.solve(&retained),
                &read_path,
                &source,
                "props.id",
                "SC9012"
            ),
            open,
            "barrel step {step}"
        );
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}
