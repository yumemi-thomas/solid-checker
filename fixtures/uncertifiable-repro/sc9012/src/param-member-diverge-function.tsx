// app-game solid-dnd-playground FlipDebugOverlay.tsx:53 (useElementTrails, `addCycleMarker` destructured from useGapTrail): a
// component hands an options object to a project hook whose helper calls one of its members; the object names that member by
// a reference to a closure destructured from a project hook's result, not by a function literal or a declared function.
import { createMemo } from "solid-js";

type TreeNode = { id: string; children: TreeNode[] };

function useChildren() {
  return { childrenOf: (node: TreeNode): TreeNode[] => node.children };
}

function createTreeView(props: { items: () => TreeNode[]; getChildren: (node: TreeNode) => TreeNode[] }) {
  const reconcile = (nodes: TreeNode[]): number =>
    nodes.reduce((count, node) => count + 1 + reconcile(props.getChildren(node)), 0);
  const total = createMemo(() => reconcile(props.items()));
  return { total };
}

export function Explorer(props: { roots: TreeNode[] }) {
  const { childrenOf } = useChildren();
  const tree = createTreeView({ items: () => props.roots, getChildren: childrenOf });
  return <p>{tree.total()}</p>;
}
