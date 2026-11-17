//! ADR 1022: a construct-only local class places its members at their `new` or receiver call.
//!
//! A class element's code (a constructor, an instance field initializer, a method) has no call edge: it runs at a
//! construction or a member call the walks do not follow, so every net calls it unplaced (ADR 0516, 0538, 0673). For one
//! shape that is too much: a class whose only reference is a single `new`, written after the class in the same function
//! body and outside JSX, in a function that is neither async nor a generator. Its constructor and instance field
//! initializers run exactly there; a method runs exactly at the one member call made directly on that `new`. With no
//! `this` and no `super` in the class the instance never escapes, so nothing else can run a member. The run is a statement
//! of the function the class is written in, on its stack, under the owner and tracking context that function's own code
//! has: the role its code takes where the class is written. So the placed member keeps the lexical role every path
//! already gives it, and the nets that refuse class members stand down for it alone.
//!
//! A member no run places (a method nothing calls, a class never constructed, a second `new`) is answered as before:
//! unplaced, an obligation, never a violation and never silence (the owner's 2026-11-17 ruling, ADR 1021).

use solid_facts::ast::{ClassElementFact, ClassElementKind, ClassFact};
use solid_facts::core::Span;

/// Where `span`'s class element runs: `Some(run)` when `span` is written in the own extent of a constructor, an instance field
/// initializer or a method of a construct-only class (no function between the two but the element's own), and that element
/// runs only at `run`, a call in the function the class is written in. AST only.
pub(crate) fn construct_only_member_run(file: &solid_facts::FileFacts, span: Span) -> Option<Span> {
    let ast = &file.ast;
    let class = ast
        .classes
        .iter()
        .filter(|class| class.span.contains(span))
        .min_by_key(|class| class.span.end - class.span.start)?;
    let element = class
        .elements
        .iter()
        .find(|element| element.value.is_some_and(|value| value.contains(span)))?;
    let value = element.value?;
    // The element's own extent: a function written inside it (other than a method's own) runs where that function runs.
    let own_function = match element.kind {
        ClassElementKind::Constructor | ClassElementKind::Method => {
            let function = ast.function_at(value)?;
            if function.r#async || function.generator || !function.body.contains(span) {
                return None;
            }
            Some(function.span)
        }
        // A field initializer that holds a function (an IIFE, a closure) is left as it was: the walks would climb from the
        // function to the initializer and place it, while the summaries keep a call there unplaced, so a claim no path makes
        // would be silence.
        ClassElementKind::Field if !element.r#static => {
            if ast.functions_within(value).next().is_some() {
                return None;
            }
            None
        }
        _ => return None,
    };
    if ast
        .functions_within(value)
        .any(|function| Some(function.span) != own_function && function.span.contains(span))
    {
        return None;
    }
    let construction = sole_construction(file, class)?;
    let run = match element.kind {
        ClassElementKind::Method => method_run(file, class, element, construction)?,
        _ => construction,
    };
    run_in_class_body(file, class, run).then_some(run)
}

/// Whether `span` is written directly in a placed member ([`construct_only_member_run`]). Only there: a site in a function
/// written inside the member (a closure, an IIFE, a helper's callback) keeps the answer it had, because the summaries that
/// would claim it at its call do not follow a call held in a class (`body_site_runs_during_call`), and a placement whose
/// claim no path makes would be silence, not a proof.
pub(crate) fn construct_only_placed(file: &solid_facts::FileFacts, span: Span) -> bool {
    construct_only_member_run(file, span).is_some()
}

/// The function of the placed constructor or method `span` is written directly in: it runs as code of the function the class
/// is written in, not as a named helper whose reads wait for a call edge. `None` for a field initializer (no function) and
/// for anything not placed.
pub(crate) fn construct_only_member_function(
    file: &solid_facts::FileFacts,
    span: Span,
) -> Option<Span> {
    construct_only_member_run(file, span)?;
    crate::owners::owning_function(&file.ast, span)
        .map(|function| function.span)
        .filter(|function| {
            file.ast.classes.iter().any(|class| {
                class
                    .elements
                    .iter()
                    .any(|element| element.value == Some(*function))
            })
        })
}

/// The run of `literal`, a placed constructor or method, for a walk that moves from a function to the call that runs it as
/// it does from an IIFE to its call: `Some(run)` when `span` is written directly in `literal` and `literal` is the function
/// of a placed member ([`construct_only_member_run`]).
pub(crate) fn construct_only_literal_run(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    span: Span,
) -> Option<Span> {
    let run = construct_only_member_run(file, span)?;
    file.ast
        .classes
        .iter()
        .any(|class| {
            class
                .elements
                .iter()
                .any(|element| element.value == Some(literal.span))
        })
        .then_some(run)
}

/// The one `new` of a construct-only class: the class is bound to a name that is not exported, has no `extends`, no
/// decorator, no computed key, no `this` and no `super`, the file runs no code it builds (`eval`), and every reference to its
/// name is that one construction's callee.
fn sole_construction(file: &solid_facts::FileFacts, class: &ClassFact) -> Option<Span> {
    let ast = &file.ast;
    if class.heritage.is_some()
        || class.elements.iter().any(|element| element.computed)
        || !ast.code_sinks.is_empty()
    {
        return None;
    }
    let text = file.source_text(class.span)?;
    if text.contains('@')
        || has_word(text, "this")
        || has_word(text, "super")
        || has_word(text, "arguments")
    {
        return None;
    }
    let declaration = class_binding(file, class)?;
    if super::flow_binding_exported(file, declaration) {
        return None;
    }
    let mut references = ast
        .reference_identifiers_to(declaration)
        .map(|reference| reference.span)
        .filter(|reference| *reference != declaration);
    let reference = references.next()?;
    if references.next().is_some() {
        return None;
    }
    let mut constructions = ast
        .calls_with_peeled_callee(reference)
        .filter(|call| call.construct);
    let construction = constructions.next()?;
    constructions.next().is_none().then_some(construction.span)
}

/// The name a class is bound to: its own (a declaration), or a `const` whose initializer is the class.
fn class_binding(file: &solid_facts::FileFacts, class: &ClassFact) -> Option<Span> {
    if let Some(name) = &class.name {
        // A named class expression binds its name inside itself only; the value is held by whatever holds the expression.
        let declared = file.ast.bindings.iter().any(|binding| {
            binding
                .initializer
                .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == class.span)
        });
        return (!declared).then_some(name.span);
    }
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding
                .initializer
                .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == class.span)
    })?;
    Some(binding.names.first()?.span)
}

/// The call that runs `element`, a method, on the construction: `new K(…).m(…)`, the member read directly on the `new` and
/// called at once. The method must be the class's only element under its key, a plain non-static method other than the
/// constructor. Any other use of the construction (discarded, stored, another member read) runs no method.
fn method_run(
    file: &solid_facts::FileFacts,
    class: &ClassFact,
    element: &ClassElementFact,
    construction: Span,
) -> Option<Span> {
    let ast = &file.ast;
    if element.r#static || element.computed {
        return None;
    }
    let key = file.source_text(element.key?)?;
    if key == "constructor"
        || class
            .elements
            .iter()
            .filter(|other| other.key.and_then(|span| file.source_text(span)) == Some(key))
            .count()
            != 1
    {
        return None;
    }
    let member = ast
        .members
        .iter()
        .find(|member| ast.peel_ts_sugar_span(member.object) == construction)?;
    if ast.computed_members.contains(&member.span) || file.source_text(member.property)? != key {
        return None;
    }
    let call = ast
        .calls_with_peeled_callee(member.span)
        .find(|call| !call.construct)?;
    Some(call.span)
}

/// Whether `run` is a call of the function the class is written in, after the class, outside JSX, in no class element, and
/// that function is neither async nor a generator: it runs on that function's stack, in the context its own code has.
fn run_in_class_body(file: &solid_facts::FileFacts, class: &ClassFact, run: Span) -> bool {
    let ast = &file.ast;
    let host = crate::owners::owning_function(ast, class.span);
    let run_host = crate::owners::owning_function(ast, run);
    if host.map(|function| function.span) != run_host.map(|function| function.span)
        || host.is_some_and(|function| {
            function.r#async || function.generator || !function.body.contains(run)
        })
        || run.start < class.span.end
        || ast.classes.iter().any(|other| other.span.contains(run))
    {
        return false;
    }
    !ast.jsx_containing(run)
        .any(|element| host.is_none_or(|function| function.span.contains(element.span)))
}

/// Whether `word` occurs in `text` as a whole identifier.
fn has_word(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    text.match_indices(word).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(ident)
            && !text[at + word.len()..].chars().next().is_some_and(ident)
    })
}
