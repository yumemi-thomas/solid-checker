use super::*;
use solid_facts::compiler::COMPILER_FACTS_PROTOCOL;
use solid_facts::core::SourceHash;
use typefacts::SourceDigest;

/// The wire schema the in-memory stub claims; the real value comes from
/// the producer's handshake, which no stub performs.
const STUB_SCHEMA: u64 = 2;

#[test]
fn returned_literal_functions_have_exact_structural_body_facts() {
    let file = test_file_facts(
        "returned.ts",
        r#"
function direct() { return (() => 1) as () => number; }
function object() { return { run: () => 2 }; }
function tuple() { return [() => 3] as const; }
function opaque() { consume(() => 4); }
"#,
    );
    let facts = structural_functions(&file);
    for (text, present) in [
        ("() => 1", true),
        ("() => 2", true),
        ("() => 3", true),
        ("() => 4", false),
    ] {
        let function = file
            .ast
            .functions
            .iter()
            .find(|function| file.source_text(function.span) == Some(text))
            .unwrap();
        let fact = facts
            .iter()
            .find(|fact| fact.body.start_byte == u64::from(function.body.start));
        assert_eq!(fact.is_some(), present, "{text}: {facts:#?}");
        if let Some(fact) = fact {
            assert_eq!(
                fact.name,
                typefacts_location(file.path.as_str(), function.span)
            );
            assert_eq!(fact.body.end_byte, u64::from(function.body.end));
            assert!(fact.arrow);
            assert!(!fact.exported);
        }
    }
}

struct Compiler;
impl CompilerFactsProvider for Compiler {
    fn analyze(
        &mut self,
        request: &AnalysisRequest,
    ) -> Result<ExecutionMap, CompilerProviderError> {
        Ok(ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: request.source_hash.clone(),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        })
    }
}

struct CountingCompiler(usize);
impl CompilerFactsProvider for CountingCompiler {
    fn analyze(
        &mut self,
        request: &AnalysisRequest,
    ) -> Result<ExecutionMap, CompilerProviderError> {
        self.0 += 1;
        Compiler.analyze(request)
    }
}

/// Stands in for a retained session: it answers with a table stamped for
/// the generation it is on, and advances the way a real session does when
/// the caller moves to the next one.
struct Types {
    source: SourceDigest,
    project_id: String,
    generation: u64,
}
impl TypeFactsProvider for Types {
    fn semantic_grouped(
        &mut self,
        _groups: &[SemanticDemandGroup<'_>],
    ) -> Result<TypeScriptTable, BackendError> {
        let generation = self.generation;
        self.generation += 1;
        Ok(TypeScriptTable::from_parts(
            STUB_SCHEMA,
            generation,
            self.project_id.clone(),
            vec![self.source.clone()],
            vec![],
            vec![],
            vec![],
        ))
    }
}

#[test]
fn imported_reference_space_does_not_add_baseline_symbol_observations() {
    let source = "import { Constructor } from 'package'; const wrapped = new (Constructor as typeof Constructor)();";
    let file = test_file_facts("src/main.ts", source);
    let demands = semantic_demands(
        dialect::default_dialect(),
        &[file],
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    let start = source.rfind("Constructor").unwrap() as u64;
    let row = demands
        .iter()
        .find(|demand| {
            demand.location.start_byte == start && demand.location.end_byte == start + 11
        })
        .unwrap();
    assert!(row.reference_space);
    assert!(!row.symbol);
    assert!(!row.runtime_identity);
}

fn test_file_facts(path: &str, source: &str) -> FileFacts {
    let ast = solid_facts::ast::extract(path, source).unwrap();
    FileFacts::new(
        Generation::new(1).unwrap(),
        source,
        ast,
        ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of(source),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        },
    )
    .unwrap()
}

#[test]
fn import_use_spaces_are_demanded_at_exact_binder_selected_references() {
    let file = test_file_facts(
        "src/main.ts",
        "import { value } from './helper'; console.log(value); type T = typeof value; function server() { 'use server'; return value; } function shadow(value: number) { return value; }",
    );
    let demands = semantic_demands(
        dialect::default_dialect(),
        std::slice::from_ref(&file),
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    let binding = file.ast.imports[0].bindings[0].local.span;
    let mut imported = 0;
    let mut shadowed = 0;
    for (reference, declaration) in &file.ast.reference_declarations {
        let location = typefacts_location(file.path.as_str(), *reference);
        let classified = demands
            .iter()
            .any(|demand| demand.location == location && demand.reference_space);
        if *declaration == binding {
            imported += 1;
            assert!(
                classified,
                "value, type-query, and server uses all need exact classification"
            );
        } else {
            shadowed += 1;
            assert!(
                !classified,
                "a shadowed binding cannot become an import use"
            );
        }
    }
    assert_eq!(imported, 3);
    assert_eq!(shadowed, 1);
}

#[test]
fn retained_demands_and_indexed_hydration_match_fresh_results() {
    let files = vec![
        test_file_facts(
            "src/b.tsx",
            "export const B = () => <div>{createSignal(1)}</div>;",
        ),
        test_file_facts(
            "src/a.ts",
            "export function A(value: number) { return value; }",
        ),
    ];
    let dialect = dialect::default_dialect();
    let fresh_demands = semantic_demands(dialect, &files, SemanticDemandOptions::NONE).unwrap();
    let mut cache = FactsCache::default();
    let retained_demands =
        semantic_demands_cached(dialect, &files, &mut cache, SemanticDemandOptions::NONE).unwrap();
    assert_eq!(retained_demands, fresh_demands);

    let fresh_table = TypeScriptTable::from_parts(
        STUB_SCHEMA,
        1,
        "project",
        vec![],
        vec![],
        vec![],
        files
            .iter()
            .rev()
            .map(|file| typefacts::FileFact {
                path: file.path.to_string().into(),
                calls: vec![].into(),
                bindings: vec![].into(),
                functions: vec![].into(),
                async_functions: vec![].into(),
            })
            .collect(),
    );
    let retained_table = fresh_table.clone();
    let fresh_table = hydrate_structural_file_facts(fresh_table, &files);
    let retained_table = hydrate_structural_file_facts_cached(retained_table, &files, &mut cache);
    assert_eq!(
        retained_table.files().collect::<Vec<_>>(),
        fresh_table.files().collect::<Vec<_>>()
    );
}

#[test]
fn semantic_demand_plan_is_complete_for_downstream_consumers() {
    let file = test_file_facts(
        "src/component.tsx",
        // Every export shape the `kind` decision can be asked about, not
        // just the declaration form: a specifier list, a class
        // declaration, an anonymous default class (whose recorded span is
        // the class node, there being no name), and a destructuring
        // declarator. Each must carry both signature facts, because
        // `export_kind_proof` reads absence at one of these spans as a
        // refusal.
        "const value = createMemo(async () => 1); const effectArgs: [() => number] = [() => 1]; createEffect(...effectArgs); const local = 1; const o: { a: number } = { a: 1 }; export { local }; export class K {} class KK extends K {} export const { a } = o; export default class {} export function Card(props: { title: string }) { const key = 'title'; const copy = { ...props }; return <div>{props[key]}{copy.title}{value()}</div>; }",
    );
    let demands = semantic_demands(
        dialect::default_dialect(),
        std::slice::from_ref(&file),
        SemanticDemandOptions::NONE,
    )
    .unwrap();

    for member in &file.ast.members {
        let location = typefacts_location(file.path.as_str(), member.object);
        assert!(
            demands
                .iter()
                .any(|demand| demand.symbol && demand.location == location),
            "member object {location:?} must retain symbol provenance"
        );
    }
    // ADR 0317: a class's `extends` expression names its base class.
    for heritage in file.ast.classes.iter().filter_map(|class| class.heritage) {
        let location = typefacts_location(file.path.as_str(), heritage);
        assert!(
            demands
                .iter()
                .any(|demand| demand.symbol && demand.location == location),
            "heritage {location:?} must retain symbol provenance"
        );
    }
    for spread in &file.ast.spreads {
        let location = typefacts_location(file.path.as_str(), spread.argument);
        assert!(
            demands
                .iter()
                .any(|demand| demand.symbol && demand.location == location),
            "spread argument {location:?} must retain symbol provenance"
        );
    }
    for call in &file.ast.calls {
        let location = typefacts_location(file.path.as_str(), call.callee);
        let demand = demands
            .iter()
            .find(|demand| demand.location == location && demand.query_location.is_some())
            .expect("every call callee needs a symbol/type query");
        assert!(demand.symbol);
        assert_eq!(demand.type_descriptor, call.arguments.is_empty());
        let property = callee_property_location(&file.source, &location);
        if property != location {
            assert!(demands.iter().any(|demand| {
                demand.location == property && demand.symbol && demand.query_location.is_none()
            }));
        }
        for argument in call
            .arguments
            .iter()
            .filter(|argument| argument.value == solid_facts::ast::ArgumentValueKind::Identifier)
        {
            let argument_location = typefacts_location(file.path.as_str(), argument.span);
            assert!(
                demands.iter().any(|demand| {
                    demand.location == argument_location
                        && demand.symbol
                        && demand.type_descriptor
                        && demand.callability
                }),
                "identifier call arguments need compiler callability for runtime semantics"
            );
        }
        for argument in call.arguments.iter().filter(|argument| !argument.spread) {
            let argument_location = typefacts_location(file.path.as_str(), argument.span);
            assert!(demands.iter().any(|demand| {
                demand.location == argument_location && demand.primitive_value_domain
            }));
        }
        for argument in call.arguments.iter().filter(|argument| argument.spread) {
            let argument_location = typefacts_location(
                file.path.as_str(),
                file.ast
                    .spreads
                    .iter()
                    .find(|spread| spread.span == argument.span)
                    .map_or(argument.span, |spread| spread.argument),
            );
            assert!(
                demands.iter().any(|demand| demand.symbol
                    && demand.tuple_shape
                    && demand.location == argument_location),
                "call spread {argument_location:?} must retain symbol provenance and exact tuple shape"
            );
        }
    }
    for import in &file.ast.imports {
        for binding in &import.bindings {
            let location = typefacts_location(file.path.as_str(), binding.local.span);
            assert!(demands.iter().any(|demand| {
                demand.location == location && demand.reference_space && demand.runtime_identity
            }));
        }
    }
    for export in &file.ast.exports {
        for item in export.specifiers.iter().chain(&export.declarations) {
            let location = typefacts_location(file.path.as_str(), item.local.span);
            // Both signature facts, at every export specifier and every
            // exported declaration name. `export_kind_proof` treats an
            // absent fact at one of these spans as a *refusal* rather than
            // as a keep-the-summary no-op, which is only honest because
            // this invariant holds: absence there is the producer finding
            // no node to classify, never the plan declining to ask.
            assert!(demands.iter().any(|demand| {
                demand.location == location
                    && demand.callability
                    && demand.constructability
                    && demand.runtime_identity
            }));
        }
    }
    assert!(
        demands.iter().any(|demand| {
            demand.r#async && demand.location.path.as_ref() == file.path.as_str()
        })
    );
    assert!(
        demands.windows(2).all(|pair| pair[0] != pair[1]),
        "the transport plan must not contain duplicate queries"
    );
    assert!(
        demands
            .windows(2)
            .all(|pair| pair[0].location != pair[1].location),
        "each syntax location must combine all requested compiler facts"
    );
    let mut reversed = vec![
        file.clone(),
        test_file_facts("src/a.ts", "export const a = 1;"),
    ];
    let planned = semantic_demands(
        dialect::default_dialect(),
        &reversed,
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    reversed.reverse();
    assert_eq!(
        planned,
        semantic_demands(
            dialect::default_dialect(),
            &reversed,
            SemanticDemandOptions::NONE,
        )
        .unwrap(),
        "query order must not depend on source traversal order"
    );
}

#[test]
fn array_shape_demands_are_limited_to_compiler_proven_jsx_maps() {
    let mut file = test_file_facts(
        "src/lists.tsx",
        "const rendered = <ul>{items().map(item => item)}</ul>; const computed = items().map(item => item);",
    );
    let map_calls = file
        .ast
        .calls
        .iter()
        .filter(|call| {
            file.ast.members.iter().any(|member| {
                member.span == call.callee && file.source_text(member.property) == Some("map")
            })
        })
        .map(|call| call.span)
        .collect::<Vec<_>>();
    assert_eq!(map_calls.len(), 2);
    Arc::make_mut(&mut file.compiler)
        .jsx_operations
        .push(solid_facts::compiler::JsxOperation {
            span: map_calls[0],
            kind: "jsx-expression".into(),
        });

    let default_demands = semantic_demands(
        dialect::default_dialect(),
        &[file.clone()],
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    assert!(
        default_demands.iter().all(|demand| !demand.array_shape),
        "the low-level NONE option must not request map receiver shapes"
    );

    let demands = semantic_demands(
        dialect::default_dialect(),
        &[file.clone()],
        SemanticDemandOptions::PREFERENCES,
    )
    .unwrap();
    for (index, call_span) in map_calls.into_iter().enumerate() {
        let member = file
            .ast
            .members
            .iter()
            .find(|member| {
                member.span
                    == file
                        .ast
                        .calls
                        .iter()
                        .find(|call| call.span == call_span)
                        .unwrap()
                        .callee
            })
            .unwrap();
        let location = typefacts_location(file.path.as_str(), member.object);
        let array_shape = demands
            .iter()
            .find(|demand| demand.location == location)
            .is_some_and(|demand| demand.array_shape);
        assert_eq!(
            array_shape,
            index == 0,
            "only the compiler-proven JSX child map should request an array-shape query"
        );
    }
}

#[test]
fn declared_key_demands_are_limited_to_intrinsic_element_spreads() {
    let file = test_file_facts(
        "src/spreads.tsx",
        "const a = <div {...props} />; const b = <Card {...props} />; const c = <ui.Box {...props} />;",
    );
    let spread_span = |tag: &str| {
        file.ast
            .jsx_elements
            .iter()
            .find(|element| file.source_text(element.name.span) == Some(tag))
            .and_then(|element| element.spreads.first())
            .map(|spread| spread.argument)
            .unwrap_or_else(|| panic!("no spread on {tag}"))
    };
    let demands = semantic_demands(
        dialect::default_dialect(),
        std::slice::from_ref(&file),
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    let declared_keys = |tag: &str| {
        let location = typefacts_location(file.path.as_str(), spread_span(tag));
        demands
            .iter()
            .any(|demand| demand.location == location && demand.declared_keys)
    };
    assert!(declared_keys("div"), "an intrinsic element's spread asks");
    assert!(
        !declared_keys("Card"),
        "a component's spread is not a handler site"
    );
    assert!(
        demands
            .iter()
            .filter(|demand| demand.declared_keys)
            .all(|demand| !demand.symbol),
        "the demand carries no symbol observation"
    );
}

/// ADR 0465: each link a member assignment or an in-place array call reads
/// past its root is asked for its primitive value domain; the root and the
/// written member are not links.
#[test]
fn store_write_chain_links_ask_for_their_primitive_value_domain() {
    let source =
        "store.v = 1; store.a.b = 2; store.n!.c.d = 3; store.items.push(1); store.items.slice(0);";
    let file = test_file_facts("src/writes.ts", source);
    let demands = semantic_demands(
        dialect::default_dialect(),
        std::slice::from_ref(&file),
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    let asked = |text: &str, nth: usize| {
        let start = source
            .match_indices(text)
            .nth(nth)
            .unwrap_or_else(|| panic!("no {text}"))
            .0;
        let span = Span::new(
            u32::try_from(start).unwrap(),
            u32::try_from(start + text.len()).unwrap(),
        );
        let location = typefacts_location(file.path.as_str(), span);
        demands
            .iter()
            .any(|demand| demand.location == location && demand.primitive_value_domain)
    };
    assert!(asked("store.a", 0), "an assignment's link");
    assert!(asked("store.n!", 0), "a wrapped link, as written");
    assert!(asked("store.n!.c", 0), "every link of a longer chain");
    assert!(asked("store.items", 0), "a mutator's receiver");
    assert!(!asked("store.v", 0), "the written member is no link");
    assert!(!asked("store.items", 1), "a non-mutating call asks nothing");
}

#[test]
fn returned_member_calls_combine_symbol_and_resolved_call_demands() {
    let file = test_file_facts(
        "src/cleanup.ts",
        "export function subscribe(source: { on(): () => void }) { return source.on(); }",
    );
    let returned = file
        .ast
        .returns
        .iter()
        .find_map(|returned| returned.callee)
        .expect("returned call");
    let location = typefacts_location(file.path.as_str(), returned);
    let demands = semantic_demands(
        dialect::default_dialect(),
        std::slice::from_ref(&file),
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    let matching = demands
        .iter()
        .filter(|demand| demand.location == location)
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1);
    assert!(matching[0].symbol);
    assert!(matching[0].resolved_call);
    assert!(matching[0].query_location.is_some());
}

/// A dialect primitive's call is demanded its resolved call with no
/// argument too, because the declaration is what names the package the
/// `creates` audits are keyed on. An argumentless local call is the
/// control, and so is a primitive's name imported from a module the
/// vocabulary does not own.
#[test]
fn an_argumentless_primitive_call_is_demanded_its_resolved_call() {
    let file = test_file_facts(
        "src/owner.ts",
        "import { getOwner, flush as settle } from \"solid-js\";\n\
             import * as Solid from \"solid-js\";\n\
             import { getOwner as foreign } from \"./local\";\n\
             function local() {}\n\
             export function probe() {\n\
               const owner = getOwner();\n\
               settle();\n\
               const again = Solid.getOwner();\n\
               foreign();\n\
               local();\n\
               return owner === again;\n\
             }",
    );
    let demands = semantic_demands(
        dialect::default_dialect(),
        std::slice::from_ref(&file),
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    let resolved = |spelling: &str| {
        let call = file
            .ast
            .calls
            .iter()
            .find(|call| file.source_text(call.callee) == Some(spelling))
            .expect(spelling);
        let location = typefacts_location(file.path.as_str(), call.callee);
        demands
            .iter()
            .any(|demand| demand.location == location && demand.resolved_call)
    };
    assert!(resolved("getOwner"));
    assert!(resolved("settle"), "an aliased named import");
    assert!(resolved("Solid.getOwner"), "a namespace member");
    assert!(
        !resolved("foreign"),
        "a module the vocabulary does not own names no primitive"
    );
    assert!(
        !resolved("local"),
        "an argumentless local call stays as it was"
    );
}

/// SC1002 needs the standard-library declaration of an argumentless call that
/// runs before an async function's first suspension (`Promise.resolve()`,
/// `Math.random()`); a call after it, and one in a function that never
/// suspends, stay as they were.
#[test]
fn an_argumentless_call_before_a_suspension_is_demanded_its_resolved_call() {
    let file = test_file_facts(
        "src/before.ts",
        "export async function probe() {\n\
           Math.random();\n\
           local();\n\
           await Promise.resolve();\n\
           Date.now();\n\
         }\n\
         function local() {}\n\
         export function quiet() {\n\
           Math.floor();\n\
         }",
    );
    let demands = semantic_demands(
        dialect::default_dialect(),
        std::slice::from_ref(&file),
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    let resolved = |spelling: &str| {
        let call = file
            .ast
            .calls
            .iter()
            .find(|call| file.source_text(call.callee) == Some(spelling))
            .expect(spelling);
        let location = typefacts_location(file.path.as_str(), call.callee);
        demands
            .iter()
            .any(|demand| demand.location == location && demand.resolved_call)
    };
    assert!(resolved("Math.random"));
    assert!(resolved("Promise.resolve"), "the awaited operand");
    assert!(!resolved("local"), "a project function is not a global");
    assert!(!resolved("Date.now"), "after the only suspension");
    assert!(!resolved("Math.floor"), "a function that never suspends");
}

#[test]
fn argumentless_factory_result_calls_demand_validity_by_exact_binding() {
    let file = test_file_facts(
        "src/captured.tsx",
        r#"declare function factory(): () => void;
declare function ordinary(): void;
function Card(parameter: () => void) {
  const returned = factory();
  const alias = returned;
  returned(); alias(); parameter(); ordinary();
  { const returned = parameter; returned(); }
  return <p />;
}"#,
    );
    let demands = semantic_demands(
        dialect::default_dialect(),
        std::slice::from_ref(&file),
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    let returned_declaration = file
        .ast
        .bindings
        .iter()
        .find(|binding| binding.call_initializer.is_some())
        .unwrap()
        .names[0]
        .span;
    let mut checked = 0;
    for call in file.ast.calls.iter().filter(|call| {
        matches!(
            file.source_text(call.callee),
            Some("returned" | "alias" | "parameter" | "ordinary")
        )
    }) {
        let expected = file.ast.reference_declaration(call.callee) == Some(returned_declaration);
        let location = typefacts_location(file.path.as_str(), call.callee);
        assert_eq!(
            demands
                .iter()
                .any(|demand| { demand.location == location && demand.resolved_call }),
            expected,
            "{}",
            file.source_text(call.callee).unwrap()
        );
        checked += 1;
    }
    assert_eq!(checked, 5);
}

#[test]
fn argumentless_nested_function_calls_demand_validity_by_exact_binding() {
    let file = test_file_facts(
        "src/captured.tsx",
        r#"declare function top(): void;
function Card(props: { value: string }, parameter: () => string) {
  const browse = () => props.value;
  function label() { return props.value; }
  const alias = browse;
  const object = { browse };
  browse(); label(); parameter(); alias(); object.browse(); top();
  { const browse = parameter; browse(); }
  return <p />;
}"#,
    );
    let demands = semantic_demands(
        dialect::default_dialect(),
        std::slice::from_ref(&file),
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    for call in &file.ast.calls {
        let location = typefacts_location(file.path.as_str(), call.callee);
        let planned = demands
            .iter()
            .find(|demand| demand.location == location)
            .unwrap();
        let declaration = file.ast.reference_declaration(call.callee);
        let expected = file.source_text(call.callee) == Some("label")
            || (file.source_text(call.callee) == Some("browse")
                && declaration.is_some_and(|span| {
                    file.ast.bindings.iter().any(|binding| {
                        binding.names.iter().any(|name| name.span == span)
                            && binding.initializer_function
                    })
                }));
        assert_eq!(planned.resolved_call, expected, "{:?}", call.span);
    }
}

/// ADRs 0190 and 0192: only an argumentless method call's resolved
/// declaration says whether the member is a standard-library built-in.
/// Unrelated bodies stay outside the leaf demand closure: a resolved
/// declaration also enters the symbol index.
#[test]
fn an_argumentless_method_call_is_demanded_its_resolved_call() {
    let file = test_file_facts(
        "src/words.ts",
        "import { onSettled } from \"solid-js\";\n\
             declare const dialog: { focus(): void } | undefined;\n\
             const fixed = \"a b\";\n\
             function local() {}\n\
             export function words(message: string) {\n\
               const head = fixed.trim();\n\
               local();\n\
               onSettled(() => { dialog?.focus(); });\n\
               void head;\n\
               return message.trim().split(\" \");\n\
             }\n\
             export const lower = (value?: string) => value?.toLowerCase().length;\n\
             export function formatted(input: string) { return input\n .toLowerCase().split(' '); }\n\
             export function commented(input: string) { return input /* receiver */ .trim().split(' '); }\n\
             export function wrapped(input: string) { return ((input) as string).trim().split(' '); }\n\
             export function chained(input: string) { const result = input.trim().toLowerCase(); return result; }\n\
             export function shadowed(subject: string) { function inner() { const subject = 'x'; const out = subject.toUpperCase().toLowerCase(); return out; } return inner(); }\n\
             export function indexed(input: string) { const result = input[0]?.toUpperCase(); return result; }",
    );
    let demands = semantic_demands(
        dialect::default_dialect(),
        std::slice::from_ref(&file),
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    let resolved = |spelling: &str| {
        let call = file
            .ast
            .calls
            .iter()
            .find(|call| file.source_text(call.callee) == Some(spelling))
            .expect(spelling);
        let location = typefacts_location(file.path.as_str(), call.callee);
        demands
            .iter()
            .any(|demand| demand.location == location && demand.resolved_call)
    };
    assert!(resolved("message.trim"), "inside a chain");
    assert!(resolved("value?.toLowerCase"), "an optional member call");
    assert!(resolved("input\n .toLowerCase"), "a formatted chain");
    assert!(
        resolved("input /* receiver */ .trim"),
        "a commented receiver"
    );
    assert!(
        resolved("((input) as string).trim"),
        "a transparent wrapper"
    );
    assert!(
        resolved("input.trim().toLowerCase"),
        "a call-result receiver in a chain"
    );
    assert!(
        !resolved("subject.toUpperCase"),
        "a local shadow is not the enclosing parameter"
    );
    assert!(resolved("input[0]?.toUpperCase"), "an indexed receiver");
    assert!(
        resolved("dialog?.focus"),
        "directly in a leaf-owner callback"
    );
    assert!(
        !resolved("fixed.trim"),
        "any other method call stays as it was"
    );
    assert!(
        !resolved("local"),
        "an argumentless plain call stays as it was"
    );
}

#[test]
fn leaf_demands_follow_local_helpers_without_demanding_unrelated_bodies() {
    let file = test_file_facts(
        "src/helpers.ts",
        r#"import { onSettled } from "solid-js";
const captured = "value";
function second() { captured.trim(); }
const first = () => { second(); };
function unused() { captured.toUpperCase(); }
function shadowed() { captured.toLowerCase(); }
function entry(shadowed: () => void) {
    shadowed();
    first();
}
onSettled(() => { entry(() => {}); });
"#,
    );
    let demands = semantic_demands(
        dialect::default_dialect(),
        std::slice::from_ref(&file),
        SemanticDemandOptions::NONE,
    )
    .unwrap();
    let resolved = |spelling: &str| {
        let call = file
            .ast
            .calls
            .iter()
            .find(|call| file.source_text(call.callee) == Some(spelling))
            .expect(spelling);
        let location = typefacts_location(file.path.as_str(), call.callee);
        demands
            .iter()
            .any(|demand| demand.location == location && demand.resolved_call)
    };
    assert!(resolved("captured.trim"), "transitive local helper");
    assert!(!resolved("captured.toUpperCase"), "unrelated function");
    assert!(!resolved("captured.toLowerCase"), "shadowed function");
}

#[test]
fn joins_all_three_fact_sources() {
    let source = "export const answer = 42;";
    let mut compiler = Compiler;
    let mut types = Types {
        project_id: "project".into(),
        generation: 1,
        source: SourceDigest {
            path: "src/a.ts".into(),
            sha256: typefacts::SourceHash::of(source),
        },
    };
    let project = build_project(
        dialect::default_dialect(),
        "project",
        1,
        vec![SourceFile {
            path: "src/a.ts".into(),
            source: source.into(),
            compiler_options: CompilerOptions::default(),
        }],
        &mut compiler,
        &mut types,
    )
    .unwrap();
    assert_eq!(project.files.len(), 1);
    assert_eq!(
        project.typescript.sources().collect::<Vec<_>>(),
        vec![&types.source]
    );
}

/// How many analyses the stub dialect's compiler has performed; proves
/// the native pipeline ran the selected dialect's compiler rather than
/// the default one.
static STUB_COMPILER_RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct RecordingStubCompiler;
impl CompilerFactsProvider for RecordingStubCompiler {
    fn analyze(
        &mut self,
        request: &AnalysisRequest,
    ) -> Result<ExecutionMap, CompilerProviderError> {
        STUB_COMPILER_RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Compiler.analyze(request)
    }
}

const STUB_RULE: solid_reactive_ir::RuleMetadata = solid_reactive_ir::RuleMetadata {
    code: "ST0001",
    name: "stub-dialect-ran",
    severity: "warning",
    uncertifiable: false,
    default_enabled: true,
    configurable: true,
    presets: &[],
};

fn stub_solve(
    _program: &solid_reactive_ir::Program,
) -> (
    Vec<solid_reactive_ir::Finding>,
    solid_reactive_ir::SolveTimings,
) {
    (
        vec![solid_reactive_ir::Finding::new(
            STUB_RULE,
            "the stub dialect's catalog examined this program".into(),
            typefacts::Location {
                path: "src/a.ts".into(),
                start_byte: 0,
                end_byte: 0,
            },
        )],
        solid_reactive_ir::SolveTimings::default(),
    )
}

static STUB_DIALECT: dialect::Dialect = dialect::Dialect {
    id: "stub-dialect",
    compiler_facts_identity: "stub-compiler:trace0:test",
    vocabulary: &solid_dialect::Solid2,
    rule_count: 1,
    compiler: || Box::new(RecordingStubCompiler),
    solve_measured: stub_solve,
    docs_url: |rule| format!("stub://docs/{rule}"),
    has_rule: |_| false,
    has_code: |_| false,
    rule_metadata: |rule| (rule == STUB_RULE.name).then_some(STUB_RULE),
    semantic_demands: dialect::SemanticDemandCapabilities::NONE,
    catalog_capabilities: solid_reactive_ir::CatalogCapabilities::SOLID_2,
};

/// An alternate dialect flows end to end through the native pipeline:
/// its compiler produces the execution maps, its catalog produces the
/// findings, and its cache keys never collide with another dialect's.
#[test]
fn alternate_dialect_flows_through_native_pipeline() {
    let source = "export const answer = 42;";
    let mut types = Types {
        project_id: "project/tsconfig.json".into(),
        generation: 1,
        source: SourceDigest {
            path: "src/a.ts".into(),
            sha256: typefacts::SourceHash::of(source),
        },
    };
    let sources = vec![SourceFile {
        path: "src/a.ts".into(),
        source: source.into(),
        compiler_options: CompilerOptions::default(),
    }];
    let facts = build_project_native(
        &STUB_DIALECT,
        "project/tsconfig.json",
        1,
        sources.clone(),
        &mut types,
    )
    .unwrap();
    assert!(
        STUB_COMPILER_RUNS.load(std::sync::atomic::Ordering::SeqCst) > 0,
        "the stub dialect's compiler must produce the execution maps"
    );
    let analysis = analyze_project_inputs_measured_with_enablement(
        &STUB_DIALECT,
        std::path::Path::new("project/tsconfig.json"),
        &sources,
        &facts,
        solid_reactive_ir::AnalysisInputs::empty(),
        RequestedRuleEnablement::default(),
    )
    .unwrap()
    .0;
    let identities = analysis
        .snapshot
        .findings
        .iter()
        .map(|finding| (finding.id.as_str(), finding.rule.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        vec![("ST0001", "stub-dialect-ran")],
        "the stub dialect's catalog must supply the findings"
    );

    let request = AnalysisRequest::new(
        "src/a.ts",
        Arc::<str>::from(source),
        CompilerOptions::default(),
    );
    let stub_key = compiler_cache_key(&STUB_DIALECT, &request).unwrap();
    assert!(stub_key.contains(STUB_DIALECT.compiler_facts_identity));
    assert!(stub_key.contains(&format!("\0{}\0", COMPILER_FACTS_PROTOCOL)));
    assert_ne!(
        stub_key,
        compiler_cache_key(dialect::default_dialect(), &request).unwrap(),
        "an execution map cached by one dialect must never answer for another"
    );
}

/// Regression for the real defect in `@solidjs/start@2.0.3`'s
/// `dist/shared/dev-toolbar/index.jsx`, which imports its own
/// `package.json`: the TypeScript program legitimately hands a `.json`
/// module to the native build alongside ordinary JS/TS sources, and the
/// build must certify it inert -- not crash, and not invoke a JS/JSX
/// compiler that has no grammar for JSON -- rather than merely tolerate
/// it.
/// Stands in for a retained session the same way [`Types`] does, but
/// over an arbitrary source set: this regression needs digests for both
/// the JS entry and the JSON module it imports, and [`Types`] only ever
/// carries one.
struct MultiSourceTypes {
    project_id: String,
    generation: u64,
    sources: Vec<SourceDigest>,
}
impl TypeFactsProvider for MultiSourceTypes {
    fn semantic_grouped(
        &mut self,
        _groups: &[SemanticDemandGroup<'_>],
    ) -> Result<TypeScriptTable, BackendError> {
        let generation = self.generation;
        self.generation += 1;
        Ok(TypeScriptTable::from_parts(
            STUB_SCHEMA,
            generation,
            self.project_id.clone(),
            self.sources.clone(),
            vec![],
            vec![],
            vec![],
        ))
    }
}

#[test]
fn a_json_import_source_is_certified_inert_without_invoking_the_compiler() {
    let index_source = "import pkg from \"./package.json\";\nexport const version = pkg.version;\n";
    let json_source = r#"{"name": "demo", "version": "1.0.0"}"#;
    let mut types = MultiSourceTypes {
        project_id: "project/tsconfig.json".into(),
        generation: 1,
        sources: vec![
            SourceDigest {
                path: "index.mjs".into(),
                sha256: typefacts::SourceHash::of(index_source),
            },
            SourceDigest {
                path: "package.json".into(),
                sha256: typefacts::SourceHash::of(json_source),
            },
        ],
    };
    let sources = vec![
        SourceFile {
            path: "index.mjs".into(),
            source: index_source.into(),
            compiler_options: CompilerOptions::default(),
        },
        SourceFile {
            path: "package.json".into(),
            source: json_source.into(),
            compiler_options: CompilerOptions::default(),
        },
    ];
    // `build_project` takes its `CompilerFactsProvider` directly rather
    // than the dialect constructing one internally, so a local counter
    // proves how many files it ran on without racing another test's
    // shared dialect-compiler counter.
    let mut compiler = CountingCompiler(0);
    let facts = build_project(
        dialect::default_dialect(),
        "project/tsconfig.json",
        1,
        sources,
        &mut compiler,
        &mut types,
    )
    .unwrap();

    let json_file = facts
        .files
        .iter()
        .find(|file| file.path.as_str() == "package.json")
        .expect("the JSON module must still be enrolled as an analyzed file");
    assert!(json_file.ast.calls.is_empty());
    assert!(json_file.ast.bindings.is_empty());
    assert!(json_file.ast.functions.is_empty());
    assert!(json_file.ast.imports.is_empty());
    assert!(json_file.ast.exports.is_empty());
    assert!(json_file.ast.members.is_empty());
    assert!(json_file.compiler.tracked_regions.is_empty());
    assert!(json_file.compiler.untracked_regions.is_empty());
    // Every execution-fact array, enumerated: the claim is that a JSON
    // module has *no* execution facts, so a new array left out of this list
    // would let the next one arrive populated with this test still green.
    assert!(json_file.compiler.discarded_regions.is_empty());
    assert!(json_file.compiler.ownership_regions.is_empty());
    assert!(json_file.compiler.callback_roles.is_empty());
    assert!(json_file.compiler.jsx_operations.is_empty());

    // Only `index.mjs` needed the dialect's compiler; the JSON module's
    // inertness is a proof from its module kind, not a question the JS/JSX
    // compiler was asked and happened to answer favorably.
    assert_eq!(
        compiler.0, 1,
        "the dialect compiler must run for index.mjs only, never for the JSON module"
    );
}

/// The JSON fix is an extension/module-kind rule, not a blanket "skip
/// anything unparseable": a source reached through a genuinely
/// unsupported extension must still fail the native build closed, exactly
/// as it did before the JSON module kind got its own path.
#[test]
fn a_non_json_unsupported_extension_still_fails_the_native_build() {
    let source = "binary";
    let mut types = Types {
        project_id: "project/tsconfig.json".into(),
        generation: 1,
        source: SourceDigest {
            path: "asset.wasm".into(),
            sha256: typefacts::SourceHash::of(source),
        },
    };
    let sources = vec![SourceFile {
        path: "asset.wasm".into(),
        source: source.into(),
        compiler_options: CompilerOptions::default(),
    }];
    let error = build_project_native(
        &STUB_DIALECT,
        "project/tsconfig.json",
        1,
        sources,
        &mut types,
    )
    .expect_err("a non-JSON unsupported extension must still fail closed, not be skipped");
    assert!(
        matches!(
            error,
            BackendError::Ast(solid_facts::ast::AstFactsError::SourceType { .. })
        ),
        "unexpected error variant: {error:?}"
    );
}

#[test]
fn reuses_ast_and_compiler_facts_by_source_identity() {
    let source = "export const answer = 42;";
    let input = SourceFile {
        path: "src/a.ts".into(),
        source: source.into(),
        compiler_options: CompilerOptions::default(),
    };
    let mut compiler = CountingCompiler(0);
    let mut types = Types {
        project_id: "project".into(),
        generation: 1,
        source: SourceDigest {
            path: "src/a.ts".into(),
            sha256: typefacts::SourceHash::of(source),
        },
    };
    let mut cache = FactsCache::default();
    let projects = [1, 2].map(|generation| {
        build_project_cached(
            dialect::default_dialect(),
            "project",
            generation,
            vec![input.clone()],
            &mut compiler,
            &mut types,
            &mut cache,
        )
        .unwrap()
    });
    assert_eq!(compiler.0, 1);
    assert!(Arc::ptr_eq(
        &projects[0].files[0].ast,
        &projects[1].files[0].ast
    ));
    assert!(Arc::ptr_eq(
        &projects[0].files[0].compiler,
        &projects[1].files[0].compiler
    ));
    assert_eq!(
        cache.stats(),
        CacheStats {
            ast_entries: 1,
            compiler_entries: 1
        }
    );
}

/// Refuses `refused.tsx` the way a compile error does, fails `broken.tsx` the
/// way an untrusted trace does, and answers every other file.
struct RefusingCompiler;
impl CompilerFactsProvider for RefusingCompiler {
    fn analyze(
        &mut self,
        request: &AnalysisRequest,
    ) -> Result<ExecutionMap, CompilerProviderError> {
        match request.path.as_str() {
            "src/refused.tsx" => Err(CompilerProviderError::Refused(format!(
                "{}: The HTML provided is malformed (1:20)",
                request.path
            ))),
            "src/broken.tsx" => Err(CompilerProviderError::Native(
                "semantic identity disagrees with the pinned producer".into(),
            )),
            _ => Compiler.analyze(request),
        }
    }
}

/// ADR 0360: a refusal withholds that file's compiler facts and keeps the
/// rest; any other compiler error still fails the build.
#[test]
fn a_refused_file_is_kept_without_compiler_facts_and_other_errors_stay_fatal() {
    let file = |path: &str, source: &str| SourceFile {
        path: path.into(),
        source: source.into(),
        compiler_options: CompilerOptions::default(),
    };
    let types = |paths: &[(&str, &str)]| Types {
        project_id: "project".into(),
        generation: 1,
        source: SourceDigest {
            path: paths[0].0.into(),
            sha256: typefacts::SourceHash::of(paths[0].1),
        },
    };
    let refused = ("src/refused.tsx", "export const view = <p><p /></p>;");
    let facts = build_project_cached(
        dialect::default_dialect(),
        "project",
        1,
        vec![file(refused.0, refused.1)],
        &mut RefusingCompiler,
        &mut types(&[refused]),
        &mut FactsCache::default(),
    )
    .expect("a refusal is not fatal");
    let execution = &facts.files[0].compiler;
    assert!(execution.withheld());
    assert_eq!(
        execution.refusal(),
        Some("src/refused.tsx: The HTML provided is malformed (1:20)")
    );
    assert!(!execution.semantic_model.source_operations_complete);
    assert!(!facts.files[0].ast.jsx_elements.is_empty());

    let broken = ("src/broken.tsx", "export const view = <p />;");
    let error = build_project_cached(
        dialect::default_dialect(),
        "project",
        1,
        vec![file(broken.0, broken.1)],
        &mut RefusingCompiler,
        &mut types(&[broken]),
        &mut FactsCache::default(),
    )
    .expect_err("an untrusted trace stays fatal");
    assert!(
        matches!(error, BackendError::NativeCompiler(_)),
        "unexpected error variant: {error:?}"
    );
}
