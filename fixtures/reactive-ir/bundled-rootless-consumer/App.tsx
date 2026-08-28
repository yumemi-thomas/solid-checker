import { type Component, createMemo, createSignal } from "solid-js";
import {
  createRootPool,
  createSingletonRoot,
  createSubRoot,
} from "@solid-primitives/rootless";

declare const opaqueFactory: () => () => number;

// These reduced local package bytes deliberately do not match the published
// rootless artifact. Local source inference preserves the exact returned memo
// where it can; the checker must not borrow missing relations from the
// first-party bundle by package name.
export const Rootless: Component = () => {
  const [count] = createSignal(0);
  const doubled = createSubRoot(() => createMemo(() => count() * 2));
  const useSingleton = createSingletonRoot(() => createMemo(() => count() * 3));
  const tripled = useSingleton();
  const usePool = createRootPool<number, () => number>(() => createMemo(() => count() * 4));
  const quadrupled = usePool(1);

  // An ambient callback has no exact local body. Its returned function keeps
  // its TypeScript type, but the checker must not guess that it is reactive.
  const useOpaque = createSingletonRoot(opaqueFactory);
  const opaque = useOpaque();

  setTimeout(() => doubled(), 0);
  setTimeout(() => tripled(), 0);
  setTimeout(() => quadrupled(), 0);
  setTimeout(() => opaque(), 0);
  return <div>{count()}</div>;
};
