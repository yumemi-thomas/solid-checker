//! Event reachability: whether a prop, member or spread reaches only event handlers.

use super::*;

/// Whether `span`, written in the function whose body is `owner_body`, runs
/// outside that function's own call according to the compiler:
///
/// - in a tracked region of the same function, an attribute or child expression
///   the compiler lowers into a tracked effect, so its reads are subscribed
///   when the region runs. A function that is an attribute value inside the
///   region (a handler, a callback prop) or an argument not proven to run
///   inline is not the region's tracking pass;
/// - in a component property the compiler lowers to a getter, which runs when
///   the consuming component reads the property, never while this function
///   builds the JSX. A consumer that reads it untracked is reported there.
pub(crate) fn runs_outside_owner_call(
    file: &solid_facts::FileFacts,
    span: Span,
    owner_body: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_facts::compiler::{CompilerExecutionDisposition, CompilerOperationKind};
    file.compiler.tracked_regions.iter().any(|region| {
        owner_body.contains(region.span)
            && region.span.contains(span)
            && !attribute_function_within(file, region.span, span, lookup)
    }) || file
        .compiler
        .semantic_model
        .operations
        .iter()
        .any(|operation| {
            operation.kind == CompilerOperationKind::ComponentProperty
                && operation.execution.disposition
                    == CompilerExecutionDisposition::ComponentPropertyGetter
                && owner_body.contains(operation.span)
                && operation.span.contains(span)
        })
}

/// ADR 0199: [`ExecutionRole::EventCallback`] for code directly in a function
/// literal written as a project component's prop, when that component only
/// ever hands the prop to DOM event dispatch.
///
/// Constructing a callback prop runs nothing; the consumer decides when it
/// runs. Here the consumer is resolved exactly (the tag's symbol is one
/// project function) and every use it makes of the prop is proven to be an
/// intrinsic element's event handler ([`prop_reaches_only_events`]). Then the
/// literal runs, if ever, when the browser dispatches that event, exactly as a
/// literal written on the element itself does, and takes that literal's role.
/// Anything less -- an unresolved tag, a consumer that calls the prop, keeps
/// it, or lets the props object escape -- answers nothing here, and the
/// literal keeps its unproven timing ([`callee_callback_timing`]).
pub(super) fn forwarded_event_prop_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    // Any enclosing literal, not only the innermost: whatever runs inside a
    // literal that runs only on dispatch runs only after it, exactly as code
    // nested in a handler written on the element does. Nested primitives and
    // inline callbacks were classified by the arms ahead of this one.
    file.ast
        .functions_body_containing(span)
        .any(|literal| {
            component_prop_literal(file, literal.span).is_some_and(|(element, prop)| {
                component_prop_reaches_only_events(
                    file,
                    element,
                    prop,
                    &[],
                    lookup,
                    &mut HashSet::default(),
                )
            })
        })
        .then_some(ExecutionRole::EventCallback)
}

/// ADR 0204: whether a function literal written as a project component's prop
/// runs while that component renders.
///
/// The tag resolves to one project function (`function_called_at`), and the
/// element has no spread, which could replace the prop. The component's props
/// parameter is one identifier with no default and is never assigned, and the
/// component's own body calls `props.<prop>` directly: not in a nested
/// function, not in JSX (a tracked region or a prop getter), not in a default
/// parameter. Rendering the element runs the component's body untracked, so
/// the call there runs the literal in that body's strict-read window.
///
/// The call may sit in a branch; like a read written in the body, it is
/// proven to run whenever that code does. Every other consumer -- one that
/// forwards the prop, keeps it, or calls it only from a closure -- proves
/// nothing here.
pub(super) fn prop_literal_invoked_during_render(
    file: &solid_facts::FileFacts,
    literal: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some((element, prop)) = component_prop_literal(file, literal) else {
        return false;
    };
    if !element.spreads.is_empty()
        || element
            .attributes
            .iter()
            .filter(|attribute| {
                attribute.namespace.is_none() && file.source_text(attribute.name) == Some(prop)
            })
            .count()
            != 1
    {
        return false;
    }
    let Some((component_file, component)) = rendered_component(file, element, lookup) else {
        return false;
    };
    component_invokes_prop_in_body(component_file, component, prop, lookup)
}

/// The project component `element`'s tag runs: its binding is current (no
/// `Kids = Later` reassignment), and a member tag (`Menu.Item`) reaches
/// exactly the declared member, as [`InvocationFlow::component_props_uses`]
/// requires.
pub(super) fn rendered_component<'f>(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    lookup: &SemanticLookup<'f>,
) -> Option<(
    &'f solid_facts::FileFacts,
    &'f solid_facts::ast::FunctionFact,
)> {
    let (component_file, component) =
        lookup.function_called_at(file.path.as_str(), element.name.span)?;
    let tag = file.ast.peel_ts_sugar_span(element.name.span);
    let member_tag = file.ast.members.iter().any(|member| member.span == tag)
        || file.ast.computed_members.binary_search(&tag).is_ok();
    (lookup.function_value_is_current(component_file, component)
        && (!member_tag
            || member_runs_declared_method(file, tag, component_file, component, lookup)))
    .then_some((component_file, component))
}

/// Whether `span` sits in an instance field initializer of some class,
/// directly or in a function written there (review round 2: an IIFE or an
/// inline callback in the initializer). The initializer runs when an
/// instance is constructed, not where the class is written.
pub(crate) fn inside_instance_field(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast.classes.iter().any(|class| {
        class.span.contains(span)
            && class.elements.iter().any(|element| {
                element.kind == solid_facts::ast::ClassElementKind::Field
                    && !element.r#static
                    && element.value.is_some_and(|value| value.contains(span))
            })
    })
}

/// Whether `span` sits in a class written inside `function`: its field
/// initializers and static blocks run at construction or class evaluation,
/// not where the function's own flow places them.
pub(super) fn class_within(file: &solid_facts::FileFacts, function: Span, span: Span) -> bool {
    file.ast
        .classes
        .iter()
        .any(|class| function.contains(class.span) && class.span.contains(span))
}

/// Whether `component`'s own body calls `props.<prop>` directly; see
/// [`prop_literal_invoked_during_render`].
pub(super) fn component_invokes_prop_in_body(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    prop: &str,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if component.r#async || component.generator {
        return false;
    }
    let Some(parameter) = component.parameters.first() else {
        return false;
    };
    if parameter.shape != solid_facts::ast::BindingShape::Identifier
        || parameter.initializer.is_some()
    {
        return false;
    }
    let Some(name) = parameter.names.first() else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span) else {
        return false;
    };
    let names_props = |span: Span| {
        lookup
            .entities()
            .at(file.path.as_str(), file.ast.peel_ts_sugar_span(span))
            == Some(symbol)
    };
    // `props = …` or `props.<prop> = …` would make the call run something else.
    if file.ast.assignments.iter().any(|assignment| {
        component.span.contains(assignment.target)
            && (names_props(assignment.target)
                || file.ast.members.iter().any(|member| {
                    member.span == file.ast.peel_ts_sugar_span(assignment.target)
                        && names_props(member.object)
                        && file.source_text(member.property) == Some(prop)
                }))
    }) {
        return false;
    }
    // Every use of `props` in the component must be a static member read
    // that is not written, deleted, or called (a call hands `props` over as
    // `this`), except the call of `prop` itself; and no member named `prop`
    // (or a computed one) may be written or deleted on any object. Anything
    // else -- an alias, a spread, an argument (a single-source `merge(props)`
    // is `props`), a destructuring -- may replace `props.<prop>` before the
    // call, which then runs something else.
    let peel = |span: Span| file.ast.peel_ts_sugar_span(span);
    // Every write form: assignments, destructuring leaves, updates and loop
    // heads (`write_targets`, `iteration_targets`), and `delete`.
    let written = |member: Span| {
        file.ast
            .assignments
            .iter()
            .map(|assignment| assignment.target)
            .chain(file.ast.write_targets.iter().copied())
            .chain(file.ast.iteration_targets.iter().copied())
            .chain(file.ast.deleted_targets.iter().copied())
            .any(|target| target.contains(member))
    };
    // A redeclaration (`var props = …`) or any write of the parameter itself.
    if crate::indexes::binding_written(file, name.span) {
        return false;
    }
    // `arguments[0]` is `props` under another name, and `eval` can name it.
    if file.ast.identifiers.iter().any(|identifier| {
        component.span.contains(identifier.span)
            && matches!(
                file.source_text(identifier.span),
                Some("arguments" | "eval")
            )
    }) {
        return false;
    }
    if file
        .ast
        .reference_declarations
        .iter()
        .filter(|(reference, declaration)| *declaration == name.span && *reference != name.span)
        .any(|(reference, _)| {
            !file.ast.members.iter().any(|member| {
                peel(member.object) == *reference
                    && !written(member.span)
                    && member_static_key(file, member).is_some_and(|key| {
                        key == prop
                            || !(file
                                .ast
                                .calls
                                .iter()
                                .any(|call| peel(call.callee) == member.span)
                                || flow_tagged_or_chained(file, member.span))
                    })
            })
        })
        || file.ast.members.iter().any(|member| {
            component.span.contains(member.span)
                && written(member.span)
                && member_static_key(file, member).is_none_or(|key| key == prop)
        })
    {
        return false;
    }
    file.ast.calls.iter().any(|call| {
        crate::owners::written_directly_in(&file.ast, component, call.span)
            && !class_within(file, component.span, call.span)
            && !file.ast.any_jsx_containing(call.span)
            && file.ast.members.iter().any(|member| {
                member.span == file.ast.peel_ts_sugar_span(call.callee)
                    && names_props(member.object)
                    && file.source_text(member.property) == Some(prop)
            })
    })
}

/// The project component element and prop name a function literal is
/// exactly the value of, when it is one.
pub(super) fn component_prop_literal(
    file: &solid_facts::FileFacts,
    literal: Span,
) -> Option<(&solid_facts::ast::JsxElementFact, &str)> {
    let (element, attribute) = file.ast.jsx_containing(literal).find_map(|element| {
        element
            .attributes
            .iter()
            .find(|attribute| {
                attribute.namespace.is_none()
                    && attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                    && attribute
                        .expression
                        .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == literal)
            })
            .map(|attribute| (element, attribute))
    })?;
    if intrinsic_element(file, element) {
        return None;
    }
    Some((element, file.source_text(attribute.name)?))
}

/// Whether `value` is exactly the value of an attribute of an intrinsic
/// element.
pub(super) fn intrinsic_attribute_value(file: &solid_facts::FileFacts, value: Span) -> bool {
    let value = file.ast.peel_ts_sugar_span(value);
    file.ast.jsx_containing(value).any(|element| {
        intrinsic_element(file, element)
            && element.attributes.iter().any(|attribute| {
                attribute
                    .expression
                    .is_some_and(|expression| file.ast.peel_ts_sugar_span(expression) == value)
            })
    })
}

/// Whether a JSX element is an intrinsic (DOM) element: a lowercase, undotted
/// tag, which JSX resolves as an element name rather than a binding.
pub(crate) fn intrinsic_element(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> bool {
    element.member_object.is_none()
        && file
            .source_text(element.name.span)
            .and_then(|name| name.chars().next())
            .is_some_and(|first| first.is_ascii_lowercase())
}

/// Whether the component `function` uses its prop `prop` only as DOM event
/// dispatch.
///
/// The props parameter is one identifier, and it and every view of it this
/// proof follows (a `const` bound to a props merge or split the dialect names,
/// [`solid_dialect::Dialect::merges_props_reactivity`] and
/// [`solid_dialect::Dialect::splits_props`]) is used only as
///
/// - a static member access: `props.<other>` is another prop, and every
///   `props.<prop>` is exactly the value of an intrinsic element's attribute
///   the compiler classifies as an event handler
///   (`<button onClick={props.onPress}>`), inside the function written as such
///   a handler (`onClick={(event) => props.onPress?.(event)}`), or exactly the
///   value of another project component's prop that satisfies this proof;
/// - a spread onto an intrinsic element, when `prop` is an un-namespaced `on…`
///   name: the runtime's spread assigns such a property as an event listener
///   (`@solidjs/web` `assignProp`), the one other property it runs, `ref`, is
///   not an `on…` name;
/// - a spread onto a project component that satisfies this proof, or onto a
///   dialect component that renders the intrinsic element a string prop
///   names, when that prop is proven a string literal for this rendering
///   chain (ADR 0313: Kobalte's `Polymorphic` renders
///   `<Dynamic {...others} component={props.as}/>`, and `as` is the
///   `"button"` its `Button` writes unless a caller passes one); or
/// - the props argument of such a merge or split, whose result is spread or
///   bound to a `const` as above.
///
/// Any other use (a call argument, a destructuring, `props[key]`, a returned
/// or stored props object) could reach the prop any way at all, so it proves
/// nothing. Neither does a cycle.
pub(super) fn prop_reaches_only_events(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    prop: &str,
    shape: &PropsShape,
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
) -> bool {
    if function.r#async || function.generator || function.rest_parameter {
        return false;
    }
    let Some(parameter) = function.parameters.first() else {
        return false;
    };
    if parameter.shape != solid_facts::ast::BindingShape::Identifier
        || parameter.initializer.is_some()
    {
        return false;
    }
    let Some(name) = parameter.names.first() else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span) else {
        return false;
    };
    // Track the active path, not completed sibling proofs. The same component
    // prop can be forwarded twice without being a recursive cycle.
    let mut visiting = visiting.clone();
    if !visiting.insert((file.path.to_string(), function.span, prop.to_owned())) {
        return false;
    }
    let mut aliases: Vec<PropsView> = vec![(symbol.clone(), name.span, shape.clone())];
    let mut followed = 0;
    let mut references = 0usize;
    while let Some((alias, declaration, alias_shape)) = aliases.get(followed).cloned() {
        followed += 1;
        for reference in lookup.symbol_references(alias.as_str()) {
            if reference.path.as_ref() != file.path.as_str() {
                return false;
            }
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                return false;
            };
            let reference = Span::new(start, end);
            if reference == declaration {
                continue;
            }
            references += 1;
            if !function.body.contains(reference) {
                return false;
            }
            let proven = if let Some(member) = file
                .ast
                .members
                .iter()
                .find(|member| member.object == reference)
            {
                file.source_text(member.property) != Some(prop)
                    || member_reaches_only_events(
                        file,
                        member.span,
                        &aliases,
                        lookup,
                        &mut visiting,
                    )
            } else if let Some(view) = props_view_call(file, reference, lookup) {
                match spread_onto(file, view) {
                    Some(element) => spread_reaches_only_events(
                        file,
                        element,
                        prop,
                        &aliases,
                        lookup,
                        &mut visiting,
                    ),
                    None => match const_binding_of(file, view, lookup) {
                        Some((binding, binding_declaration)) => {
                            let view_shape = view_call_shape(file, view, &aliases, lookup);
                            aliases.push((binding, binding_declaration, view_shape));
                            true
                        }
                        None => false,
                    },
                }
            } else if let Some(element) = spread_onto(file, reference) {
                spread_reaches_only_events(file, element, prop, &aliases, lookup, &mut visiting)
            } else if let Some((binding, binding_declaration)) =
                const_binding_of(file, reference, lookup)
            {
                // `const rest = props`, `const root = view as Props`: the same
                // object under another name.
                aliases.push((binding, binding_declaration, alias_shape.clone()));
                true
            } else {
                false
            };
            if !proven {
                return false;
            }
        }
    }
    references > 0
}

/// Whether one `props.<prop>` member access reaches only event dispatch; see
/// [`prop_reaches_only_events`].
pub(super) fn member_reaches_only_events(
    file: &solid_facts::FileFacts,
    member: Span,
    views: &[PropsView],
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
) -> bool {
    use solid_facts::compiler::CallbackRoleKind;
    let event_handlers = || {
        file.compiler
            .callback_roles
            .iter()
            .filter(|callback| callback.role == CallbackRoleKind::EventHandler)
    };
    // The value of an attribute on some element, peeled exactly.
    let attribute_on = file.ast.jsx_containing(member).find_map(|element| {
        element
            .attributes
            .iter()
            .find(|attribute| {
                attribute.namespace.is_none()
                    && attribute
                        .expression
                        .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == member)
            })
            .map(|attribute| (element, attribute))
    });
    match attribute_on {
        Some((element, _)) if intrinsic_element(file, element) => {
            event_handlers().any(|callback| callback.span.contains(member))
        }
        Some((element, attribute)) => file.source_text(attribute.name).is_some_and(|next| {
            component_prop_reaches_only_events(file, element, next, views, lookup, visiting)
        }),
        None => {
            if event_handlers().any(|callback| {
                intrinsic_attribute_value(file, callback.span)
                    && file
                        .ast
                        .functions_within(callback.span)
                        .max_by_key(|handler| handler.span.end - handler.span.start)
                        .is_some_and(|handler| handler.body.contains(member))
            }) {
                return true;
            }
            if member_in_exclusive_named_event_handler(file, member, views, lookup, visiting) {
                return true;
            }
            // Inside a literal written as another project component's prop
            // that itself reaches only event dispatch.
            let literals = file
                .ast
                .functions_owning(member)
                .map(|literal| literal.span)
                .collect::<Vec<_>>();
            for literal in literals {
                let Some((element, prop)) = component_prop_literal(file, literal) else {
                    continue;
                };
                if component_prop_reaches_only_events(file, element, prop, views, lookup, visiting)
                {
                    return true;
                }
            }
            false
        }
    }
}

/// A local arrow whose complete reference census consists only of event
/// attributes or exact component props that reach events. One use is insufficient:
/// an eager call or an escaped alias could invoke the prop during rendering.
pub(super) fn member_in_exclusive_named_event_handler(
    file: &solid_facts::FileFacts,
    member: Span,
    views: &[PropsView],
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
) -> bool {
    use solid_facts::ast::{FunctionKind, IdentifierRole};
    let Some(handler) = containing_ast_function(&file.ast, member) else {
        return false;
    };
    if handler.kind != FunctionKind::Arrow
        || handler.r#async
        || handler.generator
        || containing_ast_function(&file.ast, handler.span).is_none()
        || lookup.function_component_status(file, handler) != ComponentStatus::No
        || !lookup.function_value_is_current(file, handler)
        || file.ast.identifiers.iter().any(|id| {
            id.role == IdentifierRole::Reference && file.source_text(id.span) == Some("eval")
        })
    {
        return false;
    }
    let Some(binding) = function_binding_name(file, handler) else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), binding.span) else {
        return false;
    };
    if file.ast.jsx_elements.iter().any(|element| {
        lookup.entities().at(file.path.as_str(), element.name.span) == Some(symbol)
            || file.ast.reference_declaration(element.name.span) == Some(binding.span)
    }) {
        return false;
    }
    // Include undemanded AST references as well as the Type Facts index, and
    // require both sources to agree on the exact local declaration.
    let mut references = file
        .ast
        .identifiers
        .iter()
        .filter(|id| {
            id.role == IdentifierRole::Reference
                && file.ast.reference_declaration(id.span) == Some(binding.span)
        })
        .map(|id| id.span)
        .collect::<HashSet<_>>();
    for reference in lookup.symbol_references(symbol.as_str()) {
        if reference.path.as_ref() != file.path.as_str() {
            return false;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            return false;
        };
        let reference = Span::new(start, end);
        if reference == binding.span {
            continue;
        }
        if file.ast.reference_declaration(reference) != Some(binding.span) {
            return false;
        }
        references.insert(reference);
    }
    !references.is_empty()
        && references.iter().all(|reference| {
            file.compiler.callback_roles.iter().any(|callback| {
                callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                    && intrinsic_attribute_value(file, callback.span)
                    && file.ast.peel_ts_sugar_span(callback.span) == *reference
            }) || component_prop_literal(file, *reference).is_some_and(|(element, prop)| {
                component_prop_reaches_only_events(file, element, prop, views, lookup, visiting)
            })
        })
}

/// The element `value` is spread onto, when `value` is exactly the argument of
/// a JSX spread attribute.
pub(super) fn spread_onto(
    file: &solid_facts::FileFacts,
    value: Span,
) -> Option<&solid_facts::ast::JsxElementFact> {
    file.ast.jsx_containing(value).find(|element| {
        element
            .spreads
            .iter()
            .any(|spread| file.ast.peel_ts_sugar_span(spread.argument) == value)
    })
}

/// Whether spreading a props object carrying `prop` onto `element` reaches
/// only event dispatch; see [`prop_reaches_only_events`].
pub(super) fn spread_reaches_only_events(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    prop: &str,
    views: &[PropsView],
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
) -> bool {
    if intrinsic_element(file, element) {
        // The runtime's spread decides (ADR 0358); unaudited proves nothing.
        return lookup.dialect.spread_key_is_event(prop) == Some(true);
    }
    component_prop_reaches_only_events(file, element, prop, views, lookup, visiting)
}

/// Whether the component `element` renders runs its prop `prop` only after
/// the render: a project component whose every use of it is proven
/// ([`prop_reaches_only_events`]). A package component is never proven.
pub(super) fn component_prop_reaches_only_events(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    prop: &str,
    views: &[PropsView],
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span, String)>,
) -> bool {
    // ADR 0313: a dialect component that renders the intrinsic element a
    // string prop names (`<Dynamic component="button" {...rest}/>`) assigns
    // every other prop as a spread onto that element does. Only when the
    // prop's value on this element is proven a string literal.
    if let Some(tag) = lookup.intrinsic_tag_prop(file, element.name.span) {
        return prop != tag
            && lookup.dialect.spread_key_is_event(prop) == Some(true)
            && element_prop_is_string(file, element, tag, views);
    }
    if let Some((next_file, next_function)) =
        lookup.function_called_at(file.path.as_str(), element.name.span)
    {
        let shape = element_props_shape(file, element, views);
        return prop_reaches_only_events(next_file, next_function, prop, &shape, lookup, visiting);
    }
    false
}
