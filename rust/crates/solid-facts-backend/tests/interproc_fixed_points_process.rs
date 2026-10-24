//! The interprocedural stage's fixed points (S7e: call, returned and
//! factory propagation, the exported obligations) and its graph chunks
//! against fresh builds, across the edits each must follow exactly: a
//! summary chain losing a middle edge, a veto flipping in the middle of the
//! chain, a file added and removed in the middle of path order, and a
//! same-length edit of a callee in another file that changes only syntax a
//! caller's graph chunk reads through the function table.
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
    p: Record<string, unknown>;
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
            .join(format!("fixed-points-{tag}-{}", std::process::id()));
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

/// A signal read at the end of a call chain reaches the component through
/// every link's summary (call propagation), a returned closure (returned
/// propagation) and a factory binding (factory propagation). The exported
/// helper `run` invokes a member of its parameter (the parameter-member
/// obligation).
const STATE: &str = "import { createSignal } from \"solid-js\";\nexport const [count, setCount] = createSignal(0);\n";
const C: &str = "import { count } from \"./state\";\nexport function c() {\n  return count();\n}\nexport function run(p: { go(): void }) {\n  p.go();\n}\n";
const C_DEFERRED: &str = "import { count } from \"./state\";\nexport function c() {\n  queueMicrotask(() => count());\n  return 0;\n}\nexport function run(p: { go(): void }) {\n  queueMicrotask(() => p.go());\n}\n";
const B: &str = "import { c } from \"./c\";\nexport function b() {\n  return c();\n}\nexport function later() {\n  return () => c();\n}\n";
const B_CUT: &str = "import { c } from \"./c\";\nexport function b() {\n  c;\n  return 0;\n}\nexport function later() {\n  return () => 0;\n}\n";
const B_DEFERRED: &str = "import { c } from \"./c\";\nexport function b() {\n  queueMicrotask(() => c());\n  return 0;\n}\nexport function later() {\n  return () => c();\n}\n";
const B_VIA_BB: &str = "import { bb } from \"./bb\";\nexport function b() {\n  return bb();\n}\nexport function later() {\n  return () => bb();\n}\n";
const BB: &str = "import { c } from \"./c\";\nexport function bb() {\n  return c();\n}\n";
const A: &str = "import { b, later } from \"./b\";\nexport function a() {\n  return b();\n}\nexport function makeReader() {\n  return later();\n}\n";
const APP: &str = "import { a, makeReader } from \"./a\";\nimport { run } from \"./c\";\nexport function App() {\n  const value = a();\n  const read = makeReader();\n  const again = read();\n  run({ go() {} });\n  return <div>{value}{again}</div>;\n}\n";

#[test]
fn summary_fixed_points_follow_mid_chain_edits_like_a_fresh_build() {
    let Some(mut project) = Project::scratch(
        "chain",
        &[
            ("App.tsx", APP),
            ("a.ts", A),
            ("b.ts", B),
            ("c.ts", C),
            ("state.ts", STATE),
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
        "the cut edge must move the propagated reads"
    );
    let restored = project.edit(&[("b.ts", Some(B))]);
    retained_is_fresh(&mut builder, &restored, "mid-chain edge restored");
    // A veto in the middle: the call runs outside the synchronous extent.
    let vetoed = project.edit(&[("b.ts", Some(B_DEFERRED))]);
    let vetoed_program = retained_is_fresh(&mut builder, &vetoed, "mid-chain veto");
    assert!(
        first_difference(&chain, &vetoed_program).is_some(),
        "the vetoed edge must move the propagated reads"
    );
    let unvetoed = project.edit(&[("b.ts", Some(B))]);
    retained_is_fresh(&mut builder, &unvetoed, "mid-chain veto lifted");
    // The seed and the member invocation become deferred at the end of the
    // chain: no row to propagate, no parameter-member obligation.
    let deferred = project.edit(&[("c.ts", Some(C_DEFERRED))]);
    let deferred_program = retained_is_fresh(&mut builder, &deferred, "end of chain deferred");
    assert!(
        first_difference(&chain, &deferred_program).is_some(),
        "the deferred seed must move the propagated reads"
    );
    let direct = project.edit(&[("c.ts", Some(C))]);
    retained_is_fresh(&mut builder, &direct, "end of chain direct again");
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

/// `useForwarded` hands its parameter to `readNow`, which calls it on every
/// completion: the component's call reads the signal (ADR 0285). Whether
/// the forward counts depends on the callee's syntax in `read.ts`, which the
/// caller's graph chunk reads through the function table: the callee must
/// be a function declaration whose binding nothing writes.
const READ: &str = "export function readNow(source: () => string) {\n  return source();\n}\nexport function touch() {\n  readNow;        \n}\n";
/// The same length, and every span before the write unmoved: `readNow` is
/// now written, so its value at the call is not proven to be the
/// declaration.
const READ_WRITTEN: &str = "export function readNow(source: () => string) {\n  return source();\n}\nexport function touch() {\n  readNow = touch;\n}\n";
const HELPERS: &str = "import { readNow } from \"./read\";\nexport function useForwarded(source: () => string) {\n  return readNow(source);\n}\n";
const FORWARDED_APP: &str = "import { createSignal } from \"solid-js\";\nimport { useForwarded } from \"./helpers\";\nexport function Forwarded() {\n  const [name] = createSignal(\"a\");\n  const label = useForwarded(name);\n  return <p>{label}</p>;\n}\n";

#[test]
fn a_graph_chunk_follows_a_same_length_callee_edit_in_another_file() {
    assert_eq!(READ.len(), READ_WRITTEN.len());
    let Some(mut project) = Project::scratch(
        "callee-syntax",
        &[
            ("App.tsx", FORWARDED_APP),
            ("helpers.ts", HELPERS),
            ("read.ts", READ),
        ],
    ) else {
        return;
    };
    let mut builder = IncrementalBuilder::default();
    let first = project.analyze();
    let forwarded = retained_is_fresh(&mut builder, &first, "forwarded");
    let written = project.edit(&[("read.ts", Some(READ_WRITTEN))]);
    let written_program = retained_is_fresh(&mut builder, &written, "callee binding written");
    assert!(
        first_difference(&forwarded, &written_program).is_some(),
        "a written callee binding must withdraw the forward"
    );
    let restored = project.edit(&[("read.ts", Some(READ))]);
    let restored_program = retained_is_fresh(&mut builder, &restored, "callee restored");
    assert!(
        first_difference(&forwarded, &restored_program).is_none(),
        "back to the forward"
    );
}
