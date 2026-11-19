//! ADR 1129: what a `merge` or `omit` view of a component's own props hands an
//! intrinsic element's spread.
//!
//! `const rest = omit(merge({ type: "button" }, props), "variant")` spread onto
//! `<button {...rest}>` gives the element the keys rc.21's views enumerate, and
//! nothing else. Which keys those are is a fact about the component's callers:
//! the props object is the one the compiler builds at each JSX call site, with
//! one key per attribute written there and `children` when children are
//! written. When every caller of the component is such a site, the spread's key
//! set and the value under each listener key are known exactly, per site.
//!
//! The runtime this rests on (`@solidjs/signals@2.0.0-rc.21` `dist/dev.js`,
//! re-exported by `solid-js@2.0.0-rc.21`; `@solidjs/web@2.0.0-rc.21`
//! `dist/web.dev.js`; `solid-js` `dist/solid.dev.js`):
//! - `merge` (9576-9683) of two or more sources is a proxy over the flattened
//!   sources; its `ownKeys` (`mergeEnumerableKeys`, `collectKeys`) is the union
//!   of each plain source's own enumerable keys (`ownEnumerableKeys`, 3107), and
//!   a key's value is the LAST source that has it (`mergeLookup`, 9247-9283).
//!   One non-function source is returned as is (the same key set).
//! - `omit` (9684-9707) with string keys is a proxy whose `ownKeys` is its
//!   source's minus the listed keys (`isHidden` is `hidden.includes(key)`,
//!   8766); over a merge it filters the merge's keys.
//! - Both proxies' `set` and `deleteProperty` traps are no-ops (`trueFn`).
//! - `createComponent` (`solid.dev.js` 1403, `observedComponent` 64-90) hands
//!   the component the props object the compiled JSX built, as is (`props ||
//!   {}`); web's `spread` (683) reads the spread object's own keys
//!   (`collectProps`, `Object.keys` of each leaf, `leafKeys` 8821).
//!
//! So the spread's keys are, per call site, the union over the view's sources
//! of the keys each holds (a literal's own keys; the site's attribute names,
//! plus `children` where it writes children), minus the `omit` keys. The union
//! over all sites over-approximates what any run hands the element, which is
//! the safe direction for both consumers: SC8003 asks whether a content key may
//! be there, SC4001 asks which functions the listener keys hold.
//!
//! Everything else fails closed (no answer, the obligations stand):
//! - a spread that is not a `merge`/`omit` view (bare `{...props}` is not this
//!   proof), a callee that is not the exact Solid primitive (a shadowing
//!   `merge`, a wrapper, a member of a project object), a spread argument, a
//!   source that is neither a closed object literal (no spread, computed key,
//!   duplicate, setter or `__proto__`) nor the props parameter, a view of a view
//!   beyond `omit(merge(...))`, an `omit` key that is not a string literal (a
//!   predicate, a variable), a key the kernel cannot cook, a namespaced key;
//! - a props parameter that is not the only, plain, undefaulted, unwritten
//!   parameter of a function bound to an unwritten, unexported name, a body
//!   that spells `arguments`, a reference to the parameter or to a view binding
//!   that is not an argument of the chain, a member read (not written, deleted,
//!   iterated into or called: a call hands the receiver to the callee as
//!   `this`), an intrinsic element's spread (views only) or a type query;
//! - a reference to the component that is not the tag of a JSX element (a
//!   value use, a call, an export, a dotted tag), a TypeScript reference the
//!   binder does not account for, a site with a spread, a namespaced
//!   attribute, or an attribute or child that spells `this` (a getter the
//!   compiler builds for it would see the props object as `this`);
//! - under a listener key, a value that is not a function literal written at
//!   the site or as a literal's data property; an unaudited dialect (whether a
//!   key is a listener is unknown).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use solid_facts::FileFacts;
use solid_facts::ast;
use solid_facts::core::Span;

use crate::base::memo::build::BuildSchema;
use crate::base::memo::{Ctx, Cycle};

use super::memo::{Slot, facts};
use super::{binding_written, intrinsic_element};

/// The proof's id: the proof log's `silent` lines name it, and
/// `SOLID_CHECKER_IR_ABLATE_SILENCE` turns it off (ADR 1118's gate).
pub(crate) const PROOF: &str = "object_members.merge_omit_view_spread";

/// What a proven view spread supplies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ViewSpread {
    /// Every key the spread object may own on some run, sorted.
    pub(super) keys: Vec<Arc<str>>,
    /// The function under each listener key on every run, or `None` when one
    /// such value is not an exact function literal.
    pub(super) handlers: Option<Vec<(Arc<str>, Span)>>,
}

/// One source of a `merge` (an `omit` over the props has the props alone).
#[derive(Clone, Copy)]
enum Source {
    /// A closed object literal, by its span.
    Literal(Span),
    /// The component's props parameter.
    Props,
}

/// A resolved view chain: its sources in argument order, the `omit` keys, the
/// props parameter's declaration, and the spans the chain accounts for (each
/// argument naming the props or a view binding, and each view binding's
/// declaration).
struct Chain {
    sources: Vec<Source>,
    hidden: BTreeSet<String>,
    props: Option<Span>,
    accounted: BTreeSet<Span>,
    bindings: Vec<Span>,
}

/// What a value under a key is, for the listener question.
#[derive(Clone, Copy)]
enum Value {
    /// A function literal, by the function's span.
    Function(Span),
    Other,
}

/// ADR 1129: the view spread at `argument` (an intrinsic element's spread
/// argument in `slot`), or `None` when any step above is not proved.
pub(super) fn view_spread(
    cx: &Ctx<'_, '_, BuildSchema>,
    slot: Slot,
    argument: Span,
) -> Result<Option<ViewSpread>, Cycle> {
    if crate::site_accounting::ablated_id(PROOF) {
        return Ok(None);
    }
    let Some(file) = facts(cx, slot) else {
        return Ok(None);
    };
    // The primitive table is per path; a path held by two slots is not asked.
    if cx.slots(slot.id).len() != 1 {
        return Ok(None);
    }
    let primitives = cx.memo::<crate::indexes::CallPrimitives>(&slot.id)?;
    let mut chain = Chain {
        sources: Vec::new(),
        hidden: BTreeSet::new(),
        props: None,
        accounted: BTreeSet::new(),
        bindings: Vec::new(),
    };
    if !resolve_view(file, &primitives, argument, 0, &mut chain) {
        return Ok(None);
    }
    let Some(props) = chain.props else {
        return Ok(None);
    };
    let Some(sites) = component_sites(cx, slot, file, props, &chain) else {
        return Ok(None);
    };
    Ok(spread_of_sites(cx, file, &chain, &sites))
}

/// The view at `span`: an `omit` or `merge` call written there or held by a
/// `const` binding. `depth` 0 is the spread's own argument; only an `omit`
/// may look one level further, through a `merge`.
fn resolve_view(
    file: &FileFacts,
    primitives: &BTreeMap<Span, solid_dialect::Primitive>,
    span: Span,
    depth: u8,
    chain: &mut Chain,
) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    let call_span = if file.ast.has_reference_identifier_at(span) {
        let Some(binding) = view_binding(file, span) else {
            return false;
        };
        chain.accounted.insert(span);
        chain.bindings.push(binding.names[0].span);
        let Some(initializer) = binding.initializer else {
            return false;
        };
        file.ast.peel_ts_sugar_span(initializer)
    } else {
        span
    };
    let Some(call) = file
        .ast
        .call_at(call_span)
        .filter(|call| call.span == call_span)
    else {
        return false;
    };
    if call.construct || call.arguments.iter().any(|argument| argument.spread) {
        return false;
    }
    match primitives.get(&call.span) {
        Some(solid_dialect::Primitive::Merge) => {
            if call.arguments.is_empty() {
                return false;
            }
            for argument in &call.arguments {
                let value = file.ast.peel_ts_sugar_span(argument.span);
                if closed_literal(file, value) {
                    chain.sources.push(Source::Literal(value));
                } else if props_argument(file, value, chain) {
                    chain.sources.push(Source::Props);
                } else {
                    return false;
                }
            }
            // Exactly one props source: the per-site key set is that source's.
            chain
                .sources
                .iter()
                .filter(|source| matches!(source, Source::Props))
                .count()
                == 1
        }
        Some(solid_dialect::Primitive::Omit) => {
            if depth > 0 {
                return false;
            }
            let Some((source, keys)) = call.arguments.split_first() else {
                return false;
            };
            for key in keys {
                match &key.literal_value {
                    ast::ArgumentLiteralFact::String(key) => {
                        chain.hidden.insert(key.to_string());
                    }
                    // A predicate, a variable, a number: the hidden set is not read.
                    _ => return false,
                }
            }
            let value = file.ast.peel_ts_sugar_span(source.span);
            if props_argument(file, value, chain) {
                chain.sources.push(Source::Props);
                true
            } else {
                resolve_view(file, primitives, value, depth + 1, chain)
            }
        }
        _ => false,
    }
}

/// The `const` binding of a view name: an immutable single identifier, never
/// written, declared outside a `switch` case, with an initializer.
fn view_binding(file: &FileFacts, reference: Span) -> Option<&ast::BindingFact> {
    let declaration = file.ast.reference_declaration(reference)?;
    let binding = file
        .ast
        .bindings
        .iter()
        .find(|binding| binding.names.len() == 1 && binding.names[0].span == declaration)?;
    (binding.immutable
        && binding.shape == ast::BindingShape::Identifier
        && !binding.in_switch_case
        && binding.initializer.is_some()
        && !binding_written(file, declaration))
    .then_some(binding)
}

/// Whether `value` is an object literal whose own keys are closed: no spread,
/// computed or numeric key, duplicate, setter or prototype-setting member.
fn closed_literal(file: &FileFacts, value: Span) -> bool {
    file.ast
        .object_get_shapes
        .iter()
        .any(|shape| shape.span == value && shape.closed)
}

/// Whether `value` names a function's props parameter (recorded in `chain`;
/// a chain names one).
fn props_argument(file: &FileFacts, value: Span, chain: &mut Chain) -> bool {
    if !file.ast.has_reference_identifier_at(value) {
        return false;
    }
    let Some(declaration) = file.ast.reference_declaration(value) else {
        return false;
    };
    let is_parameter = file.ast.functions.iter().any(|function| {
        function
            .parameters
            .iter()
            .any(|parameter| parameter.names.iter().any(|name| name.span == declaration))
    });
    if !is_parameter || chain.props.is_some_and(|props| props != declaration) {
        return false;
    }
    chain.props = Some(declaration);
    chain.accounted.insert(value);
    true
}

/// The literal's own properties (not those of a nested literal).
fn own_properties(
    file: &FileFacts,
    literal: Span,
) -> impl Iterator<Item = &ast::ObjectPropertyFact> {
    file.ast.object_properties.iter().filter(move |property| {
        literal.contains(property.span)
            && !file.ast.object_get_shapes.iter().any(|nested| {
                nested.span != literal
                    && literal.contains(nested.span)
                    && nested.span.contains(property.span)
            })
    })
}

/// One call site's props object: per key, the values written for it.
type SiteKeys = BTreeMap<String, Vec<Value>>;

/// Every call site of the component whose props parameter is `props`, each
/// site's keys, when every reference to the component is one and every
/// reference to the props and the view bindings is accounted for.
fn component_sites(
    cx: &Ctx<'_, '_, BuildSchema>,
    slot: Slot,
    file: &FileFacts,
    props: Span,
    chain: &Chain,
) -> Option<Vec<SiteKeys>> {
    // The component: the function whose only parameter is the props, a plain
    // identifier with no default (`createComponent` passes `props || {}`).
    let component = file.ast.functions.iter().find(|function| {
        function.parameters.len() == 1
            && function.parameters[0]
                .names
                .iter()
                .any(|name| name.span == props)
    })?;
    let parameter = &component.parameters[0];
    if parameter.shape != ast::BindingShape::Identifier
        || parameter.names.len() != 1
        || parameter.initializer.is_some()
        || component.rest_parameter
        || binding_written(file, props)
    {
        return None;
    }
    // `arguments[0]` is the props object under another name: a spelling veto
    // (comments and strings may over-veto; it never proves anything).
    if spells(file, component.body, "arguments") {
        return None;
    }
    let name = component_name(file, component)?;
    if binding_written(file, name.span) || exported(file, name) {
        return None;
    }
    // The props and the view bindings: every reference is accounted for.
    if !references_accounted(file, props, chain, false) {
        return None;
    }
    for binding in &chain.bindings {
        if !references_accounted(file, *binding, chain, true) {
            return None;
        }
    }
    // The component's references: each a JSX tag naming it.
    let mut tags = BTreeSet::new();
    let mut sites = Vec::new();
    for (reference, _) in file
        .ast
        .reference_declarations
        .iter()
        .filter(|(_, bound)| *bound == name.span)
    {
        if in_type_query(file, *reference) {
            continue;
        }
        if let Some(element) = file
            .ast
            .jsx_elements
            .iter()
            .find(|element| element.member_object.is_none() && element.name.span == *reference)
        {
            if intrinsic_element(file, element) {
                return None;
            }
            tags.insert(*reference);
            sites.push(site_keys(file, element)?);
        } else if file.ast.jsx_elements.iter().any(|element| {
            element.member_object.is_none() && element.closing_name == Some(*reference)
        }) {
            tags.insert(*reference);
        } else {
            return None;
        }
    }
    // TypeScript's references of the component's symbol: every one is the
    // declaration or a tag the binder accounted for.
    let symbol = cx.file(slot.id, slot.rank as usize)?.symbol_at(name.span)?;
    let references = cx.symbol_references(symbol.as_str())?;
    for (path, start, end) in references {
        let (id, _) = cx.path_file(&path)?;
        let (Ok(start), Ok(end)) = (u32::try_from(start), u32::try_from(end)) else {
            return None;
        };
        let span = Span::new(start, end);
        if id != slot.id
            || !(span == name.span || tags.contains(&span) || in_type_query(file, span))
        {
            return None;
        }
    }
    (!sites.is_empty()).then_some(sites)
}

/// The name a component function is bound to: a function declaration's own
/// name, or an immutable identifier binding whose initializer is exactly the
/// function. A method, a wrapped function (`const C = wrap((p) => …)`) and an
/// anonymous function have none.
fn component_name<'f>(
    file: &'f FileFacts,
    function: &'f ast::FunctionFact,
) -> Option<&'f ast::NamedSpan> {
    if function.kind == ast::FunctionKind::Declaration {
        return function.name.as_ref();
    }
    let binding = file.ast.bindings.iter().find(|binding| {
        binding
            .initializer
            .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == function.span)
    })?;
    (binding.immutable
        && binding.shape == ast::BindingShape::Identifier
        && binding.names.len() == 1)
        .then(|| &binding.names[0])
}

/// Whether the file exports the name in any form (a spelling veto as well as
/// the exact span: `export { C }`, `export default C`, `export function C`).
fn exported(file: &FileFacts, name: &ast::NamedSpan) -> bool {
    let spelled = file.source_text(name.span);
    file.ast.exports.iter().any(|export| {
        export
            .specifiers
            .iter()
            .chain(&export.declarations)
            .chain(&export.declaration_surface_only)
            .any(|specifier| {
                specifier.local.span == name.span
                    || file.source_text(specifier.local.span) == spelled
            })
    })
}

/// Whether every reference bound to `declaration` is accounted for: an
/// argument of the chain, a type query, a member read, and for a view
/// binding (`view`) an intrinsic element's spread argument.
fn references_accounted(file: &FileFacts, declaration: Span, chain: &Chain, view: bool) -> bool {
    file.ast
        .reference_declarations
        .iter()
        .filter(|(_, bound)| *bound == declaration)
        .all(|(reference, _)| {
            chain.accounted.contains(reference)
                || in_type_query(file, *reference)
                || member_read(file, *reference)
                || (view
                    && file.ast.jsx_elements.iter().any(|element| {
                        intrinsic_element(file, element)
                            && element.spreads.iter().any(|spread| {
                                file.ast.peel_ts_sugar_span(spread.argument) == *reference
                            })
                    }))
        })
}

/// Whether `reference` is the object of a non-computed member that is only
/// read: never written, deleted, assigned by a loop head, or called (a call
/// passes the object as `this`) or used as a template tag.
fn member_read(file: &FileFacts, reference: Span) -> bool {
    let Some(member) = file
        .ast
        .members
        .iter()
        .find(|member| file.ast.peel_ts_sugar_span(member.object) == reference)
    else {
        return false;
    };
    let span = member.span;
    file.ast.computed_members.binary_search(&span).is_err()
        && !file.ast.write_targets.contains(&span)
        && !file.ast.deleted_targets.contains(&span)
        && !file.ast.iteration_targets.contains(&span)
        && !file.ast.tagged_template_tags.contains(&span)
        && !file
            .ast
            .calls
            .iter()
            .any(|call| file.ast.peel_ts_sugar_span(call.callee) == span)
        && !file
            .ast
            .assignments
            .iter()
            .any(|assignment| file.ast.peel_ts_sugar_span(assignment.target) == span)
}

fn in_type_query(file: &FileFacts, span: Span) -> bool {
    file.ast
        .type_queries
        .iter()
        .any(|query| query.contains(span))
}

/// Whether `word` is spelled as a whole word in the source at `span`.
fn spells(file: &FileFacts, span: Span, word: &str) -> bool {
    let Some(text) = file.source_text(span) else {
        return true;
    };
    let identifier = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    text.match_indices(word).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(identifier)
            && !text[at + word.len()..]
                .chars()
                .next()
                .is_some_and(identifier)
    })
}

/// One JSX call site's keys: every attribute name, and `children` when the
/// element writes children. `None` for a spread, a namespaced attribute, or a
/// value or child that spells `this`.
fn site_keys(file: &FileFacts, element: &ast::JsxElementFact) -> Option<SiteKeys> {
    if !element.spreads.is_empty() {
        return None;
    }
    let mut keys = SiteKeys::new();
    for attribute in &element.attributes {
        if attribute.namespace.is_some() {
            return None;
        }
        let name = file.source_text(attribute.name)?;
        if name.contains(':') {
            return None;
        }
        if attribute
            .value
            .is_some_and(|value| spells(file, value, "this"))
        {
            return None;
        }
        let value = match (attribute.value_kind, attribute.expression) {
            (ast::JsxAttributeValueKind::Expression, Some(expression)) => file
                .ast
                .function_at(file.ast.peel_ts_sugar_span(expression))
                .map_or(Value::Other, |function| Value::Function(function.span)),
            _ => Value::Other,
        };
        keys.entry(name.to_owned()).or_default().push(value);
    }
    if !element.children.is_empty() {
        if element
            .children
            .iter()
            .any(|child| spells(file, *child, "this"))
        {
            return None;
        }
        keys.entry("children".to_owned())
            .or_default()
            .push(Value::Other);
    }
    Some(keys)
}

/// The spread's keys and listener values over every site.
fn spread_of_sites(
    cx: &Ctx<'_, '_, BuildSchema>,
    file: &FileFacts,
    chain: &Chain,
    sites: &[SiteKeys],
) -> Option<ViewSpread> {
    // Each literal's keys (cooked by the kernel, ADR 1120) with their values.
    let mut literals = Vec::new();
    for source in &chain.sources {
        let Source::Literal(literal) = source else {
            literals.push(None);
            continue;
        };
        let mut keys = SiteKeys::new();
        for property in own_properties(file, *literal) {
            let key = crate::kernel::property_key(file, property.key)?;
            if key.contains(':') {
                return None;
            }
            let value = if property.data && !property.method {
                file.ast
                    .function_at(file.ast.peel_ts_sugar_span(property.value))
                    .map_or(Value::Other, |function| Value::Function(function.span))
            } else {
                Value::Other
            };
            keys.entry(key).or_default().push(value);
        }
        literals.push(Some(keys));
    }
    let mut keys = BTreeSet::<Arc<str>>::new();
    let mut handlers = Some(BTreeSet::<(Arc<str>, Span)>::new());
    for site in sites {
        let candidates = literals
            .iter()
            .flatten()
            .flat_map(|literal| literal.keys())
            .chain(site.keys())
            .collect::<BTreeSet<_>>();
        for key in candidates {
            if chain.hidden.contains(key.as_str()) {
                continue;
            }
            // The last source that has the key answers it (`mergeLookup`).
            let values = chain.sources.iter().zip(&literals).rev().find_map(
                |(source, literal)| match source {
                    Source::Literal(_) => literal.as_ref().and_then(|literal| literal.get(key)),
                    Source::Props => site.get(key),
                },
            )?;
            keys.insert(Arc::from(key.as_str()));
            match cx.dialect().spread_key_is_event(key) {
                Some(false) => {}
                Some(true) => {
                    for value in values {
                        match (value, handlers.as_mut()) {
                            (Value::Function(function), Some(found)) => {
                                found.insert((Arc::from(file.path.as_str()), *function));
                            }
                            _ => handlers = None,
                        }
                    }
                }
                None => handlers = None,
            }
        }
    }
    Some(ViewSpread {
        keys: keys.into_iter().collect(),
        handlers: handlers.map(|found| found.into_iter().collect()),
    })
}
