//! Exact spread-handler targets and explicit unplaced-handler obligations.

use super::*;

/// What the spread argument of an intrinsic element hands the element as event
/// handlers (ADR 0358): the `on…` functions its exact object literal holds, each
/// with the key it is held under (its event type, ADR 0361), or `None` when that
/// object cannot be resolved exactly. Keyed by the argument's span, like
/// [`Targets`].
pub(crate) type SpreadTargets = HashMap<Location, Option<Vec<SpreadHandler>>>;

/// A function a spread supplies: its file, its span and the key it is held under.
pub(crate) type SpreadHandler = (String, Span, String);

/// Whether a JSX element is an intrinsic (lowercase, unqualified) tag, the only
/// kind a spread can hand event listeners to.
pub(crate) fn intrinsic_element(file: &FileFacts, element: &ast::JsxElementFact) -> bool {
    element.member_object.is_none()
        && file
            .source_text(element.name.span)
            .is_some_and(|tag| tag.starts_with(|c: char| c.is_ascii_lowercase()))
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
        targets.push((target.path.to_string(), function.span, name.to_owned()));
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
                if lookup.spread_handler_targets().get(&key) != Some(&None) {
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
