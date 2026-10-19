use super::*;
use crate::ast::extract;

#[test]
fn startup_premise_is_confined_to_module_statement_completion() {
    for (source, expected) in [
        ("{await pending(); throw 0;}", Some(false)),
        (
            "if(!import.meta.env.SSR){await pending(); throw 0;}",
            Some(false),
        ),
        (
            "if(import.meta.env.SSR){await pending(); throw 0;}",
            Some(true),
        ),
        ("await pending();", Some(true)),
        ("throw await pending();", Some(false)),
    ] {
        let facts = extract("/project/main.ts", source).unwrap();
        let fact = facts
            .host_execution
            .iter()
            .find(|fact| fact.kind == HostExecutionSiteKind::ModuleStartupCompletion)
            .unwrap();
        assert_eq!(
            fact.predicate.evaluate_with_startup(&|_| Some(false), true),
            expected,
            "{source}: {fact:?}"
        );
    }
    for (source, expected) in [
        ("unknown.member(); target();", Some(true)),
        ("await unknown(); target();", Some(true)),
        ("class X extends unknown {} target();", Some(true)),
        ("throw 0; target();", Some(false)),
        ("unknown.member(); throw 0; target();", Some(false)),
        ("if(!import.meta.env.SSR) throw 0; target();", Some(false)),
        ("if(import.meta.env.SSR) throw 0; target();", Some(true)),
        ("if(unknown) target();", None),
        ("if(import.meta.env.SSR) target();", Some(false)),
        ("function run(){new Date(); target();} run();", None),
        (
            "async function run(){await unknown(); target();} run();",
            None,
        ),
    ] {
        let facts = extract("/project/main.ts", source).unwrap();
        let offset = u32::try_from(source.find("target()").unwrap()).unwrap();
        let fact = facts
            .host_execution
            .iter()
            .find(|fact| fact.kind == HostExecutionSiteKind::Call && fact.span.start == offset)
            .unwrap();
        let constants = |identity: &HostConstantIdentity| match identity {
            HostConstantIdentity::ViteSsr => Some(false),
            _ => None,
        };
        assert_eq!(
            fact.predicate
                .evaluate_with_startup(&constants, fact.scope.is_none()),
            expected,
            "{source}: {fact:?}"
        );
        if (source.starts_with("unknown.member") && !source.contains("throw"))
            || source.starts_with("await")
            || source.starts_with("class")
        {
            assert_eq!(
                fact.predicate.evaluate(&constants),
                None,
                "default interpretation must stay strict: {source}"
            );
        }
    }
}

#[test]
fn every_oxc_statement_variant_has_an_explicit_completion_classification() {
    let samples = [
        ("BlockStatement", "{const x=1;}", Some(true)),
        ("BreakStatement", "while(true){break;}", None),
        ("ContinueStatement", "while(true){continue;}", None),
        ("DebuggerStatement", "debugger;", None),
        ("DoWhileStatement", "do {} while(false);", None),
        ("EmptyStatement", ";", Some(true)),
        ("ExpressionStatement", "1;", Some(true)),
        ("ForInStatement", "for(const x in {}){}", None),
        ("ForOfStatement", "for(const x of []){}", None),
        ("ForStatement", "for(;;){}", None),
        ("IfStatement", "if(true){}", Some(true)),
        ("LabeledStatement", "label: {};", None),
        ("ReturnStatement", "return 0;", Some(false)),
        ("SwitchStatement", "switch(0){}", None),
        ("ThrowStatement", "throw 0;", Some(false)),
        ("TryStatement", "try{}catch{}", None),
        ("WhileStatement", "while(false){}", None),
        ("WithStatement", "with({}){}", None),
        ("VariableDeclaration", "const x=1;", Some(true)),
        ("FunctionDeclaration", "function normal(){}", Some(true)),
        ("ClassDeclaration", "class Normal{}", Some(true)),
        ("TSTypeAliasDeclaration", "type X=number;", Some(true)),
        ("TSInterfaceDeclaration", "interface X{}", Some(true)),
        ("TSEnumDeclaration", "enum X{A}", None),
        ("TSModuleDeclaration", "namespace X{}", None),
        (
            "TSGlobalDeclaration",
            "declare global {interface X{}}",
            Some(true),
        ),
        ("TSImportEqualsDeclaration", "import X=require('x');", None),
        ("ImportDeclaration", "import 'x';", Some(true)),
        ("ExportAllDeclaration", "export * from 'x';", Some(true)),
        ("ExportDefaultDeclaration", "export default 1;", Some(true)),
        ("ExportNamedDeclaration", "export const x=1;", Some(true)),
        ("TSExportAssignment", "export = 1;", None),
        (
            "TSNamespaceExportDeclaration",
            "export as namespace X;",
            Some(true),
        ),
    ];
    let mut variants = std::collections::HashSet::new();
    for (variant, source, expected) in samples {
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(
            &allocator,
            source,
            oxc_span::SourceType::ts().with_module(variant != "WithStatement"),
        )
        .with_options(oxc_parser::ParseOptions {
            allow_return_outside_function: true,
            ..Default::default()
        })
        .parse();
        assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
        let built = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
        let mut statement = &parsed.program.body[0];
        if matches!(variant, "BreakStatement" | "ContinueStatement") {
            let Statement::WhileStatement(item) = statement else {
                panic!("{source}")
            };
            let Statement::BlockStatement(block) = &item.body else {
                panic!("{source}")
            };
            statement = &block.body[0];
        }
        assert!(
            format!("{statement:?}").starts_with(variant),
            "{variant}: {statement:?}"
        );
        assert!(variants.insert(format!("{:?}", std::mem::discriminant(statement))));
        assert_eq!(
            completion(
                &built.semantic,
                &[],
                statement,
                0,
                &mut ExtractionState::new()
            )
            .evaluate(&|_| None),
            expected,
            "{variant}: {source}"
        );
    }
    assert_eq!(variants.len(), 33);
}

#[test]
fn completion_allowlist_withholds_unsupported_forms_in_every_eager_position() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../../../fixtures/reactive-ir/inferred-host-spa/completion-cases.json"
    ))
    .unwrap();
    for case in cases {
        for form in ["{};", "const result={};", "consume({});"] {
            let source = format!(
                "prefix(); {}; function consume(value:unknown){{}} {} closed();",
                case["declaration"].as_str().unwrap(),
                form.replace("{}", case["expression"].as_str().unwrap())
            );
            assert_eq!(observed(&source, "prefix()", false), Some(true), "{source}");
            assert_eq!(
                observed(&source, "closed()", false),
                case["normal"].as_bool(),
                "{source}"
            );
            let (facts, _, _, _, _) = measured(&source, false);
            assert_eq!(
                facts
                    .iter()
                    .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
                    .unwrap()
                    .predicate
                    .evaluate(&|_| None),
                case["normal"].as_bool(),
                "{source}"
            );
        }
    }
    for source in [
        "let selected=true; selected &&= closed();",
        "let selected=false; selected ||= closed();",
        "let selected=null; selected ??= closed();",
        "let selected=false; selected &&= closed();",
        "let selected=true; selected ||= closed();",
        "class A{} ++new A()[closed()];",
        "function tag(){} tag`${closed()}`;",
        "({...[closed()]});",
        "delete ({})[closed()];",
        "const value={get x(){throw 0}}; const {x}=value, y=closed();",
        "const value=[1]; const [x]=value, y=closed();",
        "using value=closed();",
    ] {
        assert_eq!(observed(source, "closed()", false), None, "{source}");
    }
}

fn measured(source: &str, exhaust: bool) -> (Vec<HostExecutionFact>, usize, usize, usize, usize) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::tsx()).parse();
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let built = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
    assert!(built.errors.is_empty(), "{:?}", built.errors);
    let mut state = ExtractionState::new();
    if exhaust {
        state.remaining = 0;
    }
    let (facts, _) = host_execution_with_state(&built.semantic, &mut state);
    (
        facts,
        built.semantic.nodes().len(),
        state.calls.visits,
        state.calls.body_visits,
        state.steps,
    )
}

#[test]
fn deep_wide_completion_visits_and_predicate_storage_are_linear() {
    // The parser/binder have their own stack needs. Count this pass's work,
    // never elapsed time, and test both pathological expression shapes.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            for deep in [
                format!("{}0", "true?0:".repeat(2000)),
                format!("{}true", "true&&".repeat(2000)),
            ] {
                let source = format!(
                    "function local(flag:boolean){{if(flag)return;throw 0}} const value={deep}; {}",
                    "local(true);".repeat(500)
                );
                let (facts, nodes, visits, bodies, steps) = measured(&source, false);
                assert_eq!(bodies, 1, "one body memo for 500 exact local calls");
                assert!(
                    visits <= 2 * nodes,
                    "{visits} completion visits / {nodes} AST nodes"
                );
                assert!(visits <= super::super::host_call_completion::FILE_NODE_BUDGET);
                assert!(steps <= EXECUTION_STEPS && steps <= 8 * nodes + 32);
                assert!(
                    facts
                        .iter()
                        .all(|fact| fact.predicate.nodes() <= PREDICATE_NODES)
                );
                assert!(
                    facts
                        .iter()
                        .map(|fact| fact.predicate.nodes())
                        .sum::<usize>()
                        <= PREDICATE_NODES * facts.len()
                );
            }
            // Wide symbolic prefixes used to clone the whole accumulated tree
            // into every following call fact; the encoded size is bounded too.
            let source = "if(import.meta.env.SSR) throw 0; ordinary();".repeat(2000);
            let (facts, nodes, visits, _, steps) = measured(&source, false);
            assert!(visits <= 2 * nodes && steps <= EXECUTION_STEPS);
            assert!(
                facts
                    .iter()
                    .all(|fact| fact.predicate.nodes() <= PREDICATE_NODES)
            );
            assert!(serde_json::to_vec(&facts).unwrap().len() <= facts.len() * 8192);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn overflow_never_creates_an_execution_witness() {
    let (facts, _, _, _, steps) = measured("if(false) closed(); ordinary();", true);
    assert_eq!(steps, 0);
    assert!(
        facts
            .iter()
            .all(|fact| fact.predicate.evaluate(&|_| Some(false)).is_none())
    );
    let branch = HostExecutionPredicate::Constant {
        identity: HostConstantIdentity::ViteSsr,
        expected: true,
    };
    let overflow = all(vec![branch; PREDICATE_NODES + 1]);
    assert_eq!(overflow, HostExecutionPredicate::Unknown);
    assert_eq!(overflow.evaluate(&|_| Some(false)), None);
    assert_eq!(
        HostExecutionPredicate::CallCompletion(Box::new(overflow)).evaluate(&|_| None),
        None
    );
    // Literal specializations must not reuse a non-return proof for a
    // different invocation environment.
    let source = "function local(flag:boolean){if(flag)throw 0} local(true); function caller(){local(false);open()}";
    assert_eq!(observed(source, "open()", false), None);
    let source = format!(
        "if({}import.meta.env.SSR{}) closed();",
        "(".repeat(100),
        " as boolean)".repeat(100)
    );
    assert_eq!(observed(&source, "closed()", false), None);
}

#[test]
fn inert_classes_complete_and_eager_or_unresolved_heritage_stays_unknown() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/inferred-host-spa/class-cases.json");
    let cases: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    for case in cases {
        let expected = case["normal"].as_bool().unwrap().then_some(true);
        let source = case["source"].as_str().unwrap();
        assert_eq!(
            observed(source, "closed()", false),
            expected,
            "{}",
            case["name"]
        );
        let (facts, _, _, _, _) = measured(source, false);
        let completion = facts
            .iter()
            .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
            .unwrap();
        assert_eq!(
            completion.predicate.evaluate(&|_| None),
            expected,
            "{}",
            case["name"]
        );
    }
}

#[test]
fn nullish_completion_composes_left_and_withholds_unknown_selection() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../../../fixtures/reactive-ir/inferred-host-spa/nullish-cases.json"
    ))
    .unwrap();
    for case in cases {
        for form in ["{};", "const value = {};", "consume({});"] {
            let source = format!(
                "{} function consume(value: unknown) {{}} prefix(); {} closed();",
                case["declaration"].as_str().unwrap(),
                form.replace("{}", case["expression"].as_str().unwrap())
            );
            let expected = case["completion"].as_bool();
            assert_eq!(observed(&source, "prefix()", false), Some(true), "{source}");
            assert_eq!(observed(&source, "closed()", false), expected, "{source}");
            let (facts, _, _, _, _) = measured(&source, false);
            let completion = facts
                .iter()
                .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
                .unwrap();
            assert_eq!(
                completion.predicate.evaluate(&|_| None),
                expected,
                "{source}"
            );
        }
    }
}

#[test]
fn unproved_constructor_completion_preserves_its_prefix_and_withholds_tails() {
    for source in [
        "class Stop {constructor(){throw 0}} prefix(); new Stop(); closed();",
        "function Stop(){throw 0} prefix(); new Stop(); closed();",
        "class Normal {} prefix(); new Normal(); closed();",
    ] {
        assert_eq!(observed(source, "prefix()", false), Some(true), "{source}");
        assert_eq!(observed(source, "closed()", false), None, "{source}");
        let (facts, _, _, _, _) = measured(source, false);
        let completion = facts
            .iter()
            .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
            .unwrap();
        assert_eq!(completion.predicate.evaluate(&|_| None), None, "{source}");
    }
}

#[test]
fn mutable_or_escaped_vite_env_never_authenticates_a_browser_continuation() {
    for source in [
        "Object.assign(import.meta.env,{SSR:true}); if(import.meta.env.SSR) throw 0; closed();",
        "const env=import.meta.env; env.SSR=true; closed();",
        "Object.assign(import.meta,{env:{SSR:true}}); closed();",
        "import.meta.env.SSR=true; closed();",
        "import.meta.env['SSR']=true; closed();",
        "delete import.meta.env.SSR; closed();",
        "Object.assign(import.meta.env,{SSR:true}); function stop(){if(import.meta.env.SSR) throw 0} stop(); closed();",
        "const env=import.meta?.env; closed();",
        "const value=import.meta.env?.unknown; closed();",
        "const env=import.meta.env; if(import.meta.env?.DEV) env.SSR=true; closed();",
    ] {
        assert_eq!(observed(source, "closed()", false), None, "{source}");
    }
}

#[test]
fn readonly_optional_vite_env_scalars_do_not_escape_the_environment() {
    for (source, expected) in [
        ("void import.meta.env?.DEV; closed();", None),
        (
            "function deferred(){if(import.meta.env?.DEV) warn()} closed();",
            Some(true),
        ),
        ("void import.meta.env?.SSR; closed();", None),
    ] {
        let allocator = oxc_allocator::Allocator::default();
        let parsed =
            oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::tsx()).parse();
        let built = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
        assert!(!opaque_host_environment(&built.semantic), "{source}");
        // Eager optional-chain completion remains unproved. Only a
        // deferred read permits the surrounding initialization to finish.
        assert_eq!(observed(source, "closed()", false), expected, "{source}");
        let (facts, _, _, _, _) = measured(source, false);
        let completion = facts
            .iter()
            .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
            .unwrap();
        assert_eq!(
            completion.predicate.evaluate(&|_| None),
            expected,
            "{source}"
        );
    }
    assert_eq!(
        observed("if(import.meta.env?.SSR) closed();", "closed()", false),
        None
    );
}

#[test]
fn exact_call_completion_instantiates_client_constants_and_preserves_normal_returns() {
    for source in [
        "function stop(){if(!import.meta.env.SSR) throw 0} stop(); closed();",
        "function stop(){if(true) throw 0} stop(); closed();",
        "function stop(){throw 0} stop(); closed();",
        "function stop(){while(true){}} stop(); closed();",
        "function stop(x:boolean){if(x) throw 0} stop(true); closed();",
        "function stop(x:unknown){void x; throw 0} stop({}); closed();",
        "const stop=(x:boolean)=>{if(!x) throw 0}; stop(false); closed();",
        "function inner(){if(!import.meta.env.SSR) throw 0} function stop(){inner()} stop(); closed();",
        "function stop(){if(import.meta.env.SSR) return; throw 0} stop(); closed();",
        "function stop(){if(true){return (()=>{throw 0})()}} stop(); closed();",
        "function stop(){if(true) throw 0} if(stop()) {} closed();",
        "function stop(){throw 0} true && stop(); closed();",
        "function stop(){throw 0} [stop(),closed()];",
        "function stop(){throw 0} ({a:stop(), b:closed()});",
        "function stop(){throw 0} outer(stop(),closed());",
        "function stop(){throw 0} const a=stop(), b=closed();",
        "function stop(){throw 0} let x=0; x+=stop(); closed();",
        "function stop(){throw 0} let obj:any={}; obj[stop()]=closed();",
        "function stop(){throw 0} stop()[closed()];",
        "function stop(){throw 0} outer(...stop()); closed();",
        "function stop(){throw 0} outer(...stop(),closed());",
        "import {isServer as ssr} from '@solidjs/web'; function stop(){if(!ssr) throw 0} stop(); closed();",
    ] {
        assert_eq!(observed(source, "closed()", false), Some(false), "{source}");
    }
    for source in [
        "function stop(){if(import.meta.env.SSR) throw 0} stop(); closed();",
        "function stop(){if(true) return; throw 0} stop(); closed();",
        "function stop(){if(flag) return; throw 0} stop(); closed();",
        "function stop(x:boolean){if(x) throw 0} stop(false); closed();",
        "function stop(x:boolean){if(x) throw 0} stop(flag); closed();",
        "async function stop(){throw 0} stop(); closed();",
        "function* stop(){throw 0} stop(); closed();",
        "unknownCall(); closed();",
        "let stop=()=>{throw 0}; stop=()=>{}; stop(); closed();",
        "function stop(){throw 0} false && stop(); closed();",
    ] {
        assert_eq!(observed(source, "closed()", false), Some(true), "{source}");
    }
    for source in [
        "function stop(){throw 0} tag`${stop()}`; closed();",
        "async function stop(){if(true) throw 0} stop(); closed();",
        "function stop(x:boolean){x=false;if(x) throw 0} stop(true); closed();",
        "function stop(x=flag){if(x) throw 0} stop(); closed();",
        "function stop(){while(true){break}} stop(); closed();",
        "function stop(){stop();throw 0} stop(); closed();",
        "function a(){b();throw 0} function b(){a()} a(); closed();",
        "function a(){b()} function b(){c()} function c(){d()} function d(){e()} function e(){throw 0} a(); closed();",
        "const obj={stop(){throw 0}}; obj.stop(); closed();",
        "declare const process:{exit():never}; process.exit(); closed();",
    ] {
        assert_eq!(observed(source, "closed()", false), None, "{source}");
    }
    let source = "function stop(){if(!import.meta.env.SSR) throw 0} stop(); closed();";
    assert_eq!(observed(source, "closed()", true), Some(true));
    assert_eq!(
        observed(
            "prefix(); function stop(){if(!import.meta.env.SSR) throw 0} stop(); closed();",
            "prefix()",
            false
        ),
        Some(true)
    );
    let ast = extract("host.ts", source).unwrap();
    let completion = ast
        .host_execution
        .iter()
        .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
        .unwrap();
    assert_eq!(completion.predicate.evaluate(&|_| Some(false)), Some(false));
    let encoded = serde_json::to_string(&ast).unwrap();
    let decoded: crate::ast::AstFacts = serde_json::from_str(&encoded).unwrap();
    assert_eq!(ast, decoded);
    let source = format!(
        "function stop(){{{} throw 0}} stop(); closed();",
        "void 0;".repeat(300)
    );
    assert_eq!(observed(&source, "closed()", false), None);
    let source = format!(
        "function stop(){{/*{}*/ throw 0}} stop(); closed();",
        "x".repeat(9000)
    );
    assert_eq!(observed(&source, "closed()", false), None);
    assert_eq!(
        HostExecutionPredicate::CallCompletion(Box::new(HostExecutionPredicate::Unknown))
            .evaluate(&|_| None),
        None
    );
}

#[test]
fn class_and_namespace_initialization_cannot_supply_a_tail_completion_premise() {
    for source in [
        "class C{static x=(()=>{throw 0})()} closed();",
        "export class C{static x=(()=>{throw 0})()} closed();",
        "export default class C{static x=(()=>{throw 0})()} closed();",
        "const C=class{static x=(()=>{throw 0})()}; closed();",
        "namespace N{throw 0} closed();",
    ] {
        assert_eq!(observed(source, "closed()", false), None, "{source}");
    }
}

#[test]
fn module_completion_is_separate_from_prefix_and_ordinary_calls_remain_feasible() {
    for (source, expected) in [
        ("closed(); throw 0;", Some(false)),
        ("closed(); await new Promise(()=>{});", None),
        ("ordinaryCall(); closed();", Some(true)),
        ("if(import.meta.env.SSR) throw 0; closed();", Some(true)),
    ] {
        let ast = extract("host.ts", source).unwrap();
        let completion = ast
            .host_execution
            .iter()
            .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
            .unwrap();
        assert_eq!(
            completion.predicate.evaluate(&|identity| match identity {
                HostConstantIdentity::ViteSsr => Some(false),
                _ => None,
            }),
            expected
        );
        assert_eq!(observed(source, "closed()", false), Some(true));
    }
    for source in [
        "((()=>{throw 0})(), closed());",
        "const a=(()=>{throw 0})(), b=closed();",
        "void ((()=>{throw 0})(), 1); closed();",
        "const stop=()=>{throw 0}; stop(); closed();",
        "function stop(){void 0; throw 0} stop(); closed();",
        "export const value=(()=>{throw 0})(); closed();",
        "export default (()=>{throw 0})(); closed();",
    ] {
        assert_eq!(observed(source, "closed()", false), Some(false));
    }
}

#[test]
fn lazy_jsx_values_and_unproved_continuations_withhold_host() {
    for (source, needle) in [
        ("const v=<Show when={false}><Dead/></Show>;", "<Dead/>"),
        ("const v=<Ignore content={closed()}/>;", "closed()"),
        (
            "const v=<Ignore>{import('./never.ts')}</Ignore>;",
            "import('./never.ts')",
        ),
        (
            "async function f(){await new Promise(()=>{}); closed();} f();",
            "closed()",
        ),
        (
            "async function f(){return (await new Promise(()=>{}), closed());} f();",
            "closed()",
        ),
        (
            "async function f(){const x=await new Promise(()=>{}), y=closed();} f();",
            "closed()",
        ),
    ] {
        assert_eq!(observed(source, needle, false), None, "{source}");
    }
    for (source, needle) in [
        ("const v=<div><Live/></div>;", "<Live/>"),
        (
            "async function f(){closed(); await new Promise(()=>{});} f();",
            "closed()",
        ),
        ("const cb=async()=>{await step()}; closed();", "closed()"),
        (
            "const v=<Show when={true}>{()=>closed()}</Show>;",
            "closed()",
        ),
    ] {
        assert_eq!(observed(source, needle, false), Some(true), "{source}");
    }
}

#[test]
fn exact_synchronous_throwing_calls_withhold_their_tail_only() {
    for source in [
        "(()=>{throw 0})(); closed();",
        "function stop(){throw 0} stop(); closed();",
        "const result=(()=>{throw 0})(); closed();",
    ] {
        assert_eq!(observed(source, "closed()", false), Some(false));
    }
    for source in [
        "(()=>{return 0})(); closed();",
        "(async()=>{throw 0})(); closed();",
        "(function*(){throw 0})(); closed();",
    ] {
        assert_eq!(observed(source, "closed()", false), Some(true));
    }
}

#[test]
fn synchronous_order_and_literal_dead_guards_are_normalized() {
    for source in [
        "const h=setTimeout(fn,0); clearTimeout(h);",
        "const h=setTimeout(fn,0); const n=1; if(!import.meta.env.SSR) clearTimeout(h);",
        "clearTimeout(setTimeout(fn,0));",
    ] {
        let facts = extract("host.ts", source).unwrap();
        assert_eq!(facts.host_call_sequences.len(), 1, "{source}");
    }
    for source in [
        "const h=setTimeout(fn,0); await step(); clearTimeout(h);",
        "const h=setTimeout(fn,0); opaque(); clearTimeout(h);",
    ] {
        let facts = extract("host.ts", source).unwrap();
        assert!(
            !facts.host_call_sequences.iter().any(|pair| {
                source[pair.first.start as usize..pair.first.end as usize].starts_with("setTimeout")
                    && source[pair.second.start as usize..pair.second.end as usize]
                        .starts_with("clearTimeout")
            }),
            "{source}"
        );
    }
    assert_eq!(
        observed("if(false) closed()", "closed()", false),
        Some(false)
    );
    assert_eq!(observed("if(true) closed()", "closed()", false), Some(true));
}

fn observed(source: &str, needle: &str, server: bool) -> Option<bool> {
    let ast = extract("host.tsx", source).unwrap_or_else(|error| panic!("{source}: {error:?}"));
    let start = u32::try_from(source.find(needle).unwrap()).unwrap();
    let site = ast
        .host_execution
        .iter()
        .find(|fact| {
            fact.span == Span::new(start, start + u32::try_from(needle.len()).unwrap())
                && !matches!(
                    fact.kind,
                    HostExecutionSiteKind::Region | HostExecutionSiteKind::ModuleCompletion
                )
        })
        .unwrap();
    site.predicate.evaluate(&|identity| match identity {
        HostConstantIdentity::ViteSsr => Some(server),
        HostConstantIdentity::Import { declaration } => ast
            .imports
            .iter()
            .flat_map(|import| &import.bindings)
            .any(|binding| {
                binding.local.span == *declaration
                    && binding.imported.as_deref() == Some("isServer")
            })
            .then_some(server),
    })
}

#[test]
fn direct_host_tests_cover_both_hosts_and_all_required_sites() {
    for (source, needle, client) in [
        ("if(import.meta.env.SSR) closed()", "closed()", false),
        ("if(!import.meta.env.SSR) closed()", "closed()", true),
        (
            "import.meta.env.SSR ? closed() : other()",
            "closed()",
            false,
        ),
        ("import.meta.env.SSR && closed()", "closed()", false),
        ("!import.meta.env.SSR && closed()", "closed()", true),
        ("import.meta.env.SSR || closed()", "closed()", true),
        ("!import.meta.env.SSR || closed()", "closed()", false),
        ("if(import.meta.env.SSR) <View/>", "<View/>", false),
        (
            "if(import.meta.env.SSR) import('./server.ts')",
            "import('./server.ts')",
            false,
        ),
    ] {
        assert_eq!(observed(source, needle, false), Some(client), "{source}");
        assert_eq!(observed(source, needle, true), Some(!client), "{source}");
    }
    assert_eq!(
        observed(
            "if(import.meta.env.SSR) object.field",
            "object.field",
            false
        ),
        Some(false)
    );
    assert_eq!(
        observed("if(import.meta.env.SSR) object.field", "object.field", true),
        None,
        "an arbitrary getter has no positive execution/completion authority"
    );
}

#[test]
fn early_guards_and_same_block_abrupt_completions() {
    for source in [
        "function f(){ if(import.meta.env.SSR) return; closed(); }",
        "function f(){ if(import.meta.env.SSR) {throw 0;} closed(); }",
        "function f(){ if(!import.meta.env.SSR) {} else {return;} closed(); }",
    ] {
        assert_eq!(observed(source, "closed()", false), Some(true));
        assert_eq!(observed(source, "closed()", true), Some(false));
    }
    for source in [
        "function f(){return; closed();}",
        "function f(){throw 0; closed();}",
    ] {
        assert_eq!(observed(source, "closed()", false), Some(false));
        assert_eq!(observed(source, "closed()", true), Some(false));
    }
    // Lexical position does not execute a hoisted function's body.
    assert_eq!(
        observed(
            "function f(){return; function g(){closed();}}",
            "closed()",
            false
        ),
        Some(true)
    );
}

#[test]
fn exact_import_identity_preserves_aliases_and_refuses_shadowing() {
    assert_eq!(
        observed(
            "import {isServer as ssr} from '@solidjs/web'; if(ssr) closed()",
            "closed()",
            false
        ),
        Some(false)
    );
    for source in [
        "const isServer = false; if(isServer) closed()",
        "import {isServer} from '@solidjs/web'; function f(isServer:boolean){if(isServer) closed()}",
        "import * as web from '@solidjs/web'; if(web.isServer) closed()",
        "if(fake.env.SSR) closed()",
        "if(import.meta.env['SSR']) closed()",
        "if(import.meta.env.SSR === true) closed()",
        "if(!!import.meta.env.SSR) closed()",
        "if(import.meta.env.SSR && flag) closed()",
        "if(import.meta.env.DEV) closed()",
        "if(import.meta.env.PROD) closed()",
    ] {
        assert_eq!(observed(source, "closed()", false), None, "{source}");
    }
}

#[test]
fn parameters_are_separate_scopes_and_never_unconditional_body_execution() {
    let source = "function f(x = closed(), {y = nested()} = {}) { body(); } f(false);";
    let ast = extract("host.ts", source).unwrap();
    let function = ast.functions.first().unwrap();
    assert_eq!(ast.host_defaults.len(), 3);
    let call = ast
        .calls
        .iter()
        .find(|call| source[call.span.start as usize..call.span.end as usize] == *"closed()")
        .unwrap();
    let default = ast
        .host_defaults
        .iter()
        .find(|default| default.span == call.span)
        .unwrap();
    assert_eq!(default.argument_index, Some(0));
    let fact = ast
        .host_execution
        .iter()
        .find(|fact| fact.span == call.span && fact.kind == HostExecutionSiteKind::Call)
        .unwrap();
    assert_eq!(fact.scope, Some(default.span));
    assert_ne!(fact.scope, Some(function.span));
    assert!(
        ast.host_defaults
            .iter()
            .any(|default| default.argument_index.is_none())
    );
    let encoded = serde_json::to_string(&ast).unwrap();
    let decoded: crate::ast::AstFacts = serde_json::from_str(&encoded).unwrap();
    assert_eq!(ast, decoded);
}

#[test]
fn unknown_constants_preserve_both_possible_arms_and_wrappers_preserve_identity() {
    assert_eq!(
        observed(
            "if((import.meta.env.SSR as boolean)) closed()",
            "closed()",
            false
        ),
        Some(false)
    );
    assert_eq!(
        observed(
            "if(!(import.meta.env.SSR satisfies boolean)) closed()",
            "closed()",
            false
        ),
        Some(true)
    );
    let ast = extract(
        "host.ts",
        "import {isServer as ssr} from '@other/web'; if(ssr) a(); else b();",
    )
    .unwrap();
    for fact in ast
        .host_execution
        .iter()
        .filter(|fact| fact.kind == HostExecutionSiteKind::Call)
    {
        assert_eq!(fact.predicate.evaluate(&|_| None), None);
        assert_ne!(fact.predicate.evaluate(&|_| None), Some(false));
    }
}

#[test]
fn dead_outer_statement_does_not_execute_nested_defaults_or_bodies() {
    let ast = extract(
        "host.ts",
        "function f(){return; function g(x = nested()){body()}} const h=(x=arrow())=>x;",
    )
    .unwrap();
    assert_eq!(ast.host_defaults.len(), 2);
    for default in &ast.host_defaults {
        let site = ast
            .host_execution
            .iter()
            .find(|fact| fact.span == default.span && fact.kind == HostExecutionSiteKind::Call)
            .unwrap();
        assert_eq!(site.scope, Some(default.span));
        assert_eq!(site.predicate.evaluate(&|_| None), Some(true));
    }
}

#[test]
fn only_void_zero_is_positive_undefined_value_evidence() {
    let ast = extract("host.ts", "f(void 0); f(undefined); function g(undefined:boolean){f(undefined)} f(false as unknown as undefined);").unwrap();
    assert_eq!(ast.host_undefined_arguments.len(), 1);
    let source = "f(void 0)";
    assert_eq!(
        ast.host_undefined_arguments[0],
        Span::new(2, u32::try_from(source.len() - 1).unwrap())
    );
}

#[test]
fn unknown_execution_is_not_dead_and_does_not_discard_following_simple_sites() {
    for source in [
        "if(flag) closed()",
        "while(flag) closed()",
        "try{closed()}catch{}",
        "obj?.method(closed())",
        "class Dormant { field = closed(); }",
        "namespace Dormant { closed(); }",
    ] {
        assert_eq!(observed(source, "closed()", false), None);
    }
    assert_eq!(
        observed("if(flag) {} closed()", "closed()", false),
        Some(true)
    );
    assert_eq!(
        observed("function f(){if(flag) return; closed()}", "closed()", false),
        None
    );
}
