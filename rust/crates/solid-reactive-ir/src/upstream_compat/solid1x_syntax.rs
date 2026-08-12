//! `v1/jsx-no-duplicate-props`, `v1/jsx-no-script-url`,
//! `v1/no-unknown-namespaces`, `v1/self-closing-comp` — eslint-plugin-solid's
//! purely structural JSX rules, ported from the 1.x reactive solver's
//! `solid_1_rules.rs` onto this checker's fact tables.
//!
//! Every rule here reads `file.ast.jsx_elements` and its nested attribute /
//! spread / object-property tables. The context is consulted twice, both
//! times for vocabulary rather than syntax: `jsx-no-script-url` recovers a
//! URL from a literal string *type* when the value's text lives in another
//! file, and `no-unknown-namespaces` asks the dialect which namespace
//! prefixes its compiler recognizes.
//!
//! # Options
//!
//! `no-unknown-namespaces { allowedNamespaces }` and `self-closing-comp
//! { component, html }` are read from the project's
//! `.solid-checker/rule-options.json` (see [`super::solid1x_options`]),
//! defaulting to upstream's defaults. `jsx-no-duplicate-props { ignoreCase }`
//! is the one option upstream ships here that the checker does not carry: no
//! upstream corpus case exercises a behaviour difference for it, and an option
//! nothing proves is a knob that can silently rot.

use std::collections::HashSet;

use solid_facts::FileFacts;
use solid_facts::ast::JsxElementFact;
use solid_facts::core::Span;

use super::{
    UpstreamCompatContext, deletion_with_leading_whitespace, fix_replace, is_lowercase_led,
    static_string_expression, text, violation,
};
use crate::StaticViolation;

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    for element in &file.ast.jsx_elements {
        jsx_no_duplicate_props(file, element, violations);
        jsx_no_script_url(file, context, element, violations);
        no_unknown_namespaces(file, context, element, violations);
        self_closing_comp(file, context, element, violations);
    }
}

/// `v1/jsx-no-duplicate-props` (SC8003) — the same prop written twice on one
/// opening tag, whether directly or hidden inside a spread, and the related
/// "more than one content source" check (`children` prop, JSX children,
/// `innerHTML`, `textContent` all fighting over the same element).
fn jsx_no_duplicate_props(
    file: &FileFacts,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    // Attributes and spread-carried object properties compete for the same
    // prop name, so both need to be in one candidate list, sorted back into
    // source order — a spread's properties are collected after direct
    // attributes but can appear anywhere among them in the actual JSX.
    //
    // Upstream's `jsxGetAllProps` looks inside a spread only when its
    // argument is literally an object expression, and then reads that
    // object's own entries: a key nested in some property's value, or a
    // property of a call's argument, is not a prop of this element.
    let mut candidates = element
        .attributes
        .iter()
        .map(|attribute| (attribute.name, text(file, attribute.name).to_owned()))
        .collect::<Vec<_>>();
    for spread in &element.spreads {
        candidates.extend(
            super::direct_object_literal_properties(file, spread.argument)
                .unwrap_or_default()
                .into_iter()
                .filter(|property| !property.computed)
                .map(|property| {
                    (
                        property.key,
                        text(file, property.key)
                            .trim_matches(['\'', '"'])
                            .to_owned(),
                    )
                }),
        );
    }
    candidates.sort_by_key(|(span, _)| (span.start, span.end));

    let mut names = HashSet::new();
    for (name_span, original) in candidates {
        let normalized = normalize_prop_name(&original);
        if !names.insert(normalized.clone()) {
            let message = if normalized == "class" {
                "Duplicate `class` props are not allowed; while it might seem to work, it can break unexpectedly. Use `classList` instead."
            } else {
                "Duplicate props are not allowed."
            };
            violations.push(violation(
                file,
                "SC8003",
                "jsx-no-duplicate-props",
                message,
                "Remove or merge the duplicate prop. JSX keeps only the last value written, so an earlier occurrence is dead and a later one silently wins.",
                name_span,
                vec![],
            ));
        }
    }

    let has_children_prop = names.contains("children");
    let has_children = !element.children.is_empty();
    let has_inner_html = names.contains("innerHTML") || names.contains("innerhtml");
    let has_text_content = names.contains("textContent");
    let used = [
        has_children_prop.then_some("`props.children`"),
        has_children.then_some("JSX children"),
        has_inner_html.then_some("`props.innerHTML`"),
        has_text_content.then_some("`props.textContent`"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if used.len() > 1 {
        violations.push(violation(
            file,
            "SC8003",
            "jsx-no-duplicate-props",
            format!("Using {} at the same time is not allowed.", used.join(", ")),
            "Pick exactly one source of content for this element. When more than one of children, innerHTML, and textContent is set, they overwrite each other and the result depends on write order.",
            element.opening,
            vec![],
        ));
    }
}

/// Normalizes a prop name the way upstream's `jsx-no-duplicate-props` does
/// before comparing for duplicates: event-ish names (`on...`) are folded to
/// lowercase and their `on:`/`oncapture:` prefix collapsed onto plain `on`,
/// and an `attr:`/`prop:` prefix is always stripped. `ignoreCase` is upstream
/// option we do not carry (see the module doc), so unlike upstream this
/// applies the on-folding based on the name alone, never on a rule option.
fn normalize_prop_name(original: &str) -> String {
    let mut normalized = original.to_owned();
    if original.starts_with("on") {
        let lowercase = original.to_ascii_lowercase();
        normalized = lowercase
            .strip_prefix("oncapture:")
            .or_else(|| lowercase.strip_prefix("on:"))
            .map_or_else(|| lowercase.clone(), |suffix| format!("on{suffix}"));
    }
    if let Some(suffix) = normalized
        .strip_prefix("attr:")
        .or_else(|| normalized.strip_prefix("prop:"))
    {
        normalized = suffix.to_owned();
    }
    normalized
}

/// `v1/jsx-no-script-url` (SC8004) — a `javascript:` URL written as a static
/// attribute value. Solid never executes these (nor do modern browsers, in
/// most contexts), so the value is either dead or a mistaken stand-in for a
/// real event handler.
fn jsx_no_script_url(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    for attribute in &element.attributes {
        // The text folder recovers this file's literal shapes; the
        // literal-string *type* recovers the same value when the binding
        // lives elsewhere (an import, an inferred const in another file).
        let value = attribute.expression.map_or_else(
            || {
                attribute
                    .value
                    .and_then(|span| super::static_string(file, span))
            },
            |span| {
                static_string_expression(context, file, span)
                    .or_else(|| super::literal_string_type(context, file, span))
            },
        );
        let Some(value) = value else {
            continue;
        };
        if is_javascript_protocol(&value) {
            violations.push(violation(
                file,
                "SC8004",
                "jsx-no-script-url",
                "For security, don't use javascript: URLs. Use event handlers instead if you can.",
                "Replace the javascript: URL with a real event handler prop (onClick, onSubmit, ...). Solid does not execute javascript: URLs, so the value only ever looked like it worked.",
                attribute.value.unwrap_or(attribute.span),
                vec![],
            ));
        }
    }
}

/// Whether `value` is a `javascript:` URL, tolerating the leading control
/// characters and interspersed tab/newline that a browser's URL parser
/// ignores (and that an attacker can use to slip the literal string past a
/// naive `.startsWith("javascript:")`).
///
/// The value is seen as source text, but a browser decodes character
/// references in attribute values before URL parsing, so `java&#9;script:`
/// and `javascript&colon;` are live URLs at runtime. Upstream's regex misses
/// these; decoding first closes that gap.
fn is_javascript_protocol(value: &str) -> bool {
    let compact = decode_character_references(value)
        .trim_start_matches(|character: char| character <= ' ')
        .chars()
        .filter(|character| !matches!(character, '\r' | '\n' | '\t'))
        .collect::<String>();
    compact
        .get(..11)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("javascript:"))
}

/// Decodes the character references a `javascript:` URL can hide behind:
/// numeric forms (`&#9;`, `&#x0A;`) and the named spellings of the
/// characters the URL parser strips or the protocol needs (`&Tab;`,
/// `&NewLine;`, `&colon;`). Everything else passes through unchanged.
fn decode_character_references(value: &str) -> std::borrow::Cow<'_, str> {
    if !value.contains('&') {
        return std::borrow::Cow::Borrowed(value);
    }
    let mut decoded = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(ampersand) = rest.find('&') {
        decoded.push_str(&rest[..ampersand]);
        rest = &rest[ampersand..];
        let Some(semicolon) = rest.find(';') else {
            break;
        };
        let reference = &rest[1..semicolon];
        let replacement = match reference {
            "Tab" => Some('\t'),
            "NewLine" => Some('\n'),
            "colon" => Some(':'),
            _ => reference
                .strip_prefix('#')
                .and_then(|digits| {
                    digits.strip_prefix(['x', 'X']).map_or_else(
                        || digits.parse().ok(),
                        |hex| u32::from_str_radix(hex, 16).ok(),
                    )
                })
                .and_then(char::from_u32),
        };
        if let Some(replacement) = replacement {
            decoded.push(replacement);
            rest = &rest[semicolon + 1..];
        } else {
            decoded.push('&');
            rest = &rest[1..];
        }
    }
    decoded.push_str(rest);
    std::borrow::Cow::Owned(decoded)
}

/// `v1/no-unknown-namespaces` (SC8012) — a JSX attribute using the
/// `namespace:name` form with a namespace that is not one of Solid's
/// compiler-recognized prefixes, is a namespace on a component (which the
/// compiler never sees, since components receive props as a plain object),
/// or is `style:`/`class:` (valid, but a prop already says the same thing
/// more plainly).
fn no_unknown_namespaces(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    // Which prefixes the compiler recognizes is dialect vocabulary, asked of
    // the dialect rather than baked into the rule: the 2.0 compiler dropped
    // every 1.x namespace except `prop:`. Upstream's `allowedNamespaces`
    // option accepts extra prefixes on top.
    let known = context.dialect.jsx_attribute_namespaces();
    let allowed = &context
        .solid1x_options
        .no_unknown_namespaces
        .allowed_namespaces;
    let component = !is_lowercase_led(text(file, element.name.span));
    for attribute in element
        .attributes
        .iter()
        .filter(|attribute| attribute.namespace.is_some())
    {
        let namespace = text(file, attribute.namespace.expect("filtered to Some above"));
        let local = text(file, attribute.local_name);
        let mut result = if component {
            let mut result = violation(
                file,
                "SC8012",
                "no-unknown-namespaces",
                "Namespaced props have no effect on components.",
                format!(
                    "Drop the `{namespace}:` prefix: components receive `{local}` as a plain prop, and Solid's compiler only special-cases namespaces on DOM elements it compiles directly."
                ),
                attribute.name,
                vec![],
            );
            result.fixes.push(fix_replace(
                file,
                attribute.name,
                format!("rename to `{local}`"),
                local,
            ));
            result
        } else if matches!(namespace, "style" | "class") && known.contains(&namespace) {
            // Recognized by the 1.x compiler (the dialect's namespace table
            // lists both), but upstream still steers authors to the plain
            // prop with this exact message — the namespaced form exists for
            // per-name toggling the plain prop usually expresses better.
            violation(
                file,
                "SC8012",
                "no-unknown-namespaces",
                format!(
                    "Using the '{namespace}:' special prefix is potentially confusing, prefer the '{namespace}' prop instead."
                ),
                format!(
                    "Replace `{namespace}:{local}` with the plain `{namespace}` prop; the namespaced form exists for edge cases the plain prop cannot express, which this usage is not."
                ),
                attribute.name,
                vec![],
            )
        } else if !known.contains(&namespace) && !allowed.iter().any(|extra| extra == namespace) {
            violation(
                file,
                "SC8012",
                "no-unknown-namespaces",
                format!(
                    "'{namespace}:' is not one of Solid's special prefixes for JSX attributes ('on:', 'oncapture:', 'use:', 'prop:', 'attr:', 'bool:')."
                ),
                "Use one of Solid's namespaces (on:, oncapture:, use:, prop:, attr:, bool:), or drop the prefix if a plain prop was intended; an unrecognized namespace compiles to nothing.",
                attribute.name,
                vec![],
            )
        } else {
            continue;
        };
        result.analysis_context = format!("JSX namespace {namespace}");
        violations.push(result);
    }
}

/// The HTML elements with no closing tag; the only ones upstream's
/// `html: "void"` policy wants self-closed.
fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

/// `v1/self-closing-comp` (SC8016) — an element whose self-closing form
/// disagrees with the configured policy for its category. Under upstream's
/// defaults (`component: "all"`, `html: "all"`) only one direction is
/// reachable — a childless element that fails to self-close — and that is
/// all this rule reported before options existed. A `"none"` (or, for HTML,
/// `"void"`) policy makes the inverse reachable: an element that self-closes
/// where the policy says it must not.
fn self_closing_comp(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    use crate::upstream_compat::solid1x_options::SelfClosePolicy;
    let options = &context.solid1x_options.self_closing_comp;
    let name = text(file, element.name.span);
    let component = !is_lowercase_led(name);
    let policy = if component {
        // `void` is not a meaningful component policy (upstream's schema
        // forbids it); treat it as the default.
        match options.component {
            SelfClosePolicy::Void => SelfClosePolicy::All,
            policy => policy,
        }
    } else {
        options.html
    };
    let wanted = match policy {
        SelfClosePolicy::All => true,
        SelfClosePolicy::Void => is_void_element(name),
        SelfClosePolicy::None => false,
    };
    if wanted {
        if element.self_closing || !children_are_insignificant(file, element) {
            return;
        }
        violations.push(violation(
            file,
            "SC8016",
            "self-closing-comp",
            "Empty components are self-closing.",
            "Self-close this tag: it has no meaningful children, so the separate closing tag is unnecessary.",
            element.opening,
            vec![fix_replace(
                file,
                Span::new(element.opening.end.saturating_sub(1), element.span.end),
                "self-close the tag",
                " />",
            )],
        ));
    } else if element.self_closing {
        violations.push(violation(
            file,
            "SC8016",
            "self-closing-comp",
            "This element should not be self-closing.",
            "Write the closing tag out: this project's rule options ask for explicit closing tags here.",
            element.opening,
            // The `/>` is replaced by `></name>`, and the whitespace that
            // separated it from the tag name or last attribute goes with it:
            // `<div />` becomes `<div></div>`, not `<div ></div>`. The
            // replacement text opens with `>`, so nothing that was holding
            // tokens apart is lost.
            vec![fix_replace(
                file,
                deletion_with_leading_whitespace(
                    &file.source,
                    Span::new(element.span.end.saturating_sub(2), element.span.end),
                ),
                "write the closing tag",
                format!("></{name}>"),
            )],
        ));
    }
}

/// Whether an opening tag has nothing between it and its closing tag worth
/// keeping the closing tag for: no children at all, or a single whitespace
/// text child that contains a newline (formatting-only, the same test
/// upstream uses — a non-breaking space is deliberately excluded, since that
/// is content, not layout whitespace).
fn children_are_insignificant(file: &FileFacts, element: &JsxElementFact) -> bool {
    element.children.is_empty()
        || (element.children.len() == 1 && {
            let content = text(file, element.children[0]);
            content.contains('\n')
                && content
                    .chars()
                    .all(|character| character != '\u{a0}' && character.is_whitespace())
        })
}

#[cfg(test)]
mod tests {
    use super::{is_javascript_protocol, is_lowercase_led, normalize_prop_name};

    #[test]
    fn normalizes_on_prefixed_names_to_lowercase_on() {
        assert_eq!(normalize_prop_name("onClick"), "onclick");
        assert_eq!(normalize_prop_name("on:click"), "onclick");
        assert_eq!(normalize_prop_name("oncapture:click"), "onclick");
    }

    #[test]
    fn strips_attr_and_prop_prefixes_unconditionally() {
        assert_eq!(normalize_prop_name("attr:title"), "title");
        assert_eq!(normalize_prop_name("prop:value"), "value");
    }

    #[test]
    fn leaves_names_that_are_not_on_prefixed_or_attr_prop_prefixed_alone() {
        assert_eq!(normalize_prop_name("class"), "class");
        assert_eq!(normalize_prop_name("id"), "id");
    }

    #[test]
    fn detects_javascript_urls_case_insensitively() {
        assert!(is_javascript_protocol("javascript:alert(1)"));
        assert!(is_javascript_protocol("JavaScript:alert(1)"));
        assert!(is_javascript_protocol("  javascript:alert(1)"));
    }

    #[test]
    fn detects_javascript_urls_with_interspersed_control_characters() {
        assert!(is_javascript_protocol("java\nscript:alert(1)"));
        assert!(is_javascript_protocol("j\ta\tv\ta\ts\tc\tr\ti\tp\tt:x"));
    }

    /// Browsers decode character references in attribute values before URL
    /// parsing, so these spellings are live `javascript:` URLs at runtime
    /// even though upstream's source-text regex misses them.
    #[test]
    fn detects_javascript_urls_hidden_behind_character_references() {
        assert!(is_javascript_protocol("java&#9;script:alert(1)"));
        assert!(is_javascript_protocol("java&#x0A;script:alert(1)"));
        assert!(is_javascript_protocol("java&Tab;script:alert(1)"));
        assert!(is_javascript_protocol("java&NewLine;script:alert(1)"));
        assert!(is_javascript_protocol("javascript&colon;alert(1)"));
    }

    #[test]
    fn does_not_flag_ordinary_urls() {
        assert!(!is_javascript_protocol("https://example.com"));
        assert!(!is_javascript_protocol("/relative/path"));
        assert!(!is_javascript_protocol(""));
        assert!(!is_javascript_protocol("https://example.com/?q=a&b=c"));
        assert!(!is_javascript_protocol("/path?fish&chips"));
    }

    #[test]
    fn classifies_element_names_by_leading_case() {
        assert!(is_lowercase_led("div"));
        assert!(!is_lowercase_led("Component"));
        assert!(!is_lowercase_led("Foo.Bar"));
    }
}
