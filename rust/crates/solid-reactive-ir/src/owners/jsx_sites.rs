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

/// A JSX creation site of the owner-call universe (batch 11, B2 and the review of #225).
#[derive(Clone, Copy, Debug)]
pub(crate) struct UniverseJsxSite {
    pub(crate) span: Span,
    /// The element's name (or the site's own span for a fragment's child).
    pub(crate) anchor: Span,
    /// Written in the children or in a prop's value of a component element that does not render them in place, with no function between that the element hands to the component
    /// (a prop's value, a whole child expression): the getter's own evaluation runs it, where the component reads the prop (F2 of the review of #225), so the function that
    /// holds the site says nothing of the stack it runs on.
    pub(crate) getter: bool,
}

/// Batch 11, B2 (the re-review of #220): the JSX creation sites the owner-call universe lists. JSX creates a render effect, a memo or a root (a component tag runs `createComponent`,
/// which dev builds run under `createRoot`) as a creating call does, so on an effect's apply stack with no owner set it throws like one (SC6002, ADR 0883), and under a leaf owner
/// it is a forbidden creation (SC3001). They are the sites [`jsx_sites`] finds minus a child the compiler left static that may hold a function (possible only), once each by span, with
/// what a component element around the site makes of it (the review of #225, F2): the body of a function the element hands to the component (a prop's value, a whole child
/// expression) is run by whoever calls it, so the site is as any other; JSX written anywhere else in the element's props or children is a getter, run where the component reads the
/// prop, a function the getter's own evaluation calls (a `.map` callback, an immediately invoked arrow, an `untrack` callback) included; what a component that renders its getter in
/// place (a dialect control-flow element, a context provider) reads is created under the owner that render sets and is no site, when it is written directly in the getter or in the
/// body of the function handed to the element (a function written inside either runs where it is called: a site like any other); any other getter is a site no fact of the function
/// that holds it can place.
pub(crate) fn universe_jsx_sites(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
) -> Vec<UniverseJsxSite> {
    // Keyed by span, in span order: a span's first site is its entry.
    let mut by_span = std::collections::BTreeMap::new();
    for site in jsx_sites(file, lookup) {
        if site.kind == JsxSiteKind::MaybeRenderEffect {
            continue;
        }
        let getter = if site.in_getter {
            match getter_relation(file, lookup, site.span) {
                GetterRelation::Plain => false,
                GetterRelation::Getter => true,
                GetterRelation::InPlace => continue,
            }
        } else {
            false
        };
        by_span
            .entry((site.span.start, site.span.end))
            .or_insert(UniverseJsxSite {
                span: site.span,
                anchor: site.anchor,
                getter,
            });
    }
    by_span.into_values().collect()
}

/// What the innermost component element around a site makes of it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GetterRelation {
    /// The outermost function between the element and the site is handed to the component (a prop's value, a whole child expression): the body runs where the function is called. So
    /// does a function written inside what a dialect control-flow element or a context provider renders in place (inside the function handed to `For`, or in `Show`'s children): it
    /// runs where it is called, which no fact of the element says.
    Plain,
    /// Written in a prop of a component that does not render it in place, with no such function between: a getter, evaluated where the component reads the prop.
    Getter,
    /// Written directly in a getter of an element that renders it in place (a dialect control-flow element, `Match` only under `Switch`, a context provider: no function
    /// between), or in the body of a function that such an element itself calls, in a position the dialect names (`For`'s row function, `Show`'s function child, `Errored`'s
    /// fallback), unless the function can hand itself on (the site's innermost function is that function): rendered or called in place, under the owner of the render that
    /// reads it.
    InPlace,
}

fn getter_relation(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
    span: Span,
) -> GetterRelation {
    let element = file
        .ast
        .jsx_containing(span)
        .filter(|element| {
            element.span.contains(span)
                && element.span != span
                && !intrinsic_element(file, element)
                && !element.name.span.contains(span)
        })
        .min_by_key(|element| element.span.end - element.span.start);
    let Some(element) = element else {
        return GetterRelation::Plain;
    };
    // The review of #225 (B3): an instance field's initializer is no part of the body of the function the class is written in, whatever element that function is the child of:
    // it runs at each construction, which may be on any stack (the owner-call universe declines it for that).
    if crate::local_access::in_instance_field_initializer_directly(file, span) {
        return GetterRelation::Plain;
    }
    // The functions written inside the element that hold the site. The outermost, handed to the component as a prop's value or as a whole child expression, runs where whoever has it
    // calls it. Any other function in the value (a `.map` callback, an immediately invoked arrow, an `untrack` callback, a branch of a conditional) is a function the getter's own
    // evaluation may call, or a value it returns to a caller no fact names: where the component reads the prop either way. The innermost is the function whose body the site is
    // written in: it runs when that function is called.
    let holders = || {
        file.ast
            .functions_owning(span)
            .filter(|function| element.span.contains(function.span))
    };
    let outermost = holders().max_by_key(|function| function.span.end - function.span.start);
    let innermost = holders().min_by_key(|function| function.span.end - function.span.start);
    // Whether the element renders what it is handed in place, under the owner of its own render: a dialect control-flow element (`stored_throw::element_prop_handled`'s list),
    // `Match` only as `Switch`'s direct child, a context provider.
    let in_place_element = renders_in_place(file, lookup, element);
    if let Some(function) =
        outermost.filter(|function| handed_to_component(file, element, function.span))
    {
        // A function handed to a dialect control-flow element is called by that element's own render, under the owner it sets, when the dialect says the tag calls a function
        // in that position (`For`'s row function, `Show`'s function child, `Errored`'s fallback: not `Show`'s `when`, which it hands on): a creation written in ITS body throws
        // nowhere, so it is no site. A function that can hand itself on (a named function expression that uses its name) runs where it is called, as a function written inside
        // it does (stored and called by an effect's apply, it creates with no owner set): a site like any other, as is any site in a function handed to any other component.
        return if innermost.is_some_and(|inner| inner.span == function.span)
            && in_place_element
            && calls_function_in_place(file, lookup, element, function)
            && !can_hand_itself_on(file, lookup, function)
        {
            GetterRelation::InPlace
        } else {
            GetterRelation::Plain
        };
    }
    // What is written directly in a getter of such an element, with no function between, is evaluated by that read, under the owner of the render. A function written inside the
    // getter (an immediately invoked arrow, a `.map` callback, a handler, an arrow stored for later) runs where it is called, so its sites are sites like any other.
    if in_place_element {
        return if outermost.is_none() {
            GetterRelation::InPlace
        } else {
            GetterRelation::Plain
        };
    }
    GetterRelation::Getter
}

/// The review of #225 (B4): whether `element` renders what it is handed in place, under the owner of its own render: a dialect control-flow element of
/// `stored_throw::element_prop_handled`'s list, `Match` only as `Switch`'s direct child (it returns its props, so anywhere else it is a component like any other: whoever
/// reads them runs the getters), and a context provider (which reads its children in place; a function it is handed as a value is stored, not run).
fn renders_in_place(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
    element: &JsxElementFact,
) -> bool {
    use solid_dialect::Primitive;
    match crate::owners::element_primitive(lookup, file, element) {
        Some(Primitive::Match) => match_under_switch(file, lookup, element),
        Some(
            Primitive::Errored
            | Primitive::For
            | Primitive::Index
            | Primitive::Repeat
            | Primitive::Show
            | Primitive::Switch
            | Primitive::Loading,
        ) => true,
        Some(_) => false,
        None => lookup.is_context_reference(file.path.as_str(), element.name.span),
    }
}

/// Whether `element`, a `Match`, is a direct child of a `Switch`, which reads its props in its own memo.
fn match_under_switch(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
    element: &JsxElementFact,
) -> bool {
    innermost_element(file, element.span).is_some_and(|parent| {
        crate::owners::element_primitive(lookup, file, parent)
            == Some(solid_dialect::Primitive::Switch)
            && parent.children.contains(&element.span)
    })
}

/// The review of #225 (B2): whether the dialect says `element` itself calls `function`, written as the whole value of one of its props or as its function child, under the
/// owner of its own render: the positions of [`solid_dialect::Dialect::control_flow_prop_function_run`] and the function child [`control_flow_child`] reads (`runs_as_child`:
/// a generator's body runs where its iterator is stepped, and `Show` calls a child only with a parameter). A function in any other prop is a value the tag hands on.
fn calls_function_in_place(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
    element: &JsxElementFact,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    let Some(primitive) = crate::owners::element_primitive(lookup, file, element) else {
        return false;
    };
    if control_flow_child(file, element, lookup).is_some_and(|(expression, run)| {
        file.ast.peel_ts_sugar_span(expression) == function.span && runs_as_child(function, run)
    }) {
        return true;
    }
    element.attributes.iter().any(|attribute| {
        attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
            && attribute
                .expression
                .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == function.span)
            && file
                .source_text(attribute.name)
                .and_then(|prop| {
                    lookup
                        .dialect
                        .control_flow_prop_function_run(primitive, prop)
                })
                .is_some_and(|run| runs_as_child(function, run))
    })
}

/// The review of #225 (B1): whether `function`, written inline, can hand itself on, so that it runs where it is called and not only where its element calls it: a function
/// expression that uses its own name inside (`{function row() { pick = row; ... }}`), or reads `arguments` (`arguments.callee`). An arrow has neither. A name no reference of
/// the producer's is known for, or a reference it lists, is a use.
fn can_hand_itself_on(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    if function.kind == solid_facts::ast::FunctionKind::Arrow {
        return false;
    }
    if file
        .source_text(function.span)
        .is_some_and(|text| mentions_word(text, "arguments"))
    {
        return true;
    }
    let Some(own) = function.name.as_ref() else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), own.span) else {
        return true;
    };
    lookup
        .symbol_references_if_present(symbol.as_str())
        .is_none_or(|references| {
            references.iter().any(|reference| {
                reference.path.as_ref() != file.path.as_str()
                    || u32::try_from(reference.start_byte)
                        .is_ok_and(|start| start != own.span.start)
            })
        })
}

/// Whether `text` holds `word` as a whole identifier.
fn mentions_word(text: &str, word: &str) -> bool {
    let identifier = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(word).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(identifier)
            && !text[at + word.len()..]
                .chars()
                .next()
                .is_some_and(identifier)
    })
}

/// Whether code written at `span` (a creating call, a call of a function, a callee's name) is a component element's getter (the review of #225, F2, found while fixing
/// it): written in the element's children or in a prop's value with no function between that the element hands to the component, so the getter's evaluation runs it where
/// the component reads the prop. A creating call is a site of the owner-call universe as a JSX creation is, and ends or is filed by the same silences, which a getter's does
/// not satisfy (the function that holds it says nothing of the stack it runs on): `<Outer slot={createMemo(() => 1)} />` with `Outer` reading `props.slot` on an effect's
/// apply creates the memo with no owner set. A call of a function written there runs the callee on that stack the same way, so it places no callee: the entry walk takes
/// the layer it enters for one entered by a run no edge shows (`<Outer slot={renderView()} />`, `renderView` returning `<Inner />`). A getter of a component that renders it
/// in place is no getter here: the call keeps the silences it had.
pub(crate) fn written_in_getter(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
    span: Span,
) -> bool {
    getter_relation(file, lookup, span) == GetterRelation::Getter
}

/// Whether `function` is the whole value of one of the element's attributes or the whole expression of one of its children: the component receives it and decides when it runs.
fn handed_to_component(
    file: &solid_facts::FileFacts,
    element: &JsxElementFact,
    function: Span,
) -> bool {
    element.attributes.iter().any(|attribute| {
        attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
            && attribute
                .expression
                .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == function)
    }) || element
        .children
        .iter()
        .any(|child| crate::execution_role::jsx_child_expression(file, *child) == Some(function))
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
    if primitive == solid_dialect::Primitive::Match && !match_under_switch(file, lookup, element) {
        return None;
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
            solid_dialect::FunctionChildRun::WithParameter => {
                crate::execution_role::function_length(function) > 0
            }
        }
}

/// ADR 0937: the element `function` is such a child of, written in place.
pub(crate) fn control_flow_parent<'f>(
    file: &'f solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<&'f JsxElementFact> {
    innermost_element(file, function.span).filter(|element| {
        control_flow_child(file, element, lookup).is_some_and(|(expression, run)| {
            file.ast.peel_ts_sugar_span(expression) == function.span && runs_as_child(function, run)
        })
    })
}

/// Whether `function` is such a child, written in place. A function named there
/// gets no edge: it keeps its other entries, and only a literal has none.
pub(crate) fn is_control_flow_child(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    control_flow_parent(file, function, lookup).is_some()
}

/// ADR 0884: the primitive's exported name, when the resolved declaration of a
/// call no rule reads is a dialect primitive in a package the dialect owns.
pub(crate) fn aliased_primitive(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    resolved_alias_primitive(file, call, lookup)
        .or_else(|| indirect_primitive_operand(file, call.callee, lookup))
}

/// The resolved-declaration attribution of ADR 0884. It never proves an
/// invocation; casts and replaced call/bind methods remain possible.
fn resolved_alias_primitive(
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

/// ADR 0902: retain a claiming-rule obligation for an indirect call whose
/// operand names a primitive exactly. This is attribution only: a custom
/// call/apply/bind member or a logical assignment's previous value may win.
fn indirect_primitive_operand(
    file: &solid_facts::FileFacts,
    mut span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    let mut seen = crate::HashSet::default();
    loop {
        span = file.ast.peel_ts_sugar_span(span);
        if !seen.insert(span) {
            return None;
        }
        if let Some(name) = crate::referenced_primitive(
            file,
            span,
            lookup.entities(),
            lookup.symbol_names(),
            lookup.dialect,
        ) {
            return Some(name.as_str().to_owned());
        }
        if let Some(member) = file.ast.member_at(span) {
            // The name describes a transport shape, never a primitive.
            if !file.ast.computed_members.contains(&member.span)
                && matches!(file.source_text(member.property), Some("call" | "apply"))
            {
                span = member.object;
                continue;
            }
        }
        if let Some(inner) = file.ast.call_at(span)
            && !inner.construct
            && let Some(member) = file
                .ast
                .member_at(file.ast.peel_ts_sugar_span(inner.callee))
            && !file.ast.computed_members.contains(&member.span)
            && file.source_text(member.property) == Some("bind")
        {
            span = member.object;
            continue;
        }
        // AssignmentFact records the two endpoints of the expression. Exact
        // equality, not a contained reference, selects the callee's RHS.
        if let Some(assignment) = file.ast.assignments.iter().find(|assignment| {
            assignment.target.start == span.start
                && assignment.value_span.end == span.end
                && (!assignment.reads_target || assignment.logical.is_some())
        }) {
            span = assignment.value_span;
            continue;
        }
        let declaration = file.ast.reference_declaration(span)?;
        let binding = file.ast.identifier_binding(declaration)?;
        if !binding.immutable {
            return None;
        }
        span = binding.initializer?;
    }
}
