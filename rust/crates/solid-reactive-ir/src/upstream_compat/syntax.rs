//! `v1/jsx-no-duplicate-props`, `v1/jsx-no-script-url`,
//! `v1/no-unknown-namespaces`, `v1/self-closing-comp` — eslint-plugin-solid's
//! purely structural JSX rules, ported from the 1.x reactive solver's
//! `solid_1_rules.rs` onto this checker's fact tables.
//!
//! Every rule here reads `file.ast.jsx_elements` and its nested attribute /
//! spread / object-property tables; none needs a TypeScript entity lookup, so
//! `check_file` does not touch `context` beyond the shape the caller shares
//! with every other upstream-compat module.
//!
//! # What upstream configures that this does not
//!
//! Upstream ships `jsx-no-duplicate-props { ignoreCase }`,
//! `no-unknown-namespaces { allowedNamespaces }`, and
//! `self-closing-comp { component, html }` as user-configurable options. This
//! checker has no per-rule options surface, so every rule below applies
//! upstream's *default* — `ignoreCase: false`, no extra allowed namespaces,
//! `component: "all"` and `html: "all"`. The last of those collapses
//! self-closing-comp to one direction: with both defaulted to `"all"`, an
//! element that already self-closes can never be "wrong" to do so, so only
//! the "should self-close but doesn't" case is reachable, and that is the
//! only case implemented.

use std::collections::HashSet;

use solid_facts::FileFacts;
use solid_facts::ast::JsxElementFact;
use solid_facts::core::Span;

use super::{UpstreamCompatContext, is_lowercase_led, text};
use crate::{Fix, StaticViolation, TextEdit, location};

pub(super) fn check_file(
    file: &FileFacts,
    _context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    for element in &file.ast.jsx_elements {
        jsx_no_duplicate_props(file, element, violations);
        jsx_no_script_url(file, element, violations);
        no_unknown_namespaces(file, element, violations);
        self_closing_comp(file, element, violations);
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
    let mut candidates = element
        .attributes
        .iter()
        .map(|attribute| (attribute.name, text(file, attribute.name).to_owned()))
        .collect::<Vec<_>>();
    for spread in &element.spreads {
        candidates.extend(
            file.ast
                .object_properties
                .iter()
                .filter(|property| !property.computed && contains(spread.argument, property.span))
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
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    for attribute in &element.attributes {
        let Some(value) = attribute
            .expression
            .or(attribute.value)
            .and_then(|span| static_string_expression(file, span))
        else {
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
fn is_javascript_protocol(value: &str) -> bool {
    let compact = value
        .trim_start_matches(|character: char| character <= ' ')
        .chars()
        .filter(|character| !matches!(character, '\r' | '\n' | '\t'))
        .collect::<String>();
    compact
        .get(..11)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("javascript:"))
}

/// `v1/no-unknown-namespaces` (SC8012) — a JSX attribute using the
/// `namespace:name` form with a namespace that is not one of Solid's
/// compiler-recognized prefixes, is a namespace on a component (which the
/// compiler never sees, since components receive props as a plain object),
/// or is `style:`/`class:` (valid, but a prop already says the same thing
/// more plainly).
fn no_unknown_namespaces(
    file: &FileFacts,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    const KNOWN: &[&str] = &[
        "on",
        "oncapture",
        "use",
        "prop",
        "attr",
        "bool",
        "xmlns",
        "xlink",
    ];
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
        } else if matches!(namespace, "style" | "class") {
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
        } else if !KNOWN.contains(&namespace) {
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

/// `v1/self-closing-comp` (SC8016) — an element with no children (or only
/// insignificant whitespace between its tags) that is not self-closing.
/// See the module doc for why only this direction is implemented.
fn self_closing_comp(
    file: &FileFacts,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
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

// --- shared helpers -------------------------------------------------------

/// Whether `outer` fully contains `inner`, byte-range-wise. Used to find the
/// object properties belonging to a `{...spread}` argument, and nothing
/// deeper: a nested object literal several levels down is not "contained" any
/// less by this test, which is exactly the same imprecision upstream's own
/// scope-walking has for a spread of a computed expression.
fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// The static string an attribute value or expression resolves to, following
/// one level of local variable indirection and one level of `+`
/// concatenation. Not a general constant-folder: it is exactly the shape
/// upstream's own scope-based `getStaticValue` recovers for the common
/// `javascript:` URL patterns (a literal, a `const url = "..."`, or
/// `"javascript:" + something`), no more.
fn static_string_expression(file: &FileFacts, span: Span) -> Option<String> {
    let source = text(file, span).trim();
    if !source.contains('+')
        && let Some(value) = static_string(file, span)
    {
        return Some(unescape_common_sequences(&value));
    }
    if source
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
    {
        return binding_initializer(file, source)
            .and_then(|(initializer, _)| static_string_expression(file, initializer));
    }
    concat_plus_joined_literal(source)
}

fn unescape_common_sequences(value: &str) -> String {
    value
        .replace("\\t", "\t")
        .replace("\\n", "\n")
        .replace("\\r", "\r")
}

/// Joins a `"a" + "b" + ...` chain of quoted literals into their concatenated
/// value, or `None` if any `+`-separated part is not itself a quoted literal
/// (including the degenerate case of no `+` at all).
fn concat_plus_joined_literal(source: &str) -> Option<String> {
    let parts = source.split('+').map(str::trim).collect::<Vec<_>>();
    if parts.len() <= 1 {
        return None;
    }
    parts
        .into_iter()
        .map(|part| {
            let quote = part.as_bytes().first().copied()?;
            (matches!(quote, b'\'' | b'"') && part.as_bytes().last().copied() == Some(quote))
                .then(|| part[1..part.len() - 1].to_owned())
        })
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.concat())
}

/// The declaration text of the nearest binding named `name` in this file, for
/// resolving a bare identifier used as an attribute value back to its
/// initializer. First match in file order, matching the reference port; a
/// real scope resolution is more than this narrow rule needs.
fn binding_initializer<'a>(file: &'a FileFacts, name: &str) -> Option<(Span, &'a str)> {
    file.ast.bindings.iter().find_map(|binding| {
        binding
            .names
            .iter()
            .any(|candidate| text(file, candidate.span) == name)
            .then(|| binding.initializer.map(|span| (span, text(file, span))))
            .flatten()
    })
}

/// The literal text of a quoted string span, unwrapping one optional
/// surrounding `{ ... }` JSX expression-container layer first (an
/// attribute's `value` span includes the braces when it came from
/// `attr={"literal"}` rather than `attr="literal"`).
fn static_string(file: &FileFacts, span: Span) -> Option<String> {
    strip_string_literal(text(file, span))
}

fn strip_string_literal(source: &str) -> Option<String> {
    let trimmed = source.trim();
    let trimmed = trimmed
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .map(str::trim)
        .unwrap_or(trimmed);
    let quote = trimmed.as_bytes().first().copied()?;
    if !matches!(quote, b'\'' | b'"' | b'`') || trimmed.as_bytes().last().copied() != Some(quote) {
        return None;
    }
    Some(trimmed[1..trimmed.len() - 1].to_owned())
}

fn fix_replace(
    file: &FileFacts,
    span: Span,
    message: impl Into<String>,
    new_text: impl Into<String>,
) -> Fix {
    Fix {
        message: message.into(),
        applicability: "safe".into(),
        edits: vec![TextEdit {
            location: location(file.path.shared(), span),
            new_text: new_text.into(),
        }],
    }
}

/// Builds a static violation, filling in the boilerplate every rule above
/// shares (identity, location, and an empty `analysis_context` — the rules
/// that need one, like `no-unknown-namespaces`, set it after the call).
fn violation(
    file: &FileFacts,
    id: &str,
    rule: &str,
    message: impl Into<String>,
    hint: impl Into<String>,
    span: Span,
    fixes: Vec<Fix>,
) -> StaticViolation {
    StaticViolation {
        id: id.into(),
        rule: rule.into(),
        message: message.into(),
        hint: hint.into(),
        location: location(file.path.shared(), span),
        analysis_context: String::new(),
        fixes,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        concat_plus_joined_literal, is_javascript_protocol, is_lowercase_led, normalize_prop_name,
        strip_string_literal,
    };

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

    #[test]
    fn does_not_flag_ordinary_urls() {
        assert!(!is_javascript_protocol("https://example.com"));
        assert!(!is_javascript_protocol("/relative/path"));
        assert!(!is_javascript_protocol(""));
    }

    #[test]
    fn strips_quotes_from_string_literals() {
        assert_eq!(strip_string_literal("\"hello\""), Some("hello".to_string()));
        assert_eq!(strip_string_literal("'hello'"), Some("hello".to_string()));
        assert_eq!(strip_string_literal("`hello`"), Some("hello".to_string()));
    }

    #[test]
    fn strips_an_expression_containers_braces_before_the_quotes() {
        assert_eq!(
            strip_string_literal("{ \"hello\" }"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn rejects_text_that_is_not_a_quoted_literal() {
        assert_eq!(strip_string_literal("foo"), None);
        assert_eq!(strip_string_literal("'unterminated"), None);
        assert_eq!(strip_string_literal(""), None);
    }

    #[test]
    fn joins_quoted_literal_concatenation() {
        assert_eq!(
            concat_plus_joined_literal("'java' + 'script:x'"),
            Some("javascript:x".to_string())
        );
    }

    #[test]
    fn refuses_to_join_when_any_part_is_not_a_literal() {
        assert_eq!(concat_plus_joined_literal("'java' + x"), None);
        assert_eq!(concat_plus_joined_literal("'just one'"), None);
    }

    #[test]
    fn classifies_element_names_by_leading_case() {
        assert!(is_lowercase_led("div"));
        assert!(!is_lowercase_led("Component"));
        assert!(!is_lowercase_led("Foo.Bar"));
    }
}
