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
    // The dialect names the boundary's props and primitive (ADR 0978); without an async boundary nothing here is asked.
    let Some(on) = crate::loading_entries::on_prop(lookup) else {
        return obligations;
    };
    for file in crate::base::reads::all_files(facts) {
        loading_on_accessor_violations(file, lookup, on, &mut obligations);
        let primitives = lookup.primitives(file);
        for (index, element) in file.ast.jsx_elements.iter().enumerate() {
            if !super::known_primitive(&primitives.jsx[index])
                .is_some_and(|primitive| lookup.dialect.boundary_on_may_hold_outside(primitive))
            {
                continue;
            }
            let Some(at) = on_to_hold(file, on, element) else {
                continue;
            };
            if let Some(reason) = unproven_hold(program, lookup, file, element) {
                obligations.push(hold_obligation(file, at, reason));
            }
        }
        // ADR 0978: the boundary entered other than as a tag. No children proof is built for these forms: each that may pass `on` is an
        // uncertifiable obligation, never a certified clean.
        for (index, call) in file.ast.calls.iter().enumerate() {
            let Some(entry) = crate::loading_entries::entry_call(lookup, file, index) else {
                continue;
            };
            let argument = call.arguments.get(entry.props());
            let (at, reason) = match crate::loading_entries::props_on(file, on, argument) {
                crate::loading_entries::PropsOn::Absent => continue,
                crate::loading_entries::PropsOn::Member(property) => {
                    // An `on` that names nothing (`on: 1`) never re-arms, as for the tag.
                    if property.data && names_nothing(file, property.value) {
                        continue;
                    }
                    (property.span, BY_CALL)
                }
                crate::loading_entries::PropsOn::Open => (
                    argument.map_or(call.span, |argument| argument.span),
                    "its props are not resolved",
                ),
            };
            obligations.push(hold_obligation(file, at, reason));
        }
        // `createLoadingBoundary(children, fallback, { on })`: the primitive behind the boundary re-arms through the same `on`.
        for call in &file.ast.calls {
            let Some(at) = primitive_on(lookup, file, call) else {
                continue;
            };
            obligations.push(hold_obligation(file, at, BY_CALL));
        }
        for at in crate::loading_entries::loading_value_references(lookup, file) {
            obligations.push(hold_obligation(file, at, AS_VALUE));
        }
        for (index, element) in file.ast.jsx_elements.iter().enumerate() {
            // The syntactic question first: the component is resolved only for a tag that passes `on`.
            if let Some(at) = on_to_hold(file, on, element)
                && crate::loading_entries::unresolved_tag(lookup, file, index, element)
            {
                obligations.push(hold_obligation(file, at, UNRESOLVED_TAG));
            }
        }
    }
    obligations
}

/// ADR 0978: why a boundary entered other than as a tag is not proven to hold nothing.
const BY_CALL: &str = "it is entered by a call, whose children this analysis does not follow";
const AS_VALUE: &str = "it is named as a value here, so it may be entered where this analysis does not follow it, with an `on` it does not see";
const UNRESOLVED_TAG: &str =
    "the component this tag names is not resolved, so it may be the boundary";

/// Where `createLoadingBoundary(children, fallback, options)` is asked about its `on`: the options (argument 2) name one, or may
/// (a value built elsewhere, a spread). `None` for no options, no `on`, or one that names nothing.
fn primitive_on(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Option<Span> {
    use crate::loading_entries::{PropsOn, props_on};
    let shape = lookup.dialect.async_boundary_shape()?;
    if lookup.primitive_at_call(file, call.span) != Some(shape.primitive) {
        return None;
    }
    let argument = call.arguments.get(shape.options_argument);
    match props_on(file, shape.on_prop, argument) {
        PropsOn::Absent => None,
        PropsOn::Member(property) => {
            (!(property.data && names_nothing(file, property.value))).then_some(property.span)
        }
        PropsOn::Open => argument.map(|argument| argument.span),
    }
}

/// Whether the tag has an `on` attribute or a spread: the syntactic question before any component is resolved.
fn passes_on(
    file: &solid_facts::FileFacts,
    on_prop: &str,
    element: &solid_facts::ast::JsxElementFact,
) -> bool {
    !element.spreads.is_empty()
        || element.attributes.iter().any(|attribute| {
            attribute.namespace.is_none() && file.source_text(attribute.local_name) == Some(on_prop)
        })
}

/// Whether `span` names nothing: no identifier, no call.
fn names_nothing(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast.identifiers_within(span).next().is_none()
        && file.ast.calls.iter().all(|call| !span.contains(call.span))
}

/// Where the tag's `on` is asked about, or `None` when it passes none: no `on` and no spread, or an `on` that names nothing (`on={1}`, which
/// has no dependency to change and never re-arms; a spread may still carry another).
fn on_to_hold(
    file: &solid_facts::FileFacts,
    on_prop: &str,
    element: &solid_facts::ast::JsxElementFact,
) -> Option<Span> {
    let on = element.attributes.iter().find(|attribute| {
        attribute.namespace.is_none() && file.source_text(attribute.local_name) == Some(on_prop)
    });
    if on.is_none() && element.spreads.is_empty() {
        return None;
    }
    if element.spreads.is_empty()
        && on.is_some_and(|attribute| {
            attribute
                .value
                .is_some_and(|value| names_nothing(file, value))
        })
    {
        return None;
    }
    Some(on.map_or(element.name.span, |attribute| attribute.span))
}

fn hold_obligation(file: &solid_facts::FileFacts, at: Span, reason: &str) -> StaticViolation {
    StaticViolation {
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
    }
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
    on_prop: &str,
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
    // `Loading({ on, … })`: the component called by hand, and `createComponent(Loading, { on, … })` (ADR 0978). The props object is one
    // argument, so the same question is asked of that object.
    for (index, call) in file.ast.calls.iter().enumerate() {
        let Some(entry) = crate::loading_entries::entry_call(lookup, file, index) else {
            continue;
        };
        let Some(argument) = call.arguments.get(entry.props()) else {
            continue;
        };
        let (at, verdict) = props_on_value(file, lookup, on_prop, argument);
        if let Some(verdict) = verdict {
            push_on_verdict(file, at, verdict, violations);
        }
    }
    // ADR 0978: every other way the boundary is entered. A value reference gives the props to code this analysis does not follow, and an
    // unresolved tag may not be the boundary at all: neither states a violation, only that `on` may be a function there.
    for at in crate::loading_entries::loading_value_references(lookup, file) {
        push_on_verdict(file, at, OnValue::Open, violations);
    }
    for (index, element) in file.ast.jsx_elements.iter().enumerate() {
        if !passes_on(file, on_prop, element)
            || !crate::loading_entries::unresolved_tag(lookup, file, index, element)
        {
            continue;
        }
        if let Some((at, value)) = jsx_on_value(file, lookup, element) {
            push_on_verdict(file, at, unproven(value), violations);
        }
    }
}

/// A value proven to be a function is no proof where the component may not be the boundary.
fn unproven(value: OnValue) -> OnValue {
    match value {
        OnValue::NotFunction => OnValue::NotFunction,
        OnValue::Function | OnValue::Open => OnValue::Open,
    }
}

/// The `on` a props argument gives: `None` where it gives none. A literal whose property set is closed (`closed_object_literal`: no spread,
/// no computed key; accessors allowed) states the absence of `on` as well as its value (ADR 0978: a getter-bearing literal is no longer
/// open merely for being one); a data property is read as the tag's `on` is; a getter or a method is open (its value is not followed); any
/// other argument is open.
fn props_on_value(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
    on_prop: &str,
    argument: &solid_facts::ast::ArgumentFact,
) -> (Span, Option<OnValue>) {
    use crate::loading_entries::{PropsOn, props_on};
    match props_on(file, on_prop, Some(argument)) {
        PropsOn::Absent => (argument.span, None),
        PropsOn::Open => (argument.span, Some(OnValue::Open)),
        PropsOn::Member(property) if property.data => {
            let value = file.ast.peel_ts_sugar_span(property.value);
            (value, Some(on_value(file, lookup, value)))
        }
        PropsOn::Member(property) => (property.span, Some(OnValue::Open)),
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
    let Some(on_prop) = crate::loading_entries::on_prop(lookup) else {
        return;
    };
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
        let Some(entry) = crate::loading_entries::entry_call(lookup, file, index) else {
            continue;
        };
        let Some(argument) = call.arguments.get(entry.props()) else {
            continue;
        };
        let (at, value) = props_on_value(file, lookup, on_prop, argument);
        let Some(value) = value else {
            continue;
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
    // ADR 0978: the sites of the other entries, as the producers ask them (SC5006's, then SC5007's).
    for (index, call) in file.ast.calls.iter().enumerate() {
        let Some(entry) = crate::loading_entries::entry_call(lookup, file, index) else {
            continue;
        };
        let argument = call.arguments.get(entry.props());
        let at = match crate::loading_entries::props_on(file, on_prop, argument) {
            crate::loading_entries::PropsOn::Absent => continue,
            crate::loading_entries::PropsOn::Member(property) => {
                if property.data && names_nothing(file, property.value) {
                    continue;
                }
                property.span
            }
            crate::loading_entries::PropsOn::Open => {
                argument.map_or(call.span, |argument| argument.span)
            }
        };
        proof_log::candidate(&CandidateRecord {
            rule: RULE_ID,
            table: "loading-on",
            path,
            at,
            alternates: &[],
            facts: &[("on", "call")],
        });
    }
    for call in &file.ast.calls {
        if let Some(at) = primitive_on(lookup, file, call) {
            proof_log::candidate(&CandidateRecord {
                rule: RULE_ID,
                table: "loading-on",
                path,
                at,
                alternates: &[],
                facts: &[("on", "primitive")],
            });
        }
    }
    for at in crate::loading_entries::loading_value_references(lookup, file) {
        proof_log::candidate(&CandidateRecord {
            rule: RULE_ID,
            table: "loading-on",
            path,
            at,
            alternates: &[],
            facts: &[("on", "reference")],
        });
    }
    for (index, element) in file.ast.jsx_elements.iter().enumerate() {
        if let Some(at) = on_to_hold(file, on_prop, element)
            && crate::loading_entries::unresolved_tag(lookup, file, index, element)
        {
            proof_log::candidate(&CandidateRecord {
                rule: RULE_ID,
                table: "loading-on",
                path,
                at,
                alternates: &[],
                facts: &[("on", "unresolved-tag")],
            });
        }
    }
    for at in crate::loading_entries::loading_value_references(lookup, file) {
        proof_log::candidate(&CandidateRecord {
            rule: "SC5007",
            table: "loading-on-value",
            path,
            at,
            alternates: &[],
            facts: &[("value", "open"), ("form", "reference")],
        });
    }
    for (index, element) in file.ast.jsx_elements.iter().enumerate() {
        if !passes_on(file, on_prop, element)
            || !crate::loading_entries::unresolved_tag(lookup, file, index, element)
        {
            continue;
        }
        if let Some((at, value)) = jsx_on_value(file, lookup, element) {
            proof_log::candidate(&CandidateRecord {
                rule: "SC5007",
                table: "loading-on-value",
                path,
                at,
                alternates: &[],
                facts: &[
                    ("value", named(unproven(value))),
                    ("form", "unresolved-tag"),
                ],
            });
        }
    }
}
