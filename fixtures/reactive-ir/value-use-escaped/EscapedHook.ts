import { createStore } from "solid-js";

const [held] = createStore({ v: 1 });

export function useHeld() {
  return held;
}
