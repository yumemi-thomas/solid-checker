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
    for file in crate::base::reads::all_files(facts) {
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
                    analysis_context: if reason.contains("not resolved")
                        || reason.contains("accessor")
                        || reason.contains("handed")
                    {
                        crate::blockers::FUNCTION_RUN_UNPLACED.to_owned()
                    } else {
                        String::new()
                    },
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
/// is typed `any`, so `tsc` is silent.
///
/// ADR 0650: every form that hands `on` is read, and only a proven non-function
/// states nothing. A value proven to be a function is a violation; anything
/// else (`any`, `unknown`, a union, a possibly undefined function, a spread not
/// closed against `on`, an options object that is no exact literal) is an
/// uncertifiable with [`crate::blockers::LOADING_ON_VALUE_UNPROVEN`].
fn loading_on_accessor_violations(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let primitives = lookup.primitives(file);
    let on_boundary = |primitive| {
        super::known_primitive(primitive)
            .is_some_and(|primitive| lookup.dialect.boundary_on_may_hold_outside(primitive))
    };
    for (index, element) in file.ast.jsx_elements.iter().enumerate() {
        if !on_boundary(&primitives.jsx[index]) {
            continue;
        }
        if let Some((at, value)) = jsx_on_value(file, lookup, element) {
            push_on_verdict(file, at, value, violations);
        }
    }
    // `Loading({ on, … })`: the component called by hand. Its one argument is
    // the props object, so the same question is asked of that object.
    for (index, call) in file.ast.calls.iter().enumerate() {
        if !on_boundary(&primitives.calls[index]) {
            continue;
        }
        let Some(argument) = call.arguments.first() else {
            continue;
        };
        if !argument.exact_object_literal {
            push_on_verdict(file, argument.span, OnValue::Open, violations);
            continue;
        }
        let Some(property) = file
            .ast
            .object_properties_within(argument.span)
            .find(|property| {
                argument.property_names.contains(&property.key)
                    && file.source_text(property.key) == Some("on")
            })
        else {
            continue;
        };
        let value = file.ast.peel_ts_sugar_span(property.value);
        let verdict = on_value(file, lookup, value);
        push_on_verdict(file, value, verdict, violations);
    }
}

/// What the runtime's `typeof deps === "function"` answers for one value.
#[derive(Clone, Copy, Eq, PartialEq)]
enum OnValue {
    /// Never a function: `on` states nothing about this rule.
    NotFunction,
    Function,
    Open,
}

fn on_value(file: &solid_facts::FileFacts, lookup: &SemanticLookup<'_>, value: Span) -> OnValue {
    if file
        .ast
        .functions
        .iter()
        .any(|function| function.span == value)
    {
        return OnValue::Function;
    }
    let entity = lookup.entity_at(file.path.as_str(), value);
    match entity.and_then(|entity| entity.callability) {
        Some(typefacts::Callability::NonCallable) => OnValue::NotFunction,
        // Callability skips a nullish constituent, and the warning needs the
        // function on the first run: an optional accessor proves nothing yet.
        Some(typefacts::Callability::Callable | typefacts::Callability::UntypedCallable)
            if entity
                .and_then(|entity| entity.runtime_value_domain)
                .is_some_and(|domain| {
                    domain.may_be_callable()
                        && !domain.may_be_undefined()
                        && !domain.may_be_other()
                        && !domain.unknown()
                }) =>
        {
            OnValue::Function
        }
        _ => OnValue::Open,
    }
}

/// The value `on` takes on `element`: the last attribute or spread that can set
/// it decides, as the compiled props merge does. `None` when nothing sets it.
fn jsx_on_value(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
    element: &solid_facts::ast::JsxElementFact,
) -> Option<(Span, OnValue)> {
    let attribute = element.attributes.iter().rfind(|attribute| {
        attribute.namespace.is_none() && file.source_text(attribute.local_name) == Some("on")
    });
    let settled_after = attribute.map_or(0, |attribute| attribute.span.end);
    for spread in element.spreads.iter().rev() {
        if spread.span.start < settled_after {
            break;
        }
        let closed = lookup
            .entity_at(file.path.as_str(), spread.argument)
            .and_then(|entity| entity.declared_keys.as_deref())
            .is_some_and(|keys| keys.iter().all(|key| key.as_ref() != "on"));
        if !closed {
            return Some((spread.argument, OnValue::Open));
        }
    }
    let attribute = attribute?;
    // `on` alone or `on="a"`: a literal, never a function.
    let expression = attribute.expression?;
    let value = file.ast.peel_ts_sugar_span(expression);
    Some((value, on_value(file, lookup, value)))
}

fn push_on_verdict(
    file: &solid_facts::FileFacts,
    at: Span,
    value: OnValue,
    violations: &mut Vec<StaticViolation>,
) {
    if value == OnValue::NotFunction {
        return;
    }
    let uncertain = value == OnValue::Open;
    violations.push(StaticViolation {
        id: "SC5007".into(),
        rule: "loading-on-accessor".into(),
        message: if uncertain {
            "`on` may be a function here, and nothing proves it is not; the boundary never calls \
             a function, so nothing would be read and the boundary would never re-arm"
                .into()
        } else {
            "`on` received a function; the boundary never calls it, so nothing is read and the \
             boundary never re-arms"
                .into()
        },
        hint: "Pass the dependency's value: `on={route()}`, or `on={[query(), page()]}` for \
               several."
            .into(),
        location: location(file.path.shared(), at),
        analysis_context: if uncertain {
            crate::blockers::LOADING_ON_VALUE_UNPROVEN.to_owned()
        } else {
            String::new()
        },
        fixes: vec![],
        uncertain,
    });
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
            // An exact setter's call reads no source; a read in its argument
            // is a call of its own, asked below.
            || program.writes.iter().any(|write| at(&write.location))
    };
    if file.ast.calls.iter().any(|call| {
        element
            .children
            .iter()
            .any(|child| child.contains(call.span))
            && file
                .ast
                .call_at(file.ast.peel_ts_sugar_span(call.callee))
                .is_none()
            && !read_at(call.span)
            && !read_at(call.callee)
            // ADR 0660: a resolved target proves nothing where no edge follows
            // its body: a method, a slot, a class member.
            && lookup.function_called_at(path, call.callee).is_none_or(
                |(target_file, target)| {
                    crate::execution_role::unplaced_run(target_file, target.body, lookup)
                },
            )
            && lookup
                .standard_library_declaration(file, call, typefacts::CallKind::Call)
                .is_none()
    }) {
        return Some("a value it reads is not resolved");
    }
    // ADR 0660: a Get may run an accessor, and a source handed to a call under
    // the boundary has no read row at it.
    if file.ast.members.iter().any(|member| {
        under(member.span) && crate::local_access::literal_member_has_getter(file, member)
    }) {
        return Some("a Get under it may run an accessor");
    }
    let callees = file
        .ast
        .calls
        .iter()
        .map(|call| file.ast.peel_ts_sugar_span(call.callee))
        .collect::<crate::HashSet<_>>();
    let is_async_source = |declaration: Span| {
        file.ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == declaration))
            .and_then(|binding| binding.call_initializer)
            .and_then(|initializer| file.ast.call_at(initializer))
            .and_then(|call| call.arguments.first())
            .is_some_and(|argument| {
                crate::owners::computation_is_async(lookup, file, argument.span)
            })
    };
    if file.ast.identifiers.iter().any(|identifier| {
        under(identifier.span)
            && identifier.role == solid_facts::ast::IdentifierRole::Reference
            && !callees.contains(&identifier.span)
            && file
                .ast
                .reference_declaration(identifier.span)
                .is_some_and(is_async_source)
    }) {
        return Some("an async source is handed to code under it");
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

/// Batch 11, slice 0 (measurement only, [`crate::proof_candidates`]): the sites SC5006 and SC5007 start from, as the proof log's `candidate` lines. The first tests are the producers'
/// own (`loading_on_outside_hold_obligations`, `loading_on_accessor_violations`) read again, and nothing here is read by them or returned: SC5006's is a boundary element with `on`
/// (or a spread that may carry it), at the `on` attribute; SC5007's every form that hands `on` (the attribute, a spread, the props object of a `Loading({ on })` call), at the value, with
/// what the value is proven to be (`not-function` is the one answer that states nothing about the rule).
pub(crate) fn log_candidates(file: &solid_facts::FileFacts, lookup: &SemanticLookup<'_>) {
    use crate::proof_log::{self, CandidateRecord};
    let primitives = lookup.primitives(file);
    let on_boundary = |primitive| {
        super::known_primitive(primitive)
            .is_some_and(|primitive| lookup.dialect.boundary_on_may_hold_outside(primitive))
    };
    let named = |value: OnValue| match value {
        OnValue::NotFunction => "not-function",
        OnValue::Function => "function",
        OnValue::Open => "open",
    };
    let path = file.path.as_str();
    for (index, element) in file.ast.jsx_elements.iter().enumerate() {
        if !on_boundary(&primitives.jsx[index]) {
            continue;
        }
        let on = element.attributes.iter().find(|attribute| {
            attribute.namespace.is_none() && file.source_text(attribute.local_name) == Some("on")
        });
        if on.is_some() || !element.spreads.is_empty() {
            proof_log::candidate(&CandidateRecord {
                rule: RULE_ID,
                table: "loading-on",
                path,
                at: on.map_or(element.name.span, |attribute| attribute.span),
                alternates: &[],
                facts: &[("on", if on.is_some() { "attribute" } else { "spread" })],
            });
        }
        if let Some((at, value)) = jsx_on_value(file, lookup, element) {
            proof_log::candidate(&CandidateRecord {
                rule: "SC5007",
                table: "loading-on-value",
                path,
                at,
                alternates: &[],
                facts: &[("value", named(value)), ("form", "element")],
            });
        }
    }
    for (index, call) in file.ast.calls.iter().enumerate() {
        if !on_boundary(&primitives.calls[index]) {
            continue;
        }
        let Some(argument) = call.arguments.first() else {
            continue;
        };
        let (at, value) = if argument.exact_object_literal {
            let Some(property) =
                file.ast
                    .object_properties_within(argument.span)
                    .find(|property| {
                        argument.property_names.contains(&property.key)
                            && file.source_text(property.key) == Some("on")
                    })
            else {
                continue;
            };
            let value = file.ast.peel_ts_sugar_span(property.value);
            (value, on_value(file, lookup, value))
        } else {
            (argument.span, OnValue::Open)
        };
        proof_log::candidate(&CandidateRecord {
            rule: "SC5007",
            table: "loading-on-value",
            path,
            at,
            alternates: &[],
            facts: &[("value", named(value)), ("form", "call")],
        });
    }
}
