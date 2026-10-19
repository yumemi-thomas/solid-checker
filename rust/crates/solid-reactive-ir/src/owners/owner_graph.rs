//! Owner graph construction: owner nodes, context seeding and propagation, requirement candidates, and leaf gates.

use super::*;

/// One owner-graph node per function, from the binding-aware name: an arrow
/// bound to `const Foo = ...` carries the same identity as `function Foo()`.
/// `symbol` is how call edges reach the node, while the semantic component
/// model and named-export status seed its context. Both passes must derive
/// binding identity from this one lookup or fresh and incremental builds
/// disagree on arrow-bound functions.
pub(super) fn owner_node(
    file: &solid_facts::FileFacts,
    indexes: &ProjectIndexes<'_>,
    entities: &EntitySymbols,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> OwnerNode {
    let name = function_binding_name(file, function);
    let symbol = name.and_then(|name| {
        entities
            .get(&location(file.path.shared(), name.span))
            .cloned()
    });
    let exported = indexes
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.functions.iter().any(|candidate| {
                candidate.exported
                    && candidate.body.start_byte == u64::from(function.body.start)
                    && candidate.body.end_byte == u64::from(function.body.end)
            })
        })
        || file.ast.exports.iter().any(|export| {
            export.span.contains(function.span)
                && !file.ast.functions.iter().any(|candidate| {
                    candidate.span != function.span
                        && export.span.contains(candidate.span)
                        && candidate.span.contains(function.span)
                })
        });
    let component_status = lookup.function_component_status(file, function);
    let component = component_status == crate::indexes::ComponentStatus::Proven;
    let component_uncertain = component_status == crate::indexes::ComponentStatus::Uncertain;
    let mut seed_context = compiler_owner_context(file, function.body);
    // ADR 0319's walk, in its owner mode, proved that every invocation of
    // this literal is a host event dispatch (a callback a helper calls from
    // the handler it returns, which the caller spreads onto an element): it
    // runs with no owner, as the same literal written on the element does.
    if lookup
        .unowned_event_literals()
        .contains(&(file.path.to_string(), function.span))
    {
        seed_context |= OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED;
    }
    match component_status {
        crate::indexes::ComponentStatus::Proven => seed_context |= OWNER_CONTEXT_OWNED,
        crate::indexes::ComponentStatus::Uncertain => {
            seed_context |=
                OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_COMPONENT_UNCERTAIN;
        }
        // An exported non-component helper may be called by code this build
        // cannot see, and such a caller supplies no owner -- so the node is
        // seeded unowned and every owner question inside it stays a proof
        // obligation. That seed *is* the open-world assumption: when the user
        // asserts the analyzed files are the whole program, the enumerated
        // call sites are the complete caller set and the seed would assert a
        // caller that does not exist. Nothing else changes; a call site the
        // analysis cannot classify still contributes its own uncertainty.
        crate::indexes::ComponentStatus::No
            if exported && name.is_some() && !lookup.program_closed =>
        {
            seed_context |= OWNER_CONTEXT_UNOWNED;
        }
        crate::indexes::ComponentStatus::No => {}
    }
    OwnerNode {
        path: file.path.to_string(),
        span: function.span,
        body: function.body,
        symbol,
        exported,
        component,
        component_uncertain,
        seed_context,
    }
}

/// The owner context bits an enclosing compiler ownership region proves for a
/// function body.
///
pub(super) fn compiler_owner_context(file: &solid_facts::FileFacts, body: Span) -> u8 {
    file.compiler
        .ownership_regions
        .iter()
        .filter(|region| region.span.contains(body))
        .fold(0, |context, region| {
            context
                | match region.kind {
                    solid_facts::compiler::OwnershipRegionKind::Owned => OWNER_CONTEXT_OWNED,
                    solid_facts::compiler::OwnershipRegionKind::Unowned => {
                        OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED
                    }
                    solid_facts::compiler::OwnershipRegionKind::Leaf => OWNER_CONTEXT_LEAF,
                    solid_facts::compiler::OwnershipRegionKind::Unknown => 0,
                }
        })
}

/// The context bits proved at the facts seam before graph propagation.
pub(super) fn seed_contexts(nodes: &[OwnerNode]) -> Vec<u8> {
    nodes.iter().map(|node| node.seed_context).collect()
}

/// Worklist fixed point over the owner graph: every context bit flows along
/// outgoing edges, transformed per edge kind, until nothing changes.
pub(super) fn propagate_owner_contexts(
    contexts: &mut [u8],
    outgoing: &[Vec<(usize, OwnerEdgeKind)>],
) {
    let mut queued = contexts
        .iter()
        .map(|context| *context != 0)
        .collect::<Vec<_>>();
    let mut worklist = queued
        .iter()
        .enumerate()
        .filter_map(|(index, queued)| queued.then_some(index))
        .collect::<VecDeque<_>>();
    while let Some(source) = worklist.pop_front() {
        queued[source] = false;
        for (target, kind) in outgoing[source].iter().copied() {
            let propagated = owner_edge_context(kind, contexts[source]);
            let next = contexts[target] | propagated;
            if next != contexts[target] {
                contexts[target] = next;
                if !queued[target] {
                    queued[target] = true;
                    worklist.push_back(target);
                }
            }
        }
    }
}

#[derive(Clone)]
pub(crate) enum OwnerTarget {
    Symbol(SymbolId),
    LocalSpan(Span),
    Function(String, Span),
}

#[derive(Clone)]
pub(crate) struct SymbolicOwnerEdge {
    pub(crate) source: Option<OwnerSource>,
    pub(crate) target: OwnerTarget,
    pub(crate) kind: OwnerEdgeKind,
}

#[derive(Clone)]
pub(crate) struct OwnerSource {
    pub(crate) path: String,
    pub(crate) span: Span,
}

#[derive(Clone)]
pub(crate) struct OwnerRequirementCandidate {
    pub(crate) operation: &'static str,
    pub(crate) operation_span: Span,
    pub(crate) owner: Option<Span>,
    pub(crate) report_mask: u8,
    pub(crate) allow_uncertain: bool,
    pub(crate) runtime_uncertain: bool,
    /// ADR 0161: the site is a call of a contract export, and whether that
    /// export registers on every call.
    pub(crate) through_contract: bool,
    pub(crate) registration_uncertain: bool,
    pub(crate) settled_target: Option<OwnerTarget>,
    /// For call-site-gated leaf owners (2.0 `onSettled`): the owner call's
    /// span, published as a [`LeafGateDecision`] once the graph settles so
    /// the leaf-operation table can be resolved against real ownership.
    pub(crate) settled_gate: Option<Span>,
    /// [`providing_region_chain`] for the operation, keyed by function span:
    /// `None` outside every owner-providing region. Whether the region answers
    /// it depends on those functions' propagated owner contexts, which a
    /// per-file fragment does not have yet; see [`root_owned_at`].
    pub(crate) providing_region_chain: Option<Vec<Span>>,
}

/// Whether a call-site-gated leaf owner (`onSettled`) actually materializes
/// as a leaf owner, resolved against the propagated owner graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LeafGateDecision {
    /// The call runs under a live children-capable owner: the callback is a
    /// leaf owner and the leaf-scope rules apply.
    Owned,
    /// The call runs out-of-band (unowned, event handler, inside a leaf):
    /// the runtime enqueues a plain callback and none of the leaf-scope
    /// throw sites exist — drop the operations.
    OutOfBand,
    /// The call site's ownership cannot be proven (exported helper,
    /// conditional owner): keep the operations, projected as uncertifiable.
    Uncertain,
}

/// Gate decisions keyed by the owner call's location bytes.
pub(crate) type SettledGateDecisions = HashMap<(String, u64, u64), LeafGateDecision>;

pub(super) fn leaf_gate_decision(
    owner_index: Option<usize>,
    nodes: &[OwnerNode],
    contexts: &[u8],
) -> LeafGateDecision {
    let Some(index) = owner_index else {
        // Module evaluation: no owner can be live, so the call is
        // out-of-band by construction.
        return LeafGateDecision::OutOfBand;
    };
    let context = contexts[index];
    let conditional = context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
        == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
    // The same escalation the owner requirements use: an exported
    // non-component helper has callers the analysis cannot see, so assuming
    // either owned or out-of-band would be a guess.
    let exported_unproven =
        nodes[index].exported && context & OWNER_CONTEXT_UNOWNED != 0 && !nodes[index].component;
    if conditional || exported_unproven {
        LeafGateDecision::Uncertain
    } else if context & OWNER_CONTEXT_OWNED != 0 {
        LeafGateDecision::Owned
    } else {
        LeafGateDecision::OutOfBand
    }
}

/// Resolves the call-site gates recorded by the leaf-and-cleanup stage: an
/// out-of-band `onSettled` sheds its leaf operations, an unprovable one keeps
/// them as uncertifiable, and everything else stays a proven violation. A
/// gate with no decision sits inside an owner-providing region the graph
/// never doubts (directly under `createRoot`), which is owned.
pub(crate) fn apply_settled_gates(
    operations: &mut Vec<crate::LeafOwnerOperation>,
    decisions: &SettledGateDecisions,
) {
    operations.retain_mut(|operation| {
        let Some(gate) = operation.call_site_gate.as_ref() else {
            return true;
        };
        match decisions.get(&(gate.path.to_string(), gate.start_byte, gate.end_byte)) {
            None | Some(LeafGateDecision::Owned) => true,
            Some(LeafGateDecision::OutOfBand) => false,
            Some(LeafGateDecision::Uncertain) => {
                operation.uncertain = true;
                true
            }
        }
    });
}

/// The owner pass sees `onCleanup` as a normal owner requirement while the
/// leaf pass sees the same call as SC3001. For an owned inline `onSettled`
/// callback, keep the leaf finding and remove only the duplicate SC4001
/// requirement. Out-of-band and uncertain gates remain conservative.
///
/// Only a function literal written *directly* in the owner's argument is that
/// callback: `onSettled(wrap(() => { onCleanup(dispose); }))` hands the arrow
/// to an opaque wrapper that may run it out-of-band, where the cleanup really
/// is unowned, so it keeps its SC4001.
pub(super) fn apply_settled_requirement_gates(
    lookup: &SemanticLookup<'_>,
    requirements: &mut Vec<OwnerRequirement>,
    decisions: &SettledGateDecisions,
) {
    if requirements.is_empty() {
        return;
    }
    // Resolved once per owned gate rather than once per requirement, and
    // through the path and span indexes rather than linear file/call scans.
    let owned_callbacks = decisions
        .iter()
        .filter(|(_, decision)| **decision == LeafGateDecision::Owned)
        .filter_map(|((path, start, end), _)| {
            let file = lookup.file_by_path(path)?;
            let span = Span::new(u32::try_from(*start).ok()?, u32::try_from(*end).ok()?);
            let argument = file.ast.call_at(span)?.arguments.first()?;
            let callback = crate::cleanup::callback_argument_literal(file, argument.span)?;
            Some((file, callback))
        })
        .collect::<Vec<_>>();
    if owned_callbacks.is_empty() {
        return;
    }
    requirements.retain(|requirement| {
        if requirement.operation != OwnerRequirementOperation::Cleanup {
            return true;
        }
        let Ok(start) = u32::try_from(requirement.location.start_byte) else {
            return true;
        };
        let Ok(end) = u32::try_from(requirement.location.end_byte) else {
            return true;
        };
        let cleanup = Span::new(start, end);
        !owned_callbacks.iter().any(|(file, callback)| {
            requirement.location.path.as_ref() == file.path.as_str()
                && callback.body.contains(cleanup)
                && containing_ast_function(&file.ast, cleanup)
                    .is_some_and(|function| function.span == callback.span)
        })
    });
}

#[derive(Clone, Copy)]
pub(crate) struct OwnerRequirementStatus {
    pub(crate) uncertain: bool,
    /// ADR 0161: the site is a call of a contract export (`through_contract`).
    pub(crate) through_contract: bool,
    pub(crate) runtime_uncertain: bool,
    pub(crate) caller_uncertain: bool,
    pub(crate) conditional_owner: bool,
    pub(crate) later_run_unowned: bool,
    pub(crate) component_uncertain: bool,
    /// The operation runs after an `await` ([`AwaitContinuations`]).
    pub(crate) after_await: bool,
    pub(crate) report: bool,
}

pub(crate) struct CachedOwnerFile {
    pub(crate) source_hash: SourceHash,
    /// See [`SemanticLookup::cross_file_proof_digest`]. Owner seeds and edges
    /// can depend on component or callback uses in another file.
    pub(crate) cross_file_proofs: Option<CrossFileProofDigest>,
    pub(crate) compiler: Arc<solid_facts::compiler::ExecutionMap>,
    pub(crate) nodes: Vec<OwnerNode>,
    pub(crate) edges: Vec<SymbolicOwnerEdge>,
    pub(crate) requirements: Vec<OwnerRequirementCandidate>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct OwnerIncrementalTimings {
    pub(crate) fragment_build: Duration,
    pub(crate) graph_assembly: Duration,
    pub(crate) propagation: Duration,
    pub(crate) requirement_emission: Duration,
    pub(crate) reused_files: u64,
    pub(crate) recomputed_files: u64,
}
