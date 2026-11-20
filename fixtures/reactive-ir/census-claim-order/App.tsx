import { createSignal } from "solid-js";

export type TreeNode = Readonly<{
}>;

export function Tree(props: { root?: TreeNode; children: MapFn }) {
}

type MapFn = (props: {
}) => JSX.Element;

export function Mapper0() {
  const cache = new Map<string, any>();
  const reconciledElements = new Map<string, any>();
  const { node } = entry;
  const id = node.id;
  let cached = cache.get(id);
  const [getNode, setNode] = createSignal(node);
  cache.set(id, {
    setNode,
  });
  for (const child of node.children) {
    const childElement = reconciledElements.get(child.id);
  }
  reconciledElements.set(id, cached.element);
}

export function Mapper1() {
  const cache = new Map<string, any>();
  const reconciledElements = new Map<string, any>();
  const { node } = entry;
  const id = node.id;
  let cached = cache.get(id);
  const [getNode, setNode] = createSignal(node);
  cache.set(id, {
    setNode,
  });
  for (const child of node.children) {
    const childElement = reconciledElements.get(child.id);
  }
  reconciledElements.set(id, cached.element);
}

gp = Object.getPrototypeOf([]);
gp[patchKey()] = function () {};
