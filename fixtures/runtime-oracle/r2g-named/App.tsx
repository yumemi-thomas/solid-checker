import { createSignal, createMemo, untrack, createEffect } from "solid-js";

// No flow: the read is written directly in the named callback.
export function DirectNamed() {
  const data = createMemo(async () => 1);
  const parentRef = () => data();
  const m = createMemo(() => parentRef());
  const [s] = createSignal({ value: untrack(parentRef) }, { ownedWrite: true });
  createEffect(parentRef, () => undefined);
  return <div>{m() + s().value}</div>;
}

// Only the untrack use.
export function OnlyUntrack() {
  const data = createMemo(async () => 1);
  const parentRef = () => data();
  const [s] = createSignal({ value: untrack(parentRef) }, { ownedWrite: true });
  return <div>{s().value}</div>;
}

// Flow: callback handed to a hook whose named closure is used only in untrack.
function useOnlyUntrack(key: () => number) {
  const parentRef = () => key();
  return untrack(parentRef);
}
export function FlowOnlyUntrack() {
  const data = createMemo(async () => 1);
  const v = useOnlyUntrack(() => data());
  return <div>{v}</div>;
}

// Flow: named closure used in a memo and in untrack.
function useMixed(key: () => number) {
  const parentRef = () => key();
  const m = createMemo(() => parentRef());
  const v = untrack(parentRef);
  return () => m() + v;
}
export function FlowMixed() {
  const data = createMemo(async () => 1);
  const r = useMixed(() => data());
  return <div>{r()}</div>;
}
export function FlowMixedPlain() {
  const [count] = createSignal(0);
  const r = useMixed(() => count());
  return <div>{r()}</div>;
}
