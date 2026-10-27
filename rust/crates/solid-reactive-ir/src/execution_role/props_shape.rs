//! Props shapes: the static shape of an element's props and the views built over it.

use super::*;

/// ADR 0313: what one proof of [`prop_reaches_only_events`] knows about the
/// keys of one props object, or one view of it, along the rendering chain it
/// follows. Every key the object can hold, each with whether its value is
/// exactly a string literal; `None` when a key the proof cannot name may be
/// present.
///
/// The chain starts at the element whose prop literal is being placed: a
/// project element's props are exactly its attributes, its spreads and its
/// children, so with no spread of an unknown object they are closed. Each
/// hop keeps that knowledge only through the dialect's props views
/// ([`view_call_shape`]) and `const` aliases, and loses it at anything else.
/// Adding a key, or marking a value not a string, only ever withholds a
/// proof.
pub(super) type PropsShape = Option<std::collections::BTreeMap<String, bool>>;

/// A props object or a view of it in the component a proof walks: its
/// symbol, its declaration and its [`PropsShape`].
pub(super) type PropsView = (SymbolId, Span, PropsShape);

/// The [`PropsShape`] of the view whose binding `reference` (after
/// transparent TypeScript wrappers) names, when it is one of `views`.
pub(super) fn view_shape_at<'v>(
    file: &solid_facts::FileFacts,
    views: &'v [PropsView],
    reference: Span,
) -> Option<&'v PropsShape> {
    let declaration = file
        .ast
        .reference_declaration(file.ast.peel_ts_sugar_span(reference))?;
    views
        .iter()
        .find(|(_, view, _)| *view == declaration)
        .map(|(_, _, shape)| shape)
}

/// The [`PropsShape`] of the props `element` renders with, in attribute
/// order: an attribute sets its key, a spread of one of `views` sets that
/// view's keys over what came before, and any other spread opens the shape.
/// Children add `children`. A value is a string literal only when written as
/// one (`as="button"`) or read as a static member of a view whose shape
/// proves it one (`component={props.as}`).
pub(super) fn element_props_shape(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    views: &[PropsView],
) -> PropsShape {
    let mut keys = std::collections::BTreeMap::new();
    for item in element_props_in_order(element) {
        match item {
            Ok(attribute) => {
                // A namespaced attribute's key is not its spelling here.
                if attribute.namespace.is_some() {
                    return None;
                }
                let name = file.source_text(attribute.name)?;
                keys.insert(
                    name.to_owned(),
                    attribute_value_is_string(file, attribute, views),
                );
            }
            Err(argument) => {
                let view = view_shape_at(file, views, argument)?.as_ref()?;
                keys.extend(view.iter().map(|(key, string)| (key.clone(), *string)));
            }
        }
    }
    if !element.self_closing || !element.children.is_empty() {
        keys.insert("children".to_owned(), false);
    }
    Some(keys)
}

/// An element's attributes (`Ok`) and spread arguments (`Err`) in source
/// order, which is the order the compiled props object takes them in.
pub(super) fn element_props_in_order(
    element: &solid_facts::ast::JsxElementFact,
) -> Vec<Result<&solid_facts::ast::JsxAttributeFact, Span>> {
    let mut items = element
        .attributes
        .iter()
        .map(|attribute| (attribute.span.start, Ok(attribute)))
        .chain(
            element
                .spreads
                .iter()
                .map(|spread| (spread.span.start, Err(spread.argument))),
        )
        .collect::<Vec<_>>();
    items.sort_by_key(|(start, _)| *start);
    items.into_iter().map(|(_, item)| item).collect()
}

/// Whether the prop `key` of the props `element` renders with is exactly a
/// string literal: the last attribute or spread that can set it does so with
/// one. A later attribute settles the key over an unknown spread before it
/// (`<Dynamic {...rest} component="a"/>`); an unknown spread after one opens
/// it again.
pub(super) fn element_prop_is_string(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    key: &str,
    views: &[PropsView],
) -> bool {
    let mut string = false;
    for item in element_props_in_order(element) {
        match item {
            Ok(attribute) => {
                if attribute.namespace.is_some() {
                    string = false;
                } else if file.source_text(attribute.name) == Some(key) {
                    string = attribute_value_is_string(file, attribute, views);
                }
            }
            Err(argument) => match view_shape_at(file, views, argument).and_then(Option::as_ref) {
                Some(view) => {
                    if let Some(value) = view.get(key) {
                        string = *value;
                    }
                }
                None => string = false,
            },
        }
    }
    string
}

/// Whether an attribute's value is exactly a string literal: written as one,
/// or a static member of a view whose shape proves that key one.
pub(super) fn attribute_value_is_string(
    file: &solid_facts::FileFacts,
    attribute: &solid_facts::ast::JsxAttributeFact,
    views: &[PropsView],
) -> bool {
    use solid_facts::ast::JsxAttributeValueKind;
    match attribute.value_kind {
        JsxAttributeValueKind::String => true,
        JsxAttributeValueKind::Expression => attribute.expression.is_some_and(|expression| {
            let expression = file.ast.peel_ts_sugar_span(expression);
            file.ast.member_at(expression).is_some_and(|member| {
                file.source_text(member.property).is_some_and(|key| {
                    view_shape_at(file, views, member.object)
                        .and_then(Option::as_ref)
                        .is_some_and(|shape| shape.get(key) == Some(&true))
                })
            })
        }),
        _ => false,
    }
}

/// The [`PropsShape`] of the props view `call` makes
/// ([`props_view_call`]): an `omit` of one view by string-literal keys drops
/// them; a `merge` of views and complete object literals takes each source's
/// keys over the earlier ones'. Any other argument opens the shape.
pub(super) fn view_call_shape(
    file: &solid_facts::FileFacts,
    call: Span,
    views: &[PropsView],
    lookup: &SemanticLookup<'_>,
) -> PropsShape {
    use solid_facts::ast::ArgumentLiteralFact;
    let call = file.ast.call_at(call)?;
    let primitive = lookup.primitive_at_call(file, call.span)?;
    if call.arguments.iter().any(|argument| argument.spread) {
        return None;
    }
    if lookup.dialect.splits_props(primitive) {
        let (source, hidden) = call.arguments.split_first()?;
        let mut keys = view_shape_at(file, views, source.span)?.clone()?;
        for key in hidden {
            let ArgumentLiteralFact::String(key) = &key.literal_value else {
                return None;
            };
            keys.remove(key.as_str());
        }
        return Some(keys);
    }
    if !lookup.dialect.merges_props_reactivity(primitive) {
        return None;
    }
    let mut keys = std::collections::BTreeMap::new();
    for argument in &call.arguments {
        if let Some(view) = view_shape_at(file, views, argument.span) {
            keys.extend(view.clone()?);
        } else if let ArgumentLiteralFact::Object(properties) = &argument.literal_value {
            // Every property a data property with a static key; a getter, a
            // spread or a computed key makes the fact `Unknown`.
            for property in properties {
                keys.insert(
                    property.name.to_string(),
                    matches!(property.value, ArgumentLiteralFact::String(_)),
                );
            }
        } else {
            return None;
        }
    }
    Some(keys)
}

/// The span of the call `reference` is the props argument of, when that call
/// is a props merge or split the dialect names: a view whose properties read
/// through to the props object and invoke none of them.
pub(super) fn props_view_call(
    file: &solid_facts::FileFacts,
    reference: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    let (call, _) = file
        .ast
        .arguments_containing(reference)
        .find(|(call, index)| {
            let argument = &call.arguments[*index];
            !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == reference
        })?;
    let primitive = lookup.primitive_at_call(file, call.span)?;
    (lookup.dialect.merges_props_reactivity(primitive) || lookup.dialect.splits_props(primitive))
        .then_some(call.span)
}

/// The `const` identifier binding initialized by exactly the expression at
/// `value`, with its symbol.
pub(super) fn const_binding_of(
    file: &solid_facts::FileFacts,
    value: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<(SymbolId, Span)> {
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding
                .initializer
                .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == value)
    })?;
    let name = binding.names.first()?;
    let symbol = lookup.entities().at(file.path.as_str(), name.span)?;
    Some((symbol.clone(), name.span))
}

/// Whether the function written at `function` is the value of a JSX attribute.
/// Whether `span` sits in a function, inside `region`, that is written as the
/// whole child expression of a component element: the component receives it
/// as `children` and decides when it runs. An intrinsic element's function
/// child is not one; the compiler inserts and runs it.
pub(super) fn component_child_function_within(
    file: &solid_facts::FileFacts,
    region: Span,
    span: Span,
) -> bool {
    functions_around(file, span).into_iter().any(|function| {
        region.contains(function.span) && component_child_function(file, function.span)
    })
}

/// Whether `function` is written as the whole child expression of a
/// component element (`<Kids>{(x) => …}</Kids>`).
pub(super) fn component_child_function(file: &solid_facts::FileFacts, function: Span) -> bool {
    file.ast
        .functions
        .iter()
        .any(|candidate| candidate.span == function)
        && file.ast.jsx_containing(function).any(|element| {
            !intrinsic_element(file, element)
                && element
                    .children
                    .iter()
                    .any(|child| jsx_child_expression(file, *child) == Some(function))
        })
}

pub(super) fn jsx_attribute_value_function(file: &solid_facts::FileFacts, function: Span) -> bool {
    file.ast.jsx_containing(function).any(|element| {
        element.attributes.iter().any(|attribute| {
            attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                && attribute
                    .expression
                    .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == function)
        })
    })
}
