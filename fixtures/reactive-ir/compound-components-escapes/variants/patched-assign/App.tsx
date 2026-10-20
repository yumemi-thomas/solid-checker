import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Root() {
  return <i />;
}

function Sub() {
  const value = count();
  return <b>{value}</b>;
}

// Uncertifiable: the project replaces `Object.assign`, so it need not store
// the member the way the standard library does. A write to `Object.assign`,
// to a global `Object`, or `Object.defineProperty(Object, …)` withholds every
// `Object.assign` holder, the way ADR 0338 withholds a patched `bind`.
Object.assign = ((_target: unknown, _members: { Sub: () => unknown }) => {
  return { Sub: () => null };
}) as typeof Object.assign;

const Tabs = Object.assign(Root, { Sub });

export function Screen() {
  return <Tabs.Sub />;
}
