import { type Component, createMemo, createSignal } from "solid-js";
import { createSubRoot } from "@solid-primitives/rootless";

// createSubRoot returns exactly the value produced by its callback. The
// relational return contract must therefore preserve this memo as a reactive
// source rather than treating the generic T as an opaque value.
export const Rootless: Component = () => {
  const [count] = createSignal(0);
  const doubled = createSubRoot(() => createMemo(() => count() * 2));
  setTimeout(() => doubled(), 0);
  return <div>{count()}</div>;
};
