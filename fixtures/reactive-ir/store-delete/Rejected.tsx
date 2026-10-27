import { createStore } from "solid-js";

const [store] = createStore<{ v?: number; o?: { b?: number }; n: { b?: number } | null }>({
  n: null,
});

// Silent: every delete here is a TypeScript error on the published rc.17
// typings, so it is TypeScript's to report. A type assertion on the operand
// is TS2703 (TypeScript sees through parentheses only); a link that may be
// `undefined` or `null` is TS18048 / TS18047 under strictNullChecks.
export function Rejected() {
  delete store.v!;
  delete (store.v as number | undefined);
  delete store.o.b;
  delete store.n.b;
}
