//! File-local static rules the pipeline runs right after source discovery:
//! checks that need discovered sources and TypeScript facts, but none of the
//! reactive stages behind them.

use crate::cleanup::callback_argument_literal;
use crate::execution_role::{allowed_callback_spans, semantic_execution_role};
use crate::identity::SymbolId;
use crate::owners::{
    component_binding_name, component_props_parameter_fix, containing_ast_function,
    enclosing_function_label, function_binding_name,
};
use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::runtime_semantics::is_proven_array_filter;
use crate::symbols::async_symbol_root;
use crate::{
    DraggableSpelling, ReactiveSourceKind, StaticDefect, StaticDefectKind, known_primitive,
    location, primitive_name,
};
use solid_dialect::Version;
use solid_facts::core::Span;
use std::collections::HashSet;
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
                        uncertain: false,
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
///
/// Solid 2.0 removes `draggable={false}`, which selects `auto` and silently
/// re-enables dragging on draggable-by-default elements (`img`, and `a` with
/// an `href`). Its published JSX types already reject the shorthand and
/// literal `true`, so those spellings belong to TypeScript and are excluded.
/// Solid 1.x stringifies boolean literals and only its type-correct shorthand
/// remains this rule's concern.
fn implicit_draggable_boolean(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for file in &ctx.facts.files {
        for element in &file.ast.jsx_elements {
            let element_name = file.source_text(element.name.span).unwrap_or_default();
            if !element_name.starts_with(|character: char| character.is_ascii_lowercase()) {
                continue;
            }
            for attribute in &element.attributes {
                if attribute.namespace.is_some()
                    || file.source_text(attribute.local_name) != Some("draggable")
                {
                    continue;
                }
                let spelling = match attribute.value_kind {
                    solid_facts::ast::JsxAttributeValueKind::Boolean => {
                        if ctx.dialect.version() == Version::V1 {
                            DraggableSpelling::Shorthand
                        } else {
                            continue;
                        }
                    }
                    solid_facts::ast::JsxAttributeValueKind::Expression => {
                        // The boolean-literal fact mirrors the attribute name
                        // span, carrying the literal value; a non-literal
                        // expression has no entry and proves nothing.
                        let literal = element
                            .boolean_properties
                            .iter()
                            .find(|property| property.name == attribute.local_name);
                        match literal {
                            Some(property)
                                if !property.value
                                    && ctx.dialect.false_attribute_value_removes_attribute() =>
                            {
                                DraggableSpelling::LiteralFalseOnDraggableDefault
                            }
                            _ => continue,
                        }
                    }
                    _ => continue,
                };
                let draggable_default = match spelling {
                    DraggableSpelling::Shorthand => DraggableDefault::Yes,
                    DraggableSpelling::LiteralFalseOnDraggableDefault => {
                        element_defaults_to_draggable(file, element, element_name)
                    }
                };
                if draggable_default == DraggableDefault::No {
                    continue;
                }
                draft.push_defect(
                    "no-implicit-draggable",
                    StaticDefect {
                        kind: StaticDefectKind::ImplicitDraggableBoolean { spelling },
                        location: location(file.path.shared(), attribute.span),
                        analysis_context: if draggable_default == DraggableDefault::Uncertain {
                            "draggable-default-uncertain".into()
                        } else {
                            element_name.to_owned()
                        },
                        fixes: vec![],
                        uncertain: draggable_default == DraggableDefault::Uncertain,
                    },
                );
            }
        }
    }
}

/// The elements whose `draggable` *auto* state is draggable — WHATWG HTML
/// "the draggable attribute": `img` elements, and `a` elements with an
/// `href` attribute.
///
/// The last source-order write owns `href`. A static string/bare attribute
/// proves presence; a dynamic expression or later spread can either preserve
/// or remove it and therefore leaves an explicit proof obligation. Absence of
/// every direct attribute and spread proves that the anchor is not draggable
/// by default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DraggableDefault {
    No,
    Yes,
    Uncertain,
}

fn element_defaults_to_draggable(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    element_name: &str,
) -> DraggableDefault {
    if element_name == "img" {
        return DraggableDefault::Yes;
    }
    if element_name != "a" {
        return DraggableDefault::No;
    }
    let direct = element
        .attributes
        .iter()
        .filter(|attribute| {
            attribute.namespace.is_none() && file.source_text(attribute.local_name) == Some("href")
        })
        .max_by_key(|attribute| attribute.span.start);
    let spread = element
        .spreads
        .iter()
        .max_by_key(|spread| spread.span.start);
    if spread.is_some_and(|spread| {
        direct.is_none_or(|attribute| spread.span.start > attribute.span.start)
    }) {
        return DraggableDefault::Uncertain;
    }
    match direct.map(|attribute| attribute.value_kind) {
        Some(
            solid_facts::ast::JsxAttributeValueKind::String
            | solid_facts::ast::JsxAttributeValueKind::Boolean,
        ) => DraggableDefault::Yes,
        Some(_) => DraggableDefault::Uncertain,
        None => DraggableDefault::No,
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
                    uncertain: false,
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

/// The WHATWG "in body" start-tag rules that implicitly close an *ancestor*
/// element, with the spec's scope boundaries applied: each walk inspects the
/// stack of open elements innermost-first and gives up at the same elements
/// the parser's scope check or implied-end-tag loop stops at, so only
/// nestings the parser would actually rewrite are reported.
/// <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody>
fn invalid_html_ancestor<'a>(
    file: &'a solid_facts::FileFacts,
    child: &str,
    ancestors: &[&'a solid_facts::ast::JsxElementFact],
) -> Option<&'a str> {
    let names = ancestors
        .iter()
        .map(|ancestor| file.source_text(ancestor.name.span).unwrap_or_default());
    let names = names.collect::<Vec<_>>();
    match child {
        // WHATWG "in body" p-closing start tags: the listed tags, plus the
        // separate rules for h1-h6, pre/listing, form, table, hr, and xmp,
        // all begin with "if the stack of open elements has a p element in
        // *button scope*, close a p element". `search` is in the spec's
        // list; the implied-end-tag walk stops at button-scope boundaries.
        "address" | "article" | "aside" | "blockquote" | "center" | "details" | "dialog"
        | "dir" | "div" | "dl" | "fieldset" | "figcaption" | "figure" | "footer" | "header"
        | "hgroup" | "main" | "menu" | "nav" | "ol" | "p" | "search" | "section" | "summary"
        | "ul" | "pre" | "listing" | "table" | "hr" | "xmp" | "h1" | "h2" | "h3" | "h4" | "h5"
        | "h6" => scan_open_elements(&names, |name| name == "p", is_button_scope_boundary),
        // The form element pointer ignores intervening elements entirely —
        // no scope walk — except that it is not consulted at all when a
        // template element is on the stack. Form is also a p-closer (button
        // scope), checked as its own walk.
        "form" => scan_open_elements(&names, |name| name == "form", |name| name == "template")
            .or_else(|| scan_open_elements(&names, |name| name == "p", is_button_scope_boundary)),
        // The li (and dd/dt) start-tag loops walk the stack from the current
        // node: a matching item generates implied end tags; any *special*
        // element other than address, div, or p breaks the loop first.
        "li" => scan_open_elements(&names, |name| name == "li", is_implied_end_tag_boundary),
        "dd" | "dt" => scan_open_elements(
            &names,
            |name| matches!(name, "dd" | "dt"),
            is_implied_end_tag_boundary,
        ),
        // button and nobr start tags use the plain "in scope" check; the a
        // start tag consults the list of active formatting elements up to
        // the last marker, and every marker-inserting element is in the
        // default scope list, so the same stop set is a sound approximation.
        "button" => scan_open_elements(&names, |name| name == "button", is_default_scope_boundary),
        "a" => scan_open_elements(&names, |name| name == "a", is_default_scope_boundary),
        "nobr" => scan_open_elements(&names, |name| name == "nobr", is_default_scope_boundary),
        _ => None,
    }
}

/// Walks the intrinsic ancestors innermost-first — the parser's stack of open
/// elements from the current node — reporting the first `target` reached
/// before any `boundary`. The target test runs first, matching the spec's
/// loops, where "node is an li element" precedes the special-category break.
fn scan_open_elements<'a>(
    names: &[&'a str],
    target: impl Fn(&str) -> bool,
    boundary: impl Fn(&str) -> bool,
) -> Option<&'a str> {
    for name in names {
        if target(name) {
            return Some(name);
        }
        if boundary(name) {
            return None;
        }
    }
    None
}

/// The default "has an element in scope" list:
/// <https://html.spec.whatwg.org/multipage/parsing.html#has-an-element-in-scope>
/// — applet, caption, html, table, td, th, marquee, object, template, plus
/// the MathML text integration points (mi, mo, mn, ms, mtext,
/// annotation-xml) and the SVG HTML integration points (foreignObject, desc,
/// title).
fn is_default_scope_boundary(name: &str) -> bool {
    matches!(
        name,
        "applet"
            | "caption"
            | "html"
            | "table"
            | "td"
            | "th"
            | "marquee"
            | "object"
            | "template"
            | "mi"
            | "mo"
            | "mn"
            | "ms"
            | "mtext"
            | "annotation-xml"
            | "foreignObject"
            | "desc"
            | "title"
    )
}

/// The "has a p element in button scope" list: the default scope list plus
/// `button`. A `<div>` inside `<p><button>` does not close the paragraph —
/// the button terminates the scope walk and the parser preserves the tree.
fn is_button_scope_boundary(name: &str) -> bool {
    name == "button" || is_default_scope_boundary(name)
}

/// The WHATWG *special* category
/// (<https://html.spec.whatwg.org/multipage/parsing.html#special>) minus
/// `address`, `div`, and `p`, which the li/dd/dt start-tag loops exempt:
/// "if node is in the special category, but is not an address, div, or p
/// element, then jump to the step labeled done". This is what keeps nested
/// lists legal — the inner `<ul>`/`<ol>`/`<dl>` stops the walk before the
/// outer `li`/`dd`/`dt` is reached, and the parser preserves
/// `<ul><li><ul><li>…` verbatim.
fn is_implied_end_tag_boundary(name: &str) -> bool {
    matches!(
        name,
        "applet"
            | "area"
            | "article"
            | "aside"
            | "base"
            | "basefont"
            | "bgsound"
            | "blockquote"
            | "body"
            | "br"
            | "button"
            | "caption"
            | "center"
            | "col"
            | "colgroup"
            | "dd"
            | "details"
            | "dir"
            | "dl"
            | "dt"
            | "embed"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "frame"
            | "frameset"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hgroup"
            | "hr"
            | "html"
            | "iframe"
            | "img"
            | "input"
            | "keygen"
            | "li"
            | "link"
            | "listing"
            | "main"
            | "marquee"
            | "menu"
            | "meta"
            | "nav"
            | "noembed"
            | "noframes"
            | "noscript"
            | "object"
            | "ol"
            | "param"
            | "plaintext"
            | "pre"
            | "script"
            | "search"
            | "section"
            | "select"
            | "source"
            | "style"
            | "summary"
            | "table"
            | "tbody"
            | "td"
            | "template"
            | "textarea"
            | "tfoot"
            | "th"
            | "thead"
            | "tr"
            | "track"
            | "ul"
            | "wbr"
            | "xmp"
            // MathML text integration points and SVG HTML integration points
            // are special too.
            | "mi"
            | "mo"
            | "mn"
            | "ms"
            | "mtext"
            | "annotation-xml"
            | "foreignObject"
            | "desc"
            | "title"
    )
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
                    uncertain: false,
                },
            );
        }
    }
}

/// SC1003: a reactive object destructured outside tracking. Component
/// parameter patterns are necessarily setup-time reads; body bindings are
/// classified through the same semantic execution-role engine as direct reads.
///
/// Two precision gates:
/// - **Roles.** Destructuring inside an event handler, a deferred/leaf
///   callback (`onSettled`, action bodies), an effect's apply callback, an
///   `untrack` callback, or a directive application reads fresh values at
///   call time — legal at runtime — so only setup-time contexts (component
///   body, module scope, unclassified spans) are flagged.
/// - **Caller-proven props.** Under a dialect requiring caller proof, a
///   destructure that binds only proven-static props is not a reactive read
///   at all; an unprovable one is reported as a proof obligation.
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
                let uncertain = match destructured_props_use(ctx, parameter, &location) {
                    crate::source_discovery::PropUse::Static => continue,
                    crate::source_discovery::PropUse::Reactive => false,
                    crate::source_discovery::PropUse::Unknown => true,
                };
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
                        uncertain,
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
            let prop_declaration = binding
                .initializer_identifier
                .as_ref()
                .and_then(|identifier| {
                    ctx.entities
                        .get(&location(file.path.shared(), identifier.span))
                })
                .and_then(|symbol| ctx.prop_sources.get(symbol))
                .map(|(_, declaration)| declaration.clone());
            let props = prop_declaration.is_some();
            let reactive_object = props || binding_initializes_reactive_store(ctx, file, binding);
            if !reactive_object {
                continue;
            }
            let role = semantic_execution_role(
                file,
                binding.pattern,
                &allowed,
                ctx.entities,
                ctx.symbol_names,
                ctx.semantic_lookup,
            );
            // Fresh-at-call-time contexts: tracked scopes re-run and
            // re-subscribe; event handlers, deferred/leaf callbacks, effect
            // apply, untrack, and directive application read current values
            // when they execute. None of those misbehaves at runtime.
            if matches!(
                role,
                crate::ExecutionRole::TrackedJsx
                    | crate::ExecutionRole::EventCallback
                    | crate::ExecutionRole::DeferredCallback
                    | crate::ExecutionRole::UntrackedCallback
                    | crate::ExecutionRole::EffectApply
                    | crate::ExecutionRole::DirectiveApply
            ) {
                continue;
            }
            // The same guard the strict-read member loop applies: a
            // destructure inside a body-defined handler or helper executes
            // when that function is invoked and reads values fresh at that
            // moment — only a proven setup-time role above makes it a
            // once-frozen snapshot.
            if crate::owners::inside_non_component_function(
                file,
                binding.pattern,
                ctx.semantic_lookup,
            ) && crate::execution_role::named_callback_execution_role(
                file,
                binding.pattern,
                ctx.semantic_lookup,
            )
            .is_none()
            {
                continue;
            }
            let mut uncertain = false;
            if let Some(declaration) = &prop_declaration {
                match destructured_props_use(ctx, binding, declaration) {
                    crate::source_discovery::PropUse::Static => continue,
                    crate::source_discovery::PropUse::Reactive => {}
                    crate::source_discovery::PropUse::Unknown => uncertain = true,
                }
            }
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
                    uncertain,
                },
            );
        }
    }
}

/// The caller classification of the props a destructuring pattern binds: the
/// named slots when they are all it binds, the whole object when a rest
/// element (or an unresolved slot) captures the remainder.
fn destructured_props_use(
    ctx: &AnalysisContext<'_>,
    binding: &solid_facts::ast::BindingFact,
    declaration: &Location,
) -> crate::source_discovery::PropUse {
    if binding.names.len() > binding.object_slots.len() {
        return ctx.props_reactivity.object_use(declaration);
    }
    ctx.props_reactivity.names_use(
        declaration,
        binding
            .object_slots
            .iter()
            .map(|slot| slot.property.as_str()),
    )
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

/// SC1002: reactive reads after an await inside a tracked async computation,
/// where dependency tracking has already ended. Accessor calls come from the
/// TypeScript-side dominance analysis; store-path and props member reads are
/// proven against the straight-line awaits the AST facts record
/// (`unconditional_awaits`), with the same precision guards — no conditional
/// dominance, no nested closures except the separately proven synchronous
/// Array#filter callback path.
fn reactive_read_after_await(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    // The producer reports an async-function fact for every project
    // function, so both per-function steps below must stay cheap: the file
    // lookup goes through this index instead of a linear scan, and the
    // tracked-computation proof — a whole-project call scan — runs only for
    // a function that has something to report. A function with no
    // after-await calls and no unconditional await cannot produce a finding,
    // so resolving its computation context is pure cost.
    let files_by_path: std::collections::HashMap<&str, &solid_facts::FileFacts> = ctx
        .facts
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    for typescript_file in ctx.facts.typescript.files() {
        for function in typescript_file.async_functions.iter() {
            let member_site = ctx
                .dialect
                .reports_member_reads_after_await()
                .then(|| member_read_site(&files_by_path, function))
                .flatten();
            if function.calls_after_await.is_empty() && member_site.is_none() {
                continue;
            }
            let Some(analysis_context) = tracked_async_computation_context(ctx, function) else {
                continue;
            };
            let mut reported_calls = HashSet::new();
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
                    end_byte: call.end_byte,
                };
                reported_calls.insert((call.path.to_string(), call.start_byte, call.end_byte));
                draft.push_defect(
                    "reactive-read-after-await",
                    StaticDefect {
                        kind: StaticDefectKind::ReactiveReadAfterAwait {
                            accessor: display.to_string(),
                        },
                        location: diagnostic_location,
                        analysis_context: analysis_context.clone(),
                        fixes: vec![],
                        uncertain: false,
                    },
                );
            }
            if let Some(site) = member_site {
                member_reads_after_await(ctx, draft, site, &analysis_context);
                let (file, ast_function, boundary) = site;
                for filter in file.ast.calls_within(ast_function.body) {
                    if filter.span.start < boundary
                        || containing_ast_function(&file.ast, filter.span)
                            .is_none_or(|owner| owner.span != ast_function.span)
                    {
                        continue;
                    }
                    let Some(callback) = inline_standard_callback(ctx, file, filter) else {
                        continue;
                    };
                    // The callback body runs before the awaiting computation
                    // resumes, so its reads are the awaiting function's reads:
                    // the same accessor-call and member-read proofs apply, with
                    // the callback itself as the owning function.
                    member_reads_after_await(
                        ctx,
                        draft,
                        (file, callback, boundary),
                        &analysis_context,
                    );
                    for callback_call in file.ast.calls_within(callback.body) {
                        if callback_call.span.start < boundary
                            || containing_ast_function(&file.ast, callback_call.span)
                                .is_none_or(|owner| owner.span != callback.span)
                        {
                            continue;
                        }
                        let Some(symbol) = ctx
                            .entities
                            .get(&location(file.path.shared(), callback_call.callee))
                        else {
                            continue;
                        };
                        let Some((name, _)) = ctx.accessors.get(symbol) else {
                            continue;
                        };
                        let key = (
                            file.path.to_string(),
                            u64::from(callback_call.callee.start),
                            u64::from(callback_call.callee.end),
                        );
                        if !reported_calls.insert(key) {
                            continue;
                        }
                        let display = callback_call.static_callee(&file.source).unwrap_or(name);
                        draft.push_defect(
                            "reactive-read-after-await",
                            StaticDefect {
                                kind: StaticDefectKind::ReactiveReadAfterAwait {
                                    accessor: display.to_string(),
                                },
                                location: location(file.path.shared(), callback_call.callee),
                                analysis_context: analysis_context.clone(),
                                fixes: vec![],
                                uncertain: false,
                            },
                        );
                    }
                }
            }
        }
    }
}

/// The function a standard-library method provably invokes *inline*, or
/// `None` when nothing proves that.
///
/// Three separate facts have to hold, and none of them follows from the
/// spelling `.filter`:
///
/// - the callee resolves to the exact built-in `Array`/`ReadonlyArray`
///   `filter` declaration (a project-defined or shadowed `filter` resolves
///   elsewhere and a `Promise#then` callback is deferred, not inline);
/// - the argument itself is potentially callable — sampled at the *argument*
///   span, the position `argument_behavior` classifies, never at the callee,
///   whose callability answers a different question;
/// - the callback is the literal function written in argument position.
///   `rows.filter(makePredicate(post => …))` hands the arrow to a wrapper that
///   may stash it and run it under a later tracking scope, so it is not proof.
///
/// An `async` callback suspends at its own first await, so its body is not one
/// synchronous extent and stays outside this proof.
fn inline_standard_callback<'f>(
    ctx: &AnalysisContext<'_>,
    file: &'f solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Option<&'f solid_facts::ast::FunctionFact> {
    let argument = call.arguments.first()?;
    let resolved = ctx
        .semantic_lookup
        .resolved_callee_call(file, call.callee)?;
    let callability = ctx
        .semantic_lookup
        .smallest_contained_callability(file.path.as_str(), argument.span);
    if !is_proven_array_filter(resolved, callability) {
        return None;
    }
    let callback = callback_argument_literal(file, argument.span)?;
    (!callback.r#async).then_some(callback)
}

/// The resolved location of an async function's member-read analysis: its
/// file, its AST function, and the end of the first await in the function's
/// own unconditional flow. `None` means the function has no straight-line
/// await, so no member read can be dominated by one and the rule has nothing
/// to prove there.
fn member_read_site<'f>(
    files_by_path: &std::collections::HashMap<&str, &'f solid_facts::FileFacts>,
    function: &typefacts::AsyncFunctionFact,
) -> Option<(
    &'f solid_facts::FileFacts,
    &'f solid_facts::ast::FunctionFact,
    u32,
)> {
    let file = files_by_path.get(&*function.expression.path)?;
    let start = u32::try_from(function.expression.start_byte).ok()?;
    let end = u32::try_from(function.expression.end_byte).ok()?;
    let expression = file.ast.peel_ts_sugar_span(Span::new(start, end));
    let ast_function = file
        .ast
        .functions
        .iter()
        .find(|candidate| candidate.span == expression)?;
    let boundary = file
        .ast
        .unconditional_awaits
        .iter()
        .filter(|awaited| {
            ast_function.body.contains(**awaited)
                && containing_ast_function(&file.ast, **awaited)
                    .is_some_and(|owner| owner.span == ast_function.span)
        })
        .map(|awaited| awaited.end)
        .min()?;
    Some((file, ast_function, boundary))
}

/// The tracked-computation proof for one async function: the call that
/// receives it as a tracked compute callback, named for the finding's
/// analysis context. `None` means no tracked computation receives the
/// function and its after-await reads are not this rule's business.
fn tracked_async_computation_context(
    ctx: &AnalysisContext<'_>,
    function: &typefacts::AsyncFunctionFact,
) -> Option<String> {
    let function_symbol = async_symbol_root(
        ctx.aliases
            .get(function.symbol.as_ref())
            .map_or(function.symbol.as_ref(), SymbolId::as_str),
        &ctx.facts.typescript,
    );
    ctx.facts.files.iter().find_map(|file| {
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
    })
}

/// Store-path and component-props member reads dominated by a straight-line
/// await of the same async computation. The dominating await must sit in the
/// function's own unconditional flow (no conditional dominance), and both the
/// await and the read must belong to the function directly (no nested
/// closures). Props follow the caller classification: a proven-static prop is
/// not reactive and stays silent, an unprovable one is a proof obligation.
fn member_reads_after_await(
    ctx: &AnalysisContext<'_>,
    draft: &mut ProgramDraft,
    site: (
        &solid_facts::FileFacts,
        &solid_facts::ast::FunctionFact,
        u32,
    ),
    analysis_context: &str,
) {
    let (file, ast_function, boundary) = site;
    for member in &file.ast.members {
        if member.span.start < boundary
            || !ast_function.body.contains(member.span)
            || containing_ast_function(&file.ast, member.span)
                .is_none_or(|owner| owner.span != ast_function.span)
        {
            continue;
        }
        // Only the complete member chain reads a store path; its prefixes are
        // the same read's steps.
        if file
            .ast
            .members
            .iter()
            .any(|candidate| candidate.object == member.span)
        {
            continue;
        }
        if file.ast.is_plain_assignment_target(member.span) {
            continue;
        }
        let Some(symbol) = ctx
            .entities
            .get(&location(file.path.shared(), member.object))
        else {
            continue;
        };
        let store = ctx.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
            && ctx.accessors.contains_key(symbol);
        let (name, uncertain) = if store {
            let Some((name, _)) = ctx.accessors.get(symbol) else {
                continue;
            };
            (name, false)
        } else if let Some((name, declaration)) = ctx.prop_sources.get(symbol) {
            let property = file.source_text(member.property).unwrap_or_default();
            match ctx.props_reactivity.prop_use(declaration, property) {
                crate::source_discovery::PropUse::Static => continue,
                crate::source_discovery::PropUse::Reactive => (name, false),
                crate::source_discovery::PropUse::Unknown => (name, true),
            }
        } else {
            continue;
        };
        let accessor = file
            .source_text(member.span)
            .and_then(|path| {
                path.find('.')
                    .map(|index| format!("{name}{}", &path[index..]))
            })
            .unwrap_or_else(|| {
                format!(
                    "{name}.{}",
                    file.source_text(member.property).unwrap_or_default()
                )
            });
        draft.push_defect(
            "reactive-read-after-await",
            StaticDefect {
                kind: StaticDefectKind::ReactiveReadAfterAwait { accessor },
                location: location(file.path.shared(), member.span),
                analysis_context: analysis_context.to_owned(),
                fixes: vec![],
                uncertain,
            },
        );
    }
}

/// SC1004: a component whose return value hinges on a reactive condition,
/// evaluated once at setup and never again. Runs after the read tables are
/// merged: "reactive condition" is answered by the draft's reads.
///
/// A test is structural only when it selects which returned tree exists:
/// an `if`/`switch` a return sits under (its recorded control tests), or the
/// conditional/logical spine of the returned expression itself. A ternary
/// nested inside a JSX attribute of a returned branch is a tracked binding,
/// not a structural branch, and stays out.
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
            let mut tests = Vec::new();
            for returned in &direct_returns {
                tests.extend(returned.control_tests.iter().copied());
                if let Some(argument) = returned.argument {
                    return_spine_tests(file, argument, &mut tests);
                }
            }
            tests.sort_unstable();
            tests.dedup();
            for test in tests.iter().filter(|test| {
                function.body.contains(**test)
                    && containing_ast_function(&file.ast, **test)
                        .is_some_and(|owner| owner.span == function.span)
            }) {
                let reactive = draft.reads.iter().any(|read| {
                    read.location.path == file.path.as_str().into()
                        && u64::from(test.start) <= read.location.start_byte
                        && read.location.end_byte <= u64::from(test.end)
                });
                if reactive {
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
                            uncertain: false,
                        },
                    );
                }
            }
        }
    }
}

/// The structural tests of one returned expression: the top-level conditional
/// chain's tests, and the guards of logical expressions whose right operand
/// renders JSX structure (`return props.user && <Profile/>`, `return cond()
/// || <Fallback/>`). Branches recurse, so a chained ternary and a logical
/// inside a ternary branch each contribute their own test; anything nested
/// deeper — a ternary inside a JSX attribute, say — is evaluated where JSX
/// tracks and is not a structural branch.
fn return_spine_tests(file: &solid_facts::FileFacts, expression: Span, tests: &mut Vec<Span>) {
    let expression = file.ast.peel_ts_sugar_span(expression);
    if let Some(conditional) = file
        .ast
        .conditional_expressions
        .iter()
        .find(|conditional| conditional.span == expression)
    {
        tests.push(conditional.test);
        return_spine_tests(file, conditional.consequent, tests);
        return_spine_tests(file, conditional.alternate, tests);
    } else if let Some(logical) = file
        .ast
        .logical_expressions
        .iter()
        .find(|logical| logical.span == expression)
    {
        if jsx_structure_within(file, logical.right) {
            tests.push(logical.left);
        }
        return_spine_tests(file, logical.left, tests);
        return_spine_tests(file, logical.right, tests);
    }
}

/// Whether a span directly holds JSX structure (an element or fragment).
fn jsx_structure_within(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast.jsx_within(span).next().is_some()
        || file
            .ast
            .jsx_fragments
            .iter()
            .any(|fragment| span.contains(*fragment))
}
