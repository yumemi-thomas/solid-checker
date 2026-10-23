use super::*;
use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
use solid_facts::core::{Generation, SourceHash, SourcePath};

const PATH: &str = "app.tsx";

/// The span of the `index`-th occurrence of `needle` in `source`.
fn span_of(source: &str, needle: &str, index: usize) -> Span {
    let start = source
        .match_indices(needle)
        .nth(index)
        .expect("needle occurs in source")
        .0;
    Span::new(
        u32::try_from(start).unwrap(),
        u32::try_from(start + needle.len()).unwrap(),
    )
}

fn project(source: &str) -> ProjectFacts {
    let generation = Generation::new(1).unwrap();
    ProjectFacts {
        generation,
        project_id: "fixture".into(),
        files: vec![FileFacts {
            generation,
            path: SourcePath::new(PATH).unwrap(),
            source_hash: SourceHash::of(source),
            source: Arc::from(source),
            ast: Arc::new(solid_facts::ast::extract(PATH, source).unwrap()),
            compiler: Arc::new(ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: Vec::new(),
                untracked_regions: Vec::new(),
                discarded_regions: Vec::new(),
                ownership_regions: Vec::new(),
                callback_roles: Vec::new(),
                jsx_operations: Vec::new(),
            }),
        }],
        typescript: TypeScriptTable::from_parts(
            3,
            1,
            "fixture",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        typescript_changes: None,
        resolved_imports: None,
        runtime_resolutions: None,
        runtime_symbol_redirects: std::collections::HashMap::default(),
        dependency_source_roots: Vec::new(),
    }
}

/// Every listed span names its symbol exactly: these fixtures describe spans
/// whose entity covers them, so `exact` and `get` agree.
fn entity_symbols(spans: &[(Span, &str)]) -> EntitySymbols {
    let entries: HashMap<(u64, u64), SymbolId> = spans
        .iter()
        .map(|(span, symbol)| {
            (
                (u64::from(span.start), u64::from(span.end)),
                SymbolId::from(*symbol),
            )
        })
        .collect();
    EntitySymbols {
        by_path: HashMap::from_iter([(PATH.to_string(), entries.clone())]),
        exact_by_path: HashMap::from_iter([(PATH.to_string(), entries)]),
    }
}

/// Builds a lookup over one synthetic file and runs `body` against it.
fn with_lookup<T>(
    source: &str,
    spans: &[(Span, &str)],
    body: impl FnOnce(&SemanticLookup<'_>) -> T,
) -> T {
    let facts = project(source);
    let entities = entity_symbols(spans);
    let ast_indexes = HashMap::default();
    let symbol_names = HashMap::default();
    let dialect = solid_dialect::Solid2;
    let lookup = SemanticLookup::new(
        &facts,
        &ast_indexes,
        &entities,
        &symbol_names,
        &dialect,
        false,
    );
    body(&lookup)
}

#[test]
fn exact_object_member_calls_require_a_closed_const_receiver() {
    for (source, admitted) in [
        (
            "export const obj = { run() {} }; function use() { obj.run(); }",
            true,
        ),
        (
            "const obj = { run: () => 1 }; function use() { obj.run(); }",
            true,
        ),
        (
            "const obj = { 'run': () => 1 }; function use() { obj.run(); }",
            true,
        ),
        (
            "function run() {} const obj = { run }; function use() { obj.run(); }",
            true,
        ),
        (
            "function use() { function bad() { keep(this); } const obj = { run() {}, bad }; obj.bad(); obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { *run() {} }; obj.run(); }",
            false,
        ),
        (
            "const obj = { run: function () {} }; function use() { obj.run(); }",
            true,
        ),
        (
            "const obj = { run() {} }; function use() { obj?.run?.(); }",
            true,
        ),
        (
            "const obj = { run() {} }; function use() { (obj as unknown as typeof obj).run(); }",
            true,
        ),
        (
            "function use() { const obj = { run() {} }; obj.run(); }",
            true,
        ),
        ("const obj = { run() {} }; obj.run();", false),
        (
            "function early() { obj.run(); } try { early(); } catch {} const obj = { run() {} };",
            false,
        ),
        (
            "import * as ns from './unknown'; const obj = { run() {} }; function use() { obj.run(); }",
            false,
        ),
        (
            "export * from './unknown'; const obj = { run() {} }; function use() { obj.run(); }",
            false,
        ),
        (
            "const obj = { run() {} }; function use() { obj.run(); } function load() { return import('./unknown'); }",
            false,
        ),
        (
            "const obj = { run() {} }; function use() { obj.run(); } require('./unknown');",
            false,
        ),
        (
            "function use() { function enter() { obj.run(); } enter(); const obj = { run() {} }; }",
            false,
        ),
        (
            "function use() { const obj = { run() {} }; function enter() { obj.run(); } enter(); }",
            false,
        ),
        (
            "function use(v) { switch(v) { case 0: const obj = { run() {} }; case 1: obj.run(); } }",
            false,
        ),
        (
            "function use() { let obj = { run() {} }; obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { run() {} }; obj = other; obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { run() {} }; obj.run = other; obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { run() {} }; alias.run = other; obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { run() {} }; Object.assign(obj, other); obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { run() {} }; keep(obj); obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { run() {} }; const alias = obj; obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { run() {} }; const f = obj.run; obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { [key]() {} }; obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { get run() { return f; } }; obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { run() {}, ...other }; obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { ...other, run() {} }; obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { run() { keep(this); } }; obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = { run() {} }; eval('obj.run = other'); obj.run(); }",
            false,
        ),
        (
            "function use() { const obj = flag ? { run() {} } : null; obj?.run(); }",
            false,
        ),
        (
            "function use() { const obj = { run() {} }; obj['run'](); }",
            false,
        ),
    ] {
        let ast = solid_facts::ast::extract(PATH, source).unwrap();
        let spans = ast
            .identifiers
            .iter()
            .filter(|id| &source[id.span.start as usize..id.span.end as usize] == "obj")
            .map(|id| (id.span, "object"))
            .collect::<Vec<_>>();
        with_lookup(source, &spans, |lookup| {
            assert_eq!(
                !lookup.object_member_targets().is_empty(),
                admitted,
                "{source}"
            );
        });
    }
}

/// Call sites keyed by target function span, as `(callee start, end)`.
/// One entry per invocation, which is what every consumer but the escape
/// test reads.
fn call_sites(source: &str, spans: &[(Span, &str)]) -> Vec<(Span, Vec<(u32, u32)>)> {
    with_lookup(source, spans, |lookup| {
        let mut sites = lookup
            .all_function_call_sites()
            .iter()
            .map(|((_, function), sites)| {
                let mut callees = sites
                    .iter()
                    .map(|site| (site.callee.start, site.callee.end))
                    .collect::<Vec<_>>();
                callees.sort_unstable();
                (*function, callees)
            })
            .collect::<Vec<_>>();
        sites.sort_by_key(|(function, _)| (function.start, function.end));
        sites
    })
}

/// Every reference to `function` that its call sites account for — the set
/// the escape test in `attribution` tests membership in.
fn accounted_references(source: &str, spans: &[(Span, &str)], function: Span) -> Vec<(u32, u32)> {
    with_lookup(source, spans, |lookup| {
        let mut references = lookup
            .function_call_site_references(PATH, function)
            .into_iter()
            .map(|(_, span)| (span.start, span.end))
            .collect::<Vec<_>>();
        references.sort_unstable();
        references
    })
}

#[test]
fn a_jsx_tag_resolving_to_a_project_function_is_a_call_site() {
    let source =
        "function Panel() {\n  return <p />;\n}\nfunction App() {\n  return <Panel />;\n}\n";
    let declaration = span_of(source, "Panel", 0);
    let tag = span_of(source, "Panel", 1);
    let panel = span_of(source, "function Panel() {\n  return <p />;\n}", 0);

    assert_eq!(
        call_sites(source, &[(declaration, "panel"), (tag, "panel")]),
        vec![(panel, vec![(tag.start, tag.end)])]
    );
}

#[test]
fn a_component_used_as_a_value_is_not_a_call_site() {
    // The rendered tag is `Wrap`, and only `Wrap` gains the edge: `Panel`
    // appears as an attribute value, which renders nothing by itself.
    let source = concat!(
        "function Panel() {\n  return <p />;\n}\n",
        "function Wrap(props) {\n  return <p />;\n}\n",
        "function App() {\n  return <Wrap child={Panel} />;\n}\n",
    );
    let panel_declaration = span_of(source, "Panel", 0);
    let panel_value = span_of(source, "Panel", 1);
    let wrap_declaration = span_of(source, "Wrap", 0);
    let wrap_tag = span_of(source, "Wrap", 1);
    let wrap = span_of(source, "function Wrap(props) {\n  return <p />;\n}", 0);

    assert_eq!(
        call_sites(
            source,
            &[
                (panel_declaration, "panel"),
                (panel_value, "panel"),
                (wrap_declaration, "wrap"),
                (wrap_tag, "wrap"),
            ]
        ),
        vec![(wrap, vec![(wrap_tag.start, wrap_tag.end)])]
    );
}

#[test]
fn a_resolvable_dotted_tag_is_an_edge_whose_callee_is_the_whole_name() {
    // TypeScript reports the component's symbol at both the whole
    // `ns.Panel` name and the `Panel` property inside it, so the tag does
    // resolve and the edge is emitted — with the whole name as its callee,
    // because that is the span the tag names. The property reference is a
    // *different* span, which is why the escape test in `attribution`
    // still reports the enumeration incomplete for a dotted render: that
    // test is byte-exact span membership, not a containment check.
    let source =
        "function Panel() {\n  return <p />;\n}\nfunction App() {\n  return <ns.Panel />;\n}\n";
    let declaration = span_of(source, "Panel", 0);
    let whole_name = span_of(source, "ns.Panel", 0);
    let property = span_of(source, "Panel", 1);
    let panel = span_of(source, "function Panel() {\n  return <p />;\n}", 0);

    assert_eq!(
        call_sites(
            source,
            &[
                (declaration, "panel"),
                (whole_name, "panel"),
                (property, "panel"),
            ]
        ),
        vec![(panel, vec![(whole_name.start, whole_name.end)])]
    );
    assert_eq!(
        accounted_references(
            source,
            &[
                (declaration, "panel"),
                (whole_name, "panel"),
                (property, "panel"),
            ],
            panel
        ),
        vec![(whole_name.start, whole_name.end)],
        "the property reference the escape test walks is not accounted for"
    );
}

#[test]
fn a_dotted_tag_without_an_exact_whole_name_entity_is_not_a_call_site() {
    // The counterfactual to the test above: no entity at the whole name,
    // as for a namespace object TypeScript could not resolve. The tag
    // names no proven declaration, so the graph keeps failing closed
    // instead of inventing a caller.
    let source =
        "function Panel() {\n  return <p />;\n}\nfunction App() {\n  return <ns.Panel />;\n}\n";
    let declaration = span_of(source, "Panel", 0);
    let property = span_of(source, "Panel", 1);

    assert!(call_sites(source, &[(declaration, "panel"), (property, "panel")]).is_empty());
}

#[test]
fn a_closing_tag_is_accounted_for_by_the_opening_tags_edge() {
    // `<Panel></Panel>` renders `Panel` once and writes its name twice.
    // TypeScript reports both occurrences as references to the same
    // symbol, so the escape test has to account for both — but the call
    // graph must still hold exactly one edge, because there is one
    // invocation.
    let source =
        "function Panel() {\n  return <p />;\n}\nfunction App() {\n  return <Panel></Panel>;\n}\n";
    let declaration = span_of(source, "Panel", 0);
    let opening = span_of(source, "Panel", 1);
    let closing = span_of(source, "Panel", 2);
    let panel = span_of(source, "function Panel() {\n  return <p />;\n}", 0);
    let entities = [
        (declaration, "panel"),
        (opening, "panel"),
        (closing, "panel"),
    ];

    assert_eq!(
        call_sites(source, &entities),
        vec![(panel, vec![(opening.start, opening.end)])],
        "one render is one edge"
    );
    assert_eq!(
        accounted_references(source, &entities, panel),
        vec![(opening.start, opening.end), (closing.start, closing.end)],
        "both occurrences of the tag name are accounted for"
    );
}

#[test]
fn a_closing_tag_cannot_account_for_a_reference_on_its_own() {
    // The invariant behind the test above: the closing name rides on the
    // edge the *opening* tag's resolution created. With no entity at the
    // opening name there is no edge, so the closing name accounts for
    // nothing — a closing tag can never be the thing that makes a
    // reference look like a call.
    let source =
        "function Panel() {\n  return <p />;\n}\nfunction App() {\n  return <Panel></Panel>;\n}\n";
    let declaration = span_of(source, "Panel", 0);
    let closing = span_of(source, "Panel", 2);
    let panel = span_of(source, "function Panel() {\n  return <p />;\n}", 0);
    let entities = [(declaration, "panel"), (closing, "panel")];

    assert!(call_sites(source, &entities).is_empty());
    assert!(accounted_references(source, &entities, panel).is_empty());
}

/// Both directions of the alias gate, so neither forcing can survive.
///
/// Forcing it true republishes the claim `@solidjs/router`'s `Navigate`
/// made: the destructured `href` *is* `props`, so a call of `href` reads as
/// a call of the props object. Forcing it false drops the identifier alias,
/// so `alias()` stops resolving to `handler` and every claim about the
/// aliased callable disappears.
#[test]
fn only_an_identifier_binding_inherits_its_initializer_identity() {
    let source = concat!(
        "const alias = handler;\n",
        "const { href } = props;\n",
        "alias();\n",
        "href();\n",
    );
    let entities = [
        (span_of(source, "alias", 0), "symbol:alias"),
        (span_of(source, "alias", 1), "symbol:alias"),
        (span_of(source, "handler", 0), "symbol:handler"),
        (span_of(source, "href", 0), "symbol:href"),
        (span_of(source, "href", 1), "symbol:href"),
        (span_of(source, "props", 0), "symbol:props"),
    ];
    let (aliased, destructured) = with_lookup(source, &entities, |lookup| {
        let file = &lookup.facts.files[0];
        let mut visited = HashSet::default();
        let aliased = lookup.direct_value_symbols(file, span_of(source, "alias", 1), &mut visited);
        let mut visited = HashSet::default();
        let destructured =
            lookup.direct_value_symbols(file, span_of(source, "href", 1), &mut visited);
        (aliased, destructured)
    });
    assert_eq!(
        aliased,
        vec![SymbolId::from("symbol:handler")],
        "an identifier binding is an alias of its initializer"
    );
    assert_eq!(
        destructured,
        vec![SymbolId::from("symbol:href")],
        "a destructured local is its own binding, never the object it came from"
    );
}

#[test]
fn nested_literal_paths_follow_only_single_use_local_bindings() {
    let source = "function App() { const inner = { read: inert }; const holder = { source: inner }; const alias = holder; use(alias); }";
    let argument = span_of(source, "alias", 1);
    with_lookup(source, &[], |lookup| {
        let file = lookup.file_by_path(PATH).unwrap();
        let resolved =
            lookup.nested_member_value(file, argument, &["source".into(), "read".into()], 16);
        assert_eq!(
            resolved.map(|(file, value)| file.source_text(value)),
            Some(Some("inert"))
        );
        assert!(
            lookup
                .nested_member_value(file, argument, &["source".into(), "read".into()], 2)
                .is_none()
        );
    });
}

/// The receiver symbol and access path `member_callee_receiver` answers
/// for `callee`.
///
/// This builds its own lookup because `with_lookup` deliberately carries
/// no AST indexes, and the member walk reads property spans out of one.
fn member_callee_path(
    source: &str,
    callee: Span,
    spans: &[(Span, &str)],
) -> Option<(String, Vec<String>)> {
    let facts = project(source);
    let ast_indexes = facts
        .files
        .iter()
        .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
        .collect::<HashMap<_, _>>();
    let entities = entity_symbols(spans);
    let symbol_names = HashMap::default();
    let dialect = solid_dialect::Solid2;
    let lookup = SemanticLookup::new(
        &facts,
        &ast_indexes,
        &entities,
        &symbol_names,
        &dialect,
        false,
    );
    lookup
        .member_callee_receiver(&facts.files[0], callee)
        .map(|(receiver, path)| (receiver.to_string(), path))
}

/// `props` and the body reference to it, the entity pair every chain
/// rooted at the first parameter needs.
fn props_entities(source: &str) -> Vec<(Span, &'static str)> {
    vec![
        (span_of(source, "props", 0), "symbol:props"),
        (span_of(source, "props", 1), "symbol:props"),
    ]
}

/// The whole chain, not its last segment.
///
/// Reverting the walk to the pre-fix rooting — resolve the immediate
/// object, report one property — answers `["includes"]` here, a property
/// of `props.modifiers` attributed to `props`. That claim can never be
/// witnessed, because a consumer matches the stated path as a *prefix* of
/// the observed access.
#[test]
fn a_member_chain_reads_the_whole_path_from_its_root() {
    let source = "function f(props, m) {\n  return props.modifiers.includes(m);\n}\n";
    let callee = span_of(source, "props.modifiers.includes", 0);

    assert_eq!(
        member_callee_path(source, callee, &props_entities(source)),
        Some((
            "symbol:props".to_owned(),
            vec!["modifiers".to_owned(), "includes".to_owned()]
        ))
    );
}

#[test]
fn a_one_segment_chain_reads_that_one_property() {
    let source = "function f(props) {\n  return props.of();\n}\n";
    let callee = span_of(source, "props.of", 0);

    assert_eq!(
        member_callee_path(source, callee, &props_entities(source)),
        Some(("symbol:props".to_owned(), vec!["of".to_owned()]))
    );
}

/// A segment that cannot be named exactly cuts the path back to the
/// longest exact prefix *from the root*, and never invents the segments
/// outside it.
#[test]
fn an_unnameable_segment_truncates_to_the_prefix_from_the_root() {
    let inside = "function f(props, key) {\n  return props.of[key].values();\n}\n";
    let at_root = "function f(props, key) {\n  return props[key].values();\n}\n";

    assert_eq!(
        member_callee_path(
            inside,
            span_of(inside, "props.of[key].values", 0),
            &props_entities(inside)
        ),
        Some(("symbol:props".to_owned(), vec!["of".to_owned()])),
        "the segment outside the computed one is not a property of `props`"
    );
    assert_eq!(
        member_callee_path(
            at_root,
            span_of(at_root, "props[key].values", 0),
            &props_entities(at_root)
        ),
        Some(("symbol:props".to_owned(), Vec::new())),
        "nothing can be named, so the row claims only a read through `props`"
    );
}

/// A chain deeper than the path limit keeps its row, cut to the limit.
///
/// Dropping it would publish the negative claim "this export performs no
/// parameter read" into a `Complete` reads set. The pair pins both sides
/// of the boundary so neither a smaller limit nor a reinstated refusal
/// survives.
#[test]
fn an_over_long_chain_is_truncated_and_never_dropped() {
    let chain = |segments: usize| {
        let names = (0..segments)
            .map(|index| format!("s{index}"))
            .collect::<Vec<_>>();
        let source = format!(
            "function f(props) {{\n  return props.{}();\n}}\n",
            names.join(".")
        );
        let callee_text = format!("props.{}", names.join("."));
        let entities = props_entities(&source);
        let callee = span_of(&source, &callee_text, 0);
        let answer = member_callee_path(&source, callee, &entities);
        (names, answer)
    };

    let (exact_names, exact) = chain(MEMBER_CALLEE_PATH_LIMIT);
    assert_eq!(
        exact,
        Some(("symbol:props".to_owned(), exact_names)),
        "a chain at the limit is reported whole"
    );

    let (over_names, over) = chain(MEMBER_CALLEE_PATH_LIMIT + 1);
    assert_eq!(
        over,
        Some((
            "symbol:props".to_owned(),
            over_names[..MEMBER_CALLEE_PATH_LIMIT].to_vec()
        )),
        "one segment past the limit keeps the row at the prefix from the root"
    );
}

/// A chain whose root is a compound expression publishes nothing.
///
/// The compiler's entity table answers a symbol at a conditional,
/// sequence, or logical span — the *leftmost operand's*. Trusting it makes
/// `k`, a boolean condition, read `["c", "slice"]`: every segment is a
/// property of the chain's result, and none is a property of `k`. Each
/// case registers that entity, so removing the identifier gate makes this
/// test report the fabricated row rather than `None`.
#[test]
fn a_compound_chain_root_is_refused_rather_than_attributed() {
    let conditional =
        "function f(props, k, n) {\n  return (k ? props.a : props.b).c.slice(n);\n}\n";
    let sequence = "function f(props, k, n) {\n  return (k, props.a).c.slice(n);\n}\n";
    let logical =
        "function f(props, fallback, n) {\n  return (props.a || fallback).c.slice(n);\n}\n";

    assert_eq!(
        member_callee_path(
            conditional,
            span_of(conditional, "(k ? props.a : props.b).c.slice", 0),
            &[
                (span_of(conditional, "k ? props.a : props.b", 0), "symbol:k"),
                (span_of(conditional, "k", 1), "symbol:k"),
            ]
        ),
        None,
        "a ternary's condition does not own the ternary result's properties"
    );
    assert_eq!(
        member_callee_path(
            sequence,
            span_of(sequence, "(k, props.a).c.slice", 0),
            &[
                (span_of(sequence, "k, props.a", 0), "symbol:k"),
                (span_of(sequence, "k", 1), "symbol:k"),
            ]
        ),
        None,
        "a discarded sequence operand does not own the result's properties"
    );
    assert_eq!(
        member_callee_path(
            logical,
            span_of(logical, "(props.a || fallback).c.slice", 0),
            &[
                (span_of(logical, "props.a || fallback", 0), "symbol:props"),
                (span_of(logical, "props", 1), "symbol:props"),
            ]
        ),
        None,
        "either branch of a logical may supply the value the chain walks"
    );
}

/// `props().slice(n)` names a property of the *call's result*.
///
/// The call is not the parameter, so the row that would be published here
/// says `props` has a `slice` property when nothing establishes that.
#[test]
fn a_call_at_the_chain_root_is_refused() {
    let source = "function f(props, n) {\n  return props().slice(n);\n}\n";

    assert_eq!(
        member_callee_path(
            source,
            span_of(source, "props().slice", 0),
            &[
                (span_of(source, "props()", 0), "symbol:props"),
                (span_of(source, "props", 1), "symbol:props"),
            ]
        ),
        None
    );
}

/// A callee that is not a member at all, and one whose own property is
/// computed, are both refused — the two gates that keep an ordinary call
/// from reading as a member access.
#[test]
fn a_non_member_or_computed_callee_is_refused() {
    let plain = "function f(notify, cb) {\n  return notify(cb);\n}\n";
    let computed = "function f(props, key) {\n  return props.of[key]();\n}\n";

    assert_eq!(
        member_callee_path(
            plain,
            span_of(plain, "notify", 1),
            &[
                (span_of(plain, "notify", 0), "symbol:notify"),
                (span_of(plain, "notify", 1), "symbol:notify"),
            ]
        ),
        None,
        "a call of a value is not a call through a property of one"
    );
    assert_eq!(
        member_callee_path(
            computed,
            span_of(computed, "props.of[key]", 0),
            &props_entities(computed)
        ),
        None,
        "`handlers[i]()` must never be read as a property named `i`"
    );
}

#[test]
fn a_renderer_declaration_must_sit_in_a_dialect_package() {
    let packages = ["solid-js", "@solidjs/signals", "@solidjs/web"];
    for inside in [
        "/p/node_modules/solid-js/types/client/component.d.ts",
        "/p/node_modules/@solidjs/web/types/client.d.ts",
        r"C:\p\node_modules\@solidjs\web\types\client.d.ts",
    ] {
        assert!(declared_in_package(inside, &packages), "{inside}");
    }
    for outside in [
        "/p/my-solid-js-tools/component.d.ts",
        "/p/node_modules/@solidjs/router/dist/index.d.ts",
        "/p/node_modules/web/solid.d.ts",
        "/p/node_modules/not-@solidjs/web/index.d.ts",
    ] {
        assert!(!declared_in_package(outside, &packages), "{outside}");
    }
}

/// Whether a later attribute or spread of the first element replaces the key
/// defined at `defined` (the text of the attribute or spread), under `dialect`.
fn event_key_replaced(
    dialect: &solid_dialect::Solid2,
    source: &str,
    defined: &str,
    key: &str,
) -> bool {
    let facts = project(source);
    let entities = entity_symbols(&[]);
    let ast_indexes = HashMap::default();
    let symbol_names = HashMap::default();
    let lookup = SemanticLookup::new(
        &facts,
        &ast_indexes,
        &entities,
        &symbol_names,
        dialect,
        false,
    );
    let file = &facts.files[0];
    let element = file.ast.jsx_elements.first().expect("an element");
    crate::object_members::event_key_may_be_replaced(
        &lookup,
        file,
        element,
        span_of(source, defined, 0),
        key,
    )
}

#[test]
fn a_later_attribute_or_spread_may_replace_an_event_key() {
    let rc17 = solid_dialect::Solid2::RC17;
    let replaced =
        |source: &str, defined: &str| event_key_replaced(&rc17, source, defined, "onClick");
    // Nothing follows, or only things that cannot hold the key.
    assert!(!replaced("<button {...h}>x</button>", "{...h}"));
    assert!(!replaced(
        "<button {...h} title=\"t\" ref={r}>x</button>",
        "{...h}"
    ));
    assert!(!replaced(
        "<button {...h} on:click={b}>x</button>",
        "{...h}"
    ));
    assert!(!replaced(
        "<button {...h} {...{ title: 't' }}>x</button>",
        "{...h}"
    ));
    assert!(!replaced(
        "<button {...h} {...{ onScroll: () => {} }}>x</button>",
        "{...h}"
    ));
    // A later `onClick` attribute, even a valueless or `undefined` one.
    assert!(replaced("<button {...h} onClick={b}>x</button>", "{...h}"));
    assert!(replaced(
        "<button {...h} onClick={undefined}>x</button>",
        "{...h}"
    ));
    // An earlier attribute is the earlier source.
    assert!(!replaced("<button onClick={b} {...h}>x</button>", "{...h}"));
    // A later exact holder with the key, and one the checker cannot resolve.
    assert!(replaced(
        "<button {...h} {...{ onClick: () => {} }}>x</button>",
        "{...h}"
    ));
    assert!(replaced("<button {...h} {...rest}>x</button>", "{...h}"));
    assert!(replaced(
        "<button onClick={cb} {...rest}>x</button>",
        "onClick={cb}"
    ));
    // rc.17 sets `onclick` as an attribute; rc.13 installs a listener of the
    // same type, which replaces a delegated one.
    let lower = "<button {...h} onclick={b}>x</button>";
    assert!(!replaced(lower, "{...h}"));
    assert!(event_key_replaced(
        &solid_dialect::Solid2::RC13,
        lower,
        "{...h}",
        "onClick"
    ));
    // An unaudited release states no grammar: a later `on…` or spread may hold it.
    assert!(event_key_replaced(
        &solid_dialect::Solid2::CONSERVATIVE,
        "<button {...h} title=\"t\">x</button>",
        "{...h}",
        "onClick"
    ));
}
