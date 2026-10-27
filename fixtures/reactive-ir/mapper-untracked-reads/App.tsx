import { createSignal, mapArray, repeat, untrack } from "solid-js";
import * as Solid from "solid-js";

const [list] = createSignal([1, 2]);
const [tag] = createSignal("a");

// Positive: the read runs untracked in the map function and never updates.
export function Rows() {
  const rows = mapArray(list, (item) => `${item}${tag()}`);
  return <div>{rows()}</div>;
}

// Positive: with `name` the runtime also warns; the read is just as stale.
export function Named() {
  const rows = mapArray(list, (item) => `${item}${tag()}`, { name: "rows" });
  return <div>{rows()}</div>;
}

// Positive: `keyed: false` hands accessors, the map function is still untracked.
export function Unkeyed() {
  const rows = mapArray(list, (item) => `${item()}${tag()}`, { keyed: false });
  return <div>{rows()}</div>;
}

// Positive: `repeat`'s map function.
export function Counted() {
  const rows = repeat(() => 2, (index) => `${index}${tag()}`);
  return <div>{rows()}</div>;
}

// Positive: a namespace spelling is the same exact export.
export function ViaNamespace() {
  const rows = Solid.mapArray(list, (item) => `${item}${tag()}`);
  return <div>{rows()}</div>;
}

// Positive: a named map function is the callback as written.
const label = (item: number) => `${item}${tag()}`;
export function ByName() {
  const rows = mapArray(list, label);
  return <div>{rows()}</div>;
}

// Negative: JSX the map function returns tracks its own expressions.
export function Tracked() {
  const rows = mapArray(list, (item) => <b>{item}{tag()}</b>);
  return <div>{rows()}</div>;
}

// Negative: an explicit one-time snapshot.
export function Snapshot() {
  const rows = mapArray(list, (item) => `${item}${untrack(() => tag())}`);
  return <div>{rows()}</div>;
}

// Negative: the list thunk is a tracked computation.
export function Source() {
  const rows = mapArray(() => [tag()], (item) => `${item}`);
  return <div>{rows()}</div>;
}

// Negative: a function the map function only returns runs when its caller does.
export function Deferred() {
  const rows = mapArray(list, (item) => () => `${item}${tag()}`);
  return <div>{rows()[0]?.()}</div>;
}
