//! ADR 0320's member-invocation refresh (S7c; S7 design §4b): each
//! function's own invocations with their dispatch context, then the
//! invocations its forwarding calls hand back from their callees.
//!
//! Three parts, from the merged graph:
//! 1. the context of every invocation the graph recorded
//!    (`dispatch_read_excluded`, [`Exclusion`]);
//! 2. the unmodeled uses of each helper's parameters
//!    ([`unmodeled_parameter_uses`]);
//! 3. the forwarding loop: a Jacobi loop over the [`ForwardEdge`]s, capped at
//!    eight rounds, each round lengthening forwarding chains by one. A chain
//!    longer than eight is truncated (retained behaviour, replayed, not
//!    fixed); each function's list grows in file, call and argument order.
//!
//! **Kept rows.** Parts 1 and 2 and the edges are a function of one file:
//! its own facts, its nodes and graph-recorded invocations, and what it reads
//! of the rest of the program. They are kept per file between builds, with
//! that file's reads ([`crate::base::reads`], as the graph's chunks keep
//! theirs), and reused while the file is unchanged, its nodes and invocations
//! are the same and every read answers as it did. Two
//! questions are asked again in every build: whether a callback consumer
//! proves a site tracked or fresh ([`Exclusion::Consumer`]: a consumer in any
//! other file can change it, so a kept answer is never proof), and where each
//! edge's callee is among this build's nodes.
//!
//! **Replayed loop.** Part 3 runs on the kept K3 solver
//! ([`crate::base::fixpoint::RoundReplay`], lemma 4′): a function's value at
//! round r is its list after r rounds, a function of its own invocations and
//! its callees' lists at round r − 1, so only the functions downstream of a
//! changed definition are solved again. Each addition lands at the position
//! it had in the loop's order, so the lists, which dependency comparisons
//! read in order (`result_view::dependency_matches`), are the loop's.
//!
//! A build whose files list a path twice, or whose nodes repeat a key, runs
//! the three parts fresh over every file, as do shadow and verify-reused
//! modes, which compare it with the kept result: a difference names the
//! first differing function, poisons the store, and the fresh one is served.

use std::collections::BTreeSet;

use super::*;
use crate::base::FileId;
use crate::base::fixpoint::{Definition, ItemKey, NodeKey, RoundReplay, Step};
use crate::base::memo::derived::SolverId;
use crate::base::reads::ReadProof;

/// The forwarding loop's round cap.
const FORWARDING_ROUNDS: u32 = 8;

/// The refresh's kept state between builds (S7c ids from 384).
const REFRESH_STATE: SolverId = SolverId(384);

/// One file's rows: each of its nodes' own invocations (parts 1 and 2), in
/// the file's node order, and its forwarding edges in call order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct FileRows {
    direct: Vec<Vec<DirectInvocation>>,
    edges: Vec<ForwardEdge>,
}

/// A file's rows as the last build computed them, with what they read.
struct KeptFile {
    /// The file's nodes (span, body), in order.
    owners: Vec<(Span, Span)>,
    /// Each node's graph-recorded invocations the rows started from.
    inputs: Vec<Vec<ParameterMemberInvocation>>,
    compiler: Arc<solid_facts::compiler::ExecutionMap>,
    rows: Arc<FileRows>,
    proof: ReadProof,
}

type Forwarded = RoundReplay<
    NodeKey,
    Vec<ParameterMemberInvocation>,
    Vec<ParameterMemberInvocation>,
    Forwarding,
>;

/// What the refresh keeps between builds.
struct RefreshState {
    files: BTreeMap<String, KeptFile>,
    /// The forwarding loop's definitions as the solver holds them: each
    /// node on an edge, its own invocations and its callees in edge order.
    definitions: BTreeMap<NodeKey, Definition<NodeKey, Vec<ParameterMemberInvocation>, Forwarding>>,
    solver: Forwarded,
}

impl Default for RefreshState {
    fn default() -> Self {
        Self {
            files: BTreeMap::new(),
            definitions: BTreeMap::new(),
            solver: RoundReplay::new(FORWARDING_ROUNDS),
        }
    }
}

/// The forwarding loop's step: a node keeps its list and appends what its
/// edges hand back from their callees' lists of the previous round.
struct ForwardStep;

impl Step<NodeKey, Vec<ParameterMemberInvocation>, Vec<ParameterMemberInvocation>, Forwarding>
    for ForwardStep
{
    fn init(
        &self,
        _: &NodeKey,
        input: &Vec<ParameterMemberInvocation>,
    ) -> Vec<ParameterMemberInvocation> {
        input.clone()
    }

    fn step(
        &self,
        _: &NodeKey,
        _: &Vec<ParameterMemberInvocation>,
        own: &Vec<ParameterMemberInvocation>,
        preds: &[(&Forwarding, Option<&Vec<ParameterMemberInvocation>>)],
    ) -> Vec<ParameterMemberInvocation> {
        let mut added = Vec::new();
        for (edge, target) in preds {
            forward_through(edge, target.map_or(&[][..], Vec::as_slice), own, &mut added);
        }
        let mut next = own.clone();
        next.extend(added);
        next
    }
}

/// Parts 1 and 2 and the edges of one file whose nodes are `owners`.
fn file_rows(
    context: &InterproceduralContext<'_, '_>,
    file: &solid_facts::FileFacts,
    owners: &[usize],
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    members: &[Vec<ParameterMemberInvocation>],
) -> FileRows {
    let InterproceduralContext {
        entities,
        symbol_names,
        lookup,
        ..
    } = context;
    let mut exclusions = SiteExclusions::new(file, entities, symbol_names, lookup);
    let mut direct = owners
        .iter()
        .map(|owner| {
            members[*owner]
                .iter()
                .map(|invocation| DirectInvocation {
                    exclusion: exclusions.at(invocation.site),
                    invocation: invocation.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    unmodeled_parameter_uses(
        file,
        owners,
        nodes,
        nodes_by_path,
        lookup,
        &mut direct,
        &mut |_, site| exclusions.at(site),
    );
    let edges = forward_edges(
        file,
        owners,
        nodes,
        nodes_by_path,
        lookup,
        &mut |_, site| exclusions.at(site),
    );
    FileRows { direct, edges }
}

/// `exclusion` decided at `site` of `file`.
fn decide(
    consumers: &mut crate::execution_role::ReadConsumerSummaries<'_, '_>,
    exclusion: Exclusion,
    file: &solid_facts::FileFacts,
    site: Span,
) -> bool {
    match exclusion {
        Exclusion::Never => false,
        Exclusion::Always => true,
        Exclusion::Consumer => consumers.proves_tracked_or_fresh(file, site),
    }
}

/// Refreshes `invoked_parameter_members`, the merged graph's, in place.
pub(super) fn refresh_member_invocation_contexts(
    context: &InterproceduralContext<'_, '_>,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    invoked_parameter_members: &mut [Vec<ParameterMemberInvocation>],
) {
    let store = context.lookup.memo_store();
    let check = store.check_solvers();
    let merged = check.then(|| invoked_parameter_members.to_vec());
    let mut state = store
        .derived()
        .solver::<RefreshState, _>(REFRESH_STATE, |state, _| std::mem::take(state));
    if crate::audit_off("refresh") {
        state = RefreshState::default();
    }
    if kept_refresh(
        context,
        nodes,
        nodes_by_path,
        invoked_parameter_members,
        &mut state,
    ) {
        if let Some(mut fresh) = merged {
            fresh_refresh(context, nodes, nodes_by_path, &mut fresh);
            if let Some(index) = invoked_parameter_members
                .iter()
                .zip(&fresh)
                .position(|(kept, fresh)| kept != fresh)
            {
                store.poison(&format!(
                    "IR refresh verify mismatch: member invocations of {}:{:?}: the kept refresh differs from a fresh one",
                    nodes[index].path, nodes[index].span
                ));
                invoked_parameter_members.clone_from_slice(&fresh);
                state = RefreshState::default();
            }
        }
        store
            .derived()
            .solver::<RefreshState, _>(REFRESH_STATE, |kept, _| *kept = state);
    } else {
        fresh_refresh(context, nodes, nodes_by_path, invoked_parameter_members);
    }
}

/// What one file's rows are this build: kept, or computed with their reads.
enum Outcome {
    Kept,
    Computed(KeptFile),
}

/// The refresh on kept rows and the kept solver; `false` (nothing changed)
/// when the build lists a path twice or repeats a node key.
fn kept_refresh(
    context: &InterproceduralContext<'_, '_>,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    members: &mut [Vec<ParameterMemberInvocation>],
    state: &mut RefreshState,
) -> bool {
    let InterproceduralContext {
        facts,
        lookup,
        unchanged_paths,
        ..
    } = context;
    let mut ids = HashMap::<&str, FileId>::default();
    for (slot, file) in facts.files.iter().enumerate() {
        let (id, rank) = lookup.memo_slot_of(slot);
        if rank != 0 || ids.insert(file.path.as_str(), id).is_some() {
            return false;
        }
    }
    let mut keys = HashMap::<NodeKey, usize>::default();
    let mut node_keys = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let Some(id) = ids.get(node.path.as_str()) else {
            return false;
        };
        let key = NodeKey {
            file: *id,
            item: ItemKey::Span(node.span),
        };
        if keys.insert(key, index).is_some() {
            return false;
        }
        node_keys.push(key);
    }
    let jobs = facts
        .files
        .iter()
        .map(|file| {
            (
                file,
                nodes_by_path
                    .get(file.path.as_str())
                    .map_or(&[][..], Vec::as_slice),
            )
        })
        .collect::<Vec<_>>();
    let kept_files = &state.files;
    let members_in: &[Vec<ParameterMemberInvocation>] = members;
    let outcomes = crate::pipeline::parallel_balanced_results(&jobs, |(file, owners)| {
        if let Some(kept) = kept_files.get(file.path.as_str())
            && reusable(
                context,
                kept,
                file,
                owners,
                nodes,
                members_in,
                unchanged_paths,
            )
        {
            crate::base::reads::replay(&kept.proof.reads);
            return Outcome::Kept;
        }
        let (rows, reads) = crate::base::reads::record(|| {
            file_rows(context, file, owners, nodes, nodes_by_path, members_in)
        });
        Outcome::Computed(KeptFile {
            owners: owners
                .iter()
                .map(|owner| (nodes[*owner].span, nodes[*owner].body))
                .collect(),
            inputs: owners
                .iter()
                .map(|owner| members_in[*owner].clone())
                .collect(),
            compiler: file.compiler.clone(),
            rows: Arc::new(rows),
            proof: ReadProof::new(reads, *lookup),
        })
    });
    let current = jobs
        .iter()
        .map(|(file, _)| file.path.as_str())
        .collect::<HashSet<_>>();
    state
        .files
        .retain(|path, _| current.contains(path.as_str()));
    for ((file, _), outcome) in jobs.iter().zip(outcomes) {
        if let Outcome::Computed(kept) = outcome {
            state.files.insert(file.path.to_string(), kept);
        }
    }
    // The exclusions a consumer decides, and each edge's callee among this
    // build's nodes.
    let mut consumers = crate::execution_role::ReadConsumerSummaries::new(lookup);
    let mut definitions =
        BTreeMap::<NodeKey, Definition<NodeKey, Vec<ParameterMemberInvocation>, Forwarding>>::new();
    let mut on_edges = BTreeSet::<NodeKey>::new();
    for (file, owners) in &jobs {
        let rows = &state.files[file.path.as_str()].rows;
        for (owner, direct) in owners.iter().zip(&rows.direct) {
            members[*owner] = direct
                .iter()
                .map(|direct| {
                    let mut invocation = direct.invocation.clone();
                    invocation.dispatch_read_excluded =
                        decide(&mut consumers, direct.exclusion, file, invocation.site);
                    invocation
                })
                .collect();
        }
        for edge in &rows.edges {
            let Some(target) = ids
                .get(edge.target.0.as_str())
                .map(|id| NodeKey {
                    file: *id,
                    item: ItemKey::Span(edge.target.1),
                })
                .filter(|target| keys.contains_key(target))
            else {
                continue;
            };
            let owner = node_keys[owners[edge.owner]];
            let forwarding = Forwarding {
                parameter: edge.parameter,
                index: edge.index,
                in_owner_body: edge.in_owner_body,
                site: edge.site,
                excluded: decide(&mut consumers, edge.exclusion, file, edge.site),
            };
            definitions
                .entry(owner)
                .or_insert_with(|| Definition {
                    input: Vec::new(),
                    preds: Vec::new(),
                })
                .preds
                .push((target, forwarding));
            on_edges.insert(owner);
            on_edges.insert(target);
        }
    }
    for key in &on_edges {
        definitions
            .entry(*key)
            .or_insert_with(|| Definition {
                input: Vec::new(),
                preds: Vec::new(),
            })
            .input = members[keys[key]].clone();
    }
    let mut changes = definitions
        .iter()
        .filter(|(key, definition)| state.definitions.get(*key) != Some(*definition))
        .map(|(key, definition)| (*key, Some(definition.clone())))
        .collect::<Vec<_>>();
    changes.extend(
        state
            .definitions
            .keys()
            .filter(|key| !definitions.contains_key(*key))
            .map(|key| (*key, None)),
    );
    state.solver.update(changes, &ForwardStep);
    for key in definitions.keys() {
        if let Some(value) = state.solver.value(key) {
            members[keys[key]].clone_from(value);
        }
    }
    state.definitions = definitions;
    true
}

/// Whether `kept` holds `file`'s rows in this build.
fn reusable(
    context: &InterproceduralContext<'_, '_>,
    kept: &KeptFile,
    file: &solid_facts::FileFacts,
    owners: &[usize],
    nodes: &[SummaryNode],
    members: &[Vec<ParameterMemberInvocation>],
    unchanged_paths: &HashSet<String>,
) -> bool {
    let lookup = context.lookup;
    unchanged_paths.contains(file.path.as_str())
        && kept.owners.len() == owners.len()
        && kept
            .owners
            .iter()
            .zip(owners)
            .all(|((span, body), owner)| nodes[*owner].span == *span && nodes[*owner].body == *body)
        && kept
            .inputs
            .iter()
            .zip(owners)
            .all(|(input, owner)| *input == members[*owner])
        && (Arc::ptr_eq(&kept.compiler, &file.compiler)
            || same_compiler_semantics(&kept.compiler, &file.compiler))
        && kept.proof.holds(lookup)
}

/// The three parts fresh over every slot: part 1 for every file, then part 2
/// for every file, then the edges, as the passes always ran (a path listed
/// twice shares its nodes between its slots), and the loop round by round.
fn fresh_refresh(
    context: &InterproceduralContext<'_, '_>,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    members: &mut [Vec<ParameterMemberInvocation>],
) {
    let InterproceduralContext {
        facts,
        entities,
        symbol_names,
        lookup,
        ..
    } = context;
    let mut direct = members
        .iter()
        .map(|list| {
            list.iter()
                .map(|invocation| DirectInvocation {
                    invocation: invocation.clone(),
                    exclusion: Exclusion::Never,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    // The slot whose facts decided each invocation's exclusion.
    let mut slots = members
        .iter()
        .map(|list| vec![0; list.len()])
        .collect::<Vec<_>>();
    let mut exclusions = facts
        .files
        .iter()
        .map(|file| SiteExclusions::new(file, entities, symbol_names, lookup))
        .collect::<Vec<_>>();
    let owners_of = |file: &solid_facts::FileFacts| {
        nodes_by_path
            .get(file.path.as_str())
            .map_or(&[][..], Vec::as_slice)
    };
    for (slot, file) in facts.files.iter().enumerate() {
        for owner in owners_of(file) {
            for (invocation, decided_by) in direct[*owner].iter_mut().zip(&mut slots[*owner]) {
                invocation.exclusion = exclusions[slot].at(invocation.invocation.site);
                *decided_by = slot;
            }
        }
    }
    for (slot, file) in facts.files.iter().enumerate() {
        let owners = owners_of(file);
        let mut aligned = owners
            .iter()
            .map(|owner| std::mem::take(&mut direct[*owner]))
            .collect::<Vec<_>>();
        unmodeled_parameter_uses(
            file,
            owners,
            nodes,
            nodes_by_path,
            lookup,
            &mut aligned,
            &mut |_, site| exclusions[slot].at(site),
        );
        for (owner, rows) in owners.iter().zip(aligned) {
            let added = rows.len() - slots[*owner].len();
            slots[*owner].extend(std::iter::repeat_n(slot, added));
            direct[*owner] = rows;
        }
    }
    let mut edges = Vec::new();
    for (slot, file) in facts.files.iter().enumerate() {
        let owners = owners_of(file);
        edges.extend(
            forward_edges(
                file,
                owners,
                nodes,
                nodes_by_path,
                lookup,
                &mut |_, site| exclusions[slot].at(site),
            )
            .into_iter()
            .map(|edge| (slot, owners[edge.owner], edge)),
        );
    }
    let mut consumers = crate::execution_role::ReadConsumerSummaries::new(lookup);
    for ((members, direct), slots) in members.iter_mut().zip(direct).zip(slots) {
        *members = direct
            .into_iter()
            .zip(slots)
            .map(|(direct, slot)| {
                let mut invocation = direct.invocation;
                invocation.dispatch_read_excluded = decide(
                    &mut consumers,
                    direct.exclusion,
                    &facts.files[slot],
                    invocation.site,
                );
                invocation
            })
            .collect();
    }
    let edges = edges
        .into_iter()
        .filter_map(|(slot, owner, edge)| {
            let target = nodes_by_path
                .get(edge.target.0.as_str())
                .into_iter()
                .flatten()
                .copied()
                .find(|candidate| nodes[*candidate].span == edge.target.1)?;
            let forwarding = Forwarding {
                parameter: edge.parameter,
                index: edge.index,
                in_owner_body: edge.in_owner_body,
                site: edge.site,
                excluded: decide(
                    &mut consumers,
                    edge.exclusion,
                    &facts.files[slot],
                    edge.site,
                ),
            };
            Some((owner, target, forwarding))
        })
        .collect::<Vec<_>>();
    for _ in 0..FORWARDING_ROUNDS {
        let mut added = BTreeMap::<usize, Vec<ParameterMemberInvocation>>::new();
        for (owner, target, forwarding) in &edges {
            let owner_added = added.entry(*owner).or_default();
            forward_through(forwarding, &members[*target], &members[*owner], owner_added);
        }
        added.retain(|_, added| !added.is_empty());
        if added.is_empty() {
            return;
        }
        for (owner, added) in added {
            members[owner].extend(added);
        }
    }
}
