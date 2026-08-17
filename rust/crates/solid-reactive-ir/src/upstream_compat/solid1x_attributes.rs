//! `v1/no-innerhtml`, `v1/no-react-specific-props`, `v1/style-prop`,
//! `v1/event-handlers`, `v1/no-array-handlers`, `v1/prefer-classlist` —
//! eslint-plugin-solid's attribute-value rules, ported from the 1.x reactive
//! solver's `solid_1_rules.rs` onto this checker's fact tables.
//!
//! Five of these six are purely structural, like every rule in the sibling
//! `solid1x_syntax` module. `event-handlers` is the exception: telling an
//! attribute value that Solid will treat as an inlined attribute
//! (`onClick="doThing"`)
//! apart from one Solid will treat as a listener needs the value's *type*,
//! not just its syntax, when the value is neither a literal nor an
//! obviously-static local (`const x = "..."; onClick={x}`). For that one
//! case this reads the resolved TypeScript type through
//! [`UpstreamCompatContext::lookup`] instead of guessing from source text —
//! the same "ask what was proven, not what the syntax suggests" preference
//! [`super::shared_reactivity`] documents for its own rules. The compiler's
//! template/static branch is the exception: it is a syntax-node-kind rule, so
//! this module delegates that exact predicate to `solid1x_syntax`.
//!
//! # Options
//!
//! `no-innerhtml { allowStatic }`, `style-prop { styleProps, allowString }`,
//! `event-handlers { ignoreCase, warnOnSpread }`, and `prefer-classlist
//! { classnames }` are read from the project's
//! `.solid-checker/rule-options.json` (see [`super::solid1x_options`]),
//! defaulting to upstream's defaults.

use std::collections::HashSet;

use solid_facts::FileFacts;
use solid_facts::ast::{JsxAttributeValueKind, JsxElementFact};

use super::{
    UpstreamCompatContext, binding_initializer, fix_replace, is_lowercase_led,
    jsx_name_is_type_checked, literal_string_type, static_string, static_string_expression, text,
    violation,
};
use crate::StaticViolation;
use typefacts::{ArrayShape, Callability};

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    for element in &file.ast.jsx_elements {
        no_innerhtml(file, context, element, violations);
        no_react_specific_props(file, element, violations);
        style_prop(file, context, element, violations);
        event_handlers(file, context, element, violations);
        no_array_handlers(file, context, element, violations);
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

const COMMON_EVENTS: &[&str] = &[
    "onAnimationEnd",
    "onAnimationIteration",
    "onAnimationStart",
    "onBeforeInput",
    "onBlur",
    "onChange",
    "onClick",
    "onContextMenu",
    "onCopy",
    "onCut",
    "onDblClick",
    "onDrag",
    "onDragEnd",
    "onDragEnter",
    "onDragExit",
    "onDragLeave",
    "onDragOver",
    "onDragStart",
    "onDrop",
    "onError",
    "onFocus",
    "onFocusIn",
    "onFocusOut",
    "onGotPointerCapture",
    "onInput",
    "onInvalid",
    "onKeyDown",
    "onKeyPress",
    "onKeyUp",
    "onLoad",
    "onLostPointerCapture",
    "onMouseDown",
    "onMouseEnter",
    "onMouseLeave",
    "onMouseMove",
    "onMouseOut",
    "onMouseOver",
    "onMouseUp",
    "onPaste",
    "onPointerCancel",
    "onPointerDown",
    "onPointerEnter",
    "onPointerLeave",
    "onPointerMove",
    "onPointerOut",
    "onPointerOver",
    "onPointerUp",
    "onReset",
    "onScroll",
    "onSelect",
    "onSubmit",
    "onToggle",
    "onTouchCancel",
    "onTouchEnd",
    "onTouchMove",
    "onTouchStart",
    "onTransitionEnd",
    "onWheel",
];

/// The canonically-cased spelling of a common DOM event handler name, chosen
/// case-insensitively, or `None` if `name` is not one of them.
fn event_name(name: &str) -> Option<&'static str> {
    COMMON_EVENTS
        .iter()
        .copied()
        .find(|event| event.eq_ignore_ascii_case(name))
}

/// `v1/event-handlers` (SC8001) — an `on...`-named DOM attribute whose value
/// Solid's compiler will inline as a plain attribute rather than attach as a
/// listener (because the value is statically a string or number), a
/// nonstandard or miscapitalized spelling of a real event
/// (`ondoubleclick`/`onclick`), and a name that is ambiguous between "event
/// handler with an unusual capitalization" and "attribute that happens to
/// start with `on`" (`onload-status`-style names the third letter of which is
/// lowercase).
fn event_handlers(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    let element_name = text(file, element.name.span);
    if element_name.contains('.') || !is_lowercase_led(element_name) {
        return; // bail if this is not a DOM/SVG element or web component
    }
    // Narrowed 2026-08-17 under AGENTS.md's absolute rule. Solid 1.x's JSX
    // types declare every standard handler under both its camelCase and its
    // all-lowercase spelling, and `HTMLAttributes` has no `on*` index
    // signature, so on a standard element TypeScript answers two of this
    // rule's three attribute arms outright:
    //
    //   * an unknown `on*` name is TS2322 "Property 'onFoo' does not exist on
    //     type 'HTMLAttributes<HTMLDivElement>'" -- in every value form,
    //     including the boolean shorthand;
    //   * a statically valued *known* handler is TS2322 "Type 'string' is not
    //     assignable to type 'EventHandlerUnion<…>'", and no static value is
    //     ever assignable, so that arm has no residue either.
    //
    // A **hyphenated** tag is different: `<my-element />` is TS2339 against
    // stock typings, so any project that actually uses one has augmented
    // `JSX.IntrinsicElements` with its own declaration -- commonly a permissive
    // one. There TypeScript is silent and this rule's claim (Solid freezes a
    // statically valued `on*` prop into the template as a plain attribute
    // rather than attaching a listener) is the only one available.
    // A hyphenated tag is one case TypeScript declines; a hyphenated *attribute
    // name* is the other, and it is checked per attribute below. Both reopen the
    // arms the narrowing otherwise hands to TypeScript.
    let custom_element = element_name.contains('-');
    for attribute in element
        .attributes
        .iter()
        .filter(|attribute| attribute.namespace.is_none())
    {
        let name = text(file, attribute.name);
        if !name.starts_with("on") || !name.as_bytes().get(2).is_some_and(u8::is_ascii_alphabetic) {
            continue;
        }
        let type_is_static = attribute.expression.is_some_and(|span| {
            if static_string_expression(context, file, span).is_some() {
                return true;
            }
            // The pinned compiler freezes only a StringLiteral or
            // NumericLiteral expression. In particular, `-1` is a unary
            // expression and `NaN` is an identifier, even though TypeScript
            // renders both as a primitive number. Keep this syntactic test in
            // lockstep with jsx-no-duplicate-props rather than parsing a
            // rendered type descriptor.
            super::solid1x_syntax::expression_is_static_literal(file, span)
        });
        // No source-text fallback here. Parsing the written text with
        // `str::parse::<f64>` accepted exactly the spellings the compiler does
        // *not* freeze — `-1` (unary), `NaN` and `Infinity` (identifiers) —
        // and it sat in this disjunction, so it decided the answer before the
        // syntactic test above could. The diagnostic claims Solid "will treat
        // the value as an attribute", which is only true for the frozen forms.
        if type_is_static
            || matches!(
                attribute.value_kind,
                JsxAttributeValueKind::Boolean | JsxAttributeValueKind::String
            )
        {
            // TypeScript's on a standard element, whatever the name: a known
            // handler rejects the value, an unknown name does not exist. Unless
            // the name carries a hyphen, which TypeScript never checks --
            // `onFoo-bar` is accepted on a `<div>` however it is valued.
            if !custom_element && jsx_name_is_type_checked(name) {
                continue;
            }
            violations.push(violation(
                file,
                "SC8001",
                "event-handlers",
                format!(
                    "The {name} prop is named as an event handler (starts with \"on\"), but Solid knows its value is a string or number, so it will be treated as an attribute. If this is intentional, name this prop attr:{name}."
                ),
                format!(
                    "Rename to attr:{name} to make the attribute reading explicit, or change the value to a function if a listener was intended."
                ),
                attribute.span,
                vec![],
            ));
            continue;
        }
        // Upstream's `ignoreCase`: handler names are accepted as written, so
        // the canonical-spelling and ambiguous-name advice below is off.
        if context.solid1x_options.event_handlers.ignore_case {
            continue;
        }
        let fixed = if name.eq_ignore_ascii_case("ondoubleclick") {
            Some("onDblClick")
        } else {
            event_name(name)
        };
        if let Some(fixed) = fixed {
            // The rename advice survives only where the name as written is a
            // *declared* spelling, so TypeScript accepts it and the remaining
            // objection is readability. Solid 1.x declares each handler as
            // both `onClick` and `onclick`, so that is the all-lowercase form:
            // `onclick` is silent to `tsc` and keeps its advice, while
            // `onClIcK`, `oncLICK`, and `ondoubleclick` are TS2322 and are now
            // TypeScript's. On a hyphenated tag nothing is declared by Solid,
            // so the advice stands there too.
            let declared_spelling = custom_element
                || !jsx_name_is_type_checked(name)
                || name == fixed.to_ascii_lowercase();
            if fixed != name && declared_spelling {
                let message = if name.eq_ignore_ascii_case("ondoubleclick") {
                    format!(
                        "The {name} prop should be renamed to {fixed}, because it's not a standard event handler."
                    )
                } else {
                    format!("The {name} prop should be renamed to {fixed} for readability.")
                };
                let mut result = violation(
                    file,
                    "SC8001",
                    "event-handlers",
                    message,
                    format!(
                        "{fixed} is the standard spelling; renaming keeps the handler recognizable and consistent with the rest of the codebase."
                    ),
                    attribute.name,
                    vec![],
                );
                result.fixes.push(fix_replace(
                    file,
                    attribute.name,
                    format!("rename to {fixed}"),
                    fixed,
                ));
                violations.push(result);
            }
        } else if (custom_element || !jsx_name_is_type_checked(name))
            && name.as_bytes().get(2).is_some_and(u8::is_ascii_lowercase)
        {
            // Ambiguous between "ongoing"-style words and an unrecognized
            // handler; two equally valid readings, so this is a suggestion
            // rather than a fix (see attributes rule of the road: only an
            // unambiguous fix is emitted).
            //
            // Reached only on a hyphenated tag: an unrecognised `on*` name on a
            // standard element does not exist on its attribute type, which is
            // TS2322's sentence, not this rule's.
            let handler_name = format!("on{}{}", name[2..3].to_ascii_uppercase(), &name[3..]);
            let attr_name = format!("attr:{name}");
            violations.push(violation(
                file,
                "SC8001",
                "event-handlers",
                format!(
                    "The {name} prop is ambiguous. If it is an event handler, change it to {handler_name}. If it is an attribute, change it to {attr_name}."
                ),
                format!(
                    "Rename explicitly: {handler_name} if Solid should attach a listener, or {attr_name} to force it to be a plain DOM attribute."
                ),
                attribute.name,
                vec![],
            ));
        }
    }
    // Upstream's `warnOnSpread`: a handler-named property carried into a DOM
    // element through a JSX spread. Solid attaches listeners from attributes
    // the compiler can see; a spread delivers the value as a plain property
    // at runtime instead. Off by default, matching upstream — which looks
    // only at a spread that is itself an object literal, reads that object's
    // own entries, and takes any plain `on*`-named identifier key (no
    // third-letter requirement: this judges the object's shape, not whether
    // the name could be a real event).
    if context.solid1x_options.event_handlers.warn_on_spread {
        for spread in &element.spreads {
            for property in
                super::direct_object_literal_properties(file, spread.argument).unwrap_or_default()
            {
                if property.computed {
                    continue;
                }
                let key = text(file, property.key);
                if key.starts_with(['\'', '"']) || !key.starts_with("on") {
                    continue;
                }
                violations.push(violation(
                    file,
                    "SC8001",
                    "event-handlers",
                    format!(
                        "The {key} prop should be written as a JSX attribute, not spread in; Solid attaches listeners from attributes its compiler can see."
                    ),
                    format!("Move {key} out of the spread and write it directly on the element."),
                    property.span,
                    vec![],
                ));
            }
        }
    }
}

/// `v1/no-array-handlers` (SC8007) — an array literal passed where an event
/// handler is expected. Solid's handler prop accepts a `[handler, data]`
/// tuple as a type-unsafe shorthand for binding extra data; passing a plain
/// array where a function was meant compiles but silently does nothing.
///
/// The `on:` namespace arm was removed on 2026-08-18 under AGENTS.md's absolute
/// rule. `onXxx` is typed `EventHandlerUnion = EHandler | BoundEventHandler`,
/// and `BoundEventHandler` is an interface with members `0` and `1`, so a
/// `[handler, data]` tuple is legal per solid-js@1.9.14's own types and only
/// this rule can speak about it. `on:xxx` is typed
/// `EventHandlerWithOptionsUnion = EHandler | EventHandlerWithOptions`, which
/// has **no** bound form at all — so every array and every tuple there is
/// already TS2322, and reporting it again duplicated the type checker.
/// Confirmed with `node scripts/tsc-oracle.mjs check --dialect v1` against the
/// real typings, in both the strict and non-strict passes.
fn no_array_handlers(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    if !is_lowercase_led(text(file, element.name.span)) {
        return; // bail if this is not a DOM/SVG element or web component
    }
    for attribute in &element.attributes {
        let name = text(file, attribute.name);
        // Any namespaced attribute is out, not just `on:`. `attr:`/`prop:` were
        // never handlers, and `on:` is TypeScript's — see the note above.
        let handler = attribute.namespace.is_none()
            && name.starts_with("on")
            && name.as_bytes().get(2).is_some_and(u8::is_ascii_alphabetic);
        if !handler {
            continue;
        }
        let array = attribute.expression.is_some_and(|span| {
            // `BoundEventHandler` is `{ 0: (data, ...event) => void; 1: any }`,
            // so TypeScript accepts a value here only when it has both numbered
            // members and the first is callable. That set — and nothing wider —
            // is what this rule owns, and `tupleShape` is what decides it: an
            // alias, a binding in another file, and an inline literal all reduce
            // to the same slot count and first-slot callability.
            if let Some(tuple) = super::expression_tuple_shape(context, file, span) {
                return tuple.has_slot(0)
                    && tuple.has_slot(1)
                    && tuple.element_zero == Some(Callability::Callable);
            }
            let source = text(file, span).trim();
            // A cast vouches for the value, as it does upstream, and it has to
            // be honoured before any test on this file's spelling: the cast of
            // an array literal still *starts* with `[`.
            if source.contains(" as ") {
                return false;
            }
            if looks_like_array_literal(source) {
                // Written here as an array literal, and yet the checker gave it
                // no fixed slots. Contextual typing is what creates those, so
                // its absence means no `[handler, data]`-shaped type applies at
                // this attribute — the ambient JSX typings are not checking it,
                // and where TypeScript declines to look this rule is the only
                // thing that can speak. Under Solid's real typings a literal in
                // a handler position always arrives here as a tuple, so this
                // branch never competes with a diagnostic.
                return true;
            }
            // A resolved shape that is not a tuple. A plain array has no `0`/`1`
            // members and is already TS2322, and a proven non-array was never a
            // handler pair; either way the rule stays out of it. `Mixed` and
            // `Unknown` prove neither, so they fall through with absence.
            if matches!(
                super::expression_array_shape(context, file, span),
                Some(ArrayShape::Array | ArrayShape::NotArray)
            ) {
                return false;
            }
            binding_initializer(context, file, span).is_some_and(|(_, _, initializer, _)| {
                looks_like_array_literal(initializer.trim_start())
            })
        });
        if array {
            violations.push(violation(
                file,
                "SC8007",
                "no-array-handlers",
                "Passing an array as an event handler is potentially type-unsafe.",
                "Use a plain function, or wrap the [handler, data] pair so its first element is unmistakably callable; an array typed as unknown[] compiles but does not check that its first element is callable.",
                attribute.span,
                vec![],
            ));
        }
    }
}

fn looks_like_array_literal(source: &str) -> bool {
    source.starts_with('[')
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
    use super::{
        event_name, is_lowercase_led, looks_like_array_literal, missing_unit,
        style_string_object_fix, to_kebab_case,
    };

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
    fn resolves_common_event_names_case_insensitively() {
        assert_eq!(event_name("onclick"), Some("onClick"));
        assert_eq!(event_name("ONCLICK"), Some("onClick"));
        assert_eq!(event_name("onDblClick"), Some("onDblClick"));
    }

    #[test]
    fn does_not_resolve_unknown_event_names() {
        assert_eq!(event_name("onfoobar"), None);
        assert_eq!(event_name("ondoubleclick"), None);
    }

    #[test]
    fn strips_quotes_from_string_literals() {
        assert_eq!(strip_string_literal("\"hello\""), Some("hello".to_string()));
        assert_eq!(strip_string_literal("{ 'hi' }"), Some("hi".to_string()));
        assert_eq!(strip_string_literal("notAString"), None);
    }

    #[test]
    fn recognizes_array_literal_prefixes() {
        assert!(looks_like_array_literal("[handler, data]"));
        assert!(!looks_like_array_literal("handler"));
        assert!(!looks_like_array_literal("(a, b)"));
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
