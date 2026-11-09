//! ADR 0912: each function's site-reach answers are kept across edits while
//! every read their queries recorded answers as it did. A query reads other
//! files through the functions it follows from a call site of the function
//! (whether that site runs, whether the runtime refuses the creation there):
//! here a component rendered only in a callback a helper in another file
//! calls. The helper's body edit keeps the component's file unchanged and
//! moves whether the render is refused (called in place in an effect apply:
//! the component never runs) or not (deferred), so the retained build must
//! follow it. Without the reads made through the component's call site, the
//! kept answer stayed "never entered" (a plant no fuzz budget caught).

use std::{env, fs, path::PathBuf, sync::Arc};

use solid_facts::{ProjectFacts, compiler::CompilerOptions};
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};
use solid_reactive_ir::{
    AnalysisInputs, IncrementalBuilder, RuleOptions, build_with_analysis_inputs_measured,
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
            .join(format!("site-reach-reads-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("solid-js.d.ts"), STUB).unwrap();
        fs::write(
            root.join("tsconfig.json"),
            r#"{"compilerOptions":{"jsx":"preserve","module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"include":["*.ts","*.tsx"]}"#,
        )
        .unwrap();
        for (name, source) in files {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, source).unwrap();
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

/// The retained builder's program for `facts`, which must equal a fresh
/// build's table for table.
fn retained_is_fresh(
    builder: &mut IncrementalBuilder,
    facts: &ProjectFacts,
    label: &str,
) -> solid_reactive_ir::Program {
    builder.set_shadow_mode(ShadowMode::Off);
    let options = RuleOptions::default();
    let vocabulary = solid_v2().vocabulary;
    let (retained, _) = builder
        .build_with_analysis_inputs_shared(facts, vocabulary, AnalysisInputs::empty(), &options)
        .unwrap();
    let (fresh, _) =
        build_with_analysis_inputs_measured(facts, vocabulary, AnalysisInputs::empty(), &options)
            .unwrap();
    if let Some(difference) = first_difference(&retained, &fresh) {
        panic!("{label}: {difference}");
    }
    fresh
}

const APP: &str = "import { createEffect, createSignal } from \"solid-js\";\nimport { run } from \"./b\";\nconst [w, setW] = createSignal(0);\nfunction Child() {\n  setW(1);\n  return <p />;\n}\nexport function App() {\n  createEffect(() => 1, () => {\n    run(() => {\n      <Child />;\n    });\n  });\n  return <div>{w()}</div>;\n}\n";
const IN_PLACE: &str = "export function run(cb: () => void) {\n  cb();\n}\n";
const DEFERRED: &str = "export function run(cb: () => void) {\n  queueMicrotask(cb);\n}\n";

#[test]
fn a_helper_edit_in_another_file_moves_a_kept_site_reach_answer() {
    let Some(mut project) = Project::scratch("refused", &[("App.tsx", APP), ("b.ts", IN_PLACE)])
    else {
        return;
    };
    let mut builder = IncrementalBuilder::default();
    let first = project.analyze();
    let before = retained_is_fresh(&mut builder, &first, "in place");
    for step in 0..2 {
        let facts = project.edit("b.ts", DEFERRED);
        let after = retained_is_fresh(&mut builder, &facts, &format!("deferred {step}"));
        assert_ne!(
            before.unreached_sites, after.unreached_sites,
            "deferring the callback moves whether the render is refused"
        );
        let facts = project.edit("b.ts", IN_PLACE);
        let back = retained_is_fresh(&mut builder, &facts, &format!("in place {step}"));
        assert_eq!(
            before.unreached_sites, back.unreached_sites,
            "back to the first answer"
        );
    }
}
