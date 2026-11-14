//! ADR 0978: every way a `Loading` boundary is entered, and what covers a read written in a layer a call hands to one.
//!
//! `Loading` is a component function, so a program is not limited to the JSX tag: it calls the function (`Loading(props)`, through an alias or a
//! namespace, `createComponent(Loading, props)`), names it as a value (`Loading.call(null, props)`, `[Loading][0](props)`,
//! `dynamicComponent(() => Loading)`, `<Dynamic component={Loading}>`, a tag a prop holds) or builds the boundary with `createLoadingBoundary`.
//! SC5006 and SC5007 were asked of the tag (SC5007 also of the direct call) and the cover walk read only a JSX element as cover, so the other forms
//! were a false clean (`on` held a read, an accessor went in as `on`) and a false positive (a read in a hand-called boundary's `children` getter
//! was read as outside). This module is the shared vocabulary of the three:
//!
//! * [`entry_call`]: a call that is exactly `Loading(props)` (the callee resolves to the dialect's boundary, as the tag does) or exactly
//!   `createComponent(Loading, props)` (the callee is the dialect's component renderer, its first argument resolves to the boundary): the
//!   props are an argument, asked like the tag's attributes.
//! * [`loading_value_references`]: every other place the boundary is named, which may enter it anywhere.
//! * [`unresolved_tag`]: a tag whose component is not resolved, which may be the boundary.
//! * [`layer_cover`]: what a function written as a member of a hand-called boundary's props, or as an argument of `createLoadingBoundary`, says of
//!   the reads it holds.

use crate::{PrimitiveName, SemanticLookup, referenced_primitive};
use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentFact, FunctionFact, JsxElementFact, ObjectPropertyFact};
use solid_facts::core::Span;

/// How an exact call enters the boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntryCall {
    /// `Loading(props)`: the props are argument 0.
    Hand,
    /// `createComponent(Loading, props)`: the props are argument 1.
    CreateComponent,
}

impl EntryCall {
    /// The index of the props argument.
    pub(crate) fn props(self) -> usize {
        match self {
            Self::Hand => 0,
            Self::CreateComponent => 1,
        }
    }
}

/// How call `index` of `file` enters the boundary, by exact resolution: the callee resolves to the dialect's boundary export (an alias, a
/// namespace member, a `const` alias the binder resolves), or is the dialect's component renderer whose first argument resolves to it. A
/// local function that happens to be named `Loading` is neither.
pub(crate) fn entry_call(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    index: usize,
) -> Option<EntryCall> {
    let call = file.ast.calls.get(index)?;
    if let Some(PrimitiveName::Known(_, spelled)) =
        lookup.primitives(file).calls.get(index)?.as_ref()
    {
        return lookup
            .dialect
            .is_async_boundary(spelled)
            .then_some(EntryCall::Hand);
    }
    // `createComponent(Loading, props)`: the callee is the dialect's component renderer, by the exact import binding (the dialect's module, the
    // export's own name), and the first argument resolves to the boundary.
    if !is_component_renderer(lookup, file, call.callee) {
        return None;
    }
    let first = call.arguments.first().filter(|first| !first.spread)?;
    let target = referenced_primitive(
        file,
        file.ast.peel_ts_sugar_span(first.span),
        lookup.entities(),
        lookup.symbol_names(),
        lookup.dialect,
    )?;
    lookup
        .dialect
        .is_async_boundary(&target)
        .then_some(EntryCall::CreateComponent)
}

/// Whether `callee` is exactly an export of the dialect's modules that renders its first argument as a component
/// ([`solid_dialect::Dialect::renders_component_argument`]): a named import of it, or a member of a namespace import of such a module.
fn is_component_renderer(lookup: &SemanticLookup<'_>, file: &FileFacts, callee: Span) -> bool {
    use solid_facts::ast::ImportKind;
    let ast = &file.ast;
    let callee = ast.peel_ts_sugar_span(callee);
    let renders = |name: &str| lookup.dialect.renders_component_argument(name) == Some(0);
    let import_of = |declaration: Span| {
        ast.imports.iter().find_map(|import| {
            import
                .bindings
                .iter()
                .find(|binding| binding.local.span == declaration)
                .map(|binding| (import, binding))
        })
    };
    if let Some(declaration) = ast.reference_declaration(callee) {
        let Some((import, binding)) = import_of(declaration) else {
            return false;
        };
        let imported = binding
            .imported
            .as_deref()
            .or_else(|| file.source_text(binding.local.span));
        return binding.kind != ImportKind::Namespace
            && !lookup
                .dialect
                .namespace_import_primitives(import.module.as_str())
                .is_empty()
            && imported.is_some_and(renders);
    }
    let Some(member) = ast
        .members
        .iter()
        .find(|member| member.span == callee && !ast.computed_members.contains(&member.span))
    else {
        return false;
    };
    let Some(declaration) = ast.reference_declaration(ast.peel_ts_sugar_span(member.object)) else {
        return false;
    };
    import_of(declaration).is_some_and(|(import, binding)| {
        binding.kind == ImportKind::Namespace
            && !lookup
                .dialect
                .namespace_import_primitives(import.module.as_str())
                .is_empty()
            && file.source_text(member.property).is_some_and(renders)
    })
}

/// The prop that re-arms the boundary, as the dialect names it (`None` where the dialect has no async boundary).
pub(crate) fn on_prop(lookup: &SemanticLookup<'_>) -> Option<&'static str> {
    lookup
        .dialect
        .async_boundary_shape()
        .map(|shape| shape.on_prop)
}

/// The properties written directly in the object literal at `literal`: not those of an object a property's value holds.
pub(crate) fn direct_members(file: &FileFacts, literal: Span) -> Vec<&ObjectPropertyFact> {
    file.ast
        .object_properties_within(literal)
        .filter(|property| {
            !file
                .ast
                .object_properties_containing(property.span)
                .any(|outer| {
                    outer.span != property.span
                        && literal.contains(outer.span)
                        && outer.value.contains(property.span)
                })
        })
        .collect()
}

/// Whether a property key is spelled `name`, quoted or not.
pub(crate) fn key_is(file: &FileFacts, key: Span, name: &str) -> bool {
    file.source_text(key)
        .is_some_and(|text| text.trim_matches(['"', '\'']) == name)
}

/// What a call's props argument says of `on`.
pub(crate) enum PropsOn<'a> {
    /// No `on`: the argument is absent, or a literal whose every key is named and none is `on`.
    Absent,
    /// A literal that names `on`: the property (a data property, a getter, a method).
    Member(&'a ObjectPropertyFact),
    /// Anything else (a spread, a computed key, a value built elsewhere): `on` may be there.
    Open,
}

/// The props argument's `on`: exact for a literal whose property set is closed (`closed_object_literal`: accessors allowed, no spread, no
/// computed key), open for every other expression.
pub(crate) fn props_on<'a>(
    file: &'a FileFacts,
    on_prop: &str,
    argument: Option<&ArgumentFact>,
) -> PropsOn<'a> {
    let Some(argument) = argument else {
        return PropsOn::Absent;
    };
    if argument.spread || !argument.closed_object_literal {
        return PropsOn::Open;
    }
    direct_members(file, argument.span)
        .into_iter()
        .find(|property| key_is(file, property.key, on_prop))
        .map_or(PropsOn::Absent, PropsOn::Member)
}

/// The places the boundary is named that are neither a tag, the callee of an exact call, nor the first argument of `createComponent`: a value
/// that may enter it anywhere. A `const` alias (`const Hold = Loading`) is not one: the binder resolves its uses, which are classified as the
/// import's are.
pub(crate) fn loading_value_references(lookup: &SemanticLookup<'_>, file: &FileFacts) -> Vec<Span> {
    let ast = &file.ast;
    let names_loading = |span: Span| {
        referenced_primitive(
            file,
            span,
            lookup.entities(),
            lookup.symbol_names(),
            lookup.dialect,
        )
        .is_some_and(|name| lookup.dialect.is_async_boundary(&name))
    };
    let mut accounted: Vec<Span> = Vec::new();
    for (index, call) in ast.calls.iter().enumerate() {
        let entry = entry_call(lookup, file, index);
        if entry == Some(EntryCall::Hand) {
            accounted.push(ast.peel_ts_sugar_span(call.callee));
        }
        if entry == Some(EntryCall::CreateComponent)
            && let Some(first) = call.arguments.first()
        {
            accounted.push(ast.peel_ts_sugar_span(first.span));
        }
    }
    let names = ast
        .jsx_elements
        .iter()
        .flat_map(|element| [Some(element.name.span), element.closing_name])
        .flatten()
        .collect::<Vec<_>>();
    let aliases = ast
        .bindings
        .iter()
        .filter(|binding| {
            binding.immutable
                && binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.names.len() == 1
        })
        .filter_map(|binding| binding.initializer_identifier.as_ref())
        .map(|initializer| initializer.span)
        .collect::<Vec<_>>();
    let references = ast
        .reference_declarations
        .iter()
        .map(|(reference, _)| *reference)
        .chain(ast.members.iter().map(|member| member.span));
    let mut found = Vec::new();
    for reference in references {
        if accounted.contains(&reference)
            || aliases.contains(&reference)
            || names.iter().any(|name| name.contains(reference))
            || !names_loading(reference)
        {
            continue;
        }
        found.push(reference);
    }
    found.sort();
    found.dedup();
    found
}

/// Whether the component `element` names is not resolved to anything the analysis reads: not an intrinsic element, not a dialect export, not a
/// project function with a body, not a declaration of an installed package (whose behaviour is read from its source or answered SC9005). A
/// parameter, a props member (`<p.as>`), the result of a call (`dynamicComponent(..)`) and a value of type `any` are. It may be the boundary.
pub(crate) fn unresolved_tag(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    index: usize,
    element: &JsxElementFact,
) -> bool {
    if !crate::owners::is_component_element(file, element) {
        return false;
    }
    if matches!(
        lookup.primitives(file).jsx.get(index),
        Some(Some(PrimitiveName::Known(..)))
    ) {
        return false;
    }
    let path = file.path.as_str();
    lookup.function_called_at(path, element.name.span).is_none()
        && lookup
            .callee_package_root(file, element.name.span)
            .is_none()
}

/// What a function layer says of the reads written in it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LayerCover {
    /// The boundary holds them: Solid renders this layer inside it.
    Covered,
    /// The layer renders outside the boundary it was handed to (a `fallback`): the walk goes on from `at`, the call that made the boundary.
    Outward(Span),
    /// Code the analysis does not follow runs this layer, so where it renders is not known.
    Unresolved,
}

/// What the layer `function` is, when a call edge does not place it. `None` leaves it "part of the function around it": a callback of a dialect
/// primitive that runs it where it is written, a function the JSX hands to a component, a standard library callback.
pub(crate) fn layer_cover(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    function: &FunctionFact,
) -> Option<LayerCover> {
    let ast = &file.ast;
    let own = function.span;
    let boundary = lookup.dialect.async_boundary_shape()?;
    // A closure that is a call's whole argument.
    for (call, position) in ast.arguments_containing(own) {
        if ast.peel_ts_sugar_span(call.arguments[position].span) != own {
            continue;
        }
        let primitive = lookup.primitive_at_call(file, call.span);
        if primitive == Some(boundary.primitive) {
            // The boundary's primitive: the held content is the hold, the fallback renders outside it.
            return Some(if position == boundary.held_argument {
                LayerCover::Covered
            } else if position == boundary.fallback_argument {
                LayerCover::Outward(call.span)
            } else {
                LayerCover::Unresolved
            });
        }
        if primitive.is_some()
            || lookup
                .standard_library_declaration(file, call, typefacts::CallKind::Call)
                .is_some()
            || lookup
                .function_called_at(file.path.as_str(), call.callee)
                .is_some()
        {
            // A primitive that runs it where it is written, the standard library, a project function whose body is read.
            continue;
        }
        return Some(LayerCover::Unresolved);
    }
    // A member of an object literal: a getter, a method, a function-valued property.
    if let Some(property) = ast
        .object_properties
        .iter()
        .find(|property| ast.peel_ts_sugar_span(property.value) == own)
    {
        for (call_index, call) in ast.calls.iter().enumerate() {
            let Some((position, argument)) = call
                .arguments
                .iter()
                .enumerate()
                .find(|(_, argument)| argument.span.contains(property.span))
            else {
                continue;
            };
            if !direct_members(file, argument.span)
                .iter()
                .any(|member| member.span == property.span)
            {
                continue;
            }
            let Some(entry) = entry_call(lookup, file, call_index) else {
                continue;
            };
            if position != entry.props() || property.computed {
                continue;
            }
            // The props the runtime reads (`Loading(props)` ->
            // `createLoadingBoundary(() => props.children, () => props.fallback, ..)`): `children` is the hold, `fallback`
            // renders outside it. Only a getter is read as the prop's value; a function value is the value itself, and the
            // runtime returns it unrun.
            let getter = !property.data && !property.method && function.parameters.is_empty();
            return Some(
                if getter && key_is(file, property.key, boundary.children_prop) {
                    LayerCover::Covered
                } else if getter && key_is(file, property.key, boundary.fallback_prop) {
                    LayerCover::Outward(call.span)
                } else {
                    LayerCover::Unresolved
                },
            );
        }
        return Some(LayerCover::Unresolved);
    }
    // An array element, a class element, a tag function: run by a way no call edge follows.
    let in_array = ast.array_literals.iter().any(|literal| {
        ast.array_literal_at(*literal).is_some_and(|array| {
            array
                .elements
                .iter()
                .any(|element| ast.peel_ts_sugar_span(element.span) == own)
        })
    });
    let in_class = ast
        .classes
        .iter()
        .flat_map(|class| &class.elements)
        .any(|element| element.value.is_some_and(|value| value.contains(own)));
    let tag = crate::owners::function_binding_name(file, function).is_some_and(|name| {
        ast.tagged_template_tags
            .iter()
            .any(|tag| ast.reference_declaration(*tag) == Some(name.span))
    });
    (in_array || in_class || tag).then_some(LayerCover::Unresolved)
}

#[cfg(test)]
mod tests {
    use super::{PropsOn, direct_members, key_is, props_on};
    use solid_facts::ast;
    use solid_facts::compiler::ExecutionMap;
    use solid_facts::core::SourceHash;

    fn file(source: &str) -> solid_facts::FileFacts {
        const PATH: &str = "test.tsx";
        solid_facts::FileFacts {
            generation: solid_facts::core::Generation::new(1).unwrap(),
            path: solid_facts::core::SourcePath::new(PATH).unwrap(),
            source_hash: SourceHash::of(source),
            source: std::sync::Arc::from(source),
            ast: std::sync::Arc::new(ast::extract(PATH, source).unwrap()),
            compiler: std::sync::Arc::new(ExecutionMap::inert(SourceHash::of(source))),
        }
    }

    /// The `on` the first call's first argument gives.
    fn on_of(source: &str) -> &'static str {
        let file = file(source);
        let argument = file
            .ast
            .calls
            .first()
            .and_then(|call| call.arguments.first());
        match props_on(&file, "on", argument) {
            PropsOn::Absent => "absent",
            PropsOn::Member(property) if property.data => "data",
            PropsOn::Member(_) => "accessor",
            PropsOn::Open => "open",
        }
    }

    #[test]
    fn a_closed_literal_states_the_absence_and_the_kind_of_on() {
        assert_eq!(on_of("f({ fallback: 0 });"), "absent");
        assert_eq!(on_of("f({ get children() { return 1; } });"), "absent");
        assert_eq!(on_of("f({ on: t, children: 1 });"), "data");
        assert_eq!(on_of("f({ get on() { return t(); } });"), "accessor");
        assert_eq!(on_of("f({ on() { return t(); } });"), "accessor");
    }

    #[test]
    fn a_literal_that_is_not_closed_or_no_literal_leaves_on_open() {
        assert_eq!(on_of("f({ ...props });"), "open");
        assert_eq!(on_of("f({ [key]: 1 });"), "open");
        // A quoted key is no static identifier: the facts do not close such a literal.
        assert_eq!(on_of("f({ 'on': t });"), "open");
        // A getter-bearing literal is closed only while no spread and no computed key can add an `on`.
        assert_eq!(
            on_of("f({ get children() { return 1; }, ...rest });"),
            "open"
        );
        assert_eq!(
            on_of("f({ ...rest, get children() { return 1; } });"),
            "open"
        );
        assert_eq!(
            on_of("f({ get children() { return 1; }, [key]: 1 });"),
            "open"
        );
        assert_eq!(on_of("f({ get [key]() { return 1; } });"), "open");
        assert_eq!(on_of("f(props);"), "open");
        assert_eq!(on_of("f(...rest);"), "open");
        assert_eq!(on_of("f();"), "absent");
    }

    #[test]
    fn only_the_members_written_in_the_literal_itself_are_direct() {
        let source = "f({ get children() { return { on: 1 }; }, fallback: { on: 2 }, on: t });";
        let file = file(source);
        let argument = file.ast.calls[0].arguments[0].span;
        let keys = direct_members(&file, argument)
            .iter()
            .map(|property| file.source_text(property.key).unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(keys, ["children", "fallback", "on"]);
        // The `on` of the nested literals is not the props'.
        assert_eq!(
            on_of("f({ get children() { return { on: 1 }; } });"),
            "absent"
        );
        assert!(key_is(&file, file.ast.object_properties[0].key, "children"));
        assert!(!key_is(&file, file.ast.object_properties[0].key, "on"));
    }
}
