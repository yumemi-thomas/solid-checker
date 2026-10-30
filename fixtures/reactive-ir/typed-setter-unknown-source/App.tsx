import { createOptimistic, createSignal } from "solid-js";
import { useCount, useOptimistic, useVals } from "./hook";

// A setter reached through a call is found by its type alone. On rc.17 an
// optimistic setter meets no owned-scope guard, so the write is not proven;
// it was certified clean, and must be an obligation.
export function TupleHook() {
  const [, setCount] = useCount();
  setCount(1);
  return <div />;
}

export function ObjectHook() {
  const { setCount } = useVals();
  setCount(1);
  return <div />;
}

export function MemberHook() {
  const vals = useVals();
  vals.setCount(1);
  return <div />;
}

// Legal at runtime, and the same type: also an obligation, never a violation.
export function OptimisticHook() {
  const [, setOptimistic] = useOptimistic();
  setOptimistic(1);
  return <div />;
}

// Controls. A setter proven `createSignal`'s is a violation; one proven
// `createOptimistic`'s is legal and silent.
export function LocalSignal() {
  const [, setCount] = createSignal(0);
  setCount(1);
  return <div />;
}

export function LocalOptimistic() {
  const [, setOptimistic] = createOptimistic(0);
  setOptimistic(1);
  return <div />;
}
