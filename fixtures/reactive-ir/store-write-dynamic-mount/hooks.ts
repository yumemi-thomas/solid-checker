import { createStore } from "solid-js";

// ADR 1234: hooks of another module that create a store per call and return it with its setter (ADR 1232's shape), one per component.
export function useMounted() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return { s1, setS1 };
}

export function useClosing() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return { s1, setS1 };
}

export function useThroughSetter() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return { s1, setS1 };
}

export function useInWindow() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return { s1, setS1 };
}

export function useOutsideTag() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return { s1, setS1 };
}

export function useShadowed() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return { s1, setS1 };
}

export function useNamespace() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return { s1, setS1 };
}
