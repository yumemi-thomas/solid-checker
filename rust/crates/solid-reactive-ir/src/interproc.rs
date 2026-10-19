//! Interprocedural reactive-read analysis.
//!
//! Builds the cross-function summary graph, propagates reactive reads through
//! calls/returns/factories, resolves typed accessors, and emits the per-call
//! reactive reads plus the export contract summaries. The orchestrator populates
//! an `InterproceduralContext` parameter object and calls `build` to obtain an
//! `InterproceduralResult`; every stage function here is module-private.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use solid_dialect::{Dialect, Primitive, TrackedCallbackTiming};
use solid_facts::ProjectFacts;
use solid_facts::core::Span;
use typefacts::{CallKind, Callability, Location, ResolvedCallValidity};

use super::runtime_semantics::{
    RuntimeArgumentBehavior, argument_behavior, literal_argument_is_not_callable,
    potentially_callable, proven_array_method_argument_behavior, resolved_parameter,
    retains_argument_value,
};
use super::{
    CallbackSchedule, CallbackSummary, ContractGenerationObligation, EntitySymbols, ExecutionRole,
    FunctionBoundary, FunctionLookup, ProjectIndexes, ReactiveRead, ReactiveSourceKind,
    ReturnSummary, SemanticLookup, StaticDefect, StaticDefectKind, SymbolId,
    allowed_callback_spans, assigned_member_function_contains, call_primitive_name,
    containing_summary_function_indexed, contract_callback_execution, function_indices_by_path,
    function_lookup_for_path, functions_for_path, items_by_containing_function, location,
    location_order, propagate_returned_summary_deltas, propagate_summary_deltas,
    push_contract_callback, push_unique_summary_read, semantic_execution_role,
};
use crate::cache::{
    CachedInterproceduralGraph, CachedInterproceduralResultFile, CachedInterproceduralResults,
    CachedReactiveSource, InterproceduralGraphContribution, InterproceduralGraphTarget,
    InterproceduralResultDependency, InterproceduralResultDependencyState, same_compiler_semantics,
};
use crate::execution_role::{
    callee_callback_timing, control_flow_execution_role, direct_callback_contains,
    host_callback_timing, missing_jsx_census, named_callback_execution_role, read_analysis_context,
    runs_outside_owner_call,
};
use crate::indexes::ComponentStatus;
use crate::owners::{
    containing_ast_function, enclosing_function_label, enclosing_render_function,
    function_binding_name, inside_non_component_function, read_escapes_runtime_stack,
    read_escapes_synchronous_extent, source_function_exported,
};
use crate::pipeline::{parallel_file_results, parallel_slice_results};

#[path = "property_gets.rs"]
mod property_gets;

#[path = "accessor_provenance.rs"]
pub(crate) mod accessor_provenance;

mod parameter_use;
use parameter_use::*;
mod retained_literal;
pub(crate) use retained_literal::*;
mod graph;
use graph::*;
mod obligations;
use obligations::*;
mod return_escape;
use return_escape::*;
mod callback_chain;
pub(crate) use callback_chain::*;
mod structured_returns;
use structured_returns::*;
mod result_view;
pub(crate) use result_view::*;
mod result_reads;
pub(crate) use result_reads::*;
mod reads;
pub(crate) use reads::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SummaryRead {
    pub(super) transported: bool,
    pub(super) symbol: SymbolId,
    pub(super) display: SymbolId,
    pub(super) kind: Option<String>,
    pub(super) declaration: Location,
    pub(super) origin: Location,
    pub(super) origin_context: String,
    /// The symbol of the summary node that *discovered* this read, kept as the
    /// row travels across call edges.
    ///
    /// Identity, not a name. `origin_context` is `nodes[owner].name` and is
    /// exactly what a provenance claim may not be built on: a read discovered
    /// in a private helper names the helper, and two nodes may share a name.
    /// `propagate_summary_deltas` copies a row verbatim, so a read that
    /// reached a node through a call still names the node it was discovered
    /// in, and `contract_export_function` compares this against the node it is
    /// projecting to tell "this export performs the read" from "this export
    /// performs it through its call to that one".
    ///
    /// `None` is "no provenance to state", never "the projecting node's own":
    /// it is the row's fail-closed value, and the projection publishes nothing
    /// for it.
    pub(super) owner: Option<SymbolId>,
    /// ADR 0276: the read is not proven to run on every normal completion of
    /// a call of the function whose summary holds this row. For a row found
    /// in that function's body, its origin is off the completion path; for a
    /// row that reached it over a call edge, no call of that edge is on it.
    pub(super) local: bool,
    /// ADR 0276: some hop below the function whose summary holds this row
    /// was not proven to run on every completion of its caller.
    pub(super) conditional: bool,
    /// Review round 1 of ADRs 0293 and 0294: the row's site may read
    /// reactive state, but nothing proves it does: a store Get whose key is
    /// undecided, or an exempt `then` whose backing may run code, or a call of
    /// a typed candidate whose held body no summary follows. At a caller it is
    /// an uncertifiable read, never a proven one, and no contract publishes it.
    pub(super) undecided: bool,
}

struct DirectReferenceContribution {
    owner: usize,
    read: SummaryRead,
    unique: bool,
}

#[derive(Clone, Default, Eq, PartialEq)]
pub(super) struct SummaryReads {
    pub(super) ordered: Vec<SummaryRead>,
    seen: HashSet<(SymbolId, Location, Location)>,
}

impl SummaryReads {
    fn key(read: &SummaryRead) -> (SymbolId, Location, Location) {
        (
            read.display.clone(),
            read.origin.clone(),
            read.declaration.clone(),
        )
    }

    pub(super) fn push(&mut self, read: SummaryRead) {
        self.seen.insert(Self::key(&read));
        self.ordered.push(read);
    }

    pub(super) fn push_unique(&mut self, read: SummaryRead) -> bool {
        // A row proven on more paths already covers this one (ADR 0276).
        if !self.seen.insert(Self::key(&read))
            && self.ordered.iter().any(|previous| {
                Self::key(previous) == Self::key(&read)
                    && previous.transported == read.transported
                    && (!previous.local || read.local)
                    && (!previous.conditional || read.conditional)
            })
        {
            return false;
        }
        self.ordered.push(read);
        true
    }

    fn take(&mut self) -> Vec<SummaryRead> {
        self.seen.clear();
        std::mem::take(&mut self.ordered)
    }

    pub(super) fn replace(&mut self, reads: Vec<SummaryRead>) {
        self.seen = reads.iter().map(Self::key).collect();
        self.ordered = reads;
    }

    pub(super) fn to_vec(&self) -> Vec<SummaryRead> {
        self.ordered.clone()
    }
}

/// Compare the semantic effect of two implementations without treating their
/// source locations as behavior. Equivalent structural methods often read the
/// same accessor from different lines; those origins remain useful for a
/// diagnostic, but must not make dispatch fail closed.
///
/// The comparison is over the *set* of distinct reads. Reading one accessor
/// twice is the same effect as reading it once, so a differing repeat count
/// must not fail the gate — but a read only one candidate performs must,
/// which a same-length containment test does not catch: `[a, a]` and
/// `[a, b]` are the same length and every member of the first appears in the
/// second. The caller unions every candidate's reads once this returns true,
/// so anything short of set equality would attribute an unproven read.
fn equivalent_summary_reads(left: &SummaryReads, right: &SummaryReads) -> bool {
    type Effect<'r> = (
        &'r SymbolId,
        &'r SymbolId,
        Option<&'r str>,
        &'r Location,
        bool,
        bool,
    );
    fn effect(reads: &SummaryReads) -> Vec<Effect<'_>> {
        reads
            .iter()
            .map(|read| {
                (
                    &read.symbol,
                    &read.display,
                    read.kind.as_deref(),
                    &read.declaration,
                    read.transported,
                    read.undecided,
                )
            })
            .collect()
    }
    let left = effect(left);
    let right = effect(right);
    left.iter().all(|read| right.contains(read)) && right.iter().all(|read| left.contains(read))
}

/// Set equality over callback timing, for the same reason as
/// [`equivalent_summary_reads`]: repeating one parameter's timing is not a
/// different effect, but a parameter only one candidate defers is.
fn equivalent_callbacks(left: &[CallbackSummary], right: &[CallbackSummary]) -> bool {
    /// Parameter, member path, execution, owner and the arguments' JSON.
    type Effect<'c> = (usize, &'c [String], &'c str, Option<&'c str>, String);
    fn effect(callbacks: &[CallbackSummary]) -> HashSet<Effect<'_>> {
        callbacks
            .iter()
            .map(|callback| {
                (
                    callback.parameter,
                    callback.path.as_slice(),
                    callback.execution.as_str(),
                    callback.owner.as_deref(),
                    serde_json::to_string(&callback.arguments)
                        .expect("contract callback arguments are JSON-safe"),
                )
            })
            .collect()
    }
    effect(left) == effect(right)
}

impl std::ops::Deref for SummaryReads {
    type Target = [SummaryRead];

    fn deref(&self) -> &Self::Target {
        &self.ordered
    }
}

#[derive(Clone)]
pub(super) struct SummaryNode {
    pub(super) path: String,
    pub(super) span: Span,
    pub(super) body: Span,
    pub(super) name: Option<String>,
    pub(super) symbol: Option<SymbolId>,
    pub(super) runtime_identity: String,
    pub(super) parameters: Vec<SymbolId>,
    pub(super) exported: bool,
    pub(super) r#async: bool,
    /// ADR 0280: the declaring file's source hash, for results that read
    /// the function's branches against a call's arguments.
    pub(super) source_hash: solid_facts::core::SourceHash,
}

/// A parameter-member invocation and the execution provenance retained for
/// contract generation. A nested callable needs more than its access path to
/// establish a direct call operation (ADR 0013).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParameterMemberInvocation {
    pub(crate) parameter: usize,
    pub(crate) path: Vec<String>,
    pub(crate) in_owner_body: bool,
    /// Exact member callee in the declaring file. Context proofs are refreshed
    /// each generation because a downstream callback consumer may change.
    pub(crate) site: Span,
    /// Type Facts resolved this member call, validly and to one declaration,
    /// to a method of a primitive wrapper in the standard library
    /// (`String.replace`, `Number.toFixed`). Whatever the caller passes, the
    /// implementation that runs is that built-in (ADR 0190).
    pub(crate) primitive_builtin: bool,
    /// The member is invoked in a source-proven context where the dispatch
    /// read rules already exclude a direct call (tracked, event or deferred).
    /// Retain the invocation for identity/dependency comparisons, but do not
    /// project an untracked dispatch question to the helper's caller.
    pub(crate) dispatch_read_excluded: bool,
    /// ADR 0320: the function uses the parameter in a way no member
    /// invocation describes -- an alias, a spread, a wrapper, an unknown
    /// callee, a closure, `arguments`. Whatever member it may invoke, no call
    /// site can resolve it; `path` is empty.
    pub(crate) unmodeled: bool,
}

impl FunctionBoundary for SummaryNode {
    fn path(&self) -> &str {
        &self.path
    }

    fn body(&self) -> Span {
        self.body
    }
}

#[derive(Clone)]
pub(super) struct InterproceduralResult {
    pub(super) reads: Arc<[ReactiveRead]>,
    pub(super) source_call_targets: Arc<HashMap<Location, accessor_provenance::Callable>>,
    pub(super) dispatch_obligations: Arc<[StaticDefect]>,
    pub(super) factory_instances: usize,
    pub(super) timings: InterproceduralTimings,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct InterproceduralTimings {
    pub(super) graph: Duration,
    pub(super) direct_summaries: Duration,
    pub(super) direct_index: Duration,
    pub(super) direct_references: Duration,
    pub(super) propagation: Duration,
    pub(super) returned_direct: Duration,
    pub(super) returned_delta: Duration,
    pub(super) call_summary_delta: Duration,
    pub(super) factory_propagation: Duration,
    pub(super) results_and_exports: Duration,
    pub(super) result_reads: Duration,
    pub(super) export_summaries: Duration,
    pub(super) graph_reused_files: u64,
    pub(super) graph_recomputed_files: u64,
    pub(super) result_reused_files: u64,
    pub(super) result_recomputed_files: u64,
}

#[cfg(test)]
mod tests;
