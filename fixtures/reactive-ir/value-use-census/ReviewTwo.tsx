import { createMemo, createSignal, createStore } from "solid-js";
import * as Ns from "./ReviewNs";

// Item 5: a pattern over a store root reached through a holder slot.
// Chrome warns (STRICT_READ_UNTRACKED); no row reads the pattern's keys.
export function PatternThroughSlot() {
  const [st] = createStore({ a: 1 });
  const o = { st };
  const { a } = o.st;
  return <p>{a}</p>;
}

// Negative: the pattern is over the root's own reference (SC1003's).
export function PatternOnRoot() {
  const [st] = createStore({ a: 1 });
  const { a } = st;
  return <p>{a}</p>;
}

// Item 6: a namespace member, thrown and called in the body. Chrome warns
// (STRICT_READ_UNTRACKED); the reference is the member's property.
export function NamespaceThrown() {
  let n = 0;
  try {
    throw Ns.nsCount;
  } catch (e) {
    n = (e as () => number)();
  }
  return <p>{n}</p>;
}

// Negative: the same member called in JSX, a tracked read.
export function NamespaceTracked() {
  return <p>{Ns.nsCount()}</p>;
}

// Item 10: a source not proven sync, a bare child of an element, which
// inserts and calls it (ASYNC_OUTSIDE_LOADING_BOUNDARY in Chrome).
export function PendingChild() {
  const v = createMemo(async () => 1);
  return <p>{v as unknown as string}</p>;
}

// Negative: a signal's accessor as a bare child.
export function SyncChild() {
  const [c] = createSignal(1);
  return <p>{c as unknown as string}</p>;
}

// Item 3: a props object used other than as `props.<key>`.
function Child(props: { v: () => number }) {
  const n = props["v"]();
  return <p>{n}</p>;
}
export function ComputedProp() {
  const [c] = createSignal(1);
  return <Child v={c} />;
}
