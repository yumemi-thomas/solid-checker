//! Exact spread-handler targets and explicit unplaced-handler obligations.

use super::*;

/// What the spread argument of an intrinsic element hands the element as event
/// handlers (ADR 0358): the `on…` functions its exact object literal holds, or
/// `None` when that object cannot be resolved exactly. Keyed by the argument's
/// span, like [`Targets`].
pub(crate) type SpreadTargets = HashMap<Location, Option<Vec<SpreadHandler>>>;

/// One `on…` function of an exact spread holder: the key it is held under,
/// and the function (its file and span).
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct SpreadHandler {
    pub(crate) key: String,
    pub(crate) path: String,
    pub(crate) span: Span,
}

/// Whether a JSX element is an intrinsic (lowercase, unqualified) tag, the only
/// kind a spread can hand event listeners to.
pub(crate) fn intrinsic_element(file: &FileFacts, element: &ast::JsxElementFact) -> bool {
    element.member_object.is_none()
        && file
            .source_text(element.name.span)
            .is_some_and(|tag| tag.starts_with(|c: char| c.is_ascii_lowercase()))
}

/// ADR 0364: whether a later attribute or spread of `element` may replace the
/// event handler defined at `after` under the prop `key`. The compiler lowers an
/// element with any spread to `spread(el, [source, ...])`, every attribute and
/// spread in source order, and the runtime keeps a key from the last source that
/// has it (`collectProps` skips a key a later source `sourceHas`); a later
/// definition replaces it even when its value is `undefined`. Two props of one
/// event type replace each other too where the type is delegated (`addEvent`
/// stores `node["_$$" + type]`), so the test is the lowercased tail. A later
/// spread replaces it unless its holder resolves exactly (ADR 0358) and holds no
/// key of that type; an unaudited release answers `true`.
pub(crate) fn event_key_may_be_replaced(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    element: &ast::JsxElementFact,
    after: Span,
    key: &str,
) -> bool {
    let event_type =
        |key: &str| (key.is_ascii() && key.len() > 2).then(|| key[2..].to_ascii_lowercase());
    let Some(wanted) = event_type(key) else {
        return true;
    };
    let same_event = |other: &str| match lookup.dialect.spread_key_is_event(other) {
        Some(false) => false,
        Some(true) => event_type(other).is_none_or(|other| other == wanted),
        None => true,
    };
    element.attributes.iter().any(|attribute| {
        attribute.span.start > after.start
            && attribute.namespace.is_none()
            && file.source_text(attribute.name).is_none_or(same_event)
    }) || element.spreads.iter().any(|spread| {
        spread.span.start > after.start
            && match lookup
                .spread_handler_targets()
                .get(&location(file.path.shared(), spread.argument))
            {
                Some(Some(held)) => held.iter().any(|held| same_event(&held.key)),
                _ => true,
            }
    })
}

/// Every spread argument of an intrinsic element, resolved as far as the exact
/// object-literal facts allow (ADR 0358). Built once per build. A direct `eval`
/// anywhere can rewrite any object, so every spread is unresolved then.
pub(crate) fn spread_index<'a>(
    facts: &'a ProjectFacts,
    lookup: &SemanticLookup<'a>,
) -> SpreadTargets {
    let mut spreads = HashMap::default();
    let eval = facts.files.iter().any(|file| {
        file.ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind == ast::ModuleHazardKind::Eval)
    });
    let stability = MemberStability::index(facts, lookup);
    let modules = ModuleCensus::new(facts, lookup);
    let mut closed = HashMap::default();
    for file in &facts.files {
        let references = references(file);
        for element in &file.ast.jsx_elements {
            if !intrinsic_element(file, element) {
                continue;
            }
            for spread in &element.spreads {
                let argument = file.ast.peel_ts_sugar_span(spread.argument);
                let targets = if eval {
                    None
                } else {
                    spread_handlers(
                        facts,
                        lookup,
                        file,
                        argument,
                        &references,
                        &stability,
                        &modules,
                        &mut closed,
                    )
                };
                spreads.insert(location(file.path.shared(), spread.argument), targets);
            }
        }
    }
    spreads
}

/// The handlers one spread argument supplies (see [`spread_index`]). An object
/// literal written in the spread is exact on its own; a const binding must pass
/// the same receiver premises a call through it does (ADR 0299).
#[allow(clippy::too_many_arguments)]
fn spread_handlers<'a>(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'a>,
    file: &'a FileFacts,
    argument: Span,
    references: &HashSet<Span>,
    stability: &MemberStability,
    modules: &ModuleCensus,
    closed: &mut HashMap<(String, Span, SymbolId), bool>,
) -> Option<Vec<SpreadHandler>> {
    if file.source_text(argument)?.trim_start().starts_with('{') {
        return on_keys(facts, lookup, file, argument, None);
    }
    if let Some(call) = file.ast.call_at(argument) {
        return package_call_handlers(lookup, file, call);
    }
    if !references.contains(&argument) {
        return None;
    }
    let symbol = lookup.entities().at(file.path.as_str(), argument)?;
    let (owner, literal, declaration, frozen) = receiver_literal(file, argument, symbol, lookup)?;
    let stable = *closed
        .entry((owner.path.to_string(), declaration, symbol.clone()))
        .or_insert_with(|| {
            receiver_closed(facts, owner, literal, declaration, symbol, lookup, modules)
        });
    if !stable {
        return None;
    }
    if frozen
        && !matches!(
            stability.state_of(facts, owner.path.as_str(), "freeze"),
            Stability::Assumed
        )
    {
        return None;
    }
    on_keys(facts, lookup, owner, literal, Some(stability))
}

/// ADR 0372: a spread of a call of an installed package's export hands the
/// element the own keys that export's returns hold, as the host read them from
/// the package's selected runtime source. When no key is an event key under the
/// release's grammar, the spread hands no handler; any event key, an unaudited
/// release or no row at all leaves it unresolved.
fn package_call_handlers(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &ast::CallFact,
) -> Option<Vec<SpreadHandler>> {
    if call.construct {
        return None;
    }
    let (import, export) = file.ast.imported_callee(&file.source, call.callee)?;
    let keys = lookup.package_return_keys(file.path.as_str(), import.module.as_str(), &export)?;
    for key in keys {
        if lookup.dialect.spread_key_is_event(key)? {
            return None;
        }
    }
    Some(Vec::new())
}

/// The `on…` functions a closed object literal holds, each resolved exactly. A
/// key that is not exactly a literal function, an accessor, or a member the
/// receiver may have replaced (`stability`) makes the whole object unresolved.
fn on_keys<'a>(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'a>,
    owner: &'a FileFacts,
    literal: Span,
    stability: Option<&MemberStability>,
) -> Option<Vec<SpreadHandler>> {
    let shape = owner
        .ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == literal)?;
    if !shape.closed {
        return None;
    }
    let mut targets = Vec::new();
    for property in &shape.properties {
        let name = property.key.as_str();
        if !lookup.dialect.spread_key_is_event(name)? {
            continue;
        }
        if property.getter.is_some() {
            return None;
        }
        if stability.is_some_and(|stability| {
            !matches!(
                stability.state_of(facts, owner.path.as_str(), name),
                Stability::Assumed
            )
        }) {
            return None;
        }
        let (target, function) = literal_function(owner, literal, name, lookup)?;
        if !receiver_inert(target, function) {
            return None;
        }
        targets.push(SpreadHandler {
            key: name.to_owned(),
            path: target.path.to_string(),
            span: function.span,
        });
    }
    Some(targets)
}

/// Handler-valued sites the exact placements cannot resolve (ADR 0358): an
/// event attribute whose value is a member read with no exact function, and an
/// intrinsic element's spread that is unresolved. Each is an uncertain
/// obligation, so the handler's owner question is never certified clean.
pub(crate) fn unresolved_handler_defects<'a>(
    facts: &'a ProjectFacts,
    lookup: &SemanticLookup<'a>,
) -> Vec<crate::StaticDefect> {
    let mut defects = Vec::new();
    for file in &facts.files {
        let members = file
            .ast
            .members
            .iter()
            .map(|member| member.span)
            .collect::<HashSet<_>>();
        for callback in &file.compiler.callback_roles {
            if callback.role != solid_facts::compiler::CallbackRoleKind::EventHandler
                || (!members.contains(&crate::owners::handler_value_span(file, callback.span))
                    && !file.ast.array_literal_elements.iter().any(|array| {
                        file.ast.peel_ts_sugar_span(array.span)
                            == file.ast.peel_ts_sugar_span(callback.span)
                    }))
                || crate::owners::callback_literal_span(file, callback.span, true).is_some()
                || lookup.object_member_function(file, callback.span).is_some()
            {
                continue;
            }
            if never_rendered_props_value(lookup, file, callback.span) {
                continue;
            }
            let Some(expression) = file.source_text(callback.span) else {
                continue;
            };
            let attribute = file
                .ast
                .jsx_elements
                .iter()
                .flat_map(|element| &element.attributes)
                .find(|attribute| {
                    attribute.expression.is_some_and(|span| {
                        file.ast.peel_ts_sugar_span(span)
                            == file.ast.peel_ts_sugar_span(callback.span)
                    })
                })
                .and_then(|attribute| file.source_text(attribute.name))
                .unwrap_or("event handler");
            defects.push(crate::StaticDefect {
                kind: crate::StaticDefectKind::HandlerTargetUnresolved {
                    attribute: attribute.to_owned(),
                    expression: expression.to_owned(),
                },
                location: location(file.path.shared(), callback.span),
                analysis_context: "handler-member-unresolved".into(),
                fixes: vec![],
                uncertain: true,
            });
        }
        for element in &file.ast.jsx_elements {
            if !intrinsic_element(file, element) {
                continue;
            }
            for spread in &element.spreads {
                let key = location(file.path.shared(), spread.argument);
                if lookup.spread_handler_targets().get(&key) != Some(&None)
                    || never_rendered_props_value(lookup, file, spread.argument)
                {
                    continue;
                }
                let expression = file.source_text(spread.argument).unwrap_or_default();
                defects.push(crate::StaticDefect {
                    kind: crate::StaticDefectKind::HandlerTargetUnresolved {
                        attribute: "...spread".into(),
                        expression: expression.to_owned(),
                    },
                    location: location(file.path.shared(), spread.argument),
                    analysis_context: "handler-spread-unresolved".into(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
        }
    }
    defects
}

/// How many `const` and view calls a never-rendered component's props value
/// is followed through.
const PROPS_VALUE_DEPTH: usize = 8;

/// ADR 0373: whether the handler value at `span` is drawn only from the props
/// of an enclosing component the closed program never renders: its sole plain,
/// unassigned parameter, a member of it, a `const` holding such a value, or a
/// view built of it (`omit(props, …)`, a `merge` whose other arguments are
/// object literals with no event key). No tag passes that component anything,
/// so the value holds no handler. The element may still be in a never-rendered
/// body for other values, which keep their obligations.
fn never_rendered_props_value(lookup: &SemanticLookup<'_>, file: &FileFacts, span: Span) -> bool {
    file.ast
        .functions
        .iter()
        .filter(|function| function.body.contains(span))
        .any(|function| {
            plain_props_parameter(file, function).is_some_and(|props| {
                props_derived(lookup, file, span, props, PROPS_VALUE_DEPTH)
                    && lookup.function_may_be_component(file, function)
                    && lookup.component_never_rendered(file, function)
            })
        })
}

/// The name of `function`'s sole parameter when it is a plain identifier with
/// no default that no assignment rewrites (ADR 0214's props root).
fn plain_props_parameter(file: &FileFacts, function: &ast::FunctionFact) -> Option<Span> {
    let [parameter] = function.parameters.as_slice() else {
        return None;
    };
    let [name] = parameter.names.as_slice() else {
        return None;
    };
    if function.rest_parameter || parameter.initializer.is_some() || parameter.pattern != name.span
    {
        return None;
    }
    let rewritten = file.ast.assignments.iter().any(|assignment| {
        file.ast
            .reference_declaration(file.ast.peel_ts_sugar_span(assignment.target))
            == Some(name.span)
    });
    (!rewritten).then_some(name.span)
}

/// Whether the value at `span` is the props root declared at `props`, a member
/// of it, or a `const` or props view drawn from it (see
/// [`never_rendered_props_value`]).
fn props_derived(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    span: Span,
    props: Span,
    depth: usize,
) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    if let Some(member) = file.ast.member_at(span) {
        return file
            .ast
            .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
            == Some(props);
    }
    if let Some(declaration) = file.ast.reference_declaration(span) {
        if declaration == props {
            return true;
        }
        return depth > 0
            && file
                .ast
                .bindings
                .iter()
                .find(|binding| {
                    binding.immutable
                        && binding.shape == ast::BindingShape::Identifier
                        && binding.names.len() == 1
                        && binding.names[0].span == declaration
                })
                .and_then(|binding| binding.initializer)
                .is_some_and(|initializer| {
                    props_derived(lookup, file, initializer, props, depth - 1)
                });
    }
    let Some(call) = file.ast.call_at(span) else {
        return false;
    };
    if call.construct || depth == 0 {
        return false;
    }
    let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
        return false;
    };
    if lookup.dialect.splits_props(primitive) {
        return call.arguments.first().is_some_and(|first| {
            !first.spread && props_derived(lookup, file, first.span, props, depth - 1)
        });
    }
    lookup.dialect.merges_props_reactivity(primitive)
        && call.arguments.iter().all(|argument| {
            !argument.spread
                && (props_derived(lookup, file, argument.span, props, depth - 1)
                    || literal_without_event_key(lookup, file, argument.span))
        })
}

/// Whether `span` is a closed object literal none of whose keys is an event
/// key under the release's grammar.
fn literal_without_event_key(lookup: &SemanticLookup<'_>, file: &FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    file.ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == span)
        .is_some_and(|shape| {
            shape.closed
                && shape.properties.iter().all(|property| {
                    property.getter.is_none()
                        && lookup.dialect.spread_key_is_event(&property.key) == Some(false)
                })
        })
}
