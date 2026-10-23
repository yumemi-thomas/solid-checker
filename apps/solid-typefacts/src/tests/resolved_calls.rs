//! Ports the resolved-call cases of the Go compiler-backed tests:
//! `tsgo/compiler_facts_test.go` (call result domain, validity, argument
//! mappings, owners, targets, assertion and never signatures, parameter object
//! shapes), `tsgo/library_binding_test.go`, `tsgo/semantic_runs_test.go`
//! (alignment and retention evidence), and the `resolvedCall` cases of
//! `hardening_integration_test.go` and `typefacts_integration_test.go`.
//!
//! One Go assertion is not portable as written:
//! `TestResolvedCallsReuseCompilerIdenticalTypeDescriptors` compares the
//! *addresses* of two `*TypeDescriptor` values. A Rust `ParameterFact` owns its
//! descriptor (the wire type has no sharing), so the memo's sharing is not
//! observable on a fact; the test checks the observable half, that the two
//! compiler-identical parameter types carry equal descriptors.

use super::project_support::{Scratch, change, entities, find, find_last, loc};
use crate::backend::ClosureBackend;
use crate::context::Context;
use crate::model::{SemanticDemandRun, SemanticDemandRunResult, SemanticScope, durable_symbol_id};
use std::collections::HashSet;
use typefacts::v3::EntityDemand;
use typefacts::{
    ArgumentMappingReason, ArgumentMappingStatus, CallKind, Callability, ConstructionWitness,
    EntityFact, Location, ObjectConstructionProperty, ResolvedCall, ResolvedCallValidity,
    RuntimeValueDomain,
};

const STRICT: &str =
    r#"{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}"#;
const BUNDLER: &str = r#"{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}"#;

fn call_demand(location: Location) -> EntityDemand {
    EntityDemand {
        location,
        resolved_call: true,
        ..EntityDemand::default()
    }
}

/// The resolved calls of `needles`, each located at its first (or last)
/// occurrence in `source`, demanded as one run.
fn resolved(
    scratch: &Scratch,
    file: &str,
    source: &str,
    needles: &[&str],
    last: bool,
) -> Vec<ResolvedCall> {
    let path = scratch.path(file);
    let mut project = scratch.open();
    let demands: Vec<EntityDemand> = needles
        .iter()
        .map(|needle| {
            call_demand(if last {
                find_last(&path, source, needle)
            } else {
                find(&path, source, needle)
            })
        })
        .collect();
    entities(&mut project, &path, &demands)
        .into_iter()
        .map(|entity| {
            (*entity
                .resolved_call
                .expect("a resolved call demand yields a fact"))
            .clone()
        })
        .collect()
}

fn single(file: &str, source: &str) -> Scratch {
    Scratch::new(&[("tsconfig.json", STRICT), (file, source)])
}

fn domain(callable: bool, undefined: bool, other: bool, unknown: bool) -> RuntimeValueDomain {
    RuntimeValueDomain::new(callable, undefined, other, unknown)
}

/// `TestDemandedCallResultDomainUsesExactCallSpans`.
#[test]
fn demanded_call_result_domain_uses_exact_call_spans() {
    let source = "declare function makeCount(): number;\n\
declare function makeThunk(): () => void;\n\
declare function make(): (() => void) | undefined;\n\
declare const handlers: Array<() => number>;\n\
declare const maybe: (() => void) | undefined;\n\
declare const anyFactory: any;\n\
declare function takesNumber(value: number): number;\n\
declare function onCleanup(callback: () => void): void;\n\
declare function makeNested(): (value: number) => number;\n\
declare function makeBuilder(): { build(): number };\n\
declare class Foo { bar(): number; }\n\
\n\
onCleanup(() => { return makeCount(); });\n\
const thunkResult = makeThunk();\n\
const unionResult = make();\n\
const indexedResult = handlers[0]();\n\
const optionalResult = maybe?.();\n\
const anyResult = anyFactory();\n\
const recoveryResult = takesNumber(\"wrong\");\n\
const nestedResult = makeNested()(2);\n\
const builderResult = makeBuilder().build();\n\
const newResult = new Foo().bar();\n";
    let scratch = single("calls.ts", source);
    let path = scratch.path("calls.ts");
    let mut project = scratch.open();
    let open = domain(true, true, true, true);
    let want = [
        ("makeCount()", domain(false, false, true, false)),
        ("makeThunk()", domain(true, false, false, false)),
        ("make()", domain(true, true, false, false)),
        ("handlers[0]()", domain(false, false, true, false)),
        ("maybe?.()", domain(false, true, false, false)),
        ("anyFactory()", open),
        ("takesNumber(\"wrong\")", open),
        ("makeNested()(2)", domain(false, false, true, false)),
        ("makeBuilder().build()", domain(false, false, true, false)),
        ("new Foo().bar()", domain(false, false, true, false)),
    ];
    let demands: Vec<EntityDemand> = want
        .iter()
        .map(|(name, _)| EntityDemand {
            location: find_last(&path, source, name),
            call_result_domain: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    for ((name, want), fact) in want.iter().zip(&facts) {
        assert_eq!(fact.call_result_domain, Some(*want), "{name}");
    }

    let call = find_last(&path, source, "makeCount()");
    let (call_start, call_end) = (call.start_byte as usize, call.end_byte as usize);
    let identifier = find(&path, source, "thunkResult").start_byte as usize;
    let absence: Vec<EntityDemand> = [
        (call_start, call_start + "makeCount".len()),
        (call_start, call_end + 1),
        (call_start + 1, call_end),
        (identifier, identifier + "thunkResult".len()),
    ]
    .iter()
    .map(|(start, end)| EntityDemand {
        location: loc(&path, *start, *end),
        call_result_domain: true,
        ..EntityDemand::default()
    })
    .collect();
    for (index, fact) in entities(&mut project, &path, &absence).iter().enumerate() {
        assert!(
            fact.call_result_domain.is_none(),
            "absence demand {index} unexpectedly returned {:?}",
            fact.call_result_domain
        );
    }

    // Keep the entity span identical while pinning the legacy domain to the
    // callee query span. The new field stays tied to the full exact call span
    // and describes the produced number instead.
    let callee = loc(&path, call_start, call_start + "makeCount".len());
    let distinction = [
        EntityDemand {
            location: loc(&path, call_start, call_end),
            query_location: Some(callee),
            runtime_value_domain: true,
            ..EntityDemand::default()
        },
        EntityDemand {
            location: loc(&path, call_start, call_end),
            call_result_domain: true,
            ..EntityDemand::default()
        },
    ];
    let facts = entities(&mut project, &path, &distinction);
    let callee_domain = facts[0].runtime_value_domain.expect("callee domain");
    assert!(
        callee_domain.may_be_callable() && !callee_domain.may_be_other(),
        "callee runtime domain = {callee_domain:?}, want callable only"
    );
    let result = facts[1].call_result_domain.expect("call-result domain");
    assert!(
        result.may_be_other() && !result.may_be_callable(),
        "call-result runtime domain = {result:?}, want other only"
    );
}

/// `TestResolvedCallDistinguishesValidRecoveryAndUnresolved`.
#[test]
fn resolved_call_distinguishes_valid_recovery_and_unresolved() {
    let source = "function takesNumber(value: number): string { return String(value); }\n\
const valid = takesNumber(1);\n\
const recovery = takesNumber(\"wrong\");\n\
const unresolved = takesNumber;\n";
    let scratch = single("calls.ts", source);
    let calls = resolved(
        &scratch,
        "calls.ts",
        source,
        &[
            "takesNumber(1)",
            "takesNumber(\"wrong\")",
            "unresolved = takesNumber",
        ],
        false,
    );
    let want = [
        ResolvedCallValidity::Valid,
        ResolvedCallValidity::Recovery,
        ResolvedCallValidity::Unresolved,
    ];
    for (index, (call, validity)) in calls.iter().zip(want).enumerate() {
        assert_eq!(call.validity, validity, "entity {index} validity");
    }
    assert!(
        calls[0].arguments.len() == 1
            && calls[0].arguments[0].status == ArgumentMappingStatus::Resolved,
        "valid call mappings = {:?}",
        calls[0].arguments
    );
    assert!(
        calls[1].arguments.len() == 1
            && calls[1].arguments[0].status == ArgumentMappingStatus::Unresolved
            && calls[1].arguments[0].unresolved == Some(ArgumentMappingReason::RecoverySignature),
        "recovery call mappings = {:?}",
        calls[1].arguments
    );
    assert!(
        calls[2].arguments.is_empty(),
        "unresolved non-call mappings = {:?}, want empty",
        calls[2].arguments
    );
    assert_eq!(calls[2].kind, CallKind::Unknown);
}

/// `TestResolvedCallUsesOutermostChainedCall`.
#[test]
fn resolved_call_uses_the_outermost_chained_call() {
    let source =
        "declare function factory(): (value: number) => string;\nconst result = factory()(1);\n";
    let scratch = single("calls.ts", source);
    let calls = resolved(&scratch, "calls.ts", source, &["factory()(1)"], false);
    assert_eq!(
        calls[0].arguments.len(),
        1,
        "want the outer call's 1 argument"
    );
}

/// `TestResolvedCallValidityPreservesDiagnosticFallbacks`.
#[test]
fn resolved_call_validity_preserves_diagnostic_fallbacks() {
    let source = "function takesNumber(value: number): string { return String(value); }\n\
function generic<T extends string>(value: T): T { return value; }\n\
declare const maybe: ((value: number) => void) | undefined;\n\
declare const either: ((value: string) => void) | ((value: number) => void);\n\
const notCallable = 1;\n\
takesNumber(1);\n\
takesNumber(\"wrong\");\n\
takesNumber();\n\
generic<number>(1);\n\
maybe(1);\n\
either(true);\n\
notCallable();\n";
    let scratch = single("calls.ts", source);
    let path = scratch.path("calls.ts");
    let mut project = scratch.open();
    use ResolvedCallValidity::{Recovery, Valid};
    let cases = [
        ("takesNumber(1);", Valid),
        ("takesNumber(\"wrong\")", Recovery),
        ("takesNumber();", Recovery),
        ("generic<number>(1)", Recovery),
        ("maybe(1)", Recovery),
        ("either(true)", Recovery),
        ("notCallable()", Recovery),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(needle, _)| {
            let start = source.rfind(needle).expect(needle);
            let span = needle.trim_end_matches(';');
            EntityDemand {
                location: loc(&path, start, start + span.len()),
                resolved_call: true,
                call_result_domain: true,
                ..EntityDemand::default()
            }
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    for (index, ((needle, want), fact)) in cases.iter().zip(&facts).enumerate() {
        let call = fact.resolved_call.as_ref().expect(needle);
        assert_eq!(call.validity, *want, "{needle}");
        let domain = fact.call_result_domain.expect(needle);
        if index == 0 {
            assert!(
                domain.may_be_other() && !domain.unknown(),
                "{needle} call-result domain = {domain:?}, want known non-function"
            );
        } else {
            assert!(
                domain.unknown(),
                "{needle} call-result domain = {domain:?}, want unknown for recovery"
            );
        }
    }
}

/// `TestResolvedCallValidatesTableShapedSyntheticWitnesses`: a consumer can
/// validate a candidate runtime witness by adding one synthetic call and
/// demanding the existing resolvedCall fact, exercising contextual generic
/// inference.
#[test]
fn resolved_call_validates_table_shaped_synthetic_witnesses() {
    let source = "type RowData = unknown;\n\
type FeatureMap = Record<string, { createTable(table: unknown): void }>;\n\
type ValidateFeatureSlots<TFeatures extends FeatureMap> = \"missing\" extends keyof TFeatures ? { missing: never } : {};\n\
interface TableOptions<TFeatures extends FeatureMap, TData extends RowData> {\n\
\tfeatures: TFeatures & ValidateFeatureSlots<TFeatures>;\n\
\tdata: TData[];\n\
\tcolumns: Array<{ accessorKey: keyof TData }>;\n\
}\n\
declare function createTable<TFeatures extends FeatureMap, TData extends RowData>(options: TableOptions<TFeatures, TData>): unknown;\n\
declare const stockFeatures: {\n\
\tcore: { createTable(table: unknown): void };\n\
};\n\
createTable({ features: stockFeatures, data: [], columns: [] });\n\
createTable({ features: { core: {} }, data: [], columns: [] });\n\
createTable({ features: stockFeatures, data: {} });\n\
createTable(null as never);\n";
    let scratch = single("witness.ts", source);
    let path = scratch.path("witness.ts");
    let mut project = scratch.open();
    let cases = [
        (
            "createTable({ features: stockFeatures, data: [], columns: [] })",
            ResolvedCallValidity::Valid,
        ),
        (
            "createTable({ features: { core: {} }, data: [], columns: [] })",
            ResolvedCallValidity::Recovery,
        ),
        (
            "createTable({ features: stockFeatures, data: {} })",
            ResolvedCallValidity::Recovery,
        ),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(needle, _)| call_demand(find(&path, source, needle)))
        .collect();
    for ((needle, want), fact) in cases.iter().zip(entities(&mut project, &path, &demands)) {
        let call = fact.resolved_call.expect(needle);
        assert_eq!(call.validity, *want, "{needle}");
        if call.validity == ResolvedCallValidity::Valid {
            assert!(
                call.arguments[0]
                    .parameter
                    .as_ref()
                    .is_some_and(|parameter| parameter.object_shape.is_none()),
                "{needle} returned an undemanded parameter object shape"
            );
        }
    }
    let shape = EntityDemand {
        location: find(&path, source, "createTable(null as never)"),
        resolved_call: true,
        parameter_object_shape: true,
        ..EntityDemand::default()
    };
    let facts = entities(&mut project, &path, &[shape]);
    let parameter = facts[0].resolved_call.as_ref().unwrap().arguments[0]
        .parameter
        .as_ref()
        .expect("TableOptions parameter");
    let shape = parameter
        .object_shape
        .as_ref()
        .expect("TableOptions parameter object shape");
    let want = [
        ("columns", ConstructionWitness::EmptyArray),
        ("data", ConstructionWitness::EmptyArray),
        ("features", ConstructionWitness::EmptyObject),
    ]
    .map(|(name, witness)| ObjectConstructionProperty {
        name: name.into(),
        witness,
    });
    assert_eq!(&*shape.required_properties, &want[..]);
}

fn shape_properties(
    file: &str,
    source: &str,
    needle: &str,
) -> Option<Vec<(String, ConstructionWitness)>> {
    let scratch = single(file, source);
    let path = scratch.path(file);
    let mut project = scratch.open();
    let demand = EntityDemand {
        location: find(&path, source, needle),
        resolved_call: true,
        parameter_object_shape: true,
        ..EntityDemand::default()
    };
    let facts = entities(&mut project, &path, &[demand]);
    let parameter = facts[0].resolved_call.as_ref().unwrap().arguments[0]
        .parameter
        .clone()
        .expect("a resolved parameter");
    parameter.object_shape.map(|shape| {
        shape
            .required_properties
            .iter()
            .map(|property| (property.name.to_string(), property.witness))
            .collect()
    })
}

/// `TestParameterObjectShapeLeavesUnprovenWitnessesUnknown`.
#[test]
fn parameter_object_shape_leaves_unproven_witnesses_unknown() {
    let source = "declare function unsafe(options: {\n\
\tcallback: () => void;\n\
\ttuple: [string];\n\
\tprimitive: string;\n\
\toptional?: number;\n\
}): void;\n\
unsafe(null as never);\n";
    let properties = shape_properties("witness.ts", source, "unsafe(null as never)").unwrap();
    let unknown = ConstructionWitness::Unknown;
    assert_eq!(
        properties,
        vec![
            ("callback".to_owned(), unknown),
            ("primitive".to_owned(), unknown),
            ("tuple".to_owned(), unknown),
        ]
    );
}

/// `TestParameterObjectShapeRejectsUnconstrainedTypeParameter`.
#[test]
fn parameter_object_shape_rejects_an_unconstrained_type_parameter() {
    let source = "declare function identity<T>(value: T): T;\nidentity(null as never);\n";
    assert_eq!(
        shape_properties("generic.ts", source, "identity(null as never)"),
        None,
        "unconstrained generic parameter object shape"
    );
}

/// The shape's required-property names use the wire spelling of an internal
/// name (Go: `wireSymbolName`, the fix of 692fc7747) without the checker's
/// unique-symbol counter (ADR 0367), and sort after every ordinary name as
/// Go's `0xFE` marker byte does.
#[test]
fn parameter_object_shape_names_use_the_wire_spelling_of_internal_names() {
    let source = "declare function consume(options: {\n\
\tzeta: string;\n\
\t[Symbol.iterator]: () => Iterator<string>;\n\
\talpha: string;\n\
\t\u{e9}clair: string;\n\
}): void;\n\
consume(null as never);\n";
    let properties = shape_properties("symbols.ts", source, "consume(null as never)").unwrap();
    let names: Vec<&str> = properties.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names.len(), 4, "{names:?}");
    assert_eq!(&names[..3], ["alpha", "zeta", "\u{e9}clair"], "{names:?}");
    assert_eq!(names[3], "__@iterator", "well-known symbol property");
}

/// `TestResolvedCallIdentifiesSelectedOverloadAndMapsArguments`.
#[test]
fn resolved_call_identifies_the_selected_overload_and_maps_arguments() {
    let source = "function select(value: string, callback: (value: string) => void): void;\n\
function select(value: number, callback: (value: number) => number): number;\n\
function select(value: string | number, callback: (value: never) => unknown) {\n\
\treturn callback(value as never);\n\
}\n\
const selected = select(1, value => value);\n";
    let scratch = single("calls.ts", source);
    let calls = resolved(
        &scratch,
        "calls.ts",
        source,
        &["select(1, value => value)"],
        true,
    );
    let call = &calls[0];
    assert_eq!(call.kind, CallKind::Call);
    assert_eq!(call.validity, ResolvedCallValidity::Valid);
    let declaration = call.declaration.as_ref().expect("selected overload");
    let newline = source.find('\n').unwrap();
    let second_overload = source[newline + 1..].find("select").unwrap() + newline + 1;
    assert_eq!(
        declaration.location.start_byte as usize, second_overload,
        "selected declaration starts at the second overload"
    );
    assert!(
        !declaration.symbol.is_empty()
            && &*declaration.name == "select"
            && &*declaration.kind == "FunctionDeclaration",
        "selected declaration identity = {declaration:?}"
    );
    assert_eq!(call.arguments.len(), 2);
    for (index, mapping) in call.arguments.iter().enumerate() {
        let parameter = mapping.parameter.as_ref().expect("a parameter");
        assert!(
            mapping.argument_index == index as u64
                && mapping.status == ArgumentMappingStatus::Resolved
                && parameter.index == index as u64,
            "argument {index} mapping = {mapping:?}"
        );
        assert!(
            !parameter.symbol.is_empty()
                && parameter.declaration.is_some()
                && parameter.type_descriptor.is_some(),
            "argument {index} parameter facts = {parameter:?}"
        );
    }
    let callability = |index: usize| {
        call.arguments[index]
            .parameter
            .as_ref()
            .unwrap()
            .callability
    };
    assert_eq!(callability(0), Callability::NonCallable, "value parameter");
    assert_eq!(callability(1), Callability::Callable, "callback parameter");
}

/// `TestResolvedCallOwnerIdentityDistinguishesSameNamedMethods`.
#[test]
fn resolved_call_owner_identity_distinguishes_same_named_methods() {
    let source = "interface CustomStorage { getItem(key: string): string }\n\
interface CustomEventTarget { removeEventListener(type: string, listener: () => void): void }\n\
interface CustomArray { push(value: number): number }\n\
interface CustomFunction { bind(thisArg: unknown): void }\n\
declare const customStorage: CustomStorage;\n\
declare const customTarget: CustomEventTarget;\n\
declare const customArray: CustomArray;\n\
declare const customFunction: CustomFunction;\n\
declare const storage: Storage;\n\
declare const target: EventTarget;\n\
declare const array: number[];\n\
declare const fn: Function;\n\
storage.getItem(\"key\");\n\
customStorage.getItem(\"key\");\n\
target.removeEventListener(\"event\", () => {});\n\
customTarget.removeEventListener(\"event\", () => {});\n\
array.push(1);\n\
customArray.push(1);\n\
fn.bind(undefined);\n\
customFunction.bind(undefined);\n";
    let scratch = single("owners.ts", source);
    let calls: [&str; 8] = [
        "storage.getItem(\"key\")",
        "customStorage.getItem(\"key\")",
        "target.removeEventListener(\"event\", () => {})",
        "customTarget.removeEventListener(\"event\", () => {})",
        "array.push(1)",
        "customArray.push(1)",
        "fn.bind(undefined)",
        "customFunction.bind(undefined)",
    ];
    let facts = resolved(&scratch, "owners.ts", source, &calls, false);
    let want = [
        "Storage.getItem",
        "CustomStorage.getItem",
        "EventTarget.removeEventListener",
        "CustomEventTarget.removeEventListener",
        "Array.push",
        "CustomArray.push",
        "Function.bind",
        "CustomFunction.bind",
    ];
    for (index, want) in want.iter().enumerate() {
        let declaration = facts[index]
            .declaration
            .as_ref()
            .unwrap_or_else(|| panic!("{} declaration is missing", calls[index]));
        assert_eq!(
            &*declaration.qualified_name, *want,
            "{} qualified identity",
            calls[index]
        );
        assert_eq!(
            declaration.standard_library,
            index % 2 == 0,
            "{} standardLibrary",
            calls[index]
        );
        if index % 2 == 0 {
            let next = facts[index + 1].declaration.as_ref().unwrap();
            assert_ne!(
                declaration.symbol,
                next.symbol,
                "{} and {} share declaration identity",
                calls[index],
                calls[index + 1]
            );
        }
    }
}

/// `TestResolvedConstructionMapsRestArguments`.
#[test]
fn resolved_construction_maps_rest_arguments() {
    let source = "class Box {\n\
\tconstructor(callback: (value: string) => void, ...labels: string[]) {}\n\
}\n\
const box = new Box(value => {}, \"first\", \"second\");\n";
    let scratch = single("construct.ts", source);
    let calls = resolved(
        &scratch,
        "construct.ts",
        source,
        &["new Box(value => {}, \"first\", \"second\")"],
        false,
    );
    let call = &calls[0];
    assert_eq!(call.validity, ResolvedCallValidity::Valid);
    assert_eq!(call.kind, CallKind::Construct);
    assert_eq!(
        call.declaration.as_ref().map(|d| &*d.qualified_name),
        Some("Box.constructor")
    );
    assert_eq!(call.arguments.len(), 3);
    let callback = call.arguments[0].parameter.as_ref().expect("callback");
    assert!(
        callback.callability == Callability::Callable && !callback.rest,
        "callback mapping = {:?}",
        call.arguments[0]
    );
    for index in [1, 2] {
        let parameter = call.arguments[index].parameter.as_ref().expect("rest");
        assert!(
            parameter.index == 1 && parameter.rest,
            "rest argument {index} mapping = {:?}",
            call.arguments[index]
        );
    }
}

/// `TestArgumentMappingsUseGenericSubstitutionAndRejectAmbiguousSpread`.
#[test]
fn argument_mappings_use_generic_substitution_and_reject_ambiguous_spread() {
    let source = "function generic<T>(value: T, callback?: (value: T) => T): T {\n\
\treturn callback ? callback(value) : value;\n\
}\n\
function pair(first: number, second: string): void {}\n\
const pairArguments: [number, string] = [1, \"two\"];\n\
generic(1, value => value);\n\
pair(...pairArguments);\n";
    let scratch = single("mapping.ts", source);
    let calls = resolved(
        &scratch,
        "mapping.ts",
        source,
        &["generic(1, value => value)", "pair(...pairArguments)"],
        true,
    );
    let generic = &calls[0];
    assert_eq!(generic.arguments.len(), 2);
    let value = generic.arguments[0].parameter.as_ref().unwrap();
    assert_eq!(
        value.type_descriptor.as_ref().map(|d| &*d.text),
        Some("number"),
        "instantiated value parameter"
    );
    let callback = generic.arguments[1].parameter.as_ref().unwrap();
    assert!(callback.optional, "callback is optional");
    assert_eq!(callback.callability, Callability::Mixed);
    assert_eq!(
        callback.type_descriptor.as_ref().map(|d| &*d.text),
        Some("((value: number) => number) | undefined")
    );
    let spread = &calls[1];
    assert_eq!(spread.arguments.len(), 1);
    let mapping = &spread.arguments[0];
    assert!(
        mapping.status == ArgumentMappingStatus::Unresolved
            && mapping.unresolved == Some(ArgumentMappingReason::SpreadArgument)
            && mapping.parameter.is_none(),
        "spread mapping = {mapping:?}"
    );
}

/// `TestResolvedCallsReuseCompilerIdenticalTypeDescriptors` (see the module
/// note: equal descriptors, not shared addresses).
#[test]
fn resolved_calls_describe_compiler_identical_types_identically() {
    let source = "function first(value: number): number { return value; }\n\
function second(value: number): number { return value; }\n\
first(1);\n\
second(2);\n";
    let scratch = single("descriptor-cache.ts", source);
    let calls = resolved(
        &scratch,
        "descriptor-cache.ts",
        source,
        &["first(1)", "second(2)"],
        true,
    );
    let descriptor = |call: &ResolvedCall| {
        call.arguments[0]
            .parameter
            .as_ref()
            .and_then(|parameter| parameter.type_descriptor.clone())
    };
    let (first, second) = (descriptor(&calls[0]), descriptor(&calls[1]));
    assert!(
        first.is_some() && first == second,
        "{first:?} vs {second:?}"
    );
}

/// `TestResolvedCallHandlesCallConstructAndIntersectionSignatures`.
#[test]
fn resolved_call_handles_call_construct_and_intersection_signatures() {
    let source = "interface Callable {\n\t(callback: () => void): void;\n}\n\
interface Constructable {\n\tnew (callback: () => void): object;\n}\n\
type Intersected = {\n\t(value: string): string;\n} & {\n\t(value: number): number;\n};\n\
declare const callable: Callable;\n\
declare const constructable: Constructable;\n\
declare const intersected: Intersected;\n\
callable(() => {});\n\
new constructable(() => {});\n\
intersected(1);\n";
    let scratch = single("signatures.ts", source);
    let calls = [
        "callable(() => {})",
        "new constructable(() => {})",
        "intersected(1)",
    ];
    let facts = resolved(&scratch, "signatures.ts", source, &calls, true);
    let want = [
        (CallKind::Call, "Callable.call", "CallSignature"),
        (
            CallKind::Construct,
            "Constructable.construct",
            "ConstructSignature",
        ),
        (CallKind::Call, "Intersected.call", "CallSignature"),
    ];
    for (index, (kind, qualified, declaration_kind)) in want.iter().enumerate() {
        let call = &facts[index];
        let declaration = call.declaration.as_ref();
        assert!(
            call.validity == ResolvedCallValidity::Valid
                && call.kind == *kind
                && declaration.is_some_and(
                    |d| &*d.qualified_name == *qualified && &*d.kind == *declaration_kind
                ),
            "{} fact = {call:?}, want {kind:?} {qualified} {declaration_kind}",
            calls[index]
        );
        assert!(
            call.arguments.len() == 1
                && call.arguments[0].status == ArgumentMappingStatus::Resolved,
            "{} mappings = {:?}",
            calls[index],
            call.arguments
        );
    }
}

/// `TestResolvedUnionCallDerivesExhaustiveTargetCandidates`.
#[test]
fn resolved_union_call_derives_exhaustive_target_candidates() {
    // The two implementations carry distinguishable literal return types:
    // structurally identical function types are subtype-reduced out of a union
    // by the compiler itself, which leaves the single selected declaration
    // fact rather than a candidate set.
    let impls = "export function implA(value: string): \"a\" {\n\treturn \"a\";\n}\n\
export function implB(value: string): \"b\" {\n\treturn \"b\";\n}\n";
    let source = "import { implA, implB } from \"./impls\";\n\
declare const cond: boolean;\n\
const dispatch = cond ? implA : implB;\n\
export const direct = dispatch(\"value\");\n\
declare const pair: [typeof implA, typeof implB];\n\
declare const index: number;\n\
export const computed = pair[index](\"value\");\n\
class Left {\n\tread(): string {\n\t\treturn \"left\";\n\t}\n}\n\
class Right {\n\tread(): number {\n\t\treturn 2;\n\t}\n}\n\
declare const union: Left | Right;\n\
export const method = union.read();\n\
interface Shape {\n\t(value: string): string;\n}\n\
declare const shaped: typeof implA | Shape;\n\
export const structural = shaped(\"value\");\n\
declare const generic: typeof implA | (<T>(value: T) => T);\n\
export const open = generic(\"value\");\n\
export const broken = dispatch(\"value\", \"extra\");\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            STRICT
                .replace(
                    "\"module\":\"esnext\"",
                    "\"module\":\"esnext\",\"moduleResolution\":\"bundler\"",
                )
                .as_str(),
        ),
        ("impls.ts", impls),
        ("union-targets.ts", source),
    ]);
    let (impls_path, source_path) = (scratch.path("impls.ts"), scratch.path("union-targets.ts"));
    let calls = [
        "dispatch(\"value\")",
        "pair[index](\"value\")",
        "union.read()",
        "shaped(\"value\")",
        "generic(\"value\")",
        "dispatch(\"value\", \"extra\")",
    ];
    let facts = resolved(&scratch, "union-targets.ts", source, &calls, false);
    let assert_candidates = |index: usize, kind: &str, qualified: &[&str], path: &str| {
        let call = &facts[index];
        assert_eq!(
            call.validity,
            ResolvedCallValidity::Valid,
            "{}",
            calls[index]
        );
        assert!(
            call.declaration.is_none(),
            "{} guessed one declaration {:?}",
            calls[index],
            call.declaration
        );
        let targets = call.targets.as_ref().expect("an exhaustive target set");
        assert!(targets.exhaustive, "{} targets = {targets:?}", calls[index]);
        assert_eq!(
            targets.candidates.len(),
            qualified.len(),
            "{}",
            calls[index]
        );
        let mut seen = HashSet::new();
        for (candidate_index, candidate) in targets.candidates.iter().enumerate() {
            assert!(
                !candidate.symbol.is_empty() && seen.insert(candidate.symbol.clone()),
                "{} candidate {candidate_index} symbol = {:?}, want distinct non-empty identities",
                calls[index],
                candidate.symbol
            );
            assert_eq!(&*candidate.kind, kind, "{} candidate kind", calls[index]);
            assert_eq!(
                &*candidate.qualified_name, qualified[candidate_index],
                "{} candidate {candidate_index} qualified name",
                calls[index]
            );
            assert_eq!(
                &*candidate.location.path, path,
                "{} candidate path",
                calls[index]
            );
            if candidate_index > 0 {
                let previous = &targets.candidates[candidate_index - 1].location;
                assert!(
                    !(previous.path > candidate.location.path
                        || (previous.path == candidate.location.path
                            && previous.start_byte > candidate.location.start_byte)),
                    "{} candidates are not deterministically ordered: {:?}",
                    calls[index],
                    targets.candidates
                );
            }
        }
    };
    // A conditional union of two exact cross-file function declarations is a
    // proven two-candidate dispatch, whether the callee is an identifier or a
    // dynamically indexed tuple slot.
    assert_candidates(0, "FunctionDeclaration", &["implA", "implB"], &impls_path);
    assert_candidates(1, "FunctionDeclaration", &["implA", "implB"], &impls_path);
    // Same-named methods keep their own class identities.
    assert_candidates(
        2,
        "MethodDeclaration",
        &["Left.read", "Right.read"],
        &source_path,
    );
    for (index, label) in [
        (3, "structural interface constituent"),
        (4, "generic constituent"),
        (5, "recovery call"),
    ] {
        assert!(
            facts[index].targets.is_none(),
            "{label} emitted target candidates {:?}, want none",
            facts[index].targets
        );
    }
    assert_eq!(
        facts[5].validity,
        ResolvedCallValidity::Recovery,
        "broken call validity"
    );
}

/// `TestResolvedOptionalCallPreservesSingleSignature`.
#[test]
fn resolved_optional_call_preserves_a_single_signature() {
    let source = "declare const text: string | undefined;\n\
text?.trim();\n\
declare const callback: ((value: number) => string) | null | undefined;\n\
callback?.(1);\n\
interface Shape { run(value: number): string }\n\
declare const object: Shape | undefined;\n\
object?.run(1);\n\
declare function left(value: number): \"left\";\n\
declare function right(value: number): \"right\";\n\
declare const union: typeof left | typeof right | undefined;\n\
union?.(1);\n\
callback?.(\"invalid\");\n\
callback(1);\n\
function generic<T extends ((value: number) => string) | undefined>(value: T) {\n\
  value?.(1);\n\
}\n";
    let scratch = single("optional.ts", source);
    let calls = [
        "text?.trim()",
        "callback?.(1)",
        "object?.run(1)",
        "union?.(1)",
        "callback?.(\"invalid\")",
        "callback(1)",
        "value?.(1)",
    ];
    let facts = resolved(&scratch, "optional.ts", source, &calls, false);
    for (index, name) in ["String.trim", "callback.call", "Shape.run"]
        .iter()
        .enumerate()
    {
        let call = &facts[index];
        assert!(
            call.validity == ResolvedCallValidity::Valid
                && call.declaration.is_some()
                && call.targets.is_none(),
            "{} lost its signature: {call:?}",
            calls[index]
        );
        let declaration = call.declaration.as_ref().unwrap();
        assert_eq!(&*declaration.qualified_name, *name, "{}", calls[index]);
        assert_eq!(
            declaration.standard_library,
            index == 0,
            "{} library identity",
            calls[index]
        );
        for mapping in call.arguments.iter() {
            assert!(
                mapping.status == ArgumentMappingStatus::Resolved && mapping.parameter.is_some(),
                "{} mapping = {mapping:?}",
                calls[index]
            );
        }
    }
    let guessed = &facts[3];
    assert!(
        guessed.declaration.is_none() && guessed.targets.is_none(),
        "{} guessed a target: {guessed:?}",
        calls[3]
    );
    let generic = &facts[6];
    assert!(
        generic
            .declaration
            .as_ref()
            .is_some_and(|d| &*d.kind == "FunctionType")
            && generic.targets.is_none(),
        "generic signature must stay structural: {generic:?}"
    );
    for index in [4, 5] {
        assert_eq!(
            facts[index].validity,
            ResolvedCallValidity::Recovery,
            "{}",
            calls[index]
        );
    }
}

/// `TestResolvedUnionCallDoesNotGuessOneConstituentDeclaration`.
#[test]
fn resolved_union_call_does_not_guess_one_constituent_declaration() {
    let source = "interface Left {\n\t(value: string): string;\n}\n\
interface Right {\n\t(value: string): number;\n}\n\
declare const union: Left | Right;\n\
const value = union(\"value\");\n";
    let scratch = single("union.ts", source);
    let calls = resolved(&scratch, "union.ts", source, &["union(\"value\")"], true);
    let call = &calls[0];
    assert_eq!(call.validity, ResolvedCallValidity::Valid);
    assert!(
        call.declaration.is_none(),
        "union call guessed declaration {:?}",
        call.declaration
    );
    assert!(
        call.arguments.len() == 1
            && call.arguments[0].status == ArgumentMappingStatus::Unresolved
            && call.arguments[0].unresolved == Some(ArgumentMappingReason::CompositeSignature),
        "union argument mappings = {:?}",
        call.arguments
    );
}

/// `TestResolvedCallRejectsStaleDeclarationNodesAfterUpdate`.
#[test]
fn resolved_call_rejects_stale_declaration_nodes_after_update() {
    let original = "export function invoke(callback: () => void) { callback(); }\n";
    let updated = format!("// shifted declaration\n{original}");
    let consumer = "import { invoke } from \"./library\";\ninvoke(() => {});\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", BUNDLER),
        ("library.ts", original),
        ("consumer.ts", consumer),
    ]);
    let (library_path, call_path) = (scratch.path("library.ts"), scratch.path("consumer.ts"));
    let mut project = scratch.open();
    let demand = call_demand(find_last(&call_path, consumer, "invoke(() => {})"));
    entities(&mut project, &call_path, std::slice::from_ref(&demand));
    project
        .update(&Context::new(), &[change(&library_path, 1, &updated)])
        .unwrap();
    let facts = entities(&mut project, &call_path, &[demand]);
    let call = facts[0].resolved_call.as_ref().expect("updated call fact");
    let declaration = call.declaration.as_ref().expect("declaration");
    let parameter = call.arguments[0]
        .parameter
        .as_ref()
        .and_then(|parameter| parameter.declaration.as_ref())
        .expect("parameter declaration");
    assert_eq!(call.arguments.len(), 1);
    assert_eq!(
        declaration.location.start_byte as usize,
        updated.find("invoke").unwrap(),
        "selected declaration must start at the current byte"
    );
    assert_eq!(
        parameter.location.start_byte as usize,
        updated.find("callback").unwrap(),
        "parameter declaration must start at the current byte"
    );
}

/// `TestResolvedCallParameterAnswersUntypedCallableForFunctionTypedParameter`.
#[test]
fn resolved_call_parameter_answers_untyped_callable_for_a_function_typed_parameter() {
    let source = "declare function register(handler: Function): void;\ndeclare const fn: () => void;\nregister(fn);\n";
    let scratch = single("parameter.ts", source);
    let calls = resolved(&scratch, "parameter.ts", source, &["register(fn)"], true);
    assert_eq!(calls[0].arguments.len(), 1);
    assert_eq!(
        calls[0].arguments[0]
            .parameter
            .as_ref()
            .map(|p| p.callability),
        Some(Callability::UntypedCallable),
        "Function-typed parameter callability"
    );
}

/// `TestResolvedCallFlagsAssertionAndNeverSignatures` (ADR 0279): from the
/// selected signature itself, not from display text.
#[test]
fn resolved_call_flags_assertion_and_never_signatures() {
    let source = "function fail(message: string): never { throw new Error(message); }\n\
type Bottom = never;\n\
function failAlias(message: string): Bottom { throw new Error(message); }\n\
function assertReady(ready: boolean): asserts ready { if (!ready) throw new Error(\"x\"); }\n\
function assertString(value: unknown): asserts value is string {}\n\
function isString(value: unknown): value is string { return typeof value === \"string\"; }\n\
function plain(value: number): void { void value; }\n\
const box = { check(ready: boolean): asserts ready {} };\n\
fail(\"a\");\n\
failAlias(\"b\");\n\
assertReady(true);\n\
assertString(\"c\");\n\
isString(1);\n\
plain(1);\n\
box.check(true);\n";
    let scratch = single("calls.ts", source);
    let cases = [
        ("fail(\"a\")", false, true),
        ("failAlias(\"b\")", false, true),
        ("assertReady(true)", true, false),
        ("assertString(\"c\")", true, false),
        ("isString(1)", false, false),
        ("plain(1)", false, false),
        ("box.check(true)", true, false),
    ];
    let needles: Vec<&str> = cases.iter().map(|(call, _, _)| *call).collect();
    let facts = resolved(&scratch, "calls.ts", source, &needles, true);
    for ((call, asserts, never), fact) in cases.iter().zip(&facts) {
        assert_eq!(
            (fact.assertion_signature, fact.never_returns),
            (*asserts, *never),
            "{call}: assertion, never"
        );
    }
}

fn library_binding_project(
    files: &[(&str, &str)],
    config: &str,
) -> (Scratch, crate::project::Project) {
    let mut all = vec![("tsconfig.json", config)];
    all.extend_from_slice(files);
    let scratch = Scratch::new(&all);
    let project = scratch.open();
    (scratch, project)
}

/// `TestStandardGlobalLibraryBindingSurvivesOnlyUnwrittenGenerations`.
#[test]
fn standard_global_library_binding_survives_only_unwritten_generations() {
    let source = "queueMicrotask(() => {});\n\
(queueMicrotask as typeof queueMicrotask)(() => {});\n\
declare const unknownScheduler: typeof queueMicrotask;\n\
unknownScheduler(() => {});\n";
    let (scratch, mut project) = library_binding_project(
        &[("calls.ts", source), ("writes.ts", "export {};\n")],
        BUNDLER,
    );
    let (calls_path, writes_path) = (scratch.path("calls.ts"), scratch.path("writes.ts"));
    let calls = [
        "queueMicrotask(() => {})",
        "(queueMicrotask as typeof queueMicrotask)(() => {})",
        "unknownScheduler(() => {})",
    ];
    let demands: Vec<EntityDemand> = calls
        .iter()
        .map(|call| call_demand(find(&calls_path, source, call)))
        .collect();
    let check = |project: &mut crate::project::Project, want: bool| {
        let facts = entities(project, &calls_path, &demands);
        assert_eq!(facts.len(), calls.len());
        for (index, fact) in facts.iter().enumerate() {
            let call = fact.resolved_call.as_ref().expect("resolved call");
            assert!(
                call.declaration
                    .as_ref()
                    .is_some_and(|declaration| declaration.standard_library),
                "selected signature must remain native for {}: {call:?}",
                calls[index]
            );
            assert_eq!(
                call.default_library_binding.is_some(),
                want && index < 2,
                "{} binding = {:?}, want {}",
                calls[index],
                call.default_library_binding,
                want && index < 2
            );
        }
    };
    check(&mut project, true);
    for (index, contents) in [
        "queueMicrotask = callback => callback();",
        "export {};",
        "globalThis.queueMicrotask = callback => callback();",
        "export {};",
    ]
    .iter()
    .enumerate()
    {
        project
            .update(
                &Context::new(),
                &[change(&writes_path, index as u64 + 1, contents)],
            )
            .unwrap();
        check(&mut project, index % 2 == 1);
    }
}

/// `TestAugmentedGlobalLibraryBindingAndRetainedWrites`.
#[test]
fn augmented_global_library_binding_and_retained_writes() {
    let source = "import { schedule } from \"./other\";\n\
setTimeout(() => {}, 1);\n\
(setTimeout as typeof setTimeout)(() => {}, 1);\n\
declare const alias: typeof setTimeout;\n\
alias(() => {}, 1);\n\
declare const holder: { schedule: typeof setTimeout };\n\
holder.schedule(() => {}, 1);\n\
schedule(() => {}, 3);\n\
function shadow(setTimeout: typeof schedule) { setTimeout(() => {}, 2); }\n";
    let blank = "export {};\n";
    let (scratch, mut project) = library_binding_project(
        &[
            ("calls.ts", source),
            (
                "overloads.d.ts",
                "declare function setTimeout<TArgs extends any[]>(callback: (...args: TArgs) => void, ms?: number, ...args: TArgs): { timer: true };",
            ),
            (
                "other.ts",
                "export function schedule(callback: () => void, ms: number) { callback(); }",
            ),
            ("writes.ts", blank),
        ],
        BUNDLER,
    );
    let (calls_path, writes_path) = (scratch.path("calls.ts"), scratch.path("writes.ts"));
    let calls = [
        "setTimeout(() => {}, 1)",
        "(setTimeout as typeof setTimeout)(() => {}, 1)",
        "alias(() => {}, 1)",
        "holder.schedule(() => {}, 1)",
        "schedule(() => {}, 3)",
        "setTimeout(() => {}, 2)",
    ];
    let demands: Vec<EntityDemand> = calls
        .iter()
        .map(|call| call_demand(find(&calls_path, source, call)))
        .collect();
    let check = |project: &mut crate::project::Project, want: bool, state: &str| {
        let facts = entities(project, &calls_path, &demands);
        for (index, fact) in facts.iter().enumerate() {
            let call = fact.resolved_call.as_ref().expect("resolved call");
            assert_eq!(
                call.validity,
                ResolvedCallValidity::Valid,
                "{}",
                calls[index]
            );
            let binding = call.default_library_binding.as_ref();
            assert_eq!(
                binding.is_some(),
                want && index < 2,
                "{} library binding = {binding:?}, want {} after {state}",
                calls[index],
                want && index < 2
            );
            if index < 2 {
                let declaration = call.declaration.as_ref().expect("selected overload");
                assert!(
                    !declaration.standard_library
                        && declaration.source_file.ends_with("overloads.d.ts"),
                    "selected overload was replaced: {call:?}"
                );
                assert!(
                    call.arguments.len() == 2 && call.arguments[0].parameter.is_some(),
                    "selected mapping lost: {call:?}"
                );
                if let Some(binding) = binding {
                    assert!(
                        binding.standard_library && &*binding.qualified_name == "setTimeout",
                        "wrong binding: {binding:?}"
                    );
                }
            }
        }
    };
    check(&mut project, true, blank);
    let writes = [
        "setTimeout = (() => ({timer: true})) as typeof setTimeout;",
        blank,
        "globalThis.setTimeout = (() => ({timer: true})) as typeof setTimeout;",
        blank,
        "window.setTimeout = (() => 1) as typeof window.setTimeout;",
        blank,
        "window[\"setTimeout\"] = (() => 1) as typeof window.setTimeout;",
        blank,
        "window[`setTimeout`] = (() => 1) as typeof window.setTimeout;",
        blank,
        "delete (window as any)[\"setTimeout\"];",
        blank,
    ];
    for (index, text) in writes.iter().enumerate() {
        project
            .update(
                &Context::new(),
                &[change(&writes_path, index as u64 + 1, text)],
            )
            .unwrap();
        check(&mut project, index % 2 == 1, text);
    }
}

fn embedded_semantic_ids(result: &SemanticDemandRunResult) -> Vec<String> {
    let mut ids: Vec<String> = result.structural.iter().map(|id| id.to_string()).collect();
    for entity in &result.entities {
        ids.push(entity.symbol.to_string());
        let Some(call) = &entity.resolved_call else {
            continue;
        };
        ids.push(call.target.to_string());
        if let Some(declaration) = &call.declaration {
            ids.push(declaration.symbol.to_string());
            ids.extend(
                declaration
                    .owners
                    .iter()
                    .map(|owner| owner.symbol.to_string()),
            );
        }
        ids.extend(
            call.arguments
                .iter()
                .filter_map(|mapping| mapping.parameter.as_ref())
                .map(|parameter| parameter.symbol.to_string()),
        );
    }
    ids
}

/// `TestSemanticDemandRunsAlignsResultsAndReportsRetentionEvidence`.
#[test]
fn semantic_demand_runs_align_results_and_report_retention_evidence() {
    let types = "export type Handler = (value: number) => string;";
    let library = "import type { Handler } from \"./types\";\n\
export function invoke(callback: Handler): string { return callback(1); }\n";
    let alpha = "import { invoke } from \"./library\";\n\
export const alpha = invoke(value => String(value));\n\
export const alphaAgain = invoke(value => String(value));\n";
    let beta = "type Calls<T extends string> = { [K in T]: (value: number) => number };\n\
declare const mapped: Calls<\"run\">;\n\
export const beta = mapped.run(1);\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", BUNDLER),
        ("types.ts", types),
        ("library.ts", library),
        ("alpha.ts", alpha),
        ("beta.ts", beta),
    ]);
    let (alpha_path, beta_path) = (scratch.path("alpha.ts"), scratch.path("beta.ts"));
    let (library_path, types_path) = (scratch.path("library.ts"), scratch.path("types.ts"));
    let mut project = scratch.open();

    let needle = "invoke(value => String(value))";
    let beta_demands = [call_demand(find(&beta_path, beta, "mapped.run(1)"))];
    let alpha_demands = [
        call_demand(find(&alpha_path, alpha, needle)),
        call_demand(find_last(&alpha_path, alpha, needle)),
    ];
    let runs = [
        SemanticDemandRun {
            path: &beta_path,
            demands: &beta_demands,
        },
        SemanticDemandRun {
            path: &alpha_path,
            demands: &alpha_demands,
        },
    ];
    let suppression = HashSet::new();
    let scope = SemanticScope {
        suppression: &suppression,
    };
    let results = project
        .semantic_demand_runs(&Context::new(), &runs, &scope)
        .unwrap();
    assert_eq!(results.len(), runs.len());
    for (run, result) in runs.iter().zip(&results) {
        assert_eq!(result.entities.len(), run.demands.len());
        assert_eq!(result.structural.len(), run.demands.len());
        for (demand, entity) in run.demands.iter().zip(&result.entities) {
            assert_eq!(entity.location, demand.location);
        }
    }

    let alpha_result = &results[1];
    assert_eq!(
        alpha_result.dependencies,
        vec![library_path.clone(), types_path.clone()],
        "alpha dependencies"
    );
    let call = alpha_result.entities[0]
        .resolved_call
        .as_ref()
        .expect("resolved call");
    let parameter = call.arguments[0].parameter.as_ref().expect("parameter");
    assert!(
        call.declaration.is_some()
            && call.arguments.len() == 1
            && parameter.declaration.is_some()
            && parameter.type_descriptor.is_some(),
        "resolved call lacks declaration, parameter, or parameter type evidence: {call:?}"
    );
    assert_eq!(
        &*call.declaration.as_ref().unwrap().location.path,
        library_path,
        "resolved declaration path"
    );
    assert_eq!(
        &*parameter.declaration.as_ref().unwrap().location.path,
        library_path,
        "parameter declaration path"
    );
    let alias_paths: Vec<&str> = parameter
        .type_descriptor
        .as_ref()
        .unwrap()
        .alias_declarations
        .iter()
        .map(|declaration| &*declaration.location.path)
        .collect();
    assert_eq!(
        alias_paths,
        [types_path.as_str()],
        "parameter type alias paths"
    );

    for (index, result) in results.iter().enumerate() {
        let ids = embedded_semantic_ids(result);
        let want_durable = ids.iter().all(|id| durable_symbol_id(id));
        assert_eq!(
            result.durable, want_durable,
            "result {index} Durable, embedded ids {ids:?}"
        );
        if index == 0 {
            assert!(
                !want_durable,
                "mapped call embedded only durable ids {ids:?}; non-durable coverage is vacuous"
            );
        } else {
            assert!(
                want_durable,
                "source-backed call embedded non-durable ids {ids:?}; durable coverage is vacuous"
            );
        }
    }
}

/// `TestProjectSupportsProjectReferencesAndMixedJavaScript`: the resolved
/// return types across a project reference and a checked JavaScript module.
#[test]
fn resolved_calls_cross_project_references_and_mixed_javascript() {
    let scratch = Scratch::new(&[
        (
            "core/tsconfig.json",
            r#"{
  "compilerOptions": {"composite": true, "strict": true, "module": "ESNext", "moduleResolution": "Bundler"},
  "include": ["source.ts", "identity.js"]
}"#,
        ),
        ("core/source.ts", "export const referenced = () => 1;\n"),
        (
            "core/identity.js",
            "\n/** @template T @param {T} value @returns {T} */\nexport function identity(value) { return value; }\n",
        ),
        (
            "app/tsconfig.json",
            r#"{
  "compilerOptions": {
    "composite": true,
    "strict": true,
    "allowJs": true,
    "checkJs": true,
    "module": "ESNext",
    "moduleResolution": "Bundler"
  },
  "references": [{"path": "../core"}],
  "include": ["app.ts"]
}"#,
        ),
        (
            "app/app.ts",
            "\nimport { referenced } from \"../core/source\";\nimport { identity } from \"../core/identity\";\nreferenced();\nidentity(\"mixed\");\n",
        ),
    ]);
    let mut project = crate::project::Project::open(
        &Context::new(),
        &scratch.path("app/tsconfig.json"),
        None,
        None,
    )
    .unwrap();
    let path = scratch.path("app/app.ts");
    let source = std::fs::read_to_string(&path).unwrap();
    for (call, want) in [
        ("referenced()", "number"),
        ("identity(\"mixed\")", "\"mixed\""),
    ] {
        let facts = entities(
            &mut project,
            &path,
            &[call_demand(find(&path, &source, call))],
        );
        let fact: &EntityFact = &facts[0];
        assert_eq!(
            &*fact.resolved_call.as_ref().unwrap().return_type_text,
            want,
            "ResolvedCall({call}) return type"
        );
    }
}

fn aliased_import() -> (std::path::PathBuf, crate::project::Project) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/aliased-import")
        .canonicalize()
        .unwrap();
    let project = crate::project::Project::open(
        &Context::new(),
        root.join("tsconfig.json").to_str().unwrap(),
        None,
        None,
    )
    .unwrap();
    (root, project)
}

/// `TestProjectResolvesCallTargetAndReturnType`.
#[test]
fn project_resolves_call_target_and_return_type() {
    let (root, mut project) = aliased_import();
    let path = root.join("use.ts").to_string_lossy().into_owned();
    let source = std::fs::read_to_string(&path).unwrap();
    let call = find(&path, &source, "localCount()");
    let mut alias = call.clone();
    alias.end_byte = alias.start_byte + "localCount".len() as u64;
    let ctx = Context::new();
    let alias_id = project.symbol_at(&ctx, &alias).unwrap();
    let original = project
        .resolve_alias(&ctx, &alias_id)
        .unwrap()
        .expect("an alias target");
    let facts = entities(&mut project, &path, &[call_demand(call)]);
    let fact = facts[0].resolved_call.as_ref().unwrap();
    assert_eq!(fact.target, original, "call target");
    assert_eq!(&*fact.return_type_text, "number", "return type");
}

/// `TestProjectUpdateRechecksChangedFileAndImporter` (the resolved-return
/// half; the affected set is covered by the update tests).
#[test]
fn project_update_rechecks_the_resolved_return_type() {
    let (root, mut project) = aliased_import();
    let source_path = root.join("source.ts").to_string_lossy().into_owned();
    project
        .update(
            &Context::new(),
            &[change(
                &source_path,
                1,
                "export const count = () => \"updated\";\n",
            )],
        )
        .unwrap();
    let path = root.join("use.ts").to_string_lossy().into_owned();
    let source = std::fs::read_to_string(&path).unwrap();
    let facts = entities(
        &mut project,
        &path,
        &[call_demand(find(&path, &source, "localCount()"))],
    );
    assert_eq!(
        &*facts[0].resolved_call.as_ref().unwrap().return_type_text,
        "string",
        "return type after update"
    );
}

/// A minimal repro per call-resolution diagnostic code (plan 1.4): a
/// TypeScript bump that renumbers or splits one of these messages would change
/// call validity silently, so each code is asserted against the compiler.
///
/// Not reproduced, so not asserted: 2635 (no source in the probed shapes
/// reached it), 2677 (a type-predicate declaration error, never inside a call
/// node), 2757 and 2810 (the latter is a JavaScript-only JSDoc hint). They stay
/// in `is_call_resolution_diagnostic` as Go has them: a code that never
/// appears only keeps a call valid, which the fail-closed rule treats as the
/// compiler's own verdict.
#[test]
fn call_resolution_diagnostic_codes_match_the_compiler() {
    let exact = r#"{"compilerOptions":{"strict":true,"exactOptionalPropertyTypes":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}"#;
    let loose = r#"{"compilerOptions":{"strict":false,"module":"esnext","target":"esnext"},"include":["*.ts"]}"#;
    let cases: [(i32, &str, &str, &str); 15] = [
        (
            2344,
            STRICT,
            "declare function f<T extends string>(a: T): void;\nf<number>(1);\n",
            "f<number>(1)",
        ),
        (
            2345,
            STRICT,
            "declare function f(a: number): void;\nf(\"x\");\n",
            "f(\"x\")",
        ),
        (
            2348,
            STRICT,
            "class A {}\ndeclare const A2: typeof A;\nA2();\n",
            "A2()",
        ),
        (2349, STRICT, "const n = 1;\nn();\n", "n()"),
        (
            2350,
            loose,
            "function f(): number { return 1; }\nnew f();\n",
            "new f()",
        ),
        (
            2379,
            exact,
            "declare function f(o: { a?: number }): void;\nf({ a: undefined });\n",
            "f({ a: undefined })",
        ),
        (
            2554,
            STRICT,
            "declare function f(a: number): void;\nf();\n",
            "f()",
        ),
        (
            2555,
            STRICT,
            "declare function f(a: number, b: number, ...r: number[]): void;\nf(1);\n",
            "f(1)",
        ),
        (
            2558,
            STRICT,
            "declare function f<T>(a: T): void;\nf<string, number>(1);\n",
            "f<string, number>(1)",
        ),
        (
            2721,
            STRICT,
            "declare const f: (() => void) | null;\nf();\n",
            "f()",
        ),
        (
            2722,
            STRICT,
            "declare const f: (() => void) | undefined;\nf();\n",
            "f()",
        ),
        (
            2723,
            STRICT,
            "declare const f: (() => void) | null | undefined;\nf();\n",
            "f()",
        ),
        (
            2769,
            STRICT,
            "declare function f(a: string): void;\ndeclare function f(a: number): void;\nf(true);\n",
            "f(true)",
        ),
        (
            2794,
            STRICT,
            "const x: Promise<number> = new Promise((resolve) => { resolve(); });\n",
            "resolve()",
        ),
        (
            6234,
            STRICT,
            "const o = { get x(): number { return 1; } };\no.x();\n",
            "o.x()",
        ),
    ];
    for (code, config, source, call) in cases {
        let scratch = Scratch::new(&[("tsconfig.json", config), ("repro.ts", source)]);
        let path = scratch.path("repro.ts");
        let mut project = scratch.open();
        // Wake the checker the way the producer would before asking.
        entities(&mut project, &path, &[]);
        let generation = project.current().unwrap();
        let program = generation.program();
        let file = program.get_source_file(&path).unwrap();
        let reported = generation.semantic_diagnostics(file);
        let start = source.find(call).unwrap() as u32;
        let end = start + call.len() as u32;
        assert!(
            reported
                .iter()
                .any(|d| d.code == code && d.pos.is_some_and(|pos| pos >= start && pos <= end)),
            "{code} not reported inside {call:?}: {reported:?}"
        );
    }
}

/// Review of ADR 0368: TypeScript reports a call whose callee is not callable
/// on the callee (TS2349). When the callee is itself a call, the diagnostic
/// spans exactly that inner call; attributing it only to the inner call left
/// the failing outer call `Valid` (returning `any`) although tsc rejects it.
#[test]
fn a_failure_reported_on_a_callee_call_marks_the_enclosing_call() {
    let source = "declare function make(): number;\n\
declare function makeFn(): (value: number) => void;\n\
make()(1);\n\
make()!(1);\n\
makeFn()(1);\n";
    let scratch = single("calls.ts", source);
    let calls = resolved(
        &scratch,
        "calls.ts",
        source,
        &["make()(1)", "make()!(1)", "makeFn()(1)"],
        false,
    );
    let validity = calls.iter().map(|call| call.validity).collect::<Vec<_>>();
    use ResolvedCallValidity::{Recovery, Valid};
    assert_eq!(validity, [Recovery, Recovery, Valid]);
    // The inner call itself is proven valid without its diagnostics.
    let inner = resolved(&scratch, "calls.ts", source, &["make()"], true);
    assert_eq!(inner[0].validity, Valid);
}

/// Review of ADR 0368, on tsrs: a package source listed after the project's
/// own root and augmenting its global interface reorders the merged
/// overloads, so the generated program (project first, package last) reports
/// TS2349 at `api.get()(count)` that the project's own program does not.
/// The call answers `Recovery` there and `Valid` in the project's program.
#[test]
fn a_package_root_after_the_project_root_can_fail_a_project_call() {
    let main = "declare global { interface API { get(): (x: () => number) => void } }\n\
declare const api: API;\n\
const count = () => 1;\n\
api.get()(count);\n\
export {};\n";
    let package = "declare global { interface API { get(): number } }\nexport const thing = 1;\n";
    let validity = |files: &str| {
        let config = format!(
            r#"{{"compilerOptions":{{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext","allowJs":true,"checkJs":false}},"files":{files}}}"#
        );
        let scratch = Scratch::new(&[
            ("tsconfig.json", &config),
            ("main.ts", main),
            ("node_modules/pkg/src/index.ts", package),
        ]);
        resolved(&scratch, "main.ts", main, &["api.get()(count)"], false)[0].validity
    };
    assert_eq!(validity(r#"["main.ts"]"#), ResolvedCallValidity::Valid);
    assert_eq!(
        validity(r#"["node_modules/pkg/src/index.ts","main.ts"]"#),
        ResolvedCallValidity::Valid
    );
    assert_eq!(
        validity(r#"["main.ts","node_modules/pkg/src/index.ts"]"#),
        ResolvedCallValidity::Recovery
    );
}
