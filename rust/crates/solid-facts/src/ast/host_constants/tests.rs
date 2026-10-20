use super::*;

fn fold(value: bool) -> Vec<HostConstantFold> {
    vec![HostConstantFold {
        specifier: "@solidjs/web".into(),
        imported: "isServer".into(),
        value,
    }]
}

#[test]
fn a_boolean_const_export_is_read_from_the_module() {
    let web = "const isServer = false;\nconst isDev = false;\nexport { isDev, isServer };\n";
    assert_eq!(
        exported_boolean_constant(Path::new("web.js"), web, "isServer"),
        Some(false)
    );
    let server = "export const isServer = true;\n";
    assert_eq!(
        exported_boolean_constant(Path::new("server.js"), server, "isServer"),
        Some(true)
    );
    // @solid-primitives/utils: not a literal.
    let utils = "const isServer = typeof window === \"undefined\";\nexport { isServer };\n";
    assert_eq!(
        exported_boolean_constant(Path::new("utils.js"), utils, "isServer"),
        None
    );
    let written = "let isServer = false;\nisServer = true;\nexport { isServer };\n";
    assert_eq!(
        exported_boolean_constant(Path::new("w.js"), written, "isServer"),
        None
    );
    let forwarded = "export { isServer } from \"./x.js\";\n";
    assert_eq!(
        exported_boolean_constant(Path::new("f.js"), forwarded, "isServer"),
        None
    );
    assert_eq!(
        exported_boolean_constant(
            Path::new("a.js"),
            "const a = true;\nexport { a as b };\n",
            "b"
        ),
        Some(true)
    );
}

#[test]
fn only_a_condition_bound_to_the_import_is_folded() {
    let source = "import { isServer } from \"@solidjs/web\";\n\
function guarded(fn) {\n  if (isServer) return;\n  onCleanup(fn);\n}\n\
function shadowed(fn) {\n  const isServer = fn();\n  if (isServer) return;\n  onCleanup(fn);\n}\n\
const isSupported = !isServer;\n\
function viaConst(fn) {\n  if (isSupported) {\n    onCleanup(fn);\n  }\n}\n";
    let browser = fold_host_constant_branches(Path::new("m.js"), source, &fold(false))
        .expect("browser folds");
    assert_eq!(browser.len(), source.len());
    assert!(browser.contains("if (!1      ) ;"), "{browser}");
    assert!(browser.contains("const isServer = fn();\n  if (isServer) return;"));
    assert!(
        browser.contains("if (!0         ) {\n    onCleanup(fn);"),
        "{browser}"
    );
    let node =
        fold_host_constant_branches(Path::new("m.js"), source, &fold(true)).expect("node folds");
    let guarded = &node[..node.find("function shadowed").unwrap()];
    assert!(guarded.contains("if (!0      ) return;"), "{guarded}");
    assert!(!guarded.contains("onCleanup"), "{guarded}");
    assert!(node.contains("const isServer = fn();\n  if (isServer) return;\n  onCleanup(fn);"));
    assert!(!node[node.find("function viaConst").unwrap()..].contains("onCleanup"));
}

#[test]
fn nothing_folds_without_a_bound_import() {
    let aliased =
        "import * as web from \"@solidjs/web\";\nfunction f() { if (web.isServer) return; g(); }\n";
    assert!(fold_host_constant_branches(Path::new("m.js"), aliased, &fold(true)).is_none());
    let other = "import { isServer } from \"@solid-primitives/utils\";\nfunction f() { if (isServer) return; g(); }\n";
    assert!(fold_host_constant_branches(Path::new("m.js"), other, &fold(true)).is_none());
    let plain = "function f() { if (false) return; g(); }\n";
    assert!(fold_host_constant_branches(Path::new("m.js"), plain, &fold(true)).is_none());
}

/// The Window-realm reading of every `if` test in `source`, in order:
/// `Some(value)` when the realm decides it, `None` otherwise.
fn realm_tests(path: &str, source: &str, acyclic: bool) -> Option<Vec<Option<bool>>> {
    let allocator = Allocator::default();
    let program = parse(&allocator, Path::new(path), source).expect("parses");
    let built = SemanticBuilder::new().build(&program);
    let answer = move || acyclic;
    let realm = WindowRealm { acyclic: &answer };
    let scope = HostConstantScope::in_window_realm(&program, &built.semantic, &[], &realm)?;
    let mut tests = Vec::new();
    for node in built.semantic.nodes().iter() {
        if let AstKind::IfStatement(statement) = node.kind() {
            tests.push(
                scope
                    .truthiness(&statement.test)
                    .and_then(|(value, folded)| folded.then_some(value)),
            );
        }
    }
    Some(tests)
}

fn realm_test(source: &str) -> Option<bool> {
    let tests = realm_tests("m.js", source, true).expect("scope");
    assert_eq!(tests.len(), 1, "{source}");
    tests[0]
}

#[test]
fn a_window_realm_decides_a_strict_typeof_comparison_with_undefined() {
    for (source, value) in [
        (
            "function f() { if (typeof window === \"undefined\") g(); }",
            false,
        ),
        (
            "function f() { if (typeof window !== \"undefined\") g(); }",
            true,
        ),
        (
            "function f() { if (\"undefined\" === typeof window) g(); }",
            false,
        ),
        (
            "function f() { if (typeof document === \"undefined\") g(); }",
            false,
        ),
        (
            "function f() { if (\"undefined\" !== typeof document) g(); }",
            true,
        ),
        (
            "function f() { if (!(typeof window === \"undefined\")) g(); }",
            true,
        ),
        (
            "function f() { if (!!(typeof window !== \"undefined\")) g(); }",
            true,
        ),
        (
            "function f() { if (((typeof (window)) === (\"undefined\"))) g(); }",
            false,
        ),
    ] {
        assert_eq!(realm_test(source), Some(value), "{source}");
    }
    let wrapped =
        "function f() { if ((typeof (window as any)) as unknown === \"undefined\") g(); }";
    assert_eq!(realm_tests("m.ts", wrapped, true).unwrap(), [Some(false)]);
}

#[test]
fn every_other_global_test_stays_undecided() {
    for source in [
        "function f() { if (typeof window == \"undefined\") g(); }",
        "function f() { if (typeof window != \"undefined\") g(); }",
        "function f() { if (typeof window === \"object\") g(); }",
        "function f() { if (typeof window < \"u\") g(); }",
        "function f() { if (typeof globalThis.window === \"undefined\") g(); }",
        "function f() { if (typeof globalThis === \"undefined\") g(); }",
        "function f() { if (typeof self === \"undefined\") g(); }",
        "function f() { if (typeof window.document === \"undefined\") g(); }",
        "function f() { if (typeof document !== \"undefined\" && document.body) g(); }",
        "function f() { if (typeof window === `undefined`) g(); }",
        "function f() { if (window === undefined) g(); }",
    ] {
        assert_eq!(realm_test(source), None, "{source}");
    }
}

#[test]
fn a_bound_window_or_document_is_not_the_global() {
    for source in [
        "function f(window) { if (typeof window === \"undefined\") g(); }",
        "function f({ document }) { if (typeof document === \"undefined\") g(); }",
        "function f(window = 1) { if (typeof window === \"undefined\") g(); }",
        "function f() { const window = undefined; if (typeof window === \"undefined\") g(); }",
        "function f() { try { g(); } catch (window) { if (typeof window === \"undefined\") g(); } }",
        "import { window } from \"x\";\nfunction f() { if (typeof window === \"undefined\") g(); }",
        "import * as document from \"x\";\nfunction f() { if (typeof document === \"undefined\") g(); }",
        "function f() { if (typeof window === \"undefined\") g(); }\nvar window;",
        "function f() { if (typeof document === \"undefined\") g(); }\nfunction document() {}",
        "function f() { if (typeof window === \"undefined\") g(); }\nclass window {}",
    ] {
        assert_eq!(realm_test(source), None, "{source}");
    }
    // A direct `eval` may declare anything: the module decides nothing.
    assert!(
        realm_tests(
            "m.js",
            "function f() { eval(\"\"); if (typeof window === \"undefined\") g(); }",
            true
        )
        .is_none()
    );
    // Outside a declared realm the comparison is no host constant.
    let source = "function f() { if (typeof window === \"undefined\") return; g(); }\n";
    assert!(fold_host_constant_branches(Path::new("m.js"), source, &[]).is_none());
    let allocator = Allocator::default();
    let program = parse(&allocator, Path::new("m.js"), source).unwrap();
    let built = SemanticBuilder::new().build(&program);
    let scope = HostConstantScope::new(&program, &built.semantic, &[]).unwrap();
    assert!(scope.is_empty());
}

/// `@tanstack/solid-query` 6.0.0-rc.0's `build/dev.js` (and `index.js`),
/// reduced to the kinds of statement before its `var isServer2` and three
/// of the guards after it.
const SOLID_QUERY_LAYOUT: &str = "import { MutationObserver } from '@tanstack/query-core';\n\
export * from '@tanstack/query-core';\n\
import { createContext, useContext, onCleanup } from 'solid-js';\n\
function createServerDehydrationChannel(client) { return client; }\n\
function createHydrationCoordinator(client) { return { client }; }\n\
var HydrationCoordinatorContext = createContext(null);\n\
var isServer = typeof window === \"undefined\";\n\
var QueryClientContext = createContext(null);\n\
var useQueryClient = (queryClient) => { if (queryClient) { return queryClient; } return useContext(QueryClientContext)(); };\n\
var QueryClientProvider = (props) => { onCleanup(() => props.client.unmount()); return isServer ? null : props.children; };\n\
var IsRestoringContext = createContext(() => false);\n\
var useIsRestoring = () => useContext(IsRestoringContext);\n\
var isServer2 = typeof window === \"undefined\";\n\
function _stripFnsForSSR(obj) {\n  if (!isServer2) return obj;\n  return {};\n}\n\
var hydratableObserverResult = (_query, result) => {\n  if (!isServer2) return result;\n  return {};\n};\n\
function useBaseQuery(options) {\n  if (isServer2) {\n    options.retry = false;\n  }\n  return options;\n}\n";

#[test]
fn a_var_alias_initialized_before_every_read_carries_the_realm() {
    assert_eq!(
        realm_tests("dev.js", SOLID_QUERY_LAYOUT, true).unwrap(),
        [None, Some(true), Some(true), Some(false)],
        "the first `if` reads a parameter"
    );
    // A const alias read at module level after its declaration has run
    // it. In a hoisted function it needs the module proof, as a `var`
    // does (review round 1): an import cycle may call it first.
    let constant = "const isServer = typeof window === \"undefined\";\nfunction f() { if (isServer) g(); }\nif (!isServer) g();\n";
    assert_eq!(
        realm_tests("m.js", constant, false).unwrap(),
        [None, Some(true)]
    );
    assert_eq!(
        realm_tests("m.js", constant, true).unwrap(),
        [Some(false), Some(true)]
    );
    // A read at module level after the declaration has run it.
    let after = "var isServer = typeof window === \"undefined\";\nif (isServer) g();\n";
    assert_eq!(realm_tests("m.js", after, true).unwrap(), [Some(false)]);
}

#[test]
fn a_var_alias_without_an_initialization_proof_stays_live() {
    let live = |source: &str| {
        let tests = realm_tests("m.js", source, true).unwrap();
        assert!(tests.iter().all(Option::is_none), "{source}: {tests:?}");
    };
    let tail =
        "var isServer = typeof window === \"undefined\";\nfunction f() { if (isServer) g(); }\n";
    // The controls below are each one change away from this positive.
    let prefix =
        "import { createContext, Context } from \"solid-js\";\nimport * as ns from \"solid-js\";\n";
    assert_eq!(realm_tests("m.js", tail, true).unwrap(), [Some(false)]);
    assert_eq!(
        realm_tests(
            "m.js",
            &format!("{prefix}var context = createContext(() => 0);\n{tail}"),
            true
        )
        .unwrap(),
        [Some(false)]
    );
    // An import cycle: a module importing this one back runs first.
    assert_eq!(realm_tests("m.js", tail, false).unwrap(), [None]);
    // Written, redeclared, `let`, exported, nested, or declared beside
    // another binding.
    live(&format!("{tail}function w() {{ isServer = false; }}\n"));
    live(&format!("{tail}var isServer;\n"));
    live("let isServer = typeof window === \"undefined\";\nfunction f() { if (isServer) g(); }\n");
    live(
        "var isServer = typeof window === \"undefined\", other = 1;\nfunction f() { if (isServer) g(); }\n",
    );
    live(
        "export var isServer = typeof window === \"undefined\";\nfunction f() { if (isServer) g(); }\n",
    );
    live(
        "function f() { var isServer = typeof window === \"undefined\"; if (isServer) g(); }\nfunction h() { f(); }\n",
    );
    // Read at module level before the declaration runs.
    live(&format!("if (isServer) g();\n{tail}"));
    live(&format!("var early = isServer;\n{tail}"));
    // Functions are hoisted, so callable before the declaration: any
    // statement before it that may call into this module refuses.
    for before in [
        "var early = f();\n",
        "f();\n",
        "var context = createContext(f);\n",
        "var context = createContext(() => f());\n",
        "var context = createContext(...[f]);\n",
        "var context = local(null);\nfunction local() {}\n",
        "var context = createContext?.(null);\n",
        "var context = ns.createContext(null);\n",
        "var context = new Context(null);\n",
        "var { a } = createContext(null);\n",
        "class Early { static { f(); } }\n",
        "class Early { static value = f(); }\n",
        "class Early extends Context {}\n",
        "class Early { [f()]() {} }\n",
        "export default f();\n",
        "if (true) f();\n",
    ] {
        live(&format!("{prefix}{before}{tail}"));
    }
}

#[test]
fn the_cycle_proof_is_asked_once_and_only_for_a_hoisted_read() {
    let asked = std::cell::Cell::new(0);
    let answer = || {
        asked.set(asked.get() + 1);
        true
    };
    let realm = WindowRealm { acyclic: &answer };
    // An arrow created after the declaration cannot run before it.
    let source =
        "const a = typeof window === \"undefined\";\nconst f = () => { if (a) return; g(); };\n";
    assert!(
        fold_host_constant_branches_in_window_realm(Path::new("m.js"), source, &[], &realm)
            .is_some()
    );
    assert_eq!(asked.get(), 0);
    let hoisted = "const a = typeof window === \"undefined\";\nfunction f() { if (a) return; g(); }\nfunction h() { if (a) return; g(); }\n";
    assert!(
        fold_host_constant_branches_in_window_realm(Path::new("m.js"), hoisted, &[], &realm)
            .is_some()
    );
    assert_eq!(asked.get(), 1);
    asked.set(0);
    let folded = fold_host_constant_branches_in_window_realm(
        Path::new("dev.js"),
        SOLID_QUERY_LAYOUT,
        &[],
        &realm,
    )
    .expect("folds");
    assert_eq!(asked.get(), 1);
    assert_eq!(folded.len(), SOLID_QUERY_LAYOUT.len());
    assert!(
        folded.contains("  if (!0        ) return obj;\n"),
        "{folded}"
    );
    assert!(folded.contains("  if (!1       ) ;\n"), "{folded}");
}

#[test]
fn load_specifiers_follow_static_loads_only() {
    let source = "import a from \"a\";\nimport \"./side.js\";\nimport type { T } from \"t\";\nexport * from \"b\";\nexport { c } from \"./c.js\";\nexport type { U } from \"u\";\nconst lazy = () => import(\"d\");\n";
    assert_eq!(
        module_load_specifiers(Path::new("m.ts"), source).unwrap(),
        ["./c.js", "./side.js", "a", "b"]
    );
    assert!(module_load_specifiers(Path::new("m.js"), "const x = require(\"a\");\n").is_none());
    assert!(module_load_specifiers(Path::new("m.ts"), "import x = require(\"a\");\n").is_none());
    assert!(
        module_load_specifiers(
            Path::new("m.js"),
            "const all = import.meta.glob(\"./*.js\");\n"
        )
        .is_none()
    );
    assert_eq!(
        module_load_specifiers(Path::new("m.js"), "const url = import.meta.url;\n").unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn a_const_alias_carries_the_realm_only_to_an_initialized_read() {
    let w = "typeof window === \"undefined\"";
    // Finding 1 of review round 1: the read is in the temporal dead zone
    // and throws, so the trap never reaches the store.
    let tdz = format!(
        "function f() {{ return {{ get() {{ if (isServer) return 1; const isServer = {w}; return 2; }} }}; }}\n"
    );
    assert_eq!(realm_tests("m.js", &tdz, true).unwrap(), [None]);
    let after = format!(
        "function f() {{ return {{ get() {{ const isServer = {w}; if (isServer) return 1; return 2; }} }}; }}\n"
    );
    assert_eq!(realm_tests("m.js", &after, true).unwrap(), [Some(false)]);
    // A closure created after the declaration exists only once it ran;
    // one created before it, or a hoisted function, may run first.
    let closure = format!("function f() {{ const a = {w}; return () => {{ if (a) g(); }}; }}\n");
    assert_eq!(realm_tests("m.js", &closure, false).unwrap(), [Some(false)]);
    let method = format!("function f() {{ const a = {w}; return {{ m() {{ if (a) g(); }} }}; }}\n");
    assert_eq!(realm_tests("m.js", &method, false).unwrap(), [Some(false)]);
    let before = format!(
        "function f() {{ const early = () => {{ if (a) g(); }}; early(); const a = {w}; }}\n"
    );
    assert_eq!(realm_tests("m.js", &before, true).unwrap(), [None]);
    let hoisted =
        format!("function f() {{ inner(); const a = {w}; function inner() {{ if (a) g(); }} }}\n");
    assert_eq!(realm_tests("m.js", &hoisted, true).unwrap(), [None]);
    // A `case` may be entered past the declaration.
    let switch = format!(
        "function f(x) {{ switch (x) {{ case 0: const a = {w}; break; case 1: if (a) g(); }} }}\n"
    );
    assert_eq!(realm_tests("m.js", &switch, true).unwrap(), [None]);
    // Module level: a hoisted function needs nothing before the
    // declaration to run module code, and no import cycle.
    let module = format!("const a = {w};\nfunction f() {{ if (a) g(); }}\n");
    assert_eq!(realm_tests("m.js", &module, true).unwrap(), [Some(false)]);
    assert_eq!(realm_tests("m.js", &module, false).unwrap(), [None]);
    let early = format!("f();\nconst a = {w};\nfunction f() {{ if (a) g(); }}\n");
    assert_eq!(realm_tests("m.js", &early, true).unwrap(), [None]);
    let exported = format!("export const a = {w};\nfunction f() {{ if (a) g(); }}\n");
    assert_eq!(realm_tests("m.js", &exported, true).unwrap(), [Some(false)]);
    let block = format!("{{ f(); const a = {w}; function f() {{ if (a) g(); }} }}\n");
    assert_eq!(realm_tests("m.js", &block, true).unwrap(), [None]);
    // Without the proof the alias reads as ADR 0166 reads it: an alias of
    // an imported host constant is still decided.
    let imported = "import { isServer } from \"@solidjs/web\";\nfunction f() { if (a) g(); const a = isServer; }\n";
    let allocator = Allocator::default();
    let program = parse(&allocator, Path::new("m.js"), imported).unwrap();
    let built = SemanticBuilder::new().build(&program);
    let answer = || true;
    let realm = WindowRealm { acyclic: &answer };
    let scope = HostConstantScope::in_window_realm(&program, &built.semantic, &fold(false), &realm)
        .unwrap();
    let test = built
        .semantic
        .nodes()
        .iter()
        .find_map(|node| match node.kind() {
            AstKind::IfStatement(statement) => Some(&statement.test),
            _ => None,
        })
        .unwrap();
    assert_eq!(scope.truthiness(test), Some((false, true)));
}

#[test]
fn a_dead_region_declaring_an_outer_binding_keeps_its_text() {
    let answer = || true;
    let realm = WindowRealm { acyclic: &answer };
    let fold_realm = |source: &str| {
        fold_host_constant_branches_in_window_realm(Path::new("m.js"), source, &[], &realm)
            .expect("folds")
    };
    // Finding 2 of review round 1: `var Reflect` is the function's own.
    let hoisted = "function get(t, p) {\n  if (typeof window === \"undefined\") { var Reflect; }\n  return Reflect.get(t, p);\n}\n";
    let folded = fold_realm(hoisted);
    assert!(folded.contains("{ var Reflect; }"), "{folded}");
    assert!(folded.contains("if (!1"), "{folded}");
    // So does a nested one, a function (hoisted out of a block in a
    // script), and a dead `else`.
    for source in [
        "function f() {\n  if (typeof window === \"undefined\") { if (x) { var y = 1; } }\n  return y;\n}\n",
        "function f() {\n  if (typeof window === \"undefined\") { function y() {} }\n  return y;\n}\n",
        "function f() {\n  if (typeof window !== \"undefined\") g(); else var y;\n  return y;\n}\n",
    ] {
        let folded = fold_realm(source);
        assert_eq!(
            folded.matches(" y").count(),
            source.matches(" y").count(),
            "{folded}"
        );
        assert_eq!(folded.len(), source.len());
    }
    // A block's own lexical declarations go with it.
    let local = "function f() {\n  if (typeof window === \"undefined\") { const y = 1; let z; class C {} g(y, z, C); }\n  return 1;\n}\n";
    let folded = fold_realm(local);
    assert!(!folded.contains("const y"), "{folded}");
    // After a live arm that returns, a dead class beside the `if` stays
    // (its name is the function's), and so does a statement holding a
    // `var`; a plain one goes.
    let tail = "function f() {\n  if (typeof window !== \"undefined\") return 1;\n  class C {}\n  { var v; }\n  g();\n  return [C, v];\n}\n";
    let folded = fold_realm(tail);
    assert!(folded.contains("class C {}"), "{folded}");
    assert!(folded.contains("{ var v; }"), "{folded}");
    assert!(!folded.contains("g();"), "{folded}");
    // ADR 0166's fold shares the rewrite.
    let imported = "import { isServer } from \"@solidjs/web\";\nfunction get(t, p) {\n  if (isServer) { var Reflect; }\n  return Reflect.get(t, p);\n}\n";
    let folded =
        fold_host_constant_branches(Path::new("m.js"), imported, &fold(false)).expect("folds");
    assert!(folded.contains("{ var Reflect; }"), "{folded}");
    assert!(folded.contains("if (!1"), "{folded}");
}

#[test]
fn load_specifiers_refuse_a_bound_require_and_an_unread_import_meta() {
    for source in [
        "import { createRequire } from \"node:module\";\nconst require = createRequire(import.meta.url);\nrequire(\"a\");\n",
        "import { createRequire as cr } from \"module\";\nconst load = cr(import.meta.url);\n",
        "import * as module from \"node:module\";\n",
        "function wrap(require) { return require(\"a\"); }\n",
        "const meta = import.meta;\nmeta.glob(\"./*.js\");\n",
        "const all = import.meta[\"glob\"](\"./*.js\");\n",
        "use(import.meta);\n",
    ] {
        assert!(
            module_load_specifiers(Path::new("m.js"), source).is_none(),
            "{source}"
        );
    }
    assert_eq!(
        module_load_specifiers(Path::new("m.js"), "const dev = import.meta.env.DEV;\n").unwrap(),
        Vec::<String>::new()
    );
}
