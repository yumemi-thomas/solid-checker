//! The summary-node index (S7c).
//!
//! The graph's nodes are every file's summary functions concatenated in
//! file order; a node's index is its position there, and three maps find it:
//! by path (`nodes_by_path`), by path and span (`indexes`, last wins on a
//! duplicate) and by declaration symbol (`by_symbol`, last wins), with the
//! function set the resolver reads. It is built afresh every build (an index
//! kept between builds and patched per file was removed: saved under 5 ms in
//! the 2026-10-25 audit).

use super::*;

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
}

impl NodeIndex {
    /// The index of `files`' nodes, in file order.
    pub(super) fn new(files: &[(&str, FileNodes<'_>)]) -> Self {
        let mut index = Self::default();
        for (_, nodes) in files {
            index.nodes.extend(nodes.nodes().iter().cloned());
        }
        index.nodes_by_path = function_indices_by_path(&index.nodes);
        for (position, node) in index.nodes.iter().enumerate() {
            index
                .indexes
                .insert((node.path.clone(), node.span), position);
            if let Some(symbol) = &node.symbol {
                index.by_symbol.insert(symbol.clone(), position);
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
}
