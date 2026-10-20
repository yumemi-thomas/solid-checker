use super::{ParameterValue, SiteStanding, argument_site_standings};
use crate::core::Span;
use std::path::Path;

fn standings(
    source: &str,
    function: &str,
    values: &[ParameterValue],
    sites: &[&str],
) -> Vec<SiteStanding> {
    let start = source
        .find(&format!("function {function}"))
        .expect("function");
    let end = source[start..]
        .find("\n}")
        .map(|offset| start + offset + 2)
        .expect("end");
    let spans = sites
        .iter()
        .map(|site| {
            let at = source[start..end].find(site).expect("site") + start;
            Span::new(at as u32, (at + site.len()) as u32)
        })
        .collect::<Vec<_>>();
    argument_site_standings(
        Path::new("test.ts"),
        source,
        Span::new(start as u32, end as u32),
        values,
        &[],
        &spans,
    )
    .expect("decides")
}

fn number(index: usize) -> ParameterValue {
    ParameterValue {
        index,
        type_of: Some("number"),
        truthy: Some(true),
        nullish: Some(false),
        undefined: Some(false),
        length: None,
    }
}

fn function(index: usize) -> ParameterValue {
    ParameterValue {
        index,
        type_of: Some("function"),
        truthy: Some(true),
        nullish: Some(false),
        undefined: Some(false),
        length: None,
    }
}

const TIMER: &str = "declare function onCleanup(f: () => void): void;\n\
declare function createEffect(a: () => unknown, b: () => void): void;\n\
function createTimer(fn: () => void, delay: number | (() => number)) {\n  if (typeof delay === \"number\") {\n    onCleanup(() => fn());\n    return;\n  }\n  createEffect(() => delay(), () => fn());\n}\n";

#[test]
fn a_number_takes_the_cleanup_arm_and_skips_the_effect() {
    assert_eq!(
        standings(
            TIMER,
            "createTimer",
            &[number(1)],
            &[
                "onCleanup(() => fn())",
                "createEffect(() => delay(), () => fn())"
            ]
        ),
        [SiteStanding::Guaranteed, SiteStanding::Unreachable]
    );
    assert_eq!(
        standings(
            TIMER,
            "createTimer",
            &[function(1)],
            &[
                "onCleanup(() => fn())",
                "createEffect(() => delay(), () => fn())"
            ]
        ),
        [SiteStanding::Unreachable, SiteStanding::Guaranteed]
    );
    assert_eq!(
        standings(
            TIMER,
            "createTimer",
            &[],
            &[
                "onCleanup(() => fn())",
                "createEffect(() => delay(), () => fn())"
            ]
        ),
        [SiteStanding::Possible, SiteStanding::Possible]
    );
}

/// Review round 4: defaults, redeclarations, optional chains and exits
/// that may run first.
#[test]
fn review_round_four_refusals() {
    let source = "declare function readA(): number;\ndeclare function fail(m: string): never;\n\
declare const sink: { put(x: number): void } | undefined;\n\
function defaulted(flag = true) {\n  if (flag) readA();\n}\n\
function redeclared(flag: boolean) {\n  var flag = true;\n  if (flag) readA();\n}\n\
function chained(flag: boolean) {\n  if (flag) return;\n  sink?.put(readA());\n}\n\
function exitFirst(ok: boolean, n = fail(\"x\")) {\n  if (ok) return;\n  readA();\n}\n";
    let truthy = ParameterValue {
        index: 0,
        truthy: Some(true),
        ..ParameterValue::default()
    };
    let falsy = ParameterValue {
        index: 0,
        truthy: Some(false),
        ..ParameterValue::default()
    };
    // `f(undefined)` takes the default, so the omitted value decides nothing.
    assert_eq!(
        standings(
            source,
            "defaulted",
            &[ParameterValue::omitted(0)],
            &["readA()"]
        ),
        [SiteStanding::Possible]
    );
    assert_eq!(
        standings(
            source,
            "redeclared",
            std::slice::from_ref(&falsy),
            &["readA()"]
        ),
        [SiteStanding::Possible]
    );
    assert_eq!(
        standings(
            source,
            "chained",
            std::slice::from_ref(&falsy),
            &["readA()"]
        ),
        [SiteStanding::Possible]
    );
    assert_eq!(
        standings(source, "chained", &[truthy], &["readA()"]),
        [SiteStanding::Unreachable]
    );
    let start = source.find("function exitFirst").unwrap();
    let end = source[start..].find("\n}").unwrap() + start + 2;
    let at = |text: &str| {
        let offset = source[start..end].find(text).unwrap() + start;
        Span::new(offset as u32, (offset + text.len()) as u32)
    };
    assert_eq!(
        argument_site_standings(
            Path::new("test.ts"),
            source,
            Span::new(start as u32, end as u32),
            &[falsy],
            &[at("fail(\"x\")")],
            &[at("readA()")],
        ),
        Some(vec![SiteStanding::Possible]),
        "a `never` default may throw before the body"
    );
}

#[test]
fn truthiness_nullishness_and_refusals() {
    let source = "declare function readA(): number;\ndeclare function readB(): number;\n\
function when(flag: boolean, value?: number) {\n  if (!flag) return 0;\n  const fallback = value ?? readA();\n  return flag && readB();\n}\n\
function written(flag: boolean) {\n  flag = !flag;\n  if (flag) readA();\n}\n\
function aliased(flag: boolean) {\n  if (arguments[0]) readA();\n}\n";
    let truthy = ParameterValue {
        index: 0,
        truthy: Some(true),
        ..ParameterValue::default()
    };
    let falsy = ParameterValue {
        index: 0,
        truthy: Some(false),
        ..ParameterValue::default()
    };
    assert_eq!(
        standings(
            source,
            "when",
            &[truthy.clone(), ParameterValue::omitted(1)],
            &["readA()", "readB()"]
        ),
        [SiteStanding::Guaranteed, SiteStanding::Guaranteed]
    );
    assert_eq!(
        standings(source, "when", &[falsy], &["readA()", "readB()"]),
        [SiteStanding::Unreachable, SiteStanding::Unreachable]
    );
    // Without values the same sites are only possible.
    assert_eq!(
        standings(source, "when", &[], &["readA()", "readB()"]),
        [SiteStanding::Possible, SiteStanding::Possible]
    );
    // A written parameter decides nothing.
    assert_eq!(
        standings(source, "written", &[truthy], &["readA()"]),
        [SiteStanding::Possible]
    );
    let start = source.find("function aliased").unwrap();
    let end = source[start..].find("\n}").unwrap() + start + 2;
    assert!(
        argument_site_standings(
            Path::new("test.ts"),
            source,
            Span::new(start as u32, end as u32),
            &[],
            &[],
            &[]
        )
        .is_none(),
        "`arguments` refuses"
    );
}

fn getter(index: usize, length: u32) -> ParameterValue {
    ParameterValue {
        length: Some(length),
        ..function(index)
    }
}

/// `@solid-primitives/utils` `access`: `typeof v === "function" && !v.length
/// ? v() : v`. A zero-length function is called on every path; a function
/// with a parameter, or a value of unknown arity, is not decided.
#[test]
fn a_function_of_known_length_decides_a_length_test() {
    let source = "declare function readIt(): number;\n\
function access(v: any) {\n  return typeof v === \"function\" && !v.length ? readIt() : v;\n}\n\
function exact(v: any) {\n  return v.length === 0 ? readIt() : 0;\n}\n\
function reversed(v: any) {\n  return 1 > v.length ? readIt() : 0;\n}\n\
function relational(v: any) {\n  return v.length >= 1 ? 0 : readIt();\n}\n\
function optional(v: any) {\n  return v?.length === 0 ? readIt() : 0;\n}\n";
    let site = ["readIt()"];
    let decided = |name: &str, value: ParameterValue| standings(source, name, &[value], &site);
    assert_eq!(decided("access", getter(0, 0)), [SiteStanding::Guaranteed]);
    assert_eq!(decided("access", getter(0, 1)), [SiteStanding::Unreachable]);
    assert_eq!(decided("access", function(0)), [SiteStanding::Possible]);
    assert_eq!(decided("access", number(0)), [SiteStanding::Unreachable]);
    assert_eq!(decided("exact", getter(0, 0)), [SiteStanding::Guaranteed]);
    assert_eq!(decided("exact", getter(0, 2)), [SiteStanding::Unreachable]);
    assert_eq!(
        decided("reversed", getter(0, 0)),
        [SiteStanding::Guaranteed]
    );
    assert_eq!(
        decided("reversed", getter(0, 1)),
        [SiteStanding::Unreachable]
    );
    assert_eq!(
        decided("relational", getter(0, 0)),
        [SiteStanding::Guaranteed]
    );
    assert_eq!(
        decided("relational", getter(0, 3)),
        [SiteStanding::Unreachable]
    );
    // A length is only a function's: a number's `.length` is not read here.
    let mut numeric = number(0);
    numeric.length = Some(0);
    assert_eq!(decided("exact", numeric), [SiteStanding::Possible]);
    // An optional chain is a different expression and decides nothing.
    assert_eq!(decided("optional", getter(0, 0)), [SiteStanding::Possible]);
}

/// A `typeof p` or a `p.length` test cannot run `p`; a call, an alias or an
/// argument may.
#[test]
fn only_a_type_or_length_test_is_an_inert_parameter_reference() {
    use super::inert_parameter_references;
    let source = "function tests(v: any) { return typeof v === \"string\" && v.length > 0 ? 1 : 0; }\n\
function calls(v: any) { return v.length ? v() : 0; }\n\
function aliases(v: any) { const w = v; return typeof v === \"function\" ? 0 : w; }\n\
function member(v: any) { return v.length(); }\n";
    let inert = |name: &str| {
        let start = source.find(&format!("function {name}")).expect("function");
        let end = start + source[start..].find("}\n").expect("end") + 1;
        inert_parameter_references(
            Path::new("test.ts"),
            source,
            Span::new(start as u32, end as u32),
            0,
        )
        .expect("parses")
        .len()
    };
    assert_eq!(inert("tests"), 2);
    // `v.length` is inert and `v()` is not.
    assert_eq!(inert("calls"), 1);
    assert_eq!(inert("aliases"), 1);
    assert_eq!(inert("member"), 0);
}
