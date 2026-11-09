import { createStore } from "solid-js";

export function useVals() {
  const [s, setS] = createStore({ v: 0 });
  return { s, setS };
}
