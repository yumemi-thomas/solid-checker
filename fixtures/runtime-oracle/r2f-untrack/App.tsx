import { createSignal, createMemo, untrack, createEffect } from "solid-js";

function useKey(key: () => number) {
  const parentRef = () => key();
  const m = createMemo(() => parentRef());
  const [s] = createSignal({ value: untrack(parentRef) }, { ownedWrite: true });
  createEffect(parentRef, () => undefined);
  return () => m() + s().value;
}

export function Plain() {
  const [count] = createSignal(0);
  const m = useKey(() => count());
  return <div>{m()}</div>;
}

export function Async() {
  const data = createMemo(async () => 1);
  const m = useKey(() => data());
  return <div>{m()}</div>;
}

// Direct baseline: the callback itself handed to untrack in the body.
function useDirect(key: () => number) {
  return untrack(key);
}
export function DirectAsync() {
  const data = createMemo(async () => 1);
  const v = useDirect(() => data());
  return <div>{v}</div>;
}

// Body call through a named const that is also a memo compute.
function useBody(key: () => number) {
  const parentRef = () => key();
  const m = createMemo(() => parentRef());
  const first = parentRef();
  return () => m() + first;
}
export function BodyCall() {
  const [count] = createSignal(0);
  const m = useBody(() => count());
  return <div>{m()}</div>;
}
