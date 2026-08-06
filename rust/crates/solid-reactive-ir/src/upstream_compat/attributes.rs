//! `v1/no-innerhtml`, `v1/no-react-specific-props`, `v1/style-prop`,
//! `v1/event-handlers`, `v1/no-array-handlers`, `v1/prefer-classlist` —
//! eslint-plugin-solid's attribute-value rules, ported from the 1.x reactive
//! solver's `solid_1_rules.rs` onto this checker's fact tables.
//!
//! Five of these six are purely structural, like every rule in the sibling
//! `syntax` module. `event-handlers` is the exception: telling an attribute
//! value that Solid will treat as an inlined attribute (`onClick="doThing"`)
//! apart from one Solid will treat as a listener needs the value's *type*,
//! not just its syntax, when the value is neither a literal nor an
//! obviously-static local (`const x = "..."; onClick={x}`). For that one
//! case this reads the resolved TypeScript type through
//! [`UpstreamCompatContext::lookup`] instead of guessing from source text —
//! the same "ask what was proven, not what the syntax suggests" preference
//! [`super::reactivity`] documents for its own rules.
//!
//! # Options
//!
//! `no-innerhtml { allowStatic }`, `style-prop { styleProps, allowString }`,
//! `event-handlers { ignoreCase, warnOnSpread }`, and `prefer-classlist
//! { classnames }` are read from the project's
//! `.solid-checker/rule-options.json` (see [`super::options`]), defaulting
//! to upstream's defaults.

use std::collections::HashSet;

use solid_facts::FileFacts;
use solid_facts::ast::{JsxAttributeValueKind, JsxElementFact};

use super::{
    UpstreamCompatContext, binding_initializer, contains, fix_replace, is_lowercase_led,
    literal_string_type, static_string, static_string_expression, text, violation,
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
    for attribute in &element.attributes {
        let name = text(file, attribute.name);
        if name == "dangerouslySetInnerHTML" {
            let mut result = violation(
                file,
                "SC8008",
                "no-innerhtml",
                "The dangerouslySetInnerHTML prop is not supported; use innerHTML instead.",
                "Solid's DOM renderer has no special case for this React prop name, so it passes through as an inert, unrecognized attribute. Use innerHTML with the same { __html: ... } shape's inner value.",
                attribute.span,
                vec![],
            );
            if let Some(expression) = attribute.expression {
                let properties = file
                    .ast
                    .object_properties
                    .iter()
                    .filter(|property| contains(expression, property.span))
                    .collect::<Vec<_>>();
                if let [only] = properties.as_slice()
                    && text(file, only.key) == "__html"
                {
                    result.fixes.push(fix_replace(
                        file,
                        attribute.span,
                        "rewrite as innerHTML",
                        format!("innerHTML={{{}}}", text(file, only.value)),
                    ));
                }
            }
            violations.push(result);
            continue;
        }
        if !matches!(name, "innerHTML" | "innerhtml") {
            continue;
        }
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
        // Upstream's `allowStatic: false`: every innerHTML value is a
        // reported injection surface, static or not.
        if !context.options.no_innerhtml.allow_static {
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
            Some(value) if !(value.contains('<') && value.contains('>')) => {
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
            // `allowStatic` defaults to true (the `false` case returned
            // above), so a value proven static at the literal (containing
            // what looks like a tag) is accepted.
            Some(_) => {}
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
/// `className`/`htmlFor`, deprecated in Solid since 1.4, and a `key` prop on
/// a DOM element, which Solid has no use for outside `<For>`/`<Index>` (and
/// even there the list primitives take identity from the array items, not a
/// prop).
fn no_react_specific_props(
    file: &FileFacts,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
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
    if is_lowercase_led(text(file, element.name.span)) {
        for attribute in element
            .attributes
            .iter()
            .filter(|attribute| text(file, attribute.name) == "key")
        {
            let mut result = violation(
                file,
                "SC8011",
                "no-react-specific-props",
                "Elements in a <For> or <Index> list do not need a key prop.",
                "No DOM element has a key prop; this is a holdover from React and the compiler passes it straight through as an inert attribute.",
                attribute.span,
                vec![],
            );
            result
                .fixes
                .push(fix_replace(file, attribute.span, "remove the key prop", ""));
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
    const LENGTH_PROPERTIES: &[&str] = &[
        "width",
        "height",
        "margin",
        "margin-top",
        "padding",
        "border-width",
        "font-size",
    ];
    // Which props carry styles is upstream's `styleProps` option (default
    // `["style"]`, and naming others *replaces* the default); `allowString`
    // accepts the string form instead of asking for an object.
    let options = &context.options.style_prop;
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
        let Some(expression) = attribute.expression else {
            continue;
        };
        for property in file
            .ast
            .object_properties
            .iter()
            .filter(|property| contains(expression, property.span))
        {
            let name = text(file, property.key).trim_matches(['\'', '"']);
            if name.chars().any(char::is_uppercase) {
                let kebab = to_kebab_case(name);
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
            }
            if LENGTH_PROPERTIES.contains(&name)
                && text(file, property.value)
                    .trim()
                    .trim_start_matches('-')
                    .parse::<f64>()
                    .is_ok_and(|value| value != 0.0)
            {
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
        }
    }
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
            let expression = text(file, span).trim();
            if binding_initializer(file, expression).is_some_and(|(initializer, _)| {
                static_string_expression(file, initializer).is_some()
            }) {
                return true;
            }
            // Neither a literal nor an obviously-static local: ask what
            // TypeScript actually resolved the whole expression to. Exact
            // span, so a string-typed operand inside a larger expression
            // cannot answer for it. This proves cases upstream's constant
            // folder cannot ('string' + dynamic() is always a string), which
            // is a declared, evidence-backed parity deviation.
            super::expression_descriptor(context, file, span)
                .is_some_and(|descriptor| static_string_or_number_type(descriptor.text.as_ref()))
        });
        if type_is_static
            || matches!(
                attribute.value_kind,
                JsxAttributeValueKind::Boolean | JsxAttributeValueKind::String
            )
            || attribute.expression.is_some_and(|span| {
                let value = text(file, span).trim();
                value.parse::<f64>().is_ok() || static_string(file, span).is_some()
            })
        {
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
        if context.options.event_handlers.ignore_case {
            continue;
        }
        let fixed = if name.eq_ignore_ascii_case("ondoubleclick") {
            Some("onDblClick")
        } else {
            event_name(name)
        };
        if let Some(fixed) = fixed {
            if fixed != name {
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
        } else if name.as_bytes().get(2).is_some_and(u8::is_ascii_lowercase) {
            // Ambiguous between "ongoing"-style words and an unrecognized
            // handler; two equally valid readings, so this is a suggestion
            // rather than a fix (see attributes rule of the road: only an
            // unambiguous fix is emitted).
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
    // at runtime instead. Off by default, matching upstream.
    if context.options.event_handlers.warn_on_spread {
        for spread in &element.spreads {
            for property in
                file.ast.object_properties.iter().filter(|property| {
                    !property.computed && contains(spread.argument, property.span)
                })
            {
                let key = text(file, property.key)
                    .trim_matches(['\'', '"'])
                    .to_owned();
                if !key.starts_with("on")
                    || !key.as_bytes().get(2).is_some_and(u8::is_ascii_alphabetic)
                {
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
        let handler = attribute
            .namespace
            .is_some_and(|namespace| text(file, namespace) == "on")
            || (name.starts_with("on")
                && name.as_bytes().get(2).is_some_and(u8::is_ascii_alphabetic));
        if !handler {
            continue;
        }
        let array = attribute.expression.is_some_and(|span| {
            // When TypeScript resolved the whole value's type, that verdict
            // is the answer: an array- or tuple-typed value is an array
            // handler however it was spelled (`handlers` bound in another
            // file), and a value the author cast to a purpose-built type
            // (`pair as SafeArray<number>`) renders as that type, not as a
            // tuple — the same vouching upstream honours for casts. Only
            // without a resolved type does the rule fall back to this
            // file's spelling, where the cast itself is the bail.
            if let Some(descriptor) = super::expression_descriptor(context, file, span) {
                return super::array_like_type(
                    descriptor.text.as_ref(),
                    super::expression_callability(context, file, span),
                );
            }
            let source = text(file, span).trim();
            if source.contains(" as ") {
                return false;
            }
            looks_like_array_literal(source)
                || binding_initializer(file, source).is_some_and(|(_, initializer)| {
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

/// Whether a rendered type is statically a string or number: the primitives
/// themselves, a string/number/template literal type, or a union of those.
/// The naive `|` split is conservative: a `|` inside a generic or a quoted
/// literal splits into parts that fail the per-part test, so a type this
/// cannot read is never called static.
fn static_string_or_number_type(descriptor: &str) -> bool {
    descriptor.split('|').map(str::trim).all(|part| {
        matches!(part, "string" | "number")
            || part.starts_with('"')
            || part.starts_with('`')
            || part.parse::<f64>().is_ok()
    })
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
    let helpers = &context.options.prefer_classlist.classnames;
    if element
        .attributes
        .iter()
        .any(|attribute| text(file, attribute.name) == "classlist")
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
            "classlist takes the same { [name]: boolean } object and updates only the classes whose value actually changed, instead of recomputing and reassigning the whole class string.",
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
            result.fixes.push(fix_replace(
                file,
                attribute.span,
                "rewrite as classlist",
                format!("classlist={{{}}}", text(file, call.arguments[0].span)),
            ));
        }
        violations.push(result);
    }
}

#[cfg(test)]
mod tests {
    use super::super::strip_string_literal;
    use super::{
        event_name, is_lowercase_led, looks_like_array_literal, style_string_object_fix,
        to_kebab_case,
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
