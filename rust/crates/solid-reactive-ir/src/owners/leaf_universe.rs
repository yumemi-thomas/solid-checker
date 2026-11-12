//! Batch 11, slice 6: the owner-graph and census silences of the owner-call universe.
//!
//! The universe ([`crate::proof_universe`], table `owner-call`) lists every call of `onCleanup` and of a creation the dialect refuses under a leaf owner
//! (SC3001: CLEANUP_IN_FORBIDDEN_SCOPE, PRIMITIVE_IN_FORBIDDEN_SCOPE) or on an effect apply's stack with no owner set (SC6002: PRIMITIVE_IN_EFFECT_CALLBACK).
//! A site no row, SC9012, discard or named silence ends gets the code's own uncertifiable row ([`crate::site_accounting`]). The silences named here rest on
//! the owner graph, the one analysis that holds every entry into a function and the owner each gives, and on one census:
//!
//! - [`SiteSilence::OwnerCallNeverRuns`]: ADR 0458's query proves no execution runs the site (every code);
//! - [`SiteSilence::OwnerCallModuleScope`]: no function owns the site and no instance field's initializer holds it, so module evaluation runs it on a
//!   stack of its own: in no leaf owner's callback and on no effect's apply (every code);
//! - [`SiteSilence::LeafNoOpenerInProgram`]: the project names no primitive that opens a leaf scope, so there is no leaf owner to forbid the call
//!   (SC3001's two codes);
//! - [`SiteSilence::LeafOwnerAbsent`]: the context of the function that holds the site has an edge, no leaf bit, and is not the bare open-world seed of an
//!   export (SC3001's two codes);
//! - [`SiteSilence::OwnerCurrent`]: the same context gives an owner on every entry (owned, a leaf owner, or both: no unowned bit), so the effect callback's
//!   guard (`inEffectCallback && context === null`) does not meet the creation (SC6002).
//!
//! The graph's contexts are the entries it holds, so the last two answer only where those are every entry the program can make, and each rests on all of
//! these facts of the site (a missing one is no silence, the site keeps its uncertifiable row):
//!
//! - the site is written directly in a function (not a parameter list, which runs when its argument is omitted; not after a suspension of an async function,
//!   which runs from a later task; not in a generator, whose body runs where an iterator is stepped);
//! - no layer around it is entered by a route the graph has no edge for ([`super::owner_emission::named_layer_entry_open`], ADR 0868: a function stored,
//!   assigned, handed on or a method), a reference that is not a direct call or an inline run;
//! - no package the build does not read may run the function ([`crate::package_hand_off::UnreadPackageHandOff`], ADR 0967): such a package may call it
//!   under a leaf owner, or on an effect's apply, through an entry no edge shows. The census silence rests on this too: a package may open the leaf scope the
//!   project's own primitives do not.
//!
//! Nothing here ever files a row; a silence names the fact it rests on ([`PremiseKind`], each with the span of the code it is about).

use super::*;
use crate::proof_log::PremiseKind;
use crate::site_accounting::{SiteEnds, SiteSilence};

/// Records the silences of every owner-call site of the project's files in `ends`. `client_build` is the build SC6002's creation sites are asked on
/// ([`crate::static_api::client_build`]).
pub(super) fn silences(
    files: &[&solid_facts::FileFacts],
    order: &[PathId],
    graph: &OwnerGraph,
    lookup: &SemanticLookup<'_>,
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
    for (listed, pid) in files.iter().zip(order) {
        if lookup.in_dependency_source(listed.path.as_str()) {
            continue;
        }
        let Some(file) = by_path.get(listed.path.as_str()).copied() else {
            continue;
        };
        let ast = &file.ast;
        for call in &ast.calls {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let codes = owner_call_codes(dialect, file, call, primitive, client_build);
            if codes.is_empty() {
                continue;
            }
            let leaf = codes.iter().any(|code| {
                matches!(
                    code,
                    SiteCode::CleanupForbidden | SiteCode::PrimitiveForbidden
                )
            });
            let effect = codes.contains(&SiteCode::EffectCreation);
            let site = call.callee;
            // No execution runs the site: no code of it can be raised.
            if matches!(
                reach.site(file, call.span),
                crate::execution_role::Reach::Unreached(_)
            ) {
                ends.end_at(
                    file,
                    site,
                    SiteSilence::OwnerCallNeverRuns,
                    PremiseKind::SiteReach,
                    || Some(call.span),
                );
                continue;
            }
            // Module evaluation: one run, on a stack of its own (an instance field's initializer runs at each construction, which may be anywhere).
            let Some(function) = ast.innermost_function_owning(call.span) else {
                if !crate::local_access::in_instance_field_initializer(file, call.span) {
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
            if unread.may_run(file, call.span) {
                continue;
            }
            if leaf && no_leaf_opener {
                ends.end_at(
                    file,
                    site,
                    SiteSilence::LeafNoOpenerInProgram,
                    PremiseKind::ProgramCensus,
                    || Some(site),
                );
                continue;
            }
            // The graph's context answers for the site only where the site is plain and no entry into a layer around it is one the graph has no edge for.
            if crate::owners::in_parameter_list(ast, call.span)
                || function.generator
                || crate::execution_role::may_follow_suspension(file, call.span)
            {
                continue;
            }
            let Some((node, context)) = graph.node_context(*pid, function.span) else {
                continue;
            };
            let leaf_free = context != 0
                && context & (OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED) == 0
                && !open_world_unowned_only(context, node);
            let owner_current = context != 0
                && context & !(OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED)
                    == 0;
            if !((leaf && leaf_free) || (effect && owner_current)) {
                continue;
            }
            if super::owner_emission::named_layer_entry_open(
                file,
                call.span,
                None,
                lookup,
                &caller_unplaced,
            ) {
                continue;
            }
            if leaf && leaf_free {
                ends.end_at(
                    file,
                    site,
                    SiteSilence::LeafOwnerAbsent,
                    PremiseKind::OwnerContextNoLeaf,
                    || Some(function.span),
                );
            }
            if effect && owner_current {
                ends.end_at(
                    file,
                    site,
                    SiteSilence::OwnerCurrent,
                    PremiseKind::OwnerContextCurrent,
                    || Some(function.span),
                );
            }
        }
    }
}
