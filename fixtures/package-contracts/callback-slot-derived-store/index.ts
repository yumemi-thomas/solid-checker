import {
  createMemo,
  createOptimistic,
  createOptimisticStore,
  createSignal,
  createStore
} from "solid-js";

export interface Cart {
  items: string[];
}

// Negative, and the 2.0 half of `@solid-primitives/flux-store`'s
// `createFluxStore`. 2.0's `createStore` has two forms and only the derived one
// takes a compute; the runtime picks between them on
// `typeof first === "function"` alone (`@solidjs/signals` `dist/dev.js:9371`),
// so an argument the declaration proves is *not* a function selects the plain
// form and is never invoked. Arity says nothing here and is not consulted --
// see `primitive_slot_roots_parameter_invoke`.
export function makeStore(initial: Cart): unknown {
  const [state] = createStore(initial);
  return state;
}

// Negative, at the arity the derived form also occupies: two arguments, and
// still the plain form, because `initial` is provably not a function and the
// second argument is the plain form's options object. Only callability
// separates them.
export function makeNamedStore(initial: Cart): unknown {
  const [state] = createStore(initial, { name: "cart" });
  return state;
}

// Positive, and what keeps the withdrawal a grounding rule rather than a
// blanket exemption for `createStore`: the derived form, with the compute
// proven callable. The tracked row survives -- with *no execution point*,
// because `Solid2::tracked_callback_timing` deliberately states no schedule for
// `createStore` (its derived overload never accepted the probe's call shape).
// Attribution is proven and is published; the schedule is not, and is left
// unstated rather than guessed as `queued`.
export function projectStore(compute: (store: Cart) => void, seed: Cart): unknown {
  const [state] = createStore(compute, seed);
  return state;
}

// The optimistic pair, one primitive over, and the case the shared arity clause
// used to erase: `createOptimisticStore`'s plain form takes *no* options
// argument at all in these typings, so two arguments already implies the
// derived form -- and the rule never needed that, because callability decides
// both. Negative first.
export function makeOptimisticStore(initial: Cart): unknown {
  const [state] = createOptimisticStore(initial);
  return state;
}

// Positive, and unscheduled for the same reason as `projectStore`:
// `createOptimisticStore` is the second primitive 2.0 states no tracked timing
// for.
export function projectOptimisticStore(
  compute: (store: Cart) => void,
  seed: Cart
): unknown {
  const [state] = createOptimisticStore(compute, seed);
  return state;
}

// Negative: `createOptimistic`'s plain form is `Exclude<T, Function>`, the
// `createSignal` shape, so an object-typed parameter is the plain form.
export function makeOptimistic(initial: Cart): unknown {
  const [value] = createOptimistic(initial);
  return value;
}

// Positive, and scheduled: unlike the store pair, `createOptimistic`'s tracked
// compute *is* measured (`optimisticComputed` is `computed` plus one field, so
// it runs during the creating call), so this row publishes `same-stack`.
export function makeDerivedOptimistic(compute: () => number): unknown {
  const [value] = createOptimistic(compute);
  return value;
}

// Negative: 2.0's `createSignal(fn, options?)` is reachable at one argument, so
// arity proves nothing here and callability alone separates the plain form
// (`Exclude<T, Function>`) from the derived one. An object-typed parameter is
// the plain form.
export function makeSignal(initial: Cart): unknown {
  const [value] = createSignal(initial);
  return value;
}

// Positive: the same primitive with the compute proven callable is the derived
// form, and keeps its tracked row.
export function makeDerivedSignal(compute: () => number): unknown {
  const [value] = createSignal(compute);
  return value;
}

// Positive control: `createMemo`'s argument 0 is the compute in every 2.0
// overload, unconditionally, so it needs no callability proof and must keep its
// row on an untyped artifact too.
export function derive(compute: () => number): () => number {
  return createMemo(compute);
}
