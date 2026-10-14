use super::*;
use solid_facts::{
    compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap, ExecutionRegion, RegionReason},
    core::{Generation, SourceHash, SourcePath},
};

fn file_with_regions(path: &str, source: &str, tracked: Vec<ExecutionRegion>) -> FileFacts {
    let ast = solid_facts::ast::extract(path, source).unwrap();
    FileFacts {
        generation: Generation::new(1).unwrap(),
        path: SourcePath::new(path).unwrap(),
        source_hash: SourceHash::of(source),
        source: Arc::from(source),
        ast: Arc::new(ast),
        compiler: Arc::new(ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of(""),
            semantic_model: Default::default(),
            tracked_regions: tracked,
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        }),
    }
}

fn file(path: &str, source: &str) -> FileFacts {
    file_with_regions(path, source, vec![])
}

/// The declaration span of the first binding named `name`.
fn declaration(file: &FileFacts, name: &str) -> Span {
    file.ast
        .identifiers
        .iter()
        .find(|identifier| {
            identifier.role == IdentifierRole::Binding
                && &file.source[identifier.span.start as usize..identifier.span.end as usize]
                    == name
        })
        .unwrap_or_else(|| panic!("no binding named {name}"))
        .span
}

fn key(key: &PropertyKey) -> String {
    match key {
        PropertyKey::Named(name) => name.to_string(),
        PropertyKey::Summarized => "*".into(),
    }
}

/// A compact, exact description of one disposition.
fn describe(disposition: &UseDisposition) -> String {
    match disposition {
        UseDisposition::Transfer { edge, to } => {
            let node = match &to.kind {
                FlowNodeKind::Seed { .. } => "seed".to_string(),
                FlowNodeKind::Binding => "binding".into(),
                FlowNodeKind::Parameter { index } => format!("parameter#{index}"),
                FlowNodeKind::Return => "return".into(),
                FlowNodeKind::ActualArgument { index } => format!("argument#{index}"),
                FlowNodeKind::Allocation { allocation } => json(allocation),
                FlowNodeKind::Property { key: property } => format!("property({})", key(property)),
                FlowNodeKind::Value => "value".into(),
                FlowNodeKind::Export { name } => format!("export({name})"),
            };
            format!("transfer:{}->{node}", json(edge))
        }
        UseDisposition::Read { event, context, .. } => {
            let event = match event {
                ReadEventKind::PropertyGet { key: property } => {
                    format!("property-get({})", key(property))
                }
                other => json(other),
            };
            assert_eq!(*context, InvocationContext::Unbound);
            format!("read:{event}")
        }
        UseDisposition::Operation { operation, .. } => match operation {
            OperationKind::FunctionMemberGet { key: property } => {
                format!("operation:function-member-get({})", key(property))
            }
            other => format!("operation:{}", json(other)),
        },
        UseDisposition::Excluded { reason } => format!("excluded:{}", json(reason)),
        UseDisposition::Unknown { reason } => format!("unknown:{}", json(reason)),
    }
}

/// A serialized value's bare tag.
fn json<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value).unwrap() {
        serde_json::Value::String(text) => text,
        serde_json::Value::Object(map) => map
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        other => other.to_string(),
    }
}

/// Every binder reference of `name`, classified, in source order.
fn census_of(path: &str, source: &str, name: &str, seed: SeedKind) -> Vec<String> {
    let file = file(path, source);
    census_in(&file, name, seed)
}

fn census_in(file: &FileFacts, name: &str, seed: SeedKind) -> Vec<String> {
    let index = FileUseIndex::new(file);
    index
        .binder_references(declaration(file, name))
        .into_iter()
        .map(|reference| describe(&index.classify(reference, seed)))
        .collect()
}

fn accessor(body: &str) -> Vec<String> {
    census_of(
        "test.ts",
        &format!(
            "declare function helper(...xs: unknown[]): unknown;\nconst value = () => 1;\n{body}"
        ),
        "value",
        SeedKind::Accessor,
    )
}

#[test]
fn calls_and_constructs_are_read_events() {
    assert_eq!(accessor("value();"), ["read:call"]);
    assert_eq!(accessor("new (value as any)();"), ["read:construct"]);
    assert_eq!(accessor("(value as any)!();"), ["read:call"]);
    assert_eq!(accessor("value`x`;"), ["read:tagged-template-call"]);
}

#[test]
fn presence_never_attests_seed_completeness() {
    let address = SourceAddress::new(&Arc::from("test.ts"), Span::new(0, 1));
    for present in [false, true] {
        let empty = coverage(&address, false, present, &[]);
        assert_eq!(
            empty,
            Coverage::Open {
                first: Some(address.clone()),
                reason: UnknownReason::ReferenceCensusIncomplete,
            }
        );
        for disposition in [
            UseDisposition::Excluded {
                reason: ExclusionReason::TypeOnly,
            },
            UseDisposition::Transfer {
                edge: FlowEdgeKind::Import,
                to: FlowNode {
                    address: address.clone(),
                    kind: FlowNodeKind::Binding,
                },
            },
        ] {
            let record = UseRecord {
                reference: address.clone(),
                frame: None,
                captured: None,
                region: UseRegion::Unattested,
                disposition,
            };
            assert_eq!(coverage(&address, false, present, &[record]), empty);
        }
    }
}

fn project_record(
    file: &FileFacts,
    fact: Option<&typefacts::SymbolFact>,
    aliases: &[SymbolId],
) -> SeedRecord {
    let id = SymbolId::from("seed");
    let files = [file.clone()];
    let mut project = ProjectUses::new(&files);
    let mut symbols = HashMap::new();
    if let Some(fact) = fact {
        symbols.insert("seed", Some(TypeScriptSymbol::Synthetic(fact)));
    }
    census(
        &crate::indexes::EntitySymbols {
            by_path: HashMap::new(),
        },
        aliases,
        &symbols,
        &mut project,
        &Canon::new("/project/tsconfig.json"),
        &Seed {
            symbol: &id,
            kind: Some(SeedKind::Accessor),
        },
    )
}

fn symbol_fact(file: &FileFacts, name: &str) -> typefacts::SymbolFact {
    let span = declaration(file, name);
    typefacts::SymbolFact {
        id: Arc::from("seed"),
        alias_target: Arc::from(""),
        references: Arc::from([]),
        declarations: Arc::from([typefacts::Declaration {
            name: Arc::from(name),
            kind: Arc::from("variable"),
            location: typefacts::Location {
                path: file.path.shared(),
                start_byte: u64::from(span.start),
                end_byte: u64::from(span.end),
            },
        }]),
    }
}

#[test]
fn missing_alias_reference_and_identity_evidence_stays_open() {
    let file = file(
        "/project/control.ts",
        "const value = () => 1; type T = typeof value;",
    );
    let fact = symbol_fact(&file, "value");
    let missing = project_record(&file, None, &[]);
    assert!(missing.seed.is_none());
    assert!(!missing.identity_gaps.is_empty());
    let empty_references = project_record(&file, Some(&fact), &[]);
    assert!(matches!(
        empty_references.coverage,
        Coverage::Open {
            reason: UnknownReason::ReferenceCensusIncomplete,
            ..
        }
    ));
    let missing_alias = project_record(&file, Some(&fact), &[SymbolId::from("missing")]);
    assert!(matches!(
        missing_alias.coverage,
        Coverage::Open {
            reason: UnknownReason::IdentityUnattested,
            ..
        }
    ));
    let mut outside = fact.clone();
    Arc::make_mut(&mut outside.declarations)[0].location.path = Arc::from("/outside/absent.ts");
    let row = project_record(&file, Some(&outside), &[]);
    assert!(row.seed.is_none());
    assert_eq!(
        row.identity_gaps
            .iter()
            .find_map(|gap| gap.location.as_ref())
            .unwrap()
            .path
            .as_ref(),
        "/outside/absent.ts"
    );
    let mut invalid = fact.clone();
    Arc::make_mut(&mut invalid.declarations)[0]
        .location
        .end_byte = u64::MAX;
    let row = project_record(&file, Some(&invalid), &[]);
    assert!(row.seed.is_none());
    assert_eq!(
        row.identity_gaps
            .iter()
            .find_map(|gap| gap.location.as_ref())
            .unwrap()
            .end_byte,
        u64::MAX
    );
    let mut ambiguous = fact.clone();
    ambiguous.declarations =
        vec![fact.declarations[0].clone(), fact.declarations[0].clone()].into();
    assert!(project_record(&file, Some(&ambiguous), &[]).seed.is_none());
}

#[test]
fn held_class_and_renderer_controls_are_unbound_open_observations() {
    for body in [
        "class C { field = value(); static field = value(); static { value(); } constructor(x = value()) {} }",
        "const view = <p class={[\"x\", { k: value() }]} />;",
        "const view = <p class={[\"x\", { k: (value(), false) }]} />;",
        "const view = <p class={{ k: (value(), false) }} />;",
        "const view = <input {...{title: 'x'}}><p class={{k: value()}} /></input>;",
        "const view = <div><input {...{title: 'x'}}><p class={[\"x\", {k: value()}]} /></input></div>;",
        "const view = <input {...{title: 'x'}}>{value()}</input>;",
    ] {
        let source = format!("function outer() {{ const value = () => true; {body} }}");
        let file = file("/project/control.tsx", &source);
        let fact = symbol_fact(&file, "value");
        let row = project_record(&file, Some(&fact), &[]);
        assert!(matches!(row.coverage, Coverage::Open { .. }), "{body}");
        assert!(!row.uses.is_empty(), "{body}");
        for usage in &row.uses {
            assert_eq!(usage.captured, None);
            if body.contains("view") {
                assert!(
                    matches!(
                        usage.disposition,
                        UseDisposition::Unknown {
                            reason: UnknownReason::RuntimeExecution
                        }
                    ),
                    "{body}"
                );
            } else if let UseDisposition::Read { context, .. } = usage.disposition {
                assert_eq!(context, InvocationContext::Unbound);
            }
        }
    }
}

#[test]
fn optional_and_wrapped_eval_cannot_establish_binding_authority() {
    for callee in ["eval?.", "(eval as any)?.", "((eval as any)!)"] {
        let source =
            format!("function f() {{ const value = () => 1; return {callee}(\"value()\"); }}");
        assert_eq!(
            implicit_uses(&source, "value"),
            [(
                format!("{callee}(\"value()\")"),
                "eval-binding-unattested".into()
            )]
        );
    }
    assert!(implicit_uses("function f(eval: Function) { const value = () => 1; return (eval as any)(\"value()\"); }", "value").is_empty());
}

#[test]
fn wrappers_are_bounded_and_never_close_a_seed() {
    let wrapped = format!("{}value{}", "(".repeat(70), " as any)".repeat(70));
    assert_eq!(
        accessor(&format!("{wrapped}();")),
        ["unknown:wrapper-depth"]
    );
    let file = file("test.ts", &format!("const value = () => 1; {wrapped}();"));
    let fact = symbol_fact(&file, "value");
    assert!(matches!(
        project_record(&file, Some(&fact), &[]).coverage,
        Coverage::Open {
            reason: UnknownReason::WrapperDepth,
            ..
        }
    ));
}

#[test]
fn an_alias_declaration_never_substitutes_for_the_missing_seed() {
    let file = file("/project/control.ts", "const alias = () => 1; alias();");
    let fact = symbol_fact(&file, "alias");
    let files = [file];
    let mut project = ProjectUses::new(&files);
    let symbols = HashMap::from([("alias", Some(TypeScriptSymbol::Synthetic(&fact)))]);
    let seed = SymbolId::from("missing-seed");
    let row = census(
        &crate::indexes::EntitySymbols {
            by_path: HashMap::new(),
        },
        &[SymbolId::from("alias")],
        &symbols,
        &mut project,
        &Canon::new("/project/tsconfig.json"),
        &Seed {
            symbol: &seed,
            kind: Some(SeedKind::Accessor),
        },
    );
    assert!(row.seed.is_none());
    assert_eq!(row.aliases.len(), 1);
    assert!(matches!(
        row.coverage,
        Coverage::Open {
            first: None,
            reason: UnknownReason::IdentityUnattested
        }
    ));
}

#[test]
fn equal_seed_keys_have_a_canonical_content_tie_break_without_dedup() {
    let mut prior = None;
    for root in ["/project", "/renamed-project"] {
        let canon = Canon::new(&format!("{root}/tsconfig.json"));
        let address = SourceAddress::new(&Arc::from(format!("{root}/seed.ts")), Span::new(0, 1));
        let first = SeedRecord {
            seed: Some(FlowNode {
                address: address.clone(),
                kind: FlowNodeKind::Seed {
                    seed: SeedKind::Accessor,
                },
            }),
            identity_gaps: vec![],
            aliases: vec![SourceAddress::new(
                &Arc::from("/outside/a.ts"),
                Span::new(0, 1),
            )],
            published: false,
            uses: vec![],
            coverage: coverage(&address, false, true, &[]),
        };
        let mut second = first.clone();
        second.aliases = vec![SourceAddress::new(
            &Arc::from(format!("{root}/z.ts")),
            Span::new(0, 1),
        )];
        let mut forward = vec![first.clone(), second.clone()];
        let mut reverse = vec![second, first];
        forward.sort_by(|left, right| canon.compare_records(left, right));
        reverse.sort_by(|left, right| canon.compare_records(left, right));
        assert_eq!(forward, reverse);
        assert_eq!(forward.len(), 2);
        let keys = forward
            .iter()
            .map(|record| canon.record_key(record))
            .collect::<Vec<_>>();
        if let Some(prior) = prior {
            assert_eq!(keys, prior);
        }
        prior = Some(keys);
    }
}

#[test]
fn arguments_are_transfers_whatever_the_callee() {
    assert_eq!(
        accessor("helper(value);"),
        ["transfer:argument->argument#0"]
    );
    // Review round 2: Proxy, Array.from and forEach are callee questions.
    assert_eq!(
        accessor("const box = new Proxy(value, {});"),
        ["transfer:argument->argument#0"]
    );
    assert_eq!(
        accessor("Array.from([0], value);"),
        ["transfer:argument->argument#1"]
    );
    assert_eq!(
        accessor("[0].forEach(value);"),
        ["transfer:argument->argument#0"]
    );
    // Round 1: access.call(value) and withAccess(value, ...) hand it over.
    assert_eq!(
        accessor("declare const access: { call(v: unknown): void }; access.call(value);"),
        ["transfer:argument->argument#0"]
    );
}

#[test]
fn spreading_the_value_itself_iterates_it() {
    assert_eq!(accessor("helper(...(value as any));"), ["read:iteration"]);
    assert_eq!(
        accessor("for (const x of value as any) {}"),
        ["read:iteration"]
    );
    assert_eq!(
        accessor("const [first] = value as any;"),
        ["read:iteration"]
    );
    assert_eq!(accessor("const copy = { ...value };"), ["read:enumeration"]);
}

#[test]
fn round_one_reflective_members_are_operations_without_rows() {
    assert_eq!(
        accessor("value.call(null); value.apply(null, []); value.bind(null); value.length;"),
        [
            "operation:function-member-get(call)",
            "operation:function-member-get(apply)",
            "operation:function-member-get(bind)",
            "operation:function-member-get(length)",
        ]
    );
    assert_eq!(
        accessor("const { name } = value;"),
        ["operation:function-destructuring"]
    );
}

#[test]
fn bindings_returns_and_containers_are_transfers() {
    assert_eq!(
        accessor("const alias = value;"),
        ["transfer:alias->binding"]
    );
    assert_eq!(
        accessor("let alias; alias = value;"),
        ["transfer:alias->binding"]
    );
    assert_eq!(
        accessor("function make() { return value; }"),
        ["transfer:return->return"]
    );
    assert_eq!(
        accessor("const make = () => value;"),
        ["transfer:return->return"]
    );
    assert_eq!(
        accessor("const box = { read: value, value };"),
        [
            "transfer:property-store->property(read)",
            "transfer:property-store->property(value)",
        ]
    );
    assert_eq!(
        accessor("declare const key: string; const box = { [key]: value };"),
        ["transfer:property-store->property(*)"]
    );
    assert_eq!(
        accessor("const box: { read?: unknown } = {}; box.read = value;"),
        ["transfer:property-store->property(read)"]
    );
    assert_eq!(
        accessor("function f(v = value) {} const g = (w = value) => w;"),
        [
            "transfer:default-initializer->parameter#0",
            "transfer:default-initializer->parameter#0",
        ]
    );
    assert_eq!(
        accessor("declare const c: boolean; const pick = c ? value : null;"),
        ["transfer:conditional-branch->value"]
    );
    assert_eq!(
        accessor("declare const other: () => number; const pick = value || other;"),
        ["transfer:logical-operand->value"]
    );
    assert_eq!(
        accessor("export { value };"),
        ["transfer:export->export(value)"]
    );
}

#[test]
fn tests_and_coercions_are_operations_without_rows() {
    assert_eq!(accessor("if (value) {}"), ["operation:test"]);
    assert_eq!(accessor("const n = value ? 1 : 2;"), ["operation:test"]);
    assert_eq!(accessor("const n = !value;"), ["operation:coercion"]);
    assert_eq!(accessor("const s = `${value}`;"), ["operation:coercion"]);
    assert_eq!(accessor("const s = 'a' + value;"), ["operation:coercion"]);
    assert_eq!(
        accessor("const n = value == null;"),
        ["operation:nullish-test"]
    );
}

#[test]
fn unnormalized_syntax_is_an_explicit_unknown() {
    // None of these is silence. Each names the missing fact.
    for body in [
        "value;",
        "const t = typeof value;",
        "declare const other: unknown; const same = value === other;",
        "void value;",
    ] {
        assert_eq!(accessor(body), ["unknown:unrecorded-parent"], "{body}");
    }
}

#[test]
fn array_elements_are_container_stores_until_a_spread() {
    // Review round 2: `helper(...[value])` stores the value at index 0 of the
    // array the spread then iterates.
    assert_eq!(
        accessor("helper(...[value]);"),
        ["transfer:container-store->property(0)"]
    );
    assert_eq!(
        accessor("declare const xs: unknown[]; const values = [1, , value, ...xs, value];"),
        [
            "transfer:container-store->property(2)",
            "transfer:container-store->property(*)",
        ]
    );
    assert_eq!(
        accessor("const copy = [...(value as any)];"),
        ["read:iteration"]
    );
}

#[test]
fn tagged_template_substitutions_are_tag_arguments() {
    // Review round 2: `tag`${value}`` invokes `tag` with the value as its
    // second argument.
    assert_eq!(
        accessor("declare function tag(s: TemplateStringsArray, v: unknown): void; tag`${value}`;"),
        ["transfer:tagged-template-substitution->argument#1"]
    );
    assert_eq!(
        accessor(
            "declare function tag(s: TemplateStringsArray, ...v: unknown[]): void; tag`${1}${value}`;"
        ),
        ["transfer:tagged-template-substitution->argument#2"]
    );
}

#[test]
fn awaits_and_jsx_children_are_recorded_events() {
    assert_eq!(
        accessor("async function f() { await value; }"),
        ["read:then-lookup"]
    );
    assert_eq!(
        census_of(
            "test.tsx",
            "const value = () => 'a';\nconst view = <p>{value}{value()}</p>;",
            "value",
            SeedKind::Accessor
        ),
        ["unknown:runtime-execution", "unknown:runtime-execution"]
    );
}

#[test]
fn pattern_defaults_and_keys_transfer_into_their_targets() {
    assert_eq!(
        accessor("declare const o: { read?: unknown }; const { read = value } = o;"),
        ["transfer:default-initializer->binding"]
    );
    assert_eq!(
        accessor("declare const xs: unknown[]; const [first = value] = xs;"),
        ["transfer:default-initializer->binding"]
    );
    assert_eq!(
        accessor("declare const o: { read?: unknown }; let read; ({ read = value } = o);"),
        ["transfer:default-initializer->binding"]
    );
}

#[test]
fn object_rest_patterns_enumerate_their_source() {
    let props = |pattern: &str| {
        census_of(
            "test.tsx",
            &format!(
                "function Child(props: {{ a: number; b: number }}) {{ const {pattern} = props; }}"
            ),
            "props",
            SeedKind::PropRoot,
        )
    };
    assert_eq!(props("{ a }"), ["read:destructuring-get"]);
    assert_eq!(props("{ a, ...rest }"), ["read:enumeration"]);
}

#[test]
fn default_initializer_uses_name_their_region() {
    // Review round 2: the call runs while the parameters bind.
    let source = "function helper(v: () => string, { text = v() } = {}, other = v()) { return v(); }\n\
                  function late(v: () => string, make = () => v()) { return make; }";
    let file = file("test.ts", source);
    let index = FileUseIndex::new(&file);
    let regions = |name: &str| {
        index
            .binder_references(declaration(&file, name))
            .into_iter()
            .map(|reference| match index.region(reference) {
                UseRegion::Unattested => "evaluation".to_string(),
                UseRegion::DefaultInitializer {
                    initializer,
                    parameters,
                } => format!(
                    "default({}){}",
                    &source[initializer.start as usize..initializer.end as usize],
                    if parameters { " in parameters" } else { "" }
                ),
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        regions("v"),
        [
            "default(v()) in parameters",
            "default(v()) in parameters",
            "evaluation",
        ]
    );
    // A closure written in a default runs later, in its own frame.
    let late = source.rfind("v: () => string").unwrap() as u32;
    let late_declaration = Span::new(late, late + 1);
    let references = index.binder_references(late_declaration);
    assert_eq!(references.len(), 1);
    assert_eq!(index.region(references[0]), UseRegion::Unattested);
}

#[test]
fn computed_members_never_guess_a_key() {
    assert_eq!(
        accessor("declare const k: 'length'; value[k];"),
        ["unknown:computed-key"]
    );
    assert_eq!(
        accessor("value['length'];"),
        ["operation:function-member-get(length)"]
    );
    // As a key, the value is converted, not projected.
    assert_eq!(
        accessor("declare const box: Record<string, number>; box[value as any];"),
        ["operation:coercion"]
    );
    assert_eq!(
        accessor(
            "declare const k: string; const box: Record<string, unknown> = {}; box[k] = value;"
        ),
        ["transfer:property-store->property(*)"]
    );
}

#[test]
fn writes_open_the_binding() {
    assert_eq!(
        census_of(
            "test.ts",
            "let value = () => 1; value = () => 2; value();",
            "value",
            SeedKind::Accessor
        ),
        ["unknown:binding-written", "read:call"]
    );
    assert_eq!(
        census_of(
            "test.ts",
            "let value = () => 1; for (value of [() => 2]) {}",
            "value",
            SeedKind::Accessor
        ),
        ["unknown:binding-written"]
    );
}

#[test]
fn erased_references_are_excluded() {
    assert_eq!(
        accessor("type Value = typeof value;"),
        ["excluded:type-only"]
    );
}

#[test]
fn stores_and_props_read_through_gets() {
    let store = |body: &str| {
        census_of(
            "test.ts",
            &format!("declare const make: () => {{ x: number }};\nconst store = make();\n{body}"),
            "store",
            SeedKind::Store,
        )
    };
    assert_eq!(store("store.x;"), ["read:property-get(x)"]);
    assert_eq!(store("const { x } = store;"), ["read:destructuring-get"]);
    assert_eq!(store("store.x = 1;"), ["unknown:write-through-seed"]);
    assert_eq!(
        store("delete (store as any).x;"),
        ["unknown:write-through-seed"]
    );
    assert_eq!(
        census_of(
            "test.tsx",
            "function Child(props: { read: () => string }) { return props.read(); }",
            "props",
            SeedKind::PropRoot,
        ),
        ["read:property-get(read)"]
    );
}

#[test]
fn jsx_props_and_spreads_are_transfers() {
    // Review round 2: the JSX prop that Child calls in its own body.
    let source = "declare function Child(p: { read: () => string }): any;\n\
                  const value = () => 'a';\n\
                  const a = <Child read={value} />;\n\
                  const b = <Child {...{ read: value }} />;\n\
                  const c = <Child {...(value as any)} />;";
    assert_eq!(
        census_of("test.tsx", source, "value", SeedKind::Accessor),
        [
            "unknown:runtime-execution",
            "unknown:runtime-execution",
            "unknown:runtime-execution",
        ]
    );
}

#[test]
fn lexical_function_containment_is_not_execution_authority() {
    let source = "const value = () => 1;\n\
                  function outer() { const inner = () => value(); return inner; }\n\
                  value();";
    let file = file("test.ts", source);
    let index = FileUseIndex::new(&file);
    let declared = declaration(&file, "value");
    let declared_frame = index.frame(declared);
    let captured = index
        .binder_references(declared)
        .into_iter()
        .map(|reference| index.frame(reference) != declared_frame)
        .collect::<Vec<_>>();
    assert_eq!(captured, [true, false]);
    // The nested helper's frame is the arrow, not `outer`.
    let first = index.binder_references(declared)[0];
    let frame = index.frame(first).unwrap();
    assert!(source[frame.start as usize..frame.end as usize].starts_with("() => value()"));
}

#[test]
fn default_initializer_reads_are_call_events() {
    // Review round 2: `{ text = v() } = {}` calls the parameter while binding.
    let source = "function helper(v: () => string, { text = v() } = {}) { return text; }";
    assert_eq!(
        census_of("test.ts", source, "v", SeedKind::Accessor),
        ["read:call"]
    );
    let source = "function helper(v: () => string, text = v()) { return text; }";
    assert_eq!(
        census_of("test.ts", source, "v", SeedKind::Accessor),
        ["read:call"]
    );
}

#[test]
fn compiler_regions_never_bind_invocation_contexts() {
    let source = "const value = () => 1;\nconst a = value();\nconst b = () => value();";
    let first = source.find("value()").unwrap() as u32;
    let second = source.rfind("() => value()").unwrap() as u32;
    let tracked = vec![
        ExecutionRegion {
            span: Span::new(first, first + 7),
            reason: RegionReason::JsxChild,
        },
        ExecutionRegion {
            span: Span::new(second, source.len() as u32),
            reason: RegionReason::JsxChild,
        },
    ];
    let file = file_with_regions("test.ts", source, tracked);
    assert_eq!(
        census_in(&file, "value", SeedKind::Accessor),
        ["unknown:runtime-execution", "unknown:runtime-execution"]
    );
}

#[test]
fn coverage_is_never_closed_over_a_used_value() {
    let address = |start| SourceAddress {
        path: Arc::from("test.ts"),
        span: Span::new(start, start + 1),
    };
    let record = |start, disposition| UseRecord {
        reference: address(start),
        frame: None,
        captured: None,
        region: UseRegion::Unattested,
        disposition,
    };
    assert_eq!(
        coverage(&address(0), false, true, &[]),
        Coverage::Open {
            first: Some(address(0)),
            reason: UnknownReason::ReferenceCensusIncomplete
        }
    );
    // No completeness attestation exists, regardless of table presence.
    assert_eq!(
        coverage(&address(0), false, false, &[]),
        Coverage::Open {
            first: Some(address(0)),
            reason: UnknownReason::ReferenceCensusIncomplete,
        }
    );
    assert_eq!(
        coverage(&address(0), true, true, &[]),
        Coverage::Open {
            first: Some(address(0)),
            reason: UnknownReason::Published,
        }
    );
    let uses = [
        record(
            5,
            UseDisposition::Excluded {
                reason: ExclusionReason::TypeOnly,
            },
        ),
        record(
            9,
            UseDisposition::Read {
                event: ReadEventKind::Call,
                at: address(9),
                context: InvocationContext::Unbound,
            },
        ),
        record(
            12,
            UseDisposition::Unknown {
                reason: UnknownReason::UnrecordedParent,
            },
        ),
    ];
    // Decision 7: the first open use in canonical order, not the most
    // severe; every unknown stays in the record.
    assert_eq!(
        coverage(&address(0), false, true, &uses),
        Coverage::Open {
            first: Some(address(9)),
            reason: UnknownReason::ReadRowUnbound,
        }
    );
    assert_eq!(
        coverage(&address(0), false, true, &uses[..1]),
        Coverage::Open {
            first: Some(address(0)),
            reason: UnknownReason::ReferenceCensusIncomplete
        }
    );
}

#[test]
fn the_census_is_deterministic() {
    let source = "declare function helper(...xs: unknown[]): unknown;\n\
                  const value = () => 1;\n\
                  helper(value); value(); const a = { value }; if (value) {} value.call(null);";
    let file = file("test.ts", source);
    let first = census_in(&file, "value", SeedKind::Accessor);
    for _ in 0..4 {
        assert_eq!(census_in(&file, "value", SeedKind::Accessor), first);
    }
    let address = SourceAddress::new(&Arc::from("test.ts"), declaration(&file, "value"));
    let record = SeedRecord {
        seed: Some(FlowNode {
            address: address.clone(),
            kind: FlowNodeKind::Seed {
                seed: SeedKind::Accessor,
            },
        }),
        identity_gaps: vec![],
        aliases: vec![],
        published: false,
        uses: vec![],
        coverage: coverage(&address, false, true, &[]),
    };
    let shadow = ValueFlowShadow {
        seeds: vec![record],
        edges: vec![],
        indexed_files: 1,
    };
    assert_eq!(shadow.audit(), shadow.audit());
    let row: serde_json::Value = serde_json::from_str(shadow.audit().trim()).unwrap();
    assert_eq!(row["coverage"]["coverage"], "open");
    assert_eq!(row["coverage"]["reason"], "reference-census-incomplete");
}

#[test]
fn published_declarations_stay_open() {
    let source = "export const [value] = [() => 1];\n\
                  export function component(props: unknown) { const local = () => 1; return props; }";
    let file = file("test.ts", source);
    let index = FileUseIndex::new(&file);
    assert!(index.published(declaration(&file, "value")));
    assert!(!index.published(declaration(&file, "props")));
    assert!(!index.published(declaration(&file, "local")));
}

// ---------------------------------------------------------------------------
// Review round 1 (c1-c2 review): every finding as a regression. The sources
// are the reviewer's reproducers, tsc-clean against solid-js 2.0.0-rc.13.
// ---------------------------------------------------------------------------

/// The implicit uses `dynamic_scope_uses` records for the binding `name`.
fn implicit_uses(source: &str, name: &str) -> Vec<(String, String)> {
    let file = file("test.ts", source);
    let index = FileUseIndex::new(&file);
    index
        .dynamic_scope_uses(declaration(&file, name))
        .into_iter()
        .map(|(span, reason)| {
            (
                file.source[span.start as usize..span.end as usize].to_string(),
                json(&reason),
            )
        })
        .collect()
}

#[test]
fn direct_eval_and_arguments_reach_the_seeds_in_scope() {
    // F1 (repro/f2/evalargs.tsx): both reads happen at runtime, and neither
    // spells a reference.
    let source = "export function viaEval() {\n\
                    const [value] = [() => 1];\n\
                    return eval(\"value()\");\n\
                  }\n\
                  export function Child(props: { a: () => number }) {\n\
                    return arguments[0].a();\n\
                  }\n\
                  export function Sliced(props: { a: () => number }) {\n\
                    return Array.prototype.slice.call(arguments);\n\
                  }\n\
                  export function Arrow(props: { a: () => number }) {\n\
                    const later = () => arguments[0];\n\
                    return later;\n\
                  }";
    assert_eq!(
        implicit_uses(source, "value"),
        [(
            "eval(\"value()\")".to_string(),
            "eval-binding-unattested".to_string()
        )]
    );
    assert_eq!(
        implicit_uses(source, "props"),
        [("arguments".to_string(), "arguments-object".to_string())]
    );
    // An arrow has no `arguments` of its own: the enclosing function's.
    let arrow = file("test.tsx", source);
    let index = FileUseIndex::new(&arrow);
    let declarations = arrow
        .ast
        .identifiers
        .iter()
        .filter(|identifier| {
            identifier.role == IdentifierRole::Binding
                && &arrow.source[identifier.span.start as usize..identifier.span.end as usize]
                    == "props"
        })
        .map(|identifier| identifier.span)
        .collect::<Vec<_>>();
    assert_eq!(declarations.len(), 3);
    for declaration in declarations {
        assert_eq!(index.dynamic_scope_uses(declaration).len(), 1);
    }
}

#[test]
fn wrapped_direct_eval_reaches_the_seed_but_indirect_eval_does_not() {
    for callee in [
        "eval",
        "(eval)",
        "(eval as any)",
        "eval!",
        "(<any>eval)",
        "(eval satisfies Function)",
    ] {
        let source =
            format!("function f() {{ const [value] = [() => 1]; return {callee}(\"value()\"); }}");
        assert_eq!(
            implicit_uses(&source, "value"),
            [(
                format!("{callee}(\"value()\")"),
                "eval-binding-unattested".into()
            )]
        );
    }
    for callee in ["(0, eval)", "globalThis.eval"] {
        let source =
            format!("function f() {{ const [value] = [() => 1]; return {callee}(\"value()\"); }}");
        assert!(implicit_uses(&source, "value").is_empty(), "{callee}");
    }
}

#[test]
fn syntax_alone_never_deletes_void_children() {
    for tag in [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ] {
        let source = format!(
            "const value = () => 1; const view = <div><section><{tag}><p class={{[\"x\", {{ k: value() }}]}} /></{tag}></section></div>;"
        );
        let start = source.find("value()").unwrap() as u32;
        let span = Span::new(start, start + "value()".len() as u32);
        let file = file_with_regions(
            "test.tsx",
            &source,
            vec![ExecutionRegion {
                span,
                reason: RegionReason::JsxAttribute,
            }],
        );
        assert_eq!(
            census_in(&file, "value", SeedKind::Accessor),
            ["unknown:runtime-execution"],
            "{tag}"
        );
    }
    for tag in ["div", "Input", "my-input"] {
        let source =
            format!("const value = () => 1; const view = <div><{tag}>{{value()}}</{tag}></div>;");
        assert_eq!(
            census_of("test.tsx", &source, "value", SeedKind::Accessor),
            ["unknown:runtime-execution"],
            "{tag}"
        );
    }
    // The runtime lowering of void-element attributes remains held.
    assert_eq!(
        census_of(
            "test.tsx",
            "const value = () => 1; const view = <input class={value()} />;",
            "value",
            SeedKind::Accessor
        ),
        ["unknown:runtime-execution"]
    );
}

#[test]
fn eval_and_arguments_outside_the_scope_reach_nothing() {
    let source = "export function other() { return eval(\"1\"); }\n\
                  export function viaParameter(props: { a: number }) {\n\
                    const [value] = [() => props.a];\n\
                    function inner() { return arguments.length; }\n\
                    return [value, inner];\n\
                  }\n\
                  export function arrowOwner() {\n\
                    return (props: { a: number }) => props.a;\n\
                  }";
    // `value` is not a parameter, and `eval` sits in another function.
    assert!(implicit_uses(source, "value").is_empty());
    // `inner`'s `arguments` is inner's own.
    assert!(implicit_uses(source, "props").is_empty());
}

#[test]
fn computed_keys_are_coerced_operands_never_names_or_destructures() {
    // F2 (repro/f1/only.ts, s9/k1.ts).
    let store = |body: &str| {
        census_of(
            "test.ts",
            &format!("const store: any = {{}};\nconst src: any = {{}};\n{body}"),
            "store",
            SeedKind::Store,
        )
    };
    assert_eq!(store("const a = { [store]: 1 };"), ["operation:coercion"]);
    assert_eq!(store("const a = { [store]() {} };"), ["operation:coercion"]);
    assert_eq!(
        store("const a = { get [store]() { return 1; } };"),
        ["operation:coercion"]
    );
    assert_eq!(
        store("const { [store]: out } = src; void out;"),
        ["operation:coercion"]
    );
    assert_eq!(
        store("const f = ({ [store]: out }: any) => out;"),
        ["operation:coercion"]
    );
    // The non-computed name is still a name, and a computed value a store.
    assert_eq!(
        census_of(
            "test.ts",
            "const store: any = {};\nconst a = { store: 1 };\nconst b = { [\"k\"]: store };",
            "store",
            SeedKind::Store
        ),
        ["transfer:property-store->property(k)"]
    );
}

/// A file whose compiler facts state `discarded` and live `operations`.
fn file_with_deletion(source: &str, discarded: Span, operations: Vec<Span>) -> FileFacts {
    let mut file = file("test.tsx", source);
    let mut compiler = (*file.compiler).clone();
    compiler.discarded_regions = vec![ExecutionRegion {
        span: discarded,
        reason: RegionReason::JsxAttribute,
    }];
    compiler.jsx_operations = std::iter::once(discarded)
        .chain(operations)
        .map(|span| solid_facts::compiler::JsxOperation {
            span,
            kind: "dynamic-attribute".into(),
        })
        .collect();
    file.compiler = Arc::new(compiler);
    file
}

#[test]
fn a_discarded_span_around_a_live_operation_excludes_nothing_inside_it() {
    // F3 (repro/f3/cls.tsx): the pinned compiler reports the class array
    // discarded and the toggle inside it live
    // (`a_class_array_reports_its_static_part_discarded_around_a_live_toggle`).
    let source = "const on = () => true;\nconst view = <p class={[\"x\", { k: on() }]} />;";
    let find = |needle: &str| {
        let start = u32::try_from(source.find(needle).unwrap()).unwrap();
        Span::new(start, start + u32::try_from(needle.len()).unwrap())
    };
    let array = find("[\"x\", { k: on() }]");
    let toggle = find("on()");
    let live = file_with_deletion(source, array, vec![toggle]);
    assert_eq!(
        census_in(&live, "on", SeedKind::Accessor),
        ["unknown:runtime-execution"]
    );
    // Operation absence supplies no deletion authority; this control stays unknown.
    let deleted = file_with_deletion(source, array, vec![]);
    assert_eq!(
        census_in(&deleted, "on", SeedKind::Accessor),
        ["unknown:runtime-execution"]
    );
}

#[test]
fn a_default_export_publishes_its_value() {
    // s2/lib.ts: `export default dflt` was `unrecorded-parent` and unpublished.
    assert_eq!(
        census_of(
            "test.ts",
            "const dflt = () => 4;\nexport default dflt;",
            "dflt",
            SeedKind::Accessor
        ),
        ["transfer:export->export(default)"]
    );
    assert_eq!(
        census_of(
            "test.ts",
            "const dflt = () => 4;\nexport default (dflt as unknown);",
            "dflt",
            SeedKind::Accessor
        ),
        ["transfer:export->export(default)"]
    );
}

#[test]
fn namespace_escapes_are_every_reference_but_a_static_member() {
    // s6/g2.ts.
    let source = "import * as ns from \"./lib\";\n\
                  export const x1 = ns.value();\n\
                  export const x2 = ns[\"value\"]();\n\
                  const k = \"value\" as const;\n\
                  export const x4 = ns[k]();\n\
                  export const x5 = Object.values(ns);\n\
                  export type T = typeof ns;";
    let file = file("test.ts", source);
    let index = FileUseIndex::new(&file);
    let escapes = index
        .namespace_escapes(declaration(&file, "ns"))
        .into_iter()
        .map(|span| {
            let line = file.source[..span.start as usize].matches('\n').count();
            (line, &file.source[span.start as usize..span.end as usize])
        })
        .collect::<Vec<_>>();
    assert_eq!(escapes, [(2, "ns"), (4, "ns"), (5, "ns")]);
}

#[test]
fn property_keys_have_one_spelling() {
    // m1.ts: `[acc]`, `{0: acc}`, `{"0": acc}`, `{[0]: acc}` all store under
    // "0", the key `store[0]` Gets.
    assert_eq!(
        accessor(
            "const a = [value]; const b = { 0: value }; const c = { \"0\": value };\n\
             const d = { [0]: value }; const e = { [\"0\"]: value };"
        ),
        [
            "transfer:container-store->property(0)",
            "transfer:property-store->property(0)",
            "transfer:property-store->property(0)",
            "transfer:property-store->property(0)",
            "transfer:property-store->property(0)",
        ]
    );
    // Spellings whose ToPropertyKey is not their text are summarized, never
    // guessed: `0x1` is "1", `1e3` is "1000", `1.0` is "1".
    assert_eq!(
        accessor("const a = { 0x1: value }; const b = { 1e3: value }; const c = { 1.0: value };"),
        [
            "transfer:property-store->property(*)",
            "transfer:property-store->property(*)",
            "transfer:property-store->property(*)",
        ]
    );
    assert_eq!(
        census_of(
            "test.ts",
            "const store: any = {};\nstore[0]; store[\"0\"];",
            "store",
            SeedKind::Store
        ),
        ["read:property-get(0)", "read:property-get(0)"]
    );
}

#[test]
fn a_proto_key_installs_a_prototype() {
    // m1/b2.
    assert_eq!(
        census_of(
            "test.ts",
            "const store: any = {};\n\
             const a = { __proto__: store };\n\
             const b = { \"__proto__\": store };\n\
             const c = { [\"__proto__\"]: store };\n\
             const __proto__ = store;\n\
             const d = { __proto__ };",
            "store",
            SeedKind::Store
        ),
        [
            "unknown:prototype-installation",
            "unknown:prototype-installation",
            // Computed and shorthand spellings define an own property.
            "transfer:property-store->property(__proto__)",
            "transfer:alias->binding",
        ]
    );
    assert_eq!(
        census_of(
            "test.ts",
            "const store: any = {}; const o: any = {}; o.__proto__ = store; o[\"__proto__\"] = store; o.x = store;",
            "store",
            SeedKind::Store
        ),
        [
            "unknown:prototype-installation",
            "unknown:prototype-installation",
            "transfer:property-store->property(x)",
        ]
    );
}

#[test]
fn dynamic_import_namespace_computed_uses_are_unknown() {
    let file = file(
        "dynamic.ts",
        r#"
async function f() {
  const m = (await (import("./lib") as any))!;
  m["value"]();
  m.value();
  const promise = import("./lib");
  const helper = await consume(import("./lib"));
  const nestedHelper = await consume(await import("./lib"));
}
"#,
    );
    assert_eq!(
        dynamic_namespace_declarations(&file),
        [declaration(&file, "m")]
    );
    let index = FileUseIndex::new(&file);
    let escapes = index.namespace_escapes(declaration(&file, "m"));
    assert_eq!(escapes.len(), 1);
    assert_eq!(
        &file.source[escapes[0].start as usize..escapes[0].end as usize + "[\"value\"]()".len()],
        "m[\"value\"]()"
    );
}

#[test]
fn escaped_proto_literal_keys_install_a_prototype() {
    assert_eq!(
        census_of(
            "test.ts",
            r#"const store: any = {};
const a = { "__pr\u006fto__": store };
const b = { __pr\u006fto__: store };
const c = { ["__pr\u006fto__"]: store };
"#,
            "store",
            SeedKind::Store
        ),
        [
            "unknown:prototype-installation",
            "unknown:prototype-installation",
            "transfer:property-store->property(*)",
        ]
    );
}

#[test]
fn protocol_uses_without_a_reference_are_recorded() {
    // m1.ts: the dispose Get at declaration and the call at block exit.
    assert_eq!(
        census_of(
            "test.ts",
            "declare const store: Disposable;\n\
             function f() { using held = store; void held; }\n\
             async function g() { await using later = (store as any); void later; }",
            "store",
            SeedKind::Store
        ),
        ["unknown:disposal-protocol", "unknown:disposal-protocol"]
    );
    // An intrinsic element's attributes are runtime operations on the
    // element, not stores into a props object.
    assert_eq!(
        census_of(
            "test.tsx",
            "declare function Comp(p: { a: () => void }): any;\n\
             const value = () => {};\n\
             const a = <div ref={value} onClick={value} title={value as any} />;\n\
             const b = <Comp a={value} />;",
            "value",
            SeedKind::Accessor
        ),
        [
            "unknown:runtime-execution",
            "unknown:runtime-execution",
            "unknown:runtime-execution",
            "unknown:runtime-execution",
        ]
    );
}

#[test]
fn canonical_order_is_project_relative() {
    let canon = Canon::new("/work/app/tsconfig.json");
    let address = |path: &str| SourceAddress {
        path: Arc::from(path),
        span: Span::new(1, 2),
    };
    assert_eq!(canon.key(&address("/work/app/src/a.ts")).0, "src/a.ts");
    // Outside the project: the absolute path, which sorts apart.
    assert_eq!(
        canon.key(&address("/store/node_modules/x/index.js")).0,
        "/store/node_modules/x/index.js"
    );
    // A checkout at another place orders the same way.
    let moved = Canon::new("/elsewhere/a-much-longer-name/tsconfig.json");
    let order = |canon: &Canon, root: &str| {
        let mut paths = [
            address(&format!("{root}/src/z.ts")),
            address("/store/node_modules/x/index.js"),
            address(&format!("{root}/lib/a.ts")),
        ];
        paths.sort_by(|left, right| canon.key(left).cmp(&canon.key(right)));
        paths
            .iter()
            .map(|path| canon.key(path).0.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        order(&canon, "/work/app"),
        order(&moved, "/elsewhere/a-much-longer-name")
    );
}
