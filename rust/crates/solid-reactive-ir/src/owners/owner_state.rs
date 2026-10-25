//! The owner graph on kept state (S7f, ADR 0393).
//!
//! The graph keeps what the owner analysis derives from the files' fragments
//! ([`CachedOwnerFile`]) between builds, keyed by stable node ids (an interned
//! path and the function's span), and re-resolves only what a build changed:
//!
//! - **Patching.** A recomputed fragment whose content equals the kept one
//!   changes nothing downstream. Otherwise its nodes, symbol winners and
//!   resolved edges are patched: its own edges are resolved again, and every
//!   file's edges only when the fragment's *shape* (its nodes' spans, symbols
//!   and generator flags, all that edge resolution reads of another file)
//!   moved.
//! - **Solve.** The context bits are solved with the dense worklist the
//!   analysis always ran ([`propagate_owner_contexts`]) over the patched
//!   graph, in the build's file order; the nodes whose context moved are what
//!   the per-file emission ([`super::owner_emission`]) is dirtied by. (Kept
//!   cone solves on K2 solvers were removed: saved under 5 ms in the
//!   2026-10-25 audit.)
//!
//! Paths are interned once ([`PathId`]); an id is never reused.

use std::collections::BTreeSet;

use super::*;

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
    namespace_nodes: BTreeSet<NodeId>,
    /// The namespace hand-outs of the last build.
    named: HashSet<NodeId>,
    candidates: HashSet<NodeId>,
    /// The final context of every node.
    contexts: HashMap<NodeId, u8>,
    // Accumulators of the build under way.
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
        file.resolved.clear();
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
        }
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
        self.files
            .get_mut(&path)
            .expect("the file is held")
            .resolved = next;
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

    // ---- the build ---------------------------------------------------

    /// Brings the graph and its contexts up to the fragments the build
    /// stored, then reports what moved. `order` lists the paths of the
    /// build's files (path order, each once).
    pub(super) fn settle(&mut self, order: &[PathId], hand_outs: &HandOuts<'_>) -> Moved {
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
        self.named = named;
        self.candidates = candidates;

        let moved_files = std::mem::take(&mut self.changed_files);
        self.structural = false;
        let mut moved = self.solve_fresh(order);
        moved.files = moved_files;
        moved
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

    fn solve_fresh(&mut self, order: &[PathId]) -> Moved {
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

    /// Debug summary for tests: `(path, span) -> context`, sorted.
    #[cfg(test)]
    pub(super) fn context_table(&self) -> std::collections::BTreeMap<(String, Span), u8> {
        self.contexts
            .iter()
            .map(|(id, context)| ((self.path_name(id.path).to_owned(), id.span), *context))
            .collect()
    }
}
