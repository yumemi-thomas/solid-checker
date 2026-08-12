//! File-local static rules the pipeline runs right after source discovery:
//! checks that need discovered sources and TypeScript facts, but none of the
//! reactive stages behind them.

use crate::identity::SymbolId;
use crate::owners::{
    component_binding_name, component_props_parameter_fix, containing_ast_function,
    enclosing_function_label, function_binding_name,
};
use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::symbols::async_symbol_root;
use crate::{StaticDefect, StaticDefectKind, location, primitive_name};
use solid_facts::core::Span;
use typefacts::Location;

/// The static-prepass stage: every prepass rule, in their pipeline order.
pub(crate) fn static_prepass(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    execution_map_incomplete(ctx, draft);
    component_props_destructure(ctx, draft);
    reactive_read_after_await(ctx, draft);
}

/// SC9004: JSX expressions the compiler left without an execution role.
fn execution_map_incomplete(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for file in &ctx.facts.files {
        for span in file.compiler.uncovered_jsx_expressions() {
            draft.push_defect(
                "execution-map-incomplete",
                StaticDefect {
                    kind: StaticDefectKind::ExecutionMapIncomplete,
                    location: location(file.path.shared(), span),
                    analysis_context: String::new(),
                    fixes: vec![],
                },
            );
        }
    }
}

/// SC1003: props objects destructured at component setup, from a component's
/// own parameter list or from an object binding over a props source.
fn component_props_destructure(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for file in &ctx.facts.files {
        for function in &file.ast.functions {
            if component_binding_name(file, function)
                .and_then(|name| {
                    file.source_text(name.span)
                        .unwrap_or_default()
                        .chars()
                        .next()
                })
                .is_some_and(char::is_uppercase)
                // Solid invokes components with one props object. A function
                // requiring additional positional parameters is not a Solid
                // component merely because its local name is capitalized.
                && function.parameters.len() <= 1
                && let Some(parameter) = function
                    .parameters
                    .first()
                    .filter(|parameter| parameter.shape == solid_facts::ast::BindingShape::Object)
            {
                let location = location(file.path.shared(), parameter.pattern);
                draft.push_defect(
                    "component-props-destructure",
                    StaticDefect {
                        kind: StaticDefectKind::ComponentPropsDestructure,
                        location,
                        analysis_context: component_binding_name(file, function)
                            .map_or_else(String::new, |name| {
                                file.source_text(name.span).unwrap_or_default().to_owned()
                            }),
                        fixes: component_props_parameter_fix(
                            ctx.facts,
                            file,
                            function,
                            parameter,
                            ctx.entities,
                        )
                        .into_iter()
                        .collect(),
                    },
                );
            }
        }
        for binding in &file.ast.bindings {
            if binding.shape != solid_facts::ast::BindingShape::Object {
                continue;
            }
            let props = binding
                .initializer_identifier
                .as_ref()
                .and_then(|identifier| {
                    ctx.entities
                        .get(&location(file.path.shared(), identifier.span))
                })
                .is_some_and(|symbol| ctx.prop_sources.contains_key(symbol));
            if props {
                let location = location(file.path.shared(), binding.pattern);
                draft.push_defect(
                    "component-props-destructure",
                    StaticDefect {
                        kind: StaticDefectKind::ComponentPropsDestructure,
                        location,
                        analysis_context: enclosing_function_label(file, binding.pattern),
                        fixes: vec![],
                    },
                );
            }
        }
    }
}

/// SC1002: reactive accessors read after an await inside a tracked async
/// computation, where dependency tracking has already ended.
fn reactive_read_after_await(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for typescript_file in ctx.facts.typescript.files() {
        for function in typescript_file.async_functions.iter() {
            for call in &function.calls_after_await {
                let Some(symbol) = ctx.entities.get(call) else {
                    continue;
                };
                let Some((name, _)) = ctx.accessors.get(symbol) else {
                    continue;
                };
                let ast_call = ctx
                    .facts
                    .files
                    .iter()
                    .find(|file| *file.path.as_str() == *call.path)
                    .and_then(|file| {
                        file.ast
                            .calls
                            .iter()
                            .find(|candidate| {
                                u64::from(candidate.callee.start) == call.start_byte
                                    && u64::from(candidate.callee.end) == call.end_byte
                            })
                            .map(|candidate| (file, candidate))
                    });
                let display = ast_call
                    .and_then(|(file, candidate)| candidate.static_callee(&file.source))
                    .unwrap_or(name);
                let diagnostic_location = Location {
                    path: call.path.clone(),
                    start_byte: call.start_byte,
                    end_byte: call.end_byte.saturating_add(1),
                };
                let function_symbol = async_symbol_root(
                    ctx.aliases
                        .get(function.symbol.as_ref())
                        .map_or(function.symbol.as_ref(), SymbolId::as_str),
                    &ctx.facts.typescript,
                );
                let Some(analysis_context) = ctx.facts.files.iter().find_map(|file| {
                    file.ast.calls.iter().find_map(|candidate| {
                        let argument = candidate.arguments.first()?;
                        let lexical = *file.path.as_str() == *function.expression.path
                            && argument.span.contains(Span::new(
                                u32::try_from(function.expression.start_byte).ok()?,
                                u32::try_from(function.expression.end_byte).ok()?,
                            ));
                        let semantic = ctx
                            .entities
                            .get(&location(file.path.shared(), argument.span))
                            .is_some_and(|symbol| {
                                async_symbol_root(symbol, &ctx.facts.typescript) == function_symbol
                            });
                        if !lexical && !semantic {
                            return None;
                        }
                        let primitive = primitive_name(
                            file.path.as_str(),
                            candidate.callee,
                            candidate.static_callee(&file.source),
                            ctx.entities,
                            ctx.symbol_names,
                            ctx.dialect,
                        )?;
                        // A tracked callback is what makes this a computation
                        // whose reads matter after an await. The list this
                        // replaced was 2.0's eight; under 1.x three of them
                        // resolve to nothing and `createComputed` was absent.
                        primitive
                            .primitive()
                            .is_some_and(|resolved| {
                                ctx.dialect
                                    .callback_semantics_at(resolved, 0, candidate.arguments.len())
                                    .tracks_reads
                            })
                            .then(|| format!("{primitive} async computation"))
                    })
                }) else {
                    continue;
                };
                draft.push_defect(
                    "reactive-read-after-await",
                    StaticDefect {
                        kind: StaticDefectKind::ReactiveReadAfterAwait {
                            accessor: display.to_string(),
                        },
                        location: diagnostic_location,
                        analysis_context,
                        fixes: vec![],
                    },
                );
            }
        }
    }
}

/// SC1004: a component whose return value hinges on a reactive condition,
/// evaluated once at setup and never again. Runs after the read tables are
/// merged: "reactive condition" is answered by the draft's reads.
pub(crate) fn component_returns_conditionally(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for file in &ctx.facts.files {
        for function in &file.ast.functions {
            let Some(name) = function_binding_name(file, function).or(function.name.as_ref())
            else {
                continue;
            };
            if !file
                .source_text(name.span)
                .unwrap_or_default()
                .chars()
                .next()
                .is_some_and(char::is_uppercase)
            {
                continue;
            }
            let mut direct_returns = file
                .ast
                .returns
                .iter()
                .filter(|returned| {
                    function.body.contains(returned.span)
                        && containing_ast_function(&file.ast, returned.span)
                            .is_some_and(|owner| owner.span == function.span)
                })
                .collect::<Vec<_>>();
            if let Some(returned) = &function.expression_return {
                direct_returns.push(returned);
            }
            for test in file.ast.conditional_tests.iter().filter(|test| {
                function.body.contains(**test)
                    && containing_ast_function(&file.ast, **test)
                        .is_some_and(|owner| owner.span == function.span)
            }) {
                let reactive = draft.reads.iter().any(|read| {
                    read.location.path == file.path.as_str().into()
                        && u64::from(test.start) <= read.location.start_byte
                        && read.location.end_byte <= u64::from(test.end)
                });
                let conditional_return = direct_returns.iter().any(|returned| {
                    returned.control_tests.contains(test)
                        || (returned.conditional
                            && returned
                                .argument
                                .is_some_and(|argument| argument.contains(*test)))
                });
                if reactive && conditional_return {
                    let location = location(file.path.shared(), *test);
                    draft.push_defect(
                        "component-returns-conditionally",
                        StaticDefect {
                            kind: StaticDefectKind::ComponentReturnsConditionally,
                            location,
                            analysis_context: file
                                .source_text(name.span)
                                .unwrap_or_default()
                                .to_owned(),
                            fixes: vec![],
                        },
                    );
                }
            }
        }
    }
}
