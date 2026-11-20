//! ADR 1134: an owner obligation written in a delegated event handler of a form control that is disabled from its mount, and
//! that nothing in the program is seen to enable, is not proven to run.
//!
//! The dialect says its delegated dispatcher passes over a node whose `disabled` property is true
//! ([`solid_dialect::Dialect::delegated_handler_skips_disabled_node`]; rc.21 `@solidjs/web` `eventHandler`, `!node.disabled`). The
//! compiler's own facts say which handlers it delegated (`GeneratedCompilerOperationKind::DelegatedEvent`). `disabled` reflects the
//! attribute on the HTML controls that have the property (`button`, `input`, `select`, `textarea`, `fieldset`, `optgroup`, `option`);
//! on any other element it is no property, and the handler runs. A violation there would claim a run no event reaches (the 2026-11-17
//! ruling, ADR 1018: a check no run reaches gives no violation), so the row becomes an obligation (`handler-on-disabled-control`).
//!
//! Only a row that would be a violation is touched, and the change is from a violation to an obligation, never to a clean. The proof
//! asks: the operation is written inside the value of the element's own `on…` attribute (not a function it names); the compiler
//! delegated that handler; the element is one of the controls above, with a bare `disabled` or `disabled={true}`, no other `disabled`
//! (a namespaced `prop:`/`attr:` one included), no spread and no `ref`; and no file of the program writes a `.disabled` member or calls
//! an attribute writer (`setAttribute`, `removeAttribute`, `toggleAttribute` and their `NS`/`Node` forms) on anything. A handler the
//! compiler did not delegate (`on:click`, an event outside the delegated set) is the browser's listener, and stays as it is: the
//! browser delivers a dispatched `click` to a disabled button's own listener (Chrome 148, measured), and pointer, wheel and
//! `contextmenu` events from the user too.

use solid_facts::FileFacts;
use solid_facts::ast::{JsxAttributeValueKind, JsxElementFact};
use solid_facts::compiler::GeneratedCompilerOperationKind;
use solid_facts::core::Span;

use crate::Program;
use crate::indexes::SemanticLookup;

/// The HTML elements whose `disabled` IDL attribute reflects the content attribute.
const DISABLEABLE: &[&str] = &[
    "button", "input", "select", "textarea", "fieldset", "optgroup", "option",
];

/// The DOM methods that write an attribute, by name, whatever their receiver.
const ATTRIBUTE_WRITERS: &[&str] = &[
    "setAttribute",
    "setAttributeNS",
    "removeAttribute",
    "removeAttributeNS",
    "toggleAttribute",
    "setAttributeNode",
    "setAttributeNodeNS",
    "removeAttributeNode",
];

/// Marks each owner violation written in a delegated handler of a control disabled for its whole life (module docs).
pub(crate) fn mark(program: &mut Program, lookup: &SemanticLookup<'_>) {
    if !lookup.dialect.delegated_handler_skips_disabled_node() {
        return;
    }
    let mut enabled_somewhere = None::<bool>;
    for requirement in &mut program.missing_owners {
        if requirement.uncertain || !requirement.report {
            continue;
        }
        let Some(file) = lookup.file_by_path(requirement.location.path.as_ref()) else {
            continue;
        };
        let (Ok(start), Ok(end)) = (
            u32::try_from(requirement.location.start_byte),
            u32::try_from(requirement.location.end_byte),
        ) else {
            continue;
        };
        let site = Span::new(start, end);
        if !in_delegated_handler_of_disabled_control(file, site) {
            continue;
        }
        if *enabled_somewhere.get_or_insert_with(|| program_may_enable(lookup)) {
            return;
        }
        requirement.uncertain = true;
        requirement.handler_on_disabled_control = true;
    }
}

/// Whether `site` is written in the value of an `on…` attribute of a disabled control that the compiler delegated.
fn in_delegated_handler_of_disabled_control(file: &FileFacts, site: Span) -> bool {
    // A recorded delegation is the compiler's own answer for that handler; completeness (`generated_operations_complete`) is
    // what an absent operation would need, and no absence is read here.
    let model = &file.compiler.semantic_model;
    if model.refusal.is_some() {
        return false;
    }
    file.ast.jsx_elements.iter().any(|element| {
        statically_disabled_control(file, element)
            && element.attributes.iter().any(|attribute| {
                attribute.namespace.is_none()
                    && attribute.value.is_some_and(|value| value.contains(site))
                    && file
                        .source_text(attribute.local_name)
                        .is_some_and(is_event_attribute)
                    && model.generated_operations.iter().any(|operation| {
                        operation.kind == GeneratedCompilerOperationKind::DelegatedEvent
                            && attribute.span.contains(operation.source_span)
                    })
            })
    })
}

/// ADR 1129 beside ADR 1134: whether a handler a `merge`/`omit` view spreads onto `element` may meet a `disabled` the dispatcher
/// honors. True for one of [`DISABLEABLE`] that writes any `disabled` attribute, holds another spread (which may carry one), or
/// whose view may own a `disabled` key. The view proof then leaves the spread's handlers unresolved: a handler that may never run
/// is no violation, and the obligation stays.
pub(crate) fn view_spread_may_meet_disabled(
    file: &FileFacts,
    element: &JsxElementFact,
    view_keys: &[std::sync::Arc<str>],
) -> bool {
    element.member_object.is_none()
        && file
            .source_text(element.name.span)
            .is_some_and(|tag| DISABLEABLE.contains(&tag))
        && (element.spreads.len() > 1
            || view_keys.iter().any(|key| key.as_ref() == "disabled")
            || element
                .attributes
                .iter()
                .any(|attribute| file.source_text(attribute.local_name) == Some("disabled")))
}

/// `onClick`: `on` and an upper-case letter, the form the compiler reads as an event (`onclick` is a property, `on:click` a listener).
fn is_event_attribute(name: &str) -> bool {
    name.strip_prefix("on")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|first| first.is_ascii_uppercase())
}

/// One of [`DISABLEABLE`], written with a single `disabled` that is bare or `{true}`, no spread and no `ref`.
fn statically_disabled_control(file: &FileFacts, element: &JsxElementFact) -> bool {
    if element.member_object.is_some()
        || !file
            .source_text(element.name.span)
            .is_some_and(|tag| DISABLEABLE.contains(&tag))
        || !element.spreads.is_empty()
    {
        return false;
    }
    let named = |attribute: &&solid_facts::ast::JsxAttributeFact, name: &str| {
        file.source_text(attribute.local_name) == Some(name)
    };
    if element
        .attributes
        .iter()
        .any(|attribute| named(&attribute, "ref"))
    {
        return false;
    }
    let mut disabled = element
        .attributes
        .iter()
        .filter(|attribute| named(attribute, "disabled"));
    let (Some(only), None) = (disabled.next(), disabled.next()) else {
        return false;
    };
    only.namespace.is_none()
        && match only.value_kind {
            JsxAttributeValueKind::Boolean => true,
            JsxAttributeValueKind::Expression => only
                .expression
                .and_then(|expression| file.source_text(file.ast.peel_ts_sugar_span(expression)))
                .is_some_and(|text| text.trim() == "true"),
            _ => false,
        }
}

/// Whether any file of the program writes a `.disabled` member or calls an attribute writer: then a control may be enabled.
fn program_may_enable(lookup: &SemanticLookup<'_>) -> bool {
    lookup.files().iter().any(|file| {
        let ast = &file.ast;
        let property = |span: Span| {
            ast.member_at(ast.peel_ts_sugar_span(span))
                .and_then(|member| file.source_text(member.property))
        };
        ast.assignments
            .iter()
            .any(|assignment| property(assignment.target) == Some("disabled"))
            || ast.calls.iter().any(|call| {
                property(call.callee).is_some_and(|name| ATTRIBUTE_WRITERS.contains(&name))
            })
    })
}
