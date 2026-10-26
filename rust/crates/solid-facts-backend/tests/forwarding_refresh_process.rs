//! The interprocedural stage's kept passes (S7c: transport, the member
//! invocation refresh and its replayed forwarding loop, the node index)
//! against fresh builds, across the edits each must follow exactly: a
//! forwarding chain losing a middle edge, a veto flipping in the middle of
//! the chain, and a file added and removed in the middle of path order.
//!
//! Each step compares the retained build with a fresh one table by table
//! (shadow mode off, so the comparison is this test's), and checks that the
//! edit moved the fresh answer, so the step exercises what it names.

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
}

declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
  }
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
            .join(format!("forwarding-{tag}-{}", std::process::id()));
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

    /// Writes, adds (`Some` for a new name) or deletes (`None`) files.
    fn edit(&mut self, files: &[(&str, Option<&str>)]) -> Arc<ProjectFacts> {
        self.version += 1;
        let changes = files
            .iter()
            .map(|(name, source)| {
                let path = self.root.join(name);
                match source {
                    Some(source) => fs::write(&path, source).unwrap(),
                    None => fs::remove_file(&path).unwrap(),
                }
                SourceChange {
                    path: path.to_string_lossy().into_owned(),
                    version: self.version,
                    source: source.map(str::to_owned),
                    compiler_options: CompilerOptions::default(),
                }
            })
            .collect();
        self.session.edit(changes, None).unwrap()
    }
}

/// The retained builder's answer for `facts`, which must equal a fresh
/// build's table for table, and the fresh program.
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

/// A helper at the end of the chain invokes a member of its parameter; each
/// link hands its own parameter on whole. The component's call raises the
/// dispatch through the chain.
const D: &str = "export function d(p: { run(): void }) {\n  p.run();\n}\n";
const D_DEFERRED: &str =
    "export function d(p: { run(): void }) {\n  queueMicrotask(() => p.run());\n}\n";
const C: &str = "import { d } from \"./d\";\nexport function c(p: { run(): void }) {\n  d(p);\n}\n";
const C_SPREAD: &str =
    "import { d } from \"./d\";\nexport function c(p: { run(): void }) {\n  d(...[p]);\n}\n";
const B: &str = "import { c } from \"./c\";\nexport function b(p: { run(): void }) {\n  c(p);\n}\n";
const B_CUT: &str = "import { c } from \"./c\";\nexport function b(p: { run(): void }) {\n  c({ run() {} });\n  p;\n}\n";
const B_VIA_BB: &str =
    "import { bb } from \"./bb\";\nexport function b(p: { run(): void }) {\n  bb(p);\n}\n";
const BB: &str =
    "import { c } from \"./c\";\nexport function bb(p: { run(): void }) {\n  c(p);\n}\n";
const A: &str = "import { b } from \"./b\";\nexport function a(p: { run(): void }) {\n  b(p);\n}\n";
const APP: &str = "import { createSignal } from \"solid-js\";\nimport { a } from \"./a\";\nconst [count, setCount] = createSignal(0);\nexport function App() {\n  a({ run() { setCount(count() + 1); } });\n  return <div />;\n}\n";

#[test]
fn a_forwarding_chain_follows_mid_chain_edits_like_a_fresh_build() {
    let Some(mut project) = Project::scratch(
        "chain",
        &[
            ("App.tsx", APP),
            ("a.ts", A),
            ("b.ts", B),
            ("c.ts", C),
            ("d.ts", D),
        ],
    ) else {
        return;
    };
    let mut builder = IncrementalBuilder::default();
    let first = project.analyze();
    let chain = retained_is_fresh(&mut builder, &first, "chain");
    // A middle edge removed, then restored.
    let cut = project.edit(&[("b.ts", Some(B_CUT))]);
    let cut_program = retained_is_fresh(&mut builder, &cut, "mid-chain edge removed");
    assert!(
        first_difference(&chain, &cut_program).is_some(),
        "the cut edge must move the forwarded invocation"
    );
    let restored = project.edit(&[("b.ts", Some(B))]);
    retained_is_fresh(&mut builder, &restored, "mid-chain edge restored");
    // A veto in the middle: the argument is no longer exact.
    let vetoed = project.edit(&[("c.ts", Some(C_SPREAD))]);
    let vetoed_program = retained_is_fresh(&mut builder, &vetoed, "mid-chain veto");
    assert!(
        first_difference(&chain, &vetoed_program).is_some(),
        "the vetoed forwarding must move the forwarded invocation"
    );
    let unvetoed = project.edit(&[("c.ts", Some(C))]);
    retained_is_fresh(&mut builder, &unvetoed, "mid-chain veto lifted");
    // The end of the chain's invocation becomes excluded: a deferred
    // callback's dispatch is not reported through its callers.
    let deferred = project.edit(&[("d.ts", Some(D_DEFERRED))]);
    retained_is_fresh(&mut builder, &deferred, "end of chain excluded");
    let direct = project.edit(&[("d.ts", Some(D))]);
    retained_is_fresh(&mut builder, &direct, "end of chain included again");
    // A file added in the middle of path order (`bb.ts` sorts between `b.ts`
    // and `c.ts`), taking over the middle link, then removed again.
    let added = project.edit(&[("bb.ts", Some(BB)), ("b.ts", Some(B_VIA_BB))]);
    retained_is_fresh(&mut builder, &added, "file added mid path order");
    let removed = project.edit(&[("b.ts", Some(B)), ("bb.ts", None)]);
    let removed_program = retained_is_fresh(&mut builder, &removed, "file removed again");
    assert!(
        first_difference(&chain, &removed_program).is_none(),
        "back to the first chain"
    );
}
