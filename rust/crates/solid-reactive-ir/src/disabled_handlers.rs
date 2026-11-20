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
        if !in_delegated_handler_of_disabled_control(lookup, file, site) {
            continue;
        }
        if *enabled_somewhere.get_or_insert_with(|| program_may_enable(lookup)) {
            return;
        }
        requirement.uncertain = true;
        requirement.handler_on_disabled_control = true;
    }
}

/// Whether `site` is written in the value of an `on…` attribute of a disabled control that the compiler delegated, or (ADR 1141) in a
/// handler that reaches a disabled control with spreads through the runtime's delegated dispatcher.
fn in_delegated_handler_of_disabled_control(lookup: &SemanticLookup<'_>, file: &FileFacts, site: Span) -> bool {
    // A recorded delegation is the compiler's own answer for that handler; completeness (`generated_operations_complete`) is
    // what an absent operation would need, and no absence is read here.
    let model = &file.compiler.semantic_model;
    file.ast.jsx_elements.iter().any(|element| {
        if !element.spreads.is_empty() {
            return spread_handler_of_disabled_control(lookup, file, element, site);
        }
        model.refusal.is_none()
            && statically_disabled_control(file, element)
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

/// ADR 1141: a control with spreads hands every attribute and spread to rc.21's `spread` as one array of sources, whose `assignProp`
/// delegates a listener key the dialect names ([`solid_dialect::Dialect::spread_event_delegated`]). `site` is in such a handler of a
/// disabled control when the control is one of [`DISABLEABLE`] with one bare or `{true}` `disabled`, no `ref`, and spreads each of
/// whose own keys is proven ([`spread_own_keys`]) with no `disabled`, `ref` or namespaced key among them (nothing can write
/// `disabled` back); and the handler is either the control's own `on…` attribute whose key is delegated, or a function an object
/// literal written in place in one of the spreads holds under a delegated key (no other reference can reach it).
fn spread_handler_of_disabled_control(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    element: &JsxElementFact,
    site: Span,
) -> bool {
    if !disabled_control_attributes(file, element) {
        return false;
    }
    let mut in_place = Vec::new();
    for spread in &element.spreads {
        let Some((keys, literal)) = spread_own_keys(lookup, file, spread.argument) else {
            return false;
        };
        if keys
            .iter()
            .any(|key| key == "disabled" || key == "ref" || key.contains(':'))
        {
            return false;
        }
        if let Some(literal) = literal {
            in_place.push((spread.argument, literal));
        }
    }
    let delegated = |key: &str| lookup.dialect.spread_event_delegated(key);
    let by_attribute = element.attributes.iter().any(|attribute| {
        attribute.namespace.is_none()
            && attribute.value.is_some_and(|value| value.contains(site))
            && file.source_text(attribute.local_name).is_some_and(delegated)
    });
    // A function an in-place literal holds directly under a key: one of ADR 0358's exact targets of that spread.
    by_attribute
        || in_place.iter().any(|(argument, literal)| {
            let Some(Some(targets)) = lookup
                .spread_handler_targets()
                .get(&crate::location(file.path.shared(), *argument))
            else {
                return false;
            };
            targets.iter().any(|(path, function)| {
                path.as_str() == file.path.as_str()
                    && function.contains(site)
                    && file.ast.object_properties.iter().any(|property| {
                        !property.computed
                            && literal.contains(property.span)
                            && file.ast.peel_ts_sugar_span(property.value) == *function
                            && file
                                .source_text(property.key)
                                .is_some_and(|key| delegated(key.trim_matches(['\'', '"'])))
                    })
            })
        })
}

/// ADR 1141: every own key a spread argument's object has on every run, and the literal's span when it is written in place: an
/// object literal in place whose shape is closed (no spread, computed key, setter or `__proto__`), or a reference to an unwritten
/// `const` initialized by one, which ADR 0358's spread resolution accepts (the binding passes its receiver premises and its
/// handlers resolve exactly). `None` for anything else.
fn spread_own_keys(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    argument: Span,
) -> Option<(Vec<String>, Option<Span>)> {
    let peeled = file.ast.peel_ts_sugar_span(argument);
    let closed_keys = |owner: &FileFacts, span: Span| {
        owner
            .ast
            .object_get_shapes
            .iter()
            .find(|shape| shape.span == span && shape.closed)
            .map(|shape| {
                shape
                    .properties
                    .iter()
                    .map(|property| property.key.to_string())
                    .collect::<Vec<_>>()
            })
    };
    if let Some(keys) = closed_keys(file, peeled) {
        return Some((keys, Some(peeled)));
    }
    // A binding: ADR 0358's exact resolution must hold for this very spread.
    let resolved = lookup
        .spread_handler_targets()
        .get(&crate::location(file.path.shared(), argument));
    if !matches!(resolved, Some(Some(_))) {
        return None;
    }
    let (owner, binding, _) = lookup.binding_at_reference(file.path.as_str(), peeled)?;
    if !binding.immutable || binding.names.len() != 1 {
        return None;
    }
    let initializer = owner.ast.peel_ts_sugar_span(binding.initializer?);
    closed_keys(owner, initializer).map(|keys| (keys, None))
}

/// `onClick`: `on` and an upper-case letter, the form the compiler reads as an event (`onclick` is a property, `on:click` a listener).
fn is_event_attribute(name: &str) -> bool {
    name.strip_prefix("on")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|first| first.is_ascii_uppercase())
}

/// One of [`DISABLEABLE`], written with a single `disabled` that is bare or `{true}`, no spread and no `ref`.
fn statically_disabled_control(file: &FileFacts, element: &JsxElementFact) -> bool {
    element.spreads.is_empty() && disabled_control_attributes(file, element)
}

/// One of [`DISABLEABLE`], written with a single `disabled` that is bare or `{true}` and no `ref` among its attributes.
fn disabled_control_attributes(file: &FileFacts, element: &JsxElementFact) -> bool {
    if element.member_object.is_some()
        || !file
            .source_text(element.name.span)
            .is_some_and(|tag| DISABLEABLE.contains(&tag))
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
