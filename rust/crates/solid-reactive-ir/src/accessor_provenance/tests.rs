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
        index.syntax(&file);
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
        index.syntax(&file);
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
