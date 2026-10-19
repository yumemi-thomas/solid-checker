
use super::*;

#[test]
fn named_receivers_use_binder_identity_and_refuse_escapes() {
    for (source, expected) in [
        (
            "const opts = { get count() { return n(); } }; new V(opts);",
            true,
        ),
        (
            "const opts = { get count() { return n(); } }; new V((opts as Options)!);",
            true,
        ),
        (
            "const opts = { get count() { return n(); } }; const alias = opts; new V(opts);",
            false,
        ),
        (
            "let opts = { get count() { return n(); } }; new V(opts);",
            false,
        ),
        ("const opts = new Proxy({}, {}); new V(opts);", false),
        (
            "const opts = { ...other, get count() { return n(); } }; new V(opts);",
            false,
        ),
        (
            "const opts = { get count() { return n(); } }; Object.defineProperty(opts, 'count', {}); new V(opts);",
            false,
        ),
        (
            "const opts = { get count() { return n(); } }; const copy = { ...opts }; new V(opts);",
            false,
        ),
        (
            "const opts = { get count() { return n(); } }; new V(opts); effect(() => ({ ...opts }));",
            true,
        ),
        (
            "const opts = { get count() { return n(); } }; before(); new V(opts); function before() { return { ...opts }; }",
            false,
        ),
        (
            "const opts = { get count() { return n(); } }; while (again) { new V(opts); }",
            false,
        ),
        (
            "const opts = { get count() { return n(); } }; { const opts = other; new V(opts); }",
            false,
        ),
        // A local inside an exported function is not exported; the
        // exported module-level binding is.
        (
            "export function App() { const opts = { get count() { return n(); } }; new V(opts); }",
            true,
        ),
        (
            "export const opts = { get count() { return n(); } }; new V(opts);",
            false,
        ),
        // Direct `eval` may delete a named receiver's getter first.
        (
            "const opts = { get count() { return n(); } }; eval('delete opts.count'); new V(opts);",
            false,
        ),
        ("new V({ get count() { return n(); } }); eval('1');", true),
    ] {
        let facts = file(source);
        let call = facts
            .ast
            .calls
            .iter()
            .find(|call| call.construct && call.arguments.len() == 1)
            .unwrap();
        assert_eq!(receiver(&facts, call, 0).is_some(), expected, "{source}");
    }
}

fn file(source: &str) -> solid_facts::FileFacts {
    use solid_facts::core::{Generation, SourceHash, SourcePath};
    let ast = solid_facts::ast::extract("helper.tsx", source).unwrap();
    solid_facts::FileFacts {
        generation: Generation::new(1).unwrap(),
        path: SourcePath::new("helper.tsx").unwrap(),
        source_hash: SourceHash::of(source),
        source: std::sync::Arc::from(source),
        ast: std::sync::Arc::new(ast),
        compiler: std::sync::Arc::new(solid_facts::compiler::ExecutionMap {
            compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of(source),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        }),
    }
}

#[test]
fn receiver_routes_are_matched_as_tokens() {
    assert!(mentions_receiver_route("{ delete this.count; }"));
    assert!(mentions_receiver_route("this"));
    assert!(mentions_receiver_route("{ return super.valueOf(); }"));
    assert!(mentions_receiver_route("{ return th\\u0069s.x; }"));
    assert!(!mentions_receiver_route("{ return thisValue(); }"));
    assert!(!mentions_receiver_route("{ return history() + superb(); }"));
    assert!(!mentions_receiver_route("{ return $this; }"));
}

#[test]
fn copying_data_function_values_does_not_select_their_bodies() {
    let facts = file("new V({ fn: () => n(), get count() { return n(); } });");
    let call = facts.ast.calls.iter().find(|call| call.construct).unwrap();
    let shape = receiver(&facts, call, 0).unwrap();
    assert!(shape.properties[0].getter.is_none());
    assert!(shape.properties[1].getter.is_some());
}

#[test]
fn a_parameter_yields_gets_only_when_every_reference_is_a_static_get() {
    for (source, expected) in [
        ("function h(o) { return o.a; }", vec![(0, "a")]),
        (
            "function h(a, o) { const x = o.b; return x + o.a; }",
            vec![(1, "b"), (1, "a")],
        ),
        ("const h = (o) => o.a;", vec![(0, "a")]),
        (
            "function h(o) { return (o as T).a + o!.b; }",
            vec![(0, "a"), (0, "b")],
        ),
        ("function h(o) { return o.a ?? 0; }", vec![(0, "a")]),
        ("function h(o) { return o.a.b; }", vec![(0, "a")]),
        // A `try` block runs to its end under the normal-completion
        // premise (ADR 0279, option (b)); its `catch` stays conditional.
        (
            "function h(o) { try { return o.a; } finally {} }",
            vec![(0, "a")],
        ),
        (
            "function h(o) { try { return 0; } catch { return o.a; } }",
            vec![],
        ),
        // Some paths only: the parameter is proven only read, but no Get
        // runs on every completion.
        ("function h(f, o) { if (f) return o.a; return 0; }", vec![]),
        ("function h(o) { return c && o.a; }", vec![]),
        ("function h(o) { return () => o.a; }", vec![]),
        (
            "function h(o) { for (const x of xs) { o.b; } return o.a; }",
            vec![(0, "a")],
        ),
        (
            "function h(o) { for (const x of xs) { break; } return o.a; }",
            vec![],
        ),
        ("function h(o) { return <i>{o.a}</i>; }", vec![]),
        ("function h(o) { class C { v = o.a } return C; }", vec![]),
        ("function h(o) { const { x = o.a } = q; return x; }", vec![]),
        ("function h(o, x = o.a) { return x; }", vec![]),
        // A reference that is not a static Get refuses the parameter.
        ("function h(o) { sink(o); return o.a; }", vec![]),
        ("function h(o) { const p = o; return p.a; }", vec![]),
        ("function h(o) { return { ...o }.a + o.a; }", vec![]),
        ("function h(o, k) { return o[k] + o.a; }", vec![]),
        ("function h(o) { return o['a']; }", vec![]),
        ("function h(o) { return o?.a; }", vec![]),
        ("function h(o) { o.m(); return o.a; }", vec![]),
        ("function h(o) { o.m`x`; return o.a; }", vec![]),
        ("function h(o) { o.a = 1; return 0; }", vec![]),
        ("function h(o) { o.a += 1; return o.b; }", vec![]),
        ("function h(o) { o.a++; return o.b; }", vec![]),
        ("function h(o) { o.a.b = 1; return o.b; }", vec![]),
        ("function h(o) { delete o.a; return o.b; }", vec![]),
        ("function h(o) { [o.a] = [1]; return o.b; }", vec![]),
        ("function h(o) { for (o.a of xs) {} return o.b; }", vec![]),
        (
            "function h(o) { return typeof o === 'object' && o.a; }",
            vec![],
        ),
        ("function h(o) { return o.\\u0061; }", vec![]),
        // The parameter or the function is refused whole.
        ("function h({ a }) { return a; }", vec![]),
        ("function h(o = {}) { return o.a; }", vec![]),
        ("function h(...o) { return o.a; }", vec![]),
        ("function h(o) { o = x; return o.a; }", vec![]),
        ("function h(o) { var o; return o.a; }", vec![]),
        ("function h(o) { return arguments[0].a + o.a; }", vec![]),
        (
            "function h(o) { return argum\\u0065nts.length + o.a; }",
            vec![],
        ),
        ("function h(o) { return o.a; } eval('1');", vec![]),
        ("function h(o) { (eval as F)('1'); return o.a; }", vec![]),
        ("async function h(o) { return o.a; }", vec![]),
        ("function* h(o) { yield o.a; }", vec![]),
        ("function h(o) { 'use server'; return o.a; }", vec![]),
        ("function h(o) { 'use strict'; return o.a; }", vec![]),
        ("'use client'; function h(o) { return o.a; }", vec![]),
    ] {
        let facts = file(source);
        let function = facts
            .ast
            .functions
            .iter()
            .find(|f| f.name.is_some() || f.kind == solid_facts::ast::FunctionKind::Arrow)
            .unwrap();
        let found = parameter_get_sites(&facts, function)
            .into_iter()
            .map(|(parameter, key, _)| (parameter, key))
            .collect::<Vec<_>>();
        assert_eq!(found, expected, "{source}");
    }
}

#[test]
fn a_getter_read_counts_only_on_the_getters_every_completion_path() {
    for (source, expected) in [
        ("({ get a() { return s(); } });", true),
        ("({ get a() { const v = s(); return v + 1; } });", true),
        ("({ get a() { return c ? s() : 0; } });", false),
        ("({ get a() { if (c) return 1; return s(); } });", false),
        ("({ get a() { return c && s(); } });", false),
        ("({ get a() { return () => s(); } });", false),
    ] {
        let facts = file(source);
        let shape = &facts.ast.object_get_shapes[0];
        let getter = getter_function(&facts, shape.properties[0].getter.unwrap()).unwrap();
        let read = facts
            .ast
            .calls
            .iter()
            .find(|call| facts.source_text(call.callee) == Some("s"))
            .unwrap();
        // The lookup-free part of `site_on_every_completion`.
        let on_path = facts.ast.completion_sites.binary_search(&read.span).is_ok()
            && containing_ast_function(&facts.ast, read.span)
                .is_some_and(|owner| owner.span == getter.span)
            && body_site_runs_during_call(&facts, getter, read.span);
        assert_eq!(on_path, expected, "{source}");
    }
}
