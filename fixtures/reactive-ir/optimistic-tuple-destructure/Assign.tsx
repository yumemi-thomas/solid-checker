// The claim: **assignment** destructuring of a reactive tuple binds a reactive
// source for the optimistic primitives too.
//
// `[count] = createOptimistic(0)` — an assignment, not a declaration — takes a
// different path in `source_discovery.rs` from `App.tsx`'s form, and that path
// is guarded by `Dialect::returns_reactive_tuple`. Until ADR 0111 that guard
// was a hardcoded `CreateSignal | CreateStore | CreateResource`: 1.x's list,
// naming neither optimistic primitive and naming one 2.0 does not have. So a
// 2.0 project assigning either bound nothing this path could see, and a read
// through it was traced to no source and reported nowhere.
//
// The ADR recorded the fix as "correct, and exercised by nothing". This is the
// exercise. Dropping `CreateOptimistic` and `CreateOptimisticStore` from
// `returns_reactive_tuple` removes exactly the `count` and `row.label`
// findings below and leaves `name`, `cell.label` and all of `App.tsx`
// untouched — verified by doing it.
//
// `createSignal` and `createStore` are the controls: both were in the old list,
// so a regression that took out the whole path would show here too.
import { createEffect, createOptimistic, createOptimisticStore, createSignal, createStore } from "solid-js";

export function OptimisticAssigned() {
  let count!: () => number;
  [count] = createOptimistic(0);
  createEffect(
    () => count(),
    () => {
      console.log(count());
    },
  );
  return <span />;
}

export function OptimisticStoreAssigned() {
  let row!: { label: string };
  [row] = createOptimisticStore({ label: "a" });
  createEffect(
    () => row.label,
    () => {
      console.log(row.label);
    },
  );
  return <span />;
}

export function SignalAssigned() {
  let name!: () => string;
  [name] = createSignal("a");
  createEffect(
    () => name(),
    () => {
      console.log(name());
    },
  );
  return <span />;
}

export function StoreAssigned() {
  let cell!: { label: string };
  [cell] = createStore({ label: "a" });
  createEffect(
    () => cell.label,
    () => {
      console.log(cell.label);
    },
  );
  return <span />;
}
