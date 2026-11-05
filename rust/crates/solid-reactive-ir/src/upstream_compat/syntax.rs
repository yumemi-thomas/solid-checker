//! `jsx-no-duplicate-props` — intrinsic content competition in both dialects,
//! plus eslint-plugin-solid's Solid 1.x DOM-slot folding.
//!
//! Every rule here reads `file.ast.jsx_elements` and its nested attribute /
//! spread / object-property tables. `jsx-no-duplicate-props { ignoreCase }`
//! is the one option upstream ships here that the checker does not carry: no
//! upstream corpus case exercises a behaviour difference for it, and an option
//! nothing proves is a knob that can silently rot.

use std::sync::Arc;

use crate::{HashMap, HashSet};

use solid_facts::FileFacts;
use solid_facts::ast::{JsxAttributeValueKind, JsxElementFact};
use solid_facts::core::Span;

use super::{is_lowercase_led, text, violation};
use crate::StaticViolation;

pub(super) fn check_file<'a>(
    file: &FileFacts,
    declared_keys: impl Fn(Span) -> Option<&'a [Arc<str>]>,
    violations: &mut Vec<StaticViolation>,
) {
    for element in &file.ast.jsx_elements {
        jsx_no_duplicate_props(file, element, &declared_keys, violations);
    }
}

/// `jsx-no-duplicate-props` (SC8003) — the same prop written twice on one
/// opening tag, whether directly or hidden inside a spread, and the related
/// "more than one content source" check (`children` prop, JSX children,
/// `innerHTML`, `innerText`, and `textContent` all fighting over the same
/// element).
fn jsx_no_duplicate_props<'a>(
    file: &FileFacts,
    element: &JsxElementFact,
    declared_keys: &impl Fn(Span) -> Option<&'a [Arc<str>]>,
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
    //
    // The slot model below describes what the *DOM* compiler does to an
    // intrinsic element's props. A component never reaches that lowering: its
    // props become a plain object, where the only slot is the key itself and
    // a later `onSave` overwrites an earlier one no matter how either is
    // spelled. Applying the DOM model there would silence real duplicates.
    let intrinsic = is_lowercase_led(text(file, element.name.span));
    // The DOM-slot folding model was the 1.x compiler's, and went with that
    // dialect (ADR 0110). It is a separate question from `intrinsic`, which
    // the content-competition check below still asks: a component's props are
    // a plain object either way.
    //
    // Slot folding is `false` for every analysis this checker runs. It was
    // gated on the 1.x catalog, which ADR 0110 retired, and no 2.0 question
    // replaces that gate, so no finding moves here. The parameter stays on
    // `duplicate_slot` because the model is a property of a *compiler*, and
    // the next vocabulary whose lowering folds slots sets it again.
    let folds_dom_slots = false;
    let mut candidates: Vec<(Span, Option<String>, &str)> = element
        .attributes
        .iter()
        .map(|attribute| {
            let static_literal = match attribute.value_kind {
                // A bare attribute and a JSX string value are both frozen
                // into the template by the compiler.
                JsxAttributeValueKind::String | JsxAttributeValueKind::Boolean => true,
                JsxAttributeValueKind::Expression => attribute
                    .expression
                    .is_some_and(|span| expression_is_static_literal(file, span)),
                JsxAttributeValueKind::Element | JsxAttributeValueKind::Fragment => false,
            };
            let name = text(file, attribute.name);
            (
                attribute.name,
                duplicate_slot(name, static_literal, folds_dom_slots),
                name,
            )
        })
        .collect::<Vec<_>>();
    for spread in &element.spreads {
        candidates.extend(
            super::direct_object_literal_properties(file, spread.argument)
                .unwrap_or_default()
                .into_iter()
                .filter(|property| !property.computed)
                .map(|property| {
                    let name = text(file, property.key).trim_matches(['\'', '"']);
                    (
                        property.key,
                        duplicate_slot(
                            name,
                            expression_is_static_literal(file, property.value),
                            folds_dom_slots,
                        ),
                        name,
                    )
                }),
        );
    }
    candidates.sort_by_key(|(span, ..)| (span.start, span.end));

    let mut names: HashMap<String, &str> = HashMap::default();
    let mut seen_slots = HashSet::default();
    for (name_span, slot, written) in candidates {
        let Some(normalized) = slot else {
            continue;
        };
        seen_slots.insert(normalized.clone());
        if let Some(first_written) = names.insert(normalized.clone(), written) {
            // Narrowed 2026-08-17 under AGENTS.md's absolute rule. When both
            // occurrences are spelled *identically*, TypeScript already makes
            // this exact claim, and which diagnostic it makes depends on where
            // the two live -- verified against the real solid-js@1.9.14
            // typings, on intrinsic elements and components alike:
            //
            //   two attributes        TS17001 "JSX elements cannot have
            //                         multiple attributes with the same name"
            //   attribute, then spread TS2783 "'class' is specified more than
            //                         once, so this usage will be overwritten"
            //   one spread object      TS1117 "An object literal cannot have
            //                         multiple properties with the same name"
            //
            // Spread overrides are deliberate and no longer report.
            // What survives is the case this rule exists
            // for: two *differently spelled* props that the DOM lowering folds
            // into one slot -- `onClick`/`onclick` both become the delegated
            // `el.$$click` write, and `attr:title`/`title` share the template
            // attribute slot. TypeScript sees two distinct, legal properties
            // and is silent.
            // Exact spellings are either TypeScript-owned (two direct
            // attributes or one object literal) or an intentional JSX spread
            // override. Only differently spelled keys that the DOM compiler
            // folds into one slot remain in this rule, and components never
            // enter that lowering.
            if written == first_written || !folds_dom_slots {
                continue;
            }
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

    let has_children_prop = seen_slots.contains("children");
    let has_children = !element.children.is_empty();
    // Only the camel-case `innerHTML` writes inner HTML. Upstream also reads a
    // lowercase `innerhtml` alias (eslint-plugin-solid 0.14.5, rule line 77),
    // but rc.13 types only `innerHTML` (TS2322 on the lowercase spelling), and
    // the runtime's property set (`ChildProperties`) has no lowercase entry, so
    // a lowercase attribute goes through `setAttribute` and overwrites nothing.
    // The kept divergence is pinned as `innerhtml/typescript-owned/003`.
    let has_inner_html = seen_slots.contains("innerHTML");
    // `innerText` is in `ChildProperties` too (rc.13 dist/web.js line 33), so a
    // property write replaces the child nodes. Upstream has no such arm; the
    // divergence is pinned as `innertext/solid-v2/content-positive/001`.
    let has_inner_text = seen_slots.contains("innerText");
    let has_text_content = seen_slots.contains("textContent");
    let used = [
        has_children_prop.then_some("`props.children`"),
        has_children.then_some("JSX children"),
        has_inner_html.then_some("`props.innerHTML`"),
        has_inner_text.then_some("`props.innerText`"),
        has_text_content.then_some("`props.textContent`"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    // The `children`-prop-plus-JSX-children pair, and *only* that pair, is
    // TS2710: "'children' are specified twice. The attribute named 'children'
    // will be overwritten." -- word for word this arm's claim, in both passes and
    // on components as well as intrinsic elements. Narrowed 2026-08-17 after
    // the TypeScript ownership audit matched the two spans.
    //
    // Any intrinsic set that also includes `innerHTML`, `innerText`, or
    // `textContent` still reports: those conflicts draw no diagnostic at all,
    // so the finding asserts more than TS2710 does even where TS2710 also
    // fires. Verified against the rc.13 typings: `innerHTML` with
    // `textContent`, `innerHTML` with JSX children, `innerText` with JSX
    // children, `innerText` with `textContent`, and `innerText` with
    // `innerHTML` are all silent. On a component these are ordinary props that
    // its implementation may combine.
    let only_the_children_pair = used.len() == 2
        && has_children_prop
        && has_children
        && !has_inner_html
        && !has_inner_text
        && !has_text_content;
    if !intrinsic {
        return;
    }
    let values = content_values(file, element);
    let child = child_content(file, element);
    let certain = |name: &str| values.get(name).copied().unwrap_or(false);
    let written = |name: &str| values.contains_key(name);
    // ADR 0651, measured in Chrome on rc.21. A write of `innerHTML` or
    // `innerText` replaces the children whatever its value, but a child with
    // nothing to render loses nothing beside `innerHTML`. Beside `textContent`
    // any child in the source, even a comment, drops the text; a nullish or
    // empty text drops nothing. Two attributes compete only with values.
    let attribute_pair = CONTENT_ATTRIBUTES
        .iter()
        .filter(|name| certain(name))
        .count()
        > 1;
    let proven = !only_the_children_pair
        && (attribute_pair
            || (written("innerHTML") && child == ChildContent::Real)
            || (written("innerText") && child == ChildContent::Real)
            || (certain("textContent") && child != ChildContent::None));
    // Only `innerHTML` beside a child with nothing to render: nothing is lost.
    let no_loss =
        child == ChildContent::Blank && used.len() == 2 && has_inner_html && !has_children_prop;
    let mut open_sources = Vec::new();
    let mut open_spread_at = None;
    let mut any_key = false;
    for spread in &element.spreads {
        let keys = spread_content_keys(file, spread.argument, declared_keys);
        match keys {
            SpreadKeys::Closed => continue,
            SpreadKeys::Declared(keys) => open_sources.extend(keys),
            SpreadKeys::Any => any_key = true,
        }
        open_spread_at = Some(spread.span.start);
    }
    // A spread that may rewrite a source after it may take the value away.
    let rewritten = open_spread_at.is_some_and(|at| {
        element.attributes.iter().any(|attribute| {
            attribute.span.start < at && CONTENT_ATTRIBUTES.contains(&text(file, attribute.name))
        })
    });
    let verdict = if proven && !rewritten {
        Some(None)
    } else {
        let spread_competes = any_key || {
            let sources = CONTENT_ATTRIBUTES
                .into_iter()
                .filter(|name| written(name) || open_sources.contains(name))
                .collect::<Vec<_>>();
            !open_sources.is_empty()
                && (sources.len() + usize::from(has_children) > 1)
                && sources != ["children"]
        };
        if spread_competes {
            Some(Some(crate::blockers::SPREAD_CONTENT_UNPROVEN))
        } else if used.len() > 1 && !only_the_children_pair && !no_loss {
            Some(Some(crate::blockers::CONTENT_SOURCE_VALUE_UNPROVEN))
        } else {
            None
        }
    };
    if let Some(blocker) = verdict {
        let message = if blocker == Some(crate::blockers::SPREAD_CONTENT_UNPROVEN) {
            "A spread here may carry `children`, `innerHTML`, `innerText` or `textContent`, and \
             nothing proves it does not compete with this element's other content."
                .to_owned()
        } else {
            format!("Using {} at the same time is not allowed.", used.join(", "))
        };
        let mut finding = violation(
            file,
            "SC8003",
            "jsx-no-duplicate-props",
            message,
            "Pick exactly one source of content for this element. When more than one of children, innerHTML, innerText, and textContent is set, they overwrite each other and the result depends on write order.",
            element.opening,
            vec![],
        );
        if let Some(blocker) = blocker {
            finding.uncertain = true;
            blocker.clone_into(&mut finding.analysis_context);
        }
        violations.push(finding);
    }
}

/// Every prop that competes for an intrinsic element's content.
const CONTENT_ATTRIBUTES: [&str; 4] = ["children", "innerHTML", "innerText", "textContent"];

/// What an element's written children render (ADR 0651).
#[derive(Clone, Copy, Eq, PartialEq)]
enum ChildContent {
    None,
    /// Children in the source that render nothing: whitespace, a comment,
    /// `{null}`, `{undefined}`, `{false}`, `{""}`.
    Blank,
    /// An expression whose value decides.
    Open,
    Real,
}

fn child_content(file: &FileFacts, element: &JsxElementFact) -> ChildContent {
    let mut content = ChildContent::None;
    for child in &element.children {
        let source = text(file, *child).trim();
        let state = if source.is_empty() || element.empty_children.contains(child) {
            ChildContent::Blank
        } else if let Some(inner) = source
            .strip_prefix('{')
            .and_then(|inner| inner.strip_suffix('}'))
        {
            let inner = inner.trim();
            if matches!(
                inner,
                "null" | "undefined" | "false" | "true" | "\"\"" | "''"
            ) {
                ChildContent::Blank
            } else if expression_is_static_literal(file, *child) {
                ChildContent::Real
            } else {
                ChildContent::Open
            }
        } else if source.starts_with("<>") {
            ChildContent::Open
        } else {
            ChildContent::Real
        };
        content = match (content, state) {
            (ChildContent::Real, _) | (_, ChildContent::Real) => ChildContent::Real,
            (ChildContent::Open, _) | (_, ChildContent::Open) => ChildContent::Open,
            _ => ChildContent::Blank,
        };
    }
    content
}

/// Each content prop written on the element, and whether its value is proven
/// to put content there: a non-empty literal, or JSX.
fn content_values<'a>(file: &'a FileFacts, element: &JsxElementFact) -> HashMap<&'a str, bool> {
    let mut values = HashMap::default();
    let filled = |span: Option<Span>| {
        span.is_some_and(|span| {
            expression_is_static_literal(file, span)
                && !matches!(text(file, span).trim(), "\"\"" | "''" | "{\"\"}" | "{''}")
        })
    };
    for attribute in &element.attributes {
        let name = text(file, attribute.name);
        if CONTENT_ATTRIBUTES.contains(&name) {
            let certain = match attribute.value_kind {
                JsxAttributeValueKind::Boolean
                | JsxAttributeValueKind::Element
                | JsxAttributeValueKind::Fragment => true,
                JsxAttributeValueKind::String => filled(attribute.value),
                JsxAttributeValueKind::Expression => filled(attribute.expression),
            };
            values.insert(name, certain);
        }
    }
    for spread in &element.spreads {
        for property in super::direct_object_literal_properties(file, spread.argument)
            .unwrap_or_default()
            .into_iter()
            .filter(|property| !property.computed)
        {
            let name = text(file, property.key).trim_matches(['\'', '"']);
            if CONTENT_ATTRIBUTES.contains(&name) {
                values.insert(name, property.data && filled(Some(property.value)));
            }
        }
    }
    values
}

/// What one spread may add to an element's content.
enum SpreadKeys {
    /// An object literal read whole, or declared keys with no content key.
    Closed,
    Declared(Vec<&'static str>),
    Any,
}

fn spread_content_keys<'a>(
    file: &FileFacts,
    argument: Span,
    declared_keys: &impl Fn(Span) -> Option<&'a [Arc<str>]>,
) -> SpreadKeys {
    if let Some(properties) = super::direct_object_literal_properties(file, argument)
        && properties.iter().all(|property| !property.computed)
        && !file
            .ast
            .spreads
            .iter()
            .any(|spread| argument.contains(spread.span))
    {
        return SpreadKeys::Closed;
    }
    let Some(keys) = declared_keys(argument) else {
        return SpreadKeys::Any;
    };
    let keys = CONTENT_ATTRIBUTES
        .into_iter()
        .filter(|name| keys.iter().any(|key| key.as_ref() == *name))
        .collect::<Vec<_>>();
    if keys.is_empty() {
        SpreadKeys::Closed
    } else {
        SpreadKeys::Declared(keys)
    }
}

/// The events the 1.x compiler delegates by default, byte-faithful to
/// `delegated_events` in the pinned compiler port
/// (`solid-1x-compiler@79b9b637`, `packages/compiler/src/shared/constants.rs`,
/// itself extracted from `babel-plugin-jsx-dom-expressions@0.40.7`). A plain
/// `on*` prop naming one of these lowers to the property write
/// `el.$$event = handler`, where a later occurrence overwrites an earlier
/// one; every other event spelling attaches via `addEventListener` and
/// never collides.
const DELEGATED_EVENTS: &[&str] = &[
    "beforeinput",
    "click",
    "contextmenu",
    "dblclick",
    "focusin",
    "focusout",
    "input",
    "keydown",
    "keyup",
    "mousedown",
    "mousemove",
    "mouseout",
    "mouseover",
    "mouseup",
    "pointerdown",
    "pointermove",
    "pointerout",
    "pointerover",
    "pointerup",
    "touchend",
    "touchmove",
    "touchstart",
];

/// The single-winner slot one prop occurrence competes in after the 1.x
/// compiler lowers it, or `None` when every occurrence stays live and no
/// duplicate exists.
///
/// Upstream's `jsx-no-duplicate-props` folds every `on*` spelling onto one
/// lowercase name — `onClick`, `onclick`, `on:click`, and `oncapture:click`
/// all become `onclick` — and reports the pair as a duplicate. The pinned
/// 1.x compiler disagrees (`packages/compiler/src/dom/attrs.rs` and
/// `dom/events.rs` at `solid-1x-compiler@79b9b637`, byte-faithful to
/// `babel-plugin-jsx-dom-expressions@0.40.7`):
///
/// - `on:evt` lowers to `_$addEventListener(el, "evt", h)` (bubble) and
///   `oncapture:evt` to `el.addEventListener("evt", h, true)` (capture) —
///   different mechanisms, and every occurrence attaches;
/// - a plain `on*` naming a non-delegated event lowers to its own
///   `addEventListener` call per occurrence — all of them fire;
/// - a plain `on*` naming a delegated event lowers to the property write
///   `el.$$evt = handler` — the one later-wins slot among event spellings;
/// - a statically string/number-valued or bare `on*` prop never becomes a
///   listener at all: the compiler freezes it into the template, where the
///   HTML parser keeps the first occurrence of an attribute name.
///
/// So only the last two forms have a single-winner slot, and only same-slot
/// pairs are duplicates. The upstream parity corpus has no case pinning the
/// folding, so this divergence is corpus-invisible; the rule page records it.
///
/// Non-event names keep upstream's shape: an `attr:`/`prop:` prefix is
/// stripped (the underlying attribute/property slot is shared with the plain
/// spelling), everything else compares as written. `ignoreCase` is the one
/// upstream option not carried (see the module doc).
///
/// **All of the above is DOM lowering.** When `intrinsic` is false the tag is
/// a component, which the compiler never lowers: its props are collected into
/// a plain object, so the slot is the key exactly as written and every
/// occurrence competes — `onSave` twice, or `on:click` twice, is a real
/// later-wins duplicate that the DOM model would wrongly excuse.
fn duplicate_slot(
    original: &str,
    value_is_static_literal: bool,
    intrinsic: bool,
) -> Option<String> {
    if !intrinsic {
        return Some(original.to_owned());
    }
    if let Some(suffix) = original.strip_prefix("on") {
        if original.starts_with("on:") || original.starts_with("oncapture:") {
            return None;
        }
        let event = suffix.to_ascii_lowercase();
        if value_is_static_literal {
            // The template attribute slot, shared with an `attr:on...`
            // spelling of the same name (both freeze into the template).
            return Some(format!("on{event}"));
        }
        if DELEGATED_EVENTS.contains(&event.as_str()) {
            return Some(format!("delegated:{event}"));
        }
        return None;
    }
    let stripped = original
        .strip_prefix("attr:")
        .or_else(|| original.strip_prefix("prop:"))
        .unwrap_or(original);
    Some(stripped.to_owned())
}

/// Whether the compiler freezes this attribute value into the template.
///
/// `classify_plan`'s inline branch (pinned `solid-1x-compiler@79b9b637`,
/// `packages/compiler/src/dom/attrs.rs`) matches exactly two expression
/// node kinds — `Expression::StringLiteral` and `Expression::NumericLiteral`
/// — and sends everything else, `BooleanLiteral` included, down the runtime
/// path. So the test is a *node-kind* test, not a "does this text look like
/// a number" test, and the two differ in both directions:
///
/// - `{-1}` and `{+1}` are `UnaryExpression`s, and `{NaN}`/`{Infinity}` are
///   `Identifier`s. None is a `NumericLiteral`; all lower dynamically.
/// - `{0x10}` and `{1_000}` *are* `NumericLiteral`s. The compiler inlines
///   them as `format_number(literal.value)` — the numeric value, `16` and
///   `1000`, not the authored lexeme — so they freeze into the template just
///   like `{16}` does.
///
/// This file has spans, not nodes, so the predicate recognizes the
/// `NumericLiteral` *lexeme* grammar instead: decimal (with optional
/// fraction and exponent), and the `0x`/`0o`/`0b` and legacy-octal radix
/// forms, each allowing `_` separators. A `BigInt` (`1n`) is a distinct node
/// kind and is rejected. The string-literal path is unchanged.
pub(super) fn expression_is_static_literal(file: &FileFacts, span: Span) -> bool {
    let trimmed = text(file, span).trim();
    let trimmed = trimmed
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .map(str::trim)
        .unwrap_or(trimmed);
    if trimmed.starts_with('`') {
        return false;
    }
    is_numeric_literal(trimmed) || super::strip_string_literal(trimmed).is_some()
}

/// Whether `text` is a JavaScript `NumericLiteral` lexeme — the grammar
/// Babel and Oxc both parse into a `NumericLiteral` node. Deliberately
/// stricter than `str::parse::<f64>` (which accepts the sign, `NaN`, `inf`,
/// and `Infinity`, none of which is a literal) and deliberately broader
/// (`parse` rejects `0x10` and `1_000`, which are literals).
fn is_numeric_literal(text: &str) -> bool {
    let digits = |body: &str, radix: u32| {
        !body.is_empty()
            && !body.starts_with('_')
            && !body.ends_with('_')
            && !body.contains("__")
            && body
                .chars()
                .all(|character| character == '_' || character.is_digit(radix))
    };
    if let Some(body) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return digits(body, 16);
    }
    if let Some(body) = text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")) {
        return digits(body, 8);
    }
    if let Some(body) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        return digits(body, 2);
    }
    // Legacy octal (`0755`) — still a NumericLiteral outside strict mode,
    // and the compiler inlines its value like any other.
    if text.len() > 1 && text.starts_with('0') && text[1..].chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => {
            let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
            (mantissa, Some(exponent))
        }
        None => (text, None),
    };
    if exponent.is_some_and(|exponent| !digits(exponent, 10)) {
        return false;
    }
    match mantissa.split_once('.') {
        // `1.`, `.5`, and `1.5` are all literals; a bare `.` is not.
        Some((whole, fraction)) => {
            (digits(whole, 10) || whole.is_empty())
                && (digits(fraction, 10) || fraction.is_empty())
                && !(whole.is_empty() && fraction.is_empty())
        }
        None => digits(mantissa, 10),
    }
}

#[cfg(test)]
mod tests;
