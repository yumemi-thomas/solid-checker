//! Retained builds against fresh ones, under inputs that change *without* a
//! source edit that moves the TypeScript table.
//!
//! The retained Reactive IR is exact only if every input of a reused cache is
//! part of its key. `IncrementalBuilder` compares its answer with a fresh
//! build here explicitly (shadow mode off), so these tests do not depend on
//! `SOLID_CHECKER_IR_SHADOW`. Each case names the reuse path it exercises
//! (`docs/` and the IR's `ReusePlan`): the study that found them is
//! tsrs-plan/p5-ir-incremental-study.md section 2.5.

use std::{env, fs, path::PathBuf, sync::Arc};

use solid_facts::{ProjectFacts, compiler::CompilerOptions};
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};
use solid_reactive_ir::{
    AnalysisInputs, IncrementalBuilder, RuleOptions, RuntimeRendering,
    build_with_analysis_inputs_measured,
    shadow::{ShadowMode, first_difference},
};

fn solid_v2() -> &'static dialect::Dialect {
    dialect::by_id("solid-v2").expect("the 2.0 dialect is registered")
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir")
        .join(name)
}

struct Project {
    session: NativeIncrementalSession,
    sources: Vec<SourceFile>,
    version: u64,
    /// The scratch directory to remove on drop.
    scratch_root: Option<PathBuf>,
}

impl Drop for Project {
    fn drop(&mut self) {
        if let Some(root) = &self.scratch_root {
            let _ = fs::remove_dir_all(root);
        }
    }
}

const STUB: &str = r#"declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(compute: () => T): () => T;
  export function createReaction(onInvalidate: () => void): (tracking: () => void) => void;
}

declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
  }
}
"#;

impl Project {
    /// A private project written to a scratch directory: `files` plus the
    /// dialect stub and a tsconfig. `None` without a producer.
    fn scratch(tag: &str, files: &[(&str, &str)]) -> Option<Self> {
        Self::scratch_with(tag, files, false)
    }

    /// [`Self::scratch`] on a producer with per-file passes and derived keys
    /// (ADR 0389): Type Facts ids stay stable across an edit, so the memos
    /// can be reused.
    fn scratch_per_file(tag: &str, files: &[(&str, &str)]) -> Option<Self> {
        Self::scratch_with(tag, files, true)
    }

    fn scratch_with(tag: &str, files: &[(&str, &str)], per_file: bool) -> Option<Self> {
        let typefacts = env::var("SOLID_TYPEFACTS_BIN").ok()?;
        let root = env::temp_dir()
            .join("sc-scratch.noindex")
            .join(format!("incremental-{tag}-{}", std::process::id()));
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
        let typescript = if per_file {
            TypeFactsSession::open(
                "/usr/bin/env",
                &project_id,
                &[
                    "SOLID_TYPEFACTS_PER_FILE_CHECKERS=1".to_owned(),
                    "SOLID_TYPEFACTS_DERIVED_KEYS=1".to_owned(),
                    typefacts,
                ],
            )
        } else {
            TypeFactsSession::open(&typefacts, &project_id, &[])
        }
        .unwrap();
        let session =
            NativeIncrementalSession::open(solid_v2(), project_id, sources.clone(), typescript)
                .unwrap();
        Some(Self {
            session,
            sources,
            version: 0,
            scratch_root: Some(root),
        })
    }

    /// Replaces the whole source of `name`.
    fn rewrite(&mut self, name: &str, source: &str) -> Arc<ProjectFacts> {
        self.version += 1;
        let file = self
            .sources
            .iter()
            .find(|file| file.path.ends_with(name))
            .expect("a file of the project");
        self.session
            .edit(
                vec![SourceChange {
                    path: file.path.clone(),
                    version: self.version,
                    source: Some(source.to_owned()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap()
    }

    /// `None` without a producer (the process tests skip silently, as the rest
    /// of this crate's do).
    fn open(fixture_name: &str, files: &[&str]) -> Option<Self> {
        let typefacts = env::var("SOLID_TYPEFACTS_BIN").ok()?;
        let root = fixture(fixture_name);
        let project_id = root
            .join("tsconfig.json")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let sources = files
            .iter()
            .map(|name| {
                let path = root.join(name);
                SourceFile {
                    path: path.canonicalize().unwrap().to_string_lossy().into_owned(),
                    source: fs::read_to_string(path).unwrap().into(),
                    compiler_options: CompilerOptions::default(),
                }
            })
            .collect::<Vec<_>>();
        let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
        let session =
            NativeIncrementalSession::open(solid_v2(), project_id, sources.clone(), typescript)
                .unwrap();
        Some(Self {
            session,
            sources,
            version: 0,
            scratch_root: None,
        })
    }

    fn analyze(&mut self) -> Arc<ProjectFacts> {
        self.session.analyze().unwrap()
    }

    /// A new generation whose only change is trailing trivia in `index`'s file,
    /// so the TypeScript table is unchanged.
    fn touch(&mut self, index: usize) -> Arc<ProjectFacts> {
        self.version += 1;
        let file = &self.sources[index];
        self.session
            .edit(
                vec![SourceChange {
                    path: file.path.clone(),
                    version: self.version,
                    source: Some(format!("{}\n// touch {}\n", file.source, self.version)),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap()
    }
}

/// The retained builder's answer for `facts` under `options`, which must equal
/// a fresh build's, table for table.
fn assert_retained_is_fresh(
    builder: &mut IncrementalBuilder,
    facts: &ProjectFacts,
    options: &RuleOptions,
    label: &str,
) {
    builder.set_shadow_mode(ShadowMode::Off);
    let vocabulary = solid_v2().vocabulary;
    let (retained, _) = builder
        .build_with_analysis_inputs_shared(facts, vocabulary, AnalysisInputs::empty(), options)
        .unwrap();
    let (fresh, _) =
        build_with_analysis_inputs_measured(facts, vocabulary, AnalysisInputs::empty(), options)
            .unwrap();
    if let Some(difference) = first_difference(&retained, &fresh) {
        panic!("{label}: {difference}");
    }
}

fn options_rendering(rendering: Option<RuntimeRendering>) -> RuleOptions {
    let mut options = RuleOptions::default();
    options.runtime.rendering = rendering;
    options
}

/// 2.5, first path: `RuleOptions` are in the retained program's identity but
/// not in the cache domain or `ReusePlan`'s late-stage gate. A generation that
/// changes only trivia keeps the TypeScript table, so the late stages' cached
/// aggregates are offered for reuse under different rendering options.
#[test]
fn changing_rule_options_between_generations_never_reuses_a_stale_late_stage() {
    let Some(mut project) = Project::open("ssr-client-boundary", &["App.tsx"]) else {
        return;
    };
    let mut builder = IncrementalBuilder::default();
    let server = options_rendering(Some(RuntimeRendering::StringSsr));
    let client = options_rendering(Some(RuntimeRendering::Csr));
    let first = project.analyze();
    assert_retained_is_fresh(&mut builder, &first, &server, "cold, server rendering");
    let second = project.touch(0);
    assert_retained_is_fresh(&mut builder, &second, &client, "edit, client rendering");
    let third = project.touch(0);
    assert_retained_is_fresh(
        &mut builder,
        &third,
        &server,
        "edit, server rendering again",
    );
}

/// 2.5, second path: `runtime_symbol_redirects` are generation-request facts
/// outside the retained TypeScript snapshot. The index rebuild honours them
/// (a non-empty map forces it) but `ReusePlan` decided "TypeScript unchanged"
/// from the snapshot delta alone, so source discovery, reachability and the
/// late stages were offered results computed under the other redirects.
/// The redirected pairs are every ordered pair of the project's own symbols;
/// the ones that move the fresh answer are the ones that matter.
#[test]
fn changing_runtime_symbol_redirects_between_generations_never_reuses_a_stale_stage() {
    let Some(mut project) = Project::open("callback-forwarding", &["App.tsx", "source.ts"]) else {
        return;
    };
    let first = project.analyze();
    let ids = first
        .typescript
        .symbols()
        .filter(|symbol| {
            let declarations = symbol.declarations();
            !declarations.is_empty()
                && declarations
                    .iter()
                    .all(|declaration| !declaration.location.path.ends_with(".d.ts"))
        })
        .map(|symbol| symbol.id().to_owned())
        .collect::<Vec<_>>();
    assert!(ids.len() > 3, "the fixture declares its own symbols");
    let options = RuleOptions::default();
    let vocabulary = solid_v2().vocabulary;
    let mut mattered = 0;
    for from in &ids {
        for to in ids.iter().filter(|to| *to != from) {
            let plain = project.touch(0);
            let mut redirected = (*project.touch(0)).clone();
            redirected
                .runtime_symbol_redirects
                .insert(from.clone(), to.clone());
            let (without, _) = build_with_analysis_inputs_measured(
                &redirected,
                vocabulary,
                AnalysisInputs::empty(),
                &options,
            )
            .unwrap();
            let (with, _) = build_with_analysis_inputs_measured(
                &plain,
                vocabulary,
                AnalysisInputs::empty(),
                &options,
            )
            .unwrap();
            // Both directions: plain then redirected, redirected then plain.
            if first_difference(&without, &with).is_some() {
                mattered += 1;
            }
            let mut forward = IncrementalBuilder::default();
            assert_retained_is_fresh(&mut forward, &plain, &options, "plain");
            assert_retained_is_fresh(&mut forward, &redirected, &options, "then redirected");
            let mut backward = IncrementalBuilder::default();
            assert_retained_is_fresh(&mut backward, &redirected, &options, "redirected");
            let mut later = (*project.touch(0)).clone();
            later.runtime_symbol_redirects.clear();
            assert_retained_is_fresh(&mut backward, &later, &options, "then plain");
        }
    }
    assert!(mattered > 0, "some redirect must move the answer");
}

const SIGNAL_S: &str = "import { createSignal } from \"solid-js\";\nexport const [count, setCount] = createSignal(0);\n";
const LITERAL_G: &str = "export const ns = { run(callback: () => void) { callback(); } };\nns.run = (callback: () => void) => callback();\n";
const OPAQUE_G: &str =
    "declare const other: { run(callback: () => void): void };\nexport const ns = other;\n";
const CALLER_F: &str = "import { count } from \"./S\";\nimport { ns } from \"./G\";\nexport function App() {\n  ns.run(() => count());\n  return <div />;\n}\n";

/// 2.5, fourth path (closed by ADR 0386): `literal_receiver_members` says
/// which member calls have a receiver bound to an exact object literal, possibly
/// declared in another file, and `call_withheld` (interprocedural graph, result
/// reads, execution roles) reads it; the cross-file part is now in the digest. `F` is unchanged
/// when the receiver's module flips between a mutated literal (withheld) and an
/// opaque value, and the two builds differ (asserted), so a retained `F` would
/// be stale. Today nothing of `F` is retained (source discovery reuses 0 files
/// after the receiver module changes), so retained equals fresh. This pins the
/// scenario so the day that gate narrows (stable ids, then the digest's
/// removal) the shadow comparison fails here instead of in a project.
#[test]
fn a_receiver_that_stops_being_a_literal_in_another_file_does_not_keep_the_old_withholding() {
    let Some(mut project) = Project::scratch(
        "literal-receiver",
        &[("S.ts", SIGNAL_S), ("G.ts", LITERAL_G), ("F.tsx", CALLER_F)],
    ) else {
        return;
    };
    let options = RuleOptions::default();
    let vocabulary = solid_v2().vocabulary;
    let fresh = |facts: &ProjectFacts| {
        build_with_analysis_inputs_measured(facts, vocabulary, AnalysisInputs::empty(), &options)
            .unwrap()
            .0
    };
    let mut builder = IncrementalBuilder::default();
    let first = project.analyze();
    assert_retained_is_fresh(&mut builder, &first, &options, "literal");
    let opaque = project.rewrite("G.ts", OPAQUE_G);
    assert_retained_is_fresh(&mut builder, &opaque, &options, "opaque");
    assert!(
        first_difference(&fresh(&first), &fresh(&opaque)).is_some(),
        "the receiver's shape must change the caller's answer"
    );
    let literal = project.rewrite("G.ts", LITERAL_G);
    assert_retained_is_fresh(&mut builder, &literal, &options, "literal again");
}

/// ADR 0392: `B` calls the tracker `R` exports. Whether that call invokes
/// a function `createReaction` returned is decided in `R`, and `B` is
/// byte-identical across the edit. The binding-chain walk is a memo per file
/// whose reads include `R`'s binding and call, and the cross-file digest binds
/// its answers: editing `R` reruns exactly the answers that read it (`R`'s own
/// and `B`'s; `U`'s is reused), and the moved answer moves the digest, so
/// every fragment reruns. An edit of `U` moves no answer: the digest stays,
/// and every fragment but `U`'s is reused.
const TRACKER_R: &str = "import { createReaction } from \"solid-js\";\nexport const other = createReaction(() => {});\nexport const track = createReaction(() => {});\n";
// `R` still names `createReaction`, so the project keeps modelling returned
// callbacks and every file's answer is asked in every build.
const PLAIN_R: &str = "import { createReaction } from \"solid-js\";\nexport const other = createReaction(() => {});\nexport const track = (tracking: () => void) => tracking();\n";
const TRACKED_B: &str = "import { createSignal } from \"solid-js\";\nimport { track } from \"./R\";\nconst [count, setCount] = createSignal(0);\nexport function watch() {\n  track(() => setCount(count() + 1));\n}\n";
const UNRELATED_U: &str =
    "export const note = \"x\";\nexport function helper(value: number) {\n  return value + 1;\n}\n";

#[test]
fn a_returned_invocation_answer_moved_by_another_file_reruns_exactly_its_readers() {
    let Some(mut project) = Project::scratch_per_file(
        "returned-invocation",
        &[
            ("R.ts", TRACKER_R),
            ("B.ts", TRACKED_B),
            ("U.ts", UNRELATED_U),
        ],
    ) else {
        return;
    };
    let options = RuleOptions::default();
    let vocabulary = solid_v2().vocabulary;
    let fresh = |facts: &ProjectFacts| {
        build_with_analysis_inputs_measured(facts, vocabulary, AnalysisInputs::empty(), &options)
            .unwrap()
            .0
    };
    let mut builder = IncrementalBuilder::default();
    let mut retained = |facts: &ProjectFacts, label: &str| {
        builder.set_shadow_mode(ShadowMode::Off);
        let (program, timings) = builder
            .build_with_analysis_inputs_shared(facts, vocabulary, AnalysisInputs::empty(), &options)
            .unwrap();
        if let Some(difference) = first_difference(&program, &fresh(facts)) {
            panic!("{label}: {difference}");
        }
        timings
    };
    let first = project.analyze();
    retained(&first, "tracker");
    let plain = project.rewrite("R.ts", PLAIN_R);
    assert!(
        first_difference(&fresh(&first), &fresh(&plain)).is_some(),
        "whether `track` is a returned tracker must change `B`'s answer"
    );
    let timings = retained(&plain, "plain tracker");
    // Reused, recomputed, recomputed to a new answer: `U`; `R` and `B`; `B`.
    assert_eq!(timings.returned_invocation_files, [1, 2, 1], "{timings:?}");
    assert_eq!(timings.source_discovery_reused_files, 0, "{timings:?}");
    let back = project.rewrite("R.ts", TRACKER_R);
    let timings = retained(&back, "tracker again");
    assert_eq!(timings.returned_invocation_files, [1, 2, 1], "{timings:?}");
    let unrelated = project.rewrite("U.ts", &UNRELATED_U.replace("\"x\"", "\"y\""));
    let timings = retained(&unrelated, "unrelated edit");
    assert_eq!(timings.returned_invocation_files, [2, 1, 0], "{timings:?}");
    assert_eq!(timings.source_discovery_reused_files, 2, "{timings:?}");
}

/// 2.5, third path (docs/open-work.md): the interprocedural graph cache embeds
/// the accessor labels of argument symbols, not their accessor state. `A`
/// hands `count` to a helper, byte-identical across the edits, while `B`
/// flips `count` between a signal getter and a plain function. Since ADR
/// 0392 an edit in another file no longer discards every fragment (`H`'s
/// are reused here); `A`'s still reruns, through the Type Facts delta (the
/// semantics of `count` moved). This pins that `A`'s retained graph equals a
/// fresh one the day that delta narrows.
const SIGNAL_B: &str =
    "import { createSignal } from \"solid-js\";\nexport const [count] = createSignal(0);\n";
const PLAIN_B: &str = "export const count = (): number => 0;\n";
const HELPER_H: &str = "export function helper(read: () => number) {\n  return read();\n}\n";
const CALLER_A: &str = "import { count } from \"./B\";\nimport { helper } from \"./H\";\nexport function App() {\n  helper(count);\n  return <div />;\n}\n";

#[test]
fn an_argument_that_stops_being_an_accessor_in_another_file_is_not_kept_in_the_graph() {
    let Some(mut project) = Project::scratch_per_file(
        "accessor-label",
        &[("B.ts", SIGNAL_B), ("H.ts", HELPER_H), ("A.tsx", CALLER_A)],
    ) else {
        return;
    };
    let options = RuleOptions::default();
    let vocabulary = solid_v2().vocabulary;
    let fresh = |facts: &ProjectFacts| {
        build_with_analysis_inputs_measured(facts, vocabulary, AnalysisInputs::empty(), &options)
            .unwrap()
            .0
    };
    let mut builder = IncrementalBuilder::default();
    let first = project.analyze();
    assert_retained_is_fresh(&mut builder, &first, &options, "signal");
    let plain = project.rewrite("B.ts", PLAIN_B);
    assert_retained_is_fresh(&mut builder, &plain, &options, "plain");
    assert!(
        first_difference(&fresh(&first), &fresh(&plain)).is_some(),
        "whether `count` is an accessor must change `A`'s answer"
    );
    let signal = project.rewrite("B.ts", SIGNAL_B);
    assert_retained_is_fresh(&mut builder, &signal, &options, "signal again");
}
