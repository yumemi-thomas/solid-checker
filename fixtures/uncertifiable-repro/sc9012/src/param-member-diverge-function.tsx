// app-game solid-dnd-playground SortableOverlayDemo.tsx:41 (solid-dnd createDnd): a component hands an options object
// to a project hook whose helper calls one of its members; the object names that member by a reference to a value a
// call returned (here a project factory's closure), not by a function literal or a declared function.
import { createMemo } from "solid-js";

type TreeNode = { id: string; children: TreeNode[] };

function makeChildren(): (node: TreeNode) => TreeNode[] {
  return (node) => node.children;
}

function createTreeView(props: { items: () => TreeNode[]; getChildren: (node: TreeNode) => TreeNode[] }) {
  const reconcile = (nodes: TreeNode[]): number =>
    nodes.reduce((count, node) => count + 1 + reconcile(props.getChildren(node)), 0);
  const total = createMemo(() => reconcile(props.items()));
  return { total };
}

export function Explorer(props: { roots: TreeNode[] }) {
  const childrenOf = makeChildren();
  const tree = createTreeView({ items: () => props.roots, getChildren: childrenOf });
  return <p>{tree.total()}</p>;
}
