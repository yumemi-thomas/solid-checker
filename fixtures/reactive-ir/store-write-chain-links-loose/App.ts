import { createStore } from "solid-js";

type State = {
  v: number;
  opt?: { b: number };
  list?: number[];
  maybe: { b: number } | null;
};
const [store] = createStore<State>({ v: 0, maybe: null });

// Violation: no link past the root.
export function SingleLink() {
  store.v = 1;
}

// Without strictNullChecks the link types drop `undefined` and `null`, and the
// facts record neither the option nor the property's optionality: these are
// uncertifiable here, where a strict project (`store-write-chain-links`) is
// silent because tsc reports TS18048/TS18047 (docs/open-work.md).
export function NullableLink() {
  store.opt.b = 1;
  store.list.push(1);
  store.maybe.b = 1;
  store.list?.push(2);
}
