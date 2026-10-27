// app-game dnd-playground App.tsx:109: a component passes a prop to a helper
// that calls a method on a member path of it (`root.children.filter`).
import { createSignal } from "solid-js";

type Block = { key: string; children: Block[] };

function removeBlocks(root: Block, keys: string[]): Block {
  return { ...root, children: root.children.filter((child) => !keys.includes(child.key)) };
}

export function Tree(props: { root: Block }) {
  const [root, setRoot] = createSignal<Block>(removeBlocks(props.root, ["a"]));
  return <button onClick={() => setRoot({ key: "root", children: [] })}>{root().children.length}</button>;
}
