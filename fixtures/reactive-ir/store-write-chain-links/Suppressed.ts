import { createStore } from "solid-js";

type State = {
  opt?: { b: number };
  list?: number[];
  deep?: { list: number[] };
  maybe: { b: number } | null;
};
const [store] = createStore<State>({ maybe: null });

// Uncertifiable: a directive silences tsc in this file, so its TS18048 is no
// premise here.
export function SuppressedLink() {
  // @ts-expect-error TS18048: 'store.opt' is possibly 'undefined'.
  store.opt.b = 1;
}
