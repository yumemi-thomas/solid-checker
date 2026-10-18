//! Owner analysis: which computations need a reactive owner, where
//! owners are provided, and the requirement/fix emission around them.

use crate::cache::{CachedLateStages, same_compiler_semantics};
use crate::pipeline::{AnalysisContext, ProgramDraft, parallel_slice_results};
use crate::{
    BuildTimings, ExecutionRole, Fix, FunctionBoundary, OwnerRequirement,
    OwnerRequirementOperation, PrimitiveName, TextEdit, call_primitive_name,
    containing_function_indexed, function_indices_by_path, jsx_primitive_name, known_primitive,
    location,
};

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use crate::cleanup::{CleanupReturnProof, function_cleanup_return_proof};
use crate::effect_api::ProofStatus;
use crate::execution_role::{argument_references_callback_symbol, execution_role, function_symbol};
use crate::identity::SymbolId;
use crate::indexes::{CrossFileProofDigest, EntitySymbols, ProjectIndexes, SemanticLookup};
use solid_dialect::{Dialect, OwnerRequirementRole, Primitive};
use solid_facts::ProjectFacts;
use solid_facts::core::{SourceHash, SourcePath, Span};

/// Runs the project-level owner fixed point, including its retained-cache
/// policy, and appends the resulting requirements to the draft.
pub(crate) fn collect_project(
    ctx: &AnalysisContext<'_>,
    project_indexes: &ProjectIndexes<'_>,
    retained_source_paths: &HashSet<String>,
    cache: Option<&mut CachedLateStages>,
    reusable: bool,
    draft: &mut ProgramDraft,
    timings: &mut BuildTimings,
) {
    let cached = reusable
        .then(|| {
            cache.as_ref().and_then(|cache| {
                cache
                    .missing_owners
                    .clone()
                    .zip(cache.settled_gates.clone())
            })
        })
        .flatten();
    if let Some((cached, gates)) = cached {
        // `cache.missing_owners` is stored *after* the requirement gates ran,
        // so the cached vector is already gated. Re-running them here would be
        // pure repeated work on the fast path.
        draft.missing_owners = cached;
        apply_settled_gates(&mut draft.leaf_operations, &gates);
        timings.owner_fixed_point_reused = true;
        timings.owner_reused_files = u64::try_from(ctx.facts.files.len()).unwrap_or(u64::MAX);
        return;
    }
    if let Some(cache) = cache {
        let (requirements, gates, owner_timings) = find_missing_owners_incremental(
            ctx.facts,
            ctx.semantic_lookup,
            project_indexes,
            ctx.symbol_names,
            retained_source_paths,
            &mut cache.owner_files,
        );
        draft.missing_owners.extend(requirements);
        apply_settled_gates(&mut draft.leaf_operations, &gates);
        apply_settled_requirement_gates(ctx.semantic_lookup, &mut draft.missing_owners, &gates);
        timings.absorb_owner(&owner_timings);
        cache.missing_owners = Some(draft.missing_owners.clone());
        cache.settled_gates = Some(gates);
    } else {
        let (requirements, gates) = find_missing_owners(
            ctx.facts,
            ctx.semantic_lookup,
            project_indexes,
            ctx.symbol_names,
        );
        draft.missing_owners.extend(requirements);
        apply_settled_gates(&mut draft.leaf_operations, &gates);
        apply_settled_requirement_gates(ctx.semantic_lookup, &mut draft.missing_owners, &gates);
        timings.owner_recomputed_files = u64::try_from(ctx.facts.files.len()).unwrap_or(u64::MAX);
    }
}

pub(crate) fn component_props_parameter_fix(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    parameter: &solid_facts::ast::BindingFact,
    entities: &EntitySymbols,
) -> Option<Fix> {
    let pattern_start = usize::try_from(parameter.pattern.start).ok()?;
    let pattern_end = usize::try_from(parameter.pattern.end).ok()?;
    let pattern = file.source.get(pattern_start..pattern_end)?;
    if !pattern.starts_with('{') || !pattern.ends_with('}') || parameter.names.is_empty() {
        return None;
    }
    let mut cursor = pattern_start + 1;
    for name in &parameter.names {
        let start = usize::try_from(name.span.start).ok()?;
        let end = usize::try_from(name.span.end).ok()?;
        if start < cursor || end > pattern_end {
            return None;
        }
        if !file.source.as_bytes()[cursor..start]
            .iter()
            .all(|byte| byte.is_ascii_whitespace() || *byte == b',')
            || file.source.get(start..end)? != file.source_text(name.span)?
        {
            return None;
        }
        cursor = end;
    }
    if !file.source.as_bytes()[cursor..pattern_end - 1]
        .iter()
        .all(|byte| byte.is_ascii_whitespace() || *byte == b',')
    {
        return None;
    }

    let used_names = file
        .ast
        .identifiers
        .iter()
        .filter(|identifier| function.body.contains(identifier.span))
        .filter_map(|identifier| file.source_text(identifier.span))
        .collect::<HashSet<_>>();
    let parameter_name = (1..)
        .map(|suffix| {
            if suffix == 1 {
                "props".into()
            } else {
                format!("props{suffix}")
            }
        })
        .find(|candidate| !used_names.contains(candidate.as_str()))?;
    let mut edits = vec![TextEdit {
        location: location(file.path.shared(), parameter.pattern),
        new_text: parameter_name.clone(),
    }];
    let mut body_references = 0;
    for name in &parameter.names {
        let declaration = location(file.path.shared(), name.span);
        let symbol = entities.get(&declaration)?;
        let symbol = facts
            .typescript
            .symbols()
            .find(|candidate| candidate.id() == symbol.as_str())?;
        for reference in symbol.references() {
            if reference.path != file.path.as_str().into() {
                return None;
            }
            let span = Span::new(
                u32::try_from(reference.start_byte).ok()?,
                u32::try_from(reference.end_byte).ok()?,
            );
            if parameter.pattern.contains(span) {
                continue;
            }
            if !function.body.contains(span)
                || (!matches!(
                    execution_role(&file.compiler, span, &[]),
                    ExecutionRole::TrackedJsx
                ) && !file.compiler.jsx_operations.iter().any(|operation| {
                    operation.kind == "jsx-expression" && operation.span.contains(span)
                }))
            {
                return None;
            }
            let start = usize::try_from(reference.start_byte).ok()?;
            let end = usize::try_from(reference.end_byte).ok()?;
            if file.source.get(start..end)? != file.source_text(name.span)? {
                return None;
            }
            body_references += 1;
            edits.push(TextEdit {
                location: reference.clone(),
                new_text: format!(
                    "{parameter_name}.{}",
                    file.source_text(name.span).unwrap_or_default()
                ),
            });
        }
    }
    if body_references == 0 {
        return None;
    }
    edits.sort_by_key(|edit| edit.location.start_byte);
    Some(Fix {
        message: "Keep component props reactive: read via props.<name> instead of destructuring"
            .into(),
        applicability: "safe".into(),
        edits,
    })
}

pub(crate) fn containing_ast_function(
    ast: &solid_facts::ast::AstFacts,
    span: Span,
) -> Option<&solid_facts::ast::FunctionFact> {
    ast.functions_body_containing(span)
        .min_by_key(|function| function.body.end - function.body.start)
}

/// Whether `span` is written directly in `function`'s own body: inside the
/// body, and inside no other function written there -- its body *or its
/// parameter list*. A nested function's default parameter lies in the outer
/// body but outside every nested body, so [`containing_ast_function`] alone
/// names the outer function for it; that code runs only when the nested
/// function is called (ADR 0204).
pub(crate) fn written_directly_in(
    ast: &solid_facts::ast::AstFacts,
    function: &solid_facts::ast::FunctionFact,
    span: Span,
) -> bool {
    function.body.contains(span)
        && !ast
            .functions_within(function.body)
            .any(|nested| nested.span != function.span && nested.span.contains(span))
}

pub(crate) const OWNER_CONTEXT_OWNED: u8 = 1;
pub(crate) const OWNER_CONTEXT_UNOWNED: u8 = 2;
pub(crate) const OWNER_CONTEXT_LEAF: u8 = 4;
/// The owned/unowned split originates in a function whose Solid component
/// identity is only conventional. Preserve this provenance through inheriting
/// callback edges so diagnostics do not misdescribe it as nullable
/// `runWithOwner` state.
pub(crate) const OWNER_CONTEXT_COMPONENT_UNCERTAIN: u8 = 8;
/// At least one concrete execution path is proved unowned. This differs from
/// the possible-unowned half of a conditional owner or ambiguous component:
/// one proven unowned invocation is enough to prove an ownership defect even
/// when other invocations may be owned.
pub(crate) const OWNER_CONTEXT_PROVEN_UNOWNED: u8 = 16;
/// The possible-unowned half of this context comes from a callback that runs
/// under its caller's owner only on its first run
/// ([`solid_dialect::CallbackOwner::InheritsFirstRun`]): a later run, from the
/// scheduler's flush, runs with no owner. Kept as its own bit so the finding
/// names that reason instead of a nullable `runWithOwner` owner.
pub(crate) const OWNER_CONTEXT_LATER_RUN_UNOWNED: u8 = 32;

/// Whether a node's unowned context is *only* the open-world assumption an
/// exported non-component entry seeds ([`owner_node`]), carried to this node
/// along call edges.
///
/// The seed sets [`OWNER_CONTEXT_UNOWNED`] without [`OWNER_CONTEXT_PROVEN_UNOWNED`]
/// -- "an unseen caller may supply no owner" -- and `Preserve` edges forward
/// the bit unchanged, so a private helper called only from exported functions
/// inherits it. Every other source of the unowned bit sets a companion bit (a
/// proven unowned edge sets the proof, a conditional edge sets owned, a
/// first-run edge sets later-run), so the bare bit names the open world and
/// nothing else. A requirement under it is a caller obligation exactly like
/// the same call written inside the exported function itself, and is no more a
/// proven violation one call-edge away.
const fn open_world_unowned_only(context: u8, node: &OwnerNode) -> bool {
    !node.component
        && !node.component_uncertain
        && context
            & (OWNER_CONTEXT_OWNED
                | OWNER_CONTEXT_UNOWNED
                | OWNER_CONTEXT_PROVEN_UNOWNED
                | OWNER_CONTEXT_LATER_RUN_UNOWNED
                | OWNER_CONTEXT_LEAF)
            == OWNER_CONTEXT_UNOWNED
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerEdgeKind {
    Preserve,
    Owned,
    Unowned,
    Conditional,
    /// The source's context on the callback's first run, and possibly no
    /// owner on a later one.
    InheritsFirstRun,
    Leaf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReturnedCallbackInvocationSite {
    pub(crate) path: String,
    pub(crate) span: Span,
    pub(crate) inherited_execution: Option<solid_dialect::Execution>,
    pub(crate) inherited_owner: OwnerEdgeKind,
}

impl ReturnedCallbackInvocationSite {
    /// Total order over every field, for sort-then-dedup. A key that omitted
    /// a field could leave equal sites non-adjacent, and `dedup` only removes
    /// adjacent duplicates.
    ///
    /// Borrows the path. `sort_by_key` calls its key function on every
    /// comparison, so cloning the path here allocated a `String` per comparison
    /// -- `O(n log n)` allocations to order a list whose elements already own
    /// their paths.
    pub(crate) fn order_key(&self) -> (&str, Span, u8, Option<u8>) {
        (
            self.path.as_str(),
            self.span,
            self.inherited_owner as u8,
            self.inherited_execution.map(|execution| execution as u8),
        )
    }
}

#[derive(Clone)]
pub(crate) struct OwnerCallbackEdge {
    pub(crate) argument: usize,
    pub(crate) kind: OwnerEdgeKind,
    pub(crate) source_path: String,
    pub(crate) source: Span,
}

#[derive(Clone)]
pub(crate) struct OwnerNode {
    pub(crate) path: String,
    pub(crate) span: Span,
    pub(crate) body: Span,
    pub(crate) symbol: Option<SymbolId>,
    pub(crate) exported: bool,
    pub(crate) component: bool,
    pub(crate) component_uncertain: bool,
    pub(crate) seed_context: u8,
}

impl FunctionBoundary for OwnerNode {
    fn path(&self) -> &str {
        &self.path
    }

    fn body(&self) -> Span {
        self.body
    }
}

pub(crate) struct OwnerFileIndex {
    pub(crate) call_primitives: Vec<Option<PrimitiveName>>,
    pub(crate) providing_regions: Vec<Span>,
}

impl OwnerFileIndex {
    /// The per-call primitive resolutions and owner-providing argument spans
    /// a file's owner analysis classifies calls against — one derivation for
    /// both the fresh and incremental passes.
    pub(crate) fn for_file(
        file: &solid_facts::FileFacts,
        entities: &EntitySymbols,
        symbol_names: &HashMap<SymbolId, SymbolId>,
        lookup: &SemanticLookup<'_>,
    ) -> Self {
        let call_primitives = file
            .ast
            .calls
            .iter()
            .map(|call| call_primitive_name(file, call, entities, symbol_names, lookup.dialect))
            .collect::<Vec<_>>();
        let providing_regions = file
            .ast
            .calls
            .iter()
            .zip(&call_primitives)
            .filter_map(|(call, primitive)| {
                let argument =
                    owner_providing_argument(file, call, known_primitive(primitive), lookup)?;
                call.arguments.get(argument).and_then(|argument| {
                    matches!(
                        argument.value,
                        solid_facts::ast::ArgumentValueKind::Identifier
                            | solid_facts::ast::ArgumentValueKind::Function
                            | solid_facts::ast::ArgumentValueKind::AsyncFunction
                    )
                    .then_some(argument.span)
                })
            })
            .chain(
                file.compiler
                    .ownership_regions
                    .iter()
                    .filter(|region| {
                        region.kind == solid_facts::compiler::OwnershipRegionKind::Owned
                    })
                    .map(|region| region.span),
            )
            .collect();
        Self {
            call_primitives,
            providing_regions,
        }
    }
}

/// One owner-graph node per function, from the binding-aware name: an arrow
/// bound to `const Foo = ...` carries the same identity as `function Foo()`.
/// `symbol` is how call edges reach the node, while the semantic component
/// model and named-export status seed its context. Both passes must derive
/// binding identity from this one lookup or fresh and incremental builds
/// disagree on arrow-bound functions.
fn owner_node(
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
fn compiler_owner_context(file: &solid_facts::FileFacts, body: Span) -> u8 {
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
fn seed_contexts(nodes: &[OwnerNode]) -> Vec<u8> {
    nodes.iter().map(|node| node.seed_context).collect()
}

/// Worklist fixed point over the owner graph: every context bit flows along
/// outgoing edges, transformed per edge kind, until nothing changes.
fn propagate_owner_contexts(contexts: &mut [u8], outgoing: &[Vec<(usize, OwnerEdgeKind)>]) {
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

fn leaf_gate_decision(
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
fn apply_settled_requirement_gates(
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

pub(crate) fn find_missing_owners(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    indexes: &ProjectIndexes<'_>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> (Vec<OwnerRequirement>, SettledGateDecisions) {
    let entities = lookup.entities();
    let owner_file_indexes = facts
        .files
        .iter()
        .map(|file| OwnerFileIndex::for_file(file, entities, symbol_names, lookup))
        .collect::<Vec<_>>();
    let mut nodes = Vec::new();
    for file in &facts.files {
        for function in &file.ast.functions {
            nodes.push(owner_node(file, indexes, entities, function, lookup));
        }
    }
    let nodes_by_path = function_indices_by_path(&nodes);
    let by_symbol = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.symbol.clone().map(|symbol| (symbol, index)))
        .collect::<HashMap<_, _>>();
    let mut contexts = seed_contexts(&nodes);
    let mut edges = Vec::<(usize, usize, OwnerEdgeKind)>::new();
    let mut requirements = Vec::new();
    let mut seen = HashSet::new();
    for (file_index, file) in facts.files.iter().enumerate() {
        for (call_index, call) in file.ast.calls.iter().enumerate() {
            let owner =
                containing_function_indexed(&nodes, &nodes_by_path, file.path.as_str(), call.span);
            if let Some(target_index) = lookup
                .object_member_function(file, call.callee)
                .and_then(|(file, function)| {
                    nodes.iter().position(|node| {
                        node.path == file.path.as_str() && node.span == function.span
                    })
                })
                .or_else(|| {
                    call_edge_symbol(file, call, entities, lookup)
                        .and_then(|symbol| by_symbol.get(symbol))
                        .copied()
                })
            {
                if let Some(owner) = owner {
                    edges.push((owner, target_index, OwnerEdgeKind::Preserve));
                } else {
                    contexts[target_index] |= OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED;
                }
            }
            for edge in owner_callback_edges(
                file,
                call,
                &owner_file_indexes[file_index].call_primitives[call_index],
                lookup,
            ) {
                let Some(argument) = call.arguments.get(edge.argument) else {
                    continue;
                };
                let Some(target_index) = owner_callback_index(
                    &nodes,
                    &nodes_by_path,
                    &by_symbol,
                    file,
                    argument.span,
                    entities,
                ) else {
                    continue;
                };
                let invocation_owner = containing_function_indexed(
                    &nodes,
                    &nodes_by_path,
                    &edge.source_path,
                    edge.source,
                );
                if let Some(owner) = invocation_owner {
                    edges.push((owner, target_index, edge.kind));
                } else {
                    contexts[target_index] |= owner_edge_context(
                        edge.kind,
                        OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
                    );
                }
            }
        }
        for callback in &file.compiler.callback_roles {
            if !matches!(
                callback.role,
                solid_facts::compiler::CallbackRoleKind::EventHandler
                    | solid_facts::compiler::CallbackRoleKind::DirectiveApply
            ) || (callback.role == solid_facts::compiler::CallbackRoleKind::DirectiveApply
                && !ref_application_on_intrinsic(file, callback.span))
            {
                continue;
            }
            if let Some(index) = owner_callback_index(
                &nodes,
                &nodes_by_path,
                &by_symbol,
                file,
                callback.span,
                entities,
            ) {
                contexts[index] |= OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED;
            }
        }
    }
    let mut outgoing = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
    for (source, target, kind) in edges {
        outgoing[source].push((target, kind));
    }
    propagate_owner_contexts(&mut contexts, &outgoing);

    let continuations = AwaitContinuations::new(facts, lookup);
    let mut settled_gates = SettledGateDecisions::new();
    for (file_index, file) in facts.files.iter().enumerate() {
        let file_nodes = nodes_by_path
            .get(file.path.as_str())
            .map_or(&[][..], Vec::as_slice);
        let root_owned_span = |span: Span| {
            root_owned_at(
                providing_region_chain(
                    &owner_file_indexes[file_index].providing_regions,
                    file_nodes
                        .iter()
                        .map(|index| (*index, nodes[*index].span, nodes[*index].body)),
                    span,
                )
                .as_deref(),
                |index| contexts[*index],
            )
        };
        for (call_index, call) in file.ast.calls.iter().enumerate() {
            let primitive =
                known_primitive(&owner_file_indexes[file_index].call_primitives[call_index]);
            let context = owner_context_at(
                &nodes,
                &nodes_by_path,
                &contexts,
                file.path.as_str(),
                call.span,
            );
            // `onSettled`'s returned cleanup is a different owner question
            // (whether an unowned settle registers at all), so it keeps the
            // enclosing context.
            let after_await = continuations.contains(file.path.as_str(), call.callee.start)
                && !matches!(
                    primitive
                        .and_then(|primitive| lookup.dialect.owner_requirement_role(primitive)),
                    Some(OwnerRequirementRole::SettledCleanup)
                );
            let context = if after_await {
                await_continuation_context(context)
            } else {
                context
            };
            let root_owned = root_owned_span(call.span);
            if !root_owned
                && primitive.is_some_and(|primitive| {
                    lookup
                        .dialect
                        .leaf_owner_requires_owned_call_site(primitive)
                })
            {
                settled_gates.insert(
                    (
                        file.path.to_string(),
                        u64::from(call.span.start),
                        u64::from(call.span.end),
                    ),
                    leaf_gate_decision(
                        containing_function_indexed(
                            &nodes,
                            &nodes_by_path,
                            file.path.as_str(),
                            call.span,
                        ),
                        &nodes,
                        &contexts,
                    ),
                );
            }
            // Which primitives carry which owner-requirement role is the
            // dialect's answer (`owner_requirement_role`), not a set spelled
            // out here. The two owner passes used to spell it out separately
            // and drifted: this one omitted `createRenderEffect`, so a render
            // effect outside any owner leaked with nothing reported.
            let role =
                primitive.and_then(|primitive| lookup.dialect.owner_requirement_role(primitive));
            let operation = match (role, primitive) {
                (Some(OwnerRequirementRole::Effect), Some(primitive)) if !root_owned => {
                    let registration =
                        crate::effect_api::classify_effect_call(file, call, primitive, lookup)
                            .owner_registration;
                    (registration != ProofStatus::No).then_some((
                        "effect",
                        context & OWNER_CONTEXT_UNOWNED != 0,
                        registration == ProofStatus::Uncertain,
                    ))
                }
                (Some(OwnerRequirementRole::Cleanup), _) if !root_owned => Some((
                    "cleanup",
                    context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                    false,
                )),
                (Some(OwnerRequirementRole::SettledCleanup), _) if !root_owned => {
                    let proof = call
                        .arguments
                        .first()
                        .and_then(|argument| {
                            owner_callback_index(
                                &nodes,
                                &nodes_by_path,
                                &by_symbol,
                                file,
                                argument.span,
                                entities,
                            )
                        })
                        .and_then(|index| {
                            let node = &nodes[index];
                            let callback_file = facts
                                .files
                                .iter()
                                .find(|candidate| candidate.path.as_str() == node.path)?;
                            let callback = callback_file
                                .ast
                                .functions
                                .iter()
                                .find(|candidate| candidate.span == node.span)?;
                            Some(function_cleanup_return_proof(
                                lookup,
                                callback_file,
                                callback,
                            ))
                        })
                        .unwrap_or(CleanupReturnProof::Unresolved);
                    match proof {
                        CleanupReturnProof::Function => Some((
                            "settled-cleanup",
                            context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                            false,
                        )),
                        CleanupReturnProof::OptionalFunction => Some((
                            "settled-cleanup",
                            context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                            true,
                        )),
                        CleanupReturnProof::Unresolved
                            if lookup.resolved_callee_call(file, call.callee).is_some_and(
                                |resolved| {
                                    resolved.validity == typefacts::ResolvedCallValidity::Valid
                                },
                            ) =>
                        {
                            Some((
                                "settled-cleanup",
                                context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                                true,
                            ))
                        }
                        CleanupReturnProof::Unresolved => None,
                        CleanupReturnProof::NoFunction => None,
                    }
                }
                _ => None,
            };
            if let Some((operation, report, runtime_uncertain)) = operation {
                let owner_index = containing_function_indexed(
                    &nodes,
                    &nodes_by_path,
                    file.path.as_str(),
                    call.span,
                );
                let proven_unowned = context & OWNER_CONTEXT_PROVEN_UNOWNED != 0;
                let component_uncertain = !proven_unowned
                    && (context & OWNER_CONTEXT_COMPONENT_UNCERTAIN != 0
                        || owner_index.is_some_and(|index| nodes[index].component_uncertain));
                let later_run_unowned = !component_uncertain
                    && !proven_unowned
                    && context & OWNER_CONTEXT_LATER_RUN_UNOWNED != 0;
                let conditional_owner = !component_uncertain
                    && !proven_unowned
                    && !later_run_unowned
                    && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                        == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
                let caller_uncertain = !proven_unowned
                    && owner_index.is_some_and(|index| {
                        (nodes[index].exported
                            && contexts[index] & OWNER_CONTEXT_UNOWNED != 0
                            && !nodes[index].component
                            && !nodes[index].component_uncertain)
                            || open_world_unowned_only(context, &nodes[index])
                    });
                let uncertain = runtime_uncertain
                    || conditional_owner
                    || later_run_unowned
                    || caller_uncertain
                    || component_uncertain;
                let operation_span = if operation == "settled-cleanup" {
                    call.arguments
                        .first()
                        .map_or(call.callee, |argument| argument.span)
                } else {
                    call.callee
                };
                push_owner_requirement(
                    &mut requirements,
                    &mut seen,
                    operation,
                    file,
                    operation_span,
                    OwnerRequirementStatus {
                        through_contract: false,
                        uncertain,
                        runtime_uncertain,
                        caller_uncertain,
                        conditional_owner,
                        later_run_unowned,
                        component_uncertain,
                        after_await,
                        report,
                    },
                    lookup,
                );
            }
        }
        for element in &file.ast.jsx_elements {
            let boundary =
                jsx_primitive_name(file, element, entities, symbol_names, lookup.dialect);
            if !boundary
                .as_deref()
                .is_some_and(|tag| lookup.dialect.is_async_boundary(tag))
            {
                continue;
            }
            let context = owner_context_at(
                &nodes,
                &nodes_by_path,
                &contexts,
                file.path.as_str(),
                element.span,
            );
            if root_owned_span(element.span) {
                continue;
            }
            let proven_unowned = context & OWNER_CONTEXT_PROVEN_UNOWNED != 0;
            let component_uncertain = !proven_unowned
                && (context & OWNER_CONTEXT_COMPONENT_UNCERTAIN != 0
                    || containing_function_indexed(
                        &nodes,
                        &nodes_by_path,
                        file.path.as_str(),
                        element.span,
                    )
                    .is_some_and(|index| nodes[index].component_uncertain));
            let later_run_unowned = !component_uncertain
                && !proven_unowned
                && context & OWNER_CONTEXT_LATER_RUN_UNOWNED != 0;
            let conditional_owner = !component_uncertain
                && !proven_unowned
                && !later_run_unowned
                && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                    == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
            push_owner_requirement(
                &mut requirements,
                &mut seen,
                "boundary",
                file,
                Span::new(element.span.start, element.name.span.end),
                OwnerRequirementStatus {
                    through_contract: false,
                    uncertain: conditional_owner || later_run_unowned || component_uncertain,
                    runtime_uncertain: false,
                    caller_uncertain: false,
                    conditional_owner,
                    later_run_unowned,
                    component_uncertain,
                    after_await: false,
                    report: context & OWNER_CONTEXT_UNOWNED != 0,
                },
                lookup,
            );
        }
    }
    attribute_dependency_requirements(
        &mut requirements,
        facts,
        lookup,
        &nodes,
        &contexts,
        &outgoing,
    );
    (requirements, settled_gates)
}

/// The function symbol a call runs, for an owner edge. An entity at a member
/// callee's composite span can name its receiver (`f.reset()`, `f.bind(…)`
/// carry `f`'s symbol), and such a call does not run the receiver
/// (ADR 0290 review round 1): it gets no edge, unless the member is the
/// standard library's `call` or `apply`, which run it before they return.
fn call_edge_symbol<'e>(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &'e EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> Option<&'e SymbolId> {
    let symbol = entities.get(&location(file.path.shared(), call.callee))?;
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let receiver = file
        .ast
        .members
        .iter()
        .find(|member| member.span == callee)
        .map(|member| file.ast.peel_ts_sugar_span(member.object));
    if receiver
        .is_some_and(|object| entities.get(&location(file.path.shared(), object)) == Some(symbol))
    {
        let invokes_receiver = lookup
            .resolved_callee_call(file, call.callee)
            .and_then(|resolved| resolved.declaration.as_ref())
            .is_some_and(|declaration| {
                declaration.standard_library
                    && matches!(
                        declaration.qualified_name.as_ref(),
                        "Function.call"
                            | "Function.apply"
                            | "CallableFunction.call"
                            | "CallableFunction.apply"
                            | "NewableFunction.call"
                            | "NewableFunction.apply"
                    )
            });
        return invokes_receiver.then_some(symbol);
    }
    Some(symbol)
}

/// ADR 0290: report an owner requirement located in a package analyzed as
/// source (ADR 0284) at the project's call that runs it there.
///
/// A requirement inside a package is reported because some owner context
/// reached its function; when that context came from a project caller, the
/// defect is the project's call, and dropping it with the package's own
/// findings would leave the project silent. The walk goes back from the
/// operation's function along the edges that carry the reporting bit, through
/// package functions only, and stops at:
///
/// - an edge from a project function: each call written directly in it whose
///   callee is the package function it enters becomes a requirement at that
///   call;
/// - an edge a package function creates the bit on itself (it schedules the
///   callback): that context is the package's own, not a caller's.
///
/// Only a requirement proven unowned (`OWNER_CONTEXT_PROVEN_UNOWNED`, not
/// uncertain) walks the proven bit and can stay a violation at the call, and
/// only when the call is on its project function's every-completion path:
/// that function's unowned context reaches a call behind a branch or an owner
/// guard on some invocations only. Every other one walks the plain unowned
/// bit and becomes an obligation there: the call may run it unowned.
///
/// An operation after an `await` inside the package is unowned whatever the
/// caller did, so it walks every edge. A project call at module scope has no
/// edge; it is found by scanning and attributed as an obligation. A bit
/// seeded inside the package (its own event handlers or module scope) has no
/// project call and is not attributed.
pub(crate) fn attribute_dependency_requirements(
    requirements: &mut Vec<OwnerRequirement>,
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    nodes: &[OwnerNode],
    contexts: &[u8],
    outgoing: &[Vec<(usize, OwnerEdgeKind)>],
) {
    if facts.dependency_source_roots.is_empty() {
        return;
    }
    let mut incoming = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
    for (source, edges) in outgoing.iter().enumerate() {
        for &(target, kind) in edges {
            incoming[target].push((source, kind));
        }
    }
    let in_dependency = |index: usize| facts.dependency_source_root(&nodes[index].path).is_some();
    let mut seen = requirements
        .iter()
        .map(|requirement| {
            (
                requirement.location.path.to_string(),
                requirement.location.start_byte,
                requirement.location.end_byte,
                operation_name(requirement.operation).to_owned(),
            )
        })
        .collect::<HashSet<_>>();
    let mut attributed = Vec::new();
    for requirement in requirements.iter() {
        if !requirement.report
            || requirement.through_dependency.is_some()
            || facts
                .dependency_source_root(&requirement.location.path)
                .is_none()
        {
            continue;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(requirement.location.start_byte),
            u32::try_from(requirement.location.end_byte),
        ) else {
            continue;
        };
        let span = Span::new(start, end);
        let Some(owner) = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                *node.path == *requirement.location.path && node.body.contains(span)
            })
            .min_by_key(|(_, node)| node.body.end - node.body.start)
            .map(|(index, _)| index)
        else {
            continue;
        };
        // After an `await` the operation is proven unowned on its own; its
        // function's entry context does not matter.
        let proven = !requirement.uncertain
            && (requirement.after_await || contexts[owner] & OWNER_CONTEXT_PROVEN_UNOWNED != 0);
        let mask = if proven {
            OWNER_CONTEXT_PROVEN_UNOWNED
        } else {
            OWNER_CONTEXT_UNOWNED
        };
        let Some(owner_file) = lookup.file_by_path(&nodes[owner].path) else {
            continue;
        };
        // ADR 0275 at the call boundary: the operation counts for a caller
        // only on its function's every-completion path, and each hop's call
        // on its caller's. The operation's own site is its call.
        let operation_site = owner_file
            .ast
            .calls
            .iter()
            .find(|call| call.callee == span || call.span == span)
            .map(|call| call.span);
        // Per package function on a path to the operation: the site in it
        // that leads there, whether that site is on the function's
        // every-completion path, and whether every hop below it is.
        // Per package function on a path to the operation: the sites in it
        // that lead there, whether one of them is on the function's
        // every-completion path, and whether every hop below it is.
        let mut best = HashMap::<usize, (Vec<Span>, bool, bool)>::new();
        best.insert(
            owner,
            (
                vec![operation_site.unwrap_or(span)],
                operation_site.is_some_and(|site| {
                    crate::interproc::site_on_every_completion(
                        lookup,
                        owner_file,
                        nodes[owner].span,
                        site,
                    )
                }),
                true,
            ),
        );
        // An operation after an `await` in the package runs unowned whatever
        // context its function was entered with (review round 1 of ADR 0290,
        // Sol): every project call that runs it is attributed.
        let carries = |kind: OwnerEdgeKind, caller: usize| {
            requirement.after_await || owner_edge_context(kind, contexts[caller]) & mask != 0
        };
        let enters =
            |file: &solid_facts::FileFacts, call: &solid_facts::ast::CallFact, node: usize| {
                // An exact invocation of the function: not a member of it
                // (`f.reset()`, `f.bind(…)`) nor a replaced binding.
                crate::execution_role::project_callee_at_parameter(file, call, 0, lookup)
                    .is_some_and(|(callee_file, callee, exact)| {
                        exact
                            && *callee_file.path.as_str() == *nodes[node].path
                            && callee.span == nodes[node].span
                    })
            };
        // ADR 0280 on the first hop: what the call's arguments decide about
        // the callee's sites. Reached when one site is guaranteed, dropped
        // when every site is unreachable, possible otherwise.
        let first_hop = |file: &solid_facts::FileFacts,
                         call: &solid_facts::ast::CallFact,
                         node_file: &'_ solid_facts::FileFacts,
                         node_function: &solid_facts::ast::FunctionFact,
                         sites: &[Span]| {
            crate::argument_values::parameter_values(file, call, node_file, node_function, lookup)
                .filter(|values| !values.is_empty())
                .and_then(|values| {
                    solid_facts::ast::argument_site_standings(
                        std::path::Path::new(node_file.path.as_str()),
                        &node_file.source,
                        node_function.span,
                        &values,
                        &lookup.never_returning_calls_within(node_file, node_function),
                        sites,
                    )
                })
                .map(|standings| {
                    if standings.contains(&solid_facts::ast::SiteStanding::Guaranteed) {
                        solid_facts::ast::SiteStanding::Guaranteed
                    } else if standings
                        .iter()
                        .all(|standing| *standing == solid_facts::ast::SiteStanding::Unreachable)
                    {
                        solid_facts::ast::SiteStanding::Unreachable
                    } else {
                        solid_facts::ast::SiteStanding::Possible
                    }
                })
        };
        let mut queue = VecDeque::from([owner]);
        while let Some(node) = queue.pop_front() {
            let (sites, own, below) = best[&node].clone();
            let Some(node_file) = lookup.file_by_path(&nodes[node].path) else {
                continue;
            };
            let Some(node_function) = node_file
                .ast
                .functions
                .iter()
                .find(|function| function.span == nodes[node].span)
            else {
                continue;
            };
            for &(caller, kind) in &incoming[node] {
                if !carries(kind, caller) {
                    continue;
                }
                let created = owner_edge_context(kind, 0) & mask != 0;
                let Some(file) = lookup.file_by_path(&nodes[caller].path) else {
                    continue;
                };
                // Only calls written directly in the caller that enter this
                // function: a callback edge names no call to report at.
                let calls = file
                    .ast
                    .calls
                    .iter()
                    .filter(|call| {
                        containing_ast_function(&file.ast, call.span)
                            .is_some_and(|function| function.span == nodes[caller].span)
                            && enters(file, call, node)
                    })
                    .collect::<Vec<_>>();
                if calls.is_empty() {
                    continue;
                }
                if in_dependency(caller) {
                    if created {
                        continue;
                    }
                    let completing = calls.iter().any(|call| {
                        crate::interproc::site_on_every_completion(
                            lookup,
                            file,
                            nodes[caller].span,
                            call.span,
                        )
                    });
                    let entry = (
                        calls.iter().map(|call| call.span).collect::<Vec<_>>(),
                        completing,
                        own && below,
                    );
                    let improves = best
                        .get(&caller)
                        .is_none_or(|previous| !(previous.1 && previous.2) && entry.1 && entry.2);
                    if improves {
                        best.insert(caller, entry);
                        queue.push_back(caller);
                    }
                    continue;
                }
                // The project caller creates the bit itself (it schedules the
                // package function, `setTimeout(f)`): the unowned run is that
                // reference's, not a direct call's (review round 1).
                if created && !requirement.after_await {
                    continue;
                }
                for call in calls {
                    let reached = match first_hop(file, call, node_file, node_function, &sites) {
                        Some(solid_facts::ast::SiteStanding::Unreachable) => continue,
                        Some(solid_facts::ast::SiteStanding::Guaranteed) => below,
                        Some(solid_facts::ast::SiteStanding::Possible) => false,
                        None => own && below,
                    };
                    // The unowned context is the project function's; it
                    // reaches this call only when the call is on that
                    // function's every-completion path (review round 1, Sol):
                    // a call behind a branch or an owner guard may run only on
                    // owned invocations.
                    let call_completes =
                        frontier_call_completes(lookup, file, nodes[caller].span, call.span);
                    attributed.push((
                        file,
                        call.span,
                        requirement.clone(),
                        proven && reached && call_completes,
                    ));
                }
            }
        }
        // A project call at module scope has no owner and no function node,
        // so no edge records it. It is attributed as an obligation: whether a
        // module-scope statement runs is not proven here.
        for &node in best.keys() {
            let (sites, _, _) = &best[&node];
            let Some(node_file) = lookup.file_by_path(&nodes[node].path) else {
                continue;
            };
            let Some(node_function) = node_file
                .ast
                .functions
                .iter()
                .find(|function| function.span == nodes[node].span)
            else {
                continue;
            };
            for file in &facts.files {
                if facts.dependency_source_root(file.path.as_str()).is_some() {
                    continue;
                }
                for call in &file.ast.calls {
                    if containing_ast_function(&file.ast, call.span).is_some()
                        || !enters(file, call, node)
                        || first_hop(file, call, node_file, node_function, sites)
                            == Some(solid_facts::ast::SiteStanding::Unreachable)
                    {
                        continue;
                    }
                    attributed.push((file, call.span, requirement.clone(), false));
                }
            }
        }
    }
    for (file, span, requirement, proven) in attributed {
        let before = requirements.len();
        push_owner_requirement(
            requirements,
            &mut seen,
            operation_name(requirement.operation),
            file,
            span,
            OwnerRequirementStatus {
                through_contract: false,
                uncertain: !proven,
                runtime_uncertain: requirement.runtime_uncertain,
                caller_uncertain: requirement.caller_uncertain,
                conditional_owner: requirement.conditional_owner,
                later_run_unowned: requirement.later_run_unowned,
                component_uncertain: requirement.component_uncertain,
                after_await: false,
                report: true,
            },
            lookup,
        );
        if requirements.len() > before
            && let Some(pushed) = requirements.last_mut()
        {
            pushed.through_dependency = Some(requirement.location.clone());
        }
    }
}

/// Whether `site`, a call written directly in the project function at
/// `function_span`, runs on that function's every completion (ADR 0276's
/// completion sites, minus an earlier `never` guard). Like
/// `interproc::site_on_every_completion`, except that only JSX, a class or a
/// pattern written inside this function defers the site: an inline handler
/// sits inside its component's JSX, and that JSX does not defer its own body.
fn frontier_call_completes(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function_span: Span,
    site: Span,
) -> bool {
    let Some(function) = file
        .ast
        .functions
        .iter()
        .find(|function| function.span == function_span)
    else {
        return false;
    };
    let inside = |span: Span| function.body.contains(span) && span.contains(site);
    file.ast.completion_sites.binary_search(&site).is_ok()
        && !function.generator
        && written_directly_in(&file.ast, function, site)
        && !file
            .ast
            .jsx_elements
            .iter()
            .any(|element| inside(element.span))
        && !file.ast.classes.iter().any(|class| inside(class.span))
        && !file
            .ast
            .bindings
            .iter()
            .any(|binding| inside(binding.pattern))
        && !file
            .ast
            .assignments
            .iter()
            .any(|assignment| inside(assignment.target))
        && !lookup.never_returning_call_before(file, function, site)
}

const fn operation_name(operation: OwnerRequirementOperation) -> &'static str {
    match operation {
        OwnerRequirementOperation::Effect => "effect",
        OwnerRequirementOperation::Cleanup => "cleanup",
        OwnerRequirementOperation::Boundary => "boundary",
        OwnerRequirementOperation::SettledCleanup => "settled-cleanup",
    }
}

pub(crate) fn discover_owner_file(
    file: &solid_facts::FileFacts,
    indexes: &ProjectIndexes<'_>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> CachedOwnerFile {
    let dialect = lookup.dialect;
    let OwnerFileIndex {
        call_primitives,
        providing_regions,
    } = OwnerFileIndex::for_file(file, entities, symbol_names, lookup);
    let nodes = file
        .ast
        .functions
        .iter()
        .map(|function| owner_node(file, indexes, entities, function, lookup))
        .collect::<Vec<_>>();
    let nodes_by_path = function_indices_by_path(&nodes);
    let owner_at = |span| {
        containing_function_indexed(&nodes, &nodes_by_path, file.path.as_str(), span)
            .map(|index| nodes[index].span)
    };
    // Keyed by function span: the emission pass resolves it to the node whose
    // propagated context it reads, as it does `owner`.
    let region_chain = |span| {
        providing_region_chain(
            &providing_regions,
            nodes.iter().map(|node| (node.span, node.span, node.body)),
            span,
        )
    };
    let callback_target = |argument: Span| {
        nodes_by_path
            .get(file.path.as_str())
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| argument.contains(nodes[*index].span))
            .max_by_key(|index| nodes[*index].span.end - nodes[*index].span.start)
            .map(|index| OwnerTarget::LocalSpan(nodes[index].span))
            .or_else(|| {
                entities
                    .get(&location(file.path.shared(), argument))
                    .cloned()
                    .map(OwnerTarget::Symbol)
            })
    };
    let mut edges = Vec::new();
    let mut requirements = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let owner = owner_at(call.span);
        let target = lookup
            .object_member_function(file, call.callee)
            .map(|(file, function)| OwnerTarget::Function(file.path.to_string(), function.span))
            .or_else(|| {
                call_edge_symbol(file, call, entities, lookup)
                    .cloned()
                    .map(OwnerTarget::Symbol)
            });
        if let Some(target) = target {
            edges.push(SymbolicOwnerEdge {
                source: owner.map(|span| OwnerSource {
                    path: file.path.to_string(),
                    span,
                }),
                target,
                kind: OwnerEdgeKind::Preserve,
            });
        }
        for edge in owner_callback_edges(file, call, &call_primitives[call_index], lookup) {
            if let Some(target) = call
                .arguments
                .get(edge.argument)
                .and_then(|argument| callback_target(argument.span))
            {
                let source = lookup
                    .files()
                    .iter()
                    .find(|candidate| candidate.path.as_str() == edge.source_path)
                    .and_then(|source_file| {
                        containing_ast_function(&source_file.ast, edge.source).map(|function| {
                            OwnerSource {
                                path: edge.source_path.clone(),
                                span: function.span,
                            }
                        })
                    });
                edges.push(SymbolicOwnerEdge {
                    source,
                    target,
                    kind: edge.kind,
                });
            }
        }
        // Not skipped here: whether the region answers the operation needs the
        // propagated context, so the emission pass decides (`root_owned_at`).
        let providing_region_chain = region_chain(call.span);
        // Same seam as the batch owner pass, for the same reason: these two
        // matched the effect set independently and disagreed about
        // `createRenderEffect`.
        let known = known_primitive(&call_primitives[call_index]);
        let role = known.and_then(|primitive| lookup.dialect.owner_requirement_role(primitive));
        let operation = match (role, known) {
            (Some(OwnerRequirementRole::Effect), Some(primitive)) => {
                let registration =
                    crate::effect_api::classify_effect_call(file, call, primitive, lookup)
                        .owner_registration;
                (registration != ProofStatus::No).then_some((
                    "effect",
                    OWNER_CONTEXT_UNOWNED,
                    None,
                    call.callee,
                    registration == ProofStatus::Uncertain,
                ))
            }
            (Some(OwnerRequirementRole::Cleanup), _) => Some((
                "cleanup",
                OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF,
                None,
                call.callee,
                false,
            )),
            (Some(OwnerRequirementRole::SettledCleanup), _) => Some((
                "settled-cleanup",
                OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF,
                call.arguments
                    .first()
                    .and_then(|argument| callback_target(argument.span)),
                call.arguments
                    .first()
                    .map_or(call.callee, |argument| argument.span),
                false,
            )),
            _ => None,
        };
        if let Some((operation, report_mask, settled_target, operation_span, runtime_uncertain)) =
            operation
        {
            let settled_gate = known_primitive(&call_primitives[call_index])
                .filter(|primitive| dialect.leaf_owner_requires_owned_call_site(*primitive))
                .map(|_| call.span);
            requirements.push(OwnerRequirementCandidate {
                operation,
                operation_span,
                owner,
                report_mask,
                allow_uncertain: true,
                runtime_uncertain,
                through_contract: false,
                registration_uncertain: false,
                settled_target,
                settled_gate,
                providing_region_chain,
            });
        }
    }
    for callback in &file.compiler.callback_roles {
        if matches!(
            callback.role,
            solid_facts::compiler::CallbackRoleKind::EventHandler
                | solid_facts::compiler::CallbackRoleKind::DirectiveApply
        ) && (callback.role != solid_facts::compiler::CallbackRoleKind::DirectiveApply
            || ref_application_on_intrinsic(file, callback.span))
            && let Some(target) = callback_target(callback.span)
        {
            edges.push(SymbolicOwnerEdge {
                source: None,
                target,
                kind: OwnerEdgeKind::Unowned,
            });
        }
    }
    for element in &file.ast.jsx_elements {
        let boundary = jsx_primitive_name(file, element, entities, symbol_names, dialect);
        if boundary
            .as_deref()
            .is_some_and(|tag| dialect.is_async_boundary(tag))
        {
            requirements.push(OwnerRequirementCandidate {
                operation: "boundary",
                operation_span: Span::new(element.span.start, element.name.span.end),
                owner: owner_at(element.span),
                report_mask: OWNER_CONTEXT_UNOWNED,
                allow_uncertain: false,
                runtime_uncertain: false,
                through_contract: false,
                registration_uncertain: false,
                settled_target: None,
                settled_gate: None,
                providing_region_chain: region_chain(element.span),
            });
        }
    }
    CachedOwnerFile {
        source_hash: file.source_hash.clone(),
        cross_file_proofs: lookup.cross_file_proof_digest(),
        compiler: file.compiler.clone(),
        nodes,
        edges,
        requirements,
    }
}

pub(crate) fn resolve_owner_target(
    path: &str,
    target: &OwnerTarget,
    nodes_by_span: &HashMap<String, HashMap<Span, usize>>,
    by_symbol: &HashMap<SymbolId, usize>,
) -> Option<usize> {
    match target {
        OwnerTarget::Symbol(symbol) => by_symbol.get(symbol).copied(),
        OwnerTarget::LocalSpan(span) => nodes_by_span
            .get(path)
            .and_then(|nodes| nodes.get(span))
            .copied(),
        OwnerTarget::Function(path, span) => nodes_by_span
            .get(path)
            .and_then(|nodes| nodes.get(span))
            .copied(),
    }
}

pub(crate) fn find_missing_owners_incremental(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    indexes: &ProjectIndexes<'_>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    retained_source_paths: &HashSet<String>,
    cache: &mut HashMap<SourcePath, CachedOwnerFile>,
) -> (
    Vec<OwnerRequirement>,
    SettledGateDecisions,
    OwnerIncrementalTimings,
) {
    let entities = lookup.entities();
    let mut timings = OwnerIncrementalTimings::default();
    let total_started = Instant::now();
    let current_paths = facts
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    cache.retain(|path, _| current_paths.contains(path.as_str()));
    let mut recomputed = Vec::new();
    for file in &facts.files {
        if retained_source_paths.contains(file.path.as_str())
            && let Some(cached) = cache.get(file.path.as_str())
            && cached.source_hash == file.source_hash
            && cached.cross_file_proofs == lookup.cross_file_proof_digest()
            && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                || same_compiler_semantics(&cached.compiler, &file.compiler))
        {
            timings.reused_files += 1;
            continue;
        }
        recomputed.push(file);
        timings.recomputed_files += 1;
    }
    for (path, discovered) in parallel_slice_results(&recomputed, |file| {
        (
            file.path.clone(),
            discover_owner_file(file, indexes, entities, symbol_names, lookup),
        )
    }) {
        cache.insert(path, discovered);
    }
    let fragment_build = total_started.elapsed();

    let graph_started = Instant::now();
    let mut nodes = Vec::new();
    for file in &facts.files {
        if let Some(fragment) = cache.get(file.path.as_str()) {
            nodes.extend(fragment.nodes.iter().cloned());
        }
    }
    let mut nodes_by_span = HashMap::<String, HashMap<Span, usize>>::new();
    for (index, node) in nodes.iter().enumerate() {
        nodes_by_span
            .entry(node.path.clone())
            .or_default()
            .insert(node.span, index);
    }
    let by_symbol = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.symbol.clone().map(|symbol| (symbol, index)))
        .collect::<HashMap<_, _>>();
    let mut contexts = seed_contexts(&nodes);
    let mut outgoing = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
    for file in &facts.files {
        let Some(fragment) = cache.get(file.path.as_str()) else {
            continue;
        };
        for edge in &fragment.edges {
            let Some(target) =
                resolve_owner_target(file.path.as_str(), &edge.target, &nodes_by_span, &by_symbol)
            else {
                continue;
            };
            if let Some(source) = edge.source.as_ref().and_then(|source| {
                nodes_by_span
                    .get(&source.path)
                    .and_then(|nodes| nodes.get(&source.span))
                    .copied()
            }) {
                outgoing[source].push((target, edge.kind));
            } else {
                contexts[target] |= owner_edge_context(
                    edge.kind,
                    OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
                );
            }
        }
    }
    let graph_assembly = graph_started.elapsed();

    let propagation_started = Instant::now();
    propagate_owner_contexts(&mut contexts, &outgoing);
    let propagation = propagation_started.elapsed();

    let requirements_started = Instant::now();
    let mut requirements = Vec::new();
    let continuations = AwaitContinuations::new(facts, lookup);
    let mut settled_gates = SettledGateDecisions::new();
    let mut seen = HashSet::new();
    for file in &facts.files {
        let Some(fragment) = cache.get(file.path.as_str()) else {
            continue;
        };
        for candidate in &fragment.requirements {
            let owner_index = candidate.owner.and_then(|span| {
                nodes_by_span
                    .get(file.path.as_str())
                    .and_then(|nodes| nodes.get(&span))
                    .copied()
            });
            let context = owner_index.map_or(OWNER_CONTEXT_UNOWNED, |index| contexts[index]);
            // As in the batch pass: effects, cleanups and contract calls, never
            // a JSX boundary or `onSettled`'s returned cleanup.
            let after_await = (candidate.through_contract
                || matches!(candidate.operation, "effect" | "cleanup"))
                && candidate.operation != "settled-cleanup"
                && continuations.contains(file.path.as_str(), candidate.operation_span.start);
            let context = if after_await {
                await_continuation_context(context)
            } else {
                context
            };
            // The batch pass skips these before recording a settled gate too.
            // Every chain span is one of this fragment's own nodes; one that
            // did not resolve would withhold the lexical answer, not grant it.
            if root_owned_at(candidate.providing_region_chain.as_deref(), |span| {
                nodes_by_span
                    .get(file.path.as_str())
                    .and_then(|nodes| nodes.get(span))
                    .map_or(OWNER_CONTEXT_LEAVES_REGION, |index| contexts[*index])
            }) {
                continue;
            }
            if let Some(gate) = candidate.settled_gate {
                settled_gates.insert(
                    (
                        file.path.to_string(),
                        u64::from(gate.start),
                        u64::from(gate.end),
                    ),
                    leaf_gate_decision(owner_index, &nodes, &contexts),
                );
            }
            if candidate.operation == "settled-cleanup" {
                let cleanup_proof = candidate
                    .settled_target
                    .as_ref()
                    .and_then(|target| {
                        resolve_owner_target(file.path.as_str(), target, &nodes_by_span, &by_symbol)
                    })
                    .and_then(|index| {
                        let node = &nodes[index];
                        let callback_file = indexes.files_by_path.get(node.path.as_str())?;
                        let callback = callback_file
                            .ast
                            .functions
                            .iter()
                            .find(|function| function.span == node.span)?;
                        Some(function_cleanup_return_proof(
                            lookup,
                            callback_file,
                            callback,
                        ))
                    })
                    .unwrap_or(CleanupReturnProof::Unresolved);
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
                match cleanup_proof {
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
                    || owner_index.is_some_and(|index| nodes[index].component_uncertain));
            let later_run_unowned = !component_uncertain
                && !proven_unowned
                && context & OWNER_CONTEXT_LATER_RUN_UNOWNED != 0;
            let conditional_owner = !component_uncertain
                && !proven_unowned
                && !later_run_unowned
                && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                    == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
            let caller_uncertain = candidate.allow_uncertain
                && !proven_unowned
                && owner_index.is_some_and(|index| {
                    (nodes[index].exported
                        && contexts[index] & OWNER_CONTEXT_UNOWNED != 0
                        && !nodes[index].component
                        && !nodes[index].component_uncertain)
                        || open_world_unowned_only(context, &nodes[index])
                });
            let cleanup_return_uncertain = candidate.operation == "settled-cleanup"
                && candidate
                    .settled_target
                    .as_ref()
                    .and_then(|target| {
                        resolve_owner_target(file.path.as_str(), target, &nodes_by_span, &by_symbol)
                    })
                    .and_then(|index| {
                        let node = &nodes[index];
                        let callback_file = indexes.files_by_path.get(node.path.as_str())?;
                        let callback = callback_file
                            .ast
                            .functions
                            .iter()
                            .find(|function| function.span == node.span)?;
                        Some(function_cleanup_return_proof(
                            lookup,
                            callback_file,
                            callback,
                        ))
                    })
                    .is_none_or(|proof| {
                        matches!(
                            proof,
                            CleanupReturnProof::OptionalFunction | CleanupReturnProof::Unresolved
                        )
                    });
            let runtime_uncertain = candidate.runtime_uncertain || cleanup_return_uncertain;
            let uncertain = runtime_uncertain
                || conditional_owner
                || later_run_unowned
                || caller_uncertain
                || component_uncertain
                || candidate.registration_uncertain;
            push_owner_requirement(
                &mut requirements,
                &mut seen,
                candidate.operation,
                file,
                candidate.operation_span,
                OwnerRequirementStatus {
                    through_contract: candidate.through_contract,
                    uncertain,
                    runtime_uncertain,
                    caller_uncertain,
                    conditional_owner,
                    later_run_unowned,
                    component_uncertain,
                    after_await,
                    report: context & candidate.report_mask != 0,
                },
                lookup,
            );
        }
    }
    attribute_dependency_requirements(
        &mut requirements,
        facts,
        lookup,
        &nodes,
        &contexts,
        &outgoing,
    );
    let requirement_emission = requirements_started.elapsed();
    (
        requirements,
        settled_gates,
        OwnerIncrementalTimings {
            fragment_build,
            graph_assembly,
            propagation,
            requirement_emission,
            ..timings
        },
    )
}

/// The owner context bits that take a function out of the synchronous extent
/// of an owner-providing region written around it. See [`root_owned_at`].
const OWNER_CONTEXT_LEAVES_REGION: u8 =
    OWNER_CONTEXT_PROVEN_UNOWNED | OWNER_CONTEXT_LATER_RUN_UNOWNED;

/// The functions standing between `span` and the innermost owner-providing
/// region around it: every function whose body contains `span` and which is
/// itself written inside that region, the region's own callback included.
/// `None` when no region contains `span`.
///
/// `functions` yields `(key, span, body)` for the file's owner nodes; the key
/// comes back unchanged, so the batch pass can carry node indices and a
/// per-file fragment, which has no indices yet, function spans. The innermost
/// region is the only one that needs asking: an outer region's chain is a
/// superset of it, so an outer region answers only where the innermost does.
pub(crate) fn providing_region_chain<K>(
    providing_regions: &[Span],
    functions: impl IntoIterator<Item = (K, Span, Span)>,
    span: Span,
) -> Option<Vec<K>> {
    let region = providing_regions
        .iter()
        .filter(|region| region.contains(span))
        .min_by_key(|region| region.end - region.start)?;
    Some(
        functions
            .into_iter()
            .filter(|(_, function, body)| body.contains(span) && region.contains(*function))
            .map(|(key, _, _)| key)
            .collect(),
    )
}

/// Whether an operation lexically inside an owner-providing region is answered
/// by that region's owner. `chain` is [`providing_region_chain`]'s answer and
/// `context_of` each chain function's propagated owner context. Both owner
/// passes, every operation kind, ask this one question.
///
/// The region's owner is current only for code that runs in the region's
/// synchronous extent, and a function nested in the region leaves that extent
/// when its context carries either bit of [`OWNER_CONTEXT_LEAVES_REGION`]:
///
/// - [`OWNER_CONTEXT_LATER_RUN_UNOWNED`], from a `createRenderEffect` apply
///   ([`solid_dialect::CallbackOwner::InheritsFirstRun`]). `@solidjs/signals`
///   2.0.0-rc.3 and rc.9 (dev and prod client builds) run every apply after
///   the first from the flush with `getOwner() === null`, so an `onCleanup`
///   there under a `createRoot` never runs on dispose and raises
///   `NO_OWNER_CLEANUP`, while the first run's cleanup registers on the root.
/// - [`OWNER_CONTEXT_PROVEN_UNOWNED`], from a callback some invocation of
///   which runs with no owner: a [`solid_dialect::CallbackOwner::None`]
///   position (a `createEffect` apply, a `createReaction` invalidation, a
///   `runWithOwner(null, fn)` callback), a compiler event-handler or
///   directive-apply role, or an `unowned` contract row. Probed on
///   `solid-js`/`@solidjs/signals`/`@solidjs/web` 2.0.0-rc.3 and rc.9, dev and
///   prod client builds, each written inside a `createRoot` (the delegated
///   handler inside a `render` callback): `getOwner()` is `null` in every one
///   of them, the `onCleanup` raises `NO_OWNER_CLEANUP` (dev) and never runs
///   on dispose, and an `onSettled` in the `createEffect` apply runs
///   out-of-band, its returned cleanup raising `SETTLED_CLEANUP_UNOWNED`.
///
/// Such an operation is judged on the owner graph instead, which makes it a
/// violation where the propagated context is proven unowned and uncertifiable
/// where it carries only the later-run bit.
///
/// Only functions *inside* the region are asked. A region written inside the
/// detached function -- a `createRoot` created in a `createEffect` apply, or a
/// compiled JSX child, whose generated render effect owns its expression --
/// answers for what it contains: probed, an `onCleanup` in either runs on that
/// owner's disposal or re-run.
///
/// A callback handed to a reviewed fresh-stack host scheduler (`setTimeout`,
/// `queueMicrotask`, `Promise.then`, an observer constructor, ...) gets the
/// dialect's unowned edge ([`fresh_stack_scheduler_edges`]) and is judged on
/// the graph like the positions above. A nested callback with no owner edge at
/// all (one handed to an unmodelled function, or to a scheduler through a
/// wrapper call) carries neither bit and is still answered by the region: a
/// pre-existing approximation this does not widen.
pub(crate) fn root_owned_at<K>(chain: Option<&[K]>, context_of: impl Fn(&K) -> u8) -> bool {
    chain.is_some_and(|chain| {
        chain
            .iter()
            .all(|function| context_of(function) & OWNER_CONTEXT_LEAVES_REGION == 0)
    })
}

pub(crate) fn owner_callback_index(
    nodes: &[OwnerNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    by_symbol: &HashMap<SymbolId, usize>,
    file: &solid_facts::FileFacts,
    argument: Span,
    entities: &EntitySymbols,
) -> Option<usize> {
    nodes_by_path
        .get(file.path.as_str())
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| argument.contains(nodes[*index].span))
        // The argument's function can itself contain nested callbacks or a
        // returned closure. Select the outermost contained function: choosing
        // the smallest one assigns the caller's owner semantics to a nested
        // callback and leaves the actual argument unclassified.
        .max_by_key(|index| nodes[*index].span.end - nodes[*index].span.start)
        .or_else(|| {
            entities
                .get(&location(file.path.shared(), argument))
                .and_then(|symbol| by_symbol.get(symbol))
                .copied()
        })
}

pub(crate) const fn owner_edge_context(kind: OwnerEdgeKind, source: u8) -> u8 {
    match kind {
        OwnerEdgeKind::Preserve => source,
        OwnerEdgeKind::Owned => OWNER_CONTEXT_OWNED,
        OwnerEdgeKind::Unowned => OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
        OwnerEdgeKind::Conditional => OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED,
        // Not proven unowned: a later run happens only if the primitive runs
        // the callback again, which needs a change nothing here proves. A
        // source that is itself proven unowned stays proven: then every run,
        // first or later, is unowned.
        OwnerEdgeKind::InheritsFirstRun => {
            source | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LATER_RUN_UNOWNED
        }
        OwnerEdgeKind::Leaf => OWNER_CONTEXT_LEAF,
    }
}

/// Calls that run after an `await` on every path through their async
/// function's own body, keyed by file and callee start.
///
/// The positions are the producer's `calls_after_await` dominance fact (the
/// same one `reactive-read-after-await` proves reads with): branches, `&&`,
/// `try`/`catch`, loops and `switch` merge conservatively, and a nested
/// closure is never scanned, so a callback handed to `runWithOwner` after the
/// `await` is not one of them. Such a call runs from a promise continuation,
/// on an otherwise empty stack. Whether an owner is current there is the
/// dialect's fresh-stack answer, the same one host schedulers get
/// ([`Dialect::fresh_stack_callback_owner`]): Solid 2.0's owner is a
/// synchronous dynamic scope, so none is. Empty for a dialect that does not
/// answer [`solid_dialect::CallbackOwner::None`].
pub(crate) struct AwaitContinuations<'a> {
    starts: HashMap<&'a str, HashSet<u64>>,
}

impl<'a> AwaitContinuations<'a> {
    pub(crate) fn new(facts: &'a ProjectFacts, lookup: &SemanticLookup<'_>) -> Self {
        let mut starts = HashMap::<&'a str, HashSet<u64>>::new();
        if lookup.dialect.fresh_stack_callback_owner() == Some(solid_dialect::CallbackOwner::None) {
            for file in facts.typescript.files() {
                for function in file.async_functions.iter() {
                    for call in &function.calls_after_await {
                        starts
                            .entry(call.path.as_ref())
                            .or_default()
                            .insert(call.start_byte);
                    }
                }
            }
        }
        Self { starts }
    }

    /// Whether the call whose callee starts at `start` runs after an `await`.
    pub(crate) fn contains(&self, path: &str, start: u32) -> bool {
        self.starts
            .get(path)
            .is_some_and(|starts| starts.contains(&u64::from(start)))
    }
}

/// The owner context of an operation that runs after an `await`
/// ([`AwaitContinuations`]): proven unowned on every run, whatever context
/// the enclosing function was entered with.
const fn await_continuation_context(context: u8) -> u8 {
    (context | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED)
        & !(OWNER_CONTEXT_OWNED
            | OWNER_CONTEXT_LEAF
            | OWNER_CONTEXT_COMPONENT_UNCERTAIN
            | OWNER_CONTEXT_LATER_RUN_UNOWNED)
}

pub(crate) fn owner_context_at(
    nodes: &[OwnerNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    contexts: &[u8],
    path: &str,
    span: Span,
) -> u8 {
    containing_function_indexed(nodes, nodes_by_path, path, span)
        .map_or(OWNER_CONTEXT_UNOWNED, |index| contexts[index])
}

/// Whether the operation at `span` runs only after the same invocation of its
/// function has seen a non-null owner: it is the right operand of
/// `getOwner() && …`, or sits in the consequent of `getOwner() ? … : …` or of
/// `if (getOwner()) …`, where the test is a call of Solid's own `getOwner`
/// resolved by symbol (a same-named local function proves nothing).
///
/// Such an operation never executes without an owner, so "no scope's disposal
/// can trigger it" is false on every path that reaches it: probed on the
/// audited 2.0.0-rc.9, dev and prod, an exported helper written this way and
/// called at module scope skips `onCleanup` (no `NO_OWNER_CLEANUP`), while the
/// unguarded helper beside it warns. The guard and the operation must share
/// one function body with no `await` between them -- an owner seen before an
/// `await` is not current after it -- and nothing else is read as a guard:
/// `owner && …` over a binding, `!getOwner() || …`, and early returns keep
/// their requirement.
fn guarded_by_owner_probe(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let owner_probe = |test: Span| {
        let test = file.ast.peel_ts_sugar_span(test);
        file.ast.calls.iter().any(|call| {
            call.span == test
                && call.arguments.is_empty()
                && lookup.primitive_at_call(file, call.span) == Some(Primitive::GetOwner)
        })
    };
    let function = containing_ast_function(&file.ast, span).map(|function| function.span);
    let same_invocation = |test: Span, region: Span| {
        containing_ast_function(&file.ast, test).map(|function| function.span) == function
            && !file
                .ast
                .awaits
                .iter()
                .any(|awaited| region.contains(*awaited) && awaited.start < span.start)
    };
    file.ast.logical_expressions.iter().any(|logical| {
        logical.operator == solid_facts::ast::LogicalOperatorKind::And
            && logical.right.contains(span)
            && owner_probe(logical.left)
            && same_invocation(logical.left, logical.right)
    }) || file.ast.conditional_expressions.iter().any(|conditional| {
        conditional.consequent.contains(span)
            && owner_probe(conditional.test)
            && same_invocation(conditional.test, conditional.consequent)
    }) || file.ast.if_regions.iter().any(|region| {
        region.consequent.contains(span)
            && owner_probe(region.test)
            && same_invocation(region.test, region.consequent)
    })
}

/// The one place an owner requirement becomes a finding seed — both owner
/// passes, every operation, every uncertainty source — which is why the
/// discarded-region guard is applied here rather than at each push site.
///
/// Two passes build these candidates (`find_missing_owners` in batch,
/// `discover_owner_file` plus the incremental emission), and the comments above
/// record what happens when a rule is duplicated across them: they drift.
pub(crate) fn push_owner_requirement(
    requirements: &mut Vec<OwnerRequirement>,
    seen: &mut HashSet<(String, u64, u64, String)>,
    operation: &str,
    file: &solid_facts::FileFacts,
    span: Span,
    status: OwnerRequirementStatus,
    lookup: &SemanticLookup<'_>,
) {
    // The compiler deleted this operation. There is no owner question to
    // answer and no obligation to record: "this cleanup will never run" is a
    // claim about a call that never happens, and `uncertain` would be wrong too
    // — nothing is unproven here, the two compilers agree the code is gone.
    // Applied at the funnel for the same reason the divergence escalation below
    // is: a new candidate kind cannot forget it.
    if crate::execution_role::discarded_region_contains(file, span) {
        return;
    }
    // The operation runs only once the same invocation has seen an owner, so
    // there is no unowned execution to report and nothing left unproven.
    if guarded_by_owner_probe(file, span, lookup) {
        return;
    }
    let location = location(file.path.as_str(), span);
    if !seen.insert((
        location.path.to_string(),
        location.start_byte,
        location.end_byte,
        operation.into(),
    )) {
        // ADR 0252: a second requirement of the same operation at the same
        // call (a guarded registration beside an unguarded optional one, say)
        // merges into the first. One certain registration proves the call
        // registers; the context is the call's either way, so only the
        // registration's own certainty folds.
        let operation = crate::OwnerRequirementOperation::from_internal(operation);
        let certain = !status.uncertain;
        if certain
            && let Some(existing) = requirements
                .iter_mut()
                .find(|existing| existing.location == location && existing.operation == operation)
        {
            existing.uncertain = existing.missing_jsx_census;
        }
        return;
    }
    {
        let missing_jsx_census = crate::execution_role::missing_jsx_census_region(file, span);
        requirements.push(OwnerRequirement {
            operation: crate::OwnerRequirementOperation::from_internal(operation),
            location,
            uncertain: status.uncertain || missing_jsx_census,
            runtime_uncertain: status.runtime_uncertain,
            caller_uncertain: status.caller_uncertain,
            conditional_owner: status.conditional_owner,
            later_run_unowned: status.later_run_unowned,
            component_uncertain: status.component_uncertain,
            after_await: status.after_await,
            missing_jsx_census,
            through_contract: status.through_contract,
            through_dependency: None,
            report: status.report,
        });
    }
}
/// Which callback arguments of a call get an owner edge, and of what kind.
///
/// Shared by both owner passes on purpose: each used to keep its own literal
/// list, and lists kept separately drift.
pub(crate) fn owner_callback_edges(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: &Option<PrimitiveName>,
    lookup: &SemanticLookup<'_>,
) -> Vec<OwnerCallbackEdge> {
    let Some(primitive) = known_primitive(primitive) else {
        let scheduled = fresh_stack_scheduler_edges(file, call, lookup);
        if !scheduled.is_empty() {
            return scheduled;
        }
        // A call that names no primitive can still be an invocation of a
        // function some primitive returned -- but only where the dialect models
        // such a function. Solid 2.0 models none, so the binding-chain walk
        // below would resolve a primitive and then be told `None` every time.
        if !lookup.models_returned_callbacks() {
            return Vec::new();
        }
        let Some((returned, result_slot)) = returned_primitive_invocation(file, call, lookup)
        else {
            return Vec::new();
        };
        return (0..call.arguments.len())
            .filter_map(|argument| {
                lookup
                    .dialect
                    .returned_callback_semantics_at(
                        returned,
                        result_slot,
                        argument,
                        call.arguments.len(),
                    )
                    .owner
                    .map(|owner| OwnerCallbackEdge {
                        argument,
                        kind: callback_owner_edge_kind(owner),
                        source_path: file.path.to_string(),
                        source: call.span,
                    })
            })
            .collect();
    };
    let mut edges = Vec::new();
    for argument in 0..call.arguments.len() {
        let Some(owner) = callback_owner_at_call(file, call, primitive, argument, lookup) else {
            continue;
        };
        let kind = callback_owner_edge_kind(owner);
        if lookup
            .dialect
            .callback_semantics_at(primitive, argument, call.arguments.len())
            .requires_return_invocation
        {
            edges.extend(
                returned_callback_invocation_sites(file, call, lookup)
                    .into_iter()
                    .map(|site| OwnerCallbackEdge {
                        argument,
                        kind: compose_owner_edge(site.inherited_owner, kind),
                        source_path: site.path,
                        source: site.span,
                    }),
            );
        } else {
            edges.push(OwnerCallbackEdge {
                argument,
                kind,
                source_path: file.path.to_string(),
                source: call.span,
            });
        }
    }
    edges
}

/// The owner edges of callbacks handed to a reviewed fresh-stack host
/// scheduler (`runtime_semantics::FRESH_STACK_SCHEDULERS`: `setTimeout`,
/// `queueMicrotask`, `Promise.then`, the observer constructors, ...).
///
/// Two separately reviewed facts compose here. The host fact -- the scheduler
/// invokes the argument only from a task or microtask queue, on an otherwise
/// empty stack -- comes from the compiler-selected standard-library
/// declaration, never from spelling. The owner fact -- what that empty stack
/// means for the owner -- is the dialect's
/// ([`Dialect::fresh_stack_callback_owner`]); a dialect that does not answer
/// gives no edge, and the callback keeps its lexical answer.
///
/// Only an argument that *is* the callback gets the edge: a function literal
/// or an identifier naming one. `setTimeout(wrap(() => ...))` hands the host
/// whatever `wrap` returns, and the arrow inside may run on `wrap`'s own
/// stack, under the scheduling call's owner.
///
/// Both owner passes reach this through [`owner_callback_edges`], so the
/// batch and incremental graphs cannot disagree about it.
fn fresh_stack_scheduler_edges(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<OwnerCallbackEdge> {
    let Some(owner) = lookup.dialect.fresh_stack_callback_owner() else {
        return Vec::new();
    };
    let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
        return Vec::new();
    };
    call.arguments
        .iter()
        .enumerate()
        .filter(|(_, argument)| {
            matches!(
                argument.value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
                    | solid_facts::ast::ArgumentValueKind::Identifier
            )
        })
        .filter(|(index, argument)| {
            let callability = lookup
                .entity_at(file.path.as_str(), argument.span)
                .and_then(|entity| entity.callability);
            crate::runtime_semantics::argument_behavior(resolved, callability, *index)
                .is_some_and(crate::runtime_semantics::RuntimeArgumentBehavior::runs_on_fresh_stack)
        })
        .map(|(argument, _)| OwnerCallbackEdge {
            argument,
            kind: callback_owner_edge_kind(owner),
            source_path: file.path.to_string(),
            source: call.span,
        })
        .collect()
}

pub(crate) const fn callback_owner_edge_kind(owner: solid_dialect::CallbackOwner) -> OwnerEdgeKind {
    match owner {
        solid_dialect::CallbackOwner::Creates => OwnerEdgeKind::Owned,
        solid_dialect::CallbackOwner::Conditional => OwnerEdgeKind::Conditional,
        solid_dialect::CallbackOwner::Inherits => OwnerEdgeKind::Preserve,
        solid_dialect::CallbackOwner::None => OwnerEdgeKind::Unowned,
        solid_dialect::CallbackOwner::InheritsFirstRun => OwnerEdgeKind::InheritsFirstRun,
        solid_dialect::CallbackOwner::Leaf => OwnerEdgeKind::Leaf,
    }
}

/// Compose the owner supplied by a returned function's invocation with the
/// owner behavior of the factory callback itself. An explicit owner result on
/// the inner callback dominates; an inheriting callback keeps the invocation
/// site's context.
pub(crate) const fn compose_owner_edge(
    invocation: OwnerEdgeKind,
    callback: OwnerEdgeKind,
) -> OwnerEdgeKind {
    match callback {
        OwnerEdgeKind::Preserve => invocation,
        _ => callback,
    }
}

/// The argument whose callback runs under an owner this call creates (or, for
/// `runWithOwner`, supplies). The two owner passes share this answer; each
/// used to keep its own literal list, and the lists disagreed with every
/// primitive outside seven names.
pub(crate) fn owner_providing_argument(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Option<Primitive>,
    lookup: &SemanticLookup<'_>,
) -> Option<usize> {
    let primitive = primitive?;
    (0..call.arguments.len()).find(|index| {
        callback_owner_at_call(file, call, primitive, *index, lookup)
            == Some(solid_dialect::CallbackOwner::Creates)
    })
}

pub(crate) fn containing_leaf_owner(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            let owner = call_primitive_name(file, call, entities, symbol_names, lookup.dialect)?;
            owner
                .primitive()
                .is_some_and(|primitive| {
                    callback_owner_at_call(file, call, primitive, index, lookup)
                        == Some(solid_dialect::CallbackOwner::Leaf)
                })
                .then(|| owner.to_string())
        })
}

/// Whether a `Loading` boundary above a render position is proven, proven
/// absent, or cannot be resolved from the analyzed project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoadingCover {
    /// A boundary encloses the position, directly or through the call sites
    /// that render the function containing it.
    Covered,
    /// Every call-site chain was followed to a mount root (a `render` or
    /// `hydrate` argument) and none passed through a boundary.
    Uncovered,
    /// A chain ends at a function nothing in the project renders -- an
    /// exported component, a route handed to a router, a lazy page -- so the
    /// boundary above it is decided by code the analysis cannot see. Missing
    /// is not absent.
    Unresolved,
}

pub(crate) fn read_loading_cover(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> LoadingCover {
    loading_cover_at(lookup, file, span, symbol_names, &mut HashSet::new())
}

fn loading_cover_at(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    visited: &mut HashSet<(String, Span)>,
) -> LoadingCover {
    let entities = lookup.entities();
    if file.ast.jsx_containing(span).any(|element| {
        jsx_element_is_loading(file, element, entities, symbol_names, lookup.dialect)
    }) {
        return LoadingCover::Covered;
    }
    if file.ast.jsx_containing(span).any(|element| {
        jsx_target_function(lookup, file, element).is_some_and(|(target_file, target)| {
            target_file.ast.jsx_within(target.body).any(|candidate| {
                jsx_element_is_loading(
                    target_file,
                    candidate,
                    entities,
                    symbol_names,
                    lookup.dialect,
                )
            })
        })
    }) {
        return LoadingCover::Covered;
    }
    // Walk outward from the innermost function. A closure nothing in the
    // project calls (a `<For>` row callback, a render prop) is part of the
    // function around it; the first function the project does render or call
    // is the one whose call sites decide.
    let mut enclosing = file.ast.functions_body_containing(span).collect::<Vec<_>>();
    enclosing.sort_by_key(|function| function.body.end - function.body.start);
    for function in enclosing {
        if !visited.insert((file.path.as_str().to_owned(), function.span)) {
            // Already being resolved on this chain: it adds no information.
            return LoadingCover::Uncovered;
        }
        let call_sites = lookup.function_call_sites(file.path.as_str(), function.span);
        if !call_sites.is_empty() {
            if file.ast.jsx_within(function.body).any(|candidate| {
                jsx_element_is_loading(file, candidate, entities, symbol_names, lookup.dialect)
            }) {
                return LoadingCover::Covered;
            }
            let mut result = LoadingCover::Uncovered;
            for (caller_file, callee) in call_sites {
                match loading_cover_at(lookup, caller_file, callee, symbol_names, visited) {
                    LoadingCover::Covered => return LoadingCover::Covered,
                    LoadingCover::Unresolved => result = LoadingCover::Unresolved,
                    LoadingCover::Uncovered => {}
                }
            }
            return result;
        }
        if function_is_mount_root(lookup, file, function) {
            return LoadingCover::Uncovered;
        }
    }
    LoadingCover::Unresolved
}

/// Whether `function` is what a `render`/`hydrate` call mounts: the function
/// literal passed as its first argument, or a function it names.
fn function_is_mount_root(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    file.ast.calls.iter().any(|call| {
        lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| lookup.dialect.mounts_component_tree(primitive))
            && call.arguments.first().is_some_and(|argument| {
                let argument = file.ast.peel_ts_sugar_span(argument.span);
                argument == function.span
                    || lookup
                        .function_called_at(file.path.as_str(), argument)
                        .is_some_and(|(target_file, target)| {
                            target_file.path == file.path && target.span == function.span
                        })
            })
    })
}

/// Whether a compiler ref/directive application role at `span` belongs to an
/// intrinsic element. The runtime applies an intrinsic element's `ref` through
/// `ref()` with no owner; a component (a capitalized or member tag) receives
/// `ref` as an ordinary prop and invokes it whenever its own code does, often
/// in its owned body. By JSX semantics a lowercase, non-member tag is
/// intrinsic, so a callback written as an attribute value of any other element
/// is not a proven ownerless application.
pub(crate) fn ref_application_on_intrinsic(file: &solid_facts::FileFacts, span: Span) -> bool {
    !file.ast.jsx_elements.iter().any(|element| {
        let tag = file
            .source
            .get(element.name.span.start as usize..element.name.span.end as usize)
            .unwrap_or_default();
        let component = tag.contains('.') || tag.starts_with(|c: char| c.is_ascii_uppercase());
        component
            && element.attributes.iter().any(|attribute| {
                attribute
                    .expression
                    .is_some_and(|value| file.ast.peel_ts_sugar_span(value).contains(span))
            })
    })
}

/// The role a pending async read plays for the missing-boundary claim: a
/// tracked read inside a non-render computation (a memo, a user effect's
/// compute, a derived signal) is not a render of the value, so it is not
/// [`ExecutionRole::TrackedJsx`] for that claim. The pending state flows on to
/// whichever render effect consumes the computation, and that consumer is the
/// one whose boundary matters.
pub(crate) fn async_read_role(
    file: &solid_facts::FileFacts,
    span: Span,
    role: crate::ExecutionRole,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> crate::ExecutionRole {
    if role != crate::ExecutionRole::TrackedJsx {
        return role;
    }
    // A function that is a JSX attribute's value (an event handler, a callback
    // prop) runs when something calls it, not while the JSX renders. Nothing
    // proves a read inside it is rendered, so it is not a render read. A
    // `children` attribute is the element's own children and keeps its role.
    if file.ast.functions_body_containing(span).any(|function| {
        file.ast.jsx_containing(function.span).any(|element| {
            element.attributes.iter().any(|attribute| {
                attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                    && attribute
                        .expression
                        .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == function.span)
                    && file
                        .source
                        .get(attribute.name.start as usize..attribute.name.end as usize)
                        != Some("children")
            })
        })
    }) {
        return crate::ExecutionRole::Unknown;
    }
    let dialect = lookup.dialect;
    let in_computation = file.ast.arguments_containing(span).any(|(call, index)| {
        let argument = &call.arguments[index];
        matches!(
            argument.value,
            solid_facts::ast::ArgumentValueKind::Identifier
                | solid_facts::ast::ArgumentValueKind::Function
                | solid_facts::ast::ArgumentValueKind::AsyncFunction
        ) && call_primitive_name(file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                    && dialect
                        .callback_semantics_at(primitive, index, call.arguments.len())
                        .tracks_reads
                    && !dialect.computation_read_is_render(primitive)
            })
            // A JSX region written inside the computation is a render again.
            && !file
                .compiler
                .tracked_regions
                .iter()
                .any(|region| argument.span.contains(region.span) && region.span.contains(span))
    });
    if in_computation {
        crate::ExecutionRole::Unknown
    } else {
        role
    }
}

pub(crate) fn jsx_element_is_loading(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> bool {
    jsx_primitive_name(file, element, entities, symbol_names, dialect)
        .as_deref()
        .is_some_and(|tag| dialect.is_async_boundary(tag))
}

pub(crate) fn jsx_target_function<'a>(
    lookup: &SemanticLookup<'a>,
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> Option<(
    &'a solid_facts::FileFacts,
    &'a solid_facts::ast::FunctionFact,
)> {
    lookup.function_called_at(file.path.as_str(), element.name.span)
}

pub(crate) fn computation_is_async(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    argument: Span,
) -> bool {
    // The computation is the argument's own function. An async function that
    // is merely nested inside it (`createMemo(() => async () => ...)`, an async
    // IIFE) is a value the computation returns or runs, not the computation:
    // the memo's result is a function and can never be pending.
    let argument = file.ast.peel_ts_sugar_span(argument);
    if lookup
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.async_functions.iter().any(|function| {
                function.can_return_async
                    && u64::from(argument.start) == function.expression.start_byte
                    && function.expression.end_byte == u64::from(argument.end)
            })
        })
    {
        return true;
    }
    file.ast
        .functions
        .iter()
        .find(|function| function.span == argument)
        .is_some_and(|function| function.r#async)
}

pub(crate) fn inside_non_component_function(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if file
        .compiler
        .callback_roles
        .iter()
        .any(|callback| callback.span.contains(span))
    {
        return false;
    }
    // An argument to a primitive that takes a callback is not "some helper
    // function's body", whatever the helper is called. Asked of the dialect
    // rather than listed: the seven names this had were 2.0's, three of which
    // 1.x does not have, and it was missing every 1.x callback-taker outside
    // the four it shares.
    if file.ast.arguments_containing(span).any(|(call, index)| {
        lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| {
                lookup
                    .dialect
                    .callback_semantics_at(primitive, index, call.arguments.len())
                    .execution
                    .is_some()
            })
    }) {
        return false;
    }
    file.ast.functions_body_containing(span).any(|function| {
        function_binding_name(file, function).is_some()
            && !object_method_escapes(file, function, lookup)
            && !lookup.function_may_be_component(file, function)
    })
}

/// ADR 0320: whether `function` is an object-literal method whose literal
/// goes anywhere other than a member dispatch the analysis resolves: handed
/// to unknown code, enumerated, returned, stored. Such a method is a stored
/// callback like a function-expression property, not a named helper whose
/// reads are claimed where it is called: it may be called where nothing
/// claims them.
pub(crate) fn object_method_escapes(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(name) = own_method_name(file, function) else {
        return false;
    };
    // An object literal's own method; a class's is selected by its instance.
    if !file
        .ast
        .object_properties
        .iter()
        .any(|property| property.key == name.span && property.method)
    {
        return false;
    }
    let Some(literal) = file
        .ast
        .object_get_shapes
        .iter()
        .filter(|shape| shape.span.contains(name.span))
        .min_by_key(|shape| shape.span.end - shape.span.start)
    else {
        return true;
    };
    !object_literal_only_dispatched(file, literal.span, lookup)
}

/// Whether every way the object literal at `literal` is used is a member
/// call on it: it is the receiver of one, or a `const` in this module whose
/// every reference is, or an argument of an exact callee in this file that
/// only reads members of its parameter.
fn object_literal_only_dispatched(
    file: &solid_facts::FileFacts,
    literal: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let called_member_of = |value: Span| {
        file.ast.members.iter().any(|member| {
            file.ast.peel_ts_sugar_span(member.object) == value
                && file
                    .ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_err()
                && file
                    .ast
                    .calls
                    .iter()
                    .any(|call| file.ast.peel_ts_sugar_span(call.callee) == member.span)
        })
    };
    let closed_argument = |value: Span| {
        file.ast
            .arguments_containing(value)
            .find(|(call, index)| file.ast.peel_ts_sugar_span(call.arguments[*index].span) == value)
            .is_some_and(|(call, index)| {
                let mut consulted = Vec::new();
                !call.arguments[index].spread
                    && lookup.parameter_only_member_accessed_consulting(
                        file,
                        call,
                        index,
                        &mut consulted,
                    )
                    && consulted
                        .iter()
                        .all(|(path, _)| path.as_str() == file.path.as_str())
            })
    };
    if called_member_of(literal) || closed_argument(literal) {
        return true;
    }
    let Some(binding) = file.ast.bindings.iter().find(|binding| {
        binding
            .initializer
            .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == literal)
    }) else {
        return false;
    };
    let [name] = binding.names.as_slice() else {
        return false;
    };
    binding.immutable
        && binding.shape == solid_facts::ast::BindingShape::Identifier
        && crate::execution_role::file_is_module(file)
        && !crate::execution_role::flow_binding_exported(file, name.span)
        && file
            .ast
            .reference_declarations
            .iter()
            .filter(|(reference, declaration)| *declaration == name.span && *reference != name.span)
            .all(|(reference, _)| called_member_of(*reference) || closed_argument(*reference))
}

pub(crate) fn inside_unclassified_callback(file: &solid_facts::FileFacts, span: Span) -> bool {
    if file
        .compiler
        .callback_roles
        .iter()
        .any(|callback| callback.span.contains(span))
    {
        return false;
    }
    containing_ast_function(&file.ast, span)
        .is_some_and(|function| function_binding_name(file, function).is_none())
}

/// Whether `span` is inside a syntactically direct function argument that a
/// known primitive provably never invokes.
///
/// Two positive proofs qualify, and nothing else: the dialect declares the
/// position a stored value (Solid 1.x `createSignal(() => value)`), or the
/// position's modelled callback belongs to a returned lazy adapter that this
/// call never consumes (`mapArray` whose result is discarded). A primitive
/// with no callback model at all — 2.0 `children`, `onCleanup` — proves
/// nothing: those callbacks do run, so their reads must not be treated as
/// dormant. Requiring the argument itself to be a function keeps an IIFE
/// passed as a value from being mistaken for dormant code.
pub(crate) fn inside_known_value_function_argument(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    // Both proofs are dialect answers that Solid 2.0 leaves at their negative
    // default, so under 2.0 this question is provably always "no" and the
    // containment query below need not run at all.
    if !lookup.models_stored_function_arguments() && !lookup.models_returned_callbacks() {
        return false;
    }
    // Only the calls whose arguments actually contain `span` can answer. The
    // former whole-file scan asked every call in the file, once per call being
    // classified, which is quadratic in a file's call count.
    file.ast
        .arguments_containing(span)
        .any(|(call, argument_index)| {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                return false;
            };
            let argument = &call.arguments[argument_index];
            matches!(
                argument.value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
            ) && ({
                let semantics = lookup.dialect.callback_semantics_at(
                    primitive,
                    argument_index,
                    call.arguments.len(),
                );
                semantics.stores_as_value
                    || (semantics.requires_return_invocation
                        && callback_execution_at_call(
                            file,
                            call,
                            primitive,
                            argument_index,
                            lookup,
                        )
                        .is_none())
            })
        })
}

/// The callback execution fact for one concrete primitive call, after proving
/// any lazy returned adapter is actually consumed.
///
/// Solid 1.x `mapArray` and `indexArray` merely allocate and return a function;
/// their list and mapper arguments run only when that returned function runs.
/// The dialect owns that runtime distinction, while this helper proves the
/// use from canonical TypeScript symbols and the enclosing AST call shape.
pub(crate) fn callback_execution_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Primitive,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::Execution> {
    if crate::effect_execution::at_call(file, call, primitive, argument, lookup)
        != crate::effect_execution::ApplyExecution::Proven
    {
        return None;
    }
    let semantics = lookup
        .dialect
        .callback_semantics_at(primitive, argument, call.arguments.len());
    if semantics.requires_return_invocation
        && !returned_callback_result_is_invoked(file, call, lookup)
    {
        return None;
    }
    semantics.execution
}

/// The callback-owner fact for one concrete primitive call, after proving
/// that callbacks implemented by a returned lazy adapter can run.
///
/// Execution, reachability, cleanup, and both owner-graph passes must ask the
/// same call-site question. Otherwise a discarded adapter can disappear from
/// read analysis while still manufacturing an impossible owner diagnostic.
pub(crate) fn callback_owner_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Primitive,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::CallbackOwner> {
    use crate::effect_execution::ApplyExecution;
    match crate::effect_execution::at_call(file, call, primitive, argument, lookup) {
        ApplyExecution::Absent => return None,
        ApplyExecution::Unknown => return Some(solid_dialect::CallbackOwner::Conditional),
        ApplyExecution::Proven => {}
    }
    if lookup
        .dialect
        .effect_apply_defer_options(primitive, argument)
        .is_some_and(|options| {
            crate::call_option_literal(file, call, options, "defer")
                == solid_dialect::OptionLiteral::True
        })
    {
        // A deferred apply has no inherited first run, including render effects.
        return Some(solid_dialect::CallbackOwner::None);
    }
    let semantics = lookup
        .dialect
        .callback_semantics_at(primitive, argument, call.arguments.len());
    if semantics.requires_return_invocation
        && !returned_callback_result_is_invoked(file, call, lookup)
    {
        return None;
    }
    if primitive == Primitive::RunWithOwner && argument == 1 {
        return run_with_owner_callback_owner(file, call, lookup);
    }
    semantics.owner
}

/// `runWithOwner` is the one ownership primitive whose owner is data. Both
/// dialects accept `Owner | null`; 2.0 makes the null spelling the documented
/// way to detach a root. Preserve a definite answer where the call site proves
/// one and keep a conditional edge everywhere else so certification cannot
/// silently assume a nullable owner exists.
pub(crate) fn run_with_owner_callback_owner(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::CallbackOwner> {
    let owner = call.arguments.first()?;
    if owner.value == solid_facts::ast::ArgumentValueKind::Undefined
        || owner.value == solid_facts::ast::ArgumentValueKind::Null
    {
        return Some(solid_dialect::CallbackOwner::None);
    }
    // `void expression` has no dedicated AST argument kind yet. This text
    // check is conservative (it can only detach an owner) and remains local
    // to the one syntax shape not represented by the fact protocol.
    let source = file.source_text(owner.span)?.trim();
    if source.starts_with("void ") {
        return Some(solid_dialect::CallbackOwner::None);
    }

    // 2.0's createOwner() is non-null by construction. Resolve the nested
    // call semantically so a local function with the same spelling proves
    // nothing.
    if lookup.primitive_at_call(file, owner.span) == Some(Primitive::CreateOwner) {
        return Some(solid_dialect::CallbackOwner::Creates);
    }
    if component_owner_binding(file, owner.span, lookup) {
        return Some(solid_dialect::CallbackOwner::Creates);
    }

    if let Some(descriptor) = lookup
        .entity_at(file.path.as_str(), owner.span)
        .and_then(|entity| entity.type_descriptor.as_deref())
    {
        // TypeScript may preserve a user alias's name rather than render its
        // nullable expansion, so absence of the word `null` is not proof.
        // The compiler-resolved alias declaration and origin module must be
        // the dialect's own Owner export; a user-local type with the same
        // spelling has no such role. Nullable/union types remain Creates here
        // only when the selected type itself is the Solid Owner branch, and
        // the existing call-site analysis keeps unresolved values conditional.
        if descriptor.alias_declarations.iter().any(|declaration| {
            lookup
                .dialect
                .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                == Some(solid_dialect::TypeRole::Owner)
        }) {
            return Some(solid_dialect::CallbackOwner::Creates);
        }
    }

    Some(solid_dialect::CallbackOwner::Conditional)
}

/// ADR 0206: whether `value` names a `const` bound to `getOwner()` written
/// directly in a proven component's body.
///
/// A proven component's body runs under an owner: the owner graph seeds it
/// owned (`owner_node`), as its render does. `getOwner()` called directly
/// there, not in a nested function or a default, therefore returns that
/// owner, never `null`, and a `const` keeps it. The owner may have been
/// disposed by the time a later callback hands it to `runWithOwner`; that is
/// not the null owner this question is about.
fn component_owner_binding(
    file: &solid_facts::FileFacts,
    value: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let value = file.ast.peel_ts_sugar_span(value);
    let Some((binding_file, binding, _)) = lookup.binding_at_reference(file.path.as_str(), value)
    else {
        return false;
    };
    let Some(initializer) = binding.call_initializer else {
        return false;
    };
    if !binding.immutable
        || binding.shape != solid_facts::ast::BindingShape::Identifier
        || lookup.primitive_at_call(binding_file, initializer) != Some(Primitive::GetOwner)
    {
        return false;
    }
    let Some(component) = containing_ast_function(&binding_file.ast, initializer) else {
        return false;
    };
    !component.r#async
        && !component.generator
        && written_directly_in(&binding_file.ast, component, initializer)
        && lookup.function_component_status(binding_file, component)
            == crate::indexes::ComponentStatus::Proven
}

/// Resolve the primitive whose returned function is the callee of `call`.
///
/// Canonical TypeScript symbols prove identity, AST binding facts prove the
/// initializer/alias chain, and the dialect's resolved primitive index proves
/// that the initializer is the actual Solid export rather than a same-spelled
/// local function.
pub(crate) fn returned_primitive_invocation(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<(Primitive, Option<usize>)> {
    let mut reference_path = file.path.as_str();
    let mut reference_span = call.callee;
    let mut seen = HashSet::new();
    loop {
        // Callee result values do not always receive an entity fact of their
        // own (tuple slots are the motivating case). Resolve them from the
        // canonical reference set of each demanded binding declaration, then
        // follow local or cross-file aliases one binding at a time.
        let (binding_file, binding, symbol) =
            lookup.binding_at_reference(reference_path, reference_span)?;
        if !seen.insert(symbol.clone()) {
            return None;
        }
        if let Some(initializer) = binding.call_initializer {
            let call_index = lookup.call_index(binding_file, initializer)?;
            let result_slot = match binding.shape {
                solid_facts::ast::BindingShape::Array => {
                    Some(binding.array_slots.iter().position(|slot| {
                        slot.as_ref().and_then(|name| {
                            lookup.entities().at(binding_file.path.as_str(), name.span)
                        }) == Some(&symbol)
                    })?)
                }
                _ => None,
            };
            return known_primitive(&lookup.primitives(binding_file).calls[call_index])
                .map(|primitive| (primitive, result_slot));
        }
        let initializer = binding.initializer_identifier.as_ref()?;
        reference_path = binding_file.path.as_str();
        reference_span = initializer.span;
    }
}

/// The execution contract of one callback argument accepted by a proven
/// returned function (including a specific destructured tuple slot).
pub(crate) fn returned_callback_execution_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::Execution> {
    // Solid 2.0 answers `None` for every returned-callback contract, so the
    // binding-chain resolution below cannot produce an answer there. Skip it
    // rather than walk it once per call and discard the result.
    if !lookup.models_returned_callbacks() {
        return None;
    }
    let (primitive, result_slot) = returned_primitive_invocation(file, call, lookup)?;
    lookup
        .dialect
        .returned_callback_semantics_at(primitive, result_slot, argument, call.arguments.len())
        .execution
}

pub(crate) fn returned_callback_result_is_invoked(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    !returned_callback_invocation_sites(file, factory_call, lookup).is_empty()
}

/// Proven project invocations of the function returned by `factory_call`.
///
/// TypeScript symbols establish that calls, JSX tags, and `.preload()` uses
/// refer to the exact factory result. AST facts establish how the value is
/// consumed. For values passed to another known callback-taking primitive,
/// the invocation inherits that callback's owner contract; direct calls and
/// JSX uses inherit their containing function's owner.
pub(crate) fn returned_callback_invocation_sites(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<ReturnedCallbackInvocationSite> {
    let direct_factory_value = |argument: Span| {
        file.ast
            .calls_within(argument)
            .max_by_key(|nested| nested.span.end - nested.span.start)
            .is_some_and(|nested| nested.span == factory_call.span)
    };
    let mut sites = Vec::new();
    for outer in file.ast.calls.iter().filter(|outer| {
        outer.span != factory_call.span
            && outer.callee.contains(factory_call.span)
            && file
                .ast
                .calls_within(outer.callee)
                .filter(|nested| nested.span != outer.span)
                .max_by_key(|nested| nested.span.end - nested.span.start)
                .is_some_and(|nested| nested.span == factory_call.span)
            // `factory(...).member()` invokes the member, not the returned
            // function, so it proves nothing about the factory's callbacks.
            // `preload` is the one member the runtime routes to the loader,
            // matching the binding-based preload proof below.
            && lookup
                .member_property_at(file, outer.callee)
                .is_none_or(|property| file.source_text(property) == Some("preload"))
    }) {
        sites.push(ReturnedCallbackInvocationSite {
            path: file.path.to_string(),
            span: outer.span,
            inherited_execution: None,
            inherited_owner: OwnerEdgeKind::Preserve,
        });
    }

    for (outer, index) in file.ast.arguments_containing(factory_call.span) {
        let primitive = lookup.primitive_at_call(file, outer.span);
        if direct_factory_value(outer.arguments[index].span)
            && primitive.is_some_and(|primitive| {
                lookup
                    .dialect
                    .callback_semantics_at(primitive, index, outer.arguments.len())
                    .execution
                    .is_some()
            })
        {
            let inherited_owner = primitive
                .and_then(|primitive| callback_owner_at_call(file, outer, primitive, index, lookup))
                .map_or(OwnerEdgeKind::Preserve, callback_owner_edge_kind);
            sites.push(ReturnedCallbackInvocationSite {
                path: file.path.to_string(),
                span: outer.span,
                inherited_execution: primitive.and_then(|primitive| {
                    lookup
                        .dialect
                        .callback_semantics_at(primitive, index, outer.arguments.len())
                        .execution
                }),
                inherited_owner,
            });
        }
    }

    let mut factory_bindings = file
        .ast
        .bindings
        .iter()
        .filter(|binding| binding.call_initializer == Some(factory_call.span))
        .flat_map(|binding| &binding.names)
        .filter_map(|name| lookup.entities().at(file.path.as_str(), name.span).cloned())
        .collect::<HashSet<_>>();
    if factory_bindings.is_empty() {
        sites.sort_by(|left, right| left.order_key().cmp(&right.order_key()));
        sites.dedup();
        return sites;
    }

    // Follow identifier aliases in every project file. The compiler's symbol
    // reference table bridges imports/re-exports; local AST bindings bridge a
    // fresh `const Alias = Imported` symbol introduced after that boundary.
    loop {
        let references = returned_binding_references(lookup, &factory_bindings);
        let aliases = lookup
            .files()
            .iter()
            .flat_map(|candidate| {
                candidate
                    .ast
                    .bindings
                    .iter()
                    .filter_map(|binding| {
                        let initializer = binding.initializer_identifier.as_ref()?;
                        returned_binding_reference(
                            candidate,
                            initializer.span,
                            lookup,
                            &factory_bindings,
                            &references,
                        )
                        .then_some(binding)
                    })
                    .flat_map(move |binding| {
                        binding.names.iter().filter_map(|name| {
                            lookup
                                .entities()
                                .at(candidate.path.as_str(), name.span)
                                .cloned()
                        })
                    })
            })
            .collect::<Vec<_>>();
        let before = factory_bindings.len();
        factory_bindings.extend(aliases);
        if factory_bindings.len() == before {
            break;
        }
    }

    let references = returned_binding_references(lookup, &factory_bindings);
    for use_file in lookup.files() {
        for outer in &use_file.ast.calls {
            if returned_binding_reference(
                use_file,
                outer.callee,
                lookup,
                &factory_bindings,
                &references,
            ) {
                sites.push(ReturnedCallbackInvocationSite {
                    path: use_file.path.to_string(),
                    span: outer.span,
                    inherited_execution: None,
                    inherited_owner: OwnerEdgeKind::Preserve,
                });
            }
            let primitive = lookup.primitive_at_call(use_file, outer.span);
            for (index, argument) in outer.arguments.iter().enumerate() {
                if returned_binding_reference(
                    use_file,
                    argument.span,
                    lookup,
                    &factory_bindings,
                    &references,
                ) && primitive.is_some_and(|primitive| {
                    lookup
                        .dialect
                        .callback_semantics_at(primitive, index, outer.arguments.len())
                        .execution
                        .is_some()
                }) {
                    let inherited_owner = primitive
                        .and_then(|primitive| {
                            callback_owner_at_call(use_file, outer, primitive, index, lookup)
                        })
                        .map_or(OwnerEdgeKind::Preserve, callback_owner_edge_kind);
                    sites.push(ReturnedCallbackInvocationSite {
                        path: use_file.path.to_string(),
                        span: outer.span,
                        inherited_execution: primitive.and_then(|primitive| {
                            lookup
                                .dialect
                                .callback_semantics_at(primitive, index, outer.arguments.len())
                                .execution
                        }),
                        inherited_owner,
                    });
                }
            }

            // `lazyResult.preload()` resolves the property as a distinct
            // method; prove the receiver from the member-expression AST.
            if use_file.ast.members.iter().any(|member| {
                outer.callee.contains(member.span)
                    && use_file.source_text(member.property) == Some("preload")
                    && returned_binding_reference(
                        use_file,
                        member.object,
                        lookup,
                        &factory_bindings,
                        &references,
                    )
            }) {
                sites.push(ReturnedCallbackInvocationSite {
                    path: use_file.path.to_string(),
                    span: outer.span,
                    inherited_execution: None,
                    inherited_owner: OwnerEdgeKind::Preserve,
                });
            }
        }

        for element in &use_file.ast.jsx_elements {
            if returned_binding_reference(
                use_file,
                element.name.span,
                lookup,
                &factory_bindings,
                &references,
            ) {
                sites.push(ReturnedCallbackInvocationSite {
                    path: use_file.path.to_string(),
                    span: element.span,
                    inherited_execution: None,
                    inherited_owner: OwnerEdgeKind::Preserve,
                });
            }
        }
    }

    sites.sort_by(|left, right| left.order_key().cmp(&right.order_key()));
    sites.dedup();
    sites
}

/// ADR 0292: `(path, function span, property span)` for each object property
/// proven to be the one route by which a project component reaches anything:
/// the `default` member of the module object a dialect component loader
/// renders ([`Dialect::renders_loaded_default_argument`]).
///
/// ```tsx
/// const Page = lazy(async () => {
///   function Cube() { start(); return <div />; }
///   return { default: Cube };
/// });
/// <Page />
/// ```
///
/// The witness holds only when all of these do:
///
/// - the loader is the call's whole argument at the dialect's slot, an
///   `async` non-generator function literal, so nothing but the call holds it;
/// - its one unconditional return (its own, not a nested function's) is a
///   closed object literal: data properties only, no spread, computed key,
///   duplicate key or `__proto__`, and no `then` (a thenable is assimilated
///   when the async return resolves, so it is not what `lazy` receives).
///   Its `default` property's value, under transparent TypeScript wrappers,
///   is a plain identifier;
/// - that identifier names a proven, synchronous, non-generator component
///   declared directly in the loader: a function declaration or a `const`
///   initialized with the function literal itself, unwritten, and referenced
///   nowhere else;
/// - the call initializes an unexported, unwritten `const`, and every
///   reference to it is a JSX tag name (an opening or closing name, never a
///   dotted one), with at least one opening tag. A `.preload()` would hand
///   the module promise to its caller, and any other use could reach it;
/// - no analyzed file holds a reflective write through a `prototype`, the
///   visible veto ADR 0288 keeps for a fresh literal.
///
/// The module object is then fresh, reachable only by the runtime that
/// selects its `default` and renders it, so the component's only invocations
/// are those renders.
pub(crate) fn lazy_default_render_witnesses<'a>(
    lookup: &SemanticLookup<'a>,
) -> HashSet<(&'a str, Span, Span)> {
    let mut witnesses = HashSet::new();
    for file in lookup.files() {
        for call in &file.ast.calls {
            if let Some((function, property)) = lazy_default_render_witness(file, call, lookup) {
                witnesses.insert((file.path.as_str(), function, property));
            }
        }
    }
    if !witnesses.is_empty()
        && crate::interproc::accessor_provenance::reflective_prototype_write(lookup)
    {
        witnesses.clear();
    }
    witnesses
}

fn lazy_default_render_witness(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<(Span, Span)> {
    use crate::interproc::accessor_provenance::{closed_properties, single_return};
    let primitive = lookup.primitive_at_call(file, call.span)?;
    let slot = lookup
        .dialect
        .renders_loaded_default_argument(primitive, call.arguments.len())?;
    if call.arguments.iter().any(|argument| argument.spread) {
        return None;
    }
    let loader_span = file.ast.peel_ts_sugar_span(call.arguments.get(slot)?.span);
    let loader = file
        .ast
        .functions
        .iter()
        .find(|function| function.span == loader_span)?;
    if !loader.r#async || loader.generator || !lazy_wrapper_only_rendered(file, call) {
        return None;
    }
    let module = file.ast.peel_ts_sugar_span(single_return(file, loader)?);
    let properties = closed_properties(file, module)?;
    // Review round 1: an own `then` makes the module object a thenable. The
    // async loader's return resolves through it, so the promise `lazy` reads
    // `default` from fulfills with whatever `then` delivers, or never.
    // `closed_properties` already refuses getters and computed keys; this
    // refuses a stored function too, including receiver-inert methods.
    if properties.iter().any(|property| {
        file.source_text(property.key)
            .is_some_and(|key| key.trim_matches(['\'', '"']) == "then")
    }) {
        return None;
    }
    let property = properties
        .into_iter()
        .find(|property| file.source_text(property.key) == Some("default"))?;
    if property.shorthand_binding.is_some() {
        return None;
    }
    let value = file.ast.peel_ts_sugar_span(property.value);
    let declaration = file.ast.reference_declaration(value)?;
    // The value's one reference: no registry, call, tag or alias elsewhere.
    if file
        .ast
        .reference_declarations
        .iter()
        .any(|(reference, target)| *target == declaration && *reference != value)
        || crate::indexes::binding_written(file, declaration)
    {
        return None;
    }
    let function = file.ast.functions.iter().find(|function| {
        let named = match function.kind {
            solid_facts::ast::FunctionKind::Declaration => function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration),
            _ => file.ast.bindings.iter().any(|binding| {
                binding.immutable
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.len() == 1
                    && binding.names[0].span == declaration
                    && binding.initializer.is_some_and(|initializer| {
                        file.ast.peel_ts_sugar_span(initializer) == function.span
                    })
            }),
        };
        // Declared in the loader's own body, inside no other function there.
        named
            && loader.body.contains(function.span)
            && !file.ast.functions_within(loader.body).any(|outer| {
                outer.span != function.span
                    && outer.span != loader.span
                    && outer.span.contains(function.span)
            })
    })?;
    (!function.r#async
        && !function.generator
        && lookup.function_value_is_current(file, function)
        && lookup.function_component_status(file, function)
            == crate::indexes::ComponentStatus::Proven)
        .then_some((function.span, property.span))
}

/// Whether the loader call initializes an unexported, unwritten `const` whose
/// every reference is a JSX tag name, with at least one opening tag.
fn lazy_wrapper_only_rendered(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> bool {
    let Some(binding) = file.ast.bindings.iter().find(|binding| {
        binding
            .initializer
            .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == call.span)
    }) else {
        return false;
    };
    if !binding.immutable
        || binding.shape != solid_facts::ast::BindingShape::Identifier
        || binding.names.len() != 1
        || file
            .ast
            .exports
            .iter()
            .any(|export| export.span.contains(binding.declaration))
    {
        return false;
    }
    let name = binding.names[0].span;
    if crate::indexes::binding_written(file, name) {
        return false;
    }
    let mut opened = false;
    for (reference, _) in file
        .ast
        .reference_declarations
        .iter()
        .filter(|(_, target)| *target == name)
    {
        let mut tag = false;
        for element in &file.ast.jsx_elements {
            if element.member_object.is_some() {
                continue;
            }
            if element.name.span == *reference {
                opened = true;
                tag = true;
            } else if element.closing_name == Some(*reference) {
                tag = true;
            }
        }
        if !tag {
            return false;
        }
    }
    opened
}

pub(crate) fn returned_binding_references(
    lookup: &SemanticLookup<'_>,
    bindings: &HashSet<SymbolId>,
) -> HashSet<(String, u64, u64)> {
    bindings
        .iter()
        .flat_map(|symbol| lookup.symbol_references(symbol.as_str()))
        .map(|reference| {
            (
                reference.path.to_string(),
                reference.start_byte,
                reference.end_byte,
            )
        })
        .collect()
}

pub(crate) fn returned_binding_reference(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    bindings: &HashSet<SymbolId>,
    references: &HashSet<(String, u64, u64)>,
) -> bool {
    lookup
        .entities()
        .at(file.path.as_str(), span)
        .is_some_and(|symbol| bindings.contains(symbol))
        || references.contains(&(
            file.path.to_string(),
            u64::from(span.start),
            u64::from(span.end),
        ))
}

/// The name that binds a function declaration, arrow bindings included.
///
/// `function.name` covers `function X() {}` and `method_name` covers a class
/// or object method; neither exists for `const X = () => {}`, whose only name
/// is the binding that initializes it. Exported to the backend because
/// contract emission's attribution ladder must answer "which export is this
/// function" for exactly the same set of declarations the IR does — reading
/// only `name`/`method_name` there made every arrow-bound export unnameable,
/// and an unnameable export silently became "not an export".
pub fn function_binding_name<'a>(
    file: &'a solid_facts::FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
) -> Option<&'a solid_facts::ast::NamedSpan> {
    function
        .name
        .as_ref()
        .or(own_method_name(file, function))
        .or_else(|| {
            file.ast
                .bindings_initializer_containing(function.span)
                .find(|binding| {
                    binding.initializer_function
                        && binding.initializer.is_some_and(|initializer| {
                            file.ast
                                .functions_within(initializer)
                                .max_by_key(|candidate| candidate.span.end - candidate.span.start)
                                .is_some_and(|candidate| candidate.span == function.span)
                        })
                })
                .and_then(|binding| binding.names.first())
        })
}

/// The method name of a function that *is* an object or class method. The
/// facts walker gives every non-arrow function nested anywhere in a method's
/// body that method's name too (`{ m() { register(function () {}) } }`); the
/// method itself is the outermost function carrying that name.
pub(crate) fn own_method_name<'a>(
    file: &'a solid_facts::FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
) -> Option<&'a solid_facts::ast::NamedSpan> {
    let name = function.method_name.as_ref()?;
    (!file.ast.functions.iter().any(|outer| {
        outer.span != function.span
            && outer.span.contains(function.span)
            && outer
                .method_name
                .as_ref()
                .is_some_and(|outer_name| outer_name.span == name.span)
    }))
    .then_some(name)
}

/// Like [`function_binding_name`], but also sees through a call initializer:
/// `const Wrapped = withTheme(({ name }) => ...)` names the arrow `Wrapped`,
/// so a wrapped component is classified like an unwrapped one. Object and
/// array initializers stay excluded — they merely *contain* functions, and
/// naming those would mint components out of data.
pub(crate) fn component_binding_name<'a>(
    file: &'a solid_facts::FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
) -> Option<&'a solid_facts::ast::NamedSpan> {
    function_binding_name(file, function).or_else(|| {
        file.ast
            .bindings_initializer_containing(function.span)
            .find(|binding| {
                binding.call_initializer.is_some()
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.len() == 1
                    && binding.initializer.is_some_and(|initializer| {
                        file.ast
                            .functions_within(initializer)
                            .max_by_key(|candidate| candidate.span.end - candidate.span.start)
                            .is_some_and(|candidate| candidate.span == function.span)
                    })
            })
            .and_then(|binding| binding.names.first())
    })
}

pub(crate) fn binding_returns_reactive_source(
    binding: &solid_facts::ast::BindingFact,
    call: &solid_facts::ast::CallFact,
) -> bool {
    binding.immutable
        && binding.shape == solid_facts::ast::BindingShape::Array
        && binding.array_slots.first().is_some_and(Option::is_some)
        && binding.call_initializer == Some(call.span)
}

pub(crate) fn returned_arrow_function(ast: &solid_facts::ast::AstFacts, span: Span) -> bool {
    ast.functions_within(span)
        .max_by_key(|function| function.span.end - function.span.start)
        .is_some_and(|function| function.kind == solid_facts::ast::FunctionKind::Arrow)
}

/// Whether a reactive read at `span` happens outside the synchronous extent
/// of the function that lexically contains it — so calling that function does
/// not perform the read, and the read must not enter its caller-visible
/// summary.
///
/// The dialect's callback vocabulary answers this exactly, and the three
/// executions divide cleanly. [`solid_dialect::Execution::Inline`] reads
/// "subscribe whatever was tracking at the call site", so they *are* the
/// caller's read and stay. [`solid_dialect::Execution::Tracked`] reads
/// subscribe the callback's own observer, and
/// [`solid_dialect::Execution::Deferred`] reads subscribe nothing the caller
/// owns; either way the caller performs no read, and attributing one to it
/// invents an untracked-read violation in a function whose only read is
/// inside a tracked or deferred callback.
///
/// One `Deferred` slot is the exception, and the owner word says which:
/// a [`solid_dialect::CallbackOwner::InheritsFirstRun`] callback (2.0
/// `createRenderEffect`'s apply) runs its first time during the call with the
/// caller's listener still current, so that run's reads are the caller's.
///
/// The read must sit inside a function *literal* in that argument. An
/// eagerly evaluated argument — `createEffect(count())` — is read while the
/// argument list is built, which is the caller's read after all.
pub(crate) fn read_escapes_synchronous_extent(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> bool {
    file.ast.arguments_containing(span).any(|(call, index)| {
        let argument = &call.arguments[index];
        if !file
            .ast
            .functions_within(argument.span)
            .any(|function| function.span.contains(span))
        {
            return false;
        }
        call_primitive_name(file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                let semantics =
                    dialect.callback_semantics_at(primitive, index, call.arguments.len());
                // A callback the primitive first runs during the call, under the
                // caller's owner and listener, performs its first run's reads
                // inside the caller's synchronous extent -- 2.0
                // `createRenderEffect`'s apply, whose attribution word is
                // `Deferred` for its later runs only. Probed: a memo whose compute
                // creates the render effect re-runs when a signal read only in
                // that apply changes. Dropping the read from the caller's summary
                // would close a `reads` domain over a read the call performs.
                if semantics.owner == Some(solid_dialect::CallbackOwner::InheritsFirstRun) {
                    // The first apply is absent, or unproven, if options can
                    // defer it. Later runs do not read for the creating caller.
                    return dialect
                        .effect_apply_defer_options(primitive, index)
                        .is_some_and(|options| {
                            matches!(
                                crate::call_option_literal(file, call, options, "defer"),
                                solid_dialect::OptionLiteral::True
                                    | solid_dialect::OptionLiteral::Unknown
                            )
                        });
                }
                matches!(
                    semantics.execution,
                    Some(solid_dialect::Execution::Tracked | solid_dialect::Execution::Deferred)
                )
            })
    })
}

/// An exact global scheduler's queued callback cannot read during the
/// enclosing function's invocation. Keep the callback implementation's own
/// summary: the scheduler must be inside `owner_body`, rather than outside
/// the function being read.
pub(crate) fn read_escapes_runtime_stack(
    file: &solid_facts::FileFacts,
    span: Span,
    owner_body: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    crate::execution_role::fresh_stack_callback_call(file, span, lookup).is_some_and(
        |(call, resolved)| {
            owner_body.contains(call.span)
                && crate::runtime_semantics::augmented_global_runtime_call(resolved)
                && !crate::runtime_semantics::dispatches_through_receiver_then(resolved)
        },
    )
}

/// The typed descriptor at a callee, kept only when it names a callable Solid
/// accessor. Tuple signals and object stores are source values but not direct
/// accessor call targets.
pub(crate) fn typed_accessor_descriptor_at<'a>(
    lookup: &SemanticLookup<'a>,
    path: &str,
    callee: Span,
) -> Option<&'a typefacts::TypeDescriptor> {
    lookup
        .smallest_contained_descriptor(path, callee)
        .filter(|descriptor| solid_accessor_declaration(descriptor, lookup.dialect).is_some())
}

/// The exact Solid accessor alias proven by Type Facts. Module plus exported
/// alias identity is required; type text and user-local names are ignored.
pub(crate) fn solid_accessor_declaration<'a>(
    descriptor: &'a typefacts::TypeDescriptor,
    dialect: &dyn Dialect,
) -> Option<&'a typefacts::Declaration> {
    descriptor.alias_declarations.iter().find(|declaration| {
        dialect.type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
            == Some(solid_dialect::TypeRole::Accessor)
    })
}

pub(crate) fn source_function_exported(
    indexes: &ProjectIndexes<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    indexes
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.functions.iter().any(|candidate| {
                candidate.exported
                    && candidate.body.start_byte == u64::from(function.body.start)
                    && candidate.body.end_byte == u64::from(function.body.end)
            })
        })
        || file.ast.exports_containing(function.span).any(|export| {
            !file.ast.functions_within(export.span).any(|candidate| {
                candidate.span != function.span && candidate.span.contains(function.span)
            })
        })
}

pub(crate) fn enclosing_render_function(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    lookup.inside_component(file, span) || lookup.inside_possible_component(file, span)
}

pub(crate) fn function_is_solid_callback(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let primitives = lookup.primitives(file);
    if file.ast.jsx_containing(function.span).any(|element| {
        !file
            .ast
            .functions_within(element.span)
            .any(|outer| outer.span != function.span && outer.span.contains(function.span))
            && jsx_primitive_name(file, element, entities, symbol_names, lookup.dialect)
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| {
                    lookup.dialect.renders_children_through_callback(primitive)
                })
    }) {
        return true;
    }
    let Some(symbol) = function_symbol(file, function, entities) else {
        return false;
    };
    let binding_name = function
        .name
        .as_ref()
        .or_else(|| function_binding_name(file, function))
        .map(|name| file.source_text(name.span).unwrap_or_default());
    file.ast.calls.iter().enumerate().any(|(call_index, call)| {
        known_primitive(&primitives.calls[call_index]).is_some_and(|primitive| {
            call.arguments.iter().enumerate().any(|(index, argument)| {
                lookup
                    .dialect
                    .callback_semantics_at(primitive, index, call.arguments.len())
                    .tracks_reads
                    && argument_references_callback_symbol(
                        file,
                        argument,
                        symbol,
                        entities,
                        symbol_names,
                    )
            })
        })
    }) || file
        .ast
        .jsx_elements
        .iter()
        .enumerate()
        .any(|(element_index, element)| {
            known_primitive(&primitives.jsx[element_index]).is_some_and(|primitive| {
                lookup.dialect.renders_children_through_callback(primitive)
            }) && file.ast.identifiers_within(element.span).any(|identifier| {
                identifier.role == solid_facts::ast::IdentifierRole::Reference
                    && !file.ast.jsx_containing(identifier.span).any(|nested| {
                        nested.span != element.span && element.span.contains(nested.span)
                    })
                    && (entities.get(&location(file.path.shared(), identifier.span))
                        == Some(symbol)
                        || binding_name == file.source_text(identifier.span))
            })
        })
}

pub(crate) fn counts_as_strict_read_root(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> bool {
    matches!(
        execution,
        ExecutionRole::EffectApply | ExecutionRole::UntrackedCallback
    ) || enclosing_render_function(file, span, lookup)
}

pub(crate) fn enclosing_function_label(file: &solid_facts::FileFacts, span: Span) -> String {
    let Some(function) = containing_ast_function(&file.ast, span) else {
        return String::new();
    };
    function_binding_name(file, function).map_or_else(String::new, |name| {
        file.source_text(name.span).unwrap_or_default().to_owned()
    })
}

pub(crate) fn analysis_context(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
    lookup: &SemanticLookup<'_>,
) -> String {
    let enclosing = enclosing_function_label(file, span);
    if let Some(rendering) = file
        .ast
        .functions_body_containing(span)
        .filter(|function| lookup.function_is_component(file, function))
        .filter_map(|function| function_binding_name(file, function).or(function.name.as_ref()))
        .min_by_key(|name| name.span.end - name.span.start)
    {
        return file
            .source_text(rendering.span)
            .unwrap_or_default()
            .to_owned();
    }
    let callback = file
        .ast
        .arguments_containing(span)
        .map(|(call, index)| (call, index, call.arguments[index].span))
        .min_by_key(|(_, _, argument)| argument.end - argument.start);
    if let Some((call, argument, _)) = callback
        && let Some(primitive) = call_primitive_name(file, call, entities, symbol_names, dialect)
    {
        // An owner-transparent inline callback (`untrack`, `flush(fn)`) runs
        // under its call site's owner, so it is described as that call site.
        // It has no label of its own: an anonymous one used to leave the
        // sentence with an empty scope name.
        if let Some(resolved) = primitive.primitive()
            && dialect.callback_preserves_owner_write_context(resolved)
            && dialect
                .callback_semantics_at(resolved, argument, call.arguments.len())
                .execution
                == Some(solid_dialect::Execution::Inline)
        {
            return analysis_context(file, call.span, entities, symbol_names, dialect, lookup);
        }
        // Which phase of a primitive an argument is, asked of the dialect
        // rather than matched here. The pair this had hardcoded is 2.0's:
        // `createEffect(compute, apply)`. 1.x's second argument is a seed
        // value threaded to the next run as `prev`, so a read in it would be
        // described as living in an "apply callback" that 1.x does not have.
        let phase = primitive.primitive().and_then(|resolved| {
            let semantics = dialect.callback_semantics_at(resolved, argument, call.arguments.len());
            semantics.execution.and_then(|execution| match execution {
                // "compute" only where reads actually subscribe: an
                // `onSettled` callback is contract-tracked (the graph
                // schedules it) but imperative to its reads, and calling
                // it a compute would describe the wrong phase.
                solid_dialect::Execution::Tracked if semantics.tracks_reads => Some("compute"),
                // Only an effect's apply slot is an apply phase; a deferred
                // executor's callback keeps its enclosing label. Which slot
                // that is -- or whether the primitive has one -- is the
                // dialect's answer, not a pair named here.
                solid_dialect::Execution::Deferred
                    if dialect.apply_callback_argument(resolved) == Some(argument) =>
                {
                    Some("apply callback")
                }
                // A root body names its root: an anonymous one has no
                // enclosing label to fall back to.
                solid_dialect::Execution::Inline
                    if dialect.callback_runs_in_created_root(resolved, argument) =>
                {
                    Some("callback")
                }
                _ => None,
            })
        });
        if let Some(phase) = phase {
            return format!("{primitive} {phase}");
        }
    }
    enclosing
}

#[cfg(test)]
mod tests {
    use super::{
        OWNER_CONTEXT_COMPONENT_UNCERTAIN, OWNER_CONTEXT_LATER_RUN_UNOWNED, OWNER_CONTEXT_LEAF,
        OWNER_CONTEXT_OWNED, OWNER_CONTEXT_PROVEN_UNOWNED, OWNER_CONTEXT_UNOWNED, OwnerEdgeKind,
        OwnerNode, binding_returns_reactive_source, callback_owner_edge_kind,
        compiler_owner_context, compose_owner_edge, owner_edge_context, propagate_owner_contexts,
        providing_region_chain, returned_arrow_function, root_owned_at, seed_contexts,
    };
    use solid_facts::ast;
    use solid_facts::compiler::{
        COMPILER_FACTS_PROTOCOL, ExecutionMap, OwnershipRegion, OwnershipRegionKind,
    };
    use solid_facts::core::SourceHash;
    use solid_facts::core::Span;

    fn returned_arrow(source: &str) -> bool {
        let source = format!("function outer() {{ return {source}; }}");
        let facts = ast::extract("test.tsx", &source).unwrap();
        facts
            .returns
            .first()
            .and_then(|returned| returned.argument)
            .is_some_and(|argument| returned_arrow_function(&facts, argument))
    }

    #[test]
    fn returned_arrow_identity_comes_from_ast_facts() {
        assert!(returned_arrow("(a = (0)) /* run */ => a"));
        assert!(returned_arrow("async => async"));
        assert!(!returned_arrow("function () { return 1; }"));
    }

    fn binding_accepts(source: &str) -> bool {
        let facts = ast::extract("test.tsx", source).unwrap();
        binding_returns_reactive_source(
            facts.bindings.first().unwrap(),
            facts.calls.first().unwrap(),
        )
    }

    #[test]
    fn returned_source_binding_identity_comes_from_ast_facts() {
        assert!(binding_accepts(
            "const [go] = /* gate */ makeGate<Array<T>>();"
        ));
        assert!(!binding_accepts("let [go] = makeGate();"));
        assert!(!binding_accepts("const [, go] = makeGate();"));
    }

    /// One file with a real AST and a hand-built execution map.
    fn owner_context_file(source: &str, compiler: ExecutionMap) -> solid_facts::FileFacts {
        const PATH: &str = "test.tsx";
        solid_facts::FileFacts {
            generation: solid_facts::core::Generation::new(1).unwrap(),
            path: solid_facts::core::SourcePath::new(PATH).unwrap(),
            source_hash: SourceHash::of(source),
            source: std::sync::Arc::from(source),
            ast: std::sync::Arc::new(ast::extract(PATH, source).unwrap()),
            compiler: std::sync::Arc::new(compiler),
        }
    }

    #[test]
    fn compiler_ownership_regions_seed_owner_analysis() {
        let source = "function run() {}";
        let facts = ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of(source),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![OwnershipRegion {
                span: Span::new(0, 17),
                kind: OwnershipRegionKind::Leaf,
            }],
            callback_roles: vec![],
            jsx_operations: vec![],
        };
        assert_eq!(
            compiler_owner_context(&owner_context_file(source, facts), Span::new(15, 17)),
            OWNER_CONTEXT_LEAF
        );
    }

    #[test]
    fn owner_edge_context_transforms_the_source_bits_per_edge_kind() {
        assert_eq!(
            owner_edge_context(OwnerEdgeKind::Preserve, OWNER_CONTEXT_OWNED),
            OWNER_CONTEXT_OWNED
        );
        assert_eq!(
            owner_edge_context(OwnerEdgeKind::Owned, OWNER_CONTEXT_UNOWNED),
            OWNER_CONTEXT_OWNED
        );
        assert_eq!(
            owner_edge_context(OwnerEdgeKind::Unowned, OWNER_CONTEXT_OWNED),
            OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED
        );
        assert_eq!(
            owner_edge_context(
                OwnerEdgeKind::Preserve,
                OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_COMPONENT_UNCERTAIN,
            ),
            OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_COMPONENT_UNCERTAIN
        );
        assert_eq!(
            owner_edge_context(OwnerEdgeKind::Conditional, 0),
            OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED
        );
        assert_eq!(
            owner_edge_context(OwnerEdgeKind::Leaf, OWNER_CONTEXT_OWNED),
            OWNER_CONTEXT_LEAF
        );
    }

    #[test]
    fn callback_owner_edge_kinds_map_one_to_one() {
        use solid_dialect::CallbackOwner;
        assert_eq!(
            callback_owner_edge_kind(CallbackOwner::Creates),
            OwnerEdgeKind::Owned
        );
        assert_eq!(
            callback_owner_edge_kind(CallbackOwner::Conditional),
            OwnerEdgeKind::Conditional
        );
        assert_eq!(
            callback_owner_edge_kind(CallbackOwner::Inherits),
            OwnerEdgeKind::Preserve
        );
        assert_eq!(
            callback_owner_edge_kind(CallbackOwner::None),
            OwnerEdgeKind::Unowned
        );
        assert_eq!(
            callback_owner_edge_kind(CallbackOwner::Leaf),
            OwnerEdgeKind::Leaf
        );
    }

    #[test]
    fn composing_owner_edges_lets_an_explicit_callback_owner_dominate() {
        assert_eq!(
            compose_owner_edge(OwnerEdgeKind::Unowned, OwnerEdgeKind::Preserve),
            OwnerEdgeKind::Unowned
        );
        assert_eq!(
            compose_owner_edge(OwnerEdgeKind::Unowned, OwnerEdgeKind::Owned),
            OwnerEdgeKind::Owned
        );
        assert_eq!(
            compose_owner_edge(OwnerEdgeKind::Owned, OwnerEdgeKind::Leaf),
            OwnerEdgeKind::Leaf
        );
    }

    fn node(seed_context: u8) -> OwnerNode {
        OwnerNode {
            path: "app.tsx".to_owned(),
            span: Span::new(0, 10),
            body: Span::new(0, 10),
            symbol: None,
            exported: false,
            component: seed_context & OWNER_CONTEXT_OWNED != 0,
            component_uncertain: false,
            seed_context,
        }
    }

    #[test]
    fn semantic_seeds_preserve_proven_owner_states() {
        let nodes = [
            node(OWNER_CONTEXT_OWNED),
            node(OWNER_CONTEXT_UNOWNED),
            node(0),
            node(OWNER_CONTEXT_LEAF),
        ];
        assert_eq!(
            seed_contexts(&nodes),
            vec![
                OWNER_CONTEXT_OWNED,
                OWNER_CONTEXT_UNOWNED,
                0,
                OWNER_CONTEXT_LEAF
            ]
        );
    }

    #[test]
    fn propagation_reaches_a_fixed_point_and_accumulates_bits() {
        // 0 --preserve--> 1 --conditional--> 2, plus a cycle 1 <-> 0.
        let mut contexts = vec![OWNER_CONTEXT_OWNED, 0, OWNER_CONTEXT_LEAF];
        let outgoing = vec![
            vec![(1, OwnerEdgeKind::Preserve)],
            vec![
                (0, OwnerEdgeKind::Preserve),
                (2, OwnerEdgeKind::Conditional),
            ],
            vec![],
        ];
        propagate_owner_contexts(&mut contexts, &outgoing);
        assert_eq!(
            contexts,
            vec![
                OWNER_CONTEXT_OWNED,
                OWNER_CONTEXT_OWNED,
                OWNER_CONTEXT_LEAF | OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED,
            ]
        );
    }

    /// Documents current behavior: only nodes whose seeded context is nonzero
    /// enter the worklist, so even an `Owned` edge (whose contribution does
    /// not depend on the source's bits) stays dormant when its source has no
    /// context of its own.
    #[test]
    fn propagation_does_not_walk_edges_out_of_context_free_nodes() {
        let mut contexts = vec![0, 0];
        let outgoing = vec![vec![(1, OwnerEdgeKind::Owned)], vec![]];
        propagate_owner_contexts(&mut contexts, &outgoing);
        assert_eq!(contexts, vec![0, 0]);
    }

    #[test]
    fn owner_providing_regions_contain_spans_inclusively() {
        let regions = [Span::new(10, 20), Span::new(40, 50)];
        let none = std::iter::empty::<((), Span, Span)>;
        assert_eq!(
            providing_region_chain(&regions, none(), Span::new(12, 18)),
            Some(vec![])
        );
        assert_eq!(
            providing_region_chain(&regions, none(), Span::new(40, 50)),
            Some(vec![])
        );
        assert_eq!(
            providing_region_chain(&regions, none(), Span::new(19, 21)),
            None
        );
        assert_eq!(providing_region_chain(&[], none(), Span::new(0, 0)), None);
    }

    /// `createRoot(() => { createEffect(c, () => { createRoot(() => { OP }) }) })`
    /// as spans: the outer root callback 10..100, the effect apply 20..90, the
    /// inner root callback 30..80, and an operation in each.
    #[test]
    fn providing_region_chain_stops_at_the_innermost_region() {
        let regions = [Span::new(10, 100), Span::new(30, 80)];
        let functions = [
            ("outer-root", Span::new(10, 100), Span::new(15, 100)),
            ("effect-apply", Span::new(20, 90), Span::new(25, 90)),
            ("inner-root", Span::new(30, 80), Span::new(35, 80)),
        ];
        let chain = |span| providing_region_chain(&regions, functions, span);
        assert_eq!(chain(Span::new(16, 18)), Some(vec!["outer-root"]));
        assert_eq!(
            chain(Span::new(26, 28)),
            Some(vec!["outer-root", "effect-apply"])
        );
        // The inner root sits inside the detached apply and answers for what
        // it contains: the apply is not in its chain.
        assert_eq!(chain(Span::new(40, 50)), Some(vec!["inner-root"]));
        let context = |function: &&str| match *function {
            "effect-apply" => OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
            _ => OWNER_CONTEXT_OWNED,
        };
        assert!(root_owned_at(chain(Span::new(16, 18)).as_deref(), context));
        assert!(!root_owned_at(chain(Span::new(26, 28)).as_deref(), context));
        assert!(root_owned_at(chain(Span::new(40, 50)).as_deref(), context));
        assert!(!root_owned_at(None::<&[&str]>, context));
    }

    #[test]
    fn root_owned_at_withholds_for_either_detaching_bit() {
        let chain = [0_u8];
        for (context, answered) in [
            (OWNER_CONTEXT_OWNED, true),
            (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED, true),
            (OWNER_CONTEXT_LEAF, true),
            (0, true),
            (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED, false),
            (
                OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LATER_RUN_UNOWNED,
                false,
            ),
        ] {
            assert_eq!(
                root_owned_at(Some(&chain[..]), |_| context),
                answered,
                "context {context:#b}"
            );
        }
    }
}
