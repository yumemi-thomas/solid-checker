//! `v1/jsx-no-duplicate-props` — eslint-plugin-solid's structural duplicate
//! JSX property rule, ported from the 1.x reactive solver's
//! `solid_1_rules.rs` onto this checker's fact tables.
//!
//! Every rule here reads `file.ast.jsx_elements` and its nested attribute /
//! spread / object-property tables. `jsx-no-duplicate-props { ignoreCase }`
//! is the one option upstream ships here that the checker does not carry: no
//! upstream corpus case exercises a behaviour difference for it, and an option
//! nothing proves is a knob that can silently rot.

use std::collections::{HashMap, HashSet};

use solid_facts::FileFacts;
use solid_facts::ast::{JsxAttributeValueKind, JsxElementFact};
use solid_facts::core::Span;

use super::{UpstreamCompatContext, is_lowercase_led, text, violation};
use crate::StaticViolation;

pub(super) fn check_file(
    file: &FileFacts,
    _context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    for element in &file.ast.jsx_elements {
        jsx_no_duplicate_props(file, element, violations);
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
    //
    // The slot model below describes what the *DOM* compiler does to an
    // intrinsic element's props. A component never reaches that lowering: its
    // props become a plain object, where the only slot is the key itself and
    // a later `onSave` overwrites an earlier one no matter how either is
    // spelled. Applying the DOM model there would silence real duplicates.
    let intrinsic = is_lowercase_led(text(file, element.name.span));
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
                duplicate_slot(name, static_literal, intrinsic),
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
                            intrinsic,
                        ),
                        name,
                    )
                }),
        );
    }
    candidates.sort_by_key(|(span, ..)| (span.start, span.end));

    let mut names: HashMap<String, &str> = HashMap::new();
    let mut seen_slots = HashSet::new();
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
            if written == first_written || !intrinsic {
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
    let has_inner_html = seen_slots.contains("innerHTML") || seen_slots.contains("innerhtml");
    let has_text_content = seen_slots.contains("textContent");
    let used = [
        has_children_prop.then_some("`props.children`"),
        has_children.then_some("JSX children"),
        has_inner_html.then_some("`props.innerHTML`"),
        has_text_content.then_some("`props.textContent`"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    // The `children`-prop-plus-JSX-children pair, and *only* that pair, is
    // TS2710: "'children' are specified twice. The attribute named 'children'
    // will be overwritten." -- word for word this arm's claim, in both passes and
    // on components as well as intrinsic elements. Narrowed 2026-08-17 after
    // `scripts/parity-tsc-ownership.mjs` matched the two spans.
    //
    // Any intrinsic set that also includes `innerHTML` or `textContent` still reports: those
    // conflicts draw no diagnostic at all, so the finding asserts more than
    // TS2710 does even where TS2710 also fires. Verified: `innerHTML` with
    // `textContent`, and `innerHTML` with JSX children, are both silent. On a
    // component these are ordinary props that its implementation may combine.
    let only_the_children_pair = used.len() == 2
        && has_children_prop
        && has_children
        && !has_inner_html
        && !has_text_content;
    if intrinsic && used.len() > 1 && !only_the_children_pair {
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
mod tests {
    use super::{duplicate_slot, is_lowercase_led, is_numeric_literal};

    /// The compiler lowers delegated events to `el.$$event = handler`, a
    /// property write where a later occurrence overwrites an earlier one —
    /// the one event spelling with a single-winner slot, folded
    /// case-insensitively because `to_event_name` lowercases the suffix.
    #[test]
    fn delegated_event_spellings_share_one_later_wins_slot() {
        assert_eq!(
            duplicate_slot("onClick", false, true),
            duplicate_slot("onclick", false, true)
        );
        assert!(duplicate_slot("onClick", false, true).is_some());
    }

    /// `on:evt` (bubble) and `oncapture:evt` (capture) lower to separate
    /// `addEventListener` calls — both attach, nothing is overwritten, so
    /// neither competes in any slot. Upstream folds all four spellings onto
    /// one name; the pinned compiler proves that wrong.
    #[test]
    fn listener_namespace_spellings_never_collide() {
        assert_eq!(duplicate_slot("on:click", false, true), None);
        assert_eq!(duplicate_slot("oncapture:click", false, true), None);
    }

    /// A non-delegated event attaches one listener per occurrence; both
    /// fire, so a repeated `onMouseEnter` is runtime-legal, not a dead prop.
    /// Names that merely start with `on` (`once`, `only`) are events to the
    /// compiler too, and non-delegated ones land here.
    #[test]
    fn non_delegated_events_never_collide() {
        assert_eq!(duplicate_slot("onMouseEnter", false, true), None);
        assert_eq!(duplicate_slot("once", false, true), None);
    }

    /// A statically string/number-valued (or bare) `on*` prop is frozen into
    /// the template, where the HTML parser keeps the first occurrence — a
    /// first-wins slot shared with the `attr:` spelling of the same name.
    #[test]
    fn static_valued_event_names_share_the_template_attribute_slot() {
        assert_eq!(
            duplicate_slot("onClick", true, true),
            Some("onclick".to_owned())
        );
        assert_eq!(
            duplicate_slot("onClick", true, true),
            duplicate_slot("attr:onclick", false, true)
        );
        // Delegated handler and template attribute are different slots:
        // the inline HTML handler and the `$$click` handler both fire.
        assert_ne!(
            duplicate_slot("onClick", true, true),
            duplicate_slot("onClick", false, true)
        );
    }

    #[test]
    fn strips_attr_and_prop_prefixes_onto_the_shared_slot() {
        assert_eq!(
            duplicate_slot("attr:title", false, true),
            Some("title".to_owned())
        );
        assert_eq!(
            duplicate_slot("prop:value", false, true),
            Some("value".to_owned())
        );
        assert_eq!(
            duplicate_slot("attr:title", false, true),
            duplicate_slot("title", true, true)
        );
    }

    #[test]
    fn leaves_names_that_are_not_on_prefixed_or_attr_prop_prefixed_alone() {
        assert_eq!(
            duplicate_slot("class", true, true),
            Some("class".to_owned())
        );
        assert_eq!(duplicate_slot("id", false, true), Some("id".to_owned()));
    }

    /// A component's props are a plain object the compiler never lowers, so
    /// the slot is the key as written: repeats collide whatever the DOM
    /// model would have said about them, and differently-spelled names do
    /// not.
    #[test]
    fn component_props_compare_by_key_identity() {
        for name in ["onSave", "on:click", "oncapture:click", "onMouseEnter"] {
            assert_eq!(
                duplicate_slot(name, false, false),
                Some(name.to_owned()),
                "{name} must keep its own slot on a component"
            );
        }
        // No DOM aliasing: these are three distinct keys on a props object.
        assert_ne!(
            duplicate_slot("onClick", false, false),
            duplicate_slot("onclick", false, false)
        );
        assert_ne!(
            duplicate_slot("attr:title", false, false),
            duplicate_slot("title", false, false)
        );
        // And the static-value distinction is DOM lowering too.
        assert_eq!(
            duplicate_slot("onClick", true, false),
            duplicate_slot("onClick", false, false)
        );
    }

    /// The compiler's inline branch matches `Expression::NumericLiteral`
    /// nodes, so the predicate has to follow the literal *grammar*: a signed
    /// or named value is a different node and lowers dynamically, while the
    /// radix and separator forms are literals the compiler freezes.
    #[test]
    fn numeric_literal_lexemes_match_the_compiler_node_kind() {
        for literal in [
            "0", "1", "42", "1_000", "0x10", "0X1f", "0b1010", "0o17", "0755", "1.5", "1.", ".5",
            "1e3", "1E-3", "1.5e+10",
        ] {
            assert!(is_numeric_literal(literal), "{literal} is a NumericLiteral");
        }
        for other in [
            "-1",
            "+1",
            "NaN",
            "inf",
            "Infinity",
            "-Infinity",
            "1n",
            "0x",
            "_1",
            "1__0",
            "1_",
            ".",
            "",
            "1e",
            "0xg",
            "true",
        ] {
            assert!(
                !is_numeric_literal(other),
                "{other} is not a NumericLiteral"
            );
        }
    }
    #[test]
    fn classifies_element_names_by_leading_case() {
        assert!(is_lowercase_led("div"));
        assert!(!is_lowercase_led("Component"));
        assert!(!is_lowercase_led("Foo.Bar"));
    }
}
