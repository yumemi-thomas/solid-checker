// @ts-nocheck
import { createStore } from "solid-js";

type State = {
  opt?: { b: number };
  list?: number[];
  deep?: { list: number[] };
  maybe: { b: number } | null;
};
const [store] = createStore<State>({ maybe: null });

// Uncertifiable: tsc does not check this file.
export function UncheckedLink() {
  store.opt.b = 1;
}
