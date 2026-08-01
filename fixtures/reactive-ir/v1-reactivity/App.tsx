import { createEffect, createMemo, createSignal } from "solid-js";
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

// v1/no-async-tracked-scope: tracking stops at the first await, so theme() is
// never a dependency and the effect stops responding to it.
export function AsyncEffect() {
  const [theme] = createSignal("dark");
  createEffect(async () => {
    await load();
    apply(theme());
  });
}

// The same await inside a synchronous scope is not this rule's business, and
// the 1.x seed argument is a value rather than a second callback: reading slot
// 1 as a tracked scope is how a 2.0-shaped table misreports this call.
export function SyncEffect() {
  const [theme] = createSignal("dark");
  createEffect((previous?: string) => {
    apply(theme());
    return previous;
  }, "light");
}

// v1/expected-function-got-expression: count() runs during setup and its
// result is bound as the listener.
export function CalledHandler() {
  const [count] = createSignal(0);
  return <button onClick={count()}>{count()}</button>;
}

// A factory call in the same position is correct — it returns the handler —
// and is the false positive a syntax-only rule cannot avoid reporting.
export function FactoryHandler() {
  return <button onClick={makeHandler()}>ok</button>;
}

declare function load(): Promise<void>;
declare function apply(theme: string): void;
declare function makeHandler(): () => void;
