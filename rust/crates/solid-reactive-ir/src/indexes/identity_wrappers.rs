//! Identity wrappers: project functions whose every completion returns one
//! parameter unchanged (ADR 0346).
//!
//! `wrap(() => body)` evaluates to the literal exactly when `wrap` returns its
//! parameter on every completion and nothing else reads that parameter: a
//! wrapper that stores or calls its callback, returns another function on some
//! path, or falls through to `undefined` answers nothing, and so does an async
//! or generator function, whose result is a promise or an iterator.

use super::*;
use crate::location;
use crate::owners::{callback_literal_span, containing_ast_function, is_identifier_span};

impl<'a> SemanticLookup<'a> {
    /// The parameter index `symbol`'s function returns unchanged, if it is an
    /// identity wrapper. Keyed by the exact declaration symbol of the function.
    pub(crate) fn identity_wrapper_parameter(&self, symbol: &str) -> Option<usize> {
        self.identity_wrappers().get(symbol).copied()
    }

    /// Every identity wrapper of the project, keyed by its declaration symbol.
    /// Built once; the owner cache digests it, so a wrapper that changes in
    /// another file invalidates the callers that resolved through it.
    pub(super) fn identity_wrappers(&self) -> &HashMap<&'a str, usize> {
        self.identity_wrappers.get_or_init(|| {
            let mut wrappers = HashMap::default();
            for file in self.facts.files.iter() {
                for function in &file.ast.functions {
                    let Some(index) = returned_parameter(file, self.entities, function) else {
                        continue;
                    };
                    let Some(symbol) = function_symbol(file, self.entities, function) else {
                        continue;
                    };
                    wrappers.entry(symbol).or_insert(index);
                }
            }
            wrappers
        })
    }
}

/// The exact symbol a function is bound to: a declaration's name, or the
/// single `const` a function literal initializes. A function reached through
/// any other binding has no symbol here, so no call names it. A declaration
/// that is written somewhere (`wrap = function …`, a redeclaration) no longer
/// holds the function it declares, so it has no symbol either.
fn function_symbol<'e>(
    file: &solid_facts::FileFacts,
    entities: &'e EntitySymbols,
    function: &solid_facts::ast::FunctionFact,
) -> Option<&'e str> {
    if function.kind == solid_facts::ast::FunctionKind::Declaration {
        let name = function.name.as_ref()?;
        if crate::indexes::binding_written_through_wrappers(file, name.span) {
            return None;
        }
        return entities
            .exact(&location(file.path.shared(), name.span))
            .map(SymbolId::as_str);
    }
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.initializer_function
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.len() == 1
            && binding.initializer.is_some_and(|initializer| {
                file.ast.peel_ts_sugar_span(initializer) == function.span
            })
    })?;
    let name = binding.names.first()?;
    entities
        .exact(&location(file.path.shared(), name.span))
        .map(SymbolId::as_str)
}

/// The parameter index `function` returns unchanged on every completion, and
/// no other reference to that parameter exists in the function.
fn returned_parameter(
    file: &solid_facts::FileFacts,
    entities: &EntitySymbols,
    function: &solid_facts::ast::FunctionFact,
) -> Option<usize> {
    if function.r#async || function.generator || function.parameters.is_empty() {
        return None;
    }
    let returns: Vec<&solid_facts::ast::ReturnFact> = if function.expression_body {
        function.expression_return.iter().collect()
    } else {
        file.ast
            .returns
            .iter()
            .filter(|returned| {
                containing_ast_function(&file.ast, returned.span)
                    .is_some_and(|owner| owner.span == function.span)
            })
            .collect()
    };
    if returns.is_empty() {
        return None;
    }
    // Every return names the same parameter, by exact symbol. A returned
    // identifier may sit inside TypeScript wrappers (`cb as X`, `cb!`) that
    // keep its value; `value_symbol` reads through them, and the identifier's
    // own span is the use that the returned set excludes below.
    let mut index = None;
    let mut completes_on_identity = false;
    let mut returned_arguments = Vec::with_capacity(returns.len());
    for returned in &returns {
        if returned.value != solid_facts::ast::ReturnValueKind::Identifier {
            return None;
        }
        let argument = returned.argument?;
        returned_arguments.push(file.ast.peel_ts_sugar_span(argument));
        let symbol = entities.value_symbol(file, argument)?;
        let position = function.parameters.iter().position(|parameter| {
            parameter.shape == solid_facts::ast::BindingShape::Identifier
                && parameter.names.len() == 1
                && entities
                    .exact(&location(file.path.shared(), parameter.names[0].span))
                    .is_some_and(|parameter_symbol| parameter_symbol == symbol)
        })?;
        match index {
            None => index = Some(position),
            Some(previous) if previous != position => return None,
            Some(_) => {}
        }
        completes_on_identity |= returned.straight_line || function.expression_body;
    }
    let index = index?;
    // Some completion path returns the parameter without a branch in front of
    // it: a straight-line return, or the expression body of an arrow. Otherwise
    // a path that falls off the end returns `undefined`.
    if !completes_on_identity {
        return None;
    }
    let parameter = &function.parameters[index].names[0];
    let parameter_text = file.source_text(parameter.span)?;
    // Any other occurrence of the name in the function, its parameter list
    // included, is a use the function does not return: a store, a call, a
    // default that captures it. `arguments` and `eval` reach it without a name.
    for identifier in &file.ast.identifiers {
        if !function.span.contains(identifier.span) {
            continue;
        }
        let text = file.source_text(identifier.span);
        if text == Some("arguments") || text == Some("eval") {
            return None;
        }
        if identifier.span != parameter.span
            && text == Some(parameter_text)
            && !returned_arguments.contains(&identifier.span)
        {
            return None;
        }
    }
    Some(index)
}

/// The function literal a call-valued argument hands back: `wrap(() => …)`
/// when `wrap` resolves exactly to an identity wrapper and the literal is the
/// argument at the wrapped parameter's position (ADR 0346). A `new`, a member
/// callee, a spread before the position, or any other expression names none.
pub(crate) fn identity_wrapped_literal_span(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    argument: Span,
) -> Option<Span> {
    // `wrap(…) as X` is the same call: its wrappers keep the value.
    let call = file.ast.call_at(file.ast.peel_ts_sugar_span(argument))?;
    if call.construct || !is_identifier_span(file, call.callee) {
        return None;
    }
    let symbol = lookup.entities().value_symbol(file, call.callee)?;
    let index = lookup.identity_wrapper_parameter(symbol.as_str())?;
    if call.arguments.len() <= index
        || call
            .arguments
            .iter()
            .take(index + 1)
            .any(|argument| argument.spread)
    {
        return None;
    }
    callback_literal_span(file, call.arguments[index].span, false)
}
