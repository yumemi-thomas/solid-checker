import { createStore } from "solid-js";

// ADR 1232: hooks of another module that create a store per call and return it with its setter.
export function useVals() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return { s1, setS1 };
}

export function useKept() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return { s1, setS1 };
}

export function useSetter() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return { s1, setS1 };
}
