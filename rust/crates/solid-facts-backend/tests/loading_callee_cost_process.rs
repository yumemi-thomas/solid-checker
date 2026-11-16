//! ADR 1011: the cover walk reads a callee's body for its own `Loading` boundary, and a callee that hands its parameter on to itself (a
//! recursive visitor: `visit(node.left, cb); visit(node.right, cb)`) is walked once, not once per path. The first cut had no memo: every read in
//! a closure handed to the visitor re-walked the recursion, about 2^17 times, so twenty small components took minutes and three recursive calls
//! in the visitor did not finish.
//!
//! Work is counted, never timed ([`solid_reactive_ir::BuildTimings::callee_walks`]): the callee parameters whose body was read. It must be the
//! visitor's own parameter, once, whatever the number of components, reads and recursive calls.

use std::{env, fs, path::PathBuf};

use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};
use solid_reactive_ir::{AnalysisInputs, RuleOptions};

/// The fixture's declarations (`solid-js`, `solid-js/internal`, `@solidjs/web`, the JSX namespace), copied from the published typings.
const STUB: &str = include_str!("../../../../fixtures/reactive-ir/loading-member-callee/solid-js.d.ts");

const TSCONFIG: &str = r#"{"compilerOptions":{"jsx":"preserve","module":"ESNext","moduleResolution":"Bundler","lib":["DOM","ES2023"],"strict":true,"target":"ES2022"},"include":["*.ts","*.tsx"]}"#;

/// `components` components, each handing the visitor a closure with `reads` reads of an async memo; the visitor calls the closure and
/// recurses `children` times (so a path-wise walk of its parameter has `children^depth` paths).
fn program(components: usize, reads: usize, children: usize) -> String {
    let mut source = String::from(
        "import { createMemo } from \"solid-js\";\nimport { render } from \"@solidjs/web\";\n\
         type Tree = { v: number; left?: Tree; right?: Tree };\n\
         const tree: Tree = { v: 1 };\n\
         const slow = createMemo(async () => { await Promise.resolve(); return 1; });\n\
         function visit(node: Tree | undefined, cb: (v: number) => unknown): void {\n  if (!node) return;\n  cb(node.v);\n",
    );
    for child in 0..children {
        let member = if child % 2 == 0 { "left" } else { "right" };
        source.push_str(&format!("  visit(node.{member}, cb);\n"));
    }
    source.push_str("}\n");
    let body = "slow(); ".repeat(reads);
    for index in 0..components {
        source.push_str(&format!(
            "function C{index}() {{\n  return <p>{{(() => {{ visit(tree, () => {{ {body}}}); return 1; }})()}}</p>;\n}}\n"
        ));
    }
    for index in 0..components {
        source.push_str(&format!("render(() => <C{index} />, document.body);\n"));
    }
    source
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_recursive_visitor_is_walked_once() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = env::temp_dir()
        .join("sc-scratch.noindex")
        .join(format!("loading-callee-cost-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let _scratch = Scratch(root.clone());
    fs::write(root.join("solid-js.d.ts"), STUB).unwrap();
    fs::write(root.join("tsconfig.json"), TSCONFIG).unwrap();
    fs::write(root.join("App.tsx"), program(1, 1, 2)).unwrap();
    let root = root.canonicalize().unwrap();
    let project_id = root.join("tsconfig.json").to_string_lossy().into_owned();
    let typefacts = PathBuf::from(typefacts)
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let solid_v2 = dialect::by_id("solid-v2").unwrap();
    let app = root.join("App.tsx");
    let first = program(1, 1, 2);
    let typescript = TypeFactsSession::open(&typefacts, &project_id, &[]).unwrap();
    let mut session = NativeIncrementalSession::open(
        solid_v2,
        project_id,
        vec![SourceFile {
            path: app.to_string_lossy().into_owned(),
            source: first.into(),
            compiler_options: CompilerOptions::default(),
        }],
        typescript,
    )
    .unwrap();
    let mut walks = Vec::new();
    // Components x reads x recursive calls; the largest is the shape that took minutes (and three calls, hours).
    for (version, (components, reads, children)) in
        [(1, 1, 2), (8, 3, 2), (24, 3, 3), (24, 3, 6)].into_iter().enumerate()
    {
        let source = program(components, reads, children);
        fs::write(&app, &source).unwrap();
        let facts = session
            .edit(
                vec![SourceChange {
                    path: app.to_string_lossy().into_owned(),
                    version: version as u64 + 1,
                    source: Some(source.clone()),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let (built, timings) = solid_reactive_ir::build_with_analysis_inputs_measured(
            &facts,
            solid_v2.vocabulary,
            AnalysisInputs::empty(),
            &RuleOptions::default(),
        )
        .unwrap();
        // The premise: every read is a row the walk was asked about.
        assert!(
            built.reads.len() >= components * reads,
            "{components} components x {reads} reads: {} read rows",
            built.reads.len()
        );
        walks.push((components, reads, children, timings.callee_walks));
    }
    assert!(
        walks.iter().all(|(_, _, _, count)| *count == 1),
        "the visitor's parameter is walked once whatever the components, reads and recursive calls: {walks:?}"
    );
}
