//! The kept summary-node index (S7c; S7 design §4c).
//!
//! The graph's nodes are every file's summary functions concatenated in
//! file order; a node's index is its position there, and three maps find it:
//! by path (`nodes_by_path`), by path and span (`indexes`, last wins on a
//! duplicate) and by declaration symbol (`by_symbol`, last wins), with the
//! function set the resolver reads. The index is kept between builds and
//! patched per file: while the build lists the same paths in the same order
//! and no file's node count changed, every node keeps its index, so a file
//! whose nodes moved swaps them in place and re-keys only its own entries.
//! Any other change (a file added, removed or reordered, a count changed)
//! builds the index afresh. Shadow and verify-reused modes compare a patched
//! index with a fresh one; a difference poisons the store and the fresh one
//! is served.

use std::ops::Range;

use super::*;
use crate::base::memo::derived::SolverId;

/// The index's kept state between builds.
const NODE_INDEX_STATE: SolverId = SolverId(386);

/// One file's nodes this build: the graph chunk's, reused, or computed now.
pub(super) enum FileNodes<'a> {
    Reused(&'a [SummaryNode]),
    Computed(Vec<SummaryNode>),
}

impl FileNodes<'_> {
    fn nodes(&self) -> &[SummaryNode] {
        match self {
            Self::Reused(nodes) => nodes,
            Self::Computed(nodes) => nodes,
        }
    }
}

/// The graph's nodes and the maps that find them.
#[derive(Default)]
pub(super) struct NodeIndex {
    pub(super) nodes: Vec<SummaryNode>,
    pub(super) nodes_by_path: HashMap<String, Vec<usize>>,
    pub(super) indexes: HashMap<(String, Span), usize>,
    pub(super) by_symbol: HashMap<SymbolId, usize>,
    /// Every node's path and span: the resolver's function set.
    pub(super) functions: Arc<HashSet<(String, Span)>>,
    /// Each file's path and node range, in file order.
    files: Vec<(String, Range<usize>)>,
    /// Each symbol's nodes, ascending: `by_symbol` is the last.
    symbol_nodes: HashMap<SymbolId, Vec<usize>>,
}

impl NodeIndex {
    /// The index of `files`' nodes, built afresh.
    fn fresh(files: &[(&str, FileNodes<'_>)]) -> Self {
        let mut index = Self::default();
        for (path, nodes) in files {
            let start = index.nodes.len();
            index.nodes.extend(nodes.nodes().iter().cloned());
            index
                .files
                .push(((*path).to_owned(), start..index.nodes.len()));
        }
        index.nodes_by_path = function_indices_by_path(&index.nodes);
        for (position, node) in index.nodes.iter().enumerate() {
            index
                .indexes
                .insert((node.path.clone(), node.span), position);
            if let Some(symbol) = &node.symbol {
                index.by_symbol.insert(symbol.clone(), position);
                index
                    .symbol_nodes
                    .entry(symbol.clone())
                    .or_default()
                    .push(position);
            }
        }
        index.functions = Arc::new(
            index
                .nodes
                .iter()
                .map(|node| (node.path.clone(), node.span))
                .collect(),
        );
        index
    }

    /// Brings the index to `files`, patched in place when every file keeps
    /// its path, position and node count; `true` when patched.
    fn update(&mut self, files: Vec<(&str, FileNodes<'_>)>) -> bool {
        let keeps_shape = self.files.len() == files.len()
            && self
                .files
                .iter()
                .zip(&files)
                .all(|((kept, range), (path, nodes))| {
                    kept == path && range.len() == nodes.nodes().len()
                });
        if !keeps_shape {
            *self = Self::fresh(&files);
            return false;
        }
        let mut functions_moved = false;
        for ((_, range), (_, nodes)) in self.files.clone().into_iter().zip(files) {
            if self.nodes[range.clone()] == *nodes.nodes() {
                continue;
            }
            functions_moved = true;
            for position in range.clone() {
                let node = &self.nodes[position];
                if self.indexes.get(&(node.path.clone(), node.span)) == Some(&position) {
                    self.indexes.remove(&(node.path.clone(), node.span));
                }
                if let Some(symbol) = &node.symbol
                    && let Some(positions) = self.symbol_nodes.get_mut(symbol)
                {
                    positions.retain(|other| *other != position);
                    match positions.last() {
                        Some(last) => {
                            self.by_symbol.insert(symbol.clone(), *last);
                        }
                        None => {
                            self.by_symbol.remove(symbol);
                            self.symbol_nodes.remove(symbol);
                        }
                    }
                }
            }
            let new = match nodes {
                FileNodes::Reused(nodes) => nodes.to_vec(),
                FileNodes::Computed(nodes) => nodes,
            };
            for (position, node) in range.clone().zip(new) {
                self.nodes[position] = node;
            }
            for position in range {
                let node = &self.nodes[position];
                self.indexes
                    .insert((node.path.clone(), node.span), position);
                if let Some(symbol) = &node.symbol {
                    let positions = self.symbol_nodes.entry(symbol.clone()).or_default();
                    let at = positions.partition_point(|other| *other < position);
                    positions.insert(at, position);
                    let last = *positions.last().expect("a symbol with a node");
                    self.by_symbol.insert(symbol.clone(), last);
                }
            }
        }
        if functions_moved {
            self.functions = Arc::new(
                self.nodes
                    .iter()
                    .map(|node| (node.path.clone(), node.span))
                    .collect(),
            );
        }
        true
    }

    /// The first table in which two indexes differ.
    fn first_difference(&self, fresh: &Self) -> Option<&'static str> {
        if self.nodes != fresh.nodes {
            Some("nodes")
        } else if self.nodes_by_path != fresh.nodes_by_path {
            Some("nodes_by_path")
        } else if self.indexes != fresh.indexes {
            Some("indexes")
        } else if self.by_symbol != fresh.by_symbol {
            Some("by_symbol")
        } else if self.functions != fresh.functions {
            Some("functions")
        } else {
            None
        }
    }
}

/// This build's node index: the kept one patched by `files`, or built
/// afresh. Hand it back with [`keep`] when the stage is done with it.
pub(super) fn node_index(
    lookup: &SemanticLookup<'_>,
    files: Vec<(&str, FileNodes<'_>)>,
) -> NodeIndex {
    let store = lookup.memo_store();
    let mut index = store
        .derived()
        .solver::<NodeIndex, _>(NODE_INDEX_STATE, |index, _| std::mem::take(index));
    if crate::audit_off("node_index") {
        index = NodeIndex::default();
    }
    let check = store
        .check_solvers()
        .then(|| {
            files
                .iter()
                .map(|(path, nodes)| (*path, FileNodes::Reused(nodes.nodes())))
                .collect::<Vec<_>>()
        })
        .map(|files| NodeIndex::fresh(&files));
    let patched = index.update(files);
    if patched
        && let Some(fresh) = check
        && let Some(table) = index.first_difference(&fresh)
    {
        store.poison(&format!(
            "IR node index verify mismatch: {table}: the patched index differs from a fresh one"
        ));
        return fresh;
    }
    index
}

/// Keeps `index` for the next build.
pub(super) fn keep(lookup: &SemanticLookup<'_>, index: NodeIndex) {
    lookup
        .memo_store()
        .derived()
        .solver::<NodeIndex, _>(NODE_INDEX_STATE, |kept, _| *kept = index);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mulberry32: a failing run replays from its seed.
    struct Random(u32);

    impl Random {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self.0.wrapping_add(0x6d2b_79f5);
            let mut t = self.0;
            t = (t ^ (t >> 15)).wrapping_mul(t | 1);
            t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
            let unit = f64::from(t ^ (t >> 14)) / 4_294_967_296.0;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let index = (unit * n as f64) as usize;
            index.min(n.saturating_sub(1))
        }
    }

    fn node(random: &mut Random, path: &str) -> SummaryNode {
        // Few spans and symbols, so a file repeats a span and files share
        // a symbol (both maps keep the last).
        let start = u32::try_from(random.below(6) * 10).unwrap();
        SummaryNode {
            path: path.into(),
            span: Span::new(start, start + 5),
            body: Span::new(start + 1, start + 5),
            name: None,
            symbol: (random.below(3) > 0).then(|| SymbolId::from(format!("s{}", random.below(5)))),
            runtime_identity: String::new(),
            parameters: Vec::new(),
            exported: false,
            r#async: false,
            source_hash: solid_facts::core::SourceHash::of(""),
        }
    }

    fn file(random: &mut Random, path: &str, count: usize) -> Vec<SummaryNode> {
        (0..count).map(|_| node(random, path)).collect()
    }

    /// Through random edits (a node's span or symbol moved, a node added or
    /// removed, a file added or removed in the middle of path order), the
    /// kept index answers what a fresh one does, and is patched exactly when
    /// every file keeps its path, position and node count.
    #[test]
    fn a_patched_node_index_equals_a_fresh_one() {
        let mut patched = 0;
        for seed in 1..=80 {
            let mut random = Random(seed);
            let mut files = (0..2 + random.below(6))
                .map(|index| {
                    let path = format!("/p/{:02}.ts", index * 2);
                    let count = random.below(5);
                    let nodes = file(&mut random, &path, count);
                    (path, nodes)
                })
                .collect::<Vec<_>>();
            let mut kept = NodeIndex::default();
            for step in 0..20 {
                let shape = files
                    .iter()
                    .map(|(path, nodes)| (path.clone(), nodes.len()))
                    .collect::<Vec<_>>();
                let was = kept
                    .files
                    .iter()
                    .map(|(path, range)| (path.clone(), range.len()))
                    .collect::<Vec<_>>();
                let reused = random.below(2) == 0;
                let input = files
                    .iter()
                    .map(|(path, nodes)| {
                        let nodes = if reused {
                            FileNodes::Reused(nodes.as_slice())
                        } else {
                            FileNodes::Computed(nodes.clone())
                        };
                        (path.as_str(), nodes)
                    })
                    .collect();
                let was_patched = kept.update(input);
                assert_eq!(
                    was_patched,
                    was == shape,
                    "seed {seed} step {step}: patch decision"
                );
                patched += usize::from(was_patched);
                let fresh = NodeIndex::fresh(
                    &files
                        .iter()
                        .map(|(path, nodes)| (path.as_str(), FileNodes::Reused(nodes.as_slice())))
                        .collect::<Vec<_>>(),
                );
                assert_eq!(
                    kept.first_difference(&fresh),
                    None,
                    "seed {seed} step {step}: the kept index differs from a fresh one"
                );
                assert_eq!(
                    kept.files, fresh.files,
                    "seed {seed} step {step}: file ranges"
                );
                assert_eq!(
                    kept.symbol_nodes, fresh.symbol_nodes,
                    "seed {seed} step {step}"
                );
                // The next edit.
                match random.below(6) {
                    0 => {
                        // A file added in the middle of path order.
                        let at = random.below(files.len() + 1);
                        let path =
                            format!("/p/{:02}x{step}.ts", at.saturating_mul(2).saturating_sub(1));
                        let count = random.below(4);
                        let nodes = file(&mut random, &path, count);
                        files.insert(at, (path, nodes));
                        files.sort_by(|left, right| left.0.cmp(&right.0));
                    }
                    1 if files.len() > 1 => {
                        // A file removed from the middle.
                        let at = random.below(files.len());
                        files.remove(at);
                    }
                    2 => {
                        // A node added or removed.
                        let at = random.below(files.len());
                        let (path, nodes) = &mut files[at];
                        if nodes.is_empty() || random.below(2) == 0 {
                            let added = node(&mut random, path);
                            nodes.push(added);
                        } else {
                            nodes.pop();
                        }
                    }
                    _ => {
                        // A node's span or symbol moved: same counts.
                        let at = random.below(files.len());
                        let (path, nodes) = &mut files[at];
                        if !nodes.is_empty() {
                            let which = random.below(nodes.len());
                            nodes[which] = node(&mut random, path);
                        }
                    }
                }
            }
        }
        assert!(patched > 500, "the patch path ran ({patched})");
    }
}
