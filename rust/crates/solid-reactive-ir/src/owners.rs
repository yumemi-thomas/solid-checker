//! Owner analysis: which computations need a reactive owner, where
//! owners are provided, and the requirement/fix emission around them.

use crate::cache::{CachedLateStages, same_compiler_semantics};
use crate::pipeline::{AnalysisContext, ProgramDraft, parallel_slice_results};
use crate::{
    BuildTimings, ExecutionRole, Fix, FunctionBoundary, OwnerRequirement,
    OwnerRequirementOperation, PrimitiveName, TextEdit, containing_function_indexed,
    function_indices_by_path, jsx_primitive_name, known_primitive, location,
};

use crate::{HashMap, HashSet};
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::cleanup::{CleanupReturnProof, function_cleanup_return_proof};
use crate::effect_api::ProofStatus;
use crate::execution_role::{argument_references_callback_symbol, execution_role, function_symbol};
use crate::identity::SymbolId;
use crate::indexes::{EntitySymbols, ProjectIndexes, SemanticLookup};
use solid_dialect::{Dialect, OwnerRequirementRole, Primitive};
use solid_facts::ProjectFacts;
use solid_facts::core::{SourceHash, Span};

mod owner_graph;
pub(crate) use owner_graph::*;
mod missing_owners;
pub(crate) use missing_owners::*;
mod context_census;
pub(crate) use context_census::*;
mod owner_state;
use owner_state::{HandOuts, OwnerGraph, PathId};
mod owner_emission;
pub(crate) use owner_emission::Emission;
use owner_emission::{EmissionInputs, emit_file};
mod owner_discovery;
pub(crate) use owner_discovery::*;
mod owner_context;
pub(crate) use owner_context::*;
mod async_context;
pub(crate) use async_context::*;
mod callback_owners;
pub(crate) use callback_owners::*;
mod bindings;
pub use bindings::*;

/// Runs the project-level owner fixed point, including its retained-cache
/// policy, and appends the resulting requirements to the draft.
pub(crate) fn collect_project(
    ctx: &AnalysisContext<'_>,
    project_indexes: &ProjectIndexes<'_>,
    unchanged_paths: &HashSet<String>,
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
        // The retained requirements and gates read what this computation and
        // the reused fragments read ([`CachedLateStages::reads`]).
        let ((gates, owner_timings, fragment_reads), reads) = crate::base::reads::record(|| {
            let (requirements, gates, owner_timings, fragment_reads) =
                find_missing_owners_incremental(
                    ctx.facts,
                    ctx.semantic_lookup,
                    project_indexes,
                    ctx.symbol_names,
                    unchanged_paths,
                    &mut cache.owners,
                    true,
                );
            draft.missing_owners.extend(requirements);
            apply_settled_gates(&mut draft.leaf_operations, &gates);
            apply_settled_requirement_gates(ctx.semantic_lookup, &mut draft.missing_owners, &gates);
            (gates, owner_timings, fragment_reads)
        });
        cache.reads.extend(&reads);
        cache.reads.extend(&fragment_reads);
        timings.absorb_owner(&owner_timings);
        cache.missing_owners = Some(draft.missing_owners.clone());
        cache.settled_gates = Some(gates);
    } else {
        // A build with no retained cache runs the same pass over a scratch
        // state: one owner analysis, so a fresh and a retained build cannot
        // disagree (the duplicate batch pass drifted, S7f-0).
        let (requirements, gates, owner_timings, _) = find_missing_owners_incremental(
            ctx.facts,
            ctx.semantic_lookup,
            project_indexes,
            ctx.symbol_names,
            unchanged_paths,
            &mut OwnerState::default(),
            false,
        );
        draft.missing_owners.extend(requirements);
        apply_settled_gates(&mut draft.leaf_operations, &gates);
        apply_settled_requirement_gates(ctx.semantic_lookup, &mut draft.missing_owners, &gates);
        timings.absorb_owner(&owner_timings);
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
    ast.innermost_function_body_containing(span)
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
/// The leaf body ([`OWNER_CONTEXT_LEAF`]) is created under a children-capable
/// owner, so the body's own owner is the children-forbidden leaf owner: an
/// `onCleanup` there throws (SC3001's claim) and a nested `onSettled` is queued
/// out of band. A run-owned path ([`OWNER_CONTEXT_OWNED`]) is children-capable
/// and carries no leaf shape, so the owner gate can tell a conditional leaf
/// body from a plain one. A leaf body created unowned never carries this bit.
pub(crate) const OWNER_CONTEXT_LEAF_OWNED: u8 = 64;
/// A synchronous owner may have been left by an await on the call path.
pub(crate) const OWNER_CONTEXT_MAY_RESUME_UNOWNED: u8 = 128;

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
/// proven violation one call-edge away. A leaf body the open-world bit creates
/// keeps the bare bit (`LEAF` is the leaf's shape, not a proof), so `LEAF` is
/// not part of the test; a creation-owned leaf body is not open-world.
const fn open_world_unowned_only(context: u8, node: &OwnerNode) -> bool {
    !node.component
        && !node.component_uncertain
        && context
            & (OWNER_CONTEXT_OWNED
                | OWNER_CONTEXT_UNOWNED
                | OWNER_CONTEXT_PROVEN_UNOWNED
                | OWNER_CONTEXT_LATER_RUN_UNOWNED
                | OWNER_CONTEXT_LEAF_OWNED)
            == OWNER_CONTEXT_UNOWNED
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerEdgeKind {
    Preserve,
    /// The source owner on an unsuspended path, or no owner after suspension.
    MayResumeUnowned,
    Owned,
    Unowned,
    Conditional,
    /// The source's context on the callback's first run, and possibly no
    /// owner on a later one.
    InheritsFirstRun,
    Leaf,
    /// A ref application runs the callback with no owner, which is proven
    /// unowned -- unless the callback is a generator. Calling a generator
    /// returns an iterator and runs none of its body, so the target resolves
    /// to [`OwnerEdgeKind::MayBeUnowned`] there.
    RefApplied,
    /// The callback may run with no owner, and nothing proves it: the bare
    /// open-world unowned bit, a caller obligation that stays uncertifiable.
    MayBeUnowned,
    /// A function a ref expression calls while the expression evaluates
    /// (ADR 0345). It runs under its enclosing function's owner, as
    /// [`OwnerEdgeKind::Preserve`] does, and a source with no owner context
    /// settles to the uncertain unowned bit ([`settle_eager_owners`]).
    EagerCall,
    /// The callback of a leaf owner that `onSettled` is, when the call sits in
    /// another leaf body (the dialect's `leaf_owner_requires_owned_call_site`).
    /// rc.13 queues `onSettled` under a children-forbidden owner, so the callback
    /// runs with no owner whatever the enclosing leaf's owner: it is proven
    /// unowned. Any other leaf callback takes [`OwnerEdgeKind::Leaf`].
    QueuedLeaf,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnerNode {
    pub(crate) path: String,
    pub(crate) span: Span,
    pub(crate) body: Span,
    pub(crate) symbol: Option<SymbolId>,
    pub(crate) exported: bool,
    /// A generator's body runs only when its iterator is iterated, never on
    /// a call, so a ref application proves nothing about its owner.
    pub(crate) generator: bool,
    pub(crate) component: bool,
    pub(crate) component_uncertain: bool,
    pub(crate) seed_context: u8,
    /// An exported, named, non-component function of a closed program. Its
    /// unknown-caller seed waits for the namespace census
    /// ([`seed_namespace_hand_outs`], ADR 0347), which is not cached per file.
    pub(crate) namespace_candidate: bool,
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
                crate::call_primitive_name_cached(
                    lookup,
                    file,
                    call,
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

#[cfg(test)]
mod tests;
