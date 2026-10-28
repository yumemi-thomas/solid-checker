//! Ports the constant-value, array-shape, tuple-shape and library-type cases of
//! `tsgo/compiler_facts_test.go`
//! (`TestDemandedConstantValueUsesExactSpansAndBoundedFolding`,
//! `TestDemandedArrayShapeUsesCompilerArrayPredicateAtExactSpans`,
//! `TestDemandedTupleShapeDescribesSlotsAndFirstElement`,
//! `TestContextualTypingDecidesJsxLiteralTupleness`,
//! `TestDemandedLibraryTypesResolveThroughSpelling`).

use super::project_support::{Scratch, entities, find_last, loc};
use typefacts::v3::EntityDemand;
use typefacts::{ArrayShape, Callability, ConstantValue, ConstantValueKind, TupleShape};

fn text(value: &str) -> Option<ConstantValue> {
    Some(ConstantValue {
        kind: ConstantValueKind::String,
        string: value.into(),
        number: 0.0,
    })
}

fn number(value: f64) -> Option<ConstantValue> {
    Some(ConstantValue {
        kind: ConstantValueKind::Number,
        string: "".into(),
        number: value,
    })
}

#[test]
fn demanded_constant_value_uses_exact_spans_and_bounded_folding() {
    let source = "import { IMPORTED } from \"./dep\";\n\
const CONST_A = \"a\";\n\
let LET_A = \"a\";\n\
enum Choice { Value = 7 }\n\
class Holder { static readonly Value = \"held\"; }\n\
function f(): string { return \"dynamic\"; }\n\
function parameterValue(parameter: string) { return parameter; }\n\
const SELF = SELF;\n\
\n\
\"<p>Hello</p>\" + \"<p>world!</p>\";\n\
\"plain\";\n\
42;\n\
1 + 2;\n\
-3;\n\
+4;\n\
`foo`;\n`foo${CONST_A}`;\n\
\n\
(\"cast\") as string;\n\
(\"satisfied\") satisfies string;\n\
(\"nonnull\")!;\n\
CONST_A + \"b\";\n\
IMPORTED + \"d\";\n\
LET_A + \"b\";\n\
Choice.Value;\n\
Holder.Value;\n\
1 + \"a\";\n\
f() + \"a\";\n\
SELF;\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}"#,
        ),
        ("constants.ts", source),
        ("dep.ts", "export const IMPORTED = \"importe\";"),
    ]);
    let path = scratch.path("constants.ts");
    let mut project = scratch.open();
    let cases: Vec<(&str, Option<ConstantValue>)> = vec![
        (
            "\"<p>Hello</p>\" + \"<p>world!</p>\"",
            text("<p>Hello</p><p>world!</p>"),
        ),
        ("\"plain\"", text("plain")),
        ("42", number(42.0)),
        ("1 + 2", number(3.0)),
        ("-3", number(-3.0)),
        ("+4", number(4.0)),
        ("`foo`", text("foo")),
        ("`foo${CONST_A}`", None),
        ("(\"cast\") as string", text("cast")),
        ("(\"satisfied\") satisfies string", text("satisfied")),
        ("(\"nonnull\")!", text("nonnull")),
        ("CONST_A + \"b\"", text("ab")),
        ("IMPORTED + \"d\"", text("imported")),
        ("LET_A + \"b\"", None),
        ("Choice.Value", number(7.0)),
        ("Holder.Value", text("held")),
        ("1 + \"a\"", None),
        ("f() + \"a\"", None),
        ("parameter", None),
        ("SELF", None),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(expression, _)| EntityDemand {
            location: find_last(&path, source, expression),
            constant_value: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((expression, want), fact) in cases.iter().zip(&facts) {
        assert_eq!(&fact.constant_value, want, "{expression} constant value");
    }

    let concatenation = cases[0].0;
    let outer = source.rfind(concatenation).unwrap();
    let leading = "\"<p>Hello</p>\"";
    let sub_nodes = entities(
        &mut project,
        &path,
        &[
            EntityDemand {
                location: loc(&path, outer, outer + leading.len()),
                constant_value: true,
                ..EntityDemand::default()
            },
            EntityDemand {
                location: loc(&path, outer, outer + concatenation.len() - 1),
                constant_value: true,
                ..EntityDemand::default()
            },
        ],
    );
    assert_eq!(sub_nodes[0].constant_value, text("<p>Hello</p>"));
    assert_eq!(
        sub_nodes[1].constant_value, None,
        "non-exact concatenation span"
    );
    let undemanded = entities(
        &mut project,
        &path,
        &[EntityDemand {
            location: demands[0].location.clone(),
            type_descriptor: true,
            ..EntityDemand::default()
        }],
    );
    assert_eq!(undemanded[0].constant_value, None, "undemanded");
}

const SHAPES_CONFIG: &str = r#"{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","lib":["esnext","dom"]},"include":["*.ts"]}"#;

#[test]
fn demanded_array_shape_uses_compiler_array_predicate_at_exact_spans() {
    let source = "type Handlers = [(data: number, event: MouseEvent) => void, number];\n\
interface SafeArray<T> extends Array<T> {}\n\
\n\
declare const aliasedTuple: Handlers;\n\
declare const bareTuple: [(d: number) => void, number];\n\
declare const readonlyTuple: readonly [string, number];\n\
declare const arrayGeneric: Array<string>;\n\
declare const readonlyArrayGeneric: ReadonlyArray<string>;\n\
declare const suffixArray: string[];\n\
declare const functionArray: ((n: number) => void)[];\n\
declare const arrayReturningFunction: () => string[];\n\
declare const plainFunction: (event: MouseEvent) => void;\n\
declare const plainString: string;\n\
declare const safeArrayValue: SafeArray<number>;\n\
declare const maybeTuple: Handlers | undefined;\n\
declare const anyValue: any;\n\
declare const neverValue: never;\n\
function genericSubject<T>(genericValue: T) { genericValue; }\n\
\n\
aliasedTuple;\n\
bareTuple;\n\
readonlyTuple;\n\
arrayGeneric;\n\
readonlyArrayGeneric;\n\
suffixArray;\n\
functionArray;\n\
arrayReturningFunction;\n\
plainFunction;\n\
plainString;\n\
safeArrayValue;\n\
maybeTuple;\n\
anyValue;\n\
neverValue;\n\
[(n: number) => n * n, 2];\n";
    let scratch = Scratch::new(&[("tsconfig.json", SHAPES_CONFIG), ("shapes.ts", source)]);
    let path = scratch.path("shapes.ts");
    let mut project = scratch.open();
    let cases = [
        // An alias renders as its own name and defeats every text test; the
        // compiler predicate sees through it to the tuple.
        ("aliasedTuple", ArrayShape::Array),
        ("bareTuple", ArrayShape::Array),
        ("readonlyTuple", ArrayShape::Array),
        ("arrayGeneric", ArrayShape::Array),
        ("readonlyArrayGeneric", ArrayShape::Array),
        ("suffixArray", ArrayShape::Array),
        // An array of functions and a function returning an array render with
        // the same trailing `[]`; asking the type separates them.
        ("functionArray", ArrayShape::Array),
        ("arrayReturningFunction", ArrayShape::NotArray),
        ("plainFunction", ArrayShape::NotArray),
        ("plainString", ArrayShape::NotArray),
        // Array-like is deliberately not enough.
        ("safeArrayValue", ArrayShape::NotArray),
        ("maybeTuple", ArrayShape::Mixed),
        ("anyValue", ArrayShape::Unknown),
        ("neverValue", ArrayShape::Unknown),
        ("genericValue", ArrayShape::Unknown),
        ("[(n: number) => n * n, 2]", ArrayShape::Array),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(expression, _)| EntityDemand {
            location: find_last(&path, source, expression),
            array_shape: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((expression, want), fact) in cases.iter().zip(&facts) {
        assert_eq!(fact.array_shape, Some(*want), "{expression} array shape");
    }

    // A span that is not exactly one expression yields no field at all, and an
    // undemanded fact stays absent. Both are distinct from Unknown: the
    // producer did not look.
    let literal = "[(n: number) => n * n, 2]";
    let start = source.rfind(literal).unwrap();
    let boundary = entities(
        &mut project,
        &path,
        &[
            EntityDemand {
                location: loc(&path, start, start + literal.len() - 1),
                array_shape: true,
                ..EntityDemand::default()
            },
            EntityDemand {
                location: loc(&path, start, start + literal.len()),
                type_descriptor: true,
                ..EntityDemand::default()
            },
        ],
    );
    assert_eq!(boundary[0].array_shape, None, "non-exact span");
    assert_eq!(boundary[1].array_shape, None, "undemanded");
}

/// A tuple shape in Go's field order: fixed length, exact length (`None` for
/// `ExactLengthKnown: false`), rest, first-slot callability and minimum
/// parameters.
fn tuple(
    fixed: u32,
    exact: Option<u32>,
    rest: bool,
    element_zero: Callability,
    minimum: u32,
) -> Option<TupleShape> {
    Some(TupleShape::for_producer(fixed, rest, element_zero, minimum, exact).unwrap())
}

#[test]
fn demanded_tuple_shape_describes_slots_and_first_element() {
    let source = "declare const pair: [(data: number, event: MouseEvent) => void, number];\n\
declare const overArity: [(a: number, b: MouseEvent, c: string) => void, number];\n\
declare const optionalArity: [(a: number, b: MouseEvent, c?: string) => void, number];\n\
declare const restArity: [(...args: unknown[]) => void, number];\n\
declare const numbers: [number, number, number];\n\
declare const single: [(event: MouseEvent) => void];\n\
declare const optionalTail: [(event: MouseEvent) => void, number?];\n\
declare const restTail: [(event: MouseEvent) => void, ...number[]];\n\
declare const spreadOnly: [...((event: MouseEvent) => void)[]];\n\
declare const roPair: readonly [(event: MouseEvent) => void, number];\n\
type Handlers = [(data: number, event: MouseEvent) => void, number];\n\
declare const aliased: Handlers;\n\
declare const plainArray: ((event: MouseEvent) => void)[];\n\
declare const readonlyArray: ReadonlyArray<(event: MouseEvent) => void>;\n\
declare const maybePair: Handlers | undefined;\n\
type OtherHandlers = [(data: string, event: MouseEvent) => void, string];\n\
declare const bothPairs: Handlers | OtherHandlers;\n\
declare const unionOverArity: Handlers | [(a: number, b: MouseEvent, c: string) => void, number];\n\
declare const unionBadHead: Handlers | [number, number];\n\
declare const unionRest: Handlers | [(e: MouseEvent) => void, ...number[]];\n\
declare const unionFunction: Handlers | ((e: MouseEvent) => void);\n\
declare const unionArray: Handlers | number[];\n\
declare const notArray: (event: MouseEvent) => void;\n\
declare const anyValue: any;\n\
declare const empty: [];\n\
declare const untypedSlot: [Function];\n\
declare const untypedUnion: [Function] | [() => void];\n\
\n\
pair;\n\
overArity;\n\
optionalArity;\n\
restArity;\n\
numbers;\n\
single;\n\
optionalTail;\n\
restTail;\n\
spreadOnly;\n\
roPair;\n\
aliased;\n\
plainArray;\n\
readonlyArray;\n\
maybePair;\n\
bothPairs;\n\
unionOverArity;\n\
unionBadHead;\n\
unionRest;\n\
unionFunction;\n\
unionArray;\n\
notArray;\n\
anyValue;\n\
empty;\n\
untypedSlot;\n\
untypedUnion;\n";
    let scratch = Scratch::new(&[("tsconfig.json", SHAPES_CONFIG), ("tuples.ts", source)]);
    let path = scratch.path("tuples.ts");
    let mut project = scratch.open();
    use Callability::{Callable, Mixed, NonCallable, Unknown, UntypedCallable};
    let cases: Vec<(&str, Option<TupleShape>)> = vec![
        ("pair", tuple(2, Some(2), false, Callable, 2)),
        // The first slot is callable but demands more arguments than a caller
        // with two will supply.
        ("overArity", tuple(2, Some(2), false, Callable, 3)),
        // Optional and rest parameters lower the requirement.
        ("optionalArity", tuple(2, Some(2), false, Callable, 2)),
        ("restArity", tuple(2, Some(2), false, Callable, 0)),
        ("numbers", tuple(3, Some(3), false, NonCallable, 0)),
        ("single", tuple(1, Some(1), false, Callable, 1)),
        // An optional slot still counts toward fixedLength.
        ("optionalTail", tuple(2, None, false, Callable, 1)),
        ("restTail", tuple(1, None, true, Callable, 1)),
        ("roPair", tuple(2, Some(2), false, Callable, 1)),
        ("aliased", tuple(2, Some(2), false, Callable, 2)),
        ("empty", tuple(0, Some(0), false, Unknown, 0)),
        // Arrays have a number index signature, not fixed slots.
        ("plainArray", None),
        ("readonlyArray", None),
        // The compiler normalizes a spread-only tuple to the array type.
        ("spreadOnly", None),
        ("maybePair", tuple(2, Some(2), false, Callable, 2)),
        ("bothPairs", tuple(2, Some(2), false, Callable, 2)),
        // The meet takes the strictest demand.
        ("unionOverArity", tuple(2, Some(2), false, Callable, 3)),
        // One head is not callable, so the union's is not provably callable.
        ("unionBadHead", tuple(2, Some(2), false, Mixed, 2)),
        // Slots are kept only where both have them.
        ("unionRest", tuple(1, None, false, Callable, 2)),
        // A constituent that is not a tuple voids the whole answer.
        ("unionFunction", None),
        ("unionArray", None),
        ("notArray", None),
        ("anyValue", None),
        // The signature-less Function-supertype family reaches elementZero
        // exactly as it reaches callability.
        ("untypedSlot", tuple(1, Some(1), false, UntypedCallable, 0)),
        ("untypedUnion", tuple(1, Some(1), false, UntypedCallable, 0)),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(expression, _)| EntityDemand {
            location: find_last(&path, source, expression),
            tuple_shape: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((expression, want), fact) in cases.iter().zip(&facts) {
        assert_eq!(&fact.tuple_shape, want, "{expression} tuple shape");
    }

    let undemanded = entities(
        &mut project,
        &path,
        &[EntityDemand {
            location: demands[0].location.clone(),
            array_shape: true,
            ..EntityDemand::default()
        }],
    );
    assert_eq!(undemanded[0].tuple_shape, None, "undemanded tuple shape");
    assert_eq!(undemanded[0].array_shape, Some(ArrayShape::Array));
}

/// Contextual typing decides whether a JSX array literal becomes a tuple, and
/// consumers depend on that: a literal in a position typed by an interface with
/// numbered members gets fixed slots, while the same literal in an
/// unconstrained position stays a plain array.
#[test]
fn contextual_typing_decides_jsx_literal_tupleness() {
    let globals = "declare namespace JSX {\n\
  interface EventHandler<T, E> { (e: E & { currentTarget: T; target: Element }): void }\n\
  interface BoundEventHandler<T, E, EH extends EventHandler<T, any> = EventHandler<T, E>> {\n\
    0: (data: any, ...e: Parameters<EH>) => void;\n\
    1: any;\n\
  }\n\
  type EventHandlerUnion<T, E, EH extends EventHandler<T, any> = EventHandler<T, E>> =\n\
    EH | BoundEventHandler<T, E, EH>;\n\
  interface IntrinsicElements {\n\
    button: { onClick?: EventHandlerUnion<HTMLButtonElement, MouseEvent> };\n\
    loose: any;\n\
  }\n\
  interface Element {}\n\
}\n";
    let source = "declare const handler: (data: number, event: MouseEvent) => void;\n\
\n\
export const Pair = () => <button onClick={[handler, 1]} />;\n\
export const NotAPair = () => <button onClick={[1, 2, 3]} />;\n\
export const Unconstrained = () => <loose onClick={[handler, 1]} />;\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"jsx":"preserve","module":"esnext","target":"esnext","lib":["esnext","dom"]},"include":["*.tsx","*.d.ts"]}"#,
        ),
        ("globals.d.ts", globals),
        ("contextual.tsx", source),
    ]);
    let path = scratch.path("contextual.tsx");
    let mut project = scratch.open();
    use Callability::{Callable, NonCallable};
    let cases: Vec<(&str, &str, usize, Option<TupleShape>)> = vec![
        (
            "bound pair",
            "[handler, 1]",
            0,
            tuple(2, Some(2), false, Callable, 2),
        ),
        (
            "wrong element types",
            "[1, 2, 3]",
            0,
            tuple(3, Some(3), false, NonCallable, 0),
        ),
        // Same literal, unconstrained position: no fixed slots at all.
        ("unconstrained", "[handler, 1]", 1, None),
    ];
    for (name, literal, nth, want) in cases {
        let start = source.match_indices(literal).nth(nth).unwrap().0;
        let facts = entities(
            &mut project,
            &path,
            &[EntityDemand {
                location: loc(&path, start, start + literal.len()),
                tuple_shape: true,
                array_shape: true,
                ..EntityDemand::default()
            }],
        );
        assert_eq!(facts[0].tuple_shape, want, "{name} tuple shape");
        // Every one of them is array-shaped, which is precisely why arrayShape
        // alone could not separate them.
        assert_eq!(facts[0].array_shape, Some(ArrayShape::Array), "{name}");
    }
}

#[test]
fn demanded_library_types_resolve_through_spelling() {
    let source = "import { Stamps, Ids } from \"./aliases\";\n\
type LocalWhen = Date;\n\
type Pair = [Date, string];\n\
interface Map<K, V> { fake: K | V }\n\
\n\
declare const written: Date;\n\
declare const aliased: LocalWhen;\n\
declare const imported: Stamps;\n\
declare const setAlias: Ids;\n\
declare const suffixArray: Date[];\n\
declare const genericArray: Array<Date>;\n\
declare const optional: Date | undefined;\n\
declare const mixedUnion: Date | Set<string>;\n\
declare const intersected: Date & { tag: \"t\" };\n\
declare const globalMap: ReadonlyMap<string, number>;\n\
declare const bytes: Uint8Array;\n\
declare const nested: { when: Date };\n\
declare const returnsDate: () => Date;\n\
declare const shadowed: Map<string, number>;\n\
declare const plain: string;\n\
declare const tuple: Pair;\n\
\n\
written;\n\
aliased;\n\
imported;\n\
setAlias;\n\
suffixArray;\n\
genericArray;\n\
optional;\n\
mixedUnion;\n\
intersected;\n\
globalMap;\n\
bytes;\n\
nested;\n\
returnsDate;\n\
shadowed;\n\
plain;\n\
tuple;\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", SHAPES_CONFIG),
        (
            "aliases.ts",
            "export type Stamps = Date[];\nexport type Ids = Set<string>;\n",
        ),
        ("library.ts", source),
    ]);
    let path = scratch.path("library.ts");
    let mut project = scratch.open();
    let cases: [(&str, &[&str]); 16] = [
        // Every spelling of the same runtime type answers the same, including a
        // local alias and one imported from another file.
        ("written", &["Date"]),
        ("aliased", &["Date"]),
        ("imported", &["Array", "Date"]),
        ("setAlias", &["Set"]),
        ("suffixArray", &["Array", "Date"]),
        ("genericArray", &["Array", "Date"]),
        // Union and intersection members are as top-level as the type itself.
        ("optional", &["Date"]),
        ("mixedUnion", &["Date", "Set"]),
        ("intersected", &["Date"]),
        ("globalMap", &["ReadonlyMap"]),
        ("bytes", &["Uint8Array"]),
        // Not top level: an object's property and a function's return type.
        ("nested", &[]),
        ("returnsDate", &[]),
        // A user-declared `Map` shadows the global.
        ("shadowed", &[]),
        ("plain", &[]),
        // A tuple's own symbol is its tuple target, so only its element types
        // contribute.
        ("tuple", &["Date"]),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(expression, _)| EntityDemand {
            location: find_last(&path, source, expression),
            library_types: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((expression, want), fact) in cases.iter().zip(&facts) {
        let got: Vec<&str> = fact
            .library_types
            .as_ref()
            .map(|names| names.iter().map(|name| &**name).collect())
            .unwrap_or_default();
        assert_eq!(&got, want, "{expression} library types");
        assert_eq!(
            fact.library_types.is_some(),
            !want.is_empty(),
            "{expression}: absent exactly when empty"
        );
    }

    let undemanded = entities(
        &mut project,
        &path,
        &[EntityDemand {
            location: demands[0].location.clone(),
            type_descriptor: true,
            ..EntityDemand::default()
        }],
    );
    assert!(undemanded[0].library_types.is_none(), "undemanded");
}

/// ADR 0436: a key set is answered only when the checker resolves it
/// completely. Every case names the expression and either its exact sorted
/// keys or `None` for an open one.
#[test]
fn demanded_declared_keys_answer_only_complete_object_types() {
    let source = "interface Props { class?: string; onClick?: () => void; id: string }\n\
type Plain = { class?: string; style?: string };\n\
type Other = { title?: string };\n\
type Indexed = { class?: string; [key: string]: unknown };\n\
type Templated = { class?: string; [key: `on${string}`]: unknown };\n\
type Hide<T, K extends readonly (keyof T)[]> = { [P in keyof T as Exclude<P, K[number]>]: T[P] };\n\
declare function omit<T extends Record<any, any>, K extends readonly (keyof T)[]>(props: T, ...keys: K): Hide<T, K>;\n\
declare function generic<T extends { class?: string }>(props: T): Hide<T, [\"class\"]>;\n\
declare function keyed<T>(props: T): Hide<T, [never]>;\n\
declare const props: Props;\n\
declare const plain: Plain;\n\
declare const either: Plain | Other;\n\
declare const optional: Plain | undefined;\n\
declare const both: Plain & Other;\n\
declare const indexed: Indexed;\n\
declare const templated: Templated;\n\
declare const loose: any;\n\
declare const unknownValue: unknown;\n\
declare const text: string;\n\
declare const mixed: Plain | string;\n\
declare const openUnion: Plain | Indexed;\n\
declare const nothing: undefined;\n\
declare const record: Record<string, string>;\n\
declare const picked: Pick<Props, \"id\">;\n\
declare const readonlyPlain: Readonly<Plain>;\n\
declare const bare: object;\n\
function body<T extends Plain>(genericP: T, genericQ: Hide<T, [\"class\"]>) { genericP; genericQ; }\n\
function bodyK<K extends readonly (keyof Plain)[]>(genericR: Hide<Plain, K>) { genericR; }\n\
\n\
omit(props, \"class\", \"onClick\");\n\
generic(plain);\n\
keyed(plain);\n\
props;\n\
plain;\n\
either;\n\
optional;\n\
both;\n\
indexed;\n\
templated;\n\
loose;\n\
unknownValue;\n\
text;\n\
mixed;\n\
openUnion;\n\
nothing;\n\
record;\n\
picked;\n\
readonlyPlain;\n\
bare;\n";
    let scratch = Scratch::new(&[("tsconfig.json", SHAPES_CONFIG), ("keys.ts", source)]);
    let path = scratch.path("keys.ts");
    let mut project = scratch.open();
    let cases: [(&str, Option<&[&str]>); 23] = [
        // `omit` of a known interface (the published `as`-clause shape) resolves to the keys that stay.
        ("omit(props, \"class\", \"onClick\")", Some(&["id"])),
        // A generic call instantiated with a concrete argument is concrete.
        ("generic(plain)", Some(&["style"])),
        ("keyed(plain)", Some(&["class", "style"])),
        ("props", Some(&["class", "id", "onClick"])),
        ("plain", Some(&["class", "style"])),
        // A union holds the keys of every member; a nullish member adds none.
        ("either", Some(&["class", "style", "title"])),
        ("optional", Some(&["class", "style"])),
        ("both", Some(&["class", "style", "title"])),
        ("indexed", None),
        ("templated", None),
        ("loose", None),
        ("unknownValue", None),
        ("text", None),
        ("mixed", None),
        ("openUnion", None),
        ("nothing", None),
        ("record", None),
        ("picked", Some(&["id"])),
        ("readonlyPlain", Some(&["class", "style"])),
        ("bare", None),
        // Inside a generic body the key set is the instantiation's, unknown.
        ("genericP", None),
        ("genericQ", None),
        // The `as` clause names its keys through a type variable.
        ("genericR", None),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(expression, _)| EntityDemand {
            location: find_last(&path, source, expression),
            declared_keys: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((expression, want), fact) in cases.iter().zip(&facts) {
        let got: Option<Vec<&str>> = fact
            .declared_keys
            .as_ref()
            .map(|names| names.iter().map(|name| &**name).collect());
        assert_eq!(
            got.as_deref(),
            want.as_deref(),
            "{expression} declared keys"
        );
    }

    let undemanded = entities(
        &mut project,
        &path,
        &[EntityDemand {
            location: demands[0].location.clone(),
            type_descriptor: true,
            ..EntityDemand::default()
        }],
    );
    assert!(undemanded[0].declared_keys.is_none(), "undemanded");
}

/// ADR 0461: a property access names a writable optional property only when
/// its resolved property symbol is optional and not readonly, the condition
/// under which `delete` of it raises neither TS2790 nor TS2704.
#[test]
fn demanded_writable_optional_property_answers_only_optional_writable_properties() {
    let source = "type State = { opt?: number; req: number; undef: number | undefined; readonly ro?: number; method?(): void; nested: { inner?: string } };\n\
declare const s: State;\n\
declare const partial: Partial<{ a: number }>;\n\
declare const required: Required<{ a?: number }>;\n\
declare const frozen: Readonly<{ a?: number }>;\n\
declare const either: { a?: number } | { a: string };\n\
declare const record: Record<string, number>;\n\
declare const loose: any;\n\
delete s.opt;\n\
delete s.req;\n\
delete s.undef;\n\
delete s.ro;\n\
delete s.method;\n\
delete s.nested.inner;\n\
delete s[\"opt\"];\n\
delete partial.a;\n\
delete required.a;\n\
delete frozen.a;\n\
delete either.a;\n\
delete record.k;\n\
delete loose.k;\n";
    let scratch = Scratch::new(&[("tsconfig.json", SHAPES_CONFIG), ("deletes.ts", source)]);
    let path = scratch.path("deletes.ts");
    let mut project = scratch.open();
    let cases: [(&str, bool); 13] = [
        ("s.opt", true),
        ("s.req", false),
        // Its type includes `undefined`, but it is not optional: TS2790 under
        // `exactOptionalPropertyTypes`.
        ("s.undef", false),
        ("s.ro", false),
        ("s.method", false),
        ("s.nested.inner", true),
        // An element access is not answered.
        ("s[\"opt\"]", false),
        ("partial.a", true),
        ("required.a", false),
        ("frozen.a", false),
        // A union property is optional when any constituent's is.
        ("either.a", true),
        ("record.k", false),
        ("loose.k", false),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(expression, _)| EntityDemand {
            location: find_last(&path, source, expression),
            writable_optional_property: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((expression, want), fact) in cases.iter().zip(&facts) {
        assert_eq!(
            fact.writable_optional_property, *want,
            "{expression} writable optional property"
        );
    }

    let undemanded = entities(
        &mut project,
        &path,
        &[EntityDemand {
            location: demands[0].location.clone(),
            type_descriptor: true,
            ..EntityDemand::default()
        }],
    );
    assert!(!undemanded[0].writable_optional_property, "undemanded");
}

/// Go writes an internal library symbol name (`\xFEtype`) as a raw byte that
/// clients read back as U+FFFD; the Rust producer spells what clients read.
#[test]
fn internal_library_symbol_names_are_spelled_as_clients_decode_go() {
    use crate::type_facts::library_symbol_name;
    assert_eq!(library_symbol_name("\u{7f}type"), "\u{fffd}type");
    assert_eq!(library_symbol_name("Date"), "Date");
    assert_eq!(
        library_symbol_name("\u{7f}@iterator@16"),
        library_symbol_name("\u{7f}@iterator@42")
    );
    assert_eq!(
        library_symbol_name("\u{7f}@iterator@16"),
        "\u{fffd}@iterator"
    );
}
