//! Read analysis context: compute reads, async roles, and the spans allowed to hold callbacks.

use super::*;

pub(crate) fn read_analysis_context(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> String {
    if execution == ExecutionRole::EffectApply {
        // The role is shared by every effect primitive's apply position; the
        // name of the primitive whose apply this is is not. It used to be
        // `createEffect` unconditionally, which misnamed every read in a
        // `createRenderEffect` apply. Where the primitive is not one exact
        // answer -- a named function passed as the apply of two different
        // primitives -- or the dialect has no spelling for it, the sentence
        // stays true without naming one: this is read context in a message,
        // not a premise of any proof.
        match effect_apply_primitive(file, span, lookup)
            .and_then(|primitive| lookup.dialect.name_of(primitive))
        {
            Some(name) => format!("{name} apply callback"),
            None => "effect apply callback".into(),
        }
    } else {
        let context = enclosing_function_label(file, span);
        format_read_context(&context, file.ast.any_conditional_test_containing(span))
    }
}

/// Message-only: join an apply read to a read of the exact same reactive
/// declaration in its paired compute. No name or type-alias matching, and no
/// claim that a conditional compute read necessarily executes on every run.
pub(crate) fn apply_compute_reads(
    facts: &solid_facts::ProjectFacts,
    reads: &[crate::ReactiveRead],
    lookup: &SemanticLookup<'_>,
    accessors: &HashMap<SymbolId, (SymbolId, typefacts::Location)>,
    origins: &HashMap<SymbolId, (SymbolId, SymbolId, typefacts::Location)>,
) -> HashSet<typefacts::Location> {
    let callback = |file: &solid_facts::FileFacts, argument: Span| {
        let argument = file.ast.peel_ts_sugar_span(argument);
        file.ast
            .function_at(argument)
            .map(|function| (file.path.to_string(), function.span))
            .or_else(|| {
                if !file.ast.identifiers.iter().any(|identifier| {
                    identifier.span == argument
                        && identifier.role == solid_facts::ast::IdentifierRole::Reference
                }) {
                    return None;
                }
                lookup
                    .entity_symbol(file, argument)
                    .and_then(|symbol| lookup.function_for_symbol(symbol))
                    .filter(|(file, function)| lookup.function_value_is_current(file, function))
                    .map(|(file, function)| (file.path.to_string(), function.span))
            })
    };
    let mut pairs = HashMap::<(String, Span), Vec<Option<(String, Span)>>>::default();
    for file in &facts.files {
        for call in &file.ast.calls {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let Some(apply_index) =
                effect_apply_argument(lookup.dialect, primitive, call.arguments.len())
            else {
                continue;
            };
            let Some(compute_index) = (0..call.arguments.len()).find(|index| {
                lookup
                    .dialect
                    .callback_semantics_at(primitive, *index, call.arguments.len())
                    .execution
                    == Some(Execution::Tracked)
            }) else {
                continue;
            };
            let Some(apply) = callback(file, call.arguments[apply_index].span) else {
                continue;
            };
            let compute = callback(file, call.arguments[compute_index].span);
            pairs.entry(apply).or_default().push(compute);
        }
    }
    let function_at = |read: &crate::ReactiveRead| {
        let file = lookup.file_by_path(&read.location.path)?;
        let span = Span::new(
            u32::try_from(read.location.start_byte).ok()?,
            u32::try_from(read.location.end_byte).ok()?,
        );
        containing_ast_function(&file.ast, span)
            .map(|function| (file.path.to_string(), function.span))
    };
    // A declaration site can allocate several sources (two calls of a
    // factory). Correlate immutable getter values, not allocation templates.
    let mut values = HashMap::<SymbolId, Option<(Option<SymbolId>, bool)>>::default();
    for file in &facts.files {
        for binding in file.ast.bindings.iter().filter(|binding| binding.immutable) {
            for name in &binding.names {
                let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span).cloned()
                else {
                    continue;
                };
                let alias = (binding.shape == solid_facts::ast::BindingShape::Identifier)
                    .then_some(binding.initializer_identifier.as_ref())
                    .flatten()
                    .and_then(|name| lookup.entities().at(file.path.as_str(), name.span))
                    .cloned();
                let global = !file.ast.any_function_body_containing(name.span)
                    && !file
                        .ast
                        .classes
                        .iter()
                        .any(|class| class.span.contains(name.span))
                    && !file.ast.is_inside_module_block(name.span);
                if values
                    .insert(symbol.clone(), Some((alias, global)))
                    .is_some()
                {
                    values.insert(symbol, None);
                }
            }
        }
    }
    let value_at =
        |file: &solid_facts::FileFacts, span: Span| {
            let call = file.ast.calls.iter().find(|call| call.span == span)?;
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            if !file.ast.identifiers.iter().any(|id| {
                id.span == callee && id.role == solid_facts::ast::IdentifierRole::Reference
            }) {
                return None;
            }
            let mut symbol = lookup.entities().at(file.path.as_str(), callee)?.clone();
            let mut visiting = HashSet::default();
            loop {
                if !visiting.insert(symbol.clone()) {
                    return None;
                }
                let (alias, global) = values.get(&symbol)?.as_ref()?;
                if let Some(alias) = alias
                    && values.get(alias).is_some_and(Option::is_some)
                {
                    symbol = alias.clone();
                    continue;
                }
                return accessors.contains_key(&symbol).then_some((symbol, *global));
            }
        };
    let read_value = |read: &crate::ReactiveRead| {
        let file = lookup.file_by_path(&read.location.path)?;
        let span = Span::new(
            u32::try_from(read.location.start_byte).ok()?,
            u32::try_from(read.location.end_byte).ok()?,
        );
        if let Some((symbol, _)) = value_at(file, span) {
            return Some(symbol);
        }
        // A helper's template-local capture is not an instance identity.
        // Only a module's immutable singleton may be recovered from origin.
        let origin = read.origin.as_ref()?;
        let file = lookup.file_by_path(&origin.path)?;
        let span = Span::new(
            u32::try_from(origin.start_byte).ok()?,
            u32::try_from(origin.end_byte).ok()?,
        );
        value_at(file, span)
            .filter(|(_, global)| *global)
            .map(|(symbol, _)| symbol)
    };
    let mut tracked_sources = reads
        .iter()
        .filter(|read| {
            !read.is_uncertifiable()
                && !read.summary_attributed
                && matches!(
                    read.execution,
                    ExecutionRole::DeferredCallback | ExecutionRole::TrackedJsx
                )
        })
        .filter_map(|read| {
            function_at(read)
                .zip(read_value(read))
                .map(|(function, value)| {
                    (function, value, read.declaration.clone(), read.kind.clone())
                })
        })
        .collect::<HashSet<_>>();
    // Local read collection can omit a named compute whose invocation has no
    // reachability row. Its exact compute position and a direct call of a proven
    // accessor still establish the wording observation. Do not follow helpers,
    // nested callbacks, asynchronous bodies, or type-only accessor candidates.
    let computes = pairs
        .values()
        .flatten()
        .filter_map(Clone::clone)
        .collect::<HashSet<_>>();
    let mut guaranteed = HashMap::<(String, Span), HashSet<Span>>::default();
    for file in &facts.files {
        for call in file
            .ast
            .calls
            .iter()
            .filter(|call| !call.construct && call.arguments.is_empty())
        {
            let Some(function) = containing_ast_function(&file.ast, call.span) else {
                continue;
            };
            if !computes.contains(&(file.path.to_string(), function.span))
                || function.r#async
                || function.generator
            {
                continue;
            }
            if file.ast.any_jsx_containing(call.span) {
                continue;
            }
            let Some(symbol) = lookup.callee_symbol(file, call.callee) else {
                continue;
            };
            let Some((_, declaration)) = accessors.get(symbol) else {
                continue;
            };
            let declaration = origins.get(symbol).map_or(declaration, |origin| &origin.2);
            let Some((value, _)) = value_at(file, call.span) else {
                continue;
            };
            let source = (
                (file.path.to_string(), function.span),
                value,
                declaration.clone(),
                "accessor".into(),
            );
            if tracked_sources.contains(&source) {
                continue;
            }
            let calls = guaranteed
                .entry((file.path.to_string(), function.span))
                .or_insert_with(|| {
                    solid_facts::ast::synchronously_unconditional_calls(
                        std::path::Path::new(file.path.as_str()),
                        &file.source,
                        function.span,
                        &[],
                    )
                    .unwrap_or_default()
                    .into_iter()
                    .collect()
                });
            if !calls.contains(&call.span) {
                continue;
            }
            tracked_sources.insert(source);
        }
    }
    reads
        .iter()
        .filter(|read| {
            read.execution == ExecutionRole::EffectApply
                && read.kind.as_ref() == "accessor"
                && !read.is_uncertifiable()
                && !read.summary_attributed
        })
        .filter(|read| {
            function_at(read)
                .and_then(|apply| pairs.get(&apply))
                .is_some_and(|computes| {
                    // Several uses of a named apply need the same answer at every pair.
                    computes.iter().all(|compute| {
                        compute.as_ref().is_some_and(|compute| {
                            read_value(read).is_some_and(|value| {
                                tracked_sources.contains(&(
                                    compute.clone(),
                                    value,
                                    read.declaration.clone(),
                                    read.kind.clone(),
                                ))
                            })
                        })
                    })
                })
        })
        .map(|read| read.location.clone())
        .collect()
}

/// The effect primitive whose apply callback `span` runs in, mirroring the two
/// ways [`semantic_execution_role_within`] reaches `EffectApply`: a named
/// function passed at an apply position, then the innermost inline apply
/// argument containing the span (inline wrappers such as `batch` inside it
/// inherit that apply's role).
pub(super) fn effect_apply_primitive(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<Primitive> {
    let roles = lookup.named_callback_roles(file);
    if let Some(callback) = file
        .ast
        .functions_body_containing(span)
        .find(|function| roles.get(function.span).admitted)
        && containing_ast_function(&file.ast, span).is_some_and(|owner| owner.span == callback.span)
        && roles.get(callback.span).effect_apply
    {
        return roles.get(callback.span).effect_apply_primitive.single();
    }
    file.ast
        .arguments_containing(span)
        .filter_map(|(call, index)| {
            let primitive = lookup.primitive_at_call(file, call.span)?;
            (effect_apply_argument(lookup.dialect, primitive, call.arguments.len()) == Some(index))
                .then(|| (call.arguments[index].span, primitive))
        })
        .min_by_key(|(argument, _)| argument.end - argument.start)
        .map(|(_, primitive)| primitive)
}

pub(super) fn format_read_context(context: &str, in_conditional_test: bool) -> String {
    if in_conditional_test {
        if context.is_empty() {
            "while evaluating a condition".into()
        } else {
            format!("{context} while evaluating a condition")
        }
    } else {
        context.into()
    }
}

pub(crate) fn async_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> ExecutionRole {
    if execution == ExecutionRole::DeferredCallback && file.ast.any_jsx_containing(span) {
        ExecutionRole::TrackedJsx
    } else {
        execution
    }
}

pub(crate) fn pending_accessor_probe(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast.arguments_containing(span).any(|(call, index)| {
        direct_callback_contains(file, call.arguments[index].span, span)
            && lookup
                .primitive_at_call(file, call.span)
                .is_some_and(|primitive| {
                    lookup
                        .dialect
                        .callback_handles_pending_accessor_read(primitive, index)
                })
    })
}

pub(crate) fn allowed_callback_spans(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    let dialect = lookup.dialect;
    let primitives = lookup.primitives(file);
    let mut spans = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let Some(primitive) = known_primitive(&primitives.calls[call_index]) else {
            continue;
        };
        for index in deferred_callback_positions(dialect, primitive, call.arguments.len()) {
            if callback_execution_at_call(file, call, primitive, index, lookup).is_none() {
                continue;
            }
            if let Some(argument) = call.arguments.get(index) {
                spans.push(argument.span);
            }
        }
    }
    spans
}
