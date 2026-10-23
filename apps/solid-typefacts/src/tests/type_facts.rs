//! Ports the type-fact cases of `tsgo/compiler_facts_test.go`
//! (callability, constructability, runtime and primitive value domains,
//! primitive literal candidates, the signature-less Function family),
//! `tsgo/semantic_runs_test.go` (the complete demanded expression),
//! `type_descriptor_integration_test.go` and
//! `entity_projection_integration_test.go`.
//!
//! Not ported here, with the reason: the halves of the Function-family test
//! that ask the compiler for `isTypeSubtypeOf` and `isFunctionObjectType`
//! (both `pub(crate)` in tsrs and outside the two exported wrappers the fork
//! adds). `TestDemandedCallResultDomainUsesExactCallSpans` and the resolved-call
//! parameter callability case are in `resolved_calls`.

use super::project_support::{Scratch, entities, find, find_last, loc};
use crate::closure::{DemandClosure, DemandInputGroup};
use crate::context::Context;
use typefacts::producer::EntityRows;
use typefacts::v3::EntityDemand;
use typefacts::{
    Callability, Constructability, EntityFact, PrimitiveLiteralCandidate, PrimitiveLiteralKind,
    PrimitiveValueDomain, RuntimeValueDomain,
};

const STRICT_ESNEXT: &str =
    r#"{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}"#;
const STRICT_BUNDLER: &str = r#"{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler"},"include":["*.ts"]}"#;

fn at(path: &str, source: &str, needle: &str, last: bool) -> typefacts::Location {
    if last {
        find_last(path, source, needle)
    } else {
        find(path, source, needle)
    }
}

fn rvd(callable: bool, undefined: bool, other: bool, unknown: bool) -> RuntimeValueDomain {
    RuntimeValueDomain::new(callable, undefined, other, unknown)
}

#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
fn pvd(
    string: bool,
    number: bool,
    boolean: bool,
    big_int: bool,
    symbol: bool,
    null: bool,
    undefined: bool,
    object: bool,
    finite: bool,
) -> PrimitiveValueDomain {
    PrimitiveValueDomain::new(
        string, number, boolean, big_int, symbol, null, undefined, object, finite, false,
    )
}

#[test]
fn demanded_callability_uses_compiler_call_signatures() {
    let source = "export function plain() {}\n\
export const value = 1;\n\
export const mixed = null as (() => void) | number;\n\
export function overloaded(value: string): string;\n\
export function overloaded(value: number): number;\n\
export function overloaded(value: string | number) { return value; }\n\
export const generic = <T>(value: T) => value;\n\
export class ConstructorOnly {}\n\
export const anyValue: any = plain;\n\
export const unknownValue: unknown = plain;\n\
export const neverValue = null as never;\n";
    let scratch = Scratch::new(&[("tsconfig.json", STRICT_ESNEXT), ("facts.ts", source)]);
    let path = scratch.path("facts.ts");
    let mut project = scratch.open();
    let cases = [
        ("plain", Callability::Callable),
        ("value", Callability::NonCallable),
        ("mixed", Callability::Mixed),
        ("overloaded", Callability::Callable),
        ("generic", Callability::Callable),
        ("ConstructorOnly", Callability::NonCallable),
        ("anyValue", Callability::Unknown),
        ("unknownValue", Callability::Unknown),
        ("neverValue", Callability::Unknown),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(name, _)| EntityDemand {
            location: find(&path, source, name),
            callability: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((name, want), fact) in cases.iter().zip(&facts) {
        assert_eq!(fact.callability, Some(*want), "{name} callability");
    }
    // Not demanded: absent, never a fabricated answer.
    assert!(facts.iter().all(|fact| fact.constructability.is_none()));
    assert_eq!(project.unavailable.count("callability"), 0);
}

/// The exact counterpart of the callability test; the class rows are why the
/// fact exists: the same declaration is nonCallable there and constructable
/// here.
#[test]
fn demanded_constructability_uses_compiler_construct_signatures() {
    let origin = "export class Widget {}\nexport function make() {}\n";
    let source = "import { Widget, make } from \"./origin\";\n\
import * as originNamespace from \"./origin\";\n\
class Local {}\n\
abstract class Abstract { abstract render(): void }\n\
function plain() {}\n\
const value = 1;\n\
interface Factory { (): Widget; new (): Widget }\n\
declare const factory: Factory;\n\
declare const constructorOnly: new () => Widget;\n\
declare const mixedConstruct: (new () => Widget) | number;\n\
declare const anyValue: any;\n\
declare const unknownValue: unknown;\n\
declare const neverValue: never;\n\
declare const middleware: Function;\n\
const aliasedWidget = Widget;\n\
const aliasedMake = make;\n\
export { Local, Abstract, plain, value, Widget as ReWidget, originNamespace, factory, constructorOnly, mixedConstruct, anyValue, unknownValue, neverValue, middleware, aliasedWidget, aliasedMake };\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", STRICT_BUNDLER),
        ("origin.ts", origin),
        ("facts.ts", source),
    ]);
    let path = scratch.path("facts.ts");
    let mut project = scratch.open();
    use Callability as C;
    use Constructability as K;
    // Each expression is matched at its LAST occurrence: the export clause.
    let cases = [
        ("Local", C::NonCallable, K::Constructable),
        // Abstract construct signatures are not filtered out.
        ("Abstract", C::NonCallable, K::Constructable),
        ("plain", C::Callable, K::NonConstructable),
        ("value", C::NonCallable, K::NonConstructable),
        ("factory", C::Callable, K::Constructable),
        ("constructorOnly", C::NonCallable, K::Constructable),
        ("mixedConstruct", C::NonCallable, K::Mixed),
        ("originNamespace", C::NonCallable, K::NonConstructable),
        ("ReWidget", C::NonCallable, K::Constructable),
        ("aliasedWidget", C::NonCallable, K::Constructable),
        ("aliasedMake", C::Callable, K::NonConstructable),
        ("anyValue", C::Unknown, K::Unknown),
        ("unknownValue", C::Unknown, K::Unknown),
        ("neverValue", C::Unknown, K::Unknown),
        // `Function` declares no signature: calling is legal (the untyped
        // call), `new` is a compile error.
        ("middleware", C::UntypedCallable, K::NonConstructable),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(name, ..)| EntityDemand {
            location: find_last(&path, source, name),
            callability: true,
            constructability: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    for ((name, callability, constructability), fact) in cases.iter().zip(&facts) {
        assert_eq!(fact.constructability, Some(*constructability), "{name}");
        assert_eq!(fact.callability, Some(*callability), "{name}");
    }

    // A class declaration name is not the export specifier's span: the type
    // there is the instance type, which has no construct signature.
    let instance = entities(
        &mut project,
        &path,
        &[EntityDemand {
            location: find(&path, source, "Local"),
            callability: true,
            constructability: true,
            ..EntityDemand::default()
        }],
    );
    assert_eq!(
        instance[0].constructability,
        Some(Constructability::NonConstructable),
        "class declaration-name constructability"
    );

    // An undemanded span carries no constructability at all.
    let undemanded = entities(
        &mut project,
        &path,
        &[EntityDemand {
            location: demands[0].location.clone(),
            callability: true,
            ..EntityDemand::default()
        }],
    );
    assert_eq!(undemanded[0].constructability, None);
}

/// The measured shapes the fact was written for: a class a bundler hides
/// behind an IIFE, a class reached only as a cross-file tuple element type,
/// and the two destructuring patterns.
#[test]
fn constructability_answers_bundler_lowered_classes() {
    let config = r#"{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler","allowJs":true},"include":["*.ts","*.js"]}"#;
    let utils = "export declare class DevtoolsCore { mount(): void }\n\
export declare function constructCoreClass(): [typeof DevtoolsCore];\n\
export declare const pair: [typeof DevtoolsCore, number];\n\
export declare const Container: { Inner: typeof DevtoolsCore; label: string };\n";
    // Authored JavaScript (see the Go test for why the file name matters).
    let envelope = "const ENVELOPE = Symbol(\"envelope\");\n\
export const ResponseEnvelope = /* @__PURE__ */ (() => {\n\
  class ResponseEnvelope {}\n\
  ResponseEnvelope.prototype[ENVELOPE] = true;\n\
  return ResponseEnvelope;\n\
})();\n";
    let source = "import { constructCoreClass, pair, Container } from \"./utils\";\n\
import { ResponseEnvelope } from \"./envelope.js\";\n\
const coreClasses = constructCoreClass();\n\
const TableDevtoolsCore = coreClasses[0];\n\
const { Inner } = Container;\n\
const { label } = Container;\n\
const [Core] = pair;\n\
var Downleveled: any;\n\
(function (Downleveled) { Downleveled[Downleveled[\"A\"] = 0] = \"A\"; })(Downleveled || (Downleveled = {}));\n\
const AnonymousClassExpr = class {};\n\
export { ResponseEnvelope, TableDevtoolsCore, Inner, label, Core, Downleveled, AnonymousClassExpr };\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", config),
        ("utils.ts", utils),
        ("envelope.js", envelope),
        ("artifact.ts", source),
    ]);
    let path = scratch.path("artifact.ts");
    let mut project = scratch.open();
    use Callability as C;
    use Constructability as K;
    let cases = [
        ("ResponseEnvelope", C::NonCallable, K::Constructable),
        ("TableDevtoolsCore", C::NonCallable, K::Constructable),
        ("Inner", C::NonCallable, K::Constructable),
        ("label", C::NonCallable, K::NonConstructable),
        ("Core", C::NonCallable, K::Constructable),
        // `any` closes no domain for either fact.
        ("Downleveled", C::Unknown, K::Unknown),
        ("AnonymousClassExpr", C::NonCallable, K::Constructable),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(name, ..)| EntityDemand {
            location: find_last(&path, source, name),
            callability: true,
            constructability: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    for ((name, callability, constructability), fact) in cases.iter().zip(&facts) {
        assert_eq!(fact.constructability, Some(*constructability), "{name}");
        assert_eq!(fact.callability, Some(*callability), "{name}");
    }
}

#[test]
fn demanded_runtime_value_domain_uses_checker_semantics() {
    let source = "\ninterface CallableInterface { (): void }\n\
type CleanupAlias = (() => void) | undefined;\n\
declare const functionValue: () => void;\n\
declare const undefinedValue: undefined;\n\
declare const cleanupUnion: (() => void) | undefined;\n\
declare const callableInterfaceValue: CallableInterface;\n\
declare const overloadedFunction: { (): void; (value: string): number };\n\
declare const aliasedCleanup: CleanupAlias;\n\
declare const callableIntersection: CallableInterface & { tag: string };\n\
declare const optionalHolder: { optionalCleanup?: () => void };\n\
optionalHolder.optionalCleanup;\n\
function constrained<T extends (() => void) | undefined>(boundedCleanup: T) { return boundedCleanup; }\n\
declare const numberValue: number;\n\
declare const nullValue: null;\n\
declare const objectValue: object;\n\
declare const promiseValue: Promise<void>;\n\
declare const callableNumber: (() => void) | number;\n\
declare const undefinedString: undefined | string;\n\
declare const cleanupNull: (() => void) | undefined | null;\n\
declare const objectIntersection: { left: true } & { right: true };\n\
declare const anyValue: any;\n\
declare const unknownValue: unknown;\n\
declare const neverValue: never;\n\
function unconstrained<T>(genericValue: T) { return genericValue; }\n\
declare const recoveryValue: MissingType;\n\
declare function voidCall(): void;\n\
const voidResult = voidCall();\n";
    let scratch = Scratch::new(&[("tsconfig.json", STRICT_ESNEXT), ("domains.ts", source)]);
    let path = scratch.path("domains.ts");
    let mut project = scratch.open();
    let callable = rvd(true, false, false, false);
    let undefined = rvd(false, true, false, false);
    let cleanup = rvd(true, true, false, false);
    let other = rvd(false, false, true, false);
    let cases = [
        ("functionValue", callable),
        ("undefinedValue", undefined),
        ("cleanupUnion", cleanup),
        ("callableInterfaceValue", callable),
        ("overloadedFunction", callable),
        ("aliasedCleanup", cleanup),
        ("callableIntersection", callable),
        ("optionalCleanup", cleanup),
        ("boundedCleanup", cleanup),
        // A void result is undefined at runtime, trusted like any other
        // declared return type.
        ("voidResult", undefined),
        ("numberValue", other),
        ("nullValue", other),
        ("objectValue", other),
        ("promiseValue", other),
        ("callableNumber", rvd(true, false, true, false)),
        ("undefinedString", rvd(false, true, true, false)),
        ("cleanupNull", rvd(true, true, true, false)),
        ("objectIntersection", other),
        ("anyValue", rvd(true, true, true, true)),
        ("unknownValue", rvd(true, true, true, true)),
        ("neverValue", rvd(false, false, false, false)),
        ("genericValue", rvd(true, true, true, true)),
        ("recoveryValue", rvd(true, true, true, true)),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(name, _)| EntityDemand {
            location: find_last(&path, source, name),
            runtime_value_domain: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((name, want), fact) in cases.iter().zip(&facts) {
        assert_eq!(fact.runtime_value_domain, Some(*want), "{name}");
    }
}

#[test]
fn demanded_primitive_value_domain_uses_checker_semantics_at_exact_spans() {
    let source = "\ntype TextAlias = string;\n\
type BrandedText = string & { readonly __brand: unique symbol };\n\
declare const textValue: TextAlias;\n\
declare const brandedText: BrandedText;\n\
declare const numberValue: number;\n\
declare const finiteValue: 42;\n\
declare const finiteUnion: 1 | 2 | \"three\";\n\
declare const overflowValue: number;\n\
declare const booleanValue: boolean;\n\
declare const bigintValue: bigint;\n\
declare const symbolValue: symbol;\n\
declare const nullValue: null;\n\
declare const undefinedValue: undefined;\n\
declare const objectValue: { value: string };\n\
declare const functionValue: () => void;\n\
declare const safeUnion: string | boolean | null | undefined;\n\
declare const unsafeUnion: string | bigint | object;\n\
function constrained<T extends string | boolean>(bounded: T) { return bounded; }\n\
function unconstrained<T>(generic: T) { return generic; }\n\
declare const anyValue: any;\n\
declare const unknownValue: unknown;\n\
declare const neverValue: never;\n\
declare const recoveryValue: MissingType;\n\
declare function voidCall(): void;\n\
const voidResult = voidCall();\n\
textValue;\nbrandedText;\nnumberValue;\nfiniteValue;\nfiniteUnion;\noverflowValue;\n\
booleanValue;\nbigintValue;\nsymbolValue;\nnullValue;\nundefinedValue;\nobjectValue;\n\
functionValue;\nsafeUnion;\nunsafeUnion;\nanyValue;\nunknownValue;\nneverValue;\n\
recoveryValue;\nvoidResult;\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", STRICT_ESNEXT),
        ("primitive-domains.ts", source),
    ]);
    let path = scratch.path("primitive-domains.ts");
    let mut project = scratch.open();
    let (f, t) = (false, true);
    let unknown = crate::type_facts::unknown_primitive_value_domain();
    let string = pvd(t, f, f, f, f, f, f, f, f);
    let object = pvd(f, f, f, f, f, f, f, t, f);
    let cases = [
        ("textValue", string),
        ("brandedText", string),
        ("numberValue", pvd(f, t, f, f, f, f, f, f, f)),
        ("finiteValue", pvd(f, t, f, f, f, f, f, f, t)),
        ("finiteUnion", pvd(t, t, f, f, f, f, f, f, t)),
        ("overflowValue", pvd(f, t, f, f, f, f, f, f, f)),
        ("booleanValue", pvd(f, f, t, f, f, f, f, f, f)),
        ("bigintValue", pvd(f, f, f, t, f, f, f, f, f)),
        ("symbolValue", pvd(f, f, f, f, t, f, f, f, f)),
        ("nullValue", pvd(f, f, f, f, f, t, f, f, f)),
        ("undefinedValue", pvd(f, f, f, f, f, f, t, f, f)),
        ("objectValue", object),
        ("functionValue", object),
        ("safeUnion", pvd(t, f, t, f, f, t, t, f, f)),
        ("unsafeUnion", pvd(t, f, f, t, f, f, f, t, f)),
        ("bounded", pvd(t, f, t, f, f, f, f, f, f)),
        ("generic", unknown),
        ("anyValue", unknown),
        ("unknownValue", unknown),
        ("neverValue", pvd(f, f, f, f, f, f, f, f, f)),
        ("recoveryValue", unknown),
        ("voidResult", pvd(f, f, f, f, f, f, t, f, f)),
    ];
    let mut demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(name, _)| {
            let statement = format!("\n{name};");
            let start = source
                .rfind(&statement)
                .map(|start| start + 1)
                .or_else(|| source.rfind(&format!("{name};")))
                .unwrap_or_else(|| panic!("{name} not found"));
            EntityDemand {
                location: loc(&path, start, start + name.len()),
                primitive_value_domain: true,
                ..EntityDemand::default()
            }
        })
        .collect();
    let text_start = source.rfind("textValue;").unwrap();
    demands.push(EntityDemand {
        location: loc(&path, text_start, text_start + "textValue".len() - 1),
        primitive_value_domain: true,
        ..EntityDemand::default()
    });
    let facts = entities(&mut project, &path, &demands);
    for ((name, want), fact) in cases.iter().zip(&facts) {
        assert!(
            fact.primitive_value_domain.is_present(),
            "{name} primitive value domain is absent"
        );
        assert_eq!(fact.primitive_value_domain, *want, "{name}");
    }
    assert!(
        !facts[cases.len()].primitive_value_domain.is_present(),
        "non-exact primitive-domain demand unexpectedly answered"
    );
}

fn literal(
    kind: PrimitiveLiteralKind,
    string: &str,
    number: f64,
    boolean: bool,
) -> PrimitiveLiteralCandidate {
    PrimitiveLiteralCandidate {
        kind,
        string: string.into(),
        number,
        boolean,
    }
}

#[test]
fn primitive_literal_candidates_are_exact_compiler_inhabitants() {
    let source = "\ndeclare const literalUnion: \"alpha\" | \"beta\" | -1 | 2 | false | true;\n\
declare const broadUnion: string | 3;\n\
declare const branded: \"tag\" & { readonly brand: unique symbol };\n\
declare enum Choice { A = \"a\", B = 2 }\n\
declare const enumValue: Choice;\n\
function constrained<T extends \"left\" | \"right\">(value: T) { value; }\n\
function generic<T>(value: T) { value; }\n\
literalUnion;\nbroadUnion;\nbranded;\nenumValue;\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", STRICT_ESNEXT),
        ("literal-candidates.ts", source),
    ]);
    let path = scratch.path("literal-candidates.ts");
    let mut project = scratch.open();
    let names = [
        "literalUnion",
        "broadUnion",
        "branded",
        "enumValue",
        "value",
    ];
    let mut demands: Vec<EntityDemand> = names
        .iter()
        .map(|name| EntityDemand {
            location: at(&path, source, &format!("{name};"), true).truncated(name.len()),
            primitive_literal_candidates: true,
            ..EntityDemand::default()
        })
        .collect();
    // The earlier constrained parameter, not the unconstrained one selected
    // by the last occurrence.
    demands.push(EntityDemand {
        location: find(&path, source, "value: T").truncated("value".len()),
        primitive_literal_candidates: true,
        ..EntityDemand::default()
    });
    let facts = entities(&mut project, &path, &demands);
    use PrimitiveLiteralKind as L;
    assert_eq!(
        facts[0].primitive_literal_candidates.as_deref(),
        Some(&vec![
            literal(L::Boolean, "", 0.0, false),
            literal(L::Boolean, "", 0.0, true),
            literal(L::Number, "", -1.0, false),
            literal(L::Number, "", 2.0, false),
            literal(L::String, "alpha", 0.0, false),
            literal(L::String, "beta", 0.0, false),
        ]),
        "literalUnion candidates"
    );
    assert_eq!(
        facts[1].primitive_literal_candidates.as_deref(),
        Some(&vec![literal(L::Number, "", 3.0, false)]),
        "broadUnion candidates"
    );
    for (index, name) in ["branded", "enumValue", "generic"].iter().enumerate() {
        let candidates = facts[index + 2].primitive_literal_candidates.as_deref();
        assert!(
            candidates.is_some_and(Vec::is_empty),
            "{name} unexpectedly has candidates {candidates:?}"
        );
    }
    let constrained = facts[5].primitive_literal_candidates.as_deref().unwrap();
    assert!(
        constrained.len() == 2
            && &*constrained[0].string == "left"
            && &*constrained[1].string == "right",
        "constrained candidates = {constrained:?}"
    );
}

trait Truncated {
    fn truncated(self, length: usize) -> Self;
}

impl Truncated for typefacts::Location {
    /// The same start, `length` bytes long.
    fn truncated(mut self, length: usize) -> Self {
        self.end_byte = self.start_byte + length as u64;
        self
    }
}

#[test]
fn callability_answers_the_signature_less_function_supertype_family() {
    let source = "declare const bare: Function;\n\
declare const callableFunction: CallableFunction;\n\
declare const newableFunction: NewableFunction;\n\
type Handler = Function;\n\
declare const aliased: Handler;\n\
interface Middleware extends Function { tag: string }\n\
declare const extended: Middleware;\n\
declare const branded: Function & { brand: \"route\" };\n\
const assignedFunction: Function = () => {};\n\
declare const plainObject: object;\n\
declare const emptyObject: {};\n\
declare const record: Record<string, unknown>;\n\
interface OnlyBind { bind(this: void): void }\n\
declare const onlyBind: OnlyBind;\n\
declare const numeric: number;\n\
declare const typedFunction: () => void;\n\
class Widget {}\n\
declare const withNumber: Function | number;\n\
declare const optional: Function | undefined;\n\
declare const eitherFunction: Function | (() => void);\n\
export { bare, callableFunction, newableFunction, aliased, extended, branded, assignedFunction, plainObject, emptyObject, record, onlyBind, numeric, typedFunction, Widget, withNumber, optional, eitherFunction };\n";
    let scratch = Scratch::new(&[("tsconfig.json", STRICT_ESNEXT), ("facts.ts", source)]);
    let path = scratch.path("facts.ts");
    let mut project = scratch.open();
    use Callability as C;
    use Constructability as K;
    let cases = [
        // The family: callable at runtime, no signature to read.
        ("bare", C::UntypedCallable, K::NonConstructable),
        ("callableFunction", C::UntypedCallable, K::NonConstructable),
        ("newableFunction", C::UntypedCallable, K::NonConstructable),
        ("aliased", C::UntypedCallable, K::NonConstructable),
        ("extended", C::UntypedCallable, K::NonConstructable),
        // The row that decides the predicate: isFunctionObjectType is false
        // for an intersection, the untyped-call rule is true.
        ("branded", C::UntypedCallable, K::NonConstructable),
        ("assignedFunction", C::UntypedCallable, K::NonConstructable),
        // Controls: outside the family.
        ("plainObject", C::NonCallable, K::NonConstructable),
        ("emptyObject", C::NonCallable, K::NonConstructable),
        ("record", C::NonCallable, K::NonConstructable),
        ("onlyBind", C::NonCallable, K::NonConstructable),
        ("numeric", C::NonCallable, K::NonConstructable),
        // Unchanged answers.
        ("typedFunction", C::Callable, K::NonConstructable),
        ("Widget", C::NonCallable, K::Constructable),
        // Aggregation.
        ("withNumber", C::Mixed, K::NonConstructable),
        ("optional", C::Mixed, K::NonConstructable),
        ("eitherFunction", C::UntypedCallable, K::NonConstructable),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(name, ..)| EntityDemand {
            location: find_last(&path, source, name),
            callability: true,
            constructability: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((name, callability, constructability), fact) in cases.iter().zip(&facts) {
        assert_eq!(fact.callability, Some(*callability), "{name} callability");
        assert_eq!(
            fact.constructability,
            Some(*constructability),
            "{name} constructability"
        );
    }
}

/// Asked of the compiler's own diagnostics: calling a `Function`-typed value
/// is legal (the untyped call) and `new`-ing one is not. That is why
/// callability gained a value for this family and constructability did not.
#[test]
fn the_function_supertype_family_is_callable_but_not_constructable() {
    let source = "declare const handler: Function;\n\
export const called = handler();\n\
export const constructed = new handler();\n";
    let scratch = Scratch::new(&[("tsconfig.json", STRICT_ESNEXT), ("facts.ts", source)]);
    let path = scratch.path("facts.ts");
    let mut project = scratch.open();
    // Wake the checker the way the producer would before asking.
    project
        .symbol_at(&Context::new(), &find(&path, source, "handler"))
        .unwrap();
    let generation = project.current().unwrap();
    let file = generation.program().get_source_file(&path).unwrap();
    let call_start = source.find("handler()").unwrap() as u32;
    let construct_start = source.find("new handler()").unwrap() as u32;
    let (mut on_call, mut on_construct) = (Vec::new(), Vec::new());
    for diagnostic in generation
        .program()
        .get_semantic_diagnostics(&tsrs_core::Context::default(), Some(file))
    {
        let Some(position) = diagnostic.pos() else {
            continue;
        };
        if position >= call_start && position < call_start + "handler()".len() as u32 {
            on_call.push(diagnostic.code());
        } else if position >= construct_start
            && position < construct_start + "new handler()".len() as u32
        {
            on_construct.push(diagnostic.code());
        }
    }
    assert!(
        on_call.is_empty(),
        "calling a Function-typed value reported {on_call:?}"
    );
    assert!(
        !on_construct.is_empty(),
        "new on a Function-typed value reported no diagnostic"
    );
}

/// `TestSemanticDemandRunsClassifiesTheCompleteDemandedExpression`: `factory`
/// and `factory()` start at the same byte but have different callability and
/// runtime domains.
#[test]
fn semantic_demand_runs_classify_the_complete_demanded_expression() {
    let source = "declare function factory(): any;\nexport const result = factory();\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true},"include":["*.ts"]}"#,
        ),
        ("run.ts", source),
    ]);
    let path = scratch.path("run.ts");
    let mut project = scratch.open();
    let start = source.find("factory();").unwrap();
    let demand = |end: usize| EntityDemand {
        location: loc(&path, start, end),
        callability: true,
        runtime_value_domain: true,
        ..EntityDemand::default()
    };
    let facts = entities(
        &mut project,
        &path,
        &[
            demand(start + "factory".len()),
            demand(start + "factory()".len()),
        ],
    );
    assert_eq!(facts[0].callability, Some(Callability::Callable));
    assert_eq!(facts[1].callability, Some(Callability::Unknown));
    assert_eq!(
        facts[0].runtime_value_domain,
        Some(rvd(true, false, false, false))
    );
    assert_eq!(
        facts[1].runtime_value_domain,
        Some(rvd(true, true, true, true))
    );
}

/// `TestDescribeTypePreservesSolidAccessorAliasOrigin` over the
/// `testdata/accessor-alias` fixture, inlined.
#[test]
fn describe_type_preserves_solid_accessor_alias_origin() {
    let config = r#"{"compilerOptions":{"module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"include":["*.ts"]}"#;
    let declaration = "declare module \"solid-js\" {\n  export type Accessor<T> = () => T;\n}\n";
    let parameter = "import type { Accessor } from \"solid-js\";\n\nexport function install(read: Accessor<number>) {\n  return read();\n}\n";
    let member = "import type { Accessor } from \"solid-js\";\n\nexport function install(props: { read: Accessor<number> }) {\n  return props.read();\n}\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", config),
        ("solid-js.d.ts", declaration),
        ("parameter.ts", parameter),
        ("member.ts", member),
    ]);
    let mut project = scratch.open();
    for (name, source, call) in [
        ("parameter.ts", parameter, "read()"),
        ("member.ts", member, "props.read()"),
    ] {
        let path = scratch.path(name);
        let mut start = source.find(call).unwrap();
        if name == "member.ts" {
            start += "props.".len();
        }
        let descriptor = project
            .describe_type_at(&Context::new(), &loc(&path, start, start + "read".len()))
            .unwrap();
        assert!(
            descriptor
                .alias_declarations
                .first()
                .is_some_and(|declaration| &*declaration.name == "Accessor"),
            "{name} descriptor = {descriptor:?}"
        );
        assert_eq!(
            &*descriptor.origin_module, "solid-js",
            "{name} origin module"
        );
    }
}

/// `TestRangeEntitiesProjectsARealRetainedAnswer`: one symbol demanded through
/// the real backend, read back through the retained table the closure
/// returned, whose public entity rows are empty.
#[test]
fn range_entities_projects_a_real_retained_answer() {
    let source = "export const exact = 1;\nexport const other = exact;\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","types":[]},"include":["*.ts"]}"#,
        ),
        ("main.ts", source),
    ]);
    let path = scratch.path("main.ts");
    let demand = EntityDemand {
        location: find_last(&path, source, "exact"),
        symbol: true,
        ..EntityDemand::default()
    };
    let mut closure = DemandClosure::new(Box::new(scratch.open()), None);
    let table = closure
        .demand_table_for_groups(
            &Context::new(),
            1,
            &[DemandInputGroup {
                path: path.clone(),
                demands: vec![demand.clone()],
            }],
            std::slice::from_ref(&path),
        )
        .unwrap();
    assert!(
        matches!(table.entities, EntityRows::Runs(_)),
        "the public rows were used; the retained path did not run"
    );
    let mut answered: Vec<EntityFact> = Vec::new();
    table.entities.for_each(|entity| {
        if entity.location == demand.location {
            answered.push(entity.clone());
        }
    });
    assert!(
        answered.len() == 1
            && !answered[0].symbol.is_empty()
            && answered[0].symbol_claim != typefacts::SymbolClaim::Unresolved,
        "demand at {:?} answered {answered:?}",
        demand.location
    );
}

/// The property order of a normalized object-literal union is a function of
/// the program under tsrs's canonical history (ADR 0367). typescript-go keys
/// `undefinedProperties` by name across the program, so there `cols?` took the
/// declaration of the first `cols` the checker met and `b` printed
/// `{ cols?: undefined; nodes: string; }`; the canonical history prints the
/// literal's own order, whichever literal is checked first.
#[test]
fn normalized_object_literal_union_text_does_not_follow_check_history() {
    let source = "export const a = [{ groups: 'g' }, { cols: 'TEMP' }];\n\
export const b = [{ nodes: 'r' }, { cols: 'TEMP' }];\n\
export const c = [{ cols: 'x' }, { nodes: 'r' }];\n";
    let scratch = Scratch::new(&[("tsconfig.json", STRICT_BUNDLER), ("a.ts", source)]);
    let path = scratch.path("a.ts");
    let cases = [
        (
            "a",
            "({ groups: string; cols?: undefined; } | { groups?: undefined; cols: string; })[]",
        ),
        (
            "b",
            "({ nodes: string; cols?: undefined; } | { nodes?: undefined; cols: string; })[]",
        ),
        (
            "c",
            "({ cols: string; nodes?: undefined; } | { cols?: undefined; nodes: string; })[]",
        ),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(name, _)| EntityDemand {
            location: find(&path, source, &format!("const {name}")),
            ..EntityDemand::default()
        })
        .map(|mut demand| {
            // The binding name, after `const `.
            demand.location.start_byte += "const ".len() as u64;
            demand.location.end_byte = demand.location.start_byte + 1;
            demand.type_descriptor = true;
            demand
        })
        .collect();
    // Source order, then `c` (whose literal names `cols` first) asked first on
    // a fresh checker.
    for reversed in [false, true] {
        let mut project = scratch.open();
        let mut asked = demands.clone();
        if reversed {
            asked.reverse();
        }
        let mut facts = entities(&mut project, &path, &asked);
        if reversed {
            facts.reverse();
        }
        for ((name, want), fact) in cases.iter().zip(&facts) {
            let got = fact.type_descriptor.as_ref().map(|d| &*d.text);
            assert_eq!(got, Some(*want), "{name}, reversed {reversed}");
        }
    }
}
