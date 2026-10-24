//! Interprocedural reactive-read analysis.
//!
//! Builds the cross-function summary graph, propagates reactive reads through
//! calls/returns/factories, resolves typed accessors, and emits the per-call
//! reactive reads plus the export summaries. The orchestrator populates
//! an `InterproceduralContext` parameter object and calls `build` to obtain an
//! `InterproceduralResult`; every stage function here is module-private.

use crate::{HashMap, HashSet};
use std::{
    collections::BTreeMap,
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
    CallbackSchedule, CallbackSummary, EntitySymbols, ExecutionRole, FunctionBoundary,
    FunctionLookup, ProjectIndexes, ReactiveRead, ReactiveSourceKind, ReturnSummary,
    SemanticLookup, StaticDefect, StaticDefectKind, SymbolId, UnresolvedCallbackObligation,
    allowed_callback_spans, assigned_member_function_contains, callback_execution_word,
    containing_summary_function_indexed, function_indices_by_path, function_lookup_for_path,
    functions_for_path, items_by_containing_function, location, location_order,
    propagate_returned_summary_deltas, propagate_summary_deltas, push_callback_summary,
    push_unique_summary_read, semantic_execution_role,
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
pub(crate) mod property_gets;

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
mod transport_reads;
pub(crate) use result_reads::*;
pub(crate) use transport_reads::{AccessorTable, SourceKindTable, TransportReads};
mod node_index;
mod reads;
mod refresh;
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
    /// in, and a consumer compares this against the node it is projecting to tell
    /// "this export performs the read" from "this export performs it through
    /// its call to that one".
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
    /// an uncertifiable read, never a proven one, and no summary publishes it.
    pub(super) undecided: bool,
}

struct DirectReferenceContribution {
    owner: usize,
    read: SummaryRead,
    unique: bool,
}

/// One function's summary rows as a canonical set (ADR 0395).
///
/// Rows that share a key `(display, origin, declaration, transported)` are
/// the same read reached on different paths. The set keeps, per key, the rows
/// no other row of the key dominates, where a row dominates another when it
/// is no worse on each of `local`, `conditional` and [`canonical_rank`]
/// (lower is better on all three). The value is therefore a function of the
/// rows offered, not of the order they arrived in, and union is its join.
///
/// Dominance is a product order on purpose. Every transfer a propagation
/// applies (`conditional |= local; local = !guaranteed; transported |= t`,
/// one more hop) is monotone in each component, so it preserves dominance:
/// a row the set drops would only ever have produced rows the survivor's
/// transfer dominates. Ranking ties of equal flags alone (a total order
/// consulted only when the flags are equal) would not be preserved: two rows
/// with incomparable flags can transfer to equal flags, and the one the
/// source dropped could then win at the caller.
///
/// `ordered` holds the set in canonical order ([`canonical_order`]), so the
/// first row a consumer finds is the canonical first row. `log` is every row
/// the set ever accepted, in arrival order, with its hop count: delta
/// propagation reads it by position, and a row it carries that the set has
/// since dropped is dominated at the receiver too.
#[derive(Clone, Default)]
pub(super) struct SummaryReads {
    ordered: Vec<SummaryRead>,
    /// `hops[i]` is the edge count from `ordered[i]`'s seed.
    hops: Vec<u32>,
    log: Vec<(u32, SummaryRead)>,
    keys: HashMap<SummaryKey, u32>,
}

type SummaryKey = (SymbolId, Location, Location, bool);

impl PartialEq for SummaryReads {
    fn eq(&self, other: &Self) -> bool {
        self.ordered == other.ordered && self.hops == other.hops
    }
}

impl Eq for SummaryReads {}

/// The rank dominance compares, after the flags: hop count, then the read's
/// location fields, then the rest of the row. `transported` is part of the
/// key, `local` and `conditional` are the flags.
///
/// The hop count leads because it is order-free (the fewest edges from the
/// row's seed) and because it keeps the explanation nearest the call: a
/// callee's own read outranks one it reaches through a further helper. Step 0
/// measured the alternative, location order alone, against the rc17 corpus:
/// it moved three related locations onto a deeper helper's read, where the
/// hop order moved none (ADR 0395).
fn canonical_rank(
    (left_hops, left): (u32, &SummaryRead),
    (right_hops, right): (u32, &SummaryRead),
) -> std::cmp::Ordering {
    left_hops
        .cmp(&right_hops)
        .then_with(|| location_order(&left.origin, &right.origin))
        .then_with(|| location_order(&left.declaration, &right.declaration))
        .then_with(|| left.display.cmp(&right.display))
        .then_with(|| left.symbol.cmp(&right.symbol))
        .then_with(|| left.kind.cmp(&right.kind))
        .then_with(|| left.origin_context.cmp(&right.origin_context))
        .then_with(|| left.owner.cmp(&right.owner))
        .then_with(|| left.undecided.cmp(&right.undecided))
}

/// The total order of `ordered`: the rank, then the key's flag, then the
/// flags.
fn canonical_order(left: (u32, &SummaryRead), right: (u32, &SummaryRead)) -> std::cmp::Ordering {
    canonical_rank(left, right)
        .then_with(|| left.1.transported.cmp(&right.1.transported))
        .then_with(|| left.1.local.cmp(&right.1.local))
        .then_with(|| left.1.conditional.cmp(&right.1.conditional))
}

/// `left` dominates `right`: the same key, and no worse on either flag or on
/// the rank. A row dominates an identical one.
fn summary_dominates(left: (u32, &SummaryRead), right: (u32, &SummaryRead)) -> bool {
    (!left.1.local || right.1.local)
        && (!left.1.conditional || right.1.conditional)
        && canonical_rank(left, right).is_le()
}

impl SummaryReads {
    fn key(read: &SummaryRead) -> SummaryKey {
        (
            read.display.clone(),
            read.origin.clone(),
            read.declaration.clone(),
            read.transported,
        )
    }

    /// Offer a seed row (no hop below it).
    pub(super) fn push(&mut self, read: SummaryRead) {
        self.insert(read, 0);
    }

    /// Offer a seed row; whether the set changed.
    pub(super) fn push_unique(&mut self, read: SummaryRead) -> bool {
        self.insert(read, 0)
    }

    /// Offer a row `hops` edges from its seed; whether the set changed.
    pub(super) fn insert(&mut self, read: SummaryRead, hops: u32) -> bool {
        let key = Self::key(&read);
        if let Some(count) = self.keys.get(&key).copied() {
            let same_key = |row: &SummaryRead| {
                row.transported == read.transported
                    && row.display == read.display
                    && row.origin == read.origin
                    && row.declaration == read.declaration
            };
            if self.ordered.iter().zip(&self.hops).any(|(row, &row_hops)| {
                same_key(row) && summary_dominates((row_hops, row), (hops, &read))
            }) {
                return false;
            }
            let mut removed = 0;
            let mut index = 0;
            while index < self.ordered.len() {
                if same_key(&self.ordered[index])
                    && summary_dominates((hops, &read), (self.hops[index], &self.ordered[index]))
                {
                    self.ordered.remove(index);
                    self.hops.remove(index);
                    removed += 1;
                } else {
                    index += 1;
                }
            }
            self.keys.insert(key, count - removed + 1);
        } else {
            self.keys.insert(key, 1);
        }
        let position = self
            .ordered
            .iter()
            .zip(&self.hops)
            .position(|(row, &row_hops)| canonical_order((hops, &read), (row_hops, row)).is_lt())
            .unwrap_or(self.ordered.len());
        self.ordered.insert(position, read.clone());
        self.hops.insert(position, hops);
        self.log.push((hops, read));
        true
    }

    /// The rows with their hop counts, in canonical order.
    pub(super) fn rows(&self) -> impl Iterator<Item = (u32, &SummaryRead)> {
        self.hops.iter().copied().zip(&self.ordered)
    }

    /// How many rows the set has accepted so far, dropped ones included.
    pub(super) fn log_len(&self) -> usize {
        self.log.len()
    }

    /// The rows accepted since `start`, for delta propagation.
    pub(super) fn log_since(&self, start: usize) -> &[(u32, SummaryRead)] {
        &self.log[start..]
    }

    /// Empty the set and return its seed rows. Only valid before any
    /// propagation: it forgets hop counts.
    fn take(&mut self) -> Vec<SummaryRead> {
        debug_assert!(self.hops.iter().all(|hops| *hops == 0));
        let reads = std::mem::take(&mut self.ordered);
        *self = Self::default();
        reads
    }

    /// Replace the set by the canonical set of these seed rows.
    pub(super) fn replace(&mut self, reads: Vec<SummaryRead>) {
        *self = Self::default();
        for read in reads {
            self.insert(read, 0);
        }
    }

    /// Rewrite every row, then rebuild the canonical set: a rewrite may
    /// change a key or a rank. Only valid before any propagation.
    pub(super) fn rewrite(&mut self, mut rewrite: impl FnMut(&mut SummaryRead)) {
        let mut reads = self.take();
        reads.iter_mut().for_each(&mut rewrite);
        self.replace(reads);
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
                        .expect("callback arguments are JSON-safe"),
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

#[derive(Clone, Debug, PartialEq)]
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
/// summaries. A nested callable needs more than its access path to
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
    /// What the reused graph and result chunks read when they ran
    /// ([`crate::base::reads`]): with the build's own reads, what the
    /// retained result read.
    pub(super) reused_reads: crate::base::reads::Reads,
    pub(super) reads: Arc<[ReactiveRead]>,
    pub(super) source_call_targets: Arc<HashMap<Location, accessor_provenance::Callable>>,
    pub(super) dispatch_obligations: Arc<[StaticDefect]>,
    pub(super) factory_instances: usize,
    pub(super) timings: InterproceduralTimings,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct InterproceduralTimings {
    pub(super) graph: Duration,
    /// Inside [`Self::graph`]: summary-node discovery (per-file, cached),
    /// `accessor_provenance::Resolver::new`, `transported_call_targets`, the
    /// graph-contribution merge (per-file, cached) and
    /// `refresh_member_invocation_contexts`.
    pub(super) graph_nodes: Duration,
    /// The kept node index's patch ([`node_index`]).
    pub(super) node_index: Duration,
    pub(super) resolver: Duration,
    pub(super) transport: Duration,
    pub(super) merge: Duration,
    pub(super) refresh: Duration,
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
    pub(super) export_work: u64,
    pub(super) fixed_points: crate::InterproceduralFixedPointTimings,
    pub(super) graph_reused_files: u64,
    pub(super) graph_recomputed_files: u64,
    pub(super) result_reused_files: u64,
    pub(super) result_recomputed_files: u64,
}

#[cfg(test)]
mod tests;
