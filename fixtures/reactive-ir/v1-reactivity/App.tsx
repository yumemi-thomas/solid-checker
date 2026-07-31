import { createMemo, createSignal } from "solid-js";
import { createStore } from "solid-js/store";

// v1/uncalled-accessor: the accessor is interpolated, so the template renders
// the function source instead of the value.
export function Templated() {
  const [count] = createSignal(0);
  return <div>{`count is ${count}`}</div>;
}

// A memo accessor is an accessor: passing it on is correct, and upstream
// reports it while leaving the structurally identical signal alone.
// https://github.com/solidjs-community/eslint-plugin-solid/issues/182
export function PassedOn() {
  const [count] = createSignal(0);
  const doubled = createMemo(() => count() * 2);
  return <div>{consume(count)}{consume(doubled)}</div>;
}

function consume(source: () => number) {
  return source();
}

// v1/no-direct-mutation: a store is a readonly proxy, so the write is dropped.
export function Mutates() {
  const [store] = createStore({ open: false });
  const toggle = () => {
    store.open = true;
  };
  return <button onClick={toggle}>{String(store.open)}</button>;
}
