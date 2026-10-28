import { createStore } from "solid-js";

type State = {
  opt?: { b: number };
  list?: number[];
  deep?: { list: number[] };
  maybe: { b: number } | null;
};
const [store] = createStore<State>({ maybe: null });

// Uncertifiable: an optional access reads the nullish link, so tsc reports
// nothing, and when the link holds a value the write is dropped.
export function OptionalLink() {
  store.list?.push(2);
  store.deep?.list.push(1);
}
