//! Batch 11, slice 6 (measurement stage): the owner-graph facts of the owner-call universe's sites.
//!
//! The universe ([`crate::proof_universe`], table `owner-call`) lists every call of `onCleanup` and of a creation the dialect refuses under a leaf owner
//! (SC3001: CLEANUP_IN_FORBIDDEN_SCOPE, PRIMITIVE_IN_FORBIDDEN_SCOPE) or on an effect apply's stack with no owner set (SC6002:
//! PRIMITIVE_IN_EFFECT_CALLBACK). A site no row, SC9012, discard or named silence ends gets the code's uncertifiable row. Which silences are provable is a
//! question of the owner graph: the function that holds the site has a context (the owners every entry into it gives: owned, unowned, leaf, ...), and a
//! context that is exactly owned, with every entry the graph holds, says no leaf owner runs the site and an owner is current at it.
//!
//! This pass writes nothing but the proof log's `candidate` lines (with `SOLID_CHECKER_IR_PROOF_CANDIDATES`), one per site of the universe, so the number of
//! sites each fact would end is read off before any silence is named: the function's context bits, whether it is exported or a component (proven or by
//! convention), whether the site is in a parameter list, after a suspension or in a generator, whether an entry into a layer around it is open
//! ([`super::owner_emission::named_layer_entry_open`]), whether ADR 0458's query proves it never runs, and the program's census (a leaf opener, a deferred
//! callback). A cleanup site also says what the SC4001 emission already proved at it (`ContextProof`).

use super::*;
use crate::proof_log::{self, CandidateRecord};

const fn yes(flag: bool) -> &'static str {
    if flag { "yes" } else { "no" }
}

/// The bits of a context, as the log spells them: `owned+leaf`, `unowned`, `0`.
fn bits(context: u8) -> String {
    let parts = [
        (OWNER_CONTEXT_OWNED, "owned"),
        (OWNER_CONTEXT_UNOWNED, "unowned"),
        (OWNER_CONTEXT_LEAF, "leaf"),
        (OWNER_CONTEXT_COMPONENT_UNCERTAIN, "component-uncertain"),
        (OWNER_CONTEXT_PROVEN_UNOWNED, "proven-unowned"),
        (OWNER_CONTEXT_LATER_RUN_UNOWNED, "later-run"),
        (OWNER_CONTEXT_LEAF_OWNED, "leaf-owned"),
        (OWNER_CONTEXT_MAY_RESUME_UNOWNED, "may-resume"),
    ]
    .into_iter()
    .filter(|(bit, _)| context & bit != 0)
    .map(|(_, name)| name)
    .collect::<Vec<_>>();
    if parts.is_empty() {
        "0".to_owned()
    } else {
        parts.join("+")
    }
}

/// Writes the facts of every owner-call site of the project's files (a build with the proof log and the dry run on); files no row and ends no site.
pub(super) fn measure(
    files: &[&solid_facts::FileFacts],
    order: &[PathId],
    graph: &OwnerGraph,
    emissions: &HashMap<PathId, Emission>,
    lookup: &SemanticLookup<'_>,
) {
    if !crate::proof_candidates::enabled() {
        return;
    }
    let dialect = lookup.dialect;
    let reach = crate::execution_role::SiteReach::new(lookup);
    // B3 (ADR 0967): whether a package nothing here reads may run the function holding the site: a silence that rests on a census or on the entries the graph
    // holds (a leaf opener the project names, an owner edge) cannot answer for what such a package runs.
    let by_path: std::collections::HashMap<&str, &solid_facts::FileFacts> = lookup
        .files()
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let unread = crate::package_hand_off::UnreadPackageHandOff::new(lookup, &by_path);
    let opener = lookup.project_opens_leaf_scope();
    let defers = lookup.project_defers_callbacks();
    // ADR 0868: the function making a run at `at` holds context zero (no edge reaches it) and is not proven never to run: the emission's question.
    let caller_unplaced = |caller_file: &solid_facts::FileFacts, at: Span| {
        lookup.note_file_read(caller_file);
        graph
            .path_id(caller_file.path.as_str())
            .is_some_and(|caller_pid| {
                caller_file
                    .ast
                    .innermost_function_owning(at)
                    .is_some_and(|caller| {
                        graph
                            .node_context(caller_pid, caller.span)
                            .is_some_and(|(_, context)| context == 0)
                            && reach.function_runs(caller_file, caller)
                    })
            })
    };
    for (listed, pid) in files.iter().zip(order) {
        if lookup.in_dependency_source(listed.path.as_str()) {
            continue;
        }
        // The file as the lookup holds it (the hand-off question is asked of that one).
        let Some(file) = by_path.get(listed.path.as_str()).copied() else {
            continue;
        };
        let ast = &file.ast;
        for call in &ast.calls {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let cleanup = primitive == Primitive::OnCleanup
                && crate::cleanup::leaf_forbids(dialect, file, call, primitive);
            let creation = primitive != Primitive::OnCleanup
                && crate::cleanup::creates_forbidden_primitive(dialect, file, call, primitive);
            let effect = dialect.throws_creating_in_effect_callback(
                primitive,
                crate::static_api::first_argument_is_function(call),
            );
            if !(cleanup || creation || effect) {
                continue;
            }
            let codes = [
                cleanup.then_some("cleanup"),
                creation.then_some("creation"),
                effect.then_some("effect"),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("+");
            let function = ast.innermost_function_owning(call.span);
            let node = function.and_then(|function| graph.node_context(*pid, function.span));
            let (kind, context, exported) = match (function, node) {
                (None, _) => ("module", "-".to_owned(), "-"),
                (Some(_), None) => ("no-node", "-".to_owned(), "-"),
                (Some(_), Some((node, context))) => (
                    if node.component {
                        "component"
                    } else if node.component_uncertain {
                        "component-uncertain"
                    } else if node.generator {
                        "generator"
                    } else {
                        "function"
                    },
                    bits(context),
                    yes(node.exported),
                ),
            };
            let in_list = crate::owners::in_parameter_list(ast, call.span);
            let open = function.is_some()
                && super::owner_emission::named_layer_entry_open(
                    file,
                    call.span,
                    None,
                    lookup,
                    &caller_unplaced,
                );
            let never = matches!(
                reach.site(file, call.span),
                crate::execution_role::Reach::Unreached(_)
            );
            // What the SC4001 emission proved at the site, for an `onCleanup` call (its operation span is the callee).
            let proof = emissions
                .get(pid)
                .and_then(|emission| {
                    emission
                        .accounted
                        .iter()
                        .find(|(span, _)| *span == call.callee)
                })
                .map_or_else(|| "none".to_owned(), |(_, proof)| format!("{proof:?}"));
            proof_log::candidate(&CandidateRecord {
                rule: "SC3001",
                table: "owner-call-ctx",
                path: file.path.as_str(),
                at: call.callee,
                alternates: &[call.span],
                facts: &[
                    ("prim", &format!("{primitive:?}")),
                    ("codes", &codes),
                    ("fn", kind),
                    ("ctx", &context),
                    ("exported", exported),
                    ("param", yes(in_list)),
                    (
                        "await",
                        yes(crate::execution_role::may_follow_suspension(file, call.span)),
                    ),
                    ("entry-open", yes(open)),
                    ("never", yes(never)),
                    ("unread-pkg", yes(unread.may_run(file, call.span))),
                    ("opener", yes(opener)),
                    ("defers", yes(defers)),
                    ("proof", &proof),
                ],
            });
        }
    }
}
