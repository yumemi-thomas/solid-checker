//! Owner analysis: which computations need a reactive owner, where
//! owners are provided, and the requirement/fix emission around them.

use crate::cache::{CachedLateStages, same_compiler_semantics};
use crate::pipeline::{AnalysisContext, ProgramDraft, parallel_slice_results};
use crate::{
    BuildTimings, ExecutionRole, Fix, FunctionBoundary, OwnerRequirement,
    OwnerRequirementOperation, PrimitiveName, TextEdit, containing_function_indexed,
    function_indices_by_path, jsx_primitive_name, known_primitive, location, primitive_name,
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
use solid_dialect::{Dialect, Primitive};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerEdgeKind {
    Preserve,
    Owned,
    Unowned,
    Conditional,
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
            .map(|call| {
                primitive_name(
                    file.path.as_str(),
                    call.callee,
                    call.static_callee(&file.source),
                    entities,
                    symbol_names,
                    lookup.dialect,
                )
            })
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
            .chain(file.compiler.ownership_regions.iter().filter_map(|region| {
                (region.kind == solid_facts::compiler::OwnershipRegionKind::Owned)
                    .then_some(region.span)
            }))
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
    let mut seed_context = compiler_owner_context(&file.compiler, function.body);
    match component_status {
        crate::indexes::ComponentStatus::Proven => seed_context |= OWNER_CONTEXT_OWNED,
        crate::indexes::ComponentStatus::Uncertain => {
            seed_context |=
                OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_COMPONENT_UNCERTAIN;
        }
        crate::indexes::ComponentStatus::No if exported && name.is_some() => {
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

fn compiler_owner_context(facts: &solid_facts::compiler::ExecutionMap, body: Span) -> u8 {
    facts
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
    pub(crate) settled_target: Option<OwnerTarget>,
    /// For call-site-gated leaf owners (2.0 `onSettled`): the owner call's
    /// span, published as a [`LeafGateDecision`] once the graph settles so
    /// the leaf-operation table can be resolved against real ownership.
    pub(crate) settled_gate: Option<Span>,
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
    pub(crate) runtime_uncertain: bool,
    pub(crate) caller_uncertain: bool,
    pub(crate) conditional_owner: bool,
    pub(crate) component_uncertain: bool,
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
            if let Some(target_index) = entities
                .get(&location(file.path.shared(), call.callee))
                .and_then(|symbol| by_symbol.get(symbol))
                .copied()
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
            ) {
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

    let mut settled_gates = SettledGateDecisions::new();
    for (file_index, file) in facts.files.iter().enumerate() {
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
            let root_owned = inside_owner_providing_region(
                &owner_file_indexes[file_index].providing_regions,
                call.span,
            );
            if !root_owned
                && let Some(symbol) = lookup.callee_symbol(file, call.callee)
                && let Some(requirements_for_call) = lookup.contract_owner_requirements(symbol)
            {
                let proven_unowned = context & OWNER_CONTEXT_PROVEN_UNOWNED != 0;
                let conditional_owner = !proven_unowned
                    && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                        == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
                let component_uncertain = context & OWNER_CONTEXT_COMPONENT_UNCERTAIN != 0;
                for requirement in requirements_for_call {
                    let operation = match requirement.operation {
                        crate::OwnerRequirementOperation::Effect => "effect",
                        crate::OwnerRequirementOperation::Cleanup => "cleanup",
                        crate::OwnerRequirementOperation::Boundary => "boundary",
                        crate::OwnerRequirementOperation::SettledCleanup => "settled-cleanup",
                    };
                    push_owner_requirement(
                        &mut requirements,
                        &mut seen,
                        operation,
                        file.path.as_str(),
                        call.span,
                        OwnerRequirementStatus {
                            uncertain: conditional_owner || component_uncertain,
                            runtime_uncertain: false,
                            caller_uncertain: false,
                            conditional_owner,
                            component_uncertain,
                            report: context & OWNER_CONTEXT_UNOWNED != 0,
                        },
                    );
                }
            }
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
            let operation = match primitive {
                // `createRenderEffect` is deliberately included alongside
                // `createEffect`: both register a computation on the owner,
                // and 2.0's render effect outside any owner leaks the same
                // way. The engine matched only `createEffect` and
                // `createTrackedEffect` before the dialect extraction; that
                // omission was the gap, not the rule.
                Some(
                    primitive @ (Primitive::CreateEffect
                    | Primitive::CreateRenderEffect
                    | Primitive::CreateTrackedEffect),
                ) if !root_owned => {
                    let registration =
                        crate::effect_api::classify_effect_call(file, call, primitive, lookup)
                            .owner_registration;
                    (registration != ProofStatus::No).then_some((
                        "effect",
                        context & OWNER_CONTEXT_UNOWNED != 0,
                        registration == ProofStatus::Uncertain,
                    ))
                }
                Some(Primitive::OnCleanup) if !root_owned => Some((
                    "cleanup",
                    context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                    false,
                )),
                Some(Primitive::OnSettled) if !root_owned => {
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
                let conditional_owner = !component_uncertain
                    && !proven_unowned
                    && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                        == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
                let caller_uncertain = !proven_unowned
                    && owner_index.is_some_and(|index| {
                        nodes[index].exported
                            && contexts[index] & OWNER_CONTEXT_UNOWNED != 0
                            && !nodes[index].component
                            && !nodes[index].component_uncertain
                    });
                let uncertain = runtime_uncertain
                    || conditional_owner
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
                    file.path.as_str(),
                    operation_span,
                    OwnerRequirementStatus {
                        uncertain,
                        runtime_uncertain,
                        caller_uncertain,
                        conditional_owner,
                        component_uncertain,
                        report,
                    },
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
            if inside_owner_providing_region(
                &owner_file_indexes[file_index].providing_regions,
                element.span,
            ) {
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
            let conditional_owner = !component_uncertain
                && !proven_unowned
                && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                    == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
            push_owner_requirement(
                &mut requirements,
                &mut seen,
                "boundary",
                file.path.as_str(),
                Span::new(element.span.start, element.name.span.end),
                OwnerRequirementStatus {
                    uncertain: conditional_owner || component_uncertain,
                    runtime_uncertain: false,
                    caller_uncertain: false,
                    conditional_owner,
                    component_uncertain,
                    report: context & OWNER_CONTEXT_UNOWNED != 0,
                },
            );
        }
    }
    (requirements, settled_gates)
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
        if let Some(symbol) = entities.get(&location(file.path.shared(), call.callee)) {
            edges.push(SymbolicOwnerEdge {
                source: owner.map(|span| OwnerSource {
                    path: file.path.to_string(),
                    span,
                }),
                target: OwnerTarget::Symbol(symbol.clone()),
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
        if inside_owner_providing_region(&providing_regions, call.span) {
            continue;
        }
        if let Some(symbol) = lookup.callee_symbol(file, call.callee)
            && let Some(contract_requirements) = lookup.contract_owner_requirements(symbol)
        {
            for requirement in contract_requirements {
                let operation = match requirement.operation {
                    crate::OwnerRequirementOperation::Effect => "effect",
                    crate::OwnerRequirementOperation::Cleanup => "cleanup",
                    crate::OwnerRequirementOperation::Boundary => "boundary",
                    crate::OwnerRequirementOperation::SettledCleanup => "settled-cleanup",
                };
                requirements.push(OwnerRequirementCandidate {
                    operation,
                    operation_span: call.span,
                    owner,
                    report_mask: OWNER_CONTEXT_UNOWNED,
                    allow_uncertain: true,
                    runtime_uncertain: false,
                    settled_target: None,
                    settled_gate: None,
                });
            }
        }
        let operation = match known_primitive(&call_primitives[call_index]) {
            // See the batch owner pass: `createRenderEffect` belongs with the
            // other effect constructors, and its earlier absence there was
            // the two passes' drift, not a narrower contract.
            Some(
                primitive @ (Primitive::CreateEffect
                | Primitive::CreateRenderEffect
                | Primitive::CreateTrackedEffect),
            ) => {
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
            Some(Primitive::OnCleanup) => Some((
                "cleanup",
                OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF,
                None,
                call.callee,
                false,
            )),
            Some(Primitive::OnSettled) => Some((
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
                settled_target,
                settled_gate,
            });
        }
    }
    for callback in &file.compiler.callback_roles {
        if matches!(
            callback.role,
            solid_facts::compiler::CallbackRoleKind::EventHandler
                | solid_facts::compiler::CallbackRoleKind::DirectiveApply
        ) && let Some(target) = callback_target(callback.span)
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
            && !inside_owner_providing_region(&providing_regions, element.span)
        {
            requirements.push(OwnerRequirementCandidate {
                operation: "boundary",
                operation_span: Span::new(element.span.start, element.name.span.end),
                owner: owner_at(element.span),
                report_mask: OWNER_CONTEXT_UNOWNED,
                allow_uncertain: false,
                runtime_uncertain: false,
                settled_target: None,
                settled_gate: None,
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
    let mut settled_gates = SettledGateDecisions::new();
    let mut seen = HashSet::new();
    for file in &facts.files {
        let Some(fragment) = cache.get(file.path.as_str()) else {
            continue;
        };
        for candidate in &fragment.requirements {
            if let Some(gate) = candidate.settled_gate {
                let owner_index = candidate.owner.and_then(|span| {
                    nodes_by_span
                        .get(file.path.as_str())
                        .and_then(|nodes| nodes.get(&span))
                        .copied()
                });
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
            let owner_index = candidate.owner.and_then(|span| {
                nodes_by_span
                    .get(file.path.as_str())
                    .and_then(|nodes| nodes.get(&span))
                    .copied()
            });
            let context = owner_index.map_or(OWNER_CONTEXT_UNOWNED, |index| contexts[index]);
            let proven_unowned = context & OWNER_CONTEXT_PROVEN_UNOWNED != 0;
            let component_uncertain = !proven_unowned
                && (context & OWNER_CONTEXT_COMPONENT_UNCERTAIN != 0
                    || owner_index.is_some_and(|index| nodes[index].component_uncertain));
            let conditional_owner = !component_uncertain
                && !proven_unowned
                && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                    == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
            let caller_uncertain = candidate.allow_uncertain
                && !proven_unowned
                && owner_index.is_some_and(|index| {
                    nodes[index].exported
                        && contexts[index] & OWNER_CONTEXT_UNOWNED != 0
                        && !nodes[index].component
                        && !nodes[index].component_uncertain
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
            let uncertain =
                runtime_uncertain || conditional_owner || caller_uncertain || component_uncertain;
            push_owner_requirement(
                &mut requirements,
                &mut seen,
                candidate.operation,
                file.path.as_str(),
                candidate.operation_span,
                OwnerRequirementStatus {
                    uncertain,
                    runtime_uncertain,
                    caller_uncertain,
                    conditional_owner,
                    component_uncertain,
                    report: context & candidate.report_mask != 0,
                },
            );
        }
    }
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

pub(crate) fn inside_owner_providing_region(providing_regions: &[Span], span: Span) -> bool {
    providing_regions
        .iter()
        .any(|argument| argument.contains(span))
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
        OwnerEdgeKind::Leaf => OWNER_CONTEXT_LEAF,
    }
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

pub(crate) fn push_owner_requirement(
    requirements: &mut Vec<OwnerRequirement>,
    seen: &mut HashSet<(String, u64, u64, String)>,
    operation: &str,
    path: &str,
    span: Span,
    status: OwnerRequirementStatus,
) {
    let location = location(path, span);
    if seen.insert((
        location.path.to_string(),
        location.start_byte,
        location.end_byte,
        operation.into(),
    )) {
        requirements.push(OwnerRequirement {
            operation: crate::OwnerRequirementOperation::from_internal(operation),
            location,
            uncertain: status.uncertain,
            runtime_uncertain: status.runtime_uncertain,
            caller_uncertain: status.caller_uncertain,
            conditional_owner: status.conditional_owner,
            component_uncertain: status.component_uncertain,
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
    // Package contracts can describe callback owner behavior that is not
    // available from the consumer's declarations. A present owner row is an
    // exact runtime claim; an omitted row remains the legacy timing-only
    // contract and must not be treated as inherited-owner proof.
    if let Some(symbol) = lookup.callee_symbol(file, call.callee)
        && let Some(callbacks) = lookup.contract_callbacks(symbol)
    {
        let contracted = callbacks
            .iter()
            .filter_map(|callback| {
                call.arguments.get(callback.parameter)?;
                let owner = callback.owner.as_deref()?;
                Some(OwnerCallbackEdge {
                    argument: callback.parameter,
                    kind: contract_callback_owner_edge_kind(owner)?,
                    source_path: file.path.to_string(),
                    source: call.span,
                })
            })
            .collect::<Vec<_>>();
        if !contracted.is_empty() {
            return contracted;
        }
    }
    let Some(primitive) = known_primitive(primitive) else {
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

pub(crate) const fn callback_owner_edge_kind(owner: solid_dialect::CallbackOwner) -> OwnerEdgeKind {
    match owner {
        solid_dialect::CallbackOwner::Creates => OwnerEdgeKind::Owned,
        solid_dialect::CallbackOwner::Conditional => OwnerEdgeKind::Conditional,
        solid_dialect::CallbackOwner::Inherits => OwnerEdgeKind::Preserve,
        solid_dialect::CallbackOwner::None => OwnerEdgeKind::Unowned,
        solid_dialect::CallbackOwner::Leaf => OwnerEdgeKind::Leaf,
    }
}

fn contract_callback_owner_edge_kind(owner: &str) -> Option<OwnerEdgeKind> {
    Some(match owner {
        "created" => OwnerEdgeKind::Owned,
        "conditional" => OwnerEdgeKind::Conditional,
        "inherited" => OwnerEdgeKind::Preserve,
        "unowned" => OwnerEdgeKind::Unowned,
        "leaf" => OwnerEdgeKind::Leaf,
        _ => return None,
    })
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
            let owner = primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
                entities,
                symbol_names,
                lookup.dialect,
            )?;
            owner
                .primitive()
                .is_some_and(|primitive| {
                    callback_owner_at_call(file, call, primitive, index, lookup)
                        == Some(solid_dialect::CallbackOwner::Leaf)
                })
                .then(|| owner.to_string())
        })
}

pub(crate) fn read_is_under_loading(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> bool {
    let entities = lookup.entities();
    if file.ast.jsx_containing(span).any(|element| {
        jsx_element_is_loading(file, element, entities, symbol_names, lookup.dialect)
    }) {
        return true;
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
        return true;
    }
    let Some(owner) = containing_ast_function(&file.ast, span) else {
        return false;
    };
    // For call sites whose target matched (file, owner), the "wrapper" the
    // second branch resolves is the owner itself, so the caller scan
    // distributes into: a Loading-wrapped call site exists, or any call site
    // exists and the owner's own body renders a Loading element.
    let call_sites = lookup.jsx_call_site_loading(file.path.as_str(), owner.span);
    call_sites.loading_wrapped
        || (call_sites.any
            && file.ast.jsx_within(owner.body).any(|candidate| {
                jsx_element_is_loading(file, candidate, entities, symbol_names, lookup.dialect)
            }))
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
    if lookup
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.async_functions.iter().any(|function| {
                function.can_return_async
                    && u64::from(argument.start) <= function.expression.start_byte
                    && function.expression.end_byte <= u64::from(argument.end)
            })
        })
    {
        return true;
    }
    file.ast
        .functions_within(argument)
        .max_by_key(|function| function.span.end - function.span.start)
        .is_some_and(|function| function.r#async)
}

/// Whether local Type Facts or an imported package contract proves that a
/// computation may return a Promise or AsyncIterable.
pub(crate) fn computation_is_async_with_contracts(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    argument: Span,
    contracted: &HashMap<SymbolId, crate::contracts::ResolvedContractBinding>,
) -> bool {
    if computation_is_async(lookup, file, argument) {
        return true;
    }
    let contracted_async_at = |span| {
        lookup
            .entities()
            .at(file.path.as_str(), span)
            .and_then(|symbol| contracted.get(symbol))
            .is_some_and(|binding| !binding.summary.async_behavior.is_empty())
    };
    if contracted_async_at(argument) {
        return true;
    }
    let Some(function) = file
        .ast
        .functions_within(argument)
        .max_by_key(|function| function.span.end - function.span.start)
    else {
        return false;
    };
    function
        .expression_return
        .as_ref()
        .and_then(|returned| returned.callee)
        .is_some_and(contracted_async_at)
        || file.ast.returns_within(function.body).any(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
                && returned.callee.is_some_and(contracted_async_at)
        })
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
            && !lookup.function_may_be_component(file, function)
    })
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
                    || (semantics.execution.is_some()
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

pub(crate) fn function_binding_name<'a>(
    file: &'a solid_facts::FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
) -> Option<&'a solid_facts::ast::NamedSpan> {
    function
        .name
        .as_ref()
        .or(function.method_name.as_ref())
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
        primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
            dialect,
        )
        .as_ref()
        .and_then(PrimitiveName::primitive)
        .is_some_and(|primitive| {
            matches!(
                dialect
                    .callback_semantics_at(primitive, index, call.arguments.len())
                    .execution,
                Some(solid_dialect::Execution::Tracked | solid_dialect::Execution::Deferred)
            )
        })
    })
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
        && let Some(primitive) = primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
            dialect,
        )
    {
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
                // Only an effect's deferred argument is an apply phase; a
                // deferred executor's callback keeps its enclosing label.
                solid_dialect::Execution::Deferred
                    if matches!(
                        resolved,
                        Primitive::CreateEffect | Primitive::CreateRenderEffect
                    ) =>
                {
                    Some("apply callback")
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
        OWNER_CONTEXT_COMPONENT_UNCERTAIN, OWNER_CONTEXT_LEAF, OWNER_CONTEXT_OWNED,
        OWNER_CONTEXT_PROVEN_UNOWNED, OWNER_CONTEXT_UNOWNED, OwnerEdgeKind, OwnerNode,
        binding_returns_reactive_source, callback_owner_edge_kind, compiler_owner_context,
        compose_owner_edge, inside_owner_providing_region, owner_edge_context,
        propagate_owner_contexts, returned_arrow_function, seed_contexts,
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

    #[test]
    fn compiler_ownership_regions_seed_owner_analysis() {
        let facts = ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of("function run() {}"),
            tracked_regions: vec![],
            untracked_regions: vec![],
            ownership_regions: vec![OwnershipRegion {
                span: Span::new(0, 17),
                kind: OwnershipRegionKind::Leaf,
            }],
            callback_roles: vec![],
            jsx_operations: vec![],
        };
        assert_eq!(
            compiler_owner_context(&facts, Span::new(15, 17)),
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
        assert!(inside_owner_providing_region(&regions, Span::new(12, 18)));
        assert!(inside_owner_providing_region(&regions, Span::new(40, 50)));
        assert!(!inside_owner_providing_region(&regions, Span::new(19, 21)));
        assert!(!inside_owner_providing_region(&[], Span::new(0, 0)));
    }
}
