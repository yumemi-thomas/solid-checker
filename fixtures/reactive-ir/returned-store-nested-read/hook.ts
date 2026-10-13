import { createMemo, createRenderEffect, createStore } from "solid-js";

declare function sink(value: unknown): void;

// The `@tanstack/solid-query` `useMutation` shape: a render effect reads the
// store and throws (ADR 0281).
export function useThrowingEffect() {
  const [state] = createStore({ search: "", failed: false });
  createRenderEffect(
    () => {
      const failed = state.failed;
      if (failed) throw new Error("failed");
    },
    () => {},
  );
  return state;
}

// A store member read consumed inside a callback body.
export function useMemoRead() {
  const [state] = createStore({ search: "" });
  createMemo(() => {
    const search = state.search;
    return search.length;
  });
  return state;
}

// A nested throw that touches no store (already proven; pinned).
export function useNestedThrow() {
  const [state] = createStore({ search: "" });
  createRenderEffect(
    () => {
      throw new Error("x");
    },
    () => {},
  );
  return state;
}

// Escapes: the root or a sub-proxy reaches other code. No proof.
export function useEscapedRoot() {
  const [state] = createStore({ search: "" });
  sink(state);
  return state;
}
export function useEscapedInArrow() {
  const [state] = createStore({ search: "" });
  sink(() => state);
  return state;
}
export function useEscapedInObject() {
  const [state] = createStore({ search: "" });
  sink({ s: state });
  return state;
}
export function useEscapedSubProxy() {
  const [state] = createStore({ search: "", items: [1] });
  sink(state.items);
  return state;
}
export function useEscapedReturnedMember() {
  const [state] = createStore({ search: "", items: [1] });
  createMemo(() => state.items);
  return state;
}
export function useEscapedThis() {
  const [state] = createStore({ search: "", items: [1] });
  createMemo(() => state.items.indexOf(1));
  return state;
}
