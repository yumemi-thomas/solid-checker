//! Ports `tsgo/runtime_binding_kinds_test.go` and the reference-space and
//! runtime-identity cases of `tsgo/compiler_facts_test.go`
//! (`TestReferenceSpaceAndCanonicalRuntimeIdentity`,
//! `TestReferenceSpaceClassifiesQualifiedTypeNames`).

use super::project_support::{Scratch, change, entities, find, find_last};
use crate::backend::ClosureBackend;
use crate::context::Context;
use typefacts::v3::EntityDemand;
use typefacts::{ReferenceSpace, RuntimeBindingKind};

const ALLOW_JS: &str = r#"{"compilerOptions":{"allowJs":true,"checkJs":true,"module":"esnext","target":"esnext"},"include":["*.js"]}"#;

fn binding_demand(location: typefacts::Location) -> EntityDemand {
    EntityDemand {
        location,
        callability: true,
        constructability: true,
        runtime_identity: true,
        declared_keys: false,
        writable_optional_property: false,
        ..EntityDemand::default()
    }
}

#[test]
fn runtime_binding_kind_censuses_initializers_and_direct_writes() {
    let source = "export var EnumLike;\n\
(function (EnumLike) { EnumLike[\"A\"] = \"a\"; })(EnumLike || (EnumLike = {}));\n\
export var Alias = EnumLike;\n\
export var IIFEClass = (() => { class Inner {}; return Inner; })();\n\
export var Mixed = {};\n\
if (globalThis.flag) Mixed = () => {};\n\
export var Open = globalThis.makeUnknown();\n\
export var PropertyOnly = {};\n\
PropertyOnly.callback = () => {};\n\
export var Brave = !!globalThis.host && globalThis.host.fn && globalThis.host.fn.name === \"fn\";\n";
    let scratch = Scratch::new(&[("tsconfig.json", ALLOW_JS), ("artifact.js", source)]);
    let path = scratch.path("artifact.js");
    let mut project = scratch.open();
    let cases = [
        ("EnumLike", RuntimeBindingKind::NonCallable),
        ("Alias", RuntimeBindingKind::NonCallable),
        ("IIFEClass", RuntimeBindingKind::Callable),
        ("Mixed", RuntimeBindingKind::Mixed),
        ("Open", RuntimeBindingKind::Open),
        ("PropertyOnly", RuntimeBindingKind::NonCallable),
        ("Brave", RuntimeBindingKind::NonCallable),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(name, _)| binding_demand(find(&path, source, name)))
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((name, want), fact) in cases.iter().zip(&facts) {
        assert_eq!(
            fact.runtime_binding_kind,
            Some(*want),
            "{name} runtime binding kind"
        );
    }
}

#[test]
fn runtime_binding_kind_appears_only_for_runtime_identity_demand() {
    let source = "var EnumLike;\n(function (E) { E.a = 1; })(EnumLike || (EnumLike = {}));\nexport { EnumLike };\n";
    let scratch = Scratch::new(&[("tsconfig.json", ALLOW_JS), ("artifact.js", source)]);
    let path = scratch.path("artifact.js");
    let mut project = scratch.open();
    let mut demand = binding_demand(find_last(&path, source, "EnumLike"));
    let facts = entities(&mut project, &path, std::slice::from_ref(&demand));
    assert_eq!(
        facts[0].runtime_binding_kind,
        Some(RuntimeBindingKind::NonCallable)
    );
    demand.runtime_identity = false;
    let facts = entities(&mut project, &path, &[demand]);
    assert_eq!(facts[0].runtime_binding_kind, None, "undemanded");
}

#[test]
fn runtime_binding_kind_follows_exact_relative_runtime_import_past_declaration_substitution() {
    let index = "import Func from \"./func.mjs\";\nexport { Func };\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"allowJs":true,"checkJs":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"files":["index.mjs","func.mjs","func.d.mts"]}"#,
        ),
        ("index.mjs", index),
        ("func.mjs", "export default function Func() {}\n"),
        // TypeScript resolves the explicit .mjs import to this declaration.
        // Its any is deliberately not runtime-kind evidence.
        (
            "func.d.mts",
            "declare const Func: any;\nexport default Func;\n",
        ),
    ]);
    let path = scratch.path("index.mjs");
    let mut project = scratch.open();
    let facts = entities(
        &mut project,
        &path,
        &[binding_demand(find_last(&path, index, "Func"))],
    );
    assert_eq!(
        facts[0].runtime_binding_kind,
        Some(RuntimeBindingKind::Callable),
        "callable runtime function"
    );
}

#[test]
fn runtime_binding_kind_censuses_runtime_export_when_sibling_declaration_closes_signatures() {
    let source = "export const published = () => {};\nexport const value = { kind: 'value' };\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"allowJs":true,"checkJs":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"files":["index.js","index.d.ts"]}"#,
        ),
        ("index.js", source),
        (
            "index.d.ts",
            "export declare const published: () => void;\nexport declare const value: { kind: string };\n",
        ),
    ]);
    let path = scratch.path("index.js");
    let mut project = scratch.open();
    let facts = entities(
        &mut project,
        &path,
        &[binding_demand(find(&path, source, "published"))],
    );
    assert_eq!(
        facts[0].runtime_binding_kind,
        Some(RuntimeBindingKind::Callable),
        "callable runtime export"
    );
}

#[test]
fn reference_space_and_canonical_runtime_identity() {
    let consumer = "import { JSX, Portal, Portal as Unused, Both } from \"./runtime\";\n\
import type { JSX as TypeOnlyJSX } from \"./runtime\";\n\
import { Shared } from \"./runtime\";\n\
import { NamedPortal } from \"./star\";\n\
import { LocalPortal } from \"./local\";\n\
import { SubpathPortal } from \"runtime-package/subpath\";\n\
type Element = TypeOnlyJSX;\n\
Portal();\n\
type BothType = Both;\n\
new Both();\n\
NamedPortal();\n\
LocalPortal();\n\
SubpathPortal();\n\
type SharedType = Shared;\n\
Shared();\n";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}"#,
        ),
        (
            "runtime.ts",
            "export interface JSX { node: unknown }\nexport function Portal() {}\nexport class Both {}\nexport type Shared = { typeOnly: true };\nexport const Shared = () => 1;\n",
        ),
        (
            "named.ts",
            "export { Portal as NamedPortal } from \"./runtime\";\n",
        ),
        ("star.ts", "export * from \"./named\";\n"),
        (
            "local.ts",
            "import { Portal } from \"./runtime\";\nexport { Portal as LocalPortal };\n",
        ),
        (
            "node_modules/runtime-package/package.json",
            r#"{"name":"runtime-package","exports":{"./subpath":"./subpath.d.ts"}}"#,
        ),
        (
            "node_modules/runtime-package/subpath.d.ts",
            "export { Portal as SubpathPortal } from \"../../runtime\";",
        ),
        ("consumer.ts", consumer),
    ]);
    let path = scratch.path("consumer.ts");
    let mut project = scratch.open();
    let names = [
        "JSX",
        "Portal",
        "Both",
        "NamedPortal",
        "LocalPortal",
        "SubpathPortal",
        "TypeOnlyJSX",
        "Unused",
        "Shared",
    ];
    let demands: Vec<EntityDemand> = names
        .iter()
        .map(|name| EntityDemand {
            location: find(&path, consumer, name),
            symbol: true,
            reference_space: true,
            runtime_identity: true,
            declared_keys: false,
            writable_optional_property: false,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    let want_spaces = [
        ReferenceSpace::Neither,
        ReferenceSpace::Value,
        ReferenceSpace::Both,
        ReferenceSpace::Value,
        ReferenceSpace::Value,
        ReferenceSpace::Value,
        ReferenceSpace::Type,
        ReferenceSpace::Neither,
        ReferenceSpace::Both,
    ];
    for (index, want) in want_spaces.iter().enumerate() {
        assert_eq!(
            facts[index].reference_space,
            Some(*want),
            "{} reference space",
            names[index]
        );
    }
    assert!(
        facts[0].runtime_identity.is_empty(),
        "type-only JSX runtime identity = {:?}",
        facts[0].runtime_identity
    );
    assert!(!facts[1].runtime_identity.is_empty(), "Portal identity");
    assert_eq!(
        facts[3].runtime_identity, facts[1].runtime_identity,
        "reexport identity"
    );
    assert!(
        facts[6].runtime_identity.is_empty(),
        "type-only import runtime identity = {:?}",
        facts[6].runtime_identity
    );
    for index in [4, 5, 7] {
        assert_eq!(
            facts[index].runtime_identity, facts[1].runtime_identity,
            "alias {} identity",
            names[index]
        );
    }
    assert!(
        !facts[8].runtime_identity.is_empty(),
        "merged type/value symbol has no runtime identity"
    );
    assert!(facts[1].runtime_identity.starts_with("runtime:h:"));
}

#[test]
fn reference_space_classifies_qualified_type_names() {
    let runtime = "\n\
\t\texport namespace JSX {\n\
\t\t\texport interface CSSProperties {\n\
\t\t\t\tcolor?: string;\n\
\t\t\t}\n\
\t\t\texport interface Element {\n\
\t\t\t\tnode: unknown;\n\
\t\t\t}\n\
\t\t}\n\
\n\
\t\texport function Portal() {}\n\
\n\
\t\texport namespace Namespace {\n\
\t\t\texport namespace Type {\n\
\t\t\t\texport interface Member {\n\
\t\t\t\t\tvalue: unknown;\n\
\t\t\t\t}\n\
\t\t\t}\n\
\t\t}\n\
\n\
\t\texport namespace Shared {\n\
\t\t\texport interface Type {\n\
\t\t\t\tvalue: unknown;\n\
\t\t\t}\n\
\t\t\texport const runtime = 1;\n\
\t\t}\n\
\t";
    let source = "\n\
\t\timport { JSX, Portal, Namespace, Shared } from \"./runtime\";\n\
\n\
\t\ttype Style = JSX.CSSProperties;\n\
\t\ttype Element = JSX.Element;\n\
\t\ttype Deep = Namespace.Type.Member;\n\
\t\ttype SharedType = Shared.Type;\n\
\n\
\t\tShared.runtime;\n\
\t\tPortal();\n\
\t";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","strict":true},"include":["*.ts"]}"#,
        ),
        ("runtime.ts", runtime),
        ("consumer.ts", source),
    ]);
    let path = scratch.path("consumer.ts");
    let mut project = scratch.open();
    let cases = [
        ("JSX", ReferenceSpace::Type),
        ("Portal", ReferenceSpace::Value),
        ("Namespace", ReferenceSpace::Type),
        ("Shared", ReferenceSpace::Both),
    ];
    let demands: Vec<EntityDemand> = cases
        .iter()
        .map(|(name, _)| EntityDemand {
            location: find(&path, source, name),
            symbol: true,
            reference_space: true,
            ..EntityDemand::default()
        })
        .collect();
    let facts = entities(&mut project, &path, &demands);
    assert_eq!(facts.len(), cases.len());
    for ((name, want), fact) in cases.iter().zip(&facts) {
        assert_eq!(fact.reference_space, Some(*want), "{name} referenceSpace");
    }
}

/// The write index answers every binding from one walk of its declaring file;
/// the reference answers each from its own walk. They must agree on every write
/// form Go counts (`=`, `||=`/`&&=`/`??=`, arithmetic compound, `++`/`--`
/// prefix and postfix, destructuring, `for-in`/`for-of` targets, parenthesized
/// targets, writes in nested closures) and on what it ignores (a shadowing
/// binding's writes, property writes, `delete`, writes in other files).
#[test]
fn write_index_agrees_with_the_per_binding_walk_on_every_write_form() {
    use crate::nodes::node_at;
    use crate::runtime_facts::{runtime_binding_kind, runtime_binding_kind_reference};
    let source = "export var Plain = {};\n\
export let Reassigned = {}; Reassigned = () => {};\n\
export let Compound = () => {}; Compound += 1;\n\
export let Increment = () => {}; Increment++;\n\
export let Decrement = () => {}; --Decrement;\n\
export let LogicalOr = {}; LogicalOr ||= () => {};\n\
export let LogicalAnd = () => {}; LogicalAnd &&= {};\n\
export let Coalesce = {}; Coalesce ??= () => {};\n\
export let Destructured = () => {}; ({ Destructured } = globalThis.o);\n\
export let ArrayTarget = () => {}; [ArrayTarget] = globalThis.a;\n\
export let Renamed = () => {}; ({ key: Renamed } = globalThis.o);\n\
export let ForOfTarget = () => {}; for (ForOfTarget of globalThis.xs) {}\n\
export let ForInTarget = () => {}; for (ForInTarget in globalThis.o) {}\n\
export let ForOfArray = () => {}; for ([ForOfArray] of globalThis.xs) {}\n\
export let Deleted = {}; delete Deleted.x;\n\
export let Prop = {}; Prop.callback = () => {};\n\
export let Shadowed = {}; function shadow(Shadowed) { Shadowed = () => {}; }\n\
export let Nested = {}; function nested() { Nested = () => {}; }\n\
export let Arrow = {}; const arrow = () => { Arrow = 1; };\n\
export let Chain = {}; let other; other = Chain = () => {};\n\
export let Same = () => {}; Same = function () {};\n\
export let Cycle1 = 0; export let Cycle2 = 0; Cycle1 = Cycle2; Cycle2 = Cycle1;\n\
export let SelfRef = {}; SelfRef = SelfRef;\n\
export let Many = {}; Many = 1; Many = () => {}; Many = \"s\";\n\
export let Paren = {}; (Paren) = () => {};\n\
export let Exponent = 2; Exponent **= 2; Exponent >>>= 1;\n\
export let Comma = {}; Comma = (0, () => {});\n\
export let Alias = Reassigned; export let AliasOfOpen = Destructured;\n\
export function Fn() {} Fn = 1;\n\
export class Cls {}\n\
export const Konst = () => {};\n";
    let other = "export let Plain = () => {}; export let Reassigned = {}; Plain = {};\n";
    let scratch = Scratch::new(&[
        ("tsconfig.json", ALLOW_JS),
        ("artifact.js", source),
        ("other.js", other),
    ]);
    let path = scratch.path("artifact.js");
    let other_path = scratch.path("other.js");
    let names: Vec<&str> = source
        .lines()
        .flat_map(|line| line.split("export ").skip(1))
        .filter_map(|part| {
            part.strip_prefix("var ")
                .or_else(|| part.strip_prefix("let "))
                .or_else(|| part.strip_prefix("const "))
                .or_else(|| part.strip_prefix("function "))
                .or_else(|| part.strip_prefix("class "))
        })
        .map(|rest| rest.split(|c: char| !c.is_alphanumeric()).next().unwrap())
        .collect();
    assert!(names.len() >= 30, "{names:?}");
    let mut locations: Vec<(String, String, typefacts::Location)> = names
        .iter()
        .map(|name| ((*name).to_owned(), path.clone(), find(&path, source, name)))
        .collect();
    for name in ["Plain", "Reassigned"] {
        locations.push((
            format!("other.{name}"),
            other_path.clone(),
            find(&other_path, other, name),
        ));
    }
    let mut project = scratch.open();
    // Index first (it is built on the first binding), reference second, then
    // both again in the opposite order: neither may depend on the other's
    // checker side effects.
    let answers = project
        .with_env(|env, _| {
            let mut answers = Vec::new();
            for round in 0..2 {
                for (name, file_path, location) in &locations {
                    let file = env.program.get_source_file(file_path).unwrap();
                    let node = node_at(file, u32::try_from(location.start_byte).unwrap());
                    let (indexed, walked) = if round == 0 {
                        let indexed = runtime_binding_kind(env, node);
                        (indexed, runtime_binding_kind_reference(env, node))
                    } else {
                        let walked = runtime_binding_kind_reference(env, node);
                        (runtime_binding_kind(env, node), walked)
                    };
                    assert_eq!(indexed, walked, "{name}, round {round}");
                    answers.push((name.clone(), indexed));
                }
            }
            answers
        })
        .unwrap();
    let kind = |name: &str| answers.iter().find(|(n, _)| n == name).unwrap().1;
    use RuntimeBindingKind::{Callable, Mixed, NonCallable, Open};
    for (name, want) in [
        ("Plain", NonCallable),
        ("Reassigned", Mixed),
        ("Compound", Mixed),
        ("Increment", Mixed),
        ("Decrement", Mixed),
        ("LogicalOr", Mixed),
        ("LogicalAnd", Mixed),
        ("Coalesce", Mixed),
        // A shorthand target names the property it declares, so `get_symbol_at_location`
        // answers the property symbol and the write is not counted (as in Go).
        ("Destructured", Callable),
        ("ArrayTarget", Open),
        ("Renamed", Open),
        ("ForOfTarget", Open),
        ("ForInTarget", Open),
        ("ForOfArray", Open),
        ("Deleted", NonCallable),
        ("Prop", NonCallable),
        ("Shadowed", NonCallable),
        ("Nested", Mixed),
        ("Arrow", NonCallable),
        ("Same", Callable),
        ("Fn", Mixed),
        ("Cls", Callable),
        ("other.Plain", Mixed),
        ("other.Reassigned", NonCallable),
    ] {
        assert_eq!(kind(name), Some(want), "{name}");
    }
}

/// The write index is generation state: an edit that adds, moves or removes a
/// write must be answered from a fresh index, never the previous generation's
/// (a freed tsrs address is reused, so a stale index would answer for the wrong
/// symbol or node).
#[test]
fn write_index_does_not_survive_an_update() {
    let plain = "export let Value = {};\nexport let Other = {};\n";
    let written = "export let Other = {};\nexport let Value = {};\nValue = () => {};\n";
    let moved =
        "// moved\n\nexport let Value = () => {};\nOther = () => {}; export let Other = {};\n";
    let scratch = Scratch::new(&[("tsconfig.json", ALLOW_JS), ("artifact.js", plain)]);
    let path = scratch.path("artifact.js");
    let mut project = scratch.open();
    let kind = |project: &mut crate::project::Project, source: &str, name: &str| {
        let demand = binding_demand(find(&path, source, name));
        entities(project, &path, &[demand])[0].runtime_binding_kind
    };
    assert_eq!(
        kind(&mut project, plain, "Value"),
        Some(RuntimeBindingKind::NonCallable)
    );
    project
        .update(&Context::new(), &[change(&path, 1, written)])
        .unwrap();
    assert_eq!(
        kind(&mut project, written, "Value"),
        Some(RuntimeBindingKind::Mixed)
    );
    assert_eq!(
        kind(&mut project, written, "Other"),
        Some(RuntimeBindingKind::NonCallable)
    );
    project
        .update(&Context::new(), &[change(&path, 2, moved)])
        .unwrap();
    assert_eq!(
        kind(&mut project, moved, "Value"),
        Some(RuntimeBindingKind::Callable)
    );
    assert_eq!(
        kind(&mut project, moved, "Other"),
        Some(RuntimeBindingKind::Mixed)
    );
    project
        .update(&Context::new(), &[change(&path, 3, plain)])
        .unwrap();
    assert_eq!(
        kind(&mut project, plain, "Value"),
        Some(RuntimeBindingKind::NonCallable)
    );
}
