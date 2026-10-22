//! JSX census regions: expressions the compiler may delete, and whether a region touches one.

use super::*;

/// Whether the untracked-rendering role at `span` rests on the *absence* of a
/// compiler census entry rather than on a compiler fact.
///
/// `semantic_execution_role_within` reaches [`ExecutionRole::UntrackedRendering`]
/// two ways. Either a compiler fact said so — an untracked region, a render
/// callback role — or nothing in the execution map classified the span at all
/// and the span is inside a component body. The second way is a proof only
/// while the census is complete over the JSX the source actually contains, and
/// it is not: each producer censuses the JSX *it* lowers, so a source-level JSX
/// expression the producer dropped reaches the checker as a hole
/// indistinguishable from "the compiler proved this never re-runs". Two ways a
/// hole arrives, and this cannot tell them apart because it does not need to:
/// the producer never censused the expression (a nested non-hydratable `<head>`
/// under 1.x; a *template-root* void element's children under 2.0, whose
/// `lower_dom_element` gates on `!is_void_element`), or it censused the
/// expression and then **retracted** the site during lowering because the path
/// discarded the child list (2.0's nested dynamic-`textContent` placeholder, the
/// textarea `value` fold, the inert `<noscript>` fast path).
///
/// Absence of a fact is not a fact. Where the narrowest JSX region containing
/// the read carries no census entry whatsoever, the read is an uncertifiable
/// proof obligation — never a proven untracked read, and equally never a
/// certification that the expression was deleted and is therefore safe, which
/// would be a second claim this has no evidence for either.
///
pub(crate) fn missing_jsx_census(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> bool {
    // Any other role was decided by a fact — a dialect-proven primitive
    // callback, an event or ref census entry, module initialization (an
    // AST-proven one-shot context that needs no census at all), a discarded
    // region (the compiler reported on the JSX and said the code is deleted) —
    // so a census hole cannot be what put the read there. A file the compiler
    // refused (ADR 0360) is one hole: no fact decided any role in it.
    if file.compiler.withheld() {
        return true;
    }
    if execution != ExecutionRole::UntrackedRendering {
        return false;
    }
    missing_jsx_census_region(file, span)
}

/// Whether the narrowest JSX region containing `span` has no compiler census
/// entry. Ownership uses the structural question directly: unlike a reactive
/// read, an owner requirement does not carry an [`ExecutionRole`] whose
/// fallback classification can gate the check.
pub(crate) fn missing_jsx_census_region(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.compiler.withheld()
        || narrowest_jsx_region_containing(file, span)
            .is_some_and(|region| !census_touches(file, region))
}

/// The innermost source-level JSX region containing `span`: an attribute
/// expression container, a spread container, or a child. solid-facts owns this
/// syntax, and it is deliberately read from the AST rather than from the
/// execution map — the whole question is what the source has that the census
/// does not.
///
/// Fragments are reached only indirectly. A fragment has no element fact — no
/// name, no attributes — so `jsx_containing` (which walks `jsx_elements`) never
/// yields one; a read inside `<>…</>` is found only through the enclosing
/// *element's* child span that covers the fragment. That is not a live hole at
/// the current pins, probed rather than assumed: a fragment nested inside an
/// element fails the compile outright in both producers ("Fragments and spread
/// children are not implemented in the AST-native milestone yet"), so the only
/// fragment reaching the checker is a component's top-level one, and its
/// children are censused as tracked regions — the role never falls through to
/// `UntrackedRendering`, so this function is not consulted for them. When a
/// producer starts lowering nested fragments, the child-span path is where a
/// fragment's own expression containers would need to appear.
pub(super) fn narrowest_jsx_region_containing(
    file: &solid_facts::FileFacts,
    span: Span,
) -> Option<Span> {
    file.ast
        .jsx_containing(span)
        .flat_map(|element| {
            let attributes = element
                .attributes
                .iter()
                .filter(|attribute| {
                    attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                })
                .filter_map(|attribute| attribute.value);
            let spreads = element.spreads.iter().map(|spread| spread.span);
            attributes
                .chain(spreads)
                .chain(element.children.iter().copied())
        })
        .filter(|region| region.contains(span))
        .min_by_key(|region| region.end - region.start)
}

/// Whether the compiler deleted the code at `span`.
///
/// The one execution fact that is not a claim about *how* code runs but about
/// whether it is emitted at all, which is why its consumers consult it before
/// every other classification rather than alongside them.
pub(crate) fn discarded_region_contains(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.compiler
        .discarded_regions
        .iter()
        .any(|region| region.span.contains(span))
}

/// Whether the compiler's census says anything at all about `region`.
///
/// Overlap, not containment in either direction: a census entry inside the
/// region and a wider entry covering it are both the compiler having reported
/// on this JSX. Only a region no entry touches is a hole.
pub(super) fn census_touches(file: &solid_facts::FileFacts, region: Span) -> bool {
    let overlaps = |candidate: Span| candidate.start < region.end && region.start < candidate.end;
    let facts = &file.compiler;
    facts
        .jsx_operations
        .iter()
        .any(|operation| overlaps(operation.span))
        || facts
            .tracked_regions
            .iter()
            .any(|tracked| overlaps(tracked.span))
        || facts
            .untracked_regions
            .iter()
            .any(|untracked| overlaps(untracked.span))
        // A discarded region is the compiler having reported on this JSX — it
        // said the code is deleted. That is a fact, so the region is not a
        // hole, and `missing_jsx_census` must not turn it into an obligation.
        || facts
            .discarded_regions
            .iter()
            .any(|discarded| overlaps(discarded.span))
        || facts.callback_roles.iter().any(|role| overlaps(role.span))
}
