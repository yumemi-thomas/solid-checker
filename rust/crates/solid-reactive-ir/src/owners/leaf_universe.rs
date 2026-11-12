//! Batch 11, slice 6: the silences of the owner-call universe that rest on the owner graph, a census or the role engine.
//!
//! The universe ([`crate::proof_universe`], table `owner-call`) lists every call of `onCleanup` and of a creation the dialect refuses under a leaf owner
//! (SC3001: CLEANUP_IN_FORBIDDEN_SCOPE, PRIMITIVE_IN_FORBIDDEN_SCOPE) or on an effect apply's stack with no owner set (SC6002: PRIMITIVE_IN_EFFECT_CALLBACK).
//! A site no row, SC9012, discard or named silence ends gets the code's own uncertifiable row ([`crate::site_accounting`]). The silences named here:
//!
//! - [`SiteSilence::OwnerCallNeverRuns`]: ADR 0458's query proves no execution runs the site (every code);
//! - [`SiteSilence::OwnerCallModuleScope`]: no function owns the site and no instance field's initializer holds it, so module evaluation runs it on a
//!   stack of its own: in no leaf owner's callback and on no effect's apply (every code);
//! - [`SiteSilence::LeafNoOpenerInProgram`]: the project names no primitive that opens a leaf scope, so there is no leaf owner to forbid the call
//!   (SC3001's two codes);
//! - [`SiteSilence::LeafOwnerAbsent`]: the context of the function that holds the site has an edge, no leaf bit, and is not the bare open-world seed of an
//!   export (SC3001's two codes);
//! - [`SiteSilence::OwnerCurrent`]: the same context gives an owner on every entry (owned, a leaf owner, or both: no unowned bit), so the effect callback's
//!   guard (`inEffectCallback && context === null`) does not meet the creation (SC6002);
//! - [`SiteSilence::EffectCreationRunsOffApply`]: the role engine places the site off every effect's apply (a handler, a deferred callback, a directive apply,
//!   rendering, module code, discarded code), and every layer around it is entered only the way that role says (SC6002);
//! - [`SiteSilence::EffectCreationOwnerSetAround`]: an exact primitive whose callback creates an owner wraps the site, every layer the climb crosses running in
//!   the primitive's synchronous run (SC6002).
//!
//! The graph's contexts are the entries it holds, and the role engine places a function from where it is written: they answer only where those are every entry
//! the program can make. Each of the graph's silences and the role silence rests on all of these facts of the site (a missing one is no silence, the site keeps its
//! uncertifiable row; the review of #220 found `EffectCreationRunsOffApply` and `EffectCreationOwnerSetAround`, then in `static_api`, asking less than the others):
//!
//! - the site is written directly in a function (not a parameter list, which runs when its argument is omitted; not after a suspension of an async function,
//!   which runs from a later task; not in a generator, or in a layer written inside one, whose body runs where an iterator is stepped: an arrow, an IIFE or a
//!   callback of an in-place primitive in a generator's body is the generator's code, up to the layer whose callback runs under an owner its primitive
//!   creates, which is current there whoever steps what is around it: B1, the re-review of #220);
//! - no layer around it is entered by a route the graph has no edge for ([`super::owner_emission::layer_entry_open_by_reference`], ADR 0868: a function stored,
//!   assigned, handed on or a method, a reference that is not a direct call or an inline run): a function that returns JSX is entered by renders alone only where every reference of its binding is a
//!   JSX tag (F1); a layer that is a generator is an open entry, written around the site or reached as the caller at the end of a call (B1, N1); and a callback
//!   an owner-creating primitive runs is entered by it alone only where nothing references it (N2). The primitive-wrapped silence needs no entry fact of its
//!   own beyond the last: the primitive's own run is the entry;
//! - no package the build does not read may run the function ([`crate::package_hand_off::UnreadPackageHandOff`], ADR 0967): such a package may call it
//!   under a leaf owner, or on an effect's apply, through an entry no edge shows. The census silence rests on this too: a package may open the leaf scope the
//!   project's own primitives do not.
//!
//! Nothing here ever files a row; a silence names the fact it rests on ([`PremiseKind`], each with the span of the code it is about). Where no silence applies for a group of
//! codes the pass writes a typed [`Decline`] to the proof log: the first fact of its order of questions that stops it (a package may run the function, a layer is entered by
//! reference, a leaf bit, an unowned entry, ...). A decline is no proof and ends nothing: it is never read by an ending or a silence, so the site keeps the uncertifiable row of
//! each code, and the gate accepts such a site only if that row stands and a decline at its span lists the code (coordinator-approved, owner delegated).

use super::*;
use crate::proof_log::PremiseKind;
use crate::site_accounting::{SiteEnds, SiteSilence};

/// Why an owner-call site has no silence from this pass for a group of its codes: one typed fact, the first the producer's order of questions meets. A decline is
/// no proof and ends nothing (never read by an ending or a silence: the site keeps the uncertifiable row of each code); it goes to the proof log only, where the gate
/// accepts an owner-call site that ends in nothing if and only if the row stands and a decline at its span lists the code. The set is closed: a reason this pass finds is
/// named here or the site is a gate hit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decline {
    /// A package the build does not read may run the function that holds the site (B3, ADR 0967): it may open a leaf scope around it or call it on an effect's apply.
    UnreadPackageMayRun,
    /// The site is in an instance field's initializer, which runs at each construction, which may be anywhere (no function owns it, and it is no module scope).
    InstanceFieldInitializer,
    /// The site is in a parameter list: it runs when a call omits its argument, which no entry places.
    ParameterList,
    /// The site is in a generator, or in a layer written inside one (up to the layer whose callback runs under an owner its primitive creates): its body runs where an
    /// iterator is stepped, which no owner edge follows.
    GeneratorBody,
    /// The site follows a suspension of its async function: it runs from a later task, which no owner edge follows.
    AfterSuspension,
    /// The owner graph holds no node for the function that holds the site.
    NoOwnerNode,
    /// No owner edge reaches the function (context zero): its entries are not known.
    NoEdgeIntoFunction,
    /// The function is an export whose only entry is the unseen caller of an open world (the bare unowned seed): who calls it, and under what owner, is not known.
    OpenWorldExport,
    /// The context has a leaf bit: some entry runs the site under a leaf owner (a leaf operation's row is what ends the site, and none stands here).
    LeafBitPresent,
    /// The context has an entry that gives no owner (or an owner that is only conventional): the creation may meet `context === null`.
    UnownedEntry,
    /// A layer around the site is entered by a route the graph has no edge for: a function stored, assigned, handed on or a method (ADR 0868).
    EntryByReference,
}

impl Decline {
    /// The reason's id in the proof log. One arm per reason, no wildcard: a new reason names its id or the build fails.
    #[deny(clippy::wildcard_enum_match_arm)]
    const fn id(self) -> &'static str {
        match self {
            Self::UnreadPackageMayRun => "unread-package-may-run",
            Self::InstanceFieldInitializer => "instance-field-initializer",
            Self::ParameterList => "parameter-list",
            Self::GeneratorBody => "generator-body",
            Self::AfterSuspension => "after-suspension",
            Self::NoOwnerNode => "no-owner-node",
            Self::NoEdgeIntoFunction => "no-edge-into-function",
            Self::OpenWorldExport => "open-world-export",
            Self::LeafBitPresent => "leaf-bit-present",
            Self::UnownedEntry => "unowned-entry",
            Self::EntryByReference => "entry-by-reference",
        }
    }
}

/// The group of codes of a site a silence of this pass answers: SC3001's two (a leaf owner forbids the call) or SC6002's one (an effect's apply refuses the creation).
#[derive(Clone, Copy)]
enum Group {
    Leaf,
    Effect,
}

/// Whether a generator is a layer around `span`, the innermost layer first, up to the layer whose callback runs under an owner its primitive creates (the owner is
/// current there whoever steps what is around it): the site's code is then a generator's, which runs where an iterator is stepped. B1, the re-review of #220: the
/// test asked the innermost function only, so a creation in an arrow, an IIFE or an `untrack` callback written inside a generator stepped from an apply was plain, and
/// the role engine, which places a generator's body by the call that makes the iterator, certified it.
fn generator_layer_around(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let mut layers = file.ast.functions_owning(span).collect::<Vec<_>>();
    layers.sort_by_key(|function| function.span.end - function.span.start);
    for function in layers {
        if function.generator {
            return true;
        }
        // The owner is current there whoever steps what is around it only where nothing but the primitive enters the callback (N2: a named function expression
        // that saved itself is run by whoever calls it).
        if super::owner_emission::creates_owner_for(file, function.span, lookup)
            && super::owner_emission::referenced_only_by_entry(file, function, lookup)
        {
            return false;
        }
    }
    false
}

/// Records the silences of every owner-call site of the project's files in `ends`, and the declines (proof log only) of the groups of codes none answers. `client_build` is
/// the build SC6002's creation sites are asked on ([`crate::static_api::client_build`]).
#[allow(clippy::too_many_arguments)]
pub(super) fn silences(
    files: &[&solid_facts::FileFacts],
    order: &[PathId],
    graph: &OwnerGraph,
    lookup: &SemanticLookup<'_>,
    entities: &crate::EntitySymbols,
    symbol_names: &crate::HashMap<crate::SymbolId, crate::SymbolId>,
    client_build: bool,
    ends: &mut SiteEnds,
) {
    use crate::proof_universe::{SiteCode, owner_call_codes};
    let dialect = lookup.dialect;
    let reach = crate::execution_role::SiteReach::new(lookup);
    // The files as the lookup holds them (the package hand-off question is asked of those).
    let by_path: std::collections::HashMap<&str, &solid_facts::FileFacts> = lookup
        .files()
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let unread = crate::package_hand_off::UnreadPackageHandOff::new(lookup, &by_path);
    let no_leaf_opener = !lookup.project_opens_leaf_scope();
    let caller_unplaced = |caller_file: &solid_facts::FileFacts, at: Span| {
        super::owner_emission::caller_holds_no_context(graph, &reach, lookup, caller_file, at)
    };
    let logging = crate::proof_log::enabled();
    for (listed, pid) in files.iter().zip(order) {
        if lookup.in_dependency_source(listed.path.as_str()) {
            continue;
        }
        let Some(file) = by_path.get(listed.path.as_str()).copied() else {
            continue;
        };
        let ast = &file.ast;
        // The callback positions the role engine takes as placed, for the role question (computed at the first creation that asks it).
        let mut allowed: Option<Vec<Span>> = None;
        // The sites of the file this pass answers for, as (the span the role, the entry and the graph are asked about, the span the site is reported at, its codes): the
        // calls of the owner-call universe, and (B2, the re-review of #220) its JSX creation sites.
        let mut candidates: Vec<(Span, Span, &'static [SiteCode])> = Vec::new();
        for call in &ast.calls {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let codes = owner_call_codes(dialect, file, call, primitive, client_build);
            if !codes.is_empty() {
                candidates.push((call.span, call.callee, codes));
            }
        }
        let jsx_codes = crate::proof_universe::jsx_site_codes(client_build);
        for (span, _anchor) in crate::owners::universe_jsx_sites(file, lookup) {
            candidates.push((span, span, jsx_codes));
        }
        for (span, site, codes) in candidates {
            let leaf = codes.iter().any(|code| {
                matches!(
                    code,
                    SiteCode::CleanupForbidden | SiteCode::PrimitiveForbidden
                )
            });
            let effect = codes.contains(&SiteCode::EffectCreation);
            // What a group of codes is declined for: written to the proof log only.
            let decline = |group: Group, reason: Decline| {
                if !logging {
                    return;
                }
                let names = codes
                    .iter()
                    .filter(|code| match group {
                        Group::Leaf => !matches!(code, SiteCode::EffectCreation),
                        Group::Effect => matches!(code, SiteCode::EffectCreation),
                    })
                    .map(|code| code.name())
                    .collect::<Vec<_>>();
                crate::proof_log::declined(
                    "owner-call",
                    &site.location(file.path.shared()),
                    reason.id(),
                    &names,
                );
            };
            // No execution runs the site: no code of it can be raised.
            if matches!(
                reach.site(file, span),
                crate::execution_role::Reach::Unreached(_)
            ) {
                ends.end_at(
                    file,
                    site,
                    SiteSilence::OwnerCallNeverRuns,
                    PremiseKind::SiteReach,
                    || Some(span),
                );
                continue;
            }
            // Module evaluation: one run, on a stack of its own (an instance field's initializer runs at each construction, which may be anywhere).
            let Some(function) = ast.innermost_function_owning(span) else {
                if crate::local_access::in_instance_field_initializer(file, span) {
                    for (open, group) in [(leaf, Group::Leaf), (effect, Group::Effect)] {
                        if open {
                            decline(group, Decline::InstanceFieldInitializer);
                        }
                    }
                } else {
                    ends.end_at(
                        file,
                        site,
                        SiteSilence::OwnerCallModuleScope,
                        PremiseKind::ModuleScope,
                        || Some(site),
                    );
                }
                continue;
            };
            // A package the build does not read may run the function (B3): no census and no entry the graph holds answers for it.
            if unread.may_run(file, span) {
                for (open, group) in [(leaf, Group::Leaf), (effect, Group::Effect)] {
                    if open {
                        decline(group, Decline::UnreadPackageMayRun);
                    }
                }
                continue;
            }
            // The groups of codes still open: the census answers SC3001's pair for a program with no leaf opener.
            let mut open_leaf = leaf;
            if leaf && no_leaf_opener {
                ends.end_at(
                    file,
                    site,
                    SiteSilence::LeafNoOpenerInProgram,
                    PremiseKind::ProgramCensus,
                    || Some(site),
                );
                open_leaf = false;
            }
            let mut open_effect = effect;
            if !open_leaf && !open_effect {
                continue;
            }
            // Every answer below speaks for the site only where it is plain: the graph's context and the role engine's placement say where code written
            // directly in a function runs, not a parameter list, a generator's body or a later task.
            let plain = if crate::owners::in_parameter_list(ast, span) {
                Err(Decline::ParameterList)
            } else if generator_layer_around(file, span, lookup) {
                Err(Decline::GeneratorBody)
            } else if crate::execution_role::may_follow_suspension(file, span) {
                Err(Decline::AfterSuspension)
            } else {
                Ok(())
            };
            if let Err(reason) = plain {
                for (open, group) in [(open_leaf, Group::Leaf), (open_effect, Group::Effect)] {
                    if open {
                        decline(group, reason);
                    }
                }
                continue;
            }
            // Whether a layer around the site is entered by a route the graph has no edge for, asked once and without the component-render premise
            // (ADR 0868, and the ruling: a package that renders a component is no more trusted than one that calls it).
            let mut entry_open_known: Option<bool> = None;
            let mut entry_open = || {
                *entry_open_known.get_or_insert_with(|| {
                    super::owner_emission::layer_entry_open_by_reference(
                        file,
                        span,
                        lookup,
                        caller_unplaced,
                    )
                })
            };
            // SC6002's two silences that rest on no graph context (they were `static_api`'s until the review of #220, which found each asking less than the
            // others: F2, F3). The role engine places the site off every effect's apply (a handler, a deferred callback, a directive apply, rendering, module
            // code, discarded code), and every layer around the site is entered only the way that role says; or an exact primitive's callback that creates
            // an owner wraps it.
            if open_effect {
                let allowed = allowed.get_or_insert_with(|| {
                    crate::execution_role::allowed_callback_spans(file, lookup)
                });
                if crate::static_api::runs_off_every_apply(
                    file,
                    span,
                    allowed,
                    entities,
                    symbol_names,
                    lookup,
                ) && !entry_open()
                {
                    ends.end_at(
                        file,
                        site,
                        SiteSilence::EffectCreationRunsOffApply,
                        PremiseKind::RoleDoesNotReport,
                        || Some(site),
                    );
                    open_effect = false;
                } else if let Some(wrapper) =
                    crate::static_api::owner_created_around_call(file, span, lookup)
                {
                    ends.end_at(
                        file,
                        site,
                        SiteSilence::EffectCreationOwnerSetAround,
                        PremiseKind::OwnerSetAround,
                        || Some(wrapper),
                    );
                    open_effect = false;
                }
            }
            if !open_leaf && !open_effect {
                continue;
            }
            let Some((node, context)) = graph.node_context(*pid, function.span) else {
                for (open, group) in [(open_leaf, Group::Leaf), (open_effect, Group::Effect)] {
                    if open {
                        decline(group, Decline::NoOwnerNode);
                    }
                }
                continue;
            };
            // Each group asks the context its own question.
            let leaf_answer = if !open_leaf {
                None
            } else if context == 0 {
                Some(Err(Decline::NoEdgeIntoFunction))
            } else if context & (OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED) != 0 {
                Some(Err(Decline::LeafBitPresent))
            } else if open_world_unowned_only(context, node) {
                Some(Err(Decline::OpenWorldExport))
            } else {
                Some(Ok(()))
            };
            let effect_answer = if !open_effect {
                None
            } else if context == 0 {
                Some(Err(Decline::NoEdgeIntoFunction))
            } else if context
                & !(OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED)
                != 0
            {
                Some(Err(Decline::UnownedEntry))
            } else {
                Some(Ok(()))
            };
            // The context says no entry is the forbidden one; it is every entry only where no layer around the site is entered by a route the graph has no
            // edge for.
            let entry_is_open = (matches!(leaf_answer, Some(Ok(())))
                || matches!(effect_answer, Some(Ok(()))))
                && entry_open();
            for (answer, group) in [(leaf_answer, Group::Leaf), (effect_answer, Group::Effect)] {
                let Some(answer) = answer else {
                    continue;
                };
                match answer {
                    Err(reason) => decline(group, reason),
                    Ok(()) if entry_is_open => decline(group, Decline::EntryByReference),
                    Ok(()) => match group {
                        Group::Leaf => ends.end_at(
                            file,
                            site,
                            SiteSilence::LeafOwnerAbsent,
                            PremiseKind::OwnerContextNoLeaf,
                            || Some(function.span),
                        ),
                        Group::Effect => ends.end_at(
                            file,
                            site,
                            SiteSilence::OwnerCurrent,
                            PremiseKind::OwnerContextCurrent,
                            || Some(function.span),
                        ),
                    },
                }
            }
        }
    }
}
