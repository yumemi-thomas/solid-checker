//! `v1/no-innerhtml`, `v1/no-react-specific-props`, `v1/style-prop`, and
//! `v1/prefer-classlist` —
//! eslint-plugin-solid's attribute-value rules, ported from the 1.x reactive
//! solver's `solid_1_rules.rs` onto this checker's fact tables.
//!
//! # Options
//!
//! `no-innerhtml { allowStatic }`, `style-prop { styleProps, allowString }`,
//! and `prefer-classlist { classnames }` are read from the project's
//! `.solid-checker/rule-options.json` (see [`super::solid1x_options`]),
//! defaulting to upstream's defaults.

use std::collections::HashSet;

use solid_facts::FileFacts;
use solid_facts::ast::JsxElementFact;

use super::{
    UpstreamCompatContext, fix_replace, is_lowercase_led, literal_string_type, static_string, text,
    violation,
};
use crate::StaticViolation;

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    for element in &file.ast.jsx_elements {
        no_innerhtml(file, context, element, violations);
        no_react_specific_props(file, element, violations);
        style_prop(file, context, element, violations);
        prefer_classlist(file, context, element, violations);
    }
}

/// `v1/no-innerhtml` (SC8008) — React's `dangerouslySetInnerHTML`, which
/// Solid does not implement, and Solid's own `innerHTML`/`innerhtml`, which
/// is a real prop but a dangerous one: it should not coexist with JSX
/// children (one silently overwrites the other), a value that is
/// unmistakably plain text belongs in `innerText` instead, and a genuinely
/// dynamic value is a standing XSS risk worth flagging even though nothing
/// here can prove it unsafe.
fn no_innerhtml(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    // The React prop only reaches this rule on a **component**. On an intrinsic
    // element `dangerouslySetInnerHTML` is not in `JSX.IntrinsicElements`, so it
    // is TS2322 ("Property 'dangerouslySetInnerHTML' does not exist on type
    // 'HTMLAttributes<HTMLDivElement>'") against the real solid-js@1.9.14
    // typings -- and that is this arm's own sentence, "the prop is not
    // supported". Narrowed 2026-08-17 after `scripts/parity-tsc-ownership.mjs`
    // matched the two spans and confirmed the claims are the same.
    //
    // A component's props are whatever it declares, so the prop is a permitted
    // key there, TypeScript is silent, and the claim -- Solid's renderer has no
    // special case for the name, so it arrives inert -- is the rule's alone.
    // (The hyphen exemption that reopened three other narrowings does not apply:
    // this name is fixed and carries none.)
    let component = !is_lowercase_led(text(file, element.name.span));
    for attribute in &element.attributes {
        let name = text(file, attribute.name);
        if name == "dangerouslySetInnerHTML" {
            if !component {
                continue;
            }
            let mut result = violation(
                file,
                "SC8008",
                "no-innerhtml",
                "The dangerouslySetInnerHTML prop is not supported; use innerHTML instead.",
                "Solid's DOM renderer has no special case for this React prop name, so it passes through as an inert, unrecognized attribute. Use innerHTML with the same { __html: ... } shape's inner value.",
                attribute.span,
                vec![],
            );
            // Upstream fixes only the exact `{{ __html: value }}` shape: the
            // value must be an object literal whose single own entry is a
            // plain `__html` key. A `__html` nested deeper, or accompanied by
            // other entries, has no unambiguous rewrite.
            if let Some(properties) = attribute
                .expression
                .and_then(|expression| super::direct_object_literal_properties(file, expression))
                && let [only] = properties.as_slice()
                && !only.computed
                && text(file, only.key) == "__html"
            {
                result.fixes.push(fix_replace(
                    file,
                    attribute.span,
                    "rewrite as innerHTML",
                    format!("innerHTML={{{}}}", text(file, only.value)),
                ));
            }
            violations.push(result);
            continue;
        }
        // Exactly `innerHTML`: a lowercase `innerhtml` is an ordinary,
        // unrecognized attribute to Solid's compiler, and upstream leaves it
        // alone (`jsx-no-duplicate-props` still counts both spellings as one
        // content source).
        if name != "innerHTML" {
            continue;
        }
        // Upstream's `allowStatic: false`: every innerHTML value is a
        // reported injection surface, static or not — and the only report;
        // the conflict and not-HTML advice below exist solely on the
        // static-acceptance path.
        if !context.solid1x_options.no_innerhtml.allow_static {
            violations.push(violation(
                file,
                "SC8008",
                "no-innerhtml",
                "The innerHTML attribute is dangerous; passing unsanitized input can lead to security vulnerabilities.",
                "innerHTML injects its value as markup verbatim. This project's rule options reject every innerHTML value; render the content as JSX instead.",
                attribute.span,
                vec![],
            ));
            continue;
        }
        // A value is static when its literal is written here, or when its
        // resolved type is a string-literal type — the same value, proven by
        // TypeScript instead of recovered from this file's text.
        let static_value = attribute
            .value
            .and_then(|span| static_string(file, span))
            .or_else(|| {
                attribute
                    .expression
                    .and_then(|span| literal_string_type(context, file, span))
            });
        match static_value {
            // A static value that is provably markup is accepted — unless
            // the element also has JSX children, in which case the two
            // content sources overwrite each other.
            Some(value) if super::solid1x_upstream_data::is_html(&value) => {
                if !element.children.is_empty() {
                    violations.push(violation(
                        file,
                        "SC8008",
                        "no-innerhtml",
                        "The innerHTML attribute should not be used on an element with child elements; they will be overwritten.",
                        "Remove either the JSX children or the innerHTML prop; whichever renders last wins, and which one that is depends on implementation detail rather than anything visible here.",
                        element.span,
                        vec![],
                    ));
                }
            }
            Some(_) => {
                let mut result = violation(
                    file,
                    "SC8008",
                    "no-innerhtml",
                    "The string passed to innerHTML does not appear to be valid HTML.",
                    "For text content, innerText is clearer and safer: it never parses its argument as markup, so there is no injection surface to reason about.",
                    attribute.span,
                    vec![],
                );
                result.fixes.push(fix_replace(
                    file,
                    attribute.name,
                    "rename to innerText",
                    "innerText",
                ));
                violations.push(result);
            }
            None => violations.push(violation(
                file,
                "SC8008",
                "no-innerhtml",
                "The innerHTML attribute is dangerous; passing unsanitized input can lead to security vulnerabilities.",
                "innerHTML injects its value as markup verbatim. If the value can ever contain user input, sanitize it before it reaches this prop.",
                attribute.span,
                vec![],
            )),
        }
    }
}

/// `v1/no-react-specific-props` (SC8011) — the React prop spellings
/// `className`/`htmlFor`, deprecated in Solid since 1.4, on a **component**.
///
/// Narrowed 2026-08-17 under AGENTS.md's absolute rule. On an intrinsic
/// element `className`, `htmlFor`, and `key` are each individually TS2322
/// ("Property 'className' does not exist on type 'HTMLAttributes<…>'") against
/// the real 1.9.14 JSX typings, so reporting them there duplicated `tsc`. The
/// `key` arm was intrinsic-only and is gone entirely.
///
/// What remains is the arm no type covers, and it is upstream's own cases 4
/// and 8: `<PascalComponent className="greeting">`. A component's props are
/// whatever it declares, so the React spelling is a permitted key on a
/// permissive component — `tsc` is silent — while a component declaring
/// `{ class?: string }` makes it a type error, which is the type system's to
/// report. The rule therefore speaks only where the answer is not already
/// given, and its claim there is the migration one: Solid forwards `class`,
/// not `className`.
fn no_react_specific_props(
    file: &FileFacts,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    // An intrinsic element's attributes are typed by `JSX.IntrinsicElements`,
    // where none of these names exists. Only a component's props can admit
    // them.
    if is_lowercase_led(text(file, element.name.span)) {
        return;
    }
    let names = element
        .attributes
        .iter()
        .map(|attribute| text(file, attribute.name))
        .collect::<HashSet<_>>();
    for (from, to) in [("className", "class"), ("htmlFor", "for")] {
        for attribute in element
            .attributes
            .iter()
            .filter(|attribute| text(file, attribute.name) == from)
        {
            let mut result = violation(
                file,
                "SC8011",
                "no-react-specific-props",
                format!("Prefer the `{to}` prop over the deprecated `{from}` prop."),
                format!(
                    "Rename {from} to {to}; Solid recognizes {from} only for React-migration compatibility and it may be removed."
                ),
                attribute.span,
                vec![],
            );
            // Only auto-fix when there is no existing `to` prop to collide
            // with; renaming into a prop this element already has would
            // create the exact duplicate-prop defect `jsx-no-duplicate-props`
            // exists to catch.
            if !names.contains(to) {
                result.fixes.push(fix_replace(
                    file,
                    attribute.name,
                    format!("rename to {to}"),
                    to,
                ));
            }
            violations.push(result);
        }
    }
}

/// `v1/style-prop` (SC8017) — the `style` prop written as a string (Solid
/// prefers an object, which it can update per-declaration instead of
/// replacing wholesale), a camelCase CSS property name inside that object
/// (Solid's `style` maps directly to `CSSStyleDeclaration`, which is
/// kebab-case, unlike React's synthetic style object), and a bare numeric
/// value for a property where that number is ambiguous (Solid never appends
/// an implicit `px`, unlike React).
fn style_prop(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    // Which props carry styles is upstream's `styleProps` option (default
    // `["style"]`, and naming others *replaces* the default); `allowString`
    // accepts the string form instead of asking for an object.
    let options = &context.solid1x_options.style_prop;
    for attribute in element.attributes.iter().filter(|attribute| {
        let name = text(file, attribute.name);
        options.style_props.iter().any(|prop| prop == name)
    }) {
        if let Some((value_span, value)) = attribute
            .value
            .and_then(|span| static_string(file, span).map(|value| (span, value)))
            .filter(|_| !options.allow_string)
        {
            let mut result = violation(
                file,
                "SC8017",
                "style-prop",
                "Use an object for the style prop instead of a string.",
                "An object lets Solid update individual CSS properties in place; a string is replaced wholesale on every update.",
                value_span,
                vec![],
            );
            if let Some(replacement) = style_string_object_fix(&value) {
                result.fixes.push(fix_replace(
                    file,
                    value_span,
                    "convert to a style object",
                    replacement,
                ));
            }
            violations.push(result);
            continue;
        }
        // The *object-key* checks below are TypeScript's on an intrinsic
        // element, and only there. `JSX.IntrinsicElements` types a `style`
        // object as `csstype`'s `CSSProperties`, and TypeScript's
        // excess-property check applies to exactly the same subject this rule
        // inspects — a fresh object literal written in place — so against the
        // real solid-js@1.9.14 typings a camelCase key is TS2561 with the
        // kebab-case suggestion, an unknown key is TS2353, and a unitless
        // number for a length is TS2322 against `MarginTop<…>`. Narrowed
        // 2026-08-17 under AGENTS.md's absolute rule.
        //
        // On a component the answer is not given: its props are whatever it
        // declares, so `<Panel style={{ fontSize: 10 }} />` is a permitted key
        // when `Panel` takes `Record<string, unknown>` and a type error when it
        // declares `JSX.CSSProperties`. Where TypeScript is silent the key is
        // still wrong the moment the component forwards it to the DOM, so the
        // rule keeps speaking there.
        //
        // The *string-form* check above is unaffected: a string `style` is
        // legal in Solid 1.x on every element, and two of its claims — a
        // declaration with a missing value, and a value that is not CSS at all
        // — are ones no type can make.
        //
        // One key shape survives on an intrinsic element too: `CSSProperties`
        // carries `[key: `-${string}`]: string | number | undefined`, so a
        // vendor-prefixed key is absorbed by the index signature and stays
        // silent whatever it is spelled — verified for `-webkitAlignContent`,
        // `-webkit-align-content`, and `-fooBar`. Upstream's own case 02 is
        // exactly that spelling, so gating the whole object on component-ness
        // would have dropped a finding no type makes.
        let object_keys_are_typed = is_lowercase_led(text(file, element.name.span));
        // Upstream inspects only a style value that is itself an object
        // literal, and only that object's own entries — an object built by a
        // helper call, or nested as some entry's value, is left alone.
        let Some(properties) = attribute
            .expression
            .and_then(|expression| super::direct_object_literal_properties(file, expression))
        else {
            continue;
        };
        for property in properties {
            // Upstream's `getPropertyName` resolves plain and quoted keys;
            // a computed key it cannot fold answers no name, which skips
            // every name-keyed check below — including the missing-unit
            // one, which must know the property is a length to report.
            let name = (!property.computed)
                .then(|| text(file, property.key).trim_matches(['\'', '"']))
                .filter(|name| !name.is_empty());
            // The narrowing, per key rather than per element: on an intrinsic
            // element TypeScript answers for every key its `CSSProperties`
            // describes, and declines only the `-`-prefixed ones its index
            // signature absorbs. A computed key names nothing, so it is also
            // TypeScript's to complain about or not.
            if object_keys_are_typed && !name.is_some_and(|name| name.starts_with('-')) {
                continue;
            }
            match name {
                // Custom properties are CSS's own escape hatch; upstream
                // skips `--` names entirely.
                Some(name) if name.starts_with("--") => {}
                Some(name) if !super::solid1x_upstream_data::is_known_css_property(name) => {
                    let kebab = to_kebab_case(name);
                    if super::solid1x_upstream_data::is_known_css_property(&kebab) {
                        let mut result = violation(
                            file,
                            "SC8017",
                            "style-prop",
                            format!("Use {kebab} instead of {name}."),
                            "Solid's style prop sets CSS properties directly, which are always kebab-case; a camelCase key is simply not a CSS property and has no effect.",
                            property.key,
                            vec![],
                        );
                        result.fixes.push(fix_replace(
                            file,
                            property.key,
                            "convert to kebab-case",
                            format!("\"{kebab}\""),
                        ));
                        violations.push(result);
                    } else {
                        // Not a CSS property under any casing. There is no
                        // rewrite to offer — kebab-casing a typo produces a
                        // different typo — so this reports without a fix,
                        // as upstream's `invalidStyleProp` does.
                        violations.push(violation(
                            file,
                            "SC8017",
                            "style-prop",
                            format!("{name} is not a valid CSS property."),
                            "Check the spelling against the CSS property list; Solid passes style object keys to the DOM as-is.",
                            property.key,
                            vec![],
                        ));
                    }
                }
                // The missing-unit advice applies only to a key that
                // *resolves* to a known length property; [`missing_unit`]
                // carries the reasoning for the keys it declines.
                _ if missing_unit(name, text(file, property.value)) => {
                    violations.push(violation(
                        file,
                        "SC8017",
                        "style-prop",
                        "This CSS property value should be a string with a unit; Solid does not automatically append a \"px\" unit.",
                        "Quote the value and add a unit, e.g. \"10px\"; an unquoted number is passed to the DOM as-is and most length properties reject a unitless value.",
                        property.value,
                        vec![],
                    ));
                }
                _ => {}
            }
        }
    }
}

/// Whether a style-object entry misses its unit: the key must *resolve* to a
/// known length property, and the value must be a bare non-zero number
/// (Solid never appends an implicit `px`, and zero is unit-optional in CSS).
///
/// Everything else declines. A resolved non-length property (`opacity`,
/// `z-index`) legally takes a bare number, so it is no missing unit. A
/// computed key the folder could not resolve (`[key]: 0.5`) names no
/// property at all — whether its number needs a unit depends on which
/// property it lands on at runtime, so reporting would false-positive on the
/// unitless-legal ones. And a `--` custom property is CSS's own escape hatch,
/// skipped even when its name happens to contain a length word
/// (`--max-width`).
fn missing_unit(name: Option<&str>, value: &str) -> bool {
    let Some(name) = name else {
        return false;
    };
    if name.starts_with("--") || !super::solid1x_upstream_data::names_length_property(name) {
        return false;
    }
    value
        .trim()
        .trim_start_matches('-')
        .parse::<f64>()
        .is_ok_and(|value| value != 0.0)
}

fn style_string_object_fix(source: &str) -> Option<String> {
    let declarations = split_css_at_top_level(source, ';')?;
    let mut properties = Vec::new();
    for declaration in declarations {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        let colon = top_level_delimiters(declaration, ':')?.into_iter().next()?;
        let name = declaration[..colon].trim();
        let value = declaration[colon + 1..].trim();
        if name.is_empty() || value.is_empty() {
            return None;
        }
        properties.push(format!(
            "{}:{}",
            serde_json::to_string(name).ok()?,
            serde_json::to_string(value).ok()?
        ));
    }
    Some(format!("{{{{{}}}}}", properties.join(",")))
}

fn split_css_at_top_level(source: &str, delimiter: char) -> Option<Vec<&str>> {
    let delimiters = top_level_delimiters(source, delimiter)?;
    let mut parts = Vec::with_capacity(delimiters.len() + 1);
    let mut start = 0;
    for index in delimiters {
        parts.push(&source[start..index]);
        start = index + delimiter.len_utf8();
    }
    parts.push(&source[start..]);
    Some(parts)
}

fn top_level_delimiters(source: &str, delimiter: char) -> Option<Vec<usize>> {
    let mut quote = None;
    let mut escaped = false;
    let mut brackets = Vec::new();
    let mut delimiters = Vec::new();
    for (index, character) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(open_quote) = quote {
            if character == open_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' | '[' | '{' => brackets.push(character),
            ')' | ']' | '}' => {
                let expected = match character {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => unreachable!(),
                };
                if brackets.pop() != Some(expected) {
                    return None;
                }
            }
            _ if character == delimiter && brackets.is_empty() => delimiters.push(index),
            _ => {}
        }
    }
    (!escaped && quote.is_none() && brackets.is_empty()).then_some(delimiters)
}

fn to_kebab_case(name: &str) -> String {
    let mut kebab = String::new();
    for character in name.chars() {
        if character.is_uppercase() {
            kebab.push('-');
            kebab.extend(character.to_lowercase());
        } else {
            kebab.push(character);
        }
    }
    kebab
}

/// `v1/prefer-classlist` (SC8013) — a `class`/`className` prop set from a
/// `classnames`-shaped helper call (`cn`, `clsx`, `classnames`) applied to an
/// object literal. Solid's `classlist` prop accepts that exact `{ [name]:
/// boolean }` shape natively and updates only the classes whose value
/// changed, where the helper call reconstructs the whole class string on
/// every render.
fn prefer_classlist(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    // Which helper names count is upstream's `classnames` option; the
    // default is upstream's default list.
    let helpers = &context.solid1x_options.prefer_classlist.classnames;
    // Upstream guards on its own lowercase `classlist` spelling only; the
    // prop Solid's compiler special-cases is `classList`, so an element
    // already written that way must count as "already using it" too.
    if element
        .attributes
        .iter()
        .any(|attribute| matches!(text(file, attribute.name), "classlist" | "classList"))
    {
        return;
    }
    for attribute in element
        .attributes
        .iter()
        .filter(|attribute| matches!(text(file, attribute.name), "class" | "className"))
    {
        let Some(expression) = attribute.expression else {
            continue;
        };
        let Some(call) = file
            .ast
            .calls
            .iter()
            .find(|call| call.span == expression && call.arguments.len() == 1)
        else {
            continue;
        };
        let Some(callee) = call.static_callee(&file.source) else {
            continue;
        };
        if !helpers.iter().any(|helper| helper == callee)
            || !text(file, call.arguments[0].span)
                .trim_start()
                .starts_with('{')
        {
            continue;
        }
        let mut result = violation(
            file,
            "SC8013",
            "prefer-classlist",
            format!(
                "The classlist prop should be used instead of {callee} to efficiently set classes based on an object."
            ),
            "classList takes the same { [name]: boolean } object and updates only the classes whose value actually changed, instead of recomputing and reassigning the whole class string.",
            attribute.span,
            vec![],
        );
        // The report matches on the helper's conventional names, as upstream
        // does — but a rewrite is only behaviour-preserving when the callee
        // really is the classnames-style helper, not a local function that
        // happens to share a name. An import from one of the packages is that
        // proof; without it the report stands and the fix is withheld.
        let imported_helper = file.ast.imports.iter().any(|import| {
            matches!(import.module.as_str(), "clsx" | "classnames")
                && import
                    .bindings
                    .iter()
                    .any(|binding| text(file, binding.local.span) == callee)
        });
        if imported_helper {
            // Upstream's fixer writes `classlist=`, a spelling the compiler
            // does not special-case; `classList` is the prop Solid actually
            // handles, so the working rewrite is the one worth offering.
            result.fixes.push(fix_replace(
                file,
                attribute.span,
                "rewrite as classList",
                format!("classList={{{}}}", text(file, call.arguments[0].span)),
            ));
        }
        violations.push(result);
    }
}

#[cfg(test)]
mod tests {
    use super::super::strip_string_literal;
    use super::{is_lowercase_led, missing_unit, style_string_object_fix, to_kebab_case};

    #[test]
    fn converts_camel_case_css_properties_to_kebab_case() {
        assert_eq!(to_kebab_case("fontSize"), "font-size");
        assert_eq!(to_kebab_case("backgroundColor"), "background-color");
        assert_eq!(to_kebab_case("borderTopWidth"), "border-top-width");
    }

    #[test]
    fn leaves_already_kebab_or_lowercase_names_unchanged() {
        assert_eq!(to_kebab_case("font-size"), "font-size");
        assert_eq!(to_kebab_case("color"), "color");
    }

    #[test]
    fn strips_quotes_from_string_literals() {
        assert_eq!(strip_string_literal("\"hello\""), Some("hello".to_string()));
        assert_eq!(strip_string_literal("{ 'hi' }"), Some("hi".to_string()));
        assert_eq!(strip_string_literal("notAString"), None);
    }

    #[test]
    fn classifies_element_names_by_leading_case() {
        assert!(is_lowercase_led("div"));
        assert!(!is_lowercase_led("Component"));
    }

    #[test]
    fn missing_unit_reports_only_resolved_length_properties() {
        // A resolved length property with a bare non-zero number.
        assert!(missing_unit(Some("width"), "10"));
        assert!(missing_unit(Some("margin-top"), "-2.5"));
        // A computed/unresolvable key names no property, so no report —
        // the value could land on a unitless-legal property like opacity.
        assert!(!missing_unit(None, "0.5"));
        // A resolved property where a bare number is legal.
        assert!(!missing_unit(Some("opacity"), "0.5"));
        assert!(!missing_unit(Some("z-index"), "2"));
        // Zero is unit-optional, a custom property is skipped even when its
        // name contains a length word, and a non-numeric value has a unit.
        assert!(!missing_unit(Some("width"), "0"));
        assert!(!missing_unit(Some("--max-width"), "10"));
        assert!(!missing_unit(Some("width"), "\"10px\""));
    }

    #[test]
    fn style_string_fixes_escape_javascript_strings_and_parse_css_boundaries() {
        assert_eq!(
            style_string_object_fix("content: \"x\""),
            Some("{{\"content\":\"\\\"x\\\"\"}}".into())
        );
        assert_eq!(
            style_string_object_fix(
                "background: url(\"data:image/svg+xml;utf8,<svg/>\"); color: red"
            ),
            Some(
                "{{\"background\":\"url(\\\"data:image/svg+xml;utf8,<svg/>\\\")\",\"color\":\"red\"}}"
                    .into()
            )
        );
        assert_eq!(style_string_object_fix("content: \"unterminated"), None);
    }
}
