//! ADR 0883: the places JSX itself creates a computation or a root. Written
//! JSX has no call node, so the owner rules saw none of them (F4, Chrome on
//! rc.21):
//!
//! - an intrinsic's dynamic child, attribute or spread is a render effect
//!   (`insert`, `effect`, `spread`): NO_OWNER_EFFECT with no owner;
//! - a fragment's dynamic child is a memo: quiet with no owner, a creating
//!   call in an apply or a leaf;
//! - a component element runs under `createComponent`'s own transparent root
//!   (ADR 0869): quiet with no owner, a creating call in an apply or a leaf;
//! - an intrinsic's child the compiler leaves static may still hold a function
//!   (`{[f]}`, an `any`), which `insert` makes an effect: possible only.
//!
//! A `ComponentGetter` region creates nothing: it is a getter, read where the
//! component reads it. Anything under a component element runs where that
//! getter is read, inside the transparent root when the body reads it there.

use solid_facts::ast::JsxElementFact;
use solid_facts::compiler::RegionReason;
use solid_facts::core::Span;

use crate::execution_role::intrinsic_element;
use crate::indexes::SemanticLookup;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JsxSiteKind {
    RenderEffect,
    /// A child the compiler left static that may hold a function.
    MaybeRenderEffect,
    FragmentMemo,
    Component,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JsxSite {
    pub(crate) kind: JsxSiteKind,
    pub(crate) span: Span,
    /// The element's name: outside the site's own tracked region, which the
    /// compiler lists as an ownership region of its own.
    pub(crate) anchor: Span,
    /// Written in a component element's children or props: a getter decides
    /// when it runs.
    pub(crate) in_getter: bool,
}

fn innermost_element(file: &solid_facts::FileFacts, span: Span) -> Option<&JsxElementFact> {
    file.ast
        .jsx_elements
        .iter()
        .filter(|element| element.span.contains(span) && element.span != span)
        .min_by_key(|element| element.span.end - element.span.start)
}

fn in_component_getter(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast.jsx_elements.iter().any(|element| {
        element.span.contains(span)
            && element.span != span
            && !intrinsic_element(file, element)
            && !element.name.span.contains(span)
    })
}

/// Whether a fragment holds `span` inside the innermost element around it.
fn fragment_child(
    file: &solid_facts::FileFacts,
    span: Span,
    element: Option<&JsxElementFact>,
) -> bool {
    file.ast.jsx_fragments.iter().any(|fragment| {
        fragment.contains(span) && element.is_none_or(|element| element.span.contains(*fragment))
    })
}

/// A static child that cannot hold a function: a literal, JSX, or a binding
/// whose type is proven not callable.
fn cannot_hold_function(
    file: &solid_facts::FileFacts,
    expression: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let span = file.ast.peel_ts_sugar_span(expression);
    let Some(text) = file.source_text(span) else {
        return false;
    };
    let element = file
        .ast
        .jsx_elements
        .iter()
        .any(|element| element.span == span)
        || file.ast.jsx_fragments.contains(&span);
    let identifier = text
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '$');
    element
        || literal_text(text)
        || (identifier
            && lookup.smallest_contained_callability(file.path.as_str(), span)
                == Some(typefacts::Callability::NonCallable))
}

/// Whether the whole of `text` is one literal: `1 && f` starts like one.
fn literal_text(text: &str) -> bool {
    if matches!(text, "null" | "undefined" | "true" | "false") {
        return true;
    }
    if let Some(quote) = text
        .chars()
        .next()
        .filter(|c| matches!(c, '"' | '\'' | '`'))
    {
        let body = &text[1..];
        let Some(inner) = body.strip_suffix(quote) else {
            return false;
        };
        let mut escaped = false;
        return inner.chars().all(|c| {
            let closes = !escaped && (c == quote || (quote == '`' && c == '$'));
            escaped = !escaped && c == '\\';
            !closes
        });
    }
    text.starts_with(|c: char| c.is_ascii_digit())
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

pub(crate) fn jsx_sites(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
) -> Vec<JsxSite> {
    let mut sites = Vec::new();
    let regions = &file.compiler.tracked_regions;
    for region in regions {
        if region.reason == RegionReason::ComponentGetter {
            continue;
        }
        let element = innermost_element(file, region.span);
        let (kind, anchor) = match element {
            Some(element) if fragment_child(file, region.span, Some(element)) => {
                (JsxSiteKind::FragmentMemo, element.name.span)
            }
            Some(element) if intrinsic_element(file, element) => {
                (JsxSiteKind::RenderEffect, element.name.span)
            }
            Some(_) => continue,
            None if fragment_child(file, region.span, None) => {
                (JsxSiteKind::FragmentMemo, region.span)
            }
            None => continue,
        };
        sites.push(JsxSite {
            kind,
            span: region.span,
            anchor,
            in_getter: in_component_getter(file, region.span),
        });
    }
    for element in &file.ast.jsx_elements {
        if !intrinsic_element(file, element) {
            sites.push(JsxSite {
                kind: JsxSiteKind::Component,
                span: element.span,
                anchor: element.name.span,
                in_getter: in_component_getter(file, element.span),
            });
            continue;
        }
        for spread in &element.spreads {
            if regions
                .iter()
                .any(|region| region.span.contains(spread.span))
            {
                continue;
            }
            sites.push(JsxSite {
                kind: JsxSiteKind::RenderEffect,
                span: spread.span,
                anchor: element.name.span,
                in_getter: in_component_getter(file, element.span),
            });
        }
    }
    for &expression in &file.ast.jsx_child_expressions {
        if regions
            .iter()
            .any(|region| region.span.contains(expression))
        {
            continue;
        }
        let Some(element) = innermost_element(file, expression) else {
            continue;
        };
        if !intrinsic_element(file, element)
            || fragment_child(file, expression, Some(element))
            || cannot_hold_function(file, expression, lookup)
        {
            continue;
        }
        sites.push(JsxSite {
            kind: JsxSiteKind::MaybeRenderEffect,
            span: expression,
            anchor: element.name.span,
            in_getter: in_component_getter(file, expression),
        });
    }
    sites
}

/// ADR 0883: the function child `element` runs under an owner of its own: the
/// element's only child, an expression, of a tag whose exact primitive the
/// dialect answers for. `Match` only as `Switch`'s direct child. Returns the
/// expression and when the tag runs a function there.
pub(crate) fn control_flow_child(
    file: &solid_facts::FileFacts,
    element: &JsxElementFact,
    lookup: &SemanticLookup<'_>,
) -> Option<(Span, solid_dialect::FunctionChildRun)> {
    let primitive_of = |element: &JsxElementFact| {
        crate::jsx_primitive_name_cached(
            lookup,
            file,
            element,
            lookup.entities(),
            lookup.symbol_names(),
            lookup.dialect,
        )
        .as_ref()
        .and_then(crate::PrimitiveName::primitive)
    };
    let primitive = primitive_of(element)?;
    let run = lookup.dialect.control_flow_child_run(primitive)?;
    if primitive == solid_dialect::Primitive::Match {
        let parent = innermost_element(file, element.span)?;
        if primitive_of(parent) != Some(solid_dialect::Primitive::Switch)
            || !parent.children.contains(&element.span)
        {
            return None;
        }
    }
    let mut children = element.children.iter().filter(|child| {
        file.source_text(**child)
            .is_some_and(|text| !text.trim().is_empty())
    });
    let (Some(child), None) = (children.next(), children.next()) else {
        return None;
    };
    // An expression container: an element child's own function child is the
    // element's to run (`<Show><Kid>{f}</Kid></Show>`).
    if !file.source_text(*child)?.starts_with('{') {
        return None;
    }
    let expression = file
        .ast
        .jsx_child_expressions
        .iter()
        .filter(|expression| child.contains(**expression))
        .max_by_key(|expression| expression.end - expression.start)?;
    Some((*expression, run))
}

/// Whether `function` declares the `.length` a `WithParameter` tag tests: a
/// first parameter that is no rest and has no default. A generator's body runs
/// only where its iterator is stepped, so no tag runs it.
pub(crate) fn runs_as_child(
    function: &solid_facts::ast::FunctionFact,
    run: solid_dialect::FunctionChildRun,
) -> bool {
    !function.generator
        && match run {
            solid_dialect::FunctionChildRun::Always => true,
            solid_dialect::FunctionChildRun::WithParameter => function
                .parameters
                .first()
                .is_some_and(|parameter| parameter.initializer.is_none()),
        }
}

/// Whether `function` is such a child, written in place. A function named there
/// gets no edge: it keeps its other entries, and only a literal has none.
pub(crate) fn is_control_flow_child(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    innermost_element(file, function.span).is_some_and(|element| {
        control_flow_child(file, element, lookup).is_some_and(|(expression, run)| {
            file.ast.peel_ts_sugar_span(expression) == function.span && runs_as_child(function, run)
        })
    })
}

/// ADR 0884: the primitive's exported name, when the resolved declaration of a
/// call no rule reads is a dialect primitive in a package the dialect owns.
pub(crate) fn aliased_primitive(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    let resolved = lookup.resolved_callee_call(file, call.callee)?;
    let declaration = resolved.declaration.as_ref()?;
    let (_, after) = declaration.source_file.rsplit_once("/node_modules/")?;
    let mut segments = after.split('/');
    let first = segments.next()?;
    let package = if first.starts_with('@') {
        format!("{first}/{}", segments.next()?)
    } else {
        first.to_owned()
    };
    (lookup.dialect.declares_primitives_in(&package)
        && lookup
            .dialect
            .primitive(declaration.name.as_ref())
            .is_some())
    .then(|| declaration.name.to_string())
}
