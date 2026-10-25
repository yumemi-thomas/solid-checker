//! The owner graph on kept state (S7f, ADR 0393).
//!
//! The graph keeps what the owner analysis derives from the files' fragments
//! ([`CachedOwnerFile`]) between builds, keyed by stable node ids (an interned
//! path and the function's span), and re-solves only what a build changed:
//!
//! - **Patching.** A recomputed fragment whose content equals the kept one
//!   changes nothing downstream. Otherwise its nodes, symbol winners and
//!   resolved edges are patched: its own edges are resolved again, and every
//!   file's edges only when the fragment's *shape* (its nodes' spans, symbols
//!   and generator flags, all that edge resolution reads of another file)
//!   moved. Targets whose incoming edges changed are the seeds.
//! - **Cones.** The context bits flow on two kept K2 solvers
//!   ([`Monotone`]): phase 1 over the direct contexts, phase 3 over the same
//!   graph with the eager calls the zero contexts settle. Each re-solves the
//!   forward cone of the seeds; phase 2 (the settle, which tests
//!   `contexts == 0`) is recomputed pointwise on the nodes it can have moved.
//!   The solvers' value changes are the nodes whose context moved, which is
//!   what the per-file emission ([`super::owner_emission`]) is dirtied by.
//! - **Fresh solve.** The dense worklist the analysis always ran
//!   ([`propagate_owner_contexts`]) is still the answer wherever the cone
//!   would not be exact or is not wanted: a one-shot build (nothing is
//!   kept), an unnamed namespace escape (the `through_unnamed` reachability
//!   passes of ADR 0347's amendment), and any `createTrackedEffect` edge
//!   ([`OwnerEdgeKind::Leaf`]). The transfer functions are monotone, so the
//!   least fixed point is the worklist's answer in every order, except for two
//!   kinds: [`OwnerEdgeKind::MayResumeUnowned`] adds the unowned and resume
//!   bits only while its source is not proven unowned, so a source that
//!   *becomes* proven after the edge fired leaves the resume bit behind. That
//!   bit is unobservable (a proven source already carries every bit it is
//!   read for: [`OWNER_CONTEXT_LEAVES_REGION`] holds the proof bit too), and
//!   verify mode compares with it masked. The solver's transfer adds the
//!   unowned and resume bits for a proven source too (`transfer`): the
//!   exact function is not monotone, and a cone over a cycle through it
//!   oscillated in the property test. The tracked-effect transfer
//!   ([`tracked_leaf_body_context`]) is not monotone in an observable bit, so
//!   a graph holding such an edge solves fresh in the worklist's order.
//!
//! Paths are interned once ([`PathId`]); an id is never reused.

use std::collections::BTreeSet;

use super::*;
use crate::base::fixpoint::{Definition, Lattice, Monotone};

/// A definition change for a solver: `None` undefines the node.
type Changes = Vec<(NodeId, Option<Definition<NodeId, Bits, OwnerEdgeKind>>)>;

/// Dense nodes, their contexts and their resolved outgoing edges.
pub(super) type DenseView = (Vec<OwnerNode>, Vec<u8>, Vec<Vec<(usize, OwnerEdgeKind)>>);

/// An analyzed path, interned for the graph's life.
pub(super) type PathId = u32;

/// A function of the owner graph: its file and span.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct NodeId {
    pub(super) path: PathId,
    pub(super) span: Span,
}

/// Owner context bits, joined by union.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Bits(pub(super) u8);

impl Lattice for Bits {
    fn bottom() -> Self {
        Self(0)
    }

    fn join(&mut self, other: &Self) {
        self.0 |= other.0;
    }
}

/// What an edge of kind `kind` carries out of a source holding `source`.
/// A source with no bit has not been reached, so it carries nothing: the
/// worklist never visits it (and `Owned` and the other constant kinds would
/// otherwise reach their targets from nowhere).
fn transfer(kind: &OwnerEdgeKind, source: &Bits) -> Bits {
    if source.0 == 0 {
        return Bits(0);
    }
    // `MayResumeUnowned` drops its unowned and resume bits for a proven
    // source, which is not monotone: a cone solve over a cycle through it can
    // oscillate. The solver adds them for a proven source too; that source
    // already holds the unowned bit, and the resume bit beside the proof is
    // unobservable (module docs).
    if *kind == OwnerEdgeKind::MayResumeUnowned {
        return Bits(source.0 | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_MAY_RESUME_UNOWNED);
    }
    Bits(owner_edge_context(*kind, source.0))
}

/// An edge as resolved against the current nodes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Resolved {
    /// `None`: the edge has no source function (module scope, or a source
    /// that names no node), so its context is a direct one.
    source: Option<NodeId>,
    target: NodeId,
    kind: OwnerEdgeKind,
}

#[derive(Clone, Debug)]
enum InternedTarget {
    Symbol(SymbolId),
    Local(Span),
    Function(PathId, Span),
}

#[derive(Clone, Debug)]
struct InternedEdge {
    source: Option<(PathId, Span)>,
    target: InternedTarget,
    kind: OwnerEdgeKind,
}

/// One file's fragment and what the graph derived from it.
pub(super) struct GraphFile {
    pub(super) path: Arc<str>,
    pub(super) fragment: CachedOwnerFile,
    /// Each node's local index, by span.
    spans: HashMap<Span, u32>,
    /// All that edge resolution reads of another file's nodes.
    shape: Vec<(Span, Option<SymbolId>, bool)>,
    edges: Vec<InternedEdge>,
    resolved: Vec<Option<Resolved>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Incoming {
    source: Option<NodeId>,
    kind: OwnerEdgeKind,
    /// The edge: its file and index in the fragment.
    from: (PathId, u32),
}

#[derive(Clone, Debug)]
struct SymbolEntry {
    /// The file's path, the sort key: facts list files by path.
    path: Arc<str>,
    index: u32,
    node: NodeId,
}

/// The hand-out answers of one build: the lookup's namespace census.
pub(super) struct HandOuts<'a> {
    /// Whether the program hands out the function at `(path, span)` by name.
    pub(super) named: &'a dyn Fn(&str, Span) -> bool,
    /// Whether the module's namespace escapes with no name readable.
    pub(super) escapes_unnamed: &'a dyn Fn(&str) -> bool,
}

/// What a build's solve moved.
#[derive(Default)]
pub(super) struct Moved {
    /// Nodes whose final context was added, removed or changed.
    pub(super) contexts: Vec<NodeId>,
    /// Fragments whose content changed (their files re-emit).
    pub(super) files: BTreeSet<PathId>,
}

#[derive(Default)]
pub(crate) struct OwnerGraph {
    paths: HashMap<Arc<str>, PathId>,
    names: Vec<Arc<str>>,
    files: HashMap<PathId, GraphFile>,
    /// Every node with its symbol, in (path, index) order: the last wins.
    symbols: HashMap<SymbolId, Vec<SymbolEntry>>,
    incoming: HashMap<NodeId, Vec<Incoming>>,
    outgoing: HashMap<NodeId, Vec<(NodeId, OwnerEdgeKind)>>,
    namespace_nodes: BTreeSet<NodeId>,
    /// Resolved edges of kind [`OwnerEdgeKind::Leaf`].
    leaf_edges: usize,
    /// The namespace hand-outs of the last build.
    named: HashSet<NodeId>,
    candidates: HashSet<NodeId>,
    /// The final context of every node.
    contexts: HashMap<NodeId, u8>,
    phase1: Monotone<NodeId, Bits, OwnerEdgeKind>,
    phase3: Monotone<NodeId, Bits, OwnerEdgeKind>,
    solvers_valid: bool,
    // Accumulators of the build under way.
    touched: BTreeSet<NodeId>,
    removed: BTreeSet<NodeId>,
    changed_files: BTreeSet<PathId>,
    structural: bool,
}

impl OwnerGraph {
    pub(super) fn intern(&mut self, path: &str) -> PathId {
        if let Some(id) = self.paths.get(path) {
            return *id;
        }
        let id = PathId::try_from(self.names.len()).expect("fewer than 2^32 paths");
        let name: Arc<str> = Arc::from(path);
        self.paths.insert(Arc::clone(&name), id);
        self.names.push(name);
        id
    }

    pub(super) fn path_id(&self, path: &str) -> Option<PathId> {
        self.paths.get(path).copied()
    }

    pub(super) fn path_name(&self, id: PathId) -> &str {
        &self.names[id as usize]
    }

    pub(super) fn file(&self, id: PathId) -> Option<&GraphFile> {
        self.files.get(&id)
    }

    pub(super) fn file_mut(&mut self, id: PathId) -> Option<&mut GraphFile> {
        self.files.get_mut(&id)
    }

    /// The ids of the paths the graph holds a fragment of.
    pub(super) fn held(&self) -> Vec<PathId> {
        self.files.keys().copied().collect()
    }

    pub(super) fn node(&self, id: NodeId) -> Option<&OwnerNode> {
        let file = self.files.get(&id.path)?;
        let index = *file.spans.get(&id.span)?;
        file.fragment.nodes.get(index as usize)
    }

    /// The node at `span` of file `path` with its final context.
    pub(super) fn node_context(&self, path: PathId, span: Span) -> Option<(&OwnerNode, u8)> {
        let id = NodeId { path, span };
        let node = self.node(id)?;
        Some((node, self.contexts.get(&id).copied().unwrap_or(0)))
    }

    pub(super) fn context(&self, path: PathId, span: Span) -> Option<u8> {
        self.contexts.get(&NodeId { path, span }).copied()
    }

    /// The node a symbolic target names from file `from`, as edge resolution
    /// reads it.
    pub(super) fn resolve_target(&self, from: PathId, target: &OwnerTarget) -> Option<NodeId> {
        match target {
            OwnerTarget::Symbol(symbol) => self.winner(symbol),
            OwnerTarget::LocalSpan(span) => self.exists(from, *span),
            OwnerTarget::Function(path, span) => self.exists(self.path_id(path)?, *span),
        }
    }

    fn exists(&self, path: PathId, span: Span) -> Option<NodeId> {
        self.files
            .get(&path)?
            .spans
            .contains_key(&span)
            .then_some(NodeId { path, span })
    }

    fn winner(&self, symbol: &SymbolId) -> Option<NodeId> {
        self.symbols.get(symbol)?.last().map(|entry| entry.node)
    }

    /// Stores `fragment` as `path`'s. Whether its content differs from the
    /// kept one's: an equal recomputed fragment keeps every derived fact and
    /// only brings its proof and hashes up to date.
    pub(super) fn set_fragment(&mut self, path: &str, fragment: CachedOwnerFile) -> bool {
        let id = self.intern(path);
        if let Some(file) = self.files.get_mut(&id)
            && file.fragment.nodes == fragment.nodes
            && file.fragment.edges == fragment.edges
            && file.fragment.requirements == fragment.requirements
        {
            file.fragment = fragment;
            return false;
        }
        self.detach(id);
        let name = Arc::clone(&self.names[id as usize]);
        let shape = fragment
            .nodes
            .iter()
            .map(|node| (node.span, node.symbol.clone(), node.generator))
            .collect::<Vec<_>>();
        let was = self.files.get(&id).map(|file| file.shape.clone());
        if was.as_ref() != Some(&shape) {
            self.structural = true;
        }
        let spans = fragment
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                (
                    node.span,
                    u32::try_from(index).expect("fewer than 2^32 nodes"),
                )
            })
            .collect::<HashMap<_, _>>();
        let edges = fragment
            .edges
            .iter()
            .map(|edge| self.intern_edge(edge))
            .collect::<Vec<_>>();
        for (index, node) in fragment.nodes.iter().enumerate() {
            let node_id = NodeId {
                path: id,
                span: node.span,
            };
            self.removed.remove(&node_id);
            self.touched.insert(node_id);
            if let Some(symbol) = &node.symbol {
                let entries = self.symbols.entry(symbol.clone()).or_default();
                let entry = SymbolEntry {
                    path: Arc::clone(&name),
                    index: u32::try_from(index).expect("fewer than 2^32 nodes"),
                    node: node_id,
                };
                let at = entries.partition_point(|existing| {
                    (&existing.path, existing.index) < (&entry.path, entry.index)
                });
                entries.insert(at, entry);
            }
            if node.namespace_candidate {
                self.namespace_nodes.insert(node_id);
            }
        }
        let resolved = vec![None; edges.len()];
        self.files.insert(
            id,
            GraphFile {
                path: name,
                fragment,
                spans,
                shape,
                edges,
                resolved,
            },
        );
        self.changed_files.insert(id);
        true
    }

    /// Drops `path`'s fragment and everything derived from it.
    pub(super) fn remove_fragment(&mut self, path: PathId) {
        if self.files.contains_key(&path) {
            self.detach(path);
            self.files.remove(&path);
            self.structural = true;
            self.changed_files.remove(&path);
        }
    }

    fn intern_edge(&mut self, edge: &SymbolicOwnerEdge) -> InternedEdge {
        InternedEdge {
            source: edge
                .source
                .as_ref()
                .map(|source| (self.intern(&source.path), source.span)),
            target: match &edge.target {
                OwnerTarget::Symbol(symbol) => InternedTarget::Symbol(symbol.clone()),
                OwnerTarget::LocalSpan(span) => InternedTarget::Local(*span),
                OwnerTarget::Function(path, span) => {
                    InternedTarget::Function(self.intern(path), *span)
                }
            },
            kind: edge.kind,
        }
    }

    /// Removes what `path`'s kept fragment contributed to the shared indexes
    /// (its file entry stays until replaced).
    fn detach(&mut self, path: PathId) {
        let Some(file) = self.files.get_mut(&path) else {
            return;
        };
        let resolved = std::mem::take(&mut file.resolved);
        let nodes = file
            .fragment
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                (
                    index,
                    NodeId {
                        path,
                        span: node.span,
                    },
                    node.symbol.clone(),
                )
            })
            .collect::<Vec<_>>();
        let name = Arc::clone(&file.path);
        for (index, resolved) in resolved.into_iter().enumerate() {
            if let Some(resolved) = resolved {
                self.unindex(path, index, &resolved);
            }
        }
        for (index, node, symbol) in nodes {
            if let Some(symbol) = symbol
                && let Some(entries) = self.symbols.get_mut(&symbol)
            {
                entries.retain(|entry| !(entry.path == name && entry.index as usize == index));
                if entries.is_empty() {
                    self.symbols.remove(&symbol);
                }
            }
            self.namespace_nodes.remove(&node);
            self.removed.insert(node);
            self.touched.remove(&node);
        }
    }

    fn index(&mut self, path: PathId, edge: usize, resolved: &Resolved) {
        let from = (path, u32::try_from(edge).expect("fewer than 2^32 edges"));
        self.incoming
            .entry(resolved.target)
            .or_default()
            .push(Incoming {
                source: resolved.source,
                kind: resolved.kind,
                from,
            });
        if let Some(source) = resolved.source {
            self.outgoing
                .entry(source)
                .or_default()
                .push((resolved.target, resolved.kind));
        }
        if resolved.kind == OwnerEdgeKind::Leaf {
            self.leaf_edges += 1;
        }
        self.touched.insert(resolved.target);
    }

    fn unindex(&mut self, path: PathId, edge: usize, resolved: &Resolved) {
        let from = (path, u32::try_from(edge).expect("fewer than 2^32 edges"));
        if let Some(list) = self.incoming.get_mut(&resolved.target) {
            list.retain(|incoming| incoming.from != from);
            if list.is_empty() {
                self.incoming.remove(&resolved.target);
            }
        }
        if let Some(source) = resolved.source
            && let Some(list) = self.outgoing.get_mut(&source)
        {
            if let Some(at) = list
                .iter()
                .position(|(target, kind)| *target == resolved.target && *kind == resolved.kind)
            {
                list.swap_remove(at);
            }
            if list.is_empty() {
                self.outgoing.remove(&source);
            }
        }
        if resolved.kind == OwnerEdgeKind::Leaf {
            self.leaf_edges -= 1;
        }
        self.touched.insert(resolved.target);
    }

    fn resolve_edge(&self, path: PathId, edge: &InternedEdge) -> Option<Resolved> {
        let target = match &edge.target {
            InternedTarget::Symbol(symbol) => self.winner(symbol)?,
            InternedTarget::Local(span) => self.exists(path, *span)?,
            InternedTarget::Function(target, span) => self.exists(*target, *span)?,
        };
        // A generator's body never runs on a ref application: nothing is proven.
        let kind = match edge.kind {
            OwnerEdgeKind::RefApplied if self.node(target).is_some_and(|node| node.generator) => {
                OwnerEdgeKind::MayBeUnowned
            }
            kind => kind,
        };
        let source = edge
            .source
            .and_then(|(source, span)| self.exists(source, span));
        Some(Resolved {
            source,
            target,
            kind,
        })
    }

    fn resolve_file(&mut self, path: PathId) {
        let Some(file) = self.files.get(&path) else {
            return;
        };
        let next = file
            .edges
            .iter()
            .map(|edge| self.resolve_edge(path, edge))
            .collect::<Vec<_>>();
        let previous = std::mem::take(
            &mut self
                .files
                .get_mut(&path)
                .expect("the file is held")
                .resolved,
        );
        let mut kept = Vec::with_capacity(next.len());
        for (index, next) in next.into_iter().enumerate() {
            let previous = previous.get(index).copied().flatten();
            if previous != next {
                if let Some(previous) = previous {
                    self.unindex(path, index, &previous);
                }
                if let Some(next) = &next {
                    self.index(path, index, next);
                }
            }
            kept.push(next);
        }
        self.files
            .get_mut(&path)
            .expect("the file is held")
            .resolved = kept;
    }

    /// Re-resolves what the build's patches can have moved.
    fn resolve(&mut self) {
        let files = if self.structural {
            self.files.keys().copied().collect::<Vec<_>>()
        } else {
            self.changed_files.iter().copied().collect::<Vec<_>>()
        };
        for path in files {
            self.resolve_file(path);
        }
    }

    // ---- definitions -------------------------------------------------

    /// The direct bits of a node: its seed, a named hand-out, and the edges
    /// with no source function.
    fn direct(&self, id: NodeId) -> u8 {
        let Some(node) = self.node(id) else { return 0 };
        let mut bits = node.seed_context;
        if self.named.contains(&id) {
            bits |= OWNER_CONTEXT_UNOWNED;
        }
        for incoming in self.incoming.get(&id).into_iter().flatten() {
            if incoming.source.is_none() {
                bits |= owner_edge_context(
                    incoming.kind,
                    OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
                );
            }
        }
        if self.candidates.contains(&id) {
            bits |= OWNER_CONTEXT_UNOWNED;
        }
        bits
    }

    fn predecessors(&self, id: NodeId) -> Vec<(NodeId, OwnerEdgeKind)> {
        let mut preds = self
            .incoming
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|incoming| incoming.source.map(|source| (source, incoming.kind)))
            .collect::<Vec<_>>();
        preds.sort_unstable_by_key(|(source, kind)| (*source, *kind as u8));
        preds.dedup();
        preds
    }

    fn definition(&self, id: NodeId, extra: u8) -> Option<Definition<NodeId, Bits, OwnerEdgeKind>> {
        self.node(id)?;
        Some(Definition {
            input: Bits(self.direct(id) | extra),
            preds: self.predecessors(id),
        })
    }

    /// The bit the eager calls of an unreached source give `id` (the settle).
    fn eager(&self, id: NodeId) -> u8 {
        let reached = |source: NodeId| self.phase1.value(&source).is_some_and(|value| value.0 != 0);
        let eager = self
            .incoming
            .get(&id)
            .into_iter()
            .flatten()
            .filter(|incoming| incoming.kind == OwnerEdgeKind::EagerCall)
            .filter_map(|incoming| incoming.source)
            .any(|source| !reached(source));
        if eager {
            owner_edge_context(OwnerEdgeKind::MayBeUnowned, 0)
        } else {
            0
        }
    }

    // ---- the build ---------------------------------------------------

    /// Brings the graph and its contexts up to the fragments the build
    /// stored, then reports what moved. `order` lists the paths of the
    /// build's files (path order, each once); `retain` says the state will be
    /// used again, which makes the kept solvers worth keeping.
    pub(super) fn settle(
        &mut self,
        order: &[PathId],
        hand_outs: &HandOuts<'_>,
        retain: bool,
        verify: bool,
    ) -> Moved {
        self.resolve();
        // Hand-outs: the census answers for the namespace candidates only.
        let mut named = HashSet::default();
        let mut candidates = HashSet::default();
        for id in &self.namespace_nodes {
            let path = self.path_name(id.path);
            if (hand_outs.named)(path, id.span) {
                named.insert(*id);
            } else if (hand_outs.escapes_unnamed)(path) {
                candidates.insert(*id);
            }
        }
        for id in self
            .named
            .symmetric_difference(&named)
            .chain(self.candidates.symmetric_difference(&candidates))
            .copied()
            .collect::<Vec<_>>()
        {
            self.touched.insert(id);
        }
        self.named = named;
        self.candidates = candidates;

        let moved_files = std::mem::take(&mut self.changed_files);
        let touched = std::mem::take(&mut self.touched);
        let removed = std::mem::take(&mut self.removed);
        self.structural = false;
        let cones = retain
            && self.candidates.is_empty()
            && self.leaf_edges == 0
            && !crate::audit_off("owner_cones");
        let mut moved = if cones {
            self.solve_cones(&touched, &removed)
        } else {
            self.solvers_valid = false;
            self.solve_fresh(order, &removed)
        };
        if verify && cones {
            self.verify(order, &mut moved);
        }
        moved.files = moved_files;
        moved
    }

    fn definitions(&self, ids: impl IntoIterator<Item = NodeId>, phase3: bool) -> Changes {
        ids.into_iter()
            .map(|id| {
                let extra = if phase3 { self.eager(id) } else { 0 };
                (id, self.definition(id, extra))
            })
            .collect()
    }

    fn solve_cones(&mut self, touched: &BTreeSet<NodeId>, removed: &BTreeSet<NodeId>) -> Moved {
        let fresh = !self.solvers_valid;
        let (changes1, all) = if fresh {
            self.phase1 = Monotone::default();
            self.phase3 = Monotone::default();
            let all = self
                .files
                .iter()
                .flat_map(|(path, file)| {
                    file.fragment.nodes.iter().map(|node| NodeId {
                        path: *path,
                        span: node.span,
                    })
                })
                .collect::<BTreeSet<_>>();
            (self.definitions(all.iter().copied(), false), Some(all))
        } else {
            let mut changes = self.definitions(touched.iter().copied(), false);
            changes.extend(removed.iter().map(|id| (*id, None)));
            (changes, None)
        };
        let solved1 = self.phase1.update(changes1, &transfer);
        // Phase 2, pointwise: the targets whose eager sources may have moved
        // in or out of zero.
        let mut affected = all.unwrap_or_else(|| touched.clone());
        for key in solved1.changed.iter().chain(touched.iter()) {
            for (target, kind) in self.outgoing.get(key).into_iter().flatten() {
                if *kind == OwnerEdgeKind::EagerCall {
                    affected.insert(*target);
                }
            }
        }
        let mut changes3 = self.definitions(affected.iter().copied(), true);
        changes3.extend(removed.iter().map(|id| (*id, None)));
        let solved3 = self.phase3.update(changes3, &transfer);
        self.solvers_valid = true;
        let mut contexts = Vec::new();
        if fresh {
            self.contexts.clear();
        }
        for key in solved3.changed {
            match self.phase3.value(&key) {
                Some(bits) => {
                    if self.contexts.insert(key, bits.0) != Some(bits.0) {
                        contexts.push(key);
                    }
                }
                None => {
                    if self.contexts.remove(&key).is_some() {
                        contexts.push(key);
                    }
                }
            }
        }
        if fresh {
            contexts = self.contexts.keys().copied().collect();
        }
        Moved {
            contexts,
            files: BTreeSet::new(),
        }
    }

    /// The dense worklist, in the order of the build's files.
    fn dense(&self, order: &[PathId]) -> (Vec<NodeId>, Vec<u8>) {
        let mut nodes = Vec::new();
        let mut index = HashMap::<NodeId, usize>::default();
        for path in order {
            let Some(file) = self.files.get(path) else {
                continue;
            };
            for node in &file.fragment.nodes {
                let id = NodeId {
                    path: *path,
                    span: node.span,
                };
                index.insert(id, nodes.len());
                nodes.push(id);
            }
        }
        let mut contexts = nodes
            .iter()
            .map(|id| self.node(*id).map_or(0, |node| node.seed_context))
            .collect::<Vec<_>>();
        let mut outgoing = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
        for (position, id) in nodes.iter().enumerate() {
            if self.named.contains(id) {
                contexts[position] |= OWNER_CONTEXT_UNOWNED;
            }
        }
        for path in order {
            let Some(file) = self.files.get(path) else {
                continue;
            };
            for resolved in file.resolved.iter().flatten() {
                let (Some(target), source) = (
                    index.get(&resolved.target).copied(),
                    resolved
                        .source
                        .and_then(|source| index.get(&source).copied()),
                ) else {
                    continue;
                };
                match source {
                    Some(source) => outgoing[source].push((target, resolved.kind)),
                    None => {
                        contexts[target] |= owner_edge_context(
                            resolved.kind,
                            OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
                        );
                    }
                }
            }
        }
        let candidates = nodes
            .iter()
            .map(|id| self.candidates.contains(id))
            .collect::<Vec<_>>();
        let uncertain = seed_unnamed_hand_outs(&mut contexts, &outgoing, &candidates);
        propagate_owner_contexts(&mut contexts, &outgoing, &uncertain);
        settle_eager_owners(&mut contexts, &outgoing, &uncertain);
        (nodes, contexts)
    }

    fn solve_fresh(&mut self, order: &[PathId], removed: &BTreeSet<NodeId>) -> Moved {
        let (nodes, contexts) = self.dense(order);
        let mut next = HashMap::<NodeId, u8>::default();
        next.reserve(nodes.len());
        let mut moved = Vec::new();
        for (id, context) in nodes.iter().zip(&contexts) {
            if self.contexts.get(id) != Some(context) {
                moved.push(*id);
            }
            next.insert(*id, *context);
        }
        moved.extend(
            removed
                .iter()
                .filter(|id| self.contexts.contains_key(id) && !next.contains_key(id))
                .copied(),
        );
        // A node a removed file held and the dense order no longer lists.
        moved.extend(
            self.contexts
                .keys()
                .filter(|id| !next.contains_key(id))
                .copied(),
        );
        moved.sort_unstable();
        moved.dedup();
        self.contexts = next;
        Moved {
            contexts: moved,
            files: BTreeSet::new(),
        }
    }

    /// Verify mode: the kept solve against the worklist, by node. The
    /// resume bit a proven source leaves behind is masked (see the module
    /// docs); a difference names the first node, and the worklist's answer
    /// replaces the kept one.
    fn verify(&mut self, order: &[PathId], moved: &mut Moved) {
        let (nodes, contexts) = self.dense(order);
        let mask = |context: u8| {
            if context & OWNER_CONTEXT_PROVEN_UNOWNED != 0 {
                context & !OWNER_CONTEXT_MAY_RESUME_UNOWNED
            } else {
                context
            }
        };
        let expected = nodes
            .iter()
            .zip(&contexts)
            .map(|(id, context)| (*id, *context))
            .collect::<HashMap<_, _>>();
        let mut differing = expected
            .iter()
            .filter(|(id, context)| {
                self.contexts
                    .get(id)
                    .is_none_or(|kept| mask(*kept) != mask(**context))
            })
            .map(|(id, context)| (*id, Some(*context)))
            .chain(
                self.contexts
                    .keys()
                    .filter(|id| !expected.contains_key(id))
                    .map(|id| (*id, None)),
            )
            .collect::<Vec<_>>();
        differing.sort_unstable();
        let Some((first, expected_context)) = differing.first().copied() else {
            return;
        };
        let message = format!(
            "IR owner verify mismatch: node {}:{:?} holds {:?}, the worklist gives {:?} ({} nodes differ)",
            self.path_name(first.path),
            first.span,
            self.contexts.get(&first),
            expected_context,
            differing.len()
        );
        eprintln!("{message}");
        if crate::shadow::ShadowMode::from_env() == crate::shadow::ShadowMode::Panic {
            panic!("{message}");
        }
        self.solvers_valid = false;
        for (id, context) in differing {
            match context {
                Some(context) => {
                    self.contexts.insert(id, context);
                }
                None => {
                    self.contexts.remove(&id);
                }
            }
            moved.contexts.push(id);
        }
        moved.contexts.sort_unstable();
        moved.contexts.dedup();
    }

    /// Dense nodes, contexts and edges in the build's file order, for the
    /// passes that walk the whole graph (dependency attribution).
    pub(super) fn dense_view(&self, order: &[PathId]) -> DenseView {
        let mut nodes = Vec::new();
        let mut index = HashMap::<NodeId, usize>::default();
        let mut contexts = Vec::new();
        for path in order {
            let Some(file) = self.files.get(path) else {
                continue;
            };
            for node in &file.fragment.nodes {
                let id = NodeId {
                    path: *path,
                    span: node.span,
                };
                index.insert(id, nodes.len());
                contexts.push(self.contexts.get(&id).copied().unwrap_or(0));
                nodes.push(node.clone());
            }
        }
        let mut outgoing = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
        for path in order {
            let Some(file) = self.files.get(path) else {
                continue;
            };
            for resolved in file.resolved.iter().flatten() {
                if let (Some(source), Some(target)) = (
                    resolved
                        .source
                        .and_then(|source| index.get(&source).copied()),
                    index.get(&resolved.target).copied(),
                ) {
                    outgoing[source].push((target, resolved.kind));
                }
            }
        }
        (nodes, contexts, outgoing)
    }

    /// Whether the kept solvers answered the last build.
    #[cfg(test)]
    pub(super) fn solvers_kept(&self) -> bool {
        self.solvers_valid
    }

    /// Debug summary for tests: `(path, span) -> context`, sorted.
    #[cfg(test)]
    pub(super) fn context_table(&self) -> std::collections::BTreeMap<(String, Span), u8> {
        self.contexts
            .iter()
            .map(|(id, context)| ((self.path_name(id.path).to_owned(), id.span), *context))
            .collect()
    }
}
