import { createStore } from "solid-js";

export const [state, setState] = createStore({ count: 0 });

export function readCount() {
  return state.count;
}
