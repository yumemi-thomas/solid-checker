//! Exact spread-handler targets and explicit unplaced-handler obligations.

use super::*;

/// What the spread argument of an intrinsic element hands the element as event
/// handlers (ADR 0358): the `on…` functions its exact object literal holds, or
/// `None` when that object cannot be resolved exactly. Keyed by the argument's
/// span, like [`Targets`].
pub(crate) type SpreadTargets = HashMap<Location, Option<Vec<(String, Span)>>>;

/// Whether a JSX element is an intrinsic (lowercase, unqualified) tag, the only
/// kind a spread can hand event listeners to.
pub(crate) fn intrinsic_element(file: &FileFacts, element: &ast::JsxElementFact) -> bool {
    element.member_object.is_none()
        && file
            .source_text(element.name.span)
            .is_some_and(|tag| tag.starts_with(|c: char| c.is_ascii_lowercase()))
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
