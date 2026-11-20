//! ADR 1140: an event handler an intrinsic element's later source overrides is never attached.
//!
//! Where an intrinsic element has a spread, the compiler hands every spread and attribute to the runtime's `spread` as one list of
//! sources in source order (`<button {...{ onClick: f }} onClick={g}>` is `spread(el, [{ onClick: f }, { onClick: g }])`), and rc.21
//! `@solidjs/web` `collectProps` skips a key that any later source has (`sourceHas`, whatever its value, `undefined` included).
//! So the function an earlier source holds under that key is never installed as a listener, and its body never runs from that
//! element: a violation there would claim a run that never happens.
//!
//! A later source supplies the key when it is an attribute of that name, an object literal written in the spread whose closed
//! shape holds the key, or a spread whose declared type has a complete key set holding it (ADR 0436's premise). A later spread
//! proven not to supply it is a closed literal without the key, or a complete declared key set without it; any other later spread
//! may supply it. The handler's placement takes the bare unowned bit (`MayBeUnowned`, never a proof of an unowned run), and every
//! owner row written inside it is an obligation named `handler-key-overridden`. A row only moves from a violation to an
//! obligation, never to a clean.

use solid_facts::FileFacts;
use solid_facts::ast::JsxElementFact;
use solid_facts::core::Span;

use crate::Program;
use crate::indexes::SemanticLookup;

/// Whether a source of `element` written after `after` (an attribute, or a spread's argument) may supply `key`.
pub(crate) fn later_source_may_supply(
    file: &FileFacts,
    lookup: &SemanticLookup<'_>,
    element: &JsxElementFact,
    after: Span,
    key: &str,
) -> bool {
    if !lookup.dialect.spread_skips_key_a_later_source_has() {
        return false;
    }
    element
        .attributes
        .iter()
        .filter(|attribute| attribute.span.start > after.start)
        .any(|attribute| file.source_text(attribute.name) == Some(key))
        || element
            .spreads
            .iter()
            .filter(|spread| spread.span.start > after.start)
            .any(|spread| spread_may_supply(file, lookup, spread.argument, key))
}

/// Whether one spread argument may hold `key`: only a closed literal written in the spread, or a complete declared key set, says
/// it does not.
fn spread_may_supply(
    file: &FileFacts,
    lookup: &SemanticLookup<'_>,
    argument: Span,
    key: &str,
) -> bool {
    let peeled = file.ast.peel_ts_sugar_span(argument);
    if file
        .source_text(peeled)
        .is_some_and(|text| text.trim_start().starts_with('{'))
        && let Some(shape) = file
            .ast
            .object_get_shapes
            .iter()
            .find(|shape| shape.span == peeled)
        && shape.closed
    {
        return shape.properties.iter().any(|property| property.key == key);
    }
    match lookup
        .entity_at(file.path.as_str(), argument)
        .and_then(|entity| entity.declared_keys.as_deref())
    {
        Some(keys) => keys.iter().any(|declared| declared.as_ref() == key),
        None => true,
    }
}

/// The key the object literal property whose value is `function` is written under, in `file` (the kernel's cooked key).
pub(crate) fn held_under(file: &FileFacts, function: Span) -> Option<String> {
    file.ast
        .object_properties
        .iter()
        .find(|property| file.ast.peel_ts_sugar_span(property.value) == function)
        .and_then(|property| crate::kernel::property_key(file, property.key))
}

/// Whether the event handler written as the value of an intrinsic element's attribute at `handler` (a compiler event-handler
/// role's span) is one a later source of the element may supply under the attribute's key.
pub(crate) fn attribute_handler_overridden(
    file: &FileFacts,
    lookup: &SemanticLookup<'_>,
    handler: Span,
) -> bool {
    let handler = file.ast.peel_ts_sugar_span(handler);
    file.ast.jsx_elements.iter().any(|element| {
        !element.spreads.is_empty()
            && crate::object_members::intrinsic_element(file, element)
            && element.attributes.iter().any(|attribute| {
                attribute
                    .expression
                    .is_some_and(|expression| file.ast.peel_ts_sugar_span(expression) == handler)
                    && file.source_text(attribute.name).is_some_and(|key| {
                        lookup.dialect.spread_key_is_event(key) == Some(true)
                            && later_source_may_supply(file, lookup, element, attribute.span, key)
                    })
            })
    })
}

/// Marks each owner row written inside a handler a later source of its element overrides (module docs).
pub(crate) fn mark(program: &mut Program, lookup: &SemanticLookup<'_>) {
    for requirement in &mut program.missing_owners {
        if !requirement.report {
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
        if in_overridden_handler(file, lookup, Span::new(start, end)) {
            requirement.uncertain = true;
            requirement.handler_key_overridden = true;
        }
    }
}

/// Whether `site` is written inside a function an intrinsic element's spread literal holds under an event key, or inside an event
/// attribute's value, when a later source of that element may supply the key.
fn in_overridden_handler(file: &FileFacts, lookup: &SemanticLookup<'_>, site: Span) -> bool {
    file.ast.jsx_elements.iter().any(|element| {
        if !crate::object_members::intrinsic_element(file, element) || element.spreads.is_empty() {
            return false;
        }
        let event_key = |key: &str| lookup.dialect.spread_key_is_event(key) == Some(true);
        let in_attribute = element.attributes.iter().any(|attribute| {
            attribute.value.is_some_and(|value| value.contains(site))
                && file.source_text(attribute.name).is_some_and(|key| {
                    event_key(key)
                        && later_source_may_supply(file, lookup, element, attribute.span, key)
                })
        });
        in_attribute
            || element.spreads.iter().any(|spread| {
                let location = crate::location(file.path.shared(), spread.argument);
                let Some(Some(targets)) = lookup.spread_handler_targets().get(&location) else {
                    return false;
                };
                targets.iter().any(|(path, function)| {
                    path.as_str() == file.path.as_str()
                        && function.contains(site)
                        && held_under(file, *function).is_some_and(|key| {
                            event_key(&key)
                                && later_source_may_supply(file, lookup, element, spread.span, &key)
                        })
                })
            })
    })
}
