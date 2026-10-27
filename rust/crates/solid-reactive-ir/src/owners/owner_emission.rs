//! Per-file requirement emission over the kept owner graph (S7f).
//!
//! A file's requirements are a function of its candidates (the fragment), the
//! final contexts of its own functions, the file's `await` continuations, the
//! lookup answers it read (recorded: they are folded into the fragment's
//! proof, so a moved answer reruns the fragment and with it the emission), and
//! two answers kept beside the rows because a file's reuse cannot see them
//! move:
//!
//! - the closed provider census's verdict for each context a `useContext`
//!   read names ([`Emission::provider`]), and
//! - for an `onSettled` cleanup, the callback it resolves to and that file's
//!   source hash ([`Emission::callbacks`]): the cleanup proof reads the
//!   callback's body. It also reads that file's entity rows, which another
//!   file's edit can move under an unchanged text, so the emission records
//!   the callback file as read (`note_file_read`).
//!
//! The dedup of `push_owner_requirement` keys on the requirement's path, so a
//! file's emission is the same whichever other files are emitted around it,
//! and the project's requirements are the files' in path order.

use super::owner_state::{NodeId, OwnerGraph, PathId};
use super::*;
use typefacts::Location;

/// One file's emitted requirements and what they were emitted under.
#[derive(Default)]
pub(crate) struct Emission {
    pub(super) requirements: Vec<OwnerRequirement>,
    pub(super) gates: Vec<((String, u64, u64), LeafGateDecision)>,
    /// The provider-free verdict of each context a `useContext` read named.
    pub(super) provider: Vec<(Location, bool)>,
    /// Each `onSettled` cleanup's resolved callback node and its file's hash.
    pub(super) callbacks: Vec<(OwnerTarget, Option<NodeId>, Option<SourceHash>)>,
}

pub(super) struct EmissionInputs<'a, 'f, 'i> {
    pub(super) facts: &'a ProjectFacts,
    pub(super) lookup: &'a SemanticLookup<'f>,
    pub(super) indexes: &'a ProjectIndexes<'i>,
    pub(super) provider_free: &'a HashSet<Location>,
}

impl EmissionInputs<'_, '_, '_> {
    /// Whether the answers `emission` was made under still hold.
    pub(super) fn holds(&self, graph: &OwnerGraph, pid: PathId, emission: &Emission) -> bool {
        emission
            .provider
            .iter()
            .all(|(key, free)| self.provider_free.contains(key) == *free)
            && emission.callbacks.iter().all(|(target, node, hash)| {
                graph.resolve_target(pid, target) == *node
                    && self.callback_hash(graph, *node) == *hash
            })
    }

    /// The source hash of the file holding the callback `node`.
    fn callback_hash(&self, graph: &OwnerGraph, node: Option<NodeId>) -> Option<SourceHash> {
        let node = graph.node(node?)?;
        let file = self.indexes.files_by_path.get(node.path.as_str())?;
        Some(file.source_hash.clone())
    }
}

/// The starts of the calls in `path` that run after an `await` on every path
/// through their async function's own body (the producer's
/// `calls_after_await` dominance fact, the one `reactive-read-after-await`
/// proves reads with): branches, `&&`, `try`/`catch`, loops and `switch` merge
/// conservatively, and a nested closure is never scanned. Such a call runs
/// from a promise continuation on an empty stack, where the dialect's
/// fresh-stack answer says whether an owner is current; Solid 2.0's owner is
/// a synchronous dynamic scope, so none is. Empty for a dialect that does not
/// answer [`solid_dialect::CallbackOwner::None`]. A file's own set: the
/// producer files each fact under the path of its call.
fn await_starts(inputs: &EmissionInputs<'_, '_, '_>, path: &str) -> HashSet<u64> {
    let mut starts = HashSet::default();
    if inputs.lookup.dialect.fresh_stack_callback_owner()
        == Some(solid_dialect::CallbackOwner::None)
        && let Some(file) = inputs.facts.typescript.file(path)
    {
        for function in file.async_functions.iter() {
            for call in &function.calls_after_await {
                if call.path.as_ref() == path {
                    starts.insert(call.start_byte);
                }
            }
        }
    }
    starts
}

pub(super) fn emit_file(
    inputs: &EmissionInputs<'_, '_, '_>,
    graph: &OwnerGraph,
    pid: PathId,
    file: &solid_facts::FileFacts,
) -> Emission {
    let lookup = inputs.lookup;
    let mut emission = Emission::default();
    let Some(entry) = graph.file(pid) else {
        return emission;
    };
    let path = file.path.as_str();
    let starts = await_starts(inputs, path);
    let mut seen = HashSet::default();
    for candidate in &entry.fragment.requirements {
        let owner = candidate
            .owner
            .and_then(|span| graph.node_context(pid, span));
        let mut context = owner.map_or(OWNER_CONTEXT_UNOWNED, |(_, context)| context);
        // A function no owner edge reaches holds context zero: its body never
        // runs from syntax. Its parameter list is no such claim (ADR 0454).
        if candidate.in_parameter_list && context == 0 {
            context = OWNER_CONTEXT_UNOWNED;
        }
        // As in the fragment: effects, cleanups and context reads, never a
        // JSX boundary or `onSettled`'s returned cleanup. A context read after
        // an `await` is unowned (ADR 0352).
        let after_await = matches!(candidate.operation, "effect" | "cleanup" | "context-read")
            && starts.contains(&u64::from(candidate.operation_span.start));
        let possible_continuation = (!after_await
            && matches!(candidate.operation, "effect" | "cleanup" | "context-read"))
        .then(|| possible_await_continuation_context(file, candidate.operation_span, context))
        .flatten();
        let context = if after_await {
            await_continuation_context(context)
        } else {
            possible_continuation.unwrap_or(context)
        };
        // A read of a provider-free context under an owner throws
        // `ContextNotFoundError` (ADR 0352).
        let provider_free_read = candidate.operation == "context-read"
            && candidate.context_argument.is_some_and(|argument| {
                context_key_at(lookup, file, argument).is_some_and(|key| {
                    let free = inputs.provider_free.contains(&key);
                    emission.provider.push((key, free));
                    free
                })
            });
        // Every chain span is one of this file's own nodes; one that did not
        // resolve would withhold the lexical answer, not grant it.
        let root_owned = possible_continuation.is_none()
            && !candidate.in_parameter_list
            && root_owned_at(candidate.providing_region_chain.as_deref(), |span| {
                graph
                    .context(pid, *span)
                    .unwrap_or(OWNER_CONTEXT_LEAVES_REGION)
            });
        // A providing region owns its reads, so a provider-free read there is
        // a provider finding, not an owner question (ADR 0352).
        let provider = provider_free_read && (root_owned || owned_on_every_path(context));
        if root_owned && !provider {
            continue;
        }
        let (operation, report_hit) = if provider {
            ("context-provider", true)
        } else {
            (candidate.operation, context & candidate.report_mask != 0)
        };
        if let Some(gate) = candidate.settled_gate {
            emission.gates.push((
                (path.to_owned(), u64::from(gate.start), u64::from(gate.end)),
                leaf_gate_decision(owner),
            ));
        }
        let mut cleanup_proof = None;
        if candidate.operation == "settled-cleanup" {
            let target = candidate
                .settled_target
                .as_ref()
                .and_then(|target| graph.resolve_target(pid, target));
            cleanup_proof = target.and_then(|target| {
                let node = graph.node(target)?;
                let callback_file = inputs.indexes.files_by_path.get(node.path.as_str())?;
                lookup.note_file_read(callback_file);
                let callback = callback_file.ast.function_at(node.span)?;
                Some(function_cleanup_return_proof(
                    lookup,
                    callback_file,
                    callback,
                ))
            });
            if let Some(symbolic) = &candidate.settled_target {
                emission.callbacks.push((
                    symbolic.clone(),
                    target,
                    inputs.callback_hash(graph, target),
                ));
            }
            let settled_call_valid = file.ast.calls.iter().any(|call| {
                call.arguments
                    .first()
                    .is_some_and(|argument| argument.span == candidate.operation_span)
                    && lookup
                        .resolved_callee_call(file, call.callee)
                        .is_some_and(|resolved| {
                            resolved.validity == typefacts::ResolvedCallValidity::Valid
                        })
            });
            match cleanup_proof.unwrap_or(CleanupReturnProof::Unresolved) {
                CleanupReturnProof::NoFunction => continue,
                CleanupReturnProof::Function => {}
                CleanupReturnProof::OptionalFunction => {
                    // The callback may return a cleanup. Preserve that as
                    // runtime uncertainty instead of certifying it absent.
                }
                CleanupReturnProof::Unresolved if settled_call_valid => {}
                CleanupReturnProof::Unresolved => continue,
            }
        }
        let proven_unowned = context & OWNER_CONTEXT_PROVEN_UNOWNED != 0;
        let component_uncertain = !proven_unowned
            && (context & OWNER_CONTEXT_COMPONENT_UNCERTAIN != 0
                || owner.is_some_and(|(node, _)| node.component_uncertain));
        let later_run_unowned = !component_uncertain
            && !proven_unowned
            && context & OWNER_CONTEXT_LATER_RUN_UNOWNED != 0;
        let conditional_owner = !component_uncertain
            && !proven_unowned
            && !later_run_unowned
            && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF_OWNED) != 0
            && context & OWNER_CONTEXT_UNOWNED != 0;
        let caller_uncertain = candidate.allow_uncertain
            && !proven_unowned
            && owner.is_some_and(|(node, node_context)| {
                (node.exported
                    && node_context & OWNER_CONTEXT_UNOWNED != 0
                    && !node.component
                    && !node.component_uncertain)
                    || open_world_unowned_only(context, node)
            });
        let cleanup_return_uncertain = candidate.operation == "settled-cleanup"
            && cleanup_proof.is_none_or(|proof| {
                matches!(
                    proof,
                    CleanupReturnProof::OptionalFunction | CleanupReturnProof::Unresolved
                )
            });
        let runtime_uncertain =
            candidate.runtime_uncertain || cleanup_return_uncertain || candidate.in_parameter_list;
        let uncertain = runtime_uncertain
            || conditional_owner
            || later_run_unowned
            || caller_uncertain
            || component_uncertain;
        push_owner_requirement(
            &mut emission.requirements,
            &mut seen,
            operation,
            file,
            candidate.operation_span,
            OwnerRequirementStatus {
                uncertain,
                runtime_uncertain,
                caller_uncertain,
                conditional_owner,
                later_run_unowned,
                component_uncertain,
                after_await,
                // A cleanup in a leaf that is not owned is reported; in an
                // owned leaf it is SC3001's claim (ADR 0347 amendment).
                report: report_hit
                    || (candidate.operation == "cleanup"
                        && context & OWNER_CONTEXT_LEAF != 0
                        && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF_OWNED) == 0),
            },
            lookup,
        );
    }
    emission
}
