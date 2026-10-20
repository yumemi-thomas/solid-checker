import { createMemo, untrack, createRoot } from "solid-js";

export function BodyRead() {
  const data = createMemo(async () => 1);
  const v = data();
  return <div>{v}</div>;
}
export function InlineUntrack() {
  const data = createMemo(async () => 1);
  const v = untrack(() => data());
  return <div>{v}</div>;
}
export function InlineRoot() {
  const data = createMemo(async () => 1);
  const v = createRoot(() => data());
  return <div>{v}</div>;
}
function viaHook(key: () => number) {
  const ref = () => key();
  return createRoot(ref);
}
export function HookRoot() {
  const data = createMemo(async () => 1);
  const v = viaHook(() => data());
  return <div>{v}</div>;
}
function viaHookPlain(key: () => number) {
  const ref = () => key();
  return ref();
}
export function HookPlain() {
  const data = createMemo(async () => 1);
  const v = viaHookPlain(() => data());
  return <div>{v}</div>;
}
