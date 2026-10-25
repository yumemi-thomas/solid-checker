//! Identity wrappers: project functions whose every completion returns one
//! parameter unchanged (ADR 0346).
//!
//! `wrap(() => body)` evaluates to the literal exactly when `wrap` returns its
//! parameter on every completion and nothing else reads that parameter: a
//! wrapper that stores or calls its callback, returns another function on some
//! path, or falls through to `undefined` answers nothing, and so does an async
//! or generator function, whose result is a promise or an iterator.
//!
//! Two L2 memos answer it (study S4), every read recorded:
//! - [`WrapperAt`], per function (file, slot, ordinal): whether it is a
//!   wrapper, reading its own file only (implied by the key, so free);
//! - [`IdentityWrapper`], per declaration symbol: the L1 index of the
//!   functions bound to the symbol ([`identity_wrapper_candidates`]), their
//!   files' order when there are several, and their [`WrapperAt`] answers.
//!
//! A retained store reuses either answer across builds until one of its reads
//! changes. An edit that moves a wrapper's spans recomputes its [`WrapperAt`]
//! answer to the same value, and the symbol's answer is kept unrun.

use std::sync::Arc;

use super::*;
use crate::base::FileId;
use crate::base::memo::build::{
    BuildSchema, BuildTables, FileRead, IDENTITY_WRAPPERS, Input, Section, WRAPPER_AT,
};
use crate::base::memo::{Ctx, Cycle, Memo, Table, TableId};
use crate::base::{BuildConstants, FileView};
use crate::owners::{callback_literal_span, containing_ast_function, is_identifier_span};

impl SemanticLookup<'_> {
    /// The parameter index `symbol`'s function returns unchanged, if it is an
    /// identity wrapper. Keyed by the exact declaration symbol of the function.
    pub(crate) fn identity_wrapper_parameter(&self, symbol: &SymbolId) -> Option<usize> {
        self.memo_world()
            .answer::<IdentityWrapper>(&symbol.shared())
    }
}

/// The identity-wrapper answer for one declaration symbol: the parameter its
/// function returns unchanged, from the first candidate function in file
/// order that is a wrapper (several functions can bind one symbol).
pub(crate) struct IdentityWrapper;

impl Memo<BuildSchema> for IdentityWrapper {
    type Key = Arc<str>;
    type Value = Option<usize>;
    const NAME: &'static str = "identity_wrappers";
    const ID: TableId = IDENTITY_WRAPPERS;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.identity_wrappers()
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, symbol: &Arc<str>) -> Result<Option<usize>, Cycle> {
        let mut candidates = ctx.wrapper_candidates(symbol);
        // By file id, then production order: file order needs the slots
        // only when the candidates span several files.
        let mut files = candidates
            .iter()
            .map(|(file, _, _)| *file)
            .collect::<Vec<_>>();
        files.dedup();
        if files.len() > 1 {
            let slots = files
                .iter()
                .map(|file| (*file, ctx.slots(*file)))
                .collect::<Vec<_>>();
            candidates.sort_by_key(|(file, rank, ordinal)| {
                let slot = slots
                    .iter()
                    .find(|(candidate, _)| candidate == file)
                    .and_then(|(_, slots)| slots.get(*rank).copied())
                    .unwrap_or(usize::MAX);
                (slot, *ordinal)
            });
        }
        for (file, rank, ordinal) in candidates {
            let rank = u32::try_from(rank).expect("fewer than 2^32 slots of one path");
            if let Some(index) = ctx.memo::<WrapperAt>(&(file, rank, ordinal))? {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    fn on_cycle(_: &Arc<str>) -> Option<usize> {
        None
    }
}

/// Whether the `ordinal`-th function of the `rank`-th slot of `file` is an
/// identity wrapper, and of which parameter. It reads its own file only.
pub(crate) struct WrapperAt;

impl Memo<BuildSchema> for WrapperAt {
    type Key = (FileId, u32, u32);
    type Value = Option<usize>;
    const NAME: &'static str = "identity_wrapper_functions";
    const ID: TableId = WRAPPER_AT;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.wrapper_at()
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        (file, rank, ordinal): &(FileId, u32, u32),
    ) -> Result<Option<usize>, Cycle> {
        let Some(read) = ctx.file(*file, *rank as usize) else {
            return Ok(None);
        };
        Ok(read
            .facts()
            .ast
            .functions
            .get(*ordinal as usize)
            .and_then(|function| returned_parameter(&read, function)))
    }

    fn on_cycle(_: &(FileId, u32, u32)) -> Option<usize> {
        None
    }

    fn implied((file, _, _): &(FileId, u32, u32)) -> Vec<Input> {
        vec![
            Input::File(*file, Section::Source),
            Input::File(*file, Section::Entities),
        ]
    }
}

/// L1 contribution: each function of the file that can be an identity
/// wrapper (synchronous, not a generator, with a parameter), by the exact
/// symbol it is bound to, as its ordinal in the file's functions.
pub(crate) fn identity_wrapper_candidates(
    view: &FileView<'_>,
    _: &BuildConstants<'_>,
) -> Vec<(Arc<str>, u32)> {
    let file = view.file;
    // The first binding that holds each function literal as its single
    // `const` name, by the literal's span.
    let mut holders = HashMap::<Span, Span>::default();
    for binding in &file.ast.bindings {
        if binding.immutable
            && binding.initializer_function
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && let [name] = binding.names.as_slice()
            && let Some(initializer) = binding.initializer
        {
            holders
                .entry(file.ast.peel_ts_sugar_span(initializer))
                .or_insert(name.span);
        }
    }
    file.ast
        .functions
        .iter()
        .enumerate()
        .filter(|(_, function)| {
            !function.r#async && !function.generator && !function.parameters.is_empty()
        })
        .filter_map(|(ordinal, function)| {
            let symbol = function_symbol(view, &holders, function)?;
            Some((
                symbol.shared(),
                u32::try_from(ordinal).expect("fewer than 2^32 functions in one file"),
            ))
        })
        .collect()
}

/// The exact symbol a function is bound to: a declaration's name, or the
/// single `const` a function literal initializes. A function reached through
/// any other binding has no symbol here, so no call names it. A declaration
/// that is written somewhere (`wrap = function …`, a redeclaration) no longer
/// holds the function it declares, so it has no symbol either.
fn function_symbol<'v>(
    view: &FileView<'v>,
    holders: &HashMap<Span, Span>,
    function: &solid_facts::ast::FunctionFact,
) -> Option<&'v SymbolId> {
    if function.kind == solid_facts::ast::FunctionKind::Declaration {
        let name = function.name.as_ref()?;
        if crate::indexes::binding_written_through_wrappers(view.file, name.span) {
            return None;
        }
        return view.exact_symbol(name.span);
    }
    view.exact_symbol(*holders.get(&function.span)?)
}

/// The parameter index `function` returns unchanged on every completion, and
/// no other reference to that parameter exists in the function.
fn returned_parameter(
    read: &FileRead<'_, '_, '_>,
    function: &solid_facts::ast::FunctionFact,
) -> Option<usize> {
    let file = read.facts();
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
        let symbol = read.value_symbol(argument)?;
        let position = function.parameters.iter().position(|parameter| {
            parameter.shape == solid_facts::ast::BindingShape::Identifier
                && parameter.names.len() == 1
                && read
                    .exact_symbol(parameter.names[0].span)
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
    let index = lookup.identity_wrapper_parameter(symbol)?;
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
