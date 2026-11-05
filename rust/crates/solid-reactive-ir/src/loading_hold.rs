//! ADR 0517: `LOADING_ON_OUTSIDE_HOLD` as an obligation.
//!
//! rc.17's `reportOutsideHold` (`@solidjs/signals` `dist/dev.js`) warns when a
//! `<Loading on>` re-armed by a change waits on a source whose current flight
//! a frame reader outside the boundary also holds: the fallback can never be
//! seen. Proving the warning needs runtime state the facts do not carry (the
//! boundary instance, the source flight of the same flush, the outside
//! reader's visibility, the display-ahead lane), so no violation is claimed
//! here. What is claimed is the absence of a proof: a boundary with `on` (or
//! a spread that may carry it) under which an async source is read that is
//! also referenced outside it, or whose children the analysis cannot see
//! into, is an uncertifiable obligation, never a certified clean. A boundary
//! under which no async source is read holds nothing and stays clean.

use crate::{Program, SemanticLookup, StaticViolation, location};
use solid_facts::core::Span;

const RULE_ID: &str = "SC5006";
const RULE_NAME: &str = "loading-on-outside-hold";

pub(crate) fn loading_on_outside_hold_obligations(
    facts: &solid_facts::ProjectFacts,
    program: &Program,
    lookup: &SemanticLookup<'_>,
) -> Vec<StaticViolation> {
    let mut obligations = Vec::new();
    for file in &facts.files {
        loading_on_accessor_violations(file, lookup, &mut obligations);
        let primitives = lookup.primitives(file);
        for (index, element) in file.ast.jsx_elements.iter().enumerate() {
            if !super::known_primitive(&primitives.jsx[index])
                .is_some_and(|primitive| lookup.dialect.boundary_on_may_hold_outside(primitive))
            {
                continue;
            }
            let on = element.attributes.iter().find(|attribute| {
                attribute.namespace.is_none()
                    && file.source_text(attribute.local_name) == Some("on")
            });
            if on.is_none() && element.spreads.is_empty() {
                continue;
            }
            // An `on` that names nothing (`on={1}`) has no dependency to
            // change, so it never re-arms; a spread may still carry another.
            if element.spreads.is_empty()
                && on.is_some_and(|attribute| {
                    attribute.value.is_some_and(|value| {
                        file.ast.identifiers_within(value).next().is_none()
                            && file.ast.calls.iter().all(|call| !value.contains(call.span))
                    })
                })
            {
                continue;
            }
            if let Some(reason) = unproven_hold(program, lookup, file, element) {
                let at = on.map_or(element.name.span, |attribute| attribute.span);
                obligations.push(StaticViolation {
                    id: RULE_ID.into(),
                    rule: RULE_NAME.into(),
                    message: format!(
                        "this Loading boundary re-arms when its `on` dependencies change, and {reason}; if a source it waits on is also read outside it in the same flush, the frame waits on that read and the fallback is never seen (Solid warns LOADING_ON_OUTSIDE_HOLD in dev), and whether that happens cannot be proven either way"
                    ),
                    hint: "Move the outside read under the boundary so one hold owns the data, or read `latest()` in `on` to show the fallback beside the held frame.".into(),
                    location: location(file.path.shared(), at),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
        }
    }
    obligations
}

/// ADR 0624: `LOADING_ON_ACCESSOR`. rc.21 warns once, on the boundary's first
/// run, when `on` evaluates to a function: it is the dependency's value, and an
/// accessor handed instead is never called, so the boundary never re-arms. `on`
/// is typed `any`, so `tsc` is silent. Proven for a function literal and for a
/// value Type Facts call a function; a mixed union is an obligation, and any
/// other value states nothing.
fn loading_on_accessor_violations(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let primitives = lookup.primitives(file);
    for (index, element) in file.ast.jsx_elements.iter().enumerate() {
        if !super::known_primitive(&primitives.jsx[index])
            .is_some_and(|primitive| lookup.dialect.boundary_on_may_hold_outside(primitive))
        {
            continue;
        }
        let Some(value) = element
            .attributes
            .iter()
            .find(|attribute| {
                attribute.namespace.is_none()
                    && file.source_text(attribute.local_name) == Some("on")
            })
            .and_then(|attribute| attribute.expression)
            .map(|value| file.ast.peel_ts_sugar_span(value))
        else {
            continue;
        };
        let literal = file
            .ast
            .functions
            .iter()
            .any(|function| function.span == value);
        let callability = lookup
            .entity_at(file.path.as_str(), value)
            .and_then(|entity| entity.callability);
        let proven = literal
            || matches!(
                callability,
                Some(typefacts::Callability::Callable | typefacts::Callability::UntypedCallable)
            );
        if !proven && callability != Some(typefacts::Callability::Mixed) {
            continue;
        }
        violations.push(StaticViolation {
            id: "SC5007".into(),
            rule: "loading-on-accessor".into(),
            message: "`on` received a function; the boundary never calls it, so nothing is read \
                      and the boundary never re-arms"
                .into(),
            hint: "Pass the dependency's value: `on={route()}`, or `on={[query(), page()]}` for \
                   several."
                .into(),
            location: location(file.path.shared(), value),
            analysis_context: String::new(),
            fixes: vec![],
            uncertain: !proven,
        });
    }
}

/// Why the boundary's hold is not proven to be its own, or `None` when no
/// async source is read under it and its children are all seen.
fn unproven_hold(
    program: &Program,
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> Option<&'static str> {
    // Children handed by an attribute (`children={…}`) are rendered where
    // the boundary puts them; this scan reads only the element's own children.
    if element
        .attributes
        .iter()
        .any(|attribute| file.source_text(attribute.local_name) == Some("children"))
    {
        return Some("its children are supplied by an attribute");
    }
    let path = file.path.as_str();
    let under = |span: Span| element.children.iter().any(|child| child.contains(span));
    let at = |location: &typefacts::Location| {
        location.path.as_ref() == path
            && u32::try_from(location.start_byte)
                .ok()
                .zip(u32::try_from(location.end_byte).ok())
                .is_some_and(|(start, end)| under(Span::new(start, end)))
    };
    // A component, a spread or an unresolved tag under the boundary renders
    // reads this file does not show. A Solid control-flow element (`Show`,
    // `For`) renders children written here, whose reads are this file's.
    let primitives = lookup.primitives(file);
    if file
        .ast
        .jsx_elements
        .iter()
        .enumerate()
        .any(|(index, nested)| {
            nested.span != element.span
                && under(nested.span)
                && super::known_primitive(&primitives.jsx[index]).is_none()
                && (nested.member_object.is_some()
                    || !nested.spreads.is_empty()
                    || file
                        .source_text(nested.name.span)
                        .and_then(|name| name.chars().next())
                        .is_none_or(|first| !first.is_ascii_lowercase()))
        })
    {
        return Some("a component rendered under it may read an async source");
    }
    if program
        .reads
        .iter()
        .any(|read| read.uncertain && at(&read.location))
    {
        return Some("a read under it is not proven");
    }
    // A no-argument call under the boundary that is no reactive read the
    // analysis recognized and no call of a resolved project function may read
    // a source it does not see (a memo a hook in another module returns).
    let read_at = |span: Span| {
        let at = |location: &typefacts::Location| {
            location.path.as_ref() == path
                && u64::from(span.start) == location.start_byte
                && u64::from(span.end) == location.end_byte
        };
        program.reads.iter().any(|read| at(&read.location))
            || program.async_reads.iter().any(|read| at(&read.location))
    };
    if file.ast.calls.iter().any(|call| {
        call.arguments.is_empty()
            && element
                .children
                .iter()
                .any(|child| child.contains(call.span))
            && file
                .ast
                .call_at(file.ast.peel_ts_sugar_span(call.callee))
                .is_none()
            && !read_at(call.span)
            && !read_at(call.callee)
            && lookup.function_called_at(path, call.callee).is_none()
            && lookup
                .standard_library_declaration(file, call, typefacts::CallKind::Call)
                .is_none()
    }) {
        return Some("a value it reads is not resolved");
    }
    let mut sources = program
        .async_reads
        .iter()
        .filter(|read| at(&read.location))
        .map(|read| &read.declaration)
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| crate::location_order(left, right));
    sources.dedup();
    let outside = |reference: &typefacts::Location, declaration: &typefacts::Location| {
        !(reference.path == declaration.path
            && reference.start_byte == declaration.start_byte
            && reference.end_byte == declaration.end_byte)
            && !(reference.path.as_ref() == path
                && u32::try_from(reference.start_byte)
                    .ok()
                    .zip(u32::try_from(reference.end_byte).ok())
                    .is_some_and(|(start, end)| element.span.contains(Span::new(start, end))))
    };
    for declaration in sources {
        let Some(symbol) = u32::try_from(declaration.start_byte)
            .ok()
            .zip(u32::try_from(declaration.end_byte).ok())
            .and_then(|(start, end)| {
                lookup
                    .entities()
                    .at(declaration.path.as_ref(), Span::new(start, end))
            })
        else {
            return Some("an async source read under it is not resolved");
        };
        let Some(references) = lookup.symbol_references_if_present(symbol.as_str()) else {
            return Some("an async source read under it has no reference list");
        };
        if references
            .iter()
            .any(|reference| outside(reference, declaration))
        {
            return Some("an async source read under it is also used outside it");
        }
    }
    None
}
