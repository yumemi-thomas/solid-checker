//! ADR 0645: a listener runs on the stack of whatever delivers it. The rules
//! place what is written in a listener, and a function they place themselves;
//! a method it calls, a listener that is not a function literal, or a function
//! that is also a JSX handler takes no role from the registration, so a
//! `window.dispatchEvent` in a body certified its reads and writes while Chrome
//! warned and threw. A listener is no JSX handler, so the delivery proofs' own
//! exclusions do not hold for it: the question is whether any dispatch may run
//! with an owner or a window ([`crate::indexes`]'s listener veto), and then
//! whether the listener reaches project code. Which event a site can deliver
//! and which listener it reaches are not read here; that is the proof.

use crate::indexes::SemanticLookup;
use crate::{StaticDefect, StaticDefectKind, location};
use solid_facts::FileFacts;
use solid_facts::ast::{CallFact, FunctionFact};
use solid_facts::core::Span;

fn registers(file: &FileFacts, callee: Span) -> bool {
    let ast = &file.ast;
    let callee = ast.peel_ts_sugar_span(callee);
    match ast.member_at(callee) {
        Some(member) if ast.computed_members.binary_search(&member.span).is_ok() => ast
            .literal_computed_members
            .iter()
            .any(|literal| literal.span == member.span && literal.key == "addEventListener"),
        Some(member) => file.source_text(member.property) == Some("addEventListener"),
        None => {
            file.source_text(callee) == Some("addEventListener")
                && ast.reference_declaration(callee).is_none()
        }
    }
}

/// A call of code the project wrote: a binding of its own, or a member of one
/// no library declares. A Solid primitive is the rules' own question.
fn calls_project_code(file: &FileFacts, call: &CallFact, lookup: &SemanticLookup<'_>) -> bool {
    let ast = &file.ast;
    if lookup.primitive_at_call(file, call.span).is_some() {
        return false;
    }
    let mut root = ast.peel_ts_sugar_span(call.callee);
    if ast.member_at(root).is_some()
        && lookup
            .resolved_callee_call(file, call.callee)
            .and_then(|resolved| resolved.declaration.as_ref())
            .is_some_and(|declaration| declaration.standard_library)
    {
        return false;
    }
    while let Some(member) = ast.member_at(root) {
        root = ast.peel_ts_sugar_span(member.object);
    }
    ast.reference_declaration(root).is_some()
}

fn body_calls_project_code(
    file: &FileFacts,
    function: &FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast
        .calls
        .iter()
        .any(|call| function.body.contains(call.span) && calls_project_code(file, call, lookup))
}

/// Whether the value registered at `listener` may run project code.
fn reaches_project_code(file: &FileFacts, listener: Span, lookup: &SemanticLookup<'_>) -> bool {
    let listener = file.ast.peel_ts_sugar_span(listener);
    if let Some(function) = file.ast.function_at(listener) {
        return body_calls_project_code(file, function, lookup);
    }
    match lookup.function_called_at(file.path.as_str(), listener) {
        Some((declared, function)) => {
            body_calls_project_code(declared, function, lookup) || captures(declared, function)
        }
        None => true,
    }
}

/// A named function takes its role from its other uses, so what it reads is no
/// question of this registration's literal: a Get of a captured store is no
/// call, and is as unplaced as one (`JSON.stringify(store)`, `for (… of store)`).
fn captures(file: &FileFacts, function: &FunctionFact) -> bool {
    file.ast.identifiers.iter().any(|identifier| {
        function.body.contains(identifier.span)
            && file
                .ast
                .reference_declaration(identifier.span)
                .is_some_and(|declaration| !function.span.contains(declaration))
    })
}

fn push(file: &FileFacts, site: Span, defects: &mut Vec<StaticDefect>) {
    defects.push(StaticDefect {
        kind: StaticDefectKind::ReactiveDispatchUnresolved {
            callee: file.source_text(site).unwrap_or("this call").to_owned(),
            member: None,
        },
        location: location(file.path.shared(), site),
        analysis_context: crate::LISTENER_REACH.to_owned(),
        fixes: vec![],
        uncertain: true,
    });
}

/// ADR 0645's obligations of one file.
pub(crate) fn check_file(
    file: &FileFacts,
    lookup: &SemanticLookup<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    let ast = &file.ast;
    let mut sites = ast
        .calls
        .iter()
        .filter(|call| registers(file, call.callee))
        .peekable();
    let read_of_the_method =
        |span: Span| registers(file, span) && ast.calls_with_peeled_callee(span).next().is_none();
    let reads = ast
        .members
        .iter()
        .map(|member| member.span)
        .chain(ast.identifiers.iter().map(|identifier| identifier.span))
        .filter(|span| read_of_the_method(*span))
        .filter(|span| !ast.members.iter().any(|member| member.property == *span))
        .collect::<Vec<_>>();
    if (sites.peek().is_none() && reads.is_empty())
        || !lookup.synchronous_listener_dispatch_present()
    {
        return;
    }
    for call in sites {
        let reaches = call
            .arguments
            .iter()
            .take(2)
            .any(|argument| argument.spread)
            || call
                .arguments
                .get(1)
                .is_some_and(|listener| reaches_project_code(file, listener.span, lookup));
        if reaches {
            push(file, call.span, defects);
        }
    }
    for read in reads {
        push(file, read, defects);
    }
}
