//! Ports the `Source*` and async cases: `async_function_integration_test.go`,
//! the `Source*` cases of `hardening_integration_test.go` and
//! `tsgo/project_memo_test.go`. `compiler_facts_test.go` has no async or
//! `Source*` case (its cases are other families).

use super::project_support::{Scratch, change, loc};
use crate::backend::ClosureBackend;
use crate::context::Context;
use crate::project::Project;
use std::path::PathBuf;
use typefacts::Location;

const ASYNC_CONFIG: &str =
    r#"{"compilerOptions":{"strict":true,"target":"ES2022"},"include":["*.ts"]}"#;

fn slice<'a>(source: &'a str, location: &Location) -> &'a str {
    &source[location.start_byte as usize..location.end_byte as usize]
}

fn location_of(path: &str, source: &str, needle: &str) -> Location {
    let start = source
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found in fixture"));
    loc(path, start, start + needle.len())
}

#[test]
fn project_discovers_semantic_async_functions_and_post_await_calls() {
    let source = r#"
declare function after(): void;
declare function nested(): void;
declare function conditionalAfter(): void;
declare function dominatedRight(): void;
declare function methodAfter(): void;
declare function switchSibling(): void;
declare function switchDominated(): void;
const inline = async () => {
  await new Promise<void>(resolve => setTimeout(resolve, 1));
  after();
  queueMicrotask(() => nested());
};
const conditional = async (wait: boolean) => {
  wait && (await Promise.resolve());
  conditionalAfter();
};
const dominated = async (read: boolean) => {
  await Promise.resolve();
  read && dominatedRight();
};
const object = {
  async method() {
    await Promise.resolve();
    methodAfter();
  },
};
const switched = async (kind: "wait" | "read") => {
  switch (kind) {
    case "wait":
      await Promise.resolve();
      break;
    case "read":
      switchSibling();
      break;
  }
};
const fullyDominatedSwitch = async (kind: "first" | "second") => {
  switch (kind) {
    case "first":
      await Promise.resolve(1);
      break;
    case "second":
      await Promise.resolve(2);
      break;
    default:
      await Promise.resolve(3);
  }
  switchDominated();
};
function promised() { return Promise.resolve(1); }
type Stream<T> = AsyncIterable<T>;
declare function makeStream(): AsyncIterable<number>;
function aliasedStream(): Stream<number> { return makeStream(); }
function maybeStream(stream: boolean): AsyncIterable<number> | number {
  return stream ? makeStream() : 1;
}
function genericStream<T extends AsyncIterable<number>>(value: T): T { return value; }
interface CustomStream extends AsyncIterable<number> {}
function customStream(value: CustomStream): CustomStream { return value; }
"#;
    let scratch = Scratch::new(&[("tsconfig.json", ASYNC_CONFIG), ("async.ts", source)]);
    let path = scratch.path("async.ts");
    let mut project = scratch.open();
    let facts = project
        .source_async_functions(&Context::new(), &path)
        .unwrap();
    let count = |name: &str| {
        facts
            .iter()
            .flat_map(|fact| &fact.calls_after_await)
            .filter(|call| slice(source, call) == name)
            .count()
    };
    let async_functions = facts.iter().filter(|fact| fact.can_return_async).count();
    let promised_functions = facts
        .iter()
        .filter(|fact| !fact.symbol.is_empty() && fact.can_return_async)
        .count();
    assert!(
        async_functions >= 11
            && promised_functions >= 11
            && count("after") == 1
            && count("nested") == 0
            && count("conditionalAfter") == 0
            && count("dominatedRight") == 1
            && count("methodAfter") == 1
            && count("switchSibling") == 0
            && count("switchDominated") == 1,
        "facts={facts:#?}; async={async_functions} promised={promised_functions}"
    );
}

#[test]
fn project_looks_up_only_demanded_async_functions_and_aliases() {
    let source = r#"
declare function consume(callback: () => unknown): void;
declare function after(): void;
const unused = async () => Promise.resolve("unused");
const selected = () => Promise.resolve("selected");
const alias = selected;
consume(alias);
consume(() => Promise.resolve("inline"));
async function awaited() {
  await Promise.resolve();
  after();
}
"#;
    let scratch = Scratch::new(&[("tsconfig.json", ASYNC_CONFIG), ("lookup.ts", source)]);
    let path = scratch.path("lookup.ts");
    let mut project = scratch.open();
    let mut alias = location_of(&path, source, "alias);");
    alias.end_byte = alias.start_byte + "alias".len() as u64;
    let locations = vec![
        alias,
        location_of(&path, source, "() => Promise.resolve(\"inline\")"),
        location_of(&path, source, "await Promise.resolve()"),
    ];
    let ctx = Context::new();
    let facts = project.async_functions_at(&ctx, &locations).unwrap();
    let again = project.async_functions_at(&ctx, &locations).unwrap();
    assert_eq!(facts, again, "memoized lookup changed facts");
    let expressions: Vec<&str> = facts
        .iter()
        .map(|fact| slice(source, &fact.expression))
        .collect();
    let joined = expressions.join("\n");
    for wanted in [
        "() => Promise.resolve(\"selected\")",
        "selected",
        "() => Promise.resolve(\"inline\")",
        "async function awaited()",
    ] {
        assert!(
            joined.contains(wanted),
            "lookup facts omit {wanted:?}: {expressions:?}"
        );
    }
    assert!(
        !joined.contains("unused"),
        "lookup classified unrelated function: {expressions:?}"
    );
    let after_calls = facts
        .iter()
        .flat_map(|fact| &fact.calls_after_await)
        .filter(|call| slice(source, call) == "after")
        .count();
    assert_eq!(after_calls, 1, "after-await calls; facts={facts:#?}");
    // The second lookup answered from the per-location memo.
    let memo = &project.source_memo()[&path];
    assert_eq!(memo.async_at_len(), 3);
}

/// Pins the reachability of cross-path async facts, which is what the
/// closure's `crossPathAsync` bail-out defends against: a declaration is
/// found through `ValueDeclaration` and import aliases are never resolved, so
/// a module-kind project dead-ends at the import specifier. Script-kind files
/// share one global scope, so a demand in one file yields a fact whose
/// expression lives in another.
#[test]
fn async_functions_at_cross_file_declaration_in_script_files() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/retained-scripts")
        .canonicalize()
        .unwrap();
    let ctx = Context::new();
    let mut project = Project::open(
        &ctx,
        root.join("tsconfig.json").to_str().unwrap(),
        None,
        None,
    )
    .unwrap();
    let demanded = crate::gocompat::clean(root.join("use-alpha.ts").to_str().unwrap());
    let calls = project.source_calls(&ctx, &demanded).unwrap();
    let locations: Vec<Location> = calls.into_iter().map(|call| call.callee).collect();
    let facts = project.async_functions_at(&ctx, &locations).unwrap();
    let foreign: Vec<String> = facts
        .iter()
        .map(|fact| crate::gocompat::clean(&fact.expression.path))
        .filter(|path| *path != demanded)
        .collect();
    assert!(
        !foreign.is_empty(),
        "no async fact resolved outside the demanded file, so crossPathAsync is unreachable here; facts={facts:#?}"
    );
}

#[test]
fn project_discovers_parser_derived_call_and_argument_spans() {
    let source = "function wrap<T>(value: T, options: { nested: number }) { return value }\nwrap<string>(\"ok\", { nested: Number(1) })\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true},"include":["*.ts"]}"#,
        ),
        ("calls.ts", source),
    ]);
    let path = scratch.path("calls.ts");
    let ctx = Context::new();
    let mut project = scratch.open();
    let calls = project.source_calls(&ctx, &path).unwrap();
    let wrap = calls
        .iter()
        .find(|call| slice(source, &call.callee) == "wrap")
        .expect("wrap call");
    assert_eq!(wrap.arguments.len(), 2, "{:?}", wrap.arguments);
    assert_eq!(slice(source, &wrap.arguments[0]), "\"ok\"");
    assert_eq!(slice(source, &wrap.arguments[1]), "{ nested: Number(1) }");
    let declarations = project.declarations(&ctx, &wrap.target).unwrap().unwrap();
    assert!(
        !declarations.is_empty() && &*declarations[0].name == "wrap",
        "{declarations:?}"
    );
}

#[test]
fn project_discovers_call_initialized_bindings() {
    let source = "declare function pair(): [() => number, number, (value: number) => void]\nconst direct = pair()\nconst [read, , write] = pair()\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true},"include":["*.ts"]}"#,
        ),
        ("bindings.ts", source),
    ]);
    let path = scratch.path("bindings.ts");
    let mut project = scratch.open();
    let bindings = project.source_bindings(&Context::new(), &path).unwrap();
    assert_eq!(bindings.len(), 2, "{bindings:?}");
    assert!(
        !bindings[0].array
            && bindings[0].names.len() == 1
            && slice(source, &bindings[0].names[0]) == "direct",
        "{:?}",
        bindings[0]
    );
    assert!(
        bindings[1].array && bindings[1].names.len() == 3,
        "{:?}",
        bindings[1]
    );
    assert_eq!(slice(source, &bindings[1].names[0]), "read");
    assert!(
        bindings[1].names[1].path.is_empty(),
        "omitted slot keeps the zero location"
    );
    assert_eq!(slice(source, &bindings[1].names[2]), "write");
}

#[test]
fn project_discovers_named_block_bodied_functions() {
    let source = "export async function load(value: string) { return value }\nexport const View = async (props: { name: string }) => { return <div>{props.name}</div> }\nconst expression = (value: number) => value + 1\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true},"include":["*.tsx"]}"#,
        ),
        ("functions.tsx", source),
    ]);
    let path = scratch.path("functions.tsx");
    let mut project = scratch.open();
    let functions = project.source_functions(&Context::new(), &path).unwrap();
    assert_eq!(functions.len(), 2, "{functions:?}");
    for (function, want) in functions.iter().zip(["load", "View"]) {
        assert_eq!(slice(source, &function.name), want);
        assert!(
            function.exported
                && function.r#async
                && function.parameters.len() == 1
                && source.as_bytes()[function.body.start_byte as usize] == b'{'
                && source.as_bytes()[function.body.end_byte as usize] == b'}',
            "function {want} facts = {function:?}"
        );
    }
    assert!(!functions[0].arrow && functions[1].arrow, "{functions:?}");
}

#[test]
fn source_facts_of_a_file_outside_the_program_are_not_found() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", r#"{"include":["*.ts"]}"#),
        ("source.ts", "export const value = 1;\n"),
    ]);
    let mut project = scratch.open();
    let ctx = Context::new();
    let missing = scratch.path("missing.ts");
    for error in [
        project.source_calls(&ctx, &missing).unwrap_err(),
        project.source_bindings(&ctx, &missing).unwrap_err(),
        project.source_functions(&ctx, &missing).unwrap_err(),
        project.source_async_functions(&ctx, &missing).unwrap_err(),
    ] {
        assert!(error.0.starts_with("not found: source file"), "{error}");
    }
}

const MEMO_CONFIG: &str = r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}"#;

/// `TestSourceFactsMemoAcrossGenerations`.
#[test]
fn source_facts_memo_across_generations() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", MEMO_CONFIG),
        (
            "a.ts",
            "import { makeThing } from \"./b\";\n\nexport const value = makeThing();\n",
        ),
        (
            "b.ts",
            "export function makeThing(): number {\n  return 1;\n}\n",
        ),
        ("c.ts", "export const unrelated = 1;\n"),
    ]);
    let (a, b, c) = (
        scratch.path("a.ts"),
        scratch.path("b.ts"),
        scratch.path("c.ts"),
    );
    let ctx = Context::new();
    let mut project = scratch.open();

    let calls = project.source_calls(&ctx, &a).unwrap();
    assert_eq!(calls.len(), 1, "expected one source call in a.ts");
    let target = calls[0].target.clone();
    assert!(
        target.starts_with("symbol:h:"),
        "durable symbol id, got {target:?}"
    );
    let declarations = project.declarations(&ctx, &target).unwrap().unwrap();
    assert!(
        declarations.len() == 1 && declarations[0].location.path.ends_with("/b.ts"),
        "expected makeThing to declare in b.ts, got {declarations:?}"
    );
    let memo_has_calls = |project: &Project, path: &str| {
        project
            .source_memo()
            .get(path)
            .is_some_and(|memo| memo.has_calls())
    };
    let sees = |affected: &[String], name: &str| affected.iter().any(|path| path.ends_with(name));

    // every update clears the memo, and held ids still resolve
    let affected = project
        .update(&ctx, &[change(&c, 1, "export const unrelated = 2;\n")])
        .unwrap()
        .affected;
    assert!(
        !sees(&affected, "/a.ts"),
        "a.ts unexpectedly affected: {affected:?}"
    );
    assert!(
        project.source_memo().is_empty(),
        "an unrelated update kept memo entries"
    );
    assert_eq!(project.source_calls(&ctx, &a).unwrap(), calls);
    assert!(memo_has_calls(&project, &a), "the query refills the memo");
    assert_eq!(
        project.declarations(&ctx, &target).unwrap().unwrap(),
        declarations,
        "declarations diverged across generations"
    );

    // shape-preserving imported body edit
    let affected = project
        .update(
            &ctx,
            &[change(
                &b,
                1,
                "export function makeThing(): number {\n  return 2;\n}\n",
            )],
        )
        .unwrap()
        .affected;
    assert!(
        !sees(&affected, "/a.ts"),
        "shape-preserving edit unexpectedly affected a.ts: {affected:?}"
    );
    assert!(
        project.source_memo().is_empty(),
        "a shape-preserving edit kept memo entries"
    );
    let reused = project.source_calls(&ctx, &a).unwrap();
    assert_eq!(reused, calls);
    // The declaration's name span is unchanged, so the durable identity is too.
    assert_eq!(
        reused[0].target, target,
        "durable id changed for an unmoved declaration"
    );

    // exported signature change
    let affected = project
        .update(
            &ctx,
            &[change(
                &b,
                2,
                "export function makeThing(): string {\n  return \"changed\";\n}\n",
            )],
        )
        .unwrap()
        .affected;
    assert!(
        sees(&affected, "/a.ts"),
        "expected a.ts in affected set, got {affected:?}"
    );
    assert!(
        !project.source_memo().contains_key(&a),
        "memo entry for a.ts survived an exported signature change"
    );

    // multi-file update
    project.source_calls(&ctx, &a).unwrap();
    assert!(
        !project.source_memo().is_empty(),
        "expected memo entries before the multi-file update"
    );
    project
        .update(
            &ctx,
            &[
                change(&c, 3, "export const unrelated = 3;\n"),
                change(
                    &b,
                    3,
                    "export function makeThing(): number {\n  return 3;\n}\n",
                ),
            ],
        )
        .unwrap();
    assert!(
        project.source_memo().is_empty(),
        "expected an empty memo after a full rebuild, found {} entries",
        project.source_memo().len()
    );
    assert!(project.declarations(&ctx, &target).unwrap().is_none());
    project.source_calls(&ctx, &a).unwrap();
    assert!(
        project.declarations(&ctx, &target).unwrap().is_some(),
        "durable id failed to re-resolve after being minted again"
    );
}

/// The memo stores only fact sets whose identities are all durable, and keeps
/// each `Source*` family apart.
#[test]
fn memo_holds_each_family_separately() {
    let source = "export function run() {\n  return 1;\n}\nexport const value = run();\n";
    let scratch = Scratch::new(&[("tsconfig.json", MEMO_CONFIG), ("a.ts", source)]);
    let a = scratch.path("a.ts");
    let ctx = Context::new();
    let mut project = scratch.open();
    project.source_functions(&ctx, &a).unwrap();
    let memo = &project.source_memo()[&a];
    assert!(memo.has_functions() && !memo.has_calls() && !memo.has_bindings() && !memo.has_async());
    project.source_bindings(&ctx, &a).unwrap();
    project.source_async_functions(&ctx, &a).unwrap();
    let memo = &project.source_memo()[&a];
    assert!(memo.has_bindings() && memo.has_async());
}
