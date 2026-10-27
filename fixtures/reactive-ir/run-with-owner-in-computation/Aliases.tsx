import * as Solid from "solid-js";
import { createMemo, createSignal, getOwner } from "solid-js";

const [signal] = createSignal(0);

// Positive: the namespace spelling is the same exact export.
export function ViaNamespace() {
  const owner = getOwner();
  const value = createMemo(() => Solid.runWithOwner(owner, () => signal()));
  return <span>{value()}</span>;
}

// Negative: a local function of the same name is not Solid's.
function runWithOwner<T>(_owner: unknown, fn: () => T): T {
  return fn();
}

export function Shadowed() {
  const owner = getOwner();
  const value = createMemo(() => runWithOwner(owner, () => signal()));
  return <span>{value()}</span>;
}

// A project wrapper that forwards to the primitive: the literal below is handed
// to project code, so the call does not place it.
function inOwner<T>(owner: Solid.Owner | null, fn: () => T): T {
  return Solid.runWithOwner(owner, fn);
}

export function Wrapped() {
  const owner = getOwner();
  const value = createMemo(() => inOwner(owner, () => signal()));
  return <span>{value()}</span>;
}
