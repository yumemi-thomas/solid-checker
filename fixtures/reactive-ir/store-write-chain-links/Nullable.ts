import { createStore } from "solid-js";

type State = {
  opt?: { b: number };
  list?: number[];
  deep?: { list: number[] };
  maybe: { b: number } | null;
};
const [store] = createStore<State>({ maybe: null });

// Silent: tsc reports each link (TS18048 at the optional ones, TS18047 at the
// nullable one), so the write is TypeScript's. This file does not type-check
// on purpose.
export function NullableLink() {
  store.opt.b = 1;
  store.list.push(1);
  store.maybe.b = 1;
}

// A string literal naming a directive is no directive: the nullable links
// above stay TypeScript's (silent).
export const note = "@ts-ignore and @ts-nocheck are text here";
