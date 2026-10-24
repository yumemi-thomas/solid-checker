//! The execution-role machine reads other files' syntax (ADR 0408):
//! a call site's role in a callee's body, a component's callers, a named
//! callback's primitive. Each stage chunk that asks a role is reused only
//! while what the role read of those files answers as it did. Every case
//! here makes a same-length edit in a file other than the one whose chunk
//! read it, checks that the edit moves the fresh answer, and compares the
//! retained build with a fresh one table by table (shadow mode off, so the
//! comparison is this test's).

use std::{env, fs, path::PathBuf, sync::Arc};

use solid_facts::{ProjectFacts, compiler::CompilerOptions};
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};
use solid_reactive_ir::{
    AnalysisInputs, BuildTimings, IncrementalBuilder, RuleOptions,
    build_with_analysis_inputs_measured,
    shadow::{ShadowMode, first_difference},
};

fn solid_v2() -> &'static dialect::Dialect {
    dialect::by_id("solid-v2").expect("the 2.0 dialect is registered")
}

const STUB: &str = r#"declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(compute: () => T): () => T;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function untrack<T>(fn: () => T): T;
  export function onCleanup(fn: () => void): void;
}

declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
    p: Record<string, unknown>;
    button: Record<string, unknown>;
  }
  interface Element {}
}
"#;

struct Project {
    session: NativeIncrementalSession,
    root: PathBuf,
    version: u64,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Project {
    /// `None` without a producer (process tests skip silently).
    fn scratch(tag: &str, files: &[(&str, &str)]) -> Option<Self> {
        let typefacts = env::var("SOLID_TYPEFACTS_BIN").ok()?;
        let root = env::temp_dir()
            .join("sc-scratch.noindex")
            .join(format!("role-reads-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("solid-js.d.ts"), STUB).unwrap();
        fs::write(
            root.join("tsconfig.json"),
            r#"{"compilerOptions":{"jsx":"preserve","module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"include":["*.ts","*.tsx"]}"#,
        )
        .unwrap();
        for (name, source) in files {
            fs::write(root.join(name), source).unwrap();
        }
        let root = root.canonicalize().unwrap();
        let project_id = root.join("tsconfig.json").to_string_lossy().into_owned();
        let sources = files
            .iter()
            .map(|(name, source)| SourceFile {
                path: root.join(name).to_string_lossy().into_owned(),
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
        let session =
            NativeIncrementalSession::open(solid_v2(), project_id, sources, typescript).unwrap();
        Some(Self {
            session,
            root,
            version: 0,
        })
    }

    fn analyze(&mut self) -> Arc<ProjectFacts> {
        self.session.analyze().unwrap()
    }

    fn edit(&mut self, name: &str, source: &str) -> Arc<ProjectFacts> {
        self.version += 1;
        let path = self.root.join(name);
        fs::write(&path, source).unwrap();
        self.session
            .edit(
                vec![SourceChange {
                    path: path.to_string_lossy().into_owned(),
                    version: self.version,
                    source: Some(source.to_owned()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap()
    }
}

/// The retained builder's answer for `facts`, which must equal a fresh
/// build's table for table; the fresh program and the retained timings.
fn retained_is_fresh(
    builder: &mut IncrementalBuilder,
    facts: &ProjectFacts,
    label: &str,
) -> (solid_reactive_ir::Program, BuildTimings) {
    builder.set_shadow_mode(ShadowMode::Off);
    let options = RuleOptions::default();
    let vocabulary = solid_v2().vocabulary;
    let (retained, timings) = builder
        .build_with_analysis_inputs_shared(facts, vocabulary, AnalysisInputs::empty(), &options)
        .unwrap();
    let (fresh, _) =
        build_with_analysis_inputs_measured(facts, vocabulary, AnalysisInputs::empty(), &options)
            .unwrap();
    eprintln!(
        "{label}: discovery {}/{} reachability {}/{} local {}/{} graph {}/{} results {}/{} owners {}/{}",
        timings.source_discovery_reused_files,
        timings.source_discovery_recomputed_files,
        timings.reachability_reused_files,
        timings.reachability_recomputed_files,
        timings.local_access_reused_files,
        timings.local_access_recomputed_files,
        timings.interprocedural_graph_reused_files,
        timings.interprocedural_graph_recomputed_files,
        timings.interprocedural_result_reused_files,
        timings.interprocedural_result_recomputed_files,
        timings.owner_reused_files,
        timings.owner_recomputed_files,
    );
    if let Some(difference) = first_difference(&retained, &fresh) {
        panic!("{label}: {difference}");
    }
    (fresh, timings)
}

/// Runs `edits` of `edited` in turn from `files`, each the same length as
/// the first, each moving the fresh answer, then back to the first: every
/// retained build must equal a fresh one.
fn same_length_edits_follow(tag: &str, files: &[(&str, &str)], edited: &str, edits: &[&str]) {
    let original = files
        .iter()
        .find(|(name, _)| *name == edited)
        .map(|(_, source)| *source)
        .expect("the edited file is one of the project's");
    for edit in edits {
        assert_eq!(edit.len(), original.len(), "{tag}: a same-length edit");
    }
    let Some(mut project) = Project::scratch(tag, files) else {
        return;
    };
    let mut builder = IncrementalBuilder::default();
    let first = project.analyze();
    let (before, _) = retained_is_fresh(&mut builder, &first, &format!("{tag}: first"));
    for (step, edit) in edits.iter().enumerate() {
        let facts = project.edit(edited, edit);
        let (after, _) = retained_is_fresh(&mut builder, &facts, &format!("{tag}: edit {step}"));
        assert!(
            first_difference(&before, &after).is_some(),
            "{tag}: edit {step} must move the fresh answer"
        );
        let facts = project.edit(edited, original);
        let (back, _) = retained_is_fresh(&mut builder, &facts, &format!("{tag}: back {step}"));
        assert!(
            first_difference(&before, &back).is_none(),
            "{tag}: back to the first answer"
        );
    }
}

/// ADR 0315: a literal handed to a project callee takes the role of the
/// callee's call of its parameter (`callee_use_site_role`). The callee's
/// file changes how it calls the parameter; the caller's file is unchanged.
const USE_SITE_APP: &str = "import { createSignal } from \"solid-js\";\nimport { on } from \"./b\";\nexport function App() {\n  const [count] = createSignal(0);\n  on(() => {\n    count();\n  });\n  return <div />;\n}\n";
const USE_SITE_DEFERRED: &str =
    "export function on(cb: () => void) {\n  queueMicrotask(function () {\n    cb();\n  });\n}\n";
const USE_SITE_INLINE: &str =
    "export function on(cb: () => void) {\n  [0].forEach(function () {\n    cb();\n  });   \n}\n";

#[test]
fn a_callee_use_site_edit_in_another_file_moves_the_callers_roles() {
    same_length_edits_follow(
        "use-site",
        &[("App.tsx", USE_SITE_APP), ("b.ts", USE_SITE_DEFERRED)],
        "b.ts",
        &[USE_SITE_INLINE],
    );
}

/// ADR 0315: a callee that only tests an options property (`p.f && …`)
/// runs nothing of it there; one that does not (`p.f || …`) leaves the
/// literal unplaced. The edit flips one operator in the callee's file:
/// every AST table the late stages compare keeps its value
/// (`cache::late_stage_inputs_match`), and so does every Type Facts row.
const TESTED_APP: &str = "import { createSignal } from \"solid-js\";\nimport { on } from \"./b\";\nexport function App() {\n  const [count] = createSignal(0);\n  on({ f: () => {\n    count();\n  } });\n  return <div />;\n}\n";
const TESTED_AND: &str = "export function on(p: { f: () => void }) {\n  p.f && queueMicrotask(function () {\n    p.f();\n  });\n}\n";
const TESTED_OR: &str = "export function on(p: { f: () => void }) {\n  p.f || queueMicrotask(function () {\n    p.f();\n  });\n}\n";

#[test]
fn an_operator_edit_in_a_callee_moves_the_callers_roles() {
    same_length_edits_follow(
        "tested",
        &[("App.tsx", TESTED_APP), ("b.ts", TESTED_AND)],
        "b.ts",
        &[TESTED_OR],
    );
}

/// The use-site case read through a helper's summary: the literal calls a
/// helper that reads the signal, so the read is the caller's
/// interprocedural result at the helper's call, placed by the callee's use
/// of the literal in another file.
const STATE: &str = "import { createSignal } from \"solid-js\";\nexport const [count, setCount] = createSignal(0);\n";
const RESULT_HELPER: &str =
    "import { count } from \"./state\";\nexport function read() {\n  return count();\n}\n";
const RESULT_APP: &str = "import { read } from \"./helper\";\nimport { on } from \"./b\";\nexport function App() {\n  on(() => {\n    read();\n  });\n  return <div />;\n}\n";

#[test]
fn a_callee_use_site_edit_in_another_file_moves_the_callers_result() {
    same_length_edits_follow(
        "use-site-result",
        &[
            ("App.tsx", RESULT_APP),
            ("b.ts", USE_SITE_DEFERRED),
            ("helper.ts", RESULT_HELPER),
            ("state.ts", STATE),
        ],
        "b.ts",
        &[USE_SITE_INLINE],
    );
}
