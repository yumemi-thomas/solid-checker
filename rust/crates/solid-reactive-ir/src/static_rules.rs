//! File-local static rules the pipeline runs right after source discovery:
//! checks that need discovered sources and TypeScript facts, but none of the
//! reactive stages behind them.

use crate::execution_role::{allowed_callback_spans, semantic_execution_role};
use crate::identity::SymbolId;
use crate::owners::{
    component_binding_name, component_props_parameter_fix, containing_ast_function,
    enclosing_function_label, function_binding_name,
};
use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::symbols::async_symbol_root;
use crate::{
    ReactiveSourceKind, StaticDefect, StaticDefectKind, known_primitive, location, primitive_name,
};
use solid_facts::core::Span;
use typefacts::Location;

/// The static-prepass stage: every prepass rule, in their pipeline order.
pub(crate) fn static_prepass(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    execution_map_incomplete(ctx, draft);
    component_props_destructure(ctx, draft);
    prefer_component_syntax(ctx, draft);
    implicit_draggable_boolean(ctx, draft);
    valid_jsx_nesting(ctx, draft);
    reactive_read_after_await(ctx, draft);
}

/// SC8018: a local JSX-returning function called as an ordinary expression
/// from JSX. Symbol identity rejects shadowed same-spelled functions; the
/// direct-return check rejects ordinary value helpers.
fn prefer_component_syntax(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for file in &ctx.facts.files {
        for function in &file.ast.functions {
            let Some(name) = function_binding_name(file, function).or(function.name.as_ref())
            else {
                continue;
            };
            let name_text = file.source_text(name.span).unwrap_or_default();
            if !name_text.starts_with(|character: char| character.is_ascii_lowercase())
                || !function_directly_returns_jsx(file, function)
            {
                continue;
            }
            for (caller_file, callee) in ctx
                .semantic_lookup
                .function_call_sites(file.path.as_str(), function.span)
            {
                let Some(call) = caller_file
                    .ast
                    .calls
                    .iter()
                    .find(|candidate| candidate.callee == callee)
                else {
                    continue;
                };
                if !caller_file.ast.any_jsx_containing(call.span)
                    || (caller_file.path == file.path && function.span.contains(call.span))
                {
                    continue;
                }
                draft.push_defect(
                    "prefer-component-syntax",
                    StaticDefect {
                        kind: StaticDefectKind::PreferComponentSyntax {
                            name: name_text.to_owned(),
                        },
                        location: location(caller_file.path.shared(), call.callee),
                        analysis_context: enclosing_function_label(caller_file, call.span),
                        fixes: vec![],
                    },
                );
            }
        }
    }
}

fn function_directly_returns_jsx(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    function
        .expression_return
        .iter()
        .chain(file.ast.returns.iter().filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        }))
        .filter_map(|returned| returned.argument)
        .any(|argument| {
            file.ast.jsx_within(argument).any(|element| {
                containing_ast_function(&file.ast, element.span)
                    .is_some_and(|owner| owner.span == function.span)
            }) || file.ast.jsx_fragments.iter().any(|fragment| {
                argument.contains(*fragment)
                    && containing_ast_function(&file.ast, *fragment)
                        .is_some_and(|owner| owner.span == function.span)
            })
        })
}

/// SC8019: `draggable` is enumerated, not boolean. JSX shorthand serializes
/// an empty value, whose HTML state is `auto`, not `true`.
fn implicit_draggable_boolean(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for file in &ctx.facts.files {
        for element in &file.ast.jsx_elements {
            let element_name = file.source_text(element.name.span).unwrap_or_default();
            if !element_name.starts_with(|character: char| character.is_ascii_lowercase()) {
                continue;
            }
            for attribute in &element.attributes {
                if attribute.namespace.is_some()
                    || attribute.value_kind != solid_facts::ast::JsxAttributeValueKind::Boolean
                    || file.source_text(attribute.local_name) != Some("draggable")
                {
                    continue;
                }
                draft.push_defect(
                    "no-implicit-draggable",
                    StaticDefect {
                        kind: StaticDefectKind::ImplicitDraggableBoolean,
                        location: location(file.path.shared(), attribute.span),
                        analysis_context: element_name.to_owned(),
                        fixes: vec![],
                    },
                );
            }
        }
    }
}

/// SC8020: nesting for which the HTML parser changes the authored tree by
/// inserting or implicitly closing elements. The rule deliberately targets
/// SSR/hydration mismatches instead of trying to encode every content model.
fn valid_jsx_nesting(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for file in &ctx.facts.files {
        for child in &file.ast.jsx_elements {
            let child_name = file.source_text(child.name.span).unwrap_or_default();
            if !is_intrinsic_jsx_name(child_name) {
                continue;
            }

            let mut ancestors = file
                .ast
                .jsx_elements
                .iter()
                .filter(|candidate| {
                    candidate.span != child.span && candidate.span.contains(child.span)
                })
                .collect::<Vec<_>>();
            ancestors.sort_by_key(|candidate| candidate.span.end - candidate.span.start);

            // A component controls what DOM it returns. Propagating a native
            // ancestor through that opaque boundary would be a guess.
            let intrinsic_ancestors = ancestors
                .into_iter()
                .take_while(|candidate| {
                    is_intrinsic_jsx_name(file.source_text(candidate.name.span).unwrap_or_default())
                })
                .collect::<Vec<_>>();
            let Some(parent) = intrinsic_ancestors.first() else {
                continue;
            };
            if intrinsic_ancestors
                .iter()
                .map(|ancestor| file.source_text(ancestor.name.span).unwrap_or_default())
                .find(|name| matches!(*name, "svg" | "foreignObject"))
                == Some("svg")
            {
                continue;
            }
            let parent_name = file.source_text(parent.name.span).unwrap_or_default();
            let invalid = if html_parser_accepts_child(parent_name, child_name) {
                invalid_html_ancestor(file, child_name, &intrinsic_ancestors)
                    .map(|name| (name, true))
            } else {
                Some((parent_name, false))
            };
            let Some((invalid_parent, ancestor)) = invalid else {
                continue;
            };

            draft.push_defect(
                "valid-jsx-nesting",
                StaticDefect {
                    kind: StaticDefectKind::InvalidJsxNesting {
                        parent: invalid_parent.to_owned(),
                        child: child_name.to_owned(),
                        ancestor,
                    },
                    location: location(file.path.shared(), child.name.span),
                    analysis_context: format!("<{invalid_parent}>"),
                    fixes: vec![],
                },
            );
        }
    }
}

fn is_intrinsic_jsx_name(name: &str) -> bool {
    name.starts_with(|character: char| character.is_ascii_lowercase())
}

fn html_parser_accepts_child(parent: &str, child: &str) -> bool {
    match parent {
        "select" => matches!(child, "hr" | "option" | "optgroup" | "script" | "template"),
        "optgroup" => child == "option",
        "option" => false,
        "tr" => matches!(child, "th" | "td" | "style" | "script" | "template"),
        "tbody" | "thead" | "tfoot" => {
            matches!(child, "tr" | "style" | "script" | "template")
        }
        "colgroup" => matches!(child, "col" | "template"),
        "table" => matches!(
            child,
            "caption" | "colgroup" | "tbody" | "tfoot" | "thead" | "style" | "script" | "template"
        ),
        "head" => matches!(
            child,
            "base"
                | "basefont"
                | "bgsound"
                | "link"
                | "meta"
                | "title"
                | "noscript"
                | "noframes"
                | "style"
                | "script"
                | "template"
        ),
        "html" => matches!(child, "head" | "body" | "frameset"),
        "frameset" => child == "frame",
        _ if is_heading(child) => !is_heading(parent),
        _ if matches!(child, "rp" | "rt") => !matches!(
            parent,
            "dd" | "dt" | "li" | "option" | "optgroup" | "p" | "rp" | "rt"
        ),
        _ if matches!(
            child,
            "caption"
                | "col"
                | "colgroup"
                | "frameset"
                | "frame"
                | "tbody"
                | "td"
                | "tfoot"
                | "th"
                | "thead"
                | "tr"
                | "head"
                | "html"
                | "body"
        ) =>
        {
            false
        }
        _ => true,
    }
}

fn is_heading(name: &str) -> bool {
    matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}

fn invalid_html_ancestor<'a>(
    file: &'a solid_facts::FileFacts,
    child: &str,
    ancestors: &[&'a solid_facts::ast::JsxElementFact],
) -> Option<&'a str> {
    let names = ancestors
        .iter()
        .map(|ancestor| file.source_text(ancestor.name.span).unwrap_or_default());
    match child {
        "address" | "article" | "aside" | "blockquote" | "center" | "details" | "dialog"
        | "dir" | "div" | "dl" | "fieldset" | "figcaption" | "figure" | "footer" | "header"
        | "hgroup" | "main" | "menu" | "nav" | "ol" | "p" | "section" | "summary" | "ul"
        | "pre" | "listing" | "table" | "hr" | "xmp" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            names.into_iter().find(|name| *name == "p")
        }
        "form" => names.into_iter().find(|name| matches!(*name, "form" | "p")),
        "li" => names.into_iter().find(|name| *name == "li"),
        "dd" | "dt" => names.into_iter().find(|name| matches!(*name, "dd" | "dt")),
        "button" => names.into_iter().find(|name| *name == "button"),
        "a" => names.into_iter().find(|name| *name == "a"),
        "nobr" => names.into_iter().find(|name| *name == "nobr"),
        _ => None,
    }
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

/// SC1003: a reactive object destructured outside tracking. Component
/// parameter patterns are necessarily setup-time reads; body bindings are
/// classified through the same semantic execution-role engine as direct reads.
fn component_props_destructure(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for file in &ctx.facts.files {
        for function in &file.ast.functions {
            // Solid invokes components with one props object. Reject the
            // overwhelmingly common non-destructured functions before the
            // whole-project component-identity lookup.
            if function.parameters.len() <= 1
                && let Some(parameter) = function
                    .parameters
                    .first()
                    .filter(|parameter| parameter.shape == solid_facts::ast::BindingShape::Object)
                && ctx.semantic_lookup.function_is_component(file, function)
            {
                let location = location(file.path.shared(), parameter.pattern);
                draft.push_defect(
                    "component-props-destructure",
                    StaticDefect {
                        kind: StaticDefectKind::ReactiveObjectDestructure {
                            source: "props".into(),
                            component_props: true,
                        },
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
        if !file
            .ast
            .bindings
            .iter()
            .any(|binding| binding.shape == solid_facts::ast::BindingShape::Object)
        {
            continue;
        }
        let allowed = allowed_callback_spans(file, ctx.semantic_lookup);
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
            let reactive_object = props || binding_initializes_reactive_store(ctx, file, binding);
            if reactive_object
                && semantic_execution_role(
                    file,
                    binding.pattern,
                    &allowed,
                    ctx.entities,
                    ctx.symbol_names,
                    ctx.semantic_lookup,
                ) != crate::ExecutionRole::TrackedJsx
            {
                let location = location(file.path.shared(), binding.pattern);
                let source = binding
                    .initializer
                    .and_then(|span| file.source_text(span))
                    .unwrap_or("reactive object")
                    .to_owned();
                draft.push_defect(
                    "component-props-destructure",
                    StaticDefect {
                        kind: StaticDefectKind::ReactiveObjectDestructure {
                            source,
                            component_props: props,
                        },
                        location,
                        analysis_context: enclosing_function_label(file, binding.pattern),
                        fixes: vec![],
                    },
                );
            }
        }
    }
}

fn binding_initializes_reactive_store(
    ctx: &AnalysisContext<'_>,
    file: &solid_facts::FileFacts,
    binding: &solid_facts::ast::BindingFact,
) -> bool {
    if binding
        .initializer_identifier
        .as_ref()
        .and_then(|identifier| {
            ctx.entities
                .get(&location(file.path.shared(), identifier.span))
        })
        .is_some_and(|symbol| ctx.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store))
    {
        return true;
    }
    let Some(call) = binding
        .call_initializer
        .and_then(|initializer| file.ast.call_at(initializer))
    else {
        return false;
    };
    let contracted_store = ctx
        .entities
        .get(&location(file.path.shared(), call.callee))
        .and_then(|symbol| ctx.contracted.get(symbol))
        .and_then(|binding| binding.summary.returns.as_ref())
        .is_some_and(|returned| returned.kind == "store-path");
    contracted_store
        || known_primitive(&primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            ctx.entities,
            ctx.symbol_names,
            ctx.dialect,
        ))
        .is_some_and(|primitive| ctx.dialect.returns_store(primitive))
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
            if !ctx.semantic_lookup.function_is_component(file, function) {
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
