use super::*;
use solid_facts::{
    compiler::ExecutionMap,
    core::{Generation, SourceHash, SourcePath},
};
use std::sync::Arc;

fn file(source: &str) -> FileFacts {
    let ast = ast::extract("provenance.ts", source).unwrap();
    FileFacts {
        generation: Generation::new(1).unwrap(),
        path: SourcePath::new("provenance.ts").unwrap(),
        source_hash: SourceHash::of(source),
        source: Arc::from(source),
        ast: Arc::new(ast),
        compiler: Arc::new(ExecutionMap {
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
fn visible_member_writes_use_normalized_targets_and_cooked_keys() {
    for source in [
        "x.rows = f;",
        "x.rows += 1;",
        "x.rows ||= f;",
        "++x.rows;",
        "x.rows--;",
        "[x.rows] = values;",
        "({ key: x.rows } = value);",
        "for (x.rows of values) {}",
        "for (x.rows in values) {}",
        "delete x.rows;",
        "x['rows'] = f;",
        "delete x['r\\u006fws'];",
        "x['rows' as const] = f;",
        "x['rows' satisfies string] = f;",
        "delete x['rows'!];",
        "x[(<string>'rows')] = f;",
        "x[`r\\u006fws`] = f;",
        "(x.rows as any) = f;",
    ] {
        let file = file(source);
        let mut index = MemberStability::default();
        index.syntax(&file, &crate::base::contributions::stability_writes(&file));
        assert!(
            matches!(index.state("rows"), Stability::Vetoed(_)),
            "{source}"
        );
        assert!(matches!(index.state("other"), Stability::Assumed));
    }
    for source in [
        "x.rows.other = f;",
        "x[x.rows] = f;",
        "const rows = x.rows;",
        "({ rows: x.other = x.rows } = value);",
        "delete x[key];",
        "x[key as string] = f;",
        "x[`r${key}ws`] = f;",
    ] {
        let file = file(source);
        let mut index = MemberStability::default();
        index.syntax(&file, &crate::base::contributions::stability_writes(&file));
        assert!(
            matches!(index.state("rows"), Stability::Assumed),
            "{source}"
        );
    }
}

#[test]
fn tuple_iteration_negative_targets_require_exact_separate_allocations() {
    for (source, separate) in [
        ("class Foo {} Foo.prototype.next = f;", true),
        ("class Foo {} const FP = Foo.prototype; FP.next = f;", true),
        (
            "class Foo {} const FP = Foo.prototype; (FP as any).next = f;",
            true,
        ),
        ("const other = () => {}; other.next = f;", true),
        (
            "const other = {}; const alias = other; alias.next = f;",
            true,
        ),
        ("const other = []; other.next = f;", true),
        ("const AP = Array.prototype; AP.next = f;", false),
        (
            "const IP = Object.getPrototypeOf([][Symbol.iterator]()); IP.next = f;",
            false,
        ),
        ("function patch(target) { target.next = f; }", false),
        ("class Foo {} Foo = Array; Foo.prototype.next = f;", false),
        (
            "let other = {}; other = Array.prototype; other.next = f;",
            false,
        ),
        ("const a = b; const b = a; a.next = f;", false),
    ] {
        let file = file(source);
        let member = file
            .ast
            .members
            .iter()
            .find(|member| {
                file.ast.write_targets.contains(&member.span)
                    && member_key(&file, member) == Some("next")
            })
            .unwrap();
        assert_eq!(
            separate_iteration_target(&file, member.object, 24),
            separate,
            "{source}"
        );
    }
    for key in [None, Some("next"), Some("return"), Some("__proto__")] {
        assert!(iteration_key(key));
    }
    for key in ["bar", "filter", "length", "0", "prototype", "iterator"] {
        assert!(!iteration_key(Some(key)), "{key}");
    }
}

#[test]
fn wildcard_target_requires_every_local_identity_input_to_be_distinct() {
    for (source, distinct) in [
        (
            "const location = {}; const other = {}; Object.assign(other, input);",
            true,
        ),
        (
            "const location = {}; const other = {}; const alias = other; Object.assign(alias, input);",
            true,
        ),
        (
            "const location = {}; const other = {}; Object.assign((other as object), input);",
            true,
        ),
        (
            "const location = {}; let target; target = {}; target = target; Object.assign(target, input);",
            true,
        ),
        (
            "const location = {}; Object.assign(location, input);",
            false,
        ),
        (
            "const location = {}; const other = location; Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; const other = input; Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; let other = {}; other = input; Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; let other = {}; ({ x: other } = input); Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; let other = {}; for (other of input) {} Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; export const other = {}; Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; const other = {}; eval(input); Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; function patch(other) { Object.assign(other, input); }",
            false,
        ),
        (
            "const location = {}; const other = {}; Object.assign(other || {}, input);",
            true,
        ),
        (
            "const location = {}; Object.assign(location || {}, input);",
            false,
        ),
        ("const location = {}; Object.assign(make(), input);", false),
        (
            "const location = {}; const other = function () {}; Object.assign(other, input);",
            true,
        ),
        (
            "const location = {}; const other = () => {}; Object.assign(other, input);",
            true,
        ),
        (
            "const location = {}; let other = () => {}; other = location; Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; declare const other: object; Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; Object.assign([{}, {}][\"01\"], input);",
            false,
        ),
        (
            "const location = {}; const holder = { patch() { Object.assign(this, input); } }; holder.patch();",
            false,
        ),
        (
            "const location = {}; const holder = { other: {}, patch() { Object.assign(this.other, input); } }; holder.patch();",
            true,
        ),
        (
            "const location = {}; const holder = { other: location, patch() { Object.assign(this.other, input); } }; holder.patch();",
            false,
        ),
        (
            "const location = {}; const holder = { other: {}, change() { this[key] = location; }, patch() { Object.assign(this.other, input); } }; holder.change(); holder.patch();",
            false,
        ),
        (
            "const location = {}; const holder = { patch() { Object.assign(this, input); } }; const borrowed = holder.patch; holder.patch();",
            false,
        ),
        (
            "const location = {}; const other = Math.random() ? {} : {}; Object.assign(other, input);",
            true,
        ),
        (
            "const location = {}; const other = Math.random() ? {} : location; Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; const other = {} && {}; Object.assign(other, input);",
            true,
        ),
        (
            "const location = {}; const other = {} && location; Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; const other = {} ?? {}; Object.assign(other, input);",
            true,
        ),
        (
            "const location = {}; const other = {} ?? location; Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; const [other] = [{}]; Object.assign(other, input);",
            true,
        ),
        (
            "const location = {}; const [other] = [location]; Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; const { other } = { other: {} }; Object.assign(other, input);",
            true,
        ),
        (
            "const location = {}; const { other } = { other: location }; Object.assign(other, input);",
            false,
        ),
        (
            "const location = {}; function patch({ other }) { Object.assign(other, input); } patch({ other: {} }); patch({ other: {} });",
            true,
        ),
        (
            "const location = {}; function patch({ other }) { Object.assign(other, input); } patch({ other: {} }); patch({ other: location });",
            false,
        ),
        (
            "const location = {}; function patch([other]) { Object.assign(other, input); } patch([{}]);",
            true,
        ),
        (
            "const location = {}; function patch([other]) { Object.assign(other, input); } patch([location]);",
            false,
        ),
        (
            "const location = {}; function patch(other) { Object.assign(other, input); } const escaped = patch; patch({});",
            false,
        ),
        (
            "const location = {}; const holder = { other: {} }; holder.other = location; Object.assign(holder.other, input);",
            false,
        ),
        (
            "const location = {}; const holder = { other: {} }; handoff(holder); Object.assign(holder.other, input);",
            false,
        ),
    ] {
        let file = file(source);
        let object = location(
            file.path.shared(),
            file.ast.bindings[0].initializer.unwrap(),
        );
        let target = file
            .ast
            .calls
            .iter()
            .find(|call| file.source_text(call.callee) == Some("Object.assign"))
            .unwrap()
            .arguments[0]
            .span;
        let facts = ProjectFacts {
            generation: file.generation,
            project_id: "fixture".into(),
            files: vec![file.clone()],
            typescript: solid_facts::TypeScriptTable::from_parts(
                3,
                1,
                "fixture",
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            typescript_changes: None,
            resolved_imports: None,
            runtime_resolutions: None,
            runtime_symbol_redirects: std::collections::HashMap::default(),
            dependency_source_roots: Vec::new(),
            dependency_source_call_errors: Vec::new(),
        };
        let entities = crate::EntitySymbols {
            by_path: HashMap::default(),
            exact_by_path: HashMap::default(),
            locations: crate::EntityLocations::default(),
        };
        let ast_indexes = HashMap::default();
        let names = HashMap::default();
        let dialect = solid_dialect::Solid2;
        let lookup = SemanticLookup::new(&facts, &ast_indexes, &entities, &names, &dialect, true);
        // A declaration at the same byte offsets in a different file is
        // not a back edge and must not short-circuit an unknown input.
        let mut visited = file
            .ast
            .bindings
            .iter()
            .flat_map(|binding| {
                binding
                    .names
                    .iter()
                    .map(|name| ("different-module.ts".to_owned(), name.span))
            })
            .collect();
        assert_eq!(
            mutation_target_is_distinct(&file, target, &object, &lookup, 32, &mut visited),
            distinct,
            "{source}"
        );
    }
}

#[test]
fn returned_members_require_the_entire_literal_to_be_closed() {
    for (source, admitted) in [
        ("const result = { rows, selected: () => rows() };", true),
        (
            "const result = { rows, selected: () => ({rows: other}) };",
            true,
        ),
        ("const result = { rows, ...other };", false),
        ("const result = { [key]: rows };", false),
        ("const result = { rows, rows: other };", false),
        ("const result = { get rows() { return read(); } };", false),
        ("const result = { rows() { return read(); } };", true),
        ("const result = { rows() { return this; } };", false),
        ("const result = { rows() { return arguments; } };", false),
        ("const result = { __proto__: other, rows };", false),
    ] {
        let file = file(source);
        let span = file.ast.bindings[0].initializer.unwrap();
        assert_eq!(
            closed_properties(&file, span).is_some(),
            admitted,
            "{source}"
        );
    }
}

#[test]
fn returned_array_census_requires_every_exact_slot_to_be_a_closed_object() {
    for (source, admitted) in [
        (
            "function make() { return [{ query: read }, { query: other }]; }",
            true,
        ),
        ("const make = () => ([{ query: read }] as const);", true),
        (
            "function make() { return [{ query: read }, ...other]; }",
            false,
        ),
        ("function make() { return [{ query: read }, ,]; }", false),
        ("function make() { return [{ query: read }, 0]; }", false),
        ("function make() { return [[{ query: read }]]; }", false),
        (
            "function make() { return [{ query: read, ...extra }]; }",
            false,
        ),
        ("function make() { return [{ [key]: read }]; }", false),
        (
            "function make() { return [{ get query() { return read; } }]; }",
            false,
        ),
        (
            "function make() { return [{ query() { return read(); } }]; }",
            true,
        ),
        (
            "function make() { return [{ query() { return this; } }]; }",
            false,
        ),
        ("function make() { return []; }", false),
    ] {
        let file = file(source);
        let returned = single_return(&file, &file.ast.functions[0]).unwrap();
        assert_eq!(
            returned_array_elements(&file, returned).is_some(),
            admitted,
            "{source}"
        );
    }
}

#[derive(Debug, PartialEq)]
enum Census {
    Here,
    AtCall,
    None,
}

#[test]
fn only_a_value_of_an_array_intrinsic_reaches_iteration() {
    use Census::{AtCall, Here, None};
    // Here: obtained and handed on in the file. AtCall: handed to a call,
    // recorded for the project's callee check (`handed_to_unseen_code`).
    for (source, expected) in [
        ("const s = Array.prototype.slice.call(x);", None),
        ("Array.isArray(x);", None),
        ("new Array(3); Array(3);", None),
        ("x instanceof Array;", None),
        ("x.constructor === Array;", None),
        ("x.constructor.name;", None),
        ("setTimeout(() => 1, 0);", None),
        ("Object.getPrototypeOf(x) === Object.prototype;", None),
        ("table[key] = value; run(table);", None),
        ("const x: Array<number> = [];", None),
        ("function f(x: Array<number>, g: Function): void {}", None),
        ("const y = x as Array<number>;", None),
        ("type T = Function | Array<string>;", None),
        ("const p = Array.prototype;", None),
        ("Array.prototype[Symbol.iterator] = f;", Here),
        ("f(Array.prototype);", AtCall),
        ("const A = Array;", None),
        ("f(Array);", AtCall),
        ("(globalThis as any).Array.prototype.x = 1;", None),
        ("window[k];", None),
        ("[].__proto__;", None),
        ("Object.getPrototypeOf([]);", None),
        ("Reflect.getPrototypeOf([]);", None),
        ("const g = Object.getPrototypeOf;", None),
        ("[].constructor.prototype;", None),
        ("new Function('x');", Here),
        ("Function('x');", Here),
        ("(0, eval)('x');", Here),
        ("setTimeout('x', 0);", Here),
        // ADR 0803 (item 3): obtained is not enough; the value must reach code
        // the checker does not see (exported, passed on, stored, returned), or
        // run a string. Each above that only obtains is no hand-out.
        ("export const p = Array.prototype;", Here),
        ("const A = Array; f(A);", AtCall),
        ("f(Object.getPrototypeOf);", AtCall),
        ("const o = { p: [].constructor.prototype };", Here),
        ("function g() { return Object.getPrototypeOf([]); }", Here),
        ("run([].__proto__);", AtCall),
        ("const p = Object.getPrototypeOf(x); p[k] = f;", Here),
        ("window[k](s);", Here),
        ("globalThis[k].call(null, s);", Here),
        ("class L extends Array {}", None),
        ("x.__proto__ = y;", None),
        ("Object.keys(Array.prototype);", None),
        // Review a9fc #1: the global object through an alias, a pattern, or a
        // descriptor; what a global object read for a test does not hand on.
        ("const g: any = globalThis; f(g.Array.prototype);", AtCall),
        ("const g: any = globalThis; g.Array.prototype[k] = f;", Here),
        ("globalThis.globalThis.Array.prototype[k] = f;", Here),
        ("const { getPrototypeOf } = Object;", Here),
        ("const { constructor: F } = () => {};", Here),
        ("const { Array: A } = globalThis;", Here),
        ("typeof window !== 'undefined';", None),
        // Review D3: the listener's `this` and event are the window.
        ("window.addEventListener('x', f);", AtCall),
        ("'ResizeObserver' in window;", None),
        ("if ('x' in globalThis) f();", None),
        // Recorded for the project's callee check (handed_to_unseen_code;
        // fixture iteration-written-slots/handed-*).
        ("bindEvent(window, 'scroll', f);", AtCall),
        // ADR 0803: a prototype held only to be compared or tested.
        (
            "function isPlainObject(obj) { let proto; return (obj != null && (!(proto = Object.getPrototypeOf(obj)) || proto === Object.prototype)); }",
            None,
        ),
        (
            "const isDict = (value) => value != null && (value = Object.getPrototypeOf(value), value === Array.prototype || value === Object.prototype);",
            None,
        ),
        (
            "const p = Object.getPrototypeOf(x); if (p === q || !p) f();",
            None,
        ),
        ("if (x === Array.prototype) f();", None),
        // ADR 0803: a reflective mutator's discarded target; the visible
        // side decides its keys.
        (
            "Object.assign(Array.prototype, { zzz() { return 1; } });",
            None,
        ),
        ("Reflect.set(Array.prototype, \"zzz\", 1);", None),
        (
            "const ok = Reflect.defineProperty(Array.prototype, \"zzz\", { value: 1 });",
            None,
        ),
        (
            "const AP = Object.assign(Array.prototype, { zzz() {} }); AP[Symbol.iterator] = f;",
            AtCall,
        ),
        ("run(Object.assign(Array.prototype, { zzz() {} }));", AtCall),
        (
            "const g = () => Object.defineProperty(Array.prototype, \"zzz\", { value: 1 });",
            AtCall,
        ),
        // Written through, handed on, or used as a value: still obtained.
        ("const p = Object.getPrototypeOf(x); p.next = f;", Here),
        ("const p = Object.getPrototypeOf(x); run(p);", AtCall),
        ("let p; (p = Object.getPrototypeOf(x), run(p));", AtCall),
        ("let p; run(p = Object.getPrototypeOf(x));", Here),
        (
            "let p; if (!(p = Object.getPrototypeOf(x))) {} p[k] = f;",
            Here,
        ),
        // Review D3: a timer calls a function with the window as `this`.
        ("setTimeout(function () { return 1; }, 0);", None),
        ("setTimeout(function () { this.Function(c)(); }, 0);", Here),
        ("setTimeout(() => this, 0);", None),
        ("setTimeout(tick, 0);", AtCall),
        // A method called on the window runs with it as `this`.
        ("window.scrollTo(0, 0);", AtCall),
        ("window.innerWidth;", None),
        ("window.addEventListener(\"x\", () => 1);", AtCall),
        ("addEventListener(\"x\", () => 1);", AtCall),
        // An event handler written on the window.
        ("window.onresize = null;", None),
        ("window.onresize = () => 1;", None),
        ("window.onresize = (e) => e.view;", Here),
        ("window.onresize = function () { this.x; };", Here),
        ("window.onresize = function () { return arguments; };", Here),
        ("window.onresize = handle;", Here),
        ("window[k] = 1;", Here),
        ("window.zzz = handle;", None),
        ("onmessage = function () { this.x; };", Here),
        ("onmessage = null;", None),
        ("Object.assign(window, { onclick: f });", AtCall),
        // The window's other names for itself.
        ("top.Function(c)();", Here),
        ("parent.Array.prototype[k] = f;", Here),
        ("window.top.Function(c)();", Here),
        ("const parent = node; parent.Function;", None),
        // Review D4: `this` read through to a global's name, in any file.
        ("export function f() { this.Array.prototype[k] = g; }", Here),
        ("export function f() { return this.Function(c); }", Here),
        ("function f() { this.name = 1; return this.values; }", None),
        // Sloppy code only (a script with no "use strict", outside classes):
        // a key no literal names, and `this` handed on.
        ("function f() { return this[k]; }", Here),
        ("export function f() { return this[k]; }", None),
        ("'use strict'; function f() { return this[k]; }", None),
        ("function f() { 'use strict'; return this[k]; }", None),
        ("class C { m() { return this[k]; } }", None),
        ("function f() { g.apply(this, arguments); }", Here),
        ("function f() { return g.bind(this); }", Here),
        ("function f() { g(this); }", AtCall),
        ("export function f() { g.apply(this, arguments); }", None),
        ("function f() { Object.keys(this); }", None),
        // A never-reassigned alias of sloppy `this` is `this`.
        ("function f() { var self = this; return self[k]; }", Here),
        ("function f() { var self = this; return self.Array; }", Here),
        (
            "function f() { var self = this; g.apply(self, arguments); }",
            Here,
        ),
        ("function f() { const self = this; g(self); }", AtCall),
        ("function f() { var self = this; return self.name; }", None),
        (
            "function f() { var self = this; self = o; return self[k]; }",
            None,
        ),
        (
            "export function f() { const self = this; return self[k]; }",
            None,
        ),
    ] {
        let (here, calls) = intrinsic_census(&file(source));
        let census = match (here, calls.is_empty()) {
            (true, _) => Here,
            (false, false) => AtCall,
            (false, true) => None,
        };
        assert_eq!(census, expected, "{source}");
    }
}

#[test]
fn returned_tuple_census_requires_every_slot_literal() {
    for (source, slots) in [
        (
            "function make() { const start = () => 1; return [read, start]; }",
            Some(2),
        ),
        (
            "function make() { return [read, () => 1] as const; }",
            Some(2),
        ),
        ("const make = () => [read, () => 1] as const;", Some(2)),
        ("function make() { return [read, , start]; }", None),
        ("function make() { return [read, ...rest]; }", None),
        ("function make() { return [...rest, start]; }", None),
        ("function make() { return []; }", None),
    ] {
        let file = file(source);
        let returned = single_return(&file, &file.ast.functions[0]).unwrap();
        assert_eq!(
            returned_tuple_elements(&file, returned).map(|elements| elements.len()),
            slots,
            "{source}"
        );
    }
}

#[test]
fn simple_array_binding_refuses_defaults_rest_nested_and_mutable() {
    for (source, admitted) in [
        ("const [running, start] = make();", true),
        ("const [, start] = make();", true),
        ("const [running, start] = make() as const;", true),
        ("let [running, start] = make();", false),
        ("var [running, start] = make();", false),
        ("const [running, start = other] = make();", false),
        ("const [running, ...rest] = make();", false),
        ("const [running, [start]] = make();", false),
        ("const [running, { start }] = make();", false),
    ] {
        let file = file(source);
        assert_eq!(
            simple_array_binding(&file.ast.bindings[0]),
            admitted,
            "{source}"
        );
    }
}

#[test]
fn tuple_slot_is_the_exact_arrow_the_factory_declares() {
    let source = "const outer = () => 0;
function make(parameter: () => number) {
  const start = () => 1;
  const typed = (() => 2) as () => number;
  let replaceable = () => 3;
  const expression = function () { return 4; };
  function declared() { return 5; }
  const wrapped = wrap(() => 6);
  return [start, typed, () => 9, (() => 10) as () => number, replaceable,
    expression, declared, wrapped, outer, parameter, async () => 11];
}";
    let file = file(source);
    let factory = file
        .ast
        .functions
        .iter()
        .find(|f| f.kind == ast::FunctionKind::Declaration && f.parameters.len() == 1)
        .unwrap();
    let returned = single_return(&file, factory).unwrap();
    let elements = returned_tuple_elements(&file, returned).unwrap();
    let admitted = elements
        .iter()
        .map(|element| slot_arrow(&file, factory.span, *element).is_some())
        .collect::<Vec<_>>();
    assert_eq!(
        admitted,
        [
            true, true, true, true, false, false, false, false, false, false, false
        ]
    );
    // A slot names the arrow itself, never the binding or the wrapper.
    let (arrow, declaration) = slot_arrow(&file, factory.span, elements[1]).unwrap();
    assert_eq!(file.source_text(arrow), Some("() => 2"));
    assert_eq!(declaration.and_then(|d| file.source_text(d)), Some("typed"));
    let (arrow, declaration) = slot_arrow(&file, factory.span, elements[3]).unwrap();
    assert_eq!(file.source_text(arrow), Some("() => 10"));
    assert_eq!(declaration, None);
}

#[test]
fn factory_return_census_excludes_alternatives_and_nested_returns() {
    for (source, admitted) in [
        ("function make() { return { rows }; }", true),
        (
            "function make() { const read = () => { return 1; }; return { rows }; }",
            true,
        ),
        (
            "function make() { if (flag) return { rows }; return other; }",
            false,
        ),
        ("function make() { if (flag) return { rows }; }", false),
        ("function make() { return; }", false),
    ] {
        let file = file(source);
        let function = file
            .ast
            .functions
            .iter()
            .find(|f| f.kind == ast::FunctionKind::Declaration)
            .unwrap();
        assert_eq!(
            single_return(&file, function).is_some(),
            admitted,
            "{source}"
        );
    }
}

#[test]
fn object_callable_definitions_never_supply_their_invocation_role() {
    for (source, stored) in [
        ("const decor = { header: () => read() };", true),
        ("const notes = () => read(); const decor = { notes };", true),
        (
            "const notes = () => read(); const decor = { notes: (notes as () => number) };",
            true,
        ),
        ("consume({ header: () => inline(() => read()) });", true),
        (
            "const notes = () => read(); function other(notes) { return { notes }; }",
            false,
        ),
        (
            "function Consumer() { const value = read(); return value; }",
            false,
        ),
    ] {
        let file = file(source);
        let read = file
            .ast
            .calls
            .iter()
            .find(|call| call.static_callee(&file.source) == Some("read"))
            .unwrap();
        assert_eq!(
            super::super::runs_in_object_callable(&file, read.span),
            stored,
            "{source}"
        );
    }
}

#[test]
fn iteration_intrinsic_target_names_array_prototype_exactly() {
    // Review a9fc #2: each spelling below is exactly `Array.prototype`, or the
    // array iterator's prototype; the alias is an immutable binding.
    for (expression, intrinsic) in [
        ("Array.prototype", true),
        ("Object.getPrototypeOf([])", true),
        ("Reflect.getPrototypeOf([])", true),
        ("([] as any).__proto__", true),
        ("[].constructor.prototype", true),
        ("Object.getPrototypeOf([][Symbol.iterator]())", true),
        ("Object.getPrototypeOf({})", false),
        ("({}).constructor.prototype", false),
        ("Object.getPrototypeOf(x)", false),
        ("x.__proto__", false),
    ] {
        let source = format!("const AP: any = {expression};\nAP[k] = f;\n");
        let facts = file(&source);
        let value = facts
            .ast
            .bindings
            .iter()
            .find_map(|binding| binding.initializer)
            .unwrap();
        assert_eq!(
            iteration_intrinsic_target(&facts, value, 24),
            intrinsic,
            "{expression}"
        );
    }
}
