//! ADR 1146: a `change` handler of a literal checkbox or radio input keeps the host-delivery proof.
//!
//! ADR 0642 keeps `change` out of [`crate::runtime_semantics::event_keeps_delivery_proof`]: a text control (and a select) delivers it by a state change with no
//! call to name, whatever makes the focused element leave (`blur()`, a move, a removal, `disabled = true`, Solid's own unmount), on the stack of whatever made
//! it. So a handler of `change` may run inside an owned scope, and the placement of its body is unknown (`execution_role.denied-event`).
//!
//! A checkbox or a radio is delivered `change` by its click activation only. Measured in Chrome 148 (the runtime oracle's headless shell 1223; the scenarios and
//! their events are in `docs/decisions.md`, ADR 1146): a user's click, the Space key (a checkbox) and the arrow keys (a radio group) deliver `click`, `input` and
//! `change` on the host's stack; `el.click()`, `label.click()` and `dispatchEvent(new MouseEvent("click"))` deliver them synchronously inside the call, and
//! `dispatchEvent(new Event("change"))` delivers the one event; nothing else does. A write of `checked`, `form.reset()`, `blur()`, `focus()` elsewhere, `remove()`,
//! `replaceWith()` and `disabled = true` on the focused control deliver no `change`. Every deliverer is the host (an empty stack) or a call the dispatch census
//! already counts (`HTMLElement.click`, `EventTarget.dispatchEvent`: [`crate::runtime_semantics::delivers_synchronously`]), exactly as for `click`, so the veto that
//! withholds a `click` handler's proofs where a dispatch may deliver inside an owned scope (ADR 1125) withholds this one's too.
//!
//! That holds only for a control that stays a checkbox or a radio. Retyped to text (`type = "text"`, `setAttribute("type", …)`, `removeAttribute("type")`) the same
//! element delivers `change` at `blur()`, `remove()`, `focus()` elsewhere and the rest, synchronously, as a text control does (measured), and then its handler may
//! run anywhere. So the proof asks, of the element: an intrinsic `<input>` with one plain `type` attribute that is the string `checkbox` or `radio`, no spread, no
//! `ref`, no `is`, and no namespaced attribute that can write the control (`use:`, `prop:`, `attr:`, `bool:`); and of the program: that delivery is intact (no
//! Proxy, `Reflect`, `with`, `Object.assign` onto a value not made in place, definer or code built from a string: ADR 1125's premise, which these would break) and
//! that no project file may write an element's `type` ([`program_may_retype`]). Installed code does not retype an element it was not handed (ADR 0300).
//!
//! The permit sits in [`crate::execution_role::handler_event_denied`], read by every consumer of a handler's event, and ablating it
//! (`SOLID_CHECKER_IR_ABLATE_ARM=change_delivery.checkable-input-activation`, or the same id as a silence) denies `change` again, as before. Each decision that
//! relies on it names the arm in the proof log.

use std::sync::OnceLock;

use solid_facts::FileFacts;
use solid_facts::ast::{ComputedKeySite, JsxAttributeValueKind, JsxElementFact};
use solid_facts::core::Span;

use crate::ExecutionRole;
use crate::indexes::SemanticLookup;
use crate::role_log::role_arm;

/// The id of the proof's arm: the proof log names it, and `SOLID_CHECKER_IR_ABLATE_ARM` (or `SOLID_CHECKER_IR_ABLATE_SILENCE`) turns the permit off.
pub(crate) const ACTIVATION_ONLY: &str = "change_delivery.checkable-input-activation";

/// The values of a `type` attribute whose control a `change` reaches by click activation alone.
const CHECKABLE_TYPES: [&str; 2] = ["checkbox", "radio"];

/// The names of the DOM's attribute API, whose mention anywhere in a project file may retype a control (`el.setAttribute("type", …)`, an alias, a pattern
/// key, `el.attributes.getNamedItem("type").value = …`): a program that spells one is no program whose controls keep their type.
const ATTRIBUTE_API: [&str; 16] = [
    "setAttribute",
    "setAttributeNS",
    "removeAttribute",
    "removeAttributeNS",
    "toggleAttribute",
    "setAttributeNode",
    "setAttributeNodeNS",
    "removeAttributeNode",
    "getAttributeNode",
    "getAttributeNodeNS",
    "attributes",
    "getNamedItem",
    "getNamedItemNS",
    "setNamedItem",
    "removeNamedItem",
    "removeNamedItemNS",
];

/// Whether the permit is on: not ablated (as an arm, for the monotone gate's M2b, or as a silence).
fn enabled() -> bool {
    static ARM_ABLATED: OnceLock<bool> = OnceLock::new();
    !crate::site_accounting::ablated_id(ACTIVATION_ONLY)
        && !*ARM_ABLATED.get_or_init(|| {
            std::env::var("SOLID_CHECKER_IR_ABLATE_ARM").is_ok_and(|id| id == ACTIVATION_ONLY)
        })
}

/// Whether the attribute `attribute` (`onChange`, `on:change`, `onchange`) names the `change` event.
fn is_change(attribute: &str) -> bool {
    let name = attribute.to_ascii_lowercase();
    let name = name.strip_prefix("on").unwrap_or(&name);
    name.strip_prefix(':').unwrap_or(name) == "change"
}

/// Whether `callback` is the value of the `change` handler attribute of a literal checkbox or radio (module docs): the syntax half of the permit, no program asked.
pub(crate) fn checkable_change_handler(file: &FileFacts, callback: Span) -> bool {
    file.ast.jsx_containing(callback).any(|element| {
        element.attributes.iter().any(|attribute| {
            attribute
                .expression
                .or(attribute.value)
                .is_some_and(|value| value.contains(callback))
                && file.source_text(attribute.name).is_some_and(is_change)
                && checkable_input(file, element)
        })
    })
}

/// The program half of the permit: delivery is intact and no project file may write an element's `type`, and the permit is not ablated. Names the arm in the
/// decision it answers.
pub(crate) fn program_keeps_control_types(lookup: &SemanticLookup<'_>) -> bool {
    if !enabled() || !lookup.event_delivery_intact() || lookup.element_retype_possible() {
        return false;
    }
    let _ = role_arm!(
        "change_delivery.checkable-input-activation",
        ExecutionRole::EventCallback
    );
    true
}

/// An intrinsic `<input>` with exactly one plain `type` attribute, the string `checkbox` or `radio`, and nothing that can retype or hand out the control: no
/// spread, no `ref`, no `is`, no `use:`, `prop:`, `attr:` or `bool:` attribute.
fn checkable_input(file: &FileFacts, element: &JsxElementFact) -> bool {
    if !crate::object_members::intrinsic_element(file, element)
        || file.source_text(element.name.span) != Some("input")
        || !element.spreads.is_empty()
    {
        return false;
    }
    let mut checkable = false;
    for attribute in &element.attributes {
        let local = file.source_text(attribute.local_name).unwrap_or_default();
        match attribute
            .namespace
            .map(|namespace| file.source_text(namespace).unwrap_or_default())
        {
            None => match local {
                "type" => {
                    let literal = attribute.value_kind == JsxAttributeValueKind::String
                        && attribute
                            .value
                            .and_then(|value| file.source_text(value))
                            .map(|text| text.trim_matches(['"', '\'']))
                            .is_some_and(|text| {
                                CHECKABLE_TYPES
                                    .iter()
                                    .any(|checkable| text.eq_ignore_ascii_case(checkable))
                            });
                    if checkable || !literal {
                        return false;
                    }
                    checkable = true;
                }
                "ref" | "is" => return false,
                _ => {}
            },
            // `on:change` is the listener itself; `class:` and `style:` set no property a control's type rests on.
            Some("on" | "oncapture" | "class" | "style") => {}
            Some(_) => return false,
        }
    }
    checkable
}

/// Whether any project file may write the `type` of an element: a write whose key is `type`, or is a computed one no proof narrows away from it
/// ([`write_may_be_type`]), or a file that spells a name of the DOM's attribute API ([`ATTRIBUTE_API`]). Installed code is not read (ADR 0300): it does not retype an
/// element it was not handed. The reflective routes (`Object.assign`, `defineProperty`, `Reflect.set`, `with`, code from a string) are the delivery premise's, asked
/// beside this answer.
pub(crate) fn program_may_retype(lookup: &SemanticLookup<'_>) -> bool {
    lookup
        .files()
        .iter()
        .filter(|file| !crate::plain_premise::is_package(file.path.as_str()))
        .any(|file| {
            let ast = &file.ast;
            ast.write_targets
                .iter()
                .chain(&ast.deleted_targets)
                .any(|target| write_may_be_type(file, *target))
                || ATTRIBUTE_API
                    .iter()
                    .any(|name| spells_word(&file.source, name))
        })
}

/// Whether the member written at `target` (an assignment, update, destructuring or iteration target, or a deleted operand) may be the property `type`: its name
/// is `type`; its key is a literal `"type"`; or its key is no literal and nothing proves it is another string.
fn write_may_be_type(file: &FileFacts, target: Span) -> bool {
    let ast = &file.ast;
    let target = ast.peel_ts_sugar_span(target);
    let at = ast.computed_keys.partition_point(|key| key.read < target);
    if let Some(key) = ast.computed_keys[at..]
        .iter()
        .take_while(|key| key.read == target)
        .find(|key| key.site == ComputedKeySite::Member)
    {
        return !key.proven || key.texts.iter().any(|text| text.as_str() == "type");
    }
    if let Some(literal) = ast
        .literal_computed_members
        .iter()
        .find(|member| member.span == target)
    {
        return literal.key.as_str() == "type";
    }
    ast.member_at(target)
        .and_then(|member| file.source_text(member.property))
        == Some("type")
}

/// Whether `source` holds `word` as an identifier: not part of a longer one.
fn spells_word(source: &str, word: &str) -> bool {
    let identifier = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    source.match_indices(word).any(|(at, _)| {
        !source[..at].chars().next_back().is_some_and(identifier)
            && !source[at + word.len()..]
                .chars()
                .next()
                .is_some_and(identifier)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arm_id_the_proof_log_names_is_the_one_ablation_names() {
        // `role_arm!` takes a literal, and the ablation a constant: they are one string.
        assert_eq!(
            ACTIVATION_ONLY,
            "change_delivery.checkable-input-activation"
        );
    }

    #[test]
    fn change_is_named_by_its_attribute_forms_only() {
        for name in ["onChange", "onchange", "on:change", "onCHANGE"] {
            assert!(is_change(name), "{name}");
        }
        for name in [
            "onInput",
            "oncapture:change",
            "change",
            "onChanged",
            "onclick",
            "",
        ] {
            // `change` bare is no attribute of a handler: the compiler reads an event as `on` and a name.
            assert_eq!(is_change(name), name == "change", "{name}");
        }
    }

    /// The file the source makes, as the analysis holds one (no compiler facts: the syntax half reads none).
    fn file(source: &str) -> FileFacts {
        use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
        use solid_facts::core::{Generation, SourceHash, SourcePath};
        let generation = Generation::new(1).unwrap();
        FileFacts {
            generation,
            path: SourcePath::new("app.tsx").unwrap(),
            source_hash: SourceHash::of(source),
            source: std::sync::Arc::from(source),
            ast: std::sync::Arc::new(solid_facts::ast::extract("app.tsx", source).unwrap()),
            compiler: std::sync::Arc::new(ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: Vec::new(),
                untracked_regions: Vec::new(),
                discarded_regions: Vec::new(),
                ownership_regions: Vec::new(),
                callback_roles: Vec::new(),
                jsx_operations: Vec::new(),
            }),
        }
    }

    /// Whether the arrow `() => {}` written in `source` is the `change` handler of a literal checkbox or radio.
    fn takes_the_proof(source: &str) -> bool {
        let at = source.find("() => {}").expect("the handler is written");
        let handler = Span::new(
            u32::try_from(at).unwrap(),
            u32::try_from(at + "() => {}".len()).unwrap(),
        );
        checkable_change_handler(&file(source), handler)
    }

    #[test]
    fn only_a_literal_checkbox_or_radio_with_nothing_to_retype_it_takes_the_proof() {
        for element in [
            "<input type=\"checkbox\" onChange={() => {}} />",
            "<input onChange={() => {}} type=\"radio\" />",
            "<input type=\"CheckBox\" onChange={() => {}} />",
            "<input type='radio' name=\"g\" checked={x()} onChange={() => {}} class:on={y()} style:color=\"red\" />",
            "<input type=\"checkbox\" onchange={() => {}} />",
        ] {
            assert!(
                takes_the_proof(&format!("const e = {element};")),
                "{element}"
            );
        }
        for element in [
            // Another control, or none a `change` reaches by activation alone.
            "<input onChange={() => {}} />",
            "<input type=\"text\" onChange={() => {}} />",
            "<input type=\"search\" onChange={() => {}} />",
            "<select onChange={() => {}} />",
            "<button type=\"checkbox\" onChange={() => {}} />",
            "<Input type=\"checkbox\" onChange={() => {}} />",
            "<ui.input type=\"checkbox\" onChange={() => {}} />",
            // A type that is no literal, or may change: a signal, a second `type`, a spread, a `ref`, `is`, a namespaced writer.
            "<input type={kind()} onChange={() => {}} />",
            "<input type={'checkbox'} onChange={() => {}} />",
            "<input type=\"checkbox\" type=\"text\" onChange={() => {}} />",
            "<input type=\"checkbox\" {...p} onChange={() => {}} />",
            "<input type=\"checkbox\" ref={r} onChange={() => {}} />",
            "<input type=\"checkbox\" is=\"x-box\" onChange={() => {}} />",
            "<input type=\"checkbox\" use:d onChange={() => {}} />",
            "<input type=\"checkbox\" prop:type=\"text\" onChange={() => {}} />",
            "<input type=\"checkbox\" attr:type=\"text\" onChange={() => {}} />",
            "<input type=\"checkbox\" bool:disabled={true} onChange={() => {}} />",
            // Another event.
            "<input type=\"checkbox\" onInput={() => {}} />",
            "<input type=\"checkbox\" onClick={() => {}} />",
        ] {
            assert!(
                !takes_the_proof(&format!("const e = {element};")),
                "{element}"
            );
        }
        // The handler is the attribute's value: an arrow elsewhere in the element is none.
        assert!(!takes_the_proof(
            "const e = <input type=\"checkbox\" title={(() => {})()} onChange={f} />;"
        ));
    }

    #[test]
    fn words_are_matched_whole() {
        assert!(spells_word("el.setAttribute('a', 1)", "setAttribute"));
        assert!(spells_word("const { attributes } = el;", "attributes"));
        assert!(!spells_word("mySetAttribute()", "setAttribute"));
        assert!(!spells_word("el.attributesList", "attributes"));
        assert!(!spells_word("$attributes", "attributes"));
    }
}
