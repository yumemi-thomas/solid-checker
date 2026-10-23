use crate::ast::extract;

fn keys(source: &str, export: &str) -> Option<Vec<String>> {
    let facts = extract("package.mjs", source).expect("parses");
    let function = facts.exported_function(export)?;
    facts
        .returned_object_keys(source, function)
        .map(|keys| keys.into_iter().map(String::from).collect())
}

#[test]
fn a_builder_const_holds_its_literal_and_written_keys() {
    // The shape of `@stylexjs/stylex@0.19.0`'s `attrs` (`lib/es/stylex.mjs`).
    let source = r#"
function props(...styles) { return { className: styles[0] }; }
function attrs(...styles) {
  const { className, style } = props.apply(this, styles);
  const result = {};
  if (className != null) {
    result.class = className;
  }
  if (style != null) {
    result.style = Object.entries(style).map(([k, v]) => `${k}:${v}`).join(';');
  }
  result['data-style-src'] = 'x';
  return result;
}
export { attrs };
"#;
    assert_eq!(
        keys(source, "attrs"),
        Some(vec![
            "class".into(),
            "data-style-src".into(),
            "style".into()
        ])
    );
}

#[test]
fn literal_returns_and_local_return_through_calls_read() {
    let source = r#"
const inner = () => ({ class: "a" });
export function outer(flag) {
  if (flag) return { style: "s" };
  if (flag === 2) return;
  return inner();
}
export const arrow = () => ({ id: "x" });
"#;
    assert_eq!(
        keys(source, "outer"),
        Some(vec!["class".into(), "style".into()])
    );
    assert_eq!(keys(source, "arrow"), Some(vec!["id".into()]));
}

#[test]
fn every_open_shape_answers_none() {
    let source = r#"
export function parameter(value) { return value; }
export function spread(value) { return { ...value, class: "a" }; }
export function computed(key) { return { [key]: 1 }; }
export function getter() { return { get onClick() { return () => {}; } }; }
export function computedWrite(key) { const result = {}; result[key] = 1; return result; }
export function handedOut() { const result = {}; Object.assign(result, {}); return result; }
export function captured() { const result = {}; [1].forEach(() => { result.a = 1; }); return result; }
export function proto() { const result = {}; result.__proto__ = {}; return result; }
export function nested() { const result = {}; result.a.b = 1; return result; }
export async function later() { return { a: 1 }; }
export function* generated() { return { a: 1 }; }
export function rewritten() { return { a: 1 }; }
rewritten = () => ({ onClick() {} });
let mutable = () => ({ a: 1 });
export { mutable };
export { other } from "./other.js";
"#;
    for export in [
        "parameter",
        "spread",
        "computed",
        "getter",
        "computedWrite",
        "handedOut",
        "captured",
        "proto",
        "nested",
        "later",
        "generated",
        "rewritten",
        "mutable",
        "other",
        "missing",
    ] {
        assert_eq!(keys(source, export), None, "{export}");
    }
    // The shapes, not the export lookup, are what refuse these.
    let facts = extract("package.mjs", source).expect("parses");
    for export in [
        "parameter",
        "spread",
        "computed",
        "getter",
        "computedWrite",
        "handedOut",
        "captured",
        "proto",
        "nested",
        "later",
        "generated",
    ] {
        assert!(facts.exported_function(export).is_some(), "{export}");
    }
    for export in ["rewritten", "mutable", "other", "missing"] {
        assert!(facts.exported_function(export).is_none(), "{export}");
    }
}

#[test]
fn an_import_binding_or_namespace_member_names_its_export() {
    let source = r#"
import * as styler from "styler";
import { attrs as a, other } from "styler";
import type { T } from "styler";
const local = () => ({});
styler.attrs(1);
a(1);
local();
styler["attrs"](1);
"#;
    let facts = extract("App.tsx", source).expect("parses");
    let named = |callee: &str| {
        let start = source.find(callee).expect("the call") as u32;
        let call = facts
            .calls
            .iter()
            .find(|call| call.span.start == start)
            .expect("a call fact");
        facts
            .imported_callee(source, call.callee)
            .map(|(import, export)| (import.module.to_string(), export.to_string()))
    };
    assert_eq!(
        named("styler.attrs(1)"),
        Some(("styler".into(), "attrs".into()))
    );
    assert_eq!(named("a(1)"), Some(("styler".into(), "attrs".into())));
    assert_eq!(named("local()"), None);
    assert_eq!(named("styler[\"attrs\"](1)"), None);
}
