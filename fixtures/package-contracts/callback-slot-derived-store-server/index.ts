import { createMemo, createStore } from "solid-js";

export interface Cart {
  items: string[];
}

// Positive, and the whole reason this fixture exists. The 2.0 runtime picks
// `createStore`'s derived form on `typeof first === "function"` alone
// (`@solidjs/signals` `dist/dev.js:9371`; `solid-js@2.0.0-rc.3
// dist/server.js:896` routes the same shape through `createProjection`), with
// no seed-argument condition -- and this entrypoint's plain form is
// `store: T | Store<T>` with no `NoFn`, so the call below is `tsc`-clean *and*
// derived. It really does invoke `compute`. A premise that required the seed at
// argument 1 withdrew this true claim, which is why the rule is callability and
// only callability.
export function projectSeedless(compute: (store: Cart) => void): unknown {
  const [state] = createStore(compute);
  return state;
}

// Positive: the same call with the seed present, so nothing here depends on
// which of the two arities the claim came from.
export function projectSeeded(compute: (store: Cart) => void, seed: Cart): unknown {
  const [state] = createStore(compute, seed);
  return state;
}

// Negative, at the same one-argument shape as `projectSeedless`: the plain form
// is admissible on this entrypoint for an object too, and an argument the
// declaration proves is not a function is never invoked. The pair is separated
// by callability and by nothing else.
export function plainStore(initial: Cart): unknown {
  const [state] = createStore(initial);
  return state;
}

// Positive control: `createMemo`'s argument 0 is the compute unconditionally, so
// it needs no callability proof. If this row ever disappears the withdrawal has
// taken the whole branch with it.
export function derive(compute: () => number): () => number {
  return createMemo(compute);
}
