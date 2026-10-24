//! The memo store's file fingerprints do not trust the producer's list of
//! changed paths (ADR 03XX-bookkeeping): a file's TypeFacts parts are
//! digested again whenever its rows are not the very rows digested last
//! build, so a path the producer leaves unnamed is still seen to change.
//!
//! The real producer answers an edit of `a.ts` and names only `a.ts`; the
//! test then replaces `b.tsx`'s TypeFacts entities behind its back (every
//! resolved call there flips to a call error) and keeps the change list
//! naming `a.ts` alone. The memo store must count `b.tsx` as changed.

use std::{env, fs, path::PathBuf, sync::Arc};

use solid_facts::{ProjectFacts, compiler::CompilerOptions};
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};
use solid_reactive_ir::{AnalysisInputs, BuildTimings, IncrementalBuilder, RuleOptions};

const STUB: &str = r#"declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(compute: () => T): () => T;
}

declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
  }
}
"#;

const A: &str = "export function value(): number {\n  return 1;\n}\n";
const B: &str = r#"import { createSignal, createMemo } from "solid-js";
import { value } from "./a";

export function App() {
  const [read] = createSignal(value());
  const doubled = createMemo(() => read() * 2);
  return <div>{doubled()}</div>;
}
"#;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn build(builder: &mut IncrementalBuilder, facts: &ProjectFacts) -> BuildTimings {
    let dialect = dialect::by_id("solid-v2").expect("the 2.0 dialect is registered");
    builder
        .build_with_analysis_inputs_shared(
            facts,
            dialect.vocabulary,
            AnalysisInputs::empty(),
            &RuleOptions::default(),
        )
        .unwrap()
        .1
}

#[test]
fn a_path_the_producer_does_not_name_is_still_fingerprinted_when_its_rows_change() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = env::temp_dir()
        .join("sc-scratch.noindex")
        .join(format!("bookkeeping-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let _scratch = Scratch(root.clone());
    fs::write(root.join("solid-js.d.ts"), STUB).unwrap();
    fs::write(
        root.join("tsconfig.json"),
        r#"{"compilerOptions":{"jsx":"preserve","module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"include":["*.ts","*.tsx"]}"#,
    )
    .unwrap();
    fs::write(root.join("a.ts"), A).unwrap();
    fs::write(root.join("b.tsx"), B).unwrap();
    let root = root.canonicalize().unwrap();
    let project_id = root.join("tsconfig.json").to_string_lossy().into_owned();
    let path = |name: &str| root.join(name).to_string_lossy().into_owned();
    let sources = [("a.ts", A), ("b.tsx", B)]
        .iter()
        .map(|(name, source)| SourceFile {
            path: path(name),
            source: (*source).into(),
            compiler_options: CompilerOptions::default(),
        })
        .collect::<Vec<_>>();
    let typefacts = PathBuf::from(typefacts)
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let dialect = dialect::by_id("solid-v2").expect("the 2.0 dialect is registered");
    let mut session =
        NativeIncrementalSession::open(dialect, project_id, sources, typescript).unwrap();
    let first = session.analyze().unwrap();
    let second = session
        .edit(
            vec![SourceChange {
                path: path("a.ts"),
                version: 1,
                source: Some(A.replace("return 1;", "return 2;")),
                compiler_options: CompilerOptions::default(),
            }],
            None,
        )
        .unwrap();
    let changes = second
        .typescript_changes
        .clone()
        .expect("a retained session reports its table's changes");
    let b = path("b.tsx");
    assert!(
        !changes.entity_paths.contains(&b),
        "the producer names only the edited file: {changes:?}"
    );

    // The producer's own answer: only the edited file moved.
    let mut builder = IncrementalBuilder::default();
    build(&mut builder, &first);
    assert_eq!(build(&mut builder, &second).memo_changed_files, 1);

    // `b.tsx`'s entities replaced, every resolved call there a call error,
    // with the change list still naming `a.ts` alone.
    let mut entities = second.typescript.entities_for_path(&b).to_vec();
    let mut flipped = 0;
    for entity in &mut entities {
        if let Some(call) = entity.resolved_call.as_mut() {
            Arc::make_mut(call).validity = typefacts::ResolvedCallValidity::Recovery;
            flipped += 1;
        }
    }
    assert!(flipped > 0, "b.tsx has resolved calls");
    let mut tampered = (*second).clone();
    tampered.typescript = tampered
        .typescript
        .clone()
        .with_entity_overrides([(b.clone(), entities)].into_iter().collect());
    let mut builder = IncrementalBuilder::default();
    build(&mut builder, &first);
    assert_eq!(
        build(&mut builder, &tampered).memo_changed_files,
        2,
        "the unnamed path's rows moved: it is fingerprinted again and found changed"
    );
}
