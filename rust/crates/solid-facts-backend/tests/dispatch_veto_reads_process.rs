//! ADR 0450: a reader of the classified dispatch veto records the veto's two
//! answers, not the whole program. Every case edits a project in steps; each
//! retained build must equal a fresh one table by table (shadow mode off, so
//! the comparison is this test's), and the steps that leave the veto's answers
//! alone must leave the readers' memo answers reused.

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
  export function runWithOwner<T>(owner: unknown, fn: () => T): T | undefined;
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
            .join(format!("dispatch-veto-{tag}-{}", std::process::id()));
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

    /// Writes `edits` (a new name adds a source) and hands them to the
    /// session as one change.
    fn edit(&mut self, edits: &[(&str, &str)]) -> Arc<ProjectFacts> {
        self.version += 1;
        let changes = edits
            .iter()
            .map(|(name, source)| {
                let path = self.root.join(name);
                fs::write(&path, source).unwrap();
                SourceChange {
                    path: path.to_string_lossy().into_owned(),
                    version: self.version,
                    source: Some((*source).to_owned()),
                    compiler_options: CompilerOptions::default(),
                }
            })
            .collect();
        self.session.edit(changes, None).unwrap()
    }

    /// Deletes the source `name`.
    fn remove(&mut self, name: &str) -> Arc<ProjectFacts> {
        self.version += 1;
        let path = self.root.join(name);
        fs::remove_file(&path).unwrap();
        self.session
            .edit(
                vec![SourceChange {
                    path: path.to_string_lossy().into_owned(),
                    version: self.version,
                    source: None,
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
    let mut options = RuleOptions::default();
    options.runtime.program_boundary = Some(solid_reactive_ir::ProgramBoundary::Closed);
    let vocabulary = solid_v2().vocabulary;
    let (retained, timings) = builder
        .build_with_analysis_inputs_shared(facts, vocabulary, AnalysisInputs::empty(), &options)
        .unwrap();
    let (fresh, _) =
        build_with_analysis_inputs_measured(facts, vocabulary, AnalysisInputs::empty(), &options)
            .unwrap();
    eprintln!(
        "{label}: site roles {:?} veto {:?} sites {:?}",
        timings.flow_site_role_answers,
        timings.dispatch_veto_answers,
        fresh
            .event_dispatch_sites
            .iter()
            .map(|site| format!(
                "{}:{} {}/{:?} {}{}",
                site.path.rsplit('/').next().unwrap_or_default(),
                site.line,
                site.receiver,
                site.caller,
                u8::from(site.withholds_owner_proofs),
                u8::from(site.withholds_read_proofs)
            ))
            .collect::<Vec<_>>()
    );
    if let Some(difference) = first_difference(&retained, &fresh) {
        panic!("{label}: {difference}");
    }
    (fresh, timings)
}

/// What an edit does to the memo answers that read the dispatch veto.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Veto {
    /// The veto's two answers are the same as before: every reader's answer is
    /// reused, whatever the edit did to the sites behind them.
    Unmoved,
    /// An answer moved: readers rerun, and the retained build equals a fresh
    /// one.
    Moved,
}

struct Step<'a> {
    label: &'a str,
    edits: &'a [(&'a str, &'a str)],
    veto: Veto,
}

/// Opens `files`, then applies each step: every retained build equals a fresh
/// one, the veto memo moves exactly where `veto` says, and the call-site role
/// memo (the reader of the veto) reruns when it moved and is reused when it
/// did not.
fn steps_follow(tag: &str, files: &[(&str, &str)], steps: &[Step<'_>]) {
    let Some(mut project) = Project::scratch(tag, files) else {
        return;
    };
    // A verify-reused mismatch only panics in shadow mode `panic`
    // (`.cargo/config.toml`); any other mode logs and serves the fresh answer.
    assert_eq!(
        env::var("SOLID_CHECKER_IR_SHADOW").as_deref(),
        Ok("panic"),
        "{tag}: run with SOLID_CHECKER_IR_SHADOW=panic"
    );
    let mut builder = IncrementalBuilder::default();
    // Every reused memo answer is recomputed and compared with the kept one:
    // a reader of the veto that reuses a stale answer names its table.
    builder.verify_reused_memos(true);
    let first = project.analyze();
    let (_, opened) = retained_is_fresh(&mut builder, &first, &format!("{tag}: first"));
    assert!(
        opened.flow_site_role_answers[1] > 0 && opened.dispatch_veto_answers[1] > 0,
        "{tag}: the project has a reader of the veto and a hazard: {opened:?}"
    );
    for (index, step) in steps.iter().enumerate() {
        let label = format!("{tag}: {}", step.label);
        let facts = project.edit(step.edits);
        let (_, timings) = retained_is_fresh(&mut builder, &facts, &label);
        // The step's own build may not ask a reader whose answer it kept; a
        // touch of the callee's module does (its flow answer asks the call
        // site's role), and verify-reused recomputes that reused answer:
        // one recorded without the veto it read differs here.
        let touched = format!("{HANDLE}// touch {index}\n");
        let facts = project.edit(&[("handle.ts", &touched)]);
        retained_is_fresh(&mut builder, &facts, &format!("{label}, then a touch"));
        // Only now the step's own counts: a stale reader fails above, by name.
        let [reused, computed, _] = timings.flow_site_role_answers;
        match step.veto {
            Veto::Unmoved => {
                assert_eq!(timings.dispatch_veto_answers[2], 0, "{label}: {timings:?}");
                assert_eq!(computed, 0, "{label}: the readers are reused: {timings:?}");
                assert!(reused > 0, "{label}: the readers are asked: {timings:?}");
            }
            Veto::Moved => {
                assert_eq!(
                    timings.dispatch_veto_answers[2], 1,
                    "{label}: the veto moves: {timings:?}"
                );
                assert!(computed > 0, "{label}: the readers rerun: {timings:?}");
            }
        }
    }
}

/// A handler the button runs, through a callee in another module, that writes a
/// signal: its role (an event callback, or unplaced while a dispatch may hand
/// the delivery its caller's owner, ADR 0356) is the call site's, which the
/// call-site role memo answers, and the `writes` table states it.
const STATE: &str = "import { createSignal } from \"solid-js\";\nexport const [count, setCount] = createSignal(0);\n";
const APP: &str = "import { handle } from \"./handle\";\nexport function App() {\n  return <button onClick={() => handle()}>go</button>;\n}\n";
const HANDLE: &str = "import { count, setCount } from \"./state\";\nexport function handle() {\n  count();\n  setCount(1);\n}\n";
const OTHER: &str = "export const label = \"x\";\n";
const OTHER_EDITED: &str = "export const label = \"y\";\n";

/// A dispatch on the global `window` reaches no JSX handler (ADR 0435): the
/// hazard is present and the veto answers `(false, false)`.
const WINDOW_SITE: &str =
    "export function announce() {\n  window.dispatchEvent(new Event(\"x\"));\n}\n";
/// A click on an element nothing places: both proofs are withheld.
const ELEMENT_SITE: &str = "export function press(el: HTMLElement) {\n  el.click();\n}\n";
const NO_SITE: &str = "export function press(el: HTMLElement) {\n  void el;\n}\n";

/// An edit that moves no dispatch site and none of its inputs leaves the
/// readers unrun, whether the veto holds or not.
#[test]
fn an_edit_elsewhere_reuses_the_readers_of_the_veto() {
    for (tag, site) in [("held", ELEMENT_SITE), ("open", WINDOW_SITE)] {
        steps_follow(
            &format!("elsewhere-{tag}"),
            &[
                ("App.tsx", APP),
                ("handle.ts", HANDLE),
                ("state.ts", STATE),
                ("press.ts", site),
                ("other.ts", OTHER),
            ],
            &[
                Step {
                    label: "edit an unrelated file",
                    edits: &[("other.ts", OTHER_EDITED)],
                    veto: Veto::Unmoved,
                },
                Step {
                    label: "edit it back",
                    edits: &[("other.ts", OTHER)],
                    veto: Veto::Unmoved,
                },
            ],
        );
    }
}

/// A site added or removed that leaves the veto's two answers as they were
/// changes no reader's answer; one that moves them reruns every reader.
#[test]
fn adding_and_removing_a_dispatch_site_moves_the_veto_when_it_changes() {
    steps_follow(
        "add-remove",
        &[
            ("App.tsx", APP),
            ("handle.ts", HANDLE),
            ("state.ts", STATE),
            ("press.ts", WINDOW_SITE),
            ("other.ts", OTHER),
        ],
        &[
            Step {
                label: "add a click that withholds",
                edits: &[("click.ts", ELEMENT_SITE)],
                veto: Veto::Moved,
            },
            Step {
                // The veto already holds: a second site changes no answer.
                label: "add a second click that withholds",
                edits: &[("click2.ts", ELEMENT_SITE)],
                veto: Veto::Unmoved,
            },
            Step {
                label: "remove the first click: the second holds the veto",
                edits: &[("click.ts", NO_SITE)],
                veto: Veto::Unmoved,
            },
            Step {
                label: "remove the second click: only `window` is left",
                edits: &[("click2.ts", NO_SITE)],
                veto: Veto::Moved,
            },
        ],
    );
}

/// A dispatch written in an event handler leaves the veto (ADR 0437); written
/// outside it, in the component body, it holds it again. The edit is in the
/// file of the dispatch, not the reader's.
const PANEL_IN_HANDLER: &str = "export function Panel(props: { el: HTMLElement }) {\n  return <button onClick={() => {\n    props.el.click();\n  }}>x</button>;\n}\n";
const PANEL_IN_BODY: &str = "export function Panel(props: { el: HTMLElement }) {\n  props.el.click();\n  return <button onClick={() => {\n  }}>x</button>;\n}\n";

#[test]
fn moving_a_dispatch_out_of_an_event_handler_holds_the_veto() {
    steps_follow(
        "handler",
        &[
            ("App.tsx", APP),
            ("handle.ts", HANDLE),
            ("state.ts", STATE),
            ("panel.tsx", PANEL_IN_HANDLER),
            ("other.ts", OTHER),
        ],
        &[
            Step {
                label: "write the click in the component body",
                edits: &[("panel.tsx", PANEL_IN_BODY)],
                veto: Veto::Moved,
            },
            Step {
                label: "write it in the handler again",
                edits: &[("panel.tsx", PANEL_IN_HANDLER)],
                veto: Veto::Moved,
            },
        ],
    );
}

/// The two answers move one at a time: a callback of `runWithOwner(null, …)`
/// has no owner but leaves its window open, so only the read proof is held.
/// Both files name `runWithOwner`, so the project's primitives do not move
/// with the edit and the veto is the only input the readers see change.
const MICROTASK_SITE: &str = "import { runWithOwner } from \"solid-js\";\nvoid runWithOwner;\nexport function press(el: HTMLElement) {\n  queueMicrotask(() => {\n    el.click();\n  });\n}\n";
const DETACHED_SITE: &str = "import { runWithOwner } from \"solid-js\";\nexport function press(el: HTMLElement) {\n  runWithOwner(null, () => {\n    el.click();\n  });\n}\n";

#[test]
fn one_answer_of_the_veto_moving_reruns_the_readers() {
    steps_follow(
        "one-bit",
        &[
            ("App.tsx", APP),
            ("handle.ts", HANDLE),
            ("state.ts", STATE),
            ("press.ts", MICROTASK_SITE),
            ("other.ts", OTHER),
        ],
        &[
            Step {
                label: "run the dispatch under runWithOwner(null)",
                edits: &[("press.ts", DETACHED_SITE)],
                veto: Veto::Moved,
            },
            Step {
                label: "back to a scheduled callback",
                edits: &[("press.ts", MICROTASK_SITE)],
                veto: Veto::Moved,
            },
        ],
    );
}

/// A receiver's declared type decides whether `click()` is `HTMLElement.click`
/// (ADR 0356). The edit is in the file that declares the receiver's type; the
/// file that dispatches on it is not touched.
const TARGET_ELEMENT: &str = "export type Target = HTMLElement;\n";
const TARGET_PLAIN: &str = "export type Target = { click(): void };\n";
const TARGET_USE: &str = "import type { Target } from \"./target\";\nexport function press(target: Target) {\n  target.click();\n}\n";

#[test]
fn changing_a_receivers_type_in_another_file_moves_the_veto() {
    steps_follow(
        "receiver",
        &[
            ("App.tsx", APP),
            ("handle.ts", HANDLE),
            ("state.ts", STATE),
            ("announce.ts", WINDOW_SITE),
            ("target.ts", TARGET_ELEMENT),
            ("use.ts", TARGET_USE),
        ],
        &[
            Step {
                label: "the receiver is no element",
                edits: &[("target.ts", TARGET_PLAIN)],
                veto: Veto::Moved,
            },
            Step {
                label: "the receiver is an element again",
                edits: &[("target.ts", TARGET_ELEMENT)],
                veto: Veto::Moved,
            },
        ],
    );
}

/// The hazard itself: with no dispatch call anywhere the veto is not asked and
/// a reader records the hazard's probe alone. The first site, and the removal
/// of the file holding the last one, flip that probe, so every reader reruns
/// and equals a fresh build.
#[test]
fn the_first_dispatch_site_and_the_removal_of_the_last_move_the_readers() {
    let files = [
        ("App.tsx", APP),
        ("handle.ts", HANDLE),
        ("state.ts", STATE),
        ("other.ts", OTHER),
    ];
    let Some(mut project) = Project::scratch("hazard", &files) else {
        return;
    };
    assert_eq!(
        env::var("SOLID_CHECKER_IR_SHADOW").as_deref(),
        Ok("panic"),
        "run with SOLID_CHECKER_IR_SHADOW=panic"
    );
    let mut builder = IncrementalBuilder::default();
    builder.verify_reused_memos(true);
    let first = project.analyze();
    let (_, opened) = retained_is_fresh(&mut builder, &first, "hazard: first");
    assert!(opened.flow_site_role_answers[1] > 0, "{opened:?}");
    assert_eq!(
        opened.dispatch_veto_answers,
        [0, 0, 0],
        "no hazard, no veto"
    );

    let touch = |project: &mut Project, builder: &mut IncrementalBuilder, label: &str, n: u32| {
        let touched = format!("{HANDLE}// touch {n}\n");
        let facts = project.edit(&[("handle.ts", &touched)]);
        retained_is_fresh(builder, &facts, &format!("{label}, then a touch"));
    };

    let facts = project.edit(&[("click.ts", ELEMENT_SITE)]);
    let (_, added) = retained_is_fresh(&mut builder, &facts, "hazard: the first site");
    assert!(
        added.flow_site_role_answers[1] > 0,
        "readers rerun: {added:?}"
    );
    assert!(
        added.dispatch_veto_answers[1] > 0,
        "the veto is asked: {added:?}"
    );
    touch(&mut project, &mut builder, "hazard: the first site", 1);

    let facts = project.remove("click.ts");
    let (_, removed) =
        retained_is_fresh(&mut builder, &facts, "hazard: the last site's file removed");
    assert!(
        removed.flow_site_role_answers[1] > 0,
        "readers rerun: {removed:?}"
    );
    assert_eq!(
        removed.dispatch_veto_answers,
        [0, 0, 0],
        "no hazard, no veto"
    );
    touch(&mut project, &mut builder, "hazard: removed", 2);
}

/// ADR 0442, per function (ADR 0450): the role a write has in a function handed
/// to a component is the role of the component's call of it. The component's
/// file changes how it calls the prop; the file of the write is not touched,
/// so the write's chunk is reused only while its recorded answer holds.
const PROP_APP: &str = "import { setCount } from \"./state\";\nimport { Panel } from \"./panel\";\nexport function App() {\n  return <Panel onPress={() => setCount(1)} />;\n}\n";
const PANEL_EVENT: &str = "export function Panel(props: { onPress: () => void }) {\n  return <button onClick={() => props.onPress()}>x</button>;\n}\n";
const PANEL_BODY: &str = "export function Panel(props: { onPress: () => void }) {\n  props.onPress();\n  return <button>x</button>;\n}\n";

#[test]
fn a_write_flow_answer_follows_the_components_call_in_another_file() {
    let files = [
        ("App.tsx", PROP_APP),
        ("panel.tsx", PANEL_EVENT),
        ("state.ts", STATE),
    ];
    let Some(mut project) = Project::scratch("write-flow", &files) else {
        return;
    };
    assert_eq!(
        env::var("SOLID_CHECKER_IR_SHADOW").as_deref(),
        Ok("panic"),
        "run with SOLID_CHECKER_IR_SHADOW=panic"
    );
    let mut builder = IncrementalBuilder::default();
    builder.verify_reused_memos(true);
    let first = project.analyze();
    let (event, _) = retained_is_fresh(&mut builder, &first, "write-flow: event");
    for (label, panel) in [("body", PANEL_BODY), ("event again", PANEL_EVENT)] {
        let facts = project.edit(&[("panel.tsx", panel)]);
        let (now, _) = retained_is_fresh(&mut builder, &facts, &format!("write-flow: {label}"));
        let moved = first_difference(&event, &now).is_some();
        assert_eq!(
            moved,
            panel == PANEL_BODY,
            "write-flow: {label}: the answer moves with the call"
        );
        // A touch of the module that holds the write: its chunk asks the
        // function's answer again, and verify-reused recomputes the kept one.
        let touched = format!("{PROP_APP}// touch {label}\n");
        let facts = project.edit(&[("App.tsx", &touched)]);
        retained_is_fresh(
            &mut builder,
            &facts,
            &format!("write-flow: {label}, then a touch"),
        );
    }
}
